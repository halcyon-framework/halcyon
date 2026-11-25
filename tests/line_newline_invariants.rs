// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, apply_insert_op};
use std::ops::Range;

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

fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut line_start = 0;

    for (byte_index, ch) in text.char_indices() {
        if ch == '\n' {
            ranges.push(line_start..byte_index);
            line_start = byte_index + ch.len_utf8();
        }
    }

    ranges.push(line_start..text.len());
    ranges
}

#[test]
fn newline_splits_logical_lines() {
    let mut text = String::from("abcd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    assert_eq!(text, "ab\ncd");
    assert_eq!(line_ranges(&text), vec![0..2, 3..5]);
}

#[test]
fn empty_lines_are_valid() {
    let mut text = String::from("ab\ncd");
    let boundaries = make_boundaries(&text);
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    assert_eq!(text, "ab\n\ncd");
    assert_eq!(line_ranges(&text), vec![0..2, 3..3, 4..6]);
}

#[test]
fn first_line_starts_at_zero() {
    let ranges = line_ranges("alpha\nbeta");

    assert_eq!(ranges.first(), Some(&(0..5)));
    assert_eq!(ranges[0].start, 0);
}

#[test]
fn last_line_may_or_may_not_end_with_newline() {
    assert_eq!(line_ranges("tail"), vec![0..4]);
    assert_eq!(line_ranges("tail\n"), vec![0..4, 5..5]);
}

#[test]
fn cursor_allowed_on_newline_boundary() {
    let text = "ab\ncd";
    let boundaries = make_boundaries(text);
    let cursor = TextCursor::at(2);
    let selection = Selection::at(2);

    assert_eq!(cursor.pos, selection.anchor);
    assert_eq!(selection.anchor, selection.head);
    assert!(
        boundaries
            .iter()
            .any(|boundary| boundary.byte_start == 2 && boundary.byte_end == 3)
    );
}
