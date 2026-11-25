use crate::core::app::AppContext;
use crate::core::theme::Theme;
use crate::renderer::custom_render_pass::{CustomRenderPass, PhysicalRect};
use crate::renderer::image_binding::{
    ImageAdmissionError, ImageBindingFailure, ImageBindingSampling,
};
use crate::renderer::wgpu_renderer::{
    GpuResources, ImageRgba8RegistrationError, PendingImageRegistration, WgpuRenderer,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const BLUE: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
const BLACK: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const WHITE_TINT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn setup_renderer() -> Option<WgpuRenderer> {
    pollster::block_on(WgpuRenderer::new_headless())
}

fn create_sampled_texture(
    gpu: &GpuResources,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    bytes: &[u8],
    bytes_per_pixel: u32,
) -> wgpu::Texture {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SPEC-HAL-263A sampled texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_pixel),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn render(
    renderer: &WgpuRenderer,
    texture_id: u64,
    uv: [f32; 2],
    tint: [f32; 4],
    background: wgpu::Color,
) -> [u8; 4] {
    renderer.render_registered_image_pixel_for_test(texture_id, uv, tint, background)
}

#[test]
fn nine_patch_production_image_pipeline_shows_stretched_and_collapsed_source_regions() {
    use crate::core::math::RectF;
    use crate::core::paint::{
        Color, NinePatchDestinationInsets, NinePatchSourceInsets, PaintSink, SurfaceFill,
        SurfaceNinePatch,
    };
    use crate::core::scene::{ImageTextureSource, SceneBuilder, SceneNode};

    const IMAGE: u64 = 19_001;
    const COLORS: [[u8; 4]; 9] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
        [0, 255, 255, 255],
        [255, 255, 255, 255],
        [128, 128, 128, 255],
        [255, 128, 0, 255],
    ];
    let mut renderer =
        setup_renderer().expect("headless adapter required for NinePatch visibility");
    let mut pixels = Vec::with_capacity(9 * 9 * 4);
    for row in 0..9 {
        for column in 0..9 {
            pixels.extend_from_slice(&COLORS[(row / 3) * 3 + column / 3]);
        }
    }
    renderer.register_image_rgba8_srgb(IMAGE, 9, 9, &pixels);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let material = SurfaceNinePatch::new(
        binding.clone(),
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::all(1.0 / 3.0),
        NinePatchDestinationInsets::new(4.0, 11.0, 7.0, 8.0),
        Color::from_srgb(1.0, 1.0, 1.0, 1.0),
    )
    .unwrap();
    let surviving_corners = [COLORS[0], COLORS[2], COLORS[6], COLORS[8]];
    for (destination, expected) in [
        (RectF::new(12.0, 7.0, 48.0, 36.0), COLORS.as_slice()),
        (RectF::new(2.0, 3.0, 9.0, 5.0), surviving_corners.as_slice()),
    ] {
        let mut builder = SceneBuilder::new();
        builder.push_admitted_surface_fill(
            destination,
            &SurfaceFill::NinePatch(material.clone()),
            7,
            None,
        );
        let scene = builder.build();
        assert_eq!(scene.root.len(), expected.len());
        for (node, expected_color) in scene.root.iter().zip(expected) {
            let SceneNode::Image(image) = node else {
                panic!("NinePatch must lower to the existing Image primitive");
            };
            assert_eq!(
                image.texture_source,
                ImageTextureSource::Admitted(binding.identity())
            );
            assert!(image.rect.width > 0.0 && image.rect.height > 0.0);
            let uv = [
                image.uv_rect.x + image.uv_rect.width * 0.5,
                image.uv_rect.y + image.uv_rect.height * 0.5,
            ];
            assert_eq!(
                render(
                    &renderer,
                    IMAGE,
                    uv,
                    [image.tint.r, image.tint.g, image.tint.b, image.tint.a],
                    BLACK
                ),
                *expected_color,
                "production Image sample for lowered cell {:?}",
                image.rect,
            );
        }
    }
}

#[test]
fn production_image_pipeline_applies_texture_alpha_once() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let half_red_half_alpha = [0x3800u16, 0, 0, 0x3800];
    let texture = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba16Float,
        bytemuck::cast_slice(&half_red_half_alpha),
        8,
    );
    renderer.register_image_from_view(
        263_001,
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
    );

    let pixel = render(&renderer, 263_001, [0.5, 0.5], WHITE_TINT, BLUE);
    assert_eq!(pixel, [188, 0, 188, 255]);
    assert_ne!(pixel, [137, 0, 225, 255]);
    drop(texture);
}

#[test]
fn production_image_pipeline_applies_tint_alpha_once() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    renderer.register_image_rgba8_srgb(263_002, 1, 1, &[255, 0, 0, 255]);

    assert_eq!(
        render(&renderer, 263_002, [0.5, 0.5], [1.0, 1.0, 1.0, 0.5], BLUE,),
        [188, 0, 188, 255]
    );
}

#[test]
fn production_image_pipeline_multiplies_texture_and_tint_alpha_once_each() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let half_red_half_alpha = [0x3800u16, 0, 0, 0x3800];
    let texture = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba16Float,
        bytemuck::cast_slice(&half_red_half_alpha),
        8,
    );
    renderer.register_image_from_view(
        263_003,
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
    );

    assert_eq!(
        render(&renderer, 263_003, [0.5, 0.5], [1.0, 1.0, 1.0, 0.5], BLUE,),
        [137, 0, 225, 255]
    );
    drop(texture);
}

#[test]
fn high_level_direct_deferred_and_custom_fronts_share_one_alpha_contract() {
    const HIGH_LEVEL: u64 = 263_010;
    const DIRECT: u64 = 263_011;
    const DEFERRED: u64 = 263_012;
    const NEAREST: u64 = 263_013;
    const CUSTOM: u64 = 263_014;
    const PREMULTIPLIED: [u8; 4] = [188, 0, 0, 128];
    const EXPECTED: [u8; 4] = [188, 0, 187, 255];

    assert_eq!(
        WgpuRenderer::normalize_image_rgba8_srgb_for_test(&[255, 0, 0, 128]),
        PREMULTIPLIED
    );
    assert_eq!(
        WgpuRenderer::normalize_image_rgba8_srgb_for_test(&[255, 128, 64, 0]),
        [0, 0, 0, 0]
    );

    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    let gpu = renderer.gpu_resources();
    renderer.register_image_rgba8_srgb(HIGH_LEVEL, 1, 1, &[255, 0, 0, 128]);

    let direct = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &PREMULTIPLIED,
        4,
    );
    renderer.register_image_texture(DIRECT, direct);

    let deferred = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &PREMULTIPLIED,
        4,
    );
    gpu.pending_registrations
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(PendingImageRegistration {
            texture_id: DEFERRED,
            view: deferred.create_view(&wgpu::TextureViewDescriptor::default()),
            nearest: false,
        });

    let nearest = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &PREMULTIPLIED,
        4,
    );
    renderer.register_image_from_view_nearest(
        NEAREST,
        nearest.create_view(&wgpu::TextureViewDescriptor::default()),
    );

    let mut custom =
        CustomRenderPass::new_with_filter(CUSTOM, gpu, PhysicalRect::new(0, 0, 1, 1), false)
            .expect("custom front creation failed");
    let mut frame = custom
        .begin_frame(Some("SPEC-HAL-263A custom front"))
        .expect("custom front frame failed");
    {
        let (encoder, view) = frame.encoder_and_view();
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SPEC-HAL-263A custom front clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.502_886_458_032_568_7,
                        g: 0.0,
                        b: 0.0,
                        a: 0.501_960_784_313_725_5,
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
    custom
        .submit_and_swap(frame)
        .expect("custom front publication failed");
    renderer.drain_pending_image_registrations();

    for texture_id in [HIGH_LEVEL, DIRECT, DEFERRED, NEAREST, CUSTOM] {
        assert_eq!(
            render(&renderer, texture_id, [0.5, 0.5], WHITE_TINT, BLUE),
            EXPECTED,
            "texture_id={texture_id}"
        );
    }

    custom.destroy(&mut renderer);
    drop(deferred);
    drop(nearest);
}

#[test]
fn production_image_pipeline_preserves_opaque_content_and_premultiplied_filtering() {
    let Some(mut renderer) = setup_renderer() else {
        return;
    };
    renderer.register_image_rgba8_srgb(263_020, 1, 1, &[64, 128, 255, 255]);
    assert_eq!(
        render(&renderer, 263_020, [0.5, 0.5], WHITE_TINT, BLACK),
        [64, 128, 255, 255]
    );

    let gpu = renderer.gpu_resources();
    let edge = create_sampled_texture(
        &gpu,
        2,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &[255, 0, 0, 255, 0, 0, 0, 0],
        4,
    );
    renderer.register_image_texture(263_021, edge);
    assert_eq!(
        render(&renderer, 263_021, [0.5, 0.5], WHITE_TINT, BLUE),
        [188, 0, 188, 255]
    );
}

#[test]
fn image_rgba8_deferred_registration_normalizes_and_resolves_reused_scene() {
    const IMAGE: u64 = 263_030;
    const DIRECT: u64 = 263_031;
    let mut renderer = setup_renderer().expect("headless Image renderer required");
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    renderer.set_image_admission_wake(Box::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let mut app = AppContext::new(Theme::dark());
    app.set_gpu_resources(renderer.gpu_resources());
    let gpu = app.gpu_resources().unwrap().clone();
    let request = gpu
        .queue_image_rgba8_srgb(IMAGE, 1, 1, vec![255, 0, 0, 128])
        .unwrap();
    assert_eq!(request.try_resolve(), Ok(None));
    assert_eq!(
        renderer.admit_image(IMAGE),
        Err(ImageAdmissionError::MissingRegistration(IMAGE))
    );
    assert_eq!(
        renderer.pending_image_rgba8_registrations_for_test(),
        Some(1)
    );
    assert!(renderer.has_pending_image_admissions());
    assert_eq!(wakes.load(Ordering::Relaxed), 1);

    // Low-level views drain first even when queued after the high-level pixels.
    let low_level = create_sampled_texture(
        &gpu,
        1,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &[255; 4],
        4,
    );
    gpu.pending_registrations
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(PendingImageRegistration {
            texture_id: IMAGE,
            view: low_level.create_view(&wgpu::TextureViewDescriptor::default()),
            nearest: false,
        });
    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    let first = request.try_resolve().unwrap().unwrap();
    assert_eq!(first.identity().texture_id(), IMAGE);
    assert_eq!(first.sampling(), ImageBindingSampling::Linear);
    assert_eq!(first.availability(), Ok(()));
    assert_eq!(first, renderer.admit_image(IMAGE).unwrap());
    assert_eq!(renderer.pending_image_rgba8_registrations_for_test(), None);
    assert!(!renderer.has_pending_image_admissions());
    assert_eq!(wakes.load(Ordering::Relaxed), 2);
    assert_eq!(
        render(&renderer, IMAGE, [0.5, 0.5], WHITE_TINT, BLUE),
        [188, 0, 187, 255]
    );
    renderer.register_image_rgba8_srgb(DIRECT, 1, 1, &[255, 0, 0, 128]);
    assert_eq!(
        render(&renderer, IMAGE, [0.5, 0.5], WHITE_TINT, BLUE),
        render(&renderer, DIRECT, [0.5, 0.5], WHITE_TINT, BLUE)
    );
    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    assert_eq!(request.try_resolve().unwrap(), Some(first.clone()));
    assert_eq!(wakes.load(Ordering::Relaxed), 2);

    let replacement = gpu
        .queue_image_rgba8_srgb(IMAGE, 1, 1, vec![0, 0, 255, 255])
        .unwrap();
    assert_eq!(replacement.try_resolve(), Ok(None));
    assert_eq!(first.availability(), Ok(()));
    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    let second = replacement.try_resolve().unwrap().unwrap();
    assert_ne!(
        first.identity().generation(),
        second.identity().generation()
    );
    assert_eq!(first.availability(), Err(ImageBindingFailure::Replaced));
    assert_eq!(second.availability(), Ok(()));
    assert_eq!(second, renderer.admit_image(IMAGE).unwrap());
}

#[test]
fn image_rgba8_deferred_registration_rejects_invalid_input_without_work() {
    const IMAGE: u64 = 263_032;
    let mut renderer = setup_renderer().expect("headless Image renderer required");
    renderer.drain_pending_image_registrations();
    assert_eq!(renderer.pending_image_rgba8_registrations_for_test(), None);
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    renderer.set_image_admission_wake(Box::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let gpu = renderer.gpu_resources();
    assert_eq!(renderer.pending_image_rgba8_registrations_for_test(), None);
    assert_eq!(
        gpu.queue_image_rgba8_srgb(IMAGE, 0, 1, Vec::new()).err(),
        Some(ImageRgba8RegistrationError::ZeroDimensions {
            width: 0,
            height: 1,
        })
    );
    assert_eq!(
        gpu.queue_image_rgba8_srgb(IMAGE, u32::MAX, u32::MAX, Vec::new())
            .err(),
        Some(ImageRgba8RegistrationError::ByteCountOverflow {
            width: u32::MAX,
            height: u32::MAX,
        })
    );
    assert_eq!(
        gpu.queue_image_rgba8_srgb(IMAGE, 1, 1, vec![0; 3]).err(),
        Some(ImageRgba8RegistrationError::DataLengthMismatch {
            expected: 4,
            actual: 3,
        })
    );
    let limit = gpu.device.limits().max_texture_dimension_2d;
    let width = limit.checked_add(1).unwrap();
    assert_eq!(
        gpu.queue_image_rgba8_srgb(IMAGE, width, 1, vec![0; width as usize * 4])
            .err(),
        Some(ImageRgba8RegistrationError::DimensionLimitExceeded {
            width,
            height: 1,
            limit,
        })
    );
    assert_eq!(renderer.pending_image_rgba8_registrations_for_test(), None);
    assert!(!renderer.has_pending_image_admissions());
    assert!(gpu.pending_registrations.lock().unwrap().is_empty());
    assert!(gpu.pending_unregistrations.lock().unwrap().is_empty());
    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    assert_eq!(
        renderer.admit_image(IMAGE),
        Err(ImageAdmissionError::MissingRegistration(IMAGE))
    );
    assert_eq!(wakes.load(Ordering::Relaxed), 0);
}

#[test]
fn image_rgba8_deferred_registration_retires_pixels_and_rejects_surviving_clones() {
    const IMAGE: u64 = 263_033;
    let renderer = setup_renderer().expect("headless Image renderer required");
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    renderer.set_image_admission_wake(Box::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let gpu = renderer.gpu_resources().clone();
    let pending = gpu
        .queue_image_rgba8_srgb(IMAGE, 4, 4, vec![255; 64])
        .unwrap();
    assert_eq!(pending.try_resolve(), Ok(None));
    assert_eq!(gpu.image_rgba8_ingress_state_for_test(), (false, Some(64)));
    assert!(renderer.has_pending_image_admissions());
    assert!(!gpu.image_admission.has_pending());
    assert_eq!(wakes.load(Ordering::Relaxed), 1);

    drop(renderer);
    assert_eq!(gpu.image_rgba8_ingress_state_for_test(), (true, None));
    assert_eq!(
        pending.try_resolve(),
        Err(ImageAdmissionError::RequestCancelled)
    );
    assert!(!gpu.image_admission.has_pending());
    assert_eq!(wakes.load(Ordering::Relaxed), 2);
    assert_eq!(
        gpu.queue_image_rgba8_srgb(IMAGE, 1, 1, vec![255; 4]).err(),
        Some(ImageRgba8RegistrationError::RendererDestroyed)
    );
    assert_eq!(gpu.image_rgba8_ingress_state_for_test(), (true, None));
    assert!(!gpu.image_admission.has_pending());
    assert_eq!(wakes.load(Ordering::Relaxed), 2);
}

#[test]
fn image_rgba8_deferred_registration_pairs_same_id_calls_to_their_own_generations() {
    const IMAGE: u64 = 263_034;
    let mut renderer = setup_renderer().expect("headless Image renderer required");
    let gpu = renderer.gpu_resources();
    let first = gpu
        .queue_image_rgba8_srgb(IMAGE, 1, 1, vec![255, 0, 0, 128])
        .unwrap();
    let second = gpu
        .queue_image_rgba8_srgb(IMAGE, 1, 1, vec![0, 0, 255, 255])
        .unwrap();
    let legacy = gpu.request_image_binding(IMAGE);
    assert_eq!(first.try_resolve(), Ok(None));
    assert_eq!(second.try_resolve(), Ok(None));
    assert_eq!(legacy.try_resolve(), Ok(None));
    assert_eq!(
        renderer.pending_image_rgba8_registrations_for_test(),
        Some(2)
    );

    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    let first = first.try_resolve().unwrap().unwrap();
    let second = second.try_resolve().unwrap().unwrap();
    assert_eq!(first.identity().texture_id(), IMAGE);
    assert_eq!(second.identity().texture_id(), IMAGE);
    assert_ne!(
        first.identity().generation(),
        second.identity().generation()
    );
    assert_eq!(first.availability(), Err(ImageBindingFailure::Replaced));
    assert_eq!(second.availability(), Ok(()));
    assert_eq!(renderer.admit_image(IMAGE).unwrap(), second);
    assert_eq!(legacy.try_resolve().unwrap(), Some(second));
    assert_eq!(renderer.pending_image_rgba8_registrations_for_test(), None);
    assert!(!renderer.has_pending_image_admissions());
    assert_eq!(
        render(&renderer, IMAGE, [0.5, 0.5], WHITE_TINT, BLUE),
        [0, 0, 255, 255]
    );
}
