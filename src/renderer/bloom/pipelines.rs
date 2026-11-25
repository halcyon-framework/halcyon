use super::{BloomPlan, BloomStage, HDR_FORMAT};
use std::num::NonZeroU64;

pub(crate) struct BloomPipelines {
    layouts: [wgpu::BindGroupLayout; 3],
    pipelines: [wgpu::RenderPipeline; 4],
}

fn layout_index(stage: BloomStage) -> usize {
    match stage {
        BloomStage::Source => 0,
        BloomStage::Horizontal | BloomStage::Vertical => 1,
        BloomStage::Composite => 2,
    }
}

pub(super) fn shader_source(stage: BloomStage) -> String {
    let common = include_str!("../shaders/bloom_common.wgsl");
    match stage {
        BloomStage::Source => format!("{common}\n{}\n{}",
            include_str!("../shaders/glass_optics.wgsl"), include_str!("../shaders/bloom_source.wgsl")),
        BloomStage::Horizontal | BloomStage::Vertical =>
            format!("{common}\n{}", include_str!("../shaders/bloom_filter.wgsl")),
        BloomStage::Composite =>
            format!("{common}\n{}", include_str!("../shaders/bloom_composite.wgsl")),
    }
}

impl BloomPipelines {
    pub(crate) fn new(device: &wgpu::Device, globals: &wgpu::BindGroupLayout, target: wgpu::TextureFormat) -> Self {
        let layouts = [BloomStage::Source, BloomStage::Horizontal, BloomStage::Composite].map(|stage| {
            let mut entries = vec![wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: stage == BloomStage::Source,
                    min_binding_size: NonZeroU64::new(stage.uniform_size()),
                },
                count: None,
            }];
            if stage != BloomStage::Source {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                });
            }
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(stage.name()), entries: &entries,
            })
        });
        let pipelines = BloomStage::ALL.map(|stage| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(stage.name()),
                source: wgpu::ShaderSource::Wgsl(shader_source(stage).into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(stage.name()),
                bind_group_layouts: &[Some(globals), Some(&layouts[layout_index(stage)])],
                immediate_size: 0,
            });
            let composite = stage == BloomStage::Composite;
            let color = wgpu::ColorTargetState {
                format: if composite { target } else { HDR_FORMAT },
                blend: composite.then_some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrc,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(stage.name()),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module, entry_point: Some("vs_main"), buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module, entry_point: Some("fs_main"), targets: &[Some(color)],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        Self { layouts, pipelines }
    }

    pub(in crate::renderer) fn get(&self, stage: BloomStage) -> &wgpu::RenderPipeline {
        &self.pipelines[stage as usize]
    }
}

/// Part of one HAL385 pool entry and its sealed GPU-use token.
pub(crate) struct BloomBacking {
    pub textures: [wgpu::Texture; 3],
    pub views: [wgpu::TextureView; 3],
    pub uniforms: [wgpu::Buffer; 4],
    pub groups: [wgpu::BindGroup; 4],
}

impl BloomBacking {
    pub(crate) fn new(
        device: &wgpu::Device,
        pipelines: &BloomPipelines,
        plan: &BloomPlan,
        texture: &impl Fn((u32, u32), wgpu::TextureFormat, wgpu::TextureUsages) -> wgpu::Texture,
    ) -> Self {
        let textures = plan.regions().map(|[_x, _y, w, h]| {
            let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
            #[cfg(test)]
            let usage = usage | wgpu::TextureUsages::COPY_SRC;
            texture((w, h), HDR_FORMAT, usage)
        });
        let views = textures.each_ref().map(|texture| texture.create_view(&Default::default()));
        let uniforms = BloomStage::ALL.map(|stage| device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(stage.name()),
            size: if stage == BloomStage::Source { plan.source_stride } else { stage.uniform_size() },
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let groups = BloomStage::ALL.map(|stage| {
            let i = stage as usize;
            let uniform = wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniforms[i], offset: 0, size: NonZeroU64::new(stage.uniform_size()),
                }),
            };
            let mut entries = vec![uniform];
            if stage != BloomStage::Source {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1, resource: wgpu::BindingResource::TextureView(&views[i - 1]),
                });
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(stage.name()),
                layout: &pipelines.layouts[layout_index(stage)],
                entries: &entries,
            })
        });
        Self { textures, views, uniforms, groups }
    }

    pub(crate) fn write(&self, queue: &wgpu::Queue, plan: &BloomPlan) {
        let uniforms = [
            bytemuck::bytes_of(&plan.source),
            bytemuck::bytes_of(&plan.horizontal),
            bytemuck::bytes_of(&plan.vertical),
            bytemuck::bytes_of(&plan.composite),
        ];
        for (buffer, data) in self.uniforms.iter().zip(uniforms) {
            queue.write_buffer(buffer, 0, data);
        }
    }
}
