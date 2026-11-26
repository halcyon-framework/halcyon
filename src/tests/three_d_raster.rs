use std::sync::{Arc, Barrier};

use crate::renderer::wgpu_renderer::{GpuResources, WgpuRenderer};
use crate::three_d::{
    AppearanceMode, AppearanceSourceDescriptor, Context3D, DrawPacketDescriptor, GeometryIndices,
    GeometrySemantics, GeometrySourceDescriptor, HeadlessRaster3DError, LocalPoint3, LocalVector3,
    ModelPlacement3, ModelRevision, ModelState, ModelStateDescriptor, Orientation3,
    OrthographicProjection, PerspectiveProjection, PolicyBudget3D, PolicyLimits3D, PositiveScale3,
    RasterMode, RenderSnapshot, RenderSnapshotDescriptor, RigidPose3, Sampler3D,
    StructuralRevision, Texture3D, TextureCoordinate2, TextureFormat3D, ViewDescriptor3,
    WorldPoint3,
};

const EXTENT: [u32; 2] = [64, 64];

#[test]
fn hal372_object128_prefix_and_checked_tail() {
    use crate::three_d::object_reveal::Object128Reveal;
    let mut original = bytemuck::Zeroable::zeroed();
    bytemuck::bytes_of_mut(&mut original).fill(0x5a);
    let object = Object128Reveal::checked(original, 0.25, -2.0, 8.0, 1).unwrap();
    let bytes = bytemuck::bytes_of(&object);
    assert_eq!(bytes.len(), 128);
    assert_eq!(&bytes[..112], bytemuck::bytes_of(&original));
    assert_eq!(&bytes[112..116], &0.25f32.to_ne_bytes());
    assert_eq!(&bytes[116..120], &(-2.0f32).to_ne_bytes());
    assert_eq!(&bytes[120..124], &8.0f32.to_ne_bytes());
    assert_eq!(&bytes[124..128], &1u32.to_ne_bytes());
    for (p, lower, span, axis) in [
        (f32::NAN, 0.0, 1.0, 0),
        (-0.01, 0.0, 1.0, 0),
        (1.01, 0.0, 1.0, 0),
        (0.5, f32::INFINITY, 1.0, 0),
        (0.5, 0.0, 0.0, 0),
        (0.5, 0.0, -1.0, 0),
        (0.5, f32::MAX, f32::MAX, 0),
        (0.5, 0.0, 1.0, 3),
    ] {
        assert!(Object128Reveal::checked(original, p, lower, span, axis).is_err());
    }
}

#[test]
fn hal372_shader_object128_all_source_families() {
    use crate::three_d::indirect::Presence3D;
    use crate::three_d::mip_sampling::SamplingFamily3D;
    use crate::three_d::object_reveal::{controlled_material_source, shader_source};
    let mut families = 0;
    for bits in 0..8 {
        let profile = Presence3D(bits);
        for sampling in [SamplingFamily3D::V1, SamplingFamily3D::V2] {
            let original = controlled_material_source(profile, sampling, false).unwrap();
            let source = controlled_material_source(profile, sampling, true).unwrap();
            assert!(original.contains("var<uniform> object:Object112;"));
            assert!(!original.contains("object_reveal_visible"));
            assert!(!source.contains("var<uniform> object:Object112;"));
            let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|e| {
                panic!("profile{bits}/{sampling:?}: {}", e.emit_to_string(&source))
            });
            let caps = if profile.direct() || profile.environment() {
                naga::valid::Capabilities::CUBE_ARRAY_TEXTURES
            } else {
                naga::valid::Capabilities::empty()
            };
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
                .validate(&module)
                .unwrap_or_else(|e| {
                    panic!("profile{bits}/{sampling:?}: {}", e.emit_to_string(&source))
                });
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some("Object128Reveal"))
                .unwrap();
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("object must be a struct")
            };
            assert_eq!(*span, 128);
            assert_eq!(
                members.iter().map(|m| m.offset).collect::<Vec<_>>(),
                [0, 64, 112, 116, 120, 124]
            );
            if profile.direct() {
                let marker = "@fragment fn shadow_fragment";
                let old = original
                    .split_once(marker)
                    .unwrap()
                    .1
                    .split_once("\n}")
                    .unwrap()
                    .0;
                let new = source
                    .split_once(marker)
                    .unwrap()
                    .1
                    .split_once("\n}")
                    .unwrap()
                    .0;
                assert_eq!(old, new, "source-complete caster body changed");
            }
            assert!(
                shader_source(source).is_err(),
                "double transformation must refuse"
            );
            let drifted = original.replace("let s=surface(i);", "let s = surface(i);");
            assert!(
                shader_source(drifted).is_err(),
                "changed controlled entry must refuse"
            );
            families += 1;
        }
    }
    assert_eq!(families, 16);
}

#[test]
fn r028_uv_gradient_original_attribution() {
    use crate::three_d::mip_sampling::{UvDomainContext3D, UvDomainSummary3D, qualify_uv_domain};
    use crate::three_d::pbr::*;
    use crate::three_d::{PbrOriginalIdentity3D as PublicIdentity, PbrSourceError3D};

    // Analytic original source, not the original external consumer's furniture: the first triangle crosses
    // the left clip plane and the second has an independently selected UV scale.
    let identity = PbrOriginalIdentity3D {
        source: 471,
        revision: 2,
        node: 5,
        mesh: 7,
        primitive: 11,
    };
    let source = |uv_scale: f32| {
        let positions = [
            [-2.0, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [-2.0, 0.5, 0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];
        let uv = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [uv_scale, 0.0],
            [0.0, uv_scale],
        ];
        let render = positions
            .into_iter()
            .zip(uv)
            .map(|(position, uv0)| PbrVertex52 {
                position,
                normal: [0.0, 0.0, 1.0],
                uv0,
                color: [255; 4],
                tangent: [1.0, 0.0, 0.0, 1.0],
            })
            .collect::<Vec<_>>();
        let input = PbrCertifiedInput3D {
            original: PbrOriginalGeometry3D {
                identity,
                vertices: render
                    .iter()
                    .map(|v| PbrOriginalVertex3D {
                        position: v.position,
                        normal: v.normal,
                        uv0: Some(v.uv0),
                        color: None,
                    })
                    .collect::<Vec<_>>()
                    .into(),
                indices: GeometryIndices::Unindexed,
                signed_tangents: Arc::from([[1.0, 0.0, 0.0, 1.0]; 6]),
                tangent_origin: PbrTangentOrigin3D::Authored,
            },
            render_vertex_sources: Arc::from([0, 1, 2, 3, 4, 5]),
            triangles: (0..2)
                .map(|triangle| PbrTriangleCertificate3D {
                    original: identity,
                    triangle,
                    certificate_identity: 472,
                    certificate_revision: 1,
                    frame: PbrFrameCertificate3D::OldCone,
                })
                .collect::<Vec<_>>()
                .into(),
            mapped: true,
            maximum_certificate_bytes: 16_384,
            maximum_verification_items: 128,
        };
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap()
    };
    let matrix = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let context = UvDomainContext3D {
        packet: 5,
        ordinal: 0xdead_beef,
        shadow_face: Some(13),
    };
    let safe = source(1.0);
    let proof = qualify_uv_domain(&safe, matrix, matrix, (192, 128), [1024; 2], context).unwrap();
    assert_eq!(
        (proof.qualified, proof.excluded, proof.conditioned),
        (2, 0, 0)
    );
    assert!(f64::from_bits(proof.minimum_determinant_margin_bits.unwrap()) > 0.0);
    assert!(f64::from_bits(proof.minimum_helper_q_lower_bits.unwrap()) > 0.0);
    let mut outside = matrix;
    outside[3][0] = 10.0;
    assert_eq!(
        qualify_uv_domain(&safe, outside, matrix, (192, 128), [1024; 2], context).unwrap(),
        UvDomainSummary3D {
            qualified: 0,
            excluded: 2,
            conditioned: 0,
            minimum_determinant_margin_bits: None,
            minimum_helper_q_lower_bits: None,
        }
    );

    // Whole-triangle texel span is not a per-pixel derivative. For the second
    // triangle at192x128, the nominal physical spans are96 and64 pixels. This
    // counterexample falsifies a mandatory whole-span squared source cap; the
    // nominal derivative comparison is not an actual GPU precision certificate.
    let texel_span = f64::from(1e17_f32) * 1024.0;
    let whole_span_squared = 2.0 * texel_span.powi(2);
    let nominal_gradient_squared = (texel_span / 96.0).powi(2) + (texel_span / 64.0).powi(2);
    assert!(whole_span_squared > f64::from(f32::MAX));
    assert!(nominal_gradient_squared < f64::from(f32::MAX));
    for scale in [1e17_f32, 1e20_f32] {
        let proof = qualify_uv_domain(
            &source(scale),
            matrix,
            matrix,
            (192, 128),
            [1024; 2],
            context,
        )
        .unwrap();
        assert_eq!(
            (proof.qualified, proof.excluded, proof.conditioned),
            (2, 0, 0)
        );
        assert!(f64::from_bits(proof.minimum_determinant_margin_bits.unwrap()) > 0.0);
        assert_eq!(proof.minimum_helper_q_lower_bits, None);
    }

    // This is an actual host/packed vertex-transform overflow: the first
    // original x=-2 multiplied by a finite f32::MAX cannot fit in binary32.
    // Refuse under that exact cause before any clip/interpolation model exists.
    let mut overflow_object = matrix;
    overflow_object[0][0] = f32::MAX;
    let error = qualify_uv_domain(
        &safe,
        overflow_object,
        matrix,
        (192, 128),
        [1024; 2],
        context,
    )
    .unwrap_err();
    let PbrError3D::UvGradient(failure) = &error else {
        panic!("{error:?}")
    };
    assert_eq!(failure.packet, 5);
    assert_eq!(failure.ordinal, 0xdead_beef);
    assert_eq!(failure.original, Some(identity));
    assert_eq!(failure.triangle, Some(0));
    assert_eq!(failure.render_triangle, Some(0));
    assert_eq!(failure.shadow_face, Some(13));
    assert_eq!(failure.corner, Some(0));
    assert_eq!(failure.axis, Some(0));
    assert_eq!(failure.operation, "packed object dot finite and within f32");
    assert!(
        failure
            .bounds_bits
            .unwrap()
            .map(f64::from_bits)
            .iter()
            .all(|bound| *bound < -f64::from(f32::MAX))
    );
    assert_eq!(
        failure.object_bits,
        overflow_object.map(|column| column.map(f32::to_bits))
    );
    assert_eq!(
        failure.position_bits,
        [Some([-2.0_f32, -0.5, 0.5].map(f32::to_bits)), None, None]
    );
    assert_eq!(failure.uv_bits, [None; 3]);
    assert_eq!(failure.clip_bits, [None; 3]);
    assert_eq!(failure.determinant_bits, None);
    assert_eq!(failure.helper_q_min_bits, None);
    assert_eq!(failure.q_plane_bits, None);
    assert_eq!(failure.uv_plane_bits, None);
    let public: PbrSourceError3D = error.clone().into();
    let PbrSourceError3D::UvGradient {
        packet,
        ordinal,
        original,
        triangle,
        render_triangle,
        shadow_face,
        operation,
        corner,
        axis,
        bounds_bits,
        determinant_bits,
        helper_q_min_bits,
        extent,
        role_extent,
        object_bits,
        projection_bits,
        position_bits,
        uv_bits,
        clip_bits,
        q_plane_bits,
        uv_plane_bits,
    } = public.clone()
    else {
        panic!("{public:?}")
    };
    assert_eq!(
        original,
        Some(PublicIdentity {
            source: 471,
            revision: 2,
            node: 5,
            mesh: 7,
            primitive: 11
        })
    );
    let roundtrip = UvGradientFailure3D {
        packet,
        ordinal,
        original: Some(identity),
        triangle,
        render_triangle,
        shadow_face,
        operation,
        corner,
        axis,
        bounds_bits,
        determinant_bits,
        helper_q_min_bits,
        extent,
        role_extent,
        object_bits,
        projection_bits,
        position_bits,
        uv_bits,
        clip_bits,
        q_plane_bits,
        uv_plane_bits,
    };
    assert_eq!(
        failure.as_ref(),
        &roundtrip,
        "public failure mapping is bit-exact"
    );
    assert_eq!(public.to_string(), error.to_string());

    // A singular screen model is not invalid source geometry, including at
    // large finite UV scales. No positive determinant/helper bound is invented.
    let mut edge_on = matrix;
    edge_on[1][1] = 0.0;
    for scale in [1.0_f32, 1e17_f32, 1e20_f32] {
        let proof = qualify_uv_domain(
            &source(scale),
            matrix,
            edge_on,
            (192, 128),
            [1024; 2],
            UvDomainContext3D {
                shadow_face: None,
                ..context
            },
        )
        .unwrap();
        assert_eq!(
            proof,
            UvDomainSummary3D {
                qualified: 2,
                excluded: 0,
                conditioned: 0,
                minimum_determinant_margin_bits: None,
                minimum_helper_q_lower_bits: None,
            }
        );
    }
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R028 attributable UV host checks: clipped source qualified2/excluded0; wholly outside excluded2; source1e17/source1e20 admitted with optional helper minimum=None, no GPU finiteness claim; source1e17 whole-span squared={} >f32MAX but nominal per-pixel gradient squared={} <f32MAX; actual host overflow {public}; singular screen models admitted without positive bounds; public mapping bit-exact; no external consumer GPU claim\n",
        whole_span_squared,
        nominal_gradient_squared,
    )).unwrap();

    // Independent review supplied the original external consumer's unchanged
    // MAIN/packet0/source10000 rev1/node0/mesh0/primitive0/triangle0 refusal.
    // These POSITION/UV/object/projection bits are
    // the actual supplied numerical counterexample, including corrected clip-Y
    // endpoints. Normal/Mikk attributes were NOT supplied: this UV-only test
    // container uses explicitly analytical normal-only attributes (source900),
    // never claims to be the original furniture material or an external consumer GPU render.
    // The qualifier depends only on these exact positions/UVs/matrices/extent.
    let positions = [
        [3209900768, 1057964898, 1049042363],
        [3209821237, 1057877702, 1049137555],
        [3209919820, 1057989914, 1049041861],
    ]
    .map(|p| p.map(f32::from_bits));
    let uvs = [
        [1064273548, 1027882352],
        [1064242086, 1028670400],
        [1064285476, 1027743456],
    ]
    .map(|p| p.map(f32::from_bits));
    let object = [
        [1065353216, 0, 0, 0],
        [0, 1065353216, 0, 0],
        [0, 0, 1065353216, 0],
        [3214514585, 3218289459, 3230033327, 1065353216],
    ]
    .map(|p| p.map(f32::from_bits));
    let projection = [
        [1067482393, 0, 0, 0],
        [0, 1072426802, 967697350, 3192614906],
        [0, 3200208408, 987133132, 3212502437],
        [0, 0, 1028465748, 0],
    ]
    .map(|p| p.map(f32::from_bits));
    let (mut render, mut input) = super::three_d_geometry::r024_abc_fixture::packet(0, false);
    for i in 0..3 {
        render[i].position = positions[i];
        render[i].uv0 = uvs[i];
        let original = &mut Arc::make_mut(&mut input.original.vertices)[i];
        original.position = positions[i];
        original.uv0 = Some(uvs[i]);
    }
    input.mapped = false;
    Arc::make_mut(&mut input.triangles)[0].frame = PbrFrameCertificate3D::NormalOnly {
        separator: [0.0, 0.0, 1.0],
        lower: [0.5; 3],
    };
    let geometry =
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    let proof = qualify_uv_domain(
        &geometry,
        object,
        projection,
        (192, 128),
        [1024; 2],
        UvDomainContext3D {
            packet: 0,
            ordinal: 0,
            shadow_face: None,
        },
    )
    .unwrap();
    assert_eq!(
        (proof.qualified, proof.excluded, proof.conditioned),
        (1, 0, 1)
    );
    // Independent binary64 interval recalculation from the supplied packed input:
    // original determinant[-.0002156019638017981,+.0001813947599707544];
    // exact real determinant=-11919563343432475850284962937730387775 *2^-139;
    // conditioned determinant[-.000017892252679980456,-.000016315864360148633].
    assert_eq!(
        proof.minimum_determinant_margin_bits,
        Some(0.000016315864360148633_f64.to_bits())
    );
    assert_eq!(
        proof.minimum_helper_q_lower_bits,
        Some(0.22723593930719696_f64.to_bits())
    );
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R028 original packed UV counterexample: MAIN packet0/ordinal0/source10000 rev1 node0 mesh0 primitive0 triangle0; exact determinant=-11919563343432475850284962937730387775*2^-139; conditioned modeled-host determinant margin={} helper_q_lower={}; qualified1/excluded0/conditioned1 at192x128 role1024; UV-only source reproduction, no full external consumer GPU claim\n",
        f64::from_bits(proof.minimum_determinant_margin_bits.unwrap()),
        f64::from_bits(proof.minimum_helper_q_lower_bits.unwrap()),
    )).unwrap();

    // Captured MAIN sofa_02 triangle63 numerical input, supplied by independent
    // review at 2026-11-01T15:38:22.122Z (message UTF8 SHA256
    // 0e2339aacffe723764d4d3af48e0ca4fffc470430538eda505b32e56a3180208).
    // POSITION/UV/object/projection/extent and attribution are exact. As above,
    // original furniture normal/Mikk data were not supplied: normal-only
    // analytical attributes are explicit test scaffolding. The first 63 render
    // triangles repeat this numeric input solely to exercise render ordinal63.
    // The last triangle reuses the positive triangle0 numerical model above to
    // check that a later successful model cannot erase prior indeterminacy.
    // This is not the original sofa primitive or a GPU scene reproduction.
    let position_bits = [
        [3210288937, 1057457430, 1048634527],
        [3210077689, 1057424993, 1048694527],
        [3210100002, 1057456000, 1048838287],
    ];
    let uv_bits = [
        [1064017944, 1025112496],
        [1063991664, 1026587008],
        [1064028862, 1026580432],
    ];
    let captured_identity = PbrOriginalIdentity3D {
        source: 10000,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let (mut render, mut input) = super::three_d_geometry::r024_abc_fixture::packet(0, false);
    for i in 0..3 {
        render[i].position = position_bits[i].map(f32::from_bits);
        render[i].uv0 = uv_bits[i].map(f32::from_bits);
        let original = &mut Arc::make_mut(&mut input.original.vertices)[i];
        original.position = render[i].position;
        original.uv0 = Some(render[i].uv0);
    }
    input.original.identity = captured_identity;
    input.original.vertices = input
        .original
        .vertices
        .iter()
        .cycle()
        .take(65 * 3)
        .cloned()
        .collect::<Vec<_>>()
        .into();
    input.original.signed_tangents = input
        .original
        .signed_tangents
        .iter()
        .cycle()
        .take(65 * 3)
        .copied()
        .collect::<Vec<_>>()
        .into();
    input.render_vertex_sources = (0..65 * 3).collect::<Vec<_>>().into();
    input.triangles = (0..65)
        .map(|triangle| PbrTriangleCertificate3D {
            original: captured_identity,
            triangle,
            certificate_identity: 10001,
            certificate_revision: 1,
            frame: PbrFrameCertificate3D::NormalOnly {
                separator: [0.0, 0.0, 1.0],
                lower: [0.5; 3],
            },
        })
        .collect::<Vec<_>>()
        .into();
    input.mapped = false;
    input.maximum_certificate_bytes = 131_072;
    input.maximum_verification_items = 4096;
    let mut render = render.repeat(65);
    for i in 0..3 {
        render[64 * 3 + i].position = positions[i];
        render[64 * 3 + i].uv0 = uvs[i];
        let original = &mut Arc::make_mut(&mut input.original.vertices)[64 * 3 + i];
        original.position = positions[i];
        original.uv0 = Some(uvs[i]);
    }
    let geometry =
        PbrGeometrySource3D::try_new_certified(render, GeometryIndices::Unindexed, input).unwrap();
    let captured = geometry.certified_input().unwrap();
    assert_eq!(captured.original.identity, captured_identity);
    assert_eq!(captured.triangles[63].triangle, 63);
    for i in 0..3 {
        assert_eq!(
            captured.original.vertices[63 * 3 + i]
                .position
                .map(f32::to_bits),
            position_bits[i]
        );
        assert_eq!(
            captured.original.vertices[63 * 3 + i]
                .uv0
                .unwrap()
                .map(f32::to_bits),
            uv_bits[i]
        );
    }
    let proof = qualify_uv_domain(
        &geometry,
        object,
        projection,
        (192, 128),
        [1024; 2],
        UvDomainContext3D {
            packet: 0,
            ordinal: 0,
            shadow_face: None,
        },
    )
    .unwrap();
    assert_eq!(
        (proof.qualified, proof.excluded, proof.conditioned),
        (65, 0, 65)
    );
    assert_eq!(
        proof.minimum_determinant_margin_bits,
        Some(0.000010611788953355834_f64.to_bits())
    );
    assert_eq!(proof.minimum_helper_q_lower_bits, None);
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R028 captured original triangle63 host admission: MAIN packet0/ordinal0/source10000 rev1 node0 mesh0 primitive0 original63/render63 at192x128 role1024; nonzero modeled determinant margin={}; original helper-q enclosure[-0.018375466166951293,-0.01837546616695116] remains indeterminate, summary helper minimum=None despite later positive model; qualified65/excluded0/conditioned65 in explicit UV-only ordinal scaffolding, no original external consumer GPU/FP16/Image claim\n",
        f64::from_bits(proof.minimum_determinant_margin_bits.unwrap()),
    )).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn r028_metal_timestamp_query_adapter_capability_preflight() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        apply_limit_buckets: false,
        force_fallback_adapter: false,
    }))
    .expect("UNVERIFIED R028 timestamp capability: selected adapter required");
    let info = adapter.get_info();
    let features = adapter.features();
    let supported = features.contains(wgpu::Features::TIMESTAMP_QUERY);
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "R028 timestamp capability: {info:?}; features={features:?}; TIMESTAMP_QUERY={supported}; no Device/QuerySet created\n"
        ),
    )
    .unwrap();
    assert_eq!(info.name, "Apple M4 Max");
    assert_eq!(info.backend, wgpu::Backend::Metal);
    assert!(supported, "R028 hardware timestamp capability BLOCK");
}

#[cfg(target_os = "macos")]
#[test]
fn r028_actual_metal_isolated_five_role_gpu_timestamp_ratio_n1_n4() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::{MipMin3D, MipRole3D, SamplingFamily3D};
    use crate::three_d::pbr::{
        MaterialPacketSource3D, MaterialSceneSource3D, PbrGeometrySource3D, PbrMaterialSource3D,
        PbrRoleBinding3D, PbrUnitDirection3D, PbrVertexSource3D,
    };
    use crate::three_d::{SamplerAddressMode::ClampToEdge, SamplerFilterMode::Linear};

    // Frozen R028 §12.4 scope: 1920x1080; 16x9 tiles of120²;864 vertices;
    // all five original1024² roles; same controlled sample_surface and factors;
    // A=full LinearMipLinear, B=single-level Linear, MAG Linear/clamp/anisotropy1;
    // 30 paired warmups then120 measured pairs, AB/BA alternating;
    // isolated per-pass submissions completed before the next; query resolve
    // after completion; hardware timestamps, no depth/resolve/BRDF/tone/compositor;
    // p95 nearest-rank114 (index113); median mean(indices59,60); ratio<=1.5.
    // Untimed source probes: all144 tile centers at local pixel60,60 and30,30;
    // UV/gradient tolerance1e-6; sampled output tolerance.004 (signed normal.008).
    let original = producer::multiscale().provenance.original.clone();
    let roles = [
        MipRole3D::OpaqueBase,
        MipRole3D::Normal,
        MipRole3D::MetallicRoughness,
        MipRole3D::Occlusion,
        MipRole3D::Emissive,
    ];
    let pixels: [Vec<u8>; 5] = std::array::from_fn(|role| {
        original
            .chunks_exact(4)
            .flat_map(|p| {
                let c = p[0];
                match role {
                    0 => [c, c, c, 255],
                    1 => [c, 255 - c, 128 + c / 2, 255],
                    2 => [255 - c, c, 255 - c, 255],
                    3 => [c, 255 - c, c, 255],
                    4 => [255 - c, c, 64 + c / 2, 255],
                    _ => unreachable!(),
                }
            })
            .collect()
    });
    let chains: [Vec<_>; 2] = [true, false].map(|full| {
        roles
            .into_iter()
            .enumerate()
            .map(|(r, role)| {
                producer::produce(
                    role,
                    1024,
                    1024,
                    pixels[r].clone(),
                    30001 + r as u64,
                    1,
                    full,
                )
            })
            .collect()
    });
    for r in 0..5 {
        assert_eq!(
            chains[0][r].provenance.original,
            chains[1][r].provenance.original
        );
        assert_eq!(chains[0][r].level_bytes(0), chains[1][r].level_bytes(0));
        assert_eq!(chains[0][r].levels.len(), 11);
        assert_eq!(chains[1][r].levels.len(), 1);
        assert!(pixels[r].chunks_exact(4).any(|p| p != &pixels[r][..4]));
    }
    let mut vertices = Vec::with_capacity(864);
    for row in 0..9 {
        for column in 0..16 {
            let left = -16.0 / 9.0 + f64::from(column) * 2.0 / 9.0;
            let right = -16.0 / 9.0 + f64::from(column + 1) * 2.0 / 9.0;
            let top = 1.0 - f64::from(row) * 2.0 / 9.0;
            let bottom = 1.0 - f64::from(row + 1) * 2.0 / 9.0;
            for (x, y, u, v) in [
                (left, bottom, 0.0, 1.0),
                (right, bottom, 1.0, 1.0),
                (right, top, 1.0, 0.0),
                (left, bottom, 0.0, 1.0),
                (right, top, 1.0, 0.0),
                (left, top, 0.0, 0.0),
            ] {
                vertices.push(PbrVertexSource3D {
                    position: LocalPoint3::try_new(x as f32, y as f32, 0.0).unwrap(),
                    normal: PbrUnitDirection3D::try_new([0.0, 0.0, 1.0]).unwrap(),
                    uv0: Some(TextureCoordinate2::try_new(u, v).unwrap()),
                    color: None,
                    tangent: Some((PbrUnitDirection3D::try_new([1.0, 0.0, 0.0]).unwrap(), -1.0)),
                });
            }
        }
    }
    assert_eq!(vertices.len(), 864);
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let view = ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        OrthographicProjection::try_new(2.0, 0.0, 10.0)
            .unwrap()
            .into(),
    );
    let mut ratios = Vec::new();
    for n in [1, 4] {
        let (renderer, available, format_features) =
            pollster::block_on(WgpuRenderer::new_headless_timestamp_for_test())
                .expect("R028 actual timestamp-enabled test Device required");
        let adapter = renderer.headless_adapter_for_test();
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.device.features(), wgpu::Features::TIMESTAMP_QUERY);
        let context = Context3D::new(gpu.clone()).unwrap();
        let geometry = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(vertices.clone(), GeometryIndices::Unindexed).unwrap(),
                None,
            )
            .unwrap();
        let mut policies = Vec::new();
        let mut scenes = Vec::new();
        let mut role_leases = Vec::new();
        for variant in 0..2 {
            let mut material = PbrMaterialSource3D::defaults(32000 + variant as u64, 1);
            material.sampling = SamplingFamily3D::V2;
            material.emissive_rgb = [1.0; 3];
            let mut samplers = Vec::new();
            for role in 0..5 {
                let source = chains[variant][role].clone();
                let sampler = producer::sampler(
                    &source,
                    31000 + variant as u64 * 10 + role as u64,
                    if variant == 0 {
                        MipMin3D::LinearMipLinear
                    } else {
                        MipMin3D::Linear
                    },
                    Linear,
                    ClampToEdge,
                    ClampToEdge,
                );
                let t = context.register_mip_chain(source, None).unwrap();
                let s = context.register_mip_sampler(sampler, None).unwrap();
                role_leases.push((
                    context.lease_mip_chain(&t).unwrap(),
                    context.lease_mip_sampler(&s).unwrap(),
                ));
                material.roles[role] = Some(PbrRoleBinding3D::V2 {
                    texture: t,
                    sampler: s,
                });
                samplers.push(sampler);
            }
            policies.push(samplers);
            let material = context.register_pbr_material(material, None).unwrap();
            let revision = variant as u64 + 1;
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
                    vec![placement],
                )
                .unwrap();
            scenes.push(Arc::new(MaterialSceneSource3D {
                snapshot,
                model,
                hdr: crate::three_d::hdr::HdrSource3D {
                    identity: 33000,
                    revision,
                    exposure_ev: 0.0,
                    maximum_rgb: [1.0; 3],
                },
                samples: n,
                direct: None,
                indirect: None,
                observe: false,
            }));
        }
        let scenes: [Arc<MaterialSceneSource3D>; 2] = scenes.try_into().ok().unwrap();
        let benchmark = context.r028_isolated_sampling_for_test(&scenes, view, format_features);
        assert_eq!(benchmark.initial_texture_upload_batches, 10);
        let mut max_sample_error = 0.0f64;
        let mut footprint_range = [f64::INFINITY, f64::NEG_INFINITY];
        for variant in 0..2 {
            let observations = benchmark.observe(variant);
            assert_eq!(observations.len(), 288);
            for (index, o) in observations.iter().enumerate() {
                assert!(o.iter().flatten().all(|v| v.is_finite()));
                let pixel = if index % 2 == 0 { 60.5 } else { 30.5 };
                let expected_uv = pixel / 120.0;
                assert_eq!(o[1][3], 1.0, "missing tile source observation");
                assert!(
                    (f64::from(o[1][0]) - expected_uv).abs() <= 1e-6,
                    "N{n} variant{variant} probe{index}: UV={:?}, expected={expected_uv}",
                    o[1]
                );
                assert!(
                    (f64::from(o[1][1]) - expected_uv).abs() <= 1e-6,
                    "N{n} variant{variant} probe{index}: UV={:?}, expected={expected_uv}",
                    o[1]
                );
                for (actual, expected) in o[0].into_iter().zip([1.0 / 120.0, 0.0, 0.0, 1.0 / 120.0])
                {
                    assert!((f64::from(actual) - expected).abs() <= 1e-6);
                }
                let footprint = 1024.0
                    * f64::from(o[0][0])
                        .hypot(f64::from(o[0][1]))
                        .max(f64::from(o[0][2]).hypot(f64::from(o[0][3])));
                footprint_range[0] = footprint_range[0].min(footprint);
                footprint_range[1] = footprint_range[1].max(footprint);
                let lod = footprint.log2() as f32;
                let sampled: Vec<_> = (0..5)
                    .map(|role| {
                        r028_reference_sample(
                            &chains[variant][role],
                            policies[variant][role],
                            [o[1][0], o[1][1]],
                            lod,
                        )
                    })
                    .collect();
                let expected_fields = [
                    [sampled[0][0], sampled[0][1], sampled[0][2]],
                    [
                        2.0 * sampled[1][0] - 1.0,
                        2.0 * sampled[1][1] - 1.0,
                        2.0 * sampled[1][2] - 1.0,
                    ],
                    [sampled[2][2], sampled[2][1], sampled[3][0]],
                    [sampled[4][0], sampled[4][1], sampled[4][2]],
                ];
                for (field, expected) in [2, 3, 4, 5].into_iter().zip(expected_fields) {
                    for c in 0..3 {
                        let error = (f64::from(o[field][c]) - expected[c]).abs();
                        assert!(
                            error <= if field == 3 { 0.008 } else { 0.004 },
                            "role{field} actual{:?} expected{expected:?}",
                            o[field]
                        );
                        max_sample_error = max_sample_error.max(error);
                    }
                }
                for c in 0..3 {
                    let mr = [
                        sampled[2][2],
                        sampled[2][1],
                        (sampled[2][2] + sampled[2][1]) * 0.5,
                    ][c];
                    let expected =
                        (sampled[0][c] + sampled[1][c] + mr + sampled[3][0] + sampled[4][c]) * 0.2;
                    assert!((f64::from(o[6][c]) - expected).abs() <= 0.004);
                    assert!((0.0..=1.0).contains(&o[6][c]));
                }
                assert_eq!(o[6][3], 1.0);
            }
            for field in [2, 3, 4, 5, 6] {
                assert_ne!(
                    observations[0][field], observations[1][field],
                    "constant role/output at untimed UV probes"
                );
            }
            assert_ne!(
                observations[0][4][2], observations[1][4][2],
                "AO must materially vary"
            );
        }
        let usage = context.resource_usage().unwrap();
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 GPU PREDECLARED N{n}: {adapter:?}; adapter_features={available:?}; enabled={:?}; period_ns={}; target1920x1080 Rgba16Float noDepth/noResolve; tiles16x9/864vertices; footprint={footprint_range:?}; untimed_role_max_error={max_sample_error}; fullChains27962020B/baseOnly20971520B; initial_texture_upload_batches={} levels60; initial_geometry_upload_batches={}; initial_uniform_upload_bytes={}; attachment_bytes={}; query/readback_payload_bytes={}; usage={usage:?}; warm_pairs30 measured_pairs120 alternatingAB/BA; p95rank114 ratioLimit1.5; queryWindow=fixedRaster+fiveSampleGrad+shortALU+attachment\n",
            gpu.device.features(), benchmark.timestamp_period_ns, benchmark.initial_texture_upload_batches,
            benchmark.initial_geometry_upload_batches, benchmark.initial_uniform_upload_bytes, benchmark.attachment_bytes, benchmark.buffer_bytes,
        )).unwrap();
        // All source uploads and both actual untimed draws are complete. The
        // benchmark's remaining methods encode draws/query operations only.
        let (ticks, ns) = benchmark.measure();
        assert_eq!(context.resource_usage().unwrap(), usage);
        for pair in 0..120 {
            let q = (pair + 30) * 4;
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R028 GPU N{n} pair={pair:03} order={} ticks={:?} A_full_ns={:.3} B_single_ns={:.3}\n",
                if pair % 2 == 0 { "AB" } else { "BA" }, &ticks[q..q+4], ns[0][pair], ns[1][pair],
            )).unwrap();
        }
        let statistics = ns.map(|mut values| {
            assert!(values.into_iter().all(|v| v.is_finite() && v > 0.0));
            values.sort_by(f64::total_cmp);
            [
                values[0],
                (values[59] + values[60]) * 0.5,
                values[113],
                values[119],
            ]
        });
        let ratio = statistics[0][2] / statistics[1][2];
        std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "R028 GPU RESULT N{n}: queries600 warm_pairs30 measured_pairs120; A_ns[min,median,p95,max]={:?}; B_ns[min,median,p95,max]={:?}; p95_ratio={ratio:.12} threshold1.5 outcome={}; warm_measure_texture_uploads=0; first_tick={} last_tick={}\n",
            statistics[0], statistics[1], if ratio <= 1.5 { "PASS" } else { "FAIL" }, ticks[0], ticks[599],
        )).unwrap();
        ratios.push((n, ratio));
        drop(benchmark);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
        assert_eq!(role_leases.len(), 10);
    }
    assert!(
        ratios.iter().all(|(_, ratio)| *ratio <= 1.5),
        "R028 isolated GPU measured FAIL; no source/workload/threshold tuning authorized: {ratios:?}"
    );
}

pub(super) fn r028_reference_sample(
    source: &crate::three_d::mip_sampling::MipChainSource3D,
    policy: crate::three_d::mip_sampling::MipSamplerSource3D,
    uv: [f32; 2],
    lambda: f32,
) -> [f64; 4] {
    use crate::three_d::mip_sampling::MipMin3D::*;
    use crate::three_d::{SamplerAddressMode as Address, SamplerFilterMode};
    let address = |i: i64, n: i64, a: Address| match a {
        Address::ClampToEdge => i.clamp(0, n - 1),
        Address::Repeat => i.rem_euclid(n),
        Address::MirrorRepeat => {
            let j = i.rem_euclid(2 * n);
            if j < n { j } else { 2 * n - 1 - j }
        }
    };
    let linear = if lambda < 0.0 {
        policy.mag == SamplerFilterMode::Linear
    } else {
        matches!(policy.min, Linear | LinearMipNearest | LinearMipLinear)
    };
    let level = |l: usize| {
        let desc = source.levels[l];
        let bytes = source.level_bytes(l).unwrap();
        let texel = |x: i64, y: i64| {
            let x = address(x, i64::from(desc.width), policy.u);
            let y = address(y, i64::from(desc.height), policy.v);
            let at = ((y * i64::from(desc.width) + x) * 4) as usize;
            std::array::from_fn::<_, 4, _>(|c| {
                if c < 3 && source.format == TextureFormat3D::Rgba8UnormSrgb {
                    super::three_d_context::r028_fixture::decode(bytes[at + c])
                } else {
                    f64::from(bytes[at + c]) / 255.0
                }
            })
        };
        if !linear {
            return texel(
                (f64::from(uv[0]) * f64::from(desc.width)).floor() as i64,
                (f64::from(uv[1]) * f64::from(desc.height)).floor() as i64,
            );
        }
        let x = f64::from(uv[0]) * f64::from(desc.width) - 0.5;
        let y = f64::from(uv[1]) * f64::from(desc.height) - 0.5;
        let ix = x.floor() as i64;
        let iy = y.floor() as i64;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let a = texel(ix, iy);
        let b = texel(ix + 1, iy);
        let c = texel(ix, iy + 1);
        let d = texel(ix + 1, iy + 1);
        std::array::from_fn(|i| {
            (1.0 - fy) * ((1.0 - fx) * a[i] + fx * b[i]) + fy * ((1.0 - fx) * c[i] + fx * d[i])
        })
    };
    if matches!(policy.min, Nearest | Linear) || lambda <= 0.0 {
        return level(0);
    }
    let lod = f64::from(lambda).min((source.levels.len() - 1) as f64);
    if matches!(policy.min, NearestMipNearest | LinearMipNearest) {
        return level(lod.round() as usize);
    }
    let low = lod.floor() as usize;
    let high = (low + 1).min(source.levels.len() - 1);
    let f = lod - lod.floor();
    let a = level(low);
    let b = level(high);
    std::array::from_fn(|c| a[c] * (1.0 - f) + b[c] * f)
}

#[test]
fn r028_actual_multiscale_explicit_levels_gradient_lod_six_min_independent_mag_and_addresses() {
    use super::three_d_context::r028_fixture as producer;
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode::*, SamplerFilterMode};
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R028: actual Device required");
    let adapter = renderer.headless_adapter_for_test();
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let source = producer::multiscale();
    let handle = context.register_mip_chain(source.clone(), None).unwrap();
    let chain = context.lease_mip_chain(&handle).unwrap();
    let mut id = 28500;
    let mut maximum_error = 0.0f64;
    for min in [
        MipMin3D::Nearest,
        MipMin3D::Linear,
        MipMin3D::NearestMipNearest,
        MipMin3D::LinearMipNearest,
        MipMin3D::NearestMipLinear,
        MipMin3D::LinearMipLinear,
    ] {
        for mag in [SamplerFilterMode::Nearest, SamplerFilterMode::Linear] {
            for (u, v) in [
                (ClampToEdge, ClampToEdge),
                (Repeat, MirrorRepeat),
                (MirrorRepeat, Repeat),
            ] {
                id += 1;
                let policy = producer::sampler(&source, id, min, mag, u, v);
                let sh = context.register_mip_sampler(policy, None).unwrap();
                let sampler = context.lease_mip_sampler(&sh).unwrap();
                let mut cases = Vec::new();
                for uv in [
                    [101.3 / 1024.0, 307.2 / 1024.0],
                    [-0.0013, 1.0017],
                    [1.003, -0.003],
                    [-1.17, -2.31],
                ] {
                    for lambda in [-2.0f32, 0.27, 2.27, 3.73, 7.21, 10.0] {
                        cases.push((uv, lambda));
                    }
                }
                let queries = cases
                    .iter()
                    .map(|(uv, l)| {
                        [
                            [uv[0], uv[1], 0.0, 1.0],
                            [2.0f32.powf(*l) / 1024.0, 0.0, 0.0, 2.0f32.powf(*l) / 1024.0],
                        ]
                    })
                    .collect::<Vec<_>>();
                let actual = sample_probe_for_test(&context, &chain, &sampler, &queries).unwrap();
                for ((uv, l), value) in cases.iter().zip(actual) {
                    let expected = r028_reference_sample(&source, policy, *uv, *l);
                    for c in 0..4 {
                        let error = (f64::from(value[c]) - expected[c]).abs();
                        maximum_error = maximum_error.max(error);
                        assert!(
                            error <= 0.01,
                            "{min:?}/{mag:?}/{u:?}/{v:?}, UV{uv:?} lambda{l}: actual{value:?}, reference{expected:?}, error{error}"
                        );
                    }
                }
            }
        }
    }
    let policy = producer::sampler(
        &source,
        28600,
        MipMin3D::NearestMipNearest,
        SamplerFilterMode::Nearest,
        ClampToEdge,
        ClampToEdge,
    );
    let sh = context.register_mip_sampler(policy, None).unwrap();
    let sampler = context.lease_mip_sampler(&sh).unwrap();
    let queries = (0..9)
        .map(|l| {
            let n = (1024 >> l) as f32;
            [
                [
                    ((101 >> l) as f32 + 0.5) / n,
                    ((307 >> l) as f32 + 0.5) / n,
                    l as f32,
                    0.0,
                ],
                [0.0; 4],
            ]
        })
        .collect::<Vec<_>>();
    let actual = sample_probe_for_test(&context, &chain, &sampler, &queries).unwrap();
    let expected = [168, 147, 132, 115, 127, 144, 136, 125, 129];
    for (value, byte) in actual.iter().zip(expected) {
        for c in 0..3 {
            assert!((f64::from(value[c]) - producer::decode(byte)).abs() < 0.001);
        }
    }
    // The checker remains an independent §12.1 source-stage oracle.
    let checker = producer::checker(MipRole3D::OpaqueBase, 1024, 28610);
    let th = context.register_mip_chain(checker.clone(), None).unwrap();
    let t = context.lease_mip_chain(&th).unwrap();
    let sp = producer::sampler(
        &checker,
        28611,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        ClampToEdge,
        ClampToEdge,
    );
    let sh = context.register_mip_sampler(sp, None).unwrap();
    let s = context.lease_mip_sampler(&sh).unwrap();
    let checker_values = sample_probe_for_test(
        &context,
        &t,
        &s,
        &[
            [[0.37, 0.61, 3.0, 0.0], [0.0; 4]],
            [
                [0.37, 0.61, 0.0, 1.0],
                [8.0 / 1024.0, 0.0, 0.0, 8.0 / 1024.0],
            ],
        ],
    )
    .unwrap();
    for v in &checker_values {
        for c in 0..3 {
            assert!((f64::from(v[c]) - producer::decode(188)).abs() <= 0.03);
        }
    }
    let linear_midpoint =
        sample_probe_for_test(&context, &t, &s, &[[[0.5, 0.5, 0.0, 0.0], [0.0; 4]]]).unwrap();
    for c in 0..3 {
        assert!(
            (linear_midpoint[0][c] - 0.5).abs() < 0.003,
            "sRGB decode must precede spatial interpolation: {linear_midpoint:?}"
        );
    }
    // Also prove no-mip MIN/MAG with genuinely one-level content. Resident
    // extra mips above and an absent mip chain here must have identical policy.
    let single = producer::produce(
        MipRole3D::OpaqueBase,
        1024,
        1024,
        source.provenance.original.to_vec(),
        28620,
        1,
        false,
    );
    let th = context.register_mip_chain(single.clone(), None).unwrap();
    let t = context.lease_mip_chain(&th).unwrap();
    for min in [MipMin3D::Nearest, MipMin3D::Linear] {
        for mag in [SamplerFilterMode::Nearest, SamplerFilterMode::Linear] {
            id += 1;
            let policy = producer::sampler(&single, id, min, mag, ClampToEdge, ClampToEdge);
            let sh = context.register_mip_sampler(policy, None).unwrap();
            let s = context.lease_mip_sampler(&sh).unwrap();
            let uv = [101.3 / 1024.0, 307.2 / 1024.0];
            let lambdas = [-2.0f32, 0.27, 3.7, 10.0];
            let queries = lambdas.map(|l| {
                [
                    [uv[0], uv[1], 0.0, 1.0],
                    [2.0f32.powf(l) / 1024.0, 0.0, 0.0, 2.0f32.powf(l) / 1024.0],
                ]
            });
            let actual = sample_probe_for_test(&context, &t, &s, &queries).unwrap();
            for (v, l) in actual.into_iter().zip(lambdas) {
                let expected = r028_reference_sample(&single, policy, uv, l);
                for c in 0..4 {
                    assert!(
                        (f64::from(v[c]) - expected[c]).abs() < 0.01,
                        "single-level {min:?}/{mag:?} lambda{l}: {v:?}/{expected:?}"
                    );
                }
            }
        }
    }
    // Real non-256-byte rows, NPOT dimensions and every uploaded level. These
    // numeric/normal source bytes must never pass through an sRGB decoder.
    for (case, role, w, h, raw) in [
        (
            0,
            MipRole3D::Occlusion,
            3,
            5,
            (0..15)
                .flat_map(|i| [if i / 3 == 4 { 255 } else { 0 }, 17, 63, 128])
                .collect(),
        ),
        (
            1,
            MipRole3D::Normal,
            3,
            5,
            (0..15)
                .flat_map(|i| {
                    [
                        if i % 2 == 0 { 0 } else { 255 },
                        if i % 3 == 0 { 255 } else { 0 },
                        128,
                        255,
                    ]
                })
                .collect(),
        ),
        (
            2,
            MipRole3D::Occlusion,
            1,
            8,
            (0..8).flat_map(|i| [i * 32, 64, 192, 255]).collect(),
        ),
    ] {
        let src = producer::produce(role, w, h, raw, 28640 + case, 1, true);
        let th = context.register_mip_chain(src.clone(), None).unwrap();
        let t = context.lease_mip_chain(&th).unwrap();
        let policy = producer::sampler(
            &src,
            28650 + case,
            MipMin3D::NearestMipNearest,
            SamplerFilterMode::Nearest,
            ClampToEdge,
            ClampToEdge,
        );
        let sh = context.register_mip_sampler(policy, None).unwrap();
        let s = context.lease_mip_sampler(&sh).unwrap();
        let queries = (0..src.levels.len())
            .map(|l| [[0.5, 0.75, l as f32, 0.0], [0.0; 4]])
            .collect::<Vec<_>>();
        let actual = sample_probe_for_test(&context, &t, &s, &queries).unwrap();
        for (l, value) in actual.iter().enumerate() {
            let expected = r028_reference_sample(&src, policy, [0.5, 0.75], l as f32);
            for c in 0..4 {
                assert!(
                    (f64::from(value[c]) - expected[c]).abs() < 0.00001,
                    "NPOT/strip {case} L{l} {value:?}/{expected:?}"
                );
            }
            if role == MipRole3D::Normal {
                assert!(2.0 * value[2] - 1.0 > 0.0);
            }
        }
    }
    assert_eq!(
        context.resource_usage().unwrap().admitted_shader_modules,
        0,
        "fixed probes retire real modules after observed completion"
    );
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R028 {adapter:?}; wgpu30.0.2/Naga30.0.2: 6MIN x2MAG x3UV policies, max linear error={maximum_error}; multiscale L0..8={actual:?}; checker LOD3/grad8={checker_values:?}\n")).unwrap();
}

#[test]
fn r026_actual_indirect_arithmetic_roughness_cube_world_axes_and_lut_filtering() {
    use super::three_d_viewport::r026_fixture as f;
    use crate::three_d::indirect::indirect_probe_for_test;
    use crate::three_d::indirect::{IndirectBlock112, Presence3D};
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R026: actual numeric Device required");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let input = |n: [f32; 3], v: [f32; 3], r: f32, ao: f32| {
        [
            [0.5, 0.5, 0.5, 0.0],
            [r, ao, 0.0, 0.0],
            [n[0], n[1], n[2], 0.0],
            [v[0], v[1], v[2], 0.0],
        ]
    };
    // These values are ONLY arithmetic operands, deliberately not given an E
    // producer certificate or sent through a production source-admission path.
    let constants = |d: f64, s: [f64; 8], ab: [f64; 2]| {
        let mut diffuse = Vec::with_capacity(32 * 32 * 6 * 4);
        for _ in 0..32 * 32 * 6 {
            diffuse.extend([f::half_down(d); 4]);
        }
        let mut spec = Vec::with_capacity(128 * 128 * 48 * 4);
        for x in s {
            for _ in 0..128 * 128 * 6 {
                spec.extend([f::half_down(x); 4]);
            }
        }
        let mut lut = Vec::with_capacity(128 * 128 * 2);
        for _ in 0..128 * 128 {
            lut.extend(ab.map(f::half_down));
        }
        [Arc::<[u16]>::from(diffuse), Arc::from(spec), Arc::from(lut)]
    };
    let e = constants(1.0, [2.0; 8], [0.5, 0.125]);
    let mut block = IndirectBlock112 {
        header: [1, 0, 0, 0],
        lobes: [[0.0; 4]; 6],
    };
    let x = [1.0, 0.0, 0.0];
    let actual = indirect_probe_for_test(
        &context,
        Presence3D(2),
        block,
        Some(&e),
        &[input(x, x, 1.0, 1.0), input(x, x, 1.0, 0.5)],
    )
    .unwrap();
    for (row, ao) in actual.iter().zip([1.0, 0.5]) {
        for c in 0..3 {
            assert!((row[c] - (ao * 0.48 / std::f32::consts::PI + 0.29)).abs() < 2e-6);
        }
    }
    block.header = [0, 1, 0, 0];
    block.lobes[0] = [1.0, 1.0, 1.0, 0.0];
    block.lobes[3] = [2.0, 2.0, 2.0, 0.0];
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let directions = [
        x,
        [-1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [d, -d, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let rows = indirect_probe_for_test(
        &context,
        Presence3D(4),
        block,
        None,
        &directions.map(|n| input(n, n, 1.0, 1.0)),
    )
    .unwrap();
    for (row, irr) in rows.iter().zip([1.0, 0.0, 2.0, 1.5, 0.0]) {
        assert!((row[0] - 0.48 * irr / std::f32::consts::PI).abs() < 2e-6);
    }
    block.header = [1, 1, 0, 0];
    block.lobes[0] = [3.0, 3.0, 3.0, 0.0];
    let rows = indirect_probe_for_test(
        &context,
        Presence3D(6),
        block,
        Some(&e),
        &[
            input(x, x, 1.0, 1.0),
            input(x, x, 1.0, 0.5),
            input(x, [-1.0, 0.0, 0.0], 1.0, 1.0),
            input(x, [0.0; 3], 1.0, 1.0),
        ],
    )
    .unwrap();
    for (row, ao) in rows[..2].iter().zip([1.0, 0.5]) {
        assert!((row[0] - (ao * 1.92 / std::f32::consts::PI + 0.29)).abs() < 2e-6);
    }
    assert_eq!(&rows[2][..3], &[0.0; 3]);
    assert_eq!(&rows[3][..3], &[0.0; 3]);
    let levels = constants(0.0, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], [1.0, 0.0]);
    let mut inputs = Vec::new();
    for i in 0..8 {
        inputs.push(input(x, x, 0.045 + 0.955 * i as f32 / 7.0, 0.0));
    }
    inputs.push(input(x, x, f32::from_bits(1.0f32.to_bits() - 1), 0.0));
    inputs.push(input(x, x, 0.0, 0.0));
    let b = IndirectBlock112 {
        header: [1, 0, 0, 0],
        lobes: [[0.0; 4]; 6],
    };
    let rows = indirect_probe_for_test(&context, Presence3D(2), b, Some(&levels), &inputs).unwrap();
    for i in 0..8 {
        assert!(
            (rows[i][0] - 0.04 * (i + 1) as f32).abs() < 1e-6,
            "level{i}: {:?}",
            rows[i]
        );
    }
    assert!((rows[8][0] - 0.32).abs() < 1e-6);
    assert_eq!(rows[9], rows[0]);
    // Actual nonconstant, matched WORLD assets from the analytical fixture.
    // X bright stays at WORLD X; changing the producer's original orientation
    // supplies different bytes, not a camera/shader rotation.
    let assets = f::assets(glam::DVec3::X, 0.125, [0.5; 3]);
    let mut world = Vec::new();
    for n in [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ] {
        world.push(input(n, n, 1.0, 1.0));
    }
    let rows = indirect_probe_for_test(&context, Presence3D(2), b, Some(&assets), &world).unwrap();
    assert!(rows[0][0] > rows[2][0] && rows[2][0] > rows[1][0]);
    for i in 3..6 {
        assert!((rows[i][0] - rows[2][0]).abs() < 0.0005);
    }
    let rotated = f::assets(glam::DVec3::Y, 0.125, [0.5; 3]);
    let rotated_rows =
        indirect_probe_for_test(&context, Presence3D(2), b, Some(&rotated), &world).unwrap();
    assert!((rotated_rows[2][0] - rows[0][0]).abs() < 0.0005);
    assert!(rotated_rows[0][0] < rotated_rows[2][0]);
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R026 actual controlled arithmetic E={actual:?}; signed G={:?}; WORLD faces={rows:?}; r0/r7/endpoints passed\n",directions)).unwrap();
}

#[test]
fn r025_actual_nearest_boolean_pcf_bias_and_six_chart_cube_taps() {
    use crate::three_d::direct_shadow::{
        DirectProbeInput3D, direct_compare_probe_for_test, point_face, point_taps,
    };
    use glam::DVec3;
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: real comparison Device required");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let two_d = |x: f32, n: [f32; 3]| DirectProbeInput3D {
        p: [x, 0.0, 0.5, 0.0],
        geometric: [n[0], n[1], n[2], 0.0],
        light: [0.0, 0.0, 1.0, 10.0],
        mode: [0, 0, 0, 0],
    };
    let assert_boolean = |rows: &Vec<[[f32; 4]; 20]>| {
        for row in rows {
            let mut sum = 0.0;
            for observation in &row[1..10] {
                assert!(
                    observation[3] == 0.0 || observation[3] == 1.0,
                    "fractional hardware PCF substituted for one Boolean: {observation:?}"
                );
                sum += observation[3];
            }
            assert_eq!(row[0][0], sum / 9.0);
            assert_eq!(row[0][0], row[0][1]);
        }
    };
    let half = direct_compare_probe_for_test(
        &context,
        &[
            two_d(-0.5, [0.0, 0.0, 1.0]),
            two_d(0.5, [0.0, 0.0, 1.0]),
            two_d(0.0, [0.0, 0.0, 1.0]),
        ],
        0,
    )
    .unwrap();
    assert_boolean(&half);
    assert_eq!(half[0][0][0], 1.0);
    assert_eq!(half[1][0][0], 0.0);
    assert_eq!(half[2][0][0], 1.0 / 3.0);
    let bias = direct_compare_probe_for_test(
        &context,
        &[two_d(0.0, [0.0, 0.0, 1.0]), two_d(0.0, [0.6, 0.0, 0.8])],
        1,
    )
    .unwrap();
    assert_boolean(&bias);
    assert_eq!(bias[0][0][0], 0.0);
    assert_eq!(bias[1][0][0], 1.0);
    assert_eq!(bias[0][0][2], 0.5 + 1.0 / 65536.0);
    let point = |q: DVec3, d: f64| {
        let q = q.normalize().as_vec3();
        DirectProbeInput3D {
            p: [q.x, q.y, q.z, d as f32],
            geometric: [-q.x, -q.y, -q.z, 0.0],
            light: [-q.x, -q.y, -q.z, 10.0],
            mode: [1, 0, 0, 0],
        }
    };
    let centers = [
        DVec3::X,
        -DVec3::X,
        DVec3::Y,
        -DVec3::Y,
        DVec3::Z,
        -DVec3::Z,
    ];
    let mut center_cases = Vec::new();
    let mut center_expected = Vec::new();
    for (face, q) in centers.into_iter().enumerate() {
        for reference in [0.05f64, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65] {
            let d = 0.1 / (reference * 9.99 + 0.01);
            center_cases.push(point(q, d));
            center_expected.push(if reference > 0.1 + 0.1 * face as f64 {
                1.0
            } else {
                0.0
            });
        }
    }
    let rho = 2.0 / 512.0;
    let crossing = DVec3::new(1.0, 0.0, 1.0 + rho * 0.5).normalize();
    let crossing_d = (0.1 / (0.35 * 9.99 + 0.01)) / crossing.abs().max_element();
    center_cases.push(point(crossing, crossing_d));
    let centers_gpu = direct_compare_probe_for_test(&context, &center_cases, 2).unwrap();
    assert_boolean(&centers_gpu);
    for (i, expected) in center_expected.into_iter().enumerate() {
        assert_eq!(
            centers_gpu[i][0][0], expected,
            "native cube layer/face {i}: {:?}",
            centers_gpu[i][0]
        );
    }
    let cross = centers_gpu.last().unwrap();
    assert_eq!(cross[0][0], 1.0 / 3.0);
    let mut directions = centers.to_vec();
    for a in 0..3 {
        for b in a + 1..3 {
            for sa in [-1.0, 1.0] {
                for sb in [-1.0, 1.0] {
                    let mut q = DVec3::ZERO;
                    q[a] = sa;
                    q[b] = sb;
                    directions.push(q.normalize());
                }
            }
        }
    }
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                directions.push(DVec3::new(x, y, z).normalize());
            }
        }
    }
    directions.push(DVec3::new(3.0, 4.0, 0.0).normalize());
    let cases = directions
        .iter()
        .map(|q| point(*q, 5.0))
        .collect::<Vec<_>>();
    let atlas = direct_compare_probe_for_test(&context, &cases, 2).unwrap();
    assert_boolean(&atlas);
    for (i, p) in cases.iter().enumerate() {
        let q = DVec3::new(f64::from(p.p[0]), f64::from(p.p[1]), f64::from(p.p[2]));
        assert_eq!(atlas[i][0][3], point_face(q) as f32);
        let taps = point_taps(q).unwrap();
        for j in 0..9 {
            let actual = DVec3::new(
                f64::from(atlas[i][1 + j][0]),
                f64::from(atlas[i][1 + j][1]),
                f64::from(atlas[i][1 + j][2]),
            );
            assert!(
                (actual - taps[j]).abs().max_element() < 0.000002,
                "base-frame tap reanchor/sign error at case{i} tap{j}"
            );
            let major = 5.0 * actual.abs().max_element();
            let z = 0.01 * (10.0 - major) / (major * 9.99);
            assert!((f64::from(atlas[i][10 + j][1]) - z).abs() < 0.00000001);
        }
    }
    let face_depth = atlas.last().unwrap()[14][1];
    assert!((face_depth - 1.0 / 666.0).abs() < 1e-8);
    assert!((face_depth - 1.0 / 999.0).abs() > 0.0001);
    // Native cube U/V quadrants distinguish the two axes on every face. The
    // nine nearby taps remain strictly within the selected source quadrant.
    let us = [-DVec3::Z, DVec3::Z, DVec3::X, DVec3::X, DVec3::X, -DVec3::X];
    let vs = [
        -DVec3::Y,
        -DVec3::Y,
        DVec3::Z,
        -DVec3::Z,
        -DVec3::Y,
        -DVec3::Y,
    ];
    let mut quadrant_cases = Vec::new();
    let mut quadrant_expected = Vec::new();
    for face in 0..6 {
        for u in [-1.0, 1.0] {
            for v in [-1.0, 1.0] {
                let q = (centers[face] + 0.25 * u * us[face] + 0.25 * v * vs[face]).normalize();
                let depth =
                    0.125 + if u > 0.0 { 0.25 } else { 0.0 } + if v > 0.0 { 0.125 } else { 0.0 };
                for reference in [0.2, 0.3, 0.45] {
                    let d = (0.1 / (reference * 9.99 + 0.01)) / q.abs().max_element();
                    quadrant_cases.push(point(q, d));
                    quadrant_expected.push(if reference > depth { 1.0 } else { 0.0 });
                }
            }
        }
    }
    let quadrants = direct_compare_probe_for_test(&context, &quadrant_cases, 3).unwrap();
    assert_boolean(&quadrants);
    for (i, expected) in quadrant_expected.into_iter().enumerate() {
        assert_eq!(quadrants[i][0][0], expected, "cube U/V quadrant {i}");
    }
    // Distinct per-face source depths and separating references make the exact
    // edge/corner tie rule falsifiable. Far-away/all-failing comparisons alone
    // cannot establish that native cube sampling agrees with X>Y>Z ownership.
    let mut tie_cases = Vec::new();
    for q in directions.iter().skip(6) {
        for reference in [0.05, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65] {
            let d = (0.1 / (reference * 9.99 + 0.01)) / q.abs().max_element();
            tie_cases.push(point(*q, d));
        }
    }
    let ties = direct_compare_probe_for_test(&context, &tie_cases, 2).unwrap();
    assert_boolean(&ties);
    for (i, row) in ties.iter().enumerate() {
        for tap in 0..9 {
            let q = DVec3::new(
                row[1 + tap][0] as f64,
                row[1 + tap][1] as f64,
                row[1 + tap][2] as f64,
            );
            let face = point_face(q);
            assert_eq!(row[10 + tap][0], face as f32);
            let stored = 0.1f32 + 0.1f32 * face as f32;
            let expected = if row[10 + tap][2] >= stored { 1.0 } else { 0.0 };
            assert_eq!(
                row[1 + tap][3],
                expected,
                "native cube face differs from qualified owner at case{i} tap{tap}, q={q:?}, face{face}, ref={}",
                row[10 + tap][2]
            );
        }
    }
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual comparison {:?}: nearest V={:?}; geometric/shading bias={:?}/{:?}; cube cross-face V={}; six centers+12 edges+8 corners; face depth={}\n",renderer.headless_adapter_for_test(),half.iter().map(|v|v[0][0]).collect::<Vec<_>>(),bias[0][0],bias[1][0],cross[0][0],face_depth)).unwrap();
}

#[test]
fn r025_actual_point_support_and_spot_shoulders_use_production_direct_evaluator() {
    use crate::three_d::direct_shadow::{DirectProbeInput3D, direct_compare_probe_for_test};
    use glam::DVec3;
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R025: real attenuation and spot Device required");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let probe = |p: DVec3, normal: DVec3| DirectProbeInput3D {
        p: [p.x as f32, p.y as f32, p.z as f32, 0.0],
        geometric: [normal.x as f32, normal.y as f32, normal.z as f32, 0.0],
        light: [0.0; 4],
        mode: [2, 0, 0, 0],
    };
    let distances = [0.0, 0.05, 1.0, 9.9, 10.0, 10.1];
    let point_cases = distances.map(|d| probe(DVec3::new(0.0, 0.0, 2.0 - d), DVec3::Z));
    let point = direct_compare_probe_for_test(&context, &point_cases, 4).unwrap();
    for (i, d) in distances.into_iter().enumerate() {
        let expected = if d == 0.0 || d >= 10.0 {
            0.0
        } else {
            let w = (1.0 - (d / 10.0).powi(4)).powi(2);
            w / (d * d).max(0.1 * 0.1) / (4.0 * std::f64::consts::PI)
        };
        for channel in 0..3 {
            let actual = f64::from(point[i][0][channel]);
            assert!(
                (actual - expected).abs() <= 0.000002 * expected.max(0.001),
                "point d{d}: {actual} != {expected}"
            );
        }
        if d == 0.0 || d >= 10.0 {
            assert_eq!(&point[i][0][..3], &[0.0; 3]);
        }
    }
    let angles = [0.0f64, 0.2, 0.45, 0.7, 0.8];
    let spot_cases = angles.map(|a| {
        let l = DVec3::new(-a.sin(), 0.0, a.cos());
        probe(DVec3::new(0.0, 0.0, 2.0) - 2.0 * l, l)
    });
    let spot = direct_compare_probe_for_test(&context, &spot_cases, 5).unwrap();
    for (i, a) in angles.into_iter().enumerate() {
        let p = DVec3::new(spot_cases[i].p[0] as f64, 0.0, spot_cases[i].p[2] as f64);
        let l = (DVec3::new(0.0, 0.0, 2.0) - p).normalize();
        let nv = l.dot((DVec3::new(0.0, 0.0, 4.0) - p).normalize());
        let t = ((a.cos() - f64::from(0.7f32.cos())) / f64::from(0.2f32.cos() - 0.7f32.cos()))
            .clamp(0.0, 1.0);
        let cone = t * t * (3.0 - 2.0 * t);
        let incident = (1.0 - 0.2f64.powi(4)).powi(2) * cone / 4.0;
        let expected = incident / (std::f64::consts::PI * 2.0 * (nv + 1.0));
        assert!(
            (f64::from(spot[i][0][0]) - expected).abs() < 0.0000001,
            "spot angle{a}: {:?} != {expected}",
            spot[i][0]
        );
        assert!(spot[i][0][..3].iter().all(|v| v.is_finite() && *v >= 0.0));
    }
    assert_eq!(&spot[4][0][..3], &[0.0; 3]);
    std::io::Write::write_fmt(&mut std::io::stderr(),format_args!("R025 actual production direct evaluator {:?}: point d0/radius/1/9.9/R/outside={:?}; spot 0/inner/shoulder/outer/outside={:?}\n", renderer.headless_adapter_for_test(), point.iter().map(|r|r[0]).collect::<Vec<_>>(),spot.iter().map(|r|r[0]).collect::<Vec<_>>())).unwrap();
}

#[test]
fn r024_actual_controlled_brdf_f32_roughness_metals_grazing_and_guard_domain() {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R024: actual Device required");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let p = |c: [f32; 3], m, r, n: [f32; 3], v: [f32; 3], l: [f32; 3]| {
        [
            [c[0], c[1], c[2], 0.0],
            [m, r, 0.0, 0.0],
            [n[0], n[1], n[2], 0.0],
            [v[0], v[1], v[2], 0.0],
            [l[0], l[1], l[2], 0.0],
        ]
    };
    let z = [0.0, 0.0, 1.0];
    let cases = [
        p([0.5; 3], 1.0, 0.0, z, z, z),
        p([0.5; 3], 1.0, 0.045, z, z, z),
        p([0.5; 3], 1.0, 1.0, z, z, z),
        p([0.5; 3], 0.0, 1.0, z, z, z),
        p(
            [0.5; 3],
            0.0,
            0.0,
            z,
            [1.0, 0.0, 0.0001],
            [-1.0, 0.0, 0.0001],
        ),
        p([0.5; 3], 0.0, 1.0, z, [0.0; 3], z),
        p([0.5; 3], 0.0, 1.0, z, z, [0.0, 0.0, -1.0]),
        p([0.5; 3], 0.0, 1.0, z, [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
    ];
    let values = crate::three_d::pbr::material_brdf_probe_for_test(&context, &cases).unwrap();
    assert_eq!(values[0], values[1]);
    assert!(values.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    let metal = 0.5 / (4.0 * std::f32::consts::PI);
    let dielectric = 0.96 * 0.5 / std::f32::consts::PI + 0.04 / (4.0 * std::f32::consts::PI);
    assert!((values[2][0] - metal).abs() < 1e-6);
    assert!((values[3][0] - dielectric).abs() < 1e-6);
    for v in &values[5..] {
        assert_eq!(&v[..3], &[0.0; 3]);
    }
    let a2 = 0.045f64.powi(4);
    let expected = 0.5 / (4.0 * std::f64::consts::PI * a2);
    assert!((f64::from(values[0][0]) / expected - 1.0).abs() < 1e-5);
    std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "R024 actual production BRDF on {:?}: r0/r.045={:?}; grazing={:?}; guards={:?}\n",
            renderer.headless_adapter_for_test(),
            values[0],
            values[4],
            &values[5..]
        ),
    )
    .unwrap();
}

fn setup(policy: PolicyLimits3D) -> Option<(WgpuRenderer, GpuResources, Context3D)> {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())?;
    let gpu = renderer.gpu_resources();
    let context = Context3D::with_policy(gpu.clone(), policy).unwrap();
    Some((renderer, gpu, context))
}

fn point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

fn uv(u: f32, v: f32) -> TextureCoordinate2 {
    TextureCoordinate2::try_new(u, v).unwrap()
}

fn normal() -> LocalVector3 {
    LocalVector3::try_new(0.0, 0.0, 1.0).unwrap()
}

fn triangle(clockwise: bool, with_uvs: bool, with_normals: bool) -> GeometrySourceDescriptor {
    let mut positions = vec![
        point(-0.78, -0.74, 0.0),
        point(0.82, -0.66, 0.0),
        point(-0.07, 0.79, 0.0),
    ];
    let mut texture_coordinates = vec![uv(0.0, 0.0), uv(1.0, 0.0), uv(0.5, 1.0)];
    if clockwise {
        positions.swap(1, 2);
        texture_coordinates.swap(1, 2);
    }
    GeometrySourceDescriptor::try_new(
        positions,
        with_normals.then(|| vec![normal(); 3]),
        with_uvs.then_some(texture_coordinates),
        None,
        GeometryIndices::Unindexed,
    )
    .unwrap()
}

fn full_quad(with_uvs: bool, with_normals: bool) -> GeometrySourceDescriptor {
    let positions = vec![
        point(-1.2, -1.2, 0.0),
        point(1.2, -1.2, 0.0),
        point(1.2, 1.2, 0.0),
        point(-1.2, -1.2, 0.0),
        point(1.2, 1.2, 0.0),
        point(-1.2, 1.2, 0.0),
    ];
    let texture_coordinates = vec![
        uv(0.0, 0.0),
        uv(1.0, 0.0),
        uv(1.0, 1.0),
        uv(0.0, 0.0),
        uv(1.0, 1.0),
        uv(0.0, 1.0),
    ];
    GeometrySourceDescriptor::try_new(
        positions,
        with_normals.then(|| vec![normal(); 6]),
        with_uvs.then_some(texture_coordinates),
        None,
        GeometryIndices::Unindexed,
    )
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

fn fallback_source() -> &'static str {
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
) -> vec3<f32> {
    return textureSample(appearance_texture, appearance_sampler, uv).rgb
        * color_factor_linear.rgb;
}
"#
}

fn mask_source(color: [f32; 3]) -> String {
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
    if uv.x + uv.y > 1.0 {{
        discard;
    }}
    return vec3<f32>({:.8}, {:.8}, {:.8});
}}
"#,
        color[0], color[1], color[2]
    )
}

fn appearance(
    source: impl Into<String>,
    semantics: GeometrySemantics,
    mode: AppearanceMode,
) -> AppearanceSourceDescriptor {
    AppearanceSourceDescriptor::try_new_with_mode(source, semantics, mode).unwrap()
}

fn placement(x: f64, y: f64, z: f64) -> ModelPlacement3 {
    ModelPlacement3::try_new(
        WorldPoint3::try_new(x, y, z).unwrap(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap()
}

fn view(projection: impl Into<crate::three_d::Projection3>) -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(WorldPoint3::origin(), Orientation3::identity()),
        projection.into(),
    )
}

fn finite_view() -> ViewDescriptor3 {
    view(PerspectiveProjection::try_finite(std::f32::consts::FRAC_PI_2, 0.1, 10.0).unwrap())
}

#[derive(Clone)]
struct Scene {
    snapshot: RenderSnapshot,
    model: ModelState,
}

fn snapshot(
    context: &Context3D,
    geometry: GeometrySourceDescriptor,
    appearance: AppearanceSourceDescriptor,
    placements: &[[f64; 3]],
    raster_mode: RasterMode,
    revision: u64,
) -> Scene {
    let geometry = context.register_geometry(geometry).unwrap();
    let appearance = context.register_appearance(appearance).unwrap();
    let packets = placements
        .iter()
        .enumerate()
        .map(|(index, [x, y, z])| {
            DrawPacketDescriptor::new(
                geometry.handle().clone(),
                appearance.handle().clone(),
                raster_mode,
                index as u64,
            )
        })
        .collect();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(revision),
            packets,
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(revision),
                placements
                    .iter()
                    .map(|[x, y, z]| placement(*x, *y, *z))
                    .collect(),
            ),
        )
        .unwrap();
    Scene { snapshot, model }
}

fn center(pixels: &[[u8; 4]]) -> [u8; 4] {
    pixels[(EXTENT[1] as usize / 2) * EXTENT[0] as usize + EXTENT[0] as usize / 2]
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_rgba(format: wgpu::TextureFormat, pixel: [u8; 4]) -> [f32; 4] {
    let mut result = [
        pixel[0] as f32 / 255.0,
        pixel[1] as f32 / 255.0,
        pixel[2] as f32 / 255.0,
        pixel[3] as f32 / 255.0,
    ];
    if matches!(
        format,
        wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        result[0] = srgb_to_linear(result[0]);
        result[1] = srgb_to_linear(result[1]);
        result[2] = srgb_to_linear(result[2]);
    }
    result
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual}, expected {expected}, tolerance {tolerance}"
    );
}

#[test]
fn gpu_layout_fallback_uniform_offsets_and_pipeline_cache_identity_are_exact() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let required = GeometrySemantics::new(false, true, false);
    let initial_usage = context.resource_usage().unwrap();
    assert_eq!(initial_usage.resident_texture_bytes, 0);
    assert_eq!(initial_usage.resident_pipeline_variants, 0);
    let first = snapshot(
        &context,
        triangle(false, true, true),
        appearance(fallback_source(), required, AppearanceMode::Opaque),
        &[[-0.25, 0.0, -1.0], [0.25, 0.0, -1.0]],
        RasterMode::SingleSided,
        1,
    );
    let admitted_usage = context.resource_usage().unwrap();
    assert_eq!(admitted_usage.resident_texture_bytes, 0);
    assert_eq!(admitted_usage.resident_pipeline_variants, 0);
    let render_origin = WorldPoint3::try_new(-10.0, -20.0, -30.0).unwrap();
    assert!(matches!(
        context.render_headless_snapshot_for_test(
            &first.snapshot,
            &first.model,
            finite_view(),
            render_origin,
            EXTENT,
            2,
        ),
        Err(HeadlessRaster3DError::UnsupportedSampleCount { requested: 2 })
    ));
    let rejected_usage = context.resource_usage().unwrap();
    assert_eq!(rejected_usage.resident_texture_bytes, 0);
    assert_eq!(rejected_usage.resident_pipeline_variants, 0);
    let output = context
        .render_headless_snapshot_for_test(
            &first.snapshot,
            &first.model,
            finite_view(),
            render_origin,
            EXTENT,
            1,
        )
        .unwrap();
    let pixel = linear_rgba(
        output.format(),
        center(&output.readback_rgba8(&gpu).unwrap()),
    );
    for component in pixel {
        assert_close(component, 1.0, 0.015);
    }

    let contract = output.layout_contract();
    assert_eq!(contract.view_min_size(), 80);
    assert_eq!(contract.object_min_size(), 112);
    assert_eq!(contract.appearance_min_size(), 16);
    assert_eq!(contract.exact_bind_group_count(), 3);
    assert!(contract.all_uniforms_use_dynamic_offsets());
    assert_eq!(
        contract.dynamic_strides(),
        [
            context.abi_layout().view_dynamic_stride(),
            context.abi_layout().object_dynamic_stride(),
            context.abi_layout().appearance_dynamic_stride(),
        ]
    );
    assert_eq!(
        output.object_offsets(),
        &[0, contract.dynamic_strides()[1] as u32]
    );
    assert_eq!(
        output.appearance_offsets(),
        &[0, contract.dynamic_strides()[2] as u32]
    );
    assert_eq!(output.view_block().camera_relative(), [10.0, 20.0, 30.0]);
    assert!(
        output
            .view_block()
            .view_projection()
            .iter()
            .flatten()
            .all(|component| component.is_finite())
    );
    assert_eq!(
        output.object_blocks()[0].model_relative()[3],
        [9.75, 20.0, 29.0, 1.0]
    );
    assert_eq!(
        output.object_blocks()[0].normal_to_world(),
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]
    );
    assert_eq!(output.appearance_blocks()[0].color_factor(), [1.0; 4]);
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 1);
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.resident_texture_bytes, 4);
    assert_eq!(usage.resident_pipeline_variants, 1);

    context
        .render_headless_snapshot_for_test(
            &first.snapshot,
            &first.model,
            finite_view(),
            render_origin,
            EXTENT,
            1,
        )
        .unwrap();
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 1);

    let broader_semantics = GeometrySemantics::new(true, true, false);
    let second = snapshot(
        &context,
        triangle(false, true, true),
        appearance(fallback_source(), broader_semantics, AppearanceMode::Opaque),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        2,
    );
    context
        .render_headless_snapshot_for_test(
            &second.snapshot,
            &second.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 1);
    context
        .render_headless_snapshot_for_test(
            &second.snapshot,
            &second.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            4,
        )
        .unwrap();
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 2);

    let masked_classification = snapshot(
        &context,
        triangle(false, true, true),
        appearance(fallback_source(), required, AppearanceMode::AlphaMask),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        3,
    );
    context
        .render_headless_snapshot_for_test(
            &masked_classification.snapshot,
            &masked_classification.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 3);

    let mut one_pipeline = PolicyLimits3D::conservative();
    one_pipeline.max_pipeline_variants = 1;
    let Some((_renderer, _gpu, bounded)) = setup(one_pipeline) else {
        return;
    };
    let single = snapshot(
        &bounded,
        triangle(false, false, false),
        appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        1,
    );
    bounded
        .render_headless_snapshot_for_test(
            &single.snapshot,
            &single.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    let double = snapshot(
        &bounded,
        triangle(false, false, false),
        appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::DoubleSided,
        2,
    );
    let error = match bounded.render_headless_snapshot_for_test(
        &double.snapshot,
        &double.model,
        finite_view(),
        WorldPoint3::origin(),
        EXTENT,
        1,
    ) {
        Err(error) => error,
        Ok(_) => panic!("second pipeline variant unexpectedly bypassed its finite budget"),
    };
    assert!(matches!(
        error,
        HeadlessRaster3DError::Context(crate::three_d::Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::PipelineVariants,
            requested: 2,
            maximum: 1,
        })
    ));
    assert_eq!(bounded.resident_pipeline_count_for_test().unwrap(), 1);
}

#[test]
fn concurrent_pipeline_reservations_publish_once_and_keep_the_budget_exact() {
    let Some((_renderer, _gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let shared = snapshot(
        &context,
        triangle(false, false, false),
        appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        1,
    );
    let start = Arc::new(Barrier::new(5));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let context = context.clone();
        let snapshot = shared.clone();
        let start = start.clone();
        workers.push(std::thread::spawn(move || {
            start.wait();
            context
                .render_headless_snapshot_for_test(
                    &snapshot.snapshot,
                    &snapshot.model,
                    finite_view(),
                    WorldPoint3::origin(),
                    EXTENT,
                    1,
                )
                .map(|_| ())
        }));
    }
    start.wait();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 1);

    let mut one_pipeline = PolicyLimits3D::conservative();
    one_pipeline.max_pipeline_variants = 1;
    let Some((_bounded_renderer, _bounded_gpu, bounded)) = setup(one_pipeline) else {
        return;
    };
    let single = snapshot(
        &bounded,
        triangle(false, false, false),
        appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        2,
    );
    let double = snapshot(
        &bounded,
        triangle(false, false, false),
        appearance(
            constant_source([0.0, 0.0, 1.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::DoubleSided,
        3,
    );
    let start = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for snapshot in [single, double] {
        let context = bounded.clone();
        let start = start.clone();
        workers.push(std::thread::spawn(move || {
            start.wait();
            context
                .render_headless_snapshot_for_test(
                    &snapshot.snapshot,
                    &snapshot.model,
                    finite_view(),
                    WorldPoint3::origin(),
                    EXTENT,
                    1,
                )
                .map(|_| ())
        }));
    }
    start.wait();
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome,
                    Err(HeadlessRaster3DError::Context(
                        crate::three_d::Context3DError::PolicyBudgetExceeded {
                            budget: PolicyBudget3D::PipelineVariants,
                            requested: 2,
                            maximum: 1,
                        }
                    ))
                )
            })
            .count(),
        1
    );
    assert_eq!(bounded.resident_pipeline_count_for_test().unwrap(), 1);
}

#[test]
fn actual_gpu_winding_oracle_accepts_ccw_and_culls_cw() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let source = constant_source([1.0, 0.0, 0.0]);
    let ccw = snapshot(
        &context,
        triangle(false, false, false),
        appearance(
            source.clone(),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        1,
    );
    let cw = snapshot(
        &context,
        triangle(true, false, false),
        appearance(
            source.clone(),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        2,
    );
    let cw_double_sided = snapshot(
        &context,
        triangle(true, false, false),
        appearance(source, GeometrySemantics::NONE, AppearanceMode::Opaque),
        &[[0.0, 0.0, -1.0]],
        RasterMode::DoubleSided,
        3,
    );

    let render = |scene: &Scene| {
        context
            .render_headless_snapshot_for_test(
                &scene.snapshot,
                &scene.model,
                finite_view(),
                WorldPoint3::origin(),
                EXTENT,
                1,
            )
            .unwrap()
    };
    let ccw_output = render(&ccw);
    let cw_output = render(&cw);
    let double_output = render(&cw_double_sided);
    let ccw_pixel = linear_rgba(
        ccw_output.format(),
        center(&ccw_output.readback_rgba8(&gpu).unwrap()),
    );
    let cw_pixel = linear_rgba(
        cw_output.format(),
        center(&cw_output.readback_rgba8(&gpu).unwrap()),
    );
    let double_pixel = linear_rgba(
        double_output.format(),
        center(&double_output.readback_rgba8(&gpu).unwrap()),
    );
    assert!(ccw_pixel[0] > 0.98 && ccw_pixel[3] > 0.98);
    assert!(cw_pixel.iter().all(|component| *component < 0.01));
    assert!(double_pixel[0] > 0.98 && double_pixel[3] > 0.98);
}

#[test]
fn headless_raster_materializes_the_snapshot_exact_generations_after_replacement() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context
        .register_geometry(triangle(false, false, false))
        .unwrap();
    let appearance_lease = context
        .register_appearance(appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let geometry_handle = geometry.handle().clone();
    let appearance_handle = appearance_lease.handle().clone();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![DrawPacketDescriptor::new(
                geometry_handle.clone(),
                appearance_handle.clone(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(ModelRevision::new(1), vec![placement(0.0, 0.0, -1.0)]),
        )
        .unwrap();
    drop(geometry);
    drop(appearance_lease);

    let replacement_geometry = context
        .replace_geometry(&geometry_handle, triangle(true, false, false))
        .unwrap();
    let replacement_appearance = context
        .replace_appearance(
            &appearance_handle,
            appearance(
                constant_source([0.0, 0.0, 1.0]),
                GeometrySemantics::NONE,
                AppearanceMode::Opaque,
            ),
        )
        .unwrap();
    context
        .retire_geometry(replacement_geometry.handle())
        .unwrap();
    context
        .retire_appearance(replacement_appearance.handle())
        .unwrap();
    drop(replacement_geometry);
    drop(replacement_appearance);

    let output = context
        .render_headless_snapshot_for_test(
            &snapshot,
            &model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    let pixel = linear_rgba(
        output.format(),
        center(&output.readback_rgba8(&gpu).unwrap()),
    );
    assert!(pixel[0] > 0.98 && pixel[2] < 0.02 && pixel[3] > 0.98);
    assert_eq!(context.gpu_resident_geometry_bytes_for_test(), 108);
}

#[test]
fn textured_snapshots_bind_exact_texture_and_sampler_generations_after_replacement() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context.register_geometry(full_quad(true, false)).unwrap();
    let sampler = context.register_sampler(Sampler3D::LINEAR_CLAMP).unwrap();
    let first_texture = context
        .register_texture(
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![255, 0, 0, 255]).unwrap(),
        )
        .unwrap();
    let first_appearance = context
        .register_appearance(
            appearance(
                fallback_source(),
                GeometrySemantics::new(false, true, false),
                AppearanceMode::Opaque,
            )
            .with_texture(first_texture.handle().clone(), sampler.handle().clone()),
        )
        .unwrap();
    let texture_handle = first_texture.handle().clone();
    let sampler_handle = sampler.handle().clone();
    let appearance_handle = first_appearance.handle().clone();
    let first_snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![DrawPacketDescriptor::new(
                geometry.handle().clone(),
                appearance_handle.clone(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();
    let first_model = context
        .admit_model_state(
            &first_snapshot,
            ModelStateDescriptor::new(ModelRevision::new(1), vec![placement(0.0, 0.0, -1.0)]),
        )
        .unwrap();
    drop(first_texture);
    drop(first_appearance);
    drop(sampler);

    let second_texture = context
        .replace_texture(
            &texture_handle,
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![0, 0, 255, 255]).unwrap(),
        )
        .unwrap();
    let second_sampler = context
        .replace_sampler(&sampler_handle, Sampler3D::default())
        .unwrap();
    let second_appearance = context
        .replace_appearance(
            &appearance_handle,
            appearance(
                fallback_source(),
                GeometrySemantics::new(false, true, false),
                AppearanceMode::Opaque,
            )
            .with_texture(
                second_texture.handle().clone(),
                second_sampler.handle().clone(),
            ),
        )
        .unwrap();
    let second_snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(2),
            vec![DrawPacketDescriptor::new(
                geometry.handle().clone(),
                second_appearance.handle().clone(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();
    let second_model = context
        .admit_model_state(
            &second_snapshot,
            ModelStateDescriptor::new(ModelRevision::new(2), vec![placement(0.0, 0.0, -1.0)]),
        )
        .unwrap();
    context.retire_texture(second_texture.handle()).unwrap();
    context.retire_sampler(second_sampler.handle()).unwrap();
    context
        .retire_appearance(second_appearance.handle())
        .unwrap();
    drop(second_texture);
    drop(second_sampler);
    drop(second_appearance);

    let render_center = |snapshot: &RenderSnapshot, model: &ModelState| {
        let output = context
            .render_headless_snapshot_for_test(
                snapshot,
                model,
                finite_view(),
                WorldPoint3::origin(),
                EXTENT,
                1,
            )
            .unwrap();
        linear_rgba(
            output.format(),
            center(&output.readback_rgba8(&gpu).unwrap()),
        )
    };
    let first_pixel = render_center(&first_snapshot, &first_model);
    let second_pixel = render_center(&second_snapshot, &second_model);
    assert!(
        first_pixel[0] > 0.98 && first_pixel[2] < 0.02,
        "old snapshot did not retain its red texture generation: {first_pixel:?}"
    );
    assert!(
        second_pixel[2] > 0.98 && second_pixel[0] < 0.02,
        "new snapshot did not bind its blue texture generation: {second_pixel:?}"
    );
    assert_eq!(context.resource_usage().unwrap().resident_texture_bytes, 12);
}

#[test]
fn actual_gpu_reverse_z_near_over_far_holds_for_all_baseline_projections() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context
        .register_geometry(triangle(false, false, false))
        .unwrap();
    let near = context
        .register_appearance(appearance(
            constant_source([1.0, 0.0, 0.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let far = context
        .register_appearance(appearance(
            constant_source([0.0, 0.0, 1.0]),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let packets = vec![
        DrawPacketDescriptor::new(
            geometry.handle().clone(),
            near.handle().clone(),
            RasterMode::SingleSided,
            0,
        ),
        DrawPacketDescriptor::new(
            geometry.handle().clone(),
            far.handle().clone(),
            RasterMode::SingleSided,
            1,
        ),
    ];
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            packets,
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(1),
                vec![placement(0.0, 0.0, -1.0), placement(0.0, 0.0, -3.0)],
            ),
        )
        .unwrap();
    let views = [
        finite_view(),
        view(PerspectiveProjection::try_infinite(std::f32::consts::FRAC_PI_2, 0.1).unwrap()),
        view(OrthographicProjection::try_new(2.0, 0.0, 10.0).unwrap()),
    ];
    for view in views {
        let output = context
            .render_headless_snapshot_for_test(
                &snapshot,
                &model,
                view,
                WorldPoint3::origin(),
                EXTENT,
                1,
            )
            .unwrap();
        let pixel = linear_rgba(
            output.format(),
            center(&output.readback_rgba8(&gpu).unwrap()),
        );
        assert!(pixel[0] > 0.98, "near red did not win: {pixel:?}");
        assert!(pixel[2] < 0.02, "far blue leaked through: {pixel:?}");
        assert!(pixel[3] > 0.98);
    }
    assert_eq!(context.resident_pipeline_count_for_test().unwrap(), 2);
}

#[test]
fn actual_gpu_sample1_and_sample4_resolve_premultiplied_edges_and_mask_discard() {
    let Some((_renderer, gpu, context)) = setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let color = [0.25, 0.5, 0.75];
    let edge = snapshot(
        &context,
        triangle(false, false, false),
        appearance(
            constant_source(color),
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        1,
    );
    let sample1 = context
        .render_headless_snapshot_for_test(
            &edge.snapshot,
            &edge.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    let sample4 = context
        .render_headless_snapshot_for_test(
            &edge.snapshot,
            &edge.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            4,
        )
        .unwrap();
    assert_eq!(sample1.extent(), EXTENT);
    assert_eq!(sample1.sample_count(), 1);
    assert_eq!(sample4.sample_count(), 4);
    let sample1_pixels = sample1.readback_rgba8(&gpu).unwrap();
    let sample4_pixels = sample4.readback_rgba8(&gpu).unwrap();
    let adapter = gpu.adapter.get_info();
    println!(
        "baseline 3D readback on {:?} {}: sample1={} sample4={} pixels",
        adapter.backend, adapter.name, sample1_pixels.len(), sample4_pixels.len(),
    );
    let sample1_center = linear_rgba(sample1.format(), center(&sample1_pixels));
    let sample4_center = linear_rgba(sample4.format(), center(&sample4_pixels));
    for channel in 0..3 {
        assert_close(sample1_center[channel], color[channel], 0.02);
        assert_close(sample4_center[channel], color[channel], 0.02);
    }
    assert_close(sample1_center[3], 1.0, 0.005);
    assert_close(sample4_center[3], 1.0, 0.005);
    let clear = linear_rgba(sample4.format(), sample4_pixels[0]);
    assert!(clear.iter().all(|component| *component < 0.005));

    let mut partial_edges = 0;
    for pixel in sample4_pixels {
        let linear = linear_rgba(sample4.format(), pixel);
        if linear[3] > 0.05 && linear[3] < 0.95 {
            partial_edges += 1;
            for channel in 0..3 {
                assert_close(linear[channel], color[channel] * linear[3], 0.035);
            }
        }
    }
    assert!(
        partial_edges > 0,
        "4x MSAA produced no partial geometry-edge pixels"
    );

    let mask = snapshot(
        &context,
        full_quad(true, false),
        appearance(
            mask_source(color),
            GeometrySemantics::new(false, true, false),
            AppearanceMode::AlphaMask,
        ),
        &[[0.0, 0.0, -1.0]],
        RasterMode::SingleSided,
        2,
    );
    let masked_sample1 = context
        .render_headless_snapshot_for_test(
            &mask.snapshot,
            &mask.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            1,
        )
        .unwrap();
    let masked_sample4 = context
        .render_headless_snapshot_for_test(
            &mask.snapshot,
            &mask.model,
            finite_view(),
            WorldPoint3::origin(),
            EXTENT,
            4,
        )
        .unwrap();
    let masked1 = masked_sample1.readback_rgba8(&gpu).unwrap();
    let masked4 = masked_sample4.readback_rgba8(&gpu).unwrap();
    let central = |pixels: &[[u8; 4]], x: usize, y: usize| {
        linear_rgba(masked_sample4.format(), pixels[y * EXTENT[0] as usize + x])
    };
    let central1 = &masked1;
    let mut saw_discarded = false;
    let mut saw_survivor = false;
    for y in 12..52 {
        for x in 12..52 {
            let pixel = central(central1, x, y);
            saw_discarded |= pixel[3] < 0.01;
            saw_survivor |= pixel[3] > 0.99;
        }
    }
    assert!(saw_discarded && saw_survivor);
    for pixel in masked4 {
        let linear = linear_rgba(masked_sample4.format(), pixel);
        if linear[3] > 0.01 {
            for channel in 0..3 {
                assert_close(linear[channel], color[channel] * linear[3], 0.04);
            }
        } else {
            assert!(linear[0..3].iter().all(|component| *component < 0.01));
        }
    }
}
