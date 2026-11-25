# SPEC-R-022: SDF RoundedRect Pipeline

### *Analytical Distance Field Rendering for Rounded Rects and Borders*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-R-021 (RoundedRect Pipeline Contract),
SPEC-R-013-v2 (Shadow Pipeline — reference pattern),
SPEC-HAL-085 (Rect Pipeline)

---

# 1. Purpose

Replace tessellation-based rounded rect and rounded border
rendering with an analytical Signed Distance Field (SDF)
pipeline. The SDF evaluates shape distance per-pixel in the
fragment shader, producing perfect geometry at any position,
any DPR, any scroll offset — eliminating all tessellation
artifacts.

This SPEC supersedes the tessellation paths in
`add_rounded_rect` and `add_rounded_border` for rounded
geometry. It satisfies SPEC-R-021 Rule 1 (RoundedRect uses
rect pipeline) by routing through a dedicated SDF pipeline
that uses per-batch GPU scissor clipping.

---

# 2. Problem

Tessellation-based rounded rects have three inherent defects:

1. **Sub-pixel border thickness variation.** Tessellated
   vertices are authored in logical space and transformed
   to screen space via scroll + DPR. The final pixel position
   of each vertex depends on the full transform chain. At
   1px border width, half-pixel misalignment produces visible
   2px antialiased edges on some sides but crisp 1px on others.

2. **Corner artifacts.** Ring meshes (outer − inner contour)
   and stroke meshes produce triangulation-dependent kinks at
   arc-to-straight junctions. Quality varies with radius,
   border width, and segment count.

3. **Performance overhead.** Each rounded rect requires CPU
   tessellation (Path::rounded_rect → flatten → triangulate),
   mesh cache lookup, and path pipeline routing. Path pipeline
   uses full-viewport scissor + shader discard for clipping,
   which breaks per-batch clip isolation (SPEC-R-021 §2.2).

---

# 3. Solution

Evaluate the rounded rect SDF analytically in the fragment
shader. The `sd_rounded_rect` function (already in
`shadow.wgsl:64-67`) computes the exact signed distance from
any pixel to the nearest edge of a rounded rect:

```wgsl
fn sd_rounded_rect(p: vec2, center: vec2, half_size: vec2, radius: f32) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}
```

**Fill:** `coverage = 1.0 - smoothstep(-aa, aa, sd)`
**Border:** `coverage = 1.0 - smoothstep(-aa, aa, abs(sd + bw/2) - bw/2)`
**Fill+Border:** Both evaluated, composited.

AA radius = 0.5 logical pixels (half a physical pixel after
DPR scaling). Resolution-independent, scroll-independent,
position-independent.

---

# 4. Architecture

```
SceneNode::RoundedRect / SceneNode::BorderRect
    ↓
Batcher::flatten_node
    ↓
PrimitiveData::SdfRect { rect, fill_color, border_color, radius, border_width }
    ↓
build_sdf_rect_buffers (4 vertices per rect, all rects batchable)
    ↓
SdfRect GPU Pipeline (sdf_rect.wgsl)
    ↓
Fragment shader: sd_rounded_rect → coverage → premul alpha
```

## 4.1 Vertex Layout

SDF params are packed per-vertex (flat interpolation) to enable
batching all rounded rects into a single draw call:

```rust
struct SdfRectVertex {
    position: [f32; 2],      // quad corner in logical coords
    fill_color: [f32; 4],    // premultiplied RGBA
    border_color: [f32; 4],  // premultiplied RGBA
    rect_center: [f32; 2],   // rect center for SDF eval
    rect_half_size: [f32; 2],// rect half dimensions
    radius: f32,             // corner radius
    border_width: f32,       // 0 = fill only
}
// = 18 floats × 4 bytes = 72 bytes per vertex
// × 4 vertices = 288 bytes per rounded rect
// vs tessellation: ~40+ vertices × 8 bytes = 320+ bytes
```

All 4 vertices of a quad carry identical SDF params (flat
interpolated). The fragment shader evaluates the SDF at
each pixel's logical position.

## 4.2 Pipeline

New `PipelineId::SdfRect` in the batcher. Uses per-batch GPU
scissor clipping (same as Rect pipeline), satisfying
SPEC-R-021 Rule 1. No shader discard for clipping.

Bind groups:
- group(0): Globals (shared with all pipelines)
- group(1): (none — all params are per-vertex)

## 4.3 Batching

All SdfRect primitives with the same clip hash batch into one
draw call. Unlike shadow/gradient which need per-batch params
uniforms, SdfRect carries all params in vertex data. One draw
call can render hundreds of rounded rects.

---

# 5. Shader

```wgsl
// sdf_rect.wgsl

struct Globals {
    view_proj: mat4x4<f32>,
    surface_size: vec2<f32>,
    time_seconds: f32,
    flags: u32,
}

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) rect_center: vec2<f32>,
    @location(4) rect_half_size: vec2<f32>,
    @location(5) radius: f32,
    @location(6) border_width: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) @interpolate(flat) fill_color: vec4<f32>,
    @location(2) @interpolate(flat) border_color: vec4<f32>,
    @location(3) @interpolate(flat) rect_center: vec2<f32>,
    @location(4) @interpolate(flat) rect_half_size: vec2<f32>,
    @location(5) @interpolate(flat) radius: f32,
    @location(6) @interpolate(flat) border_width: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj *
        vec4<f32>(input.position, 0.0, 1.0);
    output.world_pos = input.position;
    output.fill_color = input.fill_color;
    output.border_color = input.border_color;
    output.rect_center = input.rect_center;
    output.rect_half_size = input.rect_half_size;
    output.radius = input.radius;
    output.border_width = input.border_width;
    return output;
}

fn sd_rounded_rect(
    p: vec2<f32>, center: vec2<f32>,
    half_size: vec2<f32>, radius: f32,
) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) +
        min(max(d.x, d.y), 0.0) - radius;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let scale_factor = globals.view_proj[0][0] *
        globals.surface_size.x / 2.0;
    let aa = 0.5 / scale_factor; // half physical pixel

    let sd = sd_rounded_rect(
        input.world_pos,
        input.rect_center,
        input.rect_half_size,
        input.radius,
    );

    var result = vec4<f32>(0.0);

    // Fill
    if (input.fill_color.a > 0.0) {
        let fill_coverage = 1.0 - smoothstep(-aa, aa, sd);
        let fc = input.fill_color;
        result = vec4<f32>(fc.rgb * fill_coverage, fc.a * fill_coverage);
    }

    // Border
    let bw = input.border_width;
    if (bw > 0.0 && input.border_color.a > 0.0) {
        // SDF of the annular ring: border centered on the
        // rect edge (outer edge at sd=0, inner at sd=-bw).
        // Shift SDF inward by bw/2, then abs gives distance
        // to the ring centerline.
        let border_sd = abs(sd + bw * 0.5) - bw * 0.5;
        let border_coverage = 1.0 - smoothstep(-aa, aa, border_sd);
        let bc = input.border_color;
        let border = vec4<f32>(
            bc.rgb * border_coverage,
            bc.a * border_coverage,
        );
        // Composite border over fill (premultiplied)
        result = border + result * (1.0 - border.a);
    }

    if (result.a <= 0.0) { discard; }
    return result;
}
```

---

# 6. Invariants

**INV-SDF-001:** SdfRect uses per-batch GPU scissor clipping.
No shader discard for clip regions. Satisfies SPEC-R-021 Rule 1.

**INV-SDF-002:** All SDF params are per-vertex (flat
interpolated). No per-batch uniform buffer for shape geometry.
This enables batching all SdfRects into one draw call per
clip region.

**INV-SDF-003:** AA radius = 0.5 / scale_factor (half a
physical pixel). Resolution-independent, position-independent.

**INV-SDF-004:** Border is an annular ring evaluated
analytically. Uniform thickness by construction at any
sub-pixel position.

**INV-SDF-005:** The quad is expanded by 1 logical pixel on
each side to provide room for AA blending. Without expansion,
pixels at the exact rect boundary would be clipped before the
smoothstep can evaluate them.

---

# 7. Forbidden Behaviors

**FORBID-SDF-001:** SdfRect MUST NOT use the path pipeline.
The path pipeline uses full-viewport scissor + shader discard,
breaking per-batch clip isolation (SPEC-R-021 §2.2).

**FORBID-SDF-002:** SdfRect MUST NOT tessellate rounded
corners on the CPU. The fragment shader evaluates geometry
analytically.

---

# 8. Migration

## 8.1 What Changes

| Before | After |
|--------|-------|
| `add_rounded_rect` → tessellate → path pipeline | `add_rounded_rect` → SdfRect primitive → SDF pipeline |
| `add_rounded_border` → stroke mesh → path pipeline | `add_rounded_border` → SdfRect primitive (bw > 0) → SDF pipeline |

## 8.2 What Stays

- `Path::rounded_rect` — still used for fills that go through
  the path pipeline (e.g., Sparkline, Gauge backgrounds)
- `push_rounded_rect` / `push_border_rounded_rect` scene API
  — unchanged, batcher routes differently
- Shadow, gradient, color picker pipelines — unaffected

---

# 9. Implementation Phases

**Phase 1:** Shader + pipeline + vertex type. Create
`sdf_rect.wgsl`, `SdfRectVertex`, pipeline creation method.
Static demo rendering a single SDF rounded rect.

**Phase 2:** Batcher integration. `add_rounded_rect` emits
`PrimitiveData::SdfRect`. Buffer builder, GPU upload, render
dispatch. Path tessellation still available as fallback.

**Phase 3:** Border integration. `add_rounded_border` emits
`PrimitiveData::SdfRect` with border params. Remove stroke
mesh path.

**Phase 4:** Cleanup. Remove dead tessellation code paths for
rounded rects and borders. Update SPEC-R-021 contract.

---

# 10. Constitutional Compliance (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | No change. Scene graph nodes unchanged. |
| B — Phase Separation | Preserved. SDF eval is pure fragment shader. No state mutation. |
| C — ID-based Identity | No change. |
| D — Pure Data | Preserved. PrimitiveData::SdfRect is pure data. |
| E — GPU-First | Strengthened. Shape evaluation moves from CPU tessellation to GPU fragment shader. |
| F — Sovereignty | No change. |

No invariants weakened.
