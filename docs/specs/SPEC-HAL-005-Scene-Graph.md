# **SPEC-HAL-005: Halcyon — Scene Graph 2.0 (Batching, Layers, Z-Order, Clipping, Render-Ready Nodes)**

**Author:** JC
**Date:** 2025-11-16
**Status:** Draft
**Scope:** The final, production-grade scene graph model that Halcyon widgets output before GPU rendering.
**Primary Use-Case:** High-frequency trading UIs requiring predictable Z-order, clipping, layers, batch-friendly primitives, and fast diffing.
**This SPEC defines the core data model that the future renderer (SPEC-HAL-006+) consumes.**

---

# **1. Purpose**

The scene graph in SPEC-HAL-003 was a minimal skeleton.
This SPEC upgrades it into a **true GPU-ready rendering tree**, capable of:

* Massive batching (100k+ primitives)
* Arbitrary Z-order without CPU sorting
* Per-node clipping (scroll areas, windows)
* Layering (modal windows, popups)
* GPU-friendly flattening
* Hit-testing with correct transforms
* Child-to-parent transforms
* Predictable performance

This is the same domain as:

* Flutter’s DisplayList
* Skia’s SkPicture
* WebGPU render trees
* Vello’s Scene model

But tuned for your trading UI needs.

---

# **2. Guiding Principles**

1. **Widget tree ≠ scene graph.**

    * Widgets are logical UI objects.
    * Scene graph is a flattened draw tree.

2. **Scene graph nodes must be extremely cheap.**
   No dynamic trait objects.
   No boxed nodes.
   Pure POD-ish structs, tightly packed.

3. **GPU friendliness first:**

    * Batching decisions baked into graph.
    * Minimal per-frame allocations.
    * Clip stacks must be deterministically flattenable.

4. **Hit-testing uses the *same* tree (in logical coords).**

5. **Scene graph is recreated every frame.**
   But built from **static widget tree**, so cost is low.

6. **Recreation does not require allocator churn.**
   After draw-plan construction has consumed a production scene, Halcyon may clear and reuse the
   immediately preceding root and nested clip/layer/scroll node-vector storage. Reuse carries no
   prior node payload or visual state, and unused prior vectors are discarded on the next build.

---

# **3. Final Scene Graph Structure**

The scene is a list of **RenderNode**.
Each node is one of the following:

```
Scene
  ├── Layer
  │     ├── Clip
  │     │     ├── Rect
  │     │     ├── Text
  │     │     ├── Image
  │     │     └── ...
  │     └── ...
  ├── Popup Layer
  └── Modal Layer
```

Key: **Layers never clip each other**.
Clips only affect their subtree.

---

# **4. RenderNode Definition**

```rust
pub enum RenderNode {
    Rect(RectNode),
    RoundedRect(RoundedRectNode),
    Text(TextNode),
    Image(ImageNode),

    Clip(ClipNode),
    Layer(LayerNode),
}
```

Nodes DO NOT contain children.
Children are stored in `SceneBuilder` via a stack-based model (same as Skia/Flutter).

Nodes contain only **draw-time data**, not widget pointers.

---

# **5. Node Types**

### 5.1 RectNode

```rust
pub struct RectNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Color,
}
```

### 5.2 RoundedRectNode

```rust
pub struct RoundedRectNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    pub color: Color,
}
```

### 5.3 TextNode

```rust
pub struct TextNode {
    pub x: f32,
    pub y: f32,
    pub content: String,
    pub size: f32,
    pub color: Color,
}
```

### 5.4 ImageNode

```rust
pub struct ImageNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub texture_id: TextureId,
}
```

---

# **6. Clip Nodes**

Essential for:

* Text fields
* Scroll areas
* Windows
* Chart panels
* Heatmaps

Clip nodes are bounding boxes:

```rust
pub struct ClipNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

Clip push/pop is handled via a **clip stack** in the renderer.

---

# **7. Layer Nodes**

Layers create isolated render groups.

They define:

```rust
pub struct LayerNode {
    pub z_index: u32,
}
```

Layers allow:

* Tooltips
* Context menus
* Modal windows
* Drag ghosts
* Selection overlays

Renderer sorts by `z_index` **once** per frame.
No per-node sorting needed.

---

# **8. Transform Support (Future)**

Trading tools may need:

* Zoomable panels
* Custom chart transforms
* Different coordinate spaces per widget

We add:

```rust
pub struct TransformNode {
    pub matrix: [f32; 6], // 2x3 affine
}
```

But we **do NOT enable it in the base**.
This is SPEC-HAL-009 territory.

---

# **9. SceneBuilder API (Final Form)**

Widgets paint via a builder that looks like this:

```rust
pub struct SceneBuilder<'a> {
    nodes: &'a mut Vec<RenderNode>,
    clip_stack: Vec<ClipNode>,
    layer_stack: Vec<LayerNode>,
}
```

## **Adding primitives:**

```rust
pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color);
pub fn rounded_rect(&mut self, ...);
pub fn text(&mut self, ...);
pub fn image(&mut self, ...);
```

## **Pushing clip:**

```rust
pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32);
pub fn pop_clip(&mut self);
```

## **Pushing layer:**

```rust
pub fn push_layer(&mut self, z: u32);
pub fn pop_layer(&mut self);
```

`push/pop` simply append nodes into the scene:

```
ClipStart
  Rect
  Text
ClipEnd
```

Renderer turns this into appropriate GPU scissor stacks.

---

# **10. RenderNode Flattening Example**

Widget tree:

```
Window
  Column
     Button
     TextField
     ChartWidget
```

Scene:

```
Layer(z=0)
  Rect(window_bg)
  Clip(window)
    RoundedRect(button)
    Text(label)
    Clip(textfield)
      Rect(cursor)
      Text(input)
    Clip(chart)
      Rect(candles)
      Rect(axes)
Layer(z=1000)  // tooltip
  Text(tooltip)
```

Ordered, flattened, GPU-friendly.

---

# **11. Z-ordering Rules**

1. Each `LayerNode` has a z-index.
2. All primitives inside a layer retain order.
3. Layers are rendered sorted by z-index.
4. Inside a layer, order = append order.
5. Clip nodes apply only inside the same layer.

This eliminates all ambiguity.

---

# **12. Batching Rules**

Primitive type → pipeline.

Batch groups:

* Rect batch
* RoundedRect batch
* Text batch
* Image batch

SceneBuilder sorts nothing.
Renderer handles batching in one pass:

1. Iterate nodes
2. When primitive type changes → flush current batch
3. Push vertices to GPU buffers
4. Minimize pipeline switches

This gives near-zero CPU overhead.

---

# **13. Why This Matters for Trading UIs**

Your future UI will include:

* Multiple charts
* Ladders
* DOM
* Execution blotters
* Alerts
* Notification popups
* Order-entry ticket
* Hover tooltips
* Selection overlays
* Live depth heatmaps
* Fast scrolling grids

Those require:

✔ stable z-depth
✔ efficient clipping
✔ fast incremental scene creation
✔ no runtime virtual DOM
✔ no re-sorting of primitives
✔ predictable input routing
✔ accurate hit-testing
✔ ability to render 100k+ primitives per frame

This scene graph matches these guarantees.

---

# **14. Implementation Checklist (Claude Code Ready)**

### In `scene.rs`:

* [ ] Add RenderNode enum
* [ ] Add RectNode, RoundedRectNode, TextNode, ImageNode
* [ ] Add ClipNode + LayerNode
* [ ] Add Scene struct with `Vec<RenderNode>`
* [ ] Update SceneBuilder to support push/pop
* [ ] Add clip_stack & layer_stack

### In renderer:

* [ ] Implement clip stack → wgpu scissor
* [ ] Implement layer sorting
* [ ] Implement batching by primitive type
* [ ] Implement pipeline switching

### In widgets:

* [ ] Paint routines use SceneBuilder push/pop consistently
* [ ] ScrollArea uses push_clip
* [ ] Window uses push_layer
* [ ] Tooltip uses a high z-index layer
