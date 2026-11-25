//! SPEC-HAL-346 deterministic chaos/bombardment smoke runner.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use serde::{Deserialize, Serialize};

use super::constitution::{
    InteractionFuzzConstitutionCheck, InteractionFuzzForbiddenAction,
    InteractionFuzzResponsibility, InteractionFuzzRole, evaluate_constitution_boundary,
};
use crate::core::event::{KeyCode, Modifiers, PointerButton};
use crate::core::interaction_fuzz::{
    CaptureOwnerSnapshot, CoverageKey, INTERACTION_CHAOS_NO_FAILURE_CLAIM, INTERACTION_FUZZ_SCHEMA,
    INTERACTION_REPLAY_SCHEMA_VERSION, InteractionChaosCorpusEntry,
    InteractionChaosFailureDetector, InteractionChaosFailureFields,
    InteractionChaosGeneratedCommandRecord, InteractionChaosHaltReason,
    InteractionChaosHardFailureClass, InteractionChaosPointerMotionPolicy,
    InteractionChaosPointerPressurePattern, InteractionChaosPressureCoverageSummary,
    InteractionChaosPrimitiveFamily, InteractionChaosReport, InteractionChaosStreamPolicy,
    InteractionChaosSurface, InteractionCommand, InteractionConstitutionBoundaryCheckStatus,
    InteractionConstitutionBoundaryProfile, InteractionCoverageSummary, InteractionDriver,
    InteractionDriverError, InteractionFailureLayer, InteractionFailurePacket,
    InteractionFailureSeverity, InteractionFuzzMode, InteractionFuzzProfile,
    InteractionGeneratorVersionRecord, InteractionInjectionPath, InteractionOracleEvaluation,
    InteractionPrimitiveStreamRange, InteractionProgressMarker, InteractionReplayArtifact,
    InteractionSettleBoundary, InteractionStateSnapshot, InteractionTargetSnapshot,
    InteractionTimeoutPolicy, InteractionVisualDiagnosticsPolicy, InteractionVisualTraceArtifact,
    LayoutSettledSnapshot, PrimitiveCommandClass, ReplayInteractionCommand, ReplayMode,
    ReplayPrimitiveTraceStep, evaluate_capture_owner_validity, evaluate_layout_settled,
};

pub const CHAOS_SMOKE_PROFILE_ID: &str = "interaction-fuzz-chaos-smoke";
pub const CHAOS_SURFACE_BASIC_ID: &str = "CHAOS-346-BASIC-SURFACE";
pub const CHAOS_STREAM_GENERATOR_ID: &str = "CHAOS-346-PRIMITIVE-ALL";
pub const CHAOS_STREAM_GENERATOR_VERSION: &str = "1";
pub const ORACLE_346_CHAOS_NONFINITE_SINK_ID: &str = "ORACLE-346-CHAOS-NONFINITE-FINAL-SINK";
pub const ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID: &str = "ORACLE-346-CHAOS-TIMEOUT-NO-PROGRESS";
pub const ORACLE_346_CHAOS_PANIC_UNWIND_ID: &str = "ORACLE-346-CHAOS-PANIC-CRASH-UNWIND";
pub const ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID: &str = "ORACLE-346-CHAOS-CAPTURE-RELEASE-TEARDOWN";
pub const ORACLE_346_CHAOS_DIRTY_SETTLE_ID: &str = "ORACLE-346-CHAOS-DIRTY-LAYOUT-SETTLE";
pub const ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID: &str =
    "ORACLE-346-CHAOS-EVENT-ROUTER-CONTRADICTION";
pub const ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID: &str = "ORACLE-346-CHAOS-AUTHORITY-VIOLATION";
pub const ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID: &str =
    "ORACLE-346-CHAOS-REPLAY-NONDETERMINISM";
pub const ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID: &str =
    "ORACLE-346-CHAOS-UNSUPPORTED-COMMAND-OR-DOMAIN-SKIP";

const CHAOS_ROOT_SEED: u64 = 0x3460_CAA0;
const CHAOS_COMMAND_BUDGET: usize = 48;
const CHAOS_TRANSACTION_BUDGET: usize = 1;
const CHAOS_COVERAGE_WINDOW: usize = 4;
const CHAOS_RNG_ALGORITHM: &str = "halcyon-chaos-lcg";
const CHAOS_RNG_VERSION: &str = "1";
const CHAOS_ARTIFACT_DIR: &str = "target/interaction-fuzz/chaos-smoke";
const CHAOS_EXPECTED_UNSUPPORTED_DOMAINS: &[&str] = &[];
const CHAOS_CORPUS_POLICY_MEMORY_ONLY_SMOKE: &str = "memory-only-smoke";
const CHAOS_CORPUS_POLICY_PRESSURE_FRONTIER: &str = "memory-only-pressure-frontier";

#[derive(Debug, Clone)]
struct GeneratedChaosCommand {
    command: crate::core::interaction_fuzz::InteractionCommand,
    pressure_patterns: Vec<InteractionChaosPointerPressurePattern>,
    pointer_motion_policy: Option<InteractionChaosPointerMotionPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionChaosFailure {
    IncompleteProfile,
    NotChaosFailurePacket,
    MissingPrimitiveTrace,
    ShrinkDidNotPreserveFailure,
    Driver(InteractionDriverError),
}

impl From<InteractionDriverError> for InteractionChaosFailure {
    fn from(value: InteractionDriverError) -> Self {
        Self::Driver(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionChaosRunOutcome {
    ChaosReport(InteractionChaosReport),
    FailurePacket(Box<InteractionFailurePacket>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionChaosShrinkReport {
    pub schema_version: u32,
    pub schema: String,
    pub failure_id: String,
    pub shrink_seed: u64,
    pub replay_mode: ReplayMode,
    pub injection_path: InteractionInjectionPath,
    pub failure_detector: InteractionChaosFailureDetector,
    pub hard_failure_class: InteractionChaosHardFailureClass,
    pub original_primitive_count: usize,
    pub shrunk_primitive_count: usize,
    pub original_failing_command_index: usize,
    pub shrunk_failing_command_index: usize,
    pub candidate_count: usize,
    pub accepted_reduction_count: usize,
    pub preserved_injection_boundary: bool,
    pub preserved_hard_failure_class: bool,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub shrunk_trace: InteractionReplayArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DetectedChaosFailure {
    detector: InteractionChaosFailureDetector,
    hard_failure_class: InteractionChaosHardFailureClass,
    command_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimeoutNoProgressDetection {
    command_index: usize,
    elapsed_run_ms: u64,
    elapsed_transaction_ms: u64,
    failed_requirement: &'static str,
    failed_value: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CaptureTeardownDetection {
    command_index: usize,
    capture_owner_class: String,
    evaluation: InteractionOracleEvaluation,
}

#[derive(Debug, Clone, PartialEq)]
struct DirtySettleDetection {
    command_index: usize,
    dirty_state_hash: String,
    evaluation: InteractionOracleEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouterContradictionDetection {
    command_index: usize,
    capture_owner_class: String,
    routed_target_class: String,
    failed_requirement: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct AuthorityViolationDetection {
    command_index: usize,
    authority_domain: String,
    violation_class: String,
    evaluation: InteractionOracleEvaluation,
}

#[derive(Debug, Clone, PartialEq)]
struct ReplayNondeterminismDetection {
    command_index: usize,
    divergence_class: String,
    expected_trace_fingerprint: String,
    replayed_trace_fingerprint: String,
    replay_error: Option<String>,
    replayed_trace: InteractionReplayArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnsupportedCommandOrDomainSkipDetection {
    command_index: usize,
    unsupported_class: String,
    command_family: String,
    command_domain: String,
    failed_requirement: &'static str,
}

pub fn run_ci_chaos_smoke_profile() -> Result<InteractionChaosRunOutcome, InteractionChaosFailure> {
    let surface = ci_chaos_surface();
    let profile = ci_chaos_profile(&surface);
    run_chaos_smoke_profile(profile, surface)
}

pub fn run_chaos_smoke_profile(
    profile: InteractionFuzzProfile,
    surface: InteractionChaosSurface,
) -> Result<InteractionChaosRunOutcome, InteractionChaosFailure> {
    let commands = deterministic_chaos_stream(profile.root_seed, profile.command_budget);
    run_chaos_smoke_profile_with_commands(profile, surface, commands)
}

pub(crate) fn synthetic_non_finite_chaos_failure_packet()
-> (Box<InteractionFailurePacket>, usize, usize) {
    let surface = ci_chaos_surface();
    let profile = ci_chaos_profile(&surface);
    let mut commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, CHAOS_COMMAND_BUDGET);
    let failing_index = 3;
    commands[failing_index] = GeneratedChaosCommand {
        command: crate::core::interaction_fuzz::InteractionCommand::PointerMove {
            x: f32::NAN,
            y: 1.0,
        },
        pressure_patterns: vec![InteractionChaosPointerPressurePattern::RandomWalk],
        pointer_motion_policy: Some(InteractionChaosPointerMotionPolicy::RandomWalkContinuous),
    };
    let expected_stream_len = commands.len();

    let outcome = run_chaos_smoke_profile_with_commands(profile, surface, commands)
        .expect("chaos failure must be reported as packet");

    let InteractionChaosRunOutcome::FailurePacket(packet) = outcome else {
        panic!("expected failure packet");
    };

    (packet, failing_index, expected_stream_len)
}

fn run_chaos_smoke_profile_with_commands(
    profile: InteractionFuzzProfile,
    surface: InteractionChaosSurface,
    mut commands: Vec<GeneratedChaosCommand>,
) -> Result<InteractionChaosRunOutcome, InteractionChaosFailure> {
    if !profile.is_complete() {
        return Err(InteractionChaosFailure::IncompleteProfile);
    }

    commands.truncate(profile.command_budget.min(surface.command_budget));
    let generated_primitive_commands = replay_commands(&commands);

    let mut driver = InteractionDriver::new();
    let mut progress_marker_history = Vec::new();
    let mut executed_command_count = 0;
    let mut halt_reason = InteractionChaosHaltReason::CommandBudget;
    let mut seen_pressure_keys = BTreeSet::new();
    let mut no_new_coverage_count = 0;
    for (command_index, generated) in commands.iter().enumerate() {
        progress_marker_history.push(chaos_progress_marker(command_index, &driver));
        if let Some(detection) =
            detect_unsupported_command_or_domain_skip(&profile, &surface, command_index, generated)
        {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_unsupported_command_or_domain_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        if let Some(detection) = detect_timeout_or_no_progress(
            &profile.timeout_policy,
            command_index,
            &progress_marker_history,
        ) {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_timeout_no_progress_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        if let Some(detection) = detect_capture_teardown_failure(&progress_marker_history) {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_capture_teardown_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        if let Some(detection) =
            detect_dirty_settle_boundary_failure(profile.settle_boundary, &progress_marker_history)
        {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_dirty_settle_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        if let Some(detection) = detect_router_contradiction_failure(&progress_marker_history) {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_router_contradiction_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        if let Some(detection) = detect_authority_violation_failure(&progress_marker_history) {
            return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                build_chaos_authority_violation_failure_packet(
                    &profile,
                    &surface,
                    &commands,
                    &generated_primitive_commands,
                    &driver,
                    &progress_marker_history,
                    detection,
                ),
            )));
        }

        let execution = panic::catch_unwind(AssertUnwindSafe(|| {
            driver.try_execute(generated.command.clone())
        }));
        match execution {
            Ok(Ok(_events)) => {}
            Ok(Err(error)) => {
                return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                    build_chaos_non_finite_failure_packet(
                        &profile,
                        &surface,
                        &commands,
                        &generated_primitive_commands,
                        &driver,
                        command_index,
                        error,
                    ),
                )));
            }
            Err(payload) => {
                return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
                    build_chaos_panic_failure_packet(
                        &profile,
                        &surface,
                        &commands,
                        &generated_primitive_commands,
                        &driver,
                        &progress_marker_history,
                        command_index,
                        panic_payload_label(payload.as_ref()),
                    ),
                )));
            }
        }

        executed_command_count = command_index + 1;
        let new_pressure_key_count =
            record_chaos_command_pressure_keys(&mut seen_pressure_keys, generated);
        if chaos_pressure_frontier_saturated(&seen_pressure_keys) {
            if chaos_corpus_policy_saturates_on_pressure_frontier(&surface) {
                halt_reason = InteractionChaosHaltReason::CorpusSaturation;
                break;
            }

            if new_pressure_key_count == 0 {
                no_new_coverage_count += 1;
            } else {
                no_new_coverage_count = 0;
            }

            if no_new_coverage_count >= profile.coverage_window {
                halt_reason = InteractionChaosHaltReason::NoNewCoverageWindow;
                break;
            }
        } else {
            no_new_coverage_count = 0;
        }
    }

    let executed_commands = &commands[..executed_command_count];
    let generated_command_records = generated_command_records(executed_commands);
    let executed_generated_primitive_commands = replay_commands(executed_commands);
    let primitive_family_set = primitive_family_set(executed_commands);
    let pointer_pressure_patterns = pointer_pressure_patterns(executed_commands);
    let pointer_motion_policies = pointer_motion_policies(executed_commands);

    let replay = InteractionReplayArtifact::from_driver(
        ReplayMode::ExactPrimitive,
        surface.id.as_str(),
        profile.root_seed,
        None,
        &driver,
    );
    if let Some(detection) = detect_replay_nondeterminism_failure(&replay) {
        return Ok(InteractionChaosRunOutcome::FailurePacket(Box::new(
            build_chaos_replay_nondeterminism_failure_packet(
                &profile,
                &surface,
                executed_commands,
                &executed_generated_primitive_commands,
                &progress_marker_history,
                &replay,
                detection,
            ),
        )));
    }
    let coverage_summary =
        InteractionCoverageSummary::from_coverage(driver.coverage(), Vec::<String>::new());
    let pressure_coverage_summary = chaos_pressure_coverage_summary(
        &profile,
        &surface,
        executed_commands,
        &primitive_family_set,
        &pointer_pressure_patterns,
        &pointer_motion_policies,
    );
    let corpus_deltas = chaos_report_corpus_deltas(
        &profile,
        &surface,
        &primitive_family_set,
        &pointer_pressure_patterns,
        &pointer_motion_policies,
        &replay.primitive_trace,
    );
    let constitution_boundary_check_status = InteractionConstitutionBoundaryCheckStatus {
        passed: true,
        failed_checks: Vec::new(),
    };

    Ok(InteractionChaosRunOutcome::ChaosReport(
        InteractionChaosReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_id: profile.profile_id.clone(),
            run_profile_id: profile.profile_id.clone(),
            run_profile_hash: profile.profile_hash(),
            chaos_mode: profile.mode,
            surface_set: profile.chaos_surface_ids.clone(),
            seed_set: profile.seed_set.clone(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            chaos_stream_generator_ids: profile.chaos_stream_generator_ids.clone(),
            chaos_stream_generator_versions: profile.chaos_stream_generator_versions.clone(),
            stream_policy: chaos_stream_policy(),
            executed_command_budget: executed_generated_primitive_commands.len(),
            executed_transaction_budget: profile.transaction_budget,
            generated_command_records,
            generated_primitive_commands: executed_generated_primitive_commands,
            primitive_trace: replay.primitive_trace,
            primitive_family_set,
            pointer_pressure_patterns,
            pointer_motion_policies,
            hard_failure_oracle_set: profile.hard_failure_oracle_ids.clone(),
            coverage_summary,
            pressure_coverage_summary,
            corpus_deltas,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
            halt_reason,
            failure_count: 0,
            confidence_packet_emitted: false,
            visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status,
            no_failure_claim: INTERACTION_CHAOS_NO_FAILURE_CLAIM.to_string(),
        },
    ))
}

fn build_chaos_non_finite_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    failing_command_index: usize,
    error: InteractionDriverError,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert("driver_error".to_string(), format!("{error:?}"));
    observed_fields.insert(
        "failing_command_index".to_string(),
        failing_command_index.to_string(),
    );
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-hard-failure".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!("{}-{failing_command_index}", profile.profile_id),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_NONFINITE_SINK_ID.to_string(),
        invariant_name: "Non-Finite Final Sink Rejection".to_string(),
        severity: InteractionFailureSeverity::P0,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_failure_packet_records_non_finite_sink_with_replayable_stream"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(failing_command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(failing_command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: None,
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "no-hard-failure".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::PrimitiveInjection,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "InteractionDriver".to_string(),
            ORACLE_346_CHAOS_NONFINITE_SINK_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: vec![InteractionProgressMarker {
            command_index: failing_command_index,
            frame_index: driver.primitive_trace().len() as u64,
            layout_epoch: 0,
            scene_epoch: 0,
            dirty_state_hash: "not_observed".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "not_observed".to_string(),
            focus_owner_class: "not_observed".to_string(),
            routed_target_class: "raw_input_fusion".to_string(),
        }],
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_NONFINITE_SINK_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index,
            failure_detector: InteractionChaosFailureDetector::NonFiniteSink,
            hard_failure_class: InteractionChaosHardFailureClass::NonFiniteFinalSink,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_timeout_no_progress_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: TimeoutNoProgressDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert(
        "failure_detector".to_string(),
        "timeout_or_no_progress".to_string(),
    );
    observed_fields.insert(
        "failed_requirement".to_string(),
        detection.failed_requirement.to_string(),
    );
    observed_fields.insert("failed_value".to_string(), detection.failed_value.clone());
    observed_fields.insert(
        "elapsed_run_ms".to_string(),
        detection.elapsed_run_ms.to_string(),
    );
    observed_fields.insert(
        "elapsed_transaction_ms".to_string(),
        detection.elapsed_transaction_ms.to_string(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-timeout-no-progress".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!("{}-timeout-{}", profile.profile_id, detection.command_index),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID.to_string(),
        invariant_name: "Declared Timeout and No-Progress Detection".to_string(),
        severity: InteractionFailureSeverity::P0,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_timeout_failure_packet_uses_declared_policy_and_progress_markers"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: None,
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "within-declared-timeout-budget".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::Oracle,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "timeout_no_progress_detector".to_string(),
            ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::TimeoutOrNoProgress,
            hard_failure_class: InteractionChaosHardFailureClass::TimeoutHangOrNoProgress,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_panic_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    failing_command_index: usize,
    panic_payload: String,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert(
        "failure_detector".to_string(),
        "panic_crash_or_unwind".to_string(),
    );
    observed_fields.insert("panic_payload".to_string(), panic_payload);
    observed_fields.insert(
        "failing_command_index".to_string(),
        failing_command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_panic".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-panic-crash-unwind".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!("{}-panic-{}", profile.profile_id, failing_command_index),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_PANIC_UNWIND_ID.to_string(),
        invariant_name: "Panic Crash or Unwind Boundary".to_string(),
        severity: InteractionFailureSeverity::P0,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_panic_failure_packet_records_unwind_boundary_without_escape"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(failing_command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(failing_command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: None,
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "no-panic-crash-or-unwind".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::PrimitiveInjection,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "panic_unwind_detector".to_string(),
            ORACLE_346_CHAOS_PANIC_UNWIND_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_PANIC_UNWIND_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index,
            failure_detector: InteractionChaosFailureDetector::PanicCrashOrUnwind,
            hard_failure_class: InteractionChaosHardFailureClass::PanicCrashOrUnwind,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_capture_teardown_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: CaptureTeardownDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = detection.evaluation.derived_values.clone();
    observed_fields.insert(
        "failure_detector".to_string(),
        "capture_survived_release_or_teardown".to_string(),
    );
    observed_fields.insert(
        "capture_owner_class".to_string(),
        detection.capture_owner_class.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_detection".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    if let Some(requirement) = detection.evaluation.failed_requirement.clone() {
        observed_fields.insert("failed_requirement".to_string(), requirement);
    }
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-capture-release-teardown".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!("{}-capture-{}", profile.profile_id, detection.command_index),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID.to_string(),
        invariant_name: "Capture Release or Teardown Boundary".to_string(),
        severity: InteractionFailureSeverity::P1,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_capture_teardown_failure_packet_records_progress_marker_leak"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: detection.evaluation.failed_subject.clone(),
            target_kind: "capture_owner".to_string(),
            target_class: Some(detection.capture_owner_class.clone()),
        }),
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "capture-released-or-cleaned-up".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::EventRouting,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "capture_teardown_detector".to_string(),
            ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID.to_string(),
            detection.evaluation.oracle_id,
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::CaptureSurvivedReleaseOrTeardown,
            hard_failure_class: InteractionChaosHardFailureClass::PointerCaptureLeak,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_dirty_settle_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: DirtySettleDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = detection.evaluation.derived_values.clone();
    observed_fields.insert(
        "failure_detector".to_string(),
        "dirty_layout_at_settle_boundary".to_string(),
    );
    observed_fields.insert(
        "dirty_state_hash".to_string(),
        detection.dirty_state_hash.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_detection".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    if let Some(requirement) = detection.evaluation.failed_requirement.clone() {
        observed_fields.insert("failed_requirement".to_string(), requirement);
    }
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-dirty-layout-settle".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!(
            "{}-dirty-settle-{}",
            profile.profile_id, detection.command_index
        ),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_DIRTY_SETTLE_ID.to_string(),
        invariant_name: "Dirty Layout at Settle Boundary".to_string(),
        severity: InteractionFailureSeverity::P1,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_dirty_settle_failure_packet_records_deferred_mutation_marker"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: detection.evaluation.failed_subject.clone(),
            target_kind: "settle_boundary".to_string(),
            target_class: Some(format!("{:?}", profile.settle_boundary)),
        }),
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "layout-clean-at-settle-boundary".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::Layout,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "dirty_settle_detector".to_string(),
            ORACLE_346_CHAOS_DIRTY_SETTLE_ID.to_string(),
            detection.evaluation.oracle_id,
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_DIRTY_SETTLE_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::DirtyLayoutAtSettleBoundary,
            hard_failure_class: InteractionChaosHardFailureClass::DirtyLayoutAtSettleBoundary,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_router_contradiction_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: RouterContradictionDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert(
        "failure_detector".to_string(),
        "event_router_contradiction".to_string(),
    );
    observed_fields.insert(
        "failed_requirement".to_string(),
        detection.failed_requirement.to_string(),
    );
    observed_fields.insert(
        "capture_owner_class".to_string(),
        detection.capture_owner_class.clone(),
    );
    observed_fields.insert(
        "routed_target_class".to_string(),
        detection.routed_target_class.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_detection".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-event-router-contradiction".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!("{}-router-{}", profile.profile_id, detection.command_index),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID.to_string(),
        invariant_name: "Event Router Contradiction".to_string(),
        severity: InteractionFailureSeverity::P1,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_router_contradiction_failure_packet_records_divergent_route"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: Some(detection.routed_target_class.clone()),
            target_kind: "routed_target".to_string(),
            target_class: Some(detection.routed_target_class.clone()),
        }),
        incidental_target: Some(InteractionTargetSnapshot {
            target_id: Some(detection.capture_owner_class.clone()),
            target_kind: "capture_owner".to_string(),
            target_class: Some(detection.capture_owner_class.clone()),
        }),
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "captured-route-target-coherent".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::EventRouting,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "router_contradiction_detector".to_string(),
            ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::EventRouterContradiction,
            hard_failure_class: InteractionChaosHardFailureClass::EventRouterContradiction,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_authority_violation_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: AuthorityViolationDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let mut observed_fields = detection.evaluation.derived_values.clone();
    observed_fields.insert(
        "failure_detector".to_string(),
        "authority_violation".to_string(),
    );
    observed_fields.insert(
        "authority_domain".to_string(),
        detection.authority_domain.clone(),
    );
    observed_fields.insert(
        "violation_class".to_string(),
        detection.violation_class.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_detection".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    if let Some(requirement) = detection.evaluation.failed_requirement.clone() {
        observed_fields.insert("failed_requirement".to_string(), requirement);
    }
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-authority-violation".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!(
            "{}-authority-{}",
            profile.profile_id, detection.command_index
        ),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID.to_string(),
        invariant_name: "Authority Boundary Violation".to_string(),
        severity: InteractionFailureSeverity::P1,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_authority_violation_failure_packet_records_constitution_evidence"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: Some(detection.authority_domain.clone()),
            target_kind: "authority_domain".to_string(),
            target_class: Some(detection.authority_domain.clone()),
        }),
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "constitution-boundary-preserved".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::Oracle,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "authority_violation_detector".to_string(),
            ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID.to_string(),
            detection.evaluation.oracle_id,
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: false,
            failed_checks: vec![detection.violation_class.clone()],
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::AuthorityViolation,
            hard_failure_class: InteractionChaosHardFailureClass::AuthorityViolation,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_replay_nondeterminism_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    progress_marker_history: &[InteractionProgressMarker],
    expected_replay: &InteractionReplayArtifact,
    detection: ReplayNondeterminismDetection,
) -> InteractionFailurePacket {
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert(
        "failure_detector".to_string(),
        "replay_nondeterminism".to_string(),
    );
    observed_fields.insert(
        "divergence_class".to_string(),
        detection.divergence_class.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "expected_trace_fingerprint".to_string(),
        detection.expected_trace_fingerprint.clone(),
    );
    observed_fields.insert(
        "replayed_trace_fingerprint".to_string(),
        detection.replayed_trace_fingerprint.clone(),
    );
    observed_fields.insert(
        "expected_primitive_count".to_string(),
        expected_replay.primitive_trace.len().to_string(),
    );
    observed_fields.insert(
        "replayed_primitive_count".to_string(),
        detection.replayed_trace.primitive_trace.len().to_string(),
    );
    if let Some(replay_error) = detection.replay_error.clone() {
        observed_fields.insert("replay_error".to_string(), replay_error);
    }
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-replay-nondeterminism".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!(
            "{}-replay-nondeterminism-{}",
            profile.profile_id, detection.command_index
        ),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID.to_string(),
        invariant_name: "Replay Nondeterminism".to_string(),
        severity: InteractionFailureSeverity::P0,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command:
            "cargo test chaos_replay_nondeterminism_failure_packet_records_artifact_divergence"
                .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: Some(expected_replay.fixture_id.clone()),
            target_kind: "replay_artifact".to_string(),
            target_class: Some("exact_primitive_replay".to_string()),
        }),
        incidental_target: None,
        path_policy: None,
        intent_trace: expected_replay.intent_trace.clone(),
        primitive_trace: expected_replay.primitive_trace.clone(),
        shrunk_trace: expected_replay.clone(),
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "deterministic-replay".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::Replay,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "interaction_replay".to_string(),
            "replay_nondeterminism_detector".to_string(),
            ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::ReplayNondeterminism,
            hard_failure_class: InteractionChaosHardFailureClass::ReplayNondeterminism,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "failed".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: profile.expected_unsupported_domains.clone(),
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn build_chaos_unsupported_command_or_domain_failure_packet(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    generated_primitive_commands: &[ReplayInteractionCommand],
    driver: &InteractionDriver,
    progress_marker_history: &[InteractionProgressMarker],
    detection: UnsupportedCommandOrDomainSkipDetection,
) -> InteractionFailurePacket {
    let primitive_trace = replay_primitive_stream(generated_primitive_commands);
    let unsupported_domain_report = if detection.unsupported_class == "primitive_family" {
        format!("primitive_family:{}", detection.command_family)
    } else {
        detection.command_domain.clone()
    };
    let mut observed_fields = BTreeMap::new();
    observed_fields.insert(
        "failure_detector".to_string(),
        "unsupported_command_or_domain_skip".to_string(),
    );
    observed_fields.insert(
        "failed_requirement".to_string(),
        detection.failed_requirement.to_string(),
    );
    observed_fields.insert(
        "unsupported_class".to_string(),
        detection.unsupported_class.clone(),
    );
    observed_fields.insert(
        "command_family".to_string(),
        detection.command_family.clone(),
    );
    observed_fields.insert(
        "command_domain".to_string(),
        detection.command_domain.clone(),
    );
    observed_fields.insert(
        "failing_command_index".to_string(),
        detection.command_index.to_string(),
    );
    observed_fields.insert(
        "executed_primitive_count_before_detection".to_string(),
        driver.primitive_trace().len().to_string(),
    );
    let observed_state = InteractionStateSnapshot {
        state_id: "chaos-unsupported-command-or-domain-skip".to_string(),
        fields: observed_fields,
    };

    InteractionFailurePacket {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: format!(
            "{}-unsupported-skip-{}",
            profile.profile_id, detection.command_index
        ),
        run_profile_id: profile.profile_id.clone(),
        run_profile_hash: profile.profile_hash(),
        invariant_id: ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID.to_string(),
        invariant_name: "Unsupported Command or Domain Skip Reporting".to_string(),
        severity: InteractionFailureSeverity::P1,
        fixture_id: surface.id.clone(),
        root_seed: profile.root_seed,
        shrink_seed: None,
        replay_command: "cargo test chaos_unsupported_domain_failure_packet_records_skip_evidence"
            .to_string(),
        injection_path: profile.injection_path,
        settle_boundary: profile.settle_boundary,
        failing_intent_index: None,
        failing_transaction_index: Some(0),
        failing_primitive_step_index: Some(detection.command_index),
        active_intent_command: None,
        command_that_failed: generated_primitive_commands
            .get(detection.command_index)
            .cloned(),
        intended_target: Some(InteractionTargetSnapshot {
            target_id: Some(surface.id.clone()),
            target_kind: "chaos_surface".to_string(),
            target_class: Some(surface.root_surface_kind.clone()),
        }),
        resolved_target: Some(InteractionTargetSnapshot {
            target_id: Some(detection.command_domain.clone()),
            target_kind: "command_domain".to_string(),
            target_class: Some(detection.command_domain.clone()),
        }),
        incidental_target: None,
        path_policy: None,
        intent_trace: Vec::new(),
        primitive_trace: primitive_trace.clone(),
        shrunk_trace: InteractionReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            replay_mode: ReplayMode::ExactPrimitive,
            fixture_id: surface.id.clone(),
            root_seed: profile.root_seed,
            shrink_seed: None,
            intent_trace: Vec::new(),
            primitive_trace,
            coverage_keys: Vec::new(),
        },
        unshrunk_trace_reference: None,
        pre_state: surface.initial_state_snapshot.clone(),
        post_state: observed_state.clone(),
        expected_state: InteractionStateSnapshot {
            state_id: "unsupported-command-domain-reported".to_string(),
            fields: BTreeMap::new(),
        },
        observed_state: observed_state.clone(),
        probe_snapshots: Vec::new(),
        layer_classification: InteractionFailureLayer::PrimitiveInjection,
        ownership_chain: vec![
            CHAOS_STREAM_GENERATOR_ID.to_string(),
            "unsupported_command_or_domain_skip_detector".to_string(),
            ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID.to_string(),
        ],
        coverage_key: None,
        timeout_policy: profile.timeout_policy.clone(),
        progress_marker_history: progress_marker_history.to_vec(),
        visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
        visual_trace_artifacts: chaos_visual_trace_artifacts(
            profile,
            ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID,
        ),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus {
            passed: true,
            failed_checks: Vec::new(),
        },
        chaos_failure_fields: Some(InteractionChaosFailureFields {
            chaos_surface_id: surface.id.clone(),
            chaos_surface_state_id: surface.initial_state_snapshot.state_id.clone(),
            chaos_mode: profile.mode,
            chaos_stream_generator: CHAOS_STREAM_GENERATOR_ID.to_string(),
            chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
            chaos_stream_policy: chaos_stream_policy(),
            rng_algorithm: profile.rng_algorithm.clone(),
            rng_version: profile.rng_version.clone(),
            rng_seed: profile.root_seed,
            primitive_stream_range: InteractionPrimitiveStreamRange {
                start: 0,
                end: commands.len(),
            },
            failing_command_index: detection.command_index,
            failure_detector: InteractionChaosFailureDetector::UnsupportedCommandOrDomainSkip,
            hard_failure_class: InteractionChaosHardFailureClass::UnsupportedDomainSkip,
            timeout_budget: profile.timeout_policy.clone(),
            replay_determinism_status: "not_run".to_string(),
            initial_state_snapshot: surface.initial_state_snapshot.clone(),
            final_state_snapshot: observed_state,
            unsupported_domains: vec![unsupported_domain_report],
        }),
        related_specs: vec!["SPEC-HAL-346".to_string()],
        promotion_target: Some("src/tests/interaction_fuzz".to_string()),
    }
}

fn panic_payload_label(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non_string_panic_payload".to_string()
    }
}

fn chaos_visual_trace_artifacts(
    profile: &InteractionFuzzProfile,
    detector_id: &str,
) -> Vec<InteractionVisualTraceArtifact> {
    let status = profile.visual_diagnostics_policy.status();
    if status.unavailable {
        vec![InteractionVisualTraceArtifact {
            artifact_kind: "visual_diagnostics_unavailable".to_string(),
            path: format!(
                "unavailable:{}",
                status.reason.unwrap_or_else(|| "unspecified".to_string())
            ),
            detector_id: detector_id.to_string(),
        }]
    } else {
        vec![InteractionVisualTraceArtifact {
            artifact_kind: "visual_trace_json".to_string(),
            path: format!(
                "{}/diagnostics/visual-trace.json",
                profile.artifact_dir.trim_end_matches('/')
            ),
            detector_id: detector_id.to_string(),
        }]
    }
}

pub(crate) fn shrink_chaos_failure_packet(
    packet: &InteractionFailurePacket,
) -> Result<InteractionChaosShrinkReport, InteractionChaosFailure> {
    let fields = packet
        .chaos_failure_fields
        .as_ref()
        .ok_or(InteractionChaosFailure::NotChaosFailurePacket)?;
    let mut commands: Vec<ReplayInteractionCommand> = packet
        .shrunk_trace
        .primitive_trace
        .iter()
        .map(|step| step.command.clone())
        .collect();
    if commands.is_empty() {
        return Err(InteractionChaosFailure::MissingPrimitiveTrace);
    }

    let original_primitive_count = commands.len();
    let original_detection = detect_chaos_failure(&commands)
        .filter(|detected| preserves_chaos_failure(fields, *detected))
        .ok_or(InteractionChaosFailure::ShrinkDidNotPreserveFailure)?;
    let mut candidate_count = 0;
    let mut accepted_reduction_count = 0;
    let mut chunk_size = commands.len().saturating_div(2).max(1);

    while commands.len() > 1 {
        let mut changed = false;
        let mut start = 0;

        while start < commands.len() && commands.len() > 1 {
            let end = (start + chunk_size).min(commands.len());
            if end <= start || end - start == commands.len() {
                break;
            }

            let mut candidate = commands.clone();
            candidate.drain(start..end);
            candidate_count += 1;

            if detect_chaos_failure(&candidate)
                .filter(|detected| preserves_chaos_failure(fields, *detected))
                .is_some()
            {
                commands = candidate;
                accepted_reduction_count += 1;
                changed = true;
            } else {
                start += chunk_size;
            }
        }

        if changed {
            chunk_size = chunk_size.min(commands.len().saturating_sub(1).max(1));
        } else if chunk_size == 1 {
            break;
        } else {
            chunk_size = (chunk_size / 2).max(1);
        }
    }

    let shrunk_detection = detect_chaos_failure(&commands)
        .filter(|detected| preserves_chaos_failure(fields, *detected))
        .ok_or(InteractionChaosFailure::ShrinkDidNotPreserveFailure)?;
    let shrink_seed = packet.root_seed ^ 0x3460_5A11;
    let shrunk_trace = InteractionReplayArtifact {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        replay_mode: ReplayMode::ShrunkTrace,
        fixture_id: packet.fixture_id.clone(),
        root_seed: packet.root_seed,
        shrink_seed: Some(shrink_seed),
        intent_trace: Vec::new(),
        primitive_trace: replay_primitive_stream(&commands),
        coverage_keys: packet.shrunk_trace.coverage_keys.clone(),
    };

    Ok(InteractionChaosShrinkReport {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        failure_id: packet.failure_id.clone(),
        shrink_seed,
        replay_mode: ReplayMode::ShrunkTrace,
        injection_path: packet.injection_path,
        failure_detector: fields.failure_detector,
        hard_failure_class: fields.hard_failure_class,
        original_primitive_count,
        shrunk_primitive_count: commands.len(),
        original_failing_command_index: original_detection.command_index,
        shrunk_failing_command_index: shrunk_detection.command_index,
        candidate_count,
        accepted_reduction_count,
        preserved_injection_boundary: packet.injection_path
            == InteractionInjectionPath::RawInputFusion,
        preserved_hard_failure_class: shrunk_detection.hard_failure_class
            == fields.hard_failure_class,
        constitution_boundary_profile: packet.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: shrinker_constitution_boundary_check_status(),
        shrunk_trace,
    })
}

fn shrinker_constitution_boundary_check_status() -> InteractionConstitutionBoundaryCheckStatus {
    let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
        role: InteractionFuzzRole::Shrinker,
        responsibilities: vec![InteractionFuzzResponsibility::MinimizeTrace],
        forbidden_actions: Vec::new(),
        boundary_narrowed: false,
        full_confidence_claim: false,
        authority: "interaction_shrinker",
    });
    InteractionConstitutionBoundaryCheckStatus::from_oracle_evaluations(&[evaluation])
}

fn detect_chaos_failure(commands: &[ReplayInteractionCommand]) -> Option<DetectedChaosFailure> {
    let mut driver = InteractionDriver::new();
    for (command_index, command) in commands.iter().enumerate() {
        let command = crate::core::interaction_fuzz::InteractionCommand::from(command);
        if let Err(error) = driver.try_execute(command) {
            return match error {
                InteractionDriverError::NonFiniteSinkValue { .. } => Some(DetectedChaosFailure {
                    detector: InteractionChaosFailureDetector::NonFiniteSink,
                    hard_failure_class: InteractionChaosHardFailureClass::NonFiniteFinalSink,
                    command_index,
                }),
            };
        }
    }

    None
}

fn detect_unsupported_command_or_domain_skip(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    command_index: usize,
    generated: &GeneratedChaosCommand,
) -> Option<UnsupportedCommandOrDomainSkipDetection> {
    let family = InteractionChaosPrimitiveFamily::from_command(&generated.command);
    let command_family = chaos_primitive_family_key(family).to_string();
    let command_domain = chaos_command_domain(&generated.command).to_string();

    if !surface.allowed_primitive_families.contains(&family) {
        return Some(UnsupportedCommandOrDomainSkipDetection {
            command_index,
            unsupported_class: "primitive_family".to_string(),
            command_family,
            command_domain,
            failed_requirement: "allowed_primitive_family",
        });
    }

    if profile
        .disabled_domains
        .iter()
        .chain(surface.disabled_domains.iter())
        .any(|domain| domain == &command_domain)
    {
        return Some(UnsupportedCommandOrDomainSkipDetection {
            command_index,
            unsupported_class: "disabled_domain".to_string(),
            command_family,
            command_domain,
            failed_requirement: "no_disabled_domain_command",
        });
    }

    None
}

fn detect_timeout_or_no_progress(
    policy: &InteractionTimeoutPolicy,
    command_index: usize,
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<TimeoutNoProgressDetection> {
    let elapsed_run_ms = deterministic_elapsed_ms(command_index);
    let elapsed_transaction_ms = elapsed_run_ms;

    if elapsed_transaction_ms > policy.per_transaction_timeout_ms {
        return Some(TimeoutNoProgressDetection {
            command_index,
            elapsed_run_ms,
            elapsed_transaction_ms,
            failed_requirement: "per_transaction_timeout_ms",
            failed_value: elapsed_transaction_ms.to_string(),
        });
    }

    if elapsed_run_ms > policy.run_timeout_ms {
        return Some(TimeoutNoProgressDetection {
            command_index,
            elapsed_run_ms,
            elapsed_transaction_ms,
            failed_requirement: "run_timeout_ms",
            failed_value: elapsed_run_ms.to_string(),
        });
    }

    if no_progress_frames_exhausted(policy, progress_marker_history) {
        return Some(TimeoutNoProgressDetection {
            command_index,
            elapsed_run_ms,
            elapsed_transaction_ms,
            failed_requirement: "no_progress_frame_budget",
            failed_value: policy.no_progress_frame_budget.to_string(),
        });
    }

    if no_progress_settle_exhausted(policy, progress_marker_history) {
        return Some(TimeoutNoProgressDetection {
            command_index,
            elapsed_run_ms,
            elapsed_transaction_ms,
            failed_requirement: "no_progress_settle_budget",
            failed_value: policy.no_progress_settle_budget.to_string(),
        });
    }

    if let Some(settle_attempt_count) = settle_budget_exhausted(policy, progress_marker_history) {
        return Some(TimeoutNoProgressDetection {
            command_index,
            elapsed_run_ms,
            elapsed_transaction_ms,
            failed_requirement: "settle_iteration_budget",
            failed_value: settle_attempt_count.to_string(),
        });
    }

    None
}

fn deterministic_elapsed_ms(command_index: usize) -> u64 {
    command_index as u64 + 1
}

fn no_progress_frames_exhausted(
    policy: &InteractionTimeoutPolicy,
    progress_marker_history: &[InteractionProgressMarker],
) -> bool {
    no_progress_budget_exhausted(
        progress_marker_history,
        policy.no_progress_frame_budget as usize,
    )
}

fn no_progress_settle_exhausted(
    policy: &InteractionTimeoutPolicy,
    progress_marker_history: &[InteractionProgressMarker],
) -> bool {
    no_progress_budget_exhausted(
        progress_marker_history,
        policy.no_progress_settle_budget as usize,
    )
}

fn settle_budget_exhausted(
    policy: &InteractionTimeoutPolicy,
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<usize> {
    let settle_attempt_count = progress_marker_history
        .iter()
        .rev()
        .take_while(|marker| {
            marker.deferred_mutation_count > 0 || marker.dirty_state_hash.starts_with("dirty:")
        })
        .count();

    (settle_attempt_count > policy.settle_iteration_budget as usize).then_some(settle_attempt_count)
}

fn no_progress_budget_exhausted(
    progress_marker_history: &[InteractionProgressMarker],
    budget: usize,
) -> bool {
    if budget == 0 || progress_marker_history.len() < budget {
        return false;
    }

    let window = &progress_marker_history[progress_marker_history.len() - budget..];
    window
        .first()
        .is_some_and(|first| window.iter().all(|marker| marker == first))
}

fn detect_capture_teardown_failure(
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<CaptureTeardownDetection> {
    progress_marker_history.iter().find_map(|marker| {
        let owner_id = marker.capture_owner_class.strip_prefix("leaked:")?;
        let evaluation = evaluate_capture_owner_validity(CaptureOwnerSnapshot::new(
            true,
            Some(owner_id.to_string()),
            true,
            true,
            true,
            true,
            false,
            "interaction_chaos_progress_marker",
        ));
        (!evaluation.passed()).then(|| CaptureTeardownDetection {
            command_index: marker.command_index,
            capture_owner_class: marker.capture_owner_class.clone(),
            evaluation,
        })
    })
}

fn detect_dirty_settle_boundary_failure(
    settle_boundary: InteractionSettleBoundary,
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<DirtySettleDetection> {
    progress_marker_history.iter().find_map(|marker| {
        if marker.deferred_mutation_count == 0 && !marker.dirty_state_hash.starts_with("dirty:") {
            return None;
        }
        let evaluation = evaluate_layout_settled(LayoutSettledSnapshot::new(
            settle_boundary,
            true,
            vec![marker.dirty_state_hash.clone()],
            false,
            "interaction_chaos_progress_marker",
        ));
        (!evaluation.passed()).then(|| DirtySettleDetection {
            command_index: marker.command_index,
            dirty_state_hash: marker.dirty_state_hash.clone(),
            evaluation,
        })
    })
}

fn detect_router_contradiction_failure(
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<RouterContradictionDetection> {
    progress_marker_history.iter().find_map(|marker| {
        let captured_target = marker.capture_owner_class.strip_prefix("captured:")?;
        let routed_target = marker.routed_target_class.as_str();
        (captured_target != routed_target).then(|| RouterContradictionDetection {
            command_index: marker.command_index,
            capture_owner_class: marker.capture_owner_class.clone(),
            routed_target_class: marker.routed_target_class.clone(),
            failed_requirement: "captured_route_target_coherent",
        })
    })
}

fn detect_authority_violation_failure(
    progress_marker_history: &[InteractionProgressMarker],
) -> Option<AuthorityViolationDetection> {
    progress_marker_history.iter().find_map(|marker| {
        let authority_domain = marker
            .dirty_state_hash
            .strip_prefix("authority_violation:")?;
        let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
            role: InteractionFuzzRole::Oracle,
            responsibilities: vec![InteractionFuzzResponsibility::EvaluateReadOnlyOracle],
            forbidden_actions: vec![InteractionFuzzForbiddenAction::MutateWidgetStateDirectly],
            boundary_narrowed: false,
            full_confidence_claim: false,
            authority: "interaction_chaos_progress_marker",
        });
        (!evaluation.passed()).then(|| AuthorityViolationDetection {
            command_index: marker.command_index,
            authority_domain: authority_domain.to_string(),
            violation_class: "no_forbidden_boundary_action".to_string(),
            evaluation,
        })
    })
}

fn detect_replay_nondeterminism_failure(
    expected_replay: &InteractionReplayArtifact,
) -> Option<ReplayNondeterminismDetection> {
    let mut replay_driver = InteractionDriver::new();
    let replay_result = expected_replay.replay_into(&mut replay_driver);
    let replayed_trace = InteractionReplayArtifact::from_driver(
        ReplayMode::ExactPrimitive,
        expected_replay.fixture_id.as_str(),
        expected_replay.root_seed,
        expected_replay.shrink_seed,
        &replay_driver,
    );

    match replay_result {
        Ok(_) => detect_replay_artifact_mismatch(expected_replay, replayed_trace, None, None),
        Err(error) => {
            let command_index = replay_error_primitive_step_index(&error);
            detect_replay_artifact_mismatch(
                expected_replay,
                replayed_trace,
                Some(command_index),
                Some(format!("{error:?}")),
            )
        }
    }
}

fn detect_replay_artifact_mismatch(
    expected_replay: &InteractionReplayArtifact,
    replayed_trace: InteractionReplayArtifact,
    command_index_override: Option<usize>,
    replay_error: Option<String>,
) -> Option<ReplayNondeterminismDetection> {
    if replay_error.is_none() && expected_replay == &replayed_trace {
        return None;
    }

    let command_index = command_index_override.unwrap_or_else(|| {
        first_replay_artifact_divergence_index(expected_replay, &replayed_trace).unwrap_or(0)
    });
    Some(ReplayNondeterminismDetection {
        command_index,
        divergence_class: replay_artifact_divergence_class(
            expected_replay,
            &replayed_trace,
            replay_error.as_ref(),
        )
        .to_string(),
        expected_trace_fingerprint: replay_artifact_fingerprint(expected_replay),
        replayed_trace_fingerprint: replay_artifact_fingerprint(&replayed_trace),
        replay_error,
        replayed_trace,
    })
}

fn first_replay_artifact_divergence_index(
    expected_replay: &InteractionReplayArtifact,
    replayed_trace: &InteractionReplayArtifact,
) -> Option<usize> {
    for (index, (expected, replayed)) in expected_replay
        .primitive_trace
        .iter()
        .zip(replayed_trace.primitive_trace.iter())
        .enumerate()
    {
        if expected != replayed {
            return Some(index);
        }
    }

    if expected_replay.primitive_trace.len() != replayed_trace.primitive_trace.len() {
        return Some(
            expected_replay
                .primitive_trace
                .len()
                .min(replayed_trace.primitive_trace.len()),
        );
    }

    if expected_replay != replayed_trace {
        return Some(expected_replay.primitive_trace.len().saturating_sub(1));
    }

    None
}

fn replay_artifact_divergence_class(
    expected_replay: &InteractionReplayArtifact,
    replayed_trace: &InteractionReplayArtifact,
    replay_error: Option<&String>,
) -> &'static str {
    if replay_error.is_some() {
        return "replay_execution_error";
    }

    if expected_replay.primitive_trace.len() != replayed_trace.primitive_trace.len() {
        return "primitive_count_mismatch";
    }

    for (expected, replayed) in expected_replay
        .primitive_trace
        .iter()
        .zip(replayed_trace.primitive_trace.iter())
    {
        if expected.command != replayed.command {
            return "primitive_command_mismatch";
        }
        if expected.emitted_event_count != replayed.emitted_event_count {
            return "emitted_event_count_mismatch";
        }
        if expected.primitive_step_index != replayed.primitive_step_index {
            return "primitive_step_index_mismatch";
        }
    }

    if expected_replay.coverage_keys != replayed_trace.coverage_keys {
        return "coverage_key_mismatch";
    }

    if expected_replay.intent_trace != replayed_trace.intent_trace {
        return "intent_trace_mismatch";
    }

    "artifact_metadata_mismatch"
}

fn replay_artifact_fingerprint(replay: &InteractionReplayArtifact) -> String {
    serde_json::to_string(replay).expect("interaction replay artifacts must serialize")
}

fn replay_error_primitive_step_index(error: &InteractionDriverError) -> usize {
    match error {
        InteractionDriverError::NonFiniteSinkValue {
            primitive_step_index,
            ..
        } => *primitive_step_index,
    }
}

fn chaos_progress_marker(
    command_index: usize,
    driver: &InteractionDriver,
) -> InteractionProgressMarker {
    InteractionProgressMarker {
        command_index,
        frame_index: driver.primitive_trace().len() as u64,
        layout_epoch: driver.coverage().total_keys() as u64,
        scene_epoch: driver.fused_events().len() as u64,
        dirty_state_hash: format!(
            "coverage:{}:primitive:{}",
            driver.coverage().total_keys(),
            driver.primitive_trace().len()
        ),
        deferred_mutation_count: 0,
        capture_owner_class: "raw_input_fusion".to_string(),
        focus_owner_class: "not_observed".to_string(),
        routed_target_class: "raw_input_fusion".to_string(),
    }
}

fn preserves_chaos_failure(
    fields: &InteractionChaosFailureFields,
    detected: DetectedChaosFailure,
) -> bool {
    detected.detector == fields.failure_detector
        && detected.hard_failure_class == fields.hard_failure_class
}

fn chaos_report_corpus_deltas(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    primitive_family_set: &[InteractionChaosPrimitiveFamily],
    pointer_pressure_patterns: &[InteractionChaosPointerPressurePattern],
    pointer_motion_policies: &[InteractionChaosPointerMotionPolicy],
    primitive_trace: &[ReplayPrimitiveTraceStep],
) -> Vec<InteractionChaosCorpusEntry> {
    vec![InteractionChaosCorpusEntry {
        corpus_entry_id: format!(
            "{}:{}:{}",
            profile.profile_id, surface.id, CHAOS_STREAM_GENERATOR_ID
        ),
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        fixture_id: surface.id.clone(),
        chaos_surface_id: surface.id.clone(),
        generator_id: CHAOS_STREAM_GENERATOR_ID.to_string(),
        chaos_stream_generator_id: CHAOS_STREAM_GENERATOR_ID.to_string(),
        generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
        chaos_stream_generator_version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
        seed: profile.root_seed,
        rng_algorithm: profile.rng_algorithm.clone(),
        rng_version: profile.rng_version.clone(),
        primitive_stream_range: InteractionPrimitiveStreamRange {
            start: 0,
            end: primitive_trace.len(),
        },
        shrunk_trace: None,
        shrunk_primitive_trace: Vec::new(),
        hard_failure_class: None,
        coverage_keys_gained: chaos_pressure_coverage_keys(
            primitive_family_set,
            pointer_pressure_patterns,
            pointer_motion_policies,
            &profile.hard_failure_oracle_ids,
        ),
        oracle_set_used: Vec::new(),
        hard_failure_oracle_set_used: profile.hard_failure_oracle_ids.clone(),
    }]
}

pub(crate) fn chaos_failure_corpus_entry(
    packet: &InteractionFailurePacket,
    shrink_report: &InteractionChaosShrinkReport,
) -> Result<InteractionChaosCorpusEntry, InteractionChaosFailure> {
    let fields = packet
        .chaos_failure_fields
        .as_ref()
        .ok_or(InteractionChaosFailure::NotChaosFailurePacket)?;
    let mut coverage_keys = BTreeSet::new();
    coverage_keys.insert(format!(
        "chaos.failure_detector.{}",
        chaos_failure_detector_key(fields.failure_detector)
    ));
    coverage_keys.insert(format!(
        "chaos.hard_failure_class.{}",
        chaos_hard_failure_class_key(fields.hard_failure_class)
    ));

    Ok(InteractionChaosCorpusEntry {
        corpus_entry_id: format!(
            "{}:{}:{}",
            packet.failure_id, fields.chaos_surface_id, fields.failing_command_index
        ),
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        fixture_id: packet.fixture_id.clone(),
        chaos_surface_id: fields.chaos_surface_id.clone(),
        generator_id: fields.chaos_stream_generator.clone(),
        chaos_stream_generator_id: fields.chaos_stream_generator.clone(),
        generator_version: fields.chaos_stream_generator_version.clone(),
        chaos_stream_generator_version: fields.chaos_stream_generator_version.clone(),
        seed: fields.rng_seed,
        rng_algorithm: fields.rng_algorithm.clone(),
        rng_version: fields.rng_version.clone(),
        primitive_stream_range: fields.primitive_stream_range.clone(),
        shrunk_trace: Some(shrink_report.shrunk_trace.clone()),
        shrunk_primitive_trace: shrink_report.shrunk_trace.primitive_trace.clone(),
        hard_failure_class: Some(fields.hard_failure_class),
        coverage_keys_gained: coverage_keys.into_iter().collect(),
        oracle_set_used: vec![packet.invariant_id.clone()],
        hard_failure_oracle_set_used: vec![packet.invariant_id.clone()],
    })
}

fn chaos_pressure_coverage_keys(
    primitive_family_set: &[InteractionChaosPrimitiveFamily],
    pointer_pressure_patterns: &[InteractionChaosPointerPressurePattern],
    pointer_motion_policies: &[InteractionChaosPointerMotionPolicy],
    hard_failure_oracle_set: &[String],
) -> Vec<String> {
    let mut keys = BTreeSet::new();
    for family in primitive_family_set {
        keys.insert(format!(
            "chaos.primitive_family.{}",
            chaos_primitive_family_key(*family)
        ));
    }
    for pattern in pointer_pressure_patterns {
        keys.insert(format!(
            "chaos.pointer_pressure.{}",
            chaos_pointer_pressure_key(*pattern)
        ));
    }
    for policy in pointer_motion_policies {
        keys.insert(format!(
            "chaos.pointer_motion_policy.{}",
            chaos_pointer_motion_policy_key(*policy)
        ));
    }
    for oracle in hard_failure_oracle_set {
        keys.insert(format!("chaos.hard_failure_oracle.{oracle}"));
    }
    keys.into_iter().collect()
}

fn chaos_pressure_coverage_summary(
    profile: &InteractionFuzzProfile,
    surface: &InteractionChaosSurface,
    commands: &[GeneratedChaosCommand],
    primitive_family_set: &[InteractionChaosPrimitiveFamily],
    pointer_pressure_patterns: &[InteractionChaosPointerPressurePattern],
    pointer_motion_policies: &[InteractionChaosPointerMotionPolicy],
) -> InteractionChaosPressureCoverageSummary {
    let mut keys = BTreeSet::new();
    keys.insert(format!(
        "chaos_surface_class.{}",
        coverage_value_token(surface.root_surface_kind.as_str())
    ));
    if commands.len() > 1 {
        keys.insert("primitive_burst_class.multi_command_stream".to_string());
    }

    for family in primitive_family_set {
        keys.insert(format!(
            "primitive_command_class.{}",
            chaos_primitive_family_key(*family)
        ));
    }
    for pattern in pointer_pressure_patterns {
        record_pattern_pressure_dimension_keys(&mut keys, *pattern);
    }
    for policy in pointer_motion_policies {
        keys.insert(format!(
            "primitive_burst_class.pointer_motion_policy_{}",
            chaos_pointer_motion_policy_key(*policy)
        ));
    }
    let mut unsupported_domain_recorded = false;
    for domain in surface
        .expected_unsupported_domains
        .iter()
        .chain(profile.expected_unsupported_domains.iter())
    {
        unsupported_domain_recorded = true;
        keys.insert(format!(
            "unsupported_domain_class.expected_{}",
            coverage_value_token(domain.as_str())
        ));
    }
    if !unsupported_domain_recorded {
        keys.insert("unsupported_domain_class.none_declared".to_string());
    }
    for oracle in &profile.hard_failure_oracle_ids {
        keys.insert(format!(
            "hard_failure_detector_class.{}",
            coverage_value_token(oracle.as_str())
        ));
    }

    let mut active_pointer_button = false;
    let mut previous_pointer: Option<(f32, f32)> = None;
    let mut previous_delta: Option<(f32, f32)> = None;
    for generated in commands {
        record_command_pressure_dimension_keys(
            &mut keys,
            surface,
            &generated.command,
            active_pointer_button,
            previous_pointer,
            previous_delta,
        );
        match generated.command {
            InteractionCommand::PointerMove { x, y } => {
                if let Some((previous_x, previous_y)) = previous_pointer {
                    previous_delta = Some((x - previous_x, y - previous_y));
                }
                previous_pointer = Some((x, y));
            }
            InteractionCommand::PointerDown { .. } => {
                active_pointer_button = true;
            }
            InteractionCommand::PointerUp { .. } => {
                active_pointer_button = false;
            }
            _ => {}
        }
    }

    let pressure_keys: Vec<String> = keys.into_iter().collect();
    let required_dimensions = required_chaos_pressure_dimensions();
    let exercised_dimensions = exercised_pressure_dimensions(&pressure_keys);
    let exercised_dimension_set: BTreeSet<String> = exercised_dimensions.iter().cloned().collect();
    let missing_required_dimensions = required_dimensions
        .iter()
        .filter(|dimension| !exercised_dimension_set.contains(*dimension))
        .cloned()
        .collect::<Vec<_>>();

    InteractionChaosPressureCoverageSummary {
        total_keys: pressure_keys.len(),
        frontier_saturated: missing_required_dimensions.is_empty(),
        pressure_keys,
        required_dimensions,
        exercised_dimensions,
        missing_required_dimensions,
    }
}

fn required_chaos_pressure_dimensions() -> Vec<String> {
    [
        "chaos_surface_class",
        "primitive_command_class",
        "primitive_burst_class",
        "pointer_button_state_class",
        "pointer_position_region_class",
        "pointer_velocity_class",
        "pointer_acceleration_class",
        "pointer_curvature_class",
        "pointer_jitter_class",
        "edge_corner_pressure_class",
        "drag_threshold_state_class",
        "drag_duration_class",
        "wheel_burst_class",
        "key_chord_class",
        "text_input_class",
        "window_resize_class",
        "scale_factor_transition_class",
        "frame_gap_class",
        "settle_attempt_class",
        "unsupported_domain_class",
        "hard_failure_detector_class",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn exercised_pressure_dimensions(pressure_keys: &[String]) -> Vec<String> {
    let mut dimensions = BTreeSet::new();
    for key in pressure_keys {
        if let Some((dimension, _value)) = key.split_once('.') {
            dimensions.insert(dimension.to_string());
        }
    }
    dimensions.into_iter().collect()
}

fn record_pattern_pressure_dimension_keys(
    keys: &mut BTreeSet<String>,
    pattern: InteractionChaosPointerPressurePattern,
) {
    match pattern {
        InteractionChaosPointerPressurePattern::RandomWalk => {
            keys.insert("pointer_curvature_class.random_walk".to_string());
            keys.insert("pointer_velocity_class.random_walk".to_string());
        }
        InteractionChaosPointerPressurePattern::VelocityBurst => {
            keys.insert("pointer_velocity_class.velocity_burst".to_string());
        }
        InteractionChaosPointerPressurePattern::AccelerationBurst => {
            keys.insert("pointer_acceleration_class.acceleration_burst".to_string());
        }
        InteractionChaosPointerPressurePattern::Jitter => {
            keys.insert("pointer_jitter_class.jitter".to_string());
            keys.insert("pointer_curvature_class.jitter".to_string());
        }
        InteractionChaosPointerPressurePattern::EdgeCornerBias => {
            keys.insert("edge_corner_pressure_class.corner_sweep".to_string());
            keys.insert("pointer_position_region_class.edge_corner".to_string());
        }
        InteractionChaosPointerPressurePattern::DragThresholdCrossing => {
            keys.insert("drag_threshold_state_class.crossed".to_string());
        }
        InteractionChaosPointerPressurePattern::LongDrag => {
            keys.insert("drag_duration_class.long_drag".to_string());
        }
        InteractionChaosPointerPressurePattern::HoverPause => {
            keys.insert("frame_gap_class.hover_pause".to_string());
            keys.insert("primitive_burst_class.hover_pause".to_string());
        }
        InteractionChaosPointerPressurePattern::PressDragRelease => {
            keys.insert("pointer_button_state_class.press_drag_release".to_string());
        }
        InteractionChaosPointerPressurePattern::WheelBurstUnderPointer => {
            keys.insert("wheel_burst_class.under_pointer".to_string());
        }
    }
}

fn record_command_pressure_dimension_keys(
    keys: &mut BTreeSet<String>,
    surface: &InteractionChaosSurface,
    command: &InteractionCommand,
    active_pointer_button: bool,
    previous_pointer: Option<(f32, f32)>,
    previous_delta: Option<(f32, f32)>,
) {
    keys.insert(format!(
        "primitive_command_class.{}",
        primitive_command_class_key(PrimitiveCommandClass::from(command))
    ));

    match command {
        InteractionCommand::PointerMove { x, y } => {
            keys.insert(format!(
                "pointer_button_state_class.{}",
                if active_pointer_button {
                    "button_held"
                } else {
                    "no_button"
                }
            ));
            keys.insert(format!(
                "pointer_position_region_class.{}",
                pointer_position_region_key(surface, *x, *y)
            ));
            if let Some((previous_x, previous_y)) = previous_pointer {
                let delta_x = *x - previous_x;
                let delta_y = *y - previous_y;
                let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
                keys.insert(format!(
                    "pointer_velocity_class.{}",
                    pointer_velocity_key(distance)
                ));
                if let Some((previous_delta_x, previous_delta_y)) = previous_delta {
                    let accel_x = delta_x - previous_delta_x;
                    let accel_y = delta_y - previous_delta_y;
                    let acceleration = (accel_x * accel_x + accel_y * accel_y).sqrt();
                    keys.insert(format!(
                        "pointer_acceleration_class.{}",
                        pointer_acceleration_key(acceleration)
                    ));
                    let cross = previous_delta_x * delta_y - previous_delta_y * delta_x;
                    keys.insert(format!(
                        "pointer_curvature_class.{}",
                        pointer_curvature_key(cross)
                    ));
                }
            }
        }
        InteractionCommand::PointerDown { .. } => {
            keys.insert("pointer_button_state_class.primary_down".to_string());
        }
        InteractionCommand::PointerUp { .. } => {
            keys.insert("pointer_button_state_class.primary_up".to_string());
        }
        InteractionCommand::WheelPixels { .. } => {
            keys.insert("wheel_burst_class.pixel_delta".to_string());
        }
        InteractionCommand::WheelLines { .. } => {
            keys.insert("wheel_burst_class.line_delta".to_string());
        }
        InteractionCommand::KeyDown {
            key_code,
            modifiers,
            ..
        } => {
            keys.insert(format!(
                "key_chord_class.key_down_{}_{}",
                key_code_token(key_code),
                modifier_token(modifiers)
            ));
        }
        InteractionCommand::KeyUp {
            key_code,
            modifiers,
            ..
        } => {
            keys.insert(format!(
                "key_chord_class.key_up_{}_{}",
                key_code_token(key_code),
                modifier_token(modifiers)
            ));
        }
        InteractionCommand::TextInput { text, .. } => {
            keys.insert(format!("text_input_class.{}", text_input_class_key(text)));
        }
        InteractionCommand::Frame { count } => {
            keys.insert(format!("frame_gap_class.{}", frame_gap_key(*count)));
        }
        InteractionCommand::Reconcile => {
            keys.insert("settle_attempt_class.reconcile".to_string());
        }
        InteractionCommand::SetWindowSize { width, height } => {
            keys.insert(format!(
                "window_resize_class.{}",
                window_resize_class_key(*width, *height)
            ));
        }
        InteractionCommand::SetScaleFactor { scale_factor } => {
            keys.insert(format!(
                "scale_factor_transition_class.{}",
                scale_factor_class_key(*scale_factor)
            ));
        }
    }
}

fn record_chaos_command_pressure_keys(
    seen_pressure_keys: &mut BTreeSet<String>,
    generated: &GeneratedChaosCommand,
) -> usize {
    chaos_command_pressure_keys(generated)
        .into_iter()
        .filter(|key| seen_pressure_keys.insert(key.clone()))
        .count()
}

fn chaos_command_pressure_keys(generated: &GeneratedChaosCommand) -> Vec<String> {
    let mut keys = vec![format!(
        "chaos.primitive_family.{}",
        chaos_primitive_family_key(InteractionChaosPrimitiveFamily::from_command(
            &generated.command,
        ))
    )];
    keys.extend(generated.pressure_patterns.iter().map(|pattern| {
        format!(
            "chaos.pointer_pressure.{}",
            chaos_pointer_pressure_key(*pattern)
        )
    }));
    if let Some(policy) = generated.pointer_motion_policy {
        keys.push(format!(
            "chaos.pointer_motion_policy.{}",
            chaos_pointer_motion_policy_key(policy)
        ));
    }
    keys
}

fn chaos_pressure_frontier_saturated(seen_pressure_keys: &BTreeSet<String>) -> bool {
    required_chaos_pressure_keys()
        .iter()
        .all(|key| seen_pressure_keys.contains(key))
}

fn chaos_corpus_policy_saturates_on_pressure_frontier(surface: &InteractionChaosSurface) -> bool {
    surface.corpus_policy == CHAOS_CORPUS_POLICY_PRESSURE_FRONTIER
}

fn required_chaos_pressure_keys() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for family in all_required_primitive_families() {
        keys.insert(format!(
            "chaos.primitive_family.{}",
            chaos_primitive_family_key(family)
        ));
    }
    for pattern in all_required_pointer_patterns() {
        keys.insert(format!(
            "chaos.pointer_pressure.{}",
            chaos_pointer_pressure_key(pattern)
        ));
    }
    for policy in all_required_pointer_motion_policies() {
        keys.insert(format!(
            "chaos.pointer_motion_policy.{}",
            chaos_pointer_motion_policy_key(policy)
        ));
    }
    keys
}

fn chaos_primitive_family_key(family: InteractionChaosPrimitiveFamily) -> &'static str {
    match family {
        InteractionChaosPrimitiveFamily::PointerMove => "pointer_move",
        InteractionChaosPrimitiveFamily::PointerButton => "pointer_button",
        InteractionChaosPrimitiveFamily::WheelPixels => "wheel_pixels",
        InteractionChaosPrimitiveFamily::WheelLines => "wheel_lines",
        InteractionChaosPrimitiveFamily::Keyboard => "keyboard",
        InteractionChaosPrimitiveFamily::TextInput => "text_input",
        InteractionChaosPrimitiveFamily::Frame => "frame",
        InteractionChaosPrimitiveFamily::Reconcile => "reconcile",
        InteractionChaosPrimitiveFamily::WindowSize => "window_size",
        InteractionChaosPrimitiveFamily::ScaleFactor => "scale_factor",
    }
}

fn chaos_command_domain(
    command: &crate::core::interaction_fuzz::InteractionCommand,
) -> &'static str {
    match command {
        crate::core::interaction_fuzz::InteractionCommand::PointerMove { .. }
        | crate::core::interaction_fuzz::InteractionCommand::PointerDown { .. }
        | crate::core::interaction_fuzz::InteractionCommand::PointerUp { .. } => "pointer",
        crate::core::interaction_fuzz::InteractionCommand::WheelPixels { .. }
        | crate::core::interaction_fuzz::InteractionCommand::WheelLines { .. } => "scroll",
        crate::core::interaction_fuzz::InteractionCommand::KeyDown { .. }
        | crate::core::interaction_fuzz::InteractionCommand::KeyUp { .. } => "keyboard",
        crate::core::interaction_fuzz::InteractionCommand::TextInput { .. } => "text_input",
        crate::core::interaction_fuzz::InteractionCommand::Frame { .. } => "frame_clock",
        crate::core::interaction_fuzz::InteractionCommand::Reconcile => "reconcile",
        crate::core::interaction_fuzz::InteractionCommand::SetWindowSize { .. }
        | crate::core::interaction_fuzz::InteractionCommand::SetScaleFactor { .. } => {
            "window_configuration"
        }
    }
}

fn primitive_command_class_key(class: PrimitiveCommandClass) -> &'static str {
    match class {
        PrimitiveCommandClass::PointerMove => "pointer_move",
        PrimitiveCommandClass::PointerDown => "pointer_down",
        PrimitiveCommandClass::PointerUp => "pointer_up",
        PrimitiveCommandClass::WheelPixels => "wheel_pixels",
        PrimitiveCommandClass::WheelLines => "wheel_lines",
        PrimitiveCommandClass::KeyDown => "key_down",
        PrimitiveCommandClass::KeyUp => "key_up",
        PrimitiveCommandClass::TextInput => "text_input",
        PrimitiveCommandClass::Frame => "frame",
        PrimitiveCommandClass::Reconcile => "reconcile",
        PrimitiveCommandClass::SetWindowSize => "set_window_size",
        PrimitiveCommandClass::SetScaleFactor => "set_scale_factor",
    }
}

fn pointer_position_region_key(surface: &InteractionChaosSurface, x: f32, y: f32) -> &'static str {
    let edge_x = x <= 1.0 || x >= surface.window_size.0 - 1.0;
    let edge_y = y <= 1.0 || y >= surface.window_size.1 - 1.0;
    if edge_x && edge_y {
        "corner"
    } else if edge_x || edge_y {
        "edge"
    } else {
        "interior"
    }
}

fn pointer_velocity_key(distance: f32) -> &'static str {
    if distance == 0.0 {
        "stationary"
    } else if distance < 8.0 {
        "low"
    } else if distance < 64.0 {
        "medium"
    } else {
        "high"
    }
}

fn pointer_acceleration_key(acceleration: f32) -> &'static str {
    if acceleration < 1.0 {
        "stable"
    } else if acceleration < 24.0 {
        "moderate"
    } else {
        "burst"
    }
}

fn pointer_curvature_key(cross: f32) -> &'static str {
    if cross.abs() < 0.01 {
        "linear"
    } else {
        "curved"
    }
}

fn key_code_token(key_code: &KeyCode) -> String {
    coverage_value_token(format!("{key_code:?}").as_str())
}

fn modifier_token(modifiers: &Modifiers) -> &'static str {
    match (
        modifiers.ctrl,
        modifiers.alt,
        modifiers.shift,
        modifiers.meta,
    ) {
        (false, false, false, false) => "no_modifiers",
        (true, false, false, false) => "ctrl",
        (false, true, false, false) => "alt",
        (false, false, true, false) => "shift",
        (false, false, false, true) => "meta",
        _ => "multi_modifier",
    }
}

fn text_input_class_key(text: &str) -> &'static str {
    if text.is_empty() {
        "empty"
    } else if text.is_ascii() {
        "ascii"
    } else {
        "unicode"
    }
}

fn frame_gap_key(count: u32) -> &'static str {
    if count <= 1 {
        "single_frame"
    } else {
        "multi_frame"
    }
}

fn window_resize_class_key(width: f32, height: f32) -> &'static str {
    if width >= 1.0 && height >= 1.0 {
        "valid_surface_extent"
    } else {
        "degenerate_surface_extent"
    }
}

fn scale_factor_class_key(scale_factor: f32) -> &'static str {
    if (scale_factor - 1.0).abs() < f32::EPSILON {
        "identity"
    } else {
        "non_identity"
    }
}

fn coverage_value_token(value: &str) -> String {
    let mut token = String::new();
    let mut previous_was_separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            token.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            token.push('_');
            previous_was_separator = true;
        }
    }
    while token.ends_with('_') {
        token.pop();
    }
    if token.is_empty() {
        "unspecified".to_string()
    } else {
        token
    }
}

fn chaos_pointer_pressure_key(pattern: InteractionChaosPointerPressurePattern) -> &'static str {
    match pattern {
        InteractionChaosPointerPressurePattern::RandomWalk => "random_walk",
        InteractionChaosPointerPressurePattern::VelocityBurst => "velocity_burst",
        InteractionChaosPointerPressurePattern::AccelerationBurst => "acceleration_burst",
        InteractionChaosPointerPressurePattern::Jitter => "jitter",
        InteractionChaosPointerPressurePattern::EdgeCornerBias => "edge_corner_bias",
        InteractionChaosPointerPressurePattern::DragThresholdCrossing => "drag_threshold_crossing",
        InteractionChaosPointerPressurePattern::LongDrag => "long_drag",
        InteractionChaosPointerPressurePattern::HoverPause => "hover_pause",
        InteractionChaosPointerPressurePattern::PressDragRelease => "press_drag_release",
        InteractionChaosPointerPressurePattern::WheelBurstUnderPointer => {
            "wheel_burst_under_pointer"
        }
    }
}

fn chaos_pointer_motion_policy_key(policy: InteractionChaosPointerMotionPolicy) -> &'static str {
    match policy {
        InteractionChaosPointerMotionPolicy::RandomWalkContinuous => "random_walk_continuous",
        InteractionChaosPointerMotionPolicy::VelocityBurstContinuous => "velocity_burst_continuous",
        InteractionChaosPointerMotionPolicy::AccelerationBurstContinuous => {
            "acceleration_burst_continuous"
        }
        InteractionChaosPointerMotionPolicy::JitterContinuous => "jitter_continuous",
        InteractionChaosPointerMotionPolicy::EdgeCornerSweep => "edge_corner_sweep",
        InteractionChaosPointerMotionPolicy::ThresholdCrossing => "threshold_crossing",
        InteractionChaosPointerMotionPolicy::LongDragContinuous => "long_drag_continuous",
        InteractionChaosPointerMotionPolicy::HoverPauseBoundary => "hover_pause_boundary",
        InteractionChaosPointerMotionPolicy::PressDragReleasePath => "press_drag_release_path",
    }
}

fn chaos_failure_detector_key(detector: InteractionChaosFailureDetector) -> &'static str {
    match detector {
        InteractionChaosFailureDetector::PanicCrashOrUnwind => "panic_crash_or_unwind",
        InteractionChaosFailureDetector::TimeoutOrNoProgress => "timeout_or_no_progress",
        InteractionChaosFailureDetector::NonFiniteSink => "non_finite_sink",
        InteractionChaosFailureDetector::CaptureSurvivedReleaseOrTeardown => {
            "capture_survived_release_or_teardown"
        }
        InteractionChaosFailureDetector::DirtyLayoutAtSettleBoundary => {
            "dirty_layout_at_settle_boundary"
        }
        InteractionChaosFailureDetector::EventRouterContradiction => "event_router_contradiction",
        InteractionChaosFailureDetector::AuthorityViolation => "authority_violation",
        InteractionChaosFailureDetector::ReplayNondeterminism => "replay_nondeterminism",
        InteractionChaosFailureDetector::UnsupportedCommandOrDomainSkip => {
            "unsupported_command_or_domain_skip"
        }
    }
}

fn chaos_hard_failure_class_key(class: InteractionChaosHardFailureClass) -> &'static str {
    match class {
        InteractionChaosHardFailureClass::PanicCrashOrUnwind => "panic_crash_or_unwind",
        InteractionChaosHardFailureClass::TimeoutHangOrNoProgress => "timeout_hang_or_no_progress",
        InteractionChaosHardFailureClass::NonFiniteFinalSink => "non_finite_final_sink",
        InteractionChaosHardFailureClass::PointerCaptureLeak => "pointer_capture_leak",
        InteractionChaosHardFailureClass::DirtyLayoutAtSettleBoundary => {
            "dirty_layout_at_settle_boundary"
        }
        InteractionChaosHardFailureClass::EventRouterContradiction => "event_router_contradiction",
        InteractionChaosHardFailureClass::AuthorityViolation => "authority_violation",
        InteractionChaosHardFailureClass::ReplayNondeterminism => "replay_nondeterminism",
        InteractionChaosHardFailureClass::UnsupportedDomainSkip => "unsupported_domain_skip",
    }
}

pub(crate) fn ci_chaos_profile(surface: &InteractionChaosSurface) -> InteractionFuzzProfile {
    InteractionFuzzProfile {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        profile_id: CHAOS_SMOKE_PROFILE_ID.to_string(),
        mode: InteractionFuzzMode::ChaosSmoke,
        artifact_dir: CHAOS_ARTIFACT_DIR.to_string(),
        injection_path: InteractionInjectionPath::RawInputFusion,
        settle_boundary: InteractionSettleBoundary::Reconcile,
        fixture_ids: Vec::new(),
        chaos_surface_ids: vec![surface.id.clone()],
        generator_ids: Vec::new(),
        chaos_stream_generator_ids: vec![CHAOS_STREAM_GENERATOR_ID.to_string()],
        generator_versions: Vec::new(),
        chaos_stream_generator_versions: vec![InteractionGeneratorVersionRecord {
            generator_id: CHAOS_STREAM_GENERATOR_ID.to_string(),
            version: CHAOS_STREAM_GENERATOR_VERSION.to_string(),
        }],
        oracle_ids: enabled_chaos_hard_failure_oracles(),
        hard_failure_oracle_ids: enabled_chaos_hard_failure_oracles(),
        root_seed: CHAOS_ROOT_SEED,
        seed_set: vec![CHAOS_ROOT_SEED],
        rng_algorithm: CHAOS_RNG_ALGORITHM.to_string(),
        rng_version: CHAOS_RNG_VERSION.to_string(),
        command_budget: CHAOS_COMMAND_BUDGET,
        transaction_budget: CHAOS_TRANSACTION_BUDGET,
        coverage_window: CHAOS_COVERAGE_WINDOW,
        timeout_policy: chaos_timeout_policy(CHAOS_ROOT_SEED),
        visual_diagnostics_policy: chaos_visual_diagnostics_policy(),
        constitution_boundary_profile: chaos_constitution_boundary_profile(),
        disabled_domains: Vec::new(),
        expected_unsupported_domains: CHAOS_EXPECTED_UNSUPPORTED_DOMAINS
            .iter()
            .map(|domain| (*domain).to_string())
            .collect(),
    }
}

pub(crate) fn ci_chaos_surface() -> InteractionChaosSurface {
    InteractionChaosSurface {
        id: CHAOS_SURFACE_BASIC_ID.to_string(),
        description: "Headless primitive command chaos surface".to_string(),
        root_surface_kind: "interaction_driver".to_string(),
        construction_entrypoint: "src/tests/interaction_fuzz/chaos::ci_chaos_surface".to_string(),
        build_identity: "halcyon-in-process".to_string(),
        window_size: (640.0, 360.0),
        scale_factor: 1.0,
        theme: "headless-default".to_string(),
        clock_seed: CHAOS_ROOT_SEED,
        initial_focus: None,
        initial_scroll: Some((0.0, 0.0)),
        initial_selection: None,
        initial_state_snapshot: InteractionStateSnapshot {
            state_id: "chaos-surface-initial".to_string(),
            fields: BTreeMap::from([
                ("window_width".to_string(), "640".to_string()),
                ("window_height".to_string(), "360".to_string()),
                ("scale_factor".to_string(), "1".to_string()),
            ]),
        },
        allowed_primitive_families: all_required_primitive_families(),
        enabled_hard_failure_oracles: enabled_chaos_hard_failure_oracles(),
        disabled_domains: Vec::new(),
        expected_unsupported_domains: CHAOS_EXPECTED_UNSUPPORTED_DOMAINS
            .iter()
            .map(|domain| (*domain).to_string())
            .collect(),
        settle_boundary: InteractionSettleBoundary::Reconcile,
        command_budget: CHAOS_COMMAND_BUDGET,
        transaction_budget: CHAOS_TRANSACTION_BUDGET,
        coverage_window: CHAOS_COVERAGE_WINDOW,
        timeout_policy: chaos_timeout_policy(CHAOS_ROOT_SEED),
        corpus_policy: CHAOS_CORPUS_POLICY_MEMORY_ONLY_SMOKE.to_string(),
        visual_diagnostics_policy: chaos_visual_diagnostics_policy(),
        constitution_boundary_profile: chaos_constitution_boundary_profile(),
    }
}

fn deterministic_chaos_stream(seed: u64, command_budget: usize) -> Vec<GeneratedChaosCommand> {
    let mut rng = seed;
    let mut out = Vec::with_capacity(command_budget);
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::SetWindowSize {
            width: 640.0,
            height: 360.0,
        },
        [],
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::SetScaleFactor { scale_factor: 1.0 },
        [],
    );

    let mut x = 120.0;
    let mut y = 90.0;
    for _ in 0..5 {
        x = clamp_window_x(x + (next_unit(&mut rng) - 0.5) * 48.0);
        y = clamp_window_y(y + (next_unit(&mut rng) - 0.5) * 32.0);
        push_pointer_move(
            &mut out,
            x,
            y,
            InteractionChaosPointerPressurePattern::RandomWalk,
        );
    }
    push_frame(&mut out, 1, []);

    for step in 1..=4 {
        x = clamp_window_x(x + step as f32 * 18.0);
        push_pointer_move(
            &mut out,
            x,
            y,
            InteractionChaosPointerPressurePattern::VelocityBurst,
        );
    }
    push_frame(&mut out, 1, []);

    for step in 1..=4 {
        y = clamp_window_y(y + (step * step) as f32 * 3.0);
        push_pointer_move(
            &mut out,
            x,
            y,
            InteractionChaosPointerPressurePattern::AccelerationBurst,
        );
    }
    push_frame(&mut out, 1, []);

    for step in 0..6 {
        let offset = if step % 2 == 0 { 2.0 } else { -2.0 };
        push_pointer_move(
            &mut out,
            x + offset,
            y - offset,
            InteractionChaosPointerPressurePattern::Jitter,
        );
    }
    push_frame(
        &mut out,
        3,
        [InteractionChaosPointerPressurePattern::HoverPause],
    );

    for (corner_x, corner_y) in [(1.0, 1.0), (639.0, 1.0), (639.0, 359.0), (1.0, 359.0)] {
        push_pointer_move(
            &mut out,
            corner_x,
            corner_y,
            InteractionChaosPointerPressurePattern::EdgeCornerBias,
        );
    }
    push_frame(&mut out, 1, []);

    push_pointer_move(
        &mut out,
        160.0,
        120.0,
        InteractionChaosPointerPressurePattern::PressDragRelease,
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::PointerDown {
            x: 160.0,
            y: 120.0,
            button: PointerButton::Primary,
        },
        [InteractionChaosPointerPressurePattern::PressDragRelease],
    );
    push_pointer_move(
        &mut out,
        162.0,
        120.0,
        InteractionChaosPointerPressurePattern::PressDragRelease,
    );
    push_command_with_motion(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::PointerMove { x: 168.0, y: 120.0 },
        [
            InteractionChaosPointerPressurePattern::PressDragRelease,
            InteractionChaosPointerPressurePattern::DragThresholdCrossing,
        ],
        Some(InteractionChaosPointerMotionPolicy::ThresholdCrossing),
    );
    for drag_x in [220.0, 300.0, 390.0, 500.0] {
        push_command_with_motion(
            &mut out,
            crate::core::interaction_fuzz::InteractionCommand::PointerMove {
                x: drag_x,
                y: 120.0,
            },
            [
                InteractionChaosPointerPressurePattern::PressDragRelease,
                InteractionChaosPointerPressurePattern::LongDrag,
            ],
            Some(InteractionChaosPointerMotionPolicy::LongDragContinuous),
        );
    }
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::PointerUp {
            x: 500.0,
            y: 120.0,
            button: PointerButton::Primary,
        },
        [InteractionChaosPointerPressurePattern::PressDragRelease],
    );
    push_frame(&mut out, 1, []);

    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::WheelPixels {
            x: 500.0,
            y: 120.0,
            delta_x: 0.0,
            delta_y: -64.0,
        },
        [InteractionChaosPointerPressurePattern::WheelBurstUnderPointer],
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::WheelLines {
            x: 500.0,
            y: 120.0,
            delta_x: 1.0,
            delta_y: -3.0,
        },
        [InteractionChaosPointerPressurePattern::WheelBurstUnderPointer],
    );

    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::KeyDown {
            key_code: KeyCode::Tab,
            modifiers: Modifiers::default(),
            repeat: false,
        },
        [],
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::KeyUp {
            key_code: KeyCode::Tab,
            modifiers: Modifiers::default(),
        },
        [],
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::TextInput {
            text: "chaos".to_string(),
            modifiers: Modifiers::default(),
        },
        [],
    );
    push_command(
        &mut out,
        crate::core::interaction_fuzz::InteractionCommand::Reconcile,
        [],
    );

    out
}

fn push_pointer_move(
    out: &mut Vec<GeneratedChaosCommand>,
    x: f32,
    y: f32,
    pattern: InteractionChaosPointerPressurePattern,
) {
    push_command_with_motion(
        out,
        crate::core::interaction_fuzz::InteractionCommand::PointerMove { x, y },
        [pattern],
        pointer_motion_policy_for_pressure(pattern),
    );
}

fn push_frame<const N: usize>(
    out: &mut Vec<GeneratedChaosCommand>,
    count: u32,
    pressure_patterns: [InteractionChaosPointerPressurePattern; N],
) {
    let pointer_motion_policy = pressure_patterns.iter().find_map(|pattern| match pattern {
        InteractionChaosPointerPressurePattern::HoverPause => {
            Some(InteractionChaosPointerMotionPolicy::HoverPauseBoundary)
        }
        _ => None,
    });

    push_command_with_motion(
        out,
        crate::core::interaction_fuzz::InteractionCommand::Frame { count },
        pressure_patterns,
        pointer_motion_policy,
    );
}

fn push_command<const N: usize>(
    out: &mut Vec<GeneratedChaosCommand>,
    command: crate::core::interaction_fuzz::InteractionCommand,
    pressure_patterns: [InteractionChaosPointerPressurePattern; N],
) {
    push_command_with_motion(out, command, pressure_patterns, None);
}

fn push_command_with_motion<const N: usize>(
    out: &mut Vec<GeneratedChaosCommand>,
    command: crate::core::interaction_fuzz::InteractionCommand,
    pressure_patterns: [InteractionChaosPointerPressurePattern; N],
    pointer_motion_policy: Option<InteractionChaosPointerMotionPolicy>,
) {
    out.push(GeneratedChaosCommand {
        command,
        pressure_patterns: pressure_patterns.to_vec(),
        pointer_motion_policy,
    });
}

fn replay_commands(commands: &[GeneratedChaosCommand]) -> Vec<ReplayInteractionCommand> {
    commands
        .iter()
        .map(|generated| ReplayInteractionCommand::from(&generated.command))
        .collect()
}

fn generated_command_records(
    commands: &[GeneratedChaosCommand],
) -> Vec<InteractionChaosGeneratedCommandRecord> {
    let mut pointer_sample_ordinal = 0usize;
    commands
        .iter()
        .enumerate()
        .map(|(primitive_step_index, generated)| {
            let pointer_sample =
                matches!(generated.command, InteractionCommand::PointerMove { .. }).then(|| {
                    let ordinal = pointer_sample_ordinal;
                    pointer_sample_ordinal += 1;
                    ordinal
                });

            InteractionChaosGeneratedCommandRecord {
                primitive_step_index,
                command: ReplayInteractionCommand::from(&generated.command),
                command_domain: chaos_command_domain(&generated.command).to_string(),
                pressure_patterns: generated.pressure_patterns.clone(),
                pointer_motion_policy: generated.pointer_motion_policy,
                pointer_sample_ordinal: pointer_sample,
                frame_boundary: matches!(generated.command, InteractionCommand::Frame { .. }),
                reconcile_boundary: matches!(generated.command, InteractionCommand::Reconcile),
            }
        })
        .collect()
}

fn replay_primitive_stream(commands: &[ReplayInteractionCommand]) -> Vec<ReplayPrimitiveTraceStep> {
    commands
        .iter()
        .enumerate()
        .map(|(primitive_step_index, command)| ReplayPrimitiveTraceStep {
            primitive_step_index,
            command: command.clone(),
            emitted_event_count: 0,
        })
        .collect()
}

fn primitive_family_set(
    commands: &[GeneratedChaosCommand],
) -> Vec<InteractionChaosPrimitiveFamily> {
    commands
        .iter()
        .map(|generated| InteractionChaosPrimitiveFamily::from_command(&generated.command))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn pointer_pressure_patterns(
    commands: &[GeneratedChaosCommand],
) -> Vec<InteractionChaosPointerPressurePattern> {
    commands
        .iter()
        .flat_map(|generated| generated.pressure_patterns.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn pointer_motion_policies(
    commands: &[GeneratedChaosCommand],
) -> Vec<InteractionChaosPointerMotionPolicy> {
    commands
        .iter()
        .filter_map(|generated| generated.pointer_motion_policy)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn all_required_primitive_families() -> Vec<InteractionChaosPrimitiveFamily> {
    [
        InteractionChaosPrimitiveFamily::PointerMove,
        InteractionChaosPrimitiveFamily::PointerButton,
        InteractionChaosPrimitiveFamily::WheelPixels,
        InteractionChaosPrimitiveFamily::WheelLines,
        InteractionChaosPrimitiveFamily::Keyboard,
        InteractionChaosPrimitiveFamily::TextInput,
        InteractionChaosPrimitiveFamily::Frame,
        InteractionChaosPrimitiveFamily::Reconcile,
        InteractionChaosPrimitiveFamily::WindowSize,
        InteractionChaosPrimitiveFamily::ScaleFactor,
    ]
    .to_vec()
}

fn all_required_pointer_patterns() -> Vec<InteractionChaosPointerPressurePattern> {
    [
        InteractionChaosPointerPressurePattern::RandomWalk,
        InteractionChaosPointerPressurePattern::VelocityBurst,
        InteractionChaosPointerPressurePattern::AccelerationBurst,
        InteractionChaosPointerPressurePattern::Jitter,
        InteractionChaosPointerPressurePattern::EdgeCornerBias,
        InteractionChaosPointerPressurePattern::DragThresholdCrossing,
        InteractionChaosPointerPressurePattern::LongDrag,
        InteractionChaosPointerPressurePattern::HoverPause,
        InteractionChaosPointerPressurePattern::PressDragRelease,
        InteractionChaosPointerPressurePattern::WheelBurstUnderPointer,
    ]
    .to_vec()
}

fn all_required_pointer_motion_policies() -> Vec<InteractionChaosPointerMotionPolicy> {
    [
        InteractionChaosPointerMotionPolicy::RandomWalkContinuous,
        InteractionChaosPointerMotionPolicy::VelocityBurstContinuous,
        InteractionChaosPointerMotionPolicy::AccelerationBurstContinuous,
        InteractionChaosPointerMotionPolicy::JitterContinuous,
        InteractionChaosPointerMotionPolicy::EdgeCornerSweep,
        InteractionChaosPointerMotionPolicy::ThresholdCrossing,
        InteractionChaosPointerMotionPolicy::LongDragContinuous,
        InteractionChaosPointerMotionPolicy::HoverPauseBoundary,
        InteractionChaosPointerMotionPolicy::PressDragReleasePath,
    ]
    .to_vec()
}

fn pointer_motion_policy_for_pressure(
    pattern: InteractionChaosPointerPressurePattern,
) -> Option<InteractionChaosPointerMotionPolicy> {
    match pattern {
        InteractionChaosPointerPressurePattern::RandomWalk => {
            Some(InteractionChaosPointerMotionPolicy::RandomWalkContinuous)
        }
        InteractionChaosPointerPressurePattern::VelocityBurst => {
            Some(InteractionChaosPointerMotionPolicy::VelocityBurstContinuous)
        }
        InteractionChaosPointerPressurePattern::AccelerationBurst => {
            Some(InteractionChaosPointerMotionPolicy::AccelerationBurstContinuous)
        }
        InteractionChaosPointerPressurePattern::Jitter => {
            Some(InteractionChaosPointerMotionPolicy::JitterContinuous)
        }
        InteractionChaosPointerPressurePattern::EdgeCornerBias => {
            Some(InteractionChaosPointerMotionPolicy::EdgeCornerSweep)
        }
        InteractionChaosPointerPressurePattern::DragThresholdCrossing => {
            Some(InteractionChaosPointerMotionPolicy::ThresholdCrossing)
        }
        InteractionChaosPointerPressurePattern::LongDrag => {
            Some(InteractionChaosPointerMotionPolicy::LongDragContinuous)
        }
        InteractionChaosPointerPressurePattern::HoverPause => {
            Some(InteractionChaosPointerMotionPolicy::HoverPauseBoundary)
        }
        InteractionChaosPointerPressurePattern::PressDragRelease => {
            Some(InteractionChaosPointerMotionPolicy::PressDragReleasePath)
        }
        InteractionChaosPointerPressurePattern::WheelBurstUnderPointer => None,
    }
}

fn enabled_chaos_hard_failure_oracles() -> Vec<String> {
    vec![
        ORACLE_346_CHAOS_NONFINITE_SINK_ID.to_string(),
        ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID.to_string(),
        ORACLE_346_CHAOS_PANIC_UNWIND_ID.to_string(),
        ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID.to_string(),
        ORACLE_346_CHAOS_DIRTY_SETTLE_ID.to_string(),
        ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID.to_string(),
        ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID.to_string(),
        ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID.to_string(),
        ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID.to_string(),
    ]
}

fn chaos_stream_policy() -> InteractionChaosStreamPolicy {
    InteractionChaosStreamPolicy {
        direct_injection_submode: None,
        preserve_intermediate_pointer_samples: true,
        record_frame_and_reconcile_boundaries: true,
    }
}

fn chaos_timeout_policy(root_seed: u64) -> InteractionTimeoutPolicy {
    InteractionTimeoutPolicy {
        per_transaction_timeout_ms: 250,
        run_timeout_ms: 10_000,
        settle_iteration_budget: 8,
        frame_progress_budget: 8,
        no_progress_frame_budget: 4,
        no_progress_settle_budget: 4,
        monotonic_clock_seed: root_seed,
    }
}

fn chaos_visual_diagnostics_policy() -> InteractionVisualDiagnosticsPolicy {
    InteractionVisualDiagnosticsPolicy {
        capture_enabled: false,
        headless: true,
        rendering_available: false,
    }
}

fn chaos_constitution_boundary_profile() -> InteractionConstitutionBoundaryProfile {
    InteractionConstitutionBoundaryProfile {
        input_fusion_owner: "InputFusion".to_string(),
        event_routing_owner: "EventRouter".to_string(),
        capture_owner: "PointerCapture".to_string(),
        focus_owner: "FocusManager".to_string(),
        layout_owner: "Layout".to_string(),
        scene_build_owner: "SceneGraph".to_string(),
        rendering_owner: "Renderer".to_string(),
        scroll_owner: "Scroll".to_string(),
        identity_owner: "WidgetIdentity".to_string(),
        probe_owner: "interaction_probe".to_string(),
        oracle_owner: "interaction_oracle".to_string(),
        shrinking_owner: "interaction_shrinker".to_string(),
        replay_owner: "interaction_replay".to_string(),
        reporting_owner: "interaction_reporter".to_string(),
        visual_diagnostics_owner: "interaction_visual_diagnostics".to_string(),
    }
}

fn next_unit(state: &mut u64) -> f32 {
    *state = (*state).wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*state >> 32) as u32) as f32 / u32::MAX as f32
}

fn clamp_window_x(value: f32) -> f32 {
    value.clamp(1.0, 639.0)
}

fn clamp_window_y(value: f32) -> f32 {
    value.clamp(1.0, 359.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_chaos_report(outcome: InteractionChaosRunOutcome) -> InteractionChaosReport {
        match outcome {
            InteractionChaosRunOutcome::ChaosReport(report) => report,
            InteractionChaosRunOutcome::FailurePacket(packet) => {
                panic!(
                    "expected chaos report, got failure packet: {}",
                    packet.failure_id
                )
            }
        }
    }

    fn non_finite_chaos_failure_packet() -> (Box<InteractionFailurePacket>, usize, usize) {
        synthetic_non_finite_chaos_failure_packet()
    }

    #[test]
    fn chaos_profile_declares_complete_replayable_primitive_surface() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);

        assert!(profile.is_complete());
        assert_eq!(profile.mode, InteractionFuzzMode::ChaosSmoke);
        assert_eq!(
            profile.chaos_surface_ids,
            vec![CHAOS_SURFACE_BASIC_ID.to_string()]
        );
        assert_eq!(
            profile.chaos_stream_generator_versions,
            vec![InteractionGeneratorVersionRecord {
                generator_id: CHAOS_STREAM_GENERATOR_ID.to_string(),
                version: CHAOS_STREAM_GENERATOR_VERSION.to_string()
            }]
        );
        assert_eq!(
            surface.allowed_primitive_families,
            all_required_primitive_families()
        );
        assert_eq!(
            surface.enabled_hard_failure_oracles,
            enabled_chaos_hard_failure_oracles()
        );
        assert_eq!(
            profile.expected_unsupported_domains,
            CHAOS_EXPECTED_UNSUPPORTED_DOMAINS
                .iter()
                .map(|domain| (*domain).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(profile.profile_hash().len(), "sha256:".len() + 64);
    }

    #[test]
    fn deterministic_chaos_stream_records_every_required_family_pressure_pattern_and_motion_policy()
    {
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, CHAOS_COMMAND_BUDGET);

        assert_eq!(
            primitive_family_set(&commands),
            all_required_primitive_families()
        );
        assert_eq!(
            pointer_pressure_patterns(&commands),
            all_required_pointer_patterns()
        );
        assert_eq!(
            pointer_motion_policies(&commands),
            all_required_pointer_motion_policies()
        );
        assert!(commands.iter().all(|generated| {
            !matches!(
                generated.command,
                crate::core::interaction_fuzz::InteractionCommand::PointerMove { .. }
            ) || generated.pointer_motion_policy.is_some()
        }));
        assert_eq!(
            commands
                .iter()
                .filter(|generated| matches!(
                    generated.command,
                    crate::core::interaction_fuzz::InteractionCommand::PointerMove { .. }
                ))
                .count(),
            30
        );
        assert!(commands.iter().any(|generated| {
            matches!(
                generated.command,
                crate::core::interaction_fuzz::InteractionCommand::Frame { .. }
            )
        }));
        assert!(commands.iter().any(|generated| {
            matches!(
                generated.command,
                crate::core::interaction_fuzz::InteractionCommand::Reconcile
            )
        }));
    }

    #[test]
    fn chaos_smoke_report_records_replayable_stream_without_confidence_packet() {
        let report =
            expect_chaos_report(run_ci_chaos_smoke_profile().expect("chaos smoke must run"));

        assert_eq!(report.run_profile_id, CHAOS_SMOKE_PROFILE_ID);
        assert!(report.run_profile_hash.starts_with("sha256:"));
        assert_eq!(report.chaos_mode, InteractionFuzzMode::ChaosSmoke);
        assert_eq!(
            report.halt_reason,
            InteractionChaosHaltReason::CommandBudget
        );
        assert_eq!(report.failure_count, 0);
        assert!(!report.confidence_packet_emitted);
        assert_eq!(report.no_failure_claim, INTERACTION_CHAOS_NO_FAILURE_CLAIM);
        assert_eq!(
            report.primitive_family_set,
            all_required_primitive_families()
        );
        assert_eq!(
            report.pointer_pressure_patterns,
            all_required_pointer_patterns()
        );
        assert_eq!(
            report.pointer_motion_policies,
            all_required_pointer_motion_policies()
        );
        assert_eq!(
            report.chaos_stream_generator_versions,
            vec![InteractionGeneratorVersionRecord {
                generator_id: CHAOS_STREAM_GENERATOR_ID.to_string(),
                version: CHAOS_STREAM_GENERATOR_VERSION.to_string()
            }]
        );
        assert_eq!(
            report.generated_primitive_commands,
            report
                .primitive_trace
                .iter()
                .map(|step| step.command.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            report.generated_command_records.len(),
            report.generated_primitive_commands.len()
        );
        assert_eq!(
            report.generated_command_records[0].command_domain,
            "window_configuration"
        );
        let pointer_records = report
            .generated_command_records
            .iter()
            .filter(|record| matches!(record.command, ReplayInteractionCommand::PointerMove { .. }))
            .collect::<Vec<_>>();
        assert_eq!(pointer_records.len(), 30);
        assert_eq!(pointer_records[0].pointer_sample_ordinal, Some(0));
        assert_eq!(pointer_records[1].pointer_sample_ordinal, Some(1));
        assert!(
            pointer_records
                .iter()
                .all(|record| record.pointer_motion_policy.is_some())
        );
        assert!(report.generated_command_records.iter().any(|record| {
            record
                .pressure_patterns
                .contains(&InteractionChaosPointerPressurePattern::LongDrag)
                && record.pointer_motion_policy
                    == Some(InteractionChaosPointerMotionPolicy::LongDragContinuous)
        }));
        assert!(
            report
                .generated_command_records
                .iter()
                .any(|record| record.frame_boundary)
        );
        assert!(
            report
                .generated_command_records
                .iter()
                .any(|record| record.reconcile_boundary)
        );
        assert_eq!(report.corpus_deltas.len(), 1);
        let corpus_entry = &report.corpus_deltas[0];
        assert_eq!(corpus_entry.fixture_id.as_str(), CHAOS_SURFACE_BASIC_ID);
        assert_eq!(
            corpus_entry.chaos_surface_id.as_str(),
            CHAOS_SURFACE_BASIC_ID
        );
        assert_eq!(
            corpus_entry.chaos_stream_generator_id.as_str(),
            CHAOS_STREAM_GENERATOR_ID
        );
        assert_eq!(
            corpus_entry.chaos_stream_generator_version.as_str(),
            CHAOS_STREAM_GENERATOR_VERSION
        );
        assert_eq!(corpus_entry.seed, CHAOS_ROOT_SEED);
        assert_eq!(corpus_entry.rng_algorithm.as_str(), CHAOS_RNG_ALGORITHM);
        assert_eq!(corpus_entry.rng_version.as_str(), CHAOS_RNG_VERSION);
        assert_eq!(corpus_entry.primitive_stream_range.start, 0);
        assert_eq!(
            corpus_entry.primitive_stream_range.end,
            report.primitive_trace.len()
        );
        assert!(corpus_entry.shrunk_trace.is_none());
        assert!(corpus_entry.shrunk_primitive_trace.is_empty());
        assert_eq!(corpus_entry.hard_failure_class, None);
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.primitive_family.pointer_move".to_string())
        );
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.pointer_pressure.random_walk".to_string())
        );
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.pointer_motion_policy.random_walk_continuous".to_string())
        );
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.pointer_motion_policy.threshold_crossing".to_string())
        );
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_NONFINITE_SINK_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_PANIC_UNWIND_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_DIRTY_SETTLE_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID}"
        )));
        assert!(corpus_entry.coverage_keys_gained.contains(&format!(
            "chaos.hard_failure_oracle.{ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID}"
        )));
        let pressure = &report.pressure_coverage_summary;
        assert!(pressure.frontier_saturated);
        assert_eq!(pressure.total_keys, pressure.pressure_keys.len());
        assert_eq!(pressure.missing_required_dimensions, Vec::<String>::new());
        assert!(
            pressure
                .required_dimensions
                .contains(&"chaos_surface_class".to_string())
        );
        assert!(
            pressure
                .required_dimensions
                .contains(&"pointer_velocity_class".to_string())
        );
        assert!(
            pressure
                .required_dimensions
                .contains(&"hard_failure_detector_class".to_string())
        );
        for dimension in &pressure.required_dimensions {
            assert!(
                pressure.exercised_dimensions.contains(dimension),
                "missing exercised chaos pressure dimension: {dimension}"
            );
        }
        assert!(
            pressure
                .pressure_keys
                .contains(&"pointer_velocity_class.velocity_burst".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"pointer_acceleration_class.acceleration_burst".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"pointer_jitter_class.jitter".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"wheel_burst_class.under_pointer".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"key_chord_class.key_down_tab_no_modifiers".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"text_input_class.ascii".to_string())
        );
        assert!(
            pressure
                .pressure_keys
                .contains(&"unsupported_domain_class.none_declared".to_string())
        );
        assert!(pressure.pressure_keys.iter().any(|key| {
            key.starts_with("hard_failure_detector_class.")
                && key.contains("oracle_346_chaos_nonfinite_final_sink")
        }));
        assert!(report.visual_diagnostics_status.unavailable);
        assert!(report.constitution_boundary_check_status.passed);
        assert_eq!(
            report.unsupported_domains,
            CHAOS_EXPECTED_UNSUPPORTED_DOMAINS
                .iter()
                .map(|domain| (*domain).to_string())
                .collect::<Vec<_>>()
        );

        let json = serde_json::to_string(&report).expect("chaos report must serialize");
        let decoded: InteractionChaosReport =
            serde_json::from_str(&json).expect("chaos report must deserialize");
        assert_eq!(
            decoded.generated_primitive_commands,
            report.generated_primitive_commands
        );
        assert_eq!(
            decoded.generated_command_records,
            report.generated_command_records
        );
    }

    #[test]
    fn chaos_no_new_coverage_window_halts_after_pressure_frontier_saturates() {
        let mut surface = ci_chaos_surface();
        let mut profile = ci_chaos_profile(&surface);
        profile.coverage_window = 2;
        let mut commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, CHAOS_COMMAND_BUDGET);
        push_command(
            &mut commands,
            crate::core::interaction_fuzz::InteractionCommand::Reconcile,
            [],
        );
        push_command(
            &mut commands,
            crate::core::interaction_fuzz::InteractionCommand::Reconcile,
            [],
        );
        profile.command_budget = commands.len();
        surface.command_budget = commands.len();

        let report = expect_chaos_report(
            run_chaos_smoke_profile_with_commands(profile, surface, commands)
                .expect("chaos no-new-coverage run must complete"),
        );

        assert_eq!(
            report.halt_reason,
            InteractionChaosHaltReason::NoNewCoverageWindow
        );
        assert_eq!(report.executed_command_budget, report.primitive_trace.len());
        assert_eq!(
            report.primitive_family_set,
            all_required_primitive_families()
        );
        assert_eq!(
            report.pointer_pressure_patterns,
            all_required_pointer_patterns()
        );
        assert_eq!(
            report.pointer_motion_policies,
            all_required_pointer_motion_policies()
        );
    }

    #[test]
    fn chaos_corpus_saturation_policy_halts_on_pressure_frontier() {
        let mut surface = ci_chaos_surface();
        surface.corpus_policy = CHAOS_CORPUS_POLICY_PRESSURE_FRONTIER.to_string();
        let profile = ci_chaos_profile(&surface);

        let report = expect_chaos_report(
            run_chaos_smoke_profile(profile, surface).expect("chaos smoke must run"),
        );

        assert_eq!(
            report.halt_reason,
            InteractionChaosHaltReason::CorpusSaturation
        );
        assert!(report.executed_command_budget < CHAOS_COMMAND_BUDGET);
        assert_eq!(report.executed_command_budget, report.primitive_trace.len());
        assert_eq!(
            report.primitive_family_set,
            all_required_primitive_families()
        );
        assert_eq!(
            report.pointer_pressure_patterns,
            all_required_pointer_patterns()
        );
        assert_eq!(
            report.pointer_motion_policies,
            all_required_pointer_motion_policies()
        );
        assert!(!report.confidence_packet_emitted);
        assert_eq!(report.no_failure_claim, INTERACTION_CHAOS_NO_FAILURE_CLAIM);
    }

    #[test]
    fn chaos_injection_rejects_non_finite_generated_sink_command() {
        let command = crate::core::interaction_fuzz::InteractionCommand::PointerMove {
            x: f32::NAN,
            y: 1.0,
        };
        let recorded = ReplayInteractionCommand::from(&command);
        let mut driver = InteractionDriver::new();

        let result = driver.try_execute(command);

        assert!(matches!(
            result,
            Err(InteractionDriverError::NonFiniteSinkValue { .. })
        ));
        assert_eq!(driver.primitive_trace().len(), 0);
        assert!(matches!(
            recorded,
            ReplayInteractionCommand::PointerMove { .. }
        ));
    }

    #[test]
    fn chaos_failure_packet_records_non_finite_sink_with_replayable_stream() {
        let (packet, failing_index, expected_stream_len) = non_finite_chaos_failure_packet();

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_NONFINITE_SINK_ID);
        assert_eq!(packet.severity, InteractionFailureSeverity::P0);
        assert_eq!(packet.failing_primitive_step_index, Some(failing_index));
        assert!(packet.command_that_failed.is_some());
        assert!(packet.intent_trace.is_empty());
        assert_eq!(packet.primitive_trace.len(), expected_stream_len);
        assert_eq!(
            packet.shrunk_trace.primitive_commands().len(),
            expected_stream_len
        );

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(fields.chaos_surface_id.as_str(), CHAOS_SURFACE_BASIC_ID);
        assert_eq!(fields.chaos_mode, InteractionFuzzMode::ChaosSmoke);
        assert_eq!(
            fields.chaos_stream_generator.as_str(),
            CHAOS_STREAM_GENERATOR_ID
        );
        assert_eq!(
            fields.chaos_stream_generator_version.as_str(),
            CHAOS_STREAM_GENERATOR_VERSION
        );
        assert_eq!(fields.rng_seed, CHAOS_ROOT_SEED);
        assert_eq!(fields.primitive_stream_range.start, 0);
        assert_eq!(fields.primitive_stream_range.end, expected_stream_len);
        assert_eq!(fields.failing_command_index, failing_index);
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::NonFiniteSink
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::NonFiniteFinalSink
        );
        assert_eq!(fields.replay_determinism_status.as_str(), "not_run");
        assert_eq!(
            fields.unsupported_domains,
            CHAOS_EXPECTED_UNSUPPORTED_DOMAINS
                .iter()
                .map(|domain| (*domain).to_string())
                .collect::<Vec<_>>()
        );
        assert!(packet.visual_diagnostics_status.unavailable);
        assert_eq!(
            packet.visual_diagnostics_status.reason.as_deref(),
            Some("headless")
        );
        assert_eq!(packet.visual_diagnostics_status.profile_policy, "headless");
        assert_eq!(packet.visual_trace_artifacts.len(), 1);
        assert_eq!(
            packet.visual_trace_artifacts[0].artifact_kind,
            "visual_diagnostics_unavailable"
        );
        assert_eq!(
            packet.visual_trace_artifacts[0].path,
            "unavailable:headless"
        );
        assert_eq!(
            packet.visual_trace_artifacts[0].detector_id,
            ORACLE_346_CHAOS_NONFINITE_SINK_ID
        );

        let json = serde_json::to_string(&packet).expect("chaos failure packet must serialize");
        let decoded: InteractionFailurePacket =
            serde_json::from_str(&json).expect("chaos failure packet must deserialize");
        assert!(decoded.chaos_failure_fields.is_some());
        assert!(decoded.visual_diagnostics_status.unavailable);
        assert_eq!(decoded.visual_trace_artifacts.len(), 1);
    }

    #[test]
    fn chaos_failure_visual_artifact_points_to_trace_when_capture_available() {
        let surface = ci_chaos_surface();
        let mut profile = ci_chaos_profile(&surface);
        profile.visual_diagnostics_policy = InteractionVisualDiagnosticsPolicy {
            capture_enabled: true,
            headless: false,
            rendering_available: true,
        };

        let artifacts = chaos_visual_trace_artifacts(&profile, ORACLE_346_CHAOS_PANIC_UNWIND_ID);

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact_kind, "visual_trace_json");
        assert_eq!(
            artifacts[0].path,
            "target/interaction-fuzz/chaos-smoke/diagnostics/visual-trace.json"
        );
        assert_eq!(artifacts[0].detector_id, ORACLE_346_CHAOS_PANIC_UNWIND_ID);
    }

    #[test]
    fn chaos_timeout_failure_packet_uses_declared_policy_and_progress_markers() {
        let surface = ci_chaos_surface();
        let mut profile = ci_chaos_profile(&surface);
        profile.timeout_policy.per_transaction_timeout_ms = 1;
        profile.timeout_policy.run_timeout_ms = 1;
        profile.command_budget = 4;
        let mut surface = surface;
        surface.command_budget = 4;

        let outcome = run_chaos_smoke_profile(profile.clone(), surface).expect("run must complete");

        let InteractionChaosRunOutcome::FailurePacket(packet) = outcome else {
            panic!("expected timeout failure packet");
        };
        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_TIMEOUT_NO_PROGRESS_ID);
        assert_eq!(packet.timeout_policy, profile.timeout_policy);
        assert!(!packet.progress_marker_history.is_empty());
        assert_eq!(packet.failing_primitive_step_index, Some(1));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("per_transaction_timeout_ms")
        );
        let fields = packet.chaos_failure_fields.expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::TimeoutOrNoProgress
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::TimeoutHangOrNoProgress
        );
        assert_eq!(fields.timeout_budget, profile.timeout_policy);
    }

    #[test]
    fn chaos_no_progress_detector_uses_unchanged_progress_marker_budget() {
        let policy = chaos_timeout_policy(CHAOS_ROOT_SEED);
        let marker = InteractionProgressMarker {
            command_index: 7,
            frame_index: 11,
            layout_epoch: 2,
            scene_epoch: 3,
            dirty_state_hash: "unchanged".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "held".to_string(),
            focus_owner_class: "focused".to_string(),
            routed_target_class: "target".to_string(),
        };
        let markers = vec![marker; policy.no_progress_frame_budget as usize];

        let detection = detect_timeout_or_no_progress(&policy, 7, &markers)
            .expect("no-progress must be detected");

        assert_eq!(detection.failed_requirement, "no_progress_frame_budget");
        assert_eq!(
            detection.failed_value,
            policy.no_progress_frame_budget.to_string()
        );
    }

    #[test]
    fn chaos_settle_budget_exhaustion_detector_uses_dirty_progress_marker_window() {
        let mut policy = chaos_timeout_policy(CHAOS_ROOT_SEED);
        policy.per_transaction_timeout_ms = 100;
        policy.run_timeout_ms = 100;
        policy.settle_iteration_budget = 2;
        policy.no_progress_frame_budget = 10;
        policy.no_progress_settle_budget = 10;
        let markers = (0..=2)
            .map(|command_index| InteractionProgressMarker {
                command_index,
                frame_index: command_index as u64,
                layout_epoch: command_index as u64,
                scene_epoch: command_index as u64,
                dirty_state_hash: format!("dirty:{command_index}"),
                deferred_mutation_count: 1,
                capture_owner_class: "none".to_string(),
                focus_owner_class: "none".to_string(),
                routed_target_class: "raw_input_fusion".to_string(),
            })
            .collect::<Vec<_>>();

        let detection = detect_timeout_or_no_progress(&policy, 2, &markers)
            .expect("settle budget exhaustion must be detected");

        assert_eq!(detection.failed_requirement, "settle_iteration_budget");
        assert_eq!(detection.failed_value, "3");
    }

    #[test]
    fn chaos_panic_failure_packet_records_unwind_boundary_without_escape() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![chaos_progress_marker(0, &driver)];

        let packet = build_chaos_panic_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            1,
            "synthetic chaos panic".to_string(),
        );

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_PANIC_UNWIND_ID);
        assert_eq!(
            packet.layer_classification,
            InteractionFailureLayer::PrimitiveInjection
        );
        assert_eq!(packet.failing_primitive_step_index, Some(1));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("panic_payload")
                .map(String::as_str),
            Some("synthetic chaos panic")
        );
        assert_eq!(packet.progress_marker_history, progress_markers);

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::PanicCrashOrUnwind
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::PanicCrashOrUnwind
        );
        assert_eq!(fields.failing_command_index, 1);
    }

    #[test]
    fn chaos_panic_payload_label_preserves_string_payloads() {
        let static_payload = panic::catch_unwind(|| panic::resume_unwind(Box::new("static panic")))
            .expect_err("panic payload required");
        let string_payload =
            panic::catch_unwind(|| panic::resume_unwind(Box::new(String::from("owned panic"))))
                .expect_err("panic payload required");

        assert_eq!(panic_payload_label(static_payload.as_ref()), "static panic");
        assert_eq!(panic_payload_label(string_payload.as_ref()), "owned panic");
    }

    #[test]
    fn chaos_capture_teardown_detector_uses_progress_marker_history() {
        let markers = vec![InteractionProgressMarker {
            command_index: 3,
            frame_index: 8,
            layout_epoch: 2,
            scene_epoch: 4,
            dirty_state_hash: "stable".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "leaked:header-separator-1".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];

        let detection = detect_capture_teardown_failure(&markers)
            .expect("leaked capture marker must be detected");

        assert_eq!(detection.command_index, 3);
        assert_eq!(
            detection.capture_owner_class.as_str(),
            "leaked:header-separator-1"
        );
        assert!(!detection.evaluation.passed());
        assert_eq!(
            detection.evaluation.failed_requirement.as_deref(),
            Some("capture_owner_received_release_or_cancel")
        );
    }

    #[test]
    fn chaos_capture_teardown_failure_packet_records_progress_marker_leak() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![InteractionProgressMarker {
            command_index: 2,
            frame_index: 6,
            layout_epoch: 1,
            scene_epoch: 3,
            dirty_state_hash: "stable".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "leaked:header-separator-1".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];
        let detection = detect_capture_teardown_failure(&progress_markers)
            .expect("leaked capture marker must be detected");

        let packet = build_chaos_capture_teardown_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            detection,
        );

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_CAPTURE_TEARDOWN_ID);
        assert_eq!(
            packet.layer_classification,
            InteractionFailureLayer::EventRouting
        );
        assert_eq!(packet.failing_primitive_step_index, Some(2));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("capture_owner_received_release_or_cancel")
        );
        assert_eq!(packet.progress_marker_history, progress_markers);

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::CaptureSurvivedReleaseOrTeardown
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::PointerCaptureLeak
        );
        assert_eq!(fields.failing_command_index, 2);
    }

    #[test]
    fn chaos_dirty_settle_detector_uses_deferred_mutation_marker() {
        let markers = vec![InteractionProgressMarker {
            command_index: 4,
            frame_index: 9,
            layout_epoch: 3,
            scene_epoch: 5,
            dirty_state_hash: "dirty:column_resize_pending".to_string(),
            deferred_mutation_count: 1,
            capture_owner_class: "none".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];

        let detection =
            detect_dirty_settle_boundary_failure(InteractionSettleBoundary::Reconcile, &markers)
                .expect("dirty settle marker must be detected");

        assert_eq!(detection.command_index, 4);
        assert_eq!(
            detection.dirty_state_hash.as_str(),
            "dirty:column_resize_pending"
        );
        assert!(!detection.evaluation.passed());
        assert_eq!(
            detection.evaluation.failed_requirement.as_deref(),
            Some("layout_clean_at_settle_boundary")
        );
    }

    #[test]
    fn chaos_dirty_settle_failure_packet_records_deferred_mutation_marker() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![InteractionProgressMarker {
            command_index: 3,
            frame_index: 7,
            layout_epoch: 2,
            scene_epoch: 4,
            dirty_state_hash: "dirty:column_resize_pending".to_string(),
            deferred_mutation_count: 1,
            capture_owner_class: "none".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];
        let detection = detect_dirty_settle_boundary_failure(
            InteractionSettleBoundary::Reconcile,
            &progress_markers,
        )
        .expect("dirty settle marker must be detected");

        let packet = build_chaos_dirty_settle_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            detection,
        );

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_DIRTY_SETTLE_ID);
        assert_eq!(packet.layer_classification, InteractionFailureLayer::Layout);
        assert_eq!(packet.failing_primitive_step_index, Some(3));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("layout_clean_at_settle_boundary")
        );
        assert_eq!(packet.progress_marker_history, progress_markers);

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::DirtyLayoutAtSettleBoundary
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::DirtyLayoutAtSettleBoundary
        );
        assert_eq!(fields.failing_command_index, 3);
    }

    #[test]
    fn chaos_router_contradiction_detector_uses_captured_route_marker() {
        let markers = vec![InteractionProgressMarker {
            command_index: 5,
            frame_index: 10,
            layout_epoch: 3,
            scene_epoch: 5,
            dirty_state_hash: "stable".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "captured:header_separator".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "body_row".to_string(),
        }];

        let detection = detect_router_contradiction_failure(&markers)
            .expect("divergent captured route marker must be detected");

        assert_eq!(detection.command_index, 5);
        assert_eq!(
            detection.capture_owner_class.as_str(),
            "captured:header_separator"
        );
        assert_eq!(detection.routed_target_class.as_str(), "body_row");
        assert_eq!(
            detection.failed_requirement,
            "captured_route_target_coherent"
        );
    }

    #[test]
    fn chaos_router_contradiction_failure_packet_records_divergent_route() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![InteractionProgressMarker {
            command_index: 2,
            frame_index: 6,
            layout_epoch: 1,
            scene_epoch: 3,
            dirty_state_hash: "stable".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "captured:header_separator".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "body_row".to_string(),
        }];
        let detection = detect_router_contradiction_failure(&progress_markers)
            .expect("divergent captured route marker must be detected");

        let packet = build_chaos_router_contradiction_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            detection,
        );

        assert_eq!(
            packet.invariant_id,
            ORACLE_346_CHAOS_ROUTER_CONTRADICTION_ID
        );
        assert_eq!(
            packet.layer_classification,
            InteractionFailureLayer::EventRouting
        );
        assert_eq!(packet.failing_primitive_step_index, Some(2));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("captured_route_target_coherent")
        );
        assert_eq!(packet.progress_marker_history, progress_markers);

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::EventRouterContradiction
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::EventRouterContradiction
        );
        assert_eq!(fields.failing_command_index, 2);
    }

    #[test]
    fn chaos_authority_violation_detector_uses_constitution_boundary() {
        let markers = vec![InteractionProgressMarker {
            command_index: 6,
            frame_index: 11,
            layout_epoch: 3,
            scene_epoch: 5,
            dirty_state_hash: "authority_violation:layout".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "none".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];

        let detection = detect_authority_violation_failure(&markers)
            .expect("authority violation marker must be detected");

        assert_eq!(detection.command_index, 6);
        assert_eq!(detection.authority_domain.as_str(), "layout");
        assert_eq!(
            detection.violation_class.as_str(),
            "no_forbidden_boundary_action"
        );
        assert!(!detection.evaluation.passed());
        assert_eq!(
            detection.evaluation.failed_requirement.as_deref(),
            Some("no_forbidden_boundary_action")
        );
    }

    #[test]
    fn chaos_authority_violation_failure_packet_records_constitution_evidence() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![InteractionProgressMarker {
            command_index: 2,
            frame_index: 6,
            layout_epoch: 1,
            scene_epoch: 3,
            dirty_state_hash: "authority_violation:layout".to_string(),
            deferred_mutation_count: 0,
            capture_owner_class: "none".to_string(),
            focus_owner_class: "none".to_string(),
            routed_target_class: "header_separator".to_string(),
        }];
        let detection = detect_authority_violation_failure(&progress_markers)
            .expect("authority violation marker must be detected");

        let packet = build_chaos_authority_violation_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            detection,
        );

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_AUTHORITY_VIOLATION_ID);
        assert_eq!(packet.layer_classification, InteractionFailureLayer::Oracle);
        assert_eq!(packet.failing_primitive_step_index, Some(2));
        assert!(!packet.constitution_boundary_check_status.passed);
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("no_forbidden_boundary_action")
        );
        assert_eq!(packet.progress_marker_history, progress_markers);

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::AuthorityViolation
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::AuthorityViolation
        );
        assert_eq!(fields.failing_command_index, 2);
    }

    #[test]
    fn chaos_replay_nondeterminism_detector_compares_replayed_artifact() {
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let mut driver = InteractionDriver::new();
        driver
            .try_execute_all(commands.iter().map(|generated| generated.command.clone()))
            .expect("expected stream must execute");
        let expected_replay = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactPrimitive,
            CHAOS_SURFACE_BASIC_ID,
            CHAOS_ROOT_SEED,
            None,
            &driver,
        );
        let mut replayed_trace = expected_replay.clone();
        replayed_trace.primitive_trace[0].emitted_event_count += 1;

        let detection =
            detect_replay_artifact_mismatch(&expected_replay, replayed_trace.clone(), None, None)
                .expect("artifact mismatch must be detected");

        assert_eq!(detection.command_index, 0);
        assert_eq!(
            detection.divergence_class.as_str(),
            "emitted_event_count_mismatch"
        );
        assert!(detection.replay_error.is_none());
        assert_ne!(
            detection.expected_trace_fingerprint,
            detection.replayed_trace_fingerprint
        );
        assert_eq!(detection.replayed_trace, replayed_trace);
        assert!(
            detect_replay_artifact_mismatch(&expected_replay, expected_replay.clone(), None, None)
                .is_none()
        );
    }

    #[test]
    fn chaos_replay_nondeterminism_failure_packet_records_artifact_divergence() {
        let surface = ci_chaos_surface();
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let mut driver = InteractionDriver::new();
        driver
            .try_execute_all(commands.iter().map(|generated| generated.command.clone()))
            .expect("expected stream must execute");
        let expected_replay = InteractionReplayArtifact::from_driver(
            ReplayMode::ExactPrimitive,
            surface.id.as_str(),
            profile.root_seed,
            None,
            &driver,
        );
        let mut replayed_trace = expected_replay.clone();
        replayed_trace.primitive_trace[0].emitted_event_count += 1;
        let detection =
            detect_replay_artifact_mismatch(&expected_replay, replayed_trace, None, None)
                .expect("artifact mismatch must be detected");
        let progress_markers = vec![chaos_progress_marker(0, &driver)];

        let packet = build_chaos_replay_nondeterminism_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &progress_markers,
            &expected_replay,
            detection,
        );

        assert_eq!(
            packet.invariant_id,
            ORACLE_346_CHAOS_REPLAY_NONDETERMINISM_ID
        );
        assert_eq!(packet.layer_classification, InteractionFailureLayer::Replay);
        assert_eq!(packet.severity, InteractionFailureSeverity::P0);
        assert_eq!(packet.failing_primitive_step_index, Some(0));
        assert_eq!(packet.primitive_trace, expected_replay.primitive_trace);
        assert_eq!(packet.shrunk_trace, expected_replay);
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("divergence_class")
                .map(String::as_str),
            Some("emitted_event_count_mismatch")
        );

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::ReplayNondeterminism
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::ReplayNondeterminism
        );
        assert_eq!(fields.replay_determinism_status.as_str(), "failed");
        assert_eq!(fields.failing_command_index, 0);
        assert_eq!(packet.progress_marker_history, progress_markers);
    }

    #[test]
    fn chaos_unsupported_command_detector_rejects_disallowed_primitive_family() {
        let mut surface = ci_chaos_surface();
        surface
            .allowed_primitive_families
            .retain(|family| *family != InteractionChaosPrimitiveFamily::WindowSize);
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);

        let detection =
            detect_unsupported_command_or_domain_skip(&profile, &surface, 0, &commands[0])
                .expect("disallowed primitive family must be detected");

        assert_eq!(detection.command_index, 0);
        assert_eq!(detection.unsupported_class.as_str(), "primitive_family");
        assert_eq!(detection.command_family.as_str(), "window_size");
        assert_eq!(detection.command_domain.as_str(), "window_configuration");
        assert_eq!(detection.failed_requirement, "allowed_primitive_family");
    }

    #[test]
    fn chaos_unsupported_domain_failure_packet_records_skip_evidence() {
        let mut surface = ci_chaos_surface();
        surface
            .disabled_domains
            .push("window_configuration".to_string());
        let profile = ci_chaos_profile(&surface);
        let commands = deterministic_chaos_stream(CHAOS_ROOT_SEED, 4);
        let generated_primitive_commands = replay_commands(&commands);
        let driver = InteractionDriver::new();
        let progress_markers = vec![chaos_progress_marker(0, &driver)];
        let detection =
            detect_unsupported_command_or_domain_skip(&profile, &surface, 0, &commands[0])
                .expect("disabled domain command must be detected");

        let packet = build_chaos_unsupported_command_or_domain_failure_packet(
            &profile,
            &surface,
            &commands,
            &generated_primitive_commands,
            &driver,
            &progress_markers,
            detection,
        );

        assert_eq!(packet.invariant_id, ORACLE_346_CHAOS_UNSUPPORTED_SKIP_ID);
        assert_eq!(
            packet.layer_classification,
            InteractionFailureLayer::PrimitiveInjection
        );
        assert_eq!(packet.failing_primitive_step_index, Some(0));
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("failed_requirement")
                .map(String::as_str),
            Some("no_disabled_domain_command")
        );
        assert_eq!(
            packet
                .observed_state
                .fields
                .get("command_domain")
                .map(String::as_str),
            Some("window_configuration")
        );

        let fields = packet
            .chaos_failure_fields
            .as_ref()
            .expect("chaos fields required");
        assert_eq!(
            fields.failure_detector,
            InteractionChaosFailureDetector::UnsupportedCommandOrDomainSkip
        );
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::UnsupportedDomainSkip
        );
        assert_eq!(
            fields.unsupported_domains,
            vec!["window_configuration".to_string()]
        );
        assert_eq!(fields.failing_command_index, 0);
        assert_eq!(packet.progress_marker_history, progress_markers);
    }

    #[test]
    fn chaos_shrinker_minimizes_non_finite_sink_without_changing_boundary_or_class() {
        let (packet, failing_index, expected_stream_len) = non_finite_chaos_failure_packet();

        let report = shrink_chaos_failure_packet(&packet).expect("chaos shrink must succeed");

        assert_eq!(report.failure_id, packet.failure_id);
        assert_eq!(report.replay_mode, ReplayMode::ShrunkTrace);
        assert_eq!(
            report.injection_path,
            InteractionInjectionPath::RawInputFusion
        );
        assert!(report.preserved_injection_boundary);
        assert!(report.preserved_hard_failure_class);
        assert_eq!(
            report.constitution_boundary_profile.shrinking_owner,
            packet.constitution_boundary_profile.shrinking_owner
        );
        assert!(report.constitution_boundary_check_status.passed);
        assert_eq!(
            report.failure_detector,
            InteractionChaosFailureDetector::NonFiniteSink
        );
        assert_eq!(
            report.hard_failure_class,
            InteractionChaosHardFailureClass::NonFiniteFinalSink
        );
        assert_eq!(report.original_primitive_count, expected_stream_len);
        assert_eq!(report.original_failing_command_index, failing_index);
        assert_eq!(report.shrunk_primitive_count, 1);
        assert_eq!(report.shrunk_failing_command_index, 0);
        assert!(report.candidate_count >= report.accepted_reduction_count);
        assert!(report.accepted_reduction_count > 0);
        assert_eq!(
            report.shrunk_trace.primitive_commands().len(),
            report.shrunk_primitive_count
        );

        let mut replay_driver = InteractionDriver::new();
        let replay_result = replay_driver.try_execute_all(report.shrunk_trace.primitive_commands());
        assert!(matches!(
            replay_result,
            Err(InteractionDriverError::NonFiniteSinkValue {
                primitive_step_index: 0,
                ..
            })
        ));

        let json = serde_json::to_string(&report).expect("shrink report must serialize");
        let decoded: InteractionChaosShrinkReport =
            serde_json::from_str(&json).expect("shrink report must deserialize");
        assert_eq!(decoded.shrunk_primitive_count, 1);
        assert_eq!(decoded.hard_failure_class, report.hard_failure_class);
        assert!(decoded.constitution_boundary_check_status.passed);
    }

    #[test]
    fn chaos_failure_corpus_entry_records_shrunk_primitive_trace_when_present() {
        let (packet, _failing_index, _expected_stream_len) = non_finite_chaos_failure_packet();
        let shrink_report =
            shrink_chaos_failure_packet(&packet).expect("chaos shrink must succeed");

        let corpus_entry =
            chaos_failure_corpus_entry(&packet, &shrink_report).expect("corpus entry required");

        assert_eq!(corpus_entry.fixture_id, packet.fixture_id);
        assert_eq!(
            corpus_entry.chaos_surface_id.as_str(),
            CHAOS_SURFACE_BASIC_ID
        );
        assert_eq!(
            corpus_entry.chaos_stream_generator_id.as_str(),
            CHAOS_STREAM_GENERATOR_ID
        );
        assert_eq!(
            corpus_entry.chaos_stream_generator_version.as_str(),
            CHAOS_STREAM_GENERATOR_VERSION
        );
        assert_eq!(corpus_entry.seed, CHAOS_ROOT_SEED);
        assert_eq!(
            corpus_entry.hard_failure_class,
            Some(InteractionChaosHardFailureClass::NonFiniteFinalSink)
        );
        assert_eq!(corpus_entry.shrunk_primitive_trace.len(), 1);
        assert_eq!(
            corpus_entry.shrunk_primitive_trace[0].primitive_step_index,
            shrink_report.shrunk_trace.primitive_trace[0].primitive_step_index
        );
        assert!(matches!(
            corpus_entry.shrunk_primitive_trace[0].command,
            ReplayInteractionCommand::PointerMove { x, y } if x.is_nan() && y == 1.0
        ));
        assert!(corpus_entry.shrunk_trace.is_some());
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.failure_detector.non_finite_sink".to_string())
        );
        assert!(
            corpus_entry
                .coverage_keys_gained
                .contains(&"chaos.hard_failure_class.non_finite_final_sink".to_string())
        );

        let json = serde_json::to_string(&corpus_entry).expect("chaos corpus entry must serialize");
        let decoded: InteractionChaosCorpusEntry =
            serde_json::from_str(&json).expect("chaos corpus entry must deserialize");
        assert_eq!(decoded.shrunk_primitive_trace.len(), 1);
        assert_eq!(
            decoded.hard_failure_class,
            Some(InteractionChaosHardFailureClass::NonFiniteFinalSink)
        );
    }

    #[test]
    fn chaos_report_coverage_summary_is_present_but_not_a_confidence_claim() {
        let report =
            expect_chaos_report(run_ci_chaos_smoke_profile().expect("chaos smoke must run"));

        assert_eq!(report.coverage_summary.total_keys, 0);
        assert_eq!(
            report.coverage_summary.exercised_keys,
            Vec::<CoverageKey>::new()
        );
        assert!(report.pressure_coverage_summary.total_keys > 0);
        assert!(report.pressure_coverage_summary.frontier_saturated);
        assert_eq!(
            report.pressure_coverage_summary.missing_required_dimensions,
            Vec::<String>::new()
        );
        assert!(
            report
                .pressure_coverage_summary
                .exercised_dimensions
                .contains(&"primitive_command_class".to_string())
        );
        assert!(!report.confidence_packet_emitted);
    }
}
