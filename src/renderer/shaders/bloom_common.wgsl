// HAL395 §9.4. Canonical Globals and original HAL386 ancestor mask.
struct Globals {
    view_proj: mat4x4<f32>,
    surface_size: vec2<f32>,
    time_seconds: f32,
    flags: u32,
}

struct RoundedClip {
    rect_logical: vec4<f32>,
    radii_logical: vec4<f32>,
    inner_min: vec2<f32>,
    inner_max: vec2<f32>,
    outer_min: vec2<f32>,
    outer_max: vec2<f32>,
    aa_radius_logical: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
}

@group(0) @binding(0) var<uniform> globals: Globals;

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let triangle = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(triangle[index], 0.0, 1.0);
}

fn surface_scale() -> f32 {
    return globals.view_proj[0][0] * globals.surface_size.x / 2.0;
}

fn bloom_ancestor_coverage(clip: RoundedClip, enabled: u32, world: vec2<f32>) -> f32 {
    if enabled == 0u { return 1.0; }
    let local = world - clip.rect_logical.xy;
    let size = clip.rect_logical.zw;
    let aa = clip.aa_radius_logical;
    if any(local < clip.outer_min) || any(local > clip.outer_max) { return 0.0; }
    if all(local >= clip.inner_min) && all(local <= clip.inner_max) { return 1.0; }
    let tl = clip.radii_logical.x;
    let tr = clip.radii_logical.y;
    let bl = clip.radii_logical.z;
    let br = clip.radii_logical.w;
    var radius = 0.0;
    var center = vec2(0.0);
    if local.x < tl && local.y < tl { radius = tl; center = vec2(tl); }
    else if local.x > size.x - tr && local.y < tr { radius = tr; center = vec2(size.x - tr, tr); }
    else if local.x < bl && local.y > size.y - bl { radius = bl; center = vec2(bl, size.y - bl); }
    else if local.x > size.x - br && local.y > size.y - br { radius = br; center = size - vec2(br); }
    if radius > 0.0 { return 1.0 - smoothstep(radius, radius + aa, distance(local, center)); }
    var coverage = 1.0;
    if local.x < aa { coverage = min(coverage, smoothstep(0.0, aa, local.x)); }
    if local.y < aa { coverage = min(coverage, smoothstep(0.0, aa, local.y)); }
    if local.x > size.x - aa { coverage = min(coverage, smoothstep(0.0, aa, size.x - local.x)); }
    if local.y > size.y - aa { coverage = min(coverage, smoothstep(0.0, aa, size.y - local.y)); }
    return coverage;
}
