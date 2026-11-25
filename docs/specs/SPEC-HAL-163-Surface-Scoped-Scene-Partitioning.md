# SPEC-HAL-163: Surface-Scoped Scene Partitioning (FOUNDATION)

**Status:** Binding
**Amended:** 2026-09-21 — Current grayscale text has no background-reconstruction dependency; all surface-isolation invariants remain binding.
**Layer:** Rendering / Scene Architecture
**Scope:** Scene construction, derived-resource isolation, multi-surface rendering
**Depends on:**

* SPEC-HAL-084 (RenderGraph v2)
* SPEC-HAL-150 (Compositor Authority)
* SPEC-HAL-161 (Cross-Window Signals)
* SPEC-HAL-162 (Lifecycle & Teardown)

---

## 1. Problem Statement

Halcyon currently constructs **a single global `SceneGraph` per frame** and distributes it across multiple rendering
surfaces.

This model breaks down immediately when multiple windows are present:

* A global scene allows one surface's nodes to leak into another surface.
* Surface-specific derived renderer resources can introduce the same hidden cross-surface dependency.
* Attempting per-window widgets causes multiple layout invocations, violating `SINGLE-LAYOUT-PER-FRAME`.

As a result:

* Multi-window demos are forced into scene-only hacks.
* Per-window UI is architecturally impossible.
* Rendering invariants are violated by design, not by bugs.

This SPEC introduces **surface-scoped scene partitioning** as a foundation to resolve these issues without weakening
existing invariants.

---

## 2. Core Principle

> **A rendering surface owns its scene.**

There is no such thing as a “global scene” at render time.

---

## 3. Definitions

* **Surface**: A compositor-managed render target identified by `SurfaceId`.
* **SceneGraph**: A renderable graph containing background, path, image, and text primitives.
* **Surface Scene**: A `SceneGraph` that is exclusively rendered into a single `SurfaceId`.

---

## 4. Design Overview

### 4.1 Scene Ownership

* Each `SurfaceId` has **exactly one** `SceneGraph` per frame.
* SceneGraphs **must not** be shared across surfaces.
* Empty scenes are valid.

```
Frame
 ├─ Layout (global, once)
 ├─ Scene Extraction
 │    ├─ Surface A → SceneGraph A
 │    ├─ Surface B → SceneGraph B
 │    └─ Surface C → SceneGraph C
 └─ Render (per surface)
```

---

### 4.2 Layout Relationship

* Layout remains **global and singular**.
* Scene extraction happens **after layout**, using layout results.
* This SPEC **does not** introduce per-surface layout passes.

**Invariant preserved:**
`SINGLE-LAYOUT-PER-FRAME`

---

### 4.3 Derived Renderer Resource Isolation

* Current text rendering is grayscale and does not sample or require a reconstructed background.
* Text primitives are valid without background-affecting nodes; their co-presence is not a renderer invariant.
* Any derived renderer resource whose contents depend on a surface must be produced and consumed within that same surface.
* Cross-surface scene or derived-resource dependency is illegal.

SPEC-HAL-064 and SPEC-HAL-067 remain future, not-implemented LCD/subpixel designs. Reintroducing background reconstruction for such a path requires new binding authority and a surface-scoped integration; this SPEC does not pre-authorize it.

A cross-surface ownership violation is handled as follows:

* Debug: panic
* Release: skip surface render (implementation-defined, but explicit)

---

## 5. Renderer Contract Changes

### 5.1 Old Model (Illegal Going Forward)

```
build_multi(scene: &SceneGraph, snapshot: Snapshot)
```

### 5.2 New Model (Required)

```
build_multi(
    surface_scenes: &HashMap<SurfaceId, SceneGraph>,
    snapshot: Snapshot
)
```

Renderer guarantees:

* Each surface is rendered using **only its own scene**.
* No merging, no fallback, no implicit inheritance.

---

## 6. Failure Rules (Binding)

* Rendering a surface with another surface’s nodes → invariant violation.
* Binding a surface-specific derived resource produced from another surface → invariant violation.
* Falling back to a global scene → forbidden.
* Creating or copying scene nodes implicitly to satisfy a renderer requirement → forbidden.

---

## 7. Required Invariants

| ID            | Invariant                                      |
|---------------|------------------------------------------------|
| INV-SCENE-001 | SceneGraphs are surface-scoped                 |
| INV-SCENE-002 | No cross-surface scene or derived-resource dependency |
| INV-SCENE-003 | Layout executes once per frame                 |
| INV-SCENE-004 | Renderer never sees a mixed scene              |
| INV-SCENE-005 | Empty surface scenes are valid                 |

---

## 8. Explicit Non-Goals

This SPEC does **not** define:

* Widget APIs
* Per-surface layout
* UI components
* Input routing
* Focus handling
* Performance optimizations
* Demo behavior

Those are downstream consumers of this foundation.

---

## 9. Migration Notes

* Existing demos using scene-only rendering remain valid.
* Widget-based multi-window demos are **blocked until this SPEC is implemented**.
* Any workaround that reintroduces a global scene is forbidden.

---

## 10. Acceptance Criteria

This SPEC's **engine-layer invariants** are satisfied when:

* **INV-SCENE-001**: `DrawPlanBuilder::build_multi` accepts `HashMap<SurfaceId, SceneGraph>`.
  No cloning. No shared SceneGraph instances. Empty scenes are valid.
* **INV-SCENE-002**: Scene data and any surface-specific derived resources remain within their owning surface. Current grayscale text is accepted without background-affecting nodes.
* **INV-SCENE-003**: `SINGLE-LAYOUT-PER-FRAME` remains enforced (unchanged by this SPEC).
* **INV-SCENE-004**: Renderer consumes only per-surface compiled data. Transitional combined
  buffers iterate in deterministic `surface_stack` order, not `HashMap::values()`.
* **INV-SCENE-005**: Surfaces with no scene or empty scenes receive a valid clear pass.

**Distinct scene ownership** means:
* Primary surface receives the actual `SceneGraph` (move semantics).
* Other surfaces receive `SceneGraph::new()` (empty).
* `scene.clone()` across surfaces is **forbidden**.

**Not acceptance criteria** (moved to Non-Goals):
* Per-window widget trees
* Per-surface layout
* Feature-level multi-window UI

---

## 11. Status

**Binding**

This SPEC establishes a non-negotiable foundation.
All future multi-window UI work depends on it.

### 11.1 Status Clarification

Engine-layer invariants defined by SPEC-HAL-163 are **mechanically enforced**:

* `DrawPlanBuilder`, `DrawPlan`, `MultiTargetDrawPlan`, and `WgpuRenderer` are **SPEC-compliant**.
* Scene cloning across surfaces is mechanically prevented via `Option::take()` at call sites.
* Transitional combined GPU buffers iterate in deterministic `surface_stack` order.
* Per-surface compiled data is the sole input to render passes.

SPEC-HAL-163 **does not include per-window widget trees**.
Feature-level multi-window UI remains blocked by missing widget infrastructure, **not by scene partitioning**.

**End of SPEC-HAL-163**
