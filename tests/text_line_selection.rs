// SPEC-HAL-052 Stage 5: Line Selection Tests

use halcyon::core::editing::{
    ClusterBoundary, LineBoundary, find_line_boundary, line_drag_update, triple_click_selection,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for simple single-line ASCII text.
fn make_single_line_boundaries(text: &str) -> Vec<ClusterBoundary> {
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

/// Create boundaries for multi-line text with explicit line breaks.
/// Each line's content is specified separately.
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
    boundaries.push(ClusterBoundary {
        byte_start: byte_offset,
        byte_end: byte_offset,
        line_index: last_line_idx,
        x_logical: 0.0,
    });

    boundaries
}

/// Create boundaries for UTF-8 text, respecting char boundaries.
fn make_utf8_boundaries(text: &str, line_index: usize) -> Vec<ClusterBoundary> {
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
            line_index,
            x_logical: i as f32 * 10.0,
        });
    }
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: boundaries.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// 5.1 Single-Line Tests
// ============================================================================

#[test]
fn test_triple_click_single_line_start() {
    let text = "hello world";
    let boundaries = make_single_line_boundaries(text);

    // Triple-click at start (byte 0)
    let (selection, cursor) = triple_click_selection(&boundaries, 0);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 11); // "hello world" length
    assert_eq!(cursor.pos, 11);
}

#[test]
fn test_triple_click_single_line_middle() {
    let text = "hello world";
    let boundaries = make_single_line_boundaries(text);

    // Triple-click in middle (byte 5, the space)
    let (selection, cursor) = triple_click_selection(&boundaries, 5);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 11);
    assert_eq!(cursor.pos, 11);
}

#[test]
fn test_triple_click_single_line_end() {
    let text = "hello world";
    let boundaries = make_single_line_boundaries(text);

    // Triple-click at end (byte 10, 'd')
    let (selection, cursor) = triple_click_selection(&boundaries, 10);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 11);
    assert_eq!(cursor.pos, 11);
}

#[test]
fn test_find_line_boundary_single_line() {
    let text = "hello world";
    let boundaries = make_single_line_boundaries(text);

    let lb = find_line_boundary(&boundaries, 0);

    assert_eq!(lb.start, 0);
    assert_eq!(lb.end, 11);
}

// ============================================================================
// 5.2 Multi-Line Basic
// ============================================================================

#[test]
fn test_triple_click_line_0() {
    // Three lines: "abc", "def", "ghi"
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 0 (byte 0, 'a')
    let (selection, cursor) = triple_click_selection(&boundaries, 0);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 3); // "abc" ends at byte 3
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_triple_click_line_1() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 1 (byte 3, 'd')
    let (selection, cursor) = triple_click_selection(&boundaries, 3);

    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 6); // "def" ends at byte 6
    assert_eq!(cursor.pos, 6);
}

#[test]
fn test_triple_click_line_2() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 2 (byte 6, 'g')
    let (selection, cursor) = triple_click_selection(&boundaries, 6);

    assert_eq!(selection.anchor, 6);
    assert_eq!(selection.head, 9); // sentinel at byte 9
    assert_eq!(cursor.pos, 9);
}

#[test]
fn test_find_line_boundary_multiline() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    let lb0 = find_line_boundary(&boundaries, 0);
    assert_eq!(lb0.start, 0);
    assert_eq!(lb0.end, 3);

    let lb1 = find_line_boundary(&boundaries, 1);
    assert_eq!(lb1.start, 3);
    assert_eq!(lb1.end, 6);

    let lb2 = find_line_boundary(&boundaries, 2);
    assert_eq!(lb2.start, 6);
    assert_eq!(lb2.end, 9);
}

#[test]
fn test_line_drag_forward() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 0
    let initial_lb = find_line_boundary(&boundaries, 0);
    let initial_line_index = 0;

    // Drag to line 2 (byte 7, 'h')
    let selection = line_drag_update(initial_lb, initial_line_index, 7, &boundaries);

    // Should select from start of line 0 to end of line 2
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 9);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 9);
}

#[test]
fn test_line_drag_backward() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 2
    let initial_lb = find_line_boundary(&boundaries, 2);
    let initial_line_index = 2;

    // Drag backward to line 0 (byte 1, 'b')
    let selection = line_drag_update(initial_lb, initial_line_index, 1, &boundaries);

    // Should select from start of line 0 to end of line 2
    // anchor = initial_lb.end (9), head = drag_lb.start (0)
    assert_eq!(selection.anchor, 9);
    assert_eq!(selection.head, 0);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 9);
}

#[test]
fn test_line_drag_to_middle_line() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 0
    let initial_lb = find_line_boundary(&boundaries, 0);
    let initial_line_index = 0;

    // Drag to line 1 (byte 4, 'e')
    let selection = line_drag_update(initial_lb, initial_line_index, 4, &boundaries);

    // Should select lines 0..=1
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 6);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 6);
}

#[test]
fn test_line_drag_same_line() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 1
    let initial_lb = find_line_boundary(&boundaries, 1);
    let initial_line_index = 1;

    // Drag within same line (byte 5, 'f')
    let selection = line_drag_update(initial_lb, initial_line_index, 5, &boundaries);

    // Should still select just line 1
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 6);
}

// ============================================================================
// 5.3 Degenerate Cases
// ============================================================================

#[test]
fn test_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let (selection, cursor) = triple_click_selection(&boundaries, 0);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_empty_line_boundary() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let lb = find_line_boundary(&boundaries, 0);

    assert_eq!(lb.start, 0);
    assert_eq!(lb.end, 0);
}

#[test]
fn test_triple_click_at_sentinel() {
    let text = "hello";
    let boundaries = make_single_line_boundaries(text);

    // Triple-click at sentinel (byte 5)
    let (selection, cursor) = triple_click_selection(&boundaries, 5);

    // Should select the last (only) line
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 5);
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_line_boundary_out_of_range() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);

    // Request line 10 (doesn't exist)
    let lb = find_line_boundary(&boundaries, 10);

    // Should return sentinel position
    assert_eq!(lb.start, 6);
    assert_eq!(lb.end, 6);
}

#[test]
fn test_line_drag_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    let initial_lb = LineBoundary { start: 0, end: 0 };

    let selection = line_drag_update(initial_lb, 0, 0, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

// ============================================================================
// 5.4 UTF-8 Boundaries
// ============================================================================

#[test]
fn test_utf8_single_line() {
    // Cyrillic text: "Привет" (12 bytes, 6 chars)
    let text = "Привет";
    let boundaries = make_utf8_boundaries(text, 0);

    let (selection, cursor) = triple_click_selection(&boundaries, 0);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 12);
    assert_eq!(cursor.pos, 12);
}

#[test]
fn test_utf8_line_boundary() {
    let text = "Привет";
    let boundaries = make_utf8_boundaries(text, 0);

    let lb = find_line_boundary(&boundaries, 0);

    assert_eq!(lb.start, 0);
    assert_eq!(lb.end, 12);
}

#[test]
fn test_utf8_multiline() {
    // Create multi-line boundaries with UTF-8 content
    // Line 0: "Привет" (12 bytes)
    // Line 1: "мир" (6 bytes)
    let mut boundaries = Vec::new();

    // Line 0: "Привет"
    let line0 = "Привет";
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

    // Line 1: "мир"
    let line1 = "мир";
    let base_offset = line0.len();
    let chars1: Vec<(usize, char)> = line1.char_indices().collect();
    for (i, (byte_start, _)) in chars1.iter().enumerate() {
        let byte_end = if i + 1 < chars1.len() {
            base_offset + chars1[i + 1].0
        } else {
            base_offset + line1.len()
        };
        boundaries.push(ClusterBoundary {
            byte_start: base_offset + *byte_start,
            byte_end,
            line_index: 1,
            x_logical: i as f32 * 10.0,
        });
    }

    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: line0.len() + line1.len(),
        byte_end: line0.len() + line1.len(),
        line_index: 1,
        x_logical: 0.0,
    });

    // Test line 0
    let lb0 = find_line_boundary(&boundaries, 0);
    assert_eq!(lb0.start, 0);
    assert_eq!(lb0.end, 12);

    // Test line 1
    let lb1 = find_line_boundary(&boundaries, 1);
    assert_eq!(lb1.start, 12);
    assert_eq!(lb1.end, 18);

    // Triple-click on line 0
    let (selection, _) = triple_click_selection(&boundaries, 0);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 12);

    // Triple-click on line 1
    let (selection, _) = triple_click_selection(&boundaries, 12);
    assert_eq!(selection.anchor, 12);
    assert_eq!(selection.head, 18);

    // Line drag from line 0 to line 1
    let initial_lb = find_line_boundary(&boundaries, 0);
    let selection = line_drag_update(initial_lb, 0, 14, &boundaries);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 18);
}

// ============================================================================
// Selection Integration
// ============================================================================

#[test]
fn test_selection_half_open() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Triple-click on line 1
    let (selection, _) = triple_click_selection(&boundaries, 3);

    // Selection should be half-open [3, 6)
    assert_eq!(selection.start(), 3);
    assert_eq!(selection.end(), 6);
    assert!(!selection.is_empty());
}

#[test]
fn test_line_drag_produces_valid_selection() {
    let boundaries = make_multiline_boundaries(&["abc", "def", "ghi"]);

    // Forward drag
    let initial_lb = find_line_boundary(&boundaries, 0);
    let forward = line_drag_update(initial_lb, 0, 8, &boundaries);
    assert!(forward.start() <= forward.end());

    // Backward drag
    let initial_lb = find_line_boundary(&boundaries, 2);
    let backward = line_drag_update(initial_lb, 2, 1, &boundaries);
    assert!(backward.start() <= backward.end());
}

#[test]
fn test_cursor_at_line_end() {
    let boundaries = make_multiline_boundaries(&["abc", "def"]);

    let (_, cursor) = triple_click_selection(&boundaries, 0);
    assert_eq!(cursor.pos, 3); // End of line 0

    let (_, cursor) = triple_click_selection(&boundaries, 3);
    assert_eq!(cursor.pos, 6); // End of line 1 (sentinel)
}
