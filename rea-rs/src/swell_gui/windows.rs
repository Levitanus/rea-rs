use super::{
    events::EventRegistry,
    widgets::{
        Button, CheckBox, ComboBox, ControlHandle, ControlId, ControlKind,
        ControlRect, ControlRegistry, EditField, GroupBox, ListBox,
        StaticLabel,
    },
};
use crate::{ptr_wrappers::Hwnd, ReaRsError, Reaper, ReaperResult};
use rea_rs_low::raw;
use serde_derive::{Deserialize, Serialize};
use std::{cell::Cell, cell::RefCell, ffi::CString, ptr::NonNull};

pub type WindowId = String;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct WindowPlacement {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl WindowPlacement {
    pub(super) fn from_rect(rect: raw::RECT) -> Option<Self> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 1 || height < 1 {
            return None;
        }
        Some(Self {
            left: rect.left,
            top: rect.top,
            width,
            height,
        })
    }
}

/// Position of a REAPER docker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DockPosition {
    Bottom = 0,
    Left = 1,
    Top = 2,
    Right = 3,
    Floating = 4,
}

impl TryFrom<i32> for DockPosition {
    type Error = ReaRsError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Bottom),
            1 => Ok(Self::Left),
            2 => Ok(Self::Top),
            3 => Ok(Self::Right),
            4 => Ok(Self::Floating),
            -1 => Err(ReaRsError::InvalidObject("docker not found")),
            _ => Err(ReaRsError::IntEnum(format!(
                "unknown docker position: {value}"
            ))),
        }
    }
}

/// Description of a new top-level REAPER window.
#[derive(Clone, Debug)]
pub struct WindowSpec {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub resizable: bool,
    pub no_minimize: bool,
    pub no_close: bool,
    pub dock_ident: String,
    pub allow_show: bool,
}

impl WindowSpec {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        let dock_ident =
            format!("rea_rs_{}", title.to_lowercase().replace(' ', "_"));
        Self {
            title,
            width: 300,
            height: 200,
            resizable: true,
            no_minimize: false,
            no_close: false,
            dock_ident,
            allow_show: true,
        }
    }

    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    pub fn resizable(mut self, value: bool) -> Self {
        self.resizable = value;
        self
    }

    pub fn no_minimize(mut self, value: bool) -> Self {
        self.no_minimize = value;
        self
    }

    pub fn no_close(mut self, value: bool) -> Self {
        self.no_close = value;
        self
    }

    pub fn dock_ident(mut self, ident: impl Into<String>) -> Self {
        self.dock_ident = ident.into();
        self
    }

    pub fn allow_show(mut self, value: bool) -> Self {
        self.allow_show = value;
        self
    }
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self::new("rea-rs window")
    }
}

/// Callback interface for an owned REAPER window.
pub trait WindowHandler: 'static {
    fn window_id(&self) -> WindowId;
    fn window(&self) -> &ReaperWindow;
    fn on_open(&mut self) {}
    fn on_close(&mut self) -> bool {
        true
    }
    fn on_destroy(&mut self) {}
    fn on_command(&mut self, _command: super::events::WindowCommand) {}
    fn on_control_event(&mut self, _event: super::events::ControlEvent) {}
    fn on_container_event(&mut self, _event: super::events::ContainerEvent) {}
    fn on_resize(&mut self, _width: i32, _height: i32) {}
    fn on_activate(&mut self, _active: bool) {}
    fn on_timer(&mut self, _id: usize) {}
}

/// A SWELL/Win32 window handle.
pub struct ReaperWindow {
    pub(super) hwnd: Hwnd,
    pub(super) owned: bool,
    pub(crate) show_on_register: bool,
    pub(super) floating_rect: Cell<Option<raw::RECT>>,
    pub(super) docked: Cell<bool>,
    pub(super) dock_ident: Option<String>,
    pub(super) controls: RefCell<ControlRegistry>,
    pub(super) events: RefCell<EventRegistry>,
}

impl ReaperWindow {
    pub(crate) fn owned(
        hwnd: raw::HWND,
        show_on_register: bool,
        dock_ident: String,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        Ok(Self {
            hwnd,
            owned: true,
            show_on_register,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: Some(dock_ident),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
        })
    }

    pub fn from_hwnd(hwnd: raw::HWND) -> ReaperResult<Self> {
        Self::from_hwnd_with_dock_ident(hwnd, None)
    }

    /// Wraps an existing window and associates it with a logical docker ID.
    ///
    /// The ID is needed when a borrowed wrapper is used to undock a window:
    /// the native HWND may have been recreated or reparented, while the
    /// persisted floating placement is keyed by the logical docker ID.
    pub fn from_hwnd_with_dock_ident(
        hwnd: raw::HWND,
        dock_ident: impl Into<Option<String>>,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        if !Reaper::is_available() {
            return Err(ReaRsError::InvalidObject(
                "Reaper is not initialized",
            ));
        }
        let valid = unsafe { Reaper::get().swell().IsWindow(hwnd.as_ptr()) };
        if !valid {
            return Err(ReaRsError::InvalidObject("window is not valid"));
        }
        Ok(Self {
            hwnd,
            owned: false,
            show_on_register: false,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: dock_ident.into(),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
        })
    }

    /// Registers a child control using its native SWELL/Win32 integer ID.
    pub(crate) fn register_control(
        &self,
        id: ControlId,
        kind: ControlKind,
        hwnd: raw::HWND,
    ) -> ReaperResult<ControlHandle> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        let handle = ControlHandle { id, kind, hwnd };
        self.controls.borrow_mut().register(handle);
        Ok(handle)
    }

    /// Updates a registered control after native HWND recreation/reparenting.
    pub(crate) fn rebind_control(
        &self,
        id: ControlId,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        self.controls.borrow_mut().rebind(id, hwnd)
    }

    pub(crate) fn unregister_control(
        &self,
        id: ControlId,
    ) -> Option<ControlHandle> {
        self.controls.borrow_mut().unregister(id)
    }

    pub(super) fn clear_controls(&self) {
        self.controls.borrow_mut().clear();
        self.events.borrow_mut().clear();
    }

    /// Reconnects registered logical controls with their current child HWNDs.
    ///
    /// SWELL may recreate or reparent native child windows during a docker
    /// transition. The logical ID and kind remain stable, so only the HWND
    /// reverse index needs to be refreshed.
    pub(super) fn rebind_controls(&self) -> ReaperResult<()> {
        self.check_window()?;
        let ids: Vec<_> = self.controls.borrow().ids().collect();
        for id in ids {
            let hwnd = unsafe { Self::swell()?.GetDlgItem(self.hwnd(), id.0) };
            if !hwnd.is_null() {
                self.rebind_control(id, hwnd)?;
            }
        }
        Ok(())
    }

    pub fn control(&self, id: ControlId) -> Option<ControlHandle> {
        self.controls.borrow().get(id)
    }

    /// Creates a child control through SWELL's dialog-control factory.
    fn create_control_handle(
        &self,
        id: ControlId,
        kind: ControlKind,
        hwnd: raw::HWND,
    ) -> ReaperResult<ControlHandle> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        self.register_control(id, kind, hwnd)
    }

    fn prepare_control_creation(&self, rect: ControlRect) -> ReaperResult<()> {
        self.check_window()?;
        if rect.width < 1 || rect.height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid control size",
            ));
        }
        Ok(())
    }

    pub fn create_button(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<Button> {
        self.create_button_in(self.hwnd(), id, label, rect)
    }

    pub fn create_button_in(
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

    pub fn create_edit_field(
        &self,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<EditField> {
        self.create_edit_field_in(self.hwnd(), id, rect, flags)
    }

    pub fn create_edit_field_in(
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

    pub fn create_label(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<StaticLabel> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_label(
                self.hwnd(),
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

    pub fn create_checkbox(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<CheckBox> {
        self.create_checkbox_in(self.hwnd(), id, label, rect)
    }

    pub fn create_checkbox_in(
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

    pub fn create_group_box(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<GroupBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_group_box(
                self.hwnd(),
                id.0,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(GroupBox::new(self.create_control_handle(
            id,
            ControlKind::Static,
            hwnd,
        )?))
    }

    pub fn create_combo_box(
        &self,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        self.create_combo_box_in(self.hwnd(), id, rect, flags)
    }

    pub fn create_combo_box_in(
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

    pub fn create_list_box(
        &self,
        id: ControlId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        self.create_list_box_in(self.hwnd(), id, rect, styles)
    }

    pub fn create_list_box_in(
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

/// The procedure installed on owned windows.
pub(crate) unsafe extern "C" fn window_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if !Reaper::is_available() {
        return 0;
    }
    let reaper = Reaper::get_mut();
    let Some(key) = reaper.window_id_for_hwnd(hwnd) else {
        log::warn!("window message for unregistered HWND {:p}", hwnd);
        return Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR;
    };
    match msg {
        raw::WM_CLOSE => {
            let allow = reaper
                .windows
                .get_mut(&key)
                .map(|handler| handler.on_close())
                .unwrap_or(false);
            if allow {
                unsafe {
                    reaper.swell().DestroyWindow(hwnd);
                }
            }
            1
        }
        raw::WM_DESTROY => {
            if let Some(low) =
                Reaper::is_available().then(|| Reaper::get().low())
            {
                if low.pointers().DockWindowRemove.is_some() {
                    unsafe {
                        low.DockWindowRemove(hwnd);
                    }
                }
            }
            if let Some(mut handler) = reaper.windows.remove(&key) {
                handler.on_destroy();
            }
            0
        }
        raw::WM_COMMAND => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let id = (wparam as usize & 0xffff) as i32;
            let code = ((wparam as usize >> 16) & 0xffff) as i32;
            if id == raw::IDCANCEL as i32 && lparam == 0 {
                let allow = handler.on_close();
                if allow {
                    unsafe {
                        reaper.swell().DestroyWindow(hwnd);
                    }
                }
                return 1;
            }
            let command = if lparam == 0 {
                super::events::WindowCommand::Menu { id }
            } else {
                super::events::WindowCommand::Control {
                    id,
                    notification: super::events::CommandNotification::from_raw(
                        code,
                    ),
                }
            };
            handler.on_command(command);
            if lparam != 0 {
                let control_id = super::widgets::ControlId(id);
                let event =
                    handler.window().control(control_id).and_then(|control| {
                        super::events::decode_control_event(
                            control.kind,
                            control_id,
                            super::events::CommandNotification::from_raw(code),
                        )
                    });
                if let Some(event) = event {
                    let widget_response = handler
                        .window()
                        .events
                        .borrow_mut()
                        .widget_callbacks
                        .get_mut(&control_id)
                        .map(|callback| callback(event));
                    if matches!(
                        widget_response,
                        Some(super::events::EventResponse::Handled)
                    ) {
                        return 0;
                    }

                    let container = handler
                        .window()
                        .events
                        .borrow()
                        .direct_container
                        .get(&control_id)
                        .copied();
                    if let Some(container) = container {
                        let container_event =
                            super::events::ContainerEvent::child(
                                container, event,
                            );
                        let container_response = handler
                            .window()
                            .events
                            .borrow_mut()
                            .container_callbacks
                            .get_mut(&container)
                            .map(|callback| callback(container_event));
                        if matches!(
                            container_response,
                            Some(super::events::ContainerResponse::Handled)
                        ) {
                            return 0;
                        }
                        if matches!(container_response, Some(super::events::ContainerResponse::ForwardToWindow)) {
							handler.on_control_event(event);
						}
                    } else {
                        handler.on_control_event(event);
                    }
                }
            }
            0
        }
        raw::WM_HSCROLL | raw::WM_VSCROLL => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let control = handler
                .window()
                .controls
                .borrow()
                .get_by_hwnd(lparam as raw::HWND);
            if let Some(control) = control {
                handler.on_control_event(
                    super::events::ControlEvent::Scroll {
                        control: control.id,
                        code: (wparam as usize & 0xffff) as i32,
                    },
                );
            }
            0
        }
        raw::WM_NOTIFY => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if lparam == 0 {
                return 0;
            }
            let header = &*(lparam as *const super::events::NotifyHeader);
            let control_id = super::widgets::ControlId(header.id_from as i32);
            if handler.window().control(control_id).is_some() {
                handler.on_control_event(
                    super::events::ControlEvent::Notified {
                        control: control_id,
                        code: header.code,
                    },
                );
            }
            0
        }
        raw::WM_PAINT => {
            let mut paint = std::mem::zeroed();
            let hdc = reaper.swell().BeginPaint(hwnd, &mut paint);
            if !hdc.is_null() {
                reaper.swell().paint_window_background(hdc, &paint.rcPaint);
                reaper.swell().EndPaint(hwnd, &mut paint);
            }
            0
        }
        raw::WM_SIZE => {
            let mut rect = std::mem::zeroed();
            Reaper::get().swell().GetClientRect(hwnd, &mut rect);
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            handler.on_resize(rect.right - rect.left, rect.bottom - rect.top);
            0
        }
        raw::WM_TIMER => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_timer(wparam as usize);
            }
            0
        }
        raw::WM_ACTIVATE => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_activate((wparam as usize & 0xffff) != 0);
            }
            0
        }
        _ => Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR,
    }
}
