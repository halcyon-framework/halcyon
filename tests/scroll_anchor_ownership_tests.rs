// SPEC-HAL-131A Stage 8: Anchor Ownership & Call-Site Enforcement Tests
//
// Tests validating the intent-named wrappers for explicit anchor lifecycle ownership.
//
// Invariants tested:
// - INV-ANCHOR-OWN-1: Call sites must choose ownership explicitly
//   - jump_to_anchor_and_keep → anchor preserved
//   - jump_to_anchor_and_clear → anchor cleared
//   - No boolean flags exposed to UI / widget code paths

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
}

impl TestResolver {
    fn new(position: Point) -> Self {
        Self {
            position,
            call_count: Cell::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.get()
    }
}

impl AnchorResolver for TestResolver {
    fn resolve_anchor(&self, _anchor: &ScrollAnchor) -> Option<Point> {
        self.call_count.set(self.call_count.get() + 1);
        Some(self.position)
    }

    fn find_nearest_anchor(&self, _anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        None
    }
}

// =============================================================================
// Test 1: jump_to_anchor_and_keep preserves anchor
// =============================================================================

#[test]
fn jump_to_anchor_and_keep_preserves_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Set anchor
    scroll_area.set_anchor_row(42, 50.0);
    assert!(scroll_area.has_anchor());

    // Resolver returns position 500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // INV-ANCHOR-OWN-1: jump_to_anchor_and_keep preserves anchor
    let jumped = scroll_area.jump_to_anchor_and_keep(&resolver);

    assert!(jumped, "Should have jumped");
    assert!(
        scroll_area.has_anchor(),
        "Anchor must be preserved after jump_to_anchor_and_keep"
    );
}

// =============================================================================
// Test 2: jump_to_anchor_and_clear clears anchor
// =============================================================================

#[test]
fn jump_to_anchor_and_clear_clears_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Set anchor
    scroll_area.set_anchor_row(42, 50.0);
    assert!(scroll_area.has_anchor());

    // Resolver returns position 500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // INV-ANCHOR-OWN-1: jump_to_anchor_and_clear clears anchor
    let jumped = scroll_area.jump_to_anchor_and_clear(&resolver);

    assert!(jumped, "Should have jumped");
    assert!(
        !scroll_area.has_anchor(),
        "Anchor must be cleared after jump_to_anchor_and_clear"
    );
}

// =============================================================================
// Test 3: Resolver called once in _keep path
// =============================================================================

#[test]
fn jump_to_anchor_and_keep_resolver_called_once() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    scroll_area.set_anchor_row(42, 50.0);

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.jump_to_anchor_and_keep(&resolver);

    // INV-ANCHOR-COMMIT-4 (inherited): Resolver called exactly once
    assert_eq!(
        resolver.call_count(),
        1,
        "Resolver must be called exactly once"
    );
}

// =============================================================================
// Test 4: Resolver called once in _clear path
// =============================================================================

#[test]
fn jump_to_anchor_and_clear_resolver_called_once() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    scroll_area.set_anchor_row(42, 50.0);

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.jump_to_anchor_and_clear(&resolver);

    // INV-ANCHOR-COMMIT-4 (inherited): Resolver called exactly once
    assert_eq!(
        resolver.call_count(),
        1,
        "Resolver must be called exactly once"
    );
}

// =============================================================================
// Test 5: Offset correct in _keep path (identical to Stage 7)
// =============================================================================

#[test]
fn jump_to_anchor_and_keep_offset_correct() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Anchor with viewport_offset = 50
    scroll_area.set_anchor_row(42, 50.0);

    // Resolver returns anchor at y=500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.jump_to_anchor_and_keep(&resolver);

    // Expected: 500 - 50 = 450 (same as Stage 7)
    let offset = scroll_area.logical_offset();
    assert_eq!(
        offset.y, 450.0,
        "Offset must be anchor_y - viewport_offset = 500 - 50 = 450"
    );
}

// =============================================================================
// Test 6: Offset correct in _clear path (identical to Stage 7)
// =============================================================================

#[test]
fn jump_to_anchor_and_clear_offset_correct() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Anchor with viewport_offset = 50
    scroll_area.set_anchor_row(42, 50.0);

    // Resolver returns anchor at y=500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.jump_to_anchor_and_clear(&resolver);

    // Expected: 500 - 50 = 450 (same as Stage 7)
    let offset = scroll_area.logical_offset();
    assert_eq!(
        offset.y, 450.0,
        "Offset must be anchor_y - viewport_offset = 500 - 50 = 450"
    );
}

// =============================================================================
// Test 7: No anchor = no-op for _keep
// =============================================================================

#[test]
fn jump_to_anchor_and_keep_no_anchor_noop() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    scroll_area.scroll_to_y(300.0);

    assert!(!scroll_area.has_anchor());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset().y;

    let jumped = scroll_area.jump_to_anchor_and_keep(&resolver);

    assert!(!jumped, "Should not jump without anchor");
    assert_eq!(
        scroll_area.logical_offset().y,
        offset_before,
        "Offset must not change"
    );
    assert_eq!(resolver.call_count(), 0, "Resolver must not be called");
}

// =============================================================================
// Test 8: No anchor = no-op for _clear
// =============================================================================

#[test]
fn jump_to_anchor_and_clear_no_anchor_noop() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    scroll_area.scroll_to_y(300.0);

    assert!(!scroll_area.has_anchor());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset().y;

    let jumped = scroll_area.jump_to_anchor_and_clear(&resolver);

    assert!(!jumped, "Should not jump without anchor");
    assert_eq!(
        scroll_area.logical_offset().y,
        offset_before,
        "Offset must not change"
    );
    assert_eq!(resolver.call_count(), 0, "Resolver must not be called");
}
