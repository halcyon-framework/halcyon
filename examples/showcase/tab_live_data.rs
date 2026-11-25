// Showcase - High-Frequency Data tab.

use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::{Alignment, EdgeInsets, FlexParams};
use halcyon::core::text_selection::TextSelectionPolicy;
use halcyon::core::widget_tree::NodeId;
use halcyon::engines::cell_source::{Cell as SourceCell, CellSource, PropertyValue, Severity};
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::section_header::SectionHeader;
use halcyon::widgets::tabular_view::column::{Column as TreeColumn, ColumnModel};
use halcyon::widgets::tabular_view::data_adapter::ColumnInvalidationScope;
use halcyon::widgets::tree_view::TreeView;
use halcyon::widgets::{
    ReadoutCell, ReadoutColumn, ReadoutTable, StatusGrid, StatusGridCell, StatusGridMetric,
    StatusGridWideRow,
};

use super::*;

struct LiveTreeSource {
    revision: Rc<StdCell<u64>>,
    type_col_id: ColumnId,
    rev_col_id: ColumnId,
    load_col_id: ColumnId,
    rate_col_id: ColumnId,
    lag_col_id: ColumnId,
    state_col_id: ColumnId,
    node_depths: HashMap<u64, u32>,
}

impl LiveTreeSource {
    fn new(
        revision: Rc<StdCell<u64>>,
        type_col_id: ColumnId,
        rev_col_id: ColumnId,
        load_col_id: ColumnId,
        rate_col_id: ColumnId,
        lag_col_id: ColumnId,
        state_col_id: ColumnId,
        nodes: &[NodeMeta],
    ) -> Self {
        let node_depths = nodes
            .iter()
            .map(|node| (node.node_id, node.depth))
            .collect();
        Self {
            revision,
            type_col_id,
            rev_col_id,
            load_col_id,
            rate_col_id,
            lag_col_id,
            state_col_id,
            node_depths,
        }
    }

    fn wave(&self, row_id: u64, column_salt: u64, scale: f64, offset: f64) -> f64 {
        let tick = self.revision.get();
        let seed = row_id
            .wrapping_mul(73)
            .wrapping_add(column_salt.wrapping_mul(131));
        let speed = 5 + seed % 19;
        let phase = (tick.wrapping_mul(speed).wrapping_add(seed * 29) % 360) as f64;
        let range = scale * (0.55 + (seed % 37) as f64 / 80.0);
        offset + (phase.to_radians().sin() * 0.5 + 0.5) * range
    }
}

impl CellSource for LiveTreeSource {
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<SourceCell> {
        let depth = *self.node_depths.get(&row_id)?;
        let tick = self.revision.get();
        if column_id == self.type_col_id {
            return Some(SourceCell::read_only(PropertyValue::String(
                live_tree_type(depth).to_string(),
            )));
        }
        if column_id == self.rev_col_id {
            return Some(SourceCell::read_only(PropertyValue::String(format!(
                "r{:03}",
                tick.wrapping_add(row_id) % 1000
            ))));
        }
        if column_id == self.load_col_id {
            return Some(SourceCell::read_only(PropertyValue::String(format!(
                "{:.1}%",
                self.wave(row_id, 1, 82.0, 8.0).min(99.9)
            ))));
        }
        if column_id == self.rate_col_id {
            return Some(SourceCell::read_only(PropertyValue::String(format!(
                "{:.0}/s",
                self.wave(row_id, 2, 2600.0, 120.0)
            ))));
        }
        if column_id == self.lag_col_id {
            let lag = if row_id % 5 == 0 {
                self.wave(row_id, 3, 3.2, 0.2)
            } else {
                self.wave(row_id, 4, 140.0, 8.0)
            };
            let text = if row_id % 5 == 0 {
                format!("{lag:.2}s")
            } else {
                format!("{lag:.1}ms")
            };
            return Some(SourceCell::read_only(PropertyValue::String(text)));
        }
        if column_id == self.state_col_id {
            let (state, severity) = live_tree_state(row_id, tick);
            return Some(SourceCell::read_only(PropertyValue::Badge(
                state.to_string(),
                severity,
            )));
        }
        None
    }

    fn commit_cell(
        &mut self,
        _row_id: u64,
        _column_id: ColumnId,
        _new_value: PropertyValue,
    ) -> bool {
        false
    }

    fn revision(&self) -> u64 {
        self.revision.get()
    }
}

fn live_tree_type(depth: u32) -> &'static str {
    match depth {
        0 => "Session",
        1 => "Domain",
        2 => "Lane",
        _ => "Leaf",
    }
}

fn live_tree_state(row_id: u64, tick: u64) -> (&'static str, Severity) {
    match (row_id.wrapping_add(tick / 9)) % 17 {
        0 => ("WATCH", Severity::Warning),
        1 | 2 => ("BURST", Severity::Info),
        3 => ("DELAY", Severity::Warning),
        _ => ("LIVE", Severity::Success),
    }
}

fn build_live_tree_topology() -> Vec<NodeMeta> {
    let mut nodes = Vec::with_capacity(17);
    nodes.push(NodeMeta {
        node_id: 30_000,
        parent_index: None,
        depth: 0,
        has_children: true,
        label: "market.session".to_string(),
    });
    let groups = [
        ("ingress", ["mdf.ny4.a", "mdf.ny4.b", "telemetry.eu"]),
        ("routing", ["orders.router", "book.delta", "replay.shadow"]),
        (
            "execution",
            ["matcher.core", "journal.writer", "egress.bridge"],
        ),
        ("risk", ["risk.window", "sla.guard", "alerts.filter"]),
    ];
    for (group_index, (group, lanes)) in groups.iter().enumerate() {
        let parent_index = nodes.len();
        nodes.push(NodeMeta {
            node_id: 30_100 + group_index as u64,
            parent_index: Some(0),
            depth: 1,
            has_children: true,
            label: (*group).to_string(),
        });
        for (lane_index, lane) in lanes.iter().enumerate() {
            nodes.push(NodeMeta {
                node_id: 31_000 + group_index as u64 * 16 + lane_index as u64,
                parent_index: Some(parent_index),
                depth: 2,
                has_children: false,
                label: (*lane).to_string(),
            });
        }
    }
    nodes
}

#[derive(Debug, Clone)]
struct LiveDataRow {
    row_id: RowId,
    lane: &'static str,
    value: String,
    rate: String,
    latency: String,
    state: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct LiveDataColumns {
    lane: ColumnId,
    value: ColumnId,
    rate: ColumnId,
    latency: ColumnId,
    state: ColumnId,
}

struct LiveDataSource {
    rows: Vec<LiveDataRow>,
    row_ids: Vec<RowId>,
    columns: Option<LiveDataColumns>,
    events: Vec<DataInvalidationEvent>,
}

impl LiveDataSource {
    fn new(engine: &DataEngine, frame: u64) -> Self {
        let mut source = Self {
            rows: Vec::new(),
            row_ids: (0..18).map(|i| RowId::from_raw(10_000 + i)).collect(),
            columns: None,
            events: Vec::new(),
        };
        source.recompute(engine, frame);
        source
    }

    fn set_columns(
        &mut self,
        lane: ColumnId,
        value: ColumnId,
        rate: ColumnId,
        latency: ColumnId,
        state: ColumnId,
    ) {
        self.columns = Some(LiveDataColumns {
            lane,
            value,
            rate,
            latency,
            state,
        });
    }

    fn update_from_engine(&mut self, engine: &DataEngine, frame: u64) {
        self.recompute(engine, frame);
        self.events.push(DataInvalidationEvent::CellsInvalidated {
            rows: self.row_ids.clone(),
            columns: ColumnInvalidationScope::All,
        });
    }

    fn recompute(&mut self, engine: &DataEngine, frame: u64) {
        let pulse = (frame % 97) as f64;
        let rows = [
            (
                "market.ingress",
                engine.spark_a.value,
                engine.spark_a.value * 2.0 + pulse,
                engine.spark_b.value * 0.18,
                "live",
            ),
            (
                "orders.router",
                engine.delta.value,
                engine.spark_a.value * 1.4,
                engine.spark_b.value * 0.21,
                "nominal",
            ),
            (
                "matcher.core",
                engine.alpha.value,
                engine.spark_a.value * 1.1,
                engine.spark_b.value * 0.34,
                "active",
            ),
            (
                "risk.window",
                engine.omega.value.abs(),
                engine.spark_a.value * 0.7,
                engine.spark_b.value * 0.42,
                "armed",
            ),
            (
                "journal.writer",
                engine.gamma.value,
                engine.spark_a.value * 0.5,
                engine.spark_b.value * 0.63,
                "bursty",
            ),
            (
                "mdf.ny4.a",
                engine.beta.value,
                engine.spark_a.value * 2.6,
                engine.spark_b.value * 0.16,
                "synced",
            ),
            (
                "mdf.ny4.b",
                (engine.beta.value + 3.0).min(100.0),
                engine.spark_a.value * 2.4,
                engine.spark_b.value * 0.17,
                "synced",
            ),
            (
                "telemetry.eu",
                engine.delta.value * 0.8,
                engine.spark_a.value * 0.3,
                18.4 + pulse * 0.03,
                "delayed",
            ),
            (
                "allocator.pool",
                engine.beta.value * 0.9,
                engine.spark_a.value * 0.4,
                engine.spark_b.value * 0.12,
                "stable",
            ),
            (
                "gpu.queue",
                engine.omega.value.abs(),
                engine.spark_a.value * 0.2,
                engine.spark_b.value * 0.09,
                "active",
            ),
            (
                "replay.shadow",
                engine.gamma.value * 0.6,
                engine.spark_a.value * 0.15,
                2100.0 + pulse * 2.0,
                "replay",
            ),
            (
                "alerts.filter",
                (engine.alpha.value + engine.gamma.value) * 0.5,
                engine.spark_a.value * 0.12,
                engine.spark_b.value * 0.28,
                "watch",
            ),
            (
                "cutover.passive",
                engine.delta.value * 0.35,
                engine.spark_a.value * 0.08,
                engine.spark_b.value * 0.19,
                "armed",
            ),
            (
                "book.delta",
                engine.spark_a.value * 0.1,
                engine.spark_a.value * 1.9,
                engine.spark_b.value * 0.14,
                "live",
            ),
            (
                "sla.guard",
                99.982,
                engine.spark_a.value * 0.05,
                engine.spark_b.value * 0.11,
                "nominal",
            ),
            (
                "burst.bucket",
                4.2 + pulse * 0.01,
                engine.spark_a.value * 0.09,
                engine.spark_b.value * 0.31,
                "spiky",
            ),
            (
                "egress.bridge",
                engine.delta.value,
                engine.spark_a.value * 1.7,
                engine.spark_b.value * 0.23,
                "active",
            ),
            (
                "audit.tail",
                engine.gamma.value * 0.45,
                engine.spark_a.value * 0.2,
                engine.spark_b.value * 0.51,
                "stable",
            ),
        ];

        self.rows = rows
            .iter()
            .enumerate()
            .map(|(index, (lane, value, rate, latency, state))| LiveDataRow {
                row_id: self.row_ids[index],
                lane,
                value: format!("{value:.3}"),
                rate: format!("{rate:.1}/s"),
                latency: if *latency >= 1000.0 {
                    format!("{:.2}s", latency / 1000.0)
                } else {
                    format!("{latency:.2}ms")
                },
                state,
            })
            .collect();
    }
}

impl DataSource for LiveDataSource {
    fn row_ids(&self) -> &[RowId] {
        &self.row_ids
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(columns) = self.columns else {
            return ValueState::Pending;
        };
        let Some(row) = self.rows.iter().find(|entry| entry.row_id == row) else {
            return ValueState::Missing;
        };
        let value = if column == columns.lane {
            row.lane.to_string()
        } else if column == columns.value {
            row.value.clone()
        } else if column == columns.rate {
            row.rate.clone()
        } else if column == columns.latency {
            row.latency.clone()
        } else if column == columns.state {
            row.state.to_string()
        } else {
            return ValueState::Missing;
        };
        ValueState::Ready(CellValue::String(value))
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.events)
    }
}

fn live_data_readout_table(rows: Vec<Vec<ReadoutCell>>) -> ReadoutTable {
    ReadoutTable::new(
        vec![
            ReadoutColumn::new("LANE", 128.0),
            ReadoutColumn::new("VALUE", 124.0),
            ReadoutColumn::new("DELTA", 112.0),
            ReadoutColumn::new("STATE", 132.0),
        ],
        rows,
    )
    .with_selection_policy(TextSelectionPolicy::Enabled)
}

impl ShowcaseScreen {
    fn live_data_readout_rows(&self) -> Vec<Vec<ReadoutCell>> {
        let frame = self.live_data_frame % 1000;
        let tree_revision = self.live_data_tree_revision.get() % 1000;
        let ingress = self.engine.spark_a.value.clamp(0.0, 9999.0);
        let router = self.engine.delta.value.clamp(0.0, 999.0);
        let match_latency = self.engine.spark_b.value * 0.34;
        let risk_queue = self.engine.omega.value.abs();
        let replay_lag = 1.4 + (self.live_data_frame % 180) as f64 / 80.0;
        let active_workers = (34.0 + self.engine.beta.value / 10.0)
            .round()
            .clamp(32.0, 44.0) as u32;
        let invalidations = 18 * 5;
        vec![
            vec![
                ReadoutCell::new("market.ingress", false),
                ReadoutCell::new(format!("{ingress:.1}/s"), true),
                ReadoutCell::new(format!("rev{frame:03}"), true),
                ReadoutCell::new("live", true),
            ],
            vec![
                ReadoutCell::new("orders.router", false),
                ReadoutCell::new(format!("{router:.3}"), true),
                ReadoutCell::new(format!("{:.0}/s", self.engine.spark_a.value * 1.4), true),
                ReadoutCell::new("nominal", true),
            ],
            vec![
                ReadoutCell::new("matcher.p99", false),
                ReadoutCell::new(format!("{match_latency:.2}ms"), true),
                ReadoutCell::new(format!("{:.0}/s", self.engine.spark_a.value * 1.1), true),
                ReadoutCell::new("active", true),
            ],
            vec![
                ReadoutCell::new("risk.window", false),
                ReadoutCell::new(format!("{risk_queue:.1}%"), true),
                ReadoutCell::new(format!("{active_workers}/44"), true),
                ReadoutCell::new("armed", true),
            ],
            vec![
                ReadoutCell::new("replay.shadow", false),
                ReadoutCell::new(format!("{replay_lag:.2}s"), true),
                ReadoutCell::new(format!("{:.0}/s", self.engine.spark_a.value * 0.15), true),
                ReadoutCell::new("replay", true),
            ],
            vec![
                ReadoutCell::new("tree.topology", false),
                ReadoutCell::new(format!("r{tree_revision:03}"), true),
                ReadoutCell::new("20Hz", true),
                ReadoutCell::new("visible", true),
            ],
            vec![
                ReadoutCell::new("list.invalidated", false),
                ReadoutCell::new(invalidations.to_string(), true),
                ReadoutCell::new("cells", true),
                ReadoutCell::new("paint", true),
            ],
            vec![
                ReadoutCell::new("frame.clock", false),
                ReadoutCell::new(format!("{frame:03}"), true),
                ReadoutCell::new("paced", true),
                ReadoutCell::new("120fps", true),
            ],
        ]
    }

    fn live_data_session_cells(&self) -> Vec<StatusGridCell> {
        let ingress = self.engine.spark_a.value.clamp(0.0, 9999.0);
        let table_revision = self.live_data_frame % 1000;
        let active_workers = (34.0 + self.engine.beta.value / 10.0)
            .round()
            .clamp(32.0, 44.0) as u32;
        let sla = (99.995 - self.engine.spark_b.value / 20_000.0).clamp(99.900, 99.999);
        vec![
            StatusGridCell::new("INGRESS", format!("{ingress:.0}/s")),
            StatusGridCell::new("TABLE", format!("rev{table_revision:03}")),
            StatusGridCell::new("WORKERS", format!("{active_workers:02}/44")),
            StatusGridCell::new("SLA", format!("{sla:.3}%")),
            StatusGridCell::new("FRAME", format!("{table_revision:03}")),
        ]
    }

    fn live_data_probe_readout(
        &self,
    ) -> (
        Vec<StatusGridCell>,
        Vec<StatusGridWideRow>,
        Vec<StatusGridMetric>,
    ) {
        let pressure = self.engine.spark_a.value.clamp(0.0, 9999.0);
        let match_latency = self.engine.spark_b.value * 0.34;
        let replay_lag = 1.4 + (self.live_data_frame % 180) as f64 / 80.0;
        let burst = 1.0 + self.engine.gamma.value / 18.0;
        let invalidations = 18 * 5;
        (
            vec![
                StatusGridCell::new("PRESSURE", format!("{pressure:.1}/s")),
                StatusGridCell::new("MATCH P99", format!("{match_latency:.2}ms")),
                StatusGridCell::new("REPLAY", format!("{replay_lag:.2}s")),
                StatusGridCell::new("BURST", format!("{burst:.1}x")),
                StatusGridCell::new("QUEUE", format!("{:.1}%", self.engine.omega.value.abs())),
                StatusGridCell::new("GUARD", "armed"),
            ],
            vec![StatusGridWideRow::new(
                "HOT PATH",
                "market.ingress -> orders.router -> matcher.core -> egress.bridge",
            )],
            vec![
                StatusGridMetric::new("REV", format!("{:03}", self.live_data_frame % 1000)),
                StatusGridMetric::new("ROWS", "18"),
                StatusGridMetric::new("CELLS", invalidations.to_string()),
                StatusGridMetric::new("PACE", "on"),
            ],
        )
    }

    fn refresh_status_grid(
        tree: &mut halcyon::core::widget_tree::WidgetTree,
        node_id: Option<NodeId>,
        cells: Vec<StatusGridCell>,
        wide_rows: Vec<StatusGridWideRow>,
        metrics: Vec<StatusGridMetric>,
    ) {
        if let Some(node_id) = node_id {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(grid) = node.widget_mut().as_any_mut().downcast_mut::<StatusGrid>() {
                    grid.set_readout(cells, wide_rows, metrics);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }
    }

    fn refresh_readout_table(
        tree: &mut halcyon::core::widget_tree::WidgetTree,
        node_id: Option<NodeId>,
        rows: Vec<Vec<ReadoutCell>>,
    ) {
        if let Some(node_id) = node_id {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(table) = node
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<ReadoutTable>()
                {
                    table.set_rows(rows);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }
    }

    pub(super) fn refresh_live_data_table(
        &mut self,
        tree: &mut halcyon::core::widget_tree::WidgetTree,
    ) {
        self.live_data_frame = self.live_data_frame.wrapping_add(1);
        self.live_data_tree_revision.set(self.live_data_frame);
        Self::refresh_status_grid(
            tree,
            self.live_data_session_grid_id,
            self.live_data_session_cells(),
            Vec::new(),
            Vec::new(),
        );
        let (cells, wide_rows, metrics) = self.live_data_probe_readout();
        Self::refresh_status_grid(
            tree,
            self.live_data_probe_grid_id,
            cells,
            wide_rows,
            metrics,
        );
        Self::refresh_readout_table(
            tree,
            self.live_data_readout_table_id,
            self.live_data_readout_rows(),
        );
        let Some(list_id) = self.live_data_list_id else {
            return;
        };
        if let Some(node) = tree.node_mut(list_id) {
            if let Some(list_view) = node
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<ListView<LiveDataSource>>()
            {
                list_view
                    .adapter_mut()
                    .source_mut()
                    .update_from_engine(&self.engine, self.live_data_frame);
                list_view.refresh_visible_cells();
            }
        }
        if let Some(body_id) = self.live_data_body_id {
            tree.mark_paint_dirty(body_id);
        }
        if let Some(tree_id) = self.live_data_tree_id {
            if let Some(node) = tree.node_mut(tree_id) {
                if let Some(tree_view) = node.widget_mut().as_any_mut().downcast_mut::<TreeView>() {
                    tree_view.refresh_visible_cells();
                }
            }
        }
        if let Some(body_id) = self.live_data_tree_body_id {
            tree.mark_paint_dirty(body_id);
        }
    }

    pub(super) fn build_live_data(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let root = cx.tree.add_child(
            parent,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(8.0)),
            ),
            None,
            0,
        );
        cx.tree.set_flex(root, FlexParams::flexible(1.0));

        cx.tree.add_child(
            root,
            Box::new(SectionHeader::new("High-Frequency Data").with_secondary(
                "Venue pressure, execution latency, and risk state under continuous updates.",
            )),
            None,
            0,
        );

        self.live_data_tree_revision.set(self.live_data_frame);
        let session_grid = cx.tree.add_child(
            root,
            Box::new(
                StatusGrid::new(self.live_data_session_cells())
                    .with_columns(5)
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            1,
        );
        self.live_data_session_grid_id = Some(session_grid);
        cx.tree.set_flex(session_grid, FlexParams::fixed(54.0));

        let mut list_view = ListView::new(LiveDataSource::new(&self.engine, self.live_data_frame));
        let col_lane = list_view.add_column_raw("Lane", Column::new(150.0).with_flex(1.0));
        let col_value = list_view.add_column("Value", 88.0);
        let col_rate = list_view.add_column("Rate", 92.0);
        let col_latency = list_view.add_column("Latency", 86.0);
        let col_state = list_view.add_column("State", 78.0);
        list_view.adapter_mut().source_mut().set_columns(
            col_lane,
            col_value,
            col_rate,
            col_latency,
            col_state,
        );

        let hscroll_link = HorizontalScrollLink::new();
        let hscroll_freeze = Rc::new(Cell::new(false));
        list_view.set_horizontal_scroll_link(hscroll_link.clone(), hscroll_freeze.clone());

        let body_widget = list_view.create_body_widget();
        let footer_widget = list_view.create_footer_widget();
        let list_id = cx.tree.add_child(root, Box::new(list_view), None, 2);
        cx.tree.set_flex(list_id, FlexParams::flexible(1.0));
        self.live_data_list_id = Some(list_id);

        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze);
        let sa_id = cx.tree.add_child(list_id, Box::new(scroll_area), None, 0);
        let body_id = cx.tree.add_child(sa_id, Box::new(body_widget), None, 0);
        self.live_data_body_id = Some(body_id);
        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }
        cx.tree.add_child(list_id, Box::new(footer_widget), None, 0);

        let lower_row = cx.tree.add_child(
            root,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_align_items(Alignment::Start),
            ),
            None,
            3,
        );
        cx.tree.set_flex(lower_row, FlexParams::fixed(208.0));

        let readout_table = cx.tree.add_child(
            lower_row,
            Box::new(live_data_readout_table(self.live_data_readout_rows())),
            None,
            0,
        );
        self.live_data_readout_table_id = Some(readout_table);
        cx.tree.set_flex(readout_table, FlexParams::fixed(390.0));

        let (probe_cells, probe_wide_rows, probe_metrics) = self.live_data_probe_readout();
        let probe_grid = cx.tree.add_child(
            lower_row,
            Box::new(
                StatusGrid::new(probe_cells)
                    .with_columns(3)
                    .with_wide_rows(probe_wide_rows)
                    .with_metrics(probe_metrics)
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            1,
        );
        self.live_data_probe_grid_id = Some(probe_grid);
        cx.tree.set_flex(probe_grid, FlexParams::fixed(330.0));

        let mut tree_view = TreeView::new();
        tree_view.set_row_height(22.0);
        let tree_nodes = build_live_tree_topology();
        let mut tree_columns = ColumnModel::new();
        let tree_col = TreeColumn::new(172.0);
        let type_col = TreeColumn::new(72.0);
        let rev_col = TreeColumn::new(58.0);
        let load_col = TreeColumn::new(78.0);
        let rate_col = TreeColumn::new(84.0);
        let lag_col = TreeColumn::new(82.0);
        let state_col = TreeColumn::new(84.0);
        let tree_col_id = tree_col.id;
        let type_col_id = type_col.id;
        let rev_col_id = rev_col.id;
        let load_col_id = load_col.id;
        let rate_col_id = rate_col.id;
        let lag_col_id = lag_col.id;
        let state_col_id = state_col.id;
        tree_columns.push(tree_col);
        tree_columns.push(type_col);
        tree_columns.push(rev_col);
        tree_columns.push(load_col);
        tree_columns.push(rate_col);
        tree_columns.push(lag_col);
        tree_columns.push(state_col);
        tree_view.set_columns(tree_columns);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Topology");
        tree_view.set_column_title(type_col_id, "Type");
        tree_view.set_column_title(rev_col_id, "Rev");
        tree_view.set_column_title(load_col_id, "Load");
        tree_view.set_column_title(rate_col_id, "Rate");
        tree_view.set_column_title(lag_col_id, "Lag");
        tree_view.set_column_title(state_col_id, "State");
        let tree_source: Rc<RefCell<dyn CellSource>> = Rc::new(RefCell::new(LiveTreeSource::new(
            Rc::clone(&self.live_data_tree_revision),
            type_col_id,
            rev_col_id,
            load_col_id,
            rate_col_id,
            lag_col_id,
            state_col_id,
            &tree_nodes,
        )));
        tree_view.set_cell_source(Some(tree_source));
        tree_view.set_tree_data(tree_nodes);
        tree_view.set_footer_height(22.0);
        let tree_scroll_link = tree_view.horizontal_scroll_link();
        let tree_scroll_freeze = tree_view.horizontal_scroll_freeze();
        let tree_body_widget = tree_view.create_body_widget();
        let tree_footer_widget = tree_view.create_footer_widget();
        let tree_id = cx.tree.add_child(root, Box::new(tree_view), None, 4);
        self.live_data_tree_id = Some(tree_id);
        cx.tree.set_flex(tree_id, FlexParams::fixed(178.0));
        let tree_scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(tree_scroll_link)
            .with_horizontal_freeze(tree_scroll_freeze)
            .with_content_owned_vertical_translation();
        let tree_scroll_id = cx
            .tree
            .add_child(tree_id, Box::new(tree_scroll_area), None, 0);
        let tree_body_id = cx
            .tree
            .add_child(tree_scroll_id, Box::new(tree_body_widget), None, 0);
        self.live_data_tree_body_id = Some(tree_body_id);
        if let Some(node) = cx.tree.node_mut(tree_scroll_id) {
            if let Some(scroll_area) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                scroll_area.wire_content(tree_body_id);
            }
        }
        cx.tree
            .add_child(tree_id, Box::new(tree_footer_widget), None, 0);
    }
}
