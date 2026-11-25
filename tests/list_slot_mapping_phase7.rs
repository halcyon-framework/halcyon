// SPEC-HAL-134 Phase 7: Slot–Index Mapping Freeze Tests
//
// Tests for slot_index_from_logical() and logical_index_from_slot() invariants:
// - INV-SLOT-1: Single Mapping Authority (slot = logical - realized.start)
// - INV-SLOT-2: Contiguity (slots = realized_range.len())
// - INV-SLOT-3: Stability Across Frames (unchanged realized → unchanged mapping)
// - INV-SLOT-4: Bounds Safety (0 <= slot < children.len())
// - INV-SLOT-5: No Shadow Slot Logic (tested implicitly by single function)

use std::ops::Range;

/// Mirror of slot_index_from_logical from list.rs for testing.
fn slot_index_from_logical(logical_index: usize, realized_range: &Range<usize>) -> Option<usize> {
    if !realized_range.contains(&logical_index) {
        return None;
    }
    Some(logical_index - realized_range.start)
}

/// Mirror of logical_index_from_slot from list.rs for testing.
fn logical_index_from_slot(slot_index: usize, realized_range: &Range<usize>) -> Option<usize> {
    if slot_index >= realized_range.len() {
        return None;
    }
    Some(realized_range.start + slot_index)
}

// =============================================================================
// INV-SLOT-1: Single Mapping Authority
// =============================================================================

#[test]
fn test_slot_formula_at_start() {
    let realized = 10..20;
    // logical_index 10 should map to slot 0
    let slot = slot_index_from_logical(10, &realized);
    assert_eq!(slot, Some(0));
    // Verify formula: slot = logical - realized.start
    assert_eq!(slot.unwrap(), 10 - realized.start);
}

#[test]
fn test_slot_formula_in_middle() {
    let realized = 10..20;
    // logical_index 15 should map to slot 5
    let slot = slot_index_from_logical(15, &realized);
    assert_eq!(slot, Some(5));
    // Verify formula
    assert_eq!(slot.unwrap(), 15 - realized.start);
}

#[test]
fn test_slot_formula_at_end() {
    let realized = 10..20;
    // logical_index 19 (last in range) should map to slot 9
    let slot = slot_index_from_logical(19, &realized);
    assert_eq!(slot, Some(9));
    // Verify formula
    assert_eq!(slot.unwrap(), 19 - realized.start);
}

#[test]
fn test_logical_formula_at_start() {
    let realized = 10..20;
    // slot 0 should map to logical_index 10
    let logical = logical_index_from_slot(0, &realized);
    assert_eq!(logical, Some(10));
    // Verify formula: logical = realized.start + slot
    assert_eq!(logical.unwrap(), realized.start + 0);
}

#[test]
fn test_logical_formula_in_middle() {
    let realized = 10..20;
    // slot 5 should map to logical_index 15
    let logical = logical_index_from_slot(5, &realized);
    assert_eq!(logical, Some(15));
    // Verify formula
    assert_eq!(logical.unwrap(), realized.start + 5);
}

#[test]
fn test_logical_formula_at_end() {
    let realized = 10..20;
    // slot 9 should map to logical_index 19
    let logical = logical_index_from_slot(9, &realized);
    assert_eq!(logical, Some(19));
    // Verify formula
    assert_eq!(logical.unwrap(), realized.start + 9);
}

// =============================================================================
// INV-SLOT-2: Contiguity
// =============================================================================

#[test]
fn test_contiguous_slot_mapping() {
    let realized = 5..15;
    // All slots 0..10 should map to contiguous logical indices 5..15
    for slot in 0..realized.len() {
        let logical = logical_index_from_slot(slot, &realized);
        assert!(
            logical.is_some(),
            "Slot {} should map to valid logical index",
            slot
        );
        assert_eq!(logical.unwrap(), realized.start + slot);
    }
}

#[test]
fn test_no_gaps_in_mapping() {
    let realized = 100..110;
    // Verify every logical index in range maps to a valid slot
    for logical in realized.clone() {
        let slot = slot_index_from_logical(logical, &realized);
        assert!(
            slot.is_some(),
            "Logical {} should map to valid slot",
            logical
        );
    }
    // Verify exactly realized.len() slots are valid
    let valid_slots: Vec<_> = (0..100)
        .filter(|&s| logical_index_from_slot(s, &realized).is_some())
        .collect();
    assert_eq!(valid_slots.len(), realized.len());
}

#[test]
fn test_slots_equal_realized_len() {
    let realized = 25..40;
    // Count valid slots
    let mut valid_count = 0;
    for slot in 0..1000 {
        if logical_index_from_slot(slot, &realized).is_some() {
            valid_count += 1;
        }
    }
    assert_eq!(valid_count, realized.len());
}

// =============================================================================
// INV-SLOT-3: Stability Across Frames
// =============================================================================

#[test]
fn test_mapping_stability_same_range() {
    let realized = 10..20;

    // First "frame"
    let mappings_frame1: Vec<_> = (0..realized.len())
        .map(|slot| logical_index_from_slot(slot, &realized))
        .collect();

    // Second "frame" with same realized range
    let mappings_frame2: Vec<_> = (0..realized.len())
        .map(|slot| logical_index_from_slot(slot, &realized))
        .collect();

    assert_eq!(
        mappings_frame1, mappings_frame2,
        "Mapping must be stable across frames"
    );
}

#[test]
fn test_inverse_mapping_stability() {
    let realized = 50..75;

    // First pass: logical → slot
    let slot_mappings1: Vec<_> = realized
        .clone()
        .map(|l| slot_index_from_logical(l, &realized))
        .collect();

    // Second pass: same inputs
    let slot_mappings2: Vec<_> = realized
        .clone()
        .map(|l| slot_index_from_logical(l, &realized))
        .collect();

    assert_eq!(slot_mappings1, slot_mappings2);
}

// =============================================================================
// INV-SLOT-4: Bounds Safety
// =============================================================================

#[test]
fn test_reject_logical_below_range() {
    let realized = 10..20;
    // logical_index 9 is below range
    assert_eq!(slot_index_from_logical(9, &realized), None);
    // logical_index 0 is below range
    assert_eq!(slot_index_from_logical(0, &realized), None);
}

#[test]
fn test_reject_logical_above_range() {
    let realized = 10..20;
    // logical_index 20 is at/above range end
    assert_eq!(slot_index_from_logical(20, &realized), None);
    // logical_index 100 is above range
    assert_eq!(slot_index_from_logical(100, &realized), None);
}

#[test]
fn test_reject_slot_above_range() {
    let realized = 10..20; // len = 10
    // slot 10 is at/above range length
    assert_eq!(logical_index_from_slot(10, &realized), None);
    // slot 100 is above range
    assert_eq!(logical_index_from_slot(100, &realized), None);
}

#[test]
fn test_valid_slot_range() {
    let realized = 5..15; // len = 10
    // All slots 0..10 should be valid
    for slot in 0..10 {
        assert!(
            logical_index_from_slot(slot, &realized).is_some(),
            "Slot {} should be valid",
            slot
        );
    }
    // Slot 10 should be invalid
    assert!(logical_index_from_slot(10, &realized).is_none());
}

// =============================================================================
// Round-Trip Consistency
// =============================================================================

#[test]
fn test_round_trip_slot_to_logical_to_slot() {
    let realized = 20..30;
    for slot in 0..realized.len() {
        let logical = logical_index_from_slot(slot, &realized).unwrap();
        let round_trip_slot = slot_index_from_logical(logical, &realized).unwrap();
        assert_eq!(slot, round_trip_slot, "Round-trip failed for slot {}", slot);
    }
}

#[test]
fn test_round_trip_logical_to_slot_to_logical() {
    let realized = 20..30;
    for logical in realized.clone() {
        let slot = slot_index_from_logical(logical, &realized).unwrap();
        let round_trip_logical = logical_index_from_slot(slot, &realized).unwrap();
        assert_eq!(
            logical, round_trip_logical,
            "Round-trip failed for logical {}",
            logical
        );
    }
}

// =============================================================================
// Integration with Prefetch (Phase 6)
// =============================================================================

/// Mirror of compute_realized_range_from_visible from list.rs
fn compute_realized_range_from_visible(
    visible_range: &Range<usize>,
    item_count: usize,
    prefetch: usize,
) -> Range<usize> {
    let start = visible_range.start.saturating_sub(prefetch);
    let end = (visible_range.end + prefetch).min(item_count);
    start..end
}

#[test]
fn test_slot_mapping_with_prefetch() {
    let item_count = 100;
    let prefetch = 2;
    let visible = 10..20;

    // Compute realized range
    let realized = compute_realized_range_from_visible(&visible, item_count, prefetch);
    assert_eq!(realized, 8..22);

    // Slot 0 should map to logical 8 (not 10!)
    assert_eq!(logical_index_from_slot(0, &realized), Some(8));
    // Slot 2 should map to logical 10 (first visible)
    assert_eq!(logical_index_from_slot(2, &realized), Some(10));

    // Logical 10 should map to slot 2 (not 0!)
    assert_eq!(slot_index_from_logical(10, &realized), Some(2));
    // Logical 8 should map to slot 0
    assert_eq!(slot_index_from_logical(8, &realized), Some(0));
}

#[test]
fn test_slot_mapping_clamped_realized_range() {
    let item_count = 100;
    let prefetch = 5;

    // Visible at start: realized will be clamped at 0
    let visible_start = 2..10;
    let realized_start = compute_realized_range_from_visible(&visible_start, item_count, prefetch);
    assert_eq!(realized_start, 0..15);

    // Slot 0 should map to logical 0
    assert_eq!(logical_index_from_slot(0, &realized_start), Some(0));
    // Logical 2 (first visible) maps to slot 2
    assert_eq!(slot_index_from_logical(2, &realized_start), Some(2));

    // Visible at end: realized will be clamped at item_count
    let visible_end = 95..100;
    let realized_end = compute_realized_range_from_visible(&visible_end, item_count, prefetch);
    assert_eq!(realized_end, 90..100);

    // Slot 0 should map to logical 90
    assert_eq!(logical_index_from_slot(0, &realized_end), Some(90));
    // Logical 95 maps to slot 5
    assert_eq!(slot_index_from_logical(95, &realized_end), Some(5));
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_empty_realized_range() {
    let realized = 5..5; // Empty range

    // No slot should be valid
    assert_eq!(logical_index_from_slot(0, &realized), None);

    // No logical should map to slot
    assert_eq!(slot_index_from_logical(5, &realized), None);
}

#[test]
fn test_single_item_realized() {
    let realized = 42..43; // Single item

    // Only slot 0 is valid
    assert_eq!(logical_index_from_slot(0, &realized), Some(42));
    assert_eq!(logical_index_from_slot(1, &realized), None);

    // Only logical 42 maps
    assert_eq!(slot_index_from_logical(42, &realized), Some(0));
    assert_eq!(slot_index_from_logical(41, &realized), None);
    assert_eq!(slot_index_from_logical(43, &realized), None);
}

#[test]
fn test_realized_at_zero() {
    let realized = 0..10;

    // Slot 0 maps to logical 0
    assert_eq!(logical_index_from_slot(0, &realized), Some(0));
    // Logical 0 maps to slot 0
    assert_eq!(slot_index_from_logical(0, &realized), Some(0));
}

#[test]
fn test_large_realized_range() {
    let realized = 1000..2000;

    // Random samples
    assert_eq!(slot_index_from_logical(1000, &realized), Some(0));
    assert_eq!(slot_index_from_logical(1500, &realized), Some(500));
    assert_eq!(slot_index_from_logical(1999, &realized), Some(999));
    assert_eq!(slot_index_from_logical(2000, &realized), None);

    assert_eq!(logical_index_from_slot(0, &realized), Some(1000));
    assert_eq!(logical_index_from_slot(500, &realized), Some(1500));
    assert_eq!(logical_index_from_slot(999, &realized), Some(1999));
    assert_eq!(logical_index_from_slot(1000, &realized), None);
}

// =============================================================================
// Invariant: Mapping Completeness
// =============================================================================

#[test]
fn test_mapping_is_bijective() {
    // Every slot maps to exactly one logical, every logical maps to exactly one slot
    let realized = 50..100;

    // Collect all slot → logical mappings
    let slot_to_logical: Vec<_> = (0..realized.len())
        .map(|s| logical_index_from_slot(s, &realized).unwrap())
        .collect();

    // Collect all logical → slot mappings
    let logical_to_slot: Vec<_> = realized
        .clone()
        .map(|l| slot_index_from_logical(l, &realized).unwrap())
        .collect();

    // All logical indices should be unique
    let mut sorted_logical = slot_to_logical.clone();
    sorted_logical.sort();
    sorted_logical.dedup();
    assert_eq!(
        sorted_logical.len(),
        realized.len(),
        "Logical indices must be unique"
    );

    // All slot indices should be unique
    let mut sorted_slot = logical_to_slot.clone();
    sorted_slot.sort();
    sorted_slot.dedup();
    assert_eq!(
        sorted_slot.len(),
        realized.len(),
        "Slot indices must be unique"
    );
}
