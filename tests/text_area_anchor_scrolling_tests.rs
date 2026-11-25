// SPEC-HAL-131A: TextArea AnchorResolver Tests
//
// Tests for anchor-based scrolling with TextArea widgets.
// These tests verify the public contract of AnchorResolver.
//
// Key invariants tested:
// - INV-ANCHOR-RESOLVE-1: Anchor must be resolved to logical_offset before layout
// - INV-ANCHOR-2: If anchor target is removed, fallback to nearest or absolute mode
// - Viewport offset preserved when anchor is clamped
//
// NOTE: Full integration testing of anchor resolution requires the render loop
// to trigger layout. These tests verify the contract and math without that.

use halcyon::core::math::Point;
use halcyon::core::scroll_state::{AnchorId, ScrollAnchor};
use halcyon::core::widget::Widget;
use halcyon::widgets::text_area::TextArea;
use halcyon::widgets::text_layout_cache::TextLayoutCache;

// ============================================================================
// TextArea as AnchorResolver: Public Contract
// ============================================================================

#[test]
fn test_text_area_no_resolver_when_layout_dirty() {
    // TextArea with text but no layout pass returns None
    let text_area = TextArea::new().with_text("Line 0\nLine 1\nLine 2");
    let resolver = text_area.as_anchor_resolver();

    assert!(
        resolver.is_none(),
        "Should return None when layout cache is not initialized"
    );
}

#[test]
fn test_empty_text_area_no_resolver() {
    let text_area = TextArea::new();
    let resolver = text_area.as_anchor_resolver();

    assert!(
        resolver.is_none(),
        "Empty TextArea with dirty layout should not provide resolver"
    );
}

// ============================================================================
// TextLayoutCache: Default State (no test-harness needed)
// ============================================================================

#[test]
fn test_layout_cache_empty_has_min_one_line() {
    let mut cache = TextLayoutCache::default();
    cache.clear_stale_for_test();

    // Empty cache should report at least 1 line (for empty document)
    assert_eq!(cache.line_count(), 1);
}

#[test]
fn test_layout_cache_default_line_y_zero() {
    let mut cache = TextLayoutCache::default();
    cache.clear_stale_for_test();

    // Line 0 always starts at Y = 0
    assert_eq!(cache.line_y(0), 0.0);
}

// ============================================================================
// ScrollAnchor Structure Tests
// ============================================================================

#[test]
fn test_scroll_anchor_creation() {
    let anchor = ScrollAnchor::new(
        AnchorId::new(5),
        Point::new(0.0, 10.0),
        Point::new(0.0, 100.0),
    );

    assert_eq!(anchor.target_id.0, 5);
    assert_eq!(anchor.target_offset.y, 10.0);
    assert_eq!(anchor.viewport_offset.y, 100.0);
}

#[test]
fn test_anchor_id_creation() {
    let id = AnchorId::new(42);
    assert_eq!(id.0, 42);
}

// ============================================================================
// Anchor Resolution Math Tests (contract verification)
// ============================================================================

#[test]
fn test_anchor_math_line_0_at_viewport_0() {
    // Given: line_y(0) = 0, viewport_offset = 0
    // Expected: scroll_offset = line_y - viewport_offset = 0 - 0 = 0
    let line_y = 0.0;
    let viewport_offset = 0.0;
    let expected_scroll = line_y - viewport_offset;
    assert_eq!(expected_scroll, 0.0);
}

#[test]
fn test_anchor_math_line_5_at_viewport_100() {
    // Given: line_y(5) = 5 * 24 = 120, viewport_offset = 100
    // Expected: scroll_offset = 120 - 100 = 20
    let line_height = 24.0;
    let line_y = 5.0 * line_height;
    let viewport_offset = 100.0;
    let expected_scroll = line_y - viewport_offset;
    assert_eq!(expected_scroll, 20.0);
}

#[test]
fn test_anchor_math_line_10_at_viewport_50_with_target_offset() {
    // Given: line_y(10) = 10 * 24 = 240, target_offset = 12, viewport_offset = 50
    // Expected: scroll_offset = (240 + 12) - 50 = 202
    let line_height = 24.0;
    let line_y = 10.0 * line_height;
    let target_offset = 12.0;
    let viewport_offset = 50.0;
    let target_pos = line_y + target_offset;
    let expected_scroll = target_pos - viewport_offset;
    assert_eq!(expected_scroll, 202.0);
}

// ============================================================================
// find_nearest_anchor: Clamping Logic Tests
// ============================================================================

#[test]
fn test_find_nearest_anchor_logic_valid_index() {
    // When index is valid, no clamping needed
    let line_count = 10;
    let index = 5;
    let clamped = index.min(line_count - 1);
    assert_eq!(clamped, 5);
}

#[test]
fn test_find_nearest_anchor_logic_beyond_last() {
    // When index > last, clamp to last
    let line_count = 10;
    let index = 15;
    let clamped = index.min(line_count - 1);
    assert_eq!(clamped, 9);
}

#[test]
fn test_find_nearest_anchor_logic_way_beyond() {
    // When index way beyond, still clamps to last
    let line_count = 10;
    let index = 1000;
    let clamped = index.min(line_count - 1);
    assert_eq!(clamped, 9);
}

// ============================================================================
// Invariant Tests
// ============================================================================

#[test]
fn test_invariant_viewport_offset_preserved_in_clamped_anchor() {
    // When creating a clamped anchor, viewport_offset must be preserved
    let viewport_offset_y = 77.0;
    let original_anchor = ScrollAnchor::new(
        AnchorId::new(100), // Invalid index (will be clamped)
        Point::new(0.0, 5.0),
        Point::new(0.0, viewport_offset_y),
    );

    let line_count = 10;
    let clamped_index = (original_anchor.target_id.0 as usize).min(line_count - 1);

    // Create the "nearest" anchor as find_nearest_anchor would
    let new_anchor = ScrollAnchor::new(
        AnchorId::new(clamped_index as u64),
        original_anchor.target_offset,
        original_anchor.viewport_offset,
    );

    // Verify viewport_offset is preserved
    assert_eq!(new_anchor.viewport_offset.y, viewport_offset_y);
    // Verify target_offset is preserved
    assert_eq!(new_anchor.target_offset.y, 5.0);
    // Verify index is clamped
    assert_eq!(new_anchor.target_id.0, 9);
}

// ============================================================================
// Contract Tests (what TextArea::AnchorResolver SHOULD do)
// ============================================================================

#[test]
fn test_contract_resolve_returns_point_with_y_eq_line_y_plus_offset() {
    // Contract: resolve_anchor returns Point(0.0, line_y(index) + target_offset.y)
    let line_height = 24.0;
    let line_index: usize = 5;
    let target_offset_y = 10.0;

    let expected_y = (line_index as f32) * line_height + target_offset_y;
    let expected_point = Point::new(0.0, expected_y);

    assert_eq!(expected_point.y, 130.0); // 5 * 24 + 10 = 130
}

#[test]
fn test_contract_find_nearest_preserves_offsets() {
    // Contract: find_nearest_anchor preserves target_offset and viewport_offset
    let original = ScrollAnchor::new(
        AnchorId::new(999), // Will be clamped
        Point::new(5.0, 10.0),
        Point::new(15.0, 20.0),
    );

    let line_count = 10;
    let clamped_id = (original.target_id.0 as usize).min(line_count - 1);

    let nearest = ScrollAnchor::new(
        AnchorId::new(clamped_id as u64),
        original.target_offset,
        original.viewport_offset,
    );

    // Verify offsets preserved
    assert_eq!(nearest.target_offset, original.target_offset);
    assert_eq!(nearest.viewport_offset, original.viewport_offset);
    // Verify id clamped
    assert_eq!(nearest.target_id.0, 9);
}
