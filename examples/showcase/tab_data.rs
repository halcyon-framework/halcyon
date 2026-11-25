use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::prelude::*;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::divider::Divider;
use halcyon::widgets::flow_grid::FlowGrid;
use halcyon::widgets::gauge::{
    Gauge, GaugeConfig, GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone,
};
use halcyon::widgets::groupbox::GroupBox;
use halcyon::widgets::linear_gauge::{
    LinearGauge, LinearGaugeConfig, LinearGaugeOrientation, paint_linear_gauge,
};
use halcyon::widgets::list_view::{CellPainter, DefaultCellPainter, ListView};
use halcyon::widgets::panel::Panel;
use halcyon::widgets::progress_bar::ProgressBar;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::sparkline::{Sparkline, SparklineConfig};
use halcyon::widgets::tab_bar::{TabBar, TabId};
use halcyon::widgets::tabular_view::Column;
use halcyon::widgets::tabular_view::column::ColumnId;
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;

struct ProcessRow {
    pid: i64,
    name: String,
    /// CPU usage percentage, 0.0–100.0.
    cpu: f64,
    /// Memory usage in megabytes (1 GB == 1024.0 MB).
    mem_mb: f64,
    status: String,
}

/// Column-id lookup populated after `ListView::add_column` calls return.
#[derive(Default)]
struct ProcessesColumns {
    pid: Option<ColumnId>,
    name: Option<ColumnId>,
    cpu: Option<ColumnId>,
    mem: Option<ColumnId>,
    status: Option<ColumnId>,
}

/// SPEC-HAL-188 `DataSource` for the Processes table.
///
/// Snapshot mode: six static mock rows, no invalidations. RowIds are
/// freshly generated per construction (each tab switch yields a fresh
/// substrate epoch, which is acceptable for showcase mock data).
struct ProcessesDataSource {
    rows: Vec<RowId>,
    data: Vec<ProcessRow>,
    columns: ProcessesColumns,
    pending_events: Vec<DataInvalidationEvent>,
}

impl ProcessesDataSource {
    fn new() -> Self {
        let data = vec![
            ProcessRow {
                pid: 1234,
                name: "halcyon".into(),
                cpu: 12.5,
                mem_mb: 245.0,
                status: "Running".into(),
            },
            ProcessRow {
                pid: 1456,
                name: "rustc".into(),
                cpu: 85.2,
                mem_mb: 1228.8,
                status: "Running".into(),
            },
            ProcessRow {
                pid: 892,
                name: "wgpu-server".into(),
                cpu: 3.1,
                mem_mb: 180.0,
                status: "Sleeping".into(),
            },
            ProcessRow {
                pid: 5678,
                name: "node".into(),
                cpu: 45.8,
                mem_mb: 512.0,
                status: "Running".into(),
            },
            ProcessRow {
                pid: 4321,
                name: "Code Helper".into(),
                cpu: 8.3,
                mem_mb: 320.0,
                status: "Running".into(),
            },
            ProcessRow {
                pid: 2109,
                name: "firefox".into(),
                cpu: 22.1,
                mem_mb: 1843.2,
                status: "Running".into(),
            },
        ];
        let rows: Vec<RowId> = data.iter().map(|_| RowId::new()).collect();
        Self {
            rows,
            data,
            columns: ProcessesColumns::default(),
            pending_events: Vec::new(),
        }
    }

    fn set_columns(
        &mut self,
        pid: ColumnId,
        name: ColumnId,
        cpu: ColumnId,
        mem: ColumnId,
        status: ColumnId,
    ) {
        self.columns = ProcessesColumns {
            pid: Some(pid),
            name: Some(name),
            cpu: Some(cpu),
            mem: Some(mem),
            status: Some(status),
        };
    }

    fn row_index(&self, row_id: RowId) -> Option<usize> {
        self.rows.iter().position(|&r| r == row_id)
    }
}

impl DataSource for ProcessesDataSource {
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
        let r = &self.data[idx];
        if Some(column) == self.columns.pid {
            ValueState::Ready(CellValue::Int(r.pid))
        } else if Some(column) == self.columns.name {
            ValueState::Ready(CellValue::String(r.name.clone()))
        } else if Some(column) == self.columns.cpu {
            ValueState::Ready(CellValue::Float(r.cpu))
        } else if Some(column) == self.columns.mem {
            ValueState::Ready(CellValue::Float(r.mem_mb))
        } else if Some(column) == self.columns.status {
            ValueState::Ready(CellValue::String(r.status.clone()))
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

/// SPEC-HAL-310 `CellPainter` for the Processes table.
///
/// CPU% and Mem columns paint a horizontal bar gauge via
/// `paint_linear_gauge` (SPEC-HAL-311). Other columns delegate to
/// `DefaultCellPainter` (text rendering). Mirrors the EventLog pattern
/// at `src/widgets/event_log/severity_painter.rs`.
///
/// Configs are built once and reused across paint calls (no per-frame
/// allocation). `show_value` is `false` because the 24-pixel substrate
/// row height does not accommodate both a 12-pixel bar and a line of
/// value text. The bar fill itself communicates the value.
struct ProcessesCellPainter {
    cpu_column: ColumnId,
    mem_column: ColumnId,
    cpu_config: LinearGaugeConfig,
    mem_config: LinearGaugeConfig,
    default: DefaultCellPainter,
}

impl ProcessesCellPainter {
    fn new(cpu_column: ColumnId, mem_column: ColumnId) -> Self {
        let cpu_config = LinearGaugeConfig {
            min: 0.0,
            max: 100.0,
            bar_height: 12.0,
            corner_radius: 2.0,
            show_value: false,
            ..Default::default()
        };
        let mem_config = LinearGaugeConfig {
            min: 0.0,
            max: 2048.0,
            bar_height: 12.0,
            corner_radius: 2.0,
            show_value: false,
            ..Default::default()
        };
        Self {
            cpu_column,
            mem_column,
            cpu_config,
            mem_config,
            default: DefaultCellPainter,
        }
    }
}

impl CellPainter for ProcessesCellPainter {
    fn paint_cell(
        &self,
        cx: &mut halcyon::core::widget::PaintContext,
        rect: halcyon::core::math::RectF,
        row_id: RowId,
        column_id: ColumnId,
        value: ValueState<CellValue>,
        shaped: Option<&halcyon::core::text_engine::GlyphRun>,
        selected: bool,
        font_size: f32,
        text_padding: f32,
    ) {
        if column_id == self.cpu_column || column_id == self.mem_column {
            let v = match value {
                ValueState::Ready(CellValue::Float(f)) => f,
                _ => return,
            };
            let cfg = if column_id == self.cpu_column {
                &self.cpu_config
            } else {
                &self.mem_config
            };
            // Center the bar vertically inside the row cell and inset
            // horizontally so the bar sits comfortably within the column
            // boundaries.
            let bar_h = cfg.bar_height;
            let bar_y = rect.y + (rect.height - bar_h) / 2.0;
            let inset = 4.0;
            let bar_w = (rect.width - 2.0 * inset).max(0.0);
            let bar_rect = halcyon::core::math::RectF::new(rect.x + inset, bar_y, bar_w, bar_h);
            paint_linear_gauge(cx, bar_rect, cfg, v);
        } else {
            self.default.paint_cell(
                cx,
                rect,
                row_id,
                column_id,
                value,
                shaped,
                selected,
                font_size,
                text_padding,
            );
        }
    }
}

impl ShowcaseScreen {
    pub(super) fn build_data(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        // SPEC-HAL-187/188 ListView consumer (TabularSubstrate projection).
        // Column geometry is owned by the substrate (INV-164-2): one
        // resolved width per column, shared by header / body / footer.
        // CPU% and Mem cells render horizontal bar gauges via a custom
        // CellPainter that calls paint_linear_gauge (SPEC-HAL-310 +
        // SPEC-HAL-311). Other columns delegate to DefaultCellPainter.

        let source = ProcessesDataSource::new();
        let mut list_view = ListView::new(source);

        let col_pid = list_view.add_column_raw("PID", Column::new(80.0).with_reorderable(false));
        let col_name = list_view.add_column_raw("Name", Column::new(200.0).with_flex(1.0));
        let col_cpu = list_view.add_column("CPU%", 160.0);
        let col_mem = list_view.add_column("Mem", 160.0);
        let col_status = list_view.add_column_raw(
            "Status",
            Column::new(100.0)
                .with_resizable(false)
                .with_reorderable(false),
        );

        list_view
            .adapter_mut()
            .source_mut()
            .set_columns(col_pid, col_name, col_cpu, col_mem, col_status);

        list_view.set_cell_painter(Box::new(ProcessesCellPainter::new(col_cpu, col_mem)));

        // Horizontal-scroll link binds header/body/footer to body scroll
        // (zero scroll here since columns fit, but wired per canonical
        // pattern in examples/listview.rs).
        let hscroll_link = HorizontalScrollLink::new();
        let hscroll_freeze = Rc::new(Cell::new(false));
        list_view.set_horizontal_scroll_link(hscroll_link.clone(), hscroll_freeze.clone());

        // Create body and footer widgets BEFORE moving list_view into the
        // tree — they share Rc<RefCell<ListViewCore>> state with it.
        let body_widget = list_view.create_body_widget();
        let footer_widget = list_view.create_footer_widget();

        // Tree:
        //   parent
        //     └─ ListView (flexible 1.0)
        //          ├─ ScrollArea (unwired, h-scrollbar on, link wired)
        //          │     └─ ListViewBody  ← scroll_area.wire_content(body_id)
        //          └─ ListViewFooter (0-height; required for resize completeness)
        let lv_id = cx.tree.add_child(parent, Box::new(list_view), None, 0);
        cx.tree.set_flex(lv_id, FlexParams::flexible(1.0));

        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze);
        let sa_id = cx.tree.add_child(lv_id, Box::new(scroll_area), None, 0);
        let body_id = cx.tree.add_child(sa_id, Box::new(body_widget), None, 0);
        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }
        cx.tree.add_child(lv_id, Box::new(footer_widget), None, 0);
    }
}
