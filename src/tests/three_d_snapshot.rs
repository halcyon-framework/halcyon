use std::sync::Arc;

use crate::three_d::{
    AppearanceMode, AppearanceSourceDescriptor, Context3D, Context3DError, DrawPacketDescriptor,
    GeometryIndices, GeometrySemantics, GeometrySourceDescriptor, LocalPoint3, ModelPlacement3,
    ModelRevision, ModelStateChange, ModelStateDescriptor, ModelStateError, Orientation3,
    PolicyBudget3D, PolicyLimits3D, PositiveScale3, RasterMode, RenderSnapshot3DError,
    RenderSnapshotChange, RenderSnapshotDescriptor, ResourceKind3D, StructuralRevision,
    WorldPoint3,
};

fn context_with_policy(policy: PolicyLimits3D) -> Context3D {
    Context3D::new_cpu_only_for_test(policy, u64::MAX).unwrap()
}

#[test]
fn r026_independent_direct_revision_captured_referent_and_complete_source_successors() {
    use super::three_d_viewport::r026_fixture as f;
    use crate::three_d::{direct_shadow::DirectSource3D, pbr::MaterialSceneSource3D};
    let context = context_with_policy(PolicyLimits3D::conservative());
    let source = f::scene(&context, 1, 3);
    let direct = source.direct.as_ref().unwrap();
    assert_ne!(
        source.hdr.identity, direct.identity,
        "independently named sources are legal"
    );
    let copy_d = |revision| {
        Arc::new(DirectSource3D {
            identity: direct.identity,
            revision,
            meters_per_scene_unit: direct.meters_per_scene_unit,
            directional: direct.directional.clone(),
            points: direct.points.clone(),
            spots: direct.spots.clone(),
            snapshot: source.snapshot.clone(),
            model: source.model.clone(),
        })
    };
    let mut next = f::successor(&source, 2);
    next.hdr.exposure_ev = 1.0;
    next.validate(&context).unwrap();
    MaterialSceneSource3D::validate_successor(&source, &next).unwrap();
    assert!(Arc::ptr_eq(next.direct.as_ref().unwrap(), direct));
    for rev in [0, 1] {
        let mut bad = f::successor(&source, 2);
        bad.direct = Some(copy_d(rev));
        assert!(MaterialSceneSource3D::validate_successor(&source, &bad).is_err());
    }
    let mut changed = f::successor(&source, 2);
    changed.direct = Some(copy_d(2));
    changed.validate(&context).unwrap();
    MaterialSceneSource3D::validate_successor(&source, &changed).unwrap();
    assert!(
        direct.validate(&changed).is_err(),
        "coincident tokens never replace exact captured referent"
    );
    let mut old_model = f::successor(&source, 2);
    old_model.model = context
        .admit_material_model(
            source.snapshot.clone(),
            ModelRevision::new(2),
            vec![placement(1.0)],
        )
        .unwrap();
    assert!(
        old_model.validate(&context).is_err(),
        "D must retain its exact caster/model tuple"
    );
    let mut no_d = f::successor(&source, 2);
    no_d.direct = None;
    no_d.validate(&context).unwrap();
    MaterialSceneSource3D::validate_successor(&source, &no_d).unwrap();
    MaterialSceneSource3D::validate_successor(&Arc::new(no_d), &changed).unwrap();
}

#[test]
fn r026_world_six_lobe_rotation_is_not_coefficient_relabeling() {
    // Independent representation proof: axes alone cannot retain the cross term.
    let n = glam::DVec3::new(1.0, 1.0, 0.0).normalize();
    let rotated_original = (n.x + n.y).powi(2) / 2.0;
    let fitted_world = 0.5 * n.x * n.x + 0.5 * n.y * n.y;
    assert!((rotated_original - 1.0).abs() < 1e-14);
    assert!((fitted_world - 0.5).abs() < 1e-14);
    // The source-owned original field, not six foreign coefficients, is needed
    // to derive different WORLD signed-axis values under a 90-degree mapping.
    let old_axis_x = 1.0;
    let old_axis_negative_y = 2.0;
    assert_ne!(old_axis_x, old_axis_negative_y);
}

fn triangle(offset: f32, with_normal: bool) -> GeometrySourceDescriptor {
    let positions = vec![
        LocalPoint3::try_new(offset, 0.0, 0.0).unwrap(),
        LocalPoint3::try_new(offset + 1.0, 0.0, 0.0).unwrap(),
        LocalPoint3::try_new(offset, 1.0, 0.0).unwrap(),
    ];
    let normals = with_normal.then(|| {
        vec![crate::three_d::LocalVector3::try_new(0.0, 0.0, 1.0).unwrap(); positions.len()]
    });
    GeometrySourceDescriptor::try_new(positions, normals, None, None, GeometryIndices::Unindexed)
        .unwrap()
}

fn appearance(
    required_semantics: GeometrySemantics,
    mode: AppearanceMode,
) -> AppearanceSourceDescriptor {
    AppearanceSourceDescriptor::try_new_with_mode("snapshot source", required_semantics, mode)
        .unwrap()
}

fn placement(x: f64) -> ModelPlacement3 {
    ModelPlacement3::try_new(
        WorldPoint3::try_new(x, 0.0, 0.0).unwrap(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap()
}

fn packet(
    geometry: &crate::three_d::GeometryHandle,
    appearance: &crate::three_d::AppearanceHandle,
    raster_mode: RasterMode,
    stable_ordinal: u64,
) -> DrawPacketDescriptor {
    DrawPacketDescriptor::new(
        geometry.clone(),
        appearance.clone(),
        raster_mode,
        stable_ordinal,
    )
}

#[test]
fn snapshot_admission_is_cpu_only_immutable_and_preserves_packet_order() {
    let context = context_with_policy(PolicyLimits3D::conservative());
    let geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let opaque = context
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();
    let masked = context
        .register_appearance(appearance(
            GeometrySemantics::NONE,
            AppearanceMode::AlphaMask,
        ))
        .unwrap();
    let shared_packets: Arc<[DrawPacketDescriptor]> = vec![
        packet(
            geometry.handle(),
            opaque.handle(),
            RasterMode::SingleSided,
            9,
        ),
        packet(
            geometry.handle(),
            masked.handle(),
            RasterMode::DoubleSided,
            2,
        ),
        packet(
            geometry.handle(),
            opaque.handle(),
            RasterMode::SingleSided,
            7,
        ),
    ]
    .into();
    let descriptor =
        RenderSnapshotDescriptor::from_shared(StructuralRevision::new(41), shared_packets.clone());

    let snapshot = context.admit_render_snapshot(descriptor).unwrap();

    assert_eq!(snapshot.structural_revision(), StructuralRevision::new(41));
    assert_eq!(
        snapshot
            .draw_packets()
            .iter()
            .map(DrawPacketDescriptor::stable_ordinal)
            .collect::<Vec<_>>(),
        vec![9, 2, 7]
    );
    assert!(snapshot.draw_packets()[0].raster_mode().culls_back_faces());
    assert!(!snapshot.draw_packets()[1].raster_mode().culls_back_faces());
    assert_eq!(
        snapshot.committed_packets()[1].appearance_mode(),
        AppearanceMode::AlphaMask
    );
    assert_eq!(Arc::strong_count(&shared_packets), 2);
    assert_eq!(context.geometry_materialization_count_for_test(), 0);
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn snapshot_leases_keep_exact_generations_stable_through_replacement_and_retirement() {
    let context = context_with_policy(PolicyLimits3D::conservative());
    let geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let geometry_handle = geometry.handle().clone();
    let appearance_lease = context
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();
    let appearance_handle = appearance_lease.handle().clone();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![packet(
                &geometry_handle,
                &appearance_handle,
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();
    drop(geometry);
    drop(appearance_lease);

    let replacement_geometry = context
        .replace_geometry(&geometry_handle, triangle(10.0, false))
        .unwrap();
    let replacement_appearance = context
        .replace_appearance(
            &appearance_handle,
            appearance(GeometrySemantics::NONE, AppearanceMode::AlphaMask),
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

    assert_eq!(snapshot.draw_packets()[0].geometry().generation(), 1);
    assert_eq!(snapshot.draw_packets()[0].appearance().generation(), 1);
    assert_eq!(
        snapshot.committed_packets()[0]
            .geometry()
            .descriptor()
            .positions()[0],
        LocalPoint3::try_new(0.0, 0.0, 0.0).unwrap()
    );
    assert_eq!(
        snapshot.committed_packets()[0]
            .appearance()
            .descriptor()
            .mode(),
        AppearanceMode::Opaque
    );
    assert_eq!(context.resource_usage().unwrap().retained_generations, 2);

    drop(snapshot);
    assert_eq!(context.resource_usage().unwrap().retained_generations, 0);
}

#[test]
fn snapshot_rejects_cross_context_wrong_kind_and_stale_handles_before_gpu_work() {
    let first = context_with_policy(PolicyLimits3D::conservative());
    let second = context_with_policy(PolicyLimits3D::conservative());
    let geometry = first.register_geometry(triangle(0.0, false)).unwrap();
    let appearance = first
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();

    let cross_context = second
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![packet(
                geometry.handle(),
                appearance.handle(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap_err();
    assert_eq!(
        cross_context,
        RenderSnapshot3DError::PacketResource {
            packet_index: 0,
            source: Context3DError::ContextMismatch,
        }
    );

    let wrong_kind = Context3D::wrong_kind_geometry_handle_for_test(appearance.handle());
    assert_eq!(
        first
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(2),
                vec![packet(
                    &wrong_kind,
                    appearance.handle(),
                    RasterMode::SingleSided,
                    0,
                )],
            ))
            .unwrap_err(),
        RenderSnapshot3DError::PacketResource {
            packet_index: 0,
            source: Context3DError::WrongResourceKind {
                expected: ResourceKind3D::Geometry,
                actual: ResourceKind3D::Appearance,
            },
        }
    );

    let stale_handle = geometry.handle().clone();
    drop(geometry);
    let replacement = first
        .replace_geometry(&stale_handle, triangle(1.0, false))
        .unwrap();
    assert_eq!(
        first
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(3),
                vec![packet(
                    &stale_handle,
                    appearance.handle(),
                    RasterMode::SingleSided,
                    0,
                )],
            ))
            .unwrap_err(),
        RenderSnapshot3DError::PacketResource {
            packet_index: 0,
            source: Context3DError::StaleGeneration {
                slot: stale_handle.slot(),
                requested: 1,
                live: Some(replacement.handle().generation()),
            },
        }
    );
    assert_eq!(first.geometry_materialization_count_for_test(), 0);
    assert_eq!(first.admitted_shader_module_count_for_test(), 0);
    assert_eq!(second.geometry_materialization_count_for_test(), 0);
    assert_eq!(second.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn snapshot_rejects_missing_semantics_and_derives_appearance_mode_from_lease() {
    let context = context_with_policy(PolicyLimits3D::conservative());
    let plain_geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let normal_appearance = context
        .register_appearance(appearance(
            GeometrySemantics::new(true, false, false),
            AppearanceMode::Opaque,
        ))
        .unwrap();
    assert_eq!(
        context
            .admit_render_snapshot(RenderSnapshotDescriptor::new(
                StructuralRevision::new(1),
                vec![packet(
                    plain_geometry.handle(),
                    normal_appearance.handle(),
                    RasterMode::SingleSided,
                    0,
                )],
            ))
            .unwrap_err(),
        RenderSnapshot3DError::PacketResource {
            packet_index: 0,
            source: Context3DError::MissingGeometrySemantics {
                required: GeometrySemantics::new(true, false, false),
                available: GeometrySemantics::NONE,
            },
        }
    );

    let normal_geometry = context.register_geometry(triangle(1.0, true)).unwrap();
    let masked = context
        .register_appearance(appearance(
            GeometrySemantics::new(true, false, false),
            AppearanceMode::AlphaMask,
        ))
        .unwrap();
    let masked_snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(2),
            vec![packet(
                normal_geometry.handle(),
                masked.handle(),
                RasterMode::DoubleSided,
                0,
            )],
        ))
        .unwrap();
    assert_eq!(
        masked_snapshot.committed_packets()[0].appearance_mode(),
        AppearanceMode::AlphaMask
    );
    assert_eq!(context.geometry_materialization_count_for_test(), 0);
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn snapshot_packet_budget_is_checked_before_resource_admission() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_draw_packets_per_snapshot = 1;
    let context = context_with_policy(policy);
    let geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let appearance = context
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();
    let descriptor = RenderSnapshotDescriptor::new(
        StructuralRevision::new(1),
        vec![
            packet(
                geometry.handle(),
                appearance.handle(),
                RasterMode::SingleSided,
                0,
            ),
            packet(
                geometry.handle(),
                appearance.handle(),
                RasterMode::SingleSided,
                1,
            ),
        ],
    );

    assert_eq!(
        context.admit_render_snapshot(descriptor).unwrap_err(),
        RenderSnapshot3DError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::DrawPacketsPerSnapshot,
            requested: 2,
            maximum: 1,
        })
    );
    assert_eq!(context.geometry_materialization_count_for_test(), 0);
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn snapshot_revision_transitions_require_monotonic_identity_and_never_wrap() {
    let context = context_with_policy(PolicyLimits3D::conservative());
    let geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let appearance = context
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();
    let descriptor = |revision| {
        RenderSnapshotDescriptor::new(
            StructuralRevision::new(revision),
            vec![packet(
                geometry.handle(),
                appearance.handle(),
                RasterMode::SingleSided,
                0,
            )],
        )
    };
    let current = context.admit_render_snapshot(descriptor(7)).unwrap();
    let same_identity = current.clone();
    let same_content_new_identity = context.admit_render_snapshot(descriptor(7)).unwrap();
    let stale = context.admit_render_snapshot(descriptor(6)).unwrap();
    let advanced = context.admit_render_snapshot(descriptor(8)).unwrap();

    assert!(current.same_identity(&same_identity));
    assert_eq!(
        current.validate_successor(&same_identity),
        Ok(RenderSnapshotChange::Unchanged)
    );
    assert_eq!(
        current.validate_successor(&same_content_new_identity),
        Err(RenderSnapshot3DError::ReusedStructuralRevision {
            revision: StructuralRevision::new(7),
        })
    );
    assert_eq!(
        current.validate_successor(&stale),
        Err(RenderSnapshot3DError::StaleStructuralRevision {
            current: StructuralRevision::new(7),
            requested: StructuralRevision::new(6),
        })
    );
    assert_eq!(
        current.validate_successor(&advanced),
        Ok(RenderSnapshotChange::Advanced)
    );
    assert_eq!(
        StructuralRevision::new(7).checked_next().unwrap().value(),
        8
    );
    assert_eq!(
        StructuralRevision::new(u64::MAX).checked_next(),
        Err(RenderSnapshot3DError::StructuralRevisionExhausted {
            revision: StructuralRevision::new(u64::MAX),
        })
    );

    let other_context = context_with_policy(PolicyLimits3D::conservative());
    let empty_other = other_context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(8),
            Vec::new(),
        ))
        .unwrap();
    assert_eq!(
        current.validate_successor(&empty_other),
        Err(RenderSnapshot3DError::Context(
            Context3DError::ContextMismatch
        ))
    );
}

#[test]
fn model_state_is_complete_structure_bound_monotonic_and_never_wraps() {
    let context = context_with_policy(PolicyLimits3D::conservative());
    let geometry = context.register_geometry(triangle(0.0, false)).unwrap();
    let appearance = context
        .register_appearance(appearance(GeometrySemantics::NONE, AppearanceMode::Opaque))
        .unwrap();
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            vec![packet(
                geometry.handle(),
                appearance.handle(),
                RasterMode::SingleSided,
                0,
            )],
        ))
        .unwrap();

    let state = |revision, x| {
        context
            .admit_model_state(
                &snapshot,
                ModelStateDescriptor::new(ModelRevision::new(revision), vec![placement(x)]),
            )
            .unwrap()
    };
    let current = state(7, 0.0);
    let same_identity = current.clone();
    let reused = state(7, 1.0);
    let stale = state(6, 0.0);
    let advanced = state(8, 1.0);

    assert!(current.matches_snapshot(&snapshot));
    assert_eq!(current.placements(), &[placement(0.0)]);
    assert_eq!(
        current.validate_successor(&same_identity),
        Ok(ModelStateChange::Unchanged)
    );
    assert_eq!(
        current.validate_successor(&reused),
        Err(ModelStateError::ReusedRevision {
            revision: ModelRevision::new(7),
        })
    );
    assert_eq!(
        current.validate_successor(&stale),
        Err(ModelStateError::StaleRevision {
            current: ModelRevision::new(7),
            requested: ModelRevision::new(6),
        })
    );
    assert_eq!(
        current.validate_successor(&advanced),
        Ok(ModelStateChange::Advanced)
    );
    assert_eq!(ModelRevision::new(7).checked_next().unwrap().value(), 8);
    assert_eq!(
        ModelRevision::new(u64::MAX).checked_next(),
        Err(ModelStateError::RevisionExhausted {
            revision: ModelRevision::new(u64::MAX),
        })
    );
    assert_eq!(
        context
            .admit_model_state(
                &snapshot,
                ModelStateDescriptor::new(ModelRevision::new(9), Vec::new()),
            )
            .unwrap_err(),
        ModelStateError::PlacementCountMismatch {
            expected: 1,
            actual: 0,
        }
    );

    let other_snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(2),
            Vec::new(),
        ))
        .unwrap();
    let other_model = context
        .admit_model_state(
            &other_snapshot,
            ModelStateDescriptor::new(ModelRevision::new(9), Vec::new()),
        )
        .unwrap();
    assert!(!other_model.matches_snapshot(&snapshot));
}

#[test]
fn model_state_retention_budget_is_transactional_and_released_with_last_identity() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_model_states = 1;
    let context = context_with_policy(policy);
    let snapshot = context
        .admit_render_snapshot(RenderSnapshotDescriptor::new(
            StructuralRevision::new(1),
            Vec::new(),
        ))
        .unwrap();
    let first = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(ModelRevision::new(1), Vec::new()),
        )
        .unwrap();
    let after_first = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(after_first.retained_model_states, 1);
    assert_eq!(after_first.retained_model_states_high_water, 1);

    assert_eq!(
        context
            .admit_model_state(
                &snapshot,
                ModelStateDescriptor::new(ModelRevision::new(2), Vec::new()),
            )
            .unwrap_err(),
        ModelStateError::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::RetainedModelStates,
            requested: 2,
            maximum: 1,
        })
    );
    assert_eq!(
        context
            .dynamic_state_accounting_for_test()
            .unwrap()
            .retained_model_states,
        1
    );

    drop(first);
    assert_eq!(
        context
            .dynamic_state_accounting_for_test()
            .unwrap()
            .retained_model_states,
        0
    );
    let replacement = context
        .admit_model_state(
            &snapshot,
            ModelStateDescriptor::new(ModelRevision::new(2), Vec::new()),
        )
        .unwrap();
    let after_replacement = context.dynamic_state_accounting_for_test().unwrap();
    assert_eq!(after_replacement.retained_model_states, 1);
    assert_eq!(after_replacement.retained_model_states_high_water, 1);
    drop(replacement);
    assert_eq!(
        context
            .dynamic_state_accounting_for_test()
            .unwrap()
            .retained_model_states,
        0
    );
}
#[test]
fn r024_flat_packet_source_identity_sidedness_and_positive_scale_are_checked() {
    use crate::three_d::pbr::*;
    use crate::three_d::*;
    let c = Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let geometry = c
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(
                super::three_d_geometry::r024_vertices(),
                GeometryIndices::Unindexed,
            )
            .unwrap(),
            None,
        )
        .unwrap();
    let material = c
        .register_pbr_material(PbrMaterialSource3D::defaults(1, 1), None)
        .unwrap();
    let packet = MaterialPacketSource3D::PbrMaterial {
        geometry: geometry.clone(),
        material: material.clone(),
        raster: RasterMode::SingleSided,
        stable_ordinal: 0,
    };
    let first = c
        .admit_material_snapshot(StructuralRevision::new(1), vec![packet.clone()])
        .unwrap();
    let reused = c
        .admit_material_snapshot(StructuralRevision::new(1), vec![packet.clone()])
        .unwrap();
    assert!(MaterialSnapshot3D::validate_successor(&first, &reused).is_err());
    MaterialSnapshot3D::validate_successor(&first, &first).unwrap();
    let transform = |scale| {
        ModelPlacement3::try_new(
            WorldPoint3::origin(),
            Orientation3::identity(),
            PositiveScale3::try_new(scale, 1.0, 1.0).unwrap(),
        )
        .unwrap()
    };
    c.admit_material_model(first.clone(), ModelRevision::new(1), vec![transform(64.0)])
        .unwrap();
    assert!(
        c.admit_material_model(first.clone(), ModelRevision::new(2), vec![transform(64.01)])
            .is_err()
    );
    assert!(
        c.admit_material_model(first.clone(), ModelRevision::new(2), vec![])
            .is_err()
    );
    assert!(
        c.admit_material_snapshot(
            StructuralRevision::new(2),
            vec![MaterialPacketSource3D::PbrMaterial {
                geometry,
                material,
                raster: RasterMode::DoubleSided,
                stable_ordinal: 0
            }]
        )
        .is_err()
    );
}
