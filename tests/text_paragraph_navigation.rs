// SPEC-HAL-052 Stage 14: Multi-Paragraph Hard Boundaries + Soft-Wrap Block Movement Tests

use halcyon::core::editing::{
    ClusterBoundary, CursorAffinity, Selection, TextCursor, ctrl_move_down, ctrl_move_down_select,
    ctrl_move_up, ctrl_move_up_select, find_paragraph_boundary, hit_test_click, move_down,
    move_down_select, move_up, move_up_select,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for simple ASCII text where each char is one cluster on a single line.
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

/// Create boundaries for multi-line text with explicit line breaks at newlines.
/// Each character gets its own cluster, with line_index incremented at '\n'.
fn make_multiline_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_logical = 0.0;

    for (i, c) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + c.len_utf8(),
            line_index,
            x_logical,
        });
        if c == '\n' {
            line_index += 1;
            x_logical = 0.0;
        } else {
            x_logical += 10.0;
        }
    }

    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical,
    });
    boundaries
}

/// Create boundaries for soft-wrapped paragraph simulation.
/// Takes a text and wrap_width (in chars), creates visual lines by wrapping.
fn make_softwrap_boundaries(text: &str, wrap_width: usize) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_logical = 0.0;
    let mut chars_on_line = 0;

    for (i, c) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + c.len_utf8(),
            line_index,
            x_logical,
        });

        if c == '\n' {
            line_index += 1;
            x_logical = 0.0;
            chars_on_line = 0;
        } else {
            chars_on_line += 1;
            x_logical += 10.0;

            // Soft wrap at wrap_width
            if chars_on_line >= wrap_width {
                line_index += 1;
                x_logical = 0.0;
                chars_on_line = 0;
            }
        }
    }

    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical,
    });
    boundaries
}

// ============================================================================
// Ctrl+Up/Down Block Movement Tests
// ============================================================================

#[test]
fn test_ctrl_up_from_middle_to_paragraph_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5); // After "hello"
    let mut selection = Selection::at(5);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0); // Should be at paragraph start
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, 0);
}

#[test]
fn test_ctrl_up_from_paragraph_start_stays() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(0); // At start
    let mut selection = Selection::at(0);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0); // Already at start, stays
    assert!(selection.is_empty());
}

#[test]
fn test_ctrl_down_from_middle_to_paragraph_end() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5); // After "hello"
    let mut selection = Selection::at(5);

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len()); // Should be at paragraph end (sentinel)
    assert!(selection.is_empty());
}

#[test]
fn test_ctrl_down_from_paragraph_end_stays() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(text.len()); // At end
    let mut selection = Selection::at(text.len());

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len()); // Already at end, stays
    assert!(selection.is_empty());
}

#[test]
fn test_ctrl_up_clears_preferred_x() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = Some(50.0);
    let mut selection = Selection::at(5);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert!(cursor.preferred_x.is_none()); // Block movement clears preferred_x
}

#[test]
fn test_ctrl_down_clears_preferred_x() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = Some(50.0);
    let mut selection = Selection::at(5);

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert!(cursor.preferred_x.is_none()); // Block movement clears preferred_x
}

#[test]
fn test_ctrl_up_preserves_affinity() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(5, CursorAffinity::Right);
    let mut selection = Selection::at(5);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.affinity, CursorAffinity::Right); // Affinity preserved
}

#[test]
fn test_ctrl_down_preserves_affinity() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(5, CursorAffinity::Right);
    let mut selection = Selection::at(5);

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.affinity, CursorAffinity::Right); // Affinity preserved
}

// ============================================================================
// Ctrl+Up/Down with Selection Tests
// ============================================================================

#[test]
fn test_ctrl_up_select_extends_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    ctrl_move_up_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 5); // Anchor stays
    assert_eq!(selection.head, 0); // Head moves
    assert_eq!(&text[selection.start()..selection.end()], "hello");
}

#[test]
fn test_ctrl_down_select_extends_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    ctrl_move_down_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len());
    assert_eq!(selection.anchor, 5); // Anchor stays
    assert_eq!(selection.head, text.len()); // Head moves
    assert_eq!(&text[selection.start()..selection.end()], " world");
}

#[test]
fn test_ctrl_up_select_from_existing_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::new();
    selection.anchor = 5;
    selection.head = 8; // Existing selection "wor"

    ctrl_move_up_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 5); // Anchor preserved
    assert_eq!(selection.head, 0); // Head moved to start
}

#[test]
fn test_ctrl_down_select_from_existing_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::new();
    selection.anchor = 5;
    selection.head = 3; // Existing reversed selection

    ctrl_move_down_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len());
    assert_eq!(selection.anchor, 5); // Anchor preserved
    assert_eq!(selection.head, text.len()); // Head moved to end
}

// ============================================================================
// Up/Down Movement Across Visual Lines Tests
// ============================================================================

#[test]
fn test_move_up_at_first_line_goes_to_line_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);

    // At first visual line, should move to start of line
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_move_down_at_last_line_clamps() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_down(&mut cursor, &mut selection, &boundaries);

    // At last visual line, cursor stays, selection cleared
    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_move_up_select_at_first_line_extends_to_line_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 5);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_move_down_select_at_last_line_extends_to_line_end() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // At last line: extend selection to end of line (sentinel at position 11)
    assert_eq!(cursor.pos, 11);
    assert_eq!(selection.anchor, 5);
    assert_eq!(selection.head, 11);
}

// ============================================================================
// Soft-Wrap Navigation Tests
// ============================================================================

#[test]
fn test_softwrap_move_down_within_paragraph() {
    // "hello world" wrapped at 5 chars: "hello" / " worl" / "d"
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);

    // Start at 'h' (line 0)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should move to line 1 (after "hello")
    assert!(boundaries[find_boundary_index(&boundaries, cursor.pos)].line_index == 1);
}

#[test]
fn test_softwrap_move_up_within_paragraph() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);

    // Start at ' ' after "hello" (line 1)
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should move to line 0
    let idx = find_boundary_index(&boundaries, cursor.pos);
    assert_eq!(boundaries[idx].line_index, 0);
}

#[test]
fn test_softwrap_preferred_x_preserved() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);

    // Start at position with specific x
    let mut cursor = TextCursor::at(2); // 'l' in "hello" at x=20
    let mut selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be set and preserved
    assert!(cursor.preferred_x.is_some());
    let px = cursor.preferred_x.unwrap();

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should return to approximately same x position
    assert!(cursor.preferred_x.is_some());
    assert!((cursor.preferred_x.unwrap() - px).abs() < 0.1);
}

#[test]
fn test_softwrap_affinity_preserved_on_vertical_move() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);

    let mut cursor = TextCursor::at_with_affinity(2, CursorAffinity::Right);
    let mut selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.affinity, CursorAffinity::Right);

    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

// ============================================================================
// Hit-Test at Document Edges Tests
// ============================================================================

#[test]
fn test_hit_test_click_above_document() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Click above (negative y)
    hit_test_click(&mut cursor, &mut selection, &boundaries, 50.0, -10.0);

    // Should snap to first line
    assert_eq!(
        boundaries[find_boundary_index(&boundaries, cursor.pos)].line_index,
        0
    );
    assert_eq!(cursor.affinity, CursorAffinity::Left);
    assert!(selection.is_empty());
}

#[test]
fn test_hit_test_click_below_document() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click far below (y = 1000, well beyond single line)
    hit_test_click(&mut cursor, &mut selection, &boundaries, 50.0, 1000.0);

    // Should snap to last line
    let last_line = boundaries.last().map(|b| b.line_index).unwrap_or(0);
    assert_eq!(
        boundaries[find_boundary_index(&boundaries, cursor.pos)].line_index,
        last_line
    );
    assert_eq!(cursor.affinity, CursorAffinity::Left);
    assert!(selection.is_empty());
}

#[test]
fn test_hit_test_click_sets_affinity_left() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(0, CursorAffinity::Right);
    let mut selection = Selection::at(0);

    hit_test_click(&mut cursor, &mut selection, &boundaries, 50.0, 0.0);

    assert_eq!(cursor.affinity, CursorAffinity::Left); // Mouse click always Left
}

#[test]
fn test_hit_test_click_clears_selection() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 8;

    hit_test_click(&mut cursor, &mut selection, &boundaries, 50.0, 0.0);

    assert!(selection.is_empty());
    assert_eq!(selection.anchor, selection.head);
}

// ============================================================================
// Multiline Paragraph Tests
// ============================================================================

#[test]
fn test_multiline_up_down_preserves_preferred_x() {
    let text = "hello\nworld\ntest!";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'l' in "hello" (x = 30)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should be on "world" line, preferred_x preserved
    assert!(cursor.preferred_x.is_some());
    let idx = find_boundary_index(&boundaries, cursor.pos);
    assert_eq!(boundaries[idx].line_index, 1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should be on "test!" line
    let idx2 = find_boundary_index(&boundaries, cursor.pos);
    assert_eq!(boundaries[idx2].line_index, 2);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Back to "world" line
    let idx3 = find_boundary_index(&boundaries, cursor.pos);
    assert_eq!(boundaries[idx3].line_index, 1);
}

#[test]
fn test_multiline_shift_up_down_extends_selection() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'o' in "hello" (position 4)
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Selection should extend from hello to world
    assert_eq!(selection.anchor, 4);
    let idx = find_boundary_index(&boundaries, cursor.pos);
    assert_eq!(boundaries[idx].line_index, 1);
    assert_eq!(selection.head, cursor.pos);
}

// ============================================================================
// Empty Boundaries Tests
// ============================================================================

#[test]
fn test_ctrl_up_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should not panic
    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_down_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should not panic
    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_move_up_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should not panic
    move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_move_down_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should not panic
    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// Selection Collapse Across Paragraph Tests
// ============================================================================

#[test]
fn test_selection_collapse_left_uses_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Simulate selection from 3 to 8
    let mut cursor = TextCursor::at_with_affinity(8, CursorAffinity::Left);
    let mut selection = Selection::new();
    selection.anchor = 3;
    selection.head = 8;

    // Left-collapse should go to start
    use halcyon::core::editing::move_left_collapse;
    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 3); // selection.start()
    assert!(selection.is_empty());
    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

#[test]
fn test_selection_collapse_right_uses_end() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Simulate selection from 3 to 8
    let mut cursor = TextCursor::at_with_affinity(3, CursorAffinity::Right);
    let mut selection = Selection::new();
    selection.anchor = 3;
    selection.head = 8;

    // Right-collapse should go to end
    use halcyon::core::editing::move_right_collapse;
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 8); // selection.end()
    assert!(selection.is_empty());
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

// ============================================================================
// Paragraph Boundary Detection Tests
// ============================================================================

#[test]
fn test_find_paragraph_boundary_single_paragraph() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let pb = find_paragraph_boundary(&boundaries, 5);

    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, text.len());
}

#[test]
fn test_find_paragraph_boundary_at_start() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let pb = find_paragraph_boundary(&boundaries, 0);

    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, text.len());
}

#[test]
fn test_find_paragraph_boundary_at_end() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let pb = find_paragraph_boundary(&boundaries, text.len());

    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, text.len());
}

// ============================================================================
// UTF-8 / Ligature Safety Tests
// ============================================================================

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

#[test]
fn test_ctrl_up_utf8_cyrillic() {
    let text = "Привет мир"; // 19 bytes
    let boundaries = make_utf8_boundaries(text);

    let mut cursor = TextCursor::at(6); // After "При"
    let mut selection = Selection::at(6);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_ctrl_down_utf8_emoji() {
    let text = "a👍b"; // 'a' at 0, '👍' at 1-5, 'b' at 5
    let boundaries = make_utf8_boundaries(text);

    let mut cursor = TextCursor::at(1); // After 'a'
    let mut selection = Selection::at(1);

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len());
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_vertical_move_utf8_stays_at_valid_boundary() {
    let text = "café"; // 5 bytes, 4 clusters
    let boundaries = make_utf8_boundaries(text);

    let mut cursor = TextCursor::at(3); // After 'caf'
    let mut selection = Selection::at(3);

    // These should not crash or produce invalid positions
    move_up(&mut cursor, &mut selection, &boundaries);
    assert!(text.is_char_boundary(cursor.pos));

    cursor.pos = 3;
    move_down(&mut cursor, &mut selection, &boundaries);
    assert!(text.is_char_boundary(cursor.pos));
}

// ============================================================================
// Simulated Ligature Tests
// ============================================================================

/// Create boundaries with a simulated ligature (multiple bytes as one cluster).
fn make_ligature_boundaries() -> (String, Vec<ClusterBoundary>) {
    // Simulate "ffi" ligature as one cluster spanning 3 bytes
    let text = String::from("affib");

    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1, // 'a'
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 4, // "ffi" ligature
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5, // 'b'
            line_index: 0,
            x_logical: 20.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 5,
            byte_end: 5,
            line_index: 0,
            x_logical: 30.0,
        },
    ];

    (text, boundaries)
}

#[test]
fn test_ctrl_up_with_ligature() {
    let (text, boundaries) = make_ligature_boundaries();

    let mut cursor = TextCursor::at(4); // After ligature
    let mut selection = Selection::at(4);

    ctrl_move_up(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 0);
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_ctrl_down_with_ligature() {
    let (text, boundaries) = make_ligature_boundaries();

    let mut cursor = TextCursor::at(1); // After 'a'
    let mut selection = Selection::at(1);

    ctrl_move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, text.len());
    assert!(text.is_char_boundary(cursor.pos));
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Find the boundary index for a given byte position.
fn find_boundary_index(boundaries: &[ClusterBoundary], pos: usize) -> usize {
    boundaries
        .iter()
        .position(|b| b.byte_start == pos)
        .unwrap_or(0)
}
