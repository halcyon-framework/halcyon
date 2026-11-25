use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::compositor::{Compositor, MultiSurfaceContract, SceneKey, SurfaceId};
use crate::core::animation::{Animation, AnimationSample, Easing, Timeline};
use crate::core::app::RetainedSceneTestHarness;
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::scene::{ImageTextureSource, SceneBuilder, SceneGraph, SceneNode};
use crate::core::theme::Theme;
use crate::core::widget::Widget;
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::renderer::CustomRenderParticipant;
use crate::renderer::batcher::{PipelineId, PrimitiveData};
use crate::renderer::custom_render_pass::audit::{
    self as custom_render_audit, CustomRenderAuditEvent,
};
use crate::renderer::custom_render_prepare::{
    CustomRenderActivity, CustomRenderPresentationBindings, CustomRenderTargetMetrics,
    CustomRenderTargetSnapshot,
};
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::{GpuDeviceFailureReason, WgpuRenderer};
use crate::renderer::{DrawPlan, DrawPlanBuilder, PhysicalExtent, PhysicalRect};
use crate::three_d::pbr::{
    MaterialPacketSource3D, MaterialSceneSource3D, PbrAlphaMode3D, PbrGeometrySource3D,
    PbrMaterialSource3D, PbrRole3D, PbrRoleBinding3D, PbrUnitDirection3D, PbrVertexSource3D,
};
use crate::three_d::{
    AppearanceMode, AppearanceSourceDescriptor, Context3D, Context3DError, DrawPacketDescriptor,
    GeometryIndices, GeometrySemantics, GeometrySourceDescriptor, LocalPoint3, ModelPlacement3,
    ModelRevision, ModelState, ModelStateDescriptor, Orientation3, PerspectiveProjection,
    PolicyBudget3D, PolicyLimits3D, PositiveScale3, RasterMode, RenderSnapshot,
    RenderSnapshotDescriptor, RigidPose3, SampleCount3D, StructuralRevision, ViewDescriptor3,
    Viewport3D, Viewport3DError, Viewport3DLifecycleState, Viewport3DViewRevision, WorldPoint3,
    viewport_audit,
};
use crate::widgets::panel::Panel;

#[cfg(target_os = "macos")]
use crate::three_d;
#[cfg(target_os = "macos")]
#[path = "../../tests/app_frame_realization_native/arch_floor_source.rs"]
mod arch_floor_source;

#[cfg(target_os = "macos")]
#[test]
fn hal_arch_floor_current_path_resource_custody_n1_n4() {
    use crate::three_d::direct_shadow::POOL_BYTES;
    use arch_floor_source::{EXTENT, FloorSource, MOVED_SLOT, OCCURRENCES, bounds, camera};

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let adapter = renderer.headless_adapter_for_test();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    let gpu = renderer.gpu_resources();
    assert!(gpu.device.features().is_empty());
    println!("arch_floor_ledger_adapter {adapter:?}");
    let [width, height] = EXTENT;
    let pixels = u64::from(width) * u64::from(height);

    for samples in [SampleCount3D::ONE, SampleCount3D::FOUR] {
        let n = u64::from(samples.value());
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let mut fixture = FloorSource::admit(&context).unwrap();
        let mut scene = fixture.scene(&context, samples, 1).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            scene.clone(),
            Viewport3DViewRevision::new(1),
            camera(false),
        )
        .unwrap();
        let metrics = active_metrics(355_540 + n, width, height, 2.0);
        let surface = metrics.surface_id;
        let steady = pixels * (20 * n + 16) + POOL_BYTES;
        let mut previous_binding = None::<crate::renderer::image_binding::ImageBinding>;
        let mut previous_direct = None;
        let mut previous_groups = None::<Vec<wgpu::BindGroup>>;
        for (step, (view_revision, swaps)) in [(1, 1), (2, 2), (2, 3), (2, 4), (2, 4)]
            .into_iter()
            .enumerate()
        {
            if step == 2 {
                let old = fixture.placements.clone();
                fixture.move_fixture();
                assert_eq!(
                    old.iter()
                        .zip(&fixture.placements)
                        .enumerate()
                        .filter_map(|(slot, (a, b))| (a != b).then_some(slot))
                        .collect::<Vec<_>>(),
                    [MOVED_SLOT],
                );
                assert_eq!(
                    bounds(old[MOVED_SLOT]).1.y,
                    bounds(fixture.placements[MOVED_SLOT]).1.y
                );
                scene = fixture.scene(&context, samples, 2).unwrap();
            }
            if step == 3 {
                scene = fixture.scene(&context, samples, 3).unwrap();
            }
            viewport.set_pbr_timing_for_test(true);
            if step > 0 {
                assert_eq!(
                    viewport
                        .request_pbr_scene(
                            scene.clone(),
                            Viewport3DViewRevision::new(view_revision),
                            camera(true),
                        )
                        .unwrap(),
                    step != 4,
                );
            }
            r024_prepare(&mut viewport, &metrics);
            let charged = context.target_charges_for_test();
            assert_eq!(charged.0, pixels);
            assert!(charged.1 == steady || charged.1 == steady + POOL_BYTES);
            assert_eq!(viewport.hdr_swaps_for_test(surface), swaps);
            let direct = viewport.direct_proof_for_test(surface).unwrap();
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            let objects = viewport.capacity_objects_for_test(surface).unwrap();
            let groups = &objects.bindings[6..];
            assert_eq!(groups.len(), OCCURRENCES);
            assert_eq!(
                groups
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                4
            );
            assert!(groups[2..28].iter().all(|g| g == &groups[2]));
            assert!(groups[28..].iter().all(|g| g == &groups[28]));
            if let Some(prior) = &previous_groups {
                if step == 4 {
                    assert_eq!(prior, groups);
                    assert_eq!(timing.material_group2_creations, 0);
                } else {
                    assert!(groups.iter().zip(prior).all(|(new, old)| new != old));
                    assert_eq!(timing.material_group2_creations, 4);
                }
            }
            let alignment = gpu
                .device
                .limits()
                .min_uniform_buffer_offset_alignment
                .max(16);
            for (offsets, size, arena) in [
                (&objects.material_offsets, 64u32, &objects.buffers[2]),
                (&objects.object_offsets, 112u32, &objects.buffers[1]),
            ] {
                let stride = size.div_ceil(alignment) * alignment;
                assert_eq!(offsets.len(), OCCURRENCES);
                assert!(
                    offsets
                        .iter()
                        .copied()
                        .eq((0..OCCURRENCES as u32).map(|i| i * stride))
                );
                assert_eq!(arena.size(), OCCURRENCES as u64 * u64::from(stride));
            }
            previous_groups = Some(groups.to_vec());
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                match step {
                    2 => (OCCURRENCES - 1, 1),
                    3 => (OCCURRENCES, 0),
                    _ => (0, 0),
                }
            );
            if let Some(prior) = previous_direct.as_ref() {
                assert_eq!(direct.same_source_basis(prior), matches!(step, 1 | 4));
                assert_eq!(direct.pool_identity == prior.pool_identity, step == 4);
            }
            if matches!(step, 2 | 3) {
                assert_eq!(
                    viewport.assert_direct_full_parity_for_test(surface),
                    (6_480, 19_440)
                );
            }
            assert_eq!(direct.faces, [0, 1, 2]);
            assert_eq!(direct.passes_encoded, 3);
            assert_eq!(direct.primary_and_face_bytes.len(), OCCURRENCES);
            assert!(
                direct
                    .primary_and_face_bytes
                    .iter()
                    .all(|bytes| *bytes == (36 * 52, 36 * 12))
            );
            let usage = context.resource_usage().unwrap();
            assert_eq!(usage.resident_geometry_bytes, 36 * (52 + 12));
            let dynamic = context.dynamic_state_accounting_for_test().unwrap();
            assert!(dynamic.retained_cpu_bytes > 0);
            assert!(dynamic.retained_cpu_bytes_high_water >= dynamic.retained_cpu_bytes);
            renderer.drain_pending_image_registrations();
            let id = viewport.target_texture_id_for_test(surface).unwrap();
            let binding = renderer.admit_image(id).unwrap();
            let front = viewport
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap();
            assert!(front.source().same_identity(&scene));
            if let Some(prior) = &previous_binding {
                assert_eq!(
                    prior.identity().texture_id(),
                    binding.identity().texture_id()
                );
                if step == 4 {
                    assert_eq!(prior.identity(), binding.identity());
                } else {
                    assert!(binding.identity().generation() > prior.identity().generation());
                    assert!(prior.availability().is_err());
                }
            }
            println!(
                "arch_floor_ledger N{n} step={step} view={view_revision} source={:?} serial={} swaps={swaps} registration={:?} geometry={:?} materials={:?} attachments={charged:?} resident_texture_bytes={} descriptor_cpu_bytes={} dynamic={dynamic:?}",
                scene.identity(),
                front.request_serial(),
                binding.identity(),
                fixture.geometry_generation,
                fixture.material_generations,
                usage.resident_texture_bytes,
                usage.retained_cpu_descriptor_bytes,
            );
            println!(
                "r025_floor_slots N{n} step={step} reused={} rebuilt={} incoming_cpu={} seed_cpu={} retained_cpu={} peak_retained_cpu={}",
                timing.direct_slots_reused,
                timing.direct_slots_rebuilt,
                timing.direct_slot_reserved_cpu_bytes,
                timing.direct_slot_seed_cpu_bytes,
                dynamic.retained_cpu_bytes,
                dynamic.retained_cpu_bytes_high_water,
            );
            previous_binding = Some(binding);
            previous_direct = Some(direct);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (pixels, steady));
        }
        drop(previous_groups);
        viewport.prepare_custom_render(&inactive_metrics(355_540 + n, width, height));
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 16 * pixels));
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        assert!(previous_binding.unwrap().availability().is_err());
        drop((viewport, scene));
        let retired = context.dynamic_state_accounting_for_test().unwrap();
        assert_eq!(retired.retained_model_states, 0);
        assert_eq!(retired.retained_cpu_bytes, 0);
        println!(
            "arch_floor_ledger N{n} retirement final_pair={} until renderer unregister, attachments=0 retained_dynamic_cpu=0",
            16 * pixels,
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn r025_incremental_optional_seed_denial_preserves_full_path_and_last_good() {
    use crate::three_d::pbr::PbrError3D;
    use arch_floor_source::{EXTENT, FloorSource, camera};

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let [w, h] = EXTENT;
    let metrics = active_metrics(355_548, w, h, 2.0);
    let surface = metrics.surface_id;
    let new_floor = |context: Arc<Context3D>| {
        let fixture = FloorSource::admit(&context).unwrap();
        let scene = fixture.scene(&context, SampleCount3D::ONE, 1).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context,
            scene.clone(),
            Viewport3DViewRevision::new(1),
            camera(false),
        )
        .unwrap();
        viewport.set_pbr_timing_for_test(true);
        r024_prepare(&mut viewport, &metrics);
        (fixture, scene, viewport)
    };
    let baseline = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let (_, _, viewport) = new_floor(baseline.clone());
    let seeded = viewport
        .pbr_timing_for_test()
        .unwrap()
        .raster
        .direct_slot_seed_cpu_bytes;
    assert!(seeded > 0);
    let charged = baseline.dynamic_state_accounting_for_test().unwrap();
    // The seed reservation coexists with the fully reserved flat preparation.
    let flat_peak = charged.retained_cpu_bytes_high_water - seeded;
    drop(viewport);
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_dynamic_cpu_bytes = flat_peak;
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
    let (mut fixture, scene, mut viewport) = new_floor(context.clone());
    assert_eq!(
        viewport
            .pbr_timing_for_test()
            .unwrap()
            .raster
            .direct_slot_seed_cpu_bytes,
        0
    );
    assert_eq!(
        viewport.assert_direct_full_parity_for_test(surface),
        (6_480, 19_440)
    );
    renderer.drain_pending_image_registrations();
    let image = viewport.target_texture_id_for_test(surface).unwrap();
    let binding = renderer.admit_image(image).unwrap();
    let swaps = viewport.hdr_swaps_for_test(surface);
    let previous = viewport
        .status_for(surface)
        .unwrap()
        .pbr_committed()
        .unwrap();
    let previous_serial = previous.request_serial();
    assert!(previous.source().same_identity(&scene));

    fixture.move_fixture();
    let next = fixture.scene(&context, SampleCount3D::ONE, 2).unwrap();
    viewport.set_pbr_timing_for_test(true);
    assert!(
        viewport
            .request_pbr_scene(next, Viewport3DViewRevision::new(1), camera(false))
            .unwrap()
    );
    viewport.prepare_custom_render(&metrics);
    assert!(matches!(
        viewport.material_failure_for_test(surface),
        Some(PbrError3D::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::RetainedDynamicCpuBytes,
            ..
        }))
    ));
    let timing = viewport.pbr_timing_for_test().unwrap().raster;
    assert_eq!(
        (timing.direct_slots_reused, timing.direct_slots_rebuilt),
        (0, 0)
    );
    assert_eq!(viewport.hdr_swaps_for_test(surface), swaps);
    let previous = viewport
        .status_for(surface)
        .unwrap()
        .pbr_committed()
        .unwrap();
    assert_eq!(previous.request_serial(), previous_serial);
    assert!(previous.source().same_identity(&scene));
    assert!(binding.availability().is_ok());
    println!(
        "r025_flat_budget cap={flat_peak} denied_optional_seed={seeded} successor={:?}",
        viewport.material_failure_for_test(surface)
    );
}

#[test]
fn hal375_zero_view_keeps_ordinary_clipped_pixels_and_schedule() {
    use crate::core::layout::FlexParams;
    use crate::core::paint::LAYER_OVERLAY;
    use crate::core::scene::{ClipNode, Color, RoundedCorners};
    use crate::prelude::{Flex, Label};
    use crate::renderer::sdf_frame_custody::SdfFramePlan;

    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("actual Metal renderer");
    let gpu = renderer.gpu_resources();
    let (w, h) = (96, 72);
    let rect = RectF::new(0.0, 0.0, w as f32, h as f32);
    let mut compositor = Compositor::new(rect);
    let surface = compositor.create_surface(0, rect, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (w, h), 1.0)
        .unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HAL375 ordinary registered Image"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &[255, 0, 0, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: None,
            rows_per_image: None,
        },
        texture.size(),
    );
    let texture_id = crate::utils::id::WidgetInternalId::new().raw();
    renderer.register_image_from_view(texture_id, texture.create_view(&Default::default()));

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    tree.add_child(root, Box::new(Label::new("Ordinary Label")), None, 0);
    let mut harness = RetainedSceneTestHarness::new();
    let snapshot =
        CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(w, h), 1.0).unwrap();
    let mut frame = |renderer: &mut WgpuRenderer, tree: &mut WidgetTree| {
        let retained = harness.frame_with_custom_render_target(
            tree,
            root,
            Constraints::tight(rect.size()),
            &Theme::default(),
            renderer.text_engine_mut(),
            &IconRegistry::new(&[]),
            snapshot,
        );
        let mut image = SceneBuilder::new();
        image.push_image(
            RectF::new(8.0, 28.0, 52.0, 32.0),
            texture_id,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            LAYER_CONTENT,
            None,
        );
        let mut overlay = SceneBuilder::new();
        overlay.push_rect(
            RectF::new(24.0, 36.0, 10.0, 10.0),
            Color::WHITE,
            LAYER_OVERLAY,
            None,
        );
        let mut scene = SceneGraph::new();
        scene.root = retained.scene_root;
        scene.root.push(SceneNode::Clip(ClipNode {
            rect: RectF::new(12.0, 30.0, 46.0, 26.0),
            rounded: Some(RoundedCorners::uniform(6.0)),
            children: image.build().root,
            z_index: LAYER_CONTENT,
            key: None,
        }));
        scene.root.extend(overlay.build().root);
        renderer.advance_frame();
        let frame = renderer.frame_counter();
        let mut occurrences = harness
            .custom_render_presentation_bindings_mut()
            .begin_materialization_for_test(frame);
        let draw_plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
            &std::collections::HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            &mut occurrences,
            |_| 1.0,
            frame,
        ).expect("scene must satisfy DrawPlan Build admission");
        renderer
            .compile_sdf_frame_with_presentation_bindings(
                SdfFramePlan::new(draw_plan, Default::default()),
                None,
                occurrences.freeze(),
            )
            .unwrap()
            .expect_ready("native Compile must be ready")
    };
    let plain = frame(&mut renderer, &mut tree);
    let data = plain.plan().surface_compiled_data(surface).unwrap();
    let text = data.text_realization().outcome().clone();
    assert!(text.is_complete());
    assert!(plain.companion_work().next().is_none());
    let commands = data.draw_commands.clone();
    let pixels = read_compiled_surface_pixels(&renderer, plain.plan(), surface, w, h);
    assert_eq!(
        pixels[(40 * w + 28) as usize],
        [255; 4],
        "ordinary overlay draws above Image"
    );
    assert_eq!(
        pixels[(42 * w + 40) as usize],
        [255, 0, 0, 255],
        "ordinary Image draws inside clip"
    );
    assert_ne!(
        pixels[(30 * w + 12) as usize],
        [255, 0, 0, 255],
        "rounded clip masks the corner"
    );

    let context = Arc::new(Context3D::new(gpu).unwrap());
    let source = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            Vec::new(),
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &source,
            ModelStateDescriptor::new(ModelRevision::new(1), Vec::new()),
        )
        .unwrap();
    let viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        hal374_flat_view(),
        source,
        model,
        SampleCount3D::FOUR,
    )
    .unwrap();
    let inactive = tree.add_child(root, Box::new(viewport), None, 1);
    tree.set_flex(inactive, FlexParams::fixed(0.0));
    let before = custom_render_audit::snapshot();
    let after = frame(&mut renderer, &mut tree);
    assert_eq!(custom_render_audit::snapshot(), before);
    assert_eq!(context.target_charges_for_test(), (0, 0));
    assert_eq!(
        tree_viewport(&tree, inactive)
            .status_for(surface)
            .unwrap()
            .state(),
        Viewport3DLifecycleState::Inactive
    );
    assert!(after.companion_work().next().is_none());
    let data = after.plan().surface_compiled_data(surface).unwrap();
    assert_eq!(data.text_realization().outcome(), &text);
    assert_eq!(data.draw_commands, commands);
    assert_eq!(
        after.plan().render_graph_topology.execution_order().len(),
        2
    );
    assert_eq!(
        read_compiled_surface_pixels(&renderer, after.plan(), surface, w, h),
        pixels
    );
}

#[test]
fn hal375_canonical_depth_sample_usage_n1_n4() {
    let (renderer, _, _) = setup(PolicyLimits3D::conservative()).expect("actual Metal renderer");
    let gpu = renderer.gpu_resources();
    assert!(gpu.device.features().is_empty());
    let usages = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    let format = wgpu::TextureFormat::Depth32Float;
    let capabilities = format.guaranteed_format_features(gpu.device.features());
    assert!(capabilities.allowed_usages.contains(usages));
    for n in [1, 4] {
        assert!(capabilities.flags.sample_count_supported(n));
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let depth = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HAL375 exact canonical depth format and usage"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: n,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: usages,
            view_formats: &[],
        });
        let view = depth.create_view(&Default::default());
        let depth_type = if n == 1 {
            "texture_depth_2d"
        } else {
            "texture_depth_multisampled_2d"
        };
        let sample = if n == 1 { "0" } else { "i32(i)" };
        let source = format!(
            r#"
@group(0) @binding(0) var depth: {depth_type};
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@compute @workgroup_size(1) fn main() {{
    for (var i = 0u; i < {n}u; i += 1u) {{
        output[i] = textureLoad(depth, vec2<i32>(0), {sample});
    }}
}}
"#
        );
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("HAL375 actual depth loads"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("HAL375 sampled depth feasibility"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let size = u64::from(n) * 4;
        let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HAL375 real depth producer"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.625),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        }));
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &binding, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
        let error = pollster::block_on(validation_scope.pop());
        assert!(error.is_none(), "N{n} depth usage/load refused: {error:?}");
        let submission = gpu.queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = slice.get_mapped_range().expect("mapped readback range");
        let depths: &[f32] = bytemuck::cast_slice(&bytes);
        assert_eq!(depths, &vec![0.625; n as usize]);
        eprintln!("HAL375 actual Metal Features::empty N{n} Depth32Float loads {depths:?}");
    }
}

mod selection_and_loading;
use self::selection_and_loading::{hal374_flat_view, read_compiled_surface_pixels};

fn hal372_quad_source(
    context: &Context3D,
    samples: u32,
    occlusion: bool,
) -> crate::three_d::PbrScene3D {
    use crate::three_d::hdr::HdrSource3D;
    let vertices = [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ]
    .into_iter()
    .map(|p| PbrVertexSource3D {
        position: LocalPoint3::try_new(p[0], p[1], p[2]).unwrap(),
        normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
        uv0: None,
        color: None,
        tangent: None,
    })
    .collect();
    let geometry = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(
                vertices,
                GeometryIndices::from_u16(vec![0, 1, 2, 0, 2, 3]),
            )
            .unwrap(),
            None,
        )
        .unwrap();
    let mut packets = Vec::new();
    let mut placements = Vec::new();
    for i in 0..if occlusion { 2 } else { 1 } {
        let mut material = PbrMaterialSource3D::defaults(372100 + i, 1);
        material.emissive_rgb = if i == 0 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let material = context.register_pbr_material(material, None).unwrap();
        packets.push(MaterialPacketSource3D::PbrMaterial {
            geometry: geometry.clone(),
            material,
            raster: RasterMode::SingleSided,
            stable_ordinal: i,
        });
        placements.push(
            ModelPlacement3::try_new(
                WorldPoint3::try_new(0.0, 0.0, if occlusion && i == 0 { 1.0 } else { 0.0 })
                    .unwrap(),
                Orientation3::identity(),
                PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
            )
            .unwrap(),
        );
    }
    let snapshot = context
        .admit_material_snapshot(StructuralRevision::new(1), packets)
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(1), placements)
        .unwrap();
    let source = MaterialSceneSource3D {
        snapshot,
        model,
        hdr: HdrSource3D {
            identity: 372200,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [1.0; 3],
        },
        samples,
        direct: None,
        indirect: None,
        observe: false,
    };
    source.validate(context).unwrap();
    crate::three_d::PbrScene3D(Arc::new(source))
}

fn hal372_extent(
    context: &Context3D,
    source: &crate::three_d::PbrScene3D,
) -> crate::three_d::PbrObjectRevealExtent3D {
    let (job, pending) = context
        .begin_pbr_object_reveal_extent(source, 1 << 20)
        .unwrap();
    let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))))
        .join()
        .unwrap()
        .unwrap();
    context
        .finish_pbr_object_reveal_extent(pending, result, source)
        .unwrap()
}

fn hal372_update(viewport: &mut Viewport3D, timeline: &mut Timeline, seconds: f64) {
    timeline.update(seconds).unwrap();
    let mut cx = crate::core::widget::UpdateContext::new(Some(
        crate::core::animation::AnimationContext::new(timeline),
    ));
    viewport.update(0.0, &mut cx);
}

fn hal372_copy_scene_members(source: &crate::three_d::PbrScene3D) -> MaterialSceneSource3D {
    MaterialSceneSource3D {
        snapshot: source.0.snapshot.clone(),
        model: source.0.model.clone(),
        hdr: source.0.hdr.clone(),
        samples: source.0.samples,
        direct: source.0.direct.clone(),
        indirect: source.0.indirect.clone(),
        observe: source.0.observe,
    }
}

fn hal372_independent_source(
    context: &Context3D,
    samples: u32,
    identity: u64,
    ev: f32,
) -> crate::three_d::PbrScene3D {
    let source = hal372_quad_source(context, samples, false);
    let mut independent = hal372_copy_scene_members(&source);
    independent.hdr.identity = identity;
    independent.hdr.exposure_ev = ev;
    independent.validate(context).unwrap();
    crate::three_d::PbrScene3D(Arc::new(independent))
}

#[test]
fn hal372_explicit_independent_scene_replacement_aba_n1_n4() {
    use crate::three_d::{OrthographicProjection, PbrObjectRevealRequest3D};
    use std::time::Duration;
    for samples in [1, 4] {
        let (renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("actual PBR backend");
        let gpu = renderer.gpu_resources();
        let sources = [
            hal372_independent_source(&context, samples, 372_301, 0.0),
            hal372_independent_source(&context, samples, 372_302, 1.0),
        ];
        let extents = [
            hal372_extent(&context, &sources[0]),
            hal372_extent(&context, &sources[1]),
        ];
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(2.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let metrics = active_metrics(37230, 32, 32, 1.0);
        let mut reference = Vec::new();
        for source in &sources {
            let mut baseline = Viewport3D::new_pbr(
                context.clone(),
                source.clone(),
                Viewport3DViewRevision::new(1),
                view,
            )
            .unwrap();
            baseline.prepare_custom_render(&metrics);
            assert!(baseline.pbr_failure(metrics.surface_id).is_none());
            reference.push(read_r027_fp16_at(
                baseline.hdr_front_for_test(metrics.surface_id),
                &gpu,
                4,
                24,
            ));
        }
        assert_ne!(
            reference[0].map(f32::to_bits),
            reference[1].map(f32::to_bits)
        );
        let mut viewport = Viewport3D::new_pbr(
            context,
            sources[0].clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        viewport.prepare_custom_render(&metrics);
        let key = viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .target_key();
        let mut serial = viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .request_serial();
        let mut timeline = Timeline::new();
        for (step, selected) in [1usize, 0, 1, 0].into_iter().enumerate() {
            let revision = step as u64 + 1;
            let time = step as f64;
            assert!(
                viewport
                    .replace_pbr_scene(
                        sources[selected].clone(),
                        Viewport3DViewRevision::new(1),
                        view
                    )
                    .unwrap()
            );
            assert_eq!(viewport.reveal_progress_for_test(), None);
            assert!(
                viewport
                    .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                        source: sources[selected].clone(),
                        request_revision: revision,
                        duration: Duration::from_millis(900),
                        enabled: true,
                        extent: Some(extents[1 - selected].clone())
                    })
                    .is_err()
            );
            let request = PbrObjectRevealRequest3D {
                source: sources[selected].clone(),
                request_revision: revision,
                duration: Duration::from_millis(900),
                enabled: true,
                extent: Some(extents[selected].clone()),
            };
            assert!(viewport.request_pbr_object_reveal(request.clone()).unwrap());
            hal372_update(&mut viewport, &mut timeline, time);
            viewport.prepare_custom_render(&metrics);
            assert!(
                viewport.pbr_failure(metrics.surface_id).is_none(),
                "{:?}",
                viewport.pbr_failure(metrics.surface_id)
            );
            assert_eq!(
                read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 24),
                [0.0; 4]
            );
            let tuple = viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .pbr_committed()
                .unwrap();
            assert!(tuple.source().same_identity(&sources[selected]));
            assert_eq!(tuple.target_key(), key);
            assert!(tuple.request_serial() > serial);
            assert_eq!(tuple.structural_revision(), StructuralRevision::new(1));
            assert_eq!(tuple.model_revision(), ModelRevision::new(1));
            serial = tuple.request_serial();
            hal372_update(&mut viewport, &mut timeline, time);
            hal372_update(&mut viewport, &mut timeline, time + 0.45);
            viewport.prepare_custom_render(&metrics);
            assert_eq!(
                read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 8),
                [0.0; 4]
            );
            assert_eq!(
                read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 24)
                    .map(f32::to_bits),
                reference[selected].map(f32::to_bits)
            );
            assert!(!viewport.request_pbr_object_reveal(request).unwrap());
            hal372_update(&mut viewport, &mut timeline, time + 0.9);
            viewport.prepare_custom_render(&metrics);
            assert_eq!(viewport.reveal_progress_for_test(), Some(1.0));
            assert!(!viewport.needs_update());
            assert_eq!(
                read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 8)
                    .map(f32::to_bits),
                reference[selected].map(f32::to_bits)
            );
            assert!(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .pbr_committed()
                    .unwrap()
                    .source()
                    .same_identity(&sources[selected])
            );
        }
    }
}

#[test]
fn hal372_independent_replacement_refusals_preserve_request_front_and_successor_law() {
    let (renderer, context, view) = setup(PolicyLimits3D::conservative()).unwrap();
    let a = hal372_independent_source(&context, 4, 372_401, 0.0);
    let b = hal372_independent_source(&context, 4, 372_402, 1.0);
    let foreign = Context3D::new(renderer.gpu_resources()).unwrap();
    let foreign = hal372_independent_source(&foreign, 4, 372_403, 0.0);
    let mut viewport = Viewport3D::new_pbr(
        context.clone(),
        a.clone(),
        Viewport3DViewRevision::new(1),
        view,
    )
    .unwrap();
    let metrics = active_metrics(37240, 32, 32, 1.0);
    viewport.prepare_custom_render(&metrics);
    let original = viewport
        .status_for(metrics.surface_id)
        .unwrap()
        .pbr_committed()
        .unwrap();
    let serial = original.request_serial();
    let key = original.target_key();
    let image = viewport.target_texture_id_for_test(metrics.surface_id);
    let unchanged = |viewport: &mut Viewport3D| {
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
        let status = viewport.status_for(metrics.surface_id).unwrap();
        assert_eq!(status.pbr_requested().unwrap().request_serial(), serial);
        assert_eq!(status.pbr_committed().unwrap().request_serial(), serial);
        assert!(status.pbr_requested().unwrap().source().same_identity(&a));
        assert!(status.pbr_committed().unwrap().source().same_identity(&a));
        assert_eq!(status.pbr_committed().unwrap().target_key(), key);
        assert_eq!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            image
        );
    };
    assert!(
        viewport
            .request_pbr_scene(b.clone(), Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    unchanged(&mut viewport);
    let mut same_snapshot_new_hdr = hal372_copy_scene_members(&a);
    same_snapshot_new_hdr.hdr.identity = 372_404;
    assert!(
        viewport
            .request_pbr_scene(
                crate::three_d::PbrScene3D(Arc::new(same_snapshot_new_hdr)),
                Viewport3DViewRevision::new(1),
                view
            )
            .is_err()
    );
    unchanged(&mut viewport);
    assert!(
        viewport
            .replace_pbr_scene(a.clone(), Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    unchanged(&mut viewport);
    let same_domain = crate::three_d::PbrScene3D(Arc::new(hal372_copy_scene_members(&a)));
    assert!(
        viewport
            .replace_pbr_scene(same_domain, Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    unchanged(&mut viewport);
    assert!(
        viewport
            .replace_pbr_scene(foreign, Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    unchanged(&mut viewport);
    let mut invalid = hal372_copy_scene_members(&b);
    invalid.model = a.0.model.clone();
    assert!(
        viewport
            .replace_pbr_scene(
                crate::three_d::PbrScene3D(Arc::new(invalid)),
                Viewport3DViewRevision::new(1),
                view
            )
            .is_err()
    );
    unchanged(&mut viewport);
    assert!(
        viewport
            .replace_pbr_scene(b.clone(), Viewport3DViewRevision::new(1), view_at(1.0))
            .is_err()
    );
    unchanged(&mut viewport);
    let MaterialPacketSource3D::PbrMaterial { geometry, .. } = &b.0.snapshot.source_packets()[0]
    else {
        unreachable!()
    };
    context.retire_pbr_geometry(geometry).unwrap();
    assert!(
        viewport
            .replace_pbr_scene(b, Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    unchanged(&mut viewport);
    let mut successor = hal372_copy_scene_members(&a);
    successor.hdr.revision += 1;
    let successor = crate::three_d::PbrScene3D(Arc::new(successor));
    assert!(
        viewport
            .request_pbr_scene(successor.clone(), Viewport3DViewRevision::new(1), view)
            .unwrap()
    );
    viewport.prepare_custom_render(&metrics);
    assert!(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .source()
            .same_identity(&successor)
    );
    let successor_serial = viewport
        .status_for(metrics.surface_id)
        .unwrap()
        .pbr_committed()
        .unwrap()
        .request_serial();
    assert!(
        viewport
            .replace_pbr_scene(a, Viewport3DViewRevision::new(1), view)
            .is_err(),
        "same-domain regression cannot be called a replacement"
    );
    viewport.prepare_custom_render(&metrics);
    let preserved = viewport.status_for(metrics.surface_id).unwrap();
    assert!(
        preserved
            .pbr_requested()
            .unwrap()
            .source()
            .same_identity(&successor)
    );
    assert!(
        preserved
            .pbr_committed()
            .unwrap()
            .source()
            .same_identity(&successor)
    );
    assert_eq!(
        preserved.pbr_requested().unwrap().request_serial(),
        successor_serial
    );
    let c = hal372_independent_source(&context, 4, 372_405, 0.0);
    renderer.gpu_resources().device.destroy();
    let _ = renderer.gpu_resources().device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        viewport
            .replace_pbr_scene(c, Viewport3DViewRevision::new(1), view)
            .is_err()
    );
}

#[test]
fn hal372_independent_replacement_failed_last_good_and_explicit_no_effect() {
    use crate::three_d::PbrObjectRevealRequest3D;
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 2;
    let (_renderer, context, view) = setup(policy).unwrap();
    let a = hal372_independent_source(&context, 4, 372_501, 0.0);
    let b = hal372_independent_source(&context, 4, 372_502, 1.0);
    let extent = hal372_extent(&context, &b);
    let metrics = active_metrics(37250, 32, 32, 1.0);
    let mut viewport =
        Viewport3D::new_pbr(context, a.clone(), Viewport3DViewRevision::new(1), view).unwrap();
    viewport.prepare_custom_render(&metrics);
    let image = viewport.target_texture_id_for_test(metrics.surface_id);
    viewport
        .replace_pbr_scene(b.clone(), Viewport3DViewRevision::new(1), view)
        .unwrap();
    viewport
        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
            source: b.clone(),
            request_revision: 1,
            duration: std::time::Duration::from_millis(900),
            enabled: true,
            extent: Some(extent),
        })
        .unwrap();
    let mut timeline = Timeline::new();
    hal372_update(&mut viewport, &mut timeline, 0.0);
    viewport.prepare_custom_render(&metrics);
    assert!(viewport.pbr_failure(metrics.surface_id).is_some());
    let status = viewport.status_for(metrics.surface_id).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert!(status.pbr_requested().unwrap().source().same_identity(&b));
    assert!(status.pbr_committed().unwrap().source().same_identity(&a));
    assert_eq!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        image
    );
    viewport
        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
            source: b.clone(),
            request_revision: 2,
            duration: std::time::Duration::ZERO,
            enabled: false,
            extent: None,
        })
        .unwrap();
    hal372_update(&mut viewport, &mut timeline, 0.01);
    viewport.prepare_custom_render(&metrics);
    assert!(viewport.pbr_failure(metrics.surface_id).is_none());
    assert!(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .source()
            .same_identity(&b)
    );
    assert!(!viewport.needs_update());
}

#[test]
fn hal372_actual_pbr_planar_coverage_and_terminal_n1_n4() {
    use crate::three_d::{OrthographicProjection, PbrObjectRevealRequest3D};
    use std::time::Duration;
    for samples in [1, 4] {
        let (renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("HAL372 requires actual backend output");
        let gpu = renderer.gpu_resources();
        let source = hal372_quad_source(&context, samples, false);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(2.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let metrics = active_metrics(3721, 32, 32, 1.0);
        let mut baseline = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        baseline.prepare_custom_render(&metrics);
        assert!(baseline.pbr_failure(metrics.surface_id).is_none());
        let original =
            read_r027_fp16_at(baseline.hdr_front_for_test(metrics.surface_id), &gpu, 4, 24);
        assert!(original[0] > 0.0 && original[3] == 1.0);
        let extent = hal372_extent(&context, &source);
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        let request = PbrObjectRevealRequest3D {
            source: source.clone(),
            request_revision: 1,
            duration: Duration::from_millis(160),
            enabled: true,
            extent: Some(extent),
        };
        assert!(viewport.request_pbr_object_reveal(request.clone()).unwrap());
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        assert!(
            viewport.pbr_failure(metrics.surface_id).is_none(),
            "{:?}",
            viewport.pbr_failure(metrics.surface_id)
        );
        assert_eq!(
            read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 24),
            [0.0; 4]
        );
        hal372_update(&mut viewport, &mut timeline, 0.0); // first admitted p0 front starts the sole Timeline
        for (time, p, hidden, kept) in [(0.04, 0.25, 23, 24), (0.08, 0.5, 15, 16)] {
            hal372_update(&mut viewport, &mut timeline, time);
            assert_eq!(viewport.reveal_progress_for_test(), Some(p));
            viewport.prepare_custom_render(&metrics);
            assert!(viewport.pbr_failure(metrics.surface_id).is_none());
            assert_eq!(
                read_r027_fp16_at(
                    viewport.hdr_front_for_test(metrics.surface_id),
                    &gpu,
                    4,
                    hidden
                ),
                [0.0; 4]
            );
            assert_eq!(
                read_r027_fp16_at(
                    viewport.hdr_front_for_test(metrics.surface_id),
                    &gpu,
                    4,
                    kept
                )
                .map(f32::to_bits),
                original.map(f32::to_bits)
            );
        }
        assert!(!viewport.request_pbr_object_reveal(request).unwrap());
        hal372_update(&mut viewport, &mut timeline, 0.16);
        viewport.prepare_custom_render(&metrics);
        assert_eq!(viewport.reveal_progress_for_test(), Some(1.0));
        assert!(!viewport.needs_update());
        for y in [1, 15, 16, 23, 24, 30] {
            assert_eq!(
                read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, y)
                    .map(f32::to_bits),
                read_r027_fp16_at(baseline.hdr_front_for_test(metrics.surface_id), &gpu, 4, y)
                    .map(f32::to_bits)
            );
        }
        viewport
            .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                source,
                request_revision: 2,
                duration: Duration::ZERO,
                enabled: false,
                extent: None,
            })
            .unwrap();
        hal372_update(&mut viewport, &mut timeline, 0.17);
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
        assert!(!viewport.needs_update());
        assert_eq!(
            read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 1)
                .map(f32::to_bits),
            original.map(f32::to_bits)
        );
    }
}

#[test]
fn hal372_actual_hidden_front_does_not_occlude_background_n1_n4() {
    use crate::three_d::{OrthographicProjection, PbrObjectRevealRequest3D};
    for samples in [1, 4] {
        let (renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("HAL372 depth proof requires a Device");
        let gpu = renderer.gpu_resources();
        let source = hal372_quad_source(&context, samples, true);
        let q = glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 3.0, 3.0).unwrap(),
                Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap(),
            ),
            OrthographicProjection::try_new(4.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let metrics = active_metrics(3722, 32, 32, 1.0);
        let extent = hal372_extent(&context, &source);
        let mut viewport = Viewport3D::new_pbr(
            context,
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        viewport.prepare_custom_render(&metrics);
        let front = read_r027_fp16_at(
            viewport.hdr_front_for_test(metrics.surface_id),
            &gpu,
            16,
            18,
        );
        assert!(front[0] > 0.0 && front[1] == 0.0 && front[3] == 1.0);
        viewport
            .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                source,
                request_revision: 1,
                duration: std::time::Duration::from_millis(160),
                enabled: true,
                extent: Some(extent),
            })
            .unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        hal372_update(&mut viewport, &mut timeline, 0.0);
        hal372_update(&mut viewport, &mut timeline, 0.08);
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
        let background = read_r027_fp16_at(
            viewport.hdr_front_for_test(metrics.surface_id),
            &gpu,
            16,
            18,
        );
        assert!(
            background[0] == 0.0 && background[1] > 0.0 && background[3] == 1.0,
            "hidden front wrote false depth: {background:?}"
        );
    }
}
#[test]
fn hal372_timeline_cadence_retarget_resize_and_source_retirement() {
    use crate::three_d::PbrObjectRevealRequest3D;
    use std::time::Duration;
    let (_renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
    let source = hal372_quad_source(&context, 4, false);
    let extent = hal372_extent(&context, &source);
    let view = r025_view();
    let metrics = active_metrics(3723, 32, 32, 1.0);
    for hz in [60, 120] {
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        let request = crate::three_d::PbrObjectRevealRequest3D {
            source: source.clone(),
            request_revision: 1,
            duration: Duration::from_millis(160),
            enabled: true,
            extent: Some(extent.clone()),
        };
        viewport.request_pbr_object_reveal(request.clone()).unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        hal372_update(&mut viewport, &mut timeline, 0.0);
        let mut previous = 0.0;
        for frame in 1..=((hz as f64 * 0.16).ceil() as u32) {
            let time = (f64::from(frame) / f64::from(hz)).min(0.16);
            hal372_update(&mut viewport, &mut timeline, time);
            let p = viewport.reveal_progress_for_test().unwrap();
            assert!(p >= previous && p <= 1.0);
            viewport.prepare_custom_render(&metrics);
            assert!(viewport.pbr_failure(metrics.surface_id).is_none());
            assert!(Arc::ptr_eq(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .source(),
                &source.0
            ));
            assert!(!viewport.request_pbr_object_reveal(request.clone()).unwrap());
            previous = p;
        }
        assert_eq!(previous, 1.0);
        assert!(
            !viewport.needs_update(),
            "terminal N4 occurrence must stop engine demand at {hz}Hz"
        );
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        viewport.prepare_custom_render(&metrics);
        assert_eq!(swaps, viewport.hdr_swaps_for_test(metrics.surface_id));
    }
    let mut viewport = Viewport3D::new_pbr(
        context.clone(),
        source.clone(),
        Viewport3DViewRevision::new(1),
        view,
    )
    .unwrap();
    let request = PbrObjectRevealRequest3D {
        source: source.clone(),
        request_revision: 1,
        duration: Duration::from_millis(160),
        enabled: true,
        extent: Some(extent.clone()),
    };
    viewport.request_pbr_object_reveal(request.clone()).unwrap();
    let mut timeline = Timeline::new();
    hal372_update(&mut viewport, &mut timeline, 0.0);
    viewport.prepare_custom_render(&metrics);
    hal372_update(&mut viewport, &mut timeline, 0.0);
    hal372_update(&mut viewport, &mut timeline, 0.06);
    viewport.prepare_custom_render(&metrics);
    assert_eq!(viewport.reveal_progress_for_test(), Some(0.375));
    let mut retarget = request.clone();
    retarget.request_revision = 2;
    retarget.duration = Duration::from_millis(80);
    viewport.request_pbr_object_reveal(retarget).unwrap();
    hal372_update(&mut viewport, &mut timeline, 0.06);
    assert_eq!(viewport.reveal_progress_for_test(), Some(0.375));
    hal372_update(&mut viewport, &mut timeline, 0.10);
    assert_eq!(viewport.reveal_progress_for_test(), Some(0.6875));
    viewport
        .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(2), view)
        .unwrap();
    let resized = active_metrics(3723, 48, 24, 1.0);
    viewport.prepare_custom_render(&resized);
    assert!(viewport.pbr_failure(metrics.surface_id).is_none());
    assert_eq!(viewport.reveal_progress_for_test(), Some(0.6875));
    let mut inactive = resized;
    inactive.activity = CustomRenderActivity::Inactive;
    viewport.prepare_custom_render(&inactive);
    hal372_update(&mut viewport, &mut timeline, 0.14);
    assert_eq!(viewport.reveal_progress_for_test(), Some(1.0));
    viewport.prepare_custom_render(&resized);
    assert!(!viewport.needs_update());
    // A new immutable source is a new occurrence, regardless of equal geometry.
    let old = &source.0;
    let successor = crate::three_d::PbrScene3D(Arc::new(MaterialSceneSource3D {
        snapshot: old.snapshot.clone(),
        model: old.model.clone(),
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: old.hdr.identity,
            revision: old.hdr.revision + 1,
            exposure_ev: 0.0,
            maximum_rgb: old.hdr.maximum_rgb,
        },
        samples: old.samples,
        direct: None,
        indirect: None,
        observe: false,
    }));
    viewport
        .request_pbr_scene(successor.clone(), Viewport3DViewRevision::new(3), view)
        .unwrap();
    assert_eq!(viewport.reveal_progress_for_test(), None);
    let mut stale = request;
    stale.request_revision = 3;
    assert!(viewport.request_pbr_object_reveal(stale).is_err());
    viewport
        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
            source: successor.clone(),
            request_revision: 3,
            duration: Duration::from_millis(160),
            enabled: true,
            extent: Some(hal372_extent(&context, &successor)),
        })
        .unwrap();
    assert_eq!(viewport.reveal_progress_for_test(), Some(0.0));
    viewport.on_unmount();
    assert!(!viewport.needs_update());
}

#[test]
fn hal372_original_complete_caster_shadow_survives_visual_discard_n1_n4() {
    use crate::three_d::PbrObjectRevealRequest3D;
    let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    for samples in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let q = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let rotation = Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap();
        let (source, view) = r025_occluder_source(&context, samples, 0, 1, rotation, 1);
        let source = crate::three_d::PbrScene3D(source);
        let metrics = active_metrics(3724, 32, 32, 1.0);
        let mut baseline = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        baseline.prepare_custom_render(&metrics);
        let shadowed =
            read_r027_fp16_at(baseline.hdr_front_for_test(metrics.surface_id), &gpu, 8, 16);
        let (uncast, _) = r025_occluder_source(&context, samples, 0, 0, rotation, 1);
        let mut uncast = r024_viewport(context.clone(), uncast, view);
        uncast.prepare_custom_render(&metrics);
        let lit = read_r027_fp16_at(uncast.hdr_front_for_test(metrics.surface_id), &gpu, 8, 16);
        assert!(
            lit[0] > shadowed[0],
            "fixture must have a real complete-source shadow: {lit:?}/{shadowed:?}"
        );
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        viewport
            .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                source: source.clone(),
                request_revision: 1,
                duration: std::time::Duration::from_millis(160),
                enabled: true,
                extent: Some(hal372_extent(&context, &source)),
            })
            .unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        hal372_update(&mut viewport, &mut timeline, 0.0);
        let before = context.resource_usage().unwrap();
        viewport.set_pbr_timing_for_test(true);
        hal372_update(&mut viewport, &mut timeline, 0.08);
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
        let timing = viewport.pbr_timing_for_test().unwrap().raster;
        assert_eq!(timing.branch, "Dynamic");
        assert_eq!(timing.vertices, 0, "progress cannot rescan source vertices");
        assert_eq!(
            timing.shadow_faces, 0,
            "original complete shadow maps remain reusable"
        );
        let after = context.resource_usage().unwrap();
        assert_eq!(
            after.resident_geometry_bytes,
            before.resident_geometry_bytes
        );
        assert_eq!(after.resident_texture_bytes, before.resident_texture_bytes);
        assert_eq!(
            after.admitted_shader_modules,
            before.admitted_shader_modules
        );
        assert_eq!(
            read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 8, 16)
                .map(f32::to_bits),
            shadowed.map(f32::to_bits)
        );
        // Rotation makes source-world Y equal local X: the WHOLE off-camera
        // caster at local X=.3..7 is hidden, but its shadow on X<0 survives.
        assert_eq!(
            read_r027_fp16_at(
                viewport.hdr_front_for_test(metrics.surface_id),
                &gpu,
                24,
                16
            ),
            [0.0; 4]
        );
    }
}

#[test]
fn hal372_r028_mask_normal_derivatives_and_coverage_n1_n4() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{
        PbrObjectRevealRequest3D, SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear,
    };
    let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let raw = (0..64 * 64)
        .flat_map(|i| {
            if i % 64 < 31 {
                [0, 255, 0, 0]
            } else {
                [255, 0, 0, 255]
            }
        })
        .collect();
    let chain = producer::produce(MipRole3D::MaskBase, 64, 64, raw, 372300, 1, true);
    let base = r028_role(
        &context,
        chain,
        372301,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let normal = r028_role(
        &context,
        producer::produce(
            MipRole3D::Normal,
            4,
            4,
            [128, 128, 255, 255].repeat(16),
            372302,
            1,
            true,
        ),
        372303,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    for samples in [1, 4] {
        let mut material = PbrMaterialSource3D::defaults(372304, 1);
        material.sampling = SamplingFamily3D::V2;
        material.alpha_mode = PbrAlphaMode3D::Mask;
        material.roles[0] = Some(base.0.clone());
        material.roles[1] = Some(normal.0.clone());
        material.emissive_rgb = [0.25, 0.5, 0.75];
        let mut vertices = r028_quad([1.0; 2], [0.0; 2]);
        for vertex in &mut vertices {
            vertex.tangent = Some((PbrUnitDirection3D::try_new([1.0, 0.0, 0.0]).unwrap(), -1.0));
        }
        let source = crate::three_d::PbrScene3D(r024_source(
            &context,
            vertices,
            material,
            samples,
            false,
            r024_identity(),
            1,
        ));
        let metrics = active_metrics(3725, 32, 32, 1.0);
        let mut baseline = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            r024_view(2.0),
        )
        .unwrap();
        baseline.prepare_custom_render(&metrics);
        assert!(baseline.pbr_failure(metrics.surface_id).is_none());
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            r024_view(2.0),
        )
        .unwrap();
        viewport
            .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                source: source.clone(),
                request_revision: 1,
                duration: std::time::Duration::from_millis(160),
                enabled: true,
                extent: Some(hal372_extent(&context, &source)),
            })
            .unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        hal372_update(&mut viewport, &mut timeline, 0.0);
        hal372_update(&mut viewport, &mut timeline, 0.08);
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
        // Actual production pbr_fragment, not the independent test observer.
        // The V2 implicit-derivative Mask edge crosses invocation/helper quads.
        for y in [8, 15, 16, 24] {
            for x in [4, 14, 15, 24] {
                let partial =
                    read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, x, y);
                if y < 16 {
                    assert_eq!(partial, [0.0; 4]);
                } else {
                    assert_eq!(
                        partial.map(f32::to_bits),
                        read_r027_fp16_at(
                            baseline.hdr_front_for_test(metrics.surface_id),
                            &gpu,
                            x,
                            y
                        )
                        .map(f32::to_bits),
                        "V2 source sampling/normal/derivatives changed at ({x},{y})"
                    );
                }
            }
        }
        assert_eq!(
            read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 4, 24),
            [0.0; 4]
        );
        assert_eq!(
            read_r027_fp16_at(viewport.hdr_front_for_test(metrics.surface_id), &gpu, 24, 8),
            [0.0; 4]
        );
        assert_eq!(
            read_r027_fp16_at(
                viewport.hdr_front_for_test(metrics.surface_id),
                &gpu,
                24,
                24
            )
            .map(f32::to_bits),
            read_r027_fp16_at(
                baseline.hdr_front_for_test(metrics.surface_id),
                &gpu,
                24,
                24
            )
            .map(f32::to_bits)
        );
    }
}

#[test]
fn hal372_capacity_last_good_disable_and_terminal_device() {
    use crate::three_d::PbrObjectRevealRequest3D;
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 2; // Existing Tone + Object112 fill the real module quota.
    let (renderer, context, _) = setup(policy).unwrap();
    let gpu = renderer.gpu_resources();
    let source = hal372_quad_source(&context, 4, false);
    let extent = hal372_extent(&context, &source);
    let metrics = active_metrics(3726, 32, 32, 1.0);
    let mut viewport = Viewport3D::new_pbr(
        context.clone(),
        source.clone(),
        Viewport3DViewRevision::new(1),
        r025_view(),
    )
    .unwrap();
    viewport.prepare_custom_render(&metrics);
    let texture = viewport.target_texture_id_for_test(metrics.surface_id);
    let usage = context.resource_usage().unwrap();
    viewport
        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
            source: source.clone(),
            request_revision: 1,
            duration: std::time::Duration::from_millis(160),
            enabled: true,
            extent: Some(extent),
        })
        .unwrap();
    let mut timeline = Timeline::new();
    hal372_update(&mut viewport, &mut timeline, 0.0);
    viewport.prepare_custom_render(&metrics);
    assert!(matches!(
        viewport.pbr_failure(metrics.surface_id),
        Some(crate::three_d::PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ShaderModules,
                ..
            }
        ))
    ));
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::FailedWithLastGood
    );
    assert_eq!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        texture
    );
    assert_eq!(
        context.resource_usage().unwrap().admitted_shader_modules,
        usage.admitted_shader_modules
    );
    assert!(
        !viewport.needs_update(),
        "refused cold occurrence must not poll for capacity"
    );
    viewport
        .request_pbr_object_reveal(PbrObjectRevealRequest3D {
            source: source.clone(),
            request_revision: 2,
            duration: std::time::Duration::ZERO,
            enabled: false,
            extent: None,
        })
        .unwrap();
    hal372_update(&mut viewport, &mut timeline, 0.01);
    viewport.prepare_custom_render(&metrics);
    assert!(viewport.pbr_failure(metrics.surface_id).is_none());
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::Committed
    );
    assert_eq!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        texture
    );
    let (job, pending) = context
        .begin_pbr_object_reveal_extent(&source, 1 << 20)
        .unwrap();
    let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        context
            .finish_pbr_object_reveal_extent(pending, result, &source)
            .is_err()
    );
    hal372_update(&mut viewport, &mut timeline, 0.02);
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::TerminalDeviceFailure
    );
    assert!(
        viewport
            .target_texture_id_for_test(metrics.surface_id)
            .is_none()
    );
    assert!(!viewport.needs_update());
}

#[test]
fn hal372_actual_all_eight_profiles_survivor_and_p1_parity_n1_n4() {
    use crate::three_d::PbrObjectRevealRequest3D;
    let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    for samples in [1, 4] {
        for bits in 0..8 {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let source = crate::three_d::PbrScene3D(r026_fixture::scene(&context, samples, bits));
            let metrics = active_metrics(3727, 32, 32, 1.0);
            let mut baseline = Viewport3D::new_pbr(
                context.clone(),
                source.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
            baseline.prepare_custom_render(&metrics);
            assert!(baseline.pbr_failure(metrics.surface_id).is_none());
            let original = read_r027_fp16_at(
                baseline.hdr_front_for_test(metrics.surface_id),
                &gpu,
                16,
                16,
            );
            assert_eq!(original[3], 1.0);
            let mut viewport = Viewport3D::new_pbr(
                context.clone(),
                source.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
            viewport
                .request_pbr_object_reveal(PbrObjectRevealRequest3D {
                    source: source.clone(),
                    request_revision: 1,
                    duration: std::time::Duration::from_millis(160),
                    enabled: true,
                    extent: Some(hal372_extent(&context, &source)),
                })
                .unwrap();
            let mut timeline = Timeline::new();
            hal372_update(&mut viewport, &mut timeline, 0.0);
            viewport.prepare_custom_render(&metrics);
            assert!(
                viewport.pbr_failure(metrics.surface_id).is_none(),
                "profile{bits} N{samples}: {:?}",
                viewport.pbr_failure(metrics.surface_id)
            );
            assert_eq!(
                read_r027_fp16_at(
                    viewport.hdr_front_for_test(metrics.surface_id),
                    &gpu,
                    16,
                    16
                ),
                [0.0; 4]
            );
            hal372_update(&mut viewport, &mut timeline, 0.0);
            for time in [0.08, 0.16] {
                hal372_update(&mut viewport, &mut timeline, time);
                viewport.prepare_custom_render(&metrics);
                assert!(viewport.pbr_failure(metrics.surface_id).is_none());
                assert_eq!(
                    read_r027_fp16_at(
                        viewport.hdr_front_for_test(metrics.surface_id),
                        &gpu,
                        16,
                        16
                    )
                    .map(f32::to_bits),
                    original.map(f32::to_bits),
                    "original R024/D/E/G radiance changed: profile{bits} N{samples}, time{time}"
                );
            }
            assert!(!viewport.needs_update());
        }
    }
}

mod pbr_material_sampling;
use self::pbr_material_sampling::{
    assert_no_runtime_gpu_model, consumer_pbr_geometry, consumer_pbr_source, r028_observations,
    r028_quad, r028_role,
};

mod texture_and_environment;
pub(super) use self::texture_and_environment::r026_fixture;
use self::texture_and_environment::{r028_presence_material, r028_presence_scene};

fn r024_view(span: f32) -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        crate::three_d::OrthographicProjection::try_new(span, 0.0, 10.0)
            .unwrap()
            .into(),
    )
}

fn r024_source(
    context: &Context3D,
    vertices: Vec<PbrVertexSource3D>,
    material: PbrMaterialSource3D,
    n: u32,
    observe: bool,
    placement: ModelPlacement3,
    revision: u64,
) -> Arc<MaterialSceneSource3D> {
    let double = material.double_sided;
    let g = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(vertices, GeometryIndices::Unindexed).unwrap(),
            None,
        )
        .unwrap();
    let m = context.register_pbr_material(material, None).unwrap();
    let snapshot = context
        .admit_material_snapshot(
            StructuralRevision::new(revision),
            vec![MaterialPacketSource3D::PbrMaterial {
                geometry: g,
                material: m,
                raster: if double {
                    crate::three_d::RasterMode::DoubleSided
                } else {
                    crate::three_d::RasterMode::SingleSided
                },
                stable_ordinal: 0,
            }],
        )
        .unwrap();
    let model = context
        .admit_material_model(
            snapshot.clone(),
            ModelRevision::new(revision),
            vec![placement],
        )
        .unwrap();
    Arc::new(MaterialSceneSource3D {
        snapshot,
        model,
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: 700,
            revision,
            exposure_ev: 0.0,
            maximum_rgb: [1.0; 3],
        },
        samples: n,
        direct: None,
        observe,
        indirect: None,
    })
}

fn r024_viewport(
    context: Arc<Context3D>,
    source: Arc<MaterialSceneSource3D>,
    view: ViewDescriptor3,
) -> Viewport3D {
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![],
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(ModelRevision::new(1), vec![]),
        )
        .unwrap();
    let mut viewport = Viewport3D::new(
        context,
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        model,
        SampleCount3D::ONE,
    )
    .unwrap();
    viewport
        .set_material_source(source, Viewport3DViewRevision::new(1), view)
        .unwrap();
    viewport
}

#[test]
fn r024_abc_private_metal_source_frame_n1_n4() {
    use super::three_d_geometry::r024_abc_fixture;
    let captured = |s: &MaterialSceneSource3D| MaterialSceneSource3D {
        snapshot: s.snapshot.clone(),
        model: s.model.clone(),
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: s.hdr.identity,
            revision: s.hdr.revision,
            exposure_ev: s.hdr.exposure_ev,
            maximum_rgb: s.hdr.maximum_rgb,
        },
        samples: s.samples,
        direct: s.direct.clone(),
        observe: s.observe,
        indirect: s.indirect.clone(),
    };
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("R024 ABC requires an actual Metal Device");
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R024 ABC actual {adapter:?}; wgpu30.0.2/Naga30.0.2; original fixed frame/direct/HDR programs\n")).unwrap();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        // The unchanged legal normal fixture already used by the original R025
        // test. It is the SAME immutable numeric texture for every A/B/C case.
        let texture = context
            .register_texture(
                crate::three_d::Texture3D::try_new(
                    1,
                    1,
                    crate::three_d::TextureFormat3D::Rgba8Unorm,
                    vec![128, 128, 255, 255],
                )
                .unwrap(),
            )
            .unwrap();
        let sampler = context
            .register_sampler(crate::three_d::Sampler3D::default())
            .unwrap();
        let mut material = PbrMaterialSource3D::defaults(92001, 1);
        material.base_rgba = [0.5, 0.5, 0.5, 1.0];
        material.metallic = 0.0;
        material.roughness = 1.0;
        material.double_sided = true;
        material.roles[1] = Some(PbrRoleBinding3D::V1 {
            texture: texture.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        let material = context.register_pbr_material(material, None).unwrap();
        let mut responses = [0.0f32; 3];
        for (case, (mode, negative_uv, back, partial)) in [
            (0, false, false, false),
            (1, false, false, false),
            (2, false, false, false),
            (2, true, false, false),
            (2, false, true, false),
            (2, false, false, true),
        ]
        .into_iter()
        .enumerate()
        {
            let (mut render, mut input) = r024_abc_fixture::packet(mode, negative_uv);
            if partial {
                // Same pre-existing covered-domain witness, now with explicit C
                // source provenance. No assumption about Metal sample positions.
                for (i, ([x, y], uv)) in [
                    [0.0, 35.0 / 256.0],
                    [-1.0, 28.0 / 256.0],
                    [1.0, 28.0 / 256.0],
                ]
                .into_iter()
                .zip([[0.5, 1.0], [0.0, 0.0], [1.0, 0.0]])
                .enumerate()
                {
                    render[i].position = [0.5 + x, -0.5 + y, 0.0];
                    render[i].uv0 = uv;
                    let original = &mut Arc::make_mut(&mut input.original.vertices)[i];
                    original.position = render[i].position;
                    original.uv0 = Some(uv);
                }
            }
            let geometry =
                PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                    .unwrap();
            let proof = geometry.certified_frames().unwrap()[0];
            let handle = context.register_pbr_geometry(geometry, None).unwrap();
            let revision = 2 + case as u64 * 4;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry: handle,
                        material: material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let placement = if back {
                ModelPlacement3::try_new(
                    WorldPoint3::origin(),
                    Orientation3::try_from_xyzw(0.0, 1.0, 0.0, 0.0).unwrap(),
                    PositiveScale3::try_new(2.0, 1.0, 4.0).unwrap(),
                )
                .unwrap()
            } else {
                r024_identity()
            };
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![placement],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 92002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.0, 1.0],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            // r=1 gives the existing conservative f*cos<=2 envelope. One
            // directional E=1 and zero emission/indirect make bound4 valid over
            // ALL surviving samples, independently of these readbacks.
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 92003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: true,
                indirect: None,
            });
            let view = if partial {
                ViewDescriptor3::new(
                    RigidPose3::new(
                        WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
                        Orientation3::identity(),
                    ),
                    crate::three_d::OrthographicProjection::try_new(32.0, 0.1, 10.0)
                        .unwrap()
                        .into(),
                )
            } else {
                r025_view()
            };
            let size = 32;
            // The ordinary widget's existing one-pixel chrome on each side
            // leaves the SAME32x32 actual render target inside a34x34 surface.
            let window_size = size + 2;
            let rect = RectF::new(0.0, 0.0, window_size as f32, window_size as f32);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (window_size, window_size), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, size, size, 1.0);
            let mut viewport = r024_viewport(context.clone(), source.clone(), view);
            r024_prepare(&mut viewport, &metrics);
            let observed_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            let covered = !partial || n == 4;
            if covered {
                let o = r025_observation_at(&viewport, surface, &gpu, size / 2, size / 2);
                assert!(o[..10].iter().flatten().all(|x| x.is_finite()));
                assert_eq!(o[6][3], 1.0, "actual direct material fragment must survive");
                assert!(o[7][3] as f64 >= proof.normal_lower - 1e-6);
                assert!(o[8][3] as f64 >= proof.local_g_lower - 1e-6);
                // Every fixture supplies constant local T=+X, and local N.x=0.
                // This is an independent source identity, so independent centroid
                // N/T locations cannot change the ideal local GS result g=+X.
                // gamma7 encloses a four-term f32 dot; two further rounded
                // operations cover quotient/product and the GS subtraction.
                let unit = 2.0f64.powi(-24);
                let gamma7 = 7.0 * unit / (1.0 - 7.0 * unit);
                let nn = o[7][..3].iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
                let gs_error = (gamma7 + 2.0 * unit) / (1.0 - gamma7) * nn.sqrt().max(1.0);
                for k in 0..3 {
                    assert!(
                        (f64::from(o[8][k]) - if k == 0 { 1.0 } else { 0.0 }).abs() <= gs_error,
                        "actual local GS {o:?}, arithmetic bound{gs_error}"
                    );
                }
                let q = [2.0 * 128.0 / 255.0 - 1.0, 2.0 * 128.0 / 255.0 - 1.0, 1.0f64];
                let local =
                    glam::DVec3::new(f64::from(o[7][0]), f64::from(o[7][1]), f64::from(o[7][2]));
                let normal = if back {
                    glam::DVec3::new(-local.x / 2.0, local.y, -local.z / 4.0).normalize()
                } else {
                    local.normalize()
                };
                let tangent = if back {
                    -glam::DVec3::X
                } else {
                    glam::DVec3::X
                };
                let sign = if negative_uv { -1.0 } else { 1.0 };
                let bitangent = sign * normal.cross(tangent);
                let mut expected = (tangent * q[0] + bitangent * q[1] + normal * q[2]).normalize();
                if back {
                    expected = -expected;
                }
                let actual =
                    glam::DVec3::new(f64::from(o[1][0]), f64::from(o[1][1]), f64::from(o[1][2]));
                assert!(
                    (actual - expected).length() < 2e-5,
                    "mapped normal actual{actual:?} expected{expected:?}"
                );
                assert!(
                    (actual - normal).length() > 0.002,
                    "normal role must actually affect the frame"
                );
                assert!(o[6][..3].iter().all(|x| *x > 0.0 && *x < 4.0));
                assert_eq!(
                    o[0][..3],
                    [0.0, 0.0, 1.0],
                    "R025 geometric normal stays original winding/receiving side"
                );
                if !partial {
                    for k in 0..3 {
                        let tone = o[6][k] / (1.0 + o[6][k]);
                        assert!((observed_pixel[k] - tone).abs() < 0.001);
                    }
                }
                if case < 3 {
                    responses[case] = o[6][0];
                }
                std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R024 ABC N{n} case{case} mode{mode} H{sign} back{back} partial{partial}: packed certificate={proof:?}, n={:?}, g={:?}, mappedN={:?}, direct={:?}, FP16={observed_pixel:?}, GS error bound={gs_error}\n",o[7],o[8],o[1],o[6])).unwrap();
            } else {
                assert_eq!(observed_pixel, [0.0; 4]);
            }
            if partial && n == 4 {
                assert!(
                    observed_pixel[3] > 0.0 && observed_pixel[3] < 1.0,
                    "observed real partial MSAA coverage"
                );
            }
            let usage = context.resource_usage().unwrap();
            let swaps = viewport.hdr_swaps_for_test(surface);
            r024_prepare(&mut viewport, &metrics);
            assert_eq!(usage, context.resource_usage().unwrap());
            assert_eq!(swaps, viewport.hdr_swaps_for_test(surface));
            // Execute the ORIGINAL production entry too, with no observation
            // binding, preserving the same exact captured geometry/material/D.
            let mut production = captured(&source);
            production.observe = false;
            production.hdr.revision += 1;
            let production = Arc::new(production);
            viewport
                .set_material_source(production.clone(), Viewport3DViewRevision::new(1), view)
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            for k in 0..4 {
                assert!((final_pixel[k] - observed_pixel[k]).abs() < 0.001);
            }
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            let mut invalid = captured(&production);
            invalid.hdr.revision += 1;
            invalid.hdr.maximum_rgb = [0.0; 3];
            assert!(
                viewport
                    .set_material_source(Arc::new(invalid), Viewport3DViewRevision::new(1), view,)
                    .is_err(),
                "an invalid radiance certificate refuses at admission"
            );
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            let mut invalid = captured(&production);
            invalid.hdr.revision += 1;
            let unrepresentable_view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(1.0e300, 0.0, 2.0).unwrap(),
                    Orientation3::identity(),
                ),
                crate::three_d::OrthographicProjection::try_new(
                    if partial { 32.0 } else { 2.0 },
                    0.1,
                    10.0,
                )
                .unwrap()
                .into(),
            );
            viewport
                .set_material_source(
                    Arc::new(invalid),
                    Viewport3DViewRevision::new(2),
                    unrepresentable_view,
                )
                .unwrap();
            viewport.prepare_custom_render(&metrics);
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
            assert!(Arc::ptr_eq(
                status.committed_material().unwrap().source(),
                &production
            ));
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            let resized = active_metrics_for_surface(surface, size + 2, size + 2, 1.0);
            let withdrawal = viewport.prepare_custom_render(&resized);
            assert!(
                withdrawal.front_publication_changed && withdrawal.presentation_binding().is_none()
            );
            assert!(viewport.target_texture_id_for_test(surface).is_none());
            assert_eq!(
                viewport.status_for(surface).unwrap().state(),
                Viewport3DLifecycleState::ReadyWithoutCommit
            );
            let mut restored = captured(&production);
            restored.hdr.revision += 3;
            viewport
                .set_material_source(Arc::new(restored), Viewport3DViewRevision::new(3), view)
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(window_size as f32, window_size as f32)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(
                    surface,
                    PhysicalExtent::new(window_size, window_size),
                    1.0,
                )
                .unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert_eq!(viewport.hdr_front_for_test(surface).width(), size);
            let admission = viewport.custom_render_admission().unwrap();
            let mut bindings = CustomRenderPresentationBindings::default();
            bindings
                .publish_for_target(
                    surface,
                    admission.presentation_id(),
                    admission.presentation_binding(
                        viewport.target_texture_id_for_test(surface).unwrap(),
                    ),
                )
                .unwrap();
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer.drain_pending_image_registrations();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let composed = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            for k in 0..3 {
                let s = f32::from(composed[k]) / 255.0;
                let linear = if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                };
                assert!((linear - (final_pixel[k] + 1.0 - final_pixel[3])).abs() < 0.02);
            }
            assert_eq!(composed[3], 255);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R024 ABC N{n} case{case}: original production FP16={final_pixel:?}, ordinary WHITE Image={composed:?}; warm stable, compatible complete old front retained, incompatible resize withdrawn\n")).unwrap();
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
        assert!(
            (responses[0] - responses[1]).abs() > 0.01
                && (responses[1] - responses[2]).abs() > 0.01,
            "normal-dependent direct response must distinguish B smooth from A/C: {responses:?}"
        );

        // HAL-R024-S2-REAL-PRODUCTION-HANDEDNESS-DISCRIMINATOR-01:
        // architect-selected BEFORE execution. This fresh numeric normal and
        // oblique L distinguish authored H through observe=false output only.
        // Criterion1/64 applies solely to this fixed source, not GS precision.
        let handedness_texture = context
            .register_texture(
                crate::three_d::Texture3D::try_new(
                    1,
                    1,
                    crate::three_d::TextureFormat3D::Rgba8Unorm,
                    vec![128, 255, 255, 255],
                )
                .unwrap(),
            )
            .unwrap();
        let mut handedness_material = PbrMaterialSource3D::defaults(93001, 1);
        handedness_material.sampling = crate::three_d::mip_sampling::SamplingFamily3D::V1;
        handedness_material.base_rgba = [0.5, 0.5, 0.5, 1.0];
        handedness_material.metallic = 0.0;
        handedness_material.roughness = 1.0;
        handedness_material.normal_scale = 1.0;
        handedness_material.emissive_rgb = [0.0; 3];
        handedness_material.alpha_mode = PbrAlphaMode3D::Opaque;
        handedness_material.double_sided = true;
        handedness_material.roles[1] = Some(PbrRoleBinding3D::V1 {
            texture: handedness_texture.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        let handedness_material = context
            .register_pbr_material(handedness_material, None)
            .unwrap();
        let mut handedness_images = [[0u8; 4]; 2];
        for (index, negative_uv) in [false, true].into_iter().enumerate() {
            let sign = if negative_uv { -1.0f32 } else { 1.0 };
            let qx = 2.0 * 128.0 / 255.0 - 1.0f64;
            let predicted_n_dot_l = (f64::from(sign) * 0.8 + 0.6) / (qx * qx + 2.0).sqrt();
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 HANDEDNESS PREDECLARED N{n} H{sign}: C packet unchanged; normalRGBA[128,255,255,255] scale1; L[0,.8,.6] E1; observe=false; source_predicted_n_dot_l={predicted_n_dot_l}; target32x32 pixel(16,16); FP16 H+ alpha1/red>=1/64, H- alpha1/RGB0; ordinary Image H+red>H-red, H-RGB0/alpha255\n",
            )).unwrap();
            let (render, input) = r024_abc_fixture::packet(2, negative_uv);
            assert!(
                render
                    .iter()
                    .all(|v| { v.normal == [0.0, 0.0, 1.0] && v.tangent == [1.0, 0.0, 0.0, sign] })
            );
            assert!(input.original.signed_tangents.iter().all(|t| t[3] == sign));
            let geometry =
                PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                    .unwrap();
            let geometry = context.register_pbr_geometry(geometry, None).unwrap();
            let revision = 100 + index as u64;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry,
                        material: handedness_material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![r024_identity()],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 93002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.8, 0.6],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            // Existing full source envelope: r=1 gives response<=2, E=1,
            // emission/E/G absent, hence cap4 bounds ALL ideal covered values.
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 93003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 32;
            let window_size = size + 2;
            let view = r025_view();
            let rect = RectF::new(0.0, 0.0, window_size as f32, window_size as f32);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (window_size, window_size), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, size, size, 1.0);
            let mut viewport = r024_viewport(context.clone(), source.clone(), view);
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(window_size as f32, window_size as f32)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(
                    surface,
                    PhysicalExtent::new(window_size, window_size),
                    1.0,
                )
                .unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let committed = status.committed_material().unwrap();
            let requested = status.requested_material().unwrap();
            assert!(Arc::ptr_eq(committed.source(), &source));
            assert!(Arc::ptr_eq(requested.source(), &source));
            assert_eq!(
                committed.structural_revision(),
                StructuralRevision::new(revision)
            );
            assert_eq!(committed.model_revision(), ModelRevision::new(revision));
            assert_eq!(committed.view(), view);
            assert_eq!(committed.request_serial(), requested.request_serial());
            let request = committed.request_serial().value();
            assert!(request > 0);
            let key = committed.target_key();
            assert_eq!(key, requested.target_key());
            assert_eq!(key.surface_id(), surface);
            assert_eq!(
                key.physical_render_extent(),
                PhysicalExtent::new(size, size)
            );
            assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
            assert_eq!(key.sample_count().value(), n);
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            let pixel = read_r027_fp16_at(viewport.hdr_front_for_test(surface), &gpu, 16, 16);
            let admission = viewport.custom_render_admission().unwrap();
            let mut bindings = CustomRenderPresentationBindings::default();
            bindings
                .publish_for_target(
                    surface,
                    admission.presentation_id(),
                    admission.presentation_binding(front),
                )
                .unwrap();
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer.drain_pending_image_registrations();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let image = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 HANDEDNESS PRODUCTION N{n} H{sign}: observe=false source/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; ordinary WHITE Image={image:?}\n",
            )).unwrap();
            assert_eq!(
                pixel[3], 1.0,
                "opaque covered production pixel N{n} H{sign}"
            );
            if negative_uv {
                assert_eq!(pixel[..3], [0.0; 3], "negative-handed N.L branch N{n}");
                assert_eq!(image[..3], [0; 3], "negative-handed ordinary Image N{n}");
            } else {
                assert!(
                    pixel[0] >= 1.0 / 64.0,
                    "preselected production separation N{n}"
                );
            }
            assert_eq!(image[3], 255);
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .request_serial()
                    .value(),
                request
            );
            handedness_images[index] = image;
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
        assert!(
            handedness_images[0][0] > handedness_images[1][0],
            "preselected ordinary Image handedness separation N{n}: {handedness_images:?}"
        );

        // HAL-R024-S2-BACKFACE-PRODUCTION-NORMAL-SIGN-DISCRIMINATOR-01:
        // same certified LOCAL C packets and normal/material, with the selected
        // proper Y180 rotation and positive nonuniform scale. Only final
        // observe=false production output is tested, not hidden front/GS data.
        let back_placement = ModelPlacement3::try_new(
            WorldPoint3::origin(),
            Orientation3::try_from_xyzw(0.0, 1.0, 0.0, 0.0).unwrap(),
            PositiveScale3::try_new(2.0, 1.0, 4.0).unwrap(),
        )
        .unwrap();
        assert_eq!(back_placement.determinant(), 8.0);
        let mut back_images = [[0u8; 4]; 2];
        for (index, negative_uv) in [false, true].into_iter().enumerate() {
            let sign = if negative_uv { -1.0f32 } else { 1.0 };
            let qx = 2.0 * 128.0 / 255.0 - 1.0f64;
            let predicted_n_dot_l = (-f64::from(sign) * 0.8 + 0.6) / (qx * qx + 2.0).sqrt();
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 BACKFACE PREDECLARED N{n} H{sign}: unchanged C packet; Y180 scale[2,1,4]; same normalRGBA[128,255,255,255] scale1/material/L[0,.8,.6] E1; observe=false; ideal_source_predicted_n_dot_l={predicted_n_dot_l}; target32x32 pixel(16,16); BACK H+ FP16/ImageRGB0, BACK H- FP16red>=1/64/ImageRed>H+; opaque alpha1/255\n",
            )).unwrap();
            let (render, input) = r024_abc_fixture::packet(2, negative_uv);
            assert!(
                render
                    .iter()
                    .all(|v| { v.normal == [0.0, 0.0, 1.0] && v.tangent == [1.0, 0.0, 0.0, sign] })
            );
            assert!(input.original.signed_tangents.iter().all(|t| t[3] == sign));
            let original = format!("{:?}", input.original.identity);
            let geometry =
                PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                    .unwrap();
            let geometry = context.register_pbr_geometry(geometry, None).unwrap();
            let revision = 102 + index as u64;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry,
                        material: handedness_material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![back_placement],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 93002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.8, 0.6],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 93003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 32;
            let window_size = size + 2;
            let view = r025_view();
            let rect = RectF::new(0.0, 0.0, window_size as f32, window_size as f32);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (window_size, window_size), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, size, size, 1.0);
            let mut viewport = r024_viewport(context.clone(), source.clone(), view);
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(window_size as f32, window_size as f32)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(
                    surface,
                    PhysicalExtent::new(window_size, window_size),
                    1.0,
                )
                .unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let committed = status.committed_material().unwrap();
            let requested = status.requested_material().unwrap();
            assert!(Arc::ptr_eq(committed.source(), &source));
            assert!(Arc::ptr_eq(requested.source(), &source));
            assert_eq!(
                committed.structural_revision(),
                StructuralRevision::new(revision)
            );
            assert_eq!(committed.model_revision(), ModelRevision::new(revision));
            assert_eq!(committed.view(), view);
            assert_eq!(committed.request_serial(), requested.request_serial());
            let request = committed.request_serial().value();
            assert!(request > 0);
            let key = committed.target_key();
            assert_eq!(key, requested.target_key());
            assert_eq!(key.surface_id(), surface);
            assert_eq!(
                key.physical_render_extent(),
                PhysicalExtent::new(size, size)
            );
            assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
            assert_eq!(key.sample_count().value(), n);
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            let pixel = read_r027_fp16_at(viewport.hdr_front_for_test(surface), &gpu, 16, 16);
            let admission = viewport.custom_render_admission().unwrap();
            let mut bindings = CustomRenderPresentationBindings::default();
            bindings
                .publish_for_target(
                    surface,
                    admission.presentation_id(),
                    admission.presentation_binding(front),
                )
                .unwrap();
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer.drain_pending_image_registrations();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let image = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 BACKFACE PRODUCTION device={} backend={:?} N{n} H{sign}: observe=false original={original} structural/model/direct/HDRrev{revision} request{request} model={back_placement:?} key={key:?} front#{front}; FP16(16,16)={pixel:?}; ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert_eq!(
                pixel[3], 1.0,
                "opaque covered BACK production pixel N{n} H{sign}"
            );
            assert_eq!(image[3], 255, "opaque BACK ordinary Image N{n} H{sign}");
            if negative_uv {
                assert!(
                    pixel[0] >= 1.0 / 64.0,
                    "preselected BACK H- production separation N{n}"
                );
            } else {
                assert_eq!(pixel[..3], [0.0; 3], "BACK H+ N.L branch N{n}");
                assert_eq!(image[..3], [0; 3], "BACK H+ ordinary Image N{n}");
            }
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .request_serial()
                    .value(),
                request
            );
            back_images[index] = image;
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
        assert!(
            back_images[1][0] > back_images[0][0],
            "preselected BACK ordinary Image handedness separation N{n}: {back_images:?}"
        );

        // HAL-R024-S2-C-SUBPIXEL-PARTIAL-PRODUCTION-NORMAL-SIGN-01:
        // intersect the existing C partial geometry with the selected production
        // H contrast. Preserve the packet's signed UVs in the same cyclic order
        // as the old partial case; never rewrite its original signed tangent.
        let mut partial_pixels = [[0.0f32; 4]; 2];
        let mut partial_images = [[0u8; 4]; 2];
        for (index, negative_uv) in [false, true].into_iter().enumerate() {
            let sign = if negative_uv { -1.0f32 } else { 1.0 };
            let (mut render, mut input) = r024_abc_fixture::packet(2, negative_uv);
            let partial_uv = [render[2].uv0, render[0].uv0, render[1].uv0];
            for (i, ([x, y], uv)) in [
                [0.0, 35.0 / 256.0],
                [-1.0, 28.0 / 256.0],
                [1.0, 28.0 / 256.0],
            ]
            .into_iter()
            .zip(partial_uv)
            .enumerate()
            {
                render[i].position = [0.5 + x, -0.5 + y, 0.0];
                render[i].uv0 = uv;
                let original = &mut Arc::make_mut(&mut input.original.vertices)[i];
                original.position = render[i].position;
                original.uv0 = Some(uv);
            }
            assert!(
                render
                    .iter()
                    .all(|v| { v.normal == [0.0, 0.0, 1.0] && v.tangent == [1.0, 0.0, 0.0, sign] })
            );
            assert!(input.original.signed_tangents.iter().all(|t| t[3] == sign));
            let original = format!("{:?}", input.original.identity);
            let qx = 2.0 * 128.0 / 255.0 - 1.0f64;
            let predicted_n_dot_l = (f64::from(sign) * 0.8 + 0.6) / (qx * qx + 2.0).sqrt();
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 C PARTIAL PREDECLARED N{n} H{sign}: original={original} fixed partial positions/cyclic packetUV{partial_uv:?}; same normalRGBA[128,255,255,255] scale1/material/L[0,.8,.6] E1/HDRcap4; observe=false; ideal_source_predicted_n_dot_l={predicted_n_dot_l}; ortho32 target32x32 pixel(16,16); N1 FP16RGBA0/ImageWHITE, N4 equal partial alpha/H+red>=1/128/H-RGB0/ImageH+red>H-red\n",
            )).unwrap();
            let geometry =
                PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                    .unwrap();
            let geometry = context.register_pbr_geometry(geometry, None).unwrap();
            let revision = 104 + index as u64;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry,
                        material: handedness_material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![r024_identity()],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 93002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.8, 0.6],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 93003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 32;
            let window_size = size + 2;
            let rect = RectF::new(0.0, 0.0, window_size as f32, window_size as f32);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (window_size, window_size), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, size, size, 1.0);
            let view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
                    Orientation3::identity(),
                ),
                crate::three_d::OrthographicProjection::try_new(32.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let mut viewport = r024_viewport(context.clone(), source.clone(), view);
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(window_size as f32, window_size as f32)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(
                    surface,
                    PhysicalExtent::new(window_size, window_size),
                    1.0,
                )
                .unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let requested = status.requested_material().unwrap();
            let committed = status.committed_material().unwrap();
            assert!(Arc::ptr_eq(requested.source(), &source));
            assert!(Arc::ptr_eq(committed.source(), &source));
            assert_eq!(
                committed.structural_revision(),
                StructuralRevision::new(revision)
            );
            assert_eq!(committed.model_revision(), ModelRevision::new(revision));
            assert_eq!(committed.view(), view);
            assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(1));
            assert_eq!(committed.request_serial(), requested.request_serial());
            let request = committed.request_serial().value();
            assert!(request > 0);
            let key = committed.target_key();
            assert_eq!(key, requested.target_key());
            assert_eq!(key.surface_id(), surface);
            assert_eq!(
                key.physical_render_extent(),
                PhysicalExtent::new(size, size)
            );
            assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
            assert_eq!(key.sample_count().value(), n);
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            let pixel = read_r027_fp16_at(viewport.hdr_front_for_test(surface), &gpu, 16, 16);
            let admission = viewport.custom_render_admission().unwrap();
            let mut bindings = CustomRenderPresentationBindings::default();
            bindings
                .publish_for_target(
                    surface,
                    admission.presentation_id(),
                    admission.presentation_binding(front),
                )
                .unwrap();
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer.drain_pending_image_registrations();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let image = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 C PARTIAL PRODUCTION device={} backend={:?} N{n} H{sign}: observe=false original={original} structural/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert_eq!(image[3], 255);
            if n == 1 {
                assert_eq!(pixel, [0.0; 4], "N1 uncovered C partial H{sign}");
                assert_eq!(image, [255; 4], "N1 uncovered WHITE Image H{sign}");
            } else {
                assert!(
                    pixel[3] > 0.0 && pixel[3] < 1.0,
                    "N4 partial coverage H{sign}"
                );
                if negative_uv {
                    assert_eq!(pixel[..3], [0.0; 3], "N4 C partial H- N.L branch");
                } else {
                    assert!(
                        pixel[0] >= 1.0 / 128.0,
                        "preselected N4 C partial H+ response"
                    );
                }
            }
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .request_serial()
                    .value(),
                request
            );
            partial_pixels[index] = pixel;
            partial_images[index] = image;
            drop(compiled);
            drop(tree);
            drop(source);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
            let usage = context.resource_usage().unwrap();
            assert_eq!((usage.pending_requests, usage.pending_publications), (0, 0));
        }
        assert_eq!(partial_pixels[0][3], partial_pixels[1][3]);
        if n == 4 {
            assert!(
                partial_images[0][0] > partial_images[1][0],
                "preselected N4 partial H Image separation: {partial_images:?}"
            );
        }

        // HAL-R024-S2-C-NEAR-CLIP-PRODUCTION-SIGN-01: the original C packet
        // is immutable across the selected control/near-clipped view successor.
        // Both fixed pixels are captured before applying the predeclared oracle.
        let near_pose = RigidPose3::new(
            WorldPoint3::try_new(0.0, -2.0, 2.0).unwrap(),
            Orientation3::try_from_xyzw(0.38268343, 0.0, 0.0, 0.9238795).unwrap(),
        );
        let near_views = [
            ViewDescriptor3::new(
                near_pose,
                crate::three_d::OrthographicProjection::try_new(2.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            ),
            ViewDescriptor3::new(
                near_pose,
                crate::three_d::OrthographicProjection::try_new(2.0, 2.65, 10.0)
                    .unwrap()
                    .into(),
            ),
        ];
        let mut near_images = [[[[0u8; 4]; 2]; 2]; 2];
        for (sign_index, negative_uv) in [false, true].into_iter().enumerate() {
            let sign = if negative_uv { -1.0f32 } else { 1.0 };
            let (render, input) = r024_abc_fixture::packet(2, negative_uv);
            assert!(
                render
                    .iter()
                    .all(|v| { v.normal == [0.0, 0.0, 1.0] && v.tangent == [1.0, 0.0, 0.0, sign] })
            );
            assert!(input.original.signed_tangents.iter().all(|t| t[3] == sign));
            let original = format!("{:?}", input.original.identity);
            let geometry = context
                .register_pbr_geometry(
                    PbrGeometrySource3D::try_new_certified(
                        render,
                        GeometryIndices::Unindexed,
                        input,
                    )
                    .unwrap(),
                    None,
                )
                .unwrap();
            let revision = 106 + sign_index as u64;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry,
                        material: handedness_material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![r024_identity()],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 93002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.8, 0.6],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 93003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (34, 34), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
            let mut viewport = r024_viewport(context.clone(), source.clone(), near_views[0]);
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let mut control_key = None;
            for (view_index, view) in near_views.into_iter().enumerate() {
                let label = if view_index == 0 {
                    "CONTROL"
                } else {
                    "CLIPPED"
                };
                let view_revision = Viewport3DViewRevision::new(1 + view_index as u64);
                let qx = 2.0 * 128.0 / 255.0 - 1.0f64;
                let ideal_view_cosines = [17.5f64, 20.5].map(|y| {
                    let world_x = 1.0 / 32.0;
                    let world_y = (1.0 - 2.0 * y / 32.0) * 2.0f64.sqrt();
                    (2.0 - f64::from(sign) * (world_y + 2.0) - qx * world_x)
                        / ((qx * qx + 2.0).sqrt()
                            * (world_x * world_x + (world_y + 2.0).powi(2) + 4.0).sqrt())
                });
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R024 C NEAR PREDECLARED N{n} H{sign} {label}: unchanged original={original}/C/normalRGBA[128,255,255,255]/material/L[0,.8,.6]/HDRcap4; view={view:?}; observe=false; ideal_source_NdotV(top,bottom)={ideal_view_cosines:?} (source calculation, not GPU telemetry); CONTROL both alpha1, CLIPPED top alpha1/bottom alpha0 WHITE; every interior H+red>=1/64 and H-RGB0, ImageH+red>H-red\n",
                )).unwrap();
                if view_index != 0 {
                    let viewport = tree
                        .node_mut(child)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<Viewport3D>()
                        .unwrap();
                    assert!(
                        viewport
                            .set_material_source_for_test(source.clone(), view_revision, view)
                            .unwrap()
                    );
                    r024_prepare(viewport, &metrics);
                }
                let _frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(34.0, 34.0)),
                    &Theme::default(),
                    &mut FontManagerTextEngine::new(),
                    &IconRegistry::new(&[]),
                    CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0)
                        .unwrap(),
                );
                let viewport = tree_viewport(&tree, child);
                assert!(viewport.material_observation_for_test(surface).is_none());
                let status = viewport.status_for(surface).unwrap();
                assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
                let requested = status.requested_material().unwrap();
                let committed = status.committed_material().unwrap();
                assert!(Arc::ptr_eq(requested.source(), &source));
                assert!(Arc::ptr_eq(committed.source(), &source));
                assert_eq!(
                    committed.structural_revision(),
                    StructuralRevision::new(revision)
                );
                assert_eq!(committed.model_revision(), ModelRevision::new(revision));
                assert_eq!(committed.view(), view);
                assert_eq!(committed.view_revision(), view_revision);
                assert_eq!(committed.request_serial(), requested.request_serial());
                let serial = committed.request_serial().value();
                assert_eq!(serial, 1 + view_index as u64);
                let key = committed.target_key();
                assert_eq!(key, requested.target_key());
                assert_eq!(key.surface_id(), surface);
                assert_eq!(key.physical_render_extent(), PhysicalExtent::new(32, 32));
                assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
                assert_eq!(key.sample_count().value(), n);
                if let Some(control) = control_key {
                    assert_eq!(key, control);
                } else {
                    control_key = Some(key);
                }
                let front = viewport.target_texture_id_for_test(surface).unwrap();
                let points = [[16u32, 17u32], [16, 20]];
                let pixels = points.map(|[x, y]| {
                    read_r027_fp16_at(viewport.hdr_front_for_test(surface), &gpu, x, y)
                });
                renderer.drain_pending_image_registrations();
                let images = points.map(|[x, y]| {
                    renderer.render_registered_image_pixel_for_test(
                        front,
                        [(x as f32 + 0.5) / 32.0, (y as f32 + 0.5) / 32.0],
                        [1.0; 4],
                        wgpu::Color::WHITE,
                    )
                });
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R024 C NEAR PRODUCTION device={} backend={:?} N{n} H{sign} {label}: observe=false original={original} SAME material93001/1 structural/model/direct/HDRrev{revision} viewrev{} request{serial} key={key:?} front#{front}; (16,17) FP16={:?}/WHITE Image={:?}; (16,20) FP16={:?}/WHITE Image={:?}\n",
                    adapter.name, adapter.backend, 1 + view_index, pixels[0], images[0], pixels[1], images[1],
                )).unwrap();
                for point in 0..2 {
                    assert_eq!(images[point][3], 255);
                    if view_index == 1 && point == 1 {
                        assert_eq!(pixels[point], [0.0; 4], "near-rejected N{n} H{sign}");
                        assert_eq!(images[point], [255; 4], "near-rejected WHITE N{n} H{sign}");
                    } else {
                        assert_eq!(
                            pixels[point][3], 1.0,
                            "covered {label} point{point} N{n} H{sign}"
                        );
                        if negative_uv {
                            assert_eq!(
                                pixels[point][..3],
                                [0.0; 3],
                                "interior H- {label} point{point} N{n}"
                            );
                            assert_eq!(
                                images[point][..3],
                                [0; 3],
                                "interior H- Image {label} point{point} N{n}"
                            );
                        } else {
                            assert!(
                                pixels[point][0] >= 1.0 / 64.0,
                                "preselected near H+ {label} point{point} N{n}: FP16={:?} Image={:?}",
                                pixels[point],
                                images[point]
                            );
                        }
                    }
                }
                assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
                near_images[sign_index][view_index] = images;
                let audit = custom_render_audit::snapshot();
                let usage = context.resource_usage().unwrap();
                let viewport = tree
                    .node_mut(child)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .unwrap();
                assert!(
                    !viewport
                        .set_material_source_for_test(source.clone(), view_revision, view)
                        .unwrap()
                );
                assert!(
                    !viewport
                        .prepare_custom_render(&metrics)
                        .front_publication_changed
                );
                assert_eq!(custom_render_audit::snapshot(), audit);
                assert_eq!(context.resource_usage().unwrap(), usage);
                assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
                assert_eq!(
                    viewport
                        .status_for(surface)
                        .unwrap()
                        .committed_material()
                        .unwrap()
                        .request_serial()
                        .value(),
                    serial
                );
            }
            drop(tree);
            drop(source);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
            let usage = context.resource_usage().unwrap();
            assert_eq!((usage.pending_requests, usage.pending_publications), (0, 0));
        }
        for (view_index, point_count) in [(0, 2), (1, 1)] {
            for point in 0..point_count {
                assert!(
                    near_images[0][view_index][point][0] > near_images[1][view_index][point][0],
                    "preselected near Image sign contrast N{n} view{view_index} point{point}"
                );
            }
        }

        // HAL-R024-S2-B-AUTHORED-SMOOTH-PRODUCTION-HANDEDNESS-01:
        // retain the exact B packets and all preceding A/B/C production cases.
        // This predeclared normal/light pair distinguishes B's original H sign
        // through production FP16/Image, without an observation attachment.
        let b_texture = context
            .register_texture(
                crate::three_d::Texture3D::try_new(
                    1,
                    1,
                    crate::three_d::TextureFormat3D::Rgba8Unorm,
                    vec![128, 191, 255, 255],
                )
                .unwrap(),
            )
            .unwrap();
        let mut b_material = PbrMaterialSource3D::defaults(94001, 1);
        b_material.sampling = crate::three_d::mip_sampling::SamplingFamily3D::V1;
        b_material.base_rgba = [0.5, 0.5, 0.5, 1.0];
        b_material.metallic = 0.0;
        b_material.roughness = 1.0;
        b_material.normal_scale = 1.0;
        b_material.emissive_rgb = [0.0; 3];
        b_material.alpha_mode = PbrAlphaMode3D::Opaque;
        b_material.double_sided = true;
        b_material.roles[1] = Some(PbrRoleBinding3D::V1 {
            texture: b_texture.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        let b_material = context.register_pbr_material(b_material, None).unwrap();
        let mut b_images = [[0u8; 4]; 2];
        for (index, negative_uv) in [false, true].into_iter().enumerate() {
            let sign = if negative_uv { -1.0f32 } else { 1.0 };
            let (render, input) = r024_abc_fixture::packet(1, negative_uv);
            assert!(matches!(
                &input.triangles[0].frame,
                crate::three_d::pbr::PbrFrameCertificate3D::AuthoredSmooth { .. }
            ));
            assert!(
                input
                    .original
                    .signed_tangents
                    .iter()
                    .all(|t| *t == [1.0, 0.0, 0.0, sign])
            );
            for (vertex, original) in render.iter().zip(input.original.vertices.iter()) {
                assert_eq!(vertex.position, original.position);
                assert_eq!(vertex.normal, r024_abc_fixture::unit(original.normal));
                assert_eq!(vertex.tangent, [1.0, 0.0, 0.0, sign]);
                assert_eq!(Some(vertex.uv0), original.uv0);
                assert_eq!(vertex.color, [255; 4]);
            }
            let original = input.original.identity;
            let qx = 1.0 / 255.0f64;
            let qy = 127.0 / 255.0f64;
            let q_length = (qx * qx + qy * qy + 1.0).sqrt();
            let predicted_vertex_n_dot_l = input
                .original
                .vertices
                .iter()
                .map(|v| {
                    let normal = v.normal.map(f64::from);
                    let length = normal.iter().map(|x| x * x).sum::<f64>().sqrt();
                    let ny = normal[1] / length;
                    let nz = normal[2] / length;
                    ((-0.8 - 0.6 * f64::from(sign) * qy) * ny
                        + (0.6 - 0.8 * f64::from(sign) * qy) * nz)
                        / q_length
                })
                .collect::<Vec<_>>();
            if negative_uv {
                assert!(predicted_vertex_n_dot_l.iter().all(|v| *v > 0.12));
            } else {
                assert!(predicted_vertex_n_dot_l.iter().all(|v| *v < 0.0));
            }
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 B HANDEDNESS PREDECLARED N{n} H{sign}: unchanged certified AuthoredSmooth original={original:?}/positions/normals/T+X/signed UV/corners; normalRGBA[128,191,255,255] scale1, q[1/255,127/255,1]; identity model; L[0,-.8,.6] E1; ideal source vertex NdotL={predicted_vertex_n_dot_l:?} (source calculation, not GPU telemetry); H+ negative, H- positive>.12, same sign through positive convex interpolation; observe=false target32x32 pixel(16,16); H+ FP16/ImageRGB0, H- FP16red>=1/128/ImageRed>H+; opaque alpha1/255\n",
            )).unwrap();
            let geometry =
                PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                    .unwrap();
            let geometry = context.register_pbr_geometry(geometry, None).unwrap();
            let revision = 108 + index as u64;
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry,
                        material: b_material.clone(),
                        raster: RasterMode::DoubleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![r024_identity()],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 94002,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, -0.8, 0.6],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: snapshot.clone(),
                model: model.clone(),
            });
            // r=1 response*cos<=2 and E1, with zero emission and no E/G,
            // retain the existing full ideal covered-output envelope cap4.
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 94003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 32;
            let window_size = size + 2;
            let rect = RectF::new(0.0, 0.0, window_size as f32, window_size as f32);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (window_size, window_size), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, size, size, 1.0);
            let view = r025_view();
            let mut viewport = r024_viewport(context.clone(), source.clone(), view);
            r024_prepare(&mut viewport, &metrics);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
                None,
                0,
            );
            let child = tree.add_child(root, Box::new(viewport), None, 0);
            tree.replace_widget_only(
                root,
                Box::new(
                    Panel::new(child)
                        .with_padding(0.0)
                        .with_background(crate::core::paint::Color::WHITE),
                ),
            );
            let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
                &mut tree,
                root,
                Constraints::tight(Size::new(window_size as f32, window_size as f32)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(
                    surface,
                    PhysicalExtent::new(window_size, window_size),
                    1.0,
                )
                .unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let requested = status.requested_material().unwrap();
            let committed = status.committed_material().unwrap();
            assert!(Arc::ptr_eq(requested.source(), &source));
            assert!(Arc::ptr_eq(committed.source(), &source));
            assert_eq!(
                committed.structural_revision(),
                StructuralRevision::new(revision)
            );
            assert_eq!(committed.model_revision(), ModelRevision::new(revision));
            assert_eq!(
                committed.source().direct.as_ref().unwrap().revision,
                revision
            );
            assert_eq!(committed.source().hdr.revision, revision);
            assert_eq!(committed.view(), view);
            assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(1));
            assert_eq!(requested.request_serial(), committed.request_serial());
            let request = committed.request_serial().value();
            assert!(request > 0);
            let key = committed.target_key();
            assert_eq!(requested.target_key(), key);
            assert_eq!(key.surface_id(), surface);
            assert_eq!(
                key.physical_render_extent(),
                PhysicalExtent::new(size, size)
            );
            assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
            assert_eq!(key.sample_count().value(), n);
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            let pixel = read_r027_fp16_at(viewport.hdr_front_for_test(surface), &gpu, 16, 16);
            let admission = viewport.custom_render_admission().unwrap();
            let mut bindings = CustomRenderPresentationBindings::default();
            bindings
                .publish_for_target(
                    surface,
                    admission.presentation_id(),
                    admission.presentation_binding(front),
                )
                .unwrap();
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut bindings.begin_materialization_for_test(0),
                |_| 1.0,
                0,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer.drain_pending_image_registrations();
            let compiled = renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let image = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 B HANDEDNESS PRODUCTION device={} backend={:?} wgpu30.0.2/Naga30.0.2 N{n} H{sign}: observe=false original={original:?} certified B; material94001/1 structural/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; registered ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert!(pixel.iter().all(|v| v.is_finite()));
            assert_eq!(pixel[3], 1.0, "opaque covered B production N{n} H{sign}");
            assert_eq!(image[3], 255, "opaque ordinary B Image N{n} H{sign}");
            if negative_uv {
                assert!(
                    pixel[0] >= 1.0 / 128.0,
                    "preselected B H- lit N{n}: FP16={pixel:?} Image={image:?}"
                );
            } else {
                assert_eq!(pixel[..3], [0.0; 3], "B H+ N.L branch N{n}");
                assert_eq!(image, [0, 0, 0, 255], "B H+ ordinary Image N{n}");
            }
            let audit = custom_render_audit::snapshot();
            let usage = context.resource_usage().unwrap();
            let viewport = tree
                .node_mut(child)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Viewport3D>()
                .unwrap();
            assert!(
                !viewport
                    .set_material_source_for_test(
                        source.clone(),
                        Viewport3DViewRevision::new(1),
                        view
                    )
                    .unwrap()
            );
            assert!(
                !viewport
                    .prepare_custom_render(&metrics)
                    .front_publication_changed
            );
            assert_eq!(custom_render_audit::snapshot(), audit);
            assert_eq!(context.resource_usage().unwrap(), usage);
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .request_serial()
                    .value(),
                request
            );
            b_images[index] = image;
            drop(compiled);
            drop(tree);
            drop(source);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
            let usage = context.resource_usage().unwrap();
            assert_eq!((usage.pending_requests, usage.pending_publications), (0, 0));
        }
        assert!(
            b_images[1][0] > b_images[0][0],
            "preselected B H-/H+ ordinary Image contrast N{n}: {b_images:?}"
        );
    }
}

#[test]
fn material_writer_unlinked_truthful_source_family_fp16_image_n1_n4() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("non-test material writer proof requires an actual Device");
    let gpu = renderer.gpu_resources();
    let adapter = renderer.headless_adapter_for_test().clone();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        // The old constructor's empty Legacy 1/1 cannot describe this real
        // Material triangle 2/2. The production setter must replace its family.
        let legacy = context
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(1),
                vec![],
            ))
            .unwrap();
        let legacy_model = context
            .admit_model_state(
                &legacy,
                ModelStateDescriptor::new(ModelRevision::new(1), vec![]),
            )
            .unwrap();
        let view = r024_view(2.0);
        let mut viewport = Viewport3D::new(
            context.clone(),
            Viewport3DViewRevision::new(1),
            view,
            legacy,
            legacy_model,
            SampleCount3D::ONE,
        )
        .unwrap();
        let mut material = PbrMaterialSource3D::defaults(91001, 1);
        material.emissive_rgb = [1.0, 0.5, 0.25];
        let source = r024_source(
            &context,
            super::three_d_geometry::r024_vertices(),
            material,
            n,
            false,
            r024_identity(),
            2,
        );
        assert!(
            viewport
                .set_material_source(source.clone(), Viewport3DViewRevision::new(1), view,)
                .unwrap()
        );
        assert!(
            !viewport
                .set_material_source(source.clone(), Viewport3DViewRevision::new(1), view,)
                .unwrap()
        );
        let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (32, 32), 1.0)
            .unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
            None,
            0,
        );
        let child = tree.add_child(root, Box::new(viewport), None, 0);
        tree.replace_widget_only(
            root,
            Box::new(
                Panel::new(child)
                    .with_padding(0.0)
                    .with_background(crate::core::paint::Color::WHITE),
            ),
        );
        let frame = RetainedSceneTestHarness::new().frame_with_custom_render_target(
            &mut tree,
            root,
            Constraints::tight(Size::new(32.0, 32.0)),
            &Theme::default(),
            &mut FontManagerTextEngine::new(),
            &IconRegistry::new(&[]),
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(32, 32), 1.0).unwrap(),
        );
        let viewport = tree_viewport(&tree, child);
        let status = viewport.status_for(surface).unwrap();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        assert!(status.requested().is_none() && status.committed().is_none());
        assert!(status.failure().is_none() && status.device_failure().is_none());
        let requested = status.requested_material().unwrap();
        let committed = status.committed_material().unwrap();
        assert_eq!(requested.request_serial(), committed.request_serial());
        assert_eq!(committed.structural_revision(), StructuralRevision::new(2));
        assert_eq!(committed.model_revision(), ModelRevision::new(2));
        assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(1));
        assert_eq!(committed.view(), view);
        assert_eq!(committed.target_key().sample_count().value(), n);
        assert!(Arc::ptr_eq(committed.source(), &source));
        assert!(viewport.material_observation_for_test(surface).is_none());
        assert_eq!(viewport.target_statuses().len(), 1);
        let target = viewport.current_prepared_target().unwrap();
        assert_eq!(target.surface_id(), surface);
        assert_eq!(
            target.committed_ray(0.5, 0.5).unwrap().unwrap().origin(),
            view.pose().translation()
        );
        let domain = crate::three_d::LocalDomainId3::new(1);
        assert!(target.committed_domain(domain).is_none());
        assert!(
            target
                .committed_domain_ray(domain, 0.5, 0.5)
                .unwrap()
                .is_none()
        );
        let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        for (actual, expected) in final_pixel.into_iter().zip([0.5, 1.0 / 3.0, 0.2, 1.0]) {
            assert!((actual - expected).abs() < 0.0003);
        }
        let admission = viewport.custom_render_admission().unwrap();
        let mut bindings = CustomRenderPresentationBindings::default();
        bindings
            .publish_for_target(
                surface,
                admission.presentation_id(),
                admission
                    .presentation_binding(viewport.target_texture_id_for_test(surface).unwrap()),
            )
            .unwrap();
        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root;
        let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
            &std::collections::HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            &mut bindings.begin_materialization_for_test(0),
            |_| 1.0,
            0,
        ).expect("scene must satisfy DrawPlan Build admission");
        renderer.drain_pending_image_registrations();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let image = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
        for c in 0..3 {
            let encoded = f32::from(image[c]) / 255.0;
            let linear = if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            };
            assert!((linear - final_pixel[c]).abs() < 0.01);
        }
        assert_eq!(image[3], 255);
        drop(compiled);
        let before = custom_render_audit::snapshot();
        let metrics = viewport
            .current_prepared_target()
            .unwrap()
            .metrics()
            .unwrap();
        let viewport = tree
            .node_mut(child)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        assert!(
            !viewport
                .prepare_custom_render(&metrics)
                .front_publication_changed
        );
        assert_eq!(custom_render_audit::snapshot(), before);
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "UNLINKED non-test owner N{n}: {adapter:?}; Material2/2 HDR700/2; FP16={final_pixel:?} Image={image:?}; public Legacy=None; warm GPU work=0\n",
        )).unwrap();
        drop(tree);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn material_writer_last_good_families_target_currentness_and_retirement() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("material family/last-good proof requires an actual Device");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let view = r024_view(2.0);
    let legacy = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, true);
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        legacy,
        SampleCount3D::ONE,
    )
    .unwrap();
    let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
    let mut compositor = Compositor::new(rect);
    let surface = compositor.create_surface(0, rect, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (32, 32), 1.0)
        .unwrap();
    let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
    viewport.prepare_custom_render(&metrics);
    let original = viewport
        .status_for(surface)
        .unwrap()
        .committed()
        .unwrap()
        .clone();
    let original_id = viewport.target_texture_id_for_test(surface).unwrap();
    assert_eq!(
        original.target_key().color_format(),
        wgpu::TextureFormat::Bgra8UnormSrgb
    );
    let image_scene = |presentation_id| {
        let mut scene = SceneBuilder::new();
        scene.push_target_bound_image(
            rect,
            presentation_id,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            crate::core::scene::Color::WHITE,
            0,
            None,
        );
        scene.build()
    };
    let compose = |v: &Viewport3D, renderer: &mut WgpuRenderer| {
        let admission = v.custom_render_admission().unwrap();
        let mut bindings = CustomRenderPresentationBindings::default();
        bindings
            .publish_for_target(
                surface,
                admission.presentation_id(),
                admission.presentation_binding(v.target_texture_id_for_test(surface).unwrap()),
            )
            .unwrap();
        let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
            &std::collections::HashMap::from([(surface, image_scene(admission.presentation_id()))]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            &mut bindings.begin_materialization_for_test(0),
            |_| 1.0,
            0,
        ).expect("scene must satisfy DrawPlan Build admission");
        renderer.drain_pending_image_registrations();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        render_compiled_surface_center_pixel(renderer, &compiled, surface)
    };
    let red = compose(&viewport, &mut renderer);
    assert_eq!(red, [255, 0, 0, 255]);
    let mut material = PbrMaterialSource3D::defaults(91002, 1);
    material.emissive_rgb = [0.0, 1.0, 0.0];
    let unrepresentable = ModelPlacement3::try_new(
        WorldPoint3::try_new(1.0e300, 0.0, 0.0).unwrap(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let candidate = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        material.clone(),
        1,
        false,
        unrepresentable,
        2,
    );
    viewport
        .set_material_source(candidate.clone(), Viewport3DViewRevision::new(1), view)
        .unwrap();
    let before = custom_render_audit::snapshot();
    let withdrawal = viewport.prepare_custom_render(&metrics);
    assert!(withdrawal.front_publication_changed);
    assert!(withdrawal.presentation_binding().is_none());
    assert_eq!(
        custom_render_audit::snapshot().custom_submissions,
        before.custom_submissions
    );
    let failed = viewport.status_for(surface).unwrap();
    assert_eq!(failed.state(), Viewport3DLifecycleState::ReadyWithoutCommit);
    assert!(failed.requested().is_none() && failed.committed().is_none());
    let requested = failed.requested_material().unwrap();
    assert_eq!(
        requested.target_key().color_format(),
        wgpu::TextureFormat::Rgba16Float
    );
    assert_ne!(requested.target_key(), original.target_key());
    assert_eq!(requested.model_revision(), ModelRevision::new(2));
    let failed_serial = requested.request_serial();
    assert!(Arc::ptr_eq(
        failed.requested_material().unwrap().source(),
        &candidate
    ));
    assert!(failed.committed_material().is_none());
    assert!(
        failed.failure().is_none(),
        "Material math failure has no invented legacy error"
    );
    assert!(matches!(
        viewport.material_failure_for_test(surface),
        Some(crate::three_d::pbr::PbrError3D::Math(_))
    ));
    assert!(viewport.target_texture_id_for_test(surface).is_none());
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .committed_ray(0.5, 0.5)
            .unwrap()
            .is_none()
    );
    // The old red texture may remain under deferred GPU custody, but no
    // current binding can expose it to the ordinary Image draw plan.
    let admission = viewport.custom_render_admission().unwrap();
    let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
        &std::collections::HashMap::from([(surface, image_scene(admission.presentation_id()))]),
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut CustomRenderPresentationBindings::default().begin_materialization_for_test(0),
        |_| 1.0,
        0,
    ).expect("scene must satisfy DrawPlan Build admission");
    assert!(draw_plan_image_texture(&plan, surface).is_none());
    let failure = viewport.material_failure_for_test(surface).cloned();
    let unchanged_before = custom_render_audit::snapshot();
    let unchanged = viewport.prepare_custom_render(&metrics);
    assert!(!unchanged.front_publication_changed && unchanged.presentation_binding().is_none());
    assert_eq!(custom_render_audit::snapshot(), unchanged_before);
    assert_eq!(
        viewport.material_failure_for_test(surface),
        failure.as_ref()
    );
    assert_eq!(
        viewport
            .status_for(surface)
            .unwrap()
            .requested_material()
            .unwrap()
            .request_serial(),
        failed_serial
    );
    let valid = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        material.clone(),
        1,
        false,
        r024_identity(),
        3,
    );
    viewport
        .set_material_source(valid.clone(), Viewport3DViewRevision::new(1), view)
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    assert!(viewport.status_for(surface).unwrap().committed().is_none());
    let material_id = viewport.target_texture_id_for_test(surface).unwrap();
    assert_ne!(material_id, original_id);
    let green = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
    assert_eq!(green, [0.0, 0.5, 0.0, 1.0]);
    let committed = viewport
        .status_for(surface)
        .unwrap()
        .committed_material()
        .unwrap();
    assert!(Arc::ptr_eq(committed.source(), &valid));
    assert_eq!(committed.model_revision(), ModelRevision::new(3));
    assert_eq!(
        committed.target_key().color_format(),
        wgpu::TextureFormat::Rgba16Float
    );
    assert_eq!(committed.target_key().sample_count(), SampleCount3D::ONE);
    let green_image = compose(&viewport, &mut renderer);
    assert_eq!(green_image, [0, 188, 0, 255]);
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "TARGET FORMAT {:?}->{:?}: incompatible Legacy red {red:?} withdrawn, no binding/Image/ray; repeated failure stable; Material700/3 FP16={green:?} Image={green_image:?}\n",
        original.target_key().color_format(), committed.target_key().color_format(),
    )).unwrap();
    let other = active_metrics(91002, 24, 20, 1.0);
    r024_prepare(&mut viewport, &other);
    assert_eq!(viewport.target_statuses().len(), 2);
    let invalid = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        material.clone(),
        1,
        false,
        unrepresentable,
        4,
    );
    let moved_view = r024_view(3.0);
    viewport
        .set_material_source(invalid.clone(), Viewport3DViewRevision::new(2), moved_view)
        .unwrap();
    assert!(
        !viewport
            .prepare_custom_render(&metrics)
            .front_publication_changed
    );
    let failed = viewport.status_for(surface).unwrap();
    assert_eq!(failed.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert!(failed.requested().is_none() && failed.committed().is_none());
    assert_eq!(
        failed.requested_material().unwrap().target_key(),
        failed.committed_material().unwrap().target_key(),
    );
    assert!(Arc::ptr_eq(
        failed.requested_material().unwrap().source(),
        &invalid
    ));
    assert!(Arc::ptr_eq(
        failed.committed_material().unwrap().source(),
        &valid
    ));
    assert_eq!(
        failed.committed_material().unwrap().model_revision(),
        ModelRevision::new(3)
    );
    assert_eq!(
        failed.requested_material().unwrap().model_revision(),
        ModelRevision::new(4)
    );
    assert_eq!(
        viewport.target_texture_id_for_test(surface),
        Some(material_id)
    );
    assert_eq!(
        read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu),
        green
    );
    let current = viewport.current_prepared_target().unwrap();
    let last_good_ray = current.committed_ray(0.25, 0.75).unwrap().unwrap();
    assert_eq!(
        last_good_ray,
        view.ray_for_committed_extent(32, 32, 0.25, 0.75).unwrap()
    );
    assert_ne!(
        last_good_ray,
        moved_view
            .ray_for_committed_extent(32, 32, 0.25, 0.75)
            .unwrap()
    );
    assert!(Arc::ptr_eq(
        viewport
            .status_for(other.surface_id)
            .unwrap()
            .committed_material()
            .unwrap()
            .source(),
        &valid
    ));
    let mut inactive = metrics;
    inactive.activity = CustomRenderActivity::Inactive;
    assert!(
        viewport
            .prepare_custom_render(&inactive)
            .front_publication_changed
    );
    let status = viewport.status_for(surface).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::Inactive);
    assert!(status.requested().is_none() && status.committed().is_none());
    assert!(status.requested_material().is_some() && status.committed_material().is_none());
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .committed_ray(0.5, 0.5)
            .unwrap()
            .is_none()
    );
    let restored = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        material,
        1,
        false,
        r024_identity(),
        5,
    );
    viewport
        .set_material_source(restored.clone(), Viewport3DViewRevision::new(2), moved_view)
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    let restore_id = viewport.target_texture_id_for_test(surface).unwrap();
    assert_ne!(restore_id, material_id);
    let resized = active_metrics_for_surface(surface, 40, 32, 1.0);
    r024_prepare(&mut viewport, &resized);
    assert_ne!(
        viewport.target_texture_id_for_test(surface),
        Some(restore_id)
    );
    assert_eq!(
        viewport
            .status_for(surface)
            .unwrap()
            .committed_material()
            .unwrap()
            .target_key()
            .physical_render_extent(),
        PhysicalExtent::new(40, 32)
    );
    assert!(
        viewport
            .set_material_source(valid, Viewport3DViewRevision::new(2), moved_view)
            .is_err()
    );
    assert!(
        viewport
            .set_material_source(restored.clone(), Viewport3DViewRevision::new(1), view)
            .is_err()
    );
    assert!(
        viewport
            .set_material_source(restored, Viewport3DViewRevision::new(2), view)
            .is_err()
    );
    let before = custom_render_audit::snapshot();
    assert!(
        !viewport
            .prepare_custom_render(&resized)
            .front_publication_changed
    );
    assert_eq!(custom_render_audit::snapshot(), before);
    viewport.on_unmount();
    assert!(
        viewport
            .target_statuses()
            .all(|t| t.status().state() == Viewport3DLifecycleState::Retired
                && t.status().committed_material().is_none()
                && t.status().committed().is_none())
    );
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

mod material_and_direct_lighting;
use super::{three_d_context, three_d_geometry, three_d_raster};
use self::material_and_direct_lighting::{
    r024_identity, r024_prepare, r025_observation_at, r025_occluder_source,
};
pub(super) use self::material_and_direct_lighting::{r025_source, r025_view};

fn setup(policy: PolicyLimits3D) -> Option<(WgpuRenderer, Arc<Context3D>, ViewDescriptor3)> {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())?;
    let context = Arc::new(Context3D::with_policy(renderer.gpu_resources(), policy).ok()?);
    Some((renderer, context, view_at(0.0)))
}

// The fixture is an external, test-owned scene producer. It writes actual HDR
// samples and reverse-Z depth; it has no tone/final/Image path of its own.
struct R027SceneFixture {
    source: crate::three_d::hdr::HdrSource3D,
    pipeline: wgpu::RenderPipeline,
    _module: wgpu::ShaderModule,
    gpu: crate::renderer::wgpu_renderer::GpuResources,
    samples: u32,
}

impl R027SceneFixture {
    fn new(
        context: &Context3D,
        samples: u32,
        revision: u64,
        colors: [[f32; 3]; 4],
        covered: u32,
        ev: f32,
    ) -> Arc<Self> {
        Self::at_depth(context, samples, revision, colors, covered, ev, 0.5)
    }

    fn at_depth(
        context: &Context3D,
        samples: u32,
        revision: u64,
        colors: [[f32; 3]; 4],
        covered: u32,
        ev: f32,
        depth: f32,
    ) -> Arc<Self> {
        assert!(matches!(samples, 1 | 4) && covered <= samples);
        assert!(
            colors
                .iter()
                .flatten()
                .all(|c| c.is_finite() && (0.0..=60000.0).contains(c))
        );
        let source = crate::three_d::hdr::HdrSource3D {
            identity: 27,
            revision,
            exposure_ev: ev,
            maximum_rgb: std::array::from_fn(|channel| {
                colors.iter().map(|c| c[channel]).fold(0.0_f32, f32::max)
            }),
        };
        source.exposure().unwrap();
        let values = colors
            .iter()
            .map(|c| {
                format!(
                    "vec3<f32>(bitcast<f32>({}u),bitcast<f32>({}u),bitcast<f32>({}u))",
                    c[0].to_bits(),
                    c[1].to_bits(),
                    c[2].to_bits()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let sample_arg = if samples == 4 {
            "@builtin(sample_index) sample: u32"
        } else {
            ""
        };
        let sample = if samples == 4 { "sample" } else { "0u" };
        let wgsl = format!(
            r#"
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {{
 let p=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0)); return vec4(p[i],{depth},1.0);
}}
@fragment fn fs({sample_arg}) -> @location(0) vec4<f32> {{
 let i={sample}; if i >= {covered}u {{ discard; }}
 let colors=array<vec3<f32>,4>({values}); return vec4(colors[i],1.0);
}}
"#
        );
        let gpu = context.gpu_resources_for_test();
        let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut ir = naga::front::wgsl::parse_str(&wgsl).unwrap();
        crate::three_d::hdr::scene_fragment_outputs(&mut ir, &["fs"]).unwrap();
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("strict R027 test scene producer"),
                source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(ir)),
            });
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("strict scene producer layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("strict GPU HDR scene fixture, no resolve"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    targets: &crate::three_d::hdr::scene_color_targets(),
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });
        assert!(
            pollster::block_on(validation_scope.pop()).is_none(),
            "actual scene fixture must validate on Device"
        );
        Arc::new(Self {
            source,
            pipeline,
            _module: module,
            gpu,
            samples,
        })
    }

    fn encode_layers(
        layers: &[&Self],
        encoder: &mut wgpu::CommandEncoder,
        scene: &crate::three_d::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("R027 same-pass source depth/coverage layers"),
            color_attachments: &scene.attachments(),
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        for layer in layers {
            pass.set_pipeline(&layer.pipeline);
            pass.draw(0..3, 0..1);
        }
    }
}

impl crate::three_d::hdr::HdrProofSceneWriter3D for R027SceneFixture {
    fn source(&self) -> &crate::three_d::hdr::HdrSource3D {
        &self.source
    }
    fn samples(&self) -> u32 {
        self.samples
    }
    fn gpu_resources(&self) -> &crate::renderer::wgpu_renderer::GpuResources {
        &self.gpu
    }
    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &crate::three_d::hdr::HdrScene3D,
        depth: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("strict R027 GPU covered/clear scene samples"),
            color_attachments: &scene.attachments(),
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1);
    }
}

fn r027_viewport(
    context: Arc<Context3D>,
    samples: SampleCount3D,
    fixture: Arc<R027SceneFixture>,
) -> Viewport3D {
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![],
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(ModelRevision::new(1), vec![]),
        )
        .unwrap();
    let mut viewport = Viewport3D::new(
        context,
        Viewport3DViewRevision::new(1),
        view_at(0.0),
        snapshot,
        model,
        samples,
    )
    .unwrap();
    viewport.set_hdr_fixture_for_test(fixture, samples).unwrap();
    viewport
}

fn read_r027_fp16(
    texture: &wgpu::Texture,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> [f32; 4] {
    read_r027_fp16_at(texture, gpu, texture.width() / 2, texture.height() / 2)
}
fn read_r027_fp16_at(
    texture: &wgpu::Texture,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    x: u32,
    y: u32,
) -> [f32; 4] {
    assert_eq!(texture.format(), wgpu::TextureFormat::Rgba16Float);
    assert_eq!(texture.sample_count(), 1);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R027 numeric FP16 front readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let result = std::array::from_fn(|channel| {
        let bits = u16::from_le_bytes([bytes[channel * 2], bytes[channel * 2 + 1]]);
        let exponent = (bits >> 10) & 31;
        let fraction = bits & 1023;
        assert_eq!(bits >> 15, 0, "nonnegative scene display output");
        assert_ne!(exponent, 31, "finite FP16 front");
        if exponent == 0 {
            f32::from(fraction) * 2.0_f32.powi(-24)
        } else {
            (1.0 + f32::from(fraction) / 1024.0) * 2.0_f32.powi(i32::from(exponent) - 15)
        }
    });
    drop(bytes);
    buffer.unmap();
    result
}

fn r027_encoder_upload_fixture(
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> (wgpu::Buffer, wgpu::Buffer, [u8; 64]) {
    let bytes = std::array::from_fn(|i| 0x80 | i as u8);
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R027 directly mapped encoder-local upload64"),
        size: 64,
        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    });
    {
        let mut mapped = staging.slice(..).get_mapped_range_mut().unwrap();
        mapped.copy_from_slice(&bytes);
    }
    staging.unmap();
    // A fresh unmapped destination has wgpu's ordinary zero initialization.
    let destination = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R027 encoder-local destination64"),
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    (staging, destination, bytes)
}

fn read_r027_upload_bytes(
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    destination: &wgpu::Buffer,
) -> [u8; 64] {
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R027 independent producer readback64"),
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("R027 independent producer after upload outcome"),
        });
    encoder.copy_buffer_to_buffer(destination, 0, &readback, 0, 64);
    gpu.queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let bytes = readback
        .slice(..)
        .get_mapped_range().expect("mapped readback range")
        .as_ref()
        .try_into()
        .unwrap();
    readback.unmap();
    bytes
}

fn r027_encoder_local_upload_controls(gpu: &crate::renderer::wgpu_renderer::GpuResources, n: u32) {
    for submit in [false, true] {
        let (staging, destination, bytes) = r027_encoder_upload_fixture(gpu);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("R027 frame-private mapped upload probe"),
            });
        encoder.copy_buffer_to_buffer(&staging, 0, &destination, 0, 64);
        let completion = if submit {
            gpu.queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            gpu.queue.on_submitted_work_done(move || {
                drop(staging);
                send.send(()).unwrap();
            });
            Some(receive)
        } else {
            drop(encoder);
            drop(staging);
            None
        };
        let actual = read_r027_upload_bytes(gpu, &destination);
        assert_eq!(actual, if submit { bytes } else { [0; 64] });
        if let Some(completion) = completion {
            completion.recv().unwrap();
        }
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R027 ENCODER UPLOAD N{n}: submit={submit} staging_bytes=64 destination_bytes=64 readback_bytes=64 candidate_submits={} independent_readback_submits=1 readbacks=1 bytes={actual:?}; no candidate Queue write; submitted staging retained through covering completion\n",
            u32::from(submit),
        )).unwrap();
    }
}

fn r027_reusable_mapped_upload_controls(
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    n: u32,
) {
    let (staging, first_destination, first_bytes) = r027_encoder_upload_fixture(gpu);
    let identity = staging.clone();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("R027 retained staging first use"),
        });
    encoder.copy_buffer_to_buffer(&staging, 0, &first_destination, 0, 64);
    gpu.queue.submit([encoder.finish()]);
    let mut copy_submits = 1;
    let (send, receive) = std::sync::mpsc::channel();
    gpu.queue
        .on_submitted_work_done(move || send.send(()).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap();
    let mut completions = 1;
    assert_eq!(read_r027_upload_bytes(gpu, &first_destination), first_bytes);
    let mut readbacks = 1;
    drop(first_destination);

    let mut previous = first_bytes;
    let mut remaps = 0;
    let mut abandoned = 0;
    let mut preserved_readbacks = 0;
    let rearm = || {
        let (send, receive) = std::sync::mpsc::channel();
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Write, move |result| {
                send.send(result).unwrap();
            });
        // The previous writer's completion was observed separately above.
        gpu.device.poll(wgpu::PollType::Poll).unwrap();
        receive
            .try_recv()
            .expect("completed source remap must progress without another submit")
            .unwrap();
    };
    for bytes in [std::array::from_fn(|i| 32 + i as u8), first_bytes] {
        assert_eq!(
            staging, identity,
            "the actual wgpu Buffer remains identical"
        );
        rearm();
        remaps += 1;
        // A write mapping cannot be read. Close the successful rearm without
        // writing and check the same source's old bytes on the actual GPU
        // before taking the write mapping for the next upload.
        staging.unmap();
        assert_eq!(read_r027_upload_bytes(gpu, &staging), previous);
        preserved_readbacks += 1;
        rearm();
        {
            let mut mapped = staging.slice(..).get_mapped_range_mut().unwrap();
            mapped.copy_from_slice(&bytes);
        }
        staging.unmap();

        let destination = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("R027 rearmed staging untouched destination64"),
            size: 64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("R027 rearmed staging abandoned copy"),
            });
        encoder.copy_buffer_to_buffer(&staging, 0, &destination, 0, 64);
        drop(encoder);
        abandoned += 1;
        assert_eq!(read_r027_upload_bytes(gpu, &destination), [0; 64]);
        readbacks += 1;

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("R027 rearmed staging intended copy"),
            });
        encoder.copy_buffer_to_buffer(&staging, 0, &destination, 0, 64);
        gpu.queue.submit([encoder.finish()]);
        copy_submits += 1;
        let (send, receive) = std::sync::mpsc::channel();
        gpu.queue
            .on_submitted_work_done(move || send.send(()).unwrap());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap();
        completions += 1;
        let actual = read_r027_upload_bytes(gpu, &destination);
        readbacks += 1;
        assert_eq!(actual, bytes);
        assert_eq!(staging, identity);
        previous = bytes;
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R027 STAGING REARM N{n}: rearm={remaps} same_buffer=true completed_writer_before_map=true remap_polls=1/Poll remap_submits=0 abandoned_readback=[0;64] submitted_bytes={actual:?}\n",
        )).unwrap();
    }
    assert_eq!(
        (copy_submits, completions, remaps, abandoned, readbacks, preserved_readbacks),
        (3, 3, 2, 2, 5, 2)
    );
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R027 STAGING REARM N{n}: staging_buffers_created=1 staging_bytes=64 same_buffer_for_3_uses=true actual_copy_submits={copy_submits} completion_callbacks={completions} rearm_map_callbacks={remaps} witness_remaps={preserved_readbacks} abandoned_encoders={abandoned} independent_readback_submits={readbacks} preserved_source_readbacks={preserved_readbacks} total_submits={} no_staging_Queue_write no_sleep no_extra_remap_submit\n",
        copy_submits + readbacks + preserved_readbacks,
    )).unwrap();
}

fn r027_submission_stage_controls(renderer: &mut WgpuRenderer, context: &Context3D, n: u32) {
    use crate::three_d::hdr::{HdrOutput3D, HdrProofSceneWriter3D, HdrSubmissionStage3D as Stage};
    let gpu = renderer.gpu_resources();
    r027_encoder_local_upload_controls(&gpu, n);
    r027_reusable_mapped_upload_controls(&gpu, n);
    let fixture = R027SceneFixture::new(context, n, 1, [[4.0, 2.0, 1.0]; 4], 1, 0.0);
    let mut output = HdrOutput3D::new(context, 32, 32, n, &fixture.source).unwrap();
    let (staging, destination, bytes) = r027_encoder_upload_fixture(&gpu);
    let id = output.texture_id();
    let before = custom_render_audit::snapshot();
    let event_start = custom_render_audit::events().len();
    assert_eq!(
        output.render_with_submission_stage(&fixture.source, |e, s, d| {
            e.copy_buffer_to_buffer(&staging, 0, &destination, 0, 64);
            fixture.encode(e, s, d);
        }),
        Ok(Stage::Published)
    );
    let published = custom_render_audit::snapshot();
    assert_eq!(published.custom_submissions, before.custom_submissions + 1);
    assert_eq!(
        published.custom_registrations,
        before.custom_registrations + 1
    );
    assert_eq!(
        published.custom_command_encoders,
        before.custom_command_encoders + 1
    );
    assert_eq!(output.swaps_for_test(), 1);
    assert_eq!(
        &custom_render_audit::events()[event_start..],
        &[
            CustomRenderAuditEvent::Submission,
            CustomRenderAuditEvent::Registration
        ]
    );
    assert_eq!(read_r027_upload_bytes(&gpu, &destination), bytes);
    drop(staging);
    let staged_pixel = read_r027_fp16(output.front_for_test(), &gpu);
    renderer.drain_pending_image_registrations();
    let staged_image = renderer.render_registered_image_pixel_for_test(
        id,
        [16.5 / 32.0; 2],
        [1.0; 4],
        wgpu::Color::WHITE,
    );
    let before = custom_render_audit::snapshot();
    output
        .render(&fixture.source, |e, s, d| fixture.encode(e, s, d))
        .unwrap();
    let wrapped = custom_render_audit::snapshot();
    assert_eq!(wrapped.custom_submissions, before.custom_submissions + 1);
    assert_eq!(
        wrapped.custom_registrations,
        before.custom_registrations + 1
    );
    assert_eq!(output.swaps_for_test(), 2);
    assert_eq!(output.texture_id(), id);
    assert_eq!(
        read_r027_fp16(output.front_for_test(), &gpu).map(f32::to_bits),
        staged_pixel.map(f32::to_bits)
    );
    renderer.drain_pending_image_registrations();
    assert_eq!(
        renderer.render_registered_image_pixel_for_test(
            id,
            [16.5 / 32.0; 2],
            [1.0; 4],
            wgpu::Color::WHITE,
        ),
        staged_image
    );
    let mut invalid = fixture.source.clone();
    invalid.exposure_ev = 17.0;
    let expected = invalid.exposure().unwrap_err();
    let before = custom_render_audit::snapshot();
    for _ in 0..2 {
        assert_eq!(
            output.render_with_submission_stage(&invalid, |_, _, _| panic!("invalid source wrote")),
            Err((expected.clone(), Stage::BeforeWrites))
        );
        assert_eq!(
            output.render(&invalid, |_, _, _| panic!("invalid source wrote")),
            Err(expected.clone())
        );
    }
    assert_eq!(custom_render_audit::snapshot(), before);
    assert_eq!(output.swaps_for_test(), 2);
    assert_eq!(
        output.render_with_submission_stage(&fixture.source, |e, s, d| fixture.encode(e, s, d)),
        Ok(Stage::Published)
    );
    assert_eq!(
        output.swaps_for_test(),
        3,
        "valid source recovers under the same live owner"
    );
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R027 SUBMISSION STAGES N{n}: PUBLISHED frame submits=1 swaps=1 registrations=1; wrapper FP16={staged_pixel:?} Image={staged_image:?} exact; repeated BEFORE_WRITES source errors unchanged, valid recovery checked\n",
    )).unwrap();
    drop(output);
    renderer.drain_pending_image_registrations();

    for close_after_submit in [false, true] {
        let mut owner = Some(
            pollster::block_on(WgpuRenderer::new_headless()).expect("actual HDR stage Device"),
        );
        let gpu = owner.as_ref().unwrap().gpu_resources();
        let context = Context3D::new(gpu.clone()).unwrap();
        let fixture = R027SceneFixture::new(&context, n, 1, [[4.0, 2.0, 1.0]; 4], 1, 0.0);
        let mut output = HdrOutput3D::new(&context, 32, 32, n, &fixture.source).unwrap();
        let (staging, destination, bytes) = r027_encoder_upload_fixture(&gpu);
        let material = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("R027 stage-control material writes"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let block = PbrMaterialSource3D::defaults(1, 1).block();
        let called = std::cell::Cell::new(false);
        let before = custom_render_audit::snapshot();
        let owner = std::cell::RefCell::new(owner.take());
        let result = output.render_interposed_for_test(
            &fixture.source,
            |e, s, d| {
                called.set(true);
                gpu.queue
                    .write_buffer(&material, 0, bytemuck::bytes_of(&block));
                e.copy_buffer_to_buffer(&staging, 0, &destination, 0, 64);
                fixture.encode(e, s, d);
            },
            || {
                if !close_after_submit {
                    drop(owner.borrow_mut().take());
                }
            },
            || {
                if close_after_submit {
                    drop(owner.borrow_mut().take());
                }
            },
        );
        let stage = if close_after_submit {
            Stage::SubmittedSwappedUnpublished
        } else {
            Stage::WritesQueuedNoFrameSubmit
        };
        assert_eq!(
            result,
            Err((
                crate::three_d::hdr::HdrError3D::Target(
                    "renderer retired before output publication"
                ),
                stage
            ))
        );
        assert!(called.get(), "the actual writer queued its material bytes");
        let after = custom_render_audit::snapshot();
        let submits = u64::from(close_after_submit);
        assert_eq!(
            after.custom_submissions,
            before.custom_submissions + submits
        );
        assert_eq!(after.custom_registrations, before.custom_registrations);
        assert_eq!(
            after.custom_command_encoders,
            before.custom_command_encoders + 1
        );
        assert_eq!(output.swaps_for_test(), submits);
        assert!(!output.is_live(), "closed owner has no sampleable output");
        let staging = if close_after_submit {
            Some(staging)
        } else {
            drop(staging);
            None
        };
        let copied = read_r027_upload_bytes(&gpu, &destination);
        assert_eq!(copied, if close_after_submit { bytes } else { [0; 64] });
        drop(staging);
        let before = custom_render_audit::snapshot();
        let expected = crate::three_d::hdr::HdrError3D::Target("retired renderer/target");
        for _ in 0..2 {
            assert_eq!(
                output.render_with_submission_stage(&fixture.source, |_, _, _| panic!(
                    "closed owner wrote"
                )),
                Err((expected.clone(), Stage::BeforeWrites))
            );
            assert_eq!(
                output.render(&fixture.source, |_, _, _| panic!("closed owner wrote")),
                Err(expected.clone())
            );
        }
        assert_eq!(custom_render_audit::snapshot(), before);
        assert_eq!(output.swaps_for_test(), submits);
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R027 SUBMISSION STAGES N{n}: controlled renderer close stage={stage:?} writer_called=true original Tone16+Material64 queued; actual frame submits={submits} swaps={submits} registrations=0; additional encoder-local64={copied:?} after independent readback submit=1/readback=1; repeated closed-owner errors BEFORE_WRITES; no recycle or recovery claim\n",
        )).unwrap();
    }
}

#[test]
fn r027_mrt4_feature_free_pair_stress_and_late_depth_n1_n4() {
    use crate::three_d::hdr::{HdrOutput3D, HdrProofSceneWriter3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    assert_eq!(gpu.device.features(), wgpu::Features::empty());
    let context = Context3D::new(gpu.clone()).unwrap();
    for n in [1, 4] {
        let mut pair: [Vec<[u32; 4]>; 2] = Default::default();
        for (p, (color, ev)) in [(2f32.powi(-26), 16.0), (2f32.powi(-24), 14.0)]
            .into_iter()
            .enumerate()
        {
            for k in 0..=n {
                let fixture = R027SceneFixture::new(&context, n, 1, [[color; 3]; 4], k, ev);
                let mut output = HdrOutput3D::new(&context, 8, 8, n, &fixture.source).unwrap();
                output
                    .render(&fixture.source, |e, s, d| fixture.encode(e, s, d))
                    .unwrap();
                let actual = read_r027_fp16(output.front_for_test(), &gpu);
                let alpha = k as f32 / n as f32;
                assert_eq!(actual[3], alpha);
                for c in &actual[..3] {
                    assert!(c.is_finite() && (0.0..=alpha).contains(c));
                    assert!((f64::from(*c) - f64::from(alpha) / 1025.0).abs() <= 2f64.powi(-20));
                }
                if k == 0 {
                    assert_eq!(actual, [0.0; 4]);
                }
                pair[p].push(actual.map(f32::to_bits));
                std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!(
                        "R027_MRT4_PAIR N{n} k{k} EV{ev} source={color:?} final={actual:?}\n"
                    ),
                )
                .unwrap();
            }
        }
        assert_eq!(pair[0], pair[1]);
        for (rgb, ev) in [
            ([60000.0; 3], 16.0),
            ([60000.0; 3], -16.0),
            ([60000.0, 2f32.powi(-26), 0.0], 16.0),
            ([0.0; 3], 16.0),
        ] {
            let fixture = R027SceneFixture::new(&context, n, 1, [rgb; 4], n, ev);
            let mut output = HdrOutput3D::new(&context, 8, 8, n, &fixture.source).unwrap();
            output
                .render(&fixture.source, |e, s, d| fixture.encode(e, s, d))
                .unwrap();
            let actual = read_r027_fp16(output.front_for_test(), &gpu);
            assert_eq!(actual[3], 1.0);
            for c in 0..3 {
                let e = 2f64.powf(f64::from(ev)) * f64::from(rgb[c]);
                assert!((f64::from(actual[c]) - e / (1.0 + e)).abs() <= 2f64.powi(-11));
                assert!(actual[c].is_finite() && (0.0..=1.0).contains(&actual[c]));
            }
        }
        let near_k = if n == 4 { 2 } else { 1 };
        let far = R027SceneFixture::at_depth(&context, n, 1, [[60000.0; 3]; 4], n, 16.0, 0.25);
        let near = R027SceneFixture::at_depth(
            &context,
            n,
            1,
            [[2f32.powi(-26); 3]; 4],
            near_k,
            16.0,
            0.75,
        );
        let late = R027SceneFixture::at_depth(&context, n, 1, [[0.0; 3]; 4], n, 16.0, 0.125);
        let mut output = HdrOutput3D::new(&context, 8, 8, n, &far.source).unwrap();
        output
            .render(&far.source, |e, s, d| {
                R027SceneFixture::encode_layers(&[&far, &near, &late], e, s, d)
            })
            .unwrap();
        let actual = read_r027_fp16(output.front_for_test(), &gpu);
        assert_eq!(actual[3], 1.0);
        let low = 1.0 / 1025.0;
        let high = 3932160000.0 / (1.0 + 3932160000.0);
        let ideal = (f64::from(near_k) * low + f64::from(n - near_k) * high) / f64::from(n);
        for c in &actual[..3] {
            assert!(c.is_finite() && (0.0..=1.0).contains(c));
            assert!((f64::from(*c) - ideal).abs() <= 2f64.powi(-11));
        }
        std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!("R027_MRT4_DEPTH N{n} near={near_k} ideal={ideal:?} final={actual:?}\n"),
        )
        .unwrap();
    }
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn r027_mrt4_production_mapped_pbr_emission_and_four_spots_n1_n4() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::direct_shadow::{DirectSource3D, PointLight3D, SpotLight3D};
    use crate::three_d::mip_sampling::{MipMin3D, MipRole3D};
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    assert_eq!(gpu.device.features(), wgpu::Features::empty());
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        for lit in [false, true] {
            let mut pair = Vec::new();
            for (p, ev) in [16.0, 14.0].into_iter().enumerate() {
                let scalar = 2f32.powi(if p == 0 { -26 } else { -24 });
                let mut material = r028_presence_material(&context);
                material.identity = 27_420 + p as u64;
                material.normal_scale = 1.0;
                material.emissive_rgb = if lit { [0.0; 3] } else { [scalar; 3] };
                material.roles[0] = Some(
                    r028_role(
                        &context,
                        producer::produce(
                            MipRole3D::OpaqueBase,
                            2,
                            2,
                            [128, 64, 32, 255].repeat(4),
                            27_430 + p as u64,
                            1,
                            true,
                        ),
                        27_440 + p as u64,
                        MipMin3D::LinearMipLinear,
                        Linear,
                        ClampToEdge,
                        ClampToEdge,
                    )
                    .0,
                );
                assert!(material.roles.iter().all(Option::is_some));
                let mut source = r024_source(
                    &context,
                    r028_quad([1.0; 2], [0.0; 2]),
                    material,
                    n,
                    false,
                    r024_identity(),
                    1,
                );
                let s = Arc::get_mut(&mut source).unwrap();
                s.hdr.exposure_ev = ev;
                s.hdr.maximum_rgb = [64.0; 3];
                if lit {
                    // Four fixed WORLD positions, unchanged between the radiometric pair.
                    // Radius>=.5, four sources and the original r=1 evaluator give
                    // a conservative whole-domain bound below64, independent of pixels.
                    s.direct = Some(Arc::new(DirectSource3D {
                        identity: 27_450,
                        revision: 1,
                        meters_per_scene_unit: 1.0,
                        directional: None,
                        points: Arc::from([]),
                        spots: [[-0.4, -0.3], [0.4, -0.3], [-0.4, 0.3], [0.4, 0.3]]
                            .into_iter()
                            .map(|[x, y]| SpotLight3D {
                                point: PointLight3D {
                                    position: WorldPoint3::try_new(x, y, 2.0).unwrap(),
                                    intensity: [scalar; 3],
                                    radius_m: 0.5,
                                    range_m: 10.0,
                                },
                                emitter_to_scene: [0.0, 0.0, -1.0],
                                inner_radians: 0.2,
                                outer_radians: 0.7,
                            })
                            .collect::<Vec<_>>()
                            .into(),
                        snapshot: s.snapshot.clone(),
                        model: s.model.clone(),
                    }));
                }
                let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
                let metrics = active_metrics(27_460, 32, 32, 1.0);
                viewport.prepare_custom_render(&metrics);
                assert!(
                    viewport.pbr_failure(metrics.surface_id).is_none(),
                    "{:?}",
                    viewport.pbr_failure(metrics.surface_id)
                );
                let actual = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
                assert_eq!(actual[3], 1.0);
                assert!(
                    actual[..3]
                        .iter()
                        .all(|c| c.is_finite() && *c > 0.0 && *c <= 1.0)
                );
                if !lit {
                    for c in &actual[..3] {
                        assert!((f64::from(*c) - 1.0 / 1025.0).abs() <= 2f64.powi(-20));
                    }
                }
                pair.push(actual.map(f32::to_bits));
                std::io::Write::write_fmt(&mut std::io::stderr(),format_args!(
                    "R027_MRT4_PRODUCTION N{n} normal=mapped roles=5 spots={} EV{ev} source_scalar={scalar:?} final={actual:?}\n",if lit {4} else {0}
                )).unwrap();
                if !lit && p == 0 {
                    let resized = active_metrics(27_460, 800, 600, 1.0);
                    viewport.prepare_custom_render(&resized);
                    assert!(viewport.pbr_failure(resized.surface_id).is_none());
                    assert_eq!(
                        read_r027_fp16(viewport.hdr_front_for_test(resized.surface_id), &gpu)
                            .map(f32::to_bits),
                        actual.map(f32::to_bits)
                    );
                    assert_eq!(
                        viewport.hdr_front_for_test(resized.surface_id).size(),
                        wgpu::Extent3d {
                            width: 800,
                            height: 600,
                            depth_or_array_layers: 1,
                        }
                    );
                }
                drop(viewport);
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
            assert_eq!(
                pair[0], pair[1],
                "same original material/view/lighting geometry; source energy x4 and EV-2"
            );
        }
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn r027_mrt4_real_payload_and_oversized_direct_refusal_before_target_allocation() {
    use crate::three_d::hdr::{HdrOutput3D, HdrSource3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let source = HdrSource3D {
        identity: 27_470,
        revision: 1,
        exposure_ev: 0.0,
        maximum_rgb: [1.0; 3],
    };
    for (width, height, n, bytes) in [
        (1920, 1080, 4, 199_065_600),
        (2048, 1024, 4, 201_326_592),
        (800, 600, 1, 17_280_000),
    ] {
        let output = HdrOutput3D::new(&context, width, height, n, &source).unwrap();
        assert_eq!(
            context.target_charges_for_test(),
            (u64::from(width) * u64::from(height), bytes)
        );
        drop(output);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
    let scene = r025_source(&context, 4, 2, 1.0);
    let mut viewport = r024_viewport(context.clone(), scene, r025_view());
    let metrics = active_metrics(27_471, 4097, 2048, 1.0);
    viewport.prepare_custom_render(&metrics);
    assert!(matches!(
        viewport.material_failure_for_test(metrics.surface_id),
        Some(crate::three_d::pbr::PbrError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                requested: 860_028_928,
                maximum: 536_870_912,
            }
        ))
    ));
    assert!(
        viewport
            .target_texture_id_for_test(metrics.surface_id)
            .is_none()
    );
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn r027_mrt4_legacy_appearance_keeps_single_target_and_hdr_pipeline_variants_separate() {
    use crate::three_d::hdr::HdrSource3D;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    for samples in [SampleCount3D::ONE, SampleCount3D::FOUR] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let geometry = context.register_geometry(geometry(true)).unwrap();
        let appearance = context
            .register_appearance(
                AppearanceSourceDescriptor::try_new_with_mode(
                    constant_source([1.0; 3]),
                    GeometrySemantics::NONE,
                    AppearanceMode::Opaque,
                )
                .unwrap(),
            )
            .unwrap();
        let packet = DrawPacketDescriptor::new(
            geometry.handle().clone(),
            appearance.handle().clone(),
            RasterMode::SingleSided,
            0,
        );
        let snapshot = context
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(1),
                vec![packet.clone()],
            ))
            .unwrap();
        let mut baseline = make_viewport(
            context.clone(),
            Viewport3DViewRevision::new(1),
            r024_view(2.0),
            snapshot,
            samples,
        )
        .unwrap();
        let metrics = active_metrics(27_480, 32, 32, 1.0);
        baseline.prepare_custom_render(&metrics);
        let baseline_id = baseline
            .target_texture_id_for_test(metrics.surface_id)
            .unwrap();
        renderer.drain_pending_image_registrations();
        assert_eq!(
            renderer.render_registered_image_pixel_for_test(
                baseline_id,
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::TRANSPARENT
            ),
            [255; 4]
        );
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 2);
        let snapshot = context
            .admit_material_snapshot(
                StructuralRevision::new(1),
                vec![MaterialPacketSource3D::LegacyAppearance(packet)],
            )
            .unwrap();
        let model = context
            .admit_material_model(
                snapshot.clone(),
                ModelRevision::new(1),
                vec![r024_identity()],
            )
            .unwrap();
        let scene = Arc::new(MaterialSceneSource3D {
            snapshot,
            model,
            hdr: HdrSource3D {
                identity: 27_481,
                revision: 1,
                exposure_ev: 0.0,
                maximum_rgb: [1.0; 3],
            },
            samples: samples.value(),
            direct: None,
            indirect: None,
            observe: false,
        });
        let mut hdr = r024_viewport(context.clone(), scene, r024_view(2.0));
        hdr.prepare_custom_render(&active_metrics(27_482, 32, 32, 1.0));
        assert!(
            hdr.material_failure_for_test(SurfaceId::test_id(27_482))
                .is_none()
        );
        assert_eq!(
            read_r027_fp16(hdr.hdr_front_for_test(SurfaceId::test_id(27_482)), &gpu),
            [0.5, 0.5, 0.5, 1.0]
        );
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 4);
        assert_eq!(
            baseline.target_texture_id_for_test(metrics.surface_id),
            Some(baseline_id)
        );
        renderer.drain_pending_image_registrations();
        assert_eq!(
            renderer.render_registered_image_pixel_for_test(
                baseline_id,
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::TRANSPARENT
            ),
            [255; 4]
        );
        drop((baseline, hdr));
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn r027_actual_n1_n4_tone_fp16_and_target_bound_image_composition() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R027: actual Device required");
    let adapter = renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    // Direct process stderr keeps successful Device/readback evidence in the
    // ordinary test log; libtest captures println output on passing tests.
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("R027 actual Device: {adapter:?}; pinned wgpu30.0.2/Naga30.0.2\n"),
    )
    .unwrap();
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let cases = [
        (1, 1, [[4.0, 2.0, 1.0]; 4], 0.0, [0.8, 2.0 / 3.0, 0.5, 1.0]),
        (
            4,
            1,
            [[4.0, 2.0, 1.0]; 4],
            0.0,
            [0.2, 1.0 / 6.0, 0.125, 0.25],
        ),
        (1, 1, [[0.0; 3]; 4], 0.0, [0.0, 0.0, 0.0, 1.0]),
        (1, 0, [[0.0; 3]; 4], 0.0, [0.0; 4]),
        (4, 0, [[0.0; 3]; 4], 0.0, [0.0; 4]),
        (
            4,
            2,
            [[0.0; 3], [4.0, 2.0, 1.0], [0.0; 3], [0.0; 3]],
            0.0,
            [0.2, 1.0 / 6.0, 0.125, 0.5],
        ),
        (4, 2, [[1.0; 3]; 4], 16.0, [0.5, 0.5, 0.5, 0.5]),
        (4, 3, [[1.0; 3]; 4], 16.0, [0.75, 0.75, 0.75, 0.75]),
        (1, 1, [[60000.0; 3]; 4], 16.0, [1.0; 4]),
    ];
    for (case, (n, k, colors, ev, expected)) in cases.into_iter().enumerate() {
        if case < 2 {
            r027_submission_stage_controls(&mut renderer, &context, n);
        }
        let samples = SampleCount3D::try_new(n).unwrap();
        let fixture = R027SceneFixture::new(&context, n, case as u64 + 1, colors, k, ev);
        let viewport = r027_viewport(context.clone(), samples, fixture);
        let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (32, 32), 1.0)
            .unwrap();
        let mut tree = WidgetTree::new();
        let panel = tree.create_root(
            Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
            None,
            0,
        );
        let child = tree.add_child(panel, Box::new(viewport), None, 0);
        tree.replace_widget_only(
            panel,
            Box::new(
                Panel::new(child)
                    .with_padding(0.0)
                    .with_background(crate::core::paint::Color::WHITE),
            ),
        );
        let mut harness = RetainedSceneTestHarness::new();
        let frame = harness.frame_with_custom_render_target(
            &mut tree,
            panel,
            Constraints::tight(Size::new(32.0, 32.0)),
            &Theme::default(),
            &mut FontManagerTextEngine::new(),
            &IconRegistry::new(&[]),
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(32, 32), 1.0).unwrap(),
        );
        let viewport = tree_viewport(&tree, child);
        assert_eq!(
            viewport.status_for(surface).unwrap().state(),
            Viewport3DLifecycleState::Committed
        );
        let actual = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!("R027 N{n} k{k} EV{ev}: FP16={actual:?}\n"),
        )
        .unwrap();
        assert_eq!(actual[3], expected[3]);
        for channel in 0..3 {
            // One FP16 unit at 1.0 bounds storage rounding plus f32 tone error.
            assert!(
                (actual[channel] - expected[channel]).abs() <= 1.0 / 1024.0,
                "N{n} k{k}: {actual:?} != {expected:?}"
            );
            assert!(
                actual[channel] <= actual[3],
                "sampled premultiplied-linear front"
            );
        }
        let admission = viewport.custom_render_admission().unwrap();
        let mut bindings = CustomRenderPresentationBindings::default();
        bindings
            .publish_for_target(
                surface,
                admission.presentation_id(),
                admission
                    .presentation_binding(viewport.target_texture_id_for_test(surface).unwrap()),
            )
            .unwrap();
        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root;
        let scenes = std::collections::HashMap::from([(surface, scene)]);
        let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            &mut bindings.begin_materialization_for_test(0),
            |_| 1.0,
            0,
        ).expect("scene must satisfy DrawPlan Build admission");
        renderer.drain_pending_image_registrations();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let pixel = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
        for channel in 0..3 {
            let stored = f32::from(pixel[channel]) / 255.0;
            let linear = if stored <= 0.04045 {
                stored / 12.92
            } else {
                ((stored + 0.055) / 1.055).powf(2.4)
            };
            let expected_over_white = actual[channel] + 1.0 - actual[3];
            assert!(
                (linear - expected_over_white).abs() < 0.01,
                "actual Image sample/source-over/sRGB: {pixel:?}, expected {expected_over_white}"
            );
        }
        assert_eq!(pixel[3], 255);
        drop(compiled);
        drop(tree);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 0);
    }
}

#[test]
fn r027_stable_target_failures_resize_inactive_and_retirement_keep_exact_front_provenance() {
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R027: Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let fixture = R027SceneFixture::new(&context, 4, 1, [[4.0, 2.0, 1.0]; 4], 1, 0.0);
    let mut viewport = r027_viewport(context.clone(), SampleCount3D::FOUR, fixture);
    let metrics = active_metrics(27, 32, 32, 1.0);
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::Committed
    );
    let original = viewport
        .status_for(metrics.surface_id)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    let id = viewport
        .target_texture_id_for_test(metrics.surface_id)
        .unwrap();
    let before = custom_render_audit::snapshot();
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        custom_render_audit::snapshot(),
        before,
        "stable source has no encoder/submit/registration/allocation"
    );
    assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), 1);
    let valid = R027SceneFixture::new(&context, 4, 2, [[4.0, 2.0, 1.0]; 4], 1, 0.0);
    let invalid = Arc::new(R027SceneFixture {
        source: crate::three_d::hdr::HdrSource3D {
            exposure_ev: 17.0,
            ..valid.source.clone()
        },
        pipeline: valid.pipeline.clone(),
        _module: valid._module.clone(),
        gpu: valid.gpu.clone(),
        samples: valid.samples,
    });
    assert!(
        viewport
            .set_hdr_fixture_for_test(invalid, SampleCount3D::FOUR)
            .is_err()
    );
    assert_eq!(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .committed()
            .unwrap()
            .request_serial(),
        original
    );
    assert_eq!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        Some(id)
    );
    viewport.prepare_custom_render(&active_metrics(27, 40, 32, 1.0));
    let next = viewport
        .target_texture_id_for_test(metrics.surface_id)
        .unwrap();
    assert_ne!(next, id);
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let one = R027SceneFixture::new(&context, 1, 2, [[1.0; 3]; 4], 1, -16.0);
    viewport
        .set_hdr_fixture_for_test(one, SampleCount3D::ONE)
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    assert_ne!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        Some(next)
    );
    viewport.prepare_custom_render(&inactive_metrics(27, 32, 32));
    assert_eq!(
        viewport.target_texture_id_for_test(metrics.surface_id),
        None
    );
    let before = custom_render_audit::snapshot();
    viewport.prepare_custom_render(&inactive_metrics(27, 32, 32));
    assert_eq!(custom_render_audit::snapshot(), before);
    assert_eq!(context.target_charges_for_test().0, 0);
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn r027_actual_old_final_pair_blocks_successor_until_renderer_removal_and_queue_completion() {
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R027: Device required");
    let gpu = renderer.gpu_resources();
    let mut policy = PolicyLimits3D::conservative();
    // Exactly one target, as before: update the fixture's physical charge, not
    // the production limits. A retained old final pair still blocks its successor.
    policy.max_total_attachment_bytes = 32 * 32 * (20 * 4 + 16);
    policy.max_single_target_attachment_bytes = policy.max_total_attachment_bytes;
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
    let fixture = R027SceneFixture::new(&context, 4, 1, [[1.0; 3]; 4], 2, 16.0);
    let mut a = r027_viewport(context.clone(), SampleCount3D::FOUR, fixture.clone());
    let mut b = r027_viewport(context.clone(), SampleCount3D::FOUR, fixture);
    let first = active_metrics(271, 32, 32, 1.0);
    let second = active_metrics(272, 32, 32, 1.0);
    a.prepare_custom_render(&first);
    renderer.drain_pending_image_registrations();
    let actual = read_r027_fp16(a.hdr_front_for_test(first.surface_id), &gpu);
    assert_eq!(actual[3], 0.5);
    assert!(
        actual[..3]
            .iter()
            .all(|rgb| *rgb <= actual[3] && (rgb - 0.5).abs() <= 1.0 / 1024.0)
    );
    a.prepare_custom_render(&inactive_metrics(271, 32, 32));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap(); // Completion FIRST cannot release final custody.
    assert_eq!(context.target_charges_for_test(), (0, 16_384));
    b.prepare_custom_render(&second);
    assert!(matches!(
        b.status_for(second.surface_id)
            .unwrap()
            .failure()
            .unwrap()
            .error(),
        Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TotalAttachmentBytes,
            requested: 114_688,
            maximum: 98_304
        })
    ));
    assert!(b.target_texture_id_for_test(second.surface_id).is_none());
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
    // Actual capacity release retries the SAME immutable B request; a synthetic
    // source revision is neither needed nor permitted as a retirement signal.
    b.prepare_custom_render(&second);
    assert!(b.target_texture_id_for_test(second.surface_id).is_some());
    assert_eq!(context.target_charges_for_test(), (1024, 98_304));
    drop((a, b));
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn r027_renderer_shutdown_withdraws_target_and_blocks_idle_or_first_use_gpu_work() {
    let renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R027: Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let fixture = R027SceneFixture::new(&context, 1, 1, [[4.0, 2.0, 1.0]; 4], 1, 0.0);
    let mut viewport = r027_viewport(context.clone(), SampleCount3D::ONE, fixture.clone());
    assert!(
        viewport
            .set_hdr_fixture_for_test(fixture, SampleCount3D::FOUR)
            .is_err(),
        "sample-incompatible producer refused before writes"
    );
    let metrics = active_metrics(273, 32, 32, 1.0);
    viewport.prepare_custom_render(&metrics);
    assert!(
        viewport
            .target_texture_id_for_test(metrics.surface_id)
            .is_some()
    );
    drop(renderer);
    assert!(
        viewport
            .target_texture_id_for_test(metrics.surface_id)
            .is_none(),
        "closed registration owner cannot remain sampleable"
    );
    let before = custom_render_audit::snapshot();
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::Retired
    );
    let retired = custom_render_audit::snapshot();
    assert_eq!(retired.custom_submissions, before.custom_submissions);
    viewport.prepare_custom_render(&metrics);
    assert_eq!(custom_render_audit::snapshot(), retired);
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 0);
    let before = viewport_audit::snapshot();
    assert!(crate::three_d::hdr::ToneProgram3D::new(&context, 1).is_err());
    assert_eq!(
        viewport_audit::snapshot(),
        before,
        "no new controlled module after terminal closure"
    );
}

fn render_compiled_surface_center_pixel(
    renderer: &WgpuRenderer,
    compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
    surface_id: SurfaceId,
) -> [u8; 4] {
    let gpu = renderer.gpu_resources();
    let descriptor = compiled
        .get_target(surface_id)
        .expect("compiled target must exist");
    let (width, height) = descriptor.physical_size;
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("retained composition output"),
        size: wgpu::Extent3d {
            width,
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
    let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let render = renderer
        .encode_compiled_target_for_test(compiled, surface_id, &output_view)
        .expect("compiled surface data must exist");

    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("retained composition readback"),
        size: u64::from(padded_bytes_per_row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("retained composition copy"),
        });
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
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
    renderer.submit_compiled_target_for_test(render);
    gpu.queue.submit([copy.finish()]);

    let done = Arc::new(AtomicBool::new(false));
    let succeeded = Arc::new(AtomicBool::new(false));
    let done_for_callback = done.clone();
    let succeeded_for_callback = succeeded.clone();
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        succeeded_for_callback.store(result.is_ok(), Ordering::Release);
        done_for_callback.store(true, Ordering::Release);
    });
    while !done.load(Ordering::Acquire) {
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }
    assert!(
        succeeded.load(Ordering::Acquire),
        "retained composition readback failed"
    );
    let data = slice.get_mapped_range().expect("mapped readback range");
    let offset = ((height / 2) * padded_bytes_per_row + (width / 2) * 4) as usize;
    let stored = [
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ];
    drop(data);
    readback.unmap();

    match gpu.format {
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            [stored[2], stored[1], stored[0], stored[3]]
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => stored,
        format => panic!("unsupported retained composition target format {format:?}"),
    }
}

fn point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

fn view_at(x: f64) -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(x, 0.0, 0.0).unwrap(),
            Orientation3::identity(),
        ),
        PerspectiveProjection::try_finite(std::f32::consts::FRAC_PI_2, 0.1, 10.0)
            .unwrap()
            .into(),
    )
}

fn geometry(full: bool) -> GeometrySourceDescriptor {
    let positions = if full {
        vec![
            point(-1.2, -1.2, 0.0),
            point(1.2, -1.2, 0.0),
            point(1.2, 1.2, 0.0),
            point(-1.2, -1.2, 0.0),
            point(1.2, 1.2, 0.0),
            point(-1.2, 1.2, 0.0),
        ]
    } else {
        vec![
            point(-0.78, -0.74, 0.0),
            point(0.82, -0.66, 0.0),
            point(-0.07, 0.79, 0.0),
        ]
    };
    GeometrySourceDescriptor::try_new(positions, None, None, None, GeometryIndices::Unindexed)
        .unwrap()
}

fn constant_source(color: [f32; 3]) -> String {
    format!(
        r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {{
    return vec3<f32>({:.8}, {:.8}, {:.8});
}}
"#,
        color[0], color[1], color[2]
    )
}

fn snapshot(
    context: &Context3D,
    revision: u64,
    color: [f32; 3],
    raster_mode: RasterMode,
    full: bool,
) -> RenderSnapshot {
    let geometry = context.register_geometry(geometry(full)).unwrap();
    let appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new_with_mode(
                constant_source(color),
                GeometrySemantics::NONE,
                AppearanceMode::Opaque,
            )
            .unwrap(),
        )
        .unwrap();
    context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(revision),
            vec![DrawPacketDescriptor::new(
                geometry.handle().clone(),
                appearance.handle().clone(),
                raster_mode,
                0,
            )],
        ))
        .unwrap()
}

fn model(context: &Context3D, snapshot: &RenderSnapshot, revision: u64) -> ModelState {
    model_at(context, snapshot, revision, 0.0)
}

fn model_at(context: &Context3D, snapshot: &RenderSnapshot, revision: u64, x: f64) -> ModelState {
    context
        .admit_model_state(
            snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(revision),
                vec![
                    ModelPlacement3::try_new(
                        WorldPoint3::try_new(x, 0.0, -1.0).unwrap(),
                        Orientation3::identity(),
                        PositiveScale3::one(),
                    )
                    .unwrap(),
                ],
            ),
        )
        .unwrap()
}

fn make_viewport(
    context: Arc<Context3D>,
    view_revision: Viewport3DViewRevision,
    view: ViewDescriptor3,
    snapshot: RenderSnapshot,
    sample_count: SampleCount3D,
) -> Result<Viewport3D, Viewport3DError> {
    let model = model(&context, &snapshot, snapshot.structural_revision().value());
    Viewport3D::new(context, view_revision, view, snapshot, model, sample_count)
}

trait ViewportTestExt {
    fn set_scene_request(
        &mut self,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        sample_count: SampleCount3D,
    ) -> Result<(), Viewport3DError>;
}

impl ViewportTestExt for Viewport3D {
    fn set_scene_request(
        &mut self,
        view_revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        sample_count: SampleCount3D,
    ) -> Result<(), Viewport3DError> {
        let model = if self.requested_model_for_test().matches_snapshot(&snapshot) {
            self.requested_model_for_test().clone()
        } else {
            model(
                self.context(),
                &snapshot,
                snapshot.structural_revision().value(),
            )
        };
        self.set_request(view_revision, view, snapshot, model, sample_count)
    }
}

fn active_metrics(
    surface: u64,
    width: u32,
    height: u32,
    scale_factor: f32,
) -> CustomRenderTargetMetrics {
    active_metrics_for_surface(SurfaceId::test_id(surface), width, height, scale_factor)
}

fn active_metrics_for_surface(
    surface_id: SurfaceId,
    width: u32,
    height: u32,
    scale_factor: f32,
) -> CustomRenderTargetMetrics {
    let logical_width = width as f32 / scale_factor;
    let logical_height = height as f32 / scale_factor;
    let logical = RectF::new(0.0, 0.0, logical_width, logical_height);
    CustomRenderTargetMetrics {
        surface_id,
        app_frame_attempt: None,
        final_logical_rect: logical,
        final_logical_viewport: logical,
        target_logical_extent: logical.size(),
        scale_factor,
        target_physical_extent: PhysicalExtent::new(width, height),
        activity: CustomRenderActivity::Active {
            physical_render_extent: PhysicalExtent::new(width, height),
            physical_composition_coverage: PhysicalRect::new(0, 0, width, height),
        },
    }
}

fn inactive_metrics(surface: u64, width: u32, height: u32) -> CustomRenderTargetMetrics {
    let mut metrics = active_metrics(surface, width, height, 1.0);
    metrics.activity = CustomRenderActivity::Inactive;
    metrics
}

fn tree_viewport<'a>(
    tree: &'a WidgetTree,
    root: crate::core::widget_tree::NodeId,
) -> &'a Viewport3D {
    tree.node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<Viewport3D>()
        .unwrap()
}

fn image_node(nodes: &[SceneNode]) -> Option<&crate::core::scene::ImageNode> {
    for node in nodes {
        match node {
            SceneNode::Image(image) => return Some(image),
            SceneNode::RetainedFragment(fragment) => {
                if let Some(image) = image_node(&fragment.children) {
                    return Some(image);
                }
            }
            SceneNode::Clip(clip) => {
                if let Some(image) = image_node(&clip.children) {
                    return Some(image);
                }
            }
            SceneNode::Layer(layer) => {
                if let Some(image) = image_node(&layer.children) {
                    return Some(image);
                }
            }
            SceneNode::Scroll(scroll) => {
                if let Some(image) = image_node(&scroll.children) {
                    return Some(image);
                }
            }
            _ => {}
        }
    }
    None
}

fn target_bound_scene(presentation_id: crate::renderer::CustomRenderPresentationId) -> SceneGraph {
    let mut scene = SceneBuilder::new();
    scene.push_target_bound_image(
        RectF::new(0.0, 0.0, 96.0, 64.0),
        presentation_id,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        crate::core::scene::Color::WHITE,
        0,
        None,
    );
    scene.build()
}

fn draw_plan_image_texture(
    plan: &DrawPlan<MultiSurfaceContract>,
    surface_id: SurfaceId,
) -> Option<u64> {
    plan.surface_scene(surface_id)?
        .primitives
        .iter()
        .find_map(|primitive| match &primitive.data {
            PrimitiveData::Image { texture_id, .. } => Some(*texture_id),
            _ => None,
        })
}

#[test]
fn retained_viewport_prepares_after_layout_paints_one_image_and_reuses_stable_front() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let viewport_widget = make_viewport(
        context,
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(viewport_widget), None, 0);
    assert_eq!(tree.custom_render_participant_count_for_test(), 1);

    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let target =
        CustomRenderTargetSnapshot::new(SurfaceId::test_id(1), PhysicalExtent::new(96, 64), 1.0)
            .unwrap();
    let events_before = custom_render_audit::events().len();
    let first = harness.frame_with_custom_render_target(
        &mut tree,
        root,
        Constraints::tight(Size::new(96.0, 64.0)),
        &theme,
        &mut text,
        &icons,
        target,
    );
    assert!(first.scene_built);
    assert!(first.visual_changed);
    let image = image_node(&first.scene_root).unwrap();
    assert_eq!(image.uv_rect, RectF::new(0.0, 0.0, 1.0, 1.0));
    assert!(matches!(
        image.texture_source,
        ImageTextureSource::TargetBound(_)
    ));
    assert_eq!(
        image.tint,
        crate::core::scene::Color::from(crate::core::paint::Color::WHITE)
    );
    let status = tree_viewport(&tree, root)
        .status_for(SurfaceId::test_id(1))
        .unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
    assert_eq!(
        status.requested().unwrap().request_serial(),
        status.committed().unwrap().request_serial()
    );
    assert_eq!(
        &custom_render_audit::events()[events_before..],
        &[
            CustomRenderAuditEvent::Submission,
            CustomRenderAuditEvent::Registration,
        ]
    );

    let audit_before = viewport_audit::snapshot();
    let retained = harness.frame_with_custom_render_target(
        &mut tree,
        root,
        Constraints::tight(Size::new(96.0, 64.0)),
        &theme,
        &mut text,
        &icons,
        target,
    );
    assert!(!retained.scene_built);
    assert!(!retained.visual_changed);
    assert_eq!(viewport_audit::snapshot(), audit_before);
}

#[test]
fn retained_viewport_inside_rounded_panel_composes_visible_pixels_at_sample_one_and_four() {
    const WIDTH: u32 = 64;
    const HEIGHT: u32 = 64;

    let Some((mut renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let viewport_rect = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let mut compositor = Compositor::new(viewport_rect);
    let surface_one = compositor.create_surface(0, viewport_rect, SceneKey::BASE);
    let surface_four = compositor.create_surface(1, viewport_rect, SceneKey::new(4));
    for surface_id in [surface_one, surface_four] {
        renderer
            .register_headless_render_target(surface_id, (WIDTH, HEIGHT), 1.0)
            .unwrap();
    }

    let mut surface_scenes = std::collections::HashMap::new();
    let mut bindings = CustomRenderPresentationBindings::default();
    let mut retained_viewports = Vec::new();
    for (surface_id, sample_count, revision) in [
        (surface_one, SampleCount3D::ONE, 1),
        (surface_four, SampleCount3D::FOUR, 2),
    ] {
        let viewport = make_viewport(
            context.clone(),
            Viewport3DViewRevision::new(1),
            view,
            snapshot(
                &context,
                revision,
                [1.0, 0.0, 0.0],
                RasterMode::SingleSided,
                true,
            ),
            sample_count,
        )
        .unwrap();
        let mut tree = WidgetTree::new();
        let panel = tree.create_root(
            Box::new(Panel::new(crate::core::widget_tree::NodeId(0))),
            None,
            0,
        );
        let viewport_id = tree.add_child(panel, Box::new(viewport), None, 0);
        tree.replace_widget_only(
            panel,
            Box::new(
                Panel::new(viewport_id)
                    .with_padding(0.0)
                    .with_background(crate::core::paint::Color::from_hex(0x244D63))
                    .with_radius(5.0),
            ),
        );

        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::default();
        let mut text = FontManagerTextEngine::new();
        let icons = IconRegistry::new(&[]);
        let frame = harness.frame_with_custom_render_target(
            &mut tree,
            panel,
            Constraints::tight(Size::new(WIDTH as f32, HEIGHT as f32)),
            &theme,
            &mut text,
            &icons,
            CustomRenderTargetSnapshot::new(surface_id, PhysicalExtent::new(WIDTH, HEIGHT), 1.0)
                .unwrap(),
        );
        let image = image_node(&frame.scene_root).expect("Viewport3D image must be retained");
        assert_eq!(image.z_index, LAYER_CONTENT);

        let viewport = tree_viewport(&tree, viewport_id);
        let admission = viewport.custom_render_admission().unwrap();
        let texture_id = viewport
            .target_texture_id_for_test(surface_id)
            .expect("Viewport3D front must be published");
        bindings
            .publish_for_target(
                surface_id,
                admission.presentation_id(),
                admission.presentation_binding(texture_id),
            )
            .unwrap();

        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root;
        surface_scenes.insert(surface_id, scene);
        retained_viewports.push((tree, harness));
    }

    let mut builder = DrawPlanBuilder::new();
    let plan = builder.build_multi_with_presentation_bindings(
        &surface_scenes,
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut bindings.begin_materialization_for_test(0),
        |_| 1.0,
        0,
    ).expect("scene must satisfy DrawPlan Build admission");
    for surface_id in [surface_one, surface_four] {
        let surface = plan.surface_scene(surface_id).unwrap();
        assert!(
            surface.batches.iter().any(|batch| {
                batch.key.pipeline == PipelineId::SdfRect && batch.key.z_layer == 0
            })
        );
        assert!(surface.batches.iter().any(|batch| {
            batch.key.pipeline == PipelineId::Image && batch.key.z_layer == LAYER_CONTENT
        }));
    }

    renderer.drain_pending_image_registrations();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    for surface_id in [surface_one, surface_four] {
        let pixel = render_compiled_surface_center_pixel(&renderer, &compiled, surface_id);
        assert!(
            pixel[0] > 240 && pixel[1] < 16 && pixel[2] < 16 && pixel[3] > 240,
            "Viewport3D surface {surface_id:?} was covered by its Panel background: {pixel:?}"
        );
    }
    drop(retained_viewports);
}

#[test]
fn surface_dpr_resize_and_inactive_lifecycles_are_exact_and_isolated() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [0.0, 1.0, 0.0], RasterMode::SingleSided, false);
    let mut viewport = make_viewport(
        context,
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();

    let first_inactive = inactive_metrics(99, 100, 50);
    let first_inactive_surface = first_inactive.surface_id;
    let audit_before_first_inactive = viewport_audit::snapshot();
    assert!(
        !viewport
            .prepare_custom_render(&first_inactive)
            .front_publication_changed
    );
    assert_eq!(viewport_audit::snapshot(), audit_before_first_inactive);
    assert_eq!(
        viewport.status_for(first_inactive_surface).unwrap().state(),
        Viewport3DLifecycleState::Inactive
    );
    assert_eq!(
        viewport.target_metrics_for_test(first_inactive_surface),
        Some(first_inactive)
    );
    assert_eq!(
        viewport.composition_coverage_for_test(first_inactive_surface),
        None
    );
    assert!(
        viewport
            .target_texture_id_for_test(first_inactive_surface)
            .is_none()
    );

    let first = active_metrics(1, 100, 50, 1.0);
    assert!(
        viewport
            .prepare_custom_render(&first)
            .front_publication_changed
    );
    let surface_one = SurfaceId::test_id(1);
    let first_serial = viewport
        .status_for(surface_one)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    let first_texture = viewport.target_texture_id_for_test(surface_one).unwrap();

    let resized = active_metrics(1, 150, 75, 1.5);
    assert!(
        viewport
            .prepare_custom_render(&resized)
            .front_publication_changed
    );
    let resized_status = viewport.status_for(surface_one).unwrap();
    let resized_tuple = resized_status.committed().unwrap();
    assert!(resized_tuple.request_serial() > first_serial);
    assert_eq!(
        resized_tuple.target_key().physical_render_extent(),
        PhysicalExtent::new(150, 75)
    );
    assert_ne!(
        viewport.target_texture_id_for_test(surface_one).unwrap(),
        first_texture
    );
    assert_eq!(viewport.target_metrics_for_test(surface_one), Some(resized));
    assert_eq!(
        viewport.composition_coverage_for_test(surface_one),
        Some(PhysicalRect::new(0, 0, 150, 75))
    );

    let surface_two = SurfaceId::test_id(2);
    let second_surface = active_metrics(2, 200, 100, 2.0);
    viewport.prepare_custom_render(&second_surface);
    assert_ne!(
        viewport.target_texture_id_for_test(surface_one),
        viewport.target_texture_id_for_test(surface_two)
    );
    assert_eq!(
        viewport
            .status_for(surface_one)
            .unwrap()
            .committed()
            .unwrap()
            .target_key()
            .physical_render_extent(),
        PhysicalExtent::new(150, 75)
    );
    assert_eq!(
        viewport
            .status_for(surface_two)
            .unwrap()
            .committed()
            .unwrap()
            .target_key()
            .physical_render_extent(),
        PhysicalExtent::new(200, 100)
    );

    let audit_before = viewport_audit::snapshot();
    assert!(
        viewport
            .prepare_custom_render(&inactive_metrics(2, 200, 100))
            .front_publication_changed
    );
    let audit_after = viewport_audit::snapshot();
    assert_eq!(audit_after.targets, audit_before.targets);
    assert_eq!(audit_after.encodes, audit_before.encodes);
    assert_eq!(
        viewport.status_for(surface_two).unwrap().state(),
        Viewport3DLifecycleState::Inactive
    );
    assert!(viewport.target_texture_id_for_test(surface_two).is_none());
    assert!(viewport.target_texture_id_for_test(surface_one).is_some());
}

#[test]
fn terminal_device_failure_withdraws_every_front_and_preserves_target_truth() {
    let Some((renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(
        &context,
        1,
        [0.25, 0.75, 1.0],
        RasterMode::SingleSided,
        false,
    );
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics_a = active_metrics(71, 96, 64, 1.0);
    let metrics_b = active_metrics(72, 144, 96, 1.5);

    assert!(
        viewport
            .prepare_custom_render(&metrics_a)
            .front_publication_changed
    );
    assert!(
        viewport
            .prepare_custom_render(&metrics_b)
            .front_publication_changed
    );
    let surface_a = metrics_a.surface_id;
    let surface_b = metrics_b.surface_id;
    let requested_a = viewport
        .status_for(surface_a)
        .unwrap()
        .requested()
        .unwrap()
        .clone();
    let committed_a = viewport
        .status_for(surface_a)
        .unwrap()
        .committed()
        .unwrap()
        .clone();
    assert_eq!(viewport.target_statuses().len(), 2);
    let current = viewport.current_prepared_target().unwrap();
    assert_eq!(current.surface_id(), surface_b);
    assert_eq!(current.metrics(), Some(metrics_b));

    let gpu = renderer.gpu_resources();
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let context_failure = context
        .device_failure()
        .expect("destroying the captured device must publish terminal status");
    assert_eq!(context_failure.reason(), GpuDeviceFailureReason::Destroyed);

    let terminal_a = viewport.prepare_custom_render(&metrics_a);
    assert!(terminal_a.front_publication_changed);
    assert!(terminal_a.presentation_binding().is_none());
    let status_a = viewport.status_for(surface_a).unwrap();
    assert_eq!(
        status_a.state(),
        Viewport3DLifecycleState::TerminalDeviceFailure
    );
    assert_eq!(
        status_a.requested().unwrap().request_serial(),
        requested_a.request_serial()
    );
    assert_eq!(
        status_a.committed().unwrap().request_serial(),
        committed_a.request_serial()
    );
    assert_eq!(
        status_a.device_failure().unwrap().reason(),
        GpuDeviceFailureReason::Destroyed
    );
    assert!(viewport.target_texture_id_for_test(surface_a).is_none());

    let terminal_b = viewport.prepare_custom_render(&metrics_b);
    assert!(terminal_b.front_publication_changed);
    assert!(terminal_b.presentation_binding().is_none());
    assert!(
        !viewport
            .prepare_custom_render(&metrics_b)
            .front_publication_changed
    );
    assert!(viewport.target_statuses().all(|target| {
        target.status().state() == Viewport3DLifecycleState::TerminalDeviceFailure
            && target.status().device_failure().is_some()
    }));
    let current = viewport.current_prepared_target().unwrap();
    assert_eq!(current.surface_id(), surface_b);
    assert_eq!(current.metrics(), Some(metrics_b));
}

#[test]
fn three_d_wheel_anchor_committed_target_ray_requires_active_committed_target() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(
        &context,
        1,
        [0.25, 0.75, 1.0],
        RasterMode::SingleSided,
        false,
    );
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    assert!(viewport.current_prepared_target().is_none());

    let metrics = active_metrics(91, 96, 64, 1.0);
    assert!(
        viewport
            .prepare_custom_render(&metrics)
            .front_publication_changed
    );
    let current = viewport.current_prepared_target().unwrap();
    let ray = current.committed_ray(0.5, 0.5).unwrap().unwrap();
    assert_eq!(ray.origin(), view.pose().translation());

    let requested_view = view_at(0.1);
    let invalid_model = model_at(&context, &snapshot, 2, 1.0e300);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            requested_view,
            snapshot,
            invalid_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    assert!(
        !viewport
            .prepare_custom_render(&metrics)
            .front_publication_changed
    );
    {
        let failed = viewport.current_prepared_target().unwrap();
        assert_eq!(
            failed.status().state(),
            Viewport3DLifecycleState::FailedWithLastGood
        );
        assert_eq!(
            failed.status().requested().unwrap().view_revision(),
            Viewport3DViewRevision::new(2)
        );
        assert_eq!(failed.status().requested().unwrap().view(), requested_view);
        assert_eq!(
            failed.status().committed().unwrap().view_revision(),
            Viewport3DViewRevision::new(1)
        );
        assert_eq!(failed.status().committed().unwrap().view(), view);
        let last_good_ray = failed.committed_ray(0.5, 0.5).unwrap().unwrap();
        assert_eq!(last_good_ray.origin(), view.pose().translation());
        assert_ne!(last_good_ray.origin(), requested_view.pose().translation());
    }

    let inactive = inactive_metrics(91, 96, 64);
    let _ = viewport.prepare_custom_render(&inactive);
    assert_eq!(
        viewport
            .current_prepared_target()
            .unwrap()
            .committed_ray(0.5, 0.5)
            .unwrap(),
        None
    );
}

#[test]
fn renderer_and_gpu_resources_stay_observer_free_until_shared_context_admission() {
    let before = custom_render_audit::snapshot();
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let after_renderer = custom_render_audit::snapshot();
    assert_eq!(
        after_renderer.device_lifetime_observations,
        before.device_lifetime_observations
    );
    assert_eq!(
        after_renderer.device_lost_callbacks,
        before.device_lost_callbacks
    );

    let gpu = renderer.gpu_resources();
    let sibling_gpu = renderer.gpu_resources();
    let after_gpu_resources = custom_render_audit::snapshot();
    assert_eq!(after_gpu_resources, after_renderer);

    let mut invalid_policy = PolicyLimits3D::conservative();
    invalid_policy.max_live_views = 0;
    assert!(Context3D::with_policy(gpu.clone(), invalid_policy).is_err());
    assert_eq!(custom_render_audit::snapshot(), after_gpu_resources);

    let first = Context3D::new(gpu.clone()).expect("first context admission failed");
    let after_first = custom_render_audit::snapshot();
    assert_eq!(
        after_first.device_lifetime_observations,
        before.device_lifetime_observations + 1
    );
    assert_eq!(
        after_first.device_lost_callbacks,
        before.device_lost_callbacks + 1
    );

    let second = Context3D::new(sibling_gpu).expect("second context admission failed");
    assert_eq!(custom_render_audit::snapshot(), after_first);

    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let first_failure = first
        .device_failure()
        .expect("first context must observe the shared terminal event");
    let second_failure = second
        .device_failure()
        .expect("second context must observe the shared terminal event");
    assert_eq!(first_failure.reason(), GpuDeviceFailureReason::Destroyed);
    assert_eq!(second_failure, first_failure);
}

#[test]
fn viewport_target_bindings_resolve_per_surface_and_serials_order_actual_attempts() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let first_snapshot = snapshot(
        &context,
        1,
        [1.0, 0.25, 0.0],
        RasterMode::SingleSided,
        false,
    );
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let presentation_id = viewport
        .custom_render_admission()
        .unwrap()
        .presentation_id();
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 240.0, 160.0));
    let surface_a = compositor.create_surface(0, RectF::new(0.0, 0.0, 96.0, 64.0), SceneKey::BASE);
    let surface_b =
        compositor.create_surface(1, RectF::new(0.0, 0.0, 144.0, 96.0), SceneKey::new(42));
    let metrics_a = active_metrics_for_surface(surface_a, 96, 64, 1.0);
    let metrics_b = active_metrics_for_surface(surface_b, 144, 96, 1.5);
    let mut surface_scenes = std::collections::HashMap::new();
    surface_scenes.insert(surface_a, target_bound_scene(presentation_id));
    surface_scenes.insert(surface_b, target_bound_scene(presentation_id));
    let mut bindings = CustomRenderPresentationBindings::default();
    let mut builder = DrawPlanBuilder::new();

    let outcome_a = viewport.prepare_custom_render(&metrics_a);
    bindings
        .publish_for_target(
            surface_a,
            presentation_id,
            outcome_a.presentation_binding().unwrap(),
        )
        .unwrap();
    let outcome_b = viewport.prepare_custom_render(&metrics_b);
    bindings
        .publish_for_target(
            surface_b,
            presentation_id,
            outcome_b.presentation_binding().unwrap(),
        )
        .unwrap();
    let first_a_serial = viewport
        .status_for(surface_a)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    let first_b_serial = viewport
        .status_for(surface_b)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    assert!(first_a_serial < first_b_serial);
    let plan = builder.build_multi_with_presentation_bindings(
        &surface_scenes,
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut bindings.begin_materialization_for_test(0),
        |_| 1.0,
        0,
    ).expect("scene must satisfy DrawPlan Build admission");
    assert_eq!(
        draw_plan_image_texture(&plan, surface_a),
        viewport.target_texture_id_for_test(surface_a)
    );
    assert_eq!(
        draw_plan_image_texture(&plan, surface_b),
        viewport.target_texture_id_for_test(surface_b)
    );

    let second_snapshot = snapshot(
        &context,
        2,
        [0.0, 0.25, 1.0],
        RasterMode::SingleSided,
        false,
    );
    viewport
        .set_scene_request(
            Viewport3DViewRevision::new(2),
            view,
            second_snapshot,
            SampleCount3D::ONE,
        )
        .unwrap();
    bindings.begin_frame();
    let outcome_b = viewport.prepare_custom_render(&metrics_b);
    bindings
        .publish_for_target(
            surface_b,
            presentation_id,
            outcome_b.presentation_binding().unwrap(),
        )
        .unwrap();
    let outcome_a = viewport.prepare_custom_render(&metrics_a);
    bindings
        .publish_for_target(
            surface_a,
            presentation_id,
            outcome_a.presentation_binding().unwrap(),
        )
        .unwrap();
    let second_b_serial = viewport
        .status_for(surface_b)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    let second_a_serial = viewport
        .status_for(surface_a)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    assert!(first_b_serial < second_b_serial);
    assert!(second_b_serial < second_a_serial);
    let plan = builder.build_multi_with_presentation_bindings(
        &surface_scenes,
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut bindings.begin_materialization_for_test(1),
        |_| 1.0,
        1,
    ).expect("scene must satisfy DrawPlan Build admission");
    assert_eq!(
        draw_plan_image_texture(&plan, surface_a),
        viewport.target_texture_id_for_test(surface_a)
    );
    assert_eq!(
        draw_plan_image_texture(&plan, surface_b),
        viewport.target_texture_id_for_test(surface_b)
    );

    bindings.begin_frame();
    let mut inactive_metrics_b = metrics_b;
    inactive_metrics_b.activity = CustomRenderActivity::Inactive;
    let inactive_b = viewport.prepare_custom_render(&inactive_metrics_b);
    assert!(inactive_b.presentation_binding().is_none());
    let inactive_b_serial = viewport
        .status_for(surface_b)
        .unwrap()
        .requested()
        .unwrap()
        .request_serial();
    assert!(second_a_serial < inactive_b_serial);
    let stable_a = viewport.prepare_custom_render(&metrics_a);
    bindings
        .publish_for_target(
            surface_a,
            presentation_id,
            stable_a.presentation_binding().unwrap(),
        )
        .unwrap();
    let plan = builder.build_multi_with_presentation_bindings(
        &surface_scenes,
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut bindings.begin_materialization_for_test(2),
        |_| 1.0,
        2,
    ).expect("scene must satisfy DrawPlan Build admission");
    assert_eq!(
        draw_plan_image_texture(&plan, surface_a),
        viewport.target_texture_id_for_test(surface_a)
    );
    assert_eq!(draw_plan_image_texture(&plan, surface_b), None);
}

#[test]
fn failed_newer_request_keeps_compatible_last_good_but_resize_withdraws_it() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_pipeline_variants = 1;
    let Some((_renderer, context, view)) = setup(policy) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(1, 96, 64, 1.0);
    viewport.prepare_custom_render(&metrics);
    let committed_texture = viewport
        .target_texture_id_for_test(SurfaceId::test_id(1))
        .unwrap();

    let failed_snapshot = snapshot(&context, 2, [0.0, 0.0, 1.0], RasterMode::DoubleSided, false);
    viewport
        .set_scene_request(
            Viewport3DViewRevision::new(1),
            view,
            failed_snapshot,
            SampleCount3D::ONE,
        )
        .unwrap();
    let outcome = viewport.prepare_custom_render(&metrics);
    assert!(!outcome.front_publication_changed);
    let status = viewport.status_for(SurfaceId::test_id(1)).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert_eq!(status.requested().unwrap().model_revision().value(), 2);
    assert_eq!(status.committed().unwrap().model_revision().value(), 1);
    assert_eq!(
        status.failure().unwrap().request_serial(),
        status.requested().unwrap().request_serial()
    );
    assert_eq!(
        viewport.target_texture_id_for_test(SurfaceId::test_id(1)),
        Some(committed_texture)
    );

    assert!(
        viewport
            .prepare_custom_render(&active_metrics(1, 120, 80, 1.0))
            .front_publication_changed
    );
    let resized_status = viewport.status_for(SurfaceId::test_id(1)).unwrap();
    assert_eq!(
        resized_status.state(),
        Viewport3DLifecycleState::ReadyWithoutCommit
    );
    assert!(resized_status.committed().is_none());
    assert!(
        viewport
            .target_texture_id_for_test(SurfaceId::test_id(1))
            .is_none()
    );
}

#[test]
fn pending_request_and_publication_exhaustion_preserve_last_good_and_retry_after_release() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_pending_requests = 1;
    policy.max_pending_publications = 1;
    let Some((_renderer, context, view)) = setup(policy) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(1, 96, 64, 1.0);
    viewport.prepare_custom_render(&metrics);

    let second_snapshot = snapshot(&context, 2, [0.0, 1.0, 0.0], RasterMode::SingleSided, false);
    viewport
        .set_scene_request(
            Viewport3DViewRevision::new(1),
            view,
            second_snapshot,
            SampleCount3D::ONE,
        )
        .unwrap();
    let occupied_request = context.reserve_pending_request().unwrap();
    let request_failure = viewport.prepare_custom_render(&metrics);
    assert!(!request_failure.front_publication_changed);
    let status = viewport.status_for(SurfaceId::test_id(1)).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert_eq!(status.committed().unwrap().model_revision().value(), 1);
    assert_eq!(status.requested().unwrap().model_revision().value(), 2);
    assert_eq!(
        status.failure().unwrap().error(),
        &Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::PendingRequests,
            requested: 2,
            maximum: 1,
        })
    );
    drop(occupied_request);
    assert!(
        viewport
            .prepare_custom_render(&metrics)
            .front_publication_changed
    );
    assert_eq!(
        viewport
            .status_for(SurfaceId::test_id(1))
            .unwrap()
            .committed()
            .unwrap()
            .model_revision()
            .value(),
        2
    );

    let third_snapshot = snapshot(&context, 3, [0.0, 0.0, 1.0], RasterMode::SingleSided, false);
    viewport
        .set_scene_request(
            Viewport3DViewRevision::new(1),
            view,
            third_snapshot,
            SampleCount3D::ONE,
        )
        .unwrap();
    let occupied_publication = context.reserve_pending_publication().unwrap();
    let before_publication_failure = viewport_audit::snapshot();
    let publication_failure = viewport.prepare_custom_render(&metrics);
    assert!(!publication_failure.front_publication_changed);
    let status = viewport.status_for(SurfaceId::test_id(1)).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert_eq!(status.committed().unwrap().model_revision().value(), 2);
    assert_eq!(status.requested().unwrap().model_revision().value(), 3);
    assert_eq!(
        status.failure().unwrap().error(),
        &Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::PendingPublications,
            requested: 2,
            maximum: 1,
        })
    );
    let after_publication_failure = viewport_audit::snapshot();
    assert_eq!(
        after_publication_failure.structural_preparations,
        before_publication_failure.structural_preparations
    );
    assert_eq!(
        after_publication_failure.dynamic_writes,
        before_publication_failure.dynamic_writes
    );
    assert_eq!(
        after_publication_failure.gpu_buffers,
        before_publication_failure.gpu_buffers
    );
    assert_eq!(
        after_publication_failure.encodes,
        before_publication_failure.encodes
    );
    drop(occupied_publication);
    assert!(
        viewport
            .prepare_custom_render(&metrics)
            .front_publication_changed
    );
    let status = viewport.status_for(SurfaceId::test_id(1)).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
    assert_eq!(status.committed().unwrap().model_revision().value(), 3);
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.pending_requests, 0);
    assert_eq!(usage.pending_publications, 0);
}

#[test]
fn semantic_revision_identity_is_enforced_and_target_changes_get_new_serials() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let mut viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    assert!(matches!(
        viewport.set_scene_request(
            Viewport3DViewRevision::new(1),
            view_at(0.25),
            first_snapshot.clone(),
            SampleCount3D::ONE,
        ),
        Err(Viewport3DError::ReusedViewRevision { .. })
    ));
    let same_revision_new_identity =
        snapshot(&context, 1, [0.0, 1.0, 0.0], RasterMode::SingleSided, false);
    assert!(matches!(
        viewport.set_scene_request(
            Viewport3DViewRevision::new(1),
            view,
            same_revision_new_identity,
            SampleCount3D::ONE,
        ),
        Err(Viewport3DError::Snapshot(
            crate::three_d::RenderSnapshot3DError::ReusedStructuralRevision { .. }
        ))
    ));

    viewport.prepare_custom_render(&active_metrics(1, 80, 60, 1.0));
    let surface = SurfaceId::test_id(1);
    let first_serial = viewport
        .status_for(surface)
        .unwrap()
        .committed()
        .unwrap()
        .request_serial();
    viewport.prepare_custom_render(&active_metrics(1, 100, 75, 1.0));
    assert!(
        viewport
            .status_for(surface)
            .unwrap()
            .committed()
            .unwrap()
            .request_serial()
            > first_serial
    );

    viewport
        .set_scene_request(
            Viewport3DViewRevision::new(1),
            view,
            first_snapshot,
            SampleCount3D::FOUR,
        )
        .unwrap();
    viewport.prepare_custom_render(&active_metrics(1, 100, 75, 1.0));
    assert_eq!(
        viewport
            .status_for(surface)
            .unwrap()
            .committed()
            .unwrap()
            .sample_count(),
        SampleCount3D::FOUR
    );
    assert!(
        viewport
            .status_for(surface)
            .unwrap()
            .committed()
            .unwrap()
            .request_serial()
            > first_serial
    );
}

#[test]
fn two_viewports_share_context_pipeline_cache_but_not_target_publication_identity() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.5, 0.0], RasterMode::SingleSided, false);
    let mut first = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let mut second = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(1, 64, 64, 1.0);
    first.prepare_custom_render(&metrics);
    second.prepare_custom_render(&metrics);

    assert_ne!(
        first.target_texture_id_for_test(SurfaceId::test_id(1)),
        second.target_texture_id_for_test(SurfaceId::test_id(1))
    );
    assert_ne!(
        first
            .status_for(SurfaceId::test_id(1))
            .unwrap()
            .committed()
            .unwrap()
            .target_key(),
        second
            .status_for(SurfaceId::test_id(1))
            .unwrap()
            .committed()
            .unwrap()
            .target_key()
    );
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 1);
}

#[test]
fn viewport_fronts_compose_through_production_image_alpha_at_sample_one_and_four() {
    let Some((mut renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let opaque = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, true);
    let mut sample_one = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        opaque,
        SampleCount3D::ONE,
    )
    .unwrap();
    sample_one.prepare_custom_render(&active_metrics(1, 64, 64, 1.0));

    let partial = snapshot(&context, 2, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let mut sample_four = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        partial.clone(),
        SampleCount3D::FOUR,
    )
    .unwrap();
    sample_four.prepare_custom_render(&active_metrics(2, 64, 64, 1.0));

    let gpu = renderer.gpu_resources();
    let partial_model = model(&context, &partial, partial.structural_revision().value());
    let direct_sample_four = context
        .render_headless_snapshot_for_test(
            &partial,
            &partial_model,
            view,
            view.pose().translation(),
            [64, 64],
            4,
        )
        .unwrap();
    let direct_pixels = direct_sample_four.readback_rgba8(&gpu).unwrap();
    let (partial_index, partial_pixel) = direct_pixels
        .iter()
        .enumerate()
        .find(|(_, pixel)| pixel[3] > 0 && pixel[3] < 255)
        .unwrap();
    let partial_uv = [
        (partial_index % 64) as f32 / 64.0 + 0.5 / 64.0,
        (partial_index / 64) as f32 / 64.0 + 0.5 / 64.0,
    ];
    renderer.drain_pending_image_registrations();

    let opaque_id = sample_one
        .target_texture_id_for_test(SurfaceId::test_id(1))
        .unwrap();
    let sample_four_id = sample_four
        .target_texture_id_for_test(SurfaceId::test_id(2))
        .unwrap();
    let blue = wgpu::Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    assert_eq!(
        renderer.render_registered_image_pixel_for_test(opaque_id, [0.5, 0.5], [1.0; 4], blue,),
        [255, 0, 0, 255]
    );
    assert_eq!(
        renderer.render_registered_image_pixel_for_test(
            sample_four_id,
            [0.5 / 64.0, 0.5 / 64.0],
            [1.0; 4],
            blue,
        ),
        [0, 0, 255, 255]
    );
    assert_eq!(
        renderer
            .render_registered_image_pixel_for_test(sample_four_id, [0.5, 0.5], [1.0; 4], blue,),
        [255, 0, 0, 255]
    );

    let composed_partial =
        renderer.render_registered_image_pixel_for_test(sample_four_id, partial_uv, [1.0; 4], blue);
    let alpha = f32::from(partial_pixel[3]) / 255.0;
    assert!(composed_partial[0] > 0 && composed_partial[0] < 255);
    assert_eq!(composed_partial[1], 0);
    assert!(composed_partial[2] > 0 && composed_partial[2] < 255);
    assert!((srgb_u8_to_linear(composed_partial[0]) - alpha).abs() < 0.08);
    assert!((srgb_u8_to_linear(composed_partial[2]) - (1.0 - alpha)).abs() < 0.08);
}

fn srgb_u8_to_linear(value: u8) -> f32 {
    let encoded = f32::from(value) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn removal_queues_unregistration_retains_front_back_and_releases_live_view_budget() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_live_views = 1;
    let Some((mut renderer, context, view)) = setup(policy) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let viewport = make_viewport(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        SampleCount3D::ONE,
    )
    .unwrap();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(viewport), None, 0);
    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    harness.frame_with_custom_render_target(
        &mut tree,
        root,
        Constraints::tight(Size::new(64.0, 64.0)),
        &theme,
        &mut text,
        &icons,
        CustomRenderTargetSnapshot::new(SurfaceId::test_id(1), PhysicalExtent::new(64, 64), 1.0)
            .unwrap(),
    );
    renderer.drain_pending_image_registrations();
    tree.remove_node(root);
    assert_eq!(tree.custom_render_participant_count_for_test(), 0);
    let gpu = renderer.gpu_resources();
    let pending = gpu
        .pending_unregistrations
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].retained_textures.len(), 2);
    drop(pending);

    let replacement_snapshot =
        snapshot(&context, 2, [0.0, 1.0, 0.0], RasterMode::SingleSided, false);
    assert!(
        make_viewport(
            context,
            Viewport3DViewRevision::new(2),
            view,
            replacement_snapshot,
            SampleCount3D::ONE,
        )
        .is_ok()
    );
}

#[test]
fn viewport_construction_admits_identity_without_instantiating_target_raster_resources() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.0, 0.0], RasterMode::SingleSided, false);
    let viewport_before = viewport_audit::snapshot();
    let custom_before = custom_render_audit::snapshot();
    let _viewport = make_viewport(
        context,
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        SampleCount3D::FOUR,
    )
    .unwrap();
    let viewport_after = viewport_audit::snapshot();
    let custom_after = custom_render_audit::snapshot();

    assert_eq!(viewport_after.viewports, viewport_before.viewports + 1);
    assert_eq!(viewport_after.targets, viewport_before.targets);
    assert_eq!(viewport_after.depth_targets, viewport_before.depth_targets);
    assert_eq!(
        viewport_after.multisample_targets,
        viewport_before.multisample_targets
    );
    assert_eq!(viewport_after.encodes, viewport_before.encodes);
    assert_eq!(viewport_after.publications, viewport_before.publications);
    assert_eq!(
        custom_after.custom_render_passes,
        custom_before.custom_render_passes
    );
    assert_eq!(custom_after.custom_textures, custom_before.custom_textures);
    assert_eq!(
        custom_after.custom_command_encoders,
        custom_before.custom_command_encoders
    );
    assert_eq!(
        custom_after.custom_submissions,
        custom_before.custom_submissions
    );
}

#[test]
fn ordinary_two_d_work_and_context_construction_do_not_enter_viewport_machinery() {
    let before = viewport_audit::snapshot();
    assert_eq!(before, Default::default());
    let Some((_renderer, _context, _view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let mut after_context = before;
    after_context.contexts += 1;
    assert_eq!(viewport_audit::snapshot(), after_context);

    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(crate::widgets::divider::Divider::horizontal()),
        None,
        0,
    );
    let mut harness = RetainedSceneTestHarness::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    harness.frame(
        &mut tree,
        root,
        Constraints::tight(Size::new(100.0, 1.0)),
        &theme,
        &mut text,
        &icons,
    );
    assert_eq!(viewport_audit::snapshot(), after_context);
}

#[test]
fn viewport_dynamic_model_updates_reuse_warm_structural_resources_for_60_and_120_frames() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [0.2, 0.8, 1.0], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let metrics = active_metrics(1, 96, 72, 1.0);
    viewport.prepare_custom_render(&metrics);

    let warm = viewport_audit::snapshot();
    #[cfg(feature = "render-audit")]
    let cadence_warm = crate::three_d::viewport_cadence_snapshot();
    let texture = viewport.target_texture_id_for_test(surface).unwrap();
    let dynamic_bytes = viewport
        .target_dynamic_storage_bytes_for_test(surface)
        .unwrap();
    assert!(dynamic_bytes <= context.limits().policy.max_dynamic_storage_bytes_per_target);

    let mut after_sixty = None;
    for revision in 2..=121 {
        let next_model = model_at(
            &context,
            &snapshot,
            revision,
            f64::from(revision as u32) * 0.0005,
        );
        viewport
            .set_request(
                Viewport3DViewRevision::new(1),
                view,
                snapshot.clone(),
                next_model,
                SampleCount3D::ONE,
            )
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        if revision == 61 {
            after_sixty = Some(viewport_audit::snapshot());
        }
    }

    let after_sixty = after_sixty.unwrap();
    let after_one_twenty = viewport_audit::snapshot();
    for observed in [after_sixty, after_one_twenty] {
        assert_eq!(observed.resource_generations, warm.resource_generations);
        assert_eq!(observed.gpu_buffers, warm.gpu_buffers);
        assert_eq!(observed.gpu_textures, warm.gpu_textures);
        assert_eq!(observed.gpu_samplers, warm.gpu_samplers);
        assert_eq!(observed.raster_states, warm.raster_states);
        assert_eq!(observed.shader_modules, warm.shader_modules);
        assert_eq!(observed.pipelines, warm.pipelines);
        assert_eq!(
            observed.structural_preparations,
            warm.structural_preparations
        );
        assert_eq!(observed.dynamic_buffers, warm.dynamic_buffers);
        assert_eq!(observed.dynamic_bind_groups, warm.dynamic_bind_groups);
    }
    assert_eq!(after_sixty.dynamic_writes - warm.dynamic_writes, 60);
    assert_eq!(after_one_twenty.dynamic_writes - warm.dynamic_writes, 120);
    assert_eq!(
        after_one_twenty.dynamic_bytes_written - warm.dynamic_bytes_written,
        context.abi_layout().object_block_size() * 120
    );
    #[cfg(feature = "render-audit")]
    {
        let cadence = crate::three_d::viewport_cadence_snapshot();
        assert_eq!(
            cadence.custom_preparations - cadence_warm.custom_preparations,
            120
        );
        assert_eq!(cadence.encodes - cadence_warm.encodes, 120);
        assert_eq!(cadence.submissions - cadence_warm.submissions, 120);
        assert_eq!(cadence.swaps - cadence_warm.swaps, 120);
        assert_eq!(cadence.publications - cadence_warm.publications, 120);
        assert_eq!(
            cadence.unchanged_preparations,
            cadence_warm.unchanged_preparations
        );
        assert_eq!(
            cadence.failed_preparations,
            cadence_warm.failed_preparations
        );
    }
    assert_eq!(viewport.target_texture_id_for_test(surface), Some(texture));
    assert_eq!(
        viewport.target_dynamic_storage_bytes_for_test(surface),
        Some(dynamic_bytes)
    );
    let committed = viewport.status_for(surface).unwrap().committed().unwrap();
    assert_eq!(committed.structural_revision(), StructuralRevision::new(1));
    assert_eq!(committed.model_revision(), ModelRevision::new(121));
}

#[test]
fn viewport_dynamic_updates_write_only_changed_uniform_blocks() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [0.2, 0.8, 1.0], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let metrics = active_metrics(1, 96, 72, 1.0);
    viewport.prepare_custom_render(&metrics);
    let warm = viewport_audit::snapshot();

    let moved_view = view_at(0.1);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            snapshot.clone(),
            initial_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after_view = viewport_audit::snapshot();
    assert_eq!(after_view.dynamic_writes - warm.dynamic_writes, 1);
    assert_eq!(
        after_view.dynamic_bytes_written - warm.dynamic_bytes_written,
        context.abi_layout().view_block_size()
    );

    let moved_model = model_at(&context, &snapshot, 2, 0.1);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            snapshot.clone(),
            moved_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after_model = viewport_audit::snapshot();
    assert_eq!(after_model.dynamic_writes - after_view.dynamic_writes, 1);
    assert_eq!(
        after_model.dynamic_bytes_written - after_view.dynamic_bytes_written,
        context.abi_layout().object_block_size()
    );

    let equal_model = model_at(&context, &snapshot, 3, 0.1);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            snapshot,
            equal_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after_equal_model = viewport_audit::snapshot();
    assert_eq!(after_equal_model.dynamic_writes, after_model.dynamic_writes);
    assert_eq!(
        after_equal_model.dynamic_bytes_written,
        after_model.dynamic_bytes_written
    );
    assert_eq!(after_equal_model.encodes - after_model.encodes, 1);
    assert_eq!(after_equal_model.publications - after_model.publications, 1);
    assert_eq!(
        viewport
            .status_for(surface)
            .unwrap()
            .committed()
            .unwrap()
            .model_revision(),
        ModelRevision::new(3)
    );
}

#[test]
fn viewport_dynamic_failure_preserves_last_good_then_commits_camera_and_model_atomically() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.3, 0.1], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let metrics = active_metrics(1, 96, 72, 1.0);
    viewport.prepare_custom_render(&metrics);
    let texture = viewport.target_texture_id_for_test(surface).unwrap();
    let warm = viewport_audit::snapshot();
    #[cfg(feature = "render-audit")]
    let cadence_warm = crate::three_d::viewport_cadence_snapshot();

    let invalid_model = model_at(&context, &snapshot, 2, 1.0e300);
    viewport
        .set_request(
            Viewport3DViewRevision::new(1),
            view,
            snapshot.clone(),
            invalid_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let failed = viewport.status_for(surface).unwrap();
    assert_eq!(failed.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert_eq!(
        failed.requested().unwrap().model_revision(),
        ModelRevision::new(2)
    );
    assert_eq!(
        failed.committed().unwrap().model_revision(),
        ModelRevision::new(1)
    );
    assert_eq!(viewport.target_texture_id_for_test(surface), Some(texture));
    let after_failure = viewport_audit::snapshot();
    assert_eq!(after_failure.dynamic_writes, warm.dynamic_writes);
    assert_eq!(after_failure.encodes, warm.encodes);
    assert_eq!(
        after_failure.structural_preparations,
        warm.structural_preparations
    );
    assert_eq!(after_failure.gpu_buffers, warm.gpu_buffers);
    #[cfg(feature = "render-audit")]
    {
        let cadence = crate::three_d::viewport_cadence_snapshot();
        assert_eq!(
            cadence.custom_preparations - cadence_warm.custom_preparations,
            1
        );
        assert_eq!(
            cadence.failed_preparations - cadence_warm.failed_preparations,
            1
        );
        assert_eq!(cadence.encodes, cadence_warm.encodes);
        assert_eq!(cadence.publications, cadence_warm.publications);
    }

    let next_view = view_at(0.1);
    let valid_model = model_at(&context, &snapshot, 3, 0.05);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            next_view,
            snapshot,
            valid_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let committed = viewport.status_for(surface).unwrap().committed().unwrap();
    assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(2));
    assert_eq!(committed.view(), next_view);
    assert_eq!(committed.model_revision(), ModelRevision::new(3));
    let recovered = viewport_audit::snapshot();
    assert_eq!(recovered.dynamic_writes - warm.dynamic_writes, 1);
    assert_eq!(
        recovered.structural_preparations,
        warm.structural_preparations
    );
    assert_eq!(recovered.gpu_buffers, warm.gpu_buffers);
}

#[test]
fn viewport_dynamic_semantic_failure_preserves_committed_packed_bytes_atomically() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context.register_geometry(geometry(false)).unwrap();
    let appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new_with_mode(
                constant_source([0.2, 0.7, 1.0]),
                GeometrySemantics::NONE,
                AppearanceMode::Opaque,
            )
            .unwrap(),
        )
        .unwrap();
    let packets = (0..2)
        .map(|ordinal| {
            DrawPacketDescriptor::new(
                geometry.handle().clone(),
                appearance.handle().clone(),
                RasterMode::SingleSided,
                ordinal,
            )
        })
        .collect();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            packets,
        ))
        .unwrap();
    let placement = |x| {
        ModelPlacement3::try_new(
            WorldPoint3::try_new(x, 0.0, -1.0).unwrap(),
            Orientation3::identity(),
            PositiveScale3::one(),
        )
        .unwrap()
    };
    let initial_model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(1),
                vec![placement(-0.25), placement(0.25)],
            ),
        )
        .unwrap();
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let metrics = active_metrics(1, 96, 72, 1.0);
    viewport.prepare_custom_render(&metrics);
    let committed_bytes = viewport.target_dynamic_bytes_for_test(surface).unwrap();
    let warm = viewport_audit::snapshot();

    let invalid_model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(2),
                vec![placement(0.4), placement(1.0e300)],
            ),
        )
        .unwrap();
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            view_at(0.2),
            snapshot,
            invalid_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);

    assert_eq!(
        viewport.target_dynamic_bytes_for_test(surface),
        Some(committed_bytes)
    );
    let after = viewport_audit::snapshot();
    assert_eq!(after.dynamic_writes, warm.dynamic_writes);
    assert_eq!(after.encodes, warm.encodes);
    assert_eq!(after.publications, warm.publications);
    let status = viewport.status_for(surface).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
    assert_eq!(
        status.committed().unwrap().model_revision(),
        ModelRevision::new(1)
    );
}

#[test]
fn viewport_dynamic_updates_are_target_isolated_and_unchanged_targets_do_zero_work() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [0.8, 0.8, 0.2], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let first_surface = SurfaceId::test_id(1);
    let second_surface = SurfaceId::test_id(2);
    let first_metrics = active_metrics(1, 96, 72, 1.0);
    let second_metrics = active_metrics(2, 96, 72, 1.0);
    viewport.prepare_custom_render(&first_metrics);
    viewport.prepare_custom_render(&second_metrics);
    let first_texture = viewport.target_texture_id_for_test(first_surface).unwrap();
    let second_texture = viewport.target_texture_id_for_test(second_surface).unwrap();
    let warm = viewport_audit::snapshot();
    #[cfg(feature = "render-audit")]
    let cadence_warm = crate::three_d::viewport_cadence_snapshot();

    viewport.prepare_custom_render(&first_metrics);
    assert_eq!(viewport_audit::snapshot(), warm);
    #[cfg(feature = "render-audit")]
    {
        let cadence = crate::three_d::viewport_cadence_snapshot();
        assert_eq!(
            cadence.custom_preparations - cadence_warm.custom_preparations,
            1
        );
        assert_eq!(
            cadence.unchanged_preparations - cadence_warm.unchanged_preparations,
            1
        );
        assert_eq!(cadence.encodes, cadence_warm.encodes);
        assert_eq!(cadence.submissions, cadence_warm.submissions);
        assert_eq!(cadence.swaps, cadence_warm.swaps);
        assert_eq!(cadence.publications, cadence_warm.publications);
        assert_eq!(
            cadence.failed_preparations,
            cadence_warm.failed_preparations
        );
    }

    let next_model = model_at(&context, &snapshot, 2, 0.05);
    viewport
        .set_request(
            Viewport3DViewRevision::new(1),
            view,
            snapshot,
            next_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&first_metrics);
    assert_eq!(
        viewport
            .status_for(first_surface)
            .unwrap()
            .committed()
            .unwrap()
            .model_revision(),
        ModelRevision::new(2)
    );
    assert_eq!(
        viewport
            .status_for(second_surface)
            .unwrap()
            .committed()
            .unwrap()
            .model_revision(),
        ModelRevision::new(1)
    );
    viewport.prepare_custom_render(&second_metrics);

    let updated = viewport_audit::snapshot();
    assert_eq!(updated.dynamic_writes - warm.dynamic_writes, 2);
    assert_eq!(
        updated.structural_preparations,
        warm.structural_preparations
    );
    assert_eq!(updated.gpu_buffers, warm.gpu_buffers);
    assert_eq!(updated.dynamic_buffers, warm.dynamic_buffers);
    assert_eq!(updated.dynamic_bind_groups, warm.dynamic_bind_groups);
    assert_eq!(
        viewport.target_texture_id_for_test(first_surface),
        Some(first_texture)
    );
    assert_eq!(
        viewport.target_texture_id_for_test(second_surface),
        Some(second_texture)
    );
}

#[test]
fn viewport_dynamic_timeline_sample_commits_model_state_in_the_same_frame() {
    let Some((_renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [0.3, 0.7, 1.0], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let metrics = active_metrics(1, 96, 72, 1.0);
    viewport.prepare_custom_render(&metrics);
    let warm = viewport_audit::snapshot();

    let mut timeline = Timeline::new();
    timeline.update(0.0).unwrap();
    let handle = timeline
        .start(Animation::new(0.0_f32, 1.0, 1.0, Easing::Linear))
        .unwrap();
    timeline.update(1.0 / 120.0).unwrap();
    let AnimationSample::Active(sample) = timeline.sample::<f32>(handle) else {
        panic!("the admitted frame must expose an active Timeline sample");
    };
    assert!(sample > 0.0);

    let same_frame_model = model_at(&context, &snapshot, 2, f64::from(sample));
    viewport
        .set_request(
            Viewport3DViewRevision::new(1),
            view,
            snapshot,
            same_frame_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);

    let committed = viewport.status_for(surface).unwrap().committed().unwrap();
    assert_eq!(committed.model_revision(), ModelRevision::new(2));
    assert_eq!(
        committed.model().placements()[0].translation().x(),
        f64::from(sample)
    );
    let after = viewport_audit::snapshot();
    assert_eq!(after.dynamic_writes - warm.dynamic_writes, 1);
    assert_eq!(after.structural_preparations, warm.structural_preparations);
    assert_eq!(after.dynamic_buffers, warm.dynamic_buffers);
    assert_eq!(after.dynamic_bind_groups, warm.dynamic_bind_groups);
}

#[test]
fn viewport_dynamic_storage_budget_fails_before_dynamic_resource_creation() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_dynamic_storage_bytes_per_target = 1;
    let Some((_renderer, context, view)) = setup(policy) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.2, 0.2], RasterMode::SingleSided, false);
    let expected_dynamic_bytes = context
        .abi_layout()
        .view_dynamic_stride()
        .checked_add(context.abi_layout().object_dynamic_stride())
        .unwrap();
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context,
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let before = viewport_audit::snapshot();

    let outcome = viewport.prepare_custom_render(&active_metrics(1, 96, 72, 1.0));

    assert!(!outcome.front_publication_changed);
    let status = viewport.status_for(surface).unwrap();
    assert_eq!(status.state(), Viewport3DLifecycleState::ReadyWithoutCommit);
    assert_eq!(
        status.failure().unwrap().error(),
        &Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::DynamicStorageBytesPerTarget,
            requested: expected_dynamic_bytes,
            maximum: 1,
        })
    );
    assert!(status.committed().is_none());
    assert!(viewport.target_texture_id_for_test(surface).is_none());
    let after = viewport_audit::snapshot();
    assert_eq!(
        after.structural_preparations,
        before.structural_preparations
    );
    assert_eq!(after.dynamic_buffers, before.dynamic_buffers);
    assert_eq!(after.dynamic_bind_groups, before.dynamic_bind_groups);
    assert_eq!(after.dynamic_writes, before.dynamic_writes);
}

#[test]
fn viewport_structural_replacement_tracks_dynamic_high_water_and_releases_every_lease() {
    let Some((renderer, context, view)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.2, 0.2], RasterMode::SingleSided, false);
    let first_model = model_at(&context, &first_snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        first_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    assert!(
        viewport
            .prepare_custom_render(&active_metrics(1, 96, 72, 1.0))
            .front_publication_changed
    );

    let warm = context.dynamic_state_accounting_for_test().unwrap();
    assert!(warm.dynamic_storage_bytes > 0);
    assert_eq!(warm.dynamic_object_capacity, 1);
    assert_eq!(warm.dynamic_storage_slots, 1);
    assert!(warm.retained_cpu_bytes > 0);
    assert_eq!(warm.retained_model_states, 1);

    let next_snapshot = snapshot(&context, 2, [0.2, 1.0, 0.2], RasterMode::SingleSided, false);
    let next_model = model_at(&context, &next_snapshot, 2, 0.25);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            view,
            next_snapshot,
            next_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    assert!(
        viewport
            .prepare_custom_render(&active_metrics(1, 96, 72, 1.0))
            .front_publication_changed
    );

    let in_flight = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(
        in_flight.dynamic_storage_bytes,
        warm.dynamic_storage_bytes.checked_mul(2).unwrap()
    );
    assert_eq!(in_flight.dynamic_object_capacity, 2);
    assert_eq!(in_flight.dynamic_storage_slots, 2);
    assert_eq!(
        in_flight.retained_cpu_bytes,
        warm.retained_cpu_bytes.checked_mul(2).unwrap()
    );
    assert_eq!(in_flight.retained_model_states, 1);
    let _ = renderer.gpu_resources().device.poll(wgpu::PollType::wait_indefinitely());

    let replaced = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(replaced.dynamic_storage_bytes, warm.dynamic_storage_bytes);
    assert_eq!(
        replaced.dynamic_storage_bytes_high_water,
        warm.dynamic_storage_bytes.checked_mul(2).unwrap()
    );
    assert_eq!(replaced.dynamic_object_capacity, 1);
    assert_eq!(replaced.dynamic_object_capacity_high_water, 2);
    assert_eq!(replaced.dynamic_storage_slots, 1);
    assert_eq!(replaced.dynamic_storage_slots_high_water, 2);
    assert_eq!(replaced.retained_cpu_bytes, warm.retained_cpu_bytes);
    assert_eq!(
        replaced.retained_cpu_bytes_high_water,
        warm.retained_cpu_bytes.checked_mul(2).unwrap()
    );
    assert_eq!(replaced.retained_model_states, 1);
    assert_eq!(replaced.retained_model_states_high_water, 2);

    drop(viewport);
    let _ = renderer.gpu_resources().device.poll(wgpu::PollType::wait_indefinitely());
    let released = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(released.dynamic_storage_bytes, 0);
    assert_eq!(released.dynamic_object_capacity, 0);
    assert_eq!(released.dynamic_storage_slots, 0);
    assert_eq!(released.retained_cpu_bytes, 0);
    assert_eq!(released.retained_model_states, 0);
    assert_eq!(
        released.dynamic_storage_bytes_high_water,
        replaced.dynamic_storage_bytes_high_water
    );
    assert_eq!(
        released.dynamic_object_capacity_high_water,
        replaced.dynamic_object_capacity_high_water
    );
    assert_eq!(
        released.dynamic_storage_slots_high_water,
        replaced.dynamic_storage_slots_high_water
    );
    assert_eq!(
        released.retained_cpu_bytes_high_water,
        replaced.retained_cpu_bytes_high_water
    );
    assert_eq!(
        released.retained_model_states_high_water,
        replaced.retained_model_states_high_water
    );
}

#[test]
fn viewport_aggregate_object_capacity_failure_preserves_the_prior_surface_and_accounting() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_total_dynamic_object_capacity = 1;
    let Some((renderer, context, view)) = setup(policy) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.2, 0.2], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let first_surface = SurfaceId::test_id(1);
    let second_surface = SurfaceId::test_id(2);
    viewport.prepare_custom_render(&active_metrics(1, 96, 72, 1.0));
    let first_texture = viewport
        .target_texture_id_for_test(first_surface)
        .expect("the first surface must retain its published front");
    let accounting_before = context.dynamic_state_accounting_for_test().unwrap();
    let audit_before = viewport_audit::snapshot();

    let outcome = viewport.prepare_custom_render(&active_metrics(2, 96, 72, 1.0));

    assert!(!outcome.front_publication_changed);
    assert_eq!(
        viewport.target_texture_id_for_test(first_surface),
        Some(first_texture)
    );
    assert!(
        viewport
            .target_texture_id_for_test(second_surface)
            .is_none()
    );
    let second_status = viewport.status_for(second_surface).unwrap();
    assert!(second_status.committed().is_none());
    assert_eq!(
        second_status.failure().unwrap().error(),
        &Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TotalDynamicObjectCapacity,
            requested: 2,
            maximum: 1,
        })
    );
    assert_eq!(
        context.dynamic_state_accounting_for_test().unwrap(),
        accounting_before
    );
    let audit_after = viewport_audit::snapshot();
    assert_eq!(
        audit_after.structural_preparations,
        audit_before.structural_preparations
    );
    assert_eq!(audit_after.dynamic_buffers, audit_before.dynamic_buffers);
    assert_eq!(
        audit_after.dynamic_bind_groups,
        audit_before.dynamic_bind_groups
    );
    assert_eq!(audit_after.dynamic_writes, audit_before.dynamic_writes);

    assert!(
        viewport
            .prepare_custom_render(&inactive_metrics(1, 96, 72))
            .front_publication_changed
    );
    let _ = renderer.gpu_resources().device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        viewport
            .prepare_custom_render(&active_metrics(2, 96, 72, 1.0))
            .front_publication_changed
    );
    assert!(viewport.target_texture_id_for_test(first_surface).is_none());
    assert!(
        viewport
            .target_texture_id_for_test(second_surface)
            .is_some()
    );
    let recovered = viewport.status_for(second_surface).unwrap();
    assert_eq!(recovered.state(), Viewport3DLifecycleState::Committed);
    assert!(recovered.failure().is_none());
}

#[test]
fn viewport_slot_budget_failure_preserves_compatible_last_good_state() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_dynamic_storage_slots = 1;
    let Some((_renderer, context, view)) = setup(policy) else {
        return;
    };
    let first_snapshot = snapshot(&context, 1, [1.0, 0.2, 0.2], RasterMode::SingleSided, false);
    let first_model = model_at(&context, &first_snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        first_snapshot,
        first_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    viewport.prepare_custom_render(&active_metrics(1, 96, 72, 1.0));
    let first_texture = viewport
        .target_texture_id_for_test(surface)
        .expect("the first structural state must publish a front");

    let next_snapshot = snapshot(&context, 2, [0.2, 1.0, 0.2], RasterMode::SingleSided, false);
    let next_model = model_at(&context, &next_snapshot, 2, 0.25);
    viewport
        .set_request(
            Viewport3DViewRevision::new(2),
            view,
            next_snapshot,
            next_model,
            SampleCount3D::ONE,
        )
        .unwrap();
    let accounting_before = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(accounting_before.dynamic_storage_slots, 1);
    assert_eq!(accounting_before.retained_model_states, 2);
    let audit_before = viewport_audit::snapshot();

    let outcome = viewport.prepare_custom_render(&active_metrics(1, 96, 72, 1.0));

    assert!(!outcome.front_publication_changed);
    assert_eq!(
        viewport.target_texture_id_for_test(surface),
        Some(first_texture)
    );
    let status = viewport.status_for(surface).unwrap();
    assert_eq!(
        status.committed().unwrap().model_revision(),
        ModelRevision::new(1)
    );
    assert_eq!(
        status.requested().unwrap().model_revision(),
        ModelRevision::new(2)
    );
    assert_eq!(
        status.failure().unwrap().error(),
        &Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::DynamicStorageSlots,
            requested: 2,
            maximum: 1,
        })
    );
    assert_eq!(
        context.dynamic_state_accounting_for_test().unwrap(),
        accounting_before
    );
    let audit_after = viewport_audit::snapshot();
    assert_eq!(
        audit_after.structural_preparations,
        audit_before.structural_preparations
    );
    assert_eq!(audit_after.dynamic_buffers, audit_before.dynamic_buffers);
    assert_eq!(
        audit_after.dynamic_bind_groups,
        audit_before.dynamic_bind_groups
    );
    assert_eq!(audit_after.dynamic_writes, audit_before.dynamic_writes);
}

fn assert_initial_dynamic_accounting_budget_failure(
    policy: PolicyLimits3D,
    expected_budget: PolicyBudget3D,
) {
    let Some((_renderer, context, view)) = setup(policy) else {
        return;
    };
    let snapshot = snapshot(&context, 1, [1.0, 0.2, 0.2], RasterMode::SingleSided, false);
    let initial_model = model_at(&context, &snapshot, 1, 0.0);
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot,
        initial_model,
        SampleCount3D::ONE,
    )
    .unwrap();
    let surface = SurfaceId::test_id(1);
    let audit_before = viewport_audit::snapshot();

    let outcome = viewport.prepare_custom_render(&active_metrics(1, 96, 72, 1.0));

    assert!(!outcome.front_publication_changed);
    let status = viewport.status_for(surface).unwrap();
    let Viewport3DError::Context(Context3DError::PolicyBudgetExceeded {
        budget,
        requested,
        maximum,
    }) = status.failure().unwrap().error()
    else {
        panic!("expected a typed dynamic-state policy failure");
    };
    assert_eq!(*budget, expected_budget);
    assert!(*requested > *maximum);
    assert_eq!(*maximum, 1);
    assert!(status.committed().is_none());
    assert!(viewport.target_texture_id_for_test(surface).is_none());
    let accounting = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(accounting.dynamic_storage_bytes, 0);
    assert_eq!(accounting.dynamic_object_capacity, 0);
    assert_eq!(accounting.dynamic_storage_slots, 0);
    assert_eq!(accounting.retained_cpu_bytes, 0);
    let audit_after = viewport_audit::snapshot();
    assert_eq!(
        audit_after.structural_preparations,
        audit_before.structural_preparations
    );
    assert_eq!(audit_after.dynamic_buffers, audit_before.dynamic_buffers);
    assert_eq!(
        audit_after.dynamic_bind_groups,
        audit_before.dynamic_bind_groups
    );
    assert_eq!(audit_after.dynamic_writes, audit_before.dynamic_writes);
}

#[test]
fn viewport_aggregate_dynamic_bytes_fail_before_gpu_dynamic_resources() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_total_dynamic_storage_bytes = 1;
    assert_initial_dynamic_accounting_budget_failure(
        policy,
        PolicyBudget3D::TotalDynamicStorageBytes,
    );
}

#[test]
fn viewport_retained_cpu_staging_fails_before_gpu_dynamic_resources() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_dynamic_cpu_bytes = 1;
    assert_initial_dynamic_accounting_budget_failure(
        policy,
        PolicyBudget3D::RetainedDynamicCpuBytes,
    );
}

mod flat_viewport3d_transfer_tests {
    use super::*;
    use std::num::NonZeroUsize;
    use std::sync::Mutex;

    use crate::core::app::{AppContext, AppRoot, Screen, ScreenContext, ScreenFrameContext};
    use crate::core::frame_realization::{AppFrameAttemptId, AppFrameAttempts};
    use crate::core::layout::LayoutResult;
    use crate::core::rect_authority::UpdateAuthorityGuard;
    use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, WidgetDowncast};
    use crate::core::widget_tree::{NodeId, WidgetMountToken};
    use crate::three_d::transfer::FlatViewport3DTransfers;
    use crate::three_d::{
        FlatViewport3DLinkId, FlatViewport3DLinkResult, FlatViewport3DPublicationStatus,
        FlatViewport3DRetirementReason, FlatViewport3DStaleReason, FlatViewport3DTransferError,
        FlatViewport3DTransferOutcome, FlatViewport3DTransferStage, Viewport3DStatus,
    };
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::{label::Label, screen_host::ScreenHost};

    #[test]
    fn pbr_unlinked_material_source_rejects_active_g1_before_mutation() {
        let mut fixture = Fixture::new(1);
        let candidate = fixture.candidate(1, 1, 0.0);
        fixture.plan(candidate);
        fixture.active_frame(0.0);
        let geometry = consumer_pbr_geometry(&fixture.context);
        let source = consumer_pbr_source(&fixture.context, &geometry, 1, true, 1);
        let before = fixture.status();
        let serial = before.requested().unwrap().request_serial();
        let key = before.committed().unwrap().target_key();
        let usage = fixture.context.resource_usage().unwrap();
        let audit = custom_render_audit::snapshot();
        let viewport = fixture
            .tree
            .node_mut(fixture.target)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                viewport.request_pbr_scene(
                    source.clone(),
                    Viewport3DViewRevision::new(1),
                    view_at(0.0)
                ),
                Err(crate::three_d::PbrSourceError3D::Association(_))
            ));
        }
        let after = fixture.status();
        assert_eq!(after.state(), before.state());
        assert_eq!(after.requested().unwrap().request_serial(), serial);
        assert_eq!(after.committed().unwrap().target_key(), key);
        assert!(
            after
                .requested()
                .unwrap()
                .snapshot()
                .same_identity(before.requested().unwrap().snapshot())
        );
        assert!(after.pbr_requested().is_none() && after.pbr_committed().is_none());
        assert_eq!(fixture.context.resource_usage().unwrap(), usage);
        assert_eq!(custom_render_audit::snapshot(), audit);
    }

    #[derive(Clone)]
    struct Candidate {
        correlation: u64,
        revision: Viewport3DViewRevision,
        view: ViewDescriptor3,
        snapshot: RenderSnapshot,
        model: ModelState,
        derive_view_from_dt: bool,
        duplicate: bool,
    }

    struct Producer {
        id: WidgetInternalId,
        link: Option<FlatViewport3DLinkId>,
        candidate: Option<Candidate>,
        dt: Option<f32>,
        published_view: Option<ViewDescriptor3>,
        admission: Option<Result<FlatViewport3DPublicationStatus, FlatViewport3DTransferError>>,
        duplicate_admission:
            Option<Result<FlatViewport3DPublicationStatus, FlatViewport3DTransferError>>,
    }

    impl Producer {
        fn new() -> Self {
            Self {
                id: WidgetInternalId::new(),
                link: None,
                candidate: None,
                dt: None,
                published_view: None,
                admission: None,
                duplicate_admission: None,
            }
        }
    }

    impl WidgetDowncast for Producer {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for Producer {
        fn id(&self) -> WidgetInternalId {
            self.id
        }
        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
        }
        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
        fn hit_testable(&self) -> bool {
            false
        }
        fn needs_update(&self) -> bool {
            self.candidate.is_some()
        }
        fn update(&mut self, dt: f32, cx: &mut UpdateContext<'_>) {
            let Some(mut candidate) = self.candidate.take() else {
                return;
            };
            self.dt = Some(dt);
            if candidate.derive_view_from_dt {
                candidate.view = view_at(f64::from(dt));
            }
            self.published_view = Some(candidate.view);
            let link = self
                .link
                .as_ref()
                .expect("application supplies only this source's link");
            self.admission = Some(cx.publish_flat_viewport3d_request(
                link,
                candidate.correlation,
                candidate.revision,
                candidate.view,
                candidate.snapshot.clone(),
                candidate.model.clone(),
                SampleCount3D::ONE,
            ));
            if candidate.duplicate {
                self.duplicate_admission = Some(cx.publish_flat_viewport3d_request(
                    link,
                    candidate.correlation + 1,
                    candidate.revision,
                    candidate.view,
                    candidate.snapshot,
                    candidate.model,
                    SampleCount3D::ONE,
                ));
            }
        }
    }

    struct Fixture {
        renderer: WgpuRenderer,
        context: Arc<Context3D>,
        snapshot: RenderSnapshot,
        model: ModelState,
        tree: WidgetTree,
        root: NodeId,
        source: NodeId,
        target: NodeId,
        app: AppContext,
        link: FlatViewport3DLinkId,
        harness: RetainedSceneTestHarness,
        theme: Theme,
        icons: IconRegistry,
        attempts: AppFrameAttempts,
    }

    impl Fixture {
        fn new(capacity: usize) -> Self {
            let (renderer, context, _) = setup(PolicyLimits3D::conservative())
                .expect("G1 owner proof requires an actual headless GPU adapter and 3D context");
            let snapshot = snapshot(&context, 1, [0.3, 0.7, 1.0], RasterMode::SingleSided, false);
            let model = model_at(&context, &snapshot, 1, 0.0);
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
            let source = tree.add_child(root, Box::new(Producer::new()), None, 0);
            let target = tree.add_child(
                root,
                Box::new(
                    Viewport3D::new(
                        context.clone(),
                        Viewport3DViewRevision::new(1),
                        view_at(0.0),
                        snapshot.clone(),
                        model.clone(),
                        SampleCount3D::ONE,
                    )
                    .unwrap(),
                ),
                None,
                0,
            );
            let theme = Theme::default();
            let mut app = AppContext::new_with_flat_viewport3d_transfer_capacity(
                theme.clone(),
                NonZeroUsize::new(capacity).unwrap(),
            )
            .unwrap();
            let source_id = tree.node(source).unwrap().id();
            let target_id = tree.node(target).unwrap().id();
            let link = app
                .admit_flat_viewport3d_link(&mut tree, source_id, target_id)
                .unwrap();
            tree.node_mut(source)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Producer>()
                .unwrap()
                .link = Some(link.clone());
            Self {
                renderer,
                context,
                snapshot,
                model,
                tree,
                root,
                source,
                target,
                app,
                link,
                harness: RetainedSceneTestHarness::new(),
                theme,
                icons: IconRegistry::new(&[]),
                attempts: AppFrameAttempts::new(),
            }
        }

        fn candidate(&self, correlation: u64, revision: u64, x: f64) -> Candidate {
            Candidate {
                correlation,
                revision: Viewport3DViewRevision::new(revision),
                view: view_at(x),
                snapshot: self.snapshot.clone(),
                model: self.model.clone(),
                derive_view_from_dt: false,
                duplicate: false,
            }
        }

        fn producer(&self) -> &Producer {
            self.tree
                .node(self.source)
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref::<Producer>()
                .unwrap()
        }

        fn plan(&mut self, candidate: Candidate) {
            let producer = self
                .tree
                .node_mut(self.source)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Producer>()
                .unwrap();
            producer.candidate = Some(candidate);
            producer.admission = None;
            producer.duplicate_admission = None;
        }

        fn frame(&mut self, dt: f32, size: Size) -> AppFrameAttemptId {
            let attempt = self.attempts.next();
            let target = CustomRenderTargetSnapshot::new(
                SurfaceId::test_id(1),
                PhysicalExtent::new(96, 72),
                1.0,
            )
            .unwrap()
            .with_app_frame_attempt(attempt);
            self.harness.frame_with_flat_viewport3d_transfers(
                &mut self.tree,
                &mut self.app,
                self.root,
                Constraints::tight(size),
                &self.theme,
                &mut self.renderer,
                &self.icons,
                dt,
                Some(target),
                Some(attempt),
            );
            attempt
        }

        fn active_frame(&mut self, dt: f32) -> AppFrameAttemptId {
            self.frame(dt, Size::new(96.0, 72.0))
        }

        fn receipt(&mut self) -> FlatViewport3DLinkResult {
            self.app
                .flat_viewport3d_link_result(&self.link)
                .unwrap()
                .expect("publication must remain observable under its exact link")
        }

        fn take(&mut self) -> FlatViewport3DLinkResult {
            self.app
                .take_flat_viewport3d_link_result(&self.link)
                .unwrap()
                .expect("terminal receipt must be explicitly consumed")
        }

        fn status(&self) -> Viewport3DStatus {
            tree_viewport(&self.tree, self.target)
                .status_for(SurfaceId::test_id(1))
                .expect("actual frame preparation establishes this target")
                .clone()
        }

        // Supplementary mechanical lifetime tests enter the owning registry directly;
        // the canonical timed/preparation tests above and below use AppContext.
        fn mechanical_owner(&mut self) -> (FlatViewport3DTransfers, FlatViewport3DLinkId) {
            self.app.retire_flat_viewport3d_link(&self.link).unwrap();
            let mut owner = FlatViewport3DTransfers::new(NonZeroUsize::new(1).unwrap()).unwrap();
            let source = self.tree.node(self.source).unwrap().id();
            let target = self.tree.node(self.target).unwrap().id();
            let link = owner.admit(&mut self.tree, source, target).unwrap();
            (owner, link)
        }

        fn publish_mechanical(
            &self,
            owner: &mut FlatViewport3DTransfers,
            link: &FlatViewport3DLinkId,
            correlation: u64,
        ) -> WidgetMountToken {
            let source = self.tree.concrete_mount(link.source()).unwrap();
            let _update = UpdateAuthorityGuard::enter();
            assert_eq!(
                owner.publish(
                    link.source(),
                    Some(&source),
                    link,
                    correlation,
                    Viewport3DViewRevision::new(2),
                    view_at(0.25),
                    self.snapshot.clone(),
                    self.model.clone(),
                    SampleCount3D::ONE,
                ),
                Ok(FlatViewport3DPublicationStatus::AcceptedForCutoff)
            );
            source
        }
    }

    fn assert_terminal(result: &FlatViewport3DLinkResult, outcome: FlatViewport3DTransferOutcome) {
        assert_eq!(result.stage, FlatViewport3DTransferStage::Settled(outcome));
    }

    #[test]
    fn flat_viewport3d_transfer_timed_owned_publication_reaches_same_frame_preparation() {
        let mut f = Fixture::new(1);
        f.active_frame(0.01);
        let before = viewport_audit::snapshot();
        let mut candidate = f.candidate(201, 2, 0.0);
        candidate.derive_view_from_dt = true;
        f.plan(candidate);
        let dt = 1.0 / 60.0;
        let attempt = f.active_frame(dt);
        assert_eq!(f.producer().dt, Some(dt));
        assert_eq!(
            f.producer().admission,
            Some(Ok(FlatViewport3DPublicationStatus::AcceptedForCutoff))
        );
        let receipt = f.receipt();
        assert_terminal(&receipt, FlatViewport3DTransferOutcome::Transferred);
        assert_eq!(receipt.link, f.link);
        assert_eq!(
            (receipt.source, receipt.target),
            (f.link.source(), f.link.target())
        );
        assert_eq!(receipt.correlation, 201);
        assert_eq!(receipt.cutoff_frame, Some(attempt));
        let prepared = tree_viewport(&f.tree, f.target)
            .current_prepared_target()
            .unwrap();
        assert_eq!(prepared.surface_id(), SurfaceId::test_id(1));
        assert_eq!(prepared.metrics().unwrap().app_frame_attempt, Some(attempt));
        let status = prepared.status().clone();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        let requested = status.requested().unwrap();
        let committed = status.committed().unwrap();
        assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(2));
        assert_eq!(committed.view(), f.producer().published_view.unwrap());
        assert_eq!(committed.view(), view_at(f64::from(dt)));
        assert_eq!(committed.request_serial(), requested.request_serial());
        assert!(committed.snapshot().same_identity(&f.snapshot));
        assert!(committed.model().same_identity(&f.model));
        let after = viewport_audit::snapshot();
        assert_eq!(after.encodes, before.encodes + 1);
        assert_eq!(after.publications, before.publications + 1);
        assert_eq!(after.dynamic_writes, before.dynamic_writes + 1);
        assert_eq!(
            after.structural_preparations,
            before.structural_preparations
        );
        assert_eq!(after.gpu_buffers, before.gpu_buffers);
        assert_eq!(after.pipelines, before.pipelines);

        // A direct equal request is still a second writer, not an allowed no-op.
        let target = f
            .tree
            .node_mut(f.target)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        assert_eq!(
            target.set_request(
                committed.view_revision(),
                committed.view(),
                committed.snapshot().clone(),
                committed.model().clone(),
                committed.sample_count(),
            ),
            Err(Viewport3DError::LinkedWriterExclusive)
        );
        assert_eq!(viewport_audit::snapshot(), after);
        assert_eq!(f.receipt(), receipt);
    }

    #[test]
    fn flat_viewport3d_transfer_unchanged_is_consumable_without_gpu_work() {
        let mut f = Fixture::new(1);
        f.active_frame(0.01);
        let serial = f.status().committed().unwrap().request_serial();
        let before = viewport_audit::snapshot();
        f.plan(f.candidate(301, 1, 0.0));
        let attempt = f.active_frame(0.02);
        let receipt = f.receipt();
        assert_terminal(&receipt, FlatViewport3DTransferOutcome::Unchanged);
        assert_eq!(receipt.cutoff_frame, Some(attempt));
        assert_eq!(f.status().committed().unwrap().request_serial(), serial);
        assert_eq!(viewport_audit::snapshot(), before);
        assert_eq!(f.take(), receipt);
        assert!(
            f.app
                .flat_viewport3d_link_result(&f.link)
                .unwrap()
                .is_none()
        );
        f.plan(f.candidate(302, 2, 0.2));
        f.active_frame(0.02);
        assert_terminal(&f.take(), FlatViewport3DTransferOutcome::Transferred);
    }

    #[test]
    fn flat_viewport3d_transfer_duplicate_and_unconsumed_candidate_do_not_overwrite_receipt() {
        let mut f = Fixture::new(1);
        f.active_frame(0.01);
        let mut candidate = f.candidate(401, 2, 0.2);
        candidate.duplicate = true;
        f.plan(candidate);
        f.active_frame(0.02);
        assert_eq!(
            f.producer().duplicate_admission,
            Some(Err(FlatViewport3DTransferError::CandidatePending))
        );
        let first = f.receipt();
        let before = viewport_audit::snapshot();
        f.plan(f.candidate(403, 3, 0.3));
        f.active_frame(0.03);
        assert_eq!(
            f.producer().admission,
            Some(Err(FlatViewport3DTransferError::ResultUnconsumed))
        );
        assert_eq!(f.receipt(), first);
        assert_eq!(viewport_audit::snapshot(), before);
        assert_eq!(
            f.status().committed().unwrap().view_revision(),
            Viewport3DViewRevision::new(2)
        );
    }

    #[test]
    fn flat_viewport3d_transfer_wrong_updating_mount_and_outside_update_are_refused() {
        let mut f = Fixture::new(1);
        let mut foreign = Producer::new();
        foreign.link = Some(f.link.clone());
        foreign.candidate = Some(f.candidate(501, 2, 0.2));
        let foreign_node = f.tree.add_child(f.root, Box::new(foreign), None, 0);
        f.active_frame(0.02);
        let foreign = f
            .tree
            .node(foreign_node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<Producer>()
            .unwrap();
        assert_eq!(
            foreign.admission,
            Some(Err(FlatViewport3DTransferError::WrongSource))
        );
        assert!(
            f.app
                .flat_viewport3d_link_result(&f.link)
                .unwrap()
                .is_none()
        );
        let mut outside = UpdateContext::new(None);
        assert_eq!(
            outside.publish_flat_viewport3d_request(
                &f.link,
                502,
                Viewport3DViewRevision::new(2),
                view_at(0.2),
                f.snapshot.clone(),
                f.model.clone(),
                SampleCount3D::ONE,
            ),
            Err(FlatViewport3DTransferError::NotEngineUpdate)
        );
    }

    #[test]
    fn flat_viewport3d_transfer_admission_is_exclusive_bounded_and_explicitly_reusable() {
        let mut f = Fixture::new(1);
        let other_source = f.tree.add_child(f.root, Box::new(Producer::new()), None, 0);
        let source = f.tree.node(other_source).unwrap().id();
        assert_eq!(
            f.app
                .admit_flat_viewport3d_link(&mut f.tree, source, f.link.target()),
            Err(FlatViewport3DTransferError::WriterConflict)
        );
        let other_target = f.tree.add_child(
            f.root,
            Box::new(
                Viewport3D::new(
                    f.context.clone(),
                    Viewport3DViewRevision::new(1),
                    view_at(0.0),
                    f.snapshot.clone(),
                    f.model.clone(),
                    SampleCount3D::ONE,
                )
                .unwrap(),
            ),
            None,
            0,
        );
        let target = f.tree.node(other_target).unwrap().id();
        assert_eq!(
            f.app
                .admit_flat_viewport3d_link(&mut f.tree, source, target),
            Err(FlatViewport3DTransferError::CapacityExhausted)
        );
        let mut foreign_app = AppContext::new_with_flat_viewport3d_transfer_capacity(
            f.theme.clone(),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            foreign_app.flat_viewport3d_link_result(&f.link),
            Err(FlatViewport3DTransferError::ForeignApplication)
        );
        f.app.retire_flat_viewport3d_link(&f.link).unwrap();
        let wrong_target = f.tree.node(f.root).unwrap().id();
        assert_eq!(
            f.app
                .admit_flat_viewport3d_link(&mut f.tree, source, wrong_target),
            Err(FlatViewport3DTransferError::WrongTarget)
        );
        let new_link = f
            .app
            .admit_flat_viewport3d_link(&mut f.tree, source, target)
            .unwrap();
        assert_ne!(new_link, f.link);
        assert_eq!(
            f.app.flat_viewport3d_link_result(&f.link),
            Err(FlatViewport3DTransferError::Retired)
        );
        let old_target = f
            .tree
            .node_mut(f.target)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        assert!(
            old_target
                .set_request(
                    Viewport3DViewRevision::new(2),
                    view_at(0.2),
                    f.snapshot.clone(),
                    f.model.clone(),
                    SampleCount3D::ONE,
                )
                .is_ok()
        );
    }

    fn replacement_settles_original(source_replaced: bool) {
        let mut f = Fixture::new(1);
        let (mut owner, link) = f.mechanical_owner();
        let original_source_mount = f.publish_mechanical(&mut owner, &link, 601);
        let node = if source_replaced { f.source } else { f.target };
        let old_id = f.tree.node(node).unwrap().id();
        let old_mount = f.tree.concrete_mount(old_id).unwrap();
        let replacement: Box<dyn Widget> = if source_replaced {
            Box::new(Producer::new())
        } else {
            Box::new(
                Viewport3D::new(
                    f.context.clone(),
                    Viewport3DViewRevision::new(1),
                    view_at(0.0),
                    f.snapshot.clone(),
                    f.model.clone(),
                    SampleCount3D::ONE,
                )
                .unwrap(),
            )
        };
        f.tree.replace_widget_only(node, replacement);
        assert_eq!(f.tree.node(node).unwrap().id(), old_id);
        assert_ne!(f.tree.concrete_mount(old_id).unwrap(), old_mount);
        assert!(!old_mount.is_live());
        if source_replaced {
            assert!(!original_source_mount.is_live());
        }
        let result = owner.result(&link).unwrap().unwrap();
        assert_eq!(result.correlation, 601);
        assert_eq!(result.link, link);
        assert_terminal(
            &result,
            FlatViewport3DTransferOutcome::Stale(if source_replaced {
                FlatViewport3DStaleReason::SourceMountEnded
            } else {
                FlatViewport3DStaleReason::TargetMountEnded
            }),
        );
        let _update = UpdateAuthorityGuard::enter();
        owner.cutoff(&mut f.tree, None);
        drop(_update);
        assert_eq!(owner.result(&link).unwrap(), Some(result.clone()));
        let successor_id = f.tree.node(node).unwrap().id();
        let (source, target) = if source_replaced {
            (successor_id, link.target())
        } else {
            (link.source(), successor_id)
        };
        assert_eq!(
            owner.admit(&mut f.tree, source, target),
            Err(FlatViewport3DTransferError::CapacityExhausted)
        );
        assert_eq!(owner.take_result(&link).unwrap(), Some(result));
        let successor = owner.admit(&mut f.tree, source, target).unwrap();
        assert_ne!(successor, link);
    }

    #[test]
    fn flat_viewport3d_transfer_source_same_tree_id_replacement_retires_concrete_lineage() {
        replacement_settles_original(true);
    }

    #[test]
    fn flat_viewport3d_transfer_target_same_type_tree_id_replacement_retires_concrete_lineage() {
        replacement_settles_original(false);
    }

    #[test]
    fn flat_viewport3d_transfer_recycled_node_and_tree_teardown_never_retarget_old_receipt() {
        let mut f = Fixture::new(1);
        let (mut owner, link) = f.mechanical_owner();
        f.publish_mechanical(&mut owner, &link, 701);
        f.tree.remove_node(f.target);
        let successor = f.tree.add_child(
            f.root,
            Box::new(
                Viewport3D::new(
                    f.context.clone(),
                    Viewport3DViewRevision::new(1),
                    view_at(0.0),
                    f.snapshot.clone(),
                    f.model.clone(),
                    SampleCount3D::ONE,
                )
                .unwrap(),
            ),
            None,
            0,
        );
        assert_eq!(
            successor, f.target,
            "this proves actual recycled arena-slot reuse"
        );
        let successor_id = f.tree.node(successor).unwrap().id();
        assert_ne!(successor_id, link.target());
        let result = owner.result(&link).unwrap().unwrap();
        assert_terminal(
            &result,
            FlatViewport3DTransferOutcome::Stale(FlatViewport3DStaleReason::TargetMountEnded),
        );
        assert_eq!(result.target, link.target());
        assert_eq!(
            owner.admit(&mut f.tree, link.source(), successor_id),
            Err(FlatViewport3DTransferError::CapacityExhausted)
        );
        f.tree.remove_node(f.root);
        assert!(f.tree.find_by_id(link.source()).is_none());
        assert!(f.tree.find_by_id(link.target()).is_none());
        drop(f.tree);
        assert_eq!(owner.result(&link).unwrap(), Some(result.clone()));
        assert_eq!(owner.take_result(&link).unwrap(), Some(result));
        assert!(!owner.has_active());
    }

    #[test]
    fn flat_viewport3d_transfer_pending_result_is_not_consumed_and_explicit_retirement_is_bounded()
    {
        let mut f = Fixture::new(1);
        let (mut owner, link) = f.mechanical_owner();
        f.publish_mechanical(&mut owner, &link, 801);
        let pending = owner.result(&link).unwrap().unwrap();
        assert_eq!(
            pending.stage,
            FlatViewport3DTransferStage::AcceptedForCutoff
        );
        assert_eq!(pending.cutoff_frame, None);
        assert_eq!(owner.take_result(&link).unwrap(), None);
        owner.retire(&link).unwrap();
        let terminal = owner.result(&link).unwrap().unwrap();
        assert_terminal(
            &terminal,
            FlatViewport3DTransferOutcome::Retired(FlatViewport3DRetirementReason::Explicit),
        );
        assert_eq!(terminal.correlation, pending.correlation);
        assert_eq!(
            owner.admit(&mut f.tree, link.source(), link.target()),
            Err(FlatViewport3DTransferError::CapacityExhausted)
        );
        assert_eq!(owner.take_result(&link).unwrap(), Some(terminal));
        let successor = owner
            .admit(&mut f.tree, link.source(), link.target())
            .unwrap();
        assert_ne!(successor, link);
    }

    #[test]
    fn flat_viewport3d_transfer_shutdown_settles_pending_and_preserves_existing_terminal() {
        let mut f = Fixture::new(1);
        let (mut owner, link) = f.mechanical_owner();
        f.publish_mechanical(&mut owner, &link, 901);
        owner.shutdown();
        let terminal = owner.result(&link).unwrap().unwrap();
        assert_terminal(
            &terminal,
            FlatViewport3DTransferOutcome::Retired(
                FlatViewport3DRetirementReason::ApplicationShutdown,
            ),
        );
        assert_eq!(terminal.correlation, 901);
        owner.shutdown();
        assert_eq!(owner.result(&link).unwrap(), Some(terminal.clone()));
        assert_eq!(
            owner.admit(&mut f.tree, link.source(), link.target()),
            Err(FlatViewport3DTransferError::ApplicationShutdown)
        );
        let mount = f.tree.concrete_mount(link.source()).unwrap();
        assert_eq!(
            owner.publish(
                link.source(),
                Some(&mount),
                &link,
                902,
                Viewport3DViewRevision::new(3),
                view_at(0.3),
                f.snapshot.clone(),
                f.model.clone(),
                SampleCount3D::ONE,
            ),
            Err(FlatViewport3DTransferError::ApplicationShutdown)
        );
        assert_eq!(owner.take_result(&link).unwrap(), Some(terminal));

        let mut g = Fixture::new(1);
        let (mut other, other_link) = g.mechanical_owner();
        g.publish_mechanical(&mut other, &other_link, 903);
        let _update = UpdateAuthorityGuard::enter();
        other.cutoff(&mut g.tree, None);
        drop(_update);
        let transferred = other.result(&other_link).unwrap().unwrap();
        assert_terminal(&transferred, FlatViewport3DTransferOutcome::Transferred);
        other.shutdown();
        assert_eq!(other.result(&other_link).unwrap(), Some(transferred));
    }

    #[test]
    fn flat_viewport3d_transfer_known_setter_error_is_rejected_without_false_commit() {
        let mut f = Fixture::new(1);
        f.active_frame(0.01);
        let old = f.status().committed().unwrap().clone();
        let before = viewport_audit::snapshot();
        f.plan(f.candidate(1001, 1, 0.5));
        f.active_frame(0.02);
        assert_eq!(
            f.producer().admission,
            Some(Ok(FlatViewport3DPublicationStatus::AcceptedForCutoff))
        );
        assert_terminal(
            &f.receipt(),
            FlatViewport3DTransferOutcome::Rejected(Viewport3DError::ReusedViewRevision {
                revision: Viewport3DViewRevision::new(1),
            }),
        );
        let status = f.status();
        assert_eq!(
            status.committed().unwrap().request_serial(),
            old.request_serial()
        );
        assert_eq!(status.committed().unwrap().view(), old.view());
        assert_eq!(viewport_audit::snapshot(), before);
    }

    #[test]
    fn flat_viewport3d_transfer_inactive_target_and_last_good_are_distinct_from_transfer() {
        let mut f = Fixture::new(1);
        let before = viewport_audit::snapshot();
        f.plan(f.candidate(1101, 2, 0.2));
        f.frame(0.02, Size::new(0.0, 0.0));
        assert_terminal(&f.receipt(), FlatViewport3DTransferOutcome::Transferred);
        let status = f.status();
        assert_eq!(status.state(), Viewport3DLifecycleState::Inactive);
        assert!(status.committed().is_none());
        assert_eq!(viewport_audit::snapshot().encodes, before.encodes);
        assert_eq!(viewport_audit::snapshot().publications, before.publications);

        let mut g = Fixture::new(1);
        g.active_frame(0.01);
        let original = g.status().committed().unwrap().clone();
        let texture = tree_viewport(&g.tree, g.target)
            .target_texture_id_for_test(SurfaceId::test_id(1))
            .unwrap();
        let mut candidate = g.candidate(1102, 2, 0.2);
        candidate.model = model_at(&g.context, &g.snapshot, 2, 1.0e300);
        let before = viewport_audit::snapshot();
        g.plan(candidate);
        g.active_frame(0.02);
        assert_terminal(&g.receipt(), FlatViewport3DTransferOutcome::Transferred);
        let status = g.status();
        assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
        assert_eq!(
            status.requested().unwrap().view_revision(),
            Viewport3DViewRevision::new(2)
        );
        let old = status.committed().unwrap();
        assert_eq!(old.request_serial(), original.request_serial());
        assert_eq!(old.view_revision(), original.view_revision());
        assert!(old.model().same_identity(original.model()));
        assert!(status.failure().is_some());
        assert_eq!(
            tree_viewport(&g.tree, g.target).target_texture_id_for_test(SurfaceId::test_id(1)),
            Some(texture)
        );
        let after = viewport_audit::snapshot();
        assert_eq!(after.encodes, before.encodes);
        assert_eq!(after.publications, before.publications);
        assert_eq!(after.dynamic_writes, before.dynamic_writes);
    }

    struct MountedScreen {
        stub: ScreenHost,
        context: Arc<Context3D>,
        snapshot: RenderSnapshot,
        model: ModelState,
        binding: Arc<Mutex<Option<(NodeId, NodeId, FlatViewport3DLinkId)>>>,
        dropped: Arc<AtomicBool>,
    }

    impl Drop for MountedScreen {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    impl Screen for MountedScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            let root = cx.mount_child(Box::new(ScreenHost::new()));
            let mut producer = Producer::new();
            producer.candidate = Some(Candidate {
                correlation: 1201,
                revision: Viewport3DViewRevision::new(2),
                view: view_at(0.25),
                snapshot: self.snapshot.clone(),
                model: self.model.clone(),
                derive_view_from_dt: false,
                duplicate: false,
            });
            let source = cx.tree.add_child(root, Box::new(producer), None, 0);
            let target = cx.tree.add_child(
                root,
                Box::new(
                    Viewport3D::new(
                        self.context.clone(),
                        Viewport3DViewRevision::new(1),
                        view_at(0.0),
                        self.snapshot.clone(),
                        self.model.clone(),
                        SampleCount3D::ONE,
                    )
                    .unwrap(),
                ),
                None,
                0,
            );
            let source_id = cx.tree.node(source).unwrap().id();
            let target_id = cx.tree.node(target).unwrap().id();
            let link = cx
                .app
                .admit_flat_viewport3d_link(cx.tree, source_id, target_id)
                .unwrap();
            cx.tree
                .node_mut(source)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Producer>()
                .unwrap()
                .link = Some(link.clone());
            *self.binding.lock().unwrap() = Some((source, target, link));
            &mut self.stub
        }
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }
    }

    struct SuccessorScreen {
        stub: ScreenHost,
        link: FlatViewport3DLinkId,
        observed: Arc<Mutex<Option<FlatViewport3DLinkResult>>>,
    }

    impl Screen for SuccessorScreen {
        fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
            cx.mount_child(Box::new(ScreenHost::new()));
            &mut self.stub
        }
        fn take_rebuild_flag(&mut self) -> bool {
            false
        }
        fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
            *self.observed.lock().unwrap() =
                cx.app.flat_viewport3d_link_result(&self.link).unwrap();
        }
    }

    #[test]
    fn flat_viewport3d_transfer_app_receipt_survives_origin_screen_and_both_mounts() {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative())
            .expect("G1 Screen lifetime proof requires an actual GPU adapter");
        let snapshot = snapshot(&context, 1, [0.4, 0.6, 0.8], RasterMode::SingleSided, false);
        let model = model_at(&context, &snapshot, 1, 0.0);
        let theme = Theme::default();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let app = AppContext::new_with_flat_viewport3d_transfer_capacity(
            theme.clone(),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let mut app_root = AppRoot::new(app, root);
        let binding = Arc::new(Mutex::new(None));
        let dropped = Arc::new(AtomicBool::new(false));
        app_root.push_screen(Box::new(MountedScreen {
            stub: ScreenHost::new(),
            context,
            snapshot,
            model,
            binding: binding.clone(),
            dropped: dropped.clone(),
        }));
        app_root.rebuild_active_view(&mut tree);
        let (_, _, link) = binding.lock().unwrap().clone().unwrap();
        let mut harness = RetainedSceneTestHarness::new();
        let icons = IconRegistry::new(&[]);
        let mut attempts = AppFrameAttempts::new();
        let attempt = attempts.next();
        let target = CustomRenderTargetSnapshot::new(
            SurfaceId::test_id(1),
            PhysicalExtent::new(96, 72),
            1.0,
        )
        .unwrap()
        .with_app_frame_attempt(attempt);
        harness.frame_with_flat_viewport3d_transfers(
            &mut tree,
            &mut app_root.context,
            root,
            Constraints::tight(Size::new(96.0, 72.0)),
            &theme,
            &mut renderer,
            &icons,
            0.02,
            Some(target),
            Some(attempt),
        );
        let receipt = app_root
            .context
            .flat_viewport3d_link_result(&link)
            .unwrap()
            .unwrap();
        assert_terminal(&receipt, FlatViewport3DTransferOutcome::Transferred);
        let observed = Arc::new(Mutex::new(None));
        drop(app_root.replace_screen(Box::new(SuccessorScreen {
            stub: ScreenHost::new(),
            link: link.clone(),
            observed: observed.clone(),
        })));
        assert!(dropped.load(Ordering::SeqCst));
        app_root.rebuild_active_view(&mut tree);
        assert!(tree.find_by_id(link.source()).is_none());
        assert!(tree.find_by_id(link.target()).is_none());
        app_root.flush_pending_actions_all(&mut tree);
        assert_eq!(*observed.lock().unwrap(), Some(receipt.clone()));
        assert_eq!(
            app_root
                .context
                .take_flat_viewport3d_link_result(&link)
                .unwrap(),
            Some(receipt)
        );
    }

    #[test]
    fn material_writer_linked_refusal_precedes_source_validation_and_mutation() {
        let mut f = Fixture::new(1);
        f.active_frame(0.01);
        let original = f.status().committed().unwrap().clone();
        let mut material = PbrMaterialSource3D::defaults(91003, 1);
        material.emissive_rgb = [1.0; 3];
        let source = r024_source(
            &f.context,
            super::super::three_d_geometry::r024_vertices(),
            material,
            1,
            false,
            r024_identity(),
            2,
        );
        let invalid = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                exposure_ev: 17.0,
                ..source.hdr.clone()
            },
            samples: 1,
            direct: None,
            indirect: None,
            observe: false,
        });
        let before = viewport_audit::snapshot();
        let custom_before = custom_render_audit::snapshot();
        for source in [source.clone(), source, invalid] {
            let viewport = f
                .tree
                .node_mut(f.target)
                .unwrap()
                .widget_mut()
                .as_any_mut()
                .downcast_mut::<Viewport3D>()
                .unwrap();
            assert_eq!(
                viewport.set_material_source(source, original.view_revision(), original.view()),
                Err(crate::three_d::pbr::PbrError3D::Association(
                    "private material source requires an unlinked ordinary flat viewport",
                ))
            );
        }
        assert_eq!(
            f.status().committed().unwrap().request_serial(),
            original.request_serial()
        );
        assert!(f.status().requested_material().is_none());
        assert_eq!(viewport_audit::snapshot(), before);
        assert_eq!(custom_render_audit::snapshot(), custom_before);
        f.plan(f.candidate(91003, 2, 0.1));
        f.active_frame(0.02);
        assert_terminal(&f.receipt(), FlatViewport3DTransferOutcome::Transferred);
        assert_eq!(
            f.status().committed().unwrap().view_revision(),
            Viewport3DViewRevision::new(2)
        );
    }

    #[test]
    fn flat_viewport3d_transfer_disabled_and_zero_link_two_d_frames_have_no_three_d_work() {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("zero-link canonical-frame proof requires a headless adapter");
        let theme = Theme::default();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
        let panel = tree.add_child(root, Box::new(Panel::new(root)), None, 0);
        let label = tree.add_child(panel, Box::new(Label::new("ordinary 2D")), None, 0);
        tree.node_mut(panel)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Panel>()
            .unwrap()
            .child = label;
        let source = tree.node(label).unwrap().id();
        let target = tree.node(panel).unwrap().id();
        let mut disabled = AppContext::new(theme.clone());
        assert_eq!(
            disabled.admit_flat_viewport3d_link(&mut tree, source, target),
            Err(FlatViewport3DTransferError::Disabled)
        );
        let mut enabled = AppContext::new_with_flat_viewport3d_transfer_capacity(
            theme.clone(),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let before = viewport_audit::snapshot();
        let custom_before = custom_render_audit::snapshot();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        let mut attempts = AppFrameAttempts::new();
        for app in [&mut disabled, &mut enabled] {
            let attempt = attempts.next();
            harness.frame_with_flat_viewport3d_transfers(
                &mut tree,
                app,
                root,
                Constraints::tight(Size::new(96.0, 72.0)),
                &theme,
                &mut renderer,
                &icons,
                0.02,
                None,
                Some(attempt),
            );
            assert_eq!(
                viewport_audit::snapshot(),
                before,
                "disabled and configured-but-unlinked 2D work must allocate no Context3D/resources"
            );
            assert_eq!(custom_render_audit::snapshot(), custom_before);
        }
        assert_eq!(tree.custom_render_participant_count_for_test(), 0);
    }
}
