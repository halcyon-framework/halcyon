// SPEC-HAL-052 Stage 3: Text Selection Tests

#![allow(unused_assignments, unused_variables)]

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for simple ASCII text where each char is one cluster.
/// Each glyph is 10px wide.
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

/// Create boundaries for multi-line text.
fn make_multiline_boundaries(lines: &[&[(usize, usize, usize)]]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut max_byte = 0;

    for (line_index, clusters) in lines.iter().enumerate() {
        let mut x = 0.0;
        for &(byte_start, byte_end, _line) in *clusters {
            boundaries.push(ClusterBoundary {
                byte_start,
                byte_end,
                line_index,
                x_logical: x,
            });
            x += 10.0;
            if byte_end > max_byte {
                max_byte = byte_end;
            }
        }
    }

    // Sentinel on last line
    let last_line = if lines.is_empty() { 0 } else { lines.len() - 1 };
    let last_x = boundaries
        .iter()
        .filter(|b| b.line_index == last_line)
        .map(|b| b.x_logical + 10.0)
        .fold(0.0_f32, f32::max);

    boundaries.push(ClusterBoundary {
        byte_start: max_byte,
        byte_end: max_byte,
        line_index: last_line,
        x_logical: last_x,
    });

    boundaries
}

// ============================================================================
// 8.1 Basics Tests
// ============================================================================

#[test]
fn test_click_sets_anchor_and_head() {
    // Simulate mouse down at position 3
    let pos = 3;

    let mut selection = Selection::new();
    let mut cursor = TextCursor::new();

    // Mouse down behavior
    selection.anchor = pos;
    selection.head = pos;
    cursor.collapse_to(pos);

    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 3);
    assert_eq!(cursor.pos, 3);
    assert!(selection.is_empty());
}

#[test]
fn test_drag_expands_head() {
    let mut selection = Selection::new();
    let mut cursor = TextCursor::new();

    // Mouse down at position 2
    let start_pos = 2;
    selection.anchor = start_pos;
    selection.head = start_pos;
    cursor.collapse_to(start_pos);

    // Drag to position 5
    let drag_pos = 5;
    selection.extend_to(drag_pos);
    cursor.pos = drag_pos;

    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 5);
    assert_eq!(selection.start(), 2);
    assert_eq!(selection.end(), 5);
    assert!(!selection.is_empty());
}

#[test]
fn test_drag_backwards() {
    let mut selection = Selection::new();
    let mut cursor = TextCursor::new();

    // Mouse down at position 5
    let start_pos = 5;
    selection.anchor = start_pos;
    selection.head = start_pos;
    cursor.collapse_to(start_pos);

    // Drag backwards to position 2
    let drag_pos = 2;
    selection.extend_to(drag_pos);
    cursor.pos = drag_pos;

    assert_eq!(selection.anchor, 5);
    assert_eq!(selection.head, 2);
    assert_eq!(selection.start(), 2); // min
    assert_eq!(selection.end(), 5); // max
    assert!(!selection.is_empty());
}

// ============================================================================
// 8.2 Collapse Tests
// ============================================================================

#[test]
fn test_left_without_shift_collapses_to_start() {
    let boundaries = make_ascii_boundaries("hello");

    // Create selection from 2 to 4
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 4;
    let mut cursor = TextCursor::at(4);

    // Left without Shift: collapse to start
    assert!(!selection.is_empty());
    cursor.pos = selection.start();
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 2);
    assert!(selection.is_empty());
}

#[test]
fn test_right_without_shift_collapses_to_end() {
    let boundaries = make_ascii_boundaries("hello");

    // Create selection from 2 to 4
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 4;
    let mut cursor = TextCursor::at(4);

    // Right without Shift: collapse to end
    assert!(!selection.is_empty());
    cursor.pos = selection.end();
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 4);
    assert_eq!(selection.anchor, 4);
    assert_eq!(selection.head, 4);
    assert!(selection.is_empty());
}

#[test]
fn test_left_right_on_empty_selection_moves_cursor() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Left on empty selection moves cursor normally
    cursor.move_left(&boundaries);
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());

    // Right on empty selection moves cursor normally
    cursor.move_right(&boundaries);
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 2);
    assert!(selection.is_empty());
}

// ============================================================================
// 8.3 Shift-Selection Navigation Tests
// ============================================================================

#[test]
fn test_shift_left_extends_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    // Shift+Left: extend selection
    cursor.move_left(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 2);
    assert_eq!(selection.start(), 2);
    assert_eq!(selection.end(), 3);
}

#[test]
fn test_shift_right_extends_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Shift+Right: extend selection
    cursor.move_right(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(cursor.pos, 3);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 3);
    assert_eq!(selection.start(), 2);
    assert_eq!(selection.end(), 3);
}

#[test]
fn test_shift_up_extends_across_lines() {
    // "abc\ndef" - two lines
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0)], // abc
        &[(4, 5, 1), (5, 6, 1), (6, 7, 1)], // def
    ]);

    // Start on line 1 at 'd' (byte 4)
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    // Shift+Up: extend to line 0
    cursor.move_up(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(selection.anchor, 4);
    assert!(cursor.pos < 4); // Moved to line 0
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_shift_down_extends_across_lines() {
    // "abc\ndef" - two lines
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0)], // abc
        &[(4, 5, 1), (5, 6, 1), (6, 7, 1)], // def
    ]);

    // Start on line 0 at 'a' (byte 0)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Shift+Down: extend to line 1
    cursor.move_down(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(selection.anchor, 0);
    assert!(cursor.pos >= 4); // Moved to line 1
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_shift_home_extends_to_line_start() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    // Shift+Home: extend to start of line
    cursor.move_to_start_of_line(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 0);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 3);
}

#[test]
fn test_shift_end_extends_to_line_end() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Shift+End: extend to end of line (sentinel)
    cursor.move_to_end_of_line(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(cursor.pos, 5); // sentinel
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 5);
    assert_eq!(selection.start(), 2);
    assert_eq!(selection.end(), 5);
}

// ============================================================================
// 8.4 Preferred_x Interaction Tests
// ============================================================================

#[test]
fn test_shift_up_preserves_preferred_x() {
    // Three lines with different lengths
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0), (3, 4, 0), (4, 5, 0)], // abcde
        &[(6, 7, 1), (7, 8, 1)],                                  // fg
        &[
            (9, 10, 2),
            (10, 11, 2),
            (11, 12, 2),
            (12, 13, 2),
            (13, 14, 2),
        ], // hijkl
    ]);

    // Start at 'd' on line 0 (x=30)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    // Move down twice with Shift
    cursor.move_down(&boundaries);
    selection.extend_to(cursor.pos);
    let after_first_down = cursor.pos;

    cursor.move_down(&boundaries);
    selection.extend_to(cursor.pos);

    // preferred_x should be preserved, so we should land near x=30 on line 2
    // Line 2 has chars at x=0,10,20,30,40 - closest to 30 is 'k' at byte 12
    assert!(cursor.preferred_x.is_some());
}

#[test]
fn test_horizontal_shift_movement_resets_preferred_x() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // First set preferred_x via vertical movement (even though single line)
    // For testing, we'll manually set it
    cursor.preferred_x = Some(20.0);

    // Shift+Right should reset preferred_x
    cursor.move_right(&boundaries);
    selection.extend_to(cursor.pos);

    assert!(cursor.preferred_x.is_none());
}

// ============================================================================
// 8.5 Sentinel Behavior Tests
// ============================================================================

#[test]
fn test_selection_to_end_of_text() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Shift+End to select to sentinel
    cursor.move_to_end_of_line(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(cursor.pos, 5); // sentinel
    assert_eq!(selection.end(), 5);
}

#[test]
fn test_break_out_moves_to_sentinel_or_zero() {
    let boundaries = make_ascii_boundaries("hello");

    // Selection from 2 to 5 (sentinel)
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 5;
    let mut cursor = TextCursor::at(5);

    // Right without Shift: collapse to end (sentinel)
    cursor.pos = selection.end();
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 5);

    // Create selection from 0 to 3
    selection.anchor = 0;
    selection.head = 3;
    cursor.pos = 3;

    // Left without Shift: collapse to start (0)
    cursor.pos = selection.start();
    selection.clear(cursor.pos);

    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 8.6 Multi-line Tests
// ============================================================================

#[test]
fn test_anchor_line0_extend_to_line2() {
    // "abc\ndef\nghi" - three lines
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0)],    // abc
        &[(4, 5, 1), (5, 6, 1), (6, 7, 1)],    // def
        &[(8, 9, 2), (9, 10, 2), (10, 11, 2)], // ghi
    ]);

    // Start at 'b' on line 0 (byte 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // Shift+Down twice to line 2
    cursor.move_down(&boundaries);
    selection.extend_to(cursor.pos);

    cursor.move_down(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(selection.anchor, 1);
    // Head should be on line 2
    let head_boundary = boundaries.iter().find(|b| b.byte_start == cursor.pos);
    assert!(head_boundary.is_some());
    assert_eq!(head_boundary.unwrap().line_index, 2);
}

#[test]
fn test_extend_upward_from_middle() {
    // "abc\ndef\nghi" - three lines
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0)],    // abc
        &[(4, 5, 1), (5, 6, 1), (6, 7, 1)],    // def
        &[(8, 9, 2), (9, 10, 2), (10, 11, 2)], // ghi
    ]);

    // Start at 'e' on line 1 (byte 5)
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Shift+Up to line 0
    cursor.move_up(&boundaries);
    selection.extend_to(cursor.pos);

    assert_eq!(selection.anchor, 5);
    // Head should be on line 0
    let head_boundary = boundaries.iter().find(|b| b.byte_start == cursor.pos);
    assert!(head_boundary.is_some());
    assert_eq!(head_boundary.unwrap().line_index, 0);
}

#[test]
fn test_click_on_last_line_collapses_previous_selection() {
    // "abc\ndef" - two lines
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1, 0), (1, 2, 0), (2, 3, 0)], // abc
        &[(4, 5, 1), (5, 6, 1), (6, 7, 1)], // def
    ]);

    // Create selection from 0 to 2 (on line 0)
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 2;
    let mut cursor = TextCursor::at(2);

    // Click on line 1 at 'e' (byte 5)
    let click_pos = 5;
    selection.anchor = click_pos;
    selection.head = click_pos;
    cursor.collapse_to(click_pos);

    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, 5);
    assert_eq!(selection.head, 5);
}

// ============================================================================
// Additional Edge Cases
// ============================================================================

#[test]
fn test_selection_constructors() {
    let s1 = Selection::new();
    assert_eq!(s1.anchor, 0);
    assert_eq!(s1.head, 0);

    let s2 = Selection::at(42);
    assert_eq!(s2.anchor, 42);
    assert_eq!(s2.head, 42);

    let s3 = Selection::default();
    assert_eq!(s3.anchor, 0);
    assert_eq!(s3.head, 0);
}

#[test]
fn test_cursor_collapse_to() {
    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = Some(50.0);

    cursor.collapse_to(10);

    assert_eq!(cursor.pos, 10);
    assert_eq!(cursor.preferred_x, None);
}

#[test]
fn test_selection_start_end_invariant() {
    // Forward selection
    let mut s = Selection::new();
    s.anchor = 2;
    s.head = 5;
    assert_eq!(s.start(), 2);
    assert_eq!(s.end(), 5);

    // Backward selection
    s.anchor = 5;
    s.head = 2;
    assert_eq!(s.start(), 2);
    assert_eq!(s.end(), 5);

    // Empty selection
    s.anchor = 3;
    s.head = 3;
    assert_eq!(s.start(), 3);
    assert_eq!(s.end(), 3);
    assert!(s.is_empty());
}
