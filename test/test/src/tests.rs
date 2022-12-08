// #![allow(clippy::float_cmp)]
use approx;
use log::{debug, info};
use rea_rs::errors::{ReaperError, ReaperResult};
use rea_rs::project_info::{
    BoundsMode, RenderMode, RenderSettings, RenderTail, RenderTailFlags,
};

use crate::api::VersionRestriction::AllVersions;
use crate::api::{step, TestStep};
use bitvec::prelude::*;
use c_str_macro::c_str;
use rea_rs::{
    AutomationMode, Color, CommandId, EnvelopeChunk, ExtValue, Fx,
    GenericSend, GenericSendMut, HardwareSocket, MarkerRegionInfo,
    MessageBoxValue, Mutable, Pan, PanLaw, PlayRate, Position, Project,
    Reaper, RecInput, RecMode, RecMonitoring, RecOutMode, SampleAmount,
    SendDestChannels, SendMIDIProps, SendMode, SendSourceChannels, SoloMode,
    TimeMode, Track, TrackFolderState, TrackGroupParam, TrackPan,
    TrackPerformanceFlags, TrackPlayOffset, TrackSend, UndoFlags, VUMode,
    Volume, WithReaperPtr,
};
use std::collections::HashMap;
use std::fs::canonicalize;
use std::iter;
use std::path::Path;
use std::sync::mpsc;
use std::thread::sleep;
use std::time::Duration;

const _EPSILON: f64 = 0.000_000_1;

/// Creates all integration test steps to be executed. The order matters!
pub fn create_test_steps() -> impl Iterator<Item = TestStep> {
    // In theory all steps could be declared inline. But that makes the IDE
    // become terribly slow.
    let steps_a = vec![
        global_instances(),
        action(),
        projects(),
        misc(),
        misc_types(),
        ext_state(),
        markers(),
        tracks(),
        sends(),
    ]
    .into_iter();
    let user_interaction =
        vec![browse_for_file(), get_user_inputs(), show_message_box()]
            .into_iter();
    iter::empty() //
        .chain(steps_a) //
                        // .chain(user_interaction) //
}

fn global_instances() -> TestStep {
    step(AllVersions, "Global instances", |_, _| {
        // Sizes
        use std::mem::size_of_val;
        let medium_session = Reaper::get().medium_session();
        let medium_reaper = Reaper::get().medium();
        Reaper::get().show_console_msg(format!(
            "\
            Struct sizes in byte:\n\
            - reaper_high::Reaper: {high_reaper}\n\
            - reaper_medium::ReaperSession: {medium_session}\n\
            - reaper_medium::Reaper: {medium_reaper}\n\
            - reaper_low::Reaper: {low_reaper}\n\
            ",
            high_reaper = size_of_val(Reaper::get()),
            medium_session = size_of_val(&medium_session),
            medium_reaper = size_of_val(medium_reaper),
            low_reaper = size_of_val(medium_reaper.low()),
        ));
        // Low-level REAPER
        reaper_low::Reaper::make_available_globally(*medium_reaper.low());
        // reaper_low::Reaper::make_available_globally(*medium_reaper.low());
        let low = reaper_low::Reaper::get();
        println!("reaper_low::Reaper {:?}", &low);
        unsafe {
            low.ShowConsoleMsg(
                c_str!("- Hello from low-level API\n").as_ptr(),
            );
        }

        // Medium-level REAPER
        reaper_medium::Reaper::make_available_globally(medium_reaper.clone());
        // reaper_medium::Reaper::make_available_globally(medium_reaper.
        // clone());
        medium_reaper.show_console_msg("- Hello from medium-level API\n");
        Ok(())
    })
}

fn action() -> TestStep {
    step(AllVersions, "Actions", |_, _| {
        let rpr = Reaper::get_mut();
        let (send, receive) = mpsc::channel::<bool>();
        let action = rpr.register_action(
            "TestCommand",
            "command for test action work",
            move |_| {
                debug!("Write from Action!");
                send.send(true)?;
                Ok(())
            },
            rea_rs::ActionKind::NotToggleable,
        )?;
        debug!("Try perform action with id: {:?}", action.command_id);
        rpr.perform_action(action.command_id, 0, None);
        debug!("Try receive...");
        receive.try_recv().expect("expect receive from action call");
        assert!(receive.try_recv().is_err());

        debug!("Try perform again action with id: {:?}", action.command_id);
        rpr.perform_action(action.command_id, 0, None);
        debug!("Try receive...");
        receive.try_recv().expect("expect receive from action call");

        //

        let name = "TestCommand";
        let id = action.command_id;
        let result = rpr.get_action_name(id).expect(
            "should get action
            name",
        );
        debug!("got from id: {:?}", result);
        assert_eq!(result, name);
        let result = rpr.get_action_id(name).expect("should get action ID");
        debug!("got from name: {:?}", result);
        assert_eq!(result, id);
        Ok(())
    })
}

fn projects() -> TestStep {
    step(AllVersions, "Projects", |_, _| {
        let rpr = Reaper::get();
        // closes all projects.
        rpr.perform_action(CommandId::new(40886), 0, None);
        let current = rpr.current_project();
        let new = rpr.add_project_tab(false);
        assert!(current.is_current_project());
        assert!(!new.is_current_project());
        new.make_current_project();
        assert!(new.is_current_project());
        assert!(!current.is_current_project());
        assert_ne!(current, new);
        let projects: Vec<Project> = rpr.iter_projects().collect();
        assert_eq!(projects.len(), 2);
        assert!(projects.contains(&current));
        debug!("Just print projects: {:?}", projects);

        debug!("Try to test ptr here:");
        // let rpr = Reaper::get();
        let pr1 = rpr.current_project();
        pr1.require_valid()?;
        let mut pr2 = rpr.add_project_tab(true);
        pr2.require_valid()?;
        pr1.require_valid()?;
        assert_ne!(pr1, pr2);
        //
        pr2.with_valid_ptr(|pr| {
            debug!("{}", pr.name());
            Ok(())
        })?;
        pr1.close();
        // assert!(pr1.require_valid().is_err()); // will not compile.

        let mut pr = rpr.current_project();
        debug!("Getting render format:");
        debug!("{:?}", pr.get_render_format(false)?);
        debug!("Setting render directory…");
        pr.set_render_directory("my_directory")?;
        debug!("Getting render directory…");
        assert_eq!(
            pr.get_render_directory()?.as_path().to_str().unwrap(),
            "my_directory"
        );

        assert_eq!(pr.is_stopped(), true);
        assert_eq!(pr.is_playing(), false);
        pr.play();
        assert_eq!(pr.is_stopped(), false);
        assert_eq!(pr.is_playing(), true);
        pr.pause();
        assert_eq!(pr.is_stopped(), false);
        assert_eq!(pr.is_playing(), false);
        assert_eq!(pr.is_paused(), true);
        pr.stop();
        assert_eq!(pr.is_stopped(), true);
        assert_eq!(pr.is_playing(), false);

        // debug!("Test group index");
        // pr.set_track_group_name(0, "first group")?;
        // pr.set_track_group_name(5, "sixth group")?;
        // pr.set_track_group_name(62, "63th group")?;

        // assert_eq!(pr.get_track_group_name(62)?, "63th group");
        // assert_eq!(pr.get_track_group_name(0)?, "first group");
        // assert_eq!(pr.get_track_group_name(5)?, "sixth group");

        //
        debug!("Test Info Value");

        debug!("render bounds");
        assert_eq!(pr.get_render_bounds_mode(), BoundsMode::EntireProject);
        pr.set_render_bounds_mode(BoundsMode::SelectedItems);
        assert_eq!(pr.get_render_bounds_mode(), BoundsMode::SelectedItems);
        pr.set_render_bounds(2.0, 5.0);
        assert_eq!(
            pr.get_render_bounds(),
            (Position::from(2.0), Position::from(5.0))
        );

        debug!("render settings");
        assert_eq!(
            pr.get_render_settings(),
            RenderSettings::new(RenderMode::MasterMix, false, false)
        );
        pr.set_render_settings(RenderSettings::new(
            RenderMode::RednerMatrix,
            true,
            true,
        ));
        assert_eq!(
            pr.get_render_settings(),
            RenderSettings::new(RenderMode::RednerMatrix, true, true)
        );

        debug!("Render channels amount");
        assert_eq!(pr.get_render_channels_amount(), 2);
        pr.set_render_channels_amount(3);
        assert_eq!(pr.get_render_channels_amount(), 3);

        debug!("Sample rate");
        pr.set_srate(96000);
        assert_eq!(pr.get_srate(), Some(96000));
        pr.set_render_srate(22050);
        assert_eq!(pr.get_render_srate(), Some(22050));

        debug!("render tail");
        let tail = RenderTail::new(
            Duration::from_secs(2),
            RenderTailFlags::IN_TIME_SELECTION
                | RenderTailFlags::IN_ALL_REGIONS,
        );
        pr.set_render_tail(tail);
        assert_eq!(pr.get_render_tail(), tail);
        Ok(())
    })
}

fn browse_for_file() -> TestStep {
    step(AllVersions, "Browse for file", |_, _| {
        let rpr = Reaper::get();
        let result = rpr.browse_for_file("close this window!", "txt");
        assert_eq!(
            result
                .expect_err("should be user aborted error")
                .to_string(),
            ReaperError::UserAborted.to_string()
        );
        let result = rpr.browse_for_file("Choose Cargo.toml!", "toml")?;
        assert_eq!(
            *result,
            *canonicalize(Path::new("./Cargo.toml"))?.as_path()
        );
        Ok(())
    })
}

fn get_user_inputs() -> TestStep {
    step(AllVersions, "Get user inputs.", |_, _| {
        let rpr = Reaper::get();
        let captions =
            vec!["age(18)", "name(user)", "leave blank", "fate(atheist)"];
        let mut answers = HashMap::new();
        answers.insert(String::from("age(18)"), String::from("18"));
        answers.insert(String::from("name(user)"), String::from("user"));
        answers.insert(String::from("leave blank"), String::from(""));
        answers.insert(String::from("fate(atheist)"), String::from("atheist"));

        let result = rpr.get_user_inputs(
            "Fill values as asked in fields",
            captions,
            None,
        )?;
        assert_eq!(result, answers);
        Ok(())
    })
}

fn show_message_box() -> TestStep {
    step(AllVersions, "Get user inputs.", |_, _| {
        let rpr = Reaper::get();
        let result = rpr.show_message_box(
            "close message box",
            "please",
            rea_rs::MessageBoxType::Ok,
        )?;
        assert_eq!(result, MessageBoxValue::Ok);
        let result = rpr.show_message_box(
            "One more ask:",
            "press Retry",
            rea_rs::MessageBoxType::RetryCancel,
        )?;
        assert_eq!(result, MessageBoxValue::Retry);
        Ok(())
    })
}

fn misc() -> TestStep {
    step(AllVersions, "Misc little functions", |_, _| {
        let rpr = Reaper::get();
        debug!("Console message");
        rpr.show_console_msg("Hello from misc functions.");
        debug!("Global Automation mode");
        assert_eq!(rpr.get_global_automation_mode(), None);
        rpr.set_global_automation_mode(AutomationMode::Touch);
        assert_eq!(
            rpr.get_global_automation_mode(),
            Some(AutomationMode::Touch)
        );

        debug!("Prevent UI refresh");
        rpr.with_prevent_ui_refresh(|| {
            sleep(Duration::from_millis(100));
        });

        debug!("Add or Remove reascipts.");
        let path = Path::new("./awesome reascript.eel");
        let id = rpr.add_reascript(&path, rea_rs::Section::Main, true)?;
        rpr.perform_action(id, 0, None);
        rpr.remove_reascript(&path, rea_rs::Section::Main, true)?;

        debug!("Undo blocks does not crash REAPER");
        rpr.with_undo_block(
            "Add track and shake hand",
            UndoFlags::TRACK_FX | UndoFlags::TRACK_ITEMS,
            None,
            || -> ReaperResult<()> {
                let rpr = Reaper::get();
                rpr.show_console_msg("testing flags");
                // rpr.current_project().add_track(2, "shake hand");
                // sleep(Duration::from_millis(5_000));
                Ok(())
            },
        )?;
        let pr = rpr.current_project();
        rpr.perform_action(CommandId::new(40001), 0, Some(&pr));
        assert_eq!(pr.next_undo().expect("should have undo"), "Add new track");

        debug!("Let's print audio hardware:");
        debug!(
            "audio inputs: {:?}",
            rpr.iter_audio_inputs().collect::<Vec<HardwareSocket>>()
        );
        debug!(
            "audio outputs: {:?}",
            rpr.iter_audio_outputs().collect::<Vec<HardwareSocket>>()
        );
        debug!(
            "midi inputs: {:?}",
            rpr.iter_midi_inputs().collect::<Vec<HardwareSocket>>()
        );
        debug!(
            "midi outputs: {:?}",
            rpr.iter_midi_outputs().collect::<Vec<HardwareSocket>>()
        );

        // debug!("Get Samplerate");
        // rpr.audio_init();
        // assert!(
        //     [48000_u32,
        // 44100_u32].contains(&rpr.get_approximate_samplerate()) );
        Ok(())
    })
}

fn misc_types() -> TestStep {
    step(AllVersions, "Misc little types", |_, _| {
        let _rpr = Reaper::get();
        debug!("Color");
        let yellow = Color::new(255, 255, 0);
        let red = Color::new(255, 0, 0);
        assert_eq!(yellow, Color::new(255, 255, 0));
        assert_ne!(yellow, red);
        if cfg!(target_os = "linux") {
            assert_eq!(yellow.to_native(), 16776960);
            assert_eq!(Color::from_native(16776960), yellow);
        }

        //
        debug!("PlayRate");

        let plrt = PlayRate::from(0.25);
        debug!("from {:?}", plrt);
        assert_eq!(plrt.normalized(), 0.0);

        let plrt = PlayRate::from(4.0);
        debug!("from {:?}", plrt);
        assert_eq!(plrt.normalized(), 1.0);

        let plrt = PlayRate::from(1.0);
        debug!("from {:?}", plrt);
        assert_eq!(plrt.normalized(), 0.2);

        let plrt = PlayRate::from(2.5);
        debug!("from {:?}", plrt);
        assert_eq!(plrt.normalized(), 0.6);
        Ok(())
    })
}

fn ext_state() -> TestStep {
    step(AllVersions, "ExtState", |_, _| {
        info!("ExtState keep persistence between test sessions.");
        debug!("test on integer and in reaper");
        let rpr = Reaper::get();
        let mut state =
            ExtValue::new("test section", "first", Some(10), false, rpr);
        assert_eq!(state.get().expect("can not get value"), 10);
        state.set(56);
        assert_eq!(state.get().expect("can not get value"), 56);
        state.delete();
        assert!(state.get().is_none());
        state.set(56);

        debug!("test on struct and in reaper");
        let mut state: ExtValue<SampleAmount, Reaper> =
            ExtValue::new("test section", "second", None, false, rpr);
        assert_eq!(state.get(), None);
        state.set(SampleAmount::new(35896));
        assert_eq!(state.get().expect("can not get value").get(), 35896);
        state.delete();
        assert!(state.get().is_none());
        state.set(SampleAmount::new(35896));

        debug!("test on struct and in project");
        let mut pr = rpr.current_project();
        let mut state: ExtValue<SampleAmount, Project> =
            ExtValue::new("test section", "third", None, true, &pr);
        state.delete();
        assert!(state.get().is_none());
        state.set(SampleAmount::new(3344));

        assert_eq!(state.get().expect("can not get value").get(), 3344);
        state.delete();
        assert!(state.get().is_none());

        debug!("test on int and track");
        let tr = pr.get_track_mut(0).unwrap();
        let mut state = ExtValue::new("testsection", "first", 45, false, &tr);
        assert_eq!(state.get().expect("can not get value"), 45);
        state.set(15);
        assert_eq!(state.get().expect("can not get value"), 15);
        state.delete();
        assert_eq!(state.get(), None);

        Ok(())
    })
}

fn markers() -> TestStep {
    step(AllVersions, "Markers", |_, _| {
        let rpr = Reaper::get();
        let mut project = rpr.current_project();
        let idx1 = project.add_marker(
            Position::from(2.0),
            Some("my first marker"),
            None,
            3,
        )?;
        assert_eq!(idx1, 3);

        let idx2 = project.add_marker(
            Position::from(1.0),
            Some("my second marker"),
            None,
            2,
        )?;
        assert_eq!(idx2, 2);

        let idx3 = project.add_region(
            Position::from(1.5),
            Position::from(3.0),
            Some("my first region"),
            Color::new(0, 255, 255),
            2,
        )?;
        assert_eq!(idx3, 2);

        let all: Vec<MarkerRegionInfo> =
            project.iter_markers_and_regions().collect();
        // debug!("Here are all markers and regions:\n{:#?}", all);
        assert_eq!(all.len(), 3);
        assert!(all[1].is_region);
        assert_eq!(all[1].rgn_end, Position::from(3.0));

        let markers: Vec<MarkerRegionInfo> = project
            .iter_markers_and_regions()
            .filter(|info| !info.is_region)
            .collect();
        // debug!("Here are all markers:\n{:#?}", markers);
        assert_eq!(markers.len(), 2);

        let mut info = markers[0].clone();
        info.position = Position::from(4.0);
        project.set_marker_or_region(info)?;
        assert_eq!(
            project
                .iter_markers_and_regions()
                .find(|info| !info.is_region && info.user_index == 2)
                .unwrap()
                .position
                .as_duration()
                .as_secs_f64(),
            4.0
        );
        Ok(())
    })
}
fn tracks() -> TestStep {
    step(AllVersions, "Tracks", |_, _| {
        let rpr = Reaper::get();
        let mut pr = rpr.current_project();
        debug!("add track 'first'");
        pr.add_track(0, "first");
        assert!(Track::<Mutable>::from_name(&pr, "first").is_some());
        let tr1 = pr.get_track(0).unwrap();
        assert_eq!(tr1.name()?, "first");

        debug!("add track 'second'");
        let tr2 = pr.add_track(1, "second").index();
        let tr2 = pr.get_track(tr2).unwrap();
        assert_eq!(tr2.name()?, "second");
        assert_eq!(tr2.index(), 1);
        let tr2 = tr2.get();
        let tr2 = Track::<Mutable>::new(&mut pr, tr2);
        assert_eq!(tr2.index(), 1);

        debug!("add track 'third'");
        let mut tr3 = pr.add_track(2, "third");
        assert_eq!(tr3.name()?, "third");
        tr3.set_name("third new name")?;

        debug!("iter tracks mut");
        pr.iter_tracks_mut(|mut tr| {
            if tr.name()? != "second" {
                return Ok(());
            }
            debug!("set track {:?} name to 'new second'", tr);
            tr.set_name("new second")?;
            Ok(())
        })?;

        debug!("try to find track with new name");
        assert_eq!(pr.get_track(1).ok_or("no track!")?.name()?, "new second");

        let pos = Position::from_quarters(4.0, &pr);

        debug!("audio accessor");
        let mut tr = pr.get_track_mut(0).expect("Here should be track.");
        let aac = tr.add_audio_accessor()?;
        assert_eq!(aac.end(), 0.0.into());
        drop(aac);

        debug!("FX");
        let fx = tr
            .add_fx("ReaEQ", None, false, false)
            .expect("Can not add FX");
        assert!(fx.is_enabled());
        drop(fx);

        debug!("Item");
        let item = tr.add_item(pos, Duration::from_secs(2));
        assert!(!item.is_selected());
        let item =
            tr.add_midi_item(Position::from(2.0), Duration::from_secs(2));
        assert!(!item.is_selected());

        debug!("Sends");
        let mut send = tr.add_hardware_send();
        assert_eq!(send.is_mute(), false);
        send.set_mute(true)?;
        assert_eq!(send.is_mute(), true);
        tr.delete();

        let tr1 = pr.get_track(0).unwrap();
        let tr2 = pr.get_track(1).unwrap();
        let send = TrackSend::create_new(&tr1, &tr2);
        assert_eq!(tr1, send.source_track().expect("should return track."));
        assert_eq!(tr2, send.dest_track().expect("should return track."));
        let mut tr2 = pr.get_track_mut(1).unwrap();
        assert_eq!(tr2.index(), 1);

        assert_eq!(tr2.muted(), false);
        tr2.set_muted(true)?;
        assert_eq!(tr2.muted(), true);

        assert_eq!(tr2.phase_flipped(), false);
        tr2.set_phase_flipped(true)?;
        assert_eq!(tr2.phase_flipped(), true);

        assert_eq!(tr2.is_currently_monitored(), false);

        debug!("test solo");
        assert_eq!(tr2.solo(), SoloMode::NotSoloed);
        tr2.set_solo(SoloMode::Soloed)?;
        assert_eq!(tr2.solo(), SoloMode::Soloed);
        tr2.set_solo(SoloMode::SoloedInPlace)?;
        assert_eq!(tr2.solo(), SoloMode::SoloedInPlace);
        tr2.set_solo(SoloMode::NotSoloed)?;

        log::warn!("Can't test solo defeat.");

        assert!(!tr2.fx_bypassed());
        tr2.set_fx_bypassed(true)?;
        assert!(tr2.fx_bypassed());

        assert!(!tr2.rec_armed());
        tr2.set_rec_armed(true)?;
        assert!(tr2.rec_armed());
        tr2.set_rec_armed(false)?;

        assert_eq!(tr2.rec_input(), RecInput::Mono(0, false));
        tr2.set_rec_input(RecInput::Stereo(2, true))?;
        assert_eq!(tr2.rec_input(), RecInput::Stereo(2, true));

        assert_eq!(tr2.rec_mode(), RecMode::Input);
        tr2.set_rec_mode(RecMode::MidiOverdub)?;
        assert_eq!(tr2.rec_mode(), RecMode::MidiOverdub);
        // assert_eq!(tr2.rec_input(), RecordInput::MIDI(0, None)); Not equal!

        assert_eq!(tr2.rec_out_mode(), RecOutMode::PostFader.into());
        log::warn!("Something is wrong with RecOutMode");
        tr2.set_rec_out_mode(RecOutMode::PostFX)?;
        assert_eq!(tr2.rec_out_mode(), RecOutMode::PostFX.into());

        assert_eq!(tr2.rec_monitoring(), RecMonitoring::new(1, false));
        tr2.set_rec_monitoring(RecMonitoring::new(2, true))?;
        assert_eq!(tr2.rec_monitoring(), RecMonitoring::new(2, true));

        debug!("Auto Rec Arm");
        debug!("set selected to false");
        tr2.set_selected(false)?;
        debug!("set auto rec arm to true");
        tr2.set_auto_rec_arm(true)?;
        assert!(tr2.auto_rec_arm());
        assert!(!tr2.rec_armed());

        debug!("VUMode");
        assert_eq!(tr2.vu_mode(), VUMode::MultichannelPeaks);
        tr2.set_vu_mode(VUMode::LUFS_M)?;
        assert_eq!(tr2.vu_mode(), VUMode::LUFS_M);

        debug!("n channels");
        assert_eq!(tr2.n_channels(), 2);
        tr2.set_n_channels(6)?;
        assert_eq!(tr2.n_channels(), 6);
        tr2.set_n_channels(3)?;
        debug!("n channels will be even");
        assert_eq!(tr2.n_channels(), 4);

        debug!("set selected to true");
        assert!(!tr2.selected());
        tr2.set_selected(true)?;
        assert!(tr2.selected());

        debug!("let's see track dimensions: {:?}", tr2.dimensions());

        debug!("folder");
        let mut tr1 = pr.get_track_mut(0).unwrap();
        assert_eq!(tr1.folder_state(), TrackFolderState::Normal);
        tr1.set_folder_state(TrackFolderState::IsFolder(1))?;
        assert_eq!(tr1.folder_state(), TrackFolderState::IsFolder(1));
        tr1.set_folder_state(TrackFolderState::IsFolder(2))?;
        assert_eq!(tr1.folder_state(), TrackFolderState::IsFolder(2));
        tr1.set_folder_state(TrackFolderState::IsFolder(0))?;
        assert_eq!(tr1.folder_state(), TrackFolderState::IsFolder(0));

        let mut tr2 = pr.get_track_mut(1).unwrap();
        assert_eq!(tr2.folder_state(), TrackFolderState::Normal);

        debug!(
            "Midi hardware was tested in a live.\
        Automatically it will be too unstable."
        );

        debug!("perf flags");
        assert_eq!(tr2.performance_flags(), TrackPerformanceFlags::empty());
        tr2.set_performance_flags(TrackPerformanceFlags::NO_BUFFERING)?;
        assert_eq!(
            tr2.performance_flags(),
            TrackPerformanceFlags::NO_BUFFERING
        );
        tr2.set_performance_flags(TrackPerformanceFlags::NO_ANTICIPATIVE_FX)?;
        assert_eq!(
            tr2.performance_flags(),
            TrackPerformanceFlags::NO_ANTICIPATIVE_FX
        );

        debug!("hight override");
        assert!(tr2.height_override().is_none());
        tr2.set_height_override(200)?;
        assert_eq!(tr2.height_override(), Some(200));
        tr2.set_height_lock(true)?;
        assert!(tr2.height_lock().expect("should be true"));
        tr2.set_height_override(None)?;
        assert!(tr2.height_override().is_none());
        assert!(tr2.height_lock().is_none());

        debug!("volume");
        assert_eq!(tr2.volume(), Volume::from_db(0.0));
        tr2.set_volume(Volume::from(0.5))?;
        assert_eq!(tr2.volume().as_db().trunc(), -6.0);

        debug!("pan");
        assert_eq!(tr2.pan(), TrackPan::BalanceLegacy(0.0.into()));
        let pan = TrackPan::Stereo(Pan::from(-0.5), Pan::from(-0.2));
        tr2.set_pan(pan)?;
        assert_eq!(tr2.pan(), pan);
        let pan = TrackPan::Dual(Pan::from(1.0), Pan::from(-0.4));
        tr2.set_pan(pan)?;
        assert_eq!(tr2.pan(), pan);

        debug!("pan law");
        assert_eq!(tr2.pan_law(), PanLaw::Default);
        tr2.set_pan_law(PanLaw::Minus6dBCompensated)?;
        assert_eq!(tr2.pan_law(), PanLaw::Minus6dBCompensated);

        assert!(tr2.visible_in_mcp());
        assert!(tr2.visible_in_tcp());
        tr2.set_visible_in_mcp(false)?;
        tr2.set_visible_in_tcp(false)?;
        assert!(!tr2.visible_in_mcp());
        assert!(!tr2.visible_in_tcp());
        tr2.set_visible_in_mcp(true)?;
        tr2.set_visible_in_tcp(true)?;
        assert!(tr2.visible_in_mcp());
        assert!(tr2.visible_in_tcp());

        debug!("parent send");
        assert_eq!(tr2.parent_send(), Some(0));
        let psend = 2;
        tr2.set_parent_send(psend)?;
        assert_eq!(tr2.parent_send(), psend.into());
        let psend = 0;
        tr2.set_parent_send(psend)?;
        assert_eq!(tr2.parent_send(), psend.into());
        tr2.set_parent_send(None)?;
        assert!(tr2.parent_send().is_none());

        debug!("free positioning");
        assert_eq!(tr2.free_item_positioning(), false);
        tr2.set_free_item_positioning(true, true)?;
        assert_eq!(tr2.free_item_positioning(), true);
        tr2.set_free_item_positioning(false, true)?;
        assert_eq!(tr2.free_item_positioning(), false);

        debug!("beat attach mode");
        assert_eq!(tr2.beat_attach_mode(), TimeMode::Default);
        tr2.set_beat_attach_mode(TimeMode::BeatsFull)?;
        assert_eq!(tr2.beat_attach_mode(), TimeMode::BeatsFull);
        tr2.set_beat_attach_mode(TimeMode::BeatsOnlyPosition)?;
        assert_eq!(tr2.beat_attach_mode(), TimeMode::BeatsOnlyPosition);
        tr2.set_beat_attach_mode(TimeMode::Time)?;
        assert_eq!(tr2.beat_attach_mode(), TimeMode::Time);

        debug!(
            "Let's see scales: {:?}",
            (
                tr2.mcp_fx_param_scale(),
                tr2.mcp_fx_send_region_scale(),
                tr2.mcp_fx_send_scale(),
                tr2.tcp_fx_param_scale()
            )
        );
        tr2.set_mcp_fx_send_region_scale(0.7)?;
        assert!(approx::relative_eq!(
            tr2.mcp_fx_send_region_scale(),
            0.7,
            max_relative = 0.1
        ));

        debug!("play offset");
        assert_eq!(tr2.play_offset(), None);
        tr2.set_play_offset(Some(TrackPlayOffset::Samples(-300)))?;
        assert_eq!(tr2.play_offset(), Some(TrackPlayOffset::Samples(-300)));
        tr2.set_play_offset(Some(TrackPlayOffset::Seconds(-0.4)))?;
        assert_eq!(tr2.play_offset(), Some(TrackPlayOffset::Seconds(-0.4)));

        let mut tr = tr2.get_parent_track().expect("Should be folder track");
        assert_eq!(tr.index(), 0);

        let (mut low_u32, mut high_u32) =
            tr.group_membership(TrackGroupParam::MuteLead);
        let (low, high) = (
            low_u32.view_bits_mut::<Lsb0>(),
            high_u32.view_bits_mut::<Lsb0>(),
        );
        low.set(3, true);
        low.set(5, true);
        high.set(6, true);
        tr.set_group_membership(
            TrackGroupParam::MuteLead,
            low.load(),
            high.load(),
            None,
            None,
        );
        let (low_u32, high_u32) =
            tr.group_membership(TrackGroupParam::MuteLead);
        debug!("{:#b}, {:#b}", low_u32, high_u32);
        assert!(low_u32 & 0b1000 > 0);
        assert!(low_u32 & 0b100000 > 0);
        assert!(low_u32 & 0b1000000 == 0);
        assert!(high_u32 & 0b1000000 > 0);

        Ok(())
    })
}

fn sends() -> TestStep {
    step(AllVersions, "Sends", |_, _| {
        let rpr = Reaper::get();
        // rpr.perform_action(40886, 0, None);
        let mut pr = rpr.current_project();
        for idx in pr.n_tracks()..1 {
            let tr = pr.get_track_mut(idx - 1);
            match tr {
                None => continue,
                Some(tr) => tr.delete(),
            };
        }
        pr.add_track(0, "first");
        pr.add_track(1, "second");
        let tr1 = pr.get_track(0).unwrap();
        let tr2 = pr.get_track(1).unwrap();
        let mut send = TrackSend::create_new(&tr1, &tr2);
        assert_eq!(tr1, send.source_track().expect("should return track."));
        assert_eq!(tr2, send.dest_track().expect("should return track."));

        assert_eq!(send.automation_mode(), AutomationMode::None);
        send.set_automation_mode(AutomationMode::Touch)?;
        assert_eq!(send.automation_mode(), AutomationMode::Touch);

        assert_eq!(send.is_mute(), false);
        send.set_mute(true)?;
        assert_eq!(send.is_mute(), true);
        send.set_mute(false)?;

        assert_eq!(send.is_mono(), false);
        send.set_mono(true)?;
        assert_eq!(send.is_mono(), true);
        send.set_mono(false)?;

        assert_eq!(send.phase_flipped(), false);
        send.set_phase(true)?;
        assert_eq!(send.phase_flipped(), true);

        assert_eq!(send.volume(), Volume::from(1.0));
        send.set_volume(Volume::from_db(-20.0))?;
        assert_eq!(0.1, send.volume().into());

        assert_eq!(send.pan(), Pan::from(0.0));
        send.set_pan(-0.5)?;
        assert_eq!(send.pan().get(), -0.5);

        assert_eq!(send.pan_law(), PanLaw::Default);
        send.set_pan_law(PanLaw::Minus6dBCompensated)?;
        assert_eq!(send.pan_law(), PanLaw::Minus6dBCompensated);

        assert_eq!(send.send_mode(), SendMode::PostFader);
        send.set_send_mode(SendMode::PostFx)?;
        assert_eq!(send.send_mode(), SendMode::PostFx);
        send.set_send_mode(SendMode::PreFx)?;
        assert_eq!(send.send_mode(), SendMode::PreFx);

        let ch = SendSourceChannels::new(2, true);
        assert_eq!(
            send.source_channels(),
            SendSourceChannels::new(0, false).into()
        );
        send.set_source_channels(ch.into())?;
        assert_eq!(send.source_channels(), ch.into());
        send.set_source_channels(None)?;
        assert_eq!(send.source_channels(), None);
        send.set_source_channels(ch.into())?;

        assert_eq!(
            send.dest_channels(),
            SendDestChannels::new(0, false, false).into()
        );
        send.set_dest_channels(ch.into())?;
        assert_eq!(send.dest_channels(), SendDestChannels::from(ch).into());

        let properties = SendMIDIProps::new(2, 5, 16, 16);
        assert_eq!(
            send.midi_properties(),
            SendMIDIProps::new(0, 0, 0, 0).into()
        );
        send.set_midi_properties(properties)?;
        assert_eq!(send.midi_properties(), properties.into());
        send.set_midi_properties(None)?;
        assert_eq!(send.midi_properties(), None);

        send.get_envelope(EnvelopeChunk::Pan);
        assert_eq!(tr1.n_sends(), 1);
        assert_eq!(tr2.n_receives(), 1);

        send.delete()?;

        assert_eq!(tr1.n_sends(), 0);
        assert_eq!(tr2.n_receives(), 0);

        Ok(())
    })
}
