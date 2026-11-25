struct Material {
    half_size: vec2<f32>,
    center: vec2<f32>,
    scale: f32,
    radius: f32,
    sheen: f32,
    rim: f32,
    dim: f32,
    valley: f32,
    padding: vec2<f32>,
    stops: array<vec4<f32>, 4>,
    highlight: vec4<f32>,
}

@group(0) @binding(0) var<uniform> material: Material;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}

fn gradient(y: f32) -> vec3<f32> {
    var lower = material.stops[0];
    for (var index = 1u; index < 4u; index += 1u) {
        let upper = material.stops[index];
        if (y <= upper.a || index == 3u) {
            return mix(lower.rgb, upper.rgb, clamp((y - lower.a) / (upper.a - lower.a), 0.0, 1.0));
        }
        lower = upper;
    }
    return lower.rgb;
}

@fragment
fn fs_face(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = pixel.xy / material.scale - material.center;
    let q = abs(p) - material.half_size + vec2<f32>(material.radius);
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - material.radius;
    let aa = max(0.5 * fwidth(distance), 0.5 / material.scale);
    let coverage = 1.0 - smoothstep(-aa, aa, distance);
    let inner = 1.0 - smoothstep(-aa, aa, distance + 0.95);
    let rim = clamp((coverage - inner) / max(coverage, 0.000001), 0.0, 1.0);

    let uv = clamp((p + material.half_size) / (2.0 * material.half_size), vec2<f32>(0.0), vec2<f32>(1.0));
    let sheen = 1.0 - smoothstep(0.0, 1.0, length((uv - vec2<f32>(0.19, 0.04)) * vec2<f32>(2.55, 3.8)));
    let valley = smoothstep(0.46, 0.87, uv.y) * (1.0 - 0.25 * smoothstep(0.87, 1.0, uv.y));
    let body = gradient(uv.y) * (1.0 - material.dim - material.valley * valley);
    let glint = mix(body, material.highlight.rgb, material.sheen * sheen);
    let rim_light = 0.55 + 0.45 * (1.0 - uv.x) * (1.0 - uv.y);
    let face = mix(glint, material.highlight.rgb, material.rim * rim * rim_light);
    // Storage attachment transfer, when present, is the only encoding step.
    return vec4<f32>(face * coverage, coverage);
}
