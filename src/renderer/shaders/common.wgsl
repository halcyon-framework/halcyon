// SPEC-HAL-085: Common Shader Definitions
// This file contains shared types and functions used by all shaders.
// It is NOT a standalone shader module - it's included by other shaders.

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms
// All pipelines MUST use this exact layout at @group(0) @binding(0)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// SPEC-HAL-085 Section 6.2: Canonical Vertex Input Path A (QuadVertex + QuadInstance)
// Used by rect, image, text, clip application, debug overlays
struct QuadVertex {
    @location(0) local_pos: vec2<f32>, // typically (0..1, 0..1) for quad corners
    @location(1) uv: vec2<f32>,        // used by image/text, ignored by rect
}

struct QuadInstance {
    @location(2) rect: vec4<f32>,      // x,y,w,h in Absolute Logical
    @location(3) color: vec4<f32>,     // premultiplied RGBA
    @location(4) extra0: vec4<f32>,    // pipeline-specific (radius, border, etc.)
    @location(5) extra1: vec4<f32>,    // pipeline-specific (glyph params, etc.)
}

// SPEC-HAL-085 Section 6.2: Canonical Vertex Input Path B (BakedVertex)
// Used by shape/path pipelines when geometry is already expanded
struct BakedVertex {
    @location(0) pos: vec2<f32>,       // Absolute Logical
    @location(1) uv: vec2<f32>,        // optional
    @location(2) color: vec4<f32>,     // premultiplied RGBA
    @location(3) extra: vec4<f32>,     // optional
}

// Helper: Transform from Absolute Logical to NDC using view_proj
fn logical_to_ndc(globals: Globals, pos: vec2<f32>) -> vec4<f32> {
    return globals.view_proj * vec4<f32>(pos, 0.0, 1.0);
}

// Helper: Premultiply alpha for fragment output
fn premultiply(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb * color.a, color.a);
}
