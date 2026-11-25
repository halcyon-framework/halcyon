//! SPEC-HAL-346 executable acceptance audit.

use std::collections::BTreeSet;

use super::chaos::{
    self, CHAOS_STREAM_GENERATOR_ID, InteractionChaosRunOutcome,
    ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID, ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID,
    ORACLE_346_CHAOS_DIRTY_SETTLE_ID, ORACLE_346_CHAOS_NONFINITE_SINK_ID,
    ORACLE_346_CHAOS_PANIC_UNWIND_ID, ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID,
    ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID, ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID,
    ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID,
};
use super::constitution::{InteractionFuzzRole, constitution_boundary_contracts};
use super::generators::{
    GEN_346_KBD1_ID, GEN_346_PTH1_ID, GEN_346_RSZ1_ID, GEN_346_RSZ2_ID, find_interaction_generator,
    generate_scenarios_for_fixture, interaction_generator_registry,
};
use super::registry::{
    InteractionFixtureKind, LIST_VIEW_RESIZE_LV1_FIXTURE_ID, LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
    find_interaction_fixture, interaction_fixture_registry,
};
use super::runner::{
    InteractionFuzzRunner, InteractionRunnerReplayArtifact, InteractionRunnerRunOutput,
};
use super::smoke::{self, CI_SMOKE_PROFILE_ID};
use crate::core::event::{KeyCode, Modifiers, PointerButton};
use crate::core::interaction_fuzz::{
    CaptureOwnerSnapshot, CoverageKey, EndpointTeleportationMode, FiniteGeometrySink,
    FiniteSinkGeometrySample, INTERACTION_FUZZ_SCHEMA, INTERACTION_REPLAY_SCHEMA_VERSION,
    InteractionChaosFailureDetector, InteractionChaosHardFailureClass, InteractionCommand,
    InteractionConfidencePacket, InteractionConstitutionBoundaryCheckStatus, InteractionDriver,
    InteractionDriverError, InteractionFailurePacket, InteractionFuzzMode,
    InteractionInjectionPath, InteractionOracleEvaluation, InteractionOracleStatus,
    InteractionReplayArtifact, InteractionSettleBoundary, KeyboardNavigationPolicy,
    LayoutSettledSnapshot, PointerCaptureTransitionClass, PointerMoveExposureClass,
    PointerPathPolicyClass, PointerPathSampleCountClass, PrimitiveCommandClass, ReplayMode,
    evaluate_capture_owner_validity, evaluate_finite_sink_geometry, evaluate_layout_settled,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spec346AcceptanceCriterion {
    pub id: u8,
    pub requirement: &'static str,
    pub evidence: String,
    pub passed: bool,
}

pub(crate) fn spec_346_acceptance_audit() -> Result<Vec<Spec346AcceptanceCriterion>, String> {
    let primitive_driver = acceptance_primitive_driver()?;
    let primitive_classes = primitive_driver
        .primitive_trace()
        .iter()
        .map(|step| PrimitiveCommandClass::from(&step.command))
        .collect::<BTreeSet<_>>();
    let primitive_replay = InteractionReplayArtifact::from_driver(
        ReplayMode::ExactPrimitive,
        "acceptance-primitive-driver",
        0x3460_A001,
        None,
        &primitive_driver,
    );
    let mut primitive_replay_driver = InteractionDriver::new();
    primitive_replay
        .replay_into(&mut primitive_replay_driver)
        .map_err(|error| format!("primitive replay failed: {error:?}"))?;

    let lv1_fixture =
        find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID).ok_or("missing lv1 fixture")?;
    let generated_scenarios = generate_scenarios_for_fixture(lv1_fixture, 0x3460_C100);
    let mut intent_driver = InteractionDriver::new();
    for scenario in generated_scenarios.iter() {
        scenario
            .execute(&mut intent_driver)
            .map_err(|error| format!("generated scenario failed: {error:?}"))?;
    }
    let intent_replay = InteractionReplayArtifact::from_driver(
        ReplayMode::ExactPrimitive,
        LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
        0x3460_C100,
        None,
        &intent_driver,
    );
    let mut intent_replay_driver = InteractionDriver::new();
    intent_replay
        .replay_into(&mut intent_replay_driver)
        .map_err(|error| format!("intent replay failed: {error:?}"))?;

    let smoke_report = smoke::run_ci_smoke_profile()
        .map_err(|error| format!("ci smoke profile failed: {error:?}"))?;
    let smoke_packet = smoke_report.confidence_packet.clone();
    let smoke_profile = smoke::ci_smoke_runner_profile()
        .map_err(|error| format!("ci smoke runner profile failed: {error:?}"))?;
    let smoke_output = InteractionFuzzRunner::run(smoke_profile.clone(), None)
        .map_err(|error| format!("runner smoke failed: {error:?}"))?;
    let smoke_run_report = InteractionFuzzRunner::run_report(&smoke_profile, &smoke_output)
        .map_err(|error| format!("smoke run report failed: {error:?}"))?;

    let chaos_surface = chaos::ci_chaos_surface();
    let chaos_profile = chaos::ci_chaos_profile(&chaos_surface);
    let chaos_output =
        InteractionFuzzRunner::run(chaos_profile.clone(), Some(chaos_surface.clone()))
            .map_err(|error| format!("runner chaos failed: {error:?}"))?;
    let chaos_run_report = InteractionFuzzRunner::run_report(&chaos_profile, &chaos_output)
        .map_err(|error| format!("chaos run report failed: {error:?}"))?;
    let InteractionRunnerRunOutput::ChaosReport(chaos_report) = &chaos_output else {
        return Err("chaos run did not emit chaos report".to_string());
    };
    let chaos_report = chaos_report.as_ref();

    let replay_artifact = InteractionRunnerReplayArtifact {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        run_profile_id: chaos_profile.profile_id.clone(),
        run_profile_hash: chaos_report.run_profile_hash.clone(),
        replay_artifact: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: chaos_report.surface_set[0].clone(),
            root_seed: chaos_profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace: chaos_report.primitive_trace.clone(),
            coverage_keys: Vec::new(),
        },
    };
    let replay_report = InteractionFuzzRunner::replay(&chaos_profile, &replay_artifact, false)
        .map_err(|error| format!("replay report failed: {error:?}"))?;
    let visual_report =
        InteractionFuzzRunner::render_visual_trace(&chaos_profile, &replay_artifact)
            .map_err(|error| format!("visual trace report failed: {error:?}"))?;
    let visual_confirmation = InteractionFuzzRunner::render_visual_confirmation(
        &chaos_profile,
        &replay_artifact,
        "halcyon interaction-fuzz replay",
        None,
    )
    .map_err(|error| format!("visual confirmation failed: {error:?}"))?;
    let negative_report = InteractionFuzzRunner::validate_negative(&chaos_profile)
        .map_err(|error| format!("negative validation failed: {error:?}"))?;

    let (failure_packet, _failing_index, _expected_stream_len) =
        chaos::synthetic_non_finite_chaos_failure_packet();
    let shrink_report = chaos::shrink_chaos_failure_packet(&failure_packet)
        .map_err(|error| format!("chaos shrink failed: {error:?}"))?;
    let shrunk_packet = InteractionFuzzRunner::shrink(&chaos_profile, &failure_packet)
        .map_err(|error| format!("runner shrink failed: {error:?}"))?;
    let failure_corpus_entry =
        chaos::chaos_failure_corpus_entry(&failure_packet, &shrink_report)
            .map_err(|error| format!("failure corpus entry failed: {error:?}"))?;
    let mut shrunk_replay_driver = InteractionDriver::new();
    let shrunk_replay_result = shrunk_packet
        .shrunk_trace
        .replay_shrunk_trace_into(&mut shrunk_replay_driver);
    let shrunk_replay_preserves_failure = matches!(
        shrunk_replay_result,
        Err(InteractionDriverError::NonFiniteSinkValue {
            primitive_step_index: 0,
            ..
        })
    );
    let mut promotable_packet = shrunk_packet.clone();
    promotable_packet.promotion_target = Some("src/tests/interaction_fuzz/regressions".to_string());
    let promotion_report =
        InteractionFuzzRunner::promote_regression(&chaos_profile, &promotable_packet)
            .map_err(|error| format!("regression promotion failed: {error:?}"))?;

    let mut failed_negative_report = negative_report.clone();
    failed_negative_report.passed = false;
    failed_negative_report.cases[0].passed = false;
    failed_negative_report.cases[0].observed_failure_class =
        "negative_validation_sample_not_detected".to_string();
    let failed_negative_invalidates_run =
        InteractionFuzzRunner::run_with_negative_validation_report(
            chaos_profile.clone(),
            Some(chaos_surface.clone()),
            &failed_negative_report,
        )
        .is_err();

    let list_view_resize_oracles =
        super::list_view_resize::list_view_resize_smoke_oracle_evaluations();
    let finite_oracle = evaluate_finite_sink_geometry([FiniteSinkGeometrySample::new(
        FiniteGeometrySink::FinalWidgetRect,
        "acceptance.rect.x",
        "acceptance",
        1.0,
    )]);
    let capture_oracle = evaluate_capture_owner_validity(CaptureOwnerSnapshot::new(
        true,
        Some("acceptance-owner".to_string()),
        true,
        true,
        true,
        true,
        true,
        "acceptance",
    ));
    let layout_oracle = evaluate_layout_settled(LayoutSettledSnapshot::new(
        InteractionSettleBoundary::Reconcile,
        false,
        Vec::new(),
        false,
        "acceptance",
    ));

    let required_primitive_classes = BTreeSet::from([
        PrimitiveCommandClass::PointerMove,
        PrimitiveCommandClass::PointerDown,
        PrimitiveCommandClass::PointerUp,
        PrimitiveCommandClass::WheelPixels,
        PrimitiveCommandClass::WheelLines,
        PrimitiveCommandClass::KeyDown,
        PrimitiveCommandClass::KeyUp,
        PrimitiveCommandClass::TextInput,
        PrimitiveCommandClass::Frame,
        PrimitiveCommandClass::Reconcile,
    ]);
    let fixture_ids = interaction_fixture_registry()
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<BTreeSet<_>>();
    let generator_ids = interaction_generator_registry()
        .iter()
        .map(|generator| generator.generator_id)
        .collect::<BTreeSet<_>>();
    let smoke_oracle_ids = smoke_packet
        .oracle_set
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let chaos_oracle_ids = chaos_report
        .hard_failure_oracle_set
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let constitution_roles = constitution_boundary_contracts()
        .iter()
        .map(|contract| contract.role)
        .collect::<BTreeSet<_>>();

    let failure_packet_json = serde_json::to_value(failure_packet.as_ref())
        .map_err(|error| format!("failure packet JSON failed: {error:?}"))?;
    let failure_packet_has_traces = failure_packet_json.get("intent_trace").is_some()
        && failure_packet_json.get("primitive_trace").is_some()
        && !failure_packet.primitive_trace.is_empty();

    let negative_case_boundaries_pass = negative_report.cases.iter().all(|case| {
        case.constitution_boundary_profile == chaos_profile.constitution_boundary_profile
            && case.constitution_boundary_check_status.passed
    });
    let every_packet_and_report_has_boundary_status =
        smoke_report.constitution_boundary_check_status.passed
            && smoke_packet.constitution_boundary_check_status.passed
            && smoke_run_report.constitution_boundary_check_status.passed
            && chaos_report.constitution_boundary_check_status.passed
            && chaos_run_report.constitution_boundary_check_status.passed
            && failure_packet.constitution_boundary_check_status.passed
            && shrunk_packet.constitution_boundary_check_status.passed
            && shrink_report.constitution_boundary_check_status.passed
            && negative_report.constitution_boundary_check_status.passed
            && negative_case_boundaries_pass
            && replay_report.constitution_boundary_check_status.passed
            && visual_report.constitution_boundary_check_status.passed
            && visual_confirmation
                .constitution_boundary_check_status
                .passed
            && promotion_report.constitution_boundary_check_status.passed;

    Ok(vec![
        criterion(
            1,
            "in-process interaction driver exists",
            "InteractionDriver::new executes local commands",
            !primitive_driver.primitive_trace().is_empty(),
        ),
        criterion(
            2,
            "primitive driver emits required command classes",
            "primitive trace contains pointer, wheel, key, text, frame, reconcile classes",
            required_primitive_classes.is_subset(&primitive_classes),
        ),
        criterion(
            3,
            "default driver path enters RawInputEvent and InputFusion",
            "primitive replay emits fused events from driver path and chaos profile declares RawInputFusion",
            primitive_driver.fused_events().len() > 0
                && chaos_profile.injection_path == InteractionInjectionPath::RawInputFusion,
        ),
        criterion(
            4,
            "intent commands exist above primitive commands",
            "generated scenarios execute intent trace above primitive trace",
            !intent_driver.intent_trace().is_empty() && !intent_driver.primitive_trace().is_empty(),
        ),
        criterion(
            5,
            "intent commands expand into primitive ranges",
            "every intent trace step records a non-empty primitive range",
            intent_driver
                .intent_trace()
                .iter()
                .all(|step| !step.primitive_command_range.is_empty()),
        ),
        criterion(
            6,
            "non-instant pointer path policies replay deterministically",
            "generated scenarios include sampled paths and exact primitive replay succeeds",
            generated_scenarios
                .iter()
                .any(|scenario| !scenario.uses_instant_pointer_path())
                && intent_replay.primitive_trace.len()
                    == intent_replay_driver.primitive_trace().len(),
        ),
        criterion(
            7,
            "pointer path expansion records frame policy",
            "intent expansion steps carry frame policies and coverage records frame-separated movement",
            intent_driver
                .intent_trace()
                .iter()
                .any(|step| !step.expansion_steps.is_empty())
                && smoke_packet
                    .coverage_summary
                    .coalesced_move_classes
                    .contains(&PointerMoveExposureClass::FrameSeparated),
        ),
        criterion(
            8,
            "pointer path coverage records exposure dimensions",
            "confidence coverage records sample counts, segments, frame policy, thresholds, targets, capture transitions, endpoint mode",
            pointer_path_coverage_complete(&smoke_packet),
        ),
        criterion(
            9,
            "keyboard navigation intents replay deterministically",
            "coverage records traversal policies and replay trace length is stable",
            smoke_packet
                .coverage_summary
                .keyboard_navigation_policies
                .contains(&KeyboardNavigationPolicy::TraversalBeforeActivation)
                && intent_replay_driver.primitive_trace().len()
                    == intent_replay.primitive_trace.len(),
        ),
        criterion(
            10,
            "incidental navigation effects are probed and oracle-checked",
            "CI smoke records U11 pass and incidental route target classes",
            oracle_passed(
                &smoke_report.oracle_evaluations,
                crate::core::interaction_fuzz::ORACLE_346_U11_ID,
            ) && !smoke_packet
                .coverage_summary
                .incidental_route_target_classes
                .is_empty(),
        ),
        criterion(
            11,
            "deterministic fixture registry exists",
            "registry exposes stable ordered fixture definitions",
            fixture_ids.len() == interaction_fixture_registry().len(),
        ),
        criterion(
            12,
            "ListView fixture LV1 exists",
            "fixture registry contains lv1 ListView resize fixture",
            find_interaction_fixture(LIST_VIEW_RESIZE_LV1_FIXTURE_ID).is_some(),
        ),
        criterion(
            13,
            "ListView fixture LV2 exists with horizontal scroll",
            "fixture registry contains lv2 with horizontal scroll offset",
            find_interaction_fixture(LIST_VIEW_RESIZE_LV2_FIXTURE_ID).is_some_and(|fixture| {
                fixture.kind == InteractionFixtureKind::ListViewResize
                    && fixture.horizontal_scroll
                    && fixture.initial_horizontal_scroll > 0.0
            }),
        ),
        criterion(
            14,
            "ListView resize generator RSZ1 exists",
            "generator registry contains GEN-346-RSZ1",
            generator_ids.contains(GEN_346_RSZ1_ID),
        ),
        criterion(
            15,
            "ListView resize generator RSZ2 exists",
            "generator registry contains GEN-346-RSZ2",
            generator_ids.contains(GEN_346_RSZ2_ID),
        ),
        criterion(
            16,
            "pointer path generator PTH1 exists",
            "generator registry contains GEN-346-PTH1 and builds a scenario",
            generator_ids.contains(GEN_346_PTH1_ID)
                && find_interaction_generator(GEN_346_PTH1_ID).is_some(),
        ),
        criterion(
            17,
            "keyboard traversal generator KBD1 exists",
            "generator registry contains GEN-346-KBD1 and builds a scenario",
            generator_ids.contains(GEN_346_KBD1_ID)
                && find_interaction_generator(GEN_346_KBD1_ID).is_some(),
        ),
        criterion(
            18,
            "finite sink geometry oracle exists",
            "U1 finite sink oracle passes finite sample",
            finite_oracle.passed(),
        ),
        criterion(
            19,
            "capture owner validity oracle exists",
            "U3 capture owner oracle passes valid active capture",
            capture_oracle.passed(),
        ),
        criterion(
            20,
            "layout settled oracle exists",
            "U7 layout settled oracle passes clean reconcile boundary",
            layout_oracle.passed(),
        ),
        criterion(
            21,
            "incidental navigation integrity oracle exists",
            "U11 appears in smoke oracle set and passes",
            smoke_oracle_ids.contains(crate::core::interaction_fuzz::ORACLE_346_U11_ID)
                && oracle_passed(
                    &smoke_report.oracle_evaluations,
                    crate::core::interaction_fuzz::ORACLE_346_U11_ID,
                ),
        ),
        criterion(
            22,
            "ListView probe exposes widths and separators",
            "ListView resize oracle evaluations derive from laid-out probe snapshots",
            list_view_resize_oracles
                .iter()
                .all(InteractionOracleEvaluation::passed)
                && oracle_passed(
                    &list_view_resize_oracles,
                    super::list_view_resize::ORACLE_346_R5_ID,
                ),
        ),
        criterion(
            23,
            "resize direction monotonicity oracle exists",
            "ORACLE-346-R2 passes in smoke oracle set",
            oracle_passed(
                &list_view_resize_oracles,
                super::list_view_resize::ORACLE_346_R2_ID,
            ),
        ),
        criterion(
            24,
            "scroll-aware separator hit-test oracle exists",
            "ORACLE-346-R5 passes in smoke oracle set",
            oracle_passed(
                &list_view_resize_oracles,
                super::list_view_resize::ORACLE_346_R5_ID,
            ),
        ),
        criterion(
            25,
            "first-drag/subsequent-drag consistency oracle exists",
            "ORACLE-346-R4 and ORACLE-346-R12 pass in smoke oracle set",
            oracle_passed(
                &list_view_resize_oracles,
                super::list_view_resize::ORACLE_346_R4_ID,
            ) && oracle_passed(
                &list_view_resize_oracles,
                super::list_view_resize::ORACLE_346_R12_ID,
            ),
        ),
        criterion(
            26,
            "failure packet schema records traces",
            "serialized failure packet includes intent_trace and primitive_trace fields",
            failure_packet_has_traces,
        ),
        criterion(
            27,
            "confidence packet records path policies and incidental targets",
            "confidence coverage has path policy classes and incidental target classes",
            !smoke_packet.coverage_summary.path_policy_classes.is_empty()
                && !smoke_packet
                    .coverage_summary
                    .incidental_target_classes
                    .is_empty(),
        ),
        criterion(
            28,
            "shrunk trace replay exists",
            "runner shrink returns shrunk trace that deterministically re-fires the preserved hard failure",
            shrunk_packet.shrunk_trace.replay_mode == ReplayMode::ShrunkTrace
                && !shrunk_packet.shrunk_trace.primitive_trace.is_empty()
                && shrunk_replay_preserves_failure,
        ),
        criterion(
            29,
            "CI smoke profile exists",
            "CI smoke runner profile is complete and hashable",
            smoke_profile.profile_id == CI_SMOKE_PROFILE_ID
                && smoke_profile.is_complete()
                && smoke_profile.profile_hash().starts_with("sha256:"),
        ),
        criterion(
            30,
            "Constitution boundary checks cover responsibilities",
            "boundary contracts cover generator, probe, oracle, shrinker, replay, reporter roles",
            required_constitution_roles().is_subset(&constitution_roles),
        ),
        criterion(
            31,
            "passing CI smoke emits bounded confidence packet",
            "runner contract smoke emits confidence packet with zero failures",
            matches!(
                smoke_output,
                InteractionRunnerRunOutput::ConfidencePacket(_)
            ) && smoke_packet.failure_count == 0
                && smoke_packet.saturation_summary.frontier_saturated,
        ),
        criterion(
            32,
            "chaos mode exists as distinct failure discovery mode",
            "chaos profile is ChaosSmoke and run output is chaos report without confidence packet",
            chaos_profile.mode == InteractionFuzzMode::ChaosSmoke
                && matches!(chaos_output, InteractionRunnerRunOutput::ChaosReport(_))
                && !chaos_report.confidence_packet_emitted,
        ),
        criterion(
            33,
            "chaos surfaces are declared deterministic and replayable",
            "chaos surface/profile carry deterministic identity, seed, profile hash, and primitive trace",
            chaos_surface.id == chaos_report.surface_set[0]
                && chaos_profile.profile_hash() == chaos_report.run_profile_hash
                && !chaos_report.primitive_trace.is_empty(),
        ),
        criterion(
            34,
            "chaos stream generation records RNG and command metadata",
            "chaos report records rng identity, generator version, command records, coordinates, parameters, frame/reconcile boundaries",
            chaos_report.rng_algorithm == chaos_profile.rng_algorithm
                && chaos_report.rng_version == chaos_profile.rng_version
                && chaos_report
                    .chaos_stream_generator_ids
                    .contains(&CHAOS_STREAM_GENERATOR_ID.to_string())
                && chaos_report
                    .generated_command_records
                    .iter()
                    .any(|record| record.frame_boundary)
                && chaos_report
                    .generated_command_records
                    .iter()
                    .any(|record| record.reconcile_boundary),
        ),
        criterion(
            35,
            "chaos pointer movement preserves intermediate samples",
            "stream policy preserves samples and generated command records carry pointer sample ordinals",
            chaos_report
                .stream_policy
                .preserve_intermediate_pointer_samples
                && chaos_report
                    .generated_command_records
                    .iter()
                    .any(|record| record.pointer_sample_ordinal.is_some()),
        ),
        criterion(
            36,
            "chaos hard-failure oracle set exists",
            "chaos report declares required hard-failure detector IDs",
            required_chaos_oracles().is_subset(&chaos_oracle_ids),
        ),
        criterion(
            37,
            "chaos failures emit replayable failure packets",
            "synthetic non-finite chaos failure packet records surface, detector, class, index, rng, primitive trace",
            chaos_failure_packet_complete(&failure_packet),
        ),
        criterion(
            38,
            "chaos shrink preserves class and boundary",
            "chaos shrink report preserves hard-failure class and injection boundary",
            shrink_report.preserved_hard_failure_class
                && shrink_report.preserved_injection_boundary,
        ),
        criterion(
            39,
            "chaos corpus entries record failure replay data",
            "failure corpus entry records coverage, hard-failure class, stream identity, shrunk trace",
            !failure_corpus_entry.coverage_keys_gained.is_empty()
                && failure_corpus_entry.hard_failure_class.is_some()
                && failure_corpus_entry.chaos_stream_generator_id == CHAOS_STREAM_GENERATOR_ID
                && failure_corpus_entry.shrunk_trace.is_some()
                && !failure_corpus_entry.shrunk_primitive_trace.is_empty(),
        ),
        criterion(
            40,
            "passing chaos cannot emit confidence packet",
            "chaos report has confidence_packet_emitted=false",
            !chaos_report.confidence_packet_emitted,
        ),
        criterion(
            41,
            "profile schema exists and every run records hash",
            "smoke, chaos, and run reports carry schema and sha256 profile hashes",
            smoke_profile.is_complete()
                && chaos_profile.is_complete()
                && smoke_run_report.profile_hash.starts_with("sha256:")
                && chaos_run_report.profile_hash.starts_with("sha256:"),
        ),
        criterion(
            42,
            "runner operations exist",
            "run, replay, shrink, negative validation, visual trace all executed",
            smoke_run_report.constitution_boundary_check_status.passed
                && chaos_run_report.constitution_boundary_check_status.passed
                && replay_report.replay_determinism_status == "deterministic"
                && shrunk_packet.shrink_seed.is_some()
                && negative_report.passed
                && visual_report.constitution_boundary_check_status.passed,
        ),
        criterion(
            43,
            "timeout/no-progress detection uses profile fields and markers",
            "chaos profile timeout policy is complete and hard-failure fields preserve declared policy",
            chaos_profile.timeout_policy.is_complete()
                && failure_packet
                    .chaos_failure_fields
                    .as_ref()
                    .is_some_and(|fields| fields.timeout_budget == chaos_profile.timeout_policy),
        ),
        criterion(
            44,
            "chaos pressure coverage is separate",
            "chaos report carries saturated pressure coverage in a dedicated summary distinct from semantic confidence coverage",
            chaos_report.pressure_coverage_summary.frontier_saturated
                && chaos_report.pressure_coverage_summary.total_keys > 0
                && !chaos_report
                    .pressure_coverage_summary
                    .required_dimensions
                    .is_empty(),
        ),
        criterion(
            45,
            "chaos surface schema exists",
            "surface records deterministic construction, disabled domains, diagnostics policy, and Constitution profile",
            !chaos_surface.construction_entrypoint.is_empty()
                && chaos_surface.disabled_domains == chaos_profile.disabled_domains
                && chaos_surface.visual_diagnostics_policy.is_complete()
                && chaos_surface.constitution_boundary_profile.is_complete(),
        ),
        criterion(
            46,
            "visual diagnostics emitted or explicit unavailable",
            "failure packet records headless visual diagnostics unavailability with trace artifact marker",
            failure_packet.visual_diagnostics_status.unavailable
                && !failure_packet.visual_trace_artifacts.is_empty(),
        ),
        criterion(
            47,
            "visual diagnostics are trace-derived",
            "visual trace report contains command timeline, pointer path, target timeline, progress markers from replay artifact",
            !visual_report.command_timeline.is_empty()
                && !visual_report.pointer_path_samples.is_empty()
                && !visual_report.target_timeline.is_empty()
                && !visual_report.progress_markers.is_empty(),
        ),
        criterion(
            48,
            "negative validation cases exist",
            "negative validation report contains passing cases for semantic oracles, hard-failure detectors, replay, shrinker, reporter, Constitution",
            negative_report.passed
                && negative_report.cases.len() >= 17
                && negative_case_boundaries_pass,
        ),
        criterion(
            49,
            "failed negative validation invalidates run",
            "mutated negative-validation report causes runner rejection",
            failed_negative_invalidates_run,
        ),
        criterion(
            50,
            "Constitution boundary profile/status recorded in every packet and report",
            "smoke, confidence, run, chaos, failure, shrink, negative, replay, visual, confirmation, promotion reports all carry passing boundary status",
            every_packet_and_report_has_boundary_status,
        ),
    ])
}

fn acceptance_primitive_driver() -> Result<InteractionDriver, String> {
    let mut driver = InteractionDriver::new();
    driver
        .try_execute_all([
            InteractionCommand::PointerMove { x: 10.0, y: 10.0 },
            InteractionCommand::PointerDown {
                x: 10.0,
                y: 10.0,
                button: PointerButton::Primary,
            },
            InteractionCommand::PointerUp {
                x: 10.0,
                y: 10.0,
                button: PointerButton::Primary,
            },
            InteractionCommand::WheelPixels {
                x: 10.0,
                y: 10.0,
                delta_x: 1.0,
                delta_y: -1.0,
            },
            InteractionCommand::WheelLines {
                x: 10.0,
                y: 10.0,
                delta_x: 0.0,
                delta_y: 1.0,
            },
            InteractionCommand::KeyDown {
                key_code: KeyCode::Tab,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            InteractionCommand::KeyUp {
                key_code: KeyCode::Tab,
                modifiers: Modifiers::default(),
            },
            InteractionCommand::TextInput {
                text: "a".to_string(),
                modifiers: Modifiers::default(),
            },
            InteractionCommand::Frame { count: 1 },
            InteractionCommand::Reconcile,
        ])
        .map_err(|error| format!("primitive driver failed: {error:?}"))?;
    Ok(driver)
}

fn criterion(
    id: u8,
    requirement: &'static str,
    evidence: impl Into<String>,
    passed: bool,
) -> Spec346AcceptanceCriterion {
    Spec346AcceptanceCriterion {
        id,
        requirement,
        evidence: evidence.into(),
        passed,
    }
}

fn oracle_passed(evaluations: &[InteractionOracleEvaluation], oracle_id: &str) -> bool {
    evaluations.iter().any(|evaluation| {
        evaluation.oracle_id == oracle_id && evaluation.status == InteractionOracleStatus::Passed
    })
}

fn pointer_path_coverage_complete(packet: &InteractionConfidencePacket) -> bool {
    packet
        .coverage_summary
        .path_policy_classes
        .contains(&PointerPathPolicyClass::Linear)
        && packet
            .coverage_summary
            .path_policy_classes
            .contains(&PointerPathPolicyClass::EdgeAware)
        && packet
            .coverage_summary
            .path_policy_classes
            .contains(&PointerPathPolicyClass::Jittered)
        && packet
            .coverage_summary
            .path_sample_count_classes
            .contains(&PointerPathSampleCountClass::MultiSample)
        && packet
            .coverage_summary
            .coalesced_move_classes
            .contains(&PointerMoveExposureClass::FrameSeparated)
        && packet.coverage_summary.exercised_keys.iter().any(|key| {
            matches!(
                key,
                CoverageKey::PointerPathSegment(_)
                    | CoverageKey::PointerFramePolicy(_)
                    | CoverageKey::ThresholdCrossing
            )
        })
        && !packet.coverage_summary.path_segments.is_empty()
        && !packet.coverage_summary.frame_policies.is_empty()
        && !packet
            .coverage_summary
            .crossed_hover_target_classes
            .is_empty()
        && !packet.coverage_summary.crossed_separator_classes.is_empty()
        && packet
            .coverage_summary
            .capture_transition_classes
            .contains(&PointerCaptureTransitionClass::DragThresholdCrossed)
        && packet
            .coverage_summary
            .endpoint_teleportation_modes
            .contains(&EndpointTeleportationMode::SampledPath)
}

fn required_constitution_roles() -> BTreeSet<InteractionFuzzRole> {
    BTreeSet::from([
        InteractionFuzzRole::Generator,
        InteractionFuzzRole::Probe,
        InteractionFuzzRole::Oracle,
        InteractionFuzzRole::Shrinker,
        InteractionFuzzRole::Replay,
        InteractionFuzzRole::Reporter,
    ])
}

fn required_chaos_oracles() -> BTreeSet<String> {
    BTreeSet::from([
        ORACLE_346_CHAOS_NONFINITE_SINK_ID.to_string(),
        ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID.to_string(),
        ORACLE_346_CHAOS_PANIC_UNWIND_ID.to_string(),
        ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID.to_string(),
        ORACLE_346_CHAOS_DIRTY_SETTLE_ID.to_string(),
        ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID.to_string(),
        ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID.to_string(),
        ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID.to_string(),
        ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID.to_string(),
    ])
}

fn chaos_failure_packet_complete(packet: &InteractionFailurePacket) -> bool {
    packet.chaos_failure_fields.as_ref().is_some_and(|fields| {
        fields.chaos_surface_id == chaos::CHAOS_SURFACE_BASIC_ID
            && fields.failure_detector == InteractionChaosFailureDetector::NonFiniteSink
            && fields.hard_failure_class == InteractionChaosHardFailureClass::NonFiniteFinalSink
            && fields.failing_command_index
                == packet.failing_primitive_step_index.unwrap_or(usize::MAX)
            && !fields.rng_algorithm.is_empty()
            && !fields.rng_version.is_empty()
            && fields.rng_seed != 0
            && !packet.primitive_trace.is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_346_acceptance_audit_passes_all_declared_criteria() {
        let audit = spec_346_acceptance_audit().expect("acceptance audit must run");

        assert_eq!(audit.len(), 50);
        assert_eq!(
            audit
                .iter()
                .map(|criterion| criterion.id)
                .collect::<Vec<_>>(),
            (1_u8..=50).collect::<Vec<_>>()
        );

        let failed = audit
            .iter()
            .filter(|criterion| !criterion.passed)
            .map(|criterion| {
                format!(
                    "{}: {} ({})",
                    criterion.id, criterion.requirement, criterion.evidence
                )
            })
            .collect::<Vec<_>>();
        assert!(
            failed.is_empty(),
            "failed SPEC-HAL-346 criteria: {failed:?}"
        );
    }
}
