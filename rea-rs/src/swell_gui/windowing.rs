use super::windows::{DockPosition, ReaperWindow, WindowPlacement};
use crate::{ExtState, ReaRsError, Reaper, ReaperResult};
use rea_rs_low::{raw, Swell};
use std::ffi::{CStr, CString};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";

impl ReaperWindow {
    pub fn hwnd(&self) -> raw::HWND {
        self.hwnd.as_ptr()
    }
    pub fn is_owned(&self) -> bool {
        self.owned.get()
    }

    pub(crate) fn relinquish_native_ownership(&self) {
        self.owned.set(false);
    }

    pub(crate) fn destroy_owned_native(&self) {
        self.destroy_internal();
    }

    pub(crate) fn remember_floating_rect(&self) -> ReaperResult<()> {
        let rect = self.window_rect()?;
        self.floating_rect.set(Some(rect));
        if let (Some(dock_ident), Some(placement)) =
            (self.dock_ident.as_deref(), WindowPlacement::from_rect(rect))
        {
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

    pub(super) fn swell() -> ReaperResult<&'static Swell> {
        if !Reaper::is_available() {
            return Err(ReaRsError::InvalidObject(
                "Reaper is not initialized",
            ));
        }
        Ok(Reaper::get().swell())
    }

    pub(super) fn check_window(&self) -> ReaperResult<()> {
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
            unsafe {
                low.DockWindowActivate(self.hwnd());
            }
        }
        self.rebind_controls()?;
        self.apply_default_layout()?;
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
        let main_hwnd =
            low.pointers().GetMainHwnd.as_ref().ok_or_else(|| {
                ReaRsError::UnexpectedAPI("GetMainHwnd not available".into())
            })?;
        let main_hwnd = main_hwnd();
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
            let rect = self.saved_floating_rect()?.ok_or(
                ReaRsError::InvalidObject(
                    "floating window rectangle is unavailable",
                ),
            )?;
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
        let rect =
            self.saved_floating_rect()?
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
        self.rebind_controls()?;
        self.apply_default_layout()?;
        Ok(())
    }

    pub fn is_docked(&self) -> ReaperResult<bool> {
        self.check_window()?;
        if self.owned.get() {
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
        if !self.owned.get() {
            return Ok(());
        }
        self.destroy_internal();
        self.owned.set(false);
        Ok(())
    }

    pub(crate) fn destroy_internal(&self) {
        if !self.owned.replace(false) {
            return;
        }
        self.reset_ui();
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
        if self.owned.get() {
            self.destroy_internal();
        }
    }
}
