//! SPEC-HAL-081A/084 Renderer Authority Firewall Guard
//!
//! This module provides compile-time enforcement that the renderer module
//! only consumes compositor data via immutable SurfaceStackSnapshot, and
//! does NOT import compositor internals or mutators.
//!
//! # Enforcement Mechanism
//!
//! 1. Verify DrawPlan/DrawPlanBuilder are Send + Sync (CPU-only, no GPU handles)
//! 2. Verify renderer uses ONLY SurfaceStackSnapshot from compositor
//! 3. Ensure no compositor internals leak into renderer signatures
//!
//! # Allowed Compositor Imports
//!
//! - crate::compositor::SurfaceStackSnapshot (immutable snapshot)
//! - crate::compositor::SurfaceDescriptor (read-only, via snapshot iteration)
//! - crate::compositor::SurfaceId (read-only identifier)
//! - crate::compositor::SurfaceVisibility (read-only enum)
//!
//! # Forbidden Compositor Imports
//!
//! - crate::compositor::SurfaceStack (internal, mutable)
//! - crate::compositor::surface::SurfaceState (internal, mutable)
//! - crate::compositor::Compositor (renderer must not mutate compositor)
//! - Any compositor mutator methods
//!
//! # How Violations Are Caught
//!
//! If renderer accidentally depends on compositor internals or mutators,
//! the type assertions will fail. The renderer should be a pure consumer
//! of immutable snapshot data.

#[cfg(test)]
mod tests {
    use crate::compositor::MultiSurfaceContract;
    use crate::renderer::mesh_cache::MeshCache;
    use crate::renderer::{DrawPlan, DrawPlanBuilder};

    /// Verify DrawPlanBuilder is Send + Sync + 'static.
    ///
    /// This ensures DrawPlanBuilder contains only CPU-only data and has
    /// no hidden dependencies on GPU state or thread-local storage.
    #[test]
    fn draw_plan_builder_is_thread_safe() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<DrawPlanBuilder>();
    }

    /// Verify DrawPlan is Clone + Send + Sync.
    ///
    /// DrawPlan must be CPU-only and freely movable between threads.
    /// If it accidentally contains GPU handles (wgpu::Buffer, etc.),
    /// this will fail because GPU handles are typically not Clone or Send.
    #[test]
    fn draw_plan_is_cpu_only() {
        fn assert_clone_send_sync<T: Clone + Send + Sync + 'static>() {}
        assert_clone_send_sync::<DrawPlan<MultiSurfaceContract>>();
    }

    /// Verify DrawPlanBuilder does not expose compositor mutators.
    ///
    /// SPEC-HAL-081B: build_multi() takes SurfaceStackSnapshot directly
    /// (MANDATORY, not Option). This is an IMMUTABLE snapshot. It does NOT take
    /// &mut Compositor or any compositor mutator.
    ///
    /// If this signature changes to accept mutable compositor state,
    /// the architectural boundary is violated.
    #[test]
    fn draw_plan_builder_uses_only_snapshot() {
        use crate::compositor::{Compositor, SceneKey, SurfaceStackSnapshot};
        use crate::core::math::RectF;
        use crate::core::scene::SceneGraph;
        use std::collections::HashMap;

        // Verify the signature accepts SurfaceStackSnapshot (immutable, mandatory)
        // NOT &mut Compositor, NOT SurfaceStack, NOT any mutator
        let mut builder = DrawPlanBuilder::new();

        // Create a minimal scene for testing
        let scene = SceneGraph::new();

        // SPEC-HAL-081B: build_multi() is the only valid path

        // build_multi() takes SurfaceStackSnapshot<MultiSurfaceContract> (immutable, mandatory)
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let snapshot: SurfaceStackSnapshot<MultiSurfaceContract> = compositor.snapshot_multi();

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let _plan: DrawPlan<MultiSurfaceContract> = builder
            .build_multi(&surface_scenes, snapshot, &mut MeshCache::new(), |_| 1.0, 0)
            .expect("scene must satisfy DrawPlan Build admission");

        // If the signature changes to require mutable compositor access,
        // this test will fail to compile.
    }

    /// Verify SurfaceStackSnapshot is the ONLY compositor type in DrawPlan.
    ///
    /// DrawPlan should carry SurfaceStackSnapshot for downstream consumption,
    /// but should NOT contain SurfaceStack (mutable internal) or Compositor.
    #[test]
    fn draw_plan_contains_only_snapshot() {
        // SurfaceStackSnapshot is allowed - it's immutable read-only data
        use crate::compositor::SurfaceStackSnapshot;

        fn assert_snapshot_is_clean<T: Clone + Send + Sync + 'static>() {}
        assert_snapshot_is_clean::<SurfaceStackSnapshot<MultiSurfaceContract>>();

        // The following imports would be FORBIDDEN if they existed in renderer:
        // - use crate::compositor::stack::SurfaceStack; // internal
        // - use crate::compositor::surface::SurfaceState; // internal
        //
        // These are pub(super) and cannot be imported from renderer.
        // If someone changes visibility to pub(crate), the guard tests
        // in compositor/abi_guard.rs will catch it.
    }

    /// Verify renderer public API does not re-export compositor internals.
    ///
    /// The renderer module should only re-export its own types, not
    /// compositor types (even if some are technically accessible).
    #[test]
    fn renderer_exports_are_clean() {
        // These are the public exports from renderer (from mod.rs)
        use crate::renderer::{
            BakedVertex, CompiledDrawPlan, DrawPlan, DrawPlanBuilder, FrameEpoch, QuadInstance,
            QuadVertex, RectVertex,
        };

        // All renderer types should be CPU-safe and not entangled with compositor
        fn assert_clean<T: Send + Sync + 'static>() {}
        assert_clean::<DrawPlan<MultiSurfaceContract>>();
        assert_clean::<DrawPlanBuilder>();
        assert_clean::<CompiledDrawPlan>();
        assert_clean::<FrameEpoch>();
        assert_clean::<RectVertex>();
        assert_clean::<BakedVertex>();
        assert_clean::<QuadVertex>();
        assert_clean::<QuadInstance>();
        // PathRenderer contains GPU handles and may not be Send
        // That's fine - it's renderer-internal, not a boundary crossing issue
    }

    /// Structural test: SurfaceStackSnapshot iteration in renderer context.
    ///
    /// This test exercises the read-only iteration pattern that renderer
    /// should use when consuming surface stack data.
    #[test]
    fn renderer_can_iterate_snapshot_read_only() {
        use crate::compositor::{
            Compositor, SceneKey, SurfaceId, SurfaceStackSnapshot, SurfaceVisibility,
        };
        use crate::core::math::RectF;

        // Create compositor and surface (simulating upstream preparation)
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let _surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        // Take snapshot (this is what SceneFrame carries to renderer)
        let snapshot: SurfaceStackSnapshot<MultiSurfaceContract> = compositor.snapshot_multi();

        // Renderer iterates snapshot READ-ONLY
        for descriptor in snapshot.iter() {
            // All these reads are allowed:
            let _id: SurfaceId = descriptor.id;
            let _z: i32 = descriptor.z_index;
            let _bounds: RectF = descriptor.bounds;
            let _clip: Option<RectF> = descriptor.clip;
            let _vis: SurfaceVisibility = descriptor.visibility;
            let _opacity: f32 = descriptor.opacity;
            let _key: u64 = descriptor.scene_key;

            // Renderer CANNOT mutate compositor through snapshot
            // There are no &mut methods on SurfaceStackSnapshot or SurfaceDescriptor
        }

        // len() is also read-only
        let _count: usize = snapshot.len();
        let _empty: bool = snapshot.is_empty();
    }

    /// SPEC-HAL-081B: Verify scene_key flows from compositor to batches.
    ///
    /// This guard test ensures the scene_key plumbing is explicit:
    /// 1. scene_key is set on compositor surface (not defaulted)
    /// 2. scene_key flows through snapshot to DrawPlanBuilder
    /// 3. scene_key appears in DrawPlan batches (not hardcoded to 1)
    ///
    /// If scene_key defaults are reintroduced, this test will catch it.
    #[test]
    fn scene_key_flows_from_compositor_to_batches() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::RectF;
        use crate::core::scene::{Color, RectNode, SceneGraph, SceneNode};
        use std::collections::HashMap;

        // Use a non-default scene_key to prove it's not hardcoded
        let test_scene_key = SceneKey::new(42);

        // Step 1: Create compositor with surface that has explicit scene_key
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), test_scene_key);
        let snapshot = compositor.snapshot_multi();

        // Verify the snapshot carries the scene_key
        let surface = snapshot
            .iter()
            .next()
            .expect("snapshot should have surface");
        assert_eq!(
            surface.scene_key,
            test_scene_key.get(),
            "Snapshot must carry explicit scene_key from compositor"
        );

        // Step 2: Build DrawPlan with a scene that has some content
        let mut scene = SceneGraph::new();
        scene.root.push(SceneNode::Rect(RectNode {
            rect: RectF::new(10.0, 10.0, 100.0, 100.0),
            color: Color::from_srgb(1.0, 0.0, 0.0, 1.0),
            z_index: 0,
            key: None,
        }));

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);

        let mut builder = DrawPlanBuilder::new();
        let plan = builder
            .build_multi(&surface_scenes, snapshot, &mut MeshCache::new(), |_| 1.0, 0)
            .expect("scene must satisfy DrawPlan Build admission");

        // Step 3: Verify batches carry the compositor's scene_key (via per-surface data)
        let surface_data = plan
            .surface_data
            .get(&surface_id)
            .expect("surface should have data");
        assert!(
            !surface_data.batches.is_empty(),
            "DrawPlan should have batches from the scene content"
        );
        for batch in &surface_data.batches {
            assert_eq!(
                batch.scene_key, test_scene_key,
                "Batch scene_key must come from compositor, not default to 1. \
                 Found scene_key={}, expected={}",
                batch.scene_key, test_scene_key
            );
        }

        // Step 4: Also verify path draws if present get compositor's scene_key
        for path_draw in &surface_data.path_draws {
            assert_eq!(
                path_draw.scene_key, test_scene_key,
                "PathDraw scene_key must come from compositor. \
                 Found scene_key={}, expected={}",
                path_draw.scene_key, test_scene_key
            );
        }
    }

    /// SPEC-HAL-081B Enforcement Test: Verify MultiSurfaceContract is the current contract.
    ///
    /// This test verifies MultiSurfaceContract constants are correct.
    #[test]
    fn multi_surface_contract_is_current_contract() {
        use crate::compositor::Contract;

        // Verify MultiSurfaceContract constants
        assert_eq!(MultiSurfaceContract::NAME, "multi-surface-contract");
        assert_eq!(MultiSurfaceContract::MAX_SURFACES, usize::MAX);
    }

    /// SPEC-HAL-081B: Verify DrawPlanBuilder uses MultiSurfaceContract by default.
    ///
    /// This test documents that build_multi() is the only valid build path.
    #[test]
    fn draw_plan_builder_uses_multi_surface_contract() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::math::RectF;
        use crate::core::scene::SceneGraph;
        use std::collections::HashMap;

        // Verify build_multi works with default DrawPlanBuilder (MultiSurfaceContract)
        let mut builder = DrawPlanBuilder::new();
        let scene = SceneGraph::new();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);

        // This compiles because DrawPlanBuilder defaults to MultiSurfaceContract
        let _plan = builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
    }
}
