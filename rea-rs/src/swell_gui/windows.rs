use super::{
    drawing::{
        HdcSurface, LiceBitmap, LiceBitmapKind, LiceSurface, PaintInfo,
    },
    events::{EventRegistry, ScrollViewEvent, ScrollViewEventSource},
    layout::{
        self, Align, Axis, Insets, LayoutItem, LayoutOutput, OverflowPolicy,
        Panel, PanelLayout, Rect, WidgetSize,
    },
    menu::Menu,
    scroll::{ScrollOffset, ScrollState, ScrollbarRenderer},
    widgets::{
        ControlHandle, ControlKind, ControlRect, ControlRegistry,
        ReaperControl, SwellId,
    },
};
use crate::{
    ptr_wrappers::{Hwnd, ReaperHwnd},
    swell_gui::{
        host_proc::{container_event_proc, CONTAINER_WINDOW_PROCS},
        widgets::CreationContext,
    },
    ReaRsError, Reaper, ReaperResult,
};
use rea_rs_low::raw;
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::Cell,
    cell::RefCell,
    collections::HashMap,
    ptr::NonNull,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

type RenderCallback = Box<
    dyn for<'surface> FnMut(
        &PaintInfo,
        &mut LiceSurface<'surface>,
    ) -> anyhow::Result<()>,
>;
type WidgetRenderCallback = Box<
    dyn for<'surface> FnMut(
        SwellId,
        &PaintInfo,
        &mut LiceSurface<'surface>,
    ) -> anyhow::Result<()>,
>;

pub type WindowId = String;

pub(super) const SHOW_MENU_POPUP_MESSAGE: raw::UINT = raw::WM_USER + 0x3A1;
static NEXT_POPUP_REQUEST: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct WindowPlacement {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl WindowPlacement {
    pub(super) fn from_rect(rect: raw::RECT) -> Option<Self> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 1 || height < 1 {
            return None;
        }
        Some(Self {
            left: rect.left,
            top: rect.top,
            width,
            height,
        })
    }
}

/// Position of a REAPER docker as reported by REAPER.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DockPosition {
    /// Docked at the bottom of the main window.
    Bottom = 0,
    /// Docked at the left of the main window.
    Left = 1,
    /// Docked at the top of the main window.
    Top = 2,
    /// Docked at the right of the main window.
    Right = 3,
    /// Not docked; shown as a floating window.
    Floating = 4,
}

impl TryFrom<i32> for DockPosition {
    type Error = ReaRsError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Bottom),
            1 => Ok(Self::Left),
            2 => Ok(Self::Top),
            3 => Ok(Self::Right),
            4 => Ok(Self::Floating),
            -1 => Err(ReaRsError::InvalidObject("docker not found")),
            _ => Err(ReaRsError::IntEnum(format!(
                "unknown docker position: {value}"
            ))),
        }
    }
}

/// Description of a new top-level REAPER window.
#[derive(Clone, Debug)]
pub struct WindowSpec {
    /// Initial title shown by the native window.
    pub title: String,
    /// Requested initial outer width in pixels.
    pub width: i32,
    /// Requested initial outer height in pixels.
    pub height: i32,
    /// Optional minimum outer width and height in pixels while resizing.
    /// `None` leaves the native default minimum unchanged.
    pub min_size: Option<(i32, i32)>,
    /// Whether the user can resize the window.
    pub resizable: bool,
    /// Whether the window omits its minimize button.
    pub no_minimize: bool,
    /// Whether the window omits its close button.
    pub no_close: bool,
    /// Stable logical identifier used for REAPER docking and saved placement.
    pub dock_ident: String,
    /// Whether registration may show the window automatically.
    pub allow_show: bool,
}

impl WindowSpec {
    /// Creates a window specification with standard resizable-window defaults.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        let dock_ident =
            format!("rea_rs_{}", title.to_lowercase().replace(' ', "_"));
        Self {
            title,
            width: 300,
            height: 200,
            min_size: None,
            resizable: true,
            no_minimize: false,
            no_close: false,
            dock_ident,
            allow_show: true,
        }
    }

    /// Sets the requested initial outer size in pixels; zero is raised to one.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width.max(1).min(i32::MAX as u32) as i32;
        self.height = height.max(1).min(i32::MAX as u32) as i32;
        self
    }

    /// Sets the minimum outer size while the user resizes the window.
    ///
    /// Width and height are in pixels and are clamped to the representable
    /// positive native range. The constraint is delivered through
    /// `WM_GETMINMAXINFO`; it affects user-driven resizing, not explicit
    /// programmatic positioning or docking geometry.
    pub fn min_size(mut self, width: u32, height: u32) -> Self {
        self.min_size = Some((
            width.max(1).min(i32::MAX as u32) as i32,
            height.max(1).min(i32::MAX as u32) as i32,
        ));
        self
    }

    /// Sets whether the window can be resized by the user.
    pub fn resizable(mut self, value: bool) -> Self {
        self.resizable = value;
        self
    }

    /// Sets whether the window omits its minimize button.
    pub fn no_minimize(mut self, value: bool) -> Self {
        self.no_minimize = value;
        self
    }

    /// Sets whether the window omits its close button.
    pub fn no_close(mut self, value: bool) -> Self {
        self.no_close = value;
        self
    }

    /// Sets the stable REAPER docker identifier used for saved placement.
    pub fn dock_ident(mut self, ident: impl Into<String>) -> Self {
        self.dock_ident = ident.into();
        self
    }

    /// Sets whether registration may show the window automatically.
    pub fn allow_show(mut self, value: bool) -> Self {
        self.allow_show = value;
        self
    }
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self::new("rea-rs window")
    }
}

#[cfg(test)]
mod window_spec_tests {
    use super::WindowSpec;

    #[test]
    fn minimum_size_is_optional_and_clamped_to_positive_dimensions() {
        assert_eq!(WindowSpec::new("test").min_size, None);
        let spec = WindowSpec::new("test").min_size(0, u32::MAX);
        assert_eq!(spec.min_size, Some((1, i32::MAX)));
    }
}

/// Callback interface for an owned REAPER window.
///
/// Callbacks are invoked by native window dispatch, normally on REAPER's UI
/// thread. They should return promptly and avoid holding mutable REAPER state
/// across nested native message loops. Default implementations ignore events;
/// `on_close` permits closing and event callbacks report whether they handled
/// the event. Callback panics are not converted into errors by this trait.
pub trait WindowHandler: 'static {
    /// Returns the stable logical identity used to register this window.
    fn window_id(&self) -> WindowId;
    /// Returns the window wrapper managed by this handler.
    fn window(&self) -> &ReaperWindow;
    /// Called after the window is opened and registered for dispatch.
    fn on_open(&mut self) {}
    /// Called when native close is requested. Return `true` to allow closing.
    fn on_close(&mut self) -> bool {
        true
    }
    /// Called during final destruction, after the window ceases to be usable.
    fn on_destroy(&mut self) {}
    /// Called for a decoded menu command.
    fn on_command(&mut self, _command: super::events::WindowCommand) {}
    /// Called for a decoded native or virtual control notification.
    fn on_control_event(&mut self, _event: super::events::ControlEvent) {}
    /// Called after the client area changes size; dimensions are client
    /// pixels.
    fn on_resize(&mut self, _width: i32, _height: i32) {}
    /// Called when native activation changes; `active` is the new state.
    fn on_activate(&mut self, _active: bool) {}
    /// Called for a `WM_TIMER` notification. `id` is the native timer ID.
    fn on_timer(&mut self, _id: SwellId) {}
    /// Called for general window input. Return `true` to mark it handled.
    fn on_event(&mut self, _event: super::events::WindowEvent) -> bool {
        false
    }
    /// Called for input routed to a child widget. Return `true` if handled.
    fn on_widget_event(
        &mut self,
        _id: SwellId,
        _event: super::events::WindowEvent,
    ) -> bool {
        false
    }
    /// Called when a message is offered to the host-message hook.
    /// Return `true` to claim handling; otherwise native dispatch continues.
    fn handle_host_message(&self, _message: u32) -> bool {
        false
    }
}

/// A SWELL/Win32 window handle.
pub struct ReaperWindow {
    pub(super) hwnd: Hwnd,
    pub(super) owned: Cell<bool>,
    pub(crate) show_on_register: bool,
    pub(super) min_size: Option<(i32, i32)>,
    pub(super) floating_rect: Cell<Option<raw::RECT>>,
    pub(super) docked: Cell<bool>,
    pub(super) dock_ident: Option<String>,
    pub(super) controls: RefCell<ControlRegistry>,
    pub(super) events: RefCell<EventRegistry>,
    pub(super) scroll_views: RefCell<HashMap<usize, ScrollViewRuntime>>,
    pub(super) menu: RefCell<Option<Menu>>,
    pub(super) popup_request: Cell<Option<(usize, i32, i32, usize)>>,
    pub(super) lifecycle_generation: Cell<usize>,
    pub(super) layout: RefCell<WindowLayout>,
    pub(super) render_bitmap: RefCell<Option<LiceBitmap>>,
    pub(super) render_callback: RefCell<Option<RenderCallback>>,
    pub(super) widget_render_callback: RefCell<Option<WidgetRenderCallback>>,
    pub(super) virtual_hosts:
        RefCell<HashMap<SwellId, rea_rs_low::VirtualControlHost>>,
    pub(super) virtual_command_queue:
        Rc<RefCell<Vec<(i32, isize, isize, i32)>>>,
}

#[derive(Clone, Copy)]
pub(super) struct LayoutEntry {
    pub(super) id: SwellId,
    pub(super) size: WidgetSize,
}

#[derive(Clone, Copy)]
pub(super) struct PanelLayoutEntry {
    pub(super) layout_index: usize,
    pub(super) entry: LayoutEntry,
}

pub(super) struct LayoutNode {
    pub(super) entries: Vec<LayoutEntry>,
    pub(super) axis: Axis,
    pub(super) spacing: u32,
    pub(super) policy: OverflowPolicy,
    pub(super) insets: Insets,
}

pub(super) struct WindowLayout {
    pub(super) root: LayoutNode,
    pub(super) groups: HashMap<SwellId, LayoutNode>,
    pub(super) structural: HashMap<SwellId, raw::HWND>,
    pub(super) virtual_controls: HashMap<SwellId, rea_rs_low::VirtualControl>,
    pub(super) virtual_parents: HashMap<SwellId, SwellId>,
    pub(super) panel_layout: Option<PanelLayout>,
    pub(super) panel_entries: HashMap<Panel, Vec<PanelLayoutEntry>>,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            root: LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::WrapScroll,
                insets: Insets::default(),
            },
            groups: HashMap::new(),
            structural: HashMap::new(),
            virtual_controls: HashMap::new(),
            virtual_parents: HashMap::new(),
            panel_layout: None,
            panel_entries: HashMap::new(),
        }
    }
}

/// A three-HWND scrolling container. `view` owns the scrollbars, `clip` is the
/// fixed client-area viewport, and `content` is the oversized translated
/// child that contains the widgets.
pub struct ScrollView<'a> {
    window: &'a ReaperWindow,
    id: SwellId,
    view: raw::HWND,
    clip: raw::HWND,
    scrollbar_hwnd: raw::HWND,
    content: raw::HWND,
    state: Rc<RefCell<ScrollState>>,
    renderer: ScrollbarRenderer,
    viewport: Cell<super::layout::Size>,
}

pub(super) struct ScrollViewRuntime {
    pub(super) id: SwellId,
    pub(super) scrollbar_hwnd: raw::HWND,
    pub(super) view: raw::HWND,
    pub(super) clip: raw::HWND,
    pub(super) content: raw::HWND,
    pub(super) state: Rc<RefCell<ScrollState>>,
    pub(super) renderer: ScrollbarRenderer,
}

/// Private guard for the one BeginPaint/EndPaint pair owned by WM_PAINT.
pub(super) struct PaintTransaction {
    hwnd: raw::HWND,
    paint: raw::PAINTSTRUCT,
    hdc: raw::HDC,
    swell: rea_rs_low::Swell,
}

impl PaintTransaction {
    pub(super) fn begin(hwnd: raw::HWND) -> Option<Self> {
        let swell = *Reaper::get().swell();
        let mut paint = unsafe { std::mem::zeroed() };
        let hdc = unsafe { swell.BeginPaint(hwnd, &mut paint) };
        if hdc.is_null() {
            return None;
        }
        // Owned windows retain the standard SWELL/REAPER background behavior.
        unsafe { swell.paint_window_background(hdc, &paint.rcPaint) };
        Some(Self {
            hwnd,
            paint,
            hdc,
            swell,
        })
    }

    fn info(&self) -> PaintInfo {
        let mut client = raw::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { self.swell.GetClientRect(self.hwnd, &mut client) };
        PaintInfo {
            damage_rect: Rect::from(self.paint.rcPaint),
            client_rect: Rect::from(client),
        }
    }

    pub(super) fn surface(&mut self) -> Option<(PaintInfo, HdcSurface<'_>)> {
        let info = self.info();
        unsafe { HdcSurface::from_paint_hdc_with_swell(self.hdc, self.swell) }
            .map(|surface| (info, surface))
    }
}

impl Drop for PaintTransaction {
    fn drop(&mut self) {
        unsafe { self.swell.EndPaint(self.hwnd, &mut self.paint) };
    }
}

/// Runs a handler's custom paint callback against a retained LICE bitmap and
/// presents the result to the active paint HDC. The HDC is used only for
/// preserving SWELL's pre-painted background and copying the completed image.
pub(super) fn render_with_lice(
    window: &ReaperWindow,
    info: &PaintInfo,
    hdc: &mut HdcSurface<'_>,
    widget: Option<SwellId>,
) {
    let width = info.client_rect.width;
    let height = info.client_rect.height;
    if width == 0 || height == 0 {
        return;
    }

    let has_callback = match widget {
        Some(_) => window.widget_render_callback.borrow().is_some(),
        None => window.render_callback.borrow().is_some(),
    } || widget.is_some_and(|id| {
        window
            .virtual_hosts
            .try_borrow()
            .is_ok_and(|hosts| hosts.contains_key(&id))
            || window.layout.borrow().structural.keys().any(|canvas| {
                window
                    .virtual_hosts
                    .try_borrow()
                    .is_ok_and(|hosts| hosts.contains_key(canvas))
            })
    });
    if !has_callback {
        return;
    }

    // Move the optional bitmap out so its RefCell borrow does not overlap the
    // callback's mutable borrow. It is restored whether drawing succeeds.
    let mut bitmap = window.render_bitmap.borrow_mut().take();
    let result = (|| -> ReaperResult<()> {
        if let Some(existing) = bitmap.as_mut() {
            if existing.width() != width || existing.height() != height {
                existing.resize(width, height)?;
            }
        } else {
            bitmap =
                Some(LiceBitmap::new(LiceBitmapKind::System, width, height)?);
        }
        let target = bitmap.as_mut().expect("bitmap created above");
        hdc.copy_background_to_bitmap(target, info.damage_rect)?;
        {
            let mut surface = target.surface();
            match widget {
                Some(id) => {
                    if let Some(callback) =
                        window.widget_render_callback.borrow_mut().as_mut()
                    {
                        if let Err(e) = callback(id, info, &mut surface) {
                            return Err(ReaRsError::UnderlyingError(e));
                        };
                    }
                }
                None => {
                    if let Some(callback) =
                        window.render_callback.borrow_mut().as_mut()
                    {
                        if let Err(e) = callback(info, &mut surface) {
                            return Err(ReaRsError::UnderlyingError(e));
                        }
                    }
                }
            }
            if let Some(id) = widget {
                let canvas = window
                    .layout
                    .borrow()
                    .virtual_parents
                    .get(&id)
                    .copied()
                    .unwrap_or(id);
                if let Ok(mut hosts) = window.virtual_hosts.try_borrow_mut() {
                    if let Some(host) = hosts.get_mut(&canvas) {
                        let dimensions = info.client_rect;
                        unsafe {
                            host.paint(
                                surface.raw_bitmap(),
                                dimensions.width as i32,
                                dimensions.height as i32,
                                [
                                    info.damage_rect.x as i32,
                                    info.damage_rect.y as i32,
                                    (info.damage_rect.x
                                        + info.damage_rect.width)
                                        as i32,
                                    (info.damage_rect.y
                                        + info.damage_rect.height)
                                        as i32,
                                ],
                            );
                        }
                    }
                }
            }
        }
        hdc.blit_bitmap(
            target,
            super::layout::Point {
                x: info.damage_rect.x,
                y: info.damage_rect.y,
            },
            info.damage_rect,
        )
    })();
    window.render_bitmap.replace(bitmap);
    if let Err(error) = result {
        log::warn!("LICE window rendering failed: {error}");
    }
}

impl<'a> ScrollView<'a> {
    /// Returns the logical control ID assigned to this viewport.
    pub fn id(&self) -> SwellId {
        self.id
    }

    /// Returns the raw HWND that owns the scrollbars and viewport.
    pub fn hwnd(&self) -> raw::HWND {
        self.view
    }

    /// Returns the translated child HWND that contains created widgets.
    pub fn content_hwnd(&self) -> raw::HWND {
        self.content
    }

    /// Returns the renderer selected at creation time.
    pub fn renderer(&self) -> ScrollbarRenderer {
        self.renderer
    }

    /// Returns a copy of the current pure scroll state.
    pub fn state(&self) -> ScrollState {
        *self.state.borrow()
    }

    /// Sets the virtual content extent in pixels and clamps the current
    /// offset.
    pub fn set_content_size(&self, size: super::layout::Size) {
        let state = {
            let mut state = self.state.borrow_mut();
            *state = state.set_content(size);
            *state
        };
        self.move_content(state.offset());
        self.sync_scrollbars();
    }

    /// Sets the requested viewport dimensions in pixels.
    pub fn set_viewport_size(&self, size: super::layout::Size) {
        self.viewport.set(size);
        let mut state = self.state.borrow_mut();
        *state = state.set_viewport(size);
        drop(state);
        self.sync_scrollbars();
    }

    fn set_layout_sizes(
        &self,
        viewport: super::layout::Size,
        content: super::layout::Size,
    ) {
        self.viewport.set(viewport);
        {
            let mut state = self.state.borrow_mut();
            *state = state.set_viewport(viewport).set_content(content);
        }
        let mut effective_viewport = viewport;
        for _ in 0..3 {
            self.sync_scrollbars();
            let measured = self.client_size();
            let mut state = self.state.borrow_mut();
            *state = state.set_effective_viewport(measured);
            drop(state);
            if measured == effective_viewport {
                effective_viewport = measured;
                break;
            }
            effective_viewport = measured;
        }
        let state = *self.state.borrow();
        let client = self.client_size();
        let content = super::layout::Size {
            x: content.x.max(client.x),
            y: content.y.max(client.y),
        };
        let state = state.set_content(content);
        let offset = state.offset();
        unsafe {
            let _ = Reaper::get().swell().SetWindowPos(
                self.clip,
                std::ptr::null_mut(),
                0,
                0,
                client.x.max(1).min(i32::MAX as u32) as i32,
                client.y.max(1).min(i32::MAX as u32) as i32,
                (raw::SWP_NOZORDER | raw::SWP_NOACTIVATE | raw::SWP_NOREDRAW)
                    as i32,
            );
            let _ = Reaper::get().swell().SetWindowPos(
                self.content,
                std::ptr::null_mut(),
                -(offset.x.min(i32::MAX as u32) as i32),
                -(offset.y.min(i32::MAX as u32) as i32),
                content.x.max(1).min(i32::MAX as u32) as i32,
                content.y.max(1).min(i32::MAX as u32) as i32,
                (raw::SWP_NOZORDER | raw::SWP_NOACTIVATE | raw::SWP_NOREDRAW)
                    as i32,
            );
            let _ = Reaper::get().swell().InvalidateRect(
                self.clip,
                std::ptr::null(),
                1,
            );
        }
        *self.state.borrow_mut() =
            state.set_effective_viewport(effective_viewport);
        self.sync_scrollbars();
    }

    fn client_size(&self) -> super::layout::Size {
        let mut rect = raw::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { Reaper::get().swell().GetClientRect(self.view, &mut rect) };
        super::layout::Size {
            x: (rect.right - rect.left).max(0) as u32,
            y: (rect.bottom - rect.top).max(0) as u32,
        }
    }

    pub(super) fn sync_scrollbars(&self) {
        let Some(reaper) = Reaper::is_available().then(Reaper::get) else {
            return;
        };
        let bar = |horizontal| {
            if horizontal {
                raw::SB_HORZ
            } else {
                raw::SB_VERT
            }
        };
        let apply = |state: ScrollState, page: super::layout::Size| unsafe {
            let update = |which: i32,
                          visible: bool,
                          position: u32,
                          page: u32,
                          max: u32| {
                if self.renderer == ScrollbarRenderer::CoolSb
                    && reaper.low().supports_cool_scrollbars()
                {
                    let mut info = raw::SCROLLINFO {
                        cbSize: std::mem::size_of::<raw::SCROLLINFO>() as u32,
                        fMask: (raw::SIF_RANGE | raw::SIF_PAGE | raw::SIF_POS)
                            as u32,
                        nMin: 0,
                        nMax: max as i32,
                        nPage: page,
                        nPos: position as i32,
                        nTrackPos: 0,
                    };
                    reaper.low().CoolSB_SetScrollInfo(
                        self.scrollbar_hwnd,
                        which,
                        &mut info,
                        1,
                    );
                    reaper.low().CoolSB_ShowScrollBar(
                        self.scrollbar_hwnd,
                        which,
                        visible as i8,
                    );
                } else if self.renderer == ScrollbarRenderer::Native {
                    let mut info = raw::SCROLLINFO {
                        cbSize: std::mem::size_of::<raw::SCROLLINFO>() as u32,
                        fMask: (raw::SIF_RANGE | raw::SIF_PAGE | raw::SIF_POS)
                            as u32,
                        nMin: 0,
                        nMax: max as i32,
                        nPage: page,
                        nPos: position as i32,
                        nTrackPos: 0,
                    };
                    let _ = reaper.swell().set_native_scrollbar(
                        self.scrollbar_hwnd,
                        which,
                        &mut info,
                        visible,
                    );
                }
            };
            update(
                bar(true) as i32,
                state.content().x > page.x,
                state.offset().x,
                page.x,
                state.content().x.saturating_sub(1),
            );
            update(
                bar(false) as i32,
                state.content().y > page.y,
                state.offset().y,
                page.y,
                state.content().y.saturating_sub(1),
            );
        };
        let mut state = *self.state.borrow();
        let mut page = state.viewport();
        for _ in 0..3 {
            apply(state, page);
            let measured = self.client_size();
            state = state.set_effective_viewport(measured);
            *self.state.borrow_mut() = state;
            if measured == page {
                break;
            }
            page = measured;
        }
        apply(state, page);
        log::trace!(
            "scrollbar sync: renderer={:?} view={:?} scrollbar_host={:?} viewport={:?} client={:?} content={:?} offset={:?} visible=({}, {})",
            self.renderer,
            self.view,
            self.scrollbar_hwnd,
            self.viewport.get(),
            page,
            state.content(),
            state.offset(),
            state.content().x > page.x,
            state.content().y > page.y,
        );
    }

    /// Scrolls by a signed pixel delta, clamps the offset, and emits a
    /// programmatic scroll event.
    pub fn scroll_by(&self, axis: Axis, delta: i32) -> ScrollViewEvent {
        let mut state = self.state.borrow_mut();
        *state = state.scroll_by(axis, delta);
        self.move_content(state.offset());
        self.sync_scrollbars();
        let event = ScrollViewEvent {
            offset: state.offset(),
            source: ScrollViewEventSource::Programmatic,
        };
        let _ = self.window.emit_scroll_view_event(self.id, event);
        event
    }

    fn move_content(&self, offset: ScrollOffset) {
        unsafe {
            Reaper::get().swell().SetWindowPos(
                self.content,
                std::ptr::null_mut(),
                -(offset.x as i32),
                -(offset.y as i32),
                0,
                0,
                (raw::SWP_NOSIZE
                    | raw::SWP_NOZORDER
                    | raw::SWP_NOACTIVATE
                    | raw::SWP_NOREDRAW) as i32,
            );
            let _ = Reaper::get().swell().InvalidateRect(
                self.clip,
                std::ptr::null(),
                1,
            );
        }
    }
}

impl ReaperWindow {
    pub(super) fn sync_scroll_view(&self, view: raw::HWND) {
        let runtime =
            self.scroll_views
                .borrow()
                .get(&(view as usize))
                .map(|runtime| {
                    (
                        runtime.id,
                        runtime.view,
                        runtime.clip,
                        runtime.scrollbar_hwnd,
                        runtime.content,
                        Rc::clone(&runtime.state),
                        runtime.renderer,
                    )
                });
        if let Some((
            id,
            view,
            clip,
            scrollbar_hwnd,
            content,
            state,
            renderer,
        )) = runtime
        {
            ScrollView {
                window: self,
                id,
                view,
                clip,
                scrollbar_hwnd,
                content,
                state,
                renderer,
                viewport: Cell::new(super::layout::Size { x: 0, y: 0 }),
            }
            .sync_scrollbars();
        }
    }

    /// Replaces the window's attached menu bar. Passing `None` detaches and
    /// releases the currently owned menu.
    pub fn set_menu_bar(&self, menu: Option<Menu>) -> ReaperResult<()> {
        self.check_window()?;
        let handle = menu.as_ref().map_or(std::ptr::null_mut(), Menu::handle);
        let result =
            unsafe { Reaper::get().swell().SetMenu(self.hwnd(), handle) };
        if result == 0 {
            return Err(ReaRsError::UnexpectedAPI("SetMenu failed".into()));
        }
        self.menu.replace(menu);
        unsafe {
            Reaper::get().swell().DrawMenuBar(self.hwnd());
        }
        Ok(())
    }

    /// Shows a popup at screen coordinates and returns the selected item ID.
    pub fn popup_menu_at(
        &self,
        menu: &Menu,
        x: i32,
        y: i32,
    ) -> ReaperResult<Option<super::widgets::SwellId>> {
        self.check_window()?;
        Ok(menu.popup_at(self.hwnd(), x, y))
    }

    /// Shows the currently attached menu bar as a popup and returns the
    /// selected command. This runs a nested native message loop; do not call
    /// it from a window callback that holds mutable access to REAPER state.
    pub fn popup_menu_bar_at(
        &self,
        x: i32,
        y: i32,
    ) -> ReaperResult<Option<super::widgets::SwellId>> {
        self.check_window()?;
        let menu = self.menu.borrow();
        let Some(menu) = menu.as_ref() else {
            return Ok(None);
        };
        Ok(menu.popup_at(self.hwnd(), x, y))
    }

    /// Queues the attached menu bar to be shown as a popup at screen
    /// coordinates. Unlike `popup_menu_bar_at`, this runs after the current
    /// window callback has returned, avoiding modal-menu reentrancy into it.
    pub fn post_popup_menu_bar_at(&self, x: i32, y: i32) -> ReaperResult<()> {
        self.check_window()?;
        let request = NEXT_POPUP_REQUEST.fetch_add(1, Ordering::Relaxed);
        self.popup_request.set(Some((
            request,
            x,
            y,
            self.lifecycle_generation.get(),
        )));
        let ok = unsafe {
            Reaper::get().swell().PostMessage(
                self.hwnd(),
                SHOW_MENU_POPUP_MESSAGE,
                request,
                0,
            )
        };
        if ok == 0 {
            self.popup_request.set(None);
            Err(ReaRsError::UnsuccessfulOperation("PostMessage"))
        } else {
            Ok(())
        }
    }

    pub(super) fn dispatch_virtual_commands(
        &self,
    ) -> Vec<super::events::ControlEvent> {
        let commands =
            std::mem::take(&mut *self.virtual_command_queue.borrow_mut());
        commands
            .into_iter()
            .filter_map(|(command, _p1, p2, source_id)| {
                let id = SwellId(source_id as u32);
                let control =
                    self.layout.borrow().virtual_controls.get(&id).copied()?;
                if control.control_kind()
                    == rea_rs_low::VirtualControlKind::Slider
                {
                    if command == raw::WM_HSCROLL as i32
                        || command == raw::WM_VSCROLL as i32
                    {
                        control.set_value(p2 as i32);
                    }
                }
                super::events::decode_virtual_event(
                    control.control_kind(),
                    id,
                    command,
                )
            })
            .collect()
    }

    /// Registers or replaces the custom LICE renderer for this window.
    ///
    /// The retained bitmap is allocated lazily. Rendering occurs during native
    /// paint dispatch; callback errors are logged and the failed frame is not
    /// presented. The surface is valid only for the callback invocation.
    pub fn on_render<F>(&self, callback: F) -> anyhow::Result<()>
    where
        F: for<'surface> FnMut(
                &PaintInfo,
                &mut LiceSurface<'surface>,
            ) -> anyhow::Result<()>
            + 'static,
    {
        self.check_window()?;
        self.render_callback.replace(Some(Box::new(callback)));
        self.invalidate_render_if_visible()?;
        Ok(())
    }

    /// Registers or replaces a LICE renderer for custom child widgets.
    /// The surface is valid only for the callback invocation.
    pub fn on_render_widget<F>(&self, callback: F) -> anyhow::Result<()>
    where
        F: for<'surface> FnMut(
                SwellId,
                &PaintInfo,
                &mut LiceSurface<'surface>,
            ) -> anyhow::Result<()>
            + 'static,
    {
        self.check_window()?;
        self.widget_render_callback
            .replace(Some(Box::new(callback)));
        self.invalidate_render_if_visible()?;
        Ok(())
    }

    fn invalidate_render_if_visible(&self) -> ReaperResult<()> {
        if Reaper::get().window_id_for_hwnd(self.hwnd()).is_some()
            && self.is_visible()?
        {
            self.invalidate(None)?;
        }
        Ok(())
    }

    /// Removes the custom renderers and releases the retained LICE bitmap and
    /// virtual control hosts.
    pub fn clear_render(&self) -> ReaperResult<()> {
        self.check_window()?;
        self.render_callback.borrow_mut().take();
        self.widget_render_callback.borrow_mut().take();
        self.render_bitmap.borrow_mut().take();
        self.virtual_hosts.borrow_mut().clear();
        self.layout.borrow_mut().virtual_controls.clear();
        self.layout.borrow_mut().virtual_parents.clear();
        self.invalidate(None)
    }

    /// Clears the current child UI and returns a fresh root creation context.
    /// The top-level window and its lifecycle remain intact.
    pub fn build_ui<'a>(&'a self) -> anyhow::Result<CreationContext<'a>> {
        self.reset_ui();
        Ok(CreationContext::new(self))
    }

    pub(super) fn reset_ui(&self) {
        let scroll_views: Vec<_> = self
            .scroll_views
            .borrow()
            .iter()
            .map(|(_, runtime)| (runtime.scrollbar_hwnd, runtime.renderer))
            .collect();
        let controls: Vec<_> = self
            .controls
            .borrow()
            .ids()
            .filter_map(|id| self.control(id))
            .collect();
        let structural: Vec<_> = self
            .layout
            .borrow()
            .structural
            .iter()
            .map(|(id, hwnd)| (*id, *hwnd))
            .collect();
        if Reaper::is_available() {
            let reaper = Reaper::get();
            for (scrollbar_hwnd, renderer) in &scroll_views {
                if *renderer == ScrollbarRenderer::CoolSb
                    && reaper.low().supports_cool_scrollbars()
                    && !scrollbar_hwnd.is_null()
                    && unsafe { reaper.swell().IsWindow(*scrollbar_hwnd) }
                {
                    unsafe {
                        let _ =
                            reaper.low().UninitializeCoolSB(*scrollbar_hwnd);
                    }
                }
            }
            let mut hwnds: Vec<_> =
                controls.iter().map(|control| control.hwnd).collect();
            hwnds.extend(structural.iter().map(|(_, hwnd)| *hwnd));
            for runtime in self.scroll_views.borrow().values() {
                if runtime.content != self.hwnd() {
                    hwnds.push(runtime.content);
                }
            }
            hwnds.sort_unstable_by_key(|hwnd| *hwnd as usize);
            hwnds.dedup();
            unsafe {
                for hwnd in hwnds {
                    if !hwnd.is_null() && reaper.swell().IsWindow(hwnd) {
                        let _ = reaper.swell().DestroyWindow(hwnd);
                    }
                }
            }
        }
        for control in controls {
            self.unregister_control(control.id);
        }
        for (id, _) in structural {
            self.layout.borrow_mut().structural.remove(&id);
        }
        self.clear_controls();
    }
    pub(crate) fn destroy_structural_children(&self) {
        if !Reaper::is_available() {
            return;
        }
        let mut children: Vec<_> =
            self.layout.borrow().structural.values().copied().collect();
        children.sort_unstable_by_key(|hwnd| *hwnd as usize);
        children.dedup();
        for child in children {
            if unsafe { Reaper::get().swell().IsWindow(child) } {
                unsafe { Reaper::get().swell().DestroyWindow(child) };
            }
        }
    }

    fn register_widget_container(
        &self,
        container: Option<SwellId>,
        id: SwellId,
    ) {
        if let Some(container) = container {
            self.events.borrow_mut().set_direct_container(id, container);
        }
    }

    pub(super) fn install_panel_layout(
        &self,
        panel_layout: PanelLayout,
    ) -> ReaperResult<()> {
        let mut layout = self.layout.borrow_mut();
        if layout.panel_layout.is_some() {
            return Err(ReaRsError::UnsuccessfulOperation(
                "a panel layout is already installed",
            ));
        }
        layout.panel_layout = Some(panel_layout);
        Ok(())
    }

    pub(super) fn update_panel_layout(
        &self,
        update: impl FnOnce(&mut PanelLayout),
    ) -> ReaperResult<()> {
        let mut layout = self.layout.borrow_mut();
        let panel_layout = layout.panel_layout.as_mut().ok_or(
            ReaRsError::UnsuccessfulOperation(
                "this window has no managed panel layout",
            ),
        )?;
        update(panel_layout);
        Ok(())
    }

    pub(super) fn register_layout_entry(
        &self,
        container: Option<SwellId>,
        id: SwellId,
        size: WidgetSize,
    ) {
        let mut layout = self.layout.borrow_mut();
        let node = match container {
            Some(container) => layout.groups.get_mut(&container),
            None => Some(&mut layout.root),
        };
        if let Some(node) = node {
            node.entries.push(LayoutEntry { id, size });
        }
        self.register_widget_container(container, id);
    }

    pub(super) fn register_panel_layout_entry(
        &self,
        panel: Panel,
        id: SwellId,
        size: WidgetSize,
    ) {
        let mut layout = self.layout.borrow_mut();
        let layout_index =
            layout.panel_entries.get(&panel).map_or(0, Vec::len);
        let panel_layout =
            layout.panel_layout.get_or_insert_with(PanelLayout::new);
        let panel_items = panel_layout.items_mut_for_creation(panel);
        if layout_index < panel_items.len() {
            panel_items[layout_index].size = size;
        } else {
            panel_items.push(LayoutItem { size });
        }
        layout.panel_entries.entry(panel).or_default().push(
            PanelLayoutEntry {
                layout_index,
                entry: LayoutEntry { id, size },
            },
        );
    }

    #[cfg(test)]
    pub(super) fn panel_layout_entry_count(&self, panel: Panel) -> usize {
        self.layout
            .borrow()
            .panel_entries
            .get(&panel)
            .map_or(0, Vec::len)
    }

    #[cfg(test)]
    pub(super) fn container_layout_entry_count(&self, id: SwellId) -> usize {
        self.layout
            .borrow()
            .groups
            .get(&id)
            .map_or(0, |node| node.entries.len())
    }

    fn apply_layout_node(
        &self,
        bounds: Rect,
        node: &LayoutNode,
        policy: OverflowPolicy,
    ) -> ReaperResult<LayoutOutput> {
        let items: Vec<_> = node
            .entries
            .iter()
            .map(|entry| LayoutItem { size: entry.size })
            .collect();
        let output = layout::layout_flow(
            bounds,
            node.axis,
            &items,
            node.spacing,
            policy,
        );
        for placement in &output.placements {
            if let Some(entry) = node.entries.get(placement.index) {
                if let Some(control) = self.control(entry.id) {
                    ReaperControl::new(control).set_rect(ControlRect::new(
                        placement.rect.x as i32,
                        placement.rect.y as i32,
                        placement.rect.width.max(1) as i32,
                        placement.rect.height.max(1) as i32,
                    ))?;
                } else if let Some(hwnd) =
                    self.layout.borrow().structural.get(&entry.id).copied()
                {
                    unsafe {
                        Self::swell()?.SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            placement.rect.x as i32,
                            placement.rect.y as i32,
                            placement.rect.width.max(1) as i32,
                            placement.rect.height.max(1) as i32,
                            raw::SWP_NOZORDER as i32,
                        );
                    }
                } else if let Some(control) = self
                    .layout
                    .borrow()
                    .virtual_controls
                    .get(&entry.id)
                    .copied()
                {
                    control.set_rect(
                        placement.rect.x as i32,
                        placement.rect.y as i32,
                        placement.rect.width.max(1) as i32,
                        placement.rect.height.max(1) as i32,
                    );
                }
            }
        }
        Ok(output)
    }

    fn apply_container_layout(
        &self,
        id: SwellId,
        rect: Rect,
        layout: &WindowLayout,
        parent_wraps: bool,
    ) -> ReaperResult<super::layout::Size> {
        let Some(node) = layout.groups.get(&id) else {
            return Ok(rect.size());
        };
        let insets = node.insets;
        let scroll_view = self
            .scroll_views
            .borrow()
            .values()
            .find(|runtime| runtime.id == id)
            .map(|runtime| runtime.view);
        let explicit_single_line =
            node.axis == Axis::X && node.policy == OverflowPolicy::Clip;
        let wraps = if explicit_single_line {
            false
        } else if scroll_view.is_some() {
            rect.width > rect.height
        } else {
            parent_wraps
        };
        let policy = if explicit_single_line {
            OverflowPolicy::Clip
        } else if wraps {
            if scroll_view.is_some() {
                OverflowPolicy::WrapScroll
            } else {
                OverflowPolicy::Wrap
            }
        } else {
            OverflowPolicy::Scroll
        };
        let is_structural = layout.structural.contains_key(&id);
        let bounds = insets.apply(Rect::new(0, 0, rect.width, rect.height));
        // A ScrollView wraps against its viewport; nested containers may
        // extend farther than their declared size, so their extents are
        // included below when sizing the scrolling content window.
        let output = self.apply_layout_node(bounds, node, policy)?;
        let mut content_extent = super::layout::Size {
            x: output
                .content_extent
                .x
                .max(bounds.width)
                .saturating_add(insets.horizontal()),
            y: output
                .content_extent
                .y
                .max(bounds.height)
                .saturating_add(insets.vertical()),
        };
        for placement in output.placements {
            let Some(entry) = node.entries.get(placement.index) else {
                continue;
            };
            if layout.groups.contains_key(&entry.id) {
                let child_size = self.apply_container_layout(
                    entry.id,
                    placement.rect,
                    layout,
                    wraps,
                )?;
                content_extent.x = content_extent
                    .x
                    .max(placement.rect.x.saturating_add(child_size.x));
                content_extent.y = content_extent
                    .y
                    .max(placement.rect.y.saturating_add(child_size.y));
            }
        }
        if let Some(view) = scroll_view {
            if let Some((
                runtime_id,
                clip_hwnd,
                content_hwnd,
                scrollbar_hwnd,
                state,
                renderer,
            )) = self.scroll_views.borrow().get(&(view as usize)).map(
                |runtime| {
                    (
                        runtime.id,
                        runtime.clip,
                        runtime.content,
                        runtime.scrollbar_hwnd,
                        Rc::clone(&runtime.state),
                        runtime.renderer,
                    )
                },
            ) {
                let mut view_rect = raw::RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                let mut content_rect = view_rect;
                unsafe {
                    Self::swell()?.GetWindowRect(view, &mut view_rect);
                    Self::swell()?
                        .GetWindowRect(content_hwnd, &mut content_rect);
                }
                log::trace!(
                    "scroll layout allocation: window={:?} id={runtime_id:?} parent={:?} view={view:?} view_rect={view_rect:?} requested_viewport={:?} content={content_hwnd:?} content_rect_before={content_rect:?} measured_content_extent={content_extent:?} renderer={renderer:?}",
                    self.hwnd(),
                    unsafe { Self::swell()?.GetParent(view) },
                    rect.size(),
                );
                let initial_state = *state.borrow();
                ScrollView {
                    window: self,
                    id: runtime_id,
                    view,
                    clip: clip_hwnd,
                    scrollbar_hwnd,
                    content: content_hwnd,
                    state,
                    renderer,
                    viewport: Cell::new(rect.size()),
                }
                .set_layout_sizes(rect.size(), content_extent);
                let mut view_rect = raw::RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                let mut content_rect = view_rect;
                unsafe {
                    Self::swell()?.GetWindowRect(view, &mut view_rect);
                    Self::swell()?
                        .GetWindowRect(content_hwnd, &mut content_rect);
                }
                let mut view_client_rect = raw::RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                unsafe {
                    Self::swell()?.GetClientRect(view, &mut view_client_rect);
                }
                let client_size = super::layout::Size {
                    x: (view_client_rect.right - view_client_rect.left).max(0)
                        as u32,
                    y: (view_client_rect.bottom - view_client_rect.top).max(0)
                        as u32,
                };
                log::trace!(
                    "scroll layout applied: window={:?} id={runtime_id:?} view={view:?} view_rect={view_rect:?} client={:?} content={content_hwnd:?} content_rect={content_rect:?} state={:?}",
                    self.hwnd(),
                    client_size,
                    initial_state,
                );
            }
            Ok(content_extent)
        } else if is_structural {
            // Explicit structural rows are single-line clipping viewports.
            // Their children may overflow horizontally, but that must not
            // resize the row and expose the overflow.
            Ok(rect.size())
        } else {
            // GroupBox child HWNDs clip their descendants, so expand the
            // native group to contain wrapped child lanes. Its parent (the
            // ScrollView) then incorporates this size into its scroll range.
            let required = content_extent;
            if required.x > rect.width || required.y > rect.height {
                if let Some(control) = self.control(id) {
                    ReaperControl::new(control).set_rect(ControlRect::new(
                        rect.x as i32,
                        rect.y as i32,
                        required.x.max(rect.width).min(i32::MAX as u32) as i32,
                        required.y.max(rect.height).min(i32::MAX as u32)
                            as i32,
                    ))?;
                }
            }
            Ok(super::layout::Size {
                x: required.x.max(rect.width),
                y: required.y.max(rect.height),
            })
        }
    }

    pub(super) fn create_structural_child(
        &self,
        parent: raw::HWND,
        rect: ControlRect,
    ) -> ReaperResult<raw::HWND> {
        self.create_structural_child_with_style(parent, rect, 0)
    }

    pub(super) fn create_structural_child_with_style(
        &self,
        parent: raw::HWND,
        rect: ControlRect,
        extra_style: i32,
    ) -> ReaperResult<raw::HWND> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_child_window_with_style(
                parent,
                rect.width,
                rect.height,
                Some(super::window_proc),
                0,
                extra_style,
            )
        }
        .ok_or(ReaRsError::NullPtr("structural child"))?;
        unsafe {
            Self::swell()?.SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                raw::SWP_NOZORDER as i32,
            );
            Self::swell()?.ShowWindow(hwnd, raw::SW_SHOW);
        }
        Ok(hwnd)
    }

    pub(crate) fn apply_default_layout(&self) -> ReaperResult<()> {
        let mut client = raw::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            Self::swell()?.GetClientRect(self.hwnd(), &mut client);
        }
        let client = Rect::from(client);
        log::trace!(
            "applying window layout: hwnd={:?} client={}x{} entries={}",
            self.hwnd(),
            client.width,
            client.height,
            self.layout.borrow().root.entries.len(),
        );
        let bounds = self.layout.borrow().root.insets.apply(Rect::new(
            0,
            0,
            client.width,
            client.height,
        ));
        let layout = self.layout.borrow();
        let root = LayoutNode {
            entries: layout.root.entries.clone(),
            axis: layout.root.axis,
            spacing: layout.root.spacing,
            policy: layout.root.policy,
            insets: layout.root.insets,
        };
        let output = self.apply_layout_node(bounds, &root, root.policy)?;
        for placement in output.placements {
            if let Some(entry) = root.entries.get(placement.index) {
                self.apply_container_layout(
                    entry.id,
                    placement.rect,
                    &layout,
                    false,
                )?;
            }
        }
        let panel_layout = layout.panel_layout.clone();
        let panel_entries = layout.panel_entries.clone();
        drop(layout);
        if let Some(panel_layout) = panel_layout {
            let panel_rects = panel_layout.allocate(Rect::new(
                0,
                0,
                client.width,
                client.height,
            ));
            for panel in Panel::ALL {
                let Some(panel_bounds) = panel_rects.get(panel) else {
                    continue;
                };
                let registered =
                    panel_entries.get(&panel).cloned().unwrap_or_default();
                let output = layout::layout_flow(
                    panel_layout.insets.apply(panel_bounds),
                    panel.axis(),
                    panel_layout.items(panel),
                    8,
                    OverflowPolicy::WrapScroll,
                );
                let layout = self.layout.borrow();
                for placement in output.placements {
                    let Some(panel_entry) = registered
                        .iter()
                        .find(|entry| entry.layout_index == placement.index)
                    else {
                        continue;
                    };
                    let entry = panel_entry.entry;
                    let rect = placement.rect;
                    if let Some(control) = self.control(entry.id) {
                        ReaperControl::new(control).set_rect(
                            ControlRect::new(
                                rect.x as i32,
                                rect.y as i32,
                                rect.width.max(1) as i32,
                                rect.height.max(1) as i32,
                            ),
                        )?;
                    } else if let Some(hwnd) =
                        layout.structural.get(&entry.id).copied()
                    {
                        unsafe {
                            Self::swell()?.SetWindowPos(
                                hwnd,
                                std::ptr::null_mut(),
                                rect.x as i32,
                                rect.y as i32,
                                rect.width.max(1) as i32,
                                rect.height.max(1) as i32,
                                raw::SWP_NOZORDER as i32,
                            );
                        }
                    } else if let Some(control) =
                        layout.virtual_controls.get(&entry.id).copied()
                    {
                        control.set_rect(
                            rect.x as i32,
                            rect.y as i32,
                            rect.width.max(1) as i32,
                            rect.height.max(1) as i32,
                        );
                    }
                    self.apply_container_layout(
                        entry.id, rect, &layout, false,
                    )?;
                }
                for item_index in 0..panel_layout.items(panel).len() {
                    if registered
                        .iter()
                        .any(|entry| entry.layout_index == item_index)
                    {
                        continue;
                    }
                    log::warn!(
                        "panel layout contains an item without a CreationContext widget: panel={panel:?} index={item_index}; use panel(panel).button/control factories to create a native child"
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn owned(
        hwnd: raw::HWND,
        show_on_register: bool,
        dock_ident: String,
        min_size: Option<(i32, i32)>,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        Ok(Self {
            hwnd,
            owned: Cell::new(true),
            show_on_register,
            min_size,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: Some(dock_ident),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            menu: RefCell::new(None),
            popup_request: Cell::new(None),
            lifecycle_generation: Cell::new(0),
            layout: RefCell::new(WindowLayout::default()),
            render_bitmap: RefCell::new(None),
            render_callback: RefCell::new(None),
            widget_render_callback: RefCell::new(None),
            virtual_hosts: RefCell::new(HashMap::new()),
            virtual_command_queue: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// Wraps a valid existing HWND without taking ownership of it.
    ///
    /// The wrapper validates the handle at construction, but the native window
    /// can later be destroyed or recreated. Operations re-check validity.
    pub fn from_hwnd(hwnd: raw::HWND) -> ReaperResult<Self> {
        Self::from_hwnd_with_dock_ident(ReaperHwnd::from_raw(hwnd), None)
    }

    /// Wraps a deserialized or otherwise retained non-owning HWND token.
    ///
    /// Validation confirms only that the token names a current window at this
    /// instant; it does not transfer ownership or guarantee future validity.
    pub fn from_hwnd_token(hwnd: ReaperHwnd) -> ReaperResult<Self> {
        Self::from_hwnd_with_dock_ident(hwnd, None)
    }

    /// Wraps an existing window and associates it with a logical docker ID.
    ///
    /// The ID is needed when a borrowed wrapper is used to undock a window:
    /// the native HWND may have been recreated or reparented, while the
    /// persisted floating placement is keyed by the logical docker ID.
    pub fn from_hwnd_with_dock_ident(
        hwnd: ReaperHwnd,
        dock_ident: impl Into<Option<String>>,
    ) -> ReaperResult<Self> {
        hwnd.validate()?;
        let hwnd = NonNull::new(hwnd.as_raw())
            .ok_or(ReaRsError::NullPtr("window"))?;
        Ok(Self {
            hwnd,
            owned: Cell::new(false),
            show_on_register: false,
            min_size: None,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: dock_ident.into(),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            menu: RefCell::new(None),
            popup_request: Cell::new(None),
            lifecycle_generation: Cell::new(0),
            layout: RefCell::new(WindowLayout::default()),
            render_bitmap: RefCell::new(None),
            render_callback: RefCell::new(None),
            widget_render_callback: RefCell::new(None),
            virtual_hosts: RefCell::new(HashMap::new()),
            virtual_command_queue: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// Registers a child control using its native SWELL/Win32 integer ID.
    pub(crate) fn register_control(
        &self,
        id: SwellId,
        kind: ControlKind,
        hwnd: raw::HWND,
    ) -> ReaperResult<ControlHandle> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        let handle = ControlHandle { id, kind, hwnd };
        self.controls.borrow_mut().register(handle);
        Ok(handle)
    }

    /// Updates a registered control after native HWND recreation/reparenting.
    pub(crate) fn rebind_control(
        &self,
        id: SwellId,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        self.controls.borrow_mut().rebind(id, hwnd)
    }

    pub(crate) fn unregister_control(
        &self,
        id: SwellId,
    ) -> Option<ControlHandle> {
        self.controls.borrow_mut().unregister(id)
    }

    pub(super) fn clear_controls(&self) {
        self.controls.borrow_mut().clear();
        self.events.borrow_mut().clear();
        self.scroll_views.borrow_mut().clear();
        let mut layout = self.layout.borrow_mut();
        layout.root.entries.clear();
        layout.groups.clear();
        layout.structural.clear();
        layout.panel_layout = None;
        layout.panel_entries.clear();
    }

    /// Reconnects registered logical controls with their current child HWNDs.
    ///
    /// SWELL may recreate or reparent native child windows during a docker
    /// transition. The logical ID and kind remain stable, so only the HWND
    /// reverse index needs to be refreshed.
    pub(super) fn rebind_controls(&self) -> ReaperResult<()> {
        self.check_window()?;
        let ids: Vec<_> = self.controls.borrow().ids().collect();
        for id in ids {
            let hwnd =
                unsafe { Self::swell()?.GetDlgItem(self.hwnd(), id.0 as i32) };
            if !hwnd.is_null() {
                self.rebind_control(id, hwnd)?;
            }
        }
        Ok(())
    }

    pub fn control(&self, id: SwellId) -> Option<ControlHandle> {
        self.controls.borrow().get(id)
    }

    /// Applies a pure flow layout to registered controls while preserving
    /// their stable `SwellId`s. The returned metadata can be used by a
    /// viewport or diagnostics overlay to inspect clipping and overflow.
    pub fn layout_controls(
        &self,
        bounds: Rect,
        axis: Axis,
        items: &[(SwellId, WidgetSize)],
        spacing: u32,
        policy: OverflowPolicy,
    ) -> ReaperResult<LayoutOutput> {
        let layout_items: Vec<_> = items
            .iter()
            .map(|(_, size)| LayoutItem { size: *size })
            .collect();
        let output =
            layout::layout_flow(bounds, axis, &layout_items, spacing, policy);
        for placement in &output.placements {
            let Some((id, _)) = items.get(placement.index) else {
                continue;
            };
            if let Some(control) = self.control(*id) {
                ReaperControl::new(control).set_rect(ControlRect::new(
                    placement.rect.x.min(i32::MAX as u32) as i32,
                    placement.rect.y.min(i32::MAX as u32) as i32,
                    placement.rect.width.min(i32::MAX as u32) as i32,
                    placement.rect.height.min(i32::MAX as u32) as i32,
                ))?;
            }
        }
        Ok(output)
    }

    /// Applies a horizontal row layout to registered controls.
    pub fn layout_row_controls(
        &self,
        bounds: Rect,
        items: &[(SwellId, WidgetSize)],
        spacing: u32,
        align_x: Align,
        align_y: Align,
    ) -> ReaperResult<LayoutOutput> {
        let layout_items: Vec<_> = items
            .iter()
            .map(|(_, size)| LayoutItem { size: *size })
            .collect();
        let output = layout::layout_row(
            bounds,
            &layout_items,
            spacing,
            align_x,
            align_y,
        );
        for placement in &output.placements {
            let Some((id, _)) = items.get(placement.index) else {
                continue;
            };
            if let Some(control) = self.control(*id) {
                ReaperControl::new(control).set_rect(ControlRect::new(
                    placement.rect.x.min(i32::MAX as u32) as i32,
                    placement.rect.y.min(i32::MAX as u32) as i32,
                    placement.rect.width.min(i32::MAX as u32) as i32,
                    placement.rect.height.min(i32::MAX as u32) as i32,
                ))?;
            }
        }
        Ok(output)
    }

    /// Creates a child control through SWELL's dialog-control factory.
    pub(super) fn create_control_handle(
        &self,
        id: SwellId,
        kind: ControlKind,
        hwnd: raw::HWND,
    ) -> ReaperResult<ControlHandle> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        self.register_control(id, kind, hwnd)
    }

    pub(super) fn prepare_control_creation(
        &self,
        rect: ControlRect,
    ) -> ReaperResult<()> {
        self.check_window()?;
        if rect.width < 1 || rect.height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid control size",
            ));
        }
        Ok(())
    }

    pub(super) fn install_container_event_proc(
        &self,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        let swell = Self::swell()?;
        unsafe {
            let previous = swell.SetWindowLong(
                hwnd,
                raw::GWL_WNDPROC,
                container_event_proc as *const () as usize as isize,
            );
            CONTAINER_WINDOW_PROCS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .expect("container window procedure registry poisoned")
                .insert(hwnd as usize, previous);
        }
        Ok(())
    }
}
