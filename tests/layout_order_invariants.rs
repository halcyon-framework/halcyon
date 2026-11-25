// SPEC-HAL-107-V
// Any failing test indicates a violation of SPEC-HAL-107.
// Demo behavior is non-authoritative.

use halcyon::core::editing::TextCursor;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::text_layout_cache::TextLayoutCache;
use std::ops::Range;

fn rebuild_layout(text: &str, width: f32) -> TextLayoutCache {
    let mut text_engine = FontManagerTextEngine::new();
    let mut cache = TextLayoutCache::default();
    cache.rebuild(
        text,
        text.split('\n').count() as u64,
        width,
        &mut text_engine,
    );
    cache
}

fn collect_run_ranges(cache: &TextLayoutCache) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();

    for line in cache.lines() {
        for run in &line.runs {
            ranges.push(run.range.clone());
        }
    }

    ranges
}

fn non_empty_ranges(ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter(|range| !range.is_empty())
        .cloned()
        .collect()
}

fn assert_ranges_are_in_document_order(ranges: &[Range<usize>], text: &str) {
    let ranges = non_empty_ranges(ranges);

    for range in &ranges {
        assert!(range.start <= text.len());
        assert!(range.end <= text.len());
        assert!(range.start < range.end);
    }

    for pair in ranges.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        assert!(
            current.start >= previous.end,
            "layout ranges overlap or move backward: {previous:?} then {current:?}"
        );
    }
}

#[test]
fn layout_runs_are_strictly_increasing_in_byte_order() {
    let text = "alpha\nbeta\ngamma";
    let cache = rebuild_layout(text, 400.0);
    let ranges = collect_run_ranges(&cache);

    assert!(!non_empty_ranges(&ranges).is_empty());
    assert_ranges_are_in_document_order(&ranges, text);
}

#[test]
fn layout_lines_preserve_document_order() {
    let text = "alpha\nbeta\ngamma";
    let cache = rebuild_layout(text, 400.0);
    let line_starts: Vec<usize> = cache
        .lines()
        .iter()
        .filter_map(|line| {
            line.runs
                .iter()
                .filter(|run| !run.range.is_empty())
                .map(|run| run.range.start)
                .min()
        })
        .collect();

    assert_eq!(line_starts, vec![0, 6, 11]);
    assert!(line_starts.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn layout_is_deterministic_for_same_text() {
    let text = "first line\nsecond line\nthird line";
    let first = collect_run_ranges(&rebuild_layout(text, 320.0));
    let second = collect_run_ranges(&rebuild_layout(text, 320.0));

    assert_eq!(first, second);
}

#[test]
fn layout_never_swaps_or_interleaves_ranges() {
    let text = "01234\n56789\nabcde";
    let cache = rebuild_layout(text, 400.0);
    let ranges = non_empty_ranges(&collect_run_ranges(&cache));
    let mut sorted_ranges = ranges.clone();

    sorted_ranges.sort_by_key(|range| (range.start, range.end));
    assert_eq!(ranges, sorted_ranges);
    assert_ranges_are_in_document_order(&ranges, text);
}

#[test]
fn layout_rebuild_does_not_invalidate_cursor() {
    let text = "alpha\nbeta\ngamma";
    let cursor = TextCursor::at(7);
    let before = rebuild_layout(text, 400.0);
    let after = rebuild_layout(text, 180.0);

    assert!(cursor.pos <= text.len());
    assert!(
        before
            .boundaries()
            .iter()
            .any(|boundary| boundary.byte_start == cursor.pos)
    );
    assert!(
        after
            .boundaries()
            .iter()
            .any(|boundary| boundary.byte_start == cursor.pos)
    );
}
