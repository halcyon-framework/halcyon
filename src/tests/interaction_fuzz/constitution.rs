//! SPEC-HAL-346 Constitution boundary contracts for interaction fuzzing.

use std::collections::BTreeMap;

use crate::core::interaction_fuzz::{
    InteractionFailureLayer, InteractionOracleEvaluation, InteractionOracleStatus,
};

pub const ORACLE_346_C1_ID: &str = "ORACLE-346-C1";
pub const ORACLE_346_C1_NAME: &str = "Constitution Boundary Preservation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InteractionFuzzRole {
    Generator,
    Probe,
    Oracle,
    Shrinker,
    Replay,
    Reporter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InteractionFuzzResponsibility {
    EmitIntentCommands,
    ObserveRuntimeState,
    ProduceProbeSnapshot,
    EvaluateReadOnlyOracle,
    MinimizeTrace,
    ReplayRecordedTrace,
    SerializeReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InteractionFuzzForbiddenAction {
    MutateWidgetStateDirectly,
    ForceLayoutToPass,
    RebuildWidgetToPass,
    InjectOutsideDeclaredBoundary,
    SuppressRuntimeSideEffects,
    ReplaceUserPathWithInstantWithoutReduction,
    ChangeResultDuringReporting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstitutionBoundaryContract {
    pub role: InteractionFuzzRole,
    pub allowed_responsibilities: &'static [InteractionFuzzResponsibility],
    pub forbidden_actions: &'static [InteractionFuzzForbiddenAction],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionFuzzConstitutionCheck {
    pub role: InteractionFuzzRole,
    pub responsibilities: Vec<InteractionFuzzResponsibility>,
    pub forbidden_actions: Vec<InteractionFuzzForbiddenAction>,
    pub boundary_narrowed: bool,
    pub full_confidence_claim: bool,
    pub authority: &'static str,
}

const GENERATOR_RESPONSIBILITIES: &[InteractionFuzzResponsibility] =
    &[InteractionFuzzResponsibility::EmitIntentCommands];
const PROBE_RESPONSIBILITIES: &[InteractionFuzzResponsibility] = &[
    InteractionFuzzResponsibility::ObserveRuntimeState,
    InteractionFuzzResponsibility::ProduceProbeSnapshot,
];
const ORACLE_RESPONSIBILITIES: &[InteractionFuzzResponsibility] =
    &[InteractionFuzzResponsibility::EvaluateReadOnlyOracle];
const SHRINKER_RESPONSIBILITIES: &[InteractionFuzzResponsibility] =
    &[InteractionFuzzResponsibility::MinimizeTrace];
const REPLAY_RESPONSIBILITIES: &[InteractionFuzzResponsibility] =
    &[InteractionFuzzResponsibility::ReplayRecordedTrace];
const REPORTER_RESPONSIBILITIES: &[InteractionFuzzResponsibility] =
    &[InteractionFuzzResponsibility::SerializeReport];

const ALL_FORBIDDEN_ACTIONS: &[InteractionFuzzForbiddenAction] = &[
    InteractionFuzzForbiddenAction::MutateWidgetStateDirectly,
    InteractionFuzzForbiddenAction::ForceLayoutToPass,
    InteractionFuzzForbiddenAction::RebuildWidgetToPass,
    InteractionFuzzForbiddenAction::InjectOutsideDeclaredBoundary,
    InteractionFuzzForbiddenAction::SuppressRuntimeSideEffects,
    InteractionFuzzForbiddenAction::ReplaceUserPathWithInstantWithoutReduction,
    InteractionFuzzForbiddenAction::ChangeResultDuringReporting,
];

const CONSTITUTION_BOUNDARY_CONTRACTS: [ConstitutionBoundaryContract; 6] = [
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Generator,
        allowed_responsibilities: GENERATOR_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Probe,
        allowed_responsibilities: PROBE_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Oracle,
        allowed_responsibilities: ORACLE_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Shrinker,
        allowed_responsibilities: SHRINKER_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Replay,
        allowed_responsibilities: REPLAY_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
    ConstitutionBoundaryContract {
        role: InteractionFuzzRole::Reporter,
        allowed_responsibilities: REPORTER_RESPONSIBILITIES,
        forbidden_actions: ALL_FORBIDDEN_ACTIONS,
    },
];

pub fn constitution_boundary_contracts() -> &'static [ConstitutionBoundaryContract] {
    &CONSTITUTION_BOUNDARY_CONTRACTS
}

pub fn constitution_boundary_contract_for_role(
    role: InteractionFuzzRole,
) -> Option<ConstitutionBoundaryContract> {
    CONSTITUTION_BOUNDARY_CONTRACTS
        .iter()
        .copied()
        .find(|contract| contract.role == role)
}

pub fn evaluate_constitution_boundary(
    check: InteractionFuzzConstitutionCheck,
) -> InteractionOracleEvaluation {
    let Some(contract) = constitution_boundary_contract_for_role(check.role) else {
        return constitution_boundary_failed(
            check.role,
            check.authority,
            "role_has_declared_contract",
            "missing_contract".to_string(),
            derived_values(&check),
        );
    };

    if let Some(responsibility) = check
        .responsibilities
        .iter()
        .find(|responsibility| !contract.allowed_responsibilities.contains(responsibility))
        .copied()
    {
        return constitution_boundary_failed(
            check.role,
            check.authority,
            "role_responsibility_within_declared_boundary",
            responsibility_token(responsibility).to_string(),
            derived_values(&check),
        );
    }

    if let Some(action) = check
        .forbidden_actions
        .iter()
        .find(|action| contract.forbidden_actions.contains(action))
        .copied()
    {
        return constitution_boundary_failed(
            check.role,
            check.authority,
            "no_forbidden_boundary_action",
            forbidden_action_token(action).to_string(),
            derived_values(&check),
        );
    }

    if check.boundary_narrowed && check.full_confidence_claim {
        return constitution_boundary_failed(
            check.role,
            check.authority,
            "reduced_boundary_cannot_emit_full_confidence_claim",
            "full_confidence_claim".to_string(),
            derived_values(&check),
        );
    }

    InteractionOracleEvaluation {
        oracle_id: ORACLE_346_C1_ID.to_string(),
        oracle_name: ORACLE_346_C1_NAME.to_string(),
        status: InteractionOracleStatus::Passed,
        failure_layer: None,
        failed_subject: None,
        failed_requirement: None,
        failed_sink: None,
        failed_field_path: None,
        failed_authority: None,
        failed_value: None,
        derived_values: derived_values(&check),
    }
}

fn constitution_boundary_failed(
    role: InteractionFuzzRole,
    authority: &str,
    requirement: &str,
    failed_value: String,
    derived_values: BTreeMap<String, String>,
) -> InteractionOracleEvaluation {
    InteractionOracleEvaluation {
        oracle_id: ORACLE_346_C1_ID.to_string(),
        oracle_name: ORACLE_346_C1_NAME.to_string(),
        status: InteractionOracleStatus::Failed,
        failure_layer: Some(InteractionFailureLayer::Oracle),
        failed_subject: Some(role_token(role).to_string()),
        failed_requirement: Some(requirement.to_string()),
        failed_sink: None,
        failed_field_path: None,
        failed_authority: Some(authority.to_string()),
        failed_value: Some(failed_value),
        derived_values,
    }
}

fn derived_values(check: &InteractionFuzzConstitutionCheck) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    values.insert("role".to_string(), role_token(check.role).to_string());
    values.insert("authority".to_string(), check.authority.to_string());
    values.insert(
        "responsibilities".to_string(),
        check
            .responsibilities
            .iter()
            .map(|responsibility| responsibility_token(*responsibility))
            .collect::<Vec<_>>()
            .join(","),
    );
    values.insert(
        "forbidden_actions".to_string(),
        check
            .forbidden_actions
            .iter()
            .map(|action| forbidden_action_token(*action))
            .collect::<Vec<_>>()
            .join(","),
    );
    values.insert(
        "boundary_narrowed".to_string(),
        check.boundary_narrowed.to_string(),
    );
    values.insert(
        "full_confidence_claim".to_string(),
        check.full_confidence_claim.to_string(),
    );
    values
}

fn role_token(role: InteractionFuzzRole) -> &'static str {
    match role {
        InteractionFuzzRole::Generator => "generator",
        InteractionFuzzRole::Probe => "probe",
        InteractionFuzzRole::Oracle => "oracle",
        InteractionFuzzRole::Shrinker => "shrinker",
        InteractionFuzzRole::Replay => "replay",
        InteractionFuzzRole::Reporter => "reporter",
    }
}

fn responsibility_token(responsibility: InteractionFuzzResponsibility) -> &'static str {
    match responsibility {
        InteractionFuzzResponsibility::EmitIntentCommands => "emit_intent_commands",
        InteractionFuzzResponsibility::ObserveRuntimeState => "observe_runtime_state",
        InteractionFuzzResponsibility::ProduceProbeSnapshot => "produce_probe_snapshot",
        InteractionFuzzResponsibility::EvaluateReadOnlyOracle => "evaluate_read_only_oracle",
        InteractionFuzzResponsibility::MinimizeTrace => "minimize_trace",
        InteractionFuzzResponsibility::ReplayRecordedTrace => "replay_recorded_trace",
        InteractionFuzzResponsibility::SerializeReport => "serialize_report",
    }
}

fn forbidden_action_token(action: InteractionFuzzForbiddenAction) -> &'static str {
    match action {
        InteractionFuzzForbiddenAction::MutateWidgetStateDirectly => "mutate_widget_state_directly",
        InteractionFuzzForbiddenAction::ForceLayoutToPass => "force_layout_to_pass",
        InteractionFuzzForbiddenAction::RebuildWidgetToPass => "rebuild_widget_to_pass",
        InteractionFuzzForbiddenAction::InjectOutsideDeclaredBoundary => {
            "inject_outside_declared_boundary"
        }
        InteractionFuzzForbiddenAction::SuppressRuntimeSideEffects => {
            "suppress_runtime_side_effects"
        }
        InteractionFuzzForbiddenAction::ReplaceUserPathWithInstantWithoutReduction => {
            "replace_user_path_with_instant_without_reduction"
        }
        InteractionFuzzForbiddenAction::ChangeResultDuringReporting => {
            "change_result_during_reporting"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constitution_boundary_contracts_cover_required_roles() {
        let roles: Vec<InteractionFuzzRole> = constitution_boundary_contracts()
            .iter()
            .map(|contract| contract.role)
            .collect();

        assert_eq!(
            roles,
            vec![
                InteractionFuzzRole::Generator,
                InteractionFuzzRole::Probe,
                InteractionFuzzRole::Oracle,
                InteractionFuzzRole::Shrinker,
                InteractionFuzzRole::Replay,
                InteractionFuzzRole::Reporter
            ]
        );
    }

    #[test]
    fn constitution_boundary_check_passes_declared_generator_contract() {
        let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
            role: InteractionFuzzRole::Generator,
            responsibilities: vec![InteractionFuzzResponsibility::EmitIntentCommands],
            forbidden_actions: Vec::new(),
            boundary_narrowed: false,
            full_confidence_claim: true,
            authority: "interaction_generator",
        });

        assert_eq!(evaluation.oracle_id, ORACLE_346_C1_ID);
        assert_eq!(evaluation.oracle_name, ORACLE_346_C1_NAME);
        assert_eq!(evaluation.status, InteractionOracleStatus::Passed);
        assert_eq!(
            evaluation.derived_values.get("role"),
            Some(&"generator".to_string())
        );
    }

    #[test]
    fn constitution_boundary_check_reports_forbidden_probe_write() {
        let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
            role: InteractionFuzzRole::Probe,
            responsibilities: vec![InteractionFuzzResponsibility::ObserveRuntimeState],
            forbidden_actions: vec![InteractionFuzzForbiddenAction::MutateWidgetStateDirectly],
            boundary_narrowed: false,
            full_confidence_claim: true,
            authority: "list_view_probe",
        });

        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failed_requirement,
            Some("no_forbidden_boundary_action".to_string())
        );
        assert_eq!(
            evaluation.failed_value,
            Some("mutate_widget_state_directly".to_string())
        );
        assert_eq!(
            evaluation.failed_authority,
            Some("list_view_probe".to_string())
        );
    }

    #[test]
    fn constitution_boundary_check_reports_reduced_boundary_full_claim() {
        let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
            role: InteractionFuzzRole::Replay,
            responsibilities: vec![InteractionFuzzResponsibility::ReplayRecordedTrace],
            forbidden_actions: Vec::new(),
            boundary_narrowed: true,
            full_confidence_claim: true,
            authority: "primitive_replay",
        });

        assert_eq!(evaluation.status, InteractionOracleStatus::Failed);
        assert_eq!(
            evaluation.failed_requirement,
            Some("reduced_boundary_cannot_emit_full_confidence_claim".to_string())
        );
        assert_eq!(
            evaluation.failed_value,
            Some("full_confidence_claim".to_string())
        );
    }
}
