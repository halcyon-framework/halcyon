//! Normative tests for SPEC-HAL-110: Scroll Overlay & Viewport Contract
//!
//! These tests define specification truth for scrollbar geometry unification.
//! All scrollbar layout, hit-test, and hover behavior MUST comply with these invariants.

use halcyon::core::math::{Point, RectF};
use halcyon::widgets::scrollbar::{ScrollbarH, ScrollbarV};

// =============================================================================
// INV-L1: Local-Space Coordinate Invariant
// =============================================================================

/// SPEC-HAL-110 INV-L1: layout() requires local-space widget_rect (origin 0,0).
/// This test verifies that layout() works correctly with local-space coordinates.
/// A debug_assert in the implementation enforces this invariant in debug builds.
#[test]
fn inv_l1_layout_v_requires_local_space_origin() {
    let scrollbar = ScrollbarV::new();

    // Valid: local-space widget_rect with origin at (0, 0)
    let local_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let layout = scrollbar.layout(local_rect, 100.0, 300.0, 100.0, 0.0);

    // Layout should produce valid geometry
    assert!(layout.track.width > 0.0, "Track must have positive width");
    assert!(layout.track.height > 0.0, "Track must have positive height");
    assert!(
        layout.hit_rect.width > 0.0,
        "Hit rect must have positive width"
    );
}

#[test]
fn inv_l1_layout_h_requires_local_space_origin() {
    let scrollbar = ScrollbarH::new();

    // Valid: local-space widget_rect with origin at (0, 0)
    let local_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let layout = scrollbar.layout(local_rect, 200.0, 400.0, 200.0, 0.0);

    // Layout should produce valid geometry
    assert!(layout.track.width > 0.0, "Track must have positive width");
    assert!(layout.track.height > 0.0, "Track must have positive height");
    assert!(
        layout.hit_rect.height > 0.0,
        "Hit rect must have positive height"
    );
}

/// INV-L1 violation test: Passing non-zero origin triggers debug_assert.
/// This test documents that layout() with global coordinates is a contract violation.
/// In debug builds, this would panic. This test runs only in release builds.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn inv_l1_layout_v_non_zero_origin_violates_invariant() {
    let scrollbar = ScrollbarV::new();

    // Invalid: global-space widget_rect (non-zero origin)
    // In debug builds, this would trigger debug_assert!
    // In release builds, this runs but produces incorrect geometry
    let global_rect = RectF::new(100.0, 50.0, 200.0, 100.0);
    let layout = scrollbar.layout(global_rect, 100.0, 300.0, 100.0, 0.0);

    // The track would be positioned at 290 (100 + 200 - 10) instead of 190
    // This demonstrates why local-space is required
    assert!(
        layout.track.x > 200.0,
        "Non-zero origin produces incorrect track.x position (global instead of local)"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn inv_l1_layout_h_non_zero_origin_violates_invariant() {
    let scrollbar = ScrollbarH::new();

    // Invalid: global-space widget_rect (non-zero origin)
    let global_rect = RectF::new(100.0, 50.0, 200.0, 100.0);
    let layout = scrollbar.layout(global_rect, 200.0, 400.0, 200.0, 0.0);

    // The track would be positioned at 140 (50 + 100 - 10) instead of 90
    assert!(
        layout.track.y > 100.0,
        "Non-zero origin produces incorrect track.y position (global instead of local)"
    );
}

// =============================================================================
// INV-H1: Hit Expansion Invariants
// =============================================================================

/// SPEC-HAL-110 INV-H1: V-scrollbar hit_rect expands LEFT only (orthogonal to scroll).
/// Width increases by expansion amount, X shifts left.
/// Height remains exactly track_limit (never expands vertically).
#[test]
fn inv_h1_v_scrollbar_hit_rect_expands_left_only() {
    let scrollbar = ScrollbarV::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let track_limit = 100.0;
    let expansion = scrollbar.config.hit_expansion;
    let thickness = scrollbar.config.thickness;

    let layout = scrollbar.layout(widget_rect, track_limit, 300.0, 100.0, 0.0);

    // Hit rect expands LEFT: X is shifted left by expansion amount
    let expected_hit_x = widget_rect.width - thickness - expansion;
    assert_eq!(
        layout.hit_rect.x, expected_hit_x,
        "V-scrollbar hit_rect.x should expand left"
    );

    // Hit rect width = thickness + expansion (expanded left)
    let expected_hit_width = thickness + expansion;
    assert_eq!(
        layout.hit_rect.width, expected_hit_width,
        "V-scrollbar hit_rect.width should be thickness + expansion"
    );

    // Hit rect does NOT expand vertically - height = track_limit exactly
    assert_eq!(
        layout.hit_rect.height, track_limit,
        "V-scrollbar hit_rect.height should equal track_limit (no vertical expansion)"
    );

    // Hit rect starts at widget top (no upward expansion)
    assert_eq!(
        layout.hit_rect.y, widget_rect.y,
        "V-scrollbar hit_rect.y should start at widget top"
    );
}

/// SPEC-HAL-110 INV-H1: H-scrollbar hit_rect expands UPWARD only (orthogonal to scroll).
/// Height increases by expansion amount, Y shifts up.
/// Width remains exactly track_limit (never expands horizontally).
#[test]
fn inv_h1_h_scrollbar_hit_rect_expands_upward_only() {
    let scrollbar = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let track_limit = 200.0;
    let expansion = scrollbar.config.hit_expansion;
    let thickness = scrollbar.config.thickness;

    let layout = scrollbar.layout(widget_rect, track_limit, 400.0, 200.0, 0.0);

    // Hit rect expands UPWARD: Y is shifted up by expansion amount
    let expected_hit_y = widget_rect.height - thickness - expansion;
    assert_eq!(
        layout.hit_rect.y, expected_hit_y,
        "H-scrollbar hit_rect.y should expand upward"
    );

    // Hit rect height = thickness + expansion (expanded upward)
    let expected_hit_height = thickness + expansion;
    assert_eq!(
        layout.hit_rect.height, expected_hit_height,
        "H-scrollbar hit_rect.height should be thickness + expansion"
    );

    // Hit rect does NOT expand horizontally - width = track_limit exactly
    assert_eq!(
        layout.hit_rect.width, track_limit,
        "H-scrollbar hit_rect.width should equal track_limit (no horizontal expansion)"
    );

    // Hit rect starts at widget left (no leftward expansion)
    assert_eq!(
        layout.hit_rect.x, widget_rect.x,
        "H-scrollbar hit_rect.x should start at widget left"
    );
}

/// SPEC-HAL-110 INV-H2: Hit expansion never exceeds track_limit.
/// When track_limit is reduced for corner exclusion, hit_rect respects it.
#[test]
fn inv_h2_hit_expansion_respects_track_limit() {
    let scrollbar_v = ScrollbarV::new();
    let scrollbar_h = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);

    // Simulate corner exclusion: reduce track limits
    let v_track_limit = 90.0; // H scrollbar takes 10px
    let h_track_limit = 190.0; // V scrollbar takes 10px

    let v_layout = scrollbar_v.layout(widget_rect, v_track_limit, 300.0, 100.0, 0.0);
    let h_layout = scrollbar_h.layout(widget_rect, h_track_limit, 400.0, 200.0, 0.0);

    // V-scrollbar hit_rect height must not exceed v_track_limit
    assert_eq!(
        v_layout.hit_rect.height, v_track_limit,
        "V-scrollbar hit_rect.height must equal v_track_limit"
    );

    // H-scrollbar hit_rect width must not exceed h_track_limit
    assert_eq!(
        h_layout.hit_rect.width, h_track_limit,
        "H-scrollbar hit_rect.width must equal h_track_limit"
    );
}

/// SPEC-HAL-110 INV-H3: Hit expansion never overlaps corner-excluded area.
/// When both scrollbars visible, neither hit_rect enters the corner.
#[test]
fn inv_h3_hit_expansion_avoids_corner() {
    let scrollbar_v = ScrollbarV::new();
    let scrollbar_h = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let v_thickness = scrollbar_v.config.thickness;
    let h_thickness = scrollbar_h.config.thickness;

    // Both scrollbars visible: mutual corner exclusion
    let v_track_limit = widget_rect.height - h_thickness;
    let h_track_limit = widget_rect.width - v_thickness;

    let v_layout = scrollbar_v.layout(widget_rect, v_track_limit, 300.0, 100.0, 0.0);
    let h_layout = scrollbar_h.layout(widget_rect, h_track_limit, 400.0, 200.0, 0.0);

    // Corner area: bottom-right rectangle
    let corner_top = widget_rect.height - h_thickness;
    let corner_left = widget_rect.width - v_thickness;

    // V-scrollbar hit_rect bottom must not enter corner
    let v_hit_bottom = v_layout.hit_rect.y + v_layout.hit_rect.height;
    assert!(
        v_hit_bottom <= corner_top,
        "V-scrollbar hit_rect must not extend into corner area. Bottom={v_hit_bottom}, corner_top={corner_top}"
    );

    // H-scrollbar hit_rect right must not enter corner
    let h_hit_right = h_layout.hit_rect.x + h_layout.hit_rect.width;
    assert!(
        h_hit_right <= corner_left,
        "H-scrollbar hit_rect must not extend into corner area. Right={h_hit_right}, corner_left={corner_left}"
    );
}

// =============================================================================
// INV-G2: Geometry Determinism
// =============================================================================

/// SPEC-HAL-110 INV-G2: layout() produces identical geometry for paint/hit-test.
/// Calling layout() multiple times with same params returns identical results.
#[test]
fn inv_g2_layout_v_is_deterministic() {
    let scrollbar = ScrollbarV::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let track_limit = 100.0;
    let content = 300.0;
    let viewport = 100.0;
    let scroll = 50.0;

    let layout1 = scrollbar.layout(widget_rect, track_limit, content, viewport, scroll);
    let layout2 = scrollbar.layout(widget_rect, track_limit, content, viewport, scroll);

    assert_eq!(layout1.track.x, layout2.track.x);
    assert_eq!(layout1.track.y, layout2.track.y);
    assert_eq!(layout1.track.width, layout2.track.width);
    assert_eq!(layout1.track.height, layout2.track.height);
    assert_eq!(layout1.thumb.x, layout2.thumb.x);
    assert_eq!(layout1.thumb.y, layout2.thumb.y);
    assert_eq!(layout1.thumb.width, layout2.thumb.width);
    assert_eq!(layout1.thumb.height, layout2.thumb.height);
    assert_eq!(layout1.hit_rect.x, layout2.hit_rect.x);
    assert_eq!(layout1.hit_rect.y, layout2.hit_rect.y);
}

#[test]
fn inv_g2_layout_h_is_deterministic() {
    let scrollbar = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let track_limit = 200.0;
    let content = 400.0;
    let viewport = 200.0;
    let scroll = 50.0;

    let layout1 = scrollbar.layout(widget_rect, track_limit, content, viewport, scroll);
    let layout2 = scrollbar.layout(widget_rect, track_limit, content, viewport, scroll);

    assert_eq!(layout1.track.x, layout2.track.x);
    assert_eq!(layout1.track.y, layout2.track.y);
    assert_eq!(layout1.track.width, layout2.track.width);
    assert_eq!(layout1.track.height, layout2.track.height);
    assert_eq!(layout1.thumb.x, layout2.thumb.x);
    assert_eq!(layout1.thumb.y, layout2.thumb.y);
}

// =============================================================================
// INV-C1: Corner Exclusion (Cross-Widget Invariant)
// =============================================================================
//
// INV-C1: If both scrollbars are visible, no hit-test or paint may occur
// inside the bottom-right intersection rectangle.
//
// Both ScrollArea and TextArea MUST implement this identically:
//   - V scrollbar: track_limit = height - h_scrollbar_thickness
//   - H scrollbar: track_limit = width - v_scrollbar_thickness
//
// This ensures the corner area is INERT across all scrollable widgets.

/// SPEC-HAL-110 INV-C1: Corner exclusion area is inert.
/// When both scrollbars are visible, the bottom-right corner must not respond
/// to hover, drag, scroll, or pointer capture from either scrollbar.
#[test]
fn inv_c1_corner_exclusion_area_is_inert() {
    let scrollbar_v = ScrollbarV::new();
    let scrollbar_h = ScrollbarH::new();

    // Widget is 200x100, both scrollbars visible
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);
    let v_thickness = scrollbar_v.config.thickness; // typically 10.0
    let h_thickness = scrollbar_h.config.thickness; // typically 10.0

    // V scrollbar track_limit excludes H scrollbar area
    let v_track_limit = widget_rect.height - h_thickness;
    // H scrollbar track_limit excludes V scrollbar area
    let h_track_limit = widget_rect.width - v_thickness;

    let v_layout = scrollbar_v.layout(widget_rect, v_track_limit, 300.0, 100.0, 0.0);
    let h_layout = scrollbar_h.layout(widget_rect, h_track_limit, 400.0, 200.0, 0.0);

    // Pick a point in the bottom-right corner (where both scrollbars would overlap)
    let corner_point = Point::new(
        widget_rect.width - v_thickness / 2.0, // Inside V scrollbar X range
        widget_rect.height - h_thickness / 2.0, // Inside H scrollbar Y range
    );

    // V scrollbar hit_rect should NOT contain the corner point
    // (because track was limited to exclude H scrollbar area)
    assert!(
        !v_layout.hit_rect.contains(corner_point),
        "V-scrollbar hit_rect should not contain corner point ({}, {}). hit_rect: {:?}",
        corner_point.x,
        corner_point.y,
        v_layout.hit_rect
    );

    // H scrollbar hit_rect should NOT contain the corner point
    // (because track was limited to exclude V scrollbar area)
    assert!(
        !h_layout.hit_rect.contains(corner_point),
        "H-scrollbar hit_rect should not contain corner point ({}, {}). hit_rect: {:?}",
        corner_point.x,
        corner_point.y,
        h_layout.hit_rect
    );

    // Additionally verify that thumb regions don't extend into the corner
    assert!(
        !v_layout.thumb.contains(corner_point),
        "V-scrollbar thumb should not contain corner point"
    );
    assert!(
        !h_layout.thumb.contains(corner_point),
        "H-scrollbar thumb should not contain corner point"
    );
}

// =============================================================================
// Track Limit Compliance
// =============================================================================

/// Track limit reduces track height/width for corner exclusion.
#[test]
fn track_limit_reduces_v_track_height() {
    let scrollbar = ScrollbarV::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);

    // Without corner exclusion
    let layout_full = scrollbar.layout(widget_rect, 100.0, 300.0, 100.0, 0.0);
    // With corner exclusion (track_limit = 90, simulating h_scrollbar taking 10px)
    let layout_reduced = scrollbar.layout(widget_rect, 90.0, 300.0, 100.0, 0.0);

    assert_eq!(layout_full.track.height, 100.0);
    assert_eq!(layout_reduced.track.height, 90.0);
    // Hit rect should also respect track limit
    assert_eq!(layout_full.hit_rect.height, 100.0);
    assert_eq!(layout_reduced.hit_rect.height, 90.0);
}

#[test]
fn track_limit_reduces_h_track_width() {
    let scrollbar = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);

    // Without corner exclusion
    let layout_full = scrollbar.layout(widget_rect, 200.0, 400.0, 200.0, 0.0);
    // With corner exclusion (track_limit = 190, simulating v_scrollbar taking 10px)
    let layout_reduced = scrollbar.layout(widget_rect, 190.0, 400.0, 200.0, 0.0);

    assert_eq!(layout_full.track.width, 200.0);
    assert_eq!(layout_reduced.track.width, 190.0);
    // Hit rect should also respect track limit
    assert_eq!(layout_full.hit_rect.width, 200.0);
    assert_eq!(layout_reduced.hit_rect.width, 190.0);
}

// =============================================================================
// Thumb Bounds Invariant
// =============================================================================

/// Thumb is always contained within track bounds.
#[test]
fn thumb_contained_within_v_track() {
    let scrollbar = ScrollbarV::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);

    // Test at various scroll positions
    for scroll in [0.0, 50.0, 100.0, 200.0] {
        let layout = scrollbar.layout(widget_rect, 100.0, 300.0, 100.0, scroll);
        assert!(
            layout.thumb.y >= layout.track.y,
            "Thumb top should be >= track top at scroll={scroll}"
        );
        assert!(
            layout.thumb.y + layout.thumb.height <= layout.track.y + layout.track.height,
            "Thumb bottom should be <= track bottom at scroll={scroll}"
        );
    }
}

#[test]
fn thumb_contained_within_h_track() {
    let scrollbar = ScrollbarH::new();
    let widget_rect = RectF::new(0.0, 0.0, 200.0, 100.0);

    for scroll in [0.0, 50.0, 100.0, 200.0] {
        let layout = scrollbar.layout(widget_rect, 200.0, 400.0, 200.0, scroll);
        assert!(
            layout.thumb.x >= layout.track.x,
            "Thumb left should be >= track left at scroll={scroll}"
        );
        assert!(
            layout.thumb.x + layout.thumb.width <= layout.track.x + layout.track.width,
            "Thumb right should be <= track right at scroll={scroll}"
        );
    }
}
