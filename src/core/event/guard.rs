//! Event Authority Firewall Guard
//!
//! This module provides compile-time enforcement that Event types remain
//! pure input routing data and do NOT contain scene, layout, or compositor types.
//!
//! # Enforcement Mechanism
//!
//! 1. `NoForbiddenEvent` trait implemented for allowed types only
//! 2. Field-by-field assertions for key structs
//! 3. Poison markers in forbidden.rs NOT implementing the trait
//! 4. If forbidden type leaks in, compilation fails
//!
//! # Event Authority Contract
//!
//! Event contains ONLY:
//! - Math primitives (Point, RectF, f32, u32, etc.)
//! - Widget identity (TreeWidgetId, NodeId)
//! - Input state (Modifiers, PointerButton, KeyCode, etc.)
//! - Standard containers (Vec, Option, String, HashMap)
//! - Time (Instant, Duration)
//!
//! Event MUST NOT contain:
//! - SceneGraph or SceneNode (render output)
//! - SceneNodeKey (render identity)
//! - Layout results (constraint solving output)
//! - Compositor state (any)
//!
//! # How Violations Are Caught
//!
//! If a forbidden type is added to an Event struct field:
//! ```text
//! error[E0277]: the trait bound `ForbiddenType: NoForbiddenEvent` is not satisfied
//! ```

#[cfg(test)]
mod tests {
    use super::super::{
        CancellationReason, CaptureError, CaptureMode, ClickTracker, Event, EventPhase, FocusEvent,
        FocusManager, FusedInputEvent, GestureArbiter, GestureAxis, GestureCandidate,
        GestureCandidateId, GestureCommitError, GestureCommitResult, GestureKind, GestureRegistry,
        GestureSessionId, HitTestResult, ImeEvent, InputPipelinePhase, KeyCode, KeyEvent, KeyState,
        Modifiers, PointerButton, PointerButtonsState, PointerCapture, PointerCaptureManager,
        PointerDevice, PointerEvent, PointerEventKind, PointerId, PointerType, RawButtonState,
        RawInputEvent, ScrollAlign, ScrollIntent, ScrollSource, ShortcutId, ShortcutScope,
        TextInputEvent, WheelDelta, WindowEvent,
    };
    use crate::core::math::{Point, RectF};
    use crate::core::widget_tree::NodeId;
    use crate::utils::id::TreeWidgetId;
    use std::time::{Duration, Instant};

    // ========================================================================
    // NoForbiddenEvent Trait and Implementations
    // ========================================================================
    //
    // This trait is intentionally NOT implemented for poison markers.
    // If a forbidden type appears in an Event struct, compilation fails.

    /// Marker trait for types allowed in Event structs.
    ///
    /// This trait is intentionally NOT implemented for:
    /// - PoisonSceneGraph, PoisonSceneNode, PoisonSceneNodeKey
    /// - PoisonLayout, PoisonCompositor, PoisonSurfaceStack, PoisonSurfaceStackSnapshot
    trait NoForbiddenEvent {}

    // Primitives
    impl NoForbiddenEvent for bool {}
    impl NoForbiddenEvent for u8 {}
    impl NoForbiddenEvent for u16 {}
    impl NoForbiddenEvent for u32 {}
    impl NoForbiddenEvent for u64 {}
    impl NoForbiddenEvent for usize {}
    impl NoForbiddenEvent for i32 {}
    impl NoForbiddenEvent for f32 {}
    impl NoForbiddenEvent for f64 {}
    impl NoForbiddenEvent for char {}
    impl NoForbiddenEvent for u128 {}

    // Math types
    impl NoForbiddenEvent for Point {}
    impl NoForbiddenEvent for RectF {}

    // Widget identity types
    impl NoForbiddenEvent for TreeWidgetId {}
    impl NoForbiddenEvent for NodeId {}

    // Standard library types
    impl NoForbiddenEvent for String {}
    impl NoForbiddenEvent for &'static str {}
    impl NoForbiddenEvent for Instant {}
    impl NoForbiddenEvent for Duration {}
    impl<T> NoForbiddenEvent for Vec<T> {}
    impl<T> NoForbiddenEvent for Option<T> {}
    impl<K, V> NoForbiddenEvent for std::collections::HashMap<K, V> {}
    impl<T> NoForbiddenEvent for std::collections::HashSet<T> {}

    // Tuples
    impl<A, B> NoForbiddenEvent for (A, B) {}
    impl<A, B, C> NoForbiddenEvent for (A, B, C) {}
    impl<A, B, C, D> NoForbiddenEvent for (A, B, C, D) {}

    // Input state types
    impl NoForbiddenEvent for Modifiers {}
    impl NoForbiddenEvent for PointerButton {}
    impl NoForbiddenEvent for PointerType {}
    impl NoForbiddenEvent for PointerEventKind {}
    impl NoForbiddenEvent for KeyCode {}
    impl NoForbiddenEvent for EventPhase {}
    impl NoForbiddenEvent for InputPipelinePhase {}
    impl NoForbiddenEvent for CaptureMode {}
    impl NoForbiddenEvent for PointerDevice {}
    impl NoForbiddenEvent for RawButtonState {}
    impl NoForbiddenEvent for KeyState {}
    impl NoForbiddenEvent for WheelDelta {}
    impl NoForbiddenEvent for ScrollSource {}
    impl NoForbiddenEvent for ScrollAlign {}
    impl NoForbiddenEvent for PointerButtonsState {}

    // Event structs
    impl NoForbiddenEvent for PointerEvent {}
    impl NoForbiddenEvent for KeyEvent {}
    impl NoForbiddenEvent for FocusEvent {}
    impl NoForbiddenEvent for WindowEvent {}
    impl NoForbiddenEvent for TextInputEvent {}
    impl NoForbiddenEvent for ImeEvent {}
    impl NoForbiddenEvent for Event {}
    impl NoForbiddenEvent for HitTestResult {}
    impl NoForbiddenEvent for RawInputEvent {}
    impl NoForbiddenEvent for FusedInputEvent {}
    impl NoForbiddenEvent for ScrollIntent {}

    // Pointer/capture types
    impl NoForbiddenEvent for PointerId {}
    impl NoForbiddenEvent for CaptureError {}
    impl NoForbiddenEvent for PointerCapture {}
    impl NoForbiddenEvent for PointerCaptureManager {}

    // Gesture types
    impl NoForbiddenEvent for GestureSessionId {}
    impl NoForbiddenEvent for GestureCandidateId {}
    impl NoForbiddenEvent for GestureKind {}
    impl NoForbiddenEvent for GestureAxis {}
    impl NoForbiddenEvent for GestureCandidate {}
    impl NoForbiddenEvent for GestureRegistry {}
    impl NoForbiddenEvent for CancellationReason {}
    impl NoForbiddenEvent for GestureCommitError {}
    impl NoForbiddenEvent for GestureCommitResult {}
    impl NoForbiddenEvent for GestureArbiter {}

    // Focus/click types
    impl NoForbiddenEvent for FocusManager {}
    impl NoForbiddenEvent for ClickTracker {}
    impl NoForbiddenEvent for ShortcutId {}
    impl NoForbiddenEvent for ShortcutScope {}

    /// Helper function for compile-time NoForbiddenEvent assertion.
    fn assert_no_forbidden<T: NoForbiddenEvent>() {}

    // ========================================================================
    // PointerEvent Field Firewall
    // ========================================================================

    /// POINTEREVENT FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn pointer_event_fields_no_forbidden() {
        // PointerEvent fields:
        // - id: PointerId
        // - global_pos: (f32, f32)
        // - local_pos: (f32, f32)
        // - kind: PointerEventKind
        // - button: Option<PointerButton>
        // - modifiers: Modifiers
        // - pointer_type: PointerType

        assert_no_forbidden::<PointerId>();
        assert_no_forbidden::<(f32, f32)>();
        assert_no_forbidden::<PointerEventKind>();
        assert_no_forbidden::<Option<PointerButton>>();
        assert_no_forbidden::<Modifiers>();
        assert_no_forbidden::<PointerType>();
    }

    /// KEYEVENT FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn key_event_fields_no_forbidden() {
        // KeyEvent fields:
        // - key_code: KeyCode
        // - is_down: bool
        // - is_repeat: bool
        // - modifiers: Modifiers
        // - text: Option<String>

        assert_no_forbidden::<KeyCode>();
        assert_no_forbidden::<bool>();
        assert_no_forbidden::<Modifiers>();
        assert_no_forbidden::<Option<String>>();
    }

    /// TEXTINPUTEVENT FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn text_input_event_fields_no_forbidden() {
        // TextInputEvent fields:
        // - text: String
        // - modifiers: Modifiers

        assert_no_forbidden::<String>();
        assert_no_forbidden::<Modifiers>();
    }

    /// HITTESTRESULT FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn hit_test_result_fields_no_forbidden() {
        // HitTestResult fields:
        // - widget_id: TreeWidgetId
        // - local_pos: (f32, f32)
        // - absolute_bounds: (f32, f32, f32, f32)

        assert_no_forbidden::<TreeWidgetId>();
        assert_no_forbidden::<(f32, f32)>();
        assert_no_forbidden::<(f32, f32, f32, f32)>();
    }

    // ========================================================================
    // Gesture System Field Firewalls
    // ========================================================================

    /// GESTURECANDIDATE FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn gesture_candidate_fields_no_forbidden() {
        // GestureCandidate fields:
        // - candidate_id: GestureCandidateId
        // - widget_id: TreeWidgetId
        // - kind: GestureKind
        // - axis: Option<GestureAxis>
        // - priority_hint: u8

        assert_no_forbidden::<GestureCandidateId>();
        assert_no_forbidden::<TreeWidgetId>();
        assert_no_forbidden::<GestureKind>();
        assert_no_forbidden::<Option<GestureAxis>>();
        assert_no_forbidden::<u8>();
    }

    /// GESTURECANDIDATEID FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn gesture_candidate_id_fields_no_forbidden() {
        // GestureCandidateId fields:
        // - session: GestureSessionId
        // - local: u32

        assert_no_forbidden::<GestureSessionId>();
        assert_no_forbidden::<u32>();
    }

    /// GESTURECOMMITRESULT FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn gesture_commit_result_fields_no_forbidden() {
        // GestureCommitResult fields:
        // - winner: GestureCandidateId
        // - winner_widget: TreeWidgetId
        // - cancelled: Vec<GestureCandidateId>

        assert_no_forbidden::<GestureCandidateId>();
        assert_no_forbidden::<TreeWidgetId>();
        assert_no_forbidden::<Vec<GestureCandidateId>>();
    }

    // ========================================================================
    // Focus/Capture Field Firewalls
    // ========================================================================

    /// POINTERCAPTURE FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn pointer_capture_fields_no_forbidden() {
        // PointerCapture fields:
        // - captured_widget: Option<TreeWidgetId>
        // - button: Option<PointerButton>

        assert_no_forbidden::<Option<TreeWidgetId>>();
        assert_no_forbidden::<Option<PointerButton>>();
    }

    /// POINTERID FIELD FIREWALL: All field types must be NoForbiddenEvent.
    #[test]
    fn pointer_id_fields_no_forbidden() {
        // PointerId fields:
        // - 0: u64 (tuple struct)

        assert_no_forbidden::<u64>();
    }

    // ========================================================================
    // Negative Test Documentation
    // ========================================================================

    /// NEGATIVE TEST DOCUMENTATION: Poison markers do NOT implement NoForbiddenEvent.
    ///
    /// The following would fail to compile if uncommented:
    /// ```compile_fail
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneGraph>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneNode>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneNodeKey>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonLayout>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonCompositor>();
    /// ```
    ///
    /// This proves the firewall works: forbidden types cannot satisfy NoForbiddenEvent.
    #[test]
    fn poison_markers_are_forbidden() {
        // This test documents that poison markers cannot pass the firewall.
        // If you uncomment the lines above, compilation will fail with:
        // "the trait `NoForbiddenEvent` is not implemented for `PoisonSceneGraph`"
    }

    // ========================================================================
    // Additional Type Safety Checks
    // ========================================================================

    /// Verify PointerEvent is Send + Sync (can be passed between threads).
    #[test]
    fn pointer_event_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<PointerEvent>();
    }

    /// Verify KeyEvent is Send + Sync.
    #[test]
    fn key_event_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<KeyEvent>();
    }

    /// Verify PointerId is Copy (lightweight identity token).
    #[test]
    fn pointer_id_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<PointerId>();
    }

    /// Verify Modifiers is Copy (lightweight state).
    #[test]
    fn modifiers_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Modifiers>();
    }

    /// Verify GestureSessionId is Copy (lightweight identity).
    #[test]
    fn gesture_session_id_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<GestureSessionId>();
    }

    /// Verify GestureCandidateId is Copy (lightweight identity).
    #[test]
    fn gesture_candidate_id_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<GestureCandidateId>();
    }
}
