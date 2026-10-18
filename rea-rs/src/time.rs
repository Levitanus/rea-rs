use std::{
    cmp::Ordering,
    ffi::CString,
    hash::{Hash, Hasher},
    mem::MaybeUninit,
    ops::{Add, Div, Mul, Rem, Sub},
    ptr::{null, null_mut},
    time::Duration,
};

use chrono::TimeDelta;
use serde::{Deserialize, Serialize};

use crate::{
    utils::string_from_buf, Direction, Project, ProjectContext, ReaRsError,
    Reaper, ReaperResult, Take, TimeStringFormattingMode, WithReaperPtr,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerBeatPosition {
    pub measure: i32,
    pub beats: f64,
}

pub(crate) fn parse_ruler_beat_position(
    s: &str,
) -> ReaperResult<RulerBeatPosition> {
    let (measure, beats) = s.trim().split_once('.').ok_or_else(|| {
        ReaRsError::UnsuccessfulOperation("Invalid measure offset")
    })?;

    let measure = measure.parse::<i32>().map_err(|_| {
        ReaRsError::UnsuccessfulOperation("Invalid measure number")
    })?;

    let beats = beats.parse::<f64>().map_err(|_| {
        ReaRsError::UnsuccessfulOperation("Invalid beat value")
    })?;

    if !beats.is_finite() {
        return Err(ReaRsError::UnsuccessfulOperation(
            "Beat value must be finite",
        ));
    }

    Ok(RulerBeatPosition { measure, beats })
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Measure {
    pub index: u32,
    pub start: Time,
    pub end: Time,
    pub time_signature: TimeSignature,
    pub tempo: f64,
    pub qn_start: f64,
    pub qn_end: f64,
}

impl Measure {
    pub fn from_index(index: u32, project: &Project) -> ReaperResult<Self> {
        let low = Reaper::get().low();
        let (
            mut qn_start,
            mut qn_end,
            mut numerator,
            mut denominator,
            mut tempo,
        ) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let seconds = unsafe {
            low.TimeMap_GetMeasureInfo(
                project.get()?.as_ptr(),
                index as i32,
                qn_start.as_mut_ptr(),
                qn_end.as_mut_ptr(),
                numerator.as_mut_ptr(),
                denominator.as_mut_ptr(),
                tempo.as_mut_ptr(),
            )
        };
        let (qn_start, qn_end, numerator, denominator, tempo) = unsafe {
            (
                qn_start.assume_init(),
                qn_end.assume_init(),
                numerator.assume_init(),
                denominator.assume_init(),
                tempo.assume_init(),
            )
        };
        let start = Time::try_from(seconds)?;
        let end = Time::from_quarters(qn_end, project)?;
        Ok(Self {
            index,
            start,
            end,
            time_signature: TimeSignature::new(
                u32::try_from(numerator).map_err(|_| {
                    ReaRsError::InvalidObject("invalid time signature")
                })?,
                u32::try_from(denominator).map_err(|_| {
                    ReaRsError::InvalidObject("invalid time signature")
                })?,
            ),
            tempo,
            qn_start,
            qn_end,
        })
    }

    /// Find the measure containing a signed project position.
    pub fn from_time(position: Time, project: &Project) -> ReaperResult<Self> {
        let (_, relative, _) = position.time_to_beats_raw(project)?;
        let (index, _) = relative.relative(project)?;
        Self::from_index(index, project)
    }

    /// Find the measure containing a signed project position.
    pub fn from_quarters(
        position: f64,
        project: &Project,
    ) -> ReaperResult<Self> {
        let index = unsafe {
            Reaper::get().low().TimeMap_QNToMeasures(
                project.get()?.as_ptr(),
                position,
                null_mut(),
                null_mut(),
            )
        };
        Self::from_index((index - 1) as u32, project)
    }

    /// Return measure number as it is displayed in the project ruler.
    pub fn get_ruler_index(&self) -> ReaperResult<i32> {
        let s = self
            .start
            .format_timestr(crate::TimeStringFormattingMode::MeasuresBeats)?;
        Ok(parse_ruler_beat_position(&s)?.measure)
    }
}

/// Good helper to be sample-accurate.
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct SampleAmount {
    amount: u32,
}
impl SampleAmount {
    pub fn new(amount: u32) -> Self {
        Self { amount }
    }
    pub fn get(&self) -> u32 {
        self.amount
    }
    pub fn from_time(
        time: impl Into<Time>,
        samplerate: u32,
    ) -> ReaperResult<Self> {
        if samplerate == 0 {
            return Err(ReaRsError::InvalidObject(
                "sample rate must be non-zero",
            ));
        }
        let seconds = time.into().as_seconds_f64();
        let sample_amount = (samplerate as f64) * seconds;
        if sample_amount > u32::MAX as f64 {
            return Err(ReaRsError::UnsuccessfulOperationString(format!("time {seconds} and samplerate {samplerate} resulted to infinity")));
        }

        Ok(Self {
            amount: sample_amount as u32,
        })
    }
    pub fn as_time(self, samplerate: u32) -> ReaperResult<Time> {
        if samplerate == 0 {
            return Err(ReaRsError::InvalidObject(
                "sample rate must be non-zero",
            ));
        }
        let seconds = self.amount / samplerate;
        let reminder = self.amount % samplerate;
        let nanos = reminder * 1_000_000_000 / samplerate;
        Ok(Time::new(TimeDelta::new(seconds as i64, nanos).ok_or(
            ReaRsError::InvalidObject(
                "sample amount reminder bigger than second",
            ),
        )?))
    }
}

/// Position in project.
///
/// Internally holds a signed [TimeDelta] from project start.
/// Keeps interfaces to all time transformations (e.g. between secs, quarters
/// and ppq)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Time {
    pub(crate) offset: TimeDelta,
}

impl Time {
    pub fn new(time: TimeDelta) -> Self {
        Self { offset: time }
    }

    /// Access the signed time delta represented by this position.
    pub fn get(&self) -> TimeDelta {
        self.offset
    }

    /// Compare positions with allowed inacurracy.
    ///
    /// precision (or epsilon) ‒ the allowed inacurracy.
    pub fn eq_with_precision(
        self,
        other: impl Into<Self>,
        precision: TimeDelta,
    ) -> bool {
        (self.offset - other.into().offset).abs() <= precision
    }

    pub fn as_seconds_f64(&self) -> f64 {
        self.offset.as_seconds_f64()
    }
    pub fn from_seconds_f64(seconds: f64) -> ReaperResult<Self> {
        Self::try_from(seconds)
    }

    pub fn as_ppq(&self, take: &Take) -> Result<u32, ReaRsError> {
        let ppq = unsafe {
            Reaper::get().low().MIDI_GetPPQPosFromProjTime(
                take.get()?.as_mut(),
                self.as_seconds_f64(),
            )
        };
        if !ppq.is_finite() || ppq < u32::MIN as f64 || ppq >= u32::MAX as f64
        {
            return Err(ReaRsError::InvalidObject(
                "PPQ position out of range",
            ));
        }
        Ok(ppq.round() as u32)
    }

    pub fn from_ppq(ppq: u32, take: &Take) -> Result<Self, ReaRsError> {
        let val = unsafe {
            Reaper::get()
                .low()
                .MIDI_GetProjTimeFromPPQPos(take.get()?.as_mut(), ppq as f64)
        };
        Self::try_from(val)
    }

    /// Get time from QN position, as it is returned by measure info.
    ///
    /// # Note
    /// Partial measures are counted as full measures.
    pub fn from_quarters(
        quarters: f64,
        project: &Project,
    ) -> ReaperResult<Self> {
        if !quarters.is_finite() {
            return Err(ReaRsError::InvalidObject(
                "quarter-note position must be finite",
            ));
        }
        let seconds = unsafe {
            Reaper::get()
                .low()
                .TimeMap2_QNToTime(project.context().to_raw(), quarters)
        };
        Self::try_from(seconds)
    }

    /// Get time as QN position, as it is returned by measure info.
    ///
    /// # Note
    /// Partial measures are counted as full measures.
    pub fn as_quarters(self, project: &Project) -> ReaperResult<f64> {
        let quarters = unsafe {
            Reaper::get().low().TimeMap2_timeToQN(
                project.context().to_raw(),
                self.as_seconds_f64(),
            )
        };
        if quarters.is_finite() {
            Ok(quarters)
        } else {
            Err(ReaRsError::InvalidObject("invalid quarter-note position"))
        }
    }

    /// Get time from QN position, considering real length of partial measures.
    pub fn from_quarters_abs(
        quarters: f64,
        project: &Project,
    ) -> ReaperResult<Self> {
        if !quarters.is_finite() {
            return Err(ReaRsError::InvalidObject(
                "quarter-note position must be finite",
            ));
        }
        let seconds = unsafe {
            Reaper::get()
                .low()
                .TimeMap_QNToTime_abs(project.context().to_raw(), quarters)
        };
        Self::try_from(seconds)
    }

    /// Get time as QN position, considering real length of partial measures.
    pub fn as_quarters_abs(self, project: &Project) -> ReaperResult<f64> {
        let quarters = unsafe {
            Reaper::get().low().TimeMap_timeToQN_abs(
                project.context().to_raw(),
                self.as_seconds_f64(),
            )
        };
        if quarters.is_finite() {
            Ok(quarters)
        } else {
            Err(ReaRsError::InvalidObject("invalid quarter-note position"))
        }
    }

    pub fn from_beats(
        project: &Project,
        beats: BeatsPosition,
    ) -> ReaperResult<Self> {
        let (tpos, measures) = match beats {
            BeatsPosition::Absolute(b) => (b, null()),
            BeatsPosition::Relative {
                measure_index,
                beats,
            } => (beats, &(measure_index as i32) as *const i32),
        };
        let seconds = unsafe {
            Reaper::get().low().TimeMap2_beatsToTime(
                project.get()?.as_ptr(),
                tpos,
                measures,
            )
        };
        Self::from_seconds_f64(seconds)
    }

    pub fn as_beats_relative(
        &self,
        project: &Project,
    ) -> ReaperResult<(u32, f64)> {
        let (_, relative, _) = self.time_to_beats_raw(project)?;
        let relative = relative.relative(project)?;
        Ok(relative)
    }

    pub fn as_beats_absolute(&self, project: &Project) -> ReaperResult<f64> {
        let (absolute, _, _) = self.time_to_beats_raw(project)?;
        let absolute = absolute.absolute(project)?;
        Ok(absolute)
    }

    /// Get full Measure info from time.
    ///
    /// Returns
    /// absolute: `BeatsPosition::Absolute(f64)`,
    /// relative: `BeatsPosition::Relative{measure_index: u32, beats: f64}`,
    /// time_signature: `TimeSignature`
    pub fn time_to_beats_raw(
        &self,
        project: &Project,
    ) -> ReaperResult<(BeatsPosition, BeatsPosition, TimeSignature)> {
        let (mut measures, mut fullbeats, mut num, mut denom) = (
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        );
        let beats = unsafe {
            Reaper::get().low().TimeMap2_timeToBeats(
                project.get()?.as_ptr(),
                self.as_seconds_f64(),
                measures.as_mut_ptr(),
                num.as_mut_ptr(),
                fullbeats.as_mut_ptr(),
                denom.as_mut_ptr(),
            )
        };
        let measures = unsafe { measures.assume_init() };
        let num = unsafe { num.assume_init() };
        let fullbeats = unsafe { fullbeats.assume_init() };
        let denom = unsafe { denom.assume_init() };
        Ok((
            BeatsPosition::Absolute(fullbeats),
            BeatsPosition::Relative {
                measure_index: measures as u32,
                beats,
            },
            TimeSignature::new(num as u32, denom as u32),
        ))
    }

    pub fn format_timestr(
        &self,
        mode: TimeStringFormattingMode,
    ) -> ReaperResult<String> {
        let mut buf = [0_i8; 100];
        unsafe {
            Reaper::get().low().format_timestr_pos(
                self.as_seconds_f64(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                mode.into(),
            )
        };
        string_from_buf(&buf)
    }

    pub fn format_timestr_len(
        &self,
        offset: impl Into<Time>,
        mode: TimeStringFormattingMode,
    ) -> ReaperResult<String> {
        let mut buf = [0_i8; 100];
        unsafe {
            Reaper::get().low().format_timestr_len(
                self.as_seconds_f64(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                offset.into().as_seconds_f64(),
                mode.into(),
            )
        };
        string_from_buf(&buf)
    }

    pub fn parse_timestr(
        timestr: impl AsRef<str>,
        mode: TimeStringFormattingMode,
    ) -> ReaperResult<Self> {
        let s = CString::new(timestr.as_ref())?;
        let time = unsafe {
            Reaper::get()
                .low()
                .parse_timestr_pos(s.as_ptr(), mode.into())
        };
        Self::from_seconds_f64(time)
    }

    pub fn parse_timestr_len(
        timestr: impl AsRef<str>,
        offset: impl Into<Time>,
        mode: TimeStringFormattingMode,
    ) -> ReaperResult<Self> {
        let s = CString::new(timestr.as_ref())?;
        let offset = offset.into().as_seconds_f64();
        let time = unsafe {
            Reaper::get().low().parse_timestr_len(
                s.as_ptr(),
                offset,
                mode.into(),
            )
        };
        Self::from_seconds_f64(time)
    }

    pub fn checked_duration_since(&self, start: Time) -> ReaperResult<Self> {
        if self < &start {
            return Err(ReaRsError::InvalidObject(
                "Given start is larger than current ProjectTime",
            ));
        }
        Ok((self.offset - start.offset).into())
    }

    /// Get the time displayed in the project ruler.
    pub fn get_ruler_time(&self, project: &Project) -> ReaperResult<Self> {
        Ok(*self + project.get_time_offset(false)?)
    }

    /// Get beat position as it is displayed in the ruler.
    pub fn get_ruler_beat_position(&self) -> ReaperResult<RulerBeatPosition> {
        let s = self
            .format_timestr(crate::TimeStringFormattingMode::MeasuresBeats)?;
        parse_ruler_beat_position(&s)
    }

    pub fn is_negative(&self) -> bool {
        self.offset < TimeDelta::zero()
    }

    pub fn is_zero(&self) -> bool {
        self.offset.is_zero()
    }
}
impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset
    }
}
impl Eq for Time {}
impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Time {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
impl Hash for Time {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.offset.hash(state)
    }
}
impl Into<f64> for Time {
    fn into(self) -> f64 {
        self.offset.as_seconds_f64()
    }
}
impl TryFrom<f64> for Time {
    type Error = ReaRsError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let negative = value < 0.0;
        let std = Duration::try_from_secs_f64(value.abs())
            .map_err(|e| ReaRsError::UnderlyingError(e.into()))?;
        let mut offset = TimeDelta::from_std(std)
            .map_err(|e| ReaRsError::UnderlyingError(e.into()))?;
        if negative {
            offset = -offset
        }
        Ok(Self { offset })
    }
}
impl From<TimeDelta> for Time {
    fn from(value: TimeDelta) -> Self {
        Self { offset: value }
    }
}
impl From<Time> for TimeDelta {
    fn from(value: Time) -> Self {
        value.offset
    }
}
impl From<Duration> for Time {
    fn from(value: Duration) -> Self {
        Self {
            offset: TimeDelta::from_std(value).unwrap_or(TimeDelta::MAX),
        }
    }
}
impl Default for Time {
    fn default() -> Self {
        Self {
            offset: TimeDelta::zero(),
        }
    }
}
impl Add for Time {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            offset: self.offset + rhs.offset,
        }
    }
}
impl Sub for Time {
    type Output = Time;

    fn sub(self, rhs: Self) -> Self::Output {
        Time {
            offset: self.offset - rhs.offset,
        }
    }
}
impl Div<Time> for Time {
    type Output = i64;
    fn div(self, rhs: Time) -> Self::Output {
        let lhs: i128 = self
            .offset
            .num_nanoseconds()
            .expect("num nanosecond overflowed")
            .into();
        let rhs: i128 = rhs
            .offset
            .num_nanoseconds()
            .expect("num nanoseconds overflowed")
            .into();
        (lhs / rhs).try_into().expect("can not convert i128 to i64")
    }
}
impl Div<i32> for Time {
    type Output = Time;
    fn div(mut self, rhs: i32) -> Self::Output {
        self.offset = self.offset / rhs;
        self
    }
}
impl Rem<Time> for Time {
    type Output = Time;
    fn rem(self, rhs: Time) -> Self::Output {
        let lhs: i64 = self
            .offset
            .num_nanoseconds()
            .expect("num nanosecond overflowed")
            .into();
        let rhs: i64 = rhs
            .offset
            .num_nanoseconds()
            .expect("num nanoseconds overflowed")
            .into();
        let nanos: i64 = lhs % rhs;
        Time {
            offset: TimeDelta::nanoseconds(nanos),
        }
    }
}
impl Mul<i32> for Time {
    type Output = Time;
    fn mul(mut self, rhs: i32) -> Self::Output {
        self.offset = self.offset * rhs;
        self
    }
}

impl Add<TimeDelta> for Time {
    type Output = Time;

    fn add(self, rhs: TimeDelta) -> Self::Output {
        Time {
            offset: self.offset + rhs,
        }
    }
}

impl Sub<TimeDelta> for Time {
    type Output = Time;

    fn sub(self, rhs: TimeDelta) -> Self::Output {
        Time {
            offset: self.offset - rhs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Copy)]
pub struct TimeRange {
    pub start: Time,
    pub end: Time,
}
impl TimeRange {
    pub fn new(
        start: impl Into<Time>,
        end: impl Into<Time>,
    ) -> ReaperResult<Self> {
        let (start, end) = (start.into(), end.into());
        end.checked_duration_since(start)?;
        Ok(Self { start, end })
    }
    pub fn from_seconds_f64(start: f64, end: f64) -> ReaperResult<Self> {
        Ok(Self {
            start: Time::from_seconds_f64(start)?,
            end: Time::from_seconds_f64(end)?,
        })
    }
    pub fn length(&self) -> Time {
        self.end - self.start
    }
    pub fn format_timestr_len(
        &self,
        mode: TimeStringFormattingMode,
    ) -> ReaperResult<String> {
        let mut buf = [0_i8; 100];
        let length = self.end - self.start;
        unsafe {
            Reaper::get().low().format_timestr_len(
                length.as_seconds_f64(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                self.start.as_seconds_f64(),
                mode.into(),
            )
        };
        string_from_buf(&buf)
    }
}
impl<T, V> From<(T, V)> for TimeRange
where
    T: Into<Time>,
    V: Into<Time>,
{
    fn from(value: (T, V)) -> Self {
        Self {
            start: value.0.into(),
            end: value.1.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoopRangeKind {
    TimeSelection,
    LoopSelection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopRange {
    project_context: ProjectContext,
    kind: LoopRangeKind,
}
impl LoopRange {
    #[deprecated]
    pub fn new(project: &Project, kind: LoopRangeKind) -> Self {
        Self {
            project_context: project.context(),
            kind,
        }
    }

    pub fn load_from_project(
        project: &Project,
        kind: LoopRangeKind,
    ) -> ReaperResult<Option<Self>> {
        let obj = Self {
            project_context: project.context(),
            kind: kind,
        };
        let range = obj.get()?;
        if range.start == range.end || range.end <= Time::default() {
            return Ok(None);
        }
        Ok(Some(obj))
    }

    pub fn get_kind(&self) -> LoopRangeKind {
        return self.kind;
    }

    pub fn get(&self) -> ReaperResult<TimeRange> {
        let is_loop = match self.kind {
            LoopRangeKind::LoopSelection => true,
            LoopRangeKind::TimeSelection => false,
        };
        let (mut start, mut end) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let project = Project::new(self.project_context);
        unsafe {
            Reaper::get().low().GetSet_LoopTimeRange2(
                project.get()?.as_ptr(),
                false,
                is_loop,
                start.as_mut_ptr(),
                end.as_mut_ptr(),
                false,
            )
        };
        let start = Time::from_seconds_f64(unsafe { start.assume_init() })?;
        let end = Time::from_seconds_f64(unsafe { end.assume_init() })?;
        Ok(TimeRange::new(start, end)?)
    }

    pub fn set(
        &self,
        range: impl Into<TimeRange>,
        allowautoseek: bool,
    ) -> ReaperResult<()> {
        let is_loop = match self.kind {
            LoopRangeKind::LoopSelection => true,
            LoopRangeKind::TimeSelection => false,
        };
        let range = range.into();
        let (mut start, mut end) = (
            MaybeUninit::new(range.start.as_seconds_f64()),
            MaybeUninit::new(range.end.as_seconds_f64()),
        );
        let project = Project::new(self.project_context);
        unsafe {
            Reaper::get().low().GetSet_LoopTimeRange2(
                project.get()?.as_ptr(),
                true,
                is_loop,
                start.as_mut_ptr(),
                end.as_mut_ptr(),
                allowautoseek,
            )
        };
        Ok(())
    }

    pub fn get_start(&self) -> ReaperResult<Time> {
        Ok(self.get()?.start)
    }
    pub fn get_end(&self) -> ReaperResult<Time> {
        Ok(self.get()?.end)
    }

    pub fn set_start(&self, start: Time, allowseek: bool) -> ReaperResult<()> {
        let end = self.get()?.end;
        if start > end {
            return Err(ReaRsError::InvalidObject(
                "time range end precedes start",
            ));
        }
        self.set((start, end), allowseek)
    }
    pub fn set_end(&self, end: Time, allowseek: bool) -> ReaperResult<()> {
        let start = self.get()?.start;
        if end < start {
            return Err(ReaRsError::InvalidObject(
                "time range end precedes start",
            ));
        }
        self.set((start, end), allowseek)
    }

    pub fn get_length(&self) -> ReaperResult<Time> {
        Ok(self.get()?.length())
    }
    pub fn set_length(
        &self,
        length: impl Into<Time>,
        allowseek: bool,
    ) -> ReaperResult<()> {
        let start = self.get()?.start;
        let end = start + length.into();
        self.set((start, end), allowseek)
    }

    /// Move selection left or right.
    ///
    /// Returns true if snap is enabled.
    /// Returns `ReaRsError::InvalidObject` if called from time selection but
    /// not from a loop selection
    pub fn shift(&self, direction: Direction) -> ReaperResult<bool> {
        if self.kind == LoopRangeKind::TimeSelection {
            return Err(ReaRsError::InvalidObject(
                "can not shift time selection, only loop selection",
            ));
        }
        let low = Reaper::get().low();
        let project = Project::new(self.project_context);
        Ok(unsafe {
            match direction {
                Direction::Right => {
                    low.Loop_OnArrow(project.get()?.as_ptr(), 1)
                }
                Direction::Left => {
                    low.Loop_OnArrow(project.get()?.as_ptr(), -1)
                }
            }
        })
    }
}

/// Straightforward TimeSignature, that can be used as
/// [Project] parameter.
///
/// Not sure it should be used in complex musical analysis.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct TimeSignature {
    pub numerator: u32,
    pub denominator: u32,
}
impl TimeSignature {
    pub fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }
    pub fn get(&self) -> (u32, u32) {
        (self.numerator, self.denominator)
    }
}

/// Represents project position in beats.
///
/// The way metronome clicks:
/// - if time signature is 3/8, the length of a measure will be 3 beats
/// - if time signature is 5/2, the length of a measure will be 5 beats
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub enum BeatsPosition {
    Absolute(f64),
    Relative { measure_index: u32, beats: f64 },
}
impl BeatsPosition {
    /// Get absolute beat position.
    ///
    /// If is already absolute, it will be almost zero-cost, if not ‒ internal
    /// Reaper API calls will be used.
    pub fn absolute(&self, project: &Project) -> ReaperResult<f64> {
        if let Self::Absolute(f) = self {
            Ok(*f)
        } else {
            let time = Time::from_beats(project, *self)?;
            time.as_beats_absolute(project)
        }
    }
    /// Get absolute beat position.
    ///
    /// Returns
    /// (measure_index: u32, beats: f64)
    ///
    /// If is already relative, it will be almost zero-cost, if not ‒ internal
    /// Reaper API calls will be used.
    pub fn relative(&self, project: &Project) -> ReaperResult<(u32, f64)> {
        if let Self::Relative {
            measure_index,
            beats,
        } = self
        {
            Ok((*measure_index, *beats))
        } else {
            let time = Time::from_beats(project, *self)?;
            time.as_beats_relative(project)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use crate::{SampleAmount, Time};

    #[test]
    fn clamped_time() {
        let time = Time::from_seconds_f64(3.5).unwrap();
        let clamped = time
            .clamp(TimeDelta::seconds(0).into(), TimeDelta::seconds(3).into());
        let min = time.min(Time::default());
        assert_eq!(clamped, TimeDelta::seconds(3).into());
        assert_eq!(min, Time::from_seconds_f64(0.0).unwrap());
    }

    #[test]
    fn test_sample_amount() {
        assert_eq!(
            SampleAmount::from_time(TimeDelta::seconds(1), 44100)
                .unwrap()
                .get(),
            44100
        );
        assert!(SampleAmount::from_time(TimeDelta::seconds(1), 0).is_err());
        assert!(SampleAmount::new(1).as_time(0).is_err());
        assert!(SampleAmount::from_time(TimeDelta::MAX, u32::MAX).is_err());
    }

    #[test]
    fn position_preserves_time_delta_precision() {
        let seconds = 5.687253432;
        assert!(
            (Time::try_from(seconds).unwrap().as_seconds_f64() - seconds)
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn signed_seconds_round_trip() {
        for seconds in [-120.25, -0.001, 0.0, 7.125] {
            let position = Time::try_from(seconds).unwrap();
            assert!((position.as_seconds_f64() - seconds).abs() < 1e-9);
        }
        assert!(Time::try_from(f64::INFINITY).is_err());
        assert!(Time::try_from(f64::NAN).is_err());
    }

    #[test]
    fn position_can_be_compared_at_explicit_precision() {
        let precision = TimeDelta::microseconds(48);
        let left = Time::try_from(1.000_001).unwrap();
        let near = Time::try_from(1.000_049).unwrap();
        let midpoint = Time::try_from(1.000_050).unwrap();
        assert!(left.eq_with_precision(near, precision));
        assert!(!left.eq_with_precision(midpoint, precision));
    }
}
