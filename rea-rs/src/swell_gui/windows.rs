use super::{
    drawing::{
        HdcSurface, LiceBitmap, LiceBitmapKind, LiceSurface, PaintInfo,
    },
    events::{EventRegistry, ScrollViewEvent, ScrollViewEventSource},
    layout::{
        self, Align, Axis, LayoutItem, LayoutOutput, OverflowPolicy, Rect,
        WidgetSize,
    },
    scroll::{ScrollMetrics, ScrollOffset, ScrollState, ScrollbarRenderer},
    widgets::{
        ControlHandle, ControlId, ControlKind, ControlRect, ControlRegistry,
        ReaperControl,
    },
};
use crate::{
    ptr_wrappers::Hwnd,
    swell_gui::{
        host_proc::{container_event_proc, CONTAINER_WINDOW_PROCS},
        widgets::CreationContext,
    },
    ReaRsError, Reaper, ReaperResult,
};
use rea_rs_low::raw;
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::Cell, cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc,
    sync::Mutex,
};

type RenderCallback = Box<
    dyn for<'surface> FnMut(
        &PaintInfo,
        &mut LiceSurface<'surface>,
    ) -> anyhow::Result<()>,
>;
type WidgetRenderCallback = Box<
    dyn for<'surface> FnMut(
        ControlId,
        &PaintInfo,
        &mut LiceSurface<'surface>,
    ) -> anyhow::Result<()>,
>;

pub type WindowId = String;

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

/// Position of a REAPER docker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DockPosition {
    Bottom = 0,
    Left = 1,
    Top = 2,
    Right = 3,
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
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub resizable: bool,
    pub no_minimize: bool,
    pub no_close: bool,
    pub dock_ident: String,
    pub allow_show: bool,
}

impl WindowSpec {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        let dock_ident =
            format!("rea_rs_{}", title.to_lowercase().replace(' ', "_"));
        Self {
            title,
            width: 300,
            height: 200,
            resizable: true,
            no_minimize: false,
            no_close: false,
            dock_ident,
            allow_show: true,
        }
    }

    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width.max(1) as i32;
        self.height = height.max(1) as i32;
        self
    }

    pub fn resizable(mut self, value: bool) -> Self {
        self.resizable = value;
        self
    }

    pub fn no_minimize(mut self, value: bool) -> Self {
        self.no_minimize = value;
        self
    }

    pub fn no_close(mut self, value: bool) -> Self {
        self.no_close = value;
        self
    }

    pub fn dock_ident(mut self, ident: impl Into<String>) -> Self {
        self.dock_ident = ident.into();
        self
    }

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

/// Callback interface for an owned REAPER window.
pub trait WindowHandler: 'static {
    fn window_id(&self) -> WindowId;
    fn window(&self) -> &ReaperWindow;
    fn on_open(&mut self) {}
    fn on_close(&mut self) -> bool {
        true
    }
    fn on_destroy(&mut self) {}
    fn on_command(&mut self, _command: super::events::WindowCommand) {}
    fn on_control_event(&mut self, _event: super::events::ControlEvent) {}
    fn on_resize(&mut self, _width: i32, _height: i32) {}
    fn on_activate(&mut self, _active: bool) {}
    fn on_timer(&mut self, _id: usize) {}
    fn on_event(&mut self, _event: super::events::WindowEvent) -> bool {
        false
    }
    fn on_widget_event(
        &mut self,
        _id: ControlId,
        _event: super::events::WindowEvent,
    ) -> bool {
        false
    }
    fn handle_host_message(&self, _message: u32) -> bool {
        false
    }
}

/// A SWELL/Win32 window handle.
pub struct ReaperWindow {
    pub(super) hwnd: Hwnd,
    pub(super) owned: Cell<bool>,
    pub(crate) show_on_register: bool,
    pub(super) floating_rect: Cell<Option<raw::RECT>>,
    pub(super) docked: Cell<bool>,
    pub(super) dock_ident: Option<String>,
    pub(super) controls: RefCell<ControlRegistry>,
    pub(super) events: RefCell<EventRegistry>,
    pub(super) scroll_views: RefCell<HashMap<usize, ScrollViewRuntime>>,
    pub(super) layout: RefCell<WindowLayout>,
    pub(super) render_bitmap: RefCell<Option<LiceBitmap>>,
    pub(super) render_callback: RefCell<Option<RenderCallback>>,
    pub(super) widget_render_callback: RefCell<Option<WidgetRenderCallback>>,
}

#[derive(Clone, Copy)]
pub(super) struct LayoutEntry {
    pub(super) id: ControlId,
    pub(super) size: WidgetSize,
}

pub(super) struct LayoutNode {
    pub(super) entries: Vec<LayoutEntry>,
    pub(super) axis: Axis,
    pub(super) spacing: u32,
    pub(super) policy: OverflowPolicy,
}

pub(super) struct WindowLayout {
    pub(super) root: LayoutNode,
    pub(super) groups: HashMap<ControlId, LayoutNode>,
    pub(super) structural: HashMap<ControlId, raw::HWND>,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            // The central panel is a flow container too.  When its vertical
            // space is exhausted, the next root item (including a GroupBox)
            // belongs in a new horizontal lane rather than being left
            // outside the panel's usable bounds.
            root: LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::WrapScroll,
            },
            groups: HashMap::new(),
            structural: HashMap::new(),
        }
    }
}

/// A two-HWND scrolling container. The viewport owns the native scrollbar
/// state; the content window is the parent for user controls.
pub struct ScrollView<'a> {
    window: &'a ReaperWindow,
    id: ControlId,
    view: raw::HWND,
    content: raw::HWND,
    state: Rc<RefCell<ScrollState>>,
    renderer: ScrollbarRenderer,
}

pub(super) struct ScrollViewRuntime {
    pub(super) id: ControlId,
    pub(super) content: raw::HWND,
    pub(super) state: Rc<RefCell<ScrollState>>,
    pub(super) renderer: ScrollbarRenderer,
}

const SCROLLBAR_THICKNESS: u32 = 16;

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
    widget: Option<ControlId>,
) {
    let width = info.client_rect.width;
    let height = info.client_rect.height;
    if width == 0 || height == 0 {
        return;
    }

    let has_callback = match widget {
        Some(_) => window.widget_render_callback.borrow().is_some(),
        None => window.render_callback.borrow().is_some(),
    };
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

impl<'a> Drop for ScrollView<'a> {
    fn drop(&mut self) {
        if self.renderer == ScrollbarRenderer::CoolSb
            && Reaper::is_available()
            && Reaper::get().low().supports_cool_scrollbars()
        {
            unsafe {
                let _ = Reaper::get().low().UninitializeCoolSB(self.view);
            }
        }
    }
}

impl<'a> ScrollView<'a> {
    pub fn id(&self) -> ControlId {
        self.id
    }

    pub fn hwnd(&self) -> raw::HWND {
        self.view
    }

    pub fn content_hwnd(&self) -> raw::HWND {
        self.content
    }

    pub fn renderer(&self) -> ScrollbarRenderer {
        self.renderer
    }

    pub fn state(&self) -> ScrollState {
        *self.state.borrow()
    }

    pub fn set_content_size(&self, size: super::layout::Size) {
        let mut state = self.state.borrow_mut();
        *state = state.set_content(size);
        self.move_content(state.offset());
        self.sync_scrollbars();
    }

    pub fn set_viewport_size(&self, size: super::layout::Size) {
        let mut state = self.state.borrow_mut();
        *state = state.set_viewport(size);
        self.sync_scrollbars();
    }

    fn set_layout_sizes(
        &self,
        viewport: super::layout::Size,
        content: super::layout::Size,
    ) {
        let state = {
            let mut state = self.state.borrow_mut();
            *state = state.set_viewport(viewport).set_content(content);
            *state
        };
        let offset = state.offset();
        unsafe {
            let _ = Reaper::get().swell().SetWindowPos(
                self.content,
                std::ptr::null_mut(),
                -(offset.x.min(i32::MAX as u32) as i32),
                -(offset.y.min(i32::MAX as u32) as i32),
                content.x.max(1).min(i32::MAX as u32) as i32,
                content.y.max(1).min(i32::MAX as u32) as i32,
                (raw::SWP_NOZORDER | raw::SWP_NOACTIVATE) as i32,
            );
        }
        self.sync_scrollbars();
    }

    fn sync_scrollbars(&self) {
        let (_, visibility) = ScrollMetrics::with_visibility(
            self.state.borrow().content(),
            self.state.borrow().viewport(),
            SCROLLBAR_THICKNESS,
        );
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
        let state = self.state.borrow();
        let metrics = state.metrics();
        let update = |which, visible, position, page, max| unsafe {
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
                reaper
                    .low()
                    .CoolSB_SetScrollInfo(self.view, which, &mut info, 1);
                reaper.low().CoolSB_ShowScrollBar(
                    self.view,
                    which,
                    visible as i8,
                );
            }
        };
        update(
            bar(true) as i32,
            visibility.horizontal,
            metrics.offset.x,
            metrics.viewport.x,
            metrics.max_offset.x,
        );
        update(
            bar(false) as i32,
            visibility.vertical,
            metrics.offset.y,
            metrics.viewport.y,
            metrics.max_offset.y,
        );
    }

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
                (raw::SWP_NOSIZE | raw::SWP_NOZORDER | raw::SWP_NOACTIVATE)
                    as i32,
            );
        }
    }
}

impl ReaperWindow {
    /// Registers or replaces the custom LICE renderer for this window.
    ///
    /// The LICE render bitmap is allocated lazily on the next paint.
    /// The callback's error is logged and the partially rendered bitmap is
    /// not presented.
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
    pub fn on_render_widget<F>(&self, callback: F) -> anyhow::Result<()>
    where
        F: for<'surface> FnMut(
                ControlId,
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

    /// Removes the custom renderers and releases the retained LICE bitmap.
    pub fn clear_render(&self) -> ReaperResult<()> {
        self.check_window()?;
        self.render_callback.borrow_mut().take();
        self.widget_render_callback.borrow_mut().take();
        self.render_bitmap.borrow_mut().take();
        self.invalidate(None)
    }

    /// Replaces the window's child UI by invoking `build` with a fresh root
    /// creation context. The top-level window and its lifecycle remain intact.
    pub fn build_ui<'a>(&'a self) -> anyhow::Result<CreationContext<'a>> {
        self.reset_ui();
        Ok(CreationContext::new(self))
    }

    pub(super) fn reset_ui(&self) {
        let scroll_views: Vec<_> = self
            .scroll_views
            .borrow()
            .iter()
            .map(|(view, runtime)| (*view as raw::HWND, runtime.renderer))
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
            for (view, renderer) in scroll_views {
                if renderer == ScrollbarRenderer::CoolSb
                    && reaper.low().supports_cool_scrollbars()
                    && !view.is_null()
                    && unsafe { reaper.swell().IsWindow(view) }
                {
                    unsafe {
                        let _ = reaper.low().UninitializeCoolSB(view);
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
        container: Option<ControlId>,
        id: ControlId,
    ) {
        if let Some(container) = container {
            self.events.borrow_mut().set_direct_container(
                id,
                super::widgets::ContainerId(container),
            );
        }
    }

    pub(super) fn register_layout_entry(
        &self,
        container: Option<ControlId>,
        id: ControlId,
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
                }
            }
        }
        Ok(output)
    }

    fn apply_container_layout(
        &self,
        id: ControlId,
        rect: Rect,
        layout: &WindowLayout,
        parent_wraps: bool,
    ) -> ReaperResult<super::layout::Size> {
        let Some(node) = layout.groups.get(&id) else {
            return Ok(rect.size());
        };
        let scroll_view = layout.structural.get(&id).copied();
        let wraps = if scroll_view.is_some() {
            rect.width > rect.height
        } else {
            parent_wraps
        };
        let policy = if wraps {
            if scroll_view.is_some() {
                OverflowPolicy::WrapScroll
            } else {
                OverflowPolicy::Wrap
            }
        } else {
            OverflowPolicy::Scroll
        };
        let bounds = if scroll_view.is_some() {
            Rect::new(0, 0, rect.width, rect.height)
        } else {
            Rect::new(
                15,
                28,
                rect.width.saturating_sub(30),
                rect.height.saturating_sub(38),
            )
        };
        // A ScrollView wraps against its viewport; nested containers may
        // extend farther than their declared size, so their extents are
        // included below when sizing the scrolling content window.
        let output = self.apply_layout_node(bounds, node, policy)?;
        let mut content_extent = super::layout::Size {
            x: output.content_extent.x.max(bounds.width),
            y: output.content_extent.y.max(bounds.height),
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
            if let Some((runtime_id, content_hwnd, state, renderer)) =
                self.scroll_views.borrow().get(&(view as usize)).map(
                    |runtime| {
                        (
                            runtime.id,
                            runtime.content,
                            Rc::clone(&runtime.state),
                            runtime.renderer,
                        )
                    },
                )
            {
                ScrollView {
                    window: self,
                    id: runtime_id,
                    view,
                    content: content_hwnd,
                    state,
                    renderer,
                }
                .set_layout_sizes(bounds.size(), content_extent);
            }
            Ok(content_extent)
        } else {
            // GroupBox child HWNDs clip their descendants, so expand the
            // native group to contain wrapped child lanes. Its parent (the
            // ScrollView) then incorporates this size into its scroll range.
            let required = super::layout::Size {
                x: content_extent.x.saturating_add(30),
                y: content_extent.y.saturating_add(38),
            };
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
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_child_window(
                parent,
                rect.width,
                rect.height,
                Some(super::window_proc),
                0,
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
        // Keep the root flow away from the native window edge.  GroupBox
        // frames draw their border and caption slightly outside their logical
        // content rectangle, so giving the root layout a margin prevents a
        // fill-sized GroupBox from touching or overflowing the window frame.
        let client = Rect::from(client);
        log::trace!(
            "applying window layout: hwnd={:?} client={}x{} entries={}",
            self.hwnd(),
            client.width,
            client.height,
            self.layout.borrow().root.entries.len(),
        );
        let bounds = Rect::new(
            20,
            20,
            client.width.saturating_sub(40),
            client.height.saturating_sub(40),
        );
        let layout = self.layout.borrow();
        let root = LayoutNode {
            entries: layout.root.entries.clone(),
            axis: layout.root.axis,
            spacing: layout.root.spacing,
            policy: layout.root.policy,
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
        Ok(())
    }

    pub(crate) fn owned(
        hwnd: raw::HWND,
        show_on_register: bool,
        dock_ident: String,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        Ok(Self {
            hwnd,
            owned: Cell::new(true),
            show_on_register,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: Some(dock_ident),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            layout: RefCell::new(WindowLayout::default()),
            render_bitmap: RefCell::new(None),
            render_callback: RefCell::new(None),
            widget_render_callback: RefCell::new(None),
        })
    }

    pub fn from_hwnd(hwnd: raw::HWND) -> ReaperResult<Self> {
        Self::from_hwnd_with_dock_ident(hwnd, None)
    }

    /// Wraps an existing window and associates it with a logical docker ID.
    ///
    /// The ID is needed when a borrowed wrapper is used to undock a window:
    /// the native HWND may have been recreated or reparented, while the
    /// persisted floating placement is keyed by the logical docker ID.
    pub fn from_hwnd_with_dock_ident(
        hwnd: raw::HWND,
        dock_ident: impl Into<Option<String>>,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        if !Reaper::is_available() {
            return Err(ReaRsError::InvalidObject(
                "Reaper is not initialized",
            ));
        }
        let valid = unsafe { Reaper::get().swell().IsWindow(hwnd.as_ptr()) };
        if !valid {
            return Err(ReaRsError::InvalidObject("window is not valid"));
        }
        Ok(Self {
            hwnd,
            owned: Cell::new(false),
            show_on_register: false,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: dock_ident.into(),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            layout: RefCell::new(WindowLayout::default()),
            render_bitmap: RefCell::new(None),
            render_callback: RefCell::new(None),
            widget_render_callback: RefCell::new(None),
        })
    }

    /// Registers a child control using its native SWELL/Win32 integer ID.
    pub(crate) fn register_control(
        &self,
        id: ControlId,
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
        id: ControlId,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        if hwnd.is_null() {
            return Err(ReaRsError::NullPtr("control"));
        }
        self.controls.borrow_mut().rebind(id, hwnd)
    }

    pub(crate) fn unregister_control(
        &self,
        id: ControlId,
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
            let hwnd = unsafe { Self::swell()?.GetDlgItem(self.hwnd(), id.0) };
            if !hwnd.is_null() {
                self.rebind_control(id, hwnd)?;
            }
        }
        Ok(())
    }

    pub fn control(&self, id: ControlId) -> Option<ControlHandle> {
        self.controls.borrow().get(id)
    }

    /// Applies a pure flow layout to registered controls while preserving
    /// their stable `ControlId`s. The returned metadata can be used by a
    /// viewport or diagnostics overlay to inspect clipping and overflow.
    pub fn layout_controls(
        &self,
        bounds: Rect,
        axis: Axis,
        items: &[(ControlId, WidgetSize)],
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
        items: &[(ControlId, WidgetSize)],
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
        id: ControlId,
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
