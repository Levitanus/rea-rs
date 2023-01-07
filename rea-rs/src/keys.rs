use bitflags::bitflags;
use int_enum::IntEnum;
use rea_rs_low::raw;

bitflags! {
    /// Represents modifier in keybinding.
    /// Equal to Win32 ACCEL fVirt parameter.
    pub struct FVirt:u8{
        /// If Alt has to be pressed
        const FALT = 0x10;

        /// If Control has to be pressed
        const FCONTROL = 0x08;

        /// No top-level menu item is highlighted when
        /// the accelerator is used. If this flag is not specified,
        /// a top-level menu item will be highlighted, if possible,
        /// when the accelerator is used. This attribute is obsolete
        /// and retained only for backward compatibility with
        /// resource files designed for 16-bit Windows.
        const FNOINVERT = 0x02;

        /// If Shift has to be pressed.
        const FSHIFT = 0x04;

        /// If the key should be considered as virtual key.
        /// Otherwise it will be considered as character.
        ///
        /// For the keybindings this flag has to be used, otherwise
        /// they will not work on the alternative keyboard layout.
        const FVIRTKEY = 0x01;
    }
}

/// Represents keyboard keys, independent of the keyboard layout.
#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(
    Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Copy, Clone, IntEnum,
)]
pub enum VKeys {
    VK_ADD = raw::VK_ADD,
    VK_BACK = raw::VK_BACK,
    VK_CAPITAL = raw::VK_CAPITAL,
    VK_CLEAR = raw::VK_CLEAR,
    VK_CONTROL = raw::VK_CONTROL,
    VK_DECIMAL = raw::VK_DECIMAL,
    VK_DELETE = raw::VK_DELETE,
    VK_DIVIDE = raw::VK_DIVIDE,
    VK_DOWN = raw::VK_DOWN,
    VK_END = raw::VK_END,
    VK_ESCAPE = raw::VK_ESCAPE,
    VK_F1 = raw::VK_F1,
    VK_F10 = raw::VK_F10,
    VK_F11 = raw::VK_F11,
    VK_F12 = raw::VK_F12,
    VK_F13 = raw::VK_F13,
    VK_F14 = raw::VK_F14,
    VK_F15 = raw::VK_F15,
    VK_F16 = raw::VK_F16,
    VK_F17 = raw::VK_F17,
    VK_F18 = raw::VK_F18,
    VK_F19 = raw::VK_F19,
    VK_F2 = raw::VK_F2,
    VK_F20 = raw::VK_F20,
    VK_F21 = raw::VK_F21,
    VK_F22 = raw::VK_F22,
    VK_F23 = raw::VK_F23,
    VK_F24 = raw::VK_F24,
    VK_F3 = raw::VK_F3,
    VK_F4 = raw::VK_F4,
    VK_F5 = raw::VK_F5,
    VK_F6 = raw::VK_F6,
    VK_F7 = raw::VK_F7,
    VK_F8 = raw::VK_F8,
    VK_F9 = raw::VK_F9,
    VK_HELP = raw::VK_HELP,
    VK_HOME = raw::VK_HOME,
    VK_INSERT = raw::VK_INSERT,
    VK_LBUTTON = raw::VK_LBUTTON,
    VK_LEFT = raw::VK_LEFT,
    VK_LWIN = raw::VK_LWIN,
    VK_MBUTTON = raw::VK_MBUTTON,
    VK_MENU = raw::VK_MENU,
    VK_MULTIPLY = raw::VK_MULTIPLY,
    VK_NEXT = raw::VK_NEXT,
    VK_NUMLOCK = raw::VK_NUMLOCK,
    VK_NUMPAD0 = raw::VK_NUMPAD0,
    VK_NUMPAD1 = raw::VK_NUMPAD1,
    VK_NUMPAD2 = raw::VK_NUMPAD2,
    VK_NUMPAD3 = raw::VK_NUMPAD3,
    VK_NUMPAD4 = raw::VK_NUMPAD4,
    VK_NUMPAD5 = raw::VK_NUMPAD5,
    VK_NUMPAD6 = raw::VK_NUMPAD6,
    VK_NUMPAD7 = raw::VK_NUMPAD7,
    VK_NUMPAD8 = raw::VK_NUMPAD8,
    VK_NUMPAD9 = raw::VK_NUMPAD9,
    VK_PAUSE = raw::VK_PAUSE,
    VK_PRINT = raw::VK_PRINT,
    VK_PRIOR = raw::VK_PRIOR,
    VK_RBUTTON = raw::VK_RBUTTON,
    VK_RETURN = raw::VK_RETURN,
    VK_RIGHT = raw::VK_RIGHT,
    VK_SCROLL = raw::VK_SCROLL,
    VK_SELECT = raw::VK_SELECT,
    VK_SEPARATOR = raw::VK_SEPARATOR,
    VK_SHIFT = raw::VK_SHIFT,
    VK_SNAPSHOT = raw::VK_SNAPSHOT,
    VK_SPACE = raw::VK_SPACE,
    VK_SUBTRACT = raw::VK_SUBTRACT,
    VK_TAB = raw::VK_TAB,
    VK_UP = raw::VK_UP,
    VK_0 = 0x30,
    VK_1 = 0x31,
    VK_2 = 0x32,
    VK_3 = 0x33,
    VK_4 = 0x34,
    VK_5 = 0x35,
    VK_6 = 0x36,
    VK_7 = 0x37,
    VK_8 = 0x38,
    VK_9 = 0x39,
    VK_A = 0x41,
    VK_B = 0x42,
    VK_C = 0x43,
    VK_D = 0x44,
    VK_E = 0x45,
    VK_F = 0x46,
    VK_G = 0x47,
    VK_H = 0x48,
    VK_I = 0x49,
    VK_J = 0x4A,
    VK_K = 0x4B,
    VK_L = 0x4C,
    VK_M = 0x4D,
    VK_N = 0x4E,
    VK_O = 0x4F,
    VK_P = 0x50,
    VK_Q = 0x51,
    VK_R = 0x52,
    VK_S = 0x53,
    VK_T = 0x54,
    VK_U = 0x55,
    VK_V = 0x56,
    VK_W = 0x57,
    VK_X = 0x58,
    VK_Y = 0x59,
    VK_Z = 0x5A,
}

/// Combination of modifier flags and key.
/// The key can be one of [VKeys], or anything else, not represented there.
/// If `fvirt=0`, the key is, probably, considered as unicode character.
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Copy, Clone)]
pub struct KeyBinding {
    pub fvirt: FVirt,
    pub key: u16,
}
impl KeyBinding {
    pub fn new(fvirt: FVirt, key: u16) -> Self {
        Self { fvirt, key }
    }
}

/// For now — just raw keystroke lParam
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Copy, Clone)]
pub struct KeyStroke {
    pub raw: u32,
}
impl From<isize> for KeyStroke {
    fn from(value: isize) -> Self {
        Self { raw: value as u32 }
    }
}
