//! Text Rendering Pipeline
//!
//! SPEC-HAL-085: Uses shared globals at group(0) and text resources at group(1).
//!
//! Platform-specific shaders consume the same grayscale atlas resources.

pub use crate::renderer::text_vertex::TextVertex;

/// macOS: Simple grayscale mask shader.
/// CoreText already provides high-quality anti-aliased glyphs.
/// We just need to apply coverage as alpha mask.
#[cfg(target_os = "macos")]
const TEXT_SHADER: &str = include_str!("../shaders/text_macos.wgsl");

/// Non-macOS text coverage shader.
#[cfg(not(target_os = "macos"))]
const TEXT_SHADER: &str = include_str!("../shaders/text.wgsl");

/// Text rendering pipeline with bind group layout.
/// SPEC-HAL-085: Uses shared globals at group(0), text resources at group(1).
/// The globals bind group layout is shared and owned by WgpuRenderer.
pub struct TextPipeline {
    pub pipeline: wgpu::RenderPipeline,
    /// SPEC-HAL-085: Text-specific resources bind group layout (group 1)
    pub resources_bind_group_layout: wgpu::BindGroupLayout,
}

/// Create the text rendering pipeline.
/// SPEC-HAL-085: Now takes a shared globals layout to ensure consistency.
pub fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    globals_bind_group_layout: &wgpu::BindGroupLayout,
) -> TextPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Text Shader"),
        source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
    });

    // SPEC-HAL-085: Text resources bind group layout (group 1)
    // Contains atlas texture, atlas sampler, and text params.
    let resources_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Resources Bind Group Layout"),
            entries: &[
                // @group(1) @binding(0) atlas_tex
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // @group(1) @binding(1) atlas_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // @group(1) @binding(2) text_params (gamma uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

    // SPEC-HAL-085: Pipeline uses two bind groups - globals at 0, resources at 1
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Text Pipeline Layout"),
        bind_group_layouts: &[Some(globals_bind_group_layout), Some(&resources_bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Text Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(TextVertex::desc())],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    TextPipeline {
        pipeline,
        resources_bind_group_layout,
    }
}
