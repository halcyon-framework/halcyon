// SPEC-HAL-086 Stage 1: Unified Scroll Container Model
// SPEC-HAL-086 Stage 3: Dual-offset state machine (target vs visual)
// SPEC-HAL-086 Stage 4: Unified scroll input pipeline (wheel + drag)
// SPEC-HAL-086 Stage 6: Frame-driven drag visual updates
// SPEC-HAL-086 Stage 12: Both modes use smoothing (different lerp factors)
//
// Core scroll state container used by all scrollable widgets.
// Decouples scroll logic from widget implementation.
//
// Visual Update Pipeline (Stage 12):
// - Wheel input: updates target_offset via scroll_by()
// - Drag input: push_drag_raw_*() sets absolute scroll position to target
// - Frame loop calls update_visual() every frame
// - Normal mode: visual smoothly interpolates toward target (NORMAL_LERP = 0.25)
// - Drag mode: visual smoothly interpolates toward target (DRAG_LERP = 0.5)
//
// Stage 12 insight: Both wheel and drag go through frame-driven smoothing.
// This decouples visual updates from input event frequency, ensuring smooth
// motion regardless of PointerMove rate. Drag uses a snappier lerp factor
// for responsive thumb tracking while still getting frame-rate interpolation.

use crate::core::math::Size;

/// SPEC-HAL-086 Stage 4: Scroll input mode determines smoothing behavior.
///
/// - `Normal`: Wheel/programmatic scroll uses exponential smoothing
/// - `Drag`: Scrollbar thumb drag bypasses smoothing for pixel-perfect tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollInputMode {
    #[default]
    Normal,
    Drag,
}

/// Unified scroll container state.
///
/// All scrollable widgets own a `ScrollContainer` to manage:
/// - content_size: total size of scrollable content
/// - viewport_size: visible viewport dimensions
/// - scroll_offset_target: logical scroll position (precise, clamped)
/// - scroll_offset_visual: smoothed scroll position (for rendering)
///
/// SPEC-HAL-086 Stage 3: Separates target (logic) from visual (rendering).
/// The target offset is the true scroll position after user input.
/// The visual offset smoothly interpolates toward target for animation.
///
/// SPEC-HAL-086 Stage 4: Unified input pipeline. All scroll sources go through
/// target_offset -> smoothing/integration -> visual_offset. The input_mode
/// determines whether smoothing is applied (Normal) or bypassed (Drag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollContainer {
    /// Total size of the scrollable content
    pub content_size: Size,
    /// Size of the visible viewport
    pub viewport_size: Size,
    /// Target scroll offset (logical, clamped). Always in range [0, max_scroll]
    scroll_offset_target: (f32, f32),
    /// Visual scroll offset (smoothed for rendering)
    scroll_offset_visual: (f32, f32),
    /// Smoothing factor for visual interpolation (0.0 = no smoothing, 1.0 = instant)
    pub smoothing_factor: f32,
    /// SPEC-HAL-086 Stage 4: Current input mode (Normal = smooth, Drag = instant)
    input_mode: ScrollInputMode,
    /// SPEC-HAL-086 Stage 7: Pending drag scroll position (X axis).
    /// Set by PointerMove during drag, consumed by update_visual().
    /// None = no pending update, Some(x) = scroll to this X position.
    drag_pending_x: Option<f32>,
    /// SPEC-HAL-086 Stage 7: Pending drag scroll position (Y axis).
    drag_pending_y: Option<f32>,
    /// SPEC-HAL-086 Stage 8: Low-pass filter alpha for drag input samples.
    /// 0.0 = frozen (no movement), 1.0 = no filtering (raw samples pass through).
    /// Default 0.35 provides mild smoothing similar to wheel scrolling feel.
    drag_input_alpha: f32,
    /// SPEC-HAL-086 Stage 12: Whether we've had active drag input this session.
    /// Used to clear pending state on end_drag().
    has_drag_raw: bool,
    /// SPEC-HAL-268: Force needs_animation to return true (proximity scroll active)
    force_animate: bool,
}

impl Default for ScrollContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Lerp factor for wheel/programmatic scroll (Normal mode)
const NORMAL_LERP: f32 = 0.25;
/// Lerp factor for drag scroll (snappier to track thumb)
const DRAG_LERP: f32 = 0.5;

impl ScrollContainer {
    /// Default smoothing factor for scroll animation
    const DEFAULT_SMOOTHING: f32 = 0.25;

    /// SPEC-HAL-086 Stage 8: Default alpha for drag input low-pass filter.
    /// 0.35 provides mild smoothing that makes drag feel similar to wheel.
    const DEFAULT_DRAG_INPUT_ALPHA: f32 = 0.35;

    /// Create a new scroll container with zero sizes and offset.
    pub const fn new() -> Self {
        Self {
            content_size: Size::zero(),
            viewport_size: Size::zero(),
            scroll_offset_target: (0.0, 0.0),
            scroll_offset_visual: (0.0, 0.0),
            smoothing_factor: Self::DEFAULT_SMOOTHING,
            input_mode: ScrollInputMode::Normal,
            drag_pending_x: None,
            drag_pending_y: None,
            drag_input_alpha: Self::DEFAULT_DRAG_INPUT_ALPHA,
            has_drag_raw: false,
            force_animate: false,
        }
    }

    /// Set the content size (total scrollable area).
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
    }

    /// Set the viewport size (visible area).
    pub fn set_viewport_size(&mut self, size: Size) {
        self.viewport_size = size;
    }

    /// Clamp both target and visual offsets to current valid range.
    ///
    /// Stage 2.5: Scroll Metrics Revalidation on Resize.
    ///
    /// This method is specifically for resize scenarios where:
    /// - Viewport or content size has changed
    /// - max_scroll may have decreased
    /// - Both target and visual offsets may now exceed the valid range
    ///
    /// Unlike update_visual() which lerps visual toward target, this method
    /// clamps visual immediately. This prevents the scrollbar from showing
    /// an invalid position until the next frame update.
    ///
    /// Call this after setting new viewport/content sizes when scroll
    /// position must be validated immediately (before paint).
    #[inline]
    pub fn clamp_offsets_on_resize(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset_target.0 = self.scroll_offset_target.0.clamp(0.0, max.0);
        self.scroll_offset_target.1 = self.scroll_offset_target.1.clamp(0.0, max.1);
        self.scroll_offset_visual.0 = self.scroll_offset_visual.0.clamp(0.0, max.0);
        self.scroll_offset_visual.1 = self.scroll_offset_visual.1.clamp(0.0, max.1);
    }

    /// Maximum scroll offset in each axis.
    /// Returns (max_x, max_y) where each is (content_size - viewport_size).max(0).
    pub fn max_scroll(&self) -> (f32, f32) {
        (
            (self.content_size.width - self.viewport_size.width).max(0.0),
            (self.content_size.height - self.viewport_size.height).max(0.0),
        )
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 3: Dual-offset API
    // =========================================================================

    /// Get the target scroll offset (logical position).
    pub fn target_offset(&self) -> (f32, f32) {
        self.scroll_offset_target
    }

    /// Get the visual scroll offset (smoothed for rendering).
    pub fn visual_offset(&self) -> (f32, f32) {
        self.scroll_offset_visual
    }

    /// Set the target scroll offset, clamping to valid range [0, max_scroll].
    /// Does NOT modify visual offset - call `update_visual()` to animate.
    pub fn set_target_offset(&mut self, offset: (f32, f32)) {
        let max = self.max_scroll();
        self.scroll_offset_target = (offset.0.clamp(0.0, max.0), offset.1.clamp(0.0, max.1));
    }

    /// Jump to offset immediately, setting both target AND visual.
    /// Use this for programmatic scrolls that should not animate.
    pub fn jump_to_offset(&mut self, offset: (f32, f32)) {
        let max = self.max_scroll();
        let clamped = (offset.0.clamp(0.0, max.0), offset.1.clamp(0.0, max.1));
        self.scroll_offset_target = clamped;
        self.scroll_offset_visual = clamped;
    }

    /// SPEC-HAL-366: Settle X without interrupting independent Y convergence.
    pub(crate) fn jump_to_offset_x(&mut self, x: f32) {
        let max = self.max_scroll();
        let x = x.clamp(0.0, max.0);
        self.scroll_offset_target.0 = x;
        self.scroll_offset_visual.0 = x;
    }

    /// Jump only the vertical offset, preserving horizontal target and visual state.
    ///
    /// SPEC-HAL-352: Layout-change anchoring settles vertical compensation
    /// immediately without interrupting independent horizontal convergence.
    pub(crate) fn jump_to_offset_y(&mut self, y: f32) {
        let max = self.max_scroll();
        let y = y.clamp(0.0, max.1);
        self.scroll_offset_target.1 = y;
        self.scroll_offset_visual.1 = y;
    }

    /// Clamp target offset to current max_scroll bounds.
    /// Call after content/viewport size changes to ensure target stays valid.
    pub fn clamp_target(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset_target = (
            self.scroll_offset_target.0.clamp(0.0, max.0),
            self.scroll_offset_target.1.clamp(0.0, max.1),
        );
    }

    /// Update visual offset toward target using exponential smoothing.
    ///
    /// SPEC-HAL-086 Stage 3: Call this each frame (typically in paint).
    /// SPEC-HAL-086 Stage 4: Mode-aware smoothing.
    /// SPEC-HAL-086 Stage 6: Frame-driven visual updates (render loop calls this, not events).
    /// SPEC-HAL-086 Stage 12: Both modes use smoothing, with different lerp factors.
    /// - Normal mode: NORMAL_LERP (0.25) for wheel/programmatic scroll
    /// - Drag mode: DRAG_LERP (0.5) for snappier thumb tracking
    ///
    /// The `_dt` parameter is reserved for future frame-time-aware smoothing.
    pub fn update_visual(&mut self, _dt: f32) {
        // SPEC-HAL-146: Assert Update-phase authority before visual state mutation
        crate::core::rect_authority::assert_update_mutation_authorized(
            "ScrollContainer::update_visual: scroll_offset_visual",
        );

        match self.input_mode {
            ScrollInputMode::Drag => {
                // Drag mode: target is driven by thumb, but visual is still smoothed
                // to decouple from PointerMove frequency.
                self.clamp_target();

                let (vx, vy) = self.scroll_offset_visual;
                let (tx, ty) = self.scroll_offset_target;

                self.scroll_offset_visual.0 = vx + (tx - vx) * DRAG_LERP;
                self.scroll_offset_visual.1 = vy + (ty - vy) * DRAG_LERP;

                // Snap to target when close enough.
                if (self.scroll_offset_visual.0 - tx).abs() < 0.5 {
                    self.scroll_offset_visual.0 = tx;
                }
                if (self.scroll_offset_visual.1 - ty).abs() < 0.5 {
                    self.scroll_offset_visual.1 = ty;
                }
            }
            ScrollInputMode::Normal => {
                // Normal mode: existing exponential smoothing for wheel/programmatic scroll.
                self.clamp_target();

                let (vx, vy) = self.scroll_offset_visual;
                let (tx, ty) = self.scroll_offset_target;

                self.scroll_offset_visual.0 = vx + (tx - vx) * NORMAL_LERP;
                self.scroll_offset_visual.1 = vy + (ty - vy) * NORMAL_LERP;

                // Snap to target when close enough — eliminates sub-pixel jitter tail.
                // Below 0.5 logical pixels, no visual difference is perceptible.
                if (self.scroll_offset_visual.0 - tx).abs() < 0.5 {
                    self.scroll_offset_visual.0 = tx;
                }
                if (self.scroll_offset_visual.1 - ty).abs() < 0.5 {
                    self.scroll_offset_visual.1 = ty;
                }
            }
        }
    }

    /// Sync visual offset to target immediately (no animation).
    /// Use after initial layout or when visual must match target exactly.
    pub fn sync_visual_to_target(&mut self) {
        self.scroll_offset_visual = self.scroll_offset_target;
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 4: Input Mode API
    // =========================================================================

    /// Get the current input mode.
    pub fn input_mode(&self) -> ScrollInputMode {
        self.input_mode
    }

    /// Returns true if this scroll container needs continuous frame updates.
    /// - Drag mode: always true (decouple from PointerMove frequency)
    /// - Normal mode: true if visual is catching up to target (inertia)
    pub fn needs_animation(&self) -> bool {
        if self.force_animate {
            return true;
        }
        match self.input_mode {
            ScrollInputMode::Drag => true,
            ScrollInputMode::Normal => {
                let (tx, ty) = self.scroll_offset_target;
                let (vx, vy) = self.scroll_offset_visual;
                (tx - vx).abs() > 0.1 || (ty - vy).abs() > 0.1
            }
        }
    }

    /// SPEC-HAL-268: Set force-animate flag for proximity scroll
    pub fn set_force_animate(&mut self, force: bool) {
        self.force_animate = force;
    }

    /// Begin drag mode: uses DRAG_LERP smoothing for responsive thumb tracking.
    ///
    /// Call this when scrollbar thumb drag starts.
    /// Syncs visual to target immediately to avoid grab jump.
    pub fn begin_drag(&mut self) {
        self.input_mode = ScrollInputMode::Drag;
        // Sync visual to target immediately on drag start.
        self.scroll_offset_visual = self.scroll_offset_target;
    }

    /// End drag mode: switches back to NORMAL_LERP smoothing.
    ///
    /// Call this when scrollbar thumb drag ends.
    pub fn end_drag(&mut self) {
        self.input_mode = ScrollInputMode::Normal;
        // SPEC-HAL-086 Stage 7: Clear any pending drag values
        self.drag_pending_x = None;
        self.drag_pending_y = None;
        self.has_drag_raw = false;
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 7+8: Pending Drag Input API
    // =========================================================================

    /// Set pending drag scroll position for X axis (internal helper).
    ///
    /// SPEC-HAL-086 Stage 7: Internal helper, consumed by `update_visual()`.
    /// SPEC-HAL-086 Stage 8: Widgets should use `push_drag_sample_x()` instead.
    fn set_pending_drag_x(&mut self, scroll_x: f32) {
        self.drag_pending_x = Some(scroll_x);
    }

    /// Set pending drag scroll position for Y axis (internal helper).
    ///
    /// SPEC-HAL-086 Stage 7: Internal helper, consumed by `update_visual()`.
    /// SPEC-HAL-086 Stage 8: Widgets should use `push_drag_sample_y()` instead.
    fn set_pending_drag_y(&mut self, scroll_y: f32) {
        self.drag_pending_y = Some(scroll_y);
    }

    /// Push a raw drag scroll sample for X axis with low-pass filtering.
    ///
    /// SPEC-HAL-086 Stage 8: Primary API for scrollbar drag input.
    /// Applies low-pass filter: `filtered = base + (raw - base) * drag_input_alpha`
    /// where base is the previous pending value (or current target if none).
    ///
    /// Only active in Drag mode. Does nothing in Normal mode.
    /// The filtered value is stored as pending and consumed by `update_visual()`.
    /// No clamping here - clamping happens in `set_target_offset_x()`.
    pub fn push_drag_sample_x(&mut self, raw_scroll: f32) {
        if self.input_mode != ScrollInputMode::Drag {
            return;
        }
        let base = self.drag_pending_x.unwrap_or(self.scroll_offset_target.0);
        let filtered = base + (raw_scroll - base) * self.drag_input_alpha;
        self.set_pending_drag_x(filtered);
    }

    /// Push a raw drag scroll sample for Y axis with low-pass filtering.
    ///
    /// SPEC-HAL-086 Stage 8: Primary API for scrollbar drag input.
    /// See `push_drag_sample_x()` for details.
    pub fn push_drag_sample_y(&mut self, raw_scroll: f32) {
        if self.input_mode != ScrollInputMode::Drag {
            return;
        }
        let base = self.drag_pending_y.unwrap_or(self.scroll_offset_target.1);
        let filtered = base + (raw_scroll - base) * self.drag_input_alpha;
        self.set_pending_drag_y(filtered);
    }

    /// Set the drag input alpha (low-pass filter strength).
    ///
    /// SPEC-HAL-086 Stage 8: Controls how aggressively drag samples are filtered.
    /// - 0.0 = frozen (no movement)
    /// - 1.0 = no filtering (raw samples pass through, Stage 7 behavior)
    /// - 0.3-0.4 = mild smoothing (feels similar to wheel scrolling)
    #[allow(dead_code)]
    pub fn set_drag_input_alpha(&mut self, alpha: f32) {
        self.drag_input_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Get the current drag input alpha.
    #[allow(dead_code)]
    pub fn drag_input_alpha(&self) -> f32 {
        self.drag_input_alpha
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 9: Velocity-Based Drag Input API
    // =========================================================================

    /// Push a raw drag scroll position for Y axis.
    ///
    /// SPEC-HAL-086 Stage 9 (revised): For direct thumb drag, `raw` is the absolute
    /// desired scroll position in content units. Sets target only; visual will be
    /// interpolated in update_visual() using DRAG_LERP.
    ///
    /// Only active in Drag mode. Does nothing in Normal mode.
    pub fn push_drag_raw_y(&mut self, raw: f32) {
        if self.input_mode != ScrollInputMode::Drag {
            return;
        }
        // Drag mode: raw is absolute desired scroll position.
        // Only update target; visual will be interpolated in update_visual().
        self.set_target_offset_y(raw);
    }

    /// Push a raw drag scroll position for X axis.
    ///
    /// SPEC-HAL-086 Stage 9 (revised): See `push_drag_raw_y()` for details.
    pub fn push_drag_raw_x(&mut self, raw: f32) {
        if self.input_mode != ScrollInputMode::Drag {
            return;
        }
        self.set_target_offset_x(raw);
    }

    /// Scroll by a delta amount, clamping to valid range.
    ///
    /// Use for wheel input and programmatic incremental scrolls.
    /// This modifies the target offset; call `update_visual()` to animate.
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        let (x, y) = self.scroll_offset_target;
        self.set_target_offset((x + dx, y + dy));
    }

    /// Set only the X component of target offset, clamping to valid range.
    ///
    /// Use for horizontal scrollbar drag.
    pub fn set_target_offset_x(&mut self, x: f32) {
        let max = self.max_scroll();
        self.scroll_offset_target.0 = x.clamp(0.0, max.0);
    }

    /// Set only the Y component of target offset, clamping to valid range.
    ///
    /// Use for vertical scrollbar drag.
    pub fn set_target_offset_y(&mut self, y: f32) {
        let max = self.max_scroll();
        self.scroll_offset_target.1 = y.clamp(0.0, max.1);
    }

    // =========================================================================
    // Backwards compatibility (deprecated, will be removed)
    // =========================================================================

    /// DEPRECATED: Use `set_target_offset` instead.
    /// Set the scroll offset, clamping to valid range [0, max_scroll].
    #[deprecated(since = "0.1.3", note = "Use set_target_offset instead")]
    pub fn set_scroll_offset(&mut self, offset: (f32, f32)) {
        self.set_target_offset(offset);
    }

    /// DEPRECATED: Direct field access. Use `target_offset()` or `visual_offset()`.
    /// Returns the target offset for backwards compatibility.
    #[deprecated(
        since = "0.1.3",
        note = "Use target_offset() or visual_offset() instead"
    )]
    pub fn scroll_offset(&self) -> (f32, f32) {
        self.scroll_offset_target
    }
}

// ============================================================================
// Horizontal Scroll Link
// ============================================================================
//
// SPEC-HAL-086 Stage N: Explicit scroll offset linkage for external alignment.
//
// Problem: Header/footer are outside ScrollArea but must align with body scroll.
// Solution: Shared link that ScrollArea writes and header/footer read.
//
// INV-HSCROLL-LINK-1: ScrollArea is the ONLY writer.
// INV-HSCROLL-LINK-2: Readers (header/footer) must not write.
// INV-HSCROLL-LINK-3: The engine publishes the ScrollArea's presented coordinate.

use std::cell::Cell;
use std::rc::Rc;

/// Shared horizontal scroll offset for external alignment.
///
/// ScrollArea writes the current visual horizontal offset.
/// Header/footer widgets read it during paint to apply the same X translation.
/// Ordinary horizontal attachments are live-only. SPEC-HAL-366's explicitly
/// retained attachments also restore X once across sequential ScrollArea lifetimes.
///
/// # Example
///
/// ```ignore
/// // Setup: Create link and wire to ScrollArea and header
/// let link = HorizontalScrollLink::new();
/// scroll_area.set_horizontal_scroll_link(link.clone());
/// header.set_horizontal_scroll_link(link.clone());
///
/// // The engine publishes the ScrollArea's presented coordinate
/// link.set(visual_offset_x);
///
/// // Header reads during paint
/// let scroll_x = link.get();
/// let header_x = rect.x - scroll_x;
/// ```
#[derive(Debug, Clone, Default)]
pub struct HorizontalScrollLink {
    offset: Rc<Cell<f32>>,
}

impl HorizontalScrollLink {
    /// Create a new horizontal scroll link with offset 0.
    pub fn new() -> Self {
        Self {
            offset: Rc::new(Cell::new(0.0)),
        }
    }

    /// Get the current horizontal scroll offset.
    ///
    /// INV-HSCROLL-LINK-2: This is the read API for header/footer.
    #[inline]
    pub fn get(&self) -> f32 {
        self.offset.get()
    }

    /// Set the horizontal scroll offset.
    ///
    /// INV-HSCROLL-LINK-1: Only ScrollArea should call this.
    #[inline]
    pub fn set(&self, offset: f32) {
        self.offset.set(offset);
    }
}

/// Retained vertical scroll coordinate for sequential `ScrollArea` lifetimes.
///
/// Unlike the live-alignment use of [`HorizontalScrollLink`], this link does not
/// continuously align two live widgets. One `ScrollArea` writes the presented coordinate, and a
/// replacement `ScrollArea` consumes it once after its first tight layout has
/// established the new scroll bounds.
#[derive(Debug, Clone, Default)]
pub struct VerticalScrollLink {
    offset: Rc<Cell<f32>>,
}

impl VerticalScrollLink {
    /// Create a retained vertical scroll link at offset zero.
    pub fn new() -> Self {
        Self {
            offset: Rc::new(Cell::new(0.0)),
        }
    }

    #[inline]
    pub(crate) fn retained_offset(&self) -> f32 {
        self.offset.get()
    }

    #[inline]
    pub(crate) fn publish(&self, offset: f32) {
        self.offset.set(offset.max(0.0));
    }
}

// ============================================================================
// Test Harness (SPEC-HAL-109 Stage D)
// ============================================================================

#[cfg(any(test, feature = "test-harness"))]
impl ScrollContainer {
    /// Set visual offset directly for testing Stage D paint invariants.
    /// Allows testing visual vs target separation without animation.
    pub fn set_visual_offset_for_test(&mut self, offset: (f32, f32)) {
        self.scroll_offset_visual = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rect_authority::UpdateAuthorityGuard;

    #[test]
    fn test_new() {
        let sc = ScrollContainer::new();
        assert_eq!(sc.content_size, Size::zero());
        assert_eq!(sc.viewport_size, Size::zero());
        assert_eq!(sc.target_offset(), (0.0, 0.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_max_scroll() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        let max = sc.max_scroll();
        assert_eq!(max.0, 300.0); // 500 - 200
        assert_eq!(max.1, 700.0); // 1000 - 300
    }

    #[test]
    fn test_max_scroll_no_overflow() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(100.0, 100.0));
        sc.set_viewport_size(Size::new(200.0, 200.0));

        let max = sc.max_scroll();
        assert_eq!(max.0, 0.0); // Content smaller than viewport
        assert_eq!(max.1, 0.0);
    }

    #[test]
    fn test_set_target_offset_clamps() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Within bounds
        sc.set_target_offset((100.0, 200.0));
        assert_eq!(sc.target_offset(), (100.0, 200.0));

        // Clamp to max
        sc.set_target_offset((1000.0, 2000.0));
        assert_eq!(sc.target_offset(), (300.0, 700.0)); // Clamped to max

        // Clamp to min (0)
        sc.set_target_offset((-100.0, -200.0));
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_jump_to_offset() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.jump_to_offset((100.0, 200.0));
        assert_eq!(sc.target_offset(), (100.0, 200.0));
        assert_eq!(sc.visual_offset(), (100.0, 200.0)); // Both set
    }

    #[test]
    fn test_update_visual_smoothing() {
        let _guard = UpdateAuthorityGuard::enter();
        // SPEC-HAL-086 Stage 12: Normal mode uses NORMAL_LERP = 0.25
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Set target but not visual
        sc.set_target_offset((100.0, 200.0));
        assert_eq!(sc.target_offset(), (100.0, 200.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0)); // Still at origin

        // Update visual (NORMAL_LERP = 0.25 toward target)
        sc.update_visual(1.0);
        // 0 + (100 - 0) * 0.25 = 25
        assert_eq!(sc.visual_offset(), (25.0, 50.0));

        // Update again
        sc.update_visual(1.0);
        // 25 + (100 - 25) * 0.25 = 25 + 18.75 = 43.75
        assert_eq!(sc.visual_offset(), (43.75, 87.5));
    }

    #[test]
    fn test_clamp_target() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Set target to max
        sc.set_target_offset((300.0, 700.0));

        // Shrink content, making max_scroll smaller
        sc.set_content_size(Size::new(300.0, 500.0));
        // max is now (100, 200)

        // Clamp should reduce target to new max
        sc.clamp_target();
        assert_eq!(sc.target_offset(), (100.0, 200.0));
    }

    #[test]
    fn test_sync_visual_to_target() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.set_target_offset((100.0, 200.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0)); // Visual still at origin

        sc.sync_visual_to_target();
        assert_eq!(sc.visual_offset(), (100.0, 200.0)); // Now synced
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 4: Input Mode Tests
    // =========================================================================

    #[test]
    fn test_default_mode_is_normal() {
        let sc = ScrollContainer::new();
        assert_eq!(sc.input_mode(), ScrollInputMode::Normal);
    }

    #[test]
    fn test_begin_end_drag() {
        let mut sc = ScrollContainer::new();
        assert_eq!(sc.input_mode(), ScrollInputMode::Normal);

        sc.begin_drag();
        assert_eq!(sc.input_mode(), ScrollInputMode::Drag);

        sc.end_drag();
        assert_eq!(sc.input_mode(), ScrollInputMode::Normal);
    }

    #[test]
    fn test_drag_mode_smoothed_visual() {
        let _guard = UpdateAuthorityGuard::enter();
        // SPEC-HAL-086 Stage 12: Drag mode uses DRAG_LERP (0.5) smoothing
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Normal mode: uses NORMAL_LERP (0.25)
        sc.set_target_offset((100.0, 200.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0));
        sc.update_visual(1.0);
        // NORMAL_LERP = 0.25: 0 + (100 - 0) * 0.25 = 25
        assert_eq!(sc.visual_offset(), (25.0, 50.0));

        // Switch to drag mode: begin_drag syncs visual to target
        sc.begin_drag();
        // Visual synced to target (100, 200) on begin_drag
        assert_eq!(sc.visual_offset(), (100.0, 200.0));

        // Set new target and update: uses DRAG_LERP (0.5)
        sc.set_target_offset((200.0, 400.0));
        sc.update_visual(1.0);
        // DRAG_LERP = 0.5: 100 + (200 - 100) * 0.5 = 150
        assert_eq!(sc.visual_offset(), (150.0, 300.0));

        // End drag, back to NORMAL_LERP smoothing
        sc.end_drag();
        sc.set_target_offset((100.0, 200.0));
        sc.update_visual(1.0);
        // NORMAL_LERP = 0.25: 150 + (100 - 150) * 0.25 = 137.5
        assert_eq!(sc.visual_offset(), (137.5, 275.0));
    }

    #[test]
    fn test_scroll_by() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.set_target_offset((100.0, 200.0));

        // Scroll by delta
        sc.scroll_by(50.0, 100.0);
        assert_eq!(sc.target_offset(), (150.0, 300.0));

        // Scroll with negative delta
        sc.scroll_by(-25.0, -50.0);
        assert_eq!(sc.target_offset(), (125.0, 250.0));

        // Scroll beyond max (should clamp)
        sc.scroll_by(1000.0, 1000.0);
        assert_eq!(sc.target_offset(), (300.0, 700.0)); // Clamped to max
    }

    #[test]
    fn test_set_target_offset_x() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.set_target_offset((100.0, 200.0));
        sc.set_target_offset_x(50.0);
        assert_eq!(sc.target_offset(), (50.0, 200.0)); // Only X changed

        // Clamp to max
        sc.set_target_offset_x(1000.0);
        assert_eq!(sc.target_offset(), (300.0, 200.0));

        // Clamp to min
        sc.set_target_offset_x(-100.0);
        assert_eq!(sc.target_offset(), (0.0, 200.0));
    }

    #[test]
    fn test_set_target_offset_y() {
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.set_target_offset((100.0, 200.0));
        sc.set_target_offset_y(350.0);
        assert_eq!(sc.target_offset(), (100.0, 350.0)); // Only Y changed

        // Clamp to max
        sc.set_target_offset_y(2000.0);
        assert_eq!(sc.target_offset(), (100.0, 700.0));

        // Clamp to min
        sc.set_target_offset_y(-100.0);
        assert_eq!(sc.target_offset(), (100.0, 0.0));
    }

    #[test]
    fn test_wheel_drag_wheel_transition() {
        let _guard = UpdateAuthorityGuard::enter();
        // SPEC-HAL-086 Stage 12: Both wheel and drag use smoothing (different factors)
        // Simulates: wheel scroll -> thumb drag -> wheel scroll
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // 1. Wheel scroll (Normal mode, NORMAL_LERP = 0.25)
        assert_eq!(sc.input_mode(), ScrollInputMode::Normal);
        sc.scroll_by(0.0, 100.0);
        assert_eq!(sc.target_offset(), (0.0, 100.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0)); // Visual hasn't caught up
        sc.update_visual(1.0);
        // NORMAL_LERP = 0.25: 0 + 100 * 0.25 = 25
        assert_eq!(sc.visual_offset(), (0.0, 25.0));

        // 2. Start thumb drag - begin_drag syncs visual to target
        sc.begin_drag();
        assert_eq!(sc.input_mode(), ScrollInputMode::Drag);
        assert_eq!(sc.visual_offset(), (0.0, 100.0)); // Synced on begin_drag

        // Drag uses DRAG_LERP = 0.5
        sc.set_target_offset_y(300.0);
        sc.update_visual(1.0);
        // DRAG_LERP = 0.5: 100 + (300 - 100) * 0.5 = 200
        assert_eq!(sc.visual_offset(), (0.0, 200.0));

        // Drag some more
        sc.set_target_offset_y(400.0);
        sc.update_visual(1.0);
        // DRAG_LERP = 0.5: 200 + (400 - 200) * 0.5 = 300
        assert_eq!(sc.visual_offset(), (0.0, 300.0));

        // 3. End drag
        sc.end_drag();
        assert_eq!(sc.input_mode(), ScrollInputMode::Normal);
        // Position preserved: target = 400, visual = 300
        assert_eq!(sc.target_offset(), (0.0, 400.0));
        assert_eq!(sc.visual_offset(), (0.0, 300.0));

        // 4. Wheel scroll again (back to NORMAL_LERP = 0.25)
        sc.scroll_by(0.0, -100.0); // Scroll up: target = 400 - 100 = 300
        assert_eq!(sc.target_offset(), (0.0, 300.0));
        sc.update_visual(1.0);
        // NORMAL_LERP = 0.25: 300 + (300 - 300) * 0.25 = 300 (already at target)
        assert_eq!(sc.visual_offset(), (0.0, 300.0));
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 7+8: Pending Drag Input Tests (DEPRECATED)
    // Stage 9 replaced pending-based mechanism with velocity integration.
    // These tests verify the deprecated API still works for backward compat.
    // =========================================================================

    #[test]
    fn test_pending_drag_api_exists() {
        let _guard = UpdateAuthorityGuard::enter();
        // Stage 9 deprecated the pending API but it still exists.
        // It just doesn't get consumed by update_visual anymore.
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.begin_drag();
        sc.set_pending_drag_y(200.0);
        sc.set_pending_drag_x(100.0);

        // Pending values are set but Stage 9 uses velocity integration instead
        // So update_visual integrates velocity (which is 0) rather than consuming pending
        sc.update_visual(1.0);

        // With Stage 9 velocity model, target stays at 0 since no velocity was set
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_pending_drag_cleared_on_end_drag() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.begin_drag();
        sc.set_pending_drag_y(200.0);

        // End drag clears pending (and velocity)
        sc.end_drag();

        // Now switch back to drag mode and call update_visual
        sc.begin_drag();
        sc.update_visual(1.0);

        // Target should still be at origin (velocity was cleared)
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_pending_drag_not_consumed_in_normal_mode() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Set pending while in Normal mode (shouldn't happen, but test robustness)
        sc.set_pending_drag_y(200.0);

        sc.update_visual(1.0);

        // Target unchanged (Normal mode uses smoothing toward target)
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_push_drag_sample_api_exists() {
        let _guard = UpdateAuthorityGuard::enter();
        // Stage 8 push_drag_sample_* API still exists for backward compat
        // but update_visual uses velocity integration (Stage 9) instead
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));
        sc.set_drag_input_alpha(0.5);

        sc.begin_drag();
        sc.push_drag_sample_y(200.0);

        sc.update_visual(1.0);
        // With Stage 9, pending is not consumed; velocity is 0
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }

    #[test]
    fn test_push_drag_sample_ignored_in_normal_mode() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Stay in Normal mode (don't call begin_drag)
        sc.push_drag_sample_y(200.0);

        sc.update_visual(1.0);
        assert_eq!(sc.target_offset(), (0.0, 0.0)); // Ignored, no change
    }

    // =========================================================================
    // SPEC-HAL-086 Stage 12: Drag smoothing tests
    // - push_drag_raw_* sets target directly
    // - update_visual() interpolates visual toward target with DRAG_LERP
    // =========================================================================

    #[test]
    fn test_push_drag_raw_sets_target_only() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.begin_drag();
        assert_eq!(sc.target_offset(), (0.0, 0.0));
        assert_eq!(sc.visual_offset(), (0.0, 0.0));

        sc.push_drag_raw_y(100.0);

        // Target jumps to requested value
        assert_eq!(sc.target_offset(), (0.0, 100.0));
        // Visual still old until update_visual runs
        assert_eq!(sc.visual_offset(), (0.0, 0.0));

        sc.update_visual(1.0);
        // DRAG_LERP = 0.5 → halfway toward 100
        assert_eq!(sc.visual_offset(), (0.0, 50.0));
    }

    #[test]
    fn test_drag_smoothing_multiple_frames() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.begin_drag();
        sc.push_drag_raw_y(100.0);

        // frame 1: visual 0 -> 50
        sc.update_visual(1.0);
        assert_eq!(sc.visual_offset(), (0.0, 50.0));

        // frame 2: visual 50 -> 75
        sc.update_visual(1.0);
        assert_eq!(sc.visual_offset(), (0.0, 75.0));

        // frame 3: visual 75 -> 87.5
        sc.update_visual(1.0);
        assert_eq!(sc.visual_offset(), (0.0, 87.5));
    }

    #[test]
    fn test_drag_clamping_still_applies() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));
        // max_scroll_y = 700

        sc.begin_drag();
        sc.push_drag_raw_y(1000.0); // beyond max

        sc.update_visual(1.0);
        // target is clamped to 700 internally
        let (tx, ty) = sc.target_offset();
        assert_eq!(tx, 0.0);
        assert_eq!(ty, 700.0);

        // visual moved halfway toward 700 from 0
        let (vx, vy) = sc.visual_offset();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 350.0);
    }

    #[test]
    fn test_end_drag_preserves_position() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        sc.begin_drag();
        sc.push_drag_raw_y(100.0);
        sc.update_visual(1.0); // 0 -> 50

        sc.end_drag();

        // Position should be preserved when switching back to Normal
        assert_eq!(sc.target_offset(), (0.0, 100.0));
        assert_eq!(sc.visual_offset(), (0.0, 50.0));
    }

    #[test]
    fn test_push_drag_raw_ignored_in_normal_mode() {
        let _guard = UpdateAuthorityGuard::enter();
        let mut sc = ScrollContainer::new();
        sc.set_content_size(Size::new(500.0, 1000.0));
        sc.set_viewport_size(Size::new(200.0, 300.0));

        // Stay in Normal mode (don't call begin_drag)
        sc.push_drag_raw_y(100.0);
        sc.push_drag_raw_y(200.0);

        sc.update_visual(1.0);
        // Ignored, no change
        assert_eq!(sc.target_offset(), (0.0, 0.0));
    }
}
