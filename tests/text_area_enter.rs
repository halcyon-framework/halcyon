// TextArea Enter Key Regression Tests
//
// These tests verify that Enter (newline insertion) works correctly.
// Since TextArea::handle_event requires a full EventContext with WidgetTree,
// we test the exact same code path that Enter uses: apply_insert_op with "\n".
//
// This is the actual function TextArea calls on Enter:
//   apply_insert_op(document.content_mut(), &mut selection, &mut cursor, boundaries, "\n")

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, apply_insert_op};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for multi-line ASCII text.
/// Each character gets its own cluster. Newlines increment line_index.
fn make_multiline_ascii_boundaries(text: &str) -> Vec<ClusterBoundary> {
    let mut boundaries = Vec::new();
    let mut line_index = 0;
    let mut x_logical = 0.0;
    let char_width = 10.0;

    for (byte_idx, ch) in text.char_indices() {
        boundaries.push(ClusterBoundary {
            byte_start: byte_idx,
            byte_end: byte_idx + ch.len_utf8(),
            line_index,
            x_logical,
        });

        if ch == '\n' {
            line_index += 1;
            x_logical = 0.0;
        } else {
            x_logical += char_width;
        }
    }

    // Sentinel at end
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index,
        x_logical,
    });

    boundaries
}

/// Assert that a position is a valid boundary byte_start.
fn assert_valid_boundary(boundaries: &[ClusterBoundary], pos: usize, context: &str) {
    let is_valid = boundaries.iter().any(|b| b.byte_start == pos);
    assert!(
        is_valid,
        "{}: position {} is not a valid boundary. Valid positions: {:?}",
        context,
        pos,
        boundaries.iter().map(|b| b.byte_start).collect::<Vec<_>>()
    );
}

// ============================================================================
// Regression Test: Enter in middle of first line
// ============================================================================

#[test]
fn test_enter_middle_of_first_line() {
    // Initial text: "Welcome to TextArea!\nSecond line\nThird line"
    let mut text = String::from("Welcome to TextArea!\nSecond line\nThird line");
    let boundaries = make_multiline_ascii_boundaries(&text);

    // Put cursor after "Welcome" (byte pos = 7)
    let cursor_pos = 7;
    assert_valid_boundary(&boundaries, cursor_pos, "Initial cursor position");

    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    // Simulate Enter key: insert "\n"
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    // Assert document text becomes: "Welcome\n to TextArea!\nSecond line\nThird line"
    let expected = "Welcome\n to TextArea!\nSecond line\nThird line";
    assert_eq!(
        text, expected,
        "Text after Enter should have newline inserted at position 7"
    );

    // Assert cursor.pos == 8 (after the inserted newline)
    assert_eq!(
        cursor.pos, 8,
        "Cursor should advance by 1 after inserting newline"
    );

    // Assert selection collapsed at cursor.pos
    assert_eq!(
        selection.anchor, cursor.pos,
        "Selection anchor should equal cursor.pos"
    );
    assert_eq!(
        selection.head, cursor.pos,
        "Selection head should equal cursor.pos"
    );

    // Note: After text mutation, boundaries are STALE.
    // TextArea rebuilds boundaries on next layout pass.
    // We verify cursor.pos will be valid in NEW boundaries.
    let new_boundaries = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(
        &new_boundaries,
        cursor.pos,
        "Cursor position after Enter in new boundaries",
    );
}

// ============================================================================
// Regression Test: Enter on a non-first line
// ============================================================================

#[test]
fn test_enter_on_second_line() {
    // Initial text: "Welcome to TextArea!\nSecond line\nThird line"
    let mut text = String::from("Welcome to TextArea!\nSecond line\nThird line");
    let boundaries = make_multiline_ascii_boundaries(&text);

    // "Welcome to TextArea!\n" = 21 bytes
    // "Second" = 6 bytes
    // So cursor after "Second" is at byte 21 + 6 = 27
    let cursor_pos = 27;
    assert_valid_boundary(&boundaries, cursor_pos, "Initial cursor on line 2");

    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    // Simulate Enter key
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    // Expected: "Welcome to TextArea!\nSecond\n line\nThird line"
    let expected = "Welcome to TextArea!\nSecond\n line\nThird line";
    assert_eq!(text, expected, "Newline inserted after 'Second' on line 2");

    assert_eq!(cursor.pos, 28, "Cursor should be at 28 after Enter");

    // Verify cursor valid in new boundaries
    let new_boundaries = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(&new_boundaries, cursor.pos, "Cursor after Enter on line 2");
}

// ============================================================================
// Regression Test: Enter twice at different positions
// ============================================================================

#[test]
fn test_enter_twice_at_different_positions() {
    // Initial text: "ABCDEF"
    let mut text = String::from("ABCDEF");
    let boundaries = make_multiline_ascii_boundaries(&text);

    // First Enter: after "AB" (pos 2)
    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);
    assert_valid_boundary(&boundaries, cursor.pos, "First Enter position");

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    // After first Enter: "AB\nCDEF", cursor at 3
    assert_eq!(text, "AB\nCDEF", "After first Enter");
    assert_eq!(cursor.pos, 3);

    // Rebuild boundaries for new text
    let boundaries2 = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(&boundaries2, cursor.pos, "Cursor after first Enter");

    // Second Enter: after "CD" on line 2
    // "AB\n" = 3 bytes, "CD" = 2 bytes, so pos = 5
    cursor.pos = 5;
    selection.anchor = 5;
    selection.head = 5;
    assert_valid_boundary(&boundaries2, cursor.pos, "Second Enter position");

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries2, "\n");

    // After second Enter: "AB\nCD\nEF", cursor at 6
    assert_eq!(text, "AB\nCD\nEF", "After second Enter");
    assert_eq!(cursor.pos, 6);

    // Verify final state
    let boundaries3 = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(&boundaries3, cursor.pos, "Cursor after second Enter");
}

// ============================================================================
// Invariant Test: cursor.pos must be valid boundary before Enter
// ============================================================================

#[test]
fn test_cursor_must_be_valid_boundary_before_enter() {
    let text = "Hello";
    let boundaries = make_multiline_ascii_boundaries(text);

    // Valid positions for "Hello" are: 0, 1, 2, 3, 4, 5 (sentinel)
    // Test each valid position
    for pos in 0..=5 {
        assert_valid_boundary(&boundaries, pos, &format!("Position {}", pos));
    }

    // Position 6 would be INVALID (past sentinel)
    let is_invalid = !boundaries.iter().any(|b| b.byte_start == 6);
    assert!(is_invalid, "Position 6 should be invalid for 'Hello'");
}

// ============================================================================
// Edge Case: Enter at document start
// ============================================================================

#[test]
fn test_enter_at_document_start() {
    let mut text = String::from("Hello");
    let boundaries = make_multiline_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    assert_eq!(text, "\nHello", "Enter at start should prepend newline");
    assert_eq!(cursor.pos, 1, "Cursor should be at 1");

    let new_boundaries = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(&new_boundaries, cursor.pos, "Cursor after Enter at start");
}

// ============================================================================
// Edge Case: Enter at document end
// ============================================================================

#[test]
fn test_enter_at_document_end() {
    let mut text = String::from("Hello");
    let boundaries = make_multiline_ascii_boundaries(&text);

    // Cursor at sentinel (position 5)
    let mut cursor = TextCursor::at(5);
    let mut selection = Selection::at(5);

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    assert_eq!(text, "Hello\n", "Enter at end should append newline");
    assert_eq!(cursor.pos, 6, "Cursor should be at 6");

    let new_boundaries = make_multiline_ascii_boundaries(&text);
    assert_valid_boundary(&new_boundaries, cursor.pos, "Cursor after Enter at end");
}

// ============================================================================
// Edge Case: Enter with selection (should replace)
// ============================================================================

#[test]
fn test_enter_replaces_selection() {
    let mut text = String::from("Hello World");
    let boundaries = make_multiline_ascii_boundaries(&text);

    // Select "o Wo" (positions 4-8)
    let mut cursor = TextCursor::at(8);
    let mut selection = Selection::new();
    selection.anchor = 4;
    selection.head = 8;

    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    // "Hell" + "\n" + "rld" = "Hell\nrld"
    assert_eq!(
        text, "Hell\nrld",
        "Enter should replace selection with newline"
    );
    assert_eq!(cursor.pos, 5, "Cursor should be after inserted newline");
    assert_eq!(selection.anchor, 5, "Selection should collapse to cursor");
    assert_eq!(selection.head, 5, "Selection should collapse to cursor");
}

// ============================================================================
// Stress Test: Many sequential Enter presses
// ============================================================================

#[test]
fn test_many_sequential_enters() {
    let mut text = String::from("A");
    let mut boundaries = make_multiline_ascii_boundaries(&text);

    let mut cursor = TextCursor::at(1); // After 'A'
    let mut selection = Selection::at(1);

    // Press Enter 5 times
    for i in 0..5 {
        assert_valid_boundary(&boundaries, cursor.pos, &format!("Before Enter #{}", i + 1));

        apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

        // Rebuild boundaries after each insert
        boundaries = make_multiline_ascii_boundaries(&text);

        assert_valid_boundary(&boundaries, cursor.pos, &format!("After Enter #{}", i + 1));
    }

    assert_eq!(text, "A\n\n\n\n\n", "Should have 5 newlines after 'A'");
    assert_eq!(cursor.pos, 6, "Cursor should be at position 6");
}
