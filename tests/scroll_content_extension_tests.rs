// SPEC-HAL-131A: Content Extension Contract Tests (Model 1: Finite Illusion)
//
// Tests validating the scroll content extension contract for virtualized,
// unbounded feeds that grow over time.
//
// Invariants tested:
// - INV-SCROLL-EXTEND-1: Bottom extension only
// - INV-SCROLL-EXTEND-2: Viewport stability
// - INV-SCROLL-EXTEND-3: No re-layout cascade
// - INV-SCROLL-EXTEND-4: Immediate scrollbar update
// - INV-SCROLL-EXTEND-5: Jump safety

use halcyon::core::math::{Point, Size};
use halcyon::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor, ScrollState};

// =============================================================================
// Test 1: Extend While Stationary
// =============================================================================

#[test]
fn extend_while_stationary_preserves_viewport() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to middle
    state.set_offset_y(400.0);
    let offset_before = state.logical_offset.y;
    let max_before = state.max_scroll().y;

    // Extend content by 500px
    state.extend_content_height(500.0);

    // INV-SCROLL-EXTEND-2: Viewport position unchanged
    assert_eq!(
        state.logical_offset.y, offset_before,
        "Viewport offset must not change after extension"
    );

    // INV-SCROLL-EXTEND-4: max_scroll increased
    let max_after = state.max_scroll().y;
    assert_eq!(
        max_after,
        max_before + 500.0,
        "max_scroll must increase by delta"
    );

    // Content extent updated
    assert_eq!(state.content_extent.height, 1500.0);
}

#[test]
fn extend_while_stationary_visible_range_unchanged() {
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

    // Compute visible range before extension
    let first_visible_before = (state.logical_offset.y / item_height).floor() as usize;
    let visible_count = (viewport_height / item_height).ceil() as usize;
    let last_visible_before = first_visible_before + visible_count;

    // Extend content (simulating 50 more items)
    state.extend_content_height(50.0 * item_height);

    // Compute visible range after extension
    let first_visible_after = (state.logical_offset.y / item_height).floor() as usize;
    let last_visible_after = first_visible_after + visible_count;

    // INV-SCROLL-EXTEND-2: Visible range unchanged
    assert_eq!(first_visible_before, first_visible_after);
    assert_eq!(last_visible_before, last_visible_after);
}

// =============================================================================
// Test 2: Extend While At Bottom
// =============================================================================

#[test]
fn extend_at_bottom_increases_max() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to max (bottom)
    let max_before = state.max_scroll().y;
    state.set_offset_y(max_before);
    assert!(state.at_end().1, "Should be at bottom");

    let offset_before = state.logical_offset.y;

    // Extend content
    state.extend_content_height(300.0);

    // INV-SCROLL-EXTEND-4: max increased
    let max_after = state.max_scroll().y;
    assert_eq!(max_after, max_before + 300.0);

    // INV-SCROLL-EXTEND-2: Offset unchanged (user was at old bottom, stays there)
    assert_eq!(state.logical_offset.y, offset_before);

    // User is no longer at bottom (new content appeared below)
    assert!(
        !state.at_end().1,
        "Should no longer be at bottom after extension"
    );
}

#[test]
fn extend_at_bottom_can_scroll_further() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to max
    state.set_offset_y(state.max_scroll().y);
    let offset_at_old_max = state.logical_offset.y;

    // Extend content
    state.extend_content_height(500.0);

    // Can now scroll further
    state.set_offset_y(state.max_scroll().y);
    assert!(
        state.logical_offset.y > offset_at_old_max,
        "Should be able to scroll into extended area"
    );
    assert_eq!(state.logical_offset.y, 1300.0); // 1500 - 200
}

// =============================================================================
// Test 3: Extend + Programmatic Jump
// =============================================================================

#[test]
fn extend_then_jump_uses_final_offset_only() {
    // This test validates INV-SCROLL-EXTEND-5: Jump safety
    // No intermediate instantiation should occur

    let mut state = ScrollState::new();
    let item_height = 40.0;
    let initial_items = 100;

    state.set_content_extent(Size::new(400.0, initial_items as f32 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Start at top
    assert_eq!(state.logical_offset.y, 0.0);

    // Extend content significantly (100 more items)
    let extension = 100.0 * item_height;
    state.extend_content_height(extension);

    // Jump deep into the new area
    let target_item = 150; // Beyond original content
    let target_offset = target_item as f32 * item_height;
    state.set_offset_y(target_offset);

    // Verify we jumped directly to the target
    assert_eq!(state.logical_offset.y, target_offset);

    // Verify we're in the extended area
    let first_visible = (state.logical_offset.y / item_height).floor() as usize;
    assert!(
        first_visible >= 100,
        "Should be viewing extended content, got item {}",
        first_visible
    );
}

#[test]
fn extend_jump_no_intermediate_states() {
    // Tracks all offset mutations to ensure no intermediate states
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Start at position 500
    state.set_offset_y(500.0);
    let offset_before_extend = state.logical_offset.y;

    // Extend (should not change offset)
    state.extend_content_height(1000.0);
    assert_eq!(
        state.logical_offset.y, offset_before_extend,
        "Extension must not mutate offset"
    );

    // Jump to new area (single mutation)
    state.set_offset_y(2500.0);
    assert_eq!(
        state.logical_offset.y, 2500.0,
        "Jump must reach target directly"
    );
}

// =============================================================================
// Test 4: Extend With Active Anchor
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
fn extend_with_active_anchor_preserves_resolution() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set up anchor at item 25 (y=1000.0 with item_height=40)
    let anchor_id = AnchorId::new(25);
    let anchor_y = 1000.0;
    let viewport_offset_y = 50.0; // Item should appear 50px from top

    let anchor = ScrollAnchor::new(
        anchor_id,
        Point::new(0.0, 0.0),
        Point::new(0.0, viewport_offset_y),
    );
    state.set_anchor(anchor);

    // Create resolver
    let resolver = TestAnchorResolver {
        positions: vec![(anchor_id, Point::new(0.0, anchor_y))],
    };

    // Resolve anchor before extension
    state.resolve_anchor(&resolver);
    let offset_before = state.logical_offset.y;

    // Expected offset: anchor_y - viewport_offset = 1000 - 50 = 950
    assert_eq!(offset_before, 950.0);

    // Extend content below anchor (should not affect anchor resolution)
    state.extend_content_height(1000.0);

    // Anchor is now cleared (was consumed by resolve_anchor)
    // But the key point: offset was not changed by extension
    assert_eq!(
        state.logical_offset.y, offset_before,
        "Extension must not change resolved offset"
    );
}

#[test]
fn extend_does_not_trigger_anchor_resolution() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Set anchor (but don't resolve it yet)
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 100.0),
    );
    state.set_anchor(anchor);

    // Extension must not touch anchor
    state.extend_content_height(500.0);

    // INV-SCROLL-EXTEND-3: Anchor still present, not resolved or cleared
    assert!(state.anchor.is_some(), "Extension must not clear anchor");
    assert_eq!(
        state.anchor.unwrap().target_id,
        AnchorId::new(42),
        "Anchor identity must be preserved"
    );
}

// =============================================================================
// Test: Scrollbar Updates Immediately
// =============================================================================

#[test]
fn extend_updates_scrollbar_same_frame() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Position at 50% scroll
    state.set_offset_y(400.0); // 400 / 800 = 0.5
    let (_, frac_before) = state.scroll_fraction();
    let (_, thumb_before) = state.thumb_fraction();

    assert!((frac_before - 0.5).abs() < 0.01, "Should be at 50% scroll");
    assert!(
        (thumb_before - 0.2).abs() < 0.01,
        "Thumb should be 20% (200/1000)"
    );

    // Extend content
    state.extend_content_height(1000.0);

    // INV-SCROLL-EXTEND-4: Scrollbar reflects new geometry immediately
    let (_, frac_after) = state.scroll_fraction();
    let (_, thumb_after) = state.thumb_fraction();

    // New max = 2000 - 200 = 1800, offset = 400, fraction = 400/1800 ≈ 0.222
    assert!(
        frac_after < frac_before,
        "Scroll fraction should decrease after extension"
    );
    assert!(
        (frac_after - 400.0 / 1800.0).abs() < 0.01,
        "Scroll fraction should be 400/1800"
    );

    // New thumb = 200/2000 = 0.1
    assert!(
        (thumb_after - 0.1).abs() < 0.01,
        "Thumb should shrink to 10% (200/2000)"
    );
}

// =============================================================================
// Test: Edge Cases
// =============================================================================

#[test]
fn extend_zero_delta_is_noop() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(300.0);

    let content_before = state.content_extent.height;
    let offset_before = state.logical_offset.y;

    state.extend_content_height(0.0);

    assert_eq!(state.content_extent.height, content_before);
    assert_eq!(state.logical_offset.y, offset_before);
}

#[test]
fn extend_from_empty_content() {
    let mut state = ScrollState::new();
    state.set_viewport_extent(Size::new(400.0, 200.0));
    // Content extent starts at zero

    assert_eq!(state.content_extent.height, 0.0);
    assert_eq!(state.max_scroll().y, 0.0);

    state.extend_content_height(500.0);

    assert_eq!(state.content_extent.height, 500.0);
    assert_eq!(state.max_scroll().y, 300.0); // 500 - 200
}

#[test]
fn multiple_extensions_accumulate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 1000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    state.extend_content_height(100.0);
    assert_eq!(state.content_extent.height, 1100.0);

    state.extend_content_height(200.0);
    assert_eq!(state.content_extent.height, 1300.0);

    state.extend_content_height(300.0);
    assert_eq!(state.content_extent.height, 1600.0);
}

// =============================================================================
// Test: ScrollArea Forwarding
// =============================================================================

#[test]
fn scroll_area_forwards_extension() {
    use halcyon::core::math::Size;
    use halcyon::core::widget_tree::NodeId;
    use halcyon::widgets::scroll_area::ScrollArea;

    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);

    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 1000.0));
    scroll_area.scroll_to_y(300.0);

    let offset_before = scroll_area.target_scroll_y();

    scroll_area.extend_content_height(500.0);

    // Offset unchanged
    assert_eq!(scroll_area.target_scroll_y(), offset_before);
}
