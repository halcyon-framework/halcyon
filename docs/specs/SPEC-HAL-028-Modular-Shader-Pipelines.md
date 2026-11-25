# **SPEC-HAL-028: Modular Shader Pipelines**

**Author:** JC
**Date:** 2025-11-24
**Status:** Design – Not Implemented
**Scope:** Renderer Architecture
**Purpose:** Define a modular, state-driven shader pipeline system to replace the ad-hoc, monolithic WGPU shader setup.
**Prerequisites:** SPEC-HAL-005 (Scene Graph), SPEC-HAL-026 (Dirty Flags), SPEC-HAL-027 (Scene Graph Batching)

---

## Implementation Status

**Not implemented**

The renderer currently uses basic inline WGSL shaders for rect and text pipelines. The modular pipeline registry described in this SPEC does not exist.

---

## 1. Purpose

Halcyon’s renderer currently uses a minimal WGPU program with a few WGSL shaders and no formal concept of “pipeline types.”
This is fine for Tier-1 widgets, but **not sufficient** for:

* text blending / SDF / subpixel rendering
* shadows
* blurred backgrounds
* outlines
* wide strokes
* GPU-accelerated shapes
* gradient fills
* high-frequency chart primitives
* batching that groups by material type

This SPEC defines the **canonical shader pipeline architecture** — every drawable in Halcyon will pass through it.

---

## 2. Goals

### 2.1 Primary Goals

* Clearly defined **pipeline types** (SolidRect, Text, Gradient, Outline, etc.)
* Modular loading of WGSL shader modules
* Unified `PipelineKey` struct for batching
* Pipeline registry inside renderer
* No duplicate pipelines, no implicit state
* First-class support for transparency, blending, and masking

### 2.2 Secondary Goals

* Hot-reload support for shaders (dev mode)
* Reusable WGSL modules (buffers, transforms, colors)
* Make charting/advanced visualizations possible

### 2.3 Non-Goals

* No custom DSL
* No runtime shader generation
* No caching system yet (handled in SPEC-HAL-029)

---

## 3. Core Idea

Introduce a **Modular Shader Pipeline Layer** that sits between:

```
[ Scene Graph (logical) ]
          ↓
[ Batching (SPEC-101) ]
          ↓
[ Modular Shader Pipelines ]   ← THIS SPEC
          ↓
[ WGPU Rendering ]
```

The scene graph is already producing typed primitives (`RectNode`, `TextNode`, etc.).
Batching groups them by `PipelineKey`.
Pipelines select the correct shader and bind groups.

This SPEC defines the structure of that layer.

---

## 4. Pipeline Types

These are the canonical pipeline families Halcyon will support:

| Pipeline ID         | Description                                    |
| ------------------- | ---------------------------------------------- |
| **SOLID_RECT**      | Flat-colored rectangles, rounded corners       |
| **TEXT_SDF**        | Text rendering using the glyph atlas (SPEC-103) |
| **GRADIENT_LINEAR** | Linear gradients                               |
| **GRADIENT_RADIAL** | Radial gradients (later use)                   |
| **OUTLINE_RECT**    | Rectangle outlines / borders                   |
| **IMAGE**           | Static images / textures                       |
| **CLIP_MASK**       | Stencil-based masking (for ScrollArea etc.)    |
| **WIREFRAME_DEBUG** | Debug mode — optional                          |

**Initial implementation requires only:**

* SOLID_RECT
* TEXT_SDF
* CLIP_MASK

The rest come after Atlas (SPEC-103).

---

## 5. PipelineKey (Batching Interface)

### 5.1 Struct Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub kind: PipelineKind,
    pub blend_mode: BlendMode,
    pub sample_count: u8,
}
```

### 5.2 Why this works

Because batching (SPEC-101) groups primitives by `PipelineKey`, switching pipelines is minimized:

```
[Rect, Rect, Rect, Text, Text] → 2 pipeline binds
```

This is how modern engines operate (Unity/UE etc.) and avoids catastrophic state churn.

---

## 6. Shader Module Loading

### 6.1 File Structure

Recommend this directory:

```
src/
  renderer/
    pipelines/
      solid_rect.wgsl
      text_sdf.wgsl
      clip_mask.wgsl
      common.wgsl
```

### 6.2 `common.wgsl`

Contains:

* transform matrices
* color conversions
* vertex format shared between pipelines

### 6.3 Shader Registry

A new struct inside `wgpu_renderer.rs`:

```rust
pub struct PipelineRegistry {
    solid_rect: RenderPipeline,
    text_sdf: RenderPipeline,
    clip_mask: RenderPipeline,
    // …
}
```

Constructed at startup, reused forever.

---

## 7. WGPU Pipeline Construction

Every pipeline has:

* Vertex buffer layout
* Bind group layout
* Fragment shader
* Blend state
* Primitive topology
* Multisampling

The SPEC standardizes this:

### 7.1 Required Defaults

```
primitive: triangle_list
cull_mode: None
depth_test: off
multisample: inherited from swapchain
```

### 7.2 Blend Modes

```
OPAQUE    = no blending
ALPHA     = source-over
PREMULT   = premultiplied alpha
```

---

## 8. Binding Model

All pipelines share:

### 8.1 Transform Bind Group

* Projection matrix
* Viewport transform
* DPI scale

### 8.2 Material Bind Group

Depends on the pipeline:

* `SOLID_RECT` → color, radius
* `TEXT_SDF` → glyph atlas texture
* `CLIP_MASK` → mask region

---

## 9. Integration With Renderer

Add `pipeline_for(node)`:

```rust
fn pipeline_for(node: &SceneNode) -> PipelineKey {
    match node {
        SceneNode::Rect { .. } => PipelineKey::solid_rect(),
        SceneNode::Text { .. } => PipelineKey::text_sdf(),
        SceneNode::Clip { .. } => PipelineKey::clip_mask(),
    }
}
```

### 9.1 Render Loop

```
for batch in batches {
    let pipeline = registry.get(batch.key);
    pass.set_pipeline(pipeline);
    pass.draw(batch.range);
}
```

---

## 10. Hot Reload (Dev Mode)

Optional hot-reload pipeline:

* Watch WGSL files with `notify`
* Rebuild pipelines on the fly
* Disable in release mode

Halcyon becomes much easier to tweak visually.

---

## 11. Performance Expectations

* Fewer pipeline switches → **20–40% perf gain**
* More predictable frame times
* Enables atlas caching (SPEC-103)
* Allows text batching (currently impossible)

---

## 12. Implementation Checklist

### Core

* [ ] Create `renderer/pipelines/` directory
* [ ] Add WGSL modules (solid_rect, text_sdf, clip_mask, common)
* [ ] Implement `PipelineKey`
* [ ] Implement `PipelineRegistry`
* [ ] Implement `pipeline_for(node)`
* [ ] Integrate registry into main `WgpuRenderer`

### Scene Graph Integration

* [ ] Update `paint_tree()` to assign `PipelineKey` to each node
* [ ] Ensure batching respects pipeline boundaries (SPEC-101)

### Future Pipelines (not in this SPEC)

* [ ] Gradients
* [ ] Blur
* [ ] Image textures
* [ ] Chart-specific visualizations

---

## End of SPEC-HAL-028
