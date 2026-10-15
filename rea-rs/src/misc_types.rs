use crate::{
    utils::string_from_buf, Project, ReaRsError, Reaper, ReaperResult,
};
use int_enum::IntEnum;
use serde_derive::{Deserialize, Serialize};
use std::{ffi::CString, mem::MaybeUninit};

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

    use crate::{Pan, PanLaw, PanLawMode, Volume};

    #[test]
    fn checked_numeric_conversions_reject_invalid_values() {
        assert!(Volume::try_from(f64::NAN).is_err());
        assert!(Volume::try_from(-1.0).is_err());
        assert!(Pan::try_from(f64::INFINITY).is_err());
        assert!(Pan::try_from(1.1).is_err());
        assert!(PanLaw::try_from(f64::NAN).is_err());
        assert!(PanLawMode::try_from(i32::MAX).is_err());
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
