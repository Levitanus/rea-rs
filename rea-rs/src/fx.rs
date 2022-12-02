use crate::{
    utils::make_c_string_buf, Mutable, ProbablyMutable, Reaper, Track, Take,
};

pub trait Fx<T: ProbablyMutable>
where
    Self: Sized,
{
    type Parent;
    fn from_index(parent: Self::Parent, index: usize) -> Option<Self>;
    fn is_enabled(&self) -> bool;
}

pub trait FxMut
where
    Self: Sized,
{
    type Parent;
    fn set_enabled(&mut self, enable: bool);
}

pub struct TrackFX<'a, T: ProbablyMutable> {
    parent: &'a Track<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> Fx<T> for TrackFX<'a, T> {
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
}
impl<'a> FxMut for TrackFX<'a, Mutable> {
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
}
pub struct TakeFX<'a, T: ProbablyMutable> {
    parent: &'a Take<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> Fx<T> for TakeFX<'a, T> {
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
}
impl<'a> FxMut for TakeFX<'a, Mutable> {
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
}
