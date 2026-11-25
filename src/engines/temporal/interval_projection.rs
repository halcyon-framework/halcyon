// =============================================================================
// SPEC-HAL-335 §6: IntervalProjection — Per-Frame Culled Snapshot
// =============================================================================
//
// INV-335-D2: The projection intersects every descriptor against the
//             half-open visible range and the row projection before exposing
//             the frame snapshot. Sources may over-return; consumers receive
//             only paint-relevant descriptors.
// INV-335-D4: An arrow whose endpoints are not both present in the snapshot
//             is dropped here. Re-routing to visible ancestors is an
//             application-layer concern.
// INV-335-ID1: Duplicate IntervalId within one frame is a source contract
//             violation. Debug builds panic; release builds keep the first
//             occurrence.
//
// Determinism: snapshot ordering is canonical — intervals by
// (layer, row, range.start, id); bands by (row, start, end); markers by ts;
// arrows by (from, to). Identical input, identical snapshot.
//
// Buffers are reused across frames; allocations are retained.
// =============================================================================

use std::collections::{HashMap, HashSet};

use crate::model::time::TimeRange;

use super::descriptors::{
    ArrowDescriptor, IntervalDescriptor, IntervalId, IntervalSource, MarkerDescriptor,
    RowBandDescriptor, TemporalDescriptors,
};

/// Culled, canonically ordered, identity-indexed descriptor set for one frame.
#[derive(Debug, Clone, Default)]
pub struct TemporalFrameSnapshot {
    pub intervals: Vec<IntervalDescriptor>,
    pub bands: Vec<RowBandDescriptor>,
    pub arrows: Vec<ArrowDescriptor>,
    pub markers: Vec<MarkerDescriptor>,
    index: HashMap<IntervalId, usize>,
}

impl TemporalFrameSnapshot {
    pub fn contains(&self, id: IntervalId) -> bool {
        self.index.contains_key(&id)
    }

    pub fn interval(&self, id: IntervalId) -> Option<&IntervalDescriptor> {
        self.index.get(&id).map(|&i| &self.intervals[i])
    }

    /// Frame-local index into the canonical interval order — the same index
    /// space LaneLayoutEngine resolves against. Never retain across frames.
    pub fn index_of(&self, id: IntervalId) -> Option<usize> {
        self.index.get(&id).copied()
    }

    fn clear(&mut self) {
        self.intervals.clear();
        self.bands.clear();
        self.arrows.clear();
        self.markers.clear();
        self.index.clear();
    }
}

#[derive(Debug, Default)]
pub struct IntervalProjection {
    scratch: TemporalDescriptors,
    snapshot: TemporalFrameSnapshot,
    row_set: HashSet<u64>,
}

impl IntervalProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the per-frame snapshot. `rows = None` is free mode (no row
    /// filtering); `Some` filters to the given row projection.
    pub fn collect_frame(
        &mut self,
        source: &dyn IntervalSource,
        rows: Option<&[u64]>,
        visible: TimeRange,
    ) -> &TemporalFrameSnapshot {
        self.scratch.clear();
        self.snapshot.clear();
        source.collect(rows, visible, &mut self.scratch);

        let bound = rows.is_some();
        self.row_set.clear();
        if let Some(rows) = rows {
            self.row_set.extend(rows.iter().copied());
        }

        // Intervals: row filter, half-open cull, identity enforcement.
        for d in self.scratch.intervals.drain(..) {
            if bound && !self.row_set.contains(&d.row) {
                continue;
            }
            let visible_now = if d.range.is_empty() {
                // Zero-duration (milestone): point rule, so a milestone on the
                // left edge is visible.
                visible.contains(d.range.start)
            } else {
                d.range.intersects(visible)
            };
            if !visible_now {
                continue;
            }
            if self.snapshot.index.insert(d.id, 0).is_some() {
                debug_assert!(
                    false,
                    "INV-335-ID1: duplicate IntervalId {:?} in one frame",
                    d.id
                );
                continue;
            }
            self.snapshot.intervals.push(d);
        }

        // Bands: row filter + non-empty intersection.
        for b in self.scratch.bands.drain(..) {
            if bound && !self.row_set.contains(&b.row) {
                continue;
            }
            if b.range.is_empty() || !b.range.intersects(visible) {
                continue;
            }
            self.snapshot.bands.push(b);
        }

        // Markers: half-open containment.
        for m in self.scratch.markers.drain(..) {
            if !visible.contains(m.ts) {
                continue;
            }
            self.snapshot.markers.push(m);
        }

        // Arrows: both endpoints must have survived. INV-335-D4.
        for a in self.scratch.arrows.drain(..) {
            if !(self.snapshot.index.contains_key(&a.from)
                && self.snapshot.index.contains_key(&a.to))
            {
                continue;
            }
            self.snapshot.arrows.push(a);
        }

        // Canonical deterministic ordering.
        self.snapshot
            .intervals
            .sort_by_key(|d| (d.layer, d.row, d.range.start, d.id));
        self.snapshot
            .bands
            .sort_by_key(|b| (b.row, b.range.start, b.range.end));
        self.snapshot.markers.sort_by_key(|m| m.ts);
        self.snapshot.arrows.sort_by_key(|a| (a.from, a.to));

        // Rebuild the identity index against the sorted order.
        for (i, d) in self.snapshot.intervals.iter().enumerate() {
            self.snapshot.index.insert(d.id, i);
        }

        &self.snapshot
    }

    pub fn snapshot(&self) -> &TemporalFrameSnapshot {
        &self.snapshot
    }
}
