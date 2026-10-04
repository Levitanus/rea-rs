use log::info;
use rea_rs::{
    swell_gui::layout::WidgetSize, ActionKind, Button, CheckBox, ComboBox,
    ControlEvent, ControlId, EditField, EventResponse, ExtState, ListBox,
    Reaper, ReaperWindow, WindowCommand, WindowHandler, WindowId, WindowSpec,
};

const WINDOW_STATE_SECTION: &str = "rea-rs.window";
const DOCK_STATE_KEY: &str = "rea_rs_widget_gallery.dock";

struct DemoWindow {
    window: ReaperWindow,
    dock_state: CheckBox,
    button: Button,
    edit: EditField,
    checkbox: CheckBox,
    combo: ComboBox,
    list: ListBox,
}

impl DemoWindow {
    fn new() -> anyhow::Result<Self> {
        let (def_x_w, def_x_h) = (640_u32, 480_u32);
        let window = Reaper::get().create_window(
            &WindowSpec::new("rea-rs widget gallery")
                .size(def_x_w, def_x_h)
                .dock_ident("rea_rs_widget_gallery"), /* .no_close(false)
                                                       * .resizable(true), */
        )?;

        // Creation is expressed in layout terms. The panel/group owns the
        // initial geometry; callers do not provide x/y coordinates.
        let central = window.central_panel();
        let scroll_id = ControlId::new();
        let scroll_view = central.create_scroll_view(
            scroll_id,
            WidgetSize::new_fill_both(def_x_w, def_x_h),
            rea_rs::ScrollbarRenderer::Native,
        );
        let dock_state = scroll_view.create_checkbox(
            ControlId(DOCK_STATE_ID),
            "Docked",
            WidgetSize::new(120, 24),
        )?;
        let _label = scroll_view.create_label(
            ControlId(LABEL_ID),
            "SWELL widget gallery",
            WidgetSize::new(220, 24),
        )?;
        let group = scroll_view.create_group_box(
            ControlId(GROUP_ID),
            "Native controls",
            WidgetSize::new_fill_both(390, 180),
        )?;
        let button = group.create_button(
            ControlId(BUTTON_ID),
            "Click me",
            WidgetSize::new(120, 28),
        )?;
        let edit = group.create_edit_field(
            ControlId(EDIT_ID),
            WidgetSize::new(210, 28),
            0,
        )?;
        let checkbox = group.create_checkbox(
            ControlId(CHECKBOX_ID),
            "Example checkbox",
            WidgetSize::new(180, 24),
        )?;
        let combo = group.create_combo_box(
            ControlId(COMBO_ID),
            WidgetSize::new(160, 28),
            0,
        )?;
        combo.add_item("First item")?;
        combo.add_item("Second item")?;
        let list = group.create_list_box(
            ControlId(LIST_ID),
            WidgetSize::new_fill_both(350, 55)
                .set_min_x(180)
                .set_min_y(40),
            0,
        )?;
        list.add_item("List item one")?;
        list.add_item("List item two")?;
        window.on_widget_event(ControlId(BUTTON_ID), |event| {
            info!("button event: {:?}", event);
            EventResponse::Handled
        });
        window.on_widget_event(ControlId(DOCK_STATE_ID), |event| {
            info!("dock-state event: {:?}", event);
            EventResponse::ForwardToWindow
        });
        window.on_widget_event(ControlId(CHECKBOX_ID), move |event| {
            if let ControlEvent::CheckBoxChanged { .. } = event {
                match checkbox.checked() {
                    Ok(value) => info!("example checkbox value: {}", value),
                    Err(error) => {
                        info!("could not read example checkbox: {}", error)
                    }
                }
            }
            EventResponse::Handled
        });
        window.on_widget_event(ControlId(COMBO_ID), move |event| {
            if let ControlEvent::ComboSelectionChanged { .. } = event {
                match combo.selected_index() {
                    Ok(value) => info!("combo selected index: {}", value),
                    Err(error) => {
                        info!("could not read combo selection: {}", error)
                    }
                }
            }
            EventResponse::Handled
        });
        window.on_widget_event(ControlId(LIST_ID), move |event| {
            if let ControlEvent::ListSelectionChanged { .. } = event {
                match list.selected_index() {
                    Ok(value) => info!("list selected index: {}", value),
                    Err(error) => {
                        info!("could not read list selection: {}", error)
                    }
                }
            }
            EventResponse::Handled
        });
        window.on_widget_event(ControlId(EDIT_ID), move |event| {
            if let ControlEvent::EditChanged { .. } = event {
                match edit.text() {
                    Ok(value) => info!("edit value: {:?}", value),
                    Err(error) => {
                        info!("could not read edit value: {}", error)
                    }
                }
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
        button.enable(true)?;

        Ok(Self {
            window,
            dock_state,
            button,
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
                    info!("could not update dock-state checkbox: {}", error);
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
}

impl WindowHandler for DemoWindow {
    fn window_id(&self) -> WindowId {
        "rea-rs.widget-gallery".to_string()
    }

    fn window(&self) -> &ReaperWindow {
        &self.window
    }

    fn on_open(&mut self) {
        info!("window opened");
        self.update_dock_state();
        self.log_control_values();
    }

    fn on_close(&mut self) -> bool {
        info!("window close requested");
        true
    }

    fn on_destroy(&mut self) {
        info!("window destroyed");
    }

    fn on_command(&mut self, command: WindowCommand) {
        info!("window command: {:?}", command);
    }

    fn on_control_event(&mut self, event: ControlEvent) {
        info!("control event: {:?}", event);

        if let ControlEvent::CheckBoxChanged { control } = event {
            if control == ControlId(DOCK_STATE_ID) {
                let requested_docked =
                    self.dock_state.checked().unwrap_or(false);
                let result = if requested_docked {
                    self.window.dock(
                        "rea-rs widget gallery",
                        "rea_rs_widget_gallery",
                        true,
                    )
                } else {
                    self.window.float()
                };
                if let Err(error) = result {
                    info!("dock-state checkbox action failed: {}", error);
                } else {
                    self.save_dock_state(requested_docked);
                }
                self.update_dock_state();
            }
        }
    }
}
impl DemoWindow {
    fn log_control_values(&self) {
        match self.dock_state.checked() {
            Ok(value) => info!("dock-state value: {}", value),
            Err(error) => info!("could not read dock-state value: {}", error),
        }
        match self.checkbox.checked() {
            Ok(value) => info!("checkbox value: {}", value),
            Err(error) => info!("could not read checkbox value: {}", error),
        }
        match self.edit.text() {
            Ok(value) => info!("edit value: {:?}", value),
            Err(error) => info!("could not read edit value: {}", error),
        }
        match self.combo.selected_index() {
            Ok(value) => info!("combo selected index: {}", value),
            Err(error) => info!("could not read combo selection: {}", error),
        }
        match self.list.selected_index() {
            Ok(value) => info!("list selected index: {}", value),
            Err(error) => info!("could not read list selection: {}", error),
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
const WINDOW_CONTAINER_ID: i32 = 108;

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
                // `register_window_handler` consumes the handler. On an
                // error the Box is dropped, which destroys the newly-created
                // owned window. Deregister as a defensive cleanup in case a
                // future registration path inserts before failing.
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
