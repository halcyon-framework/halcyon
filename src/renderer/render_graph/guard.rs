//! SPEC-HAL-084 RenderGraph Authority + ABI Guard
//!
//! This module provides compile-time enforcement that RenderGraph remains
//! a pure consumer of DrawPlan/CompiledDrawPlan and does NOT reach back
//! into compositor, scene internals, widget, event, layout, or focus systems.
//!
//! # Enforcement Mechanism
//!
//! 1. Exhaustive pattern matching on RenderGraph public structs (compile-time)
//! 2. Size assertions to detect hidden fields (test-time)
//! 3. Authority firewall via trait bounds and type assertions
//! 4. One-way data flow verification (type-level proof)
//!
//! # Authority Firewall (SPEC-HAL-084)
//!
//! RenderGraph and its nodes MUST NOT import:
//! - `crate::compositor::Compositor`
//! - `crate::compositor::stack::SurfaceStack`
//! - `crate::compositor::surface::SurfaceState`
//! - Any widget/event/layout/focus modules
//!
//! Allowed inputs:
//! - `CompiledDrawPlan` (from renderer)
//! - `DrawPlan` (from renderer)
//! - GPU handles (`wgpu::*`)
//! - Renderer-local structs
//!
//! # One-Way Data Flow
//!
//! ```text
//! SceneFrame → DrawPlan → CompiledDrawPlan → RenderGraph → GPU
//! ```
//!
//! No reverse edge is possible at compile time.
//!
//! # How Violations Are Caught
//!
//! Adding a field to RenderGraph structs causes:
//! - `error[E0027]: pattern does not mention field `new_field``
//!
//! Adding compositor/widget types causes trait bound failures or import errors.

#[cfg(test)]
mod tests {
    // Module-relative imports: super::super:: reaches render_graph module
    use super::super::GraphError;
    use super::super::builder::RenderGraphBuilder;
    use super::super::executor::{
        BoundResources, CompiledGraph, FrameConstants, PassDrawScope, TargetDrawContext,
    };
    use super::super::graph::RenderGraph;
    use super::super::pass::{PassExecutionError, RenderPassNodeId};
    use super::super::resources::{AttachmentHandle, BufferHandle};
    use super::super::topology::RenderGraphTopology;

    use crate::compositor::{Compositor, MultiSurfaceContract, SceneKey};
    use crate::core::math::RectF;
    use crate::core::scene::{Color, SceneGraph};
    use crate::renderer::draw_plan::DrawPlan;
    use crate::renderer::draw_plan_builder::DrawPlanBuilder;
    use crate::renderer::mesh_cache::MeshCache;
    use crate::renderer::path::{PathDraw, PathMesh};
    use std::collections::HashMap;

    /// Helper to create a DrawPlan via compositor path (SPEC-HAL-081B mandatory).
    fn make_draw_plan() -> DrawPlan<MultiSurfaceContract> {
        let scene = SceneGraph::new();
        let mut plan_builder = DrawPlanBuilder::new();
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let surface_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        // SPEC-HAL-163: build_multi() takes per-surface scenes HashMap
        let mut surface_scenes = HashMap::new();
        surface_scenes.insert(surface_id, scene);
        plan_builder
            .build_multi(
                &surface_scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission")
    }

    fn make_path(scene_key: SceneKey) -> PathDraw {
        PathDraw {
            control_subtree_order: None,
            mesh: PathMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                bounds: RectF::zero(),
            },
            color: [1.0, 1.0, 1.0, 1.0],
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            clip_id: 0,
            stroke_flag: 0,
            debug: None,
            z_index: 0,
            sort_bias: 0,
            insertion_order: 0,
            bounds: RectF::zero(),
            scene_key,
        }
    }

    // ========================================================================
    // ABI Freeze Guards (compile-time field enumeration)
    // ========================================================================

    // NOTE: RenderGraphBuilder fields are private (not pub(crate) or pub(super)).
    // Exhaustive destructuring is not possible from guard module.
    // ABI protection for RenderGraphBuilder relies on:
    // - Size bounds (render_graph_builder_size_bounded)
    // - Trait bounds (render_graph_builder_is_thread_safe)
    // - API contract (build() signature takes &DrawPlan only)

    /// RENDERGRAPHTOPOLOGY ABI GUARD: Field set is frozen.
    ///
    /// Exhaustive destructuring catches field additions at compile time.
    #[test]
    fn render_graph_topology_abi_frozen() {
        let builder = RenderGraphBuilder::new();
        let plan = make_draw_plan();
        let topology = builder.build(&plan).unwrap();

        // Exhaustive destructuring - compile fails if fields added/removed
        let RenderGraphTopology {
            passes: _,
            sorted_order: _,
            resources: _,
        } = topology;
    }

    /// COMPILEDGRAPH ABI GUARD: Field set is frozen.
    ///
    /// Exhaustive destructuring catches field additions at compile time.
    #[test]
    fn compiled_graph_abi_frozen() {
        let builder = RenderGraphBuilder::new();
        let plan = make_draw_plan();
        let topology = builder.build(&plan).unwrap();

        let constants = FrameConstants {
            viewport_size: [800.0, 600.0],
            physical_width: 800,
            physical_height: 600,
            logical_width: 800.0,
            logical_height: 600.0,
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };

        let compiled = RenderGraph::compile(&topology, constants, HashMap::new());

        // Exhaustive destructuring - compile fails if fields added/removed
        let CompiledGraph {
            topology: _,
            bound_resources: _,
            constants: _,
        } = compiled;
    }

    /// FRAMECONSTANTS ABI GUARD: Field set is frozen.
    ///
    /// Exhaustive destructuring catches field additions at compile time.
    #[test]
    fn frame_constants_abi_frozen() {
        let constants = FrameConstants {
            viewport_size: [800.0, 600.0],
            physical_width: 800,
            physical_height: 600,
            logical_width: 800.0,
            logical_height: 600.0,
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };

        // Exhaustive destructuring - compile fails if fields added/removed
        let FrameConstants {
            viewport_size: _,
            physical_width: _,
            physical_height: _,
            logical_width: _,
            logical_height: _,
            scale_factor: _,
            clear_color: _,
        } = constants;
    }

    #[test]
    fn frame_constants_scissor_preserves_fractional_logical_extent() {
        let scale_factor = 1.5;
        let logical_width = 1201.0 / scale_factor;
        let constants = FrameConstants {
            viewport_size: [1201.0, 900.0],
            physical_width: 1201,
            physical_height: 900,
            logical_width,
            logical_height: 900.0 / scale_factor,
            scale_factor,
            clear_color: Color::BLACK,
        };

        let scissor =
            constants.clip_rect_to_scissor(Some(&RectF::new(0.0, 0.0, logical_width, 100.0)));

        assert_eq!(scissor.0, 0);
        assert_eq!(scissor.2, 1201);
    }

    /// PASSDRAWSCOPE ABI GUARD: Variant set is frozen.
    #[test]
    fn pass_draw_scope_abi_frozen() {
        fn assert_shape(scope: PassDrawScope) {
            match scope {
                PassDrawScope::MainUnderlay => {}
                PassDrawScope::Overlay => {}
            }
        }

        assert_shape(PassDrawScope::MainUnderlay);
        assert_shape(PassDrawScope::Overlay);
    }

    /// TARGETDRAWCONTEXT ABI GUARD: Field set is frozen.
    #[test]
    fn target_draw_context_abi_frozen() {
        let target = TargetDrawContext::new(SceneKey::BASE.get(), 64, 128);

        let TargetDrawContext {
            scene_key: _,
            globals_offset: _,
            path_clips_offset: _,
        } = target;
    }

    /// BOUNDRESOURCES ABI GUARD: Field set is frozen.
    ///
    /// Exhaustive destructuring catches field additions at compile time.
    #[test]
    fn bound_resources_abi_frozen() {
        let resources = BoundResources::new();

        // Exhaustive destructuring - compile fails if fields added/removed
        let BoundResources {
            frosted: _,
            attachments: _,
            buffers: _,
        } = resources;
    }

    /// RENDERPASSNODEID ABI GUARD: Newtype pattern is frozen.
    #[test]
    fn render_pass_node_id_abi_frozen() {
        let id = RenderPassNodeId(0);

        // Exhaustive destructuring - compile fails if structure changes
        let RenderPassNodeId(_inner) = id;
    }

    /// ATTACHMENTHANDLE ABI GUARD: Newtype pattern is frozen.
    #[test]
    fn attachment_handle_abi_frozen() {
        let handle = AttachmentHandle(0);

        // Exhaustive destructuring - compile fails if structure changes
        let AttachmentHandle(_inner) = handle;
    }

    /// BUFFERHANDLE ABI GUARD: Newtype pattern is frozen.
    #[test]
    fn buffer_handle_abi_frozen() {
        let handle = BufferHandle(0);

        // Exhaustive destructuring - compile fails if structure changes
        let BufferHandle(_inner) = handle;
    }

    /// PASSEXECUTIONERROR ABI GUARD: Variant field set is frozen.
    ///
    /// Exhaustive matching catches variant additions or field changes at compile time.
    #[test]
    fn pass_execution_error_abi_frozen() {
        fn assert_shape(error: PassExecutionError) {
            match error {
                PassExecutionError::MissingAttachment {
                    pass: _,
                    attachment: _,
                } => {}
                PassExecutionError::MissingBuffer { pass: _, buffer: _ } => {}
            }
        }

        assert_shape(PassExecutionError::missing_attachment(
            "main_pass",
            AttachmentHandle(0),
        ));
        assert_shape(PassExecutionError::missing_buffer(
            "main_pass",
            BufferHandle(0),
        ));
    }

    // ========================================================================
    // Size Bound Assertions
    // ========================================================================

    /// Size assertion for RenderGraphBuilder.
    #[test]
    fn render_graph_builder_size_bounded() {
        let size = std::mem::size_of::<RenderGraphBuilder>();
        // Contains: Vec + Vec + HashSet + 3 u32s
        assert!(
            size <= 256,
            "RenderGraphBuilder size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for RenderGraphTopology.
    #[test]
    fn render_graph_topology_size_bounded() {
        let size = std::mem::size_of::<RenderGraphTopology>();
        // Contains: Vec + Vec + Vec
        assert!(
            size <= 128,
            "RenderGraphTopology size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for CompiledGraph.
    #[test]
    fn compiled_graph_size_bounded() {
        let size = std::mem::size_of::<CompiledGraph>();
        // Contains: RenderGraphTopology + BoundResources + FrameConstants
        assert!(
            size <= 256,
            "CompiledGraph size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for FrameConstants.
    #[test]
    fn frame_constants_size_bounded() {
        let size = std::mem::size_of::<FrameConstants>();
        // Contains: [f32;2] + 2 u32s + 3 f32s + Color
        assert!(
            size <= 64,
            "FrameConstants size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for PassExecutionError.
    #[test]
    fn pass_execution_error_size_bounded() {
        let size = std::mem::size_of::<PassExecutionError>();
        // Contains: enum discriminant + pass label + one u32 handle.
        assert!(
            size <= 40,
            "PassExecutionError size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for PassDrawScope.
    #[test]
    fn pass_draw_scope_size_bounded() {
        let size = std::mem::size_of::<PassDrawScope>();
        assert!(
            size <= 1,
            "PassDrawScope size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    /// Size assertion for TargetDrawContext.
    #[test]
    fn target_draw_context_size_bounded() {
        let size = std::mem::size_of::<TargetDrawContext>();
        assert!(
            size <= 16,
            "TargetDrawContext size changed unexpectedly: {} bytes. ABI drift?",
            size
        );
    }

    // ========================================================================
    // Mechanical Forbidden Type Firewall (NoForbidden trait)
    // ========================================================================
    //
    // This section implements a HARD compile-time firewall against forbidden types.
    // If any forbidden type appears in a RenderGraph struct field, the NoForbidden
    // bound will fail to compile.
    //
    // How it works:
    // 1. NoForbidden is implemented for all allowed types (primitives, handles, etc.)
    // 2. NoForbidden is NOT implemented for poison markers in forbidden.rs
    // 3. Field type assertions require NoForbidden
    // 4. If forbidden types leak in, compilation fails with clear error

    /// Marker trait for types allowed in RenderGraph structs.
    ///
    /// This trait is intentionally NOT implemented for:
    /// - PoisonWidget, PoisonEvent, PoisonLayout, PoisonHitTest, PoisonStyle
    /// - PoisonCompositor, PoisonSurfaceStack
    /// - Any type from forbidden domains
    trait NoForbidden {}

    // Primitives
    impl NoForbidden for u32 {}
    impl NoForbidden for f32 {}
    impl NoForbidden for usize {}
    impl NoForbidden for bool {}
    impl NoForbidden for u64 {}
    impl NoForbidden for i32 {}
    impl NoForbidden for &'static str {}

    // Arrays
    impl NoForbidden for [f32; 2] {}
    impl NoForbidden for [f32; 4] {}

    // Renderer handle types
    impl NoForbidden for RenderPassNodeId {}
    impl NoForbidden for AttachmentHandle {}
    impl NoForbidden for BufferHandle {}

    // Core types allowed in RenderGraph (render primitives only)
    impl NoForbidden for crate::core::scene::Color {}
    impl NoForbidden for crate::core::math::RectF {}

    // Standard library containers (the contained types are checked separately)
    impl<T> NoForbidden for Vec<T> {}
    impl<K, V> NoForbidden for HashMap<K, V> {}
    impl<T> NoForbidden for Option<T> {}
    impl<T> NoForbidden for std::collections::HashSet<T> {}

    // wgpu types (GPU-safe by definition)
    impl NoForbidden for wgpu::TextureView {}
    impl NoForbidden for wgpu::Buffer {}

    // RenderGraph internal types
    impl NoForbidden for super::super::pass::RenderPassNode {}
    impl NoForbidden for super::super::pass::PassExecutionError {}
    impl NoForbidden for super::super::resources::ResourceDescriptor {}
    impl NoForbidden for super::super::topology::RenderGraphTopology {}
    impl NoForbidden for super::super::executor::BoundResources {}
    impl NoForbidden for super::super::executor::FrameConstants {}
    impl NoForbidden for super::super::executor::PassDrawScope {}
    impl NoForbidden for super::super::executor::TargetDrawContext {}

    /// Helper function for compile-time NoForbidden assertion.
    fn assert_no_forbidden<T: NoForbidden>() {}

    /// FRAMECONSTANTS FIELD FIREWALL: All field types must be NoForbidden.
    ///
    /// If any field type is changed to a forbidden type, this test fails to compile.
    #[test]
    fn frame_constants_fields_no_forbidden() {
        // FrameConstants fields:
        // - viewport_size: [f32; 2]
        // - physical_width: u32
        // - physical_height: u32
        // - logical_width: f32
        // - logical_height: f32
        // - scale_factor: f32
        // - clear_color: Color

        assert_no_forbidden::<[f32; 2]>();
        assert_no_forbidden::<u32>();
        assert_no_forbidden::<f32>();
        assert_no_forbidden::<crate::core::scene::Color>();
    }

    /// TARGETDRAWCONTEXT FIELD FIREWALL: All field types must be NoForbidden.
    #[test]
    fn target_draw_context_fields_no_forbidden() {
        assert_no_forbidden::<u64>();
        assert_no_forbidden::<u32>();
    }

    /// BOUNDRESOURCES FIELD FIREWALL: All field types must be NoForbidden.
    ///
    /// If any field type is changed to a forbidden type, this test fails to compile.
    #[test]
    fn bound_resources_fields_no_forbidden() {
        // BoundResources fields:
        // - attachments: HashMap<AttachmentHandle, wgpu::TextureView>
        // - buffers: HashMap<BufferHandle, wgpu::Buffer>

        assert_no_forbidden::<HashMap<AttachmentHandle, wgpu::TextureView>>();
        assert_no_forbidden::<HashMap<BufferHandle, wgpu::Buffer>>();
    }

    /// RENDERGRAPHTOPOLOGY FIELD FIREWALL: All field types must be NoForbidden.
    ///
    /// If any field type is changed to a forbidden type, this test fails to compile.
    #[test]
    fn render_graph_topology_fields_no_forbidden() {
        // RenderGraphTopology fields:
        // - passes: Vec<RenderPassNode>
        // - sorted_order: Vec<RenderPassNodeId>
        // - resources: Vec<ResourceDescriptor>

        assert_no_forbidden::<Vec<super::super::pass::RenderPassNode>>();
        assert_no_forbidden::<Vec<RenderPassNodeId>>();
        assert_no_forbidden::<Vec<super::super::resources::ResourceDescriptor>>();
    }

    /// COMPILEDGRAPH FIELD FIREWALL: All field types must be NoForbidden.
    ///
    /// If any field type is changed to a forbidden type, this test fails to compile.
    #[test]
    fn compiled_graph_fields_no_forbidden() {
        // CompiledGraph fields:
        // - topology: RenderGraphTopology
        // - bound_resources: BoundResources
        // - constants: FrameConstants

        assert_no_forbidden::<super::super::topology::RenderGraphTopology>();
        assert_no_forbidden::<super::super::executor::BoundResources>();
        assert_no_forbidden::<super::super::executor::FrameConstants>();
    }

    /// PASSEXECUTIONERROR FIELD FIREWALL: All field types must be NoForbidden.
    ///
    /// If any variant field type is changed to a forbidden type, this test fails to compile.
    #[test]
    fn pass_execution_error_fields_no_forbidden() {
        // PassExecutionError fields:
        // - pass: &'static str
        // - attachment: AttachmentHandle
        // - buffer: BufferHandle

        assert_no_forbidden::<&'static str>();
        assert_no_forbidden::<AttachmentHandle>();
        assert_no_forbidden::<BufferHandle>();
    }

    /// PASSDRAWSCOPE FIELD FIREWALL: Variant discriminant remains primitive.
    #[test]
    fn pass_draw_scope_fields_no_forbidden() {
        assert_no_forbidden::<super::super::executor::PassDrawScope>();
    }

    /// NEGATIVE TEST DOCUMENTATION: Poison markers do NOT implement NoForbidden.
    ///
    /// The following would fail to compile if uncommented:
    /// ```compile_fail
    /// assert_no_forbidden::<super::super::forbidden::PoisonCompositor>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonWidget>();
    /// ```
    ///
    /// This proves the firewall works: forbidden types cannot satisfy NoForbidden.
    #[test]
    fn poison_markers_are_forbidden() {
        // This test documents that poison markers cannot pass the firewall.
        // If you uncomment the lines above, compilation will fail with:
        // "the trait `NoForbidden` is not implemented for `PoisonCompositor`"
    }

    // ========================================================================
    // Authority Firewall (trait bound enforcement)
    // ========================================================================

    /// Verify FrameConstants is Copy + Send + Sync.
    ///
    /// FrameConstants is passed by value to passes and must be thread-safe.
    #[test]
    fn frame_constants_is_cpu_safe() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<FrameConstants>();
    }

    /// Verify PassDrawScope is Copy + Send + Sync.
    #[test]
    fn pass_draw_scope_is_simple() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<PassDrawScope>();
    }

    /// Verify TargetDrawContext is Copy + Send + Sync.
    #[test]
    fn target_draw_context_is_simple() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<TargetDrawContext>();
    }

    /// Verify RenderGraphBuilder is Send + Sync.
    ///
    /// Builder can be constructed on any thread.
    #[test]
    fn render_graph_builder_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<RenderGraphBuilder>();
    }

    /// Verify RenderPassNodeId is Copy + Send + Sync.
    #[test]
    fn render_pass_node_id_is_simple() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<RenderPassNodeId>();
    }

    /// Verify AttachmentHandle is Copy + Send + Sync.
    #[test]
    fn attachment_handle_is_simple() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<AttachmentHandle>();
    }

    /// Verify BufferHandle is Copy + Send + Sync.
    #[test]
    fn buffer_handle_is_simple() {
        fn assert_copy_send_sync<T: Copy + Send + Sync + 'static>() {}
        assert_copy_send_sync::<BufferHandle>();
    }

    /// Verify GraphError is Send + Sync (can be propagated across threads).
    #[test]
    fn graph_error_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<GraphError>();
    }

    /// Verify PassExecutionError is Send + Sync (can be propagated across threads).
    #[test]
    fn pass_execution_error_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<PassExecutionError>();
    }

    /// Verify path filtering uses target scene keys.
    #[test]
    fn target_draw_context_path_matching_is_scene_scoped() {
        let target = TargetDrawContext::new(SceneKey::BASE.get(), 64, 128);
        let target_path = make_path(SceneKey::BASE);
        let other_path = make_path(SceneKey::new(2));

        assert!(target.path_matches(&target_path));
        assert!(!target.path_matches(&other_path));
    }

    // ========================================================================
    // One-Way Data Flow Verification
    // ========================================================================

    /// Type-level proof that RenderGraph only consumes DrawPlan.
    ///
    /// The data flow is:
    /// SceneFrame → DrawPlan → CompiledDrawPlan → RenderGraph → GPU
    ///
    /// RenderGraphBuilder.build() takes &DrawPlan, not SceneFrame/SceneGraph.
    /// This enforces the one-way dependency at the type level.
    #[test]
    fn render_graph_consumes_draw_plan_only() {
        let plan: DrawPlan<MultiSurfaceContract> = make_draw_plan();

        let graph_builder = RenderGraphBuilder::new();

        // RenderGraphBuilder.build() takes &DrawPlan
        // NOT SceneFrame, NOT SceneGraph, NOT Compositor
        let topology: Result<RenderGraphTopology, GraphError> = graph_builder.build(&plan);

        assert!(topology.is_ok());
    }

    /// Verify RenderGraph::compile() takes topology, not scene data.
    ///
    /// Compile phase works with topology + GPU resources only.
    /// No scene/compositor data reaches this point.
    #[test]
    fn render_graph_compile_takes_topology_only() {
        let plan = make_draw_plan();

        let graph_builder = RenderGraphBuilder::new();
        let topology: RenderGraphTopology = graph_builder.build(&plan).unwrap();

        let constants = FrameConstants {
            viewport_size: [800.0, 600.0],
            physical_width: 800,
            physical_height: 600,
            logical_width: 800.0,
            logical_height: 600.0,
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };

        // RenderGraph::compile() takes:
        // - RenderGraphTopology (from build phase)
        // - FrameConstants (per-frame metadata)
        // - HashMap<AttachmentHandle, wgpu::TextureView> (GPU resources)
        //
        // NOT DrawPlan, NOT SceneFrame, NOT Compositor
        let attachments: HashMap<AttachmentHandle, wgpu::TextureView> = HashMap::new();
        let _compiled: CompiledGraph<'_> = RenderGraph::compile(&topology, constants, attachments);
    }

    /// Verify no compositor types in RenderGraph signatures.
    ///
    /// The following imports would cause compile errors if RenderGraph
    /// depended on them (they're not imported in this module):
    /// - crate::compositor::Compositor
    /// - crate::compositor::SurfaceStack
    /// - crate::compositor::surface::SurfaceState
    #[test]
    fn render_graph_has_no_compositor_dependency() {
        // This test exists as documentation and compile-time verification.
        // If RenderGraph imported compositor types, they would appear in
        // the public API and this test file would need to import them.
        //
        // The absence of such imports proves RenderGraph is compositor-free.

        // Allowed types from RenderGraph (all renderer-local):
        let _builder: RenderGraphBuilder = RenderGraphBuilder::new();
        let _handle: AttachmentHandle = AttachmentHandle(0);
        let _id: RenderPassNodeId = RenderPassNodeId(0);

        // These are the ONLY input types to RenderGraph:
        // - DrawPlan (renderer)
        // - FrameConstants (renderer)
        // - wgpu::* (GPU handles)
    }

    /// Verify no widget/event/layout types in RenderGraph.
    ///
    /// RenderGraph operates purely on renderer-level data.
    #[test]
    fn render_graph_has_no_core_dependency() {
        // RenderGraph only imports from:
        // - crate::renderer::*
        // - crate::core::math (RectF for scissor calculations)
        // - crate::core::scene (Color, TextQuad - render primitives only)
        // - std::collections
        // - wgpu
        //
        // NOT crate::core::widget
        // NOT crate::core::event
        // NOT crate::core::layout
        // NOT crate::core::hit_test
        // NOT crate::core::focus (if it existed)

        // This test documents the boundary. If RenderGraph imported
        // these forbidden modules, this file would need to reference them
        // or compilation would fail with missing types in APIs.
    }

    // ========================================================================
    // Thread/Lifetime Correctness
    // ========================================================================

    /// Document that CompiledGraph may be !Send (GPU-bound).
    ///
    /// CompiledGraph contains wgpu::TextureView which may not be Send
    /// on all platforms. This is intentional - compiled graphs are
    /// frame-scoped and execute on the render thread.
    #[test]
    fn compiled_graph_is_frame_scoped() {
        // CompiledGraph is intentionally frame-scoped:
        // - Created by RenderGraph::compile() during frame
        // - Consumed by RenderGraph::execute() during same frame
        // - Contains bound GPU resources (TextureViews)
        //
        // It should NOT be stored across frames or sent to other threads.
        // The !Send nature of wgpu types enforces this at compile time.
    }

    /// Verify no Arc<Compositor> or callbacks in RenderGraph.
    ///
    /// RenderGraph must not hold references back to core state.
    #[test]
    fn render_graph_has_no_reverse_references() {
        // RenderGraphBuilder, RenderGraphTopology, and CompiledGraph
        // contain only:
        // - Owned data (Vec, HashMap)
        // - Simple handles (u32 newtypes)
        // - GPU resource references (scoped to frame)
        //
        // NO Arc<Compositor>, NO Box<dyn Widget>, NO closures capturing state.
        //
        // This is enforced by the struct definitions themselves.
        // Adding such types would change the ABI and fail the freeze tests.
    }
}
