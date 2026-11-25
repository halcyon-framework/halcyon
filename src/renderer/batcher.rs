//! SPEC-HAL-027: Scene Graph Batching & Draw-Call Minimization
//! SPEC-R-001: Clip Stack Rework (Stages 1-4 Complete)
//! SPEC-R-002: Z-Index & Ordering Determinism (Stages 1-4 Complete)
//!
//! This module provides deterministic batching of scene graph primitives to minimize
//! GPU draw calls while maintaining correct Z-order and clip semantics.
//!
//! ## Clip Stack Semantics (SPEC-R-001)
//!
//! The batcher maintains a unified clip stack that propagates clip state through
//! scene graph traversal. Key invariants:
//!
//! - ClipState captures the current effective clip rect and its deterministic hash
//! - Push clips intersect with parent clip (shrinking or equal)
//! - Empty intersections produce zero-area RectF (not None)
//! - Clip hash is deterministic: depends only on stack depth and rect components
//! - All primitives (rect, text, shapes) use the same clip propagation logic
//!
//! ## SPEC-R-001 Implementation Status
//!
//! - **Stage 1**: ClipState struct, unified clip stack semantics
//! - **Stage 2**: FlatPrimitive/DrawBatch use ClipState as single source of truth
//! - **Stage 3**: Canonical scissor path in renderer with debug invariants
//! - **Stage 4**: Final cleanup - no legacy clip_rect fields remain in batch/primitive types
//!
//! ## SPEC-R-002: Z-Index & Ordering Determinism
//!
//! Ensures fully deterministic draw ordering for all primitives:
//!
//! - Each primitive gets a monotonically increasing `order_key` at insertion time
//! - `DrawBatchKey` sorts by `(z_layer, clip_hash, pipeline, texture_id, image_generation, shape_type_id)`
//! - `order_key` is only the tiebreaker within an identical `DrawBatchKey`
//! - Visual layering across different batch keys requires a strictly higher z-layer
//!   under SPEC-PM-007; insertion order is not cross-batch layering authority
//!
//! ## SPEC-R-004 Stage 18: Path Instance Deterministic Ordering
//!
//! Path instances follow a canonical sort key for deterministic GPU submission:
//! `(z_index ASC, clip_id ASC, stroke_flag ASC, debug_mode ASC, insertion_order ASC)`
//!
//! This ensures:
//! - Independent of scene graph traversal order
//! - Independent of HashMap iteration order
//! - Stable across multiple runs with identical input

use crate::compositor::{SceneKey, SurfaceId};
use crate::core::math::{Point, RectF};
use crate::core::paint::{
    ControlSubtreeMember, EffectivePaintOrder, LuminousContourAppearance, LuminousContourFronts,
    visit_luminous_contour_regions,
};
use crate::core::scene::{
    Color, ImageTextureSource, LuminousContourNode, PathNode, SceneNode, TextBackgroundSpan, ZIndex,
};
use crate::renderer::custom_render_prepare::ImageOccurrenceRecorder;
use crate::renderer::mesh_cache::{MeshCache, MeshCacheKey, PixelSnapMode};
use crate::renderer::path::{
    FillContours, Path, PathDebugMode, PathDraw, PathFillError, PathMesh, StrokeCap, StrokeJoin,
    StrokeParams,
};
use crate::renderer::pipelines::shape_pipeline::ShapeType;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ============================================================================
// SPEC-R-004 Stage 18: Path Sort Key for Deterministic Ordering
// ============================================================================

/// SPEC-R-004 Stage 18: Canonical sort key for path instances.
///
/// Defines the deterministic ordering for all PathDraw entries before GPU submission.
/// Sort order: `(z_index ASC, clip_id ASC, stroke_flag ASC, debug_mode ASC, insertion_order ASC)`
///
/// ## Ordering Guarantees
///
/// - z_index: Primary ordering by depth layer
/// - clip_id: Secondary grouping by clip region
/// - stroke_flag: Fill (0) before stroke (1) when z/clip match
/// - debug_mode: None (0) < Vertices (1) < Edges (2) < Triangles (3)
/// - insertion_order: Stable tiebreaker preserving original emission order
///
/// ## Invariants
///
/// - Two PathDraw with identical (z, clip, stroke, debug) are ordered by insertion
/// - Debug overlays use z = u32::MAX - 1, ensuring they render after normal paths
/// - No ordering is influenced by pointer addresses, hash-maps, or floating-point
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathSortKey {
    pub effective_order: EffectivePaintOrder,
    /// Z-index: primary draw order
    pub z_index: u32,
    /// INV-PATH-ORDER-STABILITY: Sort bias for stable ordering independent of presence.
    /// This comes before clip_id to ensure primitive type ordering is preserved
    /// even when optional primitives (caret) are absent.
    pub sort_bias: u32,
    /// Clip buffer index: secondary grouping
    pub clip_id: u32,
    /// Stroke flag: 0 = fill, 1 = stroke
    pub stroke_flag: u32,
    /// Debug mode as ordinal: 0 = None, 1 = Vertices, 2 = Edges, 3 = Triangles
    pub debug_mode: u32,
    /// Insertion order: stable tiebreaker
    pub insertion_order: u32,
}

impl PathSortKey {
    /// Create a sort key from a PathDraw.
    pub fn from_path_draw(path: &PathDraw) -> Self {
        Self {
            effective_order: path.effective_order(),
            z_index: path.z_index,
            sort_bias: path.sort_bias,
            clip_id: path.clip_id,
            stroke_flag: path.stroke_flag,
            debug_mode: Self::debug_mode_ordinal(path.debug),
            insertion_order: path.insertion_order,
        }
    }

    /// Convert PathDebugMode to ordinal for deterministic comparison.
    fn debug_mode_ordinal(mode: Option<PathDebugMode>) -> u32 {
        match mode {
            None => 0,
            Some(PathDebugMode::None) => 0,
            Some(PathDebugMode::Vertices) => 1,
            Some(PathDebugMode::Edges) => 2,
            Some(PathDebugMode::Triangles) => 3,
        }
    }
}

// ============================================================================
// INV-PATH-ORDER-STABILITY: Sort Bias Constants
// ============================================================================
// These constants define stable ordering slots for PATH primitives.
// Presence/absence of optional primitives (caret, selection) must not
// change the relative order of other primitives within the same z_index.

/// Sort bias for background fills (solid color backgrounds).
pub const PATH_SORT_BIAS_BACKGROUND: u32 = 0;
/// Sort bias for border primitives (ring shapes).
pub const PATH_SORT_BIAS_BORDER: u32 = 100;
/// Sort bias for scrollbar primitives.
pub const PATH_SORT_BIAS_SCROLLBAR: u32 = 200;
/// Sort bias for caret and selection primitives.
pub const PATH_SORT_BIAS_CARET: u32 = 300;

// ============================================================================
// SPEC-R-001: Clip State (Stage 1 - CPU Clip Stack Formalization)
// ============================================================================

/// Maximum clip stack depth to prevent unbounded recursion.
///
/// This is a defensive limit - typical UI hierarchies rarely exceed 16 levels.
/// In debug builds, exceeding this limit triggers a panic.
#[cfg(debug_assertions)]
const MAX_CLIP_STACK_DEPTH: usize = 64;

/// SPEC-R-001: Snapshot of the clip stack state at a point in scene traversal.
///
/// This struct represents the effective clipping region at any point during
/// scene graph traversal. It captures both the current clip rectangle and
/// a deterministic hash for batch key generation.
///
/// ## Invariants
///
/// - `rect` is always the intersection of all clips in the stack
/// - `hash` is deterministic: same stack produces same hash
/// - Empty intersections produce zero-area RectF (never None)
/// - NaN and infinite values are rejected (debug panic)
///
/// SPEC-R-020 Stage 3: Added `rounded` field for rounded clip support.
/// - Rounded corners are NOT intersected (only rects are)
/// - Hash incorporates rounded values for batch splitting
/// - None = fast scissor path, Some = GPU mask pipeline
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipState {
    /// Effective clip rectangle in logical coordinates.
    /// This is the intersection of all clips in the current stack.
    pub rect: RectF,
    /// SPEC-R-020 Stage 3: Optional rounded corners for the current clip.
    /// Only the topmost rounded clip in the stack is tracked (no nesting yet).
    pub rounded: Option<crate::core::scene::RoundedCorners>,
    /// Deterministic hash of the clip stack state.
    /// Depends only on stack depth and each rect's exact f32 components.
    /// SPEC-R-020 Stage 3: Also incorporates rounded corner values if present.
    pub hash: u64,
}

impl ClipState {
    /// Create a ClipState with no clipping (full viewport).
    pub fn none() -> Self {
        Self {
            rect: RectF::zero(),
            rounded: None,
            hash: 0,
        }
    }

    /// Create a ClipState from a specific rect and hash.
    pub fn new(rect: RectF, hash: u64) -> Self {
        Self {
            rect,
            rounded: None,
            hash,
        }
    }

    /// SPEC-R-020 Stage 3: Create a ClipState with rounded corners.
    pub fn with_rounded(
        rect: RectF,
        rounded: Option<crate::core::scene::RoundedCorners>,
        hash: u64,
    ) -> Self {
        Self {
            rect,
            rounded,
            hash,
        }
    }

    /// Returns true if this clip state has no effective clipping.
    pub fn is_none(&self) -> bool {
        self.hash == 0
    }

    /// Returns true if the clip rect has zero area.
    pub fn is_empty(&self) -> bool {
        self.rect.width <= 0.0 || self.rect.height <= 0.0
    }
}

impl Default for ClipState {
    fn default() -> Self {
        Self::none()
    }
}

/// SPEC-R-020 Stage 3: Internal clip stack entry with rect + optional rounded corners.
#[derive(Debug, Clone, Copy)]
struct ClipStackEntry {
    rect: RectF,
    // HAL386: original scroll-projected geometry; legacy intersection is unchanged.
    source_rect: RectF,
    rounded: Option<crate::core::scene::RoundedCorners>,
}

// ============================================================================
// Draw Batch Types
// ============================================================================

/// Unique identifier for a draw batch within a frame
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawBatchId(pub u32);

/// Pipeline types supported by the renderer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PipelineId {
    Rect = 1,
    Text = 2,
    Line = 3,
    Image = 4,
    /// SPEC-R-003: Shape pipeline for GPU path rendering primitives
    Shape = 5,
    /// SPEC-R-004: Path pipeline for tessellated vector paths
    Path = 6,
    /// SPEC-R-013-v2: Analytical shadow pipeline
    Shadow = 7,
    /// SPEC-R-012-v2: Gradient-filled rectangle pipeline
    Gradient = 8, // SPEC-R-012-v2
    ColorPickerSv = 9, // SPEC-HAL-272
    RadialGauge = 10,  // SPEC-HAL-278
    /// SPEC-R-022: SDF rounded rect pipeline
    SdfRect = 11,
    /// HAL381 private, viewport-scoped canonical-front presenter. No public
    /// Image ingress selects this pipeline.
    CanonicalOverNeutral = 12,
    AnalyticSurface = 13,
    FrostedGlass = 14,
}

/// Key that determines batch compatibility
///
/// Two primitives can be batched together iff they have identical batch keys.
/// Batch keys capture all GPU state that would require a pipeline/bind group switch.
///
/// ## SPEC-R-002 Stage 2: Field Ordering
///
/// Fields are ordered with `z_layer` first for emphasis. The `Ord` implementation
/// compares in order: `(z_layer, clip_hash, pipeline, texture_id, image_generation, shape_type_id)`.
/// This ensures batches are strictly grouped by Z-layer first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawBatchKey {
    pub control_subtree_order: Option<Arc<[ControlSubtreeMember]>>,
    /// Z-layer for ordering (batches cannot cross Z boundaries)
    /// SPEC-R-002: Primary sort key - z_layer is always compared first
    pub z_layer: ZIndex,
    /// Hash of the current clip stack state
    pub clip_hash: u64,
    /// Pipeline type (rect, text, line, image, shape)
    pub pipeline: PipelineId,
    /// Optional texture ID for text/image batches
    pub texture_id: Option<u64>,
    pub image_generation: Option<std::num::NonZeroU64>,
    /// SPEC-R-003: Shape type ID for batching different shape types.
    /// 0 = Circle, 1 = Ellipse, 2 = Line, 3 = Polyline.
    ///
    /// INVARIANT: Must be `Some(id)` iff `pipeline == PipelineId::Shape`.
    /// Shape batches MUST be partitioned by type - different shape types
    /// cannot share a batch even with matching clip/z-layer.
    pub shape_type_id: Option<u8>,
}

impl std::hash::Hash for DrawBatchKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.z_layer.hash(state);
        self.control_subtree_order.hash(state);
        self.clip_hash.hash(state);
        self.pipeline.hash(state);
        self.texture_id.hash(state);
        self.shape_type_id.hash(state);
        if let Some(generation) = self.image_generation {
            generation.hash(state);
        }
    }
}

/// SPEC-HAL-359: Complete effective order precedes mechanical batch grouping.
///
/// Unscoped primitives retain scalar Z ordering; closed members carry their hierarchy.
impl Ord for DrawBatchKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.effective_order()
            .cmp(&other.effective_order())
            .then_with(|| {
                self.control_subtree_order
                    .is_some()
                    .cmp(&other.control_subtree_order.is_some())
            })
            .then_with(|| self.clip_hash.cmp(&other.clip_hash))
            .then_with(|| self.pipeline.cmp(&other.pipeline))
            .then_with(|| self.texture_id.cmp(&other.texture_id))
            .then_with(|| self.image_generation.cmp(&other.image_generation))
            .then_with(|| self.shape_type_id.cmp(&other.shape_type_id))
    }
}

impl PartialOrd for DrawBatchKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl DrawBatchKey {
    pub(crate) fn effective_order(&self) -> EffectivePaintOrder {
        EffectivePaintOrder::new(self.z_layer, self.control_subtree_order.clone())
    }

    pub fn new(
        pipeline: PipelineId,
        clip_hash: u64,
        texture_id: Option<u64>,
        z_layer: ZIndex,
    ) -> Self {
        Self {
            z_layer,
            control_subtree_order: None,
            clip_hash,
            pipeline,
            texture_id,
            image_generation: None,
            shape_type_id: None,
        }
    }

    /// Create a key for solid rectangles
    pub fn rect(clip_hash: u64, z_layer: ZIndex) -> Self {
        Self::new(PipelineId::Rect, clip_hash, None, z_layer)
    }

    /// Create a key for text rendering
    pub fn text(clip_hash: u64, texture_id: u64, z_layer: ZIndex) -> Self {
        Self::new(PipelineId::Text, clip_hash, Some(texture_id), z_layer)
    }

    /// Create a key for images
    pub fn image(clip_hash: u64, texture_id: u64, z_layer: ZIndex) -> Self {
        Self::new(PipelineId::Image, clip_hash, Some(texture_id), z_layer)
    }

    /// SPEC-R-013-v2: Create a key for shadow primitives.
    /// shadow_params_hash groups shadows with identical params into the same batch.
    pub fn shadow(clip_hash: u64, shadow_params_hash: u64, z_layer: ZIndex) -> Self {
        Self::new(
            PipelineId::Shadow,
            clip_hash,
            Some(shadow_params_hash),
            z_layer,
        )
    }

    /// SPEC-R-012-v2: Create a key for gradient primitives.
    pub fn gradient(clip_hash: u64, gradient_params_hash: u64, z_layer: ZIndex) -> Self {
        Self::new(
            PipelineId::Gradient,
            clip_hash,
            Some(gradient_params_hash),
            z_layer,
        )
    }

    /// SPEC-HAL-272: Batch key for a color-picker SV square.
    /// `hue_bits` is the IEEE 754 bit pattern of the hue float, used as params hash.
    pub fn color_picker_sv(clip_hash: u64, hue_bits: u64, z_layer: ZIndex) -> Self {
        Self::new(
            PipelineId::ColorPickerSv,
            clip_hash,
            Some(hue_bits),
            z_layer,
        )
    }

    /// SPEC-HAL-278: Batch key for a radial gauge.
    pub fn radial_gauge(clip_hash: u64, param_hash: u64, z_layer: ZIndex) -> Self {
        Self::new(
            PipelineId::RadialGauge,
            clip_hash,
            Some(param_hash),
            z_layer,
        )
    }

    /// SPEC-R-022: Create a key for SDF rounded rect primitives.
    pub fn sdf_rect(clip_hash: u64, z_layer: ZIndex) -> Self {
        Self::new(PipelineId::SdfRect, clip_hash, None, z_layer)
    }

    /// SPEC-R-003: Create a key for shape primitives
    pub fn shape(clip_hash: u64, shape_type_id: u8, z_layer: ZIndex) -> Self {
        Self {
            z_layer,
            control_subtree_order: None,
            clip_hash,
            pipeline: PipelineId::Shape,
            texture_id: None,
            image_generation: None,
            shape_type_id: Some(shape_type_id),
        }
    }
}

/// Range within a vertex/index buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferRange {
    /// Start offset in elements (not bytes)
    pub start: u32,
    /// Number of elements
    pub count: u32,
}

impl BufferRange {
    pub fn new(start: u32, count: u32) -> Self {
        Self { start, count }
    }

    pub fn end(&self) -> u32 {
        self.start + self.count
    }
}

/// A single draw batch - represents one GPU draw call
///
/// SPEC-R-001 Stage 2: Uses ClipState as the single source of truth for clip info.
/// SPEC-HAL-081A: scene_key identifies which compositor surface this batch belongs to.
#[derive(Debug, Clone)]
pub struct DrawBatch {
    /// Unique ID for this batch
    pub id: DrawBatchId,
    /// Key determining batch properties
    pub key: DrawBatchKey,
    /// Range of vertices in the vertex buffer
    pub vertex_range: BufferRange,
    /// Range of indices in the index buffer
    pub index_range: BufferRange,
    /// SPEC-R-001 Stage 2: ClipState snapshot for this batch.
    /// Invariant: clip_state.hash == key.clip_hash
    pub clip_state: ClipState,
    /// SPEC-HAL-081A: Compositor surface scene_key this batch belongs to.
    /// Used for surface-ordered submission (INV-COMP-P04).
    /// Typed as SceneKey to guarantee non-zero.
    pub scene_key: SceneKey,
}

impl DrawBatch {
    /// Create a new batch with explicit scene_key.
    ///
    /// SPEC-HAL-081A: The scene_key MUST come from the compositor snapshot.
    /// There is no default - the caller must provide the scene_key explicitly.
    pub fn new(
        id: DrawBatchId,
        key: DrawBatchKey,
        vertex_range: BufferRange,
        index_range: BufferRange,
        clip_state: ClipState,
        scene_key: SceneKey,
    ) -> Self {
        Self {
            id,
            key,
            vertex_range,
            index_range,
            clip_state,
            scene_key,
        }
    }
}

/// Flattened primitive ready for batching
///
/// SPEC-R-001 Stage 2: Each primitive carries a ClipState snapshot captured
/// at insertion time. This ensures clip_state.hash == key.clip_hash.
///
/// SPEC-R-002: Each primitive has an order_key for deterministic intra-batch sorting.
#[derive(Debug, Clone)]
pub struct FlatPrimitive {
    /// Batch key for this primitive
    pub key: DrawBatchKey,
    /// Primitive data
    pub data: PrimitiveData,
    /// SPEC-R-001 Stage 2: ClipState snapshot at primitive insertion time.
    /// Invariant: clip_state.hash == key.clip_hash
    pub clip_state: ClipState,
    /// SPEC-R-002: Monotonically increasing insertion order key.
    /// Used for deterministic ordering within batches.
    /// NOT part of DrawBatchKey - only used for intra-batch sorting.
    pub order_key: u32,
}

/// Primitive vertex/index data
#[derive(Debug, Clone)]
pub enum PrimitiveData {
    /// HAL385: an order barrier, not ordinary geometry or an Image binding.
    FrostedGlass {
        rect: RectF,
        corners: crate::core::paint::RoundedCorners,
        appearance: crate::core::paint::FrostedGlassAppearance,
        ancestor: Option<(RectF,[f32;4])>,
        ordinal: u64,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
    },
    /// HAL386: one ordinal per actual occurrence, never an Image identity/hash.
    AnalyticSurface {
        rect: RectF,
        corners: crate::core::paint::RoundedCorners,
        material: crate::core::paint::SurfaceAnalytic,
        ancestor: Option<(RectF, [f32; 4])>,
        ordinal: u64,
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
    },
    LocalAmbientGlow {
        rect: RectF,
        bounds: RectF,
        appearance: crate::core::paint::LocalAmbientGlowAppearance,
    },
    /// Rectangle: 4 vertices, 6 indices
    Rect { rect: RectF, color: Color },
    /// Border rectangle: fill + stroke with optional rounded corners.
    /// INV-BORDER-GEOMETRY-01: Border conforms to shape geometry.
    BorderRect {
        rect: RectF,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
    },
    /// Rounded rectangle.
    /// SPEC-HAL-085: Routes through rect pipeline with shader-based corner AA.
    /// Shape silhouette is computed in fragment shader, not tessellated geometry.
    RoundedRect {
        rect: RectF,
        color: Color,
        radius: f32,
    },
    /// Text with optional background spans (selection/caret)
    /// SPEC-EXIT-A1: backgrounds rendered before glyphs via downsample_u sentinel
    Text {
        binding: Option<crate::core::text_engine::TextBinding>,
        position: crate::core::math::Point,
        text: String,
        font_size: f32,
        color: Color,
        /// SPEC-EXIT-A1: Background spans for selection/caret (rendered with downsample_u < 0)
        backgrounds: Vec<TextBackgroundSpan>,
    },
    /// Image quad
    Image {
        rect: RectF,
        texture_id: u64,
        uv_rect: RectF,
        tint: Color,
    },
    // SPEC-R-003: Shape primitives (Stage 1 scaffold - no rendering yet)
    /// Circle shape
    Circle {
        center: crate::core::math::Point,
        radius: f32,
        color: Color,
    },
    /// Ellipse shape
    Ellipse {
        center: crate::core::math::Point,
        radii: crate::core::math::Point,
        color: Color,
    },
    /// Line segment
    Line {
        a: crate::core::math::Point,
        b: crate::core::math::Point,
        width: f32,
        color: Color,
    },
    /// Polyline (connected line segments)
    Polyline {
        points: Vec<crate::core::math::Point>,
        width: f32,
        color: Color,
    },
    /// SPEC-HAL-358: Shared contour geometry plus inline front ranges.
    LuminousContour {
        points: Arc<[Point]>,
        core_width: f32,
        color: Color,
        appearance: LuminousContourAppearance,
        fronts: LuminousContourFronts,
        total_length: f32,
        scroll_offset: (f32, f32),
    },
    /// SPEC-R-013-v2: Shadow effect (drop, inner, ring)
    Shadow {
        /// Expanded quad rect (includes blur expansion)
        quad_rect: RectF,
        /// Shadow color (premultiplied RGBA)
        color: Color,
        /// Shadow offset for SDF evaluation
        offset: (f32, f32),
        /// Shadow params for per-batch uniform
        params: ShadowParamsData,
    },
    /// SPEC-R-012-v2: Gradient-filled rectangle
    GradientRect {
        rect: RectF,
        radius: f32,
        params: GradientParamsData,
    },
    /// SPEC-HAL-272: Saturation/value square for the color picker.
    /// `rect`   — bounding rect in logical pixels
    /// `hue`    — hue in 0..1
    /// `radius` — corner radius in logical pixels (0.0 = sharp)
    ColorPickerSv { rect: RectF, hue: f32, radius: f32 },
    /// SPEC-HAL-278: Radial gauge arc.
    RadialGauge {
        rect: RectF,
        fill: f32,
        thickness: f32,
        track_color: [f32; 4],
        fill_color: [f32; 4],
        fill_color_negative: [f32; 4],
        start_angle: f32,
        sweep_angle: f32,
        origin_fill: f32,
        mode: u32,
        fill_style: u32,
        zone_count: u32,
        zones: [(f32, f32, [f32; 4]); 4],
        major_divisions: u32,
        minor_per_major: u32,
        tick_length_major: f32,
        tick_length_minor: f32,
        tick_color: [f32; 4],
    },
    /// SPEC-R-022: SDF-evaluated rounded rect (fill + optional border).
    /// Shape evaluated analytically in fragment shader — no tessellation.
    SdfRect {
        /// Expanded quad rect (rect + 1px AA padding on each side)
        quad_rect: RectF,
        fill_color: Color,
        border_color: Color,
        /// Rect center for SDF evaluation (in logical coords)
        rect_center: (f32, f32),
        /// Rect half-size for SDF evaluation
        rect_half_size: (f32, f32),
        radius: f32,
        border_width: f32,
    },
}

/// SPEC-R-012-v2: Gradient parameters for per-batch uniform binding.
/// Gradients with identical params batch together.
#[derive(Debug, Clone)]
pub struct GradientParamsData {
    pub rect_origin: (f32, f32),
    pub rect_size: (f32, f32),
    pub p0: (f32, f32),
    pub p1: (f32, f32),
    pub corner_radius: f32,
    pub kind: u32,
    pub stops: Vec<GradientStopData>,
}

#[derive(Debug, Clone)]
pub struct GradientStopData {
    /// Linear premultiplied RGBA
    pub color: [f32; 4],
    pub offset: f32,
}

impl GradientParamsData {
    pub fn batch_hash_with_clip(&self, clip_hash: u64) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.batch_hash().hash(&mut hasher);
        clip_hash.hash(&mut hasher);
        hasher.finish()
    }

    pub fn batch_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.rect_origin.0.to_bits().hash(&mut hasher);
        self.rect_origin.1.to_bits().hash(&mut hasher);
        self.rect_size.0.to_bits().hash(&mut hasher);
        self.rect_size.1.to_bits().hash(&mut hasher);
        self.p0.0.to_bits().hash(&mut hasher);
        self.p0.1.to_bits().hash(&mut hasher);
        self.p1.0.to_bits().hash(&mut hasher);
        self.p1.1.to_bits().hash(&mut hasher);
        self.corner_radius.to_bits().hash(&mut hasher);
        self.kind.hash(&mut hasher);
        self.stops.len().hash(&mut hasher);
        for stop in &self.stops {
            stop.offset.to_bits().hash(&mut hasher);
            stop.color[0].to_bits().hash(&mut hasher);
            stop.color[1].to_bits().hash(&mut hasher);
            stop.color[2].to_bits().hash(&mut hasher);
            stop.color[3].to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// SPEC-R-013-v2: Shadow parameters for per-batch uniform binding.
/// Shadows with identical params batch together.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowParamsData {
    pub shape_center: (f32, f32),
    pub shape_half_size: (f32, f32),
    pub shape_radius: f32,
    pub sigma: f32,
    pub mode: u32,
    pub spread: f32,
}

impl ShadowParamsData {
    /// Compute a hash for batch key grouping.
    /// Shadows with identical params can batch together.
    pub fn batch_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.shape_center.0.to_bits().hash(&mut hasher);
        self.shape_center.1.to_bits().hash(&mut hasher);
        self.shape_half_size.0.to_bits().hash(&mut hasher);
        self.shape_half_size.1.to_bits().hash(&mut hasher);
        self.shape_radius.to_bits().hash(&mut hasher);
        self.sigma.to_bits().hash(&mut hasher);
        self.mode.hash(&mut hasher);
        self.spread.to_bits().hash(&mut hasher);
        hasher.finish()
    }
}

/// Collection of batches for a single frame
#[derive(Debug, Default, Clone)]
pub struct FrameBatches {
    /// Ordered list of batches (render in order)
    pub batches: Vec<DrawBatch>,
    /// Total rect vertex count across all rect batches
    pub total_vertices: u32,
    /// Total rect index count across all rect batches
    pub total_indices: u32,
    /// SPEC-HAL-263: Total image vertex count across all image batches
    pub total_image_vertices: u32,
    /// SPEC-HAL-263: Total image index count across all image batches
    pub total_image_indices: u32,
    /// SPEC-R-004 Stage 12: Path draws for the path pipeline.
    /// These are handled separately from batched primitives because path rendering
    /// uses a different pipeline with tessellated geometry.
    pub paths: Vec<PathDraw>,
    /// SPEC-R-004 Stage 13: Unique clip rectangles for path rendering.
    /// Index 0 is always the fullscreen clip (no clipping).
    /// PathDraw.clip_id indexes into this array.
    pub path_clips: Vec<RectF>,
    /// Index of first batch with z_layer >= LAYER_OVERLAY (1000).
    /// Batches [0..overlay_start_batch_idx) are underlay (drawn before text).
    /// Batches [overlay_start_batch_idx..] are overlay (drawn after text).
    pub overlay_start_batch_idx: usize,
}

impl FrameBatches {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            total_vertices: 0,
            total_indices: 0,
            total_image_vertices: 0,
            total_image_indices: 0,
            paths: Vec::new(),
            path_clips: Vec::new(),
            overlay_start_batch_idx: 0,
        }
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    /// SPEC-R-004 Stage 12: Returns true if there are path draws to render.
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LocalAmbientGlowClipRefusal {
    pub node_key: Option<crate::core::scene_identity::SceneNodeKey>,
    pub rounded_clip_depth: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AnalyticSurfaceRefusal {
    UnsupportedClip {
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        depth: usize,
    },
    InvalidClip {
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
        depth: usize,
    },
    InvalidTarget {
        node_key: Option<crate::core::scene_identity::SceneNodeKey>,
    },
}

/// Batcher: accumulates primitives and produces optimized draw batches
///
/// SPEC-R-001: The batcher maintains a unified clip stack that tracks the effective
/// clipping region during scene graph traversal. All primitives receive consistent
/// clip state through ClipState snapshots.
///
/// SPEC-R-002: The batcher assigns monotonically increasing order_key values to
/// primitives at insertion time, enabling deterministic intra-batch sorting.
///
/// SPEC-HAL-086 Stage 7: The batcher also maintains a scroll offset stack that applies
/// translation to all primitives within scroll containers.
pub struct Batcher {
    has_bloom: bool,
    bloom_refusal: Option<(Option<crate::core::scene_identity::SceneNodeKey>, crate::renderer::BloomBuildRefusal)>,
    frosted_glass_refusal: Option<(Option<crate::core::scene_identity::SceneNodeKey>,crate::renderer::draw_plan_builder::FrostedGlassBuildRefusal)>,
    frosted_glass_count: usize,
    analytic_surface_refusal: Option<AnalyticSurfaceRefusal>,
    analytic_surface_count: usize,
    local_ambient_glow_refusal: Option<LocalAmbientGlowClipRefusal>,
    control_subtree_order: Option<Arc<[ControlSubtreeMember]>>,
    /// Flattened primitives collected during scene traversal
    primitives: Vec<FlatPrimitive>,
    /// SPEC-R-001: Stack of effective clip rectangles.
    /// Each entry is the intersection of all parent clips up to that point.
    /// SPEC-R-020 Stage 3: Now stores rect + optional rounded corners.
    clip_stack: Vec<ClipStackEntry>,
    /// SPEC-R-001: Current clip state (snapshot of stack top + hash).
    /// Updated on every push_clip/pop_clip operation.
    current_clip_state: ClipState,
    /// SPEC-HAL-086 Stage 7: Stack of scroll offsets for nested scroll containers.
    /// Each entry is a single scroll container's offset.
    scroll_offset_stack: Vec<(f32, f32)>,
    /// SPEC-HAL-086 Stage 7: Current cumulative scroll offset (sum of stack).
    /// Applied to all primitive positions during insertion.
    current_scroll_offset: (f32, f32),
    /// Next batch ID
    next_batch_id: u32,
    /// SPEC-R-002: Next order key to assign to primitives.
    /// Incremented after each primitive insertion.
    next_order_key: u32,
    /// SPEC-R-004 Stage 18: Next insertion order for path draws.
    /// Incremented after each path insertion for deterministic ordering.
    next_path_order: u32,
    /// SPEC-R-004 Stage 12: Accumulated path draws.
    /// Path draws are kept separate from batched primitives because they use
    /// a different pipeline with tessellated geometry.
    path_draws: Vec<PathDraw>,
    /// SPEC-R-004 Stage 13: Maps ClipState.hash → clip_id for path rendering.
    /// Index 0 is reserved for "no clip" (fullscreen).
    path_clip_registry: HashMap<u64, u32>,
    /// SPEC-R-004 Stage 13: Ordered list of unique clip rects for path rendering.
    /// Index corresponds to clip_id. Index 0 is fullscreen (placeholder).
    path_clip_rects: Vec<RectF>,
    /// SPEC-HAL-081A: Current compositor surface scene_key.
    /// All batches and path draws created during this frame will be tagged with this key.
    ///
    /// **Contract Strictness**: This is set via `begin_frame(scene_key)` and is mandatory.
    /// The scene_key MUST come from the compositor snapshot, not from defaults.
    /// The type is non-optional - "unset scene key" is unrepresentable by construction.
    current_scene_key: SceneKey,
}

impl Batcher {
    /// Create a new Batcher.
    ///
    /// # SPEC-HAL-081A Visibility
    ///
    /// This is `pub(crate)` to prevent external crates from constructing batchers.
    /// Only `DrawPlanBuilder` within the renderer crate should create batchers.
    pub(crate) fn new() -> Self {
        Self {
            has_bloom: false,
            bloom_refusal: None,
            frosted_glass_refusal: None,
            frosted_glass_count: 0,
            control_subtree_order: None,
            analytic_surface_refusal: None,
            analytic_surface_count: 0,
            local_ambient_glow_refusal: None,
            primitives: Vec::new(),
            clip_stack: Vec::new(),
            current_clip_state: ClipState::none(),
            scroll_offset_stack: Vec::new(),
            current_scroll_offset: (0.0, 0.0),
            next_batch_id: 0,
            next_order_key: 0,
            next_path_order: 0,
            path_draws: Vec::new(),
            path_clip_registry: HashMap::new(),
            path_clip_rects: Vec::new(),
            // SPEC-HAL-081A: Placeholder - will be overwritten by begin_frame(scene_key)
            current_scene_key: SceneKey::BASE,
        }
    }

    /// Clear state for a new frame and set the compositor scene_key.
    ///
    /// # SPEC-HAL-081A: Mandatory SceneKey by Construction
    ///
    /// The `scene_key` parameter is required - there is no way to begin a frame
    /// without providing a valid SceneKey from the compositor snapshot.
    /// This makes "unset scene key" unrepresentable at the type level.
    ///
    /// # SPEC-HAL-163: Surface Isolation Guarantee
    ///
    /// This method performs a **total reset** of all batcher state. This is critical
    /// for SPEC-HAL-163 surface-scoped scene partitioning: when processing multiple
    /// surfaces, `DrawPlanBuilder` calls `begin_frame()` once per surface. The total
    /// reset ensures **zero cross-surface state leakage**.
    ///
    /// **Invariant (INV-BATCHER-RESET):** After `begin_frame()` returns, all mutable
    /// state is equivalent to a freshly constructed `Batcher`. Specifically:
    /// - `primitives` is empty
    /// - `clip_stack` is empty, `current_clip_state` is `ClipState::none()`
    /// - `scroll_offset_stack` is empty, `current_scroll_offset` is `(0.0, 0.0)`
    /// - `next_batch_id`, `next_order_key`, `next_path_order` are all `0`
    /// - `path_draws`, `path_clip_registry`, `path_clip_rects` are all empty
    ///
    /// **WARNING:** Adding new fields to `Batcher` requires updating this method
    /// to reset them. Failure to do so violates INV-BATCHER-RESET and will cause
    /// cross-surface contamination.
    ///
    /// # SPEC-HAL-081A Visibility
    ///
    /// This method is `pub(super)` to enforce that only `DrawPlanBuilder`
    /// (within the renderer module) can drive the batcher lifecycle.
    /// External code must go through the compositor → DrawPlanBuilder path.
    pub(super) fn begin_frame(&mut self, scene_key: SceneKey) {
        self.has_bloom = false;
        self.bloom_refusal = None;
        self.frosted_glass_refusal=None;
        self.frosted_glass_count=0;
        self.analytic_surface_refusal = None;
        self.analytic_surface_count = 0;
        self.local_ambient_glow_refusal = None;
        self.control_subtree_order = None;
        self.primitives.clear();
        self.clip_stack.clear();
        self.current_clip_state = ClipState::none();
        // SPEC-HAL-086 Stage 7: Clear scroll offset state
        self.scroll_offset_stack.clear();
        self.current_scroll_offset = (0.0, 0.0);
        self.next_batch_id = 0;
        self.next_order_key = 0;
        self.next_path_order = 0;
        self.path_draws.clear();
        self.path_clip_registry.clear();
        self.path_clip_rects.clear();
        // SPEC-HAL-081A: Scene key is mandatory - set from compositor snapshot
        self.current_scene_key = scene_key;

        // SPEC-HAL-163: Debug assertion guarding INV-BATCHER-RESET
        debug_assert!(
            self.primitives.is_empty(),
            "INV-BATCHER-RESET violated: primitives not cleared"
        );
        debug_assert!(
            self.clip_stack.is_empty(),
            "INV-BATCHER-RESET violated: clip_stack not cleared"
        );
        debug_assert!(
            self.scroll_offset_stack.is_empty(),
            "INV-BATCHER-RESET violated: scroll_offset_stack not cleared"
        );
        debug_assert!(
            self.path_draws.is_empty(),
            "INV-BATCHER-RESET violated: path_draws not cleared"
        );
        debug_assert!(
            self.path_clip_registry.is_empty(),
            "INV-BATCHER-RESET violated: path_clip_registry not cleared"
        );
        debug_assert!(
            self.path_clip_rects.is_empty(),
            "INV-BATCHER-RESET violated: path_clip_rects not cleared"
        );
    }

    /// SPEC-HAL-081A: Get the current compositor surface scene_key.
    ///
    /// The scene_key is always valid - it's set during `begin_frame()` and
    /// there is no "unset" state by construction.
    pub(super) fn current_scene_key(&self) -> SceneKey {
        self.current_scene_key
    }

    /// SPEC-R-001: Push a clip region onto the stack.
    ///
    /// The new clip is intersected with the current effective clip.
    /// If the intersection is empty, a zero-area rect is pushed (not skipped).
    ///
    /// SPEC-R-020 Stage 3: Added rounded parameter for rounded clipping.
    /// - Rounded corners are NOT intersected (only rects are)
    /// - Only the topmost rounded clip is tracked
    ///
    /// ## Debug Assertions
    ///
    /// - Clip rect must be finite (no NaN or infinite)
    /// - Stack depth must not exceed MAX_CLIP_STACK_DEPTH
    /// - New clip must shrink or equal the previous clip area
    pub fn push_clip(&mut self, rect: RectF, rounded: Option<crate::core::scene::RoundedCorners>) {
        // SPEC-HAL-086: Apply scroll offset to clip rect.
        // Clips inside a ScrollNode are in content space; shift to screen space
        // so they match the scroll-adjusted drawing primitives.
        // Outside ScrollNode, current_scroll_offset is (0,0) → no-op.
        let rect = self.apply_scroll_to_rect(rect);

        // Debug: validate clip rect is finite
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                // HAL386 final rounded-clip admission must return its typed Build
                // refusal rather than abort before the analytic leaf is visited.
                // Legacy ClipState intersection/projection is unchanged.
                rounded.is_some()
                    || (rect.x.is_finite()
                        && rect.y.is_finite()
                        && rect.width.is_finite()
                        && rect.height.is_finite()),
                "SPEC-R-001: Clip rect must be finite, got {:?}",
                rect
            );
            debug_assert!(
                self.clip_stack.len() < MAX_CLIP_STACK_DEPTH,
                "SPEC-R-001: Clip stack depth exceeded {} (current: {})",
                MAX_CLIP_STACK_DEPTH,
                self.clip_stack.len()
            );
        }

        // Compute effective clip as intersection with parent
        let effective_clip = if let Some(current) = self.current_clip() {
            // SPEC-R-001: Use RectF::intersection, produce zero-area on empty
            current.intersection(&rect).unwrap_or(RectF::zero())
        } else {
            rect
        };

        // Debug: verify shrinking invariant (new area <= old area)
        #[cfg(debug_assertions)]
        if let Some(current) = self.current_clip() {
            let current_area = current.width * current.height;
            let new_area = effective_clip.width * effective_clip.height;
            debug_assert!(
                new_area <= current_area + f32::EPSILON,
                "SPEC-R-001: Push clip must shrink or equal area (was {}, now {})",
                current_area,
                new_area
            );
        }

        // SPEC-R-020 Stage 3: Push clip with optional rounded corners
        self.clip_stack.push(ClipStackEntry {
            rect: effective_clip,
            source_rect: rect,
            rounded,
        });
        self.update_clip_state();
    }

    /// SPEC-R-001: Pop the top clip region from the stack.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.update_clip_state();
    }

    /// SPEC-HAL-086 Stage 7: Push a scroll offset onto the stack.
    ///
    /// The offset is added to the cumulative scroll offset. All primitives
    /// added while this offset is on the stack will have their positions
    /// shifted by -offset (scroll moves content up/left).
    pub fn push_scroll(&mut self, offset: (f32, f32)) {
        self.scroll_offset_stack.push(offset);
        self.current_scroll_offset.0 += offset.0;
        self.current_scroll_offset.1 += offset.1;
    }

    /// SPEC-HAL-086 Stage 7: Pop the top scroll offset from the stack.
    pub fn pop_scroll(&mut self) {
        if let Some(offset) = self.scroll_offset_stack.pop() {
            self.current_scroll_offset.0 -= offset.0;
            self.current_scroll_offset.1 -= offset.1;
        }
    }

    /// SPEC-HAL-086 Stage 7: Get current cumulative scroll offset.
    pub fn current_scroll(&self) -> (f32, f32) {
        self.current_scroll_offset
    }

    /// SPEC-HAL-086 Stage 7: Apply scroll offset to a rectangle.
    /// Content is shifted by -offset (scroll moves content up/left).
    fn apply_scroll_to_rect(&self, rect: RectF) -> RectF {
        let (scroll_x, scroll_y) = self.current_scroll_offset;
        RectF::new(
            rect.x - scroll_x,
            rect.y - scroll_y,
            rect.width,
            rect.height,
        )
    }

    /// SPEC-HAL-086 Stage 7: Apply scroll offset to a point.
    fn apply_scroll_to_point(&self, point: Point) -> Point {
        let (scroll_x, scroll_y) = self.current_scroll_offset;
        Point::new(point.x - scroll_x, point.y - scroll_y)
    }

    /// Get current clip rectangle (None if no clipping active)
    pub fn current_clip(&self) -> Option<RectF> {
        self.clip_stack.last().map(|entry| entry.rect)
    }

    /// SPEC-R-001: Get current clip state (rect + hash snapshot).
    pub fn clip_state(&self) -> ClipState {
        self.current_clip_state
    }

    /// Get current clip hash (for batch key generation).
    /// SPEC-R-001: This must always come from ClipState.hash.
    pub fn clip_hash(&self) -> u64 {
        self.current_clip_state.hash
    }

    /// SPEC-R-002: Get the next order key and increment the counter.
    fn next_order_key(&mut self) -> u32 {
        let key = self.next_order_key;
        self.next_order_key += 1;
        key
    }

    /// SPEC-R-001: Update the clip state from the current stack.
    ///
    /// Hash computation is deterministic:
    /// - Depends only on stack depth and each rect's exact f32 bit patterns
    /// - SPEC-R-020 Stage 3: Also hashes rounded corner values if present
    /// - Uses DefaultHasher for cross-platform consistency
    fn update_clip_state(&mut self) {
        if self.clip_stack.is_empty() {
            self.current_clip_state = ClipState::none();
            return;
        }

        let mut hasher = DefaultHasher::new();

        // Hash stack depth first (ensures different depths produce different hashes)
        self.clip_stack.len().hash(&mut hasher);

        // Hash each rect's components using bit representation for exact matching
        for entry in &self.clip_stack {
            entry.rect.x.to_bits().hash(&mut hasher);
            entry.rect.y.to_bits().hash(&mut hasher);
            entry.rect.width.to_bits().hash(&mut hasher);
            entry.rect.height.to_bits().hash(&mut hasher);
        }

        // SPEC-R-020 Stage 3: Hash rounded corners of the topmost clip
        let top_entry = self.clip_stack.last().unwrap();
        if let Some(rounded) = top_entry.rounded {
            // Hash each corner radius
            rounded.tl.to_bits().hash(&mut hasher);
            rounded.tr.to_bits().hash(&mut hasher);
            rounded.bl.to_bits().hash(&mut hasher);
            rounded.br.to_bits().hash(&mut hasher);
        }

        let hash = hasher.finish();
        let rect = top_entry.rect;
        let rounded = top_entry.rounded;

        self.current_clip_state = ClipState::with_rounded(rect, rounded, hash);
    }

    /// Add a rectangle primitive
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Applies current scroll offset to position.
    pub fn add_rect(&mut self, rect: RectF, color: Color, z_index: ZIndex) {
        // SPEC-HAL-086 Stage 7: Apply scroll offset (content shifts by -offset)
        let scrolled_rect = self.apply_scroll_to_rect(rect);

        let clip_state = self.clip_state();
        let key = DrawBatchKey::rect(clip_state.hash, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::Rect {
                rect: scrolled_rect,
                color,
            },
            clip_state,
            order_key,
        });
    }

    /// Add a border rectangle primitive
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Applies current scroll offset to position.
    /// Add a border rectangle with optional rounded corners.
    ///
    /// INV-BORDER-GEOMETRY-01: Border visuals conform to shape geometry.
    /// When radius > 0, border is rendered as rounded ring (outer - inner).
    pub fn add_border_rect(
        &mut self,
        rect: RectF,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        z_index: ZIndex,
    ) {
        let scrolled_rect = self.apply_scroll_to_rect(rect);
        let clip_state = self.clip_state();
        let key = DrawBatchKey::rect(clip_state.hash, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::BorderRect {
                rect: scrolled_rect,
                fill_color,
                border_color,
                border_width,
                radius,
            },
            clip_state,
            order_key,
        });
    }

    /// Add a rounded rectangle primitive.
    ///
    /// SPEC-R-022: RoundedRect routes through the SDF rect pipeline for actual rounded corners.
    /// CPU tessellation via path is forbidden for rounded rects.
    ///
    /// SDF rect parameters are emitted in scroll-adjusted viewport space.
    /// SPEC-HAL-086 Stage 7: Scroll applied by add_rounded_rect or add_rect (fallback).
    pub fn add_rounded_rect(
        &mut self,
        rect: RectF,
        color: Color,
        radius: f32,
        z_index: ZIndex,
        _mesh_cache: &mut MeshCache,
        _scale_factor: f32,
        _current_frame: u64,
    ) {
        if color.a < 0.001 {
            return;
        }

        if radius <= 0.0 {
            // No radius: use rect pipeline (fast path)
            // Do NOT apply scroll here — add_rect applies it internally.
            self.add_rect(rect, color, z_index);
            return;
        }

        let scrolled_rect = self.apply_scroll_to_rect(rect);

        // Expand quad by 1px for AA blending room
        let expand = 1.0;
        let quad_rect = RectF::new(
            scrolled_rect.x - expand,
            scrolled_rect.y - expand,
            scrolled_rect.width + expand * 2.0,
            scrolled_rect.height + expand * 2.0,
        );

        let cx = scrolled_rect.x + scrolled_rect.width / 2.0;
        let cy = scrolled_rect.y + scrolled_rect.height / 2.0;
        let hx = scrolled_rect.width / 2.0;
        let hy = scrolled_rect.height / 2.0;

        let fill_premul = Color::premul(color.r, color.g, color.b, color.a);

        let clip_state = self.clip_state();
        let key = DrawBatchKey::sdf_rect(clip_state.hash, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::SdfRect {
                quad_rect,
                fill_color: fill_premul,
                border_color: Color::TRANSPARENT,
                rect_center: (cx, cy),
                rect_half_size: (hx, hy),
                radius,
                border_width: 0.0,
            },
            clip_state,
            order_key,
        });
    }

    /// Add a rounded border (fill + optional border) via the SDF rect pipeline.
    ///
    /// SPEC-R-022: Renders fill + border analytically in fragment shader.
    /// INV-BORDER-GEOMETRY-01: Border visuals conform to shape geometry.
    pub fn add_rounded_border(
        &mut self,
        rect: RectF,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        z_index: ZIndex,
        _mesh_cache: &mut MeshCache,
        _scale_factor: f32,
        _current_frame: u64,
    ) {
        if fill_color.a < 0.001 && border_color.a < 0.001 {
            return;
        }
        if fill_color.a < 0.001 && border_width <= 0.0 {
            return;
        }

        if radius <= 0.0 && border_width <= 0.0 {
            // No radius, no border: plain rect
            self.add_rect(rect, fill_color, z_index);
            return;
        }

        if radius <= 0.0 {
            // Sharp corners with border: existing rect border pipeline
            self.add_border_rect(rect, fill_color, border_color, border_width, 0.0, z_index);
            return;
        }

        let scrolled_rect = self.apply_scroll_to_rect(rect);

        // Expand quad by 1px for AA + border outer edge
        let expand = 1.0;
        let quad_rect = RectF::new(
            scrolled_rect.x - expand,
            scrolled_rect.y - expand,
            scrolled_rect.width + expand * 2.0,
            scrolled_rect.height + expand * 2.0,
        );

        let cx = scrolled_rect.x + scrolled_rect.width / 2.0;
        let cy = scrolled_rect.y + scrolled_rect.height / 2.0;
        let hx = scrolled_rect.width / 2.0;
        let hy = scrolled_rect.height / 2.0;

        // Premultiply colors
        let fill_premul = Color::premul(fill_color.r, fill_color.g, fill_color.b, fill_color.a);
        let border_premul = Color::premul(
            border_color.r,
            border_color.g,
            border_color.b,
            border_color.a,
        );

        let clip_state = self.clip_state();
        let key = DrawBatchKey::sdf_rect(clip_state.hash, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::SdfRect {
                quad_rect,
                fill_color: fill_premul,
                border_color: border_premul,
                rect_center: (cx, cy),
                rect_half_size: (hx, hy),
                radius,
                border_width,
            },
            clip_state,
            order_key,
        });
    }

    /// Add a text primitive with optional background spans.
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Applies current scroll offset to position.
    /// SPEC-EXIT-A1: Background spans rendered before glyphs via downsample_u sentinel.
    pub fn add_text(
        &mut self,
        position: crate::core::math::Point,
        text: String,
        font_size: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: ZIndex,
    ) {
        self.add_bound_text(position, text, font_size, color, backgrounds, z_index, None);
    }

    fn add_bound_text(
        &mut self,
        position: crate::core::math::Point,
        text: String,
        font_size: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: ZIndex,
        binding: Option<crate::core::text_engine::TextBinding>,
    ) {
        let scrolled_position = self.apply_scroll_to_point(position);
        // Text uses a fixed texture_id of 0 for now (glyph atlas)
        let clip_state = self.clip_state();
        let key = DrawBatchKey::text(clip_state.hash, 0, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::Text {
                binding,
                position: scrolled_position,
                text,
                font_size,
                color,
                backgrounds,
            },
            clip_state,
            order_key,
        });
    }

    /// Add an image primitive
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Applies current scroll offset to position.
    pub fn add_image(
        &mut self,
        rect: RectF,
        texture_id: u64,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
    ) {
        let scrolled_rect = self.apply_scroll_to_rect(rect);
        let clip_state = self.clip_state();
        let key = DrawBatchKey::image(clip_state.hash, texture_id, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::Image {
                rect: scrolled_rect,
                texture_id,
                uv_rect,
                tint,
            },
            clip_state,
            order_key,
        });
    }

    fn add_admitted_image(
        &mut self,
        rect: RectF,
        identity: crate::renderer::image_binding::ImageBindingIdentity,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
    ) {
        self.add_image(rect, identity.texture_id(), uv_rect, tint, z_index);
        self.primitives
            .last_mut()
            .expect("new Image primitive")
            .key
            .image_generation = Some(identity.generation());
    }

    // SPEC-R-003: Shape primitive methods.

    /// Add a circle shape primitive
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Scroll is applied by add_path.
    pub fn add_circle(
        &mut self,
        center: crate::core::math::Point,
        radius: f32,
        color: Color,
        z_index: ZIndex,
    ) {
        if color.a < 0.001 || radius <= 0.0 {
            return;
        }

        let path = ellipse_path(center, radius, radius);
        let mesh = path.to_mesh();
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }

        let color_arr = [color.r, color.g, color.b, color.a];
        let path_draw = PathDraw::fill(mesh, color_arr, self.current_scene_key());
        self.add_path(path_draw, z_index);
    }

    /// Add an ellipse shape primitive
    ///
    /// SPEC-R-001 Stage 2: Captures current ClipState snapshot.
    /// SPEC-R-002: Assigns monotonically increasing order_key.
    /// SPEC-HAL-086 Stage 7: Scroll is applied by add_path.
    pub fn add_ellipse(
        &mut self,
        center: crate::core::math::Point,
        radii: crate::core::math::Point,
        color: Color,
        z_index: ZIndex,
    ) {
        if color.a < 0.001 || radii.x <= 0.0 || radii.y <= 0.0 {
            return;
        }

        let path = ellipse_path(center, radii.x, radii.y);
        let mesh = path.to_mesh();
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return;
        }

        let color_arr = [color.r, color.g, color.b, color.a];
        let path_draw = PathDraw::fill(mesh, color_arr, self.current_scene_key());
        self.add_path(path_draw, z_index);
    }

    /// Add a line segment — routed through the path stroke pipeline.
    pub fn add_line(
        &mut self,
        a: crate::core::math::Point,
        b: crate::core::math::Point,
        width: f32,
        color: Color,
        z_index: ZIndex,
    ) {
        if color.a < 0.001 || width <= 0.0 {
            return;
        }

        // Build a line segment path (unscrolled — add_path applies scroll)
        let mut path = Path::new();
        path.move_to(a.x, a.y);
        path.line_to(b.x, b.y);

        let stroke_params = StrokeParams {
            width,
            join: StrokeJoin::Miter,
            cap: StrokeCap::Butt,
            miter_limit: 4.0,
        };

        if let Some(stroke_mesh) = path.build_stroke_mesh(stroke_params) {
            let path_mesh = PathMesh {
                vertices: stroke_mesh.vertices,
                indices: stroke_mesh.indices,
                bounds: stroke_mesh.bounds,
            };
            let color_arr = [color.r, color.g, color.b, color.a];
            let path_draw = PathDraw::fill(path_mesh, color_arr, self.current_scene_key());
            self.add_path(path_draw, z_index);
        }
    }

    /// Add a polyline — routed through the path stroke pipeline.
    pub fn add_polyline(
        &mut self,
        points: Vec<crate::core::math::Point>,
        width: f32,
        color: Color,
        z_index: ZIndex,
    ) {
        if color.a < 0.001 || width <= 0.0 || points.len() < 2 {
            return;
        }

        // Build a polyline path (unscrolled — add_path applies scroll)
        let mut path = Path::new();
        path.move_to(points[0].x, points[0].y);
        for p in &points[1..] {
            path.line_to(p.x, p.y);
        }

        let stroke_params = StrokeParams {
            width,
            join: StrokeJoin::Miter,
            cap: StrokeCap::Butt,
            miter_limit: 4.0,
        };

        if let Some(stroke_mesh) = path.build_stroke_mesh(stroke_params) {
            let path_mesh = PathMesh {
                vertices: stroke_mesh.vertices,
                indices: stroke_mesh.indices,
                bounds: stroke_mesh.bounds,
            };
            let color_arr = [color.r, color.g, color.b, color.a];
            let path_draw = PathDraw::fill(path_mesh, color_arr, self.current_scene_key());
            self.add_path(path_draw, z_index);
        }
    }

    /// SPEC-HAL-358: Route an explicit luminous contour through ShapePipeline.
    pub fn add_luminous_contour(&mut self, node: &LuminousContourNode) {
        if node.color().a < 0.001 && node.appearance().front_opacity_boost() == 0.0 {
            return;
        }
        let clip_state = self.clip_state();
        let key = DrawBatchKey::shape(
            clip_state.hash,
            ShapeType::LuminousContour as u8,
            node.z_index(),
        );
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::LuminousContour {
                points: node.points().clone(),
                core_width: node.core_width(),
                color: node.color(),
                appearance: node.appearance(),
                fronts: node.fronts(),
                total_length: node.total_length(),
                scroll_offset: self.current_scroll(),
            },
            clip_state,
            order_key,
        });
    }

    /// Final composed-clip refusal, scoped to this surface's current flatten.
    pub(super) fn local_ambient_glow_refusal(&self) -> Option<LocalAmbientGlowClipRefusal> {
        self.local_ambient_glow_refusal
    }

    fn add_local_ambient_glow(&mut self, node: &crate::core::scene::LocalAmbientGlowNode) {
        if !node.appearance().is_visible() {
            return;
        }
        if let Some(index) = self.clip_stack.iter().position(|entry| {
            entry
                .rounded
                .is_some_and(|r| r.tl > 0.0 || r.tr > 0.0 || r.bl > 0.0 || r.br > 0.0)
        }) {
            self.local_ambient_glow_refusal
                .get_or_insert(LocalAmbientGlowClipRefusal {
                    node_key: node.key(),
                    rounded_clip_depth: index + 1,
                });
            return;
        }
        let clip_state = self.clip_state();
        let order_key = self.next_order_key();
        let rect = self.apply_scroll_to_rect(node.rect());
        let bounds = self.apply_scroll_to_rect(node.bounds());
        self.primitives.push(FlatPrimitive {
            key: DrawBatchKey::shape(
                clip_state.hash,
                ShapeType::LocalAmbientGlow as u8,
                node.z_index(),
            ),
            data: PrimitiveData::LocalAmbientGlow {
                rect,
                bounds,
                appearance: node.appearance(),
            },
            clip_state,
            order_key,
        });
    }

    pub(super) fn analytic_surface_refusal(&self) -> Option<AnalyticSurfaceRefusal> {
        self.analytic_surface_refusal
    }
    pub(super) fn analytic_surface_count(&self) -> usize {
        self.analytic_surface_count
    }

    // Capture the same original ancestor for normal glass and the optional optical source.
    fn optical_ancestor(&self, check_rectangular: bool)
        -> Result<Option<(RectF, [f32; 4])>, crate::renderer::BloomBuildRefusal>
    {
        use crate::renderer::BloomBuildRefusal;
        let mut ancestor = None;
        for (i, entry) in self.clip_stack.iter().enumerate() {
            if !check_rectangular && entry.rounded.is_none() {
                continue;
            }
            let raw = entry.source_rect;
            let radii = entry.rounded.map(|r| [r.tl, r.tr, r.bl, r.br]).unwrap_or([0.0; 4]);
            let positive = radii.iter().any(|r| *r > 0.0);
            if ![raw.x, raw.y, raw.width, raw.height, raw.right(), raw.bottom()]
                .into_iter().all(|v| v.is_finite() && v.abs() <= 1_048_576.0)
                || radii.iter().any(|v| !v.is_finite() || *v < 0.0)
                || (positive && (raw.width <= 0.0 || raw.height <= 0.0
                    || radii.iter().any(|v| *v > raw.width.min(raw.height) * 0.5)))
            {
                return Err(BloomBuildRefusal::InvalidClip { depth: i + 1 });
            }
            if positive && ancestor.replace((raw, radii)).is_some() {
                return Err(BloomBuildRefusal::UnsupportedClip { depth: i + 1 });
            }
        }
        Ok(ancestor)
    }

    pub(super) fn bloom_admission(&self, surface: crate::compositor::SurfaceId, scale: f32)
        -> Result<Option<crate::renderer::bloom::BloomOccurrence>,
            (Option<crate::core::scene_identity::SceneNodeKey>, crate::renderer::BloomBuildRefusal)>
    {
        if let Some(ref refusal) = self.bloom_refusal {
            return Err(refusal.clone());
        }
        if !self.has_bloom {
            return Ok(None);
        }
        crate::renderer::bloom::admit(surface, scale, &self.primitives, &self.path_draws, &self.path_clip_rects)
    }

    fn add_analytic_surface(&mut self, node: &crate::core::scene::AnalyticSurfaceNode, scale: f32) {
        use crate::renderer::BloomBuildRefusal as Bloom;
        let key = node.key();
        let rect = self.apply_scroll_to_rect(node.rect());
        let has_bloom = node.material().glass_optics().and_then(|o| o.bloom()).is_some();
        self.has_bloom |= has_bloom;
        let geometry = node.material().validate_geometry(rect, node.corners());
        if geometry == Ok(false) { return; }
        let margin = node.material().halo_extent() + 2.0 / scale;
        let invalid_target = !scale.is_finite()
            || !(0.25..=8.0).contains(&scale)
            || geometry.is_err()
            || [rect.x - margin, rect.y - margin, rect.right() + margin, rect.bottom() + margin]
                .into_iter().any(|v| !v.is_finite() || v.abs() > 1_048_576.0);
        if invalid_target {
            if has_bloom {
                let reason = if geometry == Err(crate::core::paint::AnalyticSurfaceError::UnsupportedBloomOpacity) {
                    Bloom::UnsupportedBodyOpacity
                } else {
                    Bloom::InvalidGeometry
                };
                self.bloom_refusal.get_or_insert((key, reason));
            } else {
                self.analytic_surface_refusal
                    .get_or_insert(AnalyticSurfaceRefusal::InvalidTarget { node_key: key });
            }
            return;
        }
        let ancestor = match self.optical_ancestor(has_bloom) {
            Ok(ancestor) => ancestor,
            Err(reason) if has_bloom => {
                self.bloom_refusal.get_or_insert((key, reason));
                return;
            }
            Err(reason) => {
                self.analytic_surface_refusal.get_or_insert(match reason {
                    Bloom::InvalidClip { depth } => AnalyticSurfaceRefusal::InvalidClip { node_key: key, depth },
                    Bloom::UnsupportedClip { depth } => AnalyticSurfaceRefusal::UnsupportedClip { node_key: key, depth },
                    _ => unreachable!("optical ancestor refusal"),
                });
                return;
            }
        };
        if self.current_clip().is_some_and(|c| c.width <= 0.0 || c.height <= 0.0) {
            if has_bloom { self.bloom_refusal.get_or_insert((key, Bloom::InvalidGeometry)); }
            return;
        }
        // Bound both the ordinal and counter even while rejecting a larger scene.
        if self.analytic_surface_count > 512 { return; }
        self.analytic_surface_count += 1;
        if self.analytic_surface_count > 512 { return; }
        let ordinal = self.analytic_surface_count as u64;
        let clip_state = self.clip_state();
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key: DrawBatchKey::new(PipelineId::AnalyticSurface, clip_state.hash, Some(ordinal), node.z_index()),
            data: PrimitiveData::AnalyticSurface {
                rect,
                corners: node.corners(),
                material: node.material().clone(),
                ancestor,
                ordinal,
                node_key: key,
            },
            clip_state,
            order_key,
        });
    }

    fn add_frosted_glass(&mut self, node: &crate::core::scene::FrostedGlassNode) {
        use crate::renderer::draw_plan_builder::FrostedGlassBuildRefusal as Refusal;
        use crate::renderer::BloomBuildRefusal as Bloom;
        let rect = self.apply_scroll_to_rect(node.rect());
        let key = node.key();
        let has_bloom = node.appearance().glass_optics().and_then(|o| o.bloom()).is_some();
        self.has_bloom |= has_bloom;
        let geometry = node.appearance().validate_geometry(rect, node.corners());
        if geometry == Ok(false) { return; }
        if geometry.is_err() {
            if has_bloom {
                let reason = if geometry == Err(crate::core::paint::FrostedGlassError::UnsupportedBloomOpacity) {
                    Bloom::UnsupportedBodyOpacity
                } else {
                    Bloom::InvalidGeometry
                };
                self.bloom_refusal.get_or_insert((key, reason));
            } else {
                self.frosted_glass_refusal.get_or_insert((key, Refusal::InvalidGeometry));
            }
            return;
        }
        let ancestor = match self.optical_ancestor(true) {
            Ok(ancestor) => ancestor,
            Err(reason) if has_bloom => {
                self.bloom_refusal.get_or_insert((key, reason));
                return;
            }
            Err(reason) => {
                self.frosted_glass_refusal.get_or_insert((key, match reason {
                    Bloom::InvalidClip { depth } => Refusal::InvalidClip { depth },
                    Bloom::UnsupportedClip { depth } => Refusal::UnsupportedClip { depth },
                    _ => unreachable!("optical ancestor refusal"),
                }));
                return;
            }
        };
        if self.current_clip().is_some_and(|r| r.width <= 0.0 || r.height <= 0.0) {
            if has_bloom { self.bloom_refusal.get_or_insert((key, Bloom::InvalidGeometry)); }
            return;
        }
        self.frosted_glass_count = self.frosted_glass_count.saturating_add(1).min(17);
        if self.frosted_glass_count > 16 { return; }
        let ordinal = self.frosted_glass_count as u64;
        let clip_state = self.clip_state();
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key: DrawBatchKey::new(PipelineId::FrostedGlass, clip_state.hash, Some(ordinal), node.z_index()),
            data: PrimitiveData::FrostedGlass {
                rect, corners: node.corners(), appearance: node.appearance().clone(), ancestor, ordinal, node_key: key,
            },
            clip_state,
            order_key,
        });
    }

    pub(crate) fn frosted_glass_admission(&self)->Result<usize,(Option<crate::core::scene_identity::SceneNodeKey>,crate::renderer::draw_plan_builder::FrostedGlassBuildRefusal)> {
        use crate::renderer::draw_plan_builder::FrostedGlassBuildRefusal;
        if let Some(ref error)=self.frosted_glass_refusal {return Err(error.clone());}
        // INV-385-ORDER: no same-effective-order item can move across a marker.
        for (index,p) in self.primitives.iter().enumerate() {
            let PrimitiveData::FrostedGlass{node_key,..}=&p.data else {continue};
            let order=p.key.effective_order();
            if self.primitives.iter().enumerate().any(|(i,other)|i!=index && other.key.effective_order()==order)
                || self.path_draws.iter().any(|path|!path.mesh.vertices.is_empty() && !path.mesh.indices.is_empty() && path.effective_order()==order) {
                return Err((*node_key,FrostedGlassBuildRefusal::AmbiguousOrder));
            }
        }
        Ok(self.frosted_glass_count)
    }

    /// Add a generic shape primitive (placeholder for future expansion)
    ///
    /// TODO (Stage 2+): This method will be expanded as more shape types are added
    #[allow(dead_code)]
    pub fn add_shape(&mut self, _shape_type_id: u8, _z_index: ZIndex) {
        // TODO: Implement shape-specific handling in Stage 2+
        // This is a placeholder for the shape pipeline integration
    }

    /// SPEC-R-013-v2: Add a shadow primitive.
    /// Computes expanded quad and shadow params from ShadowNode data.
    pub fn add_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        mode: crate::core::scene::ShadowMode,
        offset: (f32, f32),
        spread: f32,
        blur_radius: f32,
        color: Color,
        z_index: ZIndex,
    ) {
        use crate::core::scene::ShadowMode;

        let scrolled_rect = self.apply_scroll_to_rect(shape_rect);

        // Compute shape geometry from scrolled rect
        let cx = scrolled_rect.x + scrolled_rect.width / 2.0;
        let cy = scrolled_rect.y + scrolled_rect.height / 2.0;
        let hx = scrolled_rect.width / 2.0;
        let hy = scrolled_rect.height / 2.0;

        let mode_u32 = match mode {
            ShadowMode::Drop => 0,
            ShadowMode::Inner => 1,
            ShadowMode::Ring => 2,
        };

        let sigma = blur_radius.max(0.5);

        let params = ShadowParamsData {
            shape_center: (cx, cy),
            shape_half_size: (hx, hy),
            shape_radius,
            sigma,
            mode: mode_u32,
            spread,
        };

        // Compute expanded quad (room for Gaussian falloff)
        let expansion = match mode {
            ShadowMode::Drop => sigma * 3.0 + spread.abs() + offset.0.abs().max(offset.1.abs()),
            ShadowMode::Inner => 0.0,
            ShadowMode::Ring => spread.abs() + sigma * 3.0,
        };

        let quad_rect = if expansion > 0.0 {
            RectF::new(
                scrolled_rect.x - expansion + offset.0,
                scrolled_rect.y - expansion + offset.1,
                scrolled_rect.width + expansion * 2.0,
                scrolled_rect.height + expansion * 2.0,
            )
        } else {
            // Inner shadow: quad is the shape itself
            scrolled_rect
        };

        let clip_state = self.clip_state();
        let params_hash = params.batch_hash();
        let key = DrawBatchKey::shadow(clip_state.hash, params_hash, z_index);
        let order_key = self.next_order_key();

        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::Shadow {
                quad_rect,
                color,
                offset,
                params,
            },
            clip_state,
            order_key,
        });
    }

    /// SPEC-R-012-v2: Add a gradient rect primitive.
    pub fn add_gradient_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        gradient: &crate::core::paint::GradientDef,
        z_index: ZIndex,
    ) {
        use crate::core::paint;

        let scrolled_rect = self.apply_scroll_to_rect(rect);

        let kind_u32 = match gradient.kind {
            paint::GradientKind::Linear => 0,
            paint::GradientKind::Radial => 1,
        };

        // Color is already linear; premultiply exactly once for interpolation.
        let stops: Vec<GradientStopData> = gradient
            .stops
            .iter()
            .map(|s| {
                let a = s.color.a;
                GradientStopData {
                    color: [s.color.r * a, s.color.g * a, s.color.b * a, a],
                    offset: s.offset,
                }
            })
            .collect();

        let params = GradientParamsData {
            rect_origin: (scrolled_rect.x, scrolled_rect.y),
            rect_size: (scrolled_rect.width, scrolled_rect.height),
            p0: gradient.p0,
            p1: gradient.p1,
            corner_radius: radius,
            kind: kind_u32,
            stops,
        };

        let clip_state = self.clip_state();
        let params_hash = params.batch_hash_with_clip(clip_state.hash);
        let key = DrawBatchKey::gradient(clip_state.hash, params_hash, z_index);
        let order_key = self.next_order_key();

        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::GradientRect {
                rect: scrolled_rect,
                radius,
                params,
            },
            clip_state,
            order_key,
        });
    }

    /// SPEC-HAL-272: Add a color picker SV area primitive.
    pub fn add_color_picker_sv(
        &mut self,
        rect: RectF,
        hue: f32,
        radius: f32,
        z_index: ZIndex,
        scene_key: Option<crate::core::scene_identity::SceneNodeKey>,
    ) {
        let _ = scene_key;
        let rect = self.apply_scroll_to_rect(rect);
        let clip_hash = self.clip_state().hash;
        let hue_bits = hue.to_bits() as u64;
        let key = DrawBatchKey::color_picker_sv(clip_hash, hue_bits, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::ColorPickerSv { rect, hue, radius },
            clip_state: self.clip_state(),
            order_key,
        });
    }

    /// SPEC-HAL-278: Add a radial gauge primitive.
    pub fn add_radial_gauge(
        &mut self,
        rect: RectF,
        fill: f32,
        thickness: f32,
        track_color: [f32; 4],
        fill_color: [f32; 4],
        fill_color_negative: [f32; 4],
        start_angle: f32,
        sweep_angle: f32,
        origin_fill: f32,
        mode: u32,
        fill_style: u32,
        zone_count: u32,
        zones: [(f32, f32, [f32; 4]); 4],
        major_divisions: u32,
        minor_per_major: u32,
        tick_length_major: f32,
        tick_length_minor: f32,
        tick_color: [f32; 4],
        z_index: ZIndex,
        scene_key: Option<crate::core::scene_identity::SceneNodeKey>,
    ) {
        let _ = scene_key;
        let rect = self.apply_scroll_to_rect(rect);
        let clip_hash = self.clip_state().hash;
        let param_hash = {
            let mut h = fill.to_bits() as u64;
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(thickness.to_bits() as u64);
            for &c in &track_color {
                h = h
                    .wrapping_mul(0x517cc1b727220a95)
                    .wrapping_add(c.to_bits() as u64);
            }
            for &c in &fill_color {
                h = h
                    .wrapping_mul(0x517cc1b727220a95)
                    .wrapping_add(c.to_bits() as u64);
            }
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(start_angle.to_bits() as u64);
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(sweep_angle.to_bits() as u64);
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(origin_fill.to_bits() as u64);
            h = h.wrapping_mul(0x517cc1b727220a95).wrapping_add(mode as u64);
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(fill_style as u64);
            h = h
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add(zone_count as u64);
            h
        };
        let key = DrawBatchKey::radial_gauge(clip_hash, param_hash, z_index);
        let order_key = self.next_order_key();
        self.primitives.push(FlatPrimitive {
            key,
            data: PrimitiveData::RadialGauge {
                rect,
                fill,
                thickness,
                track_color,
                fill_color,
                fill_color_negative,
                start_angle,
                sweep_angle,
                origin_fill,
                mode,
                fill_style,
                zone_count,
                zones,
                major_divisions,
                minor_per_major,
                tick_length_major,
                tick_length_minor,
                tick_color,
            },
            clip_state: self.clip_state(),
            order_key,
        });
    }

    /// SPEC-R-004 Stage 13: Get or create a clip_id for the current clip state.
    ///
    /// - If no clip is active (ClipState::is_none()), returns 0 (fullscreen).
    /// - If the clip hash is already registered, returns the existing clip_id.
    /// - Otherwise, registers the clip rect and returns a new clip_id.
    ///
    /// GPU buffer layout:
    /// - Index 0: Fullscreen clip (always present, uploaded by renderer)
    /// - Index 1..N: Registered clip rects from path_clip_rects[0..N-1]
    ///
    /// So clip_id = 1 + path_clip_rects_index.
    fn get_or_create_path_clip_id(&mut self) -> u32 {
        let clip_state = self.clip_state();

        // No clip active → return 0 (fullscreen placeholder)
        if clip_state.is_none() {
            return 0;
        }

        // Check if this clip hash is already registered
        if let Some(&clip_id) = self.path_clip_registry.get(&clip_state.hash) {
            return clip_id;
        }

        // Register new clip rect
        // clip_id starts at 1 because index 0 is reserved for fullscreen
        let clip_id = (self.path_clip_rects.len() + 1) as u32;
        self.path_clip_registry.insert(clip_state.hash, clip_id);
        self.path_clip_rects.push(clip_state.rect);

        clip_id
    }

    /// SPEC-R-004 Stage 12/18: Add a path draw primitive with z-index.
    ///
    /// Path draws are collected separately from batched primitives because they
    /// use a dedicated pipeline with tessellated geometry. The mesh is pre-computed
    /// by the caller (from Path::to_mesh() or Path::build_stroke_mesh()).
    ///
    /// SPEC-R-004 Stage 13: Assigns clip_id based on current clip stack state.
    /// SPEC-R-004 Stage 18: Assigns insertion_order for deterministic sorting.
    /// SPEC-R-004 Stage 19: Ensures bounds is synced from mesh.
    /// SPEC-HAL-086: Applies current scroll offset to transform (same as rects).
    pub fn add_path(&mut self, mut path_draw: PathDraw, z_index: ZIndex) {
        // SPEC-PM-004: Producers must not pre-apply scroll translation.
        // Only translation components are constrained — scale/rotate are allowed.
        // Scroll translation is applied exactly once in add_path.
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                path_draw.transform[2].abs() < 0.001 && path_draw.transform[5].abs() < 0.001,
                "SPEC-PM-004: PathDraw transform contains pre-applied translation \
                 (tx={:.1}, ty={:.1}) before add_path scroll application. \
                 Scroll translation must be applied exactly once in add_path.",
                path_draw.transform[2],
                path_draw.transform[5]
            );
        }
        // Assign clip_id from current clip state
        path_draw.clip_id = self.get_or_create_path_clip_id();
        // Stage 18: Assign z_index and insertion_order for deterministic sorting
        path_draw.z_index = z_index;
        path_draw.insertion_order = self.next_path_order;
        self.next_path_order += 1;
        // SPEC-HAL-086: Apply scroll offset to path transform.
        // Rects shift by -offset via apply_scroll_to_rect(); paths must do the same
        // via the affine transform translation components [m02, m12].
        // Clip rects are already in scroll-adjusted space (push_clip applies scroll),
        // so path vertices must also be in scroll-adjusted space for correct clipping.
        let (scroll_x, scroll_y) = self.current_scroll_offset;
        if scroll_x != 0.0 || scroll_y != 0.0 {
            path_draw.transform[2] -= scroll_x;
            path_draw.transform[5] -= scroll_y;
        }
        // Stage 19: Bounds in scroll-adjusted space (matches transform).
        let mesh_bounds = path_draw.mesh.bounds;
        path_draw.bounds = RectF::new(
            mesh_bounds.x - scroll_x,
            mesh_bounds.y - scroll_y,
            mesh_bounds.width,
            mesh_bounds.height,
        );
        // SPEC-HAL-081A: scene_key is now required at PathDraw construction (typed SceneKey)
        self.path_draws.push(path_draw);
    }

    /// Flatten a scene graph into primitives.
    ///
    /// # SPEC-HAL-081A Visibility
    ///
    /// This method is `pub(super)` to enforce that only `DrawPlanBuilder`
    /// (within the renderer module) can drive scene flattening. External code
    /// must go through the compositor → DrawPlanBuilder path.
    pub(super) fn flatten_scene(
        &mut self,
        nodes: &[SceneNode],
        mesh_cache: &mut MeshCache,
        scale_factor: f32,
        current_frame: u64,
    ) {
        let mut target = None;
        for node in nodes {
            self.flatten_node(node, mesh_cache, scale_factor, current_frame, &mut target);
        }
    }

    pub(super) fn flatten_scene_for_surface(
        &mut self,
        nodes: &[SceneNode],
        mesh_cache: &mut MeshCache,
        scale_factor: f32,
        current_frame: u64,
        surface_id: SurfaceId,
        recorder: &mut ImageOccurrenceRecorder<'_>,
    ) {
        let mut target = Some((surface_id, recorder));
        for node in nodes {
            self.flatten_node(node, mesh_cache, scale_factor, current_frame, &mut target);
        }
    }

    fn resolve_path_mesh(
        node: &PathNode,
        mesh_cache: &mut MeshCache,
        scale_factor: f32,
        current_frame: u64,
    ) -> Result<(PathMesh, Option<FillContours>), PathFillError> {
        if let Some(mesh) = &node.mesh {
            return Ok((mesh.clone(), None));
        }

        FillContours::validate_scale(scale_factor)?;
        let mut contours = node.path.flatten_fill()?;
        let snap_mode = if node.is_stroke {
            PixelSnapMode::Edge
        } else {
            PixelSnapMode::Center
        };
        let key = MeshCacheKey {
            path_hash: node.path.content_hash(),
            render_width_q: MeshCacheKey::quantize_size(contours.bounds.width),
            render_height_q: MeshCacheKey::quantize_size(contours.bounds.height),
            // Actual expanded strokes supply their mesh and bypass this cache.
            stroke_width_q: 0,
            is_fill: !node.is_stroke,
            fill_rule: node.fill_rule,
            dpr_q: MeshCacheKey::quantize_dpr(scale_factor),
            dpr_bits: scale_factor.to_bits(),
            snap_mode,
        };
        let show_contour_edges = !node.is_stroke && node.debug == Some(PathDebugMode::Edges);
        let cached = mesh_cache.get(&key, current_frame);
        if cached.is_none() || show_contour_edges {
            contours.project(snap_mode, scale_factor)?;
        }
        let mesh = if let Some(mesh) = cached {
            mesh.clone()
        } else {
            let mesh = contours.tessellate_projected(node.fill_rule)?;
            mesh_cache.insert(key, mesh.clone(), current_frame);
            mesh
        };
        Ok((mesh, show_contour_edges.then_some(contours)))
    }

    fn flatten_node(
        &mut self,
        node: &SceneNode,
        mesh_cache: &mut MeshCache,
        scale_factor: f32,
        current_frame: u64,
        target: &mut Option<(SurfaceId, &mut ImageOccurrenceRecorder<'_>)>,
    ) {
        let first_primitive = self.primitives.len();
        let first_path = self.path_draws.len();
        let is_container = matches!(
            node,
            SceneNode::RetainedFragment(_)
                | SceneNode::Clip(_)
                | SceneNode::Scroll(_)
                | SceneNode::Layer(_)
        );
        match node {
            SceneNode::RetainedFragment(fragment) => {
                let enclosing = self.control_subtree_order.clone();
                if let Some(member) = fragment.control_subtree_member {
                    let mut members = enclosing.as_deref().unwrap_or(&[]).to_vec();
                    members.push(member);
                    self.control_subtree_order = Some(Arc::from(members));
                }
                for child in fragment.children.iter() {
                    self.flatten_node(child, mesh_cache, scale_factor, current_frame, target);
                }
                self.control_subtree_order = enclosing;
            }
            SceneNode::Rect(rect_node) => {
                self.add_rect(rect_node.rect, rect_node.color, rect_node.z_index);
            }
            SceneNode::BorderRect(border_node) => {
                // INV-BORDER-GEOMETRY-01: Border visuals conform to shape geometry
                if border_node.radius > 0.0 {
                    // Rounded border: route through SDF rect pipeline for correct geometry
                    self.add_rounded_border(
                        border_node.rect,
                        border_node.fill_color,
                        border_node.border_color,
                        border_node.border_width,
                        border_node.radius,
                        border_node.z_index,
                        mesh_cache,
                        scale_factor,
                        current_frame,
                    );
                } else {
                    // Rectangular border: use rect pipeline
                    self.add_border_rect(
                        border_node.rect,
                        border_node.fill_color,
                        border_node.border_color,
                        border_node.border_width,
                        0.0,
                        border_node.z_index,
                    );
                }
            }
            SceneNode::RoundedRect(rounded_node) => {
                // SPEC-R-022: Routes through SDF rect pipeline with shader-based corner AA
                self.add_rounded_rect(
                    rounded_node.rect,
                    rounded_node.color,
                    rounded_node.radius,
                    rounded_node.z_index,
                    mesh_cache,
                    scale_factor,
                    current_frame,
                );
            }
            SceneNode::Text(text_node) => {
                // SPEC-EXIT-A1: Pass backgrounds for selection/caret rendering
                self.add_bound_text(
                    text_node.position,
                    text_node.text.clone(),
                    text_node.font_size,
                    text_node.color,
                    text_node.backgrounds.clone(),
                    text_node.z_index,
                    text_node.binding.clone(),
                );
            }
            SceneNode::Image(image_node) => match image_node.texture_source {
                ImageTextureSource::Registered(texture_id) => self.add_image(
                    image_node.rect,
                    texture_id,
                    image_node.uv_rect,
                    image_node.tint,
                    image_node.z_index,
                ),
                ImageTextureSource::Admitted(identity) => self.add_admitted_image(
                    image_node.rect,
                    identity,
                    image_node.uv_rect,
                    image_node.tint,
                    image_node.z_index,
                ),
                ImageTextureSource::TargetBound(id) => {
                    let Some((surface, recorder)) = target.as_mut() else {
                        return;
                    };
                    let Some(resolved) = recorder.resolve_target_bound(id, *surface) else {
                        return;
                    };
                    let binding = resolved.binding();
                    self.add_image(
                        image_node.rect,
                        binding.texture_id(),
                        image_node.uv_rect,
                        image_node.tint,
                        image_node.z_index,
                    );
                    if let Some(primitive) = self.primitives.get_mut(first_primitive) {
                        if binding.canonical_over_neutral() {
                            primitive.key.pipeline = PipelineId::CanonicalOverNeutral;
                        }
                        primitive.key.control_subtree_order = Self::subtree_order_at_z(
                            primitive.key.z_layer,
                            &self.control_subtree_order,
                        );
                        resolved.record_inserted(scale_factor, primitive);
                    }
                    // The receipt uses the same final order as the other leaf primitives.
                    return;
                }
            },
            SceneNode::Clip(clip_node) => {
                // SPEC-R-020 Stage 3: Pass rounded corners to push_clip
                self.push_clip(clip_node.rect, clip_node.rounded);
                for child in &clip_node.children {
                    self.flatten_node(child, mesh_cache, scale_factor, current_frame, target);
                }
                self.pop_clip();
            }
            SceneNode::Layer(layer_node) => {
                // Layers are processed like normal groups for now
                // Future: offscreen rendering support
                for child in &layer_node.children {
                    self.flatten_node(child, mesh_cache, scale_factor, current_frame, target);
                }
            }
            // SPEC-HAL-086 Stage 7: Scroll transform node
            SceneNode::Scroll(scroll_node) => {
                // Push scroll offset, flatten children, pop offset
                self.push_scroll(scroll_node.offset);
                for child in &scroll_node.children {
                    self.flatten_node(child, mesh_cache, scale_factor, current_frame, target);
                }
                self.pop_scroll();
            }
            // SPEC-R-003: Shape primitives route through the path fill/stroke pipeline.
            SceneNode::Circle(circle_node) => {
                self.add_circle(
                    circle_node.center,
                    circle_node.radius,
                    circle_node.color,
                    circle_node.z_index,
                );
            }
            SceneNode::Ellipse(ellipse_node) => {
                self.add_ellipse(
                    ellipse_node.center,
                    ellipse_node.radii,
                    ellipse_node.color,
                    ellipse_node.z_index,
                );
            }
            SceneNode::Line(line_node) => {
                self.add_line(
                    line_node.a,
                    line_node.b,
                    line_node.width,
                    line_node.color,
                    line_node.z_index,
                );
            }
            SceneNode::Polyline(polyline_node) => {
                self.add_polyline(
                    polyline_node.points.clone(),
                    polyline_node.width,
                    polyline_node.color,
                    polyline_node.z_index,
                );
            }
            SceneNode::LuminousContour(node) => self.add_luminous_contour(node),
            SceneNode::LocalAmbientGlow(node) => self.add_local_ambient_glow(node),
            SceneNode::AnalyticSurface(node) => self.add_analytic_surface(node, scale_factor),
            SceneNode::FrostedGlass(node) => self.add_frosted_glass(node),
            // SPEC-R-004 Stage 12/15/16/17/18 + SPEC-HAL-264: Path primitive processing
            SceneNode::Path(path_node) => {
                // SPEC-R-004 Stage 15: Convert Color to [f32; 4]
                let color = [
                    path_node.color.r,
                    path_node.color.g,
                    path_node.color.b,
                    path_node.color.a,
                ];
                let (mesh, debug_contours) = match Self::resolve_path_mesh(
                    path_node,
                    mesh_cache,
                    scale_factor,
                    current_frame,
                ) {
                    Ok(geometry) => geometry,
                    Err(error) => {
                        tracing::error!(%error, rule = ?path_node.fill_rule, scale_factor, "path fill refused");
                        return;
                    }
                };

                let has_mesh = !mesh.vertices.is_empty() && !mesh.indices.is_empty();
                if has_mesh {
                    // SPEC-R-004 Stage 16: Choose fill vs stroke based on is_stroke flag
                    let scene_key = self.current_scene_key;
                    let path_draw = if path_node.is_stroke {
                        PathDraw::stroke(mesh.clone(), color, scene_key)
                    } else {
                        PathDraw::fill(mesh.clone(), color, scene_key)
                    };
                    // SPEC-R-004 Stage 18: Pass z_index for deterministic ordering
                    self.add_path(path_draw, path_node.z_index);
                }
                if let Some(debug_mode) = path_node.debug
                    && (has_mesh || debug_contours.is_some())
                {
                    self.emit_path_debug_overlays(&mesh, debug_mode, debug_contours.as_ref());
                }
            }
            // SPEC-HAL-086 Stage 8: Scrollbar overlay nodes are purely for hit-testing
            // They have no visual representation - the actual thumb is drawn as a RoundedRect
            SceneNode::ScrollbarV(_) | SceneNode::ScrollbarH(_) => {
                // No rendering - these are hit-test markers only
            }
            // SPEC-R-013-v2: Shadow pipeline
            SceneNode::Shadow(shadow_node) => {
                self.add_shadow(
                    shadow_node.shape_rect,
                    shadow_node.shape_radius,
                    shadow_node.mode,
                    shadow_node.offset,
                    shadow_node.spread,
                    shadow_node.blur_radius,
                    shadow_node.color,
                    shadow_node.z_index,
                );
            }
            // SPEC-R-012-v2: Gradient pipeline
            SceneNode::GradientRect(node) => {
                self.add_gradient_rect(node.rect, node.radius, &node.gradient, node.z_index);
            }
            // SPEC-HAL-272: Color picker SV area
            SceneNode::ColorPickerSv(node) => {
                self.add_color_picker_sv(node.rect, node.hue, node.radius, node.z_index, node.key);
            }
            // SPEC-HAL-278: Radial gauge
            SceneNode::RadialGauge(node) => {
                self.add_radial_gauge(
                    node.rect,
                    node.fill,
                    node.thickness,
                    node.track_color,
                    node.fill_color,
                    node.fill_color_negative,
                    node.start_angle,
                    node.sweep_angle,
                    node.origin_fill,
                    node.mode,
                    node.fill_style,
                    node.zone_count,
                    node.zones,
                    node.major_divisions,
                    node.minor_per_major,
                    node.tick_length_major,
                    node.tick_length_minor,
                    node.tick_color,
                    node.z_index,
                    node.key,
                );
            }
        }
        if !is_container {
            for primitive in &mut self.primitives[first_primitive..] {
                primitive.key.control_subtree_order =
                    Self::subtree_order_at_z(primitive.key.z_layer, &self.control_subtree_order);
            }
            for path in &mut self.path_draws[first_path..] {
                path.control_subtree_order =
                    Self::subtree_order_at_z(path.z_index, &self.control_subtree_order);
            }
        }
    }

    fn subtree_order_at_z(
        z_index: u32,
        order: &Option<Arc<[ControlSubtreeMember]>>,
    ) -> Option<Arc<[ControlSubtreeMember]>> {
        (z_index < crate::core::paint::LAYER_OVERLAY)
            .then(|| order.clone())
            .flatten()
    }

    /// SPEC-R-004 Stage 17: Emit debug overlay primitives for path visualization.
    ///
    /// This emits additional primitives (rects for vertices, lines for edges/triangles)
    /// purely through existing CPU-side batching. No GPU code touched.
    fn emit_path_debug_overlays(
        &mut self,
        mesh: &crate::renderer::path::PathMesh,
        mode: PathDebugMode,
        contours: Option<&FillContours>,
    ) {
        use crate::core::math::Point;

        // Debug overlay color: bright magenta for visibility
        let debug_color = Color::from_srgb(1.0, 0.0, 1.0, 1.0);
        // Use a high z-index so overlays appear on top
        let debug_z: ZIndex = u32::MAX - 1;

        match mode {
            PathDebugMode::None => {
                // No debug visualization
            }
            PathDebugMode::Vertices => {
                // Emit small rects for each vertex
                let marker_size = 2.0; // 2x2 logical pixels
                let half = marker_size * 0.5;
                for v in &mesh.vertices {
                    let rect = RectF::new(v[0] - half, v[1] - half, marker_size, marker_size);
                    self.add_rect(rect, debug_color, debug_z);
                }
            }
            PathDebugMode::Edges => {
                if let Some(contours) = contours {
                    for ([ax, ay], [bx, by]) in contours.edges() {
                        self.add_line(
                            Point::new(ax, ay),
                            Point::new(bx, by),
                            1.0,
                            debug_color,
                            debug_z,
                        );
                    }
                } else {
                    // Supplied meshes retain their vertex-sequence diagnostic.
                    for edge in mesh.vertices.windows(2) {
                        let a = Point::new(edge[0][0], edge[0][1]);
                        let b = Point::new(edge[1][0], edge[1][1]);
                        self.add_line(a, b, 1.0, debug_color, debug_z);
                    }
                }
            }
            PathDebugMode::Triangles => {
                // Emit three line segments for each triangle in the mesh
                let indices = &mesh.indices;
                let verts = &mesh.vertices;
                // Triangles are groups of 3 indices
                for tri in indices.chunks(3) {
                    if tri.len() == 3 {
                        let i0 = tri[0] as usize;
                        let i1 = tri[1] as usize;
                        let i2 = tri[2] as usize;

                        if i0 < verts.len() && i1 < verts.len() && i2 < verts.len() {
                            let p0 = Point::new(verts[i0][0], verts[i0][1]);
                            let p1 = Point::new(verts[i1][0], verts[i1][1]);
                            let p2 = Point::new(verts[i2][0], verts[i2][1]);

                            // Three edges of the triangle
                            self.add_line(p0, p1, 1.0, debug_color, debug_z);
                            self.add_line(p1, p2, 1.0, debug_color, debug_z);
                            self.add_line(p2, p0, 1.0, debug_color, debug_z);
                        }
                    }
                }
            }
        }
    }

    /// Build batches from accumulated primitives
    ///
    /// Algorithm (SPEC-R-002 updated):
    /// 1. Sort primitives by (z_layer ASC, order_key ASC) for deterministic ordering
    /// 2. Iterate sorted primitives
    /// 3. Start a new batch when key changes
    /// 4. Merge consecutive compatible primitives
    ///
    /// # SPEC-HAL-081A Visibility
    ///
    /// This is `pub(super)` to ensure only `DrawPlanBuilder` can drive batch building.
    /// External code must go through the compositor → DrawPlanBuilder path.
    pub(super) fn build_batches(&mut self) -> FrameBatches {
        // SPEC-R-004 Stage 12: Even with no batched primitives, we may have path draws
        if self.primitives.is_empty() && self.path_draws.is_empty() {
            return FrameBatches::new();
        }

        // SPEC-R-002 Stage 3: Sort primitives by (DrawBatchKey ASC, order_key ASC)
        // This ensures deterministic draw order regardless of scene graph traversal order.
        // Full key comparison ensures batches appear in DrawBatchKey order (z_layer first).
        // order_key is the tiebreaker within identical keys (preserves insertion order).
        self.primitives.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.order_key.cmp(&b.order_key))
        });

        let mut frame = FrameBatches::new();
        let mut current_batch: Option<BatchBuilder> = None;
        // SPEC-HAL-263: Per-pipeline offset tracking for separate vertex buffers
        let mut rect_vertex_offset: u32 = 0;
        let mut rect_index_offset: u32 = 0;
        let mut image_vertex_offset: u32 = 0;
        let mut image_index_offset: u32 = 0;
        let mut shadow_vertex_offset: u32 = 0;
        let mut shadow_index_offset: u32 = 0;
        let mut gradient_vertex_offset: u32 = 0;
        let mut gradient_index_offset: u32 = 0;
        let mut analytic_vertex_offset: u32 = 0;
        let mut analytic_index_offset: u32 = 0;
        let mut color_picker_sv_vertex_offset: u32 = 0;
        let mut color_picker_sv_index_offset: u32 = 0;
        let mut radial_gauge_vertex_offset: u32 = 0;
        let mut radial_gauge_index_offset: u32 = 0;
        let mut sdf_rect_vertex_offset: u32 = 0;
        let mut sdf_rect_index_offset: u32 = 0;
        let mut shape_vertex_offset: u32 = 0;
        let mut shape_index_offset: u32 = 0;

        for prim in &self.primitives {
            // Check if we can extend the current batch
            let can_extend = current_batch.as_ref().map_or(false, |b| b.key == prim.key);

            if !can_extend {
                // Finalize current batch if any
                if let Some(builder) = current_batch.take() {
                    if let Some(batch) = builder.finalize(DrawBatchId(self.next_batch_id)) {
                        self.next_batch_id += 1;
                        frame.batches.push(batch);
                    }
                }

                // Start new batch with pipeline-appropriate offsets
                // SPEC-R-001 Stage 2: Use ClipState from primitive
                // SPEC-HAL-081A: Pass validated scene_key from compositor
                let (v_start, i_start) = match prim.key.pipeline {
                    PipelineId::Image | PipelineId::CanonicalOverNeutral => {
                        (image_vertex_offset, image_index_offset)
                    }
                    PipelineId::Shadow => (shadow_vertex_offset, shadow_index_offset),
                    PipelineId::Gradient => (gradient_vertex_offset, gradient_index_offset),
                    PipelineId::AnalyticSurface => (analytic_vertex_offset, analytic_index_offset),
                    PipelineId::ColorPickerSv => {
                        (color_picker_sv_vertex_offset, color_picker_sv_index_offset)
                    }
                    PipelineId::RadialGauge => {
                        (radial_gauge_vertex_offset, radial_gauge_index_offset)
                    }
                    PipelineId::SdfRect => (sdf_rect_vertex_offset, sdf_rect_index_offset),
                    PipelineId::Shape => (shape_vertex_offset, shape_index_offset),
                    _ => (rect_vertex_offset, rect_index_offset),
                };
                current_batch = Some(BatchBuilder::new(
                    prim.key.clone(),
                    prim.clip_state,
                    v_start,
                    i_start,
                    self.current_scene_key,
                ));
            }

            // Add primitive to current batch, advancing pipeline-appropriate offset
            if let Some(ref mut builder) = current_batch {
                let (v_count, i_count) = primitive_vertex_index_count(&prim.data);
                builder.add_primitive(v_count, i_count);
                match prim.key.pipeline {
                    PipelineId::Image | PipelineId::CanonicalOverNeutral => {
                        image_vertex_offset += v_count;
                        image_index_offset += i_count;
                    }
                    PipelineId::Shadow => {
                        shadow_vertex_offset += v_count;
                        shadow_index_offset += i_count;
                    }
                    PipelineId::Gradient => {
                        gradient_vertex_offset += v_count;
                        gradient_index_offset += i_count;
                    }
                    PipelineId::AnalyticSurface => {
                        analytic_vertex_offset += v_count;
                        analytic_index_offset += i_count;
                    }
                    PipelineId::ColorPickerSv => {
                        color_picker_sv_vertex_offset += v_count;
                        color_picker_sv_index_offset += i_count;
                    }
                    PipelineId::RadialGauge => {
                        radial_gauge_vertex_offset += v_count;
                        radial_gauge_index_offset += i_count;
                    }
                    PipelineId::SdfRect => {
                        sdf_rect_vertex_offset += v_count;
                        sdf_rect_index_offset += i_count;
                    }
                    PipelineId::Shape => {
                        shape_vertex_offset += v_count;
                        shape_index_offset += i_count;
                    }
                    _ => {
                        rect_vertex_offset += v_count;
                        rect_index_offset += i_count;
                    }
                }
            }
        }

        // Finalize last batch
        if let Some(builder) = current_batch {
            if let Some(batch) = builder.finalize(DrawBatchId(self.next_batch_id)) {
                frame.batches.push(batch);
            }
        }

        frame.total_vertices = rect_vertex_offset;
        frame.total_indices = rect_index_offset;
        frame.total_image_vertices = image_vertex_offset;
        frame.total_image_indices = image_index_offset;

        // SPEC-R-004 Stage 18: Sort path draws by canonical sort key for deterministic ordering
        // Complete effective order, then the existing same-member path tie fields.
        // This ensures identical output regardless of scene graph traversal or hash-map iteration order.
        //
        // INV-PATH-ORDER-STABILITY: sort_bias ensures presence/absence of optional primitives
        // (caret, selection) does not change relative draw order of other PATH primitives
        // within the same scene_key and z_index. Typical bias values:
        // - background = 0, border = 100, scrollbar = 200, caret = 300
        self.path_draws
            .sort_by(|a, b| PathSortKey::from_path_draw(a).cmp(&PathSortKey::from_path_draw(b)));

        // INV-PATH-ORDER-STABILITY: Debug assertion to verify sort_bias ordering is respected.
        // Within the same z_index, paths should be ordered by sort_bias, ensuring stable
        // relative ordering regardless of which optional primitives are present.
        #[cfg(debug_assertions)]
        {
            for window in self.path_draws.windows(2) {
                let a = &window[0];
                let b = &window[1];
                // If same z_index, sort_bias must be <= (non-decreasing)
                if a.effective_order() == b.effective_order() {
                    debug_assert!(
                        a.sort_bias <= b.sort_bias
                            || (a.sort_bias == b.sort_bias && a.clip_id <= b.clip_id),
                        "INV-PATH-ORDER-STABILITY: paths with same z_index must be sorted by sort_bias. \
                         z={} but sort_bias {} > {}",
                        a.z_index,
                        a.sort_bias,
                        b.sort_bias
                    );
                }
            }
        }

        // Compute overlay_start_batch_idx: first batch with z_layer >= LAYER_OVERLAY (1000).
        // Effective order keeps the original overlay class after all main intervals.
        frame.overlay_start_batch_idx = frame
            .batches
            .iter()
            .position(|b| b.key.z_layer >= crate::core::paint::LAYER_OVERLAY)
            .unwrap_or(frame.batches.len());

        // SPEC-R-004 Stage 12: Transfer sorted path draws to frame
        frame.paths = std::mem::take(&mut self.path_draws);

        // SPEC-R-004 Stage 13: Transfer path clip rects to frame
        frame.path_clips = std::mem::take(&mut self.path_clip_rects);

        frame
    }

    /// Get collected primitives (for vertex generation)
    pub fn primitives(&self) -> &[FlatPrimitive] {
        &self.primitives
    }

    /// SPEC-R-004 Stage 12: Get collected path draws.
    pub fn path_draws(&self) -> &[PathDraw] {
        &self.path_draws
    }

    /// SPEC-R-004 Stage 13: Get collected path clip rects.
    pub fn path_clip_rects(&self) -> &[RectF] {
        &self.path_clip_rects
    }
}

impl Default for Batcher {
    fn default() -> Self {
        Self::new()
    }
}

fn ellipse_path(center: Point, rx: f32, ry: f32) -> Path {
    const K: f32 = 0.5522847498;

    let kx = rx * K;
    let ky = ry * K;
    let cx = center.x;
    let cy = center.y;

    let mut path = Path::new();
    path.move_to(cx + rx, cy);
    path.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
    path.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
    path.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
    path.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
    path.close();
    path
}

/// Helper for building a single batch
///
/// SPEC-R-001 Stage 2: Uses ClipState instead of Option<RectF>.
/// SPEC-HAL-081A: Carries scene_key for compositor surface routing.
struct BatchBuilder {
    key: DrawBatchKey,
    clip_state: ClipState,
    vertex_start: u32,
    index_start: u32,
    vertex_count: u32,
    index_count: u32,
    /// SPEC-HAL-081A: Compositor surface scene_key for this batch.
    /// Typed as SceneKey to guarantee non-zero.
    scene_key: SceneKey,
}

impl BatchBuilder {
    fn new(
        key: DrawBatchKey,
        clip_state: ClipState,
        vertex_start: u32,
        index_start: u32,
        scene_key: SceneKey,
    ) -> Self {
        Self {
            key,
            clip_state,
            vertex_start,
            index_start,
            vertex_count: 0,
            index_count: 0,
            scene_key,
        }
    }

    fn add_primitive(&mut self, vertex_count: u32, index_count: u32) {
        self.vertex_count += vertex_count;
        self.index_count += index_count;
    }

    fn finalize(self, id: DrawBatchId) -> Option<DrawBatch> {
        if self.vertex_count == 0 && self.key.pipeline != PipelineId::FrostedGlass {
            return None;
        }

        // SPEC-HAL-081A: scene_key comes from compositor, passed through BatchBuilder
        Some(DrawBatch::new(
            id,
            self.key,
            BufferRange::new(self.vertex_start, self.vertex_count),
            BufferRange::new(self.index_start, self.index_count),
            self.clip_state,
            self.scene_key,
        ))
    }
}

/// Calculate vertex and index count for a primitive
pub(crate) fn primitive_vertex_index_count(data: &PrimitiveData) -> (u32, u32) {
    match data {
        PrimitiveData::FrostedGlass {..} => (0,0),
        PrimitiveData::LocalAmbientGlow { .. } => (4, 6),
        PrimitiveData::AnalyticSurface { .. } => (4, 6),
        PrimitiveData::Rect { .. } => (4, 6), // Quad: 4 vertices, 2 triangles
        PrimitiveData::BorderRect { border_width, .. } => {
            // BorderRect renders as: 1 fill rect + 4 border rects (if border_width > 0)
            // Each rect = 4 vertices, 6 indices
            if *border_width > 0.0 {
                (20, 30) // 5 rects: fill + top + bottom + left + right
            } else {
                (4, 6) // Just the fill rect
            }
        }
        PrimitiveData::RoundedRect { .. } => (4, 6), // SPEC-HAL-085: Rect pipeline with shader AA
        PrimitiveData::Text { .. } => (0, 0),        // Text handled separately
        PrimitiveData::Image { .. } => (4, 6),       // Same as rect
        PrimitiveData::Circle { .. } => (4, 6),      // SPEC-R-003: one SDF quad
        PrimitiveData::Ellipse { .. } => (4, 6),     // SPEC-R-003: one SDF quad
        PrimitiveData::Line { .. } => (4, 6),        // SPEC-R-003: one segment quad
        PrimitiveData::Polyline { points, .. } => {
            let segment_count = points.len().saturating_sub(1) as u32;
            (segment_count * 4, segment_count * 6)
        }
        PrimitiveData::LuminousContour {
            points,
            fronts,
            total_length,
            ..
        } => {
            let segment_count =
                visit_luminous_contour_regions(points, *total_length, fronts, |_| {});
            (segment_count * 4, segment_count * 6)
        }
        PrimitiveData::Shadow { .. } => (4, 6), // One quad per shadow
        PrimitiveData::GradientRect { .. } => (4, 6), // One quad per gradient rect
        PrimitiveData::ColorPickerSv { .. } => (4, 6), // SPEC-HAL-272: One quad per SV square
        PrimitiveData::RadialGauge { .. } => (4, 6), // SPEC-HAL-278: One quad per gauge
        PrimitiveData::SdfRect { .. } => (4, 6), // SPEC-R-022: One quad per SDF rect
    }
}

#[cfg(test)]
pub(crate) fn test_flatten_counts(nodes: &[SceneNode]) -> (usize, usize) {
    let mut batcher = Batcher::new();
    batcher.begin_frame(SceneKey::BASE);
    batcher.flatten_scene(nodes, &mut MeshCache::new(), 1.0, 0);
    (batcher.primitives.len(), batcher.path_draws.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::path::FillRule;

    fn debug_path_draw_count(batcher: &Batcher) -> usize {
        batcher
            .path_draws()
            .iter()
            .filter(|path| path.color == [1.0, 0.0, 1.0, 1.0])
            .count()
    }

    fn first_sdf_rect_metrics(batcher: &Batcher) -> (RectF, (f32, f32), (f32, f32), f32) {
        batcher
            .primitives()
            .iter()
            .find_map(|prim| {
                if let PrimitiveData::SdfRect {
                    quad_rect,
                    rect_center,
                    rect_half_size,
                    radius,
                    ..
                } = &prim.data
                {
                    Some((*quad_rect, *rect_center, *rect_half_size, *radius))
                } else {
                    None
                }
            })
            .expect("expected SdfRect primitive")
    }

    #[test]
    fn test_batch_key_equality() {
        let key1 = DrawBatchKey::rect(123, 0);
        let key2 = DrawBatchKey::rect(123, 0);
        let key3 = DrawBatchKey::rect(456, 0);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_buffer_range() {
        let range = BufferRange::new(10, 20);
        assert_eq!(range.start, 10);
        assert_eq!(range.count, 20);
        assert_eq!(range.end(), 30);
    }

    #[test]
    fn test_r001_native_nested_clip_same_rect() {
        use crate::core::scene::RoundedCorners;

        let rect = RectF::new(
            f32::from_bits(0x4515f513),
            f32::from_bits(0x4532533c),
            f32::from_bits(0x42893387),
            f32::from_bits(0x42d6f831),
        );
        let rounded = RoundedCorners {
            tl: 4.0,
            tr: 8.0,
            bl: 2.0,
            br: 0.0,
        };
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.push_clip(rect, Some(rounded));
        let parent = batcher.clip_state();
        assert_eq!(parent.rect, rect);
        assert_eq!(parent.rounded, Some(rounded));
        assert_ne!(parent.hash, 0);
        // This actual second push used to hit the unchanged area assertion.
        batcher.push_clip(rect, None);
        let repeated = batcher.clip_state();
        assert_eq!(repeated.rect.width.to_bits(), rect.width.to_bits());
        assert_eq!(repeated.rect.height.to_bits(), rect.height.to_bits());
        assert_eq!(repeated.rect, rect);
        assert_eq!(repeated.rounded, None);
        assert_ne!(repeated.hash, parent.hash); // actual stack depth participates
        assert_eq!(batcher.clip_hash(), repeated.hash);
        batcher.add_rect(rect, Color::WHITE, 0);

        batcher.push_scroll((8.0, 12.0));
        let content = RectF::new(rect.x + 32.0, rect.y + 44.0, rect.width, rect.height);
        batcher.push_clip(content, Some(rounded));
        let child = batcher.clip_state();
        let expected = RectF::new(
            rect.x + 24.0,
            rect.y + 32.0,
            rect.width - 24.0,
            rect.height - 32.0,
        );
        assert_eq!(child.rect, expected);
        assert_eq!(child.rounded, Some(rounded));
        assert!(child.rect.width <= rect.width && child.rect.height <= rect.height);
        assert!(child.rect.width * child.rect.height <= rect.width * rect.height);
        assert!(
            f64::from(child.rect.x) + f64::from(child.rect.width)
                <= f64::from(rect.x) + f64::from(rect.width)
        );
        assert!(
            f64::from(child.rect.y) + f64::from(child.rect.height)
                <= f64::from(rect.y) + f64::from(rect.height)
        );
        assert!(f64::from(child.rect.x) >= f64::from(content.x) - 8.0);
        assert!(f64::from(child.rect.y) >= f64::from(content.y) - 12.0);
        batcher.add_rect(content, Color::WHITE, 1);
        batcher.pop_clip();
        assert_eq!(batcher.clip_state(), repeated);
        batcher.pop_scroll();
        assert_eq!(batcher.current_scroll(), (0.0, 0.0));
        batcher.pop_clip();
        assert_eq!(batcher.clip_state(), parent);

        batcher.push_clip(RectF::new(0.0, 0.0, 1.0, 1.0), None);
        assert_eq!(batcher.current_clip(), Some(RectF::zero()));
        assert!(batcher.clip_state().is_empty());
        assert_ne!(batcher.clip_hash(), 0);
        batcher.pop_clip();
        assert_eq!(batcher.clip_state(), parent);
        batcher.pop_clip();
        assert_eq!(batcher.clip_state(), ClipState::none());

        batcher.push_clip(RectF::new(0.0, 0.0, 100.0, 100.0), None);
        batcher.push_clip(RectF::new(50.0, 50.0, 100.0, 100.0), None);
        assert_eq!(
            batcher.current_clip(),
            Some(RectF::new(50.0, 50.0, 50.0, 50.0))
        );
        batcher.add_rect(RectF::new(50.0, 50.0, 1.0, 1.0), Color::WHITE, 2);
        batcher.pop_clip();
        batcher.pop_clip();
        assert_eq!(batcher.clip_state(), ClipState::none());
        assert!(batcher.clip_stack.is_empty());
        let frame = batcher.build_batches();
        for batch in &frame.batches {
            assert_eq!(batch.clip_state.hash, batch.key.clip_hash);
        }
    }

    #[test]
    fn test_batcher_clip_stack() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        assert!(batcher.current_clip().is_none());

        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        batcher.push_clip(clip1, None);
        assert_eq!(batcher.current_clip(), Some(clip1));

        let clip2 = RectF::new(25.0, 25.0, 50.0, 50.0);
        batcher.push_clip(clip2, None);
        // Should be intersection
        let expected = clip1.intersection(&clip2).unwrap();
        assert_eq!(batcher.current_clip(), Some(expected));

        batcher.pop_clip();
        assert_eq!(batcher.current_clip(), Some(clip1));

        batcher.pop_clip();
        assert!(batcher.current_clip().is_none());
    }

    #[test]
    fn test_batcher_batching() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add multiple rects with same z-index and no clipping
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let batches = batcher.build_batches();

        // All 3 rects should be in one batch
        assert_eq!(batches.batch_count(), 1);
        assert_eq!(batches.total_vertices, 12); // 3 rects * 4 vertices
        assert_eq!(batches.total_indices, 18); // 3 rects * 6 indices
    }

    #[test]
    fn scene_builder_preserves_shape_primitive_inputs() {
        use crate::core::scene::SceneBuilder;

        let mut builder = SceneBuilder::new();
        let red = Color::from_srgb(1.0, 0.3, 0.3, 1.0);
        let green = Color::from_srgb(0.3, 1.0, 0.3, 1.0);
        let blue = Color::from_srgb(0.3, 0.3, 1.0, 1.0);
        let circle_center = Point::new(100.0, 100.0);
        let ellipse_center = Point::new(350.0, 100.0);
        let ellipse_radii = Point::new(60.0, 30.0);
        let start = Point::new(50.0, 200.0);
        let end = Point::new(250.0, 250.0);
        let points = vec![
            Point::new(50.0, 350.0),
            Point::new(100.0, 300.0),
            Point::new(150.0, 380.0),
            Point::new(200.0, 320.0),
            Point::new(250.0, 400.0),
        ];
        builder.push_circle(circle_center, 50.0, red, 1, None);
        builder.push_ellipse(ellipse_center, ellipse_radii, blue, 1, None);
        builder.push_line(start, end, 3.0, green, 1, None);
        builder.push_polyline(points.clone(), 2.0, blue, 1, None);
        let scene = builder.build();
        let [SceneNode::Circle(circle), SceneNode::Ellipse(ellipse), SceneNode::Line(line), SceneNode::Polyline(polyline)] =
            scene.root.as_slice()
        else {
            panic!("expected one scene node for each shape, in authoring order");
        };
        assert_eq!(
            (circle.center, circle.radius, circle.color, circle.z_index, circle.key),
            (circle_center, 50.0, red, 1, None)
        );
        assert_eq!(
            (ellipse.center, ellipse.radii, ellipse.color, ellipse.z_index, ellipse.key),
            (ellipse_center, ellipse_radii, blue, 1, None)
        );
        assert_eq!(
            (line.a, line.b, line.width, line.color, line.z_index, line.key),
            (start, end, 3.0, green, 1, None)
        );
        assert_eq!(
            (&polyline.points, polyline.width, polyline.color, polyline.z_index, polyline.key),
            (&points, 2.0, blue, 1, None)
        );
    }

    #[test]
    fn test_circle_ellipse_path_circle_routes_to_fill_pipeline() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        batcher.add_circle(Point::new(50.0, 60.0), 10.0, Color::WHITE, 7);

        assert!(batcher.primitives().is_empty());
        assert_eq!(batcher.path_draws().len(), 1);

        let path_draw = &batcher.path_draws()[0];
        assert_eq!(path_draw.stroke_flag, 0);
        assert_eq!(path_draw.z_index, 7);
        assert_eq!(path_draw.color, [1.0, 1.0, 1.0, 1.0]);
        assert!((path_draw.mesh.bounds.x - 40.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.y - 50.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.width - 20.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.height - 20.0).abs() < 0.5);

        let frame = batcher.build_batches();
        assert!(frame.batches.is_empty());
        assert_eq!(frame.paths.len(), 1);
        assert!(frame.has_paths());
    }

    #[test]
    fn test_circle_ellipse_path_ellipse_routes_to_fill_pipeline() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        batcher.add_ellipse(
            Point::new(50.0, 60.0),
            Point::new(20.0, 10.0),
            Color::from_linear(0.25, 0.5, 0.75, 1.0),
            9,
        );

        assert!(batcher.primitives().is_empty());
        assert_eq!(batcher.path_draws().len(), 1);

        let path_draw = &batcher.path_draws()[0];
        assert_eq!(path_draw.stroke_flag, 0);
        assert_eq!(path_draw.z_index, 9);
        assert_eq!(path_draw.color, [0.25, 0.5, 0.75, 1.0]);
        assert!((path_draw.mesh.bounds.x - 30.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.y - 50.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.width - 40.0).abs() < 0.5);
        assert!((path_draw.mesh.bounds.height - 20.0).abs() < 0.5);
    }

    #[test]
    fn test_circle_ellipse_path_degenerate_emit_no_paths() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        batcher.add_circle(Point::new(0.0, 0.0), 0.0, Color::WHITE, 0);
        batcher.add_circle(Point::new(0.0, 0.0), 10.0, Color::TRANSPARENT, 0);
        batcher.add_ellipse(Point::new(0.0, 0.0), Point::new(0.0, 10.0), Color::WHITE, 0);
        batcher.add_ellipse(Point::new(0.0, 0.0), Point::new(10.0, 0.0), Color::WHITE, 0);
        batcher.add_ellipse(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            Color::TRANSPARENT,
            0,
        );

        assert!(batcher.primitives().is_empty());
        assert!(batcher.path_draws().is_empty());
    }

    #[test]
    fn test_shape_primitive_vertex_index_counts() {
        let circle = PrimitiveData::Circle {
            center: Point::new(10.0, 20.0),
            radius: 5.0,
            color: Color::WHITE,
        };
        let ellipse = PrimitiveData::Ellipse {
            center: Point::new(10.0, 20.0),
            radii: Point::new(8.0, 4.0),
            color: Color::WHITE,
        };
        let line = PrimitiveData::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(10.0, 0.0),
            width: 2.0,
            color: Color::WHITE,
        };
        let polyline = PrimitiveData::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
            ],
            width: 2.0,
            color: Color::WHITE,
        };

        assert_eq!(primitive_vertex_index_count(&circle), (4, 6));
        assert_eq!(primitive_vertex_index_count(&ellipse), (4, 6));
        assert_eq!(primitive_vertex_index_count(&line), (4, 6));
        assert_eq!(primitive_vertex_index_count(&polyline), (8, 12));
    }

    #[test]
    fn luminous_contour_routes_only_to_shape_and_counts_single_owner_regions() {
        let points: Arc<[Point]> = Arc::from([
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
        ]);
        let appearance = LuminousContourAppearance::try_new(5.0, 0.12, 3.5).unwrap();
        let fronts = LuminousContourFronts::try_from_slice(&[
            crate::core::paint::LuminousContourFront::try_new(10.0, 3.0, 0.8).unwrap(),
        ])
        .unwrap();
        let mut builder = crate::core::scene::SceneBuilder::new();
        builder
            .push_luminous_contour(
                points.clone(),
                2.0,
                Color::WHITE,
                appearance,
                fronts,
                4,
                None,
            )
            .unwrap();
        let scene = builder.build();
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        let clip = RectF::new(1.0, 2.0, 30.0, 40.0);
        batcher.push_clip(clip, None);
        batcher.push_scroll((3.0, 5.0));
        batcher.flatten_scene(&scene.root, &mut MeshCache::new(), 2.0, 0);

        assert!(batcher.path_draws().is_empty());
        assert_eq!(batcher.primitives().len(), 1);
        let primitive = &batcher.primitives()[0];
        assert_eq!(primitive.key.pipeline, PipelineId::Shape);
        assert_eq!(
            primitive.key.shape_type_id,
            Some(ShapeType::LuminousContour as u8)
        );
        assert_eq!(primitive.key.z_layer, 4);
        assert_eq!(primitive.key.clip_hash, primitive.clip_state.hash);
        assert_eq!(primitive.clip_state.rect, clip);
        let PrimitiveData::LuminousContour {
            points: lowered_points,
            scroll_offset,
            ..
        } = &primitive.data
        else {
            panic!("expected luminous contour primitive");
        };
        assert!(Arc::ptr_eq(&points, lowered_points));
        assert_eq!(*scroll_offset, (3.0, 5.0));
        assert_eq!(primitive_vertex_index_count(&primitive.data), (40, 60));
    }

    #[test]
    fn luminous_contour_front_opacity_preserves_legacy_alpha_eligibility() {
        for alpha in [0.0, 0.0005, 0.001, 1.0] {
            for boost in [0.0, 0.8] {
                let appearance = LuminousContourAppearance::try_new(3.0, 0.1, 2.0)
                    .unwrap()
                    .with_front_opacity_boost(boost)
                    .unwrap();
                let mut builder = crate::core::scene::SceneBuilder::new();
                builder
                    .push_luminous_contour(
                        Arc::from([Point::new(0.0, 0.0), Point::new(32.0, 0.0)]),
                        2.0,
                        Color::from_linear(0.25, 0.06, 0.015, alpha),
                        appearance,
                        LuminousContourFronts::try_from_slice(&[
                            crate::core::paint::LuminousContourFront::try_new(16.0, 4.0, 1.0).unwrap(),
                        ])
                        .unwrap(),
                        4,
                        None,
                    )
                    .unwrap();
                let scene = builder.build();
                let mut batcher = Batcher::new();
                batcher.begin_frame(SceneKey::BASE);
                batcher.flatten_scene(&scene.root, &mut MeshCache::new(), 1.0, 0);
                assert_eq!(
                    batcher.primitives().len(),
                    usize::from(alpha >= 0.001 || boost > 0.0)
                );
                assert!(batcher.path_draws().is_empty());
                if let Some(primitive) = batcher.primitives().first() {
                    let PrimitiveData::LuminousContour {
                        color, appearance, ..
                    } = &primitive.data
                    else {
                        panic!("expected one ordinary luminous-contour primitive");
                    };
                    assert_eq!(color.a, alpha);
                    assert_eq!(appearance.front_opacity_boost(), boost);
                    assert_eq!(primitive.key.pipeline, PipelineId::Shape);
                    assert_eq!(primitive.key.z_layer, 4);
                }
            }
        }
    }

    #[test]
    fn ordinary_polyline_keeps_its_path_route_when_luminous_support_exists() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.add_polyline(
            vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            2.0,
            Color::WHITE,
            1,
        );

        assert!(batcher.primitives().is_empty());
        assert_eq!(batcher.path_draws().len(), 1);
    }

    #[test]
    fn test_batcher_clip_breaks_batch() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add rect without clip
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        // Push clip and add rect
        batcher.push_clip(RectF::new(0.0, 0.0, 50.0, 50.0), None);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.pop_clip();

        // Add another rect without clip
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let batches = batcher.build_batches();

        // SPEC-R-002 Stage 3: After sorting by full DrawBatchKey, same-key primitives coalesce.
        // Before: 3 batches (unclipped, clipped, unclipped - interleaved)
        // After:  2 batches (unclipped x2 coalesced, clipped x1)
        assert_eq!(batches.batch_count(), 2);

        // Verify batch order: clip_hash=0 (unclipped) comes before clip_hash>0 (clipped)
        assert_eq!(batches.batches[0].key.clip_hash, 0);
        assert_ne!(batches.batches[1].key.clip_hash, 0);
    }

    #[test]
    fn test_batcher_z_order_breaks_batch() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 1); // Different z
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let batches = batcher.build_batches();

        // SPEC-R-002: After sorting by (z_layer, order_key), primitives with same z are coalesced.
        // Before: 3 batches (z=0, z=1, z=0 - separate due to interleaving)
        // After:  2 batches (z=0 x2 coalesced, z=1)
        assert_eq!(batches.batch_count(), 2);

        // Verify batch z-layer ordering
        assert_eq!(batches.batches[0].key.z_layer, 0);
        assert_eq!(batches.batches[1].key.z_layer, 1);
    }

    // ========================================================================
    // SPEC-R-001: ClipState Tests
    // ========================================================================

    #[test]
    fn test_clip_state_none() {
        let state = ClipState::none();
        assert!(state.is_none());
        assert_eq!(state.hash, 0);
    }

    #[test]
    fn test_clip_state_new() {
        let rect = RectF::new(10.0, 20.0, 100.0, 200.0);
        let state = ClipState::new(rect, 12345);
        assert!(!state.is_none());
        assert_eq!(state.rect, rect);
        assert_eq!(state.hash, 12345);
    }

    #[test]
    fn test_clip_state_is_empty() {
        let zero_width = ClipState::new(RectF::new(0.0, 0.0, 0.0, 100.0), 1);
        let zero_height = ClipState::new(RectF::new(0.0, 0.0, 100.0, 0.0), 1);
        let negative = ClipState::new(RectF::new(0.0, 0.0, -10.0, 100.0), 1);
        let normal = ClipState::new(RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        assert!(zero_width.is_empty());
        assert!(zero_height.is_empty());
        assert!(negative.is_empty());
        assert!(!normal.is_empty());
    }

    #[test]
    fn test_batcher_clip_state_lifecycle() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Initially no clip state
        let state0 = batcher.clip_state();
        assert!(state0.is_none());
        assert_eq!(batcher.clip_hash(), 0);

        // Push first clip
        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        batcher.push_clip(clip1, None);
        let state1 = batcher.clip_state();
        assert!(!state1.is_none());
        assert_eq!(state1.rect, clip1);
        let hash1 = state1.hash;
        assert_ne!(hash1, 0);

        // Push second clip (intersected)
        let clip2 = RectF::new(25.0, 25.0, 50.0, 50.0);
        batcher.push_clip(clip2, None);
        let state2 = batcher.clip_state();
        let expected_intersection = clip1.intersection(&clip2).unwrap();
        assert_eq!(state2.rect, expected_intersection);
        let hash2 = state2.hash;
        assert_ne!(hash2, hash1); // Different stack -> different hash

        // Pop back to first clip
        batcher.pop_clip();
        let state3 = batcher.clip_state();
        assert_eq!(state3.rect, clip1);
        assert_eq!(state3.hash, hash1); // Same stack state -> same hash

        // Pop to empty
        batcher.pop_clip();
        let state4 = batcher.clip_state();
        assert!(state4.is_none());
    }

    #[test]
    fn test_batcher_clip_hash_deterministic() {
        // Test that the same clip sequence produces the same hash
        let mut batcher1 = Batcher::new();
        let mut batcher2 = Batcher::new();

        batcher1.begin_frame(SceneKey::BASE);
        batcher2.begin_frame(SceneKey::BASE);

        let clip1 = RectF::new(10.0, 20.0, 100.0, 200.0);
        let clip2 = RectF::new(30.0, 40.0, 50.0, 60.0);

        batcher1.push_clip(clip1, None);
        batcher1.push_clip(clip2, None);

        batcher2.push_clip(clip1, None);
        batcher2.push_clip(clip2, None);

        assert_eq!(batcher1.clip_hash(), batcher2.clip_hash());
    }

    #[test]
    fn test_batcher_empty_intersection_produces_zero_rect() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Push a clip that doesn't intersect
        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        let clip2 = RectF::new(200.0, 200.0, 50.0, 50.0); // Non-overlapping

        batcher.push_clip(clip1, None);
        batcher.push_clip(clip2, None);

        let state = batcher.clip_state();
        // Empty intersection should produce zero-area rect
        assert!(state.is_empty() || state.rect.width == 0.0 || state.rect.height == 0.0);
    }

    // ========================================================================
    // SPEC-R-001 Stage 2: FlatPrimitive ClipState Tests
    // ========================================================================

    #[test]
    fn test_primitive_clip_state_invariant() {
        // Test that primitive.clip_state.hash == primitive.key.clip_hash
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add primitive without clip
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        // Push clip and add primitive
        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        batcher.push_clip(clip1, None);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        // Add another primitive with same clip
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.pop_clip();

        let primitives = batcher.primitives();

        // Verify invariant: clip_state.hash == key.clip_hash for each primitive
        for prim in primitives {
            assert_eq!(
                prim.clip_state.hash, prim.key.clip_hash,
                "SPEC-R-001 Stage 2 invariant: clip_state.hash must equal key.clip_hash"
            );
        }

        // First primitive should have no clip (hash = 0)
        assert!(primitives[0].clip_state.is_none());
        assert_eq!(primitives[0].key.clip_hash, 0);

        // Second and third primitives should have clip
        assert!(!primitives[1].clip_state.is_none());
        assert_eq!(primitives[1].clip_state.rect, clip1);
        assert_eq!(primitives[2].clip_state.rect, clip1);

        // Both clipped primitives should have same hash
        assert_eq!(primitives[1].clip_state.hash, primitives[2].clip_state.hash);
    }

    #[test]
    fn test_primitive_clip_state_captured_at_insertion() {
        // Test that clip_state is captured at primitive insertion time
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        let clip2 = RectF::new(10.0, 10.0, 50.0, 50.0);

        // Capture expected state before adding primitive
        let expected_state_none = batcher.clip_state();
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        batcher.push_clip(clip1, None);
        let expected_state_clip1 = batcher.clip_state();
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        batcher.push_clip(clip2, None);
        let expected_state_clip2 = batcher.clip_state();
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let primitives = batcher.primitives();

        // Verify each primitive captured the correct clip state
        assert_eq!(primitives[0].clip_state, expected_state_none);
        assert_eq!(primitives[1].clip_state, expected_state_clip1);
        assert_eq!(primitives[2].clip_state, expected_state_clip2);
    }

    #[test]
    fn test_two_batchers_produce_identical_clip_states() {
        // Test that identical sequences produce identical clip states
        let clip1 = RectF::new(10.0, 20.0, 100.0, 200.0);
        let clip2 = RectF::new(30.0, 40.0, 50.0, 60.0);

        let mut batcher1 = Batcher::new();
        let mut batcher2 = Batcher::new();

        batcher1.begin_frame(SceneKey::BASE);
        batcher2.begin_frame(SceneKey::BASE);

        // Identical sequence on both batchers
        batcher1.push_clip(clip1, None);
        batcher2.push_clip(clip1, None);
        batcher1.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher2.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        batcher1.push_clip(clip2, None);
        batcher2.push_clip(clip2, None);
        batcher1.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher2.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let prims1 = batcher1.primitives();
        let prims2 = batcher2.primitives();

        assert_eq!(prims1.len(), prims2.len());

        for (p1, p2) in prims1.iter().zip(prims2.iter()) {
            assert_eq!(
                p1.clip_state.hash, p2.clip_state.hash,
                "SPEC-R-001 Stage 2: Identical sequences must produce identical hashes"
            );
            assert_eq!(
                p1.clip_state.rect, p2.clip_state.rect,
                "SPEC-R-001 Stage 2: Identical sequences must produce identical rects"
            );
        }
    }

    #[test]
    fn test_batch_clip_state_from_primitive() {
        // Test that DrawBatch gets ClipState from first primitive
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        batcher.push_clip(clip1, None);
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.pop_clip();

        let frame_batches = batcher.build_batches();

        // Should have one batch for the two clipped rects
        assert_eq!(frame_batches.batch_count(), 1);

        let batch = &frame_batches.batches[0];
        // Batch clip_state should match the primitives
        assert_eq!(batch.clip_state.rect, clip1);
        assert_eq!(batch.clip_state.hash, batch.key.clip_hash);
    }

    // ========================================================================
    // SPEC-R-001 Stage 4: Final Invariant Test
    // ========================================================================

    #[test]
    fn test_SPEC_r001_batch_invariants() {
        // SPEC-R-001 Stage 4: Verify all batch invariants hold after build
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Mix of clipped and unclipped primitives
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let clip1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        batcher.push_clip(clip1, None);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let clip2 = RectF::new(25.0, 25.0, 50.0, 50.0);
        batcher.push_clip(clip2, None);
        batcher.add_rect(RectF::new(30.0, 30.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.pop_clip();
        batcher.pop_clip();

        batcher.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        let frame_batches = batcher.build_batches();

        // Verify all batches satisfy SPEC-R-001 invariants
        for batch in &frame_batches.batches {
            // Invariant 1: clip_state.hash must equal key.clip_hash
            assert_eq!(
                batch.clip_state.hash, batch.key.clip_hash,
                "SPEC-R-001: batch clip_state.hash ({}) != key.clip_hash ({})",
                batch.clip_state.hash, batch.key.clip_hash
            );

            // Invariant 2: is_none() implies hash == 0
            if batch.clip_state.is_none() {
                assert_eq!(
                    batch.clip_state.hash, 0,
                    "SPEC-R-001: ClipState.is_none() but hash != 0 ({})",
                    batch.clip_state.hash
                );
                assert_eq!(
                    batch.key.clip_hash, 0,
                    "SPEC-R-001: ClipState.is_none() but key.clip_hash != 0 ({})",
                    batch.key.clip_hash
                );
            }

            // Invariant 3: non-none implies hash != 0
            if !batch.clip_state.is_none() {
                assert_ne!(
                    batch.clip_state.hash, 0,
                    "SPEC-R-001: ClipState has clip but hash == 0"
                );
            }
        }
    }

    // ========================================================================
    // SPEC-R-002: Z-Index & Ordering Determinism Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r002_same_z_preserves_insertion_order() {
        // SPEC-R-002: Identical primitives with same Z should preserve insertion order
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add 5 rects with same Z, same clip - should be batched together
        // After sorting, order_key should determine order (which equals insertion order)
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(30.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        // Verify order_keys are monotonically increasing
        let primitives = batcher.primitives();
        assert_eq!(primitives.len(), 5);
        for i in 0..primitives.len() {
            assert_eq!(
                primitives[i].order_key, i as u32,
                "SPEC-R-002: order_key should equal insertion index"
            );
        }

        // Build batches - sorting should preserve order since all have same z_layer
        let frame_batches = batcher.build_batches();

        // Should all be in one batch
        assert_eq!(frame_batches.batch_count(), 1);
        assert_eq!(frame_batches.total_vertices, 20); // 5 rects * 4 vertices

        // Verify primitives are still in insertion order after sort
        let primitives_after = batcher.primitives();
        for i in 0..primitives_after.len() {
            assert_eq!(
                primitives_after[i].order_key, i as u32,
                "SPEC-R-002: primitives should preserve insertion order after sort (same z_layer)"
            );
        }
    }

    #[test]
    fn test_SPEC_r002_different_z_sorted_before_order_key() {
        // SPEC-R-002: Primitives with different Z must be sorted by Z first, then order_key
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add primitives in non-Z-order: z=2, z=0, z=1, z=0, z=2
        // Insertion order: 0, 1, 2, 3, 4
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 2); // order_key=0, z=2
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0); // order_key=1, z=0
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 1); // order_key=2, z=1
        batcher.add_rect(RectF::new(30.0, 0.0, 10.0, 10.0), Color::WHITE, 0); // order_key=3, z=0
        batcher.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 2); // order_key=4, z=2

        // Build batches - this sorts by (z_layer, order_key)
        let frame_batches = batcher.build_batches();

        // After sort: z=0 (order 1,3), z=1 (order 2), z=2 (order 0,4)
        // Should produce 3 batches (one per z-layer)
        assert_eq!(
            frame_batches.batch_count(),
            3,
            "SPEC-R-002: should have 3 batches (one per z-layer)"
        );

        // Verify primitives are sorted by (z_layer, order_key)
        let primitives = batcher.primitives();
        assert_eq!(primitives.len(), 5);

        // Expected order after sort: z=0/ok=1, z=0/ok=3, z=1/ok=2, z=2/ok=0, z=2/ok=4
        let expected_order = [(0, 1), (0, 3), (1, 2), (2, 0), (2, 4)]; // (z_layer, order_key)

        for (i, (expected_z, expected_ok)) in expected_order.iter().enumerate() {
            assert_eq!(
                primitives[i].key.z_layer, *expected_z,
                "SPEC-R-002: primitive {} should have z_layer={}, got {}",
                i, expected_z, primitives[i].key.z_layer
            );
            assert_eq!(
                primitives[i].order_key, *expected_ok,
                "SPEC-R-002: primitive {} should have order_key={}, got {}",
                i, expected_ok, primitives[i].order_key
            );
        }

        // Verify batches are in z-order
        assert_eq!(frame_batches.batches[0].key.z_layer, 0);
        assert_eq!(frame_batches.batches[1].key.z_layer, 1);
        assert_eq!(frame_batches.batches[2].key.z_layer, 2);
    }

    #[test]
    fn test_SPEC_r002_stage2_z_layer_batch_separation() {
        // SPEC-R-002 Stage 2: Primitives with identical everything EXCEPT z_layer
        // must produce DISTINCT batches, in ascending z-order.
        use std::cmp::Ordering;

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // All primitives: same clip (none), same pipeline (rect), same color
        // Only difference is z_layer
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 3);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 2);
        batcher.add_rect(RectF::new(30.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        batcher.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 3);
        batcher.add_rect(RectF::new(50.0, 0.0, 10.0, 10.0), Color::WHITE, 2);

        let frame_batches = batcher.build_batches();

        // Must have exactly 3 distinct batches (one per z-layer: 1, 2, 3)
        assert_eq!(
            frame_batches.batch_count(),
            3,
            "SPEC-R-002 Stage 2: different z_layers must produce distinct batches"
        );

        // Batches must be in ascending z-order
        let z_layers: Vec<ZIndex> = frame_batches
            .batches
            .iter()
            .map(|b| b.key.z_layer)
            .collect();
        assert_eq!(
            z_layers,
            vec![1, 2, 3],
            "SPEC-R-002 Stage 2: batches must be in ascending z-order"
        );

        // Verify DrawBatchKey ordering: z_layer is primary comparison key
        let key_z1 = DrawBatchKey::rect(0, 1);
        let key_z2 = DrawBatchKey::rect(0, 2);
        let key_z3 = DrawBatchKey::rect(0, 3);

        assert_eq!(key_z1.cmp(&key_z2), Ordering::Less);
        assert_eq!(key_z2.cmp(&key_z3), Ordering::Less);
        assert_eq!(key_z1.cmp(&key_z3), Ordering::Less);

        // Verify z_layer trumps clip_hash in ordering
        let key_z1_clip100 = DrawBatchKey::rect(100, 1);
        let key_z2_clip0 = DrawBatchKey::rect(0, 2);
        assert_eq!(
            key_z1_clip100.cmp(&key_z2_clip0),
            Ordering::Less,
            "SPEC-R-002 Stage 2: z_layer must be compared before clip_hash"
        );
    }

    #[test]
    fn test_SPEC_r002_stage3_renderer_batch_order_invariant() {
        // SPEC-R-002 Stage 3: Simulate renderer's batch order validation.
        // Verifies that batch sequence produced by batcher satisfies:
        //   batch[i-1].key <= batch[i].key for all i > 0
        //
        // This is the invariant the renderer's debug_assert checks.

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a complex scene with mixed z-layers, clips, and pipelines
        // Insertion order deliberately scrambled to test sorting

        // z=5, no clip
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 5);

        // z=1, no clip
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 1);

        // z=3, with clip
        batcher.push_clip(RectF::new(0.0, 0.0, 100.0, 100.0), None);
        batcher.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 3);
        batcher.pop_clip();

        // z=1, no clip (should coalesce with earlier z=1)
        batcher.add_rect(RectF::new(30.0, 0.0, 10.0, 10.0), Color::WHITE, 1);

        // z=2, no clip
        batcher.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 2);

        // z=3, no clip (different from clipped z=3)
        batcher.add_rect(RectF::new(50.0, 0.0, 10.0, 10.0), Color::WHITE, 3);

        // z=1, with different clip
        batcher.push_clip(RectF::new(50.0, 50.0, 50.0, 50.0), None);
        batcher.add_rect(RectF::new(60.0, 60.0, 10.0, 10.0), Color::WHITE, 1);
        batcher.pop_clip();

        let frame_batches = batcher.build_batches();

        // Verify: batch sequence is monotonically non-decreasing by key
        // This is exactly what the renderer's debug_assert checks
        for i in 1..frame_batches.batches.len() {
            let prev = &frame_batches.batches[i - 1];
            let curr = &frame_batches.batches[i];

            assert!(
                prev.key <= curr.key,
                "SPEC-R-002 Stage 3: batch order invariant violated at index {} - \
                 batch[{}].key ({:?}) > batch[{}].key ({:?})",
                i,
                i - 1,
                prev.key,
                i,
                curr.key
            );

            // Extra check: verify z_layer is non-decreasing
            assert!(
                prev.key.z_layer <= curr.key.z_layer,
                "SPEC-R-002 Stage 3: z_layer order violated at index {} - \
                 z_layer {} > {}",
                i,
                prev.key.z_layer,
                curr.key.z_layer
            );
        }

        // Verify z_layers are in ascending order across the batch sequence
        let z_layers: Vec<_> = frame_batches
            .batches
            .iter()
            .map(|b| b.key.z_layer)
            .collect();

        // Verify sorted (non-decreasing)
        let mut sorted_z = z_layers.clone();
        sorted_z.sort();
        assert_eq!(
            z_layers, sorted_z,
            "SPEC-R-002 Stage 3: z_layers must be in non-decreasing order"
        );
    }

    // ========================================================================
    // SPEC-R-002 Stage 4: Final Cleanup Verification Test
    // ========================================================================

    #[test]
    fn test_SPEC_r002_stage4_no_hidden_ordering_channels() {
        // SPEC-R-002 Stage 4: Verify that ONLY (DrawBatchKey, order_key) controls ordering.
        // Changing primitive geometry or insertion order of primitives with identical keys
        // must NOT affect batch ordering - only the final sorted sequence matters.

        // Test 1: Different positions, same keys → same batch order
        let mut batcher1 = Batcher::new();
        batcher1.begin_frame(SceneKey::BASE);
        batcher1.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher1.add_rect(RectF::new(100.0, 100.0, 10.0, 10.0), Color::WHITE, 1);
        batcher1.add_rect(RectF::new(200.0, 200.0, 10.0, 10.0), Color::WHITE, 2);
        let batches1 = batcher1.build_batches();

        let mut batcher2 = Batcher::new();
        batcher2.begin_frame(SceneKey::BASE);
        // Completely different positions, same z-layers
        batcher2.add_rect(RectF::new(999.0, 999.0, 50.0, 50.0), Color::WHITE, 0);
        batcher2.add_rect(RectF::new(0.0, 0.0, 1.0, 1.0), Color::WHITE, 1);
        batcher2.add_rect(RectF::new(500.0, 500.0, 100.0, 100.0), Color::WHITE, 2);
        let batches2 = batcher2.build_batches();

        // Batch count and z-layer sequence must be identical
        assert_eq!(
            batches1.batch_count(),
            batches2.batch_count(),
            "SPEC-R-002 Stage 4: position changes must not affect batch count"
        );
        for i in 0..batches1.batch_count() {
            assert_eq!(
                batches1.batches[i].key.z_layer, batches2.batches[i].key.z_layer,
                "SPEC-R-002 Stage 4: position changes must not affect batch z_layer ordering"
            );
        }

        // Test 2: Scrambled insertion order with same keys → same final batch order
        let mut batcher3 = Batcher::new();
        batcher3.begin_frame(SceneKey::BASE);
        // Insert in order: z=0, z=1, z=2
        batcher3.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher3.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        batcher3.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 2);
        let batches3 = batcher3.build_batches();

        let mut batcher4 = Batcher::new();
        batcher4.begin_frame(SceneKey::BASE);
        // Insert in scrambled order: z=2, z=0, z=1
        batcher4.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 2);
        batcher4.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher4.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        let batches4 = batcher4.build_batches();

        // Final batch order must be identical (sorted by z_layer)
        assert_eq!(batches3.batch_count(), batches4.batch_count());
        for i in 0..batches3.batch_count() {
            assert_eq!(
                batches3.batches[i].key.z_layer, batches4.batches[i].key.z_layer,
                "SPEC-R-002 Stage 4: scrambled insertion must not affect final batch z_layer order"
            );
        }

        // Test 3: Verify batch sequence is monotonically non-decreasing by key
        let mut batcher5 = Batcher::new();
        batcher5.begin_frame(SceneKey::BASE);
        // Complex scrambled insertion
        batcher5.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 5);
        batcher5.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        batcher5.add_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 3);
        batcher5.add_rect(RectF::new(30.0, 0.0, 10.0, 10.0), Color::WHITE, 1);
        batcher5.add_rect(RectF::new(40.0, 0.0, 10.0, 10.0), Color::WHITE, 5);
        let batches5 = batcher5.build_batches();

        for i in 1..batches5.batches.len() {
            assert!(
                batches5.batches[i - 1].key <= batches5.batches[i].key,
                "SPEC-R-002 Stage 4: batch[{}].key must be <= batch[{}].key",
                i - 1,
                i
            );
        }

        // Test 4: Verify geometry (RectF) does NOT affect DrawBatchKey
        let key1 = DrawBatchKey::rect(0, 0);
        let key2 = DrawBatchKey::rect(0, 0);
        assert_eq!(
            key1, key2,
            "SPEC-R-002 Stage 4: DrawBatchKey must not contain geometry"
        );

        // Test 5: Verify only z_layer, clip_hash, pipeline, texture_id, shape_type_id affect key
        let key_z0 = DrawBatchKey::rect(0, 0);
        let key_z1 = DrawBatchKey::rect(0, 1);
        let key_clip = DrawBatchKey::rect(123, 0);

        assert_ne!(
            key_z0, key_z1,
            "Different z_layer must produce different keys"
        );
        assert_ne!(
            key_z0, key_clip,
            "Different clip_hash must produce different keys"
        );
        assert!(key_z0 < key_z1, "z_layer=0 must sort before z_layer=1");
    }

    // ========================================================================
    // SPEC-R-004 Stage 12: Path Batching Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r004_path_draw_accumulation() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a simple triangle mesh
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Add path draw (Stage 18: z_index = 0)
        let path_draw = PathDraw::fill(mesh, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE);
        batcher.add_path(path_draw, 0);

        // Verify path draw was accumulated
        assert_eq!(batcher.path_draws().len(), 1);
        assert_eq!(batcher.path_draws()[0].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_SPEC_r004_path_draws_transfer_to_frame() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create two meshes
        let mesh1 = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        let mesh2 = PathMesh {
            vertices: vec![[20.0, 0.0], [30.0, 0.0], [25.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(20.0, 0.0, 10.0, 10.0),
        };

        // Add two path draws (Stage 18: z_index = 0)
        batcher.add_path(
            PathDraw::fill(mesh1, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher.add_path(
            PathDraw::stroke(mesh2, [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Verify path draws transferred to frame
        assert_eq!(frame.paths.len(), 2);
        assert_eq!(frame.paths[0].stroke_flag, 0); // fill
        assert_eq!(frame.paths[1].stroke_flag, 1); // stroke

        // Verify path_draws were cleared from batcher after transfer
        assert!(batcher.path_draws().is_empty());
    }

    #[test]
    fn test_SPEC_r004_frame_batches_has_paths() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();

        // Empty frame - no paths
        batcher.begin_frame(SceneKey::BASE);
        let frame1 = batcher.build_batches();
        assert!(!frame1.has_paths());

        // Frame with paths
        batcher.begin_frame(SceneKey::BASE);
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );
        let frame2 = batcher.build_batches();
        assert!(frame2.has_paths());
    }

    #[test]
    fn test_SPEC_r004_path_only_frame() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add only paths, no rects
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Frame should have paths but no regular batches
        assert!(frame.batches.is_empty());
        assert!(frame.has_paths());
        assert_eq!(frame.paths.len(), 1);
    }

    #[test]
    fn test_SPEC_r004_mixed_paths_and_rects() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add rects
        batcher.add_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0);
        batcher.add_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0);

        // Add path
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Should have both batched rects and paths
        assert_eq!(frame.batch_count(), 1); // rects batched together
        assert!(frame.has_paths());
        assert_eq!(frame.paths.len(), 1);
    }

    #[test]
    fn test_SPEC_r004_path_draw_transform() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Test with_transform builder
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_transform([2.0, 0.0, 100.0, 0.0, 2.0, 200.0]);

        assert_eq!(path_draw.transform, [2.0, 0.0, 100.0, 0.0, 2.0, 200.0]);
    }

    #[test]
    fn test_SPEC_r004_path_draw_clip_id() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Default clip_id is 0
        let path_draw1 = PathDraw::fill(mesh.clone(), [1.0, 1.0, 1.0, 1.0], SceneKey::BASE);
        assert_eq!(path_draw1.clip_id, 0);

        // Test with_clip_id builder
        let path_draw2 = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE).with_clip_id(5);
        assert_eq!(path_draw2.clip_id, 5);
    }

    // ========================================================================
    // SPEC-R-004 Stage 13: Clip ID Propagation Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r004_stage13_path_clip_id_no_clip() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // No clip active - path should get clip_id = 0
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        // Verify clip_id = 0 (fullscreen)
        assert_eq!(batcher.path_draws()[0].clip_id, 0);

        // No clip rects should be registered
        assert!(batcher.path_clip_rects().is_empty());
    }

    #[test]
    fn test_SPEC_r004_stage13_path_clip_id_with_clip() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Push a clip region
        batcher.push_clip(RectF::new(10.0, 20.0, 100.0, 200.0), None);

        // Add path inside clip - should get clip_id = 1
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        // Verify clip_id = 1 (first registered clip)
        assert_eq!(batcher.path_draws()[0].clip_id, 1);

        // One clip rect should be registered
        assert_eq!(batcher.path_clip_rects().len(), 1);
        assert_eq!(batcher.path_clip_rects()[0].x, 10.0);
        assert_eq!(batcher.path_clip_rects()[0].y, 20.0);
    }

    #[test]
    fn test_SPEC_r004_stage13_path_clip_id_nested_clips() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let mesh1 = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        let mesh2 = mesh1.clone();
        let mesh3 = mesh1.clone();
        let mesh4 = mesh1.clone();

        // Path at root level (no clip) → clip_id = 0
        batcher.add_path(
            PathDraw::fill(mesh1, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Push first clip
        batcher.push_clip(RectF::new(0.0, 0.0, 500.0, 500.0), None);
        // Path inside first clip → clip_id = 1
        batcher.add_path(
            PathDraw::fill(mesh2, [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Push nested clip (intersects with parent)
        batcher.push_clip(RectF::new(100.0, 100.0, 200.0, 200.0), None);
        // Path inside nested clip → clip_id = 2
        batcher.add_path(
            PathDraw::fill(mesh3, [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        // Pop back to first clip
        batcher.pop_clip();
        // Path back at first clip level → clip_id = 1 (reused)
        batcher.add_path(
            PathDraw::fill(mesh4, [1.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Verify clip_ids
        assert_eq!(batcher.path_draws()[0].clip_id, 0); // no clip
        assert_eq!(batcher.path_draws()[1].clip_id, 1); // first clip
        assert_eq!(batcher.path_draws()[2].clip_id, 2); // nested clip
        assert_eq!(batcher.path_draws()[3].clip_id, 1); // back to first clip (reused)

        // Should have 2 unique clip rects registered
        assert_eq!(batcher.path_clip_rects().len(), 2);
    }

    #[test]
    fn test_SPEC_r004_stage13_path_clips_transfer_to_frame() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Push clip and add path
        batcher.push_clip(RectF::new(50.0, 60.0, 70.0, 80.0), None);
        batcher.add_path(
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Verify path clips transferred to frame
        assert_eq!(frame.path_clips.len(), 1);
        assert_eq!(frame.path_clips[0].x, 50.0);
        assert_eq!(frame.path_clips[0].y, 60.0);
        assert_eq!(frame.path_clips[0].width, 70.0);
        assert_eq!(frame.path_clips[0].height, 80.0);

        // Verify path draw has correct clip_id
        assert_eq!(frame.paths[0].clip_id, 1);

        // Batcher's path_clip_rects should be cleared after transfer
        assert!(batcher.path_clip_rects().is_empty());
    }

    #[test]
    fn test_SPEC_r004_stage13_clip_id_reuse_same_clip() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let mesh1 = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };
        let mesh2 = mesh1.clone();

        // Push clip
        batcher.push_clip(RectF::new(100.0, 100.0, 200.0, 200.0), None);

        // Add two paths at same clip level - should get same clip_id
        batcher.add_path(
            PathDraw::fill(mesh1, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher.add_path(
            PathDraw::fill(mesh2, [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Both should have same clip_id
        assert_eq!(
            batcher.path_draws()[0].clip_id,
            batcher.path_draws()[1].clip_id
        );
        assert_eq!(batcher.path_draws()[0].clip_id, 1);

        // Only one clip rect should be registered (reuse)
        assert_eq!(batcher.path_clip_rects().len(), 1);
    }

    // ========================================================================
    // SPEC-R-004 Stage 14: Transform Packing Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r004_stage14_identity_transform() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[10.0, 20.0], [30.0, 20.0], [20.0, 40.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(10.0, 20.0, 20.0, 20.0),
        };

        // Default PathDraw has identity transform [1, 0, 0, 0, 1, 0]
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE);

        // Verify identity transform
        assert_eq!(path_draw.transform, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);

        // Apply transform to a point: x' = m00*x + m01*y + m02, y' = m10*x + m11*y + m12
        let [m00, m01, m02, m10, m11, m12] = path_draw.transform;
        let (x, y) = (10.0f32, 20.0f32);
        let x_prime = m00 * x + m01 * y + m02;
        let y_prime = m10 * x + m11 * y + m12;

        // Identity should not change the point
        assert!((x_prime - x).abs() < 1e-6);
        assert!((y_prime - y).abs() < 1e-6);
    }

    #[test]
    fn test_SPEC_r004_stage14_translation_transform() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Translation by (100, 200): [1, 0, 100, 0, 1, 200]
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_transform([1.0, 0.0, 100.0, 0.0, 1.0, 200.0]);

        assert_eq!(path_draw.transform, [1.0, 0.0, 100.0, 0.0, 1.0, 200.0]);

        // Apply to origin point
        let [m00, m01, m02, m10, m11, m12] = path_draw.transform;
        let (x, y) = (0.0f32, 0.0f32);
        let x_prime = m00 * x + m01 * y + m02;
        let y_prime = m10 * x + m11 * y + m12;

        // Should translate to (100, 200)
        assert!((x_prime - 100.0).abs() < 1e-6);
        assert!((y_prime - 200.0).abs() < 1e-6);
    }

    #[test]
    fn test_SPEC_r004_stage14_scale_transform() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Scale by 2x: [2, 0, 0, 0, 2, 0]
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_transform([2.0, 0.0, 0.0, 0.0, 2.0, 0.0]);

        assert_eq!(path_draw.transform, [2.0, 0.0, 0.0, 0.0, 2.0, 0.0]);

        // Apply to point (10, 20)
        let [m00, m01, m02, m10, m11, m12] = path_draw.transform;
        let (x, y) = (10.0f32, 20.0f32);
        let x_prime = m00 * x + m01 * y + m02;
        let y_prime = m10 * x + m11 * y + m12;

        // Should scale to (20, 40)
        assert!((x_prime - 20.0).abs() < 1e-6);
        assert!((y_prime - 40.0).abs() < 1e-6);
    }

    #[test]
    fn test_SPEC_r004_stage14_rotation_90_transform() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // 90° CCW rotation: [cos(90), -sin(90), 0, sin(90), cos(90), 0] = [0, -1, 0, 1, 0, 0]
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_transform([0.0, -1.0, 0.0, 1.0, 0.0, 0.0]);

        // Apply to point (10, 0) - should rotate to (0, 10)
        let [m00, m01, m02, m10, m11, m12] = path_draw.transform;
        let (x, y) = (10.0f32, 0.0f32);
        let x_prime = m00 * x + m01 * y + m02;
        let y_prime = m10 * x + m11 * y + m12;

        assert!((x_prime - 0.0).abs() < 1e-6);
        assert!((y_prime - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_SPEC_r004_stage14_transform_preserved_through_batcher() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Create path with custom transform (scale + rotation, zero translation per PM-004)
        let transform = [2.0, 0.5, 0.0, -0.5, 2.0, 0.0];
        let path_draw =
            PathDraw::fill(mesh, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE).with_transform(transform);

        batcher.add_path(path_draw, 0);

        // Verify transform is preserved unchanged
        assert_eq!(batcher.path_draws()[0].transform, transform);

        // Build batches and verify transfer
        let frame = batcher.build_batches();
        assert_eq!(frame.paths[0].transform, transform);
    }

    #[test]
    fn test_SPEC_r004_stage14_transform_layout_matches_wgsl() {
        // Verify transform layout matches WGSL expectations:
        // WGSL: transform_0: vec4<f32> = [m00, m01, m02, m10]
        // WGSL: transform_1: vec2<f32> = [m11, m12]
        //
        // Rust: transform: [f32; 6] = [m00, m01, m02, m10, m11, m12]
        //
        // The transform is packed as [m00, m01, m02, m10, m11, m12] on CPU,
        // then split into transform_0 (first 4) and transform_1 (last 2) on GPU.

        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0]],
            indices: vec![],
            bounds: RectF::zero(),
        };

        // Set up a distinguishable transform
        let transform = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let path_draw =
            PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE).with_transform(transform);

        // Verify layout interpretation
        // transform_0 (float32x4) should be: transform[0..4]
        let transform_0 = &path_draw.transform[0..4];
        assert_eq!(transform_0, [1.0, 2.0, 3.0, 4.0]); // [m00, m01, m02, m10]

        // transform_1 (float32x2) should be: transform[4..6]
        let transform_1 = &path_draw.transform[4..6];
        assert_eq!(transform_1, [5.0, 6.0]); // [m11, m12]
    }

    // ========================================================================
    // SPEC-R-004 Stage 15: Path Color Integration Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r004_stage15_path_color_from_scene_node() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::Path;

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with a non-white color
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.line_to(0.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: None,
            color: Color::from_srgb(0.2, 0.4, 0.8, 0.5),
            is_stroke: false,
            debug: None,
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);

        // Flatten the scene node
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Build batches to get the frame
        let frame = batcher.build_batches();

        // Verify path was created
        assert_eq!(frame.paths.len(), 1);

        // Verify color was propagated correctly.
        // Color::from_srgb stores LINEAR values — expected must match linear space.
        let c = Color::from_srgb(0.2, 0.4, 0.8, 0.5);
        let expected_color = [c.r, c.g, c.b, c.a];
        for j in 0..4 {
            assert!(
                (frame.paths[0].color[j] - expected_color[j]).abs() < 1e-6,
                "color[{}] mismatch: {} vs {}",
                j,
                frame.paths[0].color[j],
                expected_color[j]
            );
        }
    }

    #[test]
    fn test_SPEC_r004_stage15_multiple_colored_paths() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::Path;

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create three paths with different colors
        let colors = [
            Color::from_srgb(1.0, 0.0, 0.0, 1.0), // Red
            Color::from_srgb(0.0, 1.0, 0.0, 0.8), // Green with alpha
            Color::from_srgb(0.0, 0.0, 1.0, 0.5), // Blue with alpha
        ];

        let scene_nodes: Vec<_> = colors
            .iter()
            .map(|&color| {
                let mut path = Path::new();
                path.move_to(0.0, 0.0);
                path.line_to(50.0, 0.0);
                path.line_to(25.0, 50.0);
                path.close();

                SceneNode::Path(PathNode {
                    fill_rule: FillRule::NonZero,
                    path,
                    mesh: None,
                    color,
                    is_stroke: false,
                    debug: None,
                    z_index: 0,
                    key: None,
                })
            })
            .collect();

        // Flatten all scene nodes
        batcher.flatten_scene(&scene_nodes, &mut MeshCache::new(), 1.0, 0);

        // Build batches
        let frame = batcher.build_batches();

        // Verify all paths were created
        assert_eq!(frame.paths.len(), 3);

        // Verify colors in correct order
        let expected_colors: [[f32; 4]; 3] = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.8],
            [0.0, 0.0, 1.0, 0.5],
        ];

        for (i, expected) in expected_colors.iter().enumerate() {
            for j in 0..4 {
                assert!(
                    (frame.paths[i].color[j] - expected[j]).abs() < 1e-6,
                    "path {} color[{}] mismatch: {} vs {}",
                    i,
                    j,
                    frame.paths[i].color[j],
                    expected[j]
                );
            }
        }
    }

    #[test]
    fn test_SPEC_r004_stage15_path_with_precomputed_mesh_color() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with pre-computed mesh
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::from_srgb(0.5, 0.25, 0.75, 0.9),
            is_stroke: false,
            debug: None,
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);

        // Flatten the scene node
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Build batches
        let frame = batcher.build_batches();

        // Verify path was created with mesh
        assert_eq!(frame.paths.len(), 1);

        // Verify color from pre-computed mesh path.
        // Color::from_srgb stores LINEAR values — expected must match linear space.
        let c = Color::from_srgb(0.5, 0.25, 0.75, 0.9);
        let expected_color = [c.r, c.g, c.b, c.a];
        for j in 0..4 {
            assert!(
                (frame.paths[0].color[j] - expected_color[j]).abs() < 1e-6,
                "color[{}] mismatch: {} vs {}",
                j,
                frame.paths[0].color[j],
                expected_color[j]
            );
        }
    }

    #[test]
    fn test_SPEC_r004_stage15_scene_builder_push_path() {
        use crate::core::scene::{Color, SceneBuilder};
        use crate::renderer::path::Path;

        let mut builder = SceneBuilder::new();

        // Create a path using SceneBuilder
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        builder.push_path(path, Color::from_srgb(0.3, 0.6, 0.9, 1.0), 0, None);

        let scene = builder.build();

        // Flatten and verify
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.flatten_scene(&scene.root, &mut MeshCache::new(), 1.0, 0);

        let frame = batcher.build_batches();

        assert_eq!(frame.paths.len(), 1);
        // Color::from_srgb stores LINEAR values — expected must match linear space.
        let c = Color::from_srgb(0.3, 0.6, 0.9, 1.0);
        let expected_color = [c.r, c.g, c.b, c.a];
        for j in 0..4 {
            assert!(
                (frame.paths[0].color[j] - expected_color[j]).abs() < 1e-6,
                "color[{}] mismatch: {} vs {}",
                j,
                frame.paths[0].color[j],
                expected_color[j]
            );
        }
    }

    // ========================================================================
    // SPEC-R-004 Stage 16: Stroke Flag Plumbing Tests
    // ========================================================================

    #[test]
    fn test_SPEC_r004_stage16_path_stroke_flag_from_scene_node() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::Path;

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with is_stroke = true
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.line_to(0.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: None,
            color: Color::from_srgb(0.8, 0.2, 0.4, 1.0),
            is_stroke: true, // Stage 16: mark as stroke
            debug: None,
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);

        // Flatten the scene node
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Build batches to get the frame
        let frame = batcher.build_batches();

        // Verify path was created
        assert_eq!(frame.paths.len(), 1);

        // Verify stroke_flag = 1 (stroke)
        assert_eq!(
            frame.paths[0].stroke_flag, 1,
            "stroke path should have stroke_flag = 1"
        );
    }

    #[test]
    fn test_SPEC_r004_stage16_path_fill_stroke_flag_default_zero() {
        use crate::core::scene::{Color, SceneBuilder};
        use crate::renderer::path::Path;

        let mut builder = SceneBuilder::new();

        // Create a path using SceneBuilder (which sets is_stroke = false)
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        builder.push_path(path, Color::from_srgb(0.3, 0.6, 0.9, 1.0), 0, None);

        let scene = builder.build();

        // Flatten and verify
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.flatten_scene(&scene.root, &mut MeshCache::new(), 1.0, 0);

        let frame = batcher.build_batches();

        // Verify path was created
        assert_eq!(frame.paths.len(), 1);

        // Verify stroke_flag = 0 (fill)
        assert_eq!(
            frame.paths[0].stroke_flag, 0,
            "fill path should have stroke_flag = 0"
        );
    }

    #[test]
    fn test_SPEC_r004_stage16_stroke_vs_fill_only_flag_differs() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::Path;

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create identical paths, one fill, one stroke
        let create_path = || {
            let mut path = Path::new();
            path.move_to(10.0, 10.0);
            path.line_to(90.0, 10.0);
            path.line_to(50.0, 80.0);
            path.close();
            path
        };

        let color = Color::from_srgb(0.5, 0.5, 0.5, 1.0);

        // Fill path
        let fill_node = PathNode {
            fill_rule: FillRule::NonZero,
            path: create_path(),
            mesh: None,
            color,
            is_stroke: false,
            debug: None,
            z_index: 0,
            key: None,
        };

        // Stroke path (identical geometry and color)
        let stroke_node = PathNode {
            fill_rule: FillRule::NonZero,
            path: create_path(),
            mesh: None,
            color,
            is_stroke: true,
            debug: None,
            z_index: 0,
            key: None,
        };

        let scene_nodes = vec![SceneNode::Path(fill_node), SceneNode::Path(stroke_node)];

        // Flatten both
        batcher.flatten_scene(&scene_nodes, &mut MeshCache::new(), 1.0, 0);

        let frame = batcher.build_batches();

        // Verify both paths created
        assert_eq!(frame.paths.len(), 2);

        // Verify colors match
        assert_eq!(frame.paths[0].color, frame.paths[1].color);

        // Verify transforms match (both identity)
        assert_eq!(frame.paths[0].transform, frame.paths[1].transform);

        // Verify clip_ids match (both 0)
        assert_eq!(frame.paths[0].clip_id, frame.paths[1].clip_id);

        // Only stroke_flag differs
        assert_eq!(frame.paths[0].stroke_flag, 0, "first path should be fill");
        assert_eq!(
            frame.paths[1].stroke_flag, 1,
            "second path should be stroke"
        );
    }

    #[test]
    fn test_SPEC_r004_stage16_stroke_with_precomputed_mesh() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with pre-computed mesh and is_stroke = true
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::from_srgb(0.9, 0.1, 0.1, 1.0),
            is_stroke: true,
            debug: None,
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);

        // Flatten the scene node
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Build batches
        let frame = batcher.build_batches();

        // Verify path was created
        assert_eq!(frame.paths.len(), 1);

        // Verify stroke_flag = 1 even with pre-computed mesh
        assert_eq!(
            frame.paths[0].stroke_flag, 1,
            "stroke path with mesh should have stroke_flag = 1"
        );
    }

    // ========================================================================
    // SPEC-R-004 Stage 17: Path Debug Geometry Overlay Tests
    // ========================================================================

    #[test]
    fn test_stage17_vertices_overlay() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a triangle mesh with 3 vertices
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::Vertices),
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Should have path draws + vertex overlay rects
        // 1 path + 3 vertex markers (rects)
        assert_eq!(batcher.path_draws().len(), 1, "should have 1 path draw");

        // Vertex overlays are emitted as rects
        let rect_count = batcher
            .primitives()
            .iter()
            .filter(|p| matches!(p.data, PrimitiveData::Rect { .. }))
            .count();

        assert_eq!(rect_count, 3, "should have 3 vertex marker rects");
    }

    #[test]
    fn test_stage17_edges_overlay() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a square with 4 vertices (5 points including close)
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
            indices: vec![0, 1, 2, 0, 2, 3], // 2 triangles
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.line_to(0.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::Edges),
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Edges mode: emit path-stroke overlays between consecutive vertices.
        // 4 vertices → 3 edges (between consecutive pairs)
        let line_count = debug_path_draw_count(&batcher);

        assert_eq!(line_count, 3, "should have 3 edge lines (4 vertices - 1)");
    }

    fn assert_debug_edge_segments(batcher: &Batcher, expected: &[[[f32; 2]; 2]]) {
        let mut lines = batcher
            .path_draws()
            .iter()
            .filter(|draw| draw.color == [1.0, 0.0, 1.0, 1.0]);
        for &expected_segment in expected {
            let draw = lines.next().expect("source contour edge");
            let &[start_left, start_right, end_left, end_right] = draw.mesh.vertices.as_slice()
            else {
                panic!("edge diagnostic must emit a butt-capped segment");
            };
            assert_eq!(draw.mesh.indices.len(), 6);
            let midpoint = |a: [f32; 2], b: [f32; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            let segment = [
                midpoint(start_left, start_right),
                midpoint(end_left, end_right),
            ];
            for (actual, expected) in segment.into_iter().zip(expected_segment) {
                assert!(
                    actual
                        .into_iter()
                        .zip(expected)
                        .all(|(a, b)| (a - b).abs() < 1e-5),
                    "expected endpoint {expected:?}, got {actual:?}",
                );
            }
        }
        assert!(lines.next().is_none(), "unexpected edge or bridge");
    }

    #[test]
    fn test_stage17_generated_square_edges_follow_the_closed_contour() {
        let mut builder = crate::core::scene::SceneBuilder::new();
        builder.push_path_debug(
            Path::rounded_rect(2.0, 3.0, 6.0, 6.0, 0.0),
            Color::WHITE,
            PathDebugMode::Edges,
            0,
            None,
        );
        let scene = builder.build();
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.flatten_scene(&scene.root, &mut MeshCache::new(), 1.0, 0);
        assert_debug_edge_segments(
            &batcher,
            &[
                [[2.0, 3.0], [8.0, 3.0]],
                [[8.0, 3.0], [8.0, 9.0]],
                [[8.0, 9.0], [2.0, 9.0]],
                [[2.0, 9.0], [2.0, 3.0]],
            ],
        );
    }

    #[test]
    fn test_stage17_generated_disconnected_edges_match_the_warm_projection() {
        let mut path = Path::new();
        path.move_to(2.2, 3.1);
        path.line_to(7.0, 3.1);
        path.line_to(7.18, 3.15); // Snaps to the previous point at this DPR.
        path.line_to(7.0, 8.6);
        path.line_to(2.2, 8.6); // Implicitly closed by the next MoveTo.
        path.move_to(14.1, 3.31);
        path.line_to(21.1, 3.31);
        path.line_to(21.1, 8.9);
        path.line_to(14.1, 8.9);
        path.close();

        let scale = 1.25_f32;
        let bounds = path.bounds();
        let key = MeshCacheKey {
            path_hash: path.content_hash(),
            render_width_q: MeshCacheKey::quantize_size(bounds.width),
            render_height_q: MeshCacheKey::quantize_size(bounds.height),
            stroke_width_q: 0,
            is_fill: true,
            fill_rule: FillRule::NonZero,
            dpr_q: MeshCacheKey::quantize_dpr(scale),
            dpr_bits: scale.to_bits(),
            snap_mode: PixelSnapMode::Center,
        };
        let mut builder = crate::core::scene::SceneBuilder::with_scale_factor(scale);
        builder.push_path_debug(path, Color::WHITE, PathDebugMode::Edges, 0, None);
        let scene = builder.build();
        let mut cache = MeshCache::new();
        let mut batcher = Batcher::new();
        let mut cached_vertices = None;
        for frame in [1, 2, 3] {
            batcher.begin_frame(SceneKey::BASE);
            batcher.flatten_scene(&scene.root, &mut cache, scale, frame);
            assert_debug_edge_segments(
                &batcher,
                &[
                    [[2.4, 3.2], [7.2, 3.2]],
                    [[7.2, 3.2], [7.2, 8.8]],
                    [[7.2, 8.8], [2.4, 8.8]],
                    [[2.4, 8.8], [2.4, 3.2]],
                    [[14.4, 3.2], [20.8, 3.2]],
                    [[20.8, 3.2], [20.8, 8.8]],
                    [[20.8, 8.8], [14.4, 8.8]],
                    [[14.4, 8.8], [14.4, 3.2]],
                ],
            );
            assert_eq!(cache.len(), 1);
            let vertices = cache.get(&key, frame).unwrap().vertices.as_ptr();
            assert_eq!(*cached_vertices.get_or_insert(vertices), vertices);
        }
    }

    #[test]
    fn test_stage17_triangles_overlay() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a single triangle mesh
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2], // 1 triangle
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::Triangles),
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Triangles mode: emit 3 path-stroke overlays per triangle.
        // 1 triangle → 3 lines
        let line_count = debug_path_draw_count(&batcher);

        assert_eq!(line_count, 3, "should have exactly 3 lines for 1 triangle");
    }

    #[test]
    fn test_stage17_debug_none_no_overlays() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with debug = None
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: None, // No debug mode
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Should have path but no overlays
        assert_eq!(batcher.path_draws().len(), 1);

        // No rect or line primitives should be emitted
        let overlay_count = batcher
            .primitives()
            .iter()
            .filter(|p| {
                matches!(p.data, PrimitiveData::Rect { .. })
                    || matches!(p.data, PrimitiveData::Line { .. })
            })
            .count();

        assert_eq!(
            overlay_count, 0,
            "should have zero overlays when debug=None"
        );
    }

    #[test]
    fn test_stage17_debug_none_explicit() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a path with debug = Some(PathDebugMode::None)
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(50.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::None), // Explicit None mode
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // Even with Some(None), should produce no overlays
        let overlay_count = batcher
            .primitives()
            .iter()
            .filter(|p| {
                matches!(p.data, PrimitiveData::Rect { .. })
                    || matches!(p.data, PrimitiveData::Line { .. })
            })
            .count();

        assert_eq!(
            overlay_count, 0,
            "should have zero overlays when debug=Some(None)"
        );
    }

    #[test]
    fn test_stage17_scene_builder_debug() {
        use crate::core::scene::SceneBuilder;
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        // Test push_path_debug
        let mut builder = SceneBuilder::new();

        let mut path1 = Path::new();
        path1.move_to(0.0, 0.0);
        path1.line_to(10.0, 0.0);
        path1.line_to(5.0, 10.0);
        path1.close();

        builder.push_path_debug(path1, Color::WHITE, PathDebugMode::Vertices, 0, None);

        let scene = builder.build();
        assert_eq!(scene.root.len(), 1);

        // Verify the PathNode has debug set correctly
        if let SceneNode::Path(ref path_node) = scene.root[0] {
            assert_eq!(path_node.debug, Some(PathDebugMode::Vertices));
        } else {
            panic!("Expected PathNode");
        }

        // Test push_path_with_mesh_debug
        let mut builder2 = SceneBuilder::new();

        let mut path2 = Path::new();
        path2.move_to(0.0, 0.0);
        path2.line_to(10.0, 0.0);
        path2.line_to(5.0, 10.0);
        path2.close();

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        builder2.push_path_with_mesh_debug(
            path2,
            mesh,
            Color::WHITE,
            PathDebugMode::Triangles,
            0,
            None,
        );

        let scene2 = builder2.build();
        assert_eq!(scene2.root.len(), 1);

        // Verify the PathNode has debug and mesh set correctly
        if let SceneNode::Path(ref path_node) = scene2.root[0] {
            assert_eq!(path_node.debug, Some(PathDebugMode::Triangles));
            assert!(path_node.mesh.is_some());
        } else {
            panic!("Expected PathNode");
        }
    }

    #[test]
    fn test_stage17_path_draw_with_debug_builder() {
        use crate::renderer::path::{PathDebugMode, PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Default has no debug
        let pd1 = PathDraw::fill(mesh.clone(), [1.0, 1.0, 1.0, 1.0], SceneKey::BASE);
        assert_eq!(pd1.debug, None);

        // with_debug sets the mode
        let pd2 = PathDraw::fill(mesh.clone(), [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_debug(PathDebugMode::Vertices);
        assert_eq!(pd2.debug, Some(PathDebugMode::Vertices));

        let pd3 = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE)
            .with_debug(PathDebugMode::Triangles);
        assert_eq!(pd3.debug, Some(PathDebugMode::Triangles));
    }

    #[test]
    fn test_stage17_multiple_triangles_overlay() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create a mesh with 2 triangles (a square)
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
            indices: vec![0, 1, 2, 0, 2, 3], // 2 triangles
            bounds: RectF::new(0.0, 0.0, 100.0, 100.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.line_to(0.0, 100.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::Triangles),
            z_index: 0,
            key: None,
        };

        let scene_node = SceneNode::Path(path_node);
        batcher.flatten_scene(&[scene_node], &mut MeshCache::new(), 1.0, 0);

        // 2 triangles → 6 path-stroke overlays (3 per triangle)
        let line_count = debug_path_draw_count(&batcher);

        assert_eq!(line_count, 6, "should have 6 lines for 2 triangles");
    }

    // ========================================================================
    // SPEC-R-004 Stage 18: Path Instance Deterministic Ordering Tests
    // ========================================================================

    #[test]
    fn test_stage18_path_sort_key_ordering() {
        // Test that PathSortKey provides correct ordering
        let key1 = PathSortKey {
            effective_order: EffectivePaintOrder::scalar(0),
            z_index: 0,
            sort_bias: 0,
            clip_id: 0,
            stroke_flag: 0,
            debug_mode: 0,
            insertion_order: 0,
        };
        let key2 = PathSortKey {
            effective_order: EffectivePaintOrder::scalar(1),
            z_index: 1,
            sort_bias: 0,
            clip_id: 0,
            stroke_flag: 0,
            debug_mode: 0,
            insertion_order: 0,
        };
        let key3 = PathSortKey {
            effective_order: EffectivePaintOrder::scalar(0),
            z_index: 0,
            sort_bias: 0,
            clip_id: 1,
            stroke_flag: 0,
            debug_mode: 0,
            insertion_order: 0,
        };
        let key4 = PathSortKey {
            effective_order: EffectivePaintOrder::scalar(0),
            z_index: 0,
            sort_bias: 0,
            clip_id: 0,
            stroke_flag: 1,
            debug_mode: 0,
            insertion_order: 0,
        };

        // z_index is primary sort key
        assert!(key1 < key2, "z_index 0 < z_index 1");

        // clip_id is secondary (when z equal)
        assert!(key1 < key3, "clip_id 0 < clip_id 1 when z equal");

        // stroke_flag is tertiary (when z and clip equal)
        assert!(
            key1 < key4,
            "stroke_flag 0 < stroke_flag 1 when z,clip equal"
        );
    }

    #[test]
    fn test_stage18_deterministic_ordering_multiple_runs() {
        use crate::renderer::path::{PathDraw, PathMesh};

        // Create identical scenes twice and verify identical path order
        let create_scene = || {
            let mut batcher = Batcher::new();
            batcher.begin_frame(SceneKey::BASE);

            let mesh = PathMesh {
                vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
                indices: vec![0, 1, 2],
                bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
            };

            // Add paths in specific order with different z values
            batcher.add_path(
                PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
                2,
            );
            batcher.add_path(
                PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
                0,
            );
            batcher.add_path(
                PathDraw::fill(mesh.clone(), [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
                1,
            );

            batcher.build_batches()
        };

        let frame1 = create_scene();
        let frame2 = create_scene();

        // Verify both frames have same number of paths
        assert_eq!(frame1.paths.len(), frame2.paths.len());

        // Verify paths are in identical order (sorted by z_index)
        for i in 0..frame1.paths.len() {
            assert_eq!(
                frame1.paths[i].z_index, frame2.paths[i].z_index,
                "path {} z_index mismatch",
                i
            );
            assert_eq!(
                frame1.paths[i].color, frame2.paths[i].color,
                "path {} color mismatch",
                i
            );
        }

        // Verify sorted order: z=0 (green), z=1 (blue), z=2 (red)
        assert_eq!(frame1.paths[0].z_index, 0);
        assert_eq!(frame1.paths[1].z_index, 1);
        assert_eq!(frame1.paths[2].z_index, 2);
    }

    #[test]
    fn test_stage18_ordering_unaffected_by_input_order() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        // Create scene with forward order
        let mut batcher1 = Batcher::new();
        batcher1.begin_frame(SceneKey::BASE);
        batcher1.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher1.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            1,
        );
        batcher1.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
            2,
        );
        let frame1 = batcher1.build_batches();

        // Create scene with reverse input order (same z values)
        let mut batcher2 = Batcher::new();
        batcher2.begin_frame(SceneKey::BASE);
        batcher2.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
            2,
        );
        batcher2.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            1,
        );
        batcher2.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        let frame2 = batcher2.build_batches();

        // Both should produce identical z_index ordering
        assert_eq!(frame1.paths[0].z_index, frame2.paths[0].z_index);
        assert_eq!(frame1.paths[1].z_index, frame2.paths[1].z_index);
        assert_eq!(frame1.paths[2].z_index, frame2.paths[2].z_index);

        // Sorted order: z=0, z=1, z=2
        assert_eq!(frame1.paths[0].z_index, 0);
        assert_eq!(frame1.paths[1].z_index, 1);
        assert_eq!(frame1.paths[2].z_index, 2);
    }

    #[test]
    fn test_stage18_stroke_after_fill() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add stroke before fill at same z_index
        batcher.add_path(
            PathDraw::stroke(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // After sorting: fill (stroke_flag=0) should come before stroke (stroke_flag=1)
        assert_eq!(frame.paths[0].stroke_flag, 0, "fill should come first");
        assert_eq!(frame.paths[1].stroke_flag, 1, "stroke should come second");
    }

    #[test]
    fn test_stage18_debug_overlays_stable() {
        use crate::core::scene::{Color, PathNode, SceneNode};
        use crate::renderer::path::{Path, PathDebugMode, PathMesh};

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create path with debug overlay
        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(10.0, 0.0);
        path.line_to(5.0, 10.0);
        path.close();

        let path_node = PathNode {
            fill_rule: FillRule::NonZero,
            path,
            mesh: Some(mesh),
            color: Color::WHITE,
            is_stroke: false,
            debug: Some(PathDebugMode::Vertices),
            z_index: 0,
            key: None,
        };

        batcher.flatten_scene(&[SceneNode::Path(path_node)], &mut MeshCache::new(), 1.0, 0);

        // Verify path draw at z=0 and debug overlays at high z
        assert_eq!(batcher.path_draws().len(), 1);
        assert_eq!(batcher.path_draws()[0].z_index, 0);

        // Debug rects should be at z = u32::MAX - 1
        for prim in batcher.primitives() {
            if matches!(prim.data, PrimitiveData::Rect { .. }) {
                assert_eq!(
                    prim.key.z_layer,
                    u32::MAX - 1,
                    "debug overlay rects should be at z = u32::MAX - 1"
                );
            }
        }
    }

    #[test]
    fn test_stage18_mixed_debug_and_non_debug() {
        use crate::renderer::path::{PathDebugMode, PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add normal path
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Add path with debug mode
        let mut debug_path = PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE);
        debug_path.debug = Some(PathDebugMode::Vertices);
        batcher.add_path(debug_path, 0);

        let frame = batcher.build_batches();

        // Both paths should be present
        assert_eq!(frame.paths.len(), 2);

        // Normal path (debug=None → ordinal 0) should come before debug path (ordinal 1)
        let key0 = PathSortKey::from_path_draw(&frame.paths[0]);
        let key1 = PathSortKey::from_path_draw(&frame.paths[1]);
        assert!(key0 <= key1, "paths should be in sorted order");
    }

    #[test]
    fn test_stage18_insertion_order_tiebreaker() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add three paths with identical z, clip, stroke, debug
        // Only insertion order should differ
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Should preserve insertion order as tiebreaker
        assert_eq!(frame.paths[0].insertion_order, 0);
        assert_eq!(frame.paths[1].insertion_order, 1);
        assert_eq!(frame.paths[2].insertion_order, 2);

        // Colors should be in insertion order
        assert_eq!(frame.paths[0].color, [1.0, 0.0, 0.0, 1.0]); // red first
        assert_eq!(frame.paths[1].color, [0.0, 1.0, 0.0, 1.0]); // green second
        assert_eq!(frame.paths[2].color, [0.0, 0.0, 1.0, 1.0]); // blue third
    }

    #[test]
    fn test_stage18_z_index_assignment_from_batcher() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Create PathDraw with default z_index=0, pass z=5 to add_path
        let path_draw = PathDraw::fill(mesh, [1.0, 1.0, 1.0, 1.0], SceneKey::BASE);
        assert_eq!(path_draw.z_index, 0, "PathDraw default z_index should be 0");

        batcher.add_path(path_draw, 5);

        // After add_path, z_index should be assigned from the parameter
        assert_eq!(
            batcher.path_draws()[0].z_index,
            5,
            "z_index should be assigned by add_path"
        );
    }

    #[test]
    fn test_stage18_clip_id_secondary_sort() {
        use crate::renderer::path::{PathDraw, PathMesh};

        let mesh = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: RectF::new(0.0, 0.0, 10.0, 10.0),
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add path with no clip (clip_id = 0)
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Push clip and add path (clip_id = 1)
        batcher.push_clip(RectF::new(0.0, 0.0, 100.0, 100.0), None);
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        let frame = batcher.build_batches();

        // Both paths at z=0, sorted by clip_id
        assert_eq!(frame.paths[0].clip_id, 0, "clip_id=0 should come first");
        assert_eq!(frame.paths[1].clip_id, 1, "clip_id=1 should come second");
    }

    // ========================================================================
    // SPEC-R-004 Stage 19: Path Instance Bounds Tests
    // ========================================================================

    #[test]
    fn test_stage19_bounds_propagation() {
        // Test A: bounds propagation from PathMesh through PathDraw to Frame
        use crate::renderer::path::{PathDraw, PathMesh};

        // Construct PathMesh with specific bounds
        let original_bounds = RectF::new(10.0, 20.0, 30.0, 40.0);
        let mesh = PathMesh {
            vertices: vec![[10.0, 20.0], [40.0, 20.0], [25.0, 60.0]],
            indices: vec![0, 1, 2],
            bounds: original_bounds,
        };

        // Create PathDraw - bounds should be copied from mesh
        let path_draw = PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE);
        assert_eq!(
            path_draw.bounds.x, original_bounds.x,
            "PathDraw.bounds.x should match mesh"
        );
        assert_eq!(
            path_draw.bounds.y, original_bounds.y,
            "PathDraw.bounds.y should match mesh"
        );
        assert_eq!(
            path_draw.bounds.width, original_bounds.width,
            "PathDraw.bounds.width should match mesh"
        );
        assert_eq!(
            path_draw.bounds.height, original_bounds.height,
            "PathDraw.bounds.height should match mesh"
        );

        // Add to batcher and build
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.add_path(path_draw, 0);
        let frame = batcher.build_batches();

        // Verify bounds propagated to frame
        assert_eq!(frame.paths.len(), 1);
        assert_eq!(
            frame.paths[0].bounds.x, original_bounds.x,
            "Frame path bounds.x should match original"
        );
        assert_eq!(
            frame.paths[0].bounds.y, original_bounds.y,
            "Frame path bounds.y should match original"
        );
        assert_eq!(
            frame.paths[0].bounds.width, original_bounds.width,
            "Frame path bounds.width should match original"
        );
        assert_eq!(
            frame.paths[0].bounds.height, original_bounds.height,
            "Frame path bounds.height should match original"
        );
    }

    #[test]
    fn test_stage19_bounds_stable_under_sorting() {
        // Test B: bounds remain attached to their PathDraws after sorting
        use crate::renderer::path::{PathDraw, PathMesh};

        // Three meshes with distinct bounds
        let bounds1 = RectF::new(0.0, 0.0, 10.0, 10.0);
        let bounds2 = RectF::new(100.0, 100.0, 20.0, 20.0);
        let bounds3 = RectF::new(200.0, 200.0, 30.0, 30.0);

        let mesh1 = PathMesh {
            vertices: vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]],
            indices: vec![0, 1, 2],
            bounds: bounds1,
        };
        let mesh2 = PathMesh {
            vertices: vec![[100.0, 100.0], [120.0, 100.0], [110.0, 120.0]],
            indices: vec![0, 1, 2],
            bounds: bounds2,
        };
        let mesh3 = PathMesh {
            vertices: vec![[200.0, 200.0], [230.0, 200.0], [215.0, 230.0]],
            indices: vec![0, 1, 2],
            bounds: bounds3,
        };

        // Add paths with different z_index (out of order)
        // z=2 (red, bounds1), z=0 (green, bounds2), z=1 (blue, bounds3)
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);
        batcher.add_path(
            PathDraw::fill(mesh1, [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            2,
        ); // z=2, bounds1
        batcher.add_path(
            PathDraw::fill(mesh2, [0.0, 1.0, 0.0, 1.0], SceneKey::BASE),
            0,
        ); // z=0, bounds2
        batcher.add_path(
            PathDraw::fill(mesh3, [0.0, 0.0, 1.0, 1.0], SceneKey::BASE),
            1,
        ); // z=1, bounds3
        let frame = batcher.build_batches();

        // After sorting by z_index: z=0 (green/bounds2), z=1 (blue/bounds3), z=2 (red/bounds1)
        assert_eq!(frame.paths.len(), 3);

        // z=0 should have bounds2 (green)
        assert_eq!(frame.paths[0].z_index, 0);
        assert_eq!(
            frame.paths[0].bounds.x, bounds2.x,
            "z=0 path should have bounds2"
        );
        assert_eq!(
            frame.paths[0].color,
            [0.0, 1.0, 0.0, 1.0],
            "z=0 should be green"
        );

        // z=1 should have bounds3 (blue)
        assert_eq!(frame.paths[1].z_index, 1);
        assert_eq!(
            frame.paths[1].bounds.x, bounds3.x,
            "z=1 path should have bounds3"
        );
        assert_eq!(
            frame.paths[1].color,
            [0.0, 0.0, 1.0, 1.0],
            "z=1 should be blue"
        );

        // z=2 should have bounds1 (red)
        assert_eq!(frame.paths[2].z_index, 2);
        assert_eq!(
            frame.paths[2].bounds.x, bounds1.x,
            "z=2 path should have bounds1"
        );
        assert_eq!(
            frame.paths[2].color,
            [1.0, 0.0, 0.0, 1.0],
            "z=2 should be red"
        );
    }

    #[test]
    fn test_stage19_debug_overlays_unaffected() {
        // Test C: debug overlays remain primitive-level only, PathDraw bounds don't create duplicates
        use crate::renderer::path::{PathDebugMode, PathDraw, PathMesh};

        let bounds = RectF::new(50.0, 50.0, 100.0, 100.0);
        let mesh = PathMesh {
            vertices: vec![[50.0, 50.0], [150.0, 50.0], [100.0, 150.0]],
            indices: vec![0, 1, 2],
            bounds,
        };

        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        // Add a regular path
        batcher.add_path(
            PathDraw::fill(mesh.clone(), [1.0, 0.0, 0.0, 1.0], SceneKey::BASE),
            0,
        );

        // Add a path with debug mode enabled (this uses push_path_debug internally)
        let debug_path = PathDraw::fill(mesh.clone(), [0.0, 1.0, 0.0, 1.0], SceneKey::BASE)
            .with_debug(PathDebugMode::Triangles);
        batcher.add_path(debug_path, 0);

        let frame = batcher.build_batches();

        // Verify: exactly 2 PathDraw entries (one regular, one debug)
        // PathDraw.bounds should be set on both, but no extra bounding box primitives generated
        assert_eq!(frame.paths.len(), 2, "Should have exactly 2 path draws");

        // Both should have bounds set correctly
        assert_eq!(
            frame.paths[0].bounds.x, bounds.x,
            "Regular path should have bounds"
        );
        assert_eq!(
            frame.paths[1].bounds.x, bounds.x,
            "Debug path should have bounds"
        );

        // Debug path should have debug mode set
        assert!(
            frame.paths[0].debug.is_none(),
            "First path should not have debug mode"
        );
        assert_eq!(
            frame.paths[1].debug,
            Some(PathDebugMode::Triangles),
            "Second path should have debug mode"
        );

        // Debug rects (from push_path_debug) are primitive-level overlays,
        // they don't become additional PathDraw entries. Bounds field is purely data storage.
        // The debug_rects batch contains the primitive debug overlays, not path-level bounds boxes.
    }

    // NOTE: Previous tests `test_scene_key_required_panics_without_set` and
    // `test_scene_key_required_panics_for_path_without_set` were removed because
    // SPEC-HAL-081A enforcement is now by construction: begin_frame(scene_key) requires
    // a SceneKey parameter, making "unset scene_key" state unrepresentable at the type level.

    // ========================================================================
    // SPEC-PM-004: Geometry Authoring Space Contract Tests
    // ========================================================================

    #[test]
    fn test_pm004_rounded_rect_no_double_scroll() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let scroll_y: f32 = 75.0;
        let original_y: f32 = 200.0;
        batcher.push_scroll((0.0, scroll_y));
        batcher.add_rounded_rect(
            RectF::new(10.0, original_y, 80.0, 30.0),
            Color::WHITE,
            4.0,
            0,
            &mut MeshCache::new(),
            1.0,
            0,
        );

        let (quad_rect, rect_center, rect_half_size, radius) = first_sdf_rect_metrics(&batcher);
        let expected_scrolled_y = original_y - scroll_y;

        assert_eq!(radius, 4.0);
        assert!((rect_half_size.0 - 40.0).abs() < 0.5);
        assert!((rect_half_size.1 - 15.0).abs() < 0.5);

        // SDF rect geometry is emitted in scroll-adjusted viewport space.
        assert!(
            ((rect_center.1 - rect_half_size.1) - expected_scrolled_y).abs() < 0.5,
            "PM-004: SDF rect top ({:.1}) must equal original_y - scroll_y ({:.1})",
            rect_center.1 - rect_half_size.1,
            expected_scrolled_y
        );
        // Quad expands by 1px for SDF AA room.
        assert!(
            (quad_rect.y - (expected_scrolled_y - 1.0)).abs() < 0.5,
            "PM-004: SDF quad y ({:.1}) must equal scrolled top - AA expansion ({:.1})",
            quad_rect.y,
            expected_scrolled_y - 1.0
        );
        // Reject double-scroll.
        let double_scrolled_y = original_y - 2.0 * scroll_y;
        assert!(
            ((rect_center.1 - rect_half_size.1) - double_scrolled_y).abs() > 1.0,
            "PM-004: SDF rect top ({:.1}) must NOT be double-scrolled ({:.1})",
            rect_center.1 - rect_half_size.1,
            double_scrolled_y
        );
    }

    #[test]
    fn test_pm004_rounded_border_no_double_scroll() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let scroll_y: f32 = 50.0;
        let original_y: f32 = 150.0;
        batcher.push_scroll((0.0, scroll_y));
        batcher.add_rounded_border(
            RectF::new(10.0, original_y, 80.0, 30.0),
            Color::WHITE,
            Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
            1.0,
            4.0,
            0,
            &mut MeshCache::new(),
            1.0,
            0,
        );

        let (_quad_rect, rect_center, rect_half_size, radius) = first_sdf_rect_metrics(&batcher);
        let expected_scrolled_y = original_y - scroll_y;

        assert_eq!(radius, 4.0);
        assert!(
            ((rect_center.1 - rect_half_size.1) - expected_scrolled_y).abs() < 0.5,
            "PM-004: SDF border top ({:.1}) must equal original_y - scroll_y ({:.1})",
            rect_center.1 - rect_half_size.1,
            expected_scrolled_y
        );
        let double_scrolled_y = original_y - 2.0 * scroll_y;
        assert!(
            ((rect_center.1 - rect_half_size.1) - double_scrolled_y).abs() > 1.0,
            "PM-004: SDF border top ({:.1}) must NOT be double-scrolled ({:.1})",
            rect_center.1 - rect_half_size.1,
            double_scrolled_y
        );
    }

    #[test]
    fn test_pm004_pipeline_parity_rect_vs_path() {
        let mut batcher = Batcher::new();
        batcher.begin_frame(SceneKey::BASE);

        let scroll_y: f32 = 60.0;
        let original_y: f32 = 180.0;
        batcher.push_scroll((0.0, scroll_y));

        // Flat rect via rect pipeline
        batcher.add_rect(RectF::new(10.0, original_y, 80.0, 30.0), Color::WHITE, 0);
        // Rounded rect via SDF pipeline
        batcher.add_rounded_rect(
            RectF::new(10.0, original_y, 80.0, 30.0),
            Color::WHITE,
            4.0,
            0,
            &mut MeshCache::new(),
            1.0,
            0,
        );

        // Extract flat rect Y from primitives
        let flat_rect_y = match &batcher.primitives()[0].data {
            PrimitiveData::Rect { rect, .. } => rect.y,
            other => panic!("Expected Rect primitive, got {:?}", other),
        };
        // Extract SDF rect top Y.
        let (_quad_rect, rect_center, rect_half_size, _radius) = first_sdf_rect_metrics(&batcher);
        let sdf_rect_y = rect_center.1 - rect_half_size.1;

        // Pipeline parity: both pipelines must produce the same scrolled Y
        assert!(
            (flat_rect_y - sdf_rect_y).abs() < 0.5,
            "PM-004 parity: flat rect Y ({:.1}) must equal SDF rect Y ({:.1})",
            flat_rect_y,
            sdf_rect_y
        );
    }
}
