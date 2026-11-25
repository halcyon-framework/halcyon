// =============================================================================
// SPEC-HAL-335 §8: LaneLayoutEngine — Intra-Row Lane Resolution
// =============================================================================
//
// INV-335-L1: Auto packing is a pure, deterministic function of the culled
//             interval set. Identical input, identical lane assignment,
//             every frame.
// INV-335-L2: Lanes subdivide the row height the RowSource granted. This
//             engine outputs lane indices and per-row lane counts only; it
//             never grows rows and never touches pixel geometry.
//
// Layer law (Auto): lanes de-overlap concurrent PRIMARY intervals. Baseline
// and Preview overlap their primaries by design and take lane.unwrap_or(0);
// a lane value on a Primary interval is ignored under Auto — packing is the
// authority. Pairing a baseline to its primary in a multi-lane row is
// application semantics (FORBID-335-2): use Explicit or a pinned lane.
//
// Output is frame-local: lane indices are parallel to the canonical snapshot
// interval order and must not be retained across frames (INV-335-ID1).
// =============================================================================

use std::collections::HashMap;

use super::descriptors::{IntervalDescriptor, IntervalId, IntervalLayer};
use crate::model::time::Timestamp;

/// Upper bound on lanes per row. Packing overflow collapses into the last
/// lane; explicit pins clamp. Bounds all arithmetic and degenerate geometry.
pub const MAX_LANES: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanePolicy {
    /// Substrate packs: deterministic first-fit over Primary intervals,
    /// sorted by (range.start, id); lowest lane whose last end <= start.
    /// Half-open: edge-touching intervals share a lane.
    #[default]
    Auto,
    /// Application assigns via IntervalDescriptor.lane; missing lane => 0.
    Explicit,
}

#[derive(Debug, Default)]
pub struct LaneLayoutEngine {
    /// Lane per interval, parallel to the input (canonical snapshot order).
    lanes: Vec<u8>,
    /// Lane count per row id. Absent rows resolve to 1.
    row_lane_counts: HashMap<u64, u8>,
    /// Per-row first-fit state: last occupied end per lane. Reused.
    scratch_lane_ends: Vec<i64>,
}

impl LaneLayoutEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve lanes for one frame. `intervals` must be in the canonical
    /// snapshot order (layer, row, range.start, id).
    pub fn resolve(&mut self, intervals: &[IntervalDescriptor], policy: LanePolicy) {
        fn key(d: &IntervalDescriptor) -> (IntervalLayer, u64, Timestamp, IntervalId) {
            (d.layer, d.row, d.range.start, d.id)
        }
        debug_assert!(
            intervals.windows(2).all(|w| key(&w[0]) <= key(&w[1])),
            "LaneLayoutEngine requires the canonical snapshot order"
        );

        self.lanes.clear();
        self.row_lane_counts.clear();
        self.scratch_lane_ends.clear();
        let mut current_row: Option<u64> = None;

        for d in intervals {
            let lane = match policy {
                LanePolicy::Explicit => d.lane.unwrap_or(0).min(MAX_LANES - 1),
                LanePolicy::Auto => match d.layer {
                    IntervalLayer::Primary => {
                        if current_row != Some(d.row) {
                            current_row = Some(d.row);
                            self.scratch_lane_ends.clear();
                        }
                        let start = d.range.start.0;
                        // Zero-duration intervals occupy one microsecond for
                        // packing, so coincident milestones get distinct lanes.
                        let eff_end = d.range.end.0.max(start + 1);
                        let mut assigned: Option<u8> = None;
                        for (l, end) in self.scratch_lane_ends.iter_mut().enumerate() {
                            if *end <= start {
                                *end = eff_end;
                                assigned = Some(l as u8);
                                break;
                            }
                        }
                        match assigned {
                            Some(l) => l,
                            None => {
                                if self.scratch_lane_ends.len() < MAX_LANES as usize {
                                    self.scratch_lane_ends.push(eff_end);
                                    (self.scratch_lane_ends.len() - 1) as u8
                                } else {
                                    // Overflow: collapse into the last lane.
                                    let last = self.scratch_lane_ends.len() - 1;
                                    if eff_end > self.scratch_lane_ends[last] {
                                        self.scratch_lane_ends[last] = eff_end;
                                    }
                                    (MAX_LANES - 1) as u8
                                }
                            }
                        }
                    }
                    _ => d.lane.unwrap_or(0).min(MAX_LANES - 1),
                },
            };
            self.lanes.push(lane);
            let needed = lane + 1; // lane <= MAX_LANES - 1, no overflow
            let count = self.row_lane_counts.entry(d.row).or_insert(1);
            if needed > *count {
                *count = needed;
            }
        }
    }

    /// Lane of the interval at index `i` of the resolved frame's canonical
    /// interval order. Frame-local; never retain across frames.
    pub fn lane_of(&self, i: usize) -> u8 {
        self.lanes[i]
    }

    pub fn lanes(&self) -> &[u8] {
        &self.lanes
    }

    /// Lane count for a row. Rows without intervals resolve to 1.
    pub fn lane_count(&self, row: u64) -> u8 {
        self.row_lane_counts.get(&row).copied().unwrap_or(1)
    }
}
