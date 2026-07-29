use std::{
    ffi::CString, marker::PhantomData, mem::MaybeUninit, ops::Range,
    ptr::NonNull,
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    utils::{as_c_str, as_string_mut, make_c_string_buf, WithNull},
    Envelope, KnowsProject, ReaRsError, Reaper, ReaperResult, Take, Track,
    WithReaperPtr,
};

/// Parametrizes FX functionality for [TrackFX] asn [TakeFX].
pub trait FX
where
    Self: Sized,
{
    type Parent;
    /// Get FX from parent, if exists.
    fn from_index(
        parent: &Self::Parent,
        index: usize,
    ) -> ReaperResult<Option<Self>>;
    fn name(&self) -> ReaperResult<String>;
    fn index(&self) -> usize;
    fn is_enabled(&self) -> ReaperResult<bool>;
    fn is_online(&self) -> ReaperResult<bool>;
    fn is_instrument(&self) -> ReaperResult<bool>;
    fn n_inputs(&self) -> ReaperResult<usize>;
    fn n_outputs(&self) -> ReaperResult<usize>;
    fn n_params(&self) -> ReaperResult<usize>;
    fn n_presets(&self) -> ReaperResult<usize>;
    /// FX Preset name
    fn preset(&self) -> ReaperResult<String>;
    fn preset_index(&self) -> ReaperResult<usize>;
    fn copy_to_take(
        &self,
        take: &mut Take,
        desired_index: usize,
    ) -> ReaperResult<()>;
    fn copy_to_track(
        &self,
        track: &mut Track,
        desired_index: usize,
    ) -> ReaperResult<()>;

    fn set_enabled(&mut self, enable: bool) -> ReaperResult<()>;
    fn set_online(&mut self, online: bool) -> ReaperResult<()>;
    fn close_chain(&mut self) -> ReaperResult<()>;
    fn close_floating_window(&mut self) -> ReaperResult<()>;
    fn show_chain(&mut self) -> ReaperResult<()>;
    fn show_floating_window(&mut self) -> ReaperResult<()>;
    fn move_to_take(
        self,
        take: &Take,
        desired_index: usize,
    ) -> ReaperResult<()>;
    fn move_to_track(
        self,
        track: &Track,
        desired_index: usize,
    ) -> ReaperResult<()>;
    /// Preset can be as preset name from list of fx presets. Or path to
    /// `.vstpreset` file.
    fn set_preset(&mut self, preset: impl Into<String>) -> ReaperResult<()>;
    fn set_preset_index(&mut self, preset: usize) -> ReaperResult<()>;
    fn previous_preset(&mut self) -> ReaperResult<()>;
    fn next_preset(&mut self) -> ReaperResult<()>;
    fn delete(self) -> Result<(), ReaRsError>;
}

#[derive(Debug, Clone)]
pub struct TrackFX {
    parent: Track,
    index: usize,
}
impl TrackFX {
    /// On Master Track is_rec_fx represents monitoring chain.
    pub fn from_name(
        parent: &Track,
        name: impl Into<String>,
        is_rec_fx: bool,
    ) -> ReaperResult<Option<Self>> {
        let mut name = name.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_AddByName(
                parent.get()?.as_ptr(),
                as_c_str(name.with_null()).as_ptr(),
                is_rec_fx,
                0,
            )
        };
        Ok(match index {
            -1 => None,
            x => Some(Self {
                parent: Track::new(&parent.project(), parent.get()?)?,
                index: x as usize,
            }),
        })
    }
    /// Iterate through (Immutable) FX params
    pub fn iter_params(&self) -> FXParamIterator<'_, Track, Self> {
        FXParamIterator::new(self)
    }
}
impl FX for TrackFX {
    type Parent = Track;
    fn from_index(
        parent: &Self::Parent,
        index: usize,
    ) -> ReaperResult<Option<Self>> {
        let size = 512;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFXName(
                parent.get()?.as_ptr(),
                index as i32,
                buf,
                size as i32,
            )
        };
        Ok(match result {
            true => Some(Self {
                parent: Track::new(&parent.project(), parent.get()?)?,
                index,
            }),
            false => None,
        })
    }
    fn index(&self) -> usize {
        self.index
    }

    fn is_enabled(&self) -> ReaperResult<bool> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_GetEnabled(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        })
    }
    fn is_online(&self) -> ReaperResult<bool> {
        Ok(unsafe {
            !Reaper::get().low().TrackFX_GetOffline(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        })
    }
    fn is_instrument(&self) -> ReaperResult<bool> {
        let parmname =
            CString::new("is_instrument").expect("failed to make CString");
        let size = 8;
        let buf = make_c_string_buf(size).into_raw();
        let got_value = unsafe {
            Reaper::get().low().TrackFX_GetNamedConfigParm(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                parmname.as_ptr(),
                buf,
                size as i32,
            )
        };
        if !got_value {
            return Ok(false);
        }

        Ok(matches!(
            as_string_mut(buf)
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok()),
            Some(1)
        ))
    }

    fn n_inputs(&self) -> ReaperResult<usize> {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TrackFX_GetIOSize(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        match result {
            x if x < 0 => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get n_inputs."))
            }
            _ => Ok(unsafe { ins.assume_init() as usize }),
        }
    }
    fn n_outputs(&self) -> ReaperResult<usize> {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TrackFX_GetIOSize(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        match result {
            x if x < 0 => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get n_outputs.",
            )),
            _ => Ok(unsafe { outs.assume_init() as usize }),
        }
    }
    fn n_params(&self) -> ReaperResult<usize> {
        let result = unsafe {
            Reaper::get().low().TrackFX_GetNumParams(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        };
        match result {
            x if x < 0 => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get n_params."))
            }
            _ => Ok(result as usize),
        }
    }

    fn n_presets(&self) -> ReaperResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPresetIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(unsafe { presets.assume_init() } as usize)
        }
    }

    fn preset(&self) -> ReaperResult<String> {
        let size = 250;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPreset(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => {
                Ok(as_string_mut(buf)
                    .expect("Can not convert result to string."))
            }
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get preset name",
            )),
        }
    }

    fn preset_index(&self) -> ReaperResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPresetIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(result as usize)
        }
    }

    fn copy_to_take(
        &self,
        take: &mut Take,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_CopyToTake(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                take.get()?.as_ptr(),
                desired_index as i32,
                false,
            )
        })
    }

    fn copy_to_track(
        &self,
        track: &mut Track,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_CopyToTrack(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                track.get()?.as_ptr(),
                desired_index as i32,
                false,
            )
        })
    }
    fn name(&self) -> ReaperResult<String> {
        let size = 150;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFXName(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => as_string_mut(buf)
                .or(Err(ReaRsError::Str("Cannot convert name from CString"))),
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get FX name. Probably, it's deleted",
            )),
        }
    }
    fn set_enabled(&mut self, enable: bool) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_SetEnabled(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                enable,
            )
        })
    }
    fn set_online(&mut self, online: bool) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_SetOffline(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                !online,
            )
        })
    }
    fn close_chain(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                0,
            )
        })
    }
    fn close_floating_window(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                2,
            )
        })
    }
    fn show_chain(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                1,
            )
        })
    }
    fn show_floating_window(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                3,
            )
        })
    }

    fn move_to_take(
        self,
        take: &Take,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_CopyToTake(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                take.get()?.as_ptr(),
                desired_index as i32,
                true,
            )
        })
    }
    fn move_to_track(
        self,
        track: &Track,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_CopyToTrack(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                track.get()?.as_ptr(),
                desired_index as i32,
                true,
            )
        })
    }
    fn delete(self) -> Result<(), ReaRsError> {
        match unsafe {
            Reaper::get()
                .low()
                .TrackFX_Delete(self.parent.get()?.as_ptr(), self.index as i32)
        } {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("can not delete FX"))
            }
        }
    }

    fn set_preset(&mut self, preset: impl Into<String>) -> ReaperResult<()> {
        let mut name = preset.into();
        let result = unsafe {
            Reaper::get().low().TrackFX_SetPreset(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(name.with_null()).as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn set_preset_index(&mut self, preset: usize) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetPresetByIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                preset as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn previous_preset(&mut self) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_NavigatePresets(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                -1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn next_preset(&mut self) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_NavigatePresets(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
}

impl param_parent::FXParamParent<Track> for TrackFX {
    fn param(&self, index: usize) -> Option<FXParam<Track, Self>> {
        match self.n_params() {
            Ok(n_params) => match index < n_params {
                true => Some(FXParam::new(self, index)),
                false => None,
            },
            Err(_) => None,
        }
    }

    fn param_from_ident_string(
        &self,
        param: impl Into<String>,
    ) -> ReaperResult<Option<FXParam<Track, Self>>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_GetParamFromIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        let res = if index < 0 {
            None
        } else {
            self.param(index as usize)
        };
        Ok(res)
    }

    fn param_name(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParamName(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param name. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert name to String")))
    }

    fn param_ident_string(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParamIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param ident. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert name to String")))
    }

    fn param_value(&self, param: usize) -> ReaperResult<f64> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        Ok(unsafe {
            Reaper::get().low().TrackFX_GetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            )
        })
    }

    fn param_value_normalized(&self, param: usize) -> ReaperResult<f64> {
        Ok(unsafe {
            Reaper::get().low().TrackFX_GetParamNormalized(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
            )
        })
    }

    fn param_value_formatted(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFormattedParamValue(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param value. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert value to String")))
    }

    fn param_mid_value(&self, param: usize) -> ReaperResult<f64> {
        let (mut min, mut max, mut mid) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        Ok(unsafe {
            Reaper::get().low().TrackFX_GetParamEx(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
                mid.as_mut_ptr(),
            );
            mid.assume_init()
        })
    }

    fn param_value_range(&self, param: usize) -> ReaperResult<Range<f64>> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        Ok(unsafe {
            Reaper::get().low().TrackFX_GetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            );
            min.assume_init()..max.assume_init()
        })
    }

    fn param_envelope(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, Track>>> {
        let ptr = unsafe {
            Reaper::get().low().GetFXEnvelope(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        Ok(match NonNull::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, &self.parent)),
        })
    }

    fn param_step_sizes(
        &self,
        param: usize,
    ) -> ReaperResult<FXParamStepSizes> {
        let (mut step, mut small_step, mut large_step, mut is_toggle) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParameterStepSizes(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                step.as_mut_ptr(),
                small_step.as_mut_ptr(),
                large_step.as_mut_ptr(),
                is_toggle.as_mut_ptr(),
            )
        };
        match result {
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get set sizes"))
            }
            true => unsafe {
                Ok(FXParamStepSizes {
                    step: step.assume_init(),
                    small_step: small_step.assume_init(),
                    large_step: large_step.assume_init(),
                    is_toggle: is_toggle.assume_init(),
                })
            },
        }
    }

    fn param_mut(&mut self, index: usize) -> Option<FXParam<Track, Self>> {
        match self.n_params() {
            Ok(n_params) => match index < n_params {
                true => Some(FXParam::new(self, index)),
                false => None,
            },
            Err(_) => None,
        }
    }
    fn param_from_ident_string_mut(
        &mut self,
        param: impl Into<String>,
    ) -> ReaperResult<Option<FXParam<Track, Self>>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_GetParamFromIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        Ok(if index < 0 {
            None
        } else {
            self.param_mut(index as usize)
        })
    }

    fn param_envelope_mut(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, Track>>> {
        let ptr = unsafe {
            Reaper::get().low().GetFXEnvelope(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        Ok(match NonNull::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, &self.parent)),
        })
    }

    fn set_param_value(&self, param: usize, value: f64) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaRsError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }

    fn set_param_value_normalized(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetParamNormalized(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaRsError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TakeFX {
    parent: Take,
    index: usize,
}
impl TakeFX {
    pub fn from_name(
        parent: &Take,
        name: impl Into<String>,
    ) -> ReaperResult<Option<Self>> {
        let mut name = name.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_AddByName(
                parent.get()?.as_ptr(),
                as_c_str(name.with_null()).as_ptr(),
                0,
            )
        };
        Ok(match index {
            -1 => None,
            x => Some(Self {
                parent: parent.clone(),
                index: x as usize,
            }),
        })
    }
    /// Iterate through (Immutable) FX params
    pub fn iter_params(&self) -> FXParamIterator<'_, Take, Self> {
        FXParamIterator::new(self)
    }
}
impl FX for TakeFX {
    type Parent = Take;
    fn from_index(
        parent: &Self::Parent,
        index: usize,
    ) -> ReaperResult<Option<Self>> {
        let size = 512;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFXName(
                parent.get()?.as_ptr(),
                index as i32,
                buf,
                size as i32,
            )
        };
        Ok(match result {
            true => Some(Self {
                parent: parent.clone(),
                index,
            }),
            false => None,
        })
    }
    fn index(&self) -> usize {
        self.index
    }
    fn is_enabled(&self) -> ReaperResult<bool> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetEnabled(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        })
    }
    fn is_online(&self) -> ReaperResult<bool> {
        Ok(unsafe {
            !Reaper::get().low().TakeFX_GetOffline(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        })
    }
    fn is_instrument(&self) -> ReaperResult<bool> {
        let parmname =
            CString::new("is_instrument").expect("failed to make CString");
        let size = 8;
        let buf = make_c_string_buf(size).into_raw();
        let got_value = unsafe {
            Reaper::get().low().TakeFX_GetNamedConfigParm(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                parmname.as_ptr(),
                buf,
                size as i32,
            )
        };
        if !got_value {
            return Ok(false);
        }

        Ok(matches!(
            as_string_mut(buf)
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok()),
            Some(1)
        ))
    }
    fn n_inputs(&self) -> ReaperResult<usize> {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TakeFX_GetIOSize(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        match result {
            x if x < 0 => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get n_inputs."))
            }
            _ => Ok(unsafe { ins.assume_init() as usize }),
        }
    }
    fn n_outputs(&self) -> ReaperResult<usize> {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TakeFX_GetIOSize(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        match result {
            x if x < 0 => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get n_outputs.",
            )),
            _ => Ok(unsafe { outs.assume_init() as usize }),
        }
    }
    fn n_params(&self) -> ReaperResult<usize> {
        let result = unsafe {
            Reaper::get().low().TakeFX_GetNumParams(
                self.parent.get()?.as_ptr(),
                self.index as i32,
            )
        };
        match result {
            x if x < 0 => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get n_params."))
            }
            _ => Ok(result as usize),
        }
    }

    fn n_presets(&self) -> ReaperResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPresetIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(unsafe { presets.assume_init() } as usize)
        }
    }

    fn preset(&self) -> ReaperResult<String> {
        let size = 250;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPreset(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => {
                Ok(as_string_mut(buf)
                    .expect("Can not convert result to string."))
            }
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get preset name",
            )),
        }
    }

    fn preset_index(&self) -> ReaperResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPresetIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(result as usize)
        }
    }

    fn copy_to_take(
        &self,
        take: &mut Take,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_CopyToTake(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                take.get()?.as_ptr(),
                desired_index as i32,
                false,
            )
        })
    }
    fn copy_to_track(
        &self,
        track: &mut Track,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_CopyToTrack(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                track.get()?.as_ptr(),
                desired_index as i32,
                false,
            )
        })
    }
    fn name(&self) -> ReaperResult<String> {
        let size = 150;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFXName(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => as_string_mut(buf)
                .or(Err(ReaRsError::Str("Cannot convert name from CString"))),
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Can not get FX name. Probably, it's deleted",
            )),
        }
    }

    fn set_enabled(&mut self, enable: bool) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_SetEnabled(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                enable,
            )
        })
    }
    fn set_online(&mut self, online: bool) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_SetOffline(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                !online,
            )
        })
    }
    fn close_chain(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                0,
            )
        })
    }
    fn close_floating_window(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                2,
            )
        })
    }
    fn show_chain(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                1,
            )
        })
    }
    fn show_floating_window(&mut self) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                3,
            )
        })
    }
    fn move_to_take(
        self,
        take: &Take,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_CopyToTake(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                take.get()?.as_ptr(),
                desired_index as i32,
                true,
            )
        })
    }
    fn move_to_track(
        self,
        track: &Track,
        desired_index: usize,
    ) -> ReaperResult<()> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_CopyToTrack(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                track.get()?.as_ptr(),
                desired_index as i32,
                true,
            )
        })
    }
    fn delete(self) -> Result<(), ReaRsError> {
        match unsafe {
            Reaper::get()
                .low()
                .TakeFX_Delete(self.parent.get()?.as_ptr(), self.index as i32)
        } {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("can not delete FX"))
            }
        }
    }

    fn set_preset(&mut self, preset: impl Into<String>) -> ReaperResult<()> {
        let mut name = preset.into();
        let result = unsafe {
            Reaper::get().low().TakeFX_SetPreset(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(name.with_null()).as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn set_preset_index(&mut self, preset: usize) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetPresetByIndex(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                preset as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
    fn previous_preset(&mut self) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_NavigatePresets(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                -1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn next_preset(&mut self) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_NavigatePresets(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
}

impl param_parent::FXParamParent<Take> for TakeFX {
    fn param(&self, index: usize) -> Option<FXParam<Take, Self>> {
        match self.n_params() {
            Ok(n_params) => match index < n_params {
                true => Some(FXParam::new(self, index)),
                false => None,
            },
            Err(_) => None,
        }
    }

    fn param_from_ident_string(
        &self,
        param: impl Into<String>,
    ) -> ReaperResult<Option<FXParam<Take, Self>>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_GetParamFromIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        Ok(if index < 0 {
            None
        } else {
            self.param(index as usize)
        })
    }

    fn param_name(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParamName(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param name. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert name to String")))
    }

    fn param_ident_string(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParamIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param ident. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert name to String")))
    }

    fn param_value(&self, param: usize) -> ReaperResult<f64> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            )
        })
    }

    fn param_value_normalized(&self, param: usize) -> ReaperResult<f64> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetParamNormalized(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
            )
        })
    }

    fn param_value_formatted(&self, param: usize) -> ReaperResult<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFormattedParamValue(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get param value. Fx deleted?",
            ));
        }
        as_string_mut(buf)
            .or(Err(ReaRsError::Str("Can not convert value to String")))
    }

    fn param_mid_value(&self, param: usize) -> ReaperResult<f64> {
        let (mut min, mut max, mut mid) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetParamEx(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
                mid.as_mut_ptr(),
            );
            mid.assume_init()
        })
    }

    fn param_value_range(&self, param: usize) -> ReaperResult<Range<f64>> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            );
            min.assume_init()..max.assume_init()
        })
    }

    fn param_envelope(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, Take>>> {
        let ptr = unsafe {
            Reaper::get().low().TakeFX_GetEnvelope(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        Ok(match NonNull::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, &self.parent)),
        })
    }

    fn param_step_sizes(
        &self,
        param: usize,
    ) -> ReaperResult<FXParamStepSizes> {
        let (mut step, mut small_step, mut large_step, mut is_toggle) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParameterStepSizes(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                step.as_mut_ptr(),
                small_step.as_mut_ptr(),
                large_step.as_mut_ptr(),
                is_toggle.as_mut_ptr(),
            )
        };
        match result {
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get set sizes"))
            }
            true => unsafe {
                Ok(FXParamStepSizes {
                    step: step.assume_init(),
                    small_step: small_step.assume_init(),
                    large_step: large_step.assume_init(),
                    is_toggle: is_toggle.assume_init(),
                })
            },
        }
    }

    fn param_mut(&mut self, index: usize) -> Option<FXParam<Take, Self>> {
        match self.n_params() {
            Ok(n_params) => match index < n_params {
                true => Some(FXParam::new(self, index)),
                false => None,
            },
            Err(_) => None,
        }
    }
    fn param_from_ident_string_mut(
        &mut self,
        param: impl Into<String>,
    ) -> ReaperResult<Option<FXParam<Take, Self>>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_GetParamFromIdent(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        Ok(if index < 0 {
            None
        } else {
            self.param_mut(index as usize)
        })
    }

    fn param_envelope_mut(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, Take>>> {
        let ptr = unsafe {
            Reaper::get().low().TakeFX_GetEnvelope(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        Ok(match NonNull::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, &self.parent)),
        })
    }

    fn set_param_value(&self, param: usize, value: f64) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetParam(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaRsError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }

    fn set_param_value_normalized(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetParamNormalized(
                self.parent.get()?.as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaRsError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }
}

/// Parameter of a plugin. Created by [TrackFX] or [TakeFX].
#[derive(Debug)]
pub struct FXParam<P: KnowsProject, F: param_parent::FXParamParent<P>> {
    index: usize,
    fx_parent: PhantomData<P>,
    parent: F,
}
impl<P: KnowsProject, F: param_parent::FXParamParent<P>> FXParam<P, F> {
    fn new(parent: &F, index: usize) -> Self {
        Self {
            index,
            fx_parent: PhantomData::default(),
            parent: parent.clone(),
        }
    }
    pub fn envelope(
        &self,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, P>>> {
        self.parent.param_envelope(self.index, create_if_not_exists)
    }

    pub fn name(&self) -> ReaperResult<String> {
        self.parent.param_name(self.index)
    }
    /// identifying string (:wet, :bypass, or a string returned from
    /// GetParamIdent)
    pub fn ident_string(&self) -> ReaperResult<String> {
        self.parent.param_ident_string(self.index)
    }
    pub fn value_range(&self) -> ReaperResult<Range<f64>> {
        self.parent.param_value_range(self.index)
    }
    pub fn value(&self) -> ReaperResult<f64> {
        self.parent.param_value(self.index)
    }
    /// Probably, default value.
    pub fn mid_value(&self) -> ReaperResult<f64> {
        self.parent.param_mid_value(self.index)
    }
    /// String representation of value as it showed in Reaper.
    pub fn value_formatted(&self) -> ReaperResult<String> {
        self.parent.param_value_formatted(self.index)
    }
    /// Param Value, scaled to be in `0.0..1.0` range.
    pub fn value_normalized(&self) -> ReaperResult<f64> {
        self.parent.param_value_normalized(self.index)
    }
    pub fn step_sizes(&self) -> ReaperResult<FXParamStepSizes> {
        self.parent.param_step_sizes(self.index)
    }

    pub fn set_value(&mut self, value: f64) -> ReaperResult<()> {
        self.parent.set_param_value(self.index, value)
    }
    /// Set value as it was scaled to be in `0.0..1.0` range.
    pub fn set_value_normalized(&mut self, value: f64) -> ReaperResult<()> {
        assert!((0.0..1.0).contains(&value));
        self.parent.set_param_value_normalized(self.index, value)
    }
    pub fn envelope_mut(
        &self,
        create_if_not_exists: bool,
    ) -> ReaperResult<Option<Envelope<'_, P>>> {
        self.parent
            .param_envelope_mut(self.index, create_if_not_exists)
    }
}

mod param_parent {

    use std::ops::Range;

    use crate::{
        Envelope, FXParam, FXParamStepSizes, KnowsProject, ReaperResult, FX,
    };

    pub trait FXParamParent<P: KnowsProject>: FX + Clone {
        fn param(&self, index: usize) -> Option<FXParam<P, Self>>;
        fn param_from_ident_string(
            &self,
            param: impl Into<String>,
        ) -> ReaperResult<Option<FXParam<P, Self>>>;
        fn param_name(&self, param: usize) -> ReaperResult<String>;
        fn param_ident_string(&self, param: usize) -> ReaperResult<String>;
        fn param_value(&self, param: usize) -> ReaperResult<f64>;
        fn param_value_normalized(&self, param: usize) -> ReaperResult<f64>;
        fn param_value_formatted(&self, param: usize) -> ReaperResult<String>;
        fn param_mid_value(&self, param: usize) -> ReaperResult<f64>;
        fn param_value_range(&self, param: usize) -> ReaperResult<Range<f64>>;
        fn param_envelope(
            &self,
            param: usize,
            create_if_not_exists: bool,
        ) -> ReaperResult<Option<Envelope<'_, P>>>;
        fn param_step_sizes(
            &self,
            param: usize,
        ) -> ReaperResult<FXParamStepSizes>;

        fn param_mut(&mut self, index: usize) -> Option<FXParam<P, Self>>;
        fn param_from_ident_string_mut(
            &mut self,
            param: impl Into<String>,
        ) -> ReaperResult<Option<FXParam<P, Self>>>;
        fn param_envelope_mut(
            &self,
            param: usize,
            create_if_not_exists: bool,
        ) -> ReaperResult<Option<Envelope<'_, P>>>;
        fn set_param_value(
            &self,
            param: usize,
            value: f64,
        ) -> ReaperResult<()>;
        fn set_param_value_normalized(
            &self,
            param: usize,
            value: f64,
        ) -> ReaperResult<()>;
    }
}

/// [FXParam] step sizes.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FXParamStepSizes {
    pub step: f64,
    pub small_step: f64,
    pub large_step: f64,
    pub is_toggle: bool,
}

/// Indicates, that type can hold FX ([Track] or [Take])
pub trait FXParent<T: FX> {
    fn n_fx(&self) -> ReaperResult<usize>;
    fn get_fx(&self, index: usize) -> ReaperResult<Option<T>>;
    fn iter_fx(&self) -> FXIterator<'_, T, Self>
    where
        Self: Sized,
    {
        FXIterator::new(self)
    }
}

/// Iterates through all FX of [Track] os [Take].
pub struct FXIterator<'a, T: FX, P: FXParent<T>> {
    parent: &'a P,
    index: usize,
    phantom: PhantomData<T>,
}
impl<'a, T: FX, P: FXParent<T>> FXIterator<'a, T, P> {
    pub fn new(parent: &'a P) -> Self {
        Self {
            parent,
            index: 0,
            phantom: PhantomData::default(),
        }
    }
}
impl<T: FX, P: FXParent<T>> Iterator for FXIterator<'_, T, P> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.parent.n_fx().ok()? {
            return None;
        }
        let fx = self.parent.get_fx(self.index).ok()?;
        self.index += 1;
        fx
    }
}

// M: ProbablyMutable,
//     P: KnowsProject,
//     F: param_parent::FXParamParent<, M, P>

/// Iterates through all FXParams of [TrackFX] os [TakeFX].
pub struct FXParamIterator<
    'a,
    P: KnowsProject,
    F: param_parent::FXParamParent<P>,
> {
    parent: &'a F,
    index: usize,

    fx_parent: PhantomData<P>,
}
impl<'a, P: KnowsProject, F: param_parent::FXParamParent<P>>
    FXParamIterator<'a, P, F>
{
    pub fn new(parent: &'a F) -> Self {
        Self {
            parent,
            index: 0,

            fx_parent: PhantomData::default(),
        }
    }
}
impl<P: KnowsProject, F: param_parent::FXParamParent<P>> Iterator
    for FXParamIterator<'_, P, F>
{
    type Item = FXParam<P, F>;
    fn next(&mut self) -> Option<Self::Item> {
        let n_params = match self.parent.n_params() {
            Ok(n_params) => n_params,
            Err(_) => return None,
        };
        if self.index == n_params {
            return None;
        }
        let param = self.parent.param(self.index);
        self.index += 1;
        param
    }
}
