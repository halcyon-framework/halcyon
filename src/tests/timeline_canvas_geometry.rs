// SPEC-HAL-336 §5 invariant tests: TimelineCanvas pure geometry
// (SPEC-PM-009-R3: hit-testing will call these same functions).

use crate::widgets::timeline_canvas::{
    BAR_MARGIN, DASH_GAP, DASH_LEN, LANE_GAP, dash_spans, interval_paint_rect, label_fits_inside,
};

#[test]
fn interval_rect_single_lane() {
    // Row at y=100, height 28; one lane.
    let r = interval_paint_rect(40.0, 120.0, 100.0, 28.0, 0, 1, BAR_MARGIN, LANE_GAP);
    assert_eq!(r.x, 40.0);
    assert_eq!(r.width, 80.0);
    assert_eq!(r.y, 100.0 + BAR_MARGIN);
    assert_eq!(r.height, 28.0 - 2.0 * BAR_MARGIN - LANE_GAP);
}

#[test]
fn interval_rect_lane_subdivision() {
    // INV-336-R2 with 3 lanes: lane_h = (28 − 4) / 3 = 8.
    let r0 = interval_paint_rect(0.0, 10.0, 0.0, 28.0, 0, 3, BAR_MARGIN, LANE_GAP);
    let r1 = interval_paint_rect(0.0, 10.0, 0.0, 28.0, 1, 3, BAR_MARGIN, LANE_GAP);
    let r2 = interval_paint_rect(0.0, 10.0, 0.0, 28.0, 2, 3, BAR_MARGIN, LANE_GAP);
    assert_eq!(r0.y, BAR_MARGIN);
    assert_eq!(r1.y, BAR_MARGIN + 8.0);
    assert_eq!(r2.y, BAR_MARGIN + 16.0);
    assert_eq!(r0.height, 8.0 - LANE_GAP);
    // INV-335-L2: lanes subdivide; the substrate never grows rows.
    assert!(r2.y + r2.height <= 28.0 - BAR_MARGIN + 0.001);
}

#[test]
fn interval_rect_minimums() {
    // Zero/negative width clamps to 1px; degenerate lane height clamps to 1px.
    let r = interval_paint_rect(50.0, 50.0, 0.0, 28.0, 0, 1, BAR_MARGIN, LANE_GAP);
    assert_eq!(r.width, 1.0);
    let tiny = interval_paint_rect(0.0, 10.0, 0.0, 6.0, 0, 4, BAR_MARGIN, LANE_GAP);
    assert_eq!(tiny.height, 1.0);
}

#[test]
fn dash_spans_cover_and_clip() {
    // len 18, dash 4, gap 3 → starts at 0,7,14; last dash clips to 4.
    let spans = dash_spans(18.0, DASH_LEN, DASH_GAP);
    assert_eq!(spans, vec![(0.0, 4.0), (7.0, 4.0), (14.0, 4.0)]);
    // len 16 → last dash clipped to 2.
    let spans = dash_spans(16.0, DASH_LEN, DASH_GAP);
    assert_eq!(spans, vec![(0.0, 4.0), (7.0, 4.0), (14.0, 2.0)]);
    assert!(dash_spans(0.0, DASH_LEN, DASH_GAP).is_empty());
}

#[test]
fn label_rule_is_deterministic() {
    // Fits: run 40 in bar 60 with pad 8 → 40 <= 44.
    assert!(label_fits_inside(40.0, 60.0, 8.0));
    // Exact boundary fits.
    assert!(label_fits_inside(44.0, 60.0, 8.0));
    // Over by epsilon → outside.
    assert!(!label_fits_inside(44.1, 60.0, 8.0));
}
