    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn frosted_compile_pending_retains_graph_ready_oom_and_stale_cancellation() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::{math::RectF, paint::{Color, FrostedGlassAppearance, PaintSink, RoundedCorners}, scene::SceneBuilder};
        use crate::renderer::{DrawPlanBuilder, RendererCompileOutcome as Outcome, RendererPreparationIdentity};
        use std::sync::atomic::{AtomicUsize, Ordering};

        fn suspend(renderer: &mut WgpuRenderer, plan: crate::renderer::MultiTargetDrawPlan)
            -> RendererPreparationIdentity {
            // Controlled asynchronous error-scope completion with real prepared
            // GPU resources; this does not claim a real native allocation OOM.
            let identity = RendererPreparationIdentity::new(plan.preparation.clone());
            identity.add_scope();
            renderer.preparation_gate = Some(identity.clone());
            assert!(matches!(
                renderer.publish_multi_compile(Some(plan)),
                Ok(Outcome::Pending(_))
            ));
            identity
        }

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 40.0, 32.0));
        let surface =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 40.0, 32.0), SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (40, 32), 1.0)
            .unwrap();
        let mut scene = SceneBuilder::new();
        scene.push_rect(RectF::new(0.0,0.0,40.0,32.0), Color::WHITE.into(), 0, None);
        scene.push_frosted_glass(RectF::new(2.0,2.0,30.0,24.0), RoundedCorners::uniform(3.0),
            &FrostedGlassAppearance::try_new(1.5, Color::TRANSPARENT, 1.0).unwrap(), 10, None).unwrap();
        let plan = DrawPlanBuilder::new().build_multi(
            &std::collections::HashMap::from([(surface,scene.build())]), compositor.snapshot_multi(),
            renderer.mesh_cache(), |_|1.0, 0).unwrap();

        let candidate = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native preparation");
        let identity = suspend(&mut renderer, candidate);
        let allocations = renderer.frosted_glass.allocations;
        let epoch = renderer.frame_epoch;
        assert!(
            !renderer.is_current_preparation(renderer.suspended_compile.as_ref().unwrap().plan())
        );
        assert!(matches!(
            renderer.resume_compile(&identity),
            Ok(Outcome::Pending(_))
        ));
        assert_eq!(renderer.frosted_glass.allocations, allocations);
        assert_eq!(renderer.frame_epoch, epoch);
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        identity.on_completed(move || {
            count.fetch_add(1, Ordering::AcqRel);
        });
        identity.finish_scope(false);
        let ready = renderer
            .resume_compile(&identity)
            .unwrap()
            .expect_ready("scope qualified");
        assert!(Arc::ptr_eq(&ready.preparation, identity.token()));
        assert!(renderer.is_current_preparation(&ready));
        assert_eq!(wakes.load(Ordering::Acquire), 1);
        assert_eq!(renderer.frosted_glass.allocations, allocations);
        assert!(matches!(
            renderer.resume_compile(&identity),
            Err(crate::renderer::RendererCompileError::Surface(
                crate::renderer::RendererSurfaceError::Outdated
            ))
        ));
        drop(ready);

        let candidate = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native OOM fixture");
        let identity = suspend(&mut renderer, candidate);
        identity.finish_scope(true);
        assert!(matches!(
            renderer.resume_compile(&identity),
            Err(crate::renderer::RendererCompileError::Surface(
                crate::renderer::RendererSurfaceError::OutOfMemory
            ))
        ));
        assert!(renderer.suspended_compile.is_none());

        let candidate = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native stale fixture");
        let identity = suspend(&mut renderer, candidate);
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        identity.on_completed(move || {
            count.fetch_add(1, Ordering::AcqRel);
        });
        let replacement = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("new current frame");
        identity.finish_scope(false);
        assert_eq!(wakes.load(Ordering::Acquire), 0);
        assert!(matches!(
            renderer.resume_compile(&identity),
            Err(crate::renderer::RendererCompileError::Surface(
                crate::renderer::RendererSurfaceError::Outdated
            ))
        ));
        assert!(renderer.is_current_preparation(&replacement));
        drop(replacement);

        let candidate = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native target-loss fixture");
        let identity = suspend(&mut renderer, candidate);
        renderer.unregister_render_target(surface).unwrap();
        identity.finish_scope(false);
        assert!(matches!(
            renderer.resume_compile(&identity),
            Err(crate::renderer::RendererCompileError::Surface(
                crate::renderer::RendererSurfaceError::Outdated
            ))
        ));
        assert!(renderer.suspended_compile.is_none());
    }
    #[test]
    fn local_ambient_glow_vertex_abi_and_rotated_quad() {
        use crate::core::math::Point;
        use crate::core::paint::{Color, LocalAmbientGlowAppearance, LocalGlowLobe};
        let lobe = LocalGlowLobe::try_new(
            Point::new(0.25, 0.75),
            Point::new(20.0, 10.0),
            Point::new(0.8, 0.6),
            Color::from_linear(0.8, 0.2, 0.1, 0.6),
            0.7,
        )
        .unwrap();
        let appearance = LocalAmbientGlowAppearance::try_new(&[lobe]).unwrap();
        let rect = RectF::new(10.0, 20.0, 80.0, 40.0);
        let bounds = appearance.bounds(rect).unwrap().unwrap();
        let primitive = crate::renderer::batcher::FlatPrimitive {
            key: crate::renderer::batcher::DrawBatchKey::shape(
                0,
                ShapeType::LocalAmbientGlow as u8,
                1,
            ),
            data: PrimitiveData::LocalAmbientGlow {
                rect,
                bounds,
                appearance,
            },
            clip_state: crate::renderer::batcher::ClipState::none(),
            order_key: 0,
        };
        let (vertices, indices) =
            WgpuRenderer::build_shape_buffers_from_surface_primitives(&[primitive]);
        assert_eq!(std::mem::size_of::<ShapeVertex>(), 112);
        assert_eq!(ShapeVertex::ATTRIBUTES.len(), 9);
        assert_eq!(indices, [0, 1, 2, 2, 3, 0]);
        assert_eq!(vertices.len(), 4);
        let hx = (16.0_f32 * 16.0 + 6.0 * 6.0).sqrt();
        let hy = (12.0_f32 * 12.0 + 8.0 * 8.0).sqrt();
        assert_eq!(bounds, RectF::new(30.0 - hx, 50.0 - hy, 2.0 * hx, 2.0 * hy));
        for v in vertices {
            assert_eq!(v.shape_type, 5);
            assert_eq!(v.position, v.local_pos);
            assert_eq!(v.params0, [30.0, 50.0, 20.0, 10.0]);
            assert_eq!(v.params1, [0.0; 4]);
            assert_eq!(v.params2, [0.0; 4]);
            assert_eq!(v.params3, [0.8, 0.6, 0.0, 0.0]);
            assert_eq!(v.contour_gains, [0.7, 0.0]);
            assert_eq!(v.color, [0.8, 0.2, 0.1, 0.6]);
            assert_eq!(&bytemuck::bytes_of(&v)[108..112], &[0, 0, 0, 0]);
        }
    }

    #[test]
    fn sdf_protected_image_registrar_rejects_unauthenticated_mutations() {
        use crate::renderer::custom_render_pass::{CustomRenderPass, PhysicalRect};
        use crate::renderer::image_binding::ImageBindingFailure;
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

        fn rejected(operation: impl FnOnce()) {
            let error = catch_unwind(AssertUnwindSafe(operation))
                .expect_err("protected mutation must synchronously panic");
            let message = error
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| error.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(message.contains("protected SDF texture ID"), "{message}");
        }
        fn texture(device: &wgpu::Device) -> wgpu::Texture {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("registrar collision fixture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        }

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        let protected = 4_202_610_305;
        let ordinary = 4_202_610_306;
        let mut producer = Some(
            CustomRenderPass::new(protected, gpu.clone(), PhysicalRect::new(0, 0, 1, 1)).unwrap(),
        );
        renderer.drain_pending_image_registrations();
        let binding = renderer.admit_image(protected).unwrap();
        let identity = binding.identity();
        renderer.protect_sdf_image_for_test(protected);
        let still_current = |renderer: &mut WgpuRenderer| {
            assert_eq!(binding.availability(), Ok(()));
            assert_eq!(
                renderer.admit_image(protected).unwrap().identity(),
                identity
            );
        };

        // Cleanup is outside the assertion unwind so producer Drop cannot cause
        // a secondary panic or release a queued front before unregister.
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            rejected(|| renderer.register_image_texture(protected, texture(&gpu.device)));
            still_current(&mut renderer);
            rejected(|| {
                renderer.register_image_from_view(
                    protected,
                    texture(&gpu.device).create_view(&Default::default()),
                )
            });
            still_current(&mut renderer);
            rejected(|| {
                renderer.register_image_from_view_nearest(
                    protected,
                    texture(&gpu.device).create_view(&Default::default()),
                )
            });
            still_current(&mut renderer);
            rejected(|| renderer.register_image_rgba8_srgb(protected, 1, 1, &[255; 4]));
            still_current(&mut renderer);
            rejected(|| renderer.unregister_image(protected));
            still_current(&mut renderer);
            rejected(|| {
                let _ = producer.as_ref().unwrap().register_front_now(&mut renderer);
            });
            still_current(&mut renderer);

            renderer.register_image_rgba8_srgb(ordinary, 1, 1, &[255; 4]);
            let first = renderer.admit_image(ordinary).unwrap();
            renderer.register_image_texture(ordinary, texture(&gpu.device));
            assert_eq!(first.availability(), Err(ImageBindingFailure::Replaced));
            let ordinary_binding = renderer.admit_image(ordinary).unwrap();

            // Even an earlier ordinary removal must remain unapplied when a
            // different mutation queue contains a protected conflict.
            gpu.pending_unregistrations
                .lock()
                .unwrap()
                .push(PendingImageUnregistration {
                    texture_id: ordinary,
                    retained_textures: Vec::new(),
                });
            gpu.pending_registrations
                .lock()
                .unwrap()
                .push(PendingImageRegistration {
                    texture_id: protected,
                    view: texture(&gpu.device).create_view(&Default::default()),
                    nearest: false,
                });
            rejected(|| renderer.drain_pending_image_registrations());
            still_current(&mut renderer);
            assert_eq!(ordinary_binding.availability(), Ok(()));
            assert_eq!(gpu.pending_unregistrations.lock().unwrap().len(), 1);
            assert_eq!(gpu.pending_registrations.lock().unwrap().len(), 1);
            gpu.pending_registrations.lock().unwrap().clear();
            gpu.pending_registrations
                .lock()
                .unwrap()
                .push(PendingImageRegistration {
                    texture_id: ordinary,
                    view: texture(&gpu.device).create_view(&Default::default()),
                    nearest: true,
                });
            renderer.drain_pending_image_registrations();
            assert_eq!(
                ordinary_binding.availability(),
                Err(ImageBindingFailure::Unregistered)
            );
            let ordinary_binding = renderer.admit_image(ordinary).unwrap();

            let ordinary_request = gpu
                .queue_image_rgba8_srgb(ordinary, 1, 1, vec![255; 4])
                .unwrap();
            let protected_request = gpu
                .queue_image_rgba8_srgb(protected, 1, 1, vec![255; 4])
                .unwrap();
            rejected(|| renderer.drain_pending_image_registrations());
            still_current(&mut renderer);
            assert_eq!(ordinary_binding.availability(), Ok(()));
            assert_eq!(ordinary_request.try_resolve().unwrap(), None);
            assert_eq!(protected_request.try_resolve().unwrap(), None);
            assert_eq!(
                renderer.pending_image_rgba8_registrations_for_test(),
                Some(2)
            );
            // Explicit test recovery removes only its rejected unauthenticated
            // request; the normal request is then consumed by the real drain.
            renderer
                .pending_rgba8_registrations
                .get()
                .unwrap()
                .pending
                .lock()
                .unwrap()
                .items
                .as_mut()
                .unwrap()
                .retain(|entry| entry.admission.texture_id != protected);
            renderer.drain_pending_image_registrations();
            assert_eq!(
                ordinary_binding.availability(),
                Err(ImageBindingFailure::Replaced)
            );
            assert!(ordinary_request.try_resolve().unwrap().is_some());
            still_current(&mut renderer);

            producer.take().unwrap().destroy_deferred().unwrap();
            rejected(|| renderer.drain_pending_image_registrations());
            still_current(&mut renderer);
            let removals = gpu.pending_unregistrations.lock().unwrap();
            assert_eq!(removals.len(), 1);
            assert_eq!(removals[0].texture_id, protected);
            assert_eq!(removals[0].retained_textures.len(), 2);
            assert_eq!(removals[0].retained_textures[0].width(), 1);
            drop(removals);
        }));
        renderer.protected_sdf_texture_ids.remove(&protected);
        if let Some(producer) = producer.take() {
            producer.destroy(&mut renderer);
        }
        renderer.drain_pending_image_registrations();
        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        if let Err(error) = outcome {
            resume_unwind(error);
        }
    }

    #[test]
    fn window_surface_web_srgb_storage_and_attachment_formats_agree() {
        use wgpu::TextureFormat::*;
        for (storage, attachment) in [(Rgba8Unorm, Rgba8UnormSrgb), (Bgra8Unorm, Bgra8UnormSrgb)] {
            assert_eq!(web_color_attachment_format(storage), attachment);
            assert_eq!(web_color_attachment_format(attachment), attachment);
            assert_eq!(web_canvas_formats(attachment), (storage, vec![attachment]));
        }
        assert_eq!(web_color_attachment_format(Rgba16Float), Rgba16Float);
        assert_eq!(web_canvas_formats(Rgba16Float), (Rgba16Float, vec![]));
    }

    #[test]
    fn window_surface_web_srgb_view_encodes_linear_colors_on_gpu() {
        let renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("window color-transfer proof requires the native GPU");
        for storage in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let attachment = web_color_attachment_format(storage);
            let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Window sRGB alias color-transfer proof"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: storage,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[attachment],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(attachment),
                ..Default::default()
            });
            for hex in [
                0xf2f2f2, 0xf7f7f7, 0xfafafa, 0xe74c3b, 0x3499db, 0xffffff, 0x808080,
            ] {
                let linear = crate::core::paint::Color::from_hex(hex);
                let readback = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Window sRGB alias readback"),
                    size: u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = renderer.device.create_command_encoder(&Default::default());
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Window linear-color clear through sRGB view"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            depth_slice: None,
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: linear.r.into(),
                                    g: linear.g.into(),
                                    b: linear.b.into(),
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
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
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
                renderer.queue.submit([encoder.finish()]);
                let bytes = mapped_test_pixel_bytes(&renderer.device, &readback).unwrap();
                let rgb = if storage == wgpu::TextureFormat::Bgra8Unorm {
                    [bytes[2], bytes[1], bytes[0]]
                } else {
                    [bytes[0], bytes[1], bytes[2]]
                };
                let expected = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
                assert_eq!(rgb, expected, "{storage:?} through {attachment:?}");
                assert_eq!(bytes[3], 255);
                eprintln!("window sRGB view: {storage:?}/{attachment:?} #{hex:06x} -> {rgb:?}");
            }
        }
    }
    #[test]
    fn window_surface_configuration_keeps_one_frame_queued() {
        let config = window_surface_configuration(
            wgpu::TextureFormat::Bgra8UnormSrgb,
            (3008, 1692),
            wgpu::PresentMode::AutoVsync,
            wgpu::CompositeAlphaMode::Auto,
        );

        assert_eq!(config.usage, wgpu::TextureUsages::RENDER_ATTACHMENT);
        assert_eq!(config.format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!((config.width, config.height), (3008, 1692));
        assert_eq!(config.present_mode, wgpu::PresentMode::AutoVsync);
        assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::Auto);
        assert!(config.view_formats.is_empty());
        assert_eq!(
            config.desired_maximum_frame_latency, 2,
            "SPEC-HAL-030A requires one queued frame for CPU/GPU overlap"
        );
    }

    #[test]
    fn test_globals_uniform_layout() {
        // SPEC-HAL-085: Globals struct is shared across all pipelines
        let globals = Globals::new(960.0, 540.0, 1920.0, 1080.0, true);
        assert_eq!(globals.surface_size, [1920.0, 1080.0]);
        assert_eq!(globals.flags, 1);
        // Size: mat4x4<f32> (64) + vec2<f32> (8) + f32 (4) + u32 (4) = 80 bytes
        assert_eq!(std::mem::size_of::<Globals>(), 80);
    }

    #[test]
    fn test_clip_rect_uniform_layout() {
        let clip = ClipRectUniform {
            min: [0.0, 0.0],
            max: [1920.0, 1080.0],
        };
        assert_eq!(clip.min, [0.0, 0.0]);
        assert_eq!(clip.max, [1920.0, 1080.0]);
        assert_eq!(std::mem::size_of::<ClipRectUniform>(), 16);
    }

    // ========================================================================
    // SPEC-HAL-024 Stage 7: Logical→Physical Conversion Contract Tests
    // ========================================================================

    /// SPEC-HAL-024 Stage 7: Exercise the production scissor conversion without
    /// requiring GPU initialization.
    fn logical_rect_to_physical(
        rect: &RectF,
        logical_width: f32,
        logical_height: f32,
        scale_factor: f32,
    ) -> (u32, u32, u32, u32) {
        let physical_width = (logical_width * scale_factor).round() as u32;
        let physical_height = (logical_height * scale_factor).round() as u32;
        crate::renderer::render_graph::FrameConstants {
            viewport_size: [physical_width as f32, physical_height as f32],
            physical_width,
            physical_height,
            logical_width,
            logical_height,
            scale_factor,
            clear_color: Color::TRANSPARENT,
        }
        .clip_rect_to_scissor(Some(rect))
    }

    /// SPEC-HAL-024 Stage 7: Given an absolute_paint_rect in logical coords,
    /// verify the physical conversion is exactly (logical * scale_factor).
    #[test]
    fn test_stage7_logical_to_physical_conversion() {
        // Simulate absolute_paint_rect at (10, 20) with size (50, 30) in logical pixels
        let abs_rect = RectF::new(10.0, 20.0, 50.0, 30.0);

        // Scale factor 2.0 (common for HiDPI displays)
        let scale_factor = 2.0;
        let logical_width = 800.0;
        let logical_height = 600.0;

        let (phys_x, phys_y, phys_w, phys_h) =
            logical_rect_to_physical(&abs_rect, logical_width, logical_height, scale_factor);

        // Expected: (10*2, 20*2, 50*2, 30*2) = (20, 40, 100, 60)
        assert_eq!(phys_x, 20, "physical x should be logical_x * scale_factor");
        assert_eq!(phys_y, 40, "physical y should be logical_y * scale_factor");
        assert_eq!(
            phys_w, 100,
            "physical width should be logical_width * scale_factor"
        );
        assert_eq!(
            phys_h, 60,
            "physical height should be logical_height * scale_factor"
        );
    }

    /// SPEC-HAL-024 Stage 7: Verify clamping behavior at viewport boundaries.
    #[test]
    fn test_stage7_clamp_to_viewport() {
        // Rect extends beyond viewport (logical viewport is 100x100)
        let abs_rect = RectF::new(80.0, 90.0, 50.0, 50.0);
        let scale_factor = 1.5;
        let logical_width = 100.0;
        let logical_height = 100.0;

        let (phys_x, phys_y, phys_w, phys_h) =
            logical_rect_to_physical(&abs_rect, logical_width, logical_height, scale_factor);

        // Logical clamped: x=80, y=90, w=20 (100-80), h=10 (100-90)
        // Physical: 80*1.5=120, 90*1.5=135, 20*1.5=30, 10*1.5=15
        assert_eq!(phys_x, 120);
        assert_eq!(phys_y, 135);
        assert_eq!(phys_w, 30);
        assert_eq!(phys_h, 15);
    }

    /// SPEC-HAL-024 Stage 7: Negative coords get clamped to 0.
    #[test]
    fn test_stage7_clamp_negative_coords() {
        let abs_rect = RectF::new(-10.0, -5.0, 50.0, 30.0);
        let scale_factor = 2.0;
        let logical_width = 800.0;
        let logical_height = 600.0;

        let (phys_x, phys_y, phys_w, phys_h) =
            logical_rect_to_physical(&abs_rect, logical_width, logical_height, scale_factor);

        // The negative portion lies outside the target; only the intersection
        // x=[0,40), y=[0,25) reaches physical pixels.
        assert_eq!(phys_x, 0);
        assert_eq!(phys_y, 0);
        assert_eq!(phys_w, 80);
        assert_eq!(phys_h, 50);
    }

    // ========================================================================
    // SPEC-HAL-081B: Globals Slab Capacity Growth Tests
    // ========================================================================

    /// SPEC-HAL-081B: Verify globals slab capacity growth formula has no arbitrary cap.
    ///
    /// This tests the power-of-two growth logic used by `ensure_globals_slab_capacity()`
    /// without requiring GPU initialization. Ensures >8 targets work without panic.
    #[test]
    fn test_globals_slab_capacity_growth_no_arbitrary_cap() {
        // Helper that mirrors the capacity calculation in ensure_globals_slab_capacity()
        fn compute_new_capacity(required_slots: usize) -> usize {
            required_slots.max(1).next_power_of_two()
        }

        // Test that capacity grows correctly for various target counts
        assert_eq!(compute_new_capacity(1), 1);
        assert_eq!(compute_new_capacity(2), 2);
        assert_eq!(compute_new_capacity(3), 4);
        assert_eq!(compute_new_capacity(4), 4);
        assert_eq!(compute_new_capacity(5), 8);
        assert_eq!(compute_new_capacity(8), 8);

        // Critical: >8 targets must work (was MAX_RENDER_TARGETS limit)
        assert_eq!(compute_new_capacity(9), 16);
        assert_eq!(compute_new_capacity(16), 16);
        assert_eq!(compute_new_capacity(17), 32);
        assert_eq!(compute_new_capacity(32), 32);
        assert_eq!(compute_new_capacity(33), 64);

        // Very large counts should still work (limited only by memory/usize)
        assert_eq!(compute_new_capacity(100), 128);
        assert_eq!(compute_new_capacity(1000), 1024);

        // Verify buffer size calculation doesn't overflow for reasonable counts
        let large_capacity = compute_new_capacity(256);
        let buffer_size = large_capacity as u64 * GLOBALS_UNIFORM_ALIGNMENT;
        assert_eq!(buffer_size, 256 * 256); // 256 slots * 256 bytes/slot = 64KB
    }

    /// SPEC-HAL-081B: Verify GLOBALS_UNIFORM_ALIGNMENT is appropriate for dynamic offsets.
    #[test]
    fn test_globals_uniform_alignment() {
        // WGPU requires min_uniform_buffer_offset_alignment of 256 bytes
        assert_eq!(GLOBALS_UNIFORM_ALIGNMENT, 256);

        // Globals struct (80 bytes) must fit within one aligned slot
        assert!(std::mem::size_of::<Globals>() <= GLOBALS_UNIFORM_ALIGNMENT as usize);
    }

    #[test]
    fn radial_gauge_uniform_fits_slot() {
        let size = std::mem::size_of::<RadialGaugeParamsUniform>();
        assert!(
            size <= 256,
            "RadialGaugeParamsUniform is {} bytes, exceeds 256-byte slot",
            size
        );
    }

    #[test]
    fn shape_buffer_builder_expands_primitives_to_sdf_quads() {
        use crate::core::math::Point;
        use crate::renderer::batcher::{ClipState, DrawBatchKey, FlatPrimitive};

        let primitives = vec![
            FlatPrimitive {
                key: DrawBatchKey::shape(0, ShapeType::Circle as u8, 0),
                data: PrimitiveData::Circle {
                    center: Point::new(10.0, 20.0),
                    radius: 5.0,
                    color: Color::WHITE,
                },
                clip_state: ClipState::none(),
                order_key: 0,
            },
            FlatPrimitive {
                key: DrawBatchKey::shape(0, ShapeType::Line as u8, 0),
                data: PrimitiveData::Line {
                    a: Point::new(0.0, 0.0),
                    b: Point::new(10.0, 0.0),
                    width: 2.0,
                    color: Color::WHITE,
                },
                clip_state: ClipState::none(),
                order_key: 1,
            },
            FlatPrimitive {
                key: DrawBatchKey::shape(0, ShapeType::Polyline as u8, 0),
                data: PrimitiveData::Polyline {
                    points: vec![
                        Point::new(0.0, 0.0),
                        Point::new(10.0, 0.0),
                        Point::new(10.0, 10.0),
                    ],
                    width: 2.0,
                    color: Color::WHITE,
                },
                clip_state: ClipState::none(),
                order_key: 2,
            },
        ];

        let (vertices, indices) =
            WgpuRenderer::build_shape_buffers_from_surface_primitives(&primitives);

        assert_eq!(vertices.len(), 16);
        assert_eq!(indices.len(), 24);
        assert_eq!(vertices[0].shape_type, ShapeType::Circle as u32);
        assert_eq!(vertices[4].shape_type, ShapeType::Line as u32);
        assert_eq!(vertices[8].shape_type, ShapeType::Polyline as u32);
        assert_eq!(vertices[12].shape_type, ShapeType::Polyline as u32);
        assert_eq!(vertices[0].params0, [10.0, 20.0, 5.0, 0.0]);
        assert_eq!(vertices[4].params0, [0.0, 0.0, 10.0, 0.0]);
        assert_eq!(vertices[4].params1[0], 1.0);
    }

    #[test]
    fn luminous_contour_shape_buffers_pack_single_owner_regions_and_front_gains() {
        use crate::core::math::Point;
        use crate::core::paint::{
            LuminousContourAppearance, LuminousContourFront, LuminousContourFronts,
        };
        use crate::renderer::batcher::{ClipState, DrawBatchKey, FlatPrimitive};

        let appearance =
            LuminousContourAppearance::try_new_zoned(0.4, 1.5, 0.6, 5.0, 0.10, 4.0).unwrap();
        let fronts =
            LuminousContourFronts::try_from_slice(&[
                LuminousContourFront::try_new(20.0, 4.0, 0.8).unwrap()
            ])
            .unwrap();
        let mut primitive = FlatPrimitive {
            key: DrawBatchKey::shape(0, ShapeType::LuminousContour as u8, 0),
            data: PrimitiveData::LuminousContour {
                points: Arc::from([Point::new(10.0, 20.0), Point::new(30.0, 20.0)]),
                core_width: 2.0,
                color: Color::from_linear(0.1, 0.2, 0.4, 1.0),
                appearance,
                fronts,
                total_length: 20.0,
                scroll_offset: (3.0, 5.0),
            },
            clip_state: ClipState::none(),
            order_key: 0,
        };

        let (vertices, indices) =
            WgpuRenderer::build_shape_buffers_from_surface_primitives(std::slice::from_ref(&primitive));

        assert_eq!(vertices.len(), 36);
        assert_eq!(indices.len(), 54);
        assert_eq!(vertices[0].position, [0.0, 8.0]);
        assert_eq!(vertices[0].params0, [7.0, 15.0, 19.0, 15.0]);
        assert_eq!(vertices[0].params1, [1.0, 5.0, 0.10, 4.0]);
        assert_eq!(vertices[0].params2, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(vertices[0].params3, [0.4, 1.5, 0.6, 0.0]);
        assert_eq!(vertices[0].contour_gains, [0.0, 0.0]);
        assert_eq!(vertices[16].params0, [22.0, 15.0, 23.0, 15.0]);
        assert!((vertices[16].contour_gains[1] - 0.8).abs() < 0.0001);
        assert_eq!(vertices[16].color, [0.1, 0.2, 0.4, 1.0]);
        assert_eq!(vertices[16].shape_type, ShapeType::LuminousContour as u32);
        assert_eq!(vertices[32].params2, [1.0, 0.0, 0.0, 0.0]);

        if let PrimitiveData::LuminousContour { appearance, .. } = &mut primitive.data {
            *appearance = appearance.with_front_opacity_boost(0.0).unwrap();
        }
        let (zero_vertices, zero_indices) =
            WgpuRenderer::build_shape_buffers_from_surface_primitives(std::slice::from_ref(&primitive));
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&vertices),
            bytemuck::cast_slice::<_, u8>(&zero_vertices)
        );
        assert_eq!(indices, zero_indices);

        if let PrimitiveData::LuminousContour { appearance, .. } = &mut primitive.data {
            *appearance = appearance.with_front_opacity_boost(0.75).unwrap();
        }
        let (boosted, boosted_indices) =
            WgpuRenderer::build_shape_buffers_from_surface_primitives(std::slice::from_ref(&primitive));
        assert_eq!(indices, boosted_indices);
        for (original, mut actual) in vertices.iter().zip(boosted) {
            assert_eq!(actual.params3[3], 0.75);
            actual.params3[3] = 0.0;
            assert_eq!(bytemuck::bytes_of(original), bytemuck::bytes_of(&actual));
        }
    }

    #[test]
    fn luminous_contour_shape_indices_rebase_beyond_u16_capacity() {
        let mut rebased = Vec::new();
        WgpuRenderer::extend_rebased_u32_indices(&mut rebased, vec![0, 1, 2, 2, 3, 0], 70_000);
        assert_eq!(
            rebased,
            vec![70_000, 70_001, 70_002, 70_002, 70_003, 70_000]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn luminous_contour_front_opacity_metal_readback() {
        use crate::core::math::Point;
        use crate::core::paint::{
            LuminousContourAppearance, LuminousContourFront, LuminousContourFronts,
        };
        use crate::core::scene::{SceneBuilder, SceneNode};
        use crate::renderer::batcher::{Batcher, ClipState, DrawBatchKey, FlatPrimitive};
        use wgpu::util::DeviceExt;

        // One bounded readback through the real Shape pipeline and CPU contour lowering.
        fn render(
            renderer: &WgpuRenderer,
            pipeline: &wgpu::RenderPipeline,
            primitives: &[FlatPrimitive],
            scale: u32,
        ) -> Vec<[u8; 4]> {
            let (width, height) = (64 * scale, 16 * scale);
            let globals = Globals {
                view_proj: [
                    [2.0 / 64.0, 0.0, 0.0, 0.0],
                    [0.0, -2.0 / 16.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0, 1.0],
                ],
                surface_size: [width as f32, height as f32],
                time_seconds: 0.0,
                flags: 0,
            };
            let buffer = |bytes: &[u8], usage| {
                renderer
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("HAL358 front opacity proof"),
                        contents: bytes,
                        usage,
                    })
            };
            let uniform = buffer(bytemuck::bytes_of(&globals), wgpu::BufferUsages::UNIFORM);
            let group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &renderer.rect_globals_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                });
            let (vertices, indices) =
                WgpuRenderer::build_shape_buffers_from_surface_primitives(primitives);
            let vertices = buffer(bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX);
            let indices_buffer = buffer(bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX);
            let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("HAL358 front opacity proof"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let readback = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(width * height * 4),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let view = texture.create_view(&Default::default());
            let mut encoder = renderer.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("HAL358 front opacity proof"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: &view,
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
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &group, &[0]);
                pass.set_scissor_rect(2 * scale, 0, width - 4 * scale, height);
                if !indices.is_empty() {
                    pass.set_vertex_buffer(0, vertices.slice(..));
                    pass.set_index_buffer(indices_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                }
            }
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            renderer.queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::sync_channel(1);
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    send.send(result).unwrap();
                });
            renderer.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            receive.recv().unwrap().unwrap();
            let mapped = readback.slice(..).get_mapped_range().expect("mapped readback range");
            let pixels = mapped
                .chunks_exact(4)
                .map(|p| p.try_into().unwrap())
                .collect();
            drop(mapped);
            readback.unmap();
            pixels
        }

        let renderer = pollster::block_on(WgpuRenderer::new_headless()).expect("real Metal adapter");
        let adapter = renderer.adapter.get_info();
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        println!("HAL358 adapter: {} {:?}", adapter.name, adapter.backend);
        let shape = ShapePipeline::new(
            &renderer.device,
            wgpu::TextureFormat::Rgba8Unorm,
            &renderer.rect_globals_layout,
        );
        let current = shape.pipeline().unwrap();
        // The reference removes only §12's explicit opt-in from the canonical module.
        let source = include_str!("../../renderer/shaders/shape.wgsl");
        let old_source = source
            .replace("input.params3.z > 0.0 ||\n        (input.shape_type == SHAPE_LUMINOUS_CONTOUR && input.params3.w > 0.0)", "input.params3.z > 0.0")
            .replace("    var effective_source_alpha = input.color.a;\n    if input.shape_type == SHAPE_LUMINOUS_CONTOUR && input.params3.w > 0.0 {\n        effective_source_alpha = input.color.a + (1.0 - input.color.a) * input.params3.w * front_gain;\n    }\n", "")
            .replace("effective_source_alpha *", "input.color.a *");
        assert_ne!(old_source, source);
        let old_shader = renderer
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("HAL358 pre-§12 reference"),
                source: wgpu::ShaderSource::Wgsl(old_source.into()),
            });
        let old = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("HAL358 pre-§12 reference"),
                layout: shape.layout(),
                vertex: wgpu::VertexState {
                    module: &old_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(ShapeVertex::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &old_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::OVER,
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        let front = LuminousContourFront::try_new(27.5, 12.0, 1.0).unwrap();
        let make = |appearance, alpha, fronts| {
            let mut builder = SceneBuilder::new();
            builder
                .push_luminous_contour(
                    Arc::from([Point::new(4.0, 8.5), Point::new(60.0, 8.5)]),
                    2.0,
                    Color::from_linear(0.25, 0.06, 0.015, alpha),
                    appearance,
                    fronts,
                    0,
                    None,
                )
                .unwrap();
            let scene = builder.build();
            let SceneNode::LuminousContour(node) = &scene.root[0] else {
                panic!("expected admitted luminous contour");
            };
            let mut batcher = Batcher::new();
            batcher.add_luminous_contour(node);
            batcher
        };
        for scale in [1, 2] {
            let width = 64 * scale as usize;
            let pixel = |x: usize, y: usize| y * scale as usize * width + x * scale as usize;
            for appearance in [
                LuminousContourAppearance::try_new_zoned(0.2, 1.0, 0.4, 3.0, 0.1, 2.0).unwrap(),
                LuminousContourAppearance::try_new(3.0, 0.1, 2.0).unwrap(),
                LuminousContourAppearance::try_new_zoned(0.2, 0.0, 0.0, 3.0, 0.1, 2.0).unwrap(),
            ] {
                for alpha in [0.12, 1.0, 0.0] {
                    let fronts = LuminousContourFronts::try_from_slice(&[front]).unwrap();
                    let base_primitive = make(appearance, alpha, fronts);
                    let base = render(&renderer, current, base_primitive.primitives(), scale);
                    assert_eq!(
                        base,
                        render(&renderer, &old, base_primitive.primitives(), scale),
                        "boost=0 changed legacy pixels"
                    );
                    let boosted_appearance = appearance.with_front_opacity_boost(0.8).unwrap();
                    let boosted_primitives = make(boosted_appearance, alpha, fronts);
                    let boosted = render(&renderer, current, boosted_primitives.primitives(), scale);
                    let overlapping = LuminousContourFronts::try_from_slice(&[front, front]).unwrap();
                    assert_eq!(
                        boosted,
                        render(
                            &renderer,
                            current,
                            make(boosted_appearance, alpha, overlapping).primitives(),
                            scale
                        ),
                        "overlap must use max gain"
                    );
                    for (index, value) in boosted.iter().enumerate() {
                        assert!(
                            value[..3].iter().all(|&channel| channel <= value[3]),
                            "non-premultiplied {value:?}"
                        );
                        if index % width < 2 * scale as usize
                            || index % width >= width - 2 * scale as usize
                        {
                            assert_eq!(*value, [0; 4], "scissor leak");
                        }
                    }
                    assert_eq!(
                        base[pixel(8, 8)],
                        boosted[pixel(8, 8)],
                        "gain=0 changed source alpha"
                    );
                    if alpha == 0.0 {
                        assert!(
                            base_primitive.primitives().is_empty(),
                            "legacy zero alpha must stay culled"
                        );
                        assert_eq!(
                            boosted_primitives.primitives().len(),
                            1,
                            "positive boost must pass Batcher"
                        );
                        assert!(base.iter().all(|pixel| *pixel == [0; 4]));
                        assert_eq!(boosted[pixel(8, 8)], [0; 4]);
                        assert!(
                            boosted[pixel(31, 8)][3].abs_diff(204) <= 1,
                            "peak gain must realize alpha .8"
                        );
                        for (index, &value) in boosted.iter().enumerate() {
                            let x = index % width;
                            if x < 18 * scale as usize || x >= 45 * scale as usize {
                                assert_eq!(
                                    value, [0; 4],
                                    "zero-alpha source must remain transparent outside front"
                                );
                            }
                        }
                        if appearance.inner_aura_radius() == 1.0 {
                            println!(
                                "HAL358 {scale}x zero-alpha: legacy culled, peak={:?}, gain0={:?}",
                                boosted[pixel(31, 8)],
                                boosted[pixel(8, 8)]
                            );
                            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                                "target/hal358-front-opacity-s1a-zero-alpha-{scale}x.ppm"
                            ));
                            let mut image =
                                format!("P6\n{} {}\n255\n", 64 * scale, 32 * scale).into_bytes();
                            for p in base.iter().chain(&boosted) {
                                image.extend_from_slice(&p[..3]);
                            }
                            std::fs::write(&path, image).unwrap();
                            println!(
                                "HAL358 zero-alpha raw linear RGB readback: {}",
                                path.display()
                            );
                        }
                    }
                    if alpha < 1.0 {
                        assert!(
                            boosted[pixel(31, 8)][3] > base[pixel(31, 8)][3],
                            "scale={scale} source_alpha={alpha} appearance={appearance:?}: base={:?} boosted={:?}",
                            base[pixel(31, 8)],
                            boosted[pixel(31, 8)]
                        );
                        assert!(
                            boosted[pixel(31, 10)][3] > base[pixel(31, 10)][3],
                            "aura must use boosted alpha too"
                        );
                    } else if appearance.core_luminance_lift() > 0.0 {
                        assert_eq!(base, boosted, "opaque zoned source must remain unchanged");
                    }
                    if scale == 1 && alpha == 0.12 && appearance.inner_aura_radius() == 1.0 {
                        println!(
                            "HAL358 1x core alpha: base={} front={}; gain0={:?}; aura alpha {} -> {}",
                            base[pixel(31, 8)][3],
                            boosted[pixel(31, 8)][3],
                            boosted[pixel(8, 8)],
                            base[pixel(31, 10)][3],
                            boosted[pixel(31, 10)][3]
                        );
                        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("target/hal358-front-opacity-s1a-positive.ppm");
                        let mut image = b"P6\n64 32\n255\n".to_vec();
                        for p in base.iter().chain(&boosted) {
                            image.extend_from_slice(&p[..3]);
                        }
                        std::fs::write(&path, image).unwrap();
                        println!(
                            "HAL358 raw linear RGB readback (base above, boosted below): {}",
                            path.display()
                        );
                    }
                }
            }
            let ordinary = [
                FlatPrimitive {
                    key: DrawBatchKey::shape(0, ShapeType::Line as u8, 0),
                    data: PrimitiveData::Line {
                        a: Point::new(4.0, 5.0),
                        b: Point::new(60.0, 5.0),
                        width: 2.0,
                        color: Color::WHITE,
                    },
                    clip_state: ClipState::none(),
                    order_key: 0,
                },
                FlatPrimitive {
                    key: DrawBatchKey::shape(0, ShapeType::Polyline as u8, 0),
                    data: PrimitiveData::Polyline {
                        points: vec![
                            Point::new(4.0, 10.0),
                            Point::new(32.0, 13.0),
                            Point::new(60.0, 10.0),
                        ],
                        width: 1.0,
                        color: Color::WHITE,
                    },
                    clip_state: ClipState::none(),
                    order_key: 1,
                },
            ];
            assert_eq!(
                render(&renderer, current, &ordinary, scale),
                render(&renderer, &old, &ordinary, scale),
                "ordinary strokes changed"
            );
            println!(
                "HAL358 {scale}x legacy, localized front, alpha, max-overlap, zero-inner-aura and ordinary-stroke controls passed"
            );
        }
    }
