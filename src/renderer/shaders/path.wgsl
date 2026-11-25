// SPEC-HAL-085: Path Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms (shared across ALL pipelines)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0)
var<uniform> globals: Globals;

// SPEC-HAL-085 INV-SHADER-BIND-1: group(1)=pipeline-specific
// Clip rectangles storage buffer
struct ClipRect {
    min: vec2<f32>,
    max: vec2<f32>,
}

@group(1) @binding(0)
var<storage, read> clips: array<ClipRect>;

// Helper: get scale factor from view_proj (derived from matrix)
fn get_scale_factor() -> f32 {
    // Scale factor is encoded in view_proj[0][0] and surface_size
    // For 2D ortho: view_proj[0][0] = 2.0 * scale_factor / surface_size.x
    return globals.view_proj[0][0] * globals.surface_size.x / 2.0;
}

// Vertex input from PathVertex (buffer 0, per-vertex)
struct VertexInput {
    @location(0) position: vec2<f32>,
    // Instance attributes from PathInstance (buffer 1, per-instance)
    @location(1) color: vec4<f32>,
    @location(2) transform_0: vec4<f32>,  // [m00, m01, m02, m10]
    @location(3) transform_1: vec2<f32>,  // [m11, m12]
    @location(4) clip_id: u32,
    @location(5) stroke_flag: u32,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) clip_id: u32,
    @location(2) @interpolate(flat) stroke_flag: u32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // SPEC-R-004 Stage 14: Apply affine transform
    // Transform is stored as [m00, m01, m02, m10, m11, m12]
    // transform_0 = [m00, m01, m02, m10]
    // transform_1 = [m11, m12]
    // Applied as: x' = m00*x + m01*y + m02, y' = m10*x + m11*y + m12
    let m00 = input.transform_0.x;
    let m01 = input.transform_0.y;
    let m02 = input.transform_0.z;
    let m10 = input.transform_0.w;
    let m11 = input.transform_1.x;
    let m12 = input.transform_1.y;

    let local_pos = input.position;

    // Apply affine transform: path-local → logical coordinates
    let logical_pos = vec2<f32>(
        m00 * local_pos.x + m01 * local_pos.y + m02,
        m10 * local_pos.x + m11 * local_pos.y + m12
    );

    // SPEC-HAL-085: Use view_proj matrix for Logical → NDC transform
    out.position = globals.view_proj * vec4<f32>(logical_pos, 0.0, 1.0);
    out.color = input.color;
    out.clip_id = input.clip_id;
    out.stroke_flag = input.stroke_flag;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // SPEC-R-004 Stage 13: Clip test before any output
    // Fragment position is in physical pixel coordinates (top-left origin)
    let frag_pos = input.position.xy;
    let clip = clips[input.clip_id];

    // Discard fragments outside the clip rectangle
    if (frag_pos.x < clip.min.x || frag_pos.x > clip.max.x ||
        frag_pos.y < clip.min.y || frag_pos.y > clip.max.y) {
        discard;
    }

    // Output with premultiplied alpha
    let c = input.color;
    return vec4<f32>(c.rgb * c.a, c.a);
}
