use super::*;

pub(super) fn r028_quad(uv_extent: [f32; 2], offset: [f32; 2]) -> Vec<PbrVertexSource3D> {
    [
        [-1.0, -1.0],
        [1.0, -1.0],
        [1.0, 1.0],
        [-1.0, -1.0],
        [1.0, 1.0],
        [-1.0, 1.0],
    ]
    .into_iter()
    .map(|[x, y]| PbrVertexSource3D {
        position: point(x, y, 0.0),
        normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
        uv0: Some(
            crate::three_d::TextureCoordinate2::try_new(
                offset[0] + (x + 1.0) * 0.5 * uv_extent[0],
                offset[1] + (1.0 - y) * 0.5 * uv_extent[1],
            )
            .unwrap(),
        ),
        color: None,
        tangent: Some((PbrUnitDirection3D::try_new([1.0, 0.0, 0.0]).unwrap(), -1.0)),
    })
    .collect()
}

pub(super) fn r028_role(
    context: &Context3D,
    chain: crate::three_d::mip_sampling::MipChainSource3D,
    id: u64,
    min: crate::three_d::mip_sampling::MipMin3D,
    mag: crate::three_d::SamplerFilterMode,
    u: crate::three_d::SamplerAddressMode,
    v: crate::three_d::SamplerAddressMode,
) -> (
    PbrRoleBinding3D,
    crate::three_d::mip_sampling::MipChainLease3D,
    crate::three_d::mip_sampling::MipSamplerLease3D,
) {
    let sampler = super::three_d_context::r028_fixture::sampler(&chain, id, min, mag, u, v);
    let th = context.register_mip_chain(chain, None).unwrap();
    let sh = context.register_mip_sampler(sampler, None).unwrap();
    let t = context.lease_mip_chain(&th).unwrap();
    let s = context.lease_mip_sampler(&sh).unwrap();
    (
        PbrRoleBinding3D::V2 {
            texture: th,
            sampler: sh,
        },
        t,
        s,
    )
}

pub(super) fn r028_observations(
    viewport: &Viewport3D,
    surface: SurfaceId,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> Vec<[[f32; 4]; 11]> {
    let o = viewport
        .material_observation_for_test(surface)
        .expect("actual V2 fragment source-stage values");
    let texture = viewport.hdr_front_for_test(surface);
    let size = u64::from(texture.width()) * u64::from(texture.height()) * 176;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("R028 predeclared source-stage ROI readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&o.values, 0, &buffer, 0, size);
    gpu.queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |v| send.send(v).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let data = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let result = data
        .chunks_exact(176)
        .map(bytemuck::pod_read_unaligned)
        .collect();
    drop(data);
    buffer.unmap();
    result
}

#[test]
fn r028_certified_negative_blue_metal_n1_n4() {
    use super::three_d_context::{r028_fixture as producer, r028_normal_fixture as normal};
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D};
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode as Address, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("R028 certified normal requires actual Metal Device");
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 certified normal actual {adapter:?}; wgpu30.0.2/Naga30.0.2; same sample_surface/frame/direct/HDR programs\n")).unwrap();
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
        indirect: None,
        observe: s.observe,
    };
    // Frozen32x32 target, center pixel(16,16), complete source/sampler for each
    // fixture, and fixed integer/fractional/MAG cases. A has genuinely negative
    // BLUE. The nonconstant fixture's raw A,-A hull contains0; only its REAL
    // shared-UV domain and its complete finite refinement prove nonzero.
    let cases = [
        (-2.0f32, [0.25, 0.25]),
        (0.0, [0.25, 0.25]),
        (0.5, [0.25, 0.25]),
        (1.0, [0.375, 0.375]),
    ];
    let unit = 2.0f64.powi(-24);
    let gamma = |n: f64| n * unit / (1.0 - n * unit);
    for n in [1, 4] {
        for correlated in [false, true] {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let chain = if correlated {
                normal::correlated(89401)
            } else {
                normal::constant(89400, 2, 2)
            };
            assert!(
                chain.validate().is_err(),
                "old uncaptured negative-BLUE route remains closed"
            );
            let policy = producer::sampler(
                &chain,
                89402,
                MipMin3D::LinearMipLinear,
                Linear,
                if correlated {
                    Address::ClampToEdge
                } else {
                    Address::Repeat
                },
                if correlated {
                    Address::ClampToEdge
                } else {
                    Address::MirrorRepeat
                },
            );
            let sh = context.register_mip_sampler(policy, None).unwrap();
            let certificate = if correlated {
                normal::correlated_certificate(&chain, &sh, policy)
            } else {
                normal::certificate(&chain, &sh, policy, 1.0, [1.0, 0.0, 0.0])
            };
            let ch = context
                .register_certified_normal_chain(
                    chain.clone(),
                    &sh,
                    1.0,
                    Arc::new(certificate),
                    None,
                )
                .unwrap();
            let held_chain = context.lease_mip_chain(&ch).unwrap();
            let held_sampler = context.lease_mip_sampler(&sh).unwrap();
            let proof = held_chain.normal_domain_summary().unwrap();
            assert_eq!(proof.cells, 25);
            assert!(proof.minimum_projection > 1.0 / 510.0);
            let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (34, 34), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
            let view = r025_view();
            let mut viewport: Option<Viewport3D> = None;
            let mut last_production = None;
            let mut last_pixel = [0.0; 4];
            let mut maximum_sample_error = 0.0f64;
            for (index, (lambda, uv)) in cases.into_iter().enumerate() {
                let span = 16.0 * 2.0f32.powf(lambda);
                let offset = [uv[0] + span * (16.5 / 32.0), uv[1] - span * (16.5 / 32.0)];
                let mut vertices = r028_quad([-span, span], offset);
                for vertex in &mut vertices {
                    // Old valid A frame: N.face=.8, T.Tuv=.8, N.T=0; negative
                    // UV-u makes its authored Tuv=-X and handedness+1 explicit.
                    vertex.normal = PbrUnitDirection3D::try_new([0.6, 0.0, 0.8]).unwrap();
                    vertex.tangent =
                        Some((PbrUnitDirection3D::try_new([-0.8, 0.0, 0.6]).unwrap(), 1.0));
                }
                let mut material = PbrMaterialSource3D::defaults(89403, index as u64 + 1);
                material.sampling = SamplingFamily3D::V2;
                material.base_rgba = [0.5, 0.5, 0.5, 1.0];
                material.metallic = 0.0;
                material.roughness = 1.0;
                material.normal_scale = 1.0;
                material.roles[1] = Some(PbrRoleBinding3D::V2 {
                    texture: ch.clone(),
                    sampler: sh.clone(),
                });
                let revision = 10 * index as u64 + 1;
                let source = r024_source(
                    &context,
                    vertices,
                    material,
                    n,
                    true,
                    r024_identity(),
                    revision,
                );
                if let Some(v) = &mut viewport {
                    v.set_material_source(source.clone(), Viewport3DViewRevision::new(1), view)
                        .unwrap();
                } else {
                    viewport = Some(r024_viewport(context.clone(), source.clone(), view));
                }
                let v = viewport.as_mut().unwrap();
                r024_prepare(v, &metrics);
                let rows = r028_observations(v, surface, &gpu);
                let o = rows[16 * 32 + 16];
                assert!(o.iter().flatten().all(|x| x.is_finite()));
                assert_eq!(o[10][3], 1.0);
                let max_uv = f64::from(span.abs() + offset[0].abs().max(offset[1].abs()));
                let uv_error = gamma(7.0) * max_uv.max(1.0) * 8.0;
                for k in 0..2 {
                    assert!((f64::from(o[10][k]) - f64::from(uv[k])).abs() <= uv_error);
                }
                let actual_dx = glam::DVec2::new(f64::from(o[9][0]), f64::from(o[9][1]));
                let actual_dy = glam::DVec2::new(f64::from(o[9][2]), f64::from(o[9][3]));
                assert!(
                    (actual_dx - glam::DVec2::new(-f64::from(span) / 32.0, 0.0)).length()
                        <= 2.0 * uv_error
                );
                assert!(
                    (actual_dy - glam::DVec2::new(0.0, f64::from(span) / 32.0)).length()
                        <= 2.0 * uv_error
                );
                let footprint = 2.0 * actual_dx.length().max(actual_dy.length());
                let observed_lod = footprint.log2();
                let actual_uv = [o[10][0], o[10][1]];
                let samples = sample_probe_for_test(
                    &context,
                    &held_chain,
                    &held_sampler,
                    &[[[actual_uv[0], actual_uv[1], 0.0, 1.0], o[9]]],
                )
                .unwrap();
                let ideal = super::three_d_raster::r028_reference_sample(
                    &chain,
                    policy,
                    actual_uv,
                    observed_lod as f32,
                );
                let expected_q = glam::DVec3::new(
                    2.0 * ideal[0] - 1.0,
                    2.0 * ideal[1] - 1.0,
                    2.0 * ideal[2] - 1.0,
                );
                let measured_q = glam::DVec3::new(
                    2.0 * f64::from(samples[0][0]) - 1.0,
                    2.0 * f64::from(samples[0][1]) - 1.0,
                    2.0 * f64::from(samples[0][2]) - 1.0,
                );
                // Observed source-stage error is NOT a producer certificate. The
                // independent full-domain certificate above already exists. This
                // pointwise Device observation plus IEEE operation bounds qualifies
                // the frozen test cases; it does not claim a universal Metal filter
                // precision limit or newly admitted public source range.
                let sampling_error = (measured_q - expected_q).length();
                maximum_sample_error = maximum_sample_error.max(sampling_error);
                let decode_error = gamma(3.0) * 3.0f64.sqrt() * 3.0;
                let epsilon = sampling_error + decode_error;
                assert!(
                    epsilon < proof.minimum_projection
                        && measured_q.length() > proof.minimum_projection - epsilon
                );
                let direction_error =
                    2.0 * epsilon / (proof.minimum_projection - epsilon) + gamma(64.0) * 16.0;
                let ns = glam::DVec3::new(0.6, 0.0, 0.8);
                let tangent = glam::DVec3::new(-0.8, 0.0, 0.6);
                let bitangent = ns.cross(tangent);
                let expected =
                    (tangent * expected_q.x + bitangent * expected_q.y + ns * expected_q.z)
                        .normalize();
                let observed =
                    glam::DVec3::new(f64::from(o[2][0]), f64::from(o[2][1]), f64::from(o[2][2]));
                assert!(
                    (observed - expected).length() <= direction_error,
                    "actual material normal{observed:?}, ideal{expected:?}, derived observed-case bound{direction_error}"
                );
                assert!(
                    (f64::from(o[10][2]) - measured_q.z).abs() <= decode_error,
                    "same production signed-Z source stage"
                );
                if index < 2 {
                    assert!(
                        o[10][2] < 0.0,
                        "the original BLUE127 was neither clamped nor dropped"
                    );
                }
                let mut direct = captured(&source);
                direct.hdr.revision += 1;
                direct.hdr.maximum_rgb = [4.0; 3];
                // r=1 has source-independent f*cos<=2, one explicit E=1 and no
                // emission/E/G. Bound4 covers ALL surviving source samples.
                direct.direct = Some(Arc::new(DirectSource3D {
                    identity: 89404,
                    revision,
                    meters_per_scene_unit: 1.0,
                    directional: Some(Directional3D {
                        surface_to_source: [0.0, 0.0, 1.0],
                        irradiance: [1.0; 3],
                        shadow_far_m: 8.0,
                    }),
                    points: Arc::from([]),
                    spots: Arc::from([]),
                    snapshot: direct.snapshot.clone(),
                    model: direct.model.clone(),
                }));
                let direct = Arc::new(direct);
                v.set_material_source(direct.clone(), Viewport3DViewRevision::new(1), view)
                    .unwrap();
                r024_prepare(v, &metrics);
                let d = r025_observation_at(v, surface, &gpu, 16, 16);
                let actual =
                    glam::DVec3::new(f64::from(d[1][0]), f64::from(d[1][1]), f64::from(d[1][2]));
                assert!((actual - expected).length() <= direction_error);
                assert_eq!(d[0], [0.0, 0.0, 1.0, 1.0]);
                assert_eq!(d[6][3], 1.0);
                assert!(
                    d[6][..3]
                        .iter()
                        .all(|c| c.is_finite() && *c > 0.0 && *c < 4.0),
                    "normal-dependent direct material must be lit"
                );
                let mut production = captured(&direct);
                production.observe = false;
                production.hdr.revision += 1;
                let production = Arc::new(production);
                v.set_material_source(production.clone(), Viewport3DViewRevision::new(1), view)
                    .unwrap();
                r024_prepare(v, &metrics);
                assert!(v.material_observation_for_test(surface).is_none());
                let pixel = read_r027_fp16(v.hdr_front_for_test(surface), &gpu);
                // A full binary16 spacing encloses either nearest or directed store
                // rounding at each HDR/final store; f32 tone adds its gamma bound.
                let spacing = |x: f64| {
                    if x == 0.0 {
                        2.0f64.powi(-24)
                    } else {
                        2.0f64
                            .powi(x.abs().log2().floor() as i32 - 10)
                            .max(2.0f64.powi(-24))
                    }
                };
                for k in 0..3 {
                    let rgb = f64::from(d[6][k]);
                    let tone = rgb / (1.0 + rgb);
                    let bound = spacing(rgb) + spacing(tone) + gamma(3.0);
                    assert!(
                        (f64::from(pixel[k]) - tone).abs() <= bound,
                        "FP16 source-derived bound{bound}: {pixel:?} from{:?}",
                        d[6]
                    );
                }
                assert_eq!(pixel[3], 1.0);
                let usage = context.resource_usage().unwrap();
                let swaps = v.hdr_swaps_for_test(surface);
                r024_prepare(v, &metrics);
                assert_eq!(usage, context.resource_usage().unwrap());
                assert_eq!(swaps, v.hdr_swaps_for_test(surface));
                std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 certified N{n} correlated{correlated} UV{actual_uv:?} nominalLOD{lambda} observedLOD{observed_lod} footprint{footprint}: sampler={policy:?}, scaleBits={:08x}, proof={proof:?}, idealQ={expected_q:?}, actualSampleQ={measured_q:?}, measuredSampleError={sampling_error}, decodedErrorBound={decode_error}, derivedNormalBound={direction_error}, mappedN={actual:?}, direct={:?}, FP16={pixel:?}\n",1.0f32.to_bits(),d[6])).unwrap();
                last_pixel = pixel;
                last_production = Some(production);
            }
            let mut viewport = viewport.unwrap();
            let production = last_production.unwrap();
            let front = viewport.target_texture_id_for_test(surface).unwrap();
            // Candidate source admission rejects mismatched normal scale BEFORE a
            // material/default publication; the current front remains wholly old.
            let before = context.resource_usage().unwrap();
            let mut bad_material = PbrMaterialSource3D::defaults(89499, 1);
            bad_material.sampling = SamplingFamily3D::V2;
            bad_material.normal_scale = 0.0;
            bad_material.roles[1] = Some(PbrRoleBinding3D::V2 {
                texture: ch.clone(),
                sampler: sh.clone(),
            });
            assert!(context.register_pbr_material(bad_material, None).is_err());
            assert_eq!(context.resource_usage().unwrap(), before);
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            let mut failed = captured(&production);
            failed.hdr.revision += 1;
            let bad_view = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(1.0e300, 0.0, 2.0).unwrap(),
                    Orientation3::identity(),
                ),
                crate::three_d::OrthographicProjection::try_new(2.0, 0.1, 10.0)
                    .unwrap()
                    .into(),
            );
            viewport
                .set_material_source(Arc::new(failed), Viewport3DViewRevision::new(2), bad_view)
                .unwrap();
            viewport.prepare_custom_render(&metrics);
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::FailedWithLastGood);
            assert!(Arc::ptr_eq(
                status.committed_material().unwrap().source(),
                &production
            ));
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            let resized = active_metrics_for_surface(surface, 34, 34, 1.0);
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
                Constraints::tight(Size::new(34.0, 34.0)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0).unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert_eq!(viewport.hdr_front_for_test(surface).width(), 32);
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
            let decode = |b: u8| {
                let x = f64::from(b) / 255.0;
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            for k in 0..3 {
                let low = decode(composed[k].saturating_sub(1));
                let high = decode(composed[k].saturating_add(1));
                assert!(
                    (low..=high).contains(&f64::from(last_pixel[k])),
                    "ordinary Image byte rounding interval [{low},{high}] from{composed:?}, FP16{last_pixel:?}"
                );
            }
            assert_eq!(composed[3], 255);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 certified N{n} correlated{correlated}: same production FP16={last_pixel:?} -> ordinary WHITE Image={composed:?}; max observed source error={maximum_sample_error}; warm stable, exact compatible original front retained, incompatible resize withdrawn\n")).unwrap();
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
    }
}

pub(super) fn consumer_pbr_geometry(context: &Context3D) -> crate::three_d::PbrGeometry3D {
    use crate::three_d as api;
    let id = api::PbrOriginalIdentity3D {
        source: 97000,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let positions = [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let uv = [
        [0.0, 0.0],
        [1.0, 0.0],
        [0.0, 1.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    context
        .admit_pbr_geometry(
            api::PbrGeometryDescriptor3D {
                original: api::PbrOriginalGeometry3D {
                    identity: id,
                    vertices: (0..6)
                        .map(|j| api::PbrOriginalVertex3D {
                            position: positions[j],
                            normal: [0.8, 0.0, 0.6],
                            uv0: Some(uv[j]),
                            color: Some([255; 4]),
                        })
                        .collect(),
                    indices: GeometryIndices::Unindexed,
                    signed_tangents: Arc::from([[-0.6, 0.0, 0.8, 1.0]; 6]),
                    tangent_provenance: api::PbrTangentProvenance3D::Authored,
                },
                render: (0..6)
                    .map(|j| api::PbrVertex3D {
                        position: positions[j],
                        normal: [0.8, 0.0, 0.6],
                        uv0: uv[j],
                        color: [255; 4],
                        tangent: [-0.6, 0.0, 0.8, 1.0],
                    })
                    .collect(),
                render_indices: GeometryIndices::Unindexed,
                render_vertex_sources: Arc::from([0, 1, 2, 3, 4, 5]),
                triangles: (0..2)
                    .map(|triangle| api::PbrTriangleCertificate3D {
                        original: id,
                        triangle,
                        certificate_identity: 97001 + triangle as u64,
                        certificate_revision: 1,
                        frame: api::PbrFrameCertificate3D::AuthoredSmooth {
                            separator: [0.0, -1.0, 0.0],
                            lower: [0.9; 9],
                        },
                    })
                    .collect(),
                mapped: true,
                maximum_certificate_bytes: 65536,
                maximum_verification_items: 1000,
            },
            None,
        )
        .unwrap()
}

pub(super) fn consumer_pbr_source(
    context: &Context3D,
    geometry: &crate::three_d::PbrGeometry3D,
    n: u32,
    negative: bool,
    revision: u64,
) -> crate::three_d::PbrScene3D {
    use crate::three_d as api;
    let pixel = if negative {
        [255, 128, 127, 255]
    } else {
        [128, 128, 255, 255]
    };
    let original: Arc<[u8]> = pixel.repeat(4).into();
    let mut bytes = original.to_vec();
    bytes.extend(pixel);
    let bytes: Arc<[u8]> = bytes.into();
    let input = Arc::new(api::PbrMipChainDescriptor3D {
        role: api::PbrMipRole3D::Normal,
        format: api::TextureFormat3D::Rgba8Unorm,
        levels: Arc::from([
            api::PbrMipLevel3D {
                width: 2,
                height: 2,
                offset: 0,
                bytes: 16,
            },
            api::PbrMipLevel3D {
                width: 1,
                height: 1,
                offset: 16,
                bytes: 4,
            },
        ]),
        provenance: api::PbrMipProvenance3D {
            image_identity: 97100 + u64::from(negative),
            content_revision: 1,
            producer_identity: 97110,
            producer_revision: 1,
            generator_identity: 97111,
            generator_revision: 1,
            generator: api::PbrMipGenerator3D::RecursiveStoredAreaR028V1,
            origin: api::PbrImageOrigin3D::TopLeft,
            uv_set: 0,
            work: 4,
            error_bound: 0.5,
            original,
            derived: Arc::downgrade(&bytes),
        },
        texels: bytes,
    });
    let policy = api::PbrMipSamplerDescriptor3D {
        identity: 97120,
        revision: 1,
        image_identity: input.provenance.image_identity,
        image_revision: 1,
        u: api::SamplerAddressMode::ClampToEdge,
        v: api::SamplerAddressMode::ClampToEdge,
        mag: api::SamplerFilterMode::Linear,
        min: api::PbrMipMin3D::LinearMipLinear,
        extent: [2, 2],
    };
    let sampler = context.admit_pbr_mip_sampler(policy, None).unwrap();
    let texture = if negative {
        context
            .admit_pbr_certified_normal_chain(
                input.clone(),
                &sampler,
                1.0,
                api::PbrNormalCertificate3D {
                    identity: 97130,
                    revision: 1,
                    source: input,
                    sampler: sampler.clone(),
                    policy,
                    normal_scale_bits: 1.0f32.to_bits(),
                    cells: 25,
                    maximum_work: 1_000_000,
                    maximum_metadata_bytes: 65536,
                    default: api::PbrNormalWitness3D {
                        direction: [1.0, 0.0, 0.0],
                        lower: 0.9,
                    },
                    exceptions: Arc::from([]),
                },
                None,
            )
            .unwrap()
    } else {
        context.admit_pbr_mip_chain(input, None).unwrap()
    };
    let mut material =
        api::PbrMaterialDescriptor3D::new(97200, revision, api::PbrSamplingFamily3D::V2);
    material.base_rgba = [0.5, 0.5, 0.5, 1.0];
    material.metallic = 0.0;
    material.roughness = 1.0;
    material.roles.normal = Some(api::PbrRoleBinding3D::V2 { texture, sampler });
    let material = context.admit_pbr_material(material, None).unwrap();
    let snapshot = context
        .admit_pbr_snapshot(
            StructuralRevision::new(revision),
            vec![api::PbrDrawPacket3D {
                geometry: geometry.clone(),
                material,
                raster: RasterMode::SingleSided,
                stable_ordinal: 0,
            }],
        )
        .unwrap();
    let model = context
        .admit_pbr_model(
            &snapshot,
            ModelRevision::new(revision),
            vec![r024_identity()],
        )
        .unwrap();
    let direct = api::PbrDirectSource3D::capture(
        api::PbrDirectDescriptor3D {
            identity: 97300,
            revision,
            meters_per_scene_unit: 1.0,
            directional: Some(api::PbrDirectionalLight3D {
                surface_to_source: [0.0, 0.0, 1.0],
                irradiance: [1.0; 3],
                shadow_far_m: 8.0,
            }),
            points: Arc::from([]),
            spots: Arc::from([]),
        },
        &snapshot,
        &model,
    )
    .unwrap();
    // r=1 bounds f*cos<=2 for every source shading normal; E=1, emission=G=0.
    context
        .admit_pbr_scene(api::PbrSceneDescriptor3D {
            snapshot,
            model,
            hdr: api::PbrHdrSource3D {
                identity: 97400,
                revision,
                exposure_ev: 0.0,
                maximum_rgb: [4.0; 3],
            },
            samples: SampleCount3D::try_new(n).unwrap(),
            direct: Some(direct),
            indirect: None,
        })
        .unwrap()
}

#[test]
fn pbr_warm_v2_object_intervals_exact_indexed_and_refusal() {
    use crate::three_d::mip_sampling::{
        ObjectRelativePacket3D, UvDomainContext3D, UvDomainTiming3D, capture_uv_object_intervals,
        original_helper_constant_bits_for_test, projection_dot_bits_for_test, qualify_uv_domain,
        qualify_uv_domain_with_objects, qualify_uv_helper_constants_control_for_test,
        qualify_uv_projection_control_for_test,
    };
    let coefficients = [
        0.0,
        -0.0,
        f32::from_bits(1),
        -f32::from_bits(1),
        f32::from_bits(0x007f_ffff),
        -f32::from_bits(0x007f_ffff),
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        1.0,
        -1.0,
        3.5,
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7fc1_2345),
        f32::from_bits(0xffc5_4321),
    ];
    let inputs = [
        [[0.0, 0.0], [-0.0, -0.0], [-0.0, 0.0], [1.0, 1.0]],
        [[1.0, 1.0], [-2.0, -2.0], [0.75, 0.75], [1.0, 1.0]],
        [[-1.0, 1.0], [0.0, 2.0], [-0.5, -0.25], [1.0, 1.0]],
        [
            [0.0, f64::from(f32::MIN_POSITIVE)],
            [-f64::from(f32::MIN_POSITIVE), 0.0],
            [f64::from_bits(1), f64::from_bits(1)],
            [1.0, 1.0],
        ],
        [[f64::from(f32::MAX); 2]; 4],
        [[f64::MAX; 2]; 4],
        [[f64::INFINITY; 2]; 4],
    ];
    for coefficient in coefficients {
        for row in [
            [coefficient; 4],
            [coefficient, 1.0, -1.0, -0.0],
            [0.0, -coefficient, coefficient, 1.0],
        ] {
            for input in inputs {
                let [original, scalar, prepared] = projection_dot_bits_for_test(row, input);
                assert_eq!(
                    scalar,
                    original,
                    "factored scalar row bits {:?}",
                    row.map(f32::to_bits)
                );
                assert_eq!(
                    prepared,
                    original,
                    "prepared row bits {:?}",
                    row.map(f32::to_bits)
                );
            }
        }
    }
    let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let context = Context3D::new(gpu).unwrap();
    let at = UvDomainContext3D {
        packet: 2,
        ordinal: 7,
        shadow_face: None,
    };
    let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
    for indices in [
        GeometryIndices::Unindexed,
        GeometryIndices::U16(vec![0, 1, 2, 0, 1, 2].into()),
        GeometryIndices::U32(vec![3, 4, 5, 0, 1, 2].into()),
        GeometryIndices::U32(vec![3, 4, 5, 3, 4, 5].into()),
    ] {
        let geometry = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(r028_quad([1.0; 2], [0.0; 2]), indices).unwrap(),
                None,
            )
            .unwrap();
        let geometry = context.lease_pbr_geometry(&geometry).unwrap();
        let mut memo = ObjectRelativePacket3D::try_new(&geometry, identity, at).unwrap();
        // An allocated but incomplete capture cannot replace any operation.
        let mut record = UvDomainTiming3D::default();
        let expected = qualify_uv_domain(
            &geometry.data.source,
            identity,
            identity,
            (192, 128),
            [1024; 2],
            at,
        );
        assert_eq!(
            qualify_uv_domain_with_objects(
                &geometry,
                identity,
                identity,
                (192, 128),
                [1024; 2],
                at,
                Some(&memo),
                Some(&mut record)
            ),
            expected
        );
        assert_eq!(record.object_dot_calls, 24);
        assert_eq!(record.object_corner_reuses, 0);
        assert_eq!(
            capture_uv_object_intervals(
                &geometry,
                identity,
                identity,
                (192, 128),
                [1024; 2],
                at,
                &mut memo,
                None
            ),
            expected
        );
        assert!(memo.is_complete());
        for extent in [(1, 1), (3, 7), (192, 128), (1024, 1024), (8192, 4096)] {
            let mut old = UvDomainTiming3D::default();
            let mut hoisted = UvDomainTiming3D::default();
            let reference = qualify_uv_helper_constants_control_for_test::<false>(
                &geometry,
                identity,
                identity,
                extent,
                [1024; 2],
                at,
                Some(&memo),
                Some(&mut old),
            );
            assert_eq!(
                qualify_uv_helper_constants_control_for_test::<true>(
                    &geometry,
                    identity,
                    identity,
                    extent,
                    [1024; 2],
                    at,
                    Some(&memo),
                    Some(&mut hoisted)
                ),
                reference
            );
            assert!(reference.is_ok());
            assert_eq!(old.counts, hoisted.counts);
            let bits = original_helper_constant_bits_for_test(extent);
            assert_eq!(hoisted.helper_constant_bits, Some(bits));
            if let Some(old_bits) = old.helper_constant_bits {
                assert_eq!(old_bits, bits);
            }
        }
        let mut outside = identity;
        outside[3][0] = 8.0;
        let mut old = UvDomainTiming3D::default();
        let mut hoisted = UvDomainTiming3D::default();
        let excluded = qualify_uv_helper_constants_control_for_test::<false>(
            &geometry,
            identity,
            outside,
            (192, 128),
            [1024; 2],
            at,
            Some(&memo),
            Some(&mut old),
        );
        assert_eq!(
            qualify_uv_helper_constants_control_for_test::<true>(
                &geometry,
                identity,
                outside,
                (192, 128),
                [1024; 2],
                at,
                Some(&memo),
                Some(&mut hoisted)
            ),
            excluded
        );
        assert_eq!(excluded.unwrap().excluded, 2);
        assert_eq!(old.counts, hoisted.counts);
        assert_eq!(old.counts[4], 0);
        assert_eq!(
            (old.helper_delta_divisions, old.helper_gamma7_evaluations),
            (0, 0)
        );
        assert_eq!(
            (
                hoisted.helper_delta_divisions,
                hoisted.helper_gamma7_evaluations
            ),
            (2, 1)
        );
        for coefficient in [
            0.0,
            -0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            f32::from_bits(0x007f_ffff),
            -f32::from_bits(0x007f_ffff),
        ] {
            let mut projection = identity;
            projection[3][0] = coefficient;
            let reference = qualify_uv_projection_control_for_test::<false>(
                &geometry,
                identity,
                projection,
                (192, 128),
                [1024; 2],
                at,
                Some(&memo),
                None,
            );
            assert_eq!(
                qualify_uv_projection_control_for_test::<true>(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    Some(&memo),
                    None
                ),
                reference
            );
            assert_eq!(
                qualify_uv_helper_constants_control_for_test::<false>(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    Some(&memo),
                    None
                ),
                reference
            );
        }
        for fov in [52.0_f32, 48.0, 44.0, 48.0] {
            let mut projection = identity;
            projection[0][0] = 1.0 / (0.5 * fov.to_radians()).tan();
            let mut record = UvDomainTiming3D::default();
            let full = qualify_uv_domain(
                &geometry.data.source,
                identity,
                projection,
                (192, 128),
                [1024; 2],
                at,
            );
            let mut original = ObjectRelativePacket3D::try_new(&geometry, identity, at).unwrap();
            assert_eq!(
                capture_uv_object_intervals(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    &mut original,
                    None
                ),
                full
            );
            assert!(memo.same_corner_bits_for_test(&original));
            assert_eq!(
                qualify_uv_domain_with_objects(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    Some(&memo),
                    Some(&mut record)
                ),
                full
            );
            assert!(full.is_ok());
            let mut old_helper = UvDomainTiming3D::default();
            assert_eq!(
                qualify_uv_helper_constants_control_for_test::<false>(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    Some(&memo),
                    Some(&mut old_helper)
                ),
                full
            );
            assert_eq!(record.counts, old_helper.counts);
            assert_eq!(old_helper.helper_constant_bits, record.helper_constant_bits);
            assert_eq!(
                record.helper_constant_bits,
                Some(original_helper_constant_bits_for_test((192, 128)))
            );
            assert_eq!(old_helper.counts[5], 2);
            assert_eq!(
                (
                    old_helper.helper_delta_divisions,
                    old_helper.helper_gamma7_evaluations
                ),
                (4, 2)
            );
            assert_eq!(
                (
                    record.helper_delta_divisions,
                    record.helper_gamma7_evaluations
                ),
                (2, 1)
            );
            let mut helper_samples = [[std::time::Duration::ZERO; 128]; 2];
            for iteration in 0..128 {
                for mode in [iteration & 1, 1 - (iteration & 1)] {
                    let input = std::hint::black_box(&geometry);
                    let projection = std::hint::black_box(projection);
                    let start = std::time::Instant::now();
                    let result = if mode == 0 {
                        qualify_uv_helper_constants_control_for_test::<false>(
                            input,
                            identity,
                            projection,
                            (192, 128),
                            [1024; 2],
                            at,
                            Some(&memo),
                            None,
                        )
                    } else {
                        qualify_uv_helper_constants_control_for_test::<true>(
                            input,
                            identity,
                            projection,
                            (192, 128),
                            [1024; 2],
                            at,
                            Some(&memo),
                            None,
                        )
                    };
                    helper_samples[mode][iteration] = start.elapsed();
                    assert_eq!(result, full);
                }
            }
            for (mode, mut values) in helper_samples.into_iter().enumerate() {
                values.sort_unstable();
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R028_HELPER_CONSTANT_CONTROL geometry={:?} fov={fov} mode={} calls=128 triangles_per_call=2 internal_clocks=false min_ns={} median_low_ns={} median_high_ns={} p95_ns={} max_ns={} sum_ns={} helper_delta_divisions={} helper_gamma7_evaluations={} summary_equal=true\n",
                    geometry.handle, if mode == 0 { "per_triangle" } else { "hoisted" },
                    values[0].as_nanos(), values[63].as_nanos(), values[64].as_nanos(),
                    values[121].as_nanos(), values[127].as_nanos(),
                    values.iter().copied().sum::<std::time::Duration>().as_nanos(),
                    if mode == 0 { 4 } else { 2 }, if mode == 0 { 2 } else { 1 })).unwrap();
            }
            let mut scalar_timing = UvDomainTiming3D::default();
            let scalar = qualify_uv_projection_control_for_test::<false>(
                &geometry,
                identity,
                projection,
                (192, 128),
                [1024; 2],
                at,
                Some(&memo),
                Some(&mut scalar_timing),
            );
            assert_eq!(scalar, full);
            assert_eq!(record.counts, scalar_timing.counts);
            assert_eq!(record.projection_coefficient_preparations, 16);
            assert_eq!(scalar_timing.projection_coefficient_preparations, 96);
            assert_eq!(scalar_timing.object_dot_calls, 0);
            // Each mode uses the same six original corners, object capture,
            // projection and qualifier. Only coefficient preparation differs.
            let mut samples = [[std::time::Duration::ZERO; 128]; 2];
            for iteration in 0..128 {
                for mode in [iteration & 1, 1 - (iteration & 1)] {
                    let input = std::hint::black_box(&geometry);
                    let projection = std::hint::black_box(projection);
                    let start = std::time::Instant::now();
                    let result = if mode == 0 {
                        qualify_uv_projection_control_for_test::<false>(
                            input,
                            identity,
                            projection,
                            (192, 128),
                            [1024; 2],
                            at,
                            Some(&memo),
                            None,
                        )
                    } else {
                        qualify_uv_projection_control_for_test::<true>(
                            input,
                            identity,
                            projection,
                            (192, 128),
                            [1024; 2],
                            at,
                            Some(&memo),
                            None,
                        )
                    };
                    samples[mode][iteration] = start.elapsed();
                    assert_eq!(result, full);
                }
            }
            for (mode, mut values) in samples.into_iter().enumerate() {
                values.sort_unstable();
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R028_PROJECTION_COEFFICIENT_CONTROL indexed={:?} fov={} mode={} calls=128 triangles_per_call=2 corners_per_call=6 internal_clocks=false min_ns={} median_low_ns={} median_high_ns={} p95_ns={} max_ns={} sum_ns={} coefficient_preparations={} summary_equal=true\n",
                    geometry.handle, fov, if mode == 0 { "scalar" } else { "prepared" },
                    values[0].as_nanos(), values[63].as_nanos(), values[64].as_nanos(),
                    values[121].as_nanos(), values[127].as_nanos(),
                    values.iter().copied().sum::<std::time::Duration>().as_nanos(),
                    if mode == 0 { 96 } else { 16 })).unwrap();
            }
            assert_eq!(
                (
                    record.object_dot_calls,
                    record.object_corner_reuses,
                    record.projection_dot_calls
                ),
                (0, 6, 24)
            );
        }
        for case in 0..6 {
            let mut object = identity;
            let mut projection = identity;
            let mut key = at;
            match case {
                0 => object[0][0] = f32::MAX,
                1 => projection[0][0] = f32::MAX,
                2 => object[3][0] = 0.25,
                3 => object[3][1] = -0.0,
                4 => key.ordinal += 1,
                _ => key.packet += 1,
            }
            let full = qualify_uv_domain(
                &geometry.data.source,
                object,
                projection,
                (192, 128),
                [1024; 2],
                key,
            );
            let mut record = UvDomainTiming3D::default();
            assert_eq!(
                qualify_uv_helper_constants_control_for_test::<false>(
                    &geometry,
                    object,
                    projection,
                    (192, 128),
                    [1024; 2],
                    key,
                    Some(&memo),
                    None
                ),
                full
            );
            assert_eq!(
                qualify_uv_projection_control_for_test::<false>(
                    &geometry,
                    object,
                    projection,
                    (192, 128),
                    [1024; 2],
                    key,
                    Some(&memo),
                    None
                ),
                full
            );
            assert_eq!(
                qualify_uv_domain_with_objects(
                    &geometry,
                    object,
                    projection,
                    (192, 128),
                    [1024; 2],
                    key,
                    Some(&memo),
                    Some(&mut record)
                ),
                full,
                "case{case} typed first failure or summary bits"
            );
            if case < 2 {
                assert!(full.is_err());
            }
            if case >= 2 {
                assert_eq!(record.object_corner_reuses, 0);
            }
        }
        let other = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(
                    r028_quad([1.0; 2], [0.0; 2]),
                    GeometryIndices::Unindexed,
                )
                .unwrap(),
                None,
            )
            .unwrap();
        let other = context.lease_pbr_geometry(&other).unwrap();
        let mut record = UvDomainTiming3D::default();
        let full = qualify_uv_domain(
            &other.data.source,
            identity,
            identity,
            (192, 128),
            [1024; 2],
            at,
        );
        assert_eq!(
            qualify_uv_domain_with_objects(
                &other,
                identity,
                identity,
                (192, 128),
                [1024; 2],
                at,
                Some(&memo),
                Some(&mut record)
            ),
            full
        );
        assert_eq!(record.object_corner_reuses, 0);
        for (extent, role_extent) in [((0, 128), [1024; 2]), ((192, 128), [0, 1024])] {
            let scalar = qualify_uv_projection_control_for_test::<false>(
                &geometry,
                identity,
                identity,
                extent,
                role_extent,
                at,
                Some(&memo),
                None,
            );
            let prepared = qualify_uv_projection_control_for_test::<true>(
                &geometry,
                identity,
                identity,
                extent,
                role_extent,
                at,
                Some(&memo),
                None,
            );
            assert_eq!(scalar, prepared);
            assert!(prepared.is_err());
            let mut before = UvDomainTiming3D::default();
            let mut after = UvDomainTiming3D::default();
            let reference = qualify_uv_helper_constants_control_for_test::<false>(
                &geometry,
                identity,
                identity,
                extent,
                role_extent,
                at,
                Some(&memo),
                Some(&mut before),
            );
            assert_eq!(
                qualify_uv_helper_constants_control_for_test::<true>(
                    &geometry,
                    identity,
                    identity,
                    extent,
                    role_extent,
                    at,
                    Some(&memo),
                    Some(&mut after)
                ),
                reference
            );
            assert!(reference.is_err());
            assert_eq!(before.helper_delta_divisions, 0);
            assert_eq!(after.helper_delta_divisions, 0);
        }
        // A failed capture never turns its partially computed corners into proof.
        let mut invalid = identity;
        invalid[0][0] = f32::MAX;
        assert!(
            capture_uv_object_intervals(
                &geometry,
                identity,
                invalid,
                (192, 128),
                [1024; 2],
                at,
                &mut memo,
                None
            )
            .is_err()
        );
        assert!(!memo.is_complete());
    }
    for index_kind in [16, 32] {
        for missing_uv in [true, false] {
            let mut vertices = r028_quad([1.0; 2], [0.0; 2]);
            let mut projection = identity;
            if missing_uv {
                vertices[5].uv0 = None;
            } else {
                for v in &mut vertices {
                    v.position = point(v.position.x() * 0.25, v.position.y() * 0.25, 0.0);
                }
                vertices[5].position = point(-2.0, 0.25, 0.0);
                projection[0][0] = f32::MAX;
            }
            let indices = if index_kind == 16 {
                GeometryIndices::U16(vec![0, 1, 2, 0, 2, 5].into())
            } else {
                GeometryIndices::U32(vec![0, 1, 2, 0, 2, 5].into())
            };
            let geometry = context
                .register_pbr_geometry(
                    PbrGeometrySource3D::try_new(vertices, indices).unwrap(),
                    None,
                )
                .unwrap();
            let geometry = context.lease_pbr_geometry(&geometry).unwrap();
            let reference = qualify_uv_helper_constants_control_for_test::<false>(
                &geometry,
                identity,
                projection,
                (192, 128),
                [1024; 2],
                at,
                None,
                None,
            );
            let actual = qualify_uv_helper_constants_control_for_test::<true>(
                &geometry,
                identity,
                projection,
                (192, 128),
                [1024; 2],
                at,
                None,
                None,
            );
            assert_eq!(actual, reference);
            assert_eq!(
                qualify_uv_projection_control_for_test::<false>(
                    &geometry,
                    identity,
                    projection,
                    (192, 128),
                    [1024; 2],
                    at,
                    None,
                    None
                ),
                actual
            );
            let Err(crate::three_d::pbr::PbrError3D::UvGradient(failure)) = actual else {
                panic!("expected indexed late typed refusal");
            };
            assert_eq!((failure.triangle, failure.corner), (Some(1), Some(2)));
            assert_eq!(
                failure.operation,
                if missing_uv {
                    "original UV0 available"
                } else {
                    "packed clip dot finite and within f32"
                }
            );
            assert_eq!(failure.axis, if missing_uv { None } else { Some(0) });
        }
    }
}

pub(super) fn assert_no_runtime_gpu_model(t: &crate::three_d::PbrPreparationTiming3D) {
    assert_eq!((t.vertices, t.main_triangles, t.shadow_triangles), (0, 0, 0));
    assert_eq!(
        (
            t.main_uv_packets,
            t.main_uv_qualified,
            t.main_uv_excluded,
            t.main_uv_conditioned
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(t.main_uv_timed_packets, 0);
    assert_eq!(t.main_uv_substage_counts, [0; 14]);
    assert_eq!(t.main_uv_substages, [std::time::Duration::ZERO; 9]);
    assert_eq!(
        (
            t.main_object_dot_calls,
            t.main_projection_dot_calls,
            t.main_object_corner_reuses
        ),
        (0, 0, 0)
    );
    assert_eq!(t.object_interval_bytes, 0);
    for duration in [
        t.vertex_projection,
        t.uv_main,
        t.uv_main_domain,
        t.uv_shadow,
        t.uv_shadow_domain,
        t.main_uv_substage_wrapper,
        t.main_object_control,
        t.object_population,
    ] {
        assert_eq!(duration, std::time::Duration::ZERO);
    }
}

#[test]
fn pbr_direct_warm_fov_no_gpu_model_or_retained_intervals_n1_n4() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    assert_eq!(
        renderer.headless_adapter_for_test().backend,
        wgpu::Backend::Metal
    );
    let gpu = renderer.gpu_resources();
    let view = |fov: f32| {
        ViewDescriptor3::new(
            r025_view().pose(),
            PerspectiveProjection::try_finite(fov.to_radians(), 0.1, 10.0)
                .unwrap()
                .into(),
        )
    };
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let source = r028_presence_scene(&context, n, 1);
        let mut viewport = r024_viewport(context.clone(), source.clone(), view(56.0));
        let metrics = active_metrics(82000 + u64::from(n), 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        viewport.set_pbr_timing_for_test(true);
        for (i, fov) in [52.0, 48.0, 44.0, 48.0].into_iter().enumerate() {
            viewport.set_pbr_timing_for_test(true);
            viewport
                .set_material_source(
                    source.clone(),
                    Viewport3DViewRevision::new(2 + i as u64),
                    view(fov),
                )
                .unwrap();
            r024_prepare(&mut viewport, &metrics);
            let t = viewport.pbr_timing_for_test().unwrap();
            assert!(t.complete);
            assert_no_runtime_gpu_model(&t.raster);
            assert_eq!(t.raster.shadow_faces, 3);
            if t.raster.capacity_reused {
                assert_eq!(t.raster.gpu_creations, [0; 5]);
            }
        }
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(source.clone(), Viewport3DViewRevision::new(6), view(40.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let t = viewport.pbr_timing_for_test().unwrap();
        assert!(t.raster.capacity_reused);
        assert_no_runtime_gpu_model(&t.raster);
        let bits =
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu).map(f32::to_bits);
        let proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        let mut fresh = r024_viewport(context.clone(), source.clone(), view(40.0));
        let other = active_metrics(82100 + u64::from(n), 32, 32, 1.0);
        r024_prepare(&mut fresh, &other);
        assert_eq!(
            bits,
            read_r027_fp16(fresh.hdr_front_for_test(other.surface_id), &gpu).map(f32::to_bits)
        );
        let fresh_proof = fresh.direct_proof_for_test(other.surface_id).unwrap();
        assert_eq!(proof.packed, fresh_proof.packed);
        assert_eq!(proof.witnesses, fresh_proof.witnesses);
        renderer.drain_pending_image_registrations();
        let image = |v: &Viewport3D, surface| {
            renderer.render_registered_image_pixel_for_test(
                v.target_texture_id_for_test(surface).unwrap(),
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            )
        };
        assert_eq!(
            image(&viewport, metrics.surface_id),
            image(&fresh, other.surface_id)
        );
        // A genuine placement/source change still needs fresh admission and shadows.
        let model = context
            .admit_material_model(
                source.snapshot.clone(),
                ModelRevision::new(2),
                vec![
                    ModelPlacement3::try_new(
                        WorldPoint3::try_new(0.125, 0.0, 0.0).unwrap(),
                        Orientation3::identity(),
                        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
        let d = source.direct.as_ref().unwrap();
        let changed = Arc::new(MaterialSceneSource3D {
            snapshot: source.snapshot.clone(),
            model: model.clone(),
            samples: n,
            observe: false,
            indirect: None,
            hdr: crate::three_d::hdr::HdrSource3D {
                revision: 2,
                ..source.hdr.clone()
            },
            direct: Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
                identity: d.identity,
                revision: 2,
                meters_per_scene_unit: d.meters_per_scene_unit,
                directional: d.directional.clone(),
                points: d.points.clone(),
                spots: d.spots.clone(),
                snapshot: source.snapshot.clone(),
                model,
            })),
        });
        viewport.set_pbr_timing_for_test(true);
        viewport
            .set_material_source(changed, Viewport3DViewRevision::new(7), view(44.0))
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let changed = viewport.pbr_timing_for_test().unwrap();
        assert_no_runtime_gpu_model(&changed.raster);
        viewport.on_unmount();
        fresh.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(
            context
                .dynamic_state_accounting_for_test()
                .unwrap()
                .retained_cpu_bytes,
            0
        );
    }
}

#[test]
fn pbr_unlinked_material_source_metal_n1_n4() {
    // Cargo.toml's existing self dev-dependency is a separate non-cfg(test)
    // library with test-harness enabled. Type-check the actual external API;
    // the closure is compilation proof, not a second renderer or GPU fixture.
    extern crate halcyon as external_halcyon;
    let _external_api: fn(
        &mut external_halcyon::three_d::Viewport3D,
    ) -> Option<external_halcyon::three_d::PbrViewTiming3D> = |viewport| {
        viewport.set_pbr_timing_for_test(true);
        let receipt = viewport.pbr_timing_for_test();
        if let Some(timing) = receipt {
            let _: external_halcyon::three_d::PbrPreparationTiming3D = timing.raster;
            let _: external_halcyon::three_d::HdrRenderTiming3D = timing.hdr;
            let _: bool = timing.complete;
        }
        viewport.set_pbr_timing_for_test(false);
        receipt
    };
    let _external_coarse_api: fn(
        &mut external_halcyon::three_d::Viewport3D,
    ) -> Option<std::time::Duration> = |viewport| {
        viewport.set_pbr_coarse_timing_for_test(true);
        let receipt = viewport.pbr_coarse_timing_for_test();
        viewport.set_pbr_coarse_timing_for_test(false);
        receipt.map(|timing| {
            let _: bool = timing.complete;
            let _: Option<external_halcyon::three_d::Viewport3DRequestSerial> =
                timing.request_serial;
            timing.raster_total
        })
    };
    // Use the exported test-harness surface, including all copied nested types.
    let enable_timing: fn(&mut Viewport3D, bool) = Viewport3D::set_pbr_timing_for_test;
    let timing_receipt: fn(&Viewport3D) -> Option<crate::three_d::PbrViewTiming3D> =
        Viewport3D::pbr_timing_for_test;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("public consumer proof requires real Metal");
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let geometry = consumer_pbr_geometry(&context);
        let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (34, 34), 1.0)
            .unwrap();
        let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
        let positive = consumer_pbr_source(&context, &geometry, n, false, 1);
        let negative = consumer_pbr_source(&context, &geometry, n, true, 2);
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            positive.clone(),
            Viewport3DViewRevision::new(1),
            r025_view(),
        )
        .unwrap();
        assert_eq!(timing_receipt(&viewport), None);
        r024_prepare(&mut viewport, &metrics);
        let positive_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        viewport
            .request_pbr_scene(
                negative.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let negative_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
        // Only the original normal role changes. The r=1 diffuse-dominant test
        // frame maps negative-BLUE +X toward +Z, giving more sourced direct light.
        // Finite radiance4 implies tone<=4/5; no image/look golden is used.
        for k in 0..3 {
            assert!(
                positive_pixel[k] > 0.0
                    && negative_pixel[k] > positive_pixel[k]
                    && negative_pixel[k] <= 0.8
            );
        }
        assert_eq!(positive_pixel[3], 1.0);
        assert_eq!(negative_pixel[3], 1.0);
        let status = viewport.status_for(surface).unwrap();
        assert!(status.requested().is_none() && status.committed().is_none());
        let committed = status.pbr_committed().unwrap();
        assert!(committed.source().same_identity(&negative));
        assert_eq!(committed.model_revision(), ModelRevision::new(2));
        assert_eq!(
            committed.target_key().color_format(),
            wgpu::TextureFormat::Rgba16Float
        );
        let front = viewport.target_texture_id_for_test(surface).unwrap();
        let audit = custom_render_audit::snapshot();
        let usage = context.resource_usage().unwrap();
        assert!(
            !viewport
                .request_pbr_scene(
                    negative.clone(),
                    Viewport3DViewRevision::new(1),
                    r025_view()
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
        assert!(
            viewport
                .request_pbr_scene(positive, Viewport3DViewRevision::new(1), r025_view())
                .is_err()
        );
        assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
        let bad_view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(1.0e300, 0.0, 2.0).unwrap(),
                Orientation3::identity(),
            ),
            crate::three_d::OrthographicProjection::try_new(2.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        viewport
            .request_pbr_scene(negative.clone(), Viewport3DViewRevision::new(2), bad_view)
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        assert_eq!(
            viewport.status_for(surface).unwrap().state(),
            Viewport3DLifecycleState::FailedWithLastGood
        );
        assert!(viewport.pbr_failure(surface).is_some());
        assert!(
            viewport
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap()
                .source()
                .same_identity(&negative)
        );
        assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
        let withdrawal =
            viewport.prepare_custom_render(&active_metrics_for_surface(surface, 34, 34, 1.0));
        assert!(
            withdrawal.front_publication_changed && withdrawal.presentation_binding().is_none()
        );
        assert!(
            viewport
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .is_none()
        );
        viewport
            .request_pbr_scene(
                negative.clone(),
                Viewport3DViewRevision::new(3),
                r025_view(),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        // HAL-PBR-VIEW-ONLY-ZOOM-STAGE-ATTRIBUTION-01. The existing immutable
        // six-vertex/two-triangle public B source is reused; this is not the
        // original external consumer furniture or its native 2840x1904 viewport workload.
        let zoom_view = |degrees: f32| {
            ViewDescriptor3::new(
                r025_view().pose(),
                crate::three_d::PerspectiveProjection::try_new(
                    degrees.to_radians(),
                    0.1,
                    crate::three_d::PerspectiveFar::Finite(10.0),
                )
                .unwrap()
                .into(),
            )
        };
        viewport
            .request_pbr_scene(
                negative.clone(),
                Viewport3DViewRevision::new(4),
                zoom_view(56.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let zoom_key = viewport
            .status_for(surface)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .target_key();
        let mut zoom_serial = viewport
            .status_for(surface)
            .unwrap()
            .pbr_committed()
            .unwrap()
            .request_serial();
        assert!(!negative.0.observe);
        {
            let control_view = viewport
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap()
                .view();
            let control_image = viewport.target_texture_id_for_test(surface);
            let control_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            use crate::three_d::mip_sampling::{
                UvDomainContext3D, UvDomainTiming3D, qualify_uv_domain,
                qualify_uv_domain_with_timing, uv_control_geometry_for_test,
            };
            let geometry = uv_control_geometry_for_test(&negative.0);
            let identity = glam::Mat4::IDENTITY.to_cols_array_2d();
            for shadow_face in [None, Some(0)] {
                for case in 0..3 {
                    let mut object = identity;
                    let mut projection = identity;
                    if case == 1 {
                        object[0][0] = f32::MAX;
                    } else if case == 2 {
                        projection[0][0] = f32::MAX;
                    }
                    let at = UvDomainContext3D {
                        packet: 0,
                        ordinal: 0,
                        shadow_face,
                    };
                    let off_start = std::time::Instant::now();
                    let off = qualify_uv_domain(geometry, object, projection, (32, 32), [2, 2], at);
                    let off_elapsed = off_start.elapsed();
                    let mut detail = UvDomainTiming3D::default();
                    let on_start = std::time::Instant::now();
                    let on = qualify_uv_domain_with_timing(
                        geometry,
                        object,
                        projection,
                        (32, 32),
                        [2, 2],
                        at,
                        Some(&mut detail),
                    );
                    let on_elapsed = on_start.elapsed();
                    assert_eq!(
                        off, on,
                        "timing changed summary bits or typed first refusal"
                    );
                    assert_eq!(on.is_ok(), case == 0, "packed-dot control case{case}");
                    if let Ok(summary) = on {
                        assert!(detail.complete);
                        detail.enclose(on_elapsed);
                        assert_eq!(detail.body() + detail.wrapper, on_elapsed);
                        let c = detail.counts;
                        assert_eq!(c[0], summary.qualified + summary.excluded);
                        assert_eq!(c[1], 3 * c[0]);
                        assert_eq!(c[2] + c[3], summary.qualified);
                        assert_eq!(c[4], summary.qualified);
                        assert!(c[5] <= c[4] && c[6] <= c[5]);
                        assert!(c[8] <= c[7] && c[7] <= c[6]);
                        assert!(c[10] <= c[9] && c[9] <= c[8]);
                        assert_eq!(c[11] + c[12], c[4]);
                        assert!(c[11] <= c[10]);
                        assert_eq!(c[13], c[0]);
                        let _ = std::io::Write::write_fmt(
                            &mut std::io::stderr(),
                            format_args!(
                                "R028_UV_FIXTURE_CONTROL N{n} shadow_face={shadow_face:?} off_ns={} on_ns={} stages={:?} wrapper_ns={} counts={c:?}\n",
                                off_elapsed.as_nanos(),
                                on_elapsed.as_nanos(),
                                detail.stages.map(|v| v.as_nanos()),
                                detail.wrapper.as_nanos(),
                            ),
                        );
                    } else {
                        assert!(!detail.complete);
                        let crate::three_d::pbr::PbrError3D::UvGradient(failure) = on.unwrap_err()
                        else {
                            panic!("expected attributed packed-dot refusal");
                        };
                        assert_eq!(
                            failure.operation,
                            if case == 1 {
                                "packed object dot finite and within f32"
                            } else {
                                "packed clip dot finite and within f32"
                            }
                        );
                    }
                }
            }
            let at = UvDomainContext3D {
                packet: 0,
                ordinal: 0,
                shadow_face: None,
            };
            let expected = qualify_uv_domain(geometry, identity, identity, (32, 32), [2, 2], at);
            let mut control = [[std::time::Duration::ZERO; 128]; 2];
            for enabled in [false, true] {
                for sample in &mut control[usize::from(enabled)] {
                    let mut record = UvDomainTiming3D::default();
                    let input = std::hint::black_box(geometry);
                    let clock = if enabled { Some(&mut record) } else { None };
                    let start = std::time::Instant::now();
                    let result = qualify_uv_domain_with_timing(
                        input,
                        identity,
                        identity,
                        (32, 32),
                        [2, 2],
                        at,
                        clock,
                    );
                    *sample = start.elapsed();
                    assert_eq!(result, expected);
                    assert_eq!(record.complete, enabled);
                    if enabled {
                        record.enclose(*sample);
                        assert_eq!(record.body() + record.wrapper, *sample);
                    }
                    std::hint::black_box(record);
                }
            }
            let unchanged = viewport.status_for(surface).unwrap();
            assert_eq!(unchanged.state(), Viewport3DLifecycleState::Committed);
            let unchanged = unchanged.pbr_committed().unwrap();
            assert!(unchanged.source().same_identity(&negative));
            assert_eq!(unchanged.request_serial(), zoom_serial);
            assert_eq!(unchanged.target_key(), zoom_key);
            assert_eq!(unchanged.view_revision(), Viewport3DViewRevision::new(4));
            assert_eq!(unchanged.view(), control_view);
            assert_eq!(viewport.target_texture_id_for_test(surface), control_image);
            assert_eq!(
                read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu).map(f32::to_bits),
                control_pixel.map(f32::to_bits)
            );
            for (enabled, samples) in control.into_iter().enumerate() {
                let mut ordered = samples;
                ordered.sort_unstable();
                let _ = std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!(
                        "R028_UV_FIXTURE_SAMPLES N{n} mode={} repetitions=128 min_ns={} median_low_ns={} median_high_ns={} p95_ns={} max_ns={} total_ns={} identical_results=true current_front_unchanged=true samples_ns={:?}\n",
                        if enabled == 0 { "off" } else { "on" },
                        ordered[0].as_nanos(),
                        ordered[63].as_nanos(),
                        ordered[64].as_nanos(),
                        ordered[121].as_nanos(),
                        ordered[127].as_nanos(),
                        samples.into_iter().sum::<std::time::Duration>().as_nanos(),
                        samples.map(|sample| sample.as_nanos()),
                    ),
                );
            }
        }
        for (step, fov) in [52.0f32, 48.0, 44.0, 48.0].into_iter().enumerate() {
            let revision = Viewport3DViewRevision::new(5 + step as u64);
            let view = zoom_view(fov);
            enable_timing(&mut viewport, true);
            assert_eq!(timing_receipt(&viewport), Some(Default::default()));
            let start = std::time::Instant::now();
            let request_start = std::time::Instant::now();
            assert!(
                viewport
                    .request_pbr_scene(negative.clone(), revision, view)
                    .unwrap()
            );
            let request_wall = request_start.elapsed();
            let prepare_start = std::time::Instant::now();
            let outcome = viewport.prepare_custom_render(&metrics);
            let prepare_wall = prepare_start.elapsed();
            let total = start.elapsed(); // Stops BEFORE any Device wait/readback.
            assert!(outcome.front_publication_changed);
            let timing = timing_receipt(&viewport).unwrap();
            let _: crate::three_d::PbrPreparationTiming3D = timing.raster;
            let _: crate::three_d::HdrRenderTiming3D = timing.hdr;
            assert!(timing.complete && timing.hdr.complete);
            assert_eq!(timing.raster.attempts, 1);
            assert!(timing.raster.same_snapshot && timing.raster.view_changed);
            assert!(timing.raster.direct_present && !timing.raster.direct_reused);
            assert!(timing.raster.direct_source_qualification_reused);
            let direct = timing.raster;
            let internal = direct.direct_source_validation
                + direct.direct_source_basis
                + direct.direct_receiver_clipping
                + direct
                    .direct_cascade_fit
                    .into_iter()
                    .sum::<std::time::Duration>()
                + direct
                    .direct_depth_certification
                    .into_iter()
                    .sum::<std::time::Duration>()
                + direct
                    .direct_xy_pcf
                    .into_iter()
                    .sum::<std::time::Duration>()
                + direct.direct_light_other;
            assert!(direct.direct_projection_partition_complete);
            assert_eq!(direct.direct_projection_body, internal);
            assert_eq!(
                direct.direct_projection,
                internal + direct.direct_projection_wrapper
            );
            assert_eq!(direct.direct_source_triangles, 2);
            assert_eq!(direct.direct_receiver_triangles, 2);
            assert_eq!(direct.direct_cascades, 3);
            assert!(direct.direct_source_validation_reused);
            assert!((3..=75).contains(&direct.direct_candidate_attempts));
            assert!((24..=600).contains(&direct.direct_frustum_corner_tests));
            assert!(
                direct.direct_fast_triangle_visits + direct.direct_fallback_triangle_visits
                    <= direct.direct_candidate_attempts * direct.direct_source_triangles as u64,
                "only inconclusive groups require individual triangle predicates"
            );
            assert_no_runtime_gpu_model(&timing.raster);
            assert_eq!(timing.raster.branch, "New");
            assert!(timing.hdr_reused);
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            let tuple = status.pbr_committed().unwrap();
            assert!(tuple.source().same_identity(&negative));
            assert_eq!(tuple.structural_revision(), StructuralRevision::new(2));
            assert_eq!(tuple.model_revision(), ModelRevision::new(2));
            assert_eq!(tuple.view_revision(), revision);
            assert_eq!(tuple.view(), view);
            assert_eq!(tuple.target_key(), zoom_key);
            assert_eq!(
                tuple.target_key().color_format(),
                wgpu::TextureFormat::Rgba16Float
            );
            let serial = tuple.request_serial();
            assert_ne!(serial, zoom_serial);
            zoom_serial = serial;
            let image_id = viewport.target_texture_id_for_test(surface).unwrap();
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            assert!(pixel.iter().all(|v| v.is_finite() && *v >= 0.0));
            assert_eq!(pixel[3], 1.0);
            // A fresh participant has no geometry basis to reuse. Its own source
            // admission still precedes identical production FP16 and Image output.
            renderer.drain_pending_image_registrations();
            let cached_image = renderer.render_registered_image_pixel_for_test(
                image_id,
                [16.5 / 32.0; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            // Compare settled lifecycle boundaries on both sides. The Image
            // readback submits work too and can finish earlier front retirement.
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let charges_before_reference = context.target_charges_for_test();
            let source_checks = MaterialSceneSource3D::full_validation_count_for_test();
            let direct_checks =
                crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test();
            let mut reference =
                Viewport3D::new_pbr(context.clone(), negative.clone(), revision, view).unwrap();
            assert_eq!(
                MaterialSceneSource3D::full_validation_count_for_test(),
                source_checks + 1
            );
            let admitted_checks =
                crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test();
            assert_eq!(admitted_checks.0, direct_checks.0 + 1);
            enable_timing(&mut reference, true);
            r024_prepare(&mut reference, &metrics);
            assert_eq!(
                MaterialSceneSource3D::full_validation_count_for_test(),
                source_checks + 1
            );
            assert_eq!(
                crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test(),
                admitted_checks
            );
            let reference_timing = timing_receipt(&reference).unwrap();
            assert!(reference_timing.complete && reference_timing.hdr.complete);
            assert!(!reference_timing.raster.direct_source_qualification_reused);
            assert!(reference_timing.raster.direct_source_validation_reused);
            let reference_tuple = reference
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap();
            assert!(reference_tuple.source().same_identity(&negative));
            assert_eq!(reference_tuple.view(), view);
            assert_eq!(reference_tuple.view_revision(), revision);
            let reference_key = reference_tuple.target_key();
            assert_ne!(
                reference_key, zoom_key,
                "separate participants own separate keys"
            );
            assert_eq!(reference_key.surface_id(), zoom_key.surface_id());
            assert_eq!(
                reference_key.physical_render_extent(),
                zoom_key.physical_render_extent()
            );
            assert_eq!(reference_key.color_format(), zoom_key.color_format());
            assert_eq!(reference_key.sample_count(), zoom_key.sample_count());
            let reference_committed = (
                reference_tuple.request_serial(),
                reference_tuple.view(),
                reference_tuple.view_revision(),
                reference_tuple.target_key(),
            );
            let reference_pixel = read_r027_fp16(reference.hdr_front_for_test(surface), &gpu);
            assert_eq!(pixel.map(f32::to_bits), reference_pixel.map(f32::to_bits));
            renderer.drain_pending_image_registrations();
            let reference_image = renderer.render_registered_image_pixel_for_test(
                reference.target_texture_id_for_test(surface).unwrap(),
                [16.5 / 32.0; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            assert_eq!(cached_image, reference_image);
            // Source reuse must not admit a new camera that violates R025 near
            // coverage, and refusal must retain the original complete front.
            let invalid_view = ViewDescriptor3::new(
                view.pose(),
                crate::three_d::OrthographicProjection::try_new(2.0, 0.0, 10.0)
                    .unwrap()
                    .into(),
            );
            let expected_error = negative
                .0
                .direct
                .as_ref()
                .unwrap()
                .prepare_frame(&negative.0, invalid_view, view.pose().translation(), 32, 32)
                .err()
                .expect("directional near-zero is a real coverage refusal");
            enable_timing(&mut reference, true);
            assert!(
                reference
                    .request_pbr_scene(
                        negative.clone(),
                        revision.checked_next().unwrap(),
                        invalid_view,
                    )
                    .unwrap()
            );
            reference.prepare_custom_render(&metrics);
            assert_eq!(
                reference.material_failure_for_test(surface),
                Some(&expected_error)
            );
            assert!(!timing_receipt(&reference).unwrap().complete);
            let retained = reference
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap();
            assert_eq!(retained.request_serial(), reference_committed.0);
            assert_eq!(retained.view(), reference_committed.1);
            assert_eq!(retained.view_revision(), reference_committed.2);
            assert_eq!(retained.target_key(), reference_committed.3);
            assert!(retained.source().same_identity(&negative));
            assert_eq!(
                read_r027_fp16(reference.hdr_front_for_test(surface), &gpu).map(f32::to_bits),
                reference_pixel.map(f32::to_bits)
            );
            renderer.drain_pending_image_registrations();
            assert_eq!(
                renderer.render_registered_image_pixel_for_test(
                    reference.target_texture_id_for_test(surface).unwrap(),
                    [16.5 / 32.0; 2],
                    [1.0; 4],
                    wgpu::Color::WHITE,
                ),
                reference_image
            );
            drop(reference);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), charges_before_reference);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "PBR RETAINED DIRECT SOURCE {adapter:?} N{n} view={revision:?}: source_basis_reused=true fresh_source_basis_reused=false FP16={pixel:?} fresh_FP16={reference_pixel:?} Image={cached_image:?} fresh_Image={reference_image:?}; exact outputs, current source/view/target/serial and reference retirement checked\n",
            )).unwrap();
            let remainder = |parent: std::time::Duration, children: &[std::time::Duration]| {
                parent
                    .checked_sub(children.iter().copied().sum())
                    .expect("nonoverlapping nested timing")
            };
            let r = timing.raster;
            let exclusive = [
                ("set_validation", timing.request_validation),
                (
                    "request_other",
                    remainder(request_wall, &[timing.request_validation]),
                ),
                ("prepare_validation", timing.prepare_validation),
                ("frame_validation", r.frame_validation),
                ("vertex_projection", r.vertex_projection),
                (
                    "frame_other",
                    remainder(r.frame_total, &[r.frame_validation, r.vertex_projection]),
                ),
                ("main_uv_domain", r.uv_main_domain),
                ("main_uv_other", remainder(r.uv_main, &[r.uv_main_domain])),
                ("direct_projection", r.direct_projection),
                ("shadow_uv_domain", r.uv_shadow_domain),
                (
                    "shadow_uv_other",
                    remainder(r.uv_shadow, &[r.uv_shadow_domain]),
                ),
                (
                    "direct_other",
                    remainder(r.direct_total, &[r.direct_projection, r.uv_shadow]),
                ),
                (
                    "raster_other",
                    remainder(r.total, &[r.frame_total, r.uv_main, r.direct_total]),
                ),
                ("hdr_allocation", timing.hdr_allocation),
                ("hdr_encode", timing.hdr.encode_cpu),
                ("hdr_submit_swap_registration", timing.hdr.submit_swap_cpu),
                (
                    "hdr_other",
                    remainder(
                        timing.hdr_total,
                        &[timing.hdr.encode_cpu, timing.hdr.submit_swap_cpu],
                    ),
                ),
                ("material_commit", timing.material_commit),
                ("front_tuple_publication", timing.publication),
                (
                    "prepare_other",
                    remainder(
                        prepare_wall,
                        &[
                            timing.prepare_validation,
                            r.total,
                            timing.hdr_allocation,
                            timing.hdr_total,
                            timing.material_commit,
                            timing.publication,
                        ],
                    ),
                ),
                (
                    "request_prepare_handoff",
                    remainder(total, &[request_wall, prepare_wall]),
                ),
            ];
            assert_eq!(
                exclusive
                    .iter()
                    .map(|(_, d)| *d)
                    .sum::<std::time::Duration>(),
                total
            );
            let audit = custom_render_audit::snapshot();
            assert!(
                !viewport
                    .prepare_custom_render(&metrics)
                    .front_publication_changed
            );
            assert_eq!(custom_render_audit::snapshot(), audit);
            assert_eq!(timing_receipt(&viewport), Some(timing));
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(image_id));
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "PBR VIEW-ONLY STAGES {adapter:?} N{n} fixture=publicBquad6vertices2triangles/normal2x2 target32x32 view_revision={revision:?} FOVdeg={fov} serial={serial:?} source_structural=2/model=2 image_id={image_id} FP16={pixel:?} observe=false branch={} direct_present={} same_snapshot={} view_changed={} direct_reused={} hdr_reused={} vertices={} main_triangles={} shadow_triangles={} shadow_faces={} uniform_buffers={} geometry_plans={} texture_plans={} mip_plans={} request_ms={:.6} active_prepare_ms={:.6} frame_total_ms={:.6} raster_total_ms={:.6} direct_total_ms={:.6} total_request_to_committed_ms={:.6} exclusive_ms={:?}; CPU wall only; submit/swap/queued registration is not GPU completion or native display; FP16 readback excluded; unchanged prepare=0\n",
                r.branch, r.direct_present, r.same_snapshot, r.view_changed, r.direct_reused, timing.hdr_reused,
                r.vertices, r.main_triangles, r.shadow_triangles, r.shadow_faces, r.uniform_buffers, r.geometry_plans, r.texture_plans, r.mip_plans,
                timing.request_total.as_secs_f64()*1000.0, timing.active_total.as_secs_f64()*1000.0,
                r.frame_total.as_secs_f64()*1000.0, r.total.as_secs_f64()*1000.0, r.direct_total.as_secs_f64()*1000.0, total.as_secs_f64()*1000.0,
                exclusive.map(|(name, d)| (name, d.as_secs_f64()*1000.0)),
            )).unwrap();
        }
        {
            use crate::three_d::pbr::MaterialSceneSource3D;
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let charges_before = context.target_charges_for_test();
            let source_checks = MaterialSceneSource3D::full_validation_count_for_test();
            let direct_checks =
                crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test();
            let mut control = Viewport3D::new_pbr(
                context.clone(),
                negative.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
            enable_timing(&mut control, true);
            r024_prepare(&mut control, &metrics);
            assert_eq!(
                MaterialSceneSource3D::full_validation_count_for_test(),
                source_checks + 1
            );
            let admitted_checks =
                crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test();
            assert_eq!(admitted_checks.0, direct_checks.0 + 1);
            let cold = timing_receipt(&control).unwrap();
            assert!(cold.complete && cold.hdr.complete);
            assert!(cold.raster.direct_source_validation_reused);
            assert!(!cold.raster.direct_source_qualification_reused);
            assert_no_runtime_gpu_model(&cold.raster);
            let control_image = control.target_texture_id_for_test(surface).unwrap();
            let control_serial = control
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap()
                .request_serial();
            let control_pixel = read_r027_fp16(control.hdr_front_for_test(surface), &gpu);
            renderer.drain_pending_image_registrations();
            let control_rgba = renderer.render_registered_image_pixel_for_test(
                control_image,
                [16.5 / 32.0; 2],
                [1.0; 4],
                wgpu::Color::WHITE,
            );
            let reheader = |hdr| {
                crate::three_d::PbrScene3D(Arc::new(MaterialSceneSource3D {
                    snapshot: negative.0.snapshot.clone(),
                    model: negative.0.model.clone(),
                    hdr,
                    samples: negative.0.samples,
                    direct: negative.0.direct.clone(),
                    indirect: negative.0.indirect.clone(),
                    observe: false,
                }))
            };
            let mut invalid_hdr = negative.0.hdr.clone();
            invalid_hdr.maximum_rgb[1] = 0.0;
            let invalid = reheader(invalid_hdr);
            let expected: crate::three_d::PbrSourceError3D = negative
                .0
                .direct
                .as_ref()
                .unwrap()
                .validate(&invalid.0)
                .unwrap_err()
                .into();
            let audit = custom_render_audit::snapshot();
            assert_eq!(
                control
                    .request_pbr_scene(invalid, Viewport3DViewRevision::new(2), r025_view())
                    .unwrap_err(),
                expected,
            );
            assert_eq!(custom_render_audit::snapshot(), audit);
            assert_eq!(
                control.target_texture_id_for_test(surface),
                Some(control_image)
            );
            assert_eq!(
                control
                    .status_for(surface)
                    .unwrap()
                    .pbr_committed()
                    .unwrap()
                    .request_serial(),
                control_serial
            );
            assert_eq!(
                read_r027_fp16(control.hdr_front_for_test(surface), &gpu).map(f32::to_bits),
                control_pixel.map(f32::to_bits)
            );
            renderer.drain_pending_image_registrations();
            assert_eq!(
                renderer.render_registered_image_pixel_for_test(
                    control_image,
                    [16.5 / 32.0; 2],
                    [1.0; 4],
                    wgpu::Color::WHITE,
                ),
                control_rgba
            );

            let mut corrected_hdr = negative.0.hdr.clone();
            corrected_hdr.revision = 3;
            corrected_hdr.maximum_rgb = [8.0; 3];
            let corrected = reheader(corrected_hdr);
            // This producer creates new material and normal-texture leases,
            // captured by a new complete snapshot/model/direct generation.
            let successor = consumer_pbr_source(&context, &geometry, n, true, 4);
            assert!(!Arc::ptr_eq(&successor.0.snapshot, &negative.0.snapshot));
            for (source, revision, geometry_reused) in
                [(corrected, 2, true), (successor.clone(), 3, false)]
            {
                let changed_view = zoom_view(52.0);
                enable_timing(&mut control, true);
                let source_checks = MaterialSceneSource3D::full_validation_count_for_test();
                let direct_checks =
                    crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test();
                control
                    .request_pbr_scene(
                        source.clone(),
                        Viewport3DViewRevision::new(revision),
                        changed_view,
                    )
                    .unwrap();
                r024_prepare(&mut control, &metrics);
                let timing = timing_receipt(&control).unwrap();
                assert!(timing.complete);
                assert_eq!(
                    timing.raster.direct_source_qualification_reused,
                    geometry_reused
                );
                assert!(timing.raster.direct_source_validation_reused);
                assert_eq!(
                    MaterialSceneSource3D::full_validation_count_for_test(),
                    source_checks + 1
                );
                assert_eq!(
                    crate::three_d::direct_shadow::DirectSource3D::full_validation_counts_for_test().0,
                    direct_checks.0 + 1
                );
                assert_no_runtime_gpu_model(&timing.raster);
                let tuple = control
                    .status_for(surface)
                    .unwrap()
                    .pbr_committed()
                    .unwrap();
                assert!(tuple.source().same_identity(&source));
                assert_eq!(tuple.view_revision(), Viewport3DViewRevision::new(revision));
                assert_eq!(tuple.view(), changed_view);
                let mut fresh = Viewport3D::new_pbr(
                    context.clone(),
                    source.clone(),
                    Viewport3DViewRevision::new(revision),
                    changed_view,
                )
                .unwrap();
                r024_prepare(&mut fresh, &metrics);
                assert_eq!(
                    read_r027_fp16(control.hdr_front_for_test(surface), &gpu).map(f32::to_bits),
                    read_r027_fp16(fresh.hdr_front_for_test(surface), &gpu).map(f32::to_bits)
                );
                renderer.drain_pending_image_registrations();
                assert_eq!(
                    renderer.render_registered_image_pixel_for_test(
                        control.target_texture_id_for_test(surface).unwrap(),
                        [16.5 / 32.0; 2],
                        [1.0; 4],
                        wgpu::Color::WHITE,
                    ),
                    renderer.render_registered_image_pixel_for_test(
                        fresh.target_texture_id_for_test(surface).unwrap(),
                        [16.5 / 32.0; 2],
                        [1.0; 4],
                        wgpu::Color::WHITE,
                    )
                );
                drop(fresh);
                renderer.drain_pending_image_registrations();
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
            enable_timing(&mut control, true);
            control
                .request_pbr_scene(
                    successor.clone(),
                    Viewport3DViewRevision::new(4),
                    zoom_view(48.0),
                )
                .unwrap();
            r024_prepare(&mut control, &metrics);
            let timing = timing_receipt(&control).unwrap();
            assert!(timing.complete && timing.raster.direct_source_validation_reused);
            let tuple = control
                .status_for(surface)
                .unwrap()
                .pbr_committed()
                .unwrap();
            assert!(tuple.source().same_identity(&successor));
            assert_eq!(tuple.view_revision(), Viewport3DViewRevision::new(4));
            drop(control);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), charges_before);
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "PBR RADIANCE RECEIPT {adapter:?} N{n}: same-ID/revision invalid HDR typed refusal/no writes/old FP16+Image retained; corrected HDR full scan with geometry reuse; new material/texture generation full qualification, then warm validation reuse; complete current tuples and retirement checked\n",
            )).unwrap();
        }
        enable_timing(&mut viewport, false);
        assert_eq!(timing_receipt(&viewport), None);
        viewport
            .request_pbr_scene(
                negative.clone(),
                Viewport3DViewRevision::new(9),
                r025_view(),
            )
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
            Constraints::tight(Size::new(34.0, 34.0)),
            &Theme::default(),
            &mut FontManagerTextEngine::new(),
            &IconRegistry::new(&[]),
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0).unwrap(),
        );
        let viewport = tree_viewport(&tree, child);
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
        let decode = |b: u8| {
            let x = f64::from(b) / 255.0;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        };
        for k in 0..3 {
            assert!(
                (decode(image[k].saturating_sub(1))..=decode(image[k].saturating_add(1)))
                    .contains(&f64::from(negative_pixel[k]))
            );
        }
        assert_eq!(image[3], 255);
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("PUBLIC PBR consumer {adapter:?} N{n}: B/original source, old normal FP16{positive_pixel:?}, certified BLUE127 FP16{negative_pixel:?}, ordinary WHITE Image{image:?}; warm=0, typed source2/2, compatible original-front retention and incompatible withdrawal\n")).unwrap();
        drop(compiled);
        drop(tree);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));

        // After retirement, the entire existing CPU staging budget is available:
        // no qualified source or old in-flight target charge leaked.
        crate::three_d::direct_shadow::QualifiedDirectSource3D::check_retired_cpu_budget_for_test(
            &context,
        )
        .unwrap();
        // HAL-R024-S2-PUBLIC-PRODUCTION-ABSENT-NORMAL-ROLE-DISCRIMINATOR-01:
        // fixed source and comparator selected before execution. The original
        // quad remains unchanged; absence uses an actual normal-only capture,
        // and presence uses the existing B geometry with a real V2 normal role.
        use crate::three_d as api;
        let original_id = api::PbrOriginalIdentity3D {
            source: 97000,
            revision: 1,
            node: 0,
            mesh: 0,
            primitive: 0,
        };
        let positions = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let uv = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ];
        let absent_geometry = context
            .admit_pbr_geometry(
                api::PbrGeometryDescriptor3D {
                    original: api::PbrOriginalGeometry3D {
                        identity: original_id,
                        vertices: (0..6)
                            .map(|j| api::PbrOriginalVertex3D {
                                position: positions[j],
                                normal: [0.8, 0.0, 0.6],
                                uv0: Some(uv[j]),
                                color: Some([255; 4]),
                            })
                            .collect(),
                        indices: GeometryIndices::Unindexed,
                        signed_tangents: Arc::from([[-0.6, 0.0, 0.8, 1.0]; 6]),
                        tangent_provenance: api::PbrTangentProvenance3D::Authored,
                    },
                    render: (0..6)
                        .map(|j| api::PbrVertex3D {
                            position: positions[j],
                            normal: [0.8, 0.0, 0.6],
                            uv0: uv[j],
                            color: [255; 4],
                            tangent: [-0.6, 0.0, 0.8, 1.0],
                        })
                        .collect(),
                    render_indices: GeometryIndices::Unindexed,
                    render_vertex_sources: Arc::from([0, 1, 2, 3, 4, 5]),
                    triangles: (0..2)
                        .map(|triangle| api::PbrTriangleCertificate3D {
                            original: original_id,
                            triangle,
                            certificate_identity: 98001 + triangle as u64,
                            certificate_revision: 1,
                            frame: api::PbrFrameCertificate3D::NormalOnly {
                                separator: [0.0, 0.0, 1.0],
                                lower: [0.5; 3],
                            },
                        })
                        .collect(),
                    mapped: false,
                    maximum_certificate_bytes: 65536,
                    maximum_verification_items: 1000,
                },
                None,
            )
            .unwrap();
        let mut role_images = [[0u8; 4]; 2];
        for (index, present) in [false, true].into_iter().enumerate() {
            let revision = 3 + index as u64;
            let p = 2.0 * 128.0 / 255.0 - 1.0f64;
            let predicted_n_dot_l = if present {
                (-0.8 + 0.6 * (0.6 + 0.8 * p)) / (p * p + 2.0).sqrt()
            } else {
                0.36
            };
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "PUBLIC PBR NORMAL ROLE PREDECLARED N{n} present={present}: original tilted quad/identity model; L[0,.8,.6] E1; observe=false; ideal_source_predicted_n_dot_l={predicted_n_dot_l}; target32x32 pixel(16,16); ABSENT normal-only red>=1/64/ImageRed>PRESENT, PRESENT V2[128,255,255,255] FP16/ImageRGB0; opaque alpha1/255\n",
            )).unwrap();
            let mut material =
                api::PbrMaterialDescriptor3D::new(98200, revision, api::PbrSamplingFamily3D::V2);
            material.base_rgba = [0.5, 0.5, 0.5, 1.0];
            material.metallic = 0.0;
            material.roughness = 1.0;
            material.normal_scale = 1.0;
            material.emissive_rgb = [0.0; 3];
            material.alpha_mode = api::PbrAlphaMode3D::Opaque;
            material.double_sided = false;
            assert!(material.roles.normal.is_none());
            assert!(!material.required.normal);
            if present {
                let pixel = [128, 255, 255, 255];
                let original: Arc<[u8]> = pixel.repeat(4).into();
                let mut bytes = original.to_vec();
                bytes.extend(pixel);
                let bytes: Arc<[u8]> = bytes.into();
                let input = Arc::new(api::PbrMipChainDescriptor3D {
                    role: api::PbrMipRole3D::Normal,
                    format: api::TextureFormat3D::Rgba8Unorm,
                    levels: Arc::from([
                        api::PbrMipLevel3D {
                            width: 2,
                            height: 2,
                            offset: 0,
                            bytes: 16,
                        },
                        api::PbrMipLevel3D {
                            width: 1,
                            height: 1,
                            offset: 16,
                            bytes: 4,
                        },
                    ]),
                    provenance: api::PbrMipProvenance3D {
                        image_identity: 98100,
                        content_revision: 1,
                        producer_identity: 98110,
                        producer_revision: 1,
                        generator_identity: 98111,
                        generator_revision: 1,
                        generator: api::PbrMipGenerator3D::RecursiveStoredAreaR028V1,
                        origin: api::PbrImageOrigin3D::TopLeft,
                        uv_set: 0,
                        work: 4,
                        error_bound: 0.5,
                        original,
                        derived: Arc::downgrade(&bytes),
                    },
                    texels: bytes,
                });
                let sampler = context
                    .admit_pbr_mip_sampler(
                        api::PbrMipSamplerDescriptor3D {
                            identity: 98120,
                            revision: 1,
                            image_identity: input.provenance.image_identity,
                            image_revision: 1,
                            u: api::SamplerAddressMode::ClampToEdge,
                            v: api::SamplerAddressMode::ClampToEdge,
                            mag: api::SamplerFilterMode::Linear,
                            min: api::PbrMipMin3D::LinearMipLinear,
                            extent: [2, 2],
                        },
                        None,
                    )
                    .unwrap();
                let texture = context.admit_pbr_mip_chain(input, None).unwrap();
                material.roles.normal = Some(api::PbrRoleBinding3D::V2 { texture, sampler });
            }
            assert_eq!(material.roles.normal.is_some(), present);
            let material = context.admit_pbr_material(material, None).unwrap();
            let snapshot = context
                .admit_pbr_snapshot(
                    StructuralRevision::new(revision),
                    vec![api::PbrDrawPacket3D {
                        geometry: if present {
                            geometry.clone()
                        } else {
                            absent_geometry.clone()
                        },
                        material,
                        raster: RasterMode::SingleSided,
                        stable_ordinal: 0,
                    }],
                )
                .unwrap();
            let model = context
                .admit_pbr_model(
                    &snapshot,
                    ModelRevision::new(revision),
                    vec![r024_identity()],
                )
                .unwrap();
            let direct = api::PbrDirectSource3D::capture(
                api::PbrDirectDescriptor3D {
                    identity: 98300,
                    revision,
                    meters_per_scene_unit: 1.0,
                    directional: Some(api::PbrDirectionalLight3D {
                        surface_to_source: [0.0, 0.8, 0.6],
                        irradiance: [1.0; 3],
                        shadow_far_m: 8.0,
                    }),
                    points: Arc::from([]),
                    spots: Arc::from([]),
                },
                &snapshot,
                &model,
            )
            .unwrap();
            // Same full ideal evaluator bound: r=1, E=1, emission/E/G=0.
            let source = context
                .admit_pbr_scene(api::PbrSceneDescriptor3D {
                    snapshot,
                    model,
                    hdr: api::PbrHdrSource3D {
                        identity: 98400,
                        revision,
                        exposure_ev: 0.0,
                        maximum_rgb: [4.0; 3],
                    },
                    samples: SampleCount3D::try_new(n).unwrap(),
                    direct: Some(direct),
                    indirect: None,
                })
                .unwrap();
            let rect = RectF::new(0.0, 0.0, 34.0, 34.0);
            let mut compositor = Compositor::new(rect);
            let surface = compositor.create_surface(0, rect, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (34, 34), 1.0)
                .unwrap();
            let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
            let mut viewport = Viewport3D::new_pbr(
                context.clone(),
                source.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
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
                Constraints::tight(Size::new(34.0, 34.0)),
                &Theme::default(),
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(34, 34), 1.0).unwrap(),
            );
            let viewport = tree_viewport(&tree, child);
            assert!(viewport.material_observation_for_test(surface).is_none());
            let status = viewport.status_for(surface).unwrap();
            assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
            assert!(status.requested().is_none() && status.committed().is_none());
            let requested = status.pbr_requested().unwrap();
            let committed = status.pbr_committed().unwrap();
            assert!(requested.source().same_identity(&source));
            assert!(committed.source().same_identity(&source));
            assert_eq!(
                committed.structural_revision(),
                StructuralRevision::new(revision)
            );
            assert_eq!(committed.model_revision(), ModelRevision::new(revision));
            assert_eq!(committed.view(), r025_view());
            assert_eq!(committed.view_revision(), Viewport3DViewRevision::new(1));
            assert_eq!(committed.view_revision(), requested.view_revision());
            assert_eq!(committed.request_serial(), requested.request_serial());
            let request = committed.request_serial().value();
            assert!(request > 0);
            let key = committed.target_key();
            assert_eq!(key, requested.target_key());
            assert_eq!(key.surface_id(), surface);
            assert_eq!(key.physical_render_extent(), PhysicalExtent::new(32, 32));
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
                "PUBLIC PBR NORMAL ROLE PRODUCTION device={} backend={:?} N{n} present={present}: observe=false original={original_id:?} structural/model/direct/HDRrev{revision} request{request} key={key:?} front#{front}; FP16(16,16)={pixel:?}; ordinary WHITE Image={image:?}\n",
                adapter.name, adapter.backend,
            )).unwrap();
            assert_eq!(
                pixel[3], 1.0,
                "covered opaque normal role N{n} present={present}"
            );
            assert_eq!(
                image[3], 255,
                "opaque ordinary Image N{n} present={present}"
            );
            if present {
                assert_eq!(pixel[..3], [0.0; 3], "present mapped N.L branch N{n}");
                assert_eq!(image[..3], [0; 3], "present mapped ordinary Image N{n}");
            } else {
                assert!(
                    pixel[0] >= 1.0 / 64.0,
                    "preselected absent normal response N{n}"
                );
            }
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(front));
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .pbr_committed()
                    .unwrap()
                    .request_serial()
                    .value(),
                request
            );
            role_images[index] = image;
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
            role_images[0][0] > role_images[1][0],
            "preselected absent/present ordinary Image separation N{n}: {role_images:?}"
        );
    }
}

#[test]
fn pbr_unlinked_material_source_rejects_domain_owner_before_mutation() {
    use crate::three_d::{
        DomainComposition3Descriptor, DomainContent3Descriptor, DomainContentId3,
        DomainContentPlacement3, DomainEmbedding3, DomainFrame3Descriptor, DomainState3Descriptor,
        DomainStateRevision3, LocalDomain3Descriptor, LocalDomainId3, LocalDomainKind3,
        PbrSourceError3D,
    };
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("domain-writer boundary requires the actual Device");
    let context = Arc::new(Context3D::new(renderer.gpu_resources()).unwrap());
    let geometry = context.register_geometry(geometry(false)).unwrap();
    let appearance = context
        .register_appearance(
            AppearanceSourceDescriptor::try_new_with_mode(
                constant_source([1.0, 0.0, 0.0]),
                GeometrySemantics::NONE,
                AppearanceMode::Opaque,
            )
            .unwrap(),
        )
        .unwrap();
    let domain = LocalDomainId3::new(97600);
    let content = DomainContentId3::new(97601);
    let composition = context
        .admit_domain_composition(DomainComposition3Descriptor::new(
            StructuralRevision::new(1),
            vec![LocalDomain3Descriptor::new(
                domain,
                LocalDomainKind3::Spatial3,
                vec![DomainContent3Descriptor::new(
                    content,
                    vec![DrawPacketDescriptor::new(
                        geometry.handle().clone(),
                        appearance.handle().clone(),
                        RasterMode::SingleSided,
                        1,
                    )],
                )],
            )],
        ))
        .unwrap();
    let state = context
        .admit_domain_state(
            &composition,
            DomainState3Descriptor::new(
                domain,
                DomainStateRevision3::new(1),
                DomainEmbedding3::identity(),
                vec![(content, DomainContentPlacement3::identity())],
            ),
        )
        .unwrap();
    let frame = context
        .admit_domain_frame(
            &composition,
            DomainFrame3Descriptor::new(ModelRevision::new(1), vec![state]),
        )
        .unwrap();
    let mut viewport = Viewport3D::new_domain_composition(
        context.clone(),
        Viewport3DViewRevision::new(1),
        r025_view(),
        frame.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(97602, 32, 32, 1.0);
    viewport.prepare_custom_render(&metrics);
    let old = viewport.status_for(metrics.surface_id).unwrap().clone();
    assert_eq!(old.state(), Viewport3DLifecycleState::Committed);
    let source_geometry = consumer_pbr_geometry(&context);
    let source = consumer_pbr_source(&context, &source_geometry, 1, true, 1);
    let before = context.resource_usage().unwrap();
    let audit = custom_render_audit::snapshot();
    for _ in 0..2 {
        assert!(matches!(
            viewport
                .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(1), r025_view(),),
            Err(PbrSourceError3D::Association(_))
        ));
    }
    let status = viewport.status_for(metrics.surface_id).unwrap();
    assert_eq!(status.state(), old.state());
    assert!(status.pbr_requested().is_none() && status.pbr_committed().is_none());
    assert_eq!(
        status.requested().unwrap().request_serial(),
        old.requested().unwrap().request_serial(),
    );
    assert_eq!(
        status.committed().unwrap().request_serial(),
        old.committed().unwrap().request_serial(),
    );
    assert!(
        status
            .committed()
            .unwrap()
            .snapshot()
            .same_identity(composition.render_snapshot())
    );
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .committed_domain_ray(domain, 0.5, 0.5)
            .unwrap()
            .is_some()
    );
    assert_eq!(context.resource_usage().unwrap(), before);
    assert_eq!(custom_render_audit::snapshot(), audit);
}
