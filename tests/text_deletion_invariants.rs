// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op};

fn make_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_on_line = 0.0;

    for (byte_start, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start,
            byte_end: byte_start + ch.len_utf8(),
            line_index,
            x_logical: x_on_line,
        });

        if ch == '\n' {
            line_index += 1;
            x_on_line = 0.0;
        } else {
            x_on_line += 10.0;
        }
    }

    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: x_on_line,
    });

    boundaries
}

#[test]
fn backspace_deletes_previous_character() {
    let mut text = String::from("abcd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "acd");
    assert_eq!(cursor.pos, 1);
    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 1);
}

#[test]
fn delete_deletes_next_character() {
    let mut text = String::from("abcd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "acd");
    assert_eq!(cursor.pos, 1);
    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 1);
}

#[test]
fn deleting_selection_removes_exact_range() {
    let mut text = String::from("abcdef");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection { anchor: 2, head: 5 };

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "abf");
    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 2);
}

#[test]
fn deleting_newline_merges_lines() {
    let mut text = String::from("ab\ncd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    assert_eq!(text, "abcd");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn deletion_never_reorders_text() {
    let mut text = String::from("abcdef");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection { anchor: 2, head: 4 };

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Delete,
    );

    assert_eq!(text, "abef");
    assert!(text.starts_with("ab"));
    assert!(text.ends_with("ef"));
}
