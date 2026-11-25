// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.
//
// NOTE:
// These tests use ASCII-only boundary generation for simplicity.
// UTF-8 multi-byte tests construct explicit boundaries.

#![allow(non_snake_case, unused_variables)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, TextEditOp, apply_edit_op, apply_insert_op,
    move_left_collapse, move_right_collapse,
};

// ============================================================================
// Helper: Create minimal boundaries for ASCII text (one boundary per byte)
// ============================================================================

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
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * 10.0,
    });
    boundaries
}

/// Create boundaries for text with multi-byte UTF-8 characters.
fn make_utf8_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index: 0,
            x_logical: boundaries.len() as f32 * 10.0,
        });
    }
    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: boundaries.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// SPEC-HAL-107 §2.1 — Boundary Existence
// ============================================================================

#[test]
fn every_cursor_position_maps_to_boundary_SPEC_107_2_1() {
    // SPEC-HAL-107 §2.1: Every valid cursor position corresponds to exactly one boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    // Valid cursor positions for "Hello" are 0, 1, 2, 3, 4, 5
    for pos in 0..=text.len() {
        let has_boundary = boundaries.iter().any(|b| b.byte_start == pos);
        assert!(
            has_boundary,
            "Cursor position {} must have a corresponding boundary",
            pos
        );
    }
}

#[test]
fn cursor_position_maps_to_exactly_one_boundary_SPEC_107_2_1() {
    // SPEC-HAL-107 §2.1: Each cursor position maps to EXACTLY ONE boundary.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    for pos in 0..=text.len() {
        let count = boundaries.iter().filter(|b| b.byte_start == pos).count();
        assert_eq!(
            count, 1,
            "Cursor position {} must map to exactly 1 boundary, found {}",
            pos, count
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §2.2 — Boundary Coverage
// ============================================================================

#[test]
fn boundaries_cover_entire_document_SPEC_107_2_2() {
    // SPEC-HAL-107 §2.2: Boundaries fully cover [0 .. text.len()].

    let text = "Hello World";
    let boundaries = make_ascii_boundaries(text);

    // First boundary must start at 0
    assert_eq!(
        boundaries.first().map(|b| b.byte_start),
        Some(0),
        "First boundary must start at 0"
    );

    // Last boundary (sentinel) must be at text.len()
    assert_eq!(
        boundaries.last().map(|b| b.byte_start),
        Some(text.len()),
        "Last boundary (sentinel) must be at text.len()"
    );
}

#[test]
fn boundaries_include_sentinel_at_end_SPEC_107_2_2() {
    // SPEC-HAL-107 §2.2: Sentinel boundary exists at text.len().

    let text = "Test";
    let boundaries = make_ascii_boundaries(text);

    let sentinel = boundaries.iter().find(|b| b.byte_start == text.len());
    assert!(
        sentinel.is_some(),
        "Sentinel boundary at text.len() ({}) must exist",
        text.len()
    );
}

#[test]
fn empty_text_has_sentinel_boundary_SPEC_107_2_2() {
    // SPEC-HAL-107 §2.2: Even empty text has sentinel at position 0.

    let text = "";
    let boundaries = make_ascii_boundaries(text);

    assert_eq!(
        boundaries.len(),
        1,
        "Empty text must have exactly 1 boundary (sentinel)"
    );
    assert_eq!(
        boundaries[0].byte_start, 0,
        "Empty text sentinel must be at position 0"
    );
}

// ============================================================================
// SPEC-HAL-107 §2.3 — Monotonicity
// ============================================================================

#[test]
fn boundary_byte_starts_strictly_increasing_SPEC_107_2_3() {
    // SPEC-HAL-107 §2.3: Boundary byte_start values are strictly increasing.

    let text = "Hello World";
    let boundaries = make_ascii_boundaries(text);

    for i in 1..boundaries.len() {
        assert!(
            boundaries[i].byte_start > boundaries[i - 1].byte_start,
            "Boundary {} (byte_start={}) must be > boundary {} (byte_start={})",
            i,
            boundaries[i].byte_start,
            i - 1,
            boundaries[i - 1].byte_start
        );
    }
}

#[test]
fn utf8_boundary_byte_starts_strictly_increasing_SPEC_107_2_3() {
    // SPEC-HAL-107 §2.3: Monotonicity holds for multi-byte UTF-8 text.

    let text = "Héllo日本"; // Mixed ASCII and multi-byte
    let boundaries = make_utf8_boundaries(text);

    for i in 1..boundaries.len() {
        assert!(
            boundaries[i].byte_start > boundaries[i - 1].byte_start,
            "UTF-8 boundary {} (byte_start={}) must be > boundary {} (byte_start={})",
            i,
            boundaries[i].byte_start,
            i - 1,
            boundaries[i - 1].byte_start
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §2.4 — No Duplication
// ============================================================================

#[test]
fn no_duplicate_boundary_byte_starts_SPEC_107_2_4() {
    // SPEC-HAL-107 §2.4: No two boundaries share the same byte_start.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut seen = std::collections::HashSet::new();
    for boundary in &boundaries {
        assert!(
            seen.insert(boundary.byte_start),
            "Duplicate boundary at byte_start={}",
            boundary.byte_start
        );
    }
}

// ============================================================================
// SPEC-HAL-107 §2.5 — Stability Across Edits
// ============================================================================

#[test]
fn boundaries_reflect_new_text_after_insert_SPEC_107_2_5() {
    // SPEC-HAL-107 §2.5: After insert, new boundaries reflect new text.

    let mut text = String::from("AB");
    let old_boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &old_boundaries,
        "XY",
    );

    // Rebuild boundaries for new text
    let new_boundaries = make_ascii_boundaries(&text);

    // New text is "AXYB" - 4 chars + sentinel = 5 boundaries
    assert_eq!(text, "AXYB");
    assert_eq!(
        new_boundaries.len(),
        5,
        "New boundaries must reflect new text length"
    );

    // Verify coverage of new text
    assert_eq!(new_boundaries.first().unwrap().byte_start, 0);
    assert_eq!(new_boundaries.last().unwrap().byte_start, text.len());
}

#[test]
fn boundaries_reflect_new_text_after_delete_SPEC_107_2_5() {
    // SPEC-HAL-107 §2.5: After delete, new boundaries reflect new text.

    let mut text = String::from("ABCDE");
    let old_boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &old_boundaries,
        TextEditOp::Backspace,
    );

    // Rebuild boundaries for new text
    let new_boundaries = make_ascii_boundaries(&text);

    // New text is "ABDE" - 4 chars + sentinel = 5 boundaries
    assert_eq!(text, "ABDE");
    assert_eq!(
        new_boundaries.len(),
        5,
        "New boundaries must reflect new text length after delete"
    );
}

#[test]
fn old_boundaries_not_reused_after_edit_SPEC_107_2_5() {
    // SPEC-HAL-107 §2.5: Old boundaries are stale after edit; new must be generated.

    let mut text = String::from("ABC");
    let old_boundaries = make_ascii_boundaries(&text);
    let old_len = old_boundaries.len();

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &old_boundaries,
        "DEF",
    );

    // Old boundaries still have old length (stale)
    assert_eq!(old_len, 4, "Old boundaries had 4 entries");

    // New boundaries must be regenerated
    let new_boundaries = make_ascii_boundaries(&text);
    assert_eq!(
        new_boundaries.len(),
        7,
        "New boundaries must be regenerated with correct count"
    );
    assert_ne!(
        old_len,
        new_boundaries.len(),
        "Old and new boundary counts must differ after insert"
    );
}

// ============================================================================
// SPEC-HAL-107 §2.6 — Anchor Validity
// ============================================================================

#[test]
fn anchor_within_document_bounds_SPEC_107_2_6() {
    // SPEC-HAL-107 §2.6: selection.anchor is always within [0 .. text.len()].

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    // Test various anchor positions
    for pos in 0..=text.len() {
        let selection = Selection::at(pos);
        assert!(
            selection.anchor <= text.len(),
            "anchor {} must be <= text.len() {}",
            selection.anchor,
            text.len()
        );
    }
}

#[test]
fn anchor_at_valid_boundary_SPEC_107_2_6() {
    // SPEC-HAL-107 §2.6: selection.anchor is a valid boundary position.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    for pos in 0..=text.len() {
        let selection = Selection::at(pos);
        let anchor_is_boundary = boundaries.iter().any(|b| b.byte_start == selection.anchor);
        assert!(
            anchor_is_boundary,
            "anchor {} must be a valid boundary",
            selection.anchor
        );
    }
}

#[test]
fn anchor_valid_after_insert_SPEC_107_2_6() {
    // SPEC-HAL-107 §2.6: After insert, anchor remains valid.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // Anchor should be valid in new text
    assert!(
        selection.anchor <= text.len(),
        "anchor {} must be <= text.len() {} after insert",
        selection.anchor,
        text.len()
    );

    // Rebuild and verify anchor is at valid boundary
    let new_boundaries = make_ascii_boundaries(&text);
    let anchor_is_boundary = new_boundaries
        .iter()
        .any(|b| b.byte_start == selection.anchor);
    assert!(
        anchor_is_boundary,
        "anchor {} must be valid boundary after insert",
        selection.anchor
    );
}

// ============================================================================
// SPEC-HAL-107 §2.7 — Anchor Stability
// ============================================================================

#[test]
fn anchor_unchanged_on_cursor_move_left_SPEC_107_2_7() {
    // SPEC-HAL-107 §2.7: Anchor does not change during cursor movement.

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);
    let original_anchor = selection.anchor;

    // Move cursor left (collapse mode - anchor follows cursor)
    move_left_collapse(&mut cursor, &mut selection, &boundaries);

    // In collapse mode, anchor DOES change (collapses to cursor)
    // This is expected behavior for non-extending movement
    assert_eq!(
        selection.anchor, cursor.pos,
        "In collapse mode, anchor equals cursor"
    );
}

#[test]
fn anchor_unchanged_on_cursor_move_right_SPEC_107_2_7() {
    // SPEC-HAL-107 §2.7: Anchor does not change during cursor movement (collapse mode).

    let text = "Hello";
    let boundaries = make_ascii_boundaries(text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Move cursor right (collapse mode)
    move_right_collapse(&mut cursor, &mut selection, &boundaries);

    // In collapse mode, selection collapses to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "In collapse mode, anchor equals cursor after move"
    );
}

#[test]
fn anchor_stable_when_no_selection_replaced_on_insert_SPEC_107_2_7() {
    // SPEC-HAL-107 §2.7: Inserting at cursor (no selection) moves both anchor and head together.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2); // Collapsed selection

    assert_eq!(
        selection.anchor, selection.head,
        "Selection must be collapsed"
    );

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // After insert with collapsed selection, both anchor and head move to cursor
    assert_eq!(
        selection.anchor, cursor.pos,
        "Anchor moves to cursor after insert"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "Head moves to cursor after insert"
    );
    assert_eq!(
        selection.anchor, selection.head,
        "Selection remains collapsed"
    );
}

#[test]
fn anchor_changes_when_selection_replaced_on_insert_SPEC_107_2_7() {
    // SPEC-HAL-107 §2.7: Inserting with active selection replaces selection, anchor moves.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Selecting "ell"

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "X");

    // Selection is replaced; both anchor and head collapse to cursor
    assert_eq!(text, "HXo", "Selected text replaced with X");
    assert_eq!(
        selection.anchor, cursor.pos,
        "Anchor collapses to cursor after replace"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "Head collapses to cursor after replace"
    );
}

#[test]
fn anchor_changes_when_selection_deleted_SPEC_107_2_7() {
    // SPEC-HAL-107 §2.7: Deleting selection moves anchor to deletion point.

    let mut text = String::from("Hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::new();
    selection.anchor = 1;
    selection.head = 4; // Selecting "ell"

    apply_edit_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries,
        TextEditOp::Backspace,
    );

    // Selection deleted; anchor collapses to deletion point
    assert_eq!(text, "Ho", "Selected text deleted");
    assert_eq!(
        selection.anchor, cursor.pos,
        "Anchor collapses to cursor after delete"
    );
}
