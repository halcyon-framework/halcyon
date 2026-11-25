**SPEC-HAL-015: Halcyon — High-Performance Caching & Diffing Layer**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Scene graph diffing, dirty-region tracking, GPU resource reuse, partial repaints.
**Primary Use-Case:** 120–240 FPS trading dashboards with heavy charting, DOMs, and multiple views.
**Adopted by:** SPEC-HAL-354 for binding value/paint invalidation, retained widget scene fragments, and proportional renderer preparation. This draft remains non-binding where it proposes incompatible or unimplemented detail.

Depends on:

* HAL-005 (Scene Graph 2.0: layers, z-order, clipping, batching)
* HAL-007 (State Model & View-Model Discipline)
* HAL-004/014 (event loop & advanced input)

---

# 1. Goal

Make Halcyon capable of **hard real-time feeling** UIs:

* 120–240 FPS on multi-chart layouts.
* Millions of vertices in scenes (charts, ladders) without junk reallocations.
* Minimal CPU workload per frame when nothing changes.
* Localized cost when *something small* changes.

The renderer must **not** rebuild or resend the world every time if only a label changed.

---

# 2. Core Principles

1. **Stable Keys Everywhere**
   Every renderable node has a stable key across frames.

2. **Diff, Don’t Rebuild**
   We compute differences between previous and current frame’s scene graph.

3. **Dirty, Not Global**
   We track dirty **nodes**, **batches**, and **regions**, not “whole frame invalid”.

4. **Buffer Reuse by Shape**
   GPU buffers are reused by geometry “shape” (vertex layout, size), not tied to one frame.

5. **Layer-Scoped**
   All caching, diffing, and partial repaints are **per-layer** (from HAL-006).

6. **Trading Bias**
   We optimize primarily for **highly dynamic data within stable structures**
   (same chart layout, prices change every frame).

---

# 3. Key Concepts

### 3.1 SceneNodeKey

Stable identity for a render node:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneNodeKey(u64);
```

Rules:

* Widgets generate keys deterministically (e.g. widget id + local index).
* Within a layer, `(SceneNodeKey, LayerId)` pair is unique.

### 3.2 SceneNodeFingerprint

Compact fingerprint of visual properties, used for change detection:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeFingerprint(u64);
```

Example fields included:

* primitive type (rect/text/line/…)
* color (quantized)
* size / rect (quantized if needed)
* font + size for text
* z-index within layer

No floats passed directly; floats go through quantization or hashing strategy.

### 3.3 CachedNode

Cached representation per node:

```rust
pub struct CachedNode {
    pub key: SceneNodeKey,
    pub fingerprint: NodeFingerprint,
    pub layer_id: LayerId,
    pub batch_id: BatchId,
    pub vertex_range: std::ops::Range<u32>, // inside a GPU buffer
    pub index_range: std::ops::Range<u32>,  // if indexed
    pub bounds: RectF,                      // for dirty-region tracking
}
```

### 3.4 CachedLayer

Per-layer cache:

```rust
pub struct CachedLayer {
    pub nodes: fxhash::FxHashMap<SceneNodeKey, CachedNode>,
    pub batches: Vec<GpuBatch>,
    pub dirty_regions: DirtyRegions,
}
```

### 3.5 GpuBatch

Represents a draw batch suitable for one draw call:

```rust
pub struct GpuBatch {
    pub batch_id: BatchId,
    pub primitive_type: PrimitiveType,
    pub material_id: MaterialId,
    pub vertex_buffer_id: BufferId,
    pub index_buffer_id: Option<BufferId>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub dirty: bool,
    pub bounds: RectF,
}
```

PrimitiveType/MaterialId follow HAL-006.

### 3.6 DirtyRegions

Tracks rectangles that must be repainted:

```rust
pub struct DirtyRegions {
    pub rects: smallvec::SmallVec<[RectF; 8]>,
}

impl DirtyRegions {
    pub fn mark(&mut self, rect: RectF) { /* union/merge simple strategy */ }
    pub fn clear(&mut self) { self.rects.clear(); }
}
```

---

# 4. Frame-Level Cache Manager

Top-level cache object:

```rust
pub struct FrameCache {
    pub layers: fxhash::FxHashMap<LayerId, CachedLayer>,
    pub buffer_pool: BufferPool,
}

pub struct BufferPool {
    // keyed by (primitive_type, vertex_layout_id, capacity_bucket, usage_hint)
    // internal: Vec<wgpu::Buffer> and free lists
}
```

There is exactly one `FrameCache` per renderer instance.

---

# 5. Diff & Cache Update Algorithm

Given:

* `Scene` for current frame (HAL-006: layers + nodes)
* `FrameCache` from previous frame

We process **per layer**:

### 5.1 Build Node Map per Layer

For each layer in current scene:

* Build list: `Vec<RenderNode>` with associated `SceneNodeKey` + fingerprint.
* Existing keys reused; new nodes get new keys from widget layer.

### 5.2 Node Diff (per Layer)

Given `old_nodes: HashMap<SceneNodeKey, CachedNode>` and `new_nodes: Vec<RenderNode>`:

1. Initialize new `HashMap<SceneNodeKey, CachedNode>` for `next_nodes`.

2. For each `RenderNode` in new list:

   ```rust
   match old_nodes.get(&render_node.key) {
       None => {
           // New node - mark its region dirty, schedule buffer write
       }
       Some(cached) => {
           if cached.fingerprint == render_node.fingerprint {
               // Node unchanged at GPU level; reuse batch + ranges
               copy to next_nodes
           } else {
               // Node changed - mark region dirty, schedule buffer update
           }
       }
   }
   ```

3. Any `old_nodes` entries not matched → node disappeared →
   mark their previous bounds as dirty and let them be removed.

Outcome per layer:

* `next_nodes` map
* list of node changes:

    * `NewNode(RenderNode)`
    * `ChangedNode(RenderNode, CachedNode)`
    * `RemovedNode(CachedNode)`

### 5.3 Batch Rebuild Strategy

We need simple, predictable rules:

**Rule A — Batch Key**

All nodes in a batch share:

* primitive_type
* material_id
* layer_id

**Rule B — When to Reuse Batch**

* If batch membership (set of node keys) unchanged and order unchanged, **only update changed nodes’ vertex ranges**.
* If membership changed (nodes added/removed), we may:

    * either rebuild the whole batch, or
    * maintain an “append-only” strategy until buffers get too fragmented.

First implementation: **simple and reliable:**

* For each layer:

    * If *any* node set changed for a given batch key:

        * rebuild that batch’s vertex buffer from scratch.
    * Others: reuse completely.

Optimization later: partial patching if needed.

### 5.4 Dirty Regions

Per layer, mark dirty rectangles:

* For new and changed nodes: `DirtyRegions::mark(node.bounds)`.
* For removed nodes: `DirtyRegions::mark(cached.bounds)`.

These are used in two ways:

1. CPU-side: decide which layers/batches require command list rebuild.
2. Future optimization: use scissor rects per dirty area, skipping fully clean regions.

---

# 6. GPU Buffer Pooling

Avoid `create_buffer` every frame.

### 6.1 Pool Keys

Keyed by:

```rust
pub struct BufferKey {
    pub primitive_type: PrimitiveType,
    pub vertex_layout_id: u16,
    pub capacity_bucket: u32, // e.g. 1k, 2k, 4k, 8k ...
}
```

Buckets chosen as next power-of-two >= required size.

### 6.2 Pool Strategy

* On allocation request `(primitive_type, vertex_layout, needed_vertices)`:

    * bucket = round_up_pow2(needed_vertices)
    * look in free list for `BufferKey { …, capacity_bucket: bucket }`
    * if found: reuse
    * else: create new wgpu buffer, register in pool

* On frame end:

    * buffers stay allocated; **never destroyed** unless pool has hard upper bound logic later.

### 6.3 Command Lists

We don’t keep Vulkan-style command buffers; we re-record each frame.
But:

* We keep a per-layer set of draw commands (lightweight structs).
* For layers with **no dirty batches**, we can skip re-building the CPU-side command list and reuse previous recorded sequence if we ever move to persistent command buffers for a given backend. For wgpu, we re-encode each frame but use the same batch layout; the CPU cost is negligible compared to buffer uploads.

---

# 7. Partial Repaint Model

We define “partial repaint” at **layer granularity** first.

### 7.1 Layer Repaint Decision

For each layer:

* If `dirty_regions` is empty **and** no global “force redraw”:

    * We do minimal work (may still have to submit a pass due to swapchain constraints, but no data upload).
* If not empty:

    * Rebuild vertex buffers for dirty batches.
    * Re-encode that layer’s draw pass.

### 7.2 Region-Based Optimization (Optional Later)

For GPUs that benefit from scissoring:

* For each `DirtyRegions.rects`:

    * Set scissor rect and draw only relevant batches.
* For first implementation: **don’t complicate** — just redraw full layer if anything dirty.

---

# 8. Charting / Trading Specific Cases

### 8.1 Chart Scrolling / Panning

Scrolling a chart means:

* Most nodes move by a constant offset in x.
* Candle count stays similar.

We have two strategies:

1. Treat “move of many nodes” as “changed bounds” for all; boxes dirty → full layer redraw.
2. **Chart widget internally uses GPU transforms** (preferred):

    * Chart emits relatively stable geometry in local space.
    * Scrolling is a uniform `view_matrix` change → one uniform buffer update, no vertex rewrites.

SPEC stance:

* Halcyon cache layer is agnostic.
* Chart widget should implement “move via transform” where possible; diffing layer benefits automatically.

### 8.2 Blotter / Order Table

Rows update frequently, but structure (columns, row count range) is stable.

* Each row → one or several `SceneNodeKey`s.
* Fingerprints change when price/size/text changes.
* Only those rows’ nodes are re-uploaded.

### 8.3 DOM / Ladder

Price levels updated constantly.

* LadderWidget owns keys per price level.
* When price changes, only that level’s node(s) marked changed.

This is where caching pays off directly: thousands of stable lines, small subset changing every tick.

---

# 9. API Additions / Adjustments

### 9.1 SceneNodeKey Allocation

Widgets that care about caching must be able to assign stable keys:

```rust
pub trait Widget {
    // existing APIs...

    /// Optional: supply stable keys for its rendered nodes.
    /// If not provided, Halcyon can auto-generate with weaker reuse guarantees.
    fn visit_scene_keys(&self, visitor: &mut dyn FnMut(SceneNodeKeyCandidate));
}

pub struct SceneNodeKeyCandidate {
    pub local_id: u32,        // widget-local index
    pub purpose: NodePurpose, // Body/Wick/Text/…
}
```

Renderer turns `(WidgetId, local_id, purpose)` into `SceneNodeKey`.

### 9.2 RendererWgpu + FrameCache Integration

`RendererWgpu` now holds a `FrameCache`:

```rust
pub struct RendererWgpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    // ...
    frame_cache: FrameCache,
}
```

On each frame:

```rust
fn render_scene(&mut self, scene: &Scene) {
    self.frame_cache.update_from_scene(scene, &self.device, &self.queue);
    self.encode_and_submit(&self.frame_cache);
}
```

---

# 10. Implementation Checklist for Claude Code

Direct tasks:

1. **Introduce Cache Types**

    * `SceneNodeKey`, `NodeFingerprint`
    * `CachedNode`, `CachedLayer`, `FrameCache`, `BufferPool`, `GpuBatch`, `DirtyRegions`.

2. **Hashing/Fingerprint Strategy**

    * Implement a `NodeFingerprint::from_primitive(&Primitive)` that:

        * includes type, rect, color, font, etc.
        * avoids direct raw f32 equality; use quantization or bit representation.

3. **Integrate Keys into Scene**

    * Extend `Primitive`/Scene nodes to carry `SceneNodeKey`.
    * Provide a path for widgets to influence key generation via `(WidgetId, local index)`.

4. **Layer Diff Function**

    * `fn diff_layer(old: &CachedLayer, new_nodes: &[RenderNode]) -> LayerDiffResult`
    * Output: new node map, action list (new/changed/removed).

5. **Batch Rebuild Logic**

    * Group nodes by `(primitive_type, material_id)`.
    * Build `GpuBatch` list for each layer.
    * For changed batches: allocate/reuse buffers from `BufferPool` and upload vertex data.
    * For unchanged batches: reuse buffer IDs and ranges.

6. **Dirty Region Tracking**

    * For new/changed/removed nodes: mark `bounds` into `dirty_regions`.

7. **Renderer Integration**

    * `RendererWgpu::render(scene: &Scene)`:

        * call cache update
        * for each layer:

            * if layer dirty: encode draws
            * else: minimal or reused path

8. **Benchmarks/Tests**

    * Scenario A: 10,000 static nodes → no changes → CPU time per frame ~constant and tiny.
    * Scenario B: 10,000 nodes, 100 change per frame → check that only those cause uploads.
    * Scenario C: Ladder with 1,000 levels, 50 updates per frame.
    * Scenario D: Chart with stable geometry but view transform change only (later when chart uses transform).

---

# 11. Result

With SPEC-HAL-014 implemented:

* Halcyon stops behaving like a naive “rebuild and resend everything” toy.
* Trading dashboards with multiple views can run at **120–240 FPS** without choking.
* Heavy chart and DOM visuals become feasible without GPU buffer trashing.
* Future optimizations (true partial repaints, scissor-based redraw, persistent command buffers) sit on top of this design.
