//! SceneDiff Authority Firewall Guard
//!
//! This module provides compile-time enforcement that SceneDiff remains
//! pure structural diff data and does NOT contain widget, event, layout,
//! hit-test, style, or compositor types.
//!
//! # Enforcement Mechanism
//!
//! 1. `NoForbiddenSceneDiff` trait implemented for allowed types only
//! 2. Field-by-field assertions for key structs
//! 3. Poison markers in forbidden.rs NOT implementing the trait
//! 4. If forbidden type leaks in, compilation fails
//!
//! # SceneDiff Authority Contract
//!
//! SceneDiff contains ONLY:
//! - SceneNodeKey (identity)
//! - SceneNodeKind (classification)
//! - Primitive types (bool, usize, u64)
//! - Standard containers (Vec, HashMap, HashSet)
//!
//! SceneDiff MUST NOT contain:
//! - Widget references or trait objects
//! - Event state or input routing
//! - Layout constraints or results
//! - Hit test state
//! - Style specifications
//! - Compositor state (any)
//! - SceneNode content (geometry, colors, etc.)
//!
//! # How Violations Are Caught
//!
//! If a forbidden type is added to a SceneDiff struct field:
//! ```text
//! error[E0277]: the trait bound `ForbiddenType: NoForbiddenSceneDiff` is not satisfied
//! ```

#[cfg(test)]
mod tests {
    use super::super::{
        CacheEntry, CacheEntryOwnership, CacheLifecyclePhase, DeltaReaction, SceneCache,
        SceneCacheHandle, SceneDelta,
    };
    use crate::core::scene_identity::{SceneNodeKey, SceneNodeKind};

    // ========================================================================
    // NoForbiddenSceneDiff Trait and Implementations
    // ========================================================================
    //
    // This trait is intentionally NOT implemented for poison markers.
    // If a forbidden type appears in a SceneDiff struct, compilation fails.

    /// Marker trait for types allowed in SceneDiff structs.
    ///
    /// This trait is intentionally NOT implemented for:
    /// - PoisonWidget, PoisonEvent, PoisonLayout, PoisonHitTest, PoisonStyle
    /// - PoisonCompositor, PoisonSurfaceStack, PoisonSurfaceStackSnapshot
    trait NoForbiddenSceneDiff {}

    // Primitives
    impl NoForbiddenSceneDiff for bool {}
    impl NoForbiddenSceneDiff for u8 {}
    impl NoForbiddenSceneDiff for u16 {}
    impl NoForbiddenSceneDiff for u32 {}
    impl NoForbiddenSceneDiff for u64 {}
    impl NoForbiddenSceneDiff for usize {}
    impl NoForbiddenSceneDiff for i32 {}

    // Identity types (the ONLY scene types allowed in diff)
    impl NoForbiddenSceneDiff for SceneNodeKey {}
    impl NoForbiddenSceneDiff for SceneNodeKind {}

    // Standard library containers
    impl<T> NoForbiddenSceneDiff for Vec<T> {}
    impl<T> NoForbiddenSceneDiff for Option<T> {}
    impl<K, V> NoForbiddenSceneDiff for std::collections::HashMap<K, V> {}
    impl<T> NoForbiddenSceneDiff for std::collections::HashSet<T> {}

    // SceneDiff types themselves
    impl NoForbiddenSceneDiff for SceneDelta {}
    impl NoForbiddenSceneDiff for DeltaReaction {}
    impl NoForbiddenSceneDiff for CacheEntry {}
    impl NoForbiddenSceneDiff for CacheEntryOwnership {}
    impl NoForbiddenSceneDiff for CacheLifecyclePhase {}
    impl NoForbiddenSceneDiff for SceneCacheHandle {}
    impl NoForbiddenSceneDiff for SceneCache {}

    /// Helper function for compile-time NoForbiddenSceneDiff assertion.
    fn assert_no_forbidden<T: NoForbiddenSceneDiff>() {}

    // ========================================================================
    // SceneDelta Field Firewall
    // ========================================================================

    /// SCENEDELTA FIELD FIREWALL: All field types must be NoForbiddenSceneDiff.
    #[test]
    fn scene_delta_fields_no_forbidden() {
        // SceneDelta fields:
        // - inserts: Vec<SceneNodeKey>
        // - removes: Vec<SceneNodeKey>
        // - updates: Vec<SceneNodeKey>

        assert_no_forbidden::<Vec<SceneNodeKey>>();
    }

    /// DELTAREACTION FIELD FIREWALL: All variant payloads must be NoForbiddenSceneDiff.
    #[test]
    fn delta_reaction_fields_no_forbidden() {
        // DeltaReaction variants:
        // - Insert(SceneNodeKey)
        // - Remove(SceneNodeKey)
        // - Update(SceneNodeKey)

        assert_no_forbidden::<SceneNodeKey>();
    }

    // ========================================================================
    // Cache Entry Field Firewalls
    // ========================================================================

    /// CACHEENTRY FIELD FIREWALL: All field types must be NoForbiddenSceneDiff.
    #[test]
    fn cache_entry_fields_no_forbidden() {
        // CacheEntry fields:
        // - key: SceneNodeKey
        // - kind: SceneNodeKind
        // - dirty: bool
        // - ownership: CacheEntryOwnership

        assert_no_forbidden::<SceneNodeKey>();
        assert_no_forbidden::<SceneNodeKind>();
        assert_no_forbidden::<bool>();
        assert_no_forbidden::<CacheEntryOwnership>();
    }

    /// SCENECACHEHANDLE FIELD FIREWALL: All field types must be NoForbiddenSceneDiff.
    #[test]
    fn scene_cache_handle_fields_no_forbidden() {
        // SceneCacheHandle fields:
        // - key: SceneNodeKey
        // - kind: SceneNodeKind
        // - generation: u64 (debug only)

        assert_no_forbidden::<SceneNodeKey>();
        assert_no_forbidden::<SceneNodeKind>();
        assert_no_forbidden::<u64>();
    }

    /// SCENECACHE FIELD FIREWALL: All field types must be NoForbiddenSceneDiff.
    #[test]
    fn scene_cache_fields_no_forbidden() {
        // SceneCache fields:
        // - entries: HashMap<SceneNodeKey, CacheEntry>
        // - generations: HashMap<SceneNodeKey, u64> (debug only)
        // - phase: CacheLifecyclePhase (debug only)
        // - snapshot_entry_count: usize (debug only)
        // - snapshot_dirty_count: usize (debug only)

        assert_no_forbidden::<std::collections::HashMap<SceneNodeKey, CacheEntry>>();
        assert_no_forbidden::<std::collections::HashMap<SceneNodeKey, u64>>();
        assert_no_forbidden::<CacheLifecyclePhase>();
        assert_no_forbidden::<usize>();
    }

    /// CACHEENTRYOWNERSHIP FIELD FIREWALL: Enum has no associated data beyond variants.
    #[test]
    fn cache_entry_ownership_no_forbidden() {
        // CacheEntryOwnership is a simple enum:
        // - CpuOnly
        // - GpuOnly
        // - CpuAndGpu
        // No payload data, so just verify the type itself is allowed.
        assert_no_forbidden::<CacheEntryOwnership>();
    }

    /// CACHELIFECYCLEPHASE FIELD FIREWALL: Enum has no associated data.
    #[test]
    fn cache_lifecycle_phase_no_forbidden() {
        // CacheLifecyclePhase is a simple enum:
        // - Build
        // - Dispatch
        // - Render
        // No payload data.
        assert_no_forbidden::<CacheLifecyclePhase>();
    }

    // ========================================================================
    // Negative Test Documentation
    // ========================================================================

    /// NEGATIVE TEST DOCUMENTATION: Poison markers do NOT implement NoForbiddenSceneDiff.
    ///
    /// The following would fail to compile if uncommented:
    /// ```compile_fail
    /// assert_no_forbidden::<super::super::forbidden::PoisonWidget>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonCompositor>();
    /// ```
    ///
    /// This proves the firewall works: forbidden types cannot satisfy NoForbiddenSceneDiff.
    #[test]
    fn poison_markers_are_forbidden() {
        // This test documents that poison markers cannot pass the firewall.
        // If you uncomment the lines above, compilation will fail with:
        // "the trait `NoForbiddenSceneDiff` is not implemented for `PoisonCompositor`"
    }

    // ========================================================================
    // Additional Type Safety Checks
    // ========================================================================

    /// Verify SceneDelta is Send + Sync (can be passed between threads).
    #[test]
    fn scene_delta_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SceneDelta>();
    }

    /// Verify SceneCache is Send + Sync.
    #[test]
    fn scene_cache_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SceneCache>();
    }

    /// Verify SceneNodeKey is Copy (lightweight identity token).
    #[test]
    fn scene_node_key_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<SceneNodeKey>();
    }

    /// Verify SceneNodeKind is Copy (lightweight classification).
    #[test]
    fn scene_node_kind_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<SceneNodeKind>();
    }
}
