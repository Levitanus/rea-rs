use log::{info, trace, warn};
use rea_rs::{
    db_to_linear, linear_to_db,
    swell_gui::{
        layout::{WidgetFills, WidgetSize},
        LiceCombineMode, LiceTextOptions,
    },
    ActionHook, ActionKind, AutomationMode, CheckBox, Color, ComboBox,
    ControlEvent, ControlId, ControlSurface, EditField, ExtState, Font,
    FontSpec, LiceFont, ListBox, ProgressBar, RadioButton, Reaper,
    ReaperWindow, ScrollbarRenderer, SoloMode, Track, Trackbar, Volume,
    WindowHandler, WindowId, WindowSpec, WithReaperPtr,
};
use rea_rs_low::raw;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, sync::Arc};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";
const DOCK_STATE_KEY: &str = "rea_rs_widget_gallery.dock";
const DEMO_SECTION: &str = "rea-rs.track-gallery";
const EVENT_QUEUE_KEY: &str = "events";
const PAINT_OVER_KEY: &str = "paint_over";
const DEMO_WINDOW_ID: &str = "rea-rs.track-gallery";
const HOST_WINDOW_ID: &str = "rea-rs.track-gallery.host";
const PAINT_OVER_ACTION: &str = "TestSwellPaintOver";
const PAINT_OVER_DESCRIPTION: &str = "test swell paint-over";
const CSURF_TYPE: &str = "REARSPAINTover";
const TIMER_ID: usize = 0x5241;
const TIMER_INTERVAL_MS: u32 = 33;
const LIST_RESET_CONTENT: raw::UINT = 0x0184;

#[derive(Clone, Debug, Serialize, Deserialize)]
enum DemoEvent {
    RebuildTracks,
    Selection { guid: String },
    Volume { guid: String, value: f64 },
    Mute { guid: String, value: bool },
    Solo { guid: String, value: bool },
    Title { guid: String, value: String },
}

#[derive(Default)]
struct DemoLifecycle {
    paint_over: bool,
}

fn window_registered() -> bool {
    Reaper::get().is_window_registered(&DEMO_WINDOW_ID.to_string())
}

fn read_paint_over() -> bool {
    ExtState::<bool, Reaper>::existing(
        DEMO_SECTION,
        PAINT_OVER_KEY,
        true,
        Reaper::get(),
        None,
    )
    .get()
    .ok()
    .flatten()
    .unwrap_or(false)
}

fn ensure_surface_registered() {
    let is_window_registered = window_registered();
    let paint_over = read_paint_over();
    let active = is_window_registered || paint_over;
    let reaper = Reaper::get_mut();
    log::debug!(
        "gallery surface lifecycle: window_registered={} paint_over={} active={active} already_registered={}",
        is_window_registered,
        paint_over,
        reaper.has_control_surface(&CSURF_TYPE.to_string()),
    );
    if active {
        if !reaper.has_control_surface(&CSURF_TYPE.to_string()) {
            reaper.register_control_surface(Arc::new(RefCell::new(
                DemoCSurf::default(),
            )));
        }
    } else if reaper.has_control_surface(&CSURF_TYPE.to_string()) {
        let _ = reaper.unregister_control_surface(CSURF_TYPE.to_string());
    }
}

fn append_events(events: Vec<DemoEvent>) {
    if events.is_empty() || !window_registered() {
        return;
    }
    let result = (|| -> anyhow::Result<()> {
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let mut current: Vec<DemoEvent> = queue.get()?.unwrap_or_default();
        current.extend(events);
        queue.set(current)?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!("could not enqueue gallery event: {error}");
    }
}

#[derive(Default)]
struct DemoCSurf {
    editor_hwnd: RefCell<Option<usize>>,
    paint_over: RefCell<bool>,
    previous_host: RefCell<Option<ReaperWindow>>,
}

impl std::fmt::Debug for DemoCSurf {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter
            .debug_struct("DemoCSurf")
            .field("editor_hwnd", &self.editor_hwnd)
            .field("paint_over", &self.paint_over)
            .finish_non_exhaustive()
    }
}

impl DemoCSurf {
    fn push_track_event(
        &self,
        track: &mut Track,
        build: impl FnOnce(String) -> DemoEvent,
    ) -> anyhow::Result<()> {
        append_events(vec![build(track.guid()?.to_string())]);
        Ok(())
    }

    fn update_midi_editor_overlay(&self, editor_hwnd: Option<usize>) {
        log::debug!(
            "MIDI overlay update: editor_hwnd={editor_hwnd:?} paint_over={}",
            read_paint_over(),
        );
        self.remove_midi_editor_overlay();
        if !read_paint_over() {
            return;
        }
        let Some(hwnd) = editor_hwnd else {
            log::debug!("MIDI overlay skipped: no active editor HWND");
            return;
        };
        let hwnd = hwnd as raw::HWND;
        if !unsafe { Reaper::get().swell().IsWindow(hwnd) } {
            log::warn!("MIDI overlay skipped: invalid editor HWND={hwnd:p}");
            return;
        }
        let parent = unsafe { Reaper::get().swell().GetParent(hwnd) };
        log::debug!(
            "MIDI editor host selected: hwnd={hwnd:p} parent={parent:p}"
        );
        let Ok(window) = ReaperWindow::from_hwnd(hwnd) else {
            return;
        };
        let font = match Font::new(FontSpec::new("Arial").set_size(18))
            .and_then(LiceFont::from_font)
        {
            Ok(font) => font,
            Err(error) => {
                warn!("could not create overlay font: {error}");
                return;
            }
        };
        if let Err(error) = window.on_render(move |_info, surface| {
            let overlay_rect =
                rea_rs::swell_gui::layout::Rect::new(12, 12, 260, 42);
            surface.fill_rect(
                overlay_rect,
                Color::new(30, 120, 220),
                0.86,
                LiceCombineMode::Copy,
            );
            let _ = surface.draw_text(
                "SWELL Paint-over",
                overlay_rect,
                &font,
                LiceTextOptions::default(),
            );
            Ok(())
        }) {
            warn!("could not install MIDI editor renderer: {error}");
            return;
        }
        if let Err(error) = Reaper::get_mut()
            .register_window_handler(Box::new(DemoHostWindow { window }))
        {
            warn!("could not register MIDI editor overlay: {error}");
            return;
        }
        log::debug!(
            "MIDI overlay host handler bound: id={HOST_WINDOW_ID:?} hwnd={hwnd:p} parent={parent:p}"
        );
        *self.previous_host.borrow_mut() = ReaperWindow::from_hwnd(hwnd).ok();
        if let Some(host) = self.previous_host.borrow().as_ref() {
            if let Err(error) = host.invalidate(None) {
                warn!("could not invalidate MIDI editor for overlay: {error}");
            }
        }
    }

    fn remove_midi_editor_overlay(&self) {
        log::debug!(
            "MIDI overlay host handler unbind requested: id={HOST_WINDOW_ID:?} previous_hwnd={:?}",
            self.previous_host
                .borrow()
                .as_ref()
                .map(|window| window.hwnd() as usize),
        );
        if let Err(error) = Reaper::get_mut()
            .unregister_window_handler(&HOST_WINDOW_ID.to_string())
        {
            warn!("could not unregister MIDI editor overlay handler: {error}");
        }
        if let Some(window) = self.previous_host.borrow_mut().take() {
            let _ = window.invalidate(None);
        }
    }
}

impl ControlSurface for DemoCSurf {
    fn get_type_string(&self) -> String {
        CSURF_TYPE.to_string()
    }
    fn get_desc_string(&self) -> String {
        "rea-rs track gallery".to_string()
    }

    fn run(&self) -> anyhow::Result<()> {
        let current = Reaper::get()
            .active_midi_editor()
            .map(|editor| editor.get_pointer().as_ptr() as usize);
        let paint_over = read_paint_over();
        log::trace!(
            "DemoCSurf::run: current_editor={current:?} previous_editor={:?} paint_over={paint_over} previous_paint_over={:?}",
            *self.editor_hwnd.borrow(),
            *self.paint_over.borrow(),
        );
        let changed = current != *self.editor_hwnd.borrow()
            || paint_over != *self.paint_over.borrow();
        *self.editor_hwnd.borrow_mut() = current;
        *self.paint_over.borrow_mut() = paint_over;
        if changed {
            self.update_midi_editor_overlay(current);
        }
        Ok(())
    }

    fn set_track_list_change(&self) -> anyhow::Result<()> {
        append_events(vec![DemoEvent::RebuildTracks]);
        Ok(())
    }
    fn set_surface_volume(
        &self,
        track: &mut Track,
        value: f64,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Volume { guid, value })
    }
    fn set_surface_mute(
        &self,
        track: &mut Track,
        value: bool,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Mute { guid, value })
    }
    fn set_surface_solo(
        &self,
        track: &mut Track,
        value: bool,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Solo { guid, value })
    }
    fn set_surface_selected(
        &self,
        track: &mut Track,
        selected: bool,
    ) -> anyhow::Result<()> {
        if selected {
            self.on_track_selection(track)
        } else {
            Ok(())
        }
    }
    fn on_track_selection(&self, track: &mut Track) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Selection { guid })
    }
    fn set_track_title(
        &self,
        track: &mut Track,
        title: String,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Title {
            guid,
            value: title,
        })
    }
}

// Keep all owned MIDI overlay resources scoped to the control surface. When
// paint-over is disabled and the gallery window is closed, unregistering the
// surface drops this value and therefore detaches the MIDI host handler.
impl Drop for DemoCSurf {
    fn drop(&mut self) {
        log::debug!(
            "DemoCSurf dropping: editor_hwnd={:?} paint_over={:?}",
            self.editor_hwnd.get_mut(),
            self.paint_over.get_mut(),
        );
        self.remove_midi_editor_overlay();
    }
}

struct DemoWindow {
    window: ReaperWindow,
    dock_state: CheckBox,
    paint_checkbox: CheckBox,
    cursor_position: ProgressBar,
    tracklist: ListBox,
    name: EditField,
    normal: RadioButton,
    mute: RadioButton,
    solo: RadioButton,
    automation: ComboBox,
    volume: Trackbar,
    volume_value: EditField,
    volume_dragging: bool,
    tracks: Vec<String>,
    selected_guid: Option<String>,
    programmatic_update: bool,
    paint_over: bool,
    timer: Option<usize>,
}

impl DemoWindow {
    fn new() -> anyhow::Result<Self> {
        let (width, height) = (400_u32, 620_u32);
        let window = Reaper::get().create_window(
            &WindowSpec::new("rea-rs track gallery")
                .size(width, height)
                .dock_ident("rea_rs_widget_gallery"),
        )?;
        let ui = window.build_ui()?.central_panel();
        let content = ui.scroll_view(
            ControlId::new(),
            WidgetSize::new_fill_both(width, height),
            ScrollbarRenderer::Native,
        )?;
        let switches = content.row(
            ControlId::new(),
            WidgetSize::new_fill_x(width, 32)
                .set_min_y(32)
                .set_max_x(width),
        )?;
        let dock_state = switches.checkbox(
            ControlId(100),
            "Docked",
            WidgetSize::new_fill_x(110, 24)
                .set_fill_x(WidgetFills::FillPortion(1)),
        )?;
        let paint_checkbox = switches.checkbox(
            ControlId(101),
            "Paint over MIDI editor",
            WidgetSize::new_fill_x(220, 24)
                .set_fill_x(WidgetFills::FillPortion(2)),
        )?;
        let cursor_position = content.progress_bar(
            ControlId(102),
            WidgetSize::new_fill_x(width, 20).set_max_x(width),
            0,
        )?;
        cursor_position.set_range(0, 1000)?;
        let tracklist = content.list_box(
            ControlId(103),
            WidgetSize::new_fill_both(width, 100)
                .set_min_y(90)
                .set_max_x(width),
            0,
        )?;
        let inspector = content.group_box(
            ControlId(104),
            "Track inspector",
            WidgetSize::new_fill_both(width, 100).set_max_x(width),
        )?;
        let name = inspector.edit_field(
            ControlId(105),
            WidgetSize::new_fill_x(380, 26).set_max_x(width),
            0,
        )?;
        let states = inspector.row(
            ControlId::new(),
            WidgetSize::new_fill_x(380, 28).set_max_x(width),
        )?;
        let normal = states.radio_button(
            ControlId(106),
            "Normal",
            WidgetSize::new(95, 24),
            0,
        )?;
        let mute = states.radio_button(
            ControlId(107),
            "Mute",
            WidgetSize::new(85, 24),
            0,
        )?;
        let solo = states.radio_button(
            ControlId(108),
            "Solo",
            WidgetSize::new(85, 24),
            0,
        )?;
        let automation = inspector.combo_box(
            ControlId(109),
            WidgetSize::new_fill_x(380, 28).set_max_x(width),
            0,
        )?;
        for mode in [
            AutomationMode::None,
            AutomationMode::TrimRead,
            AutomationMode::Read,
            AutomationMode::Touch,
            AutomationMode::Write,
            AutomationMode::Latch,
            AutomationMode::Bypass,
        ] {
            automation.add_item(automation_name(mode))?;
        }
        let volume_row = inspector.row(
            ControlId::new(),
            WidgetSize::new_fill_x(380, 32).set_max_x(width),
        )?;
        let volume = volume_row.trackbar(
            ControlId(110),
            WidgetSize::new_fill_x(220, 28).set_max_x(width),
            0,
        )?;
        volume.set_range(0, 1000)?;
        volume.set_position(
            (Reaper::get().db_to_slider(0.0) * 1000.0)
                .round()
                .clamp(0.0, 1000.0) as i32,
        )?;
        let volume_value = volume_row.edit_field(
            ControlId(111),
            WidgetSize::new(90, 28),
            0,
        )?;
        volume_row.label(ControlId(112), "dB", WidgetSize::new(25, 28))?;
        let docked = ExtState::<bool, Reaper>::existing(
            WINDOW_STATE_SECTION,
            DOCK_STATE_KEY,
            true,
            Reaper::get(),
            None,
        )
        .get()?
        .unwrap_or(false);
        dock_state.set_checked(docked)?;
        let paint_over = read_paint_over();
        paint_checkbox.set_checked(paint_over)?;
        if docked {
            window.dock(
                "rea-rs track gallery",
                "rea_rs_widget_gallery",
                true,
            )?;
        }
        Ok(Self {
            window,
            dock_state,
            paint_checkbox,
            cursor_position,
            tracklist,
            name,
            normal,
            mute,
            solo,
            automation,
            volume,
            volume_value,
            volume_dragging: false,
            tracks: Vec::new(),
            selected_guid: None,
            programmatic_update: false,
            paint_over,
            timer: None,
        })
    }

    fn save_dock_state(&self, docked: bool) {
        let mut state = ExtState::<bool, Reaper>::existing(
            WINDOW_STATE_SECTION,
            DOCK_STATE_KEY,
            true,
            Reaper::get(),
            None,
        );
        if let Err(error) = state.set(docked) {
            warn!("could not save dock preference: {error}");
        }
    }

    fn refresh_play_position(&self) {
        let project = Reaper::get().current_project();
        let ratio = match (project.play_position(), project.length()) {
            (Ok(position), Ok(length)) if length.as_secs_f64() > 0.0 => {
                let position_seconds: f64 = position.into();
                (position_seconds / length.as_secs_f64()).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let _ = self
            .cursor_position
            .set_position((ratio * 1000.0).round() as i32);
    }

    fn rebuild_tracks(&mut self) {
        let project = Reaper::get().current_project();
        let old_guid = self.selected_track_guid();
        self.tracklist
            .control()
            .send_message(LIST_RESET_CONTENT, 0, 0)
            .ok();
        self.tracks.clear();
        for track in project.iter_tracks() {
            if let (Ok(name), Ok(guid)) = (track.name(), track.guid()) {
                let _ = self.tracklist.add_item(&name);
                self.tracks.push(guid.to_string());
            }
        }
        let selected = old_guid.and_then(|guid| {
            self.tracks.iter().position(|value| value == &guid)
        });
        if let Some(index) = selected {
            let _ = self.tracklist.select(index as i32);
        }
        self.selected_guid = selected.map(|index| self.tracks[index].clone());
        self.sync_inspector();
    }

    fn selected_track_guid(&self) -> Option<String> {
        let project = Reaper::get().current_project();
        project
            .iter_selected_tracks()
            .next()
            .and_then(|track| track.guid().ok().map(|guid| guid.to_string()))
    }

    fn with_selected_track(&self, f: impl FnOnce(&mut Track)) {
        let Some(guid) = self.selected_guid.as_deref() else {
            return;
        };
        let project = Reaper::get().current_project();
        if let Some(mut track) = project.iter_tracks().find(|track| {
            track.guid().ok().map(|id| id.to_string()).as_deref() == Some(guid)
        }) {
            f(&mut track);
        }
    }

    fn sync_inspector(&mut self) {
        self.programmatic_update = true;
        let selected = self.selected_guid.clone();
        let track = selected.as_deref().and_then(|guid| {
            Reaper::get().current_project().iter_tracks().find(|track| {
                track.guid().ok().map(|id| id.to_string()).as_deref()
                    == Some(guid)
            })
        });
        if let Some(track) = track {
            self.name.set_text(&track.name().unwrap_or_default()).ok();
            let solo_state = track.solo().unwrap_or(SoloMode::NotSoloed);
            let muted = track.muted().unwrap_or(false);
            self.solo
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state != SoloMode::NotSoloed {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            self.mute
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state == SoloMode::NotSoloed && muted {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            self.normal
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state == SoloMode::NotSoloed && !muted {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            let mode =
                track.get_automation_mode().unwrap_or(AutomationMode::None);
            self.automation.select(automation_index(mode)).ok();
            let linear =
                track.volume().map(|value| value.get()).unwrap_or(1.0);
            let db = linear_to_db(linear);
            self.volume_value.set_text(&format_db(db)).ok();
            let slider = Reaper::get().db_to_slider(db);
            trace!(
                "Track volume: linear={:?}, db={:?}, slider={:?}",
                linear,
                db,
                slider
            );
            let position = slider.round().clamp(0.0, 1000.0) as i32;
            self.volume.set_position(position).ok();
            self.set_inspector_enabled(true);
        } else {
            self.name.set_text("").ok();
            self.volume_value.set_text("").ok();
            self.normal
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.mute
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.solo
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.automation.select(-1).ok();
            self.volume.set_position(0).ok();
            self.set_inspector_enabled(false);
        }
        self.programmatic_update = false;
    }

    fn volume_is_being_dragged(&self) -> bool {
        self.volume_dragging
            || Reaper::get().swell().GetCapture() == self.volume.hwnd()
    }

    fn sync_volume(&mut self, linear: f64, update_slider: bool) {
        let db = linear_to_db(linear);
        self.programmatic_update = true;
        self.volume_value.set_text(&format_db(db)).ok();
        if update_slider {
            let position =
                Reaper::get().db_to_slider(db).round().clamp(0.0, 1000.0)
                    as i32;
            self.volume.set_position(position).ok();
        }
        self.programmatic_update = false;
    }

    fn set_inspector_enabled(&self, enabled: bool) {
        let _ = self.name.enable(enabled);
        let _ = self.normal.enable(enabled);
        let _ = self.mute.enable(enabled);
        let _ = self.solo.enable(enabled);
        let _ = self.automation.enable(enabled);
        let _ = self.volume.enable(enabled);
        let _ = self.volume_value.enable(enabled);
    }

    fn set_track_selection(&mut self, guid: Option<String>) {
        self.selected_guid = guid;
        let project = Reaper::get().current_project();
        for mut track in project.iter_tracks() {
            let selected = track.guid().ok().map(|id| id.to_string())
                == self.selected_guid;
            let _ = track.set_selected(selected);
        }
        let index = self.selected_guid.as_ref().and_then(|guid| {
            self.tracks.iter().position(|value| value == guid)
        });
        self.tracklist
            .select(index.map_or(-1, |index| index as i32))
            .ok();
        self.sync_inspector();
        if let Some(selected_track) = Reaper::get()
            .current_project()
            .iter_tracks()
            .find(|track| track.selected().unwrap_or(false))
        {
            if let Ok(pointer) = selected_track.get() {
                unsafe {
                    Reaper::get()
                        .low()
                        .CSurf_OnTrackSelection(pointer.as_ptr());
                }
            }
        }
    }

    fn drain_events(&mut self) {
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let events = queue.get().ok().flatten().unwrap_or_default();
        for event in events {
            match event {
                DemoEvent::RebuildTracks => self.rebuild_tracks(),
                DemoEvent::Selection { guid } => {
                    if self.tracks.iter().any(|value| value == &guid) {
                        self.selected_guid = Some(guid);
                    }
                    self.sync_list_selection();
                    self.sync_inspector();
                }
                DemoEvent::Volume { guid, value } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.sync_volume(
                            value,
                            !self.volume_is_being_dragged(),
                        );
                    }
                }
                DemoEvent::Mute { guid, value: _ }
                | DemoEvent::Solo { guid, value: _ } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.sync_inspector();
                    }
                }
                DemoEvent::Title { guid, value } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.programmatic_update = true;
                        self.name.set_text(&value).ok();
                        self.programmatic_update = false;
                    }
                    self.rebuild_tracks();
                }
            }
        }
        if let Err(error) = queue.set(Vec::<DemoEvent>::new()) {
            warn!("could not clear gallery event queue: {error}");
        }
    }

    fn sync_list_selection(&mut self) {
        let index = self.selected_guid.as_ref().and_then(|guid| {
            self.tracks.iter().position(|value| value == guid)
        });
        self.tracklist
            .select(index.map_or(-1, |index| index as i32))
            .ok();
    }

    fn set_paint_over(&mut self) {
        // A native close request unregisters the window handler before
        // destruction; don't interpret any late control notification as a
        // user click. Programmatic checkbox synchronization is filtered by
        // `on_control_event` before this method is reached.
        if !window_registered() {
            return;
        }
        let command_name = format!(
            "{}_section_{}",
            PAINT_OVER_ACTION,
            rea_rs::Section::Main.id()
        );
        match Reaper::get().get_action_id(command_name) {
            Ok(Some(command_id)) => {
                Reaper::get().perform_action(command_id, 0, None);
            }
            Ok(None) => warn!("paint-over action is not registered in REAPER"),
            Err(error) => {
                warn!("could not look up paint-over action: {error}")
            }
        }
        self.paint_over = read_paint_over();
        let _ = self.paint_checkbox.set_checked(self.paint_over);
        ensure_surface_registered();
    }
}

impl WindowHandler for DemoWindow {
    fn window_id(&self) -> WindowId {
        DEMO_WINDOW_ID.to_string()
    }
    fn window(&self) -> &ReaperWindow {
        &self.window
    }
    fn on_open(&mut self) {
        self.timer = self.window.start_timer(TIMER_ID, TIMER_INTERVAL_MS).ok();
        self.rebuild_tracks();
        self.refresh_play_position();
        ensure_surface_registered();
    }
    fn on_timer(&mut self, id: usize) {
        if id != TIMER_ID {
            return;
        }
        self.drain_events();
        self.refresh_play_position();
        let paint_over = read_paint_over();
        if self.paint_over != paint_over {
            self.paint_over = paint_over;
            let _ = self.paint_checkbox.set_checked(paint_over);
            ensure_surface_registered();
        }
    }
    fn on_destroy(&mut self) {
        if let Some(timer) = self.timer.take() {
            let _ = self.window.stop_timer(timer);
        }
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let _ = queue.set(Vec::<DemoEvent>::new());
        ensure_surface_registered();
    }
    fn on_control_event(&mut self, event: ControlEvent) {
        if self.programmatic_update {
            return;
        }
        match event {
            ControlEvent::CheckBoxChanged { control }
                if control == self.dock_state.id() =>
            {
                let docked = self.dock_state.checked().unwrap_or(false);
                let result = if docked {
                    self.window.dock(
                        "rea-rs track gallery",
                        "rea_rs_widget_gallery",
                        true,
                    )
                } else {
                    self.window.float()
                };
                if result.is_ok() {
                    self.save_dock_state(docked);
                }
            }
            ControlEvent::CheckBoxChanged { control }
                if control == self.paint_checkbox.id() =>
            {
                self.set_paint_over()
            }
            ControlEvent::ListSelectionChanged { control }
                if control == self.tracklist.id() =>
            {
                let index = self.tracklist.selected_index().unwrap_or(-1);
                self.set_track_selection(
                    usize::try_from(index)
                        .ok()
                        .and_then(|index| self.tracks.get(index).cloned()),
                );
            }
            ControlEvent::EditChanged { control }
                if control == self.name.id() =>
            {
                let value = self.name.text().unwrap_or_default();
                self.with_selected_track(|track| {
                    let _ = track.set_name(value);
                });
            }
            ControlEvent::RadioButtonChanged { control } => {
                self.with_selected_track(|track| {
                    if control == self.normal.id() {
                        let _ = track.set_solo(SoloMode::NotSoloed);
                        let _ = track.set_muted(false);
                    } else if control == self.mute.id() {
                        let _ = track.set_solo(SoloMode::NotSoloed);
                        let _ = track.set_muted(true);
                    } else if control == self.solo.id() {
                        let _ = track.set_muted(false);
                        let _ = track.set_solo(SoloMode::Soloed);
                    }
                });
                self.sync_inspector();
            }
            ControlEvent::ComboSelectionChanged { control }
                if control == self.automation.id() =>
            {
                let index = self.automation.selected_index().unwrap_or(-1);
                if let Some(mode) = automation_from_index(index) {
                    self.with_selected_track(|track| {
                        let _ = track.set_automation_mode(mode);
                    });
                }
            }
            ControlEvent::TrackbarChanged { control }
                if control == self.volume.id() =>
            {
                let position =
                    self.volume.position().unwrap_or(0).clamp(0, 1000) as f64;
                let linear =
                    db_to_linear(Reaper::get().slider_to_db(position));
                self.with_selected_track(|track| {
                    let _ = track.set_volume(Volume::from(linear));
                });
                self.sync_volume(linear, false);
            }
            ControlEvent::EditChanged { control }
                if control == self.volume_value.id() =>
            {
                if let Ok(db) = self
                    .volume_value
                    .text()
                    .unwrap_or_default()
                    .trim()
                    .parse::<f64>()
                {
                    if !db.is_finite() {
                        return;
                    }
                    let linear = db_to_linear(db);
                    self.with_selected_track(|track| {
                        let _ = track.set_volume(Volume::from(linear));
                    });
                    let slider = Reaper::get().db_to_slider(db);
                    self.programmatic_update = true;
                    self.volume
                        .set_position(slider.round().clamp(0.0, 1000.0) as i32)
                        .ok();
                    self.programmatic_update = false;
                }
            }
            _ => {}
        }
    }

    fn on_widget_event(
        &mut self,
        id: ControlId,
        event: rea_rs::WindowEvent,
    ) -> bool {
        if id == self.volume.id() {
            if let rea_rs::WindowEvent::Mouse { message, .. } = event {
                match message {
                    rea_rs::swell_gui::MouseMessage::Down(
                        rea_rs::swell_gui::MouseButton::Left,
                    ) => self.volume_dragging = true,
                    rea_rs::swell_gui::MouseMessage::Up(
                        rea_rs::swell_gui::MouseButton::Left,
                    ) => {
                        self.volume_dragging = false;
                        self.sync_inspector();
                    }
                    _ => {}
                }
            }
        }
        false
    }
}

struct DemoHostWindow {
    window: ReaperWindow,
}
impl WindowHandler for DemoHostWindow {
    fn window_id(&self) -> WindowId {
        log::trace!(
            "DemoHostWindow::window_id queried: id={HOST_WINDOW_ID:?} hwnd={:p}",
            self.window.hwnd(),
        );
        HOST_WINDOW_ID.to_string()
    }
    fn window(&self) -> &ReaperWindow {
        &self.window
    }
    fn handle_host_message(&self, message: u32) -> bool {
        let enabled = message == raw::WM_PAINT;
        if message == raw::WM_PAINT {
            log::trace!(
                "DemoHostWindow accepts WM_PAINT: id={HOST_WINDOW_ID:?} hwnd={:p}",
                self.window.hwnd(),
            );
        }
        enabled
    }
    fn on_destroy(&mut self) {
        log::debug!(
            "DemoHostWindow destroyed: id={HOST_WINDOW_ID:?} hwnd={:p} parent={:p}",
            self.window.hwnd(),
            unsafe { Reaper::get().swell().GetParent(self.window.hwnd()) },
        );
        // The borrowed host HWND has its original WndProc restored before
        // this callback. Drop the custom renderer and request a clean paint
        // through that original procedure.
        let _ = self.window.clear_render();
    }
}

fn automation_name(mode: AutomationMode) -> &'static str {
    match mode {
        AutomationMode::None => "None",
        AutomationMode::TrimRead => "TrimRead",
        AutomationMode::Read => "Read",
        AutomationMode::Touch => "Touch",
        AutomationMode::Write => "Write",
        AutomationMode::Latch => "Latch",
        AutomationMode::Bypass => "Bypass",
    }
}
fn automation_index(mode: AutomationMode) -> i32 {
    match mode {
        AutomationMode::None => 0,
        AutomationMode::TrimRead => 1,
        AutomationMode::Read => 2,
        AutomationMode::Touch => 3,
        AutomationMode::Write => 4,
        AutomationMode::Latch => 5,
        AutomationMode::Bypass => 6,
    }
}
fn automation_from_index(index: i32) -> Option<AutomationMode> {
    Some(match index {
        0 => AutomationMode::None,
        1 => AutomationMode::TrimRead,
        2 => AutomationMode::Read,
        3 => AutomationMode::Touch,
        4 => AutomationMode::Write,
        5 => AutomationMode::Latch,
        6 => AutomationMode::Bypass,
        _ => return None,
    })
}
fn format_db(db: f64) -> String {
    if db.is_finite() {
        format!("{db:.2}")
    } else {
        "-inf".to_string()
    }
}

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    let _queue = ExtState::<Vec<DemoEvent>, Reaper>::new(
        DEMO_SECTION,
        EVENT_QUEUE_KEY,
        Vec::new(),
        false,
        Reaper::get(),
        None,
    )?;
    let paint_over = ExtState::<bool, Reaper>::existing(
        DEMO_SECTION,
        PAINT_OVER_KEY,
        true,
        Reaper::get(),
        None,
    )
    .get()?
    .unwrap_or(false);
    let lifecycle = Arc::new(RefCell::new(DemoLifecycle {
        paint_over,
        ..Default::default()
    }));
    if paint_over {
        reaper.register_control_surface(Arc::new(RefCell::new(
            DemoCSurf::default(),
        )));
    }
    let toggle_state = lifecycle.clone();
    reaper.register_action(
        PAINT_OVER_ACTION,
        PAINT_OVER_DESCRIPTION,
        ActionKind::Toggleable(paint_over),
        move |hook: &mut ActionHook| {
            let current = read_paint_over();
            let next = !current;
            hook.set_toggle_state(next);
            toggle_state.borrow_mut().paint_over = next;
            let mut state = ExtState::<bool, Reaper>::existing(
                DEMO_SECTION,
                PAINT_OVER_KEY,
                true,
                Reaper::get(),
                None,
            );
            state.set(next)?;
            ensure_surface_registered();
            Ok(())
        },
        None,
    )?;
    reaper.register_action(
        "TestSwellGuiWidgetGallery",
        "test swell-gui widget gallery",
        ActionKind::NotToggleable,
        |_| {
            if window_registered() {
                Reaper::get_mut()
                    .unregister_window_handler(&DEMO_WINDOW_ID.to_string())?;
            } else {
                let demo_window = DemoWindow::new()?;
                Reaper::get_mut()
                    .register_window_handler(Box::new(demo_window))?;
                ensure_surface_registered();
            }
            info!("rea-rs track gallery toggled");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
