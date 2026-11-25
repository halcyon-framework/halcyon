//! Canonical relative-pointer types (SPEC-HAL-362).

use std::num::NonZeroU64;

use crate::core::event::{
    GestureCandidateId, GestureSessionId, InteractionDomainId, PointerButton, PointerId, WindowId,
};
use crate::core::math::Point;
use crate::utils::id::TreeWidgetId;

/// Motion representation requested by a gesture candidate before commit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PointerMotionPolicy {
    #[default]
    Absolute,
    RelativeLocked {
        restore: CursorRestorePolicy,
    },
}

/// Cursor restoration requested for a relative-pointer lease.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CursorRestorePolicy {
    #[default]
    None,
    DownAnchor,
}

/// A finite displacement in the bound window's +right/+down logical basis.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RelativeLogicalDelta {
    x: f32,
    y: f32,
}

impl RelativeLogicalDelta {
    pub fn try_new(x: f32, y: f32) -> Result<Self, RelativeLogicalDeltaError> {
        if !x.is_finite() {
            return Err(RelativeLogicalDeltaError::NonFiniteX);
        }
        if !y.is_finite() {
            return Err(RelativeLogicalDeltaError::NonFiniteY);
        }
        Ok(Self { x, y })
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn components(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeLogicalDeltaError {
    NonFiniteX,
    NonFiniteY,
}

/// Nonzero, engine-minted identity for one relative-pointer lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelativePointerLeaseId(NonZeroU64);

impl RelativePointerLeaseId {
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Owned lease-ID source. The future engine owner stores one instance per run.
///
/// This deliberately has no global or singleton backing.
#[cfg(any(target_os = "macos", test))]
#[derive(Debug)]
pub(crate) struct RelativePointerLeaseIdAllocator {
    next: Option<NonZeroU64>,
}

#[cfg(any(target_os = "macos", test))]
impl RelativePointerLeaseIdAllocator {
    pub(crate) fn new() -> Self {
        Self {
            next: NonZeroU64::new(1),
        }
    }

    pub(crate) fn mint(&mut self) -> Result<RelativePointerLeaseId, RelativePointerIdentityError> {
        let current = self.next.ok_or(RelativePointerIdentityError::Exhausted)?;
        self.next = current.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(RelativePointerLeaseId(current))
    }
}

#[cfg(any(target_os = "macos", test))]
impl Default for RelativePointerLeaseIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Identity of the authoritative relative-motion normalization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelativePointerNormalizationEpoch(NonZeroU64);

impl RelativePointerNormalizationEpoch {
    pub fn new(value: u64) -> Result<Self, RelativePointerIdentityError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(RelativePointerIdentityError::Zero)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerIdentityError {
    Zero,
    Exhausted,
}

/// Immutable identity binding for one admitted relative-pointer lease.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativePointerLeaseBinding {
    lease_id: RelativePointerLeaseId,
    window_id: WindowId,
    interaction_domain_id: InteractionDomainId,
    pointer_id: PointerId,
    gesture_session_id: GestureSessionId,
    winning_candidate_id: GestureCandidateId,
    capture_owner: TreeWidgetId,
    pointer_button: PointerButton,
    restore_policy: CursorRestorePolicy,
    down_anchor: Option<Point>,
    normalization_epoch: RelativePointerNormalizationEpoch,
}

impl RelativePointerLeaseBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        interaction_domain_id: InteractionDomainId,
        pointer_id: PointerId,
        gesture_session_id: GestureSessionId,
        winning_candidate_id: GestureCandidateId,
        capture_owner: TreeWidgetId,
        pointer_button: PointerButton,
        restore_policy: CursorRestorePolicy,
        down_anchor: Option<Point>,
        normalization_epoch: RelativePointerNormalizationEpoch,
    ) -> Result<Self, RelativePointerBindingError> {
        if winning_candidate_id.session != gesture_session_id {
            return Err(RelativePointerBindingError::CandidateSessionMismatch);
        }

        match (restore_policy, down_anchor) {
            (CursorRestorePolicy::None, Some(_)) => {
                return Err(RelativePointerBindingError::UnexpectedDownAnchor);
            }
            (CursorRestorePolicy::DownAnchor, None) => {
                return Err(RelativePointerBindingError::MissingDownAnchor);
            }
            (_, Some(anchor)) if !anchor.x.is_finite() || !anchor.y.is_finite() => {
                return Err(RelativePointerBindingError::NonFiniteDownAnchor);
            }
            _ => {}
        }

        Ok(Self {
            lease_id,
            window_id,
            interaction_domain_id,
            pointer_id,
            gesture_session_id,
            winning_candidate_id,
            capture_owner,
            pointer_button,
            restore_policy,
            down_anchor,
            normalization_epoch,
        })
    }

    pub const fn lease_id(&self) -> RelativePointerLeaseId {
        self.lease_id
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn interaction_domain_id(&self) -> InteractionDomainId {
        self.interaction_domain_id
    }

    pub const fn pointer_id(&self) -> PointerId {
        self.pointer_id
    }

    pub const fn gesture_session_id(&self) -> GestureSessionId {
        self.gesture_session_id
    }

    pub const fn winning_candidate_id(&self) -> GestureCandidateId {
        self.winning_candidate_id
    }

    pub const fn capture_owner(&self) -> TreeWidgetId {
        self.capture_owner
    }

    pub const fn pointer_button(&self) -> PointerButton {
        self.pointer_button
    }

    pub const fn restore_policy(&self) -> CursorRestorePolicy {
        self.restore_policy
    }

    pub const fn down_anchor(&self) -> Option<Point> {
        self.down_anchor
    }

    pub const fn normalization_epoch(&self) -> RelativePointerNormalizationEpoch {
        self.normalization_epoch
    }
}

/// Immutable projection of the active lease used to admit relative ingress.
///
/// The semantic lease owner supplies this value to InputFusion. Possessing a
/// gate does not create, renew, or terminate a lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativePointerIngressGate {
    lease_id: RelativePointerLeaseId,
    window_id: WindowId,
    pointer_id: PointerId,
    normalization_epoch: RelativePointerNormalizationEpoch,
}

impl RelativePointerIngressGate {
    pub const fn from_binding(binding: &RelativePointerLeaseBinding) -> Self {
        Self {
            lease_id: binding.lease_id(),
            window_id: binding.window_id(),
            pointer_id: binding.pointer_id(),
            normalization_epoch: binding.normalization_epoch(),
        }
    }

    pub const fn lease_id(self) -> RelativePointerLeaseId {
        self.lease_id
    }

    pub const fn window_id(self) -> WindowId {
        self.window_id
    }

    pub const fn pointer_id(self) -> PointerId {
        self.pointer_id
    }

    pub const fn normalization_epoch(self) -> RelativePointerNormalizationEpoch {
        self.normalization_epoch
    }

    pub(crate) fn admits(
        self,
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        pointer_id: PointerId,
        normalization_epoch: RelativePointerNormalizationEpoch,
    ) -> bool {
        self.lease_id == lease_id
            && self.window_id == window_id
            && self.pointer_id == pointer_id
            && self.normalization_epoch == normalization_epoch
    }

    pub(crate) fn suppresses_absolute(self, window_id: WindowId, pointer_id: PointerId) -> bool {
        self.window_id == window_id && self.pointer_id == pointer_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerBindingError {
    CandidateSessionMismatch,
    MissingDownAnchor,
    UnexpectedDownAnchor,
    NonFiniteDownAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerAdmissionError {
    GestureNotCommitted,
    HardCaptureNotOwned,
    StaleBinding,
    OwnerUnavailable,
    WindowNotFocused,
    WindowDomainMismatch,
    ConflictingLease,
    CapabilityUnsupported,
    NormalizationUnavailable,
    RestorationUnsupported,
    PlatformLockDenied,
    CursorHideDenied,
    SecurityDenied,
    IdentityExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerCancellationReason {
    ExplicitSemanticCancel,
    FocusLost,
    CaptureLost,
    OwnerUnmounted,
    WindowDestroyed,
    ApplicationShutdown,
    PlatformLockLost,
    SecurityRevoked,
    NormalizationChanged,
    InvalidRelativeMotion,
    ProtocolViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorRestoreOutcome {
    NotRequested,
    Restored,
    SkippedUnsafeContext,
    FailedPlatformOperation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativePointerMotionEvent {
    pub lease_id: RelativePointerLeaseId,
    pub pointer_id: PointerId,
    pub delta: RelativeLogicalDelta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerTerminalOutcome {
    Completed {
        restore: CursorRestoreOutcome,
    },
    Cancelled {
        reason: RelativePointerCancellationReason,
        restore: CursorRestoreOutcome,
    },
    AdmissionDenied(RelativePointerAdmissionError),
}
