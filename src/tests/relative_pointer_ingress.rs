use std::time::{Duration, Instant};

use crate::core::event::{
    FusedInputEvent, GestureCandidateId, GestureSessionId, InteractionDomainId, KeyCode, KeyState,
    Modifiers, PointerButton, PointerDevice, PointerId, RawButtonState, RawInputEvent, WindowId,
};
use crate::core::input_fusion::InputFusion;
use crate::core::math::Point;
use crate::core::relative_pointer::{
    CursorRestorePolicy, RelativePointerIngressGate, RelativePointerLeaseBinding,
    RelativePointerLeaseId, RelativePointerLeaseIdAllocator, RelativePointerNormalizationEpoch,
};
use crate::utils::id::TreeWidgetId;

fn ingress_gate(
    lease_id: RelativePointerLeaseId,
    window_id: WindowId,
    pointer_id: PointerId,
    normalization_epoch: RelativePointerNormalizationEpoch,
) -> RelativePointerIngressGate {
    let session = GestureSessionId(31);
    let binding = RelativePointerLeaseBinding::try_new(
        lease_id,
        window_id,
        InteractionDomainId(17),
        pointer_id,
        session,
        GestureCandidateId { session, local: 4 },
        TreeWidgetId(23),
        PointerButton::Primary,
        CursorRestorePolicy::None,
        None,
        normalization_epoch,
    )
    .unwrap();
    RelativePointerIngressGate::from_binding(&binding)
}

fn raw_relative(
    gate: RelativePointerIngressGate,
    time: Instant,
    delta: (f32, f32),
) -> RawInputEvent {
    RawInputEvent::RelativePointerMotion {
        lease_id: gate.lease_id(),
        normalization_epoch: gate.normalization_epoch(),
        window_id: gate.window_id(),
        pointer_id: gate.pointer_id(),
        time,
        delta,
    }
}

fn assert_relative(
    event: &FusedInputEvent,
    expected_gate: RelativePointerIngressGate,
    expected_time: Instant,
    expected_delta: (f32, f32),
) {
    let FusedInputEvent::RelativePointerMotion {
        lease_id,
        window_id,
        pointer_id,
        time,
        delta,
    } = event
    else {
        panic!("expected relative pointer motion, got {event:?}");
    };
    assert_eq!(*lease_id, expected_gate.lease_id());
    assert_eq!(*window_id, expected_gate.window_id());
    assert_eq!(*pointer_id, expected_gate.pointer_id());
    assert_eq!(*time, expected_time);
    assert_eq!(delta.components(), expected_delta);
}

#[test]
fn inactive_relative_motion_has_zero_output_and_does_not_allocate_output_storage() {
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let gate = ingress_gate(
        ids.mint().unwrap(),
        WindowId(9),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(3).unwrap(),
    );
    let mut fusion = InputFusion::new();
    let mut out = Vec::new();

    fusion.process_raw_with_relative_ingress(
        raw_relative(gate, Instant::now(), (1.0, 2.0)),
        None,
        &mut out,
    );
    fusion.flush(&mut out);

    assert!(out.is_empty());
    assert_eq!(out.capacity(), 0);
}

#[test]
fn relative_ingress_rejects_stale_identity_epoch_and_non_finite_motion_without_mutation() {
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let active = ingress_gate(
        ids.mint().unwrap(),
        WindowId(10),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(5).unwrap(),
    );
    let stale = ingress_gate(
        ids.mint().unwrap(),
        WindowId(10),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(5).unwrap(),
    );
    let time = Instant::now();
    let mut fusion = InputFusion::new();
    let mut out = Vec::new();

    fusion.process_raw_with_relative_ingress(
        raw_relative(active, time, (1.0, 2.0)),
        Some(&active),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        raw_relative(stale, time + Duration::from_millis(1), (50.0, 60.0)),
        Some(&active),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        RawInputEvent::RelativePointerMotion {
            lease_id: active.lease_id(),
            normalization_epoch: RelativePointerNormalizationEpoch::new(6).unwrap(),
            window_id: active.window_id(),
            pointer_id: active.pointer_id(),
            time: time + Duration::from_millis(2),
            delta: (70.0, 80.0),
        },
        Some(&active),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        raw_relative(
            active,
            time + Duration::from_millis(3),
            (f32::NAN, f32::INFINITY),
        ),
        Some(&active),
        &mut out,
    );

    assert!(out.is_empty());
    fusion.flush(&mut out);
    assert_eq!(out.len(), 1);
    assert_relative(&out[0], active, time, (1.0, 2.0));
}

#[test]
fn only_consecutive_motion_for_the_same_lease_and_epoch_coalesces() {
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let first = ingress_gate(
        ids.mint().unwrap(),
        WindowId(11),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(7).unwrap(),
    );
    let second = ingress_gate(
        ids.mint().unwrap(),
        WindowId(11),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(8).unwrap(),
    );
    let time = Instant::now();
    let mut fusion = InputFusion::new();
    let mut out = Vec::new();

    fusion.process_raw_with_relative_ingress(
        raw_relative(first, time, (1.5, -2.0)),
        Some(&first),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        raw_relative(first, time + Duration::from_millis(1), (2.5, 5.0)),
        Some(&first),
        &mut out,
    );
    assert!(out.is_empty());

    fusion.flush(&mut out);
    assert_eq!(out.len(), 1);
    assert_relative(&out[0], first, time + Duration::from_millis(1), (4.0, 3.0));

    out.clear();
    fusion.process_raw_with_relative_ingress(
        raw_relative(first, time + Duration::from_millis(2), (7.0, 8.0)),
        Some(&first),
        &mut out,
    );
    fusion.process_raw_with_relative_ingress(
        raw_relative(second, time + Duration::from_millis(3), (9.0, 10.0)),
        Some(&second),
        &mut out,
    );
    assert!(out.is_empty());
    fusion.flush(&mut out);
    assert_eq!(out.len(), 1);
    assert_relative(
        &out[0],
        second,
        time + Duration::from_millis(3),
        (9.0, 10.0),
    );
}

#[test]
fn semantic_events_are_ordering_barriers_for_relative_motion() {
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let gate = ingress_gate(
        ids.mint().unwrap(),
        WindowId(12),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(9).unwrap(),
    );
    let time = Instant::now();
    let barriers = [
        RawInputEvent::PointerButton {
            window_id: gate.window_id(),
            time: time + Duration::from_millis(1),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: RawButtonState::Pressed,
            position: Point::new(4.0, 5.0),
        },
        RawInputEvent::PointerButton {
            window_id: gate.window_id(),
            time: time + Duration::from_millis(1),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state: RawButtonState::Released,
            position: Point::new(4.0, 5.0),
        },
        RawInputEvent::Resize {
            window_id: gate.window_id(),
            width: 640.0,
            height: 480.0,
        },
        RawInputEvent::Key {
            window_id: gate.window_id(),
            time: time + Duration::from_millis(1),
            key_code: KeyCode::Escape,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            state: KeyState::Down,
            repeat: false,
            modifiers: Modifiers::default(),
        },
    ];

    for barrier in barriers {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        fusion.process_raw_with_relative_ingress(
            raw_relative(gate, time, (2.0, 3.0)),
            Some(&gate),
            &mut out,
        );
        fusion.process_raw_with_relative_ingress(barrier, Some(&gate), &mut out);

        assert!(!out.is_empty());
        assert_relative(&out[0], gate, time, (2.0, 3.0));
    }

    for cancellation in [
        RawInputEvent::FocusChanged {
            window_id: gate.window_id(),
            time: time + Duration::from_millis(1),
            focused: false,
        },
        RawInputEvent::ScaleFactorChanged {
            window_id: gate.window_id(),
            scale_factor: 2.0,
        },
    ] {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        fusion.process_raw_with_relative_ingress(
            raw_relative(gate, time, (2.0, 3.0)),
            Some(&gate),
            &mut out,
        );
        fusion.process_raw_with_relative_ingress(cancellation, Some(&gate), &mut out);
        fusion.flush(&mut out);
        assert!(
            out.iter()
                .all(|event| !matches!(event, FusedInputEvent::RelativePointerMotion { .. }))
        );
    }
}

#[test]
fn absolute_motion_is_suppressed_only_for_the_matching_active_ingress() {
    let mut ids = RelativePointerLeaseIdAllocator::new();
    let gate = ingress_gate(
        ids.mint().unwrap(),
        WindowId(13),
        PointerId::MOUSE,
        RelativePointerNormalizationEpoch::new(10).unwrap(),
    );
    let time = Instant::now();
    let absolute = |window_id| RawInputEvent::PointerMoved {
        window_id,
        time,
        device: PointerDevice::Mouse,
        position: Point::new(20.0, 30.0),
        delta: (4.0, 5.0),
    };

    let mut fusion = InputFusion::new();
    let mut out = Vec::new();
    fusion.process_raw_with_relative_ingress(absolute(gate.window_id()), Some(&gate), &mut out);
    fusion.flush(&mut out);
    assert!(out.is_empty());
    assert_eq!(fusion.pointer_position(), Point::zero());

    fusion.process_raw_with_relative_ingress(absolute(WindowId(14)), Some(&gate), &mut out);
    fusion.flush(&mut out);
    assert!(
        matches!(out.as_slice(), [FusedInputEvent::PointerMove { window_id, .. }] if *window_id == WindowId(14))
    );

    let mut fusion = InputFusion::new();
    let mut out = Vec::new();
    fusion.process_raw(absolute(gate.window_id()), &mut out);
    fusion.flush(&mut out);
    assert!(
        matches!(out.as_slice(), [FusedInputEvent::PointerMove { window_id, .. }] if *window_id == gate.window_id())
    );
}
