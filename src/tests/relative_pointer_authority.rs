use std::cell::RefCell;
use std::rc::Rc;

use crate::core::event::{
    CaptureMode, GestureCandidate, GestureCandidateId, GestureKind, GestureSessionId,
    InteractionDomainId, PointerButton, PointerId, WindowId,
};
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestoreOutcome, CursorRestorePolicy, PointerMotionPolicy, RelativePointerAdmissionError,
    RelativePointerLeaseIdAllocator, RelativePointerNormalizationEpoch,
};
use crate::core::relative_pointer_authority::{
    RelativePointerAdmissionFacts, RelativePointerHardCaptureFacts, RelativePointerLeaseAuthority,
};
use crate::platform::relative_pointer::{
    RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformSupport,
};
use crate::utils::id::TreeWidgetId;

const WINDOW: WindowId = WindowId(17);
const DOMAIN: InteractionDomainId = InteractionDomainId(23);
const POINTER: PointerId = PointerId(31);
const SESSION: GestureSessionId = GestureSessionId(41);
const OWNER: TreeWidgetId = TreeWidgetId(47);
const ANCHOR: Point = Point { x: 53.0, y: 59.0 };

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlatformOperation {
    Support(WindowId),
    Acquire(WindowId),
    Hide(WindowId),
    Restore(WindowId, Point),
    Release(WindowId),
    Show(WindowId),
}

struct FakePlatform {
    operations: Rc<RefCell<Vec<PlatformOperation>>>,
    hide_error: Option<RelativePointerPlatformError>,
}

impl FakePlatform {
    fn supported() -> (Self, Rc<RefCell<Vec<PlatformOperation>>>) {
        let operations = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                operations: operations.clone(),
                hide_error: None,
            },
            operations,
        )
    }

    fn denying_hide() -> (Self, Rc<RefCell<Vec<PlatformOperation>>>) {
        let (mut platform, operations) = Self::supported();
        platform.hide_error = Some(RelativePointerPlatformError::Denied);
        (platform, operations)
    }
}

impl RelativePointerPlatform for FakePlatform {
    fn support(&self, window_id: WindowId) -> RelativePointerPlatformSupport {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Support(window_id));
        RelativePointerPlatformSupport::Supported {
            normalization_epoch: RelativePointerNormalizationEpoch::new(67).unwrap(),
            restore_down_anchor: true,
        }
    }

    fn acquire(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Acquire(window_id));
        Ok(())
    }

    fn release(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Release(window_id));
        Ok(())
    }

    fn hide_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Hide(window_id));
        self.hide_error.map_or(Ok(()), Err)
    }

    fn show_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Show(window_id));
        Ok(())
    }

    fn restore_cursor(
        &mut self,
        window_id: WindowId,
        anchor: Point,
    ) -> Result<(), RelativePointerPlatformError> {
        self.operations
            .borrow_mut()
            .push(PlatformOperation::Restore(window_id, anchor));
        Ok(())
    }
}

fn relative_candidate() -> GestureCandidate {
    let mut candidate = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: SESSION,
            local: 0,
        },
        OWNER,
        GestureKind::Drag,
    );
    candidate.motion_policy = PointerMotionPolicy::RelativeLocked {
        restore: CursorRestorePolicy::DownAnchor,
    };
    candidate
}

fn valid_facts() -> RelativePointerAdmissionFacts {
    let candidate = relative_candidate();
    RelativePointerAdmissionFacts {
        window_id: WINDOW,
        interaction_domain_id: DOMAIN,
        pointer_id: POINTER,
        gesture_session_id: SESSION,
        candidate,
        committed_candidate_id: Some(candidate.candidate_id),
        pointer_button: PointerButton::Primary,
        down_anchor: Some(ANCHOR),
        focused_window_id: Some(WINDOW),
        owner_available: true,
        hard_capture: Some(RelativePointerHardCaptureFacts {
            pointer_id: POINTER,
            owner: OWNER,
            button: PointerButton::Primary,
            mode: CaptureMode::Hard,
        }),
    }
}

#[test]
fn no_admission_call_performs_no_platform_work() {
    let (platform, operations) = FakePlatform::supported();
    let authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);

    assert!(operations.borrow().is_empty());
    drop(authority);
    assert!(operations.borrow().is_empty());
}

#[test]
fn admission_preconditions_reject_before_any_platform_call() {
    let (platform, operations) = FakePlatform::supported();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);
    let valid = valid_facts();

    let mut not_committed = valid;
    not_committed.committed_candidate_id = None;
    let mut wrong_commit = valid;
    wrong_commit.committed_candidate_id = Some(GestureCandidateId {
        session: SESSION,
        local: 1,
    });
    let mut wrong_session = valid;
    wrong_session.gesture_session_id = GestureSessionId(SESSION.0 + 1);
    let mut wrong_scope = valid;
    wrong_scope.interaction_domain_id = InteractionDomainId(DOMAIN.0 + 1);
    let mut absolute = valid;
    absolute.candidate.motion_policy = PointerMotionPolicy::Absolute;
    absolute.down_anchor = None;
    let mut no_capture = valid;
    no_capture.hard_capture = None;
    let mut wrong_capture_pointer = valid;
    wrong_capture_pointer
        .hard_capture
        .as_mut()
        .unwrap()
        .pointer_id = PointerId(POINTER.0 + 1);
    let mut wrong_capture_owner = valid;
    wrong_capture_owner.hard_capture.as_mut().unwrap().owner = TreeWidgetId(OWNER.0 + 1);
    let mut wrong_capture_button = valid;
    wrong_capture_button.hard_capture.as_mut().unwrap().button = PointerButton::Secondary;
    let mut soft_capture = valid;
    soft_capture.hard_capture.as_mut().unwrap().mode = CaptureMode::Soft;
    let mut owner_missing = valid;
    owner_missing.owner_available = false;
    let mut unfocused = valid;
    unfocused.focused_window_id = None;
    let mut missing_anchor = valid;
    missing_anchor.down_anchor = None;

    for (facts, expected) in [
        (
            not_committed,
            RelativePointerAdmissionError::GestureNotCommitted,
        ),
        (
            wrong_commit,
            RelativePointerAdmissionError::GestureNotCommitted,
        ),
        (wrong_session, RelativePointerAdmissionError::StaleBinding),
        (
            wrong_scope,
            RelativePointerAdmissionError::WindowDomainMismatch,
        ),
        (absolute, RelativePointerAdmissionError::StaleBinding),
        (
            no_capture,
            RelativePointerAdmissionError::HardCaptureNotOwned,
        ),
        (
            wrong_capture_pointer,
            RelativePointerAdmissionError::HardCaptureNotOwned,
        ),
        (
            wrong_capture_owner,
            RelativePointerAdmissionError::HardCaptureNotOwned,
        ),
        (
            wrong_capture_button,
            RelativePointerAdmissionError::HardCaptureNotOwned,
        ),
        (
            soft_capture,
            RelativePointerAdmissionError::HardCaptureNotOwned,
        ),
        (
            owner_missing,
            RelativePointerAdmissionError::OwnerUnavailable,
        ),
        (unfocused, RelativePointerAdmissionError::WindowNotFocused),
        (missing_anchor, RelativePointerAdmissionError::StaleBinding),
    ] {
        assert_eq!(authority.admit(facts), Err(expected));
        assert!(operations.borrow().is_empty());
    }
}

#[test]
fn successful_admission_locks_then_hides_and_publishes_binding_and_gate() {
    let (platform, operations) = FakePlatform::supported();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);

    let activation = authority.admit(valid_facts()).unwrap();
    let binding = activation.binding();
    let gate = activation.ingress_gate();

    assert_eq!(binding.window_id(), WINDOW);
    assert_eq!(binding.interaction_domain_id(), DOMAIN);
    assert_eq!(binding.pointer_id(), POINTER);
    assert_eq!(binding.gesture_session_id(), SESSION);
    assert_eq!(
        binding.winning_candidate_id(),
        relative_candidate().candidate_id
    );
    assert_eq!(binding.capture_owner(), OWNER);
    assert_eq!(binding.pointer_button(), PointerButton::Primary);
    assert_eq!(binding.down_anchor(), Some(ANCHOR));
    assert_eq!(gate.lease_id(), binding.lease_id());
    assert_eq!(gate.window_id(), WINDOW);
    assert_eq!(gate.pointer_id(), POINTER);
    assert_eq!(authority.active(), Some(activation));
    assert_eq!(
        operations.borrow().as_slice(),
        &[
            PlatformOperation::Support(WINDOW),
            PlatformOperation::Acquire(WINDOW),
            PlatformOperation::Hide(WINDOW),
        ]
    );
}

#[test]
fn hide_denial_rolls_back_platform_state_and_publishes_no_lease() {
    let (platform, operations) = FakePlatform::denying_hide();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);

    assert_eq!(
        authority.admit(valid_facts()),
        Err(RelativePointerAdmissionError::CursorHideDenied)
    );
    assert!(authority.active().is_none());
    assert_eq!(
        operations.borrow().as_slice(),
        &[
            PlatformOperation::Support(WINDOW),
            PlatformOperation::Acquire(WINDOW),
            PlatformOperation::Hide(WINDOW),
            PlatformOperation::Show(WINDOW),
            PlatformOperation::Release(WINDOW),
        ]
    );
}

#[test]
fn active_lease_rejects_conflict_without_additional_platform_work() {
    let (platform, operations) = FakePlatform::supported();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);
    let activation = authority.admit(valid_facts()).unwrap();
    let operation_count = operations.borrow().len();

    assert_eq!(
        authority.admit(valid_facts()),
        Err(RelativePointerAdmissionError::ConflictingLease)
    );
    assert_eq!(operations.borrow().len(), operation_count);
    assert_eq!(authority.active(), Some(activation));
}

#[test]
fn stale_motion_identity_is_ignored_without_disturbing_active_lease() {
    let (platform, operations) = FakePlatform::supported();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);
    let activation = authority.admit(valid_facts()).unwrap();
    let binding = activation.binding();
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let _ = ids.mint().unwrap();
    let stale_lease_id = ids.mint().unwrap();
    let operation_count = operations.borrow().len();

    assert!(
        authority
            .binding_for_motion(stale_lease_id, WINDOW, DOMAIN, POINTER)
            .is_none()
    );
    assert!(
        authority
            .binding_for_motion(binding.lease_id(), WindowId(WINDOW.0 + 1), DOMAIN, POINTER,)
            .is_none()
    );
    assert_eq!(
        authority.binding_for_motion(binding.lease_id(), WINDOW, DOMAIN, POINTER),
        Some(binding)
    );
    assert_eq!(authority.active(), Some(activation));
    assert_eq!(operations.borrow().len(), operation_count);
}

#[test]
fn termination_restores_only_in_safe_context_and_is_idempotent() {
    let (platform, operations) = FakePlatform::supported();
    let mut authority = RelativePointerLeaseAuthority::new(WINDOW, DOMAIN, platform);

    let first = authority.admit(valid_facts()).unwrap();
    let (_, safe_cleanup) = authority
        .terminate(first.binding().lease_id(), true)
        .unwrap();
    assert_eq!(safe_cleanup.restore(), CursorRestoreOutcome::Restored);
    let operation_count = operations.borrow().len();
    assert!(
        authority
            .terminate(first.binding().lease_id(), true)
            .is_none()
    );
    assert_eq!(operations.borrow().len(), operation_count);

    let second = authority.admit(valid_facts()).unwrap();
    let (_, unsafe_cleanup) = authority
        .terminate(second.binding().lease_id(), false)
        .unwrap();
    assert_eq!(
        unsafe_cleanup.restore(),
        CursorRestoreOutcome::SkippedUnsafeContext
    );
    assert!(authority.active().is_none());
    assert_eq!(
        operations.borrow().as_slice(),
        &[
            PlatformOperation::Support(WINDOW),
            PlatformOperation::Acquire(WINDOW),
            PlatformOperation::Hide(WINDOW),
            PlatformOperation::Restore(WINDOW, ANCHOR),
            PlatformOperation::Release(WINDOW),
            PlatformOperation::Show(WINDOW),
            PlatformOperation::Support(WINDOW),
            PlatformOperation::Acquire(WINDOW),
            PlatformOperation::Hide(WINDOW),
            PlatformOperation::Release(WINDOW),
            PlatformOperation::Show(WINDOW),
        ]
    );
}
