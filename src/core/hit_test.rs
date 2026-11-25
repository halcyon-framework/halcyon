// SPEC-HAL-032: Hit-Test Acceleration
//
// Spatial grid acceleration for hit-testing in dense UIs.
// Grid stores candidates in PAINT ORDER (back-to-front).
// Query returns candidates; caller iterates in reverse for front-to-back.

use crate::core::math::{Point, RectF};
use crate::core::widget_tree::NodeId;

/// Cell width for the hit-test grid (logical pixels).
pub const HIT_TEST_CELL_WIDTH: f32 = 64.0;

/// Cell height for the hit-test grid (logical pixels).
pub const HIT_TEST_CELL_HEIGHT: f32 = 64.0;

/// Node count threshold for enabling grid acceleration.
/// Below this, naive DFS is used (grid overhead not worth it).
pub const HIT_TEST_ACCEL_THRESHOLD: usize = 128;

/// A candidate node for hit-testing, with its absolute rect.
#[derive(Clone, Copy, Debug)]
pub struct HitTestCandidate {
    pub node_id: NodeId,
    pub abs_rect: RectF,
}

/// Spatial acceleration grid for hit-testing.
///
/// Partitions the viewport into fixed-size cells. Each cell holds a list
/// of candidates in PAINT ORDER (back-to-front). Query returns candidates
/// for a point; caller iterates in reverse (front-to-back) to find topmost.
pub struct HitTestGrid {
    pub viewport: RectF,
    cell_width: f32,
    cell_height: f32,
    cols: u32,
    rows: u32,
    buckets: Vec<Vec<HitTestCandidate>>,
}

impl HitTestGrid {
    /// Create a new hit-test grid for the given viewport.
    pub fn new(viewport: RectF, cell_width: f32, cell_height: f32) -> Self {
        let cols = ((viewport.width / cell_width).ceil() as u32).max(1);
        let rows = ((viewport.height / cell_height).ceil() as u32).max(1);
        let bucket_count = (cols * rows) as usize;

        Self {
            viewport,
            cell_width,
            cell_height,
            cols,
            rows,
            buckets: vec![Vec::new(); bucket_count],
        }
    }

    /// Rebuild grid from absolute node rects.
    ///
    /// - `viewport`: logical viewport rect (e.g. (0, 0, width, height))
    /// - `nodes`: slice of (NodeId, abs_rect) pairs in PAINT ORDER (back-to-front)
    pub fn rebuild(&mut self, viewport: RectF, nodes: &[(NodeId, RectF)]) {
        self.viewport = viewport;
        self.cols = ((viewport.width / self.cell_width).ceil() as u32).max(1);
        self.rows = ((viewport.height / self.cell_height).ceil() as u32).max(1);
        let bucket_count = (self.cols * self.rows) as usize;

        // Resize buckets if needed
        if self.buckets.len() != bucket_count {
            self.buckets.resize(bucket_count, Vec::new());
        }

        // Clear all buckets
        for bucket in &mut self.buckets {
            bucket.clear();
        }

        // Intersections are half-open, including visible portions with outside origins.
        for &(node_id, abs_rect) in nodes {
            let left = abs_rect.x.max(viewport.x);
            let top = abs_rect.y.max(viewport.y);
            let right = (abs_rect.x + abs_rect.width).min(viewport.x + viewport.width);
            let bottom = (abs_rect.y + abs_rect.height).min(viewport.y + viewport.height);
            if !(left < right && top < bottom) {
                continue;
            }
            let first_col = ((left - viewport.x) / self.cell_width).floor() as u32;
            let first_row = ((top - viewport.y) / self.cell_height).floor() as u32;
            let end_col = (((right - viewport.x) / self.cell_width).ceil() as u32).min(self.cols);
            let end_row = (((bottom - viewport.y) / self.cell_height).ceil() as u32).min(self.rows);
            for row in first_row..end_row {
                for col in first_col..end_col {
                    self.buckets[(row * self.cols + col) as usize]
                        .push(HitTestCandidate { node_id, abs_rect });
                }
            }
        }
    }

    /// Query candidates at a point in absolute logical coordinates.
    ///
    /// Returns a slice of candidates in PAINT ORDER (back-to-front).
    /// Caller should iterate in reverse to find topmost hit.
    pub fn query(&self, point: Point) -> &[HitTestCandidate] {
        // If outside viewport, return empty slice
        if point.x < self.viewport.x
            || point.y < self.viewport.y
            || point.x >= self.viewport.x + self.viewport.width
            || point.y >= self.viewport.y + self.viewport.height
        {
            return &[];
        }

        let cx = ((point.x - self.viewport.x) / self.cell_width).floor();
        let cy = ((point.y - self.viewport.y) / self.cell_height).floor();
        if cx < 0.0 || cy < 0.0 {
            return &[];
        }
        let col = cx as u32;
        let row = cy as u32;
        if col >= self.cols || row >= self.rows {
            return &[];
        }

        let bucket_idx = (row * self.cols + col) as usize;
        if bucket_idx >= self.buckets.len() {
            return &[];
        }

        &self.buckets[bucket_idx]
    }

    /// Get the number of cells in the grid.
    #[allow(dead_code)]
    pub fn cell_count(&self) -> usize {
        (self.cols * self.rows) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_new() {
        let viewport = RectF::new(0.0, 0.0, 256.0, 128.0);
        let grid = HitTestGrid::new(viewport, 64.0, 64.0);

        assert_eq!(grid.cols, 4);
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.buckets.len(), 8);
    }

    #[test]
    fn test_grid_rebuild_and_query() {
        let viewport = RectF::new(0.0, 0.0, 128.0, 128.0);
        let mut grid = HitTestGrid::new(viewport, 64.0, 64.0);

        // Insert a node that spans top-left cell
        let nodes = [(NodeId(1), RectF::new(10.0, 10.0, 40.0, 40.0))];
        grid.rebuild(viewport, &nodes);

        // Query inside the node
        let candidates = grid.query(Point::new(20.0, 20.0));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].node_id, NodeId(1));

        // Query outside the node but inside viewport (different cell)
        let candidates = grid.query(Point::new(100.0, 100.0));
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_grid_paint_order() {
        let viewport = RectF::new(0.0, 0.0, 128.0, 128.0);
        let mut grid = HitTestGrid::new(viewport, 64.0, 64.0);

        // Insert two overlapping nodes in paint order (back-to-front)
        let nodes = [
            (NodeId(1), RectF::new(10.0, 10.0, 40.0, 40.0)), // back
            (NodeId(2), RectF::new(20.0, 20.0, 40.0, 40.0)), // front
        ];
        grid.rebuild(viewport, &nodes);

        // Query should return in paint order; reverse iteration finds topmost
        let candidates = grid.query(Point::new(25.0, 25.0));
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].node_id, NodeId(1)); // back
        assert_eq!(candidates[1].node_id, NodeId(2)); // front (topmost)
    }

    #[test]
    fn test_grid_outside_viewport() {
        let viewport = RectF::new(0.0, 0.0, 128.0, 128.0);
        let mut grid = HitTestGrid::new(viewport, 64.0, 64.0);

        // Insert a node entirely outside viewport
        let nodes = [(NodeId(1), RectF::new(200.0, 200.0, 50.0, 50.0))];
        grid.rebuild(viewport, &nodes);

        // Query inside viewport should find nothing
        let candidates = grid.query(Point::new(64.0, 64.0));
        assert_eq!(candidates.len(), 0);
    }
}
