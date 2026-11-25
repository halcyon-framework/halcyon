// SPEC-HAL-272: ColorPicker SV square shader.
// Renders a saturation/value square for a fixed hue.
// Vertex input: position in physical pixels (NDC transform via globals).
// Fragment: UV → HSV(hue, s=u, v=1-v) → RGB.

// ---- Group 0: rect globals ----
struct Globals {
    view_proj:    mat4x4<f32>,
    surface_size: vec2<f32>,
    time_seconds: f32,
    flags:        u32,
}
@group(0) @binding(0) var<uniform> globals: Globals;

// ---- Group 1: per-instance params (dynamic offset) ----
struct ColorPickerSvParams {
    rect_origin:   vec2<f32>,
    rect_size:     vec2<f32>,
    hue:           f32,
    corner_radius: f32,
    _pad:          vec2<f32>,
}
@group(1) @binding(0) var<uniform> params: ColorPickerSvParams;

// ---- Vertex ----
struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       world_pos:     vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let ndc_x =  2.0 * in.position.x / globals.surface_size.x - 1.0;
    let ndc_y = -2.0 * in.position.y / globals.surface_size.y + 1.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.world_pos = in.position;
    return out;
}

// ---- HSV → RGB ----
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let hp = h * 6.0;
    let x = c * (1.0 - abs(hp % 2.0 - 1.0));
    let m = v - c;
    var rgb: vec3<f32>;
    if hp < 1.0 {
        rgb = vec3<f32>(c, x, 0.0);
    } else if hp < 2.0 {
        rgb = vec3<f32>(x, c, 0.0);
    } else if hp < 3.0 {
        rgb = vec3<f32>(0.0, c, x);
    } else if hp < 4.0 {
        rgb = vec3<f32>(0.0, x, c);
    } else if hp < 5.0 {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }
    return rgb + vec3<f32>(m, m, m);
}

// ---- Fragment ----
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local = (in.world_pos - params.rect_origin) / params.rect_size;
    let s = clamp(local.x, 0.0, 1.0);
    let v = clamp(1.0 - local.y, 0.0, 1.0);
    let rgb = hsv_to_rgb(params.hue, s, v);

    // Rounded corner discard
    if params.corner_radius > 0.0 {
        let half = params.rect_size * 0.5;
        let center = params.rect_origin + half;
        let d = abs(in.world_pos - center) - half + vec2<f32>(params.corner_radius, params.corner_radius);
        let dist = length(max(d, vec2<f32>(0.0, 0.0))) - params.corner_radius;
        if dist > 0.0 {
            discard;
        }
    }

    return vec4<f32>(rgb, 1.0);
}
