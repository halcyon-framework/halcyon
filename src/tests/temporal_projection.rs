// SPEC-HAL-335 §5–§6 invariant tests: descriptors, IntervalSource, IntervalProjection.

use crate::core::paint::Color;
use crate::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer,
    IntervalProjection, IntervalSource, IntervalStyle, MarkerDescriptor, MarkerStyle,
    RowBandDescriptor, TemporalDescriptors, TemporalInvalidationEvent,
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

fn arrow(from: u64, to: u64) -> ArrowDescriptor {
    ArrowDescriptor {
        from: IntervalId(from),
        from_edge: IntervalEdge::End,
        to: IntervalId(to),
        to_edge: IntervalEdge::Start,
        critical: false,
        color: None,
    }
}

fn marker(ts: i64) -> MarkerDescriptor {
    MarkerDescriptor {
        ts: Timestamp(ts),
        color: c(),
        label: None,
        style: MarkerStyle::VerticalLine,
    }
}

fn band(row: u64, s: i64, e: i64) -> RowBandDescriptor {
    RowBandDescriptor {
        row,
        range: TimeRange::new(Timestamp(s), Timestamp(e)),
        color: c(),
    }
}

/// Deliberately over-returns: ignores rows and range entirely.
/// The projection must cull precisely (INV-335-D2).
struct VecSource {
    data: TemporalDescriptors,
    events: Vec<TemporalInvalidationEvent>,
}

impl IntervalSource for VecSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.data.intervals.iter().cloned());
        out.bands.extend(self.data.bands.iter().copied());
        out.arrows.extend(self.data.arrows.iter().copied());
        out.markers.extend(self.data.markers.iter().cloned());
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        std::mem::take(&mut self.events)
    }

    fn interval_exists(&self, id: IntervalId) -> bool {
        self.data.intervals.iter().any(|d| d.id == id)
    }
}

fn source(data: TemporalDescriptors) -> VecSource {
    VecSource {
        data,
        events: Vec::new(),
    }
}

fn window() -> TimeRange {
    TimeRange::new(Timestamp(100), Timestamp(200))
}

#[test]
fn precise_cull_half_open() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 10, 90)); // out: ends before window
    data.intervals.push(iv(2, 0, 90, 150)); // in: overlaps left edge
    data.intervals.push(iv(3, 0, 150, 250)); // in: overlaps right edge
    data.intervals.push(iv(4, 0, 200, 300)); // out: starts at half-open end
    data.intervals.push(iv(5, 0, 250, 300)); // out: beyond window
    let s = source(data);
    let mut p = IntervalProjection::new();
    let snap = p.collect_frame(&s, None, window());
    let ids: Vec<u64> = snap.intervals.iter().map(|d| d.id.0).collect();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn milestone_point_rule() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 100, 100)); // on the left edge: visible
    data.intervals.push(iv(2, 0, 150, 150)); // inside: visible
    data.intervals.push(iv(3, 0, 200, 200)); // on the half-open end: culled
    data.intervals.push(iv(4, 0, 90, 90)); // before: culled
    let s = source(data);
    let mut p = IntervalProjection::new();
    let snap = p.collect_frame(&s, None, window());
    let ids: Vec<u64> = snap.intervals.iter().map(|d| d.id.0).collect();
    assert_eq!(ids, vec![1, 2]);
}

#[test]
fn row_filter_bound_and_free() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 10, 120, 180));
    data.intervals.push(iv(2, 11, 120, 180));
    data.intervals.push(iv(3, 12, 120, 180));
    let s = source(data);
    let mut p = IntervalProjection::new();

    let snap = p.collect_frame(&s, Some(&[10, 12]), window());
    let ids: Vec<u64> = snap.intervals.iter().map(|d| d.id.0).collect();
    assert_eq!(ids, vec![1, 3]); // bound: row 11 filtered

    let snap = p.collect_frame(&s, None, window());
    assert_eq!(snap.intervals.len(), 3); // free: no row filter
}

#[test]
fn arrows_drop_hidden_endpoints() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 10, 110, 150));
    data.intervals.push(iv(2, 10, 160, 190));
    data.intervals.push(iv(3, 10, 300, 400)); // culled by range
    data.intervals.push(iv(4, 11, 110, 150)); // culled by row filter below
    data.arrows.push(arrow(1, 2)); // kept
    data.arrows.push(arrow(1, 3)); // dropped: endpoint range-culled
    data.arrows.push(arrow(4, 2)); // dropped: endpoint row-culled
    let s = source(data);
    let mut p = IntervalProjection::new();
    let snap = p.collect_frame(&s, Some(&[10]), window());
    assert_eq!(snap.arrows.len(), 1);
    assert_eq!(
        (snap.arrows[0].from, snap.arrows[0].to),
        (IntervalId(1), IntervalId(2))
    );
}

#[test]
fn markers_and_bands_culled() {
    let mut data = TemporalDescriptors::default();
    data.markers.push(marker(100)); // in (half-open start)
    data.markers.push(marker(199)); // in
    data.markers.push(marker(200)); // out (half-open end)
    data.bands.push(band(10, 0, 100)); // out: touches at start only
    data.bands.push(band(10, 150, 250)); // in
    data.bands.push(band(11, 150, 250)); // out: row filtered
    let s = source(data);
    let mut p = IntervalProjection::new();
    let snap = p.collect_frame(&s, Some(&[10]), window());
    assert_eq!(snap.markers.len(), 2);
    assert_eq!(snap.bands.len(), 1);
    assert_eq!(snap.bands[0].row, 10);
}

#[test]
fn canonical_ordering() {
    let mut data = TemporalDescriptors::default();
    let mut a = iv(7, 2, 150, 180);
    a.layer = IntervalLayer::Primary;
    let mut b = iv(3, 1, 120, 140);
    b.layer = IntervalLayer::Primary;
    let mut c1 = iv(9, 1, 110, 130);
    c1.layer = IntervalLayer::Baseline;
    let mut d = iv(1, 1, 120, 160);
    d.layer = IntervalLayer::Primary;
    // Shuffled insertion order.
    data.intervals.extend([a, b, c1, d]);
    let s = source(data);
    let mut p = IntervalProjection::new();
    let snap = p.collect_frame(&s, None, window());
    let order: Vec<u64> = snap.intervals.iter().map(|x| x.id.0).collect();
    // Baseline first; then Primary by (row, start, id): id 1 (row1,120) before
    // id 3 (row1,120, larger id), then id 7 (row2).
    assert_eq!(order, vec![9, 1, 3, 7]);
}

#[test]
#[should_panic]
fn duplicate_id_panics_in_debug() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 110, 150));
    data.intervals.push(iv(1, 0, 160, 190)); // INV-335-ID1 violation
    let s = source(data);
    let mut p = IntervalProjection::new();
    let _ = p.collect_frame(&s, None, window());
}

#[test]
fn identity_lookup_and_buffer_reuse() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 110, 150));
    data.intervals.push(iv(2, 0, 160, 190));
    let s = source(data);
    let mut p = IntervalProjection::new();

    let snap = p.collect_frame(&s, None, window());
    assert!(snap.contains(IntervalId(2)));
    assert_eq!(
        snap.interval(IntervalId(2)).unwrap().range.start,
        Timestamp(160)
    );

    // Second frame with a window that excludes id 2: no stale leakage.
    let snap = p.collect_frame(&s, None, TimeRange::new(Timestamp(100), Timestamp(155)));
    assert!(snap.contains(IntervalId(1)));
    assert!(!snap.contains(IntervalId(2)));
    assert!(snap.interval(IntervalId(2)).is_none());
    assert_eq!(snap.intervals.len(), 1);
}

#[test]
fn invalidation_events_drain() {
    let mut s = source(TemporalDescriptors::default());
    s.events.push(TemporalInvalidationEvent::IntervalsChanged);
    s.events.push(TemporalInvalidationEvent::SetReplaced);
    let drained = s.drain_invalidation_events();
    assert_eq!(drained.len(), 2);
    assert!(s.drain_invalidation_events().is_empty());
}
