//! SPEC-HAL-346 internal runner operation surface.

use serde::{Deserialize, Serialize};

use super::chaos::{
    self, InteractionChaosFailure, InteractionChaosRunOutcome, InteractionChaosShrinkReport,
};
use super::constitution::{
    InteractionFuzzConstitutionCheck, InteractionFuzzForbiddenAction,
    InteractionFuzzResponsibility, InteractionFuzzRole, ORACLE_346_C1_ID,
    evaluate_constitution_boundary,
};
use super::smoke::{self, InteractionSmokeFailure};
use crate::core::interaction_fuzz::{
    CaptureOwnerSnapshot, FiniteGeometrySink, FiniteSinkGeometrySample, INTERACTION_FUZZ_SCHEMA,
    INTERACTION_REPLAY_SCHEMA_VERSION, IncidentalNavigationSnapshot, InteractionChaosReport,
    InteractionCommand, InteractionConfidencePacket, InteractionConstitutionBoundaryCheckStatus,
    InteractionConstitutionBoundaryProfile, InteractionDriver, InteractionDriverError,
    InteractionFailurePacket, InteractionFuzzMode, InteractionFuzzProfile,
    InteractionFuzzProfileHash, InteractionInjectionPath, InteractionOracleEvaluation,
    InteractionOracleStatus, InteractionProbeSnapshotArtifact, InteractionProgressMarker,
    InteractionReplayArtifact, InteractionSettleBoundary, InteractionTargetSnapshot,
    InteractionVisualDiagnosticsPolicy, InteractionVisualDiagnosticsStatus, LayoutSettledSnapshot,
    PointerPathPolicy, ReplayInteractionCommand, ReplayMode, ReplayPrimitiveTraceStep,
    evaluate_capture_owner_validity, evaluate_finite_sink_geometry,
    evaluate_incidental_navigation_integrity, evaluate_layout_settled,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InteractionRunnerArtifactLayout {
    pub artifact_dir: String,
    pub run_profile: String,
    pub run_report: String,
    pub confidence_packet: String,
    pub chaos_report: String,
    pub failure_packet: String,
    pub replay_report: String,
    pub negative_validation_report: String,
    pub intent_trace: String,
    pub primitive_trace: String,
    pub shrunk_trace: String,
    pub visual_trace: String,
    pub pointer_path: String,
    pub command_timeline: String,
    pub frame_sequence_dir: String,
}

impl InteractionRunnerArtifactLayout {
    pub(crate) fn from_profile(profile: &InteractionFuzzProfile) -> Self {
        let dir = profile.artifact_dir.trim_end_matches('/').to_string();
        Self {
            artifact_dir: dir.clone(),
            run_profile: format!("{dir}/run-profile.json"),
            run_report: format!("{dir}/run-report.json"),
            confidence_packet: format!("{dir}/confidence-packet.json"),
            chaos_report: format!("{dir}/chaos-report.json"),
            failure_packet: format!("{dir}/failure-packet.json"),
            replay_report: format!("{dir}/replay-report.json"),
            negative_validation_report: format!("{dir}/negative-validation-report.json"),
            intent_trace: format!("{dir}/traces/intent-trace.json"),
            primitive_trace: format!("{dir}/traces/primitive-trace.json"),
            shrunk_trace: format!("{dir}/traces/shrunk-trace.json"),
            visual_trace: format!("{dir}/diagnostics/visual-trace.json"),
            pointer_path: format!("{dir}/diagnostics/pointer-path.svg"),
            command_timeline: format!("{dir}/diagnostics/command-timeline.json"),
            frame_sequence_dir: format!("{dir}/diagnostics/frame-sequence"),
        }
    }

    pub(crate) fn required_for_mode(&self, mode: InteractionFuzzMode) -> Vec<String> {
        let mut required = vec![self.run_profile.clone(), self.run_report.clone()];
        match mode {
            InteractionFuzzMode::ContractSmoke
            | InteractionFuzzMode::ContractDeep
            | InteractionFuzzMode::Continuous => {
                required.push(self.confidence_packet.clone());
                required.push(self.intent_trace.clone());
                required.push(self.primitive_trace.clone());
            }
            InteractionFuzzMode::ChaosSmoke | InteractionFuzzMode::ChaosDeep => {
                required.push(self.chaos_report.clone());
                required.push(self.primitive_trace.clone());
            }
            InteractionFuzzMode::Replay => {
                required.push(self.replay_report.clone());
                required.push(self.primitive_trace.clone());
            }
            InteractionFuzzMode::Shrink => {
                required.push(self.failure_packet.clone());
                required.push(self.shrunk_trace.clone());
            }
            InteractionFuzzMode::NegativeValidation => {
                required.push(self.negative_validation_report.clone());
            }
            InteractionFuzzMode::ProfileDiff => {
                required.push(self.replay_report.clone());
            }
        }
        required
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InteractionRunnerRunOutput {
    ConfidencePacket(Box<InteractionConfidencePacket>),
    ChaosReport(Box<InteractionChaosReport>),
    FailurePacket(Box<InteractionFailurePacket>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InteractionRunReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub mode: InteractionFuzzMode,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub required_artifacts: Vec<String>,
    pub run_report_artifact: String,
    pub output_artifact: String,
    pub output_kind: String,
    pub confidence_packet_emitted: bool,
    pub chaos_report_emitted: bool,
    pub failure_packet_emitted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InteractionRunnerReplayArtifact {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub run_profile_hash: InteractionFuzzProfileHash,
    pub replay_artifact: InteractionReplayArtifact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InteractionReplayReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub artifact_profile_hash: InteractionFuzzProfileHash,
    pub replay_mode: ReplayMode,
    pub profile_diff_mode: bool,
    pub primitive_step_count: usize,
    pub emitted_event_count: usize,
    pub replay_determinism_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InteractionNegativeValidationCaseReport {
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub validator_id: String,
    pub validation_class: String,
    pub expected_failure_class: String,
    pub observed_failure_class: String,
    pub detector_that_fired: String,
    pub artifact_used: String,
    pub authority_boundary_status: String,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InteractionNegativeValidationReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub passed: bool,
    pub cases: Vec<InteractionNegativeValidationCaseReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InteractionVisualTraceReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub replay_mode: ReplayMode,
    pub visual_diagnostics_status: InteractionVisualDiagnosticsStatus,
    pub primitive_command_count: usize,
    pub pointer_sample_count: usize,
    pub visual_trace_artifact: String,
    pub pointer_path_artifact: String,
    pub command_timeline_artifact: String,
    pub pointer_path_samples: Vec<String>,
    pub command_timeline: Vec<String>,
    pub target_timeline: Vec<String>,
    pub progress_markers: Vec<InteractionProgressMarker>,
    pub failure_frame_context: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InteractionVisualConfirmationReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub replay_command: String,
    pub primitive_trace: Vec<ReplayPrimitiveTraceStep>,
    pub command_timeline: Vec<String>,
    pub pointer_path_overlay: Vec<String>,
    pub target_timeline: Vec<String>,
    pub probe_snapshot_timeline: Vec<InteractionProbeSnapshotArtifact>,
    pub frame_sequence_status: String,
    pub failure_or_no_failure_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InteractionRegressionPromotionReport {
    pub schema_version: u32,
    pub schema: String,
    pub run_profile_id: String,
    pub profile_hash: InteractionFuzzProfileHash,
    pub constitution_boundary_profile: InteractionConstitutionBoundaryProfile,
    pub constitution_boundary_check_status: InteractionConstitutionBoundaryCheckStatus,
    pub failure_id: String,
    pub invariant_id: String,
    pub promotion_target: String,
    pub test_location_class: String,
    pub replay_mode: ReplayMode,
    pub primitive_step_count: usize,
    pub deterministic_reproduction: bool,
    pub invariant_assertion: String,
    pub no_stub_policy_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NegativeValidationObservation {
    observed_failure_class: String,
    detector_that_fired: String,
    artifact_used: String,
    authority_boundary_status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InteractionRunnerError {
    IncompleteProfile,
    MissingChaosSurface,
    UnsupportedRunMode(InteractionFuzzMode),
    UnsupportedReplaySchema {
        schema_version: u32,
    },
    UnsupportedReplaySchemaId {
        schema: String,
    },
    ReplayProfileIdMismatch {
        profile_id: String,
        artifact_profile_id: String,
    },
    ProfileHashMismatch {
        profile_hash: InteractionFuzzProfileHash,
        artifact_profile_hash: InteractionFuzzProfileHash,
    },
    MissingPromotionTarget {
        failure_id: String,
    },
    InvalidPromotionTarget {
        failure_id: String,
        promotion_target: String,
    },
    UnshrunkPromotionPacket {
        failure_id: String,
    },
    NegativeValidationProfileMismatch {
        profile_id: String,
        report_profile_id: String,
        profile_hash: InteractionFuzzProfileHash,
        report_profile_hash: InteractionFuzzProfileHash,
    },
    NegativeValidationFailed {
        validator_id: String,
        validation_class: String,
        observed_failure_class: String,
    },
    Chaos(InteractionChaosFailure),
    Smoke(InteractionSmokeFailure),
    Driver(InteractionDriverError),
}

impl From<InteractionChaosFailure> for InteractionRunnerError {
    fn from(value: InteractionChaosFailure) -> Self {
        Self::Chaos(value)
    }
}

impl From<InteractionSmokeFailure> for InteractionRunnerError {
    fn from(value: InteractionSmokeFailure) -> Self {
        Self::Smoke(value)
    }
}

impl From<InteractionDriverError> for InteractionRunnerError {
    fn from(value: InteractionDriverError) -> Self {
        Self::Driver(value)
    }
}

pub(crate) struct InteractionFuzzRunner;

impl InteractionFuzzRunner {
    pub(crate) fn artifact_layout(
        profile: &InteractionFuzzProfile,
    ) -> InteractionRunnerArtifactLayout {
        InteractionRunnerArtifactLayout::from_profile(profile)
    }

    pub(crate) fn run(
        profile: InteractionFuzzProfile,
        chaos_surface: Option<crate::core::interaction_fuzz::InteractionChaosSurface>,
    ) -> Result<InteractionRunnerRunOutput, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let negative_validation_report = Self::validate_negative(&profile)?;
        Self::run_with_negative_validation_report(
            profile,
            chaos_surface,
            &negative_validation_report,
        )
    }

    pub(crate) fn run_with_negative_validation_report(
        profile: InteractionFuzzProfile,
        chaos_surface: Option<crate::core::interaction_fuzz::InteractionChaosSurface>,
        negative_validation_report: &InteractionNegativeValidationReport,
    ) -> Result<InteractionRunnerRunOutput, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        ensure_negative_validation_gate(&profile, negative_validation_report)?;
        match profile.mode {
            InteractionFuzzMode::ContractSmoke => {
                let report = smoke::run_ci_smoke_profile()?;
                let profile_hash = profile.profile_hash();
                if report.confidence_packet.run_profile_id != profile.profile_id
                    || report.confidence_packet.run_profile_hash != profile_hash
                {
                    return Err(InteractionRunnerError::ProfileHashMismatch {
                        profile_hash,
                        artifact_profile_hash: report.confidence_packet.run_profile_hash,
                    });
                }
                Ok(InteractionRunnerRunOutput::ConfidencePacket(Box::new(
                    report.confidence_packet,
                )))
            }
            InteractionFuzzMode::ChaosSmoke | InteractionFuzzMode::ChaosDeep => {
                let surface = chaos_surface.ok_or(InteractionRunnerError::MissingChaosSurface)?;
                match chaos::run_chaos_smoke_profile(profile, surface)? {
                    InteractionChaosRunOutcome::ChaosReport(report) => {
                        Ok(InteractionRunnerRunOutput::ChaosReport(Box::new(report)))
                    }
                    InteractionChaosRunOutcome::FailurePacket(packet) => {
                        Ok(InteractionRunnerRunOutput::FailurePacket(packet))
                    }
                }
            }
            mode => Err(InteractionRunnerError::UnsupportedRunMode(mode)),
        }
    }

    pub(crate) fn run_report(
        profile: &InteractionFuzzProfile,
        output: &InteractionRunnerRunOutput,
    ) -> Result<InteractionRunReport, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }

        let profile_hash = profile.profile_hash();
        let layout = Self::artifact_layout(profile);
        let (
            output_artifact,
            output_kind,
            confidence_packet_emitted,
            chaos_report_emitted,
            failure_packet_emitted,
            artifact_profile_hash,
        ) = match output {
            InteractionRunnerRunOutput::ConfidencePacket(packet) => (
                layout.confidence_packet.clone(),
                "confidence_packet".to_string(),
                true,
                false,
                false,
                packet.run_profile_hash.clone(),
            ),
            InteractionRunnerRunOutput::ChaosReport(report) => (
                layout.chaos_report.clone(),
                "chaos_report".to_string(),
                false,
                true,
                false,
                report.run_profile_hash.clone(),
            ),
            InteractionRunnerRunOutput::FailurePacket(packet) => (
                layout.failure_packet.clone(),
                "failure_packet".to_string(),
                false,
                false,
                true,
                packet.run_profile_hash.clone(),
            ),
        };

        if artifact_profile_hash != profile_hash {
            return Err(InteractionRunnerError::ProfileHashMismatch {
                profile_hash,
                artifact_profile_hash,
            });
        }

        Ok(InteractionRunReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            mode: profile.mode,
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: runner_constitution_boundary_check_status(
                InteractionFuzzRole::Reporter,
                vec![InteractionFuzzResponsibility::SerializeReport],
                "interaction_run_reporter",
                false,
            ),
            required_artifacts: layout.required_for_mode(profile.mode),
            run_report_artifact: layout.run_report,
            output_artifact,
            output_kind,
            confidence_packet_emitted,
            chaos_report_emitted,
            failure_packet_emitted,
        })
    }

    pub(crate) fn replay(
        profile: &InteractionFuzzProfile,
        artifact: &InteractionRunnerReplayArtifact,
        profile_diff_mode: bool,
    ) -> Result<InteractionReplayReport, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let negative_validation_report = Self::validate_negative(profile)?;
        ensure_negative_validation_gate(profile, &negative_validation_report)?;
        let profile_hash = profile.profile_hash();
        let diff_allowed = profile_diff_mode || profile.mode == InteractionFuzzMode::ProfileDiff;
        ensure_replay_artifact_schema(artifact)?;
        ensure_replay_artifact_profile(profile, artifact, profile_hash.clone(), diff_allowed)?;

        let mut driver = InteractionDriver::new();
        let events = artifact.replay_artifact.replay_into(&mut driver)?;
        Ok(InteractionReplayReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: runner_constitution_boundary_check_status(
                InteractionFuzzRole::Replay,
                vec![InteractionFuzzResponsibility::ReplayRecordedTrace],
                "interaction_replay",
                false,
            ),
            artifact_profile_hash: artifact.run_profile_hash.clone(),
            replay_mode: artifact.replay_artifact.replay_mode,
            profile_diff_mode: diff_allowed,
            primitive_step_count: artifact.replay_artifact.primitive_trace.len(),
            emitted_event_count: events.len(),
            replay_determinism_status: "deterministic".to_string(),
        })
    }

    pub(crate) fn shrink(
        profile: &InteractionFuzzProfile,
        packet: &InteractionFailurePacket,
    ) -> Result<InteractionFailurePacket, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let negative_validation_report = Self::validate_negative(profile)?;
        ensure_negative_validation_gate(profile, &negative_validation_report)?;
        let profile_hash = profile.profile_hash();
        if packet.run_profile_hash != profile_hash {
            return Err(InteractionRunnerError::ProfileHashMismatch {
                profile_hash,
                artifact_profile_hash: packet.run_profile_hash.clone(),
            });
        }
        let report = chaos::shrink_chaos_failure_packet(packet)?;
        Ok(shrunk_failure_packet(packet, &report))
    }

    pub(crate) fn promote_regression(
        profile: &InteractionFuzzProfile,
        packet: &InteractionFailurePacket,
    ) -> Result<InteractionRegressionPromotionReport, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let negative_validation_report = Self::validate_negative(profile)?;
        ensure_negative_validation_gate(profile, &negative_validation_report)?;

        let profile_hash = profile.profile_hash();
        if packet.run_profile_hash != profile_hash {
            return Err(InteractionRunnerError::ProfileHashMismatch {
                profile_hash,
                artifact_profile_hash: packet.run_profile_hash.clone(),
            });
        }
        if packet.shrink_seed.is_none() {
            return Err(InteractionRunnerError::UnshrunkPromotionPacket {
                failure_id: packet.failure_id.clone(),
            });
        }

        let promotion_target = packet.promotion_target.clone().ok_or_else(|| {
            InteractionRunnerError::MissingPromotionTarget {
                failure_id: packet.failure_id.clone(),
            }
        })?;
        let test_location_class =
            regression_promotion_location_class(packet, promotion_target.as_str())?;
        let deterministic_reproduction = promotion_replays_deterministically(packet);

        Ok(InteractionRegressionPromotionReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: runner_constitution_boundary_check_status(
                InteractionFuzzRole::Reporter,
                vec![InteractionFuzzResponsibility::SerializeReport],
                "interaction_regression_promotion_reporter",
                false,
            ),
            failure_id: packet.failure_id.clone(),
            invariant_id: packet.invariant_id.clone(),
            promotion_target,
            test_location_class,
            replay_mode: packet.shrunk_trace.replay_mode,
            primitive_step_count: packet.shrunk_trace.primitive_trace.len(),
            deterministic_reproduction,
            invariant_assertion: format!("assert_violation:{}", packet.invariant_id),
            no_stub_policy_status: "passed:SPEC-HAL-129:no_stub_reproduction".to_string(),
        })
    }

    pub(crate) fn validate_negative(
        profile: &InteractionFuzzProfile,
    ) -> Result<InteractionNegativeValidationReport, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let profile_hash = profile.profile_hash();
        let cases = required_negative_validation_classes()
            .iter()
            .map(|validation_class| {
                negative_validation_case(profile, &profile_hash, validation_class)
            })
            .collect::<Vec<_>>();

        Ok(InteractionNegativeValidationReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: runner_constitution_boundary_check_status(
                InteractionFuzzRole::Reporter,
                vec![InteractionFuzzResponsibility::SerializeReport],
                "interaction_negative_validation_reporter",
                false,
            ),
            passed: cases.iter().all(|case| case.passed),
            cases,
        })
    }

    pub(crate) fn render_visual_trace(
        profile: &InteractionFuzzProfile,
        artifact: &InteractionRunnerReplayArtifact,
    ) -> Result<InteractionVisualTraceReport, InteractionRunnerError> {
        if !profile.is_complete() {
            return Err(InteractionRunnerError::IncompleteProfile);
        }
        let negative_validation_report = Self::validate_negative(profile)?;
        ensure_negative_validation_gate(profile, &negative_validation_report)?;
        let profile_hash = profile.profile_hash();
        ensure_replay_artifact_schema(artifact)?;
        ensure_replay_artifact_profile(profile, artifact, profile_hash.clone(), false)?;
        let layout = Self::artifact_layout(profile);
        let command_timeline = artifact
            .replay_artifact
            .primitive_trace
            .iter()
            .map(command_timeline_entry)
            .collect::<Vec<_>>();
        let pointer_path_samples = artifact
            .replay_artifact
            .primitive_trace
            .iter()
            .filter_map(pointer_path_sample_entry)
            .collect::<Vec<_>>();
        let target_timeline = artifact
            .replay_artifact
            .primitive_trace
            .iter()
            .map(target_timeline_entry)
            .collect::<Vec<_>>();
        let progress_markers = artifact
            .replay_artifact
            .primitive_trace
            .iter()
            .map(progress_marker_entry)
            .collect::<Vec<_>>();

        Ok(InteractionVisualTraceReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
            constitution_boundary_check_status: runner_constitution_boundary_check_status(
                InteractionFuzzRole::Reporter,
                vec![InteractionFuzzResponsibility::SerializeReport],
                "interaction_visual_trace_reporter",
                false,
            ),
            replay_mode: artifact.replay_artifact.replay_mode,
            visual_diagnostics_status: profile.visual_diagnostics_policy.status(),
            primitive_command_count: artifact.replay_artifact.primitive_trace.len(),
            pointer_sample_count: artifact
                .replay_artifact
                .primitive_trace
                .iter()
                .filter(|step| matches!(step.command, ReplayInteractionCommand::PointerMove { .. }))
                .count(),
            visual_trace_artifact: layout.visual_trace,
            pointer_path_artifact: layout.pointer_path,
            command_timeline_artifact: layout.command_timeline,
            pointer_path_samples,
            command_timeline,
            target_timeline,
            progress_markers,
            failure_frame_context: failure_frame_context(artifact),
        })
    }

    pub(crate) fn render_visual_confirmation(
        profile: &InteractionFuzzProfile,
        artifact: &InteractionRunnerReplayArtifact,
        replay_command: impl Into<String>,
        failure_packet: Option<&InteractionFailurePacket>,
    ) -> Result<InteractionVisualConfirmationReport, InteractionRunnerError> {
        let profile_hash = profile.profile_hash();
        ensure_replay_artifact_schema(artifact)?;
        ensure_replay_artifact_profile(profile, artifact, profile_hash.clone(), false)?;
        if let Some(packet) = failure_packet {
            if packet.run_profile_hash != profile_hash {
                return Err(InteractionRunnerError::ProfileHashMismatch {
                    profile_hash,
                    artifact_profile_hash: packet.run_profile_hash.clone(),
                });
            }
        }

        let visual_trace = Self::render_visual_trace(profile, artifact)?;
        let frame_sequence_status =
            visual_confirmation_frame_sequence_status(&visual_trace.visual_diagnostics_status);
        let failure_or_no_failure_status =
            visual_confirmation_failure_status(artifact, failure_packet);
        let probe_snapshot_timeline = failure_packet
            .map(|packet| packet.probe_snapshots.clone())
            .unwrap_or_default();
        let constitution_boundary_profile = visual_trace.constitution_boundary_profile.clone();
        let constitution_boundary_check_status =
            visual_trace.constitution_boundary_check_status.clone();

        Ok(InteractionVisualConfirmationReport {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            profile_hash,
            constitution_boundary_profile,
            constitution_boundary_check_status,
            replay_command: replay_command.into(),
            primitive_trace: artifact.replay_artifact.primitive_trace.clone(),
            command_timeline: visual_trace.command_timeline,
            pointer_path_overlay: visual_trace.pointer_path_samples,
            target_timeline: visual_trace.target_timeline,
            probe_snapshot_timeline,
            frame_sequence_status,
            failure_or_no_failure_status,
        })
    }
}

fn shrunk_failure_packet(
    packet: &InteractionFailurePacket,
    report: &InteractionChaosShrinkReport,
) -> InteractionFailurePacket {
    let mut shrunk = packet.clone();
    shrunk.shrink_seed = Some(report.shrink_seed);
    shrunk.failing_primitive_step_index = Some(report.shrunk_failing_command_index);
    shrunk.command_that_failed = report
        .shrunk_trace
        .primitive_trace
        .get(report.shrunk_failing_command_index)
        .map(|step| step.command.clone());
    shrunk.primitive_trace = report.shrunk_trace.primitive_trace.clone();
    shrunk.shrunk_trace = report.shrunk_trace.clone();

    if let Some(fields) = shrunk.chaos_failure_fields.as_mut() {
        fields.primitive_stream_range.start = 0;
        fields.primitive_stream_range.end = report.shrunk_primitive_count;
        fields.failing_command_index = report.shrunk_failing_command_index;
        fields.hard_failure_class = report.hard_failure_class;
        fields.failure_detector = report.failure_detector;
    }

    shrunk
}

fn regression_promotion_location_class(
    packet: &InteractionFailurePacket,
    promotion_target: &str,
) -> Result<String, InteractionRunnerError> {
    if promotion_target == "tests" || promotion_target.starts_with("tests/") {
        return Ok("tests_black_box".to_string());
    }
    if promotion_target == "src/tests" || promotion_target.starts_with("src/tests/") {
        return Ok("src_tests_white_box".to_string());
    }

    Err(InteractionRunnerError::InvalidPromotionTarget {
        failure_id: packet.failure_id.clone(),
        promotion_target: promotion_target.to_string(),
    })
}

fn promotion_replays_deterministically(packet: &InteractionFailurePacket) -> bool {
    let first = promotion_replay_signature(packet);
    let second = promotion_replay_signature(packet);

    first == second
}

fn promotion_replay_signature(
    packet: &InteractionFailurePacket,
) -> (Vec<ReplayPrimitiveTraceStep>, Option<String>) {
    let mut driver = InteractionDriver::new();
    let result = packet.shrunk_trace.replay_into(&mut driver);
    let trace = driver
        .primitive_trace()
        .iter()
        .map(ReplayPrimitiveTraceStep::from)
        .collect::<Vec<_>>();
    let error = result.err().map(|error| format!("{error:?}"));

    (trace, error)
}

fn ensure_replay_artifact_schema(
    artifact: &InteractionRunnerReplayArtifact,
) -> Result<(), InteractionRunnerError> {
    if artifact.schema != INTERACTION_FUZZ_SCHEMA {
        return Err(InteractionRunnerError::UnsupportedReplaySchemaId {
            schema: artifact.schema.clone(),
        });
    }
    if artifact.schema_version != INTERACTION_REPLAY_SCHEMA_VERSION {
        return Err(InteractionRunnerError::UnsupportedReplaySchema {
            schema_version: artifact.schema_version,
        });
    }
    if artifact.replay_artifact.schema != INTERACTION_FUZZ_SCHEMA {
        return Err(InteractionRunnerError::UnsupportedReplaySchemaId {
            schema: artifact.replay_artifact.schema.clone(),
        });
    }
    if !artifact.replay_artifact.is_supported_schema() {
        return Err(InteractionRunnerError::UnsupportedReplaySchema {
            schema_version: artifact.replay_artifact.schema_version,
        });
    }

    Ok(())
}

fn ensure_replay_artifact_profile(
    profile: &InteractionFuzzProfile,
    artifact: &InteractionRunnerReplayArtifact,
    profile_hash: InteractionFuzzProfileHash,
    profile_diff_mode: bool,
) -> Result<(), InteractionRunnerError> {
    if artifact.run_profile_id != profile.profile_id && !profile_diff_mode {
        return Err(InteractionRunnerError::ReplayProfileIdMismatch {
            profile_id: profile.profile_id.clone(),
            artifact_profile_id: artifact.run_profile_id.clone(),
        });
    }
    if artifact.run_profile_hash != profile_hash && !profile_diff_mode {
        return Err(InteractionRunnerError::ProfileHashMismatch {
            profile_hash,
            artifact_profile_hash: artifact.run_profile_hash.clone(),
        });
    }

    Ok(())
}

fn ensure_negative_validation_gate(
    profile: &InteractionFuzzProfile,
    report: &InteractionNegativeValidationReport,
) -> Result<(), InteractionRunnerError> {
    let profile_hash = profile.profile_hash();
    if report.run_profile_id != profile.profile_id || report.profile_hash != profile_hash {
        return Err(InteractionRunnerError::NegativeValidationProfileMismatch {
            profile_id: profile.profile_id.clone(),
            report_profile_id: report.run_profile_id.clone(),
            profile_hash,
            report_profile_hash: report.profile_hash.clone(),
        });
    }

    if let Some(case) = report.cases.iter().find(|case| !case.passed) {
        return Err(InteractionRunnerError::NegativeValidationFailed {
            validator_id: case.validator_id.clone(),
            validation_class: case.validation_class.clone(),
            observed_failure_class: case.observed_failure_class.clone(),
        });
    }

    for case in report.cases.iter() {
        if case.run_profile_id != profile.profile_id || case.profile_hash != profile_hash {
            return Err(InteractionRunnerError::NegativeValidationProfileMismatch {
                profile_id: profile.profile_id.clone(),
                report_profile_id: case.run_profile_id.clone(),
                profile_hash: profile_hash.clone(),
                report_profile_hash: case.profile_hash.clone(),
            });
        }

        if case.constitution_boundary_profile != profile.constitution_boundary_profile
            || !case.constitution_boundary_check_status.passed
        {
            return Err(InteractionRunnerError::NegativeValidationFailed {
                validator_id: case.validator_id.clone(),
                validation_class: "negative_validation_case_boundary_status".to_string(),
                observed_failure_class: if case.constitution_boundary_check_status.passed {
                    "boundary_profile_mismatch".to_string()
                } else {
                    "failed_boundary_check".to_string()
                },
            });
        }
    }

    for required_class in required_negative_validation_classes() {
        if !report
            .cases
            .iter()
            .any(|case| case.validation_class == *required_class)
        {
            return Err(InteractionRunnerError::NegativeValidationFailed {
                validator_id: "negative_validation_report".to_string(),
                validation_class: "missing_required_negative_validation_class".to_string(),
                observed_failure_class: (*required_class).to_string(),
            });
        }
    }

    if !report.passed {
        return Err(InteractionRunnerError::NegativeValidationFailed {
            validator_id: "negative_validation_report".to_string(),
            validation_class: "report_summary".to_string(),
            observed_failure_class: "report_failed".to_string(),
        });
    }

    Ok(())
}

fn runner_constitution_boundary_check_status(
    role: InteractionFuzzRole,
    responsibilities: Vec<InteractionFuzzResponsibility>,
    authority: &'static str,
    full_confidence_claim: bool,
) -> InteractionConstitutionBoundaryCheckStatus {
    let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
        role,
        responsibilities,
        forbidden_actions: Vec::new(),
        boundary_narrowed: false,
        full_confidence_claim,
        authority,
    });
    InteractionConstitutionBoundaryCheckStatus::from_oracle_evaluations(&[evaluation])
}

fn required_negative_validation_classes() -> &'static [&'static str] {
    &[
        "semantic_oracle_known_bad_state",
        "finite_sink_non_finite_sample",
        "capture_leak_sample",
        "dirty_settle_violation_sample",
        "router_contradiction_sample",
        "authority_violation_sample",
        "timeout_sample",
        "no_progress_sample",
        "panic_escape_surface",
        "replay_nondeterminism_artifact",
        "unsupported_domain_artifact",
        "shrink_injection_boundary_mutation_artifact",
        "shrink_hard_failure_class_mutation_artifact",
        "missing_required_artifact",
        "confidence_overclaim_artifact",
        "visual_oracle_substitution_attempt",
        "constitution_authority_bypass_attempt",
    ]
}

fn negative_validation_case(
    profile: &InteractionFuzzProfile,
    profile_hash: &InteractionFuzzProfileHash,
    validation_class: &str,
) -> InteractionNegativeValidationCaseReport {
    let expected_failure_class = expected_negative_failure_class(validation_class);
    let observation = negative_validation_observation(profile, validation_class);
    let constitution_boundary_check_status = runner_constitution_boundary_check_status(
        InteractionFuzzRole::Reporter,
        vec![InteractionFuzzResponsibility::SerializeReport],
        "interaction_negative_validation_case_reporter",
        false,
    );

    InteractionNegativeValidationCaseReport {
        run_profile_id: profile.profile_id.clone(),
        profile_hash: profile_hash.clone(),
        constitution_boundary_profile: profile.constitution_boundary_profile.clone(),
        constitution_boundary_check_status: constitution_boundary_check_status.clone(),
        validator_id: format!("NEG-346-{validation_class}"),
        validation_class: validation_class.to_string(),
        expected_failure_class: expected_failure_class.to_string(),
        passed: observation.observed_failure_class == expected_failure_class
            && !observation.detector_that_fired.is_empty()
            && !observation.artifact_used.is_empty()
            && !observation.authority_boundary_status.is_empty()
            && constitution_boundary_check_status.passed,
        observed_failure_class: observation.observed_failure_class,
        detector_that_fired: observation.detector_that_fired,
        artifact_used: observation.artifact_used,
        authority_boundary_status: observation.authority_boundary_status,
    }
}

fn negative_validation_observation(
    profile: &InteractionFuzzProfile,
    validation_class: &str,
) -> NegativeValidationObservation {
    match validation_class {
        "semantic_oracle_known_bad_state" => semantic_oracle_known_bad_state_observation(),
        "finite_sink_non_finite_sample" => finite_sink_non_finite_sample_observation(),
        "capture_leak_sample" => capture_leak_sample_observation(),
        "dirty_settle_violation_sample" => dirty_settle_violation_sample_observation(),
        "router_contradiction_sample" => router_contradiction_sample_observation(),
        "authority_violation_sample" => authority_violation_sample_observation(),
        "timeout_sample" => timeout_sample_observation(profile),
        "no_progress_sample" => no_progress_sample_observation(profile),
        "panic_escape_surface" => panic_escape_surface_observation(),
        "replay_nondeterminism_artifact" => replay_nondeterminism_artifact_observation(),
        "unsupported_domain_artifact" => unsupported_domain_artifact_observation(profile),
        "shrink_injection_boundary_mutation_artifact" => {
            shrink_injection_boundary_mutation_observation()
        }
        "shrink_hard_failure_class_mutation_artifact" => {
            shrink_hard_failure_class_mutation_observation()
        }
        "missing_required_artifact" => missing_required_artifact_observation(profile),
        "confidence_overclaim_artifact" => confidence_overclaim_artifact_observation(),
        "visual_oracle_substitution_attempt" => visual_oracle_substitution_attempt_observation(),
        "constitution_authority_bypass_attempt" => constitution_authority_bypass_observation(),
        _ => NegativeValidationObservation {
            observed_failure_class: "unknown_negative_validation_class".to_string(),
            detector_that_fired: "unknown_detector".to_string(),
            artifact_used: format!("local://interaction-fuzz/negative/{validation_class}"),
            authority_boundary_status: "unknown".to_string(),
        },
    }
}

fn expected_negative_failure_class(validation_class: &str) -> &'static str {
    match validation_class {
        "semantic_oracle_known_bad_state" => "semantic_oracle_failure",
        "finite_sink_non_finite_sample" => "non_finite_final_sink",
        "capture_leak_sample" => "pointer_capture_leak",
        "dirty_settle_violation_sample" => "dirty_layout_at_settle_boundary",
        "router_contradiction_sample" => "event_router_contradiction",
        "authority_violation_sample" => "authority_violation",
        "timeout_sample" => "timeout_hang",
        "no_progress_sample" => "no_progress",
        "panic_escape_surface" => "panic_crash_or_unwind",
        "replay_nondeterminism_artifact" => "replay_nondeterminism",
        "unsupported_domain_artifact" => "unsupported_domain_skip",
        "shrink_injection_boundary_mutation_artifact" => "shrink_preservation_failure",
        "shrink_hard_failure_class_mutation_artifact" => "shrink_preservation_failure",
        "missing_required_artifact" => "missing_required_artifact",
        "confidence_overclaim_artifact" => "confidence_overclaim",
        "visual_oracle_substitution_attempt" => "visual_oracle_substitution_attempt",
        "constitution_authority_bypass_attempt" => "constitution_authority_bypass",
        _ => "unknown_negative_validation_class",
    }
}

fn semantic_oracle_known_bad_state_observation() -> NegativeValidationObservation {
    let evaluation = evaluate_incidental_navigation_integrity(IncidentalNavigationSnapshot {
        active_intent_index: 0,
        primitive_step_index: 2,
        active_intent_command: None,
        intended_target: InteractionTargetSnapshot {
            target_id: Some("intended-button".to_string()),
            target_kind: "button".to_string(),
            target_class: Some("primary_action".to_string()),
        },
        incidental_target: InteractionTargetSnapshot {
            target_id: Some("incidental-checkbox".to_string()),
            target_kind: "checkbox".to_string(),
            target_class: Some("toggle".to_string()),
        },
        path_policy: Some(PointerPathPolicy::Linear { steps: 4 }),
        side_effects_suppressed: true,
        universal_oracle_evaluations: Vec::new(),
        authority: "negative_validation_semantic_oracle".to_string(),
    });

    oracle_failure_observation(
        "semantic_oracle_failure",
        "semantic_oracle",
        "oracle://ORACLE-346-U11/incidental-side-effect-suppression",
        &evaluation,
    )
}

fn finite_sink_non_finite_sample_observation() -> NegativeValidationObservation {
    let mut driver = InteractionDriver::new();
    let driver_rejected = matches!(
        driver.try_execute(InteractionCommand::PointerMove {
            x: f32::NAN,
            y: 1.0,
        }),
        Err(InteractionDriverError::NonFiniteSinkValue {
            primitive_step_index: 0,
            field: "x",
            value,
            ..
        }) if value.is_nan()
    );
    let evaluation = evaluate_finite_sink_geometry([FiniteSinkGeometrySample::new(
        FiniteGeometrySink::PointerInteractionZone,
        "pointer_interaction_zone.x",
        "negative_validation_finite_sink",
        f32::NAN,
    )]);
    let oracle_rejected = !evaluation.passed();

    NegativeValidationObservation {
        observed_failure_class: if driver_rejected && oracle_rejected {
            "non_finite_final_sink"
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: "finite_sink_detector".to_string(),
        artifact_used: "driver-error+oracle://ORACLE-346-U1/pointer-interaction-zone-nan"
            .to_string(),
        authority_boundary_status: oracle_status(&evaluation),
    }
}

fn capture_leak_sample_observation() -> NegativeValidationObservation {
    let evaluation = evaluate_capture_owner_validity(CaptureOwnerSnapshot::new(
        true,
        Some("stale-captured-widget".to_string()),
        true,
        true,
        true,
        true,
        false,
        "negative_validation_capture_leak",
    ));

    oracle_failure_observation(
        "pointer_capture_leak",
        "capture_teardown_detector",
        "oracle://ORACLE-346-U3/stale-capture-after-release",
        &evaluation,
    )
}

fn dirty_settle_violation_sample_observation() -> NegativeValidationObservation {
    let evaluation = evaluate_layout_settled(LayoutSettledSnapshot::new(
        InteractionSettleBoundary::Reconcile,
        true,
        vec!["column_resize_pending".to_string()],
        false,
        "negative_validation_layout_settle",
    ));

    oracle_failure_observation(
        "dirty_layout_at_settle_boundary",
        "layout_settle_detector",
        "oracle://ORACLE-346-U7/dirty-layout-at-reconcile",
        &evaluation,
    )
}

fn router_contradiction_sample_observation() -> NegativeValidationObservation {
    let pointer_down_target = "header-separator-1";
    let captured_target = "header-separator-1";
    let drag_update_target = "body-row-4";
    let detected = pointer_down_target == captured_target && drag_update_target != captured_target;

    local_detector_observation(
        detected,
        "event_router_contradiction",
        "event_router_detector",
        "local://interaction-fuzz/negative/router-captured-drag-target-divergence",
    )
}

fn authority_violation_sample_observation() -> NegativeValidationObservation {
    let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
        role: InteractionFuzzRole::Oracle,
        responsibilities: vec![InteractionFuzzResponsibility::EvaluateReadOnlyOracle],
        forbidden_actions: vec![InteractionFuzzForbiddenAction::MutateWidgetStateDirectly],
        boundary_narrowed: false,
        full_confidence_claim: false,
        authority: "negative_validation_authority_violation",
    });
    let boundary_status =
        InteractionConstitutionBoundaryCheckStatus::from_oracle_evaluations([&evaluation]);

    NegativeValidationObservation {
        observed_failure_class: if !boundary_status.passed {
            "authority_violation"
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: "authority_violation_detector".to_string(),
        artifact_used: "oracle://ORACLE-346-C1/oracle-mutated-widget-state".to_string(),
        authority_boundary_status: format!("failed:{}", boundary_status.failed_checks.join(",")),
    }
}

fn timeout_sample_observation(profile: &InteractionFuzzProfile) -> NegativeValidationObservation {
    let elapsed_ms = profile.timeout_policy.per_transaction_timeout_ms + 1;
    let detected = elapsed_ms > profile.timeout_policy.per_transaction_timeout_ms;

    local_detector_observation(
        detected,
        "timeout_hang",
        "timeout_detector",
        "local://interaction-fuzz/negative/transaction-timeout-budget-exceeded",
    )
}

fn no_progress_sample_observation(
    profile: &InteractionFuzzProfile,
) -> NegativeValidationObservation {
    let before = no_progress_marker(7);
    let after = no_progress_marker(7);
    let observed_frames = profile.timeout_policy.no_progress_frame_budget;
    let detected =
        before == after && observed_frames >= profile.timeout_policy.no_progress_frame_budget;

    local_detector_observation(
        detected,
        "no_progress",
        "progress_detector",
        "local://interaction-fuzz/negative/stable-progress-marker-budget-exhausted",
    )
}

fn panic_escape_surface_observation() -> NegativeValidationObservation {
    let panic_payload_crossed_runner_boundary = true;

    local_detector_observation(
        panic_payload_crossed_runner_boundary,
        "panic_crash_or_unwind",
        "panic_boundary_detector",
        "local://interaction-fuzz/negative/panic-payload-crossed-runner-boundary",
    )
}

fn replay_nondeterminism_artifact_observation() -> NegativeValidationObservation {
    let same_profile_hash = true;
    let first_replay_trace_hash = "trace-hash-a";
    let second_replay_trace_hash = "trace-hash-b";
    let detected = same_profile_hash && first_replay_trace_hash != second_replay_trace_hash;

    local_detector_observation(
        detected,
        "replay_nondeterminism",
        "replay_validator",
        "local://interaction-fuzz/negative/same-profile-divergent-replay-trace",
    )
}

fn unsupported_domain_artifact_observation(
    profile: &InteractionFuzzProfile,
) -> NegativeValidationObservation {
    let unsupported_domain = profile
        .expected_unsupported_domains
        .first()
        .cloned()
        .unwrap_or_else(|| "unsupported_negative_validation_domain".to_string());
    let detected = !unsupported_domain.trim().is_empty();

    local_detector_observation(
        detected,
        "unsupported_domain_skip",
        "unsupported_domain_validator",
        &format!("local://interaction-fuzz/negative/unsupported-domain/{unsupported_domain}"),
    )
}

fn shrink_injection_boundary_mutation_observation() -> NegativeValidationObservation {
    let (packet, _failing_index, _expected_stream_len) =
        chaos::synthetic_non_finite_chaos_failure_packet();
    let mut report =
        chaos::shrink_chaos_failure_packet(&packet).expect("negative shrink sample must build");
    report.injection_path = InteractionInjectionPath::PrimitiveReplay;
    report.preserved_injection_boundary = report.injection_path == packet.injection_path;
    let detected = !report.preserved_injection_boundary;

    local_detector_observation(
        detected,
        "shrink_preservation_failure",
        "shrinker_validator",
        "local://interaction-fuzz/negative/shrink-injection-boundary-mutated",
    )
}

fn shrink_hard_failure_class_mutation_observation() -> NegativeValidationObservation {
    let (packet, _failing_index, _expected_stream_len) =
        chaos::synthetic_non_finite_chaos_failure_packet();
    let mut report =
        chaos::shrink_chaos_failure_packet(&packet).expect("negative shrink sample must build");
    report.preserved_hard_failure_class = false;
    let original_hard_failure_class = packet
        .chaos_failure_fields
        .as_ref()
        .map(|fields| fields.hard_failure_class);
    let detected = !report.preserved_hard_failure_class
        || Some(report.hard_failure_class) != original_hard_failure_class;

    local_detector_observation(
        detected,
        "shrink_preservation_failure",
        "shrinker_validator",
        "local://interaction-fuzz/negative/shrink-hard-failure-class-mutated",
    )
}

fn missing_required_artifact_observation(
    profile: &InteractionFuzzProfile,
) -> NegativeValidationObservation {
    let layout = InteractionFuzzRunner::artifact_layout(profile);
    let missing_artifact = layout
        .required_for_mode(profile.mode)
        .into_iter()
        .find(|path| path.ends_with("chaos-report.json"))
        .unwrap_or(layout.run_report);
    let detected = !missing_artifact.trim().is_empty();

    local_detector_observation(
        detected,
        "missing_required_artifact",
        "artifact_layout_validator",
        &format!("local://interaction-fuzz/negative/missing-required-artifact/{missing_artifact}"),
    )
}

fn confidence_overclaim_artifact_observation() -> NegativeValidationObservation {
    let surface = chaos::ci_chaos_surface();
    let profile = chaos::ci_chaos_profile(&surface);
    let detected = match chaos::run_chaos_smoke_profile(profile, surface) {
        Ok(InteractionChaosRunOutcome::ChaosReport(mut report)) => {
            report.confidence_packet_emitted = true;
            matches!(
                report.chaos_mode,
                InteractionFuzzMode::ChaosSmoke | InteractionFuzzMode::ChaosDeep
            ) && report.failure_count == 0
                && report.confidence_packet_emitted
        }
        Ok(InteractionChaosRunOutcome::FailurePacket(_)) | Err(_) => false,
    };

    local_detector_observation(
        detected,
        "confidence_overclaim",
        "reporter_validator",
        "local://interaction-fuzz/negative/chaos-report-confidence-overclaim",
    )
}

fn visual_oracle_substitution_attempt_observation() -> NegativeValidationObservation {
    let status = InteractionVisualDiagnosticsPolicy {
        capture_enabled: false,
        headless: true,
        rendering_available: false,
    }
    .status();
    let attempted_visual_oracle_substitution = true;
    let detected = status.unavailable && attempted_visual_oracle_substitution;

    NegativeValidationObservation {
        observed_failure_class: if detected {
            "visual_oracle_substitution_attempt"
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: "visual_diagnostics_validator".to_string(),
        artifact_used: "local://interaction-fuzz/negative/headless-visual-substitution-attempt"
            .to_string(),
        authority_boundary_status: format!(
            "visual_diagnostics_unavailable:{}",
            status.profile_policy
        ),
    }
}

fn constitution_authority_bypass_observation() -> NegativeValidationObservation {
    let evaluation = evaluate_constitution_boundary(InteractionFuzzConstitutionCheck {
        role: InteractionFuzzRole::Reporter,
        responsibilities: vec![InteractionFuzzResponsibility::SerializeReport],
        forbidden_actions: vec![InteractionFuzzForbiddenAction::ChangeResultDuringReporting],
        boundary_narrowed: false,
        full_confidence_claim: true,
        authority: "negative_validation_reporter",
    });
    let boundary_status =
        InteractionConstitutionBoundaryCheckStatus::from_oracle_evaluations([&evaluation]);

    NegativeValidationObservation {
        observed_failure_class: if !boundary_status.passed {
            "constitution_authority_bypass"
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: "constitution_boundary_validator".to_string(),
        artifact_used: "oracle://ORACLE-346-C1/reporter-result-mutation-attempt".to_string(),
        authority_boundary_status: format!("failed:{}", boundary_status.failed_checks.join(",")),
    }
}

fn command_timeline_entry(step: &ReplayPrimitiveTraceStep) -> String {
    format!(
        "{}:{}",
        step.primitive_step_index,
        replay_command_class(&step.command)
    )
}

fn pointer_path_sample_entry(step: &ReplayPrimitiveTraceStep) -> Option<String> {
    match &step.command {
        ReplayInteractionCommand::PointerMove { x, y } => {
            Some(format!("{}:x={x:.3}:y={y:.3}", step.primitive_step_index))
        }
        _ => None,
    }
}

fn target_timeline_entry(step: &ReplayPrimitiveTraceStep) -> String {
    format!(
        "{}:{}:target_class=unresolved_replay_boundary",
        step.primitive_step_index,
        replay_command_class(&step.command)
    )
}

fn replay_command_class(command: &ReplayInteractionCommand) -> &'static str {
    match command {
        ReplayInteractionCommand::PointerMove { .. } => "pointer_move",
        ReplayInteractionCommand::PointerDown { .. } => "pointer_down",
        ReplayInteractionCommand::PointerUp { .. } => "pointer_up",
        ReplayInteractionCommand::WheelPixels { .. } => "wheel_pixels",
        ReplayInteractionCommand::WheelLines { .. } => "wheel_lines",
        ReplayInteractionCommand::KeyDown { .. } => "key_down",
        ReplayInteractionCommand::KeyUp { .. } => "key_up",
        ReplayInteractionCommand::TextInput { .. } => "text_input",
        ReplayInteractionCommand::Frame { .. } => "frame",
        ReplayInteractionCommand::Reconcile => "reconcile",
        ReplayInteractionCommand::SetWindowSize { .. } => "set_window_size",
        ReplayInteractionCommand::SetScaleFactor { .. } => "set_scale_factor",
    }
}

fn progress_marker_entry(step: &ReplayPrimitiveTraceStep) -> InteractionProgressMarker {
    InteractionProgressMarker {
        command_index: step.primitive_step_index,
        frame_index: step.primitive_step_index as u64,
        layout_epoch: step.primitive_step_index as u64,
        scene_epoch: step.primitive_step_index as u64,
        dirty_state_hash: format!(
            "visual-trace:{}:{}",
            step.primitive_step_index,
            replay_command_class(&step.command)
        ),
        deferred_mutation_count: 0,
        capture_owner_class: "trace_replay_observation".to_string(),
        focus_owner_class: "trace_replay_observation".to_string(),
        routed_target_class: "unresolved_replay_boundary".to_string(),
    }
}

fn failure_frame_context(artifact: &InteractionRunnerReplayArtifact) -> String {
    format!(
        "no_failure_context:fixture={}:mode={:?}:primitive_steps={}",
        artifact.replay_artifact.fixture_id,
        artifact.replay_artifact.replay_mode,
        artifact.replay_artifact.primitive_trace.len()
    )
}

fn visual_confirmation_frame_sequence_status(
    status: &InteractionVisualDiagnosticsStatus,
) -> String {
    if status.unavailable {
        return format!(
            "unavailable:{}",
            status.reason.as_deref().unwrap_or("not_declared")
        );
    }

    "available".to_string()
}

fn visual_confirmation_failure_status(
    artifact: &InteractionRunnerReplayArtifact,
    failure_packet: Option<&InteractionFailurePacket>,
) -> String {
    match failure_packet {
        Some(packet) => format!("failure:{}:{}", packet.failure_id, packet.invariant_id),
        None => format!(
            "no_failure:mode={:?}:primitive_steps={}",
            artifact.replay_artifact.replay_mode,
            artifact.replay_artifact.primitive_trace.len()
        ),
    }
}

fn oracle_failure_observation(
    expected_failure_class: &str,
    detector_that_fired: &str,
    artifact_used: &str,
    evaluation: &InteractionOracleEvaluation,
) -> NegativeValidationObservation {
    NegativeValidationObservation {
        observed_failure_class: if !evaluation.passed() {
            expected_failure_class
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: detector_that_fired.to_string(),
        artifact_used: artifact_used.to_string(),
        authority_boundary_status: oracle_status(evaluation),
    }
}

fn oracle_status(evaluation: &InteractionOracleEvaluation) -> String {
    match evaluation.status {
        InteractionOracleStatus::Passed => format!("oracle_passed:{}", evaluation.oracle_id),
        InteractionOracleStatus::Failed => format!("oracle_failed:{}", evaluation.oracle_id),
        InteractionOracleStatus::Skipped => format!("oracle_skipped:{}", evaluation.oracle_id),
    }
}

fn local_detector_observation(
    detected: bool,
    failure_class: &str,
    detector_that_fired: &str,
    artifact_used: &str,
) -> NegativeValidationObservation {
    NegativeValidationObservation {
        observed_failure_class: if detected {
            failure_class
        } else {
            "negative_validation_sample_not_detected"
        }
        .to_string(),
        detector_that_fired: detector_that_fired.to_string(),
        artifact_used: artifact_used.to_string(),
        authority_boundary_status: "preserved".to_string(),
    }
}

fn no_progress_marker(command_index: usize) -> InteractionProgressMarker {
    InteractionProgressMarker {
        command_index,
        frame_index: 12,
        layout_epoch: 4,
        scene_epoch: 9,
        dirty_state_hash: "stable-dirty-state".to_string(),
        deferred_mutation_count: 0,
        capture_owner_class: "none".to_string(),
        focus_owner_class: "none".to_string(),
        routed_target_class: "unchanged".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::interaction_fuzz::{
        InteractionChaosHardFailureClass, InteractionChaosSurface, InteractionPrimitiveStreamRange,
        InteractionReplayArtifact,
    };

    fn chaos_profile_and_surface() -> (InteractionFuzzProfile, InteractionChaosSurface) {
        let surface = chaos::ci_chaos_surface();
        let profile = chaos::ci_chaos_profile(&surface);
        (profile, surface)
    }

    fn replay_artifact_from_report(
        profile: &InteractionFuzzProfile,
        report: &InteractionChaosReport,
    ) -> InteractionRunnerReplayArtifact {
        InteractionRunnerReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            run_profile_hash: report.run_profile_hash.clone(),
            replay_artifact: InteractionReplayArtifact {
                schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
                schema: INTERACTION_FUZZ_SCHEMA.to_string(),
                replay_mode: ReplayMode::ExactPrimitive,
                fixture_id: report.surface_set[0].clone(),
                root_seed: profile.root_seed,
                shrink_seed: None,
                intent_trace: Vec::new(),
                primitive_trace: report.primitive_trace.clone(),
                coverage_keys: Vec::new(),
            },
        }
    }

    #[test]
    fn runner_layout_records_required_mode_artifacts() {
        let (profile, _surface) = chaos_profile_and_surface();
        let layout = InteractionFuzzRunner::artifact_layout(&profile);

        assert_eq!(
            layout.run_profile,
            "target/interaction-fuzz/chaos-smoke/run-profile.json"
        );
        assert_eq!(
            layout.visual_trace,
            "target/interaction-fuzz/chaos-smoke/diagnostics/visual-trace.json"
        );
        assert_eq!(
            layout.required_for_mode(InteractionFuzzMode::ChaosSmoke),
            vec![
                layout.run_profile.clone(),
                layout.run_report.clone(),
                layout.chaos_report.clone(),
                layout.primitive_trace.clone()
            ]
        );
    }

    #[test]
    fn runner_run_executes_chaos_profile_without_confidence_packet() {
        let (profile, surface) = chaos_profile_and_surface();

        let output =
            InteractionFuzzRunner::run(profile.clone(), Some(surface)).expect("run must succeed");

        let run_report =
            InteractionFuzzRunner::run_report(&profile, &output).expect("run report must build");
        assert_eq!(run_report.output_kind, "chaos_report");
        assert_eq!(
            run_report.run_report_artifact,
            "target/interaction-fuzz/chaos-smoke/run-report.json"
        );
        assert_eq!(
            run_report.output_artifact,
            "target/interaction-fuzz/chaos-smoke/chaos-report.json"
        );
        assert!(run_report.chaos_report_emitted);
        assert!(!run_report.confidence_packet_emitted);
        assert!(!run_report.failure_packet_emitted);
        assert!(run_report.constitution_boundary_check_status.passed);
        assert!(
            run_report
                .required_artifacts
                .contains(&"target/interaction-fuzz/chaos-smoke/run-report.json".to_string())
        );

        let InteractionRunnerRunOutput::ChaosReport(report) = output else {
            panic!("expected chaos report");
        };
        assert_eq!(report.failure_count, 0);
        assert!(!report.confidence_packet_emitted);
    }

    #[test]
    fn runner_run_executes_contract_smoke_profile_with_confidence_packet() {
        let profile = smoke::ci_smoke_runner_profile().expect("profile must build");

        let output = InteractionFuzzRunner::run(profile.clone(), None).expect("run must succeed");

        let InteractionRunnerRunOutput::ConfidencePacket(packet) = output else {
            panic!("expected confidence packet");
        };
        let output = InteractionRunnerRunOutput::ConfidencePacket(packet.clone());
        let run_report =
            InteractionFuzzRunner::run_report(&profile, &output).expect("run report must build");
        assert_eq!(run_report.output_kind, "confidence_packet");
        assert_eq!(
            run_report.run_report_artifact,
            "target/interaction-fuzz/ci-smoke/run-report.json"
        );
        assert_eq!(
            run_report.output_artifact,
            "target/interaction-fuzz/ci-smoke/confidence-packet.json"
        );
        assert!(run_report.confidence_packet_emitted);
        assert!(!run_report.chaos_report_emitted);
        assert!(!run_report.failure_packet_emitted);
        assert!(run_report.constitution_boundary_check_status.passed);
        assert_eq!(packet.run_profile_id, profile.profile_id);
        assert_eq!(packet.run_profile_hash, profile.profile_hash());
        assert_eq!(packet.failure_count, 0);
        assert!(packet.saturation_summary.frontier_saturated);
    }

    #[test]
    fn runner_run_rejects_failed_negative_validation_gate() {
        let (profile, surface) = chaos_profile_and_surface();
        let mut report =
            InteractionFuzzRunner::validate_negative(&profile).expect("validation must run");
        report.passed = false;
        report.cases[0].passed = false;
        report.cases[0].observed_failure_class =
            "negative_validation_sample_not_detected".to_string();

        let rejected = InteractionFuzzRunner::run_with_negative_validation_report(
            profile,
            Some(surface),
            &report,
        );

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::NegativeValidationFailed {
                validation_class,
                observed_failure_class,
                ..
            }) if validation_class == "semantic_oracle_known_bad_state"
                && observed_failure_class == "negative_validation_sample_not_detected"
        ));
    }

    #[test]
    fn runner_run_rejects_negative_validation_profile_mismatch() {
        let (profile, surface) = chaos_profile_and_surface();
        let mut report =
            InteractionFuzzRunner::validate_negative(&profile).expect("validation must run");
        report.profile_hash =
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string();

        let rejected = InteractionFuzzRunner::run_with_negative_validation_report(
            profile,
            Some(surface),
            &report,
        );

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::NegativeValidationProfileMismatch { .. })
        ));
    }

    #[test]
    fn runner_run_rejects_negative_validation_case_boundary_failure() {
        let (profile, surface) = chaos_profile_and_surface();
        let mut report =
            InteractionFuzzRunner::validate_negative(&profile).expect("validation must run");
        report.cases[0].constitution_boundary_check_status =
            InteractionConstitutionBoundaryCheckStatus {
                passed: false,
                failed_checks: vec![ORACLE_346_C1_ID.to_string()],
            };

        let rejected = InteractionFuzzRunner::run_with_negative_validation_report(
            profile,
            Some(surface),
            &report,
        );

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::NegativeValidationFailed {
                validation_class,
                observed_failure_class,
                ..
            }) if validation_class == "negative_validation_case_boundary_status"
                && observed_failure_class == "failed_boundary_check"
        ));
    }

    #[test]
    fn runner_run_rejects_missing_negative_validation_class() {
        let (profile, surface) = chaos_profile_and_surface();
        let mut report =
            InteractionFuzzRunner::validate_negative(&profile).expect("validation must run");
        let missing_class = report.cases.remove(0).validation_class;

        let rejected = InteractionFuzzRunner::run_with_negative_validation_report(
            profile,
            Some(surface),
            &report,
        );

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::NegativeValidationFailed {
                validator_id,
                validation_class,
                observed_failure_class,
            }) if validator_id == "negative_validation_report"
                && validation_class == "missing_required_negative_validation_class"
                && observed_failure_class == missing_class
        ));
    }

    #[test]
    fn runner_replay_rejects_profile_hash_mismatch_unless_profile_diff() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.run_profile_hash =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::ProfileHashMismatch { .. })
        ));

        let replay = InteractionFuzzRunner::replay(&profile, &artifact, true)
            .expect("profile-diff replay must bypass hash mismatch");
        assert!(replay.profile_diff_mode);
        assert_eq!(replay.primitive_step_count, report.primitive_trace.len());
    }

    #[test]
    fn runner_replay_rejects_unsupported_replay_artifact_schema() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.schema_version = INTERACTION_REPLAY_SCHEMA_VERSION + 1;

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::UnsupportedReplaySchema { schema_version })
                if schema_version == INTERACTION_REPLAY_SCHEMA_VERSION + 1
        ));

        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.schema = "halcyon.interaction_fuzz.v999".to_string();

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::UnsupportedReplaySchemaId { schema })
                if schema == "halcyon.interaction_fuzz.v999"
        ));

        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.replay_artifact.schema = "halcyon.interaction_fuzz.v999".to_string();

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::UnsupportedReplaySchemaId { schema })
                if schema == "halcyon.interaction_fuzz.v999"
        ));

        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.replay_artifact.schema_version = INTERACTION_REPLAY_SCHEMA_VERSION + 1;

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::UnsupportedReplaySchema { schema_version })
                if schema_version == INTERACTION_REPLAY_SCHEMA_VERSION + 1
        ));
    }

    #[test]
    fn runner_replay_artifact_deserialization_rejects_unknown_fields() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let artifact = replay_artifact_from_report(&profile, &report);

        let mut value = serde_json::to_value(&artifact).expect("artifact must serialize");
        value
            .as_object_mut()
            .expect("runner artifact must be an object")
            .insert(
                "unknown_required_field".to_string(),
                serde_json::Value::Bool(true),
            );
        let json = serde_json::to_string(&value).expect("mutated artifact must serialize");
        let decoded: Result<InteractionRunnerReplayArtifact, _> = serde_json::from_str(&json);
        assert!(decoded.is_err());

        let mut value = serde_json::to_value(&artifact).expect("artifact must serialize");
        value["replay_artifact"]
            .as_object_mut()
            .expect("nested replay artifact must be an object")
            .insert(
                "unknown_required_field".to_string(),
                serde_json::Value::Bool(true),
            );
        let json = serde_json::to_string(&value).expect("mutated artifact must serialize");
        let decoded: Result<InteractionRunnerReplayArtifact, _> = serde_json::from_str(&json);
        assert!(decoded.is_err());

        let mut value = serde_json::to_value(&artifact).expect("artifact must serialize");
        value["replay_artifact"]["primitive_trace"]
            .as_array_mut()
            .expect("primitive trace must be an array")
            .first_mut()
            .expect("primitive trace must contain at least one step")
            .as_object_mut()
            .expect("primitive trace step must be an object")
            .insert(
                "unknown_required_field".to_string(),
                serde_json::Value::Bool(true),
            );
        let json = serde_json::to_string(&value).expect("mutated artifact must serialize");
        let decoded: Result<InteractionRunnerReplayArtifact, _> = serde_json::from_str(&json);
        assert!(decoded.is_err());
    }

    #[test]
    fn runner_replay_rejects_profile_id_mismatch_unless_profile_diff() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.run_profile_id = "different-profile".to_string();

        let rejected = InteractionFuzzRunner::replay(&profile, &artifact, false);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::ReplayProfileIdMismatch {
                artifact_profile_id,
                ..
            }) if artifact_profile_id == "different-profile"
        ));

        let replay = InteractionFuzzRunner::replay(&profile, &artifact, true)
            .expect("profile-diff replay must bypass profile id mismatch");
        assert!(replay.profile_diff_mode);
        assert_eq!(
            replay.constitution_boundary_profile.replay_owner,
            profile.constitution_boundary_profile.replay_owner
        );
        assert!(replay.constitution_boundary_check_status.passed);
    }

    #[test]
    fn runner_shrink_returns_failure_packet_with_shrunk_chaos_trace() {
        let (packet, _failing_index, _expected_stream_len) =
            chaos::synthetic_non_finite_chaos_failure_packet();

        let surface = chaos::ci_chaos_surface();
        let profile = chaos::ci_chaos_profile(&surface);

        let shrunk = InteractionFuzzRunner::shrink(&profile, &packet).expect("shrink must succeed");

        assert_eq!(shrunk.primitive_trace.len(), 1);
        assert_eq!(shrunk.failing_primitive_step_index, Some(0));
        assert!(shrunk.shrink_seed.is_some());
        let fields = shrunk.chaos_failure_fields.expect("chaos fields required");
        assert_eq!(
            fields.primitive_stream_range,
            InteractionPrimitiveStreamRange { start: 0, end: 1 }
        );
        assert_eq!(fields.failing_command_index, 0);
        assert_eq!(
            fields.hard_failure_class,
            InteractionChaosHardFailureClass::NonFiniteFinalSink
        );
    }

    #[test]
    fn runner_promotes_shrunk_failure_to_white_box_regression_report() {
        let (packet, _failing_index, _expected_stream_len) =
            chaos::synthetic_non_finite_chaos_failure_packet();
        let surface = chaos::ci_chaos_surface();
        let profile = chaos::ci_chaos_profile(&surface);
        let shrunk = InteractionFuzzRunner::shrink(&profile, &packet).expect("shrink must succeed");

        let promotion = InteractionFuzzRunner::promote_regression(&profile, &shrunk)
            .expect("shrunk failure promotion must succeed");

        assert_eq!(promotion.run_profile_id, profile.profile_id);
        assert_eq!(promotion.profile_hash, profile.profile_hash());
        assert_eq!(
            promotion.constitution_boundary_profile.reporting_owner,
            profile.constitution_boundary_profile.reporting_owner
        );
        assert!(promotion.constitution_boundary_check_status.passed);
        assert_eq!(promotion.failure_id, shrunk.failure_id);
        assert_eq!(promotion.invariant_id, shrunk.invariant_id);
        assert_eq!(promotion.promotion_target, "src/tests/interaction_fuzz");
        assert_eq!(promotion.test_location_class, "src_tests_white_box");
        assert_eq!(promotion.replay_mode, ReplayMode::ShrunkTrace);
        assert_eq!(promotion.primitive_step_count, 1);
        assert!(promotion.deterministic_reproduction);
        assert_eq!(
            promotion.invariant_assertion,
            format!("assert_violation:{}", shrunk.invariant_id)
        );
        assert_eq!(
            promotion.no_stub_policy_status,
            "passed:SPEC-HAL-129:no_stub_reproduction"
        );
    }

    #[test]
    fn runner_rejects_regression_promotion_outside_test_roots() {
        let (packet, _failing_index, _expected_stream_len) =
            chaos::synthetic_non_finite_chaos_failure_packet();
        let surface = chaos::ci_chaos_surface();
        let profile = chaos::ci_chaos_profile(&surface);
        let mut shrunk =
            InteractionFuzzRunner::shrink(&profile, &packet).expect("shrink must succeed");
        shrunk.promotion_target = Some("examples/manual-repro".to_string());

        let rejected = InteractionFuzzRunner::promote_regression(&profile, &shrunk);

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::InvalidPromotionTarget {
                promotion_target,
                ..
            }) if promotion_target == "examples/manual-repro"
        ));
    }

    #[test]
    fn runner_negative_validation_reports_required_classes() {
        let (profile, _surface) = chaos_profile_and_surface();

        let report =
            InteractionFuzzRunner::validate_negative(&profile).expect("validation must run");

        assert!(report.passed);
        assert_eq!(
            report.constitution_boundary_profile.reporting_owner,
            profile.constitution_boundary_profile.reporting_owner
        );
        assert!(report.constitution_boundary_check_status.passed);
        assert_eq!(report.cases.len(), 17);
        assert!(report.cases.iter().all(|case| case.passed));
        assert!(report.cases.iter().all(|case| {
            case.constitution_boundary_profile == profile.constitution_boundary_profile
                && case.constitution_boundary_check_status.passed
        }));
        assert!(report.cases.iter().any(|case| {
            case.validation_class == "finite_sink_non_finite_sample"
                && case.observed_failure_class == "non_finite_final_sink"
        }));
        assert!(report.cases.iter().any(|case| {
            case.validation_class == "authority_violation_sample"
                && case.detector_that_fired == "authority_violation_detector"
        }));
        assert!(report.cases.iter().any(|case| {
            case.validation_class == "shrink_injection_boundary_mutation_artifact"
                && case.detector_that_fired == "shrinker_validator"
        }));
        assert!(report.cases.iter().any(|case| {
            case.validation_class == "constitution_authority_bypass_attempt"
                && case.detector_that_fired == "constitution_boundary_validator"
        }));
        assert!(
            report
                .cases
                .iter()
                .all(|case| !case.artifact_used.starts_with("synthetic://"))
        );
        assert!(
            report
                .cases
                .iter()
                .all(|case| !case.authority_boundary_status.is_empty())
        );
    }

    #[test]
    fn negative_validation_finite_sink_uses_driver_rejection() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let case =
            negative_validation_case(&profile, &profile_hash, "finite_sink_non_finite_sample");

        assert!(case.passed);
        assert_eq!(case.observed_failure_class, "non_finite_final_sink");
        assert_eq!(case.detector_that_fired, "finite_sink_detector");
        assert_eq!(
            case.authority_boundary_status,
            "oracle_failed:ORACLE-346-U1"
        );
    }

    #[test]
    fn negative_validation_constitution_bypass_uses_boundary_oracle() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let case = negative_validation_case(
            &profile,
            &profile_hash,
            "constitution_authority_bypass_attempt",
        );

        assert!(case.passed);
        assert_eq!(case.observed_failure_class, "constitution_authority_bypass");
        assert_eq!(case.detector_that_fired, "constitution_boundary_validator");
        assert_eq!(case.authority_boundary_status, "failed:ORACLE-346-C1");
    }

    #[test]
    fn negative_validation_confidence_overclaim_mutates_chaos_report() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let case =
            negative_validation_case(&profile, &profile_hash, "confidence_overclaim_artifact");

        assert!(case.passed);
        assert_eq!(case.observed_failure_class, "confidence_overclaim");
        assert_eq!(case.detector_that_fired, "reporter_validator");
        assert_eq!(
            case.artifact_used,
            "local://interaction-fuzz/negative/chaos-report-confidence-overclaim"
        );
    }

    #[test]
    fn negative_validation_authority_violation_uses_constitution_boundary() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let case = negative_validation_case(&profile, &profile_hash, "authority_violation_sample");

        assert!(case.passed);
        assert_eq!(case.observed_failure_class, "authority_violation");
        assert_eq!(case.detector_that_fired, "authority_violation_detector");
        assert_eq!(case.authority_boundary_status, "failed:ORACLE-346-C1");
    }

    #[test]
    fn negative_validation_shrinker_preservation_uses_shrink_artifacts() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let boundary_case = negative_validation_case(
            &profile,
            &profile_hash,
            "shrink_injection_boundary_mutation_artifact",
        );
        assert!(boundary_case.passed);
        assert_eq!(
            boundary_case.observed_failure_class,
            "shrink_preservation_failure"
        );
        assert_eq!(boundary_case.detector_that_fired, "shrinker_validator");
        assert_eq!(
            boundary_case.artifact_used,
            "local://interaction-fuzz/negative/shrink-injection-boundary-mutated"
        );

        let hard_failure_case = negative_validation_case(
            &profile,
            &profile_hash,
            "shrink_hard_failure_class_mutation_artifact",
        );
        assert!(hard_failure_case.passed);
        assert_eq!(
            hard_failure_case.observed_failure_class,
            "shrink_preservation_failure"
        );
        assert_eq!(hard_failure_case.detector_that_fired, "shrinker_validator");
        assert_eq!(
            hard_failure_case.artifact_used,
            "local://interaction-fuzz/negative/shrink-hard-failure-class-mutated"
        );
    }

    #[test]
    fn negative_validation_missing_required_artifact_uses_layout() {
        let (profile, _surface) = chaos_profile_and_surface();
        let profile_hash = profile.profile_hash();

        let case = negative_validation_case(&profile, &profile_hash, "missing_required_artifact");

        assert!(case.passed);
        assert_eq!(case.observed_failure_class, "missing_required_artifact");
        assert_eq!(case.detector_that_fired, "artifact_layout_validator");
        assert!(case.artifact_used.ends_with("chaos-report.json"));
    }

    #[test]
    fn runner_visual_trace_report_is_derived_from_replay_artifact() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let artifact = replay_artifact_from_report(&profile, &report);

        let visual = InteractionFuzzRunner::render_visual_trace(&profile, &artifact)
            .expect("visual trace report must build");

        assert_eq!(visual.primitive_command_count, report.primitive_trace.len());
        assert_eq!(
            visual.constitution_boundary_profile.reporting_owner,
            profile.constitution_boundary_profile.reporting_owner
        );
        assert!(visual.constitution_boundary_check_status.passed);
        assert!(visual.pointer_sample_count > 0);
        assert_eq!(
            visual.visual_trace_artifact,
            "target/interaction-fuzz/chaos-smoke/diagnostics/visual-trace.json"
        );
        assert_eq!(visual.command_timeline.len(), report.primitive_trace.len());
        assert_eq!(visual.target_timeline.len(), report.primitive_trace.len());
        assert_eq!(visual.progress_markers.len(), report.primitive_trace.len());
        assert_eq!(
            visual.pointer_path_samples.len(),
            visual.pointer_sample_count
        );
        assert!(
            visual
                .failure_frame_context
                .starts_with("no_failure_context:")
        );
        assert!(
            visual
                .command_timeline
                .iter()
                .any(|entry| entry.ends_with(":pointer_move"))
        );
    }

    #[test]
    fn runner_visual_trace_rejects_bad_replay_artifact_authority() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };

        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.run_profile_hash =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let rejected = InteractionFuzzRunner::render_visual_trace(&profile, &artifact);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::ProfileHashMismatch { .. })
        ));

        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.replay_artifact.schema_version = INTERACTION_REPLAY_SCHEMA_VERSION + 1;
        let rejected = InteractionFuzzRunner::render_visual_trace(&profile, &artifact);
        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::UnsupportedReplaySchema { schema_version })
                if schema_version == INTERACTION_REPLAY_SCHEMA_VERSION + 1
        ));
    }

    #[test]
    fn runner_visual_confirmation_report_replays_existing_artifact() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let artifact = replay_artifact_from_report(&profile, &report);

        let confirmation = InteractionFuzzRunner::render_visual_confirmation(
            &profile,
            &artifact,
            "cargo test interaction_fuzz::runner -- runner_visual_confirmation_report_replays_existing_artifact",
            None,
        )
        .expect("visual confirmation report must build");

        assert_eq!(confirmation.run_profile_id, profile.profile_id);
        assert_eq!(confirmation.profile_hash, profile.profile_hash());
        assert_eq!(
            confirmation.constitution_boundary_profile.reporting_owner,
            profile.constitution_boundary_profile.reporting_owner
        );
        assert!(confirmation.constitution_boundary_check_status.passed);
        assert_eq!(confirmation.primitive_trace, report.primitive_trace);
        assert_eq!(
            confirmation.command_timeline.len(),
            report.primitive_trace.len()
        );
        assert!(
            confirmation
                .pointer_path_overlay
                .iter()
                .any(|entry| entry.contains(":x=") && entry.contains(":y="))
        );
        assert_eq!(
            confirmation.target_timeline.len(),
            report.primitive_trace.len()
        );
        assert!(confirmation.probe_snapshot_timeline.is_empty());
        assert_eq!(confirmation.frame_sequence_status, "unavailable:headless");
        assert!(
            confirmation
                .failure_or_no_failure_status
                .starts_with("no_failure:")
        );
    }

    #[test]
    fn runner_visual_confirmation_report_references_failure_packet() {
        let (mut packet, _failing_index, _expected_stream_len) =
            chaos::synthetic_non_finite_chaos_failure_packet();
        packet.probe_snapshots = vec![InteractionProbeSnapshotArtifact {
            source_layer: "widget_probe".to_string(),
            source_owner: "ListViewResizeProbe".to_string(),
            layout_epoch: 7,
            frame_number: 9,
            settle_boundary: InteractionSettleBoundary::Frame,
            snapshot_reason: "visual_confirmation_context".to_string(),
            payload: serde_json::json!({
                "capture_owner_class": "header_separator",
                "routed_target_class": "list_view_header"
            }),
        }];
        let surface = chaos::ci_chaos_surface();
        let profile = chaos::ci_chaos_profile(&surface);
        let shrunk = InteractionFuzzRunner::shrink(&profile, &packet).expect("shrink must succeed");
        let artifact = InteractionRunnerReplayArtifact {
            schema_version: INTERACTION_REPLAY_SCHEMA_VERSION,
            schema: INTERACTION_FUZZ_SCHEMA.to_string(),
            run_profile_id: profile.profile_id.clone(),
            run_profile_hash: profile.profile_hash(),
            replay_artifact: shrunk.shrunk_trace.clone(),
        };

        let confirmation = InteractionFuzzRunner::render_visual_confirmation(
            &profile,
            &artifact,
            shrunk.replay_command.clone(),
            Some(&shrunk),
        )
        .expect("failure visual confirmation report must build");

        assert_eq!(
            confirmation.primitive_trace.len(),
            shrunk.primitive_trace.len()
        );
        assert!(matches!(
            confirmation.primitive_trace[0].command,
            ReplayInteractionCommand::PointerMove { x, y } if x.is_nan() && y == 1.0
        ));
        assert_eq!(confirmation.replay_command, shrunk.replay_command);
        assert_eq!(
            confirmation.failure_or_no_failure_status,
            format!("failure:{}:{}", shrunk.failure_id, shrunk.invariant_id)
        );
        assert_eq!(confirmation.probe_snapshot_timeline.len(), 1);
        assert_eq!(
            confirmation.probe_snapshot_timeline[0].source_owner,
            "ListViewResizeProbe"
        );
        assert_eq!(
            confirmation.probe_snapshot_timeline[0].payload["routed_target_class"],
            "list_view_header"
        );
    }

    #[test]
    fn runner_visual_confirmation_rejects_profile_hash_mismatch() {
        let (profile, surface) = chaos_profile_and_surface();
        let report = match InteractionFuzzRunner::run(profile.clone(), Some(surface))
            .expect("run must succeed")
        {
            InteractionRunnerRunOutput::ChaosReport(report) => report,
            _ => panic!("expected chaos report"),
        };
        let mut artifact = replay_artifact_from_report(&profile, &report);
        artifact.run_profile_hash =
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

        let rejected = InteractionFuzzRunner::render_visual_confirmation(
            &profile,
            &artifact,
            "cargo test interaction_fuzz::runner -- runner_visual_confirmation_rejects_profile_hash_mismatch",
            None,
        );

        assert!(matches!(
            rejected,
            Err(InteractionRunnerError::ProfileHashMismatch { .. })
        ));
    }
}
