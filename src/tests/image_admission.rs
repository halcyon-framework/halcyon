use crate::core::math::RectF;
use crate::core::scene::{Color, SceneBuilder};
use crate::renderer::batcher::PipelineId;
use crate::renderer::image_binding::{ImageAdmissionError, ImageBindingFailure};
use crate::renderer::wgpu_renderer::WgpuRenderer;

const IMAGE: u64 = 263_200;

#[test]
fn image_admission_pending_work_wakes_and_drains_when_the_scene_is_reused() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = wakes.clone();
    renderer.set_image_admission_wake(Box::new(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }));
    let gpu = renderer.gpu_resources();
    assert!(!renderer.has_pending_image_admissions());
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let request = gpu.request_image_binding(IMAGE);
    assert_eq!(request.try_resolve().unwrap(), None);
    assert!(renderer.has_pending_image_admissions());
    assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 1);

    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    assert_eq!(
        request.try_resolve().unwrap().unwrap().availability(),
        Ok(())
    );
    assert!(!renderer.has_pending_image_admissions());
    assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 2);
    crate::core::app::drain_requested_images_for_reused_scene(&mut renderer);
    assert!(request.try_resolve().unwrap().is_some());
    assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[test]
fn image_admission_renderer_teardown_cancels_pending_and_late_requests() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let gpu = renderer.gpu_resources();
    let pending = gpu.request_image_binding(IMAGE);
    assert_eq!(pending.try_resolve(), Ok(None));
    assert!(gpu.image_admission.has_pending());

    drop(renderer);
    assert_eq!(
        pending.try_resolve(),
        Err(ImageAdmissionError::RequestCancelled),
        "a surviving GpuResources clone must not preserve work for a retired renderer"
    );
    assert!(!gpu.image_admission.has_pending());
    let late = gpu.request_image_binding(IMAGE);
    assert_eq!(
        late.try_resolve(),
        Err(ImageAdmissionError::RequestCancelled)
    );
    assert!(
        !gpu.image_admission.has_pending(),
        "retired admission mailboxes must not accumulate requests"
    );
}

fn admitted_image_plan(
    renderer: &mut WgpuRenderer,
    binding: &crate::renderer::image_binding::ImageBinding,
) -> (
    crate::compositor::SurfaceId,
    crate::renderer::DrawPlan<crate::compositor::MultiSurfaceContract>,
) {
    let bounds = RectF::new(0.0, 0.0, 32.0, 32.0);
    let mut compositor = crate::compositor::Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, crate::compositor::SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (32, 32), 1.0)
        .unwrap();
    let mut builder = SceneBuilder::new();
    builder.push_admitted_image(
        bounds,
        binding,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        1,
        None,
    );
    let mut scenes = std::collections::HashMap::new();
    scenes.insert(surface, builder.build());
    let plan = crate::renderer::DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut crate::renderer::mesh_cache::MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    (surface, plan)
}

#[test]
fn image_admission_is_generation_specific_and_revoked_by_its_resource_owner() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    assert!(matches!(
        renderer.admit_image(IMAGE),
        Err(ImageAdmissionError::MissingRegistration(IMAGE))
    ));
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255, 0, 0, 255]);
    let first = renderer.admit_image(IMAGE).unwrap();
    assert_eq!(first.availability(), Ok(()));
    assert_eq!(first.identity().texture_id(), IMAGE);
    assert_eq!(renderer.admit_image(IMAGE).unwrap(), first);

    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = wakes.clone();
    let changes = std::sync::Arc::new(
        crate::renderer::image_binding::ImageBindingInvalidations::new(Some(Box::new(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }))),
    );
    first.observe(&changes);

    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 0, 255, 255]);
    let second = renderer.admit_image(IMAGE).unwrap();
    assert_ne!(first.identity(), second.identity());
    assert_eq!(first.availability(), Err(ImageBindingFailure::Replaced));
    assert_eq!(second.availability(), Ok(()));
    assert_eq!(changes.take(), vec![first.identity()]);
    assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 1);
    first.observe(&changes);
    assert!(
        changes.take().is_empty(),
        "repainting a revoked binding must not reschedule terminal failure"
    );
    assert_eq!(wakes.load(std::sync::atomic::Ordering::Relaxed), 1);

    renderer.unregister_image(IMAGE);
    assert_eq!(
        second.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 255, 0, 255]);
    assert_ne!(renderer.admit_image(IMAGE).unwrap(), second);
    assert_eq!(
        second.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
}

#[test]
fn image_admission_deferred_request_is_not_evidence_before_renderer_drain() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let request = gpu.request_image_binding(IMAGE);
    assert_eq!(request.try_resolve().unwrap(), None);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Image admission deferred registration proof"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    gpu.pending_registrations.lock().unwrap().push(
        crate::renderer::wgpu_renderer::PendingImageRegistration {
            texture_id: IMAGE,
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            nearest: true,
        },
    );
    assert!(matches!(
        renderer.admit_image(IMAGE),
        Err(ImageAdmissionError::MissingRegistration(IMAGE))
    ));
    renderer.drain_pending_image_registrations();
    let binding = request.try_resolve().unwrap().unwrap();
    assert_eq!(binding.availability(), Ok(()));
    assert_eq!(binding, renderer.admit_image(IMAGE).unwrap());
    assert_eq!(
        binding.sampling(),
        crate::renderer::image_binding::ImageBindingSampling::Nearest
    );

    let missing = gpu.request_image_binding(IMAGE + 1);
    renderer.drain_pending_image_registrations();
    assert!(matches!(
        missing.try_resolve(),
        Err(ImageAdmissionError::MissingRegistration(id)) if id == IMAGE + 1
    ));
    gpu.pending_unregistrations.lock().unwrap().push(
        crate::renderer::wgpu_renderer::PendingImageUnregistration {
            texture_id: IMAGE,
            retained_textures: Vec::new(),
        },
    );
    let removed = gpu.request_image_binding(IMAGE);
    renderer.drain_pending_image_registrations();
    assert!(matches!(
        removed.try_resolve(),
        Err(ImageAdmissionError::MissingRegistration(IMAGE))
    ));
    assert_eq!(
        binding.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
}

#[test]
fn image_admission_scene_and_batch_identity_preserve_the_exact_generation() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let first = renderer.admit_image(IMAGE).unwrap();
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 0, 255, 255]);
    let second = renderer.admit_image(IMAGE).unwrap();
    let mut builder = SceneBuilder::new();
    for binding in [&first, &second] {
        builder.push_admitted_image(
            RectF::new(0.0, 0.0, 10.0, 10.0),
            binding,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            1,
            None,
        );
    }
    let scene = builder.build();
    let mut compositor = crate::compositor::Compositor::new(RectF::new(0.0, 0.0, 32.0, 32.0));
    let surface = compositor.create_surface(
        0,
        RectF::new(0.0, 0.0, 32.0, 32.0),
        crate::compositor::SceneKey::BASE,
    );
    let mut scenes = std::collections::HashMap::new();
    scenes.insert(surface, scene);
    let plan = crate::renderer::DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut crate::renderer::mesh_cache::MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    let images: Vec<_> = plan.surface_data[&surface]
        .batches
        .iter()
        .filter(|batch| batch.key.pipeline == PipelineId::Image)
        .collect();
    assert_eq!(
        images.len(),
        2,
        "a stale generation must not merge with its replacement"
    );
    assert_ne!(
        images[0].key.image_generation,
        images[1].key.image_generation
    );
}

#[test]
fn image_admission_renderer_destruction_expires_cpu_evidence_without_retaining_gpu() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let device = std::sync::Arc::downgrade(&renderer.device());
    drop(renderer);
    assert_eq!(
        device.strong_count(),
        0,
        "CPU evidence must not pin the renderer's Device Arc"
    );
    assert_eq!(
        binding.availability(),
        Err(ImageBindingFailure::RendererDestroyed)
    );
    let Some(mut next_renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    next_renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let next = next_renderer.admit_image(IMAGE).unwrap();
    assert_ne!(binding.identity(), next.identity());
    assert_eq!(next.availability(), Ok(()));
    let next_device = next_renderer.device();
    next_device.destroy();
    let _ = next_device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(next.availability(), Err(ImageBindingFailure::DeviceLost));
    assert_eq!(
        binding.availability(),
        Err(ImageBindingFailure::RendererDestroyed)
    );
}

#[test]
fn image_admission_compile_never_substitutes_a_same_number_replacement() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let (_, plan) = admitted_image_plan(&mut renderer, &binding);
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 0, 255, 255]);
    #[cfg(debug_assertions)]
    {
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            renderer.compile_multi(&plan)
        }));
        let message = match rejected {
            Err(message) => message,
            Ok(_) => panic!("obsolete admitted generation must fail INV-IMG-005"),
        };
        let text = message
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| message.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(text.contains("INV-IMG-005"));
    }
    #[cfg(not(debug_assertions))]
    {
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        assert!(compiled.surface_data.values().all(|surface| {
            surface
                .frame_batches
                .batches
                .iter()
                .all(|batch| batch.key.pipeline != PipelineId::Image)
        }));
    }
}

#[test]
fn image_admission_registration_mutation_retires_compiled_preparation() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let (surface, plan) = admitted_image_plan(&mut renderer, &binding);
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    let gpu = renderer.gpu_resources();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Image admission execution custody proof"),
        size: wgpu::Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    assert!(
        renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .is_some()
    );

    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 0, 255, 255]);
    assert_eq!(binding.availability(), Err(ImageBindingFailure::Replaced));
    assert!(
        renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .is_none(),
        "Execute must not resolve a compiled admitted Image to its replacement"
    );
    assert!(renderer.render_multi(compiled).is_empty());

    let replacement = renderer.admit_image(IMAGE).unwrap();
    let (surface, plan) = admitted_image_plan(&mut renderer, &replacement);
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    renderer.unregister_image(IMAGE + 1);
    assert!(
        renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .is_some(),
        "a missing-registration no-op must preserve a valid preparation"
    );
    renderer.unregister_image(IMAGE);
    assert!(
        renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .is_none(),
        "resource retirement must refuse encoding before Image lookup"
    );
    assert!(renderer.render_multi(compiled).is_empty());
}

#[test]
fn image_admission_survives_pipeline_revalidation_but_not_device_loss() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255; 4]);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let (surface, plan) = admitted_image_plan(&mut renderer, &binding);
    assert!(renderer.compile_multi(&plan).unwrap().is_ready());
    renderer.set_scale_factor(surface, 2.0);
    assert!(renderer.compile_multi(&plan).unwrap().is_ready());
    assert_eq!(renderer.admit_image(IMAGE).unwrap(), binding);
    assert_eq!(binding.availability(), Ok(()));
    let device = renderer.device();
    device.destroy();
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(binding.availability(), Err(ImageBindingFailure::DeviceLost));
}
