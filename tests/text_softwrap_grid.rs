// SPEC-HAL-052 Stage 15: Soft-Wrap Cursor Grid Tests

use halcyon::core::editing::{
    ClusterBoundary, CursorAffinity, Selection, SoftWrapGrid, TextCursor, move_down,
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

/// Create boundaries for soft-wrapped text simulation.
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

/// Create boundaries with variable widths to simulate mixed-width clusters.
fn make_variable_width_boundaries() -> (String, Vec<ClusterBoundary>) {
    // "abc" where 'a'=10px, 'b'=20px, 'c'=15px
    let text = String::from("abc");
    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 2,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 30.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 3,
            byte_end: 3,
            line_index: 0,
            x_logical: 45.0,
        },
    ];
    (text, boundaries)
}

/// Create boundaries with a simulated ligature (multiple bytes as one cluster).
fn make_ligature_boundaries() -> (String, Vec<ClusterBoundary>) {
    // "affib" with "ffi" as a ligature
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

/// Create multi-line boundaries with ligatures.
fn make_multiline_ligature_boundaries() -> (String, Vec<ClusterBoundary>) {
    // "affi\nbcde" with "ffi" as a ligature on line 0
    let text = String::from("affi\nbcde");

    let boundaries = vec![
        // Line 0
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
            byte_end: 5, // '\n'
            line_index: 0,
            x_logical: 20.0,
        },
        // Line 1
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6, // 'b'
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7, // 'c'
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8, // 'd'
            line_index: 1,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 8,
            byte_end: 9, // 'e'
            line_index: 1,
            x_logical: 30.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 9,
            byte_end: 9,
            line_index: 1,
            x_logical: 40.0,
        },
    ];

    (text, boundaries)
}

// ============================================================================
// Grid Building Tests
// ============================================================================

#[test]
fn test_grid_single_line() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 1);
    assert_eq!(grid.line_start(0), Some(0));
    assert_eq!(grid.line_end(0), Some(5)); // Includes sentinel
}

#[test]
fn test_grid_multiple_lines() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 2);
    assert_eq!(grid.line_start(0), Some(0));
    assert_eq!(grid.line_end(0), Some(5)); // '\n' at index 5
    assert_eq!(grid.line_start(1), Some(6));
    assert_eq!(grid.line_end(1), Some(11)); // Sentinel at index 11
}

#[test]
fn test_grid_three_lines() {
    let text = "one\ntwo\nthree";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 3);

    // Line 0: "one\n" (indices 0-3)
    assert_eq!(grid.line_start(0), Some(0));

    // Line 1: "two\n" (indices 4-7)
    assert_eq!(grid.line_start(1), Some(4));

    // Line 2: "three" + sentinel
    assert_eq!(grid.line_start(2), Some(8));
}

#[test]
fn test_grid_soft_wrap() {
    // "hello world" wrapped at 5 chars
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Should have 3 visual lines: "hello", " worl", "d"
    assert_eq!(grid.line_count(), 3);
}

#[test]
fn test_grid_with_ligature() {
    let (_, boundaries) = make_ligature_boundaries();
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 1);
    assert_eq!(grid.line_start(0), Some(0));
    assert_eq!(grid.line_end(0), Some(3)); // Sentinel
}

#[test]
fn test_grid_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 0);
    assert_eq!(grid.line_start(0), None);
    assert_eq!(grid.line_end(0), None);
}

#[test]
fn test_grid_sentinel_on_last_line() {
    let text = "abc";
    let boundaries = make_ascii_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Sentinel should be on last line
    let sentinel_idx = boundaries.len() - 1;
    let line = grid.find_visual_line_for_index(sentinel_idx);
    assert_eq!(line, 0);
    assert_eq!(grid.line_end(0), Some(sentinel_idx));
}

// ============================================================================
// find_visual_line_for_index Tests
// ============================================================================

#[test]
fn test_find_visual_line_single_line() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    for i in 0..boundaries.len() {
        assert_eq!(grid.find_visual_line_for_index(i), 0);
    }
}

#[test]
fn test_find_visual_line_multiline() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Line 0: indices 0-5 (h, e, l, l, o, \n)
    for i in 0..=5 {
        assert_eq!(grid.find_visual_line_for_index(i), 0);
    }

    // Line 1: indices 6-11 (w, o, r, l, d, sentinel)
    for i in 6..boundaries.len() {
        assert_eq!(grid.find_visual_line_for_index(i), 1);
    }
}

#[test]
fn test_find_visual_line_soft_wrapped() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Line 0: "hello" (indices 0-4)
    for i in 0..5 {
        assert_eq!(grid.find_visual_line_for_index(i), 0);
    }

    // Line 1: " worl" (indices 5-9)
    for i in 5..10 {
        assert_eq!(grid.find_visual_line_for_index(i), 1);
    }

    // Line 2: "d" + sentinel
    assert_eq!(grid.find_visual_line_for_index(10), 2);
}

// ============================================================================
// find_best_match Tests
// ============================================================================

#[test]
fn test_find_best_match_exact() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Find best match on line 1 at x=20.0 (should be 'r' at x=20)
    let idx = grid.find_best_match(&boundaries, 1, 20.0);
    assert!(idx.is_some());
    assert_eq!(boundaries[idx.unwrap()].x_logical, 20.0);
}

#[test]
fn test_find_best_match_closest() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Find best match on line 1 at x=25.0 (between 'r' at 20 and 'l' at 30)
    let idx = grid.find_best_match(&boundaries, 1, 25.0);
    assert!(idx.is_some());
    // Should be closer to 'r' (20) or 'l' (30) - 'r' is closer
    let x = boundaries[idx.unwrap()].x_logical;
    assert!(x == 20.0 || x == 30.0);
}

#[test]
fn test_find_best_match_out_of_range() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Line 5 doesn't exist
    assert_eq!(grid.find_best_match(&boundaries, 5, 10.0), None);
}

#[test]
fn test_find_best_match_variable_width() {
    let (_, boundaries) = make_variable_width_boundaries();
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // x=5.0 should match 'a' (at 0.0), not 'b' (at 10.0)
    let idx = grid.find_best_match(&boundaries, 0, 5.0);
    assert!(idx.is_some());
    assert_eq!(boundaries[idx.unwrap()].byte_start, 0);

    // x=15.0 should match 'b' (at 10.0), not 'c' (at 30.0)
    let idx = grid.find_best_match(&boundaries, 0, 15.0);
    assert!(idx.is_some());
    assert_eq!(boundaries[idx.unwrap()].byte_start, 1);
}

// ============================================================================
// Up/Down Movement Tests
// ============================================================================

#[test]
fn test_move_up_at_first_line() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_up(&mut cursor, &mut selection, &boundaries);

    // At first line, should move to start of line
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_move_down_at_last_line() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    move_down(&mut cursor, &mut selection, &boundaries);

    // At last line, cursor stays, selection cleared
    assert_eq!(cursor.pos, 3);
    assert!(selection.is_empty());
}

#[test]
fn test_move_up_multiline() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'o' in "world" (byte 8, x=20)
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should move to line 0, matching x position
    // 'l' in "hello" is at byte 3, x=30.0 - wait, x=20.0 should map to 'l' at byte 2
    assert!(cursor.pos < 6); // On first line
    assert!(selection.is_empty());
}

#[test]
fn test_move_down_multiline() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'e' in "hello" (byte 1, x=10)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should move to line 1, matching x position
    assert!(cursor.pos >= 6); // On second line
    assert!(selection.is_empty());
}

#[test]
fn test_move_up_preserves_preferred_x() {
    let text = "hello\nworld\nfoo!!";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'o' in "foo!!" (byte 14, x=20)
    let mut cursor = TextCursor::at(14);
    let mut selection = Selection::at(14);

    move_up(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be set
    assert!(cursor.preferred_x.is_some());
    let px = cursor.preferred_x.unwrap();

    move_up(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be preserved
    assert_eq!(cursor.preferred_x, Some(px));
}

#[test]
fn test_move_down_preserves_preferred_x() {
    let text = "hello\nworld\nfoo!!";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'e' in "hello" (byte 1, x=10)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    assert!(cursor.preferred_x.is_some());
    let px = cursor.preferred_x.unwrap();

    move_down(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.preferred_x, Some(px));
}

#[test]
fn test_move_up_clamps_at_top() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'e' in "hello" (byte 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should clamp to start of first line
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_move_down_clamps_at_bottom() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'o' in "world" (byte 8)
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should stay on last line
    assert_eq!(cursor.pos, 8);
}

// ============================================================================
// Selection Tests
// ============================================================================

#[test]
fn test_move_up_select_extends_selection() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'o' in "world" (byte 8)
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Selection should extend from original to new position
    assert_eq!(selection.anchor, 8);
    assert!(cursor.pos < 6); // On first line
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_move_down_select_extends_selection() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'e' in "hello" (byte 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Selection should extend from original to new position
    assert_eq!(selection.anchor, 1);
    assert!(cursor.pos >= 6); // On second line
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_move_up_select_from_existing_selection() {
    let text = "hello\nworld\ntest!";
    let boundaries = make_multiline_boundaries(text);

    // Start with existing selection
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::new();
    selection.anchor = 6;
    selection.head = 8;

    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should stay at 6
    assert_eq!(selection.anchor, 6);
    assert!(cursor.pos < 6); // Moved to first line
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_selection_no_anchor_corruption() {
    let text = "hello\nworld\ntest!";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    // Multiple movements
    move_down_select(&mut cursor, &mut selection, &boundaries);
    let anchor = selection.anchor;

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should not change
    assert_eq!(selection.anchor, anchor);
}

// ============================================================================
// Affinity Tests
// ============================================================================

#[test]
fn test_move_up_preserves_affinity() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(8, CursorAffinity::Right);
    let mut selection = Selection::at(8);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Affinity must be preserved
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

#[test]
fn test_move_down_preserves_affinity() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(1, CursorAffinity::Right);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Affinity must be preserved
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

#[test]
fn test_move_up_select_preserves_affinity() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(8, CursorAffinity::Left);
    let mut selection = Selection::at(8);

    move_up_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

#[test]
fn test_move_down_select_preserves_affinity() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at_with_affinity(1, CursorAffinity::Left);
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

// ============================================================================
// Soft-Wrap Movement Tests
// ============================================================================

#[test]
fn test_softwrap_move_down_within_paragraph() {
    // "hello world" wrapped at 5 chars: "hello" / " worl" / "d"
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Start at 'h' (line 0)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should move to line 1
    let new_line = grid.find_visual_line_for_index(
        boundaries
            .iter()
            .position(|b| b.byte_start == cursor.pos)
            .unwrap(),
    );
    assert_eq!(new_line, 1);
}

#[test]
fn test_softwrap_move_up_within_paragraph() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Start at ' ' after "hello" (line 1)
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should move to line 0
    let new_line = grid.find_visual_line_for_index(
        boundaries
            .iter()
            .position(|b| b.byte_start == cursor.pos)
            .unwrap(),
    );
    assert_eq!(new_line, 0);
}

#[test]
fn test_softwrap_preferred_x_across_lines() {
    let text = "hello world";
    let boundaries = make_softwrap_boundaries(text, 5);

    // Start at 'l' in "hello" (byte 2, x=20)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should set preferred_x
    assert!(cursor.preferred_x.is_some());
    let px = cursor.preferred_x.unwrap();

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should match original x
    assert!(cursor.preferred_x.is_some());
    // The returned position should have similar x
    let new_idx = boundaries
        .iter()
        .position(|b| b.byte_start == cursor.pos)
        .unwrap();
    let new_x = boundaries[new_idx].x_logical;
    assert!((new_x - px).abs() < 15.0); // Within reasonable tolerance
}

// ============================================================================
// Ligature Tests
// ============================================================================

#[test]
fn test_ligature_grid_building() {
    let (_, boundaries) = make_multiline_ligature_boundaries();
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Should have 2 lines
    assert_eq!(grid.line_count(), 2);

    // Line 0 should contain the ligature
    let line0_end = grid.line_end(0).unwrap();
    assert!(line0_end >= 2); // At least 'a', 'ffi', '\n'
}

#[test]
fn test_move_down_with_ligature() {
    let (_, boundaries) = make_multiline_ligature_boundaries();

    // Start at 'ffi' ligature (byte 1)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should move to line 1
    assert!(cursor.pos >= 5); // On second line
}

#[test]
fn test_move_up_with_ligature() {
    let (_, boundaries) = make_multiline_ligature_boundaries();

    // Start at 'c' (byte 6, line 1, x=10)
    let mut cursor = TextCursor::at(6);
    let mut selection = Selection::at(6);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should move to line 0
    assert!(cursor.pos < 5);
}

// ============================================================================
// UTF-8 Tests
// ============================================================================

/// Create boundaries for UTF-8 text.
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

/// Create multi-line UTF-8 boundaries.
fn make_utf8_multiline_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_logical = 0.0;

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, (byte_start, c)) in chars.iter().enumerate() {
        let byte_end = if i + 1 < chars.len() {
            chars[i + 1].0
        } else {
            text.len()
        };
        boundaries.push(ClusterBoundary {
            byte_start: *byte_start,
            byte_end,
            line_index,
            x_logical,
        });
        if *c == '\n' {
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

#[test]
fn test_utf8_grid_emoji() {
    let text = "a👍b";
    let boundaries = make_utf8_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 1);
    assert_eq!(grid.line_start(0), Some(0));
}

#[test]
fn test_utf8_multiline_cyrillic() {
    let text = "Привет\nМир";
    let boundaries = make_utf8_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    assert_eq!(grid.line_count(), 2);
}

#[test]
fn test_utf8_move_down() {
    let text = "Привет\nМир";
    let boundaries = make_utf8_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0); // 'П'
    let mut selection = Selection::at(0);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should be on second line
    assert!(cursor.pos > 12); // "Привет\n" is 13 bytes
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_utf8_move_up() {
    let text = "Привет\nМир";
    let boundaries = make_utf8_multiline_boundaries(text);

    let mut cursor = TextCursor::at(13); // 'М' on line 1
    let mut selection = Selection::at(13);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should be on first line
    assert!(cursor.pos < 13);
    assert!(text.is_char_boundary(cursor.pos));
}

// ============================================================================
// Regression Tests - Single Line Behavior
// ============================================================================

#[test]
fn test_regression_single_line_move_up() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Same as before: at first line, move to start
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_regression_single_line_move_down() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Same as before: at last line, no movement
    assert_eq!(cursor.pos, 5);
    assert!(selection.is_empty());
}

#[test]
fn test_regression_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should not panic
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_regression_multiline_behavior() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);

    // Move from line 1 to line 0
    let mut cursor = TextCursor::at(8); // 'r' in world
    let mut selection = Selection::at(8);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should be on first line with matching x
    assert!(cursor.pos < 6);

    // Move back
    move_down(&mut cursor, &mut selection, &boundaries);

    // Should be back on second line
    assert!(cursor.pos >= 6);
}

// ============================================================================
// Grid Invariant Tests
// ============================================================================

#[test]
fn test_grid_never_creates_invalid_indices() {
    let text = "hello\nworld\ntest!";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    for line in 0..grid.line_count() {
        let start = grid.line_start(line).unwrap();
        let end = grid.line_end(line).unwrap();

        assert!(start < boundaries.len());
        assert!(end < boundaries.len());
        assert!(start <= end);
    }
}

#[test]
fn test_grid_includes_sentinel_on_last_line() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    let last_line = grid.line_count() - 1;
    let end = grid.line_end(last_line).unwrap();

    // End should be the sentinel index
    assert_eq!(end, boundaries.len() - 1);
}

#[test]
fn test_grid_does_not_reorder_boundaries() {
    let text = "hello\nworld";
    let boundaries = make_multiline_boundaries(text);
    let grid = SoftWrapGrid::from_boundaries(&boundaries);

    // Check that all line starts are in increasing order
    for line in 1..grid.line_count() {
        let prev_end = grid.line_end(line - 1).unwrap();
        let cur_start = grid.line_start(line).unwrap();
        assert!(cur_start > prev_end);
    }
}
