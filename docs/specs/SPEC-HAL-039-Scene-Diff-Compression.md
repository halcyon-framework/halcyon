# SPEC-HAL-039: Scene Diff Compression (GPU Partial Updates)

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Scope:** Renderer 2.0 (`src/renderer/*`)
**Purpose:** Minimize CPU→GPU bandwidth by updating only changed parts of the scene, instead of re-uploading full buffers every frame.

---

## 1. Purpose

Halcyon must handle:

* thousands of nodes,
* rapidly updating charts,
* blinking cursors, hover states, small UI tweaks

…without re-uploading the **entire** scene to the GPU every frame.

This SPEC defines **Scene Diff Compression**: a renderer-level mechanism that:

* compares the **previous** and **current** flattened scene,
* computes a **minimal set of buffer updates**,
* applies **partial** writes to GPU buffers,
* falls back safely to full rebuild when diffing is too complex.

UI widgets and `Scene` APIs remain unchanged. All logic lives in Renderer 2.0.

---

## 2. Motivation

Today (conceptually):

1. Widgets build a `Scene` tree.
2. Renderer flattens it into vertex/index/instance buffers.
3. On each frame, buffers are rebuilt and re-uploaded.

That is fine for small scenes, but unacceptable for:

* dense orderbooks,
* multi-chart layouts,
* high-frequency updates.

We want:

* **Stable structure** across frames (most of the scene doesn’t change),
* **In-place overwrites** for small changes (e.g. one text label),
* **Bounded work per frame** even with large scenes.

Scene Diff Compression is the bridge.

---

## 3. Requirements

### 3.1 Functional

1. **Same visual result** as full rebuild.
2. No changes to:

    * widget APIs,
    * `core::scene` public API,
    * event model.
3. Compatible with:

    * SPEC-HAL-005 (Scene Graph),
    * SPEC-HAL-027 (SceneGraph Batching),
    * SPEC-HAL-030 (Present Mode / Frame Pacing),
    * SPEC-HAL-029 (Glyph Atlas Cache).

### 3.2 Performance

1. CPU work per frame should be **O(k)** where **k** is number of changed batches, not total node count.
2. GPU uploads must be **localized** (range writes) when possible.
3. Full-buffer rebuild remains available but should be **rare**.

### 3.3 Safety / Simplicity

1. No partial updates to **in-flight** GPU resources (tied to SPEC-HAL-040).
2. Clear invariants: when diffing is too complex → fallback to full rebuild.
3. No speculative “smart” reordering that can break visual ordering or hit-testing.

---

## 4. Renderer 2.0 Context

Renderer modules (target layout):

```text
src/renderer/
  mod.rs              // Public renderer API
  gpu_state.rs        // Device, queues, swapchain, buffers
  pipelines.rs        // Pipeline creation, PipelineKey
  batcher.rs          // Scene → DrawBatch flattening
  clip.rs             // Clip stack, GPU clip state
  text.rs             // Text + glyph atlas integration
  present.rs          // Frame pacing, present modes, frame graph
```

Scene Diff Compression sits in:

* `renderer::batcher`   — defines `DrawBatch` & stable batch keys
* `renderer::gpu_state` — owns buffers and applies partial updates
* `renderer::present`   — coordinates frame lifecycle & double-buffering

---

## 5. Scene & Batch Model

### 5.1 Scene Flattening (recap from SPEC-HAL-027)

From SPEC-HAL-027, after batching we have:

```rust
/// Logical identity of a batch in a frame
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawBatchId(pub u32);

/// Key that defines “what” a batch is (not where)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DrawBatchKey {
    pub layer: u8,           // z-layer (background, widgets, overlay…)
    pub pipeline: PipelineKey,
    pub clip_chain: ClipChainId,
    pub texture: Option<TextureId>, // glyph atlas or UI textures
}

/// Range inside a GPU buffer
#[derive(Clone, Copy, Debug)]
pub struct BufferRange {
    pub vertex_offset: u32,  // in vertices
    pub vertex_count: u32,
    pub index_offset: u32,   // in indices
    pub index_count: u32,
}

/// Flattened render batch
#[derive(Clone, Debug)]
pub struct DrawBatch {
    pub id: DrawBatchId,
    pub key: DrawBatchKey,
    pub range: BufferRange,
}
```

The flattening step guarantees:

* **Stable ordering**: batches sorted by `(layer, pipeline, clip, texture)`.
* Each batch corresponds to a **contiguous** range in vertex/index buffers.

### 5.2 Frame State

We define a per-frame snapshot of what the GPU currently “expects”:

```rust
pub struct FrameBatches {
    pub batches: Vec<DrawBatch>,
    pub total_vertices: u32,
    pub total_indices: u32,
}

/// One frame’s GPU-oriented scene state
pub struct GpuFrameState {
    pub frame_id: u64,
    pub batches: FrameBatches,
    // Pointer into GPU buffers owned by gpu_state.rs
}
```

Renderer maintains:

```rust
pub struct RendererState {
    pub current: Option<GpuFrameState>, // last submitted
    pub staging: GpuFrameState,         // being built now
}
```

Scene Diff Compression compares `current.batches` vs `staging.batches`.

---

## 6. Diff Model

### 6.1 Update Types

We define a minimal set of GPU operations:

```rust
pub enum GpuUpdate {
    /// Overwrite a contiguous range of vertex and/or index data in-place
    OverwriteRange {
        vertex_offset: u32,  // in vertices
        vertex_data: Vec<u8>,
        index_offset: u32,   // in indices
        index_data: Vec<u8>,
    },

    /// Rebuild from scratch (fallback)
    FullRebuild {
        vertex_data: Vec<u8>,
        index_data: Vec<u8>,
    },
}
```

We **intentionally** do not try to implement clever inserts/removals yet.
First version:

* If structural changes break assumptions → `FullRebuild`.
* Otherwise → `OverwriteRange` per batch (or per contiguous batch block).

### 6.2 Structural Invariants for Partial Updates

Partial updates are allowed only when all are true:

1. `current.batches.len() == staging.batches.len()`
2. For each `i`:

    * `current.batches[i].key == staging.batches[i].key`
    * `current.batches[i].range.vertex_count == staging.batches[i].range.vertex_count`
    * `current.batches[i].range.index_count == staging.batches[i].range.index_count`

This means:

* Same number of batches,
* Same visual “type” of each batch,
* Same geometry footprint per batch.

Only **content** (colors, vertex positions, UVs) may differ.

If any of these invariants fails, **we fall back to FullRebuild** for v1.

---

## 7. Diff Algorithm

### 7.1 High-Level Flow

In `renderer::present`:

```rust
fn prepare_frame(&mut self, new_batches: FrameBatches) -> GpuUpdate {
    match &self.current {
        None => {
            // First frame: build from scratch
            GpuUpdate::FullRebuild { vertex_data, index_data }
        }
        Some(prev) => {
            diff_batches(&prev.batches, &new_batches)
        }
    }
}
```

### 7.2 `diff_batches` Logic

```rust
fn diff_batches(prev: &FrameBatches, curr: &FrameBatches) -> GpuUpdate {
    // 1. Quick structural check
    if prev.batches.len() != curr.batches.len() {
        return full_rebuild(curr);
    }

    // 2. Validate per-batch invariants
    for (a, b) in prev.batches.iter().zip(curr.batches.iter()) {
        if a.key != b.key {
            return full_rebuild(curr);
        }
        if a.range.vertex_count != b.range.vertex_count ||
           a.range.index_count  != b.range.index_count
        {
            return full_rebuild(curr);
        }
    }

    // 3. At this point structure is identical.
    //    We can safely reuse existing GPU buffers, writing only updated ranges.
    //    Implementation detail: we still need CPU-side packed buffers for `curr`.

    // v1 strategy: always overwrite ALL ranges in-order:
    // - Same layout, same sizes → single big memcpy
    // - Later: we can skip unchanged ranges via hashing
    full_overwrite_same_layout(curr)
}
```

### 7.3 v1 Strategy: Layout-Identical Overwrite

Instead of trying to be clever about “only changed batches,” v1 uses:

* **Single large overwrite** when structure matches:

```rust
fn full_overwrite_same_layout(curr: &FrameBatches) -> GpuUpdate {
    // Pack curr’s vertex/index data into contiguous buffers.
    // Layout matches previous, so offsets do not change.

    let (vertex_data, index_data) = pack_batches(curr);

    GpuUpdate::OverwriteRange {
        vertex_offset: 0,
        vertex_data,
        index_offset: 0,
        index_data,
    }
}
```

This gives:

* Same benefit as full rebuild in terms of transferred bytes,
* But keeps future open for **eventual per-batch diffs** (hashing, range skips),
* And doesn’t require rewriting higher-level code.

Later optimization (not mandatory for this SPEC):

* Compute per-batch hash,
* Group consecutive “changed” batches,
* Emit `OverwriteRange` per group,
* Skip untouched areas.

---

## 8. GPU Application of Updates

In `renderer::gpu_state`:

```rust
impl GpuState {
    pub fn apply_update(&mut self, update: GpuUpdate) {
        match update {
            GpuUpdate::FullRebuild { vertex_data, index_data } => {
                self.recreate_buffers(vertex_data, index_data);
            }
            GpuUpdate::OverwriteRange {
                vertex_offset,
                vertex_data,
                index_offset,
                index_data,
            } => {
                self.write_vertices(vertex_offset, &vertex_data);
                self.write_indices(index_offset, &index_data);
            }
        }
    }
}
```

**Precautions:**

* Buffers must be **large enough** to hold `curr.total_vertices` / `curr.total_indices`.
* If `curr` exceeds current capacity → ignore diff and force `FullRebuild` with larger buffers.
* No writes to buffers that are used by an in-flight frame:

    * either double-buffer frame data,
    * or synchronize with fences (see SPEC-HAL-040).

---

## 9. Interaction with Other SPECs

### 9.1 SPEC-HAL-027 — SceneGraph Batching

* Batching is responsible for:

    * stable ordering,
    * consistent grouping (same `DrawBatchKey` → same batch semantics).
* Scene Diff Compression relies on this stability:

    * if batching rules change → expect more `FullRebuild` fallsbacks.

### 9.2 SPEC-HAL-029 — Glyph Atlas Cache

* Text batches depend on textures (glyph atlas).
* If atlas re-packs or resizes:

    * texture coordinates and batch layouts can change drastically,
    * often forcing **FullRebuild** for the frame.
* This SPEC does not try to be smart about atlas changes; it simply detects structure mismatch and rebuilds.

### 9.3 SPEC-HAL-030 — Present Mode / Frame Pacing

* Diff compression must not stall the GPU:

    * prefer doing diff on CPU **while** previous frame is rendering.
* Present module chooses:

    * when to perform diff,
    * whether to skip frames under high load (frame pacing).

### 9.4 SPEC-HAL-040 — GPU Resource Lifetime Management

* This SPEC depends on SPEC-HAL-040 to define:

    * how many frame buffers exist,
    * when it’s safe to overwrite them,
    * how to handle device-lost and buffer invalidation.

---

## 10. Failure Modes & Fallbacks

Scene Diff Compression must err on the side of **correctness**.

Fallback situations:

1. Batch count differs between frames.
2. Any batch key changes (different pipeline, texture, clip, layer…).
3. Vertex/index counts differ per batch.
4. Total vertex/index requirements exceed current buffer capacity.
5. Glyph atlas changes cause inconsistent texture regions.

In all cases, this SPEC mandates:

> Fallback to `GpuUpdate::FullRebuild`.

No partial or “best-effort” updates are allowed in those states.

---

## 11. Implementation Checklist

This section is **explicitly for Claude Code** when implementing.

### 11.1 Types & Structs

* [ ] Add `DrawBatchKey`, `DrawBatchId`, `BufferRange`, `DrawBatch` to `renderer::batcher` (if not already defined by SPEC-HAL-027).
* [ ] Add `FrameBatches` and `GpuFrameState` to `renderer::batcher` or `renderer::gpu_state` (whichever is central for frame data).
* [ ] Add `GpuUpdate` enum to `renderer::gpu_state` (GPU-facing layer).

### 11.2 Renderer State

* [ ] Extend `RendererState` (probably in `renderer::mod` or `present.rs`) with:

    * `current: Option<GpuFrameState>`
    * `staging: GpuFrameState`

### 11.3 Diff Function

* [ ] Implement `fn diff_batches(prev: &FrameBatches, curr: &FrameBatches) -> GpuUpdate` with:

    * Structural equality checks (length, keys, range sizes).
    * `full_rebuild(curr)` fallback.
    * `full_overwrite_same_layout(curr)` path for identical layouts.

* [ ] Implement `full_rebuild(curr)`:

    * Packs vertex/index data for `curr` into fresh buffers.
    * Returns `GpuUpdate::FullRebuild`.

* [ ] Implement `full_overwrite_same_layout(curr)`:

    * Packs vertex/index data in the exact same layout as previous.
    * Returns `GpuUpdate::OverwriteRange` with offset = 0.

### 11.4 GPU Application

* [ ] In `renderer::gpu_state`, implement:

    * `fn recreate_buffers(&mut self, vertex_data: Vec<u8>, index_data: Vec<u8>)`
    * `fn write_vertices(&mut self, vertex_offset: u32, data: &[u8])`
    * `fn write_indices(&mut self, index_offset: u32, data: &[u8])`
* [ ] Implement `fn apply_update(&mut self, update: GpuUpdate)` that calls the above.

### 11.5 Integration

* [ ] In `renderer::present`, after scene flattening:

    * Build `FrameBatches` from the flattened scene.
    * If `current.is_none()` → `FullRebuild`.
    * Else → `diff_batches(&current.batches, &staging.batches)`.
    * Call `gpu_state.apply_update(update)`.

* [ ] After successful rendering:

    * Swap `staging` into `current` (or copy as needed).

### 11.6 Safety Checks

* [ ] Ensure buffer capacity checks:

    * If `curr.total_vertices > gpu_state.vertex_capacity` → `FullRebuild` with resized buffers.
* [ ] Do not introduce any new panics in hot paths:

    * Treat unexpected mismatches as “rebuild and log once,” not crash.

---

## 12. Future Extensions (Out of Scope for This SPEC)

Not required for this SPEC, but we may add later:

1. **Per-batch hashing**

    * Skip uploads when batch content is identical.
2. **Multi-range updates**

    * Group consecutive changed batches into a single `OverwriteRange`.
3. **Dynamic batch insertion/removal**

    * For advanced scenes where batch counts change frequently but many reuse can still happen.

For now, **strict correctness** and **stable foundation** matter more than maximum cleverness.

---

End of SPEC-HAL-039.
