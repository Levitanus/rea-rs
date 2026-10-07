//! High-level APIs for building and interacting with REAPER's SWELL GUI.
//!
//! The API provides typed controls, window lifecycle helpers, layout and
//! scrolling primitives, event decoding, menus, and retained GDI/LICE drawing
//! resources. It wraps SWELL/Win32 behavior rather than promising identical
//! native behavior on every backend; APIs that call REAPER or SWELL generally
//! require REAPER to be initialized and should be used on its UI thread.
//!
//! Public APIs are available through this module (for example,
//! `rea_rs::swell_gui::widgets::CreationContext`) and selected commonly used
//! types are re-exported at the crate root (for example,
//! `rea_rs::ReaperWindow`). The submodules group APIs by responsibility:
//! [`drawing`] for resources and paint surfaces, [`events`] for decoded input,
//! [`layout`] for geometry and flow layout, [`menu`] for native menus,
//! [`scroll`] for scroll state, [`widgets`] for controls and their creation,
//! [`windowing`] for window operations, and [`windows`] for window ownership
//! and callbacks. Native procedure dispatch is an implementation detail.

pub mod drawing;
pub mod events;
pub(crate) mod host_proc;
pub mod layout;
pub mod menu;
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
    Pen, PenStyle, TextMetrics,
};
pub use events::{
    CommandNotification, ContainerEvent, ControlEvent, EventResponse,
    KeyMessage, KeyModifiers, MouseButton, MouseButtons, MouseMessage,
    NativeKey, ScrollViewEvent, ScrollViewEventSource, WidgetEventCallback,
    WindowCommand, WindowEvent, WindowEventCallback,
};
pub use layout::SignedPoint;
pub use layout::{Panel, PanelLayout, PanelRects, PanelSizes};
pub use menu::{CustomMenuContext, CustomMenuPhase, Menu, MenuItem};
pub use scroll::{
    decode_scroll_command, ScrollCommand, ScrollMetrics, ScrollOffset,
    ScrollState, ScrollbarRenderer, ScrollbarVisibility,
};
pub use widgets::{
    Button, Canvas, CheckBox, ComboBox, ComboBoxOptions, ControlHandle,
    ControlKind, ControlRect, EditField, EditFieldOptions, GroupBox, ListBox,
    ListBoxOptions, ListView, ListViewOptions, NativeContainer, ProgressBar,
    ProgressBarOptions, RadioButton, RadioButtonOptions, ReaperControl,
    StaticLabel, SwellId, TabControl, TabControlOptions, Trackbar,
    TrackbarOptions, TreeView, TreeViewOptions, VirtualComboBox,
    VirtualIconButton, VirtualListBox, VirtualSlider, VirtualStaticText,
};
pub use widgets::{CreationContext, PanelContext};
pub use windowing::{capture_window, client_to_screen, screen_to_client};
pub use windows::{
    DockPosition, ReaperWindow, ScrollView, WindowHandler, WindowId,
    WindowSpec,
};
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_event_preserves_child_event_and_identity() {
        let container = SwellId(42);
        let child = ControlEvent::ButtonClicked {
            control: SwellId(7),
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
            id: SwellId(42),
            kind: ControlKind::Static,
            hwnd: std::ptr::null_mut(),
        };
        let container = NativeContainer::new(handle);

        assert_eq!(container.id(), SwellId(42));
        assert_eq!(container.control_id(), SwellId(42));
        assert!(container.hwnd().is_null());
    }

    #[test]
    fn event_registry_tracks_direct_native_parentage() {
        let mut registry = events::EventRegistry::default();
        let child = SwellId(7);
        let group = SwellId(42);

        registry.set_direct_container(child, group);
        registry.set_container_parent(group, SwellId(99));

        assert_eq!(registry.direct_container.get(&child), Some(&group));
        assert_eq!(registry.container_parent.get(&group), Some(&SwellId(99)));
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
        assert!(DrawTextFlags::WORD_BREAK.bits() != 0);
        assert!(DrawTextFlags::NO_PREFIX.bits() != 0);
    }

    #[test]
    fn swell_ids_use_shared_numeric_serde_representation() {
        let id = SwellId::new_menu();
        let encoded = serde_json::to_string(&id).unwrap();
        let decoded: SwellId = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, id);
        assert_eq!(u32::from(id), id.0);
        assert_ne!(SwellId::new_control(), SwellId::new_menu());
    }

    #[test]
    fn allocated_swell_ids_are_nonzero_and_preserve_u32_values() {
        let control = SwellId::new_control();
        let menu = SwellId::new_menu();
        let timer = SwellId::new_timer();
        assert_ne!(control.0, 0);
        assert_ne!(menu.0, 0);
        assert_ne!(timer.0, 0);
        assert_eq!(SwellId::from(u32::MAX).0, u32::MAX);
        assert_eq!(u32::from(SwellId(u32::MAX)), u32::MAX);
    }

    #[test]
    fn serializable_gui_value_types_round_trip() {
        let size = layout::WidgetSize::new_fill_x(180, 24)
            .set_min_x(80)
            .set_max_x(400);
        let encoded = serde_json::to_string(&size).unwrap();
        let decoded: layout::WidgetSize =
            serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, size);

        let event = ControlEvent::ButtonClicked {
            control: SwellId(7),
        };
        let encoded = serde_json::to_string(&event).unwrap();
        let decoded: ControlEvent = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn hwnd_tokens_and_control_handles_round_trip_without_validation() {
        let token = crate::ReaperHwnd::from_raw(0x1234usize as raw::HWND);
        let encoded = serde_json::to_string(&token).unwrap();
        let decoded: crate::ReaperHwnd =
            serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, token);
        assert_eq!(decoded.as_raw(), token.as_raw());

        let handle = ControlHandle {
            id: SwellId(7),
            kind: ControlKind::Button,
            hwnd: token.as_raw(),
        };
        let encoded = serde_json::to_string(&handle).unwrap();
        let decoded: ControlHandle = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, handle);
    }

    #[test]
    fn panel_context_tracks_created_items_and_updates_allocation_config() {
        let mut layout = PanelLayout::new().with_sizes(PanelSizes {
            top: 24,
            bottom: 16,
            left: 40,
            right: 32,
        });
        layout.push(
            Panel::Top,
            layout::LayoutItem {
                size: layout::WidgetSize::new(100, 24),
            },
        );
        for panel in [Panel::Bottom, Panel::Left, Panel::Right] {
            layout.push(
                panel,
                layout::LayoutItem {
                    size: layout::WidgetSize::new(20, 16),
                },
            );
        }
        let output = layout.allocate(layout::Rect::new(0, 0, 320, 200));
        assert_eq!(output.top, Some(layout::Rect::new(0, 0, 320, 24)));
        assert_eq!(output.central, layout::Rect::new(40, 24, 248, 160));
    }

    #[test]
    fn menu_control_and_timer_apis_share_the_same_id_type() {
        fn accepts_swell_id(_: SwellId) {}
        accepts_swell_id(SwellId::new_menu());
        accepts_swell_id(SwellId::new_control());
        accepts_swell_id(SwellId::new_timer());
    }

    #[test]
    fn added_native_notifications_decode_to_semantic_events() {
        let id = SwellId(77);
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
        let id = SwellId(88);
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
        let id = SwellId(91);
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
