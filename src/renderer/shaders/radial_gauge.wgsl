// SPEC-HAL-278: Radial gauge shader — circular arc/ring display
//
// Draws a circular arc on a single quad. Supports configurable arc sweep,
// origin-based fill, needle mode, and up to 4 colored zones.
// Anti-aliased ring edges via smoothstep.

struct Globals {
    view_proj:    mat4x4<f32>,
    surface_size: vec2<f32>,
    time_seconds: f32,
    flags:        u32,
};

struct RadialGaugeParams {
    track_color:  vec4<f32>,
    fill_color:   vec4<f32>,
    rect_origin:  vec2<f32>,
    rect_size:    vec2<f32>,
    fill:         f32,
    thickness:    f32,
    start_angle:  f32,
    sweep_angle:  f32,
    origin_fill:  f32,
    zone_count:   u32,
    mode:         u32,
    fill_style:   u32,
    zone0_range:  vec4<f32>,  // xy = start/end, zw unused
    zone0_color:  vec4<f32>,
    zone1_range:  vec4<f32>,
    zone1_color:  vec4<f32>,
    zone2_range:  vec4<f32>,
    zone2_color:  vec4<f32>,
    zone3_range:  vec4<f32>,
    zone3_color:  vec4<f32>,
    major_divisions: u32,
    minor_per_major: u32,
    tick_length_major: f32,
    tick_length_minor: f32,
    tick_color: vec4<f32>,
    fill_color_negative: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> params: RadialGaugeParams;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       uv:           vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = params.rect_origin + in.position * params.rect_size;
    out.clip_position = globals.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.uv = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let centered = (in.uv - vec2<f32>(0.5)) * 2.0;
    let dist = length(centered);

    // Ring mask
    let outer = 1.0;
    let inner = outer - params.thickness * 2.0;
    let aa = fwidth(dist) * 0.7;
    let ring_aa = smoothstep(inner - aa, inner + aa, dist)
                * (1.0 - smoothstep(outer - aa, outer + aa, dist));

    // Angle: top-center = 0, clockwise, normalized [0, 1]
    let angle = atan2(-centered.x, centered.y);
    let norm_angle = (angle + 3.14159265) / (2.0 * 3.14159265);

    // Compute arc-local position [0, 1] within the arc sweep.
    // The arc runs from start_angle to start_angle + sweep_angle
    // (both normalized [0,1] where 1 = full circle).
    // We need to handle wrap-around when the arc crosses the 0/1 boundary.
    let raw_delta = norm_angle - params.start_angle;
    let delta = select(raw_delta, raw_delta + 1.0, raw_delta < 0.0);
    let arc_pos = delta / params.sweep_angle;

    // Clamped fwidth: prevents ghost artifacts at arc wrap boundary
    // where fwidth() explodes due to delta discontinuity.
    let safe_fwidth = clamp(fwidth(arc_pos), 0.0005, 0.015);

    if (ring_aa < 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Outside arc → transparent (with AA at arc boundaries)
    let arc_aa = safe_fwidth * 2.0;
    let arc_mask = smoothstep(-arc_aa, arc_aa, arc_pos)
                 * (1.0 - smoothstep(1.0 - arc_aa, 1.0 + arc_aa, arc_pos));
    if (arc_mask < 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Zone color lookup: check each zone
    var base_color = params.track_color;
    if (params.zone_count > 0u && arc_pos >= params.zone0_range.x && arc_pos <= params.zone0_range.y) {
        base_color = params.zone0_color;
    }
    if (params.zone_count > 1u && arc_pos >= params.zone1_range.x && arc_pos <= params.zone1_range.y) {
        base_color = params.zone1_color;
    }
    if (params.zone_count > 2u && arc_pos >= params.zone2_range.x && arc_pos <= params.zone2_range.y) {
        base_color = params.zone2_color;
    }
    if (params.zone_count > 3u && arc_pos >= params.zone3_range.x && arc_pos <= params.zone3_range.y) {
        base_color = params.zone3_color;
    }

    // Fill logic
    var color = base_color;
    if ((params.mode & 1u) == 0u) {
        let fill_min = min(params.origin_fill, params.fill);
        let fill_max = max(params.origin_fill, params.fill);
        let is_filled = step(fill_min, arc_pos) * (1.0 - step(fill_max, arc_pos));

        if (params.fill_style == 0u) {
            // Solid: filled = fill_color, unfilled = track
            let fill_c = select(params.fill_color_negative, params.fill_color, params.fill >= params.origin_fill);
            color = mix(params.track_color, fill_c, is_filled);

        } else if (params.fill_style == 1u) {
            // Zone-colored: filled = zone colors, unfilled = track
            color = mix(params.track_color, base_color, is_filled);

        } else if (params.fill_style == 2u) {
            // Value-lerp: filled = gradient from fill_color to zone, unfilled = track
            let lerp_color = mix(params.fill_color, base_color, arc_pos);
            color = mix(params.track_color, lerp_color, is_filled);

        } else if (params.fill_style == 3u) {
            // Intensity ramp: fill_color ramps from 0 at origin to full at value
            // Unfilled = track. Filled = fill_color at ramped opacity over track.
            let fill_c = select(params.fill_color_negative, params.fill_color, params.fill >= params.origin_fill);
            let fill_span = fill_max - fill_min;
            var intensity = 0.0;
            if (fill_span > 0.001) {
                let dist_from_origin = abs(arc_pos - params.origin_fill) / fill_span;
                intensity = 0.3 + 0.7 * clamp(dist_from_origin, 0.0, 1.0);
            }
            let ramped = mix(params.track_color, fill_c, intensity);
            color = mix(params.track_color, ramped, is_filled);

        } else if (params.fill_style == 4u) {
            // Reverse intensity ramp: full at origin, fading toward value
            let fill_span = fill_max - fill_min;
            var intensity = 1.0;
            if (fill_span > 0.001) {
                let dist_from_origin = abs(arc_pos - params.origin_fill) / fill_span;
                intensity = 0.3 + 0.7 * (1.0 - clamp(dist_from_origin, 0.0, 1.0));
            }
            let ramped = mix(params.track_color, params.fill_color, intensity);
            color = mix(params.track_color, ramped, is_filled);
        }

    } else if ((params.mode & 1u) == 1u) {
        // Needle mode: zones ALWAYS visible (they are the scale).
        // Needle is a thin wedge overlaid on zones.
        let needle_half = 0.0027;
        let needle_dist = abs(arc_pos - params.fill);
        let needle_aa = 1.0 - smoothstep(needle_half - 0.002, needle_half, needle_dist);
        color = mix(base_color, params.fill_color, needle_aa);
    }

    // Tick marks
    if (params.major_divisions > 0u) {
        let major_step = 1.0 / f32(params.major_divisions);
        let minor_step = major_step / f32(params.minor_per_major + 1u);

        let nearest_minor = round(arc_pos / minor_step) * minor_step;
        let nearest_major = round(arc_pos / major_step) * major_step;

        let tick_aa = safe_fwidth * 0.8;
        let tick_half_width = safe_fwidth * 1.0;

        let major_dist = abs(arc_pos - nearest_major);
        let is_major = smoothstep(tick_half_width + tick_aa, tick_half_width - tick_aa, major_dist);

        let minor_dist = abs(arc_pos - nearest_minor);
        let is_minor = smoothstep(tick_half_width + tick_aa, tick_half_width - tick_aa, minor_dist);

        let tick_outer = 1.0;
        let major_inner = tick_outer - params.tick_length_major * 2.0;
        let minor_inner = tick_outer - params.tick_length_minor * 2.0;

        let in_major_radius = step(major_inner, dist) * step(dist, tick_outer);
        let in_minor_radius = step(minor_inner, dist) * step(dist, tick_outer);

        let tick_alpha = max(is_major * in_major_radius, is_minor * in_minor_radius);

        if (arc_pos > tick_half_width && arc_pos < 1.0 - tick_half_width) {
            if (tick_alpha > 0.01) {
                color = mix(color, params.tick_color, vec4<f32>(tick_alpha));
            }
        }
    }

    // Value pointer: thin radial line at current value
    if ((params.mode & 2u) != 0u) {
        let pointer_pos = params.fill;
        let pointer_half = safe_fwidth * 1.5;
        let pointer_dist = abs(arc_pos - pointer_pos);
        let pointer_aa = 1.0 - smoothstep(pointer_half - safe_fwidth * 0.5, pointer_half, pointer_dist);

        if (pointer_aa > 0.01) {
            let pointer_color = params.fill_color;
            color = mix(color, pointer_color, pointer_aa);
        }
    }

    return vec4<f32>(color.rgb, color.a * ring_aa * arc_mask);
}
