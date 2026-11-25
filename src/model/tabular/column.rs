// =============================================================================
// SPEC-HAL-165: TabularSubstrate Column Model
// =============================================================================
//
// INVARIANT-1: Column Model is owned exclusively by TabularSubstrate.
// INVARIANT-2: Projections may observe column state but must not mutate it.
// INVARIANT-3: No other widget may create, destroy, or reorder columns.
// INVARIANT-7: Column identity is stable across frames, scrolling, and virtualization.
// =============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Scroll domain for a column.
///
/// Determines whether a column participates in horizontal scrolling or remains
/// fixed at the left edge of the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnScrollDomain {
    /// Column scrolls horizontally with the viewport (default).
    Scrollable,
    /// Column is pinned to the left edge and ignores horizontal scroll offset.
    PinnedLeft,
    /// Column is pinned to the right edge and ignores horizontal scroll offset.
    PinnedRight,
}

impl Default for ColumnScrollDomain {
    fn default() -> Self {
        Self::Scrollable
    }
}

/// Stable, opaque column identifier.
///
/// SPEC-HAL-165: Column identity is stable across frames, scrolling, and virtualization.
/// This enables header ↔ cell alignment, state externalization, and persistent
/// user customization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnId(u64);

impl ColumnId {
    /// Create a new unique ColumnId.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        ColumnId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Reconstruct a stable column identifier from persisted or provider-owned identity.
    pub const fn from_raw(raw: u64) -> Self {
        ColumnId(raw)
    }

    /// Get the raw u64 value (for debugging/serialization).
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for ColumnId {
    fn default() -> Self {
        Self::new()
    }
}

/// Absolute minimum column width (pixels). No column may be resized below this.
pub const MIN_COL_WIDTH: f32 = 32.0;

/// Column definition with layout constraints.
///
/// SPEC-HAL-165: A Column is a first-class structural unit with immutable identity.
#[derive(Debug, Clone)]
pub struct Column {
    /// Stable, opaque identifier.
    pub id: ColumnId,
    /// Minimum width constraint.
    pub min_width: f32,
    /// Maximum width constraint.
    pub max_width: f32,
    /// Preferred width (used when space allows).
    pub preferred_width: f32,
    /// Optional flex weight for distributing remaining space.
    pub flex: Option<f32>,
    /// Optional content-derived width floor for elastic columns.
    ///
    /// When set on a column that also has `flex.is_some()`, the resolver
    /// uses this as the minimum elastic width: the resolved elastic width
    /// becomes `max(flex_share, content_width)`, then clamped by
    /// `min_width` / `max_width`. When the content floor exceeds the
    /// per-flex share of remaining space, the column claims its content
    /// width and the table's `total_width` exceeds the available
    /// viewport, producing horizontal overflow that consumers (ScrollArea)
    /// expose as horizontal scroll.
    ///
    /// Has no effect on non-elastic columns. `None` (the default)
    /// preserves the pre-existing elastic math exactly.
    ///
    /// Populated by the consuming widget after measuring cell content
    /// against its own font / scale (visual-domain knowledge that does
    /// NOT belong to data sources — Constitution Pillar D).
    pub content_width: Option<f32>,
    /// Whether this column is visible.
    pub visible: bool,
    /// Scroll domain (Scrollable or PinnedLeft).
    pub scroll_domain: ColumnScrollDomain,
    /// Whether this column can be resized by the user.
    /// Default: true. When false, resize arming and resize intents
    /// are rejected for this column.
    pub resizable: bool,
    /// Whether this column can be reordered by drag.
    /// Default: true. When false, reorder drag initiation is
    /// rejected for this column.
    pub reorderable: bool,
    /// Whether this view exposes user sorting for the column.
    /// Provider capability remains authoritative for whether a request is accepted.
    pub sortable: bool,
}

impl Column {
    /// Create a new column with specified constraints.
    pub fn new(preferred_width: f32) -> Self {
        Self {
            id: ColumnId::new(),
            min_width: 0.0,
            max_width: f32::INFINITY,
            preferred_width,
            flex: None,
            content_width: None,
            visible: true,
            scroll_domain: ColumnScrollDomain::default(),
            resizable: true,
            reorderable: true,
            sortable: true,
        }
    }

    /// Create a column with explicit ID (for deserialization/persistence).
    pub fn with_id(id: ColumnId, preferred_width: f32) -> Self {
        Self {
            id,
            min_width: 0.0,
            max_width: f32::INFINITY,
            preferred_width,
            flex: None,
            content_width: None,
            visible: true,
            scroll_domain: ColumnScrollDomain::default(),
            resizable: true,
            reorderable: true,
            sortable: true,
        }
    }

    /// Set minimum width constraint.
    pub fn with_min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    /// Set maximum width constraint.
    pub fn with_max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    /// Set a content-derived width floor for the elastic column.
    /// See `Column::content_width` for semantics.
    pub fn with_content_width(mut self, content_width: f32) -> Self {
        self.content_width = Some(content_width);
        self
    }

    /// Set flex weight for space distribution.
    pub fn with_flex(mut self, flex: f32) -> Self {
        self.flex = Some(flex);
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set scroll domain.
    pub fn with_scroll_domain(mut self, domain: ColumnScrollDomain) -> Self {
        self.scroll_domain = domain;
        self
    }

    /// Set whether this column can be resized by the user.
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Set whether this column can be reordered by drag.
    pub fn with_reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// Set whether this view exposes user sorting for the column.
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }
}

/// Column model owned by TabularSubstrate.
///
/// SPEC-HAL-165 INVARIANT-1: Owned exclusively by TabularSubstrate.
/// SPEC-HAL-165 INVARIANT-2: Projections may observe but not mutate.
/// SPEC-HAL-165 INVARIANT-3: No other widget may create, destroy, or reorder columns.
#[derive(Debug)]
pub struct ColumnModel {
    /// Ordered list of columns.
    columns: Vec<Column>,
    /// Version counter for change detection.
    version: u64,
}

impl ColumnModel {
    /// Create an empty column model.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            version: 0,
        }
    }

    /// Create a column model with initial columns.
    pub fn with_columns(columns: Vec<Column>) -> Self {
        Self {
            columns,
            version: 0,
        }
    }

    /// Get the current version (incremented on any mutation).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get all columns (ordered).
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get visible columns only (preserves order).
    pub fn visible_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.iter().filter(|c| c.visible)
    }

    /// Get column count.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Get column by ID.
    pub fn get(&self, id: ColumnId) -> Option<&Column> {
        self.columns.iter().find(|c| c.id == id)
    }

    /// Get column index by ID.
    pub fn index_of(&self, id: ColumnId) -> Option<usize> {
        self.columns.iter().position(|c| c.id == id)
    }

    /// Add a column at the end.
    ///
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn push(&mut self, column: Column) {
        self.columns.push(column);
        self.version += 1;
    }

    /// Insert a column at index.
    ///
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn insert(&mut self, index: usize, column: Column) {
        self.columns.insert(index, column);
        self.version += 1;
    }

    /// Remove column by ID.
    ///
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn remove(&mut self, id: ColumnId) -> Option<Column> {
        if let Some(index) = self.index_of(id) {
            self.version += 1;
            Some(self.columns.remove(index))
        } else {
            None
        }
    }

    /// Set column visibility.
    ///
    /// SPEC-HAL-165: Visibility toggling is a structural mutation and triggers
    /// layout invalidation.
    pub fn set_visible(&mut self, id: ColumnId, visible: bool) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| c.id == id) {
            if col.visible != visible {
                col.visible = visible;
                self.version += 1;
                return true;
            }
        }
        false
    }

    /// Update preferred width for a column.
    ///
    /// SPEC-HAL-165: Structural mutation, increments version.
    pub fn set_preferred_width(&mut self, id: ColumnId, width: f32) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| c.id == id) {
            col.preferred_width = width;
            self.version += 1;
            return true;
        }
        false
    }

    /// Set the content-derived width floor for a column.
    ///
    /// Mirrors `set_preferred_width` in shape: find by ID, mutate, bump
    /// `version`, return whether the column existed and the value
    /// changed. `width = None` clears the floor.
    ///
    /// SPEC-HAL-165: Structural mutation, increments version. Geometry
    /// resolver re-resolves on next pass because `model.version()`
    /// changed.
    pub fn set_content_width(&mut self, id: ColumnId, width: Option<f32>) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| c.id == id) {
            if col.content_width != width {
                col.content_width = width;
                self.version += 1;
                return true;
            }
        }
        false
    }

    /// Apply a user-initiated column resize.
    ///
    /// SPEC-HAL-165 INVARIANT-1: ColumnModel is the column-geometry authority;
    /// the semantic of "user drag committed a width" lives here.
    /// SPEC-HAL-169 INVARIANT-5: model applies clamping; interaction code emits
    /// `proposed_width` only.
    ///
    /// Contract:
    /// - If the column is missing or `!resizable`: return `false`, no mutation.
    /// - Clamp `proposed_width` to `[max(col.min_width, MIN_COL_WIDTH), col.max_width]`.
    /// - INV-RESIZE-CLEARS-ELASTIC: if `col.flex.is_some()`, clear it. Manual
    ///   resize is an explicit user action that overrides elastic behavior;
    ///   without this, dragging the flex column has no visual effect because
    ///   `ColumnGeometryEngine::resolve` uses `flex_width = (flex/total_flex) * remaining_space`
    ///   and ignores `preferred_width` on flex columns.
    /// - Set `preferred_width` to the clamped value.
    /// - Bump `version` once for the combined mutation.
    ///
    /// Returns `true` if the model was mutated.
    pub fn commit_user_resize(&mut self, id: ColumnId, proposed_width: f32) -> bool {
        let Some(col) = self.columns.iter_mut().find(|c| c.id == id) else {
            return false;
        };
        if !col.resizable {
            return false;
        }
        let floor = col.min_width.max(MIN_COL_WIDTH);
        let new_width = proposed_width.max(floor).min(col.max_width);

        let mut changed = false;
        if col.flex.is_some() {
            col.flex = None;
            changed = true;
        }
        if col.preferred_width != new_width {
            col.preferred_width = new_width;
            changed = true;
        }
        if changed {
            self.version += 1;
        }
        changed
    }

    /// Freeze visible elastic columns at their current resolved widths before a user resize.
    ///
    /// User resizing must not let a flex sibling absorb the target column's delta.
    /// The caller supplies frame-resolved widths from the geometry authority; this
    /// method clears only columns that are still elastic.
    pub fn freeze_elastic_widths<I>(&mut self, widths: I) -> bool
    where
        I: IntoIterator<Item = (ColumnId, f32)>,
    {
        let mut changed = false;

        for (id, resolved_width) in widths {
            let Some(col) = self.columns.iter_mut().find(|c| c.id == id) else {
                continue;
            };
            if col.flex.is_none() {
                continue;
            }

            let floor = col.min_width.max(MIN_COL_WIDTH);
            let frozen_width = resolved_width.max(floor).min(col.max_width);
            col.flex = None;
            changed = true;

            if col.preferred_width != frozen_width {
                col.preferred_width = frozen_width;
            }
        }

        if changed {
            self.version += 1;
        }
        changed
    }

    /// Set scroll domain for a column.
    ///
    /// Returns true if the domain was changed, false if column not found or already set.
    pub fn set_scroll_domain(&mut self, id: ColumnId, domain: ColumnScrollDomain) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| c.id == id) {
            if col.scroll_domain != domain {
                col.scroll_domain = domain;
                // INV-SCROLL-DOMAIN-ELASTIC: Pinned columns must not be elastic.
                // Clear flex when pinning to prevent geometry engine panic.
                if domain != ColumnScrollDomain::Scrollable {
                    col.flex = None;
                }
                self.version += 1;
                return true;
            }
        }
        false
    }

    /// Set or clear flex weight for a column.
    ///
    /// Returns true if changed, false if column not found or already set.
    pub fn set_flex(&mut self, id: ColumnId, flex: Option<f32>) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| c.id == id) {
            if col.flex != flex {
                col.flex = flex;
                self.version += 1;
                return true;
            }
        }
        false
    }

    /// Reorder columns according to a new order.
    ///
    /// Validates: same length, every ID exists in current model, no duplicates.
    /// Returns false on validation failure or if already in given order.
    pub fn reorder(&mut self, new_order: &[ColumnId]) -> bool {
        if new_order.len() != self.columns.len() {
            debug_assert!(false, "reorder: new_order length mismatch");
            return false;
        }

        // Check for duplicates and existence.
        let mut seen = std::collections::HashSet::with_capacity(new_order.len());
        for id in new_order {
            if !seen.insert(id) {
                debug_assert!(false, "reorder: duplicate ColumnId {:?}", id);
                return false;
            }
            if self.index_of(*id).is_none() {
                debug_assert!(false, "reorder: unknown ColumnId {:?}", id);
                return false;
            }
        }

        // Check if already in given order.
        let already_ordered = self
            .columns
            .iter()
            .zip(new_order.iter())
            .all(|(c, id)| c.id == *id);
        if already_ordered {
            return false;
        }

        // Build reordered vec.
        let mut reordered = Vec::with_capacity(self.columns.len());
        for id in new_order {
            let idx = self.columns.iter().position(|c| c.id == *id).unwrap();
            reordered.push(self.columns[idx].clone());
        }
        self.columns = reordered;
        self.version += 1;
        true
    }

    /// Clear all columns.
    pub fn clear(&mut self) {
        self.columns.clear();
        self.version += 1;
    }
}

impl Default for ColumnModel {
    fn default() -> Self {
        Self::new()
    }
}
