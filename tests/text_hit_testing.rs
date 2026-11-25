// SPEC-HAL-052 Stage 11: Caret Hit-Testing & Visual Mapping Tests

use halcyon::core::editing::{
    ClusterBoundary, hit_test_line, hit_test_x_y, visual_cluster_for_pos,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for single-line ASCII text.
/// Each cluster is 10 units wide in x_logical space, starting at x=0.
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

/// Create boundaries for multi-line text.
/// Each line has clusters at x_logical = 0, 10, 20, ...
/// Uses LINE_SLOT_HEIGHT = 20.0 for y-space mapping (caller's responsibility to convert pixels).
fn make_multi_line_boundaries(lines: &[&str]) -> Vec<ClusterBoundary> {
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

/// Create UTF-8 boundaries with variable cluster widths.
fn make_utf8_variable_width_boundaries(
    clusters: &[(usize, usize, usize, f32)],
) -> Vec<ClusterBoundary> {
    // Each tuple: (byte_start, byte_end, line_index, x_logical)
    clusters
        .iter()
        .map(
            |&(byte_start, byte_end, line_index, x_logical)| ClusterBoundary {
                byte_start,
                byte_end,
                line_index,
                x_logical,
            },
        )
        .collect()
}

// ============================================================================
// A. Horizontal Hit-Testing
// ============================================================================

#[test]
fn test_hit_exactly_on_cluster_center() {
    let boundaries = make_single_line_boundaries("hello");
    // Clusters at x = 0, 10, 20, 30, 40, sentinel at 50

    // Click exactly on 'e' (x=10)
    assert_eq!(hit_test_x_y(&boundaries, 10.0, 0.0), 1);

    // Click exactly on 'l' (x=20)
    assert_eq!(hit_test_x_y(&boundaries, 20.0, 0.0), 2);

    // Click exactly on 'o' (x=40)
    assert_eq!(hit_test_x_y(&boundaries, 40.0, 0.0), 4);
}

#[test]
fn test_hit_between_clusters() {
    let boundaries = make_single_line_boundaries("hello");
    // Clusters at x = 0, 10, 20, 30, 40, sentinel at 50

    // Click between 'h' (x=0) and 'e' (x=10) -> closer to 'h'
    assert_eq!(hit_test_x_y(&boundaries, 4.0, 0.0), 0);

    // Click between 'h' (x=0) and 'e' (x=10) -> closer to 'e'
    assert_eq!(hit_test_x_y(&boundaries, 6.0, 0.0), 1);

    // Click exactly midpoint (x=5) -> either is acceptable
    let pos = hit_test_x_y(&boundaries, 5.0, 0.0);
    assert!(pos == 0 || pos == 1);
}

#[test]
fn test_hit_far_left() {
    let boundaries = make_single_line_boundaries("hello");

    // Click far left (negative x)
    assert_eq!(hit_test_x_y(&boundaries, -100.0, 0.0), 0);

    // Click at x=0 exactly
    assert_eq!(hit_test_x_y(&boundaries, 0.0, 0.0), 0);
}

#[test]
fn test_hit_far_right_yields_sentinel() {
    let boundaries = make_single_line_boundaries("hello");
    // Sentinel at x=50

    // Click far right
    assert_eq!(hit_test_x_y(&boundaries, 100.0, 0.0), 5); // sentinel

    // Click past last cluster but before sentinel midpoint
    assert_eq!(hit_test_x_y(&boundaries, 44.0, 0.0), 4); // 'o' at x=40

    // Click closer to sentinel
    assert_eq!(hit_test_x_y(&boundaries, 46.0, 0.0), 5); // sentinel at x=50
}

// ============================================================================
// B. Vertical Hit-Testing
// ============================================================================

#[test]
fn test_hit_above_first_line() {
    let boundaries = make_multi_line_boundaries(&["abc", "def", "ghi"]);

    // Click above first line (negative y)
    assert_eq!(hit_test_line(&boundaries, -10.0), 0);
    assert_eq!(hit_test_line(&boundaries, -1.0), 0);
}

#[test]
fn test_hit_below_last_line() {
    let boundaries = make_multi_line_boundaries(&["abc", "def", "ghi"]);

    // Click far below last line
    // max_line = 2, LINE_SLOT_HEIGHT = 20.0, so y > 60 should return line 2
    assert_eq!(hit_test_line(&boundaries, 100.0), 2);
    assert_eq!(hit_test_line(&boundaries, 1000.0), 2);
}

#[test]
fn test_hit_between_lines() {
    let boundaries = make_multi_line_boundaries(&["abc", "def", "ghi"]);
    // LINE_SLOT_HEIGHT = 20.0
    // In hit-testing Y-space (not device pixels):
    // Line 0: y ∈ [0, 20)
    // Line 1: y ∈ [20, 40)
    // Line 2: y ∈ [40, 60)
    // (mapping real pixels → this space is caller's responsibility)

    // Click in line 0 region
    assert_eq!(hit_test_line(&boundaries, 5.0), 0);
    assert_eq!(hit_test_line(&boundaries, 15.0), 0);

    // Click in line 1 region
    assert_eq!(hit_test_line(&boundaries, 25.0), 1);
    assert_eq!(hit_test_line(&boundaries, 35.0), 1);

    // Click in line 2 region
    assert_eq!(hit_test_line(&boundaries, 45.0), 2);
    assert_eq!(hit_test_line(&boundaries, 55.0), 2);
}

#[test]
fn test_hit_single_line_always_returns_zero() {
    let boundaries = make_single_line_boundaries("hello");

    // Any y coordinate should return line 0
    assert_eq!(hit_test_line(&boundaries, -100.0), 0);
    assert_eq!(hit_test_line(&boundaries, 0.0), 0);
    assert_eq!(hit_test_line(&boundaries, 50.0), 0);
    assert_eq!(hit_test_line(&boundaries, 1000.0), 0);
}

// ============================================================================
// C. UTF-8 Cases
// ============================================================================

#[test]
fn test_utf8_variable_width_clusters() {
    // "Aéあ" - A (1 byte, width 10), é (2 bytes, width 15), あ (3 bytes, width 20)
    let boundaries = make_utf8_variable_width_boundaries(&[
        (0, 1, 0, 0.0),  // 'A' at x=0
        (1, 3, 0, 10.0), // 'é' at x=10
        (3, 6, 0, 25.0), // 'あ' at x=25
        (6, 6, 0, 45.0), // sentinel at x=45
    ]);

    // Click on 'A'
    assert_eq!(hit_test_x_y(&boundaries, 0.0, 0.0), 0);

    // Click between 'A' and 'é' (closer to 'é')
    assert_eq!(hit_test_x_y(&boundaries, 8.0, 0.0), 1);

    // Click on 'あ'
    assert_eq!(hit_test_x_y(&boundaries, 25.0, 0.0), 3);

    // Click between 'é' and 'あ' (closer to 'あ')
    assert_eq!(hit_test_x_y(&boundaries, 20.0, 0.0), 3);

    // Click far right -> sentinel
    assert_eq!(hit_test_x_y(&boundaries, 50.0, 0.0), 6);
}

#[test]
fn test_utf8_emoji_cluster() {
    // "a👍b" - emoji is 4 bytes
    let boundaries = make_utf8_variable_width_boundaries(&[
        (0, 1, 0, 0.0),  // 'a' at x=0
        (1, 5, 0, 10.0), // '👍' at x=10 (width 30 - wide emoji)
        (5, 6, 0, 40.0), // 'b' at x=40
        (6, 6, 0, 50.0), // sentinel at x=50
    ]);

    // Click on emoji
    assert_eq!(hit_test_x_y(&boundaries, 15.0, 0.0), 1);
    assert_eq!(hit_test_x_y(&boundaries, 25.0, 0.0), 1);

    // Click closer to 'b'
    assert_eq!(hit_test_x_y(&boundaries, 38.0, 0.0), 5);
}

#[test]
fn test_utf8_multiline() {
    // Line 0: "Привет" (12 bytes), Line 1: "мир" (6 bytes)
    let boundaries = make_utf8_variable_width_boundaries(&[
        // Line 0: П, р, и, в, е, т
        (0, 2, 0, 0.0),
        (2, 4, 0, 10.0),
        (4, 6, 0, 20.0),
        (6, 8, 0, 30.0),
        (8, 10, 0, 40.0),
        (10, 12, 0, 50.0),
        // Line 1: м, и, р
        (12, 14, 1, 0.0),
        (14, 16, 1, 10.0),
        (16, 18, 1, 20.0),
        // Sentinel
        (18, 18, 1, 30.0),
    ]);

    // Click on line 0, x=25 (between и and в)
    let pos = hit_test_x_y(&boundaries, 25.0, 5.0);
    let cluster = visual_cluster_for_pos(&boundaries, pos);
    assert_eq!(cluster.line_index, 0);

    // Click on line 1, x=15 (between м and и)
    let pos = hit_test_x_y(&boundaries, 15.0, 25.0);
    let cluster = visual_cluster_for_pos(&boundaries, pos);
    assert_eq!(cluster.line_index, 1);
}

// ============================================================================
// D. Round-trip Invariants
// ============================================================================

#[test]
fn test_roundtrip_hit_to_visual() {
    let boundaries = make_multi_line_boundaries(&["hello", "world"]);

    // Hit-test various positions and verify round-trip
    for x in [0.0, 15.0, 25.0, 45.0, 100.0] {
        for y in [0.0, 10.0, 25.0, 50.0] {
            let pos = hit_test_x_y(&boundaries, x, y);
            let cluster = visual_cluster_for_pos(&boundaries, pos);

            // The cluster's line should match what hit_test_line returns for that y
            let expected_line = hit_test_line(&boundaries, y);
            assert_eq!(
                cluster.line_index, expected_line,
                "Round-trip failed for ({}, {}): pos={}, cluster.line_index={}, expected_line={}",
                x, y, pos, cluster.line_index, expected_line
            );
        }
    }
}

#[test]
fn test_roundtrip_all_boundaries() {
    let boundaries = make_single_line_boundaries("hello");

    // For each boundary, visual_cluster_for_pos should return that exact boundary
    for b in &boundaries {
        let cluster = visual_cluster_for_pos(&boundaries, b.byte_start);
        assert_eq!(cluster.byte_start, b.byte_start);
        assert_eq!(cluster.line_index, b.line_index);
    }
}

#[test]
fn test_monotonic_hit_test() {
    let boundaries = make_single_line_boundaries("hello");

    // Moving right should never decrease the returned position
    let mut prev_pos = 0;
    for x in 0..60 {
        let pos = hit_test_x_y(&boundaries, x as f32, 0.0);
        assert!(
            pos >= prev_pos,
            "Hit test not monotonic: x={}, prev_pos={}, pos={}",
            x,
            prev_pos,
            pos
        );
        prev_pos = pos;
    }
}

// ============================================================================
// E. Sentinel Behavior
// ============================================================================

#[test]
fn test_sentinel_x_far_right() {
    let boundaries = make_single_line_boundaries("abc");
    // Sentinel at byte 3, x=30

    // Far right always yields sentinel
    assert_eq!(hit_test_x_y(&boundaries, 100.0, 0.0), 3);
    assert_eq!(hit_test_x_y(&boundaries, 50.0, 0.0), 3);
    assert_eq!(hit_test_x_y(&boundaries, 35.0, 0.0), 3);
}

#[test]
fn test_sentinel_y_far_below() {
    let boundaries = make_multi_line_boundaries(&["abc", "def"]);
    // Last line is 1, sentinel at byte 6

    // Far below returns last line
    let line = hit_test_line(&boundaries, 1000.0);
    assert_eq!(line, 1);

    // Hit test far below, far right -> sentinel
    let pos = hit_test_x_y(&boundaries, 100.0, 1000.0);
    assert_eq!(pos, 6); // sentinel byte_start
}

#[test]
fn test_sentinel_visual_cluster() {
    let boundaries = make_single_line_boundaries("hello");
    // Sentinel at byte 5, x=50, line 0

    let cluster = visual_cluster_for_pos(&boundaries, 5);
    assert_eq!(cluster.byte_start, 5);
    assert_eq!(cluster.byte_end, 5);
    assert_eq!(cluster.line_index, 0);
    assert_eq!(cluster.x_logical, 50.0);
}

// ============================================================================
// F. Edge Cases
// ============================================================================

#[test]
fn test_empty_boundaries() {
    let boundaries: Vec<ClusterBoundary> = vec![];

    assert_eq!(hit_test_x_y(&boundaries, 10.0, 10.0), 0);
    assert_eq!(hit_test_line(&boundaries, 10.0), 0);

    let cluster = visual_cluster_for_pos(&boundaries, 0);
    assert_eq!(cluster.byte_start, 0);
    assert_eq!(cluster.line_index, 0);
}

#[test]
fn test_single_sentinel_only() {
    let boundaries = vec![ClusterBoundary {
        byte_start: 0,
        byte_end: 0,
        line_index: 0,
        x_logical: 0.0,
    }];

    // Any click returns sentinel
    assert_eq!(hit_test_x_y(&boundaries, 0.0, 0.0), 0);
    assert_eq!(hit_test_x_y(&boundaries, 100.0, 100.0), 0);

    let cluster = visual_cluster_for_pos(&boundaries, 0);
    assert_eq!(cluster.byte_start, 0);
}

#[test]
fn test_visual_cluster_for_invalid_pos() {
    let boundaries = make_single_line_boundaries("hello");

    // Position not on a boundary - should find nearest at or after
    let cluster = visual_cluster_for_pos(&boundaries, 100);
    // Beyond end - returns sentinel
    assert_eq!(cluster.byte_start, 5);
}

#[test]
fn test_click_at_line_boundary() {
    let boundaries = make_multi_line_boundaries(&["abc", "def"]);
    // Line 0: bytes 0-2, Line 1: bytes 3-5, sentinel at 6

    // Click at y exactly at line boundary (y=20, between line 0 and 1)
    let line = hit_test_line(&boundaries, 20.0);
    assert_eq!(line, 1); // y=20 / LINE_SLOT_HEIGHT=20 = 1
}
