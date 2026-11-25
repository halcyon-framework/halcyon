//! Isolated headless material proof using the existing CustomRenderPass/Image path.
//! No Button, prepared-style, Showcase or production pipeline integration.

use std::cell::Cell;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::compositor::{Compositor, SceneKey};
use crate::core::app::RetainedSceneTestHarness;
use crate::core::icon::IconRegistry;
use crate::core::layout::{Alignment, Constraints, EdgeInsets, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::scene::{ImageTextureSource, SceneBuilder, SceneGraph, SceneNode};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, OverflowMode, WidgetTree};
use crate::renderer::DrawPlanBuilder;
use crate::renderer::custom_render_pass::{CustomRenderPass, PhysicalRect};
use crate::renderer::image_binding::ImageBinding;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::{GpuResources, WgpuRenderer};
use crate::utils::id::WidgetInternalId;
use crate::widgets::{flex::Flex, label::Label};

const SHADER: &str = include_str!("shaders/button_4e_sdf.wgsl");
const LOGICAL_WIDTH: u32 = 160;
const LOGICAL_HEIGHT: u32 = 60;
const SHADOW_Z: u32 = 0;
const FACE_Z: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Material {
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

impl Material {
    fn resting(dark: bool, scale: f32) -> Self {
        let (hex, deltas, opacity) = if dark {
            (0x3F3F3F, [0.025, 0.015, 0.045, 0.040], 0.18)
        } else {
            (0xE5E5E5, [0.032, 0.016, 0.038, 0.048], 0.10)
        };
        let base = Color::from_hex(hex);
        let srgb = (hex & 255) as f32 / 255.0;
        let linear = |value: f32| Color::from_srgb(value, value, value, 1.0).r;
        // sRGB-equivalent deltas are converted at the specified base once;
        // all spatial lighting and composition then operate in linear light.
        let light_linear = [
            linear((srgb + deltas[0]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[1]).max(0.0)),
            linear((srgb + deltas[2]).min(1.0)) - base.r,
            base.r - linear((srgb - deltas[3]).max(0.0)),
        ];
        Self {
            half_size: [60.0, 15.0],
            center: [80.0, 30.0],
            pixels_per_logical_unit: scale,
            radius: 3.0,
            hover_progress: 0.0,
            press_progress: 0.0,
            base_linear: [base.r, base.g, base.b, 1.0],
            light_linear,
            shadow: [0.0, 1.1, 1.3, opacity],
        }
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
}

fn pipeline(
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

fn write_readback(
    gpu: &GpuResources,
    output: &wgpu::Texture,
    width: u32,
    height: u32,
    path: &Path,
) {
    let row_bytes = width * 4;
    let padded =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("4E actual GPU readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("4E readback encoder"),
        });
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([copy.finish()]);
    let completion = Arc::new(Mutex::new(None));
    let callback = completion.clone();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback.lock().unwrap() = Some(result);
        });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU readback poll");
    completion
        .lock()
        .unwrap()
        .take()
        .expect("GPU readback callback")
        .expect("GPU readback mapping");
    let data = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let mut file = std::fs::File::create(path).expect("preview file");
    write!(file, "P6\n{width} {height}\n255\n").unwrap();
    for y in 0..height as usize {
        let start = y * padded as usize;
        for pixel in data[start..start + row_bytes as usize].chunks_exact(4) {
            // new_headless uses Bgra8UnormSrgb; no color correction or retouching.
            file.write_all(&[pixel[2], pixel[1], pixel[0]]).unwrap();
        }
    }
    drop(data);
    buffer.unmap();
}

// A real retained Image owner, with no Button or custom-render participant claims.
struct RetainedSdfImageOwner {
    id: WidgetInternalId,
    binding: ImageBinding,
    requested: Rc<Cell<bool>>,
    published: bool,
}

impl WidgetDowncast for RetainedSdfImageOwner {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for RetainedSdfImageOwner {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn update(&mut self, _: f32, cx: &mut UpdateContext) {
        if self.published != self.requested.get() {
            self.published = self.requested.get();
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.published != self.requested.get()
    }

    fn layout(
        &mut self,
        _: &mut LayoutContext,
        constraints: Constraints,
        _: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(constraints.constrain(Size::new(120.0, 30.0)))
    }

    fn paint(&self, cx: &mut PaintContext, face: RectF, _: &[NodeId]) {
        if !self.published {
            return;
        }
        let coverage = RectF::new(
            face.x - 4.4,
            face.y - 3.3,
            face.width + 8.8,
            face.height + 8.8,
        );
        cx.scene.push_admitted_image(
            coverage,
            &self.binding,
            RectF::new(
                15.6 / 160.0,
                11.7 / 60.0,
                coverage.width / 160.0,
                coverage.height / 60.0,
            ),
            Color::WHITE,
            LAYER_CONTENT,
            Some(SceneNodeKey::new(self.id.0)),
        );
    }
}

fn retained_coverage_replacement(
    renderer: &mut WgpuRenderer,
    gpu: &GpuResources,
    binding: ImageBinding,
    directory: &Path,
) {
    let requested = Rc::new(Cell::new(true));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Flex::row()
                .with_padding(EdgeInsets::all(20.0))
                .with_spacing(20.0)
                .with_align_items(Alignment::Start)
                .with_background(Color::from_hex(0xF2F2F2)),
        ),
        None,
        0,
    );
    tree.set_overflow(root, OverflowMode::Clip);
    let material_id = WidgetInternalId::new();
    let owner = tree.add_child(
        root,
        Box::new(RetainedSdfImageOwner {
            id: material_id,
            binding: binding.clone(),
            requested: requested.clone(),
            published: true,
        }),
        None,
        0,
    );
    let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 0);
    let owner_id = tree.node(owner).unwrap().id;
    let peer_id = tree.node(peer).unwrap().id;
    let root_id = tree.node(root).unwrap().id;
    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let extent = Size::new(320.0, 80.0);
    let bounds = RectF::new(0.0, 0.0, extent.width, extent.height);
    let mut compositor = Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (320, 80), 1.0)
        .unwrap();
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("same retained coverage observation target"),
        size: wgpu::Extent3d {
            width: 320,
            height: 80,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let first = harness.frame(
        &mut tree,
        root,
        Constraints::tight(extent),
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    let owner_before = harness.local_fragment(owner_id).unwrap();
    let peer_before = harness.local_fragment(peer_id).unwrap();
    let root_before = harness.local_fragment(root_id).unwrap();
    let actual_key = owner_before
        .iter()
        .find_map(|node| match node {
            SceneNode::Image(image) => {
                assert_eq!(
                    image.texture_source,
                    ImageTextureSource::Admitted(binding.identity())
                );
                image.key
            }
            _ => None,
        })
        .expect("widget-owned keyed admitted Image");
    assert_eq!(actual_key, SceneNodeKey::new(material_id.0));
    let face = tree.get_absolute_paint_rect(owner).unwrap();
    let peer_rect = tree.get_absolute_paint_rect(peer).unwrap();
    let epochs = [root, owner, peer].map(|node| tree.node(node).unwrap().layout_epoch);
    assert_eq!((face.width, face.height), (120.0, 30.0));
    assert_eq!(harness.image_dependency_counts(), Some((1, 1)));
    let mut readings = Vec::new();
    for phase in 0..2 {
        let frame = if phase == 0 {
            first.scene_root.clone()
        } else {
            requested.set(false);
            let second = harness.frame(
                &mut tree,
                root,
                Constraints::tight(extent),
                &theme,
                renderer.text_engine_mut(),
                &icons,
            );
            assert!(second.scene_built && second.visual_changed);
            assert!(!Arc::ptr_eq(
                &owner_before,
                &harness.local_fragment(owner_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &peer_before,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_before,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert!(harness.local_fragment(owner_id).unwrap().iter().all(
                |node| !matches!(node, SceneNode::Image(image) if image.key == Some(actual_key))
            ));
            assert_eq!(harness.image_dependency_counts(), None);
            assert_eq!(tree.get_absolute_paint_rect(owner), Some(face));
            assert_eq!(tree.get_absolute_paint_rect(peer), Some(peer_rect));
            assert_eq!(
                [root, owner, peer].map(|node| tree.node(node).unwrap().layout_epoch),
                epochs
            );
            binding
                .availability()
                .expect("removing a consumer does not revoke the producer");
            second.scene_root
        };
        let mut scene = SceneGraph::new();
        scene.root = frame;
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                phase,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("retained Image scene compiles");
        let command = renderer
            .encode_compiled_target_for_test(
                &compiled,
                surface,
                &output.create_view(&Default::default()),
            )
            .expect("retained Image scene encodes");
        renderer.submit_compiled_target_for_test(command);
        let path = directory.join(if phase == 0 {
            "4e-retained-before-1x.ppm"
        } else {
            "4e-retained-after-1x.ppm"
        });
        write_readback(gpu, &output, 320, 80, &path);
        let bytes = std::fs::read(path).unwrap();
        readings.push(bytes[b"P6\n320 80\n255\n".len()..].to_vec());
    }
    let background = &readings[1][..3];
    let mut cleared = 0;
    for y in 0..80 {
        for x in 0..320 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let old_coverage = px >= face.x - 4.4
                && px < face.x + face.width + 4.4
                && py >= face.y - 3.3
                && py < face.y + face.height + 5.5;
            let inside_face = px >= face.x
                && px < face.x + face.width
                && py >= face.y
                && py < face.y + face.height;
            let offset = (y * 320 + x) * 3;
            if old_coverage && !inside_face {
                assert_eq!(
                    &readings[1][offset..offset + 3],
                    background,
                    "old E outside F must clear"
                );
                cleared +=
                    usize::from(readings[0][offset..offset + 3] != readings[1][offset..offset + 3]);
            }
            if px >= peer_rect.x
                && px < peer_rect.x + peer_rect.width
                && py >= peer_rect.y
                && py < peer_rect.y + peer_rect.height
            {
                assert_eq!(
                    &readings[0][offset..offset + 3],
                    &readings[1][offset..offset + 3],
                    "retained peer pixels stay unchanged"
                );
            }
        }
    }
    assert!(
        cleared > 0,
        "first frame must contain visible coverage outside F"
    );
    println!(
        "retained removal cleared {cleared} former E pixels outside F; peer retained; Image dependencies retired"
    );
}

#[test]
fn button_4e_sdf_prototype_renders_rest() {
    let module = naga::front::wgsl::parse_str(SHADER).expect("experimental WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("experimental WGSL validates");
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("GPU unavailable: no preview may be claimed");
    let gpu = renderer.gpu_resources();
    assert_eq!(gpu.format, wgpu::TextureFormat::Bgra8UnormSrgb);
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("JC isolated 4E SDF experiment"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
    let bindings = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("4E private material parameters"),
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
            label: Some("4E private pipeline layout"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
    let shadow_pipeline = pipeline(&gpu, &shader, &layout, "fs_shadow");
    let face_pipeline = pipeline(&gpu, &shader, &layout, "fs_face");
    let error = pollster::block_on(validation_scope.pop());
    assert!(
        error.is_none(),
        "experimental shader GPU validation: {error:?}"
    );
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/4e-sdf-prototype");
    std::fs::create_dir_all(&directory).unwrap();

    for (index, (dark, scale, canonical_pair)) in [
        (false, 1_u32, false),
        (false, 2, false),
        (true, 1, false),
        (true, 2, false),
        (false, 1, true),
        (false, 2, true),
        (true, 1, true),
        (true, 2, true),
    ]
    .into_iter()
    .enumerate()
    {
        let width = LOGICAL_WIDTH * scale;
        let height = LOGICAL_HEIGHT * scale;
        let preview_width = width * if canonical_pair { 2 } else { 1 };
        let material = Material::resting(dark, scale as f32);
        material.validate().expect("declared material parameters");
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("4E immutable resting parameters"),
                contents: bytemuck::bytes_of(&material),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("4E material binding"),
            layout: &bindings,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let mut producer = CustomRenderPass::new(
            4_202_610_250 + index as u64,
            gpu.clone(),
            PhysicalRect::new(0, 0, width, height),
        )
        .expect("existing isolated offscreen producer");
        let mut frame = producer.begin_frame(Some("4E isolated producer")).unwrap();
        {
            let (encoder, view) = frame.encoder_and_view();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("4E shadow below face"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_scissor_rect(0, 0, width, height);
            pass.set_bind_group(0, &group, &[]);
            for (_, pipeline) in [(SHADOW_Z, &shadow_pipeline), (FACE_Z, &face_pipeline)] {
                pass.set_pipeline(pipeline);
                pass.draw(0..3, 0..1);
            }
        }
        producer.submit_and_swap(frame).unwrap();
        producer.register_front_now(&mut renderer).unwrap();

        let rect = RectF::new(
            0.0,
            0.0,
            preview_width as f32 / scale as f32,
            LOGICAL_HEIGHT as f32,
        );
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (preview_width, height), scale as f32)
            .expect("explicit headless proof target");
        let binding = canonical_pair.then(|| {
            renderer
                .admit_image(producer.texture_id())
                .expect("current experimental producer generation")
        });
        let mut scene = SceneBuilder::with_scale_factor(scale as f32);
        scene.set_clear_color(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 }).into());
        scene.begin_clip(rect, None);
        let text = renderer.text_engine_mut();
        text.set_scale(scale as f32);
        let run = text.shape("Stage", 13.0);
        let metrics = text.font_metrics(13.0);
        for member in 0..if canonical_pair { 2 } else { 1 } {
            // Explicit scene geometry only: these are not WidgetTree Button owners.
            let face = RectF::new(20.0 + member as f32 * 160.0, 15.0, 120.0, 30.0);
            if let Some(binding) = &binding {
                let (left, right, top, bottom) = if scale == 1 {
                    (4.4, 4.4, 3.3, 5.5)
                } else {
                    (4.15, 4.15, 3.05, 5.25)
                };
                let coverage = RectF::new(
                    face.x - left,
                    face.y - top,
                    face.width + left + right,
                    face.height + top + bottom,
                );
                // Matching source/destination logical spans avoid scaling the face.
                let uv = RectF::new(
                    (20.0 - left) / LOGICAL_WIDTH as f32,
                    (15.0 - top) / LOGICAL_HEIGHT as f32,
                    coverage.width / LOGICAL_WIDTH as f32,
                    coverage.height / LOGICAL_HEIGHT as f32,
                );
                let parent_clip = if member == 0 {
                    RectF::new(8.0, 8.0, 144.0, 44.0)
                } else {
                    RectF::new(184.0, 12.0, 112.0, 36.0)
                };
                scene.begin_clip(parent_clip, None);
                scene.push_admitted_image(
                    coverage,
                    binding,
                    uv,
                    crate::core::scene::Color::WHITE,
                    LAYER_CONTENT,
                    None,
                );
                println!("canonical face {face:?}, material {coverage:?}, clip {parent_clip:?}");
            } else {
                scene.push_image(
                    rect,
                    producer.texture_id(),
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                    crate::core::scene::Color::WHITE,
                    LAYER_CONTENT,
                    None,
                );
            }
            let baseline = face.y
                + crate::core::text_engine::compute_control_text_baseline_y(
                    face.height,
                    metrics.ascent,
                    metrics.descent,
                    -1.0,
                );
            text.draw(
                &run,
                face.x + face.width * 0.5 - run.width * 0.5,
                baseline - metrics.ascent,
                Color::from_hex(if dark { 0xEAEAEA } else { 0x1A1A1A }),
                LAYER_CONTENT + 2,
                &mut scene,
            );
            if canonical_pair {
                scene.end_clip();
            }
        }
        scene.end_clip();
        let scenes = HashMap::from([(surface, scene.build())]);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale as f32,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("complete Image composition");
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("4E production Image composition readback"),
            size: wgpu::Extent3d {
                width: preview_width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let command = renderer.encode_compiled_target_for_test(
            &compiled,
            surface,
            &output.create_view(&Default::default()),
        );
        let Some(command) = command else {
            let text_outcome = compiled
                .text_realization(surface)
                .map(|value| format!("{:?}", value.outcome()));
            producer.destroy(&mut renderer);
            panic!("production Image composition unavailable: {text_outcome:?}");
        };
        renderer.submit_compiled_target_for_test(command);
        let path = directory.join(format!(
            "4e-{}{}-{scale}x.ppm",
            if canonical_pair { "canonical-" } else { "" },
            if dark { "dark" } else { "light" }
        ));
        write_readback(&gpu, &output, preview_width, height, &path);
        let retained_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if canonical_pair && !dark && scale == 1 {
                retained_coverage_replacement(
                    &mut renderer,
                    &gpu,
                    binding.as_ref().unwrap().clone(),
                    &directory,
                );
            }
        }));
        producer.destroy(&mut renderer);
        if let Err(failure) = retained_result {
            std::panic::resume_unwind(failure);
        }
        println!("actual GPU preview: {}", path.display());
    }
}

#[test]
fn button_4e_sdf_generation_custody_waits_for_sampling_completion() {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use crate::renderer::compiled_draw_plan::MultiTargetDrawPlan;
    use crate::renderer::image_binding::ImageBindingFailure;

    // Test-only custody of one exact generation, not an engine integration.
    struct Generation {
        producer: RefCell<Option<CustomRenderPass>>,
        binding: ImageBinding,
        cpu_uses: Cell<usize>,
        submitted: RefCell<Vec<Arc<AtomicBool>>>,
        retirement_requested: Cell<bool>,
    }

    impl Generation {
        fn acquire(self: &Rc<Self>) -> CpuUse {
            assert!(!self.retirement_requested.get());
            self.cpu_uses.set(self.cpu_uses.get() + 1);
            CpuUse(self.clone())
        }

        fn try_retire(&self, renderer: &mut WgpuRenderer) -> bool {
            if !self.retirement_requested.get()
                || self.cpu_uses.get() != 0
                || self
                    .submitted
                    .borrow()
                    .iter()
                    .any(|done| !done.load(Ordering::Acquire))
            {
                return false;
            }
            let Some(producer) = self.producer.borrow_mut().take() else {
                return false;
            };
            producer.destroy(renderer);
            true
        }
    }

    struct CpuUse(Rc<Generation>);

    impl Drop for CpuUse {
        fn drop(&mut self) {
            self.0.cpu_uses.set(self.0.cpu_uses.get() - 1);
        }
    }

    struct SamplingFrame {
        compiled: MultiTargetDrawPlan,
        cpu: CpuUse,
    }

    impl SamplingFrame {
        fn submit(
            self,
            renderer: &WgpuRenderer,
            gpu: &GpuResources,
            surface: crate::compositor::SurfaceId,
            output: &wgpu::TextureView,
        ) -> Arc<AtomicBool> {
            let Self { compiled, cpu } = self;
            let command = renderer
                .encode_compiled_target_for_test(&compiled, surface, output)
                .expect("held admitted Image plan encodes");
            drop(compiled);
            let done = Arc::new(AtomicBool::new(false));
            cpu.0.submitted.borrow_mut().push(done.clone());
            renderer.submit_compiled_target_for_test(command);
            let callback = done.clone();
            gpu.queue.on_submitted_work_done(move || {
                callback.store(true, Ordering::Release);
            });
            // CPU custody spans emission, Compile, encoding and actual submit.
            drop(cpu);
            done
        }
    }

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("GPU unavailable: no custody proof may be claimed");
    let gpu = renderer.gpu_resources();
    let mut producer = CustomRenderPass::new(
        4_202_610_299,
        gpu.clone(),
        PhysicalRect::new(0, 0, LOGICAL_WIDTH, LOGICAL_HEIGHT),
    )
    .expect("isolated source generation");
    let mut source = producer.begin_frame(Some("custody source")).unwrap();
    {
        let (encoder, view) = source.encoder_and_view();
        // Source color is immaterial to this lifetime proof; the Image is real.
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("initialize custody source"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }
    producer.submit_and_swap(source).unwrap();
    producer.register_front_now(&mut renderer).unwrap();
    let binding = renderer.admit_image(producer.texture_id()).unwrap();
    let identity = binding.identity();
    let generation = Rc::new(Generation {
        producer: RefCell::new(Some(producer)),
        binding,
        cpu_uses: Cell::new(0),
        submitted: RefCell::new(Vec::new()),
        retirement_requested: Cell::new(false),
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let rect = RectF::new(0.0, 0.0, LOGICAL_WIDTH as f32, LOGICAL_HEIGHT as f32);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (LOGICAL_WIDTH, LOGICAL_HEIGHT), 1.0)
            .unwrap();
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("generation custody sampling target"),
            size: wgpu::Extent3d {
                width: LOGICAL_WIDTH,
                height: LOGICAL_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let cpu = generation.acquire();
        let mut scene = SceneBuilder::with_scale_factor(1.0);
        scene.push_admitted_image(
            rect,
            &cpu.0.binding,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            crate::core::scene::Color::WHITE,
            LAYER_CONTENT,
            None,
        );
        let scenes = HashMap::from([(surface, scene.build())]);
        assert!(scenes[&surface].root.iter().any(|node| matches!(
            node,
            SceneNode::Image(image)
                if image.texture_source == ImageTextureSource::Admitted(identity)
        )));
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        drop(plan);
        drop(scenes);
        let frame = SamplingFrame { compiled, cpu };

        generation.retirement_requested.set(true);
        assert_eq!(generation.cpu_uses.get(), 1);
        assert!(!generation.try_retire(&mut renderer));
        assert!(generation.producer.borrow().is_some());
        assert_eq!(generation.binding.identity(), identity);
        assert_eq!(generation.binding.availability(), Ok(()));
        println!("CPU compiled custody blocks retirement for {identity:?}");

        let completed = frame.submit(
            &renderer,
            &gpu,
            surface,
            &output.create_view(&Default::default()),
        );
        assert_eq!(generation.cpu_uses.get(), 0);
        let pending_at_observation = !completed.load(Ordering::Acquire);
        if pending_at_observation {
            assert!(!generation.try_retire(&mut renderer));
            assert_eq!(generation.binding.availability(), Ok(()));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut polls = 0;
        while !completed.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline, "sampling completion timed out");
            gpu.device
                .poll(wgpu::PollType::Poll)
                .expect("nonblocking sampling completion poll");
            polls += 1;
            std::thread::yield_now();
        }
        assert_eq!(generation.binding.availability(), Ok(()));
        assert!(generation.try_retire(&mut renderer));
        assert!(generation.producer.borrow().is_none());
        assert_eq!(
            generation.binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        println!(
            "post-submit callback observed after {polls} nonblocking polls; \
             pending at first observation: {pending_at_observation}; \
             exact generation unregistered before producer drop"
        );
    }));

    if generation.producer.borrow().is_some() {
        // On a failed assertion, unwind drops the CPU frame. Certify cleanup
        // against the same queue instead of triggering a second Drop panic.
        let done = Arc::new(AtomicBool::new(false));
        let callback = done.clone();
        gpu.queue.on_submitted_work_done(move || {
            callback.store(true, Ordering::Release);
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done.load(Ordering::Acquire) && Instant::now() < deadline {
            if gpu.device.poll(wgpu::PollType::Poll).is_err() {
                break;
            }
            std::thread::yield_now();
        }
        generation.retirement_requested.set(true);
        if !done.load(Ordering::Acquire) || !generation.try_retire(&mut renderer) {
            // A failing test must not fabricate a retirement certificate.
            if let Some(producer) = generation.producer.borrow_mut().take() {
                std::mem::forget(producer);
            }
            panic!("custody proof failed; uncertified producer retained without retirement");
        }
    }
    if let Err(failure) = result {
        std::panic::resume_unwind(failure);
    }
}
