//! Retained SWELL/HDC and LICE drawing resources.
//!
//! Resource owners are independent of windows and creation contexts. Keep
//! them in a concrete [`WindowHandler`] and borrow a surface only while its
//! native paint transaction is active.

use super::layout::Rect;
use crate::{ReaRsError, Reaper, ReaperResult};
use rea_rs_low::raw::{self, LICE_IBitmap, LICE_IFont, LICE_pixel};
use std::{ffi::CString, marker::PhantomData, ptr::NonNull};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawTextOptions {
    pub alignment: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintInfo {
    pub damage_rect: Rect,
    pub client_rect: Rect,
}

/// Returns a zero-initialized LOGFONT suitable for filling before creation.
pub fn default_logfont() -> raw::LOGFONT {
    raw::LOGFONT::default()
}

pub struct Brush {
    handle: NonNull<raw::HGDIOBJ__>,
    swell: rea_rs_low::Swell,
}

impl Brush {
    pub fn solid(color: u32) -> ReaperResult<Self> {
        let handle = Reaper::get().swell().CreateSolidBrush(color as i32);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("solid brush"))
    }

    pub fn alpha(color: u32, alpha: f32) -> ReaperResult<Self> {
        #[cfg(target_family = "unix")]
        {
            let handle = Reaper::get()
                .swell()
                .CreateSolidBrushAlpha(color as i32, alpha.clamp(0.0, 1.0));
            NonNull::new(handle)
                .map(|handle| Self {
                    handle,
                    swell: *Reaper::get().swell(),
                })
                .ok_or(ReaRsError::NullPtr("alpha brush"))
        }
        #[cfg(target_family = "windows")]
        {
            let _ = (color, alpha);
            Err(ReaRsError::UnexpectedAPI(
                "alpha brushes are not supported by this backend".into(),
            ))
        }
    }

    fn raw(&self) -> raw::HBRUSH {
        self.handle.as_ptr() as raw::HBRUSH
    }
}

impl Drop for Brush {
    fn drop(&mut self) {
        unsafe { self.swell.DeleteObject(self.handle.as_ptr()) }
    }
}

pub struct Pen {
    handle: NonNull<raw::HGDIOBJ__>,
    swell: rea_rs_low::Swell,
}

impl Pen {
    pub fn solid(width: i32, color: u32) -> ReaperResult<Self> {
        if width < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid pen width",
            ));
        }
        let handle = Reaper::get().swell().CreatePen(0, width, color as i32);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("pen"))
    }

    pub fn alpha(width: i32, color: u32, alpha: f32) -> ReaperResult<Self> {
        if width < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid pen width",
            ));
        }
        #[cfg(target_family = "unix")]
        {
            let handle = Reaper::get().swell().CreatePenAlpha(
                0,
                width,
                color as i32,
                alpha.clamp(0.0, 1.0),
            );
            NonNull::new(handle)
                .map(|handle| Self {
                    handle,
                    swell: *Reaper::get().swell(),
                })
                .ok_or(ReaRsError::NullPtr("alpha pen"))
        }
        #[cfg(target_family = "windows")]
        {
            let _ = (color, alpha);
            Err(ReaRsError::UnexpectedAPI(
                "alpha pens are not supported by this backend".into(),
            ))
        }
    }

    fn raw(&self) -> raw::HPEN {
        self.handle.as_ptr() as raw::HPEN
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        unsafe { self.swell.DeleteObject(self.handle.as_ptr()) }
    }
}

pub struct Font {
    handle: NonNull<raw::HGDIOBJ__>,
    swell: rea_rs_low::Swell,
}

impl Font {
    pub fn from_logfont(mut logfont: raw::LOGFONT) -> ReaperResult<Self> {
        let handle =
            unsafe { Reaper::get().swell().CreateFontIndirect(&mut logfont) };
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("font"))
    }

    pub fn from_handle(handle: raw::HFONT) -> ReaperResult<Self> {
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("font"))
    }

    fn raw(&self) -> raw::HFONT {
        self.handle.as_ptr() as raw::HFONT
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { self.swell.DeleteObject(self.handle.as_ptr()) }
    }
}

/// Retained LICE bitmap. The mode is passed through to REAPER's LICE API.
pub struct LiceBitmap {
    handle: NonNull<LICE_IBitmap>,
    _mode: i32,
    low: rea_rs_low::Reaper,
    swell: rea_rs_low::Swell,
}

/// LICE bitmap storage allocation policy. Exact meanings are those of
/// `LICE_CreateBitmap` in the running REAPER backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum LiceBitmapKind {
    Memory = 0,
    System = 1,
}

impl LiceBitmap {
    pub fn with_kind(
        kind: LiceBitmapKind,
        width: i32,
        height: i32,
    ) -> ReaperResult<Self> {
        Self::new(kind as i32, width, height)
    }

    pub fn new(mode: i32, width: i32, height: i32) -> ReaperResult<Self> {
        if width < 1 || height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid bitmap size",
            ));
        }
        let handle =
            Reaper::get().low().LICE_CreateBitmap(mode, width, height);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                _mode: mode,
                low: *Reaper::get().low(),
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("LICE bitmap"))
    }

    pub fn load_png(
        filename: &str,
        mode: i32,
        width: i32,
        height: i32,
    ) -> ReaperResult<Self> {
        let bitmap = Self::new(mode, width, height)?;
        let filename = CString::new(filename)?;
        let loaded = unsafe {
            Reaper::get()
                .low()
                .LICE_LoadPNG(filename.as_ptr(), bitmap.handle.as_ptr())
        };
        if loaded.is_null() {
            return Err(ReaRsError::UnsuccessfulOperation("LICE_LoadPNG"));
        }
        Ok(bitmap)
    }

    pub fn load_png_resource(
        instance: raw::HINSTANCE,
        resource_id: &std::ffi::CStr,
        mode: i32,
        width: i32,
        height: i32,
    ) -> ReaperResult<Self> {
        let bitmap = Self::new(mode, width, height)?;
        let loaded = unsafe {
            Reaper::get().low().LICE_LoadPNGFromResource(
                instance,
                resource_id.as_ptr(),
                bitmap.handle.as_ptr(),
            )
        };
        if loaded.is_null() {
            return Err(ReaRsError::UnsuccessfulOperation(
                "LICE_LoadPNGFromResource",
            ));
        }
        Ok(bitmap)
    }

    pub fn width(&self) -> i32 {
        unsafe { Reaper::get().low().LICE__GetWidth(self.handle.as_ptr()) }
    }

    pub fn height(&self) -> i32 {
        unsafe { Reaper::get().low().LICE__GetHeight(self.handle.as_ptr()) }
    }

    pub fn resize(&mut self, width: i32, height: i32) -> ReaperResult<()> {
        if width < 1 || height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid bitmap size",
            ));
        }
        let ok = unsafe {
            Reaper::get().low().LICE__resize(
                self.handle.as_ptr(),
                width,
                height,
            )
        };
        if ok {
            Ok(())
        } else {
            Err(ReaRsError::UnsuccessfulOperation("LICE bitmap resize"))
        }
    }

    pub fn surface(&mut self) -> LiceSurface<'_> {
        LiceSurface {
            handle: self.handle,
            _borrow: PhantomData,
            low: self.low,
        }
    }

    fn raw(&self) -> *mut LICE_IBitmap {
        self.handle.as_ptr()
    }
}

impl Drop for LiceBitmap {
    fn drop(&mut self) {
        unsafe { self.low.LICE__Destroy(self.handle.as_ptr()) }
    }
}

pub struct LiceFont {
    handle: NonNull<LICE_IFont>,
    // LICE font configuration references this HFONT; declaration order keeps
    // the owner alive until after the LICE font is destroyed.
    _font: Font,
    low: rea_rs_low::Reaper,
}

impl LiceFont {
    pub fn from_font(font: Font) -> ReaperResult<Self> {
        let handle = Reaper::get().low().LICE_CreateFont();
        let handle =
            NonNull::new(handle).ok_or(ReaRsError::NullPtr("LICE font"))?;
        unsafe {
            Reaper::get().low().LICE__SetFromHFont(
                handle.as_ptr(),
                font.raw(),
                0,
            );
        }
        Ok(Self {
            handle,
            _font: font,
            low: *Reaper::get().low(),
        })
    }

    pub fn set_text_color(&mut self, color: u32) {
        unsafe {
            Reaper::get()
                .low()
                .LICE__SetTextColor(self.handle.as_ptr(), color as LICE_pixel);
        }
    }

    pub fn set_combine_mode(&mut self, mode: i32, alpha: f32) {
        unsafe {
            Reaper::get().low().LICE__SetTextCombineMode(
                self.handle.as_ptr(),
                mode,
                alpha.clamp(0.0, 1.0),
            );
        }
    }
}

impl Drop for LiceFont {
    fn drop(&mut self) {
        unsafe { self.low.LICE__DestroyFont(self.handle.as_ptr()) }
    }
}

pub struct LiceSurface<'bitmap> {
    handle: NonNull<LICE_IBitmap>,
    _borrow: PhantomData<&'bitmap mut LiceBitmap>,
    low: rea_rs_low::Reaper,
}

/// Retained native image-list owner. The native list does not own source
/// bitmaps/icons added to it; callers retain those resources independently.
pub struct ImageList {
    handle: NonNull<raw::HIMAGELIST__>,
    swell: rea_rs_low::Swell,
    attached: Vec<(raw::HWND, i32)>,
}

impl ImageList {
    pub fn new() -> ReaperResult<Self> {
        let swell = *Reaper::get().swell();
        let handle = swell.ImageList_CreateEx();
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell,
                attached: Vec::new(),
            })
            .ok_or(ReaRsError::NullPtr("image list"))
    }

    pub fn add_bitmap(
        &mut self,
        bitmap: raw::HBITMAP,
        mask: raw::HBITMAP,
    ) -> ReaperResult<i32> {
        let result = unsafe {
            self.swell.ImageList_Add(self.handle.as_ptr(), bitmap, mask)
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_Add"))
        } else {
            Ok(result)
        }
    }

    pub fn add_icon(&mut self, icon: raw::HICON) -> ReaperResult<i32> {
        self.replace_icon(-1, icon)
    }

    pub fn replace_icon(
        &mut self,
        index: i32,
        icon: raw::HICON,
    ) -> ReaperResult<i32> {
        let result = unsafe {
            self.swell
                .ImageList_ReplaceIcon(self.handle.as_ptr(), index, icon)
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_ReplaceIcon"))
        } else {
            Ok(result)
        }
    }

    pub fn remove(&mut self, index: i32) -> ReaperResult<()> {
        let ok = unsafe {
            self.swell.ImageList_Remove(self.handle.as_ptr(), index)
        };
        if ok == 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_Remove"))
        } else {
            Ok(())
        }
    }

    pub fn attach_to_list_view(
        &mut self,
        hwnd: raw::HWND,
        which: i32,
    ) -> ReaperResult<()> {
        if hwnd.is_null() || !unsafe { self.swell.IsWindow(hwnd) } {
            return Err(ReaRsError::InvalidObject(
                "invalid image-list target control",
            ));
        }
        unsafe {
            self.swell
                .ListView_SetImageList(hwnd, self.handle.as_ptr(), which)
        };
        if !self.attached.contains(&(hwnd, which)) {
            self.attached.push((hwnd, which));
        }
        Ok(())
    }

    pub fn detach_from_list_view(
        &mut self,
        hwnd: raw::HWND,
        which: i32,
    ) -> ReaperResult<()> {
        if hwnd.is_null() || !unsafe { self.swell.IsWindow(hwnd) } {
            self.attached.retain(|entry| *entry != (hwnd, which));
            return Ok(());
        }
        unsafe {
            self.swell
                .ListView_SetImageList(hwnd, std::ptr::null_mut(), which)
        };
        self.attached.retain(|entry| *entry != (hwnd, which));
        Ok(())
    }
}

impl Drop for ImageList {
    fn drop(&mut self) {
        for (hwnd, which) in self.attached.drain(..) {
            if unsafe { self.swell.IsWindow(hwnd) } {
                unsafe {
                    self.swell.ListView_SetImageList(
                        hwnd,
                        std::ptr::null_mut(),
                        which,
                    )
                };
            }
        }
        unsafe { self.swell.ImageList_Destroy(self.handle.as_ptr()) };
    }
}

impl LiceSurface<'_> {
    pub fn dimensions(&self) -> (i32, i32) {
        unsafe {
            (
                self.low.LICE__GetWidth(self.handle.as_ptr()),
                self.low.LICE__GetHeight(self.handle.as_ptr()),
            )
        }
    }

    pub fn fill_rect(
        &mut self,
        rect: Rect,
        color: u32,
        alpha: f32,
        combine_mode: i32,
    ) {
        unsafe {
            self.low.LICE_FillRect(
                self.handle.as_ptr(),
                rect.x as i32,
                rect.y as i32,
                rect.width as i32,
                rect.height as i32,
                color as LICE_pixel,
                alpha.clamp(0.0, 1.0),
                combine_mode,
            );
        }
    }

    pub fn line(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        color: u32,
        alpha: f32,
        combine_mode: i32,
        antialias: bool,
    ) {
        unsafe {
            self.low.LICE_Line(
                self.handle.as_ptr(),
                from.0,
                from.1,
                to.0,
                to.1,
                color as LICE_pixel,
                alpha.clamp(0.0, 1.0),
                combine_mode,
                antialias,
            );
        }
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        font: &mut LiceFont,
        flags: u32,
    ) -> ReaperResult<()> {
        let text = CString::new(text)?;
        let mut native = raw::RECT {
            left: rect.x as i32,
            top: rect.y as i32,
            right: rect.x.saturating_add(rect.width) as i32,
            bottom: rect.y.saturating_add(rect.height) as i32,
        };
        unsafe {
            self.low.LICE__DrawText(
                font.handle.as_ptr(),
                self.handle.as_ptr(),
                text.as_ptr(),
                -1,
                &mut native,
                flags,
            );
        }
        Ok(())
    }

    pub fn blit_from(
        &mut self,
        source: &LiceBitmap,
        destination: (i32, i32),
        source_rect: Rect,
        alpha: f32,
        combine_mode: i32,
    ) {
        unsafe {
            self.low.LICE_Blit(
                self.handle.as_ptr(),
                source.raw(),
                destination.0,
                destination.1,
                source_rect.x as i32,
                source_rect.y as i32,
                source_rect.width as i32,
                source_rect.height as i32,
                alpha.clamp(0.0, 1.0),
                combine_mode,
            );
        }
    }
}

/// A borrowed view of the HDC for exactly one active paint callback.
pub struct HdcSurface<'paint> {
    hdc: NonNull<raw::HDC__>,
    _paint: PhantomData<&'paint mut ()>,
    swell: rea_rs_low::Swell,
}

struct SelectedObject<'dc, 'paint> {
    surface: &'dc mut HdcSurface<'paint>,
    previous: raw::HGDIOBJ,
}

impl<'dc, 'paint> SelectedObject<'dc, 'paint> {
    fn select(
        surface: &'dc mut HdcSurface<'paint>,
        object: raw::HGDIOBJ,
    ) -> Option<Self> {
        let previous =
            unsafe { surface.swell.SelectObject(surface.raw(), object) };
        (!previous.is_null()).then_some(Self { surface, previous })
    }

    fn hdc(&self) -> raw::HDC {
        self.surface.raw()
    }
}

impl Drop for SelectedObject<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            self.surface
                .swell
                .SelectObject(self.surface.raw(), self.previous);
        }
    }
}

impl<'paint> HdcSurface<'paint> {
    pub(super) unsafe fn from_paint_hdc(hdc: raw::HDC) -> Option<Self> {
        Self::from_paint_hdc_with_swell(hdc, *Reaper::get().swell())
    }

    pub(super) unsafe fn from_paint_hdc_with_swell(
        hdc: raw::HDC,
        swell: rea_rs_low::Swell,
    ) -> Option<Self> {
        NonNull::new(hdc).map(|hdc| Self {
            hdc,
            _paint: PhantomData,
            swell,
        })
    }

    fn raw(&self) -> raw::HDC {
        self.hdc.as_ptr()
    }

    pub fn fill_rect(&mut self, rect: Rect, brush: &Brush) {
        let rect = raw::RECT::from(rect);
        unsafe { self.swell.FillRect(self.raw(), &rect, brush.raw()) }
    }

    pub fn frame_rect(&mut self, rect: Rect, brush: &Brush) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        self.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1), brush);
        if rect.height > 1 {
            self.fill_rect(
                Rect::new(rect.x, rect.y + rect.height - 1, rect.width, 1),
                brush,
            );
        }
        if rect.height > 2 {
            self.fill_rect(
                Rect::new(rect.x, rect.y + 1, 1, rect.height - 2),
                brush,
            );
            if rect.width > 1 {
                self.fill_rect(
                    Rect::new(
                        rect.x + rect.width - 1,
                        rect.y + 1,
                        1,
                        rect.height - 2,
                    ),
                    brush,
                );
            }
        }
    }

    pub fn line(&mut self, from: (i32, i32), to: (i32, i32), pen: &Pen) {
        let Some(selection) =
            SelectedObject::select(self, pen.raw() as raw::HGDIOBJ)
        else {
            return;
        };
        unsafe {
            selection.surface.swell.MoveToEx(
                selection.hdc(),
                from.0,
                from.1,
                std::ptr::null_mut(),
            );
            selection
                .surface
                .swell
                .SWELL_LineTo(selection.hdc(), to.0, to.1);
        }
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        font: &Font,
        options: DrawTextOptions,
    ) -> ReaperResult<i32> {
        let text = CString::new(text)?;
        let mut rect = raw::RECT::from(rect);
        let Some(selection) =
            SelectedObject::select(self, font.raw() as raw::HGDIOBJ)
        else {
            return Err(ReaRsError::UnsuccessfulOperation(
                "SelectObject font",
            ));
        };
        let result = unsafe {
            selection.surface.swell.DrawText(
                selection.hdc(),
                text.as_ptr(),
                -1,
                &mut rect,
                options.alignment,
            )
        };
        Ok(result)
    }

    pub fn blit_bitmap(
        &mut self,
        source: &LiceBitmap,
        source_origin: (i32, i32),
        destination: Rect,
    ) -> ReaperResult<()> {
        let bitmap_dc = unsafe { source.low.LICE__GetDC(source.raw()) };
        if bitmap_dc.is_null() {
            return Err(ReaRsError::UnexpectedAPI(
                "LICE bitmap does not expose a compatible HDC".into(),
            ));
        }
        unsafe {
            source.swell.BitBlt(
                self.raw(),
                destination.x as i32,
                destination.y as i32,
                destination.width as i32,
                destination.height as i32,
                bitmap_dc,
                source_origin.0,
                source_origin.1,
                raw::SRCCOPY as i32,
            );
        }
        Ok(())
    }
}
