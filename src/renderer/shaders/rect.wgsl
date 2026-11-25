// SPEC-HAL-085: Rect Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms (shared across ALL pipelines)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles (bit 0 = rounded_clip_enabled)
}

// SPEC-HAL-085 Section 7.2: Pipeline-specific resources at group(1)
struct RoundedClip {
    rect_logical: vec4<f32>,   // x, y, w, h (logical)
    radii_logical: vec4<f32>,  // tl, tr, bl, br (logical)
    // SPEC-R-020 Stage 9: CPU-precomputed corner zone bounds
    inner_min: vec2<f32>,      // safe zone start (local coords)
    inner_max: vec2<f32>,      // safe zone end (local coords)
    outer_min: vec2<f32>,      // AA-expanded rect start (local coords)
    outer_max: vec2<f32>,      // AA-expanded rect end (local coords)
    aa_radius_logical: f32,    // AA radius in logical pixels
    _pad0: f32,                // Alignment padding (avoid vec3 which has 16-byte alignment!)
    _pad1: f32,
    _pad2: f32,
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0) var<uniform> globals: Globals;
// SPEC-HAL-085 INV-SHADER-BIND-1: group(1)=pipeline-specific
@group(1) @binding(0) var<uniform> rounded_clip: RoundedClip;

// Rect pipeline is a dumb, batchable quad renderer - NO shape semantics.
// Shape (silhouette) must be owned elsewhere (clip/mask or path primitive).
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    // SPEC-HAL-085: Use view_proj matrix for Absolute Logical → NDC transform
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    return output;
}

// Helper: check if rounded clipping is enabled via flags bit 0
fn is_rounded_clip_enabled() -> bool {
    return (globals.flags & 1u) != 0u;
}

// Helper: get scale factor from view_proj (derived from matrix)
fn get_scale_factor() -> f32 {
    // Scale factor is encoded in view_proj[0][0] and surface_size
    // For 2D ortho: view_proj[0][0] = 2.0 * scale_factor / surface_size.x
    return globals.view_proj[0][0] * globals.surface_size.x / 2.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // All outputs use premultiplied alpha: (r*a, g*a, b*a, a)
    let c = input.color;

    // SPEC-R-020: Global toggle - when disabled, skip all clipping
    if (!is_rounded_clip_enabled()) {
        // No rounded clipping - output premultiplied color directly
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    let scale_factor = get_scale_factor();
    let frag_physical = input.position.xy;
    let frag_logical = frag_physical / scale_factor;
    let aa = rounded_clip.aa_radius_logical;
    let local = frag_logical - rounded_clip.rect_logical.xy;
    let rect_size = rounded_clip.rect_logical.zw;

    // Outer bounds reject (AA-extended rect) - valid performance optimization
    if (local.x < rounded_clip.outer_min.x || local.y < rounded_clip.outer_min.y ||
        local.x > rounded_clip.outer_max.x || local.y > rounded_clip.outer_max.y) {
        discard;
    }

    // Fast-path: if all radii are 0 (no rounding), skip AA entirely
    // This handles the "no clip" case and rectangular clips efficiently
    let tl_r = rounded_clip.radii_logical.x;
    let tr_r = rounded_clip.radii_logical.y;
    let bl_r = rounded_clip.radii_logical.z;
    let br_r = rounded_clip.radii_logical.w;
    let has_rounding = (tl_r > 0.0) || (tr_r > 0.0) || (bl_r > 0.0) || (br_r > 0.0);

    if (!has_rounding) {
        // No rounding - scissor handles clipping, output full coverage
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    // Inner safe zone accept - valid: proves coverage=1.0 (>= aa from all boundaries)
    if (local.x >= rounded_clip.inner_min.x && local.x <= rounded_clip.inner_max.x &&
        local.y >= rounded_clip.inner_min.y && local.y <= rounded_clip.inner_max.y) {
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    // SPEC-R-020 AA-CLIP-INV-01: Per-corner + straight edge AA (only when has_rounding)
    var coverage = 1.0;

    // Top-left corner zone
    if (local.x < tl_r && local.y < tl_r) {
        if (tl_r > 0.0) {
            // Rounded corner: circular distance
            let center = vec2<f32>(tl_r, tl_r);
            let dist = distance(local, center);
            if (dist > tl_r + aa) { discard; }
            // Match the inward straight-edge AA band at the tangent.
            coverage = min(coverage, 1.0 - smoothstep(tl_r - aa, tl_r, dist));
        } else {
            // Sharp corner (radius=0): use max of edge distances
            let edge_dist = max(-local.x, -local.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Top-right corner zone
    else if (local.x > rect_size.x - tr_r && local.y < tr_r) {
        if (tr_r > 0.0) {
            let center = vec2<f32>(rect_size.x - tr_r, tr_r);
            let dist = distance(local, center);
            if (dist > tr_r + aa) { discard; }
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
            if (dist > bl_r + aa) { discard; }
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
            if (dist > br_r + aa) { discard; }
            coverage = min(coverage, 1.0 - smoothstep(br_r - aa, br_r, dist));
        } else {
            let edge_dist = max(local.x - rect_size.x, local.y - rect_size.y);
            coverage = min(coverage, 1.0 - smoothstep(0.0, aa, edge_dist));
        }
    }
    // Straight edge zones (between corners)
    else {
        // Left edge
        if (local.x < aa) {
            coverage = min(coverage, smoothstep(0.0, aa, local.x));
        }
        // Right edge
        if (local.x > rect_size.x - aa) {
            coverage = min(coverage, smoothstep(0.0, aa, rect_size.x - local.x));
        }
        // Top edge
        if (local.y < aa) {
            coverage = min(coverage, smoothstep(0.0, aa, local.y));
        }
        // Bottom edge
        if (local.y > rect_size.y - aa) {
            coverage = min(coverage, smoothstep(0.0, aa, rect_size.y - local.y));
        }
    }

    if (coverage <= 0.0) { discard; }

    // Premultiplied alpha with coverage: (r*a*cov, g*a*cov, b*a*cov, a*cov)
    let final_alpha = c.a * coverage;
    return vec4<f32>(c.rgb * final_alpha, final_alpha);
}
