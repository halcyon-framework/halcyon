# **SPEC-HAL-029: Glyph Atlas Cache**

**Author:** JC
**Date:** 2025-11-24
**Status:** Implemented
**Scope:** Text Rendering, GPU Optimization
**Purpose:** Introduce a persistent glyph atlas to eliminate redundant glyph rasterization and drastically reduce GPU submissions, CPU cost, and text rendering latency.

---

## Implementation Status

**Implemented**

Existing modules/files:
- `src/renderer/glyph_atlas.rs` — GlyphAtlas, GlyphKey, AtlasEntry, SkylinePacker
- `src/renderer/wgpu_renderer.rs` — Text pipeline, atlas bind groups, glyph caching

---

## 1. Purpose

This SPEC defines the **Glyph Atlas Cache**, a core rendering subsystem responsible for:

* Storing rasterized glyph bitmaps in a GPU atlas texture
* Avoiding redundant glyph rasterization per frame
* Allowing all text rendering to be reduced to **UV-mapped quads**
* Minimizing draw calls, state changes, and CPU work
* Stabilizing text layout metrics (shared glyph metrics)
* Preparing Halcyon for sub-millisecond real-time text rendering

This is required for:

* High-density UI
* Trading dashboards
* Charting overlays
* Massive label grids
* Multi-line text rendering (TextBlock, logs, chat, order feeds)

---

## 2. Background

Halcyon currently performs:

1. Shaping (good)
2. Per-glyph rasterization with glyphon (slow, repeated)
3. Immediate CPU → GPU transfer for each draw (slow)

This leads to:

* High CPU cost per frame
* No reuse of glyph bitmaps
* High GPU memory churn
* Inefficient draw sequences (per glyph, not per batch)

Replacing this with an **atlas** will produce:

* Persistent GPU-resident glyphs
* Drastic reduction in CPU text cost
* Batched draw calls using a single pipeline + single bind group
* Smooth FPS even under hundreds of text nodes

---

## 3. Goals

1. **Zero redundant rasterization**

    * Rasterize a glyph once → store in atlas → reuse forever.

2. **Stable glyph metrics**

    * Cached metrics tied to atlas entries.

3. **Single GPU texture per font size**

    * One atlas = one bind group.

4. **Automatic atlas growth**

    * Power-of-two expansion: 1024 → 2048 → 4096, etc.
    * At maximum size, deterministic LRU eviction reuses freed glyph rectangles.

5. **Batch-friendly primitives**

    * SceneGraph emits `TextQuadNode { uv_rect, pos_rect, color }`.

6. **Support variable sizes**

    * Each font_size has its own atlas.

---

## 4. Non-Goals

* No multi-font-family fallback yet (future SPEC).
* No subpixel LCD rendering (sRGB only).
* No signed-distance-field fonts (future upgrade).
* No dynamic atlas shrinking.

---

## 5. Architecture

### 5.1 Components

#### **GlyphAtlas**

Responsible for:

* Allocating atlas texture
* Packing glyph bitmaps
* Returning `AtlasEntry { uv, metrics }`

Fields:

```
struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    packer: GuillotinePacker,
    entries: HashMap<GlyphKey, AtlasEntry>,
    width: u32,
    height: u32,
}
```

#### **GlyphKey**

A unique key per glyph:

```
(text, font_size, font_face, variation_selector, dpi)
```

#### **AtlasEntry**

```
struct AtlasEntry {
    uv_rect: RectF,
    advance: f32,
    bearing_x: f32,
    bearing_y: f32,
    size: Size,
}
```

---

## 6. Data Flow

### Before (current text pipeline)

```
shape → rasterize → upload → draw each glyph
```

### After (atlas pipeline)

```
shape → lookup_in_cache()
    | hit → get uv + metrics
    | miss → rasterize once → pack → upload region
batch text → one pipeline → many quads
```

SceneGraph nodes:

```
TextQuadNode {
    pos: RectF,
    uv: RectF,
    color: Color,
}
```

Renderer unifies all text quads into **one draw call per atlas**.

---

## 7. Atlas Growth Strategy

### Starting size:

```
1024 × 1024
```

### Growth rule (when packer fails):

```
width *= 2
height *= 2
repack all entries (slow path, rare)
```

Maximum:

```
8192 × 8192
```

(to avoid absurd VRAM spikes)

---

## 8. Lifetime and Eviction

The atlas:

* Lives for lifetime of `WgpuRenderer`
* Grows until the maximum texture size
* At maximum size, evicts least-recently-used glyph entries deterministically
* Reuses freed atlas rectangles without moving retained glyphs

Eviction is only needed if:

* Unicode text is extremely large
* Or user switches font family

Eviction MUST NOT change existing UVs for retained glyphs. The atlas may recycle
only rectangles whose cache entries were removed. If no evicted rectangle can fit
the incoming glyph, eviction continues until a reusable slot exists or the atlas
is empty.

---

## 9. Integration with Existing Systems

### 9.1 TextEngine

Add:

* `get_glyph_from_atlas`
* `upload_glyph_if_needed`
* `atlas_for_font_size(font_size)`

### 9.2 SceneGraph

Introduce new node:

```
TextQuadNode
```

### 9.3 Batching System (SPEC-HAL-027)

TextQuads now batch under:

```
PipelineId::Text(font_size)
```

Sorting key:

```
(font_size, texture_id)
```

All text per font size → single bucket → single draw → max GPU throughput.

---

## 10. Renderer Integration

### New BindGroup Layout

```
binding 0: atlas texture
binding 1: sampler
binding 2: uniform (text params)
```

### Vertex Format

```
struct TextVertex {
    pos: vec2
    uv: vec2
    color: u32
}
```

### Pipeline

```
shader: text_quad.wgsl
blend: alpha
depth: disabled
cull: none
```

One pipeline per font size.

---

## 11. Implementation Checklist

### Phase 1 — Core Atlas

* [ ] Implement GuillotinePacker
* [ ] Create `GlyphAtlas` struct
* [ ] Implement rasterize → pack → upload
* [ ] Add atlas cache maps

### Phase 2 — TextEngine Integration

* [ ] Replace glyphon direct pixel upload
* [ ] Use atlas lookup
* [ ] Return `AtlasEntry` in GlyphRun

### Phase 3 — SceneGraph Changes

* [ ] New `TextQuadNode`
* [ ] Remove “draw text” from renderer
* [ ] TextBlock emits quads, not strings

### Phase 4 — Renderer

* [ ] Create text pipeline
* [ ] Implement quad batching
* [ ] Bind atlas automatically per draw group

---

## 12. Performance Expectations

### Current:

* Hundreds of draw calls
* Rasterization every frame
* CPU-bound

### With Atlas:

* Single draw per font size
* No per-frame rasterization
* GPU-bound, very fast
* Stable 240 FPS even with thousands of glyphs

---

## End

SPEC-HAL-029 defines the fundamental subsystem enabling Halcyon to render text **at scale**, without per-frame CPU overhead, using a stable GPU atlas with batched quads.

This SPEC must be implemented **after SPEC-026/027/028** and before Present Mode and Signals.
