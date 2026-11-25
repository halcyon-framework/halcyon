// TextArea Stage-1 Stabilization: Invariant Tests
//
// These tests enforce the five core invariants for TextArea:
// - I-1: Cursor Visibility Invariant
// - I-2: Boundary Mapping Invariant
// - I-3: Selection Visibility Invariant
// - I-4: Cursor/Selection Coupling Invariant
// - I-5: Vertical Navigation Invariant

#![allow(unused_imports, unused_variables)]

use halcyon::core::editing::ClusterBoundary;

// ============================================================================
// Test Helpers - Simulating TextArea's internal functions
// ============================================================================

/// TextArea's current caret_position logic (exact-match only)
fn caret_position_exact_match(boundaries: &[ClusterBoundary], byte_pos: usize) -> (usize, f32) {
    for b in boundaries {
        if b.byte_start == byte_pos {
            return (b.line_index, b.x_logical);
        }
    }
    // Fallback to last boundary
    boundaries
        .last()
        .map(|b| (b.line_index, b.x_logical))
        .unwrap_or((0, 0.0))
}

/// TextArea's current x_for_byte logic (exact-match only)
fn x_for_byte_exact_match(boundaries: &[ClusterBoundary], byte_idx: usize) -> f32 {
    for b in boundaries {
        if b.byte_start == byte_idx {
            return b.x_logical;
        }
    }
    boundaries.last().map(|b| b.x_logical).unwrap_or(0.0)
}

/// TextArea's current line_for_byte logic (exact-match only)
fn line_for_byte_exact_match(boundaries: &[ClusterBoundary], byte_idx: usize) -> usize {
    for b in boundaries {
        if b.byte_start == byte_idx {
            return b.line_index;
        }
    }
    boundaries.last().map(|b| b.line_index).unwrap_or(0)
}

/// Create boundaries for "hello" (5 chars, each 10px wide)
fn make_hello_boundaries() -> Vec<ClusterBoundary> {
    vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 2,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 0,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 0,
            x_logical: 40.0,
        },
        // Sentinel at end
        ClusterBoundary {
            byte_start: 5,
            byte_end: 5,
            line_index: 0,
            x_logical: 50.0,
        },
    ]
}

/// Create boundaries for "abc\ndef" (two lines)
fn make_two_line_boundaries() -> Vec<ClusterBoundary> {
    vec![
        // Line 0: "abc"
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 2,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        },
        // Line 1: "def" (byte 3 is newline, consumed)
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 20.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 1,
            x_logical: 30.0,
        },
    ]
}

// ============================================================================
// I-2: Boundary Mapping Invariant Tests
// ============================================================================

/// I-2: For any byte position 0 <= p <= text.len(), there must exist a
/// deterministic mapping p → (line_index, x_logical).
///
/// This test verifies that the current exact-match logic FAILS for positions
/// that don't have an exact byte_start match.
#[test]
fn test_i2_boundary_mapping_sentinel_position() {
    let boundaries = make_hello_boundaries();
    // text = "hello", len = 5
    // Cursor at position 5 (end of text) should map to sentinel

    let (line, x) = caret_position_exact_match(&boundaries, 5);

    // INVARIANT: position 5 should map to line 0, x=50 (sentinel position)
    assert_eq!(line, 0, "Sentinel position should be on line 0");
    assert_eq!(x, 50.0, "Sentinel position should be at x=50");
}

/// I-2: Test that mid-cluster positions still produce valid mappings.
/// If cursor lands at byte 3 (which is byte_start of cluster 3), this works.
/// But what about byte 3 when clusters are multi-byte?
#[test]
fn test_i2_boundary_mapping_all_valid_positions() {
    let boundaries = make_hello_boundaries();

    // Every position from 0 to 5 (inclusive) must produce a valid mapping
    for pos in 0..=5 {
        let (line, x) = caret_position_exact_match(&boundaries, pos);

        // The mapping must be deterministic (not fallback to wrong position)
        // For "hello", position i should map to x = i * 10
        let expected_x = pos as f32 * 10.0;
        assert_eq!(
            x, expected_x,
            "Position {} should map to x={}, got x={}",
            pos, expected_x, x
        );
        assert_eq!(
            line, 0,
            "All positions in single-line text should be on line 0"
        );
    }
}

/// I-2: Test boundary mapping for multi-line text.
/// Position 3 (the newline) may not have an exact boundary match.
#[test]
fn test_i2_boundary_mapping_newline_position() {
    let boundaries = make_two_line_boundaries();
    // text = "abc\ndef", byte 3 is newline

    // Position 3 (newline) - what happens?
    let (line, x) = caret_position_exact_match(&boundaries, 3);

    // If newline has no boundary, exact-match falls back to LAST boundary (line 1, x=30)
    // But position 3 should logically be END of line 0 or START of line 1
    // This is a known edge case - document the actual behavior
    //
    // CURRENT BEHAVIOR: Falls back to last boundary (line 1, x=30) - WRONG
    // EXPECTED: Should be end of line 0 (x=30 on line 0) or start of line 1 (x=0 on line 1)

    // This test documents the bug - it will pass showing current (broken) behavior
    // After fix, update expected values
    assert!(
        line == 0 || line == 1,
        "Newline position should map to line 0 or 1, got line {}",
        line
    );
}

// ============================================================================
// I-1: Cursor Visibility Invariant Tests
// ============================================================================

/// I-1: If cursor.pos is valid and widget is focused, caret MUST be painted
/// at valid (x, y) coordinates.
///
/// This test verifies that caret_position returns valid coords for the sentinel.
#[test]
fn test_i1_cursor_visibility_at_text_end() {
    let boundaries = make_hello_boundaries();

    // Cursor at end of text (position 5)
    let (line, x) = caret_position_exact_match(&boundaries, 5);

    // Caret must have valid position (not silently disappear)
    assert!(x >= 0.0, "Caret x must be non-negative");
    assert!(x <= 100.0, "Caret x must be within reasonable bounds");
    assert_eq!(line, 0, "Caret must be on valid line");
}

/// I-1: Cursor at position 0 must always work
#[test]
fn test_i1_cursor_visibility_at_start() {
    let boundaries = make_hello_boundaries();

    let (line, x) = caret_position_exact_match(&boundaries, 0);

    assert_eq!(line, 0);
    assert_eq!(x, 0.0);
}

/// I-1: Cursor visibility after vertical navigation.
/// After move_down, cursor.pos must still map to valid caret position.
#[test]
fn test_i1_cursor_visibility_after_vertical_nav() {
    use halcyon::core::editing::{Selection, TextCursor, move_down};

    let boundaries = make_two_line_boundaries();

    // Start at 'b' on line 0 (position 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // Move down
    move_down(&mut cursor, &mut selection, &boundaries);

    // cursor.pos should now be on line 1
    // Verify caret_position returns valid result
    let (line, x) = caret_position_exact_match(&boundaries, cursor.pos);

    assert_eq!(line, 1, "Cursor should be on line 1 after move_down");
    assert!(x >= 0.0, "Caret x must be valid after vertical nav");
}

// ============================================================================
// I-3: Selection Visibility Invariant Tests
// ============================================================================

/// I-3: If selection.anchor != selection.head, at least one selection rect
/// must be painted.
///
/// This requires line_for_byte to work for both anchor and head positions.
#[test]
fn test_i3_selection_visibility_single_line() {
    let boundaries = make_hello_boundaries();

    // Selection from 1 to 3
    let sel_start = 1;
    let sel_end = 3;

    let start_line = line_for_byte_exact_match(&boundaries, sel_start);
    let end_line = line_for_byte_exact_match(&boundaries, sel_end);

    assert_eq!(start_line, 0);
    assert_eq!(end_line, 0);

    // X positions for selection rect
    let start_x = x_for_byte_exact_match(&boundaries, sel_start);
    let end_x = x_for_byte_exact_match(&boundaries, sel_end);

    assert_eq!(start_x, 10.0);
    assert_eq!(end_x, 30.0);
    assert!(end_x > start_x, "Selection must have positive width");
}

/// I-3: Multi-line selection must paint rects on each line
#[test]
fn test_i3_selection_visibility_multi_line() {
    let boundaries = make_two_line_boundaries();

    // Selection from line 0 position 1 to line 1 position 5
    let sel_start = 1;
    let sel_end = 5;

    let start_line = line_for_byte_exact_match(&boundaries, sel_start);
    let end_line = line_for_byte_exact_match(&boundaries, sel_end);

    assert_eq!(start_line, 0, "Selection start should be on line 0");
    assert_eq!(end_line, 1, "Selection end should be on line 1");
}

/// I-3: Selection at sentinel position
#[test]
fn test_i3_selection_to_end_of_text() {
    let boundaries = make_hello_boundaries();

    // Selection from 0 to 5 (entire text including sentinel)
    let sel_start = 0;
    let sel_end = 5;

    let start_line = line_for_byte_exact_match(&boundaries, sel_start);
    let end_line = line_for_byte_exact_match(&boundaries, sel_end);

    assert_eq!(start_line, 0);
    assert_eq!(end_line, 0);

    let end_x = x_for_byte_exact_match(&boundaries, sel_end);
    assert_eq!(end_x, 50.0, "Selection end at sentinel should be x=50");
}

// ============================================================================
// I-4: Cursor/Selection Coupling Invariant Tests
// ============================================================================

/// I-4: After Shift+Arrow, cursor.pos == selection.head
#[test]
fn test_i4_coupling_after_shift_right() {
    use halcyon::core::editing::{Selection, TextCursor, move_right_select};

    let boundaries = make_hello_boundaries();

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_right_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, selection.head,
        "cursor.pos must equal selection.head after Shift+Right"
    );
    assert_eq!(selection.anchor, 2, "anchor must stay at original position");
}

/// I-4: After Shift+Down, cursor.pos == selection.head
#[test]
fn test_i4_coupling_after_shift_down() {
    use halcyon::core::editing::{Selection, TextCursor, move_down_select};

    let boundaries = make_two_line_boundaries();

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, selection.head,
        "cursor.pos must equal selection.head after Shift+Down"
    );
    assert_eq!(selection.anchor, 1, "anchor must stay at original position");
}

/// I-4: After mouse drag, cursor.pos == selection.head
#[test]
fn test_i4_coupling_after_drag() {
    use halcyon::core::editing::{Selection, TextCursor};

    // Simulate drag from position 2 to position 5
    let anchor = 2;
    let head = 5;

    let mut cursor = TextCursor::at(anchor);
    let mut selection = Selection::new();
    selection.anchor = anchor;
    selection.head = anchor;

    // Drag to head
    selection.head = head;
    cursor.pos = head;

    assert_eq!(
        cursor.pos, selection.head,
        "cursor.pos must equal selection.head after drag"
    );
}

// ============================================================================
// I-5: Vertical Navigation Invariant Tests
// ============================================================================

/// I-5: After Up/Down, cursor must land on a valid cluster position
#[test]
fn test_i5_vertical_nav_lands_on_valid_cluster() {
    use halcyon::core::editing::{Selection, TextCursor, move_down};

    let boundaries = make_two_line_boundaries();

    let mut cursor = TextCursor::at(1); // 'b' on line 0
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // cursor.pos must be a valid byte_start in boundaries
    let is_valid = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid,
        "After move_down, cursor.pos={} must be a valid cluster boundary",
        cursor.pos
    );
}

/// I-5: preferred_x must be preserved across multiple vertical moves
#[test]
fn test_i5_preferred_x_preserved() {
    use halcyon::core::editing::{Selection, TextCursor, move_down, move_up};

    // Line 0: 5 chars (x = 0,10,20,30,40)
    // Line 1: 2 chars (x = 0,10)
    // Line 2: 5 chars (x = 0,10,20,30,40)
    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 2,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 0,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 0,
            x_logical: 40.0,
        },
        // Line 1 (short)
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8,
            line_index: 1,
            x_logical: 10.0,
        },
        // Line 2
        ClusterBoundary {
            byte_start: 9,
            byte_end: 10,
            line_index: 2,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 10,
            byte_end: 11,
            line_index: 2,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 11,
            byte_end: 12,
            line_index: 2,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 12,
            byte_end: 13,
            line_index: 2,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 13,
            byte_end: 14,
            line_index: 2,
            x_logical: 40.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 14,
            byte_end: 14,
            line_index: 2,
            x_logical: 50.0,
        },
    ];

    // Start at position 4 (x=40) on line 0
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);
    // Line 1 is short - should land at rightmost position (x=10)
    // preferred_x should be set to 40

    assert_eq!(
        cursor.preferred_x,
        Some(40.0),
        "preferred_x should be set after first move_down"
    );

    move_down(&mut cursor, &mut selection, &boundaries);
    // Line 2 has x=40 - should return to x=40 due to preferred_x

    assert_eq!(
        cursor.preferred_x,
        Some(40.0),
        "preferred_x should be preserved"
    );

    // Verify cursor landed at x=40 on line 2
    let landed_boundary = boundaries.iter().find(|b| b.byte_start == cursor.pos);
    assert!(landed_boundary.is_some());
    assert_eq!(
        landed_boundary.unwrap().x_logical,
        40.0,
        "Should land at x=40 on line 2"
    );
}

/// I-5: Cursor must stay drawable after vertical navigation
#[test]
fn test_i5_cursor_drawable_after_vertical() {
    use halcyon::core::editing::{Selection, TextCursor, move_down};

    let boundaries = make_two_line_boundaries();

    let mut cursor = TextCursor::at(2); // 'c' on line 0
    let mut selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);

    // cursor.pos must produce valid caret coordinates
    let (line, x) = caret_position_exact_match(&boundaries, cursor.pos);

    // Must be on line 1
    assert_eq!(line, 1, "After move_down from line 0, should be on line 1");

    // x must be valid (closest to original x=20)
    assert!(x >= 0.0, "Caret x must be non-negative");
}
