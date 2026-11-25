# **SPEC-HAL-027: Scene Graph Batching & Draw-Call Minimization**

**Author:** JC
**Date:** 2025-11-24
**Status:** Partially Implemented
**Amended by:** SPEC-HAL-359 (binding cross-pipeline Z execution)
**Scope:** Rendering / Scene Graph
**Purpose:** Reduce GPU draw calls, batch compatible primitives, and create a stable render pipeline for Halcyon's high-frequency trading UI.

---

## Implementation Status

**Partially Implemented**

Existing modules/files:
- `src/renderer/batcher.rs` — Batcher, DrawBatchKey, FrameBatches, clip stack
- `src/renderer/wgpu_renderer.rs` — pipeline-specific lowering and immutable
  per-surface cross-pipeline execution schedule
- `src/renderer/render_graph/passes/` — schedule-driven main/overlay execution

Not yet implemented:
- Advanced instancing
- Layer-based compositing

---

## 1. Purpose

Halcyon’s renderer must sustain:

* 120–240 FPS
* Thousands of rectangles, text runs, borders
* Zero jank during rapid UI updates (charts, orderbooks, tables)

Unbatched, every widget emits standalone nodes → the renderer emits dozens or hundreds of draw calls.
This SPEC introduces deterministic **batching rules** and a unified **batched scene graph** that collapses redundant draw calls before hitting WGPU.

---

## 2. Problem Statement

Current renderer:

* Emits one WGPU draw call per node (`push_rect`, `push_text`, etc.)
* Causes *massive overdraw* and GPU pipeline switching
* Forces CPU to recreate bind groups unnecessarily
* Prevents advanced optimizations (instancing, atlas rendering, pipelines)

Halcyon needs a composable batching system.

---

## 3. Design Goals

### 3.1. Hard Requirements

* Deterministic batching
* Stable ordering
* Zero reordering bugs
* Zero layout influence (pure render layer)
* CPU cost must be **O(n)** per frame
* No GPU state thrashing

### 3.2. Nice-to-have

* Prepared for instancing
* Prepared for clip batching
* Prepared for glyph atlas (SPEC-103)

---

## 4. Scene Graph: New Model

Replace the current raw node list with a **BatchedScene**.

### 4.1. Current Model (simplified)

```
Vec<SceneNode>
```

### 4.2. New Model

```
struct BatchedScene {
    rect_batches: Vec<RectBatch>,
    text_batches: Vec<TextBatch>,
    line_batches: Vec<LineBatch>,
    clip_stack: Vec<ClipNode>,
    // future: border_batches, image_batches
}
```

Each batch contains:

* A single pipeline state
* A homogeneous list of primitives
* Shared bind groups where possible

---

## 5. Batching Rules

These rules are **strict** and deterministic.

### Rule 1 — Batch only identical pipeline states

Two nodes can be batched **iff**:

* Same pipeline type (rect, text, line, etc.)
* Same blend mode
* Same clip state
* Same texture / atlas region (for text or images)
* Same complete effective order and closed member interval under SPEC-HAL-359 §2.5

### Rule 2 — Clip state breaks batches

Clip nodes push/pop a stack.
Any change to clip stack → new batch starts.

### Rule 3 — Z-order preserved

Batches, paths, images, and text runs must not reorder nodes across Z-layers.
SPEC-HAL-359 requires Compile to retain one per-surface execution schedule after
pipeline-specific lowering. Execute follows that schedule across pipeline
families; batching may reduce state changes only where it preserves the order.

Z is resolved like:

```
for layer in sorted_layers:
    execute owner-correct pipeline commands within layer
```

Equal-Z order across different batch keys, clips, or pipelines remains
non-semantic under SPEC-PM-007.

SPEC-HAL-359 §2.5's explicitly admitted ordinary-control-subtree scope qualifies
only the scalar ordering above: member order precedes unchanged local Z inside
that closed main interval. Nested prefixes and scope identity survive flattening,
batch partitioning and the common batch/path/text Compile comparator. Equal
external placement remains non-semantic; no cross-member batch merge, local-Z
packing, overlay reclassification or Execute sorting is permitted. Ordinary
unannotated paths and this SPEC's Partially Implemented status are unchanged.

### Rule 4 — Text always goes through glyph atlas

Text nodes are grouped by:

* Font id
* Font size
* Color (optional, can be a per-instance attribute)

### Rule 5 — Rectangles grouped by fill + stroke

Rect batching key:

```
(fill_color, border_color, radius, texture_id)
```

### Rule 6 — Nodes cannot escape batch boundaries

Once a batch ends, it cannot be reopened.

---

## 6. SceneNode Format (updated)

To support batching, SceneNode must include metadata:

```
enum SceneNode {
    Rect(RectNode),
    Text(TextNode),
    Line(LineNode),
    Clip(ClipNode),
}
```

Each variant gets:

```
z_index: u8
batch_key: BatchKey
```

Where BatchKey is:

```
struct BatchKey {
    kind: NodeKind,         // rect, text, line
    pipeline: PipelineId,
    clip_hash: u64,         // current clip stack hash
    texture_id: Option<u32> // glyph atlas / images
}
```

---

## 7. Pipeline IDs

Renderer defines:

```
PIPELINE_RECT      = 1
PIPELINE_TEXT      = 2
PIPELINE_LINE      = 3
PIPELINE_IMAGE     = 4
```

Future SPECs may add more.

---

## 8. Batching Algorithm (Core)

Pseudocode:

```
let mut scene = BatchedScene::new();
let mut current_batch = None;

for node in nodes_sorted_by_z:
    match node.kind:
        Clip => {
            scene.clip_stack.apply(node);
            current_batch = None; // must flush
        }

        _ => {
            if current_batch.is_none() || current_batch.key != node.batch_key:
                current_batch = scene.start_new_batch(node.batch_key);
            current_batch.push(node);
        }
}
```

Final result:
**Minimal number of batches** that respect Z-order and clip-scope.

---

## 9. GPU Side

Renderer receives batches:

```
for batch in scene.rect_batches:
    bind pipeline_rect
    bind atlas / uniform buffers
    issue one draw call for entire batch
```

Result:

* 1000 rects → **1 draw call**
* 1000 glyphs → **1 draw call**
* No redundant bind group switches

This is trading-grade rendering.

---

## 10. Integration Steps

### Step 1 — Introduce BatchKey

Add BatchKey computation in:

* `push_rect`
* `push_text`
* `push_line`

### Step 2 — Add BatchedScene structure

`scene.rs` gains the new struct + logic.

### Step 3 — Replace old builder

SceneBuilder no longer stores `Vec<SceneNode>` — instead, nodes are sorted and batched automatically.

### Step 4 — Update wgpu_renderer

Renderer loops over batches, not individual nodes.

### Step 5 — Add clip stack hashing

Used to guarantee batch segmentation.

### Step 6 — Add tests

* “1000 rects produce 1 draw call”
* “Different clip regions produce multiple batches”
* “Z-order maintained under batching”

---

## 11. Risks / Precautions

* Must ensure batching does not reorder nodes across Z boundaries
* Clip boundaries must never merge
* Glyph atlas (SPEC-103) must use identical texture_id logic
* Must update TextEngine to produce atlas ids later

Everything here is deterministic and safe for Halcyon’s retained-mode architecture.

---

## 12. Future SPEC Interactions

### SPEC-103 — Glyph Atlas Cache

This batching SPEC is **mandatory** for that to work.

### SPEC-102 — Modular Shader Pipeline

Batching must match pipeline segmentation.

### SPEC-104 — Present Mode / Frame Pacing

Batching reduces frame time, enabling stable pacing.

### SPEC-105 — Reactive Signals

Pressure on layout invalidation aligns well with this.

---

## 13. Conclusion

This SPEC establishes the new **batched scene graph** for Halcyon, minimizing draw calls and providing deterministic, trading-grade GPU performance.

It replaces the naive per-node drawing model and prepares the renderer for atlas text, clip batching, and modular pipelines.
