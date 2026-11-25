mod forbidden;
mod guard;

use crate::compositor::{MultiSurfaceContract, SurfaceStackSnapshot};
use crate::core::math::{Point, RectF};
use crate::core::paint::{self, PaintSink};
use crate::core::scene_identity::{SceneNodeKey, SceneNodeKind};
use crate::renderer::CustomRenderPresentationId;
use crate::renderer::path::{FillRule, PathFillError};
use crate::utils::id::TreeWidgetId;
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use web_time::Instant;

// ============================================================================
// SPEC-HAL-044: SceneFrame - UI Thread to Platform Thread Transfer
// ============================================================================

/// SPEC-HAL-044: A complete frame produced by the UI thread, ready for rendering.
///
/// This struct is sent from the UI thread to the platform thread via a channel.
/// It contains only CPU-side data - no GPU handles or wgpu types.
///
/// SPEC-HAL-081A: SceneFrame with MANDATORY compositor surface stack.
#[derive(Debug)]
pub struct SceneFrame {
    /// When this frame was built (for latency tracking)
    pub build_time: Instant,
    /// Monotonic frame counter (for ordering and debugging)
    pub frame_id: u64,
    /// The scene graph to render
    pub scene: SceneGraph,
    /// SPEC-HAL-081B: Compositor surface stack (MANDATORY, MultiSurfaceContract).
    ///
    /// The renderer uses this for surface-level composition.
    /// Widgets push scene content; renderer consumes surfaces for ordering.
    /// This establishes the one-way data flow: Compositor → Renderer.
    ///
    /// This field is mandatory, not optional. The compositor path
    /// is the only valid path. Uses MultiSurfaceContract to support
    /// multi-window rendering.
    pub surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,
}

impl SceneFrame {
    /// Create a new scene frame with compositor surface stack.
    ///
    /// SPEC-HAL-081B: surface_stack is MANDATORY (MultiSurfaceContract).
    pub fn new(
        build_time: Instant,
        frame_id: u64,
        scene: SceneGraph,
        surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>,
    ) -> Self {
        Self {
            build_time,
            frame_id,
            scene,
            surface_stack,
        }
    }
}

// ============================================================================
// SPEC-HAL-045: Lock-Free Triple-Buffer Scene Swapchain
// ============================================================================

/// SPEC-HAL-045: Lock-free triple-buffer swapchain for scene frames.
///
/// Provides non-blocking frame handoff from UI thread to renderer:
/// - UI thread publishes frames via `publish()` into slot `(frame_id % 3)`
/// - Platform thread fetches via `latest()` from slot `(frame_id - 1) % 3`
/// - 3 slots ensure writer and reader never touch the same slot concurrently
///
/// Thread safety invariants:
/// - Single writer (UI thread) writes to `slots[frame_id % 3]`
/// - Single reader (render thread) reads from `slots[(frame_id - 1) % 3]`
/// - Writer and reader slots are always different for any given frame_id
/// - Atomic `latest_frame_id` with Release/Acquire ensures visibility
pub struct SceneSwapchain {
    /// 3 slots for triple-buffer; only UI thread writes, renderer only reads.
    slots: [UnsafeCell<Option<SceneFrame>>; 3],
    /// Whether at least one frame has been published.
    has_frame: AtomicBool,
    /// Latest published frame_id (monotonic from UI thread).
    latest_frame_id: AtomicU64,
}

// Safety: single writer (UI thread), single reader (render thread),
// and we never read and write the same slot concurrently because
// writer uses (frame_id % 3) and reader uses ((frame_id - 1) % 3).
unsafe impl Send for SceneSwapchain {}
unsafe impl Sync for SceneSwapchain {}

impl SceneSwapchain {
    /// Create a new triple-buffer swapchain.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: [
                UnsafeCell::new(None),
                UnsafeCell::new(None),
                UnsafeCell::new(None),
            ],
            has_frame: AtomicBool::new(false),
            latest_frame_id: AtomicU64::new(0),
        })
    }

    /// Publish a new frame from the UI thread.
    ///
    /// Lock-free - uses atomic store with Release ordering to ensure
    /// writes to the slot are visible before the reader sees the new frame_id.
    ///
    /// Slot selection uses `frame_id % 3` for deterministic ring behavior.
    pub fn publish(&self, frame: SceneFrame) {
        let id = frame.frame_id;
        let idx = (id % 3) as usize;

        // 1. Write frame into its slot (UI thread only writes).
        // Safety: Only UI thread writes to this slot, and reader uses a different slot.
        unsafe {
            *self.slots[idx].get() = Some(frame);
        }

        // 2. Publish frame_id with Release semantics so writes to the slot
        //    become visible before the reader sees the new id.
        self.latest_frame_id.store(id, Ordering::Release);
        self.has_frame.store(true, Ordering::Release);
    }

    /// Get the latest committed frame for rendering.
    ///
    /// Returns `Some(SceneFrameRef)` if a frame is available,
    /// `None` if no frame has been published yet.
    ///
    /// Lock-free - uses atomic load with Acquire ordering to synchronize
    /// with the writer's Release store.
    pub fn latest(&self) -> Option<SceneFrameRef<'_>> {
        // Nothing published yet
        if !self.has_frame.load(Ordering::Acquire) {
            return None;
        }

        // Acquire to synchronize with publish()'s Release store.
        let id = self.latest_frame_id.load(Ordering::Acquire);

        let idx = if id == 0 {
            // First frame: render directly from slot 0.
            // Writer has already stored into slot 0 before publishing id=0.
            0
        } else {
            // For all subsequent frames, read from previous frame's slot
            // to avoid touching the slot currently being written.
            let prev_id = id.wrapping_sub(1);
            (prev_id % 3) as usize
        };

        // Safety: Renderer only reads from this slot, and writer uses a different slot
        // (except for id=0 where writer has finished before setting has_frame).
        unsafe {
            let slot = &*self.slots[idx].get();
            if let Some(frame) = slot {
                Some(SceneFrameRef { frame })
            } else {
                None
            }
        }
    }
}

/// SPEC-HAL-045: Borrowed reference to a scene frame in the swapchain.
///
/// Allows the renderer to access the frame without copying or Arc overhead.
/// The lifetime is tied to the swapchain, ensuring the frame remains valid
/// for the duration of rendering.
pub struct SceneFrameRef<'a> {
    pub frame: &'a SceneFrame,
}

impl<'a> std::ops::Deref for SceneFrameRef<'a> {
    type Target = SceneFrame;

    fn deref(&self) -> &Self::Target {
        self.frame
    }
}

/// Per-corner radii for rounded rectangles
///
/// SPEC-R-020 Stage 3: Individual corner radii for precise rounded clipping.
/// All values in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedCorners {
    pub tl: f32, // top-left
    pub tr: f32, // top-right
    pub bl: f32, // bottom-left
    pub br: f32, // bottom-right
}

impl RoundedCorners {
    /// Create uniform rounded corners (all corners same radius)
    pub fn uniform(radius: f32) -> Self {
        Self {
            tl: radius,
            tr: radius,
            bl: radius,
            br: radius,
        }
    }

    /// Create rounded corners with individual radii
    pub fn new(tl: f32, tr: f32, bl: f32, br: f32) -> Self {
        Self { tl, tr, bl, br }
    }

    /// Check if all corners are zero (no rounding)
    pub fn is_zero(&self) -> bool {
        self.tl == 0.0 && self.tr == 0.0 && self.bl == 0.0 && self.br == 0.0
    }
}

// ============================================================================
// COLOR SPACE CONTRACT (BINDING LAW)
// ============================================================================
//
// 1. All CPU-side colors inside Halcyon are stored in LINEAR space.
// 2. All external colors (hex, CSS-style RGB) are interpreted as sRGB and
//    converted exactly once at input.
// 3. GPU output assumes linear input and relies on sRGB swapchain for final
//    conversion.
// 4. No mixed color spaces internally. Ever.
//
// ============================================================================

/// Convert a single channel from sRGB to linear space.
///
/// Uses the standard IEC 61966-2-1 transfer function.
#[inline]
pub fn srgb_to_linear(s: f32) -> f32 {
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert a single channel from linear to sRGB space.
///
/// For debugging/screenshots only. Not used in normal rendering.
#[inline]
pub fn linear_to_srgb(l: f32) -> f32 {
    if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Decode hex value to sRGB floats [0-1].
///
/// Returns [r, g, b] in sRGB space (NOT linear).
#[inline]
pub const fn decode_hex_srgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

/// Color in LINEAR RGBA format (0.0-1.0 range)
///
/// **COLOR SPACE CONTRACT**: All Color values are stored in LINEAR space.
/// Use `from_hex` or `from_srgb` for UI/theme colors (they convert from sRGB).
/// Use `from_linear` only for renderer-internal colors already in linear space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    // ========================================================================
    // PRIMARY CONSTRUCTORS (use these for UI/theme colors)
    // ========================================================================

    /// Create color from hex value (interpreted as sRGB, converted to linear).
    ///
    /// This is the primary constructor for UI colors. Hex values match what you
    /// see in browsers, Photoshop, and OS color pickers.
    ///
    /// # Example
    /// ```
    /// use halcyon::core::scene::Color;
    ///
    /// let red = Color::from_hex(0xFF0000);
    /// let gray = Color::from_hex(0x808080);
    /// ```
    #[inline]
    pub fn from_hex(hex: u32) -> Self {
        let [r, g, b] = decode_hex_srgb(hex);
        Self {
            r: srgb_to_linear(r),
            g: srgb_to_linear(g),
            b: srgb_to_linear(b),
            a: 1.0,
        }
    }

    /// Create color from sRGB components (converted to linear).
    ///
    /// Use this when you have sRGB float values (e.g., from CSS parsing).
    /// Alpha is NOT gamma-corrected (it's linear by definition).
    #[inline]
    pub fn from_srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: srgb_to_linear(r),
            g: srgb_to_linear(g),
            b: srgb_to_linear(b),
            a,
        }
    }

    // ========================================================================
    // LOW-LEVEL CONSTRUCTOR (renderer-internal use only)
    // ========================================================================

    /// Create color from LINEAR components (no conversion).
    ///
    /// **WARNING**: Only use this for colors already in linear space
    /// (e.g., renderer math, interpolation results, pre-converted values).
    /// For UI/theme colors, use `from_hex` or `from_srgb` instead.
    #[inline]
    pub const fn from_linear(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    // ========================================================================
    // CONSTANTS (defined in linear space)
    // ========================================================================

    /// Pure white in linear space
    pub const WHITE: Color = Color::from_linear(1.0, 1.0, 1.0, 1.0);

    /// Pure black in linear space
    pub const BLACK: Color = Color::from_linear(0.0, 0.0, 0.0, 1.0);

    /// Fully transparent (linear)
    pub const TRANSPARENT: Color = Color::from_linear(0.0, 0.0, 0.0, 0.0);

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Create a premultiplied-alpha color (for CPU-side math only).
    ///
    /// **WARNING**: Do NOT use this for colors passed to GPU render pipelines!
    /// The shader already premultiplies, so using this causes double-premultiplication
    /// (colors appear too dark/washed out).
    ///
    /// Input values should already be in LINEAR space.
    pub fn premul(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r * a,
            g: g * a,
            b: b * a,
            a,
        }
    }
}

// ============================================================================
// Conversions from paint:: types (PaintSink bridge)
// ============================================================================

impl From<paint::Color> for Color {
    fn from(c: paint::Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

impl From<paint::RoundedCorners> for RoundedCorners {
    fn from(rc: paint::RoundedCorners) -> Self {
        Self {
            tl: rc.tl,
            tr: rc.tr,
            bl: rc.bl,
            br: rc.br,
        }
    }
}

impl From<paint::TextBackgroundSpan> for TextBackgroundSpan {
    fn from(span: paint::TextBackgroundSpan) -> Self {
        Self {
            x_offset: span.x_offset,
            y_offset: span.y_offset,
            width: span.width,
            height: span.height,
            color: span.color.into(),
        }
    }
}

/// Z-order for scene nodes (higher = rendered later = on top)
pub type ZIndex = u32;

// ============================================================================
// INV-PAINT-LAYERS-FOCUS: Canonical Paint Layers
// ============================================================================
//
// Paint layers define semantic z-ordering for UI elements. Widgets MUST use
// these constants rather than hardcoded z-index values.
//
// Invariant:
// - Focus indicators MUST render ABOVE widget content
// - Focus indicators MUST render BELOW scrollbars, overlays, and system affordances
// - No widget may hardcode z-index values that escape its semantic layer
//
// Layer ordering (low to high):
//   CONTENT < FOCUS < SCROLLBAR < OVERLAY
// ============================================================================

/// Base layer for widget content (backgrounds, fills)
pub const LAYER_CONTENT: ZIndex = 2;

/// Layer for focus indicators (focus rings, outlines)
/// INV-PAINT-LAYERS-FOCUS: Above content, below scrollbars
pub const LAYER_FOCUS: ZIndex = 5;

/// Layer for scrollbar affordances (thumb, track)
/// INV-PAINT-LAYERS-FOCUS: Above focus indicators
pub const LAYER_SCROLLBAR: ZIndex = 7;

/// Base of the reserved system-overlay band (caret, tooltips, popups, dialogs).
/// Each overlay owner assigns explicit strict Z values to its text, fills, and chrome.
pub const LAYER_OVERLAY: ZIndex = 1000;

/// Scene node ID for scene graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneNodeId(pub usize);

/// SPEC-R-013-v2: Re-export ShadowMode from paint module
pub use crate::core::paint::ShadowMode;

/// SPEC-R-012-v2: Re-export gradient types from paint module
pub use crate::core::paint::{ColorStop, GradientDef, GradientKind};

/// SPEC-HAL-358: Re-export the checked luminous-contour paint model.
pub use crate::core::paint::{
    LuminousContourAppearance, LuminousContourError, LuminousContourFront, LuminousContourFronts,
};

/// Scene graph node types
#[derive(Debug, Clone)]
pub enum SceneNode {
    /// SPEC-HAL-354: Immutable engine-owned retained scene fragment.
    RetainedFragment(RetainedFragmentNode),
    /// Solid filled rectangle
    Rect(RectNode),
    /// Rectangle with border
    BorderRect(BorderRectNode),
    /// Rectangle with rounded corners
    RoundedRect(RoundedRectNode),
    /// Text run with positioned glyphs
    Text(TextNode),
    /// Textured image quad
    Image(ImageNode),
    /// Clip region for children
    Clip(ClipNode),
    /// Offscreen layer for compositing
    Layer(LayerNode),
    /// SPEC-HAL-086 Stage 7: Scroll transform for scroll containers
    /// Children are translated by -offset (scroll moves content up/left)
    Scroll(ScrollNode),
    // SPEC-R-003: GPU Path Rendering Primitives (Stage 1 scaffold)
    /// Circle shape
    Circle(CircleNode),
    /// Ellipse shape
    Ellipse(EllipseNode),
    /// Line segment
    Line(LineNode),
    /// Polyline (connected line segments)
    Polyline(PolylineNode),
    /// SPEC-HAL-358: Explicit luminous line/polyline contour.
    LuminousContour(LuminousContourNode),
    LocalAmbientGlow(LocalAmbientGlowNode),
    AnalyticSurface(AnalyticSurfaceNode),
    FrostedGlass(FrostedGlassNode),
    // SPEC-R-004: GPU Path Renderer (Stage 1 - CPU path layer)
    /// Arbitrary vector path (filled)
    Path(PathNode),
    // SPEC-HAL-086 Stage 8: Scrollbar overlay nodes for hit-testing
    /// Vertical scrollbar track (overlay, does not scroll with content)
    ScrollbarV(ScrollbarVNode),
    /// Horizontal scrollbar track (overlay, does not scroll with content)
    ScrollbarH(ScrollbarHNode),
    // SPEC-R-013-v2: Analytical shadow pipeline
    /// Drop shadow, inner shadow, or focus ring
    Shadow(ShadowNode),
    // SPEC-R-012-v2: Gradient pipeline
    /// Gradient-filled rectangle (linear or radial)
    GradientRect(GradientRectNode),
    // SPEC-HAL-272: Color picker SV area
    /// Color picker saturation/value area (custom shader)
    ColorPickerSv(ColorPickerSvNode),
    // SPEC-HAL-278: Radial gauge
    /// Circular arc/ring gauge (custom shader)
    RadialGauge(RadialGaugeNode),
}

// ============================================================================
// SPEC-HAL-083 Phase-14: SceneNode Accessor Methods
// ============================================================================

impl SceneNode {
    /// Get the key of this node, if any.
    ///
    /// Returns `None` for ephemeral nodes (no identity).
    pub fn key(&self) -> Option<SceneNodeKey> {
        match self {
            SceneNode::RetainedFragment(_) => None,
            SceneNode::Rect(n) => n.key,
            SceneNode::BorderRect(n) => n.key,
            SceneNode::RoundedRect(n) => n.key,
            SceneNode::Text(n) => n.key,
            SceneNode::Image(n) => n.key,
            SceneNode::Clip(n) => n.key,
            SceneNode::Layer(n) => n.key,
            SceneNode::Scroll(n) => n.key,
            SceneNode::Circle(n) => n.key,
            SceneNode::Ellipse(n) => n.key,
            SceneNode::Line(n) => n.key,
            SceneNode::Polyline(n) => n.key,
            SceneNode::LuminousContour(n) => n.key,
            SceneNode::LocalAmbientGlow(n) => n.key,
            SceneNode::AnalyticSurface(n) => n.key,
            SceneNode::FrostedGlass(n) => n.key,
            SceneNode::Path(n) => n.key,
            SceneNode::ScrollbarV(n) => n.key,
            SceneNode::ScrollbarH(n) => n.key,
            SceneNode::Shadow(n) => n.key,
            SceneNode::GradientRect(n) => n.key,
            SceneNode::ColorPickerSv(n) => n.key,
            SceneNode::RadialGauge(n) => n.key,
        }
    }

    /// Get the kind (type tag) of this node.
    ///
    /// Used for cache-readiness validation to ensure node types
    /// remain stable across frames for stable keys.
    pub fn kind(&self) -> SceneNodeKind {
        match self {
            SceneNode::RetainedFragment(_) => SceneNodeKind::RetainedFragment,
            SceneNode::Rect(_) => SceneNodeKind::Rect,
            SceneNode::BorderRect(_) => SceneNodeKind::BorderRect,
            SceneNode::RoundedRect(_) => SceneNodeKind::RoundedRect,
            SceneNode::Text(_) => SceneNodeKind::Text,
            SceneNode::Image(_) => SceneNodeKind::Image,
            SceneNode::Clip(_) => SceneNodeKind::Clip,
            SceneNode::Layer(_) => SceneNodeKind::Layer,
            SceneNode::Scroll(_) => SceneNodeKind::Scroll,
            SceneNode::Circle(_) => SceneNodeKind::Circle,
            SceneNode::Ellipse(_) => SceneNodeKind::Ellipse,
            SceneNode::Line(_) => SceneNodeKind::Line,
            SceneNode::Polyline(_) => SceneNodeKind::Polyline,
            SceneNode::LuminousContour(_) => SceneNodeKind::LuminousContour,
            SceneNode::LocalAmbientGlow(_) => SceneNodeKind::LocalAmbientGlow,
            SceneNode::AnalyticSurface(_) => SceneNodeKind::AnalyticSurface,
            SceneNode::FrostedGlass(_) => SceneNodeKind::FrostedGlass,
            SceneNode::Path(_) => SceneNodeKind::Path,
            SceneNode::ScrollbarV(_) => SceneNodeKind::ScrollbarV,
            SceneNode::ScrollbarH(_) => SceneNodeKind::ScrollbarH,
            SceneNode::Shadow(_) => SceneNodeKind::Shadow,
            SceneNode::GradientRect(_) => SceneNodeKind::GradientRect,
            SceneNode::ColorPickerSv(_) => SceneNodeKind::ColorPickerSv,
            SceneNode::RadialGauge(_) => SceneNodeKind::RadialGauge,
        }
    }
}

/// SPEC-HAL-354: Immutable scene nodes retained across presentation frames.
#[derive(Debug, Clone)]
pub struct RetainedFragmentNode {
    pub children: Arc<[SceneNode]>,
    /// Optional engine composition wrapper; child primitives and keys are unchanged.
    pub control_subtree_member: Option<paint::ControlSubtreeMember>,
}

/// Solid filled rectangle.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct RectNode {
    pub rect: RectF,
    pub color: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Rectangle with border stroke and optional rounded corners.
///
/// **INV-BORDER-GEOMETRY-01:** Border visuals must conform exactly to the
/// geometry of the shape they decorate. When `radius > 0`, the border is
/// rendered as a rounded ring (outer rounded rect - inner rounded rect).
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct BorderRectNode {
    pub rect: RectF,
    pub fill_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    /// Corner radius for rounded borders. Matches RoundedRectNode semantics.
    /// radius = 0 produces sharp corners (rectangular border).
    pub radius: f32,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Rectangle with rounded corners.
///
/// **Semantic Contract (Shape-Owned Silhouette):**
/// RoundedRect defines the **silhouette** (shape boundary) via real geometry.
/// The silhouette is NOT clip-defined - it uses PathBuilder::rounded_rect
/// for true curved corners (4 quadratic arcs + 4 lines).
///
/// The silhouette must be invariant across visual states (hover, pressed, focus).
/// Overlay layers must NOT redefine the shape - only fill color changes.
///
/// **Implementation:**
/// Routed through the path pipeline (SPEC-R-004) for tessellated rendering.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct RoundedRectNode {
    pub rect: RectF,
    pub color: Color,
    /// Corner radius defining the silhouette. Used by path tessellation.
    pub radius: f32,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Background span for text-native selection/caret rendering.
///
/// SPEC-EXIT-A1: Background quads rendered as part of the text pass,
/// guaranteeing correct ordering (backgrounds before glyphs) without
/// relying on z-index or submission order.
#[derive(Debug, Clone)]
pub struct TextBackgroundSpan {
    /// X offset from text position (logical pixels)
    pub x_offset: f32,
    /// Y offset from text position (logical pixels)
    pub y_offset: f32,
    /// Width of background span (logical pixels)
    pub width: f32,
    /// Height of background span (logical pixels)
    pub height: f32,
    /// Background color
    pub color: Color,
}

/// Logical text input. Compile owns the ephemeral shaped glyphs.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct TextNode {
    pub binding: Option<crate::core::text_engine::TextBinding>,
    pub position: Point,
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub z_index: ZIndex,
    /// SPEC-EXIT-A1: Background spans rendered before glyphs in text pass.
    /// Used for selection highlighting and caret rendering.
    pub backgrounds: Vec<TextBackgroundSpan>,
    pub key: Option<SceneNodeKey>,
}

// ============================================================================
// SPEC-HAL-029: Text Quad Types for Atlas-Based Rendering
// ============================================================================

/// A single text quad referencing the glyph atlas.
///
/// This is the GPU-level primitive for text rendering. Each glyph
/// becomes one TextQuad with UV coordinates into the glyph atlas.
///
/// SPEC-R-020: Added clip_rect for unified scissor-based clipping.
/// R-010B FIX: Added downsample_u for per-glyph downsample offset.
#[derive(Debug, Clone, Copy)]
pub struct TextQuad {
    /// Position rectangle in logical pixels
    pub pos: RectF,
    /// UV rectangle in atlas space (0..1)
    pub uv: RectF,
    /// Text color
    pub color: Color,
    /// Effective clip rectangle in logical pixels (None = full viewport)
    ///
    /// SPEC-R-020: Text quads now carry their clip rect for consistent
    /// scissor-based clipping with other primitives.
    pub clip_rect: Option<RectF>,
    /// R-010B FIX: Per-glyph downsample offset in UV space
    pub downsample_u: f32,
}

/// A batch of text quads sharing the same font size/atlas.
///
/// Used internally by the renderer to minimize draw calls.
#[derive(Debug, Clone)]
pub struct TextQuadBatch {
    /// Font size (determines which atlas to use)
    pub font_size: f32,
    /// Scale factor for physical pixel rendering
    pub scale_factor: f32,
    /// All quads in this batch
    pub quads: Vec<TextQuad>,
}

/// Textured image quad.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTextureSource {
    Registered(u64),
    Admitted(crate::renderer::image_binding::ImageBindingIdentity),
    TargetBound(CustomRenderPresentationId),
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    pub rect: RectF,
    pub texture_source: ImageTextureSource,
    pub uv_rect: RectF,
    pub tint: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Clip region - children are clipped to this rect.
///
/// SPEC-R-020 Stage 3: Added optional rounded corners for mask-based clipping.
/// - If rounded is None: use fast scissor-only path
/// - If rounded is Some: use GPU mask shader (with scissor as coarse clip)
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ClipNode {
    pub rect: RectF,
    pub rounded: Option<RoundedCorners>,
    pub children: Vec<SceneNode>,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Offscreen layer for effects (shadows, blur, etc.).
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct LayerNode {
    pub rect: RectF,
    pub children: Vec<SceneNode>,
    pub opacity: f32,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// SPEC-HAL-086 Stage 7: Scroll transform node.
///
/// Wraps children of a scroll container and applies a translation offset.
/// Children are translated by (-offset.0, -offset.1) during rendering.
/// This is the canonical location for scroll transforms in the scene graph.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ScrollNode {
    /// Scroll offset (content is shifted by -offset)
    pub offset: (f32, f32),
    /// Children to be rendered with scroll offset applied
    pub children: Vec<SceneNode>,
    pub key: Option<SceneNodeKey>,
}

impl ScrollNode {
    /// Create a new scroll node with the given offset
    pub fn new(offset: (f32, f32)) -> Self {
        Self {
            offset,
            children: Vec::new(),
            key: None,
        }
    }
}

// ============================================================================
// SPEC-HAL-086 Stage 8: Scrollbar Overlay Nodes
// ============================================================================

/// Vertical scrollbar overlay node for hit-testing.
///
/// This node represents the scrollbar track region in the scene graph.
/// It does NOT scroll with content - it's rendered in viewport space.
/// Used by hit-test to make ScrollArea the target when clicking scrollbars.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ScrollbarVNode {
    /// Absolute rect of the scrollbar track (hit-test region)
    pub track_abs_rect: RectF,
    /// Absolute rect of the owning ScrollArea (for local coords)
    pub owner_abs_rect: RectF,
    /// Widget ID of the owning ScrollArea
    pub widget_id: TreeWidgetId,
    pub key: Option<SceneNodeKey>,
}

/// Horizontal scrollbar overlay node for hit-testing.
///
/// This node represents the scrollbar track region in the scene graph.
/// It does NOT scroll with content - it's rendered in viewport space.
/// Used by hit-test to make ScrollArea the target when clicking scrollbars.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ScrollbarHNode {
    /// Absolute rect of the scrollbar track (hit-test region)
    pub track_abs_rect: RectF,
    /// Absolute rect of the owning ScrollArea (for local coords)
    pub owner_abs_rect: RectF,
    /// Widget ID of the owning ScrollArea
    pub widget_id: TreeWidgetId,
    pub key: Option<SceneNodeKey>,
}

/// SPEC-R-013-v2: Analytical shadow effect.
///
/// Single-pass fragment shader with rounded-rect SDF and Gaussian falloff.
/// Supports drop shadows, inner shadows, and focus rings via `mode`.
///
/// `key == None` => ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ShadowNode {
    /// Bounding rect of the casting shape (logical coordinates)
    pub shape_rect: RectF,
    /// Corner radius of the casting shape (0.0 for sharp corners)
    pub shape_radius: f32,
    /// Shadow mode: drop, inner, or ring
    pub mode: ShadowMode,
    /// Horizontal and vertical offset from shape center (logical pixels)
    pub offset: (f32, f32),
    /// Spread: positive expands shadow shape, negative contracts (logical pixels)
    pub spread: f32,
    /// Blur radius: Gaussian sigma controlling softness (logical pixels)
    pub blur_radius: f32,
    /// Shadow color (typically semi-transparent)
    pub color: Color,
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}

/// SPEC-R-012-v2: Gradient-filled rectangle.
///
/// Linear or radial gradient with up to 8 color stops.
/// Optional rounded corners via `radius`.
///
/// `key == None` => ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct GradientRectNode {
    /// Rectangle bounds (logical coordinates)
    pub rect: RectF,
    /// Corner radius (0.0 for sharp corners)
    pub radius: f32,
    /// Gradient definition
    pub gradient: GradientDef,
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}

/// SPEC-HAL-272: Color picker saturation/value area node.
///
/// Rendered by a custom GPU shader that computes HSV → RGB per-pixel.
///
/// `key == None` => ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct ColorPickerSvNode {
    /// Rectangle bounds (logical coordinates)
    pub rect: RectF,
    /// Current hue in [0.0, 1.0]
    pub hue: f32,
    /// Corner radius (0.0 for sharp corners)
    pub radius: f32,
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}

/// SPEC-HAL-278: Radial gauge node — circular arc/ring display.
///
/// Rendered by a custom GPU shader. Single quad, arc computed in fragment.
///
/// `key == None` => ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct RadialGaugeNode {
    /// Bounding square (logical coordinates)
    pub rect: RectF,
    /// Fill extent [0.0, 1.0] (0 = empty, 1 = full circle)
    pub fill: f32,
    /// Ring thickness, normalized (e.g. 0.15)
    pub thickness: f32,
    /// Track (unfilled) color as [r, g, b, a] in linear space
    pub track_color: [f32; 4],
    /// Fill color as [r, g, b, a] in linear space
    pub fill_color: [f32; 4],
    /// Fill color for negative direction (value < origin).
    /// Defaults to fill_color when no split is desired.
    pub fill_color_negative: [f32; 4],
    /// Arc start angle in normalized [0, 1] (0 = top, clockwise).
    pub start_angle: f32,
    /// Arc sweep in normalized [0, 1] (1 = full circle).
    pub sweep_angle: f32,
    /// Origin fill position in normalized [0, 1] within the arc.
    /// Fill extends FROM origin TOWARD fill value.
    pub origin_fill: f32,
    /// Rendering mode: 0 = fill arc, 1 = needle wedge.
    pub mode: u32,
    /// Fill style: 0 = solid, 1 = zone-colored, 2 = value-lerp.
    pub fill_style: u32,
    /// Zone count (0-4).
    pub zone_count: u32,
    /// Zone data: (start_t, end_t, color) per zone.
    /// Positions are normalized [0, 1] within arc sweep.
    pub zones: [(f32, f32, [f32; 4]); 4],
    /// Number of major tick intervals (0 = no ticks).
    pub major_divisions: u32,
    /// Minor ticks between each major.
    pub minor_per_major: u32,
    /// Major tick length as fraction of outer radius.
    pub tick_length_major: f32,
    /// Minor tick length as fraction of outer radius.
    pub tick_length_minor: f32,
    /// Tick line color [r, g, b, a].
    pub tick_color: [f32; 4],
    /// Z-index for draw ordering
    pub z_index: ZIndex,
    /// Optional scene node key for diffing
    pub key: Option<SceneNodeKey>,
}

// ============================================================================
// SPEC-R-003: GPU Path Rendering Primitives (Stage 1 - Non-rendering scaffold)
// ============================================================================

/// Circle shape primitive.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct CircleNode {
    pub center: Point,
    pub radius: f32,
    pub color: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

impl CircleNode {
    /// Create a new circle node
    pub fn new(center: Point, radius: f32, color: Color) -> Self {
        Self {
            center,
            radius,
            color,
            z_index: 0,
            key: None,
        }
    }

    /// Set the z-index for rendering order
    pub fn with_z_index(mut self, z_index: ZIndex) -> Self {
        self.z_index = z_index;
        self
    }
}

/// Ellipse shape primitive.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct EllipseNode {
    pub center: Point,
    pub radii: Point, // x = horizontal radius, y = vertical radius
    pub color: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

impl EllipseNode {
    /// Create a new ellipse node
    pub fn new(center: Point, radii: Point, color: Color) -> Self {
        Self {
            center,
            radii,
            color,
            z_index: 0,
            key: None,
        }
    }

    /// Set the z-index for rendering order
    pub fn with_z_index(mut self, z_index: ZIndex) -> Self {
        self.z_index = z_index;
        self
    }
}

/// Line segment primitive.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct LineNode {
    pub a: Point,
    pub b: Point,
    pub width: f32,
    pub color: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

impl LineNode {
    /// Create a new line segment node
    pub fn new(a: Point, b: Point, width: f32, color: Color) -> Self {
        Self {
            a,
            b,
            width,
            color,
            z_index: 0,
            key: None,
        }
    }

    /// Set the z-index for rendering order
    pub fn with_z_index(mut self, z_index: ZIndex) -> Self {
        self.z_index = z_index;
        self
    }
}

/// Polyline (connected line segments) primitive.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct PolylineNode {
    pub points: Vec<Point>,
    pub width: f32,
    pub color: Color,
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

impl PolylineNode {
    /// Create a new polyline node
    pub fn new(points: Vec<Point>, width: f32, color: Color) -> Self {
        Self {
            points,
            width,
            color,
            z_index: 0,
            key: None,
        }
    }

    /// Set the z-index for rendering order
    pub fn with_z_index(mut self, z_index: ZIndex) -> Self {
        self.z_index = z_index;
        self
    }
}

/// HAL385: pure current-prefix composition marker, without a captured source.
#[derive(Debug, Clone)]
pub struct FrostedGlassNode {
    rect: RectF,
    corners: paint::RoundedCorners,
    appearance: paint::FrostedGlassAppearance,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
}
impl FrostedGlassNode {
    pub const fn rect(&self)->RectF {self.rect}
    pub const fn corners(&self)->paint::RoundedCorners {self.corners}
    pub fn appearance(&self)->&paint::FrostedGlassAppearance {&self.appearance}
    pub const fn z_index(&self)->ZIndex {self.z_index}
    pub const fn key(&self)->Option<SceneNodeKey> {self.key}
}

/// HAL386/389: checked material and component-owned geometry, without GPU resources.
#[derive(Debug, Clone)]
pub struct AnalyticSurfaceNode {
    rect: RectF,
    corners: paint::RoundedCorners,
    material: paint::SurfaceAnalytic,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
}
impl AnalyticSurfaceNode {
    pub const fn rect(&self) -> RectF {
        self.rect
    }
    pub const fn corners(&self) -> paint::RoundedCorners {
        self.corners
    }
    pub fn material(&self) -> &paint::SurfaceAnalytic {
        &self.material
    }
    pub const fn z_index(&self) -> ZIndex {
        self.z_index
    }
    pub const fn key(&self) -> Option<SceneNodeKey> {
        self.key
    }
}

/// HAL-388: checked immutable local ambient glow render data.
#[derive(Debug, Clone)]
pub struct LocalAmbientGlowNode {
    rect: RectF,
    bounds: RectF,
    appearance: paint::LocalAmbientGlowAppearance,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
}
impl LocalAmbientGlowNode {
    pub const fn rect(&self) -> RectF {
        self.rect
    }
    pub const fn bounds(&self) -> RectF {
        self.bounds
    }
    pub const fn appearance(&self) -> paint::LocalAmbientGlowAppearance {
        self.appearance
    }
    pub const fn z_index(&self) -> ZIndex {
        self.z_index
    }
    pub const fn key(&self) -> Option<SceneNodeKey> {
        self.key
    }
}

/// SPEC-HAL-358: Checked immutable luminous-contour render data.
#[derive(Debug, Clone)]
pub struct LuminousContourNode {
    points: Arc<[Point]>,
    core_width: f32,
    color: Color,
    appearance: LuminousContourAppearance,
    fronts: LuminousContourFronts,
    total_length: f32,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
}

impl LuminousContourNode {
    pub fn points(&self) -> &Arc<[Point]> {
        &self.points
    }

    pub const fn core_width(&self) -> f32 {
        self.core_width
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    pub const fn appearance(&self) -> LuminousContourAppearance {
        self.appearance
    }

    pub const fn fronts(&self) -> LuminousContourFronts {
        self.fronts
    }

    pub const fn total_length(&self) -> f32 {
        self.total_length
    }

    pub const fn z_index(&self) -> ZIndex {
        self.z_index
    }

    pub const fn key(&self) -> Option<SceneNodeKey> {
        self.key
    }
}

// ============================================================================
// SPEC-R-004: GPU Path Renderer (Stage 1 - CPU path layer)
// ============================================================================

/// Arbitrary vector path primitive (Stage 1: data container only).
/// Stage 15: Added color field for fill color.
/// Stage 16: Added is_stroke flag for stroke/fill routing.
/// Stage 17: Added debug field for visualization overlays.
/// Stage 18: Added z_index for deterministic ordering.
///
/// Note: `is_stroke` is a routing hint only. Actual stroke rendering
/// (width, joins, caps, etc.) is handled by SPEC-R-006/R-016.
///
/// `key == None` ⇒ ephemeral node, recreated every frame.
#[derive(Debug, Clone)]
pub struct PathNode {
    pub path: crate::renderer::path::Path,
    pub mesh: Option<crate::renderer::path::PathMesh>,
    pub color: Color,
    /// Fill rule for uncached source geometry (SPEC-R-004 §13).
    pub fill_rule: FillRule,
    /// Stage 16: Stroke intent flag. `false` = fill, `true` = stroke.
    /// This is metadata only; visual behavior is identical until R-006/R-016.
    pub is_stroke: bool,
    /// Stage 17: Debug visualization mode. `None` = no debug overlay.
    pub debug: Option<crate::renderer::path::PathDebugMode>,
    /// Stage 18: Z-index for deterministic draw ordering.
    pub z_index: ZIndex,
    pub key: Option<SceneNodeKey>,
}

/// Scene graph - tree of drawable primitives
#[derive(Debug, Clone)]
pub struct SceneGraph {
    pub root: Vec<SceneNode>,
    /// Background clear color for the frame (set from theme.colors.surfaces.base0)
    pub clear_color: Color,
    /// SPEC-HAL-083 Phase-12: Key-to-widget mapping for stability assertions
    #[cfg(debug_assertions)]
    key_widget_map: std::collections::HashMap<SceneNodeKey, u64>,
}

#[derive(Debug, Default)]
pub(crate) struct RecycledSceneStorage {
    root: Vec<SceneNode>,
    nested: Vec<Vec<SceneNode>>,
    #[cfg(debug_assertions)]
    key_widget_map: std::collections::HashMap<SceneNodeKey, u64>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            root: Vec::new(),
            clear_color: Color::BLACK,
            #[cfg(debug_assertions)]
            key_widget_map: std::collections::HashMap::new(),
        }
    }

    /// SPEC-HAL-083 Phase-12: Get the key-to-widget mapping for stability assertions.
    #[cfg(debug_assertions)]
    pub fn key_widget_map(&self) -> &std::collections::HashMap<SceneNodeKey, u64> {
        &self.key_widget_map
    }

    pub fn clear(&mut self) {
        self.root.clear();
    }

    pub(crate) fn into_recycled_storage(mut self) -> RecycledSceneStorage {
        let mut root = std::mem::take(&mut self.root);
        let mut nested = Vec::new();
        SceneBuilder::collect_nested_node_vectors(&mut root, &mut nested);
        nested.reverse();
        root.clear();

        #[cfg(debug_assertions)]
        let key_widget_map = {
            let mut map = std::mem::take(&mut self.key_widget_map);
            map.clear();
            map
        };

        RecycledSceneStorage {
            root,
            nested,
            #[cfg(debug_assertions)]
            key_widget_map,
        }
    }

    /// SPEC-HAL-083 Phase-14: Find a node by its key.
    ///
    /// Returns `Some(&SceneNode)` if a node with the given key exists in this scene.
    /// Returns `None` if no node has this key.
    ///
    /// This is O(n) but acceptable for debug validation and cache dispatch.
    pub fn find_node_by_key(&self, key: SceneNodeKey) -> Option<&SceneNode> {
        fn find_in_nodes(nodes: &[SceneNode], key: SceneNodeKey) -> Option<&SceneNode> {
            for node in nodes {
                if node.key() == Some(key) {
                    return Some(node);
                }

                let nested = match node {
                    SceneNode::RetainedFragment(n) => find_in_nodes(&n.children, key),
                    SceneNode::Clip(n) => find_in_nodes(&n.children, key),
                    SceneNode::Layer(n) => find_in_nodes(&n.children, key),
                    SceneNode::Scroll(n) => find_in_nodes(&n.children, key),
                    _ => None,
                };

                if nested.is_some() {
                    return nested;
                }
            }

            None
        }

        find_in_nodes(&self.root, key)
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-R-001: SceneBuilder with Clip Stack Enforcement
// ============================================================================

/// Maximum clip stack depth for SceneBuilder (defensive limit).
#[cfg(debug_assertions)]
const MAX_SCENE_BUILDER_CLIP_DEPTH: usize = 64;

/// Builder for constructing scene graphs
///
/// SPEC-R-001: The SceneBuilder enforces proper clip region nesting through
/// begin_clip/end_clip pairs. All clips must wrap children - "floating" clips
/// without children are structurally prevented by the build() validation.
///
/// SPEC-HAL-086 Stage 7: Also supports begin_scroll/end_scroll for scroll transforms.
///
/// # Identity Contract (FROZEN - SPEC-HAL-083 Phase-2)
///
/// **This identity API is structurally frozen. Any extension requires explicit SPEC-HAL-083 Phase-3.**
///
/// - SceneBuilder NEVER invents identity. All `key` values come from widgets.
/// - `push_*` methods pass through the key parameter unchanged.
/// - Duplicate keys within a frame panic in debug builds.
/// - `key == None` means the node is ephemeral (recreated every frame).
pub struct SceneBuilder {
    nodes: Vec<SceneNode>,
    admitted_image_dependencies: Option<Box<Vec<crate::renderer::image_binding::ImageBinding>>>,
    recycled_node_vectors: Vec<Vec<SceneNode>>,
    /// SPEC-HAL-266: Device pixel ratio for DPR-aware snapping. Immutable after construction.
    scale_factor: f32,
    /// SPEC-R-020 Stage 5: Stack stores (rect, rounded, start_index) for clip regions
    clip_stack: Vec<(RectF, Option<RoundedCorners>, usize)>,
    /// SPEC-HAL-086 Stage 7: Stack stores (offset, start_index) for scroll transforms
    scroll_stack: Vec<((f32, f32), usize)>,
    control_subtree_stack: Vec<(paint::ControlSubtreeMember, usize)>,
    /// Background clear color for the frame
    clear_color: Color,
    /// DEBUG: Counter for push_rect calls this frame
    #[cfg(debug_assertions)]
    pub debug_rect_count: u32,
    /// DEBUG: Counter for push_rounded_rect calls this frame
    #[cfg(debug_assertions)]
    pub debug_rounded_rect_count: u32,
    /// DEBUG: Callsite info for rounded rects (file:line, rect, radius)
    #[cfg(debug_assertions)]
    pub debug_rounded_rect_callsites: Vec<(&'static str, u32, RectF, f32)>,
    /// SPEC-HAL-083: Track emitted keys for uniqueness assertion (debug only)
    #[cfg(debug_assertions)]
    emitted_keys: std::collections::HashSet<SceneNodeKey>,
    /// SPEC-HAL-083 Phase-9: Current widget being painted (for multi-key detection)
    #[cfg(debug_assertions)]
    current_painting_widget_id: Option<u64>,
    /// SPEC-HAL-083 Phase-9: Count of keyed primitives emitted per widget this frame
    #[cfg(debug_assertions)]
    widget_keyed_emissions: std::collections::HashMap<u64, usize>,
    /// SPEC-HAL-083 Phase-12: Map each emitted key to its emitting widget
    #[cfg(debug_assertions)]
    key_to_widget: std::collections::HashMap<SceneNodeKey, u64>,
    #[cfg(test)]
    node_vector_growths: usize,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self::with_scale_factor(1.0)
    }

    /// SPEC-HAL-266: Construct with explicit device pixel ratio.
    /// INV-DPR-002: scale_factor must be positive and finite.
    pub fn with_scale_factor(scale_factor: f32) -> Self {
        debug_assert!(
            scale_factor > 0.0 && scale_factor.is_finite(),
            "SPEC-HAL-266 INV-DPR-002: scale_factor must be positive and finite, got {}",
            scale_factor
        );
        Self {
            nodes: Vec::new(),
            admitted_image_dependencies: None,
            recycled_node_vectors: Vec::new(),
            scale_factor,
            clip_stack: Vec::new(),
            scroll_stack: Vec::new(),
            control_subtree_stack: Vec::new(),
            clear_color: Color::BLACK,
            #[cfg(debug_assertions)]
            debug_rect_count: 0,
            #[cfg(debug_assertions)]
            debug_rounded_rect_count: 0,
            #[cfg(debug_assertions)]
            debug_rounded_rect_callsites: Vec::new(),
            #[cfg(debug_assertions)]
            emitted_keys: std::collections::HashSet::new(),
            #[cfg(debug_assertions)]
            current_painting_widget_id: None,
            #[cfg(debug_assertions)]
            widget_keyed_emissions: std::collections::HashMap::new(),
            #[cfg(debug_assertions)]
            key_to_widget: std::collections::HashMap::new(),
            #[cfg(test)]
            node_vector_growths: 0,
        }
    }

    /// Rebuild a scene while retaining the immediately preceding scene's node-vector storage.
    ///
    /// The logical scene is still recreated each frame. Only the root and nested child-vector
    /// allocations are recycled after the preceding draw plan has consumed the scene.
    pub(crate) fn with_recycled_storage(scale_factor: f32, storage: RecycledSceneStorage) -> Self {
        let mut builder = Self::with_scale_factor(scale_factor);
        builder.nodes = storage.root;
        builder.recycled_node_vectors = storage.nested;
        #[cfg(debug_assertions)]
        {
            builder.key_to_widget = storage.key_widget_map;
        }
        builder
    }

    fn collect_nested_node_vectors(
        nodes: &mut [SceneNode],
        recycled_node_vectors: &mut Vec<Vec<SceneNode>>,
    ) {
        for node in nodes {
            let children = match node {
                SceneNode::Clip(node) => Some(&mut node.children),
                SceneNode::Layer(node) => Some(&mut node.children),
                SceneNode::Scroll(node) => Some(&mut node.children),
                _ => None,
            };
            if let Some(children) = children {
                Self::collect_nested_node_vectors(children, recycled_node_vectors);
                let mut children = std::mem::take(children);
                children.clear();
                recycled_node_vectors.push(children);
            }
        }
    }

    fn take_node_vector(&mut self, required_capacity: usize) -> Vec<SceneNode> {
        let mut nodes = self.recycled_node_vectors.pop().unwrap_or_default();
        nodes.clear();
        if nodes.capacity() < required_capacity {
            #[cfg(test)]
            {
                self.node_vector_growths += 1;
            }
            nodes.reserve(required_capacity);
        }
        nodes
    }

    /// SPEC-HAL-354: Append an immutable retained fragment without cloning its nodes.
    pub(crate) fn push_retained_fragment(&mut self, children: Arc<[SceneNode]>) {
        self.push_control_subtree_fragment(children, None);
    }

    pub(crate) fn push_control_subtree_fragment(
        &mut self,
        children: Arc<[SceneNode]>,
        control_subtree_member: Option<paint::ControlSubtreeMember>,
    ) {
        if !children.is_empty() {
            self.nodes
                .push(SceneNode::RetainedFragment(RetainedFragmentNode {
                    children,
                    control_subtree_member,
                }));
        }
    }

    /// SPEC-HAL-354: Restore debug identity testimony owned by a retained fragment.
    #[cfg(debug_assertions)]
    pub(crate) fn extend_retained_key_widget_map(
        &mut self,
        retained: &std::collections::HashMap<SceneNodeKey, u64>,
    ) {
        for (key, widget_id) in retained {
            let previous = self.key_to_widget.insert(*key, *widget_id);
            debug_assert!(
                previous.is_none(),
                "SPEC-HAL-083: retained SceneNodeKey {:?} was emitted more than once",
                key
            );
        }
    }

    /// SPEC-HAL-266: Device pixel ratio for the current render target.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    // ========================================================================
    // SPEC-HAL-083 Phase-2/9: Identity Enforcement (debug-only)
    //
    // INVARIANT: SceneBuilder NEVER invents identity.
    // All `key` values come from push_* method parameters (widget-provided).
    // This is enforced by API design: push_* takes `key: Option<SceneNodeKey>`.
    //
    // Phase-2: Uniqueness checking (duplicate keys panic).
    // Phase-9: Per-widget emission counting (multiple keys per widget panic).
    // ========================================================================

    /// SPEC-HAL-083 Phase-9: Set the current widget being painted.
    ///
    /// Call this before painting each widget to enable per-widget key emission tracking.
    /// Debug builds will panic if a widget emits more than one keyed primitive.
    #[cfg(debug_assertions)]
    pub fn set_current_painting_widget(&mut self, widget_id: u64) {
        self.current_painting_widget_id = Some(widget_id);
    }

    /// SPEC-HAL-083 Phase-9: Clear the current painting widget.
    ///
    /// Call this after painting a widget to reset the tracking state.
    #[cfg(debug_assertions)]
    pub fn clear_current_painting_widget(&mut self) {
        self.current_painting_widget_id = None;
    }

    /// SPEC-HAL-083 Phase-12: Get the key-to-widget mapping for stability assertions.
    ///
    /// Returns a map from each emitted SceneNodeKey to the widget_id that emitted it.
    /// Used by stability assertions to verify that stable keys maintain their widget affinity.
    #[cfg(debug_assertions)]
    pub fn key_widget_map(&self) -> &std::collections::HashMap<SceneNodeKey, u64> {
        &self.key_to_widget
    }

    /// SPEC-HAL-083: Assert that a scene node key is unique within this frame.
    ///
    /// Panics in debug builds if:
    /// - The same key is emitted twice (Phase-2 uniqueness)
    /// - A widget emits more than one keyed primitive (Phase-9 per-widget limit)
    ///
    /// Note: `key == None` is always allowed (ephemeral nodes).
    #[cfg(debug_assertions)]
    fn assert_key_unique(&mut self, key: Option<&SceneNodeKey>) {
        // INV: SceneBuilder never invents keys. All keys come from parameters.
        // The push_* API signature guarantees this - we only pass through.
        if let Some(k) = key {
            // Phase-2: Check global uniqueness
            assert!(
                self.emitted_keys.insert(*k),
                "SPEC-HAL-083: Duplicate SceneNodeKey in same frame: {:?}. \
                 Each keyed node must have a unique key within a frame.",
                k
            );

            // Phase-9: Check per-widget emission count
            if let Some(widget_id) = self.current_painting_widget_id {
                let count = self.widget_keyed_emissions.entry(widget_id).or_insert(0);
                *count += 1;
                assert!(
                    *count <= 1,
                    "SPEC-HAL-083 Phase-9: Widget emitted multiple SceneNodeKeys in a single frame \
                     (widget_id={}, emission_count={}). \
                     A widget may emit many primitives, but at most one may carry a SceneNodeKey.",
                    widget_id,
                    *count
                );

                // Phase-12: Record key-to-widget mapping for stability assertions
                self.key_to_widget.insert(*k, widget_id);
            }
        }
    }

    /// Set the background clear color (typically from theme.colors.surfaces.base0)
    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// SPEC-R-001: Begin a clipping region (stack-based).
    ///
    /// All nodes pushed between begin_clip() and end_clip() become children
    /// of the resulting ClipNode. Clips must always be closed with end_clip().
    ///
    /// ## Debug Assertions
    ///
    /// - Clip rect must be finite (no NaN or infinite)
    /// - Stack depth must not exceed MAX_SCENE_BUILDER_CLIP_DEPTH
    /// SPEC-R-020 Stage 5: Begin a clipping region with optional rounded corners
    pub fn begin_clip(&mut self, rect: RectF, rounded: Option<RoundedCorners>) {
        // Debug: validate clip rect is finite
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                rect.x.is_finite()
                    && rect.y.is_finite()
                    && rect.width.is_finite()
                    && rect.height.is_finite(),
                "SPEC-R-001: Clip rect must be finite, got {:?}",
                rect
            );
            debug_assert!(
                self.clip_stack.len() < MAX_SCENE_BUILDER_CLIP_DEPTH,
                "SPEC-R-001: SceneBuilder clip stack depth exceeded {} (current: {})",
                MAX_SCENE_BUILDER_CLIP_DEPTH,
                self.clip_stack.len()
            );
        }

        let start_index = self.nodes.len();
        self.clip_stack.push((rect, rounded, start_index));
    }

    /// SPEC-R-001: End the current clipping region.
    ///
    /// Collects all nodes pushed since the matching begin_clip() and wraps
    /// them in a ClipNode. The ClipNode is added to the parent level.
    ///
    /// ## Debug Assertions
    ///
    /// - Must have a matching begin_clip() call (stack not empty)
    pub fn end_clip(&mut self) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.clip_stack.is_empty(),
            "SPEC-R-001: end_clip() called without matching begin_clip()"
        );

        if let Some((clip_rect, rounded, start_index)) = self.clip_stack.pop() {
            let child_count = self.nodes.len() - start_index;
            let mut clipped_nodes = self.take_node_vector(child_count);
            clipped_nodes.extend(self.nodes.drain(start_index..));
            self.nodes.push(SceneNode::Clip(ClipNode {
                rect: clip_rect,
                rounded, // SPEC-R-020 Stage 5: Use rounded from begin_clip()
                children: clipped_nodes,
                z_index: 1,
                key: None,
            }));
        }
    }

    /// SPEC-R-001: Scoped clipping region with guaranteed cleanup.
    ///
    /// Executes `f` within a clip region, guaranteeing `end_clip()` is called
    /// even if `f` panics. This eliminates clip-stack fragility from manual
    /// begin/end pairing.
    ///
    /// # Arguments
    /// * `rect` - The clipping rectangle
    /// * `rounded` - Optional rounded corners for the clip
    /// * `f` - Closure to execute within the clip region
    ///
    /// # Panic Safety
    /// If `f` panics, `end_clip()` is still called before the panic propagates.
    ///
    /// # Panic Mode Requirement
    /// **INV-CLIP-SAFETY-1**: This guarantee requires `panic = "unwind"` (the default).
    /// In `panic = "abort"` builds, this is best-effort only — the process terminates
    /// before cleanup can run. Most Rust builds use unwind; abort mode is opt-in.
    pub fn with_clip<F, R>(&mut self, rect: RectF, rounded: Option<RoundedCorners>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.begin_clip(rect, rounded);
        // Use raw pointer to allow calling end_clip after catch_unwind
        let self_ptr = self as *mut Self;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: self is valid for the entire scope, and the closure's
            // mutable borrow ends before end_clip is called.
            f(unsafe { &mut *self_ptr })
        }));
        // INV-CLIP-SAFETY-2: Clip stack must have at least one entry from begin_clip above.
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.clip_stack.is_empty(),
            "with_clip: clip_stack empty before end_clip — begin_clip failed silently?"
        );
        self.end_clip();
        match result {
            Ok(r) => r,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    /// Get current clip stack depth (for debugging).
    pub fn clip_stack_depth(&self) -> usize {
        self.clip_stack.len()
    }

    /// SPEC-HAL-086 Stage 7: Begin a scroll transform region.
    ///
    /// All nodes pushed between begin_scroll() and end_scroll() become children
    /// of a ScrollNode. The scroll offset is applied to all children during rendering.
    pub fn begin_scroll(&mut self, offset: (f32, f32)) {
        let start_index = self.nodes.len();
        self.scroll_stack.push((offset, start_index));
    }

    /// SPEC-HAL-086 Stage 7: End the current scroll transform region.
    ///
    /// Collects all nodes pushed since the matching begin_scroll() and wraps
    /// them in a ScrollNode. The ScrollNode is added to the parent level.
    pub fn end_scroll(&mut self) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !self.scroll_stack.is_empty(),
            "SPEC-HAL-086: end_scroll() called without matching begin_scroll()"
        );

        if let Some((offset, start_index)) = self.scroll_stack.pop() {
            let child_count = self.nodes.len() - start_index;
            let mut scrolled_nodes = self.take_node_vector(child_count);
            scrolled_nodes.extend(self.nodes.drain(start_index..));
            self.nodes.push(SceneNode::Scroll(ScrollNode {
                offset,
                children: scrolled_nodes,
                key: None,
            }));
        }
    }

    #[track_caller]
    pub fn push_rect(
        &mut self,
        rect: RectF,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        {
            self.debug_rect_count += 1;
            self.assert_key_unique(key.as_ref());
        }
        self.nodes.push(SceneNode::Rect(RectNode {
            rect,
            color,
            z_index,
            key,
        }));
    }

    /// Push a border rectangle with optional rounded corners.
    ///
    /// INV-BORDER-GEOMETRY-01: Border visuals conform to shape geometry.
    /// Pass the same radius used for the underlying shape to ensure
    /// focus rings and selection borders hug rounded widgets correctly.
    pub fn push_border_rect(
        &mut self,
        rect: RectF,
        fill_color: Color,
        border_color: Color,
        border_width: f32,
        radius: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::BorderRect(BorderRectNode {
            rect,
            fill_color,
            border_color,
            border_width,
            radius,
            z_index,
            key,
        }));
    }

    #[track_caller]
    pub fn push_rounded_rect(
        &mut self,
        rect: RectF,
        color: Color,
        radius: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        {
            self.debug_rounded_rect_count += 1;
            let loc = std::panic::Location::caller();
            self.debug_rounded_rect_callsites
                .push((loc.file(), loc.line(), rect, radius));
            self.assert_key_unique(key.as_ref());
        }
        self.nodes.push(SceneNode::RoundedRect(RoundedRectNode {
            rect,
            color,
            radius,
            z_index,
            key,
        }));
    }

    /// **LAYOUT-OWNED TEXT ENTRY POINT — SPEC-HAL-159**
    ///
    /// This is the scene-level text entry point for **layout-owned text only**.
    ///
    /// # Authorized Callsites (ALL BUILDS)
    ///
    /// - `src/platform/*text_engine*.rs` — TextEngine trait implementations
    /// - `src/widgets/*.rs` — Widget internal implementation (e.g., caret rendering)
    ///
    /// # Unauthorized Callsites (use `push_text_probe` instead)
    ///
    /// - `examples/` — Renderer probes MUST use `push_text_probe` (debug-only)
    /// - Debug overlays — MUST use `push_text_probe` (debug-only)
    ///
    /// # SPEC-HAL-159: Text Ownership and Layout Authority
    ///
    /// **UI text MUST be layout-owned.** Do NOT use this API for UI semantics.
    ///
    /// If you need to display text as part of the UI:
    /// - Use `Label`, `TextBlock`, or another text widget
    /// - Use `TextEngine::draw()` from within a widget's `paint()` method
    ///
    /// Direct scene-level text rendering for UI purposes violates architectural
    /// invariants and will cause layout, coordinate, and scroll bugs.
    ///
    /// **RELEASE-GRADE ENFORCEMENT**: Unauthorized calls PANIC in ALL builds.
    #[track_caller]
    pub fn push_text(
        &mut self,
        position: Point,
        text: String,
        font_size: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // SPEC-HAL-159 INV-TEXT-OWNERSHIP-3: Verify caller is authorized (ALL BUILDS)
        Self::check_scene_text_caller("push_text");

        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Text(TextNode {
            binding: None,
            position,
            text,
            font_size,
            color,
            z_index,
            backgrounds: Vec::new(),
            key,
        }));
    }

    /// **LAYOUT-OWNED TEXT ENTRY POINT — SPEC-HAL-159**
    ///
    /// SPEC-EXIT-A1: Push text with background spans for selection/caret rendering.
    ///
    /// Background spans are rendered BEFORE glyphs in the text pass, guaranteeing
    /// correct layering without z-index or submission order dependencies.
    ///
    /// # Authorized Callsites (ALL BUILDS)
    ///
    /// - `src/platform/*text_engine*.rs` — TextEngine trait implementations
    /// - `src/widgets/*.rs` — Widget internal implementation (e.g., caret/selection)
    ///
    /// # Unauthorized Callsites (use `push_text_probe` instead)
    ///
    /// - `examples/` — Renderer probes MUST use probe API (debug-only)
    /// - Debug overlays — MUST use probe API (debug-only)
    ///
    /// # SPEC-HAL-159: Text Ownership and Layout Authority
    ///
    /// **UI text MUST be layout-owned.** Do NOT use this API for UI semantics.
    ///
    /// If you need to display text as part of the UI:
    /// - Use `Label`, `TextBlock`, or another text widget
    /// - Use `TextEngine::draw_with_backgrounds()` from within a widget
    ///
    /// **RELEASE-GRADE ENFORCEMENT**: Unauthorized calls PANIC in ALL builds.
    #[track_caller]
    pub fn push_text_with_backgrounds(
        &mut self,
        position: Point,
        text: String,
        font_size: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // SPEC-HAL-159 INV-TEXT-OWNERSHIP-3: Verify caller is authorized (ALL BUILDS)
        Self::check_scene_text_caller("push_text_with_backgrounds");

        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Text(TextNode {
            position,
            binding: None,
            text,
            font_size,
            color,
            z_index,
            backgrounds,
            key,
        }));
    }

    // ========================================================================
    // SPEC-HAL-159: Explicit escape hatch for intentional scene-level text
    // ========================================================================

    /// **DIAGNOSTIC ESCAPE HATCH — SPEC-HAL-159**
    ///
    /// Intentional scene-level text for probes, diagnostics, and live debugging.
    /// This API explicitly acknowledges layout bypass.
    ///
    /// # When to use this
    ///
    /// - Renderer probes and instrumentation
    /// - Live diagnostic overlays
    /// - Temporary debugging text
    /// - Performance counters / FPS displays
    ///
    /// # When NOT to use this
    ///
    /// - UI text that users see as part of the application
    /// - Text that participates in layout
    /// - Anything intended for ordinary release builds
    ///
    /// This function is intentionally ugly (`_probe`) to discourage casual use.
    /// Available in debug builds and internal test binaries.
    #[cfg(any(debug_assertions, test))]
    #[track_caller]
    pub fn push_text_probe(
        &mut self,
        position: Point,
        text: String,
        font_size: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        let location = std::panic::Location::caller();
        tracing::debug!(
            "[SPEC-HAL-159] Explicit scene text probe at {}:{} — layout bypass acknowledged",
            location.file(),
            location.line()
        );

        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Text(TextNode {
            position,
            binding: None,
            text,
            font_size,
            color,
            z_index,
            backgrounds: Vec::new(),
            key,
        }));
    }

    pub fn push_image(
        &mut self,
        rect: RectF,
        texture_id: u64,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Image(ImageNode {
            rect,
            texture_source: ImageTextureSource::Registered(texture_id),
            uv_rect,
            tint,
            z_index,
            key,
        }));
    }

    pub fn push_admitted_image(
        &mut self,
        rect: RectF,
        binding: &crate::renderer::image_binding::ImageBinding,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.admitted_image_dependencies
            .get_or_insert_with(Default::default)
            .push(binding.clone());
        self.nodes.push(SceneNode::Image(ImageNode {
            rect,
            texture_source: ImageTextureSource::Admitted(binding.identity()),
            uv_rect,
            tint,
            z_index,
            key,
        }));
    }

    pub(crate) fn take_admitted_image_dependencies(
        &mut self,
    ) -> Vec<crate::renderer::image_binding::ImageBinding> {
        self.admitted_image_dependencies
            .take()
            .map(|dependencies| *dependencies)
            .unwrap_or_default()
    }

    pub fn push_target_bound_image(
        &mut self,
        rect: RectF,
        presentation_id: CustomRenderPresentationId,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Image(ImageNode {
            rect,
            texture_source: ImageTextureSource::TargetBound(presentation_id),
            uv_rect,
            tint,
            z_index,
            key,
        }));
    }

    pub fn push_clip(
        &mut self,
        rect: RectF,
        children: Vec<SceneNode>,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Clip(ClipNode {
            rect,
            rounded: None, // SPEC-R-020 Stage 3: No rounding by default
            children,
            z_index,
            key,
        }));
    }

    pub fn push_layer(
        &mut self,
        rect: RectF,
        children: Vec<SceneNode>,
        opacity: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Layer(LayerNode {
            rect,
            children,
            opacity,
            z_index,
            key,
        }));
    }

    // SPEC-R-003: Shape primitive push methods.

    /// Push a circle shape node
    pub fn push_circle(
        &mut self,
        center: Point,
        radius: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Circle(CircleNode {
            center,
            radius,
            color,
            z_index,
            key,
        }));
    }

    /// Push an ellipse shape node
    pub fn push_ellipse(
        &mut self,
        center: Point,
        radii: Point,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Ellipse(EllipseNode {
            center,
            radii,
            color,
            z_index,
            key,
        }));
    }

    /// Push a line segment node
    pub fn push_line(
        &mut self,
        a: Point,
        b: Point,
        width: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Line(LineNode {
            a,
            b,
            width,
            color,
            z_index,
            key,
        }));
    }

    /// Push a polyline node
    pub fn push_polyline(
        &mut self,
        points: Vec<Point>,
        width: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Polyline(PolylineNode {
            points,
            width,
            color,
            z_index,
            key,
        }));
    }

    /// HAL386: final clip and target admission belongs to the composed CPU Build.
    pub fn push_frosted_glass(&mut self, rect:RectF, corners:paint::RoundedCorners,
        appearance:&paint::FrostedGlassAppearance, z_index:ZIndex, key:Option<SceneNodeKey>)
        ->Result<(),paint::FrostedGlassError>
    {
        if !appearance.validate_geometry(rect,corners)? {return Ok(());}
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes.push(SceneNode::FrostedGlass(FrostedGlassNode {
            rect,corners,appearance:appearance.clone(),z_index,key,
        }));
        Ok(())
    }

    /// HAL386: final clip and target admission belongs to the composed CPU Build.
    pub fn push_analytic_surface(
        &mut self,
        rect: RectF,
        corners: paint::RoundedCorners,
        material: &paint::SurfaceAnalytic,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), paint::AnalyticSurfaceError> {
        if !material.validate_geometry(rect, corners)? {
            return Ok(());
        }
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes
            .push(SceneNode::AnalyticSurface(AnalyticSurfaceNode {
                rect,
                corners,
                material: material.clone(),
                z_index,
                key,
            }));
        Ok(())
    }

    /// HAL-388: local optics admission; final inherited clips are checked at Build.
    pub fn push_local_ambient_glow(
        &mut self,
        rect: RectF,
        appearance: paint::LocalAmbientGlowAppearance,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), paint::LocalAmbientGlowError> {
        let Some(bounds) = appearance.bounds(rect)? else {
            return Ok(());
        };
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes
            .push(SceneNode::LocalAmbientGlow(LocalAmbientGlowNode {
                rect,
                bounds,
                appearance,
                z_index,
                key,
            }));
        Ok(())
    }

    /// SPEC-HAL-358: Validate and push one luminous line/polyline contour.
    pub fn push_luminous_contour(
        &mut self,
        points: Arc<[Point]>,
        core_width: f32,
        color: Color,
        appearance: LuminousContourAppearance,
        fronts: LuminousContourFronts,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), LuminousContourError> {
        if !(2..=paint::MAX_LUMINOUS_CONTOUR_POINTS).contains(&points.len()) {
            return Err(LuminousContourError::InvalidPointCount);
        }
        if !core_width.is_finite() || !(0.25..=4.0).contains(&core_width) {
            return Err(LuminousContourError::InvalidCoreWidth);
        }
        if ![color.r, color.g, color.b, color.a]
            .into_iter()
            .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
        {
            return Err(LuminousContourError::InvalidColor);
        }

        let mut total_length = 0.0;
        for point in points.iter() {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(LuminousContourError::NonFinitePoint);
            }
        }
        for segment in points.windows(2) {
            let length = segment[0].distance(segment[1]);
            if !length.is_finite() || length <= 0.0 {
                return Err(LuminousContourError::DegenerateSegment);
            }
            total_length += length;
            if !total_length.is_finite() {
                return Err(LuminousContourError::NonFiniteTotalLength);
            }
        }
        if total_length <= 0.0 {
            return Err(LuminousContourError::NonFiniteTotalLength);
        }

        for front in fronts.iter() {
            if front.distance() > total_length {
                return Err(LuminousContourError::FrontBeyondContour);
            }
            if front.half_extent() < core_width {
                return Err(LuminousContourError::FrontNarrowerThanCore);
            }
        }

        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes
            .push(SceneNode::LuminousContour(LuminousContourNode {
                points,
                core_width,
                color,
                appearance,
                fronts,
                total_length,
                z_index,
                key,
            }));
        Ok(())
    }

    /// SPEC-R-004 Stage 15/16/18: Push a path node with fill color and z-index.
    /// Sets `is_stroke = false` (fill mode), `debug = None`.
    pub fn push_path(
        &mut self,
        path: crate::renderer::path::Path,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_path_with_fill_rule(path, FillRule::NonZero, color, z_index, key);
    }

    /// Emit a path whose contours are filled together under the given rule.
    pub fn push_path_with_fill_rule(
        &mut self,
        path: crate::renderer::path::Path,
        rule: FillRule,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Path(PathNode {
            path,
            mesh: None,
            color,
            fill_rule: rule,
            is_stroke: false,
            debug: None,
            z_index,
            key,
        }));
    }

    /// SPEC-R-004: Build and push a stroked path while retaining true `Close`
    /// semantics. The produced vectors move directly into the scene mesh.
    pub fn push_stroked_path(
        &mut self,
        path: crate::renderer::path::Path,
        stroke: crate::renderer::path::StrokeParams,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        let Some(stroke_mesh) = path.build_stroke_mesh(stroke) else {
            return;
        };
        let mesh = crate::renderer::path::PathMesh {
            vertices: stroke_mesh.vertices,
            indices: stroke_mesh.indices,
            bounds: stroke_mesh.bounds,
        };

        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Path(PathNode {
            path,
            mesh: Some(mesh),
            color,
            fill_rule: FillRule::NonZero,
            is_stroke: true,
            debug: None,
            z_index,
            key,
        }));
    }

    /// SPEC-R-004 Stage 15/16/18: Push a path node with pre-computed mesh, fill color, and z-index.
    /// Sets `is_stroke = false` (fill mode), `debug = None`.
    pub fn push_path_with_mesh(
        &mut self,
        path: crate::renderer::path::Path,
        mesh: crate::renderer::path::PathMesh,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Path(PathNode {
            path,
            mesh: Some(mesh),
            color,
            fill_rule: FillRule::NonZero,
            is_stroke: false,
            debug: None,
            z_index,
            key,
        }));
    }

    /// SPEC-R-004 Stage 17/18: Push a path node with debug visualization and z-index.
    pub fn push_path_debug(
        &mut self,
        path: crate::renderer::path::Path,
        color: Color,
        mode: crate::renderer::path::PathDebugMode,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Path(PathNode {
            path,
            mesh: None,
            color,
            fill_rule: FillRule::NonZero,
            is_stroke: false,
            debug: Some(mode),
            z_index,
            key,
        }));
    }

    /// SPEC-R-004 Stage 17/18: Push a path node with pre-computed mesh, debug visualization, and z-index.
    pub fn push_path_with_mesh_debug(
        &mut self,
        path: crate::renderer::path::Path,
        mesh: crate::renderer::path::PathMesh,
        color: Color,
        mode: crate::renderer::path::PathDebugMode,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::Path(PathNode {
            path,
            mesh: Some(mesh),
            color,
            fill_rule: FillRule::NonZero,
            is_stroke: false,
            debug: Some(mode),
            z_index,
            key,
        }));
    }

    /// SPEC-HAL-265: Push an icon into the scene.
    ///
    /// Resolves the icon's path geometry, scales it from viewbox to `size` logical pixels,
    /// translates to `(x, y)`, and emits a `SceneNode::Path`. No pre-computed mesh — the
    /// mesh cache handles tessellation.
    pub fn push_icon(
        &mut self,
        definition: &crate::core::icon::IconDefinition,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        z_index: ZIndex,
    ) {
        // SPEC-HAL-266 §3.1: Snap icon position to device pixel grid
        let sf = self.scale_factor;
        let snapped_x = (x * sf).round() / sf;
        let snapped_y = (y * sf).round() / sf;
        let scale = size / definition.viewbox_width;
        for layer in &definition.layers {
            let (lr, lg, lb, la) = layer.role.resolve(color.r, color.g, color.b, color.a);
            let layer_color = Color {
                r: lr,
                g: lg,
                b: lb,
                a: la,
            };
            let scaled_path = layer.path.transformed(scale, snapped_x, snapped_y);
            self.push_path_with_fill_rule(scaled_path, layer.fill_rule, layer_color, z_index, None);
        }
    }

    /// SPEC-HAL-265: Push an icon from an `IconSpec` + `IconRegistry`.
    ///
    /// Convenience method that looks up the definition by `spec.id`.
    /// Returns `true` if the icon was found and emitted, `false` if missing (no panic).
    pub fn push_icon_spec(
        &mut self,
        spec: &crate::core::icon::IconSpec,
        registry: &crate::core::icon::IconRegistry,
        x: f32,
        y: f32,
        z_index: ZIndex,
    ) -> bool {
        if let Some(def) = registry.get(spec.id) {
            self.push_icon(def, x, y, spec.size, spec.color, z_index);
            true
        } else {
            false
        }
    }

    /// SPEC-HAL-086 Stage 8: Push a vertical scrollbar overlay node.
    ///
    /// This creates a hit-testable region for the scrollbar track.
    /// The node is rendered in viewport space (does not scroll with content).
    pub fn push_scrollbar_v(
        &mut self,
        track_abs: RectF,
        owner_abs: RectF,
        widget_id: TreeWidgetId,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::ScrollbarV(ScrollbarVNode {
            track_abs_rect: track_abs,
            owner_abs_rect: owner_abs,
            widget_id,
            key,
        }));
    }

    /// SPEC-HAL-086 Stage 8: Push a horizontal scrollbar overlay node.
    ///
    /// This creates a hit-testable region for the scrollbar track.
    /// The node is rendered in viewport space (does not scroll with content).
    pub fn push_scrollbar_h(
        &mut self,
        track_abs: RectF,
        owner_abs: RectF,
        widget_id: TreeWidgetId,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());

        self.nodes.push(SceneNode::ScrollbarH(ScrollbarHNode {
            track_abs_rect: track_abs,
            owner_abs_rect: owner_abs,
            widget_id,
            key,
        }));
    }

    /// Push multiple rects as a single PathMesh (one GPU draw call).
    ///
    /// Each rect becomes 4 vertices + 6 indices (2 triangles) in a combined mesh.
    /// The mesh is submitted as a single PathNode through the existing path pipeline.
    pub fn push_rect_mesh(&mut self, rects: &[RectF], color: Color, z_index: ZIndex) {
        if rects.is_empty() {
            return;
        }
        let mut vertices = Vec::with_capacity(rects.len() * 4);
        let mut indices = Vec::with_capacity(rects.len() * 6);
        let r0 = &rects[0];
        let mut min_x = r0.x;
        let mut min_y = r0.y;
        let mut max_x = r0.x + r0.width;
        let mut max_y = r0.y + r0.height;
        for (i, r) in rects.iter().enumerate() {
            let base = (i * 4) as u32;
            vertices.push([r.x, r.y]);
            vertices.push([r.x + r.width, r.y]);
            vertices.push([r.x + r.width, r.y + r.height]);
            vertices.push([r.x, r.y + r.height]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.width);
            max_y = max_y.max(r.y + r.height);
        }
        let bounds = RectF::new(min_x, min_y, max_x - min_x, max_y - min_y);
        let mesh = crate::renderer::path::PathMesh {
            vertices,
            indices,
            bounds,
        };
        self.push_path_with_mesh(
            crate::renderer::path::Path::new(),
            mesh,
            color,
            z_index,
            None,
        );
    }

    /// SPEC-R-013-v2: Push a shadow node.
    #[track_caller]
    pub fn push_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        mode: ShadowMode,
        offset: (f32, f32),
        spread: f32,
        blur_radius: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes.push(SceneNode::Shadow(ShadowNode {
            shape_rect,
            shape_radius,
            mode,
            offset,
            spread,
            blur_radius,
            color,
            z_index,
            key,
        }));
    }

    /// SPEC-R-012-v2: Push a gradient rect node.
    #[track_caller]
    pub fn push_gradient_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        gradient: GradientDef,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes.push(SceneNode::GradientRect(GradientRectNode {
            rect,
            radius,
            gradient,
            z_index,
            key,
        }));
    }

    /// SPEC-HAL-272: Push a color picker SV area node.
    #[track_caller]
    pub fn push_color_picker_sv(
        &mut self,
        rect: RectF,
        hue: f32,
        radius: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes.push(SceneNode::ColorPickerSv(ColorPickerSvNode {
            rect,
            hue,
            radius,
            z_index,
            key,
        }));
    }

    /// SPEC-HAL-278: Push a radial gauge node.
    #[track_caller]
    pub fn push_radial_gauge(
        &mut self,
        rect: RectF,
        fill: f32,
        thickness: f32,
        track_color: Color,
        fill_color: Color,
        fill_color_negative: Color,
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
        key: Option<SceneNodeKey>,
    ) {
        #[cfg(debug_assertions)]
        self.assert_key_unique(key.as_ref());
        self.nodes.push(SceneNode::RadialGauge(RadialGaugeNode {
            rect,
            fill,
            thickness,
            track_color: [track_color.r, track_color.g, track_color.b, track_color.a],
            fill_color: [fill_color.r, fill_color.g, fill_color.b, fill_color.a],
            fill_color_negative: [
                fill_color_negative.r,
                fill_color_negative.g,
                fill_color_negative.b,
                fill_color_negative.a,
            ],
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
            z_index,
            key,
        }));
    }

    /// DEBUG: Print per-frame primitive counts and callsites.
    /// Call this before build() to see what was pushed this frame.
    #[cfg(debug_assertions)]
    pub fn debug_print_stats(&self, label: &str) {
        tracing::debug!(
            "[SceneBuilder] {} | rect={} rounded_rect={}",
            label,
            self.debug_rect_count,
            self.debug_rounded_rect_count
        );
        for (file, line, rect, radius) in &self.debug_rounded_rect_callsites {
            tracing::debug!(
                "  rounded_rect @ {}:{} | rect=({:.1},{:.1},{:.1},{:.1}) r={:.1}",
                file,
                line,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius
            );
        }
    }

    // ========================================================================
    // SPEC-HAL-159: Release-grade scene text caller verification
    // ========================================================================

    /// SPEC-HAL-159: Verify that scene-level text APIs are called from authorized locations.
    ///
    /// This is a RELEASE-GRADE enforcer. Unauthorized calls PANIC in ALL builds.
    /// Zero tolerance for UI text rendered outside of layout-owned widgets.
    ///
    /// # Authorized Callers (ALL BUILDS)
    ///
    /// - TextEngine implementations (`*text_engine*` in path) — the conduit for widget text
    /// - Widget internal implementations (`widgets/` in path) — for caret/selection rendering
    ///
    /// # Debug-Only Callers (use `push_text_probe` instead)
    ///
    /// - Renderer probes and demos (`examples/`) — MUST use `push_text_probe` API
    /// - Debug overlays — MUST use `push_text_probe` API
    ///
    /// Unauthorized calls PANIC immediately referencing SPEC-HAL-159.
    #[track_caller]
    fn check_scene_text_caller(method_name: &str) {
        let location = std::panic::Location::caller();
        let file = location.file();
        let line = location.line();

        // Release-grade allowlist: ONLY layout-owned text paths
        // Examples and debug overlays MUST use push_text_probe (debug-only API)
        let is_authorized = Self::is_authorized_scene_text_path(file);

        if !is_authorized {
            panic!(
                "\n\
                ╔══════════════════════════════════════════════════════════════════════╗\n\
                ║  SPEC-HAL-159 VIOLATION: Unauthorized scene-level text               ║\n\
                ╠══════════════════════════════════════════════════════════════════════╣\n\
                ║  UI text MUST be layout-owned (use Label, TextBlock, etc.)          ║\n\
                ║                                                                      ║\n\
                ║  Method: SceneBuilder::{}()\n\
                ║  File:   {}:{}\n\
                ║                                                                      ║\n\
                ║  For debug overlays or renderer probes, use push_text_probe()       ║\n\
                ║  instead (debug-only API, unavailable in release builds).           ║\n\
                ╚══════════════════════════════════════════════════════════════════════╝",
                method_name, file, line
            );
        }
    }

    fn is_authorized_scene_text_path(file: &str) -> bool {
        file.contains("text_engine")
            || file.contains("widgets/")
            || file.contains("widgets\\")
            || file.contains("scene/mod")
            || file.contains("scene\\mod")
    }

    /// SPEC-R-001: Build the scene graph from accumulated nodes.
    ///
    /// ## Debug Assertions
    ///
    /// - All begin_clip() calls must have matching end_clip() calls
    /// - All begin_scroll() calls must have matching end_scroll() calls (SPEC-HAL-086 Stage 7)
    pub fn build(self) -> SceneGraph {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                self.clip_stack.is_empty(),
                "SPEC-R-001: SceneBuilder has {} unclosed begin_clip() calls",
                self.clip_stack.len()
            );
            debug_assert!(
                self.scroll_stack.is_empty(),
                "SPEC-HAL-086: SceneBuilder has {} unclosed begin_scroll() calls",
                self.scroll_stack.len()
            );
            debug_assert!(
                self.control_subtree_stack.is_empty(),
                "SPEC-HAL-359: SceneBuilder has an unclosed control subtree member"
            );
        }

        SceneGraph {
            root: self.nodes,
            clear_color: self.clear_color,
            #[cfg(debug_assertions)]
            key_widget_map: self.key_to_widget,
        }
    }
}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// PaintSink Implementation (widget→scene bridge)
// ============================================================================

impl PaintSink for SceneBuilder {
    fn push_frosted_glass(&mut self, rect:RectF, corners:paint::RoundedCorners,
        appearance:&paint::FrostedGlassAppearance, z_index:ZIndex, key:Option<SceneNodeKey>)
        ->Result<(),paint::FrostedGlassError>
    { SceneBuilder::push_frosted_glass(self,rect,corners,appearance,z_index,key) }
    fn push_local_ambient_glow(
        &mut self,
        rect: RectF,
        appearance: paint::LocalAmbientGlowAppearance,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), paint::LocalAmbientGlowError> {
        SceneBuilder::push_local_ambient_glow(self, rect, appearance, z_index, key)
    }

    fn push_analytic_surface(
        &mut self,
        rect: RectF,
        corners: paint::RoundedCorners,
        material: &paint::SurfaceAnalytic,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), paint::AnalyticSurfaceError> {
        SceneBuilder::push_analytic_surface(self, rect, corners, material, z_index, key)
    }
    fn begin_control_subtree_member(&mut self, member: paint::ControlSubtreeMember) {
        self.control_subtree_stack.push((member, self.nodes.len()));
    }

    fn end_control_subtree_member(&mut self) {
        let (member, start) = self
            .control_subtree_stack
            .pop()
            .expect("SPEC-HAL-359: unbalanced control subtree wrapper");
        let children = Arc::from(self.nodes.split_off(start));
        self.push_control_subtree_fragment(children, Some(member));
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn begin_clip(&mut self, rect: RectF, rounded: Option<paint::RoundedCorners>) {
        self.begin_clip(rect, rounded.map(|r| r.into()));
    }

    fn end_clip(&mut self) {
        self.end_clip();
    }

    fn begin_scroll(&mut self, offset: (f32, f32)) {
        self.begin_scroll(offset);
    }

    fn end_scroll(&mut self) {
        self.end_scroll();
    }

    fn push_rect(
        &mut self,
        rect: RectF,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_rect(rect, color.into(), z_index, key);
    }

    fn push_border_rect(
        &mut self,
        rect: RectF,
        color: paint::Color,
        border_width: f32,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // Map to SceneBuilder's 7-param push_border_rect with:
        // - transparent fill (border-only)
        // - color as border_color
        // - 0.0 radius (no rounding)
        self.push_border_rect(
            rect,
            Color::TRANSPARENT,
            color.into(),
            border_width,
            0.0,
            z_index,
            key,
        );
    }

    fn push_rounded_rect(
        &mut self,
        rect: RectF,
        color: paint::Color,
        corners: paint::RoundedCorners,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // Proper handling of corners:
        // - Uniform corners → pass single radius to SceneBuilder
        // - Non-uniform corners → log warning in debug, use max radius as fallback
        //   (SceneBuilder only supports uniform radius currently)
        let radius = if let Some(r) = corners.uniform_radius() {
            r
        } else {
            #[cfg(debug_assertions)]
            tracing::warn!(
                "push_rounded_rect: non-uniform corners not supported, using max radius. \
                 corners: tl={}, tr={}, bl={}, br={}",
                corners.tl,
                corners.tr,
                corners.bl,
                corners.br
            );
            corners.tl.max(corners.tr).max(corners.bl).max(corners.br)
        };
        self.push_rounded_rect(rect, color.into(), radius, z_index, key);
    }

    fn push_border_rounded_rect(
        &mut self,
        rect: RectF,
        color: paint::Color,
        border_width: f32,
        corners: paint::RoundedCorners,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // Proper handling of corners:
        // - Uniform corners → pass single radius to SceneBuilder.push_border_rect
        // - Non-uniform corners → log warning in debug, use max radius as fallback
        let radius = if let Some(r) = corners.uniform_radius() {
            r
        } else {
            #[cfg(debug_assertions)]
            tracing::warn!(
                "push_border_rounded_rect: non-uniform corners not supported, using max radius. \
                 corners: tl={}, tr={}, bl={}, br={}",
                corners.tl,
                corners.tr,
                corners.bl,
                corners.br
            );
            corners.tl.max(corners.tr).max(corners.bl).max(corners.br)
        };
        // Map to SceneBuilder's 7-param push_border_rect with:
        // - transparent fill (border-only)
        // - color as border_color
        // - extracted radius
        self.push_border_rect(
            rect,
            Color::TRANSPARENT,
            color.into(),
            border_width,
            radius,
            z_index,
            key,
        );
    }

    fn push_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        mode: paint::ShadowMode,
        offset: (f32, f32),
        spread: f32,
        blur_radius: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_shadow(
            shape_rect,
            shape_radius,
            mode,
            offset,
            spread,
            blur_radius,
            color.into(),
            z_index,
            key,
        );
    }

    fn push_gradient_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        gradient: &paint::GradientDef,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_gradient_rect(rect, radius, gradient.clone(), z_index, key);
    }

    fn push_color_picker_sv(
        &mut self,
        rect: RectF,
        hue: f32,
        radius: f32,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_color_picker_sv(rect, hue, radius, z_index, key);
    }

    fn push_radial_gauge(
        &mut self,
        rect: RectF,
        fill: f32,
        thickness: f32,
        track_color: paint::Color,
        fill_color: paint::Color,
        fill_color_negative: paint::Color,
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
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_radial_gauge(
            rect,
            fill,
            thickness,
            track_color.into(),
            fill_color.into(),
            fill_color_negative.into(),
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
            z_index,
            key,
        );
    }

    fn push_circle(
        &mut self,
        center: Point,
        radius: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_circle(center, radius, color.into(), z_index, key);
    }

    fn push_ellipse(
        &mut self,
        center: Point,
        radii: Point,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_ellipse(center, radii, color.into(), z_index, key);
    }

    fn push_path(
        &mut self,
        path: crate::renderer::path::Path,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_path(path, color.into(), z_index, key);
    }

    fn push_path_with_fill_rule(
        &mut self,
        path: crate::renderer::path::Path,
        rule: FillRule,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), PathFillError> {
        SceneBuilder::push_path_with_fill_rule(self, path, rule, color.into(), z_index, key);
        Ok(())
    }

    fn push_stroked_path(
        &mut self,
        path: crate::renderer::path::Path,
        stroke: crate::renderer::path::StrokeParams,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        SceneBuilder::push_stroked_path(self, path, stroke, color.into(), z_index, key);
    }

    fn push_icon(
        &mut self,
        definition: &crate::core::icon::IconDefinition,
        x: f32,
        y: f32,
        size: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
    ) {
        self.push_icon(definition, x, y, size, color.into(), z_index);
    }

    fn push_rect_mesh(&mut self, rects: &[RectF], color: paint::Color, z_index: paint::ZIndex) {
        self.push_rect_mesh(rects, color.into(), z_index);
    }

    fn push_text_with_backgrounds(
        &mut self,
        run: &crate::core::text_engine::GlyphRun,
        x: f32,
        y: f32,
        color: paint::Color,
        backgrounds: Vec<paint::TextBackgroundSpan>,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        let scene_backgrounds: Vec<TextBackgroundSpan> =
            backgrounds.into_iter().map(|b| b.into()).collect();
        self.push_text_with_backgrounds(
            Point::new(x, y),
            run.text.clone(),
            run.font_size,
            color.into(),
            scene_backgrounds,
            z_index,
            key,
        );
        if let Some(SceneNode::Text(node)) = self.nodes.last_mut() {
            node.binding = run.binding.clone();
        }
    }

    fn set_current_painting_widget(&mut self, widget_id: u64) {
        #[cfg(debug_assertions)]
        self.set_current_painting_widget(widget_id);
        #[cfg(not(debug_assertions))]
        let _ = widget_id;
    }

    fn clear_current_painting_widget(&mut self) {
        #[cfg(debug_assertions)]
        self.clear_current_painting_widget();
    }

    fn push_image(
        &mut self,
        rect: RectF,
        texture_id: u64,
        uv_rect: RectF,
        tint: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_image(rect, texture_id, uv_rect, tint.into(), z_index, key);
    }

    fn push_admitted_image(
        &mut self,
        rect: RectF,
        binding: &crate::renderer::image_binding::ImageBinding,
        uv_rect: RectF,
        tint: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_admitted_image(rect, binding, uv_rect, tint.into(), z_index, key);
    }

    fn push_target_bound_image(
        &mut self,
        rect: RectF,
        presentation_id: CustomRenderPresentationId,
        uv_rect: RectF,
        tint: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_target_bound_image(rect, presentation_id, uv_rect, tint.into(), z_index, key);
    }

    fn push_line(
        &mut self,
        a: crate::core::math::Point,
        b: crate::core::math::Point,
        width: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_line(a, b, width, color.into(), z_index, key);
    }

    fn push_polyline(
        &mut self,
        points: Vec<crate::core::math::Point>,
        width: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        self.push_polyline(points, width, color.into(), z_index, key);
    }

    fn push_luminous_contour(
        &mut self,
        points: Arc<[Point]>,
        core_width: f32,
        color: paint::Color,
        appearance: LuminousContourAppearance,
        fronts: LuminousContourFronts,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), LuminousContourError> {
        self.push_luminous_contour(
            points,
            core_width,
            color.into(),
            appearance,
            fronts,
            z_index,
            key,
        )
    }

    #[cfg(debug_assertions)]
    fn push_text_probe(
        &mut self,
        position: (f32, f32),
        text: &str,
        font_size: f32,
        color: paint::Color,
        z_index: paint::ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        // Forward to SceneBuilder::push_text_probe
        self.push_text_probe(
            Point::new(position.0, position.1),
            text.to_string(),
            font_size,
            color.into(),
            z_index,
            key,
        );
    }
}

#[cfg(test)]
mod luminous_contour_scene_tests {
    use super::*;

    fn appearance() -> LuminousContourAppearance {
        LuminousContourAppearance::try_new(5.0, 0.12, 4.0).unwrap()
    }

    fn assert_rejected(
        points: Arc<[Point]>,
        core_width: f32,
        color: Color,
        fronts: LuminousContourFronts,
        expected: LuminousContourError,
    ) {
        let mut builder = SceneBuilder::new();
        assert_eq!(
            builder
                .push_luminous_contour(points, core_width, color, appearance(), fronts, 1, None,),
            Err(expected)
        );
        assert!(builder.build().root.is_empty());
    }

    #[test]
    fn luminous_contour_scene_admission_preserves_shared_geometry() {
        let points: Arc<[Point]> = Arc::from([
            Point::new(10.0, 20.0),
            Point::new(30.0, 20.0),
            Point::new(30.0, 35.0),
        ]);
        let fronts =
            LuminousContourFronts::try_from_slice(&[
                LuminousContourFront::try_new(20.0, 4.0, 0.8).unwrap()
            ])
            .unwrap();
        let mut builder = SceneBuilder::new();
        builder
            .push_luminous_contour(
                points.clone(),
                2.0,
                Color::from_linear(0.1, 0.4, 0.8, 1.0),
                appearance(),
                fronts,
                3,
                None,
            )
            .unwrap();
        let scene = builder.build();

        let SceneNode::LuminousContour(node) = &scene.root[0] else {
            panic!("expected luminous contour node");
        };
        assert!(Arc::ptr_eq(&points, node.points()));
        assert_eq!(node.total_length(), 35.0);
        assert_eq!(node.fronts(), fronts);
    }

    #[test]
    fn luminous_contour_scene_admission_is_transactional_on_invalid_input() {
        let points: Arc<[Point]> = Arc::from([Point::new(1.0, 1.0), Point::new(1.0, 1.0)]);
        let mut builder = SceneBuilder::new();
        assert_eq!(
            builder.push_luminous_contour(
                points,
                2.0,
                Color::WHITE,
                appearance(),
                LuminousContourFronts::empty(),
                1,
                None,
            ),
            Err(LuminousContourError::DegenerateSegment)
        );
        assert!(builder.build().root.is_empty());
    }

    #[test]
    fn luminous_contour_scene_rejects_front_geometry_mismatch() {
        let points: Arc<[Point]> = Arc::from([Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);
        let too_far =
            LuminousContourFronts::try_from_slice(&[
                LuminousContourFront::try_new(10.5, 3.0, 1.0).unwrap()
            ])
            .unwrap();
        let mut builder = SceneBuilder::new();
        assert_eq!(
            builder.push_luminous_contour(
                points,
                2.0,
                Color::WHITE,
                appearance(),
                too_far,
                1,
                None,
            ),
            Err(LuminousContourError::FrontBeyondContour)
        );
    }

    #[test]
    fn luminous_contour_scene_admission_covers_geometry_color_and_width_boundaries() {
        let line = || Arc::<[Point]>::from(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]);

        for width in [0.25, 4.0] {
            let mut builder = SceneBuilder::new();
            assert!(
                builder
                    .push_luminous_contour(
                        line(),
                        width,
                        Color::from_linear(0.0, 1.0, 0.5, 1.0),
                        appearance(),
                        LuminousContourFronts::empty(),
                        1,
                        None,
                    )
                    .is_ok()
            );
        }

        assert_rejected(
            Arc::<[Point]>::from(vec![Point::new(0.0, 0.0)]),
            1.0,
            Color::WHITE,
            LuminousContourFronts::empty(),
            LuminousContourError::InvalidPointCount,
        );
        let too_many = (0..=paint::MAX_LUMINOUS_CONTOUR_POINTS)
            .map(|index| Point::new(index as f32, 0.0))
            .collect::<Vec<_>>();
        assert_rejected(
            Arc::<[Point]>::from(too_many),
            1.0,
            Color::WHITE,
            LuminousContourFronts::empty(),
            LuminousContourError::InvalidPointCount,
        );
        assert_rejected(
            Arc::<[Point]>::from(vec![Point::new(0.0, 0.0), Point::new(f32::NAN, 1.0)]),
            1.0,
            Color::WHITE,
            LuminousContourFronts::empty(),
            LuminousContourError::NonFinitePoint,
        );
        for width in [0.24, 4.01, f32::INFINITY] {
            assert_rejected(
                line(),
                width,
                Color::WHITE,
                LuminousContourFronts::empty(),
                LuminousContourError::InvalidCoreWidth,
            );
        }
        for color in [
            Color::from_linear(-0.01, 0.0, 0.0, 1.0),
            Color::from_linear(0.0, 0.0, 0.0, 1.01),
            Color::from_linear(0.0, f32::NAN, 0.0, 1.0),
        ] {
            assert_rejected(
                line(),
                1.0,
                color,
                LuminousContourFronts::empty(),
                LuminousContourError::InvalidColor,
            );
        }

        let max_points = (0..paint::MAX_LUMINOUS_CONTOUR_POINTS)
            .map(|index| Point::new(index as f32, 0.0))
            .collect::<Vec<_>>();
        let mut builder = SceneBuilder::new();
        assert!(
            builder
                .push_luminous_contour(
                    Arc::<[Point]>::from(max_points),
                    1.0,
                    Color::WHITE,
                    appearance(),
                    LuminousContourFronts::empty(),
                    1,
                    None,
                )
                .is_ok()
        );
    }

    #[test]
    fn luminous_contour_scene_rejects_non_finite_lengths_and_narrow_fronts() {
        assert_rejected(
            Arc::<[Point]>::from(vec![Point::new(-f32::MAX, 0.0), Point::new(f32::MAX, 0.0)]),
            1.0,
            Color::WHITE,
            LuminousContourFronts::empty(),
            LuminousContourError::DegenerateSegment,
        );
        let narrow =
            LuminousContourFronts::try_from_slice(&[
                LuminousContourFront::try_new(5.0, 2.0, 1.0).unwrap()
            ])
            .unwrap();
        assert_rejected(
            Arc::<[Point]>::from(vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)]),
            3.0,
            Color::WHITE,
            narrow,
            LuminousContourError::FrontNarrowerThanCore,
        );
    }
}

// ============================================================================
// SPEC-HAL-159: Release-Grade Scene Text Ownership Tests
// ============================================================================

#[cfg(test)]
mod scene_text_ownership_tests {
    use super::*;

    // NOTE: test_push_text_unauthorized_panics and test_push_text_with_backgrounds_unauthorized_panics
    // moved to src/tests/scene_text_ownership.rs — caller path must be outside the allowlist
    // for #[track_caller] to report an unauthorized location.

    /// SPEC-HAL-159: Verify the allowlist logic correctly identifies authorized paths.
    ///
    /// This is a unit test for the path matching logic itself.
    #[test]
    fn test_authorization_path_matching() {
        // Paths that SHOULD be authorized
        let authorized_paths = [
            "src/platform/text_engine_glyphon.rs",
            "src/platform/backend_text_engine.rs",
            "src/platform/font_manager_text_engine.rs",
            "src/widgets/label.rs",
            "src/widgets/text_field.rs",
            "src/widgets/text_area.rs",
            "D:/Code/halcyon/src/widgets/button.rs",
            "D:\\Code\\halcyon\\src\\widgets\\list.rs",
            "src/core/scene/mod.rs",
            "D:\\Code\\halcyon\\src\\core\\scene\\mod.rs",
        ];

        for path in authorized_paths {
            assert!(
                SceneBuilder::is_authorized_scene_text_path(path),
                "Path should be authorized but wasn't: {}",
                path
            );
        }

        // Paths that SHOULD NOT be authorized
        let unauthorized_paths = [
            "src/core/scene.rs",
            "src/core/app.rs",
            "examples/renderer_demo.rs",
            "examples/renderer_path_rect_parity.rs",
            "tests/integration_test.rs",
            "src/main.rs",
        ];

        for path in unauthorized_paths {
            assert!(
                !SceneBuilder::is_authorized_scene_text_path(path),
                "Path should NOT be authorized but was: {}",
                path
            );
        }
    }

    #[test]
    fn repeated_nested_scene_reuses_node_vector_storage() {
        fn populate(builder: &mut SceneBuilder) {
            builder.push_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0, None);
            builder.begin_clip(RectF::new(0.0, 0.0, 100.0, 100.0), None);
            for x in [10.0, 20.0] {
                builder.push_rect(RectF::new(x, 0.0, 5.0, 5.0), Color::WHITE, 0, None);
            }
            builder.begin_scroll((0.0, 4.0));
            for x in [30.0, 40.0, 50.0] {
                builder.push_rect(RectF::new(x, 0.0, 5.0, 5.0), Color::WHITE, 0, None);
            }
            builder.end_scroll();
            builder.end_clip();
        }

        let mut first_builder = SceneBuilder::new();
        populate(&mut first_builder);
        assert_eq!(first_builder.node_vector_growths, 2);
        let first_scene = first_builder.build();
        let root_capacity = first_scene.root.capacity();

        let mut second_builder =
            SceneBuilder::with_recycled_storage(1.0, first_scene.into_recycled_storage());
        assert_eq!(second_builder.nodes.capacity(), root_capacity);
        assert_eq!(second_builder.recycled_node_vectors.len(), 2);
        populate(&mut second_builder);
        assert_eq!(second_builder.node_vector_growths, 0);

        let second_scene = second_builder.build();
        assert_eq!(second_scene.root.len(), 2);
        let SceneNode::Clip(clip) = &second_scene.root[1] else {
            panic!("second root node must preserve the clip hierarchy");
        };
        assert_eq!(clip.children.len(), 3);
        assert!(matches!(clip.children[2], SceneNode::Scroll(_)));
    }
}

// ============================================================================
// SPEC-HAL-083 Phase-2: Identity Enforcement Tests (debug-only)
// ============================================================================

#[cfg(test)]
mod scene_identity_tests {
    use super::*;
    use crate::core::scene_identity::SceneNodeKey;

    /// SPEC-HAL-083 Phase-2: Negative example - duplicate keys must panic.
    ///
    /// This test verifies the identity contract enforcement:
    /// emitting two nodes with the same SceneNodeKey in one frame must panic.
    ///
    /// This test only runs in debug builds (where the assertion is active).
    #[test]
    #[should_panic(expected = "SPEC-HAL-083: Duplicate SceneNodeKey")]
    #[cfg(debug_assertions)]
    fn test_duplicate_scene_key_panics() {
        let mut builder = SceneBuilder::new();
        let key = SceneNodeKey::new(12345);

        // First node with this key - OK
        builder.push_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0, Some(key));

        // Second node with SAME key - must panic
        builder.push_rect(
            RectF::new(20.0, 20.0, 10.0, 10.0),
            Color::WHITE,
            0,
            Some(key),
        );
    }

    /// SPEC-HAL-083: Ephemeral nodes (key == None) are always allowed.
    ///
    /// Multiple nodes with None keys should not panic - they're ephemeral.
    #[test]
    fn test_ephemeral_nodes_allowed() {
        let mut builder = SceneBuilder::new();

        // Multiple ephemeral nodes (None keys) - all OK
        builder.push_rect(RectF::new(0.0, 0.0, 10.0, 10.0), Color::WHITE, 0, None);
        builder.push_rect(RectF::new(10.0, 0.0, 10.0, 10.0), Color::WHITE, 0, None);
        builder.push_rect(RectF::new(20.0, 0.0, 10.0, 10.0), Color::WHITE, 0, None);

        // Should complete without panic
    }

    /// SPEC-HAL-083: Different keys are allowed.
    ///
    /// Each node can have a unique key without conflict.
    #[test]
    fn test_unique_keys_allowed() {
        let mut builder = SceneBuilder::new();

        // Different keys - all OK
        builder.push_rect(
            RectF::new(0.0, 0.0, 10.0, 10.0),
            Color::WHITE,
            0,
            Some(SceneNodeKey::new(1)),
        );
        builder.push_rect(
            RectF::new(10.0, 0.0, 10.0, 10.0),
            Color::WHITE,
            0,
            Some(SceneNodeKey::new(2)),
        );
        builder.push_rect(
            RectF::new(20.0, 0.0, 10.0, 10.0),
            Color::WHITE,
            0,
            Some(SceneNodeKey::new(3)),
        );

        // Should complete without panic
    }

    #[test]
    fn test_find_node_by_key_recurses_into_containers() {
        let clip_key = SceneNodeKey::new(1);
        let layer_key = SceneNodeKey::new(2);
        let scroll_key = SceneNodeKey::new(3);
        let target_key = SceneNodeKey::new(4);

        let target = SceneNode::Rect(RectNode {
            rect: RectF::new(0.0, 0.0, 10.0, 10.0),
            color: Color::WHITE,
            z_index: 0,
            key: Some(target_key),
        });

        let scroll = SceneNode::Scroll(ScrollNode {
            offset: (0.0, 0.0),
            children: vec![target],
            key: Some(scroll_key),
        });

        let layer = SceneNode::Layer(LayerNode {
            rect: RectF::new(0.0, 0.0, 10.0, 10.0),
            children: vec![scroll],
            opacity: 1.0,
            z_index: 0,
            key: Some(layer_key),
        });

        let clip = SceneNode::Clip(ClipNode {
            rect: RectF::new(0.0, 0.0, 10.0, 10.0),
            rounded: None,
            children: vec![layer],
            z_index: 0,
            key: Some(clip_key),
        });

        let mut scene = SceneGraph::new();
        scene.root = vec![clip];

        assert_eq!(
            scene.find_node_by_key(clip_key).map(SceneNode::kind),
            Some(SceneNodeKind::Clip)
        );
        assert_eq!(
            scene.find_node_by_key(layer_key).map(SceneNode::kind),
            Some(SceneNodeKind::Layer)
        );
        assert_eq!(
            scene.find_node_by_key(scroll_key).map(SceneNode::kind),
            Some(SceneNodeKind::Scroll)
        );
        assert_eq!(
            scene.find_node_by_key(target_key).map(SceneNode::kind),
            Some(SceneNodeKind::Rect)
        );
        assert!(scene.find_node_by_key(SceneNodeKey::new(99)).is_none());
    }
}
