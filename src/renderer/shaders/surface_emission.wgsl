// HAL388 §10: pure silhouette-connected, bounded premultiplied emission.
struct SurfaceEmissionParams {
    color: vec4<f32>, response: vec4<f32>, extent: vec4<f32>, direction: vec4<f32>,
}
fn emission_smooth(x: f32) -> f32 { return x*x*(3.0-2.0*x); }
fn surface_emission_color(p: SurfaceEmissionParams, d: f32,
    gradient: vec2<f32>, coverage: f32) -> vec4<f32> {
    let peak=max(p.color.r,max(p.color.g,p.color.b));
    if peak<=0.0 { return vec4(0.0); }
    let band=emission_smooth(clamp(1.0-max(-d,0.0)/p.response.w,0.0,1.0));
    var gate=1.0;
    if p.extent.z>0.0 {
        gate=(1.0-p.extent.z)+p.extent.z*pow(max(dot(gradient,p.direction.xy),0.0),2.0);
    }
    var h=0.0;
    if p.response.z>0.0 && p.extent.x>0.0 {
        var u=clamp(1.0+d/p.response.w,0.0,1.0);
        if d>=0.0 { u=clamp(1.0-d/p.extent.x,0.0,1.0); }
        h=pow(emission_smooth(u),p.extent.y);
    }
    let alpha=p.color.a*peak;
    let wi=coverage*alpha*(1.0-(1.0-p.response.x)*(1.0-p.response.y*band*gate));
    let wh=alpha*p.response.z*h*gate;
    let w=1.0-(1.0-wi)*(1.0-wh);
    return vec4((p.color.rgb/peak)*w,w);
}
