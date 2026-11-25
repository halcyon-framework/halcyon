// SPEC-HAL-052 Stage 9: Word-Wise Deletion Tests

use halcyon::core::editing::{
    ClusterBoundary, Selection, TextCursor, apply_delete_word_back, apply_delete_word_forward,
    apply_insert_op,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create boundaries for simple ASCII text where each char is one cluster.
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
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: text.len() as f32 * 10.0,
    });
    boundaries
}

/// Create boundaries for UTF-8 text, respecting char boundaries.
fn make_utf8_boundaries(text: &str) -> Vec<ClusterBoundary> {
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
            line_index: 0,
            x_logical: i as f32 * 10.0,
        });
    }
    // Sentinel
    boundaries.push(ClusterBoundary {
        byte_start: text.len(),
        byte_end: text.len(),
        line_index: 0,
        x_logical: boundaries.len() as f32 * 10.0,
    });
    boundaries
}

// ============================================================================
// 5.1 Selection Takes Precedence
// ============================================================================

#[test]
fn test_ctrl_backspace_with_selection_deletes_selection() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 0;
    selection.head = 5; // Select "hello"

    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " world");
    assert_eq!(cursor.pos, 0);
    assert_eq!(selection.anchor, 0);
    assert_eq!(selection.head, 0);
}

#[test]
fn test_ctrl_delete_with_selection_deletes_selection() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 6;
    selection.head = 11; // Select "world"

    let mut cursor = TextCursor::at(11);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello ");
    assert_eq!(cursor.pos, 6);
}

#[test]
fn test_selection_delete_mixed_utf8() {
    let mut text = String::from("hello мир world");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::new();
    selection.anchor = 6;
    selection.head = 12; // Select "мир"

    let mut cursor = TextCursor::at(12);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello  world");
    assert_eq!(cursor.pos, 6);
}

// ============================================================================
// 5.2 Ctrl+Backspace ASCII
// ============================================================================

#[test]
fn test_ctrl_backspace_at_word_end() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5); // After "hello"
    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_backspace_in_middle_of_word() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3); // After "hel"
    let mut cursor = TextCursor::at(3);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "lo world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_backspace_on_separator() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(6); // After "hello "
    let mut cursor = TextCursor::at(6);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_backspace_multiple_separators() {
    let mut text = String::from("hello   world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(8); // After "hello   "
    let mut cursor = TextCursor::at(8);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_backspace_at_start_noop() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 5.3 Ctrl+Delete ASCII
// ============================================================================

#[test]
fn test_ctrl_delete_start_of_word() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_delete_middle_of_word() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(2); // At "llo"
    let mut cursor = TextCursor::at(2);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "he world");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_ctrl_delete_on_separator() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5); // At space
    let mut cursor = TextCursor::at(5);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_ctrl_delete_multiple_separators() {
    let mut text = String::from("hello   world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5); // At first space
    let mut cursor = TextCursor::at(5);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_ctrl_delete_at_end_noop() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 5);
}

// ============================================================================
// 5.4 UTF-8 Word Deletion
// ============================================================================

#[test]
fn test_ctrl_backspace_cyrillic() {
    let mut text = String::from("Привет мир");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::at(12); // After "Привет"
    let mut cursor = TextCursor::at(12);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " мир");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_delete_cyrillic() {
    let mut text = String::from("Привет мир");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " мир");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_backspace_emoji_with_words() {
    let mut text = String::from("hello👍world");
    let boundaries = make_utf8_boundaries(&text);

    // Cursor after emoji (byte 9)
    let mut selection = Selection::at(9);
    let mut cursor = TextCursor::at(9);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    // Emoji is not a word char, so we delete it and the previous word
    assert_eq!(text, "world");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_ctrl_delete_emoji_with_words() {
    let mut text = String::from("hello👍world");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::at(5); // At emoji
    let mut cursor = TextCursor::at(5);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    // Emoji is separator, then delete "world"
    assert_eq!(text, "hello");
    assert_eq!(cursor.pos, 5);
}

#[test]
fn test_utf8_string_stays_valid() {
    let mut text = String::from("café résumé");
    let boundaries = make_utf8_boundaries(&text);

    let mut selection = Selection::at(5); // After "café"
    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " résumé");
    assert!(text.is_char_boundary(cursor.pos));
}

// ============================================================================
// 5.5 Cluster-Aligned Deletion
// ============================================================================

#[test]
fn test_deletion_aligned_to_clusters() {
    let mut text = String::from("abc def");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3); // After "abc"
    let mut cursor = TextCursor::at(3);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " def");
    // Verify cursor at valid boundary
    assert!(text.is_char_boundary(cursor.pos));
}

#[test]
fn test_multi_cluster_word_deletion() {
    // Create boundaries where each char is a cluster
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(11); // End
    let mut cursor = TextCursor::at(11);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "hello ");
    assert_eq!(cursor.pos, 6);
}

// ============================================================================
// 5.6 Multi-Word Delete Scenarios
// ============================================================================

#[test]
fn test_multiple_spaces_backspace() {
    let mut text = String::from("foo   bar");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(6); // In spaces
    let mut cursor = TextCursor::at(6);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "bar");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_multiple_spaces_delete() {
    let mut text = String::from("foo   bar");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3); // At first space
    let mut cursor = TextCursor::at(3);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "foo");
    assert_eq!(cursor.pos, 3);
}

#[test]
fn test_hyphen_as_separator() {
    let mut text = String::from("foo-bar");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(3); // At hyphen
    let mut cursor = TextCursor::at(3);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "-bar");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_apostrophe_in_word_backspace() {
    let mut text = String::from("don't stop");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5); // After "don't"
    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " stop");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_apostrophe_in_word_delete() {
    let mut text = String::from("don't stop");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " stop");
    assert_eq!(cursor.pos, 0);
}

// ============================================================================
// 5.7 Integration
// ============================================================================

#[test]
fn test_word_delete_then_insert() {
    let mut text = String::from("hello world");
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    // Delete "hello"
    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);
    assert_eq!(text, " world");

    // Regenerate boundaries
    boundaries = make_ascii_boundaries(&text);

    // Insert replacement
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "hi");

    assert_eq!(text, "hi world");
    assert_eq!(cursor.pos, 2);
}

#[test]
fn test_multiple_word_deletes() {
    let mut text = String::from("one two three");
    let mut boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(13); // End
    let mut cursor = TextCursor::at(13);

    // Delete "three"
    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);
    assert_eq!(text, "one two ");
    boundaries = make_ascii_boundaries(&text);

    // Delete " two"
    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);
    assert_eq!(text, "one ");
    boundaries = make_ascii_boundaries(&text);

    // Delete " one"
    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);
    assert_eq!(text, "");
}

#[test]
fn test_invariant_after_word_delete() {
    let mut text = String::from("hello world");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    // Check invariant
    assert_eq!(selection.anchor, selection.head);
    assert_eq!(selection.anchor, cursor.pos);
    assert_eq!(cursor.preferred_x, None);
}

// ============================================================================
// Additional Edge Cases
// ============================================================================

#[test]
fn test_single_word_backspace() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(5);
    let mut cursor = TextCursor::at(5);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_single_word_delete() {
    let mut text = String::from("hello");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(0);
    let mut cursor = TextCursor::at(0);

    apply_delete_word_forward(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, "");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_underscore_in_word() {
    let mut text = String::from("foo_bar baz");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(7); // After "foo_bar"
    let mut cursor = TextCursor::at(7);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " baz");
    assert_eq!(cursor.pos, 0);
}

#[test]
fn test_numbers_in_word() {
    let mut text = String::from("abc123 xyz");
    let boundaries = make_ascii_boundaries(&text);

    let mut selection = Selection::at(6); // After "abc123"
    let mut cursor = TextCursor::at(6);

    apply_delete_word_back(&mut text, &mut selection, &mut cursor, &boundaries);

    assert_eq!(text, " xyz");
    assert_eq!(cursor.pos, 0);
}
