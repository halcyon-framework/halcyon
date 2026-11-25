// SPEC-HAL-131A Stage 12: Anchor Invalidation Sources Tests
//
// Tests validating the explicit invalidation hooks for content mutations.
//
// Invariants tested:
// - INV-ANCHOR-DIRTY-SOURCES-1: Anchor dirtying occurs only via:
//   - set_anchor_* (new anchor)
//   - set_content_extent (size change)
//   - explicit mutation hooks (on_items_*)
//
// Test coverage:
// - Each hook marks anchor dirty when anchor exists
// - No anchor → no dirty
// - Hooks do NOT resolve anchor
// - Hooks do NOT clear anchor

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
// Test 1: on_items_inserted marks anchor dirty when anchor exists
// =============================================================================

#[test]
fn on_items_inserted_marks_dirty_with_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor (clean state)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Anchor must be clean after resolution"
    );

    // on_items_inserted marks dirty
    scroll_area.on_items_inserted();
    assert!(
        scroll_area.is_anchor_dirty(),
        "on_items_inserted must mark anchor dirty"
    );
    assert!(scroll_area.has_anchor(), "Anchor must still exist");
}

// =============================================================================
// Test 2: on_items_removed marks anchor dirty when anchor exists
// =============================================================================

#[test]
fn on_items_removed_marks_dirty_with_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor (clean state)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert!(!scroll_area.is_anchor_dirty());

    // on_items_removed marks dirty
    scroll_area.on_items_removed();
    assert!(
        scroll_area.is_anchor_dirty(),
        "on_items_removed must mark anchor dirty"
    );
    assert!(scroll_area.has_anchor(), "Anchor must still exist");
}

// =============================================================================
// Test 3: on_items_resized marks anchor dirty when anchor exists
// =============================================================================

#[test]
fn on_items_resized_marks_dirty_with_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor (clean state)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert!(!scroll_area.is_anchor_dirty());

    // on_items_resized marks dirty
    scroll_area.on_items_resized();
    assert!(
        scroll_area.is_anchor_dirty(),
        "on_items_resized must mark anchor dirty"
    );
    assert!(scroll_area.has_anchor(), "Anchor must still exist");
}

// =============================================================================
// Test 4: on_items_inserted no-op without anchor
// =============================================================================

#[test]
fn on_items_inserted_noop_without_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    assert!(!scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    scroll_area.on_items_inserted();

    assert!(
        !scroll_area.is_anchor_dirty(),
        "No anchor: dirty flag must remain false"
    );
    assert!(!scroll_area.has_anchor());
}

// =============================================================================
// Test 5: on_items_removed no-op without anchor
// =============================================================================

#[test]
fn on_items_removed_noop_without_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    assert!(!scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    scroll_area.on_items_removed();

    assert!(
        !scroll_area.is_anchor_dirty(),
        "No anchor: dirty flag must remain false"
    );
    assert!(!scroll_area.has_anchor());
}

// =============================================================================
// Test 6: on_items_resized no-op without anchor
// =============================================================================

#[test]
fn on_items_resized_noop_without_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    assert!(!scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    scroll_area.on_items_resized();

    assert!(
        !scroll_area.is_anchor_dirty(),
        "No anchor: dirty flag must remain false"
    );
    assert!(!scroll_area.has_anchor());
}

// =============================================================================
// Test 7: Hooks do NOT resolve anchor (resolver not called)
// =============================================================================

#[test]
fn hooks_do_not_resolve_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor (dirty)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.is_anchor_dirty());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset();

    // Call all three hooks - none should resolve anchor
    scroll_area.on_items_inserted();
    scroll_area.on_items_removed();
    scroll_area.on_items_resized();

    // Resolver never called
    assert_eq!(resolver.call_count(), 0, "Hooks must NOT call resolver");

    // Offset unchanged (no resolution happened)
    assert_eq!(
        scroll_area.logical_offset(),
        offset_before,
        "Offset must be unchanged"
    );

    // Anchor still exists and still dirty
    assert!(scroll_area.has_anchor());
    assert!(scroll_area.is_anchor_dirty());
}

// =============================================================================
// Test 8: Hooks do NOT clear anchor
// =============================================================================

#[test]
fn hooks_do_not_clear_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert!(scroll_area.has_anchor());
    assert!(!scroll_area.is_anchor_dirty());

    // Call all three hooks - none should clear anchor
    scroll_area.on_items_inserted();
    assert!(
        scroll_area.has_anchor(),
        "on_items_inserted must NOT clear anchor"
    );

    // Reset to clean for next test
    scroll_area.resolve_anchor_if_needed(&resolver);

    scroll_area.on_items_removed();
    assert!(
        scroll_area.has_anchor(),
        "on_items_removed must NOT clear anchor"
    );

    // Reset to clean for next test
    scroll_area.resolve_anchor_if_needed(&resolver);

    scroll_area.on_items_resized();
    assert!(
        scroll_area.has_anchor(),
        "on_items_resized must NOT clear anchor"
    );
}

// =============================================================================
// Test 9: Multiple hook calls accumulate dirty (idempotent)
// =============================================================================

#[test]
fn multiple_hook_calls_idempotent() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 1);

    // Multiple hook calls
    scroll_area.on_items_inserted();
    scroll_area.on_items_inserted();
    scroll_area.on_items_removed();
    scroll_area.on_items_resized();
    scroll_area.on_items_resized();

    // Still only need one resolution
    assert!(scroll_area.is_anchor_dirty());
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(
        resolver.call_count(),
        2,
        "Multiple dirty marks → one resolution"
    );
    assert!(!scroll_area.is_anchor_dirty());
}

// =============================================================================
// Test 10: Hook after resolution triggers re-resolution
// =============================================================================

#[test]
fn hook_after_resolution_triggers_re_resolution() {
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

    // Hook marks dirty
    scroll_area.on_items_inserted();
    assert!(scroll_area.is_anchor_dirty());

    // Next resolve calls resolver again
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 2);
    assert!(!scroll_area.is_anchor_dirty());

    // Another hook
    scroll_area.on_items_removed();
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 3);

    // And another
    scroll_area.on_items_resized();
    scroll_area.resolve_anchor_if_needed(&resolver);
    assert_eq!(resolver.call_count(), 4);
}
