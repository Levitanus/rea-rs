use int_enum::IntEnum;

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
pub enum RecordingMode {
    Input = 0,
    StereoInput = 1,
    None = 2,
    StereoOutWithLatencyComp = 3,
    MidiOut = 4,
    MonoOut = 5,
    MonoOutWithLatencyComp = 6,
    MidiOverdub = 7,
    MidiReplace = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingOutMode {
    PostFader,
    PreFX,
    /// pre-fader
    PostFX,
}
impl RecordingOutMode {
    pub fn from_raw(mode: u32) -> Option<Self> {
        if mode & 3 == 0 {
            return None;
        }
        match mode & !3 {
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
        value | 3
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
