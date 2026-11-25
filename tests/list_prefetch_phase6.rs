// SPEC-HAL-134 Phase 6: Prefetch Policy Formalization Tests
//
// Tests for compute_realized_range_from_visible() invariants:
// - INV-PREFETCH-1: Single Authority (visible_range + prefetch only)
// - INV-PREFETCH-2: Symmetric Expansion
// - INV-PREFETCH-3: Hard Clamping
// - INV-PREFETCH-4: Determinism
// - INV-PREFETCH-5: No Shadow Logic (tested implicitly by single function)

use std::ops::Range;

/// Mirror of the pure helper from list.rs for testing.
/// This must match the implementation exactly.
fn compute_realized_range_from_visible(
    visible_range: &Range<usize>,
    item_count: usize,
    prefetch: usize,
) -> Range<usize> {
    let start = visible_range.start.saturating_sub(prefetch);
    let end = (visible_range.end + prefetch).min(item_count);
    start..end
}

// =============================================================================
// INV-PREFETCH-2: Symmetric Expansion
// =============================================================================

#[test]
fn test_symmetric_expansion_middle() {
    // Visible range in the middle, no clamping
    let visible = 10..15;
    let item_count = 100;
    let prefetch = 2;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    // Should expand symmetrically by ±2
    assert_eq!(realized.start, 8);
    assert_eq!(realized.end, 17);

    // Verify symmetry
    let before = visible.start - realized.start;
    let after = realized.end - visible.end;
    assert_eq!(before, after, "Expansion must be symmetric");
    assert_eq!(before, prefetch, "Expansion must equal prefetch");
}

#[test]
fn test_symmetric_expansion_zero_prefetch() {
    // Zero prefetch = realized equals visible
    let visible = 5..10;
    let item_count = 100;
    let prefetch = 0;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized, visible);
}

#[test]
fn test_symmetric_expansion_large_prefetch() {
    // Large prefetch, still symmetric when not clamped
    let visible = 50..60;
    let item_count = 200;
    let prefetch = 20;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 30);
    assert_eq!(realized.end, 80);

    let before = visible.start - realized.start;
    let after = realized.end - visible.end;
    assert_eq!(before, after);
}

// =============================================================================
// INV-PREFETCH-3: Hard Clamping
// =============================================================================

#[test]
fn test_clamping_at_start() {
    // Visible range near start, prefetch would go negative
    let visible = 1..5;
    let item_count = 100;
    let prefetch = 3;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    // Start clamped to 0, end not clamped
    assert_eq!(realized.start, 0, "Start must clamp to 0");
    assert_eq!(realized.end, 8, "End should expand normally");
}

#[test]
fn test_clamping_at_end() {
    // Visible range near end, prefetch would exceed item_count
    let visible = 95..100;
    let item_count = 100;
    let prefetch = 3;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    // Start not clamped, end clamped to item_count
    assert_eq!(realized.start, 92, "Start should expand normally");
    assert_eq!(realized.end, 100, "End must clamp to item_count");
}

#[test]
fn test_clamping_both_ends() {
    // Small list, prefetch would clamp both ends
    let visible = 1..3;
    let item_count = 5;
    let prefetch = 3;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 0, "Start must clamp to 0");
    assert_eq!(realized.end, 5, "End must clamp to item_count");
}

#[test]
fn test_no_overflow_on_large_prefetch() {
    // Prefetch larger than list doesn't cause overflow
    let visible = 0..5;
    let item_count = 10;
    let prefetch = 100;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 0);
    assert_eq!(realized.end, 10);
}

#[test]
fn test_no_negative_indices() {
    // Start at 0 with any prefetch
    let visible = 0..5;
    let item_count = 100;
    let prefetch = 10;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 0, "Start must never be negative");
    assert!(realized.start <= realized.end, "Range must be well-formed");
}

// =============================================================================
// INV-PREFETCH-4: Determinism
// =============================================================================

#[test]
fn test_determinism_same_inputs_same_outputs() {
    let visible = 10..20;
    let item_count = 100;
    let prefetch = 5;

    // Call multiple times with identical inputs
    let result1 = compute_realized_range_from_visible(&visible, item_count, prefetch);
    let result2 = compute_realized_range_from_visible(&visible, item_count, prefetch);
    let result3 = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn test_determinism_different_inputs_may_differ() {
    let item_count = 100;
    let prefetch = 2;

    let result1 = compute_realized_range_from_visible(&(10..15), item_count, prefetch);
    let result2 = compute_realized_range_from_visible(&(20..25), item_count, prefetch);

    assert_ne!(
        result1, result2,
        "Different inputs should produce different outputs"
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_empty_visible_range() {
    let visible = 5..5; // Empty range
    let item_count = 100;
    let prefetch = 2;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    // Empty visible still expands by prefetch on both "sides"
    assert_eq!(realized.start, 3);
    assert_eq!(realized.end, 7);
}

#[test]
fn test_single_item_visible() {
    let visible = 10..11;
    let item_count = 100;
    let prefetch = 3;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 7);
    assert_eq!(realized.end, 14);
}

#[test]
fn test_zero_items() {
    let visible = 0..0;
    let item_count = 0;
    let prefetch = 5;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    assert_eq!(realized.start, 0);
    assert_eq!(realized.end, 0);
    assert!(realized.is_empty());
}

#[test]
fn test_full_list_visible() {
    let visible = 0..100;
    let item_count = 100;
    let prefetch = 5;

    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);

    // Both ends clamped
    assert_eq!(realized.start, 0);
    assert_eq!(realized.end, 100);
}

// =============================================================================
// Invariant: Range Well-Formedness
// =============================================================================

#[test]
fn test_range_always_well_formed() {
    // Test many combinations to ensure start <= end always
    for visible_start in 0..20 {
        for visible_len in 0..10 {
            for item_count in 0..30 {
                for prefetch in 0..10 {
                    let visible_end = (visible_start + visible_len).min(item_count);
                    let visible = visible_start.min(item_count)..visible_end;

                    let realized =
                        compute_realized_range_from_visible(&visible, item_count, prefetch);

                    assert!(
                        realized.start <= realized.end,
                        "Range must be well-formed: {:?} from visible={:?}, item_count={}, prefetch={}",
                        realized,
                        visible,
                        item_count,
                        prefetch
                    );
                    assert!(
                        realized.end <= item_count,
                        "End must not exceed item_count: {:?} > {}",
                        realized.end,
                        item_count
                    );
                }
            }
        }
    }
}
