use super::layout::WidgetSize;
use super::ReaperWindow;
use crate::{
    swell_gui::{
        layout::{Axis, OverflowPolicy, Panel, PanelLayout, PanelRects},
        windows::{LayoutNode, ScrollViewRuntime},
    },
    ReaRsError, Reaper, ReaperResult, ScrollState, ScrollViewEvent,
    ScrollbarRenderer,
};
use rea_rs_low::raw;
use std::{cell::RefCell, collections::HashMap, ffi::CString, rc::Rc};

mod raw_hwnd_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(
        hwnd: &rea_rs_low::raw::HWND,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*hwnd as isize as i64)
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<rea_rs_low::raw::HWND, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = isize::try_from(i64::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)?;
        Ok(value as rea_rs_low::raw::HWND)
    }
}

/// A numeric SWELL identifier used by controls, menus, and timers.
///
/// Construct explicit stable values with the tuple constructor (including in
/// `const` declarations). `new()` and the domain-specific allocation helpers
/// provide process-local values, but do not coordinate with IDs allocated
/// outside this crate. Native scope and range rules still apply at each API
/// boundary.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct SwellId(pub u32);

impl SwellId {
    /// Allocates a process-local identifier from the shared SWELL ID sequence.
    pub fn new() -> Self {
        Self::new_control()
    }

    /// Allocates a process-local control ID in the conventional child-control
    /// range used by this crate.
    pub fn new_control() -> Self {
        Self(Self::allocate_from(&NEXT_CONTROL_ID))
    }

    /// Allocates a process-local menu command ID.
    pub fn new_menu() -> Self {
        Self(Self::allocate_from(&NEXT_MENU_ID))
    }

    /// Allocates a process-local timer ID.
    pub fn new_timer() -> Self {
        Self(Self::allocate_from(&NEXT_TIMER_ID))
    }

    /// Returns the native 32-bit representation.
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    fn allocate_from(counter: &std::sync::atomic::AtomicU32) -> u32 {
        use std::sync::atomic::Ordering;
        counter.fetch_add(1, Ordering::Relaxed)
    }
}

static NEXT_CONTROL_ID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(10_000);
static NEXT_MENU_ID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x4000);
static NEXT_TIMER_ID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(1);
impl From<u32> for SwellId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<SwellId> for u32 {
    fn from(value: SwellId) -> Self {
        value.0
    }
}

impl From<SwellId> for i32 {
    fn from(value: SwellId) -> Self {
        value.0 as i32
    }
}

/// Control families understood by the high-level event decoder.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum ControlKind {
    Button,
    CheckBox,
    RadioButton,
    EditField,
    ComboBox,
    ListBox,
    Static,
    Trackbar,
    ProgressBar,
    Tab,
    ListView,
    TreeView,
}

/// Runtime binding between a logical/native control ID and its current HWND.
///
/// This is a non-owning snapshot. Its HWND can become stale after native
/// destruction or recreation, and constructing this value does not validate
/// that the ID, kind, or handle belong together.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct ControlHandle {
    /// Logical/native child control ID.
    pub id: SwellId,
    /// Control family used when decoding native notifications.
    pub kind: ControlKind,
    /// Current raw SWELL/Win32 handle; may be null or stale.
    #[serde(with = "raw_hwnd_serde")]
    pub hwnd: raw::HWND,
}

/// Rectangle used by control positioning helpers, in parent-client pixels.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct ControlRect {
    /// Left coordinate relative to the parent client area.
    pub x: i32,
    /// Top coordinate relative to the parent client area.
    pub y: i32,
    /// Positive width in pixels.
    pub width: i32,
    /// Positive height in pixels.
    pub height: i32,
}

/// Native edit-field intent options mapped to each backend's supported styles.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct EditFieldOptions {
    /// Create a multiline text view. On macOS this selects SWELL's text-view
    /// path through its vertical/horizontal scrolling style bits.
    pub multiline: bool,
    /// Make the control read-only.
    pub read_only: bool,
    /// Accept Return in multiline mode where supported.
    pub want_return: bool,
    /// Wrap text rather than requiring horizontal scrolling, where supported.
    pub word_wrap: bool,
    /// Request vertical scrolling for multiline controls.
    pub vertical_scroll: bool,
    /// Request horizontal scrolling for multiline controls.
    pub horizontal_scroll: bool,
    /// Use password-style masking where supported.
    pub password: bool,
}

impl EditFieldOptions {
    /// Returns options with no additional styles.
    pub const fn new() -> Self {
        Self {
            multiline: false,
            read_only: false,
            want_return: false,
            word_wrap: true,
            vertical_scroll: false,
            horizontal_scroll: false,
            password: false,
        }
    }

    /// Converts semantic options into platform style bits.
    pub fn style_bits(self) -> i32 {
        let mut flags = 0;
        #[cfg(target_family = "windows")]
        {
            if self.multiline {
                flags |= 0x0004; // ES_MULTILINE
            }
            if self.read_only {
                flags |= 0x0800; // ES_READONLY
            }
            if self.want_return {
                flags |= 0x1000; // ES_WANTRETURN
            }
            if self.password {
                flags |= 0x0020; // ES_PASSWORD
            }
            if self.multiline && !self.word_wrap {
                flags |= 0x0080; // ES_AUTOHSCROLL
            }
            if self.multiline && self.vertical_scroll {
                flags |= 0x0040; // ES_AUTOVSCROLL
            }
            if self.multiline && self.horizontal_scroll {
                flags |= 0x0080; // ES_AUTOHSCROLL
            }
        }
        #[cfg(target_family = "unix")]
        {
            if self.multiline {
                flags |= 0x0004; // ES_MULTILINE
            }
            if self.read_only {
                flags |= 0x0800; // ES_READONLY
            }
            if self.want_return {
                flags |= 0x1000; // ES_WANTRETURN
            }
            if self.password {
                flags |= 0x0020; // ES_PASSWORD
            }
            if self.multiline && !self.word_wrap {
                flags |= 0x0080; // ES_AUTOHSCROLL
            }
            if self.multiline && self.vertical_scroll {
                flags |= raw::WS_VSCROLL as i32;
            }
            if self.multiline && self.horizontal_scroll {
                flags |= raw::WS_HSCROLL as i32;
            }
        }
        flags
    }

    /// Converts directly from raw platform flags. The resulting semantic
    /// fields cannot represent unknown bits; prefer `style_bits` for a
    /// deliberately typed configuration and keep arbitrary styles separately.
    pub const fn from_raw_flags(flags: i32) -> Self {
        #[cfg(target_family = "windows")]
        {
            Self {
                multiline: flags & 0x0004 != 0,
                read_only: flags & 0x0800 != 0,
                want_return: flags & 0x1000 != 0,
                word_wrap: flags & 0x0080 == 0,
                vertical_scroll: flags & 0x0040 != 0,
                horizontal_scroll: flags & 0x0080 != 0,
                password: flags & 0x0020 != 0,
            }
        }
        #[cfg(target_family = "unix")]
        {
            Self {
                multiline: flags & 0x0004 != 0,
                read_only: flags & 0x0800 != 0,
                want_return: flags & 0x1000 != 0,
                word_wrap: flags & 0x0080 == 0,
                vertical_scroll: flags & raw::WS_VSCROLL as i32 != 0,
                horizontal_scroll: flags & raw::WS_HSCROLL as i32 != 0,
                password: flags & 0x0020 != 0,
            }
        }
    }
}

impl Default for EditFieldOptions {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! define_style_options {
    ($name:ident, $doc:literal, { $($flag:ident = $value:expr => $flag_doc:literal),* $(,)? }) => {
        bitflags::bitflags! {
            #[derive(serde_derive::Serialize, serde_derive::Deserialize)]
            #[doc = $doc]
            pub struct $name: i32 {
                $(
                    #[doc = $flag_doc]
                    const $flag = $value;
                )*
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::empty()
            }
        }

        impl $name {
            /// Constructs options from backend-specific style bits.
            ///
            /// Prefer the named flags where possible. This escape hatch
            /// preserves unknown or platform-specific bits unchanged.
            pub const fn from_raw_styles(styles: i32) -> Self {
                Self { bits: styles }
            }

            /// Returns the underlying native style bits.
            pub const fn raw_styles(self) -> i32 {
                self.bits
            }

            /// Constructs a raw-style escape-hatch value.
            pub const fn raw(styles: i32) -> Self {
                Self { bits: styles }
            }
        }
    };
}

define_style_options!(
    RadioButtonOptions,
    "Semantic optional styles for a radio button. The factory always adds the native child/auto-radio base style.",
    {
        LEFT_TEXT = 0x0020 => "Place the label to the left of the indicator.",
        PUSH_LIKE = 0x1000 => "Render the radio button with push-button appearance."
    }
);
define_style_options!(
    TrackbarOptions,
    "Semantic style flags for a trackbar.",
    {
        AUTO_TICKS = 0x0001 => "Display automatically generated tick marks.",
        VERTICAL = 0x0002 => "Orient the trackbar vertically.",
        BOTH_SIDES_TICKS = 0x0008 => "Display tick marks on both sides.",
        NO_TICKS = 0x0010 => "Hide tick marks.",
        TOP_LEFT_TICKS = 0x0004 => "Display tick marks on the top or left side."
    }
);
define_style_options!(
    ProgressBarOptions,
    "Semantic style flags for a progress bar.",
    {
        SMOOTH = 0x0001 => "Use a smooth fill instead of segmented blocks.",
        VERTICAL = 0x0004 => "Orient the progress bar vertically."
    }
);
define_style_options!(
    TabControlOptions,
    "Semantic style flags for a tab control.",
    {
        BUTTONS = 0x0100 => "Use tab buttons rather than tab-shaped headers.",
        MULTISELECT = 0x0004 => "Allow multiple tab headers to be highlighted.",
        FIXED_WIDTH = 0x0400 => "Give all tab headers a fixed width.",
        BOTTOM = 0x0002 => "Place tab headers below the page area.",
        VERTICAL = 0x0080 => "Place tab headers vertically."
    }
);
define_style_options!(
    ListViewOptions,
    "Semantic style flags for a list view.",
    {
        REPORT = 0x0001 => "Use report/details view.",
        SINGLE_SELECT = 0x0004 => "Allow at most one selected item.",
        SHOW_SELECTION_ALWAYS = 0x0008 => "Keep selection visible when the control loses focus."
    }
);
define_style_options!(
    TreeViewOptions,
    "Semantic style flags for a tree view.",
    {
        HAS_BUTTONS = 0x0001 => "Display expand/collapse buttons.",
        HAS_LINES = 0x0002 => "Display lines connecting tree items.",
        LINES_AT_ROOT = 0x0004 => "Display lines connecting root items.",
        EDIT_LABELS = 0x0008 => "Allow in-place label editing.",
        SHOW_SELECTION_ALWAYS = 0x0020 => "Keep selection visible when the control loses focus."
    }
);

/// Presentation mode for a combo box.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum ComboBoxMode {
    /// Use a non-editable drop-down list (the native default).
    #[default]
    DropDownList,
    /// Use an editable text field with a drop-down list.
    DropDown,
    /// Keep the editable list visible below the text field.
    Simple,
}

/// Semantic options for a combo box.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct ComboBoxOptions {
    /// Presentation mode; defaults to a non-editable drop-down list.
    pub mode: ComboBoxMode,
    /// Keep items sorted as they are inserted.
    pub sorted: bool,
    /// Additional backend-specific style bits.
    pub raw_styles: i32,
}

impl ComboBoxOptions {
    /// Returns the native style bits for the selected presentation and extras.
    pub const fn style_bits(self) -> i32 {
        let mode = match self.mode {
            ComboBoxMode::DropDownList => 0x0003,
            ComboBoxMode::DropDown => 0x0002,
            ComboBoxMode::Simple => 0x0001,
        };
        mode | if self.sorted { 0x0100 } else { 0 } | self.raw_styles
    }

    /// Returns options with additional backend-specific native style bits.
    pub const fn with_raw_styles(mut self, styles: i32) -> Self {
        self.raw_styles = styles;
        self
    }

    /// Constructs options directly from backend-specific style bits.
    pub const fn from_raw_styles(styles: i32) -> Self {
        Self {
            mode: ComboBoxMode::DropDownList,
            sorted: false,
            raw_styles: styles,
        }
    }
}
define_style_options!(
    ListBoxOptions,
    "Semantic style flags for a list box.",
    {
        SORTED = 0x0002 => "Sort items alphabetically as they are added.",
        MULTIPLE_SELECTION = 0x0008 => "Allow multiple items to be selected.",
        EXTENDED_SELECTION = 0x0800 => "Allow range selection with keyboard modifiers.",
        NOTIFY = 0x0001 => "Send selection notifications to the parent."
    }
);

#[cfg(test)]
mod edit_option_tests {
    use super::{
        ComboBoxMode, ComboBoxOptions, EditFieldOptions, ListBoxOptions,
        TrackbarOptions,
    };

    #[test]
    fn multiline_edit_options_select_a_multiline_backend_path() {
        let options = EditFieldOptions {
            multiline: true,
            vertical_scroll: true,
            ..EditFieldOptions::new()
        };
        let flags = options.style_bits();
        assert_ne!(flags & 0x0004, 0);
        #[cfg(target_os = "macos")]
        assert_ne!(flags & rea_rs_low::raw::WS_VSCROLL as i32, 0);
    }

    #[test]
    fn single_line_options_do_not_request_scrollbar_styles() {
        let flags = EditFieldOptions::new().style_bits();
        assert_eq!(flags, 0);
        assert_eq!(EditFieldOptions::default(), EditFieldOptions::new());
    }

    #[test]
    fn combo_box_modes_are_mutually_exclusive_native_styles() {
        assert_eq!(ComboBoxOptions::default().style_bits() & 0x0003, 0x0003);
        assert_eq!(
            ComboBoxOptions {
                mode: ComboBoxMode::DropDown,
                ..ComboBoxOptions::default()
            }
            .style_bits()
                & 0x0003,
            0x0002
        );
        assert_eq!(
            ComboBoxOptions {
                mode: ComboBoxMode::Simple,
                sorted: true,
                ..ComboBoxOptions::default()
            }
            .style_bits(),
            0x0101
        );
    }

    #[test]
    fn style_options_support_named_flags_and_explicit_raw_bits() {
        let options = TrackbarOptions::VERTICAL | TrackbarOptions::AUTO_TICKS;
        assert_eq!(options.bits(), 0x0003);
        assert_eq!(TrackbarOptions::raw(0x4000).raw_styles(), 0x4000);
        assert_eq!(ListBoxOptions::default().bits(), 0);
    }
}

impl ControlRect {
    /// Creates a rectangle in parent-client coordinates.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl From<WidgetSize> for ControlRect {
    fn from(size: WidgetSize) -> Self {
        let preferred = size.preferred();
        Self::new(0, 0, preferred.x.max(1) as i32, preferred.y.max(1) as i32)
    }
}

/// Common HWND-backed control operations.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct ReaperControl {
    handle: ControlHandle,
}

/// A native HWND-backed container.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct NativeContainer {
    handle: ControlHandle,
}

impl NativeContainer {
    /// Wraps a handle as a container without validating its HWND or kind.
    pub fn new(handle: ControlHandle) -> Self {
        Self { handle }
    }

    /// Creates a container view over an existing control.
    pub fn from_control(control: ReaperControl) -> Self {
        Self::new(control.handle)
    }

    /// Returns the shared SWELL identifier used for container event routing.
    pub fn id(&self) -> SwellId {
        self.handle.id
    }

    /// Returns the underlying control ID.
    pub fn control_id(&self) -> SwellId {
        self.handle.id
    }

    /// Returns the non-owning raw HWND.
    pub fn hwnd(&self) -> raw::HWND {
        self.handle.hwnd
    }

    /// Positions the native container in its parent client area.
    pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
        ReaperControl::new(self.handle).set_rect(rect)
    }

    /// Shows or hides the native container.
    pub fn show(&self, visible: bool) -> ReaperResult<()> {
        ReaperControl::new(self.handle).show(visible)
    }

    /// Enables or disables the native container.
    pub fn enable(&self, enabled: bool) -> ReaperResult<()> {
        ReaperControl::new(self.handle).enable(enabled)
    }
}

impl ReaperControl {
    /// Wraps a control handle without validating its HWND or kind.
    pub fn new(handle: ControlHandle) -> Self {
        Self { handle }
    }

    /// Returns the stable logical control ID.
    pub fn id(&self) -> SwellId {
        self.handle.id
    }

    /// Returns the kind recorded when the control was registered.
    pub fn kind(&self) -> ControlKind {
        self.handle.kind
    }

    /// Returns the non-owning raw HWND. It may become stale after a native
    /// window transition.
    pub fn hwnd(&self) -> raw::HWND {
        self.handle.hwnd
    }

    /// Shows or hides the control.
    pub fn show(&self, visible: bool) -> ReaperResult<()> {
        let swell = ReaperWindow::swell()?;
        unsafe {
            swell.ShowWindow(
                self.hwnd(),
                if visible { raw::SW_SHOW } else { raw::SW_HIDE },
            );
        }
        Ok(())
    }

    /// Enables or disables input to the control.
    pub fn enable(&self, enabled: bool) -> ReaperResult<()> {
        let swell = ReaperWindow::swell()?;
        unsafe {
            swell.EnableWindow(self.hwnd(), if enabled { 1 } else { 0 });
        }
        Ok(())
    }

    /// Requests keyboard focus for the control.
    pub fn focus(&self) -> ReaperResult<()> {
        unsafe {
            ReaperWindow::swell()?.SetFocus(self.hwnd());
        }
        Ok(())
    }

    /// Sets the control's native text. Interior NUL bytes are rejected.
    pub fn set_text(&self, text: &str) -> ReaperResult<()> {
        let text = CString::new(text)?;
        let result = unsafe {
            ReaperWindow::swell()?.SetWindowText(self.hwnd(), text.as_ptr())
        };
        if result == 0 {
            Err(ReaRsError::UnsuccessfulOperation("SetWindowText"))
        } else {
            Ok(())
        }
    }

    /// Reads the control text into a fixed-size buffer; long text may be
    /// truncated and native empty/error results are returned as an empty
    /// string.
    pub fn text(&self) -> ReaperResult<String> {
        let mut buffer = vec![0i8; 4096];
        let length = unsafe {
            ReaperWindow::swell()?.GetWindowText(
                self.hwnd(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        if length <= 0 {
            return Ok(String::new());
        }
        Ok(unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    /// Sets the control rectangle in parent-client pixels. Width and height
    /// must be positive.
    pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
        if rect.width < 1 || rect.height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid control size",
            ));
        }
        unsafe {
            ReaperWindow::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                raw::SWP_NOZORDER as i32,
            );
        }
        Ok(())
    }

    /// Sends an arbitrary native message.
    ///
    /// This is a low-level escape hatch: message-specific integer values,
    /// pointer validity, mutability, and pointer lifetimes are the caller's
    /// responsibility. Prefer typed widget methods for routine operations.
    pub fn send_message(
        &self,
        msg: raw::UINT,
        wparam: raw::WPARAM,
        lparam: raw::LPARAM,
    ) -> ReaperResult<raw::LRESULT> {
        Ok(unsafe {
            ReaperWindow::swell()?.SendMessage(
                self.hwnd(),
                msg,
                wparam,
                lparam,
            )
        })
    }
}

macro_rules! typed_control {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(ReaperControl);
        impl $name {
            /// Wraps a control handle without validating its native kind.
            pub fn new(handle: ControlHandle) -> Self {
                Self(ReaperControl::new(handle))
            }
            /// Returns the common low-level control wrapper.
            pub fn control(&self) -> ReaperControl {
                self.0
            }
            /// Returns the logical control ID.
            pub fn id(&self) -> SwellId {
                self.0.id()
            }
            /// Returns the non-owning raw HWND.
            pub fn hwnd(&self) -> raw::HWND {
                self.0.hwnd()
            }
            /// Shows or hides the native control.
            pub fn show(&self, value: bool) -> ReaperResult<()> {
                self.0.show(value)
            }
            /// Enables or disables input to the native control.
            pub fn enable(&self, value: bool) -> ReaperResult<()> {
                self.0.enable(value)
            }
            /// Requests keyboard focus for the native control.
            pub fn focus(&self) -> ReaperResult<()> {
                self.0.focus()
            }
            /// Sets the native control text. Interior NUL bytes are rejected.
            pub fn set_text(&self, value: &str) -> ReaperResult<()> {
                self.0.set_text(value)
            }
            /// Reads text into a fixed-size buffer; long text may be
            /// truncated.
            pub fn text(&self) -> ReaperResult<String> {
                self.0.text()
            }
            /// Sets the control rectangle in parent-client pixels.
            pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
                self.0.set_rect(rect)
            }
            /// Sends a raw native message; the caller owns its parameter and
            /// pointer-safety contract.
            pub fn send_message(
                &self,
                msg: raw::UINT,
                wparam: raw::WPARAM,
                lparam: raw::LPARAM,
            ) -> ReaperResult<raw::LRESULT> {
                self.0.send_message(msg, wparam, lparam)
            }
        }
    };
}

typed_control!(Button, Button);
typed_control!(CheckBox, CheckBox);
typed_control!(RadioButton, RadioButton);
typed_control!(EditField, EditField);
typed_control!(ComboBox, ComboBox);
typed_control!(ListBox, ListBox);
typed_control!(StaticLabel, Static);
typed_control!(GroupBox, Static);
typed_control!(Trackbar, Trackbar);
typed_control!(ProgressBar, ProgressBar);
typed_control!(TabControl, Tab);
typed_control!(ListView, ListView);
typed_control!(TreeView, TreeView);

macro_rules! virtual_typed_control {
    ($name:ident) => {
        /// Experimental virtual control hosted inside a [`Canvas`].
        ///
        /// This control is not a native HWND, is not fully debugged, and may
        /// change in future releases.
        #[derive(Clone, Copy)]
        pub struct $name(rea_rs_low::VirtualControl);
        impl $name {
            /// Returns this virtual control's logical ID.
            pub fn id(&self) -> SwellId {
                SwellId(self.0.id() as u32)
            }
            /// Sets the control's rectangle in its Canvas coordinates.
            pub fn set_rect(&self, rect: ControlRect) {
                self.0.set_rect(rect.x, rect.y, rect.width, rect.height);
            }
            /// Shows or hides the virtual control.
            pub fn set_visible(&self, visible: bool) {
                self.0.set_visible(visible);
            }
            /// Enables or disables the virtual control.
            pub fn set_enabled(&self, enabled: bool) {
                self.0.set_enabled(enabled);
            }
            /// Sets the displayed text; embedded NUL bytes return an error.
            pub fn set_text(
                &self,
                text: &str,
            ) -> Result<(), std::ffi::NulError> {
                self.0.set_text(text)
            }
        }
    };
}

virtual_typed_control!(VirtualIconButton);
virtual_typed_control!(VirtualStaticText);
virtual_typed_control!(VirtualComboBox);
virtual_typed_control!(VirtualSlider);
virtual_typed_control!(VirtualListBox);

impl VirtualIconButton {
    /// Sets the checked state.
    pub fn set_checked(&self, checked: bool) {
        self.0.set_checked(checked);
    }
}
impl VirtualComboBox {
    /// Appends an item and returns its zero-based native-style index.
    pub fn add_item(&self, text: &str) -> Result<i32, std::ffi::NulError> {
        self.0.add_item(text)
    }
    /// Returns the selected item index, or the native no-selection sentinel.
    pub fn selection(&self) -> i32 {
        self.0.selection()
    }
    /// Selects an item by index; native behavior applies to invalid indices.
    pub fn set_selection(&self, index: i32) {
        self.0.set_selection(index);
    }
}
impl VirtualSlider {
    /// Sets the minimum, maximum, and center values.
    pub fn set_range(&self, min: i32, max: i32, center: i32) {
        self.0.set_range(min, max, center);
    }
    /// Returns the current slider value.
    pub fn value(&self) -> i32 {
        self.0.value()
    }
    /// Sets the current slider value.
    pub fn set_value(&self, value: i32) {
        self.0.set_value(value);
    }
}
impl VirtualListBox {
    /// Appends an item and returns its zero-based native-style index.
    pub fn add_item(&self, text: &str) -> Result<i32, std::ffi::NulError> {
        self.0.add_item(text)
    }
}

impl ListView {
    pub fn from_control(control: ReaperControl) -> ReaperResult<Self> {
        if control.kind() != ControlKind::ListView {
            return Err(ReaRsError::InvalidObject(
                "control is not a ListView",
            ));
        }
        Ok(Self(control))
    }
}

impl GroupBox {
    pub fn container(&self) -> NativeContainer {
        NativeContainer::from_control(self.control())
    }
}

impl Button {
    /// Sends the native button-click message.
    pub fn click(&self) -> ReaperResult<()> {
        self.send_message(raw::BM_CLICK, 0, 0).map(|_| ())
    }
}

impl CheckBox {
    /// Returns whether the checkbox is in the checked state.
    ///
    /// An indeterminate native checkbox is reported as `false`; use
    /// [`ReaperControl::send_message`] when the third native state matters.
    pub fn checked(&self) -> ReaperResult<bool> {
        Ok(self.send_message(raw::BM_GETCHECK, 0, 0)?
            == raw::BST_CHECKED as isize)
    }
    /// Sets the checkbox to checked or unchecked.
    pub fn set_checked(&self, checked: bool) -> ReaperResult<()> {
        self.send_message(
            raw::BM_SETCHECK,
            if checked {
                raw::BST_CHECKED as usize
            } else {
                raw::BST_UNCHECKED as usize
            },
            0,
        )
        .map(|_| ())
    }
}

impl RadioButton {
    /// Returns whether this radio button is checked.
    pub fn checked(&self) -> ReaperResult<bool> {
        Ok(self.send_message(raw::BM_GETCHECK, 0, 0)?
            == raw::BST_CHECKED as isize)
    }

    /// Checks or unchecks this radio button.
    ///
    /// Native radio-group behavior (including unchecking siblings) depends on
    /// the control styles and native parentage.
    pub fn set_checked(&self, checked: bool) -> ReaperResult<()> {
        self.send_message(
            raw::BM_SETCHECK,
            if checked {
                raw::BST_CHECKED as usize
            } else {
                raw::BST_UNCHECKED as usize
            },
            0,
        )
        .map(|_| ())
    }
}

impl EditField {
    /// Selects all text in the edit control.
    pub fn select_all(&self) -> ReaperResult<()> {
        self.send_message(raw::EM_SETSEL, 0, -1).map(|_| ())
    }
}

impl ComboBox {
    /// Returns the native selection index, or `-1` when no item is selected.
    pub fn selected_index(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::CB_GETCURSEL, 0, 0)? as i32)
    }

    /// Returns the selected item index, or `None` when no item is selected.
    pub fn selection(&self) -> ReaperResult<Option<usize>> {
        let index = self.selected_index()?;
        Ok(usize::try_from(index).ok())
    }

    /// Selects an item by zero-based index. `-1` clears the selection.
    /// Selects the item at `index`. The index is zero-based; native behavior
    /// determines how out-of-range values are handled.
    pub fn select(&self, index: i32) -> ReaperResult<()> {
        self.send_message(raw::CB_SETCURSEL, index as usize, 0)
            .map(|_| ())
    }
    /// Appends text and returns the native item index or error sentinel.
    /// Interior NUL bytes are rejected.
    pub fn add_item(&self, text: &str) -> ReaperResult<i32> {
        let text = CString::new(text)?;
        Ok(
            self.send_message(raw::CB_ADDSTRING, 0, text.as_ptr() as isize)?
                as i32,
        )
    }
}

impl ListBox {
    /// Returns the native selection index, or `-1` when no item is selected.
    pub fn selected_index(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::LB_GETCURSEL, 0, 0)? as i32)
    }

    /// Returns the selected item index, or `None` when no item is selected.
    pub fn selection(&self) -> ReaperResult<Option<usize>> {
        let index = self.selected_index()?;
        Ok(usize::try_from(index).ok())
    }

    /// Selects an item by zero-based index. `-1` clears the selection.
    /// Selects the item at `index`; `-1` clears the selection.
    pub fn select(&self, index: i32) -> ReaperResult<()> {
        self.send_message(raw::LB_SETCURSEL, index as usize, 0)
            .map(|_| ())
    }
    /// Appends text and returns the native item index or error sentinel.
    /// Interior NUL bytes are rejected.
    pub fn add_item(&self, text: &str) -> ReaperResult<i32> {
        let text = CString::new(text)?;
        Ok(
            self.send_message(raw::LB_ADDSTRING, 0, text.as_ptr() as isize)?
                as i32,
        )
    }
}

impl Trackbar {
    /// Returns the current position.
    pub fn position(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::TBM_GETPOS, 0, 0)? as i32)
    }
    /// Sets the current position; native range semantics determine clamping.
    pub fn set_position(&self, value: i32) -> ReaperResult<()> {
        self.send_message(raw::TBM_SETPOS, 1, value as isize)
            .map(|_| ())
    }
    /// Sets the range using SWELL/Win32's packed 16-bit low/high values.
    /// Values outside the low 16 bits are truncated by this native encoding.
    pub fn set_range(&self, min: i32, max: i32) -> ReaperResult<()> {
        self.send_message(
            raw::TBM_SETRANGE,
            1,
            ((max as u32) << 16 | (min as u32 & 0xffff)) as isize,
        )
        .map(|_| ())
    }
}

impl ProgressBar {
    /// Sets the current progress position.
    pub fn set_position(&self, value: i32) -> ReaperResult<()> {
        self.send_message(raw::PBM_SETPOS, value as usize, 0)
            .map(|_| ())
    }
    /// Sets the progress range. This does not clamp an already-set position.
    pub fn set_range(&self, min: u16, max: u16) -> ReaperResult<()> {
        let packed = ((max as u32) << 16) | min as u32;
        self.send_message(raw::PBM_SETRANGE, 0, packed as isize)
            .map(|_| ())
    }
}

#[derive(Default)]
pub(super) struct ControlRegistry {
    by_id: HashMap<SwellId, ControlHandle>,
    by_hwnd: HashMap<usize, SwellId>,
}

impl ControlRegistry {
    pub(super) fn register(&mut self, handle: ControlHandle) {
        self.by_hwnd.insert(handle.hwnd as usize, handle.id);
        self.by_id.insert(handle.id, handle);
    }

    pub(super) fn unregister(&mut self, id: SwellId) -> Option<ControlHandle> {
        let handle = self.by_id.remove(&id)?;
        self.by_hwnd.remove(&(handle.hwnd as usize));
        Some(handle)
    }

    pub(super) fn rebind(
        &mut self,
        id: SwellId,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        let handle = self
            .by_id
            .get_mut(&id)
            .ok_or(ReaRsError::InvalidObject("control is not registered"))?;
        self.by_hwnd.remove(&(handle.hwnd as usize));
        handle.hwnd = hwnd;
        self.by_hwnd.insert(hwnd as usize, id);
        Ok(())
    }

    pub(super) fn get(&self, id: SwellId) -> Option<ControlHandle> {
        self.by_id.get(&id).copied()
    }

    pub(super) fn get_by_hwnd(
        &self,
        hwnd: raw::HWND,
    ) -> Option<ControlHandle> {
        let id = self.by_hwnd.get(&(hwnd as usize)).copied()?;
        self.get(id)
    }

    pub(super) fn ids(&self) -> impl Iterator<Item = SwellId> + '_ {
        self.by_id.keys().copied()
    }

    pub(super) fn clear(&mut self) {
        let ids: Vec<_> = self.ids().collect();
        for id in ids {
            let _ = self.unregister(id);
        }
    }
}

/// Synchronous widget factory passed to [`ReaperWindow::build_ui`].
///
/// The root context is deliberately just a creation surface over the window;
/// it does not create an implicit panel. Child widgets are created immediately
/// as native controls. Methods such as [`Self::group_box`], [`Self::row`], and
/// [`Self::scroll_view`] create a structural native parent and return a
/// context that creates children under it. Use the pure
/// [`super::layout::PanelLayout`] API for panel allocation.
pub struct CreationContext<'a> {
    window: &'a ReaperWindow,
    parent: raw::HWND,
    container: Option<SwellId>,
    panel: Option<Panel>,
}

impl<'a> CreationContext<'a> {
    pub(super) fn new(window: &'a ReaperWindow) -> Self {
        Self {
            window,
            parent: window.hwnd(),
            container: None,
            panel: None,
        }
    }

    fn size(&self, size: WidgetSize) -> ControlRect {
        size.into()
    }

    fn register_layout_entry(&self, id: SwellId, size: WidgetSize) {
        if let Some(panel) = self.panel {
            self.window.register_panel_layout_entry(panel, id, size);
        } else {
            self.window.register_layout_entry(self.container, id, size);
        }
    }

    /// Sets the layout insets for this context's current flow container.
    pub fn with_insets(self, insets: super::layout::Insets) -> Self {
        let mut layout = self.window.layout.borrow_mut();
        let node = match self.container {
            Some(container) => layout.groups.get_mut(&container),
            None => Some(&mut layout.root),
        };
        if let Some(node) = node {
            node.insets = insets;
        }
        drop(layout);
        self
    }

    /// Starts management of a panel layout for this window's client area.
    ///
    /// The resulting [`PanelContext`] allocates top, bottom, left, right, and
    /// central regions when the window layout is applied. Add controls to a
    /// panel using [`PanelContext::panel`] and then ordinary `CreationContext`
    /// methods. Configured panel items provide ordered size overrides for
    /// matching created children; additional children use their factory
    /// `WidgetSize`. Panel bounds are recomputed on resize. Root-created
    /// controls remain in the normal root flow.
    pub fn panel_layout(
        &self,
        layout: PanelLayout,
    ) -> ReaperResult<PanelContext<'a>> {
        if self.container.is_some() || self.panel.is_some() {
            return Err(ReaRsError::UnsuccessfulOperation(
                "panel layouts can only be managed from the root creation context",
            ));
        }
        self.window.install_panel_layout(layout)?;
        Ok(PanelContext {
            window: self.window,
        })
    }

    /// Creates a sibling creation context assigned to a panel in this
    /// window's managed layout.
    pub fn panel(&self, panel: Panel) -> ReaperResult<Self> {
        self.in_panel(panel)
    }

    /// Creates a nested context that registers widgets with `panel` in the
    /// currently managed panel layout.
    pub fn in_panel(&self, panel: Panel) -> ReaperResult<Self> {
        if self.window.layout.borrow().panel_layout.is_none() {
            return Err(ReaRsError::UnsuccessfulOperation(
                "this window has no managed panel layout",
            ));
        }
        Ok(Self {
            window: self.window,
            parent: self.parent,
            container: self.container,
            panel: Some(panel),
        })
    }

    /// Creates a push button and registers it for this context's layout.
    pub fn button(
        &self,
        id: SwellId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<Button> {
        let control = self.window.create_button_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a radio button with semantic style options.
    pub fn radio_button(
        &self,
        id: SwellId,
        label: &str,
        size: WidgetSize,
        options: RadioButtonOptions,
    ) -> anyhow::Result<RadioButton> {
        let control = self.window.create_radio_button_in(
            self.parent,
            id,
            label,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a trackbar with semantic style options.
    pub fn trackbar(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: TrackbarOptions,
    ) -> anyhow::Result<Trackbar> {
        let control = self.window.create_trackbar_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a progress bar with semantic style options.
    pub fn progress_bar(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: ProgressBarOptions,
    ) -> anyhow::Result<ProgressBar> {
        let control = self.window.create_progress_bar_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a tab control with semantic style options.
    pub fn tab_control(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: TabControlOptions,
    ) -> anyhow::Result<TabControl> {
        let control = self.window.create_tab_control_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a list view with semantic style options.
    pub fn list_view(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: ListViewOptions,
    ) -> anyhow::Result<ListView> {
        let control = self.window.create_list_view_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a tree view with semantic style options.
    pub fn tree_view(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: TreeViewOptions,
    ) -> anyhow::Result<TreeView> {
        let control = self.window.create_tree_view_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates an edit field using portable intent options.
    pub fn edit_field(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: EditFieldOptions,
    ) -> anyhow::Result<EditField> {
        self.create_edit_field(id, size, options.style_bits())
    }

    fn create_edit_field(
        &self,
        id: SwellId,
        size: WidgetSize,
        flags: i32,
    ) -> anyhow::Result<EditField> {
        let control = self.window.create_edit_field_in(
            self.parent,
            id,
            self.size(size),
            flags,
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a static text label and registers it for layout.
    pub fn label(
        &self,
        id: SwellId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<StaticLabel> {
        let control = self.window.create_label_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a checkbox and registers it for layout.
    pub fn checkbox(
        &self,
        id: SwellId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<CheckBox> {
        let control = self.window.create_checkbox_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a combo box with semantic style options.
    pub fn combo_box(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: ComboBoxOptions,
    ) -> anyhow::Result<ComboBox> {
        let control = self.window.create_combo_box_in(
            self.parent,
            id,
            self.size(size),
            options.style_bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a list box with semantic style options.
    pub fn list_box(
        &self,
        id: SwellId,
        size: WidgetSize,
        options: ListBoxOptions,
    ) -> anyhow::Result<ListBox> {
        let control = self.window.create_list_box_in(
            self.parent,
            id,
            self.size(size),
            options.bits(),
        )?;
        self.register_layout_entry(id, size);
        Ok(control)
    }

    /// Creates a native group box and returns a context for its child
    /// controls.
    pub fn group_box(
        &self,
        id: SwellId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<CreationContext<'a>> {
        let group = self.window.create_group_box_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.register_layout_entry(id, size);
        self.window.layout.borrow_mut().groups.insert(
            id,
            LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::Wrap,
                insets: super::layout::Insets {
                    left: 15,
                    top: 28,
                    right: 15,
                    bottom: 10,
                },
            },
        );
        Ok(CreationContext {
            window: self.window,
            parent: group.hwnd(),
            container: Some(id),
            // The group box itself was registered in the selected panel.
            // Its descendants belong to this group's local flow layout.
            panel: None,
        })
    }

    /// Creates a nested single-line horizontal row with 8px spacing.
    ///
    /// The row is a structural child HWND, so overflowing controls are
    /// clipped to the row instead of wrapping into another lane.
    pub fn row(
        &self,
        id: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<CreationContext<'a>> {
        let rect = self.size(size);
        let row = self.window.create_structural_child(self.parent, rect)?;
        // Native controls notify their immediate parent through WM_COMMAND.
        // Route those notifications through the row to the owning window just
        // like GroupBox notifications, while retaining the row's clipping
        // and single-line layout behavior.
        self.window.install_container_event_proc(row)?;
        self.register_layout_entry(id, size);
        self.window.layout.borrow_mut().structural.insert(id, row);
        self.window.layout.borrow_mut().groups.insert(
            id,
            LayoutNode {
                entries: Vec::new(),
                axis: Axis::X,
                spacing: 8,
                policy: OverflowPolicy::Clip,
                insets: super::layout::Insets::default(),
            },
        );
        Ok(CreationContext {
            window: self.window,
            parent: row,
            container: Some(id),
            // The row itself was registered in the selected panel; children
            // are positioned in row-client coordinates.
            panel: None,
        })
    }

    /// Creates a clipped scroll viewport and returns a child creation context.
    ///
    /// `Auto` selects CoolSB when available, otherwise native scrollbars on
    /// Windows. It returns an error when no automatic backend is available;
    /// explicitly requested unsupported renderers also return an error.
    pub fn scroll_view(
        &self,
        id: SwellId,
        size: WidgetSize,
        renderer: ScrollbarRenderer,
    ) -> anyhow::Result<CreationContext<'a>> {
        let renderer = match renderer {
            ScrollbarRenderer::Auto
                if Reaper::get().low().supports_cool_scrollbars() =>
            {
                ScrollbarRenderer::CoolSb
            }
            ScrollbarRenderer::Auto if cfg!(target_family = "windows") => {
                ScrollbarRenderer::Native
            }
            ScrollbarRenderer::Auto => anyhow::bail!(
                "no visible scrollbar backend is available on this platform"
            ),
            renderer => renderer,
        };
        match renderer {
            ScrollbarRenderer::CoolSb
                if !Reaper::get().low().supports_cool_scrollbars() =>
            {
                anyhow::bail!("CoolSB scrollbar renderer is unavailable");
            }
            ScrollbarRenderer::Native if !cfg!(target_family = "windows") => {
                anyhow::bail!("native standard scrollbars are unavailable on this SWELL platform");
            }
            _ => (),
        }
        let rect = self.size(size);
        let extra_style = raw::WS_CLIPCHILDREN
            | if renderer == ScrollbarRenderer::Native {
                raw::WS_HSCROLL | raw::WS_VSCROLL
            } else {
                0
            };
        let view = self.window.create_structural_child_with_style(
            self.parent,
            rect,
            extra_style,
        )?;
        // This fixed-size client-area child is the paint/input clip boundary.
        // CoolSB remains on `view`; oversized content is a grandchild and can
        // never paint into the non-client scrollbar gutter.
        let clip = self.window.create_structural_child_with_style(
            view,
            ControlRect::new(0, 0, rect.width, rect.height),
            raw::WS_CLIPCHILDREN | raw::WS_CLIPSIBLINGS,
        )?;
        let content = self.window.create_structural_child_with_style(
            clip,
            ControlRect::new(0, 0, rect.width, rect.height),
            raw::WS_CLIPSIBLINGS,
        )?;
        self.window.install_container_event_proc(content)?;
        match renderer {
            ScrollbarRenderer::CoolSb
                if Reaper::get().low().supports_cool_scrollbars() =>
            unsafe {
                // CoolSB draws in the non-client frame of its host. Attach
                // it to the fixed viewport, not the translated content
                // child, so widgets cannot cover the scrollbar handles.
                Reaper::get().low().InitializeCoolSB(view);
            },
            ScrollbarRenderer::CoolSb => {
                unreachable!("renderer availability checked above")
            }
            ScrollbarRenderer::Native if !cfg!(target_family = "windows") => {
                unreachable!("renderer availability checked above")
            }
            ScrollbarRenderer::Native => (),
            ScrollbarRenderer::Auto => {
                unreachable!("automatic renderer resolved above")
            }
        }
        self.register_layout_entry(id, size);
        self.window.layout.borrow_mut().structural.insert(id, view);
        self.window.layout.borrow_mut().groups.insert(
            id,
            LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::WrapScroll,
                insets: super::layout::Insets::default(),
            },
        );
        self.window.scroll_views.borrow_mut().insert(
            view as usize,
            ScrollViewRuntime {
                id,
                scrollbar_hwnd: view,
                view,
                clip,
                content,
                state: Rc::new(RefCell::new(ScrollState::new())),
                renderer,
            },
        );
        Ok(CreationContext {
            window: self.window,
            parent: content,
            container: Some(id),
            // The viewport is the panel child. Its contents use the
            // ScrollView's local content coordinates and layout group.
            panel: None,
        })
    }

    /// Creates a structural child HWND intended for custom retained drawing.
    /// The child is registered by `id` and receives its own paint/input
    /// dispatch while sharing the parent handler's retained resources. Its
    /// geometry is independent of child layout; use
    /// [`Self::canvas_layout`] to opt into flow-positioning virtual children.
    pub fn canvas(
        &self,
        id: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<Canvas> {
        let hwnd = self
            .window
            .create_structural_child(self.parent, self.size(size))?;
        self.window.install_container_event_proc(hwnd)?;
        let canvas_id = if self.panel.is_some() {
            SwellId::new_control()
        } else {
            id
        };
        self.window
            .layout
            .borrow_mut()
            .structural
            .insert(canvas_id, hwnd);
        self.register_layout_entry(id, size);
        Ok(Canvas {
            id: canvas_id,
            hwnd,
        })
    }

    /// Creates a child-layout context for virtual controls inside `canvas`.
    ///
    /// Canvas layout is opt-in: plain canvases remain drawing/input viewports
    /// with explicit child coordinates. This context registers its children
    /// in a vertical flow group whose bounds follow the Canvas client area.
    pub fn canvas_layout(
        &self,
        canvas: &Canvas,
    ) -> ReaperResult<CreationContext<'a>> {
        if !self
            .window
            .layout
            .borrow()
            .structural
            .get(&canvas.id)
            .is_some_and(|hwnd| *hwnd == canvas.hwnd)
        {
            return Err(ReaRsError::InvalidObject(
                "canvas does not belong to this window",
            ));
        }
        self.window
            .layout
            .borrow_mut()
            .groups
            .entry(canvas.id)
            .or_insert(LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::Wrap,
                insets: super::layout::Insets::default(),
            });
        self.window
            .layout
            .borrow_mut()
            .virtual_parents
            .insert(canvas.id, canvas.id);
        Ok(CreationContext {
            window: self.window,
            parent: canvas.hwnd,
            container: Some(canvas.id),
            // The Canvas itself belongs to the panel; opted-in virtual
            // children are positioned in Canvas-local coordinates.
            panel: None,
        })
    }

    /// Creates an experimental virtual icon button inside a registered Canvas.
    ///
    /// Virtual controls are not native HWNDs, are not fully debugged, and may
    /// change in future releases.
    pub fn virtual_icon_button(
        &self,
        id: SwellId,
        canvas: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<VirtualIconButton> {
        self.virtual_widget(
            id,
            canvas,
            size,
            rea_rs_low::VirtualControlKind::IconButton,
        )
        .map(VirtualIconButton)
    }

    /// Creates an experimental virtual text control inside a registered
    /// Canvas.
    pub fn virtual_static_text(
        &self,
        id: SwellId,
        canvas: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<VirtualStaticText> {
        self.virtual_widget(
            id,
            canvas,
            size,
            rea_rs_low::VirtualControlKind::StaticText,
        )
        .map(VirtualStaticText)
    }

    /// Creates an experimental virtual combo box inside a registered Canvas.
    pub fn virtual_combo_box(
        &self,
        id: SwellId,
        canvas: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<VirtualComboBox> {
        self.virtual_widget(
            id,
            canvas,
            size,
            rea_rs_low::VirtualControlKind::ComboBox,
        )
        .map(VirtualComboBox)
    }

    /// Creates an experimental virtual slider inside a registered Canvas.
    pub fn virtual_slider(
        &self,
        id: SwellId,
        canvas: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<VirtualSlider> {
        self.virtual_widget(
            id,
            canvas,
            size,
            rea_rs_low::VirtualControlKind::Slider,
        )
        .map(VirtualSlider)
    }

    /// Creates an experimental virtual list box inside a registered Canvas.
    pub fn virtual_list_box(
        &self,
        id: SwellId,
        canvas: SwellId,
        size: WidgetSize,
    ) -> anyhow::Result<VirtualListBox> {
        self.virtual_widget(
            id,
            canvas,
            size,
            rea_rs_low::VirtualControlKind::ListBox,
        )
        .map(VirtualListBox)
    }

    fn virtual_widget(
        &self,
        id: SwellId,
        canvas_id: SwellId,
        size: WidgetSize,
        kind: rea_rs_low::VirtualControlKind,
    ) -> anyhow::Result<rea_rs_low::VirtualControl> {
        let parent = self
            .window
            .layout
            .borrow()
            .structural
            .get(&canvas_id)
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!("virtual controls require a registered Canvas")
            })?;
        let mut hosts = self.window.virtual_hosts.borrow_mut();
        let host = hosts.entry(canvas_id).or_insert_with(|| {
            let queue = Rc::clone(&self.window.virtual_command_queue);
            rea_rs_low::VirtualControlHost::new(
                move |command, p1, p2, source_id| {
                    queue.borrow_mut().push((command, p1, p2, source_id));
                },
            )
        });
        host.set_real_parent(parent as *mut std::ffi::c_void);
        let control =
            host.create_control(kind, id.0 as i32).ok_or_else(|| {
                anyhow::anyhow!("could not create virtual control")
            })?;
        let rect = self.size(size);
        control.set_rect(0, 0, rect.width, rect.height);
        control.set_visible(true);
        self.window
            .layout
            .borrow_mut()
            .virtual_controls
            .insert(id, control);
        self.window.register_layout_entry(Some(canvas_id), id, size);
        Ok(control)
    }

    /// Registers or replaces the event callback for a control ID.
    /// The callback returns a routing response that determines whether event
    /// dispatch is handled or forwarded.
    pub fn on_widget_event(
        &self,
        id: SwellId,
        callback: impl FnMut(
                super::events::ControlEvent,
            ) -> anyhow::Result<super::events::EventResponse>
            + 'static,
    ) {
        self.window.on_widget_event(id, callback);
    }

    /// Registers or replaces the callback for scroll movements from a view.
    pub fn on_scroll_view_event(
        &self,
        id: SwellId,
        callback: impl FnMut(
                ScrollViewEvent,
            ) -> anyhow::Result<super::events::EventResponse>
            + 'static,
    ) {
        self.window.on_scroll_view_event(id, callback);
    }

    /// Registers or replaces the callback for events bubbled from a
    /// container's child controls.
    pub fn on_container_event(
        &self,
        id: super::widgets::SwellId,
        callback: impl FnMut(
                super::events::ContainerEvent,
            ) -> anyhow::Result<super::events::EventResponse>
            + 'static,
    ) {
        self.window.on_container_event(id, callback);
    }
}

/// Builder for a window's managed [`PanelLayout`].
///
/// Create panel contexts with [`Self::panel`] and use ordinary widget
/// factories on the returned context. Configured `PanelLayout` items provide
/// ordered size overrides; further created widgets are appended using their
/// factory `WidgetSize`. The window recomputes allocation on resize.
pub struct PanelContext<'a> {
    window: &'a ReaperWindow,
}

impl<'a> PanelContext<'a> {
    /// Returns a widget-creation context targeting `panel`.
    ///
    /// Widgets are registered in creation order. Configured PanelLayout item
    /// sizes override the corresponding widget factory size; later widgets
    /// use the factory's `WidgetSize`.
    pub fn panel(&self, panel: Panel) -> CreationContext<'a> {
        CreationContext {
            window: self.window,
            parent: self.window.hwnd(),
            container: None,
            panel: Some(panel),
        }
    }

    /// Changes the fixed edge-panel extents and reapplies layout immediately.
    pub fn set_sizes(
        &self,
        sizes: super::layout::PanelSizes,
    ) -> ReaperResult<()> {
        self.window
            .update_panel_layout(|layout| layout.sizes = sizes)?;
        self.window.apply_default_layout()
    }

    /// Applies a configuration edit to the managed layout, then reapplies it.
    pub fn update(
        &self,
        update: impl FnOnce(&mut PanelLayout),
    ) -> ReaperResult<()> {
        self.window.update_panel_layout(update)?;
        self.window.apply_default_layout()
    }

    /// Returns the current panel allocation for explicit bounds.
    pub fn allocate(&self, bounds: super::layout::Rect) -> PanelRects {
        self.window
            .layout
            .borrow()
            .panel_layout
            .as_ref()
            .map_or_else(PanelRects::default, |layout| layout.allocate(bounds))
    }
}

/// Structural native child used for custom rendering and independent input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Canvas {
    id: SwellId,
    hwnd: raw::HWND,
}

impl Canvas {
    /// Returns the logical canvas ID.
    pub fn id(&self) -> SwellId {
        self.id
    }
    /// Returns the non-owning raw child HWND.
    pub fn hwnd(&self) -> raw::HWND {
        self.hwnd
    }
    /// Requests keyboard focus for the canvas.
    pub fn focus(&self) -> ReaperResult<()> {
        unsafe { Reaper::get().swell().SetFocus(self.hwnd) };
        Ok(())
    }
    /// Captures mouse input to this canvas until capture is released or lost.
    pub fn capture(&self) -> ReaperResult<()> {
        unsafe { Reaper::get().swell().SetCapture(self.hwnd) };
        Ok(())
    }
    /// Releases the current thread's mouse capture.
    pub fn release_capture(&self) -> ReaperResult<()> {
        Reaper::get().swell().ReleaseCapture();
        Ok(())
    }
    /// Positions the canvas in parent-client pixels. Width and height must be
    /// positive.
    pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
        if rect.width < 1 || rect.height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid control size",
            ));
        }
        unsafe {
            Reaper::get().swell().SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                raw::SWP_NOZORDER as i32,
            );
        }
        Ok(())
    }

    /// Shows or hides the canvas.
    pub fn show(&self, visible: bool) -> ReaperResult<()> {
        unsafe {
            ReaperWindow::swell()?.ShowWindow(
                self.hwnd,
                if visible { raw::SW_SHOW } else { raw::SW_HIDE },
            );
        }
        Ok(())
    }

    /// Enables or disables input to the canvas.
    pub fn enable(&self, enabled: bool) -> ReaperResult<()> {
        unsafe {
            ReaperWindow::swell()?
                .EnableWindow(self.hwnd, if enabled { 1 } else { 0 });
        }
        Ok(())
    }
}

impl ReaperWindow {
    fn create_native_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        kind: ControlKind,
        class: &str,
        text: Option<&str>,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ControlHandle> {
        self.prepare_control_creation(rect)?;
        let text = text.map(CString::new).transpose()?;
        let hwnd = unsafe {
            Self::swell()?.create_native_control(
                parent,
                id.0 as i32,
                class,
                text.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                styles,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("native control"))?;
        self.create_control_handle(id, kind, hwnd)
    }

    pub(super) fn create_radio_button_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        label: &str,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<RadioButton> {
        Ok(RadioButton::new(self.create_native_in(
            parent,
            id,
            ControlKind::RadioButton,
            "Button",
            Some(label),
            rect,
            0x00000009 | styles,
        )?))
    }

    pub(super) fn create_trackbar_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<Trackbar> {
        Ok(Trackbar::new(self.create_native_in(
            parent,
            id,
            ControlKind::Trackbar,
            "msctls_trackbar32",
            None,
            rect,
            styles,
        )?))
    }

    pub(super) fn create_progress_bar_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ProgressBar> {
        Ok(ProgressBar::new(self.create_native_in(
            parent,
            id,
            ControlKind::ProgressBar,
            "msctls_progress32",
            None,
            rect,
            styles,
        )?))
    }

    pub(super) fn create_tab_control_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<TabControl> {
        Ok(TabControl::new(self.create_native_in(
            parent,
            id,
            ControlKind::Tab,
            "SysTabControl32",
            None,
            rect,
            styles,
        )?))
    }

    pub(super) fn create_list_view_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListView> {
        Ok(ListView::new(self.create_native_in(
            parent,
            id,
            ControlKind::ListView,
            "SysListView32",
            None,
            rect,
            styles,
        )?))
    }

    pub(super) fn create_tree_view_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<TreeView> {
        Ok(TreeView::new(self.create_native_in(
            parent,
            id,
            ControlKind::TreeView,
            "SysTreeView32",
            None,
            rect,
            styles,
        )?))
    }

    pub(super) fn create_button_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<Button> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_button(
                parent,
                id.0 as i32,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(Button::new(self.create_control_handle(
            id,
            ControlKind::Button,
            hwnd,
        )?))
    }

    pub(super) fn create_edit_field_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<EditField> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_edit_field(
                parent,
                id.0 as i32,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                flags,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(EditField::new(self.create_control_handle(
            id,
            ControlKind::EditField,
            hwnd,
        )?))
    }

    pub(super) fn create_label_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<StaticLabel> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_label(
                parent,
                id.0 as i32,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(StaticLabel::new(self.create_control_handle(
            id,
            ControlKind::Static,
            hwnd,
        )?))
    }

    pub(super) fn create_checkbox_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<CheckBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_checkbox(
                parent,
                id.0 as i32,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(CheckBox::new(self.create_control_handle(
            id,
            ControlKind::CheckBox,
            hwnd,
        )?))
    }

    pub(super) fn create_group_box_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<GroupBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_group_box(
                parent,
                id.0 as i32,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        self.install_container_event_proc(hwnd)?;
        Ok(GroupBox::new(self.create_control_handle(
            id,
            ControlKind::Static,
            hwnd,
        )?))
    }

    pub(super) fn create_combo_box_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_combo_box(
                parent,
                id.0 as i32,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                flags,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(ComboBox::new(self.create_control_handle(
            id,
            ControlKind::ComboBox,
            hwnd,
        )?))
    }

    pub(super) fn create_list_box_in(
        &self,
        parent: raw::HWND,
        id: SwellId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_list_box(
                parent,
                id.0 as i32,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                styles,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(ListBox::new(self.create_control_handle(
            id,
            ControlKind::ListBox,
            hwnd,
        )?))
    }
}
