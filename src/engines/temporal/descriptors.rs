// =============================================================================
// SPEC-HAL-335 §5–§6: Identity, Descriptors, and the IntervalSource Contract
// =============================================================================
//
// INV-335-ID1: Every interval carries a source-owned, stable IntervalId.
//              Selection, focus, intents, and hit results are keyed on
//              IntervalId exclusively. Positional indices are never identity
//              (SPEC-PM-001).
// INV-335-D1:  Sources hold no pixel knowledge. TimeRange is the only query
//              vocabulary that crosses this trait.
// INV-335-D3:  Pull-based with invalidation events. Data changes never force
//              a frame (SPEC-HAL-258); collection happens during the frame,
//              after deferred mutations.
// =============================================================================

use crate::core::paint::Color;
use crate::model::time::{TimeRange, Timestamp};

/// Source-owned, stable interval identity. INV-335-ID1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntervalId(pub u64);

/// Paint layer. Declaration order is canonical paint order (back to front).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntervalLayer {
    Baseline,
    Primary,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalStyle {
    Solid,
    Hatched,
    Hollow,
    /// Zero-duration, row-anchored. Distinct from MarkerDescriptor, which is
    /// row-independent. They are not alternatives (SPEC-HAL-335 §6.1).
    Milestone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalEdge {
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerStyle {
    VerticalLine,
    Diamond,
    Flag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntervalDescriptor {
    pub id: IntervalId,
    /// Row identity (RowSource row_id). Ignored in free mode.
    pub row: u64,
    pub range: TimeRange,
    pub color: Color,
    pub label: Option<String>,
    /// 0.0 – 1.0.
    pub progress: f32,
    pub style: IntervalStyle,
    pub layer: IntervalLayer,
    /// Explicit lane under LanePolicy::Explicit; None under Auto.
    pub lane: Option<u8>,
}

/// Row-scoped background band: per-row non-working time, capacity strips,
/// shift windows — everything the default calendar must not know
/// (INV-335-G7). Painted beneath intervals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowBandDescriptor {
    pub row: u64,
    pub range: TimeRange,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowDescriptor {
    pub from: IntervalId,
    pub from_edge: IntervalEdge,
    pub to: IntervalId,
    pub to_edge: IntervalEdge,
    pub critical: bool,
    pub color: Option<Color>,
}

/// Row-independent date annotation spanning the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerDescriptor {
    pub ts: Timestamp,
    pub color: Color,
    pub label: Option<String>,
    pub style: MarkerStyle,
}

/// Collection buffer filled by IntervalSource::collect.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemporalDescriptors {
    pub intervals: Vec<IntervalDescriptor>,
    pub bands: Vec<RowBandDescriptor>,
    pub arrows: Vec<ArrowDescriptor>,
    pub markers: Vec<MarkerDescriptor>,
}

impl TemporalDescriptors {
    pub fn clear(&mut self) {
        self.intervals.clear();
        self.bands.clear();
        self.arrows.clear();
        self.markers.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalInvalidationEvent {
    /// Values or geometry of intervals changed.
    IntervalsChanged,
    /// Wholesale replacement; selection is pruned to surviving ids
    /// (INV-335-S2, wired at the substrate assembly).
    SetReplaced,
}

/// Pull-based data contract. INV-335-D1/D3.
pub trait IntervalSource {
    /// Append descriptors intersecting `range` for the given rows.
    /// `rows = None` in free mode. Over-returning is permitted; the
    /// projection culls precisely (INV-335-D2).
    fn collect(&self, rows: Option<&[u64]>, range: TimeRange, out: &mut TemporalDescriptors);

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent>;

    /// Identity liveness: does an interval with this id currently exist in
    /// the source, regardless of visibility? Pruning oracle for
    /// INV-335-S2 — the culled snapshot cannot answer this, because
    /// selection survives cull.
    fn interval_exists(&self, id: IntervalId) -> bool;
}
