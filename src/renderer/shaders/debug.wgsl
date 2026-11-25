// SPEC-HAL-085: Debug Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)
//
// Purpose: Visual inspection only (overdraw heatmap, mask view, atlas view, etc.)
// SPEC-HAL-085 INV-SHADER-DBG-1: Must be gated and never required for correct UI output

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

// Debug pipeline may mirror rect pipeline resources
// Must never require reading SceneCache or any semantic state

// Vertex input for debug overlays (same as rect pipeline)
struct VertexInput {
    @location(0) position: vec2<f32>,  // Absolute Logical coords
    @location(1) color: vec4<f32>,     // Debug color (premultiplied RGBA)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Simple passthrough for debug visualization
    // Color is already premultiplied
    let c = input.color;
    return vec4<f32>(c.rgb * c.a, c.a);
}
