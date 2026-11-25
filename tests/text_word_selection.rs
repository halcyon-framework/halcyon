// SPEC-HAL-052 Stage 4: Word Selection Tests

use halcyon::core::editing::{ClusterBoundary, Selection, TextCursor, find_word_at};

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
// 6.1 Simple ASCII Words
// ============================================================================

#[test]
fn test_hello_world_click_h() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on 'h' (byte 0)
    let wb = find_word_at(&boundaries, text, 0);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 5); // "hello"
    assert_eq!(&text[wb.start..wb.end], "hello");
}

#[test]
fn test_hello_world_click_space() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on space (byte 5)
    let wb = find_word_at(&boundaries, text, 5);

    assert_eq!(wb.start, 5);
    assert_eq!(wb.end, 6); // " "
    assert_eq!(&text[wb.start..wb.end], " ");
}

#[test]
fn test_hello_world_click_w() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on 'w' (byte 6)
    let wb = find_word_at(&boundaries, text, 6);

    assert_eq!(wb.start, 6);
    assert_eq!(wb.end, 11); // "world"
    assert_eq!(&text[wb.start..wb.end], "world");
}

#[test]
fn test_hello_world_click_middle_of_hello() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on 'l' (byte 2)
    let wb = find_word_at(&boundaries, text, 2);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 5);
    assert_eq!(&text[wb.start..wb.end], "hello");
}

// ============================================================================
// 6.2 Apostrophes
// ============================================================================

#[test]
fn test_dont_stop_click_apostrophe() {
    let text = "don't stop";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on apostrophe (byte 3)
    let wb = find_word_at(&boundaries, text, 3);

    // Apostrophe surrounded by word chars should be part of "don't"
    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 5);
    assert_eq!(&text[wb.start..wb.end], "don't");
}

#[test]
fn test_dont_stop_click_inside_word() {
    let text = "don't stop";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on 'o' in "don't" (byte 1)
    let wb = find_word_at(&boundaries, text, 1);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 5);
    assert_eq!(&text[wb.start..wb.end], "don't");
}

#[test]
fn test_apostrophe_at_start() {
    let text = "'hello";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on apostrophe at start (byte 0)
    let wb = find_word_at(&boundaries, text, 0);

    // Apostrophe at start is not surrounded by word chars on left
    // Should select just the apostrophe
    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 1);
    assert_eq!(&text[wb.start..wb.end], "'");
}

#[test]
fn test_apostrophe_at_end() {
    let text = "hello'";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on apostrophe at end (byte 5)
    let wb = find_word_at(&boundaries, text, 5);

    // Apostrophe at end is not followed by word char
    // Should select just the apostrophe
    assert_eq!(wb.start, 5);
    assert_eq!(wb.end, 6);
    assert_eq!(&text[wb.start..wb.end], "'");
}

// ============================================================================
// 6.3 Underscore
// ============================================================================

#[test]
fn test_foo_bar_underscore() {
    let text = "foo_bar";
    let boundaries = make_ascii_boundaries(text);

    // Double-click anywhere in "foo_bar"
    let wb = find_word_at(&boundaries, text, 0); // 'f'
    assert_eq!(&text[wb.start..wb.end], "foo_bar");

    let wb = find_word_at(&boundaries, text, 3); // '_'
    assert_eq!(&text[wb.start..wb.end], "foo_bar");

    let wb = find_word_at(&boundaries, text, 6); // 'r'
    assert_eq!(&text[wb.start..wb.end], "foo_bar");
}

#[test]
fn test_underscore_prefix() {
    let text = "_private";
    let boundaries = make_ascii_boundaries(text);

    let wb = find_word_at(&boundaries, text, 0); // '_'
    assert_eq!(&text[wb.start..wb.end], "_private");
}

// ============================================================================
// 6.4 Punctuation Run
// ============================================================================

#[test]
fn test_exclamation_marks() {
    let text = "hello!!! world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on first '!' (byte 5)
    let wb = find_word_at(&boundaries, text, 5);

    assert_eq!(wb.start, 5);
    assert_eq!(wb.end, 8); // "!!!"
    assert_eq!(&text[wb.start..wb.end], "!!!");
}

#[test]
fn test_exclamation_middle() {
    let text = "hello!!! world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on middle '!' (byte 6)
    let wb = find_word_at(&boundaries, text, 6);

    assert_eq!(&text[wb.start..wb.end], "!!!");
}

#[test]
fn test_mixed_punctuation() {
    let text = "a!?b";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on '!' (byte 1)
    let wb = find_word_at(&boundaries, text, 1);

    // '!' and '?' are both non-word chars, should group together
    assert_eq!(wb.start, 1);
    assert_eq!(wb.end, 3);
    assert_eq!(&text[wb.start..wb.end], "!?");
}

// ============================================================================
// 6.5 Mixed Unicode (Cyrillic)
// ============================================================================

#[test]
fn test_cyrillic_privet() {
    let text = "Привет мир";
    let boundaries = make_utf8_boundaries(text);

    // "Привет" is 12 bytes (6 chars * 2 bytes each)
    // Double-click on 'П' (byte 0)
    let wb = find_word_at(&boundaries, text, 0);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 12); // "Привет"
    assert_eq!(&text[wb.start..wb.end], "Привет");
}

#[test]
fn test_cyrillic_mir() {
    let text = "Привет мир";
    let boundaries = make_utf8_boundaries(text);

    // Space is at byte 12, "мир" starts at byte 13
    // Double-click on 'м' (byte 13)
    let wb = find_word_at(&boundaries, text, 13);

    assert_eq!(wb.start, 13);
    assert_eq!(wb.end, 19); // "мир"
    assert_eq!(&text[wb.start..wb.end], "мир");
}

#[test]
fn test_cyrillic_space() {
    let text = "Привет мир";
    let boundaries = make_utf8_boundaries(text);

    // Double-click on space (byte 12)
    let wb = find_word_at(&boundaries, text, 12);

    assert_eq!(wb.start, 12);
    assert_eq!(wb.end, 13);
    assert_eq!(&text[wb.start..wb.end], " ");
}

// ============================================================================
// 6.6 Word-Drag Expansion
// ============================================================================

#[test]
fn test_word_drag_forward() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on "hello"
    let initial = find_word_at(&boundaries, text, 0);
    assert_eq!(&text[initial.start..initial.end], "hello");

    // Drag into "world"
    let drag_pos = 8; // 'r' in world
    let wb = find_word_at(&boundaries, text, drag_pos);

    // Forward expansion: anchor = initial.start, head = wb.end
    let mut selection = Selection::new();
    selection.anchor = initial.start;
    selection.head = wb.end;

    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 11);
    assert_eq!(&text[selection.start()..selection.end()], "hello world");
}

#[test]
fn test_word_drag_backward() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on "world"
    let initial = find_word_at(&boundaries, text, 8);
    assert_eq!(&text[initial.start..initial.end], "world");

    // Drag backward into "hello"
    let drag_pos = 2; // 'l' in hello
    let wb = find_word_at(&boundaries, text, drag_pos);

    // Backward expansion: anchor = initial.end, head = wb.start
    let mut selection = Selection::new();
    selection.anchor = initial.end;
    selection.head = wb.start;

    assert_eq!(selection.start(), 0);
    assert_eq!(selection.end(), 11);
    assert_eq!(&text[selection.start()..selection.end()], "hello world");
}

#[test]
fn test_word_drag_into_whitespace() {
    let text = "hello   world";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on "hello"
    let initial = find_word_at(&boundaries, text, 0);
    assert_eq!(&text[initial.start..initial.end], "hello");

    // Drag into whitespace (byte 6)
    let drag_pos = 6;
    let wb = find_word_at(&boundaries, text, drag_pos);

    // wb should be the whitespace run
    assert_eq!(&text[wb.start..wb.end], "   ");

    // Forward: anchor = initial.start, head = wb.end
    let mut selection = Selection::new();
    selection.anchor = initial.start;
    selection.head = wb.end;

    assert_eq!(&text[selection.start()..selection.end()], "hello   ");
}

#[test]
fn test_word_drag_expansion_logic() {
    let text = "one two three";
    let boundaries = make_ascii_boundaries(text);

    // Double-click on "two"
    let initial = find_word_at(&boundaries, text, 4);
    assert_eq!(&text[initial.start..initial.end], "two");

    // Drag forward to "three"
    let wb_forward = find_word_at(&boundaries, text, 10);
    assert!(wb_forward.start >= initial.start);
    // Selection: anchor = initial.start, head = wb_forward.end

    // Drag backward to "one"
    let wb_backward = find_word_at(&boundaries, text, 1);
    assert!(wb_backward.end <= initial.end);
    // Selection: anchor = initial.end, head = wb_backward.start
}

// ============================================================================
// 6.7 Empty Text / Sentinel Boundaries
// ============================================================================

#[test]
fn test_empty_text() {
    let text = "";
    let boundaries: Vec<ClusterBoundary> = vec![ClusterBoundary {
        byte_start: 0,
        byte_end: 0,
        line_index: 0,
        x_logical: 0.0,
    }];

    let wb = find_word_at(&boundaries, text, 0);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 0);
}

#[test]
fn test_empty_boundaries() {
    let text = "hello";
    let boundaries: Vec<ClusterBoundary> = vec![];

    let wb = find_word_at(&boundaries, text, 0);

    assert_eq!(wb.start, 0);
    assert_eq!(wb.end, 0);
}

#[test]
fn test_sentinel_click() {
    let text = "hello";
    let boundaries = make_ascii_boundaries(text);

    // Click on sentinel (byte 5)
    let wb = find_word_at(&boundaries, text, 5);

    // Sentinel returns zero-length at position
    assert_eq!(wb.start, 5);
    assert_eq!(wb.end, 5);
}

// ============================================================================
// Double-Click Selection Integration
// ============================================================================

#[test]
fn test_double_click_selection_integration() {
    let text = "hello world";
    let boundaries = make_ascii_boundaries(text);

    // Simulate double-click on 'w' (byte 6)
    let pos = 6;
    let wb = find_word_at(&boundaries, text, pos);

    let mut selection = Selection::new();
    let mut cursor = TextCursor::new();

    // Double-click behavior
    selection.anchor = wb.start;
    selection.head = wb.end;
    cursor.collapse_to(wb.end);

    assert_eq!(selection.anchor, 6);
    assert_eq!(selection.head, 11);
    assert_eq!(cursor.pos, 11);
    assert_eq!(&text[selection.start()..selection.end()], "world");
}

// ============================================================================
// Additional Edge Cases
// ============================================================================

#[test]
fn test_single_char_word() {
    let text = "a b c";
    let boundaries = make_ascii_boundaries(text);

    let wb = find_word_at(&boundaries, text, 0);
    assert_eq!(&text[wb.start..wb.end], "a");

    let wb = find_word_at(&boundaries, text, 2);
    assert_eq!(&text[wb.start..wb.end], "b");

    let wb = find_word_at(&boundaries, text, 4);
    assert_eq!(&text[wb.start..wb.end], "c");
}

#[test]
fn test_numbers() {
    let text = "abc123def";
    let boundaries = make_ascii_boundaries(text);

    let wb = find_word_at(&boundaries, text, 0);
    assert_eq!(&text[wb.start..wb.end], "abc123def");

    let wb = find_word_at(&boundaries, text, 4);
    assert_eq!(&text[wb.start..wb.end], "abc123def");
}

#[test]
fn test_multiple_spaces() {
    let text = "a    b";
    let boundaries = make_ascii_boundaries(text);

    let wb = find_word_at(&boundaries, text, 2);
    assert_eq!(&text[wb.start..wb.end], "    ");
}
