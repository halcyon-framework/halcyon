// SPEC-HAL-131A Stage 9: Anchor Creation Contract Tests
//
// Tests validating the centralized anchor creation API.
//
// Invariants tested:
// - INV-ANCHOR-CREATE-1: Anchors created only via intent-named APIs
//   - set_anchor_at_viewport_y is the canonical creation API
//   - Calling twice replaces the existing anchor (last write wins)

use halcyon::core::math::{Point, Size};
use halcyon::core::scroll_state::{AnchorResolver, ScrollAnchor};
use halcyon::core::widget_tree::NodeId;
use halcyon::widgets::scroll_area::ScrollArea;
use std::cell::Cell;

// =============================================================================
// Test Resolver
// =============================================================================

struct TestResolver {
    position: Point,
    call_count: Cell<usize>,
    last_target_id: Cell<u64>,
}

impl TestResolver {
    fn new(position: Point) -> Self {
        Self {
            position,
            call_count: Cell::new(0),
            last_target_id: Cell::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.get()
    }

    fn last_target_id(&self) -> u64 {
        self.last_target_id.get()
    }
}

impl AnchorResolver for TestResolver {
    fn resolve_anchor(&self, anchor: &ScrollAnchor) -> Option<Point> {
        self.call_count.set(self.call_count.get() + 1);
        self.last_target_id.set(anchor.target_id.0);
        Some(self.position)
    }

    fn find_nearest_anchor(&self, _anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        None
    }
}

// =============================================================================
// Test 1: Anchor can be created via set_anchor_at_viewport_y
// =============================================================================

#[test]
fn anchor_created_via_set_anchor_at_viewport_y() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    assert!(!scroll_area.has_anchor());

    // INV-ANCHOR-CREATE-1: Create anchor via canonical API
    scroll_area.set_anchor_at_viewport_y(42, 50.0);

    assert!(scroll_area.has_anchor(), "Anchor must be created");
}

// =============================================================================
// Test 2: Created anchor resolves correctly
// =============================================================================

#[test]
fn created_anchor_resolves_correctly() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor at viewport_y = 50
    scroll_area.set_anchor_at_viewport_y(42, 50.0);

    // Resolver returns position 500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // Jump should work
    let jumped = scroll_area.jump_to_anchor_and_keep(&resolver);

    assert!(jumped, "Jump should succeed");
    assert_eq!(resolver.call_count(), 1, "Resolver called once");
    assert_eq!(resolver.last_target_id(), 42, "Correct target_id passed");

    // Expected offset: 500 - 50 = 450
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 450.0, "Offset must be anchor_y - viewport_y");
}

// =============================================================================
// Test 3: Created anchor jumps and clears correctly
// =============================================================================

#[test]
fn created_anchor_jump_and_clear() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    scroll_area.set_anchor_at_viewport_y(100, 75.0);
    assert!(scroll_area.has_anchor());

    let resolver = TestResolver::new(Point::new(0.0, 800.0));

    let jumped = scroll_area.jump_to_anchor_and_clear(&resolver);

    assert!(jumped, "Jump should succeed");
    assert!(!scroll_area.has_anchor(), "Anchor must be cleared");

    // Expected offset: 800 - 75 = 725
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 725.0);
}

// =============================================================================
// Test 4: Calling creation twice replaces anchor (last write wins)
// =============================================================================

#[test]
fn calling_twice_replaces_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create first anchor
    scroll_area.set_anchor_at_viewport_y(10, 20.0);
    assert!(scroll_area.has_anchor());

    // INV-ANCHOR-CREATE-1: Create second anchor (replaces first)
    scroll_area.set_anchor_at_viewport_y(99, 100.0);
    assert!(scroll_area.has_anchor());

    // Resolver returns position 600
    let resolver = TestResolver::new(Point::new(0.0, 600.0));

    scroll_area.jump_to_anchor_and_keep(&resolver);

    // Should use the SECOND anchor (target_id = 99, viewport_y = 100)
    assert_eq!(
        resolver.last_target_id(),
        99,
        "Second anchor must replace first"
    );

    // Expected offset: 600 - 100 = 500 (using second anchor's viewport_y)
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 500.0, "Offset computed from second anchor");
}

// =============================================================================
// Test 5: Different viewport_y values produce correct offsets
// =============================================================================

#[test]
fn viewport_y_zero_means_anchor_at_top() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // viewport_y = 0 means anchor should appear at top of viewport
    scroll_area.set_anchor_at_viewport_y(42, 0.0);

    let resolver = TestResolver::new(Point::new(0.0, 300.0));
    scroll_area.jump_to_anchor_and_keep(&resolver);

    // Expected offset: 300 - 0 = 300
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 300.0, "Anchor at viewport top");
}

#[test]
fn viewport_y_matches_viewport_height_means_anchor_at_bottom() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // viewport_y = 200 means anchor should appear at bottom of viewport
    scroll_area.set_anchor_at_viewport_y(42, 200.0);

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.jump_to_anchor_and_keep(&resolver);

    // Expected offset: 500 - 200 = 300
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 300.0, "Anchor at viewport bottom");
}

// =============================================================================
// Test 6: Creation followed by clear works
// =============================================================================

#[test]
fn create_then_clear() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.has_anchor());

    scroll_area.clear_anchor();
    assert!(!scroll_area.has_anchor());

    // Jump should be no-op
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let jumped = scroll_area.jump_to_anchor_and_keep(&resolver);

    assert!(!jumped, "No anchor means no jump");
    assert_eq!(resolver.call_count(), 0, "Resolver not called");
}
