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
