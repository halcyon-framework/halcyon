// =============================================================================
// SPEC-HAL-256: RowTopologyEngine — Row Topology Interface Contract
// =============================================================================
//
// RowTopologyEngine owns logical row ordering.
// TabularSubstrate remains orchestrator.
// Virtualization will consume topology output.
// Selection will consume topology identity.
//
// No engine may directly mutate another.
//
// =============================================================================
// OWNERSHIP BOUNDARY
// =============================================================================
//
// Responsibilities:
//   - Row identity ordering
//   - Logical row index resolution
//   - Sorted index storage and versioning
//   - Topology dirty gating
//   - Expand/collapse state (future)
//   - Mapping logical → realized rows (future)
//
// Non-Responsibilities (FORBIDDEN):
//   - Column geometry
//   - Selection state
//   - Scroll offset
//   - Virtualization window
//   - Painting
//   - Hit-testing
//   - Sort type imports (ColumnId, SortState, RowComparator)
//
// INVARIANT-RT-1: RowTopologyEngine must not reference column geometry.
// INVARIANT-RT-2: RowTopologyEngine must not own or mutate selection state.
// INVARIANT-RT-3: RowTopologyEngine must not depend on scroll offset.
// INVARIANT-RT-4: RowTopologyEngine must not perform virtualization windowing.
// INVARIANT-RT-5: RowTopologyEngine must not import substrate or widget types.
// =============================================================================

/// Topology engine trait.
///
/// SPEC-HAL-256/258: Defines the interface contract for row topology.
/// Substrate delegates sorted-order state management here.
/// Sorting logic is provided by the substrate via closure — engine owns
/// WHEN to sort (gating), substrate provides HOW (closure).
///
/// Zero widget imports: engine depends only on std.
pub(crate) trait TopologyEngine {
    /// Get the current sorted indices.
    fn sorted_indices(&self) -> &[usize];
    /// Get the topology version (bumped on every recompute).
    fn topology_version(&self) -> u64;
    /// Get the base row order epoch.
    fn base_row_order_epoch(&self) -> u64;
    /// Get the sorted indices epoch (tagged after each sort).
    fn sorted_indices_epoch(&self) -> u64;
    /// Check if topology is dirty (needs recompute evaluation).
    fn is_topology_dirty(&self) -> bool;

    /// Store sorted indices (validates permutation, bumps topology_version).
    fn set_sorted_indices(&mut self, indices: Vec<usize>, row_count: usize);
    /// Increment base epoch (invalidates sorted indices cache).
    fn increment_base_epoch(&mut self);
    /// Set topology dirty flag.
    #[cfg(test)]
    fn set_topology_dirty(&mut self, dirty: bool);

    /// Ensure sorted order is current.
    ///
    /// Engine owns ALL gating: topology_dirty, epoch mismatch, length mismatch,
    /// sort_dirty. Substrate only provides sort_dirty and the sorter closure.
    ///
    /// - `row_count`: base row count from substrate (body.row_model().len())
    /// - `tree_visible_indices`: if tree mode active, the filtered indices; else None
    /// - `sort_dirty`: from substrate (sort_state.is_dirty() || tree_dirty)
    /// - `sorter`: closure that sorts indices in-place (captures SortState + comparator)
    ///
    /// Returns true if recomputation happened.
    fn ensure_sorted_order(
        &mut self,
        row_count: usize,
        tree_visible_indices: Option<&[usize]>,
        sort_dirty: bool,
        sorter: &dyn Fn(&mut [usize]),
    ) -> bool;

    /// Logical row count (query surface for substrate delegation).
    #[allow(
        dead_code,
        reason = "Retained topology-engine row-count query boundary"
    )]
    fn logical_row_count(&self, substrate_count: usize) -> usize;
    /// Compute the effective row count for sorted index generation.
    #[allow(
        dead_code,
        reason = "Retained topology-engine row-count query boundary"
    )]
    fn compute_sorted_row_count(&self, base_row_count: usize, tree_active: bool) -> usize;
    /// Whether the viewport should be realized this frame.
    fn should_realize(&self, virtualization_dirty: bool) -> bool;
    /// Post-realization hook.
    fn on_realized(&mut self);
    /// Validate that sorted indices form a valid permutation of 0..row_count.
    fn validate_permutation(&self, indices: &[usize], row_count: usize);
}

/// Row topology engine.
///
/// SPEC-HAL-256: Owns logical row ordering, identity resolution, and sorted-order state.
/// Extraction boundary: substrate delegates row ordering questions here;
/// virtualization and selection consume topology output as read-only input.
///
/// SPEC-HAL-258: Owns topology_dirty flag — engine controls all recompute gating.
#[derive(Debug)]
pub(crate) struct RowTopologyEngine {
    /// Sorted row indices (realization order).
    sorted_indices: Vec<usize>,
    /// Epoch of the current sorted indices (tagged after each sort).
    sorted_indices_epoch: u64,
    /// Base row order epoch (incremented when row model changes).
    base_row_order_epoch: u64,
    /// Topology version (bumped on every recompute or set_sorted_indices).
    topology_version: u64,
    /// Whether topology needs recompute evaluation.
    /// SPEC-HAL-258: Owned by engine, not EngineInvalidation.
    topology_dirty: bool,
}

impl RowTopologyEngine {
    /// Create an empty row topology engine.
    pub(crate) fn new() -> Self {
        Self {
            sorted_indices: Vec::new(),
            sorted_indices_epoch: 0,
            base_row_order_epoch: 1,
            topology_version: 0,
            topology_dirty: true, // initial frame must establish topology
        }
    }

}

impl TopologyEngine for RowTopologyEngine {
    fn sorted_indices(&self) -> &[usize] {
        &self.sorted_indices
    }

    fn topology_version(&self) -> u64 {
        self.topology_version
    }

    fn base_row_order_epoch(&self) -> u64 {
        self.base_row_order_epoch
    }

    fn sorted_indices_epoch(&self) -> u64 {
        self.sorted_indices_epoch
    }

    fn is_topology_dirty(&self) -> bool {
        self.topology_dirty
    }

    fn set_sorted_indices(&mut self, indices: Vec<usize>, row_count: usize) {
        self.validate_permutation(&indices, row_count);
        self.sorted_indices = indices;
        self.topology_version += 1;
    }

    fn increment_base_epoch(&mut self) {
        self.base_row_order_epoch += 1;
        self.sorted_indices_epoch = 0;
    }

    #[cfg(test)]
    fn set_topology_dirty(&mut self, dirty: bool) {
        self.topology_dirty = dirty;
    }

    fn ensure_sorted_order(
        &mut self,
        row_count: usize,
        tree_visible_indices: Option<&[usize]>,
        sort_dirty: bool,
        sorter: &dyn Fn(&mut [usize]),
    ) -> bool {
        // Engine owns ALL gating (SPEC-HAL-258).
        let effective_base_count = tree_visible_indices.map_or(row_count, |vis| vis.len());

        let needs_recompute = self.topology_dirty
            || sort_dirty
            || self.sorted_indices_epoch != self.base_row_order_epoch
            || self.sorted_indices.len() != effective_base_count;

        if needs_recompute {
            // Build base indices from tree-filtered set or full range.
            let mut indices: Vec<usize> = if let Some(visible) = tree_visible_indices {
                visible.to_vec()
            } else {
                (0..row_count).collect()
            };

            // Apply sorting via substrate-provided closure.
            sorter(&mut indices);

            // SPEC-HAL-171 INVARIANT-SORT-4: Sorted indices must reference valid row positions.
            debug_assert!(
                indices.iter().all(|&idx| idx < row_count),
                "SPEC-HAL-171 INVARIANT-SORT-4 VIOLATION: ensure_sorted_order produced invalid index (row_count={})",
                row_count
            );

            self.sorted_indices = indices;
            self.sorted_indices_epoch = self.base_row_order_epoch;
            self.topology_version += 1;
        }

        // Always clear dirty when called.
        self.topology_dirty = false;

        needs_recompute
    }

    /// Logical row count (query surface for substrate delegation).
    ///
    /// Passthrough: substrate injects the current count; topology engine
    /// sits in the query path so future extraction can intercept here.
    fn logical_row_count(&self, substrate_count: usize) -> usize {
        substrate_count
    }

    /// Compute the effective row count for sorted index generation.
    ///
    /// Passthrough: substrate injects base_row_count and tree_active flag;
    /// topology engine sits in the decision path so future extraction can
    /// intercept here without changing substrate code.
    fn compute_sorted_row_count(&self, base_row_count: usize, _tree_active: bool) -> usize {
        base_row_count
    }

    /// Whether the viewport should be realized this frame.
    ///
    /// Passthrough: returns the virtualization_dirty flag as-is.
    /// Structural anchor for future ownership of realization gating.
    fn should_realize(&self, virtualization_dirty: bool) -> bool {
        virtualization_dirty
    }

    /// Post-realization hook.
    ///
    /// No-op: exists as structural anchor for future ownership.
    /// Will eventually track realization state within the engine.
    fn on_realized(&mut self) {}

    /// Validate that sorted indices form a valid permutation of 0..row_count.
    ///
    /// INVARIANT-RT-6: May validate row ordering but must not mutate row storage.
    /// INVARIANT-RT-7: Must not read geometry or scroll state.
    /// INVARIANT-RT-8: Validation must be pure (no side effects).
    fn validate_permutation(&self, indices: &[usize], row_count: usize) {
        debug_assert_eq!(
            indices.len(),
            row_count,
            "INV-TV-ORDERING-PERMUTATION: length mismatch"
        );

        #[cfg(debug_assertions)]
        {
            let mut seen = vec![false; row_count];
            for &i in indices {
                assert!(
                    i < row_count,
                    "INV-TV-ORDERING-PERMUTATION: index out of range"
                );
                assert!(!seen[i], "INV-TV-ORDERING-PERMUTATION: duplicate index");
                seen[i] = true;
            }
        }
    }
}
