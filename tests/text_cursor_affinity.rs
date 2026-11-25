// SPEC-HAL-052 Stage 13: Grapheme & Affinity Test Suite
//
// Canonical test module for cursor affinity and grapheme-boundary behavior.
// This locks down how caret moves through clusters, especially at cluster edges
// and between multi-codepoint graphemes.
//
// These tests form the base contract for all future editing widgets.
// No TextField, no widget state, no layout cache - pure substrate.

use halcyon::core::editing::{
    ClusterBoundary, CursorAffinity, Selection, TextCursor, move_down, move_left_collapse,
    move_left_select, move_right_collapse, move_right_select, move_up,
};

// ============================================================================
// Test Helpers - Invariant-Compatible Boundary Builders
// ============================================================================

/// Create boundaries for simple ASCII text where each char is one cluster.
///
/// Invariants enforced:
/// - boundaries[0].byte_start == 0, x_logical == 0.0
/// - Each ASCII char becomes one cluster
/// - Sentinel at text.len()
/// - All line_index == 0
fn make_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
    assert!(text.is_ascii(), "make_ascii_boundaries requires ASCII text");

    let mut boundaries = Vec::with_capacity(text.len() + 1);

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

/// Create boundaries for multiline ASCII text.
///
/// Each line's characters become individual clusters.
/// Sentinel is placed on the last line.
fn make_multiline_boundaries(lines: &[&str]) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut byte_offset = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        for (i, _) in line.char_indices() {
            boundaries.push(ClusterBoundary {
                byte_start: byte_offset + i,
                byte_end: byte_offset + i + 1,
                line_index: line_idx,
                x_logical: i as f32 * 10.0,
            });
        }
        byte_offset += line.len();
    }

    // Sentinel on last line
    let last_line = if lines.is_empty() { 0 } else { lines.len() - 1 };
    boundaries.push(ClusterBoundary {
        byte_start: byte_offset,
        byte_end: byte_offset,
        line_index: last_line,
        x_logical: lines.last().map(|l| l.len() as f32 * 10.0).unwrap_or(0.0),
    });

    boundaries
}

/// Create boundaries simulating grapheme clusters from shaper output.
///
/// This simulates real cluster shapes:
/// - "H́" (H + combining acute): bytes [0..1], [1..3] - two clusters for combining
/// - "ﬁ" (fi ligature): bytes [3..5] - one cluster for ligature
/// - Regular chars after
///
/// Text conceptually: "H́ﬁab" = H(0) + combining(1-3) + fi_lig(3-5) + a(5) + b(6)
/// But shaper produces clusters based on glyph output, not input codepoints.
fn make_grapheme_boundaries() -> Vec<ClusterBoundary> {
    // Simulating: "H" + combining_acute (2 bytes) + "fi" ligature (2 bytes) + "ab"
    // Total: 7 bytes conceptually
    // Cluster map:
    //   [0..1]  H           x=0
    //   [1..3]  combining   x=10  (2-byte combining mark treated as cluster)
    //   [3..5]  fi ligature x=20  (2-byte ligature = 1 cluster)
    //   [5..6]  a           x=30
    //   [6..7]  b           x=40
    //   [7..7]  sentinel    x=50
    vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 1,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 1,
            byte_end: 3,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 3,
            byte_end: 5,
            line_index: 0,
            x_logical: 20.0,
        },
        ClusterBoundary {
            byte_start: 5,
            byte_end: 6,
            line_index: 0,
            x_logical: 30.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 7,
            line_index: 0,
            x_logical: 40.0,
        },
        ClusterBoundary {
            byte_start: 7,
            byte_end: 7,
            line_index: 0,
            x_logical: 50.0,
        }, // sentinel
    ]
}

/// Create boundaries simulating emoji ZWJ sequence.
///
/// "👨‍👩‍👧‍👦" family emoji is multiple codepoints joined by ZWJ:
/// 👨 (4 bytes) + ZWJ (3 bytes) + 👩 (4 bytes) + ZWJ (3 bytes) + 👧 (4 bytes) + ZWJ (3 bytes) + 👦 (4 bytes)
/// = 25 bytes total
///
/// Shaper may produce multiple clusters or one depending on font support.
/// We simulate a font that renders each emoji as separate cluster:
fn make_emoji_boundaries() -> Vec<ClusterBoundary> {
    // Simulating family emoji broken into constituent parts by shaper
    // Each emoji base is 4 bytes, ZWJ is 3 bytes
    // Cluster boundaries at each emoji:
    //   [0..4]   👨        x=0
    //   [4..7]   ZWJ       x=10  (or merged with next)
    //   [7..11]  👩        x=20
    //   [11..14] ZWJ       x=30
    //   [14..18] 👧        x=40
    //   [18..21] ZWJ       x=50
    //   [21..25] 👦        x=60
    //   [25..25] sentinel  x=70
    //
    // For simplicity, simulate as 4 emoji clusters (ZWJ merged):
    vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 7,
            line_index: 0,
            x_logical: 0.0,
        }, // 👨+ZWJ
        ClusterBoundary {
            byte_start: 7,
            byte_end: 14,
            line_index: 0,
            x_logical: 10.0,
        }, // 👩+ZWJ
        ClusterBoundary {
            byte_start: 14,
            byte_end: 21,
            line_index: 0,
            x_logical: 20.0,
        }, // 👧+ZWJ
        ClusterBoundary {
            byte_start: 21,
            byte_end: 25,
            line_index: 0,
            x_logical: 30.0,
        }, // 👦
        ClusterBoundary {
            byte_start: 25,
            byte_end: 25,
            line_index: 0,
            x_logical: 40.0,
        }, // sentinel
    ]
}

/// Create boundaries simulating combining character sequence.
///
/// "é" can be e (1 byte) + combining acute (2 bytes) = 3 bytes
/// Or precomposed é (2 bytes)
///
/// We simulate decomposed form where shaper keeps them as separate clusters:
fn make_combining_boundaries() -> Vec<ClusterBoundary> {
    // Text: "café" where é is decomposed (e + combining acute)
    // c(0) + a(1) + f(2) + e(3) + combining(4-6)
    // But shaper may merge e+combining into one cluster
    //
    // Simulate shaper that keeps base + combining as separate:
    //   [0..1]  c           x=0
    //   [1..2]  a           x=10
    //   [2..3]  f           x=20
    //   [3..4]  e (base)    x=30
    //   [4..6]  combining   x=40  (2-byte combining accent)
    //   [6..6]  sentinel    x=50
    vec![
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
        },
        ClusterBoundary {
            byte_start: 4,
            byte_end: 6,
            line_index: 0,
            x_logical: 40.0,
        },
        ClusterBoundary {
            byte_start: 6,
            byte_end: 6,
            line_index: 0,
            x_logical: 50.0,
        }, // sentinel
    ]
}

/// Create boundaries for ligature simulation.
///
/// "fib" where "fi" is a ligature (2 input bytes → 1 visual cluster)
fn make_ligature_boundaries() -> Vec<ClusterBoundary> {
    // Cluster 0: bytes 0-2 (the "fi" ligature), x=0
    // Cluster 1: byte 2-3 ("b"), x=10
    // Sentinel: byte 3, x=20
    vec![
        ClusterBoundary {
            byte_start: 0,
            byte_end: 2,
            line_index: 0,
            x_logical: 0.0,
        },
        ClusterBoundary {
            byte_start: 2,
            byte_end: 3,
            line_index: 0,
            x_logical: 10.0,
        },
        ClusterBoundary {
            byte_start: 3,
            byte_end: 3,
            line_index: 0,
            x_logical: 20.0,
        },
    ]
}

// ============================================================================
// Canonical Test 1: test_affinity_collapse_left_right
// ============================================================================

#[test]
fn test_affinity_collapse_left_right() {
    // Given a selection [0, 3], verify collapse behavior and affinity
    let boundaries = make_ascii_boundaries("hello");

    // Setup: selection from 0 to 3, cursor at head (3)
    let mut cursor = TextCursor::at(3);
    cursor.affinity = CursorAffinity::Right;
    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 3;

    // Collapse left: should go to selection.start() with affinity = Left
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0, "collapse left should go to selection start");
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "collapse left sets affinity Left"
    );
    assert!(selection.is_empty(), "selection should be collapsed");

    // Reset: selection from 0 to 3
    cursor = TextCursor::at(0);
    cursor.affinity = CursorAffinity::Left;
    selection.anchor = 3;
    selection.head = 0; // reversed selection

    // Collapse right: should go to selection.end() with affinity = Right
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 3, "collapse right should go to selection end");
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "collapse right sets affinity Right"
    );
    assert!(selection.is_empty(), "selection should be collapsed");
}

// ============================================================================
// Canonical Test 2: test_affinity_boundary_jump_multi_byte_cluster
// ============================================================================

#[test]
fn test_affinity_boundary_jump_multi_byte_cluster() {
    // Verify cursor jumps entire multi-byte cluster, not interior bytes
    let boundaries = make_ligature_boundaries();
    // Clusters: [0..2] fi ligature, [2..3] b, [3..3] sentinel

    // Start at byte 2 (after ligature)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Move left: should jump to byte 0 (start of ligature), not byte 1
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 0,
        "left from after ligature jumps to ligature start"
    );
    assert_eq!(cursor.affinity, CursorAffinity::Left);

    // Move right from byte 0: should jump to byte 2 (end of ligature)
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 2,
        "right from ligature start jumps to ligature end"
    );
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

// ============================================================================
// Canonical Test 3: test_affinity_interior_byte_positions_behavior
// ============================================================================

#[test]
fn test_affinity_interior_byte_positions_behavior() {
    // Cursor.pos must always be at a boundary.byte_start
    // This test verifies that movement always lands on valid positions
    let boundaries = make_grapheme_boundaries();
    // Clusters: [0..1], [1..3], [3..5], [5..6], [6..7], [7..7]

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Navigate right through all clusters
    let expected_positions = vec![0, 1, 3, 5, 6, 7];
    let mut visited = vec![cursor.pos];

    for _ in 0..10 {
        let old_pos = cursor.pos;
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
        if cursor.pos != old_pos {
            visited.push(cursor.pos);
        }
    }

    // Should only land on valid byte_start positions
    assert_eq!(
        visited, expected_positions,
        "cursor only lands on boundary.byte_start values"
    );

    // Verify we never land on interior bytes (2, 4 are interior)
    for pos in &visited {
        assert!(*pos != 2, "should never land on interior byte 2");
        assert!(*pos != 4, "should never land on interior byte 4");
    }
}

// ============================================================================
// Canonical Test 4: test_round_trip_affinity
// ============================================================================

#[test]
fn test_round_trip_affinity() {
    // Moving right N times then left N times should return to original pos
    // Affinity should also follow predictable pattern
    let boundaries = make_ascii_boundaries("hello");

    let mut cursor = TextCursor::at(0);
    cursor.affinity = CursorAffinity::Left;
    let mut selection = Selection::at(0);

    let original_pos = cursor.pos;
    let original_affinity = cursor.affinity;

    // Move right 3 times
    for _ in 0..3 {
        move_right_collapse(&mut cursor, &mut selection, &boundaries);
    }
    assert_eq!(cursor.pos, 3);
    assert_eq!(cursor.affinity, CursorAffinity::Right);

    // Move left 3 times
    for _ in 0..3 {
        move_left_collapse(&mut cursor, &mut selection, &boundaries);
    }

    assert_eq!(
        cursor.pos, original_pos,
        "round trip returns to original position"
    );
    assert_eq!(
        cursor.affinity, original_affinity,
        "round trip returns to original affinity"
    );
}

// ============================================================================
// Canonical Test 5: test_boundary_edge_double_role_start_end
// ============================================================================

#[test]
fn test_boundary_edge_double_role_start_end() {
    // Byte 2 is both cluster_end(ligature) and cluster_start(b)
    // Movement must choose the correct side
    let boundaries = make_ligature_boundaries();
    // [0..2] fi, [2..3] b, [3..3] sentinel
    // Byte 2 is: end of cluster 0, start of cluster 1

    // At byte 2, moving left goes to cluster 0's start (byte 0)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 0,
        "left from byte 2 goes to previous cluster start"
    );

    // At byte 2, moving right goes to cluster 2's start (byte 3)
    cursor = TextCursor::at(2);
    selection = Selection::at(2);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 3,
        "right from byte 2 goes to next cluster start"
    );
}

// ============================================================================
// Canonical Test 6: test_sentinel_affinity
// ============================================================================

#[test]
fn test_sentinel_affinity() {
    let boundaries = make_ascii_boundaries("hello");
    // Sentinel at byte 5

    // At sentinel, move_right is no-op
    let mut cursor = TextCursor::at(5);
    cursor.affinity = CursorAffinity::Left; // Start with Left
    let mut selection = Selection::at(5);

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 5, "right at sentinel is no-op");
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "right sets affinity Right even if no-op"
    );

    // At sentinel, move_left steps into last cluster
    cursor = TextCursor::at(5);
    cursor.affinity = CursorAffinity::Right;
    selection = Selection::at(5);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 4, "left from sentinel enters last cluster");
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "left sets affinity Left"
    );

    // Moving back to sentinel
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 5);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "returning to sentinel sets Right"
    );
}

// ============================================================================
// Canonical Test 7: test_emoji_cluster_sequence_navigation
// ============================================================================

#[test]
fn test_emoji_cluster_sequence_navigation() {
    let boundaries = make_emoji_boundaries();
    // Clusters: [0..7], [7..14], [14..21], [21..25], [25..25] sentinel

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Navigate through all emoji clusters
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 7,
        "first right jumps entire first emoji cluster"
    );

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 14, "second right jumps second emoji cluster");

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 21, "third right jumps third emoji cluster");

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 25, "fourth right reaches sentinel");

    // Navigate back
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 21);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 14);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 7);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0, "back to start");
}

// ============================================================================
// Canonical Test 8: test_ligature_cluster_navigation
// ============================================================================

#[test]
fn test_ligature_cluster_navigation() {
    let boundaries = make_ligature_boundaries();
    // "fib" where fi is ligature: [0..2], [2..3], [3..3]

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Right skips entire ligature
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 2, "right skips entire fi ligature");
    assert_eq!(cursor.affinity, CursorAffinity::Right);

    // Right to 'b'
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 3, "right to sentinel");

    // Left back through
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 2, "left from sentinel to b start");

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0, "left from b to ligature start");
    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

// ============================================================================
// Canonical Test 9: test_combining_sequence_cluster_navigation
// ============================================================================

#[test]
fn test_combining_sequence_cluster_navigation() {
    let boundaries = make_combining_boundaries();
    // "café" decomposed: [0..1]c, [1..2]a, [2..3]f, [3..4]e, [4..6]combining, [6..6]sentinel

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Navigate to 'e' base
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 1); // a
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 2); // f
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 3); // e base

    // Next right jumps to combining cluster
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 4,
        "right from e base goes to combining cluster start"
    );

    // Next right jumps entire combining cluster to sentinel
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 6, "right from combining goes to sentinel");

    // Left back through combining
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 4, "left from sentinel to combining start");

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 3, "left from combining to e base");
}

// ============================================================================
// Canonical Test 10: test_affinity_after_selection_collapse
// ============================================================================

#[test]
fn test_affinity_after_selection_collapse() {
    let boundaries = make_ascii_boundaries("hello");

    // Selection [1, 4], collapse left
    let mut cursor = TextCursor::at(4);
    cursor.affinity = CursorAffinity::Right;
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4;

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 1);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "collapse left sets Left affinity"
    );

    // Selection [1, 4], collapse right
    cursor = TextCursor::at(1);
    cursor.affinity = CursorAffinity::Left;
    selection.anchor = 4;
    selection.head = 1;

    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 4);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "collapse right sets Right affinity"
    );

    // Verify selection is cleared
    assert!(selection.is_empty());
}

// ============================================================================
// Canonical Test 11: test_affinity_does_not_flip_spontaneously
// ============================================================================

#[test]
fn test_affinity_does_not_flip_spontaneously() {
    let boundaries = make_ascii_boundaries("hello");

    // Start with Left affinity
    let mut cursor = TextCursor::at(2);
    cursor.affinity = CursorAffinity::Left;
    let mut selection = Selection::at(2);

    // Move right sets Right
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.affinity, CursorAffinity::Right);

    // Move right again keeps Right
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "consecutive rights keep Right"
    );

    // Move left sets Left
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.affinity, CursorAffinity::Left);

    // Move left again keeps Left
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "consecutive lefts keep Left"
    );

    // At boundary (pos 0), left is no-op but still sets Left
    cursor = TextCursor::at(0);
    cursor.affinity = CursorAffinity::Right;
    selection = Selection::at(0);

    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 0, "left at start is no-op");
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "left at boundary still sets Left"
    );
}

// ============================================================================
// Canonical Test 12: test_affinity_vertical_noop_preserves_affinity
// ============================================================================

#[test]
fn test_affinity_vertical_noop_preserves_affinity() {
    let boundaries = make_multiline_boundaries(&["hello", "world"]);

    // On line 0, move_up is effectively no-op (goes to start of line)
    // But should preserve affinity per Stage-13 rules
    let mut cursor = TextCursor::at(2);
    cursor.affinity = CursorAffinity::Right;
    let mut selection = Selection::at(2);

    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "up preserves Right affinity"
    );

    // On line 1, move_down is no-op (already on last line)
    cursor = TextCursor::at(7); // 'o' in "world"
    cursor.affinity = CursorAffinity::Left;
    selection = Selection::at(7);

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Left,
        "down preserves Left affinity"
    );

    // Vertical movement between lines also preserves affinity
    cursor = TextCursor::at(2);
    cursor.affinity = CursorAffinity::Right;
    selection = Selection::at(2);

    move_down(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "down to next line preserves affinity"
    );

    move_up(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "up to previous line preserves affinity"
    );
}

// ============================================================================
// Additional supporting tests (from original Stage-13 suite)
// ============================================================================

#[test]
fn test_new_cursor_has_left_affinity() {
    let cursor = TextCursor::new();
    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

#[test]
fn test_cursor_at_has_left_affinity() {
    let cursor = TextCursor::at(5);
    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

#[test]
fn test_cursor_at_with_affinity() {
    let cursor = TextCursor::at_with_affinity(5, CursorAffinity::Right);
    assert_eq!(cursor.pos, 5);
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

#[test]
fn test_shift_left_sets_affinity_left() {
    let boundaries = make_ascii_boundaries("hello");

    let mut cursor = TextCursor::at(3);
    cursor.affinity = CursorAffinity::Right;
    let mut selection = Selection::at(3);

    move_left_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 2);
    assert_eq!(cursor.affinity, CursorAffinity::Left);
    assert!(!selection.is_empty(), "shift+left creates selection");
}

#[test]
fn test_shift_right_sets_affinity_right() {
    let boundaries = make_ascii_boundaries("hello");

    let mut cursor = TextCursor::at(2);
    cursor.affinity = CursorAffinity::Left;
    let mut selection = Selection::at(2);

    move_right_select(&mut cursor, &mut selection, &boundaries);

    assert_eq!(cursor.pos, 3);
    assert_eq!(cursor.affinity, CursorAffinity::Right);
    assert!(!selection.is_empty(), "shift+right creates selection");
}

#[test]
fn test_collapse_to_preserves_affinity() {
    let mut cursor = TextCursor::at(0);
    cursor.affinity = CursorAffinity::Right;

    cursor.collapse_to(5);

    assert_eq!(cursor.pos, 5);
    assert_eq!(
        cursor.affinity,
        CursorAffinity::Right,
        "collapse_to preserves current affinity"
    );
}

#[test]
fn test_collapse_to_with_affinity() {
    let mut cursor = TextCursor::at(0);
    cursor.affinity = CursorAffinity::Left;

    cursor.collapse_to_with_affinity(5, CursorAffinity::Right);

    assert_eq!(cursor.pos, 5);
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}

#[test]
fn test_text_cursor_move_left_sets_affinity() {
    let boundaries = make_ascii_boundaries("hello");

    let mut cursor = TextCursor::at(3);
    cursor.affinity = CursorAffinity::Right;

    cursor.move_left(&boundaries);

    assert_eq!(cursor.pos, 2);
    assert_eq!(cursor.affinity, CursorAffinity::Left);
}

#[test]
fn test_text_cursor_move_right_sets_affinity() {
    let boundaries = make_ascii_boundaries("hello");

    let mut cursor = TextCursor::at(2);
    cursor.affinity = CursorAffinity::Left;

    cursor.move_right(&boundaries);

    assert_eq!(cursor.pos, 3);
    assert_eq!(cursor.affinity, CursorAffinity::Right);
}
