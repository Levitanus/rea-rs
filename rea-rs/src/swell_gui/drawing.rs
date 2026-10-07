//! Retained SWELL resources and LICE drawing surfaces.
//!
//! Resource owners are independent of windows and creation contexts. Keep
//! them in a concrete [`super::windows::WindowHandler`]. LICE surfaces are
//! borrowed only for the duration of a paint callback.

use super::layout::{Point, Rect};
use super::widgets::ListView;
use crate::{Color, ReaRsError, Reaper, ReaperResult};
use int_enum::IntEnum;
use rea_rs_low::raw::{self, LICE_IBitmap, LICE_IFont, LICE_pixel};
use std::{ffi::CString, marker::PhantomData, ptr::NonNull};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Text alignment and layout flags accepted by SWELL drawing operations.
///
/// These are native `DT_*` flags. Not every backend honors every combination.
pub struct DrawTextOptions {
    /// Combined native alignment and line-layout flags.
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

impl DrawTextOptions {
    /// Creates options from native-compatible draw-text flags.
    pub const fn new(alignment: DrawTextFlags) -> Self {
        Self { alignment }
    }
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
    /// Replace destination pixels with source pixels.
    Copy = 0,
    /// Add source color to destination.
    Add = 1,
    /// LICE dodge blend mode.
    Dodge = 2,
    /// Multiply source and destination colors.
    Multiply = 3,
    /// LICE overlay blend mode.
    Overlay = 4,
    /// Adjust hue, saturation, and value using source pixels.
    HsvAdjust = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntEnum)]
pub enum LiceBitmapKind {
    /// CPU-memory-backed bitmap.
    Memory = 0,
    /// System bitmap suitable for APIs requiring an HDC.
    System = 1,
}

impl LiceBitmapKind {
    fn raw(self) -> i32 {
        self.int_value()
    }
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
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl ImageSize {
    /// Creates dimensions in pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A floating-point coordinate for LICE primitives that support subpixel
/// positions (including arcs and cubic Bezier curves).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LicePoint {
    pub x: f64,
    pub y: f64,
}

impl LicePoint {
    /// Creates a subpixel point in bitmap pixels.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A floating-point rectangle for LICE source regions and transformed blits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LiceRect {
    /// Creates a subpixel rectangle in bitmap pixels.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// # Safety
/// `handle` must be a valid owned HBITMAP that can be released by
/// `DeleteObject`; it must not be used or destroyed elsewhere afterward.

impl Bitmap {
    /// # Safety
    /// `handle` must be a valid owned HBITMAP that can be released by
    /// `DeleteObject`; it must not be used or destroyed elsewhere afterward.
    pub unsafe fn from_owned_handle(
        handle: raw::HBITMAP,
    ) -> ReaperResult<Self> {
        NonNull::new(handle)
            .map(|handle| Self {
                handle: handle.cast(),
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("bitmap"))
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
    pub unsafe fn from_owned_handle(handle: raw::HICON) -> ReaperResult<Self> {
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
    /// Pixel-combination mode.
    pub mode: LiceCombineMode,
    /// Request bilinear sampling when scaling.
    pub bilinear: bool,
    /// Include source alpha in the blend.
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
        self.mode.int_value()
            | if self.bilinear { 0x100 } else { 0 }
            | if self.use_source_alpha { 0x10000 } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiceTextOptions {
    /// Text alignment and layout flags.
    pub flags: DrawTextFlags,
    /// Pixel-combination behavior for the text.
    pub combine: LiceBlitOptions,
    /// Text opacity, clamped to the inclusive range `0.0..=1.0`.
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

fn native_coord(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
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
    /// Requested face name. Non-ASCII bytes are replaced when converted to
    /// the native fixed-width LOGFONT face-name field.
    pub face: String,
    /// Requested font size in pixels.
    pub size: u32,
    /// Native LOGFONT weight.
    pub weight: u16,
    /// Whether the font is italic.
    pub italic: bool,
    /// Whether the font is underlined.
    pub underline: bool,
    /// Whether the font is struck out.
    pub strikeout: bool,
    /// Character-set hint passed to the backend.
    pub charset: FontCharset,
}

impl FontSpec {
    /// Creates a font specification with a 14-pixel regular default.
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
    /// Sets the requested pixel size; zero is raised to one.
    pub fn set_size(mut self, size: u32) -> Self {
        self.size = size.max(1);
        self
    }
    /// Sets the native LOGFONT weight.
    pub fn set_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }
    /// Enables or disables italic styling.
    pub fn set_italic(mut self, value: bool) -> Self {
        self.italic = value;
        self
    }
    /// Enables or disables underline styling.
    pub fn set_underline(mut self, value: bool) -> Self {
        self.underline = value;
        self
    }
    /// Enables or disables strikeout styling.
    pub fn set_strikedout(mut self, value: bool) -> Self {
        self.strikeout = value;
        self
    }
    /// Sets the native character-set hint.
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
    /// Damaged region that triggered painting, in client coordinates.
    pub damage_rect: Rect,
    /// Full client-area bounds, in client coordinates.
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
    /// Creates a solid brush for native controls and SWELL erase procedures.
    pub fn solid(color: Color) -> ReaperResult<Self> {
        let handle = Reaper::get().swell().CreateSolidBrush(color.to_native());
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("solid brush"))
    }

    /// Creates an alpha brush where supported. Alpha is clamped to
    /// `0.0..=1.0`; Windows falls back to an opaque solid brush.
    pub fn alpha(color: Color, alpha: f32) -> ReaperResult<Self> {
        #[cfg(target_family = "unix")]
        {
            let handle = Reaper::get().swell().CreateSolidBrushAlpha(
                color.to_native(),
                alpha.clamp(0.0, 1.0),
            );
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
    /// Creates a solid GDI pen for native-control resources. For custom
    /// drawing, use the line methods on [`LiceSurface`].
    pub fn solid(width: u32, color: Color) -> ReaperResult<Self> {
        Self::with_style(width, color, PenStyle::Solid)
    }

    /// Creates a styled pen. Width is in pixels and must be nonzero.
    pub fn with_style(
        width: u32,
        color: Color,
        style: PenStyle,
    ) -> ReaperResult<Self> {
        if width == 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid pen width",
            ));
        }
        let color = swell_color(color);
        let handle = Reaper::get().swell().CreatePen(
            style.int_value(),
            width.min(i32::MAX as u32) as i32,
            color,
        );
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("pen"))
    }

    /// Creates an alpha pen where supported. Alpha is clamped to `0.0..=1.0`;
    /// Windows falls back to an opaque solid pen.
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
            log::warn!(
                "alpha pens are unsupported on Windows; using a solid pen"
            );
            Self::solid(width, color)
        }
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
    /// Creates a native font from a high-level specification.
    pub fn new(spec: FontSpec) -> ReaperResult<Self> {
        Self::from_logfont(spec.to_logfont())
    }

    /// Creates a font from a LOGFONT value.
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

    /// Wraps a non-null HFONT. This wrapper assumes responsibility for
    /// releasing it with `DeleteObject`; do not use this with a borrowed
    /// handle.
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
impl LiceBitmap {
    /// Creates a bitmap with the requested LICE storage kind and dimensions.
    pub fn with_kind(
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        Self::new(kind, width, height)
    }

    /// Creates a bitmap with the requested LICE storage kind and dimensions.
    pub fn new(
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        Self::create(kind, width, height)
    }

    fn create(
        kind: LiceBitmapKind,
        width: u32,
        height: u32,
    ) -> ReaperResult<Self> {
        let width = width.min(i32::MAX as u32) as i32;
        let height = height.min(i32::MAX as u32) as i32;
        if width < 1 || height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid bitmap size",
            ));
        }
        let handle =
            Reaper::get()
                .low()
                .LICE_CreateBitmap(kind.raw(), width, height);
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                _mode: kind.raw(),
                low: *Reaper::get().low(),
                swell: *Reaper::get().swell(),
            })
            .ok_or(ReaRsError::NullPtr("LICE bitmap"))
    }

    /// Loads a PNG into a newly allocated memory-backed bitmap.
    ///
    /// The bitmap dimensions are taken from the image. Loading fails if the
    /// filename contains an interior NUL byte or the native loader returns
    /// null. Use [`Self::new`] and an explicit load operation when a System
    /// bitmap/HDC-compatible destination is required.
    pub fn load_png(filename: &str) -> ReaperResult<Self> {
        let filename = CString::new(filename)?;
        let low = *Reaper::get().low();
        let swell = *Reaper::get().swell();
        let handle = unsafe {
            low.LICE_LoadPNG(filename.as_ptr(), std::ptr::null_mut())
        };
        NonNull::new(handle)
            .map(|handle| Self {
                handle,
                _mode: LiceBitmapKind::Memory.raw(),
                low,
                swell,
            })
            .ok_or(ReaRsError::UnsuccessfulOperation("LICE_LoadPNG"))
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

    pub(crate) fn raw(&self) -> *mut LICE_IBitmap {
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
    /// Creates a LICE font that retains the native font for its lifetime.
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

    /// Sets the text color.
    pub fn set_text_color(&mut self, color: Color) {
        unsafe {
            Reaper::get()
                .low()
                .LICE__SetTextColor(self.handle.as_ptr(), lice_pixel(color));
        }
    }

    /// Sets blend options and clamped text opacity (`0.0..=1.0`).
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
    /// Creates an owned native image list.
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

    /// Adds a bitmap and optional mask without transferring bitmap ownership.
    pub fn add_bitmap(
        &mut self,
        bitmap: &Bitmap,
        mask: Option<&Bitmap>,
    ) -> ReaperResult<i32> {
        let result = unsafe {
            self.swell.ImageList_Add(
                self.handle.as_ptr(),
                bitmap.handle.as_ptr() as raw::HBITMAP,
                mask.map_or(std::ptr::null_mut(), |mask| {
                    mask.handle.as_ptr() as raw::HBITMAP
                }),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_Add"))
        } else {
            Ok(result)
        }
    }

    /// Adds an icon without transferring icon ownership.
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
            self.swell.ImageList_ReplaceIcon(
                self.handle.as_ptr(),
                index,
                icon.raw(),
            )
        };
        if result < 0 {
            Err(ReaRsError::UnsuccessfulOperation("ImageList_ReplaceIcon"))
        } else {
            Ok(result)
        }
    }

    /// Removes the image at the zero-based index.
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

    /// Attaches this image list to a ListView slot. The list detaches itself
    /// from still-valid controls when dropped.
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
            self.swell.ListView_SetImageList(
                hwnd,
                self.handle.as_ptr(),
                which.int_value(),
            )
        };
        if !self.attached.contains(&(hwnd, which.int_value())) {
            self.attached.push((hwnd, which.int_value()));
        }
        Ok(())
    }

    /// Detaches this image list from a ListView slot if attached.
    pub fn detach_from_list_view(
        &mut self,
        list_view: &ListView,
        which: ListViewImageListKind,
    ) -> ReaperResult<()> {
        let hwnd = list_view.hwnd();
        if hwnd.is_null() || !unsafe { self.swell.IsWindow(hwnd) } {
            self.attached
                .retain(|entry| *entry != (hwnd, which.int_value()));
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
        self.attached
            .retain(|entry| *entry != (hwnd, which.int_value()));
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
    pub(crate) fn raw_bitmap(&mut self) -> *mut LICE_IBitmap {
        self.handle.as_ptr()
    }

    /// Returns the bitmap dimensions in pixels.
    pub fn dimensions(&self) -> ImageSize {
        unsafe {
            ImageSize::new(
                self.low.LICE__GetWidth(self.handle.as_ptr()).max(0) as u32,
                self.low.LICE__GetHeight(self.handle.as_ptr()).max(0) as u32,
            )
        }
    }

    /// Clears the entire bitmap to an opaque RGB color.
    pub fn clear(&mut self, color: Color) {
        unsafe { self.low.LICE_Clear(self.handle.as_ptr(), lice_pixel(color)) }
    }

    /// Clears pixels in `rect` whose bits match `mask`, applying `or_bits`.
    /// The masks are raw LICE pixel values in BGRA byte order.
    pub fn clear_rect(&mut self, rect: Rect, mask: u32, or_bits: u32) {
        unsafe {
            self.low.LICE_ClearRect(
                self.handle.as_ptr(),
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                mask,
                or_bits,
            );
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

    /// Draws the outline of a rectangle.
    pub fn draw_rect(
        &mut self,
        rect: Rect,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        unsafe {
            self.low.LICE_DrawRect(
                self.handle.as_ptr(),
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Draws a rectangle with separate background and border colors.
    pub fn bordered_rect(
        &mut self,
        rect: Rect,
        background: Color,
        border: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        unsafe {
            self.low.LICE_BorderedRect(
                self.handle.as_ptr(),
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                lice_pixel(background),
                lice_pixel(border),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Draws a circular outline. `radius` is measured in pixels.
    pub fn circle(
        &mut self,
        center: LicePoint,
        radius: f32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
    ) {
        if radius < 0.0 || !radius.is_finite() {
            return;
        }
        unsafe {
            self.low.LICE_Circle(
                self.handle.as_ptr(),
                center.x as f32,
                center.y as f32,
                radius,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
            );
        }
    }

    /// Fills a circle. `radius` is measured in pixels.
    pub fn fill_circle(
        &mut self,
        center: LicePoint,
        radius: f32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
    ) {
        if radius < 0.0 || !radius.is_finite() {
            return;
        }
        unsafe {
            self.low.LICE_FillCircle(
                self.handle.as_ptr(),
                center.x as f32,
                center.y as f32,
                radius,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
            );
        }
    }

    /// Draws a circular arc. Angles are in radians, following LICE's API.
    pub fn arc(
        &mut self,
        center: LicePoint,
        radius: f32,
        min_angle: f32,
        max_angle: f32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
    ) {
        if radius < 0.0 || !radius.is_finite() {
            return;
        }
        unsafe {
            self.low.LICE_Arc(
                self.handle.as_ptr(),
                center.x as f32,
                center.y as f32,
                radius,
                min_angle,
                max_angle,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
            );
        }
    }

    /// Draws a rounded rectangle outline. The corner radius is in pixels.
    pub fn round_rect(
        &mut self,
        rect: Rect,
        corner_radius: u32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        unsafe {
            self.low.LICE_RoundRect(
                self.handle.as_ptr(),
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                native_coord(corner_radius),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
            );
        }
    }

    /// Draws a cubic Bezier curve through the start/end points and control
    /// points. `tolerance` controls curve subdivision in LICE pixels.
    pub fn draw_cubic_bezier(
        &mut self,
        start: LicePoint,
        control1: LicePoint,
        control2: LicePoint,
        end: LicePoint,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
        tolerance: f64,
    ) {
        unsafe {
            self.low.LICE_DrawCBezier(
                self.handle.as_ptr(),
                start.x,
                start.y,
                control1.x,
                control1.y,
                control2.x,
                control2.y,
                end.x,
                end.y,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
                tolerance,
            );
        }
    }

    /// Fills under a cubic Bezier curve down to the horizontal `fill_y` line.
    pub fn fill_cubic_bezier(
        &mut self,
        start: LicePoint,
        control1: LicePoint,
        control2: LicePoint,
        end: LicePoint,
        fill_y: u32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
        antialias: bool,
        tolerance: f64,
    ) {
        unsafe {
            self.low.LICE_FillCBezier(
                self.handle.as_ptr(),
                start.x,
                start.y,
                control1.x,
                control1.y,
                control2.x,
                control2.y,
                end.x,
                end.y,
                native_coord(fill_y),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                antialias,
                tolerance,
            );
        }
    }

    /// Fills a convex polygon. Fewer than three points produce no drawing.
    pub fn fill_convex_polygon(
        &mut self,
        points: &[Point],
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        if points.len() < 3 || points.len() > i32::MAX as usize {
            return;
        }
        let mut xs: Vec<_> =
            points.iter().map(|point| native_coord(point.x)).collect();
        let mut ys: Vec<_> =
            points.iter().map(|point| native_coord(point.y)).collect();
        unsafe {
            self.low.LICE_FillConvexPolygon(
                self.handle.as_ptr(),
                xs.as_mut_ptr(),
                ys.as_mut_ptr(),
                points.len() as i32,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Reads a raw LICE pixel. The returned value is packed BGRA, as used by
    /// `LICE_pixel`; out-of-bounds coordinates return zero per LICE behavior.
    pub fn get_pixel(&self, position: Point) -> u32 {
        unsafe {
            self.low.LICE_GetPixel(
                self.handle.as_ptr(),
                native_coord(position.x),
                native_coord(position.y),
            )
        }
    }

    /// Flood-fills a contiguous region matching the seed pixel under
    /// `compare_mask`, preserving bits selected by `keep_mask`.
    pub fn simple_fill(
        &mut self,
        seed: Point,
        new_pixel: u32,
        compare_mask: u32,
        keep_mask: u32,
    ) {
        unsafe {
            self.low.LICE_SimpleFill(
                self.handle.as_ptr(),
                native_coord(seed.x),
                native_coord(seed.y),
                new_pixel,
                compare_mask,
                keep_mask,
            );
        }
    }

    /// Blurs a source rectangle into this bitmap at the given destination
    /// origin. Source and destination may alias only if LICE permits it.
    pub fn blur_from(
        &mut self,
        source: &LiceBitmap,
        destination: Point,
        source_rect: Rect,
    ) {
        unsafe {
            self.low.LICE_Blur(
                self.handle.as_ptr(),
                source.raw(),
                native_coord(destination.x),
                native_coord(destination.y),
                native_coord(source_rect.x),
                native_coord(source_rect.y),
                native_coord(source_rect.width),
                native_coord(source_rect.height),
            );
        }
    }

    /// Scales a floating-point source region into an integer destination
    /// rectangle.
    pub fn scaled_blit_from(
        &mut self,
        source: &LiceBitmap,
        destination: Rect,
        source_rect: LiceRect,
        alpha: f32,
        options: LiceBlitOptions,
    ) {
        unsafe {
            self.low.LICE_ScaledBlit(
                self.handle.as_ptr(),
                source.raw(),
                native_coord(destination.x),
                native_coord(destination.y),
                native_coord(destination.width),
                native_coord(destination.height),
                source_rect.x,
                source_rect.y,
                source_rect.width,
                source_rect.height,
                alpha.clamp(0.0, 1.0),
                options.raw(),
            );
        }
    }

    /// Rotates a floating-point source region while blitting into an integer
    /// destination rectangle. `angle` is in radians; `rotation_center` is
    /// relative to the source region.
    pub fn rotated_blit_from(
        &mut self,
        source: &LiceBitmap,
        destination: Rect,
        source_rect: LiceRect,
        angle: f32,
        clip_to_source_rect: bool,
        alpha: f32,
        options: LiceBlitOptions,
        rotation_center: LicePoint,
    ) {
        unsafe {
            self.low.LICE_RotatedBlit(
                self.handle.as_ptr(),
                source.raw(),
                native_coord(destination.x),
                native_coord(destination.y),
                native_coord(destination.width),
                native_coord(destination.height),
                source_rect.x,
                source_rect.y,
                source_rect.width,
                source_rect.height,
                angle,
                clip_to_source_rect,
                alpha.clamp(0.0, 1.0),
                options.raw(),
                rotation_center.x as f32,
                rotation_center.y as f32,
            );
        }
    }

    /// Draws simple text at a position using LICE's built-in bitmap font.
    pub fn draw_text_simple(
        &mut self,
        position: Point,
        text: &str,
        color: Color,
        alpha: f32,
        options: LiceBlitOptions,
    ) -> ReaperResult<()> {
        let text = CString::new(text)?;
        unsafe {
            self.low.LICE_DrawText(
                self.handle.as_ptr(),
                native_coord(position.x),
                native_coord(position.y),
                text.as_ptr(),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                options.raw(),
            );
        }
        Ok(())
    }

    /// Fills a triangle.
    pub fn fill_triangle(
        &mut self,
        a: Point,
        b: Point,
        c: Point,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_FillTriangle(
                self.handle.as_ptr(),
                native_coord(a.x),
                native_coord(a.y),
                native_coord(b.x),
                native_coord(b.y),
                native_coord(c.x),
                native_coord(c.y),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Fills a trapezoid described by horizontal spans at `y1` and `y2`.
    pub fn fill_trapezoid(
        &mut self,
        x1a: u32,
        x1b: u32,
        y1: u32,
        x2a: u32,
        x2b: u32,
        y2: u32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_FillTrapezoid(
                self.handle.as_ptr(),
                native_coord(x1a),
                native_coord(x1b),
                native_coord(y1),
                native_coord(x2a),
                native_coord(x2b),
                native_coord(y2),
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Draws a line with the requested integer pixel width.
    pub fn thick_line(
        &mut self,
        from: LicePoint,
        to: LicePoint,
        width: u32,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        if width == 0 {
            return;
        }
        unsafe {
            self.low.LICE_ThickFLine(
                self.handle.as_ptr(),
                from.x,
                from.y,
                to.x,
                to.y,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
                native_coord(width),
            );
        }
    }

    /// Draws a single byte-sized character using LICE's current bitmap font.
    pub fn draw_char(
        &mut self,
        position: Point,
        character: u8,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_DrawChar(
                self.handle.as_ptr(),
                native_coord(position.x),
                native_coord(position.y),
                character as i8,
                lice_pixel(color),
                alpha.clamp(0.0, 1.0),
                combine_mode.int_value(),
            );
        }
    }

    /// Adds horizontal and vertical RGBA gradients to a rectangle. Each array
    /// is ordered `[red, green, blue, alpha]`; values follow LICE's channel
    /// units and are not clamped.
    pub fn gradient_rect(
        &mut self,
        rect: Rect,
        initial: [f32; 4],
        delta_x: [f32; 4],
        delta_y: [f32; 4],
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_GradRect(
                self.handle.as_ptr(),
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                initial[0],
                initial[1],
                initial[2],
                initial[3],
                delta_x[0],
                delta_x[1],
                delta_x[2],
                delta_x[3],
                delta_y[0],
                delta_y[1],
                delta_y[2],
                delta_y[3],
                combine_mode.int_value(),
            );
        }
    }

    /// Scales and offsets RGBA channels in the selected rectangle. Arrays are
    /// ordered `[red, green, blue, alpha]`.
    pub fn multiply_add_rect(
        &mut self,
        rect: Rect,
        scale: [f32; 4],
        add: [f32; 4],
    ) {
        unsafe {
            self.low.LICE_MultiplyAddRect(
                self.handle.as_ptr(),
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                scale[0],
                scale[1],
                scale[2],
                scale[3],
                add[0],
                add[1],
                add[2],
                add[3],
            );
        }
    }

    /// Writes one pixel using the selected LICE combine mode.
    pub fn put_pixel(
        &mut self,
        position: Point,
        color: Color,
        alpha: f32,
        combine_mode: LiceCombineMode,
    ) {
        unsafe {
            self.low.LICE_PutPixel(
                self.handle.as_ptr(),
                native_coord(position.x),
                native_coord(position.y),
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
        font: &LiceFont,
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

/// The narrow HDC bridge used by the paint transaction for background
/// preservation and presenting a completed LICE bitmap.
pub(super) struct HdcSurface<'paint> {
    hdc: NonNull<raw::HDC__>,
    _paint: PhantomData<&'paint mut ()>,
}

impl<'paint> HdcSurface<'paint> {
    pub(super) unsafe fn from_paint_hdc_with_swell(
        hdc: raw::HDC,
        _swell: rea_rs_low::Swell,
    ) -> Option<Self> {
        NonNull::new(hdc).map(|hdc| Self {
            hdc,
            _paint: PhantomData,
        })
    }

    fn raw(&self) -> raw::HDC {
        self.hdc.as_ptr()
    }

    /// Copies the already-erased paint background into a LICE bitmap before
    /// LICE rendering. This is the only HDC drawing support used by custom
    /// rendering; primitives are drawn through [`LiceSurface`].
    pub(super) fn copy_background_to_bitmap(
        &mut self,
        destination: &LiceBitmap,
        rect: Rect,
    ) -> ReaperResult<()> {
        let bitmap_dc =
            unsafe { destination.low.LICE__GetDC(destination.raw()) };
        if bitmap_dc.is_null() {
            return Err(ReaRsError::UnexpectedAPI(
                "LICE bitmap does not expose a compatible HDC".into(),
            ));
        }
        unsafe {
            destination.swell.BitBlt(
                bitmap_dc,
                native_coord(rect.x),
                native_coord(rect.y),
                native_coord(rect.width),
                native_coord(rect.height),
                self.raw(),
                native_coord(rect.x),
                native_coord(rect.y),
                raw::SRCCOPY as i32,
            );
        }
        Ok(())
    }

    pub(super) fn blit_bitmap(
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
