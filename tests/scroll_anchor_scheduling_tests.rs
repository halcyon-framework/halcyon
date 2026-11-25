// SPEC-HAL-131A Stage 10: Anchor Resolution Scheduling Tests
// SPEC-HAL-131A Stage 11: Anchor Dirtying & Re-arm Semantics (adapted)
//
// Tests validating the single resolution hook for the frame pipeline.
//
// Invariants tested:
// - INV-ANCHOR-RESOLVE-SCHED-1: Anchor resolution triggered only via
//   resolve_anchor_if_needed
//   - With dirty anchor: performs exactly one jump
//   - Without anchor: no-op, resolver not called
//   - Multiple calls: idempotent (resolver called once max per dirty cycle)
// - INV-ANCHOR-RESOLVE-SCHED-2: Anchors are resolved at most once per dirtied cycle
//   - Resolution is skipped if anchor is clean

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
// Test 1: With dirty anchor → performs exactly one jump, anchor preserved
// =============================================================================

#[test]
fn resolve_anchor_if_needed_with_anchor_performs_one_jump() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Set anchor at viewport_y = 50
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.has_anchor());
    assert!(scroll_area.is_anchor_dirty(), "New anchor must be dirty");

    // Resolver returns position 500
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // INV-ANCHOR-RESOLVE-SCHED-1: Resolution via single hook
    scroll_area.resolve_anchor_if_needed(&resolver);

    // Resolver called exactly once
    assert_eq!(
        resolver.call_count(),
        1,
        "Resolver must be called exactly once"
    );

    // Offset computed correctly: 500 - 50 = 450
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 450.0, "Offset must be anchor_y - viewport_y");

    // INV-ANCHOR-RESOLVE-SCHED-2: Anchor preserved, but now clean
    assert!(
        scroll_area.has_anchor(),
        "Anchor must be preserved after resolution"
    );
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Anchor must be clean after resolution"
    );
}

// =============================================================================
// Test 2: Without anchor → no-op, resolver not called
// =============================================================================

#[test]
fn resolve_anchor_if_needed_without_anchor_is_noop() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    scroll_area.scroll_to_y(300.0);

    assert!(!scroll_area.has_anchor());
    let offset_before = scroll_area.logical_offset().y;

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // INV-ANCHOR-RESOLVE-SCHED-1: No anchor = no-op
    scroll_area.resolve_anchor_if_needed(&resolver);

    // Resolver not called
    assert_eq!(resolver.call_count(), 0, "Resolver must not be called");

    // Offset unchanged
    assert_eq!(
        scroll_area.logical_offset().y,
        offset_before,
        "Offset must not change"
    );
}

// =============================================================================
// Test 3: Multiple calls → idempotent (resolver called once max per dirty cycle)
// =============================================================================

#[test]
fn resolve_anchor_if_needed_multiple_calls_idempotent() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    scroll_area.set_anchor_at_viewport_y(42, 50.0);

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // First call: resolves and clears dirty flag (anchor preserved)
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1, "First call: resolver called once");
    let offset_after_first = scroll_area.logical_offset().y;

    // INV-ANCHOR-RESOLVE-SCHED-2: Anchor preserved but clean
    assert!(scroll_area.has_anchor(), "Anchor must be preserved");
    assert!(!scroll_area.is_anchor_dirty(), "Anchor must be clean");

    // Second call: no-op (anchor is clean)
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "Second call: resolver NOT called again"
    );
    assert_eq!(
        scroll_area.logical_offset().y,
        offset_after_first,
        "Offset unchanged"
    );

    // Third call: still no-op
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "Third call: resolver still NOT called"
    );
}

// =============================================================================
// Test 4: Offset computed correctly
// =============================================================================

#[test]
fn resolve_anchor_if_needed_offset_correct() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Anchor at viewport_y = 75
    scroll_area.set_anchor_at_viewport_y(100, 75.0);

    // Resolver returns position 800
    let resolver = TestResolver::new(Point::new(0.0, 800.0));

    scroll_area.resolve_anchor_if_needed(&resolver);

    // Expected offset: 800 - 75 = 725
    let offset = scroll_area.logical_offset();
    assert_eq!(offset.y, 725.0);
}

// =============================================================================
// Test 5: New anchor after resolution can be resolved
// =============================================================================

#[test]
fn new_anchor_after_resolution_can_be_resolved() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // First anchor
    scroll_area.set_anchor_at_viewport_y(10, 20.0);
    let resolver = TestResolver::new(Point::new(0.0, 300.0));

    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1);
    assert_eq!(scroll_area.logical_offset().y, 280.0); // 300 - 20

    // Anchor preserved after resolution (Stage 11)
    assert!(scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    // Set new anchor (replaces existing, marks dirty again)
    scroll_area.set_anchor_at_viewport_y(50, 100.0);
    assert!(scroll_area.has_anchor());
    assert!(scroll_area.is_anchor_dirty(), "New anchor must be dirty");

    // New resolver for second anchor
    let resolver2 = TestResolver::new(Point::new(0.0, 600.0));

    scroll_area.resolve_anchor_if_needed(&resolver2);
    assert_eq!(resolver2.call_count(), 1);
    assert_eq!(scroll_area.logical_offset().y, 500.0); // 600 - 100
}
