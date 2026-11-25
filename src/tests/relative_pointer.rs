use std::cell::RefCell;

use crate::core::event::{
    GestureCandidateId, GestureSessionId, InteractionDomainId, PointerButton, PointerId, WindowId,
};
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestoreOutcome, CursorRestorePolicy, PointerMotionPolicy, RelativeLogicalDelta,
    RelativeLogicalDeltaError, RelativePointerAdmissionError, RelativePointerBindingError,
    RelativePointerLeaseBinding, RelativePointerLeaseIdAllocator,
    RelativePointerNormalizationEpoch,
};
use crate::platform::relative_pointer::{
    RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformMediator,
    RelativePointerPlatformSupport,
};
use crate::utils::id::TreeWidgetId;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Operation {
    Support(WindowId),
    Acquire(WindowId),
    Hide(WindowId),
    Restore(WindowId, Point),
    Release(WindowId),
    Show(WindowId),
}

struct FakeRelativePointerPlatform {
    support: RelativePointerPlatformSupport,
    acquire_error: Option<RelativePointerPlatformError>,
    hide_error: Option<RelativePointerPlatformError>,
    restore_error: Option<RelativePointerPlatformError>,
    release_error: Option<RelativePointerPlatformError>,
    show_error: Option<RelativePointerPlatformError>,
    operations: RefCell<Vec<Operation>>,
}

impl FakeRelativePointerPlatform {
    fn supported(restore_down_anchor: bool) -> Self {
        Self {
            support: RelativePointerPlatformSupport::Supported {
                normalization_epoch: RelativePointerNormalizationEpoch::new(7).unwrap(),
                restore_down_anchor,
            },
            acquire_error: None,
            hide_error: None,
            restore_error: None,
            release_error: None,
            show_error: None,
            operations: RefCell::new(Vec::new()),
        }
    }

    fn with_support(support: RelativePointerPlatformSupport) -> Self {
        let mut platform = Self::supported(false);
        platform.support = support;
        platform
    }

    fn operations(&self) -> Vec<Operation> {
        self.operations.borrow().clone()
    }
}

impl RelativePointerPlatform for FakeRelativePointerPlatform {
    fn support(&self, window_id: WindowId) -> RelativePointerPlatformSupport {
        self.operations
            .borrow_mut()
            .push(Operation::Support(window_id));
        self.support
    }

    fn acquire(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(Operation::Acquire(window_id));
        self.acquire_error.map_or(Ok(()), Err)
    }

    fn release(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(Operation::Release(window_id));
        self.release_error.map_or(Ok(()), Err)
    }

    fn hide_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(Operation::Hide(window_id));
        self.hide_error.map_or(Ok(()), Err)
    }

    fn show_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(Operation::Show(window_id));
        self.show_error.map_or(Ok(()), Err)
    }

    fn restore_cursor(
        &mut self,
        window_id: WindowId,
        anchor: Point,
    ) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(Operation::Restore(window_id, anchor));
        self.restore_error.map_or(Ok(()), Err)
    }
}

#[test]
fn canonical_types_reject_ambiguous_identity_and_non_finite_motion() {
    assert_eq!(
        PointerMotionPolicy::default(),
        PointerMotionPolicy::Absolute
    );
    assert_eq!(
        RelativeLogicalDelta::try_new(f32::NAN, 1.0),
        Err(RelativeLogicalDeltaError::NonFiniteX)
    );
    assert_eq!(
        RelativeLogicalDelta::try_new(1.0, f32::INFINITY),
        Err(RelativeLogicalDeltaError::NonFiniteY)
    );
    assert_eq!(
        RelativeLogicalDelta::try_new(3.0, -4.0)
            .unwrap()
            .components(),
        (3.0, -4.0)
    );

    let mut ids = RelativePointerLeaseIdAllocator::new();
    let first = ids.mint().unwrap();
    let second = ids.mint().unwrap();
    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 2);
    assert_ne!(first, second);

    let epoch = RelativePointerNormalizationEpoch::new(7).unwrap();
    let session = GestureSessionId(11);
    let candidate = GestureCandidateId { session, local: 2 };
    let binding = RelativePointerLeaseBinding::try_new(
        first,
        WindowId(3),
        InteractionDomainId(4),
        PointerId::MOUSE,
        session,
        candidate,
        TreeWidgetId(5),
        PointerButton::Primary,
        CursorRestorePolicy::DownAnchor,
        Some(Point::new(8.0, 9.0)),
        epoch,
    )
    .unwrap();
    assert_eq!(binding.lease_id(), first);
    assert_eq!(binding.winning_candidate_id(), candidate);
    assert_eq!(binding.down_anchor(), Some(Point::new(8.0, 9.0)));

    assert_eq!(
        RelativePointerLeaseBinding::try_new(
            second,
            WindowId(3),
            InteractionDomainId(4),
            PointerId::MOUSE,
            session,
            GestureCandidateId {
                session: GestureSessionId(12),
                local: 2,
            },
            TreeWidgetId(5),
            PointerButton::Primary,
            CursorRestorePolicy::DownAnchor,
            Some(Point::new(8.0, 9.0)),
            epoch,
        ),
        Err(RelativePointerBindingError::CandidateSessionMismatch)
    );
    assert_eq!(
        RelativePointerLeaseBinding::try_new(
            second,
            WindowId(3),
            InteractionDomainId(4),
            PointerId::MOUSE,
            session,
            candidate,
            TreeWidgetId(5),
            PointerButton::Primary,
            CursorRestorePolicy::DownAnchor,
            None,
            epoch,
        ),
        Err(RelativePointerBindingError::MissingDownAnchor)
    );
}

#[test]
fn support_is_policy_specific_and_has_no_platform_side_effects() {
    let window = WindowId(41);
    for (support, expected) in [
        (
            RelativePointerPlatformSupport::Unsupported,
            RelativePointerAdmissionError::CapabilityUnsupported,
        ),
        (
            RelativePointerPlatformSupport::NormalizationUnavailable,
            RelativePointerAdmissionError::NormalizationUnavailable,
        ),
    ] {
        let mediator = RelativePointerPlatformMediator::new(
            FakeRelativePointerPlatform::with_support(support),
        );
        assert_eq!(
            mediator.support(window, CursorRestorePolicy::None),
            Err(expected)
        );
        assert_eq!(
            mediator.platform().operations(),
            vec![Operation::Support(window)]
        );
    }

    let mediator =
        RelativePointerPlatformMediator::new(FakeRelativePointerPlatform::supported(false));
    assert_eq!(
        mediator.support(window, CursorRestorePolicy::DownAnchor),
        Err(RelativePointerAdmissionError::RestorationUnsupported)
    );
    assert_eq!(
        mediator.platform().operations(),
        vec![Operation::Support(window)]
    );
}

#[test]
fn successful_platform_lifecycle_is_acquire_hide_restore_release_show() {
    let window = WindowId(42);
    let anchor = Point::new(80.0, 60.0);
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let lease_id = ids.mint().unwrap();
    let mut mediator =
        RelativePointerPlatformMediator::new(FakeRelativePointerPlatform::supported(true));

    let lock = mediator
        .acquire(window, CursorRestorePolicy::DownAnchor)
        .unwrap();
    assert_eq!(lock.normalization_epoch().get(), 7);
    let hidden = mediator.hide_cursor(lock, lease_id).unwrap();
    assert_eq!(hidden.lease_id(), lease_id);
    let outcome = mediator.terminate(hidden, Some(anchor));

    assert_eq!(outcome.restore(), CursorRestoreOutcome::Restored);
    assert!(outcome.is_clean());
    assert_eq!(
        mediator.platform().operations(),
        vec![
            Operation::Support(window),
            Operation::Acquire(window),
            Operation::Hide(window),
            Operation::Restore(window, anchor),
            Operation::Release(window),
            Operation::Show(window),
        ]
    );
}

#[test]
fn lock_and_hide_failures_are_typed_and_fail_closed() {
    let window = WindowId(43);
    let mut denied = FakeRelativePointerPlatform::supported(false);
    denied.acquire_error = Some(RelativePointerPlatformError::SecurityDenied);
    let mut mediator = RelativePointerPlatformMediator::new(denied);
    assert!(matches!(
        mediator.acquire(window, CursorRestorePolicy::None),
        Err(RelativePointerAdmissionError::SecurityDenied)
    ));
    assert_eq!(
        mediator.platform().operations(),
        vec![Operation::Support(window), Operation::Acquire(window)]
    );

    let mut hide_failure = FakeRelativePointerPlatform::supported(false);
    hide_failure.hide_error = Some(RelativePointerPlatformError::Denied);
    hide_failure.show_error = Some(RelativePointerPlatformError::FailedPlatformOperation);
    let mut mediator = RelativePointerPlatformMediator::new(hide_failure);
    let lock = mediator.acquire(window, CursorRestorePolicy::None).unwrap();
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let error = mediator.hide_cursor(lock, ids.mint().unwrap()).unwrap_err();

    assert_eq!(
        error.reason(),
        RelativePointerAdmissionError::CursorHideDenied
    );
    assert_eq!(
        error.show_result(),
        &Err(RelativePointerPlatformError::FailedPlatformOperation)
    );
    assert_eq!(error.release_result(), &Ok(()));
    assert_eq!(
        mediator.platform().operations(),
        vec![
            Operation::Support(window),
            Operation::Acquire(window),
            Operation::Hide(window),
            Operation::Show(window),
            Operation::Release(window),
        ]
    );
}

#[test]
fn restore_failure_cannot_skip_release_or_visibility_restoration() {
    let window = WindowId(44);
    let anchor = Point::new(12.0, 13.0);
    let mut platform = FakeRelativePointerPlatform::supported(true);
    platform.restore_error = Some(RelativePointerPlatformError::FailedPlatformOperation);
    platform.release_error = Some(RelativePointerPlatformError::FailedPlatformOperation);
    let mut mediator = RelativePointerPlatformMediator::new(platform);
    let lock = mediator
        .acquire(window, CursorRestorePolicy::DownAnchor)
        .unwrap();
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let hidden = mediator.hide_cursor(lock, ids.mint().unwrap()).unwrap();
    let outcome = mediator.terminate(hidden, Some(anchor));

    assert_eq!(
        outcome.restore(),
        CursorRestoreOutcome::FailedPlatformOperation
    );
    assert_eq!(
        outcome.release_result(),
        &Err(RelativePointerPlatformError::FailedPlatformOperation)
    );
    assert_eq!(outcome.show_result(), &Ok(()));
    assert_eq!(
        mediator.platform().operations(),
        vec![
            Operation::Support(window),
            Operation::Acquire(window),
            Operation::Hide(window),
            Operation::Restore(window, anchor),
            Operation::Release(window),
            Operation::Show(window),
        ]
    );
}

#[test]
fn unsafe_restore_context_skips_warp_but_still_releases_and_shows() {
    let window = WindowId(45);
    let mut mediator =
        RelativePointerPlatformMediator::new(FakeRelativePointerPlatform::supported(true));
    let lock = mediator
        .acquire(window, CursorRestorePolicy::DownAnchor)
        .unwrap();
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let hidden = mediator.hide_cursor(lock, ids.mint().unwrap()).unwrap();
    let outcome = mediator.terminate(hidden, None);

    assert_eq!(
        outcome.restore(),
        CursorRestoreOutcome::SkippedUnsafeContext
    );
    assert_eq!(
        mediator.platform().operations(),
        vec![
            Operation::Support(window),
            Operation::Acquire(window),
            Operation::Hide(window),
            Operation::Release(window),
            Operation::Show(window),
        ]
    );
}
