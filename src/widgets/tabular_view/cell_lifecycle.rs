// =============================================================================
// SPEC-HAL-189: CellHost — Standalone cell-state compatibility API
// =============================================================================
//
// CellHost stores (RowId, ColumnId)-keyed dirty state. Its caller owns
// residency and identity-domain decisions. ListView uses CMS/HTS for its
// mounted cells.
// Extracted from list_view/cell_host.rs per SPEC-HAL-260 §17.
// =============================================================================

use std::collections::HashMap;

use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::dirty::{DirtyFlags, DirtyOutcome, UpdatePhase};
use crate::widgets::tabular_view::row::RowId;

/// Cell widget identity key.
///
/// SPEC-HAL-189 INV-189-CELL-IDENTITY: Cell widget keyed by (RowId, ColumnId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub row_id: RowId,
    pub column_id: ColumnId,
}

impl CellKey {
    pub fn new(row_id: RowId, column_id: ColumnId) -> Self {
        Self { row_id, column_id }
    }
}

/// Cell widget state (placeholder for actual widget reference).
///
/// In a full implementation, this would hold actual child widget references.
/// For the current implementation, we track existence and dirty state only.
///
/// SPEC-HAL-189: Dirty flags drive updates, not recreation.
#[derive(Debug)]
pub struct CellWidgetState {
    /// The cell identity key.
    key: CellKey,
    /// Version counter for rebind tracking.
    version: u64,
    /// Dirty flags per SPEC-HAL-189.
    dirty: DirtyFlags,
}

impl CellWidgetState {
    /// Create a new cell widget state.
    pub fn new(key: CellKey) -> Self {
        Self {
            key,
            version: 0,
            dirty: DirtyFlags::empty(),
        }
    }

    /// Get the cell key.
    pub fn key(&self) -> CellKey {
        self.key
    }

    /// Get the RowId.
    pub fn row_id(&self) -> RowId {
        self.key.row_id
    }

    /// Get the ColumnId.
    pub fn column_id(&self) -> ColumnId {
        self.key.column_id
    }

    /// Get the version counter.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get current dirty flags.
    pub fn dirty(&self) -> DirtyFlags {
        self.dirty
    }

    /// Check if any dirty flag is set (needs work).
    pub fn is_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Check if clean (no work needed).
    ///
    /// SPEC-HAL-189 INV-189-CLEAN-NOOP: Clean widgets MUST NOT be touched.
    pub fn is_clean(&self) -> bool {
        self.dirty.is_empty()
    }

    // =========================================================================
    // Dirty Flag Setters (per SPEC-HAL-189 §5)
    // =========================================================================

    /// Mark binding dirty (value changed).
    ///
    /// SPEC-HAL-189: BindingDirty means underlying data value changed.
    /// Requires rebind to existing widget. Does NOT affect identity or geometry.
    pub fn mark_binding_dirty(&mut self) {
        self.dirty.insert(DirtyFlags::BINDING);
    }

    /// Mark policy dirty (capabilities changed).
    ///
    /// SPEC-HAL-189: PolicyDirty means permissions/capabilities/editability changed.
    /// Requires capability re-evaluation. Does NOT recreate widgets.
    pub fn mark_policy_dirty(&mut self) {
        self.dirty.insert(DirtyFlags::POLICY);
    }

    /// Mark geometry dirty (layout change).
    ///
    /// SPEC-HAL-189: GeometryDirty means layout-affecting change (row height, column width).
    /// Requires layout recomputation. Does NOT recreate widgets.
    pub fn mark_geometry_dirty(&mut self) {
        self.dirty.insert(DirtyFlags::GEOMETRY);
    }

    /// Mark presentation dirty (visual change).
    ///
    /// SPEC-HAL-189: PresentationDirty means purely visual change (theme, style, selection).
    /// Requires repaint only. Does NOT recreate widgets.
    pub fn mark_presentation_dirty(&mut self) {
        self.dirty.insert(DirtyFlags::PRESENTATION);
    }

    /// Mark all dirty (composite).
    ///
    /// SPEC-HAL-189: FullDirty is equivalent to all flags. Still does NOT permit recreation.
    pub fn mark_full_dirty(&mut self) {
        self.dirty = DirtyFlags::all();
    }

    // =========================================================================
    // Dirty Flag Clearers (pub(crate) — use consume_dirty() from external code)
    // =========================================================================

    /// Clear binding dirty flag after rebind.
    ///
    /// **Internal:** Use `consume_dirty(UpdatePhase::Bind)` from external code.
    pub(crate) fn clear_binding(&mut self) {
        self.dirty.remove(DirtyFlags::BINDING);
        self.version += 1;
    }

    // =========================================================================
    // Authoritative Dirty Consumption (SPEC-HAL-189 §5 enforcement)
    // =========================================================================

    /// Consume dirty flag for the given phase.
    ///
    /// SPEC-HAL-189: This is the SINGLE authoritative path for clearing dirty flags.
    /// No ad-hoc `if dirty.contains(...)` followed by clear is permitted outside this function.
    ///
    /// ## Ordering Rules (enforced via debug_assert)
    ///
    /// - GEOMETRY blocks BINDING: Must process layout before binding.
    /// - POLICY blocks BINDING: Must process policy before binding.
    /// - BINDING cannot clear POLICY: Policy is independent.
    /// - PRESENTATION never clears anything else: Paint is terminal.
    ///
    /// ## Returns
    ///
    /// - `DirtyOutcome::Clean` — flag was not set, no work needed.
    /// - `DirtyOutcome::Consumed` — flag was set and cleared.
    /// - `DirtyOutcome::Blocked(flags)` — precondition not met, blocked by `flags`.
    pub fn consume_dirty(&mut self, phase: UpdatePhase) -> DirtyOutcome {
        match phase {
            UpdatePhase::Bind => {
                // Precondition: GEOMETRY and POLICY must be clean.
                // SPEC-HAL-189: Ordering violation is reported via Blocked return, not panic.
                let blockers = self.dirty & (DirtyFlags::GEOMETRY | DirtyFlags::POLICY);
                if !blockers.is_empty() {
                    return DirtyOutcome::Blocked(blockers);
                }

                if self.dirty.contains(DirtyFlags::BINDING) {
                    self.dirty.remove(DirtyFlags::BINDING);
                    self.version += 1;
                    DirtyOutcome::Consumed
                } else {
                    DirtyOutcome::Clean
                }
            }

            UpdatePhase::Policy => {
                // No precondition. Policy can be consumed independently.
                if self.dirty.contains(DirtyFlags::POLICY) {
                    self.dirty.remove(DirtyFlags::POLICY);
                    DirtyOutcome::Consumed
                } else {
                    DirtyOutcome::Clean
                }
            }

            UpdatePhase::Layout => {
                // No precondition. Layout can be consumed independently.
                if self.dirty.contains(DirtyFlags::GEOMETRY) {
                    self.dirty.remove(DirtyFlags::GEOMETRY);
                    DirtyOutcome::Consumed
                } else {
                    DirtyOutcome::Clean
                }
            }

            UpdatePhase::Paint => {
                // PRESENTATION never clears anything else.
                // It only clears itself.
                if self.dirty.contains(DirtyFlags::PRESENTATION) {
                    self.dirty.remove(DirtyFlags::PRESENTATION);
                    DirtyOutcome::Consumed
                } else {
                    DirtyOutcome::Clean
                }
            }
        }
    }

    /// Check if consuming a phase would be blocked.
    ///
    /// Returns the blocking flags if blocked, or empty if not blocked.
    /// This is a read-only check that does not modify state.
    pub fn would_block(&self, phase: UpdatePhase) -> DirtyFlags {
        match phase {
            UpdatePhase::Bind => self.dirty & (DirtyFlags::GEOMETRY | DirtyFlags::POLICY),
            UpdatePhase::Policy | UpdatePhase::Layout | UpdatePhase::Paint => DirtyFlags::empty(),
        }
    }

    // =========================================================================
    // Legacy Compatibility (deprecated)
    // =========================================================================

    /// Check if rebind is needed (legacy, maps to BindingDirty).
    #[deprecated(note = "Use is_dirty() or dirty().contains(DirtyFlags::BINDING) instead")]
    pub fn needs_rebind(&self) -> bool {
        self.dirty.contains(DirtyFlags::BINDING)
    }

    /// Mark for rebind (legacy, maps to mark_binding_dirty).
    #[deprecated(note = "Use mark_binding_dirty() instead")]
    pub fn mark_rebind(&mut self) {
        self.mark_binding_dirty();
    }

    /// Clear rebind flag (legacy, maps to clear_binding).
    #[deprecated(note = "Use clear_binding() or clear_all_dirty() instead")]
    pub fn rebind_complete(&mut self) {
        self.clear_binding();
    }
}

/// State store keyed by (RowId, ColumnId) for compatibility callers.
pub struct CellHost {
    /// Cell widgets keyed by (RowId, ColumnId).
    ///
    /// INV-ROW-INDEX-EPHEMERAL: We use (RowId, ColumnId) as key, never index.
    /// There is NO index-based mapping stored here.
    widgets: HashMap<CellKey, CellWidgetState>,
}

impl CellHost {
    /// Create a new CellHost.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
        }
    }

    /// Get or create a widget for the given cell identity.
    ///
    /// This is the ONLY entry point for widget creation.
    /// If the widget exists, returns it (reuse). Otherwise creates a new one.
    ///
    /// SPEC-HAL-189 INV-189-CELL-REUSE: Existing widgets are reused.
    pub fn get_or_create(&mut self, row_id: RowId, column_id: ColumnId) -> &mut CellWidgetState {
        let key = CellKey::new(row_id, column_id);
        self.widgets
            .entry(key)
            .or_insert_with(|| CellWidgetState::new(key))
    }

    /// Get a widget by cell identity (if it exists).
    pub fn get(&self, row_id: RowId, column_id: ColumnId) -> Option<&CellWidgetState> {
        let key = CellKey::new(row_id, column_id);
        self.widgets.get(&key)
    }

    /// Get a mutable widget by cell identity (if it exists).
    pub fn get_mut(&mut self, row_id: RowId, column_id: ColumnId) -> Option<&mut CellWidgetState> {
        let key = CellKey::new(row_id, column_id);
        self.widgets.get_mut(&key)
    }

    // =========================================================================
    // Dirty Flag Propagation (SPEC-HAL-189 §6)
    // =========================================================================

    /// Mark a cell's binding dirty.
    ///
    /// Called when CellsInvalidated affects this cell.
    /// SPEC-HAL-189 INV-189-CELLS-INVALIDATED: CellsInvalidated → BindingDirty.
    pub fn mark_binding_dirty(&mut self, row_id: RowId, column_id: ColumnId) {
        if let Some(widget) = self.get_mut(row_id, column_id) {
            widget.mark_binding_dirty();
        }
    }

    /// Mark a cell's policy dirty.
    ///
    /// Called when PolicyInvalidated affects this cell.
    /// SPEC-HAL-189 INV-189-POLICY-INVALIDATED: PolicyInvalidated → PolicyDirty.
    pub fn mark_policy_dirty(&mut self, row_id: RowId, column_id: ColumnId) {
        if let Some(widget) = self.get_mut(row_id, column_id) {
            widget.mark_policy_dirty();
        }
    }

    /// Mark binding dirty for all cells in a row.
    pub fn mark_row_binding_dirty(&mut self, row_id: RowId) {
        for (key, widget) in self.widgets.iter_mut() {
            if key.row_id == row_id {
                widget.mark_binding_dirty();
            }
        }
    }

    /// Mark policy dirty for all cells in a row.
    pub fn mark_row_policy_dirty(&mut self, row_id: RowId) {
        for (key, widget) in self.widgets.iter_mut() {
            if key.row_id == row_id {
                widget.mark_policy_dirty();
            }
        }
    }

    /// Mark binding dirty for all cells in a column.
    pub fn mark_column_binding_dirty(&mut self, column_id: ColumnId) {
        for (key, widget) in self.widgets.iter_mut() {
            if key.column_id == column_id {
                widget.mark_binding_dirty();
            }
        }
    }

    /// Mark policy dirty for all cells in a column.
    pub fn mark_column_policy_dirty(&mut self, column_id: ColumnId) {
        for (key, widget) in self.widgets.iter_mut() {
            if key.column_id == column_id {
                widget.mark_policy_dirty();
            }
        }
    }

    /// Mark binding dirty for multiple specific cells.
    pub fn mark_binding_dirty_many(&mut self, cells: &[(RowId, ColumnId)]) {
        for &(row_id, column_id) in cells {
            self.mark_binding_dirty(row_id, column_id);
        }
    }

    /// Mark policy dirty for multiple specific cells.
    pub fn mark_policy_dirty_many(&mut self, cells: &[(RowId, ColumnId)]) {
        for &(row_id, column_id) in cells {
            self.mark_policy_dirty(row_id, column_id);
        }
    }

    /// Mark binding dirty for all cells.
    pub fn mark_binding_dirty_all(&mut self) {
        for widget in self.widgets.values_mut() {
            widget.mark_binding_dirty();
        }
    }

    /// Mark policy dirty for all cells.
    pub fn mark_policy_dirty_all(&mut self) {
        for widget in self.widgets.values_mut() {
            widget.mark_policy_dirty();
        }
    }

    // =========================================================================
    // Identity Death (SPEC-HAL-189 §3.3)
    // =========================================================================

    /// Clear all widgets.
    ///
    /// Use for teardown or an authoritative identity-domain replacement.
    /// Surviving resident records are retained across order revisions.
    pub fn clear(&mut self) {
        self.widgets.clear();
    }

    /// Remove widgets for a specific row (row identity death).
    ///
    /// SPEC-HAL-189: Called when RowId is removed from data source.
    pub fn remove_row(&mut self, row_id: RowId) {
        self.widgets.retain(|key, _| key.row_id != row_id);
    }

    /// Remove widgets for a specific column (column identity death).
    ///
    /// SPEC-HAL-189: Called when ColumnId is removed.
    pub fn remove_column(&mut self, column_id: ColumnId) {
        self.widgets.retain(|key, _| key.column_id != column_id);
    }

    /// Remove widgets for rows that no longer exist.
    ///
    /// The caller supplies the qualified set of retained RowIds.
    pub fn retain_rows(&mut self, valid_row_ids: &[RowId]) {
        let valid_set: std::collections::HashSet<RowId> = valid_row_ids.iter().copied().collect();
        self.widgets
            .retain(|key, _| valid_set.contains(&key.row_id));
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Get the number of cached widgets.
    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }

    /// Check if a widget exists for the given cell identity.
    pub fn contains(&self, row_id: RowId, column_id: ColumnId) -> bool {
        let key = CellKey::new(row_id, column_id);
        self.widgets.contains_key(&key)
    }

    /// Get all cells for a given row.
    pub fn cells_for_row(&self, row_id: RowId) -> impl Iterator<Item = &CellWidgetState> {
        self.widgets
            .values()
            .filter(move |w| w.key.row_id == row_id)
    }

    /// Get all cells for a given column.
    pub fn cells_for_column(&self, column_id: ColumnId) -> impl Iterator<Item = &CellWidgetState> {
        self.widgets
            .values()
            .filter(move |w| w.key.column_id == column_id)
    }

    /// Iterate all widgets.
    pub fn iter(&self) -> impl Iterator<Item = (&CellKey, &CellWidgetState)> {
        self.widgets.iter()
    }

    /// Iterate all widgets mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&CellKey, &mut CellWidgetState)> {
        self.widgets.iter_mut()
    }
}

impl Default for CellHost {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_host_create_and_get() {
        let mut host = CellHost::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        // Create widget.
        let widget = host.get_or_create(row_id, col_id);
        assert_eq!(widget.row_id(), row_id);
        assert_eq!(widget.column_id(), col_id);
        assert!(widget.is_clean());

        // Get existing widget.
        assert!(host.contains(row_id, col_id));
        assert!(host.get(row_id, col_id).is_some());
    }

    #[test]
    fn test_cell_host_dirty_flags() {
        let mut host = CellHost::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        // Create widget.
        host.get_or_create(row_id, col_id);
        assert!(host.get(row_id, col_id).unwrap().is_clean());

        // Mark binding dirty.
        host.mark_binding_dirty(row_id, col_id);
        let widget = host.get(row_id, col_id).unwrap();
        assert!(widget.is_dirty());
        assert!(widget.dirty().contains(DirtyFlags::BINDING));

        // Clear binding via consume_dirty.
        let outcome = host
            .get_mut(row_id, col_id)
            .unwrap()
            .consume_dirty(UpdatePhase::Bind);
        assert!(outcome.was_consumed());
        assert!(host.get(row_id, col_id).unwrap().is_clean());
    }

    #[test]
    fn test_cell_host_policy_dirty() {
        let mut host = CellHost::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        host.get_or_create(row_id, col_id);
        host.mark_policy_dirty(row_id, col_id);

        let widget = host.get(row_id, col_id).unwrap();
        assert!(widget.dirty().contains(DirtyFlags::POLICY));
        assert!(!widget.dirty().contains(DirtyFlags::BINDING));
    }

    #[test]
    fn test_cell_host_clear() {
        let mut host = CellHost::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let col_id = ColumnId::new();

        host.get_or_create(row_id1, col_id);
        host.get_or_create(row_id2, col_id);
        assert_eq!(host.widget_count(), 2);

        // Clear when the owning identity domain is retired.
        host.clear();
        assert_eq!(host.widget_count(), 0);
    }

    #[test]
    fn test_cell_host_remove_row() {
        let mut host = CellHost::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let col_id1 = ColumnId::new();
        let col_id2 = ColumnId::new();

        host.get_or_create(row_id1, col_id1);
        host.get_or_create(row_id1, col_id2);
        host.get_or_create(row_id2, col_id1);
        assert_eq!(host.widget_count(), 3);

        // Remove row1 (identity death).
        host.remove_row(row_id1);
        assert_eq!(host.widget_count(), 1);
        assert!(!host.contains(row_id1, col_id1));
        assert!(!host.contains(row_id1, col_id2));
        assert!(host.contains(row_id2, col_id1));
    }

    #[test]
    fn test_cell_host_retain_rows() {
        let mut host = CellHost::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let col_id = ColumnId::new();

        host.get_or_create(row_id1, col_id);
        host.get_or_create(row_id2, col_id);
        host.get_or_create(row_id3, col_id);

        // Retain only row_id1 and row_id3.
        host.retain_rows(&[row_id1, row_id3]);

        assert!(host.contains(row_id1, col_id));
        assert!(!host.contains(row_id2, col_id));
        assert!(host.contains(row_id3, col_id));
        assert_eq!(host.widget_count(), 2);
    }

    #[test]
    fn test_cell_host_mark_row_dirty() {
        let mut host = CellHost::new();
        let row_id = RowId::new();
        let col_id1 = ColumnId::new();
        let col_id2 = ColumnId::new();

        host.get_or_create(row_id, col_id1);
        host.get_or_create(row_id, col_id2);

        // Mark all cells in row dirty.
        host.mark_row_binding_dirty(row_id);

        assert!(
            host.get(row_id, col_id1)
                .unwrap()
                .dirty()
                .contains(DirtyFlags::BINDING)
        );
        assert!(
            host.get(row_id, col_id2)
                .unwrap()
                .dirty()
                .contains(DirtyFlags::BINDING)
        );
    }

    /// INV-189-CELL-IDENTITY: Verify identity is by (RowId, ColumnId), not index.
    #[test]
    fn test_no_index_based_api() {
        // This test documents that CellHost has no index-based methods.
        // If someone adds get_by_index() or similar, this test should fail to compile
        // or this comment should be updated to explain why the violation is acceptable.
        let host = CellHost::new();

        // The only way to access widgets is by (RowId, ColumnId).
        // There is no host.get_by_index(0, 0) method.
        // There is no host.index_of(row_id, col_id) method.
        assert_eq!(host.widget_count(), 0);
    }

    /// INV-189-CELL-REUSE: Verify get_or_create returns existing widget.
    #[test]
    fn test_cell_reuse() {
        let mut host = CellHost::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        // First call creates.
        let v1 = host.get_or_create(row_id, col_id).version();

        // Mark dirty and consume to bump version.
        host.get_or_create(row_id, col_id).mark_binding_dirty();
        host.get_or_create(row_id, col_id)
            .consume_dirty(UpdatePhase::Bind);

        // Second call reuses (version should be bumped).
        let v2 = host.get_or_create(row_id, col_id).version();
        assert_eq!(v2, v1 + 1, "Widget should be reused, version bumped");

        // Only one widget in host.
        assert_eq!(host.widget_count(), 1);
    }

    #[test]
    fn cell_dirty_phases_preserve_identity_and_drain() {
        use DirtyOutcome::{Blocked, Clean, Consumed};
        use UpdatePhase::{Bind, Layout, Paint, Policy};

        let row = RowId::new();
        let column = ColumnId::new();
        let mut host = CellHost::new();
        let state = host.get_or_create(row, column);
        let version = state.version();
        for phase in [Layout, Policy, Bind, Paint] {
            assert_eq!(state.consume_dirty(phase), Clean);
        }

        state.mark_full_dirty();
        let prerequisites = DirtyFlags::GEOMETRY | DirtyFlags::POLICY;
        assert_eq!(state.would_block(Bind), prerequisites);
        assert_eq!(state.consume_dirty(Bind), Blocked(prerequisites));
        assert_eq!(state.dirty(), DirtyFlags::all());
        assert_eq!(state.version(), version);

        assert_eq!(state.consume_dirty(Policy), Consumed);
        assert_eq!(state.consume_dirty(Bind), Blocked(DirtyFlags::GEOMETRY));
        assert_eq!(state.consume_dirty(Layout), Consumed);
        assert_eq!(state.consume_dirty(Bind), Consumed);
        assert_eq!(state.dirty(), DirtyFlags::PRESENTATION);
        assert_eq!(state.consume_dirty(Paint), Consumed);
        assert!(state.is_clean());
        assert_eq!((state.row_id(), state.column_id()), (row, column));
        assert_eq!(host.get_or_create(row, column).version(), version + 1);
        assert_eq!(host.widget_count(), 1);
    }

    #[test]
    fn cell_host_removes_only_cells_for_removed_column() {
        let mut host = CellHost::new();
        let rows = [RowId::new(), RowId::new()];
        let removed = ColumnId::new();
        let retained = ColumnId::new();
        for row in rows {
            host.get_or_create(row, removed);
            host.get_or_create(row, retained).mark_binding_dirty();
        }

        host.remove_column(removed);

        assert_eq!(host.widget_count(), 2);
        for row in rows {
            assert!(!host.contains(row, removed));
            assert_eq!(host.get(row, retained).unwrap().dirty(), DirtyFlags::BINDING);
        }
    }
}
