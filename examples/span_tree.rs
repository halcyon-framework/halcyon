// =============================================================================
// SpanTree Synthetic Trace — drill-in (SPEC-HAL-341)
// =============================================================================
//
// A synthetic CI pipeline trace (a ContainmentSource) feeds the unchanged
// SPEC-HAL-335 temporal pipeline through RootedContainmentSource; TimelineCanvas
// (Select policy) + TimelineHeader render one containment level read-only.
//
//   children_of(root, visible) -> the level's spans (SPEC-HAL-341 §5)
//   double-click a span -> set_root onto it + fit the frame (§6 enter)
//   zoom out past the root span -> pop to the parent + fit (§6 pop)
//
// Only the current level is ever collected and painted (INV-340-C1 / INV-341-V2)
// and the canvas row is sized to the resolved lane count, so bars hold a fixed
// height at every level. The trace holds 22 runs, each 147 nested spans
// (stages, parallel crate builds, test shards, deploy regions, leaf steps) —
// 3234 spans total.
//
// Run: cargo run --example span_tree
//   Double-click   enter a span
//   Wheel          zoom about cursor (zoom out past a span pops up a level)
//   Shift+wheel    pan the time axis
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::engines::temporal::{
    ContainmentPolicy, ContainmentSource, IntervalDescriptor, IntervalId, IntervalLayer,
    IntervalStyle, LanePolicy, RootedContainmentSource, RowGeometry, TemporalDescriptors,
    TemporalIntent, TemporalInvalidationEvent, TemporalSubstrate, TimeGeometryEngine, TimeScale,
};
use halcyon::model::date_value::DateValue;
use halcyon::model::time::{TimeFrame, TimeRange, Timestamp, from_civil};
use halcyon::model::time_value::TimeValue;
use halcyon::prelude::*;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::timeline_canvas::{TemporalCanvasPolicy, TimelineCanvas};
use halcyon::widgets::timeline_header::TimelineHeader;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 640.0;

// Trace fan-out — tuned so the tree runs to 3234 spans (22 * 147) and the top
// level packs ~8 lanes.
const NUM_RUNS: usize = 22;
const NUM_CRATES: usize = 14;
const NUM_SHARDS: usize = 10;
const TESTS_PER_SHARD: usize = 5;
const NUM_REGIONS: usize = 4;
const RUN_DUR_MS: i64 = 48 * 60 * 1000;
const RUN_STAGGER_MS: i64 = 6 * 60 * 1000;

// Fixed lane height. The canvas splits a row's height across its lanes; feeding
// a row of height lane_count * LANE_PX yields a constant bar height at every
// level instead of free mode's canvas_height / lane_count squares.
const LANE_PX: f32 = 40.0;

// Top-level runs cycle a palette so the level reads as distinct spans rather
// than a monochrome wall.
const RUN_PALETTE: [u32; 6] = [0x5E9CFF, 0x3DCC8E, 0xE8A33D, 0xC678F0, 0xE05A50, 0x4FB8C7];

// Sublevel palette (a data source has no theme access; the canvas resolves
// chrome from the theme, bar fill from the data).
const C_CHECKOUT: u32 = 0x8FA6C4;
const C_BUILD: u32 = 0x3DCC8E;
const C_TEST: u32 = 0xE8A33D;
const C_PACKAGE: u32 = 0x8FA6C4;
const C_DEPLOY: u32 = 0xC678F0;
const C_SMOKE: u32 = 0xE05A50;
const C_CRATE: u32 = 0x49B07A;
const C_SHARD: u32 = 0xCE9136;
const C_REGION: u32 = 0xA862D6;
const C_STEP: u32 = 0x6F7E92;

const CRATES: [&str; 14] = [
    "core",
    "render",
    "layout",
    "text",
    "engine",
    "widgets",
    "model",
    "platform",
    "runtime",
    "compositor",
    "agent",
    "plot",
    "oracles",
    "semantics",
];
const REGIONS: [&str; 4] = ["us-east", "us-west", "eu-central", "ap-south"];

// =============================================================================
// Synthetic trace data — a ContainmentSource (SPEC-HAL-340)
// =============================================================================

struct Span {
    id: IntervalId,
    parent: Option<IntervalId>,
    range: TimeRange,
    label: String,
    color: u32,
    style: IntervalStyle,
}

impl Span {
    fn descriptor(&self) -> IntervalDescriptor {
        IntervalDescriptor {
            id: self.id,
            row: 0,
            range: self.range,
            color: Color::from_hex(self.color),
            label: Some(self.label.clone()),
            progress: 0.0,
            style: self.style,
            layer: IntervalLayer::Primary,
            lane: None,
        }
    }
}

/// The whole synthetic forest. Enforced containment: every child's range lies
/// within its parent's (constructed that way below). children_of is a linear
/// scan — a real source would index by parent; for a 3234-span demo it is free.
struct TraceData {
    spans: Vec<Span>,
}

impl ContainmentSource for TraceData {
    fn policy(&self) -> ContainmentPolicy {
        ContainmentPolicy::Enforced
    }

    fn children_of(
        &self,
        parent: Option<IntervalId>,
        _range: TimeRange,
        out: &mut TemporalDescriptors,
    ) {
        // Direct children of `parent` only (INV-340-C1). Over-returning within
        // the level is permitted; the projection culls to range (INV-335-D2).
        for s in &self.spans {
            if s.parent == parent {
                out.intervals.push(s.descriptor());
            }
        }
    }

    fn parent_of(&self, id: IntervalId) -> Option<IntervalId> {
        self.spans
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.parent)
    }

    fn has_children(&self, id: IntervalId) -> bool {
        self.spans.iter().any(|s| s.parent == Some(id))
    }

    fn interval(&self, id: IntervalId) -> Option<IntervalDescriptor> {
        self.spans
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.descriptor())
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        Vec::new()
    }
}

// =============================================================================
// Deterministic generator — index-driven, no RNG; every child clamped within
// its parent so INV-340-C2 (enforced containment) holds by construction.
// =============================================================================

struct Builder {
    spans: Vec<Span>,
    next: u64,
}

impl Builder {
    fn add(
        &mut self,
        parent: Option<IntervalId>,
        start: Timestamp,
        end: Timestamp,
        label: String,
        color: u32,
        style: IntervalStyle,
    ) -> IntervalId {
        let end = if end.0 < start.0 { start } else { end };
        let id = IntervalId(self.next);
        self.next += 1;
        self.spans.push(Span {
            id,
            parent,
            range: TimeRange::new(start, end),
            label,
            color,
            style,
        });
        id
    }
}

fn plus(t: Timestamp, ms: i64) -> Timestamp {
    Timestamp(t.0 + ms * 1_000)
}

fn clamp_end(end: Timestamp, max: Timestamp) -> Timestamp {
    if end.0 > max.0 { max } else { end }
}

fn span_ms(start: Timestamp, end: Timestamp) -> i64 {
    (end.0 - start.0) / 1_000
}

fn range_us(r: TimeRange) -> i64 {
    r.end.0 - r.start.0
}

fn trace_base() -> Timestamp {
    from_civil(
        DateValue::new(2026, 6, 13).expect("valid trace date"),
        TimeValue::new(2, 0, None),
        TimeFrame::default(),
    )
}

/// The extent of the top level (all runs), used to fit the frame when the
/// ViewRoot is the synthetic root (popped back to the top).
fn full_trace_range() -> TimeRange {
    let last_end = (NUM_RUNS as i64 - 1) * RUN_STAGGER_MS + RUN_DUR_MS;
    TimeRange::new(trace_base(), plus(trace_base(), last_end))
}

enum Stage {
    Steps(&'static [&'static str]),
    Build,
    Test,
    Deploy,
}

/// Sequential named leaves partitioning [start, start+dur_ms].
fn gen_seq(
    b: &mut Builder,
    parent: IntervalId,
    start: Timestamp,
    dur_ms: i64,
    names: &[&str],
    color: u32,
) {
    let n = names.len().max(1) as i64;
    for (i, name) in names.iter().enumerate() {
        let ss = plus(start, i as i64 * dur_ms / n);
        let se = plus(start, (i as i64 + 1) * dur_ms / n);
        b.add(
            Some(parent),
            ss,
            se,
            name.to_string(),
            color,
            IntervalStyle::Solid,
        );
    }
}

/// `count` sequential numbered leaves partitioning [start, start+dur_ms].
fn gen_n_leaves(
    b: &mut Builder,
    parent: IntervalId,
    start: Timestamp,
    dur_ms: i64,
    count: usize,
    prefix: &str,
    color: u32,
) {
    let n = count.max(1) as i64;
    for i in 0..count {
        let ss = plus(start, i as i64 * dur_ms / n);
        let se = plus(start, (i as i64 + 1) * dur_ms / n);
        b.add(
            Some(parent),
            ss,
            se,
            format!("{} {}", prefix, i + 1),
            color,
            IntervalStyle::Solid,
        );
    }
}

fn gen_build(b: &mut Builder, stage: IntervalId, ss: Timestamp, dur_ms: i64) {
    let se = plus(ss, dur_ms);
    for c in 0..NUM_CRATES {
        let cs = plus(ss, c as i64 * dur_ms / (NUM_CRATES as i64 * 2));
        let cdur = dur_ms * (45 + (c as i64 * 31) % 50) / 100;
        let ce = clamp_end(plus(cs, cdur), se);
        let krate = b.add(
            Some(stage),
            cs,
            ce,
            format!("crate {}", CRATES[c % CRATES.len()]),
            C_CRATE,
            IntervalStyle::Solid,
        );
        gen_seq(
            b,
            krate,
            cs,
            span_ms(cs, ce),
            &["parse", "typecheck", "codegen"],
            C_STEP,
        );
    }
}

fn gen_test(b: &mut Builder, stage: IntervalId, ss: Timestamp, dur_ms: i64) {
    let se = plus(ss, dur_ms);
    for s in 0..NUM_SHARDS {
        let sh_s = plus(ss, s as i64 * dur_ms / (NUM_SHARDS as i64 * 2));
        let sh_dur = dur_ms * (55 + (s as i64 * 23) % 40) / 100;
        let sh_e = clamp_end(plus(sh_s, sh_dur), se);
        let shard = b.add(
            Some(stage),
            sh_s,
            sh_e,
            format!("shard {}", s + 1),
            C_SHARD,
            IntervalStyle::Solid,
        );
        gen_n_leaves(
            b,
            shard,
            sh_s,
            span_ms(sh_s, sh_e),
            TESTS_PER_SHARD,
            "test",
            C_STEP,
        );
    }
}

fn gen_deploy(b: &mut Builder, stage: IntervalId, ss: Timestamp, dur_ms: i64) {
    let se = plus(ss, dur_ms);
    for r in 0..NUM_REGIONS {
        let rs = plus(ss, r as i64 * dur_ms / (NUM_REGIONS as i64 * 2));
        let rdur = dur_ms * (60 + (r as i64 * 17) % 35) / 100;
        let re = clamp_end(plus(rs, rdur), se);
        let region = b.add(
            Some(stage),
            rs,
            re,
            format!("region {}", REGIONS[r % REGIONS.len()]),
            C_REGION,
            IntervalStyle::Solid,
        );
        gen_seq(
            b,
            region,
            rs,
            span_ms(rs, re),
            &["push", "migrate", "healthcheck"],
            C_STEP,
        );
    }
}

fn gen_run(b: &mut Builder, run: IntervalId, rs: Timestamp, dur_ms: i64) {
    let stages: [(&str, i64, u32, Stage); 6] = [
        (
            "checkout",
            5,
            C_CHECKOUT,
            Stage::Steps(&["fetch", "submodules", "lfs"]),
        ),
        ("build", 35, C_BUILD, Stage::Build),
        ("test", 40, C_TEST, Stage::Test),
        (
            "package",
            8,
            C_PACKAGE,
            Stage::Steps(&["bundle", "sign", "upload"]),
        ),
        ("deploy", 10, C_DEPLOY, Stage::Deploy),
        ("smoke", 2, C_SMOKE, Stage::Steps(&["health", "probe"])),
    ];
    let total: i64 = stages.iter().map(|s| s.1).sum();
    let mut cursor = 0i64;
    for (label, weight, color, kind) in stages {
        let seg = dur_ms * weight / total;
        let ss = plus(rs, cursor);
        let se = plus(rs, cursor + seg);
        let stage = b.add(
            Some(run),
            ss,
            se,
            label.to_string(),
            color,
            IntervalStyle::Solid,
        );
        match kind {
            Stage::Steps(names) => gen_seq(b, stage, ss, seg, names, C_STEP),
            Stage::Build => gen_build(b, stage, ss, seg),
            Stage::Test => gen_test(b, stage, ss, seg),
            Stage::Deploy => gen_deploy(b, stage, ss, seg),
        }
        cursor += seg;
    }
}

fn generate() -> TraceData {
    let base = trace_base();
    let mut b = Builder {
        spans: Vec::new(),
        next: 0,
    };
    for r in 0..NUM_RUNS {
        let rs = plus(base, r as i64 * RUN_STAGGER_MS);
        let re = plus(rs, RUN_DUR_MS);
        let color = RUN_PALETTE[r % RUN_PALETTE.len()];
        let run = b.add(
            None,
            rs,
            re,
            format!("pipeline #{}", 4820 + r),
            color,
            IntervalStyle::Solid,
        );
        gen_run(&mut b, run, rs, RUN_DUR_MS);
    }
    TraceData { spans: b.spans }
}

// =============================================================================
// Screen
// =============================================================================

struct SpanTreeScreen {
    placeholder: Label,
    built: bool,
    substrate: Option<Rc<RefCell<TemporalSubstrate>>>,
    canvas_id: Option<NodeId>,
    source: RootedContainmentSource,
    full_range: TimeRange,
}

impl SpanTreeScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            substrate: None,
            canvas_id: None,
            source: RootedContainmentSource::new(Box::new(generate())),
            full_range: full_trace_range(),
        }
    }
}

impl Screen for SpanTreeScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        let col = cx.mount_child(Box::new(Flex::column()));
        let tree = &mut cx.tree;

        // Frame fits the ~2.9 h run window; Hours scale keeps px/unit in the
        // engine's valid band (INV-335-G3 governs zoom from here).
        let engine = TimeGeometryEngine::new(trace_base(), TimeScale::Hours, 440.0, WINDOW_WIDTH);
        let substrate = Rc::new(RefCell::new(TemporalSubstrate::new(
            engine,
            SelectionMode::Multi,
            LanePolicy::Auto,
        )));
        self.substrate = Some(substrate.clone());

        let header = TimelineHeader::new(substrate.clone());
        // Select policy: read-only selection + the Activate intent (double-click)
        // the drill-in consumes. No Edit-tier drag.
        let canvas =
            TimelineCanvas::new(substrate.clone()).with_policy(TemporalCanvasPolicy::Select);

        tree.add_child(col, Box::new(header), None, 0);
        let canvas_id = tree.add_child(col, Box::new(canvas), None, 1);
        tree.set_flex(canvas_id, FlexParams::flexible(1.0));
        self.canvas_id = Some(canvas_id);

        &mut self.placeholder
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let (Some(canvas_id), Some(substrate)) = (self.canvas_id, self.substrate.as_ref()) else {
            return;
        };

        // 1. Enter on Activate (double-click a span). Re-root the source onto it
        //    and fit the frame to its range — SPEC-HAL-341 §6 enter, INV-341-V1.
        //    Done before collecting so this frame paints the entered level.
        let activated: Vec<IntervalId> = {
            let mut s = substrate.borrow_mut();
            s.drain_intents()
                .into_iter()
                .filter_map(|i| match i {
                    TemporalIntent::Activate { id } => Some(id),
                    _ => None,
                })
                .collect()
        };
        for id in activated {
            if let Some(d) = self.source.containment().interval(id) {
                self.source.set_root(Some(id));
                substrate.borrow_mut().geometry_mut().fit_range(d.range);
            }
        }

        // 2. Auto-pop on zoom-out past the current root's span by > 1.6x
        //    (INV-341-V3). Fitting to the parent on pop settles the view so it
        //    does not immediately re-pop. The top (root None) has no parent.
        if let Some(root_id) = self.source.root() {
            let root_range = self.source.containment().interval(root_id).map(|d| d.range);
            let visible = substrate.borrow().geometry().visible_range();
            if let Some(rr) = root_range {
                if range_us(visible) * 10 > range_us(rr) * 16 {
                    let parent = self.source.containment().parent_of(root_id);
                    self.source.set_root(parent);
                    let fit = match parent {
                        Some(p) => self.source.containment().interval(p).map(|d| d.range),
                        None => Some(self.full_range),
                    };
                    if let Some(f) = fit {
                        substrate.borrow_mut().geometry_mut().fit_range(f);
                    }
                }
            }
        }

        // 3. Collect this frame for the resolved root (free mode: rows = None).
        let lane_count = {
            let mut s = substrate.borrow_mut();
            s.begin_frame(&mut self.source, None);
            s.lanes().lane_count(0)
        };

        // 4. Size the canvas row to the lane count so bars hold a fixed height
        //    (the SPEC-HAL-335 §7 seam, fed by the projection).
        let row_height = lane_count.max(1) as f32 * LANE_PX + 4.0;
        if let Some(node) = tree.node_mut(canvas_id) {
            if let Some(canvas) = node
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<TimelineCanvas>()
            {
                canvas.set_row_geometry(vec![RowGeometry {
                    id: 0,
                    y: 0.0,
                    height: row_height,
                }]);
            }
        }
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== SpanTree Synthetic Trace (SPEC-HAL-341) ===");
        println!();
        println!(
            "{} pipeline runs at the top level, lane-packed. Each run nests",
            NUM_RUNS
        );
        println!("stages, parallel crate builds, test shards, and deploy regions —");
        println!("more than 3,000 nested spans in total.");
        println!();
        println!("  Double-click   enter a span");
        println!("  Wheel          zoom (zoom out past a span pops up a level)");
        println!("  Shift+wheel    pan the time axis");
        println!();
        println!("Close window to exit.");
        println!("===============================================");
    }

    fn mark_rebuild(&mut self) {}

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("SpanTree Synthetic Trace (SPEC-HAL-341)")
            .with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(SpanTreeScreen::new());
}
