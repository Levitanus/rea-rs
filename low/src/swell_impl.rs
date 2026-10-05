#![allow(clippy::many_single_char_names)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables)]
use crate::{
    bindings::root, register_plugin_destroy_hook, PluginContext, Swell,
    SwellFunctionPointers,
};

// This is safe (see https://doc.rust-lang.org/std/sync/struct.Once.html#examples-1).
static mut INSTANCE: Option<Swell> = None;

/// This impl block contains functions which exist in SWELL as macros and
/// therefore are not picked up by `bindgen`.
impl Swell {
    /// Creates a hidden child window with the supplied procedure.
    ///
    /// This is intentionally lower-level than `create_window`: the caller
    /// owns the child lifetime through its parent and may use the window as a
    /// structural container without registering it as a dialog control.
    pub unsafe fn create_child_window(
        &self,
        parent: root::HWND,
        width: i32,
        height: i32,
        proc_: root::DLGPROC,
        param: root::LPARAM,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            let hwnd = self.SWELL_CreateDialog(
                std::ptr::null_mut(),
                crate::raw::SWELL_CREATE_DIALOG_MAGIC as *const _,
                parent,
                proc_,
                param,
            );
            if hwnd.is_null() {
                return None;
            }
            self.SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width.max(1),
                height.max(1),
                (crate::raw::SWP_NOMOVE | crate::raw::SWP_NOZORDER) as i32,
            );
            Some(hwnd)
        }
        #[cfg(target_family = "windows")]
        {
            use std::iter::once;
            use winapi::um::winuser;
            let class = windows_class_name();
            let hwnd = winuser::CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                winuser::WS_CHILD | winuser::WS_CLIPCHILDREN,
                0,
                0,
                width.max(1),
                height.max(1),
                parent as _,
                std::ptr::null_mut(),
                winuser::GetModuleHandleW(std::ptr::null()),
                param as _,
            );
            if hwnd.is_null() {
                return None;
            }
            if let Some(real_proc) = proc_ {
                winuser::SetWindowLongPtrW(
                    hwnd,
                    winuser::GWLP_WNDPROC,
                    std::mem::transmute(real_proc),
                );
            }
            Some(hwnd as _)
        }
    }

    /// Creates a child button using the native SWELL/Win32 control API.
    ///
    /// The returned handle belongs to `parent`; the caller remains responsible
    /// for retaining the logical control ID and for destroying the parent.
    pub unsafe fn create_button(
        &self,
        parent: root::HWND,
        id: i32,
        label: *const ::std::os::raw::c_char,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd =
                self.SWELL_MakeButton(0, label, id, x, y, width, height, 0);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                label,
                "BUTTON",
                winapi::um::winuser::BS_PUSHBUTTON,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child edit control using the native SWELL/Win32 control API.
    pub unsafe fn create_edit_field(
        &self,
        parent: root::HWND,
        id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd =
                self.SWELL_MakeEditField(id, x, y, width, height, flags);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                std::ptr::null(),
                "EDIT",
                winapi::um::winuser::ES_LEFT | flags as u32,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child static label using the native SWELL/Win32 control API.
    pub unsafe fn create_label(
        &self,
        parent: root::HWND,
        id: i32,
        label: *const ::std::os::raw::c_char,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd =
                self.SWELL_MakeLabel(0, label, id, x, y, width, height, 0);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                label,
                "STATIC",
                winapi::um::winuser::SS_LEFT,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child check box using the native SWELL/Win32 control API.
    pub unsafe fn create_checkbox(
        &self,
        parent: root::HWND,
        id: i32,
        label: *const ::std::os::raw::c_char,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd =
                self.SWELL_MakeCheckBox(label, id, x, y, width, height, 0);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                label,
                "BUTTON",
                winapi::um::winuser::BS_AUTOCHECKBOX,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child group box using the native SWELL/Win32 control API.
    pub unsafe fn create_group_box(
        &self,
        parent: root::HWND,
        id: i32,
        label: *const ::std::os::raw::c_char,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd =
                self.SWELL_MakeGroupBox(label, id, x, y, width, height, 0);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                label,
                "BUTTON",
                winapi::um::winuser::BS_GROUPBOX,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child combo box using the native SWELL/Win32 control API.
    pub unsafe fn create_combo_box(
        &self,
        parent: root::HWND,
        id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd = self.SWELL_MakeCombo(id, x, y, width, height, flags);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                std::ptr::null(),
                "COMBOBOX",
                winapi::um::winuser::CBS_DROPDOWNLIST | flags as u32,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child list box using the native SWELL/Win32 control API.
    pub unsafe fn create_list_box(
        &self,
        parent: root::HWND,
        id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        styles: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_MakeSetCurParms(
                1.0, 1.0, 0.0, 0.0, parent, false, false,
            );
            let hwnd = self.SWELL_MakeListBox(id, x, y, width, height, styles);
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                std::ptr::null(),
                "LISTBOX",
                styles as u32,
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Creates a child from one of the built-in SWELL/Win32 control classes.
    /// The class name is passed through SWELL_MakeControl on Unix and to
    /// CreateWindowExW on Windows, keeping the platform factory choice here.
    pub unsafe fn create_native_control(
        &self,
        parent: root::HWND,
        id: i32,
        class_name: &str,
        text: *const ::std::os::raw::c_char,
        style: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            use std::ffi::CString;
            let class_name = CString::new(class_name).ok()?;
            let make_set_cur_parms = root::swell_functions::SWELL_MakeSetCurParms?;
            let make_control = root::swell_functions::SWELL_MakeControl?;
            make_set_cur_parms(1.0, 1.0, 0.0, 0.0, parent, false, false);
            let hwnd = make_control(
                text,
                id,
                class_name.as_ptr(),
                style,
                x,
                y,
                width,
                height,
                0,
            );
            return (!hwnd.is_null()).then_some(hwnd);
        }
        #[cfg(target_family = "windows")]
        {
            self.create_windows_control(
                parent,
                id,
                text,
                class_name,
                winapi::um::winuser::WS_CHILD
                    | winapi::um::winuser::WS_VISIBLE
                    | style as u32,
                x,
                y,
                width,
                height,
            )
        }
    }

    #[cfg(target_family = "windows")]
    unsafe fn create_windows_control(
        &self,
        parent: root::HWND,
        id: i32,
        text: *const ::std::os::raw::c_char,
        class_name: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Option<root::HWND> {
        use std::iter::once;
        use winapi::um::winuser;
        let class: Vec<u16> =
            class_name.encode_utf16().chain(once(0)).collect();
        let text = if text.is_null() {
            Vec::new()
        } else {
            std::ffi::CStr::from_ptr(text)
                .to_string_lossy()
                .encode_utf16()
                .chain(once(0))
                .collect()
        };
        let hwnd = winuser::CreateWindowExW(
            0,
            class.as_ptr(),
            if text.is_empty() {
                std::ptr::null()
            } else {
                text.as_ptr()
            },
            winuser::WS_CHILD | winuser::WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            parent as _,
            id as isize as _,
            winuser::GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        );
        (!hwnd.is_null()).then_some(hwnd as _)
    }

    /// Paints a standard REAPER/SWELL window background for a paint region.
    pub unsafe fn paint_window_background(
        &self,
        hdc: root::HDC,
        rect: *const root::RECT,
    ) {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_FillDialogBackground(hdc, rect, 0);
        }
        #[cfg(target_family = "windows")]
        {
            winapi::um::winuser::FillRect(
                hdc as _,
                rect as _,
                winapi::um::winuser::GetSysColorBrush(
                    winapi::um::winuser::COLOR_BTNFACE,
                ),
            );
        }
    }

    /// Makes the given instance available globally.
    ///
    /// After this has been called, the instance can be queried globally using
    /// `get()`.
    ///
    /// This can be called once only. Subsequent calls won't have any effect!
    pub fn make_available_globally(functions: Swell) {
        static INIT_INSTANCE: std::sync::Once = std::sync::Once::new();
        unsafe {
            INIT_INSTANCE.call_once(|| {
                INSTANCE = Some(functions);
                register_plugin_destroy_hook(|| INSTANCE = None);
            });
        }
    }

    /// Gives access to the instance which you made available globally before.
    ///
    /// # Panics
    ///
    /// This panics if [`make_available_globally()`] has not been called
    /// before.
    ///
    /// [`make_available_globally()`]: fn.make_available_globally.html
    pub fn get() -> &'static Swell {
        unsafe {
            INSTANCE.as_ref().expect(
                "call `make_available_globally()` before using `get()`",
            )
        }
    }

    /// Gives access to the SWELL function pointers.
    pub fn pointers(&self) -> &SwellFunctionPointers {
        &self.pointers
    }

    /// Returns the plug-in context.
    pub fn plugin_context(&self) -> &PluginContext {
        self.plugin_context
            .as_ref()
            .expect("plug-in context not available on demo instances")
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn CreateDialogParam(
        &self,
        hinst: root::HINSTANCE,
        resid: *const ::std::os::raw::c_char,
        par: root::HWND,
        dlgproc: root::DLGPROC,
        param: root::LPARAM,
    ) -> root::HWND {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_CreateDialog(
                root::SWELL_curmodule_dialogresource_head,
                resid,
                par,
                dlgproc,
                param,
            )
        }
        #[cfg(target_family = "windows")]
        #[allow(clippy::cast_ptr_alignment)]
        {
            // TODO-low winapi-rs is expecting the dlgproc function pointer to
            // be `extern "system"`.  What we have is `extern "C"`.
            // This caught cause issues on Windows i686 (32-bit)
            //  builds. However, in practice it didn't show any issues (tested
            // with ReaLearn). So  probably not that  important.
            winapi::um::winuser::CreateDialogParamW(
                hinst as _,
                resid as _,
                par as _,
                std::mem::transmute(dlgproc),
                param,
            ) as _
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn LoadMenu(
        &self,
        hinst: root::HINSTANCE,
        resid: *const ::std::os::raw::c_char,
    ) -> root::HMENU {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_LoadMenu(root::SWELL_curmodule_menuresource_head, resid)
        }
        #[cfg(target_family = "windows")]
        #[allow(clippy::cast_ptr_alignment)]
        {
            winapi::um::winuser::LoadMenuW(hinst as _, resid as _) as _
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn FillRect(
        &self,
        ctx: root::HDC,
        r: *const root::RECT,
        br: root::HBRUSH,
    ) {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_FillRect(ctx, r, br);
        }
        #[cfg(target_family = "windows")]
        #[allow(clippy::cast_ptr_alignment)]
        {
            winapi::um::winuser::FillRect(ctx as _, r as _, br as _);
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn DrawText(
        &self,
        ctx: root::HDC,
        buf: *const ::std::os::raw::c_char,
        len: ::std::os::raw::c_int,
        r: *mut root::RECT,
        align: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        #[cfg(target_family = "unix")]
        {
            self.SWELL_DrawText(ctx, buf, len, r, align)
        }
        #[cfg(target_family = "windows")]
        #[allow(clippy::cast_ptr_alignment)]
        {
            let utf16_string = utf8_to_16(buf);
            let result = winapi::um::winuser::DrawTextW(
                ctx as _,
                utf16_string.as_ptr(),
                len,
                r as _,
                align as _,
            );
            std::mem::drop(utf16_string);
            result
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn SetWindowText(
        &self,
        hwnd: root::HWND,
        text: *const ::std::os::raw::c_char,
    ) -> root::BOOL {
        #[cfg(target_family = "unix")]
        {
            self.SetDlgItemText(hwnd, 0, text)
        }
        #[cfg(target_family = "windows")]
        {
            let utf16_string = utf8_to_16(text);
            let result = winapi::um::winuser::SetWindowTextW(
                hwnd as _,
                utf16_string.as_ptr(),
            );
            std::mem::drop(utf16_string);
            result as _
        }
    }

    /// Attention: Whereas the Windows original returns a length, this just
    /// returns success.
    ///
    /// In order to avoid surprises, on Windows it will behave like this, too.
    ///
    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn GetWindowText(
        &self,
        hwnd: root::HWND,
        lpString: root::LPSTR,
        nMaxCount: std::os::raw::c_int,
    ) -> root::BOOL {
        #[cfg(target_family = "unix")]
        {
            self.GetDlgItemText(hwnd, 0, lpString, nMaxCount)
        }
        #[cfg(target_family = "windows")]
        {
            let len =
                with_utf16_to_8(lpString, nMaxCount, |buffer, max_size| {
                    winapi::um::winuser::GetWindowTextW(
                        hwnd as _, buffer, max_size,
                    ) as _
                });
            // Just return whether successful in order to conform to SWELL.
            if len == 0 {
                0
            } else {
                1
            }
        }
    }

    pub fn RGB(r: u8, g: u8, b: u8) -> root::DWORD {
        #[cfg(target_family = "unix")]
        {
            // SWELL says: "the byte ordering of RGB() etc is different than on
            // win32"
            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
        }
        #[cfg(target_family = "windows")]
        {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
    }

    pub fn GetRValue(color: root::DWORD) -> u8 {
        #[cfg(target_family = "unix")]
        {
            ((color >> 16) & 0xff) as _
        }
        #[cfg(target_family = "windows")]
        {
            (color & 0xff) as _
        }
    }

    pub fn GetGValue(color: root::DWORD) -> u8 {
        ((color >> 8) & 0xff) as _
    }

    pub fn GetBValue(color: root::DWORD) -> u8 {
        #[cfg(target_family = "unix")]
        {
            (color & 0xff) as _
        }
        #[cfg(target_family = "windows")]
        {
            ((color >> 16) & 0xff) as _
        }
    }
}

/// This impl block contains functions which delegate to native win32 functions
/// but don't have exactly the same signature or need some character encoding
/// conversion.
///
/// SWELL uses UTF-8-encoded strings as byte arrays (`*const i8`), exactly like
/// REAPER itself. Windows uses UTF-16-encoded strings as u16 arrays (`*const
/// u16`). It's very convenient that we can use UTF-8 throughout: Rust, REAPER,
/// SWELL ... just Windows was missing.
#[cfg(target_family = "windows")]
impl Swell {
    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn UpdateWindow(&self, hwnd: root::HWND) {
        winapi::um::winuser::UpdateWindow(hwnd as _);
    }

    /// Windows counterpart of SWELL's `GetWindowLong`.
    pub unsafe fn GetWindowLong(
        &self,
        hwnd: root::HWND,
        index: ::std::os::raw::c_int,
    ) -> root::LONG_PTR {
        winapi::um::winuser::GetWindowLongPtrW(hwnd as _, index)
    }

    /// Windows counterpart of SWELL's `SetWindowLong`.
    pub unsafe fn SetWindowLong(
        &self,
        hwnd: root::HWND,
        index: ::std::os::raw::c_int,
        value: root::LONG_PTR,
    ) -> root::LONG_PTR {
        winapi::um::winuser::SetWindowLongPtrW(hwnd as _, index, value)
    }

    /// Windows counterpart of SWELL's `DefWindowProc`.
    pub unsafe fn DefWindowProc(
        &self,
        hwnd: root::HWND,
        msg: root::UINT,
        w_param: root::WPARAM,
        l_param: root::LPARAM,
    ) -> root::LRESULT {
        winapi::um::winuser::DefWindowProcW(hwnd as _, msg, w_param, l_param)
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn SendMessage(
        &self,
        hwnd: root::HWND,
        msg: root::UINT,
        wParam: root::WPARAM,
        lParam: root::LPARAM,
    ) -> root::LRESULT {
        if lParam != 0 && lparam_is_string(msg) {
            let utf16_string = utf8_to_16(lParam as _);
            let result = winapi::um::winuser::SendMessageW(
                hwnd as _,
                msg,
                wParam,
                utf16_string.as_ptr() as _,
            );
            std::mem::drop(utf16_string);
            result
        } else {
            winapi::um::winuser::SendMessageW(hwnd as _, msg, wParam, lParam)
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn PostMessage(
        &self,
        hwnd: root::HWND,
        msg: root::UINT,
        wParam: root::WPARAM,
        lParam: root::LPARAM,
    ) -> root::BOOL {
        if lParam != 0 && lparam_is_string(msg) {
            let utf16_string = utf8_to_16(lParam as _);
            let result = winapi::um::winuser::PostMessageW(
                hwnd as _,
                msg,
                wParam,
                utf16_string.as_ptr() as _,
            );
            std::mem::drop(utf16_string);
            result as _
        } else {
            winapi::um::winuser::PostMessageW(hwnd as _, msg, wParam, lParam)
                as _
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn MessageBox(
        &self,
        hwndParent: root::HWND,
        text: *const ::std::os::raw::c_char,
        caption: *const ::std::os::raw::c_char,
        type_: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int {
        let text_utf16 = utf8_to_16(text);
        let caption_utf16 = utf8_to_16(caption);
        let result = winapi::um::winuser::MessageBoxW(
            hwndParent as _,
            text_utf16.as_ptr() as _,
            caption_utf16.as_ptr() as _,
            type_ as _,
        );
        std::mem::drop(text_utf16);
        result as _
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn SetMenuItemInfo(
        &self,
        hMenu: root::HMENU,
        pos: ::std::os::raw::c_int,
        byPos: root::BOOL,
        mi: *mut root::MENUITEMINFO,
    ) -> root::BOOL {
        let mi = *mi;
        let mut utf16_mi = utf8_to_16_menu_item_info(&mi);
        if menu_item_needs_string_conversion(mi) {
            // Sets text. Must convert it.
            let mut utf16_string = utf8_to_16(mi.dwTypeData);
            utf16_mi.dwTypeData = utf16_string.as_mut_ptr();
            let result = winapi::um::winuser::SetMenuItemInfoW(
                hMenu as _,
                pos as _,
                byPos as _,
                &utf16_mi as *const _,
            );
            std::mem::drop(utf16_string);
            result as _
        } else {
            // Doesn't set text. No conversion necessary.
            let result = winapi::um::winuser::SetMenuItemInfoW(
                hMenu as _,
                pos as _,
                byPos as _,
                &utf16_mi as *const _,
            );
            result as _
        }
    }

    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn InsertMenuItem(
        &self,
        hMenu: root::HMENU,
        pos: ::std::os::raw::c_int,
        byPos: root::BOOL,
        mi: *mut root::MENUITEMINFO,
    ) {
        let mi = *mi;
        let mut utf16_mi = utf8_to_16_menu_item_info(&mi);
        if menu_item_needs_string_conversion(mi) {
            // Sets text. Must convert it.
            let mut utf16_string = utf8_to_16(mi.dwTypeData);
            utf16_mi.dwTypeData = utf16_string.as_mut_ptr();
            let result = winapi::um::winuser::InsertMenuItemW(
                hMenu as _,
                pos as _,
                byPos as _,
                &utf16_mi as *const _,
            );
            std::mem::drop(utf16_string);
        } else {
            // Doesn't set text. No conversion necessary.
            let result = winapi::um::winuser::InsertMenuItemW(
                hMenu as _,
                pos as _,
                byPos as _,
                &utf16_mi as *const _,
            );
        }
    }

    /// **Attention:** This doesn't yet support `dwTypeData` (always `null`
    /// currently).
    ///
    /// # Safety
    ///
    /// REAPER can crash if you pass an invalid pointer.
    pub unsafe fn GetMenuItemInfo(
        &self,
        hMenu: root::HMENU,
        pos: ::std::os::raw::c_int,
        byPos: root::BOOL,
        mi: *mut root::MENUITEMINFO,
    ) -> root::BOOL {
        let mut mi = *mi;
        if !mi.dwTypeData.is_null() {
            todo!("Getting string information from menu item is not yet implemented.")
        }
        let mut utf16_mi = utf8_to_16_menu_item_info(&mi);
        let result = winapi::um::winuser::GetMenuItemInfoW(
            hMenu as _,
            pos as _,
            byPos as _,
            &mut utf16_mi as _,
        );
        mi.cbSize = utf16_mi.cbSize;
        mi.fMask = utf16_mi.fMask;
        mi.fType = utf16_mi.fType;
        mi.fState = utf16_mi.fState;
        mi.wID = utf16_mi.wID;
        mi.hSubMenu = utf16_mi.hSubMenu as _;
        mi.hbmpChecked = utf16_mi.hbmpChecked as _;
        mi.hbmpUnchecked = utf16_mi.hbmpUnchecked as _;
        mi.dwItemData = utf16_mi.dwItemData;
        mi.dwTypeData = std::ptr::null_mut();
        mi.cch = utf16_mi.cch as _;
        mi.hbmpItem = utf16_mi.hbmpItem as _;
        result as _
    }

    /// On Windows this is a constant but in SWELL this is a macro which
    /// translates to a function call.
    pub fn CF_TEXT(&self) -> root::UINT {
        #[cfg(target_family = "unix")]
        {
            unsafe {
                self.RegisterClipboardFormat(
                    c_str_macro::c_str!("SWELL__CF_TEXT").as_ptr(),
                )
            }
        }
        #[cfg(target_family = "windows")]
        1
    }
}

impl std::fmt::Debug for SwellFunctionPointers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwellFunctionPointers")
            .field("loaded_count", &self.loaded_count)
            .field("total_count", &Self::TOTAL_COUNT)
            .finish()
    }
}

/// Converts the given UTF-8 C-style string (nul terminator) to an UTF-16
/// C-style string.
///
/// # Safety
///
/// You must ensure that the given string points to an UTF-8 encoded C-style
/// string.
#[cfg(target_family = "windows")]
pub(crate) unsafe fn utf8_to_16(
    raw_utf8: *const std::os::raw::c_char,
) -> Vec<u16> {
    use std::ffi::{CStr, OsStr};
    use std::iter::once;
    // Assumes that the given pointer points to a C-style string.
    let utf8_c_str = CStr::from_ptr(raw_utf8);
    // Interpret that string as UTF-8-encoded. Fall back to replacement
    // characters if not.
    let str = utf8_c_str.to_string_lossy();
    // Now reencode it as UTF-16.
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(str.as_ref())
        .encode_wide()
        .chain(once(0))
        .collect()
}

/// Creates a UTF-16 buffer (to be filled by the given function) and writes it
/// as UTF-8 to the given target buffer.
///
/// `max_size` must include nul terminator. The given function must return the
/// actual string length *without* nul terminator.
#[cfg(target_family = "windows")]
pub(crate) unsafe fn with_utf16_to_8(
    utf8_target_buffer: *mut std::os::raw::c_char,
    requested_max_size: std::os::raw::c_int,
    fill_utf16_source_buffer: impl FnOnce(*mut u16, std::os::raw::c_int) -> usize,
) -> usize {
    let mut utf16_vec: Vec<u16> =
        Vec::with_capacity(requested_max_size as usize);
    // Returns length *without* nul terminator.
    let len =
        fill_utf16_source_buffer(utf16_vec.as_mut_ptr(), requested_max_size);
    if len == 0 {
        return 0;
    }
    utf16_vec.set_len(len);
    // nul terminator will not be part of the string because len doesn't
    // include it!
    let string = String::from_utf16_lossy(&utf16_vec);
    let c_string = match std::ffi::CString::new(string) {
        Ok(s) => s,
        Err(_) => {
            // String contained 0 byte. This would end a C-style string
            // abruptly.
            return 0;
        }
    };
    let source_bytes = c_string.as_bytes_with_nul();
    let target_bytes = std::slice::from_raw_parts_mut(
        utf8_target_buffer,
        requested_max_size as usize,
    );
    let source_bytes_signed = &*(source_bytes as *const [u8] as *const [i8]);
    target_bytes[..source_bytes.len()].copy_from_slice(source_bytes_signed);
    len
}

/// For all messages which contain a string payload, convert the string's
/// encoding.
#[cfg(target_family = "windows")]
fn lparam_is_string(msg: root::UINT) -> bool {
    use crate::raw;
    // There are probably more than just those two. Add as soon as needed.
    matches!(msg, raw::CB_INSERTSTRING | raw::CB_ADDSTRING)
}

/// cbSize doesn't matter.
/// Converts everything except `dwTypeData` (needs special treatment).
#[cfg(target_family = "windows")]
fn utf8_to_16_menu_item_info(
    mi: &root::MENUITEMINFO,
) -> winapi::um::winuser::MENUITEMINFOW {
    winapi::um::winuser::MENUITEMINFOW {
        cbSize: std::mem::size_of::<winapi::um::winuser::MENUITEMINFOW>() as _,
        fMask: mi.fMask,
        fType: mi.fType,
        fState: mi.fState,
        wID: mi.wID,
        hSubMenu: mi.hSubMenu as _,
        hbmpChecked: mi.hbmpChecked as _,
        hbmpUnchecked: mi.hbmpUnchecked as _,
        dwItemData: mi.dwItemData,
        dwTypeData: std::ptr::null_mut(),
        cch: mi.cch as _,
        hbmpItem: mi.hbmpItem as _,
    }
}

#[cfg(target_family = "windows")]
fn menu_item_needs_string_conversion(mi: root::MENUITEMINFO) -> bool {
    // Super important to use `raw` constants here because the SWELL constant
    // values deviate from the Windows constants!!!
    use crate::raw;
    (mi.fMask & raw::MIIM_TYPE) != 0 && (mi.fMask & raw::MIIM_DATA) != 0
}

/// Creates a top-level window (owned by the given parent) without requiring a
/// dialog template and returns its handle, or `None` if creation failed or the
/// window was destroyed during creation.
///
/// On Unix (SWELL), this uses `SWELL_CreateDialog` with the magic resource ID
/// `0x400000 | flags` (see `swell-dlg-generic.cpp` and `swell-dlg.mm`). The
/// given `proc` is installed as window procedure and receives `WM_CREATE`
/// synchronously (with `param` as `lParam`). Any nonzero flag bit forces the
/// window to be top-level (owned by `parent`) instead of a child window.
///
/// On Windows, this uses one-time `RegisterClassExW` plus `CreateWindowExW`.
/// The given `proc` is used as window procedure as well. `WM_CREATE` receives
/// a `CREATESTRUCTW` pointer as `lParam` (the `param` value is available via
/// `lpCreateParams`).
///
/// The window is created hidden. Show it via `ShowWindow` afterwards.
///
/// # Arguments
///
/// * `parent` - Window which will own the new window (typically REAPER's main
///   window).
/// * `title` - UTF-8 encoded title of the window.
/// * `width` - Requested client area width in pixels.
/// * `height` - Requested client area height in pixels.
/// * `resizable` - Whether the window has a resizable border.
/// * `hinstance` - Module handle of the plugin (used on Windows for class
///   registration; ignored on Unix).
/// * `proc` - Window/dialog procedure which receives all window messages.
/// * `param` - Value passed to `WM_CREATE` (Unix: as `lParam`; Windows: via
///   `CREATESTRUCTW.lpCreateParams`).
///
/// # Safety
///
/// `parent` must be a valid window handle and `proc` must be a valid function
/// pointer. Both are usually obtained from REAPER.
impl Swell {
    pub unsafe fn create_window(
        &self,
        parent: root::HWND,
        title: &str,
        width: i32,
        height: i32,
        resizable: bool,
        no_minimize: bool,
        no_close: bool,
        hinstance: root::HINSTANCE,
        proc_: root::DLGPROC,
        param: root::LPARAM,
    ) -> Option<root::HWND> {
        #[cfg(target_family = "unix")]
        {
            // Any nonzero flag bit forces a top-level (owned) window instead
            // of a child window. Bit 0 additionally makes it resizable. We
            // always set bit 1 ("no minimize") as the top-level forcing bit
            // and rely on post-creation style adjustment for the resizable
            // case:
            //
            // - Linux/GDK: bit 0 => WS_THICKFRAME|WS_CAPTION (close/min/max +
            //   resize), otherwise WS_CAPTION (title + minimize, fixed size).
            //   Bits 1 and 2 are ignored.
            // - macOS: bit 0 => +NSResizableWindowMask, bit 1 =>
            //   -Miniaturizable, bit 2 => -Closable.
            //
            // Because the desired style differs per platform and SWELL applies
            // styles only at creation time, we create with bit 0 set
            // (resizable) and, for non-resizable windows, strip WS_THICKFRAME
            // afterwards via SetWindowLong(GWL_STYLE). On Linux this updates
            // the GDK decorations (swell_oswindow_update_style), on macOS it
            // recreates the window frame with the right style mask.
            let mut flags = crate::raw::SWELL_DLG_FORCE_RESIZABLE;
            if no_minimize {
                flags |= crate::raw::SWELL_DLG_NO_MINIMIZE;
            }
            if no_close {
                flags |= crate::raw::SWELL_DLG_NO_CLOSE;
            }
            let resid =
                (crate::raw::SWELL_CREATE_DIALOG_MAGIC | flags) as usize;
            let hwnd = self.SWELL_CreateDialog(
                std::ptr::null_mut(),
                resid as *const ::std::os::raw::c_char,
                parent,
                proc_,
                param,
            );
            if hwnd.is_null() {
                return None;
            }
            // Initialize the backing style before the window is shown. This
            // avoids exposing an uninitialized/garbled native surface on the
            // first paint, especially on GDK-backed SWELL windows.
            let mut style = self.GetWindowLong(hwnd, crate::raw::GWL_STYLE);
            style |= crate::raw::WS_CAPTION as isize;
            if resizable {
                style |= crate::raw::WS_THICKFRAME as isize;
            } else {
                style &= !(crate::raw::WS_THICKFRAME as isize);
            }
            // Force SWELL to rebuild the native frame. Some backends only
            // update decorations when the caption bit changes; merely
            // writing the final style can therefore leave a borderless,
            // non-resizable native surface.
            self.SetWindowLong(
                hwnd,
                crate::raw::GWL_STYLE,
                style & !(crate::raw::WS_CAPTION as isize),
            );
            self.SetWindowLong(hwnd, crate::raw::GWL_STYLE, style);
            // Title must be set after creation. SWELL's template-less mode
            // doesn't take a title.
            let title_c = std::ffi::CString::new(title).ok()?;
            self.SetWindowText(hwnd, title_c.as_ptr());
            if !resizable {
                // Strip the resizable border. Keep WS_CAPTION.
                let style = self.GetWindowLong(hwnd, crate::raw::GWL_STYLE);
                self.SetWindowLong(
                    hwnd,
                    crate::raw::GWL_STYLE,
                    style & !(crate::raw::WS_THICKFRAME as isize),
                );
            }
            // Apply the requested size. SWELL's template-less mode starts with
            // a default size (300x200 UI-scaled on Linux, 10x10 on macOS).
            self.SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width,
                height,
                (crate::raw::SWP_NOMOVE | crate::raw::SWP_NOZORDER) as i32,
            );
            Some(hwnd)
        }
        #[cfg(target_family = "windows")]
        {
            use winapi::um::winuser;
            // Register the window class once per process. The class stores a
            // fallback procedure; the real procedure is installed via
            // SetWindowLongPtrW(GWLP_WNDPROC) after creation because WM_CREATE
            // is delivered during CreateWindowExW and must not crash if the
            // real procedure is not yet reachable.
            static REGISTER_CLASS: std::sync::Once = std::sync::Once::new();
            static CLASS_REGISTERED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            REGISTER_CLASS.call_once(|| {
                let class_name = windows_class_name();
                let wc = winuser::WNDCLASSEXW {
                    cbSize: std::mem::size_of::<winuser::WNDCLASSEXW>() as u32,
                    style: winuser::CS_HREDRAW | winuser::CS_VREDRAW,
                    lpfnWndProc: Some(windows_default_wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance as _,
                    hIcon: std::ptr::null_mut(),
                    hCursor: winuser::LoadCursorW(
                        std::ptr::null_mut(),
                        winuser::IDC_ARROW,
                    ),
                    hbrBackground: winuser::GetSysColorBrush(
                        winuser::COLOR_BTNFACE,
                    ),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: std::ptr::null_mut(),
                };
                let atom = winuser::RegisterClassExW(&wc);
                CLASS_REGISTERED
                    .store(atom != 0, std::sync::atomic::Ordering::Release);
            });
            if !CLASS_REGISTERED.load(std::sync::atomic::Ordering::Acquire) {
                return None;
            }
            let style = if resizable {
                winuser::WS_OVERLAPPEDWINDOW
            } else {
                winuser::WS_OVERLAPPEDWINDOW
                    & !(winuser::WS_THICKFRAME | winuser::WS_MAXIMIZEBOX)
            };
            let title_utf16 = windows_title_utf16(title);
            let hwnd = winuser::CreateWindowExW(
                0,
                windows_class_name().as_ptr(),
                title_utf16.as_ptr(),
                style,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                width,
                height,
                parent as _,
                std::ptr::null_mut(),
                hinstance as _,
                param as _,
            );
            if hwnd.is_null() {
                return None;
            }
            // Install the actual window procedure. WM_CREATE has already been
            // delivered to the fallback procedure above.
            if let Some(real_proc) = proc_ {
                winuser::SetWindowLongPtrW(
                    hwnd as _,
                    winuser::GWLP_WNDPROC,
                    std::mem::transmute(real_proc),
                );
            }
            Some(hwnd as _)
        }
    }
}

/// Fallback window procedure used on Windows until the real procedure is
/// installed after `CreateWindowExW` returns.
#[cfg(target_family = "windows")]
unsafe extern "system" fn windows_default_wndproc(
    hwnd: winapi::shared::windef::HWND,
    msg: winapi::shared::minwindef::UINT,
    w_param: winapi::shared::minwindef::WPARAM,
    l_param: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    winapi::um::winuser::DefWindowProcW(hwnd, msg, w_param, l_param)
}

#[cfg(target_family = "windows")]
fn windows_class_name() -> std::vec::Vec<u16> {
    use std::iter::once;
    "ReaRsWindow".encode_utf16().chain(once(0)).collect()
}

#[cfg(target_family = "windows")]
fn windows_title_utf16(title: &str) -> std::vec::Vec<u16> {
    use std::iter::once;
    title.encode_utf16().chain(once(0)).collect()
}
