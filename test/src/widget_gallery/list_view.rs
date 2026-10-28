use rea_rs::{
    swell_gui::{
        layout::WidgetSize,
        widgets::{ListViewColumn, ListViewOptions, ListViewRow},
    },
    ControlEvent, EventResponse, ListViewExtendedOptions, ReaperWindow,
    SwellId,
};

const LIST_ID: SwellId = SwellId(0x7301);
const ADD_ID: SwellId = SwellId(0x7302);
const REMOVE_ID: SwellId = SwellId(0x7303);
const CLEAR_ID: SwellId = SwellId(0x7304);
const SELECT_ID: SwellId = SwellId(0x7305);
const CHECK_ONE_ID: SwellId = SwellId(0x7306);
const CHECK_TWO_ID: SwellId = SwellId(0x7307);
const CELL_EDIT_ID: SwellId = SwellId(0x7309);
const CELL_APPLY_ID: SwellId = SwellId(0x730a);

pub(super) fn build(window: &ReaperWindow) -> anyhow::Result<()> {
    let ui = window.build_ui()?;
    let row = ui.row(SwellId(0x7300), WidgetSize::new_fill_x(620, 34))?;
    let add = row.button(ADD_ID, "Add row", WidgetSize::new(100, 28))?;
    let remove =
        row.button(REMOVE_ID, "Remove selected", WidgetSize::new(150, 28))?;
    let clear = row.button(CLEAR_ID, "Clear", WidgetSize::new(90, 28))?;
    let select =
        row.button(SELECT_ID, "Select first", WidgetSize::new(110, 28))?;
    let edit_row = ui.row(SwellId(0x730c), WidgetSize::new_fill_x(620, 32))?;
    let cell_edit = edit_row.edit_field(
        CELL_EDIT_ID,
        WidgetSize::new_fill_x(360, 28).set_min_x(180),
        rea_rs::swell_gui::widgets::EditFieldOptions::default(),
    )?;
    let apply_cell = edit_row.button(
        CELL_APPLY_ID,
        "Set selected name",
        WidgetSize::new(160, 28),
    )?;
    let list = ui.list_view(
        LIST_ID,
        WidgetSize::new_fill_both(620, 320).set_min_y(180),
        ListViewOptions::REPORT
            | ListViewOptions::SINGLE_SELECT
            | ListViewOptions::EDIT_LABELS,
    )?;
    list.set_extended_options(
        ListViewExtendedOptions::CHECKBOXES
            | ListViewExtendedOptions::TWO_CLICK_ACTIVATE,
    )?;
    list.add_column(&ListViewColumn {
        title: "Name".into(),
        width: 250,
    })?;
    list.add_column(&ListViewColumn {
        title: "Category".into(),
        width: 160,
    })?;
    list.add_column(&ListViewColumn {
        title: "Value".into(),
        width: 120,
    })?;
    let mut first_row = None;
    for row in [
        ListViewRow {
            cells: vec!["Kick".into(), "Drums".into(), "0.82".into()],
            image_index: None,
        },
        ListViewRow {
            cells: vec!["Snare".into(), "Drums".into(), "0.64".into()],
            image_index: None,
        },
        ListViewRow {
            cells: vec!["Bass".into(), "Instruments".into(), "0.91".into()],
            image_index: None,
        },
    ] {
        let id = list.insert_row(&row)?;
        first_row.get_or_insert(id);
    }

    let list_add = list;
    ui.on_widget_event(add.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            let next = list_add.item_count()? + 1;
            list_add.insert_row(&ListViewRow {
                cells: vec![
                    format!("New item {next}"),
                    "Added".into(),
                    format!("{:.2}", next as f32 / 10.0),
                ],
                image_index: None,
            })?;
        }
        Ok(EventResponse::Handled)
    });
    let list_remove = list;
    ui.on_widget_event(remove.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            for id in list_remove.selected_items()? {
                list_remove.remove_item(id)?;
            }
        }
        Ok(EventResponse::Handled)
    });
    let list_clear = list;
    ui.on_widget_event(clear.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            list_clear.clear_items()?;
        }
        Ok(EventResponse::Handled)
    });
    let list_select = list;
    ui.on_widget_event(select.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            if let Some(id) = first_row {
                list_select.set_selected(id, true)?;
                list_select.set_focused(id)?;
            }
        }
        Ok(EventResponse::Handled)
    });

    let list_edit = list;
    let cell_edit_for_apply = cell_edit;
    ui.on_widget_event(apply_cell.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            if let Some(id) = list_edit.selected_items()?.first().copied() {
                let value = cell_edit_for_apply.text()?;
                list_edit.set_cell_text(id, 0, &value)?;
                log::info!("ListView updated row {id:?} name to {value:?}");
            }
        }
        Ok(EventResponse::Handled)
    });
    ui.on_widget_event(LIST_ID, move |event| {
        match event {
            ControlEvent::ListViewClicked {
                control,
                row_index,
                column,
                double_click,
                position,
                ..
            } if control == LIST_ID => {
                log::info!(
                    "ListView clicked: row={row_index:?}, column={column:?}, double_click={double_click}, position={position:?}"
                );
            }
            ControlEvent::ListViewItemChanged {
                control,
                item,
                row_index,
                selected,
            } if control == LIST_ID => {
                let item = item.or_else(|| {
                    row_index
                        .and_then(|index| list.item_id_at(index).ok().flatten())
                });
                if selected {
                    if let Some(id) = item {
                        cell_edit.set_text(&list.cell_text(id, 0)?)?;
                    }
                }
                let cells = item.and_then(|id| {
                    (0..3)
                        .map(|column| list.cell_text(id, column).ok())
                        .collect::<Option<Vec<_>>>()
                });
                let state = item.and_then(|id| list.item_state(id).ok());
                log::info!(
                    "ListView item changed: id={item:?}, row={row_index:?}, selected={selected}, state={state:?}, cells={cells:?}"
                );
            }
            ControlEvent::ListViewColumnClicked { control, column }
                if control == LIST_ID =>
            {
                log::info!("ListView column clicked: {column}");
                list.sort_by_column(column, false)?;
            }
            _ => (),
        }
        Ok(EventResponse::Ignore)
    });
    window.apply_layout()?;
    Ok(())
}
