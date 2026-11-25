// SPEC-HAL-336 §6 invariant tests: identity hit-testing (PM-009-R3 parity
// with paint via the shared interval_paint_rect) + Primary-window selection.

use crate::core::ordered_selection::{OrderedSelectionAction, SelectionMode};
use crate::core::paint::Color;
use crate::engines::temporal::{
    IntervalDescriptor, IntervalId, IntervalLayer, IntervalProjection, IntervalSource,
    IntervalStyle, LaneLayoutEngine, LanePolicy, TemporalDescriptors, TemporalInvalidationEvent,
    TemporalSubstrate, TimeGeometryEngine, TimeScale,
};
use crate::model::time::{TimeRange, Timestamp};
use crate::widgets::timeline_canvas::{BAR_MARGIN, HitZone, RESIZE_GRIP, hit_test_primary};

const S: i64 = 1_000_000;

fn c() -> Color {
    Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    }
}

fn iv(id: u64, row: u64, s: i64, e: i64) -> IntervalDescriptor {
    IntervalDescriptor {
        id: IntervalId(id),
        row,
        range: TimeRange::new(Timestamp(s), Timestamp(e)),
        color: c(),
        label: None,
        progress: 0.0,
        style: IntervalStyle::Solid,
        layer: IntervalLayer::Primary,
        lane: None,
    }
}

struct Src(TemporalDescriptors);

impl IntervalSource for Src {
    fn collect(&self, _r: Option<&[u64]>, _g: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.0.intervals.iter().cloned());
    }
    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        Vec::new()
    }
    fn interval_exists(&self, id: IntervalId) -> bool {
        self.0.intervals.iter().any(|d| d.id == id)
    }
}

/// 16 px/s, viewport 160 px → visible [0 s, 10 s). Row 7 at y=0, height 28.
fn fixture(
    data: TemporalDescriptors,
) -> (IntervalProjection, LaneLayoutEngine, TimeGeometryEngine) {
    let geometry = TimeGeometryEngine::new(Timestamp(0), TimeScale::Seconds, 16.0, 160.0);
    let mut projection = IntervalProjection::new();
    let src = Src(data);
    projection.collect_frame(&src, None, geometry.visible_range());
    let mut lanes = LaneLayoutEngine::new();
    lanes.resolve(&projection.snapshot().intervals, LanePolicy::Auto);
    (projection, lanes, geometry)
}

fn row(_: u64) -> Option<(f32, f32)> {
    Some((0.0, 28.0))
}

#[test]
fn hit_body_returns_identity_and_instant() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 7, 2 * S, 5 * S)); // x: 32 .. 80
    let (p, lanes, g) = fixture(data);
    let (hit, ts) = hit_test_primary(p.snapshot(), &lanes, &g, row, (40.0, 14.0), false);
    assert_eq!(hit, Some((IntervalId(1), HitZone::Body)));
    assert_eq!(ts, Timestamp((40.0 / 16.0 * S as f32) as i64));
}

#[test]
fn empty_space_returns_none_with_instant() {
    let (p, lanes, g) = fixture(TemporalDescriptors::default());
    let (hit, ts) = hit_test_primary(p.snapshot(), &lanes, &g, row, (80.0, 10.0), false);
    assert_eq!(hit, None);
    assert_eq!(ts, Timestamp(5 * S));
}

#[test]
fn lane_gap_between_stacked_bars_misses() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 7, 1 * S, 6 * S)); // lane 0
    data.intervals.push(iv(2, 7, 2 * S, 7 * S)); // lane 1
    let (p, lanes, g) = fixture(data);
    // lane_h = (28 − 4) / 2 = 12; lane 0 occupies y 2..13 (gap at 13..14).
    let (top, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, (40.0, 6.0), false);
    assert_eq!(top, Some((IntervalId(1), HitZone::Body)));
    let (gap, _) = hit_test_primary(
        p.snapshot(),
        &lanes,
        &g,
        row,
        (40.0, BAR_MARGIN + 12.0 - 0.5),
        false,
    );
    assert_eq!(gap, None);
    let (bottom, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, (40.0, 20.0), false);
    assert_eq!(bottom, Some((IntervalId(2), HitZone::Body)));
}

#[test]
fn resize_grip_only_in_edit() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 7, 2 * S, 5 * S)); // right edge at x = 80
    let (p, lanes, g) = fixture(data);
    let pos = (80.0 - RESIZE_GRIP / 2.0, 14.0);
    let (no_grip, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, pos, false);
    assert_eq!(no_grip, Some((IntervalId(1), HitZone::Body)));
    let (grip, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, pos, true);
    assert_eq!(grip, Some((IntervalId(1), HitZone::ResizeEnd)));
}

#[test]
fn milestone_hits_as_diamond_square_never_grip() {
    let mut data = TemporalDescriptors::default();
    let mut m = iv(1, 7, 4 * S, 4 * S); // x = 64
    m.style = IntervalStyle::Milestone;
    data.intervals.push(m);
    let (p, lanes, g) = fixture(data);
    // bar height = 23 → half = 11.5, center y = 2 + 11.5 = 13.5.
    let (center, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, (64.0, 13.5), true);
    assert_eq!(center, Some((IntervalId(1), HitZone::Body)));
    let (outside, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, (76.5, 13.5), true);
    assert_eq!(outside, None);
}

#[test]
fn topmost_primary_wins_and_baseline_is_inert() {
    let mut data = TemporalDescriptors::default();
    let mut base = iv(10, 7, 2 * S, 5 * S);
    base.layer = IntervalLayer::Baseline;
    base.lane = Some(0);
    data.intervals.push(base);
    data.intervals.push(iv(1, 7, 2 * S, 5 * S));
    data.intervals.push(iv(2, 7, 2 * S, 5 * S)); // identical extent, later id
    let (p, lanes, g) = fixture(data);
    // Two primaries pack into lanes 0/1; point in lane 0 hits id 1, never 10.
    let (hit, _) = hit_test_primary(p.snapshot(), &lanes, &g, row, (40.0, 6.0), false);
    assert_eq!(hit, Some((IntervalId(1), HitZone::Body)));
}

#[test]
fn substrate_selection_window_excludes_non_primary() {
    let mut data = TemporalDescriptors::default();
    let mut base = iv(10, 7, 1 * S, 2 * S);
    base.layer = IntervalLayer::Baseline;
    data.intervals.push(base);
    data.intervals.push(iv(1, 7, 1 * S, 2 * S));
    data.intervals.push(iv(2, 7, 3 * S, 4 * S));
    data.intervals.push(iv(3, 7, 5 * S, 6 * S));
    let mut src = Src(data);
    let geometry = TimeGeometryEngine::new(Timestamp(0), TimeScale::Seconds, 16.0, 160.0);
    let mut sub = TemporalSubstrate::new(geometry, SelectionMode::Multi, LanePolicy::Auto);
    sub.begin_frame(&mut src, None);

    sub.reduce_selection(OrderedSelectionAction::Select { key: IntervalId(1) });
    sub.reduce_selection(OrderedSelectionAction::ExtendTo { key: IntervalId(3) });
    assert!(sub.selection().is_selected(IntervalId(1)));
    assert!(sub.selection().is_selected(IntervalId(2)));
    assert!(sub.selection().is_selected(IntervalId(3)));
    // The range sweep never touched the Baseline id.
    assert!(!sub.selection().is_selected(IntervalId(10)));
    assert_eq!(sub.selection().selection_count(), 3);
}
