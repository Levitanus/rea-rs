//! GUI support for Reaper extensions using egui-baseview.
//!
//! Enable with cargo feature `egui-baseview`.
//!
//! This module provides:
//! - `ReaperParentWindow`: a parent-window adapter that can be passed to
//!   `egui_baseview::EguiWindow::open_parented()`.
//! - Re-exports of `baseview`, `egui_baseview`, and `egui`.

pub use baseview;
pub use egui_baseview;
pub use egui_baseview::egui;

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
        HasRawWindowHandle, RawWindowHandle, XlibWindowHandle,
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

    unsafe impl HasRawWindowHandle for ReaperParentWindow {
        fn raw_window_handle(&self) -> RawWindowHandle {
            let mut handle = XlibWindowHandle::empty();
            if let Some(xid) =
                unsafe { xid_from_swell_hwnd(self.hwnd.as_ptr()) }
            {
                handle.window = xid;
            }
            RawWindowHandle::Xlib(handle)
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::ReaperParentWindow;
    use raw_window_handle::{
        HasRawWindowHandle, RawWindowHandle, Win32WindowHandle,
    };

    unsafe impl HasRawWindowHandle for ReaperParentWindow {
        fn raw_window_handle(&self) -> RawWindowHandle {
            let mut handle = Win32WindowHandle::empty();
            handle.hwnd = self.hwnd.as_ptr().cast();
            RawWindowHandle::Win32(handle)
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::ReaperParentWindow;
    use c_str_macro::c_str;
    use raw_window_handle::{
        AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle,
    };
    use rea_rs_low::Swell;

    unsafe impl HasRawWindowHandle for ReaperParentWindow {
        fn raw_window_handle(&self) -> RawWindowHandle {
            let mut handle = AppKitWindowHandle::empty();
            let ns_view = unsafe {
                Swell::get().SWELL_GetOSWindow(
                    self.hwnd.as_ptr(),
                    c_str!("NSView").as_ptr(),
                )
            };
            handle.ns_view = ns_view;
            RawWindowHandle::AppKit(handle)
        }
    }
}
