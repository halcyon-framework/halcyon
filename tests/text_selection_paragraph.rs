// SPEC-HAL-052 Stage 12: Paragraph-Bound Movement & Selection Collapse Tests

#![allow(unused_variables)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, move_down, move_end, move_end_select, move_home,
    move_home_select, move_left_collapse, move_left_select, move_right_collapse, move_right_select,
    move_to_paragraph_end, move_to_paragraph_end_select, move_to_paragraph_start,
    move_to_paragraph_start_select, move_up, move_word_left, move_word_left_select,
    move_word_right, move_word_right_select,
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

/// Create boundaries for multi-line text.
fn make_multiline_boundaries(lines: &[&str]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line_text) in lines.iter().enumerate() {
        for (i, _) in line_text.char_indices() {
            boundaries.push(ClusterBoundary {
                byte_start: byte_offset + i,
                byte_end: byte_offset + i + 1,
                line_index: line_idx,
                x_logical: i as f32 * 10.0,
            });
        }
        byte_offset += line_text.len();
    }

    // Sentinel on last line
    let last_line_idx = if lines.is_empty() { 0 } else { lines.len() - 1 };
    let last_line_len = lines.last().map(|s| s.len()).unwrap_or(0);
    boundaries.push(ClusterBoundary {
        byte_start: byte_offset,
        byte_end: byte_offset,
        line_index: last_line_idx,
        x_logical: last_line_len as f32 * 10.0,
    });

    boundaries
}

// ============================================================================
// 1. Home/End at Boundaries
// ============================================================================

#[test]
fn test_home_from_middle() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_home(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, 0);
}

#[test]
fn test_home_already_at_start() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_home(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_end_from_middle() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_end(&mut cursor, &mut selection, &boundaries);

    // End moves to sentinel (position 5, after 'o')
    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_end_already_at_end() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(5); // At sentinel
    let mut selection = Selection::at(5);

    move_end(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_home_multiline() {
    // "abc\ndef" represented as two lines
    let boundaries = make_multiline_boundaries(&["abc", "def"]);
    // Start in middle of line 1 ("def")
    let mut cursor = TextCursor::at(4); // 'd' is at byte 3, 'e' at 4
    let mut selection = Selection::at(4);

    move_home(&mut cursor, &mut selection, &boundaries);

    // Home should go to start of line 1, which is 'd' at byte 3
    assert_eq!(cursor.pos, 3);
    assert!(selection.is_empty());
}

#[test]
fn test_end_multiline() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);
    let mut cursor = TextCursor::at(3); // Start at 'd'
    let mut selection = Selection::at(3);

    move_end(&mut cursor, &mut selection, &boundaries);

    // End should go to sentinel on line 1 (byte 6)
    assert_eq!(cursor.pos, 6);
    assert!(selection.is_empty());
}

// ============================================================================
// 2. Ctrl+Home/Ctrl+End Collapse Behaviors
// ============================================================================

#[test]
fn test_ctrl_home_from_middle() {
    let boundaries = make_ascii_boundaries("hello world");
    let mut cursor = TextCursor::at(6);
    let mut selection = Selection::at(6);

    move_to_paragraph_start(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_ctrl_home_clears_selection() {
    let boundaries = make_ascii_boundaries("hello world");
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 8;

    move_to_paragraph_start(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_ctrl_end_from_middle() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_to_paragraph_end(&mut cursor, &mut selection, &boundaries);

    // Sentinel is at byte 5
    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_ctrl_end_clears_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 3;

    move_to_paragraph_end(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

// ============================================================================
// 3. Selection Collapsing Across Lines
// ============================================================================

#[test]
fn test_left_collapse_with_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4;

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Collapses to selection.start (1)
    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());
}

#[test]
fn test_left_collapse_without_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Normal left movement
    assert_eq!(cursor.pos, 2);
    assert!(selection.is_empty());
}

#[test]
fn test_right_collapse_with_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4;

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Collapses to selection.end (4)
    assert_eq!(cursor.pos, 4);
    assert!(selection.is_empty());
}

#[test]
fn test_right_collapse_without_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Normal right movement
    assert_eq!(cursor.pos, 3);
    assert!(selection.is_empty());
}

#[test]
fn test_left_collapse_reversed_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::new();
    selection.anchor = 4; // Reversed: anchor > head
    selection.head = 1;

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Collapses to selection.start (1)
    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());
}

#[test]
fn test_multiline_selection_left_collapse() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);
    let mut cursor = TextCursor::at(4); // On line 1
    let mut selection = Selection::new();
    selection.anchor = 1; // Line 0
    selection.head = 4; // Line 1

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // Collapses to start of selection (line 0)
    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());
}

// ============================================================================
// 4. Word Navigation at Boundaries
// ============================================================================

#[test]
fn test_word_left_at_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_word_left(&mut cursor, &mut selection, &boundaries, text);

    // Already at start, stays at 0
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_word_right_at_end() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(5); // Sentinel
    let mut selection = Selection::at(5);

    move_word_right(&mut cursor, &mut selection, &boundaries, text);

    // Already at end, stays at sentinel
    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_word_left_basic() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(8); // In "world"
    let mut selection = Selection::at(8);

    move_word_left(&mut cursor, &mut selection, &boundaries, text);

    // Should move to start of "world" or past the space
    assert!(cursor.pos <= 6);
    assert!(selection.is_empty());
}

#[test]
fn test_word_right_basic() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_word_right(&mut cursor, &mut selection, &boundaries, text);

    // Should move past "hello" to space or "world"
    assert!(cursor.pos >= 5);
    assert!(selection.is_empty());
}

// ============================================================================
// 5. Up/Down Collapse at First/Last Visual Line
// ============================================================================

#[test]
fn test_up_at_first_line_with_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 3;

    move_up(&mut cursor, &mut selection, &boundaries);

    // On single line, up moves to start of line and clears selection
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_down_at_last_line_with_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 2;

    move_down(&mut cursor, &mut selection, &boundaries);

    // On single line, down clamps but clears selection
    assert_eq!(cursor.pos, 2);
    assert!(selection.is_empty());
}

#[test]
fn test_up_multiline_clears_selection() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);
    let mut cursor = TextCursor::at(4); // On line 1
    let mut selection = Selection::new();
    selection.anchor = 3;
    selection.head = 5;

    move_up(&mut cursor, &mut selection, &boundaries);

    // Moves up to line 0 and clears selection
    let cluster = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(cluster.line_index, 0);
    assert!(selection.is_empty());
}

// ============================================================================
// 6. Multi-Line Selection Delete
// ============================================================================

#[test]
fn test_multiline_selection_range() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);

    let mut selection = Selection::new();
    selection.anchor = 1; // 'b' on line 0
    selection.head = 4; // 'e' on line 1

    // Selection should span [1, 4)
    assert_eq!(selection.start(), 1);
    assert_eq!(selection.end(), 4);
}

#[test]
fn test_multiline_selection_reversed() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);

    let mut selection = Selection::new();
    selection.anchor = 4; // Line 1
    selection.head = 1; // Line 0 (reversed)

    // start/end should normalize correctly
    assert_eq!(selection.start(), 1);
    assert_eq!(selection.end(), 4);
}

// ============================================================================
// Shift Selection Variants
// ============================================================================

#[test]
fn test_shift_home_creates_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_home_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 0);
    assert!(!selection.is_empty());
}

#[test]
fn test_shift_end_creates_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_end_select(&mut cursor, &mut selection, &boundaries);

    // End goes to sentinel (position 5)
    assert_eq!(cursor.pos, 5);
    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 5);
}

#[test]
fn test_ctrl_shift_home_extends_selection() {
    let boundaries = make_ascii_boundaries("hello world");
    let mut cursor = TextCursor::at(6);
    let mut selection = Selection::at(6);

    move_to_paragraph_start_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 6);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_ctrl_shift_end_extends_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_to_paragraph_end_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 5);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 5);
}

#[test]
fn test_shift_left_extends_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_left_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 2);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 2);
}

#[test]
fn test_shift_right_extends_selection() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_right_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 3);
    assert_eq!(selection.anchor, 2);
    assert_eq!(selection.head, 3);
}

#[test]
fn test_shift_word_left_extends_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    move_word_left_select(&mut cursor, &mut selection, &boundaries, text);

    assert_eq!(selection.anchor, 8);
    assert!(cursor.pos < 8);
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_shift_word_right_extends_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_word_right_select(&mut cursor, &mut selection, &boundaries, text);

    assert_eq!(selection.anchor, 0);
    assert!(cursor.pos > 0);
    assert_eq!(selection.head, cursor.pos);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // These should not panic
    move_home(&mut cursor, &mut selection, &boundaries);
    move_end(&mut cursor, &mut selection, &boundaries);
    move_to_paragraph_start(&mut cursor, &mut selection, &boundaries);
    move_to_paragraph_end(&mut cursor, &mut selection, &boundaries);
}

#[test]
fn test_single_cluster_home_end() {
    let boundaries = make_ascii_boundaries("a");
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_end(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 1); // End goes to sentinel (position 1)

    move_home(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_preferred_x_updated_on_home() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);
    cursor.preferred_x = Some(30.0);
    let mut selection = Selection::at(3);

    move_home(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be updated to the new position's x_logical
    assert_eq!(cursor.preferred_x, Some(0.0));
}

#[test]
fn test_preferred_x_cleared_on_left_right() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(20.0);
    let mut selection = Selection::at(2);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be cleared
    assert_eq!(cursor.preferred_x, None);
}
