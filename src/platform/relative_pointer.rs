//! Fakeable platform contract for relative pointer locking (SPEC-HAL-362).

use crate::core::event::WindowId;
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestoreOutcome, CursorRestorePolicy, RelativePointerAdmissionError,
    RelativePointerLeaseId, RelativePointerNormalizationEpoch,
};

/// Read-only capability result for one live window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerPlatformSupport {
    Unsupported,
    NormalizationUnavailable,
    Supported {
        normalization_epoch: RelativePointerNormalizationEpoch,
        restore_down_anchor: bool,
    },
}

/// Typed platform failure retained inside the engine/platform boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePointerPlatformError {
    Unsupported,
    NormalizationUnavailable,
    RestorationUnsupported,
    Denied,
    SecurityDenied,
    FailedPlatformOperation,
}

/// Platform operations needed by the relative-pointer lease protocol.
///
/// `support` must be side-effect free. `release` and `show_cursor` must be
/// idempotent so every failure path can attempt complete cleanup.
pub trait RelativePointerPlatform {
    fn support(&self, window_id: WindowId) -> RelativePointerPlatformSupport;

    fn acquire(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError>;

    fn release(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError>;

    fn hide_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError>;

    fn show_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError>;

    fn restore_cursor(
        &mut self,
        window_id: WindowId,
        anchor: Point,
    ) -> Result<(), RelativePointerPlatformError>;
}

/// Acquired platform lock. The engine may publish its closed-ingress lease
/// before consuming this token with `hide_cursor`.
#[derive(Debug)]
#[must_use = "a relative-pointer platform lock must be hidden or explicitly released"]
pub struct RelativePointerPlatformLock {
    window_id: WindowId,
    restore_policy: CursorRestorePolicy,
    normalization_epoch: RelativePointerNormalizationEpoch,
}

impl RelativePointerPlatformLock {
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn restore_policy(&self) -> CursorRestorePolicy {
        self.restore_policy
    }

    pub const fn normalization_epoch(&self) -> RelativePointerNormalizationEpoch {
        self.normalization_epoch
    }
}

/// Platform lock whose cursor presentation is known hidden.
#[derive(Debug)]
#[must_use = "a hidden relative-pointer platform state must be terminated"]
pub struct HiddenRelativePointerPlatformState {
    lease_id: RelativePointerLeaseId,
    window_id: WindowId,
    restore_policy: CursorRestorePolicy,
    normalization_epoch: RelativePointerNormalizationEpoch,
}

impl HiddenRelativePointerPlatformState {
    pub const fn lease_id(&self) -> RelativePointerLeaseId {
        self.lease_id
    }

    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub const fn restore_policy(&self) -> CursorRestorePolicy {
        self.restore_policy
    }

    pub const fn normalization_epoch(&self) -> RelativePointerNormalizationEpoch {
        self.normalization_epoch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativePointerPlatformActivationError {
    reason: RelativePointerAdmissionError,
    show_result: Result<(), RelativePointerPlatformError>,
    release_result: Result<(), RelativePointerPlatformError>,
}

impl RelativePointerPlatformActivationError {
    pub const fn reason(&self) -> RelativePointerAdmissionError {
        self.reason
    }

    pub const fn show_result(&self) -> &Result<(), RelativePointerPlatformError> {
        &self.show_result
    }

    pub const fn release_result(&self) -> &Result<(), RelativePointerPlatformError> {
        &self.release_result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativePointerPlatformCleanupOutcome {
    restore: CursorRestoreOutcome,
    release_result: Result<(), RelativePointerPlatformError>,
    show_result: Result<(), RelativePointerPlatformError>,
}

impl RelativePointerPlatformCleanupOutcome {
    pub const fn restore(&self) -> CursorRestoreOutcome {
        self.restore
    }

    pub const fn release_result(&self) -> &Result<(), RelativePointerPlatformError> {
        &self.release_result
    }

    pub const fn show_result(&self) -> &Result<(), RelativePointerPlatformError> {
        &self.show_result
    }

    pub fn is_clean(&self) -> bool {
        self.release_result.is_ok() && self.show_result.is_ok()
    }
}

/// Explicit platform protocol driver. It performs work only when called.
pub struct RelativePointerPlatformMediator<P> {
    platform: P,
}

impl<P: RelativePointerPlatform> RelativePointerPlatformMediator<P> {
    pub const fn new(platform: P) -> Self {
        Self { platform }
    }

    pub const fn platform(&self) -> &P {
        &self.platform
    }

    pub fn into_inner(self) -> P {
        self.platform
    }

    pub fn support(
        &self,
        window_id: WindowId,
        restore_policy: CursorRestorePolicy,
    ) -> Result<RelativePointerNormalizationEpoch, RelativePointerAdmissionError> {
        match self.platform.support(window_id) {
            RelativePointerPlatformSupport::Unsupported => {
                Err(RelativePointerAdmissionError::CapabilityUnsupported)
            }
            RelativePointerPlatformSupport::NormalizationUnavailable => {
                Err(RelativePointerAdmissionError::NormalizationUnavailable)
            }
            RelativePointerPlatformSupport::Supported {
                normalization_epoch,
                restore_down_anchor,
            } => {
                if restore_policy == CursorRestorePolicy::DownAnchor && !restore_down_anchor {
                    Err(RelativePointerAdmissionError::RestorationUnsupported)
                } else {
                    Ok(normalization_epoch)
                }
            }
        }
    }

    pub fn acquire(
        &mut self,
        window_id: WindowId,
        restore_policy: CursorRestorePolicy,
    ) -> Result<RelativePointerPlatformLock, RelativePointerAdmissionError> {
        let normalization_epoch = self.support(window_id, restore_policy)?;
        self.platform.acquire(window_id).map_err(map_lock_error)?;

        Ok(RelativePointerPlatformLock {
            window_id,
            restore_policy,
            normalization_epoch,
        })
    }

    pub fn release_unhidden(
        &mut self,
        lock: RelativePointerPlatformLock,
    ) -> Result<(), RelativePointerPlatformError> {
        self.platform.release(lock.window_id)
    }

    pub fn hide_cursor(
        &mut self,
        lock: RelativePointerPlatformLock,
        lease_id: RelativePointerLeaseId,
    ) -> Result<HiddenRelativePointerPlatformState, RelativePointerPlatformActivationError> {
        if let Err(error) = self.platform.hide_cursor(lock.window_id) {
            let show_result = self.platform.show_cursor(lock.window_id);
            let release_result = self.platform.release(lock.window_id);
            return Err(RelativePointerPlatformActivationError {
                reason: map_hide_error(error),
                show_result,
                release_result,
            });
        }

        Ok(HiddenRelativePointerPlatformState {
            lease_id,
            window_id: lock.window_id,
            restore_policy: lock.restore_policy,
            normalization_epoch: lock.normalization_epoch,
        })
    }

    /// Restore when requested and safe, then release the lock and show the
    /// cursor. Cleanup continues after every platform failure.
    pub fn terminate(
        &mut self,
        hidden: HiddenRelativePointerPlatformState,
        safe_down_anchor: Option<Point>,
    ) -> RelativePointerPlatformCleanupOutcome {
        let restore = match (hidden.restore_policy, safe_down_anchor) {
            (CursorRestorePolicy::None, _) => CursorRestoreOutcome::NotRequested,
            (CursorRestorePolicy::DownAnchor, Some(anchor))
                if anchor.x.is_finite() && anchor.y.is_finite() =>
            {
                match self.platform.restore_cursor(hidden.window_id, anchor) {
                    Ok(()) => CursorRestoreOutcome::Restored,
                    Err(_) => CursorRestoreOutcome::FailedPlatformOperation,
                }
            }
            (CursorRestorePolicy::DownAnchor, _) => CursorRestoreOutcome::SkippedUnsafeContext,
        };

        let release_result = self.platform.release(hidden.window_id);
        let show_result = self.platform.show_cursor(hidden.window_id);

        RelativePointerPlatformCleanupOutcome {
            restore,
            release_result,
            show_result,
        }
    }
}

fn map_lock_error(error: RelativePointerPlatformError) -> RelativePointerAdmissionError {
    match error {
        RelativePointerPlatformError::Unsupported => {
            RelativePointerAdmissionError::CapabilityUnsupported
        }
        RelativePointerPlatformError::NormalizationUnavailable => {
            RelativePointerAdmissionError::NormalizationUnavailable
        }
        RelativePointerPlatformError::RestorationUnsupported => {
            RelativePointerAdmissionError::RestorationUnsupported
        }
        RelativePointerPlatformError::SecurityDenied => {
            RelativePointerAdmissionError::SecurityDenied
        }
        RelativePointerPlatformError::Denied
        | RelativePointerPlatformError::FailedPlatformOperation => {
            RelativePointerAdmissionError::PlatformLockDenied
        }
    }
}

fn map_hide_error(error: RelativePointerPlatformError) -> RelativePointerAdmissionError {
    match error {
        RelativePointerPlatformError::SecurityDenied => {
            RelativePointerAdmissionError::SecurityDenied
        }
        _ => RelativePointerAdmissionError::CursorHideDenied,
    }
}
