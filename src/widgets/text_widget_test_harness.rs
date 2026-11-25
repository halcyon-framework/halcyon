// SPEC-HAL-109 Stage J: Shared Test Harness for Text Widgets
//
// This module provides a unified test interface for TextField and TextArea,
// eliminating code duplication while preserving widget-specific functionality.
//
// Common operations (trait-based):
// - Focus state manipulation
// - Cursor/selection manipulation
// - Boundary injection
//
// Widget-specific operations remain in their respective modules:
// - TextField: horizontal scroll (scroll_x), viewport construction
// - TextArea: vertical scroll (ScrollContainer), line data manipulation

#[cfg(any(test, feature = "test-harness"))]
use crate::core::editing::{ClusterBoundary, Selection, TextCursor};
#[cfg(any(test, feature = "test-harness"))]
use crate::widgets::text_layout_cache::TextLayoutCache;

/// Shared test harness interface for text editing widgets.
///
/// Both TextField and TextArea implement this trait to provide
/// consistent test APIs for common editing state manipulation.
#[cfg(any(test, feature = "test-harness"))]
pub trait TextWidgetTestHarness {
    /// Access to the widget's focused state for testing.
    fn focused_mut(&mut self) -> &mut bool;

    /// Access to the widget's cursor for testing.
    fn cursor_mut(&mut self) -> &mut TextCursor;

    /// Access to the widget's selection for testing.
    fn selection_mut(&mut self) -> &mut Selection;

    /// Access to the widget's layout cache for testing.
    fn layout_cache_mut(&mut self) -> &mut TextLayoutCache;

    // ========================================================================
    // Default implementations using the accessors above
    // ========================================================================

    /// Set focused state directly for testing.
    /// Does NOT call focus handlers or trigger side effects.
    fn set_focused_for_test(&mut self, focused: bool) {
        *self.focused_mut() = focused;
    }

    /// Inject pre-built cluster boundaries for testing.
    /// Bypasses layout() entirely - writes directly to layout_cache.
    fn set_boundaries_for_test(&mut self, boundaries: Vec<ClusterBoundary>) {
        self.layout_cache_mut().set_boundaries_for_test(boundaries);
    }

    /// K.INV1: Set cached document length to satisfy freshness guards.
    /// Call this after set_boundaries_for_test with the actual document length.
    fn set_cached_doc_len_for_test(&mut self, doc_len: usize) {
        self.layout_cache_mut().set_cached_doc_len_for_test(doc_len);
    }

    /// Set cursor position directly for testing.
    fn set_cursor_for_test(&mut self, pos: usize) {
        self.cursor_mut().pos = pos;
    }

    /// Set selection range directly for testing.
    /// Sets anchor and head, and moves cursor to head.
    fn set_selection_for_test(&mut self, anchor: usize, head: usize) {
        self.selection_mut().anchor = anchor;
        self.selection_mut().head = head;
        self.cursor_mut().pos = head;
    }

    /// Get cursor byte position for test assertions.
    fn cursor_pos_for_test(&self) -> usize
    where
        Self: TextWidgetTestHarnessRead,
    {
        self.cursor_ref().pos
    }

    /// Get selection range for test assertions.
    /// Returns (anchor, head).
    fn selection_for_test(&self) -> (usize, usize)
    where
        Self: TextWidgetTestHarnessRead,
    {
        let sel = self.selection_ref();
        (sel.anchor, sel.head)
    }
}

/// Read-only accessors for test assertions.
/// Separate trait to allow immutable access patterns.
#[cfg(any(test, feature = "test-harness"))]
pub trait TextWidgetTestHarnessRead {
    /// Read-only access to cursor.
    fn cursor_ref(&self) -> &TextCursor;

    /// Read-only access to selection.
    fn selection_ref(&self) -> &Selection;
}
