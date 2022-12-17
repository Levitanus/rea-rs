use std::{
    marker::PhantomData,
    mem::{transmute, MaybeUninit},
    ops::Range,
    ptr::NonNull,
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    errors::{ReaperError, ReaperStaticResult},
    utils::{as_c_str, as_string_mut, make_c_string_buf, WithNull},
    Envelope, Immutable, KnowsProject, Mutable, ProbablyMutable, Reaper, Take,
    Track, WithReaperPtr,
};

/// Parametrizes FX functionality for [TrackFX] asn [TakeFX].
pub trait FX<T: ProbablyMutable>
where
    Self: Sized,
{
    type Parent;
    /// Get FX from parent, if exists.
    fn from_index(parent: Self::Parent, index: usize) -> Option<Self>;
    fn name(&self) -> String;
    fn is_enabled(&self) -> bool;
    fn is_online(&self) -> bool;
    fn n_inputs(&self) -> usize;
    fn n_outputs(&self) -> usize;
    fn n_params(&self) -> usize;
    fn n_presets(&self) -> ReaperStaticResult<usize>;
    /// FX Preset name
    fn preset(&self) -> ReaperStaticResult<String>;
    fn preset_index(&self) -> ReaperStaticResult<usize>;
    fn copy_to_take(&self, take: &mut Take<Mutable>, desired_index: usize);
    fn copy_to_track(&self, track: &mut Track<Mutable>, desired_index: usize);
}

/// Parametrizes mutable FX functionality for [TrackFX] asn [TakeFX].
pub trait FXMut
where
    Self: Sized,
{
    type Parent;
    fn set_enabled(&mut self, enable: bool);
    fn set_online(&mut self, online: bool);
    fn close_chain(&mut self);
    fn close_floating_window(&mut self);
    fn show_chain(&mut self);
    fn show_floating_window(&mut self);
    fn move_to_take(self, take: &Take<Mutable>, desired_index: usize);
    fn move_to_track(self, track: &Track<Mutable>, desired_index: usize);
    /// Preset can be as preset name from list of fx presets. Or path to
    /// `.vstpreset` file.
    fn set_preset(
        &mut self,
        preset: impl Into<String>,
    ) -> ReaperStaticResult<()>;
    fn set_preset_index(&mut self, preset: usize) -> ReaperStaticResult<()>;
    fn previous_preset(&mut self) -> ReaperStaticResult<()>;
    fn next_preset(&mut self) -> ReaperStaticResult<()>;
}

pub struct TrackFX<'a, T: ProbablyMutable> {
    parent: &'a Track<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> TrackFX<'a, T> {
    /// On Master Track is_rec_fx represents monitoring chain.
    pub fn from_name(
        parent: &'a Track<'a, T>,
        name: impl Into<String>,
        is_rec_fx: bool,
    ) -> Option<Self> {
        let mut name = name.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_AddByName(
                parent.get().as_ptr(),
                as_c_str(name.with_null()).as_ptr(),
                is_rec_fx,
                0,
            )
        };
        match index {
            -1 => None,
            x => Some(Self {
                parent,
                index: x as usize,
            }),
        }
    }
    /// Iterate through (Immutable) FX params
    pub fn iter_params(&'a self) -> FXParamIterator<T, Track<'a, T>, Self> {
        FXParamIterator::new(self)
    }
}
impl<'a, T: ProbablyMutable> FX<T> for TrackFX<'a, T> {
    type Parent = &'a Track<'a, T>;
    fn from_index(parent: Self::Parent, index: usize) -> Option<Self> {
        let size = 512;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFXName(
                parent.get().as_ptr(),
                index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => Some(Self { parent, index }),
            false => None,
        }
    }

    fn is_enabled(&self) -> bool {
        unsafe {
            Reaper::get().low().TrackFX_GetEnabled(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        }
    }
    fn is_online(&self) -> bool {
        unsafe {
            !Reaper::get().low().TrackFX_GetOffline(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_inputs(&self) -> usize {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TrackFX_GetIOSize(
                self.parent.get().as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        if result < 0 {
            panic!("Failed to get n_inputs. Probably, fx deleted.");
        }
        unsafe { ins.assume_init() as usize }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_outputs(&self) -> usize {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TrackFX_GetIOSize(
                self.parent.get().as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        if result < 0 {
            panic!("Failed to get n_outputs. Probably, fx deleted.");
        }
        unsafe { outs.assume_init() as usize }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_params(&self) -> usize {
        let result = unsafe {
            Reaper::get().low().TrackFX_GetNumParams(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        };
        if result < 0 {
            panic!("Failed to get n_params. Probably, fx deleted.");
        }
        result as usize
    }

    fn n_presets(&self) -> ReaperStaticResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPresetIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaperError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(unsafe { presets.assume_init() } as usize)
        }
    }

    fn preset(&self) -> ReaperStaticResult<String> {
        let size = 250;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPreset(
                self.parent.get().as_ptr(),
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
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not get preset name",
            )),
        }
    }

    fn preset_index(&self) -> ReaperStaticResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetPresetIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaperError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(result as usize)
        }
    }

    fn copy_to_take(&self, take: &mut Take<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TrackFX_CopyToTake(
                self.parent.get().as_ptr(),
                self.index as i32,
                take.get().as_ptr(),
                desired_index as i32,
                false,
            )
        }
    }

    fn copy_to_track(&self, track: &mut Track<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TrackFX_CopyToTrack(
                self.parent.get().as_ptr(),
                self.index as i32,
                track.get().as_ptr(),
                desired_index as i32,
                false,
            )
        }
    }
    fn name(&self) -> String {
        let size = 150;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFXName(
                self.parent.get().as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
            false => panic!("Can not get FX name. Probably, it's deleted"),
        }
    }
}
impl<'a> FXMut for TrackFX<'a, Mutable> {
    type Parent = Track<'a, Mutable>;
    fn set_enabled(&mut self, enable: bool) {
        unsafe {
            Reaper::get().low().TrackFX_SetEnabled(
                self.parent.get().as_ptr(),
                self.index as i32,
                enable,
            )
        }
    }
    fn set_online(&mut self, online: bool) {
        unsafe {
            Reaper::get().low().TrackFX_SetOffline(
                self.parent.get().as_ptr(),
                self.index as i32,
                !online,
            )
        }
    }
    fn close_chain(&mut self) {
        unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                0,
            )
        }
    }
    fn close_floating_window(&mut self) {
        unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                2,
            )
        }
    }
    fn show_chain(&mut self) {
        unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                1,
            )
        }
    }
    fn show_floating_window(&mut self) {
        unsafe {
            Reaper::get().low().TrackFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                3,
            )
        }
    }

    fn move_to_take(self, take: &Take<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TrackFX_CopyToTake(
                self.parent.get().as_ptr(),
                self.index as i32,
                take.get().as_ptr(),
                desired_index as i32,
                true,
            )
        }
    }
    fn move_to_track(self, track: &Track<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TrackFX_CopyToTrack(
                self.parent.get().as_ptr(),
                self.index as i32,
                track.get().as_ptr(),
                desired_index as i32,
                true,
            )
        }
    }

    fn set_preset(
        &mut self,
        preset: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut name = preset.into();
        let result = unsafe {
            Reaper::get().low().TrackFX_SetPreset(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(name.with_null()).as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn set_preset_index(&mut self, preset: usize) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetPresetByIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                preset as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn previous_preset(&mut self) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_NavigatePresets(
                self.parent.get().as_ptr(),
                self.index as i32,
                -1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn next_preset(&mut self) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_NavigatePresets(
                self.parent.get().as_ptr(),
                self.index as i32,
                1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
}

impl<'a, M: ProbablyMutable> param_parent::FXParamParent<'a, M, Track<'a, M>>
    for TrackFX<'a, M>
{
    fn param(
        &'a self,
        index: usize,
    ) -> Option<FXParam<'a, M, Track<'a, M>, Self>> {
        match index < self.n_params() {
            true => Some(FXParam::new(self, index)),
            false => None,
        }
    }

    fn param_from_ident_string(
        &'a self,
        param: impl Into<String>,
    ) -> Option<FXParam<'a, M, Track<'a, M>, Self>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_GetParamFromIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        if index < 0 {
            None
        } else {
            self.param(index as usize)
        }
    }

    fn param_name(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParamName(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_ident_string(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParamIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_value(&self, param: usize) -> f64 {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        unsafe {
            Reaper::get().low().TrackFX_GetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            )
        }
    }

    fn param_value_normalized(&self, param: usize) -> f64 {
        unsafe {
            Reaper::get().low().TrackFX_GetParamNormalized(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
            )
        }
    }

    fn param_value_formatted(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TrackFX_GetFormattedParamValue(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_mid_value(&self, param: usize) -> f64 {
        let (mut min, mut max, mut mid) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        unsafe {
            Reaper::get().low().TrackFX_GetParamEx(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
                mid.as_mut_ptr(),
            );
            mid.assume_init()
        }
    }

    fn param_value_range(&self, param: usize) -> Range<f64> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        unsafe {
            Reaper::get().low().TrackFX_GetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            );
            min.assume_init()..max.assume_init()
        }
    }

    fn param_envelope(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, Track<'a, M>, Immutable>> {
        let ptr = unsafe {
            Reaper::get().low().GetFXEnvelope(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        match NonNull::new(ptr) {
            None => None,
            Some(ptr) => {
                Some(Envelope::new(ptr, unsafe { transmute(self.parent) }))
            }
        }
    }

    fn param_step_sizes(
        &self,
        param: usize,
    ) -> ReaperStaticResult<FXParamStepSizes> {
        let (mut step, mut small_step, mut large_step, mut is_toggle) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().TrackFX_GetParameterStepSizes(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                step.as_mut_ptr(),
                small_step.as_mut_ptr(),
                large_step.as_mut_ptr(),
                is_toggle.as_mut_ptr(),
            )
        };
        match result {
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not get set sizes",
            )),
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
}
impl<'a> param_parent::FXParamParentMut<'a, Track<'a, Mutable>>
    for TrackFX<'a, Mutable>
{
    fn param_mut(
        &'a mut self,
        index: usize,
    ) -> Option<FXParam<'a, Mutable, Track<'a, Mutable>, Self>> {
        match index < self.n_params() {
            true => Some(FXParam::new(self, index)),
            false => None,
        }
    }
    fn param_from_ident_string_mut(
        &'a mut self,
        param: impl Into<String>,
    ) -> Option<FXParam<'a, Mutable, Track<'a, Mutable>, Self>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TrackFX_GetParamFromIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        if index < 0 {
            None
        } else {
            self.param_mut(index as usize)
        }
    }

    fn param_envelope_mut(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, Track<'a, Mutable>, Mutable>> {
        let ptr = unsafe {
            Reaper::get().low().GetFXEnvelope(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        match NonNull::new(ptr) {
            None => None,
            Some(ptr) => {
                Some(Envelope::new(ptr, unsafe { transmute(self.parent) }))
            }
        }
    }

    fn set_param_value(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaperError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }

    fn set_param_value_normalized(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TrackFX_SetParamNormalized(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaperError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }
}

pub struct TakeFX<'a, T: ProbablyMutable> {
    parent: &'a Take<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> TakeFX<'a, T> {
    pub fn from_name(
        parent: &'a Take<'a, T>,
        name: impl Into<String>,
    ) -> Option<Self> {
        let mut name = name.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_AddByName(
                parent.get().as_ptr(),
                as_c_str(name.with_null()).as_ptr(),
                0,
            )
        };
        match index {
            -1 => None,
            x => Some(Self {
                parent,
                index: x as usize,
            }),
        }
    }
    /// Iterate through (Immutable) FX params
    pub fn iter_params(&'a self) -> FXParamIterator<T, Take<'a, T>, Self> {
        FXParamIterator::new(self)
    }
}
impl<'a, T: ProbablyMutable> FX<T> for TakeFX<'a, T> {
    type Parent = &'a Take<'a, T>;
    fn from_index(parent: Self::Parent, index: usize) -> Option<Self> {
        let size = 512;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFXName(
                parent.get().as_ptr(),
                index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => Some(Self { parent, index }),
            false => None,
        }
    }
    fn is_enabled(&self) -> bool {
        unsafe {
            Reaper::get().low().TakeFX_GetEnabled(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        }
    }
    fn is_online(&self) -> bool {
        unsafe {
            !Reaper::get().low().TakeFX_GetOffline(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_inputs(&self) -> usize {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TakeFX_GetIOSize(
                self.parent.get().as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        if result < 0 {
            panic!("Failed to get n_inputs. Probably, fx deleted.");
        }
        unsafe { ins.assume_init() as usize }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_outputs(&self) -> usize {
        let (mut ins, mut outs) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let result = unsafe {
            Reaper::get().low().TakeFX_GetIOSize(
                self.parent.get().as_ptr(),
                self.index as i32,
                ins.as_mut_ptr(),
                outs.as_mut_ptr(),
            )
        };
        if result < 0 {
            panic!("Failed to get n_outputs. Probably, fx deleted.");
        }
        unsafe { outs.assume_init() as usize }
    }
    /// # Panics
    ///
    /// if reaper returns error, which, probably, signals, that FX is deleted.
    fn n_params(&self) -> usize {
        let result = unsafe {
            Reaper::get().low().TakeFX_GetNumParams(
                self.parent.get().as_ptr(),
                self.index as i32,
            )
        };
        if result < 0 {
            panic!("Failed to get n_params. Probably, fx deleted.");
        }
        result as usize
    }

    fn n_presets(&self) -> ReaperStaticResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPresetIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaperError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(unsafe { presets.assume_init() } as usize)
        }
    }

    fn preset(&self) -> ReaperStaticResult<String> {
        let size = 250;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPreset(
                self.parent.get().as_ptr(),
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
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not get preset name",
            )),
        }
    }

    fn preset_index(&self) -> ReaperStaticResult<usize> {
        let mut presets = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetPresetIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                presets.as_mut_ptr(),
            )
        };
        if result < 0 {
            Err(ReaperError::UnsuccessfulOperation("Can not get n_presets."))
        } else {
            Ok(result as usize)
        }
    }

    fn copy_to_take(&self, take: &mut Take<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TakeFX_CopyToTake(
                self.parent.get().as_ptr(),
                self.index as i32,
                take.get().as_ptr(),
                desired_index as i32,
                false,
            )
        }
    }
    fn copy_to_track(&self, track: &mut Track<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TakeFX_CopyToTrack(
                self.parent.get().as_ptr(),
                self.index as i32,
                track.get().as_ptr(),
                desired_index as i32,
                false,
            )
        }
    }
    fn name(&self) -> String {
        let size = 150;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFXName(
                self.parent.get().as_ptr(),
                self.index as i32,
                buf,
                size as i32,
            )
        };
        match result {
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
            false => panic!("Can not get FX name. Probably, it's deleted"),
        }
    }
}
impl<'a> FXMut for TakeFX<'a, Mutable> {
    type Parent = Track<'a, Mutable>;
    fn set_enabled(&mut self, enable: bool) {
        unsafe {
            Reaper::get().low().TakeFX_SetEnabled(
                self.parent.get().as_ptr(),
                self.index as i32,
                enable,
            )
        }
    }
    fn set_online(&mut self, online: bool) {
        unsafe {
            Reaper::get().low().TakeFX_SetOffline(
                self.parent.get().as_ptr(),
                self.index as i32,
                !online,
            )
        }
    }
    fn close_chain(&mut self) {
        unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                0,
            )
        }
    }
    fn close_floating_window(&mut self) {
        unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                2,
            )
        }
    }
    fn show_chain(&mut self) {
        unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                1,
            )
        }
    }
    fn show_floating_window(&mut self) {
        unsafe {
            Reaper::get().low().TakeFX_Show(
                self.parent.get().as_ptr(),
                self.index as i32,
                3,
            )
        }
    }
    fn move_to_take(self, take: &Take<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TakeFX_CopyToTake(
                self.parent.get().as_ptr(),
                self.index as i32,
                take.get().as_ptr(),
                desired_index as i32,
                true,
            )
        }
    }
    fn move_to_track(self, track: &Track<Mutable>, desired_index: usize) {
        unsafe {
            Reaper::get().low().TakeFX_CopyToTrack(
                self.parent.get().as_ptr(),
                self.index as i32,
                track.get().as_ptr(),
                desired_index as i32,
                true,
            )
        }
    }

    fn set_preset(
        &mut self,
        preset: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut name = preset.into();
        let result = unsafe {
            Reaper::get().low().TakeFX_SetPreset(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(name.with_null()).as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn set_preset_index(&mut self, preset: usize) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetPresetByIndex(
                self.parent.get().as_ptr(),
                self.index as i32,
                preset as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
    fn previous_preset(&mut self) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_NavigatePresets(
                self.parent.get().as_ptr(),
                self.index as i32,
                -1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }

    fn next_preset(&mut self) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_NavigatePresets(
                self.parent.get().as_ptr(),
                self.index as i32,
                1,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set preset."))
            }
        }
    }
}

impl<'a, M: ProbablyMutable> param_parent::FXParamParent<'a, M, Take<'a, M>>
    for TakeFX<'a, M>
{
    fn param(
        &'a self,
        index: usize,
    ) -> Option<FXParam<'a, M, Take<'a, M>, Self>> {
        match index < self.n_params() {
            true => Some(FXParam::new(self, index)),
            false => None,
        }
    }

    fn param_from_ident_string(
        &'a self,
        param: impl Into<String>,
    ) -> Option<FXParam<'a, M, Take<'a, M>, Self>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_GetParamFromIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        if index < 0 {
            None
        } else {
            self.param(index as usize)
        }
    }

    fn param_name(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParamName(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_ident_string(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParamIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_value(&self, param: usize) -> f64 {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        unsafe {
            Reaper::get().low().TakeFX_GetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            )
        }
    }

    fn param_value_normalized(&self, param: usize) -> f64 {
        unsafe {
            Reaper::get().low().TakeFX_GetParamNormalized(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
            )
        }
    }

    fn param_value_formatted(&self, param: usize) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().TakeFX_GetFormattedParamValue(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                buf,
                size as i32,
            )
        };
        if !result {
            panic!("Can not get param name. Fx deleted?");
        }
        as_string_mut(buf).expect("Can not convert name to String")
    }

    fn param_mid_value(&self, param: usize) -> f64 {
        let (mut min, mut max, mut mid) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        unsafe {
            Reaper::get().low().TakeFX_GetParamEx(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
                mid.as_mut_ptr(),
            );
            mid.assume_init()
        }
    }

    fn param_value_range(&self, param: usize) -> Range<f64> {
        let (mut min, mut max) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        unsafe {
            Reaper::get().low().TakeFX_GetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                min.as_mut_ptr(),
                max.as_mut_ptr(),
            );
            min.assume_init()..max.assume_init()
        }
    }

    fn param_envelope(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, Take<'a, M>, Immutable>> {
        let ptr = unsafe {
            Reaper::get().low().TakeFX_GetEnvelope(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        match NonNull::new(ptr) {
            None => None,
            Some(ptr) => {
                Some(Envelope::new(ptr, unsafe { transmute(self.parent) }))
            }
        }
    }

    fn param_step_sizes(
        &self,
        param: usize,
    ) -> ReaperStaticResult<FXParamStepSizes> {
        let (mut step, mut small_step, mut large_step, mut is_toggle) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().TakeFX_GetParameterStepSizes(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                step.as_mut_ptr(),
                small_step.as_mut_ptr(),
                large_step.as_mut_ptr(),
                is_toggle.as_mut_ptr(),
            )
        };
        match result {
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not get set sizes",
            )),
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
}

impl<'a> param_parent::FXParamParentMut<'a, Take<'a, Mutable>>
    for TakeFX<'a, Mutable>
{
    fn param_mut(
        &'a mut self,
        index: usize,
    ) -> Option<FXParam<'a, Mutable, Take<'a, Mutable>, Self>> {
        match index < self.n_params() {
            true => Some(FXParam::new(self, index)),
            false => None,
        }
    }
    fn param_from_ident_string_mut(
        &'a mut self,
        param: impl Into<String>,
    ) -> Option<FXParam<'a, Mutable, Take<'a, Mutable>, Self>> {
        let mut param = param.into();
        let index = unsafe {
            Reaper::get().low().TakeFX_GetParamFromIdent(
                self.parent.get().as_ptr(),
                self.index as i32,
                as_c_str(param.with_null()).as_ptr(),
            )
        };
        if index < 0 {
            None
        } else {
            self.param_mut(index as usize)
        }
    }

    fn param_envelope_mut(
        &self,
        param: usize,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, Take<'a, Mutable>, Mutable>> {
        let ptr = unsafe {
            Reaper::get().low().TakeFX_GetEnvelope(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                create_if_not_exists,
            )
        };
        match NonNull::new(ptr) {
            None => None,
            Some(ptr) => {
                Some(Envelope::new(ptr, unsafe { transmute(self.parent) }))
            }
        }
    }

    fn set_param_value(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetParam(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaperError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }

    fn set_param_value_normalized(
        &self,
        param: usize,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().TakeFX_SetParamNormalized(
                self.parent.get().as_ptr(),
                self.index as i32,
                param as i32,
                value,
            )
        };
        match result {
            false => Err(ReaperError::InvalidObject(
                "Can not set value. Probably, bad value.",
            )),
            true => Ok(()),
        }
    }
}

/// Parameter of a plugin. Created by [TrackFX] or [TakeFX].
#[derive(Debug)]
pub struct FXParam<
    'a,
    M: ProbablyMutable,
    P: KnowsProject,
    F: param_parent::FXParamParent<'a, M, P>,
> {
    parent: &'a F,
    index: usize,
    fx_parent: PhantomData<P>,
    mutability: PhantomData<M>,
}
impl<
        'a,
        M: ProbablyMutable,
        P: KnowsProject,
        F: param_parent::FXParamParent<'a, M, P>,
    > FXParam<'a, M, P, F>
{
    fn new(parent: &'a F, index: usize) -> Self {
        Self {
            parent,
            index,
            fx_parent: PhantomData::default(),
            mutability: PhantomData::default(),
        }
    }
    pub fn envelope(
        &self,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, P, Immutable>> {
        self.parent.param_envelope(self.index, create_if_not_exists)
    }

    pub fn name(&self) -> String {
        self.parent.param_name(self.index)
    }
    /// identifying string (:wet, :bypass, or a string returned from
    /// GetParamIdent)
    pub fn ident_string(&self) -> String {
        self.parent.param_ident_string(self.index)
    }
    pub fn value_range(&self) -> Range<f64> {
        self.parent.param_value_range(self.index)
    }
    pub fn value(&self) -> f64 {
        self.parent.param_value(self.index)
    }
    /// Probably, default value.
    pub fn mid_value(&self) -> f64 {
        self.parent.param_mid_value(self.index)
    }
    /// String representation of value as it showed in Reaper.
    pub fn value_formatted(&self) -> String {
        self.parent.param_value_formatted(self.index)
    }
    /// Param Value, scaled to be in `0.0..1.0` range.
    pub fn value_normalized(&self) -> f64 {
        self.parent.param_value_normalized(self.index)
    }
    pub fn step_sizes(&self) -> ReaperStaticResult<FXParamStepSizes> {
        self.parent.param_step_sizes(self.index)
    }
}
impl<
        'a,
        P: KnowsProject,
        F: param_parent::FXParamParentMut<'a, P>
            + param_parent::FXParamParent<'a, Mutable, P>,
    > FXParam<'a, Mutable, P, F>
{
    pub fn set_value(&mut self, value: f64) -> ReaperStaticResult<()> {
        self.parent.set_param_value(self.index, value)
    }
    /// Set value as it was scaled to be in `0.0..1.0` range.
    pub fn set_value_normalized(
        &mut self,
        value: f64,
    ) -> ReaperStaticResult<()> {
        assert!((0.0..1.0).contains(&value));
        self.parent.set_param_value_normalized(self.index, value)
    }
    pub fn envelope_mut(
        &self,
        create_if_not_exists: bool,
    ) -> Option<Envelope<'a, P, Mutable>> {
        self.parent
            .param_envelope_mut(self.index, create_if_not_exists)
    }
}

mod param_parent {

    use std::ops::Range;

    use crate::{
        errors::ReaperStaticResult, Envelope, FXMut, FXParam,
        FXParamStepSizes, Immutable, KnowsProject, Mutable, ProbablyMutable,
        FX,
    };

    pub trait FXParamParent<'a, M: ProbablyMutable, P: KnowsProject>:
        FX<M>
    {
        fn param(&'a self, index: usize) -> Option<FXParam<'a, M, P, Self>>;
        fn param_from_ident_string(
            &'a self,
            param: impl Into<String>,
        ) -> Option<FXParam<'a, M, P, Self>>;
        fn param_name(&self, param: usize) -> String;
        fn param_ident_string(&self, param: usize) -> String;
        fn param_value(&self, param: usize) -> f64;
        fn param_value_normalized(&self, param: usize) -> f64;
        fn param_value_formatted(&self, param: usize) -> String;
        fn param_mid_value(&self, param: usize) -> f64;
        fn param_value_range(&self, param: usize) -> Range<f64>;
        fn param_envelope(
            &self,
            param: usize,
            create_if_not_exists: bool,
        ) -> Option<Envelope<'a, P, Immutable>>;
        fn param_step_sizes(
            &self,
            param: usize,
        ) -> ReaperStaticResult<FXParamStepSizes>;
    }
    pub trait FXParamParentMut<'a, P: KnowsProject>: FXMut
    where
        Self: FXParamParent<'a, Mutable, P>,
    {
        fn param_mut(
            &'a mut self,
            index: usize,
        ) -> Option<FXParam<'a, Mutable, P, Self>>;
        fn param_from_ident_string_mut(
            &'a mut self,
            param: impl Into<String>,
        ) -> Option<FXParam<'a, Mutable, P, Self>>;
        fn param_envelope_mut(
            &self,
            param: usize,
            create_if_not_exists: bool,
        ) -> Option<Envelope<'a, P, Mutable>>;
        fn set_param_value(
            &self,
            param: usize,
            value: f64,
        ) -> ReaperStaticResult<()>;
        fn set_param_value_normalized(
            &self,
            param: usize,
            value: f64,
        ) -> ReaperStaticResult<()>;
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
pub trait FXParent<'a, T: FX<Immutable> + 'a> {
    fn n_fx(&self) -> usize;
    fn get_fx(&'a self, index: usize) -> Option<T>;
    fn iter_fx(&'a self) -> FXIterator<'a, T, Self>
    where
        Self: Sized,
    {
        FXIterator::new(self)
    }
}

/// Iterates through all FX of [Track] os [Take].
pub struct FXIterator<'a, T: FX<Immutable> + 'a, P: FXParent<'a, T>> {
    parent: &'a P,
    index: usize,
    phantom: PhantomData<T>,
}
impl<'a, T: FX<Immutable> + 'a, P: FXParent<'a, T>> FXIterator<'a, T, P> {
    pub fn new(parent: &'a P) -> Self {
        Self {
            parent,
            index: 0,
            phantom: PhantomData::default(),
        }
    }
}
impl<'a, T: FX<Immutable> + 'a, P: FXParent<'a, T>> Iterator
    for FXIterator<'a, T, P>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.parent.n_fx() {
            return None;
        }
        let fx = self.parent.get_fx(self.index);
        self.index += 1;
        fx
    }
}

// M: ProbablyMutable,
//     P: KnowsProject,
//     F: param_parent::FXParamParent<'a, M, P>

/// Iterates through all FXParams of [TrackFX] os [TakeFX].
pub struct FXParamIterator<
    'a,
    M: ProbablyMutable,
    P: KnowsProject,
    F: param_parent::FXParamParent<'a, M, P>,
> {
    parent: &'a F,
    index: usize,
    mutability: PhantomData<M>,
    fx_parent: PhantomData<P>,
}
impl<
        'a,
        M: ProbablyMutable,
        P: KnowsProject,
        F: param_parent::FXParamParent<'a, M, P>,
    > FXParamIterator<'a, M, P, F>
{
    pub fn new(parent: &'a F) -> Self {
        Self {
            parent,
            index: 0,
            mutability: PhantomData::default(),
            fx_parent: PhantomData::default(),
        }
    }
}
impl<
        'a,
        M: ProbablyMutable,
        P: KnowsProject,
        F: param_parent::FXParamParent<'a, M, P>,
    > Iterator for FXParamIterator<'a, M, P, F>
{
    type Item = FXParam<'a, M, P, F>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.parent.n_params() {
            return None;
        }
        let param = self.parent.param(self.index);
        self.index += 1;
        param
    }
}
