use std::ops::Range;
use std::sync::Arc;

use crate::core::paragraph::{
    LayoutParams, Line, Paragraph, ParagraphLayoutEngine, Span, TextStyle,
};
use crate::core::text_shaper::TextShaper;

fn test_shaper() -> TextShaper {
    let font_data = std::fs::read("assets/fonts/Inter/InterVariable.ttf")
        .expect("tracked InterVariable.ttf test fixture must be readable");
    TextShaper::from_data(Arc::new(font_data)).expect("test fixture must construct a shaper")
}

fn layout(text: &str, spans: Vec<Span>, max_width: f32) -> Vec<Line> {
    let shaper = test_shaper();
    let paragraph = Paragraph::with_spans(text, spans);
    let params = LayoutParams {
        max_width,
        scale: 1.0,
        ..Default::default()
    };
    ParagraphLayoutEngine::layout(&paragraph, &shaper, &params)
}

fn line_texts(text: &str, lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.runs
                .iter()
                .map(|run| &text[run.range.clone()])
                .collect()
        })
        .collect()
}

fn line_ranges(lines: &[Line]) -> Vec<Vec<Range<usize>>> {
    lines
        .iter()
        .map(|line| line.runs.iter().map(|run| run.range.clone()).collect())
        .collect()
}

fn split_styles(first: Range<usize>, second: Range<usize>) -> Vec<Span> {
    vec![
        Span::new(first, TextStyle::default().with_size(16.0)),
        Span::new(second, TextStyle::default().with_size(20.0)),
    ]
}

#[test]
fn hard_break_embedded_in_span_keeps_post_break_line_open_for_next_span() {
    const TEXT: &str = "aa\nbbcc";
    let lines = layout(TEXT, split_styles(0..5, 5..7), f32::INFINITY);

    assert_eq!(line_texts(TEXT, &lines), ["aa", "bbcc"]);
    assert_eq!(line_ranges(&lines), [vec![0..2], vec![3..5, 5..7]]);
}

#[test]
fn hard_break_at_span_boundary_does_not_insert_an_extra_empty_line() {
    const TEXT: &str = "aa\ncc";
    let lines = layout(TEXT, split_styles(0..3, 3..5), f32::INFINITY);

    assert_eq!(line_texts(TEXT, &lines), ["aa", "cc"]);
    assert_eq!(line_ranges(&lines), [vec![0..2], vec![3..5]]);
}

#[test]
fn leading_hard_break_at_span_boundary_emits_one_empty_line() {
    const TEXT: &str = "\ncc";
    let lines = layout(TEXT, split_styles(0..1, 1..3), f32::INFINITY);

    assert_eq!(line_texts(TEXT, &lines), ["", "cc"]);
    assert_eq!(line_ranges(&lines), [vec![], vec![1..3]]);
}

#[test]
fn consecutive_hard_breaks_across_span_boundary_emit_each_line_once() {
    const TEXT: &str = "a\n\nb";
    let lines = layout(TEXT, split_styles(0..2, 2..4), f32::INFINITY);

    assert_eq!(line_texts(TEXT, &lines), ["a", "", "b"]);
    assert_eq!(line_ranges(&lines), [vec![0..1], vec![], vec![3..4]]);
}

#[test]
fn trailing_hard_break_preserves_one_canonical_empty_line() {
    const TEXT: &str = "abc\n";
    let lines = layout(
        TEXT,
        vec![Span::new(0..TEXT.len(), TextStyle::default())],
        f32::INFINITY,
    );

    assert_eq!(line_texts(TEXT, &lines), ["abc", ""]);
    assert_eq!(line_ranges(&lines), [vec![0..3], vec![]]);
    assert_eq!(lines[0].height, lines[1].height);
    assert_eq!(lines[0].baseline, lines[1].baseline);
}

#[test]
fn single_run_consecutive_hard_breaks_preserve_empty_lines() {
    const TEXT: &str = "a\n\n\nb";
    let lines = layout(
        TEXT,
        vec![Span::new(0..TEXT.len(), TextStyle::default())],
        f32::INFINITY,
    );

    assert_eq!(line_texts(TEXT, &lines), ["a", "", "", "b"]);
    assert_eq!(
        line_ranges(&lines),
        [vec![0..1], vec![], vec![], vec![4..5]]
    );
}

#[test]
fn pre_newline_content_soft_wraps_before_post_break_spans_join() {
    const TEXT: &str = "alpha beta gamma\nzq";
    let shaper = test_shaper();
    let alpha_width: f32 = shaper
        .shape_auto("alpha ", 16.0, 1.0)
        .iter()
        .map(|glyph| glyph.advance_phys)
        .sum();
    let alpha_beta_width: f32 = shaper
        .shape_auto("alpha beta ", 16.0, 1.0)
        .iter()
        .map(|glyph| glyph.advance_phys)
        .sum();
    let paragraph = Paragraph::with_spans(TEXT, split_styles(0..18, 18..19));
    let params = LayoutParams {
        max_width: (alpha_width + alpha_beta_width) * 0.5,
        scale: 1.0,
        ..Default::default()
    };
    let lines = ParagraphLayoutEngine::layout(&paragraph, &shaper, &params);
    let texts = line_texts(TEXT, &lines);

    assert!(texts.len() >= 3, "pre-newline content must soft-wrap");
    assert_eq!(texts.last().map(String::as_str), Some("zq"));
    assert_eq!(texts[..texts.len() - 1].concat(), "alpha beta gamma");
    assert_eq!(line_ranges(&lines).last(), Some(&vec![17..18, 18..19]));
}

#[test]
fn recursive_soft_wrap_keeps_9215_source_order_and_content() {
    const TEXT: &str =
        "The left half of the image is visible. Its right half is covered by an orange rectangle.";
    let lines = layout(
        TEXT,
        vec![Span::new(
            0..TEXT.len(),
            TextStyle::default().with_size(23.0),
        )],
        371.0,
    );
    let texts = line_texts(TEXT, &lines);

    assert_eq!(
        texts,
        [
            "The left half of the image is ",
            "visible. Its right half is covered by ",
            "an orange rectangle.",
        ]
    );
    assert_eq!(texts.concat(), TEXT);
}
