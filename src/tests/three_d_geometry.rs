use crate::renderer::wgpu_renderer::WgpuRenderer;
use crate::three_d::pbr::{PbrGeometrySource3D, PbrUnitDirection3D, PbrVertexSource3D};
use crate::three_d::{
    CANONICAL_COLOR_OFFSET, CANONICAL_NORMAL_OFFSET, CANONICAL_POSITION_OFFSET,
    CANONICAL_UV_OFFSET, CANONICAL_VERTEX_STRIDE, CanonicalGeometryBytes, CanonicalIndexFormat,
    Context3D, Context3DError, GeometryIndices, GeometrySemantics, GeometrySourceDescriptor,
    LinearVertexColorRgba8, LocalPoint3, LocalVector3, PolicyBudget3D, PolicyLimits3D,
    TextureCoordinate2,
};

fn point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

pub(super) fn r024_vertices() -> Vec<PbrVertexSource3D> {
    [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]]
        .into_iter()
        .zip([[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]])
        .map(|(p, uv)| PbrVertexSource3D {
            position: point(p[0], p[1], p[2]),
            normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
            uv0: Some(TextureCoordinate2::try_new(uv[0], uv[1]).unwrap()),
            color: None,
            tangent: Some((PbrUnitDirection3D::try_new([1.0, 0.0, 0.0]).unwrap(), 1.0)),
        })
        .collect()
}

/// Analytic, test-owned signed tangent attributes, not a claim that the original external consumer's
/// external default-Mikk producer or its original furniture ran in this test.
pub(super) mod r024_abc_fixture {
    use crate::three_d::GeometryIndices;
    use crate::three_d::pbr::*;
    use std::sync::Arc;

    pub fn unit(v: [f32; 3]) -> [f32; 3] {
        let v = v.map(f64::from);
        let length = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        v.map(|x| (x / length) as f32)
    }

    pub fn packet(mode: u8, negative_uv: bool) -> (Vec<PbrVertex52>, PbrCertifiedInput3D) {
        let normals = match mode {
            0 => [[0.0, 0.0, 1.0]; 3],
            1 => [[0.0, 0.8, 0.6], [0.0, 0.7, 0.7], [0.0, 0.9, 0.6]],
            2 => [[-0.8, 0.0, 0.6], [0.8, 0.0, 0.6], [-0.8, 0.0, 0.6]],
            _ => panic!("fixture mode"),
        };
        let tangents = if mode == 2 {
            [[0.6, 0.0, 0.8], [0.6, 0.0, -0.8], [0.6, 0.0, 0.8]]
        } else {
            [[1.0, 0.0, 0.0]; 3]
        };
        let h = if negative_uv { -1.0 } else { 1.0 };
        let uv = [[0.0, 0.0], [1.0, 0.0], [0.5, h]];
        let p = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
        let identity = PbrOriginalIdentity3D {
            source: 900,
            revision: 1,
            node: 0,
            mesh: 0,
            primitive: 0,
        };
        let original = PbrOriginalGeometry3D {
            identity,
            vertices: (0..3)
                .map(|i| PbrOriginalVertex3D {
                    position: p[i],
                    normal: normals[i],
                    uv0: Some(uv[i]),
                    color: None,
                })
                .collect::<Vec<_>>()
                .into(),
            indices: GeometryIndices::Unindexed,
            signed_tangents: tangents.map(|t| [t[0], t[1], t[2], h]).into(),
            tangent_origin: PbrTangentOrigin3D::Authored,
        };
        let frame = match mode {
            0 => PbrFrameCertificate3D::OldCone,
            1 => PbrFrameCertificate3D::AuthoredSmooth {
                separator: [0.0, 0.6, -0.8],
                lower: [0.5; 9],
            },
            2 => PbrFrameCertificate3D::SourceGeometric {
                // N1 is exactly in the positive cone of T0,T1 (all y=0).
                singularity: PbrSmoothSingularity3D::VertexInCone {
                    normal_vertex: true,
                    vertex: 1,
                    basis: [0, 1, 0],
                    count: 2,
                    antiparallel: false,
                },
                separator: [0.0, 1.0, 0.0],
                lower: [0.5; 9],
            },
            _ => unreachable!(),
        };
        let render = (0..3)
            .map(|i| {
                let n = if mode == 2 {
                    [0.0, 0.0, 1.0]
                } else {
                    unit(normals[i])
                };
                let t = if mode == 2 {
                    [1.0, 0.0, 0.0]
                } else {
                    unit(tangents[i])
                };
                PbrVertex52 {
                    position: p[i],
                    normal: n,
                    uv0: uv[i],
                    color: [255; 4],
                    tangent: [t[0], t[1], t[2], h],
                }
            })
            .collect();
        (
            render,
            PbrCertifiedInput3D {
                original,
                render_vertex_sources: Arc::from([0, 1, 2]),
                triangles: Arc::from([PbrTriangleCertificate3D {
                    original: identity,
                    triangle: 0,
                    certificate_identity: 901,
                    certificate_revision: 1,
                    frame,
                }]),
                mapped: true,
                maximum_certificate_bytes: 4096,
                maximum_verification_items: 16,
            },
        )
    }

    pub fn source(mode: u8, negative_uv: bool) -> PbrGeometrySource3D {
        let (render, input) = packet(mode, negative_uv);
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap()
    }
}

#[test]
fn r024_mode_c_exact_dyadic_tangent_rounding() {
    use crate::three_d::pbr::*;
    use std::sync::Arc;

    // One complete test-owned original triangle, using the exact LOCAL
    // P/N/UV/Mikk bits reported for desk geometry2/triangle3499. This is not
    // admission of the complete external HAC or a replay of its Mikk producer.
    let positions = [
        [0x3f795570, 0x3f494cf6, 0x3ee2e39d],
        [0xb501295b, 0x3f494cf4, 0x3ee3585e],
        [0xb5012bbd, 0x3f49572c, 0x3ee34a6c],
    ]
    .map(|p| p.map(f32::from_bits));
    let normals = [
        [0xba831331, 0x3e49a152, 0xbf7afcf3],
        [0x00000000, 0x3e45887f, 0xbf7b3116],
        [0x00000000, 0x3f6d2efb, 0xbec0aa4b],
    ]
    .map(|n| n.map(f32::from_bits));
    let uv = [
        [0x3ef8835d, 0x3f0ded6a],
        [0x3ef87c44, 0x3f4613cc],
        [0x3ef89903, 0x3f4613cc],
    ]
    .map(|v| v.map(f32::from_bits));
    let mikk = [
        [0xbda3be39, 0x3f7a2e40, 0x3e4910a0, 0xbf800000],
        [0xb40c863b, 0x3f7b3116, 0x3e458882, 0xbf800000],
        [0xb49c3220, 0xbec0aa31, 0xbf6d2f01, 0xbf800000],
    ]
    .map(|t| t.map(f32::from_bits));
    let f = [0xba4605c2, 0xbf104a18, 0xbf53767b].map(f32::from_bits);
    // Exact Ux=-561175905/sqrt(9333086867122065629660356845249).
    // Its unique binary32 round cell is b4453c6c, not the former b4453c64.
    let u = [0xb4453c6c, 0x3f53767f, 0xbf104a1a].map(f32::from_bits);
    let separator =
        [0x3fefffff66d3163c, 0xbf3be49813758b64, 0xbf44732b5035f283].map(f64::from_bits);
    let (mut render, mut input) = r024_abc_fixture::packet(2, true);
    for i in 0..3 {
        let original = &mut Arc::make_mut(&mut input.original.vertices)[i];
        original.position = positions[i];
        original.normal = normals[i];
        original.uv0 = Some(uv[i]);
        Arc::make_mut(&mut input.original.signed_tangents)[i] = mikk[i];
        render[i].position = positions[i];
        render[i].uv0 = uv[i];
        render[i].normal = f;
        render[i].tangent = [u[0], u[1], u[2], -1.0];
    }
    input.original.tangent_origin = PbrTangentOrigin3D::DefaultMikk {
        producer: 902,
        revision: 1,
        version: [0, 3, 0],
    };
    let certificate = &mut Arc::make_mut(&mut input.triangles)[0];
    certificate.certificate_revision = 2;
    certificate.frame = PbrFrameCertificate3D::SourceGeometric {
        singularity: PbrSmoothSingularity3D::MixedCone {
            generators: [0, 1, 4, 5],
            reverse_coefficients: true,
            antiparallel: false,
        },
        separator,
        lower: [1.0 / 16.0; 9],
    };

    let source = PbrGeometrySource3D::try_new_certified(
        render.clone(),
        GeometryIndices::Unindexed,
        input.clone(),
    )
    .expect("the exact C tangent's correct round cell must qualify");
    assert_eq!(source.certified_input(), Some(&input));
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&source.pack().unwrap()),
        bytemuck::cast_slice::<_, u8>(&render)
    );
    let proof = source.certified_frames().unwrap()[0];
    assert!(proof.cross_lower >= 1.0 / 16.0 && proof.local_g_lower > 0.0);
    assert!(proof.maximum_direction_pack_error.is_finite());

    for wrong in [0xb4453c64, 0xb4453c6b, 0xb4453c6d] {
        let mut bad = render.clone();
        for corner in &mut bad {
            corner.tangent[0] = f32::from_bits(wrong);
        }
        assert!(matches!(
            PbrGeometrySource3D::try_new_certified(bad, GeometryIndices::Unindexed, input.clone()),
            Err(PbrError3D::FrameCertificate {
                cause: "original/derived-to-packed direction not certified",
                ..
            })
        ));
    }
    let refused = |r, i| {
        assert!(PbrGeometrySource3D::try_new_certified(r, GeometryIndices::Unindexed, i).is_err());
    };
    let mut bad = render.clone();
    for corner in &mut bad {
        corner.normal[0] = f[0].next_up();
    }
    refused(bad, input.clone());
    let mut bad = render.clone();
    for corner in &mut bad {
        corner.tangent[3] = 1.0;
    }
    refused(bad, input.clone());
    for mode in [
        PbrFrameCertificate3D::OldCone,
        PbrFrameCertificate3D::AuthoredSmooth {
            separator,
            lower: [1.0 / 16.0; 9],
        },
        PbrFrameCertificate3D::NormalOnly {
            separator,
            lower: [1.0 / 16.0; 3],
        },
    ] {
        let mut bad = input.clone();
        Arc::make_mut(&mut bad.triangles)[0].frame = mode;
        refused(render.clone(), bad);
    }
    let mut bad_input = input.clone();
    let mut bad_render = render.clone();
    Arc::make_mut(&mut bad_input.original.vertices)[2].uv0 = Some(uv[1]);
    bad_render[2].uv0 = uv[1];
    refused(bad_render, bad_input); // exact zero UV determinant
    let mut bad_input = input.clone();
    let mut bad_render = render;
    Arc::make_mut(&mut bad_input.original.vertices)[2].position = positions[1];
    bad_render[2].position = positions[1];
    refused(bad_render, bad_input); // exact zero original face area
}

#[test]
fn r024_abc_source_provenance_packing_and_independent_frame_certificates() {
    use crate::three_d::pbr::*;
    use r024_abc_fixture::*;
    for mode in 0..3 {
        for negative_uv in [false, true] {
            let (render, input) = packet(mode, negative_uv);
            let original = input.clone();
            let source = PbrGeometrySource3D::try_new_certified(
                render.clone(),
                GeometryIndices::Unindexed,
                input,
            )
            .unwrap();
            assert_eq!(source.certified_input(), Some(&original));
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&source.pack().unwrap()),
                bytemuck::cast_slice::<_, u8>(&render)
            );
            let (direct, faces) = source.pack_direct().unwrap();
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&direct),
                bytemuck::cast_slice::<_, u8>(&render)
            );
            assert!(faces.iter().all(|n| n.0 == [0.0, 0.0, 1.0]));
            let proof = source.certified_frames().unwrap()[0];
            assert!(proof.cross_lower >= 1.0 / 16.0 && proof.local_g_lower > 0.0);
            assert!(proof.maximum_direction_pack_error.is_finite());
            if mode == 1 {
                assert!(source.pack().unwrap().iter().any(|v| v.normal[2] < 0.75));
            }
            if mode == 2 {
                assert_ne!(original.original.vertices[0].normal, render[0].normal);
                assert_ne!(
                    &original.original.signed_tangents[0][..3],
                    &render[0].tangent[..3]
                );
                assert!(render.iter().all(|v| v.normal == [0.0, 0.0, 1.0]
                    && v.tangent[3] == if negative_uv { -1.0 } else { 1.0 }));
            }
        }
    }
    // All diagonal crosses point +Y; an off-diagonal pair is zero. A diagonal
    // test or a shared N/T interpolation weight would falsely admit this source.
    let (mut render, mut input) = packet(1, false);
    let ns = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let ts = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]];
    for i in 0..3 {
        std::sync::Arc::make_mut(&mut input.original.vertices)[i].normal = ns[i];
        std::sync::Arc::make_mut(&mut input.original.signed_tangents)[i] =
            [ts[i][0], ts[i][1], ts[i][2], 1.0];
        render[i].normal = ns[i];
        render[i].tangent = [ts[i][0], ts[i][1], ts[i][2], 1.0];
    }
    std::sync::Arc::make_mut(&mut input.triangles)[0].frame =
        PbrFrameCertificate3D::AuthoredSmooth {
            separator: [0.0, 1.0, 0.0],
            lower: [0.125; 9],
        };
    assert!(matches!(
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input),
        Err(PbrError3D::FrameCertificate {
            normal_corner: Some(0),
            tangent_corner: Some(1),
            ..
        })
    ));
}

#[test]
fn r024_abc_source_refuses_forged_singularity_sign_identity_and_work() {
    use crate::three_d::pbr::*;
    use r024_abc_fixture::*;
    let rejected = |render, input| {
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).is_err()
    };
    let (render, mut input) = packet(2, false);
    for n in std::sync::Arc::make_mut(&mut input.original.vertices) {
        n.normal = [0.0, 0.0, 1.0];
    }
    for t in std::sync::Arc::make_mut(&mut input.original.signed_tangents) {
        *t = [1.0, 0.0, 0.0, 1.0];
    }
    assert!(
        rejected(render, input),
        "a C label is not a singularity proof"
    );
    let (mut render, input) = packet(2, true);
    render[1].tangent[3] = 1.0;
    assert!(rejected(render, input));
    let (mut render, input) = packet(2, false);
    render[1].normal = [0.0, 1.0, 0.0];
    assert!(rejected(render, input));
    let (render, mut input) = packet(2, false);
    std::sync::Arc::make_mut(&mut input.triangles)[0]
        .original
        .source += 1;
    assert!(rejected(render, input));
    let (render, mut input) = packet(2, false);
    std::sync::Arc::make_mut(&mut input.render_vertex_sources)[1] = 2;
    assert!(rejected(render, input));
    let (render, mut input) = packet(2, false);
    input.maximum_certificate_bytes = 1;
    assert!(rejected(render, input));
    let (render, mut input) = packet(2, false);
    input.maximum_verification_items = 12;
    assert!(rejected(render, input));
    let (render, mut input) = packet(2, false);
    input.original.tangent_origin = PbrTangentOrigin3D::DefaultMikk {
        producer: 0,
        revision: 1,
        version: [0, 3, 0],
    };
    assert!(rejected(render, input));
    // A supplied geometric frame may intentionally have the opposite sign to
    // the original authored smooth frame. C must use UV orientation, not copy it.
    let (render, mut input) = packet(2, true);
    for t in std::sync::Arc::make_mut(&mut input.original.signed_tangents) {
        t[3] = 1.0;
    }
    let source =
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    assert!(source.pack().unwrap().iter().all(|v| v.tangent[3] == -1.0));
}

#[test]
fn r024_abc_normal_only_does_not_consume_or_erase_a_mapped_role() {
    use crate::three_d::pbr::*;
    let (mut render, mut input) = r024_abc_fixture::packet(1, false);
    input.mapped = false;
    input.original.signed_tangents = std::sync::Arc::from([]);
    for v in std::sync::Arc::make_mut(&mut input.original.vertices) {
        v.uv0 = None;
    }
    for v in &mut render {
        v.uv0 = [0.0; 2];
    }
    std::sync::Arc::make_mut(&mut input.triangles)[0].frame = PbrFrameCertificate3D::NormalOnly {
        separator: [0.0, 1.0, 0.0],
        lower: [0.5; 3],
    };
    let source =
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    source
        .validate_roles(&PbrMaterialSource3D::defaults(1, 1))
        .unwrap();
    assert!(
        source
            .certified_input()
            .unwrap()
            .original
            .vertices
            .iter()
            .all(|v| v.uv0.is_none())
    );
    assert!(source.certified_frames().unwrap()[0].normal_lower >= 0.5);
}

#[test]
fn r024_abc_source_generations_charge_complete_certificates_and_retained_leases() {
    use r024_abc_fixture::*;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let old = context
        .register_pbr_geometry(source(2, false), None)
        .unwrap();
    let held = context.lease_pbr_geometry(&old).unwrap();
    let one = context
        .resource_usage()
        .unwrap()
        .retained_cpu_descriptor_bytes;
    assert!(
        one > 3 * 52 + 16,
        "certificate/original bytes must enter the existing shared ledger"
    );
    let (mut render, input) = packet(2, false);
    render[0].position[0] += 1.0;
    assert!(
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).is_err()
    );
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        one
    );
    let (render, mut input) = packet(2, true);
    // Changed source bytes cannot reuse the same original source revision.
    let candidate = PbrGeometrySource3D::try_new_certified(
        render.clone(),
        GeometryIndices::Unindexed,
        input.clone(),
    )
    .unwrap();
    assert!(
        context
            .register_pbr_geometry(candidate, Some(&old))
            .is_err()
    );
    input.original.identity.revision = 2;
    std::sync::Arc::make_mut(&mut input.triangles)[0].original = input.original.identity;
    let reused_certificate = PbrGeometrySource3D::try_new_certified(
        render.clone(),
        GeometryIndices::Unindexed,
        input.clone(),
    )
    .unwrap();
    assert!(
        context
            .register_pbr_geometry(reused_certificate, Some(&old))
            .is_err()
    );
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        one
    );
    std::sync::Arc::make_mut(&mut input.triangles)[0].certificate_revision = 2;
    let next = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input)
                .unwrap(),
            Some(&old),
        )
        .unwrap();
    assert!(context.lease_pbr_geometry(&old).is_err());
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        2 * one
    );
    drop(held);
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        one
    );
    context.retire_pbr_geometry(&next).unwrap();
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        0
    );
    let mut policy = PolicyLimits3D::conservative();
    policy.max_cache_residency_bytes = one - 1;
    let tiny = Context3D::new_cpu_only_for_test(policy, u64::MAX).unwrap();
    assert!(matches!(
        tiny.register_pbr_geometry(source(2, false), None),
        Err(crate::three_d::pbr::PbrError3D::Resource(_))
            | Err(crate::three_d::pbr::PbrError3D::Context(_))
    ));
    assert_eq!(
        tiny.resource_usage().unwrap().retained_cpu_descriptor_bytes,
        0
    );
}

#[test]
fn r024_abc_exact_singularity_routes_and_original_indexed_face_expansion() {
    use crate::three_d::pbr::*;
    use r024_abc_fixture::*;
    use std::sync::Arc;
    // Non-coplanar N/T edge planes meet in +Z, strictly inside both
    // independently weighted edges. Every paired original N_i,T_i is also
    // orthogonal. Existence is algebraic, not a near-zero cross/search result.
    for antiparallel in [false, true] {
        let (render, mut input) = packet(2, false);
        let ns = [[0.8, 0.0, 0.6], [-0.8, 0.0, 0.6], [0.8, 0.0, 0.6]];
        let ts = [[-0.6, 0.5, 0.8], [0.6, -0.5, 0.8], [-0.6, 0.5, 0.8]];
        for i in 0..3 {
            Arc::make_mut(&mut input.original.vertices)[i].normal = ns[i];
            let t = ts[i].map(|x| if antiparallel { -x } else { x });
            Arc::make_mut(&mut input.original.signed_tangents)[i] = [t[0], t[1], t[2], 1.0];
        }
        if let PbrFrameCertificate3D::SourceGeometric { singularity, .. } =
            &mut Arc::make_mut(&mut input.triangles)[0].frame
        {
            *singularity = PbrSmoothSingularity3D::EdgePlanes {
                normals: [0, 1],
                tangents: [0, 1],
                reverse_ray: false,
                antiparallel,
            };
        }
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    }
    // An exact three-generator interior ray has the same bounded direct proof.
    let (render, mut input) = packet(2, false);
    let ns = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]];
    let ts = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, -1.0, 1.0]];
    for i in 0..3 {
        Arc::make_mut(&mut input.original.vertices)[i].normal = ns[i];
        Arc::make_mut(&mut input.original.signed_tangents)[i] = [ts[i][0], ts[i][1], ts[i][2], 1.0];
    }
    if let PbrFrameCertificate3D::SourceGeometric { singularity, .. } =
        &mut Arc::make_mut(&mut input.triangles)[0].frame
    {
        *singularity = PbrSmoothSingularity3D::VertexInCone {
            normal_vertex: true,
            vertex: 0,
            basis: [0, 1, 2],
            count: 3,
            antiparallel: false,
        };
    }
    PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();

    // Shared original indices, distinct explicit geometric shading frames.
    // Corner expansion preserves all original element/winding identities.
    let (base, mut input) = packet(2, false);
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, -0.5],
    ];
    let mapping = [0usize, 1, 2, 1, 0, 3];
    let mut vertices = input.original.vertices.to_vec();
    vertices.push(vertices[1].clone());
    for i in 0..4 {
        vertices[i].position = positions[i];
        vertices[i].uv0 = Some([positions[i][0], positions[i][1]]);
    }
    input.original.vertices = vertices.into();
    input.original.indices = GeometryIndices::from_u16(mapping.map(|i| i as u16).to_vec());
    let original_t = input.original.signed_tangents.clone();
    input.original.signed_tangents = mapping
        .map(|i| original_t[if i == 3 { 1 } else { i }])
        .into();
    input.render_vertex_sources = mapping.map(|i| i as u32).into();
    let first = input.triangles[0].clone();
    let mut second = first.clone();
    second.triangle = 1;
    second.certificate_identity += 1;
    if let PbrFrameCertificate3D::SourceGeometric {
        singularity,
        separator,
        ..
    } = &mut second.frame
    {
        *singularity = PbrSmoothSingularity3D::VertexInCone {
            normal_vertex: true,
            vertex: 0,
            basis: [0, 1, 0],
            count: 2,
            antiparallel: false,
        };
        *separator = [0.0, 1.0, 0.5];
    }
    input.triangles = Arc::from([first, second]);
    input.maximum_certificate_bytes = 8192;
    input.maximum_verification_items = 32;
    let second_normal = unit([0.0, -0.5, 1.0]);
    let render = mapping
        .into_iter()
        .enumerate()
        .map(|(corner, original)| PbrVertex52 {
            position: positions[original],
            normal: if corner < 3 {
                [0.0, 0.0, 1.0]
            } else {
                second_normal
            },
            uv0: [positions[original][0], positions[original][1]],
            ..base[0]
        })
        .collect();
    let source =
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    let (primary, faces) = source.pack_direct().unwrap();
    for i in 0..6 {
        assert_eq!(primary[i].position, positions[mapping[i]]);
        assert_eq!(primary[i].normal, faces[i].0);
    }
    assert_ne!(faces[0].0, faces[3].0);
    assert_eq!(source.certified_input().unwrap().original.vertices.len(), 4);
    assert_eq!(primary.len(), 6);
}

#[test]
fn r024_source52_pack_offsets_cones_uv_and_sign_are_distinct_from_legacy36() {
    use crate::three_d::pbr::{PbrMaterialSource3D, PbrRole3D, PbrRoleBinding3D};
    let source = PbrGeometrySource3D::try_new(r024_vertices(), GeometryIndices::Unindexed).unwrap();
    let packed = source.pack().unwrap();
    let b = bytemuck::cast_slice::<_, u8>(&packed);
    assert_eq!(b.len(), 156);
    assert_eq!(f32_at(b, 12), 0.0);
    assert_eq!(f32_at(b, 20), 1.0);
    assert_eq!(f32_at(b, 24), 0.0);
    assert_eq!(&b[32..36], &[255; 4]);
    assert_eq!(f32_at(b, 36), 1.0);
    assert_eq!(f32_at(b, 48), 1.0);
    let mut v = r024_vertices();
    v[0].normal = PbrUnitDirection3D::try_new([1.0, 0.0, 0.0]).unwrap();
    assert!(PbrGeometrySource3D::try_new(v, GeometryIndices::Unindexed).is_err());
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
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
    let mut m = PbrMaterialSource3D::defaults(1, 1);
    m.roles[PbrRole3D::Normal as usize] = Some(PbrRoleBinding3D::V1 {
        texture: texture.handle().clone(),
        sampler: sampler.handle().clone(),
    });
    source.validate_roles(&m).unwrap();
    let mut v = r024_vertices();
    v[1].tangent.as_mut().unwrap().1 = -1.0;
    assert!(
        PbrGeometrySource3D::try_new(v, GeometryIndices::Unindexed)
            .unwrap()
            .validate_roles(&m)
            .is_err()
    );
    let mut v = r024_vertices();
    for x in &mut v {
        x.uv0 = Some(TextureCoordinate2::try_new(0.0, 0.0).unwrap());
    }
    assert!(
        PbrGeometrySource3D::try_new(v, GeometryIndices::Unindexed)
            .unwrap()
            .validate_roles(&m)
            .is_err()
    );
    let mut v = r024_vertices();
    for x in &mut v {
        x.uv0 = None;
        x.tangent = None;
    }
    let no_map = PbrGeometrySource3D::try_new(v, GeometryIndices::Unindexed).unwrap();
    no_map
        .validate_roles(&PbrMaterialSource3D::defaults(2, 1))
        .unwrap();
    assert!(no_map.validate_roles(&m).is_err());
    assert!(PbrUnitDirection3D::try_new([0.0, 0.0, 0.5]).is_err());
    assert!(PbrUnitDirection3D::try_new([f64::NAN, 0.0, 1.0]).is_err());
    assert_eq!(CANONICAL_VERTEX_STRIDE, 36);
}

fn vector(x: f32, y: f32, z: f32) -> LocalVector3 {
    LocalVector3::try_new(x, y, z).unwrap()
}

#[test]
fn r025_direct_deindex_keeps_original_triangle_faces_and_d0_primary52() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, -0.5],
    ];
    let vertices = positions
        .into_iter()
        .map(|p| PbrVertexSource3D {
            position: point(p[0], p[1], p[2]),
            normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
            uv0: None,
            color: None,
            tangent: None,
        })
        .collect();
    let source =
        PbrGeometrySource3D::try_new(vertices, GeometryIndices::from_u16(vec![0, 1, 2, 1, 0, 3]))
            .unwrap();
    let before = source.pack().unwrap();
    let (primary, faces) = source.pack_direct().unwrap();
    assert_eq!(before.len(), 4);
    assert_eq!(primary.len(), 6);
    assert_eq!(faces.len(), 6);
    for (i, src) in [0, 1, 2, 1, 0, 3].into_iter().enumerate() {
        assert_eq!(
            bytemuck::bytes_of(&primary[i]),
            bytemuck::bytes_of(&before[src])
        );
    }
    assert_eq!(faces[0].0, [0.0, 0.0, 1.0]);
    assert_eq!(faces[0].0, faces[1].0);
    assert_eq!(faces[1].0, faces[2].0);
    assert_eq!(faces[3].0, faces[4].0);
    assert_eq!(faces[4].0, faces[5].0);
    assert_ne!(faces[0].0, faces[3].0);
    assert!((faces[3].0[1] + 0.5 / 1.25f32.sqrt()).abs() < 1e-7);
    assert!((faces[3].0[2] - 1.0 / 1.25f32.sqrt()).abs() < 1e-7);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&source.pack().unwrap()),
        bytemuck::cast_slice::<_, u8>(&before)
    );
    assert_eq!(bytemuck::cast_slice::<_, u8>(&primary).len(), 6 * 52);
    assert_eq!(bytemuck::cast_slice::<_, u8>(&faces).len(), 6 * 12);
    assert_eq!(CANONICAL_VERTEX_STRIDE, 36);
}

fn positions() -> Vec<LocalPoint3> {
    vec![
        point(1.25, -2.5, 3.75),
        point(-4.0, 5.5, -6.25),
        point(7.0, 8.5, 9.25),
    ]
}

fn indexed_u16(offset: f32) -> GeometrySourceDescriptor {
    GeometrySourceDescriptor::try_new(
        vec![
            point(offset, 0.0, 0.0),
            point(offset + 1.0, 0.0, 0.0),
            point(offset, 1.0, 0.0),
        ],
        None,
        None,
        None,
        GeometryIndices::from_u16(vec![0, 1, 2]),
    )
    .unwrap()
}

fn f32_at(bytes: &[u8], offset: u64) -> f32 {
    let offset = usize::try_from(offset).unwrap();
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[test]
fn canonical_vertex_packing_is_byte_exact_at_every_fixed_offset() {
    let descriptor = GeometrySourceDescriptor::try_new(
        positions(),
        Some(vec![vector(0.0, 2.0, 0.0); 3]),
        Some(vec![TextureCoordinate2::try_new(0.25, 0.75).unwrap(); 3]),
        Some(vec![
            LinearVertexColorRgba8::from_linear_unorm8(1, 2, 3, 4),
            LinearVertexColorRgba8::from_linear_unorm8(5, 6, 7, 8),
            LinearVertexColorRgba8::from_linear_unorm8(9, 10, 11, 12),
        ]),
        GeometryIndices::Unindexed,
    )
    .unwrap();
    let packed = CanonicalGeometryBytes::pack(&descriptor).unwrap();
    let bytes = packed.vertex_bytes();

    assert_eq!(CANONICAL_POSITION_OFFSET, 0);
    assert_eq!(CANONICAL_NORMAL_OFFSET, 12);
    assert_eq!(CANONICAL_UV_OFFSET, 24);
    assert_eq!(CANONICAL_COLOR_OFFSET, 32);
    assert_eq!(CANONICAL_VERTEX_STRIDE, 36);
    assert_eq!(bytes.len(), 3 * 36);
    assert_eq!(f32_at(bytes, CANONICAL_POSITION_OFFSET), 1.25);
    assert_eq!(f32_at(bytes, CANONICAL_POSITION_OFFSET + 4), -2.5);
    assert_eq!(f32_at(bytes, CANONICAL_POSITION_OFFSET + 8), 3.75);
    assert_eq!(f32_at(bytes, CANONICAL_NORMAL_OFFSET), 0.0);
    assert_eq!(f32_at(bytes, CANONICAL_NORMAL_OFFSET + 4), 1.0);
    assert_eq!(f32_at(bytes, CANONICAL_NORMAL_OFFSET + 8), 0.0);
    assert_eq!(f32_at(bytes, CANONICAL_UV_OFFSET), 0.25);
    assert_eq!(f32_at(bytes, CANONICAL_UV_OFFSET + 4), 0.75);
    assert_eq!(
        &bytes[usize::try_from(CANONICAL_COLOR_OFFSET).unwrap()..36],
        &[1, 2, 3, 4]
    );
    assert_eq!(packed.vertex_count(), 3);
    assert_eq!(packed.index_count(), 0);
    assert_eq!(packed.index_format(), CanonicalIndexFormat::Unindexed);
    assert!(
        descriptor
            .semantics()
            .contains(GeometrySemantics::new(true, true, true))
    );
}

#[test]
fn canonical_vertex_packing_materializes_exact_optional_defaults() {
    let descriptor = GeometrySourceDescriptor::try_new(
        positions(),
        None,
        None,
        None,
        GeometryIndices::Unindexed,
    )
    .unwrap();
    let packed = CanonicalGeometryBytes::pack(&descriptor).unwrap();
    let bytes = packed.vertex_bytes();

    for vertex in 0..3_u64 {
        let base = vertex * CANONICAL_VERTEX_STRIDE;
        assert_eq!(f32_at(bytes, base + CANONICAL_NORMAL_OFFSET), 0.0);
        assert_eq!(f32_at(bytes, base + CANONICAL_NORMAL_OFFSET + 4), 0.0);
        assert_eq!(f32_at(bytes, base + CANONICAL_NORMAL_OFFSET + 8), 1.0);
        assert_eq!(f32_at(bytes, base + CANONICAL_UV_OFFSET), 0.0);
        assert_eq!(f32_at(bytes, base + CANONICAL_UV_OFFSET + 4), 0.0);
        let color = usize::try_from(base + CANONICAL_COLOR_OFFSET).unwrap();
        assert_eq!(&bytes[color..color + 4], &[255, 255, 255, 255]);
    }
    assert_eq!(descriptor.semantics(), GeometrySemantics::NONE);
}

#[test]
fn canonical_index_packing_distinguishes_unindexed_u16_and_u32_exactly() {
    let unindexed = GeometrySourceDescriptor::try_new(
        positions(),
        None,
        None,
        None,
        GeometryIndices::Unindexed,
    )
    .unwrap();
    let packed_unindexed = CanonicalGeometryBytes::pack(&unindexed).unwrap();
    assert_eq!(
        packed_unindexed.index_format(),
        CanonicalIndexFormat::Unindexed
    );
    assert!(packed_unindexed.index_bytes().is_empty());
    assert_eq!(packed_unindexed.index_buffer_size(), 0);
    assert_eq!(packed_unindexed.upload_bytes().unwrap(), 108);
    assert_eq!(packed_unindexed.gpu_resident_bytes().unwrap(), 108);

    let indexed_u16 = GeometrySourceDescriptor::try_new(
        positions(),
        None,
        None,
        None,
        GeometryIndices::from_u16(vec![2, 0, 1]),
    )
    .unwrap();
    let packed_u16 = CanonicalGeometryBytes::pack(&indexed_u16).unwrap();
    assert_eq!(packed_u16.index_format(), CanonicalIndexFormat::Uint16);
    assert_eq!(packed_u16.index_bytes(), &[2, 0, 0, 0, 1, 0]);
    assert_eq!(packed_u16.index_count(), 3);
    assert_eq!(packed_u16.index_buffer_size(), 8);
    assert_eq!(packed_u16.upload_bytes().unwrap(), 114);
    assert_eq!(packed_u16.gpu_resident_bytes().unwrap(), 116);

    let indexed_u32 = GeometrySourceDescriptor::try_new(
        positions(),
        None,
        None,
        None,
        GeometryIndices::from_u32(vec![2, 0, 1]),
    )
    .unwrap();
    let packed_u32 = CanonicalGeometryBytes::pack(&indexed_u32).unwrap();
    assert_eq!(packed_u32.index_format(), CanonicalIndexFormat::Uint32);
    assert_eq!(
        packed_u32.index_bytes(),
        &[2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]
    );
    assert_eq!(packed_u32.index_count(), 3);
    assert_eq!(packed_u32.index_buffer_size(), 12);
    assert_eq!(packed_u32.upload_bytes().unwrap(), 120);
    assert_eq!(packed_u32.gpu_resident_bytes().unwrap(), 120);
}

#[test]
fn gpu_materialization_is_lazy_device_bound_and_reuses_one_generation() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let context = Context3D::new(gpu.clone()).unwrap();
    assert_eq!(context.geometry_materialization_count_for_test(), 0);
    assert_eq!(context.gpu_resident_geometry_bytes_for_test(), 0);

    let geometry = context.register_geometry(indexed_u16(0.0)).unwrap();
    let handle = geometry.handle().clone();
    assert_eq!(context.geometry_materialization_count_for_test(), 0);
    assert_eq!(context.resource_usage().unwrap().resident_geometry_bytes, 0);

    let first = context.materialize_geometry_for_test(&handle, 114).unwrap();
    assert_eq!(first.handle(), &handle);
    assert_eq!(first.vertex_buffer_size(), 108);
    assert_eq!(first.index_buffer_size(), Some(8));
    assert_eq!(first.vertex_count(), 3);
    assert_eq!(first.index_count(), 3);
    assert_eq!(first.index_format(), CanonicalIndexFormat::Uint16);
    assert_eq!(first.upload_bytes(), 114);
    assert_eq!(first.resident_bytes(), 116);
    assert!(first.matches_device(&gpu));
    assert_eq!(context.geometry_materialization_count_for_test(), 1);
    assert_eq!(
        context.resource_usage().unwrap().resident_geometry_bytes,
        116
    );

    let reused = context.materialize_geometry_for_test(&handle, 0).unwrap();
    assert!(first.same_buffers(&reused));
    assert_eq!(context.geometry_materialization_count_for_test(), 1);
    assert_eq!(context.gpu_resident_geometry_bytes_for_test(), 116);

    let second_geometry = context.register_geometry(indexed_u16(2.0)).unwrap();
    let third_geometry = context.register_geometry(indexed_u16(4.0)).unwrap();
    let mut preparation_budget = context.preparation_upload_budget_for_test(114);
    let cached_without_charge = context
        .materialize_geometry_with_budget_for_test(&handle, &mut preparation_budget)
        .unwrap();
    assert!(first.same_buffers(&cached_without_charge));
    assert_eq!(preparation_budget.remaining_bytes(), 114);
    context
        .materialize_geometry_with_budget_for_test(
            second_geometry.handle(),
            &mut preparation_budget,
        )
        .unwrap();
    assert_eq!(preparation_budget.remaining_bytes(), 0);
    assert_eq!(
        context
            .materialize_geometry_with_budget_for_test(
                third_geometry.handle(),
                &mut preparation_budget,
            )
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::UploadBytesPerFrame,
            requested: 114,
            maximum: 0,
        }
    );
    assert_eq!(preparation_budget.remaining_bytes(), 0);
    assert_eq!(context.geometry_materialization_count_for_test(), 2);
}

#[test]
fn pre_wgpu_budget_rejections_publish_nothing_and_allow_retry() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let gpu = renderer.gpu_resources();

    let mut mesh_policy = PolicyLimits3D::conservative();
    mesh_policy.max_mesh_bytes = 114;
    let mesh_context = Context3D::with_policy(gpu.clone(), mesh_policy).unwrap();
    let mesh_geometry = mesh_context.register_geometry(indexed_u16(0.0)).unwrap();
    assert_eq!(
        mesh_context
            .materialize_geometry_for_test(mesh_geometry.handle(), 114)
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::MeshBytes,
            requested: 116,
            maximum: 114,
        }
    );
    assert_eq!(mesh_context.geometry_materialization_count_for_test(), 0);
    assert_eq!(mesh_context.gpu_resident_geometry_bytes_for_test(), 0);
    assert_eq!(
        mesh_context
            .lease_geometry(mesh_geometry.handle())
            .unwrap()
            .handle(),
        mesh_geometry.handle()
    );

    let upload_context = Context3D::new(gpu.clone()).unwrap();
    let upload_geometry = upload_context.register_geometry(indexed_u16(0.0)).unwrap();
    let upload_handle = upload_geometry.handle().clone();
    assert_eq!(
        upload_context
            .materialize_geometry_for_test(&upload_handle, 113)
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::UploadBytesPerFrame,
            requested: 114,
            maximum: 113,
        }
    );
    assert_eq!(upload_context.geometry_materialization_count_for_test(), 0);
    assert_eq!(upload_context.gpu_resident_geometry_bytes_for_test(), 0);
    assert_eq!(
        upload_context
            .lease_geometry(&upload_handle)
            .unwrap()
            .descriptor(),
        upload_geometry.descriptor()
    );
    let retried = upload_context
        .materialize_geometry_for_test(&upload_handle, 114)
        .unwrap();
    assert_eq!(retried.resident_bytes(), 116);
    assert_eq!(upload_context.geometry_materialization_count_for_test(), 1);

    let mut resident_policy = PolicyLimits3D::conservative();
    resident_policy.max_resident_geometry_bytes = 116;
    let resident_context = Context3D::with_policy(gpu.clone(), resident_policy).unwrap();
    let first = resident_context
        .register_geometry(indexed_u16(0.0))
        .unwrap();
    let first_handle = first.handle().clone();
    let second = resident_context
        .register_geometry(indexed_u16(2.0))
        .unwrap();
    let second_handle = second.handle().clone();
    let first_gpu = resident_context
        .materialize_geometry_for_test(&first_handle, 114)
        .unwrap();
    assert_eq!(
        resident_context
            .materialize_geometry_for_test(&second_handle, 114)
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ResidentGeometryBytes,
            requested: 232,
            maximum: 116,
        }
    );
    assert_eq!(
        resident_context.geometry_materialization_count_for_test(),
        1
    );
    assert_eq!(resident_context.gpu_resident_geometry_bytes_for_test(), 116);
    assert_eq!(
        resident_context
            .lease_geometry(&second_handle)
            .unwrap()
            .handle(),
        &second_handle
    );

    assert!(
        !resident_context
            .evict_unleased_geometry_cache_for_test(&first_handle)
            .unwrap()
    );
    drop(first_gpu);
    assert!(
        !resident_context
            .evict_unleased_geometry_cache_for_test(&first_handle)
            .unwrap()
    );
    drop(first);
    assert!(
        resident_context
            .evict_unleased_geometry_cache_for_test(&first_handle)
            .unwrap()
    );
    assert_eq!(resident_context.gpu_resident_geometry_bytes_for_test(), 0);
    let second_gpu = resident_context
        .materialize_geometry_for_test(&second_handle, 114)
        .unwrap();
    assert_eq!(second_gpu.resident_bytes(), 116);
    assert_eq!(
        resident_context.geometry_materialization_count_for_test(),
        2
    );
    resident_context.retire_geometry(&second_handle).unwrap();
    assert_eq!(resident_context.gpu_resident_geometry_bytes_for_test(), 116);
    drop(second_gpu);
    assert_eq!(resident_context.gpu_resident_geometry_bytes_for_test(), 116);
    drop(second);
    assert_eq!(resident_context.gpu_resident_geometry_bytes_for_test(), 0);
}

#[test]
fn replacement_and_typed_handle_errors_never_alias_or_touch_the_wrong_cache() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let first_context = Context3D::new(gpu.clone()).unwrap();
    let second_context = Context3D::new(gpu.clone()).unwrap();

    let old = first_context.register_geometry(indexed_u16(0.0)).unwrap();
    let old_handle = old.handle().clone();
    let old_gpu = first_context
        .materialize_geometry_for_test(&old_handle, 114)
        .unwrap();
    assert_eq!(
        second_context
            .materialize_geometry_for_test(&old_handle, 114)
            .unwrap_err(),
        Context3DError::ContextMismatch
    );
    assert_eq!(second_context.geometry_materialization_count_for_test(), 0);

    let appearance = first_context
        .register_appearance(
            crate::three_d::AppearanceSourceDescriptor::try_new(
                "appearance",
                GeometrySemantics::NONE,
            )
            .unwrap(),
        )
        .unwrap();
    let wrong_kind = Context3D::wrong_kind_geometry_handle_for_test(appearance.handle());
    assert_eq!(
        first_context
            .materialize_geometry_for_test(&wrong_kind, 114)
            .unwrap_err(),
        Context3DError::WrongResourceKind {
            expected: crate::three_d::ResourceKind3D::Geometry,
            actual: crate::three_d::ResourceKind3D::Appearance,
        }
    );
    assert_eq!(first_context.geometry_materialization_count_for_test(), 1);

    let new = first_context
        .replace_geometry(&old_handle, indexed_u16(2.0))
        .unwrap();
    let new_handle = new.handle().clone();
    let new_gpu = first_context
        .materialize_geometry_for_test(&new_handle, 114)
        .unwrap();
    assert!(!old_gpu.same_buffers(&new_gpu));
    assert_eq!(old_gpu.handle().generation(), 1);
    assert_eq!(new_gpu.handle().generation(), 2);
    assert_eq!(first_context.gpu_resident_geometry_bytes_for_test(), 232);
    assert_eq!(
        first_context
            .materialize_geometry_for_test(&old_handle, 114)
            .unwrap_err(),
        Context3DError::StaleGeneration {
            slot: old_handle.slot(),
            requested: 1,
            live: Some(2),
        }
    );
    assert_eq!(first_context.geometry_materialization_count_for_test(), 2);

    drop(old);
    assert_eq!(first_context.gpu_resident_geometry_bytes_for_test(), 232);
    drop(old_gpu);
    assert_eq!(first_context.gpu_resident_geometry_bytes_for_test(), 116);

    let isolated = second_context.register_geometry(indexed_u16(0.0)).unwrap();
    let mut foreign_budget = first_context.preparation_upload_budget_for_test(114);
    assert_eq!(
        second_context
            .materialize_geometry_with_budget_for_test(isolated.handle(), &mut foreign_budget,)
            .unwrap_err(),
        Context3DError::ContextMismatch
    );
    assert_eq!(second_context.geometry_materialization_count_for_test(), 0);
    assert_eq!(foreign_budget.remaining_bytes(), 114);
    let isolated_gpu = second_context
        .materialize_geometry_for_test(isolated.handle(), 114)
        .unwrap();
    assert!(isolated_gpu.matches_device(&gpu));
    assert!(!new_gpu.same_buffers(&isolated_gpu));
}
