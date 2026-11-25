# SPEC-HAL-034: GPU Text Path Merging

**Author:** JC
**Date:** 2025-11-24
**Status:** Design – Not Implemented
**Depends on:**

* SPEC-HAL-005 – Scene Graph
* SPEC-HAL-006 – Text & Font System
* SPEC-HAL-027 – SceneGraph Batching
* SPEC-HAL-029 – Glyph Atlas Cache

---

## Implementation Status

**Not implemented**

Text nodes are collected individually in `collect_text_quads`. The advanced merging and path optimization described in this SPEC is not implemented.

---

## 1. Purpose

Define how Halcyon **merges multiple text draws** into **minimal GPU paths**:

* Reduce the number of text draw calls and state changes.
* Exploit the glyph atlas (SPEC-HAL-029) to batch text with shared resources.
* Keep **widget semantics intact** (no change in layout, hit-testing, or visual output).

This SPEC is **renderer-facing**. Widgets keep emitting normal `TextNode`s; the batching/merging happens in the scene → GPU path.

---

## 2. Problem

Right now:

* Each `TextNode` → own draw call (or near enough).
* A single UI view (orderbook, labels, axes, small numbers everywhere) creates **hundreds/thousands** of text draws per frame.
* Even with glyph atlas caching, the GPU sees:

    * Frequent pipeline switches (different colors, sizes, transforms).
    * Poor locality of text data.
    * Too much CPU overhead building per-node GPU jobs.

We need a **path merging strategy** that:

* Uses the atlas efficiently.
* Groups text nodes by resource compatibility.
* Emits **batches**, not individual nodes.

---

## 3. Scope

### In scope

* New **batched text node type** in scene graph (`TextBatchNode`).
* Rules for merging `TextNode` → `TextBatchNode`.
* Integration with:

    * Glyph atlas (SPEC-HAL-029).
    * SceneGraph batching (SPEC-HAL-027).
* Clear constraints when **NOT** to merge.

### Out of scope

* Changing text layout itself (that's SPEC-HAL-025).
* Per-glyph subpixel tuning, hinting, etc.
* Localization/i18n (SPEC-HAL-023).
* Signed distance field (SDF) fonts (future SPEC if needed).

---

## 4. Design Overview

### 4.1 Core idea

Introduce a **two-tier text representation**:

1. **Logical nodes (input):**

    * Widgets emit simple `TextNode`s:

      ```rust
      enum SceneNode {
          // existing
          RectNode(RectNode),
          ClipNode(ClipNode),
          // ...
          TextNode(TextNode),
          // new
          TextBatchNode(TextBatchNode),
      }
      ```

2. **Batching pass (scene-level):**

    * A pass walks the scene and:

        * Groups compatible `TextNode`s into **batches**.
        * Produces one or more `TextBatchNode`s.
    * Renderer consumes `TextBatchNode` directly.

Widgets don’t know about batches. They just push `TextNode`s.

---

## 5. TextNode & Batch Model

### 5.1 TextNode (input)

Keep it simple and widget-friendly:

```rust
pub struct TextNode {
    pub position: Point,      // top-left in absolute logical coords
    pub text: String,         // the actual string
    pub font_size: f32,
    pub color: Color,
    pub z: i32,               // z-layer (SPEC-HAL-005)
}
```

* `position` is absolute logical (after layout).
* `z` is used for blending with other primitives and for batching constraints.

### 5.2 TextBatchNode (batched)

Represents **many text items** that share GPU state:

```rust
pub struct TextBatchGlyph {
    pub position: Point,      // top-left for this text item
    pub text: String,         // may be stored as index into a string pool later
}

pub struct TextBatchKey {
    pub font_size: f32,
    pub color: Color,
    pub z: i32,
    // future: font_id, subpixel_mode, etc.
}

pub struct TextBatchNode {
    pub key: TextBatchKey,
    pub items: Vec<TextBatchGlyph>,
}
```

* One `TextBatchNode` = group of `TextBatchGlyph`s with identical `TextBatchKey`.
* `TextBatchNode` is what the renderer turns into **one or few GPU draw calls**.

---

## 6. Merging Rules

### 6.1 When nodes are merge-compatible

Two `TextNode`s `A` and `B` are mergeable into the same batch if:

1. **Same z-layer:**

   ```rust
   A.z == B.z
   ```

2. **Same font characteristics** for now:

    * Same `font_size`.
    * Same font family (we only have default font for now; when multiple appear, include `font_id` in key).

3. **Same color:**

   ```rust
   A.color == B.color
   ```

4. **Same clip stack fingerprint:**

    * Whatever representation SPEC-HAL-028 uses to express current clip stack, we treat it as part of the batch key:

      ```rust
      pub struct TextBatchKey {
          pub font_size: f32,
          pub color: Color,
          pub z: i32,
          pub clip_id: ClipStackId, // derived from SPEC-HAL-028
      }
      ```

5. **Same transform / coordinate space:**

    * As long as `position` is already in absolute logical coordinates with no additional per-node transform, we’re safe.
    * If later we add per-node transforms, they must be identical to batch.

### 6.2 When nodes must NOT be merged

A `TextNode` must **not** be merged if:

* It relies on **individual alpha** or blend mode (future).
* It is under a different **clip stack**.
* It is under a different **z-layer**.
* It uses a **different font** (eventually).
* It’s explicitly marked “no_batch” (debug/diagnostics; optional flag).

We can support:

```rust
pub struct TextNode {
    // ...
    pub disable_batch: bool,
}
```

If `disable_batch == true`, skip merging.

---

## 7. Batching Algorithm

### 7.1 Scene-level pass

Add a pass in the `Scene` building pipeline:

1. Widgets populate scene with nodes (including `TextNode`).
2. Before sending to renderer, we run:

```rust
fn batch_text_nodes(nodes: Vec<SceneNode>) -> Vec<SceneNode>;
```

### 7.2 Algorithm sketch

* Walk the node list **in order**.
* Maintain a map:

  ```rust
  HashMap<TextBatchKey, usize> // key → index in output nodes
  ```
* For each `SceneNode`:

    * If it's not `TextNode`:

        * Just push as-is.
    * If it **is** `TextNode`:

        * Build `TextBatchKey` including `clip_id` and `z`.
        * If there is an existing `TextBatchNode` **just before or near** in the output with the same key:

            * Append a new `TextBatchGlyph` to its `items`.
        * Otherwise, create a new `TextBatchNode` and record it.

Important: we preserve **ordering constraints**:

* We **never** move text across non-text nodes that share the same z-layer and clip. If the renderer expects strict ordering, we either:

    * keep batches local (per “run” of compatible nodes); or
    * enforce that all nodes in a given z-lane are reorderable (if we decide that in SPEC-HAL-027).

For safety, initial implementation: **local batching only**:

* Only merge consecutive `TextNode`s that are compatible and separated only by other compatible `TextNode`s.
* Do not merge across arbitrary other node types.

---

## 8. Renderer Integration

### 8.1 SceneNode → GPU path

Renderer currently handles something like:

```rust
match node {
    SceneNode::RectNode(rect) => draw_rect(...),
    SceneNode::TextNode(text) => draw_text(...),
    // ...
}
```

After this SPEC:

```rust
match node {
    SceneNode::RectNode(rect) => draw_rect(...),
    SceneNode::TextNode(text) => draw_text_single(...),     // still supported
    SceneNode::TextBatchNode(batch) => draw_text_batch(...),// preferred
    // ...
}
```

### 8.2 draw_text_batch()

`draw_text_batch()` will:

* Use the glyph atlas (SPEC-HAL-029) to ensure all chars are resident.
* Build **one vertex buffer** (or streaming buffer) concatenating all glyphs for this batch.
* Issue **one draw call** per batch (per `TextBatchKey`, per clip, per z).

Pseudo:

```rust
fn draw_text_batch(batch: &TextBatchNode, cx: &mut RendererContext) {
    // 1. Bind text pipeline (shader, samplers, etc.)
    cx.bind_text_pipeline(batch.key);

    // 2. For each item, build glyph quads
    for item in &batch.items {
        atlas.ensure_glyphs(&item.text, batch.key.font_size);
        cx.push_glyphs(item.position, &item.text, &batch.key);
    }

    // 3. Submit combined draw call(s)
    cx.flush_glyphs();
}
```

---

## 9. Error Handling & Diagnostics

* If for some reason glyph atlas insertion fails (full, etc.), fallback:

    * Option A: render that text via a slow path (`TextNode` direct).
    * Option B: skip rendering and emit a `tracing::warn!` with details.

* Provide **debug flags**:

    * `HALCYON_DEBUG_TEXT_BATCH=false` → disables text batching completely (for debugging).
    * `HALCYON_DEBUG_TEXT_BATCH_COLORS=true` → optionally tint batches differently for visual inspection (not required initially, but useful later).

---

## 10. Invariants

After implementation, the following must hold:

1. **Visual Equivalence**

    * With batching enabled vs disabled, screenshots (excluding floating point noise) must be visually identical for all Tier1 examples.

2. **Stable Hit-Testing**

    * Event routing is **independent of scene graph**, so batching must not affect hit-tests at all.

3. **Deterministic Rendering**

    * Same scene graph → same batch structure (ordering & grouping), given same build & OS.

4. **Safe Fallback**

    * If batching is disabled, system still works with `TextNode` only.

---

## 11. Implementation Plan (Phased)

### Phase 1 – Scene & Types

1. Update `scene.rs`:

    * Add `TextBatchNode`, `TextBatchGlyph`, `TextBatchKey`.
    * Extend `SceneNode` with `TextBatchNode` variant.

2. Make sure `SceneBuilder` API for pushing text stays unchanged:

    * Widgets still call something like `builder.push_text(position, text, size, color, z)` → `TextNode`.
    * No widget changes required for now.

### Phase 2 – Batching Pass

3. Add `batch_text_nodes(nodes: Vec<SceneNode>) -> Vec<SceneNode>` in `scene.rs` (or a dedicated module).
4. Implement local batching:

    * Merge **only consecutive compatible** `TextNode`s.
    * Keep everything else unchanged.
5. Integrate batching step into the render pipeline:

    * When building the final scene, apply batching before handing the nodes to `wgpu_renderer`.

### Phase 3 – Renderer Support

6. In `wgpu_renderer.rs`:

    * Add handling for `SceneNode::TextBatchNode`.
    * Implement `draw_text_batch()` using existing glyph atlas logic.
    * Ensure the glyph atlas API can handle multiple strings in a loop.

7. Keep the old `TextNode` path for:

    * Debugging.
    * Temporary fallback.

### Phase 4 – Debug & Testing

8. Add `HALCYON_DEBUG_TEXT_BATCH` env flag (or a simple global config for now).

9. Add a dedicated example:

    * `examples/text_batching_test.rs` with:

        * Many labels, axes, small texts, etc.
        * Toggling batching on/off for perf comparison.

10. Run manual tests:

    * `tier1_showcase`
    * `textblock_showcase`
    * `flow_grid_*` examples

Check:

* Visual equality.
* FPS improvements with many text nodes.

---

## 12. Claude Code Implementation Checklist

When you implement this SPEC:

1. **Scene types**

    * [ ] Update `src/core/scene.rs`:

        * Add `TextBatchGlyph`, `TextBatchKey`, `TextBatchNode`.
        * Extend `SceneNode` enum.
    * [ ] Make sure `SceneBuilder::push_text()` still creates `TextNode`, not `TextBatchNode`.

2. **Batching pass**

    * [ ] Implement `fn batch_text_nodes(nodes: Vec<SceneNode>) -> Vec<SceneNode>`:

        * Local batching only.
        * Merge only compatible `TextNode`s (same font_size, color, z, clip_id).
    * [ ] Call `batch_text_nodes` in the scene build path right before giving it to `WgpuRenderer`.

3. **Renderer**

    * [ ] In `src/platform/wgpu_renderer.rs`:

        * Extend the main draw loop to handle `SceneNode::TextBatchNode`.
        * Implement `draw_text_batch(&TextBatchNode, &mut RendererContext)`:

            * Use glyph atlas to ensure glyphs.
            * Append glyph quads to a shared buffer.
            * Issue a single draw per batch key.

4. **Config / Debug**

    * [ ] Add a simple flag to disable batching entirely (for debugging).
    * [ ] Make sure that when batching is disabled, everything still works via `TextNode`.

5. **Testing**

    * [ ] Add `examples/text_batching_test.rs` with many text widgets.
    * [ ] Run:

        * `cargo run --example tier1_showcase`
        * `cargo run --example textblock_showcase`
        * `cargo run --example text_batching_test`
    * [ ] Verify:

        * Visual match with and without batching.
        * Reduced number of text-related draw calls (log them or dump stats).

If you follow this exactly, we get **GPU-friendly text batching** without touching widgets or layout, and without risking another “Legend saga.”
