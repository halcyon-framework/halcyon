# **SPEC-R-001: Clip Stack Rework**

**Author:** JC
**Date:** 2025-11-24
**Status:** Implemented (Stages 1-4 Complete)
**Scope:** Overhaul the clip stack in the renderer
**Purpose:** Fix incorrect clip propagation, prepare for per-pass modular pipelines, and enable high-performance nested clipping for scrolling, panels, overlays, charts, and complex composite widgets.

---

## Implementation Status

**Fully Implemented** (2025-11-30)

### Stage 1: CPU Clip Stack Formalization
- Introduced `ClipState` struct with `rect: RectF` and `hash: u64`
- Unified clip stack semantics: push intersects with parent, empty intersections produce zero-area RectF
- Deterministic hash based on stack depth and rect f32 bit components
- Debug assertions for clip stack invariants (balanced push/pop)
- Files: `src/renderer/batcher.rs`, `src/core/scene.rs`

### Stage 2: Batch + Renderer ClipState Wiring
- Changed `FlatPrimitive.clip_rect: Option<RectF>` to `FlatPrimitive.clip_state: ClipState`
- Changed `DrawBatch.clip_rect` to `DrawBatch.clip_state`
- Updated `BatchBuilder` to use `ClipState`
- All `add_*` methods capture clip state at insertion time
- Files: `src/renderer/batcher.rs`, `src/renderer/wgpu_renderer.rs`

### Stage 3: Canonical Scissor Path
- All GPU scissor state flows through `clip_rect_to_scissor()`
- Debug assertion: `batch.clip_state.hash == batch.key.clip_hash`
- Text path assertions for index bounds
- Comprehensive documentation of scissor paths
- Files: `src/renderer/wgpu_renderer.rs`

### Stage 4: Final Cleanup
- Audited all `clip_rect` references (all valid: TextQuad, widget API, helpers)
- Module doc updated to document all stages
- Added `test_SPEC_r001_batch_invariants` test (17 total batcher tests)
- No legacy clip_rect fields remain in batch/primitive types

---

## 1. Problem Statement

Halcyon’s current clip model is minimal and fragile:

* Clipping is applied directly during scene emission.
* Nested clips do not propagate safely.
* Some widgets (ScrollArea, Container) rely on “local hacks” to simulate clipping.
* There is no GPU-side clip stack; each draw call injects clip state inline.
* Scene batching conflicts with the current clipping approach.

This prevents:

* Proper independent scroll regions
* Nested clipping (scroll inside panel inside tab inside dock)
* GPU batching (clip invalidates batch groups)
* Modular shader pipelines
* Complex charting overlays with masked layers

We need a real clip stack.

---

## 2. Goals

1. **Introduce a global clip stack**
   Pushed/popped during scene graph traversal.

2. **Store clips in scene nodes**, not widget code
   Widgets only request clip; they don’t apply it.

3. **Renderer maintains its own GPU clip state**
   No more inline mutation per draw call.

4. **Clip rects operate strictly in absolute logical coordinates**
   No “relative to widget” ambiguity.

5. **Support nested clipping with no ambiguity**
   Correct push/pop behavior at any depth.

6. **Clips integrate cleanly with batching**
   Batching is segmented by clip hash.

7. **Clips propagate into shader pipelines**
   Required for future pipelines: rounded rects, blur, shadows.

---

## 3. Non-Goals

* No CSS-style overflow models
* No complex mask layers (future: pipeline pass)
* No stencil buffer for now — WGPU has performance pitfalls

---

## 4. Design

### 4.1 ClipNode Scene Node

Add a new node type:

```
ClipNode {
    rect: RectF,
    radius: f32   // optional for future rounded clipping
}
```

Two kinds:

* `PushClip(rect, radius)`
* `PopClip`

### 4.2 Widget → Scene Emission

Widgets request clipping through:

```rust
cx.scene.push_clip(rect, radius);
...
cx.scene.pop_clip();
```

They **do not** draw clipped content by themselves.

### 4.3 Scene Graph Traversal

Traversal produces a flattened list:

```
[PushClip, Draw, Draw, PopClip, ...]
```

Batcher sees:

* `PushClip` = start new batch region
* `PopClip` = restore previous region

### 4.4 Renderer-side Clip Stack

Renderer maintains:

```
gpu_clip_stack: Vec<ClipState>
```

Each draw call uses the **current top**.

### 4.5 Shader Integration

Pipeline receives:

```
uniform ClipState {
    float x, y, w, h;
    float radius;
};
```

Applied inside vertex/fragment shader:

* reject fragment outside clip bounds
* support soft clipping for future pipelines

---

## 5. Implementation Plan

### Step 1 — Add ClipNode types

File: `src/core/scene.rs`

### Step 2 — Add push_clip/pop_clip to SceneBuilder

File: `src/core/scene.rs`

### Step 3 — Update widgets to use clipping

Minimal changes:

* ScrollArea
* Container
* Panel
* Legend (uses inner padding clipping)

### Step 4 — Flattened scene traversal now emits clip nodes

File: `src/core/app.rs`

### Step 5 — Renderer clip stack

File: `src/platform/wgpu_renderer.rs`

### Step 6 — Shaders accept clip uniforms

File: `src/platform/shaders/*.wgsl`

### Step 7 — Batching segmentation

File: `src/core/scene.rs` or `src/platform/wgpu_renderer.rs`
Batch key includes `clip_hash`.

---

## 6. Precautions

* Clip push/pop **must be balanced**.
  Add debug assertions in debug builds.

* Clip rects must always be **absolute logical**, never relative.

* All draw operations must assume clip uniform changes break batching.

* Rounded-rect clipping requires per-pixel test; must be opt-in.

* Continuous scene rebuilding must reuse immediately preceding clip/scroll child-vector storage
  after draw-plan consumption. Historical node payloads or unused vector pools must not accumulate.

---

## 7. Notes for Claude Code

**Do not write shaders yet.**
This SPEC only defines the CPU & scene-level architecture.

Claude must:

* Implement CPU-side scene changes
* Implement clip stack in renderer
* Integrate clip segmentation into batching
* Leave shader logic as stubs or no-op pass-through
