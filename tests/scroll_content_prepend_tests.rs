// SPEC-HAL-131A: Content Prepend Contract Tests (Model 2: Top Extension / History Prepend)
//
// Tests validating the scroll content prepend contract for virtualized feeds
// where older content is loaded above the viewport (e.g., chat history).
//
// Invariants tested:
// - INV-SCROLL-PREPEND-1: Top extension only (delta >= 0)
// - INV-SCROLL-PREPEND-2: Viewport stability (offset compensation)

#![allow(unused_variables)]
// - INV-SCROLL-PREPEND-3: Anchor safety (no anchor mutation)
// - INV-SCROLL-PREPEND-4: Immediate geometry update
// - INV-SCROLL-PREPEND-5: Jump safety

use halcyon::core::math::{Point, Size};
use halcyon::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor, ScrollState};

// =============================================================================
// Test 1: Offset Compensation Correctness
// =============================================================================

#[test]
fn prepend_offset_compensation_basic() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to middle
    state.set_offset_y(400.0);
    let offset_before = state.logical_offset.y;

    // Prepend 500px of content
    state.prepend_content_height(500.0);

    // INV-SCROLL-PREPEND-2: Offset MUST increase by delta
    assert_eq!(
        state.logical_offset.y,
        offset_before + 500.0,
        "Offset must increase by delta to maintain visual stability"
    );

    // Content extent updated
    assert_eq!(state.content_extent.height, 1500.0);

    // max_scroll updated: 1500 - 200 = 1300
    assert_eq!(state.max_scroll().y, 1300.0);
}

#[test]
fn prepend_while_at_top_increases_offset_from_zero() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Start at top (offset = 0)
    assert_eq!(state.logical_offset.y, 0.0);

    // Prepend content
    state.prepend_content_height(300.0);

    // INV-SCROLL-PREPEND-2: Even at top, offset must increase
    // User was viewing items 0..N, now items 0..N are at offset 300+
    // To keep viewing same items, offset must be 300
    assert_eq!(
        state.logical_offset.y, 300.0,
        "At-top prepend must increase offset to maintain visual position"
    );
}

#[test]
fn prepend_visible_range_unchanged() {
    // Simulates List behavior: visible_range is derived from logical_offset
    let mut state = ScrollState::new();
    let item_height = 40.0;
    let viewport_height = 200.0;
    let item_count = 100;

    state.set_content_extent(Size::new(400.0, item_count as f32 * item_height));
    state.set_viewport_extent(Size::new(400.0, viewport_height));

    // Scroll to item 50
    let target_offset = 50.0 * item_height;
    state.set_offset_y(target_offset);

    // Compute visible range before prepend
    // Items are now at positions: item[i] at y = i * item_height
    let first_visible_before = (state.logical_offset.y / item_height).floor() as usize;
    let visible_count = (viewport_height / item_height).ceil() as usize;

    // Prepend 20 items worth of content
    let prepend_amount = 20.0 * item_height;
    state.prepend_content_height(prepend_amount);

    // After prepend: old items are now at y = (original_index + 20) * item_height
    // But logical_offset also increased by prepend_amount
    // So visible range calculation uses new offset / item_height

    // The OLD item 50 is now at position (50 + 20) * 40 = 2800
    // But offset is now 2000 + 800 = 2800... wait, that's not right.

    // Let me reconsider: prepend adds content ABOVE.
    // If item[50] was at y=2000 before, after prepend of 800px:
    // - item[50] is now at y=2800 (shifted down by 800)
    // - offset is now 2000 + 800 = 2800
    // - So we're still looking at the same pixel position in content space!

    // The visible range in terms of NEW indices:
    let first_visible_after = (state.logical_offset.y / item_height).floor() as usize;

    // The key insight: first_visible_after includes the 20 prepended items
    // So if we were viewing item 50-54, we're now viewing items 70-74
    // (same content, different indices because 20 were prepended above)

    // For viewport stability test: the CONTENT we see is unchanged
    // The math: offset_after / item_height = (offset_before + prepend) / item_height
    //         = offset_before/item_height + prepend/item_height
    //         = first_visible_before + (prepend/item_height)

    assert_eq!(
        first_visible_after,
        first_visible_before + 20,
        "Visible index shifts by prepended item count"
    );

    // But visually we see the same content (just with different indices)
    // The items at visible indices now are the same items as before
}

// =============================================================================
// Test 2: No Visual Jump
// =============================================================================

#[test]
fn prepend_no_visual_jump_content_position() {
    let mut state = ScrollState::new();
    let item_height = 40.0;

    state.set_content_extent(Size::new(400.0, 100.0 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Position: viewing item 25 at top of viewport
    let item_index = 25;
    let item_content_y = item_index as f32 * item_height;
    state.set_offset_y(item_content_y);

    // Before prepend: item 25 is at content y = 1000
    // Visual position of item 25 = item_content_y - offset = 1000 - 1000 = 0 (top of viewport)

    let visual_position_before = item_content_y - state.logical_offset.y;
    assert_eq!(visual_position_before, 0.0);

    // Prepend 10 items
    let prepend_count = 10;
    let prepend_height = prepend_count as f32 * item_height;
    state.prepend_content_height(prepend_height);

    // After prepend: item 25 (now index 35) is at content y = 1000 + 400 = 1400
    // Offset is now 1000 + 400 = 1400
    // Visual position = 1400 - 1400 = 0 (still at top!)

    let new_item_content_y = item_content_y + prepend_height;
    let visual_position_after = new_item_content_y - state.logical_offset.y;

    assert_eq!(
        visual_position_before, visual_position_after,
        "Visual position of content must be unchanged after prepend"
    );
}

// =============================================================================
// Test 3: Anchor Preservation
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
fn prepend_preserves_anchor_identity() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set anchor
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // Prepend content
    state.prepend_content_height(500.0);

    // INV-SCROLL-PREPEND-3: Anchor must be preserved
    assert!(state.anchor.is_some(), "Prepend must not clear anchor");
    assert_eq!(
        state.anchor.unwrap().target_id,
        AnchorId::new(42),
        "Anchor identity must be preserved"
    );
}

#[test]
fn prepend_does_not_trigger_anchor_resolution() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set anchor (but don't resolve it yet)
    let anchor = ScrollAnchor::new(
        AnchorId::new(99),
        Point::new(0.0, 10.0),
        Point::new(0.0, 100.0),
    );
    state.set_anchor(anchor);

    let anchor_before = state.anchor.unwrap();

    // Prepend content
    state.prepend_content_height(500.0);

    // INV-SCROLL-PREPEND-3: Anchor not resolved or modified
    let anchor_after = state.anchor.unwrap();
    assert_eq!(anchor_before.target_id, anchor_after.target_id);
    assert_eq!(anchor_before.target_offset, anchor_after.target_offset);
    assert_eq!(anchor_before.viewport_offset, anchor_after.viewport_offset);
}

// =============================================================================
// Test 4: Scrollbar Math Correctness
// =============================================================================

#[test]
fn prepend_scrollbar_fraction_correct() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Position at 50% scroll (offset 400, max 800)
    state.set_offset_y(400.0);
    let (_, frac_before) = state.scroll_fraction();
    assert!((frac_before - 0.5).abs() < 0.01, "Should be at 50% scroll");

    // Prepend 1000px of content
    state.prepend_content_height(1000.0);

    // New geometry:
    // - content = 2000
    // - offset = 400 + 1000 = 1400
    // - max = 2000 - 200 = 1800
    // - fraction = 1400 / 1800 ≈ 0.778

    let (_, frac_after) = state.scroll_fraction();
    let expected_frac = 1400.0 / 1800.0;
    assert!(
        (frac_after - expected_frac).abs() < 0.01,
        "Scroll fraction should be {}, got {}",
        expected_frac,
        frac_after
    );
}

#[test]
fn prepend_thumb_fraction_correct() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    let (_, thumb_before) = state.thumb_fraction();
    assert!((thumb_before - 0.2).abs() < 0.01, "Thumb should be 20%");

    // Prepend 1000px
    state.prepend_content_height(1000.0);

    // New thumb = 200/2000 = 0.1
    let (_, thumb_after) = state.thumb_fraction();
    assert!(
        (thumb_after - 0.1).abs() < 0.01,
        "Thumb should shrink to 10%"
    );
}

// =============================================================================
// Test 5: Zero Delta Is Noop
// =============================================================================

#[test]
fn prepend_zero_delta_is_noop() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(300.0);

    let content_before = state.content_extent.height;
    let offset_before = state.logical_offset.y;

    state.prepend_content_height(0.0);

    assert_eq!(state.content_extent.height, content_before);
    assert_eq!(state.logical_offset.y, offset_before);
}

// =============================================================================
// Test 6: Multiple Prepends Accumulate
// =============================================================================

#[test]
fn multiple_prepends_accumulate_correctly() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);

    let initial_offset = state.logical_offset.y;

    state.prepend_content_height(100.0);
    assert_eq!(state.content_extent.height, 1100.0);
    assert_eq!(state.logical_offset.y, initial_offset + 100.0);

    state.prepend_content_height(200.0);
    assert_eq!(state.content_extent.height, 1300.0);
    assert_eq!(state.logical_offset.y, initial_offset + 300.0);

    state.prepend_content_height(300.0);
    assert_eq!(state.content_extent.height, 1600.0);
    assert_eq!(state.logical_offset.y, initial_offset + 600.0);
}

// =============================================================================
// Test 7: Jump Safety (INV-SCROLL-PREPEND-5)
// =============================================================================

#[test]
fn prepend_then_jump_uses_final_offset_only() {
    let mut state = ScrollState::new();
    let item_height = 40.0;
    let initial_items = 100;

    state.set_content_extent(Size::new(400.0, initial_items as f32 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Start at item 50
    state.set_offset_y(50.0 * item_height);

    // Prepend 50 items
    let prepend_amount = 50.0 * item_height;
    state.prepend_content_height(prepend_amount);

    // Now offset is 50*40 + 50*40 = 4000
    assert_eq!(state.logical_offset.y, 4000.0);

    // Jump to the beginning (the new items)
    state.set_offset_y(0.0);

    // INV-SCROLL-PREPEND-5: Jump goes directly to target
    assert_eq!(state.logical_offset.y, 0.0);

    // Visible range is now the prepended items (indices 0-4)
    let first_visible = (state.logical_offset.y / item_height).floor() as usize;
    assert_eq!(first_visible, 0);
}

#[test]
fn prepend_jump_no_intermediate_states() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Start at position 500
    state.set_offset_y(500.0);
    let offset_before_prepend = state.logical_offset.y;

    // Prepend (should increase offset)
    state.prepend_content_height(1000.0);
    assert_eq!(
        state.logical_offset.y,
        offset_before_prepend + 1000.0,
        "Prepend must increase offset"
    );

    // Jump to specific target (single mutation)
    state.set_offset_y(100.0);
    assert_eq!(
        state.logical_offset.y, 100.0,
        "Jump must reach target directly"
    );
}

// =============================================================================
// Test: Prepend From Empty Content
// =============================================================================

#[test]
fn prepend_from_empty_content() {
    let mut state = ScrollState::new();
    state.set_viewport_extent(Size::new(400.0, 200.0));
    // Content extent starts at zero

    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.logical_offset.y, 0.0);

    state.prepend_content_height(500.0);

    assert_eq!(state.content_extent.height, 500.0);
    // Offset increases by delta (0 + 500 = 500)
    assert_eq!(state.logical_offset.y, 500.0);
    // max_scroll = 500 - 200 = 300
    assert_eq!(state.max_scroll().y, 300.0);
}

// =============================================================================
// Test: ScrollArea Forwarding
// =============================================================================

#[test]
fn scroll_area_forwards_prepend() {
    use halcyon::core::math::Size;
    use halcyon::core::widget_tree::NodeId;
    use halcyon::widgets::scroll_area::ScrollArea;

    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);

    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 1000.0));
    scroll_area.scroll_to_y(300.0);

    let offset_before = scroll_area.target_scroll_y();

    scroll_area.prepend_content_height(500.0);

    // Offset increased by delta
    assert_eq!(scroll_area.target_scroll_y(), offset_before + 500.0);
}

// =============================================================================
// Test: Combined Extend and Prepend
// =============================================================================

#[test]
fn extend_and_prepend_work_independently() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(400.0);

    let initial_offset = state.logical_offset.y;

    // Extend (bottom growth) - offset unchanged
    state.extend_content_height(200.0);
    assert_eq!(state.content_extent.height, 1200.0);
    assert_eq!(state.logical_offset.y, initial_offset);

    // Prepend (top growth) - offset increases
    state.prepend_content_height(300.0);
    assert_eq!(state.content_extent.height, 1500.0);
    assert_eq!(state.logical_offset.y, initial_offset + 300.0);

    // Another extend - offset still at initial + 300
    state.extend_content_height(100.0);
    assert_eq!(state.content_extent.height, 1600.0);
    assert_eq!(state.logical_offset.y, initial_offset + 300.0);
}
