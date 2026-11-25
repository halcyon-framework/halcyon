// SPEC-HAL-128 Stage 7: Pointer Type & Gesture Legality Tests
//
// These tests verify the pointer-type to gesture-kind binding invariants:
//   INV-PT1 - PointerType is Session-Scoped
//   INV-PT2 - Legality is Static
//   INV-PT3 - Reject on Registration (no panics, no side effects)
//   INV-PT4 - Arbiter Sees Only Legal Candidates
//   INV-PT5 - Mouse Backward Compatibility (Custom always allowed)
//   INV-PT6 - No Defaults
//
// Stage 7 scope: pointer-type-aware gesture registration and arbitration.

use halcyon::core::event::PointerId;
use halcyon::core::event::{
    GestureArbiter, GestureCandidate, GestureCandidateId, GestureKind, GestureRegistry,
    GestureSessionId, PointerType,
};
use halcyon::utils::id::TreeWidgetId;

// ============================================================================
// Test: INV-PT3 - Illegal gesture rejected silently (Mouse cannot register Pinch)
// ============================================================================

/// INV-PT3: Illegal gesture candidates are rejected at registration time.
/// No panics. No side effects. Session remains valid.
#[test]
fn mouse_cannot_register_pinch() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);

    // Begin Mouse session
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    // Try to register Pinch (should fail silently - Pinch is Touch-only)
    let registered =
        registry.register_candidate_request(PointerId::MOUSE, widget_id, GestureKind::Pinch, None);

    // INV-PT3: Returns false, no panic
    assert!(
        !registered,
        "INV-PT3: Mouse should NOT be able to register Pinch gesture"
    );

    // INV-PT3: Session remains valid after rejection
    assert!(
        registry.has_session(PointerId::MOUSE),
        "INV-PT3: Session should remain valid after illegal registration attempt"
    );

    // INV-PT3: No side effects - no candidate registered
    let candidates = registry.candidates(PointerId::MOUSE);
    assert!(
        candidates.is_empty(),
        "INV-PT3: No candidates should exist after illegal registration attempt"
    );
}

// ============================================================================
// Test: INV-PT3 - Illegal gesture rejected silently (Touch cannot register Resize)
// ============================================================================

/// INV-PT3: Touch pointer type does NOT allow Resize gesture.
/// Resize is Mouse-only. Rejection is silent.
#[test]
fn touch_cannot_register_resize() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);
    let touch_pointer = PointerId(1);

    // Begin Touch session
    registry.begin_session(touch_pointer, PointerType::Touch);

    // Try to register Resize (should fail silently - Resize is Mouse-only)
    let registered =
        registry.register_candidate_request(touch_pointer, widget_id, GestureKind::Resize, None);

    // INV-PT3: Returns false, no panic
    assert!(
        !registered,
        "INV-PT3: Touch should NOT be able to register Resize gesture"
    );

    // INV-PT3: Session remains valid
    assert!(
        registry.has_session(touch_pointer),
        "INV-PT3: Session should remain valid after rejection"
    );

    // INV-PT3: No candidates registered
    let candidates = registry.candidates(touch_pointer);
    assert!(
        candidates.is_empty(),
        "INV-PT3: No candidates should exist after illegal registration attempt"
    );
}

// ============================================================================
// Test: INV-PT3 - Illegal gesture rejected silently (Pen cannot register Scroll)
// ============================================================================

/// INV-PT3: Pen pointer type does NOT allow Scroll gesture.
/// Scroll is Mouse/Touch-only. Rejection is silent.
#[test]
fn pen_cannot_register_scroll() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);
    let pen_pointer = PointerId(2);

    // Begin Pen session
    registry.begin_session(pen_pointer, PointerType::Pen);

    // Try to register Scroll (should fail silently - Scroll is Mouse/Touch, not Pen)
    let registered =
        registry.register_candidate_request(pen_pointer, widget_id, GestureKind::Scroll, None);

    // INV-PT3: Returns false, no panic
    assert!(
        !registered,
        "INV-PT3: Pen should NOT be able to register Scroll gesture"
    );

    // INV-PT3: Session remains valid
    assert!(
        registry.has_session(pen_pointer),
        "INV-PT3: Session should remain valid after rejection"
    );

    // INV-PT3: No candidates registered
    let candidates = registry.candidates(pen_pointer);
    assert!(
        candidates.is_empty(),
        "INV-PT3: No candidates should exist after illegal registration attempt"
    );
}

// ============================================================================
// Test: INV-PT3 - Session continues after rejection
// ============================================================================

/// INV-PT3: Session remains valid after illegal registration.
/// Multiple rejections do not corrupt session state.
#[test]
fn session_continues_after_multiple_rejections() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);
    let touch_pointer = PointerId(1);

    // Begin Touch session
    registry.begin_session(touch_pointer, PointerType::Touch);

    // Try multiple illegal gestures
    assert!(!registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Resize,
        None
    ));
    assert!(!registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Select,
        None
    ));
    assert!(!registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Draw,
        None
    ));

    // Session still valid
    assert!(
        registry.has_session(touch_pointer),
        "Session should remain valid after multiple rejections"
    );

    // Now register a legal gesture - should succeed
    let registered =
        registry.register_candidate_request(touch_pointer, widget_id, GestureKind::Scroll, None);
    assert!(
        registered,
        "Legal gesture should register after previous rejections"
    );

    let candidates = registry.candidates(touch_pointer);
    assert_eq!(
        candidates.len(),
        1,
        "Exactly one candidate should exist (the legal one)"
    );
}

// ============================================================================
// Test: INV-PT4 - Arbiter assumes all candidates are legal
// ============================================================================

/// INV-PT4: Arbiter sees only legal candidates.
/// Arbiter assumes all candidates are legal - no legality checks inside arbitration.
#[test]
fn arbiter_assumes_legal_candidates() {
    let arbiter = GestureArbiter::new();
    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    // Create only legal candidates for Mouse
    let scroll = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 0,
        },
        widget_a,
        GestureKind::Scroll,
    );

    let drag = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 1,
        },
        widget_b,
        GestureKind::Drag,
    );

    // Arbiter resolves without legality checks
    let candidates = vec![&scroll, &drag];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);

    assert!(winner.is_some());
    // Mouse primary is Drag
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Drag,
        "INV-PT4: Arbiter correctly resolves legal candidates"
    );
}

// ============================================================================
// Test: INV-PT5 - Mouse backward compatibility (Custom always allowed)
// ============================================================================

/// INV-PT5: Mouse behavior is unchanged. Custom always allowed.
#[test]
fn mouse_custom_gesture_always_allowed() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);

    // Begin Mouse session
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    // Register Custom gesture - should succeed (INV-PT5)
    let registered =
        registry.register_candidate_request(PointerId::MOUSE, widget_id, GestureKind::Custom, None);

    assert!(
        registered,
        "INV-PT5: Custom gesture should always be allowed for Mouse"
    );

    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].kind,
        GestureKind::Custom,
        "INV-PT5: Custom gesture registered for Mouse"
    );
}

/// INV-PT5: Verify Custom is NOT allowed for Touch (only Mouse has Custom).
#[test]
fn touch_custom_gesture_not_allowed() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);
    let touch_pointer = PointerId(1);

    // Begin Touch session
    registry.begin_session(touch_pointer, PointerType::Touch);

    // Try to register Custom - should fail (not in Touch's legal set)
    let registered =
        registry.register_candidate_request(touch_pointer, widget_id, GestureKind::Custom, None);

    assert!(
        !registered,
        "INV-PT5: Custom gesture should NOT be allowed for Touch"
    );
}

// ============================================================================
// Test: INV-PT2 - Legality is static (verified via is_gesture_legal)
// ============================================================================

/// INV-PT2: Legality is static - same inputs always produce same outputs.
#[test]
fn legality_is_static() {
    // Run legality checks multiple times - result must be identical
    for _ in 0..10 {
        // Mouse legality
        assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Drag));
        assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Scroll));
        assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Resize));
        assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Select));
        assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Custom));
        assert!(!PointerType::Mouse.is_gesture_legal(GestureKind::Pinch));
        assert!(!PointerType::Mouse.is_gesture_legal(GestureKind::Swipe));
        assert!(!PointerType::Mouse.is_gesture_legal(GestureKind::Draw));

        // Touch legality
        assert!(PointerType::Touch.is_gesture_legal(GestureKind::Scroll));
        assert!(PointerType::Touch.is_gesture_legal(GestureKind::Drag));
        assert!(PointerType::Touch.is_gesture_legal(GestureKind::Pinch));
        assert!(PointerType::Touch.is_gesture_legal(GestureKind::Swipe));
        assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Resize));
        assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Select));
        assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Custom));
        assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Draw));

        // Pen legality
        assert!(PointerType::Pen.is_gesture_legal(GestureKind::Draw));
        assert!(PointerType::Pen.is_gesture_legal(GestureKind::Drag));
        assert!(PointerType::Pen.is_gesture_legal(GestureKind::Select));
        assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Scroll));
        assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Resize));
        assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Pinch));
        assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Swipe));
        assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Custom));
    }
}

// ============================================================================
// Test: INV-PT1 - PointerType is session-scoped (immutable)
// ============================================================================

/// INV-PT1: Every gesture session has exactly one PointerType, assigned at begin_session.
#[test]
fn pointer_type_is_session_scoped() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);

    // Begin Mouse session
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    // Mouse-legal gestures work
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Drag,
        None
    ));
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Resize,
        None
    ));

    // Mouse-illegal gestures rejected
    assert!(!registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Pinch,
        None
    ));

    // Pointer type was set at begin_session and governs all registrations
    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(
        candidates.len(),
        2,
        "INV-PT1: Only gestures legal for session's PointerType were registered"
    );
}

// ============================================================================
// Test: Mouse primary gesture (Drag) wins
// ============================================================================

/// For Mouse, Drag is primary and wins over Scroll when competing.
#[test]
fn mouse_primary_gesture_drag_wins() {
    let arbiter = GestureArbiter::new();
    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    let scroll = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 0,
        },
        widget_a,
        GestureKind::Scroll,
    );

    let drag = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 1,
        },
        widget_b,
        GestureKind::Drag,
    );

    let candidates = vec![&scroll, &drag];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);

    assert!(winner.is_some());
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Drag,
        "Mouse primary gesture (Drag) should win over Scroll"
    );
}

// ============================================================================
// Test: Touch primary gesture (Scroll) wins
// ============================================================================

/// For Touch, Scroll is primary and wins over Drag when competing.
#[test]
fn touch_primary_gesture_scroll_wins() {
    let arbiter = GestureArbiter::new();
    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    let drag = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 0,
        },
        widget_a,
        GestureKind::Drag,
    );

    let scroll = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 1,
        },
        widget_b,
        GestureKind::Scroll,
    );

    let candidates = vec![&drag, &scroll];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Touch);

    assert!(winner.is_some());
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Scroll,
        "Touch primary gesture (Scroll) should win over Drag"
    );
}

// ============================================================================
// Test: Pen primary gesture (Draw) wins
// ============================================================================

/// For Pen, Draw is primary and wins when competing.
#[test]
fn pen_primary_gesture_draw_wins() {
    let arbiter = GestureArbiter::new();
    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);

    let drag = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 0,
        },
        widget_a,
        GestureKind::Drag,
    );

    let draw = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 1,
        },
        widget_b,
        GestureKind::Draw,
    );

    let candidates = vec![&drag, &draw];
    let winner = arbiter.resolve_priority(&candidates, PointerType::Pen);

    assert!(winner.is_some());
    assert_eq!(
        winner.unwrap().kind,
        GestureKind::Draw,
        "Pen primary gesture (Draw) should win over Drag"
    );
}

// ============================================================================
// Test: Deterministic arbitration
// ============================================================================

/// Same input produces identical commit across runs.
#[test]
fn same_input_produces_identical_result() {
    let widget_a = TreeWidgetId(1);
    let widget_b = TreeWidgetId(2);
    let widget_c = TreeWidgetId(3);

    let resize = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 0,
        },
        widget_a,
        GestureKind::Resize,
    );

    let drag = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 1,
        },
        widget_b,
        GestureKind::Drag,
    );

    let scroll = GestureCandidate::new_with_id(
        GestureCandidateId {
            session: GestureSessionId(1),
            local: 2,
        },
        widget_c,
        GestureKind::Scroll,
    );

    // Run arbitration 10 times with same input
    for _ in 0..10 {
        let arbiter = GestureArbiter::new();
        let candidates = vec![&scroll, &resize, &drag];
        let winner = arbiter.resolve_priority(&candidates, PointerType::Mouse);

        assert!(winner.is_some());
        assert_eq!(
            winner.unwrap().kind,
            GestureKind::Drag,
            "Same input must produce same result (Drag wins for Mouse)"
        );
    }
}

// ============================================================================
// Test: Legal gestures ARE registered
// ============================================================================

/// Legal gestures for the pointer type ARE registered successfully.
#[test]
fn legal_gestures_are_registered() {
    let mut registry = GestureRegistry::new();
    let widget_id = TreeWidgetId(1);

    // Mouse session - Drag, Scroll, Resize, Select, Custom are legal
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Drag,
        None
    ));
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Scroll,
        None
    ));
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Resize,
        None
    ));
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Select,
        None
    ));
    assert!(registry.register_candidate_request(
        PointerId::MOUSE,
        widget_id,
        GestureKind::Custom,
        None
    ));

    let candidates = registry.candidates(PointerId::MOUSE);
    assert_eq!(
        candidates.len(),
        5,
        "All 5 legal Mouse gestures should be registered"
    );

    registry.clear_candidates(PointerId::MOUSE);

    // Touch session - Scroll, Drag, Pinch, Swipe are legal
    let touch_pointer = PointerId(1);
    registry.begin_session(touch_pointer, PointerType::Touch);

    assert!(registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Scroll,
        None
    ));
    assert!(registry.register_candidate_request(touch_pointer, widget_id, GestureKind::Drag, None));
    assert!(registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Pinch,
        None
    ));
    assert!(registry.register_candidate_request(
        touch_pointer,
        widget_id,
        GestureKind::Swipe,
        None
    ));

    let touch_candidates = registry.candidates(touch_pointer);
    assert_eq!(
        touch_candidates.len(),
        4,
        "All 4 legal Touch gestures should be registered"
    );

    registry.clear_candidates(touch_pointer);

    // Pen session - Draw, Drag, Select are legal
    let pen_pointer = PointerId(2);
    registry.begin_session(pen_pointer, PointerType::Pen);

    assert!(registry.register_candidate_request(pen_pointer, widget_id, GestureKind::Draw, None));
    assert!(registry.register_candidate_request(pen_pointer, widget_id, GestureKind::Drag, None));
    assert!(registry.register_candidate_request(pen_pointer, widget_id, GestureKind::Select, None));

    let pen_candidates = registry.candidates(pen_pointer);
    assert_eq!(
        pen_candidates.len(),
        3,
        "All 3 legal Pen gestures should be registered"
    );
}

// ============================================================================
// Test: Pointer isolation (different primaries per pointer type)
// ============================================================================

/// Gesture semantics are applied per pointer, never globally.
#[test]
fn pointer_isolation_different_primaries() {
    // Verify each pointer type has its own primary
    assert_eq!(
        PointerType::Mouse.primary_gesture(),
        GestureKind::Drag,
        "Mouse primary should be Drag"
    );
    assert_eq!(
        PointerType::Touch.primary_gesture(),
        GestureKind::Scroll,
        "Touch primary should be Scroll"
    );
    assert_eq!(
        PointerType::Pen.primary_gesture(),
        GestureKind::Draw,
        "Pen primary should be Draw"
    );

    // Verify is_gesture_legal differs between pointer types
    assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Resize));
    assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Resize));
    assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Resize));

    assert!(!PointerType::Mouse.is_gesture_legal(GestureKind::Pinch));
    assert!(PointerType::Touch.is_gesture_legal(GestureKind::Pinch));
    assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Pinch));

    assert!(!PointerType::Mouse.is_gesture_legal(GestureKind::Draw));
    assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Draw));
    assert!(PointerType::Pen.is_gesture_legal(GestureKind::Draw));

    // INV-PT5: Custom only for Mouse
    assert!(PointerType::Mouse.is_gesture_legal(GestureKind::Custom));
    assert!(!PointerType::Touch.is_gesture_legal(GestureKind::Custom));
    assert!(!PointerType::Pen.is_gesture_legal(GestureKind::Custom));
}
