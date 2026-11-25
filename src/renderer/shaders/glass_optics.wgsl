// HAL386 §11 / HAL389 §8: pure authored coating, shared by Pipeline13/14.
struct GlassOpticsLight {
    direction_intensity: vec4<f32>, color: vec4<f32>,
    center_radius: vec4<f32>, rotation: vec4<f32>,
}
struct GlassSurfaceOpticsParams {
    shape: vec4<f32>, response: vec4<f32>,
    reflection_tint: vec4<f32>, rim_tint: vec4<f32>,
    light0: GlassOpticsLight, light1: GlassOpticsLight,
}
fn glass_smooth(x: f32) -> f32 { return x*x*(3.0-2.0*x); }
fn glass_bevel(d: f32, width: f32) -> f32 {
    if width <= 0.0 { return 0.0; }
    return glass_smooth(clamp(1.0-max(-d,0.0)/width,0.0,1.0));
}
fn glass_light_response(light: GlassOpticsLight, uv: vec2<f32>, n: vec3<f32>, roughness: f32) -> vec2<f32> {
    let delta=uv-light.center_radius.xy;
    let ellipse=vec2(light.rotation.x*delta.x+light.rotation.y*delta.y,
        -light.rotation.y*delta.x+light.rotation.x*delta.y)/light.center_radius.zw;
    let energy=exp(-0.5*dot(ellipse,ellipse))*light.direction_intensity.w;
    let direction=light.direction_intensity.xyz;
    let halfway=normalize(direction+vec3(0.0,0.0,1.0));
    let exponent=4.0+124.0*(1.0-roughness)*(1.0-roughness);
    return vec2(energy*pow(clamp(dot(n,halfway),0.0,1.0),exponent),
        energy*pow(max(dot(n.xy,direction.xy),0.0),2.0));
}
fn glass_optics_has_effect(p: GlassSurfaceOpticsParams) -> bool {
    let reflection=p.response.x>0.0 && p.reflection_tint.a>0.0 && (p.shape.w>0.0 || p.shape.y>0.0);
    let rim=p.response.y>0.0 && p.rim_tint.a>0.0;
    let a=p.light0; let b=p.light1;
    let ar=a.direction_intensity.w>0.0 && max(a.color.r,max(a.color.g,a.color.b))>0.0;
    let br=b.direction_intensity.w>0.0 && max(b.color.r,max(b.color.g,b.color.b))>0.0;
    return (ar && (reflection || (rim && any(a.direction_intensity.xy!=vec2(0.0))))) ||
        (br && (reflection || (rim && any(b.direction_intensity.xy!=vec2(0.0)))));
}
// HAL394 §8.2: s-polarized intensity at the fixed 650/530/460 nm samples.
fn glass_film_reflectance(ci: f32, thickness_nm: f32, film_ior: f32) -> vec3<f32> {
    let substrate_ior = 1.8;
    let sin2 = 1.0 - ci*ci;
    let cf = sqrt(1.0 - sin2/(film_ior*film_ior));
    let cs = sqrt(1.0 - sin2/(substrate_ior*substrate_ior));
    let r01 = (ci - film_ior*cf)/(ci + film_ior*cf);
    let r12 = (film_ior*cf - substrate_ior*cs)/(film_ior*cf + substrate_ior*cs);
    let phase = (4.0*3.141592653589793*film_ior*thickness_nm*cf)/vec3(650.0,530.0,460.0);
    let product = r01*r12;
    let interference = 2.0*product*cos(phase);
    return (vec3(r01*r01 + r12*r12) + interference)/(vec3(1.0 + product*product) + interference);
}

fn glass_film_color(light: GlassOpticsLight, film: vec4<f32>) -> vec3<f32> {
    let rgb = light.color.rgb;
    if light.direction_intensity.w == 0.0 || all(rgb == vec3(0.0)) { return rgb; }
    let halfway = normalize(light.direction_intensity.xyz + vec3(0.0,0.0,1.0));
    let r = clamp(glass_film_reflectance(clamp(halfway.z,0.0,1.0),film.y,film.z),vec3(0.0),vec3(1.0));
    let peak = max(r.r,max(r.g,r.b));
    var hue = vec3(0.0);
    if peak > 0.0 { hue = r/peak; }
    return rgb*(vec3(1.0 - film.x) + film.x*hue);
}

fn glass_optics_color(body: vec4<f32>, p: GlassSurfaceOpticsParams, film: vec4<f32>,
    uv: vec2<f32>, d: f32, gradient: vec2<f32>) -> vec4<f32> {
    let e=glass_bevel(d,p.shape.x);
    let k=p.shape.y*e;
    var n=vec3(0.0,0.0,1.0);
    if dot(gradient,gradient)>0.0 { n=vec3(k*gradient,sqrt(max(1.0-k*k,0.0))); }
    let a=glass_light_response(p.light0,uv,n,p.shape.z);
    let b=glass_light_response(p.light1,uv,n,p.shape.z);
    var rgb0=p.light0.color.rgb;
    var rgb1=p.light1.color.rgb;
    if film.x > 0.0 {
        rgb0=glass_film_color(p.light0,film);
        rgb1=glass_film_color(p.light1,film);
    }
    let peak0=max(rgb0.r,max(rgb0.g,rgb0.b));
    let peak1=max(rgb1.r,max(rgb1.g,rgb1.b));
    let sum_a=a.x*peak0+b.x*peak1;
    let sum_r=a.y*peak0+b.y*peak1;
    let fresnel=p.shape.w+(1.0-p.shape.w)*pow(1.0-n.z,5.0);
    var result=body;
    let wf=p.response.x*p.reflection_tint.a*fresnel*min(sum_a,1.0);
    if wf>0.0 && sum_a>0.0 {
        let rgb=((a.x*rgb0+b.x*rgb1)/sum_a)*p.reflection_tint.rgb;
        result=(1.0-wf)*result+wf*vec4(rgb,1.0);
    }
    let wr=p.response.y*p.rim_tint.a*pow(e,p.response.z)*min(sum_r,1.0);
    if wr>0.0 && sum_r>0.0 {
        let rgb=((a.y*rgb0+b.y*rgb1)/sum_r)*p.rim_tint.rgb;
        result=(1.0-wr)*result+wr*vec4(rgb,1.0);
    }
    return result;
}
