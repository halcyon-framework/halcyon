// HAL395 §8.2 / §9.4. Evaluate the original coating lights before SDR capping.
struct BloomSourceUniform {
    rect: vec4<f32>,
    corners: vec4<f32>,
    ancestor: RoundedClip,
    light0: GlassOpticsLight,
    light1: GlassOpticsLight,
    shape: vec4<f32>,
    optics: vec4<f32>,
    tint: vec4<f32>,
    control: vec4<u32>,
}
@group(1) @binding(0) var<uniform> source: BloomSourceUniform;

@fragment fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let scale = surface_scale();
    let world = (vec2<f32>(source.control.yz) + floor(position.xy) + vec2(0.5)) / scale;
    let rect = source.rect;
    let q = world - (rect.xy + rect.zw * 0.5);
    var radius = source.corners.w;
    if q.x < 0.0 {
        radius = select(source.corners.z, source.corners.x, q.y < 0.0);
    } else if q.y < 0.0 {
        radius = source.corners.y;
    }
    let v = abs(q) - rect.zw * 0.5 + vec2(radius);
    let vp = max(v, vec2(0.0));
    let length_vp = length(vp);
    let d = length_vp + min(max(v.x, v.y), 0.0) - radius;
    let aa = max(fwidth(d), 0.5 / scale);
    let coverage = (1.0 - smoothstep(-aa, aa, d))
        * bloom_ancestor_coverage(source.ancestor, source.control.x, world);
    var gradient = vec2(0.0);
    if length_vp > 0.0 { gradient = sign(q) * vp / length_vp; }
    else if v.x >= v.y { gradient = vec2(sign(q.x), 0.0); }
    else { gradient = vec2(0.0, sign(q.y)); }
    let k = source.shape.y * glass_bevel(d, source.shape.x);
    var n = vec3(0.0, 0.0, 1.0);
    if dot(gradient, gradient) > 0.0 {
        n = vec3(k * gradient, sqrt(max(1.0 - k * k, 0.0)));
    }
    let uv = (world - rect.xy) / rect.zw;
    let a = glass_light_response(source.light0, uv, n, source.shape.z).x;
    let b = glass_light_response(source.light1, uv, n, source.shape.z).x;
    let fresnel = source.shape.w + (1.0 - source.shape.w) * pow(1.0 - n.z, 5.0);
    let gain = source.optics.x * source.optics.w * source.tint.a * fresnel;
    let radiance = gain * (a * source.light0.color.rgb + b * source.light1.color.rgb) * source.tint.rgb;
    let y = dot(radiance, vec3(0.2126, 0.7152, 0.0722));
    let u = clamp(y - 0.75, 0.0, 0.5);
    let excess = max(max(y - 1.0, 0.0), u * u);
    var extracted = vec3(0.0);
    if y > 0.0 { extracted = radiance * (excess / y); }
    return vec4(extracted * coverage, coverage);
}
