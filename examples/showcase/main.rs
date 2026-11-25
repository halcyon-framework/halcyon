// Halcyon — Showcase App
// Single-screen demo with tabbed navigation, breadcrumbs, and status bar.
// Content for each section is added in subsequent tasks.

#![allow(unused_imports)]

use halcyon::core::app::{
    App, AppConfig, ChildWindowPaintContext, ChildWindowSceneState, Screen, ScreenContext,
    ScreenFrameContext,
};
use halcyon::core::event::{FusedInputEvent, WindowId};
use halcyon::core::layout::{Alignment, Constraints, EdgeInsets, FlexParams, LayoutResult};
use halcyon::core::math::RectF;
use halcyon::core::paint::Color;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::signal_bus::{SignalPayload, SignalSource};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::form_binder::FormBinder;
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::divider::Divider;
use halcyon::widgets::list_view::ListView;
use halcyon::widgets::panel::Panel;
use halcyon::widgets::switch::{LabelSide, Switch};
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
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};

mod menu;
mod tab_console;
mod tab_display;
mod tab_event_log;
mod tab_inputs;
mod tab_list_view;
mod tab_live_data;
mod tab_navigator;
mod tab_node_graph;
mod tab_order_book;
mod tab_overlays;
mod tab_plot;
mod tab_plot_grids;
mod tab_property_editor;
mod tab_tree_view;
mod tab_viewport_3d;
mod tab_workbench;
mod tip;

// ─── Section enum ────────────────────────────────────────
//
// Focused widget/workflow sections. Each variant maps to one tab.
// Tab order = enum declaration order (tab_id derives from discriminant).
// Per-tab content is built by a dedicated `build_<section>` method;
// tabs without a dedicated method fall through to the placeholder
// Label in `build()`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    InputsAndControls,
    Display,
    Plot,
    PropertyEditor,
    TreeView,
    ListView,
    EventLog,
    Console,
    Navigator,
    Overlays,
    LiveData,
    PlotQuad,
    PlotDashboard,
    OrderBook,
    Workbench,
    NodeGraph,
    Viewport3D,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Section::InputsAndControls => "Inputs and Controls",
            Section::Display => "Gauges",
            Section::Plot => "Plot",
            Section::PropertyEditor => "Property Editor",
            Section::TreeView => "Tree View",
            Section::ListView => "List View",
            Section::EventLog => "Event Log",
            Section::Console => "Console",
            Section::Navigator => "Navigator",
            Section::Overlays => "Overlays and Windows",
            Section::LiveData => "HF Data",
            Section::PlotQuad => "Plot x4",
            Section::PlotDashboard => "Plot x20",
            Section::OrderBook => "Order Book",
            Section::Workbench => "Workbench",
            Section::NodeGraph => "Node Graph",
            Section::Viewport3D => "Viewport 3D",
        }
    }

    fn tab_id(self) -> TabId {
        TabId(self as u32 + 1)
    }

    fn from_tab_id(id: TabId) -> Option<Self> {
        match id.0 {
            1 => Some(Section::InputsAndControls),
            2 => Some(Section::Display),
            3 => Some(Section::Plot),
            4 => Some(Section::PropertyEditor),
            5 => Some(Section::TreeView),
            6 => Some(Section::ListView),
            7 => Some(Section::EventLog),
            8 => Some(Section::Console),
            9 => Some(Section::Navigator),
            10 => Some(Section::Overlays),
            11 => Some(Section::LiveData),
            12 => Some(Section::PlotQuad),
            13 => Some(Section::PlotDashboard),
            14 => Some(Section::OrderBook),
            15 => Some(Section::Workbench),
            16 => Some(Section::NodeGraph),
            17 => Some(Section::Viewport3D),
            _ => None,
        }
    }

    fn all() -> &'static [Section] {
        &[
            Section::InputsAndControls,
            Section::Display,
            Section::Plot,
            Section::PlotQuad,
            Section::PlotDashboard,
            Section::OrderBook,
            Section::PropertyEditor,
            Section::TreeView,
            Section::ListView,
            Section::EventLog,
            Section::Console,
            Section::Navigator,
            Section::Overlays,
            Section::LiveData,
            Section::Workbench,
            Section::NodeGraph,
            Section::Viewport3D,
        ]
    }
}

// ─── Random target generator ─────────────────────────────

struct JumpGenerator {
    value: f64,
    min: f64,
    max: f64,
    seed: u64,
    interval: Duration,
    last_jump: Instant,
}

impl JumpGenerator {
    fn new(initial: f64, min: f64, max: f64, seed: u64, interval_ms: u64) -> Self {
        Self {
            value: initial,
            min,
            max,
            seed,
            interval: Duration::from_millis(interval_ms),
            last_jump: Instant::now(),
        }
    }

    fn poll(&mut self) -> Option<f64> {
        let now = Instant::now();
        if now.duration_since(self.last_jump) >= self.interval {
            self.last_jump = now;
            self.seed = self
                .seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let rand_norm = ((self.seed >> 32) as f64) / (u32::MAX as f64);
            self.value = self.min + rand_norm * (self.max - self.min);
            Some(self.value)
        } else {
            None
        }
    }
}

// ─── Spring-damper ───────────────────────────────────────

struct SpringDamper {
    position: f64,
    velocity: f64,
    target: f64,
}

impl SpringDamper {
    fn new(initial: f64) -> Self {
        Self {
            position: initial,
            velocity: 0.0,
            target: initial,
        }
    }

    fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    fn step(&mut self, dt: f64) -> f64 {
        let diff = self.target - self.position;
        if diff.abs() < 0.01 && self.velocity.abs() < 0.01 {
            self.position = self.target;
            self.velocity = 0.0;
            return self.position;
        }

        // Substep for numerical stability at low frame rates.
        // Explicit Euler diverges with stiffness=400 when dt > ~0.01.
        let max_sub_dt = 0.004;
        let substeps = ((dt / max_sub_dt).ceil() as usize).max(1);
        let sub_dt = dt / substeps as f64;

        let stiffness = 400.0;
        let damping = 40.0;

        for _ in 0..substeps {
            let diff = self.target - self.position;
            let force = diff * stiffness - self.velocity * damping;
            self.velocity += force * sub_dt;
            self.position += self.velocity * sub_dt;
        }
        self.position
    }
}

// ─── Metric ──────────────────────────────────────────────

struct Metric {
    jump: JumpGenerator,
    spring: SpringDamper,
    value: f64,
    history: Option<VecDeque<f64>>,
}

impl Metric {
    fn new(
        initial: f64,
        min: f64,
        max: f64,
        seed: u64,
        interval_ms: u64,
        keep_history: bool,
    ) -> Self {
        Self {
            jump: JumpGenerator::new(initial, min, max, seed, interval_ms),
            spring: SpringDamper::new(initial),
            value: initial,
            history: if keep_history {
                Some(VecDeque::with_capacity(360))
            } else {
                None
            },
        }
    }

    fn tick(&mut self, dt: f64) {
        if let Some(new_val) = self.jump.poll() {
            self.spring.set_target(new_val);
        }
        self.value = self.spring.step(dt);
        if let Some(ref mut hist) = self.history {
            hist.push_back(self.value);
            if hist.len() > 360 {
                hist.pop_front();
            }
        }
    }
}

// ─── DataEngine ──────────────────────────────────────────

struct DataEngine {
    // Fake demo metrics for radial gauges
    alpha: Metric,
    beta: Metric,
    gamma: Metric,
    delta: Metric,
    omega: Metric,

    // Fake sparkline-only metrics
    spark_a: Metric,
    spark_b: Metric,

    // Uptime tracking
    start_time: Instant,

    // Timing
    last_frame: Instant,
}

impl DataEngine {
    fn new() -> Self {
        let now = Instant::now();

        Self {
            alpha: Metric::new(45.0, 0.0, 100.0, 0xDEAD_BEEF, 37, true),
            beta: Metric::new(60.0, 0.0, 100.0, 0xCAFE_BABE, 62, true),
            gamma: Metric::new(30.0, 0.0, 100.0, 0x1337_C0DE, 100, true),
            delta: Metric::new(15.0, 0.0, 100.0, 0xBAAD_F00D, 80, true),
            omega: Metric::new(0.0, -100.0, 100.0, 0xFACE_CAFE, 120, true),
            spark_a: Metric::new(500.0, 0.0, 1000.0, 0xB001_0001, 33, true),
            spark_b: Metric::new(200.0, 0.0, 500.0, 0xB001_0002, 55, true),
            start_time: now,
            last_frame: now,
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();

        // ALL metrics use fake SpringDamper data
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        self.alpha.tick(dt);
        self.beta.tick(dt);
        self.gamma.tick(dt);
        self.delta.tick(dt);
        self.omega.tick(dt);
        self.spark_a.tick(dt);
        self.spark_b.tick(dt);
    }

    fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

// ─── Process metrics (status bar) ────────────────────────
//
// A background thread samples THIS process's CPU% and resident memory via
// `sysinfo` at the crate minimum CPU interval (~200ms — cpu_usage() is a diff
// and reads zero if sampled faster). It writes the latest sample into a shared
// Mutex<ProcSample> and NEVER touches the widget tree. The status bar reads the
// shared sample every frame in `refresh_proc_label` (Screen::update), which is
// the sanctioned per-frame ingress — external data enters engine state only
// inside the frame loop, never from the thread (SPEC-HAL-002 Pillars B and F).
// The real-time perception comes from the per-frame label refresh, not from the
// sampling rate.

#[derive(Clone, Copy, Default)]
struct ProcSample {
    cpu: f32,
    mem_mb: f64,
    ready: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_proc_sampler(shared: Arc<Mutex<ProcSample>>) {
    std::thread::spawn(move || {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new();
        // First sample establishes the CPU baseline; the value is only
        // meaningful after a second sample one interval later.
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        loop {
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            if let Some(proc_) = sys.process(pid) {
                let cpu = proc_.cpu_usage();
                let mem_mb = proc_.memory() as f64 / (1024.0 * 1024.0);
                if let Ok(mut s) = shared.lock() {
                    s.cpu = cpu;
                    s.mem_mb = mem_mb;
                    s.ready = true;
                }
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn proc_status_text(metrics: &Mutex<ProcSample>) -> String {
    match metrics.lock() {
        Ok(s) if s.ready => format!("CPU {:>5.1}% · {:>4.0} MB", s.cpu, s.mem_mb),
        _ => "CPU --% · -- MB".to_string(),
    }
}

#[cfg(target_arch = "wasm32")]
fn proc_status_text(_metrics: &Mutex<ProcSample>) -> String {
    "Process CPU/MEM unavailable in browser".to_string()
}

// ─── LiveStat (status-bar live metric) ───────────────────
//
// SPEC-HAL-354: telemetry needs update ticks, not continuous paint. The screen
// invalidates this owner only when set_text reports a changed readout.
// Rendering and pointer transparency remain the same as an ordinary Label.
struct LiveStat {
    inner: Label,
}

impl LiveStat {
    fn new(text: impl Into<String>) -> Self {
        Self {
            inner: Label::new(text),
        }
    }

    fn set_text(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        if self.inner.text == text {
            false
        } else {
            self.inner.set_text(text);
            true
        }
    }
}

impl WidgetDowncast for LiveStat {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for LiveStat {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.inner.layout(cx, constraints, children)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.inner.paint(cx, rect, children);
    }

    fn id(&self) -> WidgetInternalId {
        self.inner.id()
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "LiveStat"
    }

    fn needs_update(&self) -> bool {
        true
    }
}

// ─── Screen ──────────────────────────────────────────────

struct ShowcaseScreen {
    placeholder: Label,
    current_section: Section,
    tab_sink: TabBarSink,
    breadcrumb_sink: BreadcrumbsSink,
    /// Form bindings — currently the header theme toggle's "dark_mode" key.
    binder: FormBinder,
    engine: DataEngine,
    update_count: u64,
    last_update_rate_sample: Instant,
    updates_per_second: f64,
    needs_rebuild: bool,
    /// Retained TabBar node + its last-known horizontal scroll, persisted across
    /// the rebuild a tab selection triggers (the strip is recreated each build()).
    tab_bar_id: Option<NodeId>,
    tab_scroll_x: f32,
    // Pre-allocated formatter closures (avoid per-frame Arc allocation)
    fmt_val: Arc<dyn Fn(f64) -> String>,
    // Retained widget NodeIds for data updates (avoid rebuild)
    gauge_ids: Vec<NodeId>,
    core_ids: Vec<NodeId>,
    sparkline_ids: Vec<NodeId>,
    spark_value_label_ids: Vec<NodeId>,
    progress_ids: Vec<NodeId>,
    live_data_frame: u64,
    live_data_session_grid_id: Option<NodeId>,
    live_data_readout_table_id: Option<NodeId>,
    live_data_probe_grid_id: Option<NodeId>,
    live_data_list_id: Option<NodeId>,
    live_data_body_id: Option<NodeId>,
    live_data_tree_id: Option<NodeId>,
    live_data_tree_body_id: Option<NodeId>,
    live_data_tree_revision: Rc<Cell<u64>>,
    update_rate_label_id: Option<NodeId>,
    uptime_label_id: Option<NodeId>,
    proc_label_id: Option<NodeId>,
    /// Latest process CPU/mem sample, written by the `sysinfo` sampler thread,
    /// read each frame by `refresh_proc_label`. The thread never touches the
    /// widget tree (SPEC-HAL-002 Pillars B/F).
    proc_metrics: Arc<Mutex<ProcSample>>,
    // Console tab streaming state (SPEC-HAL-305). `console_id` is the live
    // Console node while the Console tab is mounted; the rest drive the line
    // generator (see tab_console.rs).
    console_id: Option<NodeId>,
    console_seed: u64,
    console_start: Instant,
    console_next_gen: Instant,
    console_burst_remaining: u32,
    overlay_context_menu: Option<(
        halcyon::model::context_menu::ContextMenuHandle,
        halcyon::model::context_menu::SelectionSink,
    )>,
    overlay_status_grid_id: Option<NodeId>,
    overlay_toast_manager: Option<Rc<std::cell::RefCell<halcyon::model::toast::ToastManager>>>,
    overlay_menu_button_id: Option<NodeId>,
    overlay_dialog_handle: Option<halcyon::model::dialog::DialogHandle>,
    overlay_dialog_strict: bool,
    overlay_status: String,
    overlay_child_window_id: Option<WindowId>,
    overlay_child_value: String,
    plot_render_pool: Option<Arc<halcyon::plot::PlotRenderPool>>,
    _plot_data_tx: Option<crossbeam_channel::Sender<halcyon::plot::DataUpdate>>,
    _plot_ticker_handle: Option<std::thread::JoinHandle<()>>,
    viewport_3d_id: Option<NodeId>,
    viewport_3d_fly_control_id: Option<NodeId>,
    viewport_3d_target_label_id: Option<NodeId>,
    viewport_3d_revision_label_id: Option<NodeId>,
    viewport_3d_lifecycle_label_id: Option<NodeId>,
    #[cfg(feature = "render-audit")]
    viewport_3d_cadence_label_ids: [Option<NodeId>; 3],
    viewport_3d_last_diagnostics: Option<Instant>,
    viewport_3d_fly: bool,
    viewport_3d_orthographic: bool,
    viewport_3d_msaa_4x: bool,
    viewport_3d_double_sided: bool,
    viewport_3d_transform_target: Option<u32>,
}

impl ShowcaseScreen {
    fn new() -> Self {
        let proc_metrics = Arc::new(Mutex::new(ProcSample::default()));
        #[cfg(not(target_arch = "wasm32"))]
        spawn_proc_sampler(proc_metrics.clone());
        Self {
            placeholder: Label::new(""),
            current_section: Section::InputsAndControls,
            tab_sink: tab_bar_sink(),
            breadcrumb_sink: breadcrumbs_sink(),
            binder: FormBinder::new(),
            engine: DataEngine::new(),
            update_count: 0,
            last_update_rate_sample: Instant::now(),
            updates_per_second: 0.0,
            needs_rebuild: true,
            tab_bar_id: None,
            tab_scroll_x: 0.0,
            fmt_val: Arc::new(|v: f64| format!("{:.1}", v)),
            gauge_ids: Vec::new(),
            core_ids: Vec::new(),
            sparkline_ids: Vec::new(),
            spark_value_label_ids: Vec::new(),
            progress_ids: Vec::new(),
            live_data_frame: 0,
            live_data_session_grid_id: None,
            live_data_readout_table_id: None,
            live_data_probe_grid_id: None,
            live_data_list_id: None,
            live_data_body_id: None,
            live_data_tree_id: None,
            live_data_tree_body_id: None,
            live_data_tree_revision: Rc::new(Cell::new(0)),
            update_rate_label_id: None,
            uptime_label_id: None,
            proc_label_id: None,
            proc_metrics,
            console_id: None,
            console_seed: 0xDEAD_BEEF,
            console_start: Instant::now(),
            console_next_gen: Instant::now(),
            console_burst_remaining: 0,
            overlay_context_menu: None,
            overlay_status_grid_id: None,
            overlay_toast_manager: None,
            overlay_menu_button_id: None,
            overlay_dialog_handle: None,
            overlay_dialog_strict: false,
            overlay_status: "No overlay action yet".to_string(),
            overlay_child_window_id: None,
            overlay_child_value: "Initial Value".to_string(),
            plot_render_pool: None,
            _plot_data_tx: None,
            _plot_ticker_handle: None,
            viewport_3d_id: None,
            viewport_3d_fly_control_id: None,
            viewport_3d_target_label_id: None,
            viewport_3d_revision_label_id: None,
            viewport_3d_lifecycle_label_id: None,
            #[cfg(feature = "render-audit")]
            viewport_3d_cadence_label_ids: [None; 3],
            viewport_3d_last_diagnostics: None,
            viewport_3d_fly: true,
            viewport_3d_orthographic: false,
            viewport_3d_msaa_4x: true,
            viewport_3d_double_sided: false,
            viewport_3d_transform_target: Some(103),
        }
    }
}

impl ShowcaseScreen {
    fn update_values(&self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        use halcyon::widgets::gauge::Gauge;
        use halcyon::widgets::linear_gauge::LinearGauge;
        use halcyon::widgets::sparkline::Sparkline;

        // Update main gauges
        let gauge_metrics = [
            &self.engine.alpha,
            &self.engine.beta,
            &self.engine.gamma,
            &self.engine.delta,
            &self.engine.omega,
        ];
        for (&node_id, metric) in self.gauge_ids.iter().zip(gauge_metrics.iter()) {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(gauge) = node.widget_mut().as_any_mut().downcast_mut::<Gauge>() {
                    gauge.set_value(metric.value);
                }
                true
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }

        // Update core gauges
        let core_bar_metrics = [
            &self.engine.alpha,
            &self.engine.beta,
            &self.engine.gamma,
            &self.engine.delta,
            &self.engine.omega,
            &self.engine.spark_a,
            &self.engine.spark_b,
        ];
        for (&node_id, metric) in self.core_ids.iter().zip(core_bar_metrics.iter()) {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(lg) = node.widget_mut().as_any_mut().downcast_mut::<LinearGauge>() {
                    lg.set_value(metric.value);
                }
                true
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }

        // Update sparklines
        let spark_metrics: Vec<&Metric> = vec![
            &self.engine.alpha,
            &self.engine.beta,
            &self.engine.gamma,
            &self.engine.delta,
            &self.engine.omega,
            &self.engine.spark_a,
            &self.engine.spark_b,
        ];
        for (&node_id, metric) in self.sparkline_ids.iter().zip(spark_metrics.iter()) {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(spark) = node.widget_mut().as_any_mut().downcast_mut::<Sparkline>() {
                    if let Some(hist) = &metric.history {
                        spark.set_data(hist.iter().copied().collect());
                    }
                }
                true
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }

        // Update sparkline value labels
        let spark_label_metrics: Vec<&Metric> = vec![
            &self.engine.alpha,
            &self.engine.beta,
            &self.engine.gamma,
            &self.engine.delta,
            &self.engine.omega,
            &self.engine.spark_a,
            &self.engine.spark_b,
        ];
        for (&node_id, metric) in self
            .spark_value_label_ids
            .iter()
            .zip(spark_label_metrics.iter())
        {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(label) = node.widget_mut().as_any_mut().downcast_mut::<Label>() {
                    label.set_text(format!("{:.1}", metric.value));
                }
                true
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }

        // Update progress bars
        let pb_values = [
            (self.engine.beta.value / 100.0) as f32,
            (self.engine.omega.value / 100.0) as f32,
        ];
        for (&node_id, &val) in self.progress_ids.iter().zip(pb_values.iter()) {
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(pb) = node
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<halcyon::widgets::progress_bar::ProgressBar>()
                {
                    pb.set_value(val);
                }
                true
            } else {
                false
            };
            if updated {
                tree.mark_paint_dirty(node_id);
            }
        }
    }

    /// Refresh the status-bar update-rate + uptime labels. Both live in the persistent
    /// status bar (present on every tab), so — unlike `update_values`, which is
    /// Display-only (gated on `gauge_ids`) — this MUST run every frame on every
    /// tab, the same way `refresh_proc_label` does.
    fn refresh_clock_labels(&self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        if let Some(node_id) = self.update_rate_label_id {
            let text = format!("{:>3.0} screen updates/s", self.updates_per_second);
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(label) = node.widget_mut().as_any_mut().downcast_mut::<Label>() {
                    if label.text == text {
                        false
                    } else {
                        label.set_text(text);
                        true
                    }
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

        if let Some(node_id) = self.uptime_label_id {
            let uptime = self.engine.uptime();
            let text = format!(
                "uptime: {:02}:{:02}:{:02}",
                uptime.as_secs() / 3600,
                (uptime.as_secs() % 3600) / 60,
                uptime.as_secs() % 60,
            );
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(label) = node.widget_mut().as_any_mut().downcast_mut::<Label>() {
                    if label.text == text {
                        false
                    } else {
                        label.set_text(text);
                        true
                    }
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

    fn refresh_proc_label(&self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        if let Some(node_id) = self.proc_label_id {
            let text = proc_status_text(&self.proc_metrics);
            let updated = if let Some(node) = tree.node_mut(node_id) {
                if let Some(stat) = node.widget_mut().as_any_mut().downcast_mut::<LiveStat>() {
                    stat.set_text(text)
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
}

impl Screen for ShowcaseScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.current_section != Section::Plot {
            self.plot_render_pool = None;
        }

        // Clear retained IDs — tree is being rebuilt
        self.gauge_ids.clear();
        self.core_ids.clear();
        self.sparkline_ids.clear();
        self.spark_value_label_ids.clear();
        self.progress_ids.clear();
        self.live_data_session_grid_id = None;
        self.live_data_readout_table_id = None;
        self.live_data_probe_grid_id = None;
        self.live_data_list_id = None;
        self.live_data_body_id = None;
        self.live_data_tree_id = None;
        self.live_data_tree_body_id = None;
        self.update_rate_label_id = None;
        self.uptime_label_id = None;
        self.proc_label_id = None;
        self.overlay_status_grid_id = None;
        self.overlay_toast_manager = None;
        self.overlay_menu_button_id = None;
        self.viewport_3d_id = None;
        self.viewport_3d_fly_control_id = None;
        self.viewport_3d_target_label_id = None;
        self.viewport_3d_revision_label_id = None;
        self.viewport_3d_lifecycle_label_id = None;

        // Writer for the header theme toggle (read back in flush_pending_actions).
        let theme_writer = self.binder.writer();

        // BG prepass: Panel as root (bare Flex causes INV-BG-PREPASS crash).
        // Neither the Panel nor the outer Flex bakes a background: a full-bleed
        // Panel with no background paints theme.colors.surfaces.base0 live at
        // paint, and the Flex stays transparent over it. The whole root surface
        // is therefore theme-reactive, so a theme switch re-themes via repaint
        // with no rebuild.
        let panel_node = cx.mount_child(Box::new(Label::new("")));
        let outer = cx
            .tree
            .add_child(panel_node, Box::new(Flex::column()), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            panel_node,
            Panel::new(outer)
                .with_padding(0.0)
                .with_border(Color::TRANSPARENT, 0.0)
        );

        // slot 1: header row — breadcrumbs (left) + theme toggle (top-right corner)
        let header_row = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_align_items(Alignment::Center)
                    .with_padding(EdgeInsets::new(0.0, 8.0, 0.0, 0.0)),
            ),
            None,
            1,
        );
        let mut breadcrumbs = Breadcrumbs::new(self.breadcrumb_sink.clone());
        breadcrumbs.set_crumbs(&[
            (CrumbId(0), "Halcyon Showcase", true),
            (
                CrumbId(self.current_section as u32 + 1),
                self.current_section.label(),
                true,
            ),
        ]);
        breadcrumbs.set_current_crumb(Some(CrumbId(self.current_section as u32 + 1)));
        cx.tree
            .add_child(header_row, Box::new(breadcrumbs), None, 0);

        // Flexible spacer pushes the theme toggle to the top-right corner.
        let header_spacer = cx
            .tree
            .add_child(header_row, Box::new(FlexSpacer::new()), None, 1);
        cx.tree.set_flex(header_spacer, FlexParams::flexible(1.0));

        // Theme toggle, top-right: the Switch owns its "Dark" label on the leading
        // (left) side (SPEC-HAL-208A), so clicking the label toggles. The Switch
        // writes "dark_mode"; applied to the live theme in flush_pending_actions.
        // Reflects the current theme on (re)build.
        let mut theme_switch = Switch::on(cx.app.theme.is_dark())
            .with_label("Dark")
            .with_label_side(LabelSide::Leading);
        theme_switch.set_binding("dark_mode", theme_writer);
        cx.tree
            .add_child(header_row, Box::new(theme_switch), None, 2);

        // slot 2: spacer
        cx.tree
            .add_child(outer, Box::new(FixedSpacer::vertical(2.0)), None, 2);

        // slot 3: tab bar
        let mut tab_bar = TabBar::new(self.tab_sink.clone());
        let tabs: Vec<(TabId, &str, bool)> = Section::all()
            .iter()
            .map(|s| (s.tab_id(), s.label(), true))
            .collect();
        tab_bar.set_tabs(&tabs);
        tab_bar.set_active_tab(Some(self.current_section.tab_id()));
        // Restore the scroll captured before the last rebuild so selecting a tab
        // does not snap the strip back to the start (layout re-clamps).
        tab_bar.set_scroll_x(self.tab_scroll_x);
        let tab_bar_node = cx.tree.add_child(outer, Box::new(tab_bar), None, 3);
        self.tab_bar_id = Some(tab_bar_node);

        // slot 4: divider
        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 4);

        // slot 5: spacer
        cx.tree
            .add_child(outer, Box::new(FixedSpacer::vertical(4.0)), None, 5);

        // slot 6: content area — always fills available space
        let content_node = cx.tree.add_child(outer, Box::new(Flex::column()), None, 6);
        cx.tree.set_flex(content_node, FlexParams::flexible(1.0));

        match self.current_section {
            Section::Display => self.build_display(cx, content_node),
            Section::InputsAndControls => self.build_inputs_and_controls(cx, content_node),
            Section::PropertyEditor => self.build_property_editor(cx, content_node),
            Section::TreeView => self.build_tree_view(cx, content_node),
            Section::ListView => self.build_list_view(cx, content_node),
            Section::EventLog => self.build_event_log(cx, content_node),
            Section::Console => self.build_console(cx, content_node),
            #[cfg(not(target_arch = "wasm32"))]
            Section::Plot => self.build_plot(cx, content_node),
            #[cfg(not(target_arch = "wasm32"))]
            Section::PlotQuad => self.build_plot_quad(cx, content_node),
            #[cfg(not(target_arch = "wasm32"))]
            Section::PlotDashboard => self.build_plot_dashboard(cx, content_node),
            #[cfg(target_arch = "wasm32")]
            Section::Plot | Section::PlotQuad | Section::PlotDashboard => {
                cx.tree.add_child(
                    content_node,
                    Box::new(Label::new(
                        "Plot rendering is unavailable in this browser: the current PlotRenderPool requires native worker threads.",
                    )),
                    None,
                    0,
                );
            }
            Section::OrderBook => self.build_order_book(cx, content_node),
            Section::Workbench => self.build_workbench(cx, content_node),
            Section::NodeGraph => self.build_node_graph(cx, content_node),
            Section::Overlays => self.build_overlays(cx, content_node),
            Section::Navigator => self.build_navigator(cx, content_node),
            Section::LiveData => self.build_live_data(cx, content_node),
            Section::Viewport3D => self.build_viewport_3d(cx, content_node),
        }

        // slot 7: divider
        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 7);

        // slot 8: status bar row
        let uptime = self.engine.uptime();
        let uptime_str = format!(
            "uptime: {:02}:{:02}:{:02}",
            uptime.as_secs() / 3600,
            (uptime.as_secs() % 3600) / 60,
            uptime.as_secs() % 60,
        );
        let update_rate = format!("{:.0} screen updates/s", self.updates_per_second);

        let status_row = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(10.0)
                    .with_align_items(Alignment::Center)
                    .with_padding(EdgeInsets::new(3.0, 0.0, 3.0, 0.0)),
            ),
            None,
            8,
        );
        // Bar height = text line-height + symmetric vertical padding → a compact bar
        // with vertically-centered content. (Label paints at the TOP of its box, so
        // equal top/bottom padding is what actually centers the text.)
        let status_spacer = cx
            .tree
            .add_child(status_row, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.set_flex(status_spacer, FlexParams::flexible(1.0));
        let update_rate_id =
            cx.tree
                .add_child(status_row, Box::new(Label::new(&update_rate)), None, 1);
        self.update_rate_label_id = Some(update_rate_id);
        // Fixed-width status slots: values update in place, so the cluster never
        // reflows/slides and never clips. Widths are generous for worst-case text
        // at the 13px UI font.
        cx.tree.set_flex(update_rate_id, FlexParams::fixed(136.0));
        cx.tree
            .add_child(status_row, Box::new(Divider::vertical()), None, 2);
        let uptime_id = cx
            .tree
            .add_child(status_row, Box::new(Label::new(&uptime_str)), None, 3);
        self.uptime_label_id = Some(uptime_id);
        cx.tree.set_flex(uptime_id, FlexParams::fixed(144.0));
        cx.tree
            .add_child(status_row, Box::new(Divider::vertical()), None, 4);
        let gpu_id = cx
            .tree
            .add_child(status_row, Box::new(Label::new("GPU: active")), None, 5);
        cx.tree.set_flex(gpu_id, FlexParams::fixed(96.0));
        cx.tree
            .add_child(status_row, Box::new(Divider::vertical()), None, 6);
        let proc_id = cx.tree.add_child(
            status_row,
            Box::new(LiveStat::new(&proc_status_text(&self.proc_metrics))),
            None,
            7,
        );
        self.proc_label_id = Some(proc_id);
        cx.tree.set_flex(proc_id, FlexParams::fixed(216.0));

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        if self.current_section == Section::Overlays {
            return self.handle_overlays_input(event, cx);
        }
        false
    }

    fn handle_signal(
        &mut self,
        signal: &halcyon::core::signal_bus::SignalEnvelope,
        cx: &mut ScreenContext<'_>,
    ) -> bool {
        let source_window = match signal.source {
            SignalSource::Window(id) => Some(id),
            _ => None,
        };

        match &signal.payload {
            SignalPayload::DialogConfirm { value } => {
                if let Some(id) = source_window {
                    self.overlay_child_window_id = None;
                    if let Some(value) = value {
                        self.overlay_child_value = value.clone();
                        self.overlay_status =
                            format!("Child {:?} confirmed: {}", id, self.overlay_child_value);
                    } else {
                        self.overlay_status = format!("Child {:?} confirmed", id);
                    }
                    self.needs_rebuild = true;
                    cx.invalidate();
                    return true;
                }
            }
            SignalPayload::DialogCancel => {
                if let Some(id) = source_window {
                    self.overlay_child_window_id = None;
                    self.overlay_status = format!("Child {:?} cancelled", id);
                    self.needs_rebuild = true;
                    cx.invalidate();
                    return true;
                }
            }
            SignalPayload::Custom { id, data } if *id == 0x4857_4F50 => {
                if let Some(window_id) = source_window {
                    self.overlay_child_window_id = Some(window_id);
                    let detail = String::from_utf8_lossy(data);
                    self.overlay_status = format!("Child {:?} {}", window_id, detail);
                    self.needs_rebuild = true;
                    cx.invalidate();
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    fn paint_child_window_scene(
        &self,
        window: &ChildWindowSceneState,
        cx: &mut ChildWindowPaintContext<'_>,
    ) -> bool {
        self.paint_overlay_child_window_scene(window, cx)
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        // Poll selection sinks once per frame, after all events have routed.
        // Doing this in handle_input (per-event, before dispatch) misses
        // single-event activations like `i` (TextInput only, no follow-up
        // KeyUp) that set the sink during dispatch_engine_action.
        if let Some(id) = self.tab_sink.take() {
            if let Some(section) = Section::from_tab_id(id) {
                self.current_section = section;
                self.needs_rebuild = true;
            }
        }

        // Poll breadcrumb sink — clicking "Halcyon Ops" goes to Dashboard
        if let Some(crumb_id) = self.breadcrumb_sink.take() {
            if crumb_id == CrumbId(0) {
                self.current_section = Section::Display;
                self.needs_rebuild = true;
            }
        }

        // Header theme toggle: the Switch writes "dark_mode"; apply it to the live
        // theme when it differs. set_theme marks the tree dirty (theme_changed),
        // which repaints every widget against the new theme at paint time — no
        // rebuild. Do NOT call cx.invalidate() here: it forces a full screen
        // rebuild that tears down and recreates the theme Switch on every toggle,
        // mid-press, desyncing its pointer/toggle state (flips once each way then
        // sticks).
        let current_dark = cx.app.theme.is_dark();
        let mut want_dark = current_dark;
        self.binder.bind(cx, |b| {
            b.bool("dark_mode").sync(&mut want_dark);
        });
        if want_dark != current_dark {
            let mut theme = cx.app.theme.clone();
            theme.toggle();
            cx.set_theme(theme);
        }

        if self.current_section == Section::Viewport3D {
            let mut fly = self.viewport_3d_fly;
            let mut orthographic = self.viewport_3d_orthographic;
            let mut msaa_4x = self.viewport_3d_msaa_4x;
            let mut double_sided = self.viewport_3d_double_sided;
            let mut transform_target = self.viewport_3d_transform_target;
            self.binder.bind(cx, |b| {
                b.bool("viewport_3d_fly").sync(&mut fly);
                b.bool("viewport_3d_orthographic").sync(&mut orthographic);
                b.bool("viewport_3d_msaa_4x").sync(&mut msaa_4x);
                b.bool("viewport_3d_double_sided").sync(&mut double_sided);
                b.selection("viewport_3d_transform_target")
                    .sync(&mut transform_target);
            });
            self.viewport_3d_fly = fly;
            self.viewport_3d_orthographic = orthographic;
            self.viewport_3d_msaa_4x = msaa_4x;
            self.viewport_3d_double_sided = double_sided;
            self.viewport_3d_transform_target = transform_target;
        }

        self.flush_overlay_actions(cx);
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::replace(&mut self.needs_rebuild, false)
    }

    fn update(&mut self, tree: &mut halcyon::core::widget_tree::WidgetTree) {
        self.engine.tick();
        if !self.gauge_ids.is_empty() || !self.core_ids.is_empty() || !self.sparkline_ids.is_empty()
        {
            self.update_values(tree);
        }
        if self.current_section == Section::LiveData {
            self.refresh_live_data_table(tree);
        }
        // Status-bar widgets (process CPU/MEM, FPS, uptime) live on every tab,
        // so they refresh every frame — NOT gated on gauge_ids the way
        // update_values (Display-only gauge/sparkline data) is.
        self.refresh_proc_label(tree);
        self.refresh_clock_labels(tree);

        // Persist the tab strip's horizontal scroll so it survives the rebuild a
        // tab selection triggers. Read from the live bar every frame; restored in
        // build().
        if let Some(id) = self.tab_bar_id {
            if let Some(node) = tree.node_mut(id) {
                if let Some(tb) = node.widget_mut().as_any_mut().downcast_mut::<TabBar>() {
                    self.tab_scroll_x = tb.scroll_x();
                }
            }
        }

        // Console tab streams live log output while it is the active tab.
        if self.current_section == Section::Console {
            self.console_tick(tree);
        }

        if self.current_section == Section::Overlays {
            let menu_state = if self
                .overlay_context_menu
                .as_ref()
                .is_some_and(|(handle, _)| handle.borrow().is_open())
            {
                "open"
            } else {
                "closed"
            };
            let dialog_state = self
                .overlay_dialog_handle
                .as_ref()
                .map(|handle| if handle.is_open() { "open" } else { "closing" })
                .unwrap_or("closed");
            let child_state = self
                .overlay_child_window_id
                .map(|id| format!("open {:?}", id))
                .unwrap_or_else(|| "closed".to_string());
            if let (Some(id), Some(manager)) = (
                self.overlay_status_grid_id,
                self.overlay_toast_manager.as_ref(),
            ) {
                let stats = manager.borrow().stats();
                let toast_state = if stats.active > 0 || stats.pending > 0 || stats.queue_len > 0 {
                    "active"
                } else {
                    "idle"
                };
                let updated = if let Some(node) = tree.node_mut(id) {
                    if let Some(grid) = node
                        .widget_mut()
                        .as_any_mut()
                        .downcast_mut::<halcyon::widgets::StatusGrid>()
                    {
                        grid.set_readout(
                            vec![
                                halcyon::widgets::StatusGridCell::new("MENU", menu_state),
                                halcyon::widgets::StatusGridCell::new("DIALOG", dialog_state),
                                halcyon::widgets::StatusGridCell::new("TOAST", toast_state),
                                halcyon::widgets::StatusGridCell::new("PARENT", "WindowId(0)"),
                                halcyon::widgets::StatusGridCell::new(
                                    "CHILD",
                                    child_state.as_str(),
                                ),
                                halcyon::widgets::StatusGridCell::new(
                                    "VALUE",
                                    self.overlay_child_value.as_str(),
                                ),
                            ],
                            vec![halcyon::widgets::StatusGridWideRow::new(
                                "LAST ACTION",
                                self.overlay_status.as_str(),
                            )],
                            vec![
                                halcyon::widgets::StatusGridMetric::new(
                                    "PUSHED",
                                    stats.total_pushed.to_string(),
                                ),
                                halcyon::widgets::StatusGridMetric::new(
                                    "QUEUE",
                                    stats.queue_len.to_string(),
                                ),
                                halcyon::widgets::StatusGridMetric::new(
                                    "ACTIVE",
                                    stats.active.to_string(),
                                ),
                                halcyon::widgets::StatusGridMetric::new(
                                    "VISIBLE",
                                    stats.visible.to_string(),
                                ),
                                halcyon::widgets::StatusGridMetric::new(
                                    "DISMISSED",
                                    stats.dismissed.to_string(),
                                ),
                                halcyon::widgets::StatusGridMetric::new(
                                    "FLUSHED",
                                    stats.flushed_dismissed.to_string(),
                                ),
                            ],
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if updated {
                    tree.mark_paint_dirty(id);
                }
            }
        }

        if self.current_section == Section::Viewport3D {
            self.update_viewport_3d(tree);
        }

        self.update_count += 1;
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_update_rate_sample)
            .as_secs_f64();
        if elapsed >= 0.5 {
            self.updates_per_second = self.update_count as f64 / elapsed;
            #[cfg(feature = "render-audit")]
            eprintln!(
                "[HAL_SHOWCASE_CADENCE] section={:?} updates_per_second={:.1} animation={} update={} render={:?}",
                self.current_section,
                self.updates_per_second,
                tree.any_widget_needs_animation(),
                tree.any_widget_needs_update(),
                halcyon::renderer::render_cadence_snapshot(),
            );
            self.update_count = 0;
            self.last_update_rate_sample = now;
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        menu::ensure_main_menu(cx);
    }
}

#[cfg(test)]
pub(crate) fn default_2d_section_defers_custom_render_for_test() -> bool {
    let screen = ShowcaseScreen::new();
    screen.current_section == Section::Display && screen.plot_render_pool.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_defaults_three_d_navigation_to_fly() {
        assert!(ShowcaseScreen::new().viewport_3d_fly);
    }

    #[test]
    fn showcase_selects_sdf_v3_at_default_and_explicit_densities() {
        use halcyon::core::event::UiInteractionPolicy;
        use halcyon::core::theme::DensityMode;
        use std::ffi::OsString;

        let default = showcase_app_config(std::iter::empty::<OsString>());
        let compact = showcase_app_config([OsString::from("--compact-density")]);
        let regular = showcase_app_config([OsString::from("--regular-density")]);
        let comfortable = showcase_app_config([OsString::from("--comfortable-density")]);
        let intermediate = showcase_app_config([OsString::from("--intermediate-density")]);
        let ignored = showcase_app_config([OsString::from("unrelated")]);
        let compact_then_regular = showcase_app_config([
            OsString::from("--compact-density"),
            OsString::from("--regular-density"),
        ]);
        let regular_then_compact = showcase_app_config([
            OsString::from("--regular-density"),
            OsString::from("--compact-density"),
        ]);
        assert_eq!(default.theme.density, DensityMode::Regular);
        assert_eq!(compact.theme.density, DensityMode::Compact);
        assert_eq!(regular.theme.density, DensityMode::Regular);
        assert_eq!(comfortable.theme.density, DensityMode::Comfortable);
        assert_eq!(intermediate.theme.density, DensityMode::Intermediate30);
        assert_eq!(default.theme.geometry.text_field.padding_x, 6.0);
        assert_eq!(default.theme.geometry.text_field.height, 28.0);
        assert_eq!(intermediate.theme.geometry.text_field.padding_x, 7.0);
        assert_eq!(intermediate.theme.geometry.text_field.height, 30.0);
        assert_eq!(compact.theme.geometry.text_field.padding_x, 3.0);
        assert_eq!(compact.theme.geometry.text_field.height, 24.0);
        assert_eq!(regular.theme.geometry.text_field.padding_x, 6.0);
        assert_eq!(regular.theme.geometry.text_field.height, 28.0);
        assert_eq!(comfortable.theme.geometry.text_field.padding_x, 8.0);
        assert_eq!(comfortable.theme.geometry.text_field.height, 32.0);

        for (config, density) in [
            (&default, DensityMode::Regular),
            (&compact, DensityMode::Compact),
            (&regular, DensityMode::Regular),
            (&comfortable, DensityMode::Comfortable),
            (&intermediate, DensityMode::Intermediate30),
            (&ignored, DensityMode::Regular),
            (&compact_then_regular, DensityMode::Compact),
            (&regular_then_compact, DensityMode::Compact),
        ] {
            let recipe = Theme::light_with_density(density);
            assert_eq!(config.theme.density, density);
            assert_eq!(config.theme.button_material_policy, ButtonMaterialPolicy::SdfV3);
            assert_eq!(config.title, "Halcyon \u{2014} Showcase");
            assert_eq!((config.width, config.height), (780.0, 900.0));
            assert!(!config.fullscreen);
            assert!(!config.theme.is_dark());
            assert_eq!(
                config.ui_interaction_policy,
                UiInteractionPolicy::NavigationAndCapture
            );
            assert_eq!(config.ui_scale.get(), 1.0);
            assert_eq!(config.theme.typography.font_size, 13.0);
            assert_eq!(
                config.theme.typography.font_size,
                recipe.typography.font_size
            );
            assert_eq!(
                config.theme.colors.controls.background,
                recipe.colors.controls.background
            );
            assert_eq!(
                config.theme.colors.surfaces.content,
                recipe.colors.surfaces.content
            );
            let button = &config.theme.geometry.button;
            let expected_button = &recipe.geometry.button;
            assert_eq!(
                (
                    button.height,
                    button.padding_x,
                    button.padding_y,
                    button.radius
                ),
                (
                    expected_button.height,
                    expected_button.padding_x,
                    expected_button.padding_y,
                    expected_button.radius
                ),
            );
            let field = &config.theme.geometry.text_field;
            let expected_field = &recipe.geometry.text_field;
            assert_eq!(
                (
                    field.height,
                    field.padding_x,
                    field.padding_y,
                    field.radius,
                    field.icon_size,
                    field.min_hit_size
                ),
                (
                    expected_field.height,
                    expected_field.padding_x,
                    expected_field.padding_y,
                    expected_field.radius,
                    expected_field.icon_size,
                    expected_field.min_hit_size
                ),
            );
        }

        for (flags, density) in [
            (
                vec!["--regular-density", "--comfortable-density"],
                DensityMode::Regular,
            ),
            (
                vec!["--comfortable-density", "--regular-density"],
                DensityMode::Regular,
            ),
            (
                vec!["--comfortable-density", "--intermediate-density"],
                DensityMode::Comfortable,
            ),
            (
                vec!["--intermediate-density", "--comfortable-density"],
                DensityMode::Comfortable,
            ),
            (
                vec![
                    "--compact-density",
                    "--regular-density",
                    "--comfortable-density",
                    "--intermediate-density",
                ],
                DensityMode::Compact,
            ),
            (
                vec![
                    "--intermediate-density",
                    "--comfortable-density",
                    "--regular-density",
                    "--compact-density",
                ],
                DensityMode::Compact,
            ),
        ] {
            let config = showcase_app_config(flags.into_iter().map(OsString::from));
            assert_eq!(config.theme.density, density);
            assert_eq!(config.ui_scale.get(), 1.0);
        }
    }
}

fn showcase_app_config(args: impl IntoIterator<Item = std::ffi::OsString>) -> AppConfig {
    let config = AppConfig::new("Halcyon \u{2014} Showcase").with_size(780.0, 900.0);
    let mut density = None;
    for arg in args {
        let candidate = match arg.to_str() {
            Some("--compact-density") => Some((4, halcyon::core::theme::DensityMode::Compact)),
            Some("--regular-density") => Some((3, halcyon::core::theme::DensityMode::Regular)),
            Some("--comfortable-density") => {
                Some((2, halcyon::core::theme::DensityMode::Comfortable))
            }
            Some("--intermediate-density") => {
                Some((1, halcyon::core::theme::DensityMode::Intermediate30))
            }
            _ => None,
        };
        if let Some((priority, mode)) = candidate {
            if density.is_none_or(|(current_priority, _)| priority > current_priority) {
                density = Some((priority, mode));
            }
        }
    }
    let mut theme = Theme::default();
    if let Some((_, density)) = density {
        theme.set_density(density);
    }
    theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    config.with_theme(theme)
}

fn main() {
    let app = App::new(showcase_app_config(std::env::args_os().skip(1)));
    let screen = ShowcaseScreen::new();
    #[cfg(feature = "render-audit")]
    let screen = {
        let mut screen = screen;
        if std::env::var_os("HAL_SHOWCASE_AUDIT_INPUTS").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            screen.current_section = Section::InputsAndControls;
        }
        screen
    };
    app.run(screen);
}

/// Browser entry for the same Showcase composition and default configuration.
#[cfg(target_arch = "wasm32")]
pub(super) fn run_web_showcase() {
    App::new(showcase_app_config(std::iter::empty())).run(ShowcaseScreen::new());
}
