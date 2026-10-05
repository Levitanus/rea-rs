//! High-level, platform-neutral APIs for REAPER's SWELL GUI.

pub mod drawing;
pub mod events;
pub mod layout;
pub mod scroll;
pub mod widgets;
pub mod windowing;
pub mod windows;

use rea_rs_low::raw;

pub use drawing::{
    default_logfont, Bitmap, Brush, DrawTextFlags, DrawTextOptions, Font,
    FontCharset, FontSpec, Icon, ImageList, ImageSize,
    LiceBitmap, LiceBitmapKind, LiceBlitOptions, LiceCombineMode, LiceFont,
    LicePoint, LiceRect, LiceSurface, LiceTextOptions, ListViewImageListKind,
    PaintInfo, Pen, PenStyle,
};
pub use events::{
    CommandNotification, ContainerEvent, ControlEvent, EventResponse,
    KeyMessage, KeyModifiers, MouseButton, MouseButtons, MouseMessage,
    NativeKey, ScrollViewEvent, ScrollViewEventSource, WidgetEventCallback,
    WindowCommand, WindowEvent, WindowEventCallback,
};
pub use scroll::{
    decode_scroll_command, ScrollCommand, ScrollMetrics, ScrollOffset,
    ScrollState, ScrollbarRenderer, ScrollbarVisibility,
};
pub use widgets::CreationContext;
pub use widgets::{
    Button, Canvas, CheckBox, ComboBox, ContainerId, ControlHandle, ControlId,
    ControlKind, ControlRect, EditField, GroupBox, ListBox, ListView,
    NativeContainer, ProgressBar, RadioButton, ReaperControl, StaticLabel,
    TabControl, Trackbar, TreeView,
};
pub use windows::{
    DockPosition, ReaperWindow, ScrollView, WindowHandler, WindowId,
    WindowSpec,
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

    #[test]
    fn paint_info_keeps_damage_and_client_bounds_separate() {
        let info = PaintInfo {
            damage_rect: layout::Rect::new(2, 3, 4, 5),
            client_rect: layout::Rect::new(0, 0, 80, 60),
        };
        assert_eq!(info.damage_rect, layout::Rect::new(2, 3, 4, 5));
        assert_eq!(info.client_rect, layout::Rect::new(0, 0, 80, 60));
    }

    #[test]
    fn general_window_events_preserve_native_input_payloads() {
        let event = WindowEvent::Mouse {
            message: events::MouseMessage::Down(events::MouseButton::Left),
            position: layout::Point { x: 13, y: 24 },
            buttons: events::MouseButtons::LEFT,
        };
        assert!(matches!(
            event,
            WindowEvent::Mouse {
                position: layout::Point { x: 13, y: 24 },
                ..
            }
        ));
        assert!(matches!(WindowEvent::Text('x'), WindowEvent::Text('x')));
        assert!(matches!(WindowEvent::Focus(true), WindowEvent::Focus(true)));
    }

    #[test]
    fn drawing_options_are_semantic_and_default_to_left_top() {
        assert!(DrawTextOptions::default()
            .alignment
            .contains(DrawTextFlags::LEFT | DrawTextFlags::TOP));
        assert_eq!(LiceBlitOptions::default().mode, LiceCombineMode::Copy);
        assert_eq!(ImageSize::new(32, 16).width, 32);
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
