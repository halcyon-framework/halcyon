// SPEC-R-012-v2: Gradient Pipeline Shader
// Alpha model: PREMULTIPLIED - all outputs are (r*a, g*a, b*a, a)
// Blend state: One / OneMinusSrcAlpha (premultiplied alpha blending)
//
// Supports two gradient types via gradient_params.kind:
//   0 = Linear: color interpolation along a direction vector
//   1 = Radial: color interpolation from center outward
//
// Color stops arrive linear and are premultiplied once on CPU.
// Interpolation happens in linear color space for correct results.

// SPEC-HAL-085 Section 6.1: Canonical Global Uniforms (shared across ALL pipelines)
struct Globals {
    view_proj: mat4x4<f32>,     // Absolute Logical → NDC
    surface_size: vec2<f32>,    // physical pixels
    time_seconds: f32,          // SPEC-HAL-041
    flags: u32,                 // global debug toggles
}

// SPEC-R-012-v2: Color stop (linear premultiplied, padded to 32 bytes)
struct GradientStop {
    color: vec4<f32>,   // linear-space, premultiplied RGBA
    offset: f32,        // [0..1]
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

// SPEC-HAL-085A: the existing clip projection shares the gradient parameter slot.
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

// SPEC-R-012-v2: Gradient parameters (per-batch uniform)
struct GradientParams {
    // Rect bounds (logical coordinates)
    rect_origin: vec2<f32>,
    rect_size: vec2<f32>,
    // Gradient start/center (normalized 0..1)
    p0: vec2<f32>,
    // Gradient end/edge (normalized 0..1)
    p1: vec2<f32>,
    // Corner radius for alpha masking (logical pixels)
    corner_radius: f32,
    // Gradient kind: 0 = linear, 1 = radial
    kind: u32,
    // Number of active stops (2..8)
    stop_count: u32,
    clip_enabled: u32,
    // Color stops array (max 8)
    stops: array<GradientStop, 8>,
    clip: RoundedClip,
}

// SPEC-HAL-085 INV-SHADER-BIND-0: group(0)=Globals binding(0) ONLY
@group(0) @binding(0) var<uniform> globals: Globals;
// SPEC-R-012-v2: group(1)=gradient parameters
@group(1) @binding(0) var<uniform> gradient_params: GradientParams;

// Vertex input: just position (all gradient computation in fragment shader)
struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.world_pos = input.position;
    return output;
}

// Sample gradient color at parameter t [0..1]
fn sample_gradient(t: f32) -> vec4<f32> {
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
    var factor: f32;
    if range > 0.0 {
        factor = clamp((tc - lower.offset) / range, 0.0, 1.0);
    } else {
        factor = 0.0;
    }

    // Linear interpolation in linear color space (colors already linear + premultiplied)
    return mix(lower.color, upper.color, factor);
}

// Rounded rect SDF for corner masking
fn sd_rounded_rect(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

// Same coverage law and AA owner as the Rect/Image clip projection.
fn rounded_clip_coverage(frag_position: vec4<f32>) -> f32 {
    if gradient_params.clip_enabled == 0u || (globals.flags & 1u) == 0u {
        return 1.0;
    }
    let clip = gradient_params.clip;
    let scale = globals.view_proj[0][0] * globals.surface_size.x / 2.0;
    let local = frag_position.xy / scale - clip.rect_logical.xy;
    let size = clip.rect_logical.zw;
    let aa = clip.aa_radius_logical;
    if any(local < clip.outer_min) || any(local > clip.outer_max) {
        return 0.0;
    }
    let tl = clip.radii_logical.x;
    let tr = clip.radii_logical.y;
    let bl = clip.radii_logical.z;
    let br = clip.radii_logical.w;
    if !any(clip.radii_logical > vec4(0.0)) {
        return 1.0;
    }
    if all(local >= clip.inner_min) && all(local <= clip.inner_max) {
        return 1.0;
    }
    var radius = 0.0;
    var center = vec2(0.0);
    if local.x < tl && local.y < tl {
        radius = tl;
        center = vec2(tl);
    } else if local.x > size.x - tr && local.y < tr {
        radius = tr;
        center = vec2(size.x - tr, tr);
    } else if local.x < bl && local.y > size.y - bl {
        radius = bl;
        center = vec2(bl, size.y - bl);
    } else if local.x > size.x - br && local.y > size.y - br {
        radius = br;
        center = size - vec2(br);
    }
    if radius > 0.0 {
        // Same inward AA band as the adjoining straight edge: no tangent seam.
        return 1.0 - smoothstep(radius - aa, radius, distance(local, center));
    }
    var coverage = 1.0;
    if local.x < aa { coverage = min(coverage, smoothstep(0.0, aa, local.x)); }
    if local.y < aa { coverage = min(coverage, smoothstep(0.0, aa, local.y)); }
    if local.x > size.x - aa { coverage = min(coverage, smoothstep(0.0, aa, size.x - local.x)); }
    if local.y > size.y - aa { coverage = min(coverage, smoothstep(0.0, aa, size.y - local.y)); }
    return coverage;
}

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
        let dist = sd_rounded_rect(input.world_pos, center, half_size, r);
        let aa = fwidth(dist);
        let mask = 1.0 - smoothstep(-aa, aa, dist);
        color = color * mask;
    }

    // Anti-banding dither: decorrelated screen-pixel noise bounded in display space.
    // Stops and blending stay linear-premultiplied; the local inverse transfer
    // slope avoids amplifying an encoded half-step into visible near-black grain.
    let pixel = vec2<u32>(u32(input.position.x), u32(input.position.y));
    var h = pixel.x * 0x9e3779b9u + pixel.y * 0x85ebca6bu;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    let centered_noise = f32(h & 65535u) / 65535.0 - 0.5;
    let alpha = color.a;
    let straight_rgb = clamp(color.rgb / max(alpha, 1e-6), vec3(0.0), vec3(1.0));
    let inverse_srgb_slope_approx = clamp(
        vec3(0.0774) + 2.19 * straight_rgb, vec3(0.0774), vec3(2.2674));
    let delta_linear = vec3(centered_noise / 255.0) * inverse_srgb_slope_approx * alpha;
    color = vec4(clamp(color.rgb + delta_linear, vec3(0.0), vec3(alpha)), alpha);

    return color * rounded_clip_coverage(input.position);
}
