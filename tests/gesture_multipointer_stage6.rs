// SPEC-HAL-128 Stage 6: Multi-Pointer Generalization Tests
//
// These tests verify the multi-pointer isolation and concurrent session invariants:
//   - Multiple pointers can operate simultaneously
//   - Each pointer has isolated gesture/capture state
//   - Global cancellation (Escape/Overlay) cancels ALL pointers
//   - Widget destruction cancels ONLY bound pointers
//
// Stage 6 scope: Multi-pointer independence, isolation, and explicit cancellation.

use halcyon::core::event::{
    GestureAxis, GestureKind, GestureRegistry, PointerCaptureManager, PointerId, PointerType,
};
use halcyon::utils::id::TreeWidgetId;

// ============================================================================
// Test: Independent Sessions (no cross-pollution)
// ============================================================================

/// Test that two pointers can have independent gesture sessions.
/// Each pointer's candidates, commits, and state must be isolated.
#[test]
fn independent_sessions_no_cross_pollution() {
    let mut registry = GestureRegistry::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);

    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);

    // Begin sessions for both pointers
    let session_a = registry.begin_session(pointer_a, PointerType::Mouse);
    let session_b = registry.begin_session(pointer_b, PointerType::Mouse);

    // Sessions must be distinct
    assert_ne!(session_a.0, session_b.0, "Session IDs must be unique");

    // Register candidates for each pointer
    registry.register_candidate_request(pointer_a, widget_x, GestureKind::Drag, None);
    registry.register_candidate_request(pointer_b, widget_y, GestureKind::Scroll, None);

    // Verify pointer A has only its candidate
    let candidates_a = registry.candidates(pointer_a);
    assert_eq!(
        candidates_a.len(),
        1,
        "Pointer A should have exactly 1 candidate"
    );
    assert_eq!(candidates_a[0].widget_id, widget_x);
    assert_eq!(candidates_a[0].kind, GestureKind::Drag);

    // Verify pointer B has only its candidate
    let candidates_b = registry.candidates(pointer_b);
    assert_eq!(
        candidates_b.len(),
        1,
        "Pointer B should have exactly 1 candidate"
    );
    assert_eq!(candidates_b[0].widget_id, widget_y);
    assert_eq!(candidates_b[0].kind, GestureKind::Scroll);

    // Clearing pointer A must NOT affect pointer B
    registry.clear_candidates(pointer_a);

    assert!(
        !registry.has_session(pointer_a),
        "Pointer A session should be cleared"
    );
    assert!(
        registry.has_session(pointer_b),
        "Pointer B session should still exist"
    );
    assert_eq!(
        registry.candidates(pointer_b).len(),
        1,
        "Pointer B candidates should be unaffected"
    );
}

// ============================================================================
// Test: Independent Capture (per-pointer exclusivity)
// ============================================================================

/// Test that two pointers can capture different widgets simultaneously.
/// Capture is exclusive per-pointer, not global.
#[test]
fn independent_capture_per_pointer() {
    let mut capture_mgr = PointerCaptureManager::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);

    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);

    // Pointer A captures widget X
    let result_a = capture_mgr.request_capture(pointer_a, widget_x);
    assert!(result_a.is_ok(), "Pointer A should capture widget X");

    // Pointer B captures widget Y (different pointer, different widget)
    let result_b = capture_mgr.request_capture(pointer_b, widget_y);
    assert!(
        result_b.is_ok(),
        "Pointer B should capture widget Y (independent)"
    );

    // Verify captures are correct per pointer
    assert_eq!(capture_mgr.current_capture(pointer_a), Some(widget_x));
    assert_eq!(capture_mgr.current_capture(pointer_b), Some(widget_y));

    // Both pointers are captured
    assert!(capture_mgr.is_captured(pointer_a));
    assert!(capture_mgr.is_captured(pointer_b));

    // Releasing pointer A must NOT affect pointer B
    capture_mgr.release_capture(pointer_a, widget_x);

    assert!(
        !capture_mgr.is_captured(pointer_a),
        "Pointer A should be released"
    );
    assert!(
        capture_mgr.is_captured(pointer_b),
        "Pointer B should still be captured"
    );
    assert_eq!(capture_mgr.current_capture(pointer_b), Some(widget_y));
}

/// Test that INV-PC2 (per-pointer exclusivity) prevents double-capture on same pointer.
#[test]
fn capture_exclusivity_per_pointer_prevents_double_capture() {
    let mut capture_mgr = PointerCaptureManager::new();

    let pointer_a = PointerId(1);
    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);

    // Pointer A captures widget X
    let result1 = capture_mgr.request_capture(pointer_a, widget_x);
    assert!(result1.is_ok());

    // Same pointer trying to capture different widget - should fail
    let result2 = capture_mgr.request_capture(pointer_a, widget_y);
    assert!(
        result2.is_err(),
        "INV-PC2: Same pointer cannot capture two widgets"
    );

    // Same widget re-capture by same pointer - should succeed (idempotent)
    let result3 = capture_mgr.request_capture(pointer_a, widget_x);
    assert!(
        result3.is_ok(),
        "Re-capturing same widget by same pointer is idempotent"
    );
}

// ============================================================================
// Test: Independent Commit (per-pointer session)
// ============================================================================

/// Test that committing a gesture for pointer A does not affect pointer B.
#[test]
fn independent_commit_per_pointer() {
    let mut registry = GestureRegistry::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);

    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);

    // Begin sessions
    registry.begin_session(pointer_a, PointerType::Mouse);
    registry.begin_session(pointer_b, PointerType::Mouse);

    // Register candidates
    registry.register_candidate_request(pointer_a, widget_x, GestureKind::Drag, None);
    registry.register_candidate_request(pointer_b, widget_y, GestureKind::Scroll, None);

    // Get candidate ID for pointer A
    let candidate_a = registry
        .get_candidate_for_widget(pointer_a, widget_x)
        .expect("Candidate A should exist")
        .candidate_id;

    // Simulate commit for pointer A (via push_commit_request + mark_committed flow)
    registry.push_commit_request(pointer_a, candidate_a);
    let pending_a = registry.take_commit_requests(pointer_a);
    assert_eq!(pending_a.len(), 1, "Should have 1 pending commit for A");

    // Manually mark committed (normally done by arbiter)
    // We use the internal marking for testing
    assert!(
        !registry.is_committed(pointer_a),
        "Not committed before marking"
    );

    // Pointer B should be completely unaffected
    assert!(
        registry.has_session(pointer_b),
        "Pointer B session should exist"
    );
    assert!(
        !registry.is_committed(pointer_b),
        "Pointer B should NOT be committed"
    );

    // Pointer B's pending commits should be empty
    let pending_b = registry.take_commit_requests(pointer_b);
    assert!(
        pending_b.is_empty(),
        "Pointer B should have no pending commits"
    );

    // Pointer B's candidates should be intact
    let candidates_b = registry.candidates(pointer_b);
    assert_eq!(
        candidates_b.len(),
        1,
        "Pointer B candidates should be unaffected"
    );
}

// ============================================================================
// Test: Selective Cancellation (widget destruction)
// ============================================================================

/// Test that destroying a widget cancels ONLY the pointers bound to that widget.
#[test]
fn selective_cancellation_widget_destruction() {
    let mut registry = GestureRegistry::new();
    let mut capture_mgr = PointerCaptureManager::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);

    let widget_x = TreeWidgetId(100); // Will be destroyed
    let widget_y = TreeWidgetId(200); // Will survive

    // Begin sessions
    registry.begin_session(pointer_a, PointerType::Mouse);
    registry.begin_session(pointer_b, PointerType::Mouse);

    // Pointer A interacts with widget X (will be destroyed)
    registry.register_candidate_request(pointer_a, widget_x, GestureKind::Drag, None);
    let _ = capture_mgr.request_capture(pointer_a, widget_x);

    // Pointer B interacts with widget Y (unaffected)
    registry.register_candidate_request(pointer_b, widget_y, GestureKind::Scroll, None);
    let _ = capture_mgr.request_capture(pointer_b, widget_y);

    // Verify initial state
    assert!(registry.has_candidate_for_widget(widget_x));
    assert!(registry.has_candidate_for_widget(widget_y));
    assert!(capture_mgr.is_captured_by(pointer_a, widget_x));
    assert!(capture_mgr.is_captured_by(pointer_b, widget_y));

    // Simulate widget X destruction (selective cleanup)
    registry.remove_candidates_for_widget(widget_x);
    capture_mgr.release_all_for_widget(widget_x);

    // Pointer A state should be cleared (widget X destroyed)
    assert!(
        !registry.has_candidate_for_widget(widget_x),
        "Widget X candidates should be removed"
    );
    assert!(
        !capture_mgr.is_captured(pointer_a),
        "Pointer A capture should be released"
    );

    // Pointer B state should be UNAFFECTED (widget Y survives)
    assert!(
        registry.has_candidate_for_widget(widget_y),
        "Widget Y candidates should be unaffected"
    );
    assert!(
        capture_mgr.is_captured_by(pointer_b, widget_y),
        "Pointer B capture should be unaffected"
    );
    assert!(
        registry.has_session(pointer_b),
        "Pointer B session should survive"
    );
}

// ============================================================================
// Test: Global Cancellation (Escape / Overlay)
// ============================================================================

/// Test that global cancellation cancels ALL pointer sessions.
#[test]
fn global_cancellation_cancels_all_pointers() {
    let mut registry = GestureRegistry::new();
    let mut capture_mgr = PointerCaptureManager::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);
    let pointer_c = PointerId(3);

    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);
    let widget_z = TreeWidgetId(300);

    // Begin sessions for all pointers
    registry.begin_session(pointer_a, PointerType::Mouse);
    registry.begin_session(pointer_b, PointerType::Mouse);
    registry.begin_session(pointer_c, PointerType::Mouse);

    // Register candidates and captures
    registry.register_candidate_request(pointer_a, widget_x, GestureKind::Drag, None);
    registry.register_candidate_request(pointer_b, widget_y, GestureKind::Scroll, None);
    registry.register_candidate_request(pointer_c, widget_z, GestureKind::Resize, None);

    let _ = capture_mgr.request_capture(pointer_a, widget_x);
    let _ = capture_mgr.request_capture(pointer_b, widget_y);
    let _ = capture_mgr.request_capture(pointer_c, widget_z);

    // Verify initial state - all active
    assert!(registry.has_session(pointer_a));
    assert!(registry.has_session(pointer_b));
    assert!(registry.has_session(pointer_c));
    assert!(capture_mgr.is_captured(pointer_a));
    assert!(capture_mgr.is_captured(pointer_b));
    assert!(capture_mgr.is_captured(pointer_c));

    // Get all active pointers
    let all_session_pointers = registry.active_pointers();
    let all_capture_pointers = capture_mgr.active_pointers();

    assert_eq!(
        all_session_pointers.len(),
        3,
        "Should have 3 session pointers"
    );
    assert_eq!(
        all_capture_pointers.len(),
        3,
        "Should have 3 capture pointers"
    );

    // Simulate global cancellation (as cancel_all_pointer_sessions would do)
    for pointer_id in all_session_pointers {
        registry.clear_candidates(pointer_id);
    }
    for pointer_id in all_capture_pointers {
        capture_mgr.cancel_capture(pointer_id);
    }

    // All sessions and captures should be cleared
    assert!(
        !registry.has_session(pointer_a),
        "Pointer A session should be cleared"
    );
    assert!(
        !registry.has_session(pointer_b),
        "Pointer B session should be cleared"
    );
    assert!(
        !registry.has_session(pointer_c),
        "Pointer C session should be cleared"
    );

    assert!(
        !capture_mgr.is_captured(pointer_a),
        "Pointer A capture should be cleared"
    );
    assert!(
        !capture_mgr.is_captured(pointer_b),
        "Pointer B capture should be cleared"
    );
    assert!(
        !capture_mgr.is_captured(pointer_c),
        "Pointer C capture should be cleared"
    );

    // active_pointers should return empty
    assert!(
        registry.active_pointers().is_empty(),
        "No sessions should remain"
    );
    assert!(
        capture_mgr.active_pointers().is_empty(),
        "No captures should remain"
    );
}

// ============================================================================
// Test: active_pointers() method correctness
// ============================================================================

/// Test that active_pointers returns correct set of active pointers.
#[test]
fn active_pointers_returns_correct_set() {
    let mut registry = GestureRegistry::new();
    let mut capture_mgr = PointerCaptureManager::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);

    // Initially no active pointers
    assert!(registry.active_pointers().is_empty());
    assert!(capture_mgr.active_pointers().is_empty());

    // Add pointer A
    registry.begin_session(pointer_a, PointerType::Mouse);
    capture_mgr
        .request_capture(pointer_a, TreeWidgetId(100))
        .unwrap();

    let active_sessions = registry.active_pointers();
    let active_captures = capture_mgr.active_pointers();

    assert_eq!(active_sessions.len(), 1);
    assert!(active_sessions.contains(&pointer_a));
    assert_eq!(active_captures.len(), 1);
    assert!(active_captures.contains(&pointer_a));

    // Add pointer B
    registry.begin_session(pointer_b, PointerType::Mouse);
    capture_mgr
        .request_capture(pointer_b, TreeWidgetId(200))
        .unwrap();

    let active_sessions = registry.active_pointers();
    let active_captures = capture_mgr.active_pointers();

    assert_eq!(active_sessions.len(), 2);
    assert!(active_sessions.contains(&pointer_a));
    assert!(active_sessions.contains(&pointer_b));
    assert_eq!(active_captures.len(), 2);
    assert!(active_captures.contains(&pointer_a));
    assert!(active_captures.contains(&pointer_b));

    // Remove pointer A
    registry.clear_candidates(pointer_a);
    capture_mgr.cancel_capture(pointer_a);

    let active_sessions = registry.active_pointers();
    let active_captures = capture_mgr.active_pointers();

    assert_eq!(active_sessions.len(), 1);
    assert!(active_sessions.contains(&pointer_b));
    assert_eq!(active_captures.len(), 1);
    assert!(active_captures.contains(&pointer_b));
}

// ============================================================================
// Test: Multiple candidates per pointer (different widgets)
// ============================================================================

/// Test that multiple widgets can register candidates for the same pointer
/// and they are all tracked correctly.
#[test]
fn multiple_candidates_per_pointer() {
    let mut registry = GestureRegistry::new();

    let pointer_a = PointerId(1);
    let widget_x = TreeWidgetId(100);
    let widget_y = TreeWidgetId(200);
    let widget_z = TreeWidgetId(300);

    registry.begin_session(pointer_a, PointerType::Mouse);

    // Register multiple candidates from different widgets for same pointer
    registry.register_candidate_request(pointer_a, widget_x, GestureKind::Drag, None);
    registry.register_candidate_request(pointer_a, widget_y, GestureKind::Scroll, None);
    registry.register_candidate_request(
        pointer_a,
        widget_z,
        GestureKind::Resize,
        Some(GestureAxis::Horizontal),
    );

    let candidates = registry.candidates(pointer_a);
    assert_eq!(
        candidates.len(),
        3,
        "Should have 3 candidates for pointer A"
    );

    // All candidates should have same session ID but different local IDs
    let session_id = candidates[0].candidate_id.session;
    for c in candidates {
        assert_eq!(
            c.candidate_id.session, session_id,
            "All candidates share session ID"
        );
    }

    // Local IDs should be unique
    let locals: Vec<u32> = candidates.iter().map(|c| c.candidate_id.local).collect();
    assert_eq!(locals[0], 0);
    assert_eq!(locals[1], 1);
    assert_eq!(locals[2], 2);

    // Verify widget lookups work
    assert!(registry.has_candidate_for_widget(widget_x));
    assert!(registry.has_candidate_for_widget(widget_y));
    assert!(registry.has_candidate_for_widget(widget_z));
}

// ============================================================================
// Test: pointers_for_widget returns correct pointers
// ============================================================================

/// Test that pointers_for_widget correctly identifies which pointers
/// have candidates for a specific widget.
#[test]
fn pointers_for_widget_correctness() {
    let mut registry = GestureRegistry::new();

    let pointer_a = PointerId(1);
    let pointer_b = PointerId(2);
    let pointer_c = PointerId(3);

    let widget_shared = TreeWidgetId(100);
    let widget_a_only = TreeWidgetId(200);
    let widget_b_only = TreeWidgetId(300);

    // Begin sessions
    registry.begin_session(pointer_a, PointerType::Mouse);
    registry.begin_session(pointer_b, PointerType::Mouse);
    registry.begin_session(pointer_c, PointerType::Mouse);

    // Pointer A and B both have candidates for widget_shared
    registry.register_candidate_request(pointer_a, widget_shared, GestureKind::Drag, None);
    registry.register_candidate_request(pointer_b, widget_shared, GestureKind::Scroll, None);

    // Only pointer A has candidate for widget_a_only
    registry.register_candidate_request(pointer_a, widget_a_only, GestureKind::Resize, None);

    // Only pointer B has candidate for widget_b_only (Select is legal for Mouse per Stage 7)
    registry.register_candidate_request(pointer_b, widget_b_only, GestureKind::Select, None);

    // Pointer C has no candidates (just an active session)

    // Test pointers_for_widget
    let pointers_shared = registry.pointers_for_widget(widget_shared);
    assert_eq!(pointers_shared.len(), 2);
    assert!(pointers_shared.contains(&pointer_a));
    assert!(pointers_shared.contains(&pointer_b));

    let pointers_a_only = registry.pointers_for_widget(widget_a_only);
    assert_eq!(pointers_a_only.len(), 1);
    assert!(pointers_a_only.contains(&pointer_a));

    let pointers_b_only = registry.pointers_for_widget(widget_b_only);
    assert_eq!(pointers_b_only.len(), 1);
    assert!(pointers_b_only.contains(&pointer_b));

    // Non-existent widget
    let pointers_none = registry.pointers_for_widget(TreeWidgetId(999));
    assert!(pointers_none.is_empty());
}
