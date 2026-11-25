//! SPEC-HAL-346 bounded CI smoke profile for interaction fuzzing.

use std::collections::{BTreeSet, VecDeque};

use super::constitution::{
    InteractionFuzzConstitutionCheck, InteractionFuzzRole, ORACLE_346_C1_ID,
    constitution_boundary_contracts, evaluate_constitution_boundary,
};
use super::generators::{
    GEN_346_FOC1_ID, GEN_346_KBD1_ID, GEN_346_PTH1_ID, GEN_346_PTH2_ID, GEN_346_PTH3_ID,
    GEN_346_RSZ1_ID, GEN_346_RSZ2_ID, GEN_346_RSZ3_ID, GEN_346_RSZ4_ID, GEN_346_SCR1_ID,
    GeneratedInteractionScenario, GeneratedInteractionTarget, GeneratedTargetClass,
    generate_scenarios_for_fixture, interaction_generator_registry,
};
use super::list_view_resize::{
    ORACLE_346_R2_ID, ORACLE_346_R4_ID, ORACLE_346_R5_ID, ORACLE_346_R12_ID,
    list_view_resize_smoke_oracle_evaluations,
};
use super::registry::{
    LIST_VIEW_RESIZE_LV1_FIXTURE_ID, LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
    TREE_VIEW_PROJECTION_TV1_FIXTURE_ID, find_interaction_fixture,
};
use crate::core::interaction_fuzz::{
    CoverageKey, EndpointTeleportationMode, FiniteGeometrySink, FiniteSinkGeometrySample,
    INTERACTION_FUZZ_SCHEMA, INTERACTION_REPLAY_SCHEMA_VERSION, IncidentalNavigationSnapshot,
    IntentTraceStep, InteractionConfidencePacket, InteractionConfidencePacketInput,
    InteractionConstitutionBoundaryCheckStatus, InteractionConstitutionBoundaryProfile,
    InteractionCoverageSummary, InteractionDriver, InteractionDriverError, InteractionElapsedTime,
    InteractionFuzzMode, InteractionFuzzProfile, InteractionGeneratorVersionRecord,
    InteractionInjectionPath, InteractionOracleEvaluation, InteractionOracleStatus,
    InteractionSaturationSummary, InteractionSettleBoundary, InteractionTargetSnapshot,
    InteractionTimeoutPolicy, InteractionVisualDiagnosticsPolicy, KeyboardNavigationPolicy,
    ORACLE_346_U1_ID, ORACLE_346_U11_ID, PointerCaptureTransitionClass, PointerMoveExposureClass,
    PointerPathPolicyClass, PointerPathSampleCountClass, ReplayInteractionIntentCommand,
    evaluate_finite_sink_geometry, evaluate_incidental_navigation_integrity,
};

pub const CI_SMOKE_PROFILE_ID: &str = "interaction-fuzz-ci-smoke";
const CI_SMOKE_FIXTURE_IDS: &[&str] = &[
    LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
    LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
    TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
];
const CI_SMOKE_ROOT_SEEDS: &[u64] = &[0x3460_C100, 0x3460_C101];
const CI_SMOKE_SATURATION_WINDOW: usize = 2;
const CI_SMOKE_TRANSACTION_BUDGET: usize = 36;
const CI_SMOKE_COMMAND_BUDGET: usize = 256;
const CI_SMOKE_ARTIFACT_DIR: &str = "target/interaction-fuzz/ci-smoke";
const CI_SMOKE_RNG_ALGORITHM: &str = "halcyon-fixed-seed";
const CI_SMOKE_RNG_VERSION: &str = "1";
const CI_SMOKE_GENERATOR_VERSION: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionSmokeProfile {
    pub profile_id: &'static str,
    pub fixture_ids: &'static [&'static str],
    pub root_seeds: &'static [u64],
    pub saturation_window: usize,
    pub transaction_budget: usize,
    pub strict_failure_on_invariant_violation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionSmokeHaltReason {
    CoverageSaturation,
    TransactionBudget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionSmokeReport {
    pub profile_id: &'static str,
    pub halt_reason: InteractionSmokeHaltReason,
    pub saturation_window: usize,
    pub final_window_new_keys: usize,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub confidence_packet: InteractionConfidencePacket,
    pub oracle_evaluations: Vec<InteractionOracleEvaluation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionSmokeFailure {
    MissingFixture {
        fixture_id: &'static str,
    },
    Driver(InteractionDriverError),
    OracleFailure {
        failed_oracles: Vec<InteractionOracleEvaluation>,
    },
    CoverageUnsaturated {
        frontier_keys_seen: usize,
        frontier_keys_required: usize,
    },
}

impl From<InteractionDriverError> for InteractionSmokeFailure {
    fn from(value: InteractionDriverError) -> Self {
        Self::Driver(value)
    }
}

pub const CI_SMOKE_PROFILE: InteractionSmokeProfile = InteractionSmokeProfile {
    profile_id: CI_SMOKE_PROFILE_ID,
    fixture_ids: CI_SMOKE_FIXTURE_IDS,
    root_seeds: CI_SMOKE_ROOT_SEEDS,
    saturation_window: CI_SMOKE_SATURATION_WINDOW,
    transaction_budget: CI_SMOKE_TRANSACTION_BUDGET,
    strict_failure_on_invariant_violation: true,
};

pub fn ci_smoke_profile() -> InteractionSmokeProfile {
    CI_SMOKE_PROFILE
}

pub fn run_ci_smoke_profile() -> Result<InteractionSmokeReport, InteractionSmokeFailure> {
    run_smoke_profile(CI_SMOKE_PROFILE)
}

pub(crate) fn ci_smoke_runner_profile() -> Result<InteractionFuzzProfile, InteractionSmokeFailure> {
    let declared_scenarios = declared_scenarios(CI_SMOKE_PROFILE)?;
    let declared_generator_ids = declared_generator_ids(&declared_scenarios)
        .iter()
        .map(|generator_id| (*generator_id).to_string())
        .collect::<Vec<_>>();

    Ok(ci_smoke_interaction_fuzz_profile(
        CI_SMOKE_PROFILE,
        declared_generator_ids,
        ci_smoke_declared_oracle_ids(),
    ))
}

pub fn run_smoke_profile(
    profile: InteractionSmokeProfile,
) -> Result<InteractionSmokeReport, InteractionSmokeFailure> {
    let declared_scenarios = declared_scenarios(profile)?;
    let declared_frontier = declared_frontier(&declared_scenarios);
    let declared_generator_ids = declared_generator_ids(&declared_scenarios);
    let mut oracle_evaluations = constitution_smoke_evaluations();
    oracle_evaluations.extend(list_view_resize_smoke_oracle_evaluations());
    let failed_preflight_oracles: Vec<InteractionOracleEvaluation> = oracle_evaluations
        .iter()
        .filter(|evaluation| evaluation.status == InteractionOracleStatus::Failed)
        .cloned()
        .collect();

    if profile.strict_failure_on_invariant_violation && !failed_preflight_oracles.is_empty() {
        return Err(InteractionSmokeFailure::OracleFailure {
            failed_oracles: failed_preflight_oracles,
        });
    }

    let mut driver = InteractionDriver::new();
    let mut executed_fixture_ids = BTreeSet::new();
    let mut executed_generator_ids = BTreeSet::new();
    let mut used_seed_set = BTreeSet::new();
    let mut incidental_target_classes = BTreeSet::new();
    let mut window_new_keys = VecDeque::new();
    let mut halt_reason = InteractionSmokeHaltReason::TransactionBudget;

    for scenario in declared_scenarios.iter() {
        let next_transaction_count = driver.intent_trace().len() + scenario.intent_commands.len();
        if next_transaction_count > profile.transaction_budget {
            halt_reason = InteractionSmokeHaltReason::TransactionBudget;
            break;
        }

        let before_coverage: BTreeSet<CoverageKey> = driver.coverage().keys().cloned().collect();
        let before_intent_count = driver.intent_trace().len();
        scenario.execute(&mut driver)?;
        let scenario_trace = &driver.intent_trace()[before_intent_count..];
        oracle_evaluations.extend(scenario_incidental_navigation_evaluations(
            scenario,
            scenario_trace,
        ));
        let after_coverage: BTreeSet<CoverageKey> = driver.coverage().keys().cloned().collect();
        let new_key_count = after_coverage.difference(&before_coverage).count();
        window_new_keys.push_back(new_key_count);
        if window_new_keys.len() > profile.saturation_window {
            window_new_keys.pop_front();
        }

        executed_fixture_ids.insert(scenario.fixture_id);
        executed_generator_ids.insert(scenario.generator_id);
        used_seed_set.insert(scenario.root_seed);
        for target in scenario.incidental_targets.iter() {
            incidental_target_classes.insert(target_class_token(target.class).to_string());
        }

        if frontier_saturated(&declared_frontier, driver.coverage().keys())
            && executed_generator_ids == declared_generator_ids
            && executed_fixture_ids.len() == profile.fixture_ids.len()
            && used_seed_set.len() == profile.root_seeds.len()
            && window_new_keys.len() == profile.saturation_window
            && window_new_keys.iter().all(|count| *count == 0)
        {
            halt_reason = InteractionSmokeHaltReason::CoverageSaturation;
            break;
        }
    }

    let frontier_keys_seen = declared_frontier
        .iter()
        .filter(|key| driver.coverage().contains(key))
        .count();
    let frontier_keys_required = declared_frontier.len();
    let frontier_saturated = frontier_keys_seen == frontier_keys_required;

    if frontier_saturated
        && executed_generator_ids == declared_generator_ids
        && executed_fixture_ids.len() == profile.fixture_ids.len()
        && used_seed_set.len() == profile.root_seeds.len()
    {
        halt_reason = InteractionSmokeHaltReason::CoverageSaturation;
    }

    if profile.strict_failure_on_invariant_violation && !frontier_saturated {
        return Err(InteractionSmokeFailure::CoverageUnsaturated {
            frontier_keys_seen,
            frontier_keys_required,
        });
    }

    let failed_oracles: Vec<InteractionOracleEvaluation> = oracle_evaluations
        .iter()
        .filter(|evaluation| evaluation.status == InteractionOracleStatus::Failed)
        .cloned()
        .collect();

    if profile.strict_failure_on_invariant_violation && !failed_oracles.is_empty() {
        return Err(InteractionSmokeFailure::OracleFailure { failed_oracles });
    }

    let coverage_summary = InteractionCoverageSummary::from_coverage(
        driver.coverage(),
        incidental_target_classes.into_iter(),
    );
    let oracle_set: Vec<String> = oracle_evaluations
        .iter()
        .map(|evaluation| evaluation.oracle_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let profile_generator_set: Vec<String> = declared_generator_ids
        .iter()
        .map(|generator_id| (*generator_id).to_string())
        .collect();
    let fuzz_profile =
        ci_smoke_interaction_fuzz_profile(profile, profile_generator_set, oracle_set.clone());
    let run_profile_hash = fuzz_profile.profile_hash();
    let constitution_boundary_profile = fuzz_profile.constitution_boundary_profile.clone();
    let constitution_boundary_check_status =
        InteractionConstitutionBoundaryCheckStatus::from_oracle_evaluations(&oracle_evaluations);
    let fixture_set: Vec<String> = executed_fixture_ids
        .iter()
        .map(|fixture_id| (*fixture_id).to_string())
        .collect();
    let generator_set: Vec<String> = executed_generator_ids
        .iter()
        .map(|generator_id| (*generator_id).to_string())
        .collect();
    let seed_set: Vec<u64> = used_seed_set.into_iter().collect();
    let disabled_generators = disabled_generators(&executed_generator_ids);
    let failure_count = failed_oracles.len() + usize::from(!frontier_saturated);
    let confidence_packet =
        InteractionConfidencePacket::from_input(InteractionConfidencePacketInput {
            run_id: profile.profile_id.to_string(),
            run_profile_id: fuzz_profile.profile_id.clone(),
            run_profile_hash,
            halcyon_version: env!("CARGO_PKG_VERSION").to_string(),
            git_revision: "test-revision".to_string(),
            fixture_set,
            generator_set,
            oracle_set,
            seed_set,
            injection_path: InteractionInjectionPath::IntentExpansion,
            settle_boundary: InteractionSettleBoundary::Reconcile,
            transaction_count: driver.intent_trace().len(),
            coverage_summary,
            saturation_summary: InteractionSaturationSummary {
                frontier_keys_seen,
                frontier_keys_required,
                frontier_saturated,
            },
            elapsed_time: InteractionElapsedTime {
                milliseconds: driver.intent_trace().len() as u64,
            },
            failure_count,
            skipped_oracles: Vec::new(),
            disabled_generators,
            out_of_scope_domains: Vec::new(),
            timeout_policy: fuzz_profile.timeout_policy.clone(),
            visual_diagnostics_status: fuzz_profile.visual_diagnostics_policy.status(),
            constitution_boundary_profile: constitution_boundary_profile.clone(),
            constitution_boundary_check_status: constitution_boundary_check_status.clone(),
        });

    Ok(InteractionSmokeReport {
        profile_id: profile.profile_id,
        halt_reason,
        saturation_window: profile.saturation_window,
        final_window_new_keys: window_new_keys.iter().sum(),
        constitution_boundary_profile,
        constitution_boundary_check_status,
        confidence_packet,
        oracle_evaluations,
    })
}

fn ci_smoke_interaction_fuzz_profile(
    profile: InteractionSmokeProfile,
    generator_ids: Vec<String>,
    oracle_ids: Vec<String>,
) -> InteractionFuzzProfile {
    let root_seed = profile
        .root_seeds
        .first()
        .copied()
        .expect("smoke profile must declare a root seed");
    let generator_versions = generator_ids
        .iter()
        .map(|generator_id| InteractionGeneratorVersionRecord {
            generator_id: generator_id.clone(),
            version: CI_SMOKE_GENERATOR_VERSION.to_string(),
        })
        .collect();

    InteractionFuzzProfile {
        schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
        schema: INTERACTION_FUZZ_SCHEMA.to_string(),
        profile_id: profile.profile_id.to_string(),
        mode: InteractionFuzzMode::ContractSmoke,
        artifact_dir: CI_SMOKE_ARTIFACT_DIR.to_string(),
        injection_path: InteractionInjectionPath::IntentExpansion,
        settle_boundary: InteractionSettleBoundary::Reconcile,
        fixture_ids: profile
            .fixture_ids
            .iter()
            .map(|fixture_id| (*fixture_id).to_string())
            .collect(),
        chaos_surface_ids: Vec::new(),
        generator_ids,
        chaos_stream_generator_ids: Vec::new(),
        generator_versions,
        chaos_stream_generator_versions: Vec::new(),
        oracle_ids: oracle_ids.clone(),
        hard_failure_oracle_ids: oracle_ids,
        root_seed,
        seed_set: profile.root_seeds.to_vec(),
        rng_algorithm: CI_SMOKE_RNG_ALGORITHM.to_string(),
        rng_version: CI_SMOKE_RNG_VERSION.to_string(),
        command_budget: CI_SMOKE_COMMAND_BUDGET,
        transaction_budget: profile.transaction_budget,
        coverage_window: profile.saturation_window,
        timeout_policy: ci_smoke_timeout_policy(root_seed),
        visual_diagnostics_policy: ci_smoke_visual_diagnostics_policy(),
        constitution_boundary_profile: ci_smoke_constitution_boundary_profile(),
        disabled_domains: Vec::new(),
        expected_unsupported_domains: Vec::new(),
    }
}

fn ci_smoke_timeout_policy(root_seed: u64) -> InteractionTimeoutPolicy {
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

fn ci_smoke_visual_diagnostics_policy() -> InteractionVisualDiagnosticsPolicy {
    InteractionVisualDiagnosticsPolicy {
        capture_enabled: false,
        headless: true,
        rendering_available: false,
    }
}

fn ci_smoke_constitution_boundary_profile() -> InteractionConstitutionBoundaryProfile {
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

fn declared_scenarios(
    profile: InteractionSmokeProfile,
) -> Result<Vec<GeneratedInteractionScenario>, InteractionSmokeFailure> {
    let mut scenarios = Vec::new();
    for root_seed in profile.root_seeds {
        for fixture_id in profile.fixture_ids {
            let fixture = find_interaction_fixture(fixture_id)
                .ok_or(InteractionSmokeFailure::MissingFixture { fixture_id })?;
            scenarios.extend(generate_scenarios_for_fixture(fixture, *root_seed));
        }
    }
    Ok(scenarios)
}

fn declared_frontier(scenarios: &[GeneratedInteractionScenario]) -> BTreeSet<CoverageKey> {
    scenarios
        .iter()
        .flat_map(|scenario| scenario.expected_coverage.iter().cloned())
        .collect()
}

fn declared_generator_ids(scenarios: &[GeneratedInteractionScenario]) -> BTreeSet<&'static str> {
    scenarios
        .iter()
        .map(|scenario| scenario.generator_id)
        .collect()
}

fn constitution_smoke_evaluations() -> Vec<InteractionOracleEvaluation> {
    constitution_boundary_contracts()
        .iter()
        .map(|contract| {
            evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
                role: contract.role,
                responsibilities: contract.allowed_responsibilities.to_vec(),
                forbidden_actions: Vec::new(),
                boundary_narrowed: false,
                full_confidence_claim: true,
                authority: role_authority(contract.role),
            })
        })
        .collect()
}

fn ci_smoke_declared_oracle_ids() -> Vec<String> {
    let mut oracle_ids = constitution_smoke_evaluations()
        .iter()
        .map(|evaluation| evaluation.oracle_id.clone())
        .collect::<BTreeSet<_>>();
    oracle_ids.insert(ORACLE_346_U1_ID.to_string());
    oracle_ids.insert(ORACLE_346_U11_ID.to_string());
    oracle_ids.insert(ORACLE_346_R2_ID.to_string());
    oracle_ids.insert(ORACLE_346_R4_ID.to_string());
    oracle_ids.insert(ORACLE_346_R5_ID.to_string());
    oracle_ids.insert(ORACLE_346_R12_ID.to_string());
    oracle_ids.into_iter().collect()
}

fn scenario_incidental_navigation_evaluations(
    scenario: &GeneratedInteractionScenario,
    trace_steps: &[IntentTraceStep],
) -> Vec<InteractionOracleEvaluation> {
    let Some(active_step) = trace_steps.first() else {
        return Vec::new();
    };

    scenario
        .incidental_targets
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(incidental_index, incidental_target)| {
            let universal_oracle_evaluations = smoke_incidental_universal_oracle_evaluations(
                scenario.fixture_id,
                incidental_index,
                incidental_target,
            );
            let incidental_navigation_evaluation =
                evaluate_incidental_navigation_integrity(IncidentalNavigationSnapshot {
                    active_intent_index: active_step.intent_index,
                    primitive_step_index: active_step.primitive_command_range.start,
                    active_intent_command: Some(ReplayInteractionIntentCommand::from(
                        &active_step.intent_command,
                    )),
                    intended_target: smoke_target_snapshot(
                        scenario.fixture_id,
                        "intended",
                        0,
                        scenario.intended_target,
                    ),
                    incidental_target: smoke_target_snapshot(
                        scenario.fixture_id,
                        "incidental",
                        incidental_index,
                        incidental_target,
                    ),
                    path_policy: active_step.path_policy.clone(),
                    side_effects_suppressed: false,
                    universal_oracle_evaluations: universal_oracle_evaluations.clone(),
                    authority: "interaction_smoke_runner".to_string(),
                });
            universal_oracle_evaluations
                .into_iter()
                .chain(std::iter::once(incidental_navigation_evaluation))
        })
        .collect()
}

fn smoke_incidental_universal_oracle_evaluations(
    fixture_id: &'static str,
    incidental_index: usize,
    target: GeneratedInteractionTarget,
) -> Vec<InteractionOracleEvaluation> {
    let mut samples = Vec::new();
    if let Some(point) = target.point {
        let target_class = smoke_oracle_target_class_token(target.class);
        samples.push(FiniteSinkGeometrySample::new(
            FiniteGeometrySink::HitTestBounds,
            format!("{fixture_id}.incidental.{target_class}.{incidental_index}.x"),
            "interaction_smoke_runner",
            point.x,
        ));
        samples.push(FiniteSinkGeometrySample::new(
            FiniteGeometrySink::HitTestBounds,
            format!("{fixture_id}.incidental.{target_class}.{incidental_index}.y"),
            "interaction_smoke_runner",
            point.y,
        ));
    }

    vec![evaluate_finite_sink_geometry(samples)]
}

fn smoke_target_snapshot(
    fixture_id: &'static str,
    role: &'static str,
    ordinal: usize,
    target: GeneratedInteractionTarget,
) -> InteractionTargetSnapshot {
    let target_class = smoke_oracle_target_class_token(target.class);
    InteractionTargetSnapshot {
        target_id: Some(format!("{fixture_id}.{role}.{target_class}.{ordinal}")),
        target_kind: smoke_oracle_target_kind(target.class).to_string(),
        target_class: Some(target_class.to_string()),
    }
}

fn frontier_saturated<'a>(
    declared_frontier: &BTreeSet<CoverageKey>,
    observed_keys: impl Iterator<Item = &'a CoverageKey>,
) -> bool {
    let observed_keys: BTreeSet<CoverageKey> = observed_keys.cloned().collect();
    declared_frontier
        .iter()
        .all(|key| observed_keys.contains(key))
}

fn disabled_generators(executed_generator_ids: &BTreeSet<&'static str>) -> Vec<String> {
    interaction_generator_registry()
        .iter()
        .map(|definition| definition.generator_id)
        .filter(|generator_id| !executed_generator_ids.contains(generator_id))
        .map(str::to_string)
        .collect()
}

fn role_authority(role: InteractionFuzzRole) -> &'static str {
    match role {
        InteractionFuzzRole::Generator => "interaction_generator_registry",
        InteractionFuzzRole::Probe => "interaction_probe",
        InteractionFuzzRole::Oracle => "interaction_oracle",
        InteractionFuzzRole::Shrinker => "interaction_shrinker",
        InteractionFuzzRole::Replay => "interaction_replay",
        InteractionFuzzRole::Reporter => "interaction_reporter",
    }
}

fn target_class_token(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "row_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

fn smoke_oracle_target_class_token(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "body_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "header_separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

fn smoke_oracle_target_kind(class: GeneratedTargetClass) -> &'static str {
    match class {
        GeneratedTargetClass::OutsideListView => "outside_list_view",
        GeneratedTargetClass::RowCell => "body_cell",
        GeneratedTargetClass::HeaderCell => "header_cell",
        GeneratedTargetClass::Separator => "column_separator",
        GeneratedTargetClass::KeyboardTraversalStep => "keyboard_traversal_step",
        GeneratedTargetClass::KeyboardActivationTarget => "keyboard_activation_target",
        GeneratedTargetClass::ResizeDragEndpoint => "resize_drag_endpoint",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::interaction_fuzz::INTERACTION_CONFIDENCE_POSITIVE_CLAIM;

    #[test]
    fn ci_smoke_profile_declares_bounded_frontier() {
        let profile = ci_smoke_profile();

        assert_eq!(profile.profile_id, CI_SMOKE_PROFILE_ID);
        assert_eq!(
            profile.fixture_ids,
            &[
                LIST_VIEW_RESIZE_LV1_FIXTURE_ID,
                LIST_VIEW_RESIZE_LV2_FIXTURE_ID,
                TREE_VIEW_PROJECTION_TV1_FIXTURE_ID,
            ]
        );
        assert_eq!(profile.root_seeds, &[0x3460_C100, 0x3460_C101]);
        assert_eq!(profile.saturation_window, 2);
        assert_eq!(profile.transaction_budget, 36);
        assert!(profile.strict_failure_on_invariant_violation);

        let scenarios = declared_scenarios(profile).expect("fixtures must exist");
        let declared_fixture_ids = scenarios
            .iter()
            .map(|scenario| scenario.fixture_id)
            .collect::<std::collections::BTreeSet<_>>();
        let declared_transaction_count = scenarios
            .iter()
            .map(|scenario| scenario.intent_commands.len())
            .sum::<usize>();
        assert_eq!(
            declared_fixture_ids,
            profile
                .fixture_ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(profile.transaction_budget, declared_transaction_count);
    }

    #[test]
    fn ci_smoke_profile_expands_to_complete_core_profile() {
        let profile = ci_smoke_profile();
        let scenarios = declared_scenarios(profile).expect("fixtures must exist");
        let generator_ids: Vec<String> = declared_generator_ids(&scenarios)
            .iter()
            .map(|generator_id| (*generator_id).to_string())
            .collect();
        let oracle_ids = ci_smoke_declared_oracle_ids();
        let fuzz_profile = ci_smoke_interaction_fuzz_profile(profile, generator_ids, oracle_ids);

        assert!(fuzz_profile.is_complete());
        assert_eq!(fuzz_profile.profile_id, CI_SMOKE_PROFILE_ID);
        assert_eq!(fuzz_profile.mode, InteractionFuzzMode::ContractSmoke);
        assert_eq!(fuzz_profile.artifact_dir, CI_SMOKE_ARTIFACT_DIR);
        assert_eq!(fuzz_profile.command_budget, CI_SMOKE_COMMAND_BUDGET);
        assert_eq!(
            fuzz_profile.generator_versions.len(),
            fuzz_profile.generator_ids.len()
        );
        assert!(
            fuzz_profile
                .generator_versions
                .iter()
                .all(|record| record.version == CI_SMOKE_GENERATOR_VERSION)
        );
        assert_eq!(
            fuzz_profile.timeout_policy,
            ci_smoke_timeout_policy(0x3460_C100)
        );
        assert!(fuzz_profile.visual_diagnostics_policy.status().unavailable);
        assert_eq!(fuzz_profile.profile_hash().len(), "sha256:".len() + 64);
    }

    #[test]
    fn ci_smoke_run_emits_bounded_confidence_packet_artifact() {
        let report = run_ci_smoke_profile().expect("ci smoke profile must pass");
        let packet = &report.confidence_packet;

        assert_eq!(
            report.halt_reason,
            InteractionSmokeHaltReason::CoverageSaturation
        );
        assert_eq!(report.final_window_new_keys, 0);
        assert_eq!(packet.failure_count, 0);
        assert_eq!(packet.run_id, CI_SMOKE_PROFILE_ID);
        assert_eq!(packet.run_profile_id, CI_SMOKE_PROFILE_ID);
        assert!(packet.run_profile_hash.starts_with("sha256:"));
        assert_eq!(packet.halcyon_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(packet.positive_claim, INTERACTION_CONFIDENCE_POSITIVE_CLAIM);
        assert_eq!(packet.timeout_policy, ci_smoke_timeout_policy(0x3460_C100));
        assert!(packet.visual_diagnostics_status.unavailable);
        assert_eq!(
            packet.visual_diagnostics_status.reason,
            Some("headless".to_string())
        );
        assert_eq!(packet.visual_diagnostics_status.profile_policy, "headless");
        assert_eq!(
            packet.constitution_boundary_profile.input_fusion_owner,
            "InputFusion"
        );
        assert!(packet.constitution_boundary_check_status.passed);
        assert_eq!(
            report.constitution_boundary_profile,
            packet.constitution_boundary_profile
        );
        assert_eq!(
            report.constitution_boundary_check_status,
            packet.constitution_boundary_check_status
        );
        assert_eq!(
            packet.constitution_boundary_check_status.failed_checks,
            Vec::<String>::new()
        );
        assert_eq!(
            packet.fixture_set,
            vec![
                LIST_VIEW_RESIZE_LV1_FIXTURE_ID.to_string(),
                LIST_VIEW_RESIZE_LV2_FIXTURE_ID.to_string(),
                TREE_VIEW_PROJECTION_TV1_FIXTURE_ID.to_string()
            ]
        );
        assert_eq!(
            packet.generator_set,
            vec![
                GEN_346_FOC1_ID.to_string(),
                GEN_346_KBD1_ID.to_string(),
                GEN_346_PTH1_ID.to_string(),
                GEN_346_PTH2_ID.to_string(),
                GEN_346_PTH3_ID.to_string(),
                GEN_346_RSZ1_ID.to_string(),
                GEN_346_RSZ2_ID.to_string(),
                GEN_346_RSZ3_ID.to_string(),
                GEN_346_RSZ4_ID.to_string(),
                GEN_346_SCR1_ID.to_string()
            ]
        );
        assert_eq!(
            packet.oracle_set,
            vec![
                ORACLE_346_C1_ID.to_string(),
                ORACLE_346_R12_ID.to_string(),
                ORACLE_346_R2_ID.to_string(),
                ORACLE_346_R4_ID.to_string(),
                ORACLE_346_R5_ID.to_string(),
                ORACLE_346_U1_ID.to_string(),
                ORACLE_346_U11_ID.to_string()
            ]
        );
        assert!(report.oracle_evaluations.iter().any(|evaluation| {
            evaluation.oracle_id == ORACLE_346_R4_ID
                && evaluation.status == InteractionOracleStatus::Passed
        }));
        assert!(report.oracle_evaluations.iter().any(|evaluation| {
            evaluation.oracle_id == ORACLE_346_R5_ID
                && evaluation.status == InteractionOracleStatus::Passed
        }));
        assert!(report.oracle_evaluations.iter().any(|evaluation| {
            evaluation.oracle_id == ORACLE_346_U11_ID
                && evaluation.status == InteractionOracleStatus::Passed
                && evaluation
                    .derived_values
                    .get("side_effects_suppressed")
                    .is_some_and(|value| value == "false")
        }));
        assert_eq!(packet.seed_set, vec![0x3460_C100, 0x3460_C101]);
        assert_eq!(
            packet.injection_path,
            InteractionInjectionPath::IntentExpansion
        );
        assert_eq!(packet.settle_boundary, InteractionSettleBoundary::Reconcile);
        assert!(packet.transaction_count <= CI_SMOKE_TRANSACTION_BUDGET);
        assert!(packet.saturation_summary.frontier_saturated);
        assert_eq!(
            packet.saturation_summary.frontier_keys_seen,
            packet.saturation_summary.frontier_keys_required
        );
        assert_eq!(packet.disabled_generators, Vec::<String>::new());
        assert_eq!(packet.skipped_oracles, Vec::<String>::new());
        assert_eq!(packet.out_of_scope_domains, Vec::<String>::new());
        assert!(
            packet
                .coverage_summary
                .path_policy_classes
                .contains(&PointerPathPolicyClass::Linear)
        );
        assert!(
            packet
                .coverage_summary
                .path_policy_classes
                .contains(&PointerPathPolicyClass::EdgeAware)
        );
        assert!(
            packet
                .coverage_summary
                .path_policy_classes
                .contains(&PointerPathPolicyClass::Jittered)
        );
        assert_eq!(
            packet.coverage_summary.path_sample_count_classes,
            vec![PointerPathSampleCountClass::MultiSample]
        );
        assert_eq!(
            packet.coverage_summary.coalesced_move_classes,
            vec![PointerMoveExposureClass::FrameSeparated]
        );
        assert!(
            packet
                .coverage_summary
                .crossed_hover_target_classes
                .contains(&"row_cell".to_string())
        );
        assert!(
            packet
                .coverage_summary
                .crossed_separator_classes
                .contains(&"header_separator".to_string())
        );
        assert!(
            packet
                .coverage_summary
                .capture_transition_classes
                .contains(&PointerCaptureTransitionClass::PointerDown)
        );
        assert!(
            packet
                .coverage_summary
                .capture_transition_classes
                .contains(&PointerCaptureTransitionClass::DragThresholdCrossed)
        );
        assert!(
            packet
                .coverage_summary
                .capture_transition_classes
                .contains(&PointerCaptureTransitionClass::PointerUp)
        );
        assert!(
            packet
                .coverage_summary
                .incidental_route_target_classes
                .contains(&"row_cell".to_string())
        );
        assert_eq!(
            packet.coverage_summary.keyboard_navigation_policies,
            vec![
                KeyboardNavigationPolicy::ArrowTraversal,
                KeyboardNavigationPolicy::TraversalBeforeActivation
            ]
        );
        assert_eq!(
            packet.coverage_summary.endpoint_teleportation_modes,
            vec![EndpointTeleportationMode::SampledPath]
        );
        assert!(
            packet
                .coverage_summary
                .incidental_target_classes
                .contains(&"row_cell".to_string())
        );
        assert!(
            packet
                .coverage_summary
                .incidental_target_classes
                .contains(&"keyboard_traversal_step".to_string())
        );
        assert!(
            report
                .oracle_evaluations
                .iter()
                .all(InteractionOracleEvaluation::passed)
        );

        let json =
            serde_json::to_string(packet).expect("confidence packet artifact must serialize");
        let decoded: InteractionConfidencePacket =
            serde_json::from_str(&json).expect("confidence packet artifact must deserialize");
        assert_eq!(
            decoded.positive_claim,
            INTERACTION_CONFIDENCE_POSITIVE_CLAIM
        );
        assert_eq!(decoded.failure_count, 0);
    }
}
