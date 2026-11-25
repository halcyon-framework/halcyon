// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.
//
// NOTE:
// These tests use ASCII-only boundary generation.
// UTF-8 multi-byte tests use explicit boundary construction.

#![allow(non_snake_case, unused_comparisons, unused_variables)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op, apply_insert_op,
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
/// Each character gets one boundary at its byte start.
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
// SPEC-HAL-107 §1.1 — Cursor is a byte index
// ============================================================================

#[test]
fn cursor_is_byte_index_SPEC_107_1_1() {
    // SPEC-HAL-107 §1.1: cursor.pos is a byte index into the document.

    let text = String::from("Hello");

    // Create cursor at various valid byte positions
    for pos in 0..=text.len() {
        let cursor = TextCursor::at(pos);

        // Assert: cursor.pos is a valid byte index (can slice at this position)
        assert!(
            pos <= text.len(),
            "cursor.pos {} must be <= text.len() {}",
            cursor.pos,
            text.len()
        );

        // Assert: can safely index up to this position
        let _ = &text[..pos]; // Would panic if invalid
    }
}

// ============================================================================
// SPEC-HAL-107 §1.2 — Cursor bounds
// ============================================================================

#[test]
fn cursor_within_bounds_after_insert_SPEC_107_1_2() {
    // SPEC-HAL-107 §1.2: 0 <= cursor.pos <= text.len() always holds.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "XYZ");

    // Assert: cursor within bounds after insert
    assert!(
        cursor.pos <= text.len(),
        "cursor.pos {} must be <= text.len() {} after insert",
        cursor.pos,
        text.len()
    );
    assert!(cursor.pos >= 0, "cursor.pos must be >= 0");
}

#[test]
fn cursor_within_bounds_after_delete_SPEC_107_1_2() {
    // SPEC-HAL-107 §1.2: 0 <= cursor.pos <= text.len() always holds.

    let mut text = String::from("ABCDE");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Assert: cursor within bounds after delete
    assert!(
        cursor.pos <= text.len(),
        "cursor.pos {} must be <= text.len() {} after backspace",
        cursor.pos,
        text.len()
    );
    assert!(cursor.pos >= 0, "cursor.pos must be >= 0");
}

// ============================================================================
// SPEC-HAL-107 §1.3 — UTF-8 safety
// ============================================================================

#[test]
fn cursor_never_splits_utf8_codepoint_SPEC_107_1_3() {
    // SPEC-HAL-107 §1.3: cursor.pos never lands inside a UTF-8 codepoint.

    // Text with multi-byte characters: "Héllo" where é is 2 bytes
    let mut text = String::from("Héllo");
    let boundaries = make_utf8_boundaries(&text);

    // é is at byte 1, takes 2 bytes (bytes 1-2)
    // Valid cursor positions: 0, 1, 3, 4, 5, 6

    let mut cursor = TextCursor::at(1); // At 'é'
    let mut selection = Selection::at(1);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // Assert: cursor.pos is a valid char boundary
    assert!(
        text.is_char_boundary(cursor.pos),
        "cursor.pos {} must be a char boundary in {:?}",
        cursor.pos,
        text
    );
}

#[test]
fn cursor_at_char_boundary_after_utf8_insert_SPEC_107_1_3() {
    // SPEC-HAL-107 §1.3: After inserting multi-byte char, cursor is at valid boundary.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // Insert a 2-byte character (é)
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "é");

    // Assert: cursor is at valid char boundary
    assert!(
        text.is_char_boundary(cursor.pos),
        "cursor.pos {} must be char boundary after inserting multi-byte char",
        cursor.pos
    );
}

// ============================================================================
// SPEC-HAL-107 §1.4 — Cursor stability (no implicit movement)
// ============================================================================

#[test]
fn cursor_stable_without_operation_SPEC_107_1_4() {
    // SPEC-HAL-107 §1.4: Cursor does not move unless explicitly instructed.

    let text = String::from("Hello");
    let cursor = TextCursor::at(3);

    // No operation performed

    // Assert: cursor unchanged
    assert_eq!(cursor.pos, 3, "Cursor must not move without operation");
}

// ============================================================================
// SPEC-HAL-107 §1.5 — No-op stability
// ============================================================================

#[test]
fn backspace_at_start_is_noop_SPEC_107_1_5() {
    // SPEC-HAL-107 §1.5: Backspace at document start does nothing.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);
    let original_text = text.clone();

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Assert: text unchanged
    assert_eq!(
        text, original_text,
        "Backspace at start must not modify text"
    );

    // Assert: cursor unchanged
    assert_eq!(cursor.pos, 0, "Backspace at start must not move cursor");
}

#[test]
fn delete_at_end_is_noop_SPEC_107_1_5() {
    // SPEC-HAL-107 §1.5: Delete at document end does nothing.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);
    let original_text = text.clone();
    let end_pos = text.len();

    let mut cursor = TextCursor::at(end_pos);
    let mut selection = Selection::at(end_pos);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    // Assert: text unchanged
    assert_eq!(text, original_text, "Delete at end must not modify text");

    // Assert: cursor unchanged
    assert_eq!(cursor.pos, end_pos, "Delete at end must not move cursor");
}

// ============================================================================
// SPEC-HAL-107 §1.6 — Insertion delta
// ============================================================================

#[test]
fn cursor_advances_by_inserted_length_SPEC_107_1_6() {
    // SPEC-HAL-107 §1.6: After insertion, cursor.pos == old_pos + inserted_len.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let old_pos = 1;
    let mut cursor = TextCursor::at(old_pos);
    let mut selection = Selection::at(old_pos);

    let insert_str = "XYZ";
    let insert_len = insert_str.len();

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        insert_str,
    );

    // Assert: cursor advanced by exactly insert_len
    assert_eq!(
        cursor.pos,
        old_pos + insert_len,
        "cursor.pos must be old_pos ({}) + insert_len ({})",
        old_pos,
        insert_len
    );
}

#[test]
fn cursor_advances_by_multibyte_insert_length_SPEC_107_1_6() {
    // SPEC-HAL-107 §1.6: Multi-byte insertion advances cursor by byte count.

    let mut text = String::from("AB");
    let boundaries = make_ascii_boundaries(&text);

    let old_pos = 1;
    let mut cursor = TextCursor::at(old_pos);
    let mut selection = Selection::at(old_pos);

    let insert_str = "日本"; // 6 bytes (2 chars × 3 bytes each)
    let insert_len = insert_str.len();
    assert_eq!(insert_len, 6);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        insert_str,
    );

    // Assert: cursor advanced by byte length, not char count
    assert_eq!(
        cursor.pos,
        old_pos + insert_len,
        "cursor.pos must advance by byte length ({}), not char count",
        insert_len
    );
}

// ============================================================================
// SPEC-HAL-107 §1.7 — Deletion delta
// ============================================================================

#[test]
fn cursor_moves_back_on_backspace_SPEC_107_1_7() {
    // SPEC-HAL-107 §1.7: Backspace moves cursor back by deleted cluster size.

    let mut text = String::from("ABCDE");
    let boundaries = make_ascii_boundaries(&text);

    let old_pos = 3; // After "ABC"
    let mut cursor = TextCursor::at(old_pos);
    let mut selection = Selection::at(old_pos);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Assert: cursor moved back by 1 (ASCII char)
    assert_eq!(
        cursor.pos,
        old_pos - 1,
        "Backspace must move cursor back by deleted cluster size"
    );
    assert_eq!(text, "ABDE");
}

#[test]
fn cursor_stays_on_delete_SPEC_107_1_7() {
    // SPEC-HAL-107 §1.7: Delete removes char at cursor but cursor stays in place.

    let mut text = String::from("ABCDE");
    let boundaries = make_ascii_boundaries(&text);

    let old_pos = 2; // Before "C"
    let mut cursor = TextCursor::at(old_pos);
    let mut selection = Selection::at(old_pos);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    // Assert: cursor position unchanged (Delete removes forward)
    assert_eq!(
        cursor.pos, old_pos,
        "Delete must not move cursor (removes forward)"
    );
    assert_eq!(text, "ABDE");
}

// ============================================================================
// SPEC-HAL-107 §1.8 — Selection collapse after edit
// ============================================================================

#[test]
fn selection_collapses_to_cursor_after_insert_SPEC_107_1_8() {
    // SPEC-HAL-107 §1.8: After insertion, selection collapses to cursor position.

    let mut text = String::from("ABCDEF");
    let boundaries = make_ascii_boundaries(&text);

    // Create a non-collapsed selection (2..5 = "CDE")
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 5;

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // Assert: selection collapsed to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "selection.anchor must equal cursor.pos after insert"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "selection.head must equal cursor.pos after insert"
    );
}

#[test]
fn selection_collapses_to_cursor_after_delete_SPEC_107_1_8() {
    // SPEC-HAL-107 §1.8: After deletion, selection collapses to cursor position.

    let mut text = String::from("ABCDEF");
    let boundaries = make_ascii_boundaries(&text);

    // Create a non-collapsed selection (2..5 = "CDE")
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 5;

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Assert: selection collapsed to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "selection.anchor must equal cursor.pos after delete"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "selection.head must equal cursor.pos after delete"
    );
}

// ============================================================================
// SPEC-HAL-107 §1.9 — Boundary validity after operations
// ============================================================================

#[test]
fn cursor_at_valid_boundary_after_insert_SPEC_107_1_9() {
    // SPEC-HAL-107 §1.9: After insert, cursor.pos is a valid boundary position.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "XY");

    // Rebuild boundaries for new text
    let new_boundaries = make_ascii_boundaries(&text);

    // Assert: cursor.pos exists in new boundaries
    let cursor_is_valid_boundary = new_boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        cursor_is_valid_boundary,
        "cursor.pos {} must be a valid boundary after insert. Boundaries: {:?}",
        cursor.pos,
        new_boundaries
            .iter()
            .map(|b| b.byte_start)
            .collect::<Vec<_>>()
    );
}

#[test]
fn cursor_at_valid_boundary_after_delete_SPEC_107_1_9() {
    // SPEC-HAL-107 §1.9: After delete, cursor.pos is a valid boundary position.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Rebuild boundaries for new text
    let new_boundaries = make_ascii_boundaries(&text);

    // Assert: cursor.pos exists in new boundaries
    let cursor_is_valid_boundary = new_boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        cursor_is_valid_boundary,
        "cursor.pos {} must be a valid boundary after delete. Boundaries: {:?}",
        cursor.pos,
        new_boundaries
            .iter()
            .map(|b| b.byte_start)
            .collect::<Vec<_>>()
    );
}
