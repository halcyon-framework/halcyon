mod authority_firewall_guard;
pub mod batcher;
pub(crate) mod bloom;
pub use bloom::{BloomBuildRefusal, BloomCompileRefusal};
pub mod compiled_draw_plan;
pub mod custom_render_pass;
pub mod custom_render_prepare;
pub mod draw_plan;
pub mod draw_plan_builder;
pub(crate) mod frosted_glass;
pub use frosted_glass::{RendererCompileError, FrostedGlassCompileRefusal,
    RendererCompileOutcome, RendererPreparationIdentity};
mod draw_plan_guard;
pub mod glyph_atlas;
pub mod gpu_arena;
pub(crate) mod gpu_custody;
pub mod image_binding;
pub mod mesh_cache;
pub mod path;
pub mod path_renderer;
pub mod pipelines;
pub(crate) mod pixel_coverage;
mod queue_completion;
pub(crate) use queue_completion::on_submitted_work_done;
pub mod render_graph;
pub mod render_target;
mod sdf_champagne_material;
pub(crate) mod sdf_frame_custody;
#[allow(dead_code)] // Custody core only; frame-owner integration is not admitted here.
mod sdf_image_generation_custody;
#[allow(dead_code)] // CPU status model only; producer/engine commit is not wired.
pub(crate) mod sdf_material_status;
#[allow(dead_code)] // Static source admitted; App producer/consumer wiring remains separate.
mod sdf_static_material;
pub mod staging_pool;
pub mod text_realization;
pub mod text_vertex;
pub mod vertex;
pub mod wgpu_renderer;

#[cfg(feature = "render-audit")]
mod cadence_audit {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct RenderCadenceSnapshot {
        pub surface_submissions: u64,
        pub surface_presentations: u64,
        pub renderer_frame_advances: u64,
    }

    thread_local! {
        static SNAPSHOT: Cell<RenderCadenceSnapshot> = const {
            Cell::new(RenderCadenceSnapshot {
                surface_submissions: 0,
                surface_presentations: 0,
                renderer_frame_advances: 0,
            })
        };
    }

    fn update(update: impl FnOnce(&mut RenderCadenceSnapshot)) {
        SNAPSHOT.with(|snapshot| {
            let mut next = snapshot.get();
            update(&mut next);
            snapshot.set(next);
        });
    }

    pub fn snapshot() -> RenderCadenceSnapshot {
        SNAPSHOT.get()
    }

    pub(crate) fn record_surface_submission() {
        update(|snapshot| snapshot.surface_submissions += 1);
    }

    pub(crate) fn record_surface_presentation() {
        update(|snapshot| snapshot.surface_presentations += 1);
    }

    pub(crate) fn record_renderer_frame_advance() {
        update(|snapshot| snapshot.renderer_frame_advances += 1);
    }
}

#[cfg(feature = "render-audit")]
pub use cadence_audit::{RenderCadenceSnapshot, snapshot as render_cadence_snapshot};

pub use compiled_draw_plan::{CompiledDrawPlan, MultiTargetDrawPlan, MultiTargetRenderOutput};
pub use custom_render_pass::{
    CustomRenderAdmission, CustomRenderFrame, CustomRenderLifecycle, CustomRenderPass,
    CustomRenderPassError, CustomRenderPassResult, CustomRenderPresentationBinding,
    CustomRenderPresentationId, PhysicalRect,
};
pub use custom_render_prepare::{
    CustomRenderActivity, CustomRenderParticipant, CustomRenderPreparationOutcome,
    CustomRenderTargetMetrics,
};
pub use draw_plan::{DrawPlan, RectVertex};
pub use draw_plan_builder::DrawPlanBuilder;
pub use pixel_coverage::PhysicalExtent;
pub use render_target::{
    AcquiredTargetGuard, RenderTarget, RenderTargetError, RenderTargetRegistry, SurfaceBundle,
    RendererSurfaceError, TargetDescriptor,
};
pub use text_realization::{
    SurfaceTextRealization, TextRealizationFailure, TextRealizationFailureReason,
    TextRealizationOutcome,
};
pub use vertex::{BakedVertex, QuadInstance, QuadVertex};

pub use path_renderer::PathRenderer;
pub use render_graph::{
    AttachmentDesc, AttachmentHandle, BoundResources, BufferDesc, BufferHandle, CompiledGraph,
    GraphError, PassContext, PassExecutionError, PassExecutor, RenderGraphBuilder,
    RenderGraphTopology, RenderPassNode, RenderPassNodeId, ResourceDescriptor, ResourceRef,
};

// ============================================================================
// SPEC-HAL-040 Stage 1: Frame Epoch Model
// ============================================================================

/// SPEC-HAL-040: Monotonically increasing frame counter for GPU resource lifetimes.
///
/// This is the canonical source of truth for resource lifetimes in the renderer.
/// Each successful frame rendered via `render_multi()` advances the epoch by exactly 1.
///
/// ## Semantics
///
/// - Epoch is renderer-local, not tied to wall time.
/// - Strictly monotonic: each successful frame increments by 1.
/// - Used to tag: resource creation epoch, last-used epoch, safe destruction epoch.
/// - Never decremented or intentionally wrapped. Overflow is "undefined behavior /
///   practically impossible" (would require ~584 billion years at 1M fps).
///
/// ## Invariants
///
/// - Epoch advances only after `queue.submit()` and `queue.present(output)` succeed.
/// - Epoch does NOT advance on skipped frames (unfocused, surface error before submit).
/// - Only `WgpuRenderer::advance_frame()` may advance the epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameEpoch(pub u64);

impl FrameEpoch {
    /// Create epoch 0 (initial value for new renderer).
    #[inline]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Advance to the next epoch.
    #[inline]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Get the raw epoch value.
    #[inline]
    pub const fn value(self) -> u64 {
        self.0
    }
}
