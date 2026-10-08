//! A complete `swell-gui` example for a REAPER extension.
//!
//! The module registers a small **Widget Gallery** action that opens a
//! dockable window. The window is intentionally more than a static widget
//! showcase: it demonstrates how a SWELL UI can stay synchronized with REAPER
//! state and how controls can update tracks through the high-level `rea-rs`
//! API.
//!
//! ## What the example demonstrates
//!
//! - Creating a [`ReaperWindow`] with [`WindowSpec`] and restoring its docked
//!   state with [`ExtState`].
//! - Building a layout with panels, a scroll view, a list box, a group box,
//!   radio buttons, a combo box, a trackbar, an edit field, and a progress
//!   bar.
//! - Registering widget callbacks and handling [`ControlEvent`] values in a
//!   [`WindowHandler`].
//! - Keeping programmatic control updates from being interpreted as user input
//!   by using the `programmatic_update` guard.
//! - Synchronizing a track list and inspector with the current REAPER project,
//!   including track name, mute/solo state, automation mode, and volume.
//! - Sending changes made by a [`ControlSurface`] to the UI through serialized
//!   [`ExtState`] events. This avoids directly sharing UI objects with
//!   REAPER's control-surface callback thread.
//! - Installing and removing a custom renderer on the active MIDI editor. The
//!   optional paint-over action draws a zoom-aware, clipped marker at measure
//!   2 and pitch C3.
//!
//! ## How to use it
//!
//! Call [`register_actions`] while initializing the extension:
//!
//! ```rust,no_run
//! # use anyhow::Result;
//! # use rea_rs::Reaper;
//! # use crate::swell_gui::register_actions;
//! # fn initialize() -> Result<()> {
//! register_actions(Reaper::get_mut())?;
//! # Ok(())
//! # }
//! ```
//!
//! The example registers two actions:
//!
//! - `TestSwellGuiWidgetGallery` toggles the gallery window.
//! - `TestSwellPaintOver` toggles the MIDI-editor overlay.
//!
//! Trigger the first action from REAPER's action list to open the gallery.
//! The gallery can be docked or floated from its menu, and its dock preference
//! is persisted under `rea-rs.window/rea_rs_widget_gallery.dock`.
//!
//! ## Event flow
//!
//! ```text
//! REAPER action -> DemoWindow -> widget events -> Track API
//!       ^                  |                    |
//!       |                  +-- timer ----------+
//!       |                       (drain ExtState queue)
//!       +-- DemoCSurf <- ExtState event queue <- control-surface callbacks
//! ```
//!
//! `DemoWindow` owns all UI controls and performs UI-thread synchronization.
//! `DemoCSurf` only observes REAPER/control-surface state and appends
//! [`DemoEvent`] values to the queue. When the window timer fires, it drains
//! the queue, rebuilds the list when necessary, and refreshes the inspector.
//!
//! ## Adapting this example
//!
//! For a smaller application, keep the same separation of responsibilities:
//! construct controls in `new`, handle user input in `on_control_event`,
//! update controls from REAPER in timer/event callbacks, and unregister timers
//! and renderers in `on_destroy` or `Drop`. Replace the gallery-specific track
//! and MIDI code with the state owned by the extension.
use log::{debug, info, trace, warn};
use rea_rs::{
    db_to_linear, linear_to_db,
    swell_gui::{
        layout::{PanelLayout, PanelSizes, WidgetSize},
        LiceCombineMode, LiceTextOptions, Menu, MenuItem, MouseButton,
        MouseMessage, WindowEvent,
    },
    ActionHook, ActionKind, AutomationMode, CheckBox, Color, ComboBox,
    ControlEvent, ControlSurface, EditField, EventResponse, ExtState, Font,
    FontSpec, KnowsProject, LiceFont, ListBox, Measure, Panel, ProgressBar,
    RadioButton, ReaRsError, Reaper, ReaperWindow, ScrollbarRenderer,
    SoloMode, Track, Trackbar, Volume, WindowHandler, WindowId, WindowSpec,
    WithReaperPtr,
};
use rea_rs_low::raw;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, sync::Arc, time::Duration};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";
const DOCK_STATE_KEY: &str = "rea_rs_widget_gallery.dock";
const MENU_DOCK: u32 = 0x7101;
const MENU_FLOAT: u32 = 0x7102;
const PREFS_PAGE_ID: &str = "rea_rs_widget_gallery";
const DEMO_SECTION: &str = "rea-rs.track-gallery";
const EVENT_QUEUE_KEY: &str = "events";
const PAINT_OVER_KEY: &str = "paint_over";
const DEMO_WINDOW_ID: &str = "rea-rs.track-gallery";
const HOST_WINDOW_ID: &str = "rea-rs.track-gallery.host";
const PAINT_OVER_ACTION: &str = "TestSwellPaintOver";
const PAINT_OVER_DESCRIPTION: &str = "test swell paint-over";
const CSURF_TYPE: &str = "REARSPAINTover";
const TIMER_ID: rea_rs::SwellId = rea_rs::SwellId(0x5241);
const TIMER_INTERVAL: Duration = Duration::from_millis(33);
const LIST_RESET_CONTENT: raw::UINT = 0x0184;
const MIDI_CANVAS_CHILD_INDEX: usize = 1;
const MIDI_C3_PITCH: i32 = 48;

#[derive(Clone, Debug, Serialize, Deserialize)]
enum DemoEvent {
    RebuildTracks,
    Selection { guid: String },
    Volume { guid: String, value: f64 },
    Mute { guid: String, value: bool },
    Solo { guid: String, value: bool },
    Title { guid: String, value: String },
}

#[derive(Default)]
struct DemoLifecycle {
    paint_over: bool,
}

fn window_registered() -> bool {
    Reaper::get().is_window_registered(&DEMO_WINDOW_ID.to_string())
}

fn read_paint_over() -> bool {
    ExtState::<bool, Reaper>::load_value(
        DEMO_SECTION,
        PAINT_OVER_KEY,
        Reaper::get(),
        None,
    )
    .ok()
    .flatten()
    .unwrap_or(false)
}

fn ensure_surface_registered() {
    let is_window_registered = window_registered();
    let paint_over = read_paint_over();
    let active = is_window_registered || paint_over;
    let reaper = Reaper::get_mut();
    if active {
        if !reaper.has_control_surface(&CSURF_TYPE.to_string()) {
            reaper.register_control_surface(Arc::new(RefCell::new(
                DemoCSurf::default(),
            )));
        }
    } else if reaper.has_control_surface(&CSURF_TYPE.to_string()) {
        let _ = reaper.unregister_control_surface(CSURF_TYPE.to_string());
    }
}

fn append_events(events: Vec<DemoEvent>) {
    if events.is_empty() || !window_registered() {
        return;
    }
    let result = (|| -> anyhow::Result<()> {
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let mut current: Vec<DemoEvent> = queue.get()?.unwrap_or_default();
        current.extend(events);
        queue.set(current)?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!("could not enqueue gallery event: {error}");
    }
}

struct PreferencesPage {
    window: ReaperWindow,
}

impl WindowHandler for PreferencesPage {
    fn window_id(&self) -> WindowId {
        "rea-rs.widget-gallery.preferences".to_string()
    }

    fn window(&self) -> &ReaperWindow {
        &self.window
    }

    fn handle_host_message(&self, message: u32) -> anyhow::Result<bool> {
        Ok(message == raw::WM_PAINT)
    }
}

#[derive(Default)]
struct DemoCSurf {
    editor_hwnd: RefCell<Option<usize>>,
    paint_over: RefCell<bool>,
    previous_host: RefCell<Option<ReaperWindow>>,
}

impl std::fmt::Debug for DemoCSurf {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter
            .debug_struct("DemoCSurf")
            .field("editor_hwnd", &self.editor_hwnd)
            .field("paint_over", &self.paint_over)
            .finish_non_exhaustive()
    }
}

impl DemoCSurf {
    fn push_track_event(
        &self,
        track: &mut Track,
        build: impl FnOnce(String) -> DemoEvent,
    ) -> anyhow::Result<()> {
        append_events(vec![build(track.guid()?.to_string()?)]);
        Ok(())
    }

    fn update_midi_editor_overlay(&self, editor_hwnd: Option<usize>) {
        self.remove_midi_editor_overlay();
        if !read_paint_over() {
            return;
        }
        let Some(hwnd) = editor_hwnd else {
            return;
        };
        let hwnd = hwnd as raw::HWND;
        if !unsafe { Reaper::get().swell().IsWindow(hwnd) } {
            log::warn!("MIDI overlay skipped: invalid editor HWND={hwnd:p}");
            return;
        }
        let Ok(editor) = ReaperWindow::from_hwnd(hwnd) else {
            log::warn!(
                "MIDI overlay skipped: could not wrap editor HWND={hwnd:p}"
            );
            return;
        };
        let window = select_midi_canvas(&editor).unwrap_or_else(|| {
            warn!("MIDI canvas child not found; falling back to editor root");
            ReaperWindow::from_hwnd(hwnd)
                .expect("the validated MIDI editor HWND remains valid")
        });
        let Some(midi_editor) = Reaper::get().active_midi_editor() else {
            warn!("MIDI overlay skipped: active editor wrapper unavailable");
            return;
        };
        let take = match midi_editor.get_active_take() {
            Ok(take) => take,
            Err(error) => {
                warn!(
                    "MIDI overlay skipped: could not get active take: {error}"
                );
                return;
            }
        };
        let measure_start = match Measure::from_index(2, &take.project()) {
            Ok(measure) => measure.start,
            Err(error) => {
                warn!("MIDI overlay skipped: could not locate measure 2: {error}");
                return;
            }
        };
        let initial_view = match midi_editor.view_transform() {
            Ok(view) => view,
            Err(error) => {
                warn!("MIDI overlay skipped: could not read editor view: {error}");
                return;
            }
        };
        let client_rect = match window.client_rect() {
            Ok(rect) => rect,
            Err(error) => {
                warn!("MIDI overlay skipped: could not read canvas size: {error}");
                return;
            }
        };
        let width =
            client_rect.right.saturating_sub(client_rect.left).max(0) as u32;
        let height =
            client_rect.bottom.saturating_sub(client_rect.top).max(0) as u32;
        let overlay_hwnd = window.hwnd();
        let font = match Font::new(FontSpec::new("Arial").set_size(12))
            .and_then(LiceFont::from_font)
        {
            Ok(font) => font,
            Err(error) => {
                warn!("could not create overlay font: {error}");
                return;
            }
        };
        let initial_horizontal_zoom = initial_view.horizontal_pixels_per_unit;
        let initial_pixels_per_pitch = initial_view.vertical_pixels_per_pitch;
        if let Err(error) = window.on_render(move |_info, surface| {
            // Re-read the transform on each paint so scroll/zoom changes move
            // the project-time anchor with the native MIDI content. Convert
            // project time to take PPQ after each paint so moving the item
            // doesn't leave the marker at its old item-relative position.
            let view = midi_editor.view_transform()?;
            let measure_start_ppq = measure_start.as_ppq(&take)? as f64;
            let x = view.x_for_ppq(measure_start_ppq, &take).ok();
            // This child is the piano-roll canvas; its client origin is the
            // note-row origin, not the top of the editor's ruler.
            let y = view.y_for_pitch(MIDI_C3_PITCH, 0.0).ok();
            if let (Some(x), Some(y)) = (x, y) {
                let width_scale =
                    view.horizontal_pixels_per_unit / initial_horizontal_zoom;
                let height_scale =
                    view.vertical_pixels_per_pitch / initial_pixels_per_pitch;
                draw_clipped_anchor_rect(
                    surface,
                    x,
                    y,
                    100.0 * width_scale,
                    view.vertical_pixels_per_pitch * height_scale,
                    width,
                    height,
                    &font,
                );
            }
            Ok(())
        }) {
            warn!("could not install MIDI editor renderer: {error}");
            return;
        }
        if let Err(error) = Reaper::get_mut()
            .register_window_handler(Box::new(DemoHostWindow { window }))
        {
            warn!("could not register MIDI editor overlay: {error}");
            return;
        }
        *self.previous_host.borrow_mut() =
            ReaperWindow::from_hwnd(overlay_hwnd).ok();
        if let Some(host) = self.previous_host.borrow().as_ref() {
            if let Err(error) = host.invalidate(None) {
                warn!("could not invalidate MIDI editor for overlay: {error}");
            }
        }
    }

    fn remove_midi_editor_overlay(&self) {
        if let Err(error) = Reaper::get_mut()
            .unregister_window_handler(&HOST_WINDOW_ID.to_string())
        {
            warn!("could not unregister MIDI editor overlay handler: {error}");
        }
        if let Some(window) = self.previous_host.borrow_mut().take() {
            let _ = window.invalidate(None);
        }
    }
}

fn select_midi_canvas(editor: &ReaperWindow) -> Option<ReaperWindow> {
    let mut children = match editor.children() {
        Ok(children) => children,
        Err(error) => {
            warn!("could not enumerate MIDI editor children: {error}");
            return None;
        }
    };
    let container = match children.nth(MIDI_CANVAS_CHILD_INDEX)? {
        Ok(container) => container,
        Err(error) => {
            warn!("MIDI editor canvas child became unavailable: {error}");
            return None;
        }
    };
    Some(container)
}

fn draw_clipped_anchor_rect(
    surface: &mut rea_rs::swell_gui::LiceSurface<'_>,
    anchor_x: f64,
    anchor_y: f64,
    width: f64,
    height: f64,
    client_width: u32,
    client_height: u32,
    font: &LiceFont,
) {
    use rea_rs::swell_gui::layout::Rect;

    // The point marks the left edge at the measure start and the vertical
    // center of C3's row. The box scales with horizontal/vertical zoom while
    // remaining clipped to the overlay child's client area.
    let left = anchor_x;
    let top = anchor_y - height / 2.0;
    let right = left + width;
    let bottom = top + height;
    let clipped_left = left.max(0.0);
    let clipped_top = top.max(0.0);
    let clipped_right = right.min(f64::from(client_width));
    let clipped_bottom = bottom.min(f64::from(client_height));
    if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
        return;
    }

    let rect = Rect::new(
        clipped_left.floor() as u32,
        clipped_top.floor() as u32,
        (clipped_right.ceil() - clipped_left.floor()) as u32,
        (clipped_bottom.ceil() - clipped_top.floor()) as u32,
    );
    surface.fill_rect(
        rect,
        Color::new(255, 40, 180),
        0.16,
        LiceCombineMode::Copy,
    );
    surface.draw_rect(
        rect,
        Color::new(255, 40, 180),
        1.0,
        LiceCombineMode::Copy,
    );
    let label_rect =
        Rect::new(rect.x, rect.y, rect.width.min(96), 18.min(rect.height));
    let _ = surface.draw_text(
        "M2 C3 anchor",
        label_rect,
        font,
        LiceTextOptions::default(),
    );
}

impl ControlSurface for DemoCSurf {
    fn get_type_string(&self) -> String {
        CSURF_TYPE.to_string()
    }
    fn get_desc_string(&self) -> String {
        "rea-rs track gallery".to_string()
    }

    fn run(&self) -> anyhow::Result<()> {
        let current = Reaper::get()
            .active_midi_editor()
            .map(|editor| editor.get_pointer().as_ptr() as usize);
        let paint_over = read_paint_over();
        let changed = current != *self.editor_hwnd.borrow()
            || paint_over != *self.paint_over.borrow();
        *self.editor_hwnd.borrow_mut() = current;
        *self.paint_over.borrow_mut() = paint_over;
        if changed {
            self.update_midi_editor_overlay(current);
        }
        Ok(())
    }

    fn set_track_list_change(&self) -> anyhow::Result<()> {
        append_events(vec![DemoEvent::RebuildTracks]);
        Ok(())
    }
    fn set_surface_volume(
        &self,
        track: &mut Track,
        value: f64,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Volume { guid, value })
    }
    fn set_surface_mute(
        &self,
        track: &mut Track,
        value: bool,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Mute { guid, value })
    }
    fn set_surface_solo(
        &self,
        track: &mut Track,
        value: bool,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Solo { guid, value })
    }
    fn set_surface_selected(
        &self,
        track: &mut Track,
        selected: bool,
    ) -> anyhow::Result<()> {
        if selected {
            self.on_track_selection(track)
        } else {
            Ok(())
        }
    }
    fn on_track_selection(&self, track: &mut Track) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Selection { guid })
    }
    fn set_track_title(
        &self,
        track: &mut Track,
        title: String,
    ) -> anyhow::Result<()> {
        self.push_track_event(track, |guid| DemoEvent::Title {
            guid,
            value: title,
        })
    }
}

// Keep all owned MIDI overlay resources scoped to the control surface. When
// paint-over is disabled and the gallery window is closed, unregistering the
// surface drops this value and therefore detaches the MIDI host handler.
impl Drop for DemoCSurf {
    fn drop(&mut self) {
        self.remove_midi_editor_overlay();
    }
}

struct DemoWindow {
    window: ReaperWindow,
    menu_manager: DynamicManager,
    paint_checkbox: CheckBox,
    cursor_position: ProgressBar,
    tracklist: ListBox,
    name: EditField,
    normal: RadioButton,
    mute: RadioButton,
    solo: RadioButton,
    automation: ComboBox,
    volume: Trackbar,
    volume_value: EditField,
    volume_dragging: bool,
    tracks: Vec<String>,
    selected_guid: Option<String>,
    programmatic_update: bool,
    paint_over: bool,
    timer: Option<rea_rs::SwellId>,
}

/// Keeps the gallery's attached menu synchronized with host dock state.
/// The same attached menu is displayed as a popup on right-click.
#[derive(Default)]
struct DynamicManager {
    docked: Option<bool>,
}

impl DynamicManager {
    fn refresh(&mut self, window: &ReaperWindow) -> anyhow::Result<()> {
        let docked = window.is_docked().unwrap_or(false);
        if self.docked == Some(docked) {
            return Ok(());
        }
        let menu = Menu::new([
            MenuItem::command(MENU_DOCK, "Dock gallery")
                .enabled(!docked)
                .checked(docked),
            MenuItem::command(MENU_FLOAT, "Float gallery")
                .enabled(docked)
                .checked(!docked),
        ])?;
        window.set_menu_bar(Some(menu))?;
        self.docked = Some(docked);
        Ok(())
    }
}

impl DemoWindow {
    fn new() -> anyhow::Result<Self> {
        let (width, height) = (400_u32, 620_u32);
        let window = Reaper::get().create_window(
            &WindowSpec::new("rea-rs track gallery")
                .size(width, height)
                .min_size(320, 240)
                .dock_ident("rea_rs_widget_gallery"),
        )?;
        trace!(
                "gallery window created: hwnd={:?} valid={} visible={} parent={:?}",
                window.hwnd(),
                unsafe { Reaper::get().swell().IsWindow(window.hwnd()) },
                unsafe { Reaper::get().swell().IsWindowVisible(window.hwnd()) },
                unsafe { Reaper::get().swell().GetParent(window.hwnd()) },
            );
        let ui = window.build_ui()?;
        let menu = Menu::new([
            MenuItem::command(MENU_DOCK, "Dock gallery"),
            MenuItem::command(MENU_FLOAT, "Float gallery"),
        ])?;
        window.set_menu_bar(Some(menu))?;
        let panels =
            ui.panel_layout(PanelLayout::new().with_sizes(PanelSizes {
                right: 120,
                ..PanelSizes::default()
            }))?;
        let switches = panels.panel(Panel::Right);
        let content = panels
            .panel(Panel::Central)
            .scroll_view(
                rea_rs::SwellId::new_control(),
                WidgetSize::new_fill_both(1, 1),
                ScrollbarRenderer::Auto,
            )?
            .with_insets(rea_rs::swell_gui::layout::Insets {
                left: 10,
                top: 10,
                right: 10,
                bottom: 10,
            });
        let paint_checkbox = switches.checkbox(
            rea_rs::SwellId(100),
            "Paint MIDI overlay",
            WidgetSize::new_fill_x(120, 28),
        )?;
        let button = switches.button(
            rea_rs::SwellId::new_control(),
            "Hello World",
            WidgetSize::new_fill_x(120, 28),
        )?;
        window.on_widget_event(button.id(), |event| {
            if let ControlEvent::ButtonClicked { control: _ } = event {
                info!("Hello World from test extension");
                let pr = Reaper::get().current_project();
                let cursor_pos = pr.get_cursor_position()?;

                info!(
                    "cursor position is: measure: {}, beat: {}, seconds: {}",
                    Measure::from_position(cursor_pos, &pr)?.index,
                    cursor_pos.as_quarters(&pr)?,
                    cursor_pos.as_secs_f64()
                );
            }
            Ok(EventResponse::Handled)
        });
        let error_button = switches.button(
            rea_rs::SwellId::new_control(),
            "Raise Error",
            WidgetSize::new_fill_x(120, 28),
        )?;
        window.on_widget_event(error_button.id(), |event| {
            if let ControlEvent::ButtonClicked { control: _ } = event {
                return Err(ReaRsError::Str(
                    "Error from widget gallery button.",
                )
                .into());
            }
            Ok(EventResponse::Handled)
        });

        let cursor_position = content.progress_bar(
            rea_rs::SwellId(101),
            WidgetSize::new_fill_x(width, 20).set_max_x(width),
            rea_rs::swell_gui::ProgressBarOptions::default(),
        )?;
        cursor_position.set_range(0, 1000)?;
        let tracklist = content.list_box(
            rea_rs::SwellId(102),
            WidgetSize::new_fill_both(width, 100)
                .set_min_y(90)
                .set_max_x(width),
            rea_rs::swell_gui::ListBoxOptions::NOTIFY,
        )?;
        let inspector = content.group_box(
            rea_rs::SwellId(103),
            "Track inspector",
            WidgetSize::new_fill_both(width, 100).set_max_x(width),
        )?;
        let name = inspector.edit_field(
            rea_rs::SwellId(104),
            WidgetSize::new_fill_x(380, 26).set_max_x(width),
            rea_rs::swell_gui::widgets::EditFieldOptions::default(),
        )?;
        let states = inspector.row(
            rea_rs::SwellId::new_control(),
            WidgetSize::new_fill_x(380, 28).set_max_x(width),
        )?;
        let normal = states.radio_button(
            rea_rs::SwellId(105),
            "Normal",
            WidgetSize::new(95, 24),
            rea_rs::swell_gui::RadioButtonOptions::default(),
        )?;
        let mute = states.radio_button(
            rea_rs::SwellId(106),
            "Mute",
            WidgetSize::new(85, 24),
            rea_rs::swell_gui::RadioButtonOptions::default(),
        )?;
        let solo = states.radio_button(
            rea_rs::SwellId(107),
            "Solo",
            WidgetSize::new(85, 24),
            rea_rs::swell_gui::RadioButtonOptions::default(),
        )?;
        let automation = inspector.combo_box(
            rea_rs::SwellId(108),
            WidgetSize::new_fill_x(380, 28).set_max_x(width),
            rea_rs::swell_gui::ComboBoxOptions::default(),
        )?;
        for mode in [
            AutomationMode::None,
            AutomationMode::TrimRead,
            AutomationMode::Read,
            AutomationMode::Touch,
            AutomationMode::Write,
            AutomationMode::Latch,
            AutomationMode::Bypass,
        ] {
            automation.add_item(automation_name(mode))?;
        }
        let volume_row = inspector.row(
            rea_rs::SwellId::new_control(),
            WidgetSize::new_fill_x(380, 32)
                .set_max_x(width)
                .set_min_x(200),
        )?;
        let volume = volume_row.trackbar(
            rea_rs::SwellId(109),
            WidgetSize::new_fill_x(220, 28).set_max_x(width),
            rea_rs::swell_gui::TrackbarOptions::default(),
        )?;
        volume.set_range(0, 1000)?;
        volume.set_position(
            (Reaper::get().db_to_slider(0.0) * 1000.0)
                .round()
                .clamp(0.0, 1000.0) as i32,
        )?;
        let volume_value = volume_row.edit_field(
            rea_rs::SwellId(110),
            WidgetSize::new(90, 28),
            rea_rs::swell_gui::widgets::EditFieldOptions::default(),
        )?;
        volume_row.label(
            rea_rs::SwellId(111),
            "dB",
            WidgetSize::new(25, 28),
        )?;
        let docked = ExtState::<bool, Reaper>::load_value(
            WINDOW_STATE_SECTION,
            DOCK_STATE_KEY,
            Reaper::get(),
            None,
        )?
        .unwrap_or(false);
        let paint_over = read_paint_over();
        paint_checkbox.set_checked(paint_over)?;
        if docked {
            trace!(
                "gallery restoring docked state before registration: hwnd={:?}",
                window.hwnd(),
            );
            window.dock(
                "rea-rs track gallery",
                "rea_rs_widget_gallery",
                true,
            )?;
        }
        trace!(
            "gallery construction complete: hwnd={:?} valid={} visible={} docked={}",
            window.hwnd(),
            unsafe { Reaper::get().swell().IsWindow(window.hwnd()) },
            unsafe { Reaper::get().swell().IsWindowVisible(window.hwnd()) },
            window.is_docked().unwrap_or(false),
        );
        Ok(Self {
            window,
            menu_manager: DynamicManager::default(),
            paint_checkbox,
            cursor_position,
            tracklist,
            name,
            normal,
            mute,
            solo,
            automation,
            volume,
            volume_value,
            volume_dragging: false,
            tracks: Vec::new(),
            selected_guid: None,
            programmatic_update: false,
            paint_over,
            timer: None,
        })
    }

    fn save_dock_state(&self, docked: bool) {
        let mut state = ExtState::<bool, Reaper>::existing(
            WINDOW_STATE_SECTION,
            DOCK_STATE_KEY,
            true,
            Reaper::get(),
            None,
        );
        if let Err(error) = state.set(docked) {
            warn!("could not save dock preference: {error}");
        }
    }

    fn set_docked(&self, docked: bool) {
        let result = if docked {
            self.window.dock(
                "rea-rs track gallery",
                "rea_rs_widget_gallery",
                true,
            )
        } else {
            self.window.float()
        };
        if let Err(error) = result {
            warn!("could not update gallery dock state: {error}");
        } else {
            self.save_dock_state(docked);
        }
    }

    fn update_menu_state(&mut self) {
        if let Err(error) = self.menu_manager.refresh(&self.window) {
            warn!("could not refresh gallery menu state: {error}");
        }
    }

    fn refresh_play_position(&self) {
        let project = Reaper::get().current_project();
        let ratio = match (project.play_position(), project.length()) {
            (Ok(position), Ok(length)) if length.as_secs_f64() > 0.0 => {
                let position_seconds: f64 = position.into();
                (position_seconds / length.as_secs_f64()).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let _ = self
            .cursor_position
            .set_position((ratio * 1000.0).round() as i32);
    }

    fn rebuild_tracks(&mut self) {
        let project = Reaper::get().current_project();
        let old_guid = self.selected_track_guid();
        self.tracklist
            .control()
            .send_message(LIST_RESET_CONTENT, 0, 0)
            .ok();
        self.tracks.clear();
        for track in project.iter_tracks() {
            if let (Ok(name), Ok(guid)) = (track.name(), track.guid()) {
                let _ = self.tracklist.add_item(&name);
                if let Ok(guid) = guid.to_string() {
                    self.tracks.push(guid);
                }
            }
        }
        let selected = old_guid.and_then(|guid| {
            self.tracks.iter().position(|value| value == &guid)
        });
        if let Some(index) = selected {
            let _ = self.tracklist.select(index as i32);
        }
        self.selected_guid = selected.map(|index| self.tracks[index].clone());
        self.sync_inspector();
    }

    fn selected_track_guid(&self) -> Option<String> {
        let project = Reaper::get().current_project();
        project.iter_selected_tracks().next().and_then(|track| {
            track.guid().ok().and_then(|guid| guid.to_string().ok())
        })
    }

    fn with_selected_track(&self, f: impl FnOnce(&mut Track)) {
        let Some(guid) = self.selected_guid.as_deref() else {
            return;
        };
        let project = Reaper::get().current_project();
        if let Some(mut track) = project.iter_tracks().find(|track| {
            track
                .guid()
                .ok()
                .and_then(|id| id.to_string().ok())
                .as_deref()
                == Some(guid)
        }) {
            f(&mut track);
        }
    }

    fn sync_inspector(&mut self) {
        self.programmatic_update = true;
        let selected = self.selected_guid.clone();
        let track = selected.as_deref().and_then(|guid| {
            Reaper::get().current_project().iter_tracks().find(|track| {
                track
                    .guid()
                    .ok()
                    .and_then(|id| id.to_string().ok())
                    .as_deref()
                    == Some(guid)
            })
        });
        if let Some(track) = track {
            self.name.set_text(&track.name().unwrap_or_default()).ok();
            let solo_state = track.solo().unwrap_or(SoloMode::NotSoloed);
            let muted = track.muted().unwrap_or(false);
            self.solo
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state != SoloMode::NotSoloed {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            self.mute
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state == SoloMode::NotSoloed && muted {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            self.normal
                .send_message(
                    raw::BM_SETCHECK,
                    if solo_state == SoloMode::NotSoloed && !muted {
                        raw::BST_CHECKED as usize
                    } else {
                        raw::BST_UNCHECKED as usize
                    },
                    0,
                )
                .ok();
            let mode =
                track.get_automation_mode().unwrap_or(AutomationMode::None);
            self.automation.select(automation_index(mode)).ok();
            let linear =
                track.volume().map(|value| value.get()).unwrap_or(1.0);
            let db = linear_to_db(linear);
            self.volume_value.set_text(&format_db(db)).ok();
            let slider = Reaper::get().db_to_slider(db);
            trace!(
                "Track volume: linear={:?}, db={:?}, slider={:?}",
                linear,
                db,
                slider
            );
            let position = slider.round().clamp(0.0, 1000.0) as i32;
            self.volume.set_position(position).ok();
            self.set_inspector_enabled(true);
        } else {
            self.name.set_text("").ok();
            self.volume_value.set_text("").ok();
            self.normal
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.mute
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.solo
                .send_message(raw::BM_SETCHECK, raw::BST_UNCHECKED as usize, 0)
                .ok();
            self.automation.select(-1).ok();
            self.volume.set_position(0).ok();
            self.set_inspector_enabled(false);
        }
        self.programmatic_update = false;
    }

    fn volume_is_being_dragged(&self) -> bool {
        self.volume_dragging
            || rea_rs::capture_window()
                .ok()
                .flatten()
                .is_some_and(|hwnd| hwnd.as_raw() == self.volume.hwnd())
    }

    fn sync_volume(&mut self, linear: f64, update_slider: bool) {
        let db = linear_to_db(linear);
        self.programmatic_update = true;
        self.volume_value.set_text(&format_db(db)).ok();
        if update_slider {
            let position =
                Reaper::get().db_to_slider(db).round().clamp(0.0, 1000.0)
                    as i32;
            self.volume.set_position(position).ok();
        }
        self.programmatic_update = false;
    }

    fn set_inspector_enabled(&self, enabled: bool) {
        let _ = self.name.enable(enabled);
        let _ = self.normal.enable(enabled);
        let _ = self.mute.enable(enabled);
        let _ = self.solo.enable(enabled);
        let _ = self.automation.enable(enabled);
        let _ = self.volume.enable(enabled);
        let _ = self.volume_value.enable(enabled);
    }

    fn set_track_selection(&mut self, guid: Option<String>) {
        self.selected_guid = guid;
        let project = Reaper::get().current_project();
        for mut track in project.iter_tracks() {
            let selected =
                track.guid().ok().and_then(|id| id.to_string().ok())
                    == self.selected_guid;
            let _ = track.set_selected(selected);
        }
        let index = self.selected_guid.as_ref().and_then(|guid| {
            self.tracks.iter().position(|value| value == guid)
        });
        self.tracklist
            .select(index.map_or(-1, |index| index as i32))
            .ok();
        self.sync_inspector();
        if let Some(mut selected_track) = Reaper::get()
            .current_project()
            .iter_tracks()
            .find(|track| track.selected().unwrap_or(false))
        {
            let _ = selected_track.set_selected(true).ok();
        }
    }

    fn drain_events(&mut self) {
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let events = queue.get().ok().flatten().unwrap_or_default();
        for event in events {
            match event {
                DemoEvent::RebuildTracks => self.rebuild_tracks(),
                DemoEvent::Selection { guid } => {
                    if self.tracks.iter().any(|value| value == &guid) {
                        self.selected_guid = Some(guid);
                    }
                    self.sync_list_selection();
                    self.sync_inspector();
                }
                DemoEvent::Volume { guid, value } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.sync_volume(
                            value,
                            !self.volume_is_being_dragged(),
                        );
                    }
                }
                DemoEvent::Mute { guid, value: _ }
                | DemoEvent::Solo { guid, value: _ } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.sync_inspector();
                    }
                }
                DemoEvent::Title { guid, value } => {
                    if self.selected_guid.as_deref() == Some(&guid) {
                        self.programmatic_update = true;
                        self.name.set_text(&value).ok();
                        self.programmatic_update = false;
                    }
                    self.rebuild_tracks();
                }
            }
        }
        if let Err(error) = queue.set(Vec::<DemoEvent>::new()) {
            warn!("could not clear gallery event queue: {error}");
        }
    }

    fn sync_list_selection(&mut self) {
        let index = self.selected_guid.as_ref().and_then(|guid| {
            self.tracks.iter().position(|value| value == guid)
        });
        self.tracklist
            .select(index.map_or(-1, |index| index as i32))
            .ok();
    }

    fn set_paint_over(&mut self) {
        // Owned-window command handlers are temporarily detached from the
        // registry while callbacks run, so window_registered() is false for
        // normal user clicks too. Programmatic checkbox synchronization is
        // filtered by on_control_event before this method is reached.
        let command_name = format!(
            "{}_section_{}",
            PAINT_OVER_ACTION,
            rea_rs::Section::Main.id()
        );
        match Reaper::get().get_action_id(command_name) {
            Ok(Some(command_id)) => {
                if let Err(error) =
                    Reaper::get().perform_action(command_id, 0, None)
                {
                    warn!("could not invoke paint-over action: {error}");
                }
            }
            Ok(None) => warn!("paint-over action is not registered in REAPER"),
            Err(error) => {
                warn!("could not look up paint-over action: {error}")
            }
        }
        self.paint_over = read_paint_over();
        let _ = self.paint_checkbox.set_checked(self.paint_over);
        ensure_surface_registered();
    }
}

impl WindowHandler for DemoWindow {
    fn window_id(&self) -> WindowId {
        DEMO_WINDOW_ID.to_string()
    }
    fn window(&self) -> &ReaperWindow {
        &self.window
    }
    fn on_open(&mut self) -> anyhow::Result<()> {
        trace!(
            "gallery on_open entered: hwnd={:?} valid={} visible={}",
            self.window.hwnd(),
            unsafe { Reaper::get().swell().IsWindow(self.window.hwnd()) },
            unsafe {
                Reaper::get().swell().IsWindowVisible(self.window.hwnd())
            },
        );
        self.update_menu_state();
        self.timer = self.window.start_timer(TIMER_ID, TIMER_INTERVAL).ok();
        self.rebuild_tracks();
        self.refresh_play_position();
        ensure_surface_registered();
        trace!(
            "gallery on_open completed: hwnd={:?} valid={} visible={}",
            self.window.hwnd(),
            unsafe { Reaper::get().swell().IsWindow(self.window.hwnd()) },
            unsafe {
                Reaper::get().swell().IsWindowVisible(self.window.hwnd())
            },
        );
        Ok(())
    }
    fn on_command(
        &mut self,
        command: rea_rs::swell_gui::WindowCommand,
    ) -> anyhow::Result<()> {
        let rea_rs::swell_gui::WindowCommand::Menu { id } = command else {
            return Ok(());
        };
        match id.0 {
            MENU_DOCK => {
                self.set_docked(true);
            }
            MENU_FLOAT => {
                self.set_docked(false);
            }
            _ => (),
        }
        Ok(())
    }
    fn on_event(&mut self, event: WindowEvent) -> anyhow::Result<bool> {
        let WindowEvent::Mouse {
            message: MouseMessage::Up(MouseButton::Right),
            position,
            ..
        } = event
        else {
            return Ok(false);
        };
        let Ok(screen_position) = rea_rs::client_to_screen(
            self.window.hwnd().into(),
            rea_rs::SignedPoint {
                x: position.x as i32,
                y: position.y as i32,
            },
        ) else {
            return Ok(false);
        };
        if let Err(error) = self
            .window
            .post_popup_menu_bar_at(screen_position.x, screen_position.y)
        {
            warn!("could not queue dock context menu: {error}");
        }
        Ok(true)
    }
    fn on_timer(&mut self, id: rea_rs::SwellId) -> anyhow::Result<()> {
        if id != TIMER_ID {
            return Ok(());
        }
        self.drain_events();
        self.refresh_play_position();
        self.update_menu_state();
        let paint_over = read_paint_over();
        if self.paint_over != paint_over {
            self.paint_over = paint_over;
            let _ = self.paint_checkbox.set_checked(paint_over);
            ensure_surface_registered();
        }
        Ok(())
    }
    fn on_destroy(&mut self) -> anyhow::Result<()> {
        if let Some(timer) = self.timer.take() {
            let _ = self.window.stop_timer(timer);
        }
        let mut queue = ExtState::<Vec<DemoEvent>, Reaper>::existing(
            DEMO_SECTION,
            EVENT_QUEUE_KEY,
            false,
            Reaper::get(),
            None,
        );
        let _ = queue.set(Vec::<DemoEvent>::new());
        ensure_surface_registered();
        Ok(())
    }
    fn on_control_event(&mut self, event: ControlEvent) -> anyhow::Result<()> {
        trace!(
            "gallery control event received: event={event:?} programmatic_update={}",
            self.programmatic_update,
        );
        if self.programmatic_update {
            trace!("gallery control event ignored during programmatic update: {event:?}");
            return Ok(());
        }
        match event {
            ControlEvent::CheckBoxChanged { control }
                if control == self.paint_checkbox.id() =>
            {
                self.set_paint_over()
            }
            ControlEvent::ListSelectionChanged { control }
                if control == self.tracklist.id() =>
            {
                let index = self.tracklist.selected_index().unwrap_or(-1);
                debug!("LontolEvent::ListSelectionChanged. List index is: {index}");
                self.set_track_selection(
                    usize::try_from(index)
                        .ok()
                        .and_then(|index| self.tracks.get(index).cloned()),
                );
            }
            ControlEvent::EditChanged { control }
                if control == self.name.id() =>
            {
                let value = self.name.text().unwrap_or_default();
                self.with_selected_track(|track| {
                    let _ = track.set_name(value);
                });
                self.rebuild_tracks();
            }
            ControlEvent::RadioButtonChanged { control } => {
                self.with_selected_track(|track| {
                    if control == self.normal.id() {
                        let _ = track.set_solo(SoloMode::NotSoloed);
                        let _ = track.set_muted(false);
                    } else if control == self.mute.id() {
                        let _ = track.set_solo(SoloMode::NotSoloed);
                        let _ = track.set_muted(true);
                    } else if control == self.solo.id() {
                        let _ = track.set_muted(false);
                        let _ = track.set_solo(SoloMode::Soloed);
                    }
                });
                self.sync_inspector();
            }
            ControlEvent::ComboSelectionChanged { control }
                if control == self.automation.id() =>
            {
                let index = self.automation.selected_index().unwrap_or(-1);
                if let Some(mode) = automation_from_index(index) {
                    self.with_selected_track(|track| {
                        let _ = track.set_automation_mode(mode);
                    });
                }
            }
            ControlEvent::TrackbarChanged { control }
                if control == self.volume.id() =>
            {
                let position =
                    self.volume.position().unwrap_or(0).clamp(0, 1000) as f64;
                let linear =
                    db_to_linear(Reaper::get().slider_to_db(position));
                self.with_selected_track(|track| {
                    if let Ok(volume) = Volume::try_from(linear) {
                        let _ = track.set_volume(volume);
                    }
                });
                self.sync_volume(linear, false);
            }
            ControlEvent::EditChanged { control }
                if control == self.volume_value.id() =>
            {
                let text = self.volume_value.text().unwrap_or_default();
                trace!("gallery volume edit event: text={text:?}");
                if let Some(db) = parse_db_value(&text) {
                    let linear = db_to_linear(db);
                    self.with_selected_track(|track| {
                        if let Ok(volume) = Volume::try_from(linear) {
                            let _ = track.set_volume(volume);
                        }
                    });
                    let slider = Reaper::get().db_to_slider(db);
                    self.programmatic_update = true;
                    self.volume
                        .set_position(slider.round().clamp(0.0, 1000.0) as i32)
                        .ok();
                    self.programmatic_update = false;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn on_widget_event(
        &mut self,
        id: rea_rs::SwellId,
        event: rea_rs::WindowEvent,
    ) -> anyhow::Result<bool> {
        if id == self.volume.id() {
            if let rea_rs::WindowEvent::Mouse { message, .. } = event {
                match message {
                    rea_rs::swell_gui::MouseMessage::Down(
                        rea_rs::swell_gui::MouseButton::Left,
                    ) => self.volume_dragging = true,
                    rea_rs::swell_gui::MouseMessage::Up(
                        rea_rs::swell_gui::MouseButton::Left,
                    ) => {
                        self.volume_dragging = false;
                        self.sync_inspector();
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }
}

struct DemoHostWindow {
    window: ReaperWindow,
}
impl WindowHandler for DemoHostWindow {
    fn window_id(&self) -> WindowId {
        HOST_WINDOW_ID.to_string()
    }
    fn window(&self) -> &ReaperWindow {
        &self.window
    }
    fn handle_host_message(&self, message: u32) -> anyhow::Result<bool> {
        Ok(message == raw::WM_PAINT)
    }
    fn on_destroy(&mut self) -> anyhow::Result<()> {
        // The borrowed host HWND has its original WndProc restored before
        // this callback. Drop the custom renderer and request a clean paint
        // through that original procedure.
        let _ = self.window.clear_render();
        Ok(())
    }
}

fn automation_name(mode: AutomationMode) -> &'static str {
    match mode {
        AutomationMode::None => "None",
        AutomationMode::TrimRead => "TrimRead",
        AutomationMode::Read => "Read",
        AutomationMode::Touch => "Touch",
        AutomationMode::Write => "Write",
        AutomationMode::Latch => "Latch",
        AutomationMode::Bypass => "Bypass",
    }
}
fn automation_index(mode: AutomationMode) -> i32 {
    match mode {
        AutomationMode::None => 0,
        AutomationMode::TrimRead => 1,
        AutomationMode::Read => 2,
        AutomationMode::Touch => 3,
        AutomationMode::Write => 4,
        AutomationMode::Latch => 5,
        AutomationMode::Bypass => 6,
    }
}
fn automation_from_index(index: i32) -> Option<AutomationMode> {
    Some(match index {
        0 => AutomationMode::None,
        1 => AutomationMode::TrimRead,
        2 => AutomationMode::Read,
        3 => AutomationMode::Touch,
        4 => AutomationMode::Write,
        5 => AutomationMode::Latch,
        6 => AutomationMode::Bypass,
        _ => return None,
    })
}
fn format_db(db: f64) -> String {
    if db.is_finite() {
        format!("{db:.2}")
    } else {
        "-inf".to_string()
    }
}

fn parse_db_value(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::parse_db_value;

    #[test]
    fn parses_signed_integer_and_decimal_db_values() {
        assert_eq!(parse_db_value("-14"), Some(-14.0));
        assert_eq!(parse_db_value(" 1.5 "), Some(1.5));
        assert_eq!(parse_db_value("-"), None);
        assert_eq!(parse_db_value("NaN"), None);
    }
}

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_preferences_page(
        PREFS_PAGE_ID,
        "rea-rs Widget Gallery",
        |window| {
            let ui = window.build_ui()?;
            ui.label(
                rea_rs::SwellId(1),
                "This page is hosted by REAPER Preferences.",
                WidgetSize::new(320, 24),
            )?;
            Ok(Box::new(PreferencesPage { window }))
        },
    )?;
    let _queue = ExtState::<Vec<DemoEvent>, Reaper>::new(
        DEMO_SECTION,
        EVENT_QUEUE_KEY,
        Vec::new(),
        false,
        Reaper::get(),
        None,
    )?;
    let paint_over = ExtState::<bool, Reaper>::existing(
        DEMO_SECTION,
        PAINT_OVER_KEY,
        true,
        Reaper::get(),
        None,
    )
    .get()?
    .unwrap_or(false);
    let lifecycle = Arc::new(RefCell::new(DemoLifecycle {
        paint_over,
        ..Default::default()
    }));
    if paint_over {
        reaper.register_control_surface(Arc::new(RefCell::new(
            DemoCSurf::default(),
        )));
    }
    let toggle_state = lifecycle.clone();
    reaper.register_action(
        PAINT_OVER_ACTION,
        PAINT_OVER_DESCRIPTION,
        ActionKind::Toggleable(paint_over),
        move |hook: &mut ActionHook| {
            let current = read_paint_over();
            let next = !current;
            hook.set_toggle_state(next);
            toggle_state.borrow_mut().paint_over = next;
            let mut state = ExtState::<bool, Reaper>::existing(
                DEMO_SECTION,
                PAINT_OVER_KEY,
                true,
                Reaper::get(),
                None,
            );
            state.set(next)?;
            ensure_surface_registered();
            Ok(())
        },
        None,
    )?;
    reaper.register_action(
        "TestSwellGuiWidgetGallery",
        "test swell-gui widget gallery",
        ActionKind::NotToggleable,
        |_| {
            if window_registered() {
                Reaper::get_mut()
                    .unregister_window_handler(&DEMO_WINDOW_ID.to_string())?;
            } else {
                let demo_window = DemoWindow::new()?;
                trace!(
                    "registering completed gallery window: hwnd={:?} valid={} visible={}",
                    demo_window.window.hwnd(),
                    unsafe { Reaper::get().swell().IsWindow(demo_window.window.hwnd()) },
                    unsafe { Reaper::get().swell().IsWindowVisible(demo_window.window.hwnd()) },
                );
                Reaper::get_mut()
                    .register_window_handler(Box::new(demo_window))?;
                trace!("completed gallery window registration");
                ensure_surface_registered();
            }
            info!("rea-rs track gallery toggled");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
