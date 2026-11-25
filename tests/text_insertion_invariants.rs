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

#[test]
fn insertion_occurs_exactly_at_cursor() {
    let mut text = String::from("abcd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "XY");

    assert_eq!(text, "abXYcd");
    assert_eq!(cursor.pos, 4);
    assert_eq!(selection.anchor, 4);
    assert_eq!(selection.head, 4);
}

#[test]
fn insertion_preserves_prefix_and_suffix_order() {
    let mut text = String::from("prefixsuffix");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at("prefix".len());
    let mut selection = Selection::at("prefix".len());

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "-");

    assert_eq!(text, "prefix-suffix");
    assert!(text.starts_with("prefix"));
    assert!(text.ends_with("suffix"));
}

#[test]
fn inserting_at_start_of_line_is_valid() {
    let mut text = String::from("alpha\nbeta");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(6);
    let mut selection = Selection::at(6);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, ">");

    assert_eq!(text, "alpha\n>beta");
    assert_eq!(cursor.pos, 7);
}

#[test]
fn inserting_at_end_of_line_is_valid() {
    let mut text = String::from("alpha\nbeta");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "<");

    assert_eq!(text, "alpha<\nbeta");
    assert_eq!(cursor.pos, 6);
}

#[test]
fn cursor_advances_by_inserted_byte_length() {
    let mut text = String::from("ab");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "é");

    assert_eq!(text, "aéb");
    assert_eq!(cursor.pos, 1 + "é".len());
    assert_eq!(selection.anchor, cursor.pos);
    assert_eq!(selection.head, cursor.pos);
}
