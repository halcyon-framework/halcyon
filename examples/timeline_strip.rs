// =============================================================================
// Timeline Strip Demo (SPEC-HAL-334 / 335 / 336)
// =============================================================================
//
// Free-mode TemporalSubstrate composition: TimelineHeader + TimelineCanvas
// over one shared substrate. Auto lane packing, baseline layer, one critical
// dependency arrow, milestone, markers, Select policy, intent drain logged.
//
// Visual verification:
//   [ ] Header tiers (Day | Hour) pixel-aligned with canvas gridlines
//   [ ] Off-hours shading outside 09:00–17:00
//   [ ] Release-train intervals lane-packed without visual overlap
//   [ ] Baseline ghost at 30% opacity under "Integration"
//   [ ] Critical arrow Build → Deploy: 2px, semantic error color
//   [ ] Sign-off diamond at 14:00
//   [ ] Freeze line marker (10:00, labeled) and Cut flag marker (17:00)
//   [ ] Now marker at 13:30
//   [ ] Ctrl+wheel zooms about the cursor; Shift+wheel scrolls horizontally
//   [ ] Click selects (accent outline), Ctrl+click toggles, empty click clears
//   [ ] Console logs Select / Activate / ContextMenu / VisibleRangeChanged
// =============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::WidgetTree;
use halcyon::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer, IntervalSource,
    IntervalStyle, LanePolicy, MarkerDescriptor, MarkerStyle, TemporalDescriptors,
    TemporalInvalidationEvent, TemporalSubstrate, TimeGeometryEngine, TimeScale,
};
use halcyon::model::date_value::DateValue;
use halcyon::model::time::{TimeFrame, TimeRange, Timestamp, from_civil};
use halcyon::model::time_value::TimeValue;
use halcyon::prelude::*;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::timeline_canvas::{TemporalCanvasPolicy, TimelineCanvas};
use halcyon::widgets::timeline_header::TimelineHeader;

const WINDOW_WIDTH: f32 = 1100.0;
const WINDOW_HEIGHT: f32 = 420.0;

fn ts(hour: u8, minute: u8) -> Timestamp {
    let date = DateValue::new(2026, 6, 10).expect("valid demo date");
    from_civil(
        date,
        TimeValue::new(hour, minute, None),
        TimeFrame::default(),
    )
}

fn span(h0: u8, m0: u8, h1: u8, m1: u8) -> TimeRange {
    TimeRange::new(ts(h0, m0), ts(h1, m1))
}

fn bar(
    id: u64,
    range: TimeRange,
    color: u32,
    label: &str,
    progress: f32,
    style: IntervalStyle,
    layer: IntervalLayer,
) -> IntervalDescriptor {
    IntervalDescriptor {
        id: IntervalId(id),
        row: 0, // free mode: row is ignored (SPEC-HAL-335 §7)
        range,
        color: Color::from_hex(color),
        label: if label.is_empty() {
            None
        } else {
            Some(label.to_string())
        },
        progress,
        style,
        layer,
        lane: None,
    }
}

fn build_demo_data() -> TemporalDescriptors {
    let mut data = TemporalDescriptors::default();

    data.intervals.push(bar(
        1,
        span(6, 0, 9, 30),
        0x4A7BD0,
        "Build 2412",
        1.0,
        IntervalStyle::Solid,
        IntervalLayer::Primary,
    ));
    data.intervals.push(bar(
        2,
        span(8, 30, 13, 0),
        0x3F9B6E,
        "Integration",
        0.6,
        IntervalStyle::Solid,
        IntervalLayer::Primary,
    ));
    // Planned window for Integration: baseline ghost.
    data.intervals.push(bar(
        20,
        span(9, 0, 12, 0),
        0x3F9B6E,
        "",
        0.0,
        IntervalStyle::Solid,
        IntervalLayer::Baseline,
    ));
    data.intervals.push(bar(
        3,
        span(11, 0, 16, 0),
        0xB08A3E,
        "Soak",
        0.0,
        IntervalStyle::Hatched,
        IntervalLayer::Primary,
    ));
    data.intervals.push(bar(
        4,
        span(15, 30, 18, 30),
        0xA45BC0,
        "Deploy 2412",
        0.0,
        IntervalStyle::Hollow,
        IntervalLayer::Primary,
    ));
    // Sign-off milestone (zero duration; milestones render no label).
    data.intervals.push(bar(
        5,
        TimeRange::new(ts(14, 0), ts(14, 0)),
        0xC2554F,
        "",
        0.0,
        IntervalStyle::Milestone,
        IntervalLayer::Primary,
    ));

    data.arrows.push(ArrowDescriptor {
        from: IntervalId(1),
        from_edge: IntervalEdge::End,
        to: IntervalId(4),
        to_edge: IntervalEdge::Start,
        critical: true,
        color: None,
    });

    data.markers.push(MarkerDescriptor {
        ts: ts(10, 0),
        color: Color::from_hex(0x8A8F98),
        label: Some("Freeze".to_string()),
        style: MarkerStyle::VerticalLine,
    });
    data.markers.push(MarkerDescriptor {
        ts: ts(17, 0),
        color: Color::from_hex(0x8A8F98),
        label: Some("Cut".to_string()),
        style: MarkerStyle::Flag,
    });

    data
}

/// Static demo source: stable ids, no invalidation traffic.
struct DemoSource {
    data: TemporalDescriptors,
}

impl IntervalSource for DemoSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.data.intervals.iter().cloned());
        out.bands.extend(self.data.bands.iter().copied());
        out.arrows.extend(self.data.arrows.iter().copied());
        out.markers.extend(self.data.markers.iter().cloned());
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        Vec::new()
    }

    fn interval_exists(&self, id: IntervalId) -> bool {
        self.data.intervals.iter().any(|d| d.id == id)
    }
}

struct TimelineStripScreen {
    placeholder: Label,
    built: bool,
    substrate: Option<Rc<RefCell<TemporalSubstrate>>>,
    source: DemoSource,
}

impl TimelineStripScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
            substrate: None,
            source: DemoSource {
                data: build_demo_data(),
            },
        }
    }
}

impl Screen for TimelineStripScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // Engine: Hours scale, 64 px/h, anchored 05:00; fixed demo `now`.
        let mut engine = TimeGeometryEngine::new(ts(5, 0), TimeScale::Hours, 64.0, WINDOW_WIDTH);
        engine.set_now(Some(ts(13, 30)));

        let substrate = Rc::new(RefCell::new(TemporalSubstrate::new(
            engine,
            SelectionMode::Multi,
            LanePolicy::Auto,
        )));
        self.substrate = Some(substrate.clone());

        let header = TimelineHeader::new(substrate.clone());
        let canvas =
            TimelineCanvas::new(substrate.clone()).with_policy(TemporalCanvasPolicy::Select);

        let tree_root = cx.tree.root().expect("Tree must have root");
        let root = cx
            .tree
            .add_child(tree_root, Box::new(Flex::column()), None, 0);

        let title = Label::new("Release Train — 2026-06-10 — TemporalSubstrate free-mode demo");
        cx.tree.add_child(root, Box::new(title), None, 0);

        cx.tree.add_child(root, Box::new(header), None, 0);

        let canvas_id = cx.tree.add_child(root, Box::new(canvas), None, 0);
        cx.tree.set_flex(canvas_id, FlexParams::flexible(1.0));

        &mut self.placeholder
    }

    /// Per-frame drive (SPEC-HAL-335 §10: applications drain; widgets only
    /// enqueue): collection gating, resolution refresh, intent drain.
    /// `Screen::update` runs every frame regardless of the rebuild flag —
    /// `build` runs once and stays construction-only.
    fn update(&mut self, _tree: &mut WidgetTree) {
        if let Some(substrate) = &self.substrate {
            let mut s = substrate.borrow_mut();
            s.begin_frame(&mut self.source, None);
            for intent in s.drain_intents() {
                println!("[intent] {:?}", intent);
            }
        }
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== Timeline Strip Demo (SPEC-HAL-334/335/336) ===");
        println!();
        println!("Interaction:");
        println!("  Ctrl+wheel  zoom about the cursor");
        println!("  Shift+wheel scroll horizontally");
        println!("  Click       select (accent outline); Cmd/Ctrl+click toggles");
        println!("  Click empty clear selection");
        println!("  Right-click ContextMenu intent (logged)");
        println!("  Double-click Activate intent (logged)");
        println!();
        println!("Close window to exit.");
        println!("==================================================");
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
        AppConfig::new("Timeline Strip Demo (SPEC-HAL-336)").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(TimelineStripScreen::new());
}
