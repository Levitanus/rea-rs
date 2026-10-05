use log::info;
use rea_rs::{
    swell_gui::{
        layout::{Point, Rect, WidgetFills, WidgetSize},
        DrawTextFlags, LiceCombineMode, LiceTextOptions,
    },
    ActionKind, Canvas, CheckBox, Color, ComboBox, ControlEvent, ControlId,
    EditField, EventResponse, ExtState, Font, FontSpec, LiceFont, ListBox,
    Reaper, ReaperWindow, ThemeColor, WindowCommand, WindowHandler, WindowId,
    WindowSpec,
};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";
const DOCK_STATE_KEY: &str = "rea_rs_widget_gallery.dock";

struct DemoWindow {
    window: ReaperWindow,
    dock_state: CheckBox,
    edit: EditField,
    checkbox: CheckBox,
    combo: ComboBox,
    list: ListBox,
    canvas: Canvas,
}

impl DemoWindow {
    fn new() -> anyhow::Result<Self> {
        let (width, height) = (640_u32, 480_u32);
        let window = Reaper::get().create_window(
            &WindowSpec::new("rea-rs widget gallery")
                .size(width, height)
                .dock_ident("rea_rs_widget_gallery"),
        )?;

        let ui = window.build_ui()?.central_panel();
        let scroll_view_id = ControlId::new();
        let ui = ui.scroll_view(
            scroll_view_id,
            WidgetSize::new_fill_both(width, height),
            rea_rs::ScrollbarRenderer::CoolSb,
        )?;
        let dock_state = ui.checkbox(
            ControlId(DOCK_STATE_ID),
            "Docked",
            WidgetSize::new(120, 24),
        )?;
        ui.label(
            ControlId(LABEL_ID),
            "SWELL widget gallery",
            WidgetSize::new(220, 24),
        )?;
        let ui = ui.group_box(
            ControlId(GROUP_ID),
            "Native controls",
            WidgetSize::new_fill_both(390, 180),
        )?;
        let _button = ui.button(
            ControlId(BUTTON_ID),
            "Click me",
            WidgetSize::new(120, 28),
        )?;
        let edit =
            ui.edit_field(ControlId(EDIT_ID), WidgetSize::new(210, 28), 0)?;
        let checkbox = ui.checkbox(
            ControlId(CHECKBOX_ID),
            "Example checkbox",
            WidgetSize::new(180, 24),
        )?;
        let combo =
            ui.combo_box(ControlId(COMBO_ID), WidgetSize::new(160, 28), 0)?;
        combo.add_item("First item")?;
        combo.add_item("Second item")?;
        let list = ui.list_box(
            ControlId(LIST_ID),
            WidgetSize::new_fill_both(350, 55)
                .set_min_x(180)
                .set_min_y(40)
                .set_fill_x(WidgetFills::FillPortion(2)),
            0,
        )?;
        list.add_item("List item one")?;
        list.add_item("List item two")?;

        let native = ui.group_box(
            ControlId(NATIVE_GROUP_ID),
            "Additional native controls",
            WidgetSize::new_fill_both(380, 190),
        )?;
        native.radio_button(
            ControlId(RADIO_ID),
            "Radio option",
            WidgetSize::new(180, 24),
            0,
        )?;
        let trackbar = native.trackbar(
            ControlId(TRACKBAR_ID),
            WidgetSize::new(280, 28),
            0,
        )?;
        trackbar.set_range(0, 100)?;
        trackbar.set_position(35)?;
        let progress = native.progress_bar(
            ControlId(PROGRESS_ID),
            WidgetSize::new(260, 20),
            0,
        )?;
        progress.set_range(0, 100)?;
        progress.set_position(60)?;
        native.tab_control(ControlId(TAB_ID), WidgetSize::new(280, 36), 0)?;
        native.list_view(
            ControlId(LIST_VIEW_ID),
            WidgetSize::new(280, 70),
            0,
        )?;
        native.tree_view(
            ControlId(TREE_VIEW_ID),
            WidgetSize::new(280, 70),
            0,
        )?;

        let virtual_canvas = ui
            .canvas(ControlId(VIRTUAL_CANVAS_ID), WidgetSize::new(420, 180))?;
        let _ = virtual_canvas;
        let virtual_button = ui.virtual_icon_button(
            ControlId(VIRTUAL_BUTTON_ID),
            ControlId(VIRTUAL_CANVAS_ID),
            WidgetSize::new(140, 28),
        )?;
        virtual_button.set_text("Virtual button")?;
        let virtual_label = ui.virtual_static_text(
            ControlId(VIRTUAL_LABEL_ID),
            ControlId(VIRTUAL_CANVAS_ID),
            WidgetSize::new(180, 24),
        )?;
        virtual_label.set_text("WDL virtual controls")?;
        let virtual_combo = ui.virtual_combo_box(
            ControlId(VIRTUAL_COMBO_ID),
            ControlId(VIRTUAL_CANVAS_ID),
            WidgetSize::new(180, 28),
        )?;
        virtual_combo.add_item("Virtual item one")?;
        virtual_combo.add_item("Virtual item two")?;
        virtual_combo.set_selection(0);
        let virtual_slider = ui.virtual_slider(
            ControlId(VIRTUAL_SLIDER_ID),
            ControlId(VIRTUAL_CANVAS_ID),
            WidgetSize::new(240, 32),
        )?;
        virtual_slider.set_range(0, 1000, 500);
        virtual_slider.set_value(420);
        let virtual_list = ui.virtual_list_box(
            ControlId(VIRTUAL_LIST_ID),
            ControlId(VIRTUAL_CANVAS_ID),
            WidgetSize::new(240, 72),
        )?;
        virtual_list.add_item("Virtual row one")?;
        virtual_list.add_item("Virtual row two")?;
        let title_font = LiceFont::from_font(Font::new(
            FontSpec::new("Arial").set_size(18),
        )?)?;

        let canvas =
            ui.canvas(ControlId(CANVAS_ID), WidgetSize::new(280, 84))?;
        window.on_render(move |info, surface| {
            surface.clear(ThemeColor::windowtab_bg.into());
            surface.bordered_rect(
                info.client_rect,
                Color::YELLOW,
                Color::new(64, 144, 208),
                1.0,
                LiceCombineMode::Copy,
            );
            let _ = surface.draw_text(
                "Retained LICE drawing",
                Rect::new(24, 0, 380, 32),
                &title_font,
                LiceTextOptions {
                    flags: DrawTextFlags::LEFT | DrawTextFlags::TOP,
                    ..Default::default()
                },
            );
            surface.line(
                Point { x: 18, y: 52 },
                Point { x: 620, y: 52 },
                Color::new(64, 144, 208),
                1.0,
                LiceCombineMode::Copy,
                true,
            );
            Ok(())
        })?;
        window.on_render_widget(|id, info, surface| {
            if id == ControlId(CANVAS_ID) {
                surface.fill_rect(
                    info.client_rect,
                    Color::BLUE,
                    1.0,
                    LiceCombineMode::Copy,
                );
                surface.draw_rect(
                    info.client_rect,
                    Color::MAGENTA,
                    1.0,
                    LiceCombineMode::Copy,
                );
            }
            Ok(())
        })?;
        ui.on_widget_event(ControlId(BUTTON_ID), |event| {
            log::debug!("button event: {:?}", event);
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(DOCK_STATE_ID), |event| {
            log::debug!("dock-state event: {:?}", event);
            EventResponse::ForwardToWindow
        });
        ui.on_widget_event(ControlId(CHECKBOX_ID), move |event| {
            if let ControlEvent::CheckBoxChanged { .. } = event {
                log::debug!("checkbox value: {:?}", checkbox.checked());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(COMBO_ID), move |event| {
            if let ControlEvent::ComboSelectionChanged { .. } = event {
                log::debug!("combo selection: {:?}", combo.selected_index());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(LIST_ID), move |event| {
            if let ControlEvent::ListSelectionChanged { .. } = event {
                log::debug!("list selection: {:?}", list.selected_index());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(EDIT_ID), move |event| {
            if let ControlEvent::EditChanged { .. } = event {
                log::debug!("edit value: {:?}", edit.text());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(TRACKBAR_ID), move |event| {
            if matches!(event, ControlEvent::TrackbarChanged { .. }) {
                log::debug!(
                    "native trackbar position: {:?}",
                    trackbar.position()
                );
            }
            EventResponse::Handled
        });
        for id in [
            VIRTUAL_BUTTON_ID,
            VIRTUAL_COMBO_ID,
            VIRTUAL_SLIDER_ID,
            VIRTUAL_LIST_ID,
        ] {
            ui.on_widget_event(ControlId(id), move |event| {
                log::debug!("virtual widget event: {:?}", event);
                EventResponse::Handled
            });
        }

        let docked = ExtState::<bool, Reaper>::existing(
            WINDOW_STATE_SECTION,
            DOCK_STATE_KEY,
            true,
            Reaper::get(),
            None,
        )
        .get()?
        .unwrap_or(false);
        dock_state.set_checked(docked)?;
        if docked {
            window.dock(
                "rea-rs widget gallery",
                "rea_rs_widget_gallery",
                true,
            )?;
        }

        Ok(Self {
            window,
            dock_state,
            edit,
            checkbox,
            combo,
            list,
            canvas,
        })
    }

    fn update_dock_state(&self) {
        match self.window.is_docked() {
            Ok(docked) => {
                if let Err(error) = self.dock_state.set_checked(docked) {
                    info!("could not update dock state: {}", error);
                }
            }
            Err(error) => info!("could not query dock state: {}", error),
        }
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
            info!("could not save dock state: {}", error);
        }
    }

    fn log_control_values(&self) {
        info!("dock state: {:?}", self.dock_state.checked());
        info!("checkbox: {:?}", self.checkbox.checked());
        info!("edit: {:?}", self.edit.text());
        info!("combo: {:?}", self.combo.selected_index());
        info!("list: {:?}", self.list.selected_index());
    }
}

impl WindowHandler for DemoWindow {
    fn window_id(&self) -> WindowId {
        "rea-rs.widget-gallery".to_string()
    }

    fn window(&self) -> &ReaperWindow {
        &self.window
    }

    fn on_open(&mut self) {
        self.update_dock_state();
        self.log_control_values();
    }

    fn on_resize(&mut self, width: i32, height: i32) {
        log::debug!("widget gallery resized: client={}x{}", width, height);
    }

    fn on_close(&mut self) -> bool {
        true
    }

    fn on_destroy(&mut self) {
        info!("window destroyed");
    }

    fn on_command(&mut self, command: WindowCommand) {
        log::debug!("window command: {:?}", command);
    }

    fn on_control_event(&mut self, event: ControlEvent) {
        if let ControlEvent::CheckBoxChanged { control } = event {
            if control == ControlId(DOCK_STATE_ID) {
                let docked = self.dock_state.checked().unwrap_or(false);
                let result = if docked {
                    self.window.dock(
                        "rea-rs widget gallery",
                        "rea_rs_widget_gallery",
                        true,
                    )
                } else {
                    self.window.float()
                };
                if let Err(error) = result {
                    info!("dock action failed: {}", error);
                } else {
                    self.save_dock_state(docked);
                }
                self.update_dock_state();
            }
        }
    }

    fn on_widget_event(
        &mut self,
        id: ControlId,
        event: rea_rs::WindowEvent,
    ) -> bool {
        if id == self.canvas.id() {
            if matches!(
                event,
                rea_rs::WindowEvent::Mouse {
                    message: rea_rs::swell_gui::MouseMessage::Move,
                    ..
                }
            ) {
                log::trace!("canvas mouse move: {:?}", event);
            } else {
                log::debug!("canvas event: {:?}", event);
            }
            false
        } else {
            false
        }
    }

    fn on_event(&mut self, event: rea_rs::WindowEvent) -> bool {
        if matches!(
            &event,
            rea_rs::WindowEvent::Mouse {
                message: rea_rs::swell_gui::MouseMessage::Move
                    | rea_rs::swell_gui::MouseMessage::Wheel { .. },
                ..
            } | rea_rs::WindowEvent::Wheel { .. }
        ) {
            log::trace!("window event: {:?}", event);
        } else {
            log::debug!("window event: {:?}", event);
        }
        false
    }
}

const DOCK_STATE_ID: i32 = 100;
const BUTTON_ID: i32 = 101;
const EDIT_ID: i32 = 102;
const CHECKBOX_ID: i32 = 103;
const LABEL_ID: i32 = 104;
const GROUP_ID: i32 = 105;
const COMBO_ID: i32 = 106;
const LIST_ID: i32 = 107;
const CANVAS_ID: i32 = 108;
const NATIVE_GROUP_ID: i32 = 109;
const RADIO_ID: i32 = 110;
const TRACKBAR_ID: i32 = 111;
const PROGRESS_ID: i32 = 112;
const TAB_ID: i32 = 113;
const LIST_VIEW_ID: i32 = 114;
const TREE_VIEW_ID: i32 = 115;
const VIRTUAL_CANVAS_ID: i32 = 116;
const VIRTUAL_BUTTON_ID: i32 = 117;
const VIRTUAL_LABEL_ID: i32 = 118;
const VIRTUAL_COMBO_ID: i32 = 119;
const VIRTUAL_SLIDER_ID: i32 = 120;
const VIRTUAL_LIST_ID: i32 = 121;

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_action(
        "TestSwellGuiWidgetGallery",
        "test swell-gui widget gallery",
        ActionKind::NotToggleable,
        |_| {
            let demo_window = DemoWindow::new()?;
            if let Err(error) = Reaper::get_mut()
                .register_window_handler(Box::new(demo_window))
            {
                return Err(error.into());
            }
            info!("rea-rs window demo opened");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
