// SPEC-HAL-131A: Anchor Invalidation Contract Tests (Model 4)
//
// Tests validating the explicit anchor invalidation contract.
//
// Invariants tested:
// - INV-ANCHOR-INV-1: Explicit invalidation only
// - INV-ANCHOR-INV-2: Predicate-based
// - INV-ANCHOR-INV-3: No side effects
// - INV-ANCHOR-INV-4: Idempotent

use halcyon::core::math::{Point, Size};
use halcyon::core::scroll_state::{AnchorId, AnchorResolver, ScrollAnchor, ScrollState};

// =============================================================================
// Test 1: Anchor Cleared When Predicate Matches
// =============================================================================

#[test]
fn anchor_cleared_when_predicate_returns_true() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 100.0),
    );
    state.set_anchor(anchor);
    assert!(state.anchor.is_some());

    // INV-ANCHOR-INV-2: Predicate returns true → cleared
    state.invalidate_anchor_if(|a| a.target_id.0 == 42);

    assert!(state.anchor.is_none());
}

#[test]
fn anchor_cleared_by_range_predicate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 4000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Anchor at item 15
    let anchor = ScrollAnchor::new(
        AnchorId::new(15),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // Simulate trim of items 0-19 (20 items)
    let trimmed_count = 20u64;
    state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_count);

    // 15 < 20, so anchor should be cleared
    assert!(state.anchor.is_none());
}

// =============================================================================
// Test 2: Anchor Preserved When Predicate Fails
// =============================================================================

#[test]
fn anchor_preserved_when_predicate_returns_false() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 10.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // INV-ANCHOR-INV-2: Predicate returns false → preserved
    state.invalidate_anchor_if(|a| a.target_id.0 == 999);

    assert!(state.anchor.is_some());
    let a = state.anchor.unwrap();
    assert_eq!(a.target_id, AnchorId::new(42));
    assert_eq!(a.target_offset, Point::new(0.0, 10.0));
    assert_eq!(a.viewport_offset, Point::new(0.0, 50.0));
}

#[test]
fn anchor_preserved_by_range_predicate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 4000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Anchor at item 50
    let anchor = ScrollAnchor::new(
        AnchorId::new(50),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // Simulate trim of items 0-19 (20 items)
    let trimmed_count = 20u64;
    state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_count);

    // 50 >= 20, so anchor should be preserved
    assert!(state.anchor.is_some());
    assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(50));
}

// =============================================================================
// Test 3: Offset Unchanged After Invalidation
// =============================================================================

#[test]
fn offset_unchanged_when_anchor_cleared() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);
    state.set_offset_x(100.0);

    let offset_before = state.logical_offset;

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // INV-ANCHOR-INV-3: No offset mutation
    state.invalidate_anchor_if(|_| true);

    assert!(state.anchor.is_none());
    assert_eq!(
        state.logical_offset, offset_before,
        "Offset must not change"
    );
}

#[test]
fn offset_unchanged_when_anchor_preserved() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);

    let offset_before = state.logical_offset;

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // INV-ANCHOR-INV-3: No offset mutation even when anchor preserved
    state.invalidate_anchor_if(|_| false);

    assert!(state.anchor.is_some());
    assert_eq!(
        state.logical_offset, offset_before,
        "Offset must not change"
    );
}

// =============================================================================
// Test 4: No Resolver Invocation
// =============================================================================

/// Test resolver that panics if called
struct PanicResolver;

impl AnchorResolver for PanicResolver {
    fn resolve_anchor(&self, _anchor: &ScrollAnchor) -> Option<Point> {
        panic!("Resolver should never be called during invalidation");
    }

    fn find_nearest_anchor(&self, _anchor: &ScrollAnchor) -> Option<ScrollAnchor> {
        panic!("Resolver should never be called during invalidation");
    }
}

#[test]
fn invalidation_does_not_invoke_resolver() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 50.0),
    );
    state.set_anchor(anchor);

    // INV-ANCHOR-INV-3: No resolver calls
    // This would panic if resolver was invoked
    let _resolver = PanicResolver;

    // Note: invalidate_anchor_if doesn't take a resolver parameter,
    // proving by design that it cannot call one
    state.invalidate_anchor_if(|_| true);

    assert!(state.anchor.is_none());
}

// =============================================================================
// Test 5: Safe When No Anchor Exists
// =============================================================================

#[test]
fn no_anchor_is_safe_noop() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(300.0);

    assert!(state.anchor.is_none());

    let offset_before = state.logical_offset;
    let content_before = state.content_extent;
    let viewport_before = state.viewport_extent;

    // INV-ANCHOR-INV-4: No anchor = no-op
    state.invalidate_anchor_if(|_| true);

    assert!(state.anchor.is_none());
    assert_eq!(state.logical_offset, offset_before);
    assert_eq!(state.content_extent, content_before);
    assert_eq!(state.viewport_extent, viewport_before);
}

#[test]
fn predicate_not_called_when_no_anchor() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    assert!(state.anchor.is_none());

    let mut called = false;
    state.invalidate_anchor_if(|_| {
        called = true;
        true
    });

    // INV-ANCHOR-INV-4: Predicate should not be invoked when no anchor
    assert!(
        !called,
        "Predicate should not be called when no anchor exists"
    );
}

// =============================================================================
// Test 6: Typical Usage Scenario (Trim + Invalidate)
// =============================================================================

#[test]
fn trim_then_invalidate_removes_stale_anchor() {
    let mut state = ScrollState::new();
    let item_height = 40.0;
    let item_count = 100;
    state.set_content_extent(Size::new(400.0, item_count as f32 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to view items 50-54
    state.set_offset_y(50.0 * item_height);

    // Anchor on item 10 (above viewport)
    let anchor = ScrollAnchor::new(
        AnchorId::new(10),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // Trim top 20 items
    let trim_delta = 20.0 * item_height;
    state.trim_content_top(trim_delta);

    // INV-SCROLL-TRIM-4: Trim does NOT touch anchor
    assert!(state.anchor.is_some());

    // Now caller must explicitly invalidate stale anchors
    let trimmed_count = 20u64;
    state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_count);

    // Item 10 < 20, so anchor should be cleared
    assert!(state.anchor.is_none());
}

#[test]
fn trim_then_invalidate_preserves_valid_anchor() {
    let mut state = ScrollState::new();
    let item_height = 40.0;
    let item_count = 100;
    state.set_content_extent(Size::new(400.0, item_count as f32 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Scroll to view items 50-54
    state.set_offset_y(50.0 * item_height);

    // Anchor on item 50 (in viewport)
    let anchor = ScrollAnchor::new(
        AnchorId::new(50),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // Trim top 20 items
    let trim_delta = 20.0 * item_height;
    state.trim_content_top(trim_delta);

    // Caller invalidates stale anchors
    let trimmed_count = 20u64;
    state.invalidate_anchor_if(|a| a.target_id.0 < trimmed_count);

    // Item 50 >= 20, anchor should still be there
    assert!(state.anchor.is_some());
    assert_eq!(state.anchor.unwrap().target_id, AnchorId::new(50));
}

#[test]
fn multiple_trims_with_progressive_invalidation() {
    let mut state = ScrollState::new();
    let item_height = 40.0;
    state.set_content_extent(Size::new(400.0, 100.0 * item_height));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(50.0 * item_height);

    // Anchor on item 35
    let anchor = ScrollAnchor::new(
        AnchorId::new(35),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // Trim 1: Remove items 0-9
    state.trim_content_top(10.0 * item_height);
    state.invalidate_anchor_if(|a| a.target_id.0 < 10);
    assert!(
        state.anchor.is_some(),
        "Anchor at 35 should survive trim of 0-9"
    );

    // Trim 2: Remove items 10-19 (now at logical 0-9)
    state.trim_content_top(10.0 * item_height);
    state.invalidate_anchor_if(|a| a.target_id.0 < 20);
    assert!(
        state.anchor.is_some(),
        "Anchor at 35 should survive trim of 10-19"
    );

    // Trim 3: Remove items 20-29
    state.trim_content_top(10.0 * item_height);
    state.invalidate_anchor_if(|a| a.target_id.0 < 30);
    assert!(
        state.anchor.is_some(),
        "Anchor at 35 should survive trim of 20-29"
    );

    // Trim 4: Remove items 30-39 (this includes item 35)
    state.trim_content_top(10.0 * item_height);
    state.invalidate_anchor_if(|a| a.target_id.0 < 40);
    assert!(
        state.anchor.is_none(),
        "Anchor at 35 should be cleared after trim of 30-39"
    );
}

// =============================================================================
// Test: Edge Cases
// =============================================================================

#[test]
fn invalidate_with_complex_predicate() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    // Anchor with specific viewport offset
    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(10.0, 20.0),
        Point::new(30.0, 40.0),
    );
    state.set_anchor(anchor);

    // Complex predicate checking multiple fields
    state.invalidate_anchor_if(|a| a.target_id.0 == 42 && a.viewport_offset.y > 30.0);

    // 42 == 42 and 40.0 > 30.0, so anchor should be cleared
    assert!(state.anchor.is_none());
}

#[test]
fn invalidate_preserves_geometry() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));
    state.set_offset_y(500.0);

    let content_before = state.content_extent;
    let viewport_before = state.viewport_extent;
    let max_before = state.max_scroll();

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    state.invalidate_anchor_if(|_| true);

    // INV-ANCHOR-INV-3: No geometry changes
    assert_eq!(state.content_extent, content_before);
    assert_eq!(state.viewport_extent, viewport_before);
    assert_eq!(state.max_scroll(), max_before);
}

#[test]
fn idempotent_multiple_calls() {
    let mut state = ScrollState::new();
    state.set_content_extent(Size::new(400.0, 2000.0));
    state.set_viewport_extent(Size::new(400.0, 200.0));

    let anchor = ScrollAnchor::new(
        AnchorId::new(42),
        Point::new(0.0, 0.0),
        Point::new(0.0, 0.0),
    );
    state.set_anchor(anchor);

    // First call clears
    state.invalidate_anchor_if(|_| true);
    assert!(state.anchor.is_none());

    // INV-ANCHOR-INV-4: Second call is idempotent
    state.invalidate_anchor_if(|_| true);
    assert!(state.anchor.is_none());

    // Third call with different predicate is still safe
    state.invalidate_anchor_if(|a| a.target_id.0 < 100);
    assert!(state.anchor.is_none());
}
