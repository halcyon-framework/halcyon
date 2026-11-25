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

fn ancestor_coverage(world:vec2<f32>)->f32 {
    if material.control.y==0u {return 1.0;}
    let c=material.ancestor;let p=world-c.rect_logical.xy;let size=c.rect_logical.zw;
    let aa=c.aa_radius_logical;
    if any(p<c.outer_min) || any(p>c.outer_max) {return 0.0;}
    if all(p>=c.inner_min) && all(p<=c.inner_max) {return 1.0;}
    let tl=c.radii_logical.x;let tr=c.radii_logical.y;let bl=c.radii_logical.z;let br=c.radii_logical.w;
    var radius=0.0;var center=vec2(0.0);
    if p.x<tl && p.y<tl {radius=tl;center=vec2(tl);}
    else if p.x>size.x-tr && p.y<tr {radius=tr;center=vec2(size.x-tr,tr);}
    else if p.x<bl && p.y>size.y-bl {radius=bl;center=vec2(bl,size.y-bl);}
    else if p.x>size.x-br && p.y>size.y-br {radius=br;center=size-vec2(br);}
    if radius>0.0 {return 1.0-smoothstep(radius,radius+aa,distance(p,center));}
    var coverage=1.0;
    if p.x<aa {coverage=min(coverage,smoothstep(0.0,aa,p.x));}
    if p.y<aa {coverage=min(coverage,smoothstep(0.0,aa,p.y));}
    if p.x>size.x-aa {coverage=min(coverage,smoothstep(0.0,aa,size.x-p.x));}
    if p.y>size.y-aa {coverage=min(coverage,smoothstep(0.0,aa,size.y-p.y));}
    return coverage;
}
// Only actual target edges repeat. Q padding, not an internal clamp, protects ROI taps.
fn filtered_texel(global:vec2<i32>)->vec4<f32> {
    let target_max=vec2<i32>(material.target_size.xy)-vec2(1);
    let bounded=clamp(global,vec2(0),target_max);
    return textureLoad(second,bounded-vec2<i32>(material.filtered_domain.xy),0);
}
fn displaced_sample(physical:vec2<f32>)->vec4<f32> {
    let q=clamp(physical,vec2(0.5),vec2<f32>(material.target_size.xy)-vec2(0.5));
    let x=q-vec2(0.5);
    let low=vec2<i32>(floor(x));
    let f=fract(x);
    let a=(1.0-f.x)*filtered_texel(low)+f.x*filtered_texel(low+vec2(1,0));
    let b=(1.0-f.x)*filtered_texel(low+vec2(0,1))+f.x*filtered_texel(low+vec2(1,1));
    return (1.0-f.y)*a+f.y*b;
}
@fragment fn fs_main(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let global=vec2<i32>(position.xy);
    let b=textureLoad(source,global-vec2<i32>(material.snapshot.xy),0);
    let world=position.xy/material.optics.z;
    let q=world-(material.rect.xy+material.rect.zw*0.5);
    var radius=material.corners.w;
    if q.x<0.0 {if q.y<0.0 {radius=material.corners.x;}else{radius=material.corners.z;}}
    else if q.y<0.0 {radius=material.corners.y;}
    let v=abs(q)-material.rect.zw*0.5+vec2(radius);
    let vp=max(v,vec2(0.0));
    let length_vp=length(vp);
    let distance=length_vp+min(max(v.x,v.y),0.0)-radius;
    let aa=max(fwidth(distance),0.5/material.optics.z);
    let m=(1.0-smoothstep(-aa,aa,distance))*ancestor_coverage(world);
    let coverage=1.0-smoothstep(-aa,aa,distance);
    let feather=material.surface_controls.z;
    // Outside a feathered body, preserve the snapshot exactly when emission is absent.
    if feather>0.0 && distance>=0.0 && material.extensions.y==0u {return b;}
    var gradient=vec2(0.0);
    if length_vp>0.0 {gradient=sign(q)*vp/length_vp;}
    else if v.x>=v.y {gradient=vec2(sign(q.x),0.0);}
    else {gradient=vec2(0.0,sign(q.y));}
    var filtered:vec4<f32>;
    if material.surface_controls.y==0.0 {
        filtered=textureLoad(second,global-vec2<i32>(material.filtered_domain.xy),0);
    } else {
        var delta=vec2(0.0);
        if distance<0.0 && dot(gradient,gradient)>0.0 {
            let e=glass_bevel(distance,material.glass_optics.shape.x);
            delta=material.surface_controls.y*4.0*e*(1.0-e)*gradient;
        }
        filtered=displaced_sample(position.xy+material.optics.z*delta);
    }
    var tint=material.tint.rgb;
    if material.tint.a>0.0 && material.optics.x<1.0 && material.grain.y>0.0 {
        tint=micrograin_color(tint,(world-material.rect.xy)*material.optics.z,material.grain.xy,material.grain_seed.x);
    }
    let premul=vec4(tint*material.tint.a,material.tint.a);
    let g=material.optics.x*filtered+(1.0-material.optics.x)*premul;
    // Preserve the S1 operation order, including inert explicitly authored coatings.
    if feather==0.0 && material.surface_controls.x==1.0 && material.surface_controls.y==0.0 && material.extensions.y==0u
        && (material.extensions.x==0u || !glass_optics_has_effect(material.glass_optics)) {
        return (1.0-m)*b+m*g;
    }
    var coated=g;
    if material.extensions.x!=0u {
        coated=glass_optics_color(g,material.glass_optics,material.film,(world-material.rect.xy)/material.rect.zw,distance,gradient);
    }
    var body_weight=material.surface_controls.x*coverage;
    if feather>0.0 {body_weight*=smoothstep(0.0,feather,-distance);}
    let body=(1.0-body_weight)*b+body_weight*coated;
    var emission=vec4(0.0);
    if material.extensions.y!=0u {
        emission=surface_emission_color(material.emission,distance,gradient,coverage);
    }
    let combined=emission+(1.0-emission.a)*body;
    let ancestor=ancestor_coverage(world);
    return (1.0-ancestor)*b+ancestor*combined;
}
