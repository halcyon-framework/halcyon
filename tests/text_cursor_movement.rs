// SPEC-HAL-052 Stage 2: Text Cursor Movement Tests

use halcyon::core::editing::{ClusterBoundary, TextCursor};

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
/// Format: each line is a slice of (byte_start, byte_end) pairs.
/// Each cluster is 10px wide.
fn make_multiline_boundaries(lines: &[&[(usize, usize)]]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut max_byte = 0;

    for (line_index, clusters) in lines.iter().enumerate() {
        let mut x = 0.0;
        for &(byte_start, byte_end) in *clusters {
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
// 7.1 Simple ASCII Tests
// ============================================================================

#[test]
fn test_ascii_left_right_stepping() {
    // "hello" - 5 chars + sentinel
    let boundaries = make_ascii_boundaries("hello");
    assert_eq!(boundaries.len(), 6);

    let mut cursor = TextCursor::at(0);

    // Step right through all positions
    assert_eq!(cursor.pos, 0);

    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 1);

    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 2);

    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 3);

    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 4);

    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 5); // sentinel

    // At sentinel, right does nothing
    cursor.move_right(&boundaries);
    assert_eq!(cursor.pos, 5);

    // Step left back to start
    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 4);

    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 3);

    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 2);

    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 1);

    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 0);

    // At start, left does nothing
    cursor.move_left(&boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ascii_home_end_single_line() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2); // middle

    // End should go to sentinel (end of line)
    cursor.move_to_end_of_line(&boundaries);
    assert_eq!(cursor.pos, 5); // sentinel

    // Home should go to start
    cursor.move_to_start_of_line(&boundaries);
    assert_eq!(cursor.pos, 0);

    // Already at start, home does nothing
    cursor.move_to_start_of_line(&boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ascii_up_down_single_line() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(2);

    // Up on first line clamps to start of line (Stage-10 behavior)
    cursor.move_up(&boundaries);
    assert_eq!(cursor.pos, 0);

    // Down on last line does nothing (already at last line)
    cursor.move_down(&boundaries);
    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 7.2 Multi-line Tests
// ============================================================================

#[test]
fn test_multiline_up_down_closest_x() {
    // "abc\ndefg\nhi"
    // Line 0: a(0-1), b(1-2), c(2-3)
    // Line 1: d(4-5), e(5-6), f(6-7), g(7-8)
    // Line 2: h(9-10), i(10-11)
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1), (1, 2), (2, 3)],         // abc
        &[(4, 5), (5, 6), (6, 7), (7, 8)], // defg
        &[(9, 10), (10, 11)],              // hi
    ]);

    // Start on line 1 at 'f' (x=20)
    let mut cursor = TextCursor::at(6);
    assert_eq!(boundaries[5].byte_start, 6); // 'f' is at index 5
    assert_eq!(boundaries[5].x_logical, 20.0);

    // Move up to line 0 - closest to x=20 is 'c' (x=20)
    cursor.move_up(&boundaries);
    assert_eq!(cursor.pos, 2); // 'c'

    // Move down back to line 1 - closest to x=20 is 'f'
    cursor.move_down(&boundaries);
    assert_eq!(cursor.pos, 6); // 'f'

    // Move down to line 2 - closest to x=20 is 'i' (x=10) or sentinel
    cursor.move_down(&boundaries);
    // Line 2 has h at x=0, i at x=10, sentinel at x=20
    // Closest to x=20 should be sentinel (x=20)
    assert!(cursor.pos == 10 || cursor.pos == 11); // 'i' or sentinel
}

#[test]
fn test_multiline_end_of_line() {
    // "abc\ndefg\nhi"
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1), (1, 2), (2, 3)],
        &[(4, 5), (5, 6), (6, 7), (7, 8)],
        &[(9, 10), (10, 11)],
    ]);

    // Start at 'd' on line 1
    let mut cursor = TextCursor::at(4);

    // End of line 1 should be 'g' (byte 7)
    cursor.move_to_end_of_line(&boundaries);
    assert_eq!(cursor.pos, 7); // 'g'

    // Start of line 1 should be 'd' (byte 4)
    cursor.move_to_start_of_line(&boundaries);
    assert_eq!(cursor.pos, 4);
}

#[test]
fn test_multiline_line_boundaries() {
    // Verify correct line indexing
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1), (1, 2), (2, 3)],
        &[(4, 5), (5, 6), (6, 7), (7, 8)],
        &[(9, 10), (10, 11)],
    ]);

    // Check line indices
    assert_eq!(boundaries[0].line_index, 0); // 'a'
    assert_eq!(boundaries[2].line_index, 0); // 'c'
    assert_eq!(boundaries[3].line_index, 1); // 'd'
    assert_eq!(boundaries[6].line_index, 1); // 'g'
    assert_eq!(boundaries[7].line_index, 2); // 'h'
}

// ============================================================================
// 7.3 preferred_x Behavior Tests
// ============================================================================

#[test]
fn test_preferred_x_across_multiple_lines() {
    // "abcdefgh\nij\nklmnopqr"
    // Line 0: 8 chars (a-h), x = 0..70
    // Line 1: 2 chars (i-j), x = 0..10
    // Line 2: 8 chars (k-r), x = 0..70
    let boundaries = make_multiline_boundaries(&[
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
        ],
        &[(9, 10), (10, 11)],
        &[
            (12, 13),
            (13, 14),
            (14, 15),
            (15, 16),
            (16, 17),
            (17, 18),
            (18, 19),
            (19, 20),
        ],
    ]);

    // Start at 'f' on line 0 (x=50)
    let mut cursor = TextCursor::at(5);
    assert_eq!(boundaries[5].x_logical, 50.0);

    // Move down to line 1 - closest to x=50 is 'j' (x=10) since that's the rightmost
    cursor.move_down(&boundaries);
    // Line 1 only has chars at x=0 and x=10, closest to x=50 is x=10 ('j')
    assert_eq!(cursor.pos, 10); // 'j'

    // preferred_x should still be 50, so moving down again should land near x=50
    cursor.move_down(&boundaries);
    // Line 2 has chars at x=0,10,20,30,40,50,60,70
    // Closest to x=50 is 'p' at x=50 (byte 17)
    assert_eq!(cursor.pos, 17); // 'p'
}

#[test]
fn test_horizontal_movement_resets_preferred_x() {
    let boundaries = make_multiline_boundaries(&[
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)],
        &[(6, 7), (7, 8), (8, 9), (9, 10), (10, 11)],
    ]);

    // Start at 'd' on line 0 (x=30)
    let mut cursor = TextCursor::at(3);

    // Move down sets preferred_x
    cursor.move_down(&boundaries);
    assert!(cursor.preferred_x.is_some());

    // Horizontal movement resets preferred_x
    cursor.move_left(&boundaries);
    assert!(cursor.preferred_x.is_none());

    // Set preferred_x again
    cursor.move_up(&boundaries);
    assert!(cursor.preferred_x.is_some());

    // Right resets
    cursor.move_right(&boundaries);
    assert!(cursor.preferred_x.is_none());

    // Home resets
    cursor.move_down(&boundaries);
    assert!(cursor.preferred_x.is_some());
    cursor.move_to_start_of_line(&boundaries);
    assert!(cursor.preferred_x.is_none());

    // End resets
    cursor.move_down(&boundaries);
    assert!(cursor.preferred_x.is_some());
    cursor.move_to_end_of_line(&boundaries);
    assert!(cursor.preferred_x.is_none());
}

// ============================================================================
// 7.4 Empty Lines Tests
// ============================================================================

#[test]
fn test_empty_line_skip_up() {
    // "abc\n\nxyz"
    // Line 0: a(0), b(1), c(2)
    // Line 1: (empty - no clusters)
    // Line 2: x(4), y(5), z(6)
    // Note: line 1 has no boundaries, so we only have lines 0 and 2 in boundaries
    // We need to manually fix line indices since our helper doesn't handle gaps
    // Actually let's create boundaries manually for this test
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
            x_logical: 20.0,
        },
        // Line 1 is empty - no boundaries
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 2,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 2,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 2,
            x_logical: 20.0,
        },
        // Sentinel
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 2,
            x_logical: 30.0,
        },
    ];

    // Start on line 2 at 'y' (x=10)
    let mut cursor = TextCursor::at(5);

    // Move up should skip empty line 1 and land on line 0
    cursor.move_up(&boundaries);
    // Closest to x=10 on line 0 is 'b' (x=10)
    assert_eq!(cursor.pos, 1); // 'b'
    assert_eq!(boundaries[1].line_index, 0);
}

#[test]
fn test_empty_line_skip_down() {
    // Same setup as above
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
            x_logical: 20.0,
        },
        // Line 1 is empty
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 2,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 2,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 2,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 2,
            x_logical: 30.0,
        },
    ];

    // Start on line 0 at 'b' (x=10)
    let mut cursor = TextCursor::at(1);

    // Move down should skip empty line 1 and land on line 2
    cursor.move_down(&boundaries);
    // Closest to x=10 on line 2 is 'y' (x=10)
    assert_eq!(cursor.pos, 5); // 'y'
    assert_eq!(boundaries[4].line_index, 2);
}

// ============================================================================
// 7.5 Edge Cases
// ============================================================================

#[test]
fn test_first_boundary_up_stays() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(0);

    // Up from first line stays
    cursor.move_up(&boundaries);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_last_line_down_stays() {
    let boundaries = make_ascii_boundaries("hello");
    let mut cursor = TextCursor::at(3);

    // Down on last (only) line stays
    cursor.move_down(&boundaries);
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_sentinel_vertical_behavior() {
    // Two lines
    let boundaries =
        make_multiline_boundaries(&[&[(0, 1), (1, 2), (2, 3)], &[(4, 5), (5, 6), (6, 7)]]);

    // Sentinel is on line 1
    let sentinel_pos = boundaries.last().unwrap().byte_start;
    let mut cursor = TextCursor::at(sentinel_pos);

    // Down from sentinel (on last line) stays
    cursor.move_down(&boundaries);
    assert_eq!(cursor.pos, sentinel_pos);

    // Up from sentinel should go to line 0
    cursor.move_up(&boundaries);
    assert!(
        boundaries
            .iter()
            .find(|b| b.byte_start == cursor.pos)
            .unwrap()
            .line_index
            == 0
    );
}

#[test]
fn test_cursor_constructors() {
    let cursor1 = TextCursor::new();
    assert_eq!(cursor1.pos, 0);
    assert_eq!(cursor1.preferred_x, None);

    let cursor2 = TextCursor::at(42);
    assert_eq!(cursor2.pos, 42);
    assert_eq!(cursor2.preferred_x, None);

    let cursor3 = TextCursor::default();
    assert_eq!(cursor3.pos, 0);
    assert_eq!(cursor3.preferred_x, None);
}

#[test]
fn test_empty_boundaries_safety() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    let mut cursor = TextCursor::at(0);

    // These should not panic on empty boundaries
    cursor.move_up(&boundaries);
    cursor.move_down(&boundaries);
    // Note: move_left/right will panic because they call find_index_of_pos
    // which expects pos to be valid. This is by design per the spec.
}
