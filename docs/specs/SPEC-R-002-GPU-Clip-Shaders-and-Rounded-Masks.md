# **SPEC-R-002: GPU Clip Shader Implementation & Rounded Rect Masking**

**Author:** JC
**Date:** 2025-11-24
**Status:** Retired — Superseded by SPEC-R-020 (GPU Scissoring & Stencilless Clip Regions)
**Scope:** GPU-level implementation of clip masks
**Purpose:** Implement shader-side clipping and introduce GPU-fast rounded-rect masking for all clipped regions.

---

## Implementation Status

**Not implemented**

GPU-side clip shaders and rounded-rect masking are not implemented.

---

## 1. Problem Statement

SPEC-HAL-032 introduced the CPU architecture:

* ClipNode (PushClip / PopClip)
* Renderer clip stack
* Integration with batching

Now we must **actually implement the GPU clip logic**, which requires:

* Uniform transfer of active clip rect to shaders
* Fragment rejection inside shader
* Support for **rounded rectangle masks** (antd / Figma style corner rounding)

Without GPU-side clipping, the clip stack is non-functional.

This SPEC defines the shader model and renderer bindings.

---

## 2. Goals

1. **Implement shader-side clip enforcement**
   Clip bounds must be respected in all pipelines (rects, text, images, lines).

2. **Support rounded-rect clipping**
   Needed for:

    * Buttons
    * Panels
    * ScrollAreas
    * Tab bars
    * Floating UI

3. **Prepare for future pipelines**
   Including blurs, shadows, and multi-pass rendering.

4. **Zero-overhead uniform updates**
   Clip changes should only incur a uniform bind, not a pipeline switch.

5. **Ensure batching correctness**
   Clip hash must be part of the batch key.

---

## 3. Non-Goals

* No stencil buffer.
* No path clipping (Bezier/complex shapes).
* No 9-patch or SVG clipping.
* No multi-layer mask combinations.

Only axis-aligned rounded rectangles.

---

## 4. GPU Model

### 4.1 ClipState Uniform

Create a new WGSL struct:

```wgsl
struct ClipState {
    rect: vec4<f32>;    // x, y, w, h
    radius: f32;        // uniform corner radius
    _pad: vec3<f32>;    // unused, alignment padding
};
```

Uniform binding (per draw or per batch):

```
@group(0) @binding(3)
var<uniform> u_clip: ClipState;
```

### 4.2 CPU → GPU Binding

In `wgpu_renderer.rs`:

* Each batch push updates the uniform buffer.
* Clip rect comes from the active clip stack top.

### 4.3 Fragment Shader Logic

Two checks:

#### 1. Hard clip (rect clip):

```wgsl
let inside_rect =
    position.x >= u_clip.rect.x &&
    position.y >= u_clip.rect.y &&
    position.x <= u_clip.rect.x + u_clip.rect.z &&
    position.y <= u_clip.rect.y + u_clip.rect.w;

if (!inside_rect) {
    discard;
}
```

#### 2. Rounded mask (corner falloff):

If `u_clip.radius > 0.0`, compute normalized distance to corner arcs:

```wgsl
let rx = u_clip.radius;
let ry = u_clip.radius;

let px = clamp(position.x, u_clip.rect.x + rx, u_clip.rect.x + u_clip.rect.z - rx);
let py = clamp(position.y, u_clip.rect.y + ry, u_clip.rect.y + u_clip.rect.w - ry);

let dx = position.x - px;
let dy = position.y - py;

if (dx*dx + dy*dy > rx*rx) {
    discard;
}
```

This produces pixel-accurate rounded clipping.

---

## 5. Pipeline Integration

### 5.1 Affected Pipelines

* Rect fills
* Rect strokes
* Text (glyph quads)
* Lines
* Images
* Custom primitives (future)

Each WGSL shader must include:

```wgsl
@group(0) @binding(3) var<uniform> u_clip: ClipState;
```

### 5.2 Renderer Changes

In `wgpu_renderer.rs`:

* Each batch stores `clip_hash`
* Before drawing a batch:

    1. Write clip rect → uniform buffer
    2. Bind uniform group
    3. Issue draw calls

### 5.3 Scene/Batches Must Break on Clip Changes

Batch key includes:

```
pipeline_id
texture_id
clip_hash
```

Otherwise clip changes will leak into other batches.

---

## 6. Performance Analysis

**Fragment discard is cheap** on modern GPUs when the discard rate is predictable (clip rect edges).
Rounded rect math: ~6 ops per fragment — negligible.

Slow path only happens if:

* Widget clips huge off-screen areas
* Overdraw is excessive

But Halcyon is not overdraw-heavy.

Conclusion: rounded clipping is safe at 240 FPS.

---

## 7. Example (ScrollArea)

Current ScrollArea:

* PushClip(inner_rect)
* Paint children
* PopClip()

Now:

* Renderer enforces clip on GPU
* Scroll overflow reliably masked
* Nested scrolling becomes correct

---

## 8. Precautions

* Clip rect must be in **absolute logical space**, never local.
* Clip radius must be premultiplied by DPI **outside** shader.
* Must ensure **precise float rounding** on rect boundaries to avoid one-pixel bleed.
* All pipelines must be updated; missing one causes artifacts.

---

## 9. Claude Code Instructions (for future implementation)

When implementing, Claude Code must:

1. Add `ClipState` WGSL struct and binding to every shader.
2. Add uniform buffer creation and slot in renderer.
3. Update batch key to include `clip_hash`.
4. Insert GPU uniform writes before each batch draw.
5. Insert fragment discard logic into pipelines.
6. Ensure rounded clipping parameter is passed.
7. Thoroughly test ScrollArea + nested clips.

No shader fanciness. No extra pipelines. Only clipping.
