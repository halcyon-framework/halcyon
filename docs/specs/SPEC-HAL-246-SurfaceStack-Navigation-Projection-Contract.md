# SPEC-HAL-246: Navigation Context Projection Contract

**Status:** Binding
**Series:** SPEC-HAL (Navigation)
**Depends On:** SPEC-HAL-232, SPEC-HAL-233, SPEC-HAL-234, SPEC-HAL-235, SPEC-HAL-236, SPEC-HAL-237
**Layer:** Core / Navigation → Runtime Projection Composition
**Authority Owner:** NavigationSurfaceOrchestrator for one navigation context

---

## 1. Purpose

Define the read-only navigation-only projection produced by one `NavigationSurfaceOrchestrator`.

The projection contains only state whose authority belongs to that navigation context. Focus and overlay testimony come from their own authorities and are composed with one or more context projections by the runtime contract in SPEC-HAL-248. `SurfaceStack` remains a passive mapping component and never receives authority from this projection.

---

## 2. Naming

**NavigationProjection** (one navigation context)

Justification: The struct projects one orchestrator's committed navigation-surface state into a mechanically composable shape. It is not a complete frame projection.

---

## 3. Projection Struct Definition

```rust
pub struct NavigationProjection {
    /// Ordered navigation surfaces from path bottom to top.
    /// Index 0 = root surface. Index N-1 = active navigation surface.
    /// Directly sourced from the orchestrator's committed projection_path
    /// through entry_surface_map in path order.
    pub navigation_surfaces: Vec<CompositorSurfaceId>,

    /// Currently active (topmost visible) navigation surface.
    /// Directly sourced from orchestrator's active_surface field.
    /// A live frame-bound context must provide Some and that ID must occur in
    /// navigation_surfaces. None is lawful only outside a live frame binding.
    pub active_surface: Option<CompositorSurfaceId>,
}
```

### Field Sourcing

| Field | Orchestrator Source | Transformation |
|---|---|---|
| `navigation_surfaces` | committed `projection_path` → `entry_surface_map` lookup per complete entry | Order preserved; any host/path divergence or missing mapping fails the complete projection |
| `active_surface` | committed surface mechanics `active_surface` | Direct copy after exact last-surface validation |

Every field is directly sourced from the same context-local navigation authority. `overlay_surfaces` and `focused_surface` are intentionally absent: overlay composition belongs to `OverlayDomainCoordinator` plus its admitted owner port under SPEC-HAL-236, and focus belongs to the central Focus Resolver under SPEC-HAL-235.

---

## 4. Access API

Single pull-based accessor on `NavigationSurfaceOrchestrator`:

```rust
pub fn project(&self) -> Result<NavigationProjection, NavigationProjectionError>
```

### Access Rules

- Pull-only. Caller requests projection when needed.
- No callbacks.
- No observers.
- No events.
- No subscriptions.
- No change detection.
- Returns owned value. No lifetime coupling to orchestrator.
- May be called zero or more times per frame.
- An unavailable or transaction-busy source, committed host/projection-path divergence, missing path-to-surface mapping, or invalid active-surface relation returns explicit `NavigationProjectionError`; no partial context projection is returned.
- Two consecutive calls with no intervening mutation return identical results.

### Multiroot Access

For `NavigationRootRegistry` consumers, call `registry.root(id)?.project()?` for every root explicitly named by the application-owned `FrameProjectionBinding` in SPEC-HAL-248. No registry-level API selects visible roots or their order. Each root projects independently; runtime glue mechanically preserves the binding's declared context order when it composes the frame projection.

---

## 5. Binding Invariants

**INV-246-01: Zero Authority**
NavigationProjection has zero authority. It cannot cause any state change in NavigationHost, NavigationSurfaceOrchestrator, Compositor, or any other system.

**INV-246-02: Snapshot Validity**
NavigationProjection is a snapshot, not a reference. The returned struct is owned; the data it contains (`CompositorSurfaceId` values) may become stale if the orchestrator mutates after projection and therefore must not be retained across frame-composition pulls.

**INV-246-03: No Encoded Behavior**
NavigationProjection must not encode behavior. It contains identifiers and ordering. It does not contain callbacks, closures, trait objects, channels, or any executable content.

**INV-246-04: No Collapsed Authorities**
Every field maps to the one context-local `NavigationSurfaceOrchestrator`. Focus and overlay fields are forbidden because adding them would transfer or duplicate authority owned by SPEC-HAL-235/236.

**INV-246-05: Frame Reconstructibility**
NavigationProjection or the same explicit `NavigationProjectionError` can be reconstructed every frame by calling `project()`. No information is lost between frames. No accumulation is required. No prior projection is needed to compute the next one.

**INV-246-06: Idempotent Read**
Calling `project()` has no side effects. It does not mutate the orchestrator. It does not allocate compositor resources. It does not change focus, z-index, visibility, or any other state.

**INV-246-07: Context-Local Projection**
One call returns exactly one navigation context. It does not inspect peer roots, choose which roots are visible, select an effective command root, or infer application layout. Multiroot frame composition is explicit under SPEC-HAL-248.

---

## 6. Forbidden Patterns

Runtime composition and `SurfaceStack` must NEVER:

1. **Infer active surface.** Use `projection.active_surface` directly. Do not scan `navigation_surfaces` to guess which is active.

2. **Infer focus or overlays.** A context projection contains neither. Runtime composition must obtain their independent projections from the owning authorities named by SPEC-HAL-248.

3. **Infer z-order meaning.** The order of `navigation_surfaces` is authoritative within its context. Do not assign z-indices, reorder, or interpret gaps. Context order and overlay order arrive separately under SPEC-HAL-248.

4. **Infer navigation intent.** The projection contains current state only. Do not compare projections across frames to infer direction, transitions, or intent.

5. **Mutate anything upstream.** SurfaceStack must not hold `&mut` references to orchestrator, compositor, or navigation host. The projection is the only data channel.

6. **Cache projections across mutations.** A projection is valid only until the next orchestrator mutation. Do not store projections and assume they remain correct.

7. **Reconstruct projection fields from other fields.** Do not derive `active_surface` from `navigation_surfaces.last()`, or derive focus/overlay state from any navigation field. Each authority must be read explicitly.

8. **Filter, reorder, or transform projection contents.** Render what is given. Do not skip surfaces. Do not change their order. Do not apply local policy to projection data.

---

## 7. Downstream Composition Boundary

With this contract:

- each `NavigationSurfaceOrchestrator` supplies one navigation-only projection,
- SPEC-HAL-248 runtime glue reads every context explicitly named by its `FrameProjectionBinding`,
- the same runtime operation reads overlay and focus testimony from their independent authorities,
- the compositor mechanically resolves surface IDs in the resulting `NavigationFrameProjection`,
- ScreenHost stores the one resolved frame view,
- renderer input remains the separate compositor-native `SurfaceStackSnapshot` path.

Neither runtime glue, `SurfaceStack`, the compositor, nor ScreenHost may infer missing authority testimony or select roots.

**Terminology note:** "Resolution" as used in this SPEC and in SPEC-HAL-247 §6 and SPEC-HAL-248 §10 means mapping/validation only: `Compositor::apply_frame_projection()` maps each navigation-surface `SurfaceId` to a `SurfaceDescriptor`, while active-navigation, overlay, and focus IDs are existence-validated and retained as IDs in `ProjectedSurfaceView`. It preserves context grouping and every declared order. Resolution does not infer, reorder, filter, silently skip, convert overlay/focus testimony into descriptors, or interpret navigation semantics.

---

## 8. Stability Declaration

This SPEC defines a closed navigation-authority boundary. The pull-only access API, context-local sourcing, and separation from focus/overlay authority are binding.

Extension requires an explicit amendment or successor SPEC. No change may add fields to `NavigationProjection` that merge authorities or introduce events, observers, or subscriptions to replace or supplement the pull-based `project()` API. The sealed projection-versus-rendering boundary remains governed by SPEC-HAL-232/247/248.

Guard tests in `src/tests/navigation_projection_boundary_guard.rs` enforce renderer/module isolation and the sole runtime projection call sites. Projection and lifecycle tests prove the navigation-only field boundary, transaction-busy rejection, and fallible no-partial projection.

---

## 9. Runtime Implementation Note (0.176.0-beta)

The live `NavigationProjection` is navigation-only and `project()` is fallible.
It rejects unavailable or transaction-busy sources, any divergence between the
committed host path and the orchestrator's projection path, any missing
path-to-surface mapping, and any invalid active-surface relation; it never
returns a partial context. SPEC-HAL-248 runtime glue pulls every explicitly
bound standalone or registered-root source under one domain guard, validates
registered-root active-surface association through `NavigationDomainRouter`,
composes independent overlay and focus testimony, and resolves the complete
multicontext frame through `Compositor::apply_frame_projection()`.

A versioned frame binding retains one unique source participation claim per
named orchestrator. Every application-admitted root also retains permanent
same-domain participation from hidden admission through destruction, including
intervals with no frame binding. While any live or prepared claim exists, an
operation that could change this projection must prepare both its NavigationHost
commit and surface publication with matching coordinator/domain guard testimony;
direct host or orchestrator compatibility dispatch therefore cannot race the
guarded pull or bypass application-owned root coordination.
