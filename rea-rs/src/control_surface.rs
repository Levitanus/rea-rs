use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    fmt::Debug,
    ptr::null,
    slice,
    sync::Arc,
};

use anyhow::{Error, Result};
use int_enum::IntEnum;
use rea_rs_low::{raw, IReaperControlSurface};
use serde_derive::{Deserialize, Serialize};

use crate::{ptr_wrappers::MediaTrack, Mutable, ReaRsError, Reaper, Track};

pub trait ControlSurface: Debug {
    /// simple unique string with only A-Z, 0-9, no spaces or other chars
    fn get_type_string(&self) -> String;
    /// human readable description (can include instance specific info)
    fn get_desc_string(&self) -> String;
    /// string of configuration data
    fn get_config_string(&self) -> Option<String> {
        None
    }
    /// close without sending "reset" messages, prevent "reset" being sent on
    /// destructor
    fn close_no_reset(&self) -> Result<()> {
        Ok(())
    }
    /// called ~30 times per second
    fn run(&mut self) -> Result<()> {
        Ok(())
    }
    fn set_track_list_change(&self) -> Result<()> {
        Ok(())
    }
    fn set_surface_volume(
        &self,
        _track: &mut Track<Mutable>,
        _volume: f64,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_pan(
        &self,
        _track: &mut Track<Mutable>,
        _pan: f64,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_mute(
        &self,
        _track: &mut Track<Mutable>,
        _mute: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_selected(
        &self,
        _track: &mut Track<Mutable>,
        _selected: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_solo(
        &self,
        _track: &mut Track<Mutable>,
        _solo: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_recarm(
        &self,
        _track: &mut Track<Mutable>,
        _recarm: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_play_state(
        &self,
        _play: bool,
        _pause: bool,
        _rec: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_repeat_state(&self, _rep: bool) -> Result<()> {
        Ok(())
    }
    fn set_track_title(
        &self,
        _track: &mut Track<Mutable>,
        _title: String,
    ) -> Result<()> {
        Ok(())
    }

    fn get_touch_state(
        &self,
        _track: &mut Track<Mutable>,
        _is_pan: i32,
    ) -> Result<bool> {
        Ok(false)
    }

    fn set_auto_mode(&self, _mode: i32) -> Result<()> {
        Ok(())
    }

    fn reset_cached_vol_pan_states(&self) -> Result<()> {
        Ok(())
    }

    fn on_track_selection(&self, _track: &mut Track<Mutable>) -> Result<()> {
        Ok(())
    }

    /// It's a good idea to use [keys::VKeys], but I'm afraid of having
    /// modifiers included in the keysum
    fn is_key_down(&self, _key: i32) -> Result<bool> {
        Ok(false)
    }

    /// should return false, if not supported
    fn extended(&self, _call: CSurfExtended) -> Result<bool> {
        Ok(false)
    }

    /// stop control surface and unregister it from reaper.
    fn stop(&mut self) {
        let id_string = self.get_type_string();
        if let Err(e) = Reaper::get_mut().unregister_control_surface(id_string)
        {
            Reaper::get().show_console_msg(format!(
                "Error stopping control surface: {e}"
            ));
        };
    }
}

#[derive(Debug)]
pub enum CSurfExtended<'a> {
    /// clear all surface state and reset (harder reset than
    /// SetTrackListChange)
    Reset,
    /// parm2=(int*)recmonitor
    SetInputMonitor(Track<'a, Mutable>, i32),
    SetMetronome(bool),
    SetAutoRecArm(bool),
    SetRecMode(CSurfRecMode),
    SetSendVolume {
        track: Track<'a, Mutable>,
        send_idx: usize,
        volume: f64,
    },
    SetSendPan {
        track: Track<'a, Mutable>,
        send_idx: usize,
        pan: f64,
    },
    SetFxEnabled {
        track: Track<'a, Mutable>,
        fx_idx: usize,
        enabled: bool,
    },
    SetFxParam {
        track: Track<'a, Mutable>,
        fx_idx: usize,
        param_idx: usize,
        val: f64,
    },
    SetFxParamRecfx {
        track: Track<'a, Mutable>,
        fx_idx: usize,
        param_idx: usize,
        val: f64,
    },
    SetBpmAndPlayrate {
        bpm: Option<f64>,
        playrate: Option<f64>,
    },
    /// If all are None ‒ clear touched FX
    SetLastTouchedFx {
        track: Option<Track<'a, Mutable>>,
        item_idx: Option<usize>,
        fx_idx: Option<usize>,
    },
    /// If all are None ‒ clear focused FX
    SetFocusedFx {
        track: Option<Track<'a, Mutable>>,
        item_idx: Option<usize>,
        fx_idx: Option<usize>,
    },
    SetLastTouchedTrack(Track<'a, Mutable>),
    /// Leftmost visible track in mixer
    SetMixerScroll(Track<'a, Mutable>),
    /// if a csurf supports CSURF_EXT_SETPAN_EX, it should ignore
    /// CSurf_SetSurfacePan.
    SetpanEx {
        track: Track<'a, Mutable>,
        pan: CSurfPan,
    },
    SetRecvVolume {
        track: Track<'a, Mutable>,
        recv_idx: usize,
        volume: f64,
    },
    SetRecvPan {
        track: Track<'a, Mutable>,
        recv_idx: usize,
        pan: f64,
    },
    SetFxOpen {
        track: Track<'a, Mutable>,
        fx_idx: usize,
        opened: bool,
    },
    SetFxChange {
        track: Track<'a, Mutable>,
        is_rec_fx: bool,
    },
    SetProjectMarkerChange,
    TrackFxPresetChanged {
        track: Track<'a, Mutable>,
        fx_idx: usize,
    },
    /// returns nonzero if GetTouchState can take isPan=2 for width, etc
    SupportsExtendedTouch,
    MidiDeviceRemap {
        is_out: bool,
        old_idx: i32,
        new_iox: i32,
    },
}

#[repr(i32)]
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, IntEnum, Serialize, Deserialize,
)]
pub enum CSurfRecMode {
    SplitForTakes = 0,
    /// tape
    Replace = 1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CSurfPan {
    Balance(f64),
    BalanceV4(f64),
    Stereo(f64, f64),
    Dual(f64, f64),
}

/// instantiate a track from a raw mut pointer.
/// `track_from_mut(track_pointer, project)`
macro_rules! track_from_mut {
    ($mut_ptr:ident, $project:ident) => {{
        let track_ptr =
            MediaTrack::new($mut_ptr as _).expect("null pointer to track");
        Track::<Mutable>::new(&$project, track_ptr)
    }};
}

#[derive(Debug)]
pub(crate) struct ControlSurfaceWrap {
    child: Arc<RefCell<dyn ControlSurface>>,
    type_string: Option<CString>,
    desc_string: Option<CString>,
    config_string: Option<CString>,
}
impl ControlSurfaceWrap {
    pub fn new(child: Arc<RefCell<dyn ControlSurface>>) -> Self {
        Self {
            child,
            type_string: None,
            desc_string: None,
            config_string: None,
        }
    }
    fn error(&self, error: Error) {
        let formatted = format!("Error in control surface:\n{:#?}", error);
        log::error!("{}", formatted);
        Reaper::get().show_console_msg(formatted)
    }
    fn check_for_error(&self, result: Result<()>) {
        match result {
            Ok(_) => (),
            Err(e) => self.error(e),
        }
    }
}
impl IReaperControlSurface for ControlSurfaceWrap {
    fn GetTypeString(&mut self) -> *const std::os::raw::c_char {
        println!("get_type_string");
        self.type_string = Some(
            CString::new(self.child.borrow().get_type_string())
                .expect("fail to make cstring"),
        );
        self.type_string.as_ref().unwrap().as_ptr()
    }

    fn GetDescString(&mut self) -> *const std::os::raw::c_char {
        println!("get_desc_string");
        self.desc_string = Some(
            CString::new(self.child.borrow().get_desc_string())
                .expect("fail to make cstring"),
        );
        self.desc_string.as_ref().unwrap().as_ptr()
    }

    fn GetConfigString(&mut self) -> *const std::os::raw::c_char {
        println!("get_config_string");
        match self.child.borrow().get_config_string() {
            Some(line) => {
                self.config_string =
                    Some(CString::new(line).expect("fail to make cstring"));
                self.config_string.as_ref().unwrap().as_ptr()
            }
            None => null(),
        }
    }

    fn CloseNoReset(&self) {
        self.check_for_error(self.child.borrow().close_no_reset())
    }

    fn Run(&mut self) {
        // println!("run");
        self.check_for_error(self.child.borrow_mut().run())
    }

    fn SetTrackListChange(&self) {
        self.check_for_error(self.child.borrow().set_track_list_change())
    }

    fn SetSurfaceVolume(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        volume: f64,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().set_surface_volume(&mut track, volume),
        )
    }

    fn SetSurfacePan(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        pan: f64,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().set_surface_pan(&mut track, pan),
        )
    }

    fn SetSurfaceMute(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        mute: bool,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().set_surface_mute(&mut track, mute),
        )
    }

    fn SetSurfaceSelected(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        selected: bool,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child
                .borrow()
                .set_surface_selected(&mut track, selected),
        )
    }

    fn SetSurfaceSolo(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        solo: bool,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().set_surface_solo(&mut track, solo),
        )
    }

    fn SetSurfaceRecArm(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        recarm: bool,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().set_surface_recarm(&mut track, recarm),
        )
    }

    fn SetPlayState(&self, play: bool, pause: bool, rec: bool) {
        self.check_for_error(
            self.child.borrow().set_play_state(play, pause, rec),
        )
    }

    fn SetRepeatState(&self, rep: bool) {
        self.check_for_error(self.child.borrow().set_repeat_state(rep))
    }

    fn SetTrackTitle(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        title: *const std::os::raw::c_char,
    ) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        let title = unsafe { CStr::from_ptr(title) };
        let title = match title.to_str() {
            Err(e) => return self.error(e.into()),
            Ok(s) => s.to_string(),
        };
        self.check_for_error(
            self.child.borrow().set_track_title(&mut track, title),
        )
    }

    fn GetTouchState(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        is_pan: std::os::raw::c_int,
    ) -> bool {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return false;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        match self.child.borrow().get_touch_state(&mut track, is_pan) {
            Err(e) => {
                self.error(e);
                false
            }
            Ok(r) => r,
        }
    }

    fn SetAutoMode(&self, mode: std::os::raw::c_int) {
        self.check_for_error(self.child.borrow().set_auto_mode(mode))
    }

    fn ResetCachedVolPanStates(&self) {
        self.check_for_error(self.child.borrow().reset_cached_vol_pan_states())
    }

    fn OnTrackSelection(&self, trackid: *mut rea_rs_low::raw::MediaTrack) {
        let project = Reaper::get().current_project();
        let Some(pointer) = MediaTrack::new(trackid) else {
            self.error(ReaRsError::NullPtr("track").into());
            return;
        };
        let mut track = Track::<Mutable>::new(&project, pointer);
        self.check_for_error(
            self.child.borrow().on_track_selection(&mut track),
        )
    }

    fn IsKeyDown(&self, key: std::os::raw::c_int) -> bool {
        match self.child.borrow().is_key_down(key) {
            Err(e) => {
                self.error(e);
                false
            }
            Ok(r) => r,
        }
    }

    fn Extended(
        &self,
        call: std::os::raw::c_int,
        parm1: *mut std::os::raw::c_void,
        parm2: *mut std::os::raw::c_void,
        parm3: *mut std::os::raw::c_void,
    ) -> std::os::raw::c_int {
        let project = Reaper::get().current_project();
        let call = match call {
            raw::CSURF_EXT_RESET => CSurfExtended::Reset,
            raw::CSURF_EXT_SETINPUTMONITOR => {
                let track = track_from_mut!(parm1, project);
                let monitor = unsafe { *(parm2 as *mut i32) };
                CSurfExtended::SetInputMonitor(track, monitor)
            }
            raw::CSURF_EXT_SETMETRONOME => CSurfExtended::SetMetronome(
                unsafe { *(parm1 as *mut i32) } != 0,
            ),
            raw::CSURF_EXT_SETAUTORECARM => CSurfExtended::SetAutoRecArm(
                unsafe { *(parm1 as *mut i32) } != 0,
            ),
            raw::CSURF_EXT_SETRECMODE => CSurfExtended::SetRecMode(
                match unsafe { *(parm1 as *mut i32) } {
                    0 => CSurfRecMode::SplitForTakes,
                    1 => CSurfRecMode::Replace,
                    m => {
                        self.error(
                            ReaRsError::UnexpectedAPI(format!(
                                "unknown rec mode: {m}."
                            ))
                            .into(),
                        );
                        return 0;
                    }
                },
            ),
            raw::CSURF_EXT_SETSENDVOLUME => {
                let track = track_from_mut!(parm1, project);
                CSurfExtended::SetSendVolume {
                    track,
                    send_idx: unsafe { *(parm2 as *mut i32) } as usize,
                    volume: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETSENDPAN => {
                let track = track_from_mut!(parm1, project);
                CSurfExtended::SetSendPan {
                    track,
                    send_idx: unsafe { *(parm2 as *mut i32) } as usize,
                    pan: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETFXENABLED => {
                let track = track_from_mut!(parm1, project);
                CSurfExtended::SetFxEnabled {
                    track,
                    fx_idx: unsafe { *(parm2 as *mut i32) } as usize,
                    enabled: unsafe { *(parm3 as *mut i32) } != 0,
                }
            }
            raw::CSURF_EXT_SETFXPARAM => {
                let track = track_from_mut!(parm1, project);
                let fx_idx = unsafe { *(parm2 as *mut i32) } >> 16;
                let param_idx =
                    unsafe { *(parm2 as *mut i32) } / 0b1000000000000000;
                CSurfExtended::SetFxParam {
                    track,
                    fx_idx: fx_idx as usize,
                    param_idx: param_idx as usize,
                    val: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETFXPARAM_RECFX => {
                let track = track_from_mut!(parm1, project);
                let fx_idx = unsafe { *(parm2 as *mut i32) } >> 16;
                let param_idx =
                    unsafe { *(parm2 as *mut i32) } / 0b1000000000000000;
                CSurfExtended::SetFxParamRecfx {
                    track,
                    fx_idx: fx_idx as usize,
                    param_idx: param_idx as usize,
                    val: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETBPMANDPLAYRATE => {
                let bpm = match parm1.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm1 as *mut f64) }),
                };
                let playrate = match parm2.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm2 as *mut f64) }),
                };
                CSurfExtended::SetBpmAndPlayrate { bpm, playrate }
            }
            raw::CSURF_EXT_SETLASTTOUCHEDFX => {
                let track = match parm1.is_null() {
                    true => None,
                    false => Some(track_from_mut!(parm1, project)),
                };
                let item_idx = match parm2.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm2 as *mut i32) } as usize),
                };
                let fx_idx = match parm3.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm3 as *mut i32) } as usize),
                };
                CSurfExtended::SetLastTouchedFx {
                    track,
                    item_idx,
                    fx_idx,
                }
            }
            raw::CSURF_EXT_SETFOCUSEDFX => {
                let track = match parm1.is_null() {
                    true => None,
                    false => Some(track_from_mut!(parm1, project)),
                };
                let item_idx = match parm2.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm2 as *mut i32) } as usize),
                };
                let fx_idx = match parm3.is_null() {
                    true => None,
                    false => Some(unsafe { *(parm3 as *mut i32) } as usize),
                };
                CSurfExtended::SetFocusedFx {
                    track,
                    item_idx,
                    fx_idx,
                }
            }
            raw::CSURF_EXT_SETLASTTOUCHEDTRACK => {
                CSurfExtended::SetLastTouchedTrack(track_from_mut!(
                    parm1, project
                ))
            }
            raw::CSURF_EXT_SETMIXERSCROLL => {
                CSurfExtended::SetMixerScroll(track_from_mut!(parm1, project))
            }
            raw::CSURF_EXT_SETPAN_EX => {
                let track = track_from_mut!(parm1, project);
                let mode: i32 = unsafe { *(parm3 as *mut i32) };
                match mode {
                    0 => CSurfExtended::SetpanEx {
                        track,
                        pan: CSurfPan::Balance(unsafe {
                            *(parm2 as *mut f64)
                        }),
                    },
                    3 => CSurfExtended::SetpanEx {
                        track,
                        pan: CSurfPan::BalanceV4(unsafe {
                            *(parm2 as *mut f64)
                        }),
                    },
                    5 => {
                        let pan: &mut [f64] = unsafe {
                            slice::from_raw_parts_mut(parm2 as _, 2)
                        };
                        CSurfExtended::SetpanEx {
                            track,
                            pan: CSurfPan::Stereo(pan[0], pan[1]),
                        }
                    }
                    6 => {
                        let pan: &mut [f64] = unsafe {
                            slice::from_raw_parts_mut(parm2 as _, 2)
                        };
                        CSurfExtended::SetpanEx {
                            track,
                            pan: CSurfPan::Dual(pan[0], pan[1]),
                        }
                    }
                    v => {
                        self.error(
                            ReaRsError::UnexpectedAPI(format!(
                                "unknown pan mode: {v}"
                            ))
                            .into(),
                        );
                        return 0;
                    }
                }
            }
            raw::CSURF_EXT_SETRECVVOLUME => {
                let track = track_from_mut!(parm1, project);
                CSurfExtended::SetRecvVolume {
                    track,
                    recv_idx: unsafe { *(parm2 as *mut i32) } as usize,
                    volume: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETRECVPAN => {
                let track = track_from_mut!(parm1, project);
                CSurfExtended::SetRecvPan {
                    track,
                    recv_idx: unsafe { *(parm2 as *mut i32) } as usize,
                    pan: unsafe { *(parm3 as *mut f64) },
                }
            }
            raw::CSURF_EXT_SETFXOPEN => CSurfExtended::SetFxOpen {
                track: track_from_mut!(parm1, project),
                fx_idx: unsafe { *(parm2 as *mut i32) } as usize,
                opened: unsafe { *(parm3 as *mut i32) } != 0,
            },
            raw::CSURF_EXT_SETFXCHANGE => CSurfExtended::SetFxChange {
                track: track_from_mut!(parm1, project),
                is_rec_fx: unsafe { *(parm2 as *mut i32) } == 1,
            },
            raw::CSURF_EXT_SETPROJECTMARKERCHANGE => {
                CSurfExtended::SetProjectMarkerChange
            }
            raw::CSURF_EXT_TRACKFX_PRESET_CHANGED => {
                CSurfExtended::TrackFxPresetChanged {
                    track: track_from_mut!(parm1, project),
                    fx_idx: unsafe { *(parm2 as *mut i32) } as usize,
                }
            }
            raw::CSURF_EXT_SUPPORTS_EXTENDED_TOUCH => {
                CSurfExtended::SupportsExtendedTouch
            }
            raw::CSURF_EXT_MIDI_DEVICE_REMAP => {
                CSurfExtended::MidiDeviceRemap {
                    is_out: unsafe { *(parm1 as *mut i32) } != 0,
                    old_idx: unsafe { *(parm2 as *mut i32) },
                    new_iox: unsafe { *(parm3 as *mut i32) },
                }
            }
            _ => return 0,
        };
        match self.child.borrow().extended(call) {
            Err(e) => {
                self.error(e);
                0
            }
            Ok(r) => {
                if r {
                    1
                } else {
                    0
                }
            }
        }
    }
}
