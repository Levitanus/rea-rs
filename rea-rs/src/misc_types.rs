use crate::{
    utils::string_from_buf, Direction, Project, ReaRsError, Reaper,
    ReaperResult, Take, WithReaperPtr,
};
use chrono::TimeDelta;
use int_enum::IntEnum;
use serde_derive::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    ffi::CString,
    hash::{Hash, Hasher},
    mem::MaybeUninit,
    ops::{Add, Sub},
    time::Duration,
};

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Measure {
    pub index: i32,
    pub start: Position,
    pub end: Position,
    pub time_signature: TimeSignature,
    pub tempo: f64,
}

impl Measure {
    pub fn from_index(index: i32, project: &Project) -> ReaperResult<Self> {
        let low = Reaper::get().low();
        let (mut qn_end, mut numerator, mut denominator, mut tempo) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        let seconds = unsafe {
            low.TimeMap_GetMeasureInfo(
                project.get()?.as_ptr(),
                index,
                std::ptr::null_mut(),
                qn_end.as_mut_ptr(),
                numerator.as_mut_ptr(),
                denominator.as_mut_ptr(),
                tempo.as_mut_ptr(),
            )
        };
        let (qn_end, numerator, denominator, tempo) = unsafe {
            (
                qn_end.assume_init(),
                numerator.assume_init(),
                denominator.assume_init(),
                tempo.assume_init(),
            )
        };
        let start = Position::from_host_seconds(seconds)?;
        let end = Position::from_quarters(qn_end, project)?;
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
        })
    }

    /// Find the measure containing a signed project position.
    pub fn from_position(
        position: Position,
        project: &Project,
    ) -> ReaperResult<Self> {
        let low = Reaper::get().low();
        let (mut start_qn, mut end_qn) =
            (MaybeUninit::zeroed(), MaybeUninit::zeroed());
        let native_index = unsafe {
            low.TimeMap_QNToMeasures(
                project.get()?.as_ptr(),
                position.as_quarters(project)?,
                start_qn.as_mut_ptr(),
                end_qn.as_mut_ptr(),
            )
        };
        Self::from_index(native_index, project)
    }
    pub fn ppq_start(&self, take: &Take, ppq: i64) -> ReaperResult<i64> {
        let low = Reaper::get().low();
        let pos = unsafe {
            low.MIDI_GetPPQPos_StartOfMeasure(take.get()?.as_ptr(), ppq as f64)
        };
        if !pos.is_finite() || pos < i64::MIN as f64 || pos > i64::MAX as f64 {
            return Err(ReaRsError::InvalidObject(
                "invalid measure start PPQ",
            ));
        }
        Ok(pos.round() as i64)
    }
    pub fn ppq_end(&self, take: &Take, ppq: i64) -> ReaperResult<i64> {
        let low = Reaper::get().low();
        let pos = unsafe {
            low.MIDI_GetPPQPos_EndOfMeasure(take.get()?.as_ptr(), ppq as f64)
        };
        if !pos.is_finite() || pos < i64::MIN as f64 || pos > i64::MAX as f64 {
            return Err(ReaRsError::InvalidObject("invalid measure end PPQ"));
        }
        Ok(pos.round() as i64)
    }
    pub fn from_ppq(&self, take: &Take, ppq: i64) -> Result<Self, ReaRsError> {
        let pos = Position::from_ppq(ppq, take)?;
        Self::from_position(pos, &take.project())
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
    pub fn from_time(time: Duration, samplerate: u32) -> ReaperResult<Self> {
        if samplerate == 0 {
            return Err(ReaRsError::InvalidObject(
                "sample rate must be non-zero",
            ));
        }
        let amount = time
            .as_nanos()
            .checked_mul(u128::from(samplerate))
            .ok_or(ReaRsError::InvalidObject("sample amount overflow"))?
            / 1_000_000_000;
        Ok(Self {
            amount: u32::try_from(amount).map_err(|_| {
                ReaRsError::InvalidObject("sample amount overflow")
            })?,
        })
    }
    pub fn as_time(self, samplerate: u32) -> ReaperResult<Duration> {
        if samplerate == 0 {
            return Err(ReaRsError::InvalidObject(
                "sample rate must be non-zero",
            ));
        }
        let nanos = u128::from(self.amount)
            .checked_mul(1_000_000_000)
            .ok_or(ReaRsError::InvalidObject("sample duration overflow"))?
            / u128::from(samplerate);
        let seconds = u64::try_from(nanos / 1_000_000_000).map_err(|_| {
            ReaRsError::InvalidObject("sample duration overflow")
        })?;
        Ok(Duration::new(seconds, (nanos % 1_000_000_000) as u32))
    }
}

/// Represents Audio\MIDI physical out pin.
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HardwareSocket {
    index: u32,
    name: String,
}
impl HardwareSocket {
    pub fn new(index: u32, name: impl Into<String>) -> Self {
        Self {
            index,
            name: name.into(),
        }
    }
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        GetLength, Pan, PanLaw, PanLawMode, Position, SampleAmount, Volume,
    };

    #[test]
    fn test_sample_amount() {
        assert_eq!(
            SampleAmount::from_time(Duration::from_secs(1), 44100)
                .unwrap()
                .get(),
            44100
        );
        assert!(SampleAmount::from_time(Duration::from_secs(1), 0).is_err());
        assert!(SampleAmount::new(1).as_time(0).is_err());
        assert!(SampleAmount::from_time(Duration::MAX, u32::MAX).is_err());
    }

    #[test]
    fn position_preserves_time_delta_precision() {
        let seconds = 5.687253432;
        assert!(
            (Position::from_host_seconds(seconds)
                .unwrap()
                .as_seconds_f64()
                - seconds)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn checked_numeric_conversions_reject_invalid_values() {
        assert!(Volume::try_from(f64::NAN).is_err());
        assert!(Volume::try_from(-1.0).is_err());
        assert!(Pan::try_from(f64::INFINITY).is_err());
        assert!(Pan::try_from(1.1).is_err());
        assert!(PanLaw::try_from(f64::NAN).is_err());
        assert!(PanLawMode::try_from(i32::MAX).is_err());
        assert!(Position::try_from_seconds(f64::NAN).is_err());
        assert_eq!(
            Position::try_from_seconds(-1.0).unwrap().as_seconds_f64(),
            -1.0
        );
        assert!(Position::try_from_seconds(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn get_length_rejects_end_before_start() {
        let start = Position::from(Duration::from_secs(2));
        let end = Position::from(Duration::from_secs(1));
        assert!(end.get_length(start).is_err());
        assert_eq!(
            Duration::from_secs(1).get_length(start).unwrap(),
            Duration::from_secs(1)
        );
    }
}

/// Position in project.
///
/// Internally holds a signed [TimeDelta] from project start.
/// Keeps interfaces to all time transformations (e.g. between secs, quarters
/// and ppq)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub(crate) offset: TimeDelta,
}
impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset
    }
}
impl Eq for Position {}
impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
impl Hash for Position {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.offset.hash(state)
    }
}
impl From<Position> for f64 {
    fn from(value: Position) -> Self {
        value.as_seconds_f64()
    }
}
impl Default for Position {
    fn default() -> Self {
        Self {
            offset: TimeDelta::zero(),
        }
    }
}
impl From<Duration> for Position {
    fn from(value: Duration) -> Self {
        Self {
            offset: TimeDelta::from_std(value).unwrap(),
        }
    }
}
impl From<TimeDelta> for Position {
    fn from(value: TimeDelta) -> Self {
        Self { offset: value }
    }
}
impl TryFrom<Position> for Duration {
    type Error = ReaRsError;

    fn try_from(value: Position) -> Result<Self, Self::Error> {
        value.offset.to_std().map_err(|_| {
            ReaRsError::InvalidObject("negative position is not a Duration")
        })
    }
}
impl Position {
    pub fn from_seconds(seconds: f64) -> ReaperResult<Self> {
        Self::try_from_seconds(seconds)
    }

    /// Creates a position from a signed offset from project start.
    pub fn new(duration_from_project_start: Duration) -> Self {
        Self::from(duration_from_project_start)
    }
    pub fn try_from_seconds(value: f64) -> ReaperResult<Self> {
        Self::from_secs_f64(value)
    }

    /// Create a signed position or source offset from seconds.
    pub fn from_secs_f64(secs: f64) -> ReaperResult<Self> {
        if !secs.is_finite() {
            return Err(ReaRsError::InvalidObject("position must be finite"));
        }
        let micros = secs * 1_000_000.0;
        if micros < i64::MIN as f64 || micros > i64::MAX as f64 {
            return Err(ReaRsError::InvalidObject("position is out of range"));
        }
        Ok(Self {
            offset: TimeDelta::microseconds(micros.round() as i64),
        })
    }

    /// Convert a native seconds value into a signed project position.
    pub fn from_host_seconds(value: f64) -> ReaperResult<Self> {
        Self::try_from_seconds(value)
    }
    pub fn as_seconds_f64(&self) -> f64 {
        self.as_secs_f64()
    }
    /// Return the signed offset as seconds.
    pub fn as_secs_f64(&self) -> f64 {
        self.offset.num_microseconds().unwrap_or_else(|| {
            if self.offset < TimeDelta::zero() {
                i64::MIN
            } else {
                i64::MAX
            }
        }) as f64
            / 1_000_000.0
    }
    /// Access the signed time delta represented by this position.
    pub fn get(&self) -> TimeDelta {
        self.offset
    }
    pub fn try_as_duration(&self) -> ReaperResult<Duration> {
        Duration::try_from(*self)
    }
    /// Add a non-negative duration to this signed position.
    pub fn add_duration(self, duration: Duration) -> Self {
        self + duration
    }

    /// Round this position to the nearest multiple of `precision`.
    ///
    /// Precision must be a non-zero whole number of microseconds because
    /// positions are stored with microsecond precision. Midpoint values are
    /// rounded away from zero. This is explicit rather than part of `Eq` or
    /// `Ord`, so rounded comparisons cannot silently change ordering behavior.
    pub fn rounded_to(self, precision: Duration) -> ReaperResult<Self> {
        let precision_nanos = precision.as_nanos();
        if precision_nanos == 0 || precision_nanos % 1_000 != 0 {
            return Err(ReaRsError::InvalidObject(
                "position precision must be a whole number of microseconds",
            ));
        }
        let precision_micros = i128::try_from(precision_nanos / 1_000)
            .map_err(|_| {
                ReaRsError::InvalidObject("position precision out of range")
            })?;
        let value_micros =
            i128::from(self.offset.num_microseconds().ok_or(
                ReaRsError::InvalidObject("position is out of range"),
            )?);
        let rounded_micros = if value_micros < 0 {
            -(((-value_micros + precision_micros / 2) / precision_micros)
                * precision_micros)
        } else {
            ((value_micros + precision_micros / 2) / precision_micros)
                * precision_micros
        };
        let rounded_micros = i64::try_from(rounded_micros).map_err(|_| {
            ReaRsError::InvalidObject("rounded position is out of range")
        })?;
        Ok(Self {
            offset: TimeDelta::microseconds(rounded_micros),
        })
    }

    /// Compare positions after rounding both to the requested precision.
    pub fn eq_with_precision(
        self,
        other: Self,
        precision: Duration,
    ) -> ReaperResult<bool> {
        Ok(self.rounded_to(precision)? == other.rounded_to(precision)?)
    }
    pub fn as_ppq(&self, take: &Take) -> Result<i64, ReaRsError> {
        let ppq = unsafe {
            Reaper::get().low().MIDI_GetPPQPosFromProjTime(
                take.get()?.as_mut(),
                self.as_seconds_f64(),
            )
        };
        if !ppq.is_finite() || ppq < i64::MIN as f64 || ppq >= i64::MAX as f64
        {
            return Err(ReaRsError::InvalidObject(
                "PPQ position out of range",
            ));
        }
        Ok(ppq.round() as i64)
    }

    pub fn from_ppq(ppq: i64, take: &Take) -> Result<Self, ReaRsError> {
        let val = unsafe {
            Reaper::get()
                .low()
                .MIDI_GetProjTimeFromPPQPos(take.get()?.as_mut(), ppq as f64)
        };
        Self::from_host_seconds(val)
    }
}
impl Add for Position {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            offset: self.offset + rhs.offset,
        }
    }
}
impl Add<Duration> for Position {
    type Output = Position;

    fn add(self, rhs: Duration) -> Self::Output {
        Position {
            offset: self.offset + TimeDelta::from_std(rhs).unwrap(),
        }
    }
}
impl Sub for Position {
    type Output = Position;

    fn sub(self, rhs: Self) -> Self::Output {
        Position {
            offset: self.offset - rhs.offset,
        }
    }
}
impl Sub<Duration> for Position {
    type Output = Position;

    fn sub(self, rhs: Duration) -> Self::Output {
        Position {
            offset: self.offset - TimeDelta::from_std(rhs).unwrap(),
        }
    }
}
impl Position {
    /// Return the signed difference between two positions.
    pub fn difference(self, rhs: Position) -> TimeDelta {
        self.offset - rhs.offset
    }

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
        Self::from_host_seconds(seconds)
    }

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
}

pub trait GetLength {
    fn get_length(&self, start: Position) -> ReaperResult<Duration>;
}

impl GetLength for Position {
    fn get_length(&self, start: Position) -> ReaperResult<Duration> {
        if self < &start {
            return Err(ReaRsError::InvalidObject("end precedes start"));
        }
        Ok((self.offset - start.offset).to_std().unwrap())
    }
}
impl Position {
    pub fn get_length(&self, start: Position) -> ReaperResult<Duration> {
        GetLength::get_length(self, start)
    }
}

#[cfg(test)]
mod get_length_tests {
    use super::{GetLength, Position};
    use std::time::Duration;

    #[test]
    fn rejects_end_before_start() {
        let start = Position::from(Duration::from_secs(2));
        let end = Position::from(Duration::from_secs(1));
        assert!(end.get_length(start).is_err());
        assert_eq!(
            Duration::from_secs(1).get_length(start).unwrap(),
            Duration::from_secs(1)
        );
    }
}

#[cfg(test)]
mod signed_position_tests {
    use super::Position;
    use chrono::TimeDelta;
    use std::time::Duration;

    #[test]
    fn signed_seconds_round_trip() {
        for seconds in [-120.25, -0.001, 0.0, 7.125] {
            let position = Position::try_from_seconds(seconds).unwrap();
            assert!((position.as_seconds_f64() - seconds).abs() < 1e-9);
        }
        assert!(Position::try_from_seconds(f64::INFINITY).is_err());
        assert!(Position::try_from_seconds(f64::NAN).is_err());
    }

    #[test]
    fn signed_position_arithmetic_crosses_project_origin() {
        let before = Position::from_seconds(-0.25).unwrap();
        let after = Position::from_seconds(0.5).unwrap();
        assert_eq!(
            before
                .add_duration(std::time::Duration::from_millis(500))
                .as_seconds_f64(),
            0.25
        );
        assert_eq!(after.difference(before), TimeDelta::milliseconds(750));
        assert!(before.try_as_duration().is_err());
    }

    #[test]
    fn position_retains_signed_offset_value_api() {
        for seconds in [2.0, -2.0, -2.543] {
            let position = Position::from_secs_f64(seconds).unwrap();
            assert_eq!(position.as_secs_f64(), seconds);
            assert_eq!(position.get(), position.offset);
        }

        let positive = Position::from_secs_f64(2.0).unwrap();
        let negative = Position::from_secs_f64(-2.0).unwrap();
        assert_eq!(
            Duration::try_from(positive).unwrap(),
            Duration::from_secs(2)
        );
        assert!(Duration::try_from(negative).is_err());
    }

    #[test]
    fn position_can_be_compared_at_explicit_precision() {
        let precision = Duration::from_micros(100);
        let left = Position::from_secs_f64(1.000_001).unwrap();
        let near = Position::from_secs_f64(1.000_049).unwrap();
        let midpoint = Position::from_secs_f64(1.000_050).unwrap();
        assert!(left.eq_with_precision(near, precision).unwrap());
        assert!(!left.eq_with_precision(midpoint, precision).unwrap());

        let negative = Position::from_secs_f64(-1.000_050).unwrap();
        assert_eq!(
            negative.rounded_to(precision).unwrap().as_secs_f64(),
            -1.000_100
        );
        assert!(left.rounded_to(Duration::ZERO).is_err());
        assert!(left.rounded_to(Duration::from_nanos(1)).is_err());
    }
}
impl GetLength for Duration {
    fn get_length(&self, _start: Position) -> ReaperResult<Duration> {
        Ok(*self)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Volume {
    raw: f64,
}
impl Volume {
    pub fn new(raw: f64) -> ReaperResult<Self> {
        Self::try_from(raw)
    }

    pub fn get(&self) -> f64 {
        self.raw
    }
    pub fn from_db(db: f64) -> Self {
        Self {
            raw: 10.0_f64.powf(db / 20.0),
        }
    }
    pub fn as_db(&self) -> f64 {
        20.0 * self.raw.log10()
    }
}
impl TryFrom<f64> for Volume {
    type Error = ReaRsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || value < 0.0 {
            return Err(ReaRsError::InvalidObject(
                "volume must be finite and non-negative",
            ));
        }
        Ok(Self { raw: value })
    }
}
impl Into<f64> for Volume {
    fn into(self) -> f64 {
        self.raw
    }
}

#[test]
fn test_volume() {
    assert_eq!(Volume::try_from(0.0).unwrap().as_db(), -f64::INFINITY);
    assert_eq!(Volume::try_from(0.5).unwrap().as_db().trunc(), -6.0);
    assert_eq!(Volume::try_from(1.0).unwrap().as_db(), 0.0);
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Pan {
    raw: f64,
}
impl Pan {
    pub fn new(raw: f64) -> ReaperResult<Self> {
        Self::try_from(raw)
    }

    pub fn get(&self) -> f64 {
        self.raw
    }
}
impl TryFrom<f64> for Pan {
    type Error = ReaRsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
            return Err(ReaRsError::InvalidObject(
                "pan must be finite and between -1 and 1",
            ));
        }
        Ok(Self { raw: value })
    }
}
impl Into<f64> for Pan {
    fn into(self) -> f64 {
        self.raw
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub enum PanLaw {
    Default,
    Minus6dB,
    Minus3dB,
    Zero,
    Minus3dBCompensated,
    Minus6dBCompensated,
}
impl TryFrom<f64> for PanLaw {
    type Error = ReaRsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(ReaRsError::InvalidObject("unknown pan law"));
        }
        match value {
            x if x < 0.0 => Ok(Self::Default),
            x if (x - 0.5).abs() <= f64::EPSILON => Ok(Self::Minus6dB),
            x if (x - 0.707).abs() <= f64::EPSILON => Ok(Self::Minus3dB),
            x if (x - 1.0).abs() <= f64::EPSILON => Ok(Self::Zero),
            x if (x - 1.414).abs() <= f64::EPSILON => {
                Ok(Self::Minus3dBCompensated)
            }
            x if (x - 2.0).abs() <= f64::EPSILON => {
                Ok(Self::Minus6dBCompensated)
            }
            _ => Err(ReaRsError::InvalidObject("unknown pan law")),
        }
    }
}
impl Into<f64> for PanLaw {
    fn into(self) -> f64 {
        match self {
            Self::Default => -1.0,
            Self::Minus6dB => 0.5,
            Self::Minus3dB => 0.707,
            Self::Zero => 1.0,
            Self::Minus3dBCompensated => 1.414,
            Self::Minus6dBCompensated => 2.0,
        }
    }
}

#[repr(i32)]
#[derive(
    Debug, PartialEq, Eq, Clone, Copy, IntEnum, Serialize, Deserialize,
)]
pub enum PanLawMode {
    SineTaper = 0,
    HybridTaperDeprecated = 1,
    LinearTaper = 2,
    HybridTaper = 3,
}
impl TryFrom<i32> for PanLawMode {
    type Error = ReaRsError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_int(value)
            .map_err(|_| ReaRsError::InvalidObject("unknown pan law mode"))
    }
}
impl Into<i32> for PanLawMode {
    fn into(self) -> i32 {
        self.int_value()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Pitch {
    raw: f64,
}
impl Pitch {
    pub fn get(&self) -> f64 {
        self.raw
    }
}
impl From<f64> for Pitch {
    fn from(value: f64) -> Self {
        Self { raw: value }
    }
}
impl Into<f64> for Pitch {
    fn into(self) -> f64 {
        self.raw
    }
}

/// Project playback rate.
///
/// Normally, represents multiplication factor to project tempo.
///
/// Can be normalized into slider range values (0.0 .. 1.0)
///
/// # Example
/// ```no_run
/// use rea_rs::PlayRate;
/// let plrt = PlayRate::from(0.25);
/// assert_eq!(plrt.normalized(), 0.0);
///
/// let plrt = PlayRate::from(4.0);
/// assert_eq!(plrt.normalized(), 1.0);
///
/// let plrt = PlayRate::from(1.0);
/// assert_eq!(plrt.normalized(), 0.2);
///
/// let plrt = PlayRate::from(2.5);
/// assert_eq!(plrt.normalized(), 0.6);
/// ```
///
/// [Project::get_play_rate]
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct PlayRate {
    raw: f64,
}
impl PlayRate {
    /// Convert to slider value.
    pub fn normalized(&self) -> f64 {
        Reaper::get()
            .low()
            .Master_NormalizePlayRate(self.raw, false)
    }

    /// Create from slider value.
    pub fn from_normalized(value: f64) -> Self {
        let raw = Reaper::get().low().Master_NormalizePlayRate(value, true);
        PlayRate::from(raw)
    }
}
impl From<f64> for PlayRate {
    fn from(value: f64) -> Self {
        PlayRate { raw: value }
    }
}
impl Into<f64> for PlayRate {
    fn into(self) -> f64 {
        self.raw
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeRangeKind {
    TimeSelection,
    LoopSelection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeRange<'a> {
    project: &'a Project,
    kind: TimeRangeKind,
}
impl<'a> TimeRange<'a> {
    pub fn new(project: &'a Project, kind: TimeRangeKind) -> Self {
        Self { project, kind }
    }

    pub fn get_kind(&self) -> TimeRangeKind {
        return self.kind;
    }

    pub fn get(&self) -> ReaperResult<(Position, Position)> {
        unsafe {
            let is_loop = match self.kind {
                TimeRangeKind::LoopSelection => true,
                TimeRangeKind::TimeSelection => false,
            };
            let (mut start, mut end) =
                (MaybeUninit::zeroed(), MaybeUninit::zeroed());
            Reaper::get().low().GetSet_LoopTimeRange2(
                self.project.context().to_raw(),
                false,
                is_loop,
                start.as_mut_ptr(),
                end.as_mut_ptr(),
                false,
            );
            Ok((
                Position::from_host_seconds(start.assume_init())?,
                Position::from_host_seconds(end.assume_init())?,
            ))
        }
    }

    pub fn set(&self, start: Position, end: Position) {
        unsafe {
            let is_loop = match self.kind {
                TimeRangeKind::LoopSelection => true,
                TimeRangeKind::TimeSelection => false,
            };
            let (mut start, mut end) = (
                MaybeUninit::new(start.as_seconds_f64()),
                MaybeUninit::new(end.as_seconds_f64()),
            );
            Reaper::get().low().GetSet_LoopTimeRange2(
                self.project.context().to_raw(),
                true,
                is_loop,
                start.as_mut_ptr(),
                end.as_mut_ptr(),
                false,
            );
        }
    }

    pub fn get_start(&self) -> ReaperResult<Position> {
        self.get().map(|range| range.0)
    }
    pub fn get_end(&self) -> ReaperResult<Position> {
        self.get().map(|range| range.1)
    }

    pub fn set_start(&self, start: Position) -> ReaperResult<()> {
        let end = self.get()?.1;
        if start > end {
            return Err(ReaRsError::InvalidObject(
                "time range end precedes start",
            ));
        }
        self.set(start, end);
        Ok(())
    }
    pub fn set_end(&self, end: Position) -> ReaperResult<()> {
        let start = self.get()?.0;
        if end < start {
            return Err(ReaRsError::InvalidObject(
                "time range end precedes start",
            ));
        }
        self.set(start, end);
        Ok(())
    }

    pub fn get_length(&self) -> ReaperResult<Duration> {
        let (start, end) = self.get()?;
        end.get_length(start)
    }
    pub fn set_length(&self, length: Duration) -> ReaperResult<()> {
        let start = self.get()?.0;
        let end = start.add_duration(length);
        self.set(start, end);
        Ok(())
    }

    /// Move selection left or right.
    ///
    /// Returns true if snap is enabled.
    pub fn shift(&self, direction: Direction) -> bool {
        let low = Reaper::get().low();
        unsafe {
            match direction {
                Direction::Right => {
                    low.Loop_OnArrow(self.project.context().to_raw(), 1)
                }
                Direction::Left => {
                    low.Loop_OnArrow(self.project.context().to_raw(), -1)
                }
            }
        }
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

pub trait KnowsProject {
    fn project(&self) -> Project;
}

/// GUID, that can help to track object, without knowing it's pointer and
/// index.
///
/// # Note
///
/// Sorry, but i does not support serializing yet. So, for using with ExtState,
/// ith should be converted  to\from String.
///
/// [GUID::from_string], [GUID::to_string]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GUID {
    raw: rea_rs_low::raw::GUID,
}
impl Into<String> for GUID {
    fn into(self) -> String {
        self.to_string()
            .expect("GUID formatting requires initialized REAPER")
    }
}
impl GUID {
    pub fn to_string(&self) -> ReaperResult<String> {
        let mut buf = vec![0_i8; 64];
        unsafe {
            Reaper::get()
                .low()
                .guidToString(&self.raw, buf.as_mut_ptr())
        };
        string_from_buf(&buf)
    }

    pub fn from_string(value: String) -> ReaperResult<Self> {
        let mut g = MaybeUninit::zeroed();
        // debug!("GUID.from_string(): {value}");
        let value_cstring = CString::new(value)?;
        unsafe {
            Reaper::get()
                .low()
                .stringToGuid(value_cstring.as_ptr(), g.as_mut_ptr());
            let g = g.assume_init();
            Ok(Self { raw: g })
        }
    }
    pub fn new() -> Self {
        unsafe {
            let mut ptr = MaybeUninit::zeroed();
            Reaper::get().low().genGuid(ptr.as_mut_ptr());
            Self {
                raw: ptr.assume_init(),
            }
        }
    }
}
