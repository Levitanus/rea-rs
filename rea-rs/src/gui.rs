//! GUI support for Reaper extensions using egui-baseview.
//!
//! Enable with cargo feature `egui-baseview`.
//!
//! This module provides:
//! - [`ReaperParentWindow`]: a type that implements
//!   `raw_window_handle::HasWindowHandle` and
//!   `raw_window_handle::HasDisplayHandle`, allowing it to be passed directly
//!   to `egui_baseview::EguiWindow::open_parented()` to embed an egui window
//!   inside Reaper's main window.
//! - Re-exports of [`baseview`] and [`egui_baseview`] for convenience.
//!
//! # Platform notes
//!
//! | Platform | Window system | How the handle is extracted |
//! |----------|--------------|----------------------------|
//! | Linux    | X11 via GDK  | `SWELL_GetOSWindow(hwnd, "GdkWindow")` then `gdk_x11_window_get_xid` |
//! | Windows  | Win32        | Raw `HWND` cast directly |
//! | macOS    | Cocoa/NSView | `SWELL_GetOSWindow(hwnd, "NSView")` |
//!
//! # Example
//!
//! ```ignore
//! use rea_rs::Reaper;
//! use rea_rs::gui::ReaperParentWindow;
//! use egui_baseview::{EguiWindow, Settings};
//! use baseview::{Size, WindowScalePolicy};
//!
//! struct MyState;
//!
//! impl egui_baseview::Application for MyState {
//!     type UserState = ();
//!     fn new(_window: &mut baseview::Window, _state: &Self::UserState) -> Self {
//!         MyState
//!     }
//!     fn update(
//!         &mut self,
//!         ctx: &egui_baseview::egui::Context,
//!         _queue: &mut egui_baseview::Queue,
//!         _state: &Self::UserState,
//!     ) {
//!         egui_baseview::egui::Window::new("Hello Reaper").show(ctx, |ui| {
//!             ui.label("Hello from egui inside Reaper!");
//!         });
//!     }
//! }
//!
//! fn open_window() {
//!     let reaper = Reaper::get();
//!     let parent = reaper.main_window();
//!
//!     let settings = Settings {
//!         window: baseview::WindowOpenOptions {
//!             title: "My Panel".into(),
//!             size: Size::new(400.0, 300.0),
//!             scale: WindowScalePolicy::SystemScaleFactor,
//!         },
//!         clear_color: [0.1, 0.1, 0.1, 1.0],
//!     };
//!
//!     let _handle = EguiWindow::<MyState>::open_parented(&parent, settings, ());
//! }
//! ```

// Re-export the underlying crates so users only need rea-rs as a dependency.
pub use baseview;
pub use egui_baseview;
// Re-export egui from egui_baseview to avoid version mismatches.
pub use egui_baseview::egui;

use crate::ptr_wrappers::Hwnd;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

/// A handle to Reaper's main window that implements the
/// `raw_window_handle` traits required by baseview / egui-baseview.
///
/// Obtain this via [`Reaper::main_window()`].
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
    /// Do not attempt to cast or use it as a platform-native window handle
    /// on Linux/macOS without first converting via `SWELL_GetOSWindow`.
    pub unsafe fn as_raw_hwnd(&self) -> *mut rea_rs_low::raw::HWND__ {
        self.hwnd.as_ptr()
    }
}

// ─── Platform implementations ────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::{HandleError, ReaperParentWindow};
    use std::ptr::NonNull;

    use c_str_macro::c_str;
    use raw_window_handle::{
        DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
        RawWindowHandle, WindowHandle, XlibDisplayHandle, XlibWindowHandle,
    };
    use rea_rs_low::Swell;

    // GDK / X11 symbols are always present at runtime because Reaper links
    // against GTK.  We declare only what we need here; no extra link
    // directives are required.
    extern "C" {
        fn gdk_x11_window_get_xid(window: *mut std::ffi::c_void) -> std::os::raw::c_ulong;
        fn gdk_window_get_display(window: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn gdk_x11_display_get_xdisplay(
            display: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
    }

    /// Returns `(gdk_window_ptr, xid, xdisplay_ptr)` for the given SWELL HWND.
    unsafe fn gdk_info(
        hwnd: *mut rea_rs_low::raw::HWND__,
    ) -> Result<(u64, *mut std::ffi::c_void), HandleError> {
        let swell = Swell::get();
        // SWELL_GetOSWindow with type "GdkWindow" returns the backing GdkWindow*.
        let gdk_window =
            swell.SWELL_GetOSWindow(hwnd, c_str!("GdkWindow").as_ptr());
        if gdk_window.is_null() {
            return Err(HandleError::Unavailable);
        }
        let xid = gdk_x11_window_get_xid(gdk_window);
        let gdisplay = gdk_window_get_display(gdk_window);
        let xdisplay = gdk_x11_display_get_xdisplay(gdisplay);
        Ok((xid as u64, xdisplay))
    }

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let (xid, _xdisplay) =
                unsafe { gdk_info(self.hwnd.as_ptr()) }?;
            let handle = XlibWindowHandle::new(xid);
            // SAFETY: the handle is valid as long as Reaper's window exists,
            // which outlives any lifetime 'a that can be named here.
            Ok(unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xlib(handle))
            })
        }
    }

    impl HasDisplayHandle for ReaperParentWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            let (_xid, xdisplay) =
                unsafe { gdk_info(self.hwnd.as_ptr()) }?;
            let handle =
                XlibDisplayHandle::new(NonNull::new(xdisplay), 0);
            Ok(unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(handle))
            })
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{HandleError, ReaperParentWindow};
    use raw_window_handle::{
        DisplayHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
        RawWindowHandle, Win32WindowHandle, WindowHandle, WindowsDisplayHandle,
    };
    use std::num::NonZeroIsize;

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let hwnd_isize = self.hwnd.as_ptr() as isize;
            let nz = NonZeroIsize::new(hwnd_isize)
                .ok_or(HandleError::Unavailable)?;
            let handle = Win32WindowHandle::new(nz);
            Ok(unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(handle))
            })
        }
    }

    impl HasDisplayHandle for ReaperParentWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Ok(unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Windows(
                    WindowsDisplayHandle::new(),
                ))
            })
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{HandleError, ReaperParentWindow};
    use c_str_macro::c_str;
    use raw_window_handle::{
        AppKitWindowHandle, DisplayHandle, HasDisplayHandle, HasWindowHandle,
        AppKitDisplayHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
    };
    use rea_rs_low::Swell;
    use std::ptr::NonNull;

    impl HasWindowHandle for ReaperParentWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let swell = Swell::get();
            // SWELL_GetOSWindow with type "NSView" returns the backing NSView*.
            let ns_view = unsafe {
                swell.SWELL_GetOSWindow(
                    self.hwnd.as_ptr(),
                    c_str!("NSView").as_ptr(),
                )
            };
            let ptr = NonNull::new(ns_view).ok_or(HandleError::Unavailable)?;
            let handle = AppKitWindowHandle::new(ptr);
            Ok(unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle))
            })
        }
    }

    impl HasDisplayHandle for ReaperParentWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Ok(unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::AppKit(
                    AppKitDisplayHandle::new(),
                ))
            })
        }
    }
}
