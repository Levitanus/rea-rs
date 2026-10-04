//! Retained SWELL/HDC and LICE drawing resources.
//!
//! Resource owners are independent of windows and creation contexts. Keep
//! them in a concrete [`WindowHandler`] and borrow a surface only while its
//! native paint transaction is active.

use super::layout::{Point, Rect};
use super::widgets::ListView;
use crate::{Color, ReaRsError, Reaper, ReaperResult};
use int_enum::IntEnum;
use rea_rs_low::raw::{self, LICE_IBitmap, LICE_IFont, LICE_pixel};
use std::{ffi::CString, marker::PhantomData, ptr::NonNull};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawTextOptions {
    pub alignment: DrawTextFlags,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct DrawTextFlags: i32 {
        const LEFT = raw::DT_LEFT as i32;
        const CENTER = raw::DT_CENTER as i32;
        const RIGHT = raw::DT_RIGHT as i32;
        const TOP = raw::DT_TOP as i32;
        const BOTTOM = raw::DT_BOTTOM as i32;
        const VCENTER = raw::DT_VCENTER as i32;
        const SINGLE_LINE = raw::DT_SINGLELINE as i32;
        const WORD_BREAK = raw::DT_WORDBREAK as i32;
        const NO_PREFIX = raw::DT_NOPREFIX as i32;
        const END_ELLIPSIS = raw::DT_END_ELLIPSIS as i32;
    }
}

impl DrawTextFlags {
    fn raw(self) -> i32 {
        self.bits()
    }
}

impl DrawTextOptions {
    pub const fn new(alignment: DrawTextFlags) -> Self {
        Self { alignment }
    }
}

impl DrawTextFlags {
}

impl Default for DrawTextOptions {
    fn default() -> Self {
        Self {
            alignment: DrawTextFlags::LEFT | DrawTextFlags::TOP,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum LiceCombineMode {
    Copy = 0,
    Add = 1,
    Dodge = 2,
    Multiply = 3,
    Overlay = 4,
    HsvAdjust = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum LiceBitmapKind {
    Memory = 0,
    System = 1,
}

/// The ListView image-list slot receiving an attached image list.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum ListViewImageListKind {
    Normal = 0,
    Small = 1,
    State = 2,
    GroupHeader = 3,
}

/// Owned GDI bitmap. Construct only from an HBITMAP whose ownership was
/// transferred to this wrapper; it is released with DeleteObject.
pub struct Bitmap {
    handle: NonNull<raw::HGDIOBJ__>,
    swell: rea_rs_low::Swell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

impl ImageSize {
    pub const fn new(width: u32, height: u32) -> Self { Self { width, height } }
}

    /// # Safety
    /// `handle` must be a valid owned HBITMAP that can be released by
    /// `DeleteObject`; it must not be used or destroyed elsewhere afterward.

impl Bitmap {
    /// # Safety
    /// `handle` must be a valid owned HBITMAP that can be released by
    /// `DeleteObject`; it must not be used or destroyed elsewhere afterward.
    pub unsafe fn from_owned_handle(handle: raw::HBITMAP) -> ReaperResult<Self> {
        NonNull::new(handle).map(|handle| Self {
            handle: handle.cast(),
            swell: *Reaper::get().swell(),
        }).ok_or(ReaRsError::NullPtr("bitmap"))
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe { self.swell.DeleteObject(self.handle.as_ptr()) }
    }
}

/// Owned SWELL icon. Construct from a transferred `HICON` or load one by
/// name; the handle is released with `DeleteObject`.
pub struct Icon {
    handle: NonNull<raw::HGDIOBJ__>,
    swell: rea_rs_low::Swell,
}

impl Icon {
    /// # Safety
    /// `handle` must be a valid owned `HICON` releasable by `DeleteObject`;
    /// ownership is transferred and the handle must not be used or destroyed
    /// elsewhere afterward.
    pub unsafe fn from_owned_handle(
        handle: raw::HICON,
    ) -> ReaperResult<Self> {
        NonNull::new(handle)
            .map(|handle| Self {
                handle: handle.cast(),
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("icon"))
    }

    /// Loads an icon by name or path using the active SWELL backend.
    /// Supported names, paths, and image formats depend on that backend.
    #[cfg(unix)]
    pub fn load_named(
        name: &str,
        alpha_from_mask: bool,
    ) -> ReaperResult<Self> {
        let name = CString::new(name)?;
        let swell = *Reaper::get().swell();
        let handle =
            unsafe { swell.LoadNamedImage(name.as_ptr(), alpha_from_mask) };
        NonNull::new(handle)
            .map(|handle| Self {
                handle: handle.cast(),
                swell,
            })
            .ok_or(ReaRsError::NullPtr("loaded icon"))
    }

    fn raw(&self) -> raw::HICON {
        self.handle.as_ptr() as raw::HICON
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        unsafe { self.swell.DeleteObject(self.handle.as_ptr()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiceBlitOptions {
    pub mode: LiceCombineMode,
    pub bilinear: bool,
    pub use_source_alpha: bool,
}

impl Default for LiceBlitOptions {
    fn default() -> Self {
        Self {
            mode: LiceCombineMode::Copy,
            bilinear: false,
            use_source_alpha: false,
        }
    }
}

impl LiceBlitOptions {
    pub(crate) fn raw(self) -> i32 {
        self.mode.int_value() | if self.bilinear { 0x100 } else { 0 }
            | if self.use_source_alpha { 0x10000 } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiceTextOptions {
    pub flags: DrawTextFlags,
    pub combine: LiceBlitOptions,
    pub alpha: f32,
}

impl Default for LiceTextOptions {
    fn default() -> Self {
        Self {
            flags: DrawTextFlags::LEFT | DrawTextFlags::TOP,
            combine: LiceBlitOptions::default(),
            alpha: 1.0,
        }
    }
}

fn lice_pixel(color: Color) -> LICE_pixel {
    // WDL's LICE_RGBA(r, g, b, a) stores B, G, R, A in ascending bytes.
    color.b as u32
        | ((color.g as u32) << 8)
        | ((color.r as u32) << 16)
        | (0xff << 24)
}

fn swell_color(color: Color) -> i32 {
    color.to_native()
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum PenStyle {
    Solid = 0,
    Dash = 1,
    Dot = 2,
    DashDot = 3,
    DashDotDot = 4,
    Null = 5,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum FontCharset {
    Ansi = 0,
    Default = 1,
    Symbol = 2,
    ShiftJis = 128,
    Hangul = 129,
    Gb2312 = 134,
    Big5 = 136,
    Greek = 161,
    Turkish = 162,
    Hebrew = 177,
    Arabic = 178,
    Baltic = 186,
    Russian = 204,
    Thai = 222,
    EastEurope = 238,
    Oem = 255,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSpec {
    pub face: String,
    pub size: u32,
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub charset: FontCharset,
}

impl FontSpec {
    pub fn new(font_face: impl Into<String>) -> Self {
        Self {
            face: font_face.into(),
            size: 14,
            weight: 400,
            italic: false,
            underline: false,
            strikeout: false,
            charset: FontCharset::Default,
        }
    }
    pub fn set_size(mut self, size: u32) -> Self {
        self.size = size.max(1);
        self
    }
    pub fn set_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }
    pub fn set_italic(mut self, value: bool) -> Self {
        self.italic = value;
        self
    }
    pub fn set_underline(mut self, value: bool) -> Self {
        self.underline = value;
        self
    }
    pub fn set_strikedout(mut self, value: bool) -> Self {
        self.strikeout = value;
        self
    }
    pub fn set_charset(mut self, charset: FontCharset) -> Self {
        self.charset = charset;
        self
    }

    fn to_logfont(&self) -> raw::LOGFONT {
        let mut logfont = raw::LOGFONT::default();
        logfont.lfHeight = -(self.size.min(i32::MAX as u32) as i32);
        logfont.lfWeight = self.weight as i32;
        logfont.lfItalic = self.italic as i8;
        logfont.lfUnderline = self.underline as i8;
        logfont.lfStrikeOut = self.strikeout as i8;
        logfont.lfCharSet = self.charset.int_value() as i8;
        for (destination, source) in
            logfont.lfFaceName.iter_mut().zip(self.face.bytes())
        {
            *destination = if source.is_ascii() {
                source as i8
            } else {
                b'?' as i8
            };
        }
        logfont
    }
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
    pub fn solid(color: Color) -> ReaperResult<Self> {
        let handle = Reaper::get().swell().CreateSolidBrush(color.to_native());
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("solid brush"))
    }

    pub fn alpha(color: Color, alpha: f32) -> ReaperResult<Self> {
        #[cfg(target_family = "unix")]
        {
            let handle = Reaper::get()
                .swell()
                .CreateSolidBrushAlpha(color.to_native(), alpha.clamp(0.0, 1.0));
            NonNull::new(handle)
                .map(|handle| Self {
                    handle,
                    swell: *Reaper::get().swell(),
                })
                .ok_or(ReaRsError::NullPtr("alpha brush"))
        }
        #[cfg(target_family = "windows")]
        {
            let _ = alpha;
            log::warn!("alpha brushes are unsupported on Windows; using a solid brush");
            Self::solid(color)
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
    pub fn solid(width: u32, color: Color) -> ReaperResult<Self> {
        Self::with_style(width, color, PenStyle::Solid)
    }

    pub fn with_style(width: u32, color: Color, style: PenStyle) -> ReaperResult<Self> {
        if width == 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid pen width",
            ));
        }
        let color = swell_color(color);
        let handle = Reaper::get().swell().CreatePen(style.int_value(), width.min(i32::MAX as u32) as i32, color);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("pen"))
    }

    pub fn alpha(width: u32, color: Color, alpha: f32) -> ReaperResult<Self> {
        if width == 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid pen width",
            ));
        }
        #[cfg(target_family = "unix")]
        {
            let color = swell_color(color);
            let handle = Reaper::get().swell().CreatePenAlpha(
                PenStyle::Solid.int_value(),
                width.min(i32::MAX as u32) as i32,
                color,
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
            log::warn!("alpha pens are unsupported on Windows; using a solid pen");
            Self::solid(width, color)
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
    pub fn new(spec: FontSpec) -> ReaperResult<Self> {
        Self::from_logfont(spec.to_logfont())
    }

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

impl LiceBitmapKind {
    fn raw(self) -> i32 {
        self.int_value()
    }
}

/// LICE bitmap storage allocation policy. Exact meanings are those of
/// `LICE_CreateBitmap` in the running REAPER backend.
impl LiceBitmap {
    pub fn with_kind(
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        Self::new(kind, width, height)
    }

    pub fn new(kind: LiceBitmapKind, width: u32, height: u32) -> ReaperResult<Self> {
        Self::create(kind, width, height)
    }

    fn create(kind: LiceBitmapKind, width: u32, height: u32) -> ReaperResult<Self> {
        let width = width.min(i32::MAX as u32) as i32;
        let height = height.min(i32::MAX as u32) as i32;
        if width < 1 || height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid bitmap size",
            ));
        }
        let handle =
            Reaper::get().low().LICE_CreateBitmap(kind.raw(), width, height);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                _mode: kind.raw(),
                low: *Reaper::get().low(),
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("LICE bitmap"))
    }

    pub fn load_png(
        filename: &str,
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        let bitmap = Self::create(kind, width, height)?;
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
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        let bitmap = Self::create(kind, width, height)?;
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

    pub fn width(&self) -> u32 {
        unsafe { self.low.LICE__GetWidth(self.handle.as_ptr()).max(0) as u32 }
    }

    pub fn height(&self) -> u32 {
        unsafe { self.low.LICE__GetHeight(self.handle.as_ptr()).max(0) as u32 }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> ReaperResult<()> {
        if width == 0 || height == 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid bitmap size",
            ));
        }
        let ok = unsafe {
            self.low.LICE__resize(
                self.handle.as_ptr(),
                width.min(i32::MAX as u32) as i32,
                height.min(i32::MAX as u32) as i32,
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

    pub fn set_text_color(&mut self, color: Color) {
        unsafe {
            Reaper::get()
                .low()
                .LICE__SetTextColor(self.handle.as_ptr(), lice_pixel(color));
        }
    }

    pub fn set_combine_mode(&mut self, options: LiceBlitOptions, alpha: f32) {
        unsafe {
            Reaper::get().low().LICE__SetTextCombineMode(
                self.handle.as_ptr(),
                options.raw(),
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
        bitmap: &Bitmap,
        mask: Option<&Bitmap>,
    ) -> ReaperResult<i32> {
        let result = unsafe {
            self.swell.ImageList_Add(
                self.handle.as_ptr(),
                bitmap.handle.as_ptr() as raw::HBITMAP,
                mask.map_or(std::ptr::null_mut(), |mask| mask.handle.as_ptr() as raw::HBITMAP),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_Add"))
        } else {
            Ok(result)
        }
    }

    pub fn add_borrowed_icon(&mut self, icon: &Icon) -> ReaperResult<i32> {
        self.add_icon(icon)
    }

    /// Adds an icon without transferring ownership to the image list.
    pub fn add_icon(&mut self, icon: &Icon) -> ReaperResult<i32> {
        self.replace_icon(-1, icon)
    }

    pub fn replace_icon(
        &mut self,
        index: i32,
        icon: &Icon,
    ) -> ReaperResult<i32> {
        let result = unsafe {
            self.swell
                .ImageList_ReplaceIcon(self.handle.as_ptr(), index, icon.raw())
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
        list_view: &ListView,
        which: ListViewImageListKind,
    ) -> ReaperResult<()> {
        let hwnd = list_view.hwnd();
        if hwnd.is_null() || !unsafe { self.swell.IsWindow(hwnd) } {
            return Err(ReaRsError::InvalidObject(
                "invalid image-list target control",
            ));
        }
        unsafe {
            self.swell
                .ListView_SetImageList(hwnd, self.handle.as_ptr(), which.int_value())
        };
        if !self.attached.contains(&(hwnd, which.int_value())) {
            self.attached.push((hwnd, which.int_value()));
        }
        Ok(())
    }

    pub fn detach_from_list_view(
        &mut self,
        list_view: &ListView,
        which: ListViewImageListKind,
    ) -> ReaperResult<()> {
        let hwnd = list_view.hwnd();
        if hwnd.is_null() || !unsafe { self.swell.IsWindow(hwnd) } {
            self.attached.retain(|entry| *entry != (hwnd, which.int_value()));
            return Ok(());
        }
        let attached = self.attached.contains(&(hwnd, which.int_value()));
        if attached {
            unsafe {
                self.swell.ListView_SetImageList(
                    hwnd,
                    std::ptr::null_mut(),
                    which.int_value(),
                )
            };
        }
        self.attached.retain(|entry| *entry != (hwnd, which.int_value()));
        Ok(())
    }
}

impl Drop for ImageList {
    fn drop(&mut self) {
        for (hwnd, which) in self.attached.drain(..) {
                if !hwnd.is_null() && unsafe { self.swell.IsWindow(hwnd) } {
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
    pub fn dimensions(&self) -> ImageSize {
        unsafe {
            ImageSize::new(
                self.low.LICE__GetWidth(self.handle.as_ptr()).max(0) as u32,
                self.low.LICE__GetHeight(self.handle.as_ptr()).max(0) as u32,
            )
        }
    }

    pub fn fill_rect(
        &mut self,
        rect: Rect,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_FillRect(
                self.handle.as_ptr(),
                i32::try_from(rect.x).unwrap_or(i32::MAX),
                i32::try_from(rect.y).unwrap_or(i32::MAX),
                i32::try_from(rect.width).unwrap_or(i32::MAX),
                i32::try_from(rect.height).unwrap_or(i32::MAX),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    pub fn line(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
    ) {
        unsafe {
            self.low.LICE_Line(
                self.handle.as_ptr(),
                from.x as f32,
                from.y as f32,
                to.x as f32,
                to.y as f32,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
            );
        }
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        font: &mut LiceFont,
        options: LiceTextOptions,
    ) -> ReaperResult<()> {
        let text = CString::new(text)?;
        let mut native = raw::RECT {
                left: i32::try_from(rect.x).unwrap_or(i32::MAX),
                top: i32::try_from(rect.y).unwrap_or(i32::MAX),
                right: i32::try_from(rect.x.saturating_add(rect.width))
                    .unwrap_or(i32::MAX),
                bottom: i32::try_from(rect.y.saturating_add(rect.height))
                    .unwrap_or(i32::MAX),
        };
        unsafe {
            self.low.LICE__SetTextCombineMode(
                font.handle.as_ptr(),
                options.combine.raw(),
                options.alpha.clamp(0.0, 1.0),
            );
            let _ = self.low.LICE__DrawText(
                font.handle.as_ptr(),
                self.handle.as_ptr(),
                text.as_ptr(),
                -1,
                &mut native,
                options.flags.bits() as u32,
            );
        }
        Ok(())
    }

    pub fn blit_from(
        &mut self,
        source: &LiceBitmap,
        destination: Point,
        source_rect: Rect,
        alpha: f32,
        options: LiceBlitOptions,
    ) {
        unsafe {
            self.low.LICE_Blit(
                self.handle.as_ptr(),
                source.raw(),
                i32::try_from(destination.x).unwrap_or(i32::MAX),
                i32::try_from(destination.y).unwrap_or(i32::MAX),
                i32::try_from(source_rect.x).unwrap_or(i32::MAX),
                i32::try_from(source_rect.y).unwrap_or(i32::MAX),
                i32::try_from(source_rect.width).unwrap_or(i32::MAX),
                i32::try_from(source_rect.height).unwrap_or(i32::MAX),
                alpha.clamp(0.0, 1.0),
                options.raw(),
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

    pub fn line(&mut self, from: Point, to: Point, pen: &Pen) {
        let Some(selection) =
            SelectedObject::select(self, pen.raw() as raw::HGDIOBJ)
        else {
            return;
        };
        unsafe {
            selection.surface.swell.MoveToEx(
                selection.hdc(),
                i32::try_from(from.x).unwrap_or(i32::MAX),
                i32::try_from(from.y).unwrap_or(i32::MAX),
                std::ptr::null_mut(),
            );
            selection
                .surface
                .swell
                .SWELL_LineTo(
                    selection.hdc(),
                    i32::try_from(to.x).unwrap_or(i32::MAX),
                    i32::try_from(to.y).unwrap_or(i32::MAX),
                );
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
                options.alignment.raw(),
            )
        };
        Ok(result)
    }

    pub fn blit_bitmap(
        &mut self,
        source: &LiceBitmap,
        source_origin: Point,
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
                i32::try_from(source_origin.x).unwrap_or(i32::MAX),
                i32::try_from(source_origin.y).unwrap_or(i32::MAX),
                raw::SRCCOPY as i32,
            );
        }
        Ok(())
    }
}
