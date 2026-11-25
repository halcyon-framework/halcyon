// SPEC-R-003: Analytic SDF Shape Pipeline
// Alpha model: PREMULTIPLIED

struct Globals {
    view_proj: mat4x4<f32>,
    surface_size: vec2<f32>,
    time_seconds: f32,
    flags: u32,
}

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params0: vec4<f32>,
    @location(4) params1: vec4<f32>,
    @location(5) params2: vec4<f32>,
    @location(6) params3: vec4<f32>,
    @location(7) contour_gains: vec2<f32>,
    @location(8) shape_type: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) params0: vec4<f32>,
    @location(3) @interpolate(flat) params1: vec4<f32>,
    @location(4) @interpolate(flat) params2: vec4<f32>,
    @location(5) @interpolate(flat) params3: vec4<f32>,
    @location(6) @interpolate(flat) contour_gains: vec2<f32>,
    @location(7) @interpolate(flat) shape_type: u32,
}

const SHAPE_CIRCLE: u32 = 0u;
const SHAPE_ELLIPSE: u32 = 1u;
const SHAPE_LINE: u32 = 2u;
const SHAPE_POLYLINE: u32 = 3u;
const SHAPE_LUMINOUS_CONTOUR: u32 = 4u;
const SHAPE_LOCAL_AMBIENT_GLOW: u32 = 5u;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.world_pos = input.local_pos;
    output.color = input.color;
    output.params0 = input.params0;
    output.params1 = input.params1;
    output.params2 = input.params2;
    output.params3 = input.params3;
    output.contour_gains = input.contour_gains;
    output.shape_type = input.shape_type;
    return output;
}

fn pixel_aa() -> f32 {
    let scale_factor = max(globals.view_proj[0][0] * globals.surface_size.x * 0.5, 1.0);
    return 0.5 / scale_factor;
}

fn sd_circle(p: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

fn sd_ellipse(p: vec2<f32>, center: vec2<f32>, radii: vec2<f32>) -> f32 {
    let safe_radii = max(radii, vec2<f32>(0.0001));
    let q = (p - center) / safe_radii;
    return (length(q) - 1.0) * min(safe_radii.x, safe_radii.y);
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, half_width: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 0.0001);
    let h = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * h) - half_width;
}

fn shape_distance(input: VertexOutput) -> f32 {
    let p = input.world_pos;
    let p0 = input.params0;
    let p1 = input.params1;

    if (input.shape_type == SHAPE_CIRCLE) {
        return sd_circle(p, p0.xy, p0.z);
    }

    if (input.shape_type == SHAPE_ELLIPSE) {
        return sd_ellipse(p, p0.xy, p0.zw);
    }

    if (
        input.shape_type == SHAPE_LINE ||
        input.shape_type == SHAPE_POLYLINE ||
        input.shape_type == SHAPE_LUMINOUS_CONTOUR
    ) {
        return sd_segment(p, p0.xy, p0.zw, p1.x);
    }

    return 1.0;
}

fn luminous_contour_owns_fragment(input: VertexOutput) -> bool {
    let start_normal = input.params2.xy;
    let end_normal = input.params2.zw;
    if (
        dot(start_normal, start_normal) > 0.0 &&
        dot(input.world_pos - input.params0.xy, start_normal) <= 0.0
    ) {
        return false;
    }
    if (
        dot(end_normal, end_normal) > 0.0 &&
        dot(input.world_pos - input.params0.zw, end_normal) > 0.0
    ) {
        return false;
    }
    return true;
}

fn luminous_contour_front_gain(input: VertexOutput) -> f32 {
    let segment = input.params0.zw - input.params0.xy;
    let denominator = max(dot(segment, segment), 0.0001);
    let segment_t = clamp(
        dot(input.world_pos - input.params0.xy, segment) / denominator,
        0.0,
        1.0,
    );
    let eased_t = smoothstep(0.0, 1.0, segment_t);
    return mix(input.contour_gains.x, input.contour_gains.y, eased_t);
}

// HAL-388: inactive lobes return before any absent-radius division.
fn local_glow_weight(p: vec2<f32>, ellipse: vec4<f32>, rotation: vec2<f32>, alpha: f32, intensity: f32) -> f32 {
    if (alpha <= 0.0 || intensity <= 0.0) { return 0.0; }
    let d = p - ellipse.xy;
    let rotated = vec2<f32>(d.x * rotation.x + d.y * rotation.y, -d.x * rotation.y + d.y * rotation.x);
    let q = length(rotated / ellipse.zw);
    let u = clamp(1.0 - q, 0.0, 1.0);
    return alpha * intensity * u * u * (3.0 - 2.0 * u);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if (input.shape_type == SHAPE_LOCAL_AMBIENT_GLOW) {
        let w0 = local_glow_weight(input.world_pos, input.params0, input.params3.xy, input.color.a, input.contour_gains.x);
        let w1 = local_glow_weight(input.world_pos, input.params1, input.params3.zw, input.params2.a, input.contour_gains.y);
        let sum = w0 + w1;
        if (sum == 0.0) { return vec4<f32>(0.0); }
        let alpha = 1.0 - (1.0 - w0) * (1.0 - w1);
        let color = (w0 * input.color.rgb + w1 * input.params2.rgb) / sum;
        return vec4<f32>(color * alpha, alpha);
    }
    if (
        input.shape_type == SHAPE_LUMINOUS_CONTOUR &&
        !luminous_contour_owns_fragment(input)
    ) {
        discard;
    }

    let sd = shape_distance(input);
    let aa = pixel_aa();
    let core = 1.0 - smoothstep(-aa, aa, sd);
    let outside = max(sd, 0.0);
    let zoned = (
        input.params3.x > 0.0 ||
        input.params3.y > 0.0 ||
        input.params3.z > 0.0 ||
        (input.shape_type == SHAPE_LUMINOUS_CONTOUR && input.params3.w > 0.0)
    );
    var front_gain = 0.0;
    if (input.shape_type == SHAPE_LUMINOUS_CONTOUR) {
        front_gain = luminous_contour_front_gain(input);
    }

    var core_luminance_lift = input.params3.x;
    var inner_aura_intensity = input.params3.z;
    let halo_radius = input.params1.y;
    var halo_intensity = input.params1.z;
    if (input.shape_type == SHAPE_LUMINOUS_CONTOUR) {
        if (zoned) {
            core_luminance_lift = min(0.90, core_luminance_lift + 0.20 * front_gain);
            inner_aura_intensity = min(1.0, inner_aura_intensity + 0.25 * front_gain);
        } else {
            halo_intensity = min(0.35, halo_intensity + 0.10 * front_gain);
        }
    }

    var outer = 0.0;
    if (halo_radius > 0.0 && halo_intensity > 0.0) {
        let t = clamp(1.0 - outside / halo_radius, 0.0, 1.0);
        let halo_edge = 1.0 - smoothstep(
            max(halo_radius - aa, 0.0),
            halo_radius + aa,
            outside,
        );
        outer = halo_intensity * pow(t, input.params1.w) * halo_edge;
    }

    if (!zoned) {
        let coverage = core + (1.0 - core) * outer;
        if (coverage <= 0.0 || input.color.a <= 0.0) {
            discard;
        }
        let alpha = input.color.a * coverage;
        return vec4<f32>(input.color.rgb * alpha, alpha);
    }

    let inner_aura_radius = input.params3.y;
    var inner = 0.0;
    if (inner_aura_radius > 0.0 && inner_aura_intensity > 0.0) {
        let t = clamp(1.0 - outside / inner_aura_radius, 0.0, 1.0);
        let aura_edge = 1.0 - smoothstep(
            max(inner_aura_radius - aa, 0.0),
            inner_aura_radius + aa,
            outside,
        );
        inner = inner_aura_intensity * pow(t, 2.0) * aura_edge;
    }
    let aura = max(inner, outer);
    var effective_source_alpha = input.color.a;
    if input.shape_type == SHAPE_LUMINOUS_CONTOUR && input.params3.w > 0.0 {
        effective_source_alpha = input.color.a + (1.0 - input.color.a) * input.params3.w * front_gain;
    }
    let core_alpha = effective_source_alpha * core;
    let aura_alpha = effective_source_alpha * (1.0 - core) * aura;
    let alpha = core_alpha + aura_alpha;
    if (alpha <= 0.0) {
        discard;
    }

    let lifted_core_rgb = mix(
        input.color.rgb,
        vec3<f32>(1.0),
        core_luminance_lift,
    );
    let rgb = lifted_core_rgb * core_alpha + input.color.rgb * aura_alpha;
    return vec4<f32>(rgb, alpha);
}
