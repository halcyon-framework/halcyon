// SPEC-HAL-335 §3, §9–§11 invariant tests: TemporalSubstrate assembly.

use std::cell::Cell;

use crate::core::event::Modifiers;
use crate::core::ordered_selection::SelectionMode;
use crate::core::paint::Color;
use crate::engines::temporal::{
    IntervalDescriptor, IntervalId, IntervalLayer, IntervalSource, IntervalStyle, LanePolicy,
    TemporalDescriptors, TemporalIntent, TemporalInvalidationEvent, TemporalSubstrate,
    TimeGeometryEngine, TimeScale,
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

struct CountingSource {
    data: TemporalDescriptors,
    events: Vec<TemporalInvalidationEvent>,
    collects: Cell<usize>,
}

impl CountingSource {
    fn new(data: TemporalDescriptors) -> Self {
        Self {
            data,
            events: Vec::new(),
            collects: Cell::new(0),
        }
    }
}

impl IntervalSource for CountingSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        self.collects.set(self.collects.get() + 1);
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

/// Anchor 0, Seconds scale, 16 px/s, 160 px viewport → visible [0 s, 10 s).
fn substrate() -> TemporalSubstrate {
    let geometry = TimeGeometryEngine::new(Timestamp(0), TimeScale::Seconds, 16.0, 160.0);
    TemporalSubstrate::new(geometry, SelectionMode::Multi, LanePolicy::Auto)
}

const S: i64 = 1_000_000; // one second in µs

#[test]
fn collect_gated_when_inputs_stable() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 2 * S, 5 * S));
    let mut src = CountingSource::new(data);
    let mut sub = substrate();

    sub.begin_frame(&mut src, None);
    sub.begin_frame(&mut src, None);
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 1); // first frame only
    assert_eq!(sub.snapshot().intervals.len(), 1);
}

#[test]
fn range_change_triggers_collect() {
    let mut src = CountingSource::new(TemporalDescriptors::default());
    let mut sub = substrate();
    sub.begin_frame(&mut src, None);
    sub.geometry_mut().scroll_by_px(16.0); // one second forward
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 2);
}

#[test]
fn row_projection_change_triggers_collect() {
    let mut src = CountingSource::new(TemporalDescriptors::default());
    let mut sub = substrate();
    sub.begin_frame(&mut src, Some(&[1, 2]));
    sub.begin_frame(&mut src, Some(&[1, 2])); // stable: no collect
    sub.begin_frame(&mut src, Some(&[1])); // changed
    sub.begin_frame(&mut src, None); // bound -> free
    assert_eq!(src.collects.get(), 3);
}

#[test]
fn invalidation_triggers_collect_and_keeps_selection() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 2 * S, 5 * S));
    let mut src = CountingSource::new(data);
    let mut sub = substrate();
    sub.begin_frame(&mut src, None);
    sub.selection_mut().selected_keys.insert(IntervalId(1));

    src.events.push(TemporalInvalidationEvent::IntervalsChanged);
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 2);
    assert!(sub.selection().is_selected(IntervalId(1))); // never touched
}

#[test]
fn set_replaced_prunes_to_source_survivors() {
    let mut data = TemporalDescriptors::default();
    data.intervals.push(iv(1, 0, 2 * S, 5 * S)); // inside the viewport
    data.intervals.push(iv(2, 0, 20 * S, 30 * S)); // OUTSIDE the viewport
    data.intervals.push(iv(3, 0, 3 * S, 6 * S));
    let mut src = CountingSource::new(data);
    let mut sub = substrate();
    sub.begin_frame(&mut src, None);

    sub.selection_mut().selected_keys.insert(IntervalId(1));
    sub.selection_mut().selected_keys.insert(IntervalId(2));
    sub.selection_mut().selected_keys.insert(IntervalId(3));
    sub.selection_mut().focus_key = Some(IntervalId(3));
    sub.selection_mut().anchor_key = Some(IntervalId(1));

    // Replace the set: id 3 vanishes; ids 1 and 2 survive in the source.
    src.data.intervals.retain(|d| d.id != IntervalId(3));
    src.events.push(TemporalInvalidationEvent::SetReplaced);
    sub.begin_frame(&mut src, None);

    // INV-335-S2: id 2 survives despite being culled from the viewport;
    // id 3 is pruned; nothing is cleared wholesale.
    assert!(sub.selection().is_selected(IntervalId(1)));
    assert!(sub.selection().is_selected(IntervalId(2)));
    assert!(!sub.selection().is_selected(IntervalId(3)));
    assert_eq!(sub.selection().focus_key, None);
    assert_eq!(sub.selection().anchor_key, Some(IntervalId(1)));
}

#[test]
fn intents_fifo_and_drain() {
    let mut sub = substrate();
    assert!(!sub.has_pending_intents());
    sub.push_intent(TemporalIntent::Select {
        id: IntervalId(1),
        modifiers: Modifiers::default(),
    });
    sub.push_intent(TemporalIntent::Activate { id: IntervalId(1) });
    sub.push_intent(TemporalIntent::VisibleRangeChanged {
        range: TimeRange::new(Timestamp(0), Timestamp(S)),
    });
    sub.push_intent(TemporalIntent::DeselectAll);
    assert!(sub.has_pending_intents());
    let drained = sub.drain_intents();
    assert_eq!(drained.len(), 4);
    assert!(matches!(drained[0], TemporalIntent::Select { .. }));
    assert!(matches!(drained[1], TemporalIntent::Activate { .. }));
    assert!(matches!(
        drained[2],
        TemporalIntent::VisibleRangeChanged { .. }
    ));
    assert!(matches!(drained[3], TemporalIntent::DeselectAll));
    assert!(!sub.has_pending_intents());
    assert!(sub.drain_intents().is_empty());
}

#[test]
fn time_context_partial_eq_gate() {
    let mut src = CountingSource::new(TemporalDescriptors::default());
    let mut sub = substrate();
    sub.begin_frame(&mut src, None);

    let a = sub.time_context();
    let b = sub.time_context();
    assert_eq!(a, b); // unchanged frame: SignalStore::set would no-op

    sub.set_cursor(Some(Timestamp(3 * S)));
    let c1 = sub.time_context();
    assert_ne!(a, c1);

    sub.geometry_mut().scroll_by_px(16.0);
    let c2 = sub.time_context();
    assert_ne!(c1, c2);
    assert_eq!(c2.cursor, Some(Timestamp(3 * S)));
}

#[test]
fn lane_policy_change_forces_rebuild() {
    let mut data = TemporalDescriptors::default();
    let mut pinned = iv(1, 7, 2 * S, 5 * S);
    pinned.lane = Some(3);
    data.intervals.push(pinned);
    data.intervals.push(iv(2, 7, 2 * S, 5 * S));
    let mut src = CountingSource::new(data);
    let mut sub = substrate();

    sub.begin_frame(&mut src, None);
    // Auto: pin on Primary ignored; two overlapping primaries → 2 lanes.
    assert_eq!(sub.lanes().lane_count(7), 2);

    sub.set_lane_policy(LanePolicy::Explicit);
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 2); // policy change forced a rebuild
    assert_eq!(sub.lanes().lane_count(7), 4); // pin 3 honored → count 4
}

#[test]
fn resolution_cached_per_frame_and_tracks_geometry() {
    let mut src = CountingSource::new(TemporalDescriptors::default());
    let mut sub = substrate();
    sub.begin_frame(&mut src, None);
    let r0 = sub.resolution().clone();
    assert_eq!(r0.visible_range, sub.geometry().visible_range());
    assert_eq!(r0.now_x, None);

    // Geometry-only change (now): resolution must update while collection
    // stays gated (INV-335-G5 vs SPEC-PM-010 triggers are independent).
    sub.geometry_mut().set_now(Some(Timestamp(3 * S)));
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 1); // collect still gated
    assert!(sub.resolution().now_x.is_some());
    assert_ne!(sub.resolution(), &r0);

    // Range change: both collect and resolution follow.
    sub.geometry_mut().scroll_by_px(16.0);
    sub.begin_frame(&mut src, None);
    assert_eq!(src.collects.get(), 2);
    assert_eq!(sub.resolution().visible_range.start, Timestamp(S));
}
