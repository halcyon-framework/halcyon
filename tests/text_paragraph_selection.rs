// SPEC-HAL-052 Stage 6: Paragraph Selection Tests

use halcyon::core::editing::{
    ClusterBoundary, ParagraphBoundary, find_paragraph_boundary, paragraph_drag_update,
    quadruple_click_selection,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for multi-line/multi-paragraph text.
/// Each line's content is specified separately.
/// Empty strings ("") represent paragraph separators (no boundaries added for empty lines).
fn make_paragraph_boundaries(lines: &[&str]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line_text) in lines.iter().enumerate() {
        if line_text.is_empty() {
            // Empty line - don't add any boundary
            // is_empty_line will return true because no boundary exists for this line
        } else {
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

/// Create UTF-8 boundaries for a multi-paragraph setup.
fn make_utf8_paragraph_boundaries(lines: &[&str]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line_text) in lines.iter().enumerate() {
        if line_text.is_empty() {
            // Empty line - don't add any boundary
            // is_empty_line will return true because no boundary exists for this line
        } else {
            let chars: Vec<(usize, char)> = line_text.char_indices().collect();
            for (i, (char_byte_start, _)) in chars.iter().enumerate() {
                let char_byte_end = if i + 1 < chars.len() {
                    chars[i + 1].0
                } else {
                    line_text.len()
                };
                boundaries.push(ClusterBoundary {
                    byte_start: byte_offset + *char_byte_start,
                    byte_end: byte_offset + char_byte_end,
                    line_index: line_idx,
                    x_logical: i as f32 * 10.0,
                });
            }
            byte_offset += line_text.len();
        }
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

// ============================================================================
// 6.1 Basic Paragraphs
// ============================================================================

#[test]
fn test_single_paragraph_click_anywhere() {
    // Single paragraph: "abc" on line 0
    let boundaries = make_paragraph_boundaries(&["abc"]);

    // Click at start
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);

    // Click in middle
    let pb = find_paragraph_boundary(&boundaries, 1);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);

    // Click at end
    let pb = find_paragraph_boundary(&boundaries, 2);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);
}

#[test]
fn test_two_paragraphs_separated_by_empty_line() {
    // Two paragraphs: "abc", "", "def"
    // Line 0: "abc" (bytes 0-3)
    // Line 1: "" (empty, paragraph separator)
    // Line 2: "def" (bytes 3-6)
    let boundaries = make_paragraph_boundaries(&["abc", "", "def"]);

    // Click in first paragraph
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);

    // Click in second paragraph
    let pb = find_paragraph_boundary(&boundaries, 3);
    assert_eq!(pb.start, 3);
    assert_eq!(pb.end, 6);
}

#[test]
fn test_three_paragraphs_varying_size() {
    // Three paragraphs:
    // Para 1: "a" (1 char)
    // Para 2: "bcde" (4 chars)
    // Para 3: "fg" (2 chars)
    let boundaries = make_paragraph_boundaries(&["a", "", "bcde", "", "fg"]);

    // Paragraph 1
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 1);

    // Paragraph 2 (click on 'c')
    let pb = find_paragraph_boundary(&boundaries, 2);
    assert_eq!(pb.start, 1);
    assert_eq!(pb.end, 5);

    // Paragraph 3
    let pb = find_paragraph_boundary(&boundaries, 5);
    assert_eq!(pb.start, 5);
    assert_eq!(pb.end, 7);
}

#[test]
fn test_multiline_paragraph() {
    // Single paragraph spanning multiple lines (no empty line separator)
    // Line 0: "abc"
    // Line 1: "def"
    // Line 2: "ghi"
    let boundaries = make_paragraph_boundaries(&["abc", "def", "ghi"]);

    // Click anywhere should select entire paragraph
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 9);

    let pb = find_paragraph_boundary(&boundaries, 4);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 9);

    let pb = find_paragraph_boundary(&boundaries, 7);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 9);
}

// ============================================================================
// 6.2 Quadruple-Click
// ============================================================================

#[test]
fn test_quadruple_click_anchor_head_match_paragraph() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def"]);

    // Quadruple-click in first paragraph
    let (selection, cursor) = quadruple_click_selection(&boundaries, 1);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 3);
    assert_eq!(cursor.pos, 3);

    // Quadruple-click in second paragraph
    let (selection, cursor) = quadruple_click_selection(&boundaries, 4);
    assert_eq!(selection.anchor, 3);
    assert_eq!(selection.head, 6);
    assert_eq!(cursor.pos, 6);
}

#[test]
fn test_quadruple_click_cursor_at_paragraph_end() {
    let boundaries = make_paragraph_boundaries(&["hello", "", "world"]);

    let (_, cursor) = quadruple_click_selection(&boundaries, 0);
    assert_eq!(cursor.pos, 5); // End of "hello"

    let (_, cursor) = quadruple_click_selection(&boundaries, 5);
    assert_eq!(cursor.pos, 10); // End of "world" (sentinel)
}

#[test]
fn test_quadruple_click_multiline_paragraph() {
    // Paragraph with multiple lines
    let boundaries = make_paragraph_boundaries(&["abc", "def", "", "ghi"]);

    // Click in multiline paragraph
    let (selection, cursor) = quadruple_click_selection(&boundaries, 4);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 6);
    assert_eq!(cursor.pos, 6);
}

// ============================================================================
// 6.3 Paragraph Drag
// ============================================================================

#[test]
fn test_paragraph_drag_forward() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def", "", "ghi"]);

    // Quadruple-click on first paragraph
    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let initial_par_index = 0;

    // Drag to third paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 6, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 9);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 9);
}

#[test]
fn test_paragraph_drag_backward() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def", "", "ghi"]);

    // Quadruple-click on third paragraph
    let initial_pb = find_paragraph_boundary(&boundaries, 6);
    let initial_par_index = 2;

    // Drag backward to first paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 1, &boundaries);

    assert_eq!(selection.anchor, 9);
    assert_eq!(selection.head, 0);
    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 9);
}

#[test]
fn test_paragraph_drag_same_paragraph() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def"]);

    // Quadruple-click on first paragraph
    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let initial_par_index = 0;

    // Drag within same paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 2, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 3);
}

#[test]
fn test_paragraph_drag_to_middle_paragraph() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def", "", "ghi"]);

    // Quadruple-click on first paragraph
    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let initial_par_index = 0;

    // Drag to second paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 4, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 6);
}

#[test]
fn test_paragraph_drag_into_empty_paragraph() {
    // Test with empty paragraph as separator
    let boundaries = make_paragraph_boundaries(&["abc", "", "def"]);

    // The empty line is line 1 - no boundaries exist for it
    // (is_empty_line returns true when no boundaries for a line)
    let empty_line_boundary = boundaries.iter().find(|b| b.line_index == 1);
    assert!(empty_line_boundary.is_none());

    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let initial_par_index = 0;

    // Drag through empty line to second paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 4, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 6);
}

// ============================================================================
// 6.4 UTF-8
// ============================================================================

#[test]
fn test_utf8_paragraph() {
    // Cyrillic paragraph: "Привет" (12 bytes)
    let boundaries = make_utf8_paragraph_boundaries(&["Привет"]);

    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 12);
}

#[test]
fn test_utf8_two_paragraphs() {
    // Two Cyrillic paragraphs
    // "Привет" (12 bytes) + "" + "мир" (6 bytes)
    let boundaries = make_utf8_paragraph_boundaries(&["Привет", "", "мир"]);

    // First paragraph
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 12);

    // Second paragraph
    let pb = find_paragraph_boundary(&boundaries, 12);
    assert_eq!(pb.start, 12);
    assert_eq!(pb.end, 18);
}

#[test]
fn test_utf8_multiline_paragraph() {
    // Multi-line paragraph with mixed content
    let boundaries = make_utf8_paragraph_boundaries(&["Привет", "мир"]);

    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 18);

    let pb = find_paragraph_boundary(&boundaries, 14);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 18);
}

#[test]
fn test_utf8_paragraph_drag() {
    let boundaries = make_utf8_paragraph_boundaries(&["Привет", "", "мир"]);

    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let initial_par_index = 0;

    // Drag to second paragraph
    let selection = paragraph_drag_update(initial_pb, initial_par_index, 14, &boundaries);

    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 18);
}

// ============================================================================
// 6.5 Degenerate Cases
// ============================================================================

#[test]
fn test_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 0);
}

#[test]
fn test_quadruple_click_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    let (selection, cursor) = quadruple_click_selection(&boundaries, 0);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_single_empty_paragraph() {
    // Just an empty line (which is also the sentinel)
    let boundaries = vec![ClusterBoundary {
        byte_start: 0,
        byte_end: 0,
        line_index: 0,
        x_logical: 0.0,
    }];

    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 0);
}

#[test]
fn test_click_at_sentinel() {
    let boundaries = make_paragraph_boundaries(&["abc"]);

    // Click at sentinel (byte 3)
    let pb = find_paragraph_boundary(&boundaries, 3);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);
}

#[test]
fn test_paragraph_drag_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];
    let initial_pb = ParagraphBoundary { start: 0, end: 0 };

    let selection = paragraph_drag_update(initial_pb, 0, 0, &boundaries);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_consecutive_empty_lines() {
    // Multiple empty lines between paragraphs
    let boundaries = make_paragraph_boundaries(&["abc", "", "", "def"]);

    // First paragraph
    let pb = find_paragraph_boundary(&boundaries, 0);
    assert_eq!(pb.start, 0);
    assert_eq!(pb.end, 3);

    // Second paragraph (after two empty lines)
    let pb = find_paragraph_boundary(&boundaries, 3);
    assert_eq!(pb.start, 3);
    assert_eq!(pb.end, 6);
}

// ============================================================================
// Selection Integration
// ============================================================================

#[test]
fn test_paragraph_selection_half_open() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def"]);

    let (selection, _) = quadruple_click_selection(&boundaries, 0);

    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 3);
    assert!(!selection.is_empty());
}

#[test]
fn test_paragraph_drag_produces_valid_selection() {
    let boundaries = make_paragraph_boundaries(&["abc", "", "def", "", "ghi"]);

    // Forward drag
    let initial_pb = find_paragraph_boundary(&boundaries, 0);
    let forward = paragraph_drag_update(initial_pb, 0, 7, &boundaries);
    assert!(forward.start() <= forward.end());

    // Backward drag
    let initial_pb = find_paragraph_boundary(&boundaries, 6);
    let backward = paragraph_drag_update(initial_pb, 2, 1, &boundaries);
    assert!(backward.start() <= backward.end());
}
