// HAL085A / HAL394 §8: 1360-byte record, unchanged 1344-byte prefix.
struct Globals {view_proj:mat4x4<f32>,surface_size:vec2<f32>,time_seconds:f32,flags:u32}
struct RoundedClip {
    rect_logical:vec4<f32>,radii_logical:vec4<f32>,
    inner_min:vec2<f32>,inner_max:vec2<f32>,outer_min:vec2<f32>,outer_max:vec2<f32>,
    aa_radius_logical:f32,pad0:f32,pad1:f32,pad2:f32,
}
struct FrostedGlassParams {
    rect:vec4<f32>,corners:vec4<f32>,tint:vec4<f32>,optics:vec4<f32>,
    target_size:vec4<u32>,visible:vec4<u32>,snapshot:vec4<u32>,control:vec4<u32>,
    grain:vec4<f32>,grain_seed:vec4<u32>,ancestor:RoundedClip,reserved:vec4<u32>,
    weights:array<vec4<f32>,49>,
    filtered_domain:vec4<u32>,surface_controls:vec4<f32>,extensions:vec4<u32>,
    glass_optics:GlassSurfaceOpticsParams,emission:SurfaceEmissionParams,
    film:vec4<f32>,
}
@group(0) @binding(0) var<uniform> globals:Globals;
@group(1) @binding(0) var<uniform> material:FrostedGlassParams;
@group(1) @binding(1) var source:texture_2d<f32>;
@group(1) @binding(2) var second:texture_2d<f32>;
@vertex fn vs_main(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32> {
    let positions=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
    return vec4(positions[index],0.0,1.0);
}
fn weight(k:i32)->f32 {let n=u32(abs(k));return material.weights[n/4u][n%4u];}

@fragment fn fs_main(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let p=vec2<i32>(position.xy);
    let size=vec2<i32>(material.snapshot.zw);
    var color=vec4(0.0);var total=0.0;
    let r=i32(material.control.x);
    for(var k=-r;k<=r;k++) {
        let q=p+vec2(k,0);
        if q.x>=0 && q.x<size.x {let w=weight(k);color+=textureLoad(source,q,0)*w;total+=w;}
    }
    return color/total;
}
