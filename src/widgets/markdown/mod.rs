//! HAL390: immutable, source-bearing Markdown preparation and a native read-only leaf.
//!
//! Preparation is pure CPU work. Supply the resulting `Arc<MarkdownDocument>` at
//! the ordinary widget construction/update boundary. Place the view in one
//! finite-width `ScrollArea` with both axes enabled; no parsing occurs in a frame.
mod document;
mod view;

pub use document::{MarkdownDocument, MarkdownLimitKind, MarkdownLimits, MarkdownPrepareError};
pub use view::{
    MarkdownLayoutError, MarkdownReadHandoff, MarkdownReadHandoffError, MarkdownReadHandoffStatus,
    MarkdownSelectionRetention, MarkdownStyle, MarkdownStyleError, MarkdownView,
};

#[cfg(test)]
mod tests;
