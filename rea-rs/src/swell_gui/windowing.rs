use super::windows::{DockPosition, ReaperWindow, WindowPlacement};
use crate::{ExtState, ReaRsError, Reaper, ReaperResult};
use rea_rs_low::{raw, Swell};
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    time::Duration,
};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";

fn timer_interval_ms(interval: Duration) -> ReaperResult<u32> {
    let millis = interval.as_millis();
    if millis == 0 || interval.subsec_nanos() % 1_000_000 != 0 {
        return Err(ReaRsError::UnsuccessfulOperation(
            "timer interval must be a positive whole number of milliseconds",
        ));
    }
    u32::try_from(millis).map_err(|_| {
        ReaRsError::UnsuccessfulOperation("timer interval is too large")
    })
}

struct ChildWindowCollection {
    handles: Vec<raw::HWND>,
    panic: Option<Box<dyn std::any::Any + Send>>,
}

unsafe extern "C" fn collect_child_window(
    hwnd: raw::HWND,
    context: raw::LPARAM,
) -> raw::BOOL {
    let collection = &mut *(context as *mut ChildWindowCollection);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        collection.handles.push(hwnd);
    })) {
        Ok(()) => 1,
        Err(payload) => {
            collection.panic = Some(payload);
            0
        }
    }
}

impl ReaperWindow {
    /// Returns the current raw SWELL/Win32 handle.
    ///
    /// The handle is non-owning and may become invalid when the native window
    /// is destroyed or recreated. Check operations that return
    /// [`ReaperResult`] before relying on continued validity. This raw handle
    /// is exposed for interoperability; callers must uphold native API
    /// parameter and pointer-lifetime requirements when using it directly.
    pub fn hwnd(&self) -> raw::HWND {
        self.hwnd.as_ptr()
    }

    /// Returns whether this wrapper owns and is responsible for destroying
    /// the native window.
    pub fn is_owned(&self) -> bool {
        self.owned.get()
    }

    /// Returns the native parent reported by SWELL/Win32.
    ///
    /// Native APIs can report an owner instead of a parent for some
    /// top-level windows. Use [`Self::owner`] when owner relationships are
    /// what you need.
    pub fn parent(&self) -> ReaperResult<Option<Self>> {
        self.check_window()?;
        let hwnd = unsafe { Self::swell()?.GetParent(self.hwnd()) };
        Self::wrap_discovered_window(hwnd)
    }

    /// Returns the native owner reported by `GetWindow(GW_OWNER)`, if any.
    pub fn owner(&self) -> ReaperResult<Option<Self>> {
        self.check_window()?;
        let hwnd =
            unsafe { Self::swell()?.GetWindow(self.hwnd(), raw::GW_OWNER) };
        Self::wrap_discovered_window(hwnd)
    }

    /// Returns an iterator over immediate child windows in native sibling
    /// order.
    ///
    /// Returned wrappers are non-owning. The native hierarchy can change
    /// between enumeration and later use, so each operation checks that its
    /// HWND is still valid. Each item is fallible because an HWND can become
    /// invalid between enumeration and being wrapped.
    pub fn children(
        &self,
    ) -> ReaperResult<impl Iterator<Item = ReaperResult<Self>>> {
        self.check_window()?;
        let swell = Self::swell()?;
        let mut handles = Vec::new();
        let mut seen = HashSet::new();
        let mut child =
            unsafe { swell.GetWindow(self.hwnd(), raw::GW_CHILD as i32) };
        while !child.is_null() && seen.insert(child as usize) {
            handles.push(child);
            child = unsafe { swell.GetWindow(child, raw::GW_HWNDNEXT) };
        }
        Ok(handles
            .into_iter()
            .filter(|hwnd| !hwnd.is_null())
            .map(Self::from_hwnd))
    }

    /// Returns an iterator over all descendant windows in native enumeration
    /// order.
    ///
    /// SWELL's `EnumChildWindows` recursively enumerates descendants. The
    /// callback only collects raw handles; wrappers are created after native
    /// enumeration has returned.
    pub fn descendants(
        &self,
    ) -> ReaperResult<impl Iterator<Item = ReaperResult<Self>>> {
        self.check_window()?;
        let mut collection = ChildWindowCollection {
            handles: Vec::new(),
            panic: None,
        };
        unsafe {
            Self::swell()?.EnumChildWindows(
                self.hwnd(),
                Some(collect_child_window),
                &mut collection as *mut ChildWindowCollection as raw::LPARAM,
            );
        }
        if let Some(payload) = collection.panic {
            std::panic::resume_unwind(payload);
        }
        Ok(collection
            .handles
            .into_iter()
            .filter(|hwnd| !hwnd.is_null())
            .map(Self::from_hwnd))
    }

    /// Returns the native class name when SWELL can provide one.
    ///
    /// Class names are diagnostic hints, not stable cross-platform
    /// identifiers. Some SWELL-created windows do not expose a class name.
    pub fn class_name(&self) -> ReaperResult<Option<String>> {
        self.check_window()?;
        let mut buffer = vec![0i8; 512];
        let len = unsafe {
            Self::swell()?.GetClassName(
                self.hwnd(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        if len <= 0 {
            return Ok(None);
        }
        Ok(Some(
            unsafe { CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        ))
    }

    /// Moves this window to the top of its sibling z-order without changing
    /// its position, size, or activation state.
    pub fn bring_to_front(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                (raw::SWP_NOMOVE | raw::SWP_NOSIZE | raw::SWP_NOACTIVATE)
                    as i32,
            );
        }
        Ok(())
    }

    /// Reparents this window to `parent`, or makes it a top-level window when
    /// `parent` is `None`.
    ///
    /// Reparenting REAPER-owned HWNDs can break host layout, input routing,
    /// or docking. Prefer this only for windows whose lifecycle and native
    /// behavior you control. SWELL's `SetParent` return value cannot
    /// unambiguously distinguish failure from a successful change whose old
    /// parent was null, so this method validates handles but cannot report
    /// every native failure.
    pub fn reparent_to(&self, parent: Option<&Self>) -> ReaperResult<()> {
        self.check_window()?;
        if let Some(parent) = parent {
            parent.check_window()?;
            let swell = Self::swell()?;
            if self.hwnd() == parent.hwnd()
                || unsafe { swell.IsChild(self.hwnd(), parent.hwnd()) != 0 }
            {
                return Err(ReaRsError::UnsuccessfulOperation(
                    "a window cannot be reparented beneath itself",
                ));
            }
        }
        unsafe {
            Self::swell()?.SetParent(
                self.hwnd(),
                parent.map_or(std::ptr::null_mut(), Self::hwnd),
            );
        }
        Ok(())
    }

    fn wrap_discovered_window(hwnd: raw::HWND) -> ReaperResult<Option<Self>> {
        if hwnd.is_null() {
            Ok(None)
        } else {
            Self::from_hwnd(hwnd).map(Some)
        }
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

    /// Shows the window and requests repainting.
    ///
    /// An error indicates that REAPER is unavailable or the HWND is no longer
    /// valid; success does not guarantee that the host will keep it visible.
    pub fn show(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.ShowWindow(self.hwnd(), raw::SW_SHOW);
            Self::swell()?.InvalidateRect(self.hwnd(), std::ptr::null(), 1);
        }
        Ok(())
    }

    /// Hides the window without destroying it.
    pub fn hide(&self) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?.ShowWindow(self.hwnd(), raw::SW_HIDE);
        }
        Ok(())
    }

    /// Sets the native window title. Interior NUL bytes are rejected.
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

    /// Reads the title into a fixed-size native buffer.
    ///
    /// An empty string is returned when the backend reports no title. The
    /// result may be truncated if the title exceeds the buffer capacity.
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

    /// Returns the client-area rectangle in client coordinates.
    ///
    /// Native client rectangles normally have an origin of `(0, 0)`; their
    /// right and bottom edges give the client width and height.
    pub fn client_rect(&self) -> ReaperResult<raw::RECT> {
        self.check_window()?;
        let mut rect = unsafe { std::mem::zeroed() };
        unsafe {
            Self::swell()?.GetClientRect(self.hwnd(), &mut rect);
        }
        Ok(rect)
    }

    /// Returns the outer window rectangle in screen coordinates.
    ///
    /// Coordinates are signed and can be negative on multi-monitor desktops.
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

    /// Sets the outer window position and size.
    ///
    /// `x` and `y` use the coordinate space expected by the native parent;
    /// top-level windows generally use screen coordinates. Width and height
    /// must be positive. Native backends may adjust the requested geometry.
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

    /// Changes the outer size while retaining the current outer position.
    /// Width and height must be positive.
    pub fn resize(&self, width: i32, height: i32) -> ReaperResult<()> {
        let rect = self.window_rect()?;
        self.set_pos(rect.left, rect.top, width, height)
    }

    /// Enables or disables native input to this window.
    pub fn set_enabled(&self, enabled: bool) -> ReaperResult<()> {
        self.check_window()?;
        unsafe {
            Self::swell()?
                .EnableWindow(self.hwnd(), if enabled { 1 } else { 0 });
        }
        Ok(())
    }

    /// Reports native visibility, which is distinct from whether the window
    /// is unobscured on screen.
    pub fn is_visible(&self) -> ReaperResult<bool> {
        self.check_window()?;
        Ok(unsafe { Self::swell()?.IsWindowVisible(self.hwnd()) })
    }

    /// Reports whether the native window is enabled for user interaction.
    pub fn is_enabled(&self) -> ReaperResult<bool> {
        self.check_window()?;
        Ok(unsafe { Self::swell()?.IsWindowEnabled(self.hwnd()) })
    }

    /// Requests keyboard focus for this window.
    ///
    /// The host or native backend may choose a different focus target.
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
    /// `id` must be non-zero and `interval` must be a positive whole number
    /// of milliseconds representable by SWELL/Win32. The returned ID is the
    /// ID reported by SWELL/Win32 and should be passed to
    /// [`Self::stop_timer`]. Timer callbacks run on REAPER's UI thread.
    /// Native timer cadence is approximate and may be delayed or coalesced
    /// by the host or operating system; this is not a high-resolution
    /// timer.
    pub fn start_timer(
        &self,
        id: usize,
        interval: Duration,
    ) -> ReaperResult<usize> {
        self.check_window()?;
        if id == 0 {
            return Err(ReaRsError::UnsuccessfulOperation("invalid timer"));
        }
        let interval_ms = timer_interval_ms(interval)?;
        let actual = unsafe {
            Self::swell()?.SetTimer(self.hwnd(), id, interval_ms, None)
        };
        if actual == 0 {
            Err(ReaRsError::UnsuccessfulOperation("SetTimer"))
        } else {
            Ok(actual)
        }
    }

    /// Stops the timer identified by the ID returned from
    /// [`Self::start_timer`].
    pub fn stop_timer(&self, id: usize) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe { Self::swell()?.KillTimer(self.hwnd(), id) };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("KillTimer"))
        } else {
            Ok(())
        }
    }

    /// Stops all timers associated with this window.
    pub fn stop_all_timers(&self) -> ReaperResult<()> {
        self.check_window()?;
        let ok = unsafe { Self::swell()?.KillTimer(self.hwnd(), usize::MAX) };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("KillTimer"))
        } else {
            Ok(())
        }
    }

    /// Docks this owned window in REAPER under the given display name and
    /// logical identifier.
    ///
    /// Requires REAPER's `DockWindowAddEx` API. The method saves floating
    /// placement, reparents/rebinds native children as needed, and may fail if
    /// the host destroys the window during the transition. Calling it while
    /// already docked is a no-op.
    pub fn dock(
        &self,
        name: &str,
        ident: &str,
        allow_show: bool,
    ) -> ReaperResult<()> {
        self.check_window()?;
        if self.docked.get() {
            log::debug!(
                "dock request ignored: hwnd={:?} already docked",
                self.hwnd()
            );
            return Ok(());
        }
        log::warn!(
            "dock transition begin: hwnd={:?} parent={:?} generation={}",
            self.hwnd(),
            unsafe { Self::swell()?.GetParent(self.hwnd()) },
            self.lifecycle_generation.get(),
        );
        self.lifecycle_generation
            .set(self.lifecycle_generation.get().wrapping_add(1));
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
        if !unsafe { Self::swell()?.IsWindow(self.hwnd()) } {
            return Err(ReaRsError::InvalidObject(
                "window was destroyed while docking",
            ));
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
        log::warn!(
            "dock transition complete: hwnd={:?} parent={:?} generation={}",
            self.hwnd(),
            unsafe { Self::swell()?.GetParent(self.hwnd()) },
            self.lifecycle_generation.get(),
        );
        Ok(())
    }

    /// Removes an owned window from its docker and restores its saved floating
    /// position. Requires the relevant REAPER docker APIs and a valid main
    /// window; calling it when already floating is a no-op.
    pub fn float(&self) -> ReaperResult<()> {
        self.check_window()?;
        if !self.docked.get() {
            log::debug!(
                "float request ignored: hwnd={:?} already floating",
                self.hwnd()
            );
            return Ok(());
        }
        let floating_rect =
            self.saved_floating_rect()?
                .ok_or(ReaRsError::InvalidObject(
                    "floating window rectangle is unavailable",
                ))?;
        let low = Reaper::get().low();
        let main_hwnd_fn =
            low.pointers().GetMainHwnd.as_ref().ok_or_else(|| {
                ReaRsError::UnexpectedAPI("GetMainHwnd not available".into())
            })?;
        let main_hwnd = main_hwnd_fn();
        if main_hwnd.is_null()
            || !unsafe { Self::swell()?.IsWindow(main_hwnd) }
        {
            return Err(ReaRsError::NullPtr("valid main window"));
        }
        log::warn!(
            "float transition begin: hwnd={:?} parent={:?} owner={:?} generation={} rect={floating_rect:?}",
            self.hwnd(),
            unsafe { Self::swell()?.GetParent(self.hwnd()) },
            unsafe { Self::swell()?.GetWindow(self.hwnd(), raw::GW_OWNER) },
            self.lifecycle_generation.get(),
        );
        self.lifecycle_generation
            .set(self.lifecycle_generation.get().wrapping_add(1));
        if low.pointers().DockWindowRemove.is_none() {
            return Err(ReaRsError::UnexpectedAPI(
                "DockWindowRemove not available".into(),
            ));
        }
        unsafe {
            low.DockWindowRemove(self.hwnd());
        }
        if !unsafe { Self::swell()?.IsWindow(self.hwnd()) } {
            return Err(ReaRsError::InvalidObject(
                "window was destroyed while removing it from the docker",
            ));
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
        unsafe {
            Self::swell()?.SetParent(self.hwnd(), std::ptr::null_mut());
            Self::swell()?.SetWindowLong(
                self.hwnd(),
                raw::GWL_HWNDPARENT,
                main_hwnd as isize,
            );
        }
        if !unsafe { Self::swell()?.IsWindow(self.hwnd()) } {
            return Err(ReaRsError::InvalidObject(
                "window was destroyed while restoring its parent",
            ));
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
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                floating_rect.left,
                floating_rect.top,
                floating_rect.right - floating_rect.left,
                floating_rect.bottom - floating_rect.top,
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
        if !unsafe { Self::swell()?.IsWindow(self.hwnd()) } {
            return Err(ReaRsError::InvalidObject(
                "window was destroyed while recreating its floating surface",
            ));
        }
        unsafe {
            Self::swell()?.SetWindowPos(
                self.hwnd(),
                std::ptr::null_mut(),
                floating_rect.left,
                floating_rect.top,
                floating_rect.right - floating_rect.left,
                floating_rect.bottom - floating_rect.top,
                (raw::SWP_NOZORDER | raw::SWP_FRAMECHANGED) as i32,
            );
        }
        self.rebind_controls()?;
        self.apply_default_layout()?;
        Ok(())
    }

    /// Reports whether this window is currently docked.
    ///
    /// For owned windows, reports the wrapper's current lifecycle state. For
    /// borrowed windows, queries REAPER and may fail if the host API is
    /// absent.
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

    /// Requests that REAPER refresh the docker containing this window.
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

    /// Destroys the native window if this wrapper owns it.
    ///
    /// Borrowed wrappers are left intact. Destruction tears down registered
    /// child UI, timers, menus, and docker registration.
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
        if !self.hwnd().is_null() && Reaper::is_available() {
            unsafe {
                Reaper::get()
                    .swell()
                    .SetMenu(self.hwnd(), std::ptr::null_mut());
            }
        }
        self.menu.replace(None);
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

#[cfg(test)]
mod tests {
    use super::timer_interval_ms;
    use std::time::Duration;

    #[test]
    fn timer_interval_requires_positive_whole_milliseconds() {
        assert_eq!(timer_interval_ms(Duration::from_millis(1)).unwrap(), 1);
        assert!(timer_interval_ms(Duration::ZERO).is_err());
        assert!(timer_interval_ms(Duration::from_nanos(999_999)).is_err());
        assert!(timer_interval_ms(Duration::from_micros(1_500)).is_err());
    }

    #[test]
    fn timer_interval_rejects_values_outside_native_range() {
        assert!(
            timer_interval_ms(Duration::from_millis(u32::MAX as u64 + 1))
                .is_err()
        );
    }
}
