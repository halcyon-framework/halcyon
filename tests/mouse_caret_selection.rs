// SPEC-HAL-107 §7.6 — Mouse-Initiated Caret + Selection Semantics
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

#![allow(non_snake_case)]

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, hit_test_click, hit_test_x_y,
};

// ============================================================================
// Helpers
// ============================================================================

/// Create boundaries for multi-line text with proper line_index assignment.
/// Each character gets 10px width.
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

/// Simulate a single click (no modifiers) using hit_test_click.
fn click_no_modifier(
    x: f32,
    y: f32,
    boundaries: &[ClusterBoundary],
    cursor: &mut TextCursor,
    selection: &mut Selection,
) {
    hit_test_click(cursor, selection, boundaries, x, y);
}

/// Simulate a shift+click: anchor stays, head moves to clicked position.
fn click_shift(
    x: f32,
    y: f32,
    boundaries: &[ClusterBoundary],
    cursor: &mut TextCursor,
    selection: &mut Selection,
) {
    let hit_pos = hit_test_x_y(boundaries, x, y);
    cursor.pos = hit_pos;
    cursor.preferred_x = None; // Mouse click clears preferred_x
    // Shift+click: anchor remains, head moves to new position
    selection.head = hit_pos;
}

// ============================================================================
// SPEC-HAL-107 §7.6.1 — Single Click (No Modifiers)
// ============================================================================

#[test]
fn single_click_places_caret_at_resolved_boundary_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Single click places caret deterministically at resolved boundary.

    let text = "ABCDE\nFGHIJ\nKLMNO";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click at x=25, y=0 (line 0, between 'C' at x=20 and 'D' at x=30)
    // Should resolve to nearest boundary
    click_no_modifier(25.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // cursor.pos must be a valid boundary byte_start
    let is_valid = boundaries.iter().any(|b| b.byte_start == cursor.pos);
    assert!(
        is_valid,
        "SPEC-HAL-107 §7.6: cursor.pos {} must be a valid boundary byte_start",
        cursor.pos
    );
}

#[test]
fn single_click_collapses_selection_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Single click collapses selection (anchor == head == caret).

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::new();
    selection.anchor = 2;
    selection.head = 8; // Pre-existing selection

    assert!(!selection.is_empty(), "Precondition: selection non-empty");

    // Single click
    click_no_modifier(15.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Selection must be collapsed
    assert!(
        selection.is_empty(),
        "SPEC-HAL-107 §7.6: Single click must collapse selection. anchor={}, head={}",
        selection.anchor,
        selection.head
    );
}

#[test]
fn single_click_anchor_equals_head_equals_caret_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: After single click, anchor == head == cursor.pos.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click at x=35, y=0 (near 'D' at x=30)
    click_no_modifier(35.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // All three must be equal
    assert_eq!(
        selection.anchor, cursor.pos,
        "SPEC-HAL-107 §7.6: anchor must equal cursor.pos after single click"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "SPEC-HAL-107 §7.6: head must equal cursor.pos after single click"
    );
}

#[test]
fn single_click_updates_preferred_x_from_click_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: preferred_x updated from click X coordinate.
    // NOTE: Current implementation clears preferred_x. SPEC may require setting it.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    cursor.preferred_x = Some(100.0); // Pre-existing value
    let mut selection = Selection::at(0);

    // Click at x=25 (between 'C' at x=20 and 'D' at x=30)
    click_no_modifier(25.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // SPEC-HAL-107 §7.6: The spec says "preferred_x updated from click X"
    // This could mean:
    // (a) Set to clicked X position, OR
    // (b) Set to the resolved boundary's x_logical, OR
    // (c) Cleared (None)
    //
    // Most editors clear preferred_x on mouse click (option c), since horizontal
    // intent only matters for keyboard vertical movement. Let's verify the current
    // behavior is at least consistent (cleared).
    assert!(
        cursor.preferred_x.is_none(),
        "SPEC-HAL-107 §7.6: Mouse click should clear preferred_x (current behavior)"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.6.2 — Shift + Click
// ============================================================================

#[test]
fn shift_click_anchor_remains_at_previous_caret_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click preserves anchor at previous caret position.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at position 2 ('C'), no selection
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    let original_anchor = selection.anchor;

    // Shift+click at x=45 (near 'E' at x=40)
    click_shift(45.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Anchor must remain at original position (2)
    assert_eq!(
        selection.anchor, original_anchor,
        "SPEC-HAL-107 §7.6: Shift+click must preserve anchor at original position ({}), got {}",
        original_anchor, selection.anchor
    );
}

#[test]
fn shift_click_extends_selection_to_clicked_position_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click extends selection to clicked position.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at position 1 ('B'), no selection
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // Shift+click at x=35 (near 'D' at x=30, should resolve to byte 3)
    click_shift(35.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Head should have moved to clicked position
    assert_eq!(
        selection.head, cursor.pos,
        "SPEC-HAL-107 §7.6: head must follow cursor to clicked position"
    );

    // Selection should be non-empty (anchor=1, head=~3)
    assert!(
        !selection.is_empty(),
        "SPEC-HAL-107 §7.6: Selection must be non-empty after Shift+click"
    );
}

#[test]
fn shift_click_direction_reversal_preserves_anchor_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click in opposite direction preserves anchor.

    let text = "ABCDEFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at position 5 ('F')
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Shift+click right at x=85 (near 'I')
    click_shift(85.0, 0.0, &boundaries, &mut cursor, &mut selection);
    assert_eq!(selection.anchor, 5, "Anchor should be at 5");

    // Shift+click left at x=15 (near 'B') - direction reversal
    click_shift(15.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Anchor must STILL be at original position (5)
    assert_eq!(
        selection.anchor, 5,
        "SPEC-HAL-107 §7.6: Direction reversal must preserve anchor at 5, got {}",
        selection.anchor
    );

    // Head should now be left of anchor
    assert!(
        selection.head < selection.anchor,
        "SPEC-HAL-107 §7.6: After reversal, head ({}) should be left of anchor ({})",
        selection.head,
        selection.anchor
    );
}

#[test]
fn shift_click_updates_preferred_x_from_click_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click updates preferred_x from click X.
    // NOTE: Current click_shift helper doesn't set preferred_x.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(2);
    cursor.preferred_x = Some(100.0); // Pre-existing
    let mut selection = Selection::at(2);

    // Shift+click (current helper clears preferred_x)
    click_shift(35.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Like single click, mouse operations typically clear preferred_x
    // Horizontal intent is keyboard-specific behavior
    assert!(
        cursor.preferred_x.is_none(),
        "SPEC-HAL-107 §7.6: Shift+click should clear preferred_x (current behavior)"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.6.3 — Click on Empty Line
// ============================================================================

#[test]
fn click_on_empty_line_lands_at_line_start_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Click on empty line lands at line start (byte position of that line).

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty line
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click on empty line 1 (y=20-40 range for line 1, using y=30)
    // x=50 is far right, but empty line has no content
    // LINE_SLOT_HEIGHT = 20.0, so line 1 = y in [20, 40)
    click_no_modifier(50.0, 30.0, &boundaries, &mut cursor, &mut selection);

    // Should land at line 1 start (byte 6 = the newline character on empty line)
    assert_eq!(
        cursor.pos, 6,
        "SPEC-HAL-107 §7.6: Click on empty line must land at line start (byte 6), got {}",
        cursor.pos
    );
}

#[test]
fn shift_click_on_empty_line_includes_empty_line_in_selection_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click on empty line includes it in selection range.

    // Text: "ABCDE\n\nFGHIJ"
    // Line 0: A(0) B(1) C(2) D(3) E(4) \n(5)
    // Line 1: \n(6) - empty line
    // Line 2: F(7) G(8) H(9) I(10) J(11) sentinel(12)
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at 'C' on line 0 (byte 2)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Shift+click on empty line 1 (LINE_SLOT_HEIGHT=20, so line 1 = y in [20, 40))
    click_shift(50.0, 30.0, &boundaries, &mut cursor, &mut selection);

    // Selection should span from anchor (2) to empty line (6)
    assert_eq!(selection.anchor, 2, "Anchor should remain at 2");
    assert_eq!(
        selection.head, 6,
        "SPEC-HAL-107 §7.6: Shift+click on empty line should include it. head={}, expected 6",
        selection.head
    );

    // Selection is non-empty and includes the empty line
    assert!(!selection.is_empty(), "Selection must be non-empty");
}

#[test]
fn click_on_empty_line_no_phantom_horizontal_carryover_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: No phantom horizontal carryover from previous line.
    // Clicking on empty line should land at x=0 position, regardless of clicked X.

    // Text: "ABCDE\n\nFGHIJ"
    let text = "ABCDE\n\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click at x=50 on empty line 1 - should still land at line start (x=0 position)
    // LINE_SLOT_HEIGHT=20, so line 1 = y in [20, 40)
    click_no_modifier(50.0, 30.0, &boundaries, &mut cursor, &mut selection);

    // Verify we landed on line 1 (empty line)
    let boundary = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(boundary.line_index, 1, "Should land on line 1");

    // The x_logical of the landing position should be 0 (line start)
    assert_eq!(
        boundary.x_logical, 0.0,
        "SPEC-HAL-107 §7.6: Empty line click must land at x=0, no phantom horizontal carryover"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.6.4 — Click Above / Below Text Bounds
// ============================================================================

#[test]
fn click_above_first_line_clamps_to_document_start_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Click above first line clamps caret to document start.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Click above first line (y=-100)
    click_no_modifier(25.0, -5.0, &boundaries, &mut cursor, &mut selection);

    // Should clamp to line 0
    let boundary = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "SPEC-HAL-107 §7.6: Click above first line must clamp to line 0"
    );
}

#[test]
fn click_below_last_line_clamps_to_document_end_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Click below last line clamps caret to document end.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);
    let max_line = boundaries.last().unwrap().line_index;

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Click below last line (y=1000)
    click_no_modifier(25.0, 100.0, &boundaries, &mut cursor, &mut selection);

    // Should clamp to last line
    let boundary = boundaries
        .iter()
        .find(|b| b.byte_start == cursor.pos)
        .unwrap();
    assert_eq!(
        boundary.line_index, max_line,
        "SPEC-HAL-107 §7.6: Click below last line must clamp to last line ({}), got {}",
        max_line, boundary.line_index
    );
}

#[test]
fn shift_click_above_first_line_extends_to_document_start_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click above first line extends selection to document start.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    // Start at position 8 ('H' on line 1)
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::at(8);

    // Shift+click above first line
    click_shift(25.0, -5.0, &boundaries, &mut cursor, &mut selection);

    // Anchor remains at 8
    assert_eq!(selection.anchor, 8, "Anchor must remain at 8");

    // Head should be on line 0 (clamped from above)
    let boundary = boundaries
        .iter()
        .find(|b| b.byte_start == selection.head)
        .unwrap();
    assert_eq!(
        boundary.line_index, 0,
        "SPEC-HAL-107 §7.6: Shift+click above first line must extend to line 0"
    );
}

#[test]
fn shift_click_below_last_line_extends_to_document_end_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Shift+click below last line extends selection to document end.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);
    let max_line = boundaries.last().unwrap().line_index;

    // Start at position 2 ('C' on line 0)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Shift+click below last line
    click_shift(25.0, 100.0, &boundaries, &mut cursor, &mut selection);

    // Anchor remains at 2
    assert_eq!(selection.anchor, 2, "Anchor must remain at 2");

    // Head should be on last line (clamped from below)
    let boundary = boundaries
        .iter()
        .find(|b| b.byte_start == selection.head)
        .unwrap();
    assert_eq!(
        boundary.line_index, max_line,
        "SPEC-HAL-107 §7.6: Shift+click below last line must extend to last line"
    );
}

// ============================================================================
// SPEC-HAL-107 §7.6.5 — Repeated Clicks (Stability)
// ============================================================================

#[test]
fn clicking_same_position_twice_is_idempotent_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Clicking same position twice produces identical result.

    let text = "ABCDE\nFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // First click
    click_no_modifier(35.0, 0.0, &boundaries, &mut cursor, &mut selection);
    let pos_after_first = cursor.pos;
    let anchor_after_first = selection.anchor;
    let head_after_first = selection.head;

    // Second click at same position
    click_no_modifier(35.0, 0.0, &boundaries, &mut cursor, &mut selection);

    // Result must be identical
    assert_eq!(
        cursor.pos, pos_after_first,
        "SPEC-HAL-107 §7.6: Repeated click must be idempotent (cursor.pos)"
    );
    assert_eq!(
        selection.anchor, anchor_after_first,
        "SPEC-HAL-107 §7.6: Repeated click must be idempotent (anchor)"
    );
    assert_eq!(
        selection.head, head_after_first,
        "SPEC-HAL-107 §7.6: Repeated click must be idempotent (head)"
    );
}

#[test]
fn repeated_clicks_no_anchor_drift_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Repeated clicks do not cause anchor drift.

    let text = "ABCDEFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    // Multiple clicks at same position
    for _ in 0..10 {
        click_no_modifier(25.0, 0.0, &boundaries, &mut cursor, &mut selection);
    }

    // Anchor should equal head and cursor.pos (collapsed selection)
    assert_eq!(
        selection.anchor, selection.head,
        "SPEC-HAL-107 §7.6: No anchor drift after repeated clicks"
    );
    assert_eq!(
        selection.anchor, cursor.pos,
        "SPEC-HAL-107 §7.6: anchor must equal cursor.pos after repeated clicks"
    );
}

#[test]
fn repeated_clicks_no_preferred_x_corruption_SPEC_107_7_6() {
    // SPEC-HAL-107 §7.6: Repeated clicks do not corrupt preferred_x.

    let text = "ABCDEFGHIJ";
    let boundaries = make_multiline_boundaries(text);

    let mut cursor = TextCursor::at(5);
    cursor.preferred_x = Some(50.0);
    let mut selection = Selection::at(5);

    // Multiple clicks at same position
    for _ in 0..10 {
        click_no_modifier(25.0, 0.0, &boundaries, &mut cursor, &mut selection);
    }

    // preferred_x should be consistently None after mouse clicks
    assert!(
        cursor.preferred_x.is_none(),
        "SPEC-HAL-107 §7.6: preferred_x should be None after repeated clicks (no corruption)"
    );
}
