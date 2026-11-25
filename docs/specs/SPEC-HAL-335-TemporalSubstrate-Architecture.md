# SPEC-HAL-335: **TemporalSubstrate Architecture**

### *Structural Engine for Time-Positioned Projection*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Amended by:** SPEC-HAL-337 (Shared Topology Substrate) — bound-mode row-geometry source (§7)
**Layer:** Substrate (Structural Engine)
**Supersedes:** SPEC-HAL-282 (TimeAxis and TimelineCanvas) — model layer
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model)
* SPEC-HAL-114 / 135 / 136 (Ordered Selection family)
* SPEC-HAL-144 / 177 / 200 (Intent routing, deferred queue, intent policy)
* SPEC-HAL-258 (Engine Invalidation Graph & Data Flow Law)
* SPEC-HAL-259 (HierarchicalTabularSubstrate — RowSource provider)
* SPEC-PM-001 (Virtualization Identity), SPEC-PM-009 (Dual-Source Geometry Drift)

---

# 1. Purpose

Define TemporalSubstrate: a structural engine that maps identity-keyed time intervals onto horizontal pixel geometry, for consumption by projection widgets.

It is the time-domain peer of the tabular substrate family. `TimeGeometryEngine` is to the time dimension what `ColumnGeometryEngine` is to the column dimension: a single authority mapping domain values to horizontal pixels, consumed by every party — header, body, hit-test — from one per-frame resolution.

This SPEC defines architecture only. No widget, no rendering rules, no styling. Rendering is SPEC-HAL-336.

TemporalSubstrate is a substrate-level engine. It is not a widget and must never become one.

# 2. Problem Statement

SPEC-HAL-282 drew the correct seam (TimeAxis model / TimelineCanvas widget) but specified it widget-first: the model owned by one canvas, scroll state mirrored across an authority boundary, intervals without identity, headers without an owner, interactions as synchronous callbacks.

The product class — professional workstation software: scheduling, manufacturing, medical, scientific, financial — requires multiple visual manifestations of one mechanism: Gantt bodies, resource boards, machine schedulers, single-lane stage strips. Per the substrate doctrine (SPEC-HAL-259 §2): reusing a one-off widget introduces semantic contamination; copying it introduces uncontrolled duplication. Therefore: a substrate, with thin widget projections.

# 3. Decomposition

```
TemporalSubstrate
  ├─ TimeGeometryEngine        (time ↔ pixel authority, viewport, tiers, calendar)
  ├─ IntervalProjection        (identity-keyed intervals, culling, per-frame snapshot)
  ├─ LaneLayoutEngine          (intra-row lane resolution)
  ├─ OrderedSelectionState<IntervalId>   (shared engine, reused — SPEC-HAL-114/135/136)
  └─ IntentQueue<TemporalIntent>         (deferred intents — SPEC-HAL-177 pattern)
```

Row geometry is **not** a component: it is consumed through `RowSource` (§7). In bound mode the HTS provides it; the substrate never owns vertical geometry.

No engine reaches backward into another.

# 4. TimeGeometryEngine

## 4.1 Ownership

```rust
pub struct TimeGeometryEngine {
    anchor: Timestamp,        // instant at viewport x = 0
    px_per_unit: f32,
    scale: TimeScale,
    viewport_width: f32,
    frame: TimeFrame,         // civil projection (SPEC-HAL-334 §4)
    calendar: WorkingCalendar // default working-time calendar
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeScale {
    Seconds, Minutes, Hours, Days, Weeks, Months,
}
```

**INV-335-G1 (Single Horizontal Authority):** The engine is the sole owner of horizontal time position. There is no `scroll_x`, no mirrored offset, no second origin — anywhere. Horizontal navigation mutates `anchor`/`px_per_unit` here and nowhere else. This is the generalization of SPEC-HAL-164 INV-164-2 to the time dimension, and the direct application of SPEC-PM-009: one computation, one result, consumed by all.

**INV-335-G2 (Pure Mapping):** `x_of(ts: Timestamp) -> f32` and `instant_at(x: f32) -> Timestamp` are pure, deterministic, mutually inverse within the viewport, and viewport-local (x = 0 at the viewport's left edge).

**INV-335-G3 (Zoom Anchor):** `zoom_at(x: f32, factor: f32)` preserves the instant at `x`: `instant_at(x)` before == `instant_at(x)` after. Mirror of SPEC-HAL-284 INV-284-C5 in one dimension.

**INV-335-G4 (Navigation):** `scroll_by_px(dx)`, `scroll_to(ts)`, `visible_range() -> TimeRange`. All expressed through `anchor`.

## 4.2 Per-frame resolution

```rust
pub struct TimeGeometryResolution {
    pub visible_range: TimeRange,
    pub tiers: Vec<HeaderTier>,        // civil header rows for current scale
    pub gridlines: Vec<f32>,           // x of unit boundaries
    pub non_working: Vec<(f32, f32)>,  // x-spans where default calendar is non-working
    pub now_x: Option<f32>,            // today/now marker, if in range
}
```

**INV-335-G5 (One Resolution Per Frame):** `resolve()` is computed at most once per frame. Header widget, canvas widget, and hit-testing consume this one value. Independent recomputation of tiers, gridlines, or mapping by any consumer is forbidden (SPEC-PM-009-R1/R2).

**INV-335-G6 (Tier Composition):** Tier content per scale: Months→[Year, Month]; Weeks→[Month, Week]; Days→[Month, Day]; Hours→[Day, Hour]; Minutes→[Hour, Minute]; Seconds→[Minute, Second]. Civil decomposition via SPEC-HAL-334 `to_civil` under the engine's `TimeFrame`.

## 4.3 Working calendar

```rust
pub struct WorkingCalendar {
    pub working_days: [bool; 7],       // index = day_of_week (0 = Monday)
    pub day_start: TimeValue,
    pub day_end: TimeValue,
    pub holidays: Vec<DateValue>,
}
```

**INV-335-G7:** The engine's calendar is the *default* calendar — it drives `non_working` shading spans and `working_time_between(a, b)`. Per-row calendars (shifts, machine downtime, personal PTO) are **data**, expressed by the source as `RowBandDescriptor`s (§6.3). The engine never holds per-row state.

# 5. Identity

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntervalId(pub u64);
```

**INV-335-ID1:** Every interval carries a source-owned, stable `IntervalId`. Selection, focus, intents, and hit results are keyed on `IntervalId` exclusively. Positional indices into any per-frame collection are forbidden as identity (SPEC-PM-001). Reordering, refiltering, or re-feeding descriptors must not disturb identity.

# 6. Data Contract — IntervalSource

Pull-based with invalidation, mirroring the `DataSource` discipline and SPEC-HAL-258. The push-per-frame `Vec` model of SPEC-HAL-282 §4 is retired: it was the outlier against the framework's data-flow law.

```rust
pub trait IntervalSource {
    /// Append descriptors intersecting `range` for the given rows.
    /// `rows = None` in free mode (§7). Over-returning is permitted;
    /// the substrate culls precisely.
    fn collect(&self, rows: Option<&[u64]>, range: TimeRange,
               out: &mut TemporalDescriptors);

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent>;
}

pub enum TemporalInvalidationEvent {
    IntervalsChanged,        // values/geometry of intervals changed
    SetReplaced,             // wholesale replacement; selection pruned to surviving ids
}

pub struct TemporalDescriptors {
    pub intervals: Vec<IntervalDescriptor>,
    pub bands: Vec<RowBandDescriptor>,
    pub arrows: Vec<ArrowDescriptor>,
    pub markers: Vec<MarkerDescriptor>,
}
```

**INV-335-D1 (Source Purity):** Sources hold no pixel knowledge. `TimeRange` is a data-domain query; pixels never cross this trait.

**INV-335-D2 (Culling):** The substrate intersects every descriptor against `visible_range` (half-open, SPEC-HAL-334 INV-334-3) and against the row projection before exposing the frame snapshot. Consumers receive only paint-relevant descriptors.

**INV-335-D3 (Frame-Driven):** Data changes raise invalidation events; they never force a frame (SPEC-HAL-002 data-flow law). `collect` is called during the frame, after deferred mutations.

## 6.1 IntervalDescriptor

```rust
pub struct IntervalDescriptor {
    pub id: IntervalId,
    pub row: u64,                 // ignored in free mode
    pub range: TimeRange,
    pub color: Color,
    pub label: Option<String>,
    pub progress: f32,            // 0.0 – 1.0
    pub style: IntervalStyle,     // Solid | Hatched | Hollow | Milestone
    pub layer: IntervalLayer,     // Baseline | Primary | Preview
    pub lane: Option<u8>,         // Explicit lane (LanePolicy::Explicit)
}
```

`Milestone` is a zero-duration, row-anchored interval. It is distinct from `MarkerDescriptor`, which is row-independent (a date annotation spanning the canvas). They are not alternatives.

## 6.2 ArrowDescriptor

As SPEC-HAL-282 §5 (from/to `IntervalId` endpoints replace row+date pairs):

```rust
pub struct ArrowDescriptor {
    pub from: IntervalId, pub from_edge: IntervalEdge,  // Start | End
    pub to: IntervalId,   pub to_edge: IntervalEdge,
    pub critical: bool,
    pub color: Option<Color>,
}
```

**INV-335-D4 (Hidden Endpoints):** An arrow whose `from` or `to` interval is absent from the current frame snapshot (row collapsed, culled, removed) is dropped by the substrate. Re-routing to visible ancestors is an application-layer concern.

## 6.3 RowBandDescriptor

```rust
pub struct RowBandDescriptor {
    pub row: u64,
    pub range: TimeRange,
    pub color: Color,             // painted beneath intervals
}
```

The carrier for per-row non-working time, capacity strips, shift windows — everything the engine's default calendar cannot and must not know (INV-335-G7).

## 6.4 MarkerDescriptor

As SPEC-HAL-282 §5, unchanged: `{ ts: Timestamp, color, label, style: VerticalLine | Diamond | Flag }`.

# 7. Row Binding

```rust
pub trait RowSource {
    fn visible_row_count(&self) -> usize;
    fn row_y(&self, index: usize) -> f32;
    fn row_height(&self, index: usize) -> f32;
    fn row_id(&self, index: usize) -> u64;
}
```

**Bound mode:** a `RowSource` is attached — canonically a projection of the shared topology substrate (SPEC-HAL-337): row geometry derives from the substrate-published `visible_window()` surface (SPEC-HAL-337 §5, INV-337-R1), fed through this seam by an adapter; timeline widgets never hold the substrate handle (FORBID-337-2). Vertical scroll is owned by the row provider (HTS `ScrollState`); the substrate holds **no** vertical scroll field, mirrored or otherwise.

**Free mode:** no `RowSource`; the substrate presents one implicit row (id 0) spanning the widget height. Lanes (§8) provide internal vertical structure. This is the stage-strip / single-resource manifestation.

**INV-335-R1 (Ephemeral Indices):** `RowSource` indices are frame-local projection indices. Retaining them across frames, or using them as identity, is forbidden (SPEC-HAL-187 INV-ROW-INDEX-EPHEMERAL applies). Identity is `row_id`.

**INV-335-R2 (Row Height Consistency):** Both panels of a bound composition consume the same row geometry from the same provider per frame.

# 8. LaneLayoutEngine

Resolves intra-row overlap: concurrent intervals on one row.

```rust
pub enum LanePolicy {
    /// Substrate packs: deterministic first-fit. Sort by (range.start, id);
    /// assign each interval the lowest lane whose last end <= start.
    Auto,
    /// Application assigns via IntervalDescriptor.lane; substrate clamps
    /// to the row's resolved lane_count. Missing lane => lane 0.
    Explicit,
}
```

**INV-335-L1 (Determinism):** `Auto` packing is a pure function of the culled interval set: identical input, identical lane assignment, every frame. No randomness, no insertion-order dependence.

**INV-335-L2 (Geometry Boundary):** Lanes subdivide the row height the `RowSource` granted: `lane_h = (row_h - 2·margin) / lane_count`. The substrate never grows rows. A provider wanting taller multi-lane rows expresses that through its own row heights (INV-335-R2 covers both panels).

Rationale for dual policy: lane *packing* is pure interval geometry — it belongs in the engine, once, rather than re-implemented per application (reuse doctrine). Lane *meaning* (priority on top, primary assignment first) is application semantics — `Explicit` keeps it expressible. Both behind one policy type, per the policy-type law.

# 9. Selection

**INV-335-S1:** Selection state is `OrderedSelectionState<IntervalId>` — the existing shared engine (verified generic over `ItemKey: Copy + Eq + Hash`). Modes, range semantics, and interaction models are inherited from SPEC-HAL-114/135/136 unchanged. No temporal-specific selection engine exists.

**INV-335-S2:** Selection survives visibility changes (scroll, collapse, cull). On `SetReplaced`, selection is pruned to surviving ids — never cleared wholesale (SPEC-PM-001).

# 10. Intents

All interaction products are deferred intents per SPEC-HAL-177; gating per the SPEC-HAL-200 pattern; provenance per SPEC-HAL-144.

```rust
pub enum TemporalIntent {
    Select { id: IntervalId, modifiers: Modifiers },
    DeselectAll,                                  // empty-canvas press (SPEC-HAL-135 empty-space rule)
    Activate { id: IntervalId },                  // double-click / Enter
    DragMove   { id: IntervalId, new_range: TimeRange },
    DragResize { id: IntervalId, new_end: Timestamp },
    ContextMenu { id: Option<IntervalId>, ts: Timestamp },
    VerticalPanBy { dy: f32 },                    // plain-wheel / hand-tool row pan (SPEC-HAL-336 §7)
    VisibleRangeChanged { range: TimeRange },
}
```

**INV-335-I1:** No synchronous accept/reject callbacks. Widgets enqueue; applications drain in the deferred-mutation phase. Nothing mutates application state during event routing (SPEC-HAL-002 phase law). Preview ghosting during drag is application-driven via `IntervalLayer::Preview` on the next frame's data.

# 11. Shared Time Context

```rust
pub struct TimeContext {
    pub visible_range: TimeRange,
    pub cursor: Option<Timestamp>,   // instant under pointer, if hovering
}
```

The substrate exposes `TimeContext` read access with change notification through the established signal mechanism (SPEC-HAL-031). This is the cross-widget composition primitive: a schedule and a telemetry/vitals/throughput Plot sharing one time window and one cursor instant. A thin adapter to Plot's axis authority is permitted and anticipated; it is non-normative here. Nothing in this SPEC may preclude it.

# 12. Boundary Laws

**FORBID-335-1:** The substrate owns no schedule data, no persistence, no domain model. Projection only.
**FORBID-335-2:** No scheduling semantics — dependencies-as-logic, critical path, leveling, conflict detection are application layer. Arrows are paint descriptors, not constraints.
**FORBID-335-3:** The substrate owns no UI events, gestures, or keyboard handling (mirror of INV-CMS-002). Widgets translate input into engine calls and intents.
**FORBID-335-4:** No widget reaches into engine internals; consumption is `resolve()`, the frame snapshot, selection state, and the intent queue.
**FORBID-335-5:** The substrate is never a `Widget`.

# 13. Non-Goals

- Rendering rules, theming, hit-test geometry — SPEC-HAL-336
- Dependency engine / CPM / resource leveling / what-if simulation — application layer
- Undo/redo — framework-level concern
- Vertical (Y-as-coordinate) plotting — that is Plot (SPEC-HAL-296); this substrate's Y is row identity, never a coordinate

---

# Appendix A — Disposition of SPEC-HAL-282 Review Findings

| # | Finding | Resolution |
|---|---------|-----------|
| 1 | scroll_y mirror vs HTS ownership | §7: vertical scroll owned by row provider; substrate holds no field |
| 2 | origin/scroll_x double bookkeeping | INV-335-G1: single horizontal authority, no scroll_x exists |
| 3 | TimeScale missing sub-hour | §4.1: Seconds…Months |
| 4 | `accents.error` nonexistent | SPEC-HAL-336: `semantic.error` |
| 5 | bars lack identity | §5: IntervalId, INV-335-ID1 |
| 6 | index-based RowSource vs INV-ROW-INDEX-EPHEMERAL | INV-335-R1 |
| 7 | synchronous callbacks vs phase law | §10, INV-335-I1 |
| 8 | Plot relationship unstated | §11 + §13: divergence stated; shared instant via 334; adapter anticipated |
| 9 | time model fragmentation | SPEC-HAL-334 |
| 10 | zoom anchor missing | INV-335-G3 |
| 11 | headers unowned | INV-335-G5 + SPEC-HAL-336 TimelineHeader |
| 12 | arrows to hidden rows undefined | INV-335-D4 |
| 13 | no culling rule | INV-335-D2 |
| 14 | no policy type | LanePolicy (§8) + TemporalCanvasPolicy (336) |
| 15 | label/milestone ambiguity | §6.1 distinction; deterministic label rule in 336 |
| 16 | dashed outline unspecifiable | SPEC-HAL-336 rendering rules |
| 17 | per-row working time | INV-335-G7 + RowBandDescriptor (§6.3) |
| 18 | intra-row overlap | LaneLayoutEngine (§8) |
