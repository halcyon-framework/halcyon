//! Mount a TreeView showing the folders and files of a fictional project.
//! Run with `cargo run --example treeview_basic`.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget::Widget;
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::prelude::{Flex, Label};
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::{Column, ColumnId, ColumnModel};
use halcyon::widgets::tree_view::TreeView;

fn project_nodes() -> Vec<NodeMeta> {
    // Node IDs stay distinct from the parent indices in this preorder data.
    [
        (100, None, 0, true, "Project"),
        (200, Some(0), 1, true, "src"),
        (201, Some(1), 2, false, "main.rs"),
        (202, Some(1), 2, false, "app.rs"),
        (300, Some(0), 1, true, "assets"),
        (301, Some(4), 2, false, "logo.svg"),
        (400, Some(0), 1, true, "tests"),
        (401, Some(6), 2, false, "app_test.rs"),
        (102, Some(0), 1, false, "README.md"),
    ]
    .into_iter()
    .map(
        |(node_id, parent_index, depth, has_children, label)| NodeMeta {
            node_id,
            parent_index,
            depth,
            has_children,
            label: label.to_owned(),
        },
    )
    .collect()
}

fn main() {
    App::new(AppConfig::new("Halcyon - TreeView Basics").with_size(520.0, 360.0)).run(
        ProjectTree {
            placeholder: Label::new(""),
            rebuild: true,
        },
    );
}

struct ProjectTree {
    placeholder: Label,
    rebuild: bool,
}

impl Screen for ProjectTree {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let mut tree = TreeView::new();
        tree.set_tree_data(project_nodes());

        let name = ColumnId::from_raw(1);
        let mut columns = ColumnModel::new();
        columns.push(Column::with_id(name, 460.0));
        tree.set_columns(columns);
        tree.set_tree_column(name);
        tree.set_column_title(name, "Name");
        tree.expand(0); // The Project root is the first row.

        let scroll = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(tree.horizontal_scroll_link())
            .with_horizontal_freeze(tree.horizontal_scroll_freeze())
            .with_content_owned_vertical_translation();
        let body = tree.create_body_widget();
        let footer = tree.create_footer_widget();

        let root = cx.mount_child(Box::new(Flex::column()));
        let tree = cx.tree.add_child(root, Box::new(tree), None, 0);
        cx.tree.set_flex(tree, FlexParams::flexible(1.0));
        let scroll = cx.tree.add_child(tree, Box::new(scroll), None, 0);
        let body = cx.tree.add_child(scroll, Box::new(body), None, 0);
        cx.tree
            .node_mut(scroll)
            .expect("mounted tree body ScrollArea")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .expect("TreeView body is a ScrollArea")
            .wire_content(body);
        cx.tree.add_child(tree, Box::new(footer), None, 0);
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}
