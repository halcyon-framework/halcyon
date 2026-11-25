// =============================================================================
// SPEC-HAL-187/188/189: RowHost — Standalone row-state compatibility API
// =============================================================================
//
// The public RowHost stores RowId-keyed dirty state. Its caller owns residency
// and identity-domain decisions. Mounted ListView row details use
// ExpandableRowHost, and its body realizes cells through CMS/HTS.
//
// =============================================================================

use std::collections::HashMap;

use crate::widgets::tabular_view::row::RowId;

// Shared dirty tracking types — canonical location: tabular_view::dirty
pub use crate::widgets::tabular_view::dirty::{DirtyFlags, DirtyOutcome, UpdatePhase};

// =============================================================================
// Row Widget State
// =============================================================================

/// Row identity and dirty-phase state for the standalone RowHost API.
#[derive(Debug)]
pub struct RowWidgetState {
    /// The RowId this widget represents.
    row_id: RowId,
    /// Version counter for rebind tracking.
    version: u64,
    /// Dirty flags per SPEC-HAL-189.
    dirty: DirtyFlags,
}

impl RowWidgetState {
    /// Create a new row widget state.
    pub fn new(row_id: RowId) -> Self {
        Self {
            row_id,
            version: 0,
            dirty: DirtyFlags::empty(),
        }
    }

    /// Get the RowId.
    pub fn row_id(&self) -> RowId {
        self.row_id
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
    // Legacy Compatibility (deprecated but preserved for migration)
    // =========================================================================

    /// Mark for rebind (legacy, maps to mark_binding_dirty).
    ///
    /// **DEPRECATED:** Use mark_binding_dirty() for explicit flag control.
    pub fn mark_rebind(&mut self) {
        self.mark_binding_dirty();
    }

    /// Clear rebind flag after rebinding (legacy, maps to clear_binding).
    ///
    /// **DEPRECATED:** Use clear_binding() or clear_all_dirty() instead.
    pub fn rebind_complete(&mut self) {
        self.clear_binding();
    }

    /// Check if rebind is needed (legacy, maps to BindingDirty check).
    ///
    /// **DEPRECATED:** Use is_dirty() or dirty().contains(DirtyFlags::BINDING) instead.
    pub fn needs_rebind(&self) -> bool {
        self.dirty.contains(DirtyFlags::BINDING)
    }
}

// =============================================================================
// Row Host
// =============================================================================

/// RowId-keyed state store for compatibility callers.
///
/// SPEC-HAL-189 INV-189-ROWHOST: Keyed by RowId, not by index.
/// The caller owns identity-domain and residency decisions under HAL-189 §§3.3 and 7.
///
/// **RowHost is an identity cache, not a view cache.**
pub struct RowHost {
    /// Row widgets keyed by RowId.
    ///
    /// INV-ROW-INDEX-EPHEMERAL: We use RowId as key, never index.
    /// There is NO index↔RowId mapping stored here.
    widgets: HashMap<RowId, RowWidgetState>,
}

impl RowHost {
    /// Create a new RowHost.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
        }
    }

    /// Get or create a widget for the given RowId.
    ///
    /// This is the ONLY entry point for widget creation.
    /// If the widget exists, returns it (reuse). Otherwise creates a new one.
    ///
    /// SPEC-HAL-189 INV-189-ROW-REUSE: Existing widgets are reused.
    pub fn get_or_create(&mut self, row_id: RowId) -> &mut RowWidgetState {
        self.widgets
            .entry(row_id)
            .or_insert_with(|| RowWidgetState::new(row_id))
    }

    /// Get a widget by RowId (if it exists).
    pub fn get(&self, row_id: RowId) -> Option<&RowWidgetState> {
        self.widgets.get(&row_id)
    }

    /// Get a mutable widget by RowId (if it exists).
    pub fn get_mut(&mut self, row_id: RowId) -> Option<&mut RowWidgetState> {
        self.widgets.get_mut(&row_id)
    }

    // =========================================================================
    // Dirty Flag Propagation (SPEC-HAL-189 §6)
    // =========================================================================

    /// Mark a row's binding dirty.
    ///
    /// Called when CellsInvalidated affects this row.
    /// SPEC-HAL-189 INV-189-CELLS-INVALIDATED: CellsInvalidated → BindingDirty.
    pub fn mark_binding_dirty(&mut self, row_id: RowId) {
        if let Some(widget) = self.widgets.get_mut(&row_id) {
            widget.mark_binding_dirty();
        }
    }

    /// Mark a row's policy dirty.
    ///
    /// Called when PolicyInvalidated affects this row.
    /// SPEC-HAL-189 INV-189-POLICY-INVALIDATED: PolicyInvalidated → PolicyDirty.
    pub fn mark_policy_dirty(&mut self, row_id: RowId) {
        if let Some(widget) = self.widgets.get_mut(&row_id) {
            widget.mark_policy_dirty();
        }
    }

    /// Mark binding dirty for multiple rows.
    pub fn mark_binding_dirty_many(&mut self, row_ids: &[RowId]) {
        for &row_id in row_ids {
            self.mark_binding_dirty(row_id);
        }
    }

    /// Mark policy dirty for multiple rows.
    pub fn mark_policy_dirty_many(&mut self, row_ids: &[RowId]) {
        for &row_id in row_ids {
            self.mark_policy_dirty(row_id);
        }
    }

    /// Mark binding dirty for all rows.
    pub fn mark_binding_dirty_all(&mut self) {
        for widget in self.widgets.values_mut() {
            widget.mark_binding_dirty();
        }
    }

    /// Mark policy dirty for all rows.
    pub fn mark_policy_dirty_all(&mut self) {
        for widget in self.widgets.values_mut() {
            widget.mark_policy_dirty();
        }
    }

    // =========================================================================
    // Legacy Compatibility (deprecated)
    // =========================================================================

    /// Mark a row for rebind (legacy, maps to mark_binding_dirty).
    ///
    /// **DEPRECATED:** Use mark_binding_dirty() instead.
    pub fn mark_rebind(&mut self, row_id: RowId) {
        self.mark_binding_dirty(row_id);
    }

    /// Mark multiple rows for rebind (legacy, maps to mark_binding_dirty_many).
    ///
    /// **DEPRECATED:** Use mark_binding_dirty_many() instead.
    pub fn mark_rebind_many(&mut self, row_ids: &[RowId]) {
        self.mark_binding_dirty_many(row_ids);
    }

    /// Mark all rows for rebind (legacy, maps to mark_binding_dirty_all).
    ///
    /// **DEPRECATED:** Use mark_binding_dirty_all() instead.
    pub fn mark_rebind_all(&mut self) {
        self.mark_binding_dirty_all();
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

    /// Remove a specific row (row identity death).
    ///
    /// SPEC-HAL-189: Called when RowId is removed from data source.
    pub fn remove(&mut self, row_id: RowId) {
        self.widgets.remove(&row_id);
    }

    /// Remove widgets for rows that no longer exist.
    ///
    /// The caller supplies the qualified set of retained RowIds.
    pub fn retain_only(&mut self, valid_row_ids: &[RowId]) {
        let valid_set: std::collections::HashSet<RowId> = valid_row_ids.iter().copied().collect();
        self.widgets.retain(|row_id, _| valid_set.contains(row_id));
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Get the number of cached widgets.
    pub fn widget_count(&self) -> usize {
        self.widgets.len()
    }

    /// Check if a widget exists for the given RowId.
    pub fn contains(&self, row_id: RowId) -> bool {
        self.widgets.contains_key(&row_id)
    }

    /// Iterate all widgets.
    pub fn iter(&self) -> impl Iterator<Item = (&RowId, &RowWidgetState)> {
        self.widgets.iter()
    }

    /// Iterate all widgets mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&RowId, &mut RowWidgetState)> {
        self.widgets.iter_mut()
    }
}

impl Default for RowHost {
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
    fn test_row_host_create_and_get() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        // Create widget.
        let widget = host.get_or_create(row_id);
        assert_eq!(widget.row_id(), row_id);
        assert!(widget.is_clean());
        assert!(!widget.needs_rebind());

        // Get existing widget.
        assert!(host.contains(row_id));
        assert!(host.get(row_id).is_some());
    }

    #[test]
    fn test_row_host_dirty_flags() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        // Create widget.
        host.get_or_create(row_id);
        assert!(host.get(row_id).unwrap().is_clean());

        // Mark binding dirty.
        host.mark_binding_dirty(row_id);
        let widget = host.get(row_id).unwrap();
        assert!(widget.is_dirty());
        assert!(widget.dirty().contains(DirtyFlags::BINDING));
        assert!(widget.needs_rebind()); // Legacy compatibility

        // Clear binding via consume_dirty.
        let outcome = host
            .get_mut(row_id)
            .unwrap()
            .consume_dirty(UpdatePhase::Bind);
        assert!(outcome.was_consumed());
        assert!(host.get(row_id).unwrap().is_clean());
    }

    #[test]
    fn test_row_host_policy_dirty() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        host.get_or_create(row_id);
        host.mark_policy_dirty(row_id);

        let widget = host.get(row_id).unwrap();
        assert!(widget.dirty().contains(DirtyFlags::POLICY));
        assert!(!widget.dirty().contains(DirtyFlags::BINDING));
    }

    #[test]
    fn test_row_host_rebind_marking_legacy() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        // Create widget.
        host.get_or_create(row_id);

        // Mark for rebind (legacy API).
        host.mark_rebind(row_id);
        assert!(host.get(row_id).unwrap().needs_rebind());
        assert!(
            host.get(row_id)
                .unwrap()
                .dirty()
                .contains(DirtyFlags::BINDING)
        );

        // Complete rebind (legacy API).
        host.get_mut(row_id).unwrap().rebind_complete();
        assert!(!host.get(row_id).unwrap().needs_rebind());
    }

    #[test]
    fn test_row_host_clear() {
        let mut host = RowHost::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();

        host.get_or_create(row_id1);
        host.get_or_create(row_id2);
        assert_eq!(host.widget_count(), 2);

        // Clear when the owning identity domain is retired.
        host.clear();
        assert_eq!(host.widget_count(), 0);
    }

    #[test]
    fn test_row_host_retain_only() {
        let mut host = RowHost::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();

        host.get_or_create(row_id1);
        host.get_or_create(row_id2);
        host.get_or_create(row_id3);

        // Retain only row_id1 and row_id3.
        host.retain_only(&[row_id1, row_id3]);

        assert!(host.contains(row_id1));
        assert!(!host.contains(row_id2));
        assert!(host.contains(row_id3));
        assert_eq!(host.widget_count(), 2);
    }

    /// INV-ROW-INDEX-EPHEMERAL: Verify no index-based API exists.
    #[test]
    fn test_no_index_based_api() {
        // This test documents that RowHost has no index-based methods.
        // If someone adds get_by_index() or similar, this test should fail to compile
        // or this comment should be updated to explain why the violation is acceptable.
        let host = RowHost::new();

        // The only way to access widgets is by RowId.
        // There is no host.get_by_index(0) method.
        // There is no host.index_of(row_id) method.
        assert_eq!(host.widget_count(), 0);
    }

    /// INV-189-ROW-REUSE: Verify get_or_create returns existing widget.
    #[test]
    fn test_row_reuse() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        // First call creates.
        let v1 = host.get_or_create(row_id).version();

        // Mark dirty and consume to bump version.
        host.get_or_create(row_id).mark_binding_dirty();
        host.get_or_create(row_id).consume_dirty(UpdatePhase::Bind);

        // Second call reuses (version should be bumped).
        let v2 = host.get_or_create(row_id).version();
        assert_eq!(v2, v1 + 1, "Widget should be reused, version bumped");

        // Only one widget in host.
        assert_eq!(host.widget_count(), 1);
    }

    /// INV-189-CLEAN-NOOP: Verify clean widgets are identifiable.
    #[test]
    fn test_clean_noop() {
        let mut host = RowHost::new();
        let row_id = RowId::new();

        host.get_or_create(row_id);
        assert!(host.get(row_id).unwrap().is_clean());

        // Mark dirty, then consume.
        host.mark_binding_dirty(row_id);
        assert!(!host.get(row_id).unwrap().is_clean());

        host.get_mut(row_id)
            .unwrap()
            .consume_dirty(UpdatePhase::Bind);
        assert!(host.get(row_id).unwrap().is_clean());
    }

    #[test]
    fn row_dirty_phases_preserve_identity_and_drain() {
        use DirtyOutcome::{Blocked, Clean, Consumed};
        use UpdatePhase::{Bind, Layout, Paint, Policy};

        let row = RowId::new();
        let mut host = RowHost::new();
        let state = host.get_or_create(row);
        let original_version = state.version();

        for phase in [Layout, Policy, Bind, Paint] {
            assert_eq!(state.consume_dirty(phase), Clean);
        }
        assert_eq!(state.version(), original_version);

        let prerequisites = DirtyFlags::GEOMETRY | DirtyFlags::POLICY;
        for (first, second, remaining) in [
            (Layout, Policy, DirtyFlags::POLICY),
            (Policy, Layout, DirtyFlags::GEOMETRY),
        ] {
            let version = state.version();
            state.mark_geometry_dirty();
            state.mark_policy_dirty();
            state.mark_binding_dirty();
            state.mark_presentation_dirty();
            assert_eq!(state.row_id(), row);
            assert_eq!(state.version(), version);
            assert_eq!(state.dirty(), DirtyFlags::all());

            assert_eq!(state.would_block(Bind), prerequisites);
            assert!(state.would_block(first).is_empty());
            assert_eq!(state.consume_dirty(Bind), Blocked(prerequisites));
            assert_eq!(state.dirty(), DirtyFlags::all());
            assert_eq!(state.version(), version);

            assert_eq!(state.consume_dirty(Paint), Consumed);
            assert_eq!(state.dirty(), prerequisites | DirtyFlags::BINDING);
            assert_eq!(state.consume_dirty(first), Consumed);
            assert_eq!(state.consume_dirty(Bind), Blocked(remaining));
            assert_eq!(state.dirty(), remaining | DirtyFlags::BINDING);
            assert_eq!(state.consume_dirty(second), Consumed);
            assert_eq!(state.dirty(), DirtyFlags::BINDING);
            assert_eq!(state.consume_dirty(Bind), Consumed);
            assert!(state.is_clean());
            assert_eq!(state.version(), version + 1);
        }

        assert_eq!(host.get_or_create(row).version(), original_version + 2);
        assert_eq!(host.widget_count(), 1);
    }
}
