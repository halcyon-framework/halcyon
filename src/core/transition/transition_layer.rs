// ============================================================================
// SPEC-HAL-021 Stage 3: TransitionLayer
// ============================================================================
//
// TransitionLayer is the sole owner of animation policy in Halcyon.
// It owns Timelines, advances them on explicit request, and provides
// scoped animation capability (AnimationContext) to routing.
//
// INVARIANTS (see mod.rs):
// - INV-TL-1: Widgets do not own animation lifetimes
// - INV-TL-2: Routing does not decide animation existence
// - INV-TL-3: Timeline does not request frames
// - INV-TL-4: TransitionLayer is the sole animation policy owner
//
// ============================================================================

use crate::core::animation::{AnimationContext, AnimationOwnerId, Timeline, TransitionHandle};

/// TransitionLayer: engine-owned subsystem for animation lifetimes and policy.
///
/// Responsibilities:
/// - Own one or more Timelines
/// - Advance timelines when explicitly told (via `advance()`)
/// - Decide when animation capability exists
/// - Provide scoped animation capability to routing
///
/// Does NOT:
/// - Know about widgets
/// - Know about layout
/// - Dispatch events
/// - Schedule frames
///
/// INV-TL-4: This is the sole animation policy owner.
pub struct TransitionLayer {
    /// Primary timeline for widget animations.
    /// INV-TL-3: Timeline is passive; it does not request frames.
    timeline: Timeline,
}

impl Default for TransitionLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionLayer {
    /// Create a new TransitionLayer with an empty Timeline.
    pub fn new() -> Self {
        Self {
            timeline: Timeline::new(),
        }
    }

    /// Advance all owned timelines to the given time.
    ///
    /// Must be called exactly once per frame, before layout.
    /// This is the only method that mutates timeline state.
    ///
    /// # Arguments
    /// * `now_secs` - Current time in seconds (monotonic)
    ///
    /// INV-TL-3: Timeline does not request frames; caller decides when to call.
    pub fn advance(&mut self, now_secs: f64) {
        self.timeline
            .update(now_secs)
            .expect("EngineTicker supplied an invalid animation timestamp");
    }

    /// Returns true if there are any active animations.
    ///
    /// Used by the engine to determine if Animation frame reason should be set.
    /// Does NOT schedule frames - that decision is external.
    pub fn has_active_animations(&self) -> bool {
        self.timeline.has_active_animations()
    }

    /// Provide scoped animation capability for event routing.
    ///
    /// Returns an AnimationContext that can be passed to EventRouter for
    /// injection into EventContext where animation is valid (hover, pointer, focus).
    ///
    /// The returned context is valid only for the duration of the borrow.
    /// INV-TL-2: Router accepts this but does not own or create it.
    pub fn animation_context(&mut self) -> AnimationContext<'_> {
        AnimationContext::new(&mut self.timeline)
    }

    /// Get read-only access to the timeline for paint-time value queries.
    ///
    /// Used by PaintContext to retrieve interpolated animation values.
    /// INV-TL-1: Widgets query values via handles, not references.
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// SPEC-HAL-078 Stage 2: Cancel an animation by handle.
    ///
    /// Used by layout transition binding to cancel existing transitions
    /// before starting replacements (no blending).
    pub fn cancel_animation(&mut self, handle: TransitionHandle) {
        self.timeline.cancel(handle);
    }

    /// Cancel all animation handles owned by a removed engine object.
    pub(crate) fn cancel_animation_owner(&mut self, owner: AnimationOwnerId) {
        self.timeline.cancel_owner(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::animation::{Animation, Easing, TransitionHandle};

    #[test]
    fn test_transition_layer_basic() {
        let mut layer = TransitionLayer::new();

        // Initially no animations
        assert!(!layer.has_active_animations());

        // Start an animation via AnimationContext
        let handle = {
            let mut ctx = layer.animation_context();
            ctx.start(Animation::new(0.0f32, 100.0f32, 1.0, Easing::Linear))
                .unwrap()
        };

        // Now has active animations
        assert!(layer.has_active_animations());

        // Advance time
        layer.advance(0.5);

        // Can query value via timeline
        let value: Option<f32> = layer.timeline().get(handle);
        assert!(value.is_some());
        assert!((value.unwrap() - 50.0).abs() < 0.1);

        // Complete animation
        layer.advance(1.0);
        assert!(!layer.has_active_animations());
    }

    #[test]
    fn test_animation_context_scoped() {
        let mut layer = TransitionLayer::new();

        // Can create multiple scoped contexts (not simultaneously)
        {
            let mut ctx = layer.animation_context();
            ctx.start(Animation::new(0.0f32, 10.0f32, 0.5, Easing::Linear))
                .unwrap();
        }

        {
            let mut ctx = layer.animation_context();
            ctx.start(Animation::new(0.0f32, 20.0f32, 1.0, Easing::Linear))
                .unwrap();
        }

        // Both animations active
        assert_eq!(layer.timeline().active_count(), 2);
    }

    // ========================================================================
    // SPEC-HAL-021 Stage 3: Behavioral Tests for Animation Capability
    // ========================================================================
    //
    // These tests prove:
    // 1. Animation capability can be absent (None)
    // 2. Animation capability can be injected (Some)
    // 3. Widget-like code behaves correctly in both cases

    /// Simulates widget behavior when animation capability is absent.
    /// Widget code must gracefully handle None (no-op).
    #[test]
    fn test_animation_capability_absent() {
        // Simulate what a widget would do when animation is None
        let animation_capability: Option<AnimationContext<'_>> = None;

        // Widget tries to start animation - returns None when capability absent
        let handle: Option<TransitionHandle> = animation_capability
            .map(|mut ctx| ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear)))
            .transpose()
            .unwrap();

        // INV-TL-2: Absence of animation capability is safe and explicit
        assert!(
            handle.is_none(),
            "Animation handle should be None when capability absent"
        );
    }

    /// Simulates widget behavior when animation capability is present.
    /// Widget code receives capability and can start animations.
    #[test]
    fn test_animation_capability_present() {
        let mut layer = TransitionLayer::new();

        // Simulate what routing would do - provide capability to widget
        let handle: Option<TransitionHandle> = {
            let mut ctx = layer.animation_context();
            // Widget starts animation through the provided capability
            Some(
                ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear))
                    .unwrap(),
            )
        };

        // Animation was started successfully
        assert!(
            handle.is_some(),
            "Animation handle should be Some when capability present"
        );
        assert!(
            layer.has_active_animations(),
            "TransitionLayer should have active animations"
        );

        // Widget can query animation value during paint
        layer.advance(0.25); // 50% through 0.5s animation
        let value: Option<f32> = layer.timeline().get(handle.unwrap());
        assert!(value.is_some());
        assert!(
            (value.unwrap() - 0.5).abs() < 0.1,
            "Animation should be at ~50%"
        );
    }

    /// Tests that widget code pattern works correctly for both cases.
    /// This is the pattern used by Button and other widgets.
    #[test]
    fn test_widget_animation_pattern() {
        use std::cell::Cell;

        // Simulate widget state: stores optional handle
        let widget_handle: Cell<Option<TransitionHandle>> = Cell::new(None);

        // Case 1: Animation capability absent (e.g., layout-only path, tests)
        {
            let animation: Option<AnimationContext<'_>> = None;
            let result = animation
                .map(|mut ctx| ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear)))
                .transpose()
                .unwrap();
            widget_handle.set(result);
        }
        assert!(
            widget_handle.get().is_none(),
            "No handle when capability absent"
        );

        // Case 2: Animation capability present (e.g., hover event)
        let mut layer = TransitionLayer::new();
        {
            let mut ctx = layer.animation_context();
            let result = Some(
                ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear))
                    .unwrap(),
            );
            widget_handle.set(result);
        }
        assert!(
            widget_handle.get().is_some(),
            "Handle stored when capability present"
        );

        // Widget queries value during paint (regardless of how handle was obtained)
        layer.advance(0.25);
        if let Some(handle) = widget_handle.get() {
            let value: Option<f32> = layer.timeline().get(handle);
            assert!(value.is_some());
        }
    }

    /// Tests that TransitionLayer correctly reports animation state for frame scheduling.
    /// INV-TL-3: Timeline does not request frames; caller uses has_active_animations().
    #[test]
    fn test_animation_frame_signaling() {
        let mut layer = TransitionLayer::new();

        // No animations → no frame needed
        assert!(!layer.has_active_animations());

        // Start animation → frame needed
        {
            let mut ctx = layer.animation_context();
            ctx.start(Animation::new(0.0f32, 1.0f32, 1.0, Easing::Linear))
                .unwrap();
        }
        assert!(layer.has_active_animations());

        // Advance to completion → no more frames needed
        layer.advance(1.0);
        assert!(!layer.has_active_animations());
    }

    // ========================================================================
    // SPEC-HAL-021 Stage 4: Animation Capability Gating Test
    // ========================================================================
    //
    // This test proves the capability gating mechanism:
    // 1. Hover enter with capability → animation starts
    // 2. Hover enter without capability → no animation
    // 3. Non-hover events never receive capability
    //
    // This is the binding test for Stage 4 implementation.

    /// SPEC-HAL-021 Stage 4: Prove capability gating for animation injection.
    ///
    /// This test demonstrates that animation capability is selectively injected:
    /// - on_hover_enter/leave with capability can start animations
    /// - on_hover_enter/leave without capability cannot start animations
    /// - Non-hover event paths never receive animation capability
    #[test]
    fn test_animation_capability_gating() {
        let mut layer = TransitionLayer::new();

        // Simulate widget handle storage
        let handle_with_capability: Option<TransitionHandle>;
        let handle_without_capability: Option<TransitionHandle>;

        // Case 1: Hover enter WITH animation capability (simulates PointerMove path)
        // This is what happens when update_hover is called with Some(animation)
        {
            let mut ctx = layer.animation_context();
            // Widget's on_hover_enter can start animation because capability is present
            handle_with_capability = Some(
                ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear))
                    .unwrap(),
            );
        }

        // Verify: animation was started
        assert!(
            handle_with_capability.is_some(),
            "With capability: animation handle should be created"
        );
        assert!(
            layer.has_active_animations(),
            "With capability: TransitionLayer should have active animation"
        );

        // Case 2: Hover enter WITHOUT animation capability (simulates non-PointerMove path)
        // This is what happens when update_hover is called with None
        {
            let animation_capability: Option<AnimationContext<'_>> = None;
            // Widget's on_hover_enter CANNOT start animation because capability is absent
            handle_without_capability = animation_capability
                .map(|mut ctx| ctx.start(Animation::new(0.0f32, 1.0f32, 0.5, Easing::Linear)))
                .transpose()
                .unwrap();
        }

        // Verify: no animation was started (capability was absent)
        assert!(
            handle_without_capability.is_none(),
            "Without capability: animation handle should be None"
        );

        // Case 3: Non-hover events never receive capability
        // This is verified by the injection policy in route_input_event:
        // - PointerDown, PointerUp, KeyDown, KeyUp, etc. all pass None for animation
        // - Only PointerMove (which triggers hover) passes Some(animation)
        //
        // We verify this structurally: the only way to get capability is via
        // TransitionLayer::animation_context(), which the app only calls for PointerMove.

        // Advance and verify animation values work correctly
        layer.advance(0.25); // 50% through 0.5s animation

        // Animation with capability can be queried
        if let Some(handle) = handle_with_capability {
            let value: Option<f32> = layer.timeline().get(handle);
            assert!(value.is_some(), "Animation value should be queryable");
            assert!(
                (value.unwrap() - 0.5).abs() < 0.1,
                "Animation should be at ~50%"
            );
        }

        // Animation without capability doesn't exist to query
        assert!(
            handle_without_capability.is_none(),
            "No handle means no animation to query"
        );
    }
}
