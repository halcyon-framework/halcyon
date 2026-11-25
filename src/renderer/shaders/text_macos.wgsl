// SPEC-HAL-085: Text Pipeline Shader (macOS)
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms (shared across ALL pipelines)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// R-010C: Text-specific globals uniform for per-channel LCD subpixel rendering
struct TextParams {
    gamma_r: f32,
    gamma_g: f32,
    gamma_b: f32,
    _pad: f32,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) screen_uv: vec2<f32>,  // Unused on macOS but kept for vertex layout compatibility
    @location(4) downsample_u: f32,     // R-010B FIX: Per-glyph downsample offset
}

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) downsample_u: f32,
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0)
var<uniform> globals: Globals;

// SPEC-HAL-085 INV-SHADER-BIND-1: group(1)=pipeline-specific resources
@group(1) @binding(0)
var atlas_tex: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

@group(1) @binding(2)
var<uniform> text_params: TextParams;

@vertex
fn vs_main(input: VertexInput) -> VSOut {
    var output: VSOut;
    output.pos = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.downsample_u = input.downsample_u;
    return output;
}

// R-010C: TRUE SUBPIXEL LCD RENDERING (RGB MASK MODE)
// Per-channel subpixel sampling matching CoreText / Quartz
//
// SPEC-EXIT-A1: Solid color mode for text backgrounds.
// When downsample_u < 0, render as solid vertex color (no atlas sampling).
// This enables selection/caret backgrounds to render in the text pass.
//
// ALPHA FIX: All outputs must be premultiplied for correct blending.
// Blend state is One/OneMinusSrcAlpha (premultiplied alpha blending).
@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    let offset = in.downsample_u;

    // SPEC-EXIT-A1: Solid color mode for text backgrounds
    // When downsample_u < 0, skip texture sampling and output solid color
    // ALPHA FIX: Premultiply for correct blending
    if (offset < 0.0) {
        let c = in.color;
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    // Three taps for RGB subpixel structure
    let left   = textureSample(atlas_tex, atlas_sampler, in.uv - vec2<f32>(offset, 0.0));
    let center = textureSample(atlas_tex, atlas_sampler, in.uv);
    let right  = textureSample(atlas_tex, atlas_sampler, in.uv + vec2<f32>(offset, 0.0));

    // The atlas AA mask stores coverage in RGB channels.
    // We reconstruct LCD coverage by channel:
    let cov_r = left.r;
    let cov_g = center.g;
    let cov_b = right.b;

    // Apply per-channel gamma
    let gamma_r = text_params.gamma_r;
    let gamma_g = text_params.gamma_g;
    let gamma_b = text_params.gamma_b;

    let cov_r2 = pow(cov_r, 1.0 / gamma_r);
    let cov_g2 = pow(cov_g, 1.0 / gamma_g);
    let cov_b2 = pow(cov_b, 1.0 / gamma_b);

    // Apply coverage to glyph color
    // ALPHA FIX: Multiply by input alpha for correct opacity support
    // Output is premultiplied: (r * a * cov, g * a * cov, b * a * cov, a * max_cov)
    let a = in.color.a;
    let final_color = vec4<f32>(
        in.color.r * a * cov_r2,
        in.color.g * a * cov_g2,
        in.color.b * a * cov_b2,
        a * max(max(cov_r2, cov_g2), cov_b2)
    );

    return final_color;
}
