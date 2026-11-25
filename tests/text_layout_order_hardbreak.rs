// TextArea Layout Order Tests — Hard Break Cases
//
// These tests verify that after layout, run ranges are in increasing document order.
// If runs are produced in reverse order (e.g., [598..620] before [1..21]), these tests FAIL.

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
    // Filter out empty ranges
    let non_empty: Vec<_> = ranges.iter().filter(|r| !r.is_empty()).collect();

    if non_empty.is_empty() {
        return; // Nothing to check
    }

    // Check all ranges are within bounds
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

    // Check monotonic ordering: each range starts at or after the previous ends
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

/// Assert first non-empty range starts at expected position.
fn assert_first_range_start(ranges: &[Range<usize>], expected_start: usize, context: &str) {
    let first_non_empty = ranges.iter().find(|r| !r.is_empty());
    if let Some(first) = first_non_empty {
        assert_eq!(
            first.start, expected_start,
            "{}: First non-empty range should start at {}, got {:?}. All ranges: {:?}",
            context, expected_start, first, ranges
        );
    }
}

// ============================================================================
// Test Cases
// ============================================================================

#[test]
fn test_layout_order_simple_multiline() {
    // Case 1: "A\nB\nC"
    let text = "A\nB\nC";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_simple_multiline] text={:?}, ranges={:?}",
        text, ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "A\\nB\\nC");

    // First range should start at 0
    assert_first_range_start(&ranges, 0, "A\\nB\\nC");
}

#[test]
fn test_layout_order_textarea_sample() {
    // Case 2: The actual TextArea sample text
    let text = "Welcome to TextArea!\nEnd of sample content.";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_textarea_sample] text={:?}, ranges={:?}",
        text, ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "Welcome to TextArea! sample");

    // First range should start at 0
    assert_first_range_start(&ranges, 0, "Welcome to TextArea! sample");
}

#[test]
fn test_layout_order_leading_newline() {
    // Case 3: "\nLeading newline then text"
    let text = "\nLeading newline then text";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_leading_newline] text={:?}, ranges={:?}",
        text, ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "Leading newline");

    // First non-empty range should start at 1 (after the newline)
    // OR at 0 if the newline is included in a run
    // Let's be flexible: first range start should be <= 1
    let first_non_empty = ranges.iter().find(|r| !r.is_empty());
    if let Some(first) = first_non_empty {
        assert!(
            first.start <= 1,
            "Leading newline: first range should start at 0 or 1, got {:?}",
            first
        );
    }
}

#[test]
fn test_layout_order_empty_line() {
    // Case 4: "Line1\n\nLine3" (empty line in middle)
    let text = "Line1\n\nLine3";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_empty_line] text={:?}, ranges={:?}",
        text, ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "Empty line");

    // First range should start at 0
    assert_first_range_start(&ranges, 0, "Empty line");
}

#[test]
fn test_layout_order_long_text_with_newlines() {
    // A longer text with multiple lines
    let text = "First line of text here.\nSecond line is also here.\nThird line completes it.";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_long_text] text len={}, ranges={:?}",
        text.len(),
        ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "Long text");

    // First range should start at 0
    assert_first_range_start(&ranges, 0, "Long text");

    // Additional check: ranges should cover most of the text
    let total_coverage: usize = ranges.iter().map(|r| r.end - r.start).sum();
    assert!(
        total_coverage > 0,
        "Long text: should have some coverage, got {}",
        total_coverage
    );
}

#[test]
fn test_layout_order_single_line() {
    // Single line - baseline sanity check
    let text = "Just a single line";

    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();

    cache.rebuild(text, 1, 300.0, &mut text_engine);

    let ranges = collect_run_ranges(&cache);
    println!(
        "[test_layout_order_single_line] text={:?}, ranges={:?}",
        text, ranges
    );

    // Assert ranges are in order
    assert_monotonic_ranges(&ranges, text, "Single line");

    // First range should start at 0
    assert_first_range_start(&ranges, 0, "Single line");
}
