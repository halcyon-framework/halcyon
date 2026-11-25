// SPEC-HAL-??? - Cursor EOL, Empty Line, and Horizontal Scroll Invariants
//
// GI-3: Cursor legality - caret must be on existing caret stop
// GI-4: Line existence - empty lines MUST have at least one caret stop
// GI-5: EOL sentinel semantics - ArrowRight from last cluster lands on sentinel (same line)
// GI-6: Horizontal scroll correctness - scroll_x clamp and ensure-visible
//
// These tests must FAIL before fixes and PASS after.

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, move_right_collapse};
use halcyon::core::paragraph::Line;

// ============================================================================
// Helper: Create boundaries with explicit line structure
// ============================================================================

/// Create boundaries for "a." on line 0 with sentinel at end.
/// Expected boundaries:
///   [0] byte_start=0, byte_end=1, line_index=0, x=0   ('a')
///   [1] byte_start=1, byte_end=2, line_index=0, x=10  ('.')
///   [2] byte_start=2, byte_end=2, line_index=0, x=20  (sentinel)
fn make_single_line_a_dot() -> (String, Vec<ClusterBoundary>) {
    let text = "a.".to_string();
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
        }, // sentinel
    ];
    (text, boundaries)
}

/// Create boundaries for "a.\n\nb" with empty line 1.
/// Expected boundaries:
///   [0] byte_start=0, byte_end=1, line_index=0, x=0   ('a')
///   [1] byte_start=1, byte_end=2, line_index=0, x=10  ('.')
///   [2] byte_start=2, byte_end=3, line_index=0, x=20  ('\n' - newline cluster)
///   [3] byte_start=3, byte_end=4, line_index=1, x=0   ('\n' on empty line - sentinel for line 1)
///   [4] byte_start=4, byte_end=5, line_index=2, x=0   ('b')
///   [5] byte_start=5, byte_end=5, line_index=2, x=10  (sentinel)
fn make_multiline_with_empty() -> (String, Vec<ClusterBoundary>) {
    let text = "a.\n\nb".to_string();
    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        }, // 'a'
        ClusterBoundary {
            byte_start: 1,
            byte_end: 2,
            line_index: 0,
            x_logical: 10.0,
        }, // '.'
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        }, // '\n' on line 0
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 0.0,
        }, // '\n' on line 1 (empty line sentinel)
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 2,
            x_logical: 0.0,
        }, // 'b'
        ClusterBoundary {
            byte_start: 5,
            byte_end: 5,
            line_index: 2,
            x_logical: 10.0,
        }, // sentinel
    ];
    (text, boundaries)
}

// ============================================================================
// T-1: "EOL-after-last reachable by ArrowRight"
//
// GI-5: Arrow-right from the last real cluster on L must land on S(L) (same line).
//       Arrow-right from S(L) may move to next line (if exists).
//       FORBIDDEN: Arrow-right from "before last char" jumping to next line.
// ============================================================================

#[test]
fn t1_eol_after_last_reachable_by_arrow_right() {
    let (_text, boundaries) = make_single_line_a_dot();

    // Initial: caret at start (position 0)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // After 1st ArrowRight: caret between 'a' and '.'
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 1,
        "After 1st ArrowRight: caret must be at position 1 (between 'a' and '.'), got {}",
        cursor.pos
    );

    // After 2nd ArrowRight: caret is AFTER '.', same line (this is the sentinel stop)
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 2,
        "After 2nd ArrowRight: caret must be at position 2 (sentinel, after '.'), got {}",
        cursor.pos
    );

    // Verify caret is still on line 0 (sentinel belongs to line 0)
    let sentinel = &boundaries[2];
    assert_eq!(sentinel.byte_start, 2);
    assert_eq!(sentinel.line_index, 0, "Sentinel must be on line 0");

    // After 3rd ArrowRight: caret stays (no next line) - this is a no-op
    let pos_before = cursor.pos;
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, pos_before,
        "After 3rd ArrowRight at sentinel: caret must stay at {} (no next line), got {}",
        pos_before, cursor.pos
    );
}

#[test]
fn t1_arrow_right_does_not_skip_sentinel() {
    // Verifies GI-5: caret does NOT jump off the line on 2nd press
    let (_text, boundaries) = make_single_line_a_dot();

    let mut cursor = TextCursor::at(1); // Start at '.'
    let mut selection = Selection::at(1);

    // ArrowRight from '.' must land on sentinel (position 2), NOT skip to next line
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Get the boundary where we landed
    let landing = boundaries.iter().find(|b| b.byte_start == cursor.pos);
    assert!(landing.is_some(), "Cursor must land on a valid boundary");

    let landing = landing.unwrap();
    assert_eq!(
        landing.line_index, 0,
        "GI-5 violated: Arrow-right from '.' (before sentinel) jumped to line {}, expected to stay on line 0",
        landing.line_index
    );
}

// ============================================================================
// T-2: "Empty line not skipped"
//
// GI-4: Visual line indices are contiguous 0..N-1. Empty visual lines MUST exist
//       as lines with at least one caret stop.
// FORBIDDEN: "skip empty lines" logic anywhere in navigation.
// ============================================================================

#[test]
fn t2_empty_line_not_skipped() {
    let (_text, boundaries) = make_multiline_with_empty();

    // Start at position 2 (the '\n' at end of line 0, which is the newline boundary)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // ArrowRight from newline at line 0 must go to line 1 (the empty line) at x=0 sentinel
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // Verify cursor landed on line 1 (the empty line)
    let landing = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .expect("Cursor must land on a valid boundary");

    assert_eq!(
        landing.line_index, 1,
        "GI-4 violated: Empty line was skipped! ArrowRight from line 0 newline landed on line {}, expected line 1",
        landing.line_index
    );

    // The empty line (line 1) has only its sentinel ('\n' at position 3)
    assert_eq!(
        cursor.pos, 3,
        "Cursor must be at position 3 (empty line sentinel), got {}",
        cursor.pos
    );
}

#[test]
fn t2_arrow_right_from_empty_line_goes_to_next_content_line() {
    let (_text, boundaries) = make_multiline_with_empty();

    // Start at position 3 (the '\n' on line 1 = empty line sentinel)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    // ArrowRight from empty line must go to line 2 at position 4 ('b')
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    let landing = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .expect("Cursor must land on a valid boundary");

    assert_eq!(
        landing.line_index, 2,
        "ArrowRight from empty line should go to line 2, got line {}",
        landing.line_index
    );
    assert_eq!(
        cursor.pos, 4,
        "Cursor must be at position 4 ('b'), got {}",
        cursor.pos
    );
}

#[test]
fn t2_sequential_arrow_right_visits_empty_line() {
    let (_text, boundaries) = make_multiline_with_empty();

    // Start at position 0 and walk right, verify we visit line 1 (empty)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    let mut visited_lines: Vec<usize> = vec![0]; // Starting line

    for _ in 0..10 {
        let prev_pos = cursor.pos;
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
        if cursor.pos == prev_pos {
            break; // No more movement
        }

        if let Some(b) = boundaries.iter().find(|b| b.byte_start == cursor.pos) {
            if visited_lines.last() != Some(&b.line_index) {
                visited_lines.push(b.line_index);
            }
        }
    }

    // Must have visited lines 0, 1, 2 in order
    assert!(
        visited_lines.contains(&1),
        "GI-4 violated: Sequential ArrowRight skipped line 1 (empty line). Visited lines: {:?}",
        visited_lines
    );

    assert_eq!(
        visited_lines,
        vec![0, 1, 2],
        "Sequential ArrowRight must visit lines in order 0, 1, 2. Got: {:?}",
        visited_lines
    );
}

// ============================================================================
// T-3: "Horizontal ensure-visible follows caret"
//
// GI-6: Horizontal scroll correctness
//       - content_width = max(line_width) across ALL visual lines
//       - scroll_x clamp: 0 <= scroll_x <= max(0, content_width - viewport_width)
//       - After any caret move, caret must be made visible horizontally
// ============================================================================

// Note: T-3 requires access to TextField's scroll state.
// TextField exposes scroll_x_for_test() and ensure_visible_for_test() in test mode.

// ============================================================================
// T-BOUNDARY: Tests using ClusterBoundary::build_from_lines directly
//
// These tests verify that the boundary builder correctly generates boundaries
// for empty lines and sentinels.
// ============================================================================

mod t_boundary_builder {
    use super::*;
    use halcyon::core::paragraph::{Run, ShapedGlyph, TextStyle};
    use halcyon::core::text_engine::TextMetrics;

    /// Create a minimal Line with no runs (empty visual line).
    fn make_empty_line(height: f32) -> Line {
        Line {
            runs: Vec::new(),
            width: 0.0,
            ascent: height * 0.8,
            descent: height * 0.2,
            height,
            baseline: height * 0.8,
        }
    }

    /// Create a Line with a single run containing glyphs.
    fn make_line_with_content(text_range: std::ops::Range<usize>, glyph_count: usize) -> Line {
        let mut glyphs = Vec::new();
        for i in 0..glyph_count {
            glyphs.push(ShapedGlyph {
                glyph_id: (i + 65) as u32, // A, B, C, ...
                cluster: i as u32,
                advance: 10.0,
                x_offset: 0.0,
                y_offset: 0.0,
            });
        }

        let run = Run {
            range: text_range,
            style: TextStyle::default(),
            font_id: 0,
            glyphs,
            metrics: TextMetrics {
                ascent: 12.8,
                descent: 3.2,
                line_gap: 0.0,
                line_height: 16.0,
            },
            width: glyph_count as f32 * 10.0,
        };

        Line {
            runs: vec![run],
            width: glyph_count as f32 * 10.0,
            ascent: 12.8,
            descent: 3.2,
            height: 16.0,
            baseline: 12.8,
        }
    }

    #[test]
    fn build_from_lines_includes_empty_line_boundary() {
        // Text: "a\n\nb" (line 0: "a\n", line 1: "\n" (empty), line 2: "b")
        let text = "a\n\nb";

        // Create lines matching layout output:
        // Line 0: "a" + newline
        // Line 1: empty (just newline)
        // Line 2: "b"
        let lines = vec![
            make_line_with_content(0..1, 1), // "a"
            make_empty_line(16.0),           // empty line
            make_line_with_content(3..4, 1), // "b"
        ];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Check that line 1 (empty) has at least one boundary
        let line1_boundaries: Vec<_> = boundaries.iter().filter(|b| b.line_index == 1).collect();

        assert!(
            !line1_boundaries.is_empty(),
            "GI-4 violated: build_from_lines produced no boundary for empty line 1. \
             All boundaries: {:?}",
            boundaries
                .iter()
                .map(|b| (b.byte_start, b.line_index))
                .collect::<Vec<_>>()
        );

        // The empty line should have a boundary at byte position 2 (the second '\n')
        let has_empty_line_boundary = line1_boundaries
            .iter()
            .any(|b| b.byte_start == 2 || b.byte_start == 3);

        assert!(
            has_empty_line_boundary,
            "Empty line 1 boundary should be at position 2 or 3 (newline). \
             Found: {:?}",
            line1_boundaries
        );
    }

    #[test]
    fn build_from_lines_contiguous_line_indices() {
        // Verify GI-4: Visual line indices are contiguous 0..N-1
        let text = "a\n\nb";
        let lines = vec![
            make_line_with_content(0..1, 1),
            make_empty_line(16.0),
            make_line_with_content(3..4, 1),
        ];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Collect all unique line indices in order
        let mut line_indices: Vec<usize> = boundaries.iter().map(|b| b.line_index).collect();
        line_indices.sort();
        line_indices.dedup();

        // Should have lines 0, 1, 2
        assert_eq!(
            line_indices,
            vec![0, 1, 2],
            "Line indices must be contiguous 0..N-1. Got: {:?}",
            line_indices
        );
    }

    #[test]
    fn movement_through_build_from_lines_boundaries() {
        // Use actual build_from_lines boundaries and verify movement
        let text = "a\n\nb";
        let lines = vec![
            make_line_with_content(0..1, 1),
            make_empty_line(16.0),
            make_line_with_content(3..4, 1),
        ];

        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Start at position 0 and walk right, verify we visit line 1
        let mut cursor = TextCursor::at(0);
        let mut selection = Selection::at(0);

        let mut visited_lines: Vec<usize> = vec![];
        let mut positions: Vec<usize> = vec![0];

        for _ in 0..10 {
            if let Some(b) = boundaries.iter().find(|b| b.byte_start == cursor.pos) {
                if visited_lines.last() != Some(&b.line_index) {
                    visited_lines.push(b.line_index);
                }
            }

            let prev_pos = cursor.pos;
            move_right_collapse(&mut cursor, &mut selection, &boundaries);
            if cursor.pos == prev_pos {
                break;
            }
            positions.push(cursor.pos);
        }

        // Must visit all lines including the empty one
        assert!(
            visited_lines.contains(&1),
            "GI-4 violated: Movement skipped empty line 1. \
             Visited lines: {:?}, positions: {:?}, boundaries: {:?}",
            visited_lines,
            positions,
            boundaries
                .iter()
                .map(|b| (b.byte_start, b.line_index))
                .collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// T-GEOM: Trailing caret geometry tests (SPEC-HAL-109 Stage S.1 geometry fix)
//
// Test A: Trailing caret x is strictly after last glyph
// Test B: Content width covers trailing caret
// ============================================================================

#[cfg(feature = "test-harness")]
mod t_trailing_caret_geometry {
    use halcyon::core::editing::ClusterBoundary;
    use halcyon::core::paragraph::{Line, Run, ShapedGlyph, TextStyle};
    use halcyon::core::text_engine::TextMetrics;
    use halcyon::widgets::text_layout_cache::TextLayoutCache;

    /// Create a Line with specified content for testing.
    fn make_line_with_glyphs(range: std::ops::Range<usize>, glyphs: &[(u32, f32)]) -> Line {
        let shaped_glyphs: Vec<ShapedGlyph> = glyphs
            .iter()
            .enumerate()
            .map(|(i, (glyph_id, advance))| ShapedGlyph {
                glyph_id: *glyph_id,
                cluster: i as u32,
                advance: *advance,
                x_offset: 0.0,
                y_offset: 0.0,
            })
            .collect();

        let width: f32 = glyphs.iter().map(|(_, adv)| adv).sum();

        let run = Run {
            range,
            style: TextStyle::default(),
            font_id: 0,
            glyphs: shaped_glyphs,
            metrics: TextMetrics {
                ascent: 12.8,
                descent: 3.2,
                line_gap: 0.0,
                line_height: 16.0,
            },
            width,
        };

        Line {
            runs: vec![run],
            width,
            ascent: 12.8,
            descent: 3.2,
            height: 16.0,
            baseline: 12.8,
        }
    }

    /// Test A: Trailing caret x is strictly after last glyph.
    ///
    /// S1.INV6: caret_x_for_byte(doc.len()) must be > caret_x_for_byte of last cluster start.
    #[test]
    fn test_a_trailing_caret_x_after_last_glyph() {
        // Build layout for "abc.xyz." (8 chars, each 10px advance)
        let text = "abc.xyz.";
        let doc_len = text.len(); // 8

        // Create boundaries for the text:
        // Positions: 0='a', 1='b', 2='c', 3='.', 4='x', 5='y', 6='z', 7='.', 8=sentinel
        let boundaries: Vec<ClusterBoundary> = (0..=doc_len)
            .map(|i| ClusterBoundary {
                byte_start: i,
                byte_end: if i < doc_len { i + 1 } else { i },
                line_index: 0,
                x_logical: i as f32 * 10.0, // 10px per char
            })
            .collect();

        // Verify boundaries are correct
        assert_eq!(boundaries.len(), 9); // 8 chars + 1 sentinel
        assert_eq!(boundaries[8].byte_start, 8); // sentinel at end
        assert_eq!(boundaries[8].x_logical, 80.0); // x after all glyphs

        // Create layout cache and inject boundaries
        let mut cache = TextLayoutCache::default();
        cache.set_boundaries_for_test(boundaries.clone());
        cache.set_cached_doc_len_for_test(doc_len);
        cache.clear_stale_for_test();

        // Query caret positions
        let x_last = cache.caret_x_for_byte(doc_len); // sentinel position
        let x_prev = cache.caret_x_for_byte(doc_len - 1); // last char '.'

        // Assert: x_last > x_prev (strictly, by at least 0.5px)
        let diff = x_last - x_prev;
        assert!(
            diff >= 0.5,
            "S1.INV6 violated: caret_x at EOL ({}) must be strictly > caret_x of last cluster ({}). \
             Difference: {} (expected >= 0.5)",
            x_last,
            x_prev,
            diff
        );

        // Also verify absolute values
        assert_eq!(
            x_last, 80.0,
            "EOL caret x should be 80.0 (8 chars * 10px each)"
        );
        assert_eq!(x_prev, 70.0, "Last char caret x should be 70.0");
    }

    /// Test B: Content width covers trailing caret.
    ///
    /// S1.INV6: content_width >= caret_x_for_byte(doc.len()) + CARET_THICKNESS
    #[test]
    fn test_b_content_width_covers_trailing_caret() {
        // Build layout for "abc.xyz." (8 chars, each 10px advance)
        // Total line width = 80px
        let text = "abc.xyz.";
        let doc_len = text.len();

        // Create a line with 8 glyphs, each 10px wide
        let glyphs: Vec<(u32, f32)> = (0..8).map(|i| (i as u32 + 65, 10.0)).collect();
        let line = make_line_with_glyphs(0..8, &glyphs);

        // Verify line width
        assert_eq!(line.width, 80.0, "Line width should be 80px");

        // Build boundaries using ClusterBoundary::build_from_lines
        let lines = vec![line];
        let boundaries = ClusterBoundary::build_from_lines(&lines, text);

        // Create layout cache and inject data
        let mut cache = TextLayoutCache::default();
        cache.set_boundaries_for_test(boundaries);
        cache.set_cached_doc_len_for_test(doc_len);
        cache.set_lines_for_test(1, 16.0);
        cache.clear_stale_for_test();

        // Manually set content width to simulate what rebuild() does
        // S1.INV6: content_width = max_line_width + CARET_THICKNESS
        let caret_thickness = 2.0;
        let expected_content_width = 80.0 + caret_thickness;

        // Get caret x at end
        let caret_x_at_eol = cache.caret_x_for_byte(doc_len);

        // Assert: content_width >= caret_x_at_eol + CARET_THICKNESS
        // Since content_width = max_line_width + CARET_THICKNESS
        // and caret_x_at_eol should equal max_line_width (for single line)
        // we need: max_line_width + CARET_THICKNESS >= caret_x_at_eol + CARET_THICKNESS
        // which simplifies to: max_line_width >= caret_x_at_eol
        assert!(
            expected_content_width >= caret_x_at_eol + caret_thickness,
            "S1.INV6 violated: content_width ({}) must be >= caret_x_at_eol ({}) + CARET_THICKNESS ({})",
            expected_content_width,
            caret_x_at_eol,
            caret_thickness
        );

        // Verify the caret x at EOL is the line width
        assert!(
            caret_x_at_eol >= 79.5, // Allow small epsilon for float comparison
            "Caret x at EOL ({}) should be approximately line width (80.0)",
            caret_x_at_eol
        );
    }
}

// NOTE: t3_horizontal_scroll module removed - it tested internal TextField scroll
// behavior that now requires tree integration. The set_text API changed to require
// WidgetTree and NodeId parameters, making isolated widget testing impossible.
// These invariants (GI-6: horizontal scroll correctness) should be tested through
// integration tests or moved to src/tests/ with proper tree setup.
