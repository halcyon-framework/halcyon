# SPEC-HAL-255: Column Geometry Engine Extraction

**Status:** Draft (Binding upon adoption)
**Layer:** Shared Engine Layer
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-141 (Rect Mutation Authority)
- SPEC-HAL-158 (Layout Authority & Invalidation Law)
**Required By:**
- SPEC-HAL-256 (Row Topology Engine Architecture)
- SPEC-HAL-259 (HierarchicalTabularSubstrate Architecture)
- SPEC-HAL-260 (PropertyEditor & Grid Specialization Model)

---

## 1. Purpose

Extract column geometry computation out of ListView and any row topology consumer into a single, reusable, authoritative engine.

This engine becomes the only authority for:
- Column widths
- Column x-positions
- Resize rules
- Pinned column geometry
- Total content width

No view, widget, or topology implementation may compute column layout independently.

---

## 2. Motivation

Without extraction:
- ListView owns column sizing logic.
- Tree/Hierarchical views must duplicate it.
- Grid will require modified logic.
- PropertyEditor will fork and diverge.
- Resizing and pinning rules will drift.

This creates geometric fragmentation.

Column geometry is orthogonal to row topology.

Therefore it must exist as a standalone engine.

---

## 3. Non-Goals

This SPEC does not define:
- Row topology
- Tree indentation
- Cell layout
- Selection behavior
- Virtualization
- Rendering
- Scroll behavior

This SPEC defines only horizontal column layout.

---

## 4. Architectural Position

```
            ┌─────────────────────────┐
            │   RowTopologyEngine     │
            └────────────┬────────────┘
                         │
            ┌────────────▼────────────┐
            │  ColumnGeometryEngine   │
            └────────────┬────────────┘
                         │
            ┌────────────▼────────────┐
            │  View / Widget Layer    │
            └─────────────────────────┘
```

ColumnGeometryEngine:
- Consumes column definitions + container width
- Produces immutable column geometry result
- Emits no side effects

---

## 5. Responsibilities

The engine must compute:
1. Final pixel width for each column
2. X-offset for each column
3. Total content width
4. Pinned left/right partition boundaries
5. Resize constraints resolution
6. Min/max width enforcement

The engine must not:
- Inspect row data
- Inspect topology
- Perform virtualization
- Trigger layout invalidation
- Access selection state

---

## 6. Column Model Input

ColumnDefinition (conceptual):
- id
- initial_width
- min_width
- max_width
- flex (optional)
- pinned (none | left | right)
- resizable (bool)

The engine accepts:
- `&[ColumnDefinition]`
- container_width (f32 or u32)
- resize_operation (optional active drag state)

---

## 7. Output Model

ColumnGeometryResult:
- columns: Vec
  - id
  - x
  - width
  - pinned
- total_width
- left_pinned_width
- right_pinned_width

This result is immutable after computation.

Consumers may cache it.

---

## 8. Resize Semantics

Column resizing must:
- Respect min/max constraints
- Preserve container bounds
- Redistribute flex columns if defined
- Never produce negative width
- Never reorder columns
- Never mutate column definitions

Resize operation produces new geometry result.
It does not mutate input model.

---

## 9. Pinned Columns

Pinned columns are resolved before scrollable region.

Rules:
- Left pinned columns appear first.
- Right pinned columns appear last.
- Scrollable region occupies middle.
- Geometry engine computes fixed partitions.
- Scroll engine may offset only the middle region.

Pinned logic must not leak into row topology.

---

## 10. Invalidation Rules

ColumnGeometryEngine recomputes only when:
- Column definitions change
- Container width changes
- Resize operation completes
- Explicit geometry invalidation issued

It must not recompute when:
- Row projection changes
- Expansion changes
- Selection changes
- Scroll offset changes
- Editing state changes

---

## 11. Performance Constraints

The engine must:
- Run in O(n) time over column count
- Allocate at most O(n)
- Avoid heap churn during resize drag
- Be deterministic

Column count is typically small (< 50).
Performance target: negligible compared to row virtualization.

---

## 12. Binding Invariants

**CGE-1**
Column geometry is derived only from column definitions and container width.

**CGE-2**
No row topology implementation may compute column widths.

**CGE-3**
No view may directly mutate computed column geometry.

**CGE-4**
Pinned partition boundaries are determined exclusively by this engine.

**CGE-5**
Resize must never cause cross-engine invalidation loops.

**CGE-6**
Column ordering is stable and identical to input order.

---

## 13. Extraction Plan

From current ListView implementation:
1. Identify column width calculation logic
2. Identify pinned logic
3. Identify resize resolution logic
4. Remove from ListView
5. Replace with ColumnGeometryEngine invocation
6. Ensure ListView depends only on geometry result

Tree/Hierarchical view will reuse this engine without modification.

Grid and PropertyEditor must use this engine.

---

## 14. Forward Compatibility

This engine must support:
- 2D grid (column virtualization later)
- Hierarchical views
- Grouped views
- Mixed pinned + flex layouts
- PropertyEditor column constraints

Any future column-related behavior must extend this engine — not fork it.

---

## 15. Consequences

Adopting this SPEC:
- Eliminates geometric duplication.
- Enables independent row topology engines.
- Prevents ListView from becoming geometry authority.
- Reduces future drift.

Rejecting this SPEC:
- Guarantees geometry duplication.
- Forces Tree/Grid forks.
- Introduces long-term layout inconsistency.

---

## 16. Implementation Gate

Before SPEC-256 (RowTopologyEngine) begins:
- Column geometry must be fully extracted.
- ListView must consume geometry engine only.
- No column math remains inside ListView.

Only then does topology fork safely.

---

End of SPEC-HAL-255
