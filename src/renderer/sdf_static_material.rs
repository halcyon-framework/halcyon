//! Admitted static 4E source (P6), not an App producer or Button consumer.
//! The WGSL provenance comment is retained verbatim for byte-identical promotion.

use super::wgpu_renderer::GpuResources;
use crate::core::scene::Color;
use bytemuck::{Pod, Zeroable};

pub(super) const SHADER: &str = include_str!("shaders/button_4e_sdf.wgsl");
/// Left, right, top, bottom. These are only the admitted tested scale profiles.
pub(super) const OUTSETS_1X: [f32; 4] = [4.4, 4.4, 3.3, 5.5];
pub(super) const OUTSETS_2X: [f32; 4] = [4.15, 4.15, 3.05, 5.25];

/// V3's finite effect coverage is the face shadow's logical support plus
/// half a physical pixel of sampling headroom. This reproduces both frozen
/// 1x/2x mappings exactly while retaining the true target scale.
pub(super) fn v3_outsets(scale: f32) -> Option<[f32; 4]> {
    if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
        return None;
    }
    let aa = 0.5 / scale;
    Some([3.9 + aa, 3.9 + aa, 2.8 + aa, 5.0 + aa])
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct StaticSdfMaterial {
    half_size: [f32; 2],
    center: [f32; 2],
    pixels_per_logical_unit: f32,
    radius: f32,
    hover_progress: f32,
    press_progress: f32,
    base_linear: [f32; 4],
    light_linear: [f32; 4],
    shadow: [f32; 4],
}

impl StaticSdfMaterial {
    /// Face position/extent must come from the owner's admitted F/E mapping.
    /// This source does not allocate layout or select a target/clip.
    pub(super) fn resting(
        dark: bool,
        scale: f32,
        half_size: [f32; 2],
        center: [f32; 2],
    ) -> Result<Self, &'static str> {
        let (hex, deltas, opacity) = if dark {
            (0x3F3F3F, [0.025, 0.015, 0.045, 0.040], 0.18)
        } else {
            (0xE5E5E5, [0.032, 0.016, 0.038, 0.048], 0.10)
        };
        let base = Color::from_hex(hex);
        let srgb = (hex & 255) as f32 / 255.0;
        let linear = |value: f32| Color::from_srgb(value, value, value, 1.0).r;
        let light_linear = [
            linear((srgb + deltas[0]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[1]).max(0.0)),
            linear((srgb + deltas[2]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[3]).max(0.0)),
        ];
        let material = Self {
            half_size,
            center,
            pixels_per_logical_unit: scale,
            radius: 3.0,
            hover_progress: 0.0,
            press_progress: 0.0,
            base_linear: [base.r, base.g, base.b, 1.0],
            light_linear,
            shadow: [0.0, 1.1, 1.3, opacity],
        };
        material.validate()?;
        Ok(material)
    }

    fn validate(&self) -> Result<(), &'static str> {
        if !bytemuck::cast_slice::<Self, f32>(std::slice::from_ref(self))
            .iter()
            .all(|value| value.is_finite())
        {
            return Err("nonfinite material parameter");
        }
        if self.pixels_per_logical_unit <= 0.0
            || self.half_size.iter().any(|value| *value <= 0.0)
            || self.radius < 0.0
            || self.radius > self.half_size[0].min(self.half_size[1])
            || self.shadow[2] <= 0.0
            || !(0.0..=1.0).contains(&self.shadow[3])
            || !(0.0..=1.0).contains(&self.hover_progress)
            || !(0.0..=1.0).contains(&self.press_progress)
        {
            return Err("material parameter outside its declared range");
        }
        Ok(())
    }

    /// Separate V2 endpoints, keeping the P6 resting constructor and ABI intact.
    pub(super) fn v2(
        dark: bool,
        scale: f32,
        half_size: [f32; 2],
        center: [f32; 2],
        pressed: bool,
    ) -> Result<Self, &'static str> {
        let mut material = Self::resting(dark, scale, half_size, center)?;
        let srgb: f32 = if dark { 63.0 / 255.0 } else { 229.0 / 255.0 };
        let deltas: [f32; 4] = if dark {
            [0.030, 0.020, 0.070, 0.065]
        } else {
            [0.025, 0.020, 0.060, 0.085]
        };
        let linear = |value: f32| Color::from_srgb(value, value, value, 1.0).r;
        material.light_linear = [
            linear((srgb + deltas[0]).min(1.0)) - material.base_linear[0],
            material.base_linear[0] - linear((srgb - deltas[1]).max(0.0)),
            linear((srgb + deltas[2]).min(1.0)) - material.base_linear[0],
            material.base_linear[0] - linear((srgb - deltas[3]).max(0.0)),
        ];
        material.press_progress = if pressed { 1.0 } else { 0.0 };
        material.validate()?;
        Ok(material)
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    /// Separate reference-E profile; the V1/V2 constructors remain reproducible.
    pub(super) fn v3(
        dark: bool,
        scale: f32,
        half_size: [f32; 2],
        center: [f32; 2],
        pressed: bool,
    ) -> Result<Self, &'static str> {
        let mut material = Self::resting(dark, scale, half_size, center)?;
        let srgb: f32 = if dark { 72.0 / 255.0 } else { 224.0 / 255.0 };
        let base = Color::from_srgb(srgb, srgb, srgb, 1.0);
        let deltas: [f32; 4] = [0.020, 0.035, 0.060, 0.045];
        let linear = |value: f32| Color::from_srgb(value, value, value, 1.0).r;
        material.base_linear = [base.r, base.g, base.b, 1.0];
        material.light_linear = [
            linear((srgb + deltas[0]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[1]).max(0.0)),
            linear((srgb + deltas[2]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[3]).max(0.0)),
        ];
        material.shadow[3] = if dark { 0.30 } else { 0.16 };
        material.press_progress = if pressed { 1.0 } else { 0.0 };
        material.validate()?;
        Ok(material)
    }
}

pub(super) struct StaticSdfPipelines {
    pub(super) shadow: wgpu::RenderPipeline,
    pub(super) face: wgpu::RenderPipeline,
    pub(super) bindings: wgpu::BindGroupLayout,
}

impl StaticSdfPipelines {
    pub(super) fn new(gpu: &GpuResources) -> Self {
        Self::from_source(gpu, SHADER)
    }

    pub(super) fn new_v2(gpu: &GpuResources) -> Self {
        Self::from_source(gpu, include_str!("shaders/button_4e_sdf_v2.wgsl"))
    }

    pub(super) fn new_v3(gpu: &GpuResources) -> Self {
        Self::from_source(gpu, include_str!("shaders/button_4e_sdf_v3.wgsl"))
    }

    fn from_source(gpu: &GpuResources, source: &str) -> Self {
        let (shader, bindings, layout) =
            material_shader_layout(gpu, source, "admitted private static 4E source");
        Self {
            shadow: pipeline(gpu, &shader, &layout, "fs_shadow"),
            face: pipeline(gpu, &shader, &layout, "fs_face"),
            bindings,
        }
    }
}

pub(super) fn material_shader_layout(
    gpu: &GpuResources,
    source: &str,
    label: &str,
) -> (
    wgpu::ShaderModule,
    wgpu::BindGroupLayout,
    wgpu::PipelineLayout,
) {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let bindings = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Button SDF material parameters"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Button SDF private pipeline layout"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
    (shader, bindings, layout)
}

pub(super) fn pipeline(
    gpu: &GpuResources,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    entry: &str,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
}

#[cfg(test)]
mod tests {
    use super::super::wgpu_renderer::WgpuRenderer;
    use super::*;
    use wgpu::util::DeviceExt;

    #[test]
    fn static_sdf_source_preserves_abi_and_compiles() {
        assert_eq!(SHADER, include_str!("../tests/shaders/button_4e_sdf.wgsl"));
        let module = naga::front::wgsl::parse_str(SHADER).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Material"))
            .unwrap();
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            panic!("selected shader Material must remain a struct");
        };
        assert_eq!(*span as usize, std::mem::size_of::<StaticSdfMaterial>());
        let offsets = [
            std::mem::offset_of!(StaticSdfMaterial, half_size),
            std::mem::offset_of!(StaticSdfMaterial, center),
            std::mem::offset_of!(StaticSdfMaterial, pixels_per_logical_unit),
            std::mem::offset_of!(StaticSdfMaterial, radius),
            std::mem::offset_of!(StaticSdfMaterial, hover_progress),
            std::mem::offset_of!(StaticSdfMaterial, press_progress),
            std::mem::offset_of!(StaticSdfMaterial, base_linear),
            std::mem::offset_of!(StaticSdfMaterial, light_linear),
            std::mem::offset_of!(StaticSdfMaterial, shadow),
        ];
        assert_eq!(
            members
                .iter()
                .map(|member| member.offset as usize)
                .collect::<Vec<_>>(),
            offsets
        );
        let renderer = pollster::block_on(WgpuRenderer::new_headless()).expect("headless GPU");
        let gpu = renderer.gpu_resources();
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipelines = StaticSdfPipelines::new(&gpu);
        for dark in [false, true] {
            for scale in [1.0, 2.0] {
                let material =
                    StaticSdfMaterial::resting(dark, scale, [60.0, 15.0], [80.0, 30.0]).unwrap();
                let uniform = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("validated static 4E ABI"),
                        contents: material.as_bytes(),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let _group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("static 4E ABI proof"),
                    layout: &pipelines.bindings,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                });
            }
        }
        assert!(
            pollster::block_on(validation_scope.pop()).is_none(),
            "promoted static shader pipelines and ABI must validate"
        );
    }
}
