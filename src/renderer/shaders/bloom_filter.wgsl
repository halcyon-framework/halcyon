// HAL395 §9.5. Missing source ROI contributes zero without removing its weight.
struct BloomFilterUniform {
    target_bounds: vec4<u32>,
    input_roi: vec4<u32>,
    output_roi: vec4<u32>,
    kernel: vec4<u32>,
    weights: array<vec4<f32>, 49>,
}
@group(1) @binding(0) var<uniform> params: BloomFilterUniform;
@group(1) @binding(1) var input_texture: texture_2d<f32>;

@fragment fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(params.output_roi.xy) + vec2<i32>(floor(position.xy));
    let axis = select(vec2(1, 0), vec2(0, 1), params.kernel.x == 1u);
    let radius = i32(params.kernel.y);
    let origin = vec2<i32>(params.input_roi.xy);
    let extent = vec2<i32>(params.input_roi.zw);
    let target_size = vec2<i32>(params.target_bounds.zw);
    var total = 0.0;
    var sum = vec4(0.0);
    for (var tap = -radius; tap <= radius; tap += 1) {
        let p = pixel + axis * tap;
        if all(p >= vec2(0)) && all(p < target_size) {
            let k = u32(abs(tap));
            let weight = params.weights[k / 4u][k % 4u];
            total += weight;
            let local = p - origin;
            if all(local >= vec2(0)) && all(local < extent) {
                sum += weight * textureLoad(input_texture, local, 0);
            }
        }
    }
    return sum / total;
}
