# SPEC-R-012-v2: Gradient Pipeline (Linear & Radial, Multi-Stop)

### *Single-Pass Analytical Gradient Rendering*

**Author:** JC
**Status:** Design
**Version:** 2.0
**Supersedes:** SPEC-R-009 (GPU Gradient Pipeline v1), SPEC-R-012 (GPU Gradients v2)

**Depends On:**

* SPEC-R-001 Clip Stack Rework (Complete)

**No other dependencies.** Does not depend on R-005 (SDF), R-007 (blend engine), R-008 (composition graph), R-010 (texture filters), R-011 (blend modes), or any other unimplemented R-series SPEC.

---

## 1. Purpose

Add gradient-filled rectangles and rounded rectangles to Halcyon's renderer. Two gradient types:

1. **Linear gradient** — color interpolation along a direction vector. Backgrounds, buttons, headers.
2. **Radial gradient** — color interpolation from center outward. Spotlight effects, circular progress.

Multi-stop (up to 8 stops). Gamma-correct interpolation in linear color space. Single-pass fragment shader. No textures, no lookup tables, no offscreen targets.

---

## 2. Design Principles

**P-1: Analytical interpolation.** Gradient color computed per-fragment from stop positions and colors. No gradient texture baking.

**P-2: Separate pipeline.** `PipelineId::Gradient = 8`. Gradient fragments require stop array data and interpolation logic that the rect shader does not. Same reasoning as shadow pipeline separation — don't bloat the common case.

**P-3: Linear color space.** `Color` constructors decode authored sRGB when applicable; stored `Color` channels are straight linear RGBA. Gradient lowering only premultiplies them, once. Interpolation and output are premultiplied linear. SPEC-HAL-085A §6.8 is the binding implemented profile; this document remains Design.

**P-4: Inline stop storage.** Up to 8 color stops in a uniform buffer. No storage buffer, no extra bind group. 8 stops covers virtually all UI gradient use cases. If more are needed in the future, the uniform can be replaced with a storage buffer without changing the shader math.

**P-5: Rounded corner support.** Gradient rects may have rounded corners. Alpha masking via rounded-rect SDF in the fragment shader (same technique as R-020). No separate clip needed.

**P-6: Premultiplied alpha throughout.** All output is `(r*a, g*a, b*a, a)`.

---

## 3. SceneNode

### 3.1 New Variant

```rust
pub enum SceneNode {
    // ... existing variants ...

    /// SPEC-R-012-v2: Gradient-filled rectangle
    GradientRect(GradientRectNode),
}
```

### 3.2 GradientRectNode

```rust
#[derive(Debug, Clone)]
pub struct GradientRectNode {
    /// Rectangle bounds (logical coordinates)
    pub rect: RectF,
    /// Corner radius (0.0 for sharp corners)
    pub radius: f32,
    /// Gradient definition
    pub gradient: GradientDef,
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}
```

### 3.3 GradientDef

```rust
#[derive(Debug, Clone)]
pub struct GradientDef {
    /// Gradient type
    pub kind: GradientKind,
    /// Color stops (minimum 2, maximum 8, ordered by offset)
    pub stops: Vec<ColorStop>,
    /// Start point (linear) or center (radial), in normalized rect-local space [0..1]
    pub p0: (f32, f32),
    /// End point (linear) or edge point (radial), in normalized rect-local space [0..1]
    pub p1: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GradientKind {
    Linear = 0,
    Radial = 1,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorStop {
    /// Position along gradient [0.0 .. 1.0]
    pub offset: f32,
    /// Straight linear RGBA; premultiplied once before upload
    pub color: Color,
}
```

### 3.4 Coordinate Space

Gradient coordinates `p0` and `p1` are in **normalized rect-local space**:
- `(0.0, 0.0)` = top-left of rect
- `(1.0, 1.0)` = bottom-right of rect
- `(0.0, 0.0)` to `(0.0, 1.0)` = top-to-bottom vertical gradient
- `(0.0, 0.0)` to `(1.0, 0.0)` = left-to-right horizontal gradient
- `(0.0, 0.0)` to `(1.0, 1.0)` = diagonal gradient
- Radial: `p0` = center, distance to `p1` = radius

This decouples gradient definition from rect size. A vertical gradient defined as `(0,0)→(0,1)` works identically on a 50px rect and a 500px rect.

---

## 4. Shader

### 4.1 Uniforms

**Group 0** — Globals (shared, existing):
```wgsl
@group(0) @binding(0) var<uniform> globals: Globals;
```

**Group 1** — Gradient parameters (per-batch):
```wgsl
const MAX_STOPS: u32 = 8u;

struct GradientStop {
    color: vec4<f32>,  // linear-space, premultiplied
    offset: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

struct RoundedClip {
    rect_logical: vec4<f32>,
    radii_logical: vec4<f32>,
    inner_min: vec2<f32>,
    inner_max: vec2<f32>,
    outer_min: vec2<f32>,
    outer_max: vec2<f32>,
    aa_radius_logical: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

struct GradientParams {
    /// Rect bounds (logical coordinates) for local-space computation
    rect_origin: vec2<f32>,
    rect_size: vec2<f32>,
    /// Gradient start/center (normalized 0..1)
    p0: vec2<f32>,
    /// Gradient end/edge (normalized 0..1)
    p1: vec2<f32>,
    /// Corner radius for alpha masking (logical pixels)
    corner_radius: f32,
    /// Gradient kind: 0 = linear, 1 = radial
    kind: u32,
    /// Number of active stops (2..8)
    stop_count: u32,
    clip_enabled: u32,
    /// Color stops, sorted by offset
    stops: array<GradientStop, 8>,
    clip: RoundedClip,
}
```

Total uniform size: 48 + (32 × 8) + 80 = 384 bytes, within the existing 512-byte aligned slot.

### 4.2 Vertex Format

```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
}
```

Minimal vertex — just quad corners. All gradient computation in fragment shader from uniforms.

### 4.3 Linear Color Boundary

`Color` already stores linear RGB. Neither CPU Gradient lowering nor its shader
performs another sRGB decode. CPU lowering premultiplies once; the shader
interpolates and outputs premultiplied linear values under SPEC-HAL-085A §6.8.

### 4.4 Gradient Evaluation

```wgsl
fn sample_gradient(t: f32) -> vec4<f32> {
    // Clamp t to [0, 1]
    let tc = clamp(t, 0.0, 1.0);

    // Find the two stops surrounding tc
    var lower_idx = 0u;
    for (var i = 1u; i < gradient_params.stop_count; i = i + 1u) {
        if gradient_params.stops[i].offset <= tc {
            lower_idx = i;
        }
    }
    let upper_idx = min(lower_idx + 1u, gradient_params.stop_count - 1u);

    let lower = gradient_params.stops[lower_idx];
    let upper = gradient_params.stops[upper_idx];

    // Interpolation factor between the two stops
    let range = upper.offset - lower.offset;
    let factor = select(0.0, (tc - lower.offset) / range, range > 0.001);

    // Linear interpolation in linear color space (colors already linear + premultiplied)
    return mix(lower.color, upper.color, factor);
}
```

### 4.5 Fragment Shader

```wgsl
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Compute position in normalized rect-local space [0..1]
    let local = (input.world_pos - gradient_params.rect_origin) / gradient_params.rect_size;

    // Compute gradient parameter t
    var t: f32;
    if gradient_params.kind == 0u {
        // Linear: project local position onto gradient axis
        let axis = gradient_params.p1 - gradient_params.p0;
        let axis_len_sq = dot(axis, axis);
        if axis_len_sq > 0.0001 {
            t = dot(local - gradient_params.p0, axis) / axis_len_sq;
        } else {
            t = 0.0;
        }
    } else {
        // Radial: distance from center normalized by radius
        let center = gradient_params.p0;
        let edge = gradient_params.p1;
        let radius = length(edge - center);
        if radius > 0.0001 {
            t = length(local - center) / radius;
        } else {
            t = 0.0;
        }
    }

    var color = sample_gradient(t);

    // Rounded corner alpha masking
    if gradient_params.corner_radius > 0.0 {
        let center = gradient_params.rect_origin + gradient_params.rect_size * 0.5;
        let half_size = gradient_params.rect_size * 0.5;
        let r = gradient_params.corner_radius;
        let d = abs(input.world_pos - center) - half_size + vec2(r);
        let dist = length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
        let aa = fwidth(dist);
        let mask = 1.0 - smoothstep(-aa, aa, dist);
        color = color * mask;
    }

    return color;
}
```

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
    Shadow = 7,   // SPEC-R-013-v2
    /// SPEC-R-012-v2: Gradient-filled rectangles
    Gradient = 8,
}
```

### 5.2 wgpu Pipeline Configuration

```
Vertex buffers:
  buffer 0: position (vec2) — quad corners only

Bind groups:
  group 0: Globals uniform (shared)
  group 1: GradientParams uniform (per-batch)

Blend state:
  src_factor: One
  dst_factor: OneMinusSrcAlpha
  (standard premultiplied alpha)

Depth/stencil: None

Primitive: TriangleList (two triangles per gradient quad)
```

### 5.3 Batching

Gradient rects batch by `DrawBatchKey`:
- `z_layer`: as specified
- `clip_hash`: current clip state
- `pipeline`: PipelineId::Gradient
- `texture_id`: 0

**Batch break:** Different `GradientParams` values break a batch. Each unique gradient definition (stops, kind, p0, p1, corner radius) produces a separate batch. In practice, buttons with identical gradient styles batch together.

---

## 6. Batcher Integration

### 6.1 Scene Graph Processing

```rust
SceneNode::GradientRect(gradient_node) => {
    self.add_gradient_rect(
        gradient_node.rect,
        gradient_node.radius,
        &gradient_node.gradient,
        gradient_node.z_index,
    );
}
```

### 6.2 `add_gradient_rect` Method

```rust
pub fn add_gradient_rect(
    &mut self,
    rect: RectF,
    radius: f32,
    gradient: &GradientDef,
    z_index: ZIndex,
) {
    // Validate stop count
    debug_assert!(gradient.stops.len() >= 2 && gradient.stops.len() <= 8);

    // Build 4 vertices (quad corners) + 6 indices
    // Premultiply already-linear stop colors once on CPU
    // Upload GradientParams uniform
    // Add to batch with PipelineId::Gradient
}
```

### 6.3 CPU Premultiplication

Stop colors are already straight linear RGBA:

```rust
fn to_linear_premultiplied(color: Color) -> [f32; 4] {
    [color.r * color.a, color.g * color.a, color.b * color.a, color.a]
}
```

---

## 7. Widget API

### 7.1 Paint Helper

```rust
/// Emit a linear gradient rectangle.
pub fn paint_linear_gradient(
    &mut self,
    rect: RectF,
    radius: f32,
    from: (f32, f32),      // normalized start point
    to: (f32, f32),        // normalized end point
    stops: &[ColorStop],
) {
    self.emit(SceneNode::GradientRect(GradientRectNode {
        rect,
        radius,
        gradient: GradientDef {
            kind: GradientKind::Linear,
            stops: stops.to_vec(),
            p0: from,
            p1: to,
        },
        z_index: self.current_z(),
        key: None,
    }));
}

/// Emit a vertical gradient (convenience: top color to bottom color).
pub fn paint_vertical_gradient(
    &mut self,
    rect: RectF,
    radius: f32,
    top: Color,
    bottom: Color,
) {
    self.paint_linear_gradient(
        rect,
        radius,
        (0.0, 0.0),
        (0.0, 1.0),
        &[
            ColorStop { offset: 0.0, color: top },
            ColorStop { offset: 1.0, color: bottom },
        ],
    );
}

/// Emit a radial gradient rectangle.
pub fn paint_radial_gradient(
    &mut self,
    rect: RectF,
    radius: f32,
    center: (f32, f32),    // normalized center
    edge: (f32, f32),      // normalized edge point (defines radius)
    stops: &[ColorStop],
) {
    self.emit(SceneNode::GradientRect(GradientRectNode {
        rect,
        radius,
        gradient: GradientDef {
            kind: GradientKind::Radial,
            stops: stops.to_vec(),
            p0: center,
            p1: edge,
        },
        z_index: self.current_z(),
        key: None,
    }));
}
```

---

## 8. Clip Interaction

SPEC-HAL-085A §6.8 defines the implemented clip profile. CPU scissor and the
current effective rounded-clip projection apply in main and overlay layers.
The projection is copied into the existing Gradient parameter slot, keyed with
the clip dependency; it does not consume shared Rect/Image clip slots. This
does not admit additional nested clip topology beyond the existing projection.

The corner radius alpha masking (Section 4.5) handles the gradient rect's own rounded corners. This is independent of the clip stack's rounded clipping, which handles parent container boundaries.

---

## 9. Performance Characteristics

**Per-fragment cost:** One dot product or length computation for t. One loop over stops (max 8 iterations, no branching within loop). One `mix()` interpolation. One optional SDF + smoothstep for rounded corners. Comparable to the rect pipeline with rounded clip active.

**Draw calls:** One per gradient batch. Multiple gradient rects with identical params batch into one call. Varied gradients produce one call per unique definition.

**Memory:** 384 bytes per GradientParams uniform in a 512-byte aligned slot. No textures. No offscreen targets. Solid-only paths acquire no new work.

**Vertex count:** 4 vertices + 6 indices per gradient rect. Negligible.

---

## 10. Future Extensions (Not In This SPEC)

- **Angular/conical gradients** — add `GradientKind::Angular = 2`, compute t from `atan2`. Same shader structure, different t computation.
- **Elliptical gradients** — non-uniform scaling of radial distance. Minor shader change.
- **Repeating/reflecting gradients** — `fract(t)` or `1.0 - abs(fract(t * 2.0) - 1.0)` applied before `sample_gradient`. Uniform flag.
- **Gradient on arbitrary paths** — gradient as fill for path pipeline. Requires passing gradient params to path instances.
- **More than 8 stops** — replace uniform with storage buffer. Shader unchanged.

All extensions are additive. This SPEC's shader structure accommodates them without rewrite.

---

## 11. Testing

### 11.1 Visual Demo

`examples/gradient_demo.rs`:
- 2-stop vertical gradient (top to bottom)
- 2-stop horizontal gradient (left to right)
- 2-stop diagonal gradient
- 4-stop gradient (rainbow-like)
- 8-stop gradient (stress test)
- Radial gradient (center to edge)
- Rounded gradient rect (various radii)
- Gradient + clip region interaction
- Side-by-side sRGB vs linear interpolation comparison (verify no mid-tone banding)
- Multiple DPI: 1×, 2×, fractional

### 11.2 Invariant Tests

- Gradient with 2 identical stops = solid color
- Gradient with stops at 0.0 and 1.0 covers full rect
- Already-linear stop colors are premultiplied once before upload
- Gradient output is premultiplied alpha
- Corner radius masking produces zero alpha outside rounded boundary
- GradientParams uniform fits in 384 bytes within its existing 512-byte slot

---

## 12. Implementation Stages

**Stage 1:** GradientRectNode + GradientDef + GradientKind + ColorStop in scene graph. SceneNode::GradientRect variant. No rendering.

**Stage 2:** Gradient shader (gradient.wgsl). Linear and radial t computation, stop interpolation, rounded corner masking. Compile-tested.

**Stage 3:** PipelineId::Gradient. wgpu pipeline creation, vertex format, bind group layout.

**Stage 4:** Batcher integration. `add_gradient_rect()`, CPU color conversion, GradientParams uniform upload, batching.

**Stage 5:** Renderer integration. Gradient batches rendered in z-order.

**Stage 6:** Paint helpers. `paint_linear_gradient()`, `paint_vertical_gradient()`, `paint_radial_gradient()`.

**Stage 7:** Demo + tests. `gradient_demo.rs`. Visual verification.

**Stage 8:** Apply to existing widgets. ProgressBar fill, Button hover tint, Panel header backgrounds.
