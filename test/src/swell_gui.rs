use log::info;
use rea_rs::{
    swell_gui::layout::{WidgetFills, WidgetSize},
    ActionKind, Button, CheckBox, ComboBox, ControlEvent, ControlId,
    EditField, EventResponse, ExtState, ListBox, Reaper, ReaperWindow,
    WindowCommand, WindowHandler, WindowId, WindowSpec,
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
        let ui = ui.scroll_view(
            ControlId::new(),
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
        let button = ui.button(
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

        ui.on_widget_event(ControlId(BUTTON_ID), |event| {
            info!("button event: {:?}", event);
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(DOCK_STATE_ID), |event| {
            info!("dock-state event: {:?}", event);
            EventResponse::ForwardToWindow
        });
        ui.on_widget_event(ControlId(CHECKBOX_ID), move |event| {
            if let ControlEvent::CheckBoxChanged { .. } = event {
                info!("checkbox value: {:?}", checkbox.checked());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(COMBO_ID), move |event| {
            if let ControlEvent::ComboSelectionChanged { .. } = event {
                info!("combo selection: {:?}", combo.selected_index());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(LIST_ID), move |event| {
            if let ControlEvent::ListSelectionChanged { .. } = event {
                info!("list selection: {:?}", list.selected_index());
            }
            EventResponse::Handled
        });
        ui.on_widget_event(ControlId(EDIT_ID), move |event| {
            if let ControlEvent::EditChanged { .. } = event {
                info!("edit value: {:?}", edit.text());
            }
            EventResponse::Handled
        });

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
    fn on_close(&mut self) -> bool {
        true
    }
    fn on_destroy(&mut self) {
        info!("window destroyed");
    }
    fn on_command(&mut self, command: WindowCommand) {
        info!("window command: {:?}", command);
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
}

const DOCK_STATE_ID: i32 = 100;
const BUTTON_ID: i32 = 101;
const EDIT_ID: i32 = 102;
const CHECKBOX_ID: i32 = 103;
const LABEL_ID: i32 = 104;
const GROUP_ID: i32 = 105;
const COMBO_ID: i32 = 106;
const LIST_ID: i32 = 107;

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_action(
        "TestSwellGuiWidgetGallery",
        "test swell-gui widget gallery",
        ActionKind::NotToggleable,
        |_| {
            let demo_window = DemoWindow::new()?;
            let hwnd = demo_window.window.hwnd();
            if let Err(error) = Reaper::get_mut()
                .register_window_handler(Box::new(demo_window))
            {
                let _ = Reaper::get_mut().unregister_window_handler(hwnd);
                return Err(error.into());
            }
            info!("rea-rs window demo opened");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
