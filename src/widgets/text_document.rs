//! SPEC-HAL-052 Consumer: Simple UTF-8 Text Document
//!
//! A minimal text buffer for the TextEdit widget.
//! Stores UTF-8 text with change tracking for layout invalidation.

/// Simple UTF-8 text document for text editing.
///
/// Provides basic text storage with version tracking for cache invalidation.
/// This is a simple String wrapper; more sophisticated implementations
/// (ropes, piece tables) can be swapped in later.
#[derive(Debug, Clone)]
pub struct TextDocument {
    /// The text content
    content: String,
    /// Version number, incremented on each modification
    version: u64,
}

impl TextDocument {
    /// Create a new empty document.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            version: 0,
        }
    }

    /// Create a document with initial text.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            content: text.into(),
            version: 0,
        }
    }

    /// Get the text content.
    pub fn text(&self) -> &str {
        &self.content
    }

    /// Get the document version (for cache invalidation).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the length of the document in bytes.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Set the entire text content.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.content = text.into();
        self.version += 1;
    }

    /// Clear all text.
    pub fn clear(&mut self) {
        self.content.clear();
        self.version += 1;
    }

    /// Insert text at the given byte position.
    ///
    /// # Panics
    /// Panics if pos is not on a UTF-8 boundary.
    pub fn insert(&mut self, pos: usize, text: &str) {
        self.content.insert_str(pos, text);
        self.version += 1;
    }

    /// Delete a range of bytes.
    ///
    /// # Panics
    /// Panics if the range is not on UTF-8 boundaries.
    pub fn delete(&mut self, start: usize, end: usize) {
        if start < end && end <= self.content.len() {
            self.content.replace_range(start..end, "");
            self.version += 1;
        }
    }

    /// Replace a range with new text.
    ///
    /// # Panics
    /// Panics if the range is not on UTF-8 boundaries.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        if start <= end && end <= self.content.len() {
            self.content.replace_range(start..end, text);
            self.version += 1;
        }
    }

    /// Get a mutable reference to the underlying string.
    ///
    /// Caller must call `mark_modified()` after making changes.
    pub fn content_mut(&mut self) -> &mut String {
        &mut self.content
    }

    /// Mark the document as modified (increment version).
    ///
    /// Call this after using `content_mut()`.
    pub fn mark_modified(&mut self) {
        self.version += 1;
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for TextDocument {
    fn from(s: String) -> Self {
        Self::with_text(s)
    }
}

impl From<&str> for TextDocument {
    fn from(s: &str) -> Self {
        Self::with_text(s)
    }
}
