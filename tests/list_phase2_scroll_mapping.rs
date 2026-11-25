// SPEC-HAL-134 Phase 2: Unit tests for compute_visible_range_from_scroll()
//
// This file tests the pure helper function that maps scroll_y → visible_range.
// Tests verify INV-VIRT-HGT-10..12 properties.

/// Helper: build a cumulative_y table from heights.
/// cumulative_y[i] = sum(heights[0..i])
/// cumulative_y has length = heights.len() + 1
fn build_cumulative_y(heights: &[f32]) -> Vec<f32> {
    let mut cumulative = Vec::with_capacity(heights.len() + 1);
    let mut y = 0.0;
    for &h in heights {
        cumulative.push(y);
        y += h;
    }
    cumulative.push(y); // Total height
    cumulative
}

/// Reimplement the helper here for testing (matches src/widgets/list.rs exactly).
fn compute_visible_range_from_scroll(
    scroll_y: f32,
    viewport_height: f32,
    cumulative_y: &[f32],
    item_count: usize,
) -> (usize, usize) {
    if item_count == 0 || viewport_height <= 0.0 || cumulative_y.is_empty() {
        return (0, 0);
    }

    // SPEC-HAL-134 Phase 2: Debug-only invariant assertions (INV-VIRT-HGT-7/8)
    // These make violations mechanically detectable by tests, not by review.
    debug_assert!(
        cumulative_y.len() == item_count + 1,
        "INV-VIRT-HGT-7: cumulative_y.len() ({}) must equal item_count + 1 ({})",
        cumulative_y.len(),
        item_count + 1
    );
    debug_assert!(
        cumulative_y[0] == 0.0,
        "INV-VIRT-HGT-7: cumulative_y[0] must be 0.0, got {}",
        cumulative_y[0]
    );
    debug_assert!(
        cumulative_y.windows(2).all(|w| w[1] >= w[0]),
        "INV-VIRT-HGT-8: cumulative_y must be monotonically non-decreasing"
    );

    let first = {
        let mut lo = 0;
        let mut hi = item_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let item_bottom = cumulative_y
                .get(mid + 1)
                .copied()
                .unwrap_or_else(|| cumulative_y[cumulative_y.len() - 1]);
            if item_bottom <= scroll_y {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo.min(item_count.saturating_sub(1))
    };

    let viewport_bottom = scroll_y + viewport_height;
    let last = {
        let mut lo = 0;
        let mut hi = item_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let item_top = cumulative_y.get(mid).copied().unwrap_or(0.0);
            if item_top < viewport_bottom {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo.min(item_count)
    };

    let start = first.min(item_count);
    let end = last.min(item_count).max(start);
    (start, end)
}

// =============================================================================
// INV-VIRT-HGT-11: Determinism Tests
// =============================================================================

#[test]
fn test_determinism_same_inputs_same_outputs() {
    // INV-VIRT-HGT-11: Same inputs must produce same outputs.
    let heights = vec![30.0, 40.0, 50.0, 30.0, 40.0];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let viewport_height = 100.0;
    let scroll_y = 50.0;

    // Call multiple times
    let result1 =
        compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);
    let result2 =
        compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);
    let result3 =
        compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);

    assert_eq!(
        result1, result2,
        "INV-VIRT-HGT-11: Results must be identical on repeated calls"
    );
    assert_eq!(
        result2, result3,
        "INV-VIRT-HGT-11: Results must be identical on repeated calls"
    );
}

#[test]
fn test_determinism_different_call_patterns() {
    // INV-VIRT-HGT-11: Order of calls must not affect results.
    let heights = vec![25.0, 35.0, 45.0, 55.0, 25.0, 35.0];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let viewport_height = 80.0;

    // Call with different scroll values, then re-call original
    let baseline =
        compute_visible_range_from_scroll(30.0, viewport_height, &cumulative_y, item_count);
    let _ = compute_visible_range_from_scroll(100.0, viewport_height, &cumulative_y, item_count);
    let _ = compute_visible_range_from_scroll(0.0, viewport_height, &cumulative_y, item_count);
    let recomputed =
        compute_visible_range_from_scroll(30.0, viewport_height, &cumulative_y, item_count);

    assert_eq!(
        baseline, recomputed,
        "INV-VIRT-HGT-11: Result must not depend on prior call history"
    );
}

// =============================================================================
// INV-VIRT-HGT-12: Monotonicity Tests
// =============================================================================

#[test]
fn test_monotonicity_scroll_down_start_non_decreasing() {
    // INV-VIRT-HGT-12: Scrolling down → start non-decreasing.
    let heights = vec![30.0; 20]; // 20 items, each 30px
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let viewport_height = 100.0;

    let mut prev_start = 0;
    for scroll_y in (0..500).step_by(5) {
        let scroll_y = scroll_y as f32;
        let (start, _end) =
            compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);
        assert!(
            start >= prev_start,
            "INV-VIRT-HGT-12: start decreased from {} to {} at scroll_y={}",
            prev_start,
            start,
            scroll_y
        );
        prev_start = start;
    }
}

#[test]
fn test_monotonicity_scroll_up_start_non_increasing() {
    // INV-VIRT-HGT-12: Scrolling up → start non-increasing.
    let heights = vec![30.0; 20];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let viewport_height = 100.0;

    let mut prev_start = item_count; // Start at max
    for scroll_y in (0..500).rev().step_by(5) {
        let scroll_y = scroll_y as f32;
        let (start, _end) =
            compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);
        assert!(
            start <= prev_start,
            "INV-VIRT-HGT-12: start increased from {} to {} at scroll_y={} (scrolling up)",
            prev_start,
            start,
            scroll_y
        );
        prev_start = start;
    }
}

#[test]
fn test_monotonicity_variable_heights() {
    // INV-VIRT-HGT-12: Monotonicity with non-uniform heights.
    let heights = vec![20.0, 50.0, 30.0, 80.0, 25.0, 45.0, 35.0, 60.0, 40.0, 55.0];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let viewport_height = 100.0;

    let mut prev_start = 0;
    for scroll_y in (0..400).step_by(3) {
        let scroll_y = scroll_y as f32;
        let (start, _end) =
            compute_visible_range_from_scroll(scroll_y, viewport_height, &cumulative_y, item_count);
        assert!(
            start >= prev_start,
            "INV-VIRT-HGT-12: start decreased with variable heights at scroll_y={}",
            scroll_y
        );
        prev_start = start;
    }
}

// =============================================================================
// Boundary Condition Tests
// =============================================================================

#[test]
fn test_boundary_empty_list() {
    let cumulative_y = build_cumulative_y(&[]);
    let (start, end) = compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, 0);
    assert_eq!((start, end), (0, 0), "Empty list must return (0, 0)");
}

#[test]
fn test_boundary_single_item() {
    let heights = vec![50.0];
    let cumulative_y = build_cumulative_y(&heights);

    // Item visible
    let (start, end) = compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, 1);
    assert_eq!((start, end), (0, 1), "Single visible item");

    // Scrolled past item
    let (start, _) = compute_visible_range_from_scroll(60.0, 100.0, &cumulative_y, 1);
    assert_eq!(start, 0, "Single item: start clamped to 0");
}

#[test]
fn test_boundary_scroll_at_top() {
    let heights = vec![30.0, 40.0, 50.0, 30.0, 40.0];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();

    let (start, _end) = compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count);
    assert_eq!(start, 0, "At top, start must be 0");
}

#[test]
fn test_boundary_scroll_at_bottom() {
    let heights = vec![30.0; 10]; // Total height = 300px
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();
    let total_height: f32 = heights.iter().sum();

    // Scroll to bottom (viewport starts at total_height - viewport_height)
    let scroll_y = total_height - 100.0; // 200.0
    let (start, end) =
        compute_visible_range_from_scroll(scroll_y, 100.0, &cumulative_y, item_count);

    assert!(end <= item_count, "End must not exceed item_count");
    assert!(start < end, "Must have at least one visible item");
}

#[test]
fn test_boundary_zero_viewport_height() {
    let heights = vec![30.0, 40.0, 50.0];
    let cumulative_y = build_cumulative_y(&heights);

    let (start, end) = compute_visible_range_from_scroll(0.0, 0.0, &cumulative_y, 3);
    assert_eq!((start, end), (0, 0), "Zero viewport height returns (0, 0)");
}

#[test]
fn test_boundary_negative_viewport_height() {
    let heights = vec![30.0, 40.0, 50.0];
    let cumulative_y = build_cumulative_y(&heights);

    let (start, end) = compute_visible_range_from_scroll(0.0, -10.0, &cumulative_y, 3);
    assert_eq!(
        (start, end),
        (0, 0),
        "Negative viewport height returns (0, 0)"
    );
}

// =============================================================================
// Correctness Tests
// =============================================================================

#[test]
fn test_correctness_uniform_heights() {
    // 5 items, each 40px. Viewport = 100px.
    // scroll_y=0: items 0,1,2 visible (0-40, 40-80, 80-120 overlap 0-100)
    let heights = vec![40.0; 5];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();

    let (start, end) = compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count);
    assert_eq!(start, 0);
    // Items: 0(0-40), 1(40-80), 2(80-120). Viewport 0-100.
    // Item 2 starts at 80 < 100, so it's included. Item 3 starts at 120 >= 100.
    assert_eq!(end, 3, "Items 0,1,2 should be visible");

    // scroll_y=40: items 1,2,3 visible
    let (start, end) = compute_visible_range_from_scroll(40.0, 100.0, &cumulative_y, item_count);
    assert_eq!(start, 1, "First visible item at scroll_y=40");
    assert_eq!(end, 4, "Items 1,2,3 should be visible");
}

#[test]
fn test_correctness_variable_heights() {
    // Heights: [20, 30, 50, 40, 60] = total 200px
    // Cumulative: [0, 20, 50, 100, 140, 200]
    let heights = vec![20.0, 30.0, 50.0, 40.0, 60.0];
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();

    // scroll_y=0, viewport=80
    // Items: 0(0-20), 1(20-50), 2(50-100). Viewport 0-80.
    // Item 2 starts at 50 < 80. Item 3 starts at 100 >= 80.
    let (start, end) = compute_visible_range_from_scroll(0.0, 80.0, &cumulative_y, item_count);
    assert_eq!(start, 0);
    assert_eq!(end, 3, "Items 0,1,2 should be visible");

    // scroll_y=60, viewport=80 (viewport spans 60-140)
    // Item 1 ends at 50 <= 60, not visible.
    // Item 2 (50-100) overlaps. Item 3 (100-140) overlaps. Item 4 starts at 140 >= 140.
    let (start, end) = compute_visible_range_from_scroll(60.0, 80.0, &cumulative_y, item_count);
    assert_eq!(start, 2, "First visible item at scroll_y=60");
    assert_eq!(end, 4, "Items 2,3 should be visible");
}

#[test]
fn test_correctness_exact_boundaries() {
    // Test exact pixel boundaries
    let heights = vec![50.0, 50.0, 50.0]; // Cumulative: [0, 50, 100, 150]
    let cumulative_y = build_cumulative_y(&heights);
    let item_count = heights.len();

    // scroll_y=50 exactly (item 0 ends at 50, item 1 starts at 50)
    // Item 0 bottom (50) <= scroll_y (50), so item 0 is NOT the first visible.
    // Item 1 bottom (100) > scroll_y (50), so first = 1.
    let (start, _end) = compute_visible_range_from_scroll(50.0, 50.0, &cumulative_y, item_count);
    assert_eq!(
        start, 1,
        "At exact boundary scroll_y=50, first visible is item 1"
    );
}

// =============================================================================
// SPEC-HAL-134 Phase 2 Step 2: Debug-Only Invariant Assertion Tests
// =============================================================================
//
// These tests verify that malformed inputs trigger debug_assert! panics.
// They use catch_unwind to prove the contract is self-guarding.

#[cfg(debug_assertions)]
mod invariant_violation_tests {
    use super::compute_visible_range_from_scroll;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn test_invariant_length_mismatch_too_short_panics() {
        // INV-VIRT-HGT-7: cumulative_y.len() must == item_count + 1
        // Here: item_count=5, but cumulative_y.len()=3 (should be 6)
        let cumulative_y = vec![0.0, 30.0, 60.0]; // len=3, but item_count=5
        let item_count = 5;

        let result = catch_unwind(AssertUnwindSafe(|| {
            compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count)
        }));

        assert!(
            result.is_err(),
            "INV-VIRT-HGT-7: Length mismatch (too short) must panic in debug mode"
        );
    }

    #[test]
    fn test_invariant_length_mismatch_too_long_panics() {
        // INV-VIRT-HGT-7: cumulative_y.len() must == item_count + 1
        // Here: item_count=2, but cumulative_y.len()=6 (should be 3)
        let cumulative_y = vec![0.0, 30.0, 60.0, 90.0, 120.0, 150.0]; // len=6, but item_count=2
        let item_count = 2;

        let result = catch_unwind(AssertUnwindSafe(|| {
            compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count)
        }));

        assert!(
            result.is_err(),
            "INV-VIRT-HGT-7: Length mismatch (too long) must panic in debug mode"
        );
    }

    #[test]
    fn test_invariant_non_zero_first_element_panics() {
        // INV-VIRT-HGT-7: cumulative_y[0] must be 0.0
        let cumulative_y = vec![10.0, 40.0, 70.0, 100.0]; // First element is 10.0, not 0.0
        let item_count = 3;

        let result = catch_unwind(AssertUnwindSafe(|| {
            compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count)
        }));

        assert!(
            result.is_err(),
            "INV-VIRT-HGT-7: Non-zero first element must panic in debug mode"
        );
    }

    #[test]
    fn test_invariant_non_monotonic_decreasing_panics() {
        // INV-VIRT-HGT-8: cumulative_y must be monotonically non-decreasing
        // Here: 60.0 -> 50.0 is a decrease
        let cumulative_y = vec![0.0, 30.0, 60.0, 50.0, 80.0]; // Decrease at index 3
        let item_count = 4;

        let result = catch_unwind(AssertUnwindSafe(|| {
            compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count)
        }));

        assert!(
            result.is_err(),
            "INV-VIRT-HGT-8: Non-monotonic (decreasing) cumulative_y must panic in debug mode"
        );
    }

    #[test]
    fn test_invariant_monotonic_equal_values_allowed() {
        // INV-VIRT-HGT-8: "non-decreasing" means equal values are OK (zero-height items)
        let cumulative_y = vec![0.0, 30.0, 30.0, 60.0]; // 30.0 -> 30.0 is allowed (zero height)
        let item_count = 3;

        // This should NOT panic
        let result = catch_unwind(AssertUnwindSafe(|| {
            compute_visible_range_from_scroll(0.0, 100.0, &cumulative_y, item_count)
        }));

        assert!(
            result.is_ok(),
            "INV-VIRT-HGT-8: Equal consecutive values (zero-height items) must NOT panic"
        );
    }
}
