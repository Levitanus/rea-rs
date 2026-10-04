use super::{
    events::{EventRegistry, ScrollViewEvent, ScrollViewEventSource},
    layout::{
        self, Align, Axis, LayoutItem, LayoutOutput, OverflowPolicy, Panel,
        Rect, WidgetSize,
    },
    scroll::{ScrollMetrics, ScrollOffset, ScrollState, ScrollbarRenderer},
    widgets::{
        Button, CheckBox, ComboBox, ControlHandle, ControlId, ControlKind,
        ControlRect, ControlRegistry, EditField, GroupBox, ListBox,
        ReaperControl, StaticLabel,
    },
};
use crate::{ptr_wrappers::Hwnd, ReaRsError, Reaper, ReaperResult};
use rea_rs_low::raw;
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::Cell,
    cell::RefCell,
    collections::HashMap,
    ffi::CString,
    ptr::NonNull,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

static CONTAINER_WINDOW_PROCS: OnceLock<Mutex<HashMap<usize, isize>>> =
    OnceLock::new();

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
}

/// A SWELL/Win32 window handle.
pub struct ReaperWindow {
    pub(super) hwnd: Hwnd,
    pub(super) owned: bool,
    pub(crate) show_on_register: bool,
    pub(super) floating_rect: Cell<Option<raw::RECT>>,
    pub(super) docked: Cell<bool>,
    pub(super) dock_ident: Option<String>,
    pub(super) controls: RefCell<ControlRegistry>,
    pub(super) events: RefCell<EventRegistry>,
    scroll_views: RefCell<HashMap<usize, ScrollViewRuntime>>,
    layout: RefCell<WindowLayout>,
}

#[derive(Clone, Copy)]
struct LayoutEntry {
    id: ControlId,
    size: WidgetSize,
}

struct LayoutNode {
    entries: Vec<LayoutEntry>,
    axis: Axis,
    spacing: u32,
    policy: OverflowPolicy,
}

struct WindowLayout {
    root: LayoutNode,
    groups: HashMap<ControlId, LayoutNode>,
    structural: HashMap<ControlId, raw::HWND>,
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

pub struct LayoutPanel<'a> {
    window: &'a ReaperWindow,
    parent: raw::HWND,
    axis: Axis,
    container: Option<ControlId>,
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

struct ScrollViewRuntime {
    id: ControlId,
    content: raw::HWND,
    state: Rc<RefCell<ScrollState>>,
}

const SCROLLBAR_THICKNESS: u32 = 16;

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

    pub fn content(&self) -> LayoutPanel<'a> {
        LayoutPanel {
            window: self.window,
            parent: self.content,
            axis: Axis::Y,
            container: None,
        }
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

pub type LayoutContainer<'a> = LayoutPanel<'a>;

impl<'a> LayoutPanel<'a> {
    fn rect(size: WidgetSize) -> ControlRect {
        size.into()
    }

    pub fn create_button(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> ReaperResult<Button> {
        let control = self.window.create_button_in(
            self.parent,
            id,
            label,
            Self::rect(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    pub fn create_edit_field(
        &self,
        id: ControlId,
        size: WidgetSize,
        flags: i32,
    ) -> ReaperResult<EditField> {
        let control = self.window.create_edit_field_in(
            self.parent,
            id,
            Self::rect(size),
            flags,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    pub fn create_checkbox(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> ReaperResult<CheckBox> {
        let control = self.window.create_checkbox_in(
            self.parent,
            id,
            label,
            Self::rect(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    pub fn create_group_box(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> ReaperResult<LayoutContainer<'a>> {
        let _group = self.window.create_group_box_in(
            self.parent,
            id,
            label,
            Self::rect(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        self.window.layout.borrow_mut().groups.insert(
            id,
            LayoutNode {
                entries: Vec::new(),
                axis: Axis::Y,
                spacing: 8,
                policy: OverflowPolicy::Wrap,
            },
        );
        // GroupBox is a real native child window. Children therefore use the
        // GroupBox HWND as their parent and receive coordinates relative to
        // its client area.
        Ok(LayoutPanel {
            window: self.window,
            parent: _group.hwnd(),
            axis: Axis::Y,
            container: Some(id),
        })
    }

    pub fn create_label(
        &self,
        id: ControlId,
        label: &str,
        size: WidgetSize,
    ) -> ReaperResult<StaticLabel> {
        let control = self.window.create_label_in(
            self.parent,
            id,
            label,
            Self::rect(size),
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    pub fn create_combo_box(
        &self,
        id: ControlId,
        size: WidgetSize,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        let control = self.window.create_combo_box_in(
            self.parent,
            id,
            Self::rect(size),
            flags,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    pub fn create_list_box(
        &self,
        id: ControlId,
        size: WidgetSize,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        let control = self.window.create_list_box_in(
            self.parent,
            id,
            Self::rect(size),
            styles,
        )?;
        self.window.register_layout_entry(self.container, id, size);
        Ok(control)
    }

    /// Creates a viewport/content pair. Scrollbars are rendered by the
    /// viewport backend and no scrollbar child HWNDs are created.
    pub fn create_scroll_view(
        &self,
        id: ControlId,
        size: WidgetSize,
        renderer: ScrollbarRenderer,
    ) -> ReaperResult<ScrollView<'a>> {
        let rect = Self::rect(size);
        let view = self.window.create_structural_child(self.parent, rect)?;
        let content = self.window.create_structural_child(
            view,
            ControlRect::new(0, 0, rect.width, rect.height),
        )?;
        if renderer == ScrollbarRenderer::CoolSb
            && Reaper::get().low().supports_cool_scrollbars()
        {
            unsafe {
                Reaper::get().low().InitializeCoolSB(view);
            }
        }
        self.window.register_layout_entry(self.container, id, size);
        self.window.layout.borrow_mut().structural.insert(id, view);
        let state = Rc::new(RefCell::new(ScrollState::new()));
        self.window.scroll_views.borrow_mut().insert(
            view as usize,
            ScrollViewRuntime {
                id,
                content,
                state: Rc::clone(&state),
            },
        );
        Ok(ScrollView {
            window: self.window,
            id,
            view,
            content,
            state,
            renderer,
        })
    }
}

impl ReaperWindow {
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

    fn register_layout_entry(
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
    ) -> ReaperResult<Vec<Rect>> {
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
            node.policy,
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
        Ok(output
            .placements
            .into_iter()
            .map(|placement| placement.rect)
            .collect())
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
        let root_placements = self.apply_layout_node(bounds, &root)?;
        for (index, entry) in root.entries.iter().enumerate() {
            let Some(group) = layout.groups.get(&entry.id) else {
                continue;
            };
            let Some(group_rect) = root_placements.get(index).copied() else {
                continue;
            };
            let content = Rect::new(
                15,
                28,
                group_rect.width.saturating_sub(30),
                group_rect.height.saturating_sub(38),
            );
            self.apply_layout_node(content, group)?;
        }
        Ok(())
    }

    pub fn central_panel(&self) -> LayoutPanel<'_> {
        LayoutPanel {
            window: self,
            parent: self.hwnd(),
            axis: Panel::Central.axis(),
            container: None,
        }
    }

    pub fn panel(&self, panel: Panel) -> LayoutPanel<'_> {
        LayoutPanel {
            window: self,
            parent: self.hwnd(),
            axis: panel.axis(),
            container: None,
        }
    }
    pub(crate) fn owned(
        hwnd: raw::HWND,
        show_on_register: bool,
        dock_ident: String,
    ) -> ReaperResult<Self> {
        let hwnd = NonNull::new(hwnd).ok_or(ReaRsError::NullPtr("window"))?;
        Ok(Self {
            hwnd,
            owned: true,
            show_on_register,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: Some(dock_ident),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            layout: RefCell::new(WindowLayout::default()),
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
            owned: false,
            show_on_register: false,
            floating_rect: Cell::new(None),
            docked: Cell::new(false),
            dock_ident: dock_ident.into(),
            controls: RefCell::new(ControlRegistry::default()),
            events: RefCell::new(EventRegistry::default()),
            scroll_views: RefCell::new(HashMap::new()),
            layout: RefCell::new(WindowLayout::default()),
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
    fn create_control_handle(
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

    fn prepare_control_creation(&self, rect: ControlRect) -> ReaperResult<()> {
        self.check_window()?;
        if rect.width < 1 || rect.height < 1 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "invalid control size",
            ));
        }
        Ok(())
    }

    pub fn create_button(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<Button> {
        self.create_button_in(self.hwnd(), id, label, rect)
    }

    pub fn create_button_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<Button> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_button(
                parent,
                id.0,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(Button::new(self.create_control_handle(
            id,
            ControlKind::Button,
            hwnd,
        )?))
    }

    pub fn create_edit_field(
        &self,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<EditField> {
        self.create_edit_field_in(self.hwnd(), id, rect, flags)
    }

    pub fn create_edit_field_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<EditField> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_edit_field(
                parent,
                id.0,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                flags,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(EditField::new(self.create_control_handle(
            id,
            ControlKind::EditField,
            hwnd,
        )?))
    }

    pub fn create_label(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<StaticLabel> {
        self.create_label_in(self.hwnd(), id, label, rect)
    }

    pub fn create_label_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<StaticLabel> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_label(
                parent,
                id.0,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(StaticLabel::new(self.create_control_handle(
            id,
            ControlKind::Static,
            hwnd,
        )?))
    }

    pub fn create_checkbox(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<CheckBox> {
        self.create_checkbox_in(self.hwnd(), id, label, rect)
    }

    pub fn create_checkbox_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<CheckBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_checkbox(
                parent,
                id.0,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(CheckBox::new(self.create_control_handle(
            id,
            ControlKind::CheckBox,
            hwnd,
        )?))
    }

    pub fn create_group_box(
        &self,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<GroupBox> {
        self.create_group_box_in(self.hwnd(), id, label, rect)
    }

    pub fn create_group_box_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        label: &str,
        rect: ControlRect,
    ) -> ReaperResult<GroupBox> {
        self.prepare_control_creation(rect)?;
        let label = CString::new(label)?;
        let hwnd = unsafe {
            Self::swell()?.create_group_box(
                parent,
                id.0,
                label.as_ptr(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        self.install_container_event_proc(hwnd)?;
        Ok(GroupBox::new(self.create_control_handle(
            id,
            ControlKind::Static,
            hwnd,
        )?))
    }

    fn install_container_event_proc(
        &self,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        let swell = Self::swell()?;
        unsafe {
            let previous = swell.SetWindowLong(
                hwnd,
                raw::GWL_WNDPROC,
                container_event_proc as usize as isize,
            );
            CONTAINER_WINDOW_PROCS
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .expect("container window procedure registry poisoned")
                .insert(hwnd as usize, previous);
        }
        Ok(())
    }

    pub fn create_combo_box(
        &self,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        self.create_combo_box_in(self.hwnd(), id, rect, flags)
    }

    pub fn create_combo_box_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        rect: ControlRect,
        flags: i32,
    ) -> ReaperResult<ComboBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_combo_box(
                parent,
                id.0,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                flags,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(ComboBox::new(self.create_control_handle(
            id,
            ControlKind::ComboBox,
            hwnd,
        )?))
    }

    pub fn create_list_box(
        &self,
        id: ControlId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        self.create_list_box_in(self.hwnd(), id, rect, styles)
    }

    pub fn create_list_box_in(
        &self,
        parent: raw::HWND,
        id: ControlId,
        rect: ControlRect,
        styles: i32,
    ) -> ReaperResult<ListBox> {
        self.prepare_control_creation(rect)?;
        let hwnd = unsafe {
            Self::swell()?.create_list_box(
                parent,
                id.0,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                styles,
            )
        }
        .ok_or(ReaRsError::NullPtr("control"))?;
        Ok(ListBox::new(self.create_control_handle(
            id,
            ControlKind::ListBox,
            hwnd,
        )?))
    }
}

/// The procedure installed on owned windows.
pub(crate) unsafe extern "C" fn window_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if !Reaper::is_available() {
        return 0;
    }
    let reaper = Reaper::get_mut();
    let key = reaper.window_id_for_hwnd(hwnd).or_else(|| {
        // Child controls send WM_COMMAND to their immediate native parent.
        // A GroupBox is itself a native child, so commands from controls
        // inside it do not reach the top-level window procedure directly.
        // Walk the parent chain and route such messages to the owning window.
        if msg != raw::WM_COMMAND {
            return None;
        }
        let mut parent = Reaper::get().swell().GetParent(hwnd);
        while !parent.is_null() {
            if let Some(key) = reaper.window_id_for_hwnd(parent) {
                return Some(key);
            }
            parent = Reaper::get().swell().GetParent(parent);
        }
        None
    });
    let Some(key) = key else {
        log::warn!("window message for unregistered HWND {:p}", hwnd);
        return Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR;
    };
    let swell = reaper.swell() as *const rea_rs_low::Swell;
    match msg {
        raw::WM_CLOSE => {
            let allow = reaper
                .windows
                .get_mut(&key)
                .map(|handler| handler.on_close())
                .unwrap_or(false);
            if allow {
                unsafe {
                    reaper.swell().DestroyWindow(hwnd);
                }
            }
            1
        }
        raw::WM_DESTROY => {
            if let Some(low) =
                Reaper::is_available().then(|| Reaper::get().low())
            {
                if low.pointers().DockWindowRemove.is_some() {
                    unsafe {
                        low.DockWindowRemove(hwnd);
                    }
                }
            }
            if let Some(mut handler) = reaper.windows.remove(&key) {
                handler.on_destroy();
            }
            0
        }
        raw::WM_COMMAND => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let id = (wparam as usize & 0xffff) as i32;
            let code = ((wparam as usize >> 16) & 0xffff) as i32;
            if id == raw::IDCANCEL as i32 && lparam == 0 {
                let allow = handler.on_close();
                if allow {
                    unsafe {
                        reaper.swell().DestroyWindow(hwnd);
                    }
                }
                return 1;
            }
            let command = if lparam == 0 {
                super::events::WindowCommand::Menu { id }
            } else {
                super::events::WindowCommand::Control {
                    id,
                    notification: super::events::CommandNotification::from_raw(
                        code,
                    ),
                }
            };
            handler.on_command(command);
            if lparam != 0 {
                let control_id = super::widgets::ControlId(id);
                let event =
                    handler.window().control(control_id).and_then(|control| {
                        super::events::decode_control_event(
                            control.kind,
                            control_id,
                            super::events::CommandNotification::from_raw(code),
                        )
                    });
                if let Some(event) = event {
                    let result =
                        handler.window().events.borrow_mut().dispatch(event);
                    if matches!(
                        result,
                        super::events::DispatchResult::ForwardToWindow
                    ) {
                        handler.on_control_event(event);
                    }
                }
            }
            0
        }
        raw::WM_HSCROLL | raw::WM_VSCROLL => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if let Some(runtime) =
                handler.window().scroll_views.borrow().get(&(hwnd as usize))
            {
                let axis = if msg == raw::WM_HSCROLL {
                    Axis::X
                } else {
                    Axis::Y
                };
                let code = (wparam as usize & 0xffff) as i32;
                let position = ((wparam as usize >> 16) & 0xffff) as u32;
                if let Some(command) =
                    super::scroll::decode_scroll_command(code, position)
                {
                    let mut state = runtime.state.borrow_mut();
                    *state = state.apply(axis, command);
                    let offset = state.offset();
                    unsafe {
                        (*swell).SetWindowPos(
                            runtime.content,
                            std::ptr::null_mut(),
                            -(offset.x as i32),
                            -(offset.y as i32),
                            0,
                            0,
                            (raw::SWP_NOSIZE
                                | raw::SWP_NOZORDER
                                | raw::SWP_NOACTIVATE)
                                as i32,
                        );
                    }
                    let source = match command {
                        super::scroll::ScrollCommand::LineBackward
                        | super::scroll::ScrollCommand::LineForward => {
                            ScrollViewEventSource::Line
                        }
                        super::scroll::ScrollCommand::PageBackward
                        | super::scroll::ScrollCommand::PageForward => {
                            ScrollViewEventSource::Page
                        }
                        super::scroll::ScrollCommand::ThumbTrack(_)
                        | super::scroll::ScrollCommand::ThumbPosition(_) => {
                            ScrollViewEventSource::Thumb
                        }
                        _ => ScrollViewEventSource::Programmatic,
                    };
                    let event = ScrollViewEvent { offset, source };
                    let _ = handler
                        .window()
                        .emit_scroll_view_event(runtime.id, event);
                }
                return 0;
            }
            let control = handler
                .window()
                .controls
                .borrow()
                .get_by_hwnd(lparam as raw::HWND);
            if let Some(control) = control {
                let event = super::events::ControlEvent::Scroll {
                    control: control.id,
                    code: (wparam as usize & 0xffff) as i32,
                };
                if matches!(
                    handler.window().events.borrow_mut().dispatch(event),
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    handler.on_control_event(event);
                }
            }
            0
        }
        raw::WM_MOUSEWHEEL | raw::WM_MOUSEHWHEEL => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            let mut parent = hwnd;
            while !parent.is_null() {
                let runtime = handler
                    .window()
                    .scroll_views
                    .borrow()
                    .get(&(parent as usize))
                    .map(|runtime| {
                        (
                            runtime.id,
                            runtime.content,
                            Rc::clone(&runtime.state),
                        )
                    });
                if let Some((id, content, state)) = runtime {
                    let delta =
                        ((wparam as usize >> 16) & 0xffff) as i16 as i32;
                    let axis = if msg == raw::WM_MOUSEHWHEEL {
                        Axis::X
                    } else {
                        Axis::Y
                    };
                    let mut state = state.borrow_mut();
                    *state = state.scroll_wheel(axis, delta);
                    let offset = state.offset();
                    unsafe {
                        (*swell).SetWindowPos(
                            content,
                            std::ptr::null_mut(),
                            -(offset.x as i32),
                            -(offset.y as i32),
                            0,
                            0,
                            (raw::SWP_NOSIZE
                                | raw::SWP_NOZORDER
                                | raw::SWP_NOACTIVATE)
                                as i32,
                        );
                    }
                    let _ = handler.window().emit_scroll_view_event(
                        id,
                        ScrollViewEvent {
                            offset,
                            source: ScrollViewEventSource::Wheel,
                        },
                    );
                    return 0;
                }
                parent = (*swell).GetParent(parent);
            }
            0
        }
        raw::WM_NOTIFY => {
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if lparam == 0 {
                return 0;
            }
            let header = &*(lparam as *const super::events::NotifyHeader);
            let control_id = super::widgets::ControlId(header.id_from as i32);
            if handler.window().control(control_id).is_some() {
                let event = super::events::ControlEvent::Notified {
                    control: control_id,
                    code: header.code,
                };
                if matches!(
                    handler.window().events.borrow_mut().dispatch(event),
                    super::events::DispatchResult::ForwardToWindow
                ) {
                    handler.on_control_event(event);
                }
            }
            0
        }
        raw::WM_PAINT => {
            let mut paint = std::mem::zeroed();
            let hdc = reaper.swell().BeginPaint(hwnd, &mut paint);
            if !hdc.is_null() {
                reaper.swell().paint_window_background(hdc, &paint.rcPaint);
                reaper.swell().EndPaint(hwnd, &mut paint);
            }
            0
        }
        raw::WM_SIZE => {
            let mut rect = std::mem::zeroed();
            Reaper::get().swell().GetClientRect(hwnd, &mut rect);
            let Some(handler) = reaper.windows.get_mut(&key) else {
                return 0;
            };
            if let Err(error) = handler.window().apply_default_layout() {
                log::warn!("could not apply default window layout: {error}");
            }
            handler.on_resize(rect.right - rect.left, rect.bottom - rect.top);
            0
        }
        raw::WM_TIMER => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_timer(wparam as usize);
            }
            0
        }
        raw::WM_ACTIVATE => {
            if let Some(handler) = reaper.windows.get_mut(&key) {
                handler.on_activate((wparam as usize & 0xffff) != 0);
            }
            0
        }
        _ => Reaper::get()
            .swell()
            .DefWindowProc(hwnd, msg, wparam, lparam)
            as raw::INT_PTR,
    }
}

/// Forwards commands from controls whose immediate parent is a native
/// container (currently GroupBox) to the owning top-level window procedure.
/// Native child controls send `WM_COMMAND` to that immediate parent, so the
/// top-level window procedure cannot observe them unless the container is
/// subclassed.
unsafe extern "C" fn container_event_proc(
    hwnd: raw::HWND,
    msg: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if msg == raw::WM_COMMAND && Reaper::is_available() {
        let mut parent = Reaper::get().swell().GetParent(hwnd);
        while !parent.is_null() {
            if Reaper::get().window_id_for_hwnd(parent).is_some() {
                return window_proc(parent, msg, wparam, lparam);
            }
            parent = Reaper::get().swell().GetParent(parent);
        }
    }

    let previous = CONTAINER_WINDOW_PROCS.get().and_then(|registry| {
        registry.lock().ok()?.get(&(hwnd as usize)).copied()
    });
    if let Some(previous) = previous {
        let previous: unsafe extern "C" fn(
            raw::HWND,
            raw::UINT,
            raw::WPARAM,
            raw::LPARAM,
        ) -> raw::INT_PTR = std::mem::transmute(previous as usize);
        return previous(hwnd, msg, wparam, lparam);
    }

    Reaper::get()
        .swell()
        .DefWindowProc(hwnd, msg, wparam, lparam) as raw::INT_PTR
}
