# SPEC-R-013-v2: Shadow Pipeline (Analytical Single-Pass)

### *Unified Drop Shadow, Inner Shadow, and Focus Ring Rendering*

**Author:** JC
**Status:** Design
**Version:** 2.0
**Supersedes:** SPEC-R-013 (GPU Shadows & Soft Masking Pipeline), SPEC-R-014 (GPU Drop Shadows & Ambient Soft Shadows)

**Depends On:**

* SPEC-R-001 Clip Stack Rework (Complete)
* SPEC-R-020 GPU Scissoring & Stencilless Clip Regions (Complete)

**No other dependencies.** This SPEC is deliberately self-contained. It does not depend on R-005 (SDF renderer), R-007 (blend engine), R-008 (composition graph), or any other unimplemented R-series SPEC.

---

## 1. Purpose

Add shadow rendering to Halcyon's GPU pipeline. Three effect types, one shader, one pipeline:

1. **Drop shadow** — Gaussian falloff outside a shape boundary. Elevates panels, cards, popups.
2. **Inner shadow** — Gaussian falloff inside a shape boundary. Recesses text fields, scroll areas.
3. **Focus ring** — Gaussian band at a fixed distance from shape boundary. Keyboard focus affordance.

All three are analytical single-pass fragment shader effects. No offscreen render targets. No blur textures. No multi-pass convolution. One draw call per batch.

---

## 2. Design Principles

**P-1: Analytical, not sampled.** Shadow alpha is computed from a distance function and `exp()`. No texture lookups, no blur kernels, no ping-pong buffers.

**P-2: Single shader, mode flag.** All three effects share the same SDF distance computation. They differ only in how the distance value maps to alpha. One shader with a `mode` flag, not three separate shaders.

**P-3: Separate pipeline.** `PipelineId::Shadow = 7`. The rect shader is the highest-frequency shader in the system. Shadow uniforms must not bloat it. Shadow batches are z-separated from their casters and batch among themselves.

**P-4: Rounded rect SDF only.** Shadows are defined over rounded rectangles. This covers rects, rounded rects, pills, and circles (radius = half-extent). Arbitrary path shadows are explicitly deferred — rounded rects cover 99% of UI shadow use cases.

**P-5: Premultiplied alpha.** All output is `(r*a, g*a, b*a, a)`. Blends via `One / OneMinusSrcAlpha`, same as all other Halcyon pipelines.

**P-6: Constitution compliant.** Shadow nodes are pure data in the scene graph. Immutable per frame. No mutation during paint. No cross-phase leaking.

---

## 3. SceneNode

### 3.1 New Variant

```rust
pub enum SceneNode {
    // ... existing variants ...

    /// SPEC-R-013-v2: Analytical shadow effect
    Shadow(ShadowNode),
}
```

### 3.2 ShadowNode

```rust
#[derive(Debug, Clone)]
pub struct ShadowNode {
    /// Bounding rect of the casting shape (logical coordinates)
    pub shape_rect: RectF,
    /// Corner radius of the casting shape (0.0 for sharp corners)
    pub shape_radius: f32,
    /// Shadow mode
    pub mode: ShadowMode,
    /// Horizontal and vertical offset from shape center (logical pixels)
    pub offset: (f32, f32),
    /// Spread: positive expands shadow shape, negative contracts (logical pixels)
    pub spread: f32,
    /// Blur radius: Gaussian sigma controlling softness (logical pixels)
    pub blur_radius: f32,
    /// Shadow color (typically semi-transparent)
    pub color: Color,
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShadowMode {
    /// Gaussian falloff outside shape boundary
    Drop = 0,
    /// Gaussian falloff inside shape boundary
    Inner = 1,
    /// Gaussian band at fixed distance from shape boundary
    Ring = 2,
}
```

### 3.3 Quad Geometry

The shadow is drawn on an expanded quad. The expansion ensures the Gaussian falloff has room to render to near-zero alpha before the quad edge.

```
expansion = blur_radius * 3.0 + spread.abs() + offset.abs()
quad_rect = shape_rect.expanded_by(expansion)
```

The factor of 3.0 on blur_radius ensures the Gaussian reaches ~0.01% alpha at the quad edge (3σ coverage).

For `ShadowMode::Inner`, no quad expansion — the shadow is contained within the shape. The quad is the shape_rect itself.

For `ShadowMode::Ring`, expansion is `ring_distance + blur_radius * 3.0` where ring_distance is derived from spread.

---

## 4. Shader

### 4.1 Uniforms

**Group 0** — Globals (shared, existing):
```wgsl
@group(0) @binding(0) var<uniform> globals: Globals;
```

**Group 1** — Shadow parameters (per-batch):
```wgsl
struct ShadowParams {
    /// Shape center in logical coordinates
    shape_center: vec2<f32>,
    /// Shape half-size in logical coordinates
    shape_half_size: vec2<f32>,
    /// Corner radius (logical pixels)
    shape_radius: f32,
    /// Blur sigma (logical pixels)
    sigma: f32,
    /// Shadow mode: 0 = drop, 1 = inner, 2 = ring
    mode: u32,
    /// Spread (logical pixels, positive = expand shape for shadow)
    spread: f32,
}
```

### 4.2 Vertex Format

```wgsl
struct VertexInput {
    /// Quad corner position (logical coordinates)
    @location(0) position: vec2<f32>,
    /// Shadow color (premultiplied)
    @location(1) color: vec4<f32>,
    /// Offset applied to SDF evaluation point
    @location(2) shadow_offset: vec2<f32>,
}
```

### 4.3 Rounded Rect SDF

Standard rounded-rect signed distance function. Returns negative inside, positive outside, zero on boundary.

```wgsl
fn sd_rounded_rect(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}
```

### 4.4 Fragment Shader

```wgsl
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Evaluate SDF at fragment position, with offset and spread applied
    let eval_pos = input.world_pos - shadow.shadow_offset_interp;
    let half_size_spread = shadow_params.shape_half_size + vec2(shadow_params.spread);
    let radius_spread = max(shadow_params.shape_radius + shadow_params.spread, 0.0);

    let dist = sd_rounded_rect(
        eval_pos,
        shadow_params.shape_center,
        half_size_spread,
        radius_spread,
    );

    var alpha: f32;

    if shadow_params.mode == 0u {
        // Drop shadow: falloff outside shape
        // dist > 0 outside shape, 0 at boundary, < 0 inside
        let d = max(dist, 0.0);
        alpha = exp(-0.5 * pow(d / max(shadow_params.sigma, 0.001), 2.0));
    } else if shadow_params.mode == 1u {
        // Inner shadow: falloff inside shape
        // -dist > 0 inside shape
        let d = max(-dist, 0.0);
        alpha = exp(-0.5 * pow(d / max(shadow_params.sigma, 0.001), 2.0));
        // Only visible inside the shape
        if dist > 0.0 {
            alpha = 0.0;
        }
    } else {
        // Ring (focus ring): band at spread distance from boundary
        // Peak at dist == 0 (on the expanded boundary), falloff both directions
        alpha = exp(-0.5 * pow(dist / max(shadow_params.sigma, 0.001), 2.0));
    }

    // Apply color (premultiplied)
    return input.color * alpha;
}
```

### 4.5 Anti-Aliasing

The Gaussian falloff naturally provides anti-aliased edges — alpha transitions smoothly from 1.0 to 0.0 over the blur radius. No additional AA is needed. For very small blur radii (< 1.0 logical pixel), clamp sigma to 0.5 to prevent hard edges.

---

## 5. Pipeline

### 5.1 PipelineId

```rust
pub enum PipelineId {
    Rect = 1,
    Text = 2,
    Line = 3,
    Image = 4,
    Shape = 5,
    Path = 6,
    /// SPEC-R-013-v2: Analytical shadow pipeline
    Shadow = 7,
}
```

### 5.2 wgpu Pipeline Configuration

```
Vertex buffers:
  buffer 0: position (vec2), color (vec4), shadow_offset (vec2)

Bind groups:
  group 0: Globals uniform (shared with all pipelines)
  group 1: ShadowParams uniform (per-batch)

Blend state:
  src_factor: One
  dst_factor: OneMinusSrcAlpha
  (standard premultiplied alpha)

Depth/stencil: None (z-ordering via batch sort)

Primitive: TriangleList (two triangles per shadow quad)
```

### 5.3 Batching

Shadows batch by `DrawBatchKey` like all other primitives:
- `z_layer`: shadows typically at z_layer below their caster
- `clip_hash`: current clip state
- `pipeline`: PipelineId::Shadow
- `texture_id`: 0 (no texture)

Multiple shadows with identical clip state and z-layer batch into a single draw call. The ShadowParams uniform is set per-batch (all shadows in a batch share the same mode, sigma, and shape parameters).

**Batch break condition:** Different `ShadowParams` values break a batch. In practice, panels with identical shadow styles (same blur, same color) batch together. Varied styles produce one batch per unique configuration.

---

## 6. Batcher Integration

### 6.1 Scene Graph Processing

In `process_node()` (batcher.rs), add arm:

```rust
SceneNode::Shadow(shadow_node) => {
    self.add_shadow(
        shadow_node.shape_rect,
        shadow_node.shape_radius,
        shadow_node.mode,
        shadow_node.offset,
        shadow_node.spread,
        shadow_node.blur_radius,
        shadow_node.color,
        shadow_node.z_index,
    );
}
```

### 6.2 `add_shadow` Method

```rust
pub fn add_shadow(
    &mut self,
    shape_rect: RectF,
    shape_radius: f32,
    mode: ShadowMode,
    offset: (f32, f32),
    spread: f32,
    blur_radius: f32,
    color: Color,
    z_index: ZIndex,
) {
    // Compute expanded quad
    let expansion = match mode {
        ShadowMode::Drop => blur_radius * 3.0 + spread.abs() + offset.0.abs().max(offset.1.abs()),
        ShadowMode::Inner => 0.0, // contained within shape
        ShadowMode::Ring => spread.abs() + blur_radius * 3.0,
    };

    let quad_rect = RectF {
        x: shape_rect.x - expansion + offset.0,
        y: shape_rect.y - expansion + offset.1,
        width: shape_rect.width + expansion * 2.0,
        height: shape_rect.height + expansion * 2.0,
    };

    // Build 4 vertices + 6 indices for quad
    // Store ShadowParams for uniform upload
    // Add to current batch with PipelineId::Shadow
}
```

---

## 7. Widget API

### 7.1 Paint Helper

Add to `PaintContext` or equivalent paint API:

```rust
/// Emit a drop shadow for the given rect with rounded corners.
pub fn paint_drop_shadow(
    &mut self,
    shape_rect: RectF,
    shape_radius: f32,
    offset: (f32, f32),
    spread: f32,
    blur_radius: f32,
    color: Color,
) {
    self.emit(SceneNode::Shadow(ShadowNode {
        shape_rect,
        shape_radius,
        mode: ShadowMode::Drop,
        offset,
        spread,
        blur_radius,
        color,
        z_index: self.current_z() - 1, // below caster
        key: None,
    }));
}

/// Emit an inner shadow for the given rect.
pub fn paint_inner_shadow(
    &mut self,
    shape_rect: RectF,
    shape_radius: f32,
    blur_radius: f32,
    color: Color,
) {
    self.emit(SceneNode::Shadow(ShadowNode {
        shape_rect,
        shape_radius,
        mode: ShadowMode::Inner,
        offset: (0.0, 0.0),
        spread: 0.0,
        blur_radius,
        color,
        z_index: self.current_z() + 1, // above fill, below content
        key: None,
    }));
}
```

### 7.2 Focus Ring (Framework-Automatic)

The framework paints a focus ring automatically for focused widgets. This is not a widget responsibility.

**Mechanism:** After a widget's `paint()` returns, the paint traversal checks:
1. Is this widget the currently focused widget?
2. Does the widget report `hit_testable() == true` (proxy for "interactive")?
3. Is focus ring painting enabled (not suppressed by the widget)?

If all true, the framework emits:

```rust
SceneNode::Shadow(ShadowNode {
    shape_rect: widget_bounds,
    shape_radius: widget_corner_radius(), // from theme or widget
    mode: ShadowMode::Ring,
    offset: (0.0, 0.0),
    spread: focus_ring_distance, // e.g., 2.0 logical pixels outside shape
    blur_radius: focus_ring_softness, // e.g., 1.5 logical pixels
    color: theme.accent_color.with_alpha(0.6),
    z_index: widget_z + 2, // above widget, above inner shadow
    key: None,
})
```

**Widget override:** Widget trait method:

```rust
/// Override focus ring behavior. Return None for default framework ring.
/// Return Some(custom) to replace, or Some(FocusRingStyle::None) to suppress.
fn focus_ring_style(&self) -> Option<FocusRingStyle> {
    None // default: framework paints standard ring
}
```

**Theme integration:** Focus ring color, distance, and softness come from the theme:

```rust
pub struct FocusRingTheme {
    pub color: Color,        // accent color, semi-transparent
    pub distance: f32,       // logical pixels outside shape boundary
    pub softness: f32,       // blur sigma
}
```

---

## 8. Clip Interaction

Shadows must respect the clip stack.

**Drop shadows:** The expanded shadow quad may extend outside its parent's clip region. This is correct — the clip scissor or rounded clip mask will cut the shadow at the parent boundary. No special handling needed beyond existing clip infrastructure.

**Inner shadows:** Entirely contained within the shape. Clip behavior inherited from parent.

**Focus rings:** May extend outside the widget's bounds. The paint system should emit the focus ring SceneNode at the *parent's* clip level, not the widget's own clip level, to avoid self-clipping. This requires the paint traversal to emit the ring *after* popping the widget's clip (if any).

---

## 9. DPI Scaling

All distances (blur_radius, spread, offset, shape dimensions) are in logical pixels. The shader receives logical coordinates via the view_proj matrix. The SDF evaluation is in logical space. The Gaussian falloff width is `sigma` logical pixels regardless of device pixel ratio.

At 2× DPI, a 4.0 logical pixel blur renders with 8 physical pixels of falloff — naturally sharper. No per-DPI adjustment needed.

---

## 10. Performance Characteristics

**Per-fragment cost:** One SDF evaluation (`abs`, `max`, `length`, subtraction) plus one `exp()` call plus one branch on mode. Comparable to the rounded clip fragment shader (R-020) which is already in production.

**Draw calls:** One per shadow batch. Shadows with identical params + clip + z-layer batch into a single call. A typical frame with 5 shadowed panels = 1-3 shadow draw calls (depending on z-layer grouping).

**No offscreen targets.** No texture allocations. No blur passes. Zero additional GPU memory beyond the vertex/index buffer for shadow quads and the ShadowParams uniform.

**Vertex count:** 4 vertices + 6 indices per shadow. 20 shadows = 80 vertices. Negligible.

---

## 11. Testing

### 11.1 Visual Demo

`examples/shadow_demo.rs`:
- Drop shadow on rect (no radius)
- Drop shadow on rounded rect (various radii)
- Drop shadow with offset
- Drop shadow with spread (positive and negative)
- Drop shadow with varying blur radius (0.5 to 20.0)
- Inner shadow on text field-like rect
- Focus ring on button-like rect
- Nested shadows (card inside card)
- Shadow + rounded clip interaction
- Shadow at 1×, 2×, and fractional DPI

### 11.2 Invariant Tests

- Shadow quad expansion is always ≥ 3σ + |spread| + |offset|
- Shadow alpha at quad edge is < 0.01 (Gaussian 3σ guarantee)
- ShadowMode::Inner produces zero alpha outside shape boundary
- ShadowMode::Ring produces peak alpha at spread distance
- Shadow color output is premultiplied
- Shadow SceneNode is immutable after scene build
- Shadow batching groups by identical ShadowParams

---

## 12. Implementation Stages

**Stage 1:** ShadowNode + ShadowMode in scene graph. SceneNode::Shadow variant. No rendering — just data.

**Stage 2:** Shadow shader (shadow.wgsl). SDF function, mode branching, Gaussian falloff. Compile-tested but not wired.

**Stage 3:** PipelineId::Shadow. wgpu render pipeline creation, vertex format, bind group layout.

**Stage 4:** Batcher integration. `add_shadow()`, quad expansion, vertex generation, ShadowParams uniform upload. Batch key with PipelineId::Shadow.

**Stage 5:** Renderer integration. Shadow batches rendered in z-order alongside rect/text/path batches. Clip stack applied.

**Stage 6:** Paint helpers. `paint_drop_shadow()`, `paint_inner_shadow()` on paint context.

**Stage 7:** Focus ring. Framework-automatic painting after widget paint. `focus_ring_style()` override. Theme integration.

**Stage 8:** Demo + tests. `shadow_demo.rs`. Visual verification at multiple DPI. Invariant tests.

**Stage 9:** Apply to existing widgets. Panel, PopupMenu, DropdownMenu, Tooltip, ContextMenu get drop shadows. TextField, TextArea get inner shadows.

---

## 13. What This SPEC Does NOT Cover

- Shadows on arbitrary vector paths (deferred — rounded rects sufficient)
- Soft masking / alpha masks (separate concern)
- Ambient shadows / physically-based lighting (unnecessary for UI)
- Multi-pass Gaussian blur (explicitly rejected)
- Shadow animation (handled by normal scene rebuild — animate ShadowNode params)
- Text shadows (deferred — different pipeline, different SDF)
