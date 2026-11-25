# SPEC-HAL-032: Hit-Test Acceleration

**Author:** JC
**Date:** 2025-11-24
**Status:** Draft
**Scope:** Core / Event Routing / WidgetTree
**Purpose:** Make hit-testing scale to dense trading UIs without changing semantics, by introducing a spatial acceleration structure over absolute logical rects.

---

## 1. Problem

Current hit-testing is **tree-walk + rect.contains(point)**:

* Every pointer event may traverse most of the widget tree.
* For heavy layouts (FlowGrid, multi-pane charts, dense panels) this becomes O(N) per event.
* With high-frequency mouse moves or drag streams, this burns CPU for no reason.

We already have:

* A clean coordinate model (SPEC-HAL-024): absolute logical rects per node.
* Stable event routing semantics (SPEC-HAL-009): capture → target → bubble.
* A `hit_testable()` flag on widgets.

We do **not** yet have:

* Any spatial index over abs rects.
* Any way to answer “which widgets are under this point?” faster than scanning the tree.

Goal: keep **exactly** the same observable behavior, but reduce candidate set size for hit-testing from “all widgets” to “only widgets that are spatially plausible”.

---

## 2. Non-Negotiable Constraints

1. **Semantics must not change**

    * Same winning target widget as the current implementation.
    * Same capture / bubble behavior.
    * Same clip-respecting behavior (no events leaking through clipped widgets).

2. **Index uses absolute logical coordinates only**

    * Input: absolute logical `Point` (after physical→logical transform).
    * Index stores absolute logical `RectF`.
    * No DPI or physical units inside the structure.

3. **Widgets remain ignorant**

    * Widgets do not know about the index.
    * No new widget API for hit-testing; they still expose `hit_testable()`, `clip_bounds()`, etc.

4. **Index is optional & replaceable**

    * Must be possible to disable (debug builds, low widget count, tests).
    * Core event routing and WidgetTree must still work with the naive DFS path.

5. **Updates piggyback on existing layout work**

    * No extra tree walks beyond what layout / diffing already do.
    * Index rebuild is explicitly triggered from layout / app layer, not ad-hoc.

---

## 3. Design Overview

Introduce a **HitTestGrid**: a uniform spatial grid over the **current viewport** in absolute logical coordinates.

* The window viewport is partitioned into fixed-size cells (e.g. `cell_w = 64`, `cell_h = 64` logical pixels).
* Each cell contains a small list of `NodeId`s whose abs rect overlaps that cell.
* For large rects, the node appears in multiple cells.
* For hit-testing, we map the pointer’s absolute logical position to a single cell and only check candidates from that bucket (in correct Z order).

This is a **pruning layer**, not a replacement for the final logic:

* The grid only says “these N nodes could contain the point”.
* The existing event_router + clip logic still decides which widget actually wins.

---

## 4. Data Structures

### 4.1 HitTestGrid

New module: `src/core/hit_test.rs`

Core type:

```rust
pub struct HitTestGrid {
    viewport: RectF,          // absolute logical window rect
    cell_width: f32,
    cell_height: f32,
    cols: u32,
    rows: u32,

    // cells[i] is a small list of NodeId in paint / z-order (back → front)
    cells: Vec<Vec<NodeId>>,

    // optional reverse map: which cells a node occupies (for incremental updates)
    node_cells: FxHashMap<NodeId, SmallVec<[u32; 4]>>,
}
```

Grid indexing:

```rust
// (x, y) in absolute logical space
fn cell_index(&self, x: f32, y: f32) -> Option<u32> {
    // maps into [0, cols) x [0, rows), then flattens to 0..cells.len()
}
```

### 4.2 API Surface

Simple, minimal API:

```rust
impl HitTestGrid {
    pub fn new(viewport: RectF, cell_width: f32, cell_height: f32) -> Self;

    /// Clear all cells and node mappings.
    pub fn clear(&mut self);

    /// Insert a node with its absolute logical rect.
    /// Adds NodeId to all cells overlapped by rect.
    pub fn insert(&mut self, node: NodeId, rect: RectF);

    /// Remove node from all cells (if reverse map is enabled).
    pub fn remove(&mut self, node: NodeId);

    /// Rebuild from a flat list of (NodeId, RectF) in z-order.
    pub fn rebuild(&mut self, viewport: RectF, nodes: &[(NodeId, RectF)]);

    /// Query candidate nodes at a point in absolute logical space.
    /// Returns NodeIds in front-to-back or back-to-front order as needed.
    pub fn query(&self, point: Point) -> &[NodeId];
}
```

No widget knowledge, no event knowledge. Pure geometry + NodeId.

---

## 5. Integration Points

### 5.1 Ownership

Add a field to `WidgetTree` (or `App`, but `WidgetTree` is cleaner):

File: `src/core/widget_tree.rs`

```rust
pub struct WidgetTree {
    // existing fields...
    hit_test_grid: Option<HitTestGrid>,
}
```

* When the window size / viewport changes → recreate grid with updated `viewport`.
* When layout completes → rebuild or update grid.

### 5.2 Population

After layout (when all absolute rects are up to date), we already have:

* A traversal that walks nodes and knows their abs rects (for paint / scene building).

Augment that traversal to also feed the grid:

* Only insert nodes where `widget.hit_testable()` is true.
* Only insert nodes that are effectively visible (e.g. not fully clipped if you already know that).
* Insert in the **same order** as paint order, so the cell vectors encode Z correctly.

Minimal path:

1. After layout + abs rect propagation:

    * Collect `(NodeId, abs_rect)` for all hit-testable nodes into a flat `Vec`.
    * Pass that into `HitTestGrid::rebuild(viewport, &nodes)`.

2. If later you want incremental updates:

    * Use `node_cells` to remove/reinsert only changed nodes.
    * That’s optional; SPEC only requires **full rebuild** hook.

### 5.3 Hit-Test Query Usage

File: `src/core/widget_tree.rs` or `src/core/event_router.rs`

Current naive logic (conceptually):

```rust
fn hit_test(&self, point_abs: Point) -> Option<NodeId> {
    // DFS over tree, check abs_rect.contains(point_abs), track top-most winner
}
```

New logic:

```rust
fn hit_test(&self, point_abs: Point) -> Option<NodeId> {
    if let Some(grid) = &self.hit_test_grid {
        let candidates = grid.query(point_abs);

        // Iterate in correct Z order (front-to-back or reversed based on how rebuild filled)
        for node_id in candidates.iter().rev() {
            // existing semantics:
            // - fetch widget
            // - check abs_rect.contains(point_abs)
            // - respect clip chain if needed
            // - check hit_testable()
            // - first winner is the target
        }
        // If no candidate wins, we can optionally fall back to full DFS as safety.
    } else {
        // existing DFS implementation (debug or fallback)
    }
}
```

**Important:**
Clip-awareness (parent clip, scroll area, etc.) stays in the *existing* hit-test logic. The grid only gives a narrowed candidate set, it does not make final decisions.

---

## 6. Parameters & Tuning

### 6.1 Cell Size

SPEC default:

* `cell_width = 64.0`
* `cell_height = 64.0`

Logic:

* Small enough to keep cell lists short in dense UIs.
* Large enough that grid size is modest (e.g. 1920×1080 → ~30×17 ≈ 510 cells).

Make this configurable at construction time; later you can tune per-platform or via a `DebugSettings` panel.

### 6.2 Threshold for Enabling

HitTestGrid is not always worth it.

Add a simple enabling rule:

* If total hit-testable node count < `HIT_TEST_ACCEL_THRESHOLD` (e.g. 128), use the naive DFS path.
* Else, enable the grid.

This can be a constant in `hit_test.rs` or in a central `config` module.

---

## 7. Edge Cases & Safety

1. **Window Resize**

    * On every resize, recreate grid with new viewport + clear content.
    * Next layout pass must call `rebuild()` before any hit-tests use the grid.
    * Until then, either keep `hit_test_grid = None` or mark it “dirty” and fall back to DFS.

2. **Tree Mutations**

    * Adding/removing widgets between layout passes:

        * As long as you rebuild after layout, it’s fine.
    * If you later add incremental layout updates, you can extend the grid to incremental updates. Not required by this SPEC.

3. **Off-screen Widgets**

    * Widgets that are fully outside the viewport can be skipped.
    * The grid implicitly ignores them (they map to cells outside the grid).

4. **Debug Mode**

    * Provide a toggle (feature flag or runtime flag) to **disable** HitTestGrid and use plain DFS.
    * For debugging mismatches: log both results and assert equality.

---

## 8. Implementation Checklist (for Claude Code)

**Files to touch / create:**

* `src/core/hit_test.rs` **(new)**
* `src/core/widget_tree.rs`
* `src/core/app.rs` or layout/paint traversal site (where abs rects are known)
* `src/core/event_router.rs` (if hit-testing is wired there instead of WidgetTree)

**Steps:**

1. **Create HitTestGrid type**

    * New module `src/core/hit_test.rs`
    * Implement `HitTestGrid` as described:

        * `new(viewport, cell_w, cell_h)`
        * `clear()`
        * `insert(node, rect)`
        * `rebuild(viewport, &[(NodeId, RectF)])`
        * `query(point_abs) -> &[NodeId]`

2. **Wire HitTestGrid into WidgetTree**

    * Add `hit_test_grid: Option<HitTestGrid>` field.
    * Add methods:

        * `fn ensure_hit_test_grid(&mut self, viewport: RectF)`
        * `fn rebuild_hit_test_grid(&mut self, viewport: RectF, nodes: &[(NodeId, RectF)])`

3. **Build node list after layout**

    * In the layout/paint preparation path (likely `app.rs` where abs rects are updated):

        * Collect all **hit-testable** nodes with their absolute rects into a `Vec<(NodeId, RectF)>`.
        * Call `widget_tree.rebuild_hit_test_grid(viewport, &nodes)` if node count ≥ threshold.

4. **Replace naive hit_test with grid-backed variant**

    * In `WidgetTree::hit_test` (or equivalent):

        * If `hit_test_grid` present → use `grid.query(point_abs)` candidate list.
        * Iterate candidates in correct Z order and reuse existing containment + clip checks.
        * If grid absent or returns no winner → fall back to existing DFS implementation.

5. **Add debug toggle / threshold**

    * Introduce `HIT_TEST_ACCEL_THRESHOLD` constant somewhere central.
    * If total hit-testable nodes below threshold, skip building the grid and use DFS.

6. **Keep semantics identical**

    * Do not change:

        * How `hit_testable()` is computed.
        * How capture / bubble works.
        * How ScrollArea / clipping affects event routing.
    * The grid only prunes; actual logic stays untouched.

7. **Add basic tests**

    * Unit or integration tests that:

        * Compare DFS hit-test result vs grid-based result for:

            * Simple layout
            * Deep nested layout
            * Overlapping widgets
            * ScrollArea with clipped children
        * Assert identical target NodeId in all cases.
