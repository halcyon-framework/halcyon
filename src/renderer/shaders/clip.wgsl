// SPEC-HAL-085: Clip Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)
//
// Purpose: Mask application when scissor is insufficient
// (rounded clips, nested clips, future clip stack)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0)
var<uniform> globals: Globals;

// SPEC-HAL-085 Section 7.2: Clip pipeline resources at group(1)
@group(1) @binding(0)
var clip_mask_tex: texture_2d<f32>;

@group(1) @binding(1)
var clip_sampler: sampler;

// Vertex input for clip application
struct VertexInput {
    @location(0) position: vec2<f32>,  // Absolute Logical coords
    @location(1) uv: vec2<f32>,        // Mask texture coordinates
    @location(2) color: vec4<f32>,     // Content color (premultiplied RGBA)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

// SPEC-HAL-085 INV-SHADER-CLIP-1: Clip shader only attenuates coverage
// Must not modify coordinates, reorder layers, or invent geometry
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample mask coverage (stored in alpha or red channel)
    let mask = textureSample(clip_mask_tex, clip_sampler, input.uv);
    let coverage = mask.a;

    // Apply mask coverage to content color
    // Output is premultiplied: (r*a*cov, g*a*cov, b*a*cov, a*cov)
    let final_color = input.color * coverage;

    if (final_color.a <= 0.0) {
        discard;
    }

    return final_color;
}
