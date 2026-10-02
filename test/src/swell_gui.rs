use log::info;
use rea_rs::{
    ActionKind, Button, CheckBox, ComboBox, ControlEvent, ControlId,
    ControlRect, EditField, GroupBox, ListBox, Reaper, ReaperWindow,
    StaticLabel, WindowCommand, WindowHandler, WindowId, WindowSpec,
};

struct DemoWindow {
    window: ReaperWindow,
    dock_state: CheckBox,
    button: Button,
    edit: EditField,
    checkbox: CheckBox,
    label: StaticLabel,
    group: GroupBox,
    combo: ComboBox,
    list: ListBox,
}

impl DemoWindow {
    fn new() -> anyhow::Result<Self> {
        let window = Reaper::get().create_window(
            &WindowSpec::new("rea-rs widget gallery")
                .size(640, 420)
                .dock_ident("rea_rs_widget_gallery")
                .no_close(false)
                .resizable(true),
        )?;

        let dock_state = window.create_checkbox(
            ControlId(DOCK_STATE_ID),
            "Docked",
            ControlRect::new(20, 20, 120, 24),
        )?;
        let label = window.create_label(
            ControlId(LABEL_ID),
            "SWELL widget gallery",
            ControlRect::new(20, 40, 220, 24),
        )?;
        let group = window.create_group_box(
            ControlId(GROUP_ID),
            "Native controls",
            ControlRect::new(10, 70, 390, 180),
        )?;
        // Keep interactive controls parented to the dialog window.  A native
        // group box is a visual control, not a dialog container: notifications
        // sent to children are delivered to the immediate parent and SWELL
        // does not reliably forward them through the group box.  Use the
        // group's position as the origin so the controls still appear inside
        // the frame and remain discoverable with GetDlgItem after docking.
        let button = window.create_button(
            ControlId(BUTTON_ID),
            "Click me",
            ControlRect::new(30, 215, 120, 28),
        )?;
        let edit = window.create_edit_field(
            ControlId(EDIT_ID),
            ControlRect::new(170, 215, 210, 28),
            0,
        )?;
        let checkbox = window.create_checkbox(
            ControlId(CHECKBOX_ID),
            "Example checkbox",
            ControlRect::new(30, 255, 180, 24),
        )?;
        let combo = window.create_combo_box(
            ControlId(COMBO_ID),
            ControlRect::new(220, 255, 160, 28),
            0,
        )?;
        combo.add_item("First item")?;
        combo.add_item("Second item")?;
        let list = window.create_list_box(
            ControlId(LIST_ID),
            ControlRect::new(30, 290, 350, 55),
            0,
        )?;
        list.add_item("List item one")?;
        list.add_item("List item two")?;
        dock_state.set_checked(false)?;
        button.enable(true)?;

        Ok(Self {
            window,
            dock_state,
            button,
            edit,
            checkbox,
            label,
            group,
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
                let docked = self.window.is_docked().unwrap_or(false);
                let result = if docked {
                    self.window.float()
                } else {
                    self.window.dock(
                        "rea-rs widget gallery",
                        "rea_rs_widget_gallery",
                        true,
                    )
                };
                if let Err(error) = result {
                    info!("dock-state checkbox action failed: {}", error);
                }
                self.update_dock_state();
            } else if control == ControlId(CHECKBOX_ID) {
                match self.checkbox.checked() {
                    Ok(value) => info!("example checkbox value: {}", value),
                    Err(error) => {
                        info!("could not read example checkbox: {}", error)
                    }
                }
            }
        }

        if let ControlEvent::ButtonClicked { control } = event {
            if control == ControlId(BUTTON_ID) {
                // Do not send BM_CLICK from the click handler: that produces
                // another WM_COMMAND and recursively clicks the same button.
                self.log_control_values();
            }
        }

        if let ControlEvent::ComboSelectionChanged { control } = event {
            if control == ControlId(COMBO_ID) {
                match self.combo.selected_index() {
                    Ok(value) => info!("combo selected index: {}", value),
                    Err(error) => {
                        info!("could not read combo selection: {}", error)
                    }
                }
            }
        }

        if let ControlEvent::ListSelectionChanged { control } = event {
            if control == ControlId(LIST_ID) {
                match self.list.selected_index() {
                    Ok(value) => info!("list selected index: {}", value),
                    Err(error) => {
                        info!("could not read list selection: {}", error)
                    }
                }
            }
        }

        if let ControlEvent::EditChanged { control } = event {
            if control == ControlId(EDIT_ID) {
                match self.edit.text() {
                    Ok(value) => info!("edit value: {:?}", value),
                    Err(error) => {
                        info!("could not read edit value: {}", error)
                    }
                }
            }
        }
    }

    fn on_resize(&mut self, width: i32, height: i32) {
        info!("window resized to {}x{}", width, height);
        // The controls use dialog-client coordinates.  Do not move only one
        // child here: doing so breaks the layout relative to the group box.
        let _ = (width, height);
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

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_action(
        "TestSwellGuiWidgetGallery",
        "test swell-gui widget gallery",
        ActionKind::NotToggleable,
        |_| {
            let demo_window = DemoWindow::new()?;
            Reaper::get_mut()
                .register_window_handler(Box::new(demo_window))?;
            info!("rea-rs window demo opened");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
