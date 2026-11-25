# SPEC-HAL-336: **TimelineHeader and TimelineCanvas**

### *Widget Projections over TemporalSubstrate*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Supersedes:** SPEC-HAL-282 (TimeAxis and TimelineCanvas) — widget layer
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model)
* SPEC-HAL-335 (TemporalSubstrate Architecture)
* SPEC-HAL-338 (Connector Route Geometry) — arrow waypoint authority
* SPEC-HAL-096 (Theming), SPEC-HAL-332 / 332A (Professional Visual Charter)
* SPEC-PM-006 (Rendering Pre-Flight), SPEC-PM-009 (Dual-Source Geometry Drift)

---

# 1. Purpose

Define the first two widget projections over TemporalSubstrate:

- **TimelineHeader** — paints the civil tier rows.
- **TimelineCanvas** — paints row-bound intervals, bands, arrows, and markers; translates input into engine calls and intents.

Both are thin. All time geometry, identity, culling, lanes, selection state, and intent queuing live in the substrate. A Gantt body, a resource board, a machine scheduler, and a single-lane stage strip are this same canvas with different sources and modes — not different widgets.

# 2. Policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemporalCanvasPolicy {
    /// Render + viewport navigation (scroll/zoom) only.
    #[default]
    Display,
    /// + selection and activation intents.
    Select,
    /// + drag-move and drag-resize intents.
    Edit,
}
```

Each tier includes the previous. Viewport navigation is not data interaction and is available in all tiers (as with ScrollArea).

**FORBID-336-1:** In `Display`, the canvas emits no intents and renders no selection or focus affordances.
**FORBID-336-2:** In `Select`, drag-move/resize gestures are inert — no intent, no preview, no cursor change.
**FORBID-336-3:** No tier accepts typed input or owns keyboard editing. Interval mutation is application-side, via intents.

# 3. Shared Resolution Law

**INV-336-1:** TimelineHeader, TimelineCanvas, and all hit-testing consume the **same** `TimeGeometryResolution` instance per frame (INV-335-G5). Neither widget computes tiers, gridlines, mapping, or non-working spans independently. Misalignment between header and canvas is by construction a violation of this invariant, not a styling issue — the direct analogue of SPEC-HAL-164 INV-164-2.

# 4. TimelineHeader

- Paints `resolution.tiers` top to bottom; height = `tier_count × theme.list_geom.row_height` (header tier height tracks list header geometry for the current density).
- Cell text: `typography.secondary`; tier separators and cell boundaries: 1px rules in the list separator color.
- Background: `surfaces.level1`.
- Not focusable, not hit-testable beyond hover (tooltip-eligible), emits nothing.

**FORBID-336-4:** The header never owns or mutates time geometry. It has no zoom or scroll handlers; viewport gestures belong to the canvas (or the composing screen).

# 5. TimelineCanvas — Rendering Rules

**INV-336-R1 (Pipeline Safety):** All painting uses `push_rect`, `push_path` (filled), and the text engine. No `push_rounded_rect`. Per SPEC-PM-006 and the visual charter.

**Paint order (back to front):**
1. Background (`surfaces.base0`)
2. Non-working shading — `resolution.non_working` spans, `surfaces.content` at 50% opacity
3. Row bands (`RowBandDescriptor`s)
4. Gridlines — 1px verticals at `resolution.gridlines`, list separator color
5. Baseline-layer intervals — 30% opacity
6. Primary-layer intervals
7. Arrows
8. Preview-layer intervals — 50% opacity, dashed outline (R6)
9. Markers; now-marker at `resolution.now_x` — `accents.primary`, 2px, full height
10. Selection/focus affordances (Select/Edit tiers)

**INV-336-R2 (Interval Geometry):** For interval `d` on resolved row index `i`, lane `l` of `n`:

```
x = engine.x_of(d.range.start)
w = engine.x_of(d.range.end) - x
lane_h = (row_source.row_height(i) - 2·margin) / n
y = row_source.row_y(i) + margin + l·lane_h
h = lane_h - lane_gap
```

All values from the shared resolution and the frame's row geometry. Hit-testing walks the identical arithmetic (SPEC-PM-009-R3).

**INV-336-R3 (Progress):** `progress > 0.0` paints an inner fill from interval left to `x + w·progress`, same hue darkened per the charter's emphasis rules.

**INV-336-R4 (Label — deterministic):** Measure label at theme font size. If it fits inside `w - 2·text_padding`: centered vertically, left-aligned inside with padding. Otherwise: painted immediately right of the interval, clipped by the viewport. No ellipsis mode, no alternatives.

**INV-336-R5 (Milestone):** `IntervalStyle::Milestone` paints a filled diamond via `push_path`, side = lane content height, centered on `x_of(range.start)`. Zero-duration; width rules do not apply.

**INV-336-R6 (Preview Dashing):** Preview outline = four 1px border strips composed of segment rects, dash 4px, gap 3px, phase starting at each edge's origin. Deterministic; no path stroking.

**INV-336-R7 (Hatched):** `IntervalStyle::Hatched` fills with 45° stripe geometry pre-tessellated as `push_path` parallelograms clipped to the interval rect, 4px pitch. (Reference realization: the PTO treatment in professional schedulers.)

**INV-336-R8 (Arrows):** Waypoints per SPEC-HAL-338 (INV-338-G1 right-angle plan); the canvas computes no route geometry. Canvas-owned translation: exit/enter per `IntervalEdge` — directed endpoint at the bar edge's vertical center, `Start` → outward `Left`, `End` → outward `Right`. Canvas-owned realization: plan segments as segment rects, 1px; arrowhead = filled path triangle at the plan's head anchor and direction. `critical: true` → 2px, `semantic.error`. Endpoint geometry from the same frame snapshot — arrows with missing endpoints never reach the canvas (INV-335-D4; discharges INV-338-P3).

**FORBID-336-5:** No hardcoded colors. Every color above resolves through theme tokens; descriptor `color` fields are application-supplied values passed through unmodified.

# 6. Hit-Testing

**INV-336-H1:** Hit results are identity: `Option<IntervalId>` (plus `Timestamp` of the point). Lane and row resolution use the same per-frame geometry as paint — no fixed-height approximation, no independent walk (SPEC-PM-009-R3). Empty-canvas click yields `(None, ts)`.

# 7. Interaction

| Input | Tier | Effect |
|---|---|---|
| Wheel, no modifiers | Display+ | zoom about the cursor: `engine.zoom_at(cursor_x, factor)` — wheel up zooms in, down zooms out (INV-335-G3 anchor law); a pure-horizontal wheel with no modifier is ignored |
| Shift+wheel | Display+ | pan the time axis: `engine.scroll_by_px(delta_y - delta_x)` — down or left advances forward (later), up or right moves backward (INV-335-G4) |
| Space held + primary drag | Display+ | hand tool, two-axis pan, content follows the pointer: `dx` → `engine.scroll_by_px(-dx)`; `dy` → `VerticalPanBy { dy: -dy }` (bound mode); suppresses selection and Edit-drag arms while active |
| Primary press on interval | Select+ | `TemporalIntent::Select { id, modifiers }`; selection semantics per SPEC-HAL-135 (commit on press) |
| Double-click / Enter on focused | Select+ | `TemporalIntent::Activate` |
| Primary press on empty | Select+ | `TemporalIntent::DeselectAll`; empty-space rule per SPEC-HAL-135 (commit on press) |
| Secondary press | Select+ | `TemporalIntent::ContextMenu { id, ts }` (on press, not release) |
| Drag interval body | Edit | `DragMove` intent stream; ghost = application-fed Preview layer |
| Drag interval end edge (4px grip) | Edit | `DragResize` intent stream |

`VerticalPanBy { dy }` is a deferred product: the canvas never scrolls rows itself
(FORBID-337-2); the composition applies `dy` through the row provider's single
vertical-scroll path (INV-337-S1 / FORBID-337-4). Sign convention matches
ScrollArea: positive `dy` advances `scroll_y`. Keyboard: Space is the pan
modifier and is unbound from `ToggleFocused`; toggling remains available via
Cmd/Ctrl+click. Enter remains Activate.

**INV-336-I1:** Every row in this table terminates in an engine call or an enqueued intent. The canvas mutates no application state, ever (INV-335-I1).

**INV-336-I2 (Cursor Context):** While hovering, the canvas publishes `cursor: Some(instant_at(x))` into the substrate's `TimeContext`; `None` on exit.

## 7.1 SPEC-HAL-343 Spatial Interaction Policy

TimelineCanvas consumes `SpatialInteractionPolicy` only for domain-neutral pan
gesture classification. Its explicit policy table is:

| Policy entry | TimelineCanvas value | Authority |
|---|---|---|
| `pan_gesture` | `SpacePrimaryDrag` | Horizontal pan calls `TimeGeometryEngine`; vertical pan emits `VerticalPanBy`. |
| `alternate_pan_gesture` | `Disabled` | Middle drag is not a Timeline pan gesture. |
| `rubber_band_gesture` | `Disabled` | Timeline selection commits on press through `TemporalSubstrate`; no shared rubber-band selection. |
| `route_creation_gesture` | `Disabled` | Arrows are projected from temporal descriptors; users do not create routes through the spatial layer. |
| `delete_keys_enabled` | `false` | Deletion is application/temporal command authority, not a spatial key command. |
| `fit_key` / `reset_key` | `None` / `None` | Timeline exposes no shared spatial fit/reset key command. |

# 8. Composition

**Gantt / resource board (bound mode):**

```
Screen mints SharedHts (SPEC-HAL-337 §2)
SplitPane
├── Left: TreeView::with_hts(handle)  — HtsOwnership::External; task/resource columns
└── Right: Flex column
     ├── TimelineHeader               — same TemporalSubstrate (INV-336-1)
     └── TimelineCanvas               — same TemporalSubstrate; rows via hts_rows(&handle)
```

Exactly one topology substrate exists per composition: the screen owns the
`SharedHts` handle and the TreeView projects it (INV-337-O1). Timeline
widgets never hold `SharedHts` (FORBID-337-2): the sanctioned boundary is
the engine-layer adapter `hts_rows(&SharedHts) -> Vec<RowGeometry>`
(SPEC-HAL-337 §5), called once per frame by the composition and fed into
`TimelineCanvas::set_row_geometry` — the SPEC-HAL-335 §7 seam.
`RowGeometry.y` is viewport-space; bound mode is defined for
fixed-row-height substrates (INV-HTSROWS-1).

Bound-mode selection is unified at row level: the composition maps
`TemporalIntent::Select`'s interval to its `IntervalDescriptor.row` and
reduces the tree's row selection; interval-level detail remains
canvas-local. The canvas's own selection layer mirrors the row selection —
it never becomes a second authority.

Vertical scroll: HTS-owned, both panels follow (INV-335-R2). Horizontal: TimeGeometryEngine-owned, header and canvas follow (INV-336-1).

**Stage strip (free mode):** standalone `TimelineHeader + TimelineCanvas` with an empty row-geometry set — the §7 seam's free-mode definition; lanes for internal structure. Process phases, pipeline stages, session timelines.

**Cross-view sync:** any other widget (a Plot via adapter, a second canvas in another pane or window) observes `TimeContext` per SPEC-HAL-335 §11.

# 9. Acceptance Criteria

- Header and canvas pixel-aligned at every scale, zoom, and scroll position (INV-336-1 holds under property test)
- Zoom keeps the instant under the cursor fixed
- Intervals, lanes, bands, arrows, markers, milestones render per §5 at Compact/Regular/Comfortable densities
- Selection keyed on IntervalId survives scroll, collapse, and re-feed
- Hit-test parity: paint position N ↔ hit at N (INV-GEOM-PARITY)
- Bound mode: expand/collapse in TreeView updates canvas same frame; both panels share row geometry
- Free mode renders without a RowSource
- All three policy tiers enforce their FORBIDs

# 10. Framework Input Authority

Keyboard authority and Escape semantics remain governed by SPEC-HAL-328.
This contract adds no consumer-specific input override.

# 11. Non-Goals

- Dependency logic, critical-path computation, leveling — application layer feeding descriptors
- Printing/export pagination, what-if simulation engines — future SPECs
- Inline label editing — interval mutation is application-side via intents
