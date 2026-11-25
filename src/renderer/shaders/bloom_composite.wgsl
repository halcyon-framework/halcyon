struct BloomCompositeUniform {
    output_roi: vec4<u32>,
    target_bounds: vec4<u32>,
    ancestor: RoundedClip,
    response: vec4<f32>,
    control: vec4<u32>,
}
@group(1) @binding(0) var<uniform> composite: BloomCompositeUniform;
@group(1) @binding(1) var filtered: texture_2d<f32>;

@fragment fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // This ROI viewport is offset within A, so position already includes B's origin.
    let pixel = vec2<i32>(floor(position.xy));
    let local = pixel - vec2<i32>(composite.output_roi.xy);
    let rgb = textureLoad(filtered, local, 0).rgb;
    let world = (vec2<f32>(pixel) + vec2(0.5)) / surface_scale();
    let mask = bloom_ancestor_coverage(composite.ancestor, composite.control.x, world);
    return vec4(mask * composite.response.x * rgb / (vec3(1.0) + rgb), 1.0);
}
