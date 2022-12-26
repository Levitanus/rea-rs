use crate::{
    ptr_wrappers::PcmSource,
    utils::{as_string_mut, make_c_string_buf},
    KnowsProject, Mutable, Position, ProbablyMutable, Project, ProjectContext,
    Reaper, Take, WithReaperPtr,
};
use serde_derive::{Deserialize, Serialize};
use std::{mem::MaybeUninit, path::PathBuf, ptr::NonNull, time::Duration};

#[derive(Debug, PartialEq)]
pub struct Source<'a, T: ProbablyMutable> {
    take: &'a Take<'a, T>,
    ptr: PcmSource,
    should_check: bool,
}
impl<'a, T: ProbablyMutable> WithReaperPtr for Source<'a, T> {
    type Ptr = PcmSource;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.take.project()).unwrap();
        self.get_pointer()
    }
    fn make_unchecked(&mut self) {
        self.should_check = false;
    }
    fn make_checked(&mut self) {
        self.should_check = true;
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: ProbablyMutable> Source<'a, T> {
    pub fn new(take: &'a Take<'a, T>, ptr: PcmSource) -> Self {
        Self {
            take,
            ptr,
            should_check: true,
        }
    }

    pub fn take(&self) -> &Take<'a, T> {
        self.take
    }

    pub fn filename(&self) -> PathBuf {
        let size = 500;
        let buf = make_c_string_buf(size).into_raw();
        unsafe {
            Reaper::get().low().GetMediaSourceFileName(
                self.get().as_ptr(),
                buf,
                size as i32,
            )
        };
        PathBuf::from(as_string_mut(buf).expect("Can not retrieve file name"))
    }

    /// Get source media length.
    ///
    /// # Safety
    ///
    /// Reaper can return length as quarter notes. Since, there is no very good
    /// way to determine length in qn as duration, it can fail sometimes.
    pub fn length(&self) -> Duration {
        let mut is_qn = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceLength(self.get().as_ptr(), is_qn.as_mut_ptr())
        };
        match unsafe { is_qn.assume_init() } {
            true => {
                let item_start = self.take().item().position();
                let offset = self.take().start_offset();
                let start = item_start - Position::new(offset);
                let start_in_qn = start.as_quarters(self.take().project());
                let end_in_qn = start_in_qn + result;
                let end =
                    Position::from_quarters(end_in_qn, self.take().project());
                let length = end - start;
                length.as_duration()
            }
            false => Duration::from_secs_f64(result),
        }
    }

    pub fn n_channels(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceNumChannels(self.get().as_ptr())
                as usize
        }
    }

    pub fn sample_rate(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceSampleRate(self.get().as_ptr())
                as usize
        }
    }

    /// Source type ("WAV, "MIDI", etc.).
    pub fn type_string(&self) -> String {
        let size = 20;
        let buf = make_c_string_buf(size).into_raw();
        unsafe {
            Reaper::get().low().GetMediaSourceType(
                self.get().as_ptr(),
                buf,
                size as i32,
            )
        };
        as_string_mut(buf).expect("Can not convert type to string")
    }

    pub fn sub_project(&self) -> Option<Project> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetSubProjectFromSource(self.get().as_ptr())
        };
        match NonNull::new(ptr) {
            None => None,
            Some(ptr) => Project::new(ProjectContext::Proj(ptr)).into(),
        }
    }

    /// If a section/reverse block, retrieves offset/len/reverse.
    pub fn section_info(&self) -> Option<SourceSectionInfo> {
        let (mut ofst, mut len, mut rev) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().PCM_Source_GetSectionInfo(
                self.get().as_ptr(),
                ofst.as_mut_ptr(),
                len.as_mut_ptr(),
                rev.as_mut_ptr(),
            )
        };
        match result {
            false => None,
            true => Some(SourceSectionInfo {
                offset: Duration::from_secs_f64(unsafe { ofst.assume_init() }),
                length: Duration::from_secs_f64(unsafe { len.assume_init() }),
                reversed: unsafe { rev.assume_init() },
            }),
        }
    }
}
impl<'a> Source<'a, Mutable> {
    pub fn delete(&mut self) {
        unsafe { Reaper::get().low().PCM_Source_Destroy(self.get().as_ptr()) }
    }
}

/// If a section/reverse block, retrieves offset/len/reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSectionInfo {
    offset: Duration,
    length: Duration,
    reversed: bool,
}
