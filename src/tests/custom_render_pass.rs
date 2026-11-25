use crate::compositor::{Compositor, SceneKey, SurfaceId};
#[cfg(debug_assertions)]
use crate::core::math::Point;
use crate::core::math::RectF;
use crate::core::scene::{Color, SceneBuilder};
use crate::renderer::batcher::PipelineId;
use crate::renderer::custom_render_pass::{
    CustomRenderFrame, CustomRenderPass, PhysicalRect, audit,
};
use crate::renderer::draw_plan::DrawPlan;
use crate::renderer::draw_plan_builder::DrawPlanBuilder;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::{GpuResources, WgpuRenderer};
use crate::three_d::viewport_audit;
use std::collections::HashMap;
use std::sync::{
    Arc, Barrier, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

const W: u32 = 128;
const H: u32 = 96;
const TEX_ID: u64 = 295_001;

#[test]
fn physical_rect_legacy_receiver_compatibility() {
    let size: fn(&PhysicalRect) -> (u32, u32) = PhysicalRect::size;
    let is_empty: fn(&PhysicalRect) -> bool = PhysicalRect::is_empty;
    let rect = PhysicalRect::new(1, 2, 3, 4);

    assert_eq!(size(&rect), (3, 4));
    assert!(!is_empty(&rect));
}

fn setup_renderer() -> Option<WgpuRenderer> {
    pollster::block_on(WgpuRenderer::new_headless())
}

struct HdrRetirementDropProbe(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for HdrRetirementDropProbe {
    fn drop(&mut self) {
        assert_eq!(
            self.0.fetch_sub(1, Ordering::AcqRel),
            1,
            "one exact final custody release"
        );
    }
}

fn private_hdr_pass(gpu: &GpuResources) -> (CustomRenderPass, Arc<std::sync::atomic::AtomicUsize>) {
    let alive = Arc::new(std::sync::atomic::AtomicUsize::new(1));
    let pass = CustomRenderPass::new_private_hdr_final(
        crate::utils::id::WidgetInternalId::new().raw(),
        gpu.clone(),
        PhysicalRect::new(0, 0, 32, 32),
        Box::new(HdrRetirementDropProbe(alive.clone())),
    )
    .unwrap();
    (pass, alive)
}

fn publish_private_hdr_clear(pass: &mut CustomRenderPass) {
    let mut frame = pass
        .begin_frame(Some("R027 custody actual writer"))
        .unwrap();
    {
        let (encoder, view) = frame.encoder_and_view();
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("premultiplied clear final for lifecycle owner proof"),
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
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
    }
    pass.submit_and_swap(frame).unwrap();
}

#[test]
fn hdr_final_custody_requires_both_actual_removal_and_queue_completion_in_both_orders() {
    let mut renderer = setup_renderer().expect("UNVERIFIED R027: actual Device required");
    let gpu = renderer.gpu_resources();
    for complete_first in [false, true] {
        let (mut pass, alive) = private_hdr_pass(&gpu);
        let id = pass.texture_id();
        assert_eq!(pass.texture_format(), wgpu::TextureFormat::Rgba16Float);
        publish_private_hdr_clear(&mut pass);
        renderer.drain_pending_image_registrations();
        assert!(renderer.admit_image(id).is_ok());
        pass.destroy_deferred().unwrap();
        if complete_first {
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                alive.load(Ordering::Acquire),
                1,
                "completion cannot drop pending-removal custody"
            );
            assert!(renderer.admit_image(id).is_ok(), "removal has not run yet");
        }
        renderer.drain_pending_image_registrations();
        assert!(
            renderer.admit_image(id).is_err(),
            "registry removal must precede final drop"
        );
        if !complete_first {
            assert_eq!(
                alive.load(Ordering::Acquire),
                1,
                "drain alone cannot release unobserved GPU completion"
            );
        }
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(alive.load(Ordering::Acquire), 0);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(alive.load(Ordering::Acquire), 0, "no repeated release");
    }
    let legacy =
        CustomRenderPass::new(TEX_ID + 270, gpu.clone(), PhysicalRect::new(0, 0, 32, 32)).unwrap();
    assert_eq!(legacy.texture_format(), gpu.format);
    legacy.destroy_deferred().unwrap();
    renderer.drain_pending_image_registrations();
}

#[test]
fn hdr_private_custody_rejects_duplicate_and_protected_removal_without_losing_claims() {
    let mut renderer = setup_renderer().expect("UNVERIFIED R027: actual Device required");
    let gpu = renderer.gpu_resources();
    let (mut a, alive_a) = private_hdr_pass(&gpu);
    let id = a.texture_id();
    let (mut b, alive_b) = private_hdr_pass(&gpu);
    assert_ne!(id, b.texture_id());
    publish_private_hdr_clear(&mut a);
    publish_private_hdr_clear(&mut b);
    renderer.drain_pending_image_registrations();
    a.destroy_deferred().unwrap();
    b.destroy_deferred().unwrap();
    renderer.set_hdr_removal_protected_for_test(id, true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || renderer.drain_pending_image_registrations()
        ))
        .is_err()
    );
    assert_eq!(gpu.pending_unregistrations.lock().unwrap().len(), 2);
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(alive_a.load(Ordering::Acquire), 1);
    assert_eq!(alive_b.load(Ordering::Acquire), 1);
    renderer.set_hdr_removal_protected_for_test(id, false);
    gpu.pending_unregistrations.lock().unwrap().push(
        crate::renderer::wgpu_renderer::PendingImageUnregistration {
            texture_id: id,
            retained_textures: vec![],
        },
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || renderer.drain_pending_image_registrations()
        ))
        .is_err()
    );
    assert_eq!(gpu.pending_unregistrations.lock().unwrap().len(), 3);
    gpu.pending_unregistrations.lock().unwrap().pop();
    // A bare-ID replacement cannot steal the exact paired texture custody.
    let retained = {
        let mut entries = gpu.pending_unregistrations.lock().unwrap();
        std::mem::take(
            &mut entries
                .iter_mut()
                .find(|entry| entry.texture_id == id)
                .unwrap()
                .retained_textures,
        )
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || renderer.drain_pending_image_registrations()
        ))
        .is_err()
    );
    assert_eq!(alive_a.load(Ordering::Acquire), 1);
    gpu.pending_unregistrations
        .lock()
        .unwrap()
        .iter_mut()
        .find(|entry| entry.texture_id == id)
        .unwrap()
        .retained_textures = retained;
    assert!(renderer.admit_image(id).is_ok());
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(alive_a.load(Ordering::Acquire), 0);
    assert_eq!(alive_b.load(Ordering::Acquire), 0);
}

#[test]
fn hdr_renderer_shutdown_retires_registry_and_custody_without_a_queue_cycle_or_leak() {
    for queued in [false, true] {
        let mut renderer = setup_renderer().expect("UNVERIFIED R027: actual Device required");
        let gpu = renderer.gpu_resources();
        let (mut pass, alive) = private_hdr_pass(&gpu);
        publish_private_hdr_clear(&mut pass);
        renderer.drain_pending_image_registrations();
        if queued {
            pass.destroy_deferred().unwrap();
            drop(renderer);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(alive.load(Ordering::Acquire), 0);
        } else {
            drop(renderer);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                alive.load(Ordering::Acquire),
                1,
                "live pass still owns final handles"
            );
            assert!(
                pass.begin_frame(None).is_err(),
                "closed renderer cannot accept another HDR write"
            );
            pass.destroy_deferred().unwrap();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(alive.load(Ordering::Acquire), 0);
        }
        assert!(gpu.pending_unregistrations.lock().unwrap().is_empty());
    }
}

#[test]
fn closed_renderer_rejects_first_hdr_custody_without_allocating_a_final_pair() {
    let renderer = setup_renderer().expect("UNVERIFIED R027: actual Device required");
    let gpu = renderer.gpu_resources();
    drop(renderer);
    let alive = Arc::new(std::sync::atomic::AtomicUsize::new(1));
    let before = audit::snapshot();
    assert!(
        CustomRenderPass::new_private_hdr_final(
            crate::utils::id::WidgetInternalId::new().raw(),
            gpu.clone(),
            PhysicalRect::new(0, 0, 32, 32),
            Box::new(HdrRetirementDropProbe(alive.clone()))
        )
        .is_err()
    );
    assert_eq!(alive.load(Ordering::Acquire), 0);
    assert_eq!(audit::snapshot(), before);
    assert!(gpu.pending_unregistrations.lock().unwrap().is_empty());
}

fn build_image_scene(texture_id: u64) -> crate::core::scene::SceneGraph {
    let mut builder = SceneBuilder::new();
    builder.push_image(
        RectF::new(16.0, 12.0, 64.0, 48.0),
        texture_id,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        4,
        None,
    );
    builder.build()
}

fn build_draw_plan(
    renderer: &mut WgpuRenderer,
    scene: crate::core::scene::SceneGraph,
) -> (SurfaceId, DrawPlan<crate::compositor::MultiSurfaceContract>) {
    let viewport = RectF::new(0.0, 0.0, W as f32, H as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id = compositor.create_surface(0, viewport, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface_id, (W, H), 1.0)
        .expect("register_headless_render_target failed");

    let mut surface_scenes = HashMap::new();
    surface_scenes.insert(surface_id, scene);

    let mut draw_plan_builder = DrawPlanBuilder::new();
    let draw_plan = draw_plan_builder
        .build_multi(
            &surface_scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");

    (surface_id, draw_plan)
}

fn read_front_pixel(pass: &CustomRenderPass, gpu: &GpuResources) -> [u8; 4] {
    let (width, height) = pass.physical_size();
    let unpadded_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = padded_bytes_per_row as u64 * height as u64;

    let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("custom render pass readback buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("custom render pass readback encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: pass
                .front_texture_for_test()
                .expect("front texture missing"),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit(std::iter::once(encoder.finish()));

    let done = Arc::new(AtomicBool::new(false));
    let succeeded = Arc::new(AtomicBool::new(false));
    let done_for_callback = done.clone();
    let succeeded_for_callback = succeeded.clone();
    let slice = output.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        succeeded_for_callback.store(result.is_ok(), Ordering::Release);
        done_for_callback.store(true, Ordering::Release);
    });
    while !done.load(Ordering::Acquire) {
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
    assert!(succeeded.load(Ordering::Acquire));

    let data = slice.get_mapped_range().expect("mapped readback range");
    let pixel = [data[0], data[1], data[2], data[3]];
    drop(data);
    output.unmap();
    pixel
}

fn clear_back_texture(pass: &CustomRenderPass, color: wgpu::Color) -> CustomRenderFrame {
    let mut frame = pass
        .begin_frame(Some("custom render ordering test encoder"))
        .expect("begin_frame failed");
    {
        let (encoder, view) = frame.encoder_and_view();
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom render ordering test clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }
    frame
}

fn sample_front_pixel_on_same_queue(pass: &CustomRenderPass, gpu: &GpuResources) -> [u8; 4] {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("custom render same-queue sampling shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    return out;
}

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return textureLoad(source_texture, vec2<i32>(0, 0), 0);
}
"#
                .into(),
            ),
        });
    let bind_group_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("custom render same-queue sampling BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("custom render same-queue sampling pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("custom render same-queue sampling pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    let front_view = pass
        .front_texture_for_test()
        .expect("front texture missing")
        .create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("custom render same-queue sampling bind group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&front_view),
        }],
    });
    let sampled_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("custom render same-queue sampled output"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let sampled_view = sampled_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("custom render same-queue sampled readback"),
        size: wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("custom render same-queue sampling encoder"),
        });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom render same-queue sampling pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: &sampled_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &sampled_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    gpu.queue.submit(std::iter::once(encoder.finish()));

    let done = Arc::new(AtomicBool::new(false));
    let succeeded = Arc::new(AtomicBool::new(false));
    let done_for_callback = done.clone();
    let succeeded_for_callback = succeeded.clone();
    let slice = output.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        succeeded_for_callback.store(result.is_ok(), Ordering::Release);
        done_for_callback.store(true, Ordering::Release);
    });
    while !done.load(Ordering::Acquire) {
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
    assert!(succeeded.load(Ordering::Acquire));

    let data = slice.get_mapped_range().expect("mapped readback range");
    let pixel = [data[0], data[1], data[2], data[3]];
    drop(data);
    output.unmap();
    pixel
}

fn assert_logical_pixel(format: wgpu::TextureFormat, pixel: [u8; 4], expected_pixel: [u8; 4]) {
    let logical = match format {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            [pixel[2], pixel[1], pixel[0], pixel[3]]
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => pixel,
        other => panic!("unsupported custom-render test format: {other:?}"),
    };
    for (actual, expected_component) in logical.into_iter().zip(expected_pixel) {
        assert!(
            actual.abs_diff(expected_component) <= 1,
            "expected logical pixel {expected_pixel:?}, got {logical:?}"
        );
    }
}

#[derive(Clone, Copy)]
enum RegistrationBoundary {
    Direct,
    Deferred,
}

fn run_worker_publication_case(
    texture_id: u64,
    color: wgpu::Color,
    expected_pixel: [u8; 4],
    boundary: RegistrationBoundary,
) {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let pass = CustomRenderPass::new(texture_id, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let pass = Arc::new(Mutex::new(pass));
    let completion = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_pass = pass.clone();
    let worker_completion = completion.clone();
    let worker = std::thread::spawn(move || {
        {
            let mut pass = worker_pass.lock().unwrap_or_else(|e| e.into_inner());
            let frame = clear_back_texture(&pass, color);
            pass.submit_and_swap_deferred_registration(frame)
                .expect("worker submit-and-swap failed");
        }
        let (completed, wake) = &*worker_completion;
        let mut completed = completed.lock().unwrap_or_else(|e| e.into_inner());
        *completed = true;
        wake.notify_one();
    });

    let (completed, wake) = &*completion;
    let completed = completed.lock().unwrap_or_else(|e| e.into_inner());
    let (completed, timeout) = wake
        .wait_timeout_while(completed, Duration::from_secs(10), |completed| !*completed)
        .unwrap_or_else(|e| e.into_inner());
    assert!(*completed, "worker completion was not published");
    assert!(!timeout.timed_out(), "worker completion timed out");
    drop(completed);
    worker.join().expect("custom render worker panicked");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(
            registrations.is_empty(),
            "worker must not expose the front before the UI registration boundary"
        );
    }

    match boundary {
        RegistrationBoundary::Direct => {
            let pass = pass.lock().unwrap_or_else(|e| e.into_inner());
            pass.register_front_now(&mut renderer)
                .expect("direct front registration failed");
        }
        RegistrationBoundary::Deferred => {
            {
                let pass = pass.lock().unwrap_or_else(|e| e.into_inner());
                pass.queue_front_registration()
                    .expect("deferred front registration failed");
            }
            {
                let registrations = gpu
                    .pending_registrations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                assert_eq!(registrations.len(), 1);
                assert_eq!(registrations[0].texture_id, texture_id);
            }
            renderer.drain_pending_image_registrations();
        }
    }

    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(texture_id));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    let pixel = {
        let pass = pass.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(pass.swap_count(), 1);
        sample_front_pixel_on_same_queue(&pass, &gpu)
    };
    assert_logical_pixel(gpu.format, pixel, expected_pixel);

    let pass = match Arc::try_unwrap(pass) {
        Ok(pass) => pass.into_inner().unwrap_or_else(|e| e.into_inner()),
        Err(_) => panic!("custom render pass still shared after worker completion"),
    };
    pass.destroy(&mut renderer);
}

#[test]
fn worker_publication_orders_deferred_registration_before_same_queue_sampling() {
    run_worker_publication_case(
        TEX_ID + 20,
        wgpu::Color::GREEN,
        [0, 255, 0, 255],
        RegistrationBoundary::Deferred,
    );
}

#[test]
fn worker_publication_orders_direct_registration_before_same_queue_sampling() {
    run_worker_publication_case(
        TEX_ID + 21,
        wgpu::Color::BLUE,
        [0, 0, 255, 255],
        RegistrationBoundary::Direct,
    );
}

#[test]
fn resource_disjoint_producers_submit_concurrently_on_the_shared_queue() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let first = CustomRenderPass::new(TEX_ID + 22, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("first custom render pass init failed");
    let second = CustomRenderPass::new(TEX_ID + 23, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("second custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = barrier.clone();
    let first_worker = std::thread::spawn(move || {
        let mut pass = first;
        let frame = clear_back_texture(&pass, wgpu::Color::RED);
        first_barrier.wait();
        pass.submit_and_swap_deferred_registration(frame)
            .expect("first concurrent submit failed");
        pass
    });
    let second_barrier = barrier.clone();
    let second_worker = std::thread::spawn(move || {
        let mut pass = second;
        let frame = clear_back_texture(&pass, wgpu::Color::GREEN);
        second_barrier.wait();
        pass.submit_and_swap_deferred_registration(frame)
            .expect("second concurrent submit failed");
        pass
    });
    barrier.wait();

    let first = first_worker.join().expect("first producer panicked");
    let second = second_worker.join().expect("second producer panicked");
    first
        .queue_front_registration()
        .expect("first publication failed");
    second
        .queue_front_registration()
        .expect("second publication failed");
    renderer.drain_pending_image_registrations();

    let mut builder = SceneBuilder::new();
    builder.push_image(
        RectF::new(0.0, 0.0, 32.0, 32.0),
        first.texture_id(),
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        1,
        None,
    );
    builder.push_image(
        RectF::new(40.0, 0.0, 32.0, 32.0),
        second.texture_id(),
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        1,
        None,
    );
    let (_, draw_plan) = build_draw_plan(&mut renderer, builder.build());
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    assert_logical_pixel(
        gpu.format,
        sample_front_pixel_on_same_queue(&first, &gpu),
        [255, 0, 0, 255],
    );
    assert_logical_pixel(
        gpu.format,
        sample_front_pixel_on_same_queue(&second, &gpu),
        [0, 255, 0, 255],
    );

    first.destroy(&mut renderer);
    second.destroy(&mut renderer);
}

#[test]
fn showcase_style_2d_scene_has_no_custom_render_activity() {
    let before = audit::snapshot();
    let three_d_before = viewport_audit::snapshot();
    assert_eq!(before, Default::default());
    assert_eq!(three_d_before, Default::default());
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();

    let mut builder = SceneBuilder::new();
    builder.push_rect(
        RectF::new(0.0, 0.0, W as f32, H as f32),
        Color::BLACK,
        0,
        None,
    );
    builder.push_rect(RectF::new(12.0, 10.0, 36.0, 28.0), Color::WHITE, 1, None);
    builder.push_rect(
        RectF::new(56.0, 10.0, 60.0, 28.0),
        Color::from_hex(0x2ECC71),
        1,
        None,
    );

    let (_, draw_plan) = build_draw_plan(&mut renderer, builder.build());
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed")
        .expect_ready("2D draw plan must compile");
    let pass_names: Vec<_> = compiled
        .render_graph_topology
        .passes_in_order()
        .map(|pass| pass.name)
        .collect();
    assert_eq!(pass_names, ["main_pass", "overlay_pass"]);
    assert_eq!(compiled.render_graph_topology.pass_count(), 2);
    assert!(
        gpu.pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    );
    assert!(
        gpu.pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    );
    assert_eq!(audit::snapshot(), before);
    assert_eq!(viewport_audit::snapshot(), three_d_before);
}

#[test]
fn custom_render_pass_registers_front_view_for_compile() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let pass = CustomRenderPass::new(
        TEX_ID,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 64, 48),
    )
    .expect("custom render pass init failed");

    renderer.drain_pending_image_registrations();
    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_register_front_now_discards_pending_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let pass = CustomRenderPass::new(TEX_ID + 11, gpu.clone(), PhysicalRect::new(0, 0, 64, 48))
        .expect("custom render pass init failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].texture_id, TEX_ID + 11);
    }

    pass.register_front_now(&mut renderer)
        .expect("register_front_now failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(registrations.is_empty());
    }

    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_renders_to_back_then_swaps_front() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let mut pass = CustomRenderPass::new(
        TEX_ID + 1,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 32, 32),
    )
    .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let mut frame = pass
        .begin_frame(Some("custom render pass test encoder"))
        .expect("begin_frame failed");
    {
        let (encoder, view) = frame.encoder_and_view();
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom render pass test clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 1.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    pass.submit_and_swap(frame).expect("submit_and_swap failed");
    assert_eq!(pass.swap_count(), 1);
    let pixel = read_front_pixel(&pass, &renderer.gpu_resources());
    match pass.texture_format() {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            assert!(pixel[1] > 200);
            assert!(pixel[0] < 20);
            assert!(pixel[2] < 20);
            assert!(pixel[3] > 200);
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            assert!(pixel[0] < 20);
            assert!(pixel[1] > 200);
            assert!(pixel[2] < 20);
            assert!(pixel[3] > 200);
        }
        _ => {
            assert!(pixel[3] > 200);
            assert!(pixel[0] > 200 || pixel[1] > 200 || pixel[2] > 200);
        }
    }

    renderer.drain_pending_image_registrations();
    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_accepts_custom_wgsl_pipeline() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let mut pass = CustomRenderPass::new(TEX_ID + 6, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("custom render pass wgsl test shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#
                .into(),
            ),
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("custom render pass wgsl test pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("custom render pass wgsl test pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: pass.texture_format(),
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    let mut frame = pass
        .begin_frame(Some("custom render pass wgsl test encoder"))
        .expect("begin_frame failed");
    {
        let (encoder, view) = frame.encoder_and_view();
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("custom render pass wgsl test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        render_pass.set_pipeline(&pipeline);
        render_pass.draw(0..3, 0..1);
    }

    pass.submit_and_swap(frame).expect("submit_and_swap failed");
    assert_eq!(pass.swap_count(), 1);
    let pixel = read_front_pixel(&pass, &gpu);
    match pass.texture_format() {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            assert!(pixel[2] > 200);
            assert!(pixel[0] < 20);
            assert!(pixel[1] < 20);
            assert!(pixel[3] > 200);
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            assert!(pixel[0] > 200);
            assert!(pixel[1] < 20);
            assert!(pixel[2] < 20);
            assert!(pixel[3] > 200);
        }
        _ => {
            assert!(pixel[3] > 200);
            assert!(pixel[0] > 200 || pixel[1] > 200 || pixel[2] > 200);
        }
    }

    renderer.drain_pending_image_registrations();
    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_resize_recreates_front_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let mut pass = CustomRenderPass::new(
        TEX_ID + 2,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 32, 32),
    )
    .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    pass.resize_registered(&mut renderer, PhysicalRect::new(0, 0, 80, 40))
        .expect("resize_registered failed");
    assert_eq!(pass.physical_size(), (80, 40));
    assert_eq!(pass.swap_count(), 0);

    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_resize_registered_discards_pending_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let mut pass = CustomRenderPass::new(TEX_ID + 12, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");

    pass.resize_registered(&mut renderer, PhysicalRect::new(0, 0, 80, 40))
        .expect("resize_registered failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(registrations.is_empty());
    }

    assert_eq!(pass.physical_size(), (80, 40));
    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_resize_retains_old_textures_until_unregister_drain() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let mut pass = CustomRenderPass::new(TEX_ID + 7, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    pass.resize(PhysicalRect::new(0, 0, 48, 24))
        .expect("resize failed");
    assert_eq!(pass.physical_size(), (48, 24));

    {
        let removals = gpu
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].texture_id, TEX_ID + 7);
        assert_eq!(removals[0].retained_textures.len(), 2);
    }

    renderer.drain_pending_image_registrations();
    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_repeated_resize_discards_stale_pending_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let mut pass = CustomRenderPass::new(TEX_ID + 9, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    pass.resize(PhysicalRect::new(0, 0, 48, 24))
        .expect("first resize failed");
    pass.resize(PhysicalRect::new(0, 0, 64, 32))
        .expect("second resize failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].texture_id, TEX_ID + 9);
    }
    {
        let removals = gpu
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(removals.len(), 2);
        assert_eq!(
            removals
                .iter()
                .map(|removal| removal.retained_textures.len())
                .sum::<usize>(),
            4
        );
    }

    renderer.drain_pending_image_registrations();
    assert_eq!(pass.physical_size(), (64, 32));
    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    assert!(compiled.is_ready());

    pass.destroy(&mut renderer);
}

#[test]
fn custom_render_pass_deferred_destroy_retains_textures_until_unregister_drain() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let pass = CustomRenderPass::new(TEX_ID + 8, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    pass.destroy_deferred()
        .expect("deferred destroy queue failed");

    {
        let removals = gpu
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].texture_id, TEX_ID + 8);
        assert_eq!(removals[0].retained_textures.len(), 2);
    }

    renderer.drain_pending_image_registrations();

    let removals = gpu
        .pending_unregistrations
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    assert!(removals.is_empty());
}

#[test]
fn custom_render_pass_deferred_destroy_discards_initial_pending_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let pass = CustomRenderPass::new(TEX_ID + 10, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].texture_id, TEX_ID + 10);
    }

    pass.destroy_deferred()
        .expect("deferred destroy queue failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(registrations.is_empty());
    }
    {
        let removals = gpu
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].texture_id, TEX_ID + 10);
        assert_eq!(removals[0].retained_textures.len(), 2);
    }

    renderer.drain_pending_image_registrations();

    let registrations = gpu
        .pending_registrations
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    assert!(registrations.is_empty());
}

#[test]
fn custom_render_pass_destroy_discards_initial_pending_registration() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let gpu = renderer.gpu_resources();
    let pass = CustomRenderPass::new(TEX_ID + 13, gpu.clone(), PhysicalRect::new(0, 0, 32, 32))
        .expect("custom render pass init failed");

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].texture_id, TEX_ID + 13);
    }

    pass.destroy(&mut renderer);

    {
        let registrations = gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert!(registrations.is_empty());
    }

    renderer.drain_pending_image_registrations();

    let registrations = gpu
        .pending_registrations
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    assert!(registrations.is_empty());
}

#[cfg(debug_assertions)]
#[test]
fn custom_render_pass_destroy_unregisters_before_drop() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let pass = CustomRenderPass::new(
        TEX_ID + 3,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 32, 32),
    )
    .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    pass.destroy(&mut renderer);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = renderer.compile_multi(&draw_plan);
    }));
    let panic_payload = result.expect_err("unregistered custom texture must panic in debug");
    let panic_msg = panic_payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(panic_msg.contains("INV-IMG-005"));
}

#[cfg(debug_assertions)]
#[test]
fn custom_render_pass_deferred_destroy_drains_before_compile() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let pass = CustomRenderPass::new(
        TEX_ID + 4,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 32, 32),
    )
    .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let (_, draw_plan) = build_draw_plan(&mut renderer, build_image_scene(pass.texture_id()));
    pass.destroy_deferred()
        .expect("deferred destroy queue failed");
    renderer.drain_pending_image_registrations();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = renderer.compile_multi(&draw_plan);
    }));
    let panic_payload = result.expect_err("deferred unregistered texture must panic in debug");
    let panic_msg = panic_payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(panic_msg.contains("INV-IMG-005"));
}

#[test]
fn custom_render_pass_image_batches_with_clip_z_order_and_surrounding_primitives() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };

    let pass = CustomRenderPass::new(
        TEX_ID + 5,
        renderer.gpu_resources(),
        PhysicalRect::new(0, 0, 64, 48),
    )
    .expect("custom render pass init failed");
    renderer.drain_pending_image_registrations();

    let image_rect = RectF::new(16.0, 12.0, 64.0, 48.0);
    let clip_rect = RectF::new(20.0, 16.0, 40.0, 24.0);

    let mut builder = SceneBuilder::new();
    builder.push_rect(
        RectF::new(0.0, 0.0, W as f32, H as f32),
        Color::BLACK,
        0,
        None,
    );
    builder.with_clip(clip_rect, None, |scene| {
        scene.push_image(
            image_rect,
            pass.texture_id(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            5,
            None,
        );
    });
    builder.push_rect(RectF::new(4.0, 4.0, 20.0, 10.0), Color::WHITE, 10, None);
    #[cfg(debug_assertions)]
    builder.push_text_probe(
        Point::new(8.0, 84.0),
        "axis".to_string(),
        12.0,
        Color::WHITE,
        8,
        None,
    );

    let (surface_id, draw_plan) = build_draw_plan(&mut renderer, builder.build());
    let surface = draw_plan
        .surface_scene(surface_id)
        .expect("surface scene missing");

    let image_batch = surface
        .batches
        .iter()
        .find(|batch| batch.key.pipeline == PipelineId::Image)
        .expect("image batch missing");
    let image_texture_id = image_batch.key.texture_id;
    let image_z_layer = image_batch.key.z_layer;
    let image_clip_rect = image_batch.clip_state.rect;
    let image_clip_hash = image_batch.clip_state.hash;
    let has_background_rect = surface
        .batches
        .iter()
        .any(|batch| batch.key.pipeline == PipelineId::Rect && batch.key.z_layer == 0);
    let has_foreground_rect = surface
        .batches
        .iter()
        .any(|batch| batch.key.pipeline == PipelineId::Rect && batch.key.z_layer == 10);

    let compiled = renderer
        .compile_multi(&draw_plan)
        .expect("compile_multi failed");
    let compiled_some = compiled.is_ready();
    #[cfg(debug_assertions)]
    let has_text = matches!(&compiled, crate::renderer::RendererCompileOutcome::Ready(plan) if plan.has_text());

    pass.destroy(&mut renderer);

    assert_eq!(image_texture_id, Some(TEX_ID + 5));
    assert_eq!(image_z_layer, 5);
    assert_eq!(image_clip_rect, clip_rect);
    assert!(image_clip_hash != 0);
    assert!(has_background_rect);
    assert!(has_foreground_rect);
    assert!(compiled_some);
    #[cfg(debug_assertions)]
    assert!(has_text);
}
