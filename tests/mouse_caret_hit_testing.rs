// SPEC-HAL-107 §6 — Mouse Hit-Testing & Caret Placement Invariants
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

#![allow(non_snake_case, unused_variables)]

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, hit_test, hit_test_x_y};
use halcyon::core::paragraph::Line;

// ============================================================================
// Helpers
// ============================================================================

/// Create boundaries for ASCII text with 10px per character, single line
fn make_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let char_width = 10.0;
    for (i, c) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: i,
            byte_end: i + c.len_utf8(),
            line_index: 0,
            x_logical: i as f32 * char_width,
        });
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * char_width,
    });
    boundaries
}

/// Create multi-line boundaries with proper line_index assignment
fn make_multiline_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_on_line = 0.0;
    let char_width = 10.0;

    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index,
            x_logical: x_on_line,
        });
        if ch == '\n' {
            line_index += 1;
            x_on_line = 0.0;
        } else {
            x_on_line += char_width;
        }
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: x_on_line,
    });
    boundaries
}

/// Create Lines from multiline text (used with hit_test)
fn make_lines(text: &str) -> Vec<Line> {
    let line_height = 20.0;
    let ascent = 14.0;
    let descent = 6.0;
    text.split('\n')
        .map(|_| Line {
            runs: Vec::new(),
            width: 100.0, // placeholder
            ascent,
            descent,
            height: line_height,
            baseline: ascent,
        })
        .collect()
}

/// Simulate a mouse click and update cursor/selection (single-line)
fn resolve_click_single_line(
    x: f32,
    boundaries: &[ClusterBoundary],
    cursor: &mut TextCursor,
    selection: &mut Selection,
    shift: bool,
) {
    let hit_pos = hit_test_x_y(boundaries, x, 0.0);
    cursor.pos = hit_pos;
    cursor.preferred_x = None;
    if shift {
        selection.head = hit_pos;
    } else {
        selection.anchor = hit_pos;
        selection.head = hit_pos;
    }
}

/// Simulate a mouse click and update cursor/selection (multi-line)
fn resolve_click_multiline(
    x: f32,
    y: f32,
    boundaries: &[ClusterBoundary],
    lines: &[Line],
    cursor: &mut TextCursor,
    selection: &mut Selection,
    shift: bool,
) {
    let hit_pos = hit_test(boundaries, lines, x, y);
    cursor.pos = hit_pos;
    cursor.preferred_x = None;
    if shift {
        selection.head = hit_pos;
    } else {
        selection.anchor = hit_pos;
        selection.head = hit_pos;
    }
}

// =============================================================================
// SPEC-HAL-107 §6.1 — Coordinate → Boundary Mapping
// =============================================================================

#[test]
fn mouse_click_resolves_to_single_boundary_SPEC_107_6_1() {
    // Any mouse click must resolve to exactly one boundary
    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    // Click at various X positions
    for x in [0.0, 15.0, 25.0, 45.0, 100.0] {
        let hit_pos = hit_test_x_y(&boundaries, x, 0.0);
        // hit_pos must be a valid byte index
        assert!(
            hit_pos <= text.len(),
            "hit_pos {} exceeds text length {}",
            hit_pos,
            text.len()
        );
        // hit_pos must match some boundary.byte_start
        let matching = boundaries.iter().any(|b| b.byte_start == hit_pos);
        assert!(
            matching,
            "hit_pos {} does not match any boundary.byte_start",
            hit_pos
        );
    }
}

#[test]
fn mouse_click_never_lands_between_boundaries_SPEC_107_6_1() {
    // Resolution must land ON a boundary, not between them
    let text = "abc";
    let boundaries = make_ascii_boundaries(text);

    // Collect all valid boundary byte_start values
    let valid_positions: Vec<usize> = boundaries.iter().map(|b| b.byte_start).collect();

    // Click at many positions
    for x in (0..100).map(|i| i as f32) {
        let hit_pos = hit_test_x_y(&boundaries, x, 0.0);
        assert!(
            valid_positions.contains(&hit_pos),
            "hit_pos {} is not a valid boundary position. Valid: {:?}",
            hit_pos,
            valid_positions
        );
    }
}

#[test]
fn caret_position_equals_boundary_byte_start_SPEC_107_6_1() {
    // After mouse click, cursor.pos must equal some boundary.byte_start
    let text = "Test";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::default();
    let mut selection = Selection::default();

    // Click at x=25 (between 'e' at x=10 and 's' at x=20, nearest is 's' at 2)
    resolve_click_single_line(25.0, &boundaries, &mut cursor, &mut selection, false);

    // cursor.pos must be a boundary byte_start
    let found = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        found,
        "cursor.pos {} is not a valid boundary byte_start",
        cursor.pos
    );
}

#[test]
fn mouse_click_never_lands_inside_utf8_codepoint_SPEC_107_6_1() {
    // UTF-8 multi-byte: "日本" (3 bytes each)
    let text = "日本"; // 6 bytes total
    let mut boundaries = Vec::new();
    let char_width = 20.0;

    // '日' at bytes 0..3
    boundaries.push(ClusterBoundary {
        byte_start: 0,
        byte_end: 3,
        line_index: 0,
        x_logical: 0.0,
    });
    // '本' at bytes 3..6
    boundaries.push(ClusterBoundary {
        byte_start: 3,
        byte_end: 6,
        line_index: 0,
        x_logical: char_width,
    });
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: 6,
        byte_end: 6,
        line_index: 0,
        x_logical: char_width * 2.0,
    });

    // Click in various positions - must land on 0, 3, or 6 (never 1, 2, 4, 5)
    for x in (0..100).map(|i| i as f32) {
        let hit_pos = hit_test_x_y(&boundaries, x, 0.0);
        assert!(
            hit_pos == 0 || hit_pos == 3 || hit_pos == 6,
            "hit_pos {} is inside a UTF-8 codepoint (valid: 0, 3, 6)",
            hit_pos
        );
    }
}

// =============================================================================
// SPEC-HAL-107 §6.2 — Horizontal Resolution (X axis)
// =============================================================================

#[test]
fn mouse_click_x_maps_to_nearest_boundary_SPEC_107_6_2() {
    let text = "ab"; // boundaries at x=0, x=10, x=20
    let boundaries = make_ascii_boundaries(text);

    // Click at x=4 -> nearest is x=0 (boundary 0)
    let hit = hit_test_x_y(&boundaries, 4.0, 0.0);
    assert_eq!(hit, 0);

    // Click at x=6 -> nearest is x=10 (boundary 1)
    let hit = hit_test_x_y(&boundaries, 6.0, 0.0);
    assert_eq!(hit, 1);

    // Click at x=14 -> nearest is x=10 (boundary 1)
    let hit = hit_test_x_y(&boundaries, 14.0, 0.0);
    assert_eq!(hit, 1);

    // Click at x=16 -> nearest is x=20 (boundary 2, sentinel)
    let hit = hit_test_x_y(&boundaries, 16.0, 0.0);
    assert_eq!(hit, 2);
}

#[test]
fn mouse_click_x_clamps_left_of_line_start_SPEC_107_6_2() {
    let text = "abc";
    let boundaries = make_ascii_boundaries(text);

    // Click far left of first boundary
    let hit = hit_test_x_y(&boundaries, -100.0, 0.0);
    assert_eq!(
        hit, 0,
        "click left of line start should clamp to position 0"
    );
}

#[test]
fn mouse_click_x_clamps_right_of_line_end_SPEC_107_6_2() {
    let text = "abc"; // boundaries at 0,10,20,30 (sentinel)
    let boundaries = make_ascii_boundaries(text);

    // Click far right of last boundary
    let hit = hit_test_x_y(&boundaries, 1000.0, 0.0);
    assert_eq!(
        hit,
        text.len(),
        "click right of line end should clamp to text.len()"
    );
}

// =============================================================================
// SPEC-HAL-107 §6.3 — Vertical Resolution (Y axis)
// =============================================================================

#[test]
fn mouse_click_y_resolves_to_single_line_SPEC_107_6_3() {
    let text = "abc\ndef\nghi";
    let boundaries = make_multiline_boundaries(text);
    let lines = make_lines(text);

    // Click in middle of second line (y=25, line_height=20)
    let hit = hit_test(&boundaries, &lines, 5.0, 25.0);

    // Find the boundary for hit_pos and check its line_index
    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(boundary.line_index, 1, "y=25 should resolve to line 1");
}

#[test]
fn mouse_click_y_clamps_above_first_line_SPEC_107_6_3() {
    let text = "abc\ndef";
    let boundaries = make_multiline_boundaries(text);
    let lines = make_lines(text);

    // Click above first line
    let hit = hit_test(&boundaries, &lines, 5.0, -100.0);

    // Should resolve to first line (line_index 0)
    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "click above first line should clamp to line 0"
    );
}

#[test]
fn mouse_click_y_clamps_below_last_line_SPEC_107_6_3() {
    let text = "abc\ndef";
    let boundaries = make_multiline_boundaries(text);
    let lines = make_lines(text);

    // Click below last line
    let hit = hit_test(&boundaries, &lines, 5.0, 1000.0);

    // Should resolve to last line (line_index 1)
    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 1,
        "click below last line should clamp to last line"
    );
}

#[test]
fn mouse_click_y_uses_per_line_bounds_not_average_SPEC_107_6_3() {
    // SPEC-HAL-107 §6.3.6: Y resolution must use per-line vertical bounds, not average height
    //
    // Setup: Two lines with UNEQUAL heights
    // - Line 0: height=40 (tall), spans y=0..40
    // - Line 1: height=20 (normal), spans y=40..60
    //
    // Average height = (40+20)/2 = 30
    //
    // Bug demonstration:
    // - Click at y=35 (inside line 0's bounds 0..40)
    // - With average-height mapping: floor(35/30) = 1 → WRONG
    // - With per-line bounds: 0 <= 35 < 40 → line 0 → CORRECT

    // Create boundaries for "abc\ndef"
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
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 0,
            x_logical: 30.0,
        }, // newline
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 1,
            x_logical: 30.0,
        }, // sentinel
    ];

    // Lines with UNEQUAL heights (this is the key to exposing the bug)
    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 40.0,
            ascent: 30.0,
            descent: 10.0,
            height: 40.0, // TALL line
            baseline: 30.0,
        },
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0, // Normal line
            baseline: 14.0,
        },
    ];

    // Click at y=35, which is clearly inside line 0 (spans 0..40)
    let hit = hit_test(&boundaries, &lines, 5.0, 35.0);

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "y=35 must resolve to line 0 (spans 0..40), not line 1. Average-height mapping is forbidden."
    );
}

// =============================================================================
// SPEC-HAL-107 §6.4 — Combined X/Y Resolution
// =============================================================================

#[test]
fn mouse_click_resolves_line_before_x_SPEC_107_6_4() {
    let text = "short\nlongerline";
    let boundaries = make_multiline_boundaries(text);
    let lines = make_lines(text);

    // Click at x=100 on first line (y=10) - first line is short
    let hit = hit_test(&boundaries, &lines, 100.0, 10.0);

    // Should clamp to end of first line, not jump to second
    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "horizontal clamping should not cross lines"
    );
}

#[test]
fn mouse_click_never_crosses_lines_SPEC_107_6_4() {
    let text = "aa\nbbbbbbbb";
    let boundaries = make_multiline_boundaries(text);
    let lines = make_lines(text);

    // Click at x=70 on first line (which only has 2 chars at x=0,10)
    let hit = hit_test(&boundaries, &lines, 70.0, 10.0);

    // Must stay on line 0, not jump to line 1
    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "click must not cross to another line"
    );
}

#[test]
fn mouse_click_x_must_not_escape_resolved_line_SPEC_107_6_4() {
    // SPEC-HAL-107 §6.4: X resolution is strictly constrained to resolved line
    //
    // Setup:
    // - Line 0: "ab" (2 chars, boundaries at x=0, x=10, x=20)
    // - Line 1: "cdefghij" (8 chars, boundaries at x=0..80)
    //
    // Click at (x=50, y=5) — Y is clearly inside line 0
    // x=50 is closer to line 1's boundaries (x=50 is 'd' on line 1)
    // But X resolution must be constrained to line 0's boundaries
    //
    // If global-nearest was used: would pick line 1's boundary at x=50
    // With §6.4 enforcement: must stay on line 0, clamp to x=20 (end of line)

    let boundaries = vec![
        // Line 0: "ab\n"
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
        }, // newline
        // Line 1: "cdefghij"
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8,
            line_index: 1,
            x_logical: 40.0,
        },
        ClusterBoundary {
            byte_start: 8,
            byte_end: 9,
            line_index: 1,
            x_logical: 50.0,
        }, // 'h' at x=50
        ClusterBoundary {
            byte_start: 9,
            byte_end: 10,
            line_index: 1,
            x_logical: 60.0,
        },
        ClusterBoundary {
            byte_start: 10,
            byte_end: 11,
            line_index: 1,
            x_logical: 70.0,
        },
        ClusterBoundary {
            byte_start: 11,
            byte_end: 11,
            line_index: 1,
            x_logical: 80.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 80.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click at x=50, y=5 — clearly inside line 0 (y=0..20), but x=50 is closer to line 1's glyph
    let hit = hit_test(&boundaries, &lines, 50.0, 5.0);

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "X resolution must not escape to line 1 even though x=50 is closer to line 1's boundaries. \
         §6.4 requires X search be constrained to resolved line."
    );
}

// =============================================================================
// SPEC-HAL-107 §6.5 — Newline Semantics
// =============================================================================

#[test]
fn mouse_click_after_line_end_places_caret_before_newline_SPEC_107_6_5() {
    // SPEC-HAL-107 §6.5 Invariant A: End-of-line click
    // Clicking past visual end of line must land BEFORE newline, on last boundary of that line
    //
    // Setup: "abc\ndef"
    // Line 0: "abc\n" — boundaries at byte 0,1,2,3 (newline at byte 3)
    // Line 1: "def" — boundaries at byte 4,5,6,7

    let boundaries = vec![
        // Line 0: "abc\n"
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
        }, // 'b'
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        }, // 'c'
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 0,
            x_logical: 30.0,
        }, // '\n'
        // Line 1: "def"
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 0.0,
        }, // 'd'
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 10.0,
        }, // 'e'
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 20.0,
        }, // 'f'
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 1,
            x_logical: 30.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 40.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click far right on line 0 (x=100, y=10)
    let hit = hit_test(&boundaries, &lines, 100.0, 10.0);

    // Must land on line 0, at or before newline boundary (byte 3)
    assert!(
        hit <= 3,
        "click after line end must land at or before newline (byte 3), got {}",
        hit
    );

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "caret must stay on line 0, not jump to line 1"
    );
}

#[test]
fn mouse_click_on_newline_boundary_is_owned_by_previous_line_SPEC_107_6_5() {
    // SPEC-HAL-107 §6.5 Invariant B: Newline boundary legality
    // The newline character's boundary is part of the PRECEDING line
    // It must have line_index of the line it terminates, not the next line
    //
    // Setup: "ab\ncd"
    // Byte 2 is '\n' — it belongs to line 0

    let boundaries = vec![
        // Line 0: "ab\n"
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
        }, // 'b'
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        }, // '\n' — MUST be line_index 0
        // Line 1: "cd"
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 0.0,
        }, // 'c'
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 10.0,
        }, // 'd'
        ClusterBoundary {
            byte_start: 5,
            byte_end: 5,
            line_index: 1,
            x_logical: 20.0,
        }, // sentinel
    ];

    // Verify the newline boundary exists and is owned by line 0
    let newline_boundary = boundaries.iter().find(|b| b.byte_start == 2).unwrap();
    assert_eq!(
        newline_boundary.line_index, 0,
        "newline boundary at byte 2 must be owned by line 0 (the line it terminates)"
    );

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 20.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click at X position of newline on line 0 (x=20, y=10)
    let hit = hit_test(&boundaries, &lines, 20.0, 10.0);

    // Must land on newline boundary (byte 2) which is line 0
    let hit_boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        hit_boundary.line_index, 0,
        "clicking on newline position must resolve to line 0, not line 1"
    );
}

#[test]
fn mouse_click_must_not_skip_newline_to_next_line_SPEC_107_6_5() {
    // SPEC-HAL-107 §6.5 Invariant C: No implicit newline skipping
    // Mouse hit-testing must NEVER skip over a newline to reach a "closer" glyph on next line
    //
    // Setup: Short line followed by line starting at x=0
    // Line 0: "a\n" (ends at x=10, newline at x=10)
    // Line 1: "bcdef" (starts at x=0)
    //
    // Click at (x=5, y=10) on line 0
    // x=5 is closer to line 1's 'b' at x=0 than to line 0's newline at x=10
    // But we MUST stay on line 0

    let boundaries = vec![
        // Line 0: "a\n"
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
        }, // '\n'
        // Line 1: "bcdef"
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 1,
            x_logical: 0.0,
        }, // 'b'
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 10.0,
        }, // 'c'
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 20.0,
        }, // 'd'
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 30.0,
        }, // 'e'
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 40.0,
        }, // 'f'
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 1,
            x_logical: 50.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 20.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 50.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click at x=5 on line 0 (y=10)
    // This is between 'a' at x=0 and '\n' at x=10, should resolve to 'a' or '\n'
    let hit = hit_test(&boundaries, &lines, 5.0, 10.0);

    let hit_boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        hit_boundary.line_index, 0,
        "click on line 0 must not skip newline to reach line 1's 'b' at x=0"
    );

    // Also verify we didn't somehow land on line 1's boundary
    assert!(
        hit < 2,
        "hit position {} must be on line 0 (bytes 0-1), not line 1 (bytes 2+)",
        hit
    );
}

// =============================================================================
// SPEC-HAL-107 §6.6 — Empty / Short Line Handling
// =============================================================================

#[test]
fn mouse_click_on_empty_line_places_caret_at_line_start_SPEC_107_6_6() {
    // SPEC-HAL-107 §6.6 Invariant A: Empty line ownership
    // If a line has no glyph boundaries except newline, caret lands at line start
    //
    // Setup: "a\n\nb" — line 1 is empty (just a newline at byte 2)
    // Line 0: "a\n" (bytes 0-1)
    // Line 1: "\n" (byte 2 only — empty line)
    // Line 2: "b" (bytes 3-4)

    let boundaries = vec![
        // Line 0: "a\n"
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
        }, // '\n'
        // Line 1: empty line (only has the newline boundary)
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 1,
            x_logical: 0.0,
        }, // '\n' (this IS the empty line)
        // Line 2: "b"
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 2,
            x_logical: 0.0,
        }, // 'b'
        ClusterBoundary {
            byte_start: 4,
            byte_end: 4,
            line_index: 2,
            x_logical: 10.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 20.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 0.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        }, // empty line, width=0
        Line {
            runs: Vec::new(),
            width: 10.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click on empty line 1 (y=30, in range 20..40)
    let hit = hit_test(&boundaries, &lines, 50.0, 30.0);

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 1,
        "click on empty line must land on that empty line, not jump elsewhere"
    );
    assert_eq!(hit, 2, "caret must land at line start (byte 2)");
}

#[test]
fn mouse_click_on_consecutive_empty_lines_is_deterministic_SPEC_107_6_6() {
    // SPEC-HAL-107 §6.6 Invariant C: Determinism across multiple empty lines
    // For "a\n\n\nb", clicking on empty line 1 vs empty line 2 must be distinct
    //
    // Setup: "a\n\n\nb"
    // Line 0: "a\n" (bytes 0-1)
    // Line 1: "\n" (byte 2) — first empty line
    // Line 2: "\n" (byte 3) — second empty line
    // Line 3: "b" (bytes 4-5)

    let boundaries = vec![
        // Line 0: "a\n"
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
        }, // '\n'
        // Line 1: first empty line
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 1,
            x_logical: 0.0,
        }, // '\n'
        // Line 2: second empty line
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 2,
            x_logical: 0.0,
        }, // '\n'
        // Line 3: "b"
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 3,
            x_logical: 0.0,
        }, // 'b'
        ClusterBoundary {
            byte_start: 5,
            byte_end: 5,
            line_index: 3,
            x_logical: 10.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 20.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 0.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        }, // empty
        Line {
            runs: Vec::new(),
            width: 0.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        }, // empty
        Line {
            runs: Vec::new(),
            width: 10.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click on first empty line (line 1, y=30, in range 20..40)
    let hit1 = hit_test(&boundaries, &lines, 50.0, 30.0);
    let boundary1 = boundaries.iter().find(|b| b.byte_start == hit1).unwrap();
    assert_eq!(
        boundary1.line_index, 1,
        "click on line 1 must resolve to line 1"
    );
    assert_eq!(hit1, 2, "line 1 caret must be at byte 2");

    // Click on second empty line (line 2, y=50, in range 40..60)
    let hit2 = hit_test(&boundaries, &lines, 50.0, 50.0);
    let boundary2 = boundaries.iter().find(|b| b.byte_start == hit2).unwrap();
    assert_eq!(
        boundary2.line_index, 2,
        "click on line 2 must resolve to line 2"
    );
    assert_eq!(hit2, 3, "line 2 caret must be at byte 3");

    // Verify they are distinct
    assert_ne!(
        hit1, hit2,
        "consecutive empty lines must resolve to distinct positions"
    );
}

#[test]
fn mouse_click_on_zero_width_line_does_not_jump_SPEC_107_6_6() {
    // SPEC-HAL-107 §6.6 Invariant D: Zero-width line safety
    // If a line's layout width is 0.0, caret still lands at line start, never escapes
    //
    // Setup: Line with width=0 but valid boundary
    // Line 0: "abc\n" (normal)
    // Line 1: has boundary but width=0 (zero-width line)
    // Line 2: "def" (normal)

    let boundaries = vec![
        // Line 0: "abc\n"
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
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 0,
            x_logical: 30.0,
        }, // '\n'
        // Line 1: zero-width line (has single boundary at x=0)
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 0.0,
        }, // '\n' or single char
        // Line 2: "def"
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 2,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 2,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8,
            line_index: 2,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 8,
            byte_end: 8,
            line_index: 2,
            x_logical: 30.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 40.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 0.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        }, // zero width!
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click on zero-width line (line 1, y=30) with large X
    let hit = hit_test(&boundaries, &lines, 100.0, 30.0);

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(
        boundary.line_index, 1,
        "click on zero-width line must stay on that line, not jump to line 0 or 2"
    );
    assert_eq!(hit, 4, "caret must land at line 1 start (byte 4)");
}

#[test]
fn mouse_click_past_short_line_clamps_to_last_boundary_SPEC_107_6_6() {
    // Short line (not empty) — clicking past end clamps to last boundary on that line
    //
    // Setup: "ab\ncdefgh"
    // Line 0: "ab\n" (short, 3 chars)
    // Line 1: "cdefgh" (long)

    let boundaries = vec![
        // Line 0: "ab\n"
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
        }, // 'b'
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        }, // '\n'
        // Line 1: "cdefgh"
        ClusterBoundary {
            byte_start: 3,
            byte_end: 4,
            line_index: 1,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 5,
            line_index: 1,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 1,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 1,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 8,
            line_index: 1,
            x_logical: 40.0,
        },
        ClusterBoundary {
            byte_start: 8,
            byte_end: 9,
            line_index: 1,
            x_logical: 50.0,
        },
        ClusterBoundary {
            byte_start: 9,
            byte_end: 9,
            line_index: 1,
            x_logical: 60.0,
        }, // sentinel
    ];

    let lines = vec![
        Line {
            runs: Vec::new(),
            width: 30.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
        Line {
            runs: Vec::new(),
            width: 60.0,
            ascent: 14.0,
            descent: 6.0,
            height: 20.0,
            baseline: 14.0,
        },
    ];

    // Click far right on short line 0 (x=200, y=10)
    let hit = hit_test(&boundaries, &lines, 200.0, 10.0);

    let boundary = boundaries.iter().find(|b| b.byte_start == hit).unwrap();
    assert_eq!(boundary.line_index, 0, "must stay on short line 0");
    assert!(
        hit <= 2,
        "must clamp to last valid position on line 0 (byte 0, 1, or 2)"
    );
}

// =============================================================================
// SPEC-HAL-107 §6.7 — Selection Reset Rules
// =============================================================================

#[test]
fn mouse_plain_click_collapses_selection_SPEC_107_6_7() {
    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::default();
    let mut selection = Selection { anchor: 0, head: 5 }; // pre-existing selection

    // Plain click (no shift)
    resolve_click_single_line(25.0, &boundaries, &mut cursor, &mut selection, false);

    // Selection must be collapsed: anchor == head
    assert_eq!(
        selection.anchor, selection.head,
        "plain click must collapse selection"
    );
    assert_eq!(
        cursor.pos, selection.anchor,
        "cursor.pos must equal anchor after collapse"
    );
}

#[test]
fn mouse_shift_click_preserves_anchor_SPEC_107_6_7() {
    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor::default();
    let mut selection = Selection { anchor: 1, head: 1 }; // caret at position 1

    let original_anchor = selection.anchor;

    // Shift-click at different position
    resolve_click_single_line(35.0, &boundaries, &mut cursor, &mut selection, true);

    // Anchor must be preserved
    assert_eq!(
        selection.anchor, original_anchor,
        "shift-click must preserve anchor"
    );
    // Head must have moved
    assert_ne!(
        selection.head, original_anchor,
        "shift-click must move head"
    );
    assert_eq!(cursor.pos, selection.head, "cursor.pos must equal new head");
}

#[test]
fn mouse_click_clears_preferred_x_SPEC_107_6_7() {
    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);
    let mut cursor = TextCursor {
        pos: 0,
        preferred_x: Some(50.0),
        affinity: Default::default(),
    };
    let mut selection = Selection::default();

    // Any click (plain or shift) clears preferred_x
    resolve_click_single_line(25.0, &boundaries, &mut cursor, &mut selection, false);

    assert_eq!(
        cursor.preferred_x, None,
        "mouse click must clear preferred_x"
    );
}

// =============================================================================
// SPEC-HAL-107 §6.8 — Stability & Determinism
// =============================================================================

#[test]
fn identical_clicks_produce_identical_results_SPEC_107_6_8() {
    let text = "Test string";
    let boundaries = make_ascii_boundaries(text);

    let x = 35.0;
    let hit1 = hit_test_x_y(&boundaries, x, 0.0);
    let hit2 = hit_test_x_y(&boundaries, x, 0.0);
    let hit3 = hit_test_x_y(&boundaries, x, 0.0);

    assert_eq!(hit1, hit2);
    assert_eq!(hit2, hit3);
}

#[test]
fn mouse_resolution_independent_of_previous_cursor_position_SPEC_107_6_8() {
    let text = "abcdef";
    let boundaries = make_ascii_boundaries(text);

    // Same click, different starting cursor positions
    let mut cursor1 = TextCursor {
        pos: 0,
        ..Default::default()
    };
    let mut cursor2 = TextCursor {
        pos: 3,
        ..Default::default()
    };
    let mut cursor3 = TextCursor {
        pos: 6,
        ..Default::default()
    };
    let mut sel1 = Selection::default();
    let mut sel2 = Selection::default();
    let mut sel3 = Selection::default();

    let click_x = 25.0;
    resolve_click_single_line(click_x, &boundaries, &mut cursor1, &mut sel1, false);
    resolve_click_single_line(click_x, &boundaries, &mut cursor2, &mut sel2, false);
    resolve_click_single_line(click_x, &boundaries, &mut cursor3, &mut sel3, false);

    // All should resolve to same position regardless of starting cursor
    assert_eq!(cursor1.pos, cursor2.pos);
    assert_eq!(cursor2.pos, cursor3.pos);
}

// =============================================================================
// SPEC-HAL-107 §6.9 — UTF-8 Safety
// =============================================================================

#[test]
fn mouse_click_never_lands_inside_utf8_codepoint_SPEC_107_6_9() {
    // Test with emoji: "👋" is 4 bytes
    let text = "a👋b"; // bytes: a(1), 👋(4), b(1) = 6 total
    let mut boundaries = Vec::new();
    let char_width = 15.0;

    // 'a' at byte 0
    boundaries.push(ClusterBoundary {
        byte_start: 0,
        byte_end: 1,
        line_index: 0,
        x_logical: 0.0,
    });
    // '👋' at bytes 1..5
    boundaries.push(ClusterBoundary {
        byte_start: 1,
        byte_end: 5,
        line_index: 0,
        x_logical: char_width,
    });
    // 'b' at byte 5
    boundaries.push(ClusterBoundary {
        byte_start: 5,
        byte_end: 6,
        line_index: 0,
        x_logical: char_width * 2.0,
    });
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: 6,
        byte_end: 6,
        line_index: 0,
        x_logical: char_width * 3.0,
    });

    // Click various positions - must never land on 2, 3, 4 (inside emoji)
    for x in (0..100).map(|i| i as f32) {
        let hit = hit_test_x_y(&boundaries, x, 0.0);
        assert!(
            hit == 0 || hit == 1 || hit == 5 || hit == 6,
            "hit {} is inside UTF-8 codepoint (valid: 0, 1, 5, 6)",
            hit
        );
    }
}

// =============================================================================
// SPEC-HAL-107 §6.10 — Boundary Authority
// =============================================================================

#[test]
fn mouse_hit_testing_uses_only_existing_boundaries_SPEC_107_6_10() {
    // Create sparse boundaries (not every byte)
    let boundaries = vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 3,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 3,
            byte_end: 6,
            line_index: 0,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 6,
            line_index: 0,
            x_logical: 60.0,
        },
    ];

    // Any click must land on 0, 3, or 6
    for x in (0..100).map(|i| i as f32) {
        let hit = hit_test_x_y(&boundaries, x, 0.0);
        assert!(
            hit == 0 || hit == 3 || hit == 6,
            "hit {} does not match an existing boundary byte_start",
            hit
        );
    }
}

#[test]
fn mouse_hit_testing_must_not_compute_new_positions_SPEC_107_6_10() {
    // Even with unusual x_logical spacing, hit_test must use boundaries as-is
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
            x_logical: 100.0,
        }, // big gap
        ClusterBoundary {
            byte_start: 2,
            byte_end: 2,
            line_index: 0,
            x_logical: 105.0,
        },
    ];

    // Click at x=50 (middle of big gap) - must still land on existing boundary
    let hit = hit_test_x_y(&boundaries, 50.0, 0.0);
    assert!(
        hit == 0 || hit == 1 || hit == 2,
        "hit {} is not an existing boundary",
        hit
    );

    // Click at x=102 - should land on boundary at 100 or 105
    let hit = hit_test_x_y(&boundaries, 102.0, 0.0);
    assert!(
        hit == 0 || hit == 1 || hit == 2,
        "hit {} is not an existing boundary",
        hit
    );
}
