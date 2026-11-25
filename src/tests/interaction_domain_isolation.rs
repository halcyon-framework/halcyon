// SPEC-HAL-151: Interaction Domain Isolation Tests
//
// Engine-internal tests verifying per-window interaction state isolation.
//
// INV-151-ACTIVE-WINDOW-GATE: Only the active window receives pointer/keyboard events.
// INV-151-FOCUS-TEARDOWN: Focus loss cancels all pointer sessions and clears state.
//
// These tests verify:
// 1. Focus switch cancels drag in window A, no callbacks in window B
// 2. Non-active window pointer events are ignored (except allowed events)

use crate::core::event::{
    FusedInputEvent, InteractionDomains, PointerButton, PointerButtonsState, PointerDevice,
    PointerId, PointerType, WindowId,
};
use std::time::Instant;

/// Test 1: Focus switch cancels drag in window A, window B unaffected.
///
/// INV-151-FOCUS-TEARDOWN: When window A loses focus, all pointer sessions,
/// gesture candidates, captures, hover, and scroll capture are cleared.
/// Window B (even if it gains focus) starts fresh with no inherited state.
#[test]
fn test_focus_switch_cancels_drag_in_losing_window() {
    let mut domains = InteractionDomains::new();

    // Register a second window
    let window_b = WindowId(1);
    domains.register_window(window_b);

    // Simulate window A (PRIMARY) has active state: capture, gesture, hover, scroll
    {
        let state_a = domains.primary_state_mut();
        // Set up capture
        state_a
            .pointer_capture_manager
            .request_capture(PointerId::MOUSE, crate::utils::id::TreeWidgetId(100))
            .unwrap();
        // Set up gesture session
        state_a
            .gesture_registry
            .begin_session(PointerId::MOUSE, PointerType::Mouse);
        // Set hover
        state_a.hovered_widget = Some(crate::utils::id::TreeWidgetId(100));
        // Set scroll capture
        state_a.scroll_capture_owner = Some(crate::utils::id::TreeWidgetId(100));
    }

    // Verify window A has active state
    {
        let state_a = domains.primary_state();
        assert!(
            state_a
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        assert!(state_a.gesture_registry.has_session(PointerId::MOUSE));
        assert!(state_a.hovered_widget.is_some());
        assert!(state_a.scroll_capture_owner.is_some());
    }

    // Window A loses focus (simulating user clicking window B)
    // INV-151-FOCUS-TEARDOWN: This should clear all state
    let _ = domains.handle_focus_changed(WindowId::PRIMARY, false);

    // Verify window A's state is cleared
    {
        let state_a = domains.primary_state();
        assert!(
            !state_a
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE),
            "INV-151-FOCUS-TEARDOWN: Capture must be released on focus loss"
        );
        assert!(
            !state_a.gesture_registry.has_session(PointerId::MOUSE),
            "INV-151-FOCUS-TEARDOWN: Gesture sessions must be cleared on focus loss"
        );
        assert!(
            state_a.hovered_widget.is_none(),
            "INV-151-FOCUS-TEARDOWN: Hover must be cleared on focus loss"
        );
        assert!(
            state_a.scroll_capture_owner.is_none(),
            "INV-151-FOCUS-TEARDOWN: Scroll capture must be cleared on focus loss"
        );
    }

    // Window B gains focus
    let _ = domains.handle_focus_changed(window_b, true);

    // Verify window B has clean state (no inherited state from A)
    {
        let state_b = domains.resolve_window_mut(window_b).unwrap();
        assert!(
            !state_b
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE),
            "Window B must start with no captures"
        );
        assert!(
            !state_b.gesture_registry.has_session(PointerId::MOUSE),
            "Window B must start with no gesture sessions"
        );
        assert!(
            state_b.hovered_widget.is_none(),
            "Window B must start with no hover"
        );
        assert!(
            state_b.scroll_capture_owner.is_none(),
            "Window B must start with no scroll capture"
        );
    }

    // Verify active window is now B
    assert_eq!(
        domains.active_window(),
        Some(window_b),
        "Active window must be window B after focus"
    );
}

/// Test 2: Non-active window pointer events are ignored (except allowed events).
///
/// INV-151-ACTIVE-WINDOW-GATE: Only FocusChanged, Resize, and Tick events
/// are routed from non-active windows. All other events are dropped.
#[test]
fn test_non_active_window_events_ignored() {
    let mut domains = InteractionDomains::new();

    // Register window B
    let window_b = WindowId(1);
    domains.register_window(window_b);

    // Window A (PRIMARY) is active by default
    assert_eq!(domains.active_window(), Some(WindowId::PRIMARY));

    // Test: PointerMove from window B should be rejected
    let pointer_move_b = FusedInputEvent::PointerMove {
        window_id: window_b,
        time: Instant::now(),
        pos: crate::core::math::Point::new(100.0, 100.0),
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::default(),
    };
    assert!(
        !domains.should_route_event(&pointer_move_b),
        "INV-151-ACTIVE-WINDOW-GATE: PointerMove from non-active window must be dropped"
    );

    // Test: PointerDown from window B should be rejected
    let pointer_down_b = FusedInputEvent::PointerDown {
        window_id: window_b,
        time: Instant::now(),
        pos: crate::core::math::Point::new(100.0, 100.0),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    assert!(
        !domains.should_route_event(&pointer_down_b),
        "INV-151-ACTIVE-WINDOW-GATE: PointerDown from non-active window must be dropped"
    );

    // Test: KeyDown from window B should be rejected
    let key_down_b = FusedInputEvent::KeyDown {
        window_id: window_b,
        time: Instant::now(),
        key_code: crate::core::event::KeyCode::Tab,
        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
        reservation: None,
        modifiers: crate::core::event::Modifiers::default(),
        repeat: false,
    };
    assert!(
        !domains.should_route_event(&key_down_b),
        "INV-151-ACTIVE-WINDOW-GATE: KeyDown from non-active window must be dropped"
    );

    // Test: FocusChanged from window B should be ALLOWED (needed to switch focus)
    let focus_changed_b = FusedInputEvent::FocusChanged {
        window_id: window_b,
        time: Instant::now(),
        focused: true,
    };
    assert!(
        domains.should_route_event(&focus_changed_b),
        "INV-151-ACTIVE-WINDOW-GATE: FocusChanged must always be routed"
    );

    // Test: Resize from window B should be ALLOWED
    let resize_b = FusedInputEvent::Resize {
        window_id: window_b,
        width: 800.0,
        height: 600.0,
    };
    assert!(
        domains.should_route_event(&resize_b),
        "INV-151-ACTIVE-WINDOW-GATE: Resize must always be routed"
    );

    // Test: Tick from window B should be ALLOWED
    let tick_b = FusedInputEvent::Tick {
        window_id: window_b,
        time: Instant::now(),
        dt: std::time::Duration::from_millis(16),
    };
    assert!(
        domains.should_route_event(&tick_b),
        "INV-151-ACTIVE-WINDOW-GATE: Tick must always be routed"
    );

    // Test: Events from active window A should be allowed
    let pointer_move_a = FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: crate::core::math::Point::new(100.0, 100.0),
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::default(),
    };
    assert!(
        domains.should_route_event(&pointer_move_a),
        "Events from active window must be routed"
    );

    // After window B gains focus, events from A should be rejected
    let _ = domains.handle_focus_changed(window_b, true);
    assert_eq!(domains.active_window(), Some(window_b));

    assert!(
        !domains.should_route_event(&pointer_move_a),
        "INV-151-ACTIVE-WINDOW-GATE: PointerMove from non-active window A must be dropped after focus switch"
    );

    // Now events from B should be allowed
    assert!(
        domains.should_route_event(&pointer_move_b),
        "Events from newly active window B must be routed"
    );
}
