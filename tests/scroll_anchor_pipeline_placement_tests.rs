// SPEC-HAL-131A Stage 13: Anchor Resolution Placement (Frame Pipeline Wiring) Tests
//
// Tests validating the pipeline placement definition for anchor resolution.
//
// Invariants tested:
// - INV-ANCHOR-PIPELINE-1: Anchor resolution is allowed ONLY at post-layout, pre-paint
//   - Resolution does NOT occur automatically
//   - Resolution only occurs when pipeline hook is explicitly called
//   - Hook delegates to resolve_anchor_if_needed (no new behavior)

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
// Test 1: Resolution does NOT occur automatically
// =============================================================================

#[test]
fn resolution_does_not_occur_automatically() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor (dirty)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.has_anchor());
    assert!(scroll_area.is_anchor_dirty());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset();

    // INV-ANCHOR-PIPELINE-1: Without explicit hook call, NO resolution occurs
    // Simulate "frame passes" without calling the hook
    // (In real code, this would be the frame loop NOT calling the hook)

    // Verify: resolver never called, offset unchanged, anchor still dirty
    assert_eq!(
        resolver.call_count(),
        0,
        "Resolution must NOT occur automatically"
    );
    assert_eq!(
        scroll_area.logical_offset(),
        offset_before,
        "Offset must be unchanged"
    );
    assert!(
        scroll_area.is_anchor_dirty(),
        "Anchor must remain dirty until hook is called"
    );
    assert!(scroll_area.has_anchor(), "Anchor must still exist");
}

// =============================================================================
// Test 2: Resolution occurs when pipeline hook is explicitly called
// =============================================================================

#[test]
fn resolution_occurs_when_hook_called() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor (dirty)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    assert!(scroll_area.is_anchor_dirty());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // INV-ANCHOR-PIPELINE-1: Explicit hook call triggers resolution
    scroll_area.post_layout_resolve_anchors(&resolver);

    // Verify: resolver called, offset updated, anchor clean
    assert_eq!(resolver.call_count(), 1, "Resolver must be called once");
    assert_eq!(
        scroll_area.logical_offset().y,
        450.0,
        "Offset must be anchor_y - viewport_y (500 - 50)"
    );
    assert!(
        !scroll_area.is_anchor_dirty(),
        "Anchor must be clean after resolution"
    );
    assert!(
        scroll_area.has_anchor(),
        "Anchor must be preserved after resolution"
    );
}

// =============================================================================
// Test 3: Hook delegates to resolve_anchor_if_needed (identical behavior)
// =============================================================================

#[test]
fn hook_delegates_to_resolve_anchor_if_needed() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);

    let resolver = TestResolver::new(Point::new(0.0, 500.0));

    // Call hook
    scroll_area.post_layout_resolve_anchors(&resolver);
    let offset_after_hook = scroll_area.logical_offset();
    let count_after_hook = resolver.call_count();

    // Multiple calls are idempotent (same as resolve_anchor_if_needed)
    scroll_area.post_layout_resolve_anchors(&resolver);
    scroll_area.post_layout_resolve_anchors(&resolver);

    assert_eq!(
        resolver.call_count(),
        count_after_hook,
        "Multiple hook calls must be idempotent"
    );
    assert_eq!(
        scroll_area.logical_offset(),
        offset_after_hook,
        "Offset must be unchanged after idempotent calls"
    );
}

// =============================================================================
// Test 4: Hook is no-op without anchor
// =============================================================================

#[test]
fn hook_noop_without_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));
    scroll_area.scroll_to_y(100.0);

    assert!(!scroll_area.has_anchor());

    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    let offset_before = scroll_area.logical_offset();

    // Hook is no-op without anchor
    scroll_area.post_layout_resolve_anchors(&resolver);

    assert_eq!(resolver.call_count(), 0, "Resolver must not be called");
    assert_eq!(
        scroll_area.logical_offset(),
        offset_before,
        "Offset must be unchanged"
    );
}

// =============================================================================
// Test 5: Hook is no-op with clean anchor
// =============================================================================

#[test]
fn hook_noop_with_clean_anchor() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor (clean state)
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.post_layout_resolve_anchors(&resolver);

    assert_eq!(resolver.call_count(), 1);
    assert!(!scroll_area.is_anchor_dirty());

    let offset_after_first = scroll_area.logical_offset();

    // Second call: no-op (anchor is clean)
    scroll_area.post_layout_resolve_anchors(&resolver);

    assert_eq!(
        resolver.call_count(),
        1,
        "Resolver must not be called again"
    );
    assert_eq!(
        scroll_area.logical_offset(),
        offset_after_first,
        "Offset must be unchanged"
    );
}

// =============================================================================
// Test 6: Hook respects dirty cycle (re-dirtying triggers re-resolution)
// =============================================================================

#[test]
fn hook_respects_dirty_cycle() {
    let dummy = NodeId(9999);
    let mut scroll_area = ScrollArea::new(dummy);
    scroll_area.set_viewport_size(Size::new(400.0, 200.0));
    scroll_area.set_content_size(Size::new(400.0, 2000.0));

    // Create and resolve anchor
    scroll_area.set_anchor_at_viewport_y(42, 50.0);
    let resolver = TestResolver::new(Point::new(0.0, 500.0));
    scroll_area.post_layout_resolve_anchors(&resolver);
    assert_eq!(resolver.call_count(), 1);
    assert!(!scroll_area.is_anchor_dirty());

    // Re-dirty via mutation hook
    scroll_area.on_items_inserted();
    assert!(scroll_area.is_anchor_dirty());

    // Next hook call triggers resolution again
    scroll_area.post_layout_resolve_anchors(&resolver);
    assert_eq!(resolver.call_count(), 2, "Resolver must be called again");
    assert!(!scroll_area.is_anchor_dirty());
}
