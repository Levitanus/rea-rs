use super::layout::Point;
use super::scroll::ScrollOffset;
use super::widgets::{ControlId, ControlKind};
use super::windows::ReaperWindow;
use crate::keys::{KeyStroke, VKeys};
use rea_rs_low::raw;
use std::collections::HashMap;

/// A decoded `WM_COMMAND` notification code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandNotification {
    Clicked,
    SetFocus,
    KillFocus,
    EditChange,
    ComboSelectionChanged,
    ComboEditChanged,
    ComboDropDown,
    ComboCloseUp,
    ListDoubleClick,
    Other(i32),
}

impl CommandNotification {
    pub(super) fn from_raw(code: i32) -> Self {
        match code {
            0 => Self::Clicked,
            256 => Self::SetFocus,
            512 => Self::KillFocus,
            768 => Self::EditChange,
            1 => Self::ComboSelectionChanged,
            5 => Self::ComboEditChanged,
            7 => Self::ComboDropDown,
            8 => Self::ComboCloseUp,
            2 => Self::ListDoubleClick,
            other => Self::Other(other),
        }
    }
}

/// A decoded `WM_COMMAND` event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowCommand {
    Menu {
        id: i32,
    },
    Control {
        id: i32,
        notification: CommandNotification,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlEvent {
    ButtonClicked {
        control: ControlId,
    },
    CheckBoxChanged {
        control: ControlId,
    },
    RadioButtonChanged {
        control: ControlId,
    },
    EditChanged {
        control: ControlId,
    },
    ComboSelectionChanged {
        control: ControlId,
    },
    ComboEditChanged {
        control: ControlId,
    },
    ListSelectionChanged {
        control: ControlId,
    },
    ListDoubleClick {
        control: ControlId,
    },
    TrackbarChanged {
        control: ControlId,
    },
    TabSelectionChanged {
        control: ControlId,
    },
    ListViewItemChanged {
        control: ControlId,
    },
    ListViewColumnClicked {
        control: ControlId,
    },
    TreeSelectionChanged {
        control: ControlId,
    },
    TreeItemExpanding {
        control: ControlId,
    },
    TreeBeginDrag {
        control: ControlId,
    },
    VirtualButtonClicked {
        control: ControlId,
    },
    VirtualSliderChanged {
        control: ControlId,
    },
    VirtualComboSelectionChanged {
        control: ControlId,
    },
    VirtualListSelectionChanged {
        control: ControlId,
    },
    VirtualListDoubleClick {
        control: ControlId,
    },
    Scroll {
        control: ControlId,
        code: i32,
    },
    Notified {
        control: ControlId,
        code: u32,
    },
    FocusGained {
        control: ControlId,
    },
    FocusLost {
        control: ControlId,
    },
    OtherCommand {
        control: ControlId,
        notification: CommandNotification,
    },
}

/// The semantic source of a ScrollView movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollViewEventSource {
    Wheel,
    Line,
    Page,
    Thumb,
    Programmatic,
}

impl ScrollViewEventSource {
    pub const fn is_user_initiated(self) -> bool {
        !matches!(self, Self::Programmatic)
    }
}

/// A decoded ScrollView movement.
///
/// Unlike [`ControlEvent::Scroll`], this event does not expose a native
/// notification code. The offset is already clamped and expressed in content
/// coordinates, so consumers do not need to understand SWELL or Win32.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollViewEvent {
    pub offset: ScrollOffset,
    pub source: ScrollViewEventSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerEvent {
    Child {
        container: super::widgets::ContainerId,
        event: ControlEvent,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventResponse {
    Handled,
    Ignore,
    ForwardToParent,
    ForwardToWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct MouseButtons: u16 {
        const LEFT = 0x0001;
        const RIGHT = 0x0002;
        const SHIFT = 0x0004;
        const CONTROL = 0x0008;
        const MIDDLE = 0x0010;
        const X1 = 0x0020;
        const X2 = 0x0040;
    }
}

impl MouseButtons {
    pub const fn from_key_state(bits: u16) -> Self {
        Self::from_bits_truncate(bits)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseMessage {
    Move,
    Down(MouseButton),
    Up(MouseButton),
    DoubleClick(MouseButton),
    Wheel { horizontal: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyMessage {
    Down,
    Up,
    SystemDown,
    SystemUp,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0x01;
        const CONTROL = 0x02;
        const ALT = 0x04;
        const META = 0x08;
        const CAPS_LOCK = 0x10;
        const NUM_LOCK = 0x20;
    }
}

/// General window input/lifecycle event, independent of native controls.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    Canvas {
        control: ControlId,
        event: Box<WindowEvent>,
    },
    Mouse {
        message: MouseMessage,
        position: Point,
        buttons: MouseButtons,
    },
    Wheel {
        horizontal: bool,
        delta: i32,
        position: Point,
    },
    Key {
        message: KeyMessage,
        key: NativeKey,
        stroke: KeyStroke,
        modifiers: KeyModifiers,
    },
    Text(char),
    Focus(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeKey {
    Known(VKeys),
    Other(u32),
}

/// Callback for general window/Canvas input events.
pub type WindowEventCallback = Box<dyn FnMut(WindowEvent) -> EventResponse>;

pub type WidgetEventCallback = Box<dyn FnMut(ControlEvent) -> EventResponse>;
pub type ScrollViewEventCallback =
    Box<dyn FnMut(ScrollViewEvent) -> EventResponse>;
pub type ContainerEventCallback =
    Box<dyn FnMut(ContainerEvent) -> EventResponse>;

#[derive(Default)]
pub(super) struct EventRegistry {
    pub(super) widget_callbacks: HashMap<ControlId, WidgetEventCallback>,
    pub(super) scroll_view_callbacks:
        HashMap<ControlId, ScrollViewEventCallback>,
    pub(super) container_callbacks:
        HashMap<super::widgets::ContainerId, ContainerEventCallback>,
    pub(super) direct_container:
        HashMap<ControlId, super::widgets::ContainerId>,
    pub(super) container_parent:
        HashMap<super::widgets::ContainerId, super::widgets::ContainerId>,
}

/// Result of routing a native control event through the explicit callback
/// tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DispatchResult {
    Handled,
    ForwardToWindow,
}

impl EventRegistry {
    pub(super) fn register_widget_callback(
        &mut self,
        id: ControlId,
        callback: WidgetEventCallback,
    ) {
        self.widget_callbacks.insert(id, callback);
    }

    pub(super) fn register_container_callback(
        &mut self,
        id: super::widgets::ContainerId,
        callback: ContainerEventCallback,
    ) {
        self.container_callbacks.insert(id, callback);
    }

    pub(super) fn set_direct_container(
        &mut self,
        child: ControlId,
        container: super::widgets::ContainerId,
    ) {
        self.direct_container.insert(child, container);
    }

    pub(super) fn set_container_parent(
        &mut self,
        container: super::widgets::ContainerId,
        parent: super::widgets::ContainerId,
    ) {
        self.container_parent.insert(container, parent);
    }

    pub(super) fn clear(&mut self) {
        self.widget_callbacks.clear();
        self.scroll_view_callbacks.clear();
        self.container_callbacks.clear();
        self.direct_container.clear();
        self.container_parent.clear();
    }

    pub(super) fn register_scroll_view_callback(
        &mut self,
        id: ControlId,
        callback: ScrollViewEventCallback,
    ) {
        self.scroll_view_callbacks.insert(id, callback);
    }

    pub(super) fn dispatch_scroll_view(
        &mut self,
        id: ControlId,
        event: ScrollViewEvent,
    ) -> DispatchResult {
        let Some(mut callback) = self.scroll_view_callbacks.remove(&id) else {
            return DispatchResult::ForwardToWindow;
        };
        let response = callback(event);
        self.scroll_view_callbacks.insert(id, callback);
        match response {
            EventResponse::Handled => DispatchResult::Handled,
            EventResponse::ForwardToWindow => DispatchResult::ForwardToWindow,
            EventResponse::Ignore | EventResponse::ForwardToParent => {
                DispatchResult::Handled
            }
        }
    }

    /// Routes an event through the widget, its direct container, and any
    /// explicitly configured container parents.  Callbacks are temporarily
    /// removed while they run so a callback may safely replace its own
    /// registration without causing a `RefCell` reborrow panic.
    pub(super) fn dispatch(&mut self, event: ControlEvent) -> DispatchResult {
        let control = event.control();
        if let Some(mut callback) = self.widget_callbacks.remove(&control) {
            let response = callback(event);
            self.widget_callbacks.insert(control, callback);
            match response {
                EventResponse::Handled => return DispatchResult::Handled,
                EventResponse::ForwardToWindow => {
                    return DispatchResult::ForwardToWindow
                }
                EventResponse::Ignore | EventResponse::ForwardToParent => {}
            }
        }

        let mut current = self.direct_container.get(&control).copied();
        let mut visited = std::collections::HashSet::new();
        while let Some(container) = current {
            if !visited.insert(container) {
                break;
            }
            let container_event = ContainerEvent::child(container, event);
            let Some(mut callback) =
                self.container_callbacks.remove(&container)
            else {
                current = self.container_parent.get(&container).copied();
                continue;
            };
            let response = callback(container_event);
            self.container_callbacks.insert(container, callback);
            match response {
                EventResponse::Handled => return DispatchResult::Handled,
                EventResponse::ForwardToWindow => {
                    return DispatchResult::ForwardToWindow
                }
                EventResponse::ForwardToParent => {
                    current = self.container_parent.get(&container).copied();
                }
                EventResponse::Ignore => break,
            }
        }
        DispatchResult::ForwardToWindow
    }
}

impl ControlEvent {
    pub(super) const fn control(self) -> ControlId {
        match self {
            Self::ButtonClicked { control }
            | Self::CheckBoxChanged { control }
            | Self::RadioButtonChanged { control }
            | Self::EditChanged { control }
            | Self::ComboSelectionChanged { control }
            | Self::ComboEditChanged { control }
            | Self::ListSelectionChanged { control }
            | Self::ListDoubleClick { control }
            | Self::TrackbarChanged { control }
            | Self::TabSelectionChanged { control }
            | Self::ListViewItemChanged { control }
            | Self::ListViewColumnClicked { control }
            | Self::TreeSelectionChanged { control }
            | Self::TreeItemExpanding { control }
            | Self::TreeBeginDrag { control }
            | Self::VirtualButtonClicked { control }
            | Self::VirtualSliderChanged { control }
            | Self::VirtualComboSelectionChanged { control }
            | Self::VirtualListSelectionChanged { control }
            | Self::VirtualListDoubleClick { control }
            | Self::Scroll { control, .. }
            | Self::Notified { control, .. }
            | Self::FocusGained { control }
            | Self::FocusLost { control }
            | Self::OtherCommand { control, .. } => control,
        }
    }
}

pub(super) fn decode_notify_event(
    kind: ControlKind,
    id: ControlId,
    code: u32,
) -> ControlEvent {
    match (kind, code) {
        (ControlKind::Tab, raw::TCN_SELCHANGE) => {
            ControlEvent::TabSelectionChanged { control: id }
        }
        (ControlKind::ListView, raw::LVN_ITEMCHANGED) => {
            ControlEvent::ListViewItemChanged { control: id }
        }
        (ControlKind::ListView, raw::LVN_COLUMNCLICK) => {
            ControlEvent::ListViewColumnClicked { control: id }
        }
        (ControlKind::TreeView, raw::TVN_SELCHANGED) => {
            ControlEvent::TreeSelectionChanged { control: id }
        }
        (ControlKind::TreeView, raw::TVN_ITEMEXPANDING) => {
            ControlEvent::TreeItemExpanding { control: id }
        }
        (ControlKind::TreeView, raw::TVN_BEGINDRAG) => {
            ControlEvent::TreeBeginDrag { control: id }
        }
        _ => ControlEvent::Notified { control: id, code },
    }
}

impl ContainerEvent {
    pub const fn child(
        container: super::widgets::ContainerId,
        event: ControlEvent,
    ) -> Self {
        Self::Child { container, event }
    }
}

#[repr(C)]
pub(super) struct NotifyHeader {
    pub hwnd_from: raw::HWND,
    pub id_from: usize,
    pub code: u32,
}

pub(super) fn decode_virtual_event(
    kind: rea_rs_low::VirtualControlKind,
    id: ControlId,
    command: i32,
) -> Option<ControlEvent> {
    match kind {
        rea_rs_low::VirtualControlKind::IconButton
            if command == raw::WM_COMMAND as i32 =>
        {
            Some(ControlEvent::VirtualButtonClicked { control: id })
        }
        rea_rs_low::VirtualControlKind::Slider
            if command == raw::WM_HSCROLL as i32
                || command == raw::WM_VSCROLL as i32 =>
        {
            Some(ControlEvent::VirtualSliderChanged { control: id })
        }
        rea_rs_low::VirtualControlKind::Slider
            if command != raw::WM_COMMAND as i32 =>
        {
            None
        }
        rea_rs_low::VirtualControlKind::ComboBox
            if command == raw::WM_COMMAND as i32 =>
        {
            Some(ControlEvent::VirtualComboSelectionChanged { control: id })
        }
        rea_rs_low::VirtualControlKind::ListBox
            if command == raw::WM_USER as i32 + 101 =>
        {
            Some(ControlEvent::VirtualListSelectionChanged { control: id })
        }
        rea_rs_low::VirtualControlKind::ListBox
            if command == raw::WM_USER as i32 + 102 =>
        {
            Some(ControlEvent::VirtualListDoubleClick { control: id })
        }
        _ => None,
    }
}

pub(super) fn decode_control_event(
    kind: ControlKind,
    id: ControlId,
    notification: CommandNotification,
) -> Option<ControlEvent> {
    match (kind, notification) {
        (ControlKind::Button, CommandNotification::Clicked) => {
            Some(ControlEvent::ButtonClicked { control: id })
        }
        (ControlKind::CheckBox, CommandNotification::Clicked) => {
            Some(ControlEvent::CheckBoxChanged { control: id })
        }
        (ControlKind::RadioButton, CommandNotification::Clicked) => {
            Some(ControlEvent::RadioButtonChanged { control: id })
        }
        (ControlKind::EditField, CommandNotification::EditChange) => {
            Some(ControlEvent::EditChanged { control: id })
        }
        (
            ControlKind::ComboBox,
            CommandNotification::ComboSelectionChanged,
        ) => Some(ControlEvent::ComboSelectionChanged { control: id }),
        (ControlKind::ComboBox, CommandNotification::ComboEditChanged) => {
            Some(ControlEvent::ComboEditChanged { control: id })
        }
        (ControlKind::ListBox, CommandNotification::ComboSelectionChanged) => {
            Some(ControlEvent::ListSelectionChanged { control: id })
        }
        (ControlKind::ListBox, CommandNotification::ListDoubleClick) => {
            Some(ControlEvent::ListDoubleClick { control: id })
        }
        (ControlKind::Trackbar, CommandNotification::Other(_)) => {
            Some(ControlEvent::TrackbarChanged { control: id })
        }
        (_, CommandNotification::SetFocus) => {
            Some(ControlEvent::FocusGained { control: id })
        }
        (_, CommandNotification::KillFocus) => {
            Some(ControlEvent::FocusLost { control: id })
        }
        (_, notification) => Some(ControlEvent::OtherCommand {
            control: id,
            notification,
        }),
    }
}

impl ReaperWindow {
    pub fn on_widget_event(
        &self,
        id: ControlId,
        callback: impl FnMut(ControlEvent) -> EventResponse + 'static,
    ) {
        self.events
            .borrow_mut()
            .register_widget_callback(id, Box::new(callback));
    }

    /// Registers a semantic callback for a ScrollView. The callback receives
    /// a clamped content offset and a platform-neutral movement source.
    pub fn on_scroll_view_event(
        &self,
        id: ControlId,
        callback: impl FnMut(ScrollViewEvent) -> EventResponse + 'static,
    ) {
        self.events
            .borrow_mut()
            .register_scroll_view_callback(id, Box::new(callback));
    }

    /// Delivers a semantic ScrollView event to the registered callback.
    ///
    /// This is primarily used by native viewport message handlers. Keeping
    /// the delivery operation on `ReaperWindow` avoids exposing the internal
    /// event registry while allowing platform adapters to remain separate from
    /// the public event contract.
    pub(crate) fn emit_scroll_view_event(
        &self,
        id: ControlId,
        event: ScrollViewEvent,
    ) -> EventResponse {
        match self.events.borrow_mut().dispatch_scroll_view(id, event) {
            DispatchResult::Handled => EventResponse::Handled,
            DispatchResult::ForwardToWindow => EventResponse::ForwardToWindow,
        }
    }

    pub fn on_container_event(
        &self,
        id: super::widgets::ContainerId,
        callback: impl FnMut(ContainerEvent) -> EventResponse + 'static,
    ) {
        self.events
            .borrow_mut()
            .register_container_callback(id, Box::new(callback));
    }

    pub fn set_control_container(
        &self,
        child: ControlId,
        container: super::widgets::ContainerId,
    ) {
        self.events
            .borrow_mut()
            .set_direct_container(child, container);
    }

    pub fn set_container_parent(
        &self,
        container: super::widgets::ContainerId,
        parent: super::widgets::ContainerId,
    ) {
        self.events
            .borrow_mut()
            .set_container_parent(container, parent);
    }
}
