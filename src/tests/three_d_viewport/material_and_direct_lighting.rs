use super::*;

pub(super) fn r024_identity() -> ModelPlacement3 {
    ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap()
}

pub(in crate::tests) fn r025_view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        crate::three_d::OrthographicProjection::try_new(2.0, 0.1, 10.0)
            .unwrap()
            .into(),
    )
}

#[test]
fn pbr_direct_warm_fov_coarse_receipt_n1_n4() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    assert_eq!(
        renderer.headless_adapter_for_test().backend,
        wgpu::Backend::Metal
    );
    let gpu = renderer.gpu_resources();
    let view = |degrees: f32| {
        ViewDescriptor3::new(
            r025_view().pose(),
            PerspectiveProjection::try_finite(degrees.to_radians(), 0.1, 10.0)
                .unwrap()
                .into(),
        )
    };
    for (n, mask) in [1, 4]
        .into_iter()
        .flat_map(|n| [false, true].map(move |mask| (n, mask)))
    {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let source = if mask {
            use super::three_d_context::r028_fixture as producer;
            use crate::three_d::mip_sampling::{MipMin3D, MipRole3D, SamplingFamily3D};
            let base = r028_role(
                &context,
                producer::produce(MipRole3D::MaskBase, 2, 2, vec![255; 16], 28910, 1, true),
                28920,
                MipMin3D::LinearMipLinear,
                crate::three_d::SamplerFilterMode::Linear,
                crate::three_d::SamplerAddressMode::ClampToEdge,
                crate::three_d::SamplerAddressMode::ClampToEdge,
            );
            let mut material = PbrMaterialSource3D::defaults(28900, 1);
            material.sampling = SamplingFamily3D::V2;
            material.alpha_mode = PbrAlphaMode3D::Mask;
            material.roles[0] = Some(base.0);
            material.metallic = 0.0;
            material.roughness = 1.0;
            let mut scene = r024_source(
                &context,
                r028_quad([1.0; 2], [0.0; 2]),
                material,
                n,
                false,
                r024_identity(),
                1,
            );
            let s = Arc::get_mut(&mut scene).unwrap();
            s.hdr.maximum_rgb = [64.0; 3];
            s.direct = Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
                identity: 28930,
                revision: 1,
                meters_per_scene_unit: 1.0,
                directional: Some(crate::three_d::direct_shadow::Directional3D {
                    surface_to_source: [0.0, 0.0, 1.0],
                    irradiance: [1.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
                snapshot: s.snapshot.clone(),
                model: s.model.clone(),
            }));
            scene
        } else {
            r028_presence_scene(&context, n, 1)
        };
        let metrics = active_metrics(82400 + u64::from(n) * 2 + u64::from(mask), 32, 32, 1.0);
        let surface = metrics.surface_id;
        use crate::three_d::direct_shadow::DirectSource3D;
        let validations = || {
            let (direct, mr_scans) = DirectSource3D::full_validation_counts_for_test();
            (
                MaterialSceneSource3D::full_validation_count_for_test(),
                direct,
                mr_scans,
            )
        };
        let initial_counts = validations();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            crate::three_d::PbrScene3D(source.clone()),
            Viewport3DViewRevision::new(1),
            view(56.0),
        )
        .unwrap();
        let admitted = validations();
        assert_eq!(
            admitted,
            (
                initial_counts.0 + 1,
                initial_counts.1 + 1,
                initial_counts.2 + usize::from(!mask)
            )
        );
        assert_eq!(
            std::mem::size_of::<crate::three_d::pbr::ValidatedMaterialSceneSource3D>(),
            std::mem::size_of::<Arc<MaterialSceneSource3D>>()
        );
        assert!(viewport.pbr_timing_for_test().is_none());
        assert!(viewport.pbr_coarse_timing_for_test().is_none());
        viewport.set_pbr_coarse_timing_for_test(true);
        viewport.set_pbr_timing_for_test(false);
        assert!(viewport.pbr_coarse_timing_for_test().is_some());
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(
            validations(),
            admitted,
            "cold producer repeated source admission"
        );
        assert!(!viewport.pbr_coarse_timing_for_test().unwrap().complete);
        viewport.set_pbr_timing_for_test(true);
        viewport.set_pbr_coarse_timing_for_test(false);
        assert!(viewport.pbr_timing_for_test().is_some());
        assert!(viewport.pbr_coarse_timing_for_test().is_none());
        viewport.set_pbr_coarse_timing_for_test(true);
        assert!(viewport.pbr_timing_for_test().is_none());
        assert!(viewport.pbr_coarse_timing_for_test().is_some());
        viewport.set_pbr_timing_for_test(false);
        assert!(viewport.pbr_coarse_timing_for_test().is_some());
        viewport.set_pbr_coarse_timing_for_test(false);

        let mut revision = 1;
        // Test-only completion observation warms two real retained slots. It is
        // outside every measured preparation and is not runtime maintenance.
        for fov in [52.0, 48.0, 44.0, 48.0] {
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            revision += 1;
            viewport
                .set_material_source(
                    source.clone(),
                    Viewport3DViewRevision::new(revision),
                    view(fov),
                )
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
        }
        assert_eq!(viewport.capacity_counts_for_test(surface).0, 2);
        let mut baseline = None;
        let mut detailed = None;
        for mode in ["OFF", "DETAILED", "COARSE"] {
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            match mode {
                "DETAILED" => viewport.set_pbr_timing_for_test(true),
                "COARSE" => viewport.set_pbr_coarse_timing_for_test(true),
                _ => {}
            }
            revision += 1;
            let requested = Viewport3DViewRevision::new(revision);
            viewport
                .set_material_source(source.clone(), requested, view(40.0))
                .unwrap();
            std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("PBR_COARSE_CONTROL N={n} mask={mask} mode={mode} BEGIN\n"),
            )
            .unwrap();
            r024_prepare(&mut viewport, &metrics);
            std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("PBR_COARSE_CONTROL N={n} mask={mask} mode={mode} END\n"),
            )
            .unwrap();
            let tuple = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap();
            assert_eq!(tuple.view_revision(), requested);
            assert!(Arc::ptr_eq(tuple.source(), &source));
            match mode {
                "OFF" => {
                    assert!(viewport.pbr_timing_for_test().is_none());
                    assert!(viewport.pbr_coarse_timing_for_test().is_none());
                }
                "DETAILED" => {
                    let t = viewport.pbr_timing_for_test().unwrap();
                    assert!(t.complete && t.raster.capacity_reused);
                    assert_no_runtime_gpu_model(&t.raster);
                    assert!(t.raster.direct_projection_partition_complete);
                    assert!(viewport.pbr_coarse_timing_for_test().is_none());
                    detailed = Some(t.raster);
                }
                "COARSE" => {
                    let t = viewport.pbr_coarse_timing_for_test().unwrap();
                    assert!(viewport.pbr_timing_for_test().is_none());
                    assert!(t.complete && t.capacity_reused);
                    assert_eq!(t.target_key, Some(tuple.target_key()));
                    assert_eq!(t.view_revision, Some(requested));
                    assert_eq!(t.request_serial, Some(tuple.request_serial()));
                    assert_eq!(
                        t.source_identity,
                        Some((source.hdr.identity, source.hdr.revision))
                    );
                    assert_eq!(t.structural_revision, Some(tuple.structural_revision()));
                    assert_eq!(t.model_revision, Some(tuple.model_revision()));
                    let d = source.direct.as_ref().unwrap();
                    assert_eq!(t.direct_identity, Some((d.identity, d.revision)));
                    let d = detailed.unwrap();
                    assert_eq!(
                        (
                            t.main_uv_packets,
                            t.main_uv_qualified,
                            t.main_uv_excluded,
                            t.main_uv_conditioned,
                            t.shadow_uv_triangles,
                            t.shadow_faces
                        ),
                        (
                            d.main_uv_packets,
                            d.main_uv_qualified,
                            d.main_uv_excluded,
                            d.main_uv_conditioned,
                            d.shadow_triangles,
                            d.shadow_faces
                        )
                    );
                    assert_eq!(t.shadow_faces, 3);
                    assert_eq!(t.shadow_uv_triangles, 0);
                    assert_eq!(
                        (t.packet_vertex_projection, t.uv_main, t.uv_shadow),
                        (
                            std::time::Duration::ZERO,
                            std::time::Duration::ZERO,
                            std::time::Duration::ZERO
                        )
                    );
                    let raster_phases =
                        t.packet_vertex_projection + t.uv_main + t.direct_projection + t.uv_shadow;
                    assert_eq!(
                        raster_phases + t.raster_unattributed.unwrap(),
                        t.raster_total
                    );
                    assert!(
                        t.render_total
                            >= t.prepare_validation
                                + t.raster_total
                                + t.hdr_total
                                + t.material_commit
                    );
                    assert!(t.active_total >= t.render_total + t.publication);
                    assert!(t.preparation_total >= t.active_total);
                    assert!(t.request_total >= t.request_validation);
                    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                        "PBR_COARSE_RECEIPT N={n} mask={mask} serial={} revision={} vertex_ns={} uv_main_ns={} direct_ns={} shadow_uv_ns={} raster_unattributed_ns={} raster_total_ns={} validation_ns={} hdr_ns={} commit_ns={} render_total_ns={} publication_ns={} active_total_ns={} preparation_total_ns={} request_ns={} request_validation_ns={} packets={} qualified={} excluded={} conditioned={} shadow_triangles={} faces={}\n",
                        tuple.request_serial().value(), revision, t.packet_vertex_projection.as_nanos(), t.uv_main.as_nanos(), t.direct_projection.as_nanos(), t.uv_shadow.as_nanos(), t.raster_unattributed.unwrap().as_nanos(), t.raster_total.as_nanos(), t.prepare_validation.as_nanos(), t.hdr_total.as_nanos(), t.material_commit.as_nanos(), t.render_total.as_nanos(), t.publication.as_nanos(), t.active_total.as_nanos(), t.preparation_total.as_nanos(), t.request_total.as_nanos(), t.request_validation.as_nanos(), t.main_uv_packets, t.main_uv_qualified, t.main_uv_excluded, t.main_uv_conditioned, t.shadow_uv_triangles, t.shadow_faces)).unwrap();
                }
                _ => unreachable!(),
            }
            // Observation is after the receipt, outside its CPU intervals.
            let proof = viewport.direct_proof_for_test(surface).unwrap();
            let pixel =
                read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu).map(f32::to_bits);
            renderer.drain_pending_image_registrations();
            let image = renderer.render_registered_image_pixel_for_test(
                viewport.target_texture_id_for_test(surface).unwrap(),
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            if let Some((packed, witnesses, fp16, registered)) = &baseline {
                assert_eq!(&proof.packed, packed);
                assert_eq!(&proof.witnesses, witnesses);
                assert_eq!(&pixel, fp16);
                assert_eq!(&image, registered);
            } else {
                baseline = Some((proof.packed, proof.witnesses, pixel, image));
            }
            let audit = custom_render_audit::snapshot();
            r024_prepare(&mut viewport, &metrics);
            assert_eq!(audit, custom_render_audit::snapshot());
            if mode == "COARSE" {
                let t = viewport.pbr_coarse_timing_for_test().unwrap();
                assert!(!t.complete && t.request_serial.is_none());
                assert_eq!(
                    (t.raster_total, t.uv_main, t.hdr_total),
                    (
                        std::time::Duration::ZERO,
                        std::time::Duration::ZERO,
                        std::time::Duration::ZERO
                    )
                );
            }
            viewport.set_pbr_timing_for_test(false);
            viewport.set_pbr_coarse_timing_for_test(false);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            revision += 1;
            viewport
                .set_material_source(
                    source.clone(),
                    Viewport3DViewRevision::new(revision),
                    view(44.0),
                )
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
        }
        assert_eq!(
            validations(),
            admitted,
            "same-source FOV repeated source admission"
        );
        let original_basis = viewport.direct_proof_for_test(surface).unwrap();
        let successor = |hdr| {
            Arc::new(MaterialSceneSource3D {
                snapshot: source.snapshot.clone(),
                model: source.model.clone(),
                samples: n,
                direct: source.direct.clone(),
                indirect: None,
                observe: false,
                hdr,
            })
        };
        let mut hdr = source.hdr.clone();
        hdr.revision += 1;
        let mut invalid_hdr = hdr.clone();
        invalid_hdr.maximum_rgb[1] = 0.0;
        let invalid_source = successor(invalid_hdr);
        let expected = invalid_source.validate(&context).unwrap_err();
        let before = validations();
        let old_tuple = viewport
            .status_for(surface)
            .unwrap()
            .committed_material()
            .unwrap()
            .clone();
        let old_front = viewport.target_texture_id_for_test(surface);
        assert_eq!(
            viewport
                .set_material_source(
                    invalid_source,
                    Viewport3DViewRevision::new(revision + 1),
                    view(56.0)
                )
                .unwrap_err(),
            expected
        );
        assert_eq!(
            validations(),
            (before.0 + 1, before.1 + 1, before.2 + usize::from(!mask))
        );
        assert_eq!(viewport.target_texture_id_for_test(surface), old_front);
        let audit = custom_render_audit::snapshot();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(audit, custom_render_audit::snapshot());
        assert_eq!(
            viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial(),
            old_tuple.request_serial()
        );
        let next = successor(hdr);
        assert!(!Arc::ptr_eq(&source, &next));
        assert!(Arc::ptr_eq(&source.snapshot, &next.snapshot));
        assert!(Arc::ptr_eq(&source.model, &next.model));
        assert!(Arc::ptr_eq(
            source.direct.as_ref().unwrap(),
            next.direct.as_ref().unwrap()
        ));
        let before = validations();
        revision += 1;
        viewport
            .set_material_source(
                next.clone(),
                Viewport3DViewRevision::new(revision),
                view(56.0),
            )
            .unwrap();
        let next_admitted = validations();
        assert_eq!(
            next_admitted,
            (before.0 + 1, before.1 + 1, before.2 + usize::from(!mask))
        );
        viewport.set_pbr_timing_for_test(true);
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(validations(), next_admitted);
        assert!(
            original_basis.same_source_basis(&viewport.direct_proof_for_test(surface).unwrap())
        );
        let mut counts = next_admitted;
        for fov in [52.0, 48.0, 44.0, 48.0] {
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            revision += 1;
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source(
                    next.clone(),
                    Viewport3DViewRevision::new(revision),
                    view(fov),
                )
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            assert_eq!(
                validations(),
                counts,
                "HDR successor repeated a full source/MR scan"
            );
            let tuple = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap();
            assert!(Arc::ptr_eq(tuple.source(), &next));
            assert_eq!(tuple.source().hdr.revision, 2);
            assert_eq!(tuple.view_revision(), Viewport3DViewRevision::new(revision));
            let receipt = viewport.pbr_timing_for_test().unwrap();
            assert!(receipt.complete && receipt.raster.direct_source_validation_reused);
            assert_eq!(receipt.raster.shadow_faces, 3);
            assert_no_runtime_gpu_model(&receipt.raster);
            if receipt.raster.capacity_reused {
                assert_eq!(receipt.raster.gpu_creations, [0; 5]);
            }
            let current = viewport.direct_proof_for_test(surface).unwrap();
            assert!(original_basis.same_source_basis(&current));
            assert_eq!(current.faces, [0, 1, 2]);
            let actual =
                read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu).map(f32::to_bits);
            renderer.drain_pending_image_registrations();
            let actual_image = renderer.render_registered_image_pixel_for_test(
                viewport.target_texture_id_for_test(surface).unwrap(),
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            let mut reference = r024_viewport(context.clone(), next.clone(), view(fov));
            r024_prepare(&mut reference, &metrics);
            let expected = reference.direct_proof_for_test(surface).unwrap();
            assert_eq!(current.packed, expected.packed);
            assert_eq!(current.witnesses, expected.witnesses);
            assert_eq!(
                actual,
                read_r027_fp16(reference.hdr_front_for_test(surface), &gpu).map(f32::to_bits)
            );
            renderer.drain_pending_image_registrations();
            assert_eq!(
                actual_image,
                renderer.render_registered_image_pixel_for_test(
                    reference.target_texture_id_for_test(surface).unwrap(),
                    [0.5; 2],
                    [1.0; 4],
                    wgpu::Color::WHITE
                )
            );
            // A separate owner has a separate first source admission; it is
            // outside the source-generation count for this viewport.
            assert_eq!(
                validations(),
                (counts.0 + 1, counts.1 + 1, counts.2 + usize::from(!mask))
            );
            counts = validations();
        }
        let source = next;
        let invalid = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            samples: n,
            direct: source.direct.clone(),
            indirect: None,
            observe: false,
            hdr: crate::three_d::hdr::HdrSource3D {
                exposure_ev: f32::NAN,
                ..source.hdr.clone()
            },
        });
        let old = viewport
            .status_for(surface)
            .unwrap()
            .committed_material()
            .unwrap()
            .request_serial();
        let audit = custom_render_audit::snapshot();
        let off_error = viewport
            .set_material_source(
                invalid.clone(),
                Viewport3DViewRevision::new(revision + 1),
                view(40.0),
            )
            .unwrap_err();
        viewport.set_pbr_coarse_timing_for_test(true);
        assert_eq!(
            viewport
                .set_material_source(
                    invalid,
                    Viewport3DViewRevision::new(revision + 1),
                    view(40.0)
                )
                .unwrap_err(),
            off_error
        );
        assert!(!viewport.pbr_coarse_timing_for_test().unwrap().complete);
        assert!(viewport.pbr_timing_for_test().is_none());
        assert_eq!(
            old,
            viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial()
        );
        assert_eq!(audit, custom_render_audit::snapshot());
    }
}

#[test]
fn pbr_direct_warm_fov_retained_capacity_encoder_bound_n1_n4() {
    fn depth_layers(
        gpu: &crate::renderer::wgpu_renderer::GpuResources,
        texture: &wgpu::Texture,
    ) -> Vec<u8> {
        let bytes = 1024u64 * 1024 * 3 * 4;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("current three directional depth faces"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4096),
                    rows_per_image: Some(1024),
                },
            },
            wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 3,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let result = readback.slice(..).get_mapped_range().expect("mapped readback range").to_vec();
        readback.unmap();
        result
    }
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    assert_eq!(
        renderer.headless_adapter_for_test().backend,
        wgpu::Backend::Metal
    );
    assert_eq!(renderer.headless_adapter_for_test().name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    let view = |degrees: f32| {
        ViewDescriptor3::new(
            r025_view().pose(),
            PerspectiveProjection::try_finite(degrees.to_radians(), 0.1, 10.0)
                .unwrap()
                .into(),
        )
    };
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let source = r025_source(&context, n, 0, 1.0);
        let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (32, 32), 1.0)
            .unwrap();
        let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
        let mut viewport = r024_viewport(context.clone(), source.clone(), view(56.0));
        r024_prepare(&mut viewport, &metrics);
        let initial = viewport.capacity_objects_for_test(surface).unwrap();
        let mut objects = vec![initial];
        let mut serial = viewport
            .status_for(surface)
            .unwrap()
            .committed_material()
            .unwrap()
            .request_serial();
        // No readback or blocking poll occurs between these overlapping requests.
        for (i, fov) in [52.0, 48.0, 44.0, 48.0].into_iter().enumerate() {
            viewport.set_pbr_timing_for_test(true);
            let revision = Viewport3DViewRevision::new(2 + i as u64);
            viewport
                .set_material_source(source.clone(), revision, view(fov))
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let committed = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap();
            assert_eq!(committed.view_revision(), revision);
            assert!(committed.request_serial() > serial);
            serial = committed.request_serial();
            let current = viewport.capacity_objects_for_test(surface).unwrap();
            assert_ne!(
                current.buffers,
                objects.last().unwrap().buffers,
                "current generation was overwritten"
            );
            assert_ne!(current.textures, objects.last().unwrap().textures);
            let t = viewport.pbr_timing_for_test().unwrap();
            assert!(t.complete);
            assert_eq!(t.raster.shadow_faces, 3);
            assert!(
                current.staging.is_some(),
                "selected path did not own a staging slot"
            );
            if t.raster.capacity_reused {
                assert_eq!(t.raster.gpu_creations, [0; 5]);
            }
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "PBR_CAPACITY_FIXTURE N{n} fov={fov} reused={} creates={:?} slots={} ready={} faces={}\n",
                t.raster.capacity_reused,
                t.raster.gpu_creations,
                t.raster.capacity_slots,
                t.raster.capacity_ready_revision,
                t.raster.shadow_faces
            )).unwrap();
            objects.push(current);
        }
        assert!(viewport.capacity_counts_for_test(surface).0 >= 2);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(source.clone(), Viewport3DViewRevision::new(6), view(40.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let reused = viewport.capacity_objects_for_test(surface).unwrap();
        let previous = objects
            .iter()
            .find(|p| p.buffers == reused.buffers)
            .expect("existing capacity handle not reused");
        assert_eq!(previous.bindings, reused.bindings);
        assert_eq!(previous.textures, reused.textures);
        assert_eq!(previous.views, reused.views);
        assert_eq!(previous.sampler, reused.sampler);
        assert_eq!(
            previous.cpu, reused.cpu,
            "immutable packet/role packing was rebuilt"
        );
        if previous.staging.is_some() {
            assert_eq!(previous.staging, reused.staging);
        }
        assert!(
            viewport
                .pbr_timing_for_test()
                .unwrap()
                .raster
                .capacity_reused
        );
        assert_eq!(
            viewport.pbr_timing_for_test().unwrap().raster.gpu_creations,
            [0; 5]
        );
        let proof = viewport.direct_proof_for_test(surface).unwrap();
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        let mut fresh = r024_viewport(context.clone(), source.clone(), view(40.0));
        let other = compositor.create_surface(1, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(other, (32, 32), 1.0)
            .unwrap();
        let other_metrics = active_metrics_for_surface(other, 32, 32, 1.0);
        r024_prepare(&mut fresh, &other_metrics);
        let fresh_proof = fresh.direct_proof_for_test(other).unwrap();
        assert_eq!(
            proof.packed, fresh_proof.packed,
            "Light608/Shadow1984 changed"
        );
        assert_eq!(
            proof.witnesses, fresh_proof.witnesses,
            "candidate/depth witnesses changed"
        );
        assert_eq!(
            depth_layers(&gpu, &reused.textures[0]),
            depth_layers(
                &gpu,
                &fresh.capacity_objects_for_test(other).unwrap().textures[0]
            ),
            "all current shadow texels differ from fresh complete rendering"
        );
        assert_eq!(
            pixel.map(f32::to_bits),
            read_r027_fp16(fresh.hdr_front_for_test(other), &gpu).map(f32::to_bits)
        );
        let fresh_objects = fresh.capacity_objects_for_test(other).unwrap();
        assert_ne!(fresh_objects.buffers, reused.buffers);
        assert_ne!(fresh_objects.textures, reused.textures);
        renderer.drain_pending_image_registrations();
        let image = |v: &Viewport3D, surface, renderer: &WgpuRenderer| {
            renderer.render_registered_image_pixel_for_test(
                v.target_texture_id_for_test(surface).unwrap(),
                [16.5 / 32.0; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            )
        };
        assert_eq!(
            image(&viewport, surface, &renderer),
            image(&fresh, other, &renderer)
        );
        let stable = custom_render_audit::snapshot();
        let usage = context.resource_usage().unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(stable, custom_render_audit::snapshot());
        assert_eq!(usage, context.resource_usage().unwrap());
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(source.clone(), Viewport3DViewRevision::new(7), view(38.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let second_reuse = viewport.capacity_objects_for_test(surface).unwrap();
        let original = objects
            .iter()
            .find(|p| p.staging.is_some() && p.buffers == second_reuse.buffers)
            .expect("a previously submitted staging slot was not rearmed");
        assert_eq!(
            original, &second_reuse,
            "real staging/buffer/binding/map identity changed"
        );
        assert_eq!(
            viewport.pbr_timing_for_test().unwrap().raster.gpu_creations,
            [0; 5]
        );
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        drop(second_reuse);
        // An invalid current view is refused before touching the retained front.
        let old = viewport.target_texture_id_for_test(surface);
        let bad = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(1.0e300, 0.0, 2.0).unwrap(),
                Orientation3::identity(),
            ),
            view(40.0).projection(),
        );
        viewport
            .set_material_source(source.clone(), Viewport3DViewRevision::new(8), bad)
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        assert!(viewport.material_failure_for_test(surface).is_some());
        assert_eq!(viewport.target_texture_id_for_test(surface), old);
        assert_eq!(
            viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .view_revision(),
            Viewport3DViewRevision::new(7)
        );
        assert_eq!(
            pixel.map(f32::to_bits),
            read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu).map(f32::to_bits)
        );
        viewport
            .set_material_source(source.clone(), Viewport3DViewRevision::new(9), view(42.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let reused_revision = Arc::new(MaterialSceneSource3D {
            hdr: crate::three_d::hdr::HdrSource3D {
                exposure_ev: -0.0,
                ..source.hdr.clone()
            },
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            direct: source.direct.clone(),
            samples: n,
            indirect: None,
            observe: false,
        });
        let old = viewport.target_texture_id_for_test(surface);
        assert!(
            viewport
                .set_material_source(reused_revision, Viewport3DViewRevision::new(10), view(44.0))
                .is_err()
        );
        assert_eq!(viewport.target_texture_id_for_test(surface), old);
        let changed_hdr = Arc::new(MaterialSceneSource3D {
            hdr: crate::three_d::hdr::HdrSource3D {
                revision: 2,
                exposure_ev: -0.0,
                ..source.hdr.clone()
            },
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            direct: source.direct.clone(),
            samples: n,
            indirect: None,
            observe: false,
        });
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(
                changed_hdr.clone(),
                Viewport3DViewRevision::new(10),
                view(44.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert!(
            !viewport
                .pbr_timing_for_test()
                .unwrap()
                .raster
                .capacity_reused
        );
        assert_eq!(
            viewport.capacity_counts_for_test(surface),
            (0, 0, 0),
            "HDR bits retained the old capacity family"
        );
        assert!(
            viewport
                .capacity_objects_for_test(surface)
                .unwrap()
                .staging
                .is_none()
        );
        let bad_hdr = Arc::new(MaterialSceneSource3D {
            hdr: crate::three_d::hdr::HdrSource3D {
                maximum_rgb: [0.0; 3],
                ..changed_hdr.hdr.clone()
            },
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            direct: source.direct.clone(),
            samples: n,
            indirect: None,
            observe: false,
        });
        let old = viewport.target_texture_id_for_test(surface);
        assert!(
            viewport
                .set_material_source(bad_hdr, Viewport3DViewRevision::new(11), view(48.0))
                .is_err()
        );
        assert_eq!(viewport.target_texture_id_for_test(surface), old);
        viewport
            .set_material_source(
                changed_hdr.clone(),
                Viewport3DViewRevision::new(11),
                view(46.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert!(viewport.capacity_counts_for_test(surface).0 >= 2);
        let replacement = r024_source(
            &context,
            super::three_d_geometry::r024_vertices(),
            PbrMaterialSource3D::defaults(1, 2),
            n,
            false,
            r024_identity(),
            2,
        );
        let snapshot = replacement.snapshot.clone();
        let model = replacement.model.clone();
        let direct = source.direct.as_ref().unwrap();
        let next_source = Arc::new(MaterialSceneSource3D {
            snapshot: snapshot.clone(),
            model: model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                revision: 3,
                ..source.hdr.clone()
            },
            samples: n,
            observe: false,
            indirect: None,
            direct: Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
                identity: direct.identity,
                revision: 2,
                meters_per_scene_unit: direct.meters_per_scene_unit,
                directional: direct.directional.clone(),
                points: direct.points.clone(),
                spots: direct.spots.clone(),
                snapshot,
                model,
            })),
        });
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(
                next_source.clone(),
                Viewport3DViewRevision::new(12),
                view(48.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(viewport.capacity_counts_for_test(surface), (0, 0, 0));
        assert!(
            !viewport
                .pbr_timing_for_test()
                .unwrap()
                .raster
                .capacity_reused
        );
        assert!(
            viewport
                .capacity_objects_for_test(surface)
                .unwrap()
                .staging
                .is_none()
        );
        fresh.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        r024_prepare(&mut viewport, &other_metrics);
        viewport
            .set_material_source(next_source, Viewport3DViewRevision::new(13), view(44.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        r024_prepare(&mut viewport, &other_metrics);
        let one = viewport.capacity_objects_for_test(surface).unwrap();
        let two = viewport.capacity_objects_for_test(other).unwrap();
        assert_ne!(one.buffers, two.buffers);
        assert_ne!(one.bindings, two.bindings);
        assert_ne!(one.textures, two.textures);
        assert_ne!(one.staging, two.staging);
        assert!(one.staging.is_some() && two.staging.is_some());
        let other_front = viewport.target_texture_id_for_test(other);
        renderer.resize_render_target(surface, (34, 30)).unwrap();
        let resized = active_metrics_for_surface(surface, 34, 30, 1.0);
        r024_prepare(&mut viewport, &resized);
        assert_eq!(viewport.capacity_counts_for_test(surface), (0, 0, 0));
        assert_eq!(viewport.target_texture_id_for_test(other), other_front);
        assert!(viewport.capacity_counts_for_test(other).0 >= 2);
        let mut inactive = other_metrics;
        inactive.activity = CustomRenderActivity::Inactive;
        viewport.prepare_custom_render(&inactive);
        assert_eq!(viewport.capacity_counts_for_test(other), (0, 0, 0));
        assert!(viewport.target_texture_id_for_test(other).is_none());
        drop((objects, reused, fresh_objects, one, two));
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(
            context
                .dynamic_state_accounting_for_test()
                .unwrap()
                .dynamic_storage_bytes,
            0
        );
        assert_eq!(
            context
                .dynamic_state_accounting_for_test()
                .unwrap()
                .dynamic_storage_slots,
            0
        );
        assert_eq!(context.target_charges_for_test(), (0, 0));
        renderer.unregister_render_target(surface).unwrap();
        renderer.unregister_render_target(other).unwrap();
        // A one-destination-slot budget cannot borrow the committed slot to
        // prepare its successor, even when previous GPU work has completed.
        for (slots, bytes, expected) in [
            (4, 67_108_864, PolicyBudget3D::DynamicStorageSlots),
            (64, 30_000, PolicyBudget3D::DynamicStorageBytesPerTarget),
        ] {
            let mut policy = PolicyLimits3D::conservative();
            policy.max_dynamic_storage_slots = slots;
            policy.max_dynamic_storage_bytes_per_target = bytes;
            let bounded = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
            let bounded_source = r025_source(&bounded, n, 0, 1.0);
            let mut limited = r024_viewport(bounded.clone(), bounded_source.clone(), view(56.0));
            let metrics = active_metrics(9900 + u64::from(n), 32, 32, 1.0);
            r024_prepare(&mut limited, &metrics);
            let old_front = limited.target_texture_id_for_test(metrics.surface_id);
            let old_swaps = limited.hdr_swaps_for_test(metrics.surface_id);
            let before = bounded.dynamic_state_accounting_for_test().unwrap();
            limited
                .set_material_source(bounded_source, Viewport3DViewRevision::new(2), view(52.0))
                .unwrap();
            limited.prepare_custom_render(&metrics);
            assert!(matches!(
                limited.material_failure_for_test(metrics.surface_id),
                Some(crate::three_d::pbr::PbrError3D::Context(
                    Context3DError::PolicyBudgetExceeded {
                        budget,
                        ..
                    }
                )) if *budget == expected
            ));
            assert_eq!(
                limited.target_texture_id_for_test(metrics.surface_id),
                old_front
            );
            assert_eq!(limited.hdr_swaps_for_test(metrics.surface_id), old_swaps);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let rolled_back = bounded.dynamic_state_accounting_for_test().unwrap();
            let staging_bytes = limited
                .capacity_objects_for_test(metrics.surface_id)
                .unwrap()
                .staging
                .as_ref()
                .map_or(0, wgpu::Buffer::size);
            assert_eq!(
                before.dynamic_storage_slots + u32::from(staging_bytes != 0),
                rolled_back.dynamic_storage_slots
            );
            assert_eq!(
                before.dynamic_storage_bytes + staging_bytes,
                rolled_back.dynamic_storage_bytes
            );
            limited.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(
                bounded
                    .dynamic_state_accounting_for_test()
                    .unwrap()
                    .dynamic_storage_slots,
                0
            );
        }
    }
}

#[test]
fn pbr_direct_warm_fov_encoder_abort_submit_custody_n1_n4() {
    use crate::three_d::hdr::{
        HdrOutput3D, HdrSubmissionStage3D as Stage, MaterialScenePreparation3D,
        prepare_material_scene, prepare_material_scene_with_capacity,
    };
    let view = |degrees: f32| {
        ViewDescriptor3::new(
            r025_view().pose(),
            PerspectiveProjection::try_finite(degrees.to_radians(), 0.1, 10.0)
                .unwrap()
                .into(),
        )
    };
    for n in [1, 4] {
        for submitted in [false, true] {
            for observe_completion in [true, false] {
                let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
                assert_eq!(
                    renderer.headless_adapter_for_test().backend,
                    wgpu::Backend::Metal
                );
                let gpu = renderer.gpu_resources();
                let queue = Arc::downgrade(&gpu.queue);
                let device = Arc::downgrade(&gpu.device);
                let context = Context3D::new(gpu.clone()).unwrap();
                let source = r025_source(&context, n, 0, 1.0);
                let mut output = HdrOutput3D::new(&context, 32, 32, n, &source.hdr).unwrap();
                let mut current = None;
                let mut first = prepare_material_scene(
                    &context,
                    source.clone(),
                    view(56.0),
                    view(56.0).pose().translation(),
                    (32, 32),
                    None,
                    None,
                    None,
                )
                .unwrap();
                output
                    .render(&source.hdr, |encoder, scene, depth| {
                        first.encode(None, encoder, scene, depth)
                    })
                    .unwrap();
                first.commit(&mut current);
                let old_front = output.front_for_test().clone();
                let old_pixel = observe_completion.then(|| read_r027_fp16(&old_front, &gpu));
                let mut capacity = None;
                let mut capacity_budget = None;
                let mut candidate = prepare_material_scene_with_capacity(
                    &context,
                    source.clone(),
                    view(52.0),
                    view(56.0).pose().translation(),
                    (32, 32),
                    &mut current,
                    &mut capacity,
                    &mut capacity_budget,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
                let completion = candidate
                    .encoder_completion()
                    .expect("real selected warm writer");
                let before = custom_render_audit::snapshot();
                let mut invalid_hdr = source.hdr.clone();
                invalid_hdr.exposure_ev = f32::NAN;
                let untouched = output.render_encoder_bound(
                    &invalid_hdr,
                    completion.clone(),
                    |_, _, _, _, _| panic!("prewrite error invoked writer"),
                );
                assert!(matches!(untouched, Err((_, Stage::BeforeWrites))));
                assert_eq!(
                    before.custom_submissions,
                    custom_render_audit::snapshot().custom_submissions
                );
                assert!(!completion.submitted());
                let sentinel = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("selected upload abandoned-encoder witness"),
                    size: 64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let staging = match &candidate {
                    MaterialScenePreparation3D::New(p) => {
                        p.capacity_objects_for_test().unwrap().staging.unwrap()
                    }
                    _ => panic!("selected candidate must own its capacity generation"),
                };
                let owner = std::cell::RefCell::new(Some(renderer));
                let before = custom_render_audit::snapshot();
                let swaps = output.swaps_for_test();
                let result = output.render_encoder_bound_interposed_for_test(
                    &source.hdr,
                    completion.clone(),
                    |encoder, scene, depth, tone, bytes| {
                        candidate.encode_encoder_bound(encoder, scene, depth, tone, bytes);
                        // This test-only copy witnesses the actual staging content
                        // in the same encoder as every selected production upload.
                        encoder.copy_buffer_to_buffer(&staging, 0, &sentinel, 0, 64);
                    },
                    || {
                        if !submitted {
                            drop(owner.borrow_mut().take());
                        }
                    },
                    || {
                        if submitted {
                            drop(owner.borrow_mut().take());
                        }
                    },
                );
                assert!(
                    matches!(result, Err((_, stage)) if stage == if submitted { Stage::SubmittedSwappedUnpublished } else { Stage::WritesQueuedNoFrameSubmit })
                );
                let after = custom_render_audit::snapshot();
                assert_eq!(
                    after.custom_submissions - before.custom_submissions,
                    u64::from(submitted)
                );
                assert_eq!(output.swaps_for_test() - swaps, u64::from(submitted));
                assert_eq!(after.custom_registrations, before.custom_registrations);
                assert_eq!(completion.submitted(), submitted);
                assert!(!output.is_live());
                drop(candidate);
                drop(current);
                drop(capacity);
                drop(capacity_budget);
                drop(output);
                if submitted && !completion.complete() {
                    assert!(
                        context
                            .dynamic_state_accounting_for_test()
                            .unwrap()
                            .dynamic_storage_slots
                            > 0,
                        "uncompleted submitted destination/staging charge released early"
                    );
                }
                if observe_completion {
                    let actual = read_r027_upload_bytes(&gpu, &sentinel);
                    let mut expected = [0u8; 64];
                    if submitted {
                        expected[..16]
                            .copy_from_slice(bytemuck::bytes_of(&[1.0f32, 0.0, 0.0, 0.0]));
                        let vb = crate::three_d::direct_shadow::metric_view(
                            view(52.0),
                            view(56.0).pose().translation(),
                            1.0,
                            32,
                            32,
                        )
                        .unwrap();
                        expected[16..].copy_from_slice(&bytemuck::bytes_of(&vb)[..48]);
                    }
                    assert_eq!(
                        actual, expected,
                        "unrelated submit executed an abandoned candidate"
                    );
                    assert_eq!(
                        read_r027_fp16(&old_front, &gpu).map(f32::to_bits),
                        old_pixel.unwrap().map(f32::to_bits)
                    );
                    assert_eq!(
                        context
                            .dynamic_state_accounting_for_test()
                            .unwrap()
                            .dynamic_storage_slots,
                        0
                    );
                    assert_eq!(context.target_charges_for_test(), (0, 0));
                }
                drop((
                    staging, sentinel, old_front, completion, source, context, gpu,
                ));
                // No completion poll in this control. Callback-held custody may
                // retain native handles, but must not retain Queue/Context Arcs.
                assert!(
                    queue.upgrade().is_none(),
                    "completion payload retained its own Queue"
                );
                assert!(
                    device.upgrade().is_none(),
                    "completion payload retained its Device/Context"
                );
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "PBR_CAPACITY_CUSTODY N{n} submitted={submitted} observed={observe_completion} submit_delta={} swap_delta={} registration_delta=0 no_arc_cycle=true\n", u64::from(submitted), u64::from(submitted))).unwrap();
            }
        }
    }
}

pub(in crate::tests) fn r025_source(
    context: &Context3D,
    n: u32,
    kind: u32,
    k: f64,
) -> Arc<MaterialSceneSource3D> {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D, PointLight3D, SpotLight3D};
    let mut material = PbrMaterialSource3D::defaults(1, 1);
    material.base_rgba = [1.0; 4];
    let mut scene = r024_source(
        context,
        super::three_d_geometry::r024_vertices(),
        material,
        n,
        false,
        r024_identity(),
        1,
    );
    let source = Arc::get_mut(&mut scene).unwrap();
    // r=1 gives f*cos<=2; this finite source's summed incident envelope is
    // <=4/.5²=16 for point/spot, or1 for directional. Thus64 conservatively
    // covers EVERY sample and the independent zero emission, not readback.
    source.hdr.maximum_rgb = [64.0; 3];
    let point = PointLight3D {
        position: WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
        intensity: [4.0; 3],
        radius_m: 0.5,
        range_m: 10.0,
    };
    source.direct = Some(Arc::new(DirectSource3D {
        identity: 700,
        revision: 1,
        meters_per_scene_unit: k,
        directional: if kind == 0 {
            Some(Directional3D {
                surface_to_source: [0.0, 0.0, 1.0],
                irradiance: [1.0; 3],
                shadow_far_m: 8.0,
            })
        } else {
            None
        },
        points: if kind == 1 {
            vec![point.clone()].into()
        } else {
            Arc::from([])
        },
        spots: if kind == 2 {
            vec![SpotLight3D {
                point,
                emitter_to_scene: [0.0, 0.0, -1.0],
                inner_radians: 0.2,
                outer_radians: 0.7,
            }]
            .into()
        } else {
            Arc::from([])
        },
        snapshot: source.snapshot.clone(),
        model: source.model.clone(),
    }));
    scene
}

#[test]
fn r025_actual_directional_point_spot_material_to_r027_fp16_n1_n4() {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: actual Device required");
    let adapter = renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    let gpu = renderer.gpu_resources();
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("R025 captured actual Device {adapter:?}; wgpu30.0.2/Naga30.0.2\n"),
    )
    .unwrap();
    for n in [1, 4] {
        for kind in 0..3 {
            for k in [1.0, 2.0] {
                let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
                let scene = r025_source(&context, n, kind, k);
                let mut viewport = r024_viewport(context.clone(), scene, r025_view());
                let metrics = active_metrics(910 + u64::from(kind), 32, 32, 1.0);
                r024_prepare(&mut viewport, &metrics);
                assert!(
                    viewport
                        .material_observation_for_test(metrics.surface_id)
                        .is_none(),
                    "actual production direct entry, no observer substitution"
                );
                let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
                let incident = if kind == 0 {
                    1.0
                } else {
                    let d = 2.0f64 * k;
                    let ratio = d / 10.0;
                    4.0 * (1.0 - ratio.powi(4)).powi(2) / (d * d)
                };
                // The observed center is x=+1/32,y=-1/32 scene units, not exactly
                // the axis. Bound its finite direction/half-vector variation relative
                // to the independently calculated on-axis r=1 reference.
                let radiance = incident / (4.0 * std::f64::consts::PI);
                let expected = radiance / (1.0 + radiance);
                for c in 0..3 {
                    assert!(
                        (f64::from(pixel[c]) - expected).abs() < 0.001,
                        "N{n} kind{kind} k{k} {pixel:?} expected{expected}"
                    );
                }
                assert_eq!(pixel[3], 1.0);
                let usage = context.resource_usage().unwrap();
                assert!(usage.resident_texture_bytes >= crate::three_d::direct_shadow::POOL_BYTES);
                let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
                r024_prepare(&mut viewport, &metrics);
                assert_eq!(swaps, viewport.hdr_swaps_for_test(metrics.surface_id));
                assert_eq!(usage, context.resource_usage().unwrap());
                std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual N{n} kind{kind} k{k}: FP16={pixel:?}; source bound64; full depth pool={}B\n",crate::three_d::direct_shadow::POOL_BYTES)).unwrap();
                viewport.on_unmount();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
    }
}

pub(super) fn r025_occluder_source(
    context: &Context3D,
    n: u32,
    kind: u32,
    caster: u32,
    rotation: Orientation3,
    revision: u64,
) -> (Arc<MaterialSceneSource3D>, ViewDescriptor3) {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D, PointLight3D, SpotLight3D};
    let q = glam::DQuat::from_xyzw(
        f64::from(rotation.x()),
        f64::from(rotation.y()),
        f64::from(rotation.z()),
        f64::from(rotation.w()),
    );
    let placement =
        ModelPlacement3::try_new(WorldPoint3::origin(), rotation, PositiveScale3::one()).unwrap();
    let mut packets = Vec::new();
    let mut placements = Vec::new();
    for object in 0..if caster == 0 { 1 } else { 2 } {
        let (x0, x1, y0, y1, z) = if object == 0 {
            (-1.0, 1.0, -1.0, 1.0, 0.0)
        } else if caster == 4 {
            (2.3, 2.7, -0.2, 0.2, 1.0)
        } else {
            (0.3, 0.7, -0.2, 0.2, 1.0)
        };
        let vertices = [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]]
            .into_iter()
            .zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])
            .map(|(p, uv)| PbrVertexSource3D {
                position: point(p[0], p[1], p[2]),
                normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
                uv0: Some(crate::three_d::TextureCoordinate2::try_new(uv[0], uv[1]).unwrap()),
                color: if object == 1 && caster == 6 {
                    Some(crate::three_d::LinearVertexColorRgba8::from_linear_unorm8(
                        255, 255, 255, 0,
                    ))
                } else {
                    None
                },
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
        let mut material = PbrMaterialSource3D::defaults(100 + object, revision);
        if object == 1 && caster == 1 {
            material.base_rgba[3] = 0.0; // Opaque ignores visibility alpha.
        }
        if object == 1 && matches!(caster, 2 | 3 | 5 | 6) {
            material.alpha_mode = PbrAlphaMode3D::Mask;
            if caster == 3 {
                material.base_rgba[3] = 0.5; // Equality with cutoff survives.
            } else if caster == 5 {
                material.base_rgba[3] = 0.25;
            }
            let tex = context
                .register_texture(
                    crate::three_d::Texture3D::try_new(
                        1,
                        1,
                        crate::three_d::TextureFormat3D::Rgba8UnormSrgb,
                        vec![255, 255, 255, if caster == 2 { 0 } else { 255 }],
                    )
                    .unwrap(),
                )
                .unwrap();
            let sampler = context
                .register_sampler(crate::three_d::Sampler3D::default())
                .unwrap();
            material.roles[0] = Some(PbrRoleBinding3D::V1 {
                texture: tex.handle().clone(),
                sampler: sampler.handle().clone(),
            });
        }
        let material = context.register_pbr_material(material, None).unwrap();
        packets.push(MaterialPacketSource3D::PbrMaterial {
            geometry,
            material,
            raster: RasterMode::SingleSided,
            stable_ordinal: object,
        });
        placements.push(placement);
    }
    let snapshot = context
        .admit_material_snapshot(StructuralRevision::new(revision), packets)
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(revision), placements)
        .unwrap();
    let emitter = q * glam::DVec3::new(1.0, 0.0, 2.0);
    let direction = (q * glam::DVec3::new(1.0, 0.0, 2.0)).normalize();
    let point_light = PointLight3D {
        position: WorldPoint3::try_new(emitter.x, emitter.y, emitter.z).unwrap(),
        intensity: [4.0; 3],
        radius_m: 0.5,
        range_m: 10.0,
    };
    let direct = DirectSource3D {
        identity: 701,
        revision,
        meters_per_scene_unit: 1.0,
        directional: if kind == 0 {
            Some(Directional3D {
                surface_to_source: direction.to_array(),
                irradiance: [1.0; 3],
                shadow_far_m: 8.0,
            })
        } else {
            None
        },
        points: if kind == 1 {
            vec![point_light.clone()].into()
        } else {
            Arc::from([])
        },
        spots: if kind == 2 {
            vec![SpotLight3D {
                point: point_light,
                emitter_to_scene: (-direction).to_array(),
                inner_radians: 0.2,
                outer_radians: 0.7,
            }]
            .into()
        } else {
            Arc::from([])
        },
        snapshot: snapshot.clone(),
        model: model.clone(),
    };
    let source = Arc::new(MaterialSceneSource3D {
        snapshot,
        model,
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: 701,
            revision,
            exposure_ev: 0.0,
            maximum_rgb: [64.0; 3],
        },
        samples: n,
        direct: Some(Arc::new(direct)),
        observe: false,
        indirect: None,
    });
    let camera = q * glam::DVec3::new(0.0, 0.0, 2.0);
    let view = ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(camera.x, camera.y, camera.z).unwrap(),
            rotation,
        ),
        crate::three_d::OrthographicProjection::try_new(0.4, 0.1, 10.0)
            .unwrap()
            .into(),
    );
    (source, view)
}

#[test]
fn r025_actual_v2_shadow_mask_base_uv_opaque_and_absence_n1_n4() {
    use super::three_d_context::r028_fixture as p;
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    use crate::three_d::mip_sampling::{MipMin3D, MipRole3D, SamplingFamily3D};
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        {
            let geometry = consumer_pbr_geometry(&context);
            let role = |kind, size, pixel: [u8; 4], id| {
                r028_role(
                    &context,
                    p::produce(
                        kind,
                        size,
                        size,
                        pixel.repeat((size * size) as usize),
                        id,
                        1,
                        true,
                    ),
                    id + 1,
                    MipMin3D::LinearMipLinear,
                    Linear,
                    ClampToEdge,
                    ClampToEdge,
                )
            };
            let normal = role(MipRole3D::Normal, 4, [128, 128, 255, 255], 98200);
            let mr = role(MipRole3D::MetallicRoughness, 8, [255, 255, 0, 255], 98210);
            let opaque_base = role(MipRole3D::OpaqueBase, 2, [255, 255, 255, 0], 98220);
            let clear_mask = role(MipRole3D::MaskBase, 2, [255, 255, 255, 0], 98230);
            // Keep the receiver independent of the caster's V2 alpha sources.
            // Its original certified B geometry and constant normal are unchanged.
            let receiver_normal = context
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
            let receiver_sampler = context
                .register_sampler(crate::three_d::Sampler3D::default())
                .unwrap();
            let mut receiver = PbrMaterialSource3D::defaults(98102, 1);
            receiver.metallic = 0.0;
            receiver.roughness = 1.0;
            receiver.roles[1] = Some(PbrRoleBinding3D::V1 {
                texture: receiver_normal.handle().clone(),
                sampler: receiver_sampler.handle().clone(),
            });
            let receiver = context.register_pbr_material(receiver, None).unwrap();
            // Reuse the existing off-camera geometry/light arrangement: the
            // caster at x=.5,z=1 lies outside this .4-wide view and shadows its
            // center along L=normalize(1,0,2). No color-pass caster covers it.
            let view = ViewDescriptor3::new(
                r025_view().pose(),
                crate::three_d::OrthographicProjection::try_new(0.4, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let scene_for = |revision,
                             caster: Option<(
                crate::three_d::pbr::PbrMaterialHandle3D,
                RasterMode,
            )>| {
                let mut packets = vec![MaterialPacketSource3D::PbrMaterial {
                    geometry: geometry.0.clone(),
                    material: receiver.clone(),
                    raster: RasterMode::SingleSided,
                    stable_ordinal: 0,
                }];
                let mut placements = vec![r024_identity()];
                if let Some((material, raster)) = caster {
                    let placement = ModelPlacement3::try_new(
                        WorldPoint3::try_new(0.5, 0.0, 1.0).unwrap(),
                        Orientation3::identity(),
                        PositiveScale3::try_new(0.2, 0.2, 1.0).unwrap(),
                    )
                    .unwrap();
                    // Repeat the packet without expanding the complete caster
                    // bounds. The second draw still has its own dynamic offsets.
                    for _ in 0..2 {
                        packets.push(MaterialPacketSource3D::PbrMaterial {
                            geometry: geometry.0.clone(),
                            material: material.clone(),
                            raster,
                            stable_ordinal: placements.len() as u64,
                        });
                        placements.push(placement);
                    }
                }
                let snapshot = context
                    .admit_material_snapshot(StructuralRevision::new(revision), packets)
                    .unwrap();
                let model = context
                    .admit_material_model(
                        snapshot.clone(),
                        ModelRevision::new(revision),
                        placements,
                    )
                    .unwrap();
                Arc::new(MaterialSceneSource3D {
                    direct: Some(Arc::new(DirectSource3D {
                        identity: 98100,
                        revision,
                        meters_per_scene_unit: 1.0,
                        directional: Some(Directional3D {
                            surface_to_source: glam::DVec3::new(1.0, 0.0, 2.0)
                                .normalize()
                                .to_array(),
                            irradiance: [1.0; 3],
                            shadow_far_m: 8.0,
                        }),
                        points: Arc::from([]),
                        spots: Arc::from([]),
                        snapshot: snapshot.clone(),
                        model: model.clone(),
                    })),
                    snapshot,
                    model,
                    hdr: crate::three_d::hdr::HdrSource3D {
                        identity: 98101,
                        revision,
                        exposure_ev: 0.0,
                        // Same conservative r=1 source envelope as the existing
                        // off-camera fixture; no emission or indirect source.
                        maximum_rgb: [64.0; 3],
                    },
                    samples: n,
                    observe: false,
                    indirect: None,
                })
            };
            let control = scene_for(1, None);
            control.validate(&context).unwrap();
            let mut viewport = r024_viewport(context.clone(), control.clone(), view);
            let metrics = active_metrics(98100 + u64::from(n), 32, 32, 1.0);
            viewport.set_pbr_timing_for_test(true);
            r024_prepare(&mut viewport, &metrics);
            let lit = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            assert!(lit.iter().all(|v| v.is_finite()));
            assert!(lit[0] > 0.0);
            assert_eq!(lit[3], 1.0);
            renderer.drain_pending_image_registrations();
            let lit_image = renderer.render_registered_image_pixel_for_test(
                viewport
                    .target_texture_id_for_test(metrics.surface_id)
                    .unwrap(),
                [16.5 / 32.0; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            let cases = [
                (
                    "Opaque/base0/factor0",
                    PbrAlphaMode3D::Opaque,
                    Some(opaque_base.0.clone()),
                    0.0,
                    true,
                    false,
                ),
                (
                    "Mask/base0",
                    PbrAlphaMode3D::Mask,
                    Some(clear_mask.0.clone()),
                    1.0,
                    false,
                    false,
                ),
                (
                    "Mask/base1/factor=cutoff",
                    PbrAlphaMode3D::Mask,
                    Some(clear_mask.0.clone()),
                    0.5,
                    true,
                    false,
                ),
                (
                    "Mask/absent-base/factor.25",
                    PbrAlphaMode3D::Mask,
                    None,
                    0.25,
                    false,
                    false,
                ),
                (
                    "Mask/absent-base/factor=cutoff",
                    PbrAlphaMode3D::Mask,
                    None,
                    0.5,
                    true,
                    false,
                ),
                (
                    "Opaque/double-sided/changed-factors",
                    PbrAlphaMode3D::Opaque,
                    Some(opaque_base.0.clone()),
                    0.0,
                    true,
                    true,
                ),
            ];
            let mut view_revision = Viewport3DViewRevision::new(1);
            let mut previous_material = None;
            let mut previous_binding = None;
            for (step, (label, mode, base, alpha, blocked, double_sided)) in
                cases.into_iter().enumerate()
            {
                let revision = step as u64 + 2;
                let base_present = base.is_some();
                let replace_mask_texture = mode == PbrAlphaMode3D::Mask && base_present && blocked;
                let base = if replace_mask_texture {
                    let Some(PbrRoleBinding3D::V2 { texture, sampler }) = base else {
                        unreachable!("V2 Mask successor")
                    };
                    let chain =
                        p::produce(MipRole3D::MaskBase, 2, 2, vec![255; 16], 98230, 2, true);
                    let mut next_sampler = p::sampler(
                        &chain,
                        98231,
                        MipMin3D::LinearMipLinear,
                        Linear,
                        ClampToEdge,
                        ClampToEdge,
                    );
                    next_sampler.revision = 2;
                    let next_texture = context.register_mip_chain(chain, Some(&texture)).unwrap();
                    let next_sampler = context
                        .register_mip_sampler(next_sampler, Some(&sampler))
                        .unwrap();
                    assert!(matches!(
                        context.lease_mip_chain(&texture),
                        Err(crate::three_d::pbr::PbrError3D::Resource(_))
                    ));
                    assert!(matches!(
                        context.lease_mip_sampler(&sampler),
                        Err(crate::three_d::pbr::PbrError3D::Resource(_))
                    ));
                    Some(PbrRoleBinding3D::V2 {
                        texture: next_texture,
                        sampler: next_sampler,
                    })
                } else {
                    base
                };
                let mut material = PbrMaterialSource3D::defaults(98103, revision);
                material.sampling = SamplingFamily3D::V2;
                material.alpha_mode = mode;
                material.base_rgba = [0.5, 0.5, if double_sided { 0.7 } else { 0.5 }, alpha];
                material.metallic = 0.0;
                material.roughness = if double_sided { 0.9 } else { 1.0 };
                material.double_sided = double_sided;
                material.roles[0] = base;
                material.roles[1] = Some(normal.0.clone());
                material.roles[2] = Some(mr.0.clone());
                let admitted = context
                    .register_pbr_material(material.clone(), previous_material.as_ref())
                    .unwrap();
                if let Some(old) = previous_material.replace(admitted.clone()) {
                    assert!(matches!(
                        context.lease_pbr_material(&old),
                        Err(crate::three_d::pbr::PbrError3D::Resource(_))
                    ));
                }
                let raster = if material.double_sided {
                    RasterMode::DoubleSided
                } else {
                    RasterMode::SingleSided
                };
                let source = scene_for(revision, Some((admitted, raster)));
                source.validate(&context).unwrap();
                viewport.set_pbr_timing_for_test(true);
                assert!(
                    viewport
                        .set_material_source_for_test(source.clone(), view_revision, view)
                        .unwrap()
                );
                r024_prepare(&mut viewport, &metrics);
                let timing = viewport.pbr_timing_for_test().unwrap();
                assert!(timing.complete && timing.hdr.complete);
                assert_eq!(timing.raster.material_group2_creations, 2);
                assert_eq!(timing.raster.material_group2_hits, 1);
                let objects = viewport
                    .capacity_objects_for_test(metrics.surface_id)
                    .unwrap();
                let groups = &objects.bindings[6..];
                assert_eq!(groups.len(), 3);
                assert_ne!(groups[0], groups[1]);
                assert_eq!(groups[1], groups[2]);
                assert_ne!(objects.material_offsets[1], objects.material_offsets[2]);
                if let Some(old) = previous_binding.replace(groups[1].clone()) {
                    assert_ne!(old, groups[1]);
                }
                assert_no_runtime_gpu_model(&timing.raster);
                // Only the explicitly invoked offline model below visits UV triangles.
                let offline_shadow_uv = mode == PbrAlphaMode3D::Mask && base_present;
                let proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
                assert_eq!(proof.faces, [0, 1, 2]);
                assert_eq!(proof.passes_encoded, 3);
                assert_eq!(proof.depth_format, wgpu::TextureFormat::Depth32Float);
                assert_eq!(proof.depth_samples, 1);
                let committed = viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .committed_material()
                    .unwrap();
                assert!(Arc::ptr_eq(committed.source(), &source));
                assert_eq!(
                    committed.structural_revision(),
                    StructuralRevision::new(revision)
                );
                assert_eq!(committed.model_revision(), ModelRevision::new(revision));
                assert_eq!(committed.view(), view);
                assert_eq!(committed.view_revision(), view_revision);
                assert_eq!(committed.target_key().sample_count().value(), n);
                assert_eq!(
                    committed.target_key().color_format(),
                    wgpu::TextureFormat::Rgba16Float
                );
                let serial = committed.request_serial();
                let key = committed.target_key();
                let front = viewport
                    .target_texture_id_for_test(metrics.surface_id)
                    .unwrap();
                assert!(
                    viewport
                        .material_observation_for_test(metrics.surface_id)
                        .is_none()
                );
                let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
                renderer.drain_pending_image_registrations();
                let image = renderer.render_registered_image_pixel_for_test(
                    front,
                    [16.5 / 32.0; 2],
                    [1.0; 4],
                    wgpu::Color::WHITE,
                );
                assert_eq!(pixel[3], 1.0);
                if blocked {
                    assert_eq!(&pixel[..3], &[0.0; 3], "off-camera {label} did not cast");
                    assert_eq!(image, [0, 0, 0, 255]);
                } else {
                    assert_eq!(pixel.map(f32::to_bits), lit.map(f32::to_bits), "{label}");
                    assert_eq!(image, lit_image, "{label}");
                }
                // Explicit offline model only: this finite row's modeled packed
                // clip sum overflows. Production admission never calls this model.
                // Keep its role/face attribution as diagnostic evidence.
                let mut overflowing = glam::Mat4::IDENTITY.to_cols_array_2d();
                overflowing[0][0] = f32::MAX * 0.75;
                overflowing[3][0] = f32::MAX * 0.75;
                let main_error = crate::three_d::direct_shadow::qualify_material_uvs_for_test(
                    &context,
                    &source,
                    view,
                    overflowing,
                    None,
                )
                .unwrap_err();
                let crate::three_d::pbr::PbrError3D::UvGradient(main) = main_error else {
                    panic!("expected offline modeled clip overflow")
                };
                assert_eq!(main.packet, 1);
                assert_eq!(main.role_extent, [8, 8]);
                assert_eq!(main.shadow_face, None);
                assert_eq!(main.operation, "packed clip dot finite and within f32");
                let shadow_probe = crate::three_d::direct_shadow::qualify_material_uvs_for_test(
                    &context,
                    &source,
                    view,
                    overflowing,
                    Some(0),
                );
                if offline_shadow_uv {
                    let crate::three_d::pbr::PbrError3D::UvGradient(shadow) =
                        shadow_probe.unwrap_err()
                    else {
                        panic!("offline Mask base model was skipped")
                    };
                    assert_eq!(shadow.packet, 1);
                    assert_eq!(shadow.role_extent, [2, 2]);
                    assert_eq!(shadow.shadow_face, Some(0));
                    assert_eq!(shadow.operation, main.operation);
                } else {
                    shadow_probe.unwrap();
                }
                if mode == PbrAlphaMode3D::Opaque {
                    // Opaque still validates every provided source role before
                    // admission; an unused shadow normal cannot hide a bad role.
                    material.roles[1] = Some(mr.0.clone());
                    assert!(matches!(
                        context.register_pbr_material(material, None),
                        Err(crate::three_d::pbr::PbrError3D::Role { .. })
                    ));
                }
                assert_eq!(
                    viewport.target_texture_id_for_test(metrics.surface_id),
                    Some(front)
                );
                assert_eq!(
                    viewport
                        .status_for(metrics.surface_id)
                        .unwrap()
                        .committed_material()
                        .unwrap()
                        .request_serial(),
                    serial
                );
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R025 MASK-ONLY SHADOW {adapter:?} N{n} {label}: source rev{revision} request{serial:?} key={key:?} off-camera caster; depth faces={:?}/encoded{}; main_triangles={} shadow_uv_triangles={} uv_shadow_domain={:?}; FP16={pixel:?} Image={image:?} control_FP16={lit:?} control_Image={lit_image:?}; actual production observe=false\n",
                    proof.faces, proof.passes_encoded, timing.raster.main_triangles, timing.raster.shadow_triangles, timing.raster.uv_shadow_domain,
                )).unwrap();
                if mode == PbrAlphaMode3D::Mask && base_present && blocked {
                    let invalid_view = ViewDescriptor3::new(
                        view.pose(),
                        crate::three_d::OrthographicProjection::try_new(0.4, 0.0, 10.0)
                            .unwrap()
                            .into(),
                    );
                    let error = source
                        .direct
                        .as_ref()
                        .unwrap()
                        .prepare_frame(&source, invalid_view, view.pose().translation(), 32, 32)
                        .err()
                        .unwrap();
                    view_revision = view_revision.checked_next().unwrap();
                    viewport
                        .set_material_source_for_test(source.clone(), view_revision, invalid_view)
                        .unwrap();
                    viewport.prepare_custom_render(&metrics);
                    assert_eq!(
                        viewport.material_failure_for_test(metrics.surface_id),
                        Some(&error)
                    );
                    let old = viewport
                        .status_for(metrics.surface_id)
                        .unwrap()
                        .committed_material()
                        .unwrap();
                    assert_eq!(old.request_serial(), serial);
                    assert_eq!(old.target_key(), key);
                    assert!(Arc::ptr_eq(old.source(), &source));
                    assert_eq!(
                        viewport.target_texture_id_for_test(metrics.surface_id),
                        Some(front)
                    );
                    assert_eq!(
                        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu)
                            .map(f32::to_bits),
                        pixel.map(f32::to_bits)
                    );
                    view_revision = view_revision.checked_next().unwrap();
                    viewport
                        .set_material_source_for_test(source, view_revision, view)
                        .unwrap();
                    r024_prepare(&mut viewport, &metrics);
                }
            }
            let other = active_metrics(98110 + u64::from(n), 40, 32, 1.0);
            viewport.set_pbr_timing_for_test(true);
            r024_prepare(&mut viewport, &other);
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(timing.material_group2_creations, 2);
            assert_eq!(timing.material_group2_hits, 1);
            let objects = viewport
                .capacity_objects_for_test(other.surface_id)
                .unwrap();
            assert_eq!(objects.bindings[7], objects.bindings[8]);
            assert_ne!(previous_binding.unwrap(), objects.bindings[7]);
            drop(objects);
            viewport.on_unmount();
        }
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        let usage = context.resource_usage().unwrap();
        assert_eq!((usage.pending_requests, usage.pending_publications), (0, 0));
        assert_eq!(usage.resident_geometry_bytes, 0);
    }
}

#[test]
fn r025_actual_off_camera_opaque_mask_movement_and_six_cube_depth_orientations() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: actual geometric depth Device required");
    let gpu = renderer.gpu_resources();
    let rotations = [
        glam::Quat::IDENTITY,
        glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        glam::Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        glam::Quat::from_rotation_y(std::f32::consts::PI),
        glam::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    ];
    for n in [1, 4] {
        for kind in 0..3 {
            for (r, q) in rotations
                .into_iter()
                .enumerate()
                .take(if kind == 1 { 6 } else { 1 })
            {
                let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
                let rotation = Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap();
                let (source, view) = r025_occluder_source(&context, n, kind, 0, rotation, 1);
                let mut viewport = r024_viewport(context.clone(), source, view);
                let metrics = active_metrics(930 + u64::from(kind), 32, 32, 1.0);
                r024_prepare(&mut viewport, &metrics);
                let lit = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
                assert!(lit[0] > 0.005);
                let proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
                assert_eq!(
                    proof.depth_2d,
                    wgpu::Extent3d {
                        width: 1024,
                        height: 1024,
                        depth_or_array_layers: 7
                    }
                );
                assert_eq!(
                    proof.depth_cube,
                    wgpu::Extent3d {
                        width: 512,
                        height: 512,
                        depth_or_array_layers: 24
                    }
                );
                assert_eq!(proof.depth_format, wgpu::TextureFormat::Depth32Float);
                assert_eq!(proof.depth_samples, 1);
                assert!(proof.light_bytes >= 608 && proof.transform_bytes >= 1984);
                assert_eq!(proof.primary_and_face_bytes, vec![(6 * 52, 6 * 12)]);
                assert_eq!(proof.faces.len(), [3, 6, 1][kind as usize]);
                assert_eq!(
                    proof.counts,
                    [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]][kind as usize]
                );
                let modes = if r == 0 {
                    vec![1, 2, 3, 4, 5, 6, 0]
                } else {
                    vec![1, 0]
                };
                let mut results = Vec::new();
                for (step, caster) in modes.into_iter().enumerate() {
                    let (source, view) =
                        r025_occluder_source(&context, n, kind, caster, rotation, step as u64 + 2);
                    viewport.set_pbr_timing_for_test(true);
                    viewport
                        .set_material_source_for_test(source, Viewport3DViewRevision::new(1), view)
                        .unwrap();
                    r024_prepare(&mut viewport, &metrics);
                    let timing = viewport.pbr_timing_for_test().unwrap().raster;
                    assert_eq!(
                        (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                        (0, 0)
                    );
                    let pixel =
                        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
                    assert_eq!(pixel[3], 1.0);
                    if caster == 1 || caster == 3 {
                        assert_eq!(
                            &pixel[..3],
                            &[0.0; 3],
                            "real occluder failed N{n} kind{kind} rotation{r}: {pixel:?}"
                        );
                    } else {
                        assert!(
                            (pixel[0] - lit[0]).abs() < 0.001,
                            "removed/masked/moved caster did not invalidate real map"
                        );
                    }
                    results.push((caster, pixel));
                }
                std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual geometry N{n} kind{kind} rotation{r} off-camera caster: lit={lit:?}, successors={results:?}\n")).unwrap();
                viewport.on_unmount();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert_eq!(
                    context.resource_usage().unwrap().resident_geometry_bytes,
                    0,
                    "D-only expanded geometry outlived its last actual user"
                );
                assert_eq!(
                    context.resource_usage().unwrap().admitted_shader_modules,
                    0,
                    "D/tone modules outlived Queue completion"
                );
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert_eq!(context.target_charges_for_test(), (0, 0));
            }
        }
    }
}

#[test]
fn r025_incremental_local_lights_preserve_mask_and_near_cap_refusal_n1_n4() {
    use crate::three_d::direct_shadow::DirectSource3D;
    use crate::three_d::pbr::{MaterialSnapshot3D, PbrError3D};

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let foreign_renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let foreign_gpu = foreign_renderer.gpu_resources();
    assert!(!Arc::ptr_eq(&gpu.device, &foreign_gpu.device));
    let foreign = Context3D::new(foreign_gpu).unwrap();
    for n in [1, 4] {
        for kind in [1, 2] {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let (base, view) =
                r025_occluder_source(&context, n, kind, 3, Orientation3::identity(), 1);
            let rebind = |old: &MaterialSceneSource3D,
                          snapshot: Arc<MaterialSnapshot3D>,
                          placements: Vec<ModelPlacement3>,
                          revision: u64| {
                let model = context
                    .admit_material_model(
                        snapshot.clone(),
                        ModelRevision::new(revision),
                        placements,
                    )
                    .unwrap();
                let light = old.direct.as_ref().unwrap();
                Arc::new(MaterialSceneSource3D {
                    snapshot: snapshot.clone(),
                    model: model.clone(),
                    hdr: crate::three_d::hdr::HdrSource3D {
                        revision,
                        ..old.hdr.clone()
                    },
                    samples: n,
                    direct: Some(Arc::new(DirectSource3D {
                        identity: light.identity,
                        revision,
                        meters_per_scene_unit: light.meters_per_scene_unit,
                        directional: light.directional.clone(),
                        points: light.points.clone(),
                        spots: light.spots.clone(),
                        snapshot,
                        model,
                    })),
                    indirect: None,
                    observe: false,
                })
            };
            let mut packets = base.snapshot.source_packets().to_vec();
            let MaterialPacketSource3D::PbrMaterial {
                geometry,
                material,
                raster,
                ..
            } = packets[0].clone()
            else {
                unreachable!()
            };
            let translate = |x, z| {
                ModelPlacement3::try_new(
                    WorldPoint3::try_new(x, 0.0, z).unwrap(),
                    Orientation3::identity(),
                    PositiveScale3::one(),
                )
                .unwrap()
            };
            let mut placements = vec![translate(0.0, 0.0); packets.len()];
            // Real repeated occurrences out of view and out of light range.
            // They are still part of every current complete caster sequence.
            for i in 2..8 {
                packets.push(MaterialPacketSource3D::PbrMaterial {
                    geometry: geometry.clone(),
                    material: material.clone(),
                    raster,
                    stable_ordinal: i,
                });
                placements.push(translate(20.0 + i as f64 * 2.0, 0.0));
            }
            let snapshot = context
                .admit_material_snapshot(StructuralRevision::new(1), packets)
                .unwrap();
            let first = rebind(&base, snapshot.clone(), placements.clone(), 1);
            let mut viewport = r024_viewport(context.clone(), first.clone(), view);
            let metrics = active_metrics(938 + u64::from(kind), 32, 32, 1.0);
            let surface = metrics.surface_id;
            r024_prepare(&mut viewport, &metrics);
            let occluded = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            assert_eq!(occluded, [0.0, 0.0, 0.0, 1.0]);

            placements[1] = translate(2.0, 0.0);
            let moved = rebind(&first, snapshot.clone(), placements.clone(), 2);
            let proof = viewport.direct_proof_for_test(surface).unwrap();
            let origin = view.pose().translation();
            assert!(proof.slot_preflight_for_test(&context, &first, &moved, origin));
            let changed_origin =
                WorldPoint3::try_new(origin.x().next_up(), origin.y(), origin.z()).unwrap();
            assert!(!proof.slot_preflight_for_test(&context, &first, &moved, changed_origin));
            assert!(!proof.slot_preflight_for_test(&foreign, &first, &moved, origin));
            let mut stale = rebind(&first, snapshot.clone(), placements.clone(), 2);
            Arc::get_mut(&mut stale).unwrap().direct = first.direct.clone();
            assert!(matches!(
                viewport.set_material_source_for_test(stale, Viewport3DViewRevision::new(1), view),
                Err(PbrError3D::Direct { .. }) | Err(PbrError3D::Association(_))
            ));

            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source_for_test(moved.clone(), Viewport3DViewRevision::new(1), view)
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            assert_eq!(
                viewport.assert_direct_full_parity_for_test(surface),
                (16, 0)
            );
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (7, 1)
            );
            let lit = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            assert!(lit[0] > 0.005 && lit[3] == 1.0);
            let serial = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial();
            let swaps = viewport.hdr_swaps_for_test(surface);

            placements[1] = translate(0.5, 1.0);
            let refused = rebind(&moved, snapshot.clone(), placements.clone(), 3);
            let full_error = refused
                .direct
                .as_ref()
                .unwrap()
                .prepare_frame(&refused, view, view.pose().translation(), 32, 32)
                .err()
                .expect("full near-cap refusal");
            assert!(matches!(
                &full_error,
                PbrError3D::Direct {
                    field: "point near cap" | "spot near cap",
                    ..
                }
            ));
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source_for_test(refused, Viewport3DViewRevision::new(1), view)
                .unwrap();
            viewport.prepare_custom_render(&metrics);
            assert_eq!(
                viewport
                    .material_failure_for_test(surface)
                    .unwrap()
                    .to_string(),
                full_error.to_string()
            );
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (7, 1)
            );
            assert_eq!(viewport.hdr_swaps_for_test(surface), swaps);
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .request_serial(),
                serial
            );
            assert_eq!(
                read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu),
                lit
            );
            println!(
                "r025_local_slots N{n} kind={kind} shadow={occluded:?} lit={lit:?} refused={full_error}"
            );

            placements[1] = translate(2.0, 0.0);
            let mut current = moved;
            for (case, revision) in [("exposure", 4), ("bound", 5), ("light", 6), ("k", 7)] {
                let mut next = rebind(&current, snapshot.clone(), placements.clone(), revision);
                let next_source = Arc::get_mut(&mut next).unwrap();
                let direct = Arc::get_mut(next_source.direct.as_mut().unwrap()).unwrap();
                match case {
                    "exposure" => next_source.hdr.exposure_ev = 1.0,
                    "bound" => next_source.hdr.maximum_rgb[0] = 65.0,
                    "light" if kind == 1 => Arc::make_mut(&mut direct.points)[0].intensity[0] = 3.5,
                    "light" => Arc::make_mut(&mut direct.spots)[0].point.intensity[0] = 3.5,
                    "k" => direct.meters_per_scene_unit = 1.0f64.next_up(),
                    _ => unreachable!(),
                }
                viewport.set_pbr_timing_for_test(true);
                viewport
                    .set_material_source_for_test(
                        next.clone(),
                        Viewport3DViewRevision::new(1),
                        view,
                    )
                    .unwrap();
                r024_prepare(&mut viewport, &metrics);
                let timing = viewport.pbr_timing_for_test().unwrap().raster;
                assert_eq!(
                    (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                    (0, 0),
                    "{case}"
                );
                assert_eq!(
                    viewport.assert_direct_full_parity_for_test(surface),
                    (16, 0)
                );
                current = next;
            }
            let signed_zero = translate(-0.0, 0.0);
            assert_ne!(
                placements[0].translation().x().to_bits(),
                signed_zero.translation().x().to_bits()
            );
            placements[0] = signed_zero;
            let next = rebind(&current, snapshot.clone(), placements.clone(), 8);
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source_for_test(next.clone(), Viewport3DViewRevision::new(1), view)
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (7, 1)
            );
            assert_eq!(
                viewport.assert_direct_full_parity_for_test(surface),
                (16, 0)
            );

            let mut replacement = rebind(&next, snapshot.clone(), placements.clone(), 9);
            Arc::get_mut(&mut replacement).unwrap().hdr.identity += 1;
            viewport.set_pbr_timing_for_test(true);
            viewport
                .replace_pbr_scene(
                    crate::three_d::PbrScene3D(replacement.clone()),
                    Viewport3DViewRevision::new(1),
                    view,
                )
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (0, 0)
            );

            let mut packets = snapshot.source_packets().to_vec();
            packets.swap(0, 1);
            placements.swap(0, 1);
            let reordered = context
                .admit_material_snapshot(StructuralRevision::new(2), packets)
                .unwrap();
            let next = rebind(&replacement, reordered.clone(), placements.clone(), 10);
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source_for_test(next.clone(), Viewport3DViewRevision::new(1), view)
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (0, 0)
            );
            assert_eq!(
                viewport.assert_direct_full_parity_for_test(surface),
                (16, 0)
            );

            let resized = active_metrics(938 + u64::from(kind), 40, 32, 1.0);
            let next = rebind(&next, reordered, placements, 11);
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source_for_test(next, Viewport3DViewRevision::new(1), view)
                .unwrap();
            r024_prepare(&mut viewport, &resized);
            let timing = viewport.pbr_timing_for_test().unwrap().raster;
            assert_eq!(
                (timing.direct_slots_reused, timing.direct_slots_rebuilt),
                (0, 0)
            );
            viewport.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
}

#[test]
fn r025_actual_full_nine_sources_and_budget_failure_preserve_old_front_then_zero_d_retires() {
    use crate::three_d::direct_shadow::{DirectSource3D, POOL_BYTES, PointLight3D, SpotLight3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: real Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let mut policy = PolicyLimits3D::conservative();
        policy.max_total_attachment_bytes = POOL_BYTES + (20 * u64::from(n) + 16) * 32 * 32;
        policy.max_single_target_attachment_bytes = policy.max_total_attachment_bytes;
        let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
        let mut source = r025_source(&context, n, 0, 1.0);
        let scene = Arc::get_mut(&mut source).unwrap();
        let d = Arc::get_mut(scene.direct.as_mut().unwrap()).unwrap();
        d.points = (0..4)
            .map(|i| PointLight3D {
                position: WorldPoint3::try_new(0.1 * f64::from(i), 0.0, 2.0).unwrap(),
                intensity: [
                    0.1 * (i + 1) as f32,
                    0.05 * (i + 1) as f32,
                    0.025 * (i + 1) as f32,
                ],
                radius_m: 0.5,
                range_m: 10.0,
            })
            .collect::<Vec<_>>()
            .into();
        d.spots = d
            .points
            .iter()
            .map(|p| SpotLight3D {
                point: p.clone(),
                emitter_to_scene: glam::DVec3::new(-p.position.x(), 0.0, -2.0)
                    .normalize()
                    .to_array(),
                inner_radians: 0.2,
                outer_radians: 0.7,
            })
            .collect::<Vec<_>>()
            .into();
        let mut viewport = r024_viewport(context.clone(), source.clone(), r025_view());
        let metrics = active_metrics(950, 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        assert_eq!(proof.counts, [1, 4, 4, 0]);
        assert_eq!(proof.faces.len(), 31);
        let mut faces = proof.faces.clone();
        faces.sort_unstable();
        assert_eq!(faces, (0..31).collect::<Vec<_>>());
        assert!(pixel[0] > pixel[1] && pixel[1] > pixel[2]);
        let receiver = glam::DVec3::new(1.0 / 32.0, -1.0 / 32.0, 0.0);
        let v = (glam::DVec3::new(0.0, 0.0, 2.0) - receiver).normalize();
        let mut envelope_color = [1.0 / (std::f64::consts::PI * 2.0 * (v.z + 1.0)); 3];
        for p in source.direct.as_ref().unwrap().points.iter() {
            let delta = glam::DVec3::new(p.position.x(), p.position.y(), p.position.z()) - receiver;
            let d = delta.length();
            let nl = delta.z / d;
            let w = (1.0 - (d / p.range_m).powi(4)).powi(2);
            let f_cos = nl / (std::f64::consts::PI * (nl + 1.0) * (v.z + 1.0));
            for c in 0..3 {
                envelope_color[c] += 2.0 * f_cos * f64::from(p.intensity[c]) * w / d.powi(2);
            }
        }
        for c in 0..3 {
            let expected = envelope_color[c] / (1.0 + envelope_color[c]);
            assert!(
                (f64::from(pixel[c]) - expected).abs() < 0.001,
                "all9 actual direct sum {pixel:?} versus r1 closed form {expected}"
            );
        }
        let before = context.target_charges_for_test();
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        let next_direct = DirectSource3D {
            identity: 700,
            revision: 2,
            meters_per_scene_unit: 1.0,
            directional: source.direct.as_ref().unwrap().directional.clone(),
            points: source.direct.as_ref().unwrap().points.clone(),
            spots: source.direct.as_ref().unwrap().spots.clone(),
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
        };
        let next = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                revision: 2,
                ..source.hdr.clone()
            },
            samples: n,
            direct: Some(Arc::new(next_direct)),
            observe: false,
            indirect: None,
        });
        viewport
            .set_material_source_for_test(next, Viewport3DViewRevision::new(1), r025_view())
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        assert!(matches!(
            viewport.material_failure_for_test(metrics.surface_id),
            Some(crate::three_d::pbr::PbrError3D::Context(
                Context3DError::PolicyBudgetExceeded {
                    budget: PolicyBudget3D::TotalAttachmentBytes,
                    ..
                }
            ))
        ));
        assert_eq!(context.target_charges_for_test(), before);
        assert_eq!(swaps, viewport.hdr_swaps_for_test(metrics.surface_id));
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            pixel
        );
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &source
        ));
        let absent = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                revision: 3,
                ..source.hdr.clone()
            },
            samples: n,
            direct: None,
            observe: false,
            indirect: None,
        });
        viewport
            .set_material_source_for_test(absent, Viewport3DViewRevision::new(1), r025_view())
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            [0.0, 0.0, 0.0, 1.0]
        );
        assert!(viewport.direct_proof_for_test(metrics.surface_id).is_none());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(context.resource_usage().unwrap().resident_texture_bytes < POOL_BYTES);
        assert_eq!(
            context.target_charges_for_test().1,
            (20 * u64::from(n) + 16) * 32 * 32
        );
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 N{n} all9 sources/31 actual faces: {pixel:?}; failed full-pool successor kept original source/front; D=0 covered black and shadow charges retired\n")).unwrap();
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn r025_actual_last_local_slot_light_movement_and_three_directional_cascades() {
    use crate::three_d::direct_shadow::{PointLight3D, SpotLight3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: actual slot/cascade Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        for kind in [1, 2] {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let mut viewport = None;
            let mut observed = Vec::new();
            let metrics = active_metrics(958, 32, 32, 1.0);
            for (step, caster) in [0, 1, 1].into_iter().enumerate() {
                let (mut source, view) = r025_occluder_source(
                    &context,
                    n,
                    kind,
                    caster,
                    Orientation3::identity(),
                    step as u64 + 1,
                );
                let scene = Arc::get_mut(&mut source).unwrap();
                let d = Arc::get_mut(scene.direct.as_mut().unwrap()).unwrap();
                let mut selected = if kind == 1 {
                    d.points[0].clone()
                } else {
                    d.spots[0].point.clone()
                };
                if step == 2 {
                    selected.position = WorldPoint3::try_new(-1.0, 0.0, 2.0).unwrap();
                }
                let mut points = vec![
                    PointLight3D {
                        position: WorldPoint3::try_new(4.0, 4.0, 2.0).unwrap(),
                        intensity: [0.0; 3],
                        radius_m: 0.5,
                        range_m: 10.0,
                    };
                    3
                ];
                points.push(selected);
                if kind == 1 {
                    d.points = points.into();
                } else {
                    d.spots = points
                        .into_iter()
                        .map(|p| {
                            let axis =
                                -glam::DVec3::new(p.position.x(), p.position.y(), p.position.z())
                                    .normalize();
                            SpotLight3D {
                                point: p,
                                emitter_to_scene: axis.to_array(),
                                inner_radians: 0.2,
                                outer_radians: 0.7,
                            }
                        })
                        .collect::<Vec<_>>()
                        .into();
                }
                if let Some(v) = viewport.as_mut() {
                    Viewport3D::set_material_source_for_test(
                        v,
                        source,
                        Viewport3DViewRevision::new(1),
                        view,
                    )
                    .unwrap();
                } else {
                    viewport = Some(r024_viewport(context.clone(), source, view));
                }
                let v = viewport.as_mut().unwrap();
                r024_prepare(v, &metrics);
                let proof = v.direct_proof_for_test(metrics.surface_id).unwrap();
                assert_eq!(
                    proof.counts,
                    if kind == 1 {
                        [0, 4, 0, 0]
                    } else {
                        [0, 0, 4, 0]
                    }
                );
                observed.push(read_r027_fp16(
                    v.hdr_front_for_test(metrics.surface_id),
                    &gpu,
                ));
            }
            assert!(observed[0][0] > 0.01 && observed[2][0] > 0.01);
            assert_eq!(
                &observed[1][..3],
                &[0.0; 3],
                "slot3 sampled another source map"
            );
            assert!(observed.iter().all(|p| p[3] == 1.0));
            std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!(
                    "R025 actual N{n} kind{kind} slot3 lit/blocked/moved-light={observed:?}\n"
                ),
            )
            .unwrap();
            viewport.as_mut().unwrap().on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let mut results = Vec::new();
        let metrics = active_metrics(959, 32, 32, 1.0);
        for perspective in [false, true] {
            for (cascade, camera_z) in [1.0, 2.0, 6.0].into_iter().enumerate() {
                let mut values = Vec::new();
                for caster in [0, 1] {
                    let (source, _) =
                        r025_occluder_source(&context, n, 0, caster, Orientation3::identity(), 1);
                    let projection = if perspective {
                        PerspectiveProjection::try_finite(0.05, 0.1, 10.0)
                            .unwrap()
                            .into()
                    } else {
                        crate::three_d::OrthographicProjection::try_new(0.4, 0.1, 10.0)
                            .unwrap()
                            .into()
                    };
                    let view = ViewDescriptor3::new(
                        RigidPose3::new(
                            WorldPoint3::try_new(0.0, 0.0, camera_z).unwrap(),
                            Orientation3::identity(),
                        ),
                        projection,
                    );
                    let frame = source
                        .direct
                        .as_ref()
                        .unwrap()
                        .prepare_frame(&source, view, WorldPoint3::origin(), 32, 32)
                        .unwrap();
                    assert_eq!(frame.depth_proofs.len(), 3);
                    for (c, proof) in frame.depth_proofs.iter().enumerate() {
                        assert!(proof.range[0] > 0.0 && proof.range[1] < 1.0);
                        assert!(proof.corners.iter().all(|z| z[0] > 0.0 && z[1] < 1.0));
                        assert_eq!(
                            proof.row,
                            frame.transforms.matrices[c].map(|column| column[2])
                        );
                        assert!(
                            proof.t == 1.0 || (1..=24).any(|k| proof.t == 1.0 - 2.0f64.powi(-k))
                        );
                    }
                    if !perspective && camera_z == 1.0 && caster == 0 {
                        let proof = &frame.depth_proofs[2];
                        assert!(proof.t < 1.0);
                        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!(
                            "R025 corrected N{n} camera1 cascade2: t={:.17}, packed row={:?}, all-source/corner z={:?}, far corners={:?}\n",proof.t,proof.row,proof.range,&proof.corners[4..]
                        )).unwrap();
                    }
                    let mut viewport = r024_viewport(context.clone(), source, view);
                    r024_prepare(&mut viewport, &metrics);
                    values.push(read_r027_fp16(
                        viewport.hdr_front_for_test(metrics.surface_id),
                        &gpu,
                    ));
                    viewport.on_unmount();
                    renderer.drain_pending_image_registrations();
                    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                }
                assert!(values[0][0] > 0.01);
                assert_eq!(
                    &values[1][..3],
                    &[0.0; 3],
                    "directional cascade{cascade} projection{perspective}"
                );
                results.push((perspective, cascade, values));
            }
        }
        std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!(
                "R025 actual N{n} orthographic/perspective three-cascade lit/blocked={results:?}\n"
            ),
        )
        .unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn r025_actual_depth_envelopes_cover_rotated_scaled_origins_and_refuse_extreme_cancellation() {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: envelope Device required");
    let gpu = renderer.gpu_resources();
    let configurations = [
        (glam::Quat::IDENTITY, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 1.0),
        (
            glam::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            [1032.0, -2048.0, 8.0],
            [2.0, 1.0, 4.0],
            2.0,
        ),
        (
            glam::Quat::from_rotation_y(std::f32::consts::PI),
            [-4096.0, 72.0, -32.0],
            [1.0, 3.0, 2.0],
            1.0,
        ),
    ];
    for n in [1, 4] {
        for (case, (q, translation, scale, k)) in configurations.into_iter().enumerate() {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let rotation = Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap();
            let (mut source, _) = r025_occluder_source(&context, n, 0, 0, rotation, 1);
            let scene = Arc::get_mut(&mut source).unwrap();
            let origin =
                WorldPoint3::try_new(translation[0], translation[1], translation[2]).unwrap();
            let placement = ModelPlacement3::try_new(
                origin,
                rotation,
                PositiveScale3::try_new(scale[0], scale[1], scale[2]).unwrap(),
            )
            .unwrap();
            scene.model = context
                .admit_material_model(
                    scene.snapshot.clone(),
                    ModelRevision::new(2),
                    vec![placement],
                )
                .unwrap();
            let d = Arc::get_mut(scene.direct.as_mut().unwrap()).unwrap();
            d.model = scene.model.clone();
            d.meters_per_scene_unit = k;
            let dq = glam::DQuat::from_xyzw(
                f64::from(rotation.x()),
                f64::from(rotation.y()),
                f64::from(rotation.z()),
                f64::from(rotation.w()),
            );
            let camera =
                glam::DVec3::from_array(translation) + dq * glam::DVec3::new(0.0, 0.0, 2.0);
            let view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(camera.x, camera.y, camera.z).unwrap(),
                    rotation,
                ),
                crate::three_d::OrthographicProjection::try_new(0.4, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            let frame = source
                .direct
                .as_ref()
                .unwrap()
                .prepare_frame(&source, view, origin, 32, 32)
                .unwrap();
            assert_eq!(frame.depth_proofs.len(), 3);
            for proof in &frame.depth_proofs {
                assert!(proof.range[0] > 0.0 && proof.range[1] < 1.0);
                assert!(proof.corners.iter().all(|z| z[0] > 0.0 && z[1] < 1.0));
            }
            let mut viewport = r024_viewport(context.clone(), source, view);
            let metrics = active_metrics(960, 32, 32, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            assert!(pixel[0] > 0.0 && pixel[..3].iter().all(|v| v.is_finite()));
            assert_eq!(pixel[3], 1.0);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual N{n} envelope rotation/origin/scale case{case}: {pixel:?}; three complete strict depth intervals\n")).unwrap();
            viewport.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let source = r025_source(&context, n, 0, 1.0);
        let mut viewport = r024_viewport(context.clone(), source.clone(), r025_view());
        let metrics = active_metrics(961, 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        let charges = context.target_charges_for_test();
        // Finite f32 source and exactly representable inverse translation. The
        // source-to-metric cancellation uncertainty is comparable to the entire
        // fitted source span; none of the25 allowed contractions certifies it.
        let center = 1e20f32;
        let step = 2.0f32.powi(44);
        let vertices = [
            [center - step, -1.0, 0.0],
            [center + step, -1.0, 0.0],
            [center, 1.0, 0.0],
        ]
        .map(|p| PbrVertexSource3D {
            position: point(p[0], p[1], p[2]),
            normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
            uv0: None,
            color: None,
            tangent: None,
        })
        .to_vec();
        let placement = ModelPlacement3::try_new(
            WorldPoint3::try_new(-f64::from(center), 0.0, 0.0).unwrap(),
            Orientation3::identity(),
            PositiveScale3::one(),
        )
        .unwrap();
        let mut next = r024_source(
            &context,
            vertices,
            PbrMaterialSource3D::defaults(1, 2),
            n,
            false,
            placement,
            2,
        );
        let s = Arc::get_mut(&mut next).unwrap();
        s.hdr.maximum_rgb = [64.0; 3];
        s.direct = Some(Arc::new(DirectSource3D {
            identity: s.hdr.identity,
            revision: s.hdr.revision,
            meters_per_scene_unit: 1.0,
            directional: Some(Directional3D {
                surface_to_source: glam::DVec3::new(1.0, 0.0, 2.0).normalize().to_array(),
                irradiance: [1.0; 3],
                shadow_far_m: 8.0,
            }),
            points: Arc::from([]),
            spots: Arc::from([]),
            snapshot: s.snapshot.clone(),
            model: s.model.clone(),
        }));
        let refusal = next
            .direct
            .as_ref()
            .unwrap()
            .prepare_frame(&next, r025_view(), WorldPoint3::origin(), 32, 32)
            .err()
            .expect("extreme source unexpectedly certified");
        assert!(
            matches!(
                refusal,
                crate::three_d::pbr::PbrError3D::Direct {
                    field: "directional f32 coverage",
                    cause: "no finite certified depth envelope among25 candidates"
                }
            ),
            "{refusal:?}"
        );
        viewport
            .set_material_source_for_test(next, Viewport3DViewRevision::new(1), r025_view())
            .unwrap();
        let audit = custom_render_audit::snapshot();
        viewport.prepare_custom_render(&metrics);
        assert_eq!(
            custom_render_audit::snapshot(),
            audit,
            "refused envelope encoded/submitted or published"
        );
        assert_eq!(
            viewport.material_failure_for_test(metrics.surface_id),
            Some(&refusal)
        );
        assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
        assert_eq!(context.target_charges_for_test(), charges);
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            pixel
        );
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &source
        ));
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 N{n} extreme cancellation: {refusal:?}; no encode/submit/publication, original source/front/charges preserved\n")).unwrap();
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

pub(super) fn r025_observation_at(
    viewport: &Viewport3D,
    surface: SurfaceId,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    x: u32,
    y: u32,
) -> [[f32; 4]; 20] {
    let observation = viewport.material_observation_for_test(surface).unwrap();
    let width = viewport.hdr_front_for_test(surface).width();
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R025 actual source-flat fragment readback"),
        size: 320,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(
        &observation.values,
        u64::from(y * width + x) * 320,
        &buffer,
        0,
        320,
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let result = bytemuck::pod_read_unaligned(&bytes);
    drop(bytes);
    buffer.unmap();
    result
}

fn r025_observed_source(
    context: &Context3D,
    n: u32,
    partial: bool,
    back: bool,
) -> (Arc<MaterialSceneSource3D>, ViewDescriptor3) {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    let rotation = if back {
        Orientation3::try_from_xyzw(0.0, 1.0, 0.0, 0.0).unwrap()
    } else {
        Orientation3::identity()
    };
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        rotation,
        if partial {
            PositiveScale3::one()
        } else {
            PositiveScale3::try_new(2.0, 1.0, 4.0).unwrap()
        },
    )
    .unwrap();
    let mut material = PbrMaterialSource3D::defaults(2, 1);
    material.double_sided = back;
    let (vertices, indices) = if partial {
        let mut v = super::three_d_geometry::r024_vertices();
        for (i, ([x, y], uv)) in [
            [0.0, 35.0 / 256.0],
            [-1.0, 28.0 / 256.0],
            [1.0, 28.0 / 256.0],
        ]
        .into_iter()
        .zip([[0.5, 1.0], [0.0, 0.0], [1.0, 0.0]])
        .enumerate()
        {
            v[i].position = point(0.5 + x, -0.5 + y, 0.0);
            v[i].uv0 = Some(crate::three_d::TextureCoordinate2::try_new(uv[0], uv[1]).unwrap());
            v[i].normal = PbrUnitDirection3D::try_new(
                [[0.0, 0.0, 1.0], [0.6, 0.0, 0.8], [-0.6, 0.0, 0.8]][i],
            )
            .unwrap();
            v[i].tangent = Some((
                PbrUnitDirection3D::try_new(
                    [[1.0, 0.0, 0.0], [0.8, 0.0, -0.6], [0.8, 0.0, 0.6]][i],
                )
                .unwrap(),
                1.0,
            ));
            v[i].color = Some(crate::three_d::LinearVertexColorRgba8::from_linear_unorm8(
                if i == 0 { 255 } else { 0 },
                if i == 0 { 255 } else { 0 },
                if i == 0 { 255 } else { 0 },
                255,
            ));
        }
        let tex = context
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
        material.roles[1] = Some(PbrRoleBinding3D::V1 {
            texture: tex.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        (v, GeometryIndices::Unindexed)
    } else {
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, -0.5],
        ]
        .into_iter()
        .map(|p| PbrVertexSource3D {
            position: point(p[0], p[1], p[2]),
            normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
            uv0: None,
            color: None,
            tangent: None,
        })
        .collect();
        (v, GeometryIndices::from_u16(vec![0, 1, 2, 1, 0, 3]))
    };
    let geometry = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(vertices, indices).unwrap(),
            None,
        )
        .unwrap();
    let material = context.register_pbr_material(material, None).unwrap();
    let snapshot = context
        .admit_material_snapshot(
            StructuralRevision::new(1),
            vec![MaterialPacketSource3D::PbrMaterial {
                geometry,
                material,
                raster: if back {
                    RasterMode::DoubleSided
                } else {
                    RasterMode::SingleSided
                },
                stable_ordinal: 0,
            }],
        )
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(1), vec![placement])
        .unwrap();
    let direct = DirectSource3D {
        identity: 704,
        revision: 1,
        meters_per_scene_unit: if partial { 1.0 } else { 2.0 },
        directional: Some(Directional3D {
            surface_to_source: [0.0, 0.0, 1.0],
            irradiance: [1.0; 3],
            shadow_far_m: 16.0,
        }),
        points: Arc::from([]),
        spots: Arc::from([]),
        snapshot: snapshot.clone(),
        model: model.clone(),
    };
    let source = Arc::new(MaterialSceneSource3D {
        snapshot,
        model,
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: 704,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [4.0; 3],
        },
        samples: n,
        direct: Some(Arc::new(direct)),
        observe: true,
        indirect: None,
    });
    let eye = if partial {
        WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap()
    } else {
        WorldPoint3::try_new(if back { -0.5 } else { 0.5 }, 0.0, 4.0).unwrap()
    };
    let view = ViewDescriptor3::new(
        RigidPose3::new(eye, Orientation3::identity()),
        crate::three_d::OrthographicProjection::try_new(
            if partial { 32.0 } else { 2.0 },
            0.1,
            10.0,
        )
        .unwrap()
        .into(),
    );
    (source, view)
}

#[test]
fn r025_actual_flat_shared_faces_nonuniform_backface_clipping_and_partial_centroid_n4() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: actual flat-interstage Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        for back in [false, true] {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let (source, view) = r025_observed_source(&context, n, false, back);
            let mut viewport = r024_viewport(context.clone(), source, view);
            let metrics = active_metrics(970, 32, 32, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let a = r025_observation_at(&viewport, metrics.surface_id, &gpu, 16, 8);
            let b = r025_observation_at(&viewport, metrics.surface_id, &gpu, 16, 24);
            assert_eq!(a[0][3], 1.0);
            assert_eq!(b[0][3], 1.0);
            assert!((a[0][2] - 1.0).abs() < 0.00001);
            let expected = [
                0.0,
                if back {
                    2.0 / 5.0f32.sqrt()
                } else {
                    -2.0 / 5.0f32.sqrt()
                },
                1.0 / 5.0f32.sqrt(),
            ];
            for c in 0..3 {
                assert!(
                    (b[0][c] - expected[c]).abs() < 0.00001,
                    "flat source face after true inverse transpose/backface: {b:?}"
                );
            }
            assert_eq!(a[2][..3], [0.0, 0.0, 1.0]);
            assert!((b[2][1] + 1.0 / 5.0f32.sqrt()).abs() < 0.00001);
            assert_eq!(a[1][3], if back { 0.0 } else { 1.0 });
            assert!(
                (b[0][1] - b[1][1]).abs() > 0.8,
                "geometric face was replaced by smooth shading N"
            );
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 N{n} actual clipped shared faces k2 scale(2,1,4) back{back}: NgA={:?},NgB={:?},shadeB={:?}\n",a[0],b[0],b[1])).unwrap();
            viewport.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let (source, view) = r025_observed_source(&context, n, true, false);
        let mut viewport = r024_viewport(context.clone(), source, view);
        let metrics = active_metrics(971, 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let row = r025_observation_at(&viewport, metrics.surface_id, &gpu, 16, 16);
        if n == 1 {
            assert_eq!(pixel, [0.0; 4]);
            assert_eq!(row[0], [0.0; 4]);
        } else {
            assert!(
                pixel[3] > 0.0 && pixel[3] < 1.0,
                "actual partial N4 sample coverage must be observed"
            );
            assert_eq!(row[0], [0.0, 0.0, 1.0, 1.0]);
            assert!(row[7][3] >= 0.75 && row[8][3] >= 3.0 / 32.0);
            assert!(row[4].iter().all(|c| (0.0..=1.0).contains(c)));
            let x = row[3][0];
            let y = row[3][1];
            let a = glam::Vec2::new(0.5, -0.5 + 35.0 / 256.0);
            let b = glam::Vec2::new(-0.5, -0.5 + 28.0 / 256.0);
            let c = glam::Vec2::new(1.5, -0.5 + 28.0 / 256.0);
            let p = glam::Vec2::new(x, y);
            let area = (b - a).perp_dot(c - a);
            let wb = (p - a).perp_dot(c - a) / area;
            let wc = (b - a).perp_dot(p - a) / area;
            assert!(wb >= 0.0 && wc >= 0.0 && wb + wc <= 1.0);
            assert!(row[9][1] < 0.0, "UV0 stays center, not silently centroid");
        }
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual center-outside N{n}: FP16={pixel:?}; flatNg={:?}; local bounds={}/{}; color={:?}; receiver={:?}; centerUV={:?}\n",row[0],row[7][3],row[8][3],row[4],row[3],row[9])).unwrap();
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

fn r024_observation(
    viewport: &Viewport3D,
    surface: SurfaceId,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> [[f32; 4]; 11] {
    let o = viewport
        .material_observation_for_test(surface)
        .expect("real material fragment observation");
    let texture = viewport.hdr_front_for_test(surface);
    let index = (texture.height() / 2) * texture.width() + texture.width() / 2;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("actual R024 numeric observation readback"),
        size: 176,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&o.values, u64::from(index) * 176, &buffer, 0, 176);
    gpu.queue.submit([encoder.finish()]);
    let (s, r) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |v| s.send(v).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    r.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let out = bytemuck::pod_read_unaligned(&bytes);
    drop(bytes);
    buffer.unmap();
    out
}

#[test]
fn r025_actual_production_direct_partial_n4_fp16_to_ordinary_white_image() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: Image-composition Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let (mut source, view) = r025_observed_source(&context, n, true, false);
        Arc::get_mut(&mut source).unwrap().observe = false;
        let viewport = r024_viewport(context.clone(), source, view);
        let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (34, 34), 1.0)
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
            Constraints::tight(Size::new(34.0, 34.0)),
            &Theme::default(),
            &mut FontManagerTextEngine::new(),
            &IconRegistry::new(&[]),
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0).unwrap(),
        );
        let viewport = tree_viewport(&tree, child);
        assert_eq!(
            viewport.status_for(surface).unwrap().state(),
            Viewport3DLifecycleState::Committed,
            "{:?}",
            viewport.material_failure_for_test(surface)
        );
        assert!(viewport.material_observation_for_test(surface).is_none());
        let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        if n == 1 {
            assert_eq!(final_pixel, [0.0; 4]);
        } else {
            assert!(final_pixel[3] > 0.0 && final_pixel[3] < 1.0);
            assert!(
                final_pixel[..3]
                    .iter()
                    .all(|c| *c >= 0.0 && *c <= final_pixel[3])
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
        let composed = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
        for c in 0..3 {
            let encoded = f32::from(composed[c]) / 255.0;
            let linear = if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            };
            assert!(
                (linear - (final_pixel[c] + 1.0 - final_pixel[3])).abs() < 0.02,
                "R025 ordinary sampled premultiplied Image {composed:?} from {final_pixel:?}"
            );
        }
        assert_eq!(composed[3], 255);
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual production D N{n} partial coverage: FP16={final_pixel:?} -> ordinary WHITE-tint/white-backdrop Image={composed:?}\n")).unwrap();
    }
}

#[test]
fn r025_explicit_d_on_empty_geometry_still_owns_full_pool_and_zero_d_withdraws_it() {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D, POOL_BYTES};
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R025: Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let snapshot = context
        .admit_material_snapshot(StructuralRevision::new(1), vec![])
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(1), vec![])
        .unwrap();
    let direct = DirectSource3D {
        identity: 705,
        revision: 1,
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
    };
    let scene = Arc::new(MaterialSceneSource3D {
        snapshot: snapshot.clone(),
        model: model.clone(),
        hdr: crate::three_d::hdr::HdrSource3D {
            identity: 705,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [0.0; 3],
        },
        samples: 1,
        direct: Some(Arc::new(direct)),
        observe: false,
        indirect: None,
    });
    let mut viewport = r024_viewport(context.clone(), scene.clone(), r025_view());
    let metrics = active_metrics(975, 32, 32, 1.0);
    r024_prepare(&mut viewport, &metrics);
    assert_eq!(
        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
        [0.0; 4]
    );
    assert_eq!(
        viewport
            .direct_proof_for_test(metrics.surface_id)
            .unwrap()
            .faces
            .len(),
        3
    );
    assert!(context.resource_usage().unwrap().resident_texture_bytes >= POOL_BYTES);
    let absent = Arc::new(MaterialSceneSource3D {
        snapshot,
        model,
        hdr: crate::three_d::hdr::HdrSource3D {
            revision: 2,
            ..scene.hdr.clone()
        },
        samples: 1,
        direct: None,
        observe: false,
        indirect: None,
    });
    viewport
        .set_material_source_for_test(absent, Viewport3DViewRevision::new(1), r025_view())
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    assert!(viewport.direct_proof_for_test(metrics.surface_id).is_none());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert!(context.resource_usage().unwrap().resident_texture_bytes < POOL_BYTES);
    viewport.on_unmount();
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

pub(super) fn r024_prepare(viewport: &mut Viewport3D, metrics: &CustomRenderTargetMetrics) {
    let model_calls = crate::three_d::mip_sampling::uv_model_invocations_for_test();
    viewport.prepare_custom_render(metrics);
    assert_eq!(
        crate::three_d::mip_sampling::uv_model_invocations_for_test(),
        model_calls,
        "production preparation invoked the offline GPU model"
    );
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::Committed,
        "actual material failure: {:?}; status {:?}",
        viewport.material_failure_for_test(metrics.surface_id),
        viewport.status_for(metrics.surface_id)
    );
}

#[test]
fn r024_actual_geometry_material_centroid_n1_n4_frame_color_and_receiver() {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R024: actual Device required");
    let adapter = renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("R024 captured Device {adapter:?}; wgpu30.0.2/Naga30.0.2\n"),
    )
    .unwrap();
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    for mapped in [false, true] {
        for n in [1, 4] {
            let mut vertices = super::three_d_geometry::r024_vertices();
            let xy = [
                [0.0, 35.0 / 256.0],
                [-1.0, 28.0 / 256.0],
                [1.0, 28.0 / 256.0],
            ];
            let normals = [[0.0, 0.0, 1.0], [0.6, 0.0, 0.8], [-0.6, 0.0, 0.8]];
            let tangents = [[1.0, 0.0, 0.0], [0.8, 0.0, -0.6], [0.8, 0.0, 0.6]];
            for i in 0..3 {
                vertices[i].position = point(0.5 + xy[i][0], -0.5 + xy[i][1], 0.0);
                vertices[i].uv0 = Some(
                    crate::three_d::TextureCoordinate2::try_new(
                        [0.5, 0.0, 1.0][i],
                        [1.0, 0.0, 0.0][i],
                    )
                    .unwrap(),
                );
                vertices[i].normal =
                    PbrUnitDirection3D::try_new(if mapped { normals[i] } else { [0.0, 0.0, 1.0] })
                        .unwrap();
                vertices[i].tangent =
                    mapped.then(|| (PbrUnitDirection3D::try_new(tangents[i]).unwrap(), 1.0));
                vertices[i].color =
                    Some(crate::three_d::LinearVertexColorRgba8::from_linear_unorm8(
                        if i == 0 { 255 } else { 0 },
                        if i == 0 { 255 } else { 0 },
                        if i == 0 { 255 } else { 0 },
                        255,
                    ));
            }
            let mut material = PbrMaterialSource3D::defaults(1, 1);
            material.emissive_rgb = [1.0; 3];
            material.normal_scale = 0.0;
            if mapped {
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
                material.roles[1] = Some(PbrRoleBinding3D::V1 {
                    texture: texture.handle().clone(),
                    sampler: sampler.handle().clone(),
                });
            }
            let source = r024_source(&context, vertices, material, n, true, r024_identity(), 1);
            let mut viewport = r024_viewport(context.clone(), source, r024_view(32.0));
            let metrics = active_metrics(701 + u64::from(mapped) * 10 + u64::from(n), 32, 32, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            if n == 1 {
                assert_eq!(pixel, [0.0; 4], "center lies outside the legal triangle");
                continue;
            }
            assert!(
                pixel[3] > 0.0 && pixel[3] < 1.0,
                "actual backend must demonstrate non-center partial coverage, not assume abstract S: {pixel:?}"
            );
            let o = r024_observation(&viewport, metrics.surface_id, &gpu);
            assert!(o.iter().flatten().all(|v| v.is_finite()));
            assert!(o[0][3] >= 0.75);
            if mapped {
                assert!(o[1][3] >= 3.0 / 32.0);
                assert!(o[9][3] >= 0.75);
                assert!(o[3][..3].iter().map(|v| v * v).sum::<f32>() > 0.99);
            }
            assert!(o[4].iter().all(|v| (0.0..=1.0).contains(v)));
            assert_eq!(o[4][3], 1.0);
            assert!(
                (o[10][0] - 0.5).abs() < 0.001 && (o[10][1] + 4.0).abs() < 0.001,
                "UV0 must remain center-evaluated even while other attributes use covered centroid: {:?}",
                o[10]
            );
            assert!(o[8][..3].iter().all(|v| *v >= 0.0));
            let y = o[5][1] + 0.5;
            let wa = (y - 28.0 / 256.0) / (7.0 / 256.0);
            let wc = (1.0 - wa + (o[5][0] - 0.5)) * 0.5;
            let wb = 1.0 - wa - wc;
            // f32 transform/interpolation tolerance, not a source or shader epsilon.
            assert!(
                [wa, wb, wc].iter().all(|v| *v >= -2e-4 && *v <= 1.0 + 2e-4),
                "receiver left source primitive: {:?}",
                o[5]
            );
            for c in 0..3 {
                assert!((pixel[c] - 0.5 * pixel[3]).abs() <= 1.0 / 1024.0);
            }
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R024 actual N4 mapped={mapped} center-outside FP16={pixel:?}; localN={:?} g={:?} color={:?} receiver={:?}\n",o[0],o[1],o[4],o[5])).unwrap();
        }
    }
}

#[test]
fn r024_actual_roles_opaque_mask_emission_and_warm_last_good() {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R024: actual Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    for n in [1, 4] {
        let mut material = PbrMaterialSource3D::defaults(1, 1);
        material.emissive_rgb = [1.0, 0.5, 0.25];
        material.metallic = 0.8;
        material.roughness = 0.4;
        for (role, pixel, format) in [
            (
                PbrRole3D::Base,
                [128, 128, 128, 0],
                crate::three_d::TextureFormat3D::Rgba8UnormSrgb,
            ),
            (
                PbrRole3D::MetallicRoughness,
                [7, 64, 192, 255],
                crate::three_d::TextureFormat3D::Rgba8Unorm,
            ),
            (
                PbrRole3D::Occlusion,
                [0, 250, 251, 255],
                crate::three_d::TextureFormat3D::Rgba8Unorm,
            ),
            (
                PbrRole3D::Emissive,
                [128, 128, 128, 1],
                crate::three_d::TextureFormat3D::Rgba8UnormSrgb,
            ),
        ] {
            let t = context
                .register_texture(
                    crate::three_d::Texture3D::try_new(1, 1, format, pixel.to_vec()).unwrap(),
                )
                .unwrap();
            let s = context
                .register_sampler(crate::three_d::Sampler3D::default())
                .unwrap();
            material.roles[role as usize] = Some(PbrRoleBinding3D::V1 {
                texture: t.handle().clone(),
                sampler: s.handle().clone(),
            });
        }
        let source = r024_source(
            &context,
            super::three_d_geometry::r024_vertices(),
            material.clone(),
            n,
            true,
            r024_identity(),
            1,
        );
        let mut viewport = r024_viewport(context.clone(), source.clone(), r024_view(2.0));
        let metrics = active_metrics(730 + n as u64, 33, 33, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let o = r024_observation(&viewport, metrics.surface_id, &gpu);
        let srgb = ((128.0f32 / 255.0 + 0.055) / 1.055).powf(2.4);
        for c in 0..3 {
            assert!((o[6][c] - srgb).abs() < 0.002);
        }
        assert_eq!(o[6][3], 0.0);
        assert_eq!(o[7][2], 0.0);
        assert!((o[7][0] - 0.8 * 192.0 / 255.0).abs() < 1e-5);
        assert!((o[7][1] - 0.4 * 64.0 / 255.0).abs() < 1e-5);
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert_eq!(pixel[3], 1.0);
        for c in 0..3 {
            let e = srgb * [1.0, 0.5, 0.25][c];
            assert!((pixel[c] - e / (1.0 + e)).abs() < 0.001);
        }
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        let usage = context.resource_usage().unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
        assert_eq!(context.resource_usage().unwrap(), usage);
        // A failed source certificate never replaces the original committed tuple.
        let bad = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                identity: 700,
                revision: 2,
                exposure_ev: 0.0,
                maximum_rgb: [0.0; 3],
            },
            samples: n,
            direct: None,
            observe: true,
            indirect: None,
        });
        assert!(
            viewport
                .set_material_source_for_test(bad, Viewport3DViewRevision::new(1), r024_view(2.0))
                .is_err()
        );
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &source
        ));
        material.alpha_mode = PbrAlphaMode3D::Mask;
        let mask = r024_source(
            &context,
            super::three_d_geometry::r024_vertices(),
            material,
            n,
            true,
            r024_identity(),
            2,
        );
        viewport
            .set_material_source_for_test(mask, Viewport3DViewRevision::new(1), r024_view(2.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            [0.0; 4]
        );
        viewport.prepare_custom_render(&inactive_metrics(730 + n as u64, 33, 33));
        assert!(
            viewport
                .target_texture_id_for_test(metrics.surface_id)
                .is_none()
        );
    }
}

#[test]
fn r024_production_entry_to_hdr_and_ordinary_target_bound_image() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R024: actual Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    for n in [1, 4] {
        for partial in [false, true] {
            let mut vertices = super::three_d_geometry::r024_vertices();
            if partial {
                for (v, [x, y]) in vertices.iter_mut().zip([
                    [0.0, 35.0 / 256.0],
                    [-1.0, 28.0 / 256.0],
                    [1.0, 28.0 / 256.0],
                ]) {
                    v.position = point(0.5 + x, -0.5 + y, 0.0);
                }
            }
            let mut material = PbrMaterialSource3D::defaults(1, 1);
            material.emissive_rgb = [0.8, 0.4, 0.2];
            let source = r024_source(&context, vertices, material, n, false, r024_identity(), 1);
            let viewport = r024_viewport(
                context.clone(),
                source.clone(),
                r024_view(if partial { 32.0 } else { 2.0 }),
            );
            // The existing Panel contributes one physical pixel on each side.
            // Qualify a 32x32 child target, independently of the 34x34 surface.
            let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (34, 34), 1.0)
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
                Constraints::tight(Size::new(34.0, 34.0)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0).unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert_eq!(
                viewport.status_for(surface).unwrap().state(),
                Viewport3DLifecycleState::Committed,
                "{:?}",
                viewport.material_failure_for_test(surface)
            );
            assert!(
                viewport.material_observation_for_test(surface).is_none(),
                "REAL production material fragment, no diagnostic substitute"
            );
            let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            assert_eq!(
                (
                    viewport.hdr_front_for_test(surface).width(),
                    viewport.hdr_front_for_test(surface).height()
                ),
                (32, 32)
            );
            let coverage = if partial {
                if n == 4 { 0.25 } else { 0.0 }
            } else {
                1.0
            };
            assert_eq!(final_pixel[3], coverage);
            for c in 0..3 {
                let e = [0.8, 0.4, 0.2][c];
                assert!((final_pixel[c] - coverage * e / (1.0 + e)).abs() < 0.001);
            }
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
            let composed = render_compiled_surface_center_pixel(&renderer, &compiled, surface);
            for c in 0..3 {
                let encoded = f32::from(composed[c]) / 255.0;
                let linear = if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                assert!(
                    (linear - (final_pixel[c] + 1.0 - coverage)).abs() < 0.02,
                    "unchanged premultiplied-linear Image composition: {composed:?}"
                );
            }
            assert_eq!(composed[3], 255);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R024 production entry N{n} partial={partial} -> R027 FP16={final_pixel:?} -> ordinary Image={composed:?}\n")).unwrap();
        }
    }
}

#[test]
fn r024_actual_authored_sampler_uv_orientation_and_linear_numeric_roles() {
    use crate::three_d::{
        Sampler3D, SamplerAddressMode as A, SamplerFilterMode as F, Texture3D, TextureCoordinate2,
        TextureFormat3D,
    };
    let renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R024 Device");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let pixels = vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let texture = context
        .register_texture(
            Texture3D::try_new(2, 2, TextureFormat3D::Rgba8UnormSrgb, pixels).unwrap(),
        )
        .unwrap();
    let cases = [
        (
            A::ClampToEdge,
            A::ClampToEdge,
            F::Nearest,
            F::Nearest,
            [0.25, 0.25],
            [1.0, 0.0, 0.0],
            false,
        ),
        (
            A::ClampToEdge,
            A::ClampToEdge,
            F::Nearest,
            F::Nearest,
            [0.25, 0.75],
            [0.0, 0.0, 1.0],
            false,
        ),
        (
            A::ClampToEdge,
            A::ClampToEdge,
            F::Nearest,
            F::Nearest,
            [1.25, 0.25],
            [0.0, 1.0, 0.0],
            false,
        ),
        (
            A::Repeat,
            A::Repeat,
            F::Nearest,
            F::Nearest,
            [1.25, 1.25],
            [1.0, 0.0, 0.0],
            false,
        ),
        (
            A::MirrorRepeat,
            A::MirrorRepeat,
            F::Nearest,
            F::Nearest,
            [1.25, 1.25],
            [1.0, 1.0, 1.0],
            false,
        ),
        (
            A::ClampToEdge,
            A::ClampToEdge,
            F::Linear,
            F::Nearest,
            [0.5, 0.5],
            [0.5; 3],
            false,
        ),
        (
            A::Repeat,
            A::Repeat,
            F::Nearest,
            F::Linear,
            [0.5, 0.5],
            [0.5; 3],
            true,
        ),
    ];
    for (u, v, mag, min, uv, expected, large_gradient) in cases {
        let s = context
            .register_sampler(Sampler3D::new(u, v, mag, min))
            .unwrap();
        let mut m = PbrMaterialSource3D::defaults(1, 1);
        m.roles[0] = Some(PbrRoleBinding3D::V1 {
            texture: texture.handle().clone(),
            sampler: s.handle().clone(),
        });
        let mut vertices = super::three_d_geometry::r024_vertices();
        for vertex in &mut vertices {
            vertex.uv0 = Some(
                TextureCoordinate2::try_new(
                    uv[0]
                        + if large_gradient {
                            vertex.position.x() * 264.0
                        } else {
                            0.0
                        },
                    uv[1]
                        + if large_gradient {
                            vertex.position.y() * 264.0
                        } else {
                            0.0
                        },
                )
                .unwrap(),
            );
        }
        let source = r024_source(&context, vertices, m, 4, true, r024_identity(), 1);
        let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
        let metrics = active_metrics(790, 33, 33, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let o = r024_observation(&viewport, metrics.surface_id, &gpu);
        for c in 0..3 {
            assert!(
                (o[6][c] - expected[c]).abs() < 0.002,
                "source sampler {u:?}/{v:?} MAG{mag:?} MIN{min:?}, top-left UV {uv:?}: {:?}",
                o[6]
            );
        }
    }
}

#[test]
fn r024_actual_local_first_dual_transform_and_full_backface_normal_flip() {
    use crate::three_d::{Sampler3D, Texture3D, TextureCoordinate2, TextureFormat3D};
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R024 Device");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    for (pixel, scale, back) in [
        ([255, 128, 255, 255], 1.0, false),
        ([128, 255, 255, 255], 1.0, false),
        ([128, 128, 128, 255], 1024.0, false),
        ([128, 128, 128, 255], -1024.0, false),
        ([255, 128, 255, 255], 1.0, true),
    ] {
        let t = context
            .register_texture(
                Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, pixel.to_vec()).unwrap(),
            )
            .unwrap();
        let s = context.register_sampler(Sampler3D::default()).unwrap();
        let mut m = PbrMaterialSource3D::defaults(1, 1);
        m.normal_scale = scale;
        m.double_sided = back;
        m.roles[1] = Some(PbrRoleBinding3D::V1 {
            texture: t.handle().clone(),
            sampler: s.handle().clone(),
        });
        let mut vertices = super::three_d_geometry::r024_vertices();
        if back {
            vertices.swap(1, 2);
            for v in &mut vertices {
                v.normal = PbrUnitDirection3D::try_new([0.0, 0.0, -1.0]).unwrap();
            }
        }
        let source = r024_source(&context, vertices, m, 4, true, r024_identity(), 1);
        let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
        let metrics = active_metrics(800, 33, 33, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let o = r024_observation(&viewport, metrics.surface_id, &gpu);
        let n = &o[2];
        assert!(n.iter().all(|v| v.is_finite()));
        assert!((n[..3].iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 2e-5);
        assert!(n[2] > 0.0);
        if pixel[0] == 255 {
            assert!(if back { n[0] < -0.7 } else { n[0] > 0.7 });
        } else if pixel[1] == 255 {
            assert!(n[1] > 0.7);
        } else {
            assert!(n[0] * scale > 0.0 && n[1] * scale > 0.0);
        }
    }
    let mut vertices = super::three_d_geometry::r024_vertices();
    let b = 2.0 * 2.0f64.sqrt() / 5.0;
    for i in 0..3 {
        let sign = if i == 1 { -1.0 } else { 1.0 };
        vertices[i].position = point([-1.0, 0.0, 1.0][i], [1.0, -1.0, 1.0][i], 0.0);
        vertices[i].normal = PbrUnitDirection3D::try_new([0.2, sign * b, 0.8]).unwrap();
        vertices[i].tangent = Some((
            PbrUnitDirection3D::try_new([0.8, -sign * b, 0.2]).unwrap(),
            1.0,
        ));
        vertices[i].uv0 =
            Some(TextureCoordinate2::try_new([0.0, 0.5, 1.0][i], [0.0, 1.0, 0.0][i]).unwrap());
    }
    let t = context
        .register_texture(
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![128, 128, 255, 255])
                .unwrap(),
        )
        .unwrap();
    let s = context.register_sampler(Sampler3D::default()).unwrap();
    let mut m = PbrMaterialSource3D::defaults(1, 1);
    m.normal_scale = 0.0;
    m.roles[1] = Some(PbrRoleBinding3D::V1 {
        texture: t.handle().clone(),
        sampler: s.handle().clone(),
    });
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 4.0).unwrap(),
    )
    .unwrap();
    let source = r024_source(&context, vertices, m.clone(), 4, true, placement, 1);
    let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
    let metrics = active_metrics(801, 33, 33, 1.0);
    r024_prepare(&mut viewport, &metrics);
    let o = r024_observation(&viewport, metrics.surface_id, &gpu);
    let expected = [12.0 / 17.0, 0.0, -3.0 / 17.0];
    for c in 0..3 {
        assert!((o[1][c] - expected[c]).abs() < 2e-5);
    }
    assert!((0..3).map(|c| o[2][c] * o[3][c]).sum::<f32>().abs() < 2e-5);
    assert!(o[1][3] > 0.7);
    // The admitted source cone boundary and maximum positive scale ratio execute
    // on the same controlled shader, not just the CPU admission path.
    let radial = 7.0f64.sqrt() / 4.0;
    let mut boundary = super::three_d_geometry::r024_vertices();
    for v in &mut boundary {
        v.normal = PbrUnitDirection3D::try_new([radial, 0.0, 0.75]).unwrap();
        v.tangent = Some((
            PbrUnitDirection3D::try_new([0.75, 0.0, -radial]).unwrap(),
            1.0,
        ));
    }
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 64.0).unwrap(),
    )
    .unwrap();
    let boundary = r024_source(&context, boundary, m, 4, true, placement, 2);
    viewport
        .set_material_source_for_test(boundary, Viewport3DViewRevision::new(1), r024_view(2.0))
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    let o = r024_observation(&viewport, metrics.surface_id, &gpu);
    assert!(o.iter().flatten().all(|v| v.is_finite()));
    assert_eq!(o[0][2], 0.75);
    assert_eq!(o[9][0], 0.75);
    assert!(o[1][3] >= 3.0 / 32.0);
    assert!((0..3).map(|c| o[2][c] * o[3][c]).sum::<f32>().abs() < 2e-5);
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "R024 actual N4 source cones=.75, scale ratio64: N={:?}, T={:?}, g={:?}\n",
            o[2], o[3], o[1]
        ),
    )
    .unwrap();

    // HAL-R024-S2-LOCAL-FIRST-GS-REAL-PRODUCTION-DISCRIMINATOR-01:
    // preserve all observer cases above. This separate, predeclared production
    // pair uses the exact A/B/A counterexample and observes only FP16/Image.
    drop(viewport);
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let mut production_vertices = super::three_d_geometry::r024_vertices();
    for (i, v) in production_vertices.iter_mut().enumerate() {
        let sign = if i == 1 { -1.0 } else { 1.0 };
        v.position = point([-1.0, 0.0, 1.0][i], [1.0, -1.0, 1.0][i], 0.0);
        v.normal = PbrUnitDirection3D::try_new([0.2, sign * b, 0.8]).unwrap();
        v.tangent = Some((
            PbrUnitDirection3D::try_new([0.8, -sign * b, 0.2]).unwrap(),
            1.0,
        ));
        v.uv0 = Some(TextureCoordinate2::try_new([0.0, 0.5, 1.0][i], [0.0, 1.0, 0.0][i]).unwrap());
        v.color = Some(crate::three_d::LinearVertexColorRgba8::OPAQUE_WHITE);
    }
    let geometry = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(production_vertices, GeometryIndices::Unindexed).unwrap(),
            None,
        )
        .unwrap();
    let production_placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 4.0).unwrap(),
    )
    .unwrap();
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    for (sample_index, n) in [1, 4].into_iter().enumerate() {
        let mut images = [[0u8; 4]; 2];
        for (case, normal_pixel) in [[128, 255, 255, 255], [128, 0, 255, 255]]
            .into_iter()
            .enumerate()
        {
            let label = if case == 0 { "POS" } else { "NEG" };
            let qx = 2.0 * 128.0 / 255.0 - 1.0f64;
            let qy = if case == 0 { 1.0 } else { -1.0 };
            let predicted_n_dot_l =
                (0.8 * qy + 0.6 * (1.0 - qx) / 2.0f64.sqrt()) / (qx * qx + 2.0).sqrt();
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 LOCAL GS PREDECLARED N{n} {label}: exact UNIT A/B/A source/H+; ideal center weights[.25,.5,.25], localg[12/17,0,-3/17], rejected world-first g=0 (ideal); modelscale[1,1,4]; normalRGBA{normal_pixel:?} scale1; L[0,.8,.6] E1; observe=false; ideal_source_predicted_n_dot_l={predicted_n_dot_l}; target33x33 pixel(16,16); POS FP16red>=1/64/ImageRed>0, NEG FP16/ImageRGB0; opaque alpha1/255\n",
            )).unwrap();
            let texture = context
                .register_texture(
                    Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, normal_pixel.to_vec())
                        .unwrap(),
                )
                .unwrap();
            let sampler = context.register_sampler(Sampler3D::default()).unwrap();
            let revision = 100 + sample_index as u64 * 2 + case as u64;
            let mut material = PbrMaterialSource3D::defaults(99001, revision);
            material.base_rgba = [0.5, 0.5, 0.5, 1.0];
            material.metallic = 0.0;
            material.roughness = 1.0;
            material.normal_scale = 1.0;
            material.emissive_rgb = [0.0; 3];
            material.alpha_mode = PbrAlphaMode3D::Opaque;
            material.double_sided = false;
            material.roles[1] = Some(PbrRoleBinding3D::V1 {
                texture: texture.handle().clone(),
                sampler: sampler.handle().clone(),
            });
            let material = context.register_pbr_material(material, None).unwrap();
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry: geometry.clone(),
                        material,
                        raster: RasterMode::SingleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![production_placement],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 99002,
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
            // r=1 gives a full ideal response bound<=2; E=1 and emission/E/G=0.
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 99003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 33;
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
                "R024 LOCAL GS PRODUCTION device={} backend={:?} N{n} {label}: observe=false exact A/B/A source, model={production_placement:?}, material/structural/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert_eq!(pixel[3], 1.0, "covered opaque local-GS N{n} {label}");
            assert!(pixel[..3].iter().all(|v| v.is_finite()));
            assert_eq!(image[3], 255, "opaque ordinary Image local-GS N{n} {label}");
            if case == 0 {
                assert!(pixel[0] >= 1.0 / 64.0, "preselected local-GS POS N{n}");
                assert!(image[0] > 0, "local-GS POS ordinary Image N{n}");
            } else {
                assert_eq!(pixel[..3], [0.0; 3], "local-GS NEG N.L branch N{n}");
                assert_eq!(image, [0, 0, 0, 255], "local-GS NEG ordinary Image N{n}");
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
            images[case] = image;
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
            images[0][0] > images[1][0],
            "preselected local-GS POS/NEG ordinary Image separation N{n}: {images:?}"
        );
    }

    // HAL-R024-S2-MAX64-REAL-PRODUCTION-FRAME-RESPONSE-01:
    // keep the scale4 production pair and ratio64 observer assertions above.
    // This independently predeclared pair changes only the proper model scale
    // and immutable source generations; FP16/Image remain the observations.
    let maximum_placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 64.0).unwrap(),
    )
    .unwrap();
    for (sample_index, n) in [1, 4].into_iter().enumerate() {
        let mut images = [[0u8; 4]; 2];
        for (case, normal_pixel) in [[128, 255, 255, 255], [128, 0, 255, 255]]
            .into_iter()
            .enumerate()
        {
            let label = if case == 0 { "POS" } else { "NEG" };
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R024 MAX64 PREDECLARED N{n} {label}: unchanged UNIT A/B/A source/H+/UV/white; ideal center weights[.25,.5,.25], localg[12/17,0,-3/17]; modelscale[1,1,64]; normalRGBA{normal_pixel:?} scale1; L[0,.8,.6] E1; observe=false; ideal source NdotV=.0413403825668, NdotL POS=.590487479625/NEG=-.540879020545, POS toned red~.0865573508 (source predictions, not GPU telemetry); target33x33 pixel(16,16); POS FP16red>=1/32/ImageRed>0, NEG FP16/ImageRGB0; opaque alpha1/255\n",
            )).unwrap();
            let texture = context
                .register_texture(
                    Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, normal_pixel.to_vec())
                        .unwrap(),
                )
                .unwrap();
            let sampler = context.register_sampler(Sampler3D::default()).unwrap();
            let revision = 104 + sample_index as u64 * 2 + case as u64;
            let mut material = PbrMaterialSource3D::defaults(99001, revision);
            material.base_rgba = [0.5, 0.5, 0.5, 1.0];
            material.metallic = 0.0;
            material.roughness = 1.0;
            material.normal_scale = 1.0;
            material.emissive_rgb = [0.0; 3];
            material.alpha_mode = PbrAlphaMode3D::Opaque;
            material.double_sided = false;
            material.roles[1] = Some(PbrRoleBinding3D::V1 {
                texture: texture.handle().clone(),
                sampler: sampler.handle().clone(),
            });
            let material = context.register_pbr_material(material, None).unwrap();
            let snapshot = context
                .admit_material_snapshot(
                    StructuralRevision::new(revision),
                    vec![MaterialPacketSource3D::PbrMaterial {
                        geometry: geometry.clone(),
                        material,
                        raster: RasterMode::SingleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_material_model(
                    snapshot.clone(),
                    ModelRevision::new(revision),
                    vec![maximum_placement],
                )
                .unwrap();
            let direct = Arc::new(DirectSource3D {
                identity: 99002,
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
            // The same r=1 full ideal response bound<=2, E=1 and no emission/E/G
            // retain the predeclared conservative complete HDR cap4.
            let source = Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 99003,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples: n,
                direct: Some(direct),
                observe: false,
                indirect: None,
            });
            let size = 33;
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
                "R024 MAX64 PRODUCTION device={} backend={:?} wgpu30.0.2/Naga30.0.2 N{n} {label}: observe=false unchanged A/B/A geometry; model={maximum_placement:?}, material/structural/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; registered ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert_eq!(pixel[3], 1.0, "covered opaque MAX64 N{n} {label}");
            assert!(pixel.iter().all(|v| v.is_finite()));
            assert_eq!(image[3], 255, "opaque ordinary Image MAX64 N{n} {label}");
            if case == 0 {
                assert!(
                    pixel[0] >= 1.0 / 32.0,
                    "preselected MAX64 POS N{n}: FP16={pixel:?} Image={image:?}"
                );
                assert!(image[0] > 0, "MAX64 POS ordinary Image N{n}");
            } else {
                assert_eq!(pixel[..3], [0.0; 3], "MAX64 NEG N.L branch N{n}");
                assert_eq!(image, [0, 0, 0, 255], "MAX64 NEG ordinary Image N{n}");
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
            images[case] = image;
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
            images[0][0] > images[1][0],
            "preselected MAX64 POS/NEG ordinary Image separation N{n}: {images:?}"
        );
    }
}

#[test]
fn r024_actual_mask_thresholds_clipping_and_warm_model_view_reuse() {
    let renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R024 Device");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    for n in [1, 4] {
        for (alpha, cutoff, coverage) in [(127, 0.5, 0.0), (128, 0.5, 1.0), (255, 2.0, 0.0)] {
            let mut vertices = super::three_d_geometry::r024_vertices();
            for v in &mut vertices {
                v.color = Some(crate::three_d::LinearVertexColorRgba8::from_linear_unorm8(
                    255, 255, 255, alpha,
                ));
            }
            let mut m = PbrMaterialSource3D::defaults(1, 1);
            m.emissive_rgb = [1.0; 3];
            m.alpha_mode = PbrAlphaMode3D::Mask;
            m.cutoff = cutoff;
            let source = r024_source(&context, vertices, m, n, false, r024_identity(), 1);
            let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
            let metrics = active_metrics(811, 33, 33, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            assert_eq!(
                pixel,
                [0.5 * coverage, 0.5 * coverage, 0.5 * coverage, coverage]
            );
        }
    }
    // This actual primitive crosses the clip-volume side planes. Center-UV
    // helpers and centroid position/color/frame still use the same fixed shader.
    let mut vertices = super::three_d_geometry::r024_vertices();
    for v in &mut vertices {
        v.position = point(v.position.x() * 4.0, v.position.y() * 4.0, v.position.z());
    }
    let mut m = PbrMaterialSource3D::defaults(1, 1);
    m.emissive_rgb = [1.0; 3];
    let source = r024_source(&context, vertices, m, 4, true, r024_identity(), 1);
    let mut viewport = r024_viewport(context.clone(), source.clone(), r024_view(2.0));
    let metrics = active_metrics(812, 33, 33, 1.0);
    r024_prepare(&mut viewport, &metrics);
    let o = r024_observation(&viewport, metrics.surface_id, &gpu);
    assert!(o.iter().flatten().all(|v| v.is_finite()));
    assert!(o[0][3] >= 0.75 && o[4].iter().all(|v| (0.0..=1.0).contains(v)));
    assert!(o[5][0].abs() < 0.001 && o[5][1].abs() < 0.001);
    let usage = context.resource_usage().unwrap();
    let state = context.dynamic_state_accounting_for_test().unwrap();
    let model = context
        .admit_material_model(
            source.snapshot.clone(),
            ModelRevision::new(2),
            vec![
                ModelPlacement3::try_new(
                    WorldPoint3::try_new(0.1, 0.0, 0.0).unwrap(),
                    Orientation3::identity(),
                    PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
    let next = Arc::new(MaterialSceneSource3D {
        snapshot: source.snapshot.clone(),
        model,
        hdr: crate::three_d::hdr::HdrSource3D {
            revision: 2,
            ..source.hdr.clone()
        },
        samples: 4,
        direct: None,
        observe: true,
        indirect: None,
    });
    viewport
        .set_material_source_for_test(next.clone(), Viewport3DViewRevision::new(1), r024_view(2.0))
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    assert_eq!(
        context.resource_usage().unwrap(),
        usage,
        "model-only uses the same resident geometry, roles, modules, pipelines"
    );
    let after = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(after.dynamic_storage_bytes, state.dynamic_storage_bytes);
    assert_eq!(after.dynamic_storage_slots, state.dynamic_storage_slots);
    assert_eq!(after.retained_cpu_bytes, state.retained_cpu_bytes);
    let moved = ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.2, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        crate::three_d::OrthographicProjection::try_new(2.0, 0.0, 10.0)
            .unwrap()
            .into(),
    );
    viewport
        .set_material_source_for_test(next, Viewport3DViewRevision::new(2), moved)
        .unwrap();
    r024_prepare(&mut viewport, &metrics);
    assert_eq!(context.resource_usage().unwrap(), usage);
    let old_id = viewport
        .target_texture_id_for_test(metrics.surface_id)
        .unwrap();
    let resized = active_metrics(812, 40, 35, 1.0);
    r024_prepare(&mut viewport, &resized);
    assert_ne!(
        viewport
            .target_texture_id_for_test(metrics.surface_id)
            .unwrap(),
        old_id
    );
    assert_eq!(viewport.hdr_front_for_test(metrics.surface_id).width(), 40);
}

#[test]
fn r024_actual_private_shared_pipeline_refusal_preserves_complete_old_front() {
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("UNVERIFIED R024 Device");
    let gpu = renderer.gpu_resources();
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 2;
    policy.max_pipeline_variants = 2;
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
    let initial_device_owners = Arc::strong_count(&gpu.device);
    let mut m = PbrMaterialSource3D::defaults(1, 1);
    m.emissive_rgb = [1.0; 3];
    let source = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        m.clone(),
        1,
        false,
        r024_identity(),
        1,
    );
    let mut viewport = r024_viewport(context.clone(), source.clone(), r024_view(2.0));
    let metrics = active_metrics(820, 33, 33, 1.0);
    r024_prepare(&mut viewport, &metrics);
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.admitted_shader_modules, 2);
    assert_eq!(usage.resident_pipeline_variants, 2);
    let legacy=context.register_appearance(AppearanceSourceDescriptor::try_new(r#"
fn halcyon_appearance(position_render_relative:vec3<f32>,normal_world_interpolated:vec3<f32>,
 view_direction_world:vec3<f32>,uv:vec2<f32>,vertex_color_linear:vec4<f32>,front_facing:bool,
 color_factor_linear:vec4<f32>,appearance_texture:texture_2d<f32>,appearance_sampler:sampler)->vec3<f32> {return vec3(1.0);}
"#,GeometrySemantics::NONE).unwrap()).unwrap();
    assert!(
        matches!(
            context.admit_appearance_for_test(legacy.handle(), GeometrySemantics::NONE),
            Err(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ShaderModules,
                requested: 4,
                maximum: 2
            })
        ),
        "legacy admission sees BOTH actual private PBR and retained R027 modules"
    );
    context.retire_appearance(legacy.handle()).unwrap();
    drop(legacy);
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 2);
    let id = viewport.target_texture_id_for_test(metrics.surface_id);
    let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
    m.double_sided = true;
    let candidate = r024_source(
        &context,
        super::three_d_geometry::r024_vertices(),
        m,
        1,
        false,
        r024_identity(),
        2,
    );
    let retire: Vec<_> = source
        .snapshot
        .source_packets()
        .iter()
        .chain(candidate.snapshot.source_packets())
        .map(|p| match p {
            MaterialPacketSource3D::PbrMaterial {
                geometry, material, ..
            } => (geometry.clone(), material.clone()),
            _ => unreachable!(),
        })
        .collect();
    viewport
        .set_material_source_for_test(candidate, Viewport3DViewRevision::new(1), r024_view(2.0))
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::FailedWithLastGood
    );
    assert!(matches!(
        viewport.material_failure_for_test(metrics.surface_id),
        Some(crate::three_d::pbr::PbrError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::PipelineVariants,
                requested: 3,
                maximum: 2
            }
        ))
    ));
    assert_eq!(viewport.target_texture_id_for_test(metrics.surface_id), id);
    assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
    assert!(Arc::ptr_eq(
        viewport
            .committed_material_source_for_test(metrics.surface_id)
            .unwrap(),
        &source
    ));
    assert_eq!(
        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
        [0.5, 0.5, 0.5, 1.0]
    );
    let serial = viewport
        .status_for(metrics.surface_id)
        .unwrap()
        .requested_material()
        .unwrap()
        .request_serial();
    viewport.prepare_custom_render(&metrics);
    assert_eq!(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .requested_material()
            .unwrap()
            .request_serial(),
        serial,
        "candidate's own rollback must not create a retry loop"
    );
    assert_eq!(
        context.resource_usage().unwrap().resident_pipeline_variants,
        2
    );
    for (g, m) in retire {
        context.retire_pbr_material(&m).unwrap();
        context.retire_pbr_geometry(&g).unwrap();
    }
    assert_eq!(
        context.resource_usage().unwrap().admitted_shader_modules,
        2,
        "real old-front/source custody still holds the modules"
    );
    drop(viewport);
    drop(source);
    assert_eq!(
        Arc::strong_count(&gpu.device),
        initial_device_owners + 1,
        "only the context's resident raster owner was added; pending source/resource completion cannot hold a strong Device callback cycle"
    );
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let retired = context.resource_usage().unwrap();
    assert_eq!(retired.admitted_shader_modules, 0);
    assert_eq!(retired.resident_pipeline_variants, 0);
    assert_eq!(retired.resident_geometry_bytes, 0);
}
