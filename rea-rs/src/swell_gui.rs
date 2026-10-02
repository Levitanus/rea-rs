//! High-level, platform-neutral window management for REAPER's SWELL API.
//!
//! This module intentionally does not provide widgets, painting, or a general
//! input system. It provides owned windows, borrowed HWND wrappers, docking,
//! lifetime management, and a small set of window callbacks.

use crate::{
    ptr_wrappers::Hwnd, ExtState, ReaRsError, Reaper, ReaperResult,
};
use rea_rs_low::{raw, Swell};
use serde::{Deserialize, Serialize};
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::Cell,
    ffi::{CStr, CString},
    ptr::NonNull,
};

/// Identifier used by the window-handler registry.
pub type WindowId = isize;

const WINDOW_STATE_SECTION: &str = "rea-rs.window";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WindowPlacement {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl WindowPlacement {
    fn from_rect(rect: raw::RECT) -> Option<Self> {
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
    Other(i32),
}

impl CommandNotification {
    fn from_raw(code: i32) -> Self {
        match code {
            0 => Self::Clicked,
            256 => Self::SetFocus,
            512 => Self::KillFocus,
            768 => Self::EditChange,
            1 => Self::ComboSelectionChanged,
            5 => Self::ComboEditChanged,
            7 => Self::ComboDropDown,
            8 => Self::ComboCloseUp,
            other => Self::Other(other),
        }
    }
}

/// A decoded `WM_COMMAND` event.
///
/// The low word of `wParam` is exposed as the command/control ID. For menu
/// commands, `lParam` is null and the event is represented by `Menu`. For
/// control notifications, `lParam` identifies the originating control and the
/// high word of `wParam` is decoded into `CommandNotification`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowCommand {
    Menu { id: i32 },
    Control {
        id: i32,
        notification: CommandNotification,
    },
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
    fn window(&self) -> &ReaperWindow;
    fn on_open(&mut self) {}
    /// Return `true` to allow the close operation, or `false` to veto it.
    fn on_close(&mut self) -> bool {
        true
    }
    fn on_destroy(&mut self) {}
    fn on_command(&mut self, _command: WindowCommand) {}
    fn on_resize(&mut self, _width: i32, _height: i32) {}
    fn on_activate(&mut self, _active: bool) {}
    fn on_timer(&mut self, _id: usize) {}
}

/// A SWELL/Win32 window handle.
///
/// An owned window is destroyed on drop. A borrowed wrapper never destroys or
/// subclasses its HWND.
pub struct ReaperWindow {
    hwnd: Hwnd,
    owned: bool,
    pub(crate) show_on_register: bool,
    floating_rect: Cell<Option<raw::RECT>>,
    docked: Cell<bool>,
    dock_ident: Option<String>,
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
        })
    }

    pub fn hwnd(&self) -> raw::HWND {
        self.hwnd.as_ptr()
    }
    pub fn is_owned(&self) -> bool {
        self.owned
    }

    pub(crate) fn remember_floating_rect(&self) -> ReaperResult<()> {
        let rect = self.window_rect()?;
        self.floating_rect.set(Some(rect));
        if let (Some(dock_ident), Some(placement)) = (
            self.dock_ident.as_deref(),
            WindowPlacement::from_rect(rect),
        ) {
            let mut state = ExtState::<WindowPlacement, Reaper>::existing(
                WINDOW_STATE_SECTION,
                dock_ident,
                false,
                Reaper::get(),
                None,
            );
            state.set(placement)?;
        }
        Ok(())
    }

    fn saved_floating_rect(&self) -> ReaperResult<Option<raw::RECT>> {
        let Some(dock_ident) = self.dock_ident.as_deref() else {
            return Ok(self.floating_rect.get());
        };
        let state = ExtState::<WindowPlacement, Reaper>::existing(
            WINDOW_STATE_SECTION,
            dock_ident,
            false,
            Reaper::get(),
            None,
        );
        Ok(state.get()?.map(|placement: WindowPlacement| raw::RECT {
            left: placement.left,
            top: placement.top,
            right: placement.left + placement.width,
            bottom: placement.top + placement.height,
        }))
    }

    fn swell() -> ReaperResult<&'static Swell> {
        if !Reaper::is_available() {
            return Err(ReaRsError::InvalidObject(
                "Reaper is not initialized",
            ));
        }
        Ok(Reaper::get().swell())
    }

    fn check_window(&self) -> ReaperResult<()> {
        let swell = Self::swell()?;
        if unsafe { swell.IsWindow(self.hwnd()) } {
            Ok(())
        } else {
            Err(ReaRsError::InvalidObject("window is not valid"))
        }
    }

    pub fn show(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.ShowWindow(self.hwnd(), raw::SW_SHOW);
            Self::swell()?.InvalidateRect(self.hwnd(), std::ptr::null(), 1);
        }
        Ok(())
    }

    pub fn hide(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.ShowWindow(self.hwnd(), raw::SW_HIDE);
        }
        Ok(())
    }

    pub fn set_title(&self, title: &str) -> ReaperResult<()> {
        self.check_window()?;
        let title = CString::new(title)?;
        let ok = unsafe {
            Self::swell()?.SetWindowText(self.hwnd(), title.as_ptr())
        };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("SetWindowText"))
        } else {
            Ok(())
        }
    }

    pub fn title(&self) -> ReaperResult<String> {
        self.check_window()?;
        let mut buffer = vec![0i8; 4096];
        let ok = unsafe {
            Self::swell()?.GetWindowText(
                self.hwnd(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        if ok == 0 {
            return Ok(String::new());
        }
        Ok(unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn client_rect(&self) -> ReaperResult<raw::RECT> {
        self.check_window()?;
        let mut rect = unsafe { std::mem::zeroed() };
        unsafe {
            Self::swell()?.GetClientRect(self.hwnd(), &mut rect);
        }
        Ok(rect)
    }

    pub fn window_rect(&self) -> ReaperResult<raw::RECT> {
        self.check_window()?;
        let mut rect = unsafe { std::mem::zeroed() };
        let ok =
            unsafe { Self::swell()?.GetWindowRect(self.hwnd(), &mut rect) };
        if !ok {
            Err(ReaRsError::UnsuccessfulOperation("GetWindowRect"))
        } else {
            Ok(rect)
        }
    }

    pub fn set_pos(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> ReaperResult<()> {
        self.check_window()?;
        if width < 1 || height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid window size",
            ));
        }
        unsafe {
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                (raw::SWP_NOZORDER | raw::SWP_FRAMECHANGED) as i32,
            );
        }
        Ok(())
    }

    pub fn resize(&self, width: i32, height: i32) -> ReaperResult<()> {
        let rect = self.window_rect()?;
        self.set_pos(rect.left, rect.top, width, height)
    }

    pub fn set_enabled(&self, enabled: bool) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?
                .EnableWindow(self.hwnd(), if enabled { 1 } else { 0 });
        }
        Ok(())
    }

    pub fn is_visible(&self) -> ReaperResult<bool> {
        self.check_window()?;
        Ok(unsafe { Self::swell()?.IsWindowVisible(self.hwnd()) })
    }

    pub fn is_enabled(&self) -> ReaperResult<bool> {
        self.check_window()?;
        Ok(unsafe { Self::swell()?.IsWindowEnabled(self.hwnd()) })
    }

    pub fn focus(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.SetFocus(self.hwnd());
        }
        Ok(())
    }

    /// Invalidates a window region and requests repainting.
    ///
    /// `rect` is in client coordinates. Pass `None` to invalidate the whole
    /// client area.
    pub fn invalidate(&self, rect: Option<&raw::RECT>) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe {
            Self::swell()?.InvalidateRect(
                self.hwnd(),
                rect.map_or(std::ptr::null(), |value| value as *const _),
                1,
            )
        };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("InvalidateRect"))
        } else {
            Ok(())
        }
    }

    /// Sends a message synchronously and returns the window procedure's
    /// result. The caller blocks until the target window processes the
    /// message, so this is appropriate when the result is needed immediately
    /// and re-entrant callback execution is acceptable.
    pub fn send_message(
        &self,
        msg: raw::UINT,
        wparam: raw::WPARAM,
        lparam: raw::LPARAM,
    ) -> ReaperResult<raw::LRESULT> {
        self.check_window()?;
        Ok(unsafe {
            Self::swell()?.SendMessage(self.hwnd(), msg, wparam, lparam)
        })
    }

    /// Posts a message asynchronously and returns after it has been queued.
    /// The target window processes it later on the host message loop; there is
    /// no return value from the target procedure.
    pub fn post_message(
        &self,
        msg: raw::UINT,
        wparam: raw::WPARAM,
        lparam: raw::LPARAM,
    ) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe {
            Self::swell()?.PostMessage(self.hwnd(), msg, wparam, lparam)
        };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("PostMessage"))
        } else {
            Ok(())
        }
    }

    /// Starts a periodic timer that delivers `WM_TIMER` to this window.
    ///
    /// `id` must be non-zero and `interval_ms` is the requested period. The
    /// returned ID is the ID reported by SWELL/Win32 and should be passed to
    /// [`Self::stop_timer`]. Timer callbacks run on REAPER's UI thread.
    pub fn start_timer(
        &self,
        id: usize,
        interval_ms: u32,
    ) -> ReaperResult<usize> {
        self.check_window()?;
        if id == 0 || interval_ms == 0 {
            return Err(ReaRsError::UnsuccessfulOperation("invalid timer"));
        }
        let actual = unsafe {
            Self::swell()?.SetTimer(self.hwnd(), id, interval_ms, None)
        };
        if actual == 0 {
            Err(ReaRsError::UnsuccessfulOperation("SetTimer"))
        } else {
            Ok(actual)
        }
    }

    pub fn stop_timer(&self, id: usize) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe { Self::swell()?.KillTimer(self.hwnd(), id) };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("KillTimer"))
        } else {
            Ok(())
        }
    }

    pub fn stop_all_timers(&self) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe { Self::swell()?.KillTimer(self.hwnd(), usize::MAX) };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("KillTimer"))
        } else {
            Ok(())
        }
    }

    pub fn dock(
        &self,
        name: &str,
        ident: &str,
        allow_show: bool,
    ) -> ReaperResult<()> {
        self.check_window()?;
        let name = CString::new(name)?;
        let ident = CString::new(ident)?;
        // Preserve only the floating geometry. A docker resize is not a
        // suitable floating-window restore rectangle.
        if !self.docked.get() {
            self.remember_floating_rect()?;
        }
        let low = Reaper::get().low();
        if low.pointers().DockWindowAddEx.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockWindowAddEx not available".into(),
            ));
        }
        unsafe {
            low.DockWindowAddEx(
                self.hwnd(),
                name.as_ptr(),
                ident.as_ptr(),
                allow_show,
            );
        }
        self.docked.set(true);
        // Docking can leave a newly-created SWELL window hidden until the
        // next host layout pass. Explicitly show and repaint it so the docked
        // content is initialized immediately.
        self.show()?;
        if low.pointers().DockWindowActivate.is_some() {
            unsafe { low.DockWindowActivate(self.hwnd()); }
        }
        Ok(())
    }

    pub fn float(&self) -> ReaperResult<()> {
        self.check_window()?;
        let low = Reaper::get().low();
        if low.pointers().DockWindowRemove.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockWindowRemove not available".into(),
            ));
        }
        unsafe {
            low.DockWindowRemove(self.hwnd());
        }
        self.docked.set(false);
        // Docking reparents the HWND into REAPER's docker. Removing the
        // docking registration does not restore the original top-level
        // ownership, so explicitly detach it and restore REAPER's main HWND
        // as the owner. Without this, SWELL continues treating the window as
        // a docker child: it may remain invisible and cannot be resized as a
        // floating window.
        // Restore top-level ownership and the last floating geometry. SWELL
        // removes the native frame while the HWND is a docker child.
        let main_hwnd = low
            .pointers()
            .GetMainHwnd
            .as_ref()
            .ok_or_else(|| {
                ReaRsError::UnexpectedAPI("GetMainHwnd not available".into())
            })?;
        let main_hwnd = unsafe { main_hwnd() };
        if main_hwnd.is_null() {
            return Err(ReaRsError::NullPtr("main window"));
        }
        unsafe {
            Self::swell()?.SetParent(self.hwnd(), std::ptr::null_mut());
            Self::swell()?.SetWindowLong(
                self.hwnd(),
                raw::GWL_HWNDPARENT,
                main_hwnd as isize,
            );
        }
        let style = unsafe {
            Self::swell()?.GetWindowLong(self.hwnd(), raw::GWL_STYLE)
        };
        unsafe {
            Self::swell()?.SetWindowLong(
                self.hwnd(),
                raw::GWL_STYLE,
                style & !(raw::WS_CAPTION as isize),
            );
            Self::swell()?.SetWindowLong(
                self.hwnd(),
                raw::GWL_STYLE,
                style | raw::WS_CAPTION as isize | raw::WS_THICKFRAME as isize,
            );
            let rect = self
                .saved_floating_rect()?
                .ok_or(ReaRsError::InvalidObject(
                    "floating window rectangle is unavailable",
                ))?;
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                (raw::SWP_NOZORDER | raw::SWP_FRAMECHANGED) as i32,
            );
        }
        // SWELL treats `SW_SHOW` on an already logically-visible window as
        // `SW_SHOWNA`, which can hide the native backend window during a
        // parent transition. Force a native teardown/recreate pair here.
        self.hide()?;
        self.show()?;
        // The SWELL native-surface transition can reset the top-level
        // position to its default (0, 0). Apply the saved floating geometry
        // once more after the surface has been recreated.
        let rect = self
            .saved_floating_rect()?
            .ok_or(ReaRsError::InvalidObject(
                "floating window rectangle is unavailable",
            ))?;
        unsafe {
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                (raw::SWP_NOZORDER | raw::SWP_FRAMECHANGED) as i32,
            );
        }
        Ok(())
    }

    pub fn is_docked(&self) -> ReaperResult<bool> {
        self.check_window()?;
        if self.owned {
            return Ok(self.docked.get());
        }
        let low = Reaper::get().low();
        if low.pointers().DockIsChildOfDock.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockIsChildOfDock not available".into(),
            ));
        }
        let mut floating = false;
        let dock_index =
            unsafe { low.DockIsChildOfDock(self.hwnd(), &mut floating) };
        // REAPER returns the containing docker index, or -1 when floating.
        Ok(dock_index >= 0)
    }

    pub fn refresh_dock(&self) -> ReaperResult<()> {
        self.check_window()?;
        let low = Reaper::get().low();
        if low.pointers().DockWindowRefreshForHWND.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockWindowRefreshForHWND not available".into(),
            ));
        }
        unsafe {
            low.DockWindowRefreshForHWND(self.hwnd());
        }
        Ok(())
    }

    /// Returns the position of a REAPER docker by its docker ID.
    ///
    /// The result is `-1` when the index is not found, `0` bottom, `1` left,
    /// `2` top, `3` right, and `4` floating. This queries the docker itself;
    /// it does not return the position of this window and does not require the
    /// window to be docked. A missing docker is returned as an error.
    pub fn dock_position(&self, dock_id: i32) -> ReaperResult<DockPosition> {
        let low = Reaper::get().low();
        if low.pointers().DockGetPosition.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockGetPosition not available".into(),
            ));
        }
        DockPosition::try_from(low.DockGetPosition(dock_id))
    }

    pub fn destroy(&mut self) -> ReaperResult<()> {
        if !self.owned {
            return Ok(());
        }
        self.destroy_internal();
        self.owned = false;
        Ok(())
    }

    pub(crate) fn destroy_internal(&self) {
        if !Reaper::is_available() {
            return;
        }
        let swell = Reaper::get().swell();
        unsafe {
            let _ = swell.KillTimer(self.hwnd(), usize::MAX);
            let low = Reaper::get().low();
            if low.pointers().DockWindowRemove.is_some() {
                low.DockWindowRemove(self.hwnd());
            }
            if swell.IsWindow(self.hwnd()) {
                swell.DestroyWindow(self.hwnd());
            }
        }
    }
}

impl Drop for ReaperWindow {
    fn drop(&mut self) {
        if self.owned {
            self.destroy_internal();
        }
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
    let key = hwnd as isize;
    if !reaper.windows.contains_key(&key) {
        log::warn!("window message for unregistered HWND {:p}", hwnd);
        return Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR;
    }
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
            // REAPER's docker keeps a slot registration independently of the
            // native HWND. Remove that registration before dropping the
            // handler, otherwise the slot can remain active after closing the
            // window and later crash during docker layout/resize.
            if let Some(low) = Reaper::is_available().then(|| Reaper::get().low()) {
                if low.pointers().DockWindowRemove.is_some() {
                    unsafe { low.DockWindowRemove(hwnd); }
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
                    unsafe { reaper.swell().DestroyWindow(hwnd); }
                }
                return 1;
            }
            let command = if lparam == 0 {
                WindowCommand::Menu { id }
            } else {
                WindowCommand::Control {
                    id,
                    notification: CommandNotification::from_raw(code),
                }
            };
            handler.on_command(command);
            0
        }
        raw::WM_PAINT => {
            let mut paint = std::mem::zeroed();
            let hdc = reaper.swell().BeginPaint(hwnd, &mut paint);
            if !hdc.is_null() {
                reaper
                    .swell()
                    .paint_window_background(hdc, &paint.rcPaint);
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
