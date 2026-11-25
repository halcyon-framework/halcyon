// SPEC-HAL-131A Stage 11: Anchor Dirtying & Re-arm Semantics Tests
//
// Tests validating state-driven anchor resolution and the dirty/clean cycle.
//
// Invariants tested:
// - INV-ANCHOR-RESOLVE-SCHED-2: Anchors are resolved at most once per dirtied cycle
//   - Anchor resolves once, stays stable across multiple frames (no re-resolve)
//   - After mark_anchor_dirty(), next resolve_anchor_if_needed resolves again

//   - Clearing anchor removes dirty state
//   - No anchor → no resolver calls

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

    fn reset_count(&self) {
        self.call_count.set(0);
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
// Test 1: Anchor resolves once, stays stable across multiple frames
// =============================================================================

#[test]
fn anchor_resolves_once_stays_stable_across_frames() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.is_anchor_dirty(), "New anchor must be dirty");

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // Frame 1: Resolve
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1, "Frame 1: resolver called once");
    assert!(scroll_area.has_anchor(), "Anchor must persist");
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Anchor must be clean after resolution"
    );
    let offset = scroll_area.logical_offset().y;
    assert_eq!(offset, 450.0, "Offset correct: 500 - 50 = 450");

    // Frame 2: No re-resolve (anchor is clean)
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "Frame 2: resolver NOT called again"
    );
    assert_eq!(scroll_area.logical_offset().y, offset, "Offset unchanged");

    // Frame 3: Still no re-resolve
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "Frame 3: resolver still NOT called"
    );
    assert_eq!(
        scroll_area.logical_offset().y,
        offset,
        "Offset still unchanged"
    );

    // Frame 4: Even more frames
    scroll_area.resolve_anchor_if_needed(&resolver);
    scroll_area.resolve_anchor_if_needed(&resolver);
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "Many frames: resolver still only called once"
    );
}

// =============================================================================
// Test 2: After mark_anchor_dirty(), next resolve resolves again
// =============================================================================

#[test]
fn mark_anchor_dirty_triggers_re_resolution() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1, "Initial resolution");
    assert!(!scroll_area.is_anchor_dirty());

    // Second call: no-op (clean)
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1, "Clean anchor: no re-resolve");

    // Mark dirty
    scroll_area.mark_anchor_dirty();
    assert!(
        scroll_area.is_anchor_dirty(),
        "Anchor must be dirty after mark_anchor_dirty"
    );

    // Third call: resolves again
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        2,
        "Dirty anchor: resolver called again"
    );
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Anchor clean after resolution"
    );

    // Fourth call: no-op again (clean)
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        2,
        "Clean anchor again: no re-resolve"
    );
}

// =============================================================================
// Test 3: Clearing anchor removes dirty state
// =============================================================================

#[test]
fn clearing_anchor_removes_dirty_state() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor (dirty)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.has_anchor());
    assert!(scroll_area.is_anchor_dirty());

    // Clear anchor
    scroll_area.clear_anchor();
    assert!(!scroll_area.has_anchor(), "Anchor must be cleared");
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Dirty state must be cleared"
    );

    // Resolve should be no-op
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 0, "No anchor: resolver not called");
}

// =============================================================================
// Test 4: No anchor → no resolver calls
// =============================================================================

#[test]
fn no_anchor_no_resolver_calls() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    scroll_area.scroll_to_y(100.0);

    assert!(!scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset().y;

    // Multiple resolve calls should all be no-ops
    scroll_area.resolve_anchor_if_needed(&resolver);
    scroll_area.resolve_anchor_if_needed(&resolver);
    scroll_area.resolve_anchor_if_needed(&resolver);

    assert_eq!(resolver.call_count(), 0, "No anchor: resolver never called");
    assert_eq!(
        scroll_area.logical_offset().y,
        offset_before,
        "Offset unchanged"
    );
}

// =============================================================================
// Test 5: Content size change marks anchor dirty
// =============================================================================

#[test]
fn content_size_change_marks_anchor_dirty() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1);
    assert!(!scroll_area.is_anchor_dirty());

    // Change content size - should mark anchor dirty
    scroll_area.set_content_size(Size::new(400.0, 3000.0));
    assert!(
        scroll_area.is_anchor_dirty(),
        "Content size change must mark anchor dirty"
    );

    // Next resolve should call resolver
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        2,
        "Dirty anchor from content change: resolver called"
    );
    assert!(!scroll_area.is_anchor_dirty());
}

// =============================================================================
// Test 6: Same content size does not mark dirty
// =============================================================================

#[test]
fn same_content_size_does_not_mark_dirty() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1);
    assert!(!scroll_area.is_anchor_dirty());

    // Set same content size - should NOT mark anchor dirty
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Same content size must not mark dirty"
    );

    // Next resolve should be no-op
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        1,
        "No dirty: resolver not called again"
    );
}

// =============================================================================
// Test 7: mark_anchor_dirty no-op without anchor
// =============================================================================

#[test]
fn mark_anchor_dirty_noop_without_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    assert!(!scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    // mark_anchor_dirty should be no-op without anchor
    scroll_area.mark_anchor_dirty();
    assert!(
        !scroll_area.is_anchor_dirty(),
        "No anchor: dirty flag must remain false"
    );

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 0, "No anchor: resolver not called");
}
