# SPEC-HAL-081B: Multi-Surface Compositor

**Status:** CLOSED (Fully Implemented)
**Closure Date:** 2026-01-18
**Freeze Date:** 2026-01-16
**Author:** Design Document
**Date:** 2026-01-16
**Depends On:** SPEC-HAL-081A (closed)
**Related:** SPEC-HAL-081C (Overlay Surface Integration), SPEC-HAL-030A (narrow window-surface queue-depth amendment)

> **CLOSURE NOTICE:** This SPEC is fully implemented and permanently closed as of 2026-01-18.
> The multi-window rendering execution model is complete. No provisional state remains.
> No changes permitted without a new SPEC explicitly reopening this area.
> SPEC-HAL-030A reopens only the renderer-owned `desired_maximum_frame_latency`
> value and its shared configuration construction path; every other 081B
> contract remains closed.

## Abstract

This SPEC defines the multi-surface compositor execution model. It specifies the routing model, batcher
changes, renderer contract, and compile-time boundaries.

**Scope clarification:**

- This SPEC (081B) covers: batching, scene_key partitioning, renderer contracts, render target registry
- Overlay surface contract (roles, provenance, lifetime, z-order) is defined in SPEC-HAL-081C

## Implementation Status

**FULLY IMPLEMENTED AND CLOSED.**

All components are production-ready:

- `SurfaceRole` sealed trait with `BaseSurfaceTag` and `OverlaySurfaceTag` implementations
- `SurfaceDescriptor<R: SurfaceRole>` generic parameter (default=BaseSurfaceTag)
- `SurfaceStackSnapshot` typed to expose only `BaseSurfaceTag` surfaces under BaseContract
- ABI guard updated to enforce exhaustive field destructuring
- Multi-surface batching via `compile_multi()` / `render_multi()`
- Scene-key partitioning in batcher
- `RenderTargetRegistry` with RAII acquisition model
- Swapchain configuration authority solely in `WgpuRenderer`
- All debug-only diagnostics properly gated

**Binding Invariants (INV-RT-00x series):**

- INV-RT-004: No configure while acquired
- INV-RT-005/007/008: Acquire-only rendering, RAII enforced
- No registry introspection in release builds
- No size/scale/config state in registry
- Renderer is sole authority for `wgpu::SurfaceConfiguration`

---

## 0. Phase-2 Entry Conditions

Phase-2 implementation may begin ONLY when ALL of the following are true:

1. **Feature flag required**: The `phase2` cargo feature must be explicitly enabled. Without this flag, Phase2 type does
   not exist and cannot be referenced.
2. **Explicit compositor API fork**: Phase-2 APIs (e.g., `snapshot_phase2()`, `build_partitioned()`) must be distinct
   from Phase-1 APIs. No modification of existing Phase-1 signatures.
3. **Zero changes to 081A**: SPEC-HAL-081A is frozen. Phase-2 work must not amend, relax, or redefine any 081A invariant.
   If 081A blocks progress, file an SPEC amendment request.
4. **081C entry criteria met**: All entry criteria listed in SPEC-HAL-081C Section 9 must be satisfied before overlay
   surfaces are integrated.

Violation of any entry condition is a STOP condition.

---

## 1. Phase-2 Responsibilities

### 1.1 Multi-Surface Composition Semantics

Phase-2 introduces multiple compositor surfaces rendered in a single frame:

- **Base Surface**: Full-viewport content (z=0), always present
- **Overlay Surfaces**: Floating UI elements (tooltips, dropdowns, popovers)
- **Modal Surfaces**: Focus-trapping overlays (dialogs, sheets)

Surfaces are composited in z-order. Each surface:

- Has a unique `scene_key: u64` (non-zero, assigned by compositor)
- Owns a distinct region of the scene graph
- May have independent clip, opacity, and transform

### 1.2 Overlay / Modal Surface Ordering

```
z-order (back to front):
  [0] Base surface (scene_key=1)
  [1] Overlay A (scene_key=2, tooltip)
  [2] Overlay B (scene_key=3, dropdown)
  [3] Modal (scene_key=4, dialog)
```

Ordering is determined solely by compositor `z_index`. The renderer iterates surfaces in compositor-provided order and
submits batches per surface.

### 1.3 Scene-Key Ownership (Authoritative Source)

**Authoritative source of scene_key: Compositor**

The scene_key lifecycle:

1. **Created**: `Compositor::create_surface()` assigns unique scene_key
2. **Bound**: Widget tree associates content with scene_key via `PaintContext`
3. **Immutable**: Once a primitive enters the batcher, its scene_key cannot change
4. **Consumed**: Renderer receives pre-partitioned batches by scene_key

The compositor is the sole authority. Widgets do not invent scene_keys.

---

## 2. Routing Model Decision

### 2.1 Options Considered

**Option A: Per-Surface DrawPlan Instances**

- Each surface produces its own `DrawPlan`
- Multiple `DrawPlanBuilder` invocations per frame
- Pros: Clean separation, simple mental model
- Cons: Multiple scene graph traversals, duplicated batcher state, complex frame coordination

**Option B: Single DrawPlan with Pre-Partitioned Batches**

- One `DrawPlan` containing all surfaces
- Batches are pre-grouped by `scene_key` during build phase
- Renderer iterates surface stack and submits matching batches
- Pros: Single traversal, single batcher, O(1) lookup per surface
- Cons: Requires batch partitioning logic in batcher

**Option C: Multi-Pass Batcher**

- Batcher makes one pass per surface
- Scene graph filtered by surface ownership
- Pros: Simple batcher logic
- Cons: O(S) traversals, scene graph must encode surface ownership

### 2.2 Decision: Option B — Single DrawPlan with Pre-Partitioned Batches

**Rationale:**

1. **Single traversal**: Scene graph is walked once. O(N) not O(S×N).
2. **Batch locality**: Primitives for the same surface are contiguous in memory.
3. **Renderer simplicity**: Renderer receives `HashMap<scene_key, Vec<BatchRange>>` and iterates surfaces in stack
   order.
4. **Phase-1 compatibility**: Degenerates to current behavior when S=1.

**Rejected alternatives:**

- **Option A rejected**: Multiple DrawPlans require frame-level coordination and duplicate batcher allocations. Does not
  scale.
- **Option C rejected**: Multi-pass traversal is O(S×N). Scene graph would need surface annotations, violating
  separation of concerns.

---

## 3. Batcher & Scene Changes (Design Only)

### 3.1 Where Surface Awareness Is Introduced

| Component    | Phase-1                          | Phase-2                                            |
|--------------|----------------------------------|----------------------------------------------------|
| SceneGraph   | No surface awareness             | No change (surfaces are compositor concern)        |
| PaintContext | Implicit single surface          | Carries `current_scene_key: u64`                   |
| Batcher      | `set_scene_key()` once per frame | `set_scene_key()` per surface region               |
| DrawPlan     | Flat batch list                  | `batches_by_surface: HashMap<u64, Vec<DrawBatch>>` |

### 3.2 PaintContext Surface Binding

```rust
// Phase-2: PaintContext carries surface context
impl PaintContext {
    /// Current surface's scene_key (set by compositor via widget tree)
    pub fn scene_key(&self) -> u64;

    /// Enter a child surface region (for overlay content)
    pub fn with_surface<F>(&mut self, scene_key: u64, f: F)
    where
        F: FnOnce(&mut PaintContext);
}
```

The scene_key flows from compositor → widget tree → PaintContext → Batcher.

### 3.3 Batcher Changes

```rust
// Phase-2: Batcher tracks current surface and partitions output
impl Batcher {
    /// Set current surface for subsequent primitives
    pub(super) fn set_scene_key(&mut self, scene_key: u64);

    /// Build batches, partitioned by scene_key
    pub fn build_batches_partitioned(&mut self) -> PartitionedBatches;
}

struct PartitionedBatches {
    /// Batches grouped by scene_key, in insertion order within each group
    by_surface: HashMap<u64, Vec<DrawBatch>>,
    /// Path draws grouped by scene_key
    paths_by_surface: HashMap<u64, Vec<PathDraw>>,
    /// Total counts for buffer allocation
    total_vertices: u32,
    total_indices: u32,
}
```

### 3.4 Where scene_key Becomes Immutable

```
Compositor.create_surface(z, bounds)
    ↓ scene_key assigned (u64, unique, non-zero)
Widget tree receives surface assignment
    ↓ scene_key flows through layout/paint
PaintContext.scene_key() returns current surface
    ↓ scene_key passed to batcher
Batcher.add_rect(...) tags primitive with scene_key
    ↓ scene_key IMMUTABLE from this point
DrawBatch.scene_key is fixed
    ↓
Renderer consumes batches by scene_key
```

**Immutability boundary**: Once a primitive is added to the batcher, its scene_key is frozen.

---

## 4. Renderer Contract

### 4.1 Eliminate O(S×B) Filtering

**Phase-1 (current):**

```rust
// O(S×B) - filter batches per surface
for surface in surface_stack.iter() {
for batch in draw_plan.batches.iter() {
if batch.scene_key == surface.scene_key {
render_batch(batch);
}
}
}
```

**Phase-2 (required):**

```rust
// O(S + B) - direct lookup per surface
for surface in surface_stack.iter() {
if let Some(batches) = draw_plan.batches_by_surface.get( & surface.scene_key) {
for batch in batches {
render_batch(batch);
}
}
}
```

### 4.2 Data Structure Consumed by Renderer

```rust
/// Phase-2 DrawPlan structure
pub struct DrawPlan<Phase2> {
    /// Batches partitioned by scene_key (O(1) lookup)
    pub(crate) batches_by_surface: HashMap<u64, Vec<DrawBatch>>,

    /// Path draws partitioned by scene_key
    pub(crate) paths_by_surface: HashMap<u64, Vec<PathDraw>>,

    /// Clip rects (shared across surfaces)
    pub(crate) path_clips: Vec<RectF>,

    /// Flattened primitives (for text quad building)
    pub(crate) primitives: Vec<FlatPrimitive>,

    /// Surface stack (ordering authority)
    pub(crate) surface_stack: SurfaceStackSnapshot<Phase2>,

    /// Frame metadata
    pub(crate) clear_color: Color,
    pub(crate) total_vertices: u32,
    pub(crate) total_indices: u32,
}
```

### 4.3 Renderer Iteration Order

1. Iterate `surface_stack` in z-order (compositor authority)
2. For each visible surface, lookup `batches_by_surface[scene_key]`
3. Submit batches in insertion order within each surface
4. Handle clip state transitions between surfaces

---

## 5. Invariants

### 5.1 Phase-2 Invariants (New)

| ID         | Invariant                                                                    |
|------------|------------------------------------------------------------------------------|
| INV-P2-001 | Every primitive has exactly one scene_key, assigned before batcher insertion |
| INV-P2-002 | scene_key is immutable after batcher insertion                               |
| INV-P2-003 | Renderer iterates surfaces in compositor z-order, not arbitrary batch order  |
| INV-P2-004 | Batch lookup per surface is O(1) via HashMap                                 |
| INV-P2-005 | No O(S×B) filtering in renderer hot path                                     |
| INV-P2-006 | Scene graph has no surface awareness (surfaces are compositor/paint concern) |
| INV-P2-007 | Base surface (scene_key=1) is always present and visible                     |

### 5.2 Phase-1 Invariants (Unchanged)

| ID           | Status    | Invariant                                         |
|--------------|-----------|---------------------------------------------------|
| INV-COMP-P01 | Unchanged | Compositor orders surfaces by z-index             |
| INV-COMP-P03 | Unchanged | Compositor exposes immutable SurfaceStackSnapshot |
| INV-COMP-P04 | Unchanged | Surface stack is authoritative for draw order     |
| INV-COMP-N05 | Unchanged | Compositor does not own SceneGraph nodes          |
| INV-COMP-N11 | Unchanged | Compositor does not perform hit-testing           |
| INV-RG-002   | Unchanged | DrawPlanBuilder is sole consumer of SceneGraph    |

### 5.3 Phase-1 Assumptions Invalidated by Phase-2

| Assumption               | Phase-1                   | Phase-2                      |
|--------------------------|---------------------------|------------------------------|
| Single surface           | Enforced (MAX_SURFACES=1) | Multiple surfaces allowed    |
| scene_key always 1       | True                      | scene_key varies per surface |
| Flat batch list          | Yes                       | Partitioned by scene_key     |
| O(B) renderer            | Yes                       | O(S + B) but S is small      |
| AssertBaseContract bound | Required on build APIs    | Forbidden on Phase-2 APIs    |

### 5.4 Shared Attachment Sizing Invariant

**INV-RENDER-SHARED-ATTACHMENT**

Any renderer-owned attachment whose dimensions participate in scissor or viewport
validity MUST be either:
(a) owned per surface, or
(b) explicitly sized to the maximum configured dimensions across all registered surfaces.

Forbidden:
- sizing such attachments from `self.physical_width/height`
- deriving size from "last configured surface"

Violation is a renderer bug.

Cross-reference: SPEC-R-020 (GPU Scissoring) for scissor validity requirements.

---

## 6. Compile Boundaries

### 6.1 New APIs Requiring Phase2

```rust
// Only available with #[cfg(feature = "phase2")]
impl DrawPlanBuilder<Phase2> {
    /// Build with pre-partitioned batches
    pub fn build_partitioned(
        &mut self,
        scene: &SceneGraph,
        surface_stack: SurfaceStackSnapshot<Phase2>,
    ) -> DrawPlan<Phase2>;
}

impl Compositor {
    /// Snapshot for multi-surface rendering
    pub fn snapshot_phase2(&self) -> SurfaceStackSnapshot<Phase2>;
}

impl PaintContext {
    /// Enter a child surface region
    pub fn with_surface<F>(&mut self, scene_key: u64, f: F);
}
```

### 6.2 Phase-1 APIs Forbidden Under Phase2

| API                                                             | Phase-1   | Phase-2                          |
|-----------------------------------------------------------------|-----------|----------------------------------|
| `DrawPlanBuilder<P: AssertBaseContract>::build_with_surfaces()` | Required  | Forbidden (compile error)        |
| `Compositor::snapshot() -> SurfaceStackSnapshot<BaseContract>`  | Available | Returns BaseContract, not Phase2 |
| Single-surface debug assertions                                 | Active    | Disabled                         |

### 6.3 Type-Level Enforcement

```rust
// Phase-1 builder cannot accept Phase-2 snapshot
fn base_contract_build<P: AssertBaseContract>(
    builder: &mut DrawPlanBuilder<P>,
    snapshot: SurfaceStackSnapshot<P>,  // P must be BaseContract
) -> DrawPlan<P>;

// Phase-2 builder cannot satisfy AssertBaseContract
// impl AssertBaseContract for Phase2 {} // NOT IMPLEMENTED

// This fails to compile:
// let builder: DrawPlanBuilder<Phase2> = ...;
// builder.build_with_surfaces(...); // Error: Phase2 does not implement AssertBaseContract
```

---

## 7. Migration Path

### 7.1 Enabling Phase-2

1. Add `phase2 = []` to `[features]` in Cargo.toml
2. Uncomment `Phase2` struct in `compositor/phase.rs`
3. Implement `build_partitioned()` in DrawPlanBuilder
4. Update renderer to use partitioned batches
5. Add PaintContext surface binding

### 7.2 Backward Compatibility

- Phase-1 code continues to work unchanged
- Phase-1 APIs remain available (frozen)
- Phase-2 is additive, not replacement
- Feature flag isolates Phase-2 code paths

---

## 8. Open Questions

1. **Clip state across surfaces**: Do surfaces share clip state or reset?
2. **Text atlas partitioning**: One atlas or per-surface?
3. **GPU buffer layout**: Interleaved or per-surface regions?
4. **Overlay input routing**: How does hit-testing interact with surfaces?

These are deferred to implementation phase.

---

## 9. Summary

| Question                                        | Answer                                                                              |
|-------------------------------------------------|-------------------------------------------------------------------------------------|
| **Where** is a primitive assigned to a surface? | In PaintContext, before batcher insertion                                           |
| **When** is scene_key bound?                    | At widget paint time, immutable after batcher insertion                             |
| **By whom** is scene_key assigned?              | Compositor creates surfaces; widget tree binds content to surfaces via PaintContext |
| **How** does renderer consume?                  | Pre-partitioned HashMap lookup, O(1) per surface                                    |

Phase-1 remains frozen. This SPEC defines Phase-2 contracts without implementation.

---

## 10. SPEC Authority Boundaries

### 10.1 Ownership Table

| SPEC      | Owns                                                                        | Does NOT Own                                                      |
|----------|-----------------------------------------------------------------------------|-------------------------------------------------------------------|
| **081A** | Enforcement, guards, compile-time constraints, phase gates, STOP conditions | Batching internals, renderer contracts, overlay surface contract  |
| **081B** | Batching, scene_key partitioning, renderer contracts, DrawPlan structure    | Enforcement invariants (081A), overlay roles/lifetime (081C)      |
| **081C** | Overlay roles, provenance, lifetime, z-order, routing boundaries            | Batching (081B), enforcement (081A), hit-testing/focus (upstream) |

### 10.2 Cross-SPEC Reference Rules

| Reference Pattern | Allowed                                          | Forbidden                                               |
|-------------------|--------------------------------------------------|---------------------------------------------------------|
| 081B → 081A       | Reference as frozen base; cite invariants by ID  | Amend, relax, or redefine 081A invariants               |
| 081B → 081C       | Reference for overlay existence and surface role | Depend on overlay behavior, modal logic, dismissal      |
| 081C → 081A       | Reference as frozen base                         | Amend 081A                                              |
| 081C → 081B       | Reference for multi-surface availability         | Depend on batching internals or renderer implementation |

### 10.3 Invariant Namespace Ownership

Each SPEC owns distinct invariant prefixes. Cross-SPEC invariant duplication is FORBIDDEN.

| Prefix                 | Owner | Description                                 |
|------------------------|-------|---------------------------------------------|
| `INV-COMP-N*`          | 081A  | Compositor negative authority               |
| `INV-COMP-P*`          | 081A  | Compositor positive authority               |
| `INV-OVERLAY-*`        | 081A  | Overlay hardening (high-level enforcement)  |
| `PRE-MW-*`             | 081A  | Multi-window preconditions                  |
| `INV-MW-*`             | 081A  | Multi-window invariants                     |
| `STOP-01` to `STOP-09` | 081A  | 081A stop conditions                        |
| `INV-P2-*`             | 081B  | Phase-2 specific invariants                 |
| `INV-SURF-ROLE-*`      | 081C  | Surface role classification                 |
| `INV-PROV-*`           | 081C  | Surface provenance                          |
| `INV-LIFE-*`           | 081C  | Surface lifetime                            |
| `INV-ZORD-*`           | 081C  | Z-order constraints                         |
| `INV-ROUTE-*`          | 081C  | Routing integration                         |
| `INV-WIDGET-*`         | 081C  | Widget semantics (zero at compositor level) |
| `STOP-081C-*`          | 081C  | 081C stop conditions                        |

### 10.4 Authority Question Test

A reader should be able to answer: *"If this breaks, which SPEC is violated?"*

| Symptom                            | Violated SPEC                       |
|------------------------------------|------------------------------------|
| Compositor mutates widget geometry | 081A (INV-COMP-N01)                |
| Batch has wrong scene_key          | 081B (INV-P2-001, INV-P2-002)      |
| Overlay outlives parent            | 081C (INV-LIFE-07), 081A (STOP-05) |
| O(S×B) filtering in renderer       | 081B (INV-P2-005)                  |
| Overlay z-index below base         | 081C (INV-ZORD-01, INV-ZORD-02)    |
| Compositor performs hit-testing    | 081A (INV-COMP-N11, STOP-08)       |

## Scoped reopening for HAL-385 S1 Compile refusal and output custody (2026-11-13)

HAL-385 section8 explicitly reopens only the following closed 081B renderer boundaries for the independently authorized real-backdrop material. Compile entry points evolve from wgpu::SurfaceError to RendererCompileError::{Surface(wgpu::SurfaceError),FrostedGlass{surface_id,node_key,reason}}. Existing surface-error handling is preserved under the Surface variant; exact appearance/graph/resource-policy refusals are no longer mislabeled as native surface loss. All actual repo callers and checked pattern matches migrate together.

**Binding narrow owner decision — 2026-11-17:** Compile returns `Result<RendererCompileOutcome<T>, RendererCompileError>`, where the separate outcome is `Ready(T)`, `NoTargets` or `Pending(preparation identity)`. The exact error enum remains Surface/FrostedGlass. Pending is admitted only for browser frosted-allocation error-scope qualification and cannot be encoded, submitted, presented, treated as NoTargets, or projected as a false surface error. The existing renderer owns the suspended preparation; one-shot completion uses the existing App event path and revalidates current identity/target/device before use. HAL385 section8.3 defines readiness, real OutOfMemory handling and cancellation; section9's sealed command/backing custody remains unchanged. No compositor routing, target acquisition, widget or input authority is reopened.

HAL-385 section 9 supersedes the initial S1 raw-command exposure clause after an actual replacement-command counterexample. MultiTargetRenderOutput owns one private non-Clone EncodedTargetCommands value pairing the original command and its exact transient use. Direct public mutable command_buffer access is removed; surface_id and surface_output retain their existing meanings. Ordinary submission/cancellation consumes the sealed original. Explicit consuming into_raw_parts preserves raw-command capability but marks its frost backing Unconfirmed before extraction; rewrapping through the ordinary no-custody constructor cannot restore that authority. All affected in-repository callers migrate together. This is an explicit source-API change, not claimed source compatibility. The internal SDF output forwards the same sealed value; neither SurfaceId lookup nor a separately retained lease can authenticate a replacement command. The exact methods, queue validation, failure handling and proof are binding in HAL-385 section 9.

MultiTargetDrawPlan remains a declarative per-surface description with no acquired swapchain texture or physical frost resource ownership. SurfaceId/SceneKey, compositor order, target registration, acquisition and presentation responsibilities do not change. No cross-surface capture, alternate compositor or relaxation of the closed multi-window invariants is adopted. The rest of HAL-081B remains closed.
