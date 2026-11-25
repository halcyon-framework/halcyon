// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.
//
// NOTE:
// These tests construct multi-line boundaries manually.
// line_index field determines visual line membership.

#![allow(non_snake_case, unused_variables)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, move_down, move_down_select, move_left_collapse,
    move_right_collapse, move_up, move_up_select,
};

// ============================================================================
// Helper: Create boundaries for multi-line ASCII text
// ============================================================================

/// Create boundaries for multi-line text.
/// Each character gets a boundary; newlines are treated as line separators.
/// Line indices are assigned based on newline positions.
fn make_multiline_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_on_line = 0.0;
    let char_width = 10.0;

    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index,
            x_logical: x_on_line,
        });

        if ch == '\n' {
            line_index += 1;
            x_on_line = 0.0;
        } else {
            x_on_line += char_width;
        }
    }

    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: x_on_line,
    });

    boundaries
}

/// Create boundaries for single-line ASCII text.
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

// ============================================================================
// SPEC-HAL-107 §4.1 — Vertical Movement is Line-Based
// ============================================================================

#[test]
fn move_up_changes_line_not_just_bytes_SPEC_107_4_1() {
    // SPEC-HAL-107 §4.1: Vertical movement is line-based, not byte-based.

    let text = "ABC\nDEF\nGHI";
    // Line 0: A(0) B(1) C(2) \n(3)
    // Line 1: D(4) E(5) F(6) \n(7)
    // Line 2: G(8) H(9) I(10) sentinel(11)
    let boundaries = make_multiline_boundaries(text);

    // Start on line 2 (at 'H', byte 9)
    let mut cursor = TextCursor::at(9);
    let mut selection = Selection::at(9);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should land on line 1, not just move back a few bytes
    // 'H' is at x=10.0 (second char), so should land on 'E' at byte 5
    assert!(
        cursor.pos >= 4 && cursor.pos <= 7,
        "move_up must land on line 1 (bytes 4-7), got {}",
        cursor.pos
    );
}

#[test]
fn move_down_changes_line_not_just_bytes_SPEC_107_4_1() {
    // SPEC-HAL-107 §4.1: Vertical movement is line-based, not byte-based.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    // Start on line 0 (at 'B', byte 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should land on line 1, not just move forward a few bytes
    assert!(
        cursor.pos >= 4 && cursor.pos <= 7,
        "move_down must land on line 1 (bytes 4-7), got {}",
        cursor.pos
    );
}

// ============================================================================
// SPEC-HAL-107 §4.2 — Preferred Column Handling
// ============================================================================

#[test]
fn preferred_column_captured_on_first_vertical_move_SPEC_107_4_2() {
    // SPEC-HAL-107 §4.2: Preferred column captured on first vertical move.

    let text = "ABCDE\nFGH\nIJKLM";
    let boundaries = make_multiline_boundaries(text);

    // Start on line 0, column 3 (at 'D', x=30.0)
    let mut cursor = TextCursor::at(3);
    cursor.preferred_x = None; // Ensure not set
    let mut selection = Selection::at(3);

    // First vertical move should capture preferred_x
    move_down(&mut cursor, &mut selection, &boundaries);

    assert!(
        cursor.preferred_x.is_some(),
        "preferred_x must be set after first vertical move"
    );
}

#[test]
fn preferred_column_preserved_across_consecutive_vertical_moves_SPEC_107_4_2() {
    // SPEC-HAL-107 §4.2: Preferred column preserved across consecutive vertical moves.

    // Lines of varying lengths:
    // Line 0: ABCDEFGH (8 chars, x: 0-70)
    // Line 1: IJ       (2 chars, x: 0-10)
    // Line 2: KLMNOPQR (8 chars, x: 0-70)
    let text = "ABCDEFGH\nIJ\nKLMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    // Start on line 0, column 5 (at 'F', x=50.0)
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = None;
    let mut selection = Selection::at(5);

    // Move down to short line 1 - cursor will clamp to end
    move_down(&mut cursor, &mut selection, &boundaries);
    let preferred_after_first = cursor.preferred_x;

    // Move down to line 2 - should return to column 5 position
    move_down(&mut cursor, &mut selection, &boundaries);
    let preferred_after_second = cursor.preferred_x;

    assert_eq!(
        preferred_after_first, preferred_after_second,
        "preferred_x must be preserved across consecutive vertical moves"
    );
}

#[test]
fn preferred_column_cleared_on_horizontal_move_SPEC_107_4_2() {
    // SPEC-HAL-107 §4.2: Horizontal movement clears preferred column.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    // Start with a vertical move to establish preferred_x
    let mut cursor = TextCursor::at(5); // Line 1, 'E'
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);
    assert!(
        cursor.preferred_x.is_some(),
        "preferred_x should be set after vertical move"
    );

    // Horizontal move should clear preferred_x
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    assert!(
        cursor.preferred_x.is_none(),
        "preferred_x must be cleared after horizontal move"
    );
}

#[test]
fn horizontal_move_left_clears_preferred_column_SPEC_107_4_2() {
    // SPEC-HAL-107 §4.2: Left arrow clears preferred column.

    let text = "ABC\nDEF";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Establish preferred_x
    move_up(&mut cursor, &mut selection, &boundaries);
    assert!(cursor.preferred_x.is_some());

    // Left should clear it
    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    assert!(
        cursor.preferred_x.is_none(),
        "preferred_x must be cleared after move_left"
    );
}

// ============================================================================
// SPEC-HAL-107 §4.3 — Edge Clamping
// ============================================================================

#[test]
fn move_up_on_first_line_clamps_to_start_SPEC_107_4_3() {
    // SPEC-HAL-107 §4.3: Up on first line moves to start of line.

    let text = "ABCDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    // Start on line 0, in the middle (at 'D', byte 3)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should clamp to start of line 0 (byte 0)
    assert_eq!(
        cursor.pos, 0,
        "move_up on first line must move to line start"
    );
}

#[test]
fn move_down_on_last_line_is_noop_SPEC_107_4_3() {
    // SPEC-HAL-107 §4.3: Down on last line does not change position.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    // Start on line 2 (at 'H', byte 9)
    let mut cursor = TextCursor::at(9);
    let mut selection = Selection::at(9);

    let old_pos = cursor.pos;
    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, old_pos,
        "move_down on last line must not change position"
    );
}

#[test]
fn move_up_at_document_start_is_noop_SPEC_107_4_3() {
    // SPEC-HAL-107 §4.3: Up at position 0 on first line is no-op.

    let text = "ABCDEF";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_up(&mut cursor, &mut selection, &boundaries);

    // At start of first line, up should stay at 0
    assert_eq!(cursor.pos, 0, "move_up at document start must stay at 0");
}

#[test]
fn move_down_at_document_end_is_noop_SPEC_107_4_3() {
    // SPEC-HAL-107 §4.3: Down at document end on last line is no-op.

    let text = "ABC";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(3); // Sentinel position
    let mut selection = Selection::at(3);

    let old_pos = cursor.pos;
    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, old_pos,
        "move_down at document end must not change position"
    );
}

// ============================================================================
// SPEC-HAL-107 §4.4 — Collapse vs Select
// ============================================================================

#[test]
fn move_up_collapses_selection_SPEC_107_4_4() {
    // SPEC-HAL-107 §4.4: move_up clears/collapses selection.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(9); // Line 2
    let mut selection = Selection::new();
    selection.anchor = 5;
    selection.head = 9; // Non-empty selection

    move_up(&mut cursor, &mut selection, &boundaries);

    assert!(selection.is_empty(), "move_up must collapse selection");
    assert_eq!(
        selection.anchor, cursor.pos,
        "selection.anchor must equal cursor after collapse"
    );
}

#[test]
fn move_down_collapses_selection_SPEC_107_4_4() {
    // SPEC-HAL-107 §4.4: move_down clears/collapses selection.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(1); // Line 0
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 2; // Non-empty selection

    move_down(&mut cursor, &mut selection, &boundaries);

    assert!(selection.is_empty(), "move_down must collapse selection");
}

#[test]
fn move_up_select_extends_selection_SPEC_107_4_4() {
    // SPEC-HAL-107 §4.4: move_up_select extends selection, anchor fixed.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(9); // Line 2, 'H'
    let mut selection = Selection::at(9);

    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should be at original position (9)
    assert_eq!(
        selection.anchor, 9,
        "move_up_select must preserve anchor at original position"
    );

    // Head should follow cursor
    assert_eq!(
        selection.head, cursor.pos,
        "move_up_select: head must follow cursor"
    );

    // Selection should be non-empty
    assert!(
        !selection.is_empty(),
        "Selection must be non-empty after move_up_select"
    );
}

#[test]
fn move_down_select_extends_selection_SPEC_107_4_4() {
    // SPEC-HAL-107 §4.4: move_down_select extends selection, anchor fixed.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(1); // Line 0, 'B'
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should be at original position (1)
    assert_eq!(
        selection.anchor, 1,
        "move_down_select must preserve anchor at original position"
    );

    // Head should follow cursor
    assert_eq!(
        selection.head, cursor.pos,
        "move_down_select: head must follow cursor"
    );

    // Selection should be non-empty
    assert!(
        !selection.is_empty(),
        "Selection must be non-empty after move_down_select"
    );
}

#[test]
fn move_up_select_preserves_anchor_through_multiple_moves_SPEC_107_4_4() {
    // SPEC-HAL-107 §4.4: Anchor preserved through multiple select moves.

    let text = "ABC\nDEF\nGHI";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(9); // Line 2
    let mut selection = Selection::at(9);

    move_up_select(&mut cursor, &mut selection, &boundaries);
    let anchor_after_first = selection.anchor;

    move_up_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        selection.anchor, anchor_after_first,
        "Anchor must remain fixed through multiple move_up_select"
    );
}

// ============================================================================
// SPEC-HAL-107 §4.5 — Boundary Validity
// ============================================================================

#[test]
fn cursor_lands_on_valid_boundary_after_move_up_SPEC_107_4_5() {
    // SPEC-HAL-107 §4.5: Cursor always lands on an existing boundary.

    let text = "ABCDEF\nGHIJKL\nMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(10); // Line 1
    let mut selection = Selection::at(10);

    move_up(&mut cursor, &mut selection, &boundaries);

    let is_valid_boundary = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid_boundary,
        "cursor.pos {} must be a valid boundary after move_up",
        cursor.pos
    );
}

#[test]
fn cursor_lands_on_valid_boundary_after_move_down_SPEC_107_4_5() {
    // SPEC-HAL-107 §4.5: Cursor always lands on an existing boundary.

    let text = "ABCDEF\nGHIJKL\nMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(3); // Line 0
    let mut selection = Selection::at(3);

    move_down(&mut cursor, &mut selection, &boundaries);

    let is_valid_boundary = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid_boundary,
        "cursor.pos {} must be a valid boundary after move_down",
        cursor.pos
    );
}

#[test]
fn cursor_lands_on_valid_boundary_through_full_vertical_traversal_SPEC_107_4_5() {
    // SPEC-HAL-107 §4.5: Every position visited during vertical traversal is valid.

    let text = "Line1\nLine2\nLine3\nLine4";
    let boundaries = make_multiline_boundaries(text);

    // Start at bottom, traverse up
    let mut cursor = TextCursor::at(18); // Line 3
    let mut selection = Selection::at(18);

    // Move up through all lines
    for _ in 0..4 {
        let is_valid = boundaries.iter().any(|b| b.byte_start == cursor.pos);
        assert!(
            is_valid,
            "Every cursor position must be valid boundary during vertical traversal. pos={}",
            cursor.pos
        );
        move_up(&mut cursor, &mut selection, &boundaries);
    }
}

// ============================================================================
// SPEC-HAL-107 §4.6 — UTF-8 Safety
// ============================================================================

/// Create boundaries for multi-line text with UTF-8 characters.
fn make_multiline_utf8_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_on_line = 0.0;
    let char_width = 10.0;

    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index,
            x_logical: x_on_line,
        });

        if ch == '\n' {
            line_index += 1;
            x_on_line = 0.0;
        } else {
            x_on_line += char_width;
        }
    }

    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: x_on_line,
    });

    boundaries
}

#[test]
fn move_up_never_lands_inside_utf8_codepoint_SPEC_107_4_6() {
    // SPEC-HAL-107 §4.6: Vertical movement never lands inside a multi-byte codepoint.

    // Mix of ASCII and multi-byte chars on each line
    let text = "Héllo\n日本語\nWorld";
    let boundaries = make_multiline_utf8_boundaries(text);

    // Start on last line
    let mut cursor = TextCursor::at(text.rfind('W').unwrap());
    let mut selection = Selection::at(cursor.pos);

    // Move up through all lines
    for _ in 0..3 {
        move_up(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_up",
            cursor.pos
        );
    }
}

#[test]
fn move_down_never_lands_inside_utf8_codepoint_SPEC_107_4_6() {
    // SPEC-HAL-107 §4.6: Vertical movement never lands inside a multi-byte codepoint.

    let text = "Héllo\n日本語\nWorld";
    let boundaries = make_multiline_utf8_boundaries(text);

    // Start at beginning
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Move down through all lines
    for _ in 0..3 {
        move_down(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_down",
            cursor.pos
        );
    }
}

#[test]
fn move_up_select_never_lands_inside_utf8_codepoint_SPEC_107_4_6() {
    // SPEC-HAL-107 §4.6: Select movement also respects UTF-8 boundaries.

    let text = "日本\n語テ\nスト";
    let boundaries = make_multiline_utf8_boundaries(text);

    // Start on last line
    let mut cursor = TextCursor::at(text.len() - 3); // Before last char
    let mut selection = Selection::at(cursor.pos);

    for _ in 0..3 {
        move_up_select(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_up_select",
            cursor.pos
        );
    }
}

#[test]
fn move_down_select_never_lands_inside_utf8_codepoint_SPEC_107_4_6() {
    // SPEC-HAL-107 §4.6: Select movement also respects UTF-8 boundaries.

    let text = "日本\n語テ\nスト";
    let boundaries = make_multiline_utf8_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    for _ in 0..3 {
        move_down_select(&mut cursor, &mut selection, &boundaries);
        assert!(
            text.is_char_boundary(cursor.pos),
            "cursor.pos {} must be char boundary after move_down_select",
            cursor.pos
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §4.7 — Empty Document / Single Line
// ============================================================================

#[test]
fn move_up_on_empty_boundaries_is_noop_SPEC_107_4_7() {
    // SPEC-HAL-107 §4.7: Empty boundaries = no-op.

    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0, "move_up with empty boundaries must be no-op");
}

#[test]
fn move_down_on_empty_boundaries_is_noop_SPEC_107_4_7() {
    // SPEC-HAL-107 §4.7: Empty boundaries = no-op.

    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, 0,
        "move_down with empty boundaries must be no-op"
    );
}

#[test]
fn single_line_move_up_clamps_to_start_SPEC_107_4_7() {
    // SPEC-HAL-107 §4.7: Single line - up moves to start.

    let text = "SingleLine";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0, "move_up on single line must move to start");
}

#[test]
fn single_line_move_down_is_noop_SPEC_107_4_7() {
    // SPEC-HAL-107 §4.7: Single line - down is no-op.

    let text = "SingleLine";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    let old_pos = cursor.pos;
    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(
        cursor.pos, old_pos,
        "move_down on single line must not change position"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.2 — Horizontal Intent Preservation (Vertical Movement)
// ============================================================================

#[test]
fn cursor_down_preserves_horizontal_intent_SPEC_107_7_2() {
    // SPEC-HAL-107 §7.2: Down arrow preserves horizontal intent (preferred_x).
    // When moving down to a line of equal or greater width, cursor lands at same X.

    // Lines of equal width:
    // Line 0: ABCDE\n  (5 chars, x: 0,10,20,30,40)
    // Line 1: FGHIJ\n  (5 chars, x: 0,10,20,30,40)
    // Line 2: KLMNO    (5 chars, x: 0,10,20,30,40)
    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2, x=20.0)
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = None;
    let mut selection = Selection::at(2);

    // Move down to line 1
    move_down(&mut cursor, &mut selection, &boundaries);

    // Should land at 'H' on line 1 (byte 8, x=20.0 - same horizontal position)
    assert_eq!(
        cursor.pos, 8,
        "SPEC-HAL-107 §7.2: Down must land at same X position. Expected 8 ('H'), got {}",
        cursor.pos
    );

    // preferred_x should be set
    assert!(
        cursor.preferred_x.is_some(),
        "preferred_x must be set after vertical move"
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        20.0,
        "preferred_x must match original X position"
    );
}

#[test]
fn cursor_up_preserves_horizontal_intent_SPEC_107_7_2() {
    // SPEC-HAL-107 §7.2: Up arrow preserves horizontal intent (preferred_x).
    // When moving up to a line of equal or greater width, cursor lands at same X.

    // Lines of equal width:
    // Line 0: ABCDE\n  (5 chars)
    // Line 1: FGHIJ\n  (5 chars)
    // Line 2: KLMNO    (5 chars)
    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'N' on line 2 (byte 15, x=30.0)
    let mut cursor = TextCursor::at(15);
    cursor.preferred_x = None;
    let mut selection = Selection::at(15);

    // Move up to line 1
    move_up(&mut cursor, &mut selection, &boundaries);

    // Should land at 'I' on line 1 (byte 9, x=30.0 - same horizontal position)
    // Line 1: F(6,x=0) G(7,x=10) H(8,x=20) I(9,x=30) J(10,x=40)
    assert_eq!(
        cursor.pos, 9,
        "SPEC-HAL-107 §7.2: Up must land at same X position. Expected 9 ('I'), got {}",
        cursor.pos
    );

    // preferred_x should be preserved
    assert!(
        cursor.preferred_x.is_some(),
        "preferred_x must be set after vertical move"
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        30.0,
        "preferred_x must match original X position"
    );
}

#[test]
fn cursor_down_clamps_to_line_end_if_shorter_SPEC_107_7_2() {
    // SPEC-HAL-107 §7.2: If target line is shorter, cursor clamps to line end.
    // Horizontal intent is preserved in preferred_x for future moves.

    // Lines of varying widths:
    // Line 0: ABCDEFGH\n  (8 chars, x: 0-70)
    // Line 1: IJ\n        (2 chars, x: 0-10)
    // Line 2: KLMNOPQR    (8 chars, x: 0-70)
    let text = "ABCDEFGH\nIJ\nKLMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'F' on line 0 (byte 5, x=50.0)
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = None;
    let mut selection = Selection::at(5);

    // Move down to short line 1
    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 is only "IJ\n", so cursor should clamp to end of content
    // 'J' is at byte 10, \n is at byte 11
    // Expected: land at 'J' (byte 10) or newline boundary (byte 11)
    assert!(
        cursor.pos >= 9 && cursor.pos <= 11,
        "SPEC-HAL-107 §7.2: Down to shorter line must clamp to line end. Expected 9-11, got {}",
        cursor.pos
    );

    // preferred_x must be PRESERVED at original value (50.0), not overwritten
    assert_eq!(
        cursor.preferred_x.unwrap(),
        50.0,
        "SPEC-HAL-107 §7.2: preferred_x must be preserved when clamping to shorter line"
    );

    // Move down again to line 2 (long line) - should return to original X
    move_down(&mut cursor, &mut selection, &boundaries);

    // Should land at 'P' on line 2 (x=50.0)
    // Line 2 starts at byte 12: K(12) L(13) M(14) N(15) O(16) P(17) Q(18) R(19)
    assert_eq!(
        cursor.pos, 17,
        "SPEC-HAL-107 §7.2: After short line, must return to preferred_x. Expected 17 ('P'), got {}",
        cursor.pos
    );
}

#[test]
fn cursor_up_clamps_to_line_end_if_shorter_SPEC_107_7_2() {
    // SPEC-HAL-107 §7.2: If target line is shorter, cursor clamps to line end.
    // Horizontal intent is preserved in preferred_x for future moves.

    // Lines of varying widths:
    // Line 0: ABCDEFGH\n  (8 chars)
    // Line 1: IJ\n        (2 chars)
    // Line 2: KLMNOPQR    (8 chars)
    let text = "ABCDEFGH\nIJ\nKLMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'P' on line 2 (byte 17, x=50.0)
    let mut cursor = TextCursor::at(17);
    cursor.preferred_x = None;
    let mut selection = Selection::at(17);

    // Move up to short line 1
    move_up(&mut cursor, &mut selection, &boundaries);

    // Should clamp to end of line 1
    assert!(
        cursor.pos >= 9 && cursor.pos <= 11,
        "SPEC-HAL-107 §7.2: Up to shorter line must clamp to line end. Expected 9-11, got {}",
        cursor.pos
    );

    // preferred_x must be preserved
    assert_eq!(
        cursor.preferred_x.unwrap(),
        50.0,
        "SPEC-HAL-107 §7.2: preferred_x must be preserved when clamping to shorter line"
    );

    // Move up again to line 0 (long line)
    move_up(&mut cursor, &mut selection, &boundaries);

    // Should land at 'F' on line 0 (byte 5, x=50.0)
    assert_eq!(
        cursor.pos, 5,
        "SPEC-HAL-107 §7.2: After short line, must return to preferred_x. Expected 5 ('F'), got {}",
        cursor.pos
    );
}

#[test]
fn cursor_vertical_movement_ignores_previous_line_width_SPEC_107_7_2() {
    // SPEC-HAL-107 §7.2: Vertical movement uses preferred_x, not previous line width.
    // Moving from short line to long line should NOT stop at previous line's width.

    // Lines:
    // Line 0: ABCDEFGHIJ\n  (10 chars, x: 0-90)
    // Line 1: KL\n          (2 chars, x: 0-10)
    // Line 2: MNOPQRSTUV    (10 chars, x: 0-90)
    let text = "ABCDEFGHIJ\nKL\nMNOPQRSTUV";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 0 (byte 7, x=70.0)
    let mut cursor = TextCursor::at(7);
    cursor.preferred_x = None;
    let mut selection = Selection::at(7);

    // Move down to short line 1 - clamps to 'L' but remembers x=70
    move_down(&mut cursor, &mut selection, &boundaries);
    let pos_after_first = cursor.pos;

    // Move down to long line 2 - should land at x=70, NOT at previous line's end
    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 2 starts at byte 14: M(14) N(15) O(16) P(17) Q(18) R(19) S(20) T(21) U(22) V(23)
    // x=70 corresponds to 'T' at byte 21
    assert_eq!(
        cursor.pos, 21,
        "SPEC-HAL-107 §7.2: Must use preferred_x, not previous line width. Expected 21 ('T'), got {}. Previous pos was {}",
        cursor.pos, pos_after_first
    );

    // Verify preferred_x is still 70.0
    assert_eq!(
        cursor.preferred_x.unwrap(),
        70.0,
        "preferred_x must remain at original intent (70.0)"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.3 — Vertical Movement Across Empty / Zero-Width Lines
// ============================================================================

#[test]
fn cursor_down_into_empty_line_lands_at_start_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Down into empty line lands at line start.
    // preferred_x is preserved for future moves.

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty line
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2, x=20.0)
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = None;
    let mut selection = Selection::at(2);

    // Move down to empty line 1
    move_down(&mut cursor, &mut selection, &boundaries);

    // Should land at line 1 start (the \n at byte 6)
    assert_eq!(
        cursor.pos, 6,
        "SPEC-HAL-107 §7.3: Down into empty line must land at line start. Expected 6, got {}",
        cursor.pos
    );

    // preferred_x must be preserved (20.0 from 'C')
    assert_eq!(
        cursor.preferred_x.unwrap(),
        20.0,
        "SPEC-HAL-107 §7.3: preferred_x must be preserved when entering empty line"
    );
}

#[test]
fn cursor_up_into_empty_line_lands_at_start_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Up into empty line lands at line start.
    // preferred_x is preserved for future moves.

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty line
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 2 (byte 9, x=20.0)
    let mut cursor = TextCursor::at(9);
    cursor.preferred_x = None;
    let mut selection = Selection::at(9);

    // Move up to empty line 1
    move_up(&mut cursor, &mut selection, &boundaries);

    // Should land at line 1 start (the \n at byte 6)
    assert_eq!(
        cursor.pos, 6,
        "SPEC-HAL-107 §7.3: Up into empty line must land at line start. Expected 6, got {}",
        cursor.pos
    );

    // preferred_x must be preserved (20.0 from 'H')
    assert_eq!(
        cursor.preferred_x.unwrap(),
        20.0,
        "SPEC-HAL-107 §7.3: preferred_x must be preserved when entering empty line"
    );
}

#[test]
fn cursor_down_across_consecutive_empty_lines_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Down across multiple consecutive empty lines.
    // Deterministic per-line landing, no skipping, preferred_x unchanged.

    // Text: "ABC\n\n\n\nDEF"
    // Line 0: A(0) B(1) C(2) \n(3)
    // Line 1: \n(4) - empty
    // Line 2: \n(5) - empty
    // Line 3: \n(6) - empty
    // Line 4: D(7) E(8) F(9) sentinel(10)
    let text = "ABC\n\n\n\nDEF";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'B' on line 0 (byte 1, x=10.0)
    let mut cursor = TextCursor::at(1);
    cursor.preferred_x = None;
    let mut selection = Selection::at(1);

    // Move down - should land on line 1 (empty)
    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 4,
        "First down: expected line 1 (byte 4), got {}",
        cursor.pos
    );
    let px = cursor.preferred_x.unwrap();

    // Move down - should land on line 2 (empty)
    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 5,
        "Second down: expected line 2 (byte 5), got {}",
        cursor.pos
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px,
        "preferred_x must not change"
    );

    // Move down - should land on line 3 (empty)
    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 6,
        "Third down: expected line 3 (byte 6), got {}",
        cursor.pos
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px,
        "preferred_x must not change"
    );
}

#[test]
fn cursor_up_across_consecutive_empty_lines_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Up across multiple consecutive empty lines.
    // Deterministic per-line landing, no skipping, preferred_x unchanged.

    // Text: "ABC\n\n\n\nDEF"
    // Line 0: A(0) B(1) C(2) \n(3)
    // Line 1: \n(4) - empty
    // Line 2: \n(5) - empty
    // Line 3: \n(6) - empty
    // Line 4: D(7) E(8) F(9) sentinel(10)
    let text = "ABC\n\n\n\nDEF";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'E' on line 4 (byte 8, x=10.0)
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = None;
    let mut selection = Selection::at(8);

    // Move up - should land on line 3 (empty)
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 6,
        "First up: expected line 3 (byte 6), got {}",
        cursor.pos
    );
    let px = cursor.preferred_x.unwrap();

    // Move up - should land on line 2 (empty)
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 5,
        "Second up: expected line 2 (byte 5), got {}",
        cursor.pos
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px,
        "preferred_x must not change"
    );

    // Move up - should land on line 1 (empty)
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 4,
        "Third up: expected line 1 (byte 4), got {}",
        cursor.pos
    );
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px,
        "preferred_x must not change"
    );
}

#[test]
fn cursor_empty_to_nonempty_reprojects_preferred_x_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Moving from empty line to non-empty line.
    // Reproject using preserved preferred_x, ignore empty line width (0).

    // Text: "ABCDEFGH\n\nIJKLMNOP"
    // Line 0: A(0) B(1) C(2) D(3) E(4) F(5) G(6) H(7) \n(8)
    // Line 1: \n(9) - empty
    // Line 2: I(10) J(11) K(12) L(13) M(14) N(15) O(16) P(17) sentinel(18)
    let text = "ABCDEFGH\n\nIJKLMNOP";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'F' on line 0 (byte 5, x=50.0)
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = None;
    let mut selection = Selection::at(5);

    // Move down to empty line 1
    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 9, "Should land on empty line 1");
    assert_eq!(cursor.preferred_x.unwrap(), 50.0, "preferred_x preserved");

    // Move down to line 2 - should reproject to x=50
    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 2: I(10,x=0) J(11,x=10) K(12,x=20) L(13,x=30) M(14,x=40) N(15,x=50)
    assert_eq!(
        cursor.pos, 15,
        "SPEC-HAL-107 §7.3: Must reproject preferred_x from empty line. Expected 15 ('N'), got {}",
        cursor.pos
    );
}

#[test]
fn cursor_nonempty_empty_nonempty_preserves_intent_SPEC_107_7_3() {
    // SPEC-HAL-107 §7.3: Non-empty → empty → non-empty sequence.
    // Empty lines do not reset or clamp intent.
    // Final landing uses original preferred_x.

    // Text: "ABCDEFGHIJ\n\n\nKLMNOPQRST"
    // Line 0: A-J (10 chars, x: 0-90)
    // Line 1: empty
    // Line 2: empty
    // Line 3: K-T (10 chars, x: 0-90)
    let text = "ABCDEFGHIJ\n\n\nKLMNOPQRST";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 0 (byte 7, x=70.0)
    let mut cursor = TextCursor::at(7);
    cursor.preferred_x = None;
    let mut selection = Selection::at(7);

    // Move down through empty lines to line 3
    move_down(&mut cursor, &mut selection, &boundaries); // Line 1 (empty)
    move_down(&mut cursor, &mut selection, &boundaries); // Line 2 (empty)
    move_down(&mut cursor, &mut selection, &boundaries); // Line 3 (non-empty)

    // Should land at x=70 on line 3
    // Line 3 starts at byte 13: K(13) L(14) M(15) N(16) O(17) P(18) Q(19) R(20) S(21) T(22)
    // x=70 corresponds to 'R' at byte 20
    assert_eq!(
        cursor.pos, 20,
        "SPEC-HAL-107 §7.3: After empty lines, must use original preferred_x. Expected 20 ('R'), got {}",
        cursor.pos
    );

    // preferred_x must still be 70.0
    assert_eq!(
        cursor.preferred_x.unwrap(),
        70.0,
        "preferred_x must remain at original intent through empty lines"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.4 — Vertical Movement Edge Constraints
// ============================================================================

#[test]
fn cursor_up_at_top_is_noop_preserves_preferred_x_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: move_up on first line is noop, preferred_x unchanged.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2, x=20.0), set preferred_x
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(2);

    let pos_before = cursor.pos;
    let px_before = cursor.preferred_x;

    // move_up at top line
    move_up(&mut cursor, &mut selection, &boundaries);

    // Position should clamp to line start (existing behavior) but preferred_x preserved
    // Note: §4.3 says "up on first line moves to start", so pos changes to 0
    // But preferred_x must NOT be reset
    assert_eq!(
        cursor.preferred_x, px_before,
        "SPEC-HAL-107 §7.4: preferred_x must not change at top boundary. Was {:?}, now {:?}",
        px_before, cursor.preferred_x
    );
}

#[test]
fn cursor_down_at_bottom_is_noop_preserves_preferred_x_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: move_down on last line is noop, preferred_x unchanged.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'M' on line 2 (byte 14, x=20.0), set preferred_x
    let mut cursor = TextCursor::at(14);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(14);

    let pos_before = cursor.pos;
    let px_before = cursor.preferred_x;

    // move_down at bottom line
    move_down(&mut cursor, &mut selection, &boundaries);

    // Position should not change (noop)
    assert_eq!(
        cursor.pos, pos_before,
        "SPEC-HAL-107 §7.4: move_down at bottom must be noop. Expected {}, got {}",
        pos_before, cursor.pos
    );

    // preferred_x must not change
    assert_eq!(
        cursor.preferred_x, px_before,
        "SPEC-HAL-107 §7.4: preferred_x must not change at bottom boundary"
    );
}

#[test]
fn cursor_up_cannot_cross_past_empty_top_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: Empty lines at top do not allow crossing past document start.

    // Text: "\n\nABCDE" (two empty lines, then content)
    // Line 0: \n(0) - empty
    // Line 1: \n(1) - empty
    // Line 2: A(2) B(3) C(4) D(5) E(6) sentinel(7)
    let text = "\n\nABCDE";
    let boundaries = make_multiline_boundaries(text);

    // Start at empty line 1 (byte 1)
    let mut cursor = TextCursor::at(1);
    cursor.preferred_x = Some(30.0);
    let mut selection = Selection::at(1);

    // Move up to line 0 (empty)
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0, "Should land on line 0 (byte 0)");

    // Move up again - should stay at line 0 start (noop or clamp)
    let pos_before = cursor.pos;
    move_up(&mut cursor, &mut selection, &boundaries);

    // Cannot cross past document start
    assert_eq!(
        cursor.pos, 0,
        "SPEC-HAL-107 §7.4: Cannot cross past document start. Expected 0, got {}",
        cursor.pos
    );
}

#[test]
fn cursor_down_cannot_cross_past_empty_bottom_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: Empty lines at bottom do not allow crossing past document end.

    // Text: "ABCDE\n\n" (content, then two empty lines)
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty
    // Line 2: sentinel(7) - empty (just sentinel)
    let text = "ABCDE\n\n";
    let boundaries = make_multiline_boundaries(text);

    // Start at empty line 1 (byte 6)
    let mut cursor = TextCursor::at(6);
    cursor.preferred_x = Some(30.0);
    let mut selection = Selection::at(6);

    // Move down - should stay at or near end
    move_down(&mut cursor, &mut selection, &boundaries);
    let pos_after_first = cursor.pos;

    // Move down again - should not move further
    move_down(&mut cursor, &mut selection, &boundaries);

    // Cannot cross past document end
    assert!(
        cursor.pos <= boundaries.last().unwrap().byte_start,
        "SPEC-HAL-107 §7.4: Cannot cross past document end. pos={}, max={}",
        cursor.pos,
        boundaries.last().unwrap().byte_start
    );
}

#[test]
fn cursor_repeated_up_at_top_is_stable_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: Repeated move_up at top edge is stable (no drift, no state mutation).

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at line 0
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(2);

    // First move_up clamps to start
    move_up(&mut cursor, &mut selection, &boundaries);
    let pos_after_first = cursor.pos;
    let px_after_first = cursor.preferred_x;

    // Repeated move_up should not change anything
    for i in 0..5 {
        move_up(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            cursor.pos, pos_after_first,
            "SPEC-HAL-107 §7.4: Repeated up at top must be stable. Iteration {}, expected {}, got {}",
            i, pos_after_first, cursor.pos
        );
        assert_eq!(
            cursor.preferred_x, px_after_first,
            "SPEC-HAL-107 §7.4: preferred_x must remain stable at top. Iteration {}",
            i
        );
    }
}

#[test]
fn cursor_repeated_down_at_bottom_is_stable_SPEC_107_7_4() {
    // SPEC-HAL-107 §7.4: Repeated move_down at bottom edge is stable (no drift, no state mutation).

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at line 1 (last line)
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(8);

    // First move_down should be noop (already on last line)
    move_down(&mut cursor, &mut selection, &boundaries);
    let pos_after_first = cursor.pos;
    let px_after_first = cursor.preferred_x;

    // Repeated move_down should not change anything
    for i in 0..5 {
        move_down(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            cursor.pos, pos_after_first,
            "SPEC-HAL-107 §7.4: Repeated down at bottom must be stable. Iteration {}, expected {}, got {}",
            i, pos_after_first, cursor.pos
        );
        assert_eq!(
            cursor.preferred_x, px_after_first,
            "SPEC-HAL-107 §7.4: preferred_x must remain stable at bottom. Iteration {}",
            i
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §7.5 — Selection Semantics During Vertical Movement
// ============================================================================

// ---------------------------------------------------------------------------
// §7.5.1: No modifier (plain Up/Down) — Selection collapses
// ---------------------------------------------------------------------------

#[test]
fn plain_up_collapses_selection_clears_anchor_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Plain Up collapses selection, anchor cleared, preferred_x preserved.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start with non-empty selection on line 2
    // Selection: from byte 8 ('H') to byte 15 ('N')
    let mut cursor = TextCursor::at(15);
    cursor.preferred_x = Some(30.0);
    let mut selection = Selection::new();
    selection.anchor = 8;
    selection.head = 15;

    assert!(!selection.is_empty(), "Precondition: selection non-empty");

    // Plain move_up should collapse selection
    move_up(&mut cursor, &mut selection, &boundaries);

    // Selection must be collapsed (anchor == head == cursor.pos)
    assert!(
        selection.is_empty(),
        "SPEC-HAL-107 §7.5: Plain Up must collapse selection. anchor={}, head={}",
        selection.anchor,
        selection.head
    );
    assert_eq!(
        selection.anchor, cursor.pos,
        "SPEC-HAL-107 §7.5: anchor must equal cursor.pos after collapse"
    );
}

#[test]
fn plain_down_collapses_selection_clears_anchor_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Plain Down collapses selection, anchor cleared, preferred_x preserved.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start with non-empty selection spanning lines 0-1
    // Selection: from byte 2 ('C') to byte 8 ('H')
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 8;

    assert!(!selection.is_empty(), "Precondition: selection non-empty");

    // Plain move_down should collapse selection
    move_down(&mut cursor, &mut selection, &boundaries);

    // Selection must be collapsed
    assert!(
        selection.is_empty(),
        "SPEC-HAL-107 §7.5: Plain Down must collapse selection. anchor={}, head={}",
        selection.anchor,
        selection.head
    );
    assert_eq!(
        selection.anchor, cursor.pos,
        "SPEC-HAL-107 §7.5: anchor must equal cursor.pos after collapse"
    );
}

#[test]
fn plain_vertical_movement_preserves_preferred_x_after_collapse_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: After selection collapse, preferred_x is preserved (not reset).

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start with selection and established preferred_x
    let mut cursor = TextCursor::at(15);
    cursor.preferred_x = Some(30.0); // established from prior vertical move
    let mut selection = Selection::new();
    selection.anchor = 8;
    selection.head = 15;

    let px_before = cursor.preferred_x;

    // Plain move_up collapses selection
    move_up(&mut cursor, &mut selection, &boundaries);

    // preferred_x must be preserved through the collapse
    assert_eq!(
        cursor.preferred_x, px_before,
        "SPEC-HAL-107 §7.5: preferred_x must be preserved after collapse. Was {:?}, now {:?}",
        px_before, cursor.preferred_x
    );
}

// ---------------------------------------------------------------------------
// §7.5.2: With Shift (Shift+Up / Shift+Down) — Selection extends
// ---------------------------------------------------------------------------

#[test]
fn shift_up_extends_selection_anchor_fixed_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Up extends selection upward, anchor remains at original position.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'N' on line 2 (byte 15, x=30.0), no selection
    let mut cursor = TextCursor::at(15);
    cursor.preferred_x = None;
    let mut selection = Selection::at(15);

    assert!(selection.is_empty(), "Precondition: no selection");

    // Shift+Up (move_up_select)
    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor must be fixed at original position (15)
    assert_eq!(
        selection.anchor, 15,
        "SPEC-HAL-107 §7.5: Shift+Up must fix anchor at original position (15), got {}",
        selection.anchor
    );

    // Head follows cursor
    assert_eq!(
        selection.head, cursor.pos,
        "SPEC-HAL-107 §7.5: head must follow cursor"
    );

    // Selection is non-empty
    assert!(
        !selection.is_empty(),
        "SPEC-HAL-107 §7.5: Selection must be non-empty after Shift+Up"
    );

    // Another Shift+Up
    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor must STILL be 15
    assert_eq!(
        selection.anchor, 15,
        "SPEC-HAL-107 §7.5: Anchor must remain fixed after second Shift+Up, got {}",
        selection.anchor
    );
}

#[test]
fn shift_down_extends_selection_anchor_fixed_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Down extends selection downward, anchor remains at original position.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2, x=20.0), no selection
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = None;
    let mut selection = Selection::at(2);

    assert!(selection.is_empty(), "Precondition: no selection");

    // Shift+Down (move_down_select)
    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor must be fixed at original position (2)
    assert_eq!(
        selection.anchor, 2,
        "SPEC-HAL-107 §7.5: Shift+Down must fix anchor at original position (2), got {}",
        selection.anchor
    );

    // Head follows cursor
    assert_eq!(
        selection.head, cursor.pos,
        "SPEC-HAL-107 §7.5: head must follow cursor"
    );

    // Selection is non-empty
    assert!(
        !selection.is_empty(),
        "SPEC-HAL-107 §7.5: Selection must be non-empty after Shift+Down"
    );

    // Another Shift+Down
    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor must STILL be 2
    assert_eq!(
        selection.anchor, 2,
        "SPEC-HAL-107 §7.5: Anchor must remain fixed after second Shift+Down, got {}",
        selection.anchor
    );
}

#[test]
fn shift_vertical_preserves_preferred_x_during_selection_growth_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: preferred_x preserved across selection growth.

    // Lines of varying width:
    // Line 0: ABCDEFGH\n  (8 chars, x: 0-70)
    // Line 1: IJ\n        (2 chars, x: 0-10)
    // Line 2: KLMNOPQR    (8 chars, x: 0-70)
    let text = "ABCDEFGH\nIJ\nKLMNOPQR";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'F' on line 0 (byte 5, x=50.0)
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = None;
    let mut selection = Selection::at(5);

    // Shift+Down to short line 1 - clamps, but preferred_x should be captured at 50.0
    move_down_select(&mut cursor, &mut selection, &boundaries);
    let px_first = cursor.preferred_x.unwrap();
    assert_eq!(px_first, 50.0, "preferred_x should be captured at 50.0");

    // Shift+Down to line 2 - preferred_x should be preserved
    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px_first,
        "SPEC-HAL-107 §7.5: preferred_x must be preserved during selection growth"
    );

    // Cursor should land at x=50 on line 2 ('N' at byte 15)
    // Line 2: K(12) L(13) M(14) N(15) O(16) P(17) Q(18) R(19)
    assert_eq!(
        cursor.pos, 17,
        "Should land at 'P' (byte 17, x=50) on line 2, got {}",
        cursor.pos
    );
}

#[test]
fn shift_direction_reversal_preserves_anchor_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Up then Shift+Down reversal preserves anchor, head moves.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 1 (byte 8, x=20.0)
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = None;
    let mut selection = Selection::at(8);

    // Shift+Up (select upward)
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 8, "Anchor fixed at 8");

    // Now Shift+Down (reverse direction)
    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor must STILL be 8 (the original starting point)
    assert_eq!(
        selection.anchor, 8,
        "SPEC-HAL-107 §7.5: Direction reversal must preserve anchor at original position (8), got {}",
        selection.anchor
    );

    // Head follows cursor
    assert_eq!(
        selection.head, cursor.pos,
        "head must follow cursor after reversal"
    );
}

#[test]
fn shift_direction_reversal_can_shrink_selection_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Reversing direction shrinks selection correctly.

    let text = "ABCDE\nFGHIJ\nKLMNO\nPQRST";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 1 (byte 8, x=20.0)
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = None;
    let mut selection = Selection::at(8);

    // Shift+Down twice (extend selection downward)
    move_down_select(&mut cursor, &mut selection, &boundaries);
    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Now selection spans from anchor=8 to head (somewhere on line 3)
    let head_after_expand = selection.head;
    assert!(
        head_after_expand > selection.anchor,
        "Selection expanded downward"
    );

    // Reverse: Shift+Up (shrink selection)
    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor still 8
    assert_eq!(selection.anchor, 8, "Anchor must remain fixed at 8");

    // Head has moved up (shrunk)
    assert!(
        selection.head < head_after_expand,
        "SPEC-HAL-107 §7.5: Reversing must shrink selection. head was {}, now {}",
        head_after_expand,
        selection.head
    );
}

// ---------------------------------------------------------------------------
// §7.5.3: Edge interaction with Shift
// ---------------------------------------------------------------------------

#[test]
fn shift_up_at_top_does_not_expand_past_start_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Up at top line does NOT expand selection past document start.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2), select upward
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = None;
    let mut selection = Selection::at(2);

    // Shift+Up at top line
    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should be at original (2), head at line start (0)
    assert_eq!(selection.anchor, 2, "Anchor fixed at 2");

    // Head should be at line start (0), not negative or invalid
    assert_eq!(
        selection.head, 0,
        "SPEC-HAL-107 §7.5: Shift+Up at top must clamp head to document start (0), got {}",
        selection.head
    );

    // Cursor pos should also be at 0
    assert_eq!(cursor.pos, 0, "cursor.pos should be at document start");
}

#[test]
fn shift_down_at_bottom_does_not_expand_past_end_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Down at bottom line does NOT expand selection past document end.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);
    let doc_end = boundaries.last().unwrap().byte_start;

    // Start at 'H' on line 1 (byte 8), select downward
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = None;
    let mut selection = Selection::at(8);

    // Shift+Down at bottom line
    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should be at original (8)
    assert_eq!(selection.anchor, 8, "Anchor fixed at 8");

    // Head should not exceed document end
    assert!(
        selection.head <= doc_end,
        "SPEC-HAL-107 §7.5: Shift+Down at bottom must not expand past document end. head={}, max={}",
        selection.head,
        doc_end
    );

    // Cursor should not exceed document end
    assert!(
        cursor.pos <= doc_end,
        "cursor.pos must not exceed document end"
    );
}

#[test]
fn shift_repeated_up_at_top_is_stable_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Repeated Shift+Up at top is stable (selection does not drift or mutate).

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2)
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(2);

    // First Shift+Up
    move_up_select(&mut cursor, &mut selection, &boundaries);
    let anchor_after = selection.anchor;
    let head_after = selection.head;
    let pos_after = cursor.pos;

    // Repeated Shift+Up at top edge should be stable
    for i in 0..5 {
        move_up_select(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            selection.anchor, anchor_after,
            "SPEC-HAL-107 §7.5: Anchor must remain stable at top. Iteration {}, expected {}, got {}",
            i, anchor_after, selection.anchor
        );
        assert_eq!(
            selection.head, head_after,
            "SPEC-HAL-107 §7.5: Head must remain stable at top. Iteration {}, expected {}, got {}",
            i, head_after, selection.head
        );
        assert_eq!(
            cursor.pos, pos_after,
            "SPEC-HAL-107 §7.5: cursor.pos must remain stable at top. Iteration {}",
            i
        );
    }
}

#[test]
fn shift_repeated_down_at_bottom_is_stable_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Repeated Shift+Down at bottom is stable.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 1 (byte 8)
    let mut cursor = TextCursor::at(8);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(8);

    // First Shift+Down
    move_down_select(&mut cursor, &mut selection, &boundaries);
    let anchor_after = selection.anchor;
    let head_after = selection.head;
    let pos_after = cursor.pos;

    // Repeated Shift+Down at bottom edge should be stable
    for i in 0..5 {
        move_down_select(&mut cursor, &mut selection, &boundaries);
        assert_eq!(
            selection.anchor, anchor_after,
            "SPEC-HAL-107 §7.5: Anchor must remain stable at bottom. Iteration {}, expected {}, got {}",
            i, anchor_after, selection.anchor
        );
        assert_eq!(
            selection.head, head_after,
            "SPEC-HAL-107 §7.5: Head must remain stable at bottom. Iteration {}, expected {}, got {}",
            i, head_after, selection.head
        );
        assert_eq!(
            cursor.pos, pos_after,
            "SPEC-HAL-107 §7.5: cursor.pos must remain stable at bottom. Iteration {}",
            i
        );
    }
}

// ---------------------------------------------------------------------------
// §7.5.4: Empty-line interaction with Shift
// ---------------------------------------------------------------------------

#[test]
fn shift_up_through_empty_line_deterministic_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Up through empty line is deterministic, empty line participates.

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'H' on line 2 (byte 9, x=20.0)
    let mut cursor = TextCursor::at(9);
    cursor.preferred_x = None;
    let mut selection = Selection::at(9);

    // First Shift+Up goes to empty line 1
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 9, "Anchor fixed at 9");
    assert_eq!(cursor.pos, 6, "Should land on empty line 1 (byte 6)");
    assert_eq!(selection.head, 6, "head follows cursor to empty line");

    // Second Shift+Up goes to line 0
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 9, "Anchor still at 9");
    // Should land at x=20 on line 0 = 'C' at byte 2
    assert_eq!(cursor.pos, 2, "Should land at 'C' (byte 2) on line 0");
    assert_eq!(selection.head, 2, "head follows cursor");

    // Selection spans from 9 to 2 (via empty line)
    assert!(!selection.is_empty(), "Selection must be non-empty");
}

#[test]
fn shift_down_through_empty_line_deterministic_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: Shift+Down through empty line is deterministic, empty line participates.

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2, x=20.0)
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = None;
    let mut selection = Selection::at(2);

    // First Shift+Down goes to empty line 1
    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 2, "Anchor fixed at 2");
    assert_eq!(cursor.pos, 6, "Should land on empty line 1 (byte 6)");
    assert_eq!(selection.head, 6, "head follows cursor to empty line");

    // Second Shift+Down goes to line 2
    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 2, "Anchor still at 2");
    // Should land at x=20 on line 2 = 'H' at byte 9
    assert_eq!(cursor.pos, 9, "Should land at 'H' (byte 9) on line 2");
    assert_eq!(selection.head, 9, "head follows cursor");

    // Selection spans from 2 to 9 (via empty line)
    assert!(!selection.is_empty(), "Selection must be non-empty");
}

#[test]
fn shift_through_empty_line_preserves_preferred_x_SPEC_107_7_5() {
    // SPEC-HAL-107 §7.5: preferred_x unaffected by empty line during selection extension.

    // Lines:
    // Line 0: ABCDEFGH\n  (8 chars, x: 0-70)
    // Line 1: \n          (empty)
    // Line 2: IJKLMNOP    (8 chars, x: 0-70)
    let text = "ABCDEFGH\n\nIJKLMNOP";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'F' on line 0 (byte 5, x=50.0)
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = None;
    let mut selection = Selection::at(5);

    // Shift+Down to empty line 1
    move_down_select(&mut cursor, &mut selection, &boundaries);
    let px_after_empty = cursor.preferred_x.unwrap();
    assert_eq!(
        px_after_empty, 50.0,
        "preferred_x should be 50.0 after entering empty line"
    );

    // Shift+Down to line 2 - preferred_x should still be 50.0
    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.preferred_x.unwrap(),
        px_after_empty,
        "SPEC-HAL-107 §7.5: preferred_x must be preserved through empty line during selection"
    );

    // Should land at x=50 on line 2 = 'N' at byte 15
    // Line 2: I(10) J(11) K(12) L(13) M(14) N(15) O(16) P(17)
    assert_eq!(
        cursor.pos, 15,
        "Should land at 'N' (byte 15, x=50) on line 2, got {}",
        cursor.pos
    );
}
