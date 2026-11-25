// SPEC-HAL-052 Stage 10: Vertical Navigation Tests

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, find_best_x_match, find_cluster_index, move_down,
    move_down_select, move_up, move_up_select,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for two visual lines of ASCII text.
/// Line 0: chars 0..len0
/// Line 1: chars len0..len0+len1
fn make_two_line_boundaries(line0: &str, line1: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();

    // Line 0
    for (i, _) in line0.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + 1,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }

    // Line 1
    let offset = line0.len();
    for (i, _) in line1.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: offset + i,
            byte_end: offset + i + 1,
            line_index: 1,
            x_logical: i as f32 * 10.0,
        });
    }

    // Sentinel on last line
    let total_len = line0.len() + line1.len();
    boundaries.push(ClusterBoundary {
        byte_start: total_len,
        byte_end: total_len,
        line_index: 1,
        x_logical: line1.len() as f32 * 10.0,
    });

    boundaries
}

/// Create boundaries with variable x_logical positions per line.
/// Each line is a slice of (byte_start, x_logical) pairs.
fn make_variable_x_boundaries(lines: &[&[(usize, f32)]]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut last_byte_start = 0;
    let mut last_line_max_x = 0.0f32;

    for (line_idx, line) in lines.iter().enumerate() {
        let mut line_max_x = 0.0f32;
        for &(byte_start, x_logical) in *line {
            boundaries.push(ClusterBoundary {
                byte_start,
                byte_end: byte_start + 1,
                line_index: line_idx,
                x_logical,
            });
            if byte_start >= last_byte_start {
                last_byte_start = byte_start + 1;
            }
            if x_logical > line_max_x {
                line_max_x = x_logical;
            }
        }
        last_line_max_x = line_max_x;
    }

    // Sentinel - positioned after the last cluster on the last line
    let last_line_idx = if lines.is_empty() { 0 } else { lines.len() - 1 };
    boundaries.push(ClusterBoundary {
        byte_start: last_byte_start,
        byte_end: last_byte_start,
        line_index: last_line_idx,
        x_logical: last_line_max_x + 10.0, // After last cluster
    });

    boundaries
}

/// Create UTF-8 boundaries for multi-line text.
fn make_utf8_two_line_boundaries(line0: &str, line1: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();

    // Line 0
    let chars0: Vec<(usize, char)> = line0.char_indices().collect();
    for (i, (byte_start, _)) in chars0.iter().enumerate() {
        let byte_end = if i + 1 < chars0.len() {
            chars0[i + 1].0
        } else {
            line0.len()
        };
        boundaries.push(ClusterBoundary {
            byte_start: *byte_start,
            byte_end,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }

    // Line 1
    let offset = line0.len();
    let chars1: Vec<(usize, char)> = line1.char_indices().collect();
    for (i, (byte_start, _)) in chars1.iter().enumerate() {
        let byte_end = if i + 1 < chars1.len() {
            chars1[i + 1].0
        } else {
            line1.len()
        };
        boundaries.push(ClusterBoundary {
            byte_start: offset + *byte_start,
            byte_end: offset + byte_end,
            line_index: 1,
            x_logical: i as f32 * 10.0,
        });
    }

    // Sentinel
    let total_len = line0.len() + line1.len();
    boundaries.push(ClusterBoundary {
        byte_start: total_len,
        byte_end: total_len,
        line_index: 1,
        x_logical: line1.chars().count() as f32 * 10.0,
    });

    boundaries
}

// ============================================================================
// 10.1 Helper Tests
// ============================================================================

#[test]
fn test_find_cluster_index_basic() {
    let boundaries = make_two_line_boundaries("abc", "def");

    assert_eq!(find_cluster_index(&boundaries, 0), 0);
    assert_eq!(find_cluster_index(&boundaries, 1), 1);
    assert_eq!(find_cluster_index(&boundaries, 2), 2);
    assert_eq!(find_cluster_index(&boundaries, 3), 3); // first of line 1
    assert_eq!(find_cluster_index(&boundaries, 6), 6); // sentinel
}

#[test]
fn test_find_cluster_index_not_found() {
    let boundaries = make_two_line_boundaries("abc", "def");

    // Position 99 doesn't exist - should return 0 (no panic)
    assert_eq!(find_cluster_index(&boundaries, 99), 0);
}

#[test]
fn test_find_cluster_index_empty() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    assert_eq!(find_cluster_index(&boundaries, 0), 0);
}

#[test]
fn test_find_best_x_match_basic() {
    let boundaries = make_two_line_boundaries("abc", "def");

    // Line 0 has x_logical: 0, 10, 20
    // Find closest to x=15 -> should be 10 (byte 1) or 20 (byte 2)
    let pos = find_best_x_match(&boundaries, 0, 15.0);
    assert!(pos == 1 || pos == 2);

    // Find closest to x=0 -> byte 0
    assert_eq!(find_best_x_match(&boundaries, 0, 0.0), 0);

    // Find closest to x=25 -> byte 2 (x=20)
    assert_eq!(find_best_x_match(&boundaries, 0, 25.0), 2);
}

#[test]
fn test_find_best_x_match_empty_line() {
    // Line 1 has no clusters in this setup
    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 1,
            line_index: 0,
            x_logical: 10.0,
        },
    ];

    // Line 1 has no clusters - returns 0 (default)
    assert_eq!(find_best_x_match(&boundaries, 1, 5.0), 0);
}

// ============================================================================
// 10.2 Basic Vertical Movement
// ============================================================================

#[test]
fn test_move_down_basic() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1); // 'b' on line 0, x=10
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Should be on line 1, closest to x=10 -> byte 4 ('e')
    assert_eq!(cursor.pos, 4);
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_move_up_basic() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4); // 'e' on line 1, x=10
    let mut selection = Selection::at(4);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should be on line 0, closest to x=10 -> byte 1 ('b')
    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());
}

#[test]
fn test_move_up_at_top_clamp() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1); // 'b' on line 0
    let mut selection = Selection::at(1);

    move_up(&mut cursor, &mut selection, &boundaries);

    // At first line - should clamp to start of line 0 (byte 0)
    assert_eq!(cursor.pos, 0);
    assert!(selection.is_empty());
}

#[test]
fn test_move_down_at_bottom_clamp() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4); // 'e' on line 1 (last line)
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);

    // At last line - no-op on position
    assert_eq!(cursor.pos, 4);
    assert!(selection.is_empty());
}

#[test]
fn test_move_up_then_down_roundtrip() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4); // 'e' on line 1
    let mut selection = Selection::at(4);

    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 1); // 'b' on line 0

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 4); // Back to 'e' on line 1
}

// ============================================================================
// 10.3 preferred_x Retention
// ============================================================================

#[test]
fn test_preferred_x_set_on_first_vertical() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(2); // 'c' on line 0, x=20
    let mut selection = Selection::at(2);

    assert!(cursor.preferred_x.is_none());

    move_down(&mut cursor, &mut selection, &boundaries);

    // preferred_x should be set to 20.0
    assert_eq!(cursor.preferred_x, Some(20.0));
    // Should land on 'f' (byte 5, x=20)
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_preferred_x_retained_across_moves() {
    // Line 0: 5 chars (0-4), Line 1: 3 chars (5-7)
    let boundaries = make_two_line_boundaries("abcde", "xyz");

    let mut cursor = TextCursor::at(4); // 'e' on line 0, x=40
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 ends at sentinel (byte 8, x=30), which is closest to preferred_x=40
    // (sentinel at x=30 is closer to 40 than 'z' at x=20)
    assert_eq!(cursor.pos, 8);
    assert_eq!(cursor.preferred_x, Some(40.0));

    move_up(&mut cursor, &mut selection, &boundaries);

    // Back to line 0, preferred_x=40 -> should return to byte 4 ('e')
    assert_eq!(cursor.pos, 4);
    assert_eq!(cursor.preferred_x, Some(40.0));
}

#[test]
fn test_preferred_x_preserved_after_line_0_clamp() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1); // 'b' on line 0, x=10
    cursor.preferred_x = Some(10.0);
    let mut selection = Selection::at(1);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Clamped to start of line 0
    assert_eq!(cursor.pos, 0);
    // SPEC-HAL-107 §7.4: preferred_x is preserved at edge, NOT reset to 0
    assert_eq!(cursor.preferred_x, Some(10.0));
}

// ============================================================================
// 10.4 Shift Variants & Anchor Behavior
// ============================================================================

#[test]
fn test_shift_up_creates_selection() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4); // 'e' on line 1
    let mut selection = Selection::at(4); // Empty selection

    move_up_select(&mut cursor, &mut selection, &boundaries);

    // Anchor should be frozen at original position (4)
    assert_eq!(selection.anchor, 4);
    // Head moves with cursor
    assert_eq!(selection.head, cursor.pos);
    assert_eq!(cursor.pos, 1); // 'b' on line 0
    assert!(!selection.is_empty());
}

#[test]
fn test_shift_down_creates_selection() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1); // 'b' on line 0
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(selection.anchor, 1);
    assert_eq!(selection.head, 4); // 'e' on line 1
    assert_eq!(cursor.pos, 4);
}

#[test]
fn test_shift_up_twice_anchor_stays() {
    // Three lines for this test
    let mut boundaries = Vec::new();
    for i in 0..3 {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + 1,
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }
    for i in 0..3 {
        boundaries.push(ClusterBoundary {
            byte_start: 3 + i,
            byte_end: 4 + i,
            line_index: 1,
            x_logical: i as f32 * 10.0,
        });
    }
    for i in 0..3 {
        boundaries.push(ClusterBoundary {
            byte_start: 6 + i,
            byte_end: 7 + i,
            line_index: 2,
            x_logical: i as f32 * 10.0,
        });
    }
    boundaries.push(ClusterBoundary {
        byte_start: 9,
        byte_end: 9,
        line_index: 2,
        x_logical: 30.0,
    });

    let mut cursor = TextCursor::at(7); // Line 2, x=10
    let mut selection = Selection::at(7);

    // First Shift+Up
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 7);
    assert_eq!(cursor.pos, 4); // Line 1

    // Second Shift+Up - anchor should stay at 7
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 7);
    assert_eq!(cursor.pos, 1); // Line 0
}

#[test]
fn test_shift_down_then_up_anchor_stays() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1); // 'b' on line 0
    let mut selection = Selection::at(1);

    // Shift+Down
    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 1);
    assert_eq!(cursor.pos, 4);

    // Shift+Up - anchor stays, head moves back
    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(selection.anchor, 1);
    assert_eq!(cursor.pos, 1);
    // Selection should now be empty (anchor == head)
    assert!(selection.is_empty());
}

// ============================================================================
// 10.5 UTF-8 Visual Lines
// ============================================================================

#[test]
fn test_utf8_vertical_movement() {
    // Line 0: "Привет" (6 chars, 12 bytes)
    // Line 1: "мир" (3 chars, 6 bytes)
    let boundaries = make_utf8_two_line_boundaries("Привет", "мир");

    let mut cursor = TextCursor::at(4); // Third char on line 0, x=20
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 has chars at x=0,10,20 -> closest to x=20 is char at index 2
    // Line 1 starts at byte 12, third char is at byte 12 + 4 = 16
    assert_eq!(cursor.pos, 16);
}

#[test]
fn test_utf8_preferred_x_with_different_widths() {
    // Create boundaries with different cluster widths
    let boundaries = vec![
        // Line 0: wide char at x=0 (4 bytes)
        ClusterBoundary {
            byte_start: 0,
            byte_end: 4,
            line_index: 0,
            x_logical: 0.0,
        },
        // Line 0: narrow char at x=20 (1 byte)
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 0,
            x_logical: 20.0,
        },
        // Line 1: char at x=0 (2 bytes)
        ClusterBoundary {
            byte_start: 5,
            byte_end: 7,
            line_index: 1,
            x_logical: 0.0,
        },
        // Line 1: char at x=15 (2 bytes)
        ClusterBoundary {
            byte_start: 7,
            byte_end: 9,
            line_index: 1,
            x_logical: 15.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 9,
            byte_end: 9,
            line_index: 1,
            x_logical: 25.0,
        },
    ];

    let mut cursor = TextCursor::at(4); // x=20 on line 0
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Closest to x=20 on line 1 is x=15 (byte 7)
    assert_eq!(cursor.pos, 7);
    assert_eq!(cursor.preferred_x, Some(20.0));
}

// ============================================================================
// 10.6 Empty Selection Behavior
// ============================================================================

#[test]
fn test_move_up_clears_existing_selection() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5; // Non-empty selection

    move_up(&mut cursor, &mut selection, &boundaries);

    // Selection should be cleared
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, cursor.pos);
    assert_eq!(selection.head, cursor.pos);
}

#[test]
fn test_move_down_clears_existing_selection() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 3;

    move_down(&mut cursor, &mut selection, &boundaries);

    assert!(selection.is_empty());
    assert_eq!(selection.anchor, cursor.pos);
}

#[test]
fn test_clamp_at_last_line_clears_selection() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(4); // On last line
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5;

    move_down(&mut cursor, &mut selection, &boundaries);

    // Position unchanged, but selection cleared
    assert_eq!(cursor.pos, 4);
    assert!(selection.is_empty());
}

// ============================================================================
// 10.7 Multi-line with Uneven Line Widths
// ============================================================================

#[test]
fn test_uneven_line_widths_best_match() {
    // Line 0: chars at x = 0, 10, 20, 30, 40 (5 chars)
    // Line 1: chars at x = 0, 50 (2 chars, big gap)
    // Line 2: chars at x = 0, 10, 20 (3 chars), sentinel at x=30
    let boundaries = make_variable_x_boundaries(&[
        &[(0, 0.0), (1, 10.0), (2, 20.0), (3, 30.0), (4, 40.0)],
        &[(5, 0.0), (6, 50.0)],
        &[(7, 0.0), (8, 10.0), (9, 20.0)],
    ]);

    // Start at line 0, x=40 (byte 4)
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 has x=0 and x=50. Closest to 40 is 50 (byte 6)
    assert_eq!(cursor.pos, 6);
    assert_eq!(cursor.preferred_x, Some(40.0));

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 2 has x=0,10,20 plus sentinel at x=30
    // Closest to 40 is sentinel at x=30 (byte 10), distance 10
    // (vs byte 9 at x=20, distance 20)
    assert_eq!(cursor.pos, 10);
    assert_eq!(cursor.preferred_x, Some(40.0));
}

#[test]
fn test_extreme_left_position() {
    let boundaries = make_variable_x_boundaries(&[
        &[(0, 0.0), (1, 100.0), (2, 200.0)],
        &[(3, 50.0), (4, 150.0)],
    ]);

    let mut cursor = TextCursor::at(0); // x=0
    let mut selection = Selection::at(0);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 closest to x=0 is x=50 (byte 3)
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_extreme_right_position() {
    let boundaries =
        make_variable_x_boundaries(&[&[(0, 0.0), (1, 100.0), (2, 200.0)], &[(3, 50.0), (4, 80.0)]]);

    let mut cursor = TextCursor::at(2); // x=200
    let mut selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Line 1 has x=50, x=80, and sentinel at x=90
    // Closest to x=200 is sentinel at x=90 (byte 5), distance 110
    // (vs byte 4 at x=80, distance 120)
    assert_eq!(cursor.pos, 5);
}

// ============================================================================
// 10.8 Edge Cases
// ============================================================================

#[test]
fn test_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Should be no-ops
    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);

    move_up_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);

    move_down_select(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_single_line_up() {
    // Only one line
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
            byte_end: 2,
            line_index: 0,
            x_logical: 20.0,
        },
    ];

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Should clamp to start of line 0
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_single_line_down() {
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
            byte_end: 2,
            line_index: 0,
            x_logical: 20.0,
        },
    ];

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // Already at last line - no-op
    assert_eq!(cursor.pos, 1);
    assert!(selection.is_empty());
}

#[test]
fn test_cursor_at_sentinel() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(6); // Sentinel on line 1
    let mut selection = Selection::at(6);

    move_up(&mut cursor, &mut selection, &boundaries);

    // Sentinel is on line 1, x=30. Line 0 closest to x=30 is x=20 (byte 2)
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_invariants_after_vertical_move() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down(&mut cursor, &mut selection, &boundaries);

    // cursor.pos is a legal cluster boundary
    assert!(boundaries.iter().any(|b| b.byte_start == cursor.pos));

    // Selection is empty, anchor == head == cursor.pos
    assert!(selection.is_empty());
    assert_eq!(selection.anchor, cursor.pos);
    assert_eq!(selection.head, cursor.pos);

    // preferred_x is set
    assert!(cursor.preferred_x.is_some());
}

#[test]
fn test_invariants_after_select_move() {
    let boundaries = make_two_line_boundaries("abc", "def");

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    move_down_select(&mut cursor, &mut selection, &boundaries);

    // cursor.pos is a legal cluster boundary
    assert!(boundaries.iter().any(|b| b.byte_start == cursor.pos));

    // Selection head == cursor.pos
    assert_eq!(selection.head, cursor.pos);

    // Anchor is at original position
    assert_eq!(selection.anchor, 1);

    // preferred_x is set
    assert!(cursor.preferred_x.is_some());
}
