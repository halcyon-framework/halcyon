# SPEC-HAL-259: HierarchicalTabularSubstrate Architecture

**Status:** Draft (Binding upon adoption)
**Amended by:** SPEC-HAL-337 (Shared Topology Substrate) — ownership (§4.1) and consumer mutation law (§8)
**Layer:** Substrate (Structural Engine)
**Depends On:**
- SPEC-HAL-158 (Layout Authority & Invalidation Law)
- SPEC-HAL-255 (Column Geometry Engine Extraction)
- SPEC-HAL-256 (Row Topology Engine Architecture)
- SPEC-HAL-257 (Virtualization Strategy Layer)
- SPEC-HAL-258 (Engine Invalidation Graph & Data Flow Law)

---

## 1. Purpose

Define HierarchicalTabularSubstrate (HTS) as a structural engine that combines:
- Hierarchical row topology
- Tabular column geometry
- Virtualization strategy
- Deterministic projection
- Tree metadata propagation

This SPEC defines architecture only.
No widget, no rendering, no styling, no UI interaction policy.

HTS is a substrate-level engine.
It is not a widget and must never become one.

---

## 2. Problem Statement

Flat TabularSubstrate assumes:
- Linear row topology
- Flat virtualization window
- No subtree collapsing semantics

TreeSubstrate assumes:
- Hierarchical topology
- No column geometry
- No tabular semantics

Future components require:
- Hierarchical rows
- Multiple columns
- Pinned columns
- Resize / reorder
- Collapsed subtree virtualization
- Grid-like behaviors later

Reusing ListView directly introduces semantic contamination and special cases.

Copying TabularSubstrate introduces uncontrolled duplication.

Therefore:

We introduce a structural substrate composed of extracted engines.

---

## 3. High-Level Decomposition

HierarchicalTabularSubstrate is composed of:
1. ColumnGeometryEngine (shared)
2. RowTopologyEngine (HierarchicalRowTopology implementation)
3. VirtualizationStrategy (HierarchicalBlockVirtualization)
4. SelectionEngine (shared)
5. ScrollDomain (shared)
6. ProjectionCache (local)

No engine is allowed to reach backward into another.

---

## 4. Structural Composition

```
HierarchicalTabularSubstrate
  ├─ ColumnGeometryEngine
  ├─ RowTopologyEngine (Hierarchical)
  ├─ VirtualizationStrategy (HierarchicalBlock)
  ├─ SelectionEngine
  ├─ ScrollState
  └─ ProjectionCache
```

HTS owns orchestration only.

Each engine owns its own invariants.

### 4.1 Ownership (SPEC-HAL-337)

The substrate is owned through the shared handle exclusively:
`SharedHts = Rc<RefCell<HierarchicalTabularSubstrate>>` (SPEC-HAL-337 §2).
Exactly one instance exists per topology (INV-337-O1). No widget, core,
or engine holds a by-value `HierarchicalTabularSubstrate` field
(INV-337-O2). Widgets are projections over an application-level shared
substrate: a widget may mint the substrate for single-view ergonomics
(`HtsOwnership::Internal`) or accept an injected handle
(`HtsOwnership::External`) — the substrate is always behind the handle,
one code shape, no dual paths. Access is through frame-ephemeral guards
only (INV-337-A1).

---

## 5. Engine Responsibilities

### 5.1 ColumnGeometryEngine

Responsible for:
- Column widths
- Resize logic
- Reorder logic
- Pinning
- Visibility
- Total content width

Forbidden:
- No row knowledge
- No topology access
- No virtualization logic

Column geometry must be identical across:
- TabularSubstrate
- HierarchicalTabularSubstrate
- GridSubstrate
- PropertyEditorSubstrate

Single source of truth.

---

### 5.2 RowTopologyEngine (Hierarchical)

Responsible for:
- Parent/child relationships
- Expansion state
- Depth
- Stable projection ordering
- O(1) row_index lookup

Forbidden:
- No geometry
- No scrolling
- No column logic
- No selection logic

Projection must be deterministic and stable.

---

### 5.3 VirtualizationStrategy (HierarchicalBlock)

Responsible for:
- Windowing visible rows
- Treating collapsed subtrees as a single virtual block
- Mapping scroll offset -> row window
- Efficient skip over collapsed branches

Forbidden:
- No mutation of topology
- No column geometry access
- No selection mutation

Virtualization must depend only on:
- Projected row count
- Row heights
- Scroll position

---

### 5.4 SelectionEngine

Responsible for:
- Selection state
- Anchor logic
- Range extension
- Deterministic visible-order traversal

Must operate only on:
- Row IDs
- Visible projection

Forbidden:
- No topology mutation
- No geometry mutation
- No virtualization window rebuilds

---

### 5.5 ScrollState

Responsible for:
- Scroll offsets
- Viewport height
- Scroll bounds

Must not:
- Trigger projection rebuild
- Trigger topology recompute
- Trigger geometry recompute

---

## 6. Projection Model

HTS projection =

```
Topology Projection
         ↓
Virtualization Window
         ↓
Visible Row Slice
         ↓
Tabular Row Mapping (RowId + TreeMeta)
```

Projection is strictly layered.

Each stage receives immutable input from the previous stage.

---

## 7. Invariants

**INV-HTS-001 — Engine Isolation**

No engine may mutate another engine's state.

All communication is unidirectional via orchestrator.

---

**INV-HTS-002 — Deterministic Projection**

Given:
- Topology revision
- Column geometry revision
- Scroll state
- Selection state

Projection must be deterministic.

---

**INV-HTS-003 — Collapsed Subtree Block Invariance**

Collapsed subtree must:
- Occupy exactly one visible block in virtualization.
- Not allocate projection entries for hidden descendants.

---

**INV-HTS-004 — Geometry Independence**

Topology changes must not force column geometry recompute.

Column resize must not trigger topology rebuild.

---

**INV-HTS-005 — Invalidation Graph Compliance**

Invalidation must follow SPEC-HAL-258.

Allowed flow:

- Topology -> Projection
- Geometry -> Layout
- Scroll -> Virtualization
- Selection -> Highlight state

Forbidden:

- Projection -> Topology
- Virtualization -> Geometry
- Selection -> Topology

---

## 8. Public API Surface (Conceptual)

HierarchicalTabularSubstrate exposes:
- `set_topology_adapter(...)`
- `set_columns(...)`
- `set_row_height(...)`
- `expand(node_id)`
- `collapse(node_id)`
- `toggle(node_id)`
- `scroll_to(offset)`
- `visible_rows() -> &[VisibleRow]`
- `column_layout() -> &[ColumnLayout]`
- `revision()`

No rendering API.

No UI events.

No gesture handling.

Consumer mutation flows through the SPEC-HAL-337 §4 mutation law:
structural state mutates only through deferred intents, never during
event routing (INV-337-M1); interaction state keeps its existing
single event-phase write channels (INV-337-M2). The classification of
mutation classes is owned by SPEC-HAL-337 §4 and is not restated here.

---

## 9. Why This Is Not ListView

ListView:
- Flat topology
- Flat virtualization
- Implicit coupling

HTS:
- Pluggable topology
- Pluggable virtualization
- Engine-level separation

ListView becomes:

FlatTabularSubstrate specialization.

HTS becomes:

HierarchicalTabularSubstrate specialization.

Both share extracted engines.

---

## 10. Migration Plan

Phase 1: Extract ColumnGeometryEngine from TabularSubstrate (SPEC-255).

Phase 2: Extract VirtualizationStrategy interface (SPEC-257).

Phase 3: Refactor TabularSubstrate to use FlatRowTopology.

Phase 4: Introduce HierarchicalRowTopology.

Phase 5: Implement HierarchicalTabularSubstrate.

No duplication allowed before extraction.

---

## 11. Performance Model

Target characteristics:
- O(1) row_index lookup
- O(log n) scroll window mapping (if using prefix sums)
- O(k) projection slice for visible rows
- No full tree traversal on scroll
- No geometry recompute on topology mutation

Hierarchical block virtualization is expected to reduce projection cost drastically for deep trees with collapsed branches.

---

## 12. What This Enables

- TreeView without ListView hacks
- PropertyEditor as topology specialization
- Grouped table views
- Expandable sections
- Trading blotters with expandable instrument groups
- Medical record trees with multi-column details
- Future 2D Grid substrate with different topology engine

This architecture supports:
- High-frequency data updates
- Large hierarchical datasets
- Deterministic UI behavior
- Engine-level composability

---

## 13. Explicit Non-Goals

- Rendering
- Widget composition
- Inline editing mechanics
- Cell-level navigation
- Drag-drop semantics

These belong to higher layers.

---

## 14. Architectural Commitment

This SPEC commits the project to:

Engine extraction before duplication.

Topology specialization instead of widget branching.

Strict invalidation graph.

If violated, future complexity will explode combinatorially.

This SPEC establishes the substrate-level foundation for hierarchical tabular systems going forward.

---

End of SPEC-HAL-259
