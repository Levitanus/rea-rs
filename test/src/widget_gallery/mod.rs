//! Focused ListView and TreeView demos, opened by a separate REAPER action.

mod list_view;
mod tree_view;

use rea_rs::{Reaper, ReaperWindow, WindowHandler, WindowId, WindowSpec};

const LIST_WINDOW_ID: &str = "rea-rs.widget-gallery.list-view";
const TREE_WINDOW_ID: &str = "rea-rs.widget-gallery.tree-view";

struct GalleryWindow {
    window: ReaperWindow,
    kind: GalleryKind,
}

#[derive(Clone, Copy)]
enum GalleryKind {
    ListView,
    TreeView,
}

impl GalleryWindow {
    fn open(kind: GalleryKind) -> anyhow::Result<()> {
        let id = match kind {
            GalleryKind::ListView => LIST_WINDOW_ID,
            GalleryKind::TreeView => TREE_WINDOW_ID,
        };
        if Reaper::get().is_window_registered(&id.to_string()) {
            Reaper::get_mut().unregister_window_handler(&id.to_string())?;
            return Ok(());
        }

        let title = match kind {
            GalleryKind::ListView => "ListView Demo",
            GalleryKind::TreeView => "TreeView Demo",
        };
        let window = Reaper::get().create_window(
            &WindowSpec::new(title).size(640, 400).min_size(420, 260),
        )?;
        match kind {
            GalleryKind::ListView => list_view::build(&window)?,
            GalleryKind::TreeView => tree_view::build(&window)?,
        };
        Reaper::get_mut()
            .register_window_handler(Box::new(Self { window, kind }))?;
        Ok(())
    }
}

impl WindowHandler for GalleryWindow {
    fn window_id(&self) -> WindowId {
        match self.kind {
            GalleryKind::ListView => LIST_WINDOW_ID,
            GalleryKind::TreeView => TREE_WINDOW_ID,
        }
        .to_string()
    }

    fn window(&self) -> &ReaperWindow {
        &self.window
    }
}

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_action(
        "TestSwellGuiListViewDemo",
        "test swell-gui ListView demo",
        rea_rs::ActionKind::NotToggleable,
        |_| GalleryWindow::open(GalleryKind::ListView),
        None,
    )?;
    reaper.register_action(
        "TestSwellGuiTreeViewDemo",
        "test swell-gui TreeView demo",
        rea_rs::ActionKind::NotToggleable,
        |_| GalleryWindow::open(GalleryKind::TreeView),
        None,
    )?;
    Ok(())
}
