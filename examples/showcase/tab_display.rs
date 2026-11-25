// Showcase — Indicators tab content.
// Method body moved from examples/showcase.rs:770-1090 (HEAD cb61e7e2).

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent};
use halcyon::core::layout::{Constraints, FlexParams, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{Color, LAYER_CONTENT, RoundedCorners};
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;
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
use web_time::{Duration, Instant};

use super::*;

struct ShapePrimitivePreview {
    id: WidgetInternalId,
}

impl ShapePrimitivePreview {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ShapePrimitivePreview {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ShapePrimitivePreview {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            520.0
        };
        LayoutResult::fit(Size::new(width.max(constraints.min_width), 148.0))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rounded_rect(
            rect,
            cx.theme.colors.surfaces.level1,
            RoundedCorners::uniform(4.0),
            1,
            None,
        );

        let left = rect.x + 18.0;
        let mid_y = rect.y + 58.0;
        let label_y = rect.y + rect.height - 28.0;
        let green = Color::from_hex(0x2ECC71);
        let blue = Color::from_hex(0x3498DB);
        let amber = Color::from_hex(0xE3A322);
        let red = Color::from_hex(0xE74C3C);

        cx.scene
            .push_circle(Point::new(left + 34.0, mid_y), 24.0, green, 2, None);
        cx.scene.push_ellipse(
            Point::new(left + 116.0, mid_y),
            Point::new(36.0, 22.0),
            blue,
            2,
            None,
        );
        cx.scene.push_line(
            Point::new(left + 184.0, mid_y + 24.0),
            Point::new(left + 268.0, mid_y - 24.0),
            4.0,
            amber,
            2,
            None,
        );
        cx.scene.push_polyline(
            vec![
                Point::new(left + 322.0, mid_y + 18.0),
                Point::new(left + 352.0, mid_y - 22.0),
                Point::new(left + 386.0, mid_y + 10.0),
                Point::new(left + 428.0, mid_y - 18.0),
                Point::new(left + 468.0, mid_y + 18.0),
            ],
            3.0,
            red,
            2,
            None,
        );

        for (i, label) in ["circle", "ellipse", "line", "polyline"].iter().enumerate() {
            let x = left + [14.0, 91.0, 200.0, 348.0][i];
            let run = cx.text_engine.shape(label, 11.0);
            cx.text_engine.draw(
                &run,
                x,
                label_y,
                cx.theme.colors.typography.secondary,
                LAYER_CONTENT + 1,
                cx.scene,
            );
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "ShapePrimitivePreview"
    }
}

impl ShowcaseScreen {
    pub(super) fn build_display(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        self.gauge_ids.clear();
        self.core_ids.clear();
        self.sparkline_ids.clear();
        self.spark_value_label_ids.clear();
        self.progress_ids.clear();
        // ScrollArea → Flex::column (content is direct child of scroll)
        let scroll = cx
            .tree
            .add_child(parent, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));

        let dashboard_col =
            cx.tree
                .add_child(scroll, Box::new(Flex::column().with_spacing(4.0)), None, 0);
        halcyon::replace_widget!(cx.tree, scroll, ScrollArea::new(dashboard_col));

        // ─── Section: System Gauges ──────────────────────
        let gauges_box = cx.tree.add_child(
            dashboard_col,
            Box::new(GroupBox::new(NodeId(0), "System Gauges")),
            None,
            0,
        );
        let gauge_col = cx.tree.add_child(
            gauges_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        let gauge_row1 =
            cx.tree
                .add_child(gauge_col, Box::new(Flex::row().with_spacing(8.0)), None, 0);
        let gauge_row2 =
            cx.tree
                .add_child(gauge_col, Box::new(Flex::row().with_spacing(8.0)), None, 1);
        halcyon::replace_widget!(
            cx.tree,
            gauges_box,
            GroupBox::new(gauge_col, "System Gauges")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );

        // Demo gauges — all fake data, various styles
        let gauge_specs: &[(&str, &Metric, Color, GaugeFillStyle, GaugeMode, f32, f64)] = &[
            (
                "CPU",
                &self.engine.alpha,
                Color::from_hex(0x3498DB),
                GaugeFillStyle::ZoneColored,
                GaugeMode::Fill,
                270.0,
                0.0,
            ),
            (
                "MEMORY",
                &self.engine.beta,
                Color::from_hex(0x3498DB),
                GaugeFillStyle::IntensityRamp,
                GaugeMode::Fill,
                270.0,
                0.0,
            ),
            (
                "DISK I/O",
                &self.engine.gamma,
                Color::from_hex(0x00BCD4),
                GaugeFillStyle::Solid,
                GaugeMode::Needle,
                270.0,
                0.0,
            ),
            (
                "NETWORK",
                &self.engine.delta,
                Color::from_hex(0xE74C3C),
                GaugeFillStyle::Solid,
                GaugeMode::Fill,
                360.0,
                0.0,
            ),
        ];

        let zones = vec![
            GaugeZone {
                start: 0.0,
                end: 30.0,
                color: Color::from_hex(0x3498DB), // blue — idle
            },
            GaugeZone {
                start: 30.0,
                end: 60.0,
                color: Color::from_hex(0x2ECC71), // green — normal
            },
            GaugeZone {
                start: 60.0,
                end: 85.0,
                color: Color::from_hex(0xF39C12), // orange — warning
            },
            GaugeZone {
                start: 85.0,
                end: 100.0,
                color: Color::from_hex(0xE74C3C), // red — critical
            },
        ];

        for (i, &(label, metric, color, fill_style, mode, arc_deg, origin)) in
            gauge_specs.iter().enumerate()
        {
            let g = Gauge::new()
                .with_config(GaugeConfig {
                    min: 0.0,
                    max: 100.0,
                    origin,
                    arc_degrees: arc_deg,
                    zones: if matches!(fill_style, GaugeFillStyle::ZoneColored) {
                        zones.clone()
                    } else {
                        Vec::new()
                    },
                    mode,
                    label: Some(label.to_string()),
                    fill_color: Some(color),
                    fill_style,
                    show_pointer: matches!(mode, GaugeMode::Fill),
                    thickness: if matches!(mode, GaugeMode::Needle) {
                        0.12
                    } else {
                        0.15
                    },
                    formatter: Some(self.fmt_val.clone()),
                    tick_marks: GaugeTickMarks {
                        major_divisions: 5,
                        minor_per_major: if matches!(mode, GaugeMode::Needle) {
                            0
                        } else {
                            1
                        },
                        show_labels: true,
                        ..Default::default()
                    },
                    live: true,
                    ..Default::default()
                })
                .with_value(metric.value);
            let (parent, order) = if i < 3 {
                (gauge_row1, i as i16)
            } else {
                (gauge_row2, (i - 3) as i16)
            };
            let g_id = cx.tree.add_child(parent, Box::new(g), None, order);
            cx.tree.set_flex(g_id, FlexParams::flexible(1.0));
            self.gauge_ids.push(g_id);
        }

        // GPU: Bidirectional sentiment gauge (origin at center = 0)
        let gpu_gauge = Gauge::new()
            .with_config(GaugeConfig {
                min: -100.0,
                max: 100.0,
                origin: 0.0,
                arc_degrees: 270.0,
                zones: Vec::new(),
                mode: GaugeMode::Fill,
                label: Some("GPU".to_string()),
                fill_color: Some(Color::from_hex(0x2ECC71)),
                fill_color_negative: Some(Color::from_hex(0xE74C3C)),
                fill_style: GaugeFillStyle::IntensityRamp,
                show_pointer: true,
                thickness: 0.15,
                formatter: Some(self.fmt_val.clone()),
                tick_marks: GaugeTickMarks {
                    major_divisions: 4,
                    minor_per_major: 1,
                    show_labels: true,
                    ..Default::default()
                },
                live: true,
                ..Default::default()
            })
            .with_value(0.0);
        let gpu_id = cx.tree.add_child(gauge_row2, Box::new(gpu_gauge), None, 1);
        cx.tree.set_flex(gpu_id, FlexParams::flexible(1.0));
        self.gauge_ids.push(gpu_id);

        // ─── Combined row: CPU Cores (left) + Metrics History (right) ───
        // ─── Section: Metrics ──────────────────────
        let metrics_box = cx.tree.add_child(
            dashboard_col,
            Box::new(GroupBox::new(NodeId(0), "Metrics")),
            None,
            1,
        );
        let metrics_grid = FlowGrid::new(180.0).with_spacing(8.0);
        let metrics_flex = metrics_grid.item_flex_params();
        let metrics_grid_id = cx
            .tree
            .add_child(metrics_box, Box::new(metrics_grid), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            metrics_box,
            GroupBox::new(metrics_grid_id, "System Metrics")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );

        let metric_specs: &[(&str, &Metric, Color)] = &[
            ("CPU", &self.engine.alpha, Color::from_hex(0x2ECC71)),
            ("Memory", &self.engine.beta, Color::from_hex(0x3498DB)),
            ("Disk I/O", &self.engine.gamma, Color::from_hex(0x00BCD4)),
            ("Network", &self.engine.delta, Color::from_hex(0xF39C12)),
            ("GPU", &self.engine.omega, Color::from_hex(0x1ABC9C)),
            (
                "Requests/s",
                &self.engine.spark_a,
                Color::from_hex(0x9B59B6),
            ),
            ("Latency", &self.engine.spark_b, Color::from_hex(0xE74C3C)),
        ];

        for (i, (name, metric, color)) in metric_specs.iter().enumerate() {
            let card = cx.tree.add_child(
                metrics_grid_id,
                Box::new(Flex::column().with_spacing(2.0)),
                None,
                i as i16,
            );
            cx.tree.set_flex(card, metrics_flex);

            // Row 1: name + value
            let header = cx.tree.add_child(card, Box::new(Flex::row()), None, 0);
            cx.tree
                .add_child(header, Box::new(Label::new(*name).with_size(11.0)), None, 0);
            cx.tree
                .add_child(header, Box::new(FixedSpacer::horizontal(6.0)), None, 1);
            let val_label_id = cx.tree.add_child(
                header,
                Box::new(Label::new(&format!("{:.1}", metric.value)).with_size(11.0)),
                None,
                2,
            );
            self.spark_value_label_ids.push(val_label_id);

            // Row 2: horizontal bar
            let lg = LinearGauge::new(LinearGaugeConfig {
                min: 0.0,
                max: match *name {
                    "Requests/s" => 1000.0,
                    "Latency" => 500.0,
                    _ => 100.0,
                },
                orientation: LinearGaugeOrientation::Horizontal,
                fill_color: Some(*color),
                bar_height: 6.0,
                corner_radius: 3.0,
                show_value: false,
                live: true,
                ..Default::default()
            })
            .with_value(metric.value);
            let lg_id = cx.tree.add_child(card, Box::new(lg), None, 1);
            self.core_ids.push(lg_id);

            // Row 3: sparkline
            let data: Vec<f64> = metric
                .history
                .as_ref()
                .map(|h| h.iter().copied().collect())
                .unwrap_or_default();
            let spark = Sparkline::new()
                .with_config(SparklineConfig {
                    width: 160.0,
                    height: 20.0,
                    color: Some(*color),
                    fill: true,
                    live: true,
                })
                .with_data(data);
            let spark_id = cx.tree.add_child(card, Box::new(spark), None, 2);
            self.sparkline_ids.push(spark_id);
        }

        // ─── Section: Shape Primitives ──────────────────────
        let shapes_box = cx.tree.add_child(
            dashboard_col,
            Box::new(GroupBox::new(NodeId(0), "Shape Primitives")),
            None,
            2,
        );
        let shapes_col = cx.tree.add_child(
            shapes_box,
            Box::new(Flex::column().with_spacing(6.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            shapes_box,
            GroupBox::new(shapes_col, "Shape Primitives")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        cx.tree.add_child(
            shapes_col,
            Box::new(Label::new("SPEC-R-003 primitives through PaintSink")),
            None,
            0,
        );
        cx.tree
            .add_child(shapes_col, Box::new(ShapePrimitivePreview::new()), None, 1);

        // ─── Section: System Status ──────────────────────
        let status_box = cx.tree.add_child(
            dashboard_col,
            Box::new(GroupBox::new(NodeId(0), "System Status")),
            None,
            3,
        );
        let status_row = cx.tree.add_child(
            status_box,
            Box::new(Flex::row().with_wrap().with_spacing(16.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            status_box,
            GroupBox::new(status_row, "System Status")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );

        let pb1 = ProgressBar::new()
            .with_value((self.engine.beta.value / 100.0) as f32)
            .with_label("Disk Usage")
            .with_display_style(halcyon::core::paint::TrackDisplayStyle::Pill);
        let pb1_id = cx.tree.add_child(status_row, Box::new(pb1), None, 0);
        cx.tree.set_flex(pb1_id, FlexParams::flexible(1.0));
        self.progress_ids.push(pb1_id);

        let pb2 = ProgressBar::new()
            .with_value((self.engine.omega.value / 100.0) as f32)
            .with_label("Memory Pool")
            .with_display_style(halcyon::core::paint::TrackDisplayStyle::Pill);
        let pb2_id = cx.tree.add_child(status_row, Box::new(pb2), None, 1);
        cx.tree.set_flex(pb2_id, FlexParams::flexible(1.0));
        self.progress_ids.push(pb2_id);

        // BusyIndicator — completes locked Display spec
        // (label / progress / busy / gauge / linear gauge / sparkline).
        let bi =
            halcyon::widgets::busy_indicator::BusyIndicator::new().with_label("Indexing files...");
        cx.tree.add_child(status_row, Box::new(bi), None, 2);

        // Bottom padding
        cx.tree
            .add_child(dashboard_col, Box::new(FixedSpacer::vertical(4.0)), None, 4);
    }
}
