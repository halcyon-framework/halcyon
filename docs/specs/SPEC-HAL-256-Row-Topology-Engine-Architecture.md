# SPEC-HAL-256: Row Topology Engine Architecture

**Status:** Draft (Binding upon adoption)
**Layer:** Shared Engine Layer
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-158 (Layout Authority & Invalidation Law)
- SPEC-HAL-255 (Column Geometry Engine Extraction)
**Required By:**
- SPEC-HAL-257 (Virtualization Strategy Layer)
- SPEC-HAL-259 (HierarchicalTabularSubstrate Architecture)
- SPEC-HAL-260 (PropertyEditor & Grid Specialization Model)

---

## 1. Purpose

Define a pluggable Row Topology Engine abstraction that governs:
- Row identity ordering
- Parent/child relationships
- Depth
- Visibility
- Structural projection

This engine replaces implicit row logic embedded inside ListView.

Row topology must become an independent engine, orthogonal to:
- Column geometry
- Selection
- Editing
- Virtualization
- Rendering

---

## 2. Motivation

ListView currently conflates:
- Row ordering
- Structural visibility
- Projection rules
- Tree metadata

This prevents:
- Safe hierarchical views
- Block virtualization of collapsed subtrees
- Grid topology support
- PropertyEditor specialization

Topology is not geometry.
Topology is not rendering.
Topology is not selection.

Topology must be isolated.

---

## 3. Definition

Row Topology Engine is responsible for:
- Producing a stable linear projection
- Providing depth metadata
- Providing parent index relationships
- Providing expand/collapse behavior (if applicable)
- Emitting revision numbers

It must not:
- Compute column widths
- Own scroll state
- Own selection
- Own editing
- Perform virtualization
- Emit layout invalidations

---

## 4. Architectural Placement

```
        Data Model
             │
             ▼
   ┌─────────────────────┐
   │ RowTopologyEngine   │
   └─────────┬───────────┘
             │
             ▼
   ┌─────────────────────┐
   │ Virtualization      │
   └─────────┬───────────┘
             │
             ▼
   ┌─────────────────────┐
   │ View / Widget       │
   └─────────────────────┘
```

Topology defines structure.
Virtualization defines windowing.
View consumes projection.

No upward calls allowed.

---

## 5. Core Responsibilities

A RowTopologyEngine must provide:
1. `row_count()`
2. `row_id_at(index)`
3. `row_index_of(id)`
4. `depth_at(index)`
5. `parent_index(index)`
6. `revision()`

Optional capabilities (if hierarchical):
7. `is_expandable(index)`
8. `is_expanded(index)`
9. `apply_expansion(op)`

---

## 6. Topology Types

The architecture must support multiple independent implementations.

### 6.1 FlatRowTopology

- Simple ordered list
- Depth always 0
- No expansion
- No parent relationships

Used by:
- ListView
- Basic tables

---

### 6.2 HierarchicalRowTopology

- Tree structure
- Pre-order linear projection
- Expand/collapse
- Parent index computation
- Depth tracking

Used by:
- TreeView
- PropertyEditor
- Grouped lists

---

### 6.3 GroupedRowTopology

- Flat list with logical grouping
- Group headers as synthetic nodes
- Expand/collapse groups
- No recursive hierarchy

Used by:
- Filtered lists
- Sectioned tables

---

### 6.4 GridRowTopology

- 2D structure projected row-major
- Row index refers to logical row
- Column handled separately
- No tree semantics

Used by:
- Spreadsheet-like Grid

---

## 7. Binding Invariants

**RTE-1**
Topology projection must be deterministic.

**RTE-2**
Projection order must not depend on geometry.

**RTE-3**
Topology must not inspect column engine.

**RTE-4**
Topology must not perform virtualization.

**RTE-5**
Revision must increase monotonically on structural mutation.

**RTE-6**
Parent index must refer only to indices within projection.

**RTE-7**
Depth must be consistent with parent_index.

**RTE-8**
Topology must be pure structural authority.

---

## 8. Expand/Collapse Rules (Hierarchical Only)

Expansion:
- Affects projection only.
- Does not alter underlying model.
- Must prune hidden descendants.
- Must preserve expansion set intersection with reachable nodes.

Collapse:
- Removes descendants from projection.
- Does not mutate model.
- Must revalidate any dependent focus (outside engine).

Topology does not own focus.
Focus engine must consume topology projection.

---

## 9. Revision Semantics

Revision increases when:
- Structure changes
- Expansion state changes
- Root set changes

Revision must not increase when:
- Scroll changes
- Selection changes
- Geometry changes
- Column resize occurs

---

## 10. Invalidation Boundaries

Topology engine emits only:
- revision

Consumers decide what to recompute.

Topology must not:
- Trigger column recompute
- Trigger selection rebuild
- Trigger virtualization rebuild directly

Unidirectional flow:

Data -> Topology -> Virtualization -> View

Never reverse.

---

## 11. Performance Constraints

Projection computation:
- Flat: O(n)
- Hierarchical: O(n) with pruning
- Parent lookup: O(1) via row_index map
- Expand/collapse: O(subtree)

No engine-to-engine recursion allowed.

---

## 12. Separation from Virtualization

Topology produces full logical projection.

Virtualization decides which rows are visible in viewport.

This allows:
- Collapsed subtree virtualization block
- Grid cell windowing
- Group folding

Topology must not know about viewport.

---

## 13. Extraction Plan

1. Extract flat row logic from ListView into FlatRowTopology.
2. Move hierarchical logic into HierarchicalRowTopology.
3. Replace direct ListView projection with injected topology engine.
4. Ensure ListView does not compute parent/depth itself.
5. Introduce revision-driven requery mechanism.

ListView becomes a consumer of RowTopologyEngine.

---

## 14. PropertyEditor Implication

PropertyEditor must use:
- HierarchicalRowTopology
- ColumnGeometryEngine
- EditingEngine
- SelectionEngine

PropertyEditor must not inherit ListView logic directly.

---

## 15. Grid Implication

Grid must use:
- FlatRowTopology
- ColumnGeometryEngine
- CellNavigationEngine
- 2D VirtualizationStrategy

Grid must not share ListView projection logic.

---

## 16. Risks

Without this abstraction:
- Tree and Grid diverge permanently.
- ListView accumulates special cases.
- Performance becomes coupled to feature growth.

With this abstraction:
- Clear structural boundaries.
- Predictable performance.
- Controlled specialization.

---

## 17. Implementation Gate

Before SPEC-HAL-257:
- RowTopologyEngine trait must exist.
- FlatRowTopology must be implemented.
- HierarchicalRowTopology must wrap Tree.
- ListView must consume FlatRowTopology only.

Only then proceed to VirtualizationStrategy separation.

---

End of SPEC-HAL-256
