// SPEC-R-013-v2: Shadow Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)
//
// Supports three modes via shadow_params.mode:
//   0 = Drop shadow:  Gaussian falloff outside shape boundary
//   1 = Inner shadow: Gaussian falloff inside shape boundary
//   2 = Ring:         Gaussian band at shape boundary (focus ring)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms (shared across ALL pipelines)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// SPEC-R-013-v2: Shadow-specific parameters (per-batch uniform)
struct ShadowParams {
    // Shape geometry (logical coordinates)
    shape_center: vec2<f32>,
    shape_half_size: vec2<f32>,
    // Corner radius (logical pixels)
    shape_radius: f32,
    // Gaussian sigma (logical pixels)
    sigma: f32,
    // 0 = drop, 1 = inner, 2 = ring
    mode: u32,
    // Spread: expand/contract shadow shape (logical pixels)
    spread: f32,
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0) var<uniform> globals: Globals;
// SPEC-R-013-v2: group(1)=shadow parameters
@group(1) @binding(0) var<uniform> shadow_params: ShadowParams;

// Vertex input: quad corner + color + offset
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shadow_offset: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_pos: vec2<f32>,
    @location(2) shadow_offset: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    output.world_pos = input.position;
    output.shadow_offset = input.shadow_offset;
    return output;
}

// Rounded rect signed distance function.
// Returns negative inside, positive outside, zero on boundary.
fn sd_rounded_rect(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Evaluate SDF at fragment position, applying shadow offset and spread
    let eval_pos = input.world_pos - input.shadow_offset;
    let half_size_spread = shadow_params.shape_half_size + vec2(shadow_params.spread);
    let radius_spread = max(shadow_params.shape_radius + shadow_params.spread, 0.0);

    let dist = sd_rounded_rect(
        eval_pos,
        shadow_params.shape_center,
        half_size_spread,
        radius_spread,
    );

    // Clamp sigma to minimum 0.5 to prevent hard edges at very small blur radii
    let sigma = max(shadow_params.sigma, 0.5);

    var alpha: f32;

    if shadow_params.mode == 0u {
        // Drop shadow: Gaussian falloff outside shape boundary
        // dist > 0 outside, 0 at boundary, < 0 inside
        let d = max(dist, 0.0);
        alpha = exp(-0.5 * pow(d / sigma, 2.0));
    } else if shadow_params.mode == 1u {
        // Inner shadow: Gaussian falloff inside shape boundary
        let d = max(-dist, 0.0);
        alpha = exp(-0.5 * pow(d / sigma, 2.0));
        // Zero alpha outside the shape
        if dist > 0.0 {
            alpha = 0.0;
        }
    } else {
        // Ring (focus ring): Gaussian band centered on the boundary
        // Peak at dist == 0, falloff symmetrically
        alpha = exp(-0.5 * pow(dist / sigma, 2.0));
    }

    // Apply shadow color (premultiplied alpha)
    let c = input.color;

    // Anti-banding dither: add ±0.5/255 noise to break quantization steps
    // Uses integer hash of screen pixel position — fully deterministic across frames
    let pixel = vec2<u32>(u32(input.position.x), u32(input.position.y));
    var h = (pixel.x * 73856093u) ^ (pixel.y * 19349663u);
    h = (h >> 16u) ^ h;
    let dither_noise = f32(h & 255u) / 255.0;
    let dither = (dither_noise - 0.5) / 255.0;

    return vec4<f32>(c.rgb * alpha + vec3(dither * alpha), c.a * alpha);
}
