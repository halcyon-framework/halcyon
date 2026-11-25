// SPEC-HAL-296 §22: PlotRenderer — GPU rendering engine for a single Plot.
//
// Phase 3: PlotRenderPool. Renderer is wrapped in Arc<Mutex> and shared
// with a pool of worker threads. Generation counters skip stale jobs.

use super::AxisKind;
use super::color_scheme::PlotColorScheme;
use crate::plot::grid;
use crate::plot::price_level::PriceLevel;
use crate::plot::series::candle::{Uniforms, Vertex as OverlayVertex};
use crate::plot::series::circle::CIRCLE_SHADER;
use crate::plot::series::circle::CircleVertex;
use crate::plot::series::{PhysicalRect, Series};
use crate::plot::transform::Transform;
use crate::plot::view_state::ViewState;
use crate::renderer::custom_render_pass::{
    CustomRenderPass, CustomRenderPassError, CustomRenderPassResult,
    PhysicalRect as RenderPhysicalRect,
};
use crate::renderer::wgpu_renderer::GpuResources;
use wgpu::util::DeviceExt;

/// Snapshot of Plot state needed to produce one frame.
/// No references to Plot, no Arc, no Widget state.
pub(crate) struct RenderParams {
    pub view_state: ViewState,
    /// Exact nominated tag carried back with the completed frame.
    pub reference_tag: Option<PriceLevel>,
    pub x_axis_kind: AxisKind,
    pub y_axis_kind: AxisKind,
    pub scale_factor: f32,

    /// Local crosshair position in logical pixels. None = hidden.
    pub crosshair_pos: Option<(f32, f32)>,
    /// True if local crosshair should be suppressed (another Plot owns probe).
    pub crosshair_suppressed: bool,

    /// Box selection corners in logical pixels. None = no box.
    pub box_selection: Option<(f32, f32, f32, f32)>,

    /// Pre-computed selection highlight rects in PHYSICAL pixels.
    pub selection_rects: Vec<[f32; 4]>,

    /// Synced crosshair from shared Probe — physical-pixel X position.
    /// None if no probe, probe inactive, or self-sourced.
    pub synced_crosshair_px: Option<f32>,

    /// Series indices to skip during rendering (hidden by user toggle).
    pub hidden_series: std::collections::HashSet<usize>,

    /// Pre-computed price level line + tag background vertices.
    pub level_vertices: Vec<OverlayVertex>,

    /// Tool overlay vertices (measurement line, guides, etc.)
    pub tool_overlay_vertices: Vec<OverlayVertex>,

    /// Pre-computed circle marker vertices (SDF quads).
    pub circle_vertices: Vec<CircleVertex>,

    /// Pre-computed line marker vertices (overlay quads + arrowheads).
    pub line_vertices: Vec<OverlayVertex>,

    /// Pre-computed Bezier curve vertices (overlay quads + arrowheads).
    pub bezier_vertices: Vec<OverlayVertex>,

    /// Pre-computed styled path vertices (per-segment colored overlay quads).
    pub styled_path_vertices: Vec<OverlayVertex>,

    /// Pre-computed path marker vertices (uniform-style polyline overlay quads).
    pub path_vertices: Vec<OverlayVertex>,

    pub color_scheme: PlotColorScheme,
}

/// SPEC-HAL-296 §22: GPU rendering engine for a single Plot.
///
/// Owns the offscreen texture, overlay pipeline, and all GPU buffers.
/// Receives render parameters from Plot; produces completed frames.
pub(crate) struct PlotRenderer {
    gpu: GpuResources,

    // Offscreen texture lifecycle
    render_pass: Option<CustomRenderPass>,
    pub(crate) texture_width: u32,
    pub(crate) texture_height: u32,

    // Overlay pipeline (crosshair, grid, selection, box selection)
    overlay_pipeline: wgpu::RenderPipeline,
    overlay_uniform_buffer: wgpu::Buffer,
    overlay_bind_group: wgpu::BindGroup,

    // Reusable vertex buffer for overlays (crosshair, etc.)
    overlay_buffer: Option<wgpu::Buffer>,

    // Circle marker pipeline (SDF shader, same as CircleSeries)
    circle_pipeline: wgpu::RenderPipeline,
    circle_uniform_buffer: wgpu::Buffer,
    circle_bind_group: wgpu::BindGroup,

    // Data series
    series: Vec<Box<dyn Series>>,
    series_generation: u64,
    series_data_generation: u64,
    presentation_pending: bool,

    // Generation counter for stale-job skipping
    last_rendered_gen: u64,
}

impl PlotRenderer {
    /// Create a new PlotRenderer with initialized GPU resources.
    pub(crate) fn new(
        gpu: GpuResources,
        width: u32,
        height: u32,
        series: Vec<Box<dyn Series>>,
        texture_id: u64,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_publication(gpu, width, height, series, texture_id, true)
    }

    pub(crate) fn new_unpublished(
        gpu: GpuResources,
        width: u32,
        height: u32,
        series: Vec<Box<dyn Series>>,
        texture_id: u64,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_publication(gpu, width, height, series, texture_id, false)
    }

    fn new_with_publication(
        gpu: GpuResources,
        width: u32,
        height: u32,
        mut series: Vec<Box<dyn Series>>,
        texture_id: u64,
        publish_initial_front: bool,
    ) -> CustomRenderPassResult<Self> {
        let rect = RenderPhysicalRect::new(0, 0, width, height);
        let render_pass = if publish_initial_front {
            CustomRenderPass::new(texture_id, gpu.clone(), rect)?
        } else {
            CustomRenderPass::new_unpublished(texture_id, gpu.clone(), rect)?
        };
        for s in series.iter_mut() {
            s.init(&gpu.device, &gpu.queue, gpu.format);
        }

        // Overlay pipeline
        let overlay_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Plot Overlay Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    crate::plot::series::candle::OVERLAY_SHADER.into(),
                ),
            });

        let uniform_data = [width as f32, height as f32];
        let overlay_uniform_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Plot Overlay Uniforms"),
                    contents: bytemuck::cast_slice(&uniform_data),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let bgl = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Plot Overlay BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let overlay_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plot Overlay BG"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: overlay_uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Plot Overlay Pipeline Layout"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });

        let overlay_pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Plot Overlay Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &overlay_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(OverlayVertex::desc())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &overlay_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        // Circle marker pipeline (SDF shader, same as CircleSeries)
        let circle_uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Plot Circle Marker Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let circle_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Plot Circle Marker Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let circle_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Plot Circle Marker Bind Group"),
            layout: &circle_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: circle_uniform_buffer.as_entire_binding(),
            }],
        });

        let circle_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Plot Circle Marker Pipeline Layout"),
                    bind_group_layouts: &[Some(&circle_bind_group_layout)],
                    immediate_size: 0,
                });

        let circle_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Plot Circle Marker Shader"),
                source: wgpu::ShaderSource::Wgsl(CIRCLE_SHADER.into()),
            });

        let circle_pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Plot Circle Marker Pipeline"),
                layout: Some(&circle_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &circle_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(CircleVertex::desc())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &circle_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let renderer = Self {
            gpu,
            render_pass: Some(render_pass),
            texture_width: width,
            texture_height: height,
            overlay_pipeline,
            overlay_uniform_buffer,
            overlay_bind_group,
            overlay_buffer: None,
            circle_pipeline,
            circle_uniform_buffer,
            circle_bind_group,
            series,
            series_generation: 0,
            series_data_generation: 0,
            presentation_pending: false,
            last_rendered_gen: 0,
        };
        #[cfg(test)]
        crate::renderer::custom_render_pass::audit::record_plot_renderer();
        Ok(renderer)
    }

    /// Resize the offscreen texture.
    pub(crate) fn resize(&mut self, width: u32, height: u32) -> CustomRenderPassResult<()> {
        if let Some(render_pass) = self.render_pass.as_mut() {
            render_pass.resize(RenderPhysicalRect::new(0, 0, width, height))?;
        }
        self.presentation_pending = false;
        self.series_generation = self.series_generation.wrapping_add(1);
        self.update_target_extent(width, height);
        Ok(())
    }

    pub(crate) fn resize_unpublished(
        &mut self,
        width: u32,
        height: u32,
    ) -> CustomRenderPassResult<()> {
        if let Some(render_pass) = self.render_pass.as_mut() {
            render_pass.resize_unpublished(RenderPhysicalRect::new(0, 0, width, height))?;
        }
        self.presentation_pending = false;
        self.series_generation = self.series_generation.wrapping_add(1);
        self.update_target_extent(width, height);
        Ok(())
    }

    pub(crate) fn retarget_unpublished(
        &mut self,
        texture_id: u64,
        width: u32,
        height: u32,
    ) -> CustomRenderPassResult<()> {
        let replacement = CustomRenderPass::new_unpublished(
            texture_id,
            self.gpu.clone(),
            RenderPhysicalRect::new(0, 0, width, height),
        )?;
        if let Some(previous) = self.render_pass.replace(replacement) {
            previous.destroy_deferred()?;
        }
        self.presentation_pending = false;
        self.series_generation = self.series_generation.wrapping_add(1);
        self.update_target_extent(width, height);
        Ok(())
    }

    pub(crate) fn unpublish_front(&mut self) -> CustomRenderPassResult<()> {
        if let Some(render_pass) = self.render_pass.as_ref() {
            render_pass.queue_front_unregistration()?;
        }
        self.presentation_pending = false;
        self.series_generation = self.series_generation.wrapping_add(1);
        Ok(())
    }

    fn update_target_extent(&mut self, width: u32, height: u32) {
        self.texture_width = width;
        self.texture_height = height;

        // Update overlay uniforms
        let uniform_data = [width as f32, height as f32];
        self.gpu.queue.write_buffer(
            &self.overlay_uniform_buffer,
            0,
            bytemuck::cast_slice(&uniform_data),
        );
    }

    /// Render one frame to the offscreen texture.
    pub(crate) fn render(&mut self, params: &RenderParams) -> CustomRenderPassResult<()> {
        if self.presentation_pending {
            return Err(CustomRenderPassError::ContentPinned);
        }

        let viewport = PhysicalRect {
            x: 0,
            y: 0,
            width: self.texture_width,
            height: self.texture_height,
        };

        let transform = Transform::from_view_state_with_axes(
            &params.view_state,
            self.texture_width as f32,
            self.texture_height as f32,
            params.x_axis_kind,
            params.y_axis_kind,
        );

        // Prepare visible series
        for (i, s) in self.series.iter_mut().enumerate() {
            if !params.hidden_series.contains(&i) {
                s.apply_color_scheme(&params.color_scheme);
                s.prepare(&self.gpu.device, &self.gpu.queue, &transform, viewport);
            }
        }

        // Collect visible indices sorted by z_order
        let mut order: Vec<usize> = (0..self.series.len())
            .filter(|i| !params.hidden_series.contains(i))
            .collect();
        order.sort_by_key(|&i| self.series[i].z_order());

        let sf = params.scale_factor;
        let w = self.texture_width as f32;
        let h = self.texture_height as f32;

        // Build crosshair vertices
        let crosshair_vertex_count = if !params.crosshair_suppressed {
            if let Some((cx, cy)) = params.crosshair_pos {
                let cx = (cx * sf).round();
                let cy = (cy * sf).round();
                let lw = 1.0_f32;
                let color = params.color_scheme.crosshair.to_array();

                let verts = [
                    // Horizontal line
                    OverlayVertex {
                        position: [0.0, cy - lw * 0.5],
                        color,
                    },
                    OverlayVertex {
                        position: [w, cy - lw * 0.5],
                        color,
                    },
                    OverlayVertex {
                        position: [0.0, cy + lw * 0.5],
                        color,
                    },
                    OverlayVertex {
                        position: [0.0, cy + lw * 0.5],
                        color,
                    },
                    OverlayVertex {
                        position: [w, cy - lw * 0.5],
                        color,
                    },
                    OverlayVertex {
                        position: [w, cy + lw * 0.5],
                        color,
                    },
                    // Vertical line
                    OverlayVertex {
                        position: [cx - lw * 0.5, 0.0],
                        color,
                    },
                    OverlayVertex {
                        position: [cx + lw * 0.5, 0.0],
                        color,
                    },
                    OverlayVertex {
                        position: [cx - lw * 0.5, h],
                        color,
                    },
                    OverlayVertex {
                        position: [cx - lw * 0.5, h],
                        color,
                    },
                    OverlayVertex {
                        position: [cx + lw * 0.5, 0.0],
                        color,
                    },
                    OverlayVertex {
                        position: [cx + lw * 0.5, h],
                        color,
                    },
                ];

                let ch_data = bytemuck::cast_slice(&verts);
                let needed = ch_data.len() as u64;
                match &self.overlay_buffer {
                    Some(buf) if buf.size() >= needed => {
                        self.gpu.queue.write_buffer(buf, 0, ch_data);
                    }
                    _ => {
                        self.overlay_buffer = Some(self.gpu.device.create_buffer_init(
                            &wgpu::util::BufferInitDescriptor {
                                label: Some("Plot Overlay Buffer"),
                                contents: ch_data,
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            },
                        ));
                    }
                }

                // Update overlay uniforms
                let data = [w, h];
                self.gpu.queue.write_buffer(
                    &self.overlay_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&data),
                );

                12u32
            } else {
                0
            }
        } else {
            0
        };

        // Synced crosshair from shared Probe
        let synced_crosshair_buffer = if let Some(px) = params.synced_crosshair_px {
            let px = px.round();
            let lw = 1.0_f32;
            let color = params.color_scheme.synced_crosshair.to_array();

            let verts = [
                OverlayVertex {
                    position: [px - lw * 0.5, 0.0],
                    color,
                },
                OverlayVertex {
                    position: [px + lw * 0.5, 0.0],
                    color,
                },
                OverlayVertex {
                    position: [px - lw * 0.5, h],
                    color,
                },
                OverlayVertex {
                    position: [px - lw * 0.5, h],
                    color,
                },
                OverlayVertex {
                    position: [px + lw * 0.5, 0.0],
                    color,
                },
                OverlayVertex {
                    position: [px + lw * 0.5, h],
                    color,
                },
            ];

            let data = bytemuck::cast_slice(&verts);
            Some((
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Plot Synced Crosshair"),
                        contents: data,
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                6u32,
            ))
        } else {
            None
        };

        // Box selection vertices
        let box_selection_buffer = if let Some((bx1, by1, bx2, by2)) = params.box_selection {
            let bx1 = (bx1 * sf).round();
            let by1 = (by1 * sf).round();
            let bx2 = (bx2 * sf).round();
            let by2 = (by2 * sf).round();

            let lw = 2.0_f32;
            let color = params.color_scheme.box_selection_border.to_array();

            let left = bx1.min(bx2);
            let right = bx1.max(bx2);
            let top = by1.min(by2);
            let bottom = by1.max(by2);

            let mut verts = Vec::with_capacity(24);
            // Top edge
            verts.push(OverlayVertex {
                position: [left, top - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, top - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [left, top + lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [left, top + lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, top - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, top + lw * 0.5],
                color,
            });
            // Bottom edge
            verts.push(OverlayVertex {
                position: [left, bottom - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, bottom - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [left, bottom + lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [left, bottom + lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, bottom - lw * 0.5],
                color,
            });
            verts.push(OverlayVertex {
                position: [right, bottom + lw * 0.5],
                color,
            });
            // Left edge
            verts.push(OverlayVertex {
                position: [left - lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [left + lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [left - lw * 0.5, bottom],
                color,
            });
            verts.push(OverlayVertex {
                position: [left - lw * 0.5, bottom],
                color,
            });
            verts.push(OverlayVertex {
                position: [left + lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [left + lw * 0.5, bottom],
                color,
            });
            // Right edge
            verts.push(OverlayVertex {
                position: [right - lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [right + lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [right - lw * 0.5, bottom],
                color,
            });
            verts.push(OverlayVertex {
                position: [right - lw * 0.5, bottom],
                color,
            });
            verts.push(OverlayVertex {
                position: [right + lw * 0.5, top],
                color,
            });
            verts.push(OverlayVertex {
                position: [right + lw * 0.5, bottom],
                color,
            });

            let data = bytemuck::cast_slice(&verts);
            Some((
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Plot Box Selection Buffer"),
                        contents: data,
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                24u32,
            ))
        } else {
            None
        };

        // Selection highlight vertices
        let selection_highlight = if !params.selection_rects.is_empty() {
            let mut verts: Vec<OverlayVertex> = Vec::new();

            for &rect in &params.selection_rects {
                let (x1, y1, x2, y2) = (
                    rect[0].round(),
                    rect[1].round(),
                    rect[2].round(),
                    rect[3].round(),
                );

                // Semi-transparent fill wash
                let fill_color = params.color_scheme.selection_fill.to_array();
                verts.push(OverlayVertex {
                    position: [x1, y1],
                    color: fill_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1],
                    color: fill_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2],
                    color: fill_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2],
                    color: fill_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1],
                    color: fill_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y2],
                    color: fill_color,
                });

                // Thin bright border (1px)
                let bw = 1.0_f32;
                let border_color = params.color_scheme.selection_border.to_array();
                // Top
                verts.push(OverlayVertex {
                    position: [x1, y1],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1 + bw],
                    color: border_color,
                });
                // Bottom
                verts.push(OverlayVertex {
                    position: [x1, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y2],
                    color: border_color,
                });
                // Left
                verts.push(OverlayVertex {
                    position: [x1, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1 + bw, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1 + bw, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x1 + bw, y2 - bw],
                    color: border_color,
                });
                // Right
                verts.push(OverlayVertex {
                    position: [x2 - bw, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2 - bw, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2 - bw, y2 - bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y1 + bw],
                    color: border_color,
                });
                verts.push(OverlayVertex {
                    position: [x2, y2 - bw],
                    color: border_color,
                });
            }

            if !verts.is_empty() {
                let data = bytemuck::cast_slice(&verts);
                Some((
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Plot Selection Highlight"),
                            contents: data,
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                    verts.len() as u32,
                ))
            } else {
                None
            }
        } else {
            None
        };

        // Grid lines
        let grid_buffer = {
            let grid_verts = grid::build_grid_vertices(
                &transform,
                w,
                h,
                6,
                4,
                params.color_scheme.grid.to_array(),
            );
            if !grid_verts.is_empty() {
                let data = bytemuck::cast_slice(&grid_verts);
                Some((
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Plot Grid Buffer"),
                            contents: data,
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                    grid_verts.len() as u32,
                ))
            } else {
                None
            }
        };

        // Render pass
        let render_pass = self
            .render_pass
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)?;
        let mut frame = render_pass.begin_frame(Some("Plot Encoder"))?;

        {
            let (encoder, view) = frame.encoder_and_view();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Plot Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear({
                            let bg = params.color_scheme.background;
                            wgpu::Color {
                                r: bg.r as f64,
                                g: bg.g as f64,
                                b: bg.b as f64,
                                a: bg.a as f64,
                            }
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Draw grid lines behind data
            if let Some((ref buf, count)) = grid_buffer {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..count, 0..1);
            }

            // Series
            for &idx in &order {
                self.series[idx].render(&mut pass, viewport);
            }

            // Selection highlight
            if let Some((ref sel_buf, sel_count)) = selection_highlight {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, sel_buf.slice(..));
                pass.draw(0..sel_count, 0..1);
            }

            // Price level lines + tag backgrounds
            if !params.level_vertices.is_empty() {
                let level_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Price Level Buffer"),
                            contents: bytemuck::cast_slice(&params.level_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, level_buffer.slice(..));
                pass.draw(0..params.level_vertices.len() as u32, 0..1);
            }

            // Line markers (overlay pipeline — rotated quads + arrowheads)
            if !params.line_vertices.is_empty() {
                let line_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Line Marker Buffer"),
                            contents: bytemuck::cast_slice(&params.line_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, line_buffer.slice(..));
                pass.draw(0..params.line_vertices.len() as u32, 0..1);
            }

            // Bezier curve markers (overlay pipeline)
            if !params.bezier_vertices.is_empty() {
                let bezier_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Bezier Marker Buffer"),
                            contents: bytemuck::cast_slice(&params.bezier_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, bezier_buffer.slice(..));
                pass.draw(0..params.bezier_vertices.len() as u32, 0..1);
            }

            // Styled path markers (overlay pipeline)
            if !params.styled_path_vertices.is_empty() {
                let sp_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Styled Path Buffer"),
                            contents: bytemuck::cast_slice(&params.styled_path_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, sp_buffer.slice(..));
                pass.draw(0..params.styled_path_vertices.len() as u32, 0..1);
            }

            // Path markers (overlay pipeline)
            if !params.path_vertices.is_empty() {
                let path_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Path Marker Buffer"),
                            contents: bytemuck::cast_slice(&params.path_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, path_buffer.slice(..));
                pass.draw(0..params.path_vertices.len() as u32, 0..1);
            }

            // Circle markers (SDF shader)
            if !params.circle_vertices.is_empty() {
                let circle_uniforms = Uniforms {
                    viewport_size: [viewport.width as f32, viewport.height as f32],
                };
                self.gpu.queue.write_buffer(
                    &self.circle_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[circle_uniforms]),
                );
                let circle_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Circle Marker Buffer"),
                            contents: bytemuck::cast_slice(&params.circle_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.circle_pipeline);
                pass.set_bind_group(0, &self.circle_bind_group, &[]);
                pass.set_vertex_buffer(0, circle_buffer.slice(..));
                pass.draw(0..params.circle_vertices.len() as u32, 0..1);
            }

            // Tool overlay (measurement lines, guides, etc.)
            if !params.tool_overlay_vertices.is_empty() {
                let tool_buffer =
                    self.gpu
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Tool Overlay Buffer"),
                            contents: bytemuck::cast_slice(&params.tool_overlay_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, tool_buffer.slice(..));
                pass.draw(0..params.tool_overlay_vertices.len() as u32, 0..1);
            }

            // Synced crosshair
            if let Some((ref sync_buf, sync_count)) = synced_crosshair_buffer {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, sync_buf.slice(..));
                pass.draw(0..sync_count, 0..1);
            }

            // Local crosshair
            if crosshair_vertex_count > 0 {
                if let Some(ref buf) = self.overlay_buffer {
                    pass.set_pipeline(&self.overlay_pipeline);
                    pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..crosshair_vertex_count, 0..1);
                }
            }

            // Box selection outline
            if let Some((ref box_buf, box_count)) = box_selection_buffer {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_bind_group(0, &self.overlay_bind_group, &[]);
                pass.set_vertex_buffer(0, box_buf.slice(..));
                pass.draw(0..box_count, 0..1);
            }
        }

        self.render_pass
            .as_mut()
            .ok_or(CustomRenderPassError::Destroyed)?
            .submit_and_swap_deferred_registration(frame)?;
        self.presentation_pending = true;
        Ok(())
    }

    pub(crate) fn last_rendered_generation(&self) -> u64 {
        self.last_rendered_gen
    }

    pub(crate) fn set_last_rendered_generation(&mut self, generation: u64) {
        self.last_rendered_gen = generation;
    }

    pub(crate) fn series_mut(&mut self) -> &mut [Box<dyn Series>] {
        self.series_generation = self.series_generation.wrapping_add(1);
        self.series_data_generation += 1;
        &mut self.series
    }

    pub(super) fn series_data_generation(&self) -> u64 {
        self.series_data_generation
    }

    pub(super) fn gpu(&self) -> &GpuResources {
        &self.gpu
    }

    pub(crate) fn series_generation(&self) -> u64 {
        self.series_generation
    }

    pub(crate) fn presentation_pending(&self) -> bool {
        self.presentation_pending
    }

    pub(crate) fn present_pending_frame(&mut self) -> CustomRenderPassResult<bool> {
        if !self.presentation_pending {
            return Ok(false);
        }
        let render_pass = self
            .render_pass
            .as_ref()
            .ok_or(crate::renderer::custom_render_pass::CustomRenderPassError::Destroyed)?;
        render_pass.queue_front_registration()?;
        self.presentation_pending = false;
        Ok(true)
    }

    pub(super) fn request_pending_plot_front(
        &self,
        current: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> CustomRenderPassResult<crate::renderer::image_binding::ImageBindingRequest> {
        if !self.presentation_pending {
            return Err(CustomRenderPassError::Destroyed);
        }
        self.render_pass
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)?
            .request_plot_front_admission(current)
    }

    pub(super) fn finish_plot_registration(&mut self) {
        self.presentation_pending = false;
    }

    pub(crate) fn destroy_deferred(&mut self) {
        if let Some(render_pass) = self.render_pass.take() {
            let _ = render_pass.destroy_deferred();
            self.gpu.image_admission.notify();
        }
    }
}

impl Drop for PlotRenderer {
    fn drop(&mut self) {
        self.destroy_deferred();
    }
}
