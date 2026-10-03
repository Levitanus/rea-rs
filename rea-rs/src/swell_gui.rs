//! High-level, platform-neutral APIs for REAPER's SWELL GUI.

pub mod events;
pub mod layout;
pub mod widgets;
pub mod windowing;
pub mod windows;

use rea_rs_low::raw;

pub use events::{
    CommandNotification, ContainerEvent, ControlEvent,
    EventResponse, WidgetEventCallback, WindowCommand,
};
pub use widgets::{
    Button, CheckBox, ComboBox, ContainerId, ControlHandle, ControlId,
    ControlKind, ControlRect, EditField, GroupBox, ListBox, ListView,
    NativeContainer, ProgressBar, RadioButton, ReaperControl, StaticLabel,
    TabControl, Trackbar, TreeView,
};
pub use windows::{
    DockPosition, LayoutContainer, LayoutPanel, ReaperWindow, WindowHandler,
    WindowId, WindowSpec,
};
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_event_preserves_child_event_and_identity() {
        let container = ContainerId(ControlId(42));
        let child = ControlEvent::ButtonClicked {
            control: ControlId(7),
        };

        assert_eq!(
            ContainerEvent::child(container, child),
            ContainerEvent::Child {
                container,
                event: child,
            }
        );
    }

    #[test]
    fn native_container_keeps_stable_control_identity() {
        let handle = ControlHandle {
            id: ControlId(42),
            kind: ControlKind::Static,
            hwnd: std::ptr::null_mut(),
        };
        let container = NativeContainer::new(handle);

        assert_eq!(container.id(), ContainerId(ControlId(42)));
        assert_eq!(container.control_id(), ControlId(42));
        assert!(container.hwnd().is_null());
    }

    #[test]
    fn event_registry_tracks_direct_native_parentage() {
        let mut registry = events::EventRegistry::default();
        let child = ControlId(7);
        let group = ContainerId(ControlId(42));

        registry.set_direct_container(child, group);
        registry.set_container_parent(group, ContainerId(ControlId(99)));

        assert_eq!(registry.direct_container.get(&child), Some(&group));
        assert_eq!(
            registry.container_parent.get(&group),
            Some(&ContainerId(ControlId(99)))
        );
    }
}

/// The procedure installed on owned windows.
pub(crate) unsafe extern "C" fn window_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    windows::window_proc(hwnd, msg, wparam, lparam)
}
