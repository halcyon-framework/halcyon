// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.
//
// NOTE:
// These tests use ASCII-only boundary generation for simplicity.
// UTF-8 multi-byte tests construct explicit boundaries.

#![allow(non_snake_case)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, move_left_collapse, move_left_select,
    move_right_collapse, move_right_select,
};

// ============================================================================
// Helper: Create minimal boundaries for ASCII text (one boundary per byte)
// ============================================================================

fn make_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    for (i, _) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + 1,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * 10.0,
    });
    boundaries
}

/// Create boundaries for text with multi-byte UTF-8 characters.
fn make_utf8_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index: 0,
            x_logical: boundaries.len() as f32 * 10.0,
        });
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: boundaries.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// SPEC-HAL-107 §3.1 — Single-Step Movement
// ============================================================================

#[test]
fn move_left_goes_to_previous_boundary_SPEC_107_3_1() {
    // SPEC-HAL-107 §3.1: Left arrow moves to previous boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3); // After "Hel"
    let mut selection = Selection::at(3);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor moved to position 2
    assert_eq!(
        cursor.pos, 2,
        "move_left must move to previous boundary (2), got {}",
        cursor.pos
    );
}

#[test]
fn move_right_goes_to_next_boundary_SPEC_107_3_1() {
    // SPEC-HAL-107 §3.1: Right arrow moves to next boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(2); // After "He"
    let mut selection = Selection::at(2);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor moved to position 3
    assert_eq!(
        cursor.pos, 3,
        "move_right must move to next boundary (3), got {}",
        cursor.pos
    );
}

#[test]
fn move_left_never_skips_boundaries_SPEC_107_3_1() {
    // SPEC-HAL-107 §3.1: Left movement never skips boundaries.

    let text = "ABCDE";
    let boundaries = make_ascii_boundaries(text);

    // Start at end, walk left through all boundaries
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    let expected_positions = [4, 3, 2, 1, 0];

    for expected in expected_positions {
        move_left_collapse(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            cursor.pos, expected,
            "Sequential left movement must hit every boundary. Expected {}, got {}",
            expected, cursor.pos
        );
    }
}

#[test]
fn move_right_never_skips_boundaries_SPEC_107_3_1() {
    // SPEC-HAL-107 §3.1: Right movement never skips boundaries.

    let text = "ABCDE";
    let boundaries = make_ascii_boundaries(text);

    // Start at beginning, walk right through all boundaries
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    let expected_positions = [1, 2, 3, 4, 5];

    for expected in expected_positions {
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            cursor.pos, expected,
            "Sequential right movement must hit every boundary. Expected {}, got {}",
            expected, cursor.pos
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §3.2 — Edge No-Ops
// ============================================================================

#[test]
fn move_left_at_start_is_noop_SPEC_107_3_2() {
    // SPEC-HAL-107 §3.2: Left at position 0 does nothing.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor unchanged
    assert_eq!(cursor.pos, 0, "move_left at start must not change cursor");
}

#[test]
fn move_right_at_end_is_noop_SPEC_107_3_2() {
    // SPEC-HAL-107 §3.2: Right at text.len() does nothing.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);
    let end_pos = text.len();

    let mut cursor = TextCursor::at(end_pos);
    let mut selection = Selection::at(end_pos);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor unchanged
    assert_eq!(
        cursor.pos, end_pos,
        "move_right at end must not change cursor"
    );
}

#[test]
fn move_left_select_at_start_is_noop_SPEC_107_3_2() {
    // SPEC-HAL-107 §3.2: Left select at position 0 does nothing to cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_left_select(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor unchanged
    assert_eq!(
        cursor.pos, 0,
        "move_left_select at start must not move cursor"
    );
}

#[test]
fn move_right_select_at_end_is_noop_SPEC_107_3_2() {
    // SPEC-HAL-107 §3.2: Right select at text.len() does nothing to cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);
    let end_pos = text.len();

    let mut cursor = TextCursor::at(end_pos);
    let mut selection = Selection::at(end_pos);

    move_right_select(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor unchanged
    assert_eq!(
        cursor.pos, end_pos,
        "move_right_select at end must not move cursor"
    );
}

// ============================================================================
// SPEC-HAL-107 §3.3 — Collapse vs Select
// ============================================================================

#[test]
fn move_left_collapse_collapses_selection_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Collapse mode - selection collapses to cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: selection collapsed to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "Collapse mode: anchor must equal cursor"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "Collapse mode: head must equal cursor"
    );
    assert!(selection.is_empty(), "Selection must be collapsed (empty)");
}

#[test]
fn move_right_collapse_collapses_selection_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Collapse mode - selection collapses to cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: selection collapsed to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "Collapse mode: anchor must equal cursor"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "Collapse mode: head must equal cursor"
    );
    assert!(selection.is_empty(), "Selection must be collapsed (empty)");
}

#[test]
fn move_left_select_extends_selection_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Select mode - anchor fixed, head follows cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);
    let original_anchor = selection.anchor;

    move_left_select(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor moved
    assert_eq!(cursor.pos, 2, "Cursor must move left");

    // Assert: anchor unchanged
    assert_eq!(
        selection.anchor, original_anchor,
        "Select mode: anchor must remain fixed"
    );

    // Assert: head follows cursor
    assert_eq!(
        selection.head, cursor.pos,
        "Select mode: head must follow cursor"
    );

    // Assert: selection is non-empty
    assert!(
        !selection.is_empty(),
        "Selection must be non-empty after select"
    );
}

#[test]
fn move_right_select_extends_selection_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Select mode - anchor fixed, head follows cursor.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);
    let original_anchor = selection.anchor;

    move_right_select(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor moved
    assert_eq!(cursor.pos, 3, "Cursor must move right");

    // Assert: anchor unchanged
    assert_eq!(
        selection.anchor, original_anchor,
        "Select mode: anchor must remain fixed"
    );

    // Assert: head follows cursor
    assert_eq!(
        selection.head, cursor.pos,
        "Select mode: head must follow cursor"
    );

    // Assert: selection is non-empty
    assert!(
        !selection.is_empty(),
        "Selection must be non-empty after select"
    );
}

#[test]
fn move_left_collapse_with_selection_collapses_to_start_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Collapse with existing selection collapses to selection start.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Selecting "ell"

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor collapses to selection start
    assert_eq!(
        cursor.pos, 1,
        "Collapse with selection must move cursor to selection start"
    );
    assert!(selection.is_empty(), "Selection must be collapsed");
}

#[test]
fn move_right_collapse_with_selection_collapses_to_end_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Collapse with existing selection collapses to selection end.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Selecting "ell"

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor collapses to selection end
    assert_eq!(
        cursor.pos, 4,
        "Collapse with selection must move cursor to selection end"
    );
    assert!(selection.is_empty(), "Selection must be collapsed");
}

#[test]
fn move_left_select_preserves_anchor_through_multiple_moves_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Anchor remains fixed through multiple select movements.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    // First move establishes anchor
    move_left_select(&mut cursor, &mut selection, &boundaries);
    let anchor_after_first = selection.anchor;

    // Subsequent moves must preserve anchor
    move_left_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        selection.anchor, anchor_after_first,
        "Anchor must remain fixed through multiple select moves"
    );

    move_left_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        selection.anchor, anchor_after_first,
        "Anchor must remain fixed through multiple select moves"
    );
}

#[test]
fn move_right_select_preserves_anchor_through_multiple_moves_SPEC_107_3_3() {
    // SPEC-HAL-107 §3.3: Anchor remains fixed through multiple select movements.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // First move establishes anchor
    move_right_select(&mut cursor, &mut selection, &boundaries);
    let anchor_after_first = selection.anchor;

    // Subsequent moves must preserve anchor
    move_right_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        selection.anchor, anchor_after_first,
        "Anchor must remain fixed through multiple select moves"
    );

    move_right_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        selection.anchor, anchor_after_first,
        "Anchor must remain fixed through multiple select moves"
    );
}

// ============================================================================
// SPEC-HAL-107 §3.4 — Boundary Legality
// ============================================================================

#[test]
fn cursor_lands_on_valid_boundary_after_move_left_SPEC_107_3_4() {
    // SPEC-HAL-107 §3.4: Cursor always lands on an existing boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor.pos is a valid boundary
    let is_valid_boundary = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid_boundary,
        "Cursor must land on valid boundary after move_left. pos={}, boundaries={:?}",
        cursor.pos,
        boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
    );
}

#[test]
fn cursor_lands_on_valid_boundary_after_move_right_SPEC_107_3_4() {
    // SPEC-HAL-107 §3.4: Cursor always lands on an existing boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Assert: cursor.pos is a valid boundary
    let is_valid_boundary = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid_boundary,
        "Cursor must land on valid boundary after move_right. pos={}, boundaries={:?}",
        cursor.pos,
        boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
    );
}

#[test]
fn cursor_lands_on_valid_boundary_through_full_traversal_SPEC_107_3_4() {
    // SPEC-HAL-107 §3.4: Every position visited during traversal is a valid boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    // Traverse left to right
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    while cursor.pos < text.len() {
        let is_valid = boundaries.iter().any(|b| b.byte_start == cursor.pos);
        assert!(
            is_valid,
            "Every cursor position must be valid boundary during traversal. pos={}",
            cursor.pos
        );
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
    }

    // Final position must also be valid
    let is_valid = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid,
        "Final cursor position must be valid boundary. pos={}",
        cursor.pos
    );
}

// ============================================================================
// SPEC-HAL-107 §3.5 — UTF-8 Safety
// ============================================================================

#[test]
fn move_left_never_lands_inside_utf8_codepoint_SPEC_107_3_5() {
    // SPEC-HAL-107 §3.5: Movement never lands inside a multi-byte codepoint.

    // "Héllo日本" - mixed ASCII and multi-byte
    let text = "Héllo日本";
    let boundaries = make_utf8_boundaries(text);

    // Start at end, walk left
    let mut cursor = TextCursor::at(text.len());
    let mut selection = Selection::at(text.len());

    while cursor.pos > 0 {
        move_left_collapse(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_left in {:?}",
            cursor.pos,
            text
        );
    }
}

#[test]
fn move_right_never_lands_inside_utf8_codepoint_SPEC_107_3_5() {
    // SPEC-HAL-107 §3.5: Movement never lands inside a multi-byte codepoint.

    // "Héllo日本" - mixed ASCII and multi-byte
    let text = "Héllo日本";
    let boundaries = make_utf8_boundaries(text);

    // Start at beginning, walk right
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    while cursor.pos < text.len() {
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_right in {:?}",
            cursor.pos,
            text
        );
    }
}

#[test]
fn move_left_select_never_lands_inside_utf8_codepoint_SPEC_107_3_5() {
    // SPEC-HAL-107 §3.5: Select movement also respects UTF-8 boundaries.

    let text = "日本語テスト"; // All multi-byte
    let boundaries = make_utf8_boundaries(text);

    let mut cursor = TextCursor::at(text.len());
    let mut selection = Selection::at(text.len());

    while cursor.pos > 0 {
        move_left_select(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_left_select",
            cursor.pos
        );
    }
}

#[test]
fn move_right_select_never_lands_inside_utf8_codepoint_SPEC_107_3_5() {
    // SPEC-HAL-107 §3.5: Select movement also respects UTF-8 boundaries.

    let text = "日本語テスト"; // All multi-byte
    let boundaries = make_utf8_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    while cursor.pos < text.len() {
        move_right_select(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_right_select",
            cursor.pos
        );
    }
}

#[test]
fn utf8_movement_visits_correct_byte_positions_SPEC_107_3_5() {
    // SPEC-HAL-107 §3.5: Movement through UTF-8 text visits correct byte offsets.

    // "é" is 2 bytes (0xC3 0xA9), "日" is 3 bytes
    let text = "Aé日B"; // A(1) + é(2) + 日(3) + B(1) = 7 bytes
    let boundaries = make_utf8_boundaries(text);

    // Expected byte positions: 0 (A), 1 (é), 3 (日), 6 (B), 7 (sentinel)
    let expected = [0, 1, 3, 6, 7];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    let mut visited = vec![cursor.pos];
    while cursor.pos < text.len() {
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
        visited.push(cursor.pos);
    }

    assert_eq!(
        visited, expected,
        "UTF-8 movement must visit exact byte positions. Expected {:?}, got {:?}",
        expected, visited
    );
}

// ============================================================================
// SPEC-HAL-107 §7.1 — Per-Line Horizontal Movement (Multi-Line)
// ============================================================================

/// Helper: Create boundaries for multi-line text with explicit line_index.
/// Text: "AB\nCD\nEF" (3 lines, 2 chars each + newlines)
fn make_multiline_boundaries() -> Vec<ClusterBoundary> {
    // Line 0: "AB\n" -> bytes 0,1,2 (newline at 2)
    // Line 1: "CD\n" -> bytes 3,4,5 (newline at 5)
    // Line 2: "EF"   -> bytes 6,7 (no trailing newline)
    vec![
        // Line 0: A, B, \n
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
        }, // \n sentinel for line 0
        // Line 1: C, D, \n
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 20.0,
        }, // \n sentinel for line 1
        // Line 2: E, F, sentinel
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 2,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8,
            line_index: 2,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 8,
            byte_end: 8,
            line_index: 2,
            x_logical: 20.0,
        }, // sentinel
    ]
}

#[test]
fn cursor_left_moves_to_previous_boundary_SPEC_107_7_1() {
    // SPEC-HAL-107 §7.1: Left arrow moves to previous boundary.byte_start on same line.
    // Text: "AB\nCD\nEF"
    let boundaries = make_multiline_boundaries();

    // Cursor at position 4 (D on line 1), move left should go to 3 (C on line 1)
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, 3,
        "SPEC-HAL-107 §7.1: Left must move to previous boundary on same line. Expected 3 (C), got {}",
        cursor.pos
    );

    // Verify we stayed on line 1
    let landing_boundary = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(
        landing_boundary.line_index, 1,
        "SPEC-HAL-107 §7.1: Left must not cross line boundary. Expected line 1, got {}",
        landing_boundary.line_index
    );
}

#[test]
fn cursor_right_moves_to_next_boundary_SPEC_107_7_1() {
    // SPEC-HAL-107 §7.1: Right arrow moves to next boundary.byte_start on same line.
    // Text: "AB\nCD\nEF"
    let boundaries = make_multiline_boundaries();

    // Cursor at position 3 (C on line 1), move right should go to 4 (D on line 1)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, 4,
        "SPEC-HAL-107 §7.1: Right must move to next boundary on same line. Expected 4 (D), got {}",
        cursor.pos
    );

    // Verify we stayed on line 1
    let landing_boundary = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(
        landing_boundary.line_index, 1,
        "SPEC-HAL-107 §7.1: Right must not cross line boundary. Expected line 1, got {}",
        landing_boundary.line_index
    );
}

#[test]
fn cursor_left_at_line_start_crosses_to_prev_line_newline_k2_inv3() {
    // K2.INV3: At start of line, left moves to previous line's newline.
    // This overrides SPEC-HAL-107 §7.1 for newline traversal.
    // Text: "AB\nCD\nEF"
    let boundaries = make_multiline_boundaries();

    // Cursor at position 3 (C, start of line 1), move left should go to newline at 2
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, 2,
        "K2.INV3: Left at line start must move to previous line's newline. Expected 2, got {}",
        cursor.pos
    );
}

#[test]
fn cursor_right_at_newline_crosses_to_next_line_k2_inv3() {
    // K2.INV3: At newline boundary, right moves to next line start.
    // This overrides SPEC-HAL-107 §7.1 for newline traversal.
    // Text: "AB\nCD\nEF"
    let boundaries = make_multiline_boundaries();

    // Cursor at position 5 (newline at end of line 1), move right should cross to 6 (E on line 2)
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, 6,
        "K2.INV3: Right at newline must move to next line start. Expected 6, got {}",
        cursor.pos
    );
}
