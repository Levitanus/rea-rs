use super::ReaperWindow;
use super::layout::WidgetSize;
use crate::{ReaRsError, ReaperResult};
use rea_rs_low::raw;
use std::{collections::HashMap, ffi::CString};

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
