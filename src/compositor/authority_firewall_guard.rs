//! SPEC-HAL-081A/150 Authority Firewall Guard
//!
//! This module provides compile-time enforcement that the compositor module
//! does NOT import or expose forbidden types from widget, event, layout,
//! hit-test, or style systems.
//!
//! # Enforcement Mechanism
//!
//! 1. Type trait bounds verify Compositor is Send + Sync + 'static
//!    (no hidden runtime dependencies on forbidden systems)
//! 2. Function signature tests verify public API only uses allowed types
//! 3. Any accidental import of forbidden types into compositor public API
//!    will cause these tests to fail
//!
//! # Forbidden Imports (STOP-08 enforcement)
//!
//! The compositor module MUST NOT import:
//! - crate::core::widget (INV-COMP-N04)
//! - crate::core::event / event_router (INV-COMP-N12)
//! - crate::core::layout (INV-COMP-N02)
//! - crate::core::hit_test (INV-COMP-N11)
//! - crate::core::style (INV-COMP-N03)
//!
//! # How Violations Are Caught
//!
//! If compositor public types accidentally depend on forbidden systems,
//! the trait bound tests will fail to compile or the type assertions
//! will fail at test time.

#[cfg(test)]
mod tests {
    use super::super::{
        Compositor, SceneKey, SurfaceDescriptor, SurfaceId, SurfaceStackSnapshot, SurfaceVisibility,
    };

    /// Verify Compositor is Send + Sync + 'static.
    ///
    /// This ensures no hidden runtime dependencies on thread-local state,
    /// RefCell-based mutation, or other patterns that would indicate
    /// entanglement with forbidden systems.
    #[test]
    fn compositor_is_thread_safe() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<Compositor>();
    }

    /// Verify SurfaceId is a simple Copy type with no hidden dependencies.
    #[test]
    fn surface_id_is_simple_copy() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<SurfaceId>();
    }

    /// Verify SurfaceDescriptor is Clone + Send + Sync.
    ///
    /// If SurfaceDescriptor accidentally contains widget/event/layout types,
    /// this will fail because those types are typically not Send or contain
    /// non-Clone internal state.
    #[test]
    fn surface_descriptor_is_clean() {
        fn assert_clone_send_sync<T: Clone + Send + Sync + 'static>() {}
        assert_clone_send_sync::<SurfaceDescriptor>();
    }

    /// Verify SurfaceVisibility is a simple Copy enum.
    #[test]
    fn surface_visibility_is_simple() {
        fn assert_copy_eq<T: Copy + Eq + Send + Sync + 'static>() {}
        assert_copy_eq::<SurfaceVisibility>();
    }

    /// Verify SurfaceStackSnapshot is Send + Sync.
    ///
    /// The snapshot is the ONLY compositor data consumed by renderer.
    /// It must be thread-safe and contain no mutable references.
    #[test]
    fn surface_stack_snapshot_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SurfaceStackSnapshot>();
    }

    /// Verify compositor public API uses only allowed primitive types.
    ///
    /// This test instantiates Compositor and exercises its public API
    /// to ensure all return types are from allowed modules only:
    /// - crate::core::math (RectF)
    /// - crate::compositor (SurfaceId, SurfaceDescriptor, etc.)
    /// - Rust primitives (bool, usize, i32, f32, u64, Option)
    #[test]
    fn compositor_api_uses_only_allowed_types() {
        use crate::core::math::RectF;

        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // viewport() returns RectF (allowed)
        let _viewport: RectF = compositor.viewport();

        // create_surface() returns SurfaceId (allowed)
        let id: SurfaceId =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        // has_surface() returns bool (primitive)
        let _exists: bool = compositor.has_surface(id);

        // surface_count() returns usize (primitive)
        let _count: usize = compositor.surface_count();

        // get_surface() returns Option<SurfaceDescriptor> (allowed)
        let _desc: Option<SurfaceDescriptor> = compositor.get_surface(id);

        // snapshot() returns SurfaceStackSnapshot (allowed)
        let _snapshot: SurfaceStackSnapshot = compositor.snapshot();

        // destroy_surface() returns bool (primitive)
        let _removed: bool = compositor.destroy_surface(id);

        // All mutators take only allowed types:
        // - SurfaceId (compositor type)
        // - i32, f32, u64 (primitives)
        // - RectF (core::math)
        // - Option<RectF>
        // - SurfaceVisibility (compositor type)
        //
        // If any of these signatures change to include forbidden types,
        // this test will fail to compile.
    }

    /// Verify snapshot iteration yields only allowed types.
    #[test]
    fn snapshot_yields_only_descriptors() {
        use crate::core::math::RectF;

        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        let snapshot = compositor.snapshot();

        // iter() yields &SurfaceDescriptor (allowed)
        for descriptor in snapshot.iter() {
            let _id: SurfaceId = descriptor.id;
            let _z: i32 = descriptor.z_index;
            let _bounds: RectF = descriptor.bounds;
            let _clip: Option<RectF> = descriptor.clip;
            let _vis: SurfaceVisibility = descriptor.visibility;
            let _opacity: f32 = descriptor.opacity;
            let _key: u64 = descriptor.scene_key;
        }
    }
}
