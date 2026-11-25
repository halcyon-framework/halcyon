// HAL386 §8 / HAL389 §7 / HAL085A §6.9. Straight-linear inputs, premultiplied OVER.
struct Globals {
    view_proj: mat4x4<f32>, surface_size: vec2<f32>, time_seconds: f32, flags: u32,
}
struct RoundedClip {
    rect_logical: vec4<f32>, radii_logical: vec4<f32>,
    inner_min: vec2<f32>, inner_max: vec2<f32>,
    outer_min: vec2<f32>, outer_max: vec2<f32>,
    aa_radius_logical: f32, _pad0: f32, _pad1: f32, _pad2: f32,
}
struct Light {
    direction_intensity: vec4<f32>, color: vec4<f32>,
    center_radius: vec4<f32>, rotation: vec4<f32>,
}
struct AnalyticSurfaceParams {
    rect: vec4<f32>, gradient: vec4<f32>,
    base0: vec4<f32>, base1: vec4<f32>, base2: vec4<f32>, rim: vec4<f32>,
    corners: vec4<f32>, shape: vec4<f32>, optics: vec4<f32>, limits: vec4<f32>,
    light0: Light, light1: Light, clip_control: vec4<u32>, clip: RoundedClip,
    micrograin: vec4<f32>, micrograin_seed: vec4<u32>,
    extensions: vec4<u32>, glass_optics: GlassSurfaceOpticsParams, emission: SurfaceEmissionParams,
    film: vec4<f32>,
}
@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> material: AnalyticSurfaceParams;
struct VertexInput { @location(0) position: vec2<f32> }
struct VertexOutput { @builtin(position) position: vec4<f32>, @location(0) world: vec2<f32> }
@vertex fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = globals.view_proj * vec4(input.position, 0.0, 1.0);
    out.world = input.position;
    return out;
}

// Identical geometric coverage to the existing Gradient RoundedClipUniform owner.
// This profile's admitted mask is mandatory, independent of diagnostic flags.
fn ancestor_coverage(position: vec4<f32>) -> f32 {
    if material.clip_control.x == 0u { return 1.0; }
    let clip = material.clip;
    let scale = globals.view_proj[0][0] * globals.surface_size.x / 2.0;
    let local = position.xy / scale - clip.rect_logical.xy;
    let size = clip.rect_logical.zw;
    let aa = clip.aa_radius_logical;
    if any(local < clip.outer_min) || any(local > clip.outer_max) { return 0.0; }
    if all(local >= clip.inner_min) && all(local <= clip.inner_max) { return 1.0; }
    let tl = clip.radii_logical.x; let tr = clip.radii_logical.y;
    let bl = clip.radii_logical.z; let br = clip.radii_logical.w;
    var radius = 0.0; var center = vec2(0.0);
    if local.x < tl && local.y < tl { radius = tl; center = vec2(tl); }
    else if local.x > size.x-tr && local.y < tr { radius = tr; center = vec2(size.x-tr,tr); }
    else if local.x < bl && local.y > size.y-bl { radius = bl; center = vec2(bl,size.y-bl); }
    else if local.x > size.x-br && local.y > size.y-br { radius = br; center = size-vec2(br); }
    if radius > 0.0 { return 1.0-smoothstep(radius,radius+aa,distance(local,center)); }
    var coverage = 1.0;
    if local.x < aa { coverage=min(coverage,smoothstep(0.0,aa,local.x)); }
    if local.y < aa { coverage=min(coverage,smoothstep(0.0,aa,local.y)); }
    if local.x > size.x-aa { coverage=min(coverage,smoothstep(0.0,aa,size.x-local.x)); }
    if local.y > size.y-aa { coverage=min(coverage,smoothstep(0.0,aa,size.y-local.y)); }
    return coverage;
}
fn luminance(c: vec3<f32>) -> f32 { return dot(c,vec3(0.2126,0.7152,0.0722)); }
// HAL387 arithmetic is shared by the analytic face and HAL385 tint.
// RGB is face radiance; w is the independent directional rim gate.
fn illuminate(light: Light, uv: vec2<f32>, normal: vec3<f32>, base: vec3<f32>) -> vec4<f32> {
    let delta=uv-light.center_radius.xy;
    let ellipse=vec2(light.rotation.x*delta.x+light.rotation.y*delta.y,
        -light.rotation.y*delta.x+light.rotation.x*delta.y)/light.center_radius.zw;
    let energy=exp(-0.5*dot(ellipse,ellipse))*light.direction_intensity.w;
    let direction=light.direction_intensity.xyz;
    let halfway=normalize(direction+vec3(0.0,0.0,1.0));
    let diffuse=clamp(dot(normal,direction),0.0,1.0);
    let exponent=4.0+60.0*(1.0-material.shape.z)*(1.0-material.shape.z);
    let specular=pow(clamp(dot(normal,halfway),0.0,1.0),exponent);
    let fresnel=0.04+0.96*pow(1.0-clamp(halfway.z,0.0,1.0),5.0);
    let radiance=energy*light.color.rgb;
    return vec4(radiance*(base*material.optics.x*diffuse+
        vec3(fresnel*material.optics.y*specular)),
        energy*luminance(light.color.rgb)*diffuse*specular);
}
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv=(input.world-material.rect.xy)/material.rect.zw;
    let axis=material.gradient.zw-material.gradient.xy;
    let t=clamp(dot(uv-material.gradient.xy,axis)/dot(axis,axis),0.0,1.0);
    var base: vec3<f32>;
    if t <= 0.30 { base=mix(material.base0.rgb,material.base1.rgb,t/0.30); }
    else { base=mix(material.base1.rgb,material.base2.rgb,(t-0.30)/0.70); }
    let q=input.world-(material.rect.xy+material.rect.zw*0.5);
    var radius=material.corners.w;
    if q.x < 0.0 {
        if q.y < 0.0 { radius=material.corners.x; } else { radius=material.corners.z; }
    } else if q.y < 0.0 { radius=material.corners.y; }
    let v=abs(q)-material.rect.zw*0.5+vec2(radius);
    let vp=max(v,vec2(0.0));
    let length_vp=length(vp);
    let d=length_vp+min(max(v.x,v.y),0.0)-radius;
    let scale=globals.view_proj[0][0]*globals.surface_size.x/2.0;
    let aa=max(fwidth(d),0.5/scale);
    let coverage=1.0-smoothstep(-aa,aa,d);
    var gradient=vec2(0.0);
    if length_vp > 0.0 { gradient=sign(q)*vp/length_vp; }
    else if v.x >= v.y { gradient=vec2(sign(q.x),0.0); }
    else { gradient=vec2(0.0,sign(q.y)); }
    var edge=0.0;
    if material.shape.x > 0.0 { edge=clamp(1.0-max(-d,0.0)/material.shape.x,0.0,1.0); }
    var normal=vec3(0.0,0.0,1.0);
    if edge > 0.0 && dot(gradient,gradient) > 0.0 {
        let k=material.shape.y*edge;
        normal=vec3(k*normalize(gradient),sqrt(max(1.0-k*k,0.0)));
    }
    let a=illuminate(material.light0,uv,normal,base);
    let b=illuminate(material.light1,uv,normal,base);
    let reflected=base*(1.0+material.optics.w)+a.rgb+b.rgb;
    let yr=luminance(reflected);
    let allowed=luminance(base)*(1.0+material.limits.x)+0.008*edge;
    var surface=vec3(0.0);
    if yr > 0.0 {
        if yr <= allowed { surface=reflected; }
        else { surface=reflected*(allowed/yr); }
    }
    surface=surface/max(1.0,max(surface.r,max(surface.g,surface.b)));
    if material.micrograin.y > 0.0 {
        surface=micrograin_color(surface,(input.world-material.rect.xy)*scale,material.micrograin.xy,material.micrograin_seed.x);
    }
    let weight=edge*material.optics.z*material.rim.a*clamp(a.w+b.w,0.0,1.0);
    var color=surface;
    if weight > 0.0 { color=mix(surface,material.rim.rgb,weight); }
    if material.extensions.x != 0u {
        color=glass_optics_color(vec4(color,1.0),material.glass_optics,material.film,uv,d,gradient).rgb;
    }
    if material.extensions.y != 0u {
        let body_alpha=material.limits.y*coverage;
        let body=vec4(color*body_alpha,body_alpha);
        let emission=surface_emission_color(material.emission,d,gradient,coverage);
        return ancestor_coverage(input.position)*(emission+(1.0-emission.a)*body);
    }
    let alpha=material.limits.y*coverage*ancestor_coverage(input.position);
    return vec4(color*alpha,alpha);
}
