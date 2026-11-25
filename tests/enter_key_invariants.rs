// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, apply_insert_op};

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

fn press_enter(original: &str, cursor_pos: usize) -> (String, TextCursor, Selection) {
    let mut text = original.to_string();
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    (text, cursor, selection)
}

#[test]
fn enter_inserts_single_newline() {
    let (text, _, _) = press_enter("abcd", 2);

    assert_eq!(text, "ab\ncd");
    assert_eq!(text.matches('\n').count(), 1);
}

#[test]
fn enter_preserves_prefix_and_suffix_order() {
    let (text, _, _) = press_enter("prefixsuffix", "prefix".len());

    assert_eq!(text, "prefix\nsuffix");
    assert_eq!(text.replace('\n', ""), "prefixsuffix");
}

#[test]
fn enter_advances_cursor_by_one_byte() {
    let (_, cursor, selection) = press_enter("abcd", 2);

    assert_eq!(cursor.pos, 3);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 3);
}

#[test]
fn enter_at_column_zero_inserts_before_line() {
    let (text, cursor, _) = press_enter("abc", 0);

    assert_eq!(text, "\nabc");
    assert_eq!(cursor.pos, 1);
}

#[test]
fn enter_in_middle_splits_line() {
    let (text, _, _) = press_enter("abcd", 2);

    assert_eq!(&text[..2], "ab");
    assert_eq!(&text[3..], "cd");
    assert_eq!(text.as_bytes()[2], b'\n');
}

#[test]
fn enter_at_end_creates_empty_line() {
    let (text, cursor, _) = press_enter("abc", 3);

    assert_eq!(text, "abc\n");
    assert_eq!(cursor.pos, text.len());
}

#[test]
fn enter_never_does_nothing() {
    let original = "abc";
    let (text, _, _) = press_enter(original, 1);

    assert_ne!(text, original);
    assert_eq!(text.len(), original.len() + 1);
}

#[test]
fn enter_never_moves_unrelated_text() {
    let (text, _, _) = press_enter("alphaOMEGA", "alpha".len());

    assert_eq!(text, "alpha\nOMEGA");
    assert_eq!(text.replace('\n', ""), "alphaOMEGA");
}

#[test]
fn enter_never_reorders_text() {
    let original = "abcdef";
    let (text, _, _) = press_enter(original, 3);

    assert_eq!(text, "abc\ndef");
    assert_eq!(text.replace('\n', ""), original);
}
