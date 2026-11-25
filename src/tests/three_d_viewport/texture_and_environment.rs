use super::*;

#[test]
fn r028_actual_1024_five_role_gradient8_source_stage_roi_and_fp16_n1_n4() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::{MipMin3D, MipRole3D, SamplingFamily3D};
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: real Device required");
    let gpu = renderer.gpu_resources();
    let adapter = renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 actual {adapter:?}; wgpu30.0.2/Naga30.0.2; source ROI x/y8..24, linear error<=.03\n")).unwrap();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let solid = |pixel: [u8; 4]| pixel.repeat(1024 * 1024);
    let chains = [
        producer::checker(MipRole3D::OpaqueBase, 1024, 28001),
        producer::produce(
            MipRole3D::Normal,
            1024,
            1024,
            solid([128, 128, 128, 255]),
            28002,
            1,
            true,
        ),
        producer::produce(
            MipRole3D::MetallicRoughness,
            1024,
            1024,
            solid([32, 128, 192, 64]),
            28003,
            1,
            true,
        ),
        producer::produce(
            MipRole3D::Occlusion,
            1024,
            1024,
            solid([191, 17, 61, 29]),
            28004,
            1,
            true,
        ),
        producer::checker(MipRole3D::Emissive, 1024, 28005),
    ];
    let resources = chains
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            r028_role(
                &context,
                t,
                28100 + i as u64,
                MipMin3D::LinearMipLinear,
                Linear,
                ClampToEdge,
                ClampToEdge,
            )
        })
        .collect::<Vec<_>>();
    for n in [1, 4] {
        let mut material = PbrMaterialSource3D::defaults(28000, 1);
        material.sampling = SamplingFamily3D::V2;
        material.emissive_rgb = [1.0; 3];
        material.occlusion_strength = 0.5;
        for i in 0..5 {
            material.roles[i] = Some(resources[i].0.clone());
        }
        let source = r024_source(
            &context,
            r028_quad([0.25; 2], [0.0; 2]),
            material,
            n,
            true,
            r024_identity(),
            1,
        );
        let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
        let metrics = active_metrics(28000 + u64::from(n), 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let observations = r028_observations(&viewport, metrics.surface_id, &gpu);
        let expected = producer::decode(188);
        let mut max_error = 0.0f64;
        let mut max_area_error = 0.0f64;
        for y in 8..24 {
            for x in 8..24 {
                let o = observations[y * 32 + x];
                assert!(o.iter().flatten().all(|x| x.is_finite()));
                for c in 0..3 {
                    max_error = max_error.max((f64::from(o[6][c]) - expected).abs());
                    max_area_error = max_area_error.max((f64::from(o[6][c]) - 0.5).abs());
                }
                assert_eq!(o[6][3], 1.0);
                assert!((o[7][0] - 192.0 / 255.0).abs() < 0.0001);
                assert!((o[7][1] - 128.0 / 255.0).abs() < 0.0001);
                assert!((o[7][2] - (1.0 + 0.5 * (191.0 / 255.0 - 1.0))).abs() < 0.0001);
                for (a, b) in o[9].into_iter().zip([1.0 / 128.0, 0.0, 0.0, 1.0 / 128.0]) {
                    assert!(
                        (a - b).abs() < 0.00001,
                        "actual pre-discard derivatives {:?}",
                        o[9]
                    );
                }
                assert!(o[10][2] > 0.0 && (o[10][2] - 1.0 / 255.0).abs() < 0.000001);
            }
        }
        assert!(
            max_error <= 0.03,
            "N{n} source-stage max linear error {max_error}"
        );
        assert!(
            max_area_error <= 0.03,
            "N{n} independent checker area-reference error {max_area_error}"
        );
        let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert_eq!(pixel[3], 1.0);
        for c in 0..3 {
            assert!((f64::from(pixel[c]) - expected / (1.0 + expected)).abs() < 0.001);
        }
        assert!(context.resource_usage().unwrap().resident_texture_bytes >= 27_962_020);
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 N{n} real gradient8 source error={max_error}, rawBase={:?}, MR/AO={:?}, dq={:?}, FP16={pixel:?}; five chains27962020B\n",observations[16*32+16][6],observations[16*32+16][7],observations[16*32+16][9])).unwrap();
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn r028_actual_nonconstant_multiscale_production_fragment_integer_fractional_gradients_n1_n4() {
    use super::three_d_context::r028_fixture as p;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual nonconstant fragment Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    let base = p::multiscale();
    let emission = p::produce(
        MipRole3D::Emissive,
        1024,
        1024,
        base.provenance.original.to_vec(),
        28950,
        1,
        true,
    );
    assert_eq!(
        base.texels, emission.texels,
        "the two independently role-qualified producer outputs are byte-equal here"
    );
    let policy = p::sampler(
        &base,
        28951,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let b = r028_role(
        &context,
        base.clone(),
        28951,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let e = r028_role(
        &context,
        emission,
        28952,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    // Predeclared physical ROI4..28, source-linear tolerance.01. These are
    // actual dpdx/dpdy in sample_surface, not fixed gradients in a probe shader.
    let mut maximum_error = 0.0f64;
    for n in [1, 4] {
        for lambda in [0.0f32, 1.0, 2.0, 3.0, 3.35, 4.6, 7.0, 8.25] {
            let step = 2.0f32.powf(lambda) / 1024.0;
            let span = 32.0 * step;
            let mut material = PbrMaterialSource3D::defaults(28953, 1);
            material.sampling = SamplingFamily3D::V2;
            material.emissive_rgb = [1.0; 3];
            material.roles[0] = Some(b.0.clone());
            material.roles[4] = Some(e.0.clone());
            let source = r024_source(
                &context,
                r028_quad([span; 2], [101.0 / 1024.0, 307.0 / 1024.0]),
                material,
                n,
                true,
                r024_identity(),
                1,
            );
            let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
            let metrics = active_metrics(28950 + u64::from(n), 32, 32, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let values = r028_observations(&viewport, metrics.surface_id, &gpu);
            for y in 4..28 {
                for x in 4..28 {
                    let o = values[y * 32 + x];
                    for (a, b) in o[9].into_iter().zip([step, 0.0, 0.0, step]) {
                        assert!(
                            (a - b).abs() < 0.00001,
                            "source footprint{lambda} actual{:?}",
                            o[9]
                        );
                    }
                    let uv = [o[10][0], o[10][1]];
                    let expected =
                        super::three_d_raster::r028_reference_sample(&base, policy, uv, lambda);
                    for c in 0..3 {
                        let error = (f64::from(o[6][c]) - expected[c]).abs();
                        maximum_error = maximum_error.max(error);
                        assert!(
                            error <= 0.01,
                            "real material N{n} lambda{lambda}, ({x},{y}) {uv:?}: raw{:?}, reference{expected:?}",
                            o[6]
                        );
                    }
                }
            }
            let o = values[16 * 32 + 16];
            let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            for c in 0..3 {
                assert!((final_pixel[c] - o[6][c] / (1.0 + o[6][c])).abs() < 0.0015);
            }
            assert_eq!(final_pixel[3], 1.0);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 actual production sample_surface N{n} lambda{lambda}: dxdy={:?}, rawBase={:?}, FP16={final_pixel:?}\n",o[9],o[6])).unwrap();
            viewport.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 nonconstant production fragment N1/N4 integer/fractional LOD max source-linear error={maximum_error}\n")).unwrap();
}

/// Independent R028 §12.2 reference: integrate piecewise-constant ORIGINAL
/// sRGB texel cells in linear f64. No mip bytes, mip producer, GPU observations,
/// point quadrature, or material/tone operations enter this reference.
struct R028OriginalAreaReference {
    linear: Vec<[f64; 3]>,
}

impl R028OriginalAreaReference {
    fn new(original_rgba: &[u8]) -> Self {
        assert_eq!(original_rgba.len(), 1024 * 1024 * 4);
        let decode = |byte: u8| {
            let encoded = f64::from(byte) / 255.0;
            if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            }
        };
        let linear = original_rgba
            .chunks_exact(4)
            .map(|rgba| {
                assert_eq!(rgba[3], 255);
                [decode(rgba[0]), decode(rgba[1]), decode(rgba[2])]
            })
            .collect();
        Self { linear }
    }

    fn rectangle(&self, bounds: [f64; 4]) -> [f64; 3] {
        let [u0, v0, u1, v1] = bounds;
        assert!(
            bounds
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        );
        assert!(u0 < u1 && v0 < v1);
        let [x0, y0, x1, y1] = bounds.map(|v| v * 1024.0);
        let mut integral = [0.0; 3];
        for y in y0.floor() as usize..y1.ceil() as usize {
            let height = y1.min((y + 1) as f64) - y0.max(y as f64);
            for x in x0.floor() as usize..x1.ceil() as usize {
                let width = x1.min((x + 1) as f64) - x0.max(x as f64);
                let area = width * height;
                for (sum, value) in integral.iter_mut().zip(self.linear[y * 1024 + x]) {
                    *sum += area * value;
                }
            }
        }
        integral.map(|sum| sum / ((x1 - x0) * (y1 - y0)))
    }

    fn sixteen_subareas(&self, center: [f64; 2], step: f64) -> [f64; 3] {
        let lower = center.map(|v| v - step / 2.0);
        let mut mean = [0.0; 3];
        for b in 0..4 {
            for a in 0..4 {
                let value = self.rectangle([
                    lower[0] + f64::from(a) * step / 4.0,
                    lower[1] + f64::from(b) * step / 4.0,
                    lower[0] + f64::from(a + 1) * step / 4.0,
                    lower[1] + f64::from(b + 1) * step / 4.0,
                ]);
                for c in 0..3 {
                    mean[c] += value[c] / 16.0;
                }
            }
        }
        let direct = self.rectangle([
            lower[0],
            lower[1],
            center[0] + step / 2.0,
            center[1] + step / 2.0,
        ]);
        for c in 0..3 {
            assert!(
                (mean[c] - direct[c]).abs() <= 1e-9,
                "invalid independent area reference: sixteen={mean:?}, direct={direct:?}"
            );
        }
        mean
    }
}

#[test]
fn r028_actual_120_frame_multiscale_temporal_reprojected_area_reference_n1_n4() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::{MipMin3D, SamplingFamily3D};
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};

    // Frozen before execution by HAL-R028-Q12_2-TEMPORAL-120-FRAME-AREA-CONFORMANCE-01:
    // f=0..119; lambda=8f/119; p=2^lambda; step=p/1024; 32² retained target;
    // original quad/camera; ROI x,y=14..=17; sixteen equal-area subrectangles;
    // original linear source-cell integration; 1920 errors/N; nearest-rank
    // p95 at one-based1824; threshold.05. Nothing depends on measured output.
    const FRAMES: usize = 120;
    const PIXELS_PER_FRAME: usize = 16;
    const P95_INDEX: usize = 1823;
    const LIMIT: f64 = 0.05;
    let fingerprint = |bytes: &[u8]| {
        bytes.iter().fold(0xcbf29ce484222325u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
    };
    let chain = producer::multiscale();
    assert_eq!(chain.levels.len(), 11);
    assert_eq!(chain.texels.len(), 5_592_404);
    assert_eq!(chain.provenance.image_identity, 28011);
    assert_eq!(chain.provenance.content_revision, 1);
    let original = chain.provenance.original.clone();
    let original_fingerprint = fingerprint(&original);
    let chain_fingerprint = fingerprint(&chain.texels);
    let reference = R028OriginalAreaReference::new(&original);
    // Validate fractional boundaries independently of the sixteen-subarea sum.
    let cell = reference.rectangle([
        101.0 / 1024.0,
        307.0 / 1024.0,
        102.0 / 1024.0,
        308.0 / 1024.0,
    ]);
    assert_eq!(cell, reference.linear[307 * 1024 + 101]);
    let fractional = reference.rectangle([
        101.25 / 1024.0,
        307.25 / 1024.0,
        102.25 / 1024.0,
        308.25 / 1024.0,
    ]);
    for c in 0..3 {
        let independently_weighted = 0.5625 * reference.linear[307 * 1024 + 101][c]
            + 0.1875 * reference.linear[307 * 1024 + 102][c]
            + 0.1875 * reference.linear[308 * 1024 + 101][c]
            + 0.0625 * reference.linear[308 * 1024 + 102][c];
        assert!((fractional[c] - independently_weighted).abs() <= 1e-12);
    }
    let mut results = Vec::new();
    for n in [1, 4] {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("UNVERIFIED R028 §12.2: real retained-target Device required");
        let adapter = renderer.headless_adapter_for_test();
        assert_eq!(
            adapter.backend,
            wgpu::Backend::Metal,
            "this evidence unit requires Metal"
        );
        assert_eq!(
            adapter.name, "Apple M4 Max",
            "this evidence unit requires the selected adapter"
        );
        let gpu = renderer.gpu_resources();
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let role = r028_role(
            &context,
            chain.clone(),
            29101,
            MipMin3D::LinearMipLinear,
            Linear,
            ClampToEdge,
            ClampToEdge,
        );
        assert!(Arc::ptr_eq(&role.1.source.provenance.original, &original));
        assert!(Arc::ptr_eq(&role.1.source.texels, &chain.texels));
        let mut material = PbrMaterialSource3D::defaults(29102, 1);
        material.sampling = SamplingFamily3D::V2;
        material.roles[0] = Some(role.0.clone());
        let first_step = 1.0f32 / 1024.0;
        let mut source = r024_source(
            &context,
            r028_quad([32.0 * first_step; 2], [0.5 - 16.0 * first_step; 2]),
            material,
            n,
            true,
            r024_identity(),
            1,
        );
        let mut viewport = r024_viewport(context.clone(), source.clone(), r024_view(2.0));
        let metrics = active_metrics(29100 + u64::from(n), 32, 32, 1.0);
        let mut target_id = None;
        let mut previous_request = 0;
        let mut resident_bytes = None;
        let mut errors = Vec::with_capacity(FRAMES * PIXELS_PER_FRAME);
        let mut previous_footprint = 0.0;
        let mut footprint_range = [f64::INFINITY, f64::NEG_INFINITY];
        let mut lod_range = [f64::INFINITY, f64::NEG_INFINITY];
        let mut maximum_uv_delta = 0.0f64;
        let mut maximum_gradient_delta = 0.0f64;
        let mut first_raw = None;
        let mut nonconstant = false;
        let mut crossed = [false; 8];
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 Q12.2 PREDECLARED N{n} {adapter:?}; wgpu30.0.2/Naga30.0.2; 120frames/ROI14..=17/16subareas/rank1824/limit.05; image28011r1 producer{}r{} generator{}r{}; original_FNV1a64={original_fingerprint:016x}; chain_FNV1a64={chain_fingerprint:016x}; chain5592404B/11levels; LinearMipLinear/Linear/ClampToEdge/anisotropy1\n",
            chain.provenance.producer_identity, chain.provenance.producer_revision,
            chain.provenance.generator_identity, chain.provenance.generator_revision,
        )).unwrap();
        for f in 0..FRAMES {
            let lambda = 8.0 * f as f64 / 119.0;
            let nominal_footprint = 2.0f64.powf(lambda);
            let step = nominal_footprint / 1024.0;
            if f != 0 {
                let previous = source.clone();
                let MaterialPacketSource3D::PbrMaterial {
                    geometry,
                    material,
                    raster,
                    stable_ordinal,
                } = &previous.snapshot.source_packets()[0]
                else {
                    panic!("wrong measured packet")
                };
                let geometry = context
                    .register_pbr_geometry(
                        PbrGeometrySource3D::try_new(
                            r028_quad([(32.0 * step) as f32; 2], [(0.5 - 16.0 * step) as f32; 2]),
                            GeometryIndices::Unindexed,
                        )
                        .unwrap(),
                        Some(geometry),
                    )
                    .unwrap();
                let revision = f as u64 + 1;
                let snapshot = context
                    .admit_material_snapshot(
                        StructuralRevision::new(revision),
                        vec![MaterialPacketSource3D::PbrMaterial {
                            geometry,
                            material: material.clone(),
                            raster: *raster,
                            stable_ordinal: *stable_ordinal,
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
                source = Arc::new(MaterialSceneSource3D {
                    snapshot,
                    model,
                    hdr: crate::three_d::hdr::HdrSource3D {
                        revision,
                        ..previous.hdr.clone()
                    },
                    samples: n,
                    direct: None,
                    indirect: None,
                    observe: true,
                });
                assert!(
                    viewport
                        .set_material_source_for_test(
                            source.clone(),
                            Viewport3DViewRevision::new(1),
                            r024_view(2.0),
                        )
                        .unwrap()
                );
                assert!(
                    Arc::ptr_eq(
                        viewport
                            .committed_material_source_for_test(metrics.surface_id)
                            .unwrap(),
                        &previous,
                    ),
                    "request must not relabel the previous front before publication"
                );
                assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), f as u64);
            }
            r024_prepare(&mut viewport, &metrics);
            renderer.drain_pending_image_registrations();
            assert!(Arc::ptr_eq(
                viewport
                    .committed_material_source_for_test(metrics.surface_id)
                    .unwrap(),
                &source,
            ));
            assert_eq!(source.hdr.revision, f as u64 + 1);
            assert_eq!(
                viewport.hdr_swaps_for_test(metrics.surface_id),
                f as u64 + 1
            );
            let request = viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial()
                .value();
            assert!(request > previous_request);
            previous_request = request;
            let id = viewport.target_texture_id_for_test(metrics.surface_id);
            if let Some(original_id) = target_id {
                assert_eq!(id, original_id);
            } else {
                target_id = Some(id);
            }
            let usage = context.resource_usage().unwrap();
            if let Some(bytes) = resident_bytes {
                assert_eq!(usage.resident_texture_bytes, bytes);
            } else {
                resident_bytes = Some(usage.resident_texture_bytes);
            }
            let values = r028_observations(&viewport, metrics.surface_id, &gpu);
            assert_eq!(values.len(), 32 * 32);
            let mut frame_error = 0.0f64;
            let mut frame_footprints = [f64::INFINITY, f64::NEG_INFINITY];
            let mut frame_lods = [f64::INFINITY, f64::NEG_INFINITY];
            let mut covered = 0;
            for y in 14..=17 {
                for x in 14..=17 {
                    let o = values[y * 32 + x];
                    assert!(
                        o.iter().flatten().all(|value| value.is_finite()),
                        "N{n} frame{f} ({x},{y}) nonfinite observation"
                    );
                    assert_eq!(
                        (o[5][3], o[6][3], o[10][3]),
                        (1.0, 1.0, 1.0),
                        "missing/uncovered source-stage observation N{n} frame{f} ({x},{y})"
                    );
                    let center = [
                        0.5 + (x as f64 + 0.5 - 16.0) * step,
                        0.5 + (y as f64 + 0.5 - 16.0) * step,
                    ];
                    for (observed, expected) in o[10][..2].iter().zip(center) {
                        let delta = (f64::from(*observed) - expected).abs();
                        maximum_uv_delta = maximum_uv_delta.max(delta);
                        assert!(
                            delta <= 1e-5,
                            "N{n} frame{f} ({x},{y}) UV mismatch {:?} vs{center:?}",
                            o[10]
                        );
                    }
                    for (observed, expected) in o[9].into_iter().zip([step, 0.0, 0.0, step]) {
                        let delta = (f64::from(observed) - expected).abs();
                        maximum_gradient_delta = maximum_gradient_delta.max(delta);
                        assert!(
                            delta <= 1e-5,
                            "N{n} frame{f} derivative mismatch {:?} vs step{step}",
                            o[9]
                        );
                    }
                    let dx = f64::from(o[9][0]).hypot(f64::from(o[9][1]));
                    let dy = f64::from(o[9][2]).hypot(f64::from(o[9][3]));
                    let footprint = 1024.0 * dx.max(dy);
                    assert!(footprint.is_finite() && footprint > 0.0);
                    let lod = footprint.log2();
                    frame_footprints[0] = frame_footprints[0].min(footprint);
                    frame_footprints[1] = frame_footprints[1].max(footprint);
                    frame_lods[0] = frame_lods[0].min(lod);
                    frame_lods[1] = frame_lods[1].max(lod);
                    let expected = reference.sixteen_subareas(center, step);
                    let mut error = 0.0f64;
                    for c in 0..3 {
                        assert!((0.0..=1.0).contains(&o[6][c]));
                        error = error.max((f64::from(o[6][c]) - expected[c]).abs());
                    }
                    errors.push(error);
                    frame_error = frame_error.max(error);
                    covered += 1;
                }
            }
            assert_eq!(covered, PIXELS_PER_FRAME);
            assert_eq!(errors.len(), (f + 1) * PIXELS_PER_FRAME);
            assert!(
                frame_footprints[0] > previous_footprint,
                "nonmonotone observed sweep at N{n} frame{f}"
            );
            previous_footprint = frame_footprints[1];
            for (i, observed) in crossed.iter_mut().enumerate() {
                *observed |= frame_lods[0] >= (i + 1) as f64;
            }
            footprint_range[0] = footprint_range[0].min(frame_footprints[0]);
            footprint_range[1] = footprint_range[1].max(frame_footprints[1]);
            lod_range[0] = lod_range[0].min(frame_lods[0]);
            lod_range[1] = lod_range[1].max(frame_lods[1]);
            let raw = values[16 * 32 + 16][6];
            // Compare the SAME pixel across frames, not different ROI pixels
            // within one frame, to require a genuine temporal value change.
            let raw_rgb = [raw[0], raw[1], raw[2]];
            if let Some(first) = first_raw {
                nonconstant |= raw_rgb != first;
            } else {
                first_raw = Some(raw_rgb);
            }
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R028 Q12.2 N{n} f={f:03} hdr={} request={request} covered={covered}/16 lambda={lambda:.9} measured_p={frame_footprints:.12?} measured_lod={frame_lods:.12?} dxdy={:?} uv={:?} raw={raw:?} max_error={frame_error:.12}\n",
                source.hdr.revision, values[16 * 32 + 16][9], values[16 * 32 + 16][10],
            )).unwrap();
        }
        assert_eq!(errors.len(), 1920);
        assert!(
            crossed.into_iter().all(|value| value),
            "missing observed mip boundary"
        );
        assert!(nonconstant, "constant/fixed-value temporal surrogate");
        assert_eq!(fingerprint(&original), original_fingerprint);
        assert_eq!(fingerprint(&chain.texels), chain_fingerprint);
        assert!(Arc::ptr_eq(&role.1.source.provenance.original, &original));
        assert!(Arc::ptr_eq(&role.1.source.texels, &chain.texels));
        let above_limit = errors.iter().filter(|error| **error > LIMIT).count();
        errors.sort_by(f64::total_cmp);
        let p95 = errors[P95_INDEX];
        let maximum = errors[errors.len() - 1];
        let fp16 = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert_eq!(
            fp16,
            [0.0, 0.0, 0.0, 1.0],
            "base-only material has no invented illumination"
        );
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 Q12.2 RESULT N{n}: frames=120 pixels={} rank=1824 p95={p95:.12} max={maximum:.12} above_limit={above_limit} footprint={footprint_range:.12?} lod={lod_range:.12?} max_uv_delta={maximum_uv_delta:.12} max_gradient_delta={maximum_gradient_delta:.12} nonconstant={nonconstant} target={target_id:?} resident_texture_bytes={resident_bytes:?} FP16={fp16:?} outcome={}\n",
            errors.len(), if p95 <= LIMIT { "PASS" } else { "FAIL" },
        )).unwrap();
        results.push((n, p95, maximum));
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
    assert!(
        results.iter().all(|(_, p95, _)| *p95 <= LIMIT),
        "R028 §12.2 measured FAIL (no source/ROI/reference/sampler change permitted): {results:?}"
    );
}

pub(super) fn r028_presence_material(context: &Context3D) -> PbrMaterialSource3D {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::{MipMin3D, MipRole3D, SamplingFamily3D};
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut material = PbrMaterialSource3D::defaults(28800, 1);
    material.sampling = SamplingFamily3D::V2;
    material.base_rgba = [0.5, 0.5, 0.5, 1.0];
    material.metallic = 0.0;
    material.normal_scale = 0.0;
    material.emissive_rgb = [0.125; 3];
    let roles = [
        MipRole3D::OpaqueBase,
        MipRole3D::Normal,
        MipRole3D::MetallicRoughness,
        MipRole3D::Occlusion,
        MipRole3D::Emissive,
    ];
    let resources = roles
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let p = if r == MipRole3D::Normal {
                [128, 128, 255, 255]
            } else {
                [255; 4]
            };
            r028_role(
                context,
                producer::produce(r, 2, 2, p.repeat(4), 28810 + i as u64, 1, true),
                28820 + i as u64,
                MipMin3D::LinearMipLinear,
                Linear,
                ClampToEdge,
                ClampToEdge,
            )
        })
        .collect::<Vec<_>>();
    for i in 0..5 {
        material.roles[i] = Some(resources[i].0.clone());
    }
    material
}

pub(super) fn r028_presence_scene(context: &Context3D, n: u32, bits: u8) -> Arc<MaterialSceneSource3D> {
    let material = r028_presence_material(context);
    let mut scene = r024_source(
        context,
        r028_quad([1.0; 2], [0.0; 2]),
        material,
        n,
        false,
        r024_identity(),
        1,
    );
    let s = Arc::get_mut(&mut scene).unwrap();
    // Same whole-domain fixture bound as the accepted R026 source: r=1,
    // directional E<=1, emission=1/8, E<=3pi/4, spec<=3/4,
    // WORLD G<=27/32; the conservative mechanical response is below64.
    s.hdr.maximum_rgb = [64.0; 3];
    if bits & 1 != 0 {
        s.direct = Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
            identity: 28830,
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
    }
    if bits & 6 != 0 {
        let e = (bits & 2 != 0).then(|| {
            r026_fixture::environment(context, 28840, glam::DVec3::X, 0.0, [0.25, 0.5, 0.75])
        });
        s.indirect = Some(r026_fixture::attach(context, s, e, bits & 4 != 0, 1));
    }
    scene
}

#[test]
fn r028_actual_eight_de_g_v2_production_profiles_n1_n4_and_ordinary_white_image() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual Device required");
    let gpu = renderer.gpu_resources();
    let adapter = renderer.headless_adapter_for_test().clone();
    for n in [1, 4] {
        for bits in 0..8 {
            let baseline_context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let baseline_source = r026_fixture::scene(&baseline_context, n, bits);
            let mut baseline = r024_viewport(baseline_context, baseline_source, r025_view());
            let metrics = active_metrics(28800 + u64::from(bits), 32, 32, 1.0);
            r024_prepare(&mut baseline, &metrics);
            let v1_pixel = read_r027_fp16(baseline.hdr_front_for_test(metrics.surface_id), &gpu);
            baseline.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            drop(baseline);
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let source = r028_presence_scene(&context, n, bits);
            source.validate(&context).unwrap();
            let viewport = r024_viewport(context.clone(), source.clone(), r025_view());
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
                "V2 bits{bits} N{n}: {:?}",
                viewport.material_failure_for_test(surface)
            );
            assert!(
                viewport.material_observation_for_test(surface).is_none(),
                "production entry must produce actual output"
            );
            assert!(Arc::ptr_eq(
                &viewport
                    .committed_material_source_for_test(surface)
                    .unwrap(),
                &source
            ));
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            for c in 0..4 {
                assert!(
                    (pixel[c] - v1_pixel[c]).abs() < 0.001,
                    "same evaluator V1{v1_pixel:?}/V2{pixel:?}"
                );
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
                let linear = super::three_d_context::r028_fixture::decode(image[c]);
                assert!(
                    (linear - f64::from(pixel[c])).abs() < 0.02,
                    "V2 {bits:03b} N{n} FP16{pixel:?} Image{image:?}"
                );
            }
            assert_eq!(image[3], 255);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 {adapter:?} V2 DEG={bits:03b} N{n}: V1={v1_pixel:?} FP16={pixel:?} WHITE Image={image:?}\n")).unwrap();
            drop(compiled);
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
    }
}

#[test]
fn r028_actual_retained_chain_capacity_failure_preserves_old_front_and_warm_no_work() {
    use super::three_d_context::r028_fixture as p;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual retirement Device required");
    let gpu = renderer.gpu_resources();
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resident_texture_bytes = 400;
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
    let resource = r028_role(
        &context,
        p::produce(
            MipRole3D::Emissive,
            4,
            4,
            [255, 0, 0, 255].repeat(16),
            28901,
            1,
            true,
        ),
        28902,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let mut material = PbrMaterialSource3D::defaults(28900, 1);
    material.sampling = SamplingFamily3D::V2;
    material.emissive_rgb = [1.0; 3];
    material.roles[4] = Some(resource.0.clone());
    let source = r024_source(
        &context,
        r028_quad([1.0; 2], [0.0; 2]),
        material,
        4,
        false,
        r024_identity(),
        1,
    );
    assert_eq!(
        context.resource_usage().unwrap().resident_texture_bytes,
        0,
        "source admission cannot allocate V2 GPU content"
    );
    let mut viewport = r024_viewport(context.clone(), source.clone(), r024_view(2.0));
    let metrics = active_metrics(28900, 32, 32, 1.0);
    r024_prepare(&mut viewport, &metrics);
    renderer.drain_pending_image_registrations();
    let first = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
    assert_eq!(first, [0.5, 0.0, 0.0, 1.0]);
    let usage = context.resource_usage().unwrap();
    assert_eq!(
        usage.resident_texture_bytes, 104,
        "84B old chain +20B actual material/default raster content"
    );
    let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
    let id = viewport.target_texture_id_for_test(metrics.surface_id);
    let audit = custom_render_audit::events();
    let events = viewport_audit::snapshot();
    r024_prepare(&mut viewport, &metrics);
    assert_eq!(context.resource_usage().unwrap(), usage);
    assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
    assert_eq!(custom_render_audit::events(), audit);
    assert_eq!(viewport_audit::snapshot(), events);
    let PbrRoleBinding3D::V2 {
        texture: old_texture,
        sampler: old_sampler,
    } = &resource.0
    else {
        panic!()
    };
    let next_chain = p::produce(
        MipRole3D::Emissive,
        8,
        8,
        [0, 255, 0, 255].repeat(64),
        28901,
        2,
        true,
    );
    let mut next_sampler = p::sampler(
        &next_chain,
        28902,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    next_sampler.revision = 2;
    let texture = context
        .register_mip_chain(next_chain, Some(old_texture))
        .unwrap();
    let sampler = context
        .register_mip_sampler(next_sampler, Some(old_sampler))
        .unwrap();
    let mut material = PbrMaterialSource3D::defaults(28903, 1);
    material.sampling = SamplingFamily3D::V2;
    material.emissive_rgb = [1.0; 3];
    material.roles[4] = Some(PbrRoleBinding3D::V2 { texture, sampler });
    let next = r024_source(
        &context,
        r028_quad([1.0; 2], [0.0; 2]),
        material,
        4,
        false,
        r024_identity(),
        2,
    );
    viewport
        .set_material_source_for_test(next, Viewport3DViewRevision::new(1), r024_view(2.0))
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    assert!(matches!(
        viewport.material_failure_for_test(metrics.surface_id),
        Some(crate::three_d::pbr::PbrError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ResidentTextureBytes,
                requested: 444,
                maximum: 400
            }
        ))
    ));
    assert_eq!(
        viewport.status_for(metrics.surface_id).unwrap().state(),
        Viewport3DLifecycleState::FailedWithLastGood
    );
    assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
    assert_eq!(viewport.target_texture_id_for_test(metrics.surface_id), id);
    assert!(Arc::ptr_eq(
        viewport
            .committed_material_source_for_test(metrics.surface_id)
            .unwrap(),
        &source
    ));
    assert_eq!(
        first,
        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu)
    );
    assert_eq!(
        context.resource_usage().unwrap().resident_texture_bytes,
        104
    );
    viewport.on_unmount();
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
    assert_eq!(
        context.resource_usage().unwrap().resident_texture_bytes,
        20,
        "old V2 generation releases only after real last-use completion"
    );
}

#[test]
fn r028_actual_mask_derivatives_precede_discard_and_helper_lanes_n1_n4() {
    use super::three_d_context::r028_fixture as p;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual Mask Device required");
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
    // Source edge at496: level5 texel15 stores A128. Thus the even x14
    // invocation discards beside the surviving odd x15 invocation in its quad.
    let raw = (0..1024 * 1024)
        .flat_map(|i| {
            if i % 1024 < 496 {
                [0, 255, 0, 0]
            } else {
                [255, 0, 0, 255]
            }
        })
        .collect();
    let chain = p::produce(MipRole3D::MaskBase, 1024, 1024, raw, 28910, 1, true);
    let policy = p::sampler(
        &chain,
        28911,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let resource = r028_role(
        &context,
        chain.clone(),
        28911,
        MipMin3D::LinearMipLinear,
        Linear,
        ClampToEdge,
        ClampToEdge,
    );
    for n in [1, 4] {
        let mut material = PbrMaterialSource3D::defaults(28912, 1);
        material.sampling = SamplingFamily3D::V2;
        material.alpha_mode = PbrAlphaMode3D::Mask;
        material.cutoff = 0.5;
        material.emissive_rgb = [0.25, 0.5, 0.75];
        material.roles[0] = Some(resource.0.clone());
        let source = r024_source(
            &context,
            r028_quad([1.0; 2], [0.0; 2]),
            material,
            n,
            true,
            r024_identity(),
            1,
        );
        let mut viewport = r024_viewport(context.clone(), source, r024_view(2.0));
        let metrics = active_metrics(28910 + u64::from(n), 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let values = r028_observations(&viewport, metrics.surface_id, &gpu);
        for y in 8..24 {
            for x in 8..24 {
                let uv = [(x as f32 + 0.5) / 32.0, (y as f32 + 0.5) / 32.0];
                let expected =
                    super::three_d_raster::r028_reference_sample(&chain, policy, uv, 5.0);
                let covered = expected[3] >= 0.5;
                let o = values[y * 32 + x];
                if covered {
                    for c in 0..4 {
                        assert!((f64::from(o[6][c]) - expected[c]).abs() < 0.01);
                    }
                    for (a, b) in o[9].into_iter().zip([1.0 / 32.0, 0.0, 0.0, 1.0 / 32.0]) {
                        assert!((a - b).abs() < 0.00001);
                    }
                } else {
                    assert_eq!(o, [[0.0; 4]; 11]);
                }
            }
        }
        assert_eq!(
            read_r027_fp16_at(
                viewport.hdr_front_for_test(metrics.surface_id),
                &gpu,
                14,
                16
            ),
            [0.0; 4]
        );
        let surviving = read_r027_fp16_at(
            viewport.hdr_front_for_test(metrics.surface_id),
            &gpu,
            15,
            16,
        );
        assert_eq!(surviving[3], 1.0);
        assert!((surviving[0] - 0.2).abs() < 0.001);
        assert_eq!(
            values[16 * 32 + 15][6][1],
            0.0,
            "transparent green cannot contaminate straight Mask RGB"
        );

        // HAL-R028-S2-MASK-HELPER-EDGE-ORDINARY-IMAGE-PRODUCTION-01:
        // preserve the source-stage observer assertions above, then reuse its
        // exact immutable source capture with observation disabled. These final
        // FP16/Image samples do not directly measure helper-lane execution.
        let adapter = renderer.headless_adapter_for_test().clone();
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        assert_eq!(adapter.name, "Apple M4 Max");
        let old_status = viewport.status_for(metrics.surface_id).unwrap();
        let old_committed = old_status.committed_material().unwrap();
        let observed_source = old_committed.source().clone();
        let old_serial = old_committed.request_serial().value();
        let structural_revision = old_committed.structural_revision();
        let model_revision = old_committed.model_revision();
        let key = old_committed.target_key();
        let view = old_committed.view();
        let view_revision = old_committed.view_revision();
        assert!(observed_source.observe);
        let production_source = Arc::new(MaterialSceneSource3D {
            snapshot: observed_source.snapshot.clone(),
            model: observed_source.model.clone(),
            hdr: crate::three_d::hdr::HdrSource3D {
                identity: observed_source.hdr.identity,
                revision: observed_source.hdr.revision.checked_add(1).unwrap(),
                exposure_ev: observed_source.hdr.exposure_ev,
                maximum_rgb: observed_source.hdr.maximum_rgb,
            },
            samples: observed_source.samples,
            direct: observed_source.direct.clone(),
            observe: false,
            indirect: observed_source.indirect.clone(),
        });
        assert!(Arc::ptr_eq(
            &production_source.snapshot,
            &observed_source.snapshot
        ));
        assert!(Arc::ptr_eq(
            &production_source.model,
            &observed_source.model
        ));
        assert!(!production_source.observe);
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 MASK PRODUCTION PREDECLARED {adapter:?} N{n}: SAME snapshot/model/material/1024-square MaskBase chain/LinearMipLinear Clamp/cutoff.5/view32; observe=false HDR#{} rev{}; FP16(14,16)=RGBA0/ImageWHITE, FP16(15,16) alpha1/red(.199,.201), Image alpha255/red(0,255)/distinct from WHITE; independent emission[.25,.5,.75] unchanged, final green/blue are not asserted zero; source-stage green0 remains separate; WHITE tint/background at exact texel-center UVs\n",
            production_source.hdr.identity, production_source.hdr.revision,
        )).unwrap();
        assert!(
            viewport
                .set_material_source_for_test(production_source.clone(), view_revision, view)
                .unwrap()
        );
        r024_prepare(&mut viewport, &metrics);
        assert!(
            viewport
                .material_observation_for_test(metrics.surface_id)
                .is_none()
        );
        let status = viewport.status_for(metrics.surface_id).unwrap();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        let requested = status.requested_material().unwrap();
        let committed = status.committed_material().unwrap();
        assert!(Arc::ptr_eq(requested.source(), &production_source));
        assert!(Arc::ptr_eq(committed.source(), &production_source));
        assert_eq!(committed.structural_revision(), structural_revision);
        assert_eq!(committed.model_revision(), model_revision);
        assert_eq!(committed.view(), view);
        assert_eq!(committed.view_revision(), view_revision);
        assert_eq!(committed.target_key(), key);
        assert_eq!(requested.target_key(), key);
        assert_eq!(key.surface_id(), metrics.surface_id);
        assert_eq!(key.physical_render_extent(), PhysicalExtent::new(32, 32));
        assert_eq!(key.color_format(), wgpu::TextureFormat::Rgba16Float);
        assert_eq!(key.sample_count().value(), n);
        let serial = committed.request_serial().value();
        assert_eq!(serial, old_serial.checked_add(1).unwrap());
        assert_eq!(requested.request_serial(), committed.request_serial());
        let front = viewport
            .target_texture_id_for_test(metrics.surface_id)
            .unwrap();
        let discarded = read_r027_fp16_at(
            viewport.hdr_front_for_test(metrics.surface_id),
            &gpu,
            14,
            16,
        );
        let surviving = read_r027_fp16_at(
            viewport.hdr_front_for_test(metrics.surface_id),
            &gpu,
            15,
            16,
        );
        renderer.drain_pending_image_registrations();
        let discarded_image = renderer.render_registered_image_pixel_for_test(
            front,
            [14.5 / 32.0, 16.5 / 32.0],
            [1.0; 4],
            wgpu::Color::WHITE,
        );
        let surviving_image = renderer.render_registered_image_pixel_for_test(
            front,
            [15.5 / 32.0, 16.5 / 32.0],
            [1.0; 4],
            wgpu::Color::WHITE,
        );
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 MASK PRODUCTION device={} backend={:?} N{n}: observe=false SAME snapshot/model {structural_revision:?}/{model_revision:?} HDR#{} rev{} request{serial} key={key:?} front#{front}; x14 FP16={discarded:?}/WHITE Image={discarded_image:?}; x15 FP16={surviving:?}/WHITE Image={surviving_image:?}\n",
            adapter.name, adapter.backend, production_source.hdr.identity, production_source.hdr.revision,
        )).unwrap();
        assert_eq!(discarded, [0.0; 4]);
        assert_eq!(surviving[3], 1.0);
        assert!(surviving[0] > 0.199 && surviving[0] < 0.201);
        assert_eq!(discarded_image, [255; 4]);
        assert_eq!(surviving_image[3], 255);
        assert!(surviving_image[0] > 0 && surviving_image[0] < 255);
        assert_ne!(surviving_image, discarded_image);
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
                .request_serial()
                .value(),
            serial,
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        let usage = context.resource_usage().unwrap();
        assert_eq!((usage.pending_requests, usage.pending_publications), (0, 0));
    }
}

#[test]
fn r028_actual_zero_large_uv_prewrite_refusal_and_target_sample_resize_absence_n1_n4() {
    use super::three_d_context::r028_fixture as p;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode::Repeat, SamplerFilterMode::Linear};
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual numeric/target Device required");
    let adapter = renderer.headless_adapter_for_test().clone();
    let gpu = renderer.gpu_resources();
    let successor = |context: &Context3D,
                     old: &MaterialSceneSource3D,
                     vertices: Vec<PbrVertexSource3D>,
                     placement: ModelPlacement3,
                     revision: u64| {
        let packets = old.snapshot.source_packets();
        let MaterialPacketSource3D::PbrMaterial {
            geometry,
            material,
            raster,
            stable_ordinal,
        } = &packets[0]
        else {
            panic!()
        };
        let geometry = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(vertices, GeometryIndices::Unindexed).unwrap(),
                Some(geometry),
            )
            .unwrap();
        let snapshot = context
            .admit_material_snapshot(
                StructuralRevision::new(revision),
                vec![MaterialPacketSource3D::PbrMaterial {
                    geometry,
                    material: material.clone(),
                    raster: *raster,
                    stable_ordinal: *stable_ordinal,
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
                revision,
                ..old.hdr.clone()
            },
            samples: old.samples,
            direct: None,
            indirect: None,
            observe: true,
        })
    };
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let resource = r028_role(
            &context,
            p::produce(
                MipRole3D::Emissive,
                2,
                2,
                [64, 128, 192, 255].repeat(4),
                28930,
                1,
                true,
            ),
            28931,
            MipMin3D::LinearMipLinear,
            Linear,
            Repeat,
            Repeat,
        );
        let mut material = PbrMaterialSource3D::defaults(28932, 1);
        material.sampling = SamplingFamily3D::V2;
        material.emissive_rgb = [1.0; 3];
        material.roles[4] = Some(resource.0.clone());
        let zero = r024_source(
            &context,
            r028_quad([0.0; 2], [0.0; 2]),
            material,
            n,
            true,
            r024_identity(),
            1,
        );
        let mut viewport = r024_viewport(context.clone(), zero.clone(), r024_view(2.0));
        let metrics = active_metrics(28930 + u64::from(n), 32, 32, 1.0);
        let mut inactive = metrics.clone();
        inactive.activity = CustomRenderActivity::Inactive;
        let usage = context.resource_usage().unwrap();
        let audit = crate::three_d::audit::snapshot();
        viewport.prepare_custom_render(&inactive);
        assert_eq!(context.resource_usage().unwrap(), usage);
        assert_eq!(crate::three_d::audit::snapshot(), audit);
        assert_eq!(context.target_charges_for_test(), (0, 0));
        r024_prepare(&mut viewport, &metrics);
        let o = r028_observations(&viewport, metrics.surface_id, &gpu);
        assert_eq!(o[16 * 32 + 16][9], [0.0; 4]);
        let initial = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        for (c, byte) in [64, 128, 192].into_iter().enumerate() {
            let v = p::decode(byte);
            assert!((f64::from(initial[c]) - v / (1.0 + v)).abs() < 0.001);
        }
        let large = successor(
            &context,
            &zero,
            r028_quad([4096.0; 2], [0.0; 2]),
            r024_identity(),
            2,
        );
        viewport
            .set_material_source_for_test(
                large.clone(),
                Viewport3DViewRevision::new(1),
                r024_view(2.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let o = r028_observations(&viewport, metrics.surface_id, &gpu);
        assert_eq!(o[16 * 32 + 16][9], [128.0, 0.0, 0.0, 128.0]);
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            initial
        );
        let texture_id = viewport.target_texture_id_for_test(metrics.surface_id);
        let mut next = r026_fixture::successor(&large, 3);
        next.samples = if n == 1 { 4 } else { 1 };
        next.observe = true;
        let next = Arc::new(next);
        viewport
            .set_material_source_for_test(
                next.clone(),
                Viewport3DViewRevision::new(1),
                r024_view(2.0),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        assert_ne!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            texture_id,
            "sample incompatibility must mint a target-specific identity"
        );
        let texture_id = viewport.target_texture_id_for_test(metrics.surface_id);
        let resized = active_metrics(28930 + u64::from(n), 16, 16, 1.0);
        r024_prepare(&mut viewport, &resized);
        assert_ne!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            texture_id
        );
        assert_eq!(viewport.hdr_front_for_test(metrics.surface_id).width(), 16);
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            initial
        );
        let values = r028_observations(&viewport, metrics.surface_id, &gpu);
        assert_eq!(values[8 * 16 + 8][9], [256.0, 0.0, 0.0, 256.0]);
        let before = crate::three_d::audit::snapshot();
        let usage = context.resource_usage().unwrap();
        r024_prepare(&mut viewport, &resized);
        assert_eq!(crate::three_d::audit::snapshot(), before);
        assert_eq!(context.resource_usage().unwrap(), usage);
        // The former vertex*model overflow control predicted GPU arithmetic.
        // Current admission instead checks actual submitted host data. Rebase to
        // this finite camera: the required object translation cannot fit f32.
        let invalid_origin = WorldPoint3::try_new(f64::MAX / 4.0, 0.0, 2.0).unwrap();
        let expected: crate::three_d::pbr::PbrError3D = r024_identity()
            .checked_transform(invalid_origin)
            .unwrap_err()
            .into();
        let invalid_view = ViewDescriptor3::new(
            RigidPose3::new(invalid_origin, Orientation3::identity()),
            r024_view(2.0).projection(),
        );
        let last_good_id = viewport
            .target_texture_id_for_test(metrics.surface_id)
            .unwrap();
        renderer.drain_pending_image_registrations();
        let last_good_image = renderer.render_registered_image_pixel_for_test(
            last_good_id,
            [0.5; 2],
            [1.0; 4],
            wgpu::Color::TRANSPARENT,
        );
        viewport
            .set_material_source_for_test(next.clone(), Viewport3DViewRevision::new(2), invalid_view)
            .unwrap();
        let before = crate::three_d::audit::snapshot();
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        viewport.prepare_custom_render(&resized);
        let failure = viewport
            .material_failure_for_test(metrics.surface_id)
            .expect("actual host object-translation packing failure");
        assert_eq!(failure, &expected);
        assert_eq!(
            viewport.pbr_failure(metrics.surface_id),
            Some(crate::three_d::PbrSourceError3D::from(failure.clone()))
        );
        assert_eq!(
            viewport.status_for(metrics.surface_id).unwrap().state(),
            Viewport3DLifecycleState::FailedWithLastGood
        );
        assert_eq!(crate::three_d::audit::snapshot().encodes, before.encodes);
        assert_eq!(viewport.hdr_swaps_for_test(metrics.surface_id), swaps);
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &next
        ));
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            initial
        );
        assert_eq!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            Some(last_good_id)
        );
        renderer.drain_pending_image_registrations();
        assert_eq!(
            renderer.render_registered_image_pixel_for_test(
                last_good_id,
                [0.5; 2],
                [1.0; 4],
                wgpu::Color::TRANSPARENT,
            ),
            last_good_image,
            "the actual ordinary Image must retain the complete last-good front"
        );
        for value in [f32::NAN, f32::INFINITY] {
            assert!(crate::three_d::TextureCoordinate2::try_new(value, 0.0).is_err());
        }
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 viewport host prewrite/last-good: {adapter:?}; initialN{n} failedTargetN{} at16x16; finite camera requires unrepresentable actual packed object translation; exact {expected:?}; no new encode/HDR swap; retained exact prior source Image#{last_good_id} RGBA8={last_good_image:?} FP16={initial:?}; zero/4096 UV, sample/resize/inactive/retirement checks passed; no original external consumer GPU claim\n",
            next.samples,
        )).unwrap();
    }
}

/// Test-owned producer, deliberately separate from runtime source validation.
/// The original HDR field is a+b*dot(axis, direction). Diffuse convolution is
/// analytic; the normalized GGX prefilter is evaluated by finite quadrature.
/// LUT coefficients integrate the SAME regularized Smith/Schlick reference.
/// This is a bounded analytical fixture, not an imported-asset producer claim.
pub(in crate::tests) mod r026_fixture {
    use super::*;
    use crate::three_d::indirect::*;
    use std::sync::OnceLock;

    pub fn successor(s: &MaterialSceneSource3D, revision: u64) -> MaterialSceneSource3D {
        let mut hdr = s.hdr.clone();
        hdr.revision = revision;
        MaterialSceneSource3D {
            snapshot: s.snapshot.clone(),
            model: s.model.clone(),
            hdr,
            samples: s.samples,
            direct: s.direct.clone(),
            indirect: s.indirect.clone(),
            observe: false,
        }
    }

    pub fn half_down(x: f64) -> u16 {
        assert!(x.is_finite() && (0.0..=65504.0).contains(&x));
        let b = (x as f32).to_bits();
        let exp = ((b >> 23) & 255) as i32 - 127 + 15;
        let mut h = if exp <= 0 {
            if exp < -10 {
                0
            } else {
                (((b & 0x7fffff) | 0x800000) >> (14 - exp)) as u16
            }
        } else {
            ((exp as u16) << 10) | ((b & 0x7fffff) >> 13) as u16
        };
        if f64::from(decode_half(h)) > x {
            h -= 1;
        }
        h
    }
    pub fn evidence(id: u64, derivation: SourceDerivation3D, work: u64) -> Arc<ProducerEvidence3D> {
        Arc::new(ProducerEvidence3D {
            original_identity: id,
            original_revision: 1,
            producer_identity: 26001,
            producer_revision: 1,
            evidence_identity: id,
            world_identity: 26,
            original_handedness: Some(OriginalHandedness3D::Right),
            handedness_correction: None,
            original_to_world: Some(Orientation3::identity()),
            integration_work: work,
            // Deliberately loose: exact source/reference coefficients both lie
            // in [0,1]; no convergence difference is mislabeled a strict error.
            // Pixel-level tolerances below use the actual quantized assets.
            absolute_error_bound: 1.0,
            derivation: Arc::new(derivation),
        })
    }
    pub fn cube_direction(face: usize, x: u32, y: u32, n: u32) -> glam::DVec3 {
        let u = 2.0 * (f64::from(x) + 0.5) / f64::from(n) - 1.0;
        let v = 2.0 * (f64::from(y) + 0.5) / f64::from(n) - 1.0;
        glam::DVec3::from_array(match face {
            0 => [1.0, -v, -u],
            1 => [-1.0, -v, u],
            2 => [u, 1.0, v],
            3 => [u, -1.0, -v],
            4 => [u, -v, 1.0],
            5 => [-u, -v, -1.0],
            _ => panic!(),
        })
        .normalize()
    }
    fn lut_value(nv: f64, r: f64, samples: u32) -> [f64; 2] {
        let r = r.max(0.045);
        let a2 = r.powi(4);
        let vx = (1.0 - nv * nv).sqrt();
        let mut ab = [0.0; 2];
        for i in 0..samples {
            let u = (f64::from(i) + 0.5) / f64::from(samples);
            let v = f64::from(i.reverse_bits()) / 4294967296.0;
            let nh = ((1.0 - v) / (1.0 + (a2 - 1.0) * v)).sqrt();
            let vh = vx * (1.0 - nh * nh).sqrt() * (std::f64::consts::TAU * u).cos() + nv * nh;
            let nl = 2.0 * vh * nh - nv;
            if nl > 0.0 && vh > 0.0 {
                let g1 = |x: f64| 2.0 * x / (x + (a2 + (1.0 - a2) * x * x).sqrt());
                let weight = g1(nl) * g1(nv) * vh / (nh * nv);
                let fc = (1.0 - vh.clamp(0.0, 1.0)).powi(5);
                ab[0] += (1.0 - fc) * weight / f64::from(samples);
                ab[1] += fc * weight / f64::from(samples);
            }
        }
        // Producer numerical approximation: project a quadrature overshoot
        // onto the energy simplex before downward binary16 quantization.
        // This records an approximation to the reference, never a runtime fix.
        let sum = ab[0] + ab[1];
        if sum > 1.0 {
            ab[0] /= sum;
            ab[1] /= sum;
        }
        ab
    }
    fn lut() -> Arc<[u16]> {
        static VALUE: OnceLock<Arc<[u16]>> = OnceLock::new();
        VALUE
            .get_or_init(|| {
                let mut bits = Vec::with_capacity(128 * 128 * 2);
                for y in 0..128 {
                    for x in 0..128 {
                        let ab =
                            lut_value((x as f64 + 0.5) / 128.0, (y as f64 + 0.5) / 128.0, 1024);
                        bits.extend(ab.map(half_down));
                    }
                }
                // Independent smooth endpoint integral: at nv=r=1 the reference
                // total is 1-ln(2), not an arbitrary hand-authored A/B pair.
                let edge = lut_value(1.0, 1.0, 16384);
                assert!((edge[0] + edge[1] - (1.0 - 2.0f64.ln())).abs() < 0.0001);
                bits.into()
            })
            .clone()
    }
    pub fn assets(axis: glam::DVec3, b: f64, a: [f64; 3]) -> [Arc<[u16]>; 3] {
        let mut diffuse = Vec::with_capacity(32 * 32 * 6 * 4);
        for face in 0..6 {
            for y in 0..32 {
                for x in 0..32 {
                    let d = cube_direction(face, x, y, 32);
                    for c in 0..3 {
                        diffuse.push(half_down(
                            std::f64::consts::PI * a[c]
                                + 2.0 * std::f64::consts::PI / 3.0 * b * axis.dot(d),
                        ));
                    }
                    diffuse.push(0); // E alpha is deliberately nonsemantic.
                }
            }
        }
        let mut spec = Vec::with_capacity(128 * 128 * 48 * 4);
        for level in 0..8 {
            let r = 0.045 + 0.955 * level as f64 / 7.0;
            let a2 = r.powi(4);
            let mut numerator = 0.0;
            let mut denominator = 0.0;
            for i in 0..16384 {
                let u = (i as f64 + 0.5) / 16384.0;
                let q = (1.0 - u) / (1.0 + (a2 - 1.0) * u);
                let nl = 2.0 * q - 1.0;
                if nl > 0.0 {
                    numerator += nl * nl;
                    denominator += nl;
                }
            }
            let mu = numerator / denominator;
            if level == 7 {
                assert!((mu - 2.0 / 3.0).abs() < 0.0001);
            }
            for face in 0..6 {
                for y in 0..128 {
                    for x in 0..128 {
                        let d = cube_direction(face, x, y, 128);
                        for c in 0..3 {
                            spec.push(half_down(a[c] + mu * b * axis.dot(d)));
                        }
                        spec.push(0);
                    }
                }
            }
        }
        [diffuse.into(), spec.into(), lut()]
    }
    pub fn environment(
        context: &Context3D,
        id: u64,
        axis: glam::DVec3,
        b: f64,
        a: [f64; 3],
    ) -> Arc<EnvironmentSource3D> {
        environment_mapped(context, id, axis, b, a, Orientation3::identity())
    }
    pub fn environment_mapped(
        context: &Context3D,
        id: u64,
        axis: glam::DVec3,
        b: f64,
        a: [f64; 3],
        mapping: Orientation3,
    ) -> Arc<EnvironmentSource3D> {
        let q = glam::DQuat::from_xyzw(
            f64::from(mapping.x()),
            f64::from(mapping.y()),
            f64::from(mapping.z()),
            f64::from(mapping.w()),
        )
        .normalize();
        let assets = assets(q * axis, b, a);
        let derivation = SourceDerivation3D::Environment {
            method: Arc::from(
                "analytic linear WORLD HDR; exact cosine diffuse; normalized GGX quadrature 16384; regularized Smith/Schlick split-sum 1024 Hammersley; downward binary16",
            ),
            original_field: vec![a[0], a[1], a[2], b, axis.x, axis.y, axis.z].into(),
            reference_observations: vec![
                std::f64::consts::PI,
                2.0 / 3.0,
                1.0 - 2.0f64.ln(),
                1024.0,
                16384.0,
            ]
            .into(),
            derived: assets.each_ref().map(Arc::downgrade),
        };
        let mut evidence = evidence(id, derivation, 128 * 128 * 1024 + 8 * 16384);
        Arc::get_mut(&mut evidence).unwrap().original_to_world = Some(mapping);
        let mut handles = Vec::new();
        for (role, bits) in [
            EnvironmentRole3D::Diffuse,
            EnvironmentRole3D::Specular,
            EnvironmentRole3D::BrdfLut,
        ]
        .into_iter()
        .zip(assets)
        {
            handles.push(Some(
                context
                    .register_environment_asset(
                        id + role as u64,
                        1,
                        role,
                        evidence.clone(),
                        bits,
                        None,
                    )
                    .unwrap(),
            ));
        }
        let sampler = context
            .register_sampler(crate::three_d::Sampler3D::LINEAR_CLAMP)
            .unwrap();
        Arc::new(
            EnvironmentSource3D::from_handles(
                context,
                evidence,
                handles.try_into().ok().unwrap(),
                Some(sampler.handle().clone()),
            )
            .unwrap(),
        )
    }
    pub fn attach(
        context: &Context3D,
        scene: &MaterialSceneSource3D,
        environment: Option<Arc<EnvironmentSource3D>>,
        g: bool,
        revision: u64,
    ) -> Arc<IndirectSource3D> {
        let gi = if g {
            Some(scene.snapshot.source_packets().iter().enumerate().map(|(index,p)| {
            let MaterialPacketSource3D::PbrMaterial{stable_ordinal:ordinal,..}=p else{return None;};
            // Two explicitly specified patches: T=[[0,1/4],[1/4,0]].
            // Independent emitted L0=(a,2a), then L1=(a/2,a/4),
            // L2=(a/16,a/8). Store receiver patch0's L1+L2, not L0.
            let first=std::array::from_fn(|l|[0.5*(l+1+index) as f64/4.0;3]);
            let second=std::array::from_fn(|l|[0.0625*(l+1+index) as f64/4.0;3]);
            for l in 0..6 {let a=(l+1+index)as f64/4.0;
                let t=|v:[f64;2]|[v[1]/4.0,v[0]/4.0];
                let l1=t([a,2.0*a]);let l2=t(l1);
                assert_eq!(first[l],[l1[0];3]);assert_eq!(second[l],[l2[0];3]);
            }
            let lobes=std::array::from_fn(|l|std::array::from_fn(|c|(first[l][c]+second[l][c])as f32));
            let evidence=evidence(29000+index as u64,SourceDerivation3D::StaticTransport {
                method:Arc::from("two unit-area mutually visible diffuse patches, rho*formfactor=1/4; finite two-application analytical transport, six WORLD original directions"),
                original_transport_field:vec![0.0,0.25,0.25,0.0,0.25,0.5,0.75,1.0,1.25,1.5,2.0].into(),first,second,
            },12);
            let handle=context.register_gi_source(GiSource3D {
                identity:30000+index as u64,revision,packet:*ordinal,patch:index as u64+1,evidence,
                dependencies:GiDependencies3D{snapshot:scene.snapshot.clone(),model:scene.model.clone(),direct:scene.direct.clone(),environment:environment.clone(),visibility_revision:1},
                transport_source:1,first_transport:2,second_transport:3,lobes,
            },None).unwrap();
            Some(context.lease_gi_source(&handle).unwrap())
        }).collect::<Vec<_>>().into())
        } else {
            None
        };
        Arc::new(IndirectSource3D {
            identity: 26000,
            revision,
            world_identity: 26,
            environment,
            gi,
            visibility_revision: 1,
        })
    }
    pub fn scene(context: &Context3D, n: u32, bits: u8) -> Arc<MaterialSceneSource3D> {
        let mut material = PbrMaterialSource3D::defaults(26, 1);
        material.base_rgba = [0.5, 0.5, 0.5, 1.0];
        material.metallic = 0.0;
        material.emissive_rgb = [0.125; 3];
        let mut scene = r024_source(
            context,
            super::super::three_d_geometry::r024_vertices(),
            material,
            n,
            false,
            r024_identity(),
            1,
        );
        let s = Arc::get_mut(&mut scene).unwrap();
        // Whole-domain certificate, not sampled pixels: r=1 response*cos<=2,
        // directional E<=1; emission<=1/8; E diffuse<=3pi/4,
        // spec<=3/4, each G lobe<=27/32. Even the looser mechanical
        // f32 component bound is <10, with ample representational slack to64.
        s.hdr.maximum_rgb = [64.0; 3];
        if bits & 1 != 0 {
            s.direct = Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
                identity: 7026,
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
        }
        if bits & 6 != 0 {
            let e = (bits & 2 != 0)
                .then(|| environment(context, 26100, glam::DVec3::X, 0.0, [0.25, 0.5, 0.75]));
            s.indirect = Some(attach(context, s, e, bits & 4 != 0, 1));
        }
        scene
    }
}

#[test]
fn pbr_unlinked_material_source_all_eight_presence_rows_n1_n4() {
    use crate::three_d as api;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("public eight-row proof requires real Metal");
    let adapter = renderer.headless_adapter_for_test().clone();
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_eq!(adapter.name, "Apple M4 Max");
    let gpu = renderer.gpu_resources();
    let evidence = |id| api::PbrProducerEvidence3D {
        original_identity: id,
        original_revision: 1,
        producer_identity: 98000,
        producer_revision: 1,
        evidence_identity: id,
        world_identity: 26,
        original_handedness: Some(api::PbrOriginalHandedness3D::Right),
        handedness_correction: None,
        original_to_world: Some(Orientation3::identity()),
        integration_work: 16384,
        absolute_error_bound: 1.0,
    };
    for n in [1, 4] {
        for bits in 0..8u8 {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let geometry = consumer_pbr_geometry(&context);
            let seed = consumer_pbr_source(&context, &geometry, n, false, 1);
            let snapshot = seed.snapshot();
            let model = seed.model();
            let direct = if bits & 1 != 0 { seed.direct() } else { None };
            let environment = if bits & 2 != 0 {
                // Existing independently source-tested analytic field L(w)=a+.125X,
                // cosine/GGX preintegration and numerical LUT; no invented assets.
                let assets = r026_fixture::assets(glam::DVec3::X, 0.125, [0.25, 0.5, 0.75]);
                let input = api::PbrEnvironmentInput3D::capture(
                    evidence(98100),
                    Arc::from(
                        "analytic L(w)=a+.125X; existing source-tested cosine/GGX integrations",
                    ),
                    Arc::from([0.25, 0.5, 0.75, 0.125, 1.0, 0.0, 0.0]),
                    Arc::from([2.0 / 3.0, 1.0 - 2.0f64.ln()]),
                    assets,
                );
                let mut handles = Vec::new();
                for (j, role) in [
                    api::PbrEnvironmentRole3D::Diffuse,
                    api::PbrEnvironmentRole3D::Specular,
                    api::PbrEnvironmentRole3D::BrdfLut,
                ]
                .into_iter()
                .enumerate()
                {
                    handles.push(
                        context
                            .admit_pbr_environment_asset(&input, 98110 + j as u64, 1, role, None)
                            .unwrap(),
                    );
                }
                let sampler = context
                    .register_sampler(api::Sampler3D::LINEAR_CLAMP)
                    .unwrap();
                assert!(
                    context
                        .admit_pbr_environment(
                            &input,
                            [Some(handles[0].clone()), None, Some(handles[2].clone())],
                            Some(sampler.handle().clone())
                        )
                        .is_err()
                );
                Some(
                    context
                        .admit_pbr_environment(
                            &input,
                            std::array::from_fn(|j| Some(handles[j].clone())),
                            Some(sampler.handle().clone()),
                        )
                        .unwrap(),
                )
            } else {
                None
            };
            let gi = if bits & 4 != 0 {
                // Six finite independent WORLD patches share the stated toy operator
                // T(x)=x/4: exactly L1=T(L0), L2=T(L1), storedG=L1+L2 (no L0).
                let l0 = [1.0f64, 0.5, 0.25];
                let first = [l0.map(|v| v / 4.0); 6];
                let second = first.map(|v| v.map(|v| v / 4.0));
                let lobes = std::array::from_fn(|j| {
                    std::array::from_fn(|k| (first[j][k] + second[j][k]) as f32)
                });
                let input = api::PbrTransportInput3D::capture(
                    evidence(98200),
                    Arc::from("finite six-WORLD-lobe toy T(x)=x/4 applied twice"),
                    Arc::from([0.25, 1.0, 0.5, 0.25]),
                    first,
                    second,
                );
                let g = context
                    .admit_pbr_gi(
                        api::PbrGiDescriptor3D {
                            identity: 98210,
                            revision: 1,
                            packet: 0,
                            patch: 1,
                            evidence: input,
                            dependencies: api::PbrGiDependencies3D {
                                snapshot: snapshot.clone(),
                                model: model.clone(),
                                direct: direct.clone(),
                                environment: environment.clone(),
                                visibility_revision: 1,
                            },
                            transport_source: 98220,
                            first_transport: 98221,
                            second_transport: 98222,
                            lobes,
                        },
                        None,
                    )
                    .unwrap();
                Some(Arc::from([Some(g)]))
            } else {
                None
            };
            let indirect = if bits & 6 != 0 {
                Some(
                    context
                        .capture_pbr_indirect(api::PbrIndirectDescriptor3D {
                            identity: 98300,
                            revision: 1,
                            world_identity: 26,
                            environment,
                            gi,
                            visibility_revision: 1,
                        })
                        .unwrap(),
                )
            } else {
                None
            };
            // Even the full role maxima with r=1,c=.5 give diffuse<2608, spec<=4096,
            // direct<=2, emission0;60000 is a conservative ALL-sample source bound.
            let source = context
                .admit_pbr_scene(api::PbrSceneDescriptor3D {
                    snapshot,
                    model,
                    hdr: api::PbrHdrSource3D {
                        identity: 98400,
                        revision: 2,
                        exposure_ev: 0.0,
                        maximum_rgb: [60000.0; 3],
                    },
                    samples: SampleCount3D::try_new(n).unwrap(),
                    direct,
                    indirect,
                })
                .unwrap();
            assert_eq!(
                source.presence(),
                (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0)
            );
            let mut viewport = Viewport3D::new_pbr(
                context.clone(),
                source.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
            let metrics = active_metrics(98400 + u64::from(bits), 16, 16, 1.0);
            r024_prepare(&mut viewport, &metrics);
            let pixel = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
            assert!(pixel.iter().all(|v| v.is_finite()));
            assert_eq!(pixel[3], 1.0);
            if bits == 0 {
                assert_eq!(pixel, [0.0, 0.0, 0.0, 1.0]);
            } else {
                assert!(pixel[..3].iter().all(|v| *v > 0.0 && *v <= 1.0));
            }
            assert!(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .pbr_committed()
                    .unwrap()
                    .source()
                    .same_identity(&source)
            );
            let audit = custom_render_audit::snapshot();
            let usage = context.resource_usage().unwrap();
            viewport.prepare_custom_render(&metrics);
            assert_eq!(custom_render_audit::snapshot(), audit);
            assert_eq!(context.resource_usage().unwrap(), usage);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("PUBLIC PBR D/E/G bits{bits:03b} N{n} {adapter:?}: FP16={pixel:?}; exact world/transport source, warm=0\n")).unwrap();
            viewport.on_unmount();
            drop(viewport);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert_eq!(context.target_charges_for_test(), (0, 0));
        }
    }
}

#[test]
fn r026_actual_eight_profiles_n1_n4_production_pbr_fp16_and_ordinary_image() {
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R026: real Device required");
    let gpu = renderer.gpu_resources();
    let counts = || (
        MaterialSceneSource3D::full_validation_count_for_test(),
        MaterialSceneSource3D::full_indirect_validation_count_for_test(),
    );
    let adapter = renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!("R026 actual Device {adapter:?}; wgpu30.0.2/Naga30.0.2\n"),
    )
    .unwrap();
    for n in [1, 4] {
        for bits in 0..8 {
            let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
            let source = r026_fixture::scene(&context, n, bits);
            let before = counts();
            let viewport = Viewport3D::new_pbr(
                context.clone(),
                crate::three_d::PbrScene3D(source.clone()),
                Viewport3DViewRevision::new(1),
                r025_view(),
            ).unwrap();
            let admitted = counts();
            assert_eq!(admitted, (before.0 + 1, before.1 + usize::from(bits & 6 != 0)));
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
            assert_eq!(
                viewport.status_for(surface).unwrap().state(),
                Viewport3DLifecycleState::Committed,
                "profile{bits} N{n}: {:?}",
                viewport.material_failure_for_test(surface)
            );
            assert_eq!(counts(), admitted, "cold profile{bits} N{n} repeated source validation");
            assert!(viewport.material_observation_for_test(surface).is_none());
            let final_pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
            assert_eq!(final_pixel[3], 1.0);
            assert!(
                final_pixel[..3]
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
            );
            if bits == 0 {
                for c in 0..3 {
                    assert!((final_pixel[c] - 1.0 / 9.0).abs() < 0.0002);
                }
            }
            if bits & 2 != 0 {
                assert!(final_pixel[0] < final_pixel[1] && final_pixel[1] < final_pixel[2]);
            }
            let usage = context.resource_usage().unwrap();
            let pool = if bits & 1 != 0 {
                crate::three_d::direct_shadow::POOL_BYTES
            } else {
                0
            };
            assert!(
                usage.resident_texture_bytes
                    >= pool
                        + if bits & 2 != 0 {
                            crate::three_d::indirect::ENVIRONMENT_BYTES
                        } else {
                            0
                        }
            );
            if bits & 2 == 0 {
                assert!(
                    usage.resident_texture_bytes < pool + 1024,
                    "absent E must allocate no float assets: {usage:?}"
                );
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
            for c in 0..3 {
                let v = f32::from(composed[c]) / 255.0;
                let linear = if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                };
                assert!(
                    (linear - final_pixel[c]).abs() < 0.02,
                    "profile{bits} N{n} final{final_pixel:?} Image{composed:?}"
                );
            }
            assert_eq!(composed[3], 255);
            std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R026 actual DEG bits={bits:03b} N{n} FP16={final_pixel:?} WHITE Image={composed:?}; textures={}B\n",usage.resident_texture_bytes)).unwrap();
            drop(compiled);
            {
                let viewport = tree.node_mut(child).unwrap().widget.as_any_mut()
                    .downcast_mut::<Viewport3D>().unwrap();
                let metrics = active_metrics_for_surface(surface, 32, 32, 1.0);
                let view = |degrees: f32| ViewDescriptor3::new(
                    r025_view().pose(),
                    PerspectiveProjection::try_finite(degrees.to_radians(), 0.1, 10.0).unwrap().into(),
                );
                let observe = |viewport: &Viewport3D, renderer: &mut WgpuRenderer| {
                    let pixel = read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu);
                    assert_eq!(pixel[3], 1.0);
                    assert!(pixel.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                    renderer.drain_pending_image_registrations();
                    let image = renderer.render_registered_image_pixel_for_test(
                        viewport.target_texture_id_for_test(surface).unwrap(),
                        [16.5 / 32.0; 2], [1.0; 4], wgpu::Color::WHITE,
                    );
                    for c in 0..3 {
                        let v = f32::from(image[c]) / 255.0;
                        let linear = if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
                        assert!((linear - pixel[c]).abs() < 0.02);
                    }
                    assert_eq!(image[3], 255);
                    (pixel.map(f32::to_bits), image)
                };
                let mut serial = viewport.status_for(surface).unwrap()
                    .committed_material().unwrap().request_serial().value();
                for (i, fov) in [52.0, 48.0, 44.0, 48.0].into_iter().enumerate() {
                    let before = counts();
                    let textures = crate::three_d::audit::snapshot().gpu_textures;
                    viewport.set_pbr_timing_for_test(true);
                    viewport.request_pbr_scene(
                        crate::three_d::PbrScene3D(source.clone()),
                        Viewport3DViewRevision::new(2 + i as u64), view(fov),
                    ).unwrap();
                    r024_prepare(viewport, &metrics);
                    assert_eq!(counts(), before, "profile{bits} N{n} FOV{fov} repeated source/indirect validation");
                    assert_eq!(crate::three_d::audit::snapshot().gpu_textures, textures,
                        "unchanged source assets were rematerialized");
                    let t = viewport.pbr_timing_for_test().unwrap();
                    assert!(t.complete);
                    assert_no_runtime_gpu_model(&t.raster);
                    assert_eq!((t.raster.geometry_plans, t.raster.mip_plans), (0, 0));
                    let current = viewport.status_for(surface).unwrap().committed_material().unwrap();
                    assert!(Arc::ptr_eq(current.source(), &source));
                    assert_eq!(current.view_revision(), Viewport3DViewRevision::new(2 + i as u64));
                    assert_eq!(current.view(), view(fov));
                    assert_eq!(current.request_serial().value(), serial + 1);
                    serial += 1;
                    assert_eq!(current.target_key().sample_count().value(), n);
                    let shadow = viewport.direct_proof_for_test(surface);
                    assert_eq!(shadow.is_some(), bits & 1 != 0);
                    if let Some(shadow) = shadow {
                        assert_eq!(shadow.faces, [0, 1, 2]);
                        assert!(shadow.passes_encoded >= 3);
                        assert_eq!(t.raster.shadow_faces, 3);
                    }
                    observe(viewport, &mut renderer);
                }
                let old_output = observe(viewport, &mut renderer);
                let mut next = r026_fixture::successor(&source, 2);
                if let Some(indirect) = &source.indirect {
                    next.indirect = Some(Arc::new(crate::three_d::indirect::IndirectSource3D {
                        identity: indirect.identity, revision: indirect.revision + 1,
                        world_identity: indirect.world_identity,
                        environment: indirect.environment.clone(), gi: indirect.gi.clone(),
                        visibility_revision: indirect.visibility_revision,
                    }));
                }
                let next = Arc::new(next);
                assert!(!Arc::ptr_eq(&source, &next));
                let before = counts();
                viewport.request_pbr_scene(
                    crate::three_d::PbrScene3D(next.clone()), Viewport3DViewRevision::new(5), view(48.0),
                ).unwrap();
                let successor_counts = counts();
                assert_eq!(successor_counts, (before.0 + 1, before.1 + usize::from(bits & 6 != 0)));
                r024_prepare(viewport, &metrics);
                assert_eq!(counts(), successor_counts);
                let current = viewport.status_for(surface).unwrap().committed_material().unwrap();
                assert!(Arc::ptr_eq(current.source(), &next));
                assert_eq!(current.source().hdr.revision, 2);
                assert_eq!(current.view_revision(), Viewport3DViewRevision::new(5));
                assert_eq!(current.request_serial().value(), serial + 1);
                assert_eq!(observe(viewport, &mut renderer), old_output,
                    "revision-only source successor changed exact FP16/Image");
                viewport.request_pbr_scene(
                    crate::three_d::PbrScene3D(next.clone()), Viewport3DViewRevision::new(6), view(40.0),
                ).unwrap();
                r024_prepare(viewport, &metrics);
                assert_eq!(counts(), successor_counts, "successor FOV repeated source validation");
                let current = viewport.status_for(surface).unwrap().committed_material().unwrap();
                assert!(Arc::ptr_eq(current.source(), &next));
                assert_eq!(current.view_revision(), Viewport3DViewRevision::new(6));
                assert_eq!(current.view(), view(40.0));
                assert_eq!(current.request_serial().value(), serial + 2);

                if bits == 2 || bits == 7 {
                    let old_output = observe(viewport, &mut renderer);
                    let front = viewport.target_texture_id_for_test(surface);
                    let serial = viewport.status_for(surface).unwrap()
                        .committed_material().unwrap().request_serial();
                    let mut invalid = r026_fixture::successor(&next, 3);
                    let old = next.indirect.as_ref().unwrap();
                    invalid.indirect = Some(Arc::new(crate::three_d::indirect::IndirectSource3D {
                        identity: old.identity, revision: old.revision + 1, world_identity: 0,
                        environment: old.environment.clone(), gi: old.gi.clone(),
                        visibility_revision: old.visibility_revision,
                    }));
                    let invalid = Arc::new(invalid);
                    let expected: crate::three_d::PbrSourceError3D =
                        invalid.validate(&context).unwrap_err().into();
                    let before = counts();
                    let audit = custom_render_audit::snapshot();
                    assert_eq!(viewport.request_pbr_scene(
                        crate::three_d::PbrScene3D(invalid),
                        Viewport3DViewRevision::new(7), view(40.0),
                    ).unwrap_err(), expected);
                    assert_eq!(counts(), (before.0 + 1, before.1 + 1));
                    assert_eq!(custom_render_audit::snapshot(), audit);
                    assert_eq!(viewport.target_texture_id_for_test(surface), front);
                    let committed = viewport.status_for(surface).unwrap().committed_material().unwrap();
                    assert_eq!(committed.request_serial(), serial);
                    assert!(Arc::ptr_eq(committed.source(), &next));
                    assert_eq!(observe(viewport, &mut renderer), old_output);
                }
                std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                    "R026 SOURCE CAPTURE N{n} bits{bits:03b}: initial_scene=1 initial_indirect={} cold_repeats=0 FOV52/48/44/48_scene_indirect_repeats=0/0 successor_scene=1 successor_indirect={} successor_FOV_repeats=0; current_shadow_faces={} actual_FP16_Image=true\n",
                    usize::from(bits & 6 != 0), usize::from(bits & 6 != 0), if bits & 1 != 0 { 3 } else { 0 },
                )).unwrap();
            }
            drop(tree);
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
}

#[test]
fn r026_actual_source_capture_rejects_foreign_and_retired_context_n1_n4() {
    use crate::three_d::pbr::{ValidatedMaterialSceneSource3D, validate_material_scene_source};
    for n in [1, 4] {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        assert_eq!(renderer.headless_adapter_for_test().backend, wgpu::Backend::Metal);
        let gpu = renderer.gpu_resources();
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let source = r026_fixture::scene(&context, n, 7);
        let proof = ValidatedMaterialSceneSource3D::new(&context, source.clone()).unwrap();
        assert!(proof.matches(&context, &source));
        let foreign = Context3D::new(gpu.clone()).unwrap();
        let expected = source.validate(&foreign).unwrap_err();
        assert!(!proof.matches(&foreign, &source));
        let before = MaterialSceneSource3D::full_validation_count_for_test();
        let indirect = MaterialSceneSource3D::full_indirect_validation_count_for_test();
        assert_eq!(validate_material_scene_source(&foreign, &source, Some(&proof)), Err(expected));
        assert_eq!(MaterialSceneSource3D::full_validation_count_for_test(), before + 1);
        assert_eq!(MaterialSceneSource3D::full_indirect_validation_count_for_test(), indirect);

        let mut viewport = Viewport3D::new_pbr(
            context.clone(), crate::three_d::PbrScene3D(source.clone()),
            Viewport3DViewRevision::new(1), r025_view(),
        ).unwrap();
        let metrics = active_metrics(26500 + u64::from(n), 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        renderer.drain_pending_image_registrations();
        assert!(viewport.target_texture_id_for_test(metrics.surface_id).is_some());
        drop(renderer);
        assert!(!proof.matches(&context, &source), "closed renderer owner authenticated source reuse");
        assert!(viewport.target_texture_id_for_test(metrics.surface_id).is_none());
        let audit = custom_render_audit::snapshot();
        viewport.prepare_custom_render(&metrics);
        assert_eq!(viewport.status_for(metrics.surface_id).unwrap().state(), Viewport3DLifecycleState::Retired);
        let after = custom_render_audit::snapshot();
        assert_eq!(after.custom_submissions, audit.custom_submissions);
        assert_eq!(after.custom_command_encoders, audit.custom_command_encoders);
        viewport.prepare_custom_render(&metrics);
        assert_eq!(custom_render_audit::snapshot(), after);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R026 SOURCE CUSTODY N{n}: full actual D/E/G capture; foreign Context on same Device misses and preserves original first refusal; actual renderer shutdown invalidates capture and withdraws front; retired preparation submits/encodes=0\n",
        )).unwrap();
    }
}

#[test]
fn r026_actual_independent_e_exposure_and_d_revisions_reuse_only_current_shadow_maps() {
    use r026_fixture as f;
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R026: actual revision Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let mut source = f::scene(&context, n, 3);
        Arc::get_mut(Arc::get_mut(&mut source).unwrap().direct.as_mut().unwrap())
            .unwrap()
            .identity = 700;
        let mut viewport = r024_viewport(context.clone(), source.clone(), r025_view());
        let metrics = active_metrics(2601, 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let first = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        let old_front = viewport.target_texture_id_for_test(metrics.surface_id);
        let e = f::environment(&context, 28000, glam::DVec3::X, 0.0, [0.75; 3]);
        let mut next = f::successor(&source, 2);
        next.indirect = Some(f::attach(&context, &next, Some(e), false, 2));
        let next = Arc::new(next);
        assert!(Arc::ptr_eq(
            source.direct.as_ref().unwrap(),
            next.direct.as_ref().unwrap()
        ));
        viewport
            .set_material_source_for_test(next.clone(), Viewport3DViewRevision::new(1), r025_view())
            .unwrap();
        assert_eq!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            old_front,
            "source Update does not relabel/write the old front"
        );
        r024_prepare(&mut viewport, &metrics);
        let second = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &next
        ));
        assert!(second[0] > first[0]);
        let after = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        assert_eq!(proof.pool_identity, after.pool_identity);
        assert_eq!(
            proof.passes_encoded, after.passes_encoded,
            "E-only change must not rewrite unchanged D maps"
        );
        let mut exposure = f::successor(&next, 3);
        exposure.hdr.exposure_ev = 1.0;
        let exposure = Arc::new(exposure);
        viewport
            .set_material_source_for_test(
                exposure.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let exposed = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert!(exposed[0] > second[0]);
        let after_exposure = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        assert_eq!(proof.pool_identity, after_exposure.pool_identity);
        assert_eq!(proof.passes_encoded, after_exposure.passes_encoded);
        // A distinct complete source with unchanged full revision cannot publish,
        // including E changes and D presence changes.
        let mut same_rev = f::successor(&exposure, 3);
        same_rev.direct = None;
        assert!(
            viewport
                .set_material_source_for_test(
                    Arc::new(same_rev),
                    Viewport3DViewRevision::new(1),
                    r025_view()
                )
                .is_err()
        );
        let mut light = f::successor(&exposure, 4);
        let mut stale_e = f::successor(&exposure, 3);
        stale_e.indirect = Some(f::attach(
            &context,
            &stale_e,
            source.indirect.as_ref().unwrap().environment.clone(),
            false,
            3,
        ));
        assert!(
            viewport
                .set_material_source_for_test(
                    Arc::new(stale_e),
                    Viewport3DViewRevision::new(1),
                    r025_view()
                )
                .is_err()
        );
        let old = light.direct.as_ref().unwrap();
        let mut directional = old.directional.clone().unwrap();
        directional.irradiance = [2.0; 3];
        light.direct = Some(Arc::new(crate::three_d::direct_shadow::DirectSource3D {
            identity: old.identity,
            revision: 2,
            meters_per_scene_unit: old.meters_per_scene_unit,
            directional: Some(directional),
            points: old.points.clone(),
            spots: old.spots.clone(),
            snapshot: old.snapshot.clone(),
            model: old.model.clone(),
        }));
        let light = Arc::new(light);
        viewport
            .set_material_source_for_test(
                light.clone(),
                Viewport3DViewRevision::new(1),
                r025_view(),
            )
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let changed = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert!(changed[0] > exposed[0]);
        let new_proof = viewport.direct_proof_for_test(metrics.surface_id).unwrap();
        assert_ne!(proof.pool_identity, new_proof.pool_identity);
        assert!(new_proof.passes_encoded > 0);
        let usage = context.resource_usage().unwrap();
        let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
        let audit = custom_render_audit::events();
        let viewport_events = viewport_audit::snapshot();
        r024_prepare(&mut viewport, &metrics);
        assert_eq!(usage, context.resource_usage().unwrap());
        assert_eq!(swaps, viewport.hdr_swaps_for_test(metrics.surface_id));
        assert_eq!(audit, custom_render_audit::events());
        assert_eq!(viewport_events, viewport_audit::snapshot());
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R026 N{n} independent E/D/HDR revisions: {first:?}->{second:?}->{exposed:?}->{changed:?}; E/exposure shadow pool={} passes={} unchanged\n",proof.pool_identity,proof.passes_encoded)).unwrap();
        viewport.on_unmount();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn r026_actual_exhausted_e_successor_preserves_original_front_and_inflight_charges() {
    use r026_fixture as f;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R026: retirement Device required");
    let gpu = renderer.gpu_resources();
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resident_texture_bytes = crate::three_d::indirect::ENVIRONMENT_BYTES + 24;
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).unwrap());
    let source = f::scene(&context, 1, 2);
    let mut viewport = r024_viewport(context.clone(), source.clone(), r025_view());
    let metrics = active_metrics(2602, 32, 32, 1.0);
    r024_prepare(&mut viewport, &metrics);
    renderer.drain_pending_image_registrations();
    let first = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
    let swaps = viewport.hdr_swaps_for_test(metrics.surface_id);
    let id = viewport.target_texture_id_for_test(metrics.surface_id);
    let e = f::environment(&context, 28001, glam::DVec3::X, 0.0, [0.75; 3]);
    let mut next = f::successor(&source, 2);
    next.indirect = Some(f::attach(&context, &next, Some(e), false, 2));
    viewport
        .set_material_source_for_test(Arc::new(next), Viewport3DViewRevision::new(1), r025_view())
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    assert!(
        viewport
            .material_failure_for_test(metrics.surface_id)
            .is_some()
    );
    assert_eq!(swaps, viewport.hdr_swaps_for_test(metrics.surface_id));
    assert_eq!(id, viewport.target_texture_id_for_test(metrics.surface_id));
    assert!(Arc::ptr_eq(
        viewport
            .committed_material_source_for_test(metrics.surface_id)
            .unwrap(),
        &source
    ));
    assert_eq!(
        first,
        read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu)
    );
    assert_eq!(
        context.resource_usage().unwrap().resident_texture_bytes,
        policy.max_resident_texture_bytes
    );
    viewport.on_unmount();
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(
        context.resource_usage().unwrap().resident_texture_bytes,
        24,
        "only unchanged legacy fallback and retained material defaults survive retirement"
    );
}

#[test]
fn r026_actual_per_draw_world_g_slots_material_ao_and_current_bake_provenance_n1_n4() {
    use r026_fixture as f;
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R026: per-patch Device required");
    let gpu = renderer.gpu_resources();
    for n in [1, 4] {
        let context = Arc::new(Context3D::new(gpu.clone()).unwrap());
        let base = f::scene(&context, n, 6);
        let MaterialPacketSource3D::PbrMaterial {
            geometry, material, ..
        } = &base.snapshot.source_packets()[0]
        else {
            panic!()
        };
        let texture = context
            .register_texture(
                crate::three_d::Texture3D::try_new(
                    1,
                    1,
                    crate::three_d::TextureFormat3D::Rgba8Unorm,
                    vec![128, 0, 255, 0],
                )
                .unwrap(),
            )
            .unwrap();
        let sampler = context
            .register_sampler(crate::three_d::Sampler3D::LINEAR_CLAMP)
            .unwrap();
        let mut m = PbrMaterialSource3D::defaults(26, 2);
        m.base_rgba = [0.5, 0.5, 0.5, 1.0];
        m.metallic = 0.0;
        m.emissive_rgb = [0.125; 3];
        m.roles[3] = Some(PbrRoleBinding3D::V1 {
            texture: texture.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        let right = context.register_pbr_material(m, Some(material)).unwrap();
        // The retained old material lease is still usable in this captured
        // source; its handle successor is not silently substituted into it.
        // Register a separate current left identity with the same exact factors.
        let mut left = PbrMaterialSource3D::defaults(27, 1);
        left.base_rgba = [0.5, 0.5, 0.5, 1.0];
        left.metallic = 0.0;
        left.emissive_rgb = [0.125; 3];
        let left = context.register_pbr_material(left, None).unwrap();
        let snapshot = context
            .admit_material_snapshot(
                StructuralRevision::new(2),
                vec![
                    MaterialPacketSource3D::PbrMaterial {
                        geometry: geometry.clone(),
                        material: left,
                        raster: RasterMode::SingleSided,
                        stable_ordinal: 10,
                    },
                    MaterialPacketSource3D::PbrMaterial {
                        geometry: geometry.clone(),
                        material: right,
                        raster: RasterMode::SingleSided,
                        stable_ordinal: 20,
                    },
                ],
            )
            .unwrap();
        let placements = [-1.0, 1.0].map(|x| {
            ModelPlacement3::try_new(
                WorldPoint3::try_new(x, 0.0, 0.0).unwrap(),
                Orientation3::identity(),
                PositiveScale3::try_new(0.75, 0.75, 0.75).unwrap(),
            )
            .unwrap()
        });
        let model = context
            .admit_material_model(snapshot.clone(), ModelRevision::new(2), placements.to_vec())
            .unwrap();
        let mut scene = f::successor(&base, 2);
        scene.snapshot = snapshot;
        scene.model = model;
        let old_e = Some(f::environment(
            &context,
            29001,
            glam::DVec3::X,
            0.125,
            [0.25, 0.5, 0.75],
        ));
        scene.indirect = Some(f::attach(&context, &scene, old_e.clone(), true, 2));
        let scene = Arc::new(scene);
        let mut viewport = r024_viewport(context.clone(), scene.clone(), r024_view(4.0));
        let metrics = active_metrics(2603, 32, 32, 1.0);
        r024_prepare(&mut viewport, &metrics);
        let texture = viewport.hdr_front_for_test(metrics.surface_id);
        let pixels = [
            read_r027_fp16_at(texture, &gpu, 8, 16),
            read_r027_fp16_at(texture, &gpu, 24, 16),
        ];
        let assets = f::assets(glam::DVec3::X, 0.125, [0.25, 0.5, 0.75]);
        for (index, x) in [8u32, 24].into_iter().enumerate() {
            let px = (2.0 * (f64::from(x) + 0.5) / 32.0 - 1.0) * 2.0;
            let py = -0.0625;
            let nv = 2.0 / (4.0 + px * px + py * py).sqrt();
            let u = nv * 128.0 - 0.5;
            let lo = u.floor() as i32;
            let frac = u - u.floor();
            let lut = |c: usize| {
                let a = f64::from(crate::three_d::indirect::decode_half(
                    assets[2][(127 * 128 + lo.clamp(0, 127) as usize) * 2 + c],
                ));
                let b = f64::from(crate::three_d::indirect::decode_half(
                    assets[2][(127 * 128 + (lo + 1).clamp(0, 127) as usize) * 2 + c],
                ));
                a * (1.0 - frac) + b * frac
            };
            let fview = 0.04 + 0.96 * (1.0 - nv).powi(5);
            let ao = if index == 0 { 1.0 } else { 128.0 / 255.0 };
            let g = 0.5625 * (5 + index) as f64 / 4.0;
            for c in 0..3 {
                // WORLD N=+Z samples the four face4 center texels at UV(.5,.5),
                // not the first texel of the +X face of this nonconstant field.
                let e = [(15, 15), (16, 15), (15, 16), (16, 16)]
                    .into_iter()
                    .map(|(x, y)| {
                        f64::from(crate::three_d::indirect::decode_half(
                            assets[0][(4 * 32 * 32 + y * 32 + x) * 4 + c],
                        )) / 4.0
                    })
                    .sum::<f64>();
                let spec = ([0.25, 0.5, 0.75][c] + (2.0 / 3.0) * 0.125 * (px * nv / 2.0))
                    * (0.04 * lut(0) + lut(1));
                let hdr = 0.125 + ao * 0.5 * (1.0 - fview) * (e + g) / std::f64::consts::PI + spec;
                assert!(
                    (f64::from(pixels[index][c]) - hdr / (1.0 + hdr)).abs() < 0.001,
                    "N{n} slot{index} channel{c} {:?}",
                    pixels[index]
                );
            }
            assert_eq!(pixels[index][3], 1.0);
        }
        assert!(
            pixels[1][0] < pixels[0][0],
            "AO must attenuate diffuse only, not erase emission/specular"
        );
        let mut stale = f::successor(&scene, 3);
        stale.model = context
            .admit_material_model(
                scene.snapshot.clone(),
                ModelRevision::new(3),
                vec![r024_identity(), r024_identity()],
            )
            .unwrap();
        assert!(
            stale.validate(&context).is_err(),
            "moving furniture must invalidate G, E remains independently retained"
        );
        assert!(Arc::ptr_eq(
            old_e.as_ref().unwrap(),
            scene
                .indirect
                .as_ref()
                .unwrap()
                .environment
                .as_ref()
                .unwrap()
        ));
        assert!(Arc::ptr_eq(
            viewport
                .committed_material_source_for_test(metrics.surface_id)
                .unwrap(),
            &scene
        ));
        let usage = context.resource_usage().unwrap();
        let angle = std::f32::consts::PI / 12.0;
        let yaw = Orientation3::try_from_xyzw(0.0, (angle / 2.0).sin(), 0.0, (angle / 2.0).cos())
            .unwrap();
        let view = ViewDescriptor3::new(
            RigidPose3::new(WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(), yaw),
            crate::three_d::OrthographicProjection::try_new(4.0, 0.0, 10.0)
                .unwrap()
                .into(),
        );
        viewport
            .set_material_source_for_test(scene.clone(), Viewport3DViewRevision::new(2), view)
            .unwrap();
        r024_prepare(&mut viewport, &metrics);
        let current = viewport
            .committed_material_source_for_test(metrics.surface_id)
            .unwrap();
        assert!(Arc::ptr_eq(current, &scene));
        assert!(Arc::ptr_eq(
            current
                .indirect
                .as_ref()
                .unwrap()
                .environment
                .as_ref()
                .unwrap(),
            old_e.as_ref().unwrap()
        ));
        assert_eq!(
            context.resource_usage().unwrap().resident_texture_bytes,
            usage.resident_texture_bytes,
            "view yaw must reuse WORLD assets, not allocate rotated cubes"
        );
        std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R026 N{n} per-draw WORLD G10/G20 and numeric AO.R128: {pixels:?}; original E retained, moved-model G refused\n")).unwrap();
        viewport.on_unmount();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}
