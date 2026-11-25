// JC-directed opt-in V2: soft sculpted rest and immediate concave press; V1 is unchanged.
struct Material {
    half_size: vec2<f32>,
    center: vec2<f32>,
    pixels_per_logical_unit: f32,
    radius: f32,
    hover_progress: f32,
    press_progress: f32,
    base_linear: vec4<f32>,
    light_linear: vec4<f32>,
    shadow: vec4<f32>, // logical offset.xy, sigma, opacity
}

@group(0) @binding(0) var<uniform> material: Material;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

fn rounded_distance(p: vec2<f32>) -> f32 {
    let q = abs(p) - material.half_size + vec2<f32>(material.radius);
    return length(max(q, vec2<f32>(0.0)))
        + min(max(q.x, q.y), 0.0) - material.radius;
}

fn outward_normal(p: vec2<f32>) -> vec2<f32> {
    let step = 0.25;
    let gradient = vec2<f32>(
        rounded_distance(p + vec2<f32>(step, 0.0))
            - rounded_distance(p - vec2<f32>(step, 0.0)),
        rounded_distance(p + vec2<f32>(0.0, step))
            - rounded_distance(p - vec2<f32>(0.0, step)),
    ) / (2.0 * step);
    return gradient / max(length(gradient), 0.000001);
}

@fragment
fn fs_shadow(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pixel.xy / material.pixels_per_logical_unit - material.center;
    let d = rounded_distance(p - material.shadow.xy);
    let outside = max(d, 0.0);
    // Experimental tail: unchanged through 2.5 sigma, zero from 3 sigma.
    let tail = 1.0 - smoothstep(2.5 * material.shadow.z, 3.0 * material.shadow.z, outside);
    let alpha = material.shadow.w * mix(1.0, 0.30, material.press_progress)
        * exp(-0.5 * outside * outside / (material.shadow.z * material.shadow.z)) * tail;
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}

@fragment
fn fs_face(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pixel.xy / material.pixels_per_logical_unit - material.center;
    let d = rounded_distance(p);
    let aa = max(fwidth(d), 0.5 / material.pixels_per_logical_unit);
    let coverage = 1.0 - smoothstep(-aa, aa, d);
    let t = clamp((-d) / 3.5, 0.0, 1.0);
    let envelope = 1.0 - smoothstep(0.0, 1.0, t);
    let normal = outward_normal(p);
    let upper = 1.0 - smoothstep(-material.half_size.y, 0.0, p.y);
    let lower = smoothstep(0.0, material.half_size.y, p.y);
    // Illuminate the upper-left outward-facing rim.
    let bright_rim = envelope
        * max(dot(normal, normalize(vec2<f32>(-0.25, -0.97))), 0.0);
    let dark_rim = envelope
        * max(dot(normal, normalize(vec2<f32>(0.20, 0.98))), 0.0);
    // Broad, continuous relief belongs to the same face; no stroked frame.
    let raised = material.light_linear.x * upper - material.light_linear.y * lower
        + material.light_linear.z * bright_rim - material.light_linear.w * dark_rim;
    let concave = -1.25 * material.light_linear.x * upper
        + 0.70 * material.light_linear.y * lower
        - material.light_linear.w * bright_rim + material.light_linear.z * dark_rim
        - (0.06 * material.base_linear.r + 0.004);
    let lighting = mix(raised, concave, material.press_progress);
    let face = clamp(material.base_linear.rgb + vec3<f32>(lighting),
        vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(face * coverage, coverage);
}
