# SPEC-HAL-257: Virtualization Strategy Layer

**Status:** Draft (Binding upon adoption)
**Layer:** Shared Engine Layer
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-255 (Column Geometry Engine Extraction)
- SPEC-HAL-256 (Row Topology Engine Architecture)
**Required By:**
- SPEC-HAL-259 (HierarchicalTabularSubstrate Architecture)
- SPEC-HAL-260 (PropertyEditor & Grid Specialization Model)

---

## 1. Purpose

Define a dedicated Virtualization Strategy Layer that:
- Converts logical row projection into a viewport window
- Determines which rows are materialized
- Computes visible range
- Supports different structural models

Virtualization must not be embedded inside ListView.

Virtualization must be independent from:
- Row topology
- Column geometry
- Selection
- Editing
- Rendering

---

## 2. Motivation

Virtualization currently assumes:
- Flat, linear row model
- Uniform row behavior
- No structural blocks

This breaks for:
- Hierarchical collapsed subtrees
- Group folding
- Grid 2D windowing
- Large data sets requiring block virtualization

Virtualization is a strategy, not a fixed algorithm.

---

## 3. Architectural Placement

```
Data Model
     │
     ▼
RowTopologyEngine
     │
     ▼
VirtualizationStrategy
     │
     ▼
View / Widget
```

Strictly unidirectional.

Virtualization consumes topology projection.
Virtualization produces viewport window.

---

## 4. Definition

VirtualizationStrategy is responsible for:
- Mapping logical rows to viewport rows
- Determining first visible row index
- Determining last visible row index
- Handling overscan
- Block collapsing behavior (if supported)

VirtualizationStrategy must not:
- Inspect data model directly
- Modify topology
- Modify selection
- Modify geometry

---

## 5. Core Responsibilities

A VirtualizationStrategy must provide:
1. `update_viewport(scroll_offset, viewport_height)`
2. `visible_range() -> (start, end)`
3. `total_extent()`
4. `materialize_range(start, end)`

It must operate on:
- Row count from topology
- Row height model
- Expansion state (if hierarchical)

---

## 6. Virtualization Variants

### 6.1 FlatVirtualization

- Linear window over flat projection
- Uniform row height (or precomputed per-row height)
- O(1) visible range computation

Used by:
- ListView
- Basic tables

---

### 6.2 HierarchicalBlockVirtualization

- Collapsed subtree treated as a single logical block
- Expanded subtree materialized normally
- Scroll jump stability preserved
- Avoids per-row traversal for hidden descendants

Used by:
- TreeView
- PropertyEditor

---

### 6.3 GroupVirtualization

- Group header visible even when collapsed
- Collapsed group treated as block
- Optional sticky group headers

Used by:
- Sectioned lists

---

### 6.4 GridVirtualization (2D)

- Row windowing
- Column windowing
- Cell window intersection
- Independent horizontal and vertical virtualization

Used by:
- Spreadsheet/Grid

---

## 7. Binding Invariants

**VSL-1**
Virtualization must not mutate topology.

**VSL-2**
Virtualization must not inspect underlying data model.

**VSL-3**
Visible range must be contiguous.

**VSL-4**
Overscan must be deterministic.

**VSL-5**
Viewport updates must not alter projection order.

**VSL-6**
Revision must increase only on viewport change.

**VSL-7**
Virtualization must not trigger geometry recompute.

---

## 8. Separation from RowTopology

Topology produces:
- Stable logical projection
- Depth metadata
- Parent indices

Virtualization produces:
- Visible window
- Scroll offset mapping
- Materialized row indices

Topology does not know about viewport.

Virtualization does not know about tree semantics.

HierarchicalBlockVirtualization must consume only:
- `row_count()`
- `depth_at()`
- `is_expanded()`

It must not walk adapter directly.

---

## 9. Scroll Model

Virtualization depends on a ScrollModel providing:
- scroll_offset
- viewport_height
- content_extent

ScrollModel must not call back into topology.

---

## 10. Performance Requirements

Visible range computation:
- Flat: O(1)
- HierarchicalBlock: O(visible nodes)
- Grid: O(visible rows + visible columns)

Virtualization must not:
- Traverse entire projection per frame
- Rebuild row identity map
- Trigger selection rebuild

---

## 11. Invalidation Graph Rules

Allowed flow:

```
Topology revision change
         ↓
Virtualization window recompute
         ↓
View re-render
```

Forbidden:

- Virtualization -> Topology mutation
- Virtualization -> Selection mutation
- Virtualization -> Geometry mutation

No cross-engine loops allowed.

---

## 12. Expand/Collapse Interaction

On expand/collapse:
- Topology revision increases
- Virtualization recomputes window
- Scroll stability must be preserved

Collapsed subtree must:
- Occupy zero logical rows in projection
- Not be traversed during window calculation

Block-based strategy may optionally:
- Cache subtree extent

---

## 13. Grid Implication

GridVirtualization must:
- Handle row-major projection
- Window rows independently of columns
- Combine vertical and horizontal windowing

Grid must not reuse hierarchical strategy.

---

## 14. Extraction Plan

1. Extract flat virtualization logic from ListView.
2. Encapsulate it into FlatVirtualizationStrategy.
3. Replace direct viewport slicing inside ListView.
4. Introduce strategy injection point.
5. Implement HierarchicalBlockVirtualization for TreeView.

ListView becomes:

RowTopologyEngine + VirtualizationStrategy + ColumnGeometryEngine + SelectionEngine

No embedded virtualization logic.

---

## 15. Performance Risk Mitigation

The real performance risk is invalidation loops.

Prevent via:
- Strict engine ownership boundaries
- Single-direction invalidation
- Explicit revision tracking
- No implicit cross-calls

Each engine observes upstream revision only.

---

## 16. Suitability for High-Risk Domains

Trading / Medical UI requires:
- Predictable frame timing
- No structural spikes
- No full rebuild on scroll
- Stable row identity

This architecture enables:
- Deterministic O(1) scroll windowing
- Controlled subtree expansion cost
- No hidden cross-engine recompute

Provided invariants are respected.

---

## 17. Implementation Gate

Before SPEC-HAL-258:
- VirtualizationStrategy trait defined
- FlatVirtualization extracted
- ListView consuming FlatVirtualization
- No direct slicing of projection in widgets

Only then proceed to TreeView construction.

---

End of SPEC-HAL-257
