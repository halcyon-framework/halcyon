// SPEC-HAL-052 Stage 8: Text Insertion Ops Tests

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op, apply_insert_op,
};

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
// 5.1 Insert into Empty Document
// ============================================================================

#[test]
fn test_insert_ascii_into_empty() {
    let mut text = String::new();
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "a");

    assert_eq!(text, "a");
    assert_eq!(cursor.pos, 1);
    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 1);
}

#[test]
fn test_insert_emoji_into_empty() {
    let mut text = String::new();
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "👍");

    assert_eq!(text, "👍");
    assert_eq!(cursor.pos, 4); // emoji is 4 bytes
}

#[test]
fn test_insert_cyrillic_into_empty() {
    let mut text = String::new();
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "Привет",
    );

    assert_eq!(text, "Привет");
    assert_eq!(cursor.pos, 12); // 6 chars * 2 bytes each
}

// ============================================================================
// 5.2 Insert at Start/Middle/End
// ============================================================================

#[test]
fn test_insert_ascii_at_start() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(text, "Xhello");
    assert_eq!(cursor.pos, 1);
}

#[test]
fn test_insert_ascii_at_middle() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(text, "heXllo");
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_insert_ascii_at_end() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(text, "helloX");
    assert_eq!(cursor.pos, 6);
}

#[test]
fn test_insert_utf8_at_middle() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "日本");

    assert_eq!(text, "he日本llo");
    assert_eq!(cursor.pos, 2 + 6); // 2 chars * 3 bytes each
}

#[test]
fn test_caret_pos_equals_old_plus_len() {
    let mut text = String::from("abc");
    let boundaries = make_ascii_boundaries(&text);

    let old_pos = 1;
    let mut selection = Selection::at(old_pos);
    let mut cursor = TextCursor::at(old_pos);

    let insert_str = "XYZ";
    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        insert_str,
    );

    assert_eq!(cursor.pos, old_pos + insert_str.len());
}

// ============================================================================
// 5.3 Replace a Selection
// ============================================================================

#[test]
fn test_replace_ascii_selection() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5; // Select "hello"

    let mut cursor = TextCursor::at(5);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "hi");

    assert_eq!(text, "hi world");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_replace_utf8_selection() {
    let mut text = String::from("Привет мир");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 12; // Select "Привет"

    let mut cursor = TextCursor::at(12);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "Здравствуй",
    );

    assert_eq!(text, "Здравствуй мир");
    assert_eq!(cursor.pos, 20); // 10 chars * 2 bytes
}

#[test]
fn test_replace_with_shorter_string() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Select "ell"

    let mut cursor = TextCursor::at(4);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "i");

    assert_eq!(text, "hio");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_replace_with_longer_string() {
    let mut text = String::from("hi");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 2; // Select all

    let mut cursor = TextCursor::at(2);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "hello world",
    );

    assert_eq!(text, "hello world");
    assert_eq!(cursor.pos, 11);
}

#[test]
fn test_replace_multibyte_with_ascii() {
    let mut text = String::from("日本語");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 9; // Select all (3 chars * 3 bytes)

    let mut cursor = TextCursor::at(9);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "Japanese",
    );

    assert_eq!(text, "Japanese");
    assert_eq!(cursor.pos, 8);
}

#[test]
fn test_replace_ascii_with_multibyte() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5; // Select all

    let mut cursor = TextCursor::at(5);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "日本");

    assert_eq!(text, "日本");
    assert_eq!(cursor.pos, 6); // 2 chars * 3 bytes
}

// ============================================================================
// 5.4 Collapse Invariant
// ============================================================================

#[test]
fn test_collapse_invariant_after_insert() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_collapse_invariant_after_replace() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4;

    let mut cursor = TextCursor::at(4);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "XYZ");

    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_preferred_x_cleared() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(50.0);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(cursor.preferred_x, None);
}

// ============================================================================
// 5.5 Integration with Boundaries
// ============================================================================

#[test]
fn test_boundaries_regenerated_after_insert() {
    let mut text = String::from("ab");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    assert_eq!(text, "aXb");

    // Regenerate boundaries (as caller would)
    let new_boundaries = make_ascii_boundaries(&text);

    // Verify new boundaries are correct
    assert_eq!(new_boundaries.len(), 4); // a, X, b, sentinel
    assert_eq!(new_boundaries[0].byte_start, 0);
    assert_eq!(new_boundaries[1].byte_start, 1);
    assert_eq!(new_boundaries[2].byte_start, 2);
    assert_eq!(new_boundaries[3].byte_start, 3); // sentinel
}

#[test]
fn test_cursor_at_valid_position_after_insert() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "日");

    // Verify cursor is at valid UTF-8 boundary
    assert!(text.is_char_boundary(cursor.pos));
}

// ============================================================================
// 5.6 Multiple Sequential Inserts
// ============================================================================

#[test]
fn test_sequential_inserts() {
    let mut text = String::new();
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    // Insert "h"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "h");
    assert_eq!(text, "h");
    boundaries = make_ascii_boundaries(&text);

    // Insert "e"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "e");
    assert_eq!(text, "he");
    boundaries = make_ascii_boundaries(&text);

    // Insert "llo"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "llo");
    assert_eq!(text, "hello");

    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_sequential_utf8_inserts() {
    let mut text = String::new();
    let mut boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    // Insert "П"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "П");
    boundaries = make_utf8_boundaries(&text);

    // Insert "ривет"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "ривет");

    assert_eq!(text, "Привет");
    assert_eq!(cursor.pos, 12);
}

// ============================================================================
// 5.7 Insert After Backspace/Delete from Stage 7
// ============================================================================

#[test]
fn test_insert_after_backspace() {
    let mut text = String::from("hello");
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3);
    let mut cursor = TextCursor::at(3);

    // Backspace to delete 'l'
    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );
    assert_eq!(text, "helo");
    assert_eq!(cursor.pos, 2);

    // Regenerate boundaries
    boundaries = make_ascii_boundaries(&text);

    // Insert "LL"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "LL");
    assert_eq!(text, "heLLlo");
    assert_eq!(cursor.pos, 4);
}

#[test]
fn test_insert_after_delete() {
    let mut text = String::from("hello");
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    // Delete 'e'
    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );
    assert_eq!(text, "hllo");
    assert_eq!(cursor.pos, 1);

    // Regenerate boundaries
    boundaries = make_ascii_boundaries(&text);

    // Insert "EE"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "EE");
    assert_eq!(text, "hEEllo");
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_delete_selection_then_insert() {
    let mut text = String::from("hello world");
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 6; // Select "hello "

    let mut cursor = TextCursor::at(6);

    // Delete selection via backspace
    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );
    assert_eq!(text, "world");
    assert_eq!(cursor.pos, 0);

    // Regenerate boundaries
    boundaries = make_ascii_boundaries(&text);

    // Insert replacement
    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "goodbye ",
    );
    assert_eq!(text, "goodbye world");
    assert_eq!(cursor.pos, 8);
}

// ============================================================================
// Additional Edge Cases
// ============================================================================

#[test]
fn test_insert_empty_string() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2);
    let mut cursor = TextCursor::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "");

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_replace_selection_with_empty_string() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Select "ell"

    let mut cursor = TextCursor::at(4);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "");

    assert_eq!(text, "ho");
    assert_eq!(cursor.pos, 1);
}

#[test]
fn test_insert_multiline_string() {
    let mut text = String::from("ab");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(1);
    let mut cursor = TextCursor::at(1);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        "X\nY\nZ",
    );

    assert_eq!(text, "aX\nY\nZb");
    assert_eq!(cursor.pos, 6); // 1 + 5 bytes
}
