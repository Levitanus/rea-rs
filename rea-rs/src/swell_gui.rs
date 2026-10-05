//! High-level, platform-neutral APIs for REAPER's SWELL GUI.

pub mod drawing;
pub mod events;
pub(crate) mod host_proc;
pub mod layout;
pub mod scroll;
pub mod widgets;
pub mod windowing;
pub mod windows;

use rea_rs_low::raw;

pub use drawing::{
    default_logfont, Bitmap, Brush, DrawTextFlags, DrawTextOptions, Font,
    FontCharset, FontSpec, Icon, ImageList, ImageSize, LiceBitmap,
    LiceBitmapKind, LiceBlitOptions, LiceCombineMode, LiceFont, LicePoint,
    LiceRect, LiceSurface, LiceTextOptions, ListViewImageListKind, PaintInfo,
    Pen, PenStyle,
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
    TabControl, Trackbar, TreeView, VirtualComboBox, VirtualIconButton,
    VirtualListBox, VirtualSlider, VirtualStaticText,
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

    #[test]
    fn added_native_notifications_decode_to_semantic_events() {
        let id = ControlId(77);
        assert_eq!(
            events::decode_notify_event(
                ControlKind::Tab,
                id,
                raw::TCN_SELCHANGE
            ),
            ControlEvent::TabSelectionChanged { control: id },
        );
        assert_eq!(
            events::decode_notify_event(
                ControlKind::ListView,
                id,
                raw::LVN_ITEMCHANGED
            ),
            ControlEvent::ListViewItemChanged { control: id },
        );
        assert_eq!(
            events::decode_notify_event(
                ControlKind::TreeView,
                id,
                raw::TVN_SELCHANGED
            ),
            ControlEvent::TreeSelectionChanged { control: id },
        );
        assert_eq!(
            events::decode_notify_event(ControlKind::TreeView, id, 123),
            ControlEvent::Notified {
                control: id,
                code: 123
            },
        );
    }

    #[test]
    fn trackbar_scroll_commands_decode_as_value_changes() {
        let id = ControlId(88);
        assert_eq!(
            events::decode_control_event(
                ControlKind::Trackbar,
                id,
                CommandNotification::Other(0),
            ),
            Some(ControlEvent::TrackbarChanged { control: id }),
        );
    }

    #[test]
    fn virtual_widget_commands_ignore_nonsemantic_messages() {
        let id = ControlId(91);
        assert_eq!(
            events::decode_virtual_event(
                rea_rs_low::VirtualControlKind::Slider,
                id,
                raw::WM_HSCROLL as i32,
            ),
            Some(ControlEvent::VirtualSliderChanged { control: id }),
        );
        assert_eq!(
            events::decode_virtual_event(
                rea_rs_low::VirtualControlKind::Slider,
                id,
                0,
            ),
            None,
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
    host_proc::window_proc(hwnd, msg, wparam, lparam)
}
