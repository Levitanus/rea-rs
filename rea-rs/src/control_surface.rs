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

use crate::{ptr_wrappers::MediaTrack, ReaRsError, Reaper, Track};

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
    fn run(&self) -> Result<()> {
        Ok(())
    }
    fn set_track_list_change(&self) -> Result<()> {
        Ok(())
    }
    fn set_surface_volume(
        &self,
        _track: &mut Track,
        _volume: f64,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_pan(&self, _track: &mut Track, _pan: f64) -> Result<()> {
        Ok(())
    }
    fn set_surface_mute(&self, _track: &mut Track, _mute: bool) -> Result<()> {
        Ok(())
    }
    fn set_surface_selected(
        &self,
        _track: &mut Track,
        _selected: bool,
    ) -> Result<()> {
        Ok(())
    }
    fn set_surface_solo(&self, _track: &mut Track, _solo: bool) -> Result<()> {
        Ok(())
    }
    fn set_surface_recarm(
        &self,
        _track: &mut Track,
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
        _track: &mut Track,
        _title: String,
    ) -> Result<()> {
        Ok(())
    }

    fn get_touch_state(
        &self,
        _track: &mut Track,
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

    fn on_track_selection(&self, _track: &mut Track) -> Result<()> {
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
pub enum CSurfExtended {
    /// clear all surface state and reset (harder reset than
    /// SetTrackListChange)
    Reset,
    /// parm2=(int*)recmonitor
    SetInputMonitor(Track, i32),
    SetMetronome(bool),
    SetAutoRecArm(bool),
    SetRecMode(CSurfRecMode),
    SetSendVolume {
        track: Track,
        send_idx: usize,
        volume: f64,
    },
    SetSendPan {
        track: Track,
        send_idx: usize,
        pan: f64,
    },
    SetFxEnabled {
        track: Track,
        fx_idx: usize,
        enabled: bool,
    },
    SetFxParam {
        track: Track,
        fx_idx: usize,
        param_idx: usize,
        val: f64,
    },
    SetFxParamRecfx {
        track: Track,
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
        track: Option<Track>,
        item_idx: Option<usize>,
        fx_idx: Option<usize>,
    },
    /// If all are None ‒ clear focused FX
    SetFocusedFx {
        track: Option<Track>,
        item_idx: Option<usize>,
        fx_idx: Option<usize>,
    },
    SetLastTouchedTrack(Track),
    /// Leftmost visible track in mixer
    SetMixerScroll(Track),
    /// if a csurf supports CSURF_EXT_SETPAN_EX, it should ignore
    /// CSurf_SetSurfacePan.
    SetpanEx {
        track: Track,
        pan: CSurfPan,
    },
    SetRecvVolume {
        track: Track,
        recv_idx: usize,
        volume: f64,
    },
    SetRecvPan {
        track: Track,
        recv_idx: usize,
        pan: f64,
    },
    SetFxOpen {
        track: Track,
        fx_idx: usize,
        opened: bool,
    },
    SetFxChange {
        track: Track,
        is_rec_fx: bool,
    },
    SetProjectMarkerChange,
    TrackFxPresetChanged {
        track: Track,
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

#[derive(Debug)]
pub(crate) struct ControlSurfaceWrap {
    child: Arc<RefCell<dyn ControlSurface>>,
    // Cache static CStrings since they rarely change
    // This avoids memory leaks while ensuring pointer validity
    type_string_cache: Option<CString>,
    desc_string_cache: Option<CString>,
    config_string_cache: Option<CString>,
    // Store the last values to detect when we need to regenerate
    last_type_string: Option<String>,
    last_desc_string: Option<String>,
    last_config_string: Option<String>,
}
impl ControlSurfaceWrap {
    pub fn new(child: Arc<RefCell<dyn ControlSurface>>) -> Self {
        Self {
            child,
            type_string_cache: None,
            desc_string_cache: None,
            config_string_cache: None,
            last_type_string: None,
            last_desc_string: None,
            last_config_string: None,
        }
    }
    
    // Helper function to get or create a cached CString
    // Since these are typically static, we cache them to avoid memory leaks
    fn get_cached_cstring(cache: &mut Option<CString>, last_value: &mut Option<String>, new_value: String) -> *const std::os::raw::c_char {
        // Check if we need to regenerate the CString
        if last_value.as_ref() != Some(&new_value) {
            // Create new CString and update cache
            match CString::new(new_value.clone()) {
                Ok(cstring) => {
                    *cache = Some(cstring);
                    *last_value = Some(new_value);
                }
                Err(_) => {
                    return std::ptr::null();
                }
            }
        }
        
        // Return pointer to cached CString
        cache.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null())
    }
    fn error(&self, error: Error) {
        let formatted = format!("Error in control surface:\n{:#?}", error);
        log::error!("{:?}", error);
        Reaper::get().show_console_msg(formatted)
    }
    
    fn check_for_error(&self, result: Result<()>) {
        match result {
            Ok(_) => (),
            Err(e) => self.error(e),
        }
    }
    
    // Helper function for safe pointer dereferencing with null checks
    fn safe_deref_i32(&self, ptr: *mut std::os::raw::c_void) -> Option<i32> {
        if ptr.is_null() {
            self.error(ReaRsError::UnexpectedAPI("Null pointer dereference attempted".to_string()).into());
            None
        } else {
            Some(unsafe { *(ptr as *mut i32) })
        }
    }
    
    // Helper function for safe pointer dereferencing with null checks
    fn safe_deref_f64(&self, ptr: *mut std::os::raw::c_void) -> Option<f64> {
        if ptr.is_null() {
            self.error(ReaRsError::UnexpectedAPI("Null pointer dereference attempted".to_string()).into());
            None
        } else {
            Some(unsafe { *(ptr as *mut f64) })
        }
    }
    
    // Helper function for safe slice creation with bounds checking
    fn safe_slice_f64(&self, ptr: *mut std::os::raw::c_void, len: usize) -> Option<&mut [f64]> {
        if ptr.is_null() {
            self.error(ReaRsError::UnexpectedAPI("Null pointer dereference attempted".to_string()).into());
            None
        } else {
            Some(unsafe { slice::from_raw_parts_mut(ptr as *mut f64, len) })
        }
    }
    fn track_from_mut(
        &self,
        track_mut: *mut rea_rs_low::raw::MediaTrack,
        function_name: &'static str,
    ) -> Option<Track> {
        match MediaTrack::new(track_mut) {
            None => {
                log::warn!(
                    "null track pointer from function {}",
                    function_name,
                );
                None
            }
            Some(ptr) => Some(Track::new(None, ptr)),
        }
    }
}
impl IReaperControlSurface for ControlSurfaceWrap {
    fn GetTypeString(&mut self) -> *const std::os::raw::c_char {
        println!("get_type_string");
        let new_value = self.child.borrow().get_type_string();
        Self::get_cached_cstring(&mut self.type_string_cache, &mut self.last_type_string, new_value)
    }

    fn GetDescString(&mut self) -> *const std::os::raw::c_char {
        println!("get_desc_string");
        let new_value = self.child.borrow().get_desc_string();
        Self::get_cached_cstring(&mut self.desc_string_cache, &mut self.last_desc_string, new_value)
    }

    fn GetConfigString(&mut self) -> *const std::os::raw::c_char {
        println!("get_config_string");
        let new_value = self.child.borrow().get_config_string();
        match new_value {
            Some(line) => Self::get_cached_cstring(&mut self.config_string_cache, &mut self.last_config_string, line),
            None => null(),
        }
    }

    fn CloseNoReset(&self) {
        self.check_for_error(self.child.borrow().close_no_reset())
    }

    fn Run(&mut self) {
        // println!("run");
        self.check_for_error(self.child.borrow().run())
    }

    fn SetTrackListChange(&self) {
        self.check_for_error(self.child.borrow().set_track_list_change())
    }

    fn SetSurfaceVolume(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        volume: f64,
    ) {
        if let Some(mut track) =
            self.track_from_mut(trackid, "SetSurfaceVolume")
        {
            self.check_for_error(
                self.child.borrow().set_surface_volume(&mut track, volume),
            )
        }
    }

    fn SetSurfacePan(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        pan: f64,
    ) {
        if let Some(mut track) = self.track_from_mut(trackid, "SetSurfacePan")
        {
            self.check_for_error(
                self.child.borrow().set_surface_pan(&mut track, pan),
            )
        }
    }

    fn SetSurfaceMute(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        mute: bool,
    ) {
        if let Some(mut track) = self.track_from_mut(trackid, "SetSurfaceMute")
        {
            self.check_for_error(
                self.child.borrow().set_surface_mute(&mut track, mute),
            )
        }
    }

    fn SetSurfaceSelected(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        selected: bool,
    ) {
        if let Some(mut track) =
            self.track_from_mut(trackid, "SetSurfaceSelected")
        {
            self.check_for_error(
                self.child
                    .borrow()
                    .set_surface_selected(&mut track, selected),
            )
        }
    }

    fn SetSurfaceSolo(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        solo: bool,
    ) {
        if let Some(mut track) = self.track_from_mut(trackid, "SetSurfaceSolo")
        {
            self.check_for_error(
                self.child.borrow().set_surface_solo(&mut track, solo),
            )
        }
    }

    fn SetSurfaceRecArm(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        recarm: bool,
    ) {
        if let Some(mut track) =
            self.track_from_mut(trackid, "SetSurfaceRecArm")
        {
            self.check_for_error(
                self.child.borrow().set_surface_recarm(&mut track, recarm),
            )
        }
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
        if let Some(mut track) = self.track_from_mut(trackid, "SetTrackTitle")
        {
            let title = unsafe { CStr::from_ptr(title) };
            let title = match title.to_str() {
                Err(e) => return self.error(e.into()),
                Ok(s) => s.to_string(),
            };
            self.check_for_error(
                self.child.borrow().set_track_title(&mut track, title),
            )
        }
    }

    fn GetTouchState(
        &self,
        trackid: *mut rea_rs_low::raw::MediaTrack,
        is_pan: std::os::raw::c_int,
    ) -> bool {
        let Some(mut track) = self.track_from_mut(trackid, "GetTouchState")
        else {
            return false;
        };
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
        if let Some(mut track) =
            self.track_from_mut(trackid, "OnTrackSelection")
        {
            self.check_for_error(
                self.child.borrow().on_track_selection(&mut track),
            )
        }
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
        let call = match call {
            raw::CSURF_EXT_RESET => CSurfExtended::Reset,
            raw::CSURF_EXT_SETINPUTMONITOR => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETINPUTMONITOR",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                let monitor = match self.safe_deref_i32(parm2) {
                    Some(val) => val,
                    None => return 0,
                };
                CSurfExtended::SetInputMonitor(track, monitor)
            }
            raw::CSURF_EXT_SETMETRONOME => {
                CSurfExtended::SetMetronome((parm1 as usize) != 0)
            }
            raw::CSURF_EXT_SETAUTORECARM => {
                CSurfExtended::SetAutoRecArm((parm1 as usize) != 0)
            }
            raw::CSURF_EXT_SETRECMODE => CSurfExtended::SetRecMode(
                match self.safe_deref_i32(parm1) {
                    Some(0) => CSurfRecMode::SplitForTakes,
                    Some(1) => CSurfRecMode::Replace,
                    Some(m) => {
                        self.error(
                            ReaRsError::UnexpectedAPI(format!(
                                "unknown rec mode: {m}."
                            ))
                            .into(),
                        );
                        return 0;
                    }
                    None => return 0,
                },
            ),
            raw::CSURF_EXT_SETSENDVOLUME => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETSENDVOLUME",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                CSurfExtended::SetSendVolume {
                    track,
                    send_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                    volume: match self.safe_deref_f64(parm3) {
                        Some(val) => val,
                        None => return 0,
                    },
                }
            }
            raw::CSURF_EXT_SETSENDPAN => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETSENDPAN",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                CSurfExtended::SetSendPan {
                    track,
                    send_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                    pan: match self.safe_deref_f64(parm3) {
                        Some(val) => val,
                        None => return 0,
                    },
                }
            }
            raw::CSURF_EXT_SETFXENABLED => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETFXENABLED",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                CSurfExtended::SetFxEnabled {
                    track,
                    fx_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                    enabled: (parm3 as usize) != 0,
                }
            }
            raw::CSURF_EXT_SETFXPARAM => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETFXPARAM",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                let param_value = match self.safe_deref_i32(parm2) {
                    Some(val) => val,
                    None => return 0,
                };
                let fx_idx = param_value >> 16;
                let param_idx = param_value / 0b1000000000000000;
                let val = match self.safe_deref_f64(parm3) {
                    Some(val) => val,
                    None => return 0,
                };
                CSurfExtended::SetFxParam {
                    track,
                    fx_idx: fx_idx as usize,
                    param_idx: param_idx as usize,
                    val,
                }
            }
            raw::CSURF_EXT_SETFXPARAM_RECFX => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETFXPARAM_RECFX",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                let param_value = match self.safe_deref_i32(parm2) {
                    Some(val) => val,
                    None => return 0,
                };
                let fx_idx = param_value >> 16;
                let param_idx = param_value / 0b1000000000000000;
                let val = match self.safe_deref_f64(parm3) {
                    Some(val) => val,
                    None => return 0,
                };
                CSurfExtended::SetFxParamRecfx {
                    track,
                    fx_idx: fx_idx as usize,
                    param_idx: param_idx as usize,
                    val,
                }
            }
            raw::CSURF_EXT_SETBPMANDPLAYRATE => {
                let bpm = match parm1.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_f64(parm1) {
                        Some(val) => val,
                        None => return 0,
                    }),
                };
                let playrate = match parm2.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_f64(parm2) {
                        Some(val) => val,
                        None => return 0,
                    }),
                };
                CSurfExtended::SetBpmAndPlayrate { bpm, playrate }
            }
            raw::CSURF_EXT_SETLASTTOUCHEDFX => {
                let track = match parm1.is_null() {
                    true => None,
                    false => Some(
                        match self.track_from_mut(
                            parm1 as *mut rea_rs_low::raw::MediaTrack,
                            "CSURF_EXT_SETLASTTOUCHEDFX",
                        ) {
                            None => return 0,
                            Some(track) => track,
                        },
                    ),
                };
                let item_idx = match parm2.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    }),
                };
                let fx_idx = match parm3.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_i32(parm3) {
                        Some(val) => val as usize,
                        None => return 0,
                    }),
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
                    false => Some(
                        match self.track_from_mut(
                            parm1 as *mut rea_rs_low::raw::MediaTrack,
                            "CSURF_EXT_SETFOCUSEDFX",
                        ) {
                            None => return 0,
                            Some(track) => track,
                        },
                    ),
                };
                let item_idx = match parm2.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    }),
                };
                let fx_idx = match parm3.is_null() {
                    true => None,
                    false => Some(match self.safe_deref_i32(parm3) {
                        Some(val) => val as usize,
                        None => return 0,
                    }),
                };
                CSurfExtended::SetFocusedFx {
                    track,
                    item_idx,
                    fx_idx,
                }
            }
            raw::CSURF_EXT_SETLASTTOUCHEDTRACK => {
                CSurfExtended::SetLastTouchedTrack(
                    match self.track_from_mut(
                        parm1 as *mut rea_rs_low::raw::MediaTrack,
                        "SetLastTouchedTrack",
                    ) {
                        None => return 0,
                        Some(track) => track,
                    },
                )
            }
            raw::CSURF_EXT_SETMIXERSCROLL => CSurfExtended::SetMixerScroll(
                match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETMIXERSCROLL",
                ) {
                    None => return 0,
                    Some(track) => track,
                },
            ),
            raw::CSURF_EXT_SETPAN_EX => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETPAN_EX",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                let mode = match self.safe_deref_i32(parm3) {
                    Some(val) => val,
                    None => return 0,
                };
                match mode {
                    0 => {
                        let val = match self.safe_deref_f64(parm2) {
                            Some(val) => val,
                            None => return 0,
                        };
                        CSurfExtended::SetpanEx {
                            track,
                            pan: CSurfPan::Balance(val),
                        }
                    },
                    3 => {
                        let val = match self.safe_deref_f64(parm2) {
                            Some(val) => val,
                            None => return 0,
                        };
                        CSurfExtended::SetpanEx {
                            track,
                            pan: CSurfPan::BalanceV4(val),
                        }
                    },
                    5 => {
                        let pan = match self.safe_slice_f64(parm2, 2) {
                            Some(slice) => slice,
                            None => return 0,
                        };
                        CSurfExtended::SetpanEx {
                            track,
                            pan: CSurfPan::Stereo(pan[0], pan[1]),
                        }
                    }
                    6 => {
                        let pan = match self.safe_slice_f64(parm2, 2) {
                            Some(slice) => slice,
                            None => return 0,
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
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETRECVVOLUME",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                CSurfExtended::SetRecvVolume {
                    track,
                    recv_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                    volume: match self.safe_deref_f64(parm3) {
                        Some(val) => val,
                        None => return 0,
                    },
                }
            }
            raw::CSURF_EXT_SETRECVPAN => {
                let track = match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETRECVPAN",
                ) {
                    None => return 0,
                    Some(track) => track,
                };
                CSurfExtended::SetRecvPan {
                    track,
                    recv_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                    pan: match self.safe_deref_f64(parm3) {
                        Some(val) => val,
                        None => return 0,
                    },
                }
            }
            raw::CSURF_EXT_SETFXOPEN => CSurfExtended::SetFxOpen {
                track: match self.track_from_mut(
                    parm1 as *mut rea_rs_low::raw::MediaTrack,
                    "CSURF_EXT_SETFXOPEN",
                ) {
                    None => return 0,
                    Some(track) => track,
                },
                fx_idx: match self.safe_deref_i32(parm2) {
                    Some(val) => val as usize,
                    None => return 0,
                },
                opened: (parm3 as usize) != 0,
            },
            raw::CSURF_EXT_SETFXCHANGE => {
                // REAPER passes flags in parm2 as INT_PTR, not as int*.
                let flags = parm2 as usize;
                CSurfExtended::SetFxChange {
                    track: match self.track_from_mut(
                        parm1 as *mut rea_rs_low::raw::MediaTrack,
                        "CSURF_EXT_SETFXCHANGE",
                    ) {
                        None => return 0,
                        Some(track) => track,
                    },
                    is_rec_fx: (flags & 1) != 0,
                }
            }
            raw::CSURF_EXT_SETPROJECTMARKERCHANGE => {
                CSurfExtended::SetProjectMarkerChange
            }
            raw::CSURF_EXT_TRACKFX_PRESET_CHANGED => {
                CSurfExtended::TrackFxPresetChanged {
                    track: match self.track_from_mut(
                        parm1 as *mut rea_rs_low::raw::MediaTrack,
                        "TrackFxPresetChanged",
                    ) {
                        None => return 0,
                        Some(track) => track,
                    },
                    fx_idx: match self.safe_deref_i32(parm2) {
                        Some(val) => val as usize,
                        None => return 0,
                    },
                }
            }
            raw::CSURF_EXT_SUPPORTS_EXTENDED_TOUCH => {
                CSurfExtended::SupportsExtendedTouch
            }
            raw::CSURF_EXT_MIDI_DEVICE_REMAP => {
                CSurfExtended::MidiDeviceRemap {
                    is_out: (parm1 as usize) != 0,
                    old_idx: parm2 as isize as i32,
                    new_iox: parm3 as isize as i32,
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
