// HAL387 §7.3: exact box integral of the bilinear lattice reconstruction.
fn micrograin_cdf(z: f32) -> f32 {
    if z <= -1.0 { return 0.0; }
    if z < 0.0 { return 0.5*(z+1.0)*(z+1.0); }
    if z < 1.0 { return 1.0-0.5*(1.0-z)*(1.0-z); }
    return 1.0;
}
fn micrograin_lattice(index: vec2<i32>, seed: u32) -> f32 {
    var h=(bitcast<u32>(index.x)*0x9e3779b9u) ^
        (bitcast<u32>(index.y)*0x85ebca6bu) ^ seed;
    h=(h ^ (h >> 16u))*0x7feb352du;
    h=(h ^ (h >> 15u))*0x846ca68bu;
    h=h ^ (h >> 16u);
    return f32(h >> 8u)*(2.0/16777215.0)-1.0;
}
fn micrograin_field(local_physical: vec2<f32>, s: f32, seed: u32) -> f32 {
    let q=local_physical/s;
    let whole=floor(q);
    let base=vec2<i32>(whole);
    let fraction=q-whole;
    let half_width=0.5/s;
    var wx: vec4<f32>;
    var wy: vec4<f32>;
    for (var j=0u; j<4u; j++) {
        let index=f32(j)-1.0;
        wx[j]=s*(micrograin_cdf(fraction.x+half_width-index)-micrograin_cdf(fraction.x-half_width-index));
        wy[j]=s*(micrograin_cdf(fraction.y+half_width-index)-micrograin_cdf(fraction.y-half_width-index));
    }
    var sum=0.0;
    for (var j=0u; j<4u; j++) {
        for (var k=0u; k<4u; k++) {
            sum+=wx[j]*wy[k]*micrograin_lattice(base+vec2<i32>(i32(j)-1,i32(k)-1),seed);
        }
    }
    let normalization=dot(wx,vec4(1.0))*dot(wy,vec4(1.0));
    return clamp(sum/normalization,-1.0,1.0);
}
fn micrograin_encode(x: f32) -> f32 {
    if x <= 0.0031308 { return 12.92*x; }
    return 1.055*pow(x,1.0/2.4)-0.055;
}
fn micrograin_decode(x: f32) -> f32 {
    if x <= 0.04045 { return x/12.92; }
    return pow((x+0.055)/1.055,2.4);
}

// HAL387 §7.4: one pure reference-material colour operation for face and tint.
fn micrograin_color(surface: vec3<f32>, local_physical: vec2<f32>, grain: vec2<f32>, seed: u32) -> vec3<f32> {
    let maximum=max(surface.r,max(surface.g,surface.b));
    let y=dot(surface,vec3(0.2126,0.7152,0.0722));
    if grain.y <= 0.0 || maximum <= 0.0 || y <= 0.0 { return surface; }
    let noise=micrograin_field(local_physical,grain.x,seed);
    let y2=micrograin_decode(clamp(micrograin_encode(y)+noise*grain.y,0.0,1.0));
    let normalized=surface/maximum;
    let perturbed=normalized*(y2/dot(normalized,vec3(0.2126,0.7152,0.0722)));
    return perturbed/max(1.0,max(perturbed.r,max(perturbed.g,perturbed.b)));
}
