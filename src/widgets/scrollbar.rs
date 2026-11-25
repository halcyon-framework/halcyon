// SPEC-HAL-086 Stage 8: Scrollbar widgets
// SPEC-HAL-086 Stage 5: Fix drag jitter (captured geometry at drag start, unclamped output)
// SPEC-HAL-086 Stage 10: Correct thumb→scroll mapping (no teleporting)
// SPEC-HAL-110: Unified scrollbar layout and invariants
//
// ScrollbarV and ScrollbarH are scrollbar state containers that:
// - Track visual state (Normal/Hovered/Pressed)
// - Track drag state (dragging, drag_start values)
// - Capture full track geometry at drag start for correct mapping
// - Provide color selection based on state
//
// Geometry is computed on-demand via layout() - the single source of truth.
// These are not standalone Widget implementations - they are owned by ScrollArea/TextArea.
//
// =============================================================================
// SPEC-HAL-110 INVARIANTS
// =============================================================================
//
// INV-L1: Local-Space Coordinates
//   - layout() receives local-space widget_rect (origin = 0,0)
//   - Caller is responsible for global translation after layout
//   - Enforced via debug_assert in layout()
//
// INV-C1: Corner Exclusion
//   If both scrollbars are visible, no hit-test or paint may occur inside
//   the bottom-right intersection rectangle. Widgets enforce this by:
//   - V scrollbar: track_limit = height - h_scrollbar_thickness
//   - H scrollbar: track_limit = width - v_scrollbar_thickness
//   Both ScrollArea and TextArea MUST implement this identically.
//
// INV-G2: Geometry Determinism
//   layout() produces identical geometry for paint, hit-test, and hover.
//   Same inputs always yield same ScrollbarLayout.
//
// INV-H1: Hit Expansion Direction
//   - V scrollbar: hit_rect expands LEFT only (orthogonal to scroll)
//   - H scrollbar: hit_rect expands UPWARD only (orthogonal to scroll)
//
// INV-H2: Hit Expansion Bounds
//   Hit expansion never exceeds track_limit.
//
// INV-H3: Hit Expansion Corner Avoidance
//   Hit expansion never overlaps corner-excluded area.
//
// =============================================================================

use crate::core::math::RectF;
use crate::core::paint::Color;
use crate::core::theme::ColorSpec;

// =============================================================================
// SPEC-HAL-086 Stage 10: Correct thumb→scroll mapping helpers
// =============================================================================

/// Map a vertical scrollbar drag to content scroll offset.
///
/// SPEC-HAL-086 Stage 10: This formula ensures:
/// - Thumb movement is strictly proportional to pointer delta
/// - No jump when first clicking the thumb
/// - Proper clamping via travel and max_scroll
///
/// # Arguments
/// - `pointer_y`: Current global pointer Y
/// - `drag_start_pointer`: Global Y at begin_drag()
/// - `drag_start_scroll`: Scroll offset at begin_drag()
/// - `track_top`: Top of the gutter (global coords)
/// - `track_bottom`: Bottom of the gutter (global coords)
/// - `thumb_len`: Height of the thumb in pixels
/// - `max_scroll`: Maximum scroll offset (content_height - viewport_height)
///
/// # Returns
/// Content scroll offset in [0, max_scroll]
pub fn thumb_drag_to_scroll_y(
    pointer_y: f32,
    drag_start_pointer: f32,
    drag_start_scroll: f32,
    track_top: f32,
    track_bottom: f32,
    thumb_len: f32,
    max_scroll: f32,
) -> f32 {
    let track_len = (track_bottom - track_top).max(0.0);
    let travel = (track_len - thumb_len).max(0.0);

    // Nothing to scroll or thumb can't move → pinned at 0
    if travel <= 0.0 || max_scroll <= 0.0 {
        return 0.0;
    }

    // Where was the thumb at drag start?
    let start_thumb_top = track_top + (drag_start_scroll / max_scroll) * travel;

    // Move thumb by the same delta as the pointer
    let delta_pixels = pointer_y - drag_start_pointer;
    let new_thumb_top_unclamped = start_thumb_top + delta_pixels;

    // Clamp thumb to valid range
    let min_thumb_top = track_top;
    let max_thumb_top = track_top + travel;
    let new_thumb_top = new_thumb_top_unclamped.clamp(min_thumb_top, max_thumb_top);

    // Convert back to scroll offset
    let thumb_frac = (new_thumb_top - track_top) / travel;
    thumb_frac * max_scroll
}

/// Map a horizontal scrollbar drag to content scroll offset.
///
/// SPEC-HAL-086 Stage 10: See `thumb_drag_to_scroll_y()` for details.
pub fn thumb_drag_to_scroll_x(
    pointer_x: f32,
    drag_start_pointer: f32,
    drag_start_scroll: f32,
    track_left: f32,
    track_right: f32,
    thumb_len: f32,
    max_scroll: f32,
) -> f32 {
    let track_len = (track_right - track_left).max(0.0);
    let travel = (track_len - thumb_len).max(0.0);

    if travel <= 0.0 || max_scroll <= 0.0 {
        return 0.0;
    }

    let start_thumb_left = track_left + (drag_start_scroll / max_scroll) * travel;
    let delta_pixels = pointer_x - drag_start_pointer;
    let new_thumb_left_unclamped = start_thumb_left + delta_pixels;

    let min_thumb_left = track_left;
    let max_thumb_left = track_left + travel;
    let new_thumb_left = new_thumb_left_unclamped.clamp(min_thumb_left, max_thumb_left);

    let thumb_frac = (new_thumb_left - track_left) / travel;
    thumb_frac * max_scroll
}

/// Scrollbar visual state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

/// Shared scrollbar configuration
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarConfig {
    /// Width of the scrollbar track (vertical) or height (horizontal)
    pub thickness: f32,
    /// Minimum thumb size (never smaller than this)
    pub min_thumb_size: f32,
    /// Corner radius for rounded thumb
    pub corner_radius: f32,
    /// Extra hit area expansion (makes scrollbar easier to click)
    /// The hit area extends this many pixels beyond the visual track
    pub hit_expansion: f32,
}

// =============================================================================
// SPEC-HAL-096D: Scrollbar Hit Expansion Invariants
// =============================================================================
//
// INV-SCROLLBAR-HIT-1: Hit expansion authority
//   Scrollbar hit expansion is read ONLY from `ScrollbarConfig.hit_expansion`.
//   No hardcoded literals allowed at consumption sites.
//
// INV-SCROLLBAR-HIT-2: Hit expansion scope
//   Hit expansion affects ONLY hit-testing (pointer targeting).
//   Must NEVER affect:
//     - layout constraints or sizing
//     - paint geometry (visual bounds)
//     - scroll offsets or math
//     - visibility logic
//
// INV-SCROLLBAR-HIT-3: Theme sync location
//   Theme values must be synced ONLY in layout() (canonical sync point).
//   Event handlers and paint code must NEVER read theme directly.
//   They consume only from cached config values.
//
// =============================================================================

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme.scrollbar_geom in layout()
            thickness: 10.0,
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme.scrollbar_geom in layout()
            min_thumb_size: 22.0,
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme.scrollbar_geom in layout()
            corner_radius: 5.0,
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme.scrollbar_geom in layout()
            hit_expansion: 6.0,
        }
    }
}

// =============================================================================
// SPEC-HAL-110: Unified Scrollbar Layout (single source of truth)
// =============================================================================

/// Unified scrollbar layout - single source of truth for paint, hit-test, hover, drag.
///
/// SPEC-HAL-110 INV-G2: Paint = Hit-Test Geometry.
/// This struct is computed once and used for ALL scrollbar operations.
/// No independent geometry calculations allowed anywhere else.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarLayout {
    /// Track rectangle (full scrollbar area, already accounts for corner exclusion)
    pub track: RectF,
    /// Thumb rectangle (draggable portion)
    pub thumb: RectF,
    /// Hit rect for pointer detection (may be expanded for easier clicking)
    pub hit_rect: RectF,
}

impl ScrollbarLayout {
    pub(super) fn translated(self, x: f32, y: f32) -> Self {
        Self {
            track: self.track.translate(x, y),
            thumb: self.thumb.translate(x, y),
            hit_rect: self.hit_rect.translate(x, y),
        }
    }
}

pub(super) const OVERLAY_SCROLLBAR_HIDE_DELAY_SECS: f32 = 1.5;

/// Vertical scrollbar state
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarV {
    /// Current visual state
    pub state: ScrollbarState,
    /// Whether thumb is being dragged
    pub dragging: bool,
    /// Drag start position (Y coordinate, global)
    pub drag_start_y: f32,
    /// Scroll offset at drag start
    pub drag_start_scroll: f32,
    /// SPEC-HAL-086 Stage 10: Track top (global coords) at drag start
    pub drag_track_top: f32,
    /// SPEC-HAL-086 Stage 10: Track bottom (global coords) at drag start
    pub drag_track_bottom: f32,
    /// SPEC-HAL-086 Stage 10: Thumb height at drag start
    pub drag_thumb_len: f32,
    /// SPEC-HAL-086 Stage 5: Captured max_scroll at drag start (stable ratio)
    pub drag_max_scroll: f32,
    /// Configuration
    pub config: ScrollbarConfig,
}

impl Default for ScrollbarV {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollbarV {
    pub fn new() -> Self {
        Self {
            state: ScrollbarState::Normal,
            dragging: false,
            drag_start_y: 0.0,
            drag_start_scroll: 0.0,
            drag_track_top: 0.0,
            drag_track_bottom: 0.0,
            drag_thumb_len: 0.0,
            drag_max_scroll: 0.0,
            config: ScrollbarConfig::default(),
        }
    }

    /// SPEC-HAL-110: Unified layout computation.
    ///
    /// Single source of truth for paint, hit-test, hover, and drag initiation.
    /// `track_limit` already accounts for gutter reservation and corner exclusion.
    ///
    /// # Coordinate Invariant (INV-L1)
    ///
    /// - `widget_rect` is always local-space (origin = 0,0)
    /// - `layout()` never receives global coordinates
    /// - Caller is responsible for global translation after layout
    ///
    /// # Arguments
    /// - `widget_rect`: Full widget rectangle (local-space, origin at 0,0)
    /// - `track_limit`: Track height limit (widget_rect.height minus h_scrollbar if both visible)
    /// - `content`: Total content height
    /// - `viewport`: Visible viewport height (used for max_scroll calculation)
    /// - `scroll`: Current scroll offset
    #[must_use = "ScrollbarLayout must be used for render and hit-testing"]
    pub fn layout(
        &self,
        widget_rect: RectF,
        track_limit: f32,
        content: f32,
        viewport: f32,
        scroll: f32,
    ) -> ScrollbarLayout {
        // INV-L1: widget_rect must be local-space (origin at 0,0)
        debug_assert!(
            widget_rect.x == 0.0 && widget_rect.y == 0.0,
            "ScrollbarV::layout() requires local-space widget_rect (origin 0,0), got ({}, {})",
            widget_rect.x,
            widget_rect.y
        );

        let thickness = self.config.thickness;
        let max_scroll = (content - viewport).max(0.0);

        // Track positioned at right edge of widget, height limited by track_limit
        let track = RectF::new(
            widget_rect.x + widget_rect.width - thickness,
            widget_rect.y,
            thickness,
            track_limit,
        );

        // Thumb height proportional to track/content ratio (like the old geometry() method)
        let ratio = if content > 0.0 {
            (track_limit / content).min(1.0)
        } else {
            1.0
        };
        let thumb_height = (track_limit * ratio)
            .max(self.config.min_thumb_size)
            .min(track_limit);

        // Thumb Y position based on scroll position
        // FIX C2: Clamp scroll_ratio to [0, 1] to prevent thumb overdraw.
        // Without clamping, transient scroll > max_scroll causes thumb to escape track.
        let available_track = track_limit - thumb_height;
        let scroll_ratio = if max_scroll > 0.0 {
            (scroll / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_y = widget_rect.y + available_track * scroll_ratio;

        let thumb = RectF::new(track.x, thumb_y, thickness, thumb_height);

        // Hit rect extends to the left for easier clicking
        let expansion = self.config.hit_expansion;
        let hit_rect = RectF::new(
            widget_rect.x + widget_rect.width - thickness - expansion,
            widget_rect.y,
            thickness + expansion,
            track_limit,
        );

        ScrollbarLayout {
            track,
            thumb,
            hit_rect,
        }
    }

    /// Begin drag operation
    ///
    /// SPEC-HAL-086 Stage 10: Captures full track geometry at drag start
    /// for correct thumb→scroll mapping throughout the drag session.
    ///
    /// # Arguments
    /// - `y`: Global pointer Y at mouse down
    /// - `current_scroll`: Scroll offset at mouse down
    /// - `track_top`: Top of track in global coords
    /// - `track_bottom`: Bottom of track in global coords
    /// - `thumb_len`: Thumb height in pixels
    /// - `max_scroll`: Maximum scroll offset
    pub fn begin_drag(
        &mut self,
        y: f32,
        current_scroll: f32,
        track_top: f32,
        track_bottom: f32,
        thumb_len: f32,
        max_scroll: f32,
    ) {
        self.dragging = true;
        self.drag_start_y = y;
        self.drag_start_scroll = current_scroll;
        self.drag_track_top = track_top;
        self.drag_track_bottom = track_bottom;
        self.drag_thumb_len = thumb_len;
        self.drag_max_scroll = max_scroll;
        self.state = ScrollbarState::Pressed;
    }

    /// Update drag and return new scroll offset
    ///
    /// SPEC-HAL-086 Stage 10: Uses correct thumb→scroll mapping formula.
    /// Thumb movement is strictly proportional to pointer delta.
    /// Returns clamped value in [0, max_scroll].
    ///
    /// # Arguments
    /// - `current_y`: Current pointer Y position (global coordinates)
    ///
    /// # Returns
    /// New scroll offset based on drag delta, clamped to [0, max_scroll]
    pub fn update_drag(&self, current_y: f32) -> f32 {
        if !self.dragging {
            return self.drag_start_scroll;
        }

        thumb_drag_to_scroll_y(
            current_y,
            self.drag_start_y,
            self.drag_start_scroll,
            self.drag_track_top,
            self.drag_track_bottom,
            self.drag_thumb_len,
            self.drag_max_scroll,
        )
    }

    /// End drag operation
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.state = ScrollbarState::Normal;
    }

    /// Get thumb color based on current state.
    /// Colors are read from theme's ColorSpec (single source of truth).
    pub fn thumb_color(&self, colors: &ColorSpec) -> Color {
        match self.state {
            ScrollbarState::Normal => colors.scrollbars.thumb,
            ScrollbarState::Hovered => colors.scrollbars.thumb_hover,
            ScrollbarState::Pressed => colors.scrollbars.thumb_hover, // SPEC-096 has no active
        }
    }

    /// Check if scrollbar should be visible (content overflows viewport)
    pub fn is_visible(&self, content_height: f32, viewport_height: f32) -> bool {
        content_height > viewport_height
    }
}

/// Horizontal scrollbar state
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarH {
    /// Current visual state
    pub state: ScrollbarState,
    /// Whether thumb is being dragged
    pub dragging: bool,
    /// Drag start position (X coordinate, global)
    pub drag_start_x: f32,
    /// Scroll offset at drag start
    pub drag_start_scroll: f32,
    /// SPEC-HAL-086 Stage 10: Track left (global coords) at drag start
    pub drag_track_left: f32,
    /// SPEC-HAL-086 Stage 10: Track right (global coords) at drag start
    pub drag_track_right: f32,
    /// SPEC-HAL-086 Stage 10: Thumb width at drag start
    pub drag_thumb_len: f32,
    /// SPEC-HAL-086 Stage 5: Captured max_scroll at drag start (stable ratio)
    pub drag_max_scroll: f32,
    /// Configuration
    pub config: ScrollbarConfig,
}

impl Default for ScrollbarH {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollbarH {
    pub fn new() -> Self {
        Self {
            state: ScrollbarState::Normal,
            dragging: false,
            drag_start_x: 0.0,
            drag_start_scroll: 0.0,
            drag_track_left: 0.0,
            drag_track_right: 0.0,
            drag_thumb_len: 0.0,
            drag_max_scroll: 0.0,
            config: ScrollbarConfig::default(),
        }
    }

    /// SPEC-HAL-110: Unified layout computation.
    ///
    /// Single source of truth for paint, hit-test, hover, and drag initiation.
    /// `track_limit` already accounts for gutter reservation and corner exclusion.
    ///
    /// # Coordinate Invariant (INV-L1)
    ///
    /// - `widget_rect` is always local-space (origin = 0,0)
    /// - `layout()` never receives global coordinates
    /// - Caller is responsible for global translation after layout
    ///
    /// # Arguments
    /// - `widget_rect`: Full widget rectangle (local-space, origin at 0,0)
    /// - `track_limit`: Track width limit (widget_rect.width minus v_scrollbar if both visible)
    /// - `content`: Total content width
    /// - `viewport`: Visible viewport width (used for max_scroll calculation)
    /// - `scroll`: Current scroll offset
    #[must_use = "ScrollbarLayout must be used for render and hit-testing"]
    pub fn layout(
        &self,
        widget_rect: RectF,
        track_limit: f32,
        content: f32,
        viewport: f32,
        scroll: f32,
    ) -> ScrollbarLayout {
        // INV-L1: widget_rect must be local-space (origin at 0,0)
        debug_assert!(
            widget_rect.x == 0.0 && widget_rect.y == 0.0,
            "ScrollbarH::layout() requires local-space widget_rect (origin 0,0), got ({}, {})",
            widget_rect.x,
            widget_rect.y
        );

        let thickness = self.config.thickness;
        let max_scroll = (content - viewport).max(0.0);

        // Track positioned at bottom edge of widget, width limited by track_limit
        let track = RectF::new(
            widget_rect.x,
            widget_rect.y + widget_rect.height - thickness,
            track_limit,
            thickness,
        );

        // Thumb width proportional to track/content ratio (like the old geometry() method)
        let ratio = if content > 0.0 {
            (track_limit / content).min(1.0)
        } else {
            1.0
        };
        let thumb_width = (track_limit * ratio)
            .max(self.config.min_thumb_size)
            .min(track_limit);

        // Thumb X position based on scroll position
        // FIX C2: Clamp scroll_ratio to [0, 1] to prevent thumb overdraw.
        // Without clamping, transient scroll > max_scroll causes thumb to escape track.
        let available_track = track_limit - thumb_width;
        let scroll_ratio = if max_scroll > 0.0 {
            (scroll / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_x = widget_rect.x + available_track * scroll_ratio;

        let thumb = RectF::new(thumb_x, track.y, thumb_width, thickness);

        // Hit rect extends upward for easier clicking
        let expansion = self.config.hit_expansion;
        let hit_rect = RectF::new(
            widget_rect.x,
            widget_rect.y + widget_rect.height - thickness - expansion,
            track_limit,
            thickness + expansion,
        );

        ScrollbarLayout {
            track,
            thumb,
            hit_rect,
        }
    }

    /// Begin drag operation
    ///
    /// SPEC-HAL-086 Stage 10: Captures full track geometry at drag start
    /// for correct thumb→scroll mapping throughout the drag session.
    pub fn begin_drag(
        &mut self,
        x: f32,
        current_scroll: f32,
        track_left: f32,
        track_right: f32,
        thumb_len: f32,
        max_scroll: f32,
    ) {
        self.dragging = true;
        self.drag_start_x = x;
        self.drag_start_scroll = current_scroll;
        self.drag_track_left = track_left;
        self.drag_track_right = track_right;
        self.drag_thumb_len = thumb_len;
        self.drag_max_scroll = max_scroll;
        self.state = ScrollbarState::Pressed;
    }

    /// Update drag and return new scroll offset
    ///
    /// SPEC-HAL-086 Stage 10: Uses correct thumb→scroll mapping formula.
    /// Thumb movement is strictly proportional to pointer delta.
    /// Returns clamped value in [0, max_scroll].
    pub fn update_drag(&self, current_x: f32) -> f32 {
        if !self.dragging {
            return self.drag_start_scroll;
        }

        thumb_drag_to_scroll_x(
            current_x,
            self.drag_start_x,
            self.drag_start_scroll,
            self.drag_track_left,
            self.drag_track_right,
            self.drag_thumb_len,
            self.drag_max_scroll,
        )
    }

    /// End drag operation
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.state = ScrollbarState::Normal;
    }

    /// Get thumb color based on current state.
    /// Colors are read from theme's ColorSpec (single source of truth).
    pub fn thumb_color(&self, colors: &ColorSpec) -> Color {
        match self.state {
            ScrollbarState::Normal => colors.scrollbars.thumb,
            ScrollbarState::Hovered => colors.scrollbars.thumb_hover,
            ScrollbarState::Pressed => colors.scrollbars.thumb_hover, // SPEC-096 has no active
        }
    }

    /// Check if scrollbar should be visible (content overflows viewport)
    pub fn is_visible(&self, content_width: f32, viewport_width: f32) -> bool {
        content_width > viewport_width
    }
}

// =============================================================================
// SPEC-HAL-110 Tests: Unified Scrollbar Layout
// Tests moved to tests/SPEC/SPEC_hal_110_scrollbar_layout.rs (normative SPEC tests)
