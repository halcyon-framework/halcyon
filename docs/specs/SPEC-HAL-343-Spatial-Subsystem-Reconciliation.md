# SPEC-HAL-343: Spatial Subsystem Reconciliation

### Plot, Timeline, Infinite Canvas, and Node Graph Authority Boundaries

**Author:** JC
**Status:** Adopted
**Depends on:**

* SPEC-HAL-002 (Constitution and Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-293 (Node Graph Model)
* SPEC-HAL-294 (Node Flow Editor)
* SPEC-HAL-296 (Plot)
* SPEC-HAL-335 (TemporalSubstrate Architecture)
* SPEC-HAL-336 (TimelineHeader and TimelineCanvas)
* SPEC-HAL-338 (Connector Route Geometry)
* SPEC-HAL-261 (Engine Boundary and Ownership Law)
* SPEC-PM-006 (Rendering Pre-Flight Checklist)
* SPEC-PM-009 (Dual-Source Geometry Drift)

---

# 1. Purpose

Halcyon currently contains multiple spatially interactive subsystems:

* Plot and candle charts
* Timeline and temporal canvas
* Infinite canvas
* Node graph and node flow editor

They overlap in viewport transforms, grid rendering, hit testing, selection,
dragging, guide rendering, route geometry, cursor policy, and visual overlay
behavior.

This SPEC records the discovery process, identifies the architectural problem,
rejects false solutions, and defines the target reconciliation architecture.

The purpose is not to merge all domain widgets into one widget. The purpose is
to establish a shared spatial interaction substrate below domain widgets, while
preserving each domain's semantic authority.

---

# 2. Discovery Record

This SPEC was created after inspection of the real repository state.

## 2.1 Plot

Observed files:

* `src/plot/mod.rs`
* `docs/specs/SPEC-HAL-296-Plot.md`
* `docs/specs/SPEC-HAL-297-Plot-Tool-System.md`
* `docs/specs/SPEC-HAL-299-Plot-Price-Levels.md`

Observed structure:

* `Plot` is a full widget.
* It owns chart-specific axes, transforms, tools, probes, price levels, series,
  GPU rendering paths, hit testing, and interaction handling.
* It is built for data visualization and trading-grade chart behavior.

Conclusion:

Plot is not a general spatial substrate. It is a domain widget over numerical
and temporal data.

## 2.2 Timeline

Observed files:

* `src/widgets/timeline_canvas.rs`
* `docs/specs/SPEC-HAL-335-TemporalSubstrate-Architecture.md`
* `docs/specs/SPEC-HAL-336-TimelineHeader-and-TimelineCanvas.md`
* `docs/specs/SPEC-HAL-338-Connector-Route-Geometry.md`

Observed structure:

* Timeline has its own paint rules, hit testing, drag state, row/lane geometry,
  time mapping, arrow rendering, and policy modes.
* SPEC-HAL-336 already states that TimelineHeader and TimelineCanvas should be
  thin projections over TemporalSubstrate.
* The implementation still contains canvas-local logic that should ultimately
  be substrate-fed or shared through a route/interaction layer.

Conclusion:

Timeline is partly aligned with a substrate architecture, but it still carries
  local spatial-interaction mechanics that overlap with plot and node graph.

## 2.3 Infinite Canvas

Observed files:

* `src/model/canvas.rs`
* `docs/specs/SPEC-HAL-284-Infinite-Canvas.md`
* `docs/specs/SPEC-HAL-285-Canvas-Item-Protocol.md`

Observed structure:

* InfiniteCanvas defines world/screen conversion, viewport state, grid
  configuration, zoom, pan, hit regions, and read-only projection records.
* It explicitly forbids owning domain objects, selection, document state, or
  mutation authority.

Conclusion:

InfiniteCanvas is the correct existing foundation for non-axis spatial
surfaces. It is not yet sufficient as the complete shared interaction layer for
plot, timeline, and node graph.

## 2.4 Node Graph

Observed files:

* `src/model/node_graph.rs`
* `src/model/node_graph_projection.rs`
* `src/model/node_editor.rs`
* `examples/showcase/tab_node_graph.rs`
* `examples/node_graph.rs`
* `docs/specs/SPEC-HAL-293-Node-Graph-Model.md`
* `docs/specs/SPEC-HAL-294-Node-Flow-Editor.md`

Observed structure:

* NodeGraphDocument is a document authority.
* NodeEditorSession is ephemeral editor authority.
* NodeGraphProjection derives nodes, ports, wires, frames, hit regions, and
  visual states.
* NodeFlowView uses InfiniteCanvas and CanvasProjection.
* The showcase demo still hand-paints several behaviors directly, including
  grid, HUD, theme palette, wires, nodes, ports, and alignment guides.

Conclusion:

The node graph direction is structurally correct: it is not built on Plot and
does not own domain state inside the canvas. The weak point is duplicated
interaction/rendering realization in the demo and in adjacent subsystems.

---

# 3. Problem Statement

Halcyon has parallel spatial systems with overlapping responsibilities.

The problem is not that node graph did not reuse Plot. Reusing Plot would be
wrong. The problem is that Plot, Timeline, and Node Graph each carry portions
of a spatial interaction stack that should be shared below them.

## 3.1 Concrete Duplications

The following concerns appear in more than one subsystem:

* Viewport transform and fit-to-content
* Grid painting and pixel-stable grid lines
* Rubber-band selection
* Hit testing by geometric primitive
* Selection state projection
* Move/resize drag sessions
* Cursor policy during hover vs active drag
* Alignment guides and structural guides
* Connector/route geometry
* HUD/status overlays
* Theme-sensitive canvas palettes
* Deletion/duplicate/nudge action binding

## 3.2 Risks

**RISK-343-1: Interaction Drift**
The same user gesture can acquire different semantics across plot, timeline,
and node graph.

**RISK-343-2: Visual Drift**
Grid, guides, wires, arrows, overlays, and handles can diverge in color,
weight, padding, and density.

**RISK-343-3: Geometry Drift**
Paint geometry and hit-test geometry can diverge when each subsystem computes
layout independently.

**RISK-343-4: Authority Drift**
Local widgets can accidentally become authorities for state that belongs to
documents, sessions, engines, or application composition.

**RISK-343-5: Migration Debt**
Every new spatial feature implemented in only one subsystem multiplies future
porting work.

---

# 4. Rejected Solutions

## 4.1 Make Plot the Base

Rejected.

Plot owns domain concerns: axes, series, candles, price levels, probes, chart
tools, and chart-specific rendering. Node graph and timeline should not inherit
those concepts.

**FORBID-343-1:** Node graph and timeline MUST NOT be built on Plot.

## 4.2 Make Timeline the Base

Rejected.

Timeline owns temporal semantics, lane resolution, row correspondence, and
time-axis behavior. Node graph is not temporal, and plot has axis semantics
that are not reducible to timeline rows.

**FORBID-343-2:** Node graph and plot MUST NOT be built on TimelineCanvas or
TemporalSubstrate.

## 4.3 Make InfiniteCanvas Own Domain Behavior

Rejected.

InfiniteCanvas is a coordinate substrate. It must not own graph documents,
timeline intervals, plot series, selection authority, delete commands, or
application mutation.

**FORBID-343-3:** InfiniteCanvas MUST NOT become a domain widget.

## 4.4 Leave Systems Disconnected

Rejected.

Disconnected systems preserve short-term freedom but guarantee long-term drift.

**FORBID-343-4:** New spatial interaction features MUST NOT be implemented only
inside one domain subsystem when the behavior is domain-agnostic.

---

# 5. Target Architecture

The shared base is a spatial interaction substrate below domain widgets.

Layer order:

1. Renderer and SceneGraph primitives
2. Spatial substrate
3. Domain projection adapters
4. Domain widgets and composition surfaces
5. Application/document authorities

## 5.1 Spatial Substrate

The spatial substrate owns no domain data. It provides reusable mechanics:

* Neutral surface-space to viewport-space transform
* Viewport state helpers
* Fit-to-content
* Grid realization
* Geometric hit testing
* Rubber-band geometry
* Common cursor policy
* Common guide rendering
* Common handle/anchor/attachment-point visual grammar
* Common connector route plans and realization hooks
* Common HUD/overlay placement rules

Existing InfiniteCanvas and CanvasProjection are the starting point. They are
not replaced. They are extended or complemented only where shared behavior is
domain-agnostic.

This SPEC is lossless over the existing canvas model. Existing `Canvas*` names
and behavior remain valid compatibility surface until a migration proves parity
and the dependent canvas SPECS are amended.

**INV-343-S1: Spatial Substrate Is Domain-Free**
The spatial substrate may know about world geometry, projected IDs, hit regions,
routes, guides, and viewport transforms. It MUST NOT know about candles, price
levels, intervals, tasks, nodes, ports, wires, frames, or document transactions.

**INV-343-S2: Domain Authority Remains External**
All persistent mutation remains in the domain authority:

* Plot data and chart objects remain plot/application authority.
* Timeline data remains TemporalSubstrate/application authority.
* Node graph data remains NodeGraphDocument authority.

**INV-343-S3: Session Authority Remains External**
Selection, active drag, hovered entity, clipboard, search overlays, and inline
edit drafts remain in the domain session/application layer unless a later SPEC
defines a domain-neutral session protocol.

**INV-343-S4: Coordinate Authority Is Split**
The spatial substrate may own neutral affine viewport mechanics. It MUST NOT own
domain coordinate transforms such as plot data-to-axis mapping or temporal
time-to-x mapping. Domains convert semantic coordinates into neutral projected
geometry before handing data to the spatial substrate.

## 5.2 Professional Spatial API

The shared substrate API is a domain-neutral projection and interaction API.
It lives under `src/model/spatial/`. Existing `src/model/canvas.rs` types may
be migrated, wrapped, or re-exported only if the resulting public API obeys this
section.

### 5.2.1 API Names

The public API must use these names:

| API | Role |
|---|---|
| `SpatialViewportState` | Durable domain-free camera state for affine spatial surfaces. |
| `SpatialAxisOrientation` | Explicit signed axis orientation for an affine spatial surface. |
| `SpatialTransform` | Ephemeral checked surface-space to pixel-space conversion. |
| `SpatialProjection` | Disposable frame-local projected geometry. |
| `SpatialProjectionRevision` | Revision value for projection invalidation and parity checks. |
| `SpatialItem` | One projected primitive or compound primitive. |
| `SpatialItemId` | Opaque projected identity. |
| `SpatialItemRole` | Domain-neutral visual and interaction role. |
| `SpatialPrimitive` | Projected geometry primitive. |
| `SpatialHitRegion` | Geometry used for pointer hit testing. |
| `SpatialRoutePlan` | Domain-neutral connector/arrow/route geometry. |
| `SpatialGuidePlan` | Active and structural guide geometry. |
| `SpatialOverlayPlan` | HUD and non-domain overlay placement. |
| `SpatialVisualPolicy` | Theme-resolved spatial visual tokens. |
| `SpatialInteractionPolicy` | Gesture and cursor policy table. |
| `SpatialIntent` | Domain-neutral interaction event emitted by the substrate. |
| `SpatialAdapter` | Pure adapter from domain snapshot/session snapshot to projection and back to domain intent. |

The current `CanvasTransform`, `CanvasViewportState`, `CanvasProjection`,
`CanvasEntityId`, `CanvasHitRegion`, `CanvasPaintDescriptor`, and
`CanvasIntent` concepts are not discarded. They are the compatibility source
for the `Spatial*` API.

`CanvasEntityId` remains the compatibility/source identity for existing canvas
consumers. `SpatialItemId` is the opaque adapter-facing identity for the new
API. A migration may wrap or translate `CanvasEntityId`; it must not delete,
rename, or flatten existing `CanvasEntityId` variants until parity tests pass
and SPEC-HAL-284 / SPEC-HAL-285 are explicitly amended.

### 5.2.2 Affine Viewport Contract

This section governs `AffineSurface` only. Plot axes, Plot series transforms,
and Timeline time-to-x mapping remain domain-owned `DomainMappedSurface`
behavior and are not changed by this contract.

`SpatialViewportState` is the durable camera authority supplied by the owning
session or application. Its semantic state is exactly:

| Field | Contract |
|---|---|
| `world_center` | Finite world-space point at the center of the viewport. |
| `pixels_per_world_unit` | Finite, strictly positive scalar applied with equal magnitude on both axes. |

It contains no `SpatialTransform`, pixel rectangle, axis orientation, visible
bounds, global world bounds, minimum or maximum span, or zoom-clamp policy.
Consumer-specific limits, including the existing InfiniteCanvas zoom range,
remain consumer policy outside the neutral camera state.

**INV-343-AV1: Camera State Is Durable and Domain-Free**
Pan, zoom, and fit operations produce a new `SpatialViewportState`; they never
make an ephemeral transform authoritative. The state MUST NOT contain domain
coordinates, layout geometry, render-target geometry, or consumer policy.

`SpatialAxisOrientation` is an explicit closed axis-aligned signed basis. For
each world axis it states whether increasing world values map toward increasing
or decreasing pixel coordinates:

| Direction | Sign in the affine mapping |
|---|---|
| `IncreasingPixels` | `+1` |
| `DecreasingPixels` | `-1` |

Every affine surface supplies both X and Y directions explicitly through its
surface or adapter configuration. Orientation is not mutable camera state.
Existing InfiniteCanvas and NodeGraph compatibility consumers use
`IncreasingPixels` for both axes. A Cartesian surface uses
`IncreasingPixels` for X and `DecreasingPixels` for Y.

**INV-343-AV2: Orientation Is Explicit and Axis-Aligned**
The neutral substrate has no implicit y-up or y-down convention. Axis
orientation permits sign inversion only. It MUST NOT permit axis swapping,
rotation, shear, perspective, or arbitrary matrix coefficients.

`SpatialTransform` is an immutable frame-local mapping materialized from one
validated `SpatialViewportState`, one explicit `SpatialAxisOrientation`, and
one caller-owned `RectF`. The rectangle is expressed in the same pixel-space
coordinate system used for paint and pointer input. The transform captures the
validated rectangle snapshot so forward mapping, inverse mapping, visible
bounds, culling, and grid realization use the same target geometry.
Here, pixel-space means the logical screen/layout coordinates carried by
`RectF` and described by SPEC-HAL-284 INV-284-C3. Physical-pixel and device-scale
conversion remain outside the spatial substrate.

For scale `s = pixels_per_world_unit`, orientation signs `sx` and `sy`, world
center `(cx, cy)`, and rectangle center `(pxc, pyc)`, the mapping is:

```text
pixel_x = pxc + sx * (world_x - cx) * s
pixel_y = pyc + sy * (world_y - cy) * s

world_x = cx + (pixel_x - pxc) / (sx * s)
world_y = cy + (pixel_y - pyc) / (sy * s)
```

**INV-343-AV3: Transform Is Ephemeral and Target-Complete**
`SpatialTransform` is created for the current target rectangle and is never
stored inside `SpatialViewportState` or mutated as interaction state. Mapping
methods use the captured rectangle; they MUST NOT accept a second rectangle
that could disagree with the validated target.

The visible world bounds are derived from the state and captured rectangle:

```text
half_world_width  = rect.width  / (2 * pixels_per_world_unit)
half_world_height = rect.height / (2 * pixels_per_world_unit)
```

The effective X and Y spans may therefore differ with rectangle aspect ratio,
but the per-unit scale magnitude remains equal. Fit-to-content may derive one
new center and one new scalar scale from finite content bounds, target
rectangle, and explicit padding; it MUST NOT introduce anisotropic camera
scale.

**INV-343-AV4: Pan and Anchored Zoom Change Camera State**
Pan translates `world_center` by an explicit world-space delta. Anchored zoom
holds the target rectangle and axis orientation fixed. For finite positive zoom
factor `f`, old scale `s`, anchor world point `(ax, ay)`, its old pixel position
`(apx, apy)`, rectangle center `(pxc, pyc)`, and orientation signs `sx` and
`sy`, the new state is uniquely determined by:

```text
new_scale    = s * f
new_center.x = ax - (apx - pxc) / (sx * new_scale)
new_center.y = ay - (apy - pyc) / (sy * new_scale)
```

For cursor-anchored zoom, the old transform maps the cursor pixel to the anchor
world point and `(apx, apy)` is the cursor pixel. For an explicit world anchor,
the old transform supplies `(apx, apy)`. The old transform establishes this
mapping only; it is not mutated.

**INV-343-AV5: Invalid Affine State Fails Explicitly**
Construction and affine operations reject a non-finite center, scale,
rectangle, zoom factor, input, or result; a non-positive scale or factor; and a
rectangle with non-positive width or height. Arithmetic overflow is failure.
The spatial substrate MUST NOT repair invalid state through silent clamping,
invent global world bounds, or introduce speculative minimum or maximum span
policy.

World coordinates and scale arithmetic remain `f64` through normalization and
mapping, with conversion to `f32` only at the pixel boundary. This numerical
representation contract does not transfer evaluator tolerance, path
subdivision, tessellation, or domain precision policy into the spatial
substrate.

**INV-343-AV6: Canvas Compatibility Is Rect-Aware**
The compatibility bridge for existing Canvas and NodeGraph consumers preserves
their y-down, uniform-scale behavior through the explicit
`IncreasingPixels`/`IncreasingPixels` orientation. Conversion between legacy
origin-anchored `CanvasViewportState` and center-anchored
`SpatialViewportState` is valid only with the caller-owned pixel rectangle and
the checked rules in SPEC-HAL-284. Draft representation-level `From`
conversions are not compatibility authority.

### 5.2.3 Identity

`SpatialItemId` is opaque. It may carry stable equality and hashing, but it
must not expose domain nouns or domain storage keys in the spatial API.

Required identity properties:

* Stable within a projection revision.
* Stable across revisions when the same domain object projects to the same
  spatial item.
* Distinct for separate spatial roles of the same domain object when hit
  semantics differ.
* Non-authoritative: it identifies projection output only, not document state.

**FORBID-343-5:** Spatial identity MUST NOT expose variants named after domain
objects such as node, port, frame, interval, candle, order, task, or lane.

### 5.2.4 Roles

`SpatialItemRole` defines neutral roles only:

| Role | Meaning |
|---|---|
| `Surface` | Main spatial content area. |
| `Item` | Primary movable/selectable visual object. |
| `Container` | Grouping or containing region. |
| `Anchor` | Connection, resize, or attachment point. |
| `Handle` | Direct-manipulation handle. |
| `Route` | Connector, arrow, wire, dependency, or path. |
| `Guide` | Alignment, snap, or structural guide. |
| `Label` | Text-bearing projected area. |
| `Overlay` | HUD, status, tooltip, or non-domain panel. |
| `Background` | Grid, axis backdrop, or surface fill. |

Domain-specific display names may be stored only in adapter-owned metadata or
paint text, not in role names.

### 5.2.5 Primitives

`SpatialPrimitive` contains projected geometry only:

| Primitive | Data |
|---|---|
| `Rect` | Axis-aligned projected rectangle. |
| `RoundedRect` | Projected rectangle plus radius token. |
| `Circle` | Center and radius. |
| `Line` | Segment endpoints and stroke role. |
| `Path` | Ordered points or curve controls. |
| `Route` | `SpatialRoutePlan`. |
| `Guide` | `SpatialGuidePlan`. |
| `TextBounds` | Text layout bounds supplied by the domain or text system. |
| `Overlay` | `SpatialOverlayPlan`. |

The primitive set is geometric. It does not contain document objects.

`SpatialRoutePlan` consumes route geometry produced under SPEC-HAL-338. The
spatial API may carry and realize route plans, but it is not the authority for
route math.

### 5.2.6 Projection

`SpatialProjection` is the only object handed from a domain adapter to the
substrate for paint and hit testing of domain-neutral spatial primitives.
Domain-owned render paths that are not spatial primitives remain outside this
projection.

Required fields:

| Field | Meaning |
|---|---|
| `revision` | Projection revision. |
| `viewport` | Optional `SpatialViewportState`; required for an affine surface and absent for a domain-mapped surface. |
| `axis_orientation` | Optional `SpatialAxisOrientation`; required with an affine viewport and absent for a domain-mapped surface. |
| `coordinate_mode` | `AffineSurface` or `DomainMappedSurface`. |
| `items` | Ordered projected items. |
| `hit_regions` | Hit-test geometry keyed by `SpatialItemId`. |
| `routes` | Route plans keyed by `SpatialItemId`. |
| `guides` | Active and structural guide plans. |
| `overlays` | HUD/status/tooltip placement plans. |
| `visual_policy` | Resolved spatial visual policy. |

`AffineSurface` means the substrate owns neutral pan/zoom/fit mechanics and the
projection supplies both `viewport` and `axis_orientation`. `DomainMappedSurface`
means a domain engine owns the semantic coordinate map and the substrate
receives already projected geometry; it does not reinterpret that geometry
through the affine viewport contract and supplies neither field.

**INV-343-P0: Projection Has No Mutation Channel**
`SpatialProjection` is read-only. It cannot contain closures, callbacks, mutable
authority references, or transaction handles.

**INV-343-P0A: Coordinate Mode Admission Is Closed**
`SpatialProjection` construction or admission at the spatial-substrate boundary
MUST accept exactly two viewport combinations:

* `AffineSurface`: `viewport` and `axis_orientation` are both present.
* `DomainMappedSurface`: `viewport` and `axis_orientation` are both absent.

Every other combination is invalid and must fail explicitly. An adapter MUST
emit one admitted combination; neither the projection nor the substrate may
silently ignore a surplus field.

### 5.2.7 Adapter Boundary

`SpatialAdapter` is a pure boundary. It does two jobs:

1. Build `SpatialProjection` from a domain snapshot and a session snapshot.
2. Translate `SpatialIntent` into domain-owned intents.

It does not apply mutations.

Required adapter shape:

| Function | Contract |
|---|---|
| `build_projection` | Pure projection from domain snapshot, session snapshot, optional affine viewport snapshot, optional affine axis-orientation configuration, and policy input. |
| `translate_intent` | Pure translation from spatial intent plus adapter context into domain intent values. |

The adapter receives immutable inputs. Domain widgets, sessions, documents, or
application controllers apply the translated domain intents.
An affine adapter receives both its viewport and axis orientation explicitly at
projection construction. A domain-mapped adapter receives neither. Adapters
MUST pass those inputs through closed coordinate-mode admission and MUST NOT
infer orientation or silently discard surplus affine input.

**INV-343-A1: Adapter Is Not Authority**
`SpatialAdapter` MUST NOT mutate a document, session, engine, or application
state.

**INV-343-A2: Domain Intent Is Opaque to Spatial**
The spatial substrate may transport adapter-produced domain intents, but it MUST
NOT inspect or execute their semantic contents.

### 5.2.8 Interaction Intent

`SpatialIntent` contains only domain-neutral interaction facts:

| Intent | Meaning |
|---|---|
| `ViewportChanged` | Pan, zoom, or fit produced replacement viewport state for the external owner. |
| `HoverChanged` | Pointer hover target changed. |
| `Press` | Pointer press on item, hit region, or empty surface. |
| `DragStarted` | Drag gesture crossed activation threshold. |
| `DragUpdated` | Active drag moved in surface coordinates. |
| `DragCommitted` | Active drag ended successfully. |
| `DragCanceled` | Active drag canceled. |
| `RubberBandChanged` | Rubber-band rectangle changed. |
| `RubberBandCommitted` | Rubber-band gesture committed. |
| `KeyCommand` | Shared spatial key command was invoked. |

`SpatialIntent` may include modifiers, pointer button, hit item id, local
surface position, screen position, and drag delta. It MUST NOT include domain
commands such as delete-node, move-interval, edit-price-level, or remove-wire.

### 5.2.9 Visual Policy

`SpatialVisualPolicy` resolves common spatial roles:

* Canvas background
* Grid minor and major lines
* Active guide
* Structural guide
* Selection outline
* Drag preview
* Handle/anchor fill and stroke
* Route default, selected, hovered, blocked, and preview strokes
* HUD background, border, text, and edge padding

Domains may provide role accents. They must not define independent canvas
palettes for shared roles.

### 5.2.10 Interaction Policy

`SpatialInteractionPolicy` resolves:

* Pan activation gesture
* Zoom or scroll behavior
* Rubber-band gesture
* Selection toggle gesture
* Drag cursor transition
* Fit/reset key bindings
* Delete key eligibility
* Route creation gesture class

Policies are explicit tables. Domain widgets may override entries only by
declaring the override in their SPEC.

## 5.3 Domain Projection Adapters

Each domain projects into shared spatial primitives without leaking domain nouns
into the substrate API:

* Plot projects chart hit targets, annotations, tools, and overlays through a
  plot adapter.
* Timeline projects intervals, markers, arrows, rows, and lanes through a
  temporal adapter.
* Node graph projects nodes, ports, wires, frames, and guides through a node
  graph adapter.

**INV-343-P1: Projection Is One-Way**
Authority -> projection -> paint/hit. No projection may write back to authority.

**INV-343-P2: Adapter Output Is Disposable**
Adapters produce frame-local projection data. They are caches only if identical
to a fresh rebuild.

**INV-343-P3: Labels Are Spatial Constraints**
Any adapter that projects text-bearing items must also project label bounds or
reserved label regions. Shared layout, fit, alignment, guide, and route logic
must treat those bounds as occupied space unless a domain SPEC explicitly marks
the label as decorative.

**INV-343-P4: Label Measurement Authority Is External**
Measured label bounds are supplied by the domain adapter from the existing text,
layout, or domain measurement authority. The spatial substrate MUST NOT invent
or remeasure domain labels.

## 5.4 Shared Interaction Grammar

The following gestures must be defined once and reused:

| Gesture | Shared meaning |
|---|---|
| Space + primary drag | Pan spatial surface |
| Wheel over spatial surface | Domain-selected zoom or scroll policy |
| Primary press on projected item | Select/activate per domain policy |
| Shift + primary drag on empty space | Rubber-band select |
| Delete / Backspace | Delete selected deletable entities |
| F / Home | Fit or reset viewport per surface policy |
| Cmd/Ctrl + click | Toggle selection where selection is supported |

Domain widgets may override only through an explicit policy table.

**INV-343-I1: Cursor Policy Is Shared**
Hover over interactive content uses the default cursor unless the domain has a
standard text, resize, or link affordance. Move/crosshair/drag cursors are
active-drag affordances, not passive hover affordances.

**INV-343-I2: Gesture Tables Are Binding**
Every spatial widget must document its deviations from the shared interaction
grammar in its SPEC. Silent divergence is invalid.

## 5.5 Shared Visual Grammar

The following visual classes must resolve through shared tokens or shared
realization rules:

* Canvas background
* Grid minor/major lines
* Selection outline
* Drag preview
* Resize handles
* Anchor/port handles
* Alignment guides
* Connector lines/arrows
* HUD/status overlays

**INV-343-V1: Theme Branches Are Centralized**
Light/dark canvas palettes must not be hand-tuned independently in each demo.
A domain may pass semantic role colors, but substrate classes resolve through a
shared visual policy.

**INV-343-V2: Guides Are Not Grid**
Alignment and structural guides must not share the visual identity of grid
lines. They are interaction overlays with separate role colors and opacity.

**INV-343-V3: Active vs Passive Overlay**
An active snap/drag guide and a passive structural guide are different visual
roles. They must not be collapsed into one color/thickness rule.

**INV-343-V4: Overlay Edge Padding Is Viewport-Relative**
HUD and status overlays are positioned in viewport space with explicit edge
padding. They MUST NOT inherit world transform scale, grid origin, or content
layout offsets.

---

# 6. Actual Solution

The actual solution is a staged extraction, not a rewrite.

## 6.1 Keep Domain Models

No domain model is replaced.

* Keep Plot as chart domain widget.
* Keep TemporalSubstrate as temporal authority.
* Keep NodeGraphDocument and NodeEditorSession as node graph authorities.
* Keep InfiniteCanvas and CanvasProjection as the spatial substrate base.

## 6.2 Extract Shared Spatial Policies

Create `src/model/spatial/` and consolidate shared policy modules for:

* Spatial viewport behavior
* Spatial grid realization
* Spatial guide realization
* Spatial connector route realization over SPEC-HAL-338 route plans
* Spatial handle/anchor styling
* Spatial HUD placement
* Spatial cursor policy

These policies operate on geometry and semantic roles only.

`src/model/spatial/` owns spatial projection, interaction policy, visual policy,
and realization policy. It does not supersede `src/engines/route_geometry/` or
any SPEC-HAL-338 route computation authority.

## 6.3 Convert Domain Widgets Incrementally

Migration order:

1. Define the `Spatial*` API and add a compatibility bridge from existing
   `Canvas*` projection concepts without deleting existing `Canvas*` behavior.
2. Node graph demo: remove demo-local duplicate grid/HUD/guide/palette rules
   after shared policies exist.
3. Timeline canvas: move only domain-neutral route geometry, cursor policy, and
   drag mechanics to shared spatial policies while preserving TemporalSubstrate
   authority, row/lane authority, policy gates, and engine calls.
4. Plot: adapt non-axis overlays, chart annotations, connector markers, and
   selection/hit affordances where they match spatial policy. Do not migrate
   plot axes, series rendering, probes, or chart-specific tools into the shared
   layer.

## 6.4 Preserve Output During Migration

Every extraction must be behavior-preserving unless the SPEC for that subsystem
explicitly changes behavior.

**INV-343-M1: No Visual Regression Extraction**
An extraction is valid only if the original and extracted path produce equivalent
geometry, hit-test results, and scene output for the same inputs, except for
explicitly authorized style corrections.

**INV-343-M2: No Authority Transfer**
Extraction may move realization logic. It must not move document/session
authority into the spatial substrate.

**INV-343-M3: One Behavior, One Owner**
After migration, each shared spatial behavior has exactly one owner. Domain
widgets may consume the behavior or explicitly opt out.

---

# 7. Required Audit Matrix

Each spatial subsystem must be audited against this matrix.

| Concern | Current owner/file | Current authority | Violation to remove | Target owner/API | Required proof |
|---|---|---|---|---|---|
| Affine viewport mechanics | `src/model/canvas.rs`, demos, domain widgets | Mixed | Duplicate pan/zoom/fit behavior or implicit axis convention | `SpatialViewportState`, `SpatialAxisOrientation`, `SpatialTransform` | Checked forward/inverse parity for y-down Canvas compatibility and y-up Cartesian mapping; equal-scale visible bounds from the same rectangle |
| Domain coordinate mapping | Plot, TemporalSubstrate, domain engines | Domain | Spatial substrate owning semantic axes/time | Domain adapter before `SpatialProjection` | No axis or time mapping code in `src/model/spatial/` |
| Grid realization | Plot, timeline, node graph demos | Mixed | Independent color, spacing, clipping, and resize behavior | `SpatialVisualPolicy` + grid realization helper | Grid fills resized viewport and resolves theme roles centrally |
| Hit testing | Plot, timeline canvas, `CanvasProjection`, demo helpers | Mixed | Paint geometry and hit geometry drift | `SpatialHitRegion` keyed by `SpatialItemId` | Hit-test parity fixtures for each migrated primitive |
| Selection | Plot/session, temporal/application, `NodeEditorSession` | Domain/session | Selection stored in substrate | Domain session, translated via `SpatialIntent` | No selected-domain-object storage in substrate |
| Drag state | Plot/local, timeline/local, node graph session/demo | Domain/session + shared gesture mechanics | Active gesture rules duplicated per widget | `SpatialInteractionPolicy` + domain session | Cursor and drag threshold parity across spatial widgets |
| Route geometry | Plot connectors, SPEC-338, node wires | Mixed | Separate connector math per subsystem | SPEC-HAL-338 route helpers consumed by `SpatialRoutePlan` | Same route control points for equivalent route class |
| Guides | Node alignment, temporal markers, local ad hoc overlays | Mixed | Guides sharing grid identity or ignoring labels/padding | `SpatialGuidePlan` + guide visual roles | Active/structural guide count, color, and occupied-label exclusion tests |
| HUD/overlay | Node demo, plot overlays, timeline overlays | Mixed | World-relative HUD placement and missing edge padding | `SpatialOverlayPlan` | Overlay position invariant under pan/zoom/resize |
| Theme canvas palette | Demos and domain widgets | Mixed | Hand-tuned light/dark palettes per widget | `SpatialVisualPolicy` | One policy source for shared spatial roles |
| Deletion/action routing | Domain widgets | Domain/application | Substrate executing semantic deletes | Domain command layer after `SpatialIntent` translation | Delete tests prove substrate emits intent only |

No subsystem may be marked complete until every row has an explicit owner.

---

# 8. Acceptance Criteria

* `src/model/spatial/` exists with the `Spatial*` API names defined in this
  SPEC, or `src/model/canvas.rs` explicitly re-exports those names as the public
  compatibility boundary.
* Existing `Canvas*` behavior remains available until parity tests pass and
  SPEC-HAL-284 / SPEC-HAL-285 are amended.
* `SpatialViewportState` contains only a finite world center and finite positive
  equal-axis scale; target geometry, orientation, bounds, and clamp policy stay
  outside durable camera state.
* Every `AffineSurface` supplies an explicit axis orientation and materializes
  an immutable checked `SpatialTransform` from one validated target rectangle.
* Forward and inverse mapping, visible bounds, grid realization, pan, and
  anchored zoom obey §5.2.2 for both y-down Canvas compatibility and y-up
  Cartesian surfaces.
* No public `Spatial*` type or enum variant contains domain nouns.
* `SpatialProjection` contains no mutation channel.
* `SpatialProjection` is required only for domain-neutral spatial primitives;
  plot series, plot axes, timeline engine rendering, and other domain-owned
  render paths are not forced through it.
* `SpatialAdapter` functions are pure and receive immutable domain/session
  inputs.
* `SpatialRoutePlan` consumes SPEC-HAL-338 route geometry and does not become a
  second route math authority.
* New node graph features use `SpatialProjection` and shared spatial policies
  when behavior is domain-agnostic.
* Timeline route geometry follows SPEC-HAL-338 and does not inline new
  independent route arithmetic.
* Timeline time-to-x mapping remains TemporalSubstrate/domain authority.
* Plot axes, series rendering, probes, and chart-specific tools remain plot
  authority.
* Light/dark canvas visual roles resolve through `SpatialVisualPolicy`.
* Paint/hit-test parity is tested for every migrated spatial primitive.
* Cursor policy is consistent across spatial widgets unless overridden by a
  domain SPEC.
* Deletion, selection, drag commit, wire creation, interval mutation, and chart
  mutation remain outside the spatial substrate.
* HUD/status overlays preserve configured viewport-edge padding under pan, zoom,
  and window resize.

---

# 9. Non-Goals

* Rewriting Plot.
* Rewriting Timeline.
* Rewriting NodeGraphDocument or NodeEditorSession.
* Making one universal domain widget.
* Moving domain transactions into InfiniteCanvas.
* Removing existing domain SPECs.
* Solving force-directed graph layout.
* Solving timeline leveling or critical path computation.
* Solving plot axis/series rendering.
* A generic matrix, coordinate portability, axis-swap, rotation, shear,
  perspective, or 3D transform framework.
* Global world bounds, global zoom/span clamps, or evaluator/path precision
  policy.

---

# 10. Decision

The node graph implementation is not invalid because it did not reuse Plot.

The correct architectural direction is:

* Plot stays domain-specific.
* Timeline stays temporal-substrate-specific.
* Node graph stays document/session/projection-specific.
* InfiniteCanvas and CanvasProjection become the spatial substrate base.
* Shared spatial policies are extracted below all three.

The disaster condition is not "three domain widgets exist." The disaster
condition is "three domain widgets independently reimplement the same spatial
interaction laws." This SPEC forbids that drift.

---

# 11. Authority Status

The declared status of this SPEC is `Adopted`. Under SPEC-HAL-309, it carries
exactly that declared status.

The existence of `src/model/spatial/`, tests, changelog entries, or binding
cross-references in SPEC-HAL-284, SPEC-HAL-285, and SPEC-HAL-294 may establish
implementation evidence or the narrow obligations stated by those SPECs. None
changes this SPEC's declared status. This SPEC defines no adoption procedure.
