// SPEC-HAL-131A: Content Trim Contract Tests (Model 3: Bidirectional Removal)
//
// Tests validating the scroll content trim contract for memory-bounded
// virtualized feeds that discard off-screen content.
//
// Invariants tested:
// - INV-SCROLL-TRIM-1: Directional legality (delta >= 0)
// - INV-SCROLL-TRIM-2: Offset compensation

// - INV-SCROLL-TRIM-3: Viewport stability
// - INV-SCROLL-TRIM-4: Anchor safety
// - INV-SCROLL-TRIM-5: Geometry correctness
// - INV-SCROLL-TRIM-6: No hidden side effects

use halcyon::core::math::{Point, Size};
use halcyon::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor, ScrollState};

// =============================================================================
// Test 1: Top Trim - Offset Compensation
// =============================================================================

#[test]
fn trim_top_offset_compensation_basic() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to middle
    state.set_offset_y(400.0);
    let offset_before = state.logical_offset.y;

    // Trim 100px from top
    state.trim_content_top(100.0);

    // INV-SCROLL-TRIM-2: Offset decreased by delta
    assert_eq!(
        state.logical_offset.y,
        offset_before - 100.0,
        "Offset must decrease by trim delta"
    );

    // INV-SCROLL-TRIM-5: Content extent shrinks
    assert_eq!(state.content_extent.height, 900.0);

    // max_scroll updated: 900 - 200 = 700
    assert_eq!(state.max_scroll().y, 700.0);
}

#[test]
fn trim_top_clamps_offset_to_zero() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll near top
    state.set_offset_y(50.0);

    // Trim more than current offset
    state.trim_content_top(100.0);

    // INV-SCROLL-TRIM-2: Offset clamped to 0
    assert_eq!(state.logical_offset.y, 0.0);
    assert_eq!(state.content_extent.height, 900.0);
}

#[test]
fn trim_top_at_origin_stays_at_zero() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // At origin
    assert_eq!(state.logical_offset.y, 0.0);

    // Trim from top
    state.trim_content_top(200.0);

    // Offset stays at 0
    assert_eq!(state.logical_offset.y, 0.0);
    assert_eq!(state.content_extent.height, 800.0);
}

// =============================================================================
// Test 2: Bottom Trim - Offset Unchanged Unless Clamped
// =============================================================================

#[test]
fn trim_bottom_offset_unchanged_when_not_at_bottom() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to middle
    state.set_offset_y(300.0);
    let offset_before = state.logical_offset.y;

    // Trim 200px from bottom
    state.trim_content_bottom(200.0);

    // INV-SCROLL-TRIM-2: Offset unchanged
    assert_eq!(state.logical_offset.y, offset_before);

    // INV-SCROLL-TRIM-5: Content extent shrinks
    assert_eq!(state.content_extent.height, 800.0);

    // max_scroll = 800 - 200 = 600
    assert_eq!(state.max_scroll().y, 600.0);
}

#[test]
fn trim_bottom_clamps_offset_when_at_bottom() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to max (800)
    state.set_offset_y(800.0);
    assert!(state.at_end().1);

    // Trim 300 from bottom
    state.trim_content_bottom(300.0);

    // INV-SCROLL-TRIM-3: Offset clamped to new max
    // New max = 700 - 200 = 500
    assert_eq!(state.logical_offset.y, 500.0);
    assert_eq!(state.content_extent.height, 700.0);
}

#[test]
fn trim_bottom_partial_clamp() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to 600 (max is 800)
    state.set_offset_y(600.0);

    // Trim 300 from bottom - new max is 500, offset must clamp
    state.trim_content_bottom(300.0);

    assert_eq!(state.logical_offset.y, 500.0);
    assert_eq!(state.content_extent.height, 700.0);
}

// =============================================================================
// Test 3: Viewport Stability
// =============================================================================

#[test]
fn trim_top_viewport_stable_when_off_screen() {
    let mut state = ScrollState::new();
    let item_height = 40.0;

    state.set_content_extent(Size::new(400.0, 100.0 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // View item 50 at top of viewport
    let item_index = 50;
    let item_content_y = item_index as f32 * item_height;
    state.set_offset_y(item_content_y);

    // Visual position of item 50 before trim
    let visual_before = item_content_y - state.logical_offset.y;
    assert_eq!(visual_before, 0.0);

    // Trim 10 items from top (fully off-screen above viewport)
    let trim_amount = 10.0 * item_height;
    state.trim_content_top(trim_amount);

    // After trim: item 50 is now item 40 (conceptually), at position (50-10)*40 = 1600
    // But offset also decreased by 400: 2000 - 400 = 1600
    // Visual position = 1600 - 1600 = 0 (still at top!)
    let new_item_content_y = item_content_y - trim_amount;
    let visual_after = new_item_content_y - state.logical_offset.y;

    assert_eq!(
        visual_before, visual_after,
        "INV-SCROLL-TRIM-3: Visual position must be stable"
    );
}

#[test]
fn trim_bottom_viewport_stable_when_off_screen() {
    let mut state = ScrollState::new();
    let item_height = 40.0;

    state.set_content_extent(Size::new(400.0, 100.0 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // View item 20 at top of viewport
    let item_index = 20;
    let item_content_y = item_index as f32 * item_height;
    state.set_offset_y(item_content_y);

    let visual_before = item_content_y - state.logical_offset.y;
    let offset_before = state.logical_offset.y;

    // Trim 20 items from bottom (fully off-screen below viewport)
    let trim_amount = 20.0 * item_height;
    state.trim_content_bottom(trim_amount);

    // INV-SCROLL-TRIM-3: Offset unchanged, viewport stable
    assert_eq!(state.logical_offset.y, offset_before);

    let visual_after = item_content_y - state.logical_offset.y;
    assert_eq!(visual_before, visual_after);
}

// =============================================================================
// Test 4: Anchor Safety
// =============================================================================

/// Test resolver that maps AnchorId to a fixed position
struct TestAnchorResolver {
    positions: Vec<(AnchorId, Point)>,
}

impl AnchorResolver for TestAnchorResolver {
    fn resolve_anchor(&self, anchor: &ScrollAnchor) -> Option<Point> {
        self.positions
            .iter()
            .find(|(id, _)| *id == anchor.target_id)
            .map(|(_, pos)| *pos)
    }

    fn find_nearest_anchor(&self, _anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        None
    }
}

#[test]
fn trim_top_preserves_anchor_identity() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set anchor to item in viewport
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // Trim content from top (above anchor)
    state.trim_content_top(100.0);

    // INV-SCROLL-TRIM-4: Anchor not mutated
    assert!(state.anchor.is_some(), "Anchor must be preserved");
    assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(42));
}

#[test]
fn trim_bottom_preserves_anchor_identity() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set anchor
    let anchor = ScrollAnchor::new(
        AnchorId::new(99),
        Point::new(0.0, 10.0),
        Point::new(0.0, 100.0),
    );
    state.set_anchor(anchor);

    // Trim content from bottom
    state.trim_content_bottom(500.0);

    // INV-SCROLL-TRIM-4: Anchor not mutated
    assert!(state.anchor.is_some());
    let a = state.anchor.unwrap();
    assert_eq!(a.target_id, AnchorId::new(99));
    assert_eq!(a.target_offset, Point::new(0.0, 10.0));
    assert_eq!(a.viewport_offset, Point::new(0.0, 100.0));
}

#[test]
fn trim_does_not_trigger_anchor_resolution() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);

    let offset_before = state.logical_offset.y;

    // Set anchor (but don't resolve)
    let anchor = ScrollAnchor::new(
        AnchorId::new(77),
        Point::new(0.0, 0.0),
        Point::new(0.0, 25.0),
    );
    state.set_anchor(anchor);

    // Trim operations
    state.trim_content_top(100.0);
    state.trim_content_bottom(100.0);

    // INV-SCROLL-TRIM-6: No hidden side effects
    // Anchor still present (not resolved, not cleared)
    assert!(state.anchor.is_some());
    assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(77));

    // Offset changed only by documented compensation
    assert_eq!(state.logical_offset.y, offset_before - 100.0);
}

// =============================================================================
// Test 5: Geometry Correctness
// =============================================================================

#[test]
fn trim_top_geometry_updates_immediately() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(400.0);

    let max_before = state.max_scroll().y;
    let (_, thumb_before) = state.thumb_fraction();

    state.trim_content_top(200.0);

    // INV-SCROLL-TRIM-5: Geometry updates same frame
    let max_after = state.max_scroll().y;
    assert_eq!(max_after, max_before - 200.0);

    // Thumb fraction changes (viewport same, content smaller)
    let (_, thumb_after) = state.thumb_fraction();
    assert!(
        thumb_after > thumb_before,
        "Thumb should grow when content shrinks"
    );
}

#[test]
fn trim_bottom_geometry_updates_immediately() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(200.0);

    let max_before = state.max_scroll().y;

    state.trim_content_bottom(300.0);

    // INV-SCROLL-TRIM-5: max_scroll updates immediately
    let max_after = state.max_scroll().y;
    assert_eq!(max_after, max_before - 300.0);

    // Content extent shrinks
    assert_eq!(state.content_extent.height, 700.0);
}

#[test]
fn trim_scrollbar_fraction_correct() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Position at 50% scroll
    state.set_offset_y(400.0);
    let (_, frac_before) = state.scroll_fraction();
    assert!((frac_before - 0.5).abs() < 0.01);

    // Trim 200 from top
    state.trim_content_top(200.0);

    // New state: content=800, offset=200, max=600
    // fraction = 200/600 = 0.333...
    let (_, frac_after) = state.scroll_fraction();
    let expected = 200.0 / 600.0;
    assert!(
        (frac_after - expected).abs() < 0.01,
        "Expected {}, got {}",
        expected,
        frac_after
    );
}

// =============================================================================
// Test 6: Zero Delta Is Noop
// =============================================================================

#[test]
fn trim_top_zero_is_noop() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(300.0);

    let content_before = state.content_extent.height;
    let offset_before = state.logical_offset.y;

    state.trim_content_top(0.0);

    assert_eq!(state.content_extent.height, content_before);
    assert_eq!(state.logical_offset.y, offset_before);
}

#[test]
fn trim_bottom_zero_is_noop() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(300.0);

    let content_before = state.content_extent.height;
    let offset_before = state.logical_offset.y;

    state.trim_content_bottom(0.0);

    assert_eq!(state.content_extent.height, content_before);
    assert_eq!(state.logical_offset.y, offset_before);
}

// =============================================================================
// Test 7: Multiple Trims Accumulate
// =============================================================================

#[test]
fn multiple_top_trims_accumulate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);

    state.trim_content_top(100.0);
    assert_eq!(state.content_extent.height, 900.0);
    assert_eq!(state.logical_offset.y, 400.0);

    state.trim_content_top(100.0);
    assert_eq!(state.content_extent.height, 800.0);
    assert_eq!(state.logical_offset.y, 300.0);

    state.trim_content_top(100.0);
    assert_eq!(state.content_extent.height, 700.0);
    assert_eq!(state.logical_offset.y, 200.0);
}

#[test]
fn multiple_bottom_trims_accumulate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(200.0);

    state.trim_content_bottom(100.0);
    assert_eq!(state.content_extent.height, 900.0);
    assert_eq!(state.logical_offset.y, 200.0);

    state.trim_content_bottom(100.0);
    assert_eq!(state.content_extent.height, 800.0);
    assert_eq!(state.logical_offset.y, 200.0);

    state.trim_content_bottom(100.0);
    assert_eq!(state.content_extent.height, 700.0);
    assert_eq!(state.logical_offset.y, 200.0);
}

// =============================================================================
// Test 8: Edge Cases
// =============================================================================

#[test]
fn trim_more_than_content_clamps() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 500.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(150.0);

    // Try to trim more than content
    state.trim_content_top(1000.0);

    // Should clamp to content height
    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.logical_offset.y, 0.0);
}

#[test]
fn trim_bottom_more_than_content_clamps() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 500.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(100.0);

    // Try to trim more than content
    state.trim_content_bottom(1000.0);

    // Should clamp to content height
    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.logical_offset.y, 0.0);
}

#[test]
fn trim_from_empty_content_is_safe() {
    let mut state = ScrollState::new();
    state.set_viewport_extent(Size::new(400.0, 200.0));
    // Content is zero

    state.trim_content_top(100.0);
    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.logical_offset.y, 0.0);

    state.trim_content_bottom(100.0);
    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.logical_offset.y, 0.0);
}

// =============================================================================
// Test 9: Combined Operations
// =============================================================================

#[test]
fn trim_top_and_bottom_together() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(400.0);

    // Trim from both ends
    state.trim_content_top(100.0);
    assert_eq!(state.logical_offset.y, 300.0);
    assert_eq!(state.content_extent.height, 900.0);

    state.trim_content_bottom(100.0);
    assert_eq!(state.logical_offset.y, 300.0); // Unchanged
    assert_eq!(state.content_extent.height, 800.0);
}

#[test]
fn extend_then_trim_roundtrip() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(400.0);

    let offset_initial = state.logical_offset.y;
    let content_initial = state.content_extent.height;

    // Extend bottom, then trim bottom
    state.extend_content_height(200.0);
    assert_eq!(state.content_extent.height, 1200.0);
    assert_eq!(state.logical_offset.y, offset_initial); // Unchanged

    state.trim_content_bottom(200.0);
    assert_eq!(state.content_extent.height, content_initial);
    assert_eq!(state.logical_offset.y, offset_initial);
}

#[test]
fn prepend_then_trim_top_roundtrip() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(400.0);

    let offset_initial = state.logical_offset.y;
    let content_initial = state.content_extent.height;

    // Prepend top, then trim top
    state.prepend_content_height(200.0);
    assert_eq!(state.content_extent.height, 1200.0);
    assert_eq!(state.logical_offset.y, offset_initial + 200.0);

    state.trim_content_top(200.0);
    assert_eq!(state.content_extent.height, content_initial);
    assert_eq!(state.logical_offset.y, offset_initial);
}

// =============================================================================
// Test 10: ScrollArea Forwarding
// =============================================================================

#[test]
fn scroll_area_forwards_trim_top() {
    use halcyon::core::math::Size;
    use halcyon::core::widget_tree::NodeId;
    use halcyon::widgets::scroll_area::ScrollArea;

    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);

    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 1000.0));
    scroll_area.scroll_to_y(400.0);

    let offset_before = scroll_area.target_scroll_y();

    scroll_area.trim_content_top(100.0);

    // Offset decreased by delta
    assert_eq!(scroll_area.target_scroll_y(), offset_before - 100.0);
}

#[test]
fn scroll_area_forwards_trim_bottom() {
    use halcyon::core::math::Size;
    use halcyon::core::widget_tree::NodeId;
    use halcyon::widgets::scroll_area::ScrollArea;

    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);

    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 1000.0));
    scroll_area.scroll_to_y(300.0);

    let offset_before = scroll_area.target_scroll_y();

    scroll_area.trim_content_bottom(200.0);

    // Offset unchanged (not near bottom)
    assert_eq!(scroll_area.target_scroll_y(), offset_before);
}
