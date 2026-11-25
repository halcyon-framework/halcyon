// Showcase — List View tab content.
//
// Ports examples/listview.rs (the authoritative ListView demo, SPEC-HAL-179/179A)
// into the showcase, substituting `parent` (the showcase's content_node) for the
// standalone's root Flex column:
//
//   content_node (passed in by ShowcaseScreen::build)
//     +- ListView (flex 1.0)
//          +- ScrollArea (horizontal scrollbar enabled, scroll-link + freeze wired)
//          |    +- ListViewBody
//          +- ListViewFooter (0-height structural no-op)
//
// Six columns (Index pinned-left, Name, Description, Status, Choice, Category)
// total 1150px, exceeding the tab content area on a default-sized window, so the
// horizontal scrollbar engages and header/body move in lockstep via the
// HorizontalScrollLink. ROW_COUNT rows prove virtualization (only the visible rows
// are realized). Data is synchronous, in-memory, and deterministic
// (DemoDataSource, SPEC-HAL-188), so the showcase is reproducible across runs.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use super::ShowcaseScreen;
use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::widgets::list_view::ListView;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::ColumnId;
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, ColumnInvalidationScope, DataInvalidationEvent, DataSource,
    Extent, UniverseMode, ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use halcyon::widgets::tabular_view::{Column, ColumnScrollDomain};

// Number of rows (proves virtualization at scale).
const ROW_COUNT: usize = 10_000;

// Footer remains mounted for structural resize completeness, but is hidden in
// the showcase because it has no content of its own.
const FOOTER_HEIGHT: f32 = 0.0;

// Column widths. Total 80 + 200 + 400 + 150 + 120 + 200 = 1150px > tab content width
// on a default window -> horizontal scroll engages.
const COL_INDEX_WIDTH: f32 = 80.0;
const COL_NAME_WIDTH: f32 = 200.0;
const COL_DESC_WIDTH: f32 = 400.0;
const COL_STATUS_WIDTH: f32 = 150.0;
const COL_CHOICE_WIDTH: f32 = 120.0;
const COL_CATEGORY_WIDTH: f32 = 200.0;

// Column identifiers, set after ListView configures its columns.
#[derive(Default)]
struct DemoColumns {
    index: Option<ColumnId>,
    name: Option<ColumnId>,
    description: Option<ColumnId>,
    status: Option<ColumnId>,
    choice: Option<ColumnId>,
    category: Option<ColumnId>,
}

// In-memory, deterministic data source (SPEC-HAL-188).
struct DemoDataSource {
    rows: Vec<RowId>,
    // O(1) RowId -> position lookup. The framework re-queries every visible
    // cell per frame, so an O(N) lookup here would freeze at scale.
    index_by_id: HashMap<RowId, usize>,
    columns: DemoColumns,
    choice_overrides: HashMap<RowId, String>,
    pending_events: Vec<DataInvalidationEvent>,
}

impl DemoDataSource {
    fn new() -> Self {
        let rows: Vec<RowId> = (0..ROW_COUNT).map(|_| RowId::new()).collect();
        let index_by_id: HashMap<RowId, usize> =
            rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
        Self {
            rows,
            index_by_id,
            columns: DemoColumns::default(),
            choice_overrides: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    fn set_columns(
        &mut self,
        index: ColumnId,
        name: ColumnId,
        description: ColumnId,
        status: ColumnId,
        choice: ColumnId,
        category: ColumnId,
    ) {
        self.columns.index = Some(index);
        self.columns.name = Some(name);
        self.columns.description = Some(description);
        self.columns.status = Some(status);
        self.columns.choice = Some(choice);
        self.columns.category = Some(category);
    }

    fn row_index(&self, row_id: RowId) -> Option<usize> {
        self.index_by_id.get(&row_id).copied()
    }
}

fn choice_options() -> Vec<(String, String)> {
    ["Alpha", "Beta", "Gamma", "Delta"]
        .into_iter()
        .map(|value| (value.to_string(), value.to_string()))
        .collect()
}

impl DataSource for DemoDataSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(idx) = self.row_index(row) else {
            return ValueState::Missing;
        };

        if Some(column) == self.columns.index {
            ValueState::Ready(CellValue::Int(idx as i64))
        } else if Some(column) == self.columns.name {
            ValueState::Ready(CellValue::String(format!("Item {}", idx)))
        } else if Some(column) == self.columns.description {
            ValueState::Ready(CellValue::String(format!(
                "Description for row {} - demo data with some extra text",
                idx
            )))
        } else if Some(column) == self.columns.status {
            let status = match idx % 3 {
                0 => "Active",
                1 => "Pending",
                _ => "Archived",
            };
            ValueState::Ready(CellValue::String(status.to_string()))
        } else if Some(column) == self.columns.choice {
            let choice =
                self.choice_overrides
                    .get(&row)
                    .cloned()
                    .unwrap_or_else(|| match idx % 4 {
                        0 => "Alpha".to_string(),
                        1 => "Beta".to_string(),
                        2 => "Gamma".to_string(),
                        _ => "Delta".to_string(),
                    });
            ValueState::Ready(CellValue::String(choice))
        } else if Some(column) == self.columns.category {
            let category = match idx % 4 {
                0 => "Engineering",
                1 => "Marketing",
                2 => "Finance",
                _ => "Operations",
            };
            ValueState::Ready(CellValue::String(category.to_string()))
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, column: ColumnId) -> CellCapabilities {
        if Some(column) == self.columns.choice {
            CellCapabilities {
                visible: true,
                readable: true,
                editable: true,
                committable: true,
                redacted: false,
            }
        } else {
            CellCapabilities::default()
        }
    }

    fn commit_cell(&mut self, row: RowId, column: ColumnId, value: CellValue) -> bool {
        if Some(column) != self.columns.choice {
            return false;
        }

        let CellValue::String(choice) = value else {
            return false;
        };

        self.choice_overrides.insert(row, choice);
        self.pending_events
            .push(DataInvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnInvalidationScope::Specific(vec![column]),
            });
        true
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

impl ShowcaseScreen {
    /// Build the List View tab content.
    ///
    /// Mirrors examples/listview.rs:233-314, substituting `parent` (the
    /// showcase's content_node) for the standalone's root Flex column.
    pub(super) fn build_list_view(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let source = DemoDataSource::new();

        let mut list_view = ListView::new(source);

        // Index is pinned left -- fixed during horizontal scroll.
        let col_index = list_view.add_column_raw(
            "Index",
            Column::new(COL_INDEX_WIDTH).with_scroll_domain(ColumnScrollDomain::PinnedLeft),
        );
        let col_name = list_view.add_column("Name", COL_NAME_WIDTH);
        let col_desc = list_view.add_column("Description", COL_DESC_WIDTH);
        let col_status = list_view.add_column("Status", COL_STATUS_WIDTH);
        let col_choice = list_view.add_column("Choice", COL_CHOICE_WIDTH);
        let col_category = list_view.add_column("Category", COL_CATEGORY_WIDTH);
        list_view.set_column_combo_options(col_choice, choice_options());

        list_view.adapter_mut().source_mut().set_columns(
            col_index,
            col_name,
            col_desc,
            col_status,
            col_choice,
            col_category,
        );

        list_view.set_footer_height(FOOTER_HEIGHT);

        // Horizontal scroll link + freeze: header/body move in lockstep.
        let hscroll_link = HorizontalScrollLink::new();
        let hscroll_freeze = Rc::new(Cell::new(false));
        list_view.set_horizontal_scroll_link(hscroll_link.clone(), hscroll_freeze.clone());

        // Body + footer share Rc state with the ListView -- create both before
        // the ListView moves into the tree.
        let body_widget = list_view.create_body_widget();
        let footer_widget = list_view.create_footer_widget();

        // ListView fills the tab's content area.
        let list_view_id = cx.tree.add_child(parent, Box::new(list_view), None, 0);
        cx.tree.set_flex(list_view_id, FlexParams::flexible(1.0));

        // ScrollArea is the first child of ListView: horizontal scrollbar on,
        // scroll link + freeze wired so the header tracks body h-scroll.
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze);
        let scroll_area_id = cx
            .tree
            .add_child(list_view_id, Box::new(scroll_area), None, 0);

        // ListViewBody is the content of the ScrollArea.
        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        // Wire ScrollArea to its content.
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        // Footer is the second child of ListView.
        cx.tree
            .add_child(list_view_id, Box::new(footer_widget), None, 0);
    }
}
