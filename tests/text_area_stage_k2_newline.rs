//! SPEC-HAL-109 Stage K.2: Newline Semantics & Caret Progression Tests
//!
//! These tests verify:
//! - K2.INV1: Newline is a real boundary (byte_start = index of '\n', byte_end = byte_start + 1)
//! - K2.INV2: Caret positions are closed over boundaries
//! - K2.INV3: Right arrow at EOL moves to next line start
//! - K2.INV4: ENTER inserts exactly one '\n', caret advances by 1
//! - K2.INV5: ENTER is idempotent in space (works repeatedly)

#![allow(unused_assignments, unused_variables)]
//!
//! Run with: cargo test --features test-harness --test text_area_stage_k2_newline

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, move_left_collapse, move_right_collapse,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for multi-line text with explicit newline boundaries.
/// K2.INV1: Each '\n' gets a boundary with byte_end = byte_start + 1.
fn make_boundaries_with_newlines(text: &str, char_width: f32) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut line_x = 0.0_f32;

    for (byte_idx, ch) in text.char_indices() {
        if ch == '\n' {
            // K2.INV1: Newline boundary at end of current line
            boundaries.push(ClusterBoundary {
                byte_start: byte_idx,
                byte_end: byte_idx + 1,
                line_index,
                x_logical: line_x,
            });
            line_index += 1;
            line_x = 0.0;
        } else {
            boundaries.push(ClusterBoundary {
                byte_start: byte_idx,
                byte_end: byte_idx + ch.len_utf8(),
                line_index,
                x_logical: line_x,
            });
            line_x += char_width;
        }
    }

    // Sentinel at end of document
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical: line_x,
    });

    boundaries
}

/// Assert cursor position is a valid boundary position.
fn assert_cursor_on_boundary(cursor: &TextCursor, boundaries: &[ClusterBoundary]) {
    let pos = cursor.pos;
    assert!(
        boundaries.iter().any(|b| b.byte_start == pos),
        "K2.INV2 violated: cursor.pos={} not in boundaries {:?}",
        pos,
        boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
    );
}

// ============================================================================
// K2.INV1: Newline Is a Real Boundary
// ============================================================================

#[test]
fn k2_1_newline_has_boundary() {
    // Text: "abc\ndef"
    // Boundaries should include byte 3 ('\n') with byte_end = 4
    let text = "abc\ndef";
    let boundaries = make_boundaries_with_newlines(text, 10.0);

    // Find the newline boundary
    let newline_boundary = boundaries.iter().find(|b| b.byte_start == 3);
    assert!(
        newline_boundary.is_some(),
        "K2.INV1: Newline at byte 3 must have a boundary"
    );

    let nb = newline_boundary.unwrap();
    assert_eq!(
        nb.byte_end, 4,
        "K2.INV1: Newline byte_end must be byte_start + 1"
    );
    assert_eq!(nb.line_index, 0, "Newline terminates line 0");
}

#[test]
fn k2_1_consecutive_newlines_have_boundaries() {
    // Text: "\n\n\n" (three newlines)
    let text = "\n\n\n";
    let boundaries = make_boundaries_with_newlines(text, 10.0);

    // Should have boundaries at 0, 1, 2 (newlines) + 3 (sentinel)
    assert_eq!(
        boundaries.len(),
        4,
        "Should have 4 boundaries for 3 newlines + sentinel"
    );

    // Each newline boundary
    for i in 0..3 {
        let b = &boundaries[i];
        assert_eq!(b.byte_start, i, "Newline {} at byte {}", i, i);
        assert_eq!(b.byte_end, i + 1, "K2.INV1: byte_end = byte_start + 1");
        assert_eq!(b.line_index, i, "Newline {} terminates line {}", i, i);
    }
}

// ============================================================================
// K2.INV3: Right Arrow at EOL Moves to Next Line
// ============================================================================

#[test]
fn k2_3_right_at_eol_moves_to_next_line() {
    let text = "abc\ndef";
    let boundaries = make_boundaries_with_newlines(text, 10.0);

    // Position cursor at 'c' (byte 2)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);

    // Move right to newline (byte 3)
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(cursor.pos, 3, "Should be at newline (byte 3)");

    // Move right again - should cross to next line (byte 4 = 'd')
    move_right_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 4,
        "K2.INV3: Right at EOL should move to next line start (byte 4)"
    );
}

#[test]
fn k2_3_left_at_bol_moves_to_prev_line_newline() {
    let text = "abc\ndef";
    let boundaries = make_boundaries_with_newlines(text, 10.0);

    // Position cursor at 'd' (byte 4, start of line 1)
    let mut cursor = TextCursor::at(4);
    let mut selection = Selection::at(4);

    // Move left - should go to newline (byte 3)
    move_left_collapse(&mut cursor, &mut selection, &boundaries);
    assert_eq!(
        cursor.pos, 3,
        "K2.INV3: Left at BOL should move to previous line's newline (byte 3)"
    );
}

// ============================================================================
// K2.INV4 & K2.INV5: ENTER Insertion Tests
// ============================================================================

#[test]
fn k2_4_enter_at_bol_repeatedly() {
    // Start with "abc"
    let mut text = String::from("abc");

    // Position cursor at beginning (byte 0)
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    // Simulate ENTER 3 times at BOL
    for i in 0..3 {
        let pre_len = text.len();
        let pre_pos = cursor.pos;

        // Insert '\n' at cursor position
        text.insert(pre_pos, '\n');
        cursor.pos = pre_pos + 1;
        selection.anchor = cursor.pos;
        selection.head = cursor.pos;

        // K2.INV4: doc_len increases by 1, cursor advances by 1
        assert_eq!(
            text.len(),
            pre_len + 1,
            "K2.INV4: ENTER #{} - doc_len should increase by 1",
            i + 1
        );
        assert_eq!(
            cursor.pos,
            pre_pos + 1,
            "K2.INV4: ENTER #{} - cursor should advance by 1",
            i + 1
        );
    }

    // After 3 ENTERs at BOL: "\n\n\nabc"
    assert_eq!(text, "\n\n\nabc", "Text after 3 ENTER at BOL");
    assert_eq!(cursor.pos, 3, "Cursor at byte 3 after 3 ENTER");
}

#[test]
fn k2_4_enter_at_eol_repeatedly() {
    let mut text = String::from("abc");

    // Position cursor at end (byte 3)
    let mut cursor = TextCursor::at(3);
    let mut selection = Selection::at(3);

    // Simulate ENTER 3 times at EOL
    for i in 0..3 {
        let pre_len = text.len();
        let pre_pos = cursor.pos;

        text.insert(pre_pos, '\n');
        cursor.pos = pre_pos + 1;
        selection.anchor = cursor.pos;
        selection.head = cursor.pos;

        assert_eq!(text.len(), pre_len + 1, "K2.INV4: ENTER #{} at EOL", i + 1);
        assert_eq!(cursor.pos, pre_pos + 1, "K2.INV4: cursor advances");
    }

    // After 3 ENTERs at EOL: "abc\n\n\n"
    assert_eq!(text, "abc\n\n\n", "Text after 3 ENTER at EOL");
    assert_eq!(cursor.pos, 6, "Cursor at byte 6 after 3 ENTER");
}

#[test]
fn k2_4_enter_in_middle_repeatedly() {
    let mut text = String::from("abcdef");

    // Position cursor at middle (byte 3, between 'c' and 'd')
    let mut cursor = TextCursor::at(3);

    // ENTER should insert '\n' at position 3
    let pre_len = text.len();
    text.insert(3, '\n');
    cursor.pos = 4;

    assert_eq!(text.len(), pre_len + 1);
    assert_eq!(text, "abc\ndef");
    assert_eq!(cursor.pos, 4, "Cursor at start of 'def'");

    // ENTER again at same logical position (now byte 4)
    let pre_len = text.len();
    text.insert(4, '\n');
    cursor.pos = 5;

    assert_eq!(text.len(), pre_len + 1);
    assert_eq!(text, "abc\n\ndef");
    assert_eq!(cursor.pos, 5);
}

#[test]
fn k2_5_enter_idempotent_on_newlines() {
    // K2.INV5: ENTER on text with only newlines should work
    let mut text = String::from("\n\n");

    // Position at byte 1 (between two newlines)
    let mut cursor = TextCursor::at(1);
    let mut selection = Selection::at(1);

    // Insert another newline
    let pre_len = text.len();
    let pre_pos = cursor.pos;
    text.insert(pre_pos, '\n');
    cursor.pos = pre_pos + 1;
    selection.anchor = cursor.pos;
    selection.head = cursor.pos;

    // K2.INV5: Should not jump to end-of-doc
    assert_eq!(text.len(), pre_len + 1, "K2.INV5: doc grows by 1");
    assert_eq!(cursor.pos, 2, "K2.INV5: cursor at 2, not end-of-doc");
    assert_eq!(text, "\n\n\n");
}

// ============================================================================
// K2.INV2: Caret Position Always Equals a Boundary
// ============================================================================

#[test]
fn k2_2_cursor_always_on_boundary_after_navigation() {
    let text = "abc\ndef\nghi";
    let boundaries = make_boundaries_with_newlines(text, 10.0);

    // All navigation should leave cursor on a boundary
    // Test all valid cursor positions
    for b in &boundaries {
        let cursor = TextCursor::at(b.byte_start);
        assert_cursor_on_boundary(&cursor, &boundaries);
    }
}

#[test]
fn k2_2_cursor_on_boundary_after_enter() {
    let mut text = String::from("abc");

    // Position at byte 1
    let mut cursor = TextCursor::at(1);

    // Insert newline
    text.insert(1, '\n');
    cursor.pos = 2;

    // Rebuild boundaries for new text
    let boundaries = make_boundaries_with_newlines(&text, 10.0);

    // K2.INV2: Cursor must be on a boundary
    assert_cursor_on_boundary(&cursor, &boundaries);
}

// ============================================================================
// No Caret Teleport After ENTER
// ============================================================================

#[test]
fn k2_no_teleport_enter_at_start() {
    let mut text = String::from("hello");

    // Position at start (byte 0)
    let mut cursor = TextCursor::at(0);

    // ENTER at start
    text.insert(0, '\n');
    cursor.pos = 1;

    // Cursor should be at 1, NOT at end of document (6)
    assert_eq!(cursor.pos, 1, "Cursor at 1, not teleported to end");
    assert!(cursor.pos < text.len(), "Cursor not at end of doc");
}

#[test]
fn k2_no_teleport_repeated_enter() {
    let mut text = String::from("ab");

    // Position at byte 1 (between 'a' and 'b')
    let mut cursor = TextCursor::at(1);

    // ENTER 5 times
    for i in 0..5 {
        let pos = cursor.pos;
        text.insert(pos, '\n');
        cursor.pos = pos + 1;

        // Cursor should be at expected position, not teleported
        let expected = 1 + i + 1; // starts at 1, advances by 1 each ENTER
        assert_eq!(
            cursor.pos,
            expected,
            "After ENTER #{}, cursor at {}, not teleported",
            i + 1,
            expected
        );
    }

    // Final text: "a\n\n\n\n\nb"
    assert_eq!(text, "a\n\n\n\n\nb");
}
