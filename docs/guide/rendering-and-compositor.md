---
doc-id: halcyon-guide-rendering
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-005
  - SPEC-HAL-027
  - SPEC-HAL-028
  - SPEC-HAL-030
  - SPEC-HAL-354
---

# Rendering and Compositor

Scene-to-GPU path at revision `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`.
Two-axis claim tags per SPEC-HAL-309 §7. Frame admission and scene
composition are covered in `runtime-and-frame-lifecycle.md`; this document
starts where a presented scene exists.

## 1. Ownership and Data Flow

```text
widgets paint into SceneBuilder            (src/core/scene/mod.rs:1218)
    -> SceneGraph per surface              (src/core/scene/mod.rs:1096)
    -> SceneGraph diff + SceneCache        (src/core/app.rs:11200-11238, SPEC-HAL-083)
    -> DrawPlanBuilder::build_multi()      (src/renderer/draw_plan_builder.rs:12)
    -> DrawPlan (CPU-only)                 (src/renderer/draw_plan_builder.rs:14)
    -> WgpuRenderer::compile_multi() + render_multi()
                                           (src/renderer/draw_plan_builder.rs:16)
```

The builder's own invariants define the boundary
[implementation: implemented — `src/renderer/draw_plan_builder.rs:19-26`]:

- INV-RG-002: `DrawPlanBuilder` is the ONLY consumer of `SceneGraph`.
- INV-SCENE-001: SceneGraphs are surface-scoped.
- INV-SCENE-002: no cross-surface background/text dependency.
- `WgpuRenderer` never imports or sees `SceneGraph`.
- `DrawPlan` can be constructed without GPU initialization.
- The batcher is owned by the builder, not the renderer.

Per-surface scene ownership is move-only — "SceneGraphs must NEVER be cloned
across surfaces" [implementation: implemented — `src/core/app.rs:2041-2052`,
`:11245-11259`].

## 2. Scene Model

- `SceneGraph { root: Vec<SceneNode>, clear_color, key_widget_map }` with
  key lookup and recycled-storage conversion
  [implementation: implemented — `src/core/scene/mod.rs:1096-1161`].
- `SceneBuilder` owns the emission surface: primitive pushes
  (`push_rect`, `push_border_rect`, `push_rounded_rect`, `push_text`,
  `push_text_with_backgrounds`, `push_text_probe`, `push_image`), clip stack
  (`begin_clip`/`end_clip`/`with_clip`), scroll stack
  (`begin_scroll`/`end_scroll`), and identity: every keyed emission is
  tracked (`emitted_keys`, `key_to_widget`,
  `set_current_painting_widget`), with key-uniqueness asserted
  [implementation: implemented — `src/core/scene/mod.rs:1218-1843`,
  `assert_key_unique` at `:1417`].
- Retained fragments enter the scene as immutable `Arc<[SceneNode]>`
  children via `push_retained_fragment`, and their key→widget maps merge in
  [implementation: implemented — `src/core/scene/mod.rs:1342-1356`];
  the `SceneNode::RetainedFragment` variant is traversed by downstream
  consumers [implementation: implemented —
  `src/renderer/draw_plan_builder.rs:43-50`].
- The scene structure model (layers/clips/z-order, "Layers never clip each
  other. Clips only affect their subtree") is SPEC-HAL-005
  [authority: draft-intent — SPEC-HAL-005:5 (`Status: Draft`), :67-86].
  Guard modules exist beside the scene (`src/core/scene/guard.rs`,
  `src/core/scene/forbidden.rs`).

## 3. Identity Diff and Scene Cache

At the UI→render handoff of a presented frame, the loop extracts scene keys,
computes an identity-based delta, debug-asserts identity stability, then
dispatches cache operations — diff strictly before render
[implementation: implemented — `src/core/app.rs:11200-11238`, symbols
`extract_scene_keys`, `compute_diff_from_keys`, `dispatch_cache_operations`;
authority tags in code: SPEC-HAL-083 Phase-4/11/12/16]. Cache consumers and
`SceneCacheExecutionMode` are constructed with the loop
[implementation: implemented — `src/core/app.rs:9333`, imports `:28-31`].

## 4. Batching

The batcher is deterministic by construction
[implementation: implemented — `src/renderer/batcher.rs:1-40`]:

- Unified clip stack: push clips intersect with the parent (shrinking or
  equal), empty intersections become zero-area rects, clip hash is
  deterministic (SPEC-R-001 Stages 1-4 complete per module doc).
- Ordering: every primitive gets a monotonic `order_key`; primitives sort by
  `(z_layer ASC, order_key ASC)` before batching, making output independent
  of traversal order (SPEC-R-002 Stages 1-4 complete per module doc).
- Path instances sort by
  `(z_index, clip_id, stroke_flag, debug_mode, insertion_order)`
  (SPEC-R-004 Stage 18 per module doc).

Per-function surface, verified 2026-07-24 (`src/renderer/batcher.rs`):
clip/scroll state (`push_clip` `:924`, `pop_clip` `:980`,
`push_scroll`/`pop_scroll` `:990`/`:997`, `clip_state`/`clip_hash`
`:1033`/`:1039`); one `add_*` ingress per primitive family — rect
`:1097`, border rect `:1124`, rounded rect/border `:1158`/`:1220`, text
`:1301`, image `:1334`, circle `:1366`, ellipse `:1393`, line `:1416`,
polyline `:1453`, shape `:1494`, shadow `:1501`, gradient `:1577`,
color-picker SV `:1637`, radial gauge `:1660`, path `:1791`; output
accessors `primitives`/`path_draws`/`path_clip_rects` `:2357-2367`.
Batch identity is `DrawBatchKey` with per-pipeline constructors
(`:333-395`) over `PipelineId` (`:247`); frame output is `FrameBatches`
(`:695-743`) [implementation: implemented].

SPEC-HAL-027's strict rules (batch only identical pipeline state; clip
change breaks batches; z-order preserved; text through the glyph atlas;
rect grouping by fill/stroke; no batch reopening)
[authority: binding-as-declared (`Status: Partially Implemented`,
SPEC-HAL-027:5) — SPEC-HAL-027:101-148]. Its own status section scopes the
implementation: `batcher.rs` + `wgpu_renderer.rs` exist; image batching,
advanced instancing, and layer-based compositing are **not** implemented
[authority: as above — SPEC-HAL-027:13-22 | implementation: partial].

## 5. GPU Pipelines

Live `WgpuRenderer` state (fields, `src/renderer/wgpu_renderer.rs:582-645`):
device/queue (`Arc`), surface format, a `default_present_mode`, and
per-material pipelines — rect, shadow, gradient, color-picker SV,
radial-gauge, optional SDF rect, and a `ShapePipeline`
[implementation: implemented]. A `pipelines` module holds the analytic-SDF
shape/path/text pipeline contracts (SPEC-R-003)
[implementation: implemented — `src/renderer/pipelines/mod.rs:1-9`].

Two-axis caution on SPEC-HAL-028 (modular shader pipeline registry): its
declared status is `Design – Not Implemented` and its status text says the
renderer uses "basic inline WGSL shaders for rect and text pipelines"
[authority: design-intent — SPEC-HAL-028:5, :14-16]. The live field
inventory above shows substantially more than rect+text, while the
*registry* described by the SPEC is still not in evidence at the cited
lines — the pipelines are hand-assembled fields. Classified: registry
`absent`; the SPEC's own status prose is stale about the pipeline inventory
(recorded for audit).

Supporting renderer infrastructure
[implementation: implemented — `src/renderer/mod.rs:1-39`]: batcher,
compiled draw plan (`CompiledDrawPlan`, `MultiTargetDrawPlan`), custom
render passes, glyph atlas, `gpu_arena`, mesh cache, path renderer, a
render-graph module (`RenderGraphBuilder`, `RenderPassNode`,
`CompiledGraph`, …), render targets (`RenderTargetRegistry`,
`SurfaceBundle`), staging pool, vertex types.

Resource lifetime is governed by the SPEC-HAL-040 frame epoch: a
renderer-local, strictly monotonic counter advancing by exactly 1 per
successful `render_multi()` frame, only after `queue.submit()` and
`output.present()` succeed; it tags creation/last-used/safe-destruction
epochs [implementation: implemented — `src/renderer/mod.rs:41-60`].

## 6. Compositor Boundary

The compositor is a closed subsystem: "Multi-Window Architecture (MWA)
layer is procedurally closed" as of 2026-01-21; modification requires
explicit SPEC amendment [implementation: implemented (declaration in
module doc) — `src/compositor/mod.rs:1-14`].

Authority is enumerated in the module doc
[implementation: implemented — `src/compositor/mod.rs:22-41`]:

- MUST NOT: hit-test (INV-COMP-N11); decide capture/dismissal/focus/
  modality (N12); mutate widget geometry (N01); participate in layout
  (N02); resolve styles/themes (N03); own widgets (N04); own SceneGraph
  nodes (N05); generate visuals not backed by surfaces/layers (N06).
- MAY ONLY: order surfaces by z (P01); composite layers (P02); expose
  ordered SurfaceStack descriptors (P03); hand off to RenderGraph/swapchain
  (P04); track surface lifetime (P05); surface-level occlusion (P06);
  expose bounds/clips/transforms (P07).

Contracts: `BaseContract` (SPEC-HAL-081A, single base surface,
`snapshot()`) vs `MultiSurfaceContract` (SPEC-HAL-081B, `snapshot_multi()`)
[implementation: implemented — `src/compositor/mod.rs:16-20`, `:60-63`].
Contract internals, verified 2026-07-24 (`src/compositor/contract.rs`):
each marker implements `Contract { NAME, MAX_SURFACES }` —
`BaseContract` = "current-contract"/1 (`:142-147`),
`MultiSurfaceContract` = "multi-surface-contract"/`usize::MAX`
(`:168-173`); bound-restriction traits `AssertBaseContract` /
`AssertMultiSurfaceContract` (`:186-197`); the single-surface invariant
panics in both debug and release
(`assert_base_contract_surface_count`, `:211-219`); `Contracted<P>` is a
zero-cost `PhantomData` marker carrying the contract at type level
(`:241-269`); `SceneKey(NonZeroU64)` (`:55`)
[implementation: implemented].
Lockdown: `SurfaceStack` is `pub(crate)`; the only externally renderable
surface data is a `SurfaceStackSnapshot` [implementation: implemented —
`src/compositor/mod.rs:73-76`]. SPEC-HAL-081C role types compile but are
"NOT embedded in current contract structs … unused until future SPEC
implementation begins" [implementation: present-but-dormant —
`src/compositor/mod.rs:78-86`].

The production loop instantiates `DrawPlanBuilder<MultiSurfaceContract>`
[implementation: implemented — `src/core/app.rs:9135-9137`].

## 7. Presentation Policy

- What exists: a fixed `default_present_mode` on the renderer
  [implementation: implemented — `src/renderer/wgpu_renderer.rs:590`] and
  the frame-level presentation gate (unchanged frames are not presented;
  `runtime-and-frame-lifecycle.md` §4 step 17).
- What does not: `PresentIntent`, explicit present-mode selection,
  swapchain mode switching, platform-specific overrides — SPEC-HAL-030's
  own §10 declares them Not Implemented
  [authority: SPEC-HAL-030:5 (`Status: Design — Partially Implemented`) |
  implementation: absent — SPEC-HAL-030:224-230].

No performance numbers appear in this document: no benchmark or runtime
measurement was captured at this revision (any would be `unknown`).

## 8. Extension Seams and Forbidden Crossings

- Extend drawing by emitting scene primitives through `SceneBuilder` /
  `PaintSink` — not by touching the renderer from widgets
  (INV-RG-002, §1).
- Custom GPU work goes through `CustomRenderPass` / render-graph types
  re-exported at `src/renderer/mod.rs:21-39`; Plot demonstrates the
  offscreen-texture pattern (`src/plot/mod.rs:1-8`, covered in the pending
  plot guide).
- Never clone SceneGraphs across surfaces; never consume SceneGraph outside
  DrawPlanBuilder; never hit-test or resolve styles in the compositor
  (§1, §6).
