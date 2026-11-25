// TextArea Layout Order Tests — After Enter Insertion
//
// These tests verify that after inserting a newline (Enter key),
// the rebuilt layout has run ranges in increasing document order.
//
// This simulates EXACTLY what TextArea does:
// 1. Get boundaries from current layout
// 2. Call apply_insert_op with "\n"
// 3. Rebuild layout from new text
// 4. Verify ordering

use halcyon::core::editing::{Selection, TextCursor, apply_insert_op};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::text_layout_cache::TextLayoutCache;
use std::ops::Range;

// ============================================================================
// Test Helpers
// ============================================================================

/// Collect all run ranges from lines in document order.
fn collect_run_ranges(cache: &TextLayoutCache) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for line in cache.lines() {
        for run in &line.runs {
            ranges.push(run.range.clone());
        }
    }
    ranges
}

/// Assert that run ranges are monotonically increasing (start >= prev_end).
fn assert_monotonic_ranges(ranges: &[Range<usize>], text: &str, context: &str) {
    let non_empty: Vec<_> = ranges.iter().filter(|r| !r.is_empty()).collect();

    if non_empty.is_empty() {
        return;
    }

    for (i, range) in non_empty.iter().enumerate() {
        assert!(
            range.start <= text.len() && range.end <= text.len(),
            "{}: Range {} ({:?}) exceeds text length {}",
            context,
            i,
            range,
            text.len()
        );
    }

    for i in 1..non_empty.len() {
        let prev = non_empty[i - 1];
        let curr = non_empty[i];

        assert!(
            curr.start >= prev.end,
            "{}: Ranges OUT OF ORDER! \
             Range {} ({:?}) starts before range {} ({:?}) ends. \
             All ranges: {:?}",
            context,
            i,
            curr,
            i - 1,
            prev,
            ranges
        );
    }
}

// ============================================================================
// Test Cases — Enter Insertion
// ============================================================================

#[test]
fn test_enter_middle_of_first_line_layout_order() {
    // Initial text
    let mut text = String::from("Welcome to TextArea!");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    // Build initial layout
    cache.rebuild(&text, 1, 300.0, &mut text_engine);
    let boundaries_before = cache.boundaries().to_vec();

    println!(
        "[test_enter_middle] BEFORE: text={:?}, boundaries={:?}",
        text,
        boundaries_before
            .iter()
            .map(|b| b.byte_start)
            .collect::<Vec<_>>()
    );

    // Simulate Enter at position 7 (after "Welcome")
    let cursor_pos = 7;
    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    // This is EXACTLY what TextArea does: use OLD boundaries with insert
    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries_before,
        "\n",
    );

    println!(
        "[test_enter_middle] AFTER INSERT: text={:?}, cursor={}",
        text, cursor.pos
    );

    // Now rebuild layout from NEW text
    cache.rebuild(&text, 2, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!("[test_enter_middle] AFTER REBUILD: ranges={:?}", ranges);

    // THE CRITICAL ASSERTION: ranges must be in order
    assert_monotonic_ranges(&ranges, &text, "After Enter at middle");

    // Sanity check: text should be "Welcome\n to TextArea!"
    assert_eq!(text, "Welcome\n to TextArea!");
}

#[test]
fn test_enter_at_start_layout_order() {
    let mut text = String::from("Hello World");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    // Build initial layout
    cache.rebuild(&text, 1, 300.0, &mut text_engine);
    let boundaries_before = cache.boundaries().to_vec();

    // Enter at position 0
    let mut cursor = TextCursor::at(0);
    let mut selection = Selection::at(0);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries_before,
        "\n",
    );

    println!("[test_enter_at_start] AFTER INSERT: text={:?}", text);

    // Rebuild layout
    cache.rebuild(&text, 2, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!("[test_enter_at_start] ranges={:?}", ranges);

    assert_monotonic_ranges(&ranges, &text, "After Enter at start");
    assert_eq!(text, "\nHello World");
}

#[test]
fn test_enter_at_end_layout_order() {
    let mut text = String::from("Hello World");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    // Build initial layout
    cache.rebuild(&text, 1, 300.0, &mut text_engine);
    let boundaries_before = cache.boundaries().to_vec();

    // Enter at end (position 11)
    let cursor_pos = text.len();
    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries_before,
        "\n",
    );

    println!("[test_enter_at_end] AFTER INSERT: text={:?}", text);

    // Rebuild layout
    cache.rebuild(&text, 2, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!("[test_enter_at_end] ranges={:?}", ranges);

    assert_monotonic_ranges(&ranges, &text, "After Enter at end");
    assert_eq!(text, "Hello World\n");
}

#[test]
fn test_enter_creates_empty_line_layout_order() {
    // Text with existing newline, add another to create empty line
    let mut text = String::from("Line1\nLine2");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(&text, 1, 300.0, &mut text_engine);
    let boundaries_before = cache.boundaries().to_vec();

    // Enter at position 6 (start of "Line2")
    let cursor_pos = 6;
    let mut cursor = TextCursor::at(cursor_pos);
    let mut selection = Selection::at(cursor_pos);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries_before,
        "\n",
    );

    println!("[test_empty_line] AFTER INSERT: text={:?}", text);

    cache.rebuild(&text, 2, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!("[test_empty_line] ranges={:?}", ranges);

    assert_monotonic_ranges(&ranges, &text, "After Enter creates empty line");
    assert_eq!(text, "Line1\n\nLine2");
}

#[test]
fn test_multiple_enters_layout_order() {
    let mut text = String::from("ABCDEF");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    // First Enter at position 2
    cache.rebuild(&text, 1, 300.0, &mut text_engine);
    let boundaries = cache.boundaries().to_vec();

    let mut cursor = TextCursor::at(2);
    let mut selection = Selection::at(2);
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries, "\n");

    println!("[test_multiple_enters] After first Enter: text={:?}", text);

    // Rebuild and check
    cache.rebuild(&text, 2, 300.0, &mut text_engine);
    let ranges1 = collect_run_ranges(&cache);
    assert_monotonic_ranges(&ranges1, &text, "After first Enter");

    // Second Enter at position 5 (after "CD")
    let boundaries2 = cache.boundaries().to_vec();
    cursor.pos = 5;
    selection.anchor = 5;
    selection.head = 5;
    apply_insert_op(&mut text, &mut selection, &mut cursor, &boundaries2, "\n");

    println!("[test_multiple_enters] After second Enter: text={:?}", text);

    // Rebuild and check
    cache.rebuild(&text, 3, 300.0, &mut text_engine);
    let ranges2 = collect_run_ranges(&cache);
    println!("[test_multiple_enters] final ranges={:?}", ranges2);

    assert_monotonic_ranges(&ranges2, &text, "After second Enter");
    assert_eq!(text, "AB\nCD\nEF");
}

#[test]
fn test_textarea_exact_scenario() {
    // Reproduce the exact scenario from TextArea demo
    let mut text = String::from("Welcome to TextArea!\nSecond line\nThird line");
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    // Initial layout
    cache.rebuild(&text, 1, 300.0, &mut text_engine);

    println!("[test_textarea_exact] Initial text: {:?}", text);
    let initial_ranges = collect_run_ranges(&cache);
    println!("[test_textarea_exact] Initial ranges: {:?}", initial_ranges);

    // Check initial layout is correct
    assert_monotonic_ranges(&initial_ranges, &text, "Initial layout");

    // Now simulate Enter at position 7 (after "Welcome")
    let boundaries_before = cache.boundaries().to_vec();
    let mut cursor = TextCursor::at(7);
    let mut selection = Selection::at(7);

    apply_insert_op(
        &mut text,
        &mut selection,
        &mut cursor,
        &boundaries_before,
        "\n",
    );

    println!("[test_textarea_exact] After Enter: {:?}", text);

    // Rebuild layout
    cache.rebuild(&text, 2, 300.0, &mut text_engine);

    let ranges_after = collect_run_ranges(&cache);
    println!(
        "[test_textarea_exact] After rebuild ranges: {:?}",
        ranges_after
    );

    // THE CRITICAL CHECK
    assert_monotonic_ranges(&ranges_after, &text, "After Enter in TextArea scenario");
}
