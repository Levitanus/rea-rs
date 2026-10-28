use rea_rs::{
    swell_gui::{layout::WidgetSize, widgets::TreeViewOptions},
    ControlEvent, EventResponse, ReaperWindow, SwellId,
};

const TREE_ID: SwellId = SwellId(0x7401);
const ADD_CHILD_ID: SwellId = SwellId(0x7402);
const ADD_ROOT_ID: SwellId = SwellId(0x7403);
const REMOVE_ID: SwellId = SwellId(0x7404);

pub(super) fn build(window: &ReaperWindow) -> anyhow::Result<()> {
    let ui = window.build_ui()?;
    let root_row = ui.row(SwellId(0x7400), WidgetSize::new_fill_x(620, 34))?;
    let add_child = root_row.button(
        ADD_CHILD_ID,
        "Add child",
        WidgetSize::new(110, 28),
    )?;
    let add_root =
        root_row.button(ADD_ROOT_ID, "Add root", WidgetSize::new(110, 28))?;
    let remove = root_row.button(
        REMOVE_ID,
        "Remove selected",
        WidgetSize::new(150, 28),
    )?;
    let tree = ui.tree_view(
        TREE_ID,
        WidgetSize::new_fill_both(620, 320).set_min_y(180),
        TreeViewOptions::HAS_BUTTONS
            | TreeViewOptions::HAS_LINES
            | TreeViewOptions::LINES_AT_ROOT,
    )?;

    let instruments =
        tree.insert_node_with_children(None, "Instruments", true)?;
    let drums = tree.insert_node_with_children(None, "Drums", true)?;
    let strings =
        tree.insert_node_with_children(Some(instruments), "Strings", true)?;
    tree.insert_node(Some(instruments), "Keys")?;
    tree.insert_node(Some(strings), "Electric guitar")?;
    tree.insert_node(Some(drums), "Kick")?;
    tree.insert_node(Some(drums), "Snare")?;
    let tree_initial = tree;
    for id in [instruments, drums, strings] {
        tree_initial.set_expanded(id, true)?;
    }
    for id in [instruments, drums, strings] {
        tree.set_expanded(id, true)?;
    }
    for id in [instruments, drums, strings] {
        log::debug!(
            "TreeView demo initialized node: id={id:?}, label={:?}",
            tree_initial.node_text(id)?
        );
    }

    let tree_add_child = tree;
    ui.on_widget_event(add_child.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            let parent = tree_add_child.selected_item()?;
            log::debug!("TreeView add child: selected parent={parent:?}");
            let label = format!(
                "Child {}",
                tree_add_child.descendants(None)?.len() + 1
            );
            let id = tree_add_child
                .insert_node_with_children(parent, &label, false)?;
            if let Some(parent) = parent {
                tree_add_child.set_has_children(parent, true)?;
                tree_add_child.set_expanded(parent, true)?;
            }
            tree_add_child.set_selected(id)?;
        }
        Ok(EventResponse::Handled)
    });
    let tree_add_root = tree;
    ui.on_widget_event(add_root.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            let label =
                format!("Root {}", tree_add_root.descendants(None)?.len() + 1);
            let id = tree_add_root.insert_node(None, &label)?;
            tree_add_root.set_selected(id)?;
        }
        Ok(EventResponse::Handled)
    });
    let tree_remove = tree;
    ui.on_widget_event(remove.id(), move |event| {
        if matches!(event, ControlEvent::ButtonClicked { .. }) {
            if let Some(id) = tree_remove.selected_item()? {
                tree_remove.remove_node(id)?;
            }
        }
        Ok(EventResponse::Handled)
    });
    ui.on_widget_event(TREE_ID, move |event| {
        match event {
            ControlEvent::TreeSelectionChanged { item, .. } => {
                log::info!("TreeView selected node: {item:?}");
            }
            ControlEvent::TreeItemExpanding {
                item: Some(item),
                expanding,
                ..
            } => {
                log::info!("TreeView native expand action: item={item:?}, expanding={expanding}");
            }
            ControlEvent::TreeClick { row_index, double_click, .. } => {
                log::debug!("TreeView click: row={row_index:?}, double_click={double_click}");
            }
            _ => (),
        }
        Ok(EventResponse::Ignore)
    });
    window.apply_layout()?;
    Ok(())
}
