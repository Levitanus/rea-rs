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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ControlId(pub i32);

impl ControlId {
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlHandle {
    pub id: ControlId,
    pub kind: ControlKind,
    pub hwnd: raw::HWND,
}

/// Rectangle used by control positioning helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ControlRect {
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
    pub fn new(handle: ControlHandle) -> Self {
        Self { handle }
    }

    pub fn from_control(control: ReaperControl) -> Self {
        Self::new(control.handle)
    }

    pub fn id(&self) -> ContainerId {
        ContainerId(self.handle.id)
    }

    pub fn control_id(&self) -> ControlId {
        self.handle.id
    }

    pub fn hwnd(&self) -> raw::HWND {
        self.handle.hwnd
    }

    pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
        ReaperControl::new(self.handle).set_rect(rect)
    }

    pub fn show(&self, visible: bool) -> ReaperResult<()> {
        ReaperControl::new(self.handle).show(visible)
    }

    pub fn enable(&self, enabled: bool) -> ReaperResult<()> {
        ReaperControl::new(self.handle).enable(enabled)
    }
}

impl ReaperControl {
    pub fn new(handle: ControlHandle) -> Self {
        Self { handle }
    }

    pub fn id(&self) -> ControlId {
        self.handle.id
    }

    pub fn kind(&self) -> ControlKind {
        self.handle.kind
    }

    pub fn hwnd(&self) -> raw::HWND {
        self.handle.hwnd
    }

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

    pub fn enable(&self, enabled: bool) -> ReaperResult<()> {
        let swell = ReaperWindow::swell()?;
        unsafe {
            swell.EnableWindow(self.hwnd(), if enabled { 1 } else { 0 });
        }
        Ok(())
    }

    pub fn focus(&self) -> ReaperResult<()> {
        unsafe {
            ReaperWindow::swell()?.SetFocus(self.hwnd());
        }
        Ok(())
    }

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
            pub fn new(handle: ControlHandle) -> Self {
                Self(ReaperControl::new(handle))
            }
            pub fn control(&self) -> ReaperControl {
                self.0
            }
            pub fn id(&self) -> ControlId {
                self.0.id()
            }
            pub fn hwnd(&self) -> raw::HWND {
                self.0.hwnd()
            }
            pub fn show(&self, value: bool) -> ReaperResult<()> {
                self.0.show(value)
            }
            pub fn enable(&self, value: bool) -> ReaperResult<()> {
                self.0.enable(value)
            }
            pub fn focus(&self) -> ReaperResult<()> {
                self.0.focus()
            }
            pub fn set_text(&self, value: &str) -> ReaperResult<()> {
                self.0.set_text(value)
            }
            pub fn text(&self) -> ReaperResult<String> {
                self.0.text()
            }
            pub fn set_rect(&self, rect: ControlRect) -> ReaperResult<()> {
                self.0.set_rect(rect)
            }
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

impl GroupBox {
    pub fn container(&self) -> NativeContainer {
        NativeContainer::from_control(self.control())
    }
}

impl Button {
    pub fn click(&self) -> ReaperResult<()> {
        self.send_message(raw::BM_CLICK, 0, 0).map(|_| ())
    }
}

impl CheckBox {
    pub fn checked(&self) -> ReaperResult<bool> {
        Ok(self.send_message(raw::BM_GETCHECK, 0, 0)?
            == raw::BST_CHECKED as isize)
    }
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
    pub fn select_all(&self) -> ReaperResult<()> {
        self.send_message(raw::EM_SETSEL, 0, -1).map(|_| ())
    }
}

impl ComboBox {
    pub fn selected_index(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::CB_GETCURSEL, 0, 0)? as i32)
    }
    pub fn select(&self, index: i32) -> ReaperResult<()> {
        self.send_message(raw::CB_SETCURSEL, index as usize, 0)
            .map(|_| ())
    }
    pub fn add_item(&self, text: &str) -> ReaperResult<i32> {
        let text = CString::new(text)?;
        Ok(
            self.send_message(raw::CB_ADDSTRING, 0, text.as_ptr() as isize)?
                as i32,
        )
    }
}

impl ListBox {
    pub fn selected_index(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::LB_GETCURSEL, 0, 0)? as i32)
    }
    pub fn select(&self, index: i32) -> ReaperResult<()> {
        self.send_message(raw::LB_SETCURSEL, index as usize, 0)
            .map(|_| ())
    }
    pub fn add_item(&self, text: &str) -> ReaperResult<i32> {
        let text = CString::new(text)?;
        Ok(
            self.send_message(raw::LB_ADDSTRING, 0, text.as_ptr() as isize)?
                as i32,
        )
    }
}

impl Trackbar {
    pub fn position(&self) -> ReaperResult<i32> {
        Ok(self.send_message(raw::TBM_GETPOS, 0, 0)? as i32)
    }
    pub fn set_position(&self, value: i32) -> ReaperResult<()> {
        self.send_message(raw::TBM_SETPOS, 1, value as isize)
            .map(|_| ())
    }
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
    pub fn set_position(&self, value: i32) -> ReaperResult<()> {
        self.send_message(raw::PBM_SETPOS, value as usize, 0)
            .map(|_| ())
    }
    pub fn set_range(&self, min: i32, max: i32) -> ReaperResult<()> {
        self.send_message(raw::PBM_SETRANGE32, min as usize, max as isize)
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
/// it does not create an implicit panel.  Layout panels are available through
/// the opt-in helper methods below.
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

    /// Uses the existing declarative flow layout as an opt-in container.
    pub fn panel(&self, _panel: Panel) -> CreationContext<'a> {
        CreationContext {
            window: self.window,
            parent: self.parent,
            container: self.container,
        }
    }

    pub fn central_panel(&self) -> CreationContext<'a> {
        self.panel(Panel::Central)
    }

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
            },
        );
        Ok(CreationContext {
            window: self.window,
            parent: group.hwnd(),
            container: Some(id),
        })
    }

    pub fn scroll_view(
        &self,
        id: ControlId,
        size: WidgetSize,
        renderer: ScrollbarRenderer,
    ) -> anyhow::Result<CreationContext<'a>> {
        let rect = self.size(size);
        let view = self.window.create_structural_child(self.parent, rect)?;
        let content = self.window.create_structural_child(
            view,
            ControlRect::new(0, 0, rect.width, rect.height),
        )?;
        if renderer == ScrollbarRenderer::CoolSb
            && Reaper::get().low().supports_cool_scrollbars()
        {
            unsafe {
                Reaper::get().low().InitializeCoolSB(view);
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
            },
        );
        self.window.scroll_views.borrow_mut().insert(
            view as usize,
            ScrollViewRuntime {
                id,
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
    /// dispatch while sharing the parent handler's retained resources.
    pub fn canvas(
        &self,
        id: ControlId,
        size: WidgetSize,
    ) -> anyhow::Result<Canvas> {
        let hwnd = self
            .window
            .create_structural_child(self.parent, self.size(size))?;
        self.window.layout.borrow_mut().structural.insert(id, hwnd);
        self.window.register_layout_entry(self.container, id, size);
        Ok(Canvas { id, hwnd })
    }

    pub fn on_widget_event(
        &self,
        id: ControlId,
        callback: impl FnMut(super::events::ControlEvent) -> super::events::EventResponse
            + 'static,
    ) {
        self.window.on_widget_event(id, callback);
    }

    pub fn on_scroll_view_event(
        &self,
        id: ControlId,
        callback: impl FnMut(ScrollViewEvent) -> super::events::EventResponse
            + 'static,
    ) {
        self.window.on_scroll_view_event(id, callback);
    }

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
    pub fn id(&self) -> ControlId {
        self.id
    }
    pub fn hwnd(&self) -> raw::HWND {
        self.hwnd
    }
    pub fn focus(&self) -> ReaperResult<()> {
        unsafe { Reaper::get().swell().SetFocus(self.hwnd) };
        Ok(())
    }
    pub fn capture(&self) -> ReaperResult<()> {
        unsafe { Reaper::get().swell().SetCapture(self.hwnd) };
        Ok(())
    }
    pub fn release_capture(&self) -> ReaperResult<()> {
        Reaper::get().swell().ReleaseCapture();
        Ok(())
    }
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
