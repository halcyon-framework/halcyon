//! Isolated relative-pointer lease admission and platform ownership.

use crate::core::event::{
    CaptureMode, GestureCandidate, GestureCandidateId, GestureKind, GestureSessionId,
    InteractionDomainId, PointerButton, PointerId, WindowId,
};
use crate::core::math::Point;
#[cfg(any(target_os = "macos", test))]
use crate::core::relative_pointer::{
    CursorRestorePolicy, PointerMotionPolicy, RelativePointerLeaseIdAllocator,
};
use crate::core::relative_pointer::{
    RelativePointerAdmissionError, RelativePointerIngressGate, RelativePointerLeaseBinding,
    RelativePointerLeaseId,
};
use crate::platform::relative_pointer::RelativePointerPlatformCleanupOutcome;
#[cfg(any(target_os = "macos", test))]
use crate::platform::relative_pointer::{
    HiddenRelativePointerPlatformState, RelativePointerPlatform, RelativePointerPlatformMediator,
};
use crate::utils::id::TreeWidgetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelativePointerHardCaptureFacts {
    pub pointer_id: PointerId,
    pub owner: TreeWidgetId,
    pub button: PointerButton,
    pub mode: CaptureMode,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RelativePointerAdmissionFacts {
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub window_id: WindowId,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub interaction_domain_id: InteractionDomainId,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub pointer_id: PointerId,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub gesture_session_id: GestureSessionId,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub candidate: GestureCandidate,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub committed_candidate_id: Option<GestureCandidateId>,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub pointer_button: PointerButton,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub down_anchor: Option<Point>,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub focused_window_id: Option<WindowId>,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub owner_available: bool,
    #[cfg_attr(
        not(any(target_os = "macos", test)),
        allow(
            dead_code,
            reason = "Shared HAL-362 admission fact; concrete lock backend currently installed only on macOS"
        )
    )]
    pub hard_capture: Option<RelativePointerHardCaptureFacts>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RelativePointerLeaseActivation {
    binding: RelativePointerLeaseBinding,
    ingress_gate: RelativePointerIngressGate,
    gesture_kind: GestureKind,
}

impl RelativePointerLeaseActivation {
    pub const fn binding(self) -> RelativePointerLeaseBinding {
        self.binding
    }

    pub const fn ingress_gate(self) -> RelativePointerIngressGate {
        self.ingress_gate
    }

    pub const fn gesture_kind(self) -> GestureKind {
        self.gesture_kind
    }
}

#[cfg(any(target_os = "macos", test))]
struct ActiveRelativePointerLease {
    activation: RelativePointerLeaseActivation,
    platform_state: HiddenRelativePointerPlatformState,
}

#[cfg(any(target_os = "macos", test))]
pub(crate) struct RelativePointerLeaseAuthority<P: RelativePointerPlatform> {
    window_id: WindowId,
    interaction_domain_id: InteractionDomainId,
    allocator: RelativePointerLeaseIdAllocator,
    platform: RelativePointerPlatformMediator<P>,
    active: Option<ActiveRelativePointerLease>,
}

pub(crate) trait RelativePointerLeaseControl {
    fn admit(
        &mut self,
        facts: RelativePointerAdmissionFacts,
    ) -> Result<RelativePointerLeaseActivation, RelativePointerAdmissionError>;
    fn active(&self) -> Option<RelativePointerLeaseActivation>;
    fn binding_for_motion(
        &self,
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        pointer_id: PointerId,
    ) -> Option<RelativePointerLeaseBinding>;
    fn restoration_supported(&self, binding: RelativePointerLeaseBinding) -> bool;
    fn terminate(
        &mut self,
        expected_lease_id: RelativePointerLeaseId,
        safe_to_restore_down_anchor: bool,
    ) -> Option<(
        RelativePointerLeaseBinding,
        RelativePointerPlatformCleanupOutcome,
    )>;
}

#[cfg(any(target_os = "macos", test))]
impl<P: RelativePointerPlatform> RelativePointerLeaseAuthority<P> {
    pub(crate) fn new(
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        platform: P,
    ) -> Self {
        Self {
            window_id,
            interaction_domain_id,
            allocator: RelativePointerLeaseIdAllocator::new(),
            platform: RelativePointerPlatformMediator::new(platform),
            active: None,
        }
    }

    pub(crate) fn admit(
        &mut self,
        facts: RelativePointerAdmissionFacts,
    ) -> Result<RelativePointerLeaseActivation, RelativePointerAdmissionError> {
        if self.active.is_some() {
            return Err(RelativePointerAdmissionError::ConflictingLease);
        }

        let restore_policy = self.validate(facts)?;
        let lock = self.platform.acquire(facts.window_id, restore_policy)?;
        let normalization_epoch = lock.normalization_epoch();
        let lease_id = match self.allocator.mint() {
            Ok(lease_id) => lease_id,
            Err(_) => {
                if let Err(error) = self.platform.release_unhidden(lock) {
                    tracing::error!(
                        ?error,
                        "relative pointer lock release failed after identity exhaustion"
                    );
                }
                return Err(RelativePointerAdmissionError::IdentityExhausted);
            }
        };
        let binding = match RelativePointerLeaseBinding::try_new(
            lease_id,
            facts.window_id,
            facts.interaction_domain_id,
            facts.pointer_id,
            facts.gesture_session_id,
            facts.candidate.candidate_id,
            facts.candidate.widget_id,
            facts.pointer_button,
            restore_policy,
            facts.down_anchor,
            normalization_epoch,
        ) {
            Ok(binding) => binding,
            Err(_) => {
                if let Err(error) = self.platform.release_unhidden(lock) {
                    tracing::error!(
                        ?error,
                        "relative pointer lock release failed after stale binding"
                    );
                }
                return Err(RelativePointerAdmissionError::StaleBinding);
            }
        };

        let platform_state = self.platform.hide_cursor(lock, lease_id).map_err(|error| {
            if error.show_result().is_err() || error.release_result().is_err() {
                tracing::error!(
                    show_result = ?error.show_result(),
                    release_result = ?error.release_result(),
                    "relative pointer hide rollback failed"
                );
            }
            error.reason()
        })?;
        let activation = RelativePointerLeaseActivation {
            binding,
            ingress_gate: RelativePointerIngressGate::from_binding(&binding),
            gesture_kind: facts.candidate.kind,
        };
        self.active = Some(ActiveRelativePointerLease {
            activation,
            platform_state,
        });
        Ok(activation)
    }

    pub(crate) fn active(&self) -> Option<RelativePointerLeaseActivation> {
        self.active.as_ref().map(|active| active.activation)
    }

    pub(crate) fn binding_for_motion(
        &self,
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        pointer_id: PointerId,
    ) -> Option<RelativePointerLeaseBinding> {
        let binding = self.active()?.binding();
        (binding.lease_id() == lease_id
            && binding.window_id() == window_id
            && binding.interaction_domain_id() == interaction_domain_id
            && binding.pointer_id() == pointer_id)
            .then_some(binding)
    }

    pub(crate) fn terminate(
        &mut self,
        expected_lease_id: RelativePointerLeaseId,
        safe_to_restore_down_anchor: bool,
    ) -> Option<(
        RelativePointerLeaseBinding,
        RelativePointerPlatformCleanupOutcome,
    )> {
        if self.active()?.binding().lease_id() != expected_lease_id {
            return None;
        }

        let active = self.active.take()?;
        let binding = active.activation.binding();
        let safe_anchor = safe_to_restore_down_anchor
            .then_some(binding.down_anchor())
            .flatten();
        let cleanup = self.platform.terminate(active.platform_state, safe_anchor);
        Some((binding, cleanup))
    }

    fn validate(
        &self,
        facts: RelativePointerAdmissionFacts,
    ) -> Result<CursorRestorePolicy, RelativePointerAdmissionError> {
        if facts.window_id != self.window_id
            || facts.interaction_domain_id != self.interaction_domain_id
        {
            return Err(RelativePointerAdmissionError::WindowDomainMismatch);
        }
        if facts.committed_candidate_id != Some(facts.candidate.candidate_id) {
            return Err(RelativePointerAdmissionError::GestureNotCommitted);
        }
        if facts.candidate.candidate_id.session != facts.gesture_session_id {
            return Err(RelativePointerAdmissionError::StaleBinding);
        }
        let restore_policy = match facts.candidate.motion_policy {
            PointerMotionPolicy::Absolute => {
                return Err(RelativePointerAdmissionError::StaleBinding);
            }
            PointerMotionPolicy::RelativeLocked { restore } => restore,
        };
        let matching_capture = facts.hard_capture.is_some_and(|capture| {
            capture.pointer_id == facts.pointer_id
                && capture.owner == facts.candidate.widget_id
                && capture.button == facts.pointer_button
                && capture.mode == CaptureMode::Hard
        });
        if !matching_capture {
            return Err(RelativePointerAdmissionError::HardCaptureNotOwned);
        }
        if !facts.owner_available {
            return Err(RelativePointerAdmissionError::OwnerUnavailable);
        }
        if facts.focused_window_id != Some(facts.window_id) {
            return Err(RelativePointerAdmissionError::WindowNotFocused);
        }
        match (restore_policy, facts.down_anchor) {
            (CursorRestorePolicy::None, None) => {}
            (CursorRestorePolicy::DownAnchor, Some(anchor))
                if anchor.x.is_finite() && anchor.y.is_finite() => {}
            _ => return Err(RelativePointerAdmissionError::StaleBinding),
        }
        Ok(restore_policy)
    }
}

#[cfg(any(target_os = "macos", test))]
impl<P: RelativePointerPlatform> Drop for RelativePointerLeaseAuthority<P> {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            let cleanup = self.platform.terminate(active.platform_state, None);
            if !cleanup.is_clean() {
                tracing::error!(?cleanup, "relative pointer drop cleanup failed");
            }
        }
    }
}

#[cfg(any(target_os = "macos", test))]
impl<P: RelativePointerPlatform> RelativePointerLeaseControl for RelativePointerLeaseAuthority<P> {
    fn admit(
        &mut self,
        facts: RelativePointerAdmissionFacts,
    ) -> Result<RelativePointerLeaseActivation, RelativePointerAdmissionError> {
        Self::admit(self, facts)
    }

    fn active(&self) -> Option<RelativePointerLeaseActivation> {
        Self::active(self)
    }

    fn binding_for_motion(
        &self,
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        pointer_id: PointerId,
    ) -> Option<RelativePointerLeaseBinding> {
        Self::binding_for_motion(self, lease_id, window_id, interaction_domain_id, pointer_id)
    }

    fn restoration_supported(&self, binding: RelativePointerLeaseBinding) -> bool {
        self.platform
            .support(binding.window_id(), CursorRestorePolicy::DownAnchor)
            .is_ok_and(|epoch| epoch == binding.normalization_epoch())
    }

    fn terminate(
        &mut self,
        expected_lease_id: RelativePointerLeaseId,
        safe_to_restore_down_anchor: bool,
    ) -> Option<(
        RelativePointerLeaseBinding,
        RelativePointerPlatformCleanupOutcome,
    )> {
        Self::terminate(self, expected_lease_id, safe_to_restore_down_anchor)
    }
}
