//! SPEC-HAL-346 deterministic ListView resize fixture.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use serde::Serialize;

use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
use crate::core::event::{InteractionDomainState, PointerId};
use crate::core::event_router::EventRouter;
use crate::core::interaction_fuzz::{
    InteractionCommand, InteractionDriver, InteractionFailureLayer, InteractionOracleEvaluation,
    InteractionOracleStatus, InteractionProbeSnapshotArtifact, InteractionSettleBoundary,
};
use crate::core::layout::Constraints;
use crate::core::math::RectF;
use crate::core::math::Size;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::theme::Theme;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::tabular_view::{
    CellCapabilities, CellValue, Column, ColumnId, ColumnScrollDomain, DataInvalidationEvent,
    DataSource, Extent, RowId, UniverseMode, ValueState,
};

use crate::widgets::list_view::{ListView, ListViewBody, ListViewFooter};
use crate::widgets::scroll_area::ScrollArea;

pub const LIST_VIEW_RESIZE_ROW_COUNT: usize = 1_000;
pub const LIST_VIEW_RESIZE_WINDOW_WIDTH: f32 = 800.0;
pub const LIST_VIEW_RESIZE_WINDOW_HEIGHT: f32 = 800.0;
pub const LIST_VIEW_RESIZE_FOOTER_HEIGHT: f32 = 24.0;
pub const LIST_VIEW_RESIZE_COL_INDEX_WIDTH: f32 = 80.0;
pub const LIST_VIEW_RESIZE_COL_NAME_WIDTH: f32 = 200.0;
pub const LIST_VIEW_RESIZE_COL_DESCRIPTION_WIDTH: f32 = 400.0;
pub const LIST_VIEW_RESIZE_COL_STATUS_WIDTH: f32 = 150.0;
pub const LIST_VIEW_RESIZE_COL_CATEGORY_WIDTH: f32 = 200.0;
pub const LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL: f32 = 115.0;
pub const LIST_VIEW_RESIZE_SEPARATOR_HIT_HALF_WIDTH: f32 = 4.0;
pub const ORACLE_346_R2_ID: &str = "ORACLE-346-R2";
pub const ORACLE_346_R2_NAME: &str = "Direction Monotonicity";
pub const ORACLE_346_R4_ID: &str = "ORACLE-346-R4";
pub const ORACLE_346_R4_NAME: &str = "No Opposite-Direction First Drag";
pub const ORACLE_346_R5_ID: &str = "ORACLE-346-R5";
pub const ORACLE_346_R5_NAME: &str = "Scroll-Aware Separator Hit-Test";
pub const ORACLE_346_R12_ID: &str = "ORACLE-346-R12";
pub const ORACLE_346_R12_NAME: &str = "Repeated Drag Consistency";
pub const LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER: &str = "widget_probe";
pub const LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER: &str = "ListViewResizeProbe";
pub const LIST_VIEW_RESIZE_PROBE_SNAPSHOT_REASON: &str = "post_layout_probe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListViewResizeFixtureColumns {
    pub index: ColumnId,
    pub name: ColumnId,
    pub description: ColumnId,
    pub status: ColumnId,
    pub category: ColumnId,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeColumnProbe {
    pub column_id: ColumnId,
    pub label: &'static str,
    pub display_index: usize,
    pub visible: bool,
    pub resizable: bool,
    pub reorderable: bool,
    pub scroll_domain: ColumnScrollDomain,
    pub preferred_width: f32,
    pub resolved_x_offset: f32,
    pub resolved_width: f32,
    pub min_width: f32,
    pub max_width: Option<f32>,
    pub separator_x_viewport: Option<f32>,
    pub separator_x_content: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeSeparatorProbe {
    pub left_column_id: ColumnId,
    pub right_column_id: ColumnId,
    pub content_x: f32,
    pub viewport_x: f32,
    pub scroll_adjusted: bool,
    pub hit_half_width: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeRectProbe {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<RectF> for ListViewResizeRectProbe {
    fn from(value: RectF) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeCellProbe {
    pub column_id: ColumnId,
    pub rect: ListViewResizeRectProbe,
    pub content_state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeRealizedRowProbe {
    pub row_id: u64,
    pub data_index: usize,
    pub visual_index: usize,
    pub rect: ListViewResizeRectProbe,
    pub cells: Vec<ListViewResizeCellProbe>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeProbeSnapshot {
    pub source_layer: String,
    pub source_owner: String,
    pub layout_epoch: u64,
    pub frame_number: u64,
    pub settle_boundary: InteractionSettleBoundary,
    pub snapshot_reason: String,
    pub widget_id: u64,
    pub root_rect: ListViewResizeRectProbe,
    pub header_rect: ListViewResizeRectProbe,
    pub body_rect: ListViewResizeRectProbe,
    pub footer_rect: ListViewResizeRectProbe,
    pub horizontal_scroll: f32,
    pub vertical_scroll_offset: f32,
    pub resize_state: String,
    pub active_resize_separator: Option<ColumnId>,
    pub hovered_separator: Option<ColumnId>,
    pub pointer_in_expansion_region: bool,
    pub columns: Vec<ListViewResizeColumnProbe>,
    pub separators: Vec<ListViewResizeSeparatorProbe>,
    pub realized_rows: Vec<ListViewResizeRealizedRowProbe>,
    pub selected_rows: Vec<u64>,
    pub focused_row: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeTransactionProbe {
    pub transaction_index: usize,
    pub column_id: ColumnId,
    pub column_label: &'static str,
    pub dx: f32,
    pub width_before: f32,
    pub width_after: f32,
    pub clamp_reached: bool,
    pub clamp_boundary: Option<f32>,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListViewResizeSeparatorHitTestProbe {
    pub separator_index: usize,
    pub left_column_id: ColumnId,
    pub scroll_domain: ColumnScrollDomain,
    pub viewport_local_x: f32,
    pub horizontal_scroll_offset: f32,
    pub observed_test_x: f32,
    pub authority: &'static str,
}

#[derive(Debug)]
pub struct ListViewResizeFixtureSource {
    rows: Vec<RowId>,
    index_by_id: HashMap<RowId, usize>,
    columns: Option<ListViewResizeFixtureColumns>,
    pending_events: Vec<DataInvalidationEvent>,
}

impl ListViewResizeFixtureSource {
    pub fn new(row_count: usize) -> Self {
        let rows: Vec<RowId> = (0..row_count)
            .map(|index| RowId::from_raw(index as u64 + 1))
            .collect();
        let index_by_id = rows
            .iter()
            .enumerate()
            .map(|(index, row_id)| (*row_id, index))
            .collect();

        Self {
            rows,
            index_by_id,
            columns: None,
            pending_events: Vec::new(),
        }
    }

    pub fn set_columns(&mut self, columns: ListViewResizeFixtureColumns) {
        self.columns = Some(columns);
    }

    pub fn columns(&self) -> Option<ListViewResizeFixtureColumns> {
        self.columns
    }

    pub fn row_index(&self, row_id: RowId) -> Option<usize> {
        self.index_by_id.get(&row_id).copied()
    }
}

impl DataSource for ListViewResizeFixtureSource {
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
        let Some(index) = self.row_index(row) else {
            return ValueState::Missing;
        };
        let Some(columns) = self.columns else {
            return ValueState::Missing;
        };

        if column == columns.index {
            ValueState::Ready(CellValue::Int(index as i64))
        } else if column == columns.name {
            ValueState::Ready(CellValue::String(format!("Item {index}")))
        } else if column == columns.description {
            ValueState::Ready(CellValue::String(format!(
                "Description for row {index} - resize fixture data"
            )))
        } else if column == columns.status {
            let status = match index % 3 {
                0 => "Active",
                1 => "Pending",
                _ => "Archived",
            };
            ValueState::Ready(CellValue::String(status.to_string()))
        } else if column == columns.category {
            let category = match index % 4 {
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

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

pub struct ListViewResizeFixture {
    pub list_view: ListView<ListViewResizeFixtureSource>,
    pub body: ListViewBody<ListViewResizeFixtureSource>,
    pub footer: ListViewFooter<ListViewResizeFixtureSource>,
    pub horizontal_scroll_link: HorizontalScrollLink,
    pub horizontal_scroll_freeze: Rc<Cell<bool>>,
    pub columns: ListViewResizeFixtureColumns,
    pub window_width: f32,
    pub window_height: f32,
}

impl ListViewResizeFixture {
    pub fn new() -> Self {
        Self::with_row_count(LIST_VIEW_RESIZE_ROW_COUNT)
    }

    pub fn with_row_count(row_count: usize) -> Self {
        Self::with_row_count_and_name_flex(row_count, false)
    }

    pub fn with_flex_name() -> Self {
        Self::with_row_count_and_name_flex(LIST_VIEW_RESIZE_ROW_COUNT, true)
    }

    fn with_row_count_and_name_flex(row_count: usize, name_flex: bool) -> Self {
        let source = ListViewResizeFixtureSource::new(row_count);
        let mut list_view = ListView::new(source);
        let name_column = if name_flex {
            Column::new(LIST_VIEW_RESIZE_COL_NAME_WIDTH).with_flex(1.0)
        } else {
            Column::new(LIST_VIEW_RESIZE_COL_NAME_WIDTH)
        };
        let description_width = if name_flex {
            160.0
        } else {
            LIST_VIEW_RESIZE_COL_DESCRIPTION_WIDTH
        };
        let status_width = if name_flex {
            120.0
        } else {
            LIST_VIEW_RESIZE_COL_STATUS_WIDTH
        };
        let category_width = if name_flex {
            120.0
        } else {
            LIST_VIEW_RESIZE_COL_CATEGORY_WIDTH
        };

        let columns = ListViewResizeFixtureColumns {
            index: list_view.add_column_raw(
                "Index",
                Column::new(LIST_VIEW_RESIZE_COL_INDEX_WIDTH)
                    .with_scroll_domain(ColumnScrollDomain::PinnedLeft),
            ),
            name: list_view.add_column_raw("Name", name_column),
            description: list_view.add_column("Description", description_width),
            status: list_view.add_column("Status", status_width),
            category: list_view.add_column("Category", category_width),
        };

        list_view.adapter_mut().source_mut().set_columns(columns);
        list_view.set_footer_height(LIST_VIEW_RESIZE_FOOTER_HEIGHT);

        let horizontal_scroll_link = HorizontalScrollLink::new();
        let horizontal_scroll_freeze = Rc::new(Cell::new(false));
        list_view.set_horizontal_scroll_link(
            horizontal_scroll_link.clone(),
            horizontal_scroll_freeze.clone(),
        );

        let body = list_view.create_body_widget();
        let footer = list_view.create_footer_widget();

        Self {
            list_view,
            body,
            footer,
            horizontal_scroll_link,
            horizontal_scroll_freeze,
            columns,
            window_width: LIST_VIEW_RESIZE_WINDOW_WIDTH,
            window_height: LIST_VIEW_RESIZE_WINDOW_HEIGHT,
        }
    }
}

pub fn list_view_resize_probe_snapshot(
    list_view: &ListView<ListViewResizeFixtureSource>,
    columns: ListViewResizeFixtureColumns,
) -> ListViewResizeProbeSnapshot {
    let structure = list_view.test_structural_probe_entry();
    let horizontal_scroll = structure.horizontal_scroll_offset;
    let columns = list_view_resize_probe_columns(list_view, columns);
    let realized_rows = list_view_resize_realized_rows(list_view, &columns);
    let mut separators = Vec::new();
    let mut content_x = 0.0;

    for index in 0..columns.len().saturating_sub(1) {
        let left = &columns[index];
        let right = &columns[index + 1];
        content_x += left.resolved_width;
        let separator_content_x = content_x - 1.0;
        let scroll_adjusted = left.scroll_domain == ColumnScrollDomain::Scrollable;
        let viewport_x = if scroll_adjusted {
            separator_content_x - horizontal_scroll
        } else {
            separator_content_x
        };

        separators.push(ListViewResizeSeparatorProbe {
            left_column_id: left.column_id,
            right_column_id: right.column_id,
            content_x: separator_content_x,
            viewport_x,
            scroll_adjusted,
            hit_half_width: LIST_VIEW_RESIZE_SEPARATOR_HIT_HALF_WIDTH,
        });
    }

    ListViewResizeProbeSnapshot {
        source_layer: LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER.to_string(),
        source_owner: LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER.to_string(),
        layout_epoch: 1,
        frame_number: 0,
        settle_boundary: InteractionSettleBoundary::Reconcile,
        snapshot_reason: LIST_VIEW_RESIZE_PROBE_SNAPSHOT_REASON.to_string(),
        widget_id: structure.widget_id,
        root_rect: ListViewResizeRectProbe::from(structure.root_rect),
        header_rect: ListViewResizeRectProbe::from(structure.header_rect),
        body_rect: ListViewResizeRectProbe::from(structure.body_rect),
        footer_rect: ListViewResizeRectProbe::from(structure.footer_rect),
        horizontal_scroll,
        vertical_scroll_offset: structure.vertical_scroll_offset,
        resize_state: structure.resize_state.to_string(),
        active_resize_separator: structure.active_resize_separator,
        hovered_separator: structure.hovered_separator,
        pointer_in_expansion_region: structure.pointer_in_expansion_region,
        columns,
        separators,
        realized_rows,
        selected_rows: structure.selected_rows,
        focused_row: structure.focused_row,
    }
}

fn list_view_resize_probe_columns(
    list_view: &ListView<ListViewResizeFixtureSource>,
    columns: ListViewResizeFixtureColumns,
) -> Vec<ListViewResizeColumnProbe> {
    list_view
        .test_column_probe_entries()
        .into_iter()
        .map(|entry| ListViewResizeColumnProbe {
            column_id: entry.column_id,
            label: list_view_resize_column_label(columns, entry.column_id),
            display_index: entry.display_index,
            visible: entry.visible,
            resizable: entry.resizable,
            reorderable: entry.reorderable,
            scroll_domain: entry.scroll_domain,
            preferred_width: entry.preferred_width,
            resolved_x_offset: entry
                .resolved_x_offset
                .expect("laid-out fixture column must expose resolved x offset"),
            resolved_width: entry
                .resolved_width
                .expect("laid-out fixture column must expose resolved width"),
            min_width: entry.min_width,
            max_width: entry.max_width,
            separator_x_viewport: entry.separator_x_viewport,
            separator_x_content: entry.separator_x_content,
        })
        .collect()
}

fn list_view_resize_realized_rows(
    list_view: &ListView<ListViewResizeFixtureSource>,
    columns: &[ListViewResizeColumnProbe],
) -> Vec<ListViewResizeRealizedRowProbe> {
    let total_width: f32 = columns
        .iter()
        .filter(|column| column.visible)
        .map(|column| column.resolved_width)
        .sum();
    let rows = list_view.test_realized_row_projections();
    let cells = list_view.test_realized_cell_probe_entries();

    rows.into_iter()
        .enumerate()
        .map(|(visual_index, row)| {
            let row_cells = cells
                .iter()
                .filter(|cell| cell.row_id == row.row_id)
                .map(|cell| ListViewResizeCellProbe {
                    column_id: cell.column_id,
                    rect: ListViewResizeRectProbe::from(cell.rect),
                    content_state: list_view_resize_content_state(&cell.content_state),
                })
                .collect();

            ListViewResizeRealizedRowProbe {
                row_id: row.row_id.raw(),
                data_index: row.row_index,
                visual_index,
                rect: ListViewResizeRectProbe::from(row.bounds(total_width)),
                cells: row_cells,
            }
        })
        .collect()
}

fn list_view_resize_column_label(
    columns: ListViewResizeFixtureColumns,
    column_id: ColumnId,
) -> &'static str {
    if column_id == columns.index {
        "Index"
    } else if column_id == columns.name {
        "Name"
    } else if column_id == columns.description {
        "Description"
    } else if column_id == columns.status {
        "Status"
    } else if column_id == columns.category {
        "Category"
    } else {
        "Unknown"
    }
}

fn list_view_resize_content_state(value: &ValueState<CellValue>) -> String {
    match value {
        ValueState::Ready(CellValue::Null) => "ready:null".to_string(),
        ValueState::Ready(CellValue::Bool(value)) => format!("ready:bool:{value}"),
        ValueState::Ready(CellValue::Int(value)) => format!("ready:int:{value}"),
        ValueState::Ready(CellValue::Float(value)) if value.is_finite() => {
            format!("ready:float:{value}")
        }
        ValueState::Ready(CellValue::Float(_)) => "ready:float:non_finite".to_string(),
        ValueState::Ready(CellValue::String(value)) => format!("ready:string:{value}"),
        ValueState::Pending => "pending".to_string(),
        ValueState::Denied { reason } => {
            format!("denied:{}", reason.as_deref().unwrap_or("unspecified"))
        }
        ValueState::Error { error } => {
            format!("error:{}", error.as_deref().unwrap_or("unspecified"))
        }
        ValueState::Missing => "missing".to_string(),
    }
}

pub fn evaluate_resize_direction_monotonicity(
    probe: ListViewResizeTransactionProbe,
) -> InteractionOracleEvaluation {
    let mut derived_values = resize_transaction_derived_values(&probe);
    let failed_requirement = if probe.clamp_reached {
        match probe.clamp_boundary {
            Some(boundary) if f32_eq(probe.width_after, boundary) => None,
            Some(_) => Some("clamped_resize_equals_boundary"),
            None => Some("clamped_resize_declares_boundary"),
        }
    } else if probe.dx > 0.0 && probe.width_after > probe.width_before {
        None
    } else if probe.dx < 0.0 && probe.width_after < probe.width_before {
        None
    } else if f32_eq(probe.dx, 0.0) && f32_eq(probe.width_after, probe.width_before) {
        None
    } else if probe.dx > 0.0 {
        Some("positive_dx_increases_width")
    } else if probe.dx < 0.0 {
        Some("negative_dx_decreases_width")
    } else {
        Some("zero_dx_preserves_width")
    };

    if let Some(requirement) = failed_requirement {
        derived_values.insert(
            "observed_width_delta".to_string(),
            (probe.width_after - probe.width_before).to_string(),
        );
        resize_oracle_failed(
            ORACLE_346_R2_ID,
            ORACLE_346_R2_NAME,
            InteractionFailureLayer::WidgetState,
            Some(format!("column:{:?}", probe.column_id)),
            requirement,
            Some(probe.width_after.to_string()),
            Some(probe.authority.to_string()),
            derived_values,
        )
    } else {
        resize_oracle_passed(ORACLE_346_R2_ID, ORACLE_346_R2_NAME, derived_values)
    }
}

pub fn evaluate_scroll_aware_separator_hit_test(
    probe: ListViewResizeSeparatorHitTestProbe,
) -> InteractionOracleEvaluation {
    let expected_test_x = match probe.scroll_domain {
        ColumnScrollDomain::Scrollable => probe.viewport_local_x + probe.horizontal_scroll_offset,
        ColumnScrollDomain::PinnedLeft | ColumnScrollDomain::PinnedRight => probe.viewport_local_x,
    };
    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "separator_index".to_string(),
        probe.separator_index.to_string(),
    );
    derived_values.insert(
        "left_column_id".to_string(),
        format!("{:?}", probe.left_column_id),
    );
    derived_values.insert(
        "scroll_domain".to_string(),
        format!("{:?}", probe.scroll_domain),
    );
    derived_values.insert(
        "viewport_local_x".to_string(),
        probe.viewport_local_x.to_string(),
    );
    derived_values.insert(
        "horizontal_scroll_offset".to_string(),
        probe.horizontal_scroll_offset.to_string(),
    );
    derived_values.insert("expected_test_x".to_string(), expected_test_x.to_string());
    derived_values.insert(
        "observed_test_x".to_string(),
        probe.observed_test_x.to_string(),
    );
    derived_values.insert("authority".to_string(), probe.authority.to_string());

    if f32_eq(probe.observed_test_x, expected_test_x) {
        resize_oracle_passed(ORACLE_346_R5_ID, ORACLE_346_R5_NAME, derived_values)
    } else {
        resize_oracle_failed(
            ORACLE_346_R5_ID,
            ORACLE_346_R5_NAME,
            InteractionFailureLayer::EventRouting,
            Some(format!("separator:{}", probe.separator_index)),
            "scroll_aware_separator_hit_test",
            Some(probe.observed_test_x.to_string()),
            Some(probe.authority.to_string()),
            derived_values,
        )
    }
}

pub fn evaluate_no_opposite_direction_first_drag(
    first: ListViewResizeTransactionProbe,
    subsequent: ListViewResizeTransactionProbe,
) -> InteractionOracleEvaluation {
    let first_direction = resize_direction_class(&first);
    let subsequent_direction = resize_direction_class(&subsequent);
    let mut derived_values = first_subsequent_derived_values(&first, &subsequent);
    derived_values.insert("first_direction".to_string(), first_direction.to_string());
    derived_values.insert(
        "subsequent_direction".to_string(),
        subsequent_direction.to_string(),
    );

    let failed_requirement = if first_direction != subsequent_direction {
        Some("first_drag_direction_matches_subsequent_drag")
    } else if first_direction == "opposite" {
        Some("first_drag_obeys_resize_direction")
    } else {
        None
    };

    if let Some(requirement) = failed_requirement {
        resize_oracle_failed(
            ORACLE_346_R4_ID,
            ORACLE_346_R4_NAME,
            InteractionFailureLayer::WidgetState,
            Some(format!("column:{:?}", first.column_id)),
            requirement,
            Some((first.width_after - first.width_before).to_string()),
            Some(first.authority.to_string()),
            derived_values,
        )
    } else {
        resize_oracle_passed(ORACLE_346_R4_ID, ORACLE_346_R4_NAME, derived_values)
    }
}

pub fn evaluate_repeated_resize_consistency(
    first: ListViewResizeTransactionProbe,
    repeated: ListViewResizeTransactionProbe,
) -> InteractionOracleEvaluation {
    let mut derived_values = first_subsequent_derived_values(&first, &repeated);
    let failed_requirement = if first.column_label != repeated.column_label {
        Some("repeated_resize_same_logical_column")
    } else if !f32_eq(first.width_before, repeated.width_before) {
        Some("repeated_resize_identical_initial_width")
    } else if !f32_eq(first.dx, repeated.dx) {
        Some("repeated_resize_identical_dx")
    } else if !f32_eq(first.width_after, repeated.width_after) {
        Some("repeated_resize_identical_result")
    } else {
        None
    };

    if let Some(requirement) = failed_requirement {
        derived_values.insert("first_result".to_string(), first.width_after.to_string());
        derived_values.insert(
            "repeated_result".to_string(),
            repeated.width_after.to_string(),
        );
        resize_oracle_failed(
            ORACLE_346_R12_ID,
            ORACLE_346_R12_NAME,
            InteractionFailureLayer::WidgetState,
            Some(format!("column:{:?}", first.column_id)),
            requirement,
            Some(repeated.width_after.to_string()),
            Some(repeated.authority.to_string()),
            derived_values,
        )
    } else {
        resize_oracle_passed(ORACLE_346_R12_ID, ORACLE_346_R12_NAME, derived_values)
    }
}

pub(crate) fn list_view_resize_smoke_oracle_evaluations() -> Vec<InteractionOracleEvaluation> {
    let mut routed_harness = RoutedResizeHarness::new(0.0);
    let first = routed_harness.route_index_resize(0, 25.0);
    routed_harness.layout();
    let subsequent = routed_harness.route_index_resize(1, 25.0);

    let mut repeated_first_harness = RoutedResizeHarness::new(0.0);
    let mut repeated_second_harness = RoutedResizeHarness::new(0.0);
    let repeated_first = repeated_first_harness.route_index_resize(0, 33.0);
    let repeated_second = repeated_second_harness.route_index_resize(0, 33.0);

    let (scrolled_snapshot, _) =
        laid_out_resize_probe_snapshot(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);
    let pinned_separator = &scrolled_snapshot.separators[0];
    let scrollable_separator = &scrolled_snapshot.separators[1];

    vec![
        evaluate_resize_direction_monotonicity(first.clone()),
        evaluate_no_opposite_direction_first_drag(first, subsequent),
        evaluate_repeated_resize_consistency(repeated_first, repeated_second),
        evaluate_scroll_aware_separator_hit_test(ListViewResizeSeparatorHitTestProbe {
            separator_index: 0,
            left_column_id: pinned_separator.left_column_id,
            scroll_domain: scrolled_snapshot.columns[0].scroll_domain,
            viewport_local_x: pinned_separator.viewport_x,
            horizontal_scroll_offset: scrolled_snapshot.horizontal_scroll,
            observed_test_x: pinned_separator.content_x,
            authority: "list_view_resize_smoke_probe",
        }),
        evaluate_scroll_aware_separator_hit_test(ListViewResizeSeparatorHitTestProbe {
            separator_index: 1,
            left_column_id: scrollable_separator.left_column_id,
            scroll_domain: scrolled_snapshot.columns[1].scroll_domain,
            viewport_local_x: scrollable_separator.viewport_x,
            horizontal_scroll_offset: scrolled_snapshot.horizontal_scroll,
            observed_test_x: scrollable_separator.content_x,
            authority: "list_view_resize_smoke_probe",
        }),
    ]
}

struct RoutedResizeHarness {
    tree: WidgetTree,
    root: NodeId,
    columns: ListViewResizeFixtureColumns,
    hscroll_freeze: Rc<Cell<bool>>,
    theme: Theme,
    text_engine: FontManagerTextEngine,
    router: EventRouter,
    domain_state: InteractionDomainState,
    binding_registry: BindingRegistry,
}

impl RoutedResizeHarness {
    fn new(initial_horizontal_scroll: f32) -> Self {
        let _guard = LayoutAuthorityGuard::enter();
        let fixture = ListViewResizeFixture::new();
        let columns = fixture.columns;
        let hscroll_link = fixture.horizontal_scroll_link.clone();
        let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
        hscroll_link.set(initial_horizontal_scroll);
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut text_engine = FontManagerTextEngine::new();

        let root = tree.create_root(Box::new(fixture.list_view), None, 0);
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze.clone());
        let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
        let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
        tree.node_mut(scroll_area_id)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body_id);
        tree.add_child(root, Box::new(fixture.footer), None, 0);

        test_begin_frame_layout_tracking();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(
                LIST_VIEW_RESIZE_WINDOW_WIDTH,
                LIST_VIEW_RESIZE_WINDOW_HEIGHT,
            )),
            &theme,
            &mut text_engine,
            None,
        );

        Self {
            tree,
            root,
            columns,
            hscroll_freeze,
            theme,
            text_engine,
            router: EventRouter::new(),
            domain_state: InteractionDomainState::new(),
            binding_registry: BindingRegistry::new(),
        }
    }

    fn layout(&mut self) {
        let _guard = LayoutAuthorityGuard::enter();
        test_begin_frame_layout_tracking();
        layout_tree(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(
                LIST_VIEW_RESIZE_WINDOW_WIDTH,
                LIST_VIEW_RESIZE_WINDOW_HEIGHT,
            )),
            &self.theme,
            &mut self.text_engine,
            None,
        );
    }

    fn route_index_resize(
        &mut self,
        transaction_index: usize,
        dx: f32,
    ) -> ListViewResizeTransactionProbe {
        let width_before =
            list_view_preferred_column_width(&self.tree, self.root, self.columns.index);
        let separator_x = list_view_column_width(&self.tree, self.root, self.columns.index) - 1.0;
        let y = 12.0;
        let mut driver = InteractionDriver::new();
        let fused = driver.execute_all([
            InteractionCommand::pointer_down(separator_x, y),
            InteractionCommand::pointer_move(separator_x + dx, y),
            InteractionCommand::pointer_up(separator_x + dx, y),
        ]);

        for event in fused {
            self.router.route_input_event(
                &mut self.tree,
                event,
                Some(self.root),
                &mut self.text_engine,
                None,
                &mut self.domain_state,
                &mut self.binding_registry,
            );
        }

        assert!(
            !self
                .domain_state
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        assert!(!self.hscroll_freeze.get());

        ListViewResizeTransactionProbe {
            transaction_index,
            column_id: self.columns.index,
            column_label: "Index",
            dx,
            width_before,
            width_after: list_view_preferred_column_width(
                &self.tree,
                self.root,
                self.columns.index,
            ),
            clamp_reached: false,
            clamp_boundary: None,
            authority: "event_router",
        }
    }
}

fn laid_out_resize_probe_snapshot(
    initial_horizontal_scroll: f32,
) -> (ListViewResizeProbeSnapshot, ListViewResizeFixtureColumns) {
    let _guard = LayoutAuthorityGuard::enter();
    let fixture = ListViewResizeFixture::new();
    let columns = fixture.columns;
    let hscroll_link = fixture.horizontal_scroll_link.clone();
    let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
    hscroll_link.set(initial_horizontal_scroll);
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    let root = tree.create_root(Box::new(fixture.list_view), None, 0);
    let scroll_area = ScrollArea::new_unwired()
        .enable_horizontal_scrollbar(true)
        .with_horizontal_scroll_link(hscroll_link.clone())
        .with_horizontal_freeze(hscroll_freeze);
    let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
    let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
    tree.node_mut(scroll_area_id)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body_id);
    tree.add_child(root, Box::new(fixture.footer), None, 0);

    test_begin_frame_layout_tracking();
    layout_tree(
        &mut tree,
        root,
        Constraints::tight(Size::new(
            LIST_VIEW_RESIZE_WINDOW_WIDTH,
            LIST_VIEW_RESIZE_WINDOW_HEIGHT,
        )),
        &theme,
        &mut text_engine,
        None,
    );

    let list_view = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
        .unwrap();
    (list_view_resize_probe_snapshot(list_view, columns), columns)
}

fn list_view_column_width(tree: &WidgetTree, root: NodeId, column_id: ColumnId) -> f32 {
    tree.node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
        .unwrap()
        .test_column_width(column_id)
        .unwrap()
}

fn list_view_preferred_column_width(tree: &WidgetTree, root: NodeId, column_id: ColumnId) -> f32 {
    tree.node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
        .unwrap()
        .test_column_preferred_width(column_id)
        .unwrap()
}

fn resize_transaction_derived_values(
    probe: &ListViewResizeTransactionProbe,
) -> BTreeMap<String, String> {
    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "transaction_index".to_string(),
        probe.transaction_index.to_string(),
    );
    derived_values.insert("column_id".to_string(), format!("{:?}", probe.column_id));
    derived_values.insert("column_label".to_string(), probe.column_label.to_string());
    derived_values.insert("dx".to_string(), probe.dx.to_string());
    derived_values.insert("width_before".to_string(), probe.width_before.to_string());
    derived_values.insert("width_after".to_string(), probe.width_after.to_string());
    derived_values.insert(
        "width_delta".to_string(),
        (probe.width_after - probe.width_before).to_string(),
    );
    derived_values.insert("clamp_reached".to_string(), probe.clamp_reached.to_string());
    derived_values.insert(
        "clamp_boundary".to_string(),
        probe
            .clamp_boundary
            .map(|boundary| boundary.to_string())
            .unwrap_or_default(),
    );
    derived_values.insert("authority".to_string(), probe.authority.to_string());
    derived_values
}

fn first_subsequent_derived_values(
    first: &ListViewResizeTransactionProbe,
    subsequent: &ListViewResizeTransactionProbe,
) -> BTreeMap<String, String> {
    let mut derived_values = BTreeMap::new();
    derived_values.insert(
        "first_column_id".to_string(),
        format!("{:?}", first.column_id),
    );
    derived_values.insert(
        "first_column_label".to_string(),
        first.column_label.to_string(),
    );
    derived_values.insert(
        "subsequent_column_id".to_string(),
        format!("{:?}", subsequent.column_id),
    );
    derived_values.insert(
        "subsequent_column_label".to_string(),
        subsequent.column_label.to_string(),
    );
    derived_values.insert("first_dx".to_string(), first.dx.to_string());
    derived_values.insert("subsequent_dx".to_string(), subsequent.dx.to_string());
    derived_values.insert(
        "first_width_before".to_string(),
        first.width_before.to_string(),
    );
    derived_values.insert(
        "subsequent_width_before".to_string(),
        subsequent.width_before.to_string(),
    );
    derived_values.insert(
        "first_width_after".to_string(),
        first.width_after.to_string(),
    );
    derived_values.insert(
        "subsequent_width_after".to_string(),
        subsequent.width_after.to_string(),
    );
    derived_values.insert("first_authority".to_string(), first.authority.to_string());
    derived_values.insert(
        "subsequent_authority".to_string(),
        subsequent.authority.to_string(),
    );
    derived_values
}

fn resize_direction_class(probe: &ListViewResizeTransactionProbe) -> &'static str {
    let width_delta = probe.width_after - probe.width_before;
    if f32_eq(probe.dx, 0.0) {
        if f32_eq(width_delta, 0.0) {
            "same"
        } else {
            "opposite"
        }
    } else if (probe.dx > 0.0 && width_delta > 0.0) || (probe.dx < 0.0 && width_delta < 0.0) {
        "matching"
    } else if f32_eq(width_delta, 0.0) && probe.clamp_reached {
        "clamped"
    } else {
        "opposite"
    }
}

fn resize_oracle_passed(
    oracle_id: &'static str,
    oracle_name: &'static str,
    derived_values: BTreeMap<String, String>,
) -> InteractionOracleEvaluation {
    InteractionOracleEvaluation {
        oracle_id: oracle_id.to_string(),
        oracle_name: oracle_name.to_string(),
        status: InteractionOracleStatus::Passed,
        failure_layer: None,
        failed_subject: None,
        failed_requirement: None,
        failed_sink: None,
        failed_field_path: None,
        failed_authority: None,
        failed_value: None,
        derived_values,
    }
}

fn resize_oracle_failed(
    oracle_id: &'static str,
    oracle_name: &'static str,
    failure_layer: InteractionFailureLayer,
    failed_subject: Option<String>,
    failed_requirement: &'static str,
    failed_value: Option<String>,
    failed_authority: Option<String>,
    derived_values: BTreeMap<String, String>,
) -> InteractionOracleEvaluation {
    InteractionOracleEvaluation {
        oracle_id: oracle_id.to_string(),
        oracle_name: oracle_name.to_string(),
        status: InteractionOracleStatus::Failed,
        failure_layer: Some(failure_layer),
        failed_subject,
        failed_requirement: Some(failed_requirement.to_string()),
        failed_sink: None,
        failed_field_path: None,
        failed_authority,
        failed_value,
        derived_values,
    }
}

fn f32_eq(left: f32, right: f32) -> bool {
    (left - right).abs() <= f32::EPSILON
}

impl Default for ListViewResizeFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
    use crate::core::event::{
        Event, EventPhase, FusedInputEvent, InteractionDomainState, KeyboardAuthorityMode,
        Modifiers, PointerButton, PointerEvent, PointerEventKind, PointerId, PointerType,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::interaction_fuzz::{InteractionCommand, InteractionDriver};
    use crate::core::layout::Constraints;
    use crate::core::math::{Point, Size};
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::theme::Theme;
    use crate::core::widget::EventContext;
    use crate::core::widget_tree::{NodeId, WidgetTree};
    use crate::model::tabular::column::MIN_COL_WIDTH;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::scroll_area::ScrollArea;

    #[test]
    fn resize_fixture_builds_authoritative_columns() {
        let mut fixture = ListViewResizeFixture::with_row_count(3);

        assert_eq!(fixture.list_view.row_count(), 3);
        assert_eq!(fixture.window_width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(fixture.window_height, LIST_VIEW_RESIZE_WINDOW_HEIGHT);
        assert_ne!(fixture.columns.index, fixture.columns.name);
        assert_ne!(fixture.columns.name, fixture.columns.description);

        let (index_cell, name_cell, columns) = {
            let adapter = fixture.list_view.adapter_mut();
            let source = adapter.source();
            let row = source
                .row_ids()
                .first()
                .copied()
                .expect("fixture must create at least one row");

            (
                source.read_cell(row, fixture.columns.index),
                source.read_cell(row, fixture.columns.name),
                source.columns(),
            )
        };

        assert_eq!(index_cell, ValueState::Ready(CellValue::Int(0)));
        assert_eq!(
            name_cell,
            ValueState::Ready(CellValue::String("Item 0".to_string()))
        );
        assert_eq!(columns, Some(fixture.columns));
    }

    #[test]
    fn resize_fixture_wires_horizontal_scroll_link() {
        let fixture = ListViewResizeFixture::new();

        assert_eq!(fixture.horizontal_scroll_link.get(), 0.0);
        assert!(!fixture.horizontal_scroll_freeze.get());

        fixture.horizontal_scroll_link.set(128.0);
        assert_eq!(fixture.horizontal_scroll_link.get(), 128.0);
    }

    #[test]
    fn resize_fixture_probe_exposes_widths_and_separator_positions() {
        let (snapshot, columns) = laid_out_resize_probe_snapshot(0.0);

        assert_eq!(snapshot.source_layer, LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER);
        assert_eq!(snapshot.source_owner, LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER);
        assert_eq!(snapshot.layout_epoch, 1);
        assert_eq!(snapshot.frame_number, 0);
        assert_eq!(
            snapshot.settle_boundary,
            InteractionSettleBoundary::Reconcile
        );
        assert_eq!(
            snapshot.snapshot_reason,
            LIST_VIEW_RESIZE_PROBE_SNAPSHOT_REASON
        );
        assert!(snapshot.widget_id > 0);
        assert_eq!(snapshot.root_rect.x, 0.0);
        assert_eq!(snapshot.root_rect.y, 0.0);
        assert_eq!(snapshot.root_rect.width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(snapshot.root_rect.height, LIST_VIEW_RESIZE_WINDOW_HEIGHT);
        assert_eq!(snapshot.header_rect.x, 0.0);
        assert_eq!(snapshot.header_rect.y, 0.0);
        assert_eq!(snapshot.header_rect.width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(snapshot.header_rect.height, 24.0);
        assert_eq!(snapshot.body_rect.x, 0.0);
        assert_eq!(snapshot.body_rect.y, 24.0);
        assert_eq!(snapshot.body_rect.width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(
            snapshot.body_rect.height,
            LIST_VIEW_RESIZE_WINDOW_HEIGHT - 24.0 - LIST_VIEW_RESIZE_FOOTER_HEIGHT
        );
        assert_eq!(snapshot.footer_rect.x, 0.0);
        assert_eq!(
            snapshot.footer_rect.y,
            LIST_VIEW_RESIZE_WINDOW_HEIGHT - LIST_VIEW_RESIZE_FOOTER_HEIGHT
        );
        assert_eq!(snapshot.footer_rect.width, LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(snapshot.footer_rect.height, LIST_VIEW_RESIZE_FOOTER_HEIGHT);
        assert_eq!(snapshot.horizontal_scroll, 0.0);
        assert_eq!(snapshot.vertical_scroll_offset, 0.0);
        assert_eq!(snapshot.resize_state, "idle");
        assert_eq!(snapshot.active_resize_separator, None);
        assert_eq!(snapshot.hovered_separator, None);
        assert!(!snapshot.pointer_in_expansion_region);
        assert!(snapshot.selected_rows.is_empty());
        assert_eq!(snapshot.focused_row, None);
        assert_eq!(snapshot.columns.len(), 5);
        assert_eq!(snapshot.separators.len(), 4);
        assert_eq!(
            snapshot.columns[0].resolved_width,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH
        );
        assert_eq!(snapshot.columns[0].display_index, 0);
        assert!(snapshot.columns[0].visible);
        assert!(snapshot.columns[0].resizable);
        assert!(snapshot.columns[0].reorderable);
        assert_eq!(snapshot.columns[0].resolved_x_offset, 0.0);
        assert_eq!(snapshot.columns[0].min_width, 0.0);
        assert_eq!(snapshot.columns[0].max_width, None);
        assert_eq!(
            snapshot.columns[1].resolved_width,
            LIST_VIEW_RESIZE_COL_NAME_WIDTH
        );
        assert_eq!(
            snapshot.columns[0].preferred_width,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH
        );
        assert_eq!(
            snapshot.columns[1].preferred_width,
            LIST_VIEW_RESIZE_COL_NAME_WIDTH
        );
        assert_eq!(
            snapshot.columns[0].separator_x_content,
            Some(LIST_VIEW_RESIZE_COL_INDEX_WIDTH - 1.0)
        );
        assert_eq!(
            snapshot.columns[0].separator_x_viewport,
            Some(LIST_VIEW_RESIZE_COL_INDEX_WIDTH - 1.0)
        );
        assert_eq!(snapshot.separators[0].left_column_id, columns.index);
        assert_eq!(snapshot.separators[0].right_column_id, columns.name);
        assert_eq!(
            snapshot.separators[0].content_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH - 1.0
        );
        assert_eq!(
            snapshot.separators[0].viewport_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH - 1.0
        );
        assert!(!snapshot.separators[0].scroll_adjusted);
        assert_eq!(
            snapshot.separators[1].content_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH + LIST_VIEW_RESIZE_COL_NAME_WIDTH - 1.0
        );
        assert_eq!(
            snapshot.separators[1].viewport_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH + LIST_VIEW_RESIZE_COL_NAME_WIDTH - 1.0
        );
        assert!(snapshot.separators[1].scroll_adjusted);
        assert!(!snapshot.realized_rows.is_empty());
        assert_eq!(snapshot.realized_rows[0].row_id, 1);
        assert_eq!(snapshot.realized_rows[0].data_index, 0);
        assert_eq!(snapshot.realized_rows[0].visual_index, 0);
        assert_eq!(snapshot.realized_rows[0].rect.x, 0.0);
        assert_eq!(snapshot.realized_rows[0].rect.y, 0.0);
        assert_eq!(
            snapshot.realized_rows[0].rect.width,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH
                + LIST_VIEW_RESIZE_COL_NAME_WIDTH
                + LIST_VIEW_RESIZE_COL_DESCRIPTION_WIDTH
                + LIST_VIEW_RESIZE_COL_STATUS_WIDTH
                + LIST_VIEW_RESIZE_COL_CATEGORY_WIDTH
        );
        assert_eq!(snapshot.realized_rows[0].cells.len(), 5);
        assert_eq!(snapshot.realized_rows[0].cells[0].column_id, columns.index);
        assert_eq!(
            snapshot.realized_rows[0].cells[0].content_state,
            "ready:int:0"
        );
        assert_eq!(snapshot.realized_rows[0].cells[0].rect.x, 0.0);
        assert_eq!(
            snapshot.realized_rows[0].cells[0].rect.width,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH
        );
        assert_eq!(snapshot.realized_rows[0].cells[1].column_id, columns.name);
        assert_eq!(
            snapshot.realized_rows[0].cells[1].content_state,
            "ready:string:Item 0"
        );

        let json = serde_json::to_value(&snapshot).expect("probe snapshot must serialize");
        let artifact = InteractionProbeSnapshotArtifact::from_serializable(
            LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER,
            LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER,
            snapshot.layout_epoch,
            snapshot.frame_number,
            snapshot.settle_boundary,
            LIST_VIEW_RESIZE_PROBE_SNAPSHOT_REASON,
            &snapshot,
        )
        .expect("probe snapshot artifact must serialize");

        assert_eq!(artifact.source_layer, LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER);
        assert_eq!(artifact.source_owner, LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER);
        assert_eq!(artifact.payload["resize_state"], "idle");
        assert_eq!(
            artifact.payload["realized_rows"][0]["cells"][0]["content_state"],
            "ready:int:0"
        );
        assert_eq!(json["source_layer"], LIST_VIEW_RESIZE_PROBE_SOURCE_LAYER);
        assert_eq!(json["source_owner"], LIST_VIEW_RESIZE_PROBE_SOURCE_OWNER);
        assert_eq!(json["layout_epoch"], 1);
        assert_eq!(json["frame_number"], 0);
        assert_eq!(json["settle_boundary"], "reconcile");
        assert_eq!(
            json["snapshot_reason"],
            LIST_VIEW_RESIZE_PROBE_SNAPSHOT_REASON
        );
        assert_eq!(json["root_rect"]["width"], LIST_VIEW_RESIZE_WINDOW_WIDTH);
        assert_eq!(json["body_rect"]["y"], 24.0);
        assert_eq!(
            json["footer_rect"]["height"],
            LIST_VIEW_RESIZE_FOOTER_HEIGHT
        );
        assert_eq!(json["vertical_scroll_offset"], 0.0);
        assert_eq!(json["resize_state"], "idle");
        assert_eq!(json["selected_rows"], serde_json::json!([]));
        assert_eq!(json["focused_row"], serde_json::Value::Null);
        assert_eq!(json["columns"][0]["display_index"], 0);
        assert_eq!(json["columns"][0]["max_width"], serde_json::Value::Null);
        assert_eq!(json["realized_rows"][0]["data_index"], 0);
        assert_eq!(
            json["realized_rows"][0]["cells"][0]["content_state"],
            "ready:int:0"
        );
    }

    #[test]
    fn resize_fixture_probe_adjusts_scrollable_separator_viewport_positions() {
        let (snapshot, _) =
            laid_out_resize_probe_snapshot(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);

        assert_eq!(
            snapshot.horizontal_scroll,
            LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
        );
        assert_eq!(
            snapshot.separators[0].viewport_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH - 1.0
        );
        assert_eq!(
            snapshot.separators[1].viewport_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH + LIST_VIEW_RESIZE_COL_NAME_WIDTH
                - LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
                - 1.0
        );
        assert_eq!(
            snapshot.separators[2].viewport_x,
            LIST_VIEW_RESIZE_COL_INDEX_WIDTH
                + LIST_VIEW_RESIZE_COL_NAME_WIDTH
                + LIST_VIEW_RESIZE_COL_DESCRIPTION_WIDTH
                - LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL
                - 1.0
        );
    }

    #[test]
    fn resize_direction_monotonicity_oracle_passes_real_routed_resize() {
        let mut harness = RoutedResizeHarness::new(0.0);
        let transaction = harness.route_index_resize(0, 25.0);

        let evaluation = evaluate_resize_direction_monotonicity(transaction);

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R2_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_R2_NAME);
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(
            evaluation.derived_values.get("width_before"),
            Some(&LIST_VIEW_RESIZE_COL_INDEX_WIDTH.to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("width_after"),
            Some(&(LIST_VIEW_RESIZE_COL_INDEX_WIDTH + 25.0).to_string())
        );
    }

    #[test]
    fn resize_direction_monotonicity_oracle_reports_opposite_direction() {
        let columns = ListViewResizeFixture::new().columns;
        let evaluation = evaluate_resize_direction_monotonicity(ListViewResizeTransactionProbe {
            transaction_index: 0,
            column_id: columns.index,
            column_label: "Index",
            dx: 25.0,
            width_before: 80.0,
            width_after: 55.0,
            clamp_reached: false,
            clamp_boundary: None,
            authority: "synthetic_probe",
        });

        assert!(!evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R2_ID);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::WidgetState)
        );
        assert_eq!(
            evaluation.failed_requirement,
            Some("positive_dx_increases_width".to_string())
        );
        assert_eq!(evaluation.failed_value, Some("55".to_string()));
    }

    #[test]
    fn scroll_aware_separator_hit_test_oracle_passes_pinned_and_scrollable_domains() {
        let (snapshot, _) =
            laid_out_resize_probe_snapshot(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);
        let pinned_separator = &snapshot.separators[0];
        let scrollable_separator = &snapshot.separators[1];

        let pinned_evaluation =
            evaluate_scroll_aware_separator_hit_test(ListViewResizeSeparatorHitTestProbe {
                separator_index: 0,
                left_column_id: pinned_separator.left_column_id,
                scroll_domain: snapshot.columns[0].scroll_domain,
                viewport_local_x: pinned_separator.viewport_x,
                horizontal_scroll_offset: snapshot.horizontal_scroll,
                observed_test_x: pinned_separator.content_x,
                authority: "list_view_hit_test",
            });
        let scrollable_evaluation =
            evaluate_scroll_aware_separator_hit_test(ListViewResizeSeparatorHitTestProbe {
                separator_index: 1,
                left_column_id: scrollable_separator.left_column_id,
                scroll_domain: snapshot.columns[1].scroll_domain,
                viewport_local_x: scrollable_separator.viewport_x,
                horizontal_scroll_offset: snapshot.horizontal_scroll,
                observed_test_x: scrollable_separator.content_x,
                authority: "list_view_hit_test",
            });

        assert!(pinned_evaluation.passed());
        assert!(scrollable_evaluation.passed());
        assert_eq!(pinned_evaluation.oracle_id, ORACLE_346_R5_ID);
        assert_eq!(
            scrollable_evaluation.derived_values.get("expected_test_x"),
            Some(&scrollable_separator.content_x.to_string())
        );
    }

    #[test]
    fn scroll_aware_separator_hit_test_oracle_reports_viewport_x_used_for_scrollable_column() {
        let (snapshot, _) =
            laid_out_resize_probe_snapshot(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);
        let scrollable_separator = &snapshot.separators[1];

        let evaluation =
            evaluate_scroll_aware_separator_hit_test(ListViewResizeSeparatorHitTestProbe {
                separator_index: 1,
                left_column_id: scrollable_separator.left_column_id,
                scroll_domain: snapshot.columns[1].scroll_domain,
                viewport_local_x: scrollable_separator.viewport_x,
                horizontal_scroll_offset: snapshot.horizontal_scroll,
                observed_test_x: scrollable_separator.viewport_x,
                authority: "synthetic_hit_test",
            });

        assert!(!evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R5_ID);
        assert_eq!(
            evaluation.failure_layer,
            Some(InteractionFailureLayer::EventRouting)
        );
        assert_eq!(
            evaluation.failed_requirement,
            Some("scroll_aware_separator_hit_test".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("expected_test_x"),
            Some(&scrollable_separator.content_x.to_string())
        );
    }

    #[test]
    fn first_subsequent_drag_consistency_oracle_passes_real_routed_resizes() {
        let mut harness = RoutedResizeHarness::new(0.0);
        let first = harness.route_index_resize(0, 25.0);
        harness.layout();
        let subsequent = harness.route_index_resize(1, 25.0);

        let evaluation = evaluate_no_opposite_direction_first_drag(first, subsequent);

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R4_ID);
        assert_eq!(
            evaluation.derived_values.get("first_direction"),
            Some(&"matching".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("subsequent_direction"),
            Some(&"matching".to_string())
        );
    }

    #[test]
    fn routed_resize_moves_subsequent_columns_without_resizing_neighbor() {
        let mut harness = RoutedResizeHarness::new(0.0);
        let columns = harness.columns;
        let dx = 25.0;
        let neighbor_x_before = harness.column_resolved_x(columns.name);
        let neighbor_width_before =
            list_view_preferred_column_width(&harness.tree, harness.root, columns.name);

        let transaction = harness.route_separator_resize(0, 0, columns.index, "Index", dx);
        harness.layout();

        assert_eq!(transaction.width_after, transaction.width_before + dx);
        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.name),
            neighbor_width_before
        );
        assert_eq!(
            harness.column_resolved_x(columns.name),
            neighbor_x_before + dx
        );
    }

    #[test]
    fn routed_resize_from_late_separator_moves_subsequent_columns_without_resizing_neighbor() {
        let mut harness = RoutedResizeHarness::new(LIST_VIEW_RESIZE_LV2_INITIAL_HORIZONTAL_SCROLL);
        let columns = harness.columns;
        let dx = 25.0;
        let neighbor_x_before = harness.column_resolved_x(columns.category);
        let neighbor_width_before =
            list_view_preferred_column_width(&harness.tree, harness.root, columns.category);

        let transaction = harness.route_separator_resize(0, 3, columns.status, "Status", dx);
        harness.layout();

        assert_eq!(transaction.width_after, transaction.width_before + dx);
        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.category),
            neighbor_width_before
        );
        assert_eq!(
            harness.column_resolved_x(columns.category),
            neighbor_x_before + dx
        );
    }

    #[test]
    fn routed_resize_keeps_elastic_sibling_frozen_while_subsequent_columns_translate() {
        let mut harness = RoutedResizeHarness::with_flex_name(0.0);
        let columns = harness.columns;
        let dx = 25.0;
        let elastic_width_before =
            list_view_column_width(&harness.tree, harness.root, columns.name);
        let elastic_preferred_before =
            list_view_preferred_column_width(&harness.tree, harness.root, columns.name);
        let neighbor_x_before = harness.column_resolved_x(columns.category);
        let neighbor_width_before =
            list_view_preferred_column_width(&harness.tree, harness.root, columns.category);

        let transaction = harness.route_separator_resize(0, 3, columns.status, "Status", dx);
        harness.layout();

        assert_eq!(transaction.width_after, transaction.width_before + dx);
        assert_eq!(
            list_view_column_width(&harness.tree, harness.root, columns.name),
            elastic_width_before
        );
        assert_ne!(elastic_preferred_before, elastic_width_before);
        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.name),
            elastic_width_before
        );
        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.category),
            neighbor_width_before
        );
        assert_eq!(
            harness.column_resolved_x(columns.category),
            neighbor_x_before + dx
        );
    }

    #[test]
    fn routed_resize_flex_target_moves_subsequent_columns_in_both_directions() {
        for dx in [25.0, -25.0] {
            let mut harness = RoutedResizeHarness::with_flex_name(0.0);
            let columns = harness.columns;
            let target_width_before =
                list_view_column_width(&harness.tree, harness.root, columns.name);
            let neighbor_x_before = harness.column_resolved_x(columns.description);
            let neighbor_width_before =
                list_view_preferred_column_width(&harness.tree, harness.root, columns.description);

            let transaction = harness.route_separator_resize(0, 1, columns.name, "Name", dx);
            harness.layout();

            assert_eq!(transaction.width_after, target_width_before + dx);
            assert_eq!(
                list_view_preferred_column_width(&harness.tree, harness.root, columns.description),
                neighbor_width_before
            );
            assert_eq!(
                harness.column_resolved_x(columns.description),
                neighbor_x_before + dx
            );
        }
    }

    #[test]
    fn routed_resize_flex_target_reverse_drag_moves_subsequent_columns_back_after_layout() {
        let mut harness = RoutedResizeHarness::with_flex_name(0.0);
        let columns = harness.columns;
        let target_width_before = list_view_column_width(&harness.tree, harness.root, columns.name);
        let neighbor_x_before = harness.column_resolved_x(columns.description);
        let first_dx = 40.0;
        let final_dx = -30.0;

        harness.route_separator_drag_deltas_with_layouts(1, &[first_dx, final_dx]);

        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.name),
            target_width_before + final_dx
        );
        assert_eq!(
            harness.column_resolved_x(columns.description),
            neighbor_x_before + final_dx
        );
    }

    #[test]
    fn routed_resize_after_elastic_sibling_reverse_drag_moves_subsequent_columns_back_after_layout()
    {
        let mut harness = RoutedResizeHarness::with_flex_name(0.0);
        let columns = harness.columns;
        let elastic_width_before =
            list_view_column_width(&harness.tree, harness.root, columns.name);
        let target_width_before =
            list_view_column_width(&harness.tree, harness.root, columns.status);
        let neighbor_x_before = harness.column_resolved_x(columns.category);
        let first_dx = 40.0;
        let final_dx = -30.0;

        harness.route_separator_drag_deltas_with_layouts(3, &[first_dx, final_dx]);

        assert_eq!(
            list_view_column_width(&harness.tree, harness.root, columns.name),
            elastic_width_before
        );
        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.status),
            target_width_before + final_dx
        );
        assert_eq!(
            harness.column_resolved_x(columns.category),
            neighbor_x_before + final_dx
        );
    }

    #[test]
    fn routed_resize_reverse_after_min_clamp_moves_subsequent_columns_immediately() {
        let mut harness = RoutedResizeHarness::with_flex_name(0.0);
        let columns = harness.columns;
        let target_width_before =
            list_view_column_width(&harness.tree, harness.root, columns.status);
        let neighbor_x_before = harness.column_resolved_x(columns.category);
        let first_dx = -200.0;
        let final_dx = -190.0;
        let expected_width = MIN_COL_WIDTH + (final_dx - first_dx);

        harness.route_separator_drag_deltas_with_layouts(3, &[first_dx, final_dx]);

        assert_eq!(
            list_view_preferred_column_width(&harness.tree, harness.root, columns.status),
            expected_width
        );
        assert_eq!(
            harness.column_resolved_x(columns.category),
            neighbor_x_before + (expected_width - target_width_before)
        );
    }

    #[test]
    fn first_subsequent_drag_consistency_oracle_reports_opposite_first_drag() {
        let columns = ListViewResizeFixture::new().columns;
        let first = ListViewResizeTransactionProbe {
            transaction_index: 0,
            column_id: columns.index,
            column_label: "Index",
            dx: 25.0,
            width_before: 80.0,
            width_after: 55.0,
            clamp_reached: false,
            clamp_boundary: None,
            authority: "synthetic_first_drag",
        };
        let subsequent = ListViewResizeTransactionProbe {
            transaction_index: 1,
            column_id: columns.index,
            column_label: "Index",
            dx: 25.0,
            width_before: 80.0,
            width_after: 105.0,
            clamp_reached: false,
            clamp_boundary: None,
            authority: "synthetic_subsequent_drag",
        };

        let evaluation = evaluate_no_opposite_direction_first_drag(first, subsequent);

        assert!(!evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R4_ID);
        assert_eq!(
            evaluation.failed_requirement,
            Some("first_drag_direction_matches_subsequent_drag".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("first_direction"),
            Some(&"opposite".to_string())
        );
    }

    #[test]
    fn repeated_resize_consistency_oracle_passes_identical_routed_transactions() {
        let mut first_harness = RoutedResizeHarness::new(0.0);
        let mut repeated_harness = RoutedResizeHarness::new(0.0);
        let first = first_harness.route_index_resize(0, 33.0);
        let repeated = repeated_harness.route_index_resize(0, 33.0);

        let evaluation = evaluate_repeated_resize_consistency(first, repeated);

        assert!(evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R12_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_R12_NAME);
    }

    #[test]
    fn repeated_resize_consistency_oracle_reports_divergent_result() {
        let columns = ListViewResizeFixture::new().columns;
        let first = ListViewResizeTransactionProbe {
            transaction_index: 0,
            column_id: columns.index,
            column_label: "Index",
            dx: 25.0,
            width_before: 80.0,
            width_after: 105.0,
            clamp_reached: false,
            clamp_boundary: None,
            authority: "synthetic_first_drag",
        };
        let repeated = ListViewResizeTransactionProbe {
            transaction_index: 0,
            column_id: columns.index,
            column_label: "Index",
            dx: 25.0,
            width_before: 80.0,
            width_after: 104.0,
            clamp_reached: false,
            clamp_boundary: None,
            authority: "synthetic_repeated_drag",
        };

        let evaluation = evaluate_repeated_resize_consistency(first, repeated);

        assert!(!evaluation.passed());
        assert_eq!(evaluation.oracle_id, ORACLE_346_R12_ID);
        assert_eq!(
            evaluation.failed_requirement,
            Some("repeated_resize_identical_result".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("first_result"),
            Some(&"105".to_string())
        );
        assert_eq!(
            evaluation.derived_values.get("repeated_result"),
            Some(&"104".to_string())
        );
    }

    #[test]
    fn interaction_driver_resizes_first_list_view_column() {
        let _guard = LayoutAuthorityGuard::enter();
        let fixture = ListViewResizeFixture::new();
        let columns = fixture.columns;
        let hscroll_link = fixture.horizontal_scroll_link.clone();
        let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut text_engine = FontManagerTextEngine::new();

        let root = tree.create_root(Box::new(fixture.list_view), None, 0);
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze.clone());
        let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
        let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
        tree.node_mut(scroll_area_id)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body_id);
        tree.add_child(root, Box::new(fixture.footer), None, 0);

        test_begin_frame_layout_tracking();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(
                LIST_VIEW_RESIZE_WINDOW_WIDTH,
                LIST_VIEW_RESIZE_WINDOW_HEIGHT,
            )),
            &theme,
            &mut text_engine,
            None,
        );

        let initial_width = list_view_column_width(&tree, root, columns.index);
        assert_eq!(initial_width, LIST_VIEW_RESIZE_COL_INDEX_WIDTH);

        let separator_x = initial_width - 1.0;
        let y = 12.0;
        let drag_dx = 25.0;
        let mut driver = InteractionDriver::new();
        let fused = driver.execute_all([
            InteractionCommand::pointer_down(separator_x, y),
            InteractionCommand::pointer_move(separator_x + drag_dx, y),
            InteractionCommand::pointer_up(separator_x + drag_dx, y),
        ]);
        assert!(
            fused
                .iter()
                .any(|event| matches!(event, FusedInputEvent::DragUpdate { .. }))
        );

        let list_view_id = tree.node(root).unwrap().id;
        let mut has_pointer_capture = false;
        let mut handled = Vec::new();
        for event in fused {
            let Some(event) = pointer_event_from_fused(event) else {
                continue;
            };
            let result =
                dispatch_to_list_view(&mut tree, root, list_view_id, event, has_pointer_capture);
            if result.capture_requested {
                has_pointer_capture = true;
            }
            if result.release_requested {
                has_pointer_capture = false;
            }
            handled.push(result.handled);
        }

        assert_eq!(handled, vec![true, true, true]);
        assert!(!has_pointer_capture);
        assert!(!hscroll_freeze.get());

        let resized_width = list_view_preferred_column_width(&tree, root, columns.index);
        assert_eq!(resized_width, initial_width + drag_dx);
    }

    #[test]
    fn routed_interaction_driver_resizes_first_list_view_column() {
        let _guard = LayoutAuthorityGuard::enter();
        let fixture = ListViewResizeFixture::new();
        let columns = fixture.columns;
        let hscroll_link = fixture.horizontal_scroll_link.clone();
        let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut text_engine = FontManagerTextEngine::new();

        let root = tree.create_root(Box::new(fixture.list_view), None, 0);
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze.clone());
        let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
        let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
        tree.node_mut(scroll_area_id)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body_id);
        tree.add_child(root, Box::new(fixture.footer), None, 0);

        test_begin_frame_layout_tracking();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(
                LIST_VIEW_RESIZE_WINDOW_WIDTH,
                LIST_VIEW_RESIZE_WINDOW_HEIGHT,
            )),
            &theme,
            &mut text_engine,
            None,
        );

        let initial_width = list_view_column_width(&tree, root, columns.index);
        assert_eq!(initial_width, LIST_VIEW_RESIZE_COL_INDEX_WIDTH);

        let separator_x = initial_width - 1.0;
        let y = 12.0;
        let drag_dx = 25.0;
        let mut driver = InteractionDriver::new();
        let fused = driver.execute_all([
            InteractionCommand::pointer_down(separator_x, y),
            InteractionCommand::pointer_move(separator_x + drag_dx, y),
            InteractionCommand::pointer_up(separator_x + drag_dx, y),
        ]);
        assert!(
            fused
                .iter()
                .any(|event| matches!(event, FusedInputEvent::DragStart { .. }))
        );
        assert!(
            fused
                .iter()
                .any(|event| matches!(event, FusedInputEvent::DragUpdate { .. }))
        );

        let mut router = EventRouter::new();
        let mut domain_state = InteractionDomainState::new();
        let mut binding_registry = BindingRegistry::new();
        for event in fused {
            router.route_input_event(
                &mut tree,
                event,
                Some(root),
                &mut text_engine,
                None,
                &mut domain_state,
                &mut binding_registry,
            );
        }

        assert!(
            !domain_state
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        assert!(!hscroll_freeze.get());

        let resized_width = list_view_preferred_column_width(&tree, root, columns.index);
        assert_eq!(resized_width, initial_width + drag_dx);
    }

    struct DispatchResult {
        handled: bool,
        capture_requested: bool,
        release_requested: bool,
    }

    struct RoutedResizeHarness {
        tree: WidgetTree,
        root: NodeId,
        columns: ListViewResizeFixtureColumns,
        hscroll_freeze: Rc<Cell<bool>>,
        theme: Theme,
        text_engine: FontManagerTextEngine,
        router: EventRouter,
        domain_state: InteractionDomainState,
        binding_registry: BindingRegistry,
    }

    impl RoutedResizeHarness {
        fn new(initial_horizontal_scroll: f32) -> Self {
            Self::with_fixture(ListViewResizeFixture::new(), initial_horizontal_scroll)
        }

        fn with_flex_name(initial_horizontal_scroll: f32) -> Self {
            Self::with_fixture(
                ListViewResizeFixture::with_flex_name(),
                initial_horizontal_scroll,
            )
        }

        fn with_fixture(fixture: ListViewResizeFixture, initial_horizontal_scroll: f32) -> Self {
            let _guard = LayoutAuthorityGuard::enter();
            let columns = fixture.columns;
            let hscroll_link = fixture.horizontal_scroll_link.clone();
            let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
            hscroll_link.set(initial_horizontal_scroll);
            let mut tree = WidgetTree::new();
            let theme = Theme::default();
            let mut text_engine = FontManagerTextEngine::new();

            let root = tree.create_root(Box::new(fixture.list_view), None, 0);
            let scroll_area = ScrollArea::new_unwired()
                .enable_horizontal_scrollbar(true)
                .with_horizontal_scroll_link(hscroll_link)
                .with_horizontal_freeze(hscroll_freeze.clone());
            let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
            let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
            tree.node_mut(scroll_area_id)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .wire_content(body_id);
            tree.add_child(root, Box::new(fixture.footer), None, 0);

            test_begin_frame_layout_tracking();
            layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(
                    LIST_VIEW_RESIZE_WINDOW_WIDTH,
                    LIST_VIEW_RESIZE_WINDOW_HEIGHT,
                )),
                &theme,
                &mut text_engine,
                None,
            );

            Self {
                tree,
                root,
                columns,
                hscroll_freeze,
                theme,
                text_engine,
                router: EventRouter::new(),
                domain_state: InteractionDomainState::new(),
                binding_registry: BindingRegistry::new(),
            }
        }

        fn layout(&mut self) {
            let _guard = LayoutAuthorityGuard::enter();
            test_begin_frame_layout_tracking();
            layout_tree(
                &mut self.tree,
                self.root,
                Constraints::tight(Size::new(
                    LIST_VIEW_RESIZE_WINDOW_WIDTH,
                    LIST_VIEW_RESIZE_WINDOW_HEIGHT,
                )),
                &self.theme,
                &mut self.text_engine,
                None,
            );
        }

        fn route_index_resize(
            &mut self,
            transaction_index: usize,
            dx: f32,
        ) -> ListViewResizeTransactionProbe {
            let width_before =
                list_view_preferred_column_width(&self.tree, self.root, self.columns.index);
            let separator_x =
                list_view_column_width(&self.tree, self.root, self.columns.index) - 1.0;
            let y = 12.0;
            let mut driver = InteractionDriver::new();
            let fused = driver.execute_all([
                InteractionCommand::pointer_down(separator_x, y),
                InteractionCommand::pointer_move(separator_x + dx, y),
                InteractionCommand::pointer_up(separator_x + dx, y),
            ]);

            for event in fused {
                self.router.route_input_event(
                    &mut self.tree,
                    event,
                    Some(self.root),
                    &mut self.text_engine,
                    None,
                    &mut self.domain_state,
                    &mut self.binding_registry,
                );
            }

            assert!(
                !self
                    .domain_state
                    .pointer_capture_manager
                    .is_captured(PointerId::MOUSE)
            );
            assert!(!self.hscroll_freeze.get());

            ListViewResizeTransactionProbe {
                transaction_index,
                column_id: self.columns.index,
                column_label: "Index",
                dx,
                width_before,
                width_after: list_view_preferred_column_width(
                    &self.tree,
                    self.root,
                    self.columns.index,
                ),
                clamp_reached: false,
                clamp_boundary: None,
                authority: "event_router",
            }
        }

        fn route_separator_resize(
            &mut self,
            transaction_index: usize,
            separator_index: usize,
            observed_column_id: ColumnId,
            observed_column_label: &'static str,
            dx: f32,
        ) -> ListViewResizeTransactionProbe {
            let width_before =
                list_view_preferred_column_width(&self.tree, self.root, observed_column_id);
            let separator_x = self.separator_viewport_x(separator_index);
            let y = 12.0;
            let mut driver = InteractionDriver::new();
            let fused = driver.execute_all([
                InteractionCommand::pointer_down(separator_x, y),
                InteractionCommand::pointer_move(separator_x + dx, y),
                InteractionCommand::pointer_up(separator_x + dx, y),
            ]);

            for event in fused {
                self.router.route_input_event(
                    &mut self.tree,
                    event,
                    Some(self.root),
                    &mut self.text_engine,
                    None,
                    &mut self.domain_state,
                    &mut self.binding_registry,
                );
            }

            assert!(
                !self
                    .domain_state
                    .pointer_capture_manager
                    .is_captured(PointerId::MOUSE)
            );
            assert!(!self.hscroll_freeze.get());

            ListViewResizeTransactionProbe {
                transaction_index,
                column_id: observed_column_id,
                column_label: observed_column_label,
                dx,
                width_before,
                width_after: list_view_preferred_column_width(
                    &self.tree,
                    self.root,
                    observed_column_id,
                ),
                clamp_reached: false,
                clamp_boundary: None,
                authority: "event_router",
            }
        }

        fn route_separator_drag_deltas_with_layouts(
            &mut self,
            separator_index: usize,
            deltas: &[f32],
        ) {
            assert!(
                !deltas.is_empty(),
                "resize drag path must include at least one move delta"
            );
            let separator_x = self.separator_viewport_x(separator_index);
            let y = 12.0;
            let mut driver = InteractionDriver::new();

            self.route_fused_events(
                driver.execute(InteractionCommand::pointer_down(separator_x, y)),
            );
            for dx in deltas {
                self.route_fused_events(
                    driver.execute(InteractionCommand::pointer_move(separator_x + *dx, y)),
                );
                self.layout();
            }
            let final_dx = *deltas.last().unwrap();
            self.route_fused_events(
                driver.execute(InteractionCommand::pointer_up(separator_x + final_dx, y)),
            );
            self.layout();

            assert!(
                !self
                    .domain_state
                    .pointer_capture_manager
                    .is_captured(PointerId::MOUSE)
            );
            assert!(!self.hscroll_freeze.get());
        }

        fn route_fused_events(&mut self, events: Vec<FusedInputEvent>) {
            for event in events {
                self.router.route_input_event(
                    &mut self.tree,
                    event,
                    Some(self.root),
                    &mut self.text_engine,
                    None,
                    &mut self.domain_state,
                    &mut self.binding_registry,
                );
            }
        }

        fn separator_viewport_x(&self, separator_index: usize) -> f32 {
            self.probe_snapshot().separators[separator_index].viewport_x
        }

        fn column_resolved_x(&self, column_id: ColumnId) -> f32 {
            self.probe_snapshot()
                .columns
                .into_iter()
                .find(|column| column.column_id == column_id)
                .unwrap()
                .resolved_x_offset
        }

        fn probe_snapshot(&self) -> ListViewResizeProbeSnapshot {
            let list_view = self
                .tree
                .node(self.root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
                .unwrap();
            list_view_resize_probe_snapshot(list_view, self.columns)
        }
    }

    fn laid_out_resize_probe_snapshot(
        initial_horizontal_scroll: f32,
    ) -> (ListViewResizeProbeSnapshot, ListViewResizeFixtureColumns) {
        let _guard = LayoutAuthorityGuard::enter();
        let fixture = ListViewResizeFixture::new();
        let columns = fixture.columns;
        let hscroll_link = fixture.horizontal_scroll_link.clone();
        let hscroll_freeze = fixture.horizontal_scroll_freeze.clone();
        hscroll_link.set(initial_horizontal_scroll);
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut text_engine = FontManagerTextEngine::new();

        let root = tree.create_root(Box::new(fixture.list_view), None, 0);
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link.clone())
            .with_horizontal_freeze(hscroll_freeze);
        let scroll_area_id = tree.add_child(root, Box::new(scroll_area), None, 0);
        let body_id = tree.add_child(scroll_area_id, Box::new(fixture.body), None, 0);
        tree.node_mut(scroll_area_id)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body_id);
        tree.add_child(root, Box::new(fixture.footer), None, 0);

        test_begin_frame_layout_tracking();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(
                LIST_VIEW_RESIZE_WINDOW_WIDTH,
                LIST_VIEW_RESIZE_WINDOW_HEIGHT,
            )),
            &theme,
            &mut text_engine,
            None,
        );

        let list_view = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
            .unwrap();
        (list_view_resize_probe_snapshot(list_view, columns), columns)
    }

    fn list_view_column_width(tree: &WidgetTree, root: NodeId, column_id: ColumnId) -> f32 {
        tree.node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
            .unwrap()
            .test_column_width(column_id)
            .unwrap()
    }

    fn list_view_preferred_column_width(
        tree: &WidgetTree,
        root: NodeId,
        column_id: ColumnId,
    ) -> f32 {
        tree.node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ListView<ListViewResizeFixtureSource>>()
            .unwrap()
            .test_column_preferred_width(column_id)
            .unwrap()
    }

    fn dispatch_to_list_view(
        tree: &mut WidgetTree,
        root: NodeId,
        widget_id: crate::utils::id::TreeWidgetId,
        event: Event,
        has_pointer_capture: bool,
    ) -> DispatchResult {
        let tree_ptr = tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            widget_id,
            tree_ptr,
            0,
            std::ptr::null_mut(),
            None,
            None,
            has_pointer_capture,
            false,
            KeyboardAuthorityMode::Navigation,
        );
        let handled =
            tree.node_mut(root)
                .unwrap()
                .widget
                .handle_event(&mut cx, &event, EventPhase::Target);

        DispatchResult {
            handled,
            capture_requested: cx.is_capture_requested().is_some(),
            release_requested: cx.is_release_requested(),
        }
    }

    fn pointer_event_from_fused(event: FusedInputEvent) -> Option<Event> {
        match event {
            FusedInputEvent::PointerDown { pos, button, .. } => {
                Some(pointer_event(PointerEventKind::Down, Some(button), pos))
            }
            FusedInputEvent::DragUpdate { current, .. } => {
                Some(pointer_event(PointerEventKind::Move, None, current))
            }
            FusedInputEvent::PointerUp { pos, button, .. } => {
                Some(pointer_event(PointerEventKind::Up, Some(button), pos))
            }
            _ => None,
        }
    }

    fn pointer_event(kind: PointerEventKind, button: Option<PointerButton>, pos: Point) -> Event {
        Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (pos.x, pos.y),
            local_pos: (pos.x, pos.y),
            kind,
            button,
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        })
    }
}
