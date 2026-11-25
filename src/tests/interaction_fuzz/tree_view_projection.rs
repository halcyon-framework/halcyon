//! SPEC-HAL-346 deterministic TreeView projection fixture.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::math::RectF;
use crate::engines::cell_source::{Cell, CellSource, PropertyValue};
use crate::engines::hierarchical::NodeMeta;
use crate::widgets::tabular_view::{Column, ColumnId, ColumnModel, ColumnScrollDomain};
use crate::widgets::tree_view::{
    TreeView, TreeViewBody, TreeViewColumnProbeEntry, TreeViewDisclosureHitZoneProbeEntry,
    TreeViewIndentationGeometryProbeEntry, TreeViewRealizedRowProbeEntry,
    TreeViewStructuralProbeEntry, TreeViewVisibleProjectionProbeEntry,
};

pub const TREE_VIEW_PROJECTION_WINDOW_WIDTH: f32 = 640.0;
pub const TREE_VIEW_PROJECTION_WINDOW_HEIGHT: f32 = 320.0;
pub const TREE_VIEW_PROJECTION_ROW_COUNT: usize = 5;
pub const TREE_VIEW_PROJECTION_TREE_COLUMN_WIDTH: f32 = 260.0;
pub const TREE_VIEW_PROJECTION_VALUE_COLUMN_WIDTH: f32 = 180.0;
pub const TREE_VIEW_PROJECTION_STATUS_COLUMN_WIDTH: f32 = 120.0;
const TREE_VIEW_PROJECTION_FOOTER_HEIGHT: f32 = 24.0;
const TREE_VIEW_PROJECTION_ROW_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreeViewProjectionFixtureColumns {
    pub tree: ColumnId,
    pub value: ColumnId,
    pub status: ColumnId,
}

#[derive(Debug, Clone)]
pub(crate) struct TreeViewProjectionProbeSnapshot {
    pub structure: TreeViewStructuralProbeEntry,
    pub columns: Vec<TreeViewColumnProbeEntry>,
    pub realized_rows: Vec<TreeViewRealizedRowProbeEntry>,
    pub visible_projection: Vec<TreeViewVisibleProjectionProbeEntry>,
    pub disclosure_hit_zones: Vec<TreeViewDisclosureHitZoneProbeEntry>,
    pub indentation_geometry: Vec<TreeViewIndentationGeometryProbeEntry>,
}

pub(crate) struct TreeViewProjectionFixture {
    pub tree_view: TreeView,
    pub body: TreeViewBody,
    pub columns: TreeViewProjectionFixtureColumns,
    pub window_width: f32,
    pub window_height: f32,
}

#[derive(Debug, Clone)]
struct TreeViewProjectionFixtureSource {
    cells: HashMap<(u64, ColumnId), Cell>,
    revision: u64,
}

impl TreeViewProjectionFixtureSource {
    fn new(columns: TreeViewProjectionFixtureColumns) -> Self {
        let mut cells = HashMap::new();
        for (node_id, value, status) in [
            (10, "project", "active"),
            (20, "source", "active"),
            (30, "main.rs", "clean"),
            (40, "lib.rs", "dirty"),
            (50, "tests", "queued"),
        ] {
            cells.insert(
                (node_id, columns.value),
                Cell::read_only(PropertyValue::String(value.to_string())),
            );
            cells.insert(
                (node_id, columns.status),
                Cell::read_only(PropertyValue::String(status.to_string())),
            );
        }
        Self { cells, revision: 1 }
    }
}

impl CellSource for TreeViewProjectionFixtureSource {
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<Cell> {
        self.cells.get(&(row_id, column_id)).cloned()
    }

    fn commit_cell(&mut self, row_id: u64, column_id: ColumnId, new_value: PropertyValue) -> bool {
        let Some(cell) = self.cells.get_mut(&(row_id, column_id)) else {
            return false;
        };
        cell.value = new_value;
        self.revision += 1;
        true
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

fn tree_view_projection_nodes() -> Vec<NodeMeta> {
    vec![
        NodeMeta {
            node_id: 10,
            parent_index: None,
            depth: 0,
            has_children: true,
            label: "Project".to_string(),
        },
        NodeMeta {
            node_id: 20,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: "src".to_string(),
        },
        NodeMeta {
            node_id: 30,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "main.rs".to_string(),
        },
        NodeMeta {
            node_id: 40,
            parent_index: Some(1),
            depth: 2,
            has_children: false,
            label: "lib.rs".to_string(),
        },
        NodeMeta {
            node_id: 50,
            parent_index: Some(0),
            depth: 1,
            has_children: false,
            label: "tests".to_string(),
        },
    ]
}

pub(crate) fn tree_view_projection_fixture() -> TreeViewProjectionFixture {
    let tree_column = Column::new(TREE_VIEW_PROJECTION_TREE_COLUMN_WIDTH)
        .with_scroll_domain(ColumnScrollDomain::PinnedLeft);
    let value_column = Column::new(TREE_VIEW_PROJECTION_VALUE_COLUMN_WIDTH);
    let status_column = Column::new(TREE_VIEW_PROJECTION_STATUS_COLUMN_WIDTH);
    let columns = TreeViewProjectionFixtureColumns {
        tree: tree_column.id,
        value: value_column.id,
        status: status_column.id,
    };

    let mut model = ColumnModel::new();
    model.push(tree_column);
    model.push(value_column);
    model.push(status_column);

    let source: Rc<RefCell<dyn CellSource>> =
        Rc::new(RefCell::new(TreeViewProjectionFixtureSource::new(columns)));

    let mut tree_view = TreeView::new();
    tree_view.set_tree_data(tree_view_projection_nodes());
    tree_view.set_columns(model);
    tree_view.set_tree_column(columns.tree);
    tree_view.set_column_title(columns.tree, "Node");
    tree_view.set_column_title(columns.value, "Value");
    tree_view.set_column_title(columns.status, "Status");
    tree_view.set_row_height(TREE_VIEW_PROJECTION_ROW_HEIGHT);
    tree_view.set_footer_height(TREE_VIEW_PROJECTION_FOOTER_HEIGHT);
    tree_view.set_cell_source(Some(source));

    let body = tree_view.create_body_widget();
    TreeViewProjectionFixture {
        tree_view,
        body,
        columns,
        window_width: TREE_VIEW_PROJECTION_WINDOW_WIDTH,
        window_height: TREE_VIEW_PROJECTION_WINDOW_HEIGHT,
    }
}

pub(crate) fn laid_out_tree_view_projection_probe_snapshot() -> TreeViewProjectionProbeSnapshot {
    use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
    use crate::core::layout::Constraints;
    use crate::core::math::Size;
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::scroll_area::ScrollArea;

    let _guard = LayoutAuthorityGuard::enter();
    let mut fixture = tree_view_projection_fixture();
    let footer = fixture.tree_view.create_footer_widget();
    let horizontal_scroll_link = fixture.tree_view.horizontal_scroll_link();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(Box::new(fixture.tree_view), None, 0);
    let scroll_area = ScrollArea::new_unwired()
        .enable_horizontal_scrollbar(true)
        .with_horizontal_scroll_link(horizontal_scroll_link);
    let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
    let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
    tree.node_mut(scroll_area_id)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body_id);
    tree.add_child(root, Box::new(footer), None, 0);

    test_begin_frame_layout_tracking();
    layout_tree(
        &mut tree,
        root,
        Constraints::tight(Size::new(
            TREE_VIEW_PROJECTION_WINDOW_WIDTH,
            TREE_VIEW_PROJECTION_WINDOW_HEIGHT,
        )),
        &theme,
        &mut text_engine,
        None,
    );

    let tree_view = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<TreeView>()
        .unwrap();
    let body = tree
        .node(body_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<TreeViewBody>()
        .unwrap();

    TreeViewProjectionProbeSnapshot {
        structure: tree_view.test_structural_probe_entry(),
        columns: tree_view.test_column_probe_entries(),
        realized_rows: body.test_realized_row_probe_entries(),
        visible_projection: body.test_visible_projection_probe_entries(),
        disclosure_hit_zones: body.test_disclosure_hit_zone_probe_entries(),
        indentation_geometry: body.test_indentation_geometry_probe_entries(),
    }
}

#[test]
fn tree_view_basic_projection_probe_exposes_tabular_and_hierarchy_state() {
    let snapshot = laid_out_tree_view_projection_probe_snapshot();

    assert_eq!(
        snapshot.structure.root_rect,
        RectF::new(
            0.0,
            0.0,
            TREE_VIEW_PROJECTION_WINDOW_WIDTH,
            TREE_VIEW_PROJECTION_WINDOW_HEIGHT
        )
    );
    assert_eq!(snapshot.structure.header_rect.height, 24.0);
    assert_eq!(
        snapshot.structure.body_rect.height,
        TREE_VIEW_PROJECTION_WINDOW_HEIGHT - 24.0 - TREE_VIEW_PROJECTION_FOOTER_HEIGHT
    );
    assert_eq!(snapshot.structure.footer_rect.height, 24.0);
    assert_eq!(snapshot.structure.resize_state, "idle");

    assert_eq!(snapshot.columns.len(), 3);
    assert_eq!(snapshot.columns[0].display_index, 0);
    assert_eq!(
        snapshot.columns[0].scroll_domain,
        ColumnScrollDomain::PinnedLeft
    );
    assert!(snapshot.columns.iter().all(|column| column.visible));
    assert!(
        snapshot
            .columns
            .iter()
            .all(|column| column.resolved_width.is_some())
    );

    assert_eq!(snapshot.realized_rows.len(), 5);
    assert_eq!(
        snapshot
            .visible_projection
            .iter()
            .map(|row| row.node_id)
            .collect::<Vec<_>>(),
        vec![10, 20, 30, 40, 50]
    );
    assert_eq!(snapshot.visible_projection[2].depth, 2);
    assert_eq!(snapshot.visible_projection[2].label, "main.rs");

    assert_eq!(snapshot.structure.expanded_node_ids, vec![10, 20]);
    assert_eq!(
        snapshot
            .disclosure_hit_zones
            .iter()
            .map(|zone| zone.node_id)
            .collect::<Vec<_>>(),
        vec![10, 20]
    );

    let grandchild_indent = snapshot
        .indentation_geometry
        .iter()
        .find(|entry| entry.node_id == 30)
        .expect("grandchild indentation must be recorded");
    assert_eq!(grandchild_indent.depth, 2);
    assert_eq!(grandchild_indent.indent_x, 40.0);

    let root_tree_cell = snapshot.realized_rows[0]
        .cells
        .iter()
        .find(|cell| cell.column_id == snapshot.columns[0].column_id)
        .expect("tree column cell must be recorded");
    assert_eq!(root_tree_cell.content_state.as_deref(), Some("Project"));

    let root_value_cell = snapshot.realized_rows[0]
        .cells
        .iter()
        .find(|cell| cell.column_id == snapshot.columns[1].column_id)
        .expect("value column cell must be recorded");
    assert_eq!(root_value_cell.content_state.as_deref(), Some("project"));
}
