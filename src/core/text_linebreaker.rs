//! SPEC-HAL-025: Multi-line Text Layout - LineBreaker
//!
//! DEPRECATED: This module is maintained for backward compatibility with TextBlock.
//! For new code, use `ParagraphLayoutEngine` from `crate::core::paragraph`.
//!
//! Migration guide:
//! - `LineModel` → `Line` + `Run` from paragraph.rs
//! - `TextAlign` → Re-exported from paragraph.rs (canonical location)
//! - `LineBreaker::break_lines()` → `ParagraphLayoutEngine::layout_with_fonts()`

use crate::core::text_engine::{GlyphRun, TextEngine};

// Re-export TextAlign from canonical location (SPEC-048)
pub use crate::core::paragraph::TextAlign;

/// A single line of text with layout information.
///
/// DEPRECATED: Use `Line` from `crate::core::paragraph` for new code.
/// This struct is maintained for TextBlock compatibility.
#[derive(Debug, Clone)]
pub struct LineModel {
    /// Byte range into original text
    pub start: usize,
    pub end: usize,
    /// Shaped glyph run for this line
    pub run: GlyphRun,
    /// Measured width of this line
    pub width: f32,
    /// Line height from metrics
    pub line_height: f32,
}

/// LineBreaker - breaks text into lines based on width constraints
pub struct LineBreaker;

impl LineBreaker {
    /// Break text into lines that fit within max_width
    ///
    /// Algorithm:
    /// 1. Split into paragraphs by \n
    /// 2. For each paragraph, split into tokens (words + whitespace)
    /// 3. Greedy packing: add tokens until width exceeds max_width
    /// 4. If single token > max_width, break by characters
    ///
    /// Each line gets its own metrics from the shaped run.
    pub fn break_lines(
        text: &str,
        max_width: f32,
        font_size: f32,
        text_engine: &mut dyn TextEngine,
    ) -> Vec<LineModel> {
        let mut lines = Vec::new();

        // Handle empty text
        if text.is_empty() {
            let run = text_engine.shape("", font_size);
            let metrics = text_engine.metrics("", font_size);
            lines.push(LineModel {
                start: 0,
                end: 0,
                run,
                width: 0.0,
                line_height: metrics.line_height,
            });
            return lines;
        }

        // Split into paragraphs by \n
        let mut paragraph_start = 0;
        for (i, c) in text.char_indices() {
            if c == '\n' {
                // Process paragraph up to this point
                Self::break_paragraph(
                    text,
                    paragraph_start,
                    i,
                    max_width,
                    font_size,
                    text_engine,
                    &mut lines,
                );
                paragraph_start = i + 1;
            }
        }

        // Process final paragraph
        if paragraph_start <= text.len() {
            Self::break_paragraph(
                text,
                paragraph_start,
                text.len(),
                max_width,
                font_size,
                text_engine,
                &mut lines,
            );
        }

        lines
    }

    fn break_paragraph(
        text: &str,
        para_start: usize,
        para_end: usize,
        max_width: f32,
        font_size: f32,
        text_engine: &mut dyn TextEngine,
        lines: &mut Vec<LineModel>,
    ) {
        let paragraph = &text[para_start..para_end];

        // Handle empty paragraph
        if paragraph.is_empty() {
            let run = text_engine.shape("", font_size);
            let metrics = text_engine.metrics("", font_size);
            lines.push(LineModel {
                start: para_start,
                end: para_end,
                run,
                width: 0.0,
                line_height: metrics.line_height,
            });
            return;
        }

        // Tokenize into words and whitespace
        let tokens = Self::tokenize(paragraph);

        let mut line_start = 0;
        let mut line_tokens: Vec<&str> = Vec::new();
        let mut line_width = 0.0;

        for token in &tokens {
            let token_width = text_engine.measure(token, font_size).0;

            // Check if adding this token would exceed max_width
            if !line_tokens.is_empty() && line_width + token_width > max_width {
                // Emit current line with per-line metrics
                let line_text: String = line_tokens.concat();
                let line_end = line_start + line_text.len();
                let run = text_engine.shape(&line_text, font_size);
                let metrics = text_engine.metrics(&line_text, font_size);
                let width = run.width;

                lines.push(LineModel {
                    start: para_start + line_start,
                    end: para_start + line_end,
                    run,
                    width,
                    line_height: metrics.line_height,
                });

                line_start = line_end;
                line_tokens.clear();
                line_width = 0.0;
            }

            // Handle token that's wider than max_width (break by characters)
            if token_width > max_width && line_tokens.is_empty() {
                Self::break_long_token(
                    text,
                    para_start + line_start,
                    token,
                    max_width,
                    font_size,
                    text_engine,
                    lines,
                );
                line_start += token.len();
                continue;
            }

            line_tokens.push(token);
            line_width += token_width;
        }

        // Emit remaining tokens with per-line metrics
        if !line_tokens.is_empty() {
            let line_text: String = line_tokens.concat();
            let run = text_engine.shape(&line_text, font_size);
            let metrics = text_engine.metrics(&line_text, font_size);
            let width = run.width;

            lines.push(LineModel {
                start: para_start + line_start,
                end: para_end,
                run,
                width,
                line_height: metrics.line_height,
            });
        }
    }

    fn break_long_token(
        _text: &str,
        token_start: usize,
        token: &str,
        max_width: f32,
        font_size: f32,
        text_engine: &mut dyn TextEngine,
        lines: &mut Vec<LineModel>,
    ) {
        // Break by characters
        let mut char_start = 0;
        let mut current_chars = String::new();
        let mut current_width = 0.0;

        for (i, c) in token.char_indices() {
            let char_str = &token[i..i + c.len_utf8()];
            let char_width = text_engine.measure(char_str, font_size).0;

            if !current_chars.is_empty() && current_width + char_width > max_width {
                // Emit current characters with per-line metrics
                let run = text_engine.shape(&current_chars, font_size);
                let metrics = text_engine.metrics(&current_chars, font_size);
                let width = run.width;

                lines.push(LineModel {
                    start: token_start + char_start,
                    end: token_start + i,
                    run,
                    width,
                    line_height: metrics.line_height,
                });

                char_start = i;
                current_chars.clear();
                current_width = 0.0;
            }

            current_chars.push(c);
            current_width += char_width;
        }

        // Emit remaining characters with per-line metrics
        if !current_chars.is_empty() {
            let run = text_engine.shape(&current_chars, font_size);
            let metrics = text_engine.metrics(&current_chars, font_size);
            let width = run.width;

            lines.push(LineModel {
                start: token_start + char_start,
                end: token_start + token.len(),
                run,
                width,
                line_height: metrics.line_height,
            });
        }
    }

    /// Split text into tokens (words and whitespace)
    fn tokenize(text: &str) -> Vec<&str> {
        let mut tokens = Vec::new();
        let mut start = 0;
        let mut in_whitespace = text
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(false);

        for (i, c) in text.char_indices() {
            let is_ws = c.is_whitespace();
            if is_ws != in_whitespace {
                if i > start {
                    tokens.push(&text[start..i]);
                }
                start = i;
                in_whitespace = is_ws;
            }
        }

        // Push final token
        if start < text.len() {
            tokens.push(&text[start..]);
        }

        tokens
    }
}
