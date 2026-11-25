// Opt-in reference-E V3. V1/V2 remain separate, reproducible material sources.
struct Material {
    half_size: vec2<f32>,
    center: vec2<f32>,
    pixels_per_logical_unit: f32,
    radius: f32,
    hover_progress: f32,
    press_progress: f32,
    base_linear: vec4<f32>,
    light_linear: vec4<f32>,
    shadow: vec4<f32>,
}

@group(0) @binding(0) var<uniform> material: Material;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
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

fn srgb_to_linear(value: f32) -> f32 {
    return select(pow((value + 0.055) / 1.055, 2.4), value / 12.92, value <= 0.04045);
}

fn linear_to_srgb(value: f32) -> f32 {
    return select(1.055 * pow(value, 1.0 / 2.4) - 0.055, 12.92 * value, value <= 0.0031308);
}

fn grain_hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn grain_noise(uv: vec2<f32>) -> f32 {
    let cell = floor(uv);
    let f = fract(uv);
    let blend = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(
        mix(grain_hash(cell), grain_hash(cell + vec2<f32>(1.0, 0.0)), blend.x),
        mix(grain_hash(cell + vec2<f32>(0.0, 1.0)),
            grain_hash(cell + vec2<f32>(1.0, 1.0)), blend.x), blend.y,
    ) - 0.5;
}

@fragment
fn fs_shadow(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pixel.xy / material.pixels_per_logical_unit - material.center;
    let shifted = p - material.shadow.xy;
    let outside = max(rounded_distance(shifted), 0.0);
    let tail = 1.0 - smoothstep(2.5 * material.shadow.z, 3.0 * material.shadow.z, outside);
    // Direction changes opacity, never the admitted finite shadow support.
    let direction = 0.75 + 0.25
        * max(dot(outward_normal(shifted), normalize(vec2<f32>(0.65, 0.76))), 0.0);
    let alpha = material.shadow.w * mix(1.0, 0.45, material.press_progress) * direction
        * exp(-0.5 * outside * outside / (material.shadow.z * material.shadow.z)) * tail;
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}

@fragment
fn fs_face(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pixel.xy / material.pixels_per_logical_unit - material.center;
    let d = rounded_distance(p);
    let aa = max(fwidth(d), 0.5 / material.pixels_per_logical_unit);
    let coverage = 1.0 - smoothstep(-aa, aa, d);
    let normal = outward_normal(p);
    let upper = 1.0 - smoothstep(-material.half_size.y, 0.0, p.y);
    let lower = smoothstep(0.0, material.half_size.y, p.y);
    let relief = 1.0 - smoothstep(0.65, 2.0, max(-d, 0.0));
    let bright_rim = relief * max(dot(normal, normalize(vec2<f32>(-0.25, -0.97))), 0.0);
    let dark_rim = relief * max(dot(normal, normalize(vec2<f32>(0.20, 0.98))), 0.0);
    let raised = material.light_linear.x * upper - material.light_linear.y * lower
        + material.light_linear.z * bright_rim - material.light_linear.w * dark_rim;
    let concave = -1.25 * material.light_linear.x * upper
        + 0.70 * material.light_linear.y * lower
        - material.light_linear.w * bright_rim + material.light_linear.z * dark_rim
        - (0.06 * material.base_linear.r + 0.004);

    // Material-local UVs are independent of screen location and frame/time.
    // Attenuate the fine octave by its actual pixel footprint at both scales.
    let material_uv = (p + material.half_size) / (2.0 * material.half_size);
    let material_position = material_uv * (2.0 * material.half_size);
    let fine_uv = material_position * vec2<f32>(0.42, 1.7);
    let footprint = max(fwidth(fine_uv.x), fwidth(fine_uv.y));
    let fine_weight = 1.0 - smoothstep(0.5, 1.25, footprint);
    let grain = 2.0 * (0.80 * grain_noise(material_position * vec2<f32>(0.20, 0.65))
        + 0.20 * fine_weight * grain_noise(fine_uv));
    let base_srgb = linear_to_srgb(material.base_linear.r);
    let grain_linear = srgb_to_linear(clamp(base_srgb + grain * (1.35 / 255.0), 0.0, 1.0))
        - material.base_linear.r;
    let lighting = mix(raised, concave, material.press_progress) + grain_linear;
    let bulk = clamp(material.base_linear.rgb + vec3<f32>(lighting), vec3<f32>(0.0), vec3<f32>(1.0));

    // A thin material perimeter inside the SAME SDF face; no FocusCue/stroke node.
    let inner = 1.0 - smoothstep(-aa, aa, d + mix(0.65, 0.75, material.press_progress));
    let perimeter = clamp((coverage - inner) / max(coverage, 0.000001), 0.0, 1.0);
    let edge_srgb = select(134.0 / 255.0, 32.0 / 255.0, base_srgb < 0.5);
    let edge = vec3<f32>(srgb_to_linear(edge_srgb));
    let face = mix(bulk, edge, perimeter);
    return vec4<f32>(face * coverage, coverage);
}
