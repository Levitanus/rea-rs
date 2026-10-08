use std::{ffi::CString, marker::PhantomData, ptr::NonNull};

use crate::raw::LICE_IBitmap;

#[repr(C)]
struct HostOpaque {
    _private: [u8; 0],
}
#[repr(C)]
struct ControlOpaque {
    _private: [u8; 0],
}

type CommandCallback = Option<
    unsafe extern "C" fn(
        context: *mut std::ffi::c_void,
        command: i32,
        p1: isize,
        p2: isize,
        source_id: i32,
    ),
>;

extern "C" {
    fn rea_wdl_host_create(
        callback: CommandCallback,
        context: *mut std::ffi::c_void,
    ) -> *mut HostOpaque;
    fn rea_wdl_host_destroy(host: *mut HostOpaque);
    fn rea_wdl_host_set_real_parent(
        host: *mut HostOpaque,
        hwnd: *mut std::ffi::c_void,
    );
    fn rea_wdl_control_create(
        host: *mut HostOpaque,
        kind: i32,
        id: i32,
    ) -> *mut ControlOpaque;
    fn rea_wdl_control_set_rect(
        control: *mut ControlOpaque,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    );
    fn rea_wdl_control_set_visible(control: *mut ControlOpaque, visible: i32);
    fn rea_wdl_control_set_text(control: *mut ControlOpaque, text: *const i8);
    fn rea_wdl_control_set_enabled(control: *mut ControlOpaque, enabled: i32);
    fn rea_wdl_control_set_check_state(
        control: *mut ControlOpaque,
        checked: i32,
    );
    fn rea_wdl_control_set_range(
        control: *mut ControlOpaque,
        minimum: i32,
        maximum: i32,
        center: i32,
    );
    fn rea_wdl_control_set_value(control: *mut ControlOpaque, value: i32);
    fn rea_wdl_control_get_value(control: *mut ControlOpaque) -> i32;
    fn rea_wdl_control_add_item(
        control: *mut ControlOpaque,
        text: *const i8,
    ) -> i32;
    fn rea_wdl_control_set_selection(control: *mut ControlOpaque, index: i32);
    fn rea_wdl_control_get_selection(control: *mut ControlOpaque) -> i32;
    fn rea_wdl_control_id(control: *mut ControlOpaque) -> i32;
    fn rea_wdl_host_mouse_down(host: *mut HostOpaque, x: i32, y: i32) -> i32;
    fn rea_wdl_host_mouse_move(host: *mut HostOpaque, x: i32, y: i32);
    fn rea_wdl_host_mouse_up(host: *mut HostOpaque, x: i32, y: i32);
    fn rea_wdl_host_mouse_double_click(
        host: *mut HostOpaque,
        x: i32,
        y: i32,
    ) -> i32;
    fn rea_wdl_host_mouse_wheel(
        host: *mut HostOpaque,
        x: i32,
        y: i32,
        delta: i32,
    ) -> i32;
    fn rea_wdl_host_capture_lost(host: *mut HostOpaque);
    fn rea_wdl_host_paint(
        host: *mut HostOpaque,
        bitmap: *mut LICE_IBitmap,
        width: i32,
        height: i32,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum VirtualControlKind {
    IconButton = 0,
    StaticText = 1,
    ComboBox = 2,
    Slider = 3,
    ListBox = 4,
}

pub struct VirtualControlHost {
    handle: NonNull<HostOpaque>,
    context: *mut std::ffi::c_void,
    _not_send_sync: PhantomData<*mut ()>,
}

impl VirtualControlHost {
    pub fn new(
        callback: impl FnMut(i32, isize, isize, i32) + 'static,
    ) -> Self {
        unsafe extern "C" fn trampoline(
            context: *mut std::ffi::c_void,
            command: i32,
            p1: isize,
            p2: isize,
            source_id: i32,
        ) {
            if context.is_null() {
                return;
            }
            let callback =
                &mut *(context as *mut Box<dyn FnMut(i32, isize, isize, i32)>);
            callback(command, p1, p2, source_id);
        }
        let callback: Box<Box<dyn FnMut(i32, isize, isize, i32)>> =
            Box::new(Box::new(callback));
        let context = Box::into_raw(callback) as *mut std::ffi::c_void;
        let handle = unsafe { rea_wdl_host_create(Some(trampoline), context) };
        let Some(handle) = NonNull::new(handle) else {
            unsafe {
                drop(Box::from_raw(
                    context as *mut Box<dyn FnMut(i32, isize, isize, i32)>,
                ));
            }
            panic!("could not create WDL virtual-control host");
        };
        Self {
            handle,
            context,
            _not_send_sync: PhantomData,
        }
    }

    pub fn create_control(
        &mut self,
        kind: VirtualControlKind,
        id: i32,
    ) -> Option<VirtualControl> {
        NonNull::new(unsafe {
            rea_wdl_control_create(self.handle.as_ptr(), kind as i32, id)
        })
        .map(|handle| VirtualControl { handle, kind })
    }

    pub fn set_real_parent(&mut self, hwnd: *mut std::ffi::c_void) {
        unsafe { rea_wdl_host_set_real_parent(self.handle.as_ptr(), hwnd) }
    }

    pub fn mouse_down(&mut self, x: i32, y: i32) -> bool {
        unsafe { rea_wdl_host_mouse_down(self.handle.as_ptr(), x, y) != 0 }
    }
    pub fn mouse_move(&mut self, x: i32, y: i32) {
        unsafe { rea_wdl_host_mouse_move(self.handle.as_ptr(), x, y) }
    }
    pub fn mouse_up(&mut self, x: i32, y: i32) {
        unsafe { rea_wdl_host_mouse_up(self.handle.as_ptr(), x, y) }
    }
    pub fn mouse_double_click(&mut self, x: i32, y: i32) -> bool {
        unsafe {
            rea_wdl_host_mouse_double_click(self.handle.as_ptr(), x, y) != 0
        }
    }
    pub fn mouse_wheel(&mut self, x: i32, y: i32, delta: i32) -> bool {
        unsafe {
            rea_wdl_host_mouse_wheel(self.handle.as_ptr(), x, y, delta) != 0
        }
    }
    pub fn capture_lost(&mut self) {
        unsafe { rea_wdl_host_capture_lost(self.handle.as_ptr()) }
    }

    /// Paint into the caller-owned LICE target. The bitmap must remain valid
    /// for the call.
    pub unsafe fn paint(
        &mut self,
        bitmap: *mut LICE_IBitmap,
        width: i32,
        height: i32,
        clip: [i32; 4],
    ) {
        rea_wdl_host_paint(
            self.handle.as_ptr(),
            bitmap,
            width,
            height,
            clip[0],
            clip[1],
            clip[2],
            clip[3],
        );
    }
}

impl Drop for VirtualControlHost {
    fn drop(&mut self) {
        unsafe {
            rea_wdl_host_destroy(self.handle.as_ptr());
            if !self.context.is_null() {
                drop(Box::from_raw(
                    self.context
                        as *mut Box<dyn FnMut(i32, isize, isize, i32)>,
                ));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct VirtualControl {
    handle: NonNull<ControlOpaque>,
    kind: VirtualControlKind,
}

impl VirtualControl {
    pub fn set_rect(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            rea_wdl_control_set_rect(self.handle.as_ptr(), x, y, width, height)
        }
    }
    pub fn set_visible(&self, visible: bool) {
        unsafe {
            rea_wdl_control_set_visible(self.handle.as_ptr(), visible as i32)
        }
    }
    pub fn set_enabled(&self, enabled: bool) {
        unsafe {
            rea_wdl_control_set_enabled(self.handle.as_ptr(), enabled as i32)
        }
    }
    pub fn set_text(&self, text: &str) -> Result<(), std::ffi::NulError> {
        let text = CString::new(text)?;
        unsafe {
            rea_wdl_control_set_text(self.handle.as_ptr(), text.as_ptr())
        };
        Ok(())
    }
    pub fn set_checked(&self, checked: bool) {
        unsafe {
            rea_wdl_control_set_check_state(
                self.handle.as_ptr(),
                checked as i32,
            )
        }
    }
    pub fn set_range(&self, minimum: i32, maximum: i32, center: i32) {
        unsafe {
            rea_wdl_control_set_range(
                self.handle.as_ptr(),
                minimum,
                maximum,
                center,
            )
        }
    }
    pub fn set_value(&self, value: i32) {
        unsafe { rea_wdl_control_set_value(self.handle.as_ptr(), value) }
    }
    pub fn value(&self) -> i32 {
        unsafe { rea_wdl_control_get_value(self.handle.as_ptr()) }
    }
    pub fn add_item(&self, text: &str) -> Result<i32, std::ffi::NulError> {
        let text = CString::new(text)?;
        Ok(unsafe {
            rea_wdl_control_add_item(self.handle.as_ptr(), text.as_ptr())
        })
    }
    pub fn set_selection(&self, index: i32) {
        unsafe { rea_wdl_control_set_selection(self.handle.as_ptr(), index) }
    }
    pub fn selection(&self) -> i32 {
        unsafe { rea_wdl_control_get_selection(self.handle.as_ptr()) }
    }
    pub fn kind(&self) -> VirtualControlKind {
        self.kind
    }
    pub fn control_kind(&self) -> VirtualControlKind {
        self.kind
    }
    pub fn id(&self) -> i32 {
        unsafe { rea_wdl_control_id(self.handle.as_ptr()) }
    }
}
