//! Search projection harness for demos and tests.
//!
//! Mediates between search intent and projection output. Holds a master
//! option list and produces projections in response to text queries.
//! All text → meaning logic lives here, nowhere else.
//!
//! This is a deliberate harness — not a stopgap, not embedded widget logic.
//! When real data providers exist, they replace this harness without
//! touching ComboBox internals.

use crate::model::selection_resolver::OptionId;

/// Match mode for filtering options against a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// Label must start with query (case-insensitive).
    Prefix,
    /// Label must contain query anywhere (case-insensitive).
    Substring,
}

impl Default for MatchMode {
    fn default() -> Self {
        MatchMode::Prefix
    }
}

/// Output of a search projection query.
pub struct ProjectionOutput {
    pub items: Vec<(OptionId, String)>,
    /// Per-item match range: (byte_offset, byte_len) parallel to items.
    /// Empty when query is empty (no highlighting needed).
    pub match_ranges: Vec<(usize, usize)>,
    pub highlight_hint: Option<OptionId>,
    pub min_visible_rows: usize,
    pub max_visible_rows: usize,
}

/// Search projection harness for demos and tests.
///
/// Pure object: no widget state, no side effects.
/// Receives a query string, produces a projection.
/// Reusable by combo_box_demo and future ComboBox tests.
pub struct SearchProjectionHarness {
    options: Vec<(OptionId, String)>,
    options_lower: Vec<String>,
    min_visible_rows: usize,
    max_visible_rows: usize,
}

impl SearchProjectionHarness {
    pub fn new(options: &[(OptionId, &str)]) -> Self {
        Self {
            options: options
                .iter()
                .map(|(id, label)| (*id, label.to_string()))
                .collect(),
            options_lower: options
                .iter()
                .map(|(_, label)| label.to_lowercase())
                .collect(),
            min_visible_rows: 0,
            max_visible_rows: 8,
        }
    }

    /// Produce a filtered projection from a query string.
    ///
    /// Empty query returns all items (no filter, no match ranges).
    /// Non-empty query filters by match_mode and returns per-item match ranges.
    /// Highlight hint is the first match.
    pub fn query(&self, text: &str, match_mode: MatchMode) -> ProjectionOutput {
        if text.is_empty() {
            return ProjectionOutput {
                items: self.options.clone(),
                match_ranges: Vec::new(),
                highlight_hint: None,
                min_visible_rows: self.min_visible_rows,
                max_visible_rows: self.max_visible_rows,
            };
        }

        let lower = text.to_lowercase();
        let query_byte_len = lower.len();
        let mut filtered = Vec::new();
        let mut match_ranges = Vec::new();

        for ((id, label), label_lower) in self.options.iter().zip(self.options_lower.iter()) {
            let offset = match match_mode {
                MatchMode::Prefix => {
                    if label_lower.starts_with(&lower) {
                        Some(0)
                    } else {
                        None
                    }
                }
                MatchMode::Substring => label_lower.find(&*lower),
            };
            if let Some(byte_offset) = offset {
                filtered.push((*id, label.clone()));
                match_ranges.push((byte_offset, query_byte_len));
            }
        }

        let highlight_hint = filtered.first().map(|(id, _)| *id);

        ProjectionOutput {
            items: filtered,
            match_ranges,
            highlight_hint,
            min_visible_rows: self.min_visible_rows,
            max_visible_rows: self.max_visible_rows,
        }
    }
}

// =============================================================================
// Immediate Search Harness (formerly debounced)
// =============================================================================

/// Search projection harness that executes queries immediately.
///
/// Wraps `SearchProjectionHarness` with the same API surface as the former
/// debounced variant. All queries execute synchronously — no debounce,
/// no artificial delay, no timer-driven state machine.
///
/// Duration parameters are accepted for API compatibility but ignored.
pub struct DebouncedSearchHarness {
    inner: SearchProjectionHarness,
    pending: Option<ProjectionOutput>,
}

impl DebouncedSearchHarness {
    pub fn new(
        options: &[(OptionId, &str)],
        _debounce: std::time::Duration,
        _delay: std::time::Duration,
    ) -> Self {
        Self {
            inner: SearchProjectionHarness::new(options),
            pending: None,
        }
    }

    /// Submit a query. Executes immediately; result available on next `poll()`.
    pub fn submit(&mut self, query: String, match_mode: MatchMode) {
        let result = self.inner.query(&query, match_mode);
        self.pending = Some(result);
    }

    /// Poll for results. Returns the result from the last `submit()`, if any.
    pub fn poll(&mut self) -> Option<ProjectionOutput> {
        self.pending.take()
    }

    /// Whether a result is pending retrieval.
    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    /// Cancel any pending result.
    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.pending = None;
    }
}
