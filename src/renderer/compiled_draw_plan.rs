//! SPEC-HAL-084: CompiledDrawPlan — Compile Phase Output
//!
//! CompiledDrawPlan is the GPU-ready output of the Compile phase. It contains
//! all resources needed for command encoding in the Execute phase.
//!
//! # Data Flow
//!
//! ```text
//! DrawPlan (CPU-only, from Build phase)
//!     ↓
//! WgpuRenderer::compile_multi()  ← Compile phase
//!     ↓
//! MultiTargetDrawPlan (GPU-ready)
//!     ↓
//! WgpuRenderer::render_multi()  ← Execute phase (command encoding only)
//! ```
//!
//! # Invariants (SPEC-HAL-084)
//!
//! - INV-COMPILE-001: CompiledDrawPlan contains all GPU-prepared data for a frame
//! - INV-COMPILE-002: Compile phase performs all buffer uploads and resource binding
//! - INV-COMPILE-003: Execute phase only encodes commands, no buffer writes
//! - INV-COMPILE-004: CompiledDrawPlan is valid only for the frame it was compiled for
//!
//! # SPEC-HAL-081A Current Contract Invariants
//!
//! - INV-COMP-P04: Surface stack is authoritative for draw order
//! - Surface stack is MANDATORY (not optional)

use crate::compositor::{
    BaseContract, Contract, Contracted, MultiSurfaceContract, SurfaceId, SurfaceStackSnapshot,
};
use crate::core::paint::EffectivePaintOrder;
use crate::core::scene::TextQuad;
use crate::renderer::batcher::FrameBatches;
use crate::renderer::render_graph::{AttachmentHandle, RenderGraphTopology};
use crate::renderer::render_target::TargetDescriptor;
use std::collections::HashMap;

/// SPEC-HAL-359: One immutable per-surface execution command.
///
/// Commands retain the admitted cross-pipeline Z order after Compile has
/// lowered each primitive family into its pipeline-specific GPU buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SurfaceDrawCommand {
    FrostedGlass {
        order: EffectivePaintOrder,
        z_index:u32,
        ordinal:u64,
    },
    Batch {
        order: EffectivePaintOrder,
        z_index: u32,
        batch_index: usize,
    },
    Path {
        order: EffectivePaintOrder,
        z_index: u32,
        path_index: usize,
    },
    Text {
        order: EffectivePaintOrder,
        z_index: u32,
        quad_start: u32,
        quad_count: u32,
    },
}

impl SurfaceDrawCommand {
    pub(crate) const fn z_index(&self) -> u32 {
        match self {
            Self::Batch { z_index, .. }
            | Self::FrostedGlass {z_index,..}
            | Self::Path { z_index, .. }
            | Self::Text { z_index, .. } => *z_index,
        }
    }

    pub(crate) const fn tie_rank(&self) -> u8 {
        match self {
            Self::Batch { .. } => 0,
            Self::FrostedGlass {..} => 0,
            Self::Path { .. } => 1,
            Self::Text { .. } => 2,
        }
    }

    pub(crate) fn order(&self) -> &EffectivePaintOrder {
        match self {
            Self::Batch { order, .. } | Self::Path { order, .. } | Self::Text { order, .. }
            | Self::FrostedGlass {order,..} => {
                order
            }
        }
    }
}

/// SPEC-HAL-084: Compile phase output containing GPU-ready data.
///
/// **Note:** This generic struct is retained for type-level contract enforcement.
/// Production rendering uses `MultiTargetDrawPlan` via `compile_multi()`/`render_multi()`.
///
/// # Contract Parameterization (SPEC-HAL-081A)
///
/// The `P` type parameter encodes the compositor contract at the type level.
/// This prevents incompatible code from accidentally using the wrong compiled plans.
///
/// # Lifetime
///
/// CompiledDrawPlan is valid only for the frame it was compiled for.
/// It holds references to GPU resources that may be reused or invalidated
/// in subsequent frames.
///
/// # Type-Level Enforcement (SPEC-HAL-084)
///
/// This type is designed to prevent misuse at compile time:
///
/// 1. **No Clone**: Contains `wgpu::SurfaceTexture` which is not Clone.
///    Prevents using the same compiled plan twice.
///
/// 2. **Move semantics**: `render_multi()` takes plans by value, not reference.
///    After render, the plan is consumed and cannot be reused.
///
/// 3. **No public constructor**: All fields are `pub(crate)`.
///    Only the renderer can create instances.
///
/// 4. **Frame-scoped**: The `surface_output` field ties this to a specific
///    surface texture acquisition, preventing cross-frame storage.
///
/// 5. **Contract-scoped**: The contract parameter ensures plans can't be
///    accidentally used with incompatible rendering logic.
pub struct CompiledDrawPlan<P: Contract = BaseContract> {
    // ========================================================================
    // Surface State (from compile phase)
    // ========================================================================
    /// The surface texture output acquired during compile.
    /// Passed to execute phase for final presentation.
    #[allow(dead_code, reason = "Retained exact acquired surface texture custody")]
    pub(crate) surface_output: wgpu::SurfaceTexture,

    // ========================================================================
    // Batch Metadata (from DrawPlan, enriched during compile)
    // ========================================================================
    /// Frame batches with draw ranges and pipeline metadata.
    /// Includes rect batches, text batches, path draws.
    pub(crate) frame_batches: FrameBatches,

    // ========================================================================
    // Text Pipeline Data (built during compile)
    // ========================================================================
    /// Text quads built from primitives using glyph atlas.
    /// Each quad maps to 6 indices in the text index buffer.
    pub(crate) text_quads: Vec<TextQuad>,

    // ========================================================================
    // Clip State (computed during compile)
    // ========================================================================
    /// Map from clip_state.hash to dynamic uniform buffer offset.
    /// Used for per-batch rounded clip binding during execute.
    #[allow(dead_code, reason = "Retained compiled rounded-clip binding metadata")]
    pub(crate) rounded_clip_offsets: HashMap<u64, u32>,

    // ========================================================================
    // Frame Metadata
    // ========================================================================
    /// Clear color converted to wgpu format.
    #[allow(dead_code, reason = "Retained compiled clear-color metadata")]
    pub(crate) clear_color: wgpu::Color,

    // ========================================================================
    // SPEC-HAL-081A: Compositor Surface Stack (Ordering Authority)
    // ========================================================================
    /// Compositor surface stack for surface-ordered rendering.
    ///
    /// INV-COMP-P04: The surface stack is authoritative for draw order.
    /// All rendering MUST iterate surfaces in stack order.
    ///
    /// This is NOT optional - it's mandatory per SPEC-HAL-081A.
    /// The contract parameter `P` is propagated from the snapshot.
    #[allow(dead_code, reason = "Retained compositor ordering snapshot")]
    pub(crate) surface_stack: SurfaceStackSnapshot<P>,

    /// Phase marker for compile-time enforcement.
    /// Zero-cost abstraction - PhantomData adds no runtime overhead.
    pub(crate) _phase: Contracted<P>,
}

impl<P: Contract> CompiledDrawPlan<P> {
    /// Check if there are any rect batches to render.
    pub fn has_rect_batches(&self) -> bool {
        use crate::renderer::batcher::PipelineId;
        self.frame_batches
            .batches
            .iter()
            .any(|b| b.key.pipeline == PipelineId::Rect)
    }

    /// Check if there are any text quads to render.
    pub fn has_text(&self) -> bool {
        !self.text_quads.is_empty()
    }

    /// Check if there are any path draws.
    pub fn has_paths(&self) -> bool {
        !self.frame_batches.paths.is_empty()
    }
}

// ============================================================================
// SPEC-HAL-081B: Multi-Target Compiled Draw Plan
// ============================================================================

// ============================================================================
// SPEC-HAL-163: Per-Surface Compiled Data
// ============================================================================

/// SPEC-HAL-163: Per-surface compiled rendering data.
///
/// Each surface has its own frame batches, text quads, and clear color.
/// This enforces INV-SCENE-001 (surface-scoped scenes) and INV-SCENE-002
/// (no cross-surface dependency).
#[derive(Debug, Clone)]
pub struct SurfaceCompiledData {
    pub(crate) text_realization: crate::renderer::text_realization::SurfaceTextRealization,
    /// Frame batches for this surface only.
    pub(crate) frame_batches: FrameBatches,
    /// Text quads for this surface only.
    pub(crate) text_quads: Vec<TextQuad>,
    /// Offset, in text quads, into the frame-global text GPU buffers.
    pub(crate) text_quad_offset: u32,
    /// Offset, in path draws, into the frame-global path renderer batch.
    pub(crate) path_draw_offset: usize,
    /// SPEC-HAL-359: Immutable cross-pipeline execution order for this surface.
    pub(crate) draw_commands: Vec<SurfaceDrawCommand>,
    /// Clear color for this surface.
    pub(crate) clear_color: wgpu::Color,
    /// Rounded clip offsets for this surface.
    pub(crate) rounded_clip_offsets: HashMap<u64, u32>,
    /// SPEC-R-013-v2: Shadow params dynamic uniform offsets, keyed by ShadowParamsData batch_hash
    pub(crate) shadow_params_offsets: HashMap<u64, u32>,
    /// SPEC-R-012-v2: Gradient params dynamic uniform offsets
    pub(crate) gradient_params_offsets: HashMap<u64, u32>,
    pub(crate) analytic_surface_params_offsets: HashMap<u64, u32>,
    /// SPEC-HAL-272: ColorPicker SV params dynamic uniform offsets
    pub(crate) color_picker_sv_params_offsets: HashMap<u64, u32>,
    /// SPEC-HAL-278: RadialGauge params dynamic uniform offsets
    pub(crate) radial_gauge_params_offsets: HashMap<u64, u32>,
}

impl SurfaceCompiledData {
    /// Text preparation evidence for this surface; not a presentation receipt.
    pub fn text_realization(&self) -> &crate::renderer::text_realization::SurfaceTextRealization {
        &self.text_realization
    }
}

/// SPEC-HAL-081B: Multi-target compile phase output.
/// SPEC-HAL-163: Extended for surface-scoped scene partitioning.
///
/// A purely declarative GPU work description. Contains target descriptors
/// (metadata only) and per-surface batch data, but NO acquired textures or ownership.
/// Texture acquisition is an execution-time concern handled by `render_multi()`.
///
/// # SPEC-HAL-163: Surface Isolation
///
/// Each surface has its own `SurfaceCompiledData` containing:
/// - Frame batches specific to that surface
/// - Text quads specific to that surface
/// - Clear color for that surface
///
/// This enforces:
/// - INV-SCENE-001: SceneGraphs are surface-scoped
/// - INV-SCENE-002: No cross-surface background/text dependency
/// - INV-SCENE-004: Renderer never sees a mixed scene
///
/// # Invariants
///
/// - INV-MT-001: One TargetDescriptor per base surface in snapshot
/// - INV-MT-002: Targets are in compositor z-order (lowest to highest)
/// - INV-MT-003: Overlays are never present (filtered by compositor)
///
/// # Usage
///
/// ```ignore
/// // Multi-surface rendering path:
/// // Native single-frame driver. Browser App handles Pending through its
/// // completion wake and resumes the same renderer preparation.
/// let compiled = renderer.compile_multi(&draw_plan)?.expect_ready("native Compile");
/// let outputs = renderer.render_multi(compiled);
/// for output in outputs {
///     renderer.submit_and_present(output);
/// }
/// ```
pub struct MultiTargetDrawPlan {
    pub(crate) frosted_surfaces:HashMap<SurfaceId,crate::renderer::frosted_glass::FrostedSurfacePlan>,
    pub(crate) presence: HashMap<SurfaceId, Vec<crate::renderer::wgpu_renderer::PresenceFrameUse>>,
    pub(crate) preparation: std::sync::Arc<()>,
    pub(crate) preparation_epoch: crate::renderer::FrameEpoch,
    // ========================================================================
    // Target Descriptors (one per base surface)
    // ========================================================================
    /// Target descriptors (metadata only), one per base surface in z-order.
    /// No acquired textures - acquisition happens in render_multi().
    pub(crate) target_descriptors: Vec<TargetDescriptor>,

    // ========================================================================
    // SPEC-HAL-163: Per-Surface Compiled Data
    // ========================================================================
    /// Per-surface compiled data (batches, text quads, clear color).
    /// INV-SCENE-001: Each surface has its own rendering data.
    pub(crate) surface_data: HashMap<SurfaceId, SurfaceCompiledData>,

    // ========================================================================
    // SPEC-HAL-081B: Surface Stack (Ordering Authority)
    // ========================================================================
    /// Compositor surface stack for surface-ordered rendering.
    /// MultiSurfaceContract permits multiple base surfaces.
    pub(crate) surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,

    /// RenderGraph topology built from the DrawPlan during compile.
    pub(crate) render_graph_topology: RenderGraphTopology,
    /// Output attachment handle used by the RenderGraph passes.
    pub(crate) render_graph_output: AttachmentHandle,

    /// Phase marker.
    pub(crate) _phase: Contracted<MultiSurfaceContract>,
}

impl MultiTargetDrawPlan {
    /// Complete-required text preparation for a surface in this plan. Its
    /// resource validity ends on the renderer's next Compile or frame advance.
    pub fn text_realization(
        &self,
        id: SurfaceId,
    ) -> Option<&crate::renderer::text_realization::SurfaceTextRealization> {
        self.surface_data
            .get(&id)
            .map(SurfaceCompiledData::text_realization)
    }

    /// Get the number of render targets.
    pub fn target_count(&self) -> usize {
        self.target_descriptors.len()
    }

    /// Check if there are any targets to render.
    pub fn has_targets(&self) -> bool {
        !self.target_descriptors.is_empty()
    }

    /// Iterate over target descriptors in z-order.
    pub fn targets(&self) -> impl Iterator<Item = &TargetDescriptor> {
        self.target_descriptors.iter()
    }

    /// Get a target descriptor by SurfaceId.
    pub fn get_target(&self, id: SurfaceId) -> Option<&TargetDescriptor> {
        self.target_descriptors.iter().find(|t| t.surface_id == id)
    }

    /// SPEC-HAL-163: Get compiled data for a specific surface.
    pub fn surface_compiled_data(&self, id: SurfaceId) -> Option<&SurfaceCompiledData> {
        self.surface_data.get(&id)
    }

    /// Check if any surface has rect batches.
    pub fn has_rect_batches(&self) -> bool {
        use crate::renderer::batcher::PipelineId;
        self.surface_data.values().any(|data| {
            data.frame_batches
                .batches
                .iter()
                .any(|b| b.key.pipeline == PipelineId::Rect)
        })
    }

    /// Check if any surface has text quads.
    pub fn has_text(&self) -> bool {
        self.surface_data
            .values()
            .any(|data| !data.text_quads.is_empty())
    }

    /// Check if any surface has path draws.
    pub fn has_paths(&self) -> bool {
        self.surface_data
            .values()
            .any(|data| !data.frame_batches.paths.is_empty())
    }
}

/// SPEC-HAL-081B: Render output for a single target in multi-surface rendering.
///
/// One of these is produced per target during render_multi().
///
/// HAL385 §9 deliberately seals the original command. Explicit raw submission
/// uses consuming `into_raw_parts`, quarantining any transient frost backing.
///
/// ```compile_fail,E0609
/// use halcyon::renderer::MultiTargetRenderOutput;
/// fn frosted_glass_cannot_replace_managed_raw(output: &mut MultiTargetRenderOutput) {
///     let _borrow = &mut output.command_buffer;
/// }
/// ```
///
/// ```compile_fail,E0616
/// use halcyon::renderer::MultiTargetRenderOutput;
/// fn frosted_glass_command_owner_is_private(output: &mut MultiTargetRenderOutput) {
///     let _borrow = &mut output.commands;
/// }
/// ```
///
/// ```
/// use halcyon::{compositor::SurfaceId, renderer::MultiTargetRenderOutput};
/// let _: fn(SurfaceId,wgpu::CommandBuffer,wgpu::SurfaceTexture)->MultiTargetRenderOutput = MultiTargetRenderOutput::new;
/// let _: fn(MultiTargetRenderOutput)->(SurfaceId,wgpu::CommandBuffer,wgpu::SurfaceTexture) = MultiTargetRenderOutput::into_raw_parts;
/// let _: fn(MultiTargetRenderOutput) = MultiTargetRenderOutput::cancel_unsubmitted;
/// ```
pub struct MultiTargetRenderOutput {
    commands: crate::renderer::frosted_glass::EncodedTargetCommands,
    /// The SurfaceId this output is for.
    pub surface_id: SurfaceId,
    /// The surface texture to present.
    pub surface_output: wgpu::SurfaceTexture,
}
impl MultiTargetRenderOutput {
    /// Ordinary output without transient frosting custody (HAL385 §8.8).
    pub fn new(surface_id:SurfaceId, command_buffer:wgpu::CommandBuffer,
        surface_output:wgpu::SurfaceTexture)->Self {
        Self{surface_id,commands:crate::renderer::frosted_glass::EncodedTargetCommands::unmanaged(command_buffer),surface_output}
    }
    pub(crate) fn from_encoded(surface_id:SurfaceId,commands:crate::renderer::frosted_glass::EncodedTargetCommands,surface_output:wgpu::SurfaceTexture)->Self {
        Self {surface_id,commands,surface_output}
    }
    pub(crate) fn into_submission_parts(self)->(crate::renderer::frosted_glass::EncodedTargetCommands,wgpu::SurfaceTexture) {
        (self.commands,self.surface_output)
    }
    /// Explicitly dispose of a still-owned unsubmitted command before releasing
    /// its frost backing. Consuming raw export relinquishes that authority.
    pub fn cancel_unsubmitted(self) {
        drop(self.commands);
        drop(self.surface_output);
    }
    /// Consuming raw export preserves custom submission capability but cannot
    /// certify completion or reattach the original frost use to a replacement.
    pub fn into_raw_parts(self)->(SurfaceId,wgpu::CommandBuffer,wgpu::SurfaceTexture) {
        (self.surface_id,self.commands.into_raw(),self.surface_output)
    }
}
