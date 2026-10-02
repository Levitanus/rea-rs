use super::widgets::{ControlId, ControlKind};
use super::windows::ReaperWindow;
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
pub enum ContainerResponse {
    Handled,
    Ignore,
    ForwardToParent,
    ForwardToWindow,
}

pub type WidgetEventCallback = Box<dyn FnMut(ControlEvent) -> EventResponse>;
pub type ContainerEventCallback =
    Box<dyn FnMut(ContainerEvent) -> ContainerResponse>;

#[derive(Default)]
pub(super) struct EventRegistry {
    pub(super) widget_callbacks: HashMap<ControlId, WidgetEventCallback>,
    pub(super) container_callbacks:
        HashMap<super::widgets::ContainerId, ContainerEventCallback>,
    pub(super) direct_container:
        HashMap<ControlId, super::widgets::ContainerId>,
    pub(super) container_parent:
        HashMap<super::widgets::ContainerId, super::widgets::ContainerId>,
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
        self.container_callbacks.clear();
        self.direct_container.clear();
        self.container_parent.clear();
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

    pub fn on_container_event(
        &self,
        id: super::widgets::ContainerId,
        callback: impl FnMut(ContainerEvent) -> ContainerResponse + 'static,
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
