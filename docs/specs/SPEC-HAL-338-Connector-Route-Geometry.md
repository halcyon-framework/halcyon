# SPEC-HAL-338: **Connector Route Geometry**

### *Pure Waypoint Authority for Routed Connectors*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Layer:** Engine (Structural Geometry)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-000 (Naming) — module/type conventions
* SPEC-HAL-335 (TemporalSubstrate) — first-consumer architecture, INV-335-D4 endpoint-validity pattern
* SPEC-HAL-336 (TimelineCanvas) — first consumer's realization rules (INV-336-R8)
* SPEC-HAL-288 (Wire Rendering), SPEC-HAL-296B (Plot Connectors) — specified future consumers
* SPEC-PM-009 (Dual-Source Geometry Drift) — one computation, one result

---

# 1. Purpose

Define the single authority for **connector route geometry**: the computation that turns two directed endpoints and a style policy into an ordered waypoint plan plus an arrowhead anchor and direction.

It is the connector-domain peer of the geometry-engine family: `ColumnGeometryEngine` owns the column dimension, `TimeGeometryEngine` owns the time dimension, this engine owns the path between two directed points. One arithmetic, N consumers — never N inlined copies.

This SPEC defines geometry only. No drawing, no styling, no widget.

# 2. Problem Statement

Three specified consumers of routed-connector geometry exist:

1. **SPEC-HAL-336 INV-336-R8** — timeline dependency arrows. Implemented; the waypoint arithmetic is inlined in `TimelineCanvas::paint`.
2. **SPEC-HAL-288 §4** — node-graph wires (`Bezier`, `Straight`, `RightAngle`). Draft; unimplemented.
3. **SPEC-HAL-296B** — plot connector primitives. Draft; unimplemented.

Leaving the arithmetic inside one widget while two specified consumers approach is the CellMechanicsSubstrate failure repeated: extraction deferred past the second consumer cost a revert and a re-extraction. The arrow math is at its cheapest extraction point now — small, untangled, fully covered by the zero-visual-change bar. Per the substrate doctrine (SPEC-HAL-259 §2): reuse of a one-off widget contaminates; copying duplicates; therefore a shared engine with thin consumers.

Extraction now; interface generalization at consumer two. The v1 surface is hard-coded to right-angle routing — no generics, no trait parameters.

# 3. Authority Boundary

**The engine owns, exclusively:**

- Waypoint computation: directed endpoints + style policy → ordered route points.
- Arrowhead anchor and arrowhead direction as plan outputs.
- The geometric parameters of routing (stub length, terminal inset).

**The engine never owns:**

- **Endpoint semantics.** Consumers translate their domain objects to `(point, direction)` pairs: `IntervalEdge` → bar-edge point (336), port → port anchor (288), world coordinate → screen point (296B). The engine has no knowledge of intervals, ports, rows, or data.
- **Realization.** Drawing the plan — segment rects (336), stroke meshes (288), rotated quads (296B) — is consumer property. All are legitimate renderers over the same plan; this SPEC mandates none.
- **Styling and state policy.** Color, thickness, critical emphasis, hover/selected states: theme- and consumer-side.
- **Hit priority and z-order.** Consumer/spec-local.
- **Endpoint validity.** Whether both endpoints exist in the current frame is the consumer's snapshot discipline (pattern: INV-335-D4 — connectors with missing endpoints never reach the engine, let alone paint).
- **Obstacle avoidance.** The force-directed solver of SPEC-HAL-296B §4 is a **separate, optional authority beside this engine** (DP-296B-4: framework provides the solver, consumer decides). Route geometry answers "what is the canonical path between these two directed points"; the solver answers "how do many paths avoid many obstacles." They do not merge.

**DP-338-1 (Split authority):** Route geometry = shared pure math (this SPEC). Realization = per-subsystem. Styling/state = per consumer SPEC. Obstacle solving = separate optional authority. No all-in-one connector engine exists or may be built.

# 4. Module and Types

Module: `src/engines/route_geometry/mod.rs` — sibling of `column_geometry`, under the engines doctrine (`src/engines/mod.rs`): invariant-owning resolve logic, no widget, layout, or rendering dependencies. The engine is stateless; its surface is plain data types and pure functions. Points are `crate::core::math::Point` (f32, logical pixels).

```rust
/// Outward direction of the face a route departs from or arrives at.
/// Horizontal only: every specified consumer exits and enters horizontally
/// (IntervalEdge in 336; horizontal port exit/entry per INV-288-G1/G4).
/// A vertical variant requires a consumer SPEC that produces one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    Left,   // sign −1
    Right,  // sign +1
}

/// A directed route endpoint: where, and which way the face points outward.
/// The route departs the source along `dir` and arrives at the target
/// against `dir` (travel-at-arrival = opposite of the target's `dir`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteEndpoint {
    pub point: Point,
    pub dir: RouteDirection,
}

/// Geometric routing parameters. No visual meaning; lengths only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteParams {
    /// Minimum horizontal run out of the source face before the
    /// vertical transition.
    pub stub: f32,
    /// Terminal inset: the polyline stops this far short of the target
    /// anchor, leaving room the consumer's arrowhead realization occupies.
    pub head_length: f32,
}

/// Routed right-angle plan. Fixed-size: zero allocation per route
/// (SPEC-HAL-000 Principle 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RightAnglePlan {
    /// Ordered waypoints: [exit, stub corner, transition corner, approach end].
    /// Consecutive equal points are legal (degenerate segments); consumers
    /// realize or skip them deterministically.
    pub points: [Point; 4],
    /// Arrowhead anchor: the target endpoint itself.
    pub head_anchor: Point,
    /// Arrowhead direction: travel direction at arrival = opposite of the
    /// target endpoint's `dir`.
    pub head_dir: RouteDirection,
}
```

**Policy type (mandatory per the policy-type law):**

```rust
/// Route style policy. Normative now; enters code with the first
/// style-dispatching surface (§7), per the SPEC-HAL-337 HtsOwnership
/// pattern: the type lands with its first consumer.
pub enum RouteStyle {
    Straight,
    RightAngle,
    Bezier,
}
```

`RouteStyle` is the single source of truth for connector style vocabulary. SPEC-HAL-288's `WireStyle` reconciles to it (alias or replacement) when 288 is implemented; that amendment belongs to 288's implementation, not to this SPEC's adoption.

# 5. v1 Routing Law — RightAngle

The v1 public surface is one pure function:

```rust
pub fn route_right_angle(
    from: RouteEndpoint,
    to: RouteEndpoint,
    params: RouteParams,
) -> RightAnglePlan
```

**INV-338-G1 (Right-Angle Plan):** With `sign(Left) = −1.0`, `sign(Right) = +1.0`:

```
p0 = from.point
p1 = ( from.point.x + sign(from.dir) · params.stub , from.point.y )
p2 = ( p1.x , to.point.y )
p3 = ( to.point.x + sign(to.dir) · params.head_length , to.point.y )

head_anchor = to.point
head_dir    = opposite(to.dir)
```

This is, term for term, the arithmetic SPEC-HAL-336's canvas executes today (`mid_x = fx + out_dir·stub`; `approach_x = tx − head_dir·head_length`; H–V–H). The extraction's acceptance bar is byte-identical scene output.

**INV-338-G2 (Backward Targets):** The plan above is the complete right-angle law for **all** relative endpoint positions, including targets behind the source: the approach segment legally runs back across the transition (`p3.x` on the far side of `p2.x`). No loop-around exists for `RightAngle`. Loop-around routing is a `Bezier` law (INV-288-G2) and is defined under §7.

**INV-338-G3 (Degeneracy):** Zero-length segments (`from.y == to.y`, or stub and approach coinciding) are legal plan states, produced deterministically, never special-cased away by the engine. Realization handles them (the 336 realization already clamps segment extents to thickness).

# 6. Purity and Determinism

**INV-338-P1 (Geometry Purity):** The engine holds no theme, color, thickness, scene, layer, or widget knowledge. A `RightAnglePlan` is positions and a direction — nothing else crosses the boundary. Mirror of INV-335-D1 at the connector seam.

**INV-338-P2 (Determinism):** `route_right_angle` is a pure, stateless, allocation-free function. Identical inputs produce identical plans, every call, every frame. No engine state exists to mutate.

**INV-338-P3 (Consumer-Side Validity):** The engine routes whatever coordinates it is handed. Same-frame endpoint validity — both endpoints resolved from the same snapshot, hidden/culled endpoints dropped before routing — is the consumer's obligation (INV-335-D4 is the binding precedent for 336; 288/296B restate it for their domains).

**FORBID-338-1:** No realization API of any kind in this module — no `push_rect`, no `Path`, no scene, no `PaintContext` import.
**FORBID-338-2:** No styling fields on any route type. A "critical" flag, a color, a width on a route input or plan is a violation; those live in consumer descriptors.
**FORBID-338-3:** No obstacle, occupancy, or collision input. The moment routing consults other routes or scene contents, it has become the 296B solver's concern, which remains separate.

# 7. Defined Now, Implemented at the Node Graph

The following are **normative upon adoption** and **implemented when SPEC-HAL-288's consumer lands** — not before, and never speculatively:

- **Style dispatch.** A style-dispatching entry taking `RouteStyle` arrives with the second implemented style. v1 has exactly one entry (`route_right_angle`) and no dead match arms.
- **`Straight`.** Plan = `[from.point, approach]` with `approach = to.point − unit(to.point − from.point) · head_length`; head at `to.point`, direction = travel unit. (Generalizes `head_dir` beyond axis-alignment; the plan type generalizes with it.)
- **`Bezier`.** Control-point law per INV-288-G1 (horizontal-exit/entry spread `dx = max(|Δx|, 50)·0.5`); **backward law per INV-288-G2** — target left of source routes with a vertical loop offset to clear the nodes. Bezier plans carry endpoints + two control points, not waypoints.
- **Route hit distance.** Minimum point-to-route distance via the per-segment clamped-projection formula — already written identically in SPEC-HAL-288 INV-288-H2 and SPEC-HAL-296B §8.1. One implementation, here, when the first hit-testing consumer needs it. Hit *priority* stays consumer-side (§3).
- **Arrowhead tessellation helper.** Three triangle vertices from `(anchor, dir, length, half_width)` — shareable realization geometry. The 336 canvas keeps its existing `arrow_head` realization (byte-identical bar); the helper lands with the first consumer that wants it.

**FORBID-338-4:** None of the above may be implemented ahead of its consumer. Defined-but-unconsumed code arms are the half-measure this framework rejects.

# 8. Consumers and Relationship Map

| Spec | Relationship |
|---|---|
| SPEC-HAL-336 INV-336-R8 | First consumer. Waypoint computation cites this SPEC; realization rules (segment rects, 1px / 2px critical, `semantic.error`, filled-triangle head, exit/enter per `IntervalEdge`) remain in 336. Amendment lands **after** the extraction validates (gates green, visually identical), per the house sequence. |
| SPEC-HAL-288 §4 | Consumes all three styles when the node graph lands. `WireStyle` reconciles to `RouteStyle` then (§4). Realization (path-pipeline stroke meshes per INV-288-R1) stays 288's. |
| SPEC-HAL-296B | Connector primitives may consume plan computation; the force-directed `PlotRouter` remains a separate optional authority (DP-296B-4). 296B's rendering (rotated quads, overlay pipeline) stays 296B's. |
| SPEC-HAL-335 | INV-335-D4 is the endpoint-validity pattern every consumer follows (INV-338-P3). |

# 9. Migration

1. **Extraction** — one CC task: create `src/engines/route_geometry/mod.rs` (types of §4 minus `RouteStyle`, function of §5); `TimelineCanvas` translates `IntervalEdge` → `RouteEndpoint`, calls `route_right_angle`, realizes the plan through its existing `h_seg`/`v_seg`/`arrow_head` unchanged. Acceptance: `cargo check --lib` clean; scene output byte-identical; zero visual change.
2. **SPEC-HAL-336 amendment** — after step 1 validates: INV-336-R8 reworded to cite SPEC-HAL-338 for waypoint computation; all realization language retained in 336.

# 10. Acceptance Criteria

- `route_right_angle` reproduces the pre-extraction canvas arrow geometry exactly (zero visual change; identical scene rects and head triangles for identical inputs)
- Determinism property holds: identical `(from, to, params)` → identical plan
- Module imports contain no theme, scene, widget, or paint types (FORBID-338-1 mechanically checkable)
- Plan construction performs no heap allocation
- Degenerate-segment inputs (`from.y == to.y`; backward targets) produce stable plans without special-casing

# 11. Non-Goals

- Realization of any kind — 336/288/296B own their renderers over the shared plan
- Obstacle avoidance, crossing minimization, route bundling — SPEC-HAL-296B §4 territory
- Styling, theming, visual state, hit priority, z-order — consumer SPECs
- Endpoint semantics or domain translation — consumers
- Vertical route directions — absent a consumer SPEC that produces them
