//! HAL386/389/388/394: HAL085A 704-byte ABI, unchanged 688-byte prefix.
use crate::core::math::RectF;
use crate::core::paint::{AnalyticSurfaceLight, GlassSurfaceOptics, RoundedCorners,
    SurfaceAnalytic, SurfaceEmission};
use crate::renderer::draw_plan::GradientVertex;
use crate::renderer::wgpu_renderer::RoundedClipUniform;
use bytemuck::{Pod, Zeroable};

pub(crate) const CAPACITY: usize = 512;
pub(crate) fn light_params(light: AnalyticSurfaceLight) -> [[f32; 4]; 4] {
    let d = light.direction();
    let c = light.color();
    let center = light.center_uv();
    let radii = light.radii_uv();
    let rotation = light.rotation_cos_sin();
    [[d[0], d[1], d[2], light.strength()], [c.r, c.g, c.b, 0.0],
        [center.x, center.y, radii.x, radii.y], [rotation.x, rotation.y, 0.0, 0.0]]
}

/// Shared CPU packing for the two consumers of HAL386 §11's one coating.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct GlassSurfaceOpticsParams {
    pub shape: [f32; 4],
    pub response: [f32; 4],
    pub reflection_tint: [f32; 4],
    pub rim_tint: [f32; 4],
    pub lights: [[[f32; 4]; 4]; 2],
}
impl GlassSurfaceOpticsParams {
    pub fn new(value: &GlassSurfaceOptics) -> Self {
        let p = value.parameters();
        let reflection = value.reflection_tint();
        let rim = value.rim_tint();
        Self {
            shape: [p.bevel_width, p.bevel_slope, p.roughness, p.normal_reflectance],
            response: [p.reflection_strength, p.rim_strength, p.rim_falloff, 0.0],
            reflection_tint: [reflection.r, reflection.g, reflection.b, reflection.a],
            rim_tint: [rim.r, rim.g, rim.b, rim.a],
            lights: value.lights().map(light_params),
        }
    }

    pub(crate) fn film_tail(value: &GlassSurfaceOptics) -> [f32; 4] {
        value.thin_film().map_or([0.0; 4], |film| {
            [film.strength(), film.thickness_nm(), film.film_ior(), 0.0]
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct SurfaceEmissionParams {
    pub color: [f32; 4],
    pub response: [f32; 4],
    pub extent: [f32; 4],
    pub direction: [f32; 4],
}
impl SurfaceEmissionParams {
    pub fn new(value: &SurfaceEmission) -> Self {
        let p = value.parameters();
        let c = value.color();
        Self {
            color: [c.r, c.g, c.b, c.a],
            response: [p.face_strength, p.edge_strength, p.halo_strength, p.edge_width],
            extent: [p.halo_radius, p.falloff, p.directionality, 0.0],
            direction: [p.direction.x, p.direction.y, 0.0, 0.0],
        }
    }
}

pub(crate) fn shader_source() -> String {
    format!("{}\n{}\n{}\n{}", include_str!("../../shaders/micrograin.wgsl"),
        include_str!("../../shaders/glass_optics.wgsl"),
        include_str!("../../shaders/surface_emission.wgsl"),
        include_str!("../../shaders/analytic_surface.wgsl"))
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct AnalyticSurfaceParams {
    pub rect: [f32; 4],
    pub gradient: [f32; 4],
    pub base: [[f32; 4]; 3],
    pub rim: [f32; 4],
    pub corners: [f32; 4],
    pub shape: [f32; 4],
    pub optics: [f32; 4],
    pub limits: [f32; 4],
    pub lights: [[[f32; 4]; 4]; 2],
    pub clip_control: [u32; 4],
    pub clip: RoundedClipUniform,
    pub micrograin: [f32; 4],
    pub micrograin_seed: [u32; 4],
    pub extensions: [u32; 4],
    pub glass_optics: GlassSurfaceOpticsParams,
    pub emission: SurfaceEmissionParams,
    pub film: [f32; 4],
}
impl AnalyticSurfaceParams {
    pub(crate) fn new(
        rect: RectF,
        corners: RoundedCorners,
        material: &SurfaceAnalytic,
        ancestor: Option<(RectF, [f32; 4])>,
        aa: f32,
    ) -> Self {
        let mut value = Self::zeroed();
        let base = material.base().definition();
        let p = material.parameters();
        value.rect = [rect.x, rect.y, rect.width, rect.height];
        value.gradient = [base.p0.0, base.p0.1, base.p1.0, base.p1.1];
        for (output, stop) in value.base.iter_mut().zip(&base.stops) {
            *output = [stop.color.r, stop.color.g, stop.color.b, stop.color.a];
        }
        value.corners = [corners.tl, corners.tr, corners.bl, corners.br];
        value.shape = [p.bevel_width, p.bevel_slope, p.roughness, 0.0];
        value.optics = [
            p.diffuse_strength,
            p.specular_strength,
            0.0,
            p.ambient_strength,
        ];
        value.limits = [p.max_interior_uplift, p.opacity, 0.0, 0.0];
        if let Some(rim) = material.rim() {
            let c = rim.color();
            value.rim = [c.r, c.g, c.b, c.a];
            value.optics[2] = rim.strength();
        }
        for (output, light) in value.lights.iter_mut().zip(material.lights()) {
            let d = light.direction();
            let c = light.color();
            let center = light.center_uv();
            let radii = light.radii_uv();
            let rotation = light.rotation_cos_sin();
            *output = [
                [d[0], d[1], d[2], light.strength()],
                [c.r, c.g, c.b, 0.0],
                [center.x, center.y, radii.x, radii.y],
                [rotation.x, rotation.y, 0.0, 0.0],
            ];
        }
        if let Some((r, radii)) = ancestor {
            value.clip_control[0] = 1;
            value.clip = RoundedClipUniform::new([r.x, r.y, r.width, r.height], radii, aa);
        }
        if let Some(grain) = material.micrograin() {
            value.micrograin = [grain.scale_px(), grain.amplitude_srgb(), 0.0, 0.0];
            value.micrograin_seed = [grain.seed(), 0, 0, 0];
        }
        if let Some(optics) = material.glass_optics() {
            value.extensions[0] = 1;
            value.glass_optics = GlassSurfaceOpticsParams::new(optics);
            value.film = GlassSurfaceOpticsParams::film_tail(optics);
        }
        if let Some(emission) = material.surface_emission() {
            value.extensions[1] = 1;
            value.emission = SurfaceEmissionParams::new(emission);
        }
        value
    }
}

pub(crate) struct AnalyticSurfacePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub params: wgpu::Buffer,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub stride: u32,
}
impl AnalyticSurfacePipeline {
    pub fn try_new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals: &wgpu::BindGroupLayout,
    ) -> Result<Self, crate::renderer::RendererSurfaceError> {
        let limits = device.limits();
        let alignment = u64::from(limits.min_uniform_buffer_offset_alignment).max(1);
        let size = std::mem::size_of::<AnalyticSurfaceParams>() as u64;
        let stride = size.div_ceil(alignment) * alignment;
        if u64::from(limits.max_uniform_buffer_binding_size) < size
            || stride * CAPACITY as u64 > limits.max_buffer_size
            || stride * CAPACITY as u64 > u64::from(u32::MAX)
        {
            return Err(crate::renderer::RendererSurfaceError::OutOfMemory);
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HAL386 analytic params"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(size),
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HAL386 analytic surface"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HAL386 analytic surface"),
            bind_group_layouts: &[Some(globals), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HAL386 analytic surface"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(GradientVertex::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let params = buffer(
            "HAL386 parameters",
            stride * CAPACITY as u64,
            wgpu::BufferUsages::UNIFORM,
        );
        let vertices = buffer("HAL386 vertices", 2048 * 8, wgpu::BufferUsages::VERTEX);
        let indices = buffer("HAL386 indices", 3072 * 2, wgpu::BufferUsages::INDEX);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HAL386 parameters"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &params,
                    offset: 0,
                    size: std::num::NonZeroU64::new(size),
                }),
            }],
        });
        Ok(Self {
            pipeline,
            bind_group,
            params,
            vertices,
            indices,
            stride: stride as u32,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::math::Point;
    use crate::core::paint::analytic_surface_tests::fixture;

    #[test]
    fn analytic_surface_uniform_abi_is_exact_and_reserved_lanes_zero() {
        assert_eq!(std::mem::size_of::<AnalyticSurfaceParams>(), 704);
        assert_eq!(std::mem::size_of::<GlassSurfaceOpticsParams>(), 192);
        assert_eq!(std::mem::size_of::<SurfaceEmissionParams>(), 64);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, gradient), 16);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, base), 32);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, rim), 80);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, corners), 96);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, shape), 112);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, optics), 128);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, limits), 144);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, lights), 160);
        assert_eq!(
            std::mem::offset_of!(AnalyticSurfaceParams, clip_control),
            288
        );
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, clip), 304);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, micrograin), 384);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, extensions), 416);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, glass_optics), 432);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, emission), 624);
        assert_eq!(std::mem::offset_of!(AnalyticSurfaceParams, film), 688);
        assert_eq!(
            std::mem::offset_of!(AnalyticSurfaceParams, micrograin_seed),
            400
        );
        let p = AnalyticSurfaceParams::new(
            RectF::new(1.0, 2.0, 80.0, 60.0),
            RoundedCorners::uniform(12.0),
            &fixture(0.5, 0.2, 0.6),
            None,
            1.0,
        );
        assert_eq!(p.shape[3], 0.0);
        assert_eq!(p.limits, [0.25, 0.5, 0.0, 0.0]);
        assert_eq!(p.clip_control, [0; 4]);
        assert_eq!(p.micrograin, [0.0; 4]);
        assert_eq!(p.micrograin_seed, [0; 4]);
        assert_eq!(p.extensions, [0; 4]);
        assert!(bytemuck::bytes_of(&p.glass_optics).iter().all(|b| *b == 0));
        assert!(bytemuck::bytes_of(&p.emission).iter().all(|b| *b == 0));
        assert!(bytemuck::bytes_of(&p.clip).iter().all(|b| *b == 0));
        for l in p.lights {
            assert_eq!(l[1][3], 0.0);
            assert_eq!(&l[3][2..], &[0.0, 0.0]);
        }
    }

    pub(crate) fn distance(rect: RectF, c: RoundedCorners, p: Point) -> (f64, [f64; 2]) {
        let q = [
            f64::from(p.x - rect.x - rect.width * 0.5),
            f64::from(p.y - rect.y - rect.height * 0.5),
        ];
        let r = f64::from(match (q[0] < 0.0, q[1] < 0.0) {
            (true, true) => c.tl,
            (false, true) => c.tr,
            (true, false) => c.bl,
            _ => c.br,
        });
        let v = [
            q[0].abs() - f64::from(rect.width) * 0.5 + r,
            q[1].abs() - f64::from(rect.height) * 0.5 + r,
        ];
        let vp = [v[0].max(0.0), v[1].max(0.0)];
        let len = vp[0].hypot(vp[1]);
        let sign = |x: f64| if x == 0.0 { 0.0 } else { x.signum() };
        let g = if len > 0.0 {
            [sign(q[0]) * vp[0] / len, sign(q[1]) * vp[1] / len]
        } else if v[0] >= v[1] {
            [sign(q[0]), 0.0]
        } else {
            [0.0, sign(q[1])]
        };
        (len + v[0].max(v[1]).min(0.0) - r, g)
    }
    /// Independent f64 oracle from the public values, not the packed GPU record.
    pub(crate) fn reference(
        m: &SurfaceAnalytic,
        rect: RectF,
        c: RoundedCorners,
        p: Point,
        aa: f64,
    ) -> [f64; 4] {
        reference_at_scale(m, rect, c, p, aa, 1.0)
    }

    pub(crate) fn reference_at_scale(
        m: &SurfaceAnalytic,
        rect: RectF,
        c: RoundedCorners,
        p: Point,
        aa: f64,
        scale: f64,
    ) -> [f64; 4] {
        let f = m.parameters();
        let base = m.base().definition();
        let uv = [
            f64::from(p.x - rect.x) / f64::from(rect.width),
            f64::from(p.y - rect.y) / f64::from(rect.height),
        ];
        let axis = [
            f64::from(base.p1.0 - base.p0.0),
            f64::from(base.p1.1 - base.p0.1),
        ];
        let t = (((uv[0] - f64::from(base.p0.0)) * axis[0]
            + (uv[1] - f64::from(base.p0.1)) * axis[1])
            / (axis[0] * axis[0] + axis[1] * axis[1]))
            .clamp(0.0, 1.0);
        let (a, b, u) = if t <= f64::from(0.30_f32) {
            (0, 1, t / f64::from(0.30_f32))
        } else {
            (
                1,
                2,
                (t - f64::from(0.30_f32)) / (1.0 - f64::from(0.30_f32)),
            )
        };
        let rgb = |c: crate::core::paint::Color| [f64::from(c.r), f64::from(c.g), f64::from(c.b)];
        let ca = rgb(base.stops[a].color);
        let cb = rgb(base.stops[b].color);
        let b = [
            ca[0] + (cb[0] - ca[0]) * u,
            ca[1] + (cb[1] - ca[1]) * u,
            ca[2] + (cb[2] - ca[2]) * u,
        ];
        let (d, gradient) = distance(rect, c, p);
        let edge = if f.bevel_width > 0.0 {
            (1.0 - (-d).max(0.0) / f64::from(f.bevel_width)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let length = gradient[0].hypot(gradient[1]);
        let k = f64::from(f.bevel_slope) * edge;
        let n = if edge > 0.0 && length > 0.0 {
            [
                k * gradient[0] / length,
                k * gradient[1] / length,
                (1.0 - k * k).max(0.0).sqrt(),
            ]
        } else {
            [0.0, 0.0, 1.0]
        };
        let y = |v: [f64; 3]| v[0] * 0.2126 + v[1] * 0.7152 + v[2] * 0.0722;
        let mut r = b.map(|x| x * (1.0 + f64::from(f.ambient_strength)));
        let mut gate = 0.0;
        for light in m.lights() {
            let delta = [
                uv[0] - f64::from(light.center_uv().x),
                uv[1] - f64::from(light.center_uv().y),
            ];
            let rot = light.rotation_cos_sin();
            let radii = light.radii_uv();
            let ex =
                (f64::from(rot.x) * delta[0] + f64::from(rot.y) * delta[1]) / f64::from(radii.x);
            let ey =
                (-f64::from(rot.y) * delta[0] + f64::from(rot.x) * delta[1]) / f64::from(radii.y);
            let energy = (-0.5 * (ex * ex + ey * ey)).exp() * f64::from(light.strength());
            let l = light.direction().map(f64::from);
            let h = [l[0], l[1], l[2] + 1.0];
            let hl = (h.iter().map(|x| x * x).sum::<f64>()).sqrt();
            let h = h.map(|x| x / hl);
            let dot = |a: [f64; 3], b: [f64; 3]| {
                a.into_iter()
                    .zip(b)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    .clamp(0.0, 1.0)
            };
            let diffuse = dot(n, l);
            let spec = dot(n, h).powf(4.0 + 60.0 * (1.0 - f64::from(f.roughness)).powi(2));
            let fresnel = 0.04 + 0.96 * (1.0 - h[2].clamp(0.0, 1.0)).powi(5);
            let color = rgb(light.color());
            for i in 0..3 {
                r[i] += energy
                    * color[i]
                    * (b[i] * f64::from(f.diffuse_strength) * diffuse
                        + fresnel * f64::from(f.specular_strength) * spec);
            }
            gate += energy * y(color) * diffuse * spec;
        }
        let allowed = y(b) * (1.0 + f64::from(f.max_interior_uplift)) + 0.008 * edge;
        let yr = y(r);
        let mut s = if yr == 0.0 {
            [0.0; 3]
        } else if yr <= allowed {
            r
        } else {
            r.map(|v| v * (allowed / yr))
        };
        let gamut = s.into_iter().fold(1.0_f64, f64::max);
        s = s.map(|v| v / gamut);
        if let Some(grain) = m.micrograin() {
            s = micrograin_color(
                s,
                grain,
                [
                    f64::from(p.x - rect.x) * scale,
                    f64::from(p.y - rect.y) * scale,
                ],
            );
        }
        if let Some(rim) = m.rim() {
            let w = edge * f64::from(rim.strength() * rim.color().a) * gate.clamp(0.0, 1.0);
            if w > 0.0 {
                let c = rgb(rim.color());
                for i in 0..3 {
                    s[i] += (c[i] - s[i]) * w;
                }
            }
        }
        if let Some(optics) = m.glass_optics() {
            let coated = coating_reference(optics, uv, d, gradient, [s[0], s[1], s[2], 1.0]);
            s = [coated[0], coated[1], coated[2]];
        }
        let t = ((d + aa) / (2.0 * aa)).clamp(0.0, 1.0);
        let coverage = 1.0 - t * t * (3.0 - 2.0 * t);
        let alpha = f64::from(f.opacity) * coverage;
        let body = [s[0] * alpha, s[1] * alpha, s[2] * alpha, alpha];
        if let Some(emission) = m.surface_emission() {
            let e = emission_reference(emission, d, gradient, coverage);
            std::array::from_fn(|i| e[i] + (1.0 - e[3]) * body[i])
        } else { body }
    }

    /// Independent f64 equations from checked public values, not shader packing.
    pub(crate) fn thin_film_reflectance_reference(
        ci: f64,
        thickness_nm: f64,
        film_ior: f64,
    ) -> [f64; 3] {
        let substrate_ior = 1.8;
        let sin2 = 1.0 - ci.powi(2);
        let cf = (1.0 - sin2 / film_ior.powi(2)).sqrt();
        let cs = (1.0 - sin2 / (substrate_ior * substrate_ior)).sqrt();
        let r01 = (ci - film_ior * cf) / (ci + film_ior * cf);
        let r12 = (film_ior * cf - substrate_ior * cs) / (film_ior * cf + substrate_ior * cs);
        let optical_path = 4.0 * std::f64::consts::PI * film_ior * thickness_nm * cf;
        [650.0, 530.0, 460.0].map(|wavelength| {
            let interference = 2.0 * r01 * r12 * (optical_path / wavelength).cos();
            (r01.powi(2) + r12.powi(2) + interference) / (1.0 + (r01 * r12).powi(2) + interference)
        })
    }

    fn thin_film_hue(reflectance: [f64; 3]) -> [f64; 3] {
        let peak = reflectance.into_iter().fold(0.0_f64, f64::max);
        if peak > 0.0 { reflectance.map(|r| r / peak) } else { [0.0; 3] }
    }

    #[test]
    fn analytic_surface_thin_film_matches_reference_and_only_appends_a_tail() {
        use crate::core::paint::{ThinFilmParams, analytic_surface_tests::coating};

        for (elevation, reflectance, hue) in [
            (5.0_f64, [0.10326, 0.01061, 0.06586], [1.0, 0.103, 0.638]),
            (80.0, [0.01436, 0.02811, 0.08006], [0.179, 0.351, 1.0]),
        ] {
            let ci = ((1.0 + elevation.to_radians().sin()) * 0.5).sqrt();
            let rgb = thin_film_reflectance_reference(ci, 320.0, 1.4);
            for (actual, reference) in rgb.into_iter().zip(reflectance) {
                assert!((actual - reference).abs() <= 5.1e-6, "{elevation}° {rgb:?}");
            }
            for (actual, reference) in thin_film_hue(rgb).into_iter().zip(hue) {
                assert!((actual - reference).abs() < 0.001, "{elevation}° {rgb:?}");
            }
            assert_ne!(rgb, thin_film_reflectance_reference(ci, 360.0, 1.4));
            assert_ne!(rgb, thin_film_reflectance_reference(ci, 320.0, 1.6));
        }
        assert_eq!(thin_film_hue([0.0; 3]), [0.0; 3]);

        let opaque = coating(0.6);
        let [absent, zero, active] = [None, Some(0.0), Some(0.5)].map(|strength| {
            let film = strength.map(|s| ThinFilmParams::try_new(s, 320.0, 1.4).unwrap());
            let material = fixture(0.5, 0.2, 0.6)
                .with_glass_optics(Some(opaque.try_with_thin_film(film).unwrap()));
            AnalyticSurfaceParams::new(
                RectF::new(1.0, 2.0, 80.0, 60.0), RoundedCorners::uniform(12.0), &material, None, 1.0,
            )
        });
        assert_eq!(bytemuck::bytes_of(&absent), bytemuck::bytes_of(&zero));
        assert_eq!(absent.film, [0.0; 4]);
        assert_eq!(active.film, [0.5, 320.0, 1.4, 0.0]);
        assert_eq!(&bytemuck::bytes_of(&absent)[..688], &bytemuck::bytes_of(&active)[..688]);
    }

    pub(crate) fn coating_reference(value: &GlassSurfaceOptics, uv: [f64; 2], d: f64,
        gradient: [f64; 2], body: [f64; 4]) -> [f64; 4] {
        let p = value.parameters();
        let smooth = |x: f64| x*x*(3.0-2.0*x);
        let e = if p.bevel_width>0.0 {smooth((1.0-(-d).max(0.0)/f64::from(p.bevel_width)).clamp(0.0,1.0))} else {0.0};
        let k = f64::from(p.bevel_slope)*e;
        let n = if gradient[0]!=0.0 || gradient[1]!=0.0 {
            [k*gradient[0],k*gradient[1],(1.0-k*k).max(0.0).sqrt()]
        } else {[0.0,0.0,1.0]};
        let mut a_sum=0.0;let mut r_sum=0.0;let mut reflected=[0.0;3];let mut rim=[0.0;3];
        for light in value.lights() {
            let center=light.center_uv();let radius=light.radii_uv();let rotation=light.rotation_cos_sin();
            let delta=[uv[0]-f64::from(center.x),uv[1]-f64::from(center.y)];
            let ex=(f64::from(rotation.x)*delta[0]+f64::from(rotation.y)*delta[1])/f64::from(radius.x);
            let ey=(-f64::from(rotation.y)*delta[0]+f64::from(rotation.x)*delta[1])/f64::from(radius.y);
            let envelope=(-0.5*(ex*ex+ey*ey)).exp()*f64::from(light.strength());
            let l=light.direction().map(f64::from);let h=[l[0],l[1],l[2]+1.0];
            let len=h.iter().map(|v|v*v).sum::<f64>().sqrt();let h=h.map(|v|v/len);
            let dot=n.iter().zip(h).map(|(a,b)|a*b).sum::<f64>().clamp(0.0,1.0);
            let a=envelope*dot.powf(4.0+124.0*(1.0-f64::from(p.roughness)).powi(2));
            let r=envelope*(n[0]*l[0]+n[1]*l[1]).max(0.0).powi(2);
            let color=light.color();let mut rgb=[color.r as f64,color.g as f64,color.b as f64];
            if let Some(film) = value.thin_film() {
                let hue = thin_film_hue(thin_film_reflectance_reference(
                    h[2].clamp(0.0, 1.0), film.thickness_nm().into(), film.film_ior().into(),
                ));
                let strength = f64::from(film.strength());
                for (channel, hue) in rgb.iter_mut().zip(hue) {
                    *channel *= (1.0 - strength) + strength * hue;
                }
            }
            let peak=rgb.into_iter().fold(0.0_f64,f64::max);a_sum+=a*peak;r_sum+=r*peak;
            for i in 0..3 {reflected[i]+=a*rgb[i];rim[i]+=r*rgb[i];}
        }
        let f=f64::from(p.normal_reflectance)+(1.0-f64::from(p.normal_reflectance))*(1.0-n[2]).powi(5);
        let reflection=value.reflection_tint();let rim_tint=value.rim_tint();
        let wf=f64::from(p.reflection_strength)*f64::from(reflection.a)*f*a_sum.min(1.0);
        let wr=f64::from(p.rim_strength)*f64::from(rim_tint.a)*e.powf(f64::from(p.rim_falloff))*r_sum.min(1.0);
        let mut out=body;
        for (weight,sum,color,tint) in [(wf,a_sum,reflected,reflection),(wr,r_sum,rim,rim_tint)] {
            if weight>0.0 && sum>0.0 {
                let tint=[tint.r as f64,tint.g as f64,tint.b as f64];
                for i in 0..3 {out[i]=(1.0-weight)*out[i]+weight*(color[i]/sum)*tint[i];}
                out[3]=(1.0-weight)*out[3]+weight;
            }
        }
        out
    }
    pub(crate) fn emission_reference(value: &SurfaceEmission, d: f64, g: [f64;2], coverage: f64) -> [f64;4] {
        let p=value.parameters();let color=value.color();let rgb=[color.r as f64,color.g as f64,color.b as f64];
        let peak=rgb.into_iter().fold(0.0_f64,f64::max);if peak==0.0{return [0.0;4];}
        let smooth=|x:f64| x*x*(3.0-2.0*x);
        let band=smooth((1.0-(-d).max(0.0)/f64::from(p.edge_width)).clamp(0.0,1.0));
        let gate=if p.directionality==0.0 {1.0} else {
            1.0-f64::from(p.directionality)+f64::from(p.directionality)*
                (g[0]*f64::from(p.direction.x)+g[1]*f64::from(p.direction.y)).max(0.0).powi(2)
        };
        let h=if p.halo_strength>0.0 {
            let u=if d>=0.0 {1.0-d/f64::from(p.halo_radius)}else{1.0+d/f64::from(p.edge_width)};
            smooth(u.clamp(0.0,1.0)).powf(f64::from(p.falloff))
        }else{0.0};
        let alpha=f64::from(color.a)*peak;
        let wi=coverage*alpha*(1.0-(1.0-f64::from(p.face_strength))*(1.0-f64::from(p.edge_strength)*band*gate));
        let wh=alpha*f64::from(p.halo_strength)*h*gate;let w=1.0-(1.0-wi)*(1.0-wh);
        [rgb[0]/peak*w,rgb[1]/peak*w,rgb[2]/peak*w,w]
    }

    #[test]
    fn analytic_surface_s2_common_shader_layout_and_finite_optics_emission() {
        let source=shader_source();
        let module=naga::front::wgsl::parse_str(&source).unwrap_or_else(|e|panic!("{}",e.emit_to_string(&source)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::all()).validate(&module).unwrap();
        let ty=module.types.iter().find(|(_,t)|t.name.as_deref()==Some("AnalyticSurfaceParams")).unwrap().1;
        let naga::TypeInner::Struct{members,span}= &ty.inner else {panic!("analytic uniform")};
        assert_eq!(*span, 704);
        assert_eq!(members.iter().map(|member| member.offset).collect::<Vec<_>>(),
            [0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 224, 288, 304, 384, 400,
                416, 432, 624, 688]);
        for alignment in [16_u64,64,256,512] {
            let stride=704_u64.div_ceil(alignment)*alignment;
            assert!(stride>=704);assert_eq!(stride%alignment,0);
            if alignment==256 {assert_eq!(stride,768);}
        }
        use crate::core::paint::analytic_surface_tests::{coating,emission};
        let coat=coating(0.6);let emit=emission();
        for body in [[0.0;4],[1.0;4],[0.2,0.1,0.0,0.5],[1.0e-40,0.0,0.0,1.0e-40]] {
            for d in [-80.0,-8.0,-1.0,0.0,1.0,17.999,18.0,20.0] {
                for g in [[0.0,0.0],[1.0,0.0],[0.0,1.0],[-1.0,0.0],[0.0,-1.0]] {
                    let v=coating_reference(&coat,[0.4,0.3],d,g,body);
                    let e=emission_reference(&emit,d,g,if d<0.0{1.0}else{0.0});
                    for out in [v,e] {
                        assert!(out.into_iter().all(|x|x.is_finite() && (-1.0e-12..=1.0+1.0e-12).contains(&x)));
                        assert!(out[..3].iter().all(|x|*x<=out[3]+1.0e-12));
                    }
                    if d>=18.0 {assert_eq!(e,[0.0;4]);}
                }
            }
        }
        let a=emission_reference(&emit,-1.0e-8,[0.0,1.0],0.5);
        let b=emission_reference(&emit,1.0e-8,[0.0,1.0],0.5);
        assert!(a.iter().zip(b).all(|(a,b)|(a-b).abs()<1.0e-8));
        assert_ne!(coating_reference(&coat,[0.2,0.2],-1.0,[1.0,0.0],[0.0,0.0,0.0,1.0]),
            coating_reference(&coating(-0.6),[0.2,0.2],-1.0,[1.0,0.0],[0.0,0.0,0.0,1.0]));
    }

    pub(crate) fn encode(x: f64) -> f64 {
        if x <= 0.0031308 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }
    pub(crate) fn decode(x: f64) -> f64 {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    }
    pub(crate) fn luminance(c: [f64; 3]) -> f64 {
        c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
    }

    /// Independent integral: integrate each linear half of the triangle by
    /// trapezoid area, rather than copying the shader's piecewise CDF.
    fn triangle_integral(lo: f64, hi: f64) -> f64 {
        [(-1.0_f64, 0.0_f64), (0.0, 1.0)]
            .into_iter()
            .map(|(a, b)| {
                let left = lo.max(a);
                let right = hi.min(b);
                if right <= left {
                    0.0
                } else {
                    let triangle = |x: f64| 1.0 - x.abs();
                    (right - left) * (triangle(left) + triangle(right)) * 0.5
                }
            })
            .sum()
    }
    pub(crate) fn micrograin_field(
        local: [f64; 2],
        grain: crate::core::paint::SurfaceMicrograin,
    ) -> f64 {
        let scale = f64::from(grain.scale_px());
        let q = local.map(|v| v / scale);
        let base = q.map(|v| v.floor() as i32);
        let fraction = [q[0] - q[0].floor(), q[1] - q[1].floor()];
        let weights = fraction.map(|f| {
            [-1, 0, 1, 2].map(|j| {
                scale
                    * triangle_integral(
                        f - 0.5 / scale - f64::from(j),
                        f + 0.5 / scale - f64::from(j),
                    )
            })
        });
        let mut total = 0.0;
        for j in 0..4 {
            for k in 0..4 {
                let x = (base[0] + j as i32 - 1) as u32;
                let y = (base[1] + k as i32 - 1) as u32;
                let mut h = x.wrapping_mul(0x9e3779b9) ^ y.wrapping_mul(0x85ebca6b) ^ grain.seed();
                h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
                h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
                h ^= h >> 16;
                let value = f64::from(h >> 8) * (2.0 / 16777215.0) - 1.0;
                total += weights[0][j] * weights[1][k] * value;
            }
        }
        let norm = weights[0].iter().sum::<f64>() * weights[1].iter().sum::<f64>();
        assert!(norm.is_finite() && norm > 0.0);
        (total / norm).clamp(-1.0, 1.0)
    }
    pub(crate) fn micrograin_color(
        s: [f64; 3],
        grain: crate::core::paint::SurfaceMicrograin,
        local: [f64; 2],
    ) -> [f64; 3] {
        let maximum = s.into_iter().fold(0.0_f64, f64::max);
        let y = luminance(s);
        if grain.amplitude_srgb() == 0.0 || maximum == 0.0 || y <= 0.0 {
            return s;
        }
        let noise = micrograin_field(local, grain);
        let y2 = decode((encode(y) + noise * f64::from(grain.amplitude_srgb())).clamp(0.0, 1.0));
        let n = s.map(|v| v / maximum);
        let q = n.map(|v| v * (y2 / luminance(n)));
        let gamut = q.into_iter().fold(1.0_f64, f64::max);
        q.map(|v| v / gamut)
    }

    // Shared by CPU and real GPU spatial controls; the field is not demeaned
    // before checking the required finite-crop mean.
    pub(crate) fn spatial_statistics(field: &[f64], side: usize) -> [f64; 8] {
        assert_eq!(field.len(), side * side);
        let mean = field.iter().sum::<f64>() / field.len() as f64;
        let variance = field.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / field.len() as f64;
        let sd = variance.sqrt();
        let rows: Vec<f64> = field
            .chunks_exact(side)
            .map(|r| r.iter().sum::<f64>() / side as f64)
            .collect();
        let cols: Vec<f64> = (0..side)
            .map(|x| (0..side).map(|y| field[y * side + x]).sum::<f64>() / side as f64)
            .collect();
        let ratio = |values: &[f64]| {
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / side as f64).sqrt() / sd
        };
        let corr = |lag: usize, vertical: bool| {
            let mut sum = 0.0;
            for y in 0..side {
                for x in 0..side {
                    if (if vertical { y } else { x }) + lag < side {
                        let other = if vertical {
                            (y + lag) * side + x
                        } else {
                            y * side + x + lag
                        };
                        sum += (field[y * side + x] - mean) * (field[other] - mean);
                    }
                }
            }
            sum / (side * (side - lag)) as f64 / variance
        };
        [
            mean,
            sd,
            ratio(&rows),
            ratio(&cols),
            corr(16, false),
            corr(16, true),
            corr(32, false),
            corr(32, true),
        ]
    }
    pub(crate) fn assert_spatial_statistics(stats: [f64; 8]) {
        assert!(stats[0].abs() <= 0.02, "mean {stats:?}");
        assert!(stats[1] > 0.0 && stats.iter().all(|v| v.is_finite()));
        assert!(stats[2] <= 0.15 && stats[3] <= 0.15, "axes {stats:?}");
        assert!(
            stats[4..].iter().all(|v| v.abs() <= 0.05),
            "correlation {stats:?}"
        );
    }

    #[test]
    fn analytic_surface_micrograin_prefix_tail_and_zero_equivalence() {
        use crate::core::paint::SurfaceMicrograin;
        let m = fixture(0.5, 0.2, 0.6);
        let rect = RectF::new(0.0, 0.0, 100.0, 60.0);
        let params = |m: &SurfaceAnalytic| {
            AnalyticSurfaceParams::new(rect, RoundedCorners::uniform(12.0), m, None, 1.0)
        };
        let old = params(&m);
        let grain = SurfaceMicrograin::try_new(1.5, 2.0 / 255.0, 387).unwrap();
        let textured = params(&m.clone().with_micrograin(Some(grain)));
        assert_eq!(
            &bytemuck::bytes_of(&old)[..384],
            &bytemuck::bytes_of(&textured)[..384]
        );
        assert_eq!(textured.micrograin, [1.5, 2.0 / 255.0, 0.0, 0.0]);
        assert_eq!(textured.micrograin_seed, [387, 0, 0, 0]);
        let zero = params(&m.with_micrograin(Some(
            SurfaceMicrograin::try_new(3.0, 0.0, u32::MAX).unwrap(),
        )));
        assert_eq!(bytemuck::bytes_of(&zero), bytemuck::bytes_of(&old));
    }

    #[test]
    fn analytic_surface_micrograin_reference_finite_hue_and_budget() {
        use crate::core::paint::SurfaceMicrograin;
        let grain = SurfaceMicrograin::try_new(0.75, 2.0 / 255.0, 387).unwrap();
        let mut state = 0x12345678u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            f64::from(state) / f64::from(u32::MAX)
        };
        for i in 0..100_008 {
            let s = match i {
                0 => [0.0; 3],
                1 => [1.0e-40; 3],
                2 => [1.0; 3],
                3 => [1.0, 0.0, 0.0],
                4 => [0.0, 1.0, 0.0],
                5 => [0.0, 0.0, 1.0],
                6 => [0.003; 3],
                7 => [0.9, 0.2, 0.03],
                _ => [next(), next(), next()],
            };
            let q = micrograin_color(s, grain, [next() * 1024.0 - 512.0, next() * 1024.0 - 512.0]);
            assert!(q.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
            assert!(
                (encode(luminance(q)) - encode(luminance(s))).abs()
                    <= f64::from(grain.amplitude_srgb()) + 1e-12
            );
            let a = s.into_iter().fold(0.0_f64, f64::max);
            let b = q.into_iter().fold(0.0_f64, f64::max);
            if a > 0.0 && b > 0.0 {
                for channel in 0..3 {
                    assert!((s[channel] / a - q[channel] / b).abs() < 1e-12);
                }
            }
            if a == 0.0 {
                assert_eq!(q, s);
            }
        }
        for opacity in [0.0, 0.5, 1.0] {
            let m = fixture(opacity, 0.2, 0.6);
            let rect = RectF::new(-3.25, 2.75, 100.0, 60.0);
            let c = RoundedCorners::uniform(12.0);
            for p in [
                Point::new(-2.75, 30.0),
                Point::new(40.0, 32.0),
                Point::new(96.0, 30.0),
            ] {
                let old = reference(&m, rect, c, p, 1.0);
                let new = reference_at_scale(
                    &m.clone().with_micrograin(Some(grain)),
                    rect,
                    c,
                    p,
                    1.0,
                    2.0,
                );
                assert_eq!(old[3], new[3]);
                assert!(new[..3].iter().all(|v| *v >= 0.0 && *v <= new[3]));
            }
        }
    }

    #[test]
    fn analytic_surface_micrograin_footprint_stationarity_and_spatial_controls() {
        use crate::core::paint::SurfaceMicrograin;
        for seed in [387, 305419896] {
            for scale in [0.75, 1.5, 3.0] {
                let g = SurfaceMicrograin::try_new(scale, 2.0 / 255.0, seed).unwrap();
                let field: Vec<f64> = (0..512 * 512)
                    .map(|i| micrograin_field([(i % 512) as f64 + 0.5, (i / 512) as f64 + 0.5], g))
                    .collect();
                let stats = spatial_statistics(&field, 512);
                println!(
                    "HAL387 CPU seed={seed} scale={scale} [mean,sd,row,col,corrX16,corrY16,corrX32,corrY32]={stats:?}"
                );
                assert_spatial_statistics(stats);
                for local in [[-0.5, -0.5], [0.5, 0.5], [1048576.25, -1048576.75]] {
                    let n = micrograin_field(local, g);
                    assert_eq!(n, micrograin_field(local, g));
                    assert!((-1.0..=1.0).contains(&n));
                    assert!((n - micrograin_field([local[0] + 1e-6, local[1]], g)).abs() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn reflective_rim_light_zero_preserves_lit_face_and_direction_changes_edge() {
        let rect = RectF::new(0.0, 0.0, 100.0, 60.0);
        let c = RoundedCorners::uniform(12.0);
        let m = fixture(1.0, 0.0, 0.6);
        let none =
            SurfaceAnalytic::try_smoked_glass(m.base().clone(), m.lights(), m.parameters(), None)
                .unwrap();
        for p in [
            Point::new(50.0, 30.0),
            Point::new(0.5, 30.0),
            Point::new(99.5, 30.0),
        ] {
            assert_eq!(
                reference(&m, rect, c, p, 1.0),
                reference(&none, rect, c, p, 1.0)
            );
        }
        let a = fixture(1.0, 0.2, 0.6);
        let b = fixture(1.0, 0.2, -0.6);
        assert_ne!(
            reference(&a, rect, c, Point::new(0.5, 30.0), 1.0),
            reference(&b, rect, c, Point::new(0.5, 30.0), 1.0)
        );
        for value in [0.0, 1.0e-40, 0.0001, 1.0] {
            let mut d = m.base().definition().clone();
            for s in &mut d.stops {
                s.color = crate::core::paint::Color::from_linear(value, value, value, 1.0);
            }
            let m = SurfaceAnalytic::try_smoked_glass(
                crate::core::paint::SurfaceGradient::new(d).unwrap(),
                a.lights(),
                a.parameters(),
                a.rim(),
            )
            .unwrap();
            for p in [
                Point::new(50.0, 30.0),
                Point::new(0.0, 30.0),
                Point::new(0.0, 0.0),
            ] {
                let v = reference(&m, rect, c, p, 1.0);
                assert!(
                    v.into_iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
                );
                assert!(v[..3].iter().all(|x| *x <= v[3]));
            }
        }
    }
}
