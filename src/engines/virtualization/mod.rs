// =============================================================================
// SPEC-HAL-257: VirtualizationEngine — Virtualization Interface Contract
// =============================================================================
//
// VirtualizationEngine owns visible row windowing.
// TabularSubstrate remains orchestrator.
// Topology output is consumed as read-only input (future).
// Selection observes virtualization boundaries (future).
//
// No engine may directly mutate another.
//
// =============================================================================
// OWNERSHIP BOUNDARY
// =============================================================================
//
// Responsibilities:
//   - Visible row range computation
//   - Viewport-to-row mapping
//   - Row realization windowing
//
// Non-Responsibilities (FORBIDDEN):
//   - Column geometry
//   - Selection state
//   - Scroll offset ownership
//   - Row ordering decisions (consumes topology output only)
//   - Painting
//   - Hit-testing
//
// INVARIANT-VZ-1: No row realization loop may linearly scan from row 0
//                 to find first_visible when fixed-height is known.
// INVARIANT-VZ-2: Variable-height mapping must be sublinear (≤ O(log N))
//                 after initial index build.
// INVARIANT-VZ-3: Visible realization remains O(visible) only (plus mapping cost).
// INVARIANT-VZ-4: ScrollArea smooth scroll can animate; virtualization must
//                 handle it cheaply per frame.
// INVARIANT-VZ-5: Compute-visible-range must not perform O(N) scans per frame.
// =============================================================================

use crate::model::tabular::row_heights::RowHeights;

/// Virtualization strategy trait.
///
/// SPEC-HAL-257: Defines the interface contract for visible row windowing.
/// Substrate delegates visible range computation here; topology provides
/// ordering, virtualization produces the visible slice.
pub(crate) trait VirtualizationStrategy {
    /// Compute [first_visible, last_visible_exclusive) in sorted-index space.
    ///
    /// Returns (first, last_exclusive, y_offset_at_first).
    /// f64 viewport bounds and y-offset per SPEC-PM-017 INV-PM-017-2.
    fn compute_visible_range(
        &mut self,
        viewport_top: f64,
        viewport_bottom: f64,
        row_heights: &RowHeights,
        sorted_indices: &[usize],
        topology_stamp: u64,
    ) -> (usize, usize, f64);

    /// Visible range query surface for substrate delegation.
    ///
    /// Passthrough: substrate injects the current start/end; virtualization
    /// engine sits in the query path so future extraction can intercept here.
    #[allow(
        dead_code,
        reason = "Retained virtualization-engine range query boundary"
    )]
    fn visible_range_from(&self, start: usize, end: usize) -> (usize, usize);
}

impl VirtualizationStrategy for VirtualizationEngine {
    fn compute_visible_range(
        &mut self,
        viewport_top: f64,
        viewport_bottom: f64,
        row_heights: &RowHeights,
        sorted_indices: &[usize],
        topology_stamp: u64,
    ) -> (usize, usize, f64) {
        // SPEC-PM-017: boundary shim — the legacy VirtualizationEngine
        // stays f32 internally until the rung-5 audit converts it.
        let (first, last, y_offset) = VirtualizationEngine::compute_visible_range(
            self,
            viewport_top as f32,
            viewport_bottom as f32,
            row_heights,
            sorted_indices,
            topology_stamp,
        );
        (first, last, y_offset as f64)
    }

    fn visible_range_from(&self, start: usize, end: usize) -> (usize, usize) {
        VirtualizationEngine::visible_range_from(self, start, end)
    }
}

/// Virtualization engine.
///
/// SPEC-HAL-257: Owns visible row windowing and viewport-to-row mapping.
/// Extraction boundary: substrate delegates row realization windowing here;
/// topology provides ordering, virtualization produces the visible slice.
///
/// Fixed-height rows: O(1) viewport→row mapping via arithmetic.
/// Variable-height rows: O(log N) mapping via prefix-sum binary search.
/// Prefix sums are rebuilt O(N) only when topology or heights change.
#[derive(Debug)]
pub(crate) struct VirtualizationEngine {
    /// Prefix sum of row heights in sorted order (variable-height only).
    /// prefix_sums[i] = cumulative height of sorted rows 0..i.
    /// Length = row_count + 1 when built (prefix_sums[0] = 0).
    prefix_sums: Vec<f32>,
    /// Last seen row count for staleness detection.
    last_row_count: usize,
    /// Last seen RowHeights height version for staleness detection.
    last_height_version: u64,
    /// Last seen topology stamp for staleness detection.
    last_topology_stamp: u64,
}

impl VirtualizationEngine {
    /// Create an empty virtualization engine.
    pub(crate) fn new() -> Self {
        Self {
            prefix_sums: Vec::new(),
            last_row_count: 0,
            last_height_version: 0,
            last_topology_stamp: 0,
        }
    }

    /// Compute [first_visible, last_visible_exclusive) in sorted-index space.
    ///
    /// Returns (first, last_exclusive, y_offset_at_first).
    ///
    /// INV-VZ-1: Fixed-height → O(1) arithmetic, no scan.
    /// INV-VZ-2: Variable-height → O(log N) binary search after O(N) prefix build.
    /// INV-VZ-5: No linear scan from row 0 per frame.
    pub(crate) fn compute_visible_range(
        &mut self,
        viewport_top: f32,
        viewport_bottom: f32,
        row_heights: &RowHeights,
        sorted_indices: &[usize],
        topology_stamp: u64,
    ) -> (usize, usize, f32) {
        let row_count = sorted_indices.len();
        if row_count == 0 || viewport_bottom <= viewport_top {
            return (0, 0, 0.0);
        }

        if let Some(h) = row_heights.fixed_height() {
            // O(1) path: fixed row height
            self.compute_fixed_height(viewport_top, viewport_bottom, h, row_count)
        } else {
            // O(log N) path: variable row heights with prefix-sum index
            self.ensure_prefix_sums(row_heights, sorted_indices, topology_stamp);
            self.compute_variable_height(viewport_top, viewport_bottom, row_count)
        }
    }

    /// O(1) visible range for uniform row height.
    fn compute_fixed_height(
        &self,
        viewport_top: f32,
        viewport_bottom: f32,
        h: f32,
        row_count: usize,
    ) -> (usize, usize, f32) {
        if h <= 0.0 {
            return (0, 0, 0.0);
        }

        // First visible: first row whose bottom > viewport_top.
        // Row i has bottom = (i+1)*h. Solve (i+1)*h > viewport_top → i > viewport_top/h - 1.
        // Equivalent to floor(viewport_top / h).
        let first = (viewport_top / h).floor().max(0.0) as usize;
        let first = first.min(row_count);

        // Last visible (exclusive): first row whose top > viewport_bottom.
        // Row i has top = i*h. Old code processes row if top <= viewport_bottom.
        // Exclusive end = floor(viewport_bottom / h) + 1.
        let last = ((viewport_bottom / h).floor().max(0.0) as usize)
            .saturating_add(1)
            .min(row_count);

        let y_offset = first as f32 * h;
        (first, last, y_offset)
    }

    /// O(log N) visible range for variable row heights using prefix-sum binary search.
    fn compute_variable_height(
        &self,
        viewport_top: f32,
        viewport_bottom: f32,
        row_count: usize,
    ) -> (usize, usize, f32) {
        debug_assert_eq!(
            self.prefix_sums.len(),
            row_count + 1,
            "prefix_sums not built"
        );

        // First visible: smallest i where prefix_sums[i+1] > viewport_top.
        // prefix_sums[1..] contains bottom edges of rows 0..row_count.
        // partition_point finds first element where predicate is false.
        let first = self.prefix_sums[1..].partition_point(|&p| p <= viewport_top);
        let first = first.min(row_count);

        // Last visible (exclusive): first i where prefix_sums[i] > viewport_bottom.
        // prefix_sums[i] = top of row i. Old code processes row if top <= viewport_bottom.
        let last = self
            .prefix_sums
            .partition_point(|&p| p <= viewport_bottom)
            .min(row_count);

        let y_offset = self.prefix_sums[first];
        (first, last, y_offset)
    }

    /// Rebuild prefix sums if topology or heights have changed.
    ///
    /// O(N) rebuild, but only when stale. Subsequent frames with unchanged
    /// topology/heights skip the rebuild entirely.
    fn ensure_prefix_sums(
        &mut self,
        row_heights: &RowHeights,
        sorted_indices: &[usize],
        topology_stamp: u64,
    ) {
        let row_count = sorted_indices.len();
        let height_version = row_heights.height_version();

        if self.prefix_sums.len() == row_count + 1
            && self.last_row_count == row_count
            && self.last_height_version == height_version
            && self.last_topology_stamp == topology_stamp
        {
            return; // Cache valid
        }

        // Rebuild O(N)
        self.prefix_sums.clear();
        self.prefix_sums.reserve(row_count + 1);
        self.prefix_sums.push(0.0);
        let mut cumulative = 0.0;
        for &row_index in sorted_indices {
            cumulative += row_heights.height_at(row_index);
            self.prefix_sums.push(cumulative);
        }

        self.last_row_count = row_count;
        self.last_height_version = height_version;
        self.last_topology_stamp = topology_stamp;
    }

    /// Visible range query surface for substrate delegation.
    ///
    /// Passthrough: substrate injects the current start/end; virtualization
    /// engine sits in the query path so future extraction can intercept here.
    #[allow(
        dead_code,
        reason = "Retained virtualization-engine range query boundary"
    )]
    pub(crate) fn visible_range_from(&self, start: usize, end: usize) -> (usize, usize) {
        (start, end)
    }
}
