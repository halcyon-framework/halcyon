// SPEC-HAL-052 Stage 7: Basic Editing Ops Tests

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for simple ASCII text where each char is one cluster.
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
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * 10.0,
    });
    boundaries
}

/// Create boundaries for UTF-8 text, respecting char boundaries.
fn make_utf8_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    for (i, (byte_start, _)) in chars.iter().enumerate() {
        let byte_end = if i + 1 < chars.len() {
            chars[i + 1].0
        } else {
            text.len()
        };
        boundaries.push(ClusterBoundary {
            byte_start: *byte_start,
            byte_end,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: boundaries.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// 5.1 Selection Deletion
// ============================================================================

#[test]
fn test_delete_middle_range() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 5; // Select "llo"

    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace, // Op doesn't matter when selection is non-empty
    );

    assert_eq!(text, "he world");
    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 2);
}

#[test]
fn test_delete_beginning() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 2; // Select "he"

    let mut cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "llo");
    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_delete_ending() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 3;
    selection.head = 5; // Select "lo"

    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "hel");
    assert_eq!(cursor.pos, 3);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 3);
}

#[test]
fn test_delete_whole_text() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5; // Select all

    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "");
    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_delete_selection_reversed() {
    // Selection with anchor > head
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 4;
    selection.head = 1; // Reversed selection "ell"

    let mut cursor = TextCursor::at(1);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "ho");
    assert_eq!(cursor.pos, 1); // Collapses to start of selection
    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 1);
}

// ============================================================================
// 5.2 Backspace Behavior
// ============================================================================

#[test]
fn test_backspace_at_start_noop() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_backspace_ascii_cluster() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3); // After "hel"
    let mut cursor = TextCursor::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "helo");
    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 2);
}

#[test]
fn test_backspace_utf8_cyrillic() {
    // "Привет" - each char is 2 bytes
    let mut text = String::from("Привет");
    let boundaries = make_utf8_boundaries(&text);

    // Cursor after 'р' (byte 4)
    let mut selection = Selection::at(4);
    let mut cursor = TextCursor::at(4);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "Пивет"); // Removed 'р'
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_backspace_utf8_emoji() {
    // "a👍b" - emoji is 4 bytes
    let mut text = String::from("a👍b");
    let boundaries = make_utf8_boundaries(&text);
    // Boundaries: 'a' at 0-1, '👍' at 1-5, 'b' at 5-6, sentinel at 6

    // Cursor after emoji (byte 5)
    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "ab");
    assert_eq!(cursor.pos, 1);
}

#[test]
fn test_backspace_after_first_char() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "ello");
    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 5.3 Forward Delete
// ============================================================================

#[test]
fn test_delete_at_end_noop() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5); // At sentinel
    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_delete_ascii_cluster() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2); // Before 'l'
    let mut cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "helo");
    assert_eq!(cursor.pos, 2); // Unchanged
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 2);
}

#[test]
fn test_delete_utf8_cyrillic() {
    let mut text = String::from("Привет");
    let boundaries = make_utf8_boundaries(&text);

    // Delete 'р' at byte 2
    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "Пивет");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_delete_utf8_emoji() {
    let mut text = String::from("a👍b");
    let boundaries = make_utf8_boundaries(&text);

    // Delete emoji at byte 1
    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "ab");
    assert_eq!(cursor.pos, 1);
}

#[test]
fn test_delete_first_char() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "ello");
    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 5.4 Half-Open Invariant After Deletion
// ============================================================================

#[test]
fn test_invariant_after_selection_delete() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 8;

    let mut cursor = TextCursor::at(8);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    // Assert invariant
    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_invariant_after_backspace() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3);
    let mut cursor = TextCursor::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_invariant_after_delete() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
}

// ============================================================================
// 5.5 Integration with Boundaries
// ============================================================================

#[test]
fn test_full_cluster_removal_ascii() {
    let mut text = String::from("abc");
    let boundaries = make_ascii_boundaries(&text);

    // Delete 'b' (1 byte cluster)
    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "ac");
    // Verify it's still valid UTF-8
    assert!(text.is_char_boundary(0));
    assert!(text.is_char_boundary(1));
    assert!(text.is_char_boundary(2));
}

#[test]
fn test_full_cluster_removal_utf8() {
    // "café" - 'é' is 2 bytes
    let mut text = String::from("café");
    let boundaries = make_utf8_boundaries(&text);

    // Backspace after 'é' (byte 5)
    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "caf");
    // Verify valid UTF-8 boundaries
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_boundary_consistency_after_multiple_ops() {
    let mut text = String::from("hello");

    // First delete
    let boundaries = make_ascii_boundaries(&text);
    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );
    assert_eq!(text, "helo");

    // Regenerate boundaries (as caller would)
    let boundaries = make_ascii_boundaries(&text);

    // Second backspace
    selection = Selection::at(2);
    cursor = TextCursor::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );
    assert_eq!(text, "hlo");

    // Verify cursor is at valid position
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_empty_text_backspace() {
    let mut text = String::from("");
    let boundaries = vec![ClusterBoundary {
        byte_start: 0,
        byte_end: 0,
        line_index: 0,
        x_logical: 0.0,
    }];

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_empty_text_delete() {
    let mut text = String::from("");
    let boundaries = vec![ClusterBoundary {
        byte_start: 0,
        byte_end: 0,
        line_index: 0,
        x_logical: 0.0,
    }];

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "");
    assert_eq!(cursor.pos, 0);
}
