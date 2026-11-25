// SPEC-R-022: SDF RoundedRect Pipeline
// Evaluates rounded rect distance field per-pixel.
// Handles fill, border, and fill+border in one shader.
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
    @location(1) fill_color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) rect_center: vec2<f32>,
    @location(4) rect_half_size: vec2<f32>,
    @location(5) radius: f32,
    @location(6) border_width: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) @interpolate(flat) fill_color: vec4<f32>,
    @location(2) @interpolate(flat) border_color: vec4<f32>,
    @location(3) @interpolate(flat) rect_center: vec2<f32>,
    @location(4) @interpolate(flat) rect_half_size: vec2<f32>,
    @location(5) @interpolate(flat) radius: f32,
    @location(6) @interpolate(flat) border_width: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_proj * vec4<f32>(input.position, 0.0, 1.0);
    output.world_pos = input.position;
    output.fill_color = input.fill_color;
    output.border_color = input.border_color;
    output.rect_center = input.rect_center;
    output.rect_half_size = input.rect_half_size;
    output.radius = input.radius;
    output.border_width = input.border_width;
    return output;
}

// Inigo Quilez rounded box SDF (same as shadow.wgsl)
fn sd_rounded_rect(p: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p - center) - half_size + vec2(radius);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // AA radius: half a physical pixel in logical coords
    let scale_factor = globals.view_proj[0][0] * globals.surface_size.x / 2.0;
    let aa = 0.5 / scale_factor;

    let sd = sd_rounded_rect(
        input.world_pos,
        input.rect_center,
        input.rect_half_size,
        input.radius,
    );

    var result = vec4<f32>(0.0);

    // Fill
    if (input.fill_color.a > 0.0) {
        let fill_cov = 1.0 - smoothstep(-aa, aa, sd);
        let fc = input.fill_color;
        result = vec4<f32>(fc.rgb * fill_cov, fc.a * fill_cov);
    }

    // Border (annular ring)
    let bw = input.border_width;
    if (bw > 0.0 && input.border_color.a > 0.0) {
        let border_sd = abs(sd + bw * 0.5) - bw * 0.5;
        let border_cov = 1.0 - smoothstep(-aa, aa, border_sd);
        let bc = input.border_color;
        let border = vec4<f32>(bc.rgb * border_cov, bc.a * border_cov);
        // Composite border over fill (premultiplied alpha)
        result = border + result * (1.0 - border.a);
    }

    if (result.a <= 0.0) { discard; }
    return result;
}
