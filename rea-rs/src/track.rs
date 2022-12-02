use std::{marker::PhantomData, ptr::NonNull};

use reaper_medium::{MediaTrack, ReaperPointer};

use crate::{
    errors::{ReaperError, ReaperResult},
    utils::{as_c_str, as_mut_i8, as_string_mut, make_c_string_buf, WithNull},
    AudioAccessor, AudioAccessorParent, Fx, Immutable, Mutable,
    ProbablyMutable, Project, Reaper, TrackFX, WithReaperPtr,
};

#[derive(Debug, PartialEq)]
pub struct Track<'a, T: ProbablyMutable> {
    ptr: MediaTrack,
    should_check: bool,
    project: &'a Project,
    info_buf_size: usize,
    phantom_mut: PhantomData<T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr for Track<'a, T> {
    fn get_pointer(&self) -> ReaperPointer {
        ReaperPointer::MediaTrack(self.ptr)
    }

    fn make_unchecked(&mut self) {
        self.should_check = false
    }

    fn make_checked(&mut self) {
        self.should_check = true
    }

    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: ProbablyMutable> Track<'a, T> {
    pub fn new(project: &'a Project, pointer: MediaTrack) -> Self {
        Self {
            ptr: pointer,
            project,
            should_check: true,
            info_buf_size: 512,
            phantom_mut: PhantomData,
        }
    }
    pub fn from_index(project: &'a Project, index: usize) -> Option<Self> {
        let ptr = project.get_track_ptr(index)?;
        Some(Self::new(project, ptr))
    }
    pub fn from_name(
        project: &'a Project,
        name: impl Into<String>,
    ) -> Option<Self> {
        let name = name.into();
        let track = project.iter_tracks().find(|tr| {
            tr.get_name().expect("Can not retrieve track name") == name
        })?;
        let index = track.index();
        Self::from_index(project, index)
    }
    pub fn get(&self) -> MediaTrack {
        self.require_valid_2(&self.project).unwrap();
        self.ptr
    }

    fn get_info_string(
        &self,
        category: impl Into<String>,
    ) -> ReaperResult<String> {
        unsafe {
            let buf = make_c_string_buf(self.info_buf_size).into_raw();
            let result = Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
                buf,
                false,
            );
            match result {
                false => Err(ReaperError::UnsuccessfulOperation(
                    "Can not get info string",
                )
                .into()),
                true => Ok(as_string_mut(buf)?),
            }
        }
    }

    pub fn get_name(&self) -> ReaperResult<String> {
        self.get_info_string("P_NAME")
    }

    fn get_info_value(&self, category: impl Into<String>) -> f64 {
        unsafe {
            Reaper::get().low().GetMediaTrackInfo_Value(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
            )
        }
    }

    pub fn index(&self) -> usize {
        self.get_info_value("IP_TRACKNUMBER") as usize - 1
    }
}
impl<'a> Track<'a, Immutable> {
    pub fn get_fx(&self, index: usize) -> Option<TrackFX<Immutable>> {
        let fx = TrackFX::from_index(self, index);
        fx
    }
}
impl<'a> Track<'a, Mutable> {
    pub fn new_mut(project: &'a mut Project, pointer: MediaTrack) -> Self {
        Self::new(project, pointer)
    }
    fn set_info_string(
        &mut self,
        category: impl Into<String>,
        value: impl Into<String>,
    ) -> ReaperResult<()> {
        unsafe {
            let result = Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
                as_mut_i8(value.into().as_str()),
                true,
            );
            match result {
                false => Err(ReaperError::UnsuccessfulOperation(
                    "Can not set info string.",
                )
                .into()),
                true => Ok(()),
            }
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> ReaperResult<()> {
        self.set_info_string("P_NAME", name)
    }

    pub fn get_fx_mut(&mut self, index: usize) -> Option<TrackFX<Mutable>> {
        let fx = TrackFX::from_index(self, index);
        fx
    }

    pub fn add_audio_accessor(
        &'a mut self,
    ) -> ReaperResult<AudioAccessor<Self, Mutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .CreateTrackAudioAccessor(self.get().as_ptr())
        };
        let ptr = NonNull::new(ptr).ok_or(ReaperError::NullPtr)?;
        Ok(AudioAccessor::new(self, ptr))
    }
}
impl<'a, T: ProbablyMutable> AudioAccessorParent<'a> for Track<'a, T> {
    fn project(&'a self) -> &'a Project {
        self.project
    }
}
