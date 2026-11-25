// SPEC-HAL-107-V §13
// Any failing test indicates a violation of SPEC-HAL-107 §13 Deletion Modifier Semantics.
// Demo behavior is non-authoritative.
//
// §13 Summary:
// - Shift does NOT change deletion semantics
// - Ctrl/Opt modifies granularity only (word vs cluster)
// - Ctrl/Opt + Shift has no additional semantics beyond Ctrl/Opt

#![allow(non_snake_case)]
// - Range deletion is selection-driven only

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, TextEditOp, apply_delete_word_back,
    apply_delete_word_forward, apply_edit_op,
};

// ============================================================================
// Helper: Create boundaries for ASCII text
// ============================================================================

fn make_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
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

// ============================================================================
// §13.1 — Shift Is Semantically Inert for Deletion
// ============================================================================

#[test]
fn backspace_is_idempotent_shift_inert_SPEC_107_13_1() {
    // SPEC-HAL-107 §13.1: Shift+Backspace behaves identically to Backspace.
    // Substrate has no Shift flag; calling apply_edit_op twice with same state must yield same result.

    let original = "Hello";
    let boundaries = make_ascii_boundaries(original);

    // Run 1: Backspace
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(3); // After "Hel"
    let mut selection1 = Selection::at(3);
    apply_edit_op(
        &mut text1,
        &mut selection1,
        &mut cursor1,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Run 2: Backspace again (simulating Shift+Backspace - same call)
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(3);
    let mut selection2 = Selection::at(3);
    apply_edit_op(
        &mut text2,
        &mut selection2,
        &mut cursor2,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(
        text1, text2,
        "Backspace must be deterministic (Shift-inert)"
    );
    assert_eq!(cursor1.pos, cursor2.pos, "Cursor position must match");
    assert_eq!(text1, "Helo", "Expected 'l' deleted");
    assert_eq!(cursor1.pos, 2, "Cursor should be at byte 2");
}

#[test]
fn delete_is_idempotent_shift_inert_SPEC_107_13_1() {
    // SPEC-HAL-107 §13.1: Shift+Delete behaves identically to Delete.

    let original = "Hello";
    let boundaries = make_ascii_boundaries(original);

    // Run 1: Delete
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(2); // After "He"
    let mut selection1 = Selection::at(2);
    apply_edit_op(
        &mut text1,
        &mut selection1,
        &mut cursor1,
        &boundaries,
        TextEditOp::Delete,
    );

    // Run 2: Delete again (simulating Shift+Delete)
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(2);
    let mut selection2 = Selection::at(2);
    apply_edit_op(
        &mut text2,
        &mut selection2,
        &mut cursor2,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text1, text2, "Delete must be deterministic (Shift-inert)");
    assert_eq!(cursor1.pos, cursor2.pos, "Cursor position must match");
    assert_eq!(text1, "Helo", "Expected 'l' deleted");
    assert_eq!(cursor1.pos, 2, "Cursor should stay at byte 2");
}

#[test]
fn ctrl_backspace_is_idempotent_shift_inert_SPEC_107_13_1() {
    // SPEC-HAL-107 §13.3: Ctrl+Shift+Backspace behaves identically to Ctrl+Backspace.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    // Run 1: Ctrl+Backspace
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(11); // End of "hello world"
    let mut selection1 = Selection::at(11);
    apply_delete_word_back(&mut text1, &mut selection1, &mut cursor1, &boundaries);

    // Run 2: Ctrl+Backspace again (simulating Ctrl+Shift+Backspace)
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(11);
    let mut selection2 = Selection::at(11);
    apply_delete_word_back(&mut text2, &mut selection2, &mut cursor2, &boundaries);

    assert_eq!(
        text1, text2,
        "Ctrl+Backspace must be deterministic (Shift-inert)"
    );
    assert_eq!(cursor1.pos, cursor2.pos, "Cursor position must match");
}

#[test]
fn ctrl_delete_is_idempotent_shift_inert_SPEC_107_13_1() {
    // SPEC-HAL-107 §13.3: Ctrl+Shift+Delete behaves identically to Ctrl+Delete.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    // Run 1: Ctrl+Delete
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(0); // Start
    let mut selection1 = Selection::at(0);
    apply_delete_word_forward(&mut text1, &mut selection1, &mut cursor1, &boundaries);

    // Run 2: Ctrl+Delete again (simulating Ctrl+Shift+Delete)
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(0);
    let mut selection2 = Selection::at(0);
    apply_delete_word_forward(&mut text2, &mut selection2, &mut cursor2, &boundaries);

    assert_eq!(
        text1, text2,
        "Ctrl+Delete must be deterministic (Shift-inert)"
    );
    assert_eq!(cursor1.pos, cursor2.pos, "Cursor position must match");
}

// ============================================================================
// §13.2 — Ctrl/Opt Modifies Granularity Only
// ============================================================================

#[test]
fn backspace_deletes_one_cluster_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Backspace deletes one cluster.

    let original = "Hello";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(3); // After "Hel"
    let mut selection = Selection::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(
        text, "Helo",
        "Backspace should delete exactly one cluster ('l')"
    );
    assert_eq!(cursor.pos, 2, "Cursor should move back one cluster");
}

#[test]
fn ctrl_backspace_deletes_word_unit_backward_middle_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Ctrl+Backspace deletes one word unit backward.
    // Test at middle of word.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(8); // After "hello wo"
    let mut selection = Selection::at(8);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    // Should delete "wo" back to word start (at space or word boundary)
    assert!(
        text.len() < original.len(),
        "Word deletion must remove characters"
    );
    assert!(cursor.pos < 8, "Cursor must move backward");
}

#[test]
fn ctrl_backspace_deletes_word_unit_backward_at_boundary_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Ctrl+Backspace at word boundary.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(6); // After "hello " (at start of "world")
    let mut selection = Selection::at(6);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    // Should delete backward (space and/or previous word)
    assert!(
        text.len() < original.len(),
        "Word deletion must remove characters"
    );
}

#[test]
fn delete_deletes_one_cluster_forward_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Delete deletes one cluster forward.

    let original = "Hello";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(2); // After "He"
    let mut selection = Selection::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(
        text, "Helo",
        "Delete should delete exactly one cluster ('l')"
    );
    assert_eq!(cursor.pos, 2, "Cursor should stay at same position");
}

#[test]
fn ctrl_delete_deletes_word_unit_forward_middle_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Ctrl+Delete deletes one word unit forward.
    // Test at middle of word.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(2); // After "he"
    let mut selection = Selection::at(2);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    // Should delete "llo" forward to word end
    assert!(
        text.len() < original.len(),
        "Word deletion must remove characters"
    );
    assert_eq!(
        cursor.pos, 2,
        "Cursor stays at same position for forward delete"
    );
}

#[test]
fn ctrl_delete_deletes_word_unit_forward_at_boundary_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Ctrl+Delete at word boundary.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(5); // After "hello" (before space)
    let mut selection = Selection::at(5);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    // Should delete forward (space and/or next word)
    assert!(
        text.len() < original.len(),
        "Word deletion must remove characters"
    );
}

#[test]
fn backspace_at_line_boundary_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Backspace at line boundary deletes newline.

    let original = "hello\nworld";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(6); // Start of "world" (after \n)
    let mut selection = Selection::at(6);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(
        text, "helloworld",
        "Backspace at line start should delete newline"
    );
    assert_eq!(cursor.pos, 5, "Cursor should be at end of 'hello'");
}

#[test]
fn delete_at_line_boundary_SPEC_107_13_2() {
    // SPEC-HAL-107 §13.2: Delete at line end deletes newline.

    let original = "hello\nworld";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(5); // At newline position
    let mut selection = Selection::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "helloworld", "Delete at newline should delete it");
    assert_eq!(cursor.pos, 5, "Cursor should stay at same position");
}

// ============================================================================
// §13.4 — Selection Always Dominates
// ============================================================================

#[test]
fn backspace_with_selection_deletes_selection_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: Backspace with selection deletes selection, ignores modifiers.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection { anchor: 2, head: 8 }; // "llo wo" selected

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "herld", "Selection must be deleted");
    assert_eq!(cursor.pos, 2, "Cursor at selection start");
    assert_eq!(selection.anchor, 2, "Selection collapsed");
    assert_eq!(selection.head, 2, "Selection collapsed");
}

#[test]
fn delete_with_selection_deletes_selection_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: Delete with selection deletes selection, ignores modifiers.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection { anchor: 2, head: 8 }; // "llo wo" selected

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "herld", "Selection must be deleted");
    assert_eq!(cursor.pos, 2, "Cursor at selection start");
    assert_eq!(selection.anchor, 2, "Selection collapsed");
    assert_eq!(selection.head, 2, "Selection collapsed");
}

#[test]
fn ctrl_backspace_with_selection_deletes_selection_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: Ctrl+Backspace with selection deletes selection (not word).

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection { anchor: 2, head: 8 }; // "llo wo" selected

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "herld", "Selection must be deleted, not word");
    assert_eq!(cursor.pos, 2, "Cursor at selection start");
}

#[test]
fn ctrl_delete_with_selection_deletes_selection_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: Ctrl+Delete with selection deletes selection (not word).

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    let mut text = original.to_string();
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection { anchor: 2, head: 8 }; // "llo wo" selected

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "herld", "Selection must be deleted, not word");
    assert_eq!(cursor.pos, 2, "Cursor at selection start");
}

#[test]
fn all_deletion_paths_identical_with_selection_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: All deletion functions produce identical results with selection.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);
    let sel_anchor = 3;
    let sel_head = 9;

    // Backspace path
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(sel_head);
    let mut selection1 = Selection {
        anchor: sel_anchor,
        head: sel_head,
    };
    apply_edit_op(
        &mut text1,
        &mut selection1,
        &mut cursor1,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Delete path
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(sel_head);
    let mut selection2 = Selection {
        anchor: sel_anchor,
        head: sel_head,
    };
    apply_edit_op(
        &mut text2,
        &mut selection2,
        &mut cursor2,
        &boundaries,
        TextEditOp::Delete,
    );

    // Ctrl+Backspace path
    let mut text3 = original.to_string();
    let mut cursor3 = TextCursor::at(sel_head);
    let mut selection3 = Selection {
        anchor: sel_anchor,
        head: sel_head,
    };
    apply_delete_word_back(&mut text3, &mut selection3, &mut cursor3, &boundaries);

    // Ctrl+Delete path
    let mut text4 = original.to_string();
    let mut cursor4 = TextCursor::at(sel_head);
    let mut selection4 = Selection {
        anchor: sel_anchor,
        head: sel_head,
    };
    apply_delete_word_forward(&mut text4, &mut selection4, &mut cursor4, &boundaries);

    // All must be identical
    assert_eq!(text1, text2, "Backspace vs Delete with selection");
    assert_eq!(text2, text3, "Delete vs Ctrl+Backspace with selection");
    assert_eq!(text3, text4, "Ctrl+Backspace vs Ctrl+Delete with selection");

    assert_eq!(cursor1.pos, cursor2.pos, "Cursor: Backspace vs Delete");
    assert_eq!(cursor2.pos, cursor3.pos, "Cursor: Delete vs Ctrl+Backspace");
    assert_eq!(
        cursor3.pos, cursor4.pos,
        "Cursor: Ctrl+Backspace vs Ctrl+Delete"
    );
}

#[test]
fn reversed_selection_deleted_identically_SPEC_107_13_4() {
    // SPEC-HAL-107 §13.4: Selection direction doesn't affect deletion.

    let original = "hello world";
    let boundaries = make_ascii_boundaries(original);

    // Forward selection: anchor < head
    let mut text1 = original.to_string();
    let mut cursor1 = TextCursor::at(8);
    let mut selection1 = Selection { anchor: 3, head: 8 };
    apply_edit_op(
        &mut text1,
        &mut selection1,
        &mut cursor1,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Reversed selection: anchor > head
    let mut text2 = original.to_string();
    let mut cursor2 = TextCursor::at(3);
    let mut selection2 = Selection { anchor: 8, head: 3 };
    apply_edit_op(
        &mut text2,
        &mut selection2,
        &mut cursor2,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(
        text1, text2,
        "Selection direction must not affect deletion result"
    );
    assert_eq!(
        cursor1.pos, cursor2.pos,
        "Cursor position must match regardless of selection direction"
    );
}
