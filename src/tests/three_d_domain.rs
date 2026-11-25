use std::sync::Arc;

use crate::compositor::SurfaceId;
use crate::core::math::RectF;
use crate::renderer::wgpu_renderer::{GpuResources, WgpuRenderer};
use crate::renderer::{
    CustomRenderActivity, CustomRenderParticipant, CustomRenderTargetMetrics, PhysicalExtent,
    PhysicalRect,
};
use crate::three_d::viewport_audit;
use crate::three_d::{
    AppearanceMode, AppearanceSourceDescriptor, Context3D, Context3DError, DomainAssociation3Error,
    DomainComposition3, DomainComposition3Descriptor, DomainComposition3Error,
    DomainContent3Descriptor, DomainContentId3, DomainContentPlacement3, DomainEmbedding3,
    DomainFrame3, DomainFrame3Descriptor, DomainFrame3Error, DomainPoint2, DomainPoint3,
    DomainRetentionBudget3D, DomainRetentionError3D, DomainRetentionLimits3D, DomainState3,
    DomainState3Descriptor, DomainState3Error, DomainStateRevision3, DomainVector3,
    DrawPacketDescriptor, GeometryIndices, GeometrySemantics, GeometrySourceDescriptor,
    LocalDomain3Descriptor, LocalDomainId3, LocalDomainKind3, LocalPoint3, ModelPlacement3,
    ModelRevision, ModelStateDescriptor, Orientation3, PerspectiveProjection, PlanarRayAccess3,
    PlanarRaySide3, PolicyBudget3D, PolicyLimits3D, PositiveScale3, RasterMode, Ray3,
    RenderSnapshotDescriptor, RigidPose3, SampleCount3D, StructuralRevision, ViewDescriptor3,
    Viewport3D, Viewport3DError, Viewport3DLifecycleState, Viewport3DViewRevision, WorldPoint3,
    WorldVector3,
};

const PLANAR_DOMAIN: LocalDomainId3 = LocalDomainId3::new(10);
const SPATIAL_DOMAIN: LocalDomainId3 = LocalDomainId3::new(20);
const PLANAR_CONTENT: DomainContentId3 = DomainContentId3::new(100);
const SPATIAL_CONTENT: DomainContentId3 = DomainContentId3::new(200);

fn point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

fn triangle() -> GeometrySourceDescriptor {
    GeometrySourceDescriptor::try_new(
        vec![
            point(-0.78, -0.74, 0.0),
            point(0.82, -0.66, 0.0),
            point(-0.07, 0.79, 0.0),
        ],
        None,
        None,
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

fn appearance(color: [f32; 3]) -> AppearanceSourceDescriptor {
    AppearanceSourceDescriptor::try_new_with_mode(
        constant_source(color),
        GeometrySemantics::NONE,
        AppearanceMode::Opaque,
    )
    .unwrap()
}

fn two_domain_composition(
    context: &Context3D,
    revision: u64,
    two_planar_packets: bool,
) -> DomainComposition3 {
    try_two_domain_composition(context, revision, two_planar_packets).unwrap()
}

fn try_two_domain_composition(
    context: &Context3D,
    revision: u64,
    two_planar_packets: bool,
) -> Result<DomainComposition3, DomainComposition3Error> {
    let geometry = context.register_geometry(triangle()).unwrap();
    let red = context
        .register_appearance(appearance([1.0, 0.0, 0.0]))
        .unwrap();
    let green = context
        .register_appearance(appearance([0.0, 1.0, 0.0]))
        .unwrap();
    let blue = context
        .register_appearance(appearance([0.0, 0.0, 1.0]))
        .unwrap();
    let mut planar_packets = vec![DrawPacketDescriptor::new(
        geometry.handle().clone(),
        red.handle().clone(),
        RasterMode::SingleSided,
        9,
    )];
    if two_planar_packets {
        planar_packets.push(DrawPacketDescriptor::new(
            geometry.handle().clone(),
            green.handle().clone(),
            RasterMode::SingleSided,
            2,
        ));
    }
    context.admit_domain_composition(DomainComposition3Descriptor::new(
        StructuralRevision::new(revision),
        vec![
            LocalDomain3Descriptor::new(
                PLANAR_DOMAIN,
                LocalDomainKind3::Planar2,
                vec![DomainContent3Descriptor::new(
                    PLANAR_CONTENT,
                    planar_packets,
                )],
            ),
            LocalDomain3Descriptor::new(
                SPATIAL_DOMAIN,
                LocalDomainKind3::Spatial3,
                vec![DomainContent3Descriptor::new(
                    SPATIAL_CONTENT,
                    vec![DrawPacketDescriptor::new(
                        geometry.handle().clone(),
                        blue.handle().clone(),
                        RasterMode::SingleSided,
                        7,
                    )],
                )],
            ),
        ],
    ))
}

fn state(
    context: &Context3D,
    composition: &DomainComposition3,
    domain_id: LocalDomainId3,
    revision: u64,
    embedding: DomainEmbedding3,
    placement: DomainContentPlacement3,
) -> DomainState3 {
    let content_id = match domain_id {
        PLANAR_DOMAIN => PLANAR_CONTENT,
        SPATIAL_DOMAIN => SPATIAL_CONTENT,
        _ => panic!("test helper received unknown domain"),
    };
    context
        .admit_domain_state(
            composition,
            DomainState3Descriptor::new(
                domain_id,
                DomainStateRevision3::new(revision),
                embedding,
                vec![(content_id, placement)],
            ),
        )
        .unwrap()
}

fn frame(
    context: &Context3D,
    composition: &DomainComposition3,
    revision: u64,
    planar: DomainState3,
    spatial: DomainState3,
) -> DomainFrame3 {
    context
        .admit_domain_frame(
            composition,
            DomainFrame3Descriptor::new(ModelRevision::new(revision), vec![spatial, planar]),
        )
        .unwrap()
}

fn embedding(x: f64, y: f64, z: f64, scale: f32) -> DomainEmbedding3 {
    DomainEmbedding3::try_new(
        WorldPoint3::try_new(x, y, z).unwrap(),
        Orientation3::identity(),
        scale,
    )
    .unwrap()
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

fn active_metrics(surface: u64, width: u32, height: u32) -> CustomRenderTargetMetrics {
    let logical = RectF::new(0.0, 0.0, width as f32, height as f32);
    CustomRenderTargetMetrics {
        surface_id: SurfaceId::test_id(surface),
        app_frame_attempt: None,
        final_logical_rect: logical,
        final_logical_viewport: logical,
        target_logical_extent: logical.size(),
        scale_factor: 1.0,
        target_physical_extent: PhysicalExtent::new(width, height),
        activity: CustomRenderActivity::Active {
            physical_render_extent: PhysicalExtent::new(width, height),
            physical_composition_coverage: PhysicalRect::new(0, 0, width, height),
        },
    }
}

fn inactive_metrics(surface: u64, width: u32, height: u32) -> CustomRenderTargetMetrics {
    let mut metrics = active_metrics(surface, width, height);
    metrics.activity = CustomRenderActivity::Inactive;
    metrics
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-6,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn domain_admission_is_explicit_deterministic_complete_and_transactional() {
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let composition = two_domain_composition(&context, 3, true);
    assert_eq!(
        composition.structural_revision(),
        StructuralRevision::new(3)
    );
    assert_eq!(composition.domain_count(), 2);
    assert_eq!(composition.content_count(), 2);
    assert_eq!(
        composition.domains().collect::<Vec<_>>(),
        vec![
            (PLANAR_DOMAIN, LocalDomainKind3::Planar2),
            (SPATIAL_DOMAIN, LocalDomainKind3::Spatial3),
        ]
    );
    assert_eq!(
        composition
            .render_snapshot()
            .draw_packets()
            .iter()
            .map(DrawPacketDescriptor::stable_ordinal)
            .collect::<Vec<_>>(),
        vec![9, 2, 7]
    );

    let planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        embedding(1.0, 0.0, -1.0, 2.0),
        DomainContentPlacement3::identity(),
    );
    let spatial = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        8,
        embedding(-2.0, 0.0, -3.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let admitted_frame = frame(&context, &composition, 11, planar.clone(), spatial.clone());
    assert_eq!(admitted_frame.states()[0].domain_id(), PLANAR_DOMAIN);
    assert_eq!(admitted_frame.states()[1].domain_id(), SPATIAL_DOMAIN);
    assert_eq!(admitted_frame.model().placements().len(), 3);
    assert_eq!(
        admitted_frame.model().placements()[0],
        admitted_frame.model().placements()[1]
    );
    assert_eq!(
        admitted_frame.model().placements()[0].translation(),
        WorldPoint3::try_new(1.0, 0.0, -1.0).unwrap()
    );
    assert_eq!(
        admitted_frame.model().placements()[2].translation(),
        WorldPoint3::try_new(-2.0, 0.0, -3.0).unwrap()
    );

    assert_eq!(
        context
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(4),
                Vec::new(),
            ))
            .unwrap_err(),
        DomainComposition3Error::EmptyComposition
    );
    let empty_domain =
        LocalDomain3Descriptor::new(PLANAR_DOMAIN, LocalDomainKind3::Planar2, Vec::new());
    assert_eq!(
        context
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(4),
                vec![empty_domain],
            ))
            .unwrap_err(),
        DomainComposition3Error::EmptyDomain {
            domain_id: PLANAR_DOMAIN,
        }
    );
    let packet = composition.render_snapshot().draw_packets()[0].clone();
    let duplicate_domain = LocalDomain3Descriptor::new(
        PLANAR_DOMAIN,
        LocalDomainKind3::Planar2,
        vec![DomainContent3Descriptor::new(
            PLANAR_CONTENT,
            vec![packet.clone()],
        )],
    );
    assert_eq!(
        context
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(4),
                vec![duplicate_domain.clone(), duplicate_domain],
            ))
            .unwrap_err(),
        DomainComposition3Error::DuplicateDomainId {
            domain_id: PLANAR_DOMAIN,
        }
    );
    assert_eq!(
        context
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(4),
                vec![LocalDomain3Descriptor::new(
                    PLANAR_DOMAIN,
                    LocalDomainKind3::Planar2,
                    vec![
                        DomainContent3Descriptor::new(PLANAR_CONTENT, vec![packet.clone()],),
                        DomainContent3Descriptor::new(PLANAR_CONTENT, vec![packet.clone()],),
                    ],
                )],
            ))
            .unwrap_err(),
        DomainComposition3Error::DuplicateContentId {
            domain_id: PLANAR_DOMAIN,
            content_id: PLANAR_CONTENT,
        }
    );
    assert_eq!(
        context
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(4),
                vec![LocalDomain3Descriptor::new(
                    PLANAR_DOMAIN,
                    LocalDomainKind3::Planar2,
                    vec![DomainContent3Descriptor::new(PLANAR_CONTENT, Vec::new())],
                )],
            ))
            .unwrap_err(),
        DomainComposition3Error::EmptyContent {
            domain_id: PLANAR_DOMAIN,
            content_id: PLANAR_CONTENT,
        }
    );

    let missing = context
        .admit_domain_state(
            &composition,
            DomainState3Descriptor::new(
                PLANAR_DOMAIN,
                DomainStateRevision3::new(2),
                DomainEmbedding3::identity(),
                Vec::new(),
            ),
        )
        .unwrap_err();
    assert_eq!(
        missing,
        DomainState3Error::MissingContentPlacement {
            domain_id: PLANAR_DOMAIN,
            content_id: PLANAR_CONTENT,
        }
    );
    let duplicate = context
        .admit_domain_state(
            &composition,
            DomainState3Descriptor::new(
                PLANAR_DOMAIN,
                DomainStateRevision3::new(2),
                DomainEmbedding3::identity(),
                vec![
                    (PLANAR_CONTENT, DomainContentPlacement3::identity()),
                    (PLANAR_CONTENT, DomainContentPlacement3::identity()),
                ],
            ),
        )
        .unwrap_err();
    assert_eq!(
        duplicate,
        DomainState3Error::DuplicateContentPlacement {
            domain_id: PLANAR_DOMAIN,
            content_id: PLANAR_CONTENT,
        }
    );
    assert_eq!(
        context
            .admit_domain_state(
                &composition,
                DomainState3Descriptor::new(
                    PLANAR_DOMAIN,
                    DomainStateRevision3::new(2),
                    DomainEmbedding3::identity(),
                    vec![(
                        DomainContentId3::new(999),
                        DomainContentPlacement3::identity(),
                    )],
                ),
            )
            .unwrap_err(),
        DomainState3Error::ExtraContentPlacement {
            domain_id: PLANAR_DOMAIN,
            content_id: DomainContentId3::new(999),
        }
    );
    assert_eq!(
        context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(ModelRevision::new(12), vec![planar]),
            )
            .unwrap_err(),
        DomainFrame3Error::MissingDomainState {
            domain_id: SPATIAL_DOMAIN,
        }
    );
    let admitted_planar = admitted_frame.state(PLANAR_DOMAIN).unwrap().clone();
    assert_eq!(
        context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(
                    ModelRevision::new(12),
                    vec![admitted_planar.clone(), admitted_planar.clone()],
                ),
            )
            .unwrap_err(),
        DomainFrame3Error::DuplicateDomainState {
            domain_id: PLANAR_DOMAIN,
        }
    );

    let foreign_composition = two_domain_composition(&context, 4, false);
    let foreign_planar = state(
        &context,
        &foreign_composition,
        PLANAR_DOMAIN,
        1,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    assert_eq!(
        context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(ModelRevision::new(12), vec![foreign_planar.clone()]),
            )
            .unwrap_err(),
        DomainFrame3Error::ForeignDomainState {
            domain_id: PLANAR_DOMAIN,
        }
    );
    assert_eq!(
        admitted_planar.validate_successor(&foreign_planar),
        Err(DomainState3Error::CompositionIdentityMismatch)
    );

    let extra_domain_id = LocalDomainId3::new(999);
    let extra_content_id = DomainContentId3::new(999);
    let extra_composition = context
        .admit_domain_composition(DomainComposition3Descriptor::new(
            StructuralRevision::new(5),
            vec![LocalDomain3Descriptor::new(
                extra_domain_id,
                LocalDomainKind3::Spatial3,
                vec![DomainContent3Descriptor::new(
                    extra_content_id,
                    vec![packet],
                )],
            )],
        ))
        .unwrap();
    let extra_state = context
        .admit_domain_state(
            &extra_composition,
            DomainState3Descriptor::new(
                extra_domain_id,
                DomainStateRevision3::new(1),
                DomainEmbedding3::identity(),
                vec![(extra_content_id, DomainContentPlacement3::identity())],
            ),
        )
        .unwrap();
    assert_eq!(
        context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(ModelRevision::new(12), vec![extra_state]),
            )
            .unwrap_err(),
        DomainFrame3Error::ExtraDomainState {
            domain_id: extra_domain_id,
        }
    );

    let other_context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    assert_eq!(
        other_context
            .admit_domain_state(
                &composition,
                DomainState3Descriptor::new(
                    PLANAR_DOMAIN,
                    DomainStateRevision3::new(3),
                    DomainEmbedding3::identity(),
                    vec![(PLANAR_CONTENT, DomainContentPlacement3::identity())],
                ),
            )
            .unwrap_err(),
        DomainState3Error::Context(Context3DError::ContextMismatch)
    );

    let mut bounded_policy = PolicyLimits3D::conservative();
    bounded_policy.max_draw_packets_per_snapshot = 1;
    let bounded = Context3D::new_cpu_only_for_test(bounded_policy, u64::MAX).unwrap();
    let bounded_geometry = bounded.register_geometry(triangle()).unwrap();
    let bounded_appearance = bounded
        .register_appearance(appearance([1.0, 1.0, 1.0]))
        .unwrap();
    let bounded_packet = DrawPacketDescriptor::new(
        bounded_geometry.handle().clone(),
        bounded_appearance.handle().clone(),
        RasterMode::SingleSided,
        0,
    );
    let usage_before = bounded.domain_retention_usage().unwrap();
    assert_eq!(
        bounded
            .admit_domain_composition(DomainComposition3Descriptor::new(
                StructuralRevision::new(1),
                vec![LocalDomain3Descriptor::new(
                    PLANAR_DOMAIN,
                    LocalDomainKind3::Planar2,
                    vec![DomainContent3Descriptor::new(
                        PLANAR_CONTENT,
                        vec![bounded_packet.clone(), bounded_packet],
                    )],
                )],
            ))
            .unwrap_err(),
        DomainComposition3Error::Snapshot(crate::three_d::RenderSnapshot3DError::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::DrawPacketsPerSnapshot,
                requested: 2,
                maximum: 1,
            },
        ),)
    );
    assert_eq!(bounded.domain_retention_usage().unwrap(), usage_before);
}

#[test]
fn domain_similarity_math_preserves_roles_winding_and_planar_side_policy() {
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let composition = two_domain_composition(&context, 1, false);
    let quarter_turn_z = Orientation3::try_from_xyzw(
        0.0,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    )
    .unwrap();
    let domain_embedding = DomainEmbedding3::try_new(
        WorldPoint3::try_new(10.0, -2.0, 5.0).unwrap(),
        quarter_turn_z,
        2.0,
    )
    .unwrap();
    let quarter_turn_x = Orientation3::try_from_xyzw(
        std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
    )
    .unwrap();
    let content_placement = DomainContentPlacement3::new(
        DomainPoint3::try_new(1.0, 2.0, 3.0).unwrap(),
        quarter_turn_x,
        PositiveScale3::try_new(2.0, 3.0, 4.0).unwrap(),
    );
    let planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        domain_embedding,
        content_placement,
    );
    let spatial = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        1,
        embedding(0.0, 0.0, -2.0, 4.0),
        DomainContentPlacement3::identity(),
    );
    let admitted_frame = frame(&context, &composition, 1, planar.clone(), spatial.clone());

    let world = planar
        .planar_point_to_world(DomainPoint2::try_new(1.0, 2.0).unwrap())
        .unwrap();
    assert_close(world.x(), 6.0);
    assert_close(world.y(), 0.0);
    assert_close(world.z(), 5.0);
    let round_trip = planar.world_point_to_domain(world).unwrap();
    assert_close(round_trip.x(), 1.0);
    assert_close(round_trip.y(), 2.0);
    assert_close(round_trip.z(), 0.0);

    let off_plane = planar
        .world_point_to_domain(WorldPoint3::try_new(6.0, 0.0, 11.0).unwrap())
        .unwrap();
    assert_close(off_plane.x(), 1.0);
    assert_close(off_plane.y(), 2.0);
    assert_close(off_plane.z(), 3.0);
    let world_vector = planar
        .domain_vector_to_world(DomainVector3::try_new(1.0, 2.0, 3.0).unwrap())
        .unwrap();
    assert_close(world_vector.x(), -4.0);
    assert_close(world_vector.y(), 2.0);
    assert_close(world_vector.z(), 6.0);
    let local_vector = planar.world_vector_to_domain(world_vector).unwrap();
    assert_close(local_vector.x(), 1.0);
    assert_close(local_vector.y(), 2.0);
    assert_close(local_vector.z(), 3.0);

    let lowered = admitted_frame.model().placements()[0];
    assert_eq!(
        lowered.orientation(),
        quarter_turn_z.compose_local(quarter_turn_x).unwrap()
    );
    assert_eq!(
        lowered.scale(),
        PositiveScale3::try_new(4.0, 6.0, 8.0).unwrap()
    );
    assert!(lowered.determinant() > 0.0);
    assert_close(lowered.translation().x(), 6.0);
    assert_close(lowered.translation().y(), 0.0);
    assert_close(lowered.translation().z(), 11.0);

    let ray = Ray3::try_new(
        WorldPoint3::try_new(6.0, 0.0, 11.0).unwrap(),
        WorldVector3::try_new(0.0, 0.0, -1.0).unwrap(),
    )
    .unwrap();
    let domain_ray = planar.world_ray_to_domain(ray).unwrap();
    assert_close(domain_ray.origin().x(), 1.0);
    assert_close(domain_ray.origin().y(), 2.0);
    assert_close(domain_ray.origin().z(), 3.0);
    assert_close(domain_ray.world_units_per_local_parameter(), 2.0);
    let hit = domain_ray
        .intersect_planar(PlanarRayAccess3::FrontOnly)
        .unwrap()
        .unwrap();
    assert_eq!(hit.approached_side(), PlanarRaySide3::Front);
    assert_close(hit.point().u(), 1.0);
    assert_close(hit.point().v(), 2.0);
    assert_close(hit.domain_parameter(), 3.0);
    assert_close(hit.world_distance(), 6.0);
    assert_eq!(
        domain_ray
            .intersect_planar(PlanarRayAccess3::BackOnly)
            .unwrap(),
        None
    );
    let back_ray = Ray3::try_new(
        WorldPoint3::try_new(6.0, 0.0, -1.0).unwrap(),
        WorldVector3::try_new(0.0, 0.0, 1.0).unwrap(),
    )
    .unwrap();
    let back_domain_ray = planar.world_ray_to_domain(back_ray).unwrap();
    let back_hit = back_domain_ray
        .intersect_planar(PlanarRayAccess3::BackOnly)
        .unwrap()
        .unwrap();
    assert_eq!(back_hit.approached_side(), PlanarRaySide3::Back);
    assert_close(back_hit.point().u(), 1.0);
    assert_close(back_hit.point().v(), 2.0);
    assert_close(back_hit.domain_parameter(), 3.0);
    assert_close(back_hit.world_distance(), 6.0);
    assert_eq!(
        back_domain_ray
            .intersect_planar(PlanarRayAccess3::FrontOnly)
            .unwrap(),
        None
    );
    assert_eq!(
        spatial
            .world_ray_to_domain(ray)
            .unwrap()
            .intersect_planar(PlanarRayAccess3::TwoSided),
        Err(DomainAssociation3Error::PlanarDomainRequired {
            domain_id: SPATIAL_DOMAIN,
            actual: LocalDomainKind3::Spatial3,
        })
    );

    let spatial_local = DomainPoint3::try_new(1.0, -2.0, 3.0).unwrap();
    let spatial_world = spatial.spatial_point_to_world(spatial_local).unwrap();
    assert_close(spatial_world.x(), 4.0);
    assert_close(spatial_world.y(), -8.0);
    assert_close(spatial_world.z(), 10.0);
    let spatial_round_trip = spatial.world_point_to_domain(spatial_world).unwrap();
    assert_close(spatial_round_trip.x(), spatial_local.x());
    assert_close(spatial_round_trip.y(), spatial_local.y());
    assert_close(spatial_round_trip.z(), spatial_local.z());
    let spatial_world_vector = spatial
        .domain_vector_to_world(DomainVector3::try_new(1.0, -2.0, 3.0).unwrap())
        .unwrap();
    assert_close(spatial_world_vector.x(), 4.0);
    assert_close(spatial_world_vector.y(), -8.0);
    assert_close(spatial_world_vector.z(), 12.0);
    let spatial_vector_round_trip = spatial
        .world_vector_to_domain(spatial_world_vector)
        .unwrap();
    assert_close(spatial_vector_round_trip.x(), 1.0);
    assert_close(spatial_vector_round_trip.y(), -2.0);
    assert_close(spatial_vector_round_trip.z(), 3.0);
    let spatial_domain_ray = spatial
        .world_ray_to_domain(
            Ray3::try_new(
                WorldPoint3::try_new(0.0, 0.0, 10.0).unwrap(),
                WorldVector3::try_new(0.0, 0.0, -1.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(spatial_domain_ray.kind(), LocalDomainKind3::Spatial3);
    assert_close(spatial_domain_ray.origin().z(), 3.0);
    assert_close(spatial_domain_ray.direction().z(), -1.0);
    assert_close(spatial_domain_ray.world_units_per_local_parameter(), 4.0);
    assert_close(spatial_domain_ray.point_at(2.0).unwrap().z(), 1.0);

    assert!(
        DomainEmbedding3::try_new(WorldPoint3::origin(), Orientation3::identity(), 0.0,).is_err()
    );
    assert!(
        DomainEmbedding3::try_new(WorldPoint3::origin(), Orientation3::identity(), -1.0,).is_err()
    );
    assert!(
        DomainEmbedding3::try_new(
            WorldPoint3::origin(),
            Orientation3::identity(),
            f32::INFINITY,
        )
        .is_err()
    );
    assert!(DomainPoint3::try_new(f64::NAN, 0.0, 0.0).is_err());
    assert!(DomainVector3::try_new(0.0, f64::INFINITY, 0.0).is_err());
    let overflow_state = context.admit_domain_state(
        &composition,
        DomainState3Descriptor::new(
            PLANAR_DOMAIN,
            DomainStateRevision3::new(2),
            DomainEmbedding3::try_new(WorldPoint3::origin(), Orientation3::identity(), 1.0e10)
                .unwrap(),
            vec![(
                PLANAR_CONTENT,
                DomainContentPlacement3::new(
                    DomainPoint3::origin(),
                    Orientation3::identity(),
                    PositiveScale3::try_new(1.0e30, 1.0, 1.0).unwrap(),
                ),
            )],
        ),
    );
    assert_eq!(
        overflow_state.unwrap_err(),
        DomainState3Error::Math(crate::three_d::Math3DError::NonRepresentableModelTransform)
    );
    assert_eq!(
        planar.spatial_point_to_world(DomainPoint3::origin()),
        Err(DomainState3Error::WrongDomainKind {
            domain_id: PLANAR_DOMAIN,
            required: LocalDomainKind3::Spatial3,
            actual: LocalDomainKind3::Planar2,
        })
    );
}

#[test]
fn domain_state_and_frame_revisions_preserve_independent_identity() {
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let composition = two_domain_composition(&context, 1, false);
    let planar_1 = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    let spatial_4 = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        4,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    let current = frame(
        &context,
        &composition,
        1,
        planar_1.clone(),
        spatial_4.clone(),
    );
    assert_eq!(current.validate_successor(&current.clone()), Ok(()));

    let reused_planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        embedding(1.0, 0.0, 0.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    assert_eq!(
        planar_1.validate_successor(&reused_planar),
        Err(DomainState3Error::ReusedRevision {
            revision: DomainStateRevision3::new(1),
        })
    );
    let stale_spatial = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        3,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    assert_eq!(
        spatial_4.validate_successor(&stale_spatial),
        Err(DomainState3Error::StaleRevision {
            current: DomainStateRevision3::new(4),
            requested: DomainStateRevision3::new(3),
        })
    );

    let planar_2 = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        2,
        embedding(2.0, 0.0, 0.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let successor = frame(
        &context,
        &composition,
        2,
        planar_2.clone(),
        spatial_4.clone(),
    );
    assert_eq!(current.validate_successor(&successor), Ok(()));
    assert!(
        successor
            .state(SPATIAL_DOMAIN)
            .unwrap()
            .same_identity(&spatial_4)
    );
    assert!(
        !successor
            .state(PLANAR_DOMAIN)
            .unwrap()
            .same_identity(&planar_1)
    );
    assert_eq!(
        successor.model().placements()[1].translation(),
        current.model().placements()[1].translation()
    );
    assert_eq!(
        DomainStateRevision3::new(u64::MAX).checked_next(),
        Err(DomainState3Error::RevisionExhausted {
            revision: DomainStateRevision3::new(u64::MAX),
        })
    );

    let failed_candidate = context.admit_domain_state(
        &composition,
        DomainState3Descriptor::new(
            PLANAR_DOMAIN,
            DomainStateRevision3::new(3),
            DomainEmbedding3::identity(),
            Vec::new(),
        ),
    );
    assert!(failed_candidate.is_err());
    assert!(
        successor
            .state(PLANAR_DOMAIN)
            .unwrap()
            .same_identity(&planar_2)
    );
    assert!(
        successor
            .state(SPATIAL_DOMAIN)
            .unwrap()
            .same_identity(&spatial_4)
    );
}

#[test]
fn domain_retention_limits_count_unique_identities_and_release_exactly() {
    let conservative = DomainRetentionLimits3D::conservative();
    assert_eq!(
        DomainRetentionLimits3D::try_new(
            0,
            conservative.max_live_domain_states(),
            conservative.max_live_domain_frames(),
            conservative.max_retained_domain_metadata_bytes(),
        ),
        Err(DomainRetentionError3D::InvalidConfiguration {
            field: "max_live_domain_compositions",
        })
    );
    let domain_limits = DomainRetentionLimits3D::try_new(
        1,
        2,
        1,
        conservative.max_retained_domain_metadata_bytes(),
    )
    .unwrap();
    let context = Context3D::new_cpu_only_with_domain_retention_for_test(
        PolicyLimits3D::conservative(),
        domain_limits,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(context.domain_retention_limits(), domain_limits);
    let composition = two_domain_composition(&context, 1, false);
    let composition_clone = composition.clone();
    assert_eq!(
        context
            .domain_retention_usage()
            .unwrap()
            .live_domain_compositions(),
        1
    );
    assert!(composition.same_identity(&composition_clone));
    assert_eq!(
        try_two_domain_composition(&context, 2, false).unwrap_err(),
        DomainComposition3Error::Retention(DomainRetentionError3D::BudgetExceeded {
            budget: DomainRetentionBudget3D::LiveDomainCompositions,
            requested: 2,
            maximum: 1,
        })
    );

    let planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    let spatial = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        1,
        DomainEmbedding3::identity(),
        DomainContentPlacement3::identity(),
    );
    assert_eq!(
        context
            .domain_retention_usage()
            .unwrap()
            .live_domain_states(),
        2
    );
    assert_eq!(
        context
            .admit_domain_state(
                &composition,
                DomainState3Descriptor::new(
                    PLANAR_DOMAIN,
                    DomainStateRevision3::new(2),
                    DomainEmbedding3::identity(),
                    vec![(PLANAR_CONTENT, DomainContentPlacement3::identity())],
                ),
            )
            .unwrap_err(),
        DomainState3Error::Retention(DomainRetentionError3D::BudgetExceeded {
            budget: DomainRetentionBudget3D::LiveDomainStates,
            requested: 3,
            maximum: 2,
        })
    );

    let admitted_frame = frame(&context, &composition, 1, planar.clone(), spatial.clone());
    let frame_clone = admitted_frame.clone();
    assert_eq!(
        context
            .domain_retention_usage()
            .unwrap()
            .live_domain_frames(),
        1
    );
    assert_eq!(
        context
            .admit_domain_frame(
                &composition,
                DomainFrame3Descriptor::new(
                    ModelRevision::new(2),
                    vec![planar.clone(), spatial.clone()],
                ),
            )
            .unwrap_err(),
        DomainFrame3Error::Retention(DomainRetentionError3D::BudgetExceeded {
            budget: DomainRetentionBudget3D::LiveDomainFrames,
            requested: 2,
            maximum: 1,
        })
    );
    let charged = context.domain_retention_usage().unwrap();
    assert!(charged.retained_domain_metadata_bytes() > 0);
    assert_eq!(charged.live_domain_compositions_high_water(), 1);
    assert_eq!(charged.live_domain_states_high_water(), 2);
    assert_eq!(charged.live_domain_frames_high_water(), 1);

    drop(admitted_frame);
    drop(planar);
    drop(spatial);
    drop(composition);
    drop(composition_clone);
    assert_eq!(context.domain_retention_usage().unwrap(), charged);
    drop(frame_clone);
    let released = context.domain_retention_usage().unwrap();
    assert_eq!(released.live_domain_compositions(), 0);
    assert_eq!(released.live_domain_states(), 0);
    assert_eq!(released.live_domain_frames(), 0);
    assert_eq!(released.retained_domain_metadata_bytes(), 0);
    assert_eq!(
        released.retained_domain_metadata_bytes_high_water(),
        charged.retained_domain_metadata_bytes_high_water()
    );

    let sizing_context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let sizing_composition = two_domain_composition(&sizing_context, 1, false);
    let exact_bytes = sizing_composition.retained_metadata_bytes();
    drop(sizing_composition);
    let byte_limits = DomainRetentionLimits3D::try_new(
        2,
        conservative.max_live_domain_states(),
        conservative.max_live_domain_frames(),
        exact_bytes,
    )
    .unwrap();
    let byte_context = Context3D::new_cpu_only_with_domain_retention_for_test(
        PolicyLimits3D::conservative(),
        byte_limits,
        u64::MAX,
    )
    .unwrap();
    let exact = two_domain_composition(&byte_context, 1, false);
    assert_eq!(
        byte_context
            .domain_retention_usage()
            .unwrap()
            .retained_domain_metadata_bytes(),
        exact_bytes
    );
    assert_eq!(
        try_two_domain_composition(&byte_context, 2, false).unwrap_err(),
        DomainComposition3Error::Retention(DomainRetentionError3D::BudgetExceeded {
            budget: DomainRetentionBudget3D::RetainedDomainMetadataBytes,
            requested: exact_bytes * 2,
            maximum: exact_bytes,
        })
    );
    drop(exact);
    assert_eq!(
        byte_context
            .domain_retention_usage()
            .unwrap()
            .retained_domain_metadata_bytes(),
        0
    );
}

fn gpu_setup(policy: PolicyLimits3D) -> Option<(WgpuRenderer, GpuResources, Arc<Context3D>)> {
    let renderer = pollster::block_on(WgpuRenderer::new_headless())?;
    let gpu = renderer.gpu_resources();
    let context = Arc::new(Context3D::with_policy(gpu.clone(), policy).ok()?);
    Some((renderer, gpu, context))
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn center_linear_rgba(
    format: wgpu::TextureFormat,
    pixels: &[[u8; 4]],
    width: usize,
    height: usize,
) -> [f32; 4] {
    let pixel = pixels[(height / 2) * width + width / 2];
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

fn assert_opaque_color(actual: [f32; 4], expected: [f32; 3]) {
    for (actual, expected) in actual[..3].iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= 0.02,
            "expected color {expected:?}, got {actual:?}"
        );
    }
    assert!((actual[3] - 1.0).abs() <= 0.02);
}

#[test]
fn domain_lowering_renders_independent_domains_in_one_shared_depth_world() {
    let Some((_renderer, gpu, context)) = gpu_setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let composition = two_domain_composition(&context, 1, false);
    let front_planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        embedding(0.0, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let unchanged_spatial = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        1,
        embedding(0.0, 0.0, -3.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let front_frame = frame(
        &context,
        &composition,
        1,
        front_planar,
        unchanged_spatial.clone(),
    );
    let view = view_at(0.0);
    let front = context
        .render_headless_snapshot_for_test(
            front_frame.composition().render_snapshot(),
            front_frame.model(),
            view,
            view.pose().translation(),
            [64, 64],
            1,
        )
        .unwrap();
    assert_opaque_color(
        center_linear_rgba(front.format(), &front.readback_rgba8(&gpu).unwrap(), 64, 64),
        [1.0, 0.0, 0.0],
    );

    let moved_planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        2,
        embedding(0.0, 0.0, -4.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let moved_frame = frame(
        &context,
        &composition,
        2,
        moved_planar,
        unchanged_spatial.clone(),
    );
    assert!(
        moved_frame
            .state(SPATIAL_DOMAIN)
            .unwrap()
            .same_identity(&unchanged_spatial)
    );
    let moved = context
        .render_headless_snapshot_for_test(
            moved_frame.composition().render_snapshot(),
            moved_frame.model(),
            view,
            view.pose().translation(),
            [64, 64],
            1,
        )
        .unwrap();
    assert_opaque_color(
        center_linear_rgba(moved.format(), &moved.readback_rgba8(&gpu).unwrap(), 64, 64),
        [0.0, 0.0, 1.0],
    );
}

#[test]
fn structural_domain_removal_preserves_sibling_resources_and_placement() {
    let Some((_renderer, gpu, context)) = gpu_setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context.register_geometry(triangle()).unwrap();
    let red = context
        .register_appearance(appearance([1.0, 0.0, 0.0]))
        .unwrap();
    let blue = context
        .register_appearance(appearance([0.0, 0.0, 1.0]))
        .unwrap();
    let planar_packet = DrawPacketDescriptor::new(
        geometry.handle().clone(),
        red.handle().clone(),
        RasterMode::SingleSided,
        11,
    );
    let spatial_packet = DrawPacketDescriptor::new(
        geometry.handle().clone(),
        blue.handle().clone(),
        RasterMode::SingleSided,
        22,
    );
    let initial_composition = context
        .admit_domain_composition(DomainComposition3Descriptor::new(
            StructuralRevision::new(1),
            vec![
                LocalDomain3Descriptor::new(
                    PLANAR_DOMAIN,
                    LocalDomainKind3::Planar2,
                    vec![DomainContent3Descriptor::new(
                        PLANAR_CONTENT,
                        vec![planar_packet],
                    )],
                ),
                LocalDomain3Descriptor::new(
                    SPATIAL_DOMAIN,
                    LocalDomainKind3::Spatial3,
                    vec![DomainContent3Descriptor::new(
                        SPATIAL_CONTENT,
                        vec![spatial_packet.clone()],
                    )],
                ),
            ],
        ))
        .unwrap();
    let initial_planar = state(
        &context,
        &initial_composition,
        PLANAR_DOMAIN,
        1,
        embedding(0.0, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let initial_spatial = state(
        &context,
        &initial_composition,
        SPATIAL_DOMAIN,
        1,
        embedding(0.0, 0.0, -2.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let initial_frame = frame(
        &context,
        &initial_composition,
        1,
        initial_planar.clone(),
        initial_spatial.clone(),
    );
    let sibling_placement = initial_frame.model().placements()[1];
    let view = view_at(0.0);
    let mut viewport = Viewport3D::new_domain_composition(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        initial_frame.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(35, 64, 64);
    viewport.prepare_custom_render(&metrics);
    let warm = viewport_audit::snapshot();

    let reduced_composition = context
        .admit_domain_composition(DomainComposition3Descriptor::new(
            StructuralRevision::new(2),
            vec![LocalDomain3Descriptor::new(
                SPATIAL_DOMAIN,
                LocalDomainKind3::Spatial3,
                vec![DomainContent3Descriptor::new(
                    SPATIAL_CONTENT,
                    vec![spatial_packet],
                )],
            )],
        ))
        .unwrap();
    let reduced_spatial = state(
        &context,
        &reduced_composition,
        SPATIAL_DOMAIN,
        1,
        embedding(0.0, 0.0, -2.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let reduced_frame = context
        .admit_domain_frame(
            &reduced_composition,
            DomainFrame3Descriptor::new(ModelRevision::new(2), vec![reduced_spatial.clone()]),
        )
        .unwrap();
    assert_eq!(
        reduced_composition.domains().collect::<Vec<_>>(),
        vec![(SPATIAL_DOMAIN, LocalDomainKind3::Spatial3)]
    );
    assert_eq!(reduced_frame.model().placements(), &[sibling_placement]);
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(1),
            view,
            reduced_frame.clone(),
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after = viewport_audit::snapshot();
    assert_eq!(after.resource_generations, warm.resource_generations);
    assert_eq!(after.shader_modules, warm.shader_modules);
    assert_eq!(after.pipelines, warm.pipelines);

    let reduced_output = context
        .render_headless_snapshot_for_test(
            reduced_frame.composition().render_snapshot(),
            reduced_frame.model(),
            view,
            view.pose().translation(),
            [64, 64],
            1,
        )
        .unwrap();
    assert_opaque_color(
        center_linear_rgba(
            reduced_output.format(),
            &reduced_output.readback_rgba8(&gpu).unwrap(),
            64,
            64,
        ),
        [0.0, 0.0, 1.0],
    );

    let before_release = context.domain_retention_usage().unwrap();
    assert_eq!(before_release.live_domain_compositions(), 2);
    assert_eq!(before_release.live_domain_states(), 3);
    assert_eq!(before_release.live_domain_frames(), 2);
    drop(initial_frame);
    drop(initial_planar);
    drop(initial_spatial);
    drop(initial_composition);
    let after_release = context.domain_retention_usage().unwrap();
    assert_eq!(after_release.live_domain_compositions(), 1);
    assert_eq!(after_release.live_domain_states(), 1);
    assert_eq!(after_release.live_domain_frames(), 1);
    assert!(
        after_release.retained_domain_metadata_bytes()
            < before_release.retained_domain_metadata_bytes()
    );
}

#[test]
fn domain_viewport_preserves_association_last_good_and_narrow_updates() {
    let Some((_renderer, _gpu, context)) = gpu_setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let composition = two_domain_composition(&context, 1, false);
    let planar_1 = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        1,
        embedding(0.0, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let spatial_1 = state(
        &context,
        &composition,
        SPATIAL_DOMAIN,
        1,
        embedding(1.5, 0.0, -2.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let frame_1 = frame(&context, &composition, 1, planar_1, spatial_1.clone());
    let consumer_source_sentinel = [0x41_u8; 32];
    let initial_view = view_at(0.0);
    let mut viewport = Viewport3D::new_domain_composition(
        context.clone(),
        Viewport3DViewRevision::new(1),
        initial_view,
        frame_1.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(31, 96, 72);
    viewport.prepare_custom_render(&metrics);

    let initial_status = viewport.current_prepared_target().unwrap();
    assert_eq!(
        initial_status.status().state(),
        Viewport3DLifecycleState::Committed
    );
    let (initial_association, initial_ray) = initial_status
        .committed_domain_ray(PLANAR_DOMAIN, 0.5, 0.5)
        .unwrap()
        .unwrap();
    assert_eq!(initial_association.domain_id(), PLANAR_DOMAIN);
    assert_eq!(
        initial_association.target_key().surface_id(),
        SurfaceId::test_id(31)
    );
    assert_eq!(
        initial_association.target_key().physical_render_extent(),
        PhysicalExtent::new(96, 72)
    );
    assert_eq!(
        initial_association.view_revision(),
        Viewport3DViewRevision::new(1)
    );
    assert_eq!(
        initial_association.structural_revision(),
        StructuralRevision::new(1)
    );
    assert_eq!(initial_association.model_revision(), ModelRevision::new(1));
    assert_eq!(
        initial_association.domain_state_revision(),
        DomainStateRevision3::new(1)
    );
    assert!(
        initial_ray
            .intersect_planar(PlanarRayAccess3::TwoSided)
            .unwrap()
            .is_some()
    );
    let spatial_ray = initial_status
        .committed_domain_ray(SPATIAL_DOMAIN, 0.5, 0.5)
        .unwrap()
        .unwrap()
        .1;
    assert_eq!(spatial_ray.kind(), LocalDomainKind3::Spatial3);
    assert_close(spatial_ray.world_units_per_local_parameter(), 1.0);
    assert!(spatial_ray.point_at(1.0).is_ok());
    assert!(
        initial_status
            .committed_domain(LocalDomainId3::new(404))
            .is_none()
    );
    assert!(
        initial_status
            .committed_domain_ray(PLANAR_DOMAIN, f64::NAN, 0.5)
            .is_err()
    );
    assert_eq!(
        viewport
            .set_request(
                Viewport3DViewRevision::new(1),
                initial_view,
                frame_1.composition().render_snapshot().clone(),
                frame_1.model().clone(),
                SampleCount3D::ONE,
            )
            .unwrap_err(),
        Viewport3DError::RequestKindMismatch
    );

    let warm = viewport_audit::snapshot();
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(1),
            initial_view,
            frame_1.clone(),
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    assert_eq!(viewport_audit::snapshot(), warm);

    let moved_view = view_at(0.1);
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            frame_1.clone(),
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
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .status()
            .committed()
            .unwrap()
            .domain_frame()
            .unwrap()
            .same_identity(&frame_1)
    );
    assert_eq!(consumer_source_sentinel, [0x41_u8; 32]);

    let planar_2 = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        2,
        embedding(0.2, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let frame_2 = frame(&context, &composition, 2, planar_2, spatial_1.clone());
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            frame_2.clone(),
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after_domain_move = viewport_audit::snapshot();
    assert_eq!(
        after_domain_move.dynamic_writes - after_view.dynamic_writes,
        1
    );
    assert_eq!(
        after_domain_move.dynamic_bytes_written - after_view.dynamic_bytes_written,
        context.abi_layout().object_block_size()
    );
    let committed_before_failure = viewport
        .current_prepared_target()
        .unwrap()
        .committed_domain(PLANAR_DOMAIN)
        .unwrap();
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .status()
            .committed()
            .unwrap()
            .domain_frame()
            .unwrap()
            .state(SPATIAL_DOMAIN)
            .unwrap()
            .same_identity(&spatial_1)
    );

    let invalid_planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        3,
        embedding(1.0e300, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let invalid_frame = frame(&context, &composition, 3, invalid_planar, spatial_1.clone());
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            invalid_frame,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let failed_status = viewport.current_prepared_target().unwrap();
    assert_eq!(
        failed_status.status().state(),
        Viewport3DLifecycleState::FailedWithLastGood
    );
    assert_eq!(
        failed_status.status().requested().unwrap().model_revision(),
        ModelRevision::new(3)
    );
    assert_eq!(
        failed_status.status().committed().unwrap().model_revision(),
        ModelRevision::new(2)
    );
    assert!(failed_status.is_current_domain_association(&committed_before_failure));
    let (failed_association, failed_ray) = failed_status
        .committed_domain_ray(PLANAR_DOMAIN, 0.5, 0.5)
        .unwrap()
        .unwrap();
    assert_eq!(
        failed_association.request_serial(),
        committed_before_failure.request_serial()
    );
    assert_eq!(
        failed_association.domain_state_revision(),
        DomainStateRevision3::new(2)
    );
    assert!(
        failed_ray
            .intersect_planar(PlanarRayAccess3::TwoSided)
            .unwrap()
            .is_some()
    );

    let recovered_planar = state(
        &context,
        &composition,
        PLANAR_DOMAIN,
        4,
        embedding(-0.2, 0.0, -1.0, 1.0),
        DomainContentPlacement3::identity(),
    );
    let recovered_frame = frame(&context, &composition, 4, recovered_planar, spatial_1);
    viewport
        .set_domain_composition_request(
            Viewport3DViewRevision::new(2),
            moved_view,
            recovered_frame,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let recovered_status = viewport.current_prepared_target().unwrap();
    assert_eq!(
        recovered_status.status().state(),
        Viewport3DLifecycleState::Committed
    );
    assert!(!recovered_status.is_current_domain_association(&committed_before_failure));
    let recovered_association = recovered_status.committed_domain(PLANAR_DOMAIN).unwrap();

    viewport.prepare_custom_render(&active_metrics(31, 128, 96));
    let resized_status = viewport.current_prepared_target().unwrap();
    assert_eq!(
        resized_status.status().state(),
        Viewport3DLifecycleState::Committed
    );
    assert!(!resized_status.is_current_domain_association(&recovered_association));
    assert_eq!(
        resized_status
            .committed_domain(PLANAR_DOMAIN)
            .unwrap()
            .target_key()
            .physical_render_extent(),
        PhysicalExtent::new(128, 96)
    );

    viewport.prepare_custom_render(&inactive_metrics(31, 96, 72));
    let inactive_status = viewport.current_prepared_target().unwrap();
    assert_eq!(
        inactive_status.status().state(),
        Viewport3DLifecycleState::Inactive
    );
    assert!(
        inactive_status
            .committed_domain_ray(PLANAR_DOMAIN, 0.5, 0.5)
            .unwrap()
            .is_none()
    );
}

#[test]
fn flat_viewport_performs_zero_domain_work() {
    let Some((_renderer, _gpu, context)) = gpu_setup(PolicyLimits3D::conservative()) else {
        return;
    };
    let geometry = context.register_geometry(triangle()).unwrap();
    let appearance = context
        .register_appearance(appearance([0.25, 0.5, 1.0]))
        .unwrap();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![DrawPacketDescriptor::new(
                geometry.handle().clone(),
                appearance.handle().clone(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();
    let model = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(
                ModelRevision::new(1),
                vec![
                    ModelPlacement3::try_new(
                        WorldPoint3::try_new(0.0, 0.0, -1.0).unwrap(),
                        Orientation3::identity(),
                        PositiveScale3::one(),
                    )
                    .unwrap(),
                ],
            ),
        )
        .unwrap();
    let view = view_at(0.0);
    let before = viewport_audit::snapshot();
    let mut viewport = Viewport3D::new(
        context.clone(),
        Viewport3DViewRevision::new(1),
        view,
        snapshot.clone(),
        model.clone(),
        SampleCount3D::ONE,
    )
    .unwrap();
    let metrics = active_metrics(41, 64, 64);
    viewport.prepare_custom_render(&metrics);
    viewport
        .set_request(
            Viewport3DViewRevision::new(1),
            view,
            snapshot,
            model,
            SampleCount3D::ONE,
        )
        .unwrap();
    viewport.prepare_custom_render(&metrics);
    let after = viewport_audit::snapshot();
    assert_eq!(
        after.domain_composition_admissions,
        before.domain_composition_admissions
    );
    assert_eq!(
        after.domain_state_admissions,
        before.domain_state_admissions
    );
    assert_eq!(
        after.domain_frame_admissions,
        before.domain_frame_admissions
    );
    assert_eq!(
        after.domain_content_lowerings,
        before.domain_content_lowerings
    );
    assert_eq!(
        after.domain_association_lookups,
        before.domain_association_lookups
    );
    assert!(
        viewport
            .current_prepared_target()
            .unwrap()
            .status()
            .committed()
            .unwrap()
            .domain_frame()
            .is_none()
    );
    let usage = context.domain_retention_usage().unwrap();
    assert_eq!(usage.live_domain_compositions(), 0);
    assert_eq!(usage.live_domain_states(), 0);
    assert_eq!(usage.live_domain_frames(), 0);
    assert_eq!(usage.retained_domain_metadata_bytes(), 0);
    assert_eq!(usage.live_domain_compositions_high_water(), 0);
    assert_eq!(usage.live_domain_states_high_water(), 0);
    assert_eq!(usage.live_domain_frames_high_water(), 0);
    assert_eq!(usage.retained_domain_metadata_bytes_high_water(), 0);
}
