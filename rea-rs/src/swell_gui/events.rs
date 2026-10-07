//! Semantic events decoded from SWELL/Win32 messages.
//!
//! These values retain the logical control identity and commonly useful
//! payload while hiding native message decoding. They do not own controls or
//! guarantee that a control remains registered after an event is delivered.

use super::layout::Point;
use super::scroll::ScrollOffset;
use super::widgets::{ControlKind, SwellId};
use super::windows::ReaperWindow;
use crate::keys::{KeyStroke, VKeys};
use rea_rs_low::raw;
use std::collections::HashMap;

/// A decoded `WM_COMMAND` notification code.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
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
        id: super::widgets::SwellId,
    },
    Control {
        id: SwellId,
        notification: CommandNotification,
    },
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum ControlEvent {
    ButtonClicked {
        control: SwellId,
    },
    CheckBoxChanged {
        control: SwellId,
    },
    RadioButtonChanged {
        control: SwellId,
    },
    EditChanged {
        control: SwellId,
    },
    ComboSelectionChanged {
        control: SwellId,
    },
    ComboEditChanged {
        control: SwellId,
    },
    ListSelectionChanged {
        control: SwellId,
    },
    ListDoubleClick {
        control: SwellId,
    },
    TrackbarChanged {
        control: SwellId,
    },
    TabSelectionChanged {
        control: SwellId,
    },
    ListViewItemChanged {
        control: SwellId,
    },
    ListViewColumnClicked {
        control: SwellId,
    },
    TreeSelectionChanged {
        control: SwellId,
    },
    TreeItemExpanding {
        control: SwellId,
    },
    TreeBeginDrag {
        control: SwellId,
    },
    /// Experimental virtual-control notification; virtual controls are not
    /// fully debugged and these event shapes may change before stabilization.
    VirtualButtonClicked {
        control: SwellId,
    },
    /// Experimental virtual-control notification; virtual controls are not
    /// fully debugged and these event shapes may change before stabilization.
    VirtualSliderChanged {
        control: SwellId,
    },
    /// Experimental virtual-control notification; virtual controls are not
    /// fully debugged and these event shapes may change before stabilization.
    VirtualComboSelectionChanged {
        control: SwellId,
    },
    /// Experimental virtual-control notification; virtual controls are not
    /// fully debugged and these event shapes may change before stabilization.
    VirtualListSelectionChanged {
        control: SwellId,
    },
    /// Experimental virtual-control notification; virtual controls are not
    /// fully debugged and these event shapes may change before stabilization.
    VirtualListDoubleClick {
        control: SwellId,
    },
    Scroll {
        control: SwellId,
        code: i32,
    },
    Notified {
        control: SwellId,
        code: u32,
    },
    FocusGained {
        control: SwellId,
    },
    FocusLost {
        control: SwellId,
    },
    OtherCommand {
        control: SwellId,
        notification: CommandNotification,
    },
}

/// The semantic source of a ScrollView movement.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
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
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub struct ScrollViewEvent {
    pub offset: ScrollOffset,
    pub source: ScrollViewEventSource,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum ContainerEvent {
    Child {
        container: super::widgets::SwellId,
        event: ControlEvent,
    },
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum EventResponse {
    Handled,
    Ignore,
    ForwardToParent,
    ForwardToWindow,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

bitflags::bitflags! {
    #[derive(
        Default,
        serde_derive::Serialize,
        serde_derive::Deserialize
    )]
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

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum MouseMessage {
    Move,
    Down(MouseButton),
    Up(MouseButton),
    DoubleClick(MouseButton),
    Wheel { horizontal: bool },
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    serde_derive::Serialize,
    serde_derive::Deserialize,
)]
pub enum KeyMessage {
    Down,
    Up,
    SystemDown,
    SystemUp,
}

bitflags::bitflags! {
    #[derive(
        Default,
        serde_derive::Serialize,
        serde_derive::Deserialize
    )]
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
        control: SwellId,
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
pub type WindowEventCallback =
    Box<dyn FnMut(WindowEvent) -> anyhow::Result<EventResponse>>;

impl ReaperWindow {
    /// Registers a fallible callback for general window input events.
    pub fn on_window_event(
        &self,
        callback: impl FnMut(WindowEvent) -> anyhow::Result<EventResponse>
            + 'static,
    ) {
        *self.window_event_callback.borrow_mut() = Some(Box::new(callback));
    }
}

pub(super) fn dispatch_window_event(
    window: &ReaperWindow,
    event: WindowEvent,
) -> EventResponse {
    if window.callbacks_disabled.get() {
        return EventResponse::Handled;
    }
    let callback = window.window_event_callback.borrow_mut().take();
    let Some(mut callback) = callback else {
        return EventResponse::ForwardToWindow;
    };
    let mut event = Some(event);
    match invoke_callback("window input event", &mut || {
        callback(event.take().expect("window event dispatched once"))
    }) {
        Ok(response) => {
            let mut slot = window.window_event_callback.borrow_mut();
            if slot.is_none() {
                *slot = Some(callback);
            }
            response
        }
        Err(_) => {
            window.callbacks_disabled.set(true);
            EventResponse::Handled
        }
    }
}

pub type WidgetEventCallback =
    Box<dyn FnMut(ControlEvent) -> anyhow::Result<EventResponse>>;
pub type ScrollViewEventCallback =
    Box<dyn FnMut(ScrollViewEvent) -> anyhow::Result<EventResponse>>;
pub type ContainerEventCallback =
    Box<dyn FnMut(ContainerEvent) -> anyhow::Result<EventResponse>>;

fn report_callback_failure(context: &str, error: &anyhow::Error) {
    log::error!("GUI callback failed ({context}): {error:#}");
    if crate::Reaper::is_available() {
        let message = format!("GUI callback failed ({context}): {error}\n");
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::Reaper::get().show_console_msg(message);
        }));
    }
}

pub(crate) fn invoke_callback<T>(
    context: &str,
    callback: &mut impl FnMut() -> anyhow::Result<T>,
) -> Result<T, anyhow::Error> {
    invoke_callback_with_reporter(context, callback, |error| {
        report_callback_failure(context, error);
    })
}

fn invoke_callback_with_reporter<T>(
    _context: &str,
    callback: &mut impl FnMut() -> anyhow::Result<T>,
    mut report: impl FnMut(&anyhow::Error),
) -> Result<T, anyhow::Error> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            report(&error);
            Err(error)
        }
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            let error = anyhow::anyhow!("callback panicked: {message}");
            report(&error);
            Err(error)
        }
    }
}

#[cfg(test)]
mod callback_boundary_tests {
    use super::invoke_callback_with_reporter;
    use std::cell::Cell;

    #[test]
    fn callback_boundary_preserves_success_value_without_reporting() {
        let reports = Cell::new(0);
        let mut callback = || Ok::<_, anyhow::Error>(42);

        let value = invoke_callback_with_reporter(
            "test callback",
            &mut callback,
            |_| {
                reports.set(reports.get() + 1);
            },
        );

        assert_eq!(value.unwrap(), 42);
        assert_eq!(reports.get(), 0);
    }

    #[test]
    fn callback_boundary_reports_error_exactly_once() {
        let reports = Cell::new(0);
        let mut callback =
            || Err::<(), _>(anyhow::anyhow!("expected failure"));

        let error = invoke_callback_with_reporter(
            "test callback",
            &mut callback,
            |_| {
                reports.set(reports.get() + 1);
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("expected failure"));
        assert_eq!(reports.get(), 1);
    }

    #[test]
    fn callback_boundary_normalizes_and_reports_panic_exactly_once() {
        let reports = Cell::new(0);
        let mut callback =
            || -> anyhow::Result<()> { panic!("expected panic") };

        let error = invoke_callback_with_reporter(
            "test callback",
            &mut callback,
            |_| {
                reports.set(reports.get() + 1);
            },
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("callback panicked: expected panic"));
        assert_eq!(reports.get(), 1);
    }
}
#[derive(Default)]
pub(super) struct EventRegistry {
    pub(super) widget_callbacks: HashMap<SwellId, WidgetEventCallback>,
    pub(super) scroll_view_callbacks:
        HashMap<SwellId, ScrollViewEventCallback>,
    pub(super) container_callbacks:
        HashMap<super::widgets::SwellId, ContainerEventCallback>,
    pub(super) direct_container: HashMap<SwellId, super::widgets::SwellId>,
    pub(super) container_parent:
        HashMap<super::widgets::SwellId, super::widgets::SwellId>,
}

/// Result of routing a native control event through the explicit callback
/// tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DispatchResult {
    Handled,
    ForwardToWindow,
    CallbackFailed,
}

impl EventRegistry {
    pub(super) fn register_widget_callback(
        &mut self,
        id: SwellId,
        callback: WidgetEventCallback,
    ) {
        self.widget_callbacks.insert(id, callback);
    }

    pub(super) fn take_widget_callback(
        &mut self,
        id: SwellId,
    ) -> Option<WidgetEventCallback> {
        self.widget_callbacks.remove(&id)
    }

    pub(super) fn restore_widget_callback(
        &mut self,
        id: SwellId,
        callback: WidgetEventCallback,
    ) {
        self.widget_callbacks.entry(id).or_insert(callback);
    }

    pub(super) fn register_container_callback(
        &mut self,
        id: super::widgets::SwellId,
        callback: ContainerEventCallback,
    ) {
        self.container_callbacks.insert(id, callback);
    }

    pub(super) fn set_direct_container(
        &mut self,
        child: SwellId,
        container: super::widgets::SwellId,
    ) {
        self.direct_container.insert(child, container);
    }

    pub(super) fn set_container_parent(
        &mut self,
        container: super::widgets::SwellId,
        parent: super::widgets::SwellId,
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

    pub(super) fn restore_missing_into(&mut self, destination: &mut Self) {
        for (id, callback) in self.widget_callbacks.drain() {
            destination.widget_callbacks.entry(id).or_insert(callback);
        }
        for (id, callback) in self.scroll_view_callbacks.drain() {
            destination
                .scroll_view_callbacks
                .entry(id)
                .or_insert(callback);
        }
        for (id, callback) in self.container_callbacks.drain() {
            destination
                .container_callbacks
                .entry(id)
                .or_insert(callback);
        }
        for (id, parent) in self.direct_container.drain() {
            destination.direct_container.entry(id).or_insert(parent);
        }
        for (id, parent) in self.container_parent.drain() {
            destination.container_parent.entry(id).or_insert(parent);
        }
    }

    pub(super) fn register_scroll_view_callback(
        &mut self,
        id: SwellId,
        callback: ScrollViewEventCallback,
    ) {
        self.scroll_view_callbacks.insert(id, callback);
    }

    pub(super) fn take_scroll_view_callback(
        &mut self,
        id: SwellId,
    ) -> Option<ScrollViewEventCallback> {
        self.scroll_view_callbacks.remove(&id)
    }

    pub(super) fn restore_scroll_view_callback(
        &mut self,
        id: SwellId,
        callback: ScrollViewEventCallback,
    ) {
        self.scroll_view_callbacks.entry(id).or_insert(callback);
    }

    /// Routes an event through the widget, its direct container, and any
    /// explicitly configured container parents.  Callbacks are temporarily
    /// removed while they run so a callback may safely replace its own
    /// registration without causing a `RefCell` reborrow panic.
    pub(super) fn dispatch(&mut self, event: ControlEvent) -> DispatchResult {
        let control = event.control();
        if let Some(mut callback) = self.widget_callbacks.remove(&control) {
            let response =
                invoke_callback("widget event", &mut || callback(event));
            let Ok(response) = response else {
                return DispatchResult::CallbackFailed;
            };
            if !self.widget_callbacks.contains_key(&control) {
                self.widget_callbacks.insert(control, callback);
            }
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
            let response = invoke_callback("container event", &mut || {
                callback(container_event)
            });
            let Ok(response) = response else {
                return DispatchResult::CallbackFailed;
            };
            if !self.container_callbacks.contains_key(&container) {
                self.container_callbacks.insert(container, callback);
            }
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

#[cfg(test)]
mod event_failure_tests {
    use super::{ControlEvent, DispatchResult, EventRegistry};
    use crate::swell_gui::widgets::SwellId;

    #[test]
    fn failed_widget_callback_consumes_event_and_is_not_restored() {
        let mut registry = EventRegistry::default();
        let id = SwellId(11);
        registry.register_widget_callback(
            id,
            Box::new(|_| Err(anyhow::anyhow!("widget failed"))),
        );

        assert_eq!(
            registry.dispatch(ControlEvent::ButtonClicked { control: id }),
            DispatchResult::CallbackFailed,
        );
        assert!(!registry.widget_callbacks.contains_key(&id));
    }
}

impl ControlEvent {
    pub(super) const fn control(self) -> SwellId {
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
    id: SwellId,
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
        container: super::widgets::SwellId,
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
    id: SwellId,
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
    id: SwellId,
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
        id: SwellId,
        mut callback: impl FnMut(ControlEvent) -> anyhow::Result<EventResponse>
            + 'static,
    ) {
        self.events.borrow_mut().register_widget_callback(
            id,
            Box::new(move |event| callback(event)),
        );
    }

    /// Registers a semantic callback for a ScrollView. The callback receives
    /// a clamped content offset and a platform-neutral movement source.
    pub fn on_scroll_view_event(
        &self,
        id: SwellId,
        mut callback: impl FnMut(ScrollViewEvent) -> anyhow::Result<EventResponse>
            + 'static,
    ) {
        self.events.borrow_mut().register_scroll_view_callback(
            id,
            Box::new(move |event| callback(event)),
        );
    }

    /// Delivers a semantic ScrollView event to the registered callback.
    ///
    /// This is primarily used by native viewport message handlers. Keeping
    /// the delivery operation on `ReaperWindow` avoids exposing the internal
    /// event registry while allowing platform adapters to remain separate from
    /// the public event contract.
    pub(crate) fn emit_scroll_view_event(
        &self,
        id: SwellId,
        event: ScrollViewEvent,
    ) -> EventResponse {
        if self.callbacks_disabled.get() {
            return EventResponse::Handled;
        }
        let callback = self.events.borrow_mut().take_scroll_view_callback(id);
        let Some(mut callback) = callback else {
            return EventResponse::ForwardToWindow;
        };
        let response =
            invoke_callback("ScrollView event", &mut || callback(event));
        if let Ok(response) = response {
            let mut events = self.events.borrow_mut();
            if !events.scroll_view_callbacks.contains_key(&id) {
                events.restore_scroll_view_callback(id, callback);
            }
            return response;
        }
        self.callbacks_disabled.set(true);
        EventResponse::Handled
    }

    pub fn on_container_event(
        &self,
        id: super::widgets::SwellId,
        mut callback: impl FnMut(ContainerEvent) -> anyhow::Result<EventResponse>
            + 'static,
    ) {
        self.events.borrow_mut().register_container_callback(
            id,
            Box::new(move |event| callback(event)),
        );
    }

    pub(super) fn dispatch_control_event(
        &self,
        event: ControlEvent,
    ) -> DispatchResult {
        if self.callbacks_disabled.get() {
            return DispatchResult::Handled;
        }
        let control = event.control();
        let (widget_callback, mut registry) = {
            let mut events = self.events.borrow_mut();
            let callback = events.take_widget_callback(control);
            let detached = std::mem::take(&mut *events);
            (callback, detached)
        };
        let result = if let Some(mut callback) = widget_callback {
            match invoke_callback("widget event", &mut || callback(event)) {
                Ok(response) => {
                    registry.restore_widget_callback(control, callback);
                    match response {
                        EventResponse::Handled => DispatchResult::Handled,
                        EventResponse::ForwardToWindow => {
                            DispatchResult::ForwardToWindow
                        }
                        EventResponse::Ignore
                        | EventResponse::ForwardToParent => {
                            registry.dispatch(event)
                        }
                    }
                }
                Err(_) => DispatchResult::Handled,
            }
        } else {
            registry.dispatch(event)
        };
        registry.restore_missing_into(&mut self.events.borrow_mut());
        if result == DispatchResult::CallbackFailed {
            self.callbacks_disabled.set(true);
        }
        result
    }

    pub fn set_control_container(
        &self,
        child: SwellId,
        container: super::widgets::SwellId,
    ) {
        self.events
            .borrow_mut()
            .set_direct_container(child, container);
    }

    pub fn set_container_parent(
        &self,
        container: super::widgets::SwellId,
        parent: super::widgets::SwellId,
    ) {
        self.events
            .borrow_mut()
            .set_container_parent(container, parent);
    }
}
