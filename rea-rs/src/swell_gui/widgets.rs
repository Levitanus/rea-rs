use super::layout::WidgetSize;
use super::ReaperWindow;
use crate::{
    swell_gui::{
        layout::{Axis, OverflowPolicy, Panel},
        windows::{LayoutNode, ScrollViewRuntime},
    },
    ReaRsError, Reaper, ReaperResult, ScrollState, ScrollViewEvent,
    ScrollbarRenderer,
};
use rea_rs_low::raw;
use std::{cell::RefCell, collections::HashMap, ffi::CString, rc::Rc};

/// Native control ID assigned to a child window.
///
/// IDs must be unique among controls belonging to the same native window.
/// [`ControlId::new`] allocates a process-local value; explicit tuple
/// construction is available when a stable, caller-managed ID is required.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ControlId(pub i32);

impl ControlId {
    /// Allocates a process-local ID from the crate's control-ID sequence.
    ///
    /// This does not coordinate with IDs created outside this crate; avoid
    /// collisions with IDs used by other native controls in the same window.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicI32, Ordering};
        static NEXT_ID: AtomicI32 = AtomicI32::new(10_000);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Stable identity of a native container used for explicit event routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ContainerId(pub ControlId);

/// Control families understood by the high-level event decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlHandle {
    /// Logical/native child control ID.
    pub id: ControlId,
    /// Control family used when decoding native notifications.
    pub kind: ControlKind,
    /// Current raw SWELL/Win32 handle; may be null or stale.
    pub hwnd: raw::HWND,
}

/// Rectangle used by control positioning helpers, in parent-client pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReaperControl {
    handle: ControlHandle,
}

/// A native HWND-backed container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    /// Returns the logical container identity used for event routing.
    pub fn id(&self) -> ContainerId {
        ContainerId(self.handle.id)
    }

    /// Returns the underlying control ID.
    pub fn control_id(&self) -> ControlId {
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
    pub fn id(&self) -> ControlId {
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
            pub fn id(&self) -> ControlId {
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
            pub fn id(&self) -> ControlId {
                ControlId(self.0.id())
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
    by_id: HashMap<ControlId, ControlHandle>,
    by_hwnd: HashMap<usize, ControlId>,
}

impl ControlRegistry {
    pub(super) fn register(&mut self, handle: ControlHandle) {
        self.by_hwnd.insert(handle.hwnd as usize, handle.id);
        self.by_id.insert(handle.id, handle);
    }

    pub(super) fn unregister(
        &mut self,
        id: ControlId,
    ) -> Option<ControlHandle> {
        let handle = self.by_id.remove(&id)?;
        self.by_hwnd.remove(&(handle.hwnd as usize));
        Some(handle)
    }

    pub(super) fn rebind(
        &mut self,
        id: ControlId,
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

    pub(super) fn get(&self, id: ControlId) -> Option<ControlHandle> {
        self.by_id.get(&id).copied()
    }

    pub(super) fn get_by_hwnd(
        &self,
        hwnd: raw::HWND,
    ) -> Option<ControlHandle> {
        let id = self.by_hwnd.get(&(hwnd as usize)).copied()?;
        self.get(id)
    }

    pub(super) fn ids(&self) -> impl Iterator<Item = ControlId> + '_ {
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
/// context that creates children under it. [`Self::panel`] is currently a
/// pass-through and does not allocate a panel or alter layout; use the pure
/// [`super::layout::PanelLayout`] API for panel allocation.
pub struct CreationContext<'a> {
    window: &'a ReaperWindow,
    parent: raw::HWND,
    container: Option<ControlId>,
}

impl<'a> CreationContext<'a> {
    pub(super) fn new(window: &'a ReaperWindow) -> Self {
        Self {
            window,
            parent: window.hwnd(),
            container: None,
        }
    }

    fn size(&self, size: WidgetSize) -> ControlRect {
        size.into()
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

    /// Creates a push button and registers it for this context's layout.
    pub fn button(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<Button> {
        let control = self.window.create_button_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a radio button with the supplied native style bits.
    /// `styles` is passed through to SWELL/Win32 and is backend-dependent.
    pub fn radio_button(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<RadioButton> {
        let control = self.window.create_radio_button_in(
            self.parent,
            id,
            label,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a trackbar with native style bits passed through unchanged.
    pub fn trackbar(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<Trackbar> {
        let control = self.window.create_trackbar_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a progress bar with native style bits passed through unchanged.
    pub fn progress_bar(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<ProgressBar> {
        let control = self.window.create_progress_bar_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a tab control with native style bits passed through unchanged.
    pub fn tab_control(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<TabControl> {
        let control = self.window.create_tab_control_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a list view with native style bits passed through unchanged.
    pub fn list_view(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<ListView> {
        let control = self.window.create_list_view_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a tree view with native style bits passed through unchanged.
    pub fn tree_view(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<TreeView> {
        let control = self.window.create_tree_view_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates an edit field with native style/flag bits passed through.
    /// Their exact multiline and scrolling behavior depends on the backend.
    pub fn edit_field(
        &self,
        id: ControlId,
        size: WidgetSize,
        flags: i32,
    ) -> anyhow::Result<EditField> {
        let control = self.window.create_edit_field_in(
            self.parent,
            id,
            self.size(size),
            flags,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a static text label and registers it for layout.
    pub fn label(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<StaticLabel> {
        let control = self.window.create_label_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a checkbox and registers it for layout.
    pub fn checkbox(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<CheckBox> {
        let control = self.window.create_checkbox_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a combo box with native flag bits passed through unchanged.
    pub fn combo_box(
        &self,
        id: ControlId,
        size: WidgetSize,
        flags: i32,
    ) -> anyhow::Result<ComboBox> {
        let control = self.window.create_combo_box_in(
            self.parent,
            id,
            self.size(size),
            flags,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a list box with native style bits passed through unchanged.
    pub fn list_box(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> anyhow::Result<ListBox> {
        let control = self.window.create_list_box_in(
            self.parent,
            id,
            self.size(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Returns this context unchanged.
    ///
    /// `panel` currently has no effect on native parentage or placement. Use
    /// [`super::layout::PanelLayout`] directly when panel geometry is needed.
    pub fn panel(&self, _panel: Panel) -> CreationContext<'a> {
        CreationContext {
            window: self.window,
            parent: self.parent,
            container: self.container,
        }
    }

    /// Selects the nominal central panel; currently equivalent to
    /// [`Self::panel`].
    pub fn central_panel(&self) -> CreationContext<'a> {
        self.panel(Panel::Central)
    }

    /// Creates a native group box and returns a context for its child
    /// controls.
    pub fn group_box(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> anyhow::Result<CreationContext<'a>> {
        let group = self.window.create_group_box_in(
            self.parent,
            id,
            label,
            self.size(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
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
        })
    }

    /// Creates a nested single-line horizontal row with 8px spacing.
    ///
    /// The row is a structural child HWND, so overflowing controls are
    /// clipped to the row instead of wrapping into another lane.
    pub fn row(
        &self,
        id: ControlId,
        size: WidgetSize,
    ) -> anyhow::Result<CreationContext<'a>> {
        let rect = self.size(size);
        let row = self.window.create_structural_child(self.parent, rect)?;
        self.window.register_layout_entry(self.container, id, size);
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
        })
    }

    /// Creates a clipped scroll viewport and returns a child creation context.
    ///
    /// `Auto` selects CoolSB when available, otherwise native scrollbars on
    /// Windows. It returns an error when no automatic backend is available;
    /// explicitly requested unsupported renderers also return an error.
    pub fn scroll_view(
        &self,
        id: ControlId,
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
        self.window.register_layout_entry(self.container, id, size);
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
        })
    }

    /// Creates a structural child HWND intended for custom retained drawing.
    /// The child is registered by `id` and receives its own paint/input
    /// dispatch while sharing the parent handler's retained resources. It is
    /// currently registered as a layout container and may receive flow-based
    /// child positioning; the opt-in isolated viewport behavior has not yet
    /// been implemented.
    pub fn canvas(
        &self,
        id: ControlId,
        size: WidgetSize,
    ) -> anyhow::Result<Canvas> {
        let hwnd = self
            .window
            .create_structural_child(self.parent, self.size(size))?;
        self.window.install_container_event_proc(hwnd)?;
        {
            let mut layout = self.window.layout.borrow_mut();
            layout.structural.insert(id, hwnd);
            layout.groups.insert(
                id,
                LayoutNode {
                    entries: Vec::new(),
                    axis: Axis::Y,
                    spacing: 8,
                    policy: OverflowPolicy::Wrap,
                    insets: super::layout::Insets::default(),
                },
            );
        }
        self.window.register_layout_entry(self.container, id, size);
        Ok(Canvas { id, hwnd })
    }

    /// Creates an experimental virtual icon button inside a registered Canvas.
    ///
    /// Virtual controls are not native HWNDs, are not fully debugged, and may
    /// change in future releases.
    pub fn virtual_icon_button(
        &self,
        id: ControlId,
        canvas: ControlId,
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
        id: ControlId,
        canvas: ControlId,
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
        id: ControlId,
        canvas: ControlId,
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
        id: ControlId,
        canvas: ControlId,
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
        id: ControlId,
        canvas: ControlId,
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
        id: ControlId,
        canvas_id: ControlId,
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
        let control = host.create_control(kind, id.0).ok_or_else(|| {
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
        id: ControlId,
        callback: impl FnMut(super::events::ControlEvent) -> super::events::EventResponse
            + 'static,
    ) {
        self.window.on_widget_event(id, callback);
    }

    /// Registers or replaces the callback for scroll movements from a view.
    pub fn on_scroll_view_event(
        &self,
        id: ControlId,
        callback: impl FnMut(ScrollViewEvent) -> super::events::EventResponse
            + 'static,
    ) {
        self.window.on_scroll_view_event(id, callback);
    }

    /// Registers or replaces the callback for events bubbled from a
    /// container's child controls.
    pub fn on_container_event(
        &self,
        id: super::widgets::ContainerId,
        callback: impl FnMut(
                super::events::ContainerEvent,
            ) -> super::events::EventResponse
            + 'static,
    ) {
        self.window.on_container_event(id, callback);
    }
}

/// Structural native child used for custom rendering and independent input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Canvas {
    id: ControlId,
    hwnd: raw::HWND,
}

impl Canvas {
    /// Returns the logical canvas ID.
    pub fn id(&self) -> ControlId {
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
}

impl ReaperWindow {
    fn create_native_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
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
                id.0,
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
        id: ControlId,
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
        id: ControlId,
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
        id: ControlId,
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
        id: ControlId,
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
        id: ControlId,
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
        id: ControlId,
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
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<Button> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_button(
                parent,
                id.0,
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
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<EditField> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_edit_field(
                parent,
                id.0,
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
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<StaticLabel> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_label(
                parent,
                id.0,
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
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<CheckBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_checkbox(
                parent,
                id.0,
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
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<GroupBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_group_box(
                parent,
                id.0,
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
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_combo_box(
                parent,
                id.0,
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
        id: ControlId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_list_box(
                parent,
                id.0,
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
