use int_enum::IntEnum;

use crate::{
    errors::{ReaperError, ReaperResult},
    HardwareSocket, Reaper,
};

pub enum Section {
    Main,
    Id(u32),
}
impl Section {
    pub fn id(&self) -> u32 {
        match self {
            Self::Main => 0,
            Self::Id(id) => *id,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum AutomationMode {
    None = -1,
    TrimRead = 0,
    Read = 1,
    Touch = 2,
    Write = 3,
    Latch = 4,
    Bypass = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum MessageBoxType {
    Ok = 0,
    OkCancel = 1,
    AbortRetryIgnore = 2,
    YesNoCancel = 3,
    YesNo = 4,
    RetryCancel = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum MessageBoxValue {
    Ok = 1,
    Cancel = 2,
    Abort = 3,
    Retry = 4,
    Ignore = 5,
    Yes = 6,
    No = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Right,
    Left,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum SoloMode {
    NotSoloed = 0,
    Soloed = 1,
    SoloedInPlace = 2,
    SafeSoloed = 5,
    SafeSoloedInPlace = 6,
}

/// Track recording mode
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
pub enum RecMode {
    Input = 0,
    StereoOut = 1,
    None = 2,
    StereoOutWithLatencyComp = 3,
    MidiOut = 4,
    MonoOut = 5,
    MonoOutWithLatencyComp = 6,
    MidiOverdub = 7,
    MidiReplace = 8,
}

/// Track recording input.
#[derive(Debug, Clone, PartialEq)]
pub enum RecInput {
    /// MIDI Channel (`0` → all), HardwareSocket (`None` → all).
    /// Can hold special socket: `HardwareSocket{62, "Virtual Keyboard"}`.
    MIDI(u8, Option<HardwareSocket>),
    /// channel offset, is from rea_route
    Mono(u32, bool),
    /// channel offset, is from rea_route
    Stereo(u32, bool),
    /// channel offset, is from rea_route
    Multichannel(u32, bool),
}
impl RecInput {
    fn pack_rea_route(rea_route: bool, ch: u32) -> u32 {
        match rea_route {
            true => ch + 512,
            false => ch,
        }
    }

    pub fn from_raw(value: f64) -> ReaperResult<Self> {
        if value < 0.0 {
            return Err(ReaperError::InvalidObject(
                "Can not convert to RecordingInput",
            )
            .into());
        }
        let value = value as u32;
        if value & 4096 > 0 {
            let channel = value & 0b11111;
            let hw_idx = value << 5 & 0b111111;
            let socket = match hw_idx {
                63 => None,
                62 => HardwareSocket::new(62, "Virtual Keyboard").into(),
                x => Reaper::get().get_midi_input(x as usize),
            };
            Ok(Self::MIDI(channel as u8, socket))
        } else {
            let mut offset = value & 1023;
            let rea_route = value >= 512;
            if rea_route {
                offset -= 512
            };
            match value & 2048 > 0 {
                true => Ok(Self::Multichannel(offset, rea_route)),
                false => match value & 1024 > 0 {
                    true => Ok(Self::Stereo(offset, rea_route)),
                    false => Ok(Self::Mono(offset, rea_route)),
                },
            }
        }
    }
    pub fn to_raw(self) -> f64 {
        let mut is_midi = 0;
        let mut is_stereo = 0;
        let mut is_multichannel = 0;
        let value: u32 = match self {
            Self::MIDI(ch, socket) => {
                is_midi = 4096;
                let socket = match socket {
                    None => 63,
                    Some(x) => x.index(),
                };
                ch as u32 + socket >> 5
            }
            Self::Mono(ch, rea_route) => Self::pack_rea_route(rea_route, ch),
            Self::Stereo(ch, rea_route) => {
                is_stereo = 1024;
                Self::pack_rea_route(rea_route, ch)
            }
            Self::Multichannel(ch, rea_route) => {
                is_multichannel = 2048;
                Self::pack_rea_route(rea_route, ch)
            }
        };
        let value = value | is_midi | is_stereo | is_multichannel;
        value as f64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecOutMode {
    PostFader,
    PreFX,
    /// pre-fader
    PostFX,
}
impl RecOutMode {
    pub fn from_raw(mode: u32) -> Option<Self> {
        match mode {
            0 => Self::PostFader.into(),
            1 => Self::PreFX.into(),
            2 => Self::PostFX.into(),
            _ => None,
        }
    }
    pub fn to_raw(&self) -> u32 {
        let value = match self {
            Self::PostFader => 0,
            Self::PreFX => 1,
            Self::PostFX => 2,
        };
        value
    }
}

/// Track VU Mode.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VUMode {
    Disabled,
    StereoPeaks,
    MultichannelPeaks,
    StereoRMS,
    CombinedRMS,
    LUFS_M,
    LUFS_S_ReadoutMax,
    LUFS_S_ReadoutCurrent,
    LUFS_OnChannels_1_2,
}
impl VUMode {
    pub fn from_raw(raw: u32) -> Self {
        if raw & 1 == 1 {
            return Self::Disabled;
        }
        if raw & 32 == 32 {
            return Self::LUFS_OnChannels_1_2;
        }
        match raw & 30 {
            0 => Self::StereoPeaks,
            2 => Self::MultichannelPeaks,
            4 => Self::StereoRMS,
            8 => Self::CombinedRMS,
            12 => Self::LUFS_M,
            16 => Self::LUFS_S_ReadoutMax,
            20 => Self::LUFS_S_ReadoutCurrent,
            x => panic!("Can not convert value {} to VUMode!", x),
        }
    }
    pub fn to_raw(&self) -> u32 {
        match self {
            Self::Disabled => 1,
            Self::LUFS_OnChannels_1_2 => 32,
            Self::StereoPeaks => 0 | 30,
            Self::MultichannelPeaks => 2 | 30,
            Self::StereoRMS => 4 | 30,
            Self::CombinedRMS => 8 | 30,
            Self::LUFS_M => 12 | 30,
            Self::LUFS_S_ReadoutMax => 16 | 30,
            Self::LUFS_S_ReadoutCurrent => 20 | 30,
        }
    }
}

/// Represents relations of the track in folders structure.
///
/// The whole folder hierarchy can be build, probably, only with full-project
/// iteration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackFolderState {
    /// is between two tracks.
    Normal,
    /// 0 → normal, 1 → small, 2 → smallest.
    IsFolder(u32),
    /// depth of «going up»
    ///
    /// `Last(1)` → 1 level up, `Last(2)` → 2 levels up etc.
    Last(u32),
}
impl TrackFolderState {
    pub fn from_raw(depth: i32, compact: u32) -> Self {
        if depth == 0 {
            return Self::Normal;
        }
        if depth == 1 {
            return Self::IsFolder(compact);
        }
        if depth > 1 {
            panic!("Can not convert value {} to TrackFolderState", depth);
        }
        Self::Last(depth.abs() as u32)
    }
    /// get depth and compact
    pub fn to_raw(self) -> (i32, Option<u32>) {
        match self {
            Self::Normal => (0, None),
            Self::IsFolder(compact) => (1, compact.into()),
            Self::Last(depth) => (-(depth as i32), None),
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, IntEnum)]
pub enum TimeMode {
    /// Project default
    Default = -1,
    Time = 0,
    /// position length rate
    BeatsFull = 1,
    BeatsOnlyPosition = 2,
}
