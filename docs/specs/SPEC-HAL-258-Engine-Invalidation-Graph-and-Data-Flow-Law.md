# SPEC-HAL-258: Engine Invalidation Graph & Data Flow Law

**Status:** Draft (Binding upon adoption)
**Layer:** Architectural Law
**Depends On:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-255 (Column Geometry Engine Extraction)
- SPEC-HAL-256 (Row Topology Engine Architecture)
- SPEC-HAL-257 (Virtualization Strategy Layer)

---

## 1. Purpose

Define a strict, enforceable invalidation graph between engines:
- ColumnGeometryEngine
- RowTopologyEngine
- VirtualizationStrategy
- SelectionEngine
- EditingEngine
- ScrollModel
- View / Widget layer

The goal is to prevent:
- Cross-engine mutation
- Circular invalidation
- Implicit recomputation cascades
- Performance collapse under structural changes

This SPEC defines a law, not a guideline.

---

## 2. Core Principle

All engine relationships must be:
- Unidirectional
- Revision-driven
- Observable-only
- Non-recursive

No engine may call back into an upstream engine.

---

## 3. The Invalidation Graph

Allowed Flow:

```
Data Model
     ↓
RowTopologyEngine
     ↓
VirtualizationStrategy
     ↓
SelectionEngine
     ↓
View / Widget
```

Parallel branch:

```
ColumnGeometryEngine
         ↓
View / Widget
```

Scroll branch:

```
ScrollModel
     ↓
VirtualizationStrategy
```

Editing branch:

```
EditingEngine
      ↓
Data Model
```

No other edges are permitted.

---

## 4. Engine Ownership Rules

**EIG-1**
RowTopologyEngine owns projection order.

**EIG-2**
VirtualizationStrategy owns viewport window.

**EIG-3**
ColumnGeometryEngine owns column widths and layout.

**EIG-4**
SelectionEngine owns selection state only.

**EIG-5**
EditingEngine owns transient edit state only.

**EIG-6**
View layer owns rendering only.

No engine may mutate another engine's internal state.

---

## 5. Revision Discipline

Every engine must expose:

`revision() -> u64`

Rules:

**EIG-7**
An engine may only observe upstream revision values.

**EIG-8**
Downstream engines recompute only if upstream revision changed.

**EIG-9**
Recomputation must not mutate upstream revision.

**EIG-10**
Revisions must be monotonic.

---

## 6. Forbidden Patterns (Death Loops)

The following are strictly forbidden:

### 6.1 Topology <-> Geometry Coupling

```
Topology change
  -> Geometry recompute
  -> Row remeasure
  -> Topology rebuild
```

Illegal.

Topology and geometry must be independent.

---

### 6.2 Virtualization -> Topology Mutation

```
Virtualization sees window change
  -> Expands subtree
  -> Topology revision bump
  -> Virtualization recompute
```

Illegal.

Viewport must not alter structure.

---

### 6.3 Selection -> Projection Mutation

```
Selection change
  -> Row insertion/removal
  -> Topology revision
```

Illegal.

Selection never changes structure.

---

### 6.4 Geometry -> Virtualization Back-Edge

```
Column resize
  -> Row height change
  -> Virtualization window rebuild
  -> Geometry recompute
```

Illegal unless row height model explicitly depends on column geometry and the dependency is one-directional.

---

## 7. Scroll Interaction Law

ScrollModel may only:
- Provide scroll_offset
- Provide viewport_height

It may not:
- Inspect topology
- Modify projection
- Adjust selection

VirtualizationStrategy consumes ScrollModel.
ScrollModel does not consume VirtualizationStrategy.

---

## 8. Expand/Collapse Law

On expand/collapse:
1. RowTopologyEngine increments revision.
2. VirtualizationStrategy observes revision.
3. VirtualizationStrategy recomputes visible window.
4. View re-renders.

No other side effects allowed.

---

## 9. Selection Stability Law

SelectionEngine must:
- Store identity by stable RowId / NodeId.
- Never store projection indices as authority.
- Re-map indices on topology revision change.

Selection must not:
- Trigger topology rebuild.
- Trigger geometry recompute.

---

## 10. Editing Isolation Law

EditingEngine may:
- Mutate Data Model.
- Trigger topology revision.

EditingEngine must not:
- Touch virtualization window.
- Touch geometry state.
- Touch selection directly.

Selection updates must occur only via revision observation.

---

## 11. Engine Boundaries Table

| Engine | May Observe | May Mutate | Forbidden |
|---|---|---|---|
| RowTopology | Data Model | Own projection | Geometry, Virtualization |
| Virtualization | Topology, Scroll | Own window | Topology |
| ColumnGeometry | Columns config | Own layout | Topology |
| Selection | Topology revision | Own selection | Projection |
| Editing | Data Model | Data Model | View |
| View | All | None | All engines |

---

## 12. Performance Law

The only valid recompute chain is:

```
Model mutation
  -> Topology revision++
  -> Virtualization recompute
  -> View update
```

Everything else is a violation.

No engine may cause recomputation of an upstream engine.

---

## 13. Deterministic Frame Law

During a single frame:
- Each engine may recompute at most once.
- No nested recompute allowed.
- No cascading invalidation beyond one direction.

---

## 14. Detection Strategy

To enforce this SPEC:
- Each engine logs its revision.
- Debug builds assert no upstream revision change during downstream recompute.
- Optional: engine recompute counters per frame.

Any loop is a bug.

---

## 15. Implications for ListView / TreeView / Grid

ListView must become:

FlatRowTopology + FlatVirtualization + ColumnGeometryEngine + SelectionEngine

TreeView must become:

HierarchicalRowTopology + HierarchicalVirtualization + ColumnGeometryEngine + SelectionEngine

Grid must become:

FlatRowTopology + GridVirtualization + ColumnGeometryEngine + CellSelectionEngine

No engine reuse through implicit coupling.

Reuse only via extracted engines.

---

## 16. Why This Is Mandatory

Without this law:
- Expansion will trigger geometry rebuild.
- Geometry rebuild will trigger virtualization.
- Virtualization will alter projection.
- Projection will alter selection.
- Selection will alter projection.

That loop kills performance in large trees and grids.

This SPEC prevents that structurally.

---

## 17. Adoption Requirement

Before implementing:
- HierarchicalTabularSubstrate
- GridView
- PropertyEditor

All engines must be separated according to SPEC-255, 256, 257.

No mixed-engine substrate allowed.

---

## 18. Non-Negotiable Rule

If any engine:
- Reads and mutates another engine
- Calls upstream logic
- Creates recompute loops

The architecture is considered broken and must be refactored before proceeding.

---

End of SPEC-HAL-258
