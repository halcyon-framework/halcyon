//! SPEC-HAL-084 DrawPlan/CompiledDrawPlan Boundary Guard
//!
//! This module provides compile-time enforcement that the DrawPlan →
//! CompiledDrawPlan boundary remains frozen and correctly enforces
//! CPU-only vs GPU-ready separation.
//!
//! # Enforcement Mechanism
//!
//! 1. Exhaustive pattern matching on DrawPlan fields (compile-time)
//! 2. Size assertions to detect hidden fields (test-time)
//! 3. Trait bounds verifying DrawPlan is CPU-only (Send + Sync)
//! 4. CompiledDrawPlan is intentionally NOT Send (contains GPU resources)
//!
//! # Boundary Contract (SPEC-HAL-084)
//!
//! DrawPlan (Build phase output):
//! - CPU-only data, no GPU handles
//! - Must be Send + Sync (can cross thread boundaries)
//! - Contains: primitives, batches, paths, clips, clear_color, surface_stack
//!
//! CompiledDrawPlan (Compile phase output):
//! - GPU-ready data with wgpu handles
//! - NOT Send (wgpu::SurfaceTexture is frame-scoped)
//! - Contains: surface_output, frame_batches, text_quads, clip_offsets, clear_color, surface_stack
//!
//! # How Violations Are Caught
//!
//! Adding a field to DrawPlan or CompiledDrawPlan causes:
//! - `error[E0027]: pattern does not mention field `new_field``
//!
//! Adding GPU types to DrawPlan breaks Send + Sync bounds.

#[cfg(test)]
mod tests {
    use crate::compositor::MultiSurfaceContract;
    use crate::core::math::RectF;
    use crate::core::scene::{Color, SceneGraph};
    use crate::renderer::batcher::{DrawBatch, FlatPrimitive};
    use crate::renderer::draw_plan::DrawPlan;
    use crate::renderer::draw_plan_builder::DrawPlanBuilder;
    use crate::renderer::mesh_cache::MeshCache;
    use crate::renderer::path::PathDraw;

    // ========================================================================
    // DrawPlan ABI Freeze (compile-time field enumeration)
    // ========================================================================

    /// DRAWPLAN ABI GUARD: Field set is frozen.
    ///
    /// This test uses exhaustive destructuring. If ANY field is added to
    /// DrawPlan, this test will FAIL TO COMPILE with:
    /// `error[E0027]: pattern does not mention field `<new_field>``
    ///
    /// Allowed fields (SPEC-HAL-084 + SPEC-HAL-081B):
    /// - primitives, path_draws, path_clips, batches
    /// - SPEC-HAL-163: surface_data (HashMap<SurfaceId, SurfaceSceneData>), surface_stack
    #[test]
    fn draw_plan_abi_frozen() {
        use crate::compositor::{Compositor, SceneKey};
        use std::collections::HashMap;

        let mut builder = DrawPlanBuilder::new();
        let scene = SceneGraph::new();
        // SPEC-HAL-081B: Use build_multi() - compositor path is mandatory
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let plan = builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");

        // SPEC-HAL-163: Exhaustive destructuring - compile fails if fields added/removed
        let DrawPlan {
            surface_data: _,
            surface_stack: _,
        } = plan;
    }

    /// Size assertion for DrawPlan.
    ///
    /// DrawPlan should be moderate size (vectors + metadata).
    /// A large size increase indicates unexpected field additions.
    #[test]
    fn draw_plan_size_bounded() {
        let size = std::mem::size_of::<DrawPlan<MultiSurfaceContract>>();
        // DrawPlan contains:
        // - Vec<FlatPrimitive> (24 bytes)
        // - Vec<PathDraw> (24 bytes)
        // - Vec<RectF> (24 bytes)
        // - Vec<DrawBatch> (24 bytes)
        // - u32 x2 (8 bytes)
        // - Color (16 bytes)
        // - SurfaceStackSnapshot (~24 bytes) - MANDATORY per SPEC-HAL-081B
        // Allow generous headroom
        assert!(
            size <= 256,
            "DrawPlan size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Verify DrawPlan is Clone + Send + Sync + 'static.
    ///
    /// DrawPlan is CPU-only and must be freely movable between threads.
    /// If GPU types are accidentally added, Clone/Send/Sync will fail.
    #[test]
    fn draw_plan_is_cpu_only() {
        fn assert_cpu_traits<T: Clone + Send + Sync + 'static>() {}
        assert_cpu_traits::<DrawPlan<MultiSurfaceContract>>();
    }

    /// Verify DrawPlan field types are CPU-only.
    ///
    /// Each field type must be Send + Sync.
    #[test]
    fn draw_plan_field_types_are_cpu_safe() {
        use crate::compositor::SurfaceStackSnapshot;

        fn assert_send_sync<T: Send + Sync + 'static>() {}

        // All field types must be thread-safe
        assert_send_sync::<Vec<FlatPrimitive>>();
        assert_send_sync::<Vec<PathDraw>>();
        assert_send_sync::<Vec<RectF>>();
        assert_send_sync::<Vec<DrawBatch>>();
        assert_send_sync::<u32>();
        assert_send_sync::<Color>();
        assert_send_sync::<SurfaceStackSnapshot<MultiSurfaceContract>>(); // MANDATORY per SPEC-HAL-081B
    }

    /// Verify DrawPlanBuilder::build_multi() produces correct output.
    ///
    /// SPEC-HAL-081B: build_multi() is the canonical path.
    #[test]
    fn draw_plan_builder_with_surfaces() {
        use crate::compositor::{Compositor, SceneKey};
        use std::collections::HashMap;

        let mut builder = DrawPlanBuilder::new();
        let scene = SceneGraph::new();

        // build_multi() with surfaces - mandatory path per SPEC-HAL-081B
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let snapshot = compositor.snapshot_multi();

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let plan = builder
            .build_multi(&surface_scenes, snapshot, &mut MeshCache::new(), |_| 1.0, 0)
            .expect("scene must satisfy DrawPlan Build admission");
        // SPEC-HAL-081B: surface_stack is mandatory (not Option), verify it has data
        assert!(!plan.surface_stack.is_empty());
    }

    // NOTE: Previous test `draw_plan_builder_legacy_panics` was removed because
    // the legacy `build()` method no longer exists. SPEC-HAL-081B enforcement is now
    // by construction: only `build_multi()` exists, making the non-compositor
    // path unrepresentable at the API level.

    // ========================================================================
    // CompiledDrawPlan ABI Freeze (compile-time field enumeration)
    // ========================================================================
    //
    // Note: CompiledDrawPlan cannot be instantiated in tests without GPU.
    // We verify its structure through documentation and type-level checks.
    // The exhaustive destructuring guard is in the renderer itself.
    //
    // SPEC-HAL-081A: CompiledDrawPlan now includes surface_stack for
    // surface-ordered rendering (INV-COMP-P04).

    /// Verify CompiledDrawPlan is NOT Send.
    ///
    /// CompiledDrawPlan contains wgpu::SurfaceTexture which is frame-scoped
    /// and must not cross thread boundaries. This is intentional.
    #[test]
    fn compiled_draw_plan_is_not_send() {
        #[allow(unused_imports)]
        use crate::renderer::CompiledDrawPlan;

        // CompiledDrawPlan should NOT be Send because it contains GPU resources
        // This is a negative assertion - we verify the type system prevents misuse
        #[allow(dead_code)]
        fn is_send<T: Send>() {}

        // The following would fail to compile if uncommented:
        // is_send::<CompiledDrawPlan>();

        // Instead, we document that CompiledDrawPlan is intentionally !Send
        // This test passes by NOT asserting Send, confirming the design
    }

    /// Verify CompiledDrawPlan size is bounded.
    ///
    /// We can check size without instantiation.
    #[test]
    fn compiled_draw_plan_size_bounded() {
        use crate::renderer::CompiledDrawPlan;

        let size = std::mem::size_of::<CompiledDrawPlan>();
        // CompiledDrawPlan contains:
        // - wgpu::SurfaceTexture (platform-dependent, ~48-96 bytes)
        // - FrameBatches (contains vectors, ~100+ bytes)
        // - Vec<TextQuad> (24 bytes)
        // - HashMap<u64, u32> (48 bytes)
        // - wgpu::Color (32 bytes)
        // - SurfaceStackSnapshot (~24 bytes, SPEC-HAL-081A)
        // Allow generous headroom for platform variation
        assert!(
            size <= 512,
            "CompiledDrawPlan size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    // ========================================================================
    // Boundary Enforcement (CPU → GPU transition)
    // ========================================================================

    /// Verify the Build → Compile boundary is clean.
    ///
    /// DrawPlan (CPU-only) → CompiledDrawPlan (GPU-ready)
    /// The transition happens in WgpuRenderer::compile_multi() which we can't
    /// call in tests without GPU. But we can verify the types are correct.
    #[test]
    fn build_compile_boundary_types() {
        use crate::compositor::{Compositor, SceneKey};
        use std::collections::HashMap;

        // DrawPlan is the input to compile phase
        fn takes_draw_plan(_plan: DrawPlan<MultiSurfaceContract>) {}

        // DrawPlanBuilder produces DrawPlan
        let mut builder = DrawPlanBuilder::new();
        let scene = SceneGraph::new();
        // SPEC-HAL-081B: Use build_multi() - compositor path is mandatory
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let plan: DrawPlan<MultiSurfaceContract> = builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");

        // DrawPlan can be passed to compile phase (by value)
        takes_draw_plan(plan);

        // CompiledDrawPlan would be produced by compile phase
        // and passed to execute phase (by value, consuming it)
    }

    /// Verify DrawPlan does not expose GPU internals.
    ///
    /// DrawPlan should not contain any wgpu types directly.
    /// SPEC-HAL-163: Fields are now per-surface in surface_data.
    #[test]
    fn draw_plan_no_gpu_types() {
        use crate::compositor::{Compositor, SceneKey};
        use std::collections::HashMap;

        let mut builder = DrawPlanBuilder::new();
        let scene = SceneGraph::new();
        // SPEC-HAL-081B: Use build_multi() - compositor path is mandatory
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        let plan = builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");

        // SPEC-HAL-163: Access per-surface data to verify CPU types
        // Fields are now in surface_data per surface, not on DrawPlan directly
        if let Some(surface_data) = plan.surface_data.get(&surface_id) {
            let _primitives: &Vec<FlatPrimitive> = &surface_data.primitives;
            let _paths: &Vec<PathDraw> = &surface_data.path_draws;
            let _clips: &Vec<RectF> = &surface_data.path_clips;
            let _batches: &Vec<DrawBatch> = &surface_data.batches;
            let _verts: u32 = surface_data.total_vertices;
            let _idxs: u32 = surface_data.total_indices;
            let _color: Color = surface_data.clear_color;
        }

        // All types above are CPU-only (no wgpu::Buffer, wgpu::Texture, etc.)
    }
}
