use crate::{
    ptr_wrappers::{MediaItemTake, PcmSource},
    utils::string_from_buf,
    KnowsProject, Position, Project, ProjectContext, ReaRsError, Reaper,
    ReaperResult, Take, Volume, WithReaperPtr,
};
use chrono::TimeDelta;
use int_enum::IntEnum;
use serde_derive::{Deserialize, Serialize};
use std::{
    ffi::CString,
    mem::MaybeUninit,
    ops::{Add, Sub},
    path::PathBuf,
    ptr::NonNull,
    time::Duration,
};

#[derive(Debug, PartialEq)]
pub struct Source {
    take: Option<MediaItemTake>,
    ptr: PcmSource,
    should_check: bool,
}
impl WithReaperPtr for Source {
    type Ptr = PcmSource;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Result<Self::Ptr, ReaRsError> {
        let project = match self.take()? {
            Some(take) => Project::new(ProjectContext::Proj(
                take.project().get_pointer(),
            )),
            None => Project::new(ProjectContext::CurrentProject),
        };
        self.require_valid_2(&project)
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
impl Source {
    pub fn new<'a>(
        take: impl Into<Option<&'a Take>>,
        ptr: PcmSource,
    ) -> ReaperResult<Self> {
        let take = take.into();
        Ok(Self {
            take: if let Some(take) = take {
                Some(take.get()?)
            } else {
                None
            },
            ptr,
            should_check: true,
        })
    }

    pub fn take(&self) -> ReaperResult<Option<Take>> {
        if let Some(ptr) = self.take {
            Ok(Some(Take::new(ptr, None)?))
        } else {
            Ok(None)
        }
    }

    pub fn filename(&self) -> ReaperResult<PathBuf> {
        let size = 500;
        let mut buf = vec![0_i8; size];
        unsafe {
            Reaper::get().low().GetMediaSourceFileName(
                self.get()?.as_ptr(),
                buf.as_mut_ptr(),
                size as i32,
            )
        };
        Ok(PathBuf::from(string_from_buf(&buf)?))
    }

    /// Create Source from file.
    ///
    /// if midi_as_file == true MIDI would not be imported as project MIDI
    pub fn create_from_file(
        file: PathBuf,
        midi_as_file: bool,
    ) -> ReaperResult<Self> {
        let c_string = CString::new(file.to_string_lossy().to_string())?;
        let ptr = unsafe {
            Reaper::get().low().PCM_Source_CreateFromFileEx(
                c_string.as_ptr(),
                !midi_as_file,
            )
        };
        Self::new(
            None,
            PcmSource::new(ptr).ok_or(ReaRsError::NullPtr("source"))?,
        )
    }

    ///Create a PCM_source from a "type" (use this if you're going to load its
    /// state via LoadState/ProjectStateContext). Valid types include
    /// "WAVE", "MIDI", or whatever plug-ins define as well.
    pub fn create_from_type(
        type_string: impl Into<String>,
    ) -> ReaperResult<Self> {
        let type_string = type_string.into();
        let c_string = CString::new(type_string)?;
        let ptr = unsafe {
            Reaper::get().low().PCM_Source_CreateFromType(
                c_string.as_ptr(),
            )
        };
        Self::new(
            None,
            PcmSource::new(ptr).ok_or(ReaRsError::NullPtr("source"))?,
        )
    }

    /// Get source media length.
    ///
    /// # Safety
    ///
    /// Reaper can return length as quarter notes. Since, there is no very good
    /// way to determine length in qn as duration, it can fail sometimes.
    pub fn length(&self) -> ReaperResult<Duration> {
        let mut is_qn = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceLength(self.get()?.as_ptr(), is_qn.as_mut_ptr())
        };
        match unsafe { is_qn.assume_init() } {
            true => {
                let Some(take) = self.take()? else {
                    return Err(ReaRsError::InvalidObject(
                        "no take on source",
                    ));
                };
                let item = take.parent_item()?;
                let item_start = item.position()?;
                let start_offset = take.start_offset()?;
                let project = item.project();
                let start = Position::from(item_start - start_offset.into());
                let start_in_qn = start.as_quarters(&project);
                let end_in_qn = start_in_qn + result;
                let end = Position::from_quarters(end_in_qn, &project);
                let length = end - start;
                Ok(length.as_duration())
            }
            false => Ok(Duration::from_secs_f64(result)),
        }
    }

    pub fn n_channels(&self) -> ReaperResult<usize> {
        Ok(unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceNumChannels(self.get()?.as_ptr())
                as usize
        })
    }

    pub fn sample_rate(&self) -> ReaperResult<usize> {
        Ok(unsafe {
            Reaper::get()
                .low()
                .GetMediaSourceSampleRate(self.get()?.as_ptr())
                as usize
        })
    }

    /// Source type ("WAV, "MIDI", etc.).
    pub fn type_string(&self) -> ReaperResult<String> {
        let size = 20;
        let mut buf = vec![0_i8; size];
        unsafe {
            Reaper::get().low().GetMediaSourceType(
                self.get()?.as_ptr(),
                buf.as_mut_ptr(),
                size as i32,
            )
        };
        Ok(string_from_buf(&buf)?)
    }

    pub fn sub_project(&self) -> ReaperResult<Option<Project>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetSubProjectFromSource(self.get()?.as_ptr())
        };
        match NonNull::new(ptr) {
            None => Ok(None),
            Some(ptr) => Ok(Some(Project::new(ProjectContext::Proj(ptr)))),
        }
    }

    /// If a section/reverse block, retrieves offset/len/reverse.
    pub fn section_info(&self) -> ReaperResult<Option<SourceSectionInfo>> {
        let (mut ofst, mut len, mut rev) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let result = unsafe {
            Reaper::get().low().PCM_Source_GetSectionInfo(
                self.get()?.as_ptr(),
                ofst.as_mut_ptr(),
                len.as_mut_ptr(),
                rev.as_mut_ptr(),
            )
        };
        match result {
            false => Ok(None),
            true => Ok(Some(SourceSectionInfo {
                offset: Duration::from_secs_f64(unsafe { ofst.assume_init() }),
                length: Duration::from_secs_f64(unsafe { len.assume_init() }),
                reversed: unsafe { rev.assume_init() },
            })),
        }
    }

    pub fn calculate_normalization(
        &self,
        units: SourceNoramlizeUnit,
        target: Volume,
        start: SourceOffset,
        end: SourceOffset,
    ) -> ReaperResult<Volume> {
        let result = unsafe {
            Reaper::get().low().CalculateNormalization(
                self.get()?.as_ptr(),
                units.int_value(),
                target.get(),
                start.as_secs_f64(),
                end.as_secs_f64(),
            )
        };
        Ok(Volume::from(result))
    }

    pub fn delete(&mut self) -> ReaperResult<()> {
        unsafe { Reaper::get().low().PCM_Source_Destroy(self.get()?.as_ptr()) }
        Ok(())
    }
}

#[test]
fn test_source_offset() {
    let offset = SourceOffset::from_secs_f64(2.0);
    assert_eq!(offset.as_secs_f64(), 2.0);
    let offset = SourceOffset::from_secs_f64(-2.0);
    assert_eq!(offset.as_secs_f64(), -2.0);
    let offset = SourceOffset::from_secs_f64(-2.543);
    assert_eq!(offset.as_secs_f64(), -2.543);
}

#[derive(
    Debug,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Hash,
    Copy,
    Clone,
    Serialize,
    Deserialize,
)]
pub struct SourceOffset {
    offset: TimeDelta,
}
impl SourceOffset {
    pub fn from_secs_f64(secs: f64) -> Self {
        let duration = Duration::from_secs_f64(secs.abs());
        let offset = TimeDelta::from_std(duration).unwrap();
        if secs.is_sign_negative() {
            Self { offset: -offset }
        } else {
            Self { offset }
        }
    }
    pub fn get(&self) -> TimeDelta {
        self.offset
    }
    pub fn as_secs_f64(&self) -> f64 {
        let seconds = self.offset.num_seconds();
        let nanoseconds = self.offset.num_microseconds().unwrap();
        seconds as f64 + (nanoseconds - seconds * 1000000) as f64 / 1000000.0
    }
}
impl From<TimeDelta> for SourceOffset {
    fn from(value: TimeDelta) -> Self {
        Self { offset: value }
    }
}
impl From<Position> for SourceOffset {
    fn from(value: Position) -> Self {
        Self {
            offset: TimeDelta::from_std(value.as_duration()).unwrap(),
        }
    }
}
impl Into<Position> for SourceOffset {
    fn into(self) -> Position {
        self.offset.abs().to_std().unwrap().into()
    }
}
impl From<Duration> for SourceOffset {
    fn from(value: Duration) -> Self {
        Self {
            offset: TimeDelta::from_std(value).unwrap(),
        }
    }
}
impl Into<Duration> for SourceOffset {
    fn into(self) -> Duration {
        self.offset.abs().to_std().unwrap()
    }
}
impl Add<SourceOffset> for SourceOffset {
    type Output = SourceOffset;

    fn add(self, rhs: SourceOffset) -> Self::Output {
        SourceOffset::from(self.offset + rhs.offset)
    }
}
impl Add<Duration> for SourceOffset {
    type Output = SourceOffset;

    fn add(self, rhs: Duration) -> Self::Output {
        SourceOffset::from(self.offset + TimeDelta::from_std(rhs).unwrap())
    }
}
impl Sub<SourceOffset> for SourceOffset {
    type Output = SourceOffset;

    fn sub(self, rhs: SourceOffset) -> Self::Output {
        SourceOffset::from(self.offset - rhs.offset)
    }
}
impl Sub<Duration> for SourceOffset {
    type Output = SourceOffset;

    fn sub(self, rhs: Duration) -> Self::Output {
        SourceOffset::from(self.offset - TimeDelta::from_std(rhs).unwrap())
    }
}

/// If a section/reverse block, retrieves offset/len/reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSectionInfo {
    offset: Duration,
    length: Duration,
    reversed: bool,
}

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(
    Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Copy, Clone, IntEnum,
)]
pub enum SourceNoramlizeUnit {
    LUFS_I = 0,
    RMS_I = 1,
    Peak = 2,
    TruePeak = 3,
    LUFS_M = 4,
    LUFS_S = 5,
}
