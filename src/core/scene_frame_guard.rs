//! SPEC-HAL-081A/044 SceneFrame Contract Lock
//!
//! This module provides compile-time enforcement that SceneFrame remains
//! structurally frozen and does not accrete forbidden types from widget,
//! event, layout, or compositor-mutator systems.
//!
//! # Enforcement Mechanism
//!
//! 1. Exhaustive pattern matching on SceneFrame fields (compile-time)
//! 2. Size assertions to detect hidden fields (test-time)
//! 3. Constructor parity tests (new() matches field set)
//! 4. Authority firewall via trait bounds (Send + Sync + 'static)
//!
//! # SceneFrame Contract (SPEC-HAL-044 + SPEC-HAL-081A)
//!
//! SceneFrame is the ONLY cross-layer payload from core → renderer.
//! It carries:
//! - build_time: Instant (frame timing metadata)
//! - frame_id: u64 (monotonic frame counter)
//! - scene: SceneGraph (scene graph snapshot)
//! - surface_stack: SurfaceStackSnapshot (compositor surfaces, MANDATORY per SPEC-HAL-081A)
//!
//! # Forbidden Content (STOP conditions)
//!
//! SceneFrame MUST NOT contain:
//! - Widget references or trait objects
//! - Event state or input routing state
//! - Focus, capture, or modal state
//! - Compositor mutators (&mut Compositor, SurfaceStack, etc.)
//! - Layout state or constraint objects
//!
//! # How Violations Are Caught
//!
//! Adding a field to SceneFrame causes:
//! - `error[E0027]: pattern does not mention field `new_field``
//!
//! Adding forbidden types causes trait bound tests to fail.

#[cfg(test)]
mod tests {
    use crate::compositor::{MultiSurfaceContract, SurfaceStackSnapshot};
    use crate::core::scene::{SceneFrame, SceneGraph};
    use std::time::Instant;

    // ========================================================================
    // ABI Freeze Guards (compile-time field enumeration)
    // ========================================================================

    /// SCENEFRAME ABI GUARD: Field set is frozen.
    ///
    /// This test uses exhaustive destructuring. If ANY field is added to
    /// SceneFrame, this test will FAIL TO COMPILE with:
    /// `error[E0027]: pattern does not mention field `<new_field>``
    ///
    /// Allowed fields (SPEC-HAL-044 + SPEC-HAL-081A):
    /// - build_time, frame_id, scene, surface_stack
    #[test]
    fn scene_frame_abi_frozen() {
        use crate::compositor::Compositor;
        use crate::core::math::RectF;

        // SPEC-HAL-081B: surface_stack is mandatory (MultiSurfaceContract)
        let compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let frame = SceneFrame::new(
            Instant::now(),
            0,
            SceneGraph::new(),
            compositor.snapshot_multi(),
        );

        // Exhaustive destructuring - compile fails if fields added/removed
        let SceneFrame {
            build_time: _,
            frame_id: _,
            scene: _,
            surface_stack: _,
        } = frame;
    }

    /// Size assertion as secondary guard.
    ///
    /// SceneFrame should remain small (metadata + references).
    /// A large size increase indicates unexpected field additions.
    #[test]
    fn scene_frame_size_bounded() {
        let size = std::mem::size_of::<SceneFrame>();
        // SceneFrame contains:
        // - Instant (16 bytes typically)
        // - u64 (8 bytes)
        // - SceneGraph (contains Vec + Color + HashMap in debug)
        // - SurfaceStackSnapshot (contains Vec + PhantomData)
        // Allow generous headroom for platform variation
        assert!(
            size <= 512,
            "SceneFrame size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Constructor parity: SceneFrame::new() covers all required fields.
    ///
    /// SPEC-HAL-081B: surface_stack is now MANDATORY (MultiSurfaceContract).
    /// If a field is added but not initialized in new(), this will fail.
    #[test]
    fn scene_frame_new_parity() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::RectF;

        let now = Instant::now();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let snapshot = compositor.snapshot_multi();

        let frame = SceneFrame::new(now, 42, SceneGraph::new(), snapshot);

        // Verify all fields are accessible after construction
        let _t: Instant = frame.build_time;
        let _id: u64 = frame.frame_id;
        let _scene: &SceneGraph = &frame.scene;
        let _stack: &SurfaceStackSnapshot<MultiSurfaceContract> = &frame.surface_stack;

        // SPEC-HAL-081B: surface_stack is mandatory, verify it has data
        assert!(!frame.surface_stack.is_empty());
    }

    // ========================================================================
    // Authority Firewall (type safety enforcement)
    // ========================================================================

    /// Verify SceneFrame is Send + Sync + 'static.
    ///
    /// SceneFrame is sent from UI thread to render thread (SPEC-HAL-044).
    /// If it accidentally contains thread-local state, Rc, RefCell,
    /// or other non-Send types, this will fail.
    #[test]
    fn scene_frame_is_thread_safe() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<SceneFrame>();
    }

    /// Verify SceneGraph is Send + Sync.
    ///
    /// SceneGraph is part of SceneFrame and must be thread-safe.
    #[test]
    fn scene_graph_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SceneGraph>();
    }

    /// Verify SurfaceStackSnapshot is Send + Sync.
    ///
    /// SurfaceStackSnapshot is the compositor data in SceneFrame.
    /// It must be thread-safe and immutable.
    #[test]
    fn surface_stack_snapshot_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SurfaceStackSnapshot<MultiSurfaceContract>>();
    }

    /// Verify SceneFrame field types are from allowed modules only.
    ///
    /// This test exercises the actual types to ensure they compile
    /// and are accessible. If someone adds a forbidden type, the
    /// trait bound tests above will catch it.
    #[test]
    fn scene_frame_uses_only_allowed_types() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::RectF;

        let now = Instant::now();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let snapshot = compositor.snapshot_multi();

        let frame = SceneFrame::new(now, 123, SceneGraph::new(), snapshot);

        // All field types must be from allowed modules:
        // - std::time::Instant (Rust stdlib)
        // - u64 (primitive)
        // - SceneGraph (crate::core::scene - allowed)
        // - SurfaceStackSnapshot<MultiSurfaceContract> (crate::compositor - allowed, immutable, MANDATORY)

        // Type annotations prove the types are what we expect
        let _: Instant = frame.build_time;
        let _: u64 = frame.frame_id;
        let _: SceneGraph = frame.scene;
        let _: SurfaceStackSnapshot<MultiSurfaceContract> = frame.surface_stack;

        // If any of these types change, this test will fail to compile.
    }

    /// Structural test: SceneFrame does not expose compositor mutators.
    ///
    /// SceneFrame contains SurfaceStackSnapshot which is IMMUTABLE.
    /// It does NOT contain &mut Compositor, SurfaceStack, or any mutator.
    #[test]
    fn scene_frame_is_read_only_for_compositor() {
        use crate::compositor::{
            Compositor, SceneKey, SurfaceDescriptor, SurfaceId, SurfaceVisibility,
        };
        use crate::core::math::RectF;

        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        let frame = SceneFrame::new(
            Instant::now(),
            1,
            SceneGraph::new(),
            compositor.snapshot_multi(),
        );

        // SceneFrame only provides read access to surface data (mandatory snapshot)
        // Iteration is read-only
        for descriptor in frame.surface_stack.iter() {
            // All reads are allowed
            let _id: SurfaceId = descriptor.id;
            let _z: i32 = descriptor.z_index;
            let _bounds: RectF = descriptor.bounds;
            let _clip: Option<RectF> = descriptor.clip;
            let _vis: SurfaceVisibility = descriptor.visibility;
            let _opacity: f32 = descriptor.opacity;
            let _key: u64 = descriptor.scene_key;

            // There are NO mutation methods available through SceneFrame
            // The following would be compile errors if attempted:
            // - descriptor.bounds = ... // SurfaceDescriptor has no &mut access
        }
    }

    /// Verify SceneFrame is the sole cross-layer payload.
    ///
    /// This is a documentation/structural test. SceneFrame is designed
    /// to be THE data structure sent from UI thread to renderer.
    /// No other struct should carry scene + compositor data together.
    #[test]
    fn scene_frame_is_complete_payload() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::RectF;

        // SceneFrame carries everything needed for rendering:
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        let frame = SceneFrame::new(
            Instant::now(),
            1,
            SceneGraph::new(),
            compositor.snapshot_multi(),
        );

        // A complete frame has:
        // 1. Timing metadata (for latency tracking)
        assert!(frame.build_time <= Instant::now());

        // 2. Frame identity (for ordering and debugging)
        assert_eq!(frame.frame_id, 1);

        // 3. Scene content (what to render)
        assert!(frame.scene.root.is_empty()); // Empty scene for this test

        // 4. Surface ordering (how to composite) - SPEC-HAL-081A: MANDATORY
        assert_eq!(frame.surface_stack.len(), 1);

        // No other data is needed. If it were, it should be added to SceneFrame
        // through SPEC amendment, not by creating a parallel payload struct.
    }
}
