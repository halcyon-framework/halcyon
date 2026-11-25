use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use crate::core::event::{
    GestureCandidateId, GestureSessionId, InteractionDomainId, PointerButton, PointerId,
    RawInputEvent, WindowId,
};
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestorePolicy, RelativePointerIngressGate, RelativePointerLeaseBinding,
    RelativePointerLeaseIdAllocator, RelativePointerNormalizationEpoch,
};
use crate::platform::relative_pointer::{
    RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformSupport,
};
use crate::platform::relative_pointer_macos::{
    MacOsRelativePointerPlatform, MacOsRelativePointerWindowOps, relative_motion_from_device_event,
};
use crate::utils::id::TreeWidgetId;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Call {
    Lock(bool),
    Visible(bool),
    Restore(Point),
}

#[derive(Clone, Default)]
struct FakeWindow {
    calls: Rc<RefCell<Vec<Call>>>,
    deny_lock: Rc<Cell<bool>>,
    fail_restore: Rc<Cell<bool>>,
}

impl MacOsRelativePointerWindowOps for FakeWindow {
    fn set_locked(&self, locked: bool) -> Result<(), RelativePointerPlatformError> {
        self.calls.borrow_mut().push(Call::Lock(locked));
        if locked && self.deny_lock.get() {
            Err(RelativePointerPlatformError::Denied)
        } else {
            Ok(())
        }
    }

    fn set_visible(&self, visible: bool) {
        self.calls.borrow_mut().push(Call::Visible(visible));
    }

    fn restore(&self, anchor: Point) -> Result<(), RelativePointerPlatformError> {
        self.calls.borrow_mut().push(Call::Restore(anchor));
        if self.fail_restore.get() {
            Err(RelativePointerPlatformError::FailedPlatformOperation)
        } else {
            Ok(())
        }
    }
}

#[test]
fn native_adapter_reports_pinned_point_normalization_and_down_anchor() {
    let window = FakeWindow::default();
    let platform = MacOsRelativePointerPlatform::new(WindowId::PRIMARY, window.clone());

    assert_eq!(
        platform.support(WindowId::PRIMARY),
        RelativePointerPlatformSupport::Supported {
            normalization_epoch: RelativePointerNormalizationEpoch::new(1).unwrap(),
            restore_down_anchor: true,
        }
    );
    assert_eq!(
        platform.support(WindowId(8)),
        RelativePointerPlatformSupport::Unsupported
    );
    assert!(window.calls.borrow().is_empty());
}

#[test]
fn native_adapter_restores_while_hidden_then_releases_and_shows_once() {
    let window = FakeWindow::default();
    let mut platform = MacOsRelativePointerPlatform::new(WindowId::PRIMARY, window.clone());
    let anchor = Point::new(20.0, 30.0);

    platform.acquire(WindowId::PRIMARY).unwrap();
    platform.hide_cursor(WindowId::PRIMARY).unwrap();
    platform.restore_cursor(WindowId::PRIMARY, anchor).unwrap();
    platform.release(WindowId::PRIMARY).unwrap();
    platform.show_cursor(WindowId::PRIMARY).unwrap();
    platform.release(WindowId::PRIMARY).unwrap();
    platform.show_cursor(WindowId::PRIMARY).unwrap();

    assert_eq!(
        *window.calls.borrow(),
        vec![
            Call::Lock(true),
            Call::Visible(false),
            Call::Restore(anchor),
            Call::Lock(false),
            Call::Visible(true),
        ]
    );
}

#[test]
fn native_adapter_rejects_denied_lock_and_cleans_up_after_restore_failure() {
    let window = FakeWindow::default();
    let mut platform = MacOsRelativePointerPlatform::new(WindowId::PRIMARY, window.clone());
    assert_eq!(
        platform.acquire(WindowId(8)),
        Err(RelativePointerPlatformError::Unsupported)
    );
    assert!(window.calls.borrow().is_empty());

    window.deny_lock.set(true);
    assert_eq!(
        platform.acquire(WindowId::PRIMARY),
        Err(RelativePointerPlatformError::Denied)
    );
    assert_eq!(
        platform.hide_cursor(WindowId::PRIMARY),
        Err(RelativePointerPlatformError::Denied)
    );

    window.deny_lock.set(false);
    window.fail_restore.set(true);
    platform.acquire(WindowId::PRIMARY).unwrap();
    platform.hide_cursor(WindowId::PRIMARY).unwrap();
    assert_eq!(
        platform.restore_cursor(WindowId::PRIMARY, Point::new(20.0, 30.0)),
        Err(RelativePointerPlatformError::FailedPlatformOperation)
    );
    platform.release(WindowId::PRIMARY).unwrap();
    platform.show_cursor(WindowId::PRIMARY).unwrap();
    assert_eq!(
        *window.calls.borrow(),
        vec![
            Call::Lock(true),
            Call::Lock(true),
            Call::Visible(false),
            Call::Restore(Point::new(20.0, 30.0)),
            Call::Lock(false),
            Call::Visible(true),
        ]
    );
}

fn gate() -> RelativePointerIngressGate {
    let session = GestureSessionId(31);
    let binding = RelativePointerLeaseBinding::try_new(
        RelativePointerLeaseIdAllocator::new().mint().unwrap(),
        WindowId::PRIMARY,
        InteractionDomainId::PRIMARY,
        PointerId::MOUSE,
        session,
        GestureCandidateId { session, local: 4 },
        TreeWidgetId(23),
        PointerButton::Primary,
        CursorRestorePolicy::None,
        None,
        RelativePointerNormalizationEpoch::new(7).unwrap(),
    )
    .unwrap();
    RelativePointerIngressGate::from_binding(&binding)
}

#[test]
fn native_device_motion_is_identity_normalized_and_lease_bound() {
    let gate = gate();
    let time = Instant::now();
    let raw = relative_motion_from_device_event(
        Some(gate),
        Some(WindowId::PRIMARY),
        WindowId::PRIMARY,
        (2.25, -3.5),
        time,
    )
    .unwrap();
    assert!(matches!(
        raw,
        RawInputEvent::RelativePointerMotion {
            lease_id,
            normalization_epoch,
            window_id,
            pointer_id,
            time: event_time,
            delta: (2.25, -3.5),
        } if lease_id == gate.lease_id()
            && normalization_epoch == gate.normalization_epoch()
            && window_id == gate.window_id()
            && pointer_id == gate.pointer_id()
            && event_time == time
    ));
    assert!(
        relative_motion_from_device_event(
            None,
            Some(WindowId::PRIMARY),
            WindowId::PRIMARY,
            (1.0, 2.0),
            time
        )
        .is_none()
    );
    assert!(
        relative_motion_from_device_event(Some(gate), None, WindowId::PRIMARY, (1.0, 2.0), time)
            .is_none()
    );
    assert!(
        relative_motion_from_device_event(
            Some(gate),
            Some(WindowId(8)),
            WindowId::PRIMARY,
            (1.0, 2.0),
            time
        )
        .is_none()
    );
    assert!(
        relative_motion_from_device_event(
            Some(gate),
            Some(WindowId::PRIMARY),
            WindowId(8),
            (1.0, 2.0),
            time
        )
        .is_none()
    );
    assert!(
        relative_motion_from_device_event(
            Some(gate),
            Some(WindowId::PRIMARY),
            WindowId::PRIMARY,
            (f64::INFINITY, 2.0),
            time
        )
        .is_none()
    );
    assert!(
        relative_motion_from_device_event(
            Some(gate),
            Some(WindowId::PRIMARY),
            WindowId::PRIMARY,
            (f64::MAX, 2.0),
            time
        )
        .is_none()
    );
}
