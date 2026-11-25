// SPEC-HAL-335 §8 invariant tests: LaneLayoutEngine.

use crate::core::paint::Color;
use crate::engines::temporal::{
    IntervalDescriptor, IntervalId, IntervalLayer, IntervalStyle, LaneLayoutEngine, LanePolicy,
    MAX_LANES,
};
use crate::model::time::{TimeRange, Timestamp};

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

#[test]
fn auto_first_fit_and_half_open_sharing() {
    // Canonical order within one row: sorted by start.
    let input = vec![
        iv(1, 5, 0, 10),
        iv(2, 5, 5, 15),
        iv(3, 5, 10, 20), // shares lane 0: end 10 <= start 10 (half-open)
        iv(4, 5, 12, 18), // lanes 0 (ends 20) and 1 (ends 15) busy → lane 2
    ];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    assert_eq!(e.lanes(), &[0, 1, 0, 2]);
    assert_eq!(e.lane_count(5), 3);
}

#[test]
fn rows_pack_independently() {
    let input = vec![
        iv(1, 1, 0, 10),
        iv(2, 1, 5, 15),
        iv(3, 2, 0, 10),
        iv(4, 2, 20, 30),
    ];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    assert_eq!(e.lanes(), &[0, 1, 0, 0]);
    assert_eq!(e.lane_count(1), 2);
    assert_eq!(e.lane_count(2), 1);
    assert_eq!(e.lane_count(999), 1); // absent row resolves to 1
}

#[test]
fn coincident_milestones_get_distinct_lanes() {
    let input = vec![iv(1, 7, 50, 50), iv(2, 7, 50, 50)];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    assert_eq!(e.lanes(), &[0, 1]);
    assert_eq!(e.lane_count(7), 2);
}

#[test]
fn auto_layer_law() {
    // Canonical order: Baseline block, then Primary block.
    let mut baseline = iv(10, 3, 0, 10);
    baseline.layer = IntervalLayer::Baseline;
    let mut preview = iv(11, 3, 0, 10);
    preview.layer = IntervalLayer::Preview;
    preview.lane = Some(2); // pinned non-Primary participates in the count
    let mut pinned_primary = iv(1, 3, 0, 10);
    pinned_primary.lane = Some(5); // ignored under Auto: packing is authority
    let input = vec![baseline, pinned_primary, iv(2, 3, 5, 15), preview];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    // baseline → 0; primary id 1 → packed 0 (pin ignored); id 2 → 1; preview → 2.
    assert_eq!(e.lanes(), &[0, 0, 1, 2]);
    assert_eq!(e.lane_count(3), 3);
}

#[test]
fn explicit_policy_honors_pins_and_clamps() {
    let mut a = iv(1, 4, 0, 10);
    a.lane = Some(1);
    let mut b = iv(2, 4, 0, 10);
    b.lane = None; // => 0
    let mut over = iv(3, 4, 0, 10);
    over.lane = Some(200); // clamps to MAX_LANES - 1
    let input = vec![a, b, over];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Explicit);
    assert_eq!(e.lanes(), &[1, 0, MAX_LANES - 1]);
    assert_eq!(e.lane_count(4), MAX_LANES);
}

#[test]
fn deterministic_across_runs() {
    let input = vec![
        iv(1, 1, 0, 100),
        iv(2, 1, 10, 30),
        iv(3, 1, 20, 60),
        iv(4, 1, 30, 40),
    ];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    let first: Vec<u8> = e.lanes().to_vec();
    e.resolve(&input, LanePolicy::Auto);
    assert_eq!(e.lanes(), first.as_slice()); // INV-335-L1
}

#[test]
fn packing_overflow_collapses_into_last_lane() {
    // 70 fully overlapping primaries on one row.
    let input: Vec<IntervalDescriptor> = (0..70).map(|i| iv(i as u64 + 1, 8, 0, 1_000)).collect();
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
    assert_eq!(e.lane_count(8), MAX_LANES);
    assert!(e.lanes().iter().all(|&l| l < MAX_LANES));
    assert_eq!(e.lanes()[69], MAX_LANES - 1);
}

#[test]
#[should_panic]
fn non_canonical_order_panics_in_debug() {
    // start 20 before start 10 within one row violates the precondition.
    let input = vec![iv(1, 1, 20, 30), iv(2, 1, 10, 15)];
    let mut e = LaneLayoutEngine::new();
    e.resolve(&input, LanePolicy::Auto);
}
