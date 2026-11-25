// SPEC-HAL-263: Image Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles (bit 0 = rounded_clip_enabled)
}

// SPEC-HAL-263 Section 6.1: Rounded Clip UBO (identical to rect.wgsl)
struct RoundedClip {
    rect_logical: vec4<f32>,   // x, y, w, h (logical)
    radii_logical: vec4<f32>,  // tl, tr, bl, br (logical)
    inner_min: vec2<f32>,      // safe zone start (local coords)
    inner_max: vec2<f32>,      // safe zone end (local coords)
    outer_min: vec2<f32>,      // AA-expanded rect start (local coords)
    outer_max: vec2<f32>,      // AA-expanded rect end (local coords)
    aa_radius_logical: f32,    // AA radius in logical pixels
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0)
var<uniform> globals: Globals;

// SPEC-HAL-263 Section 6.1: Image pipeline resources at group(1)
@group(1) @binding(0)
var image_tex: texture_2d<f32>;

@group(1) @binding(1)
var image_sampler: sampler;

@group(1) @binding(2)
var<uniform> rounded_clip: RoundedClip;

// SPEC-HAL-263: Vertex input for image quads
struct VertexInput {
    @location(0) position: vec2<f32>,  // Absolute Logical coords
    @location(1) uv: vec2<f32>,        // Texture coordinates
    @location(2) color: vec4<f32>,     // Tint color (straight linear RGBA)
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

// Helper: check if rounded clipping is enabled via flags bit 0
fn is_rounded_clip_enabled() -> bool {
    return (globals.flags & 1u) != 0u;
}

// Helper: get scale factor from view_proj (derived from matrix)
fn get_scale_factor() -> f32 {
    return globals.view_proj[0][0] * globals.surface_size.x / 2.0;
}

// SPEC-HAL-263: Compute rounded clip coverage (identical to rect.wgsl)
fn compute_rounded_clip_coverage(frag_position: vec4<f32>) -> f32 {
    if (!is_rounded_clip_enabled()) {
        return 1.0;
    }

    let scale_factor = get_scale_factor();
    let frag_physical = frag_position.xy;
    let frag_logical = frag_physical / scale_factor;
    let aa = rounded_clip.aa_radius_logical;
    let local = frag_logical - rounded_clip.rect_logical.xy;
    let rect_size = rounded_clip.rect_logical.zw;

    // Outer bounds reject (AA-extended rect)
    if (local.x < rounded_clip.outer_min.x || local.y < rounded_clip.outer_min.y ||
        local.x > rounded_clip.outer_max.x || local.y > rounded_clip.outer_max.y) {
        return 0.0;
    }

    // Fast-path: if all radii are 0, skip AA entirely
    let tl_r = rounded_clip.radii_logical.x;
    let tr_r = rounded_clip.radii_logical.y;
    let bl_r = rounded_clip.radii_logical.z;
    let br_r = rounded_clip.radii_logical.w;
    let has_rounding = (tl_r > 0.0) || (tr_r > 0.0) || (bl_r > 0.0) || (br_r > 0.0);

    if (!has_rounding) {
        return 1.0;
    }

    // Inner safe zone accept
    if (local.x >= rounded_clip.inner_min.x && local.x <= rounded_clip.inner_max.x &&
        local.y >= rounded_clip.inner_min.y && local.y <= rounded_clip.inner_max.y) {
        return 1.0;
    }

    var coverage = 1.0;

    // Top-left corner zone
    if (local.x < tl_r && local.y < tl_r) {
        if (tl_r > 0.0) {
            let center = vec2<f32>(tl_r, tl_r);
            let dist = distance(local, center);
            if (dist > tl_r + aa) { return 0.0; }
            // Match the inward straight-edge AA band at the tangent.
            coverage = min(coverage, 1.0 - smoothstep(tl_r - aa, tl_r, dist));
        } else {
            let edge_dist = max(-local.x, -local.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Top-right corner zone
    else if (local.x > rect_size.x - tr_r && local.y < tr_r) {
        if (tr_r > 0.0) {
            let center = vec2<f32>(rect_size.x - tr_r, tr_r);
            let dist = distance(local, center);
            if (dist > tr_r + aa) { return 0.0; }
            coverage = min(coverage, 1.0 - smoothstep(tr_r - aa, tr_r, dist));
        } else {
            let edge_dist = max(local.x - rect_size.x, -local.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Bottom-left corner zone
    else if (local.x < bl_r && local.y > rect_size.y - bl_r) {
        if (bl_r > 0.0) {
            let center = vec2<f32>(bl_r, rect_size.y - bl_r);
            let dist = distance(local, center);
            if (dist > bl_r + aa) { return 0.0; }
            coverage = min(coverage, 1.0 - smoothstep(bl_r - aa, bl_r, dist));
        } else {
            let edge_dist = max(-local.x, local.y - rect_size.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Bottom-right corner zone
    else if (local.x > rect_size.x - br_r && local.y > rect_size.y - br_r) {
        if (br_r > 0.0) {
            let center = vec2<f32>(rect_size.x - br_r, rect_size.y - br_r);
            let dist = distance(local, center);
            if (dist > br_r + aa) { return 0.0; }
            coverage = min(coverage, 1.0 - smoothstep(br_r - aa, br_r, dist));
        } else {
            let edge_dist = max(local.x - rect_size.x, local.y - rect_size.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Straight edge zones
    else {
        if (local.x < aa) {
            coverage = min(coverage, smoothstep(0.0, aa, local.x));
        }
        if (local.x > rect_size.x - aa) {
            coverage = min(coverage, smoothstep(0.0, aa, rect_size.x - local.x));
        }
        if (local.y < aa) {
            coverage = min(coverage, smoothstep(0.0, aa, local.y));
        }
        if (local.y > rect_size.y - aa) {
            coverage = min(coverage, smoothstep(0.0, aa, rect_size.y - local.y));
        }
    }

    return coverage;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(image_tex, image_sampler, input.uv);

    // Sampled texture is premultiplied-linear; tint is straight-linear.
    let source = vec4<f32>(
        tex_color.rgb * input.color.rgb * input.color.a,
        input.color.a * tex_color.a
    );

    // SPEC-HAL-263: Apply rounded clip coverage (identical model to rect pipeline)
    let coverage = compute_rounded_clip_coverage(input.position);
    if (coverage <= 0.0) { discard; }

    // Premultiplied alpha with coverage
    return source * coverage;
}
