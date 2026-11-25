# SPEC-HAL-081C — Overlay Surface Integration

**Status:** Compositor-Complete (upstream routing pending)
**Extends:** SPEC-HAL-081A (Compositor Enforcement, frozen base)
**Scope:** Overlay Surface Contract (compositor-level only)
**Audience:** Compositor, Renderer, Overlay implementers
**Depends on:**
- SPEC-HAL-081A (Compositor Enforcement, Phase-1 frozen)
- SPEC-HAL-081B (Multi-Surface Compositor, batching/renderer contracts)
- SPEC-HAL-127 (Overlay Stack and Input Capture)
- SPEC-HAL-137 (Overlay Stack Hardening)
- SPEC-HAL-137B (Overlay Focus, Capture, Modal Enforcement)
- SPEC-HAL-150 (Compositor Authority and Limits)

---

## Implementation Status

### Implemented (compositor-internal only)

Overlay support exists as **compositor-internal state** and stack operations:

- **`SurfaceState.parent_surface: Option<SurfaceId>`** — Overlay provenance tracking (internal only)
- **`SurfaceStack::create_overlay_surface()`** — `pub(crate)` overlay construction
- **Structural role inference** — `is_base()` / `is_overlay()` via `parent_surface.is_some()`
- **Snapshot filtering** — `snapshot()` returns only base surfaces to renderer

### Implemented Invariants

| Invariant | Enforcement |
|-----------|-------------|
| INV-PROV-01 (parent must exist) | `create_overlay_surface()` returns `None` if parent not found |
| INV-PROV-02 (parent must be base) | `create_overlay_surface()` returns `None` if parent is overlay |
| INV-ZORD-02 (overlay z > parent z) | `create_overlay_surface()` returns `None` if z_index <= parent |
| INV-LIFE-04/05 (cascade destroy) | `destroy_surface()` removes all overlays when base destroyed |

### Renderer Isolation

- **`SurfaceStackSnapshot<BaseContract>`** contains only `SurfaceDescriptor<BaseSurfaceTag>`
- **Overlays are invisible to renderer** — filtered at snapshot boundary
- **No `parent_surface` in `SurfaceDescriptor`** — ABI frozen per SPEC-HAL-081A

### Query Hardening (Snapshot & Read Paths)

| Read Path | Overlay Visibility |
|-----------|-------------------|
| `Compositor::snapshot()` | Excluded (base only) |
| `Compositor::snapshot_unchecked()` | Excluded (test-only, base only) |
| `Compositor::get_surface()` | Returns `None` for overlays |
| `Compositor::has_surface()` | Bool only (no data leak) |
| `Compositor::overlay_parent()` | **Explicit overlay API** |
| `Compositor::overlays_of()` | **Explicit overlay API** |

Overlay visibility is **explicit and exclusive** — only `overlay_parent()` and `overlays_of()` expose overlay information.

### Hard Authority Boundaries (SPEC-HAL-150)

The compositor has **NO** authority over:
- Input routing
- Capture semantics
- Focus management
- Modal behavior
- Hit-testing

The renderer has **NO** knowledge of:
- Overlay surfaces
- Parent-child relationships
- Surface roles

### NOT IMPLEMENTED (design-only)

The following remain design-only and require upstream routing layer (SPEC-HAL-127/137/137B):

- Input capture, focus, modal behavior (compositor has no authority per SPEC-HAL-150)
- Hit-testing on overlay surfaces (upstream responsibility)
- Dismissal logic (upstream responsibility)
- Overlay kind semantics (Tooltip/Menu/Dialog — upstream metadata)
- INV-PROV-05 (`SurfaceDescriptor` exposing `parent_surface`) — **deferred**: provenance stays internal
- Section 6.2 fields (`surface_role`, `parent_surface` in descriptor) — **not added**: ABI frozen

---

## 1. Purpose and Scope

SPEC-HAL-081C defines the **surface-level contract** for overlay integration in the compositor.

This SPEC addresses ONLY:
- Surface role classification (Base vs Overlay)
- Surface provenance (parent-child relationships)
- Surface lifetime constraints
- Z-order rules between base and overlay surfaces
- Integration points with upstream routing

This SPEC does NOT address:
- Widget semantics (overlays are surfaces, NOT widgets)
- Hit-testing logic (upstream responsibility per SPEC-127/137/137B)
- Input capture implementation (upstream responsibility)
- Focus management (upstream responsibility)
- Modal/non-modal behavior (upstream responsibility)
- Batching/renderer contracts (see SPEC-HAL-081B)
- Scene-key partitioning (see SPEC-HAL-081B)

---

## 2. Surface Role Classification

### 2.1 Role Enumeration

Every surface MUST have exactly one role:

| Role | Description |
|------|-------------|
| `Base` | Primary content surface. May have overlay children. Default role. |
| `Overlay` | Auxiliary surface rendered above a parent. MUST have parent surface. |

### 2.2 Role Constraints

**INV-SURF-ROLE-01**
A surface's role is immutable for its lifetime.

**INV-SURF-ROLE-02**
Base surfaces MAY exist without a parent.

**INV-SURF-ROLE-03**
Overlay surfaces MUST have a parent surface.

**INV-SURF-ROLE-04**
Overlay surfaces CANNOT be parents of other overlays (no overlay nesting at compositor level).

---

## 3. Surface Provenance Rules

### 3.1 Parent-Child Relationship

**INV-PROV-01**
An overlay surface's `parent_surface` field MUST reference a valid, existing surface.

**INV-PROV-02**
The parent surface MUST be a `Base` surface.

**INV-PROV-03**
Parent-child relationships are immutable once established.

**INV-PROV-04**
Compositor tracks provenance for ordering only; it does NOT interpret provenance semantically.

### 3.2 Provenance Exposure

**INV-PROV-05**
`SurfaceDescriptor` exposes `parent_surface: Option<SurfaceId>` for upstream routing to inspect.

**INV-PROV-06**
Upstream routing (per SPEC-127/137/137B) uses provenance to determine dismissal scope and capture boundaries.

**INV-PROV-07**
Compositor MUST NOT use provenance for dismissal, capture, or routing decisions.

---

## 4. Surface Lifetime Rules

### 4.1 Creation

**INV-LIFE-01**
Base surfaces may be created at any time.

**INV-LIFE-02**
Overlay surfaces may only be created when their parent surface exists.

**INV-LIFE-03**
Creating an overlay with a non-existent parent is a STOP condition.

### 4.2 Destruction

**INV-LIFE-04**
Overlay surfaces MUST be destroyed when their parent surface is destroyed.

**INV-LIFE-05**
Destroying a base surface with active overlay children MUST cascade-destroy all overlay children.

**INV-LIFE-06**
Overlay destruction MUST be synchronous with parent destruction (no orphaned overlays).

### 4.3 Lifetime Invariant

**INV-LIFE-07**
An overlay surface MUST NOT outlive its parent surface. This is a STOP condition (STOP-05 in 081A).

---

## 5. Z-Order Constraints

### 5.1 Base vs Overlay Ordering

**INV-ZORD-01**
All overlay surfaces MUST render above all base surfaces.

**INV-ZORD-02**
Overlay z-index MUST be strictly greater than its parent base surface z-index.

**INV-ZORD-03**
Overlay z-index MUST be strictly greater than all base surface z-indices in the stack.

### 5.2 Overlay-to-Overlay Ordering

**INV-ZORD-04**
Overlays with the same parent are ordered by their z-index (higher = on top).

**INV-ZORD-05**
Overlays with different parents follow their respective z-indices without cross-parent constraints.

### 5.3 Z-Index Assignment

**INV-ZORD-06**
Compositor assigns overlay z-indices from a reserved range above base surfaces.

**INV-ZORD-07**
Recommended: Base surfaces use z-index range [0, 999]. Overlay surfaces use z-index range [1000, max].

---

## 6. Upstream Routing Integration

### 6.1 Authority Boundary

**INV-ROUTE-01**
Compositor exposes overlay surfaces via `SurfaceStackSnapshot` (INV-COMP-P03).

**INV-ROUTE-02**
Upstream routing (SPEC-127/137/137B) is SOLE AUTHORITY for:
- Hit-testing overlay bounds
- Input capture decisions
- Modal enforcement
- Focus trapping
- Dismissal logic

**INV-ROUTE-03**
Compositor is READ-ONLY to upstream routing. Routing queries compositor state; compositor never queries routing state.

### 6.2 Surface Descriptor Fields

Overlay integration requires the following fields in `SurfaceDescriptor`:

| Field | Type | Description |
|-------|------|-------------|
| `surface_role` | `SurfaceRole` | Base or Overlay |
| `parent_surface` | `Option<SurfaceId>` | Parent for overlays, None for base |

These fields enable upstream routing to make correct decisions without compositor involvement.

---

## 7. Zero Widget Semantics

### 7.1 Compositor-Level Enforcement

**INV-WIDGET-01**
Overlay surfaces have NO widget semantics at compositor level.

**INV-WIDGET-02**
Compositor does NOT know or care what widgets render to an overlay surface.

**INV-WIDGET-03**
Compositor does NOT distinguish tooltip vs dialog vs menu at surface level.

**INV-WIDGET-04**
Overlay "kind" (Tooltip, Menu, Dialog, etc. per SPEC-137) is upstream metadata, NOT compositor state.

### 7.2 Rendering Neutrality

**INV-WIDGET-05**
Compositor renders overlay surfaces identically to base surfaces (apply bounds, clip, opacity, z-order).

**INV-WIDGET-06**
Overlay surface content is opaque to compositor (just a scene_key reference).

---

## 8. Stop Conditions

Implementation MUST HALT immediately if any of the following occur:

| ID | Condition |
|----|-----------|
| STOP-081C-01 | Overlay surface created without valid parent surface |
| STOP-081C-02 | Overlay surface outlives parent surface |
| STOP-081C-03 | Compositor performs hit-testing on overlay surfaces |
| STOP-081C-04 | Compositor makes capture/focus/modal decisions |
| STOP-081C-05 | Overlay surface participates in layout |
| STOP-081C-06 | Overlay rendering without upstream routing integration |
| STOP-081C-07 | Compositor interprets overlay kind (Tooltip/Menu/Dialog) |
| STOP-081C-08 | Overlay z-index assigned below any base surface z-index |

Upon STOP condition: cease implementation, file deviation report, await SPEC amendment.

---

## 9. Entry Criteria

Overlay surface integration may begin ONLY when ALL of the following are true:

- [x] Phase 1 complete (Core Surfaces, SPEC-HAL-081A)
- [x] SceneFrame carries SurfaceStackSnapshot
- [x] DrawPlanBuilder consumes surface stack
- [x] Widget paint does not iterate compositor
- [ ] SPEC-HAL-081B multi-surface batching implemented
- [ ] Upstream routing layer exists (SPEC-127/137/137B enforcement points)
- [ ] SurfaceRole and parent_surface fields added to SurfaceDescriptor
- [ ] Marker types (BaseSurfaceTag, OverlaySurfaceTag) defined

---

## 10. Completion Criteria

Overlay surface integration is complete when ALL of the following are true:

- [ ] At least one overlay surface type functional (e.g., tooltip)
- [ ] INV-SURF-ROLE-* verified
- [ ] INV-PROV-* verified
- [ ] INV-LIFE-* verified
- [ ] INV-ZORD-* verified
- [ ] INV-ROUTE-* verified
- [ ] INV-WIDGET-* verified
- [ ] No STOP-081C-* conditions triggered

---

## 11. Non-Goals

This SPEC explicitly does NOT cover:

| Topic | Reason |
|-------|--------|
| Overlay widget implementation | Widget layer, not compositor layer |
| Hit-test algorithms | Upstream routing (SPEC-127/137/137B) |
| Focus management | Upstream routing (SPEC-137B) |
| Modal capture logic | Upstream routing (SPEC-137/137B) |
| Dismissal behavior | Upstream routing (SPEC-137) |
| Overlay animations | Future SPEC |
| Overlay anchoring to widgets | Widget layer concern |
| Batching/renderer contracts | SPEC-HAL-081B |
| Scene-key partitioning | SPEC-HAL-081B |

---

## 12. Relationship to Other SPECs

| SPEC | Relationship |
|-----|--------------|
| SPEC-HAL-081A | Phase-1 enforcement (frozen); 081C extends with overlay specifics |
| SPEC-HAL-081B | Multi-surface compositor contracts (batching, scene_key, renderer) |
| SPEC-HAL-127 | Upstream authority for overlay stack and input capture |
| SPEC-HAL-137 | Upstream authority for overlay hardening and modal contract |
| SPEC-HAL-137B | Upstream authority for focus/capture enforcement |
| SPEC-HAL-150 | Compositor authority limits (compositor reads, routing decides) |

---

## Revision History

| Date | Change |
|------|--------|
| 2026-01-16 | **COMPOSITOR CLOSURE**. All compositor-layer overlay work complete. Query hardening verified. No further compositor changes without new SPEC. |
| 2026-01-16 | Added Implementation Status section. Compositor-internal overlay support verified. |
| 2026-01-16 | Renumbered from 081B to 081C to resolve collision with Multi-Surface Compositor SPEC |
| 2026-01 | Initial draft. Overlay surface preparation (no implementation). |
