//! GUI support for Reaper extensions using egui-baseview.
//!
//! Enable with cargo feature `egui-baseview`.
//!
//! This module provides:
//! - `ReaperParentWindow`: a parent-window adapter that can be passed to
//!   `egui_baseview::EguiWindow::open_parented()`.
//! - Re-exports of `baseview`, `egui_baseview`, and `egui`.

pub use baseview;
pub use egui;
pub use egui_baseview;
pub type Queue = egui_baseview::ExtraOutputCommands;

use crate::ptr_wrappers::Hwnd;

/// A handle to Reaper's main window that can be used as a parent for
/// baseview/egui-baseview windows.
pub struct ReaperParentWindow {
    hwnd: Hwnd,
}

impl ReaperParentWindow {
    pub(crate) fn new(hwnd: Hwnd) -> Self {
        Self { hwnd }
    }

    /// Returns the raw SWELL/Win32 HWND pointer.
    ///
    /// # Safety
    ///
    /// The pointer is valid for the lifetime of the Reaper process.
    pub unsafe fn as_raw_hwnd(&self) -> *mut rea_rs_low::raw::HWND__ {
        self.hwnd.as_ptr()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::ReaperParentWindow;
    use c_str_macro::c_str;
    use raw_window_handle::{
        HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
    };
    use rea_rs_low::Swell;

    extern "C" {
        fn gdk_x11_window_get_xid(
            window: *mut std::ffi::c_void,
        ) -> std::os::raw::c_ulong;
    }

    unsafe fn xid_from_swell_hwnd(
        hwnd: *mut rea_rs_low::raw::HWND__,
    ) -> Option<std::os::raw::c_ulong> {
        let gdk_window =
            Swell::get().SWELL_GetOSWindow(hwnd, c_str!("GdkWindow").as_ptr());
        if gdk_window.is_null() {
            return None;
        }
        Some(gdk_x11_window_get_xid(gdk_window))
    }

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(
            &self,
        ) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
            let xid = unsafe { xid_from_swell_hwnd(self.hwnd.as_ptr()) }
                .ok_or(raw_window_handle::HandleError::Unavailable)?;
            let raw = RawWindowHandle::Xlib(XlibWindowHandle::new(xid));
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::ReaperParentWindow;
    use std::num::NonZeroIsize;
    use raw_window_handle::{
        HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    };

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(
            &self,
        ) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
            let hwnd = NonZeroIsize::new(self.hwnd.as_ptr() as isize)
                .ok_or(raw_window_handle::HandleError::Unavailable)?;
            let raw = RawWindowHandle::Win32(Win32WindowHandle::new(hwnd));
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::ReaperParentWindow;
    use c_str_macro::c_str;
    use std::ptr::NonNull;
    use raw_window_handle::{
        AppKitWindowHandle, HasWindowHandle, RawWindowHandle, WindowHandle,
    };
    use rea_rs_low::Swell;

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(
            &self,
        ) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
            let ns_view = unsafe {
                Swell::get().SWELL_GetOSWindow(
                    self.hwnd.as_ptr(),
                    c_str!("NSView").as_ptr(),
                )
            };
            let ns_view = NonNull::new(ns_view)
                .ok_or(raw_window_handle::HandleError::Unavailable)?;
            let raw = RawWindowHandle::AppKit(AppKitWindowHandle::new(ns_view));
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }
}

// ─── DockableEguiWindow
// ───────────────────────────────────────────────────────

/// Dock slot index used by [`DockableEguiWindow::dock`].
///
/// REAPER numbers its docker panes starting from 0.  Pass [`DOCK_FLOATING`] to
/// keep the window floating (no dock).
pub const DOCK_FLOATING: Option<u32> = None;

/// Wraps a [`baseview`] / [`egui_baseview`] window so that it can be opened as
/// a free-floating OS window **or** embedded inside a REAPER docker pane.
///
/// # Usage
///
/// ```ignore
/// static mut WIN: Option<DockableEguiWindow> = None;
///
/// fn my_action(_flag: i32) -> rea_rs::ReaRsResult<()> {
///     let win = unsafe { WIN.get_or_insert_with(DockableEguiWindow::new_default) };
///     win.set_dock(None, MyState::default(), |_ctx, _q, _s| {}, my_ui);
/// }
///
/// fn my_ui(ui: &mut egui::Ui, queue: &mut Queue, state: &mut MyState) { … }
/// ```
///
/// Calling [`set_dock`](DockableEguiWindow::set_dock) again with a different
/// value destroys the old window and opens a fresh one in the requested
/// position.
///
/// Call [`poll_resize`](DockableEguiWindow::poll_resize) from a REAPER timer
/// (or from any main-thread callback) so that REAPER dock resize events are
/// propagated to the embedded egui viewport.
pub struct DockableEguiWindow {
    title: String,
    /// Stable identity string used by REAPER to persist dock position across
    /// sessions (passed to `DockWindowAddEx`).
    ident: String,
    size: baseview::dpi::Size,
    current_dock: Option<u32>,
    state: DockWindowRunState,
}

enum DockWindowRunState {
    Closed,
    /// A free-floating `open_blocking` window running on its own thread.
    Floating {
        /// Sending any value here causes the egui loop to close the viewport
        /// on the next frame.
        close_tx: std::sync::mpsc::SyncSender<()>,
        _thread: std::thread::JoinHandle<()>,
    },
    /// A window embedded inside a REAPER docker pane via
    /// `SWELL_CreateXBridgeWindow` + `DockWindowAddEx`.
    #[cfg(target_os = "linux")]
    Docked {
        window_handle: baseview::WindowHandle,
        bridge_hwnd: Hwnd,
    },
}

// Helper: wraps a raw X11 Window ID so baseview can use it as a parent.
#[cfg(target_os = "linux")]
struct XlibParent(u64);

#[cfg(target_os = "linux")]
impl raw_window_handle::HasWindowHandle for XlibParent {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
    {
        let raw = raw_window_handle::RawWindowHandle::Xlib(
            raw_window_handle::XlibWindowHandle::new(self.0),
        );
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

// Inner state that carries the user state plus a channel receiver for
// programmatic close when the window is floating.
struct WithCloseRx<S> {
    inner: S,
    close_rx: std::sync::mpsc::Receiver<()>,
}

impl DockableEguiWindow {
    /// Create a new descriptor.  The window is **not** opened yet; call
    /// [`set_dock`](Self::set_dock) to open it.
    pub fn new(
        title: impl Into<String>,
        ident: impl Into<String>,
        size: baseview::dpi::Size,
    ) -> Self {
        Self {
            title: title.into(),
            ident: ident.into(),
            size,
            current_dock: Some(u32::MAX), /* sentinel – force open on first
                                           * call */
            state: DockWindowRunState::Closed,
        }
    }

    /// Returns the dock slot the window is currently in, or `None` if
    /// floating.
    pub fn current_dock(&self) -> Option<u32> {
        match &self.state {
            DockWindowRunState::Closed => None,
            DockWindowRunState::Floating { .. } => None,
            #[cfg(target_os = "linux")]
            DockWindowRunState::Docked { .. } => self.current_dock,
        }
    }

    /// Returns `true` when an egui window is currently open.
    pub fn is_open(&self) -> bool {
        !matches!(self.state, DockWindowRunState::Closed)
    }

    /// Open or re-open the window with the given dock configuration.
    ///
    /// * `dock_slot = None` → floating OS window (`open_blocking` thread).
    /// * `dock_slot = Some(n)` → embedded in REAPER docker pane `n`.
    ///
    /// If the requested dock slot equals the current one the call is a no-op.
    /// Otherwise the existing window is closed first.
    ///
    /// `build` runs once when the egui context is first created.
    /// `update` runs every frame.
    pub fn set_dock<S, B, U>(
        &mut self,
        dock_slot: Option<u32>,
        state: S,
        build: B,
        update: U,
    ) where
        S: Send + 'static,
        B: FnMut(&egui::Context, &mut Queue, &mut S)
            + Send
            + 'static,
        U: FnMut(&mut egui::Ui, &mut Queue, &mut S)
            + Send
            + 'static,
    {
        if self.current_dock == dock_slot && self.is_open() {
            return;
        }
        self.close();
        self.open_impl(dock_slot, state, build, update);
    }

    /// Programmatically close the window (no-op if already closed).
    pub fn close(&mut self) {
        match std::mem::replace(&mut self.state, DockWindowRunState::Closed) {
            DockWindowRunState::Closed => {}
            DockWindowRunState::Floating { close_tx, _thread } => {
                let _ = close_tx.try_send(());
                // We do not join – the thread will self-terminate on the next
                // egui frame after it receives the signal.
            }
            #[cfg(target_os = "linux")]
            DockWindowRunState::Docked {
                window_handle,
                bridge_hwnd,
            } => {
                window_handle.close();
                let reaper_low = crate::Reaper::get().low();
                let swell = rea_rs_low::Swell::get();
                unsafe {
                    reaper_low.DockWindowRemove(bridge_hwnd.as_ptr());
                    swell.DestroyWindow(bridge_hwnd.as_ptr());
                }
            }
        }
        self.current_dock = None;
    }

    /// Call this from a REAPER timer (or any main-thread periodic callback) to
    /// propagate REAPER docker resize events into the embedded egui viewport.
    ///
    /// On Linux this triggers `SWELL`'s built-in child-window resize mechanism
    /// (timer id 1010 in `xbridgeProc`).  No-op on other platforms or when the
    /// window is floating / closed.
    pub fn poll_resize(&self) {
        #[cfg(target_os = "linux")]
        if let DockWindowRunState::Docked { bridge_hwnd, .. } = &self.state {
            unsafe {
                rea_rs_low::Swell::get().SetTimer(
                    bridge_hwnd.as_ptr(),
                    1010,
                    50,
                    None,
                );
            }
        }
    }

    // ── private helpers
    // ──────────────────────────────────────────────────────

    fn open_impl<S, B, U>(
        &mut self,
        dock_slot: Option<u32>,
        state: S,
        build: B,
        update: U,
    ) where
        S: Send + 'static,
        B: FnMut(&egui::Context, &mut Queue, &mut S)
            + Send
            + 'static,
        U: FnMut(&mut egui::Ui, &mut Queue, &mut S)
            + Send
            + 'static,
    {
        match dock_slot {
            None => self.open_floating(state, build, update),
            #[cfg(target_os = "linux")]
            Some(slot) => {
                if let Err(e) =
                    self.open_docked_linux(slot, state, build, update)
                {
                    log::error!("DockableEguiWindow: failed to dock: {e}");
                    // Fall back to floating on error
                    self.current_dock = None;
                }
            }
            #[cfg(not(target_os = "linux"))]
            Some(_) => {
                // Docking not yet implemented on Windows/macOS – fall back to
                // floating.
                self.open_floating(state, build, update);
            }
        }
        self.current_dock = dock_slot;
    }

    fn open_floating<S, B, U>(&mut self, state: S, mut build: B, update: U)
    where
        S: Send + 'static,
        B: FnMut(&egui::Context, &mut Queue, &mut S)
            + Send
            + 'static,
        U: FnMut(&mut egui::Ui, &mut Queue, &mut S)
            + Send
            + 'static,
    {
        let (close_tx, close_rx) = std::sync::mpsc::sync_channel::<()>(1);

        let wrapped_state = WithCloseRx {
            inner: state,
            close_rx,
        };

        let wrapped_build =
            move |ctx: &egui::Context,
                  queue: &mut Queue,
                  s: &mut WithCloseRx<S>| {
                build(ctx, queue, &mut s.inner);
            };

        let mut update = update;
        let wrapped_update =
            move |ui: &mut egui::Ui,
                  queue: &mut Queue,
                  s: &mut WithCloseRx<S>| {
                if s.close_rx.try_recv().is_ok() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                update(ui, queue, &mut s.inner);
            };

        let settings = egui_baseview::EguiWindowSettings::default()
            .with_tile(self.title.clone())
            .with_size(self.size)
            .with_scale_policy(baseview::WindowScalePolicy::SystemScaleFactor)
            .with_graphics_config(egui_baseview::GraphicsConfig::default());

        let thread = std::thread::spawn(move || {
            egui_baseview::EguiWindow::open_blocking(
                settings,
                wrapped_state,
                wrapped_build,
                |_full_output, _viewport_output, _state| {},
                wrapped_update,
            );
        });

        self.state = DockWindowRunState::Floating {
            close_tx,
            _thread: thread,
        };
    }

    #[cfg(target_os = "linux")]
    fn open_docked_linux<S, B, U>(
        &mut self,
        slot: u32,
        state: S,
        build: B,
        update: U,
    ) -> Result<(), String>
    where
        S: Send + 'static,
        B: FnMut(&egui::Context, &mut Queue, &mut S)
            + Send
            + 'static,
        U: FnMut(&mut egui::Ui, &mut Queue, &mut S)
            + Send
            + 'static,
    {
        use rea_rs_low::raw;

        let reaper_low = crate::Reaper::get().low();
        let swell = rea_rs_low::Swell::get();

        let (w, h) = match self.size {
            baseview::dpi::Size::Logical(size) => {
                (size.width as i32, size.height as i32)
            }
            baseview::dpi::Size::Physical(size) => {
                (size.width as i32, size.height as i32)
            }
        };
        let rect = raw::RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };

        let mut x11_wref: *mut std::ffi::c_void = std::ptr::null_mut();

        // Create a top-level SWELL HWND whose underlying GDK/X11 window can
        // host a foreign X11 child (our egui-baseview GL window).
        let bridge_raw = unsafe {
            swell.SWELL_CreateXBridgeWindow(
                std::ptr::null_mut(), // NULL parent → top-level
                &mut x11_wref,
                &rect,
            )
        };

        let bridge_hwnd = std::ptr::NonNull::new(bridge_raw)
            .ok_or("SWELL_CreateXBridgeWindow returned NULL")?;

        let x11_xid = x11_wref as u64;
        if x11_xid == 0 {
            unsafe { swell.DestroyWindow(bridge_raw) };
            return Err(
                "SWELL_CreateXBridgeWindow gave a zero X11 window ID".into()
            );
        }

        // Register this HWND with REAPER's docker system.
        let c_title =
            std::ffi::CString::new(self.title.as_str()).unwrap_or_default();
        let c_ident =
            std::ffi::CString::new(self.ident.as_str()).unwrap_or_default();
        unsafe {
            reaper_low.DockWindowAddEx(
                bridge_raw,
                c_title.as_ptr(),
                c_ident.as_ptr(),
                true, // allowShow
            );
            // Record which dock pane the user wants (persisted by REAPER).
            reaper_low.Dock_UpdateDockID(c_ident.as_ptr(), slot as i32);
            // Ask REAPER to show/place the window.
            swell.ShowWindow(bridge_raw, raw::SW_SHOW as i32);
        }

        // Open the egui-baseview window parented to the X11 bridge window.
        let xlib_parent = XlibParent(x11_xid);
        let settings = egui_baseview::EguiWindowSettings::default()
            .with_tile(self.title.clone())
            .with_size(self.size)
            .with_scale_policy(baseview::WindowScalePolicy::SystemScaleFactor)
            .with_graphics_config(egui_baseview::GraphicsConfig::default());

        let window_handle = egui_baseview::EguiWindow::open_parented(
            &xlib_parent,
            settings,
            state,
            build,
            |_full_output, _viewport_output, _state| {},
            update,
        );

        // Trigger initial resize so the egui viewport fills the bridge window.
        unsafe {
            swell.SetTimer(bridge_raw, 1010, 100, None);
        }

        self.state = DockWindowRunState::Docked {
            window_handle,
            bridge_hwnd,
        };
        Ok(())
    }
}
