// =============================================================================
// SPEC-HAL-335 §3, §9–§11: TemporalSubstrate — Composite Assembly
// =============================================================================
//
// Composition over subsumption: the substrate owns the three temporal
// engines plus the cross-cutting state — selection, intent queue, time
// context. It is a substrate-level engine; it is not a widget and must
// never become one (FORBID-335-5). It owns no UI events (FORBID-335-3).
//
// INV-335-S1: Selection is OrderedSelectionState<IntervalId> — the shared
//             engine (SPEC-HAL-114/135/136). No temporal-specific selection
//             engine exists.
// INV-335-S2: Selection survives scroll, collapse, and cull. On SetReplaced
//             it is pruned to SOURCE-surviving ids (IntervalSource::
//             interval_exists), never cleared wholesale. The culled snapshot
//             is not the pruning oracle.
// INV-335-I1: No synchronous callbacks. Widgets enqueue TemporalIntent;
//             applications drain in the deferred-mutation phase
//             (SPEC-HAL-177). Nothing here mutates application state.
// INV-335-D3 / SPEC-PM-010: Collection is gated by exhaustive triggers —
//             invalidation events, visible-range change, row-projection
//             change, lane-policy change. Each trigger is tested.
//
// TimeContext derives Clone + PartialEq precisely so the application layer
// can publish it through SignalStore::set (SPEC-HAL-031), whose PartialEq
// gate provides change notification. The substrate owns no signals.
// =============================================================================

use crate::core::event::Modifiers;
use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, ReduceResult, SelectionMode,
    reduce_ordered_selection,
};
use crate::model::time::{TimeRange, Timestamp};

use super::containment::ContainmentSource;
use super::descriptors::{IntervalId, IntervalLayer, IntervalSource, TemporalInvalidationEvent};
use super::emphasis::{EmphasisRelation, EmphasisState};
use super::interval_projection::{IntervalProjection, TemporalFrameSnapshot};
use super::lane_layout::{LaneLayoutEngine, LanePolicy};
use super::time_geometry::{TimeGeometryEngine, TimeGeometryResolution};

/// Deferred interaction products. SPEC-HAL-335 §10.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalIntent {
    Select {
        id: IntervalId,
        modifiers: Modifiers,
    },
    /// Empty-canvas press cleared the selection — SPEC-HAL-336 §7 /
    /// SPEC-HAL-135 empty-space rule (commit on press).
    DeselectAll,
    Activate {
        id: IntervalId,
    },
    DragMove {
        id: IntervalId,
        new_range: TimeRange,
    },
    DragResize {
        id: IntervalId,
        new_end: Timestamp,
    },
    ContextMenu {
        id: Option<IntervalId>,
        ts: Timestamp,
    },
    /// Plain-wheel / hand-tool vertical pan request — SPEC-HAL-336 §7.
    /// The composition applies `dy` through the row provider's single
    /// vertical-scroll path (INV-337-S1 / FORBID-337-4). Positive `dy`
    /// advances scroll_y (ScrollArea convention).
    VerticalPanBy {
        dy: f32,
    },
    VisibleRangeChanged {
        range: TimeRange,
    },
}

/// Shared time context. SPEC-HAL-335 §11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeContext {
    pub visible_range: TimeRange,
    pub cursor: Option<Timestamp>,
}

/// Shared handle: header, canvas, and the composing screen consume one
/// substrate instance (INV-336-1). FORBID-335-5: the substrate itself
/// remains a non-widget; widgets hold this handle.
pub type SharedTemporalSubstrate = std::rc::Rc<std::cell::RefCell<TemporalSubstrate>>;

/// Ordered oracle over the contiguous Primary block of the canonical
/// snapshot order (layer-major sort guarantees contiguity).
struct PrimaryOracle<'a> {
    snapshot: &'a TemporalFrameSnapshot,
    lo: usize,
    hi: usize,
}

impl OrderedIndex<IntervalId> for PrimaryOracle<'_> {
    fn len(&self) -> usize {
        self.hi - self.lo
    }

    fn key_at(&self, index: usize) -> IntervalId {
        self.snapshot.intervals[self.lo + index].id
    }

    fn index_of(&self, key: IntervalId) -> Option<usize> {
        self.snapshot
            .index_of(key)
            .and_then(|i| (i >= self.lo && i < self.hi).then(|| i - self.lo))
    }
}

pub struct TemporalSubstrate {
    geometry: TimeGeometryEngine,
    projection: IntervalProjection,
    lanes: LaneLayoutEngine,
    lane_policy: LanePolicy,
    selection: OrderedSelectionState<IntervalId>,
    emphasis: EmphasisState,
    intents: Vec<TemporalIntent>,
    cursor: Option<Timestamp>,
    /// Per-frame geometry resolution (INV-335-G5): computed once at the end
    /// of begin_frame; header, canvas, and hit-testing read this cache.
    resolution: TimeGeometryResolution,
    // Collect cache keys (SPEC-PM-010: exhaustive triggers).
    last_visible: Option<TimeRange>,
    last_rows: Vec<u64>,
    last_rows_some: bool,
    collect_forced: bool,
}

impl TemporalSubstrate {
    pub fn new(
        geometry: TimeGeometryEngine,
        selection_mode: SelectionMode,
        lane_policy: LanePolicy,
    ) -> Self {
        let resolution = geometry.resolve();
        Self {
            geometry,
            projection: IntervalProjection::new(),
            lanes: LaneLayoutEngine::new(),
            lane_policy,
            selection: OrderedSelectionState::new(selection_mode),
            emphasis: EmphasisState::new(),
            intents: Vec::new(),
            cursor: None,
            resolution,
            last_visible: None,
            last_rows: Vec::new(),
            last_rows_some: false,
            collect_forced: true, // first frame always collects
        }
    }

    // ------------------------------------------------------------------ frame

    /// Per-frame entry point, called after deferred mutations (INV-335-D3).
    /// `rows = None` is free mode; `Some` is the row projection from the
    /// row provider (e.g. HTS visible rows).
    pub fn begin_frame(&mut self, source: &mut dyn IntervalSource, rows: Option<&[u64]>) {
        let events = source.drain_invalidation_events();
        let data_dirty = !events.is_empty();
        let set_replaced = events
            .iter()
            .any(|e| matches!(e, TemporalInvalidationEvent::SetReplaced));

        let visible = self.geometry.visible_range();
        let rows_some = rows.is_some();
        let rows_slice: &[u64] = rows.unwrap_or(&[]);
        let inputs_changed = self.last_visible != Some(visible)
            || self.last_rows_some != rows_some
            || self.last_rows.as_slice() != rows_slice;

        if data_dirty || inputs_changed || self.collect_forced {
            self.projection.collect_frame(&*source, rows, visible);
            self.lanes
                .resolve(&self.projection.snapshot().intervals, self.lane_policy);
            self.last_visible = Some(visible);
            self.last_rows_some = rows_some;
            self.last_rows.clear();
            self.last_rows.extend_from_slice(rows_slice);
            self.collect_forced = false;
        }

        if set_replaced {
            self.prune_selection(&*source);
        }

        // INV-335-G5: one resolution per frame, recomputed unconditionally —
        // geometry-only changes (now, calendar, frame) must reflect even when
        // collection stays gated.
        self.resolution = self.geometry.resolve();
    }

    /// INV-335-S2: prune to source-surviving ids only. Never wholesale.
    fn prune_selection(&mut self, source: &dyn IntervalSource) {
        self.selection
            .selected_keys
            .retain(|k| source.interval_exists(*k));
        if let Some(f) = self.selection.focus_key {
            if !source.interval_exists(f) {
                self.selection.focus_key = None;
            }
        }
        if let Some(a) = self.selection.anchor_key {
            if !source.interval_exists(a) {
                self.selection.anchor_key = None;
            }
        }
        if let Some(a) = self.selection.range_anchor_key {
            if !source.interval_exists(a) {
                self.selection.range_anchor_key = None;
            }
        }
    }

    // ------------------------------------------------------------- accessors

    pub fn geometry(&self) -> &TimeGeometryEngine {
        &self.geometry
    }

    /// Horizontal navigation flows through the engine (INV-335-G1). The next
    /// begin_frame detects the visible-range change and recollects.
    pub fn geometry_mut(&mut self) -> &mut TimeGeometryEngine {
        &mut self.geometry
    }

    /// INV-336-1 / INV-335-G5: the per-frame resolution shared by header,
    /// canvas, and hit-testing. Valid after begin_frame.
    pub fn resolution(&self) -> &TimeGeometryResolution {
        &self.resolution
    }

    pub fn snapshot(&self) -> &TemporalFrameSnapshot {
        self.projection.snapshot()
    }

    pub fn lanes(&self) -> &LaneLayoutEngine {
        &self.lanes
    }

    pub fn lane_policy(&self) -> LanePolicy {
        self.lane_policy
    }

    pub fn set_lane_policy(&mut self, policy: LanePolicy) {
        if self.lane_policy != policy {
            self.lane_policy = policy;
            self.collect_forced = true;
        }
    }

    pub fn selection(&self) -> &OrderedSelectionState<IntervalId> {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut OrderedSelectionState<IntervalId> {
        &mut self.selection
    }

    /// INV-335-S1: selection mutations route through the shared reducer
    /// (SPEC-HAL-114/135/136) against a Primary-layer oracle. Range sweeps
    /// never touch Baseline/Preview ids.
    pub fn reduce_selection(
        &mut self,
        action: OrderedSelectionAction<IntervalId>,
    ) -> ReduceResult<IntervalId> {
        let snapshot = self.projection.snapshot();
        let lo = snapshot
            .intervals
            .partition_point(|d| d.layer < IntervalLayer::Primary);
        let hi = snapshot
            .intervals
            .partition_point(|d| d.layer <= IntervalLayer::Primary);
        let oracle = PrimaryOracle { snapshot, lo, hi };
        reduce_ordered_selection(&mut self.selection, action, &oracle)
    }

    // -------------------------------------------------------------- emphasis

    /// The emphasis state, parallel to `selection()` (SPEC-HAL-342). Valid
    /// after this frame's `resolve_emphasis`.
    pub fn emphasis(&self) -> &EmphasisState {
        &self.emphasis
    }

    /// Set the hovered focus — the emphasis peer of `set_cursor`. Cheap;
    /// resolution is deferred to `resolve_emphasis` (INV-342-E4).
    pub fn set_emphasis_focus(&mut self, focus: Option<IntervalId>) {
        self.emphasis.set_focus(focus);
    }

    /// Resolve the linked set for the current focus under `relation`, against
    /// the current visible range. `containment` supplies the Containment
    /// relation's data; `None` (or a flat source) degrades Containment to
    /// Identity (INV-342-E7). Gated on focus/relation change (INV-342-E4); one
    /// hop only (INV-342-E3).
    pub fn resolve_emphasis(
        &mut self,
        relation: EmphasisRelation,
        containment: Option<&dyn ContainmentSource>,
    ) {
        let visible = self.geometry.visible_range();
        self.emphasis.resolve(relation, containment, visible);
    }

    // --------------------------------------------------------------- intents

    /// Widgets enqueue during event routing. INV-335-I1.
    pub fn push_intent(&mut self, intent: TemporalIntent) {
        self.intents.push(intent);
    }

    /// Applications drain in the deferred-mutation phase. FIFO.
    pub fn drain_intents(&mut self) -> Vec<TemporalIntent> {
        std::mem::take(&mut self.intents)
    }

    pub fn has_pending_intents(&self) -> bool {
        !self.intents.is_empty()
    }

    // ---------------------------------------------------------- time context

    /// Cursor instant under the pointer, fed by the canvas (INV-336-I2).
    pub fn set_cursor(&mut self, cursor: Option<Timestamp>) {
        self.cursor = cursor;
    }

    pub fn time_context(&self) -> TimeContext {
        TimeContext {
            visible_range: self.geometry.visible_range(),
            cursor: self.cursor,
        }
    }
}
