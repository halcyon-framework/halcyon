// =============================================================================
// SPEC-HAL-259: HierarchicalBlockVirtualization — Hierarchical Virtualization Strategy
// =============================================================================
//
// Implements VirtualizationStrategy for hierarchical row topology.
// Initially behaves as flat virtualization over projected (visible) rows.
//
// Contract:
//   - Depends only on projected row count, row heights, scroll position.
//   - Must not mutate topology.
//   - Must not access column geometry.
//
// INV-HTS-003: Collapsed subtrees occupy exactly one visible block in
//   virtualization. This is achieved by the topology engine producing
//   projected indices that exclude collapsed descendants — virtualization
//   sees only visible rows.
//
// INVARIANT-VZ-1: No linear scan from row 0 for fixed-height.
// INVARIANT-VZ-2: Variable-height mapping must be sublinear after index build.
// =============================================================================

use crate::engines::virtualization::VirtualizationStrategy;
use crate::model::tabular::row_heights::RowHeights;

/// Hierarchical block virtualization strategy.
///
/// SPEC-HAL-259 §5.3: Windowing visible rows with awareness of collapsed subtrees
/// as single virtual blocks. Initially operates as flat virtualization over
/// projected (post-tree-filtering) rows.
///
/// INV-HTS-005: Invalidation follows SPEC-HAL-258. Scroll → Virtualization only.
/// No backward dependency to topology or geometry.
#[derive(Debug)]
pub(crate) struct HierarchicalBlockVirtualization {
    /// Prefix sum of row heights in sorted order (variable-height only).
    /// f64 per SPEC-PM-017 FORBID-PM-017-2 (no f32 accumulation of
    /// content extents).
    prefix_sums: Vec<f64>,
    /// Last seen row count for staleness detection.
    last_row_count: usize,
    /// Last seen RowHeights height version for staleness detection.
    last_height_version: u64,
    /// Last seen topology stamp for staleness detection.
    last_topology_stamp: u64,
    last_fixed_window: Option<FixedWindow>,
}

#[derive(Debug)]
struct FixedWindow {
    viewport: (f64, f64),
    height: f32,
    row_count: usize,
    range: (usize, usize, f64),
}

impl HierarchicalBlockVirtualization {
    pub(crate) fn new() -> Self {
        Self {
            prefix_sums: Vec::new(),
            last_row_count: 0,
            last_height_version: 0,
            last_topology_stamp: 0,
            last_fixed_window: None,
        }
    }

    /// O(1) visible range for uniform row height.
    fn compute_fixed_height(
        &self,
        viewport_top: f64,
        viewport_bottom: f64,
        h: f32,
        row_count: usize,
    ) -> (usize, usize, f64) {
        if h <= 0.0 {
            return (0, 0, 0.0);
        }
        // SPEC-PM-017 INV-PM-017-2: content-space products in f64.
        let h = h as f64;
        let first = (viewport_top / h).floor().max(0.0) as usize;
        let first = first.min(row_count);
        let last = ((viewport_bottom / h).floor().max(0.0) as usize)
            .saturating_add(1)
            .min(row_count);
        let y_offset = first as f64 * h;
        (first, last, y_offset)
    }

    /// O(log N) visible range for variable row heights using prefix-sum binary search.
    fn compute_variable_height(
        &self,
        viewport_top: f64,
        viewport_bottom: f64,
        row_count: usize,
    ) -> (usize, usize, f64) {
        debug_assert_eq!(
            self.prefix_sums.len(),
            row_count + 1,
            "prefix_sums not built"
        );
        let first = self.prefix_sums[1..].partition_point(|&p| p <= viewport_top);
        let first = first.min(row_count);
        let last = self
            .prefix_sums
            .partition_point(|&p| p <= viewport_bottom)
            .min(row_count);
        let y_offset = self.prefix_sums[first];
        (first, last, y_offset)
    }

    /// Rebuild prefix sums if topology or heights have changed.
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
            return;
        }

        self.prefix_sums.clear();
        self.prefix_sums.reserve(row_count + 1);
        self.prefix_sums.push(0.0);
        // SPEC-PM-017 FORBID-PM-017-2: accumulate in f64.
        let mut cumulative: f64 = 0.0;
        for &row_index in sorted_indices {
            cumulative += row_heights.height_at(row_index) as f64;
            self.prefix_sums.push(cumulative);
        }

        self.last_row_count = row_count;
        self.last_height_version = height_version;
        self.last_topology_stamp = topology_stamp;
    }
}

impl VirtualizationStrategy for HierarchicalBlockVirtualization {
    fn compute_visible_range(
        &mut self,
        viewport_top: f64,
        viewport_bottom: f64,
        row_heights: &RowHeights,
        sorted_indices: &[usize],
        topology_stamp: u64,
    ) -> (usize, usize, f64) {
        let row_count = sorted_indices.len();
        if row_count == 0 || viewport_bottom <= viewport_top {
            return (0, 0, 0.0);
        }

        if let Some(h) = row_heights.fixed_height() {
            let viewport = (viewport_top, viewport_bottom);
            // Scroll sync and a subsequent layout can ask about the same
            // window. Only the numeric range is cached here, never its rows.
            if let Some(window) = &self.last_fixed_window
                && window.viewport == viewport
                && window.height == h
                && window.row_count == row_count
            {
                return window.range;
            }
            let range = self.compute_fixed_height(viewport_top, viewport_bottom, h, row_count);
            self.last_fixed_window = Some(FixedWindow {
                viewport,
                height: h,
                row_count,
                range,
            });
            range
        } else {
            self.ensure_prefix_sums(row_heights, sorted_indices, topology_stamp);
            self.compute_variable_height(viewport_top, viewport_bottom, row_count)
        }
    }

    fn visible_range_from(&self, start: usize, end: usize) -> (usize, usize) {
        (start, end)
    }
}
