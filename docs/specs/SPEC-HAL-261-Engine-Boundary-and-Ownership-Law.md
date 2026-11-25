# SPEC-HAL-261: TabularSubstrate Orchestration Boundary & Engine Contract Law

**Status:** Binding
**Layer:** Substrate / Engine Boundary Law
**Depends On:**
- SPEC-HAL-255 (Column Geometry Engine Extraction)
- SPEC-HAL-256 (Row Topology Engine Architecture)
- SPEC-HAL-257 (Virtualization Strategy Layer)
- SPEC-HAL-258 (Engine Invalidation Graph & Data Flow Law)

**Amended by:** SPEC-HAL-353 for SPEC-HAL-188-backed consumers: provider order revisions/windows replace local
comparator, complete `sorted_indices`, and full-row topology as the ordering/virtualization input

---

## 1. Purpose

This SPEC freezes the architectural boundary between TabularSubstrate (orchestrator) and its extracted engines:
- ColumnGeometryEngine (SPEC-HAL-255)
- RowTopologyEngine (SPEC-HAL-256)
- VirtualizationEngine (SPEC-HAL-257)
- Engine invalidation graph (SPEC-HAL-258)

It defines:
- Ownership of state
- Gating authority
- Call ordering
- Dirty propagation rules
- Forbidden cross-layer behavior

This SPEC is not about behavior change.
It is about permanent structural boundaries.

For ListView and every SPEC-HAL-188-backed consumer, SPEC-HAL-353 supersedes §§2.1, 2.2, 2.4, 4, 5, and 6 wherever
they require `SortState`, `Comparator`, complete `sorted_indices`, or a local sort closure as the order path. Those
consumers use provider-confirmed `OrderRevision`, bounded RowId windows, and identity-to-ordinal lookup. Geometry
ownership, phase isolation, and the prohibition on cross-engine mutation remain binding.

---

## 2. Architectural Roles

### 2.1 TabularSubstrate (Orchestrator)

**Owns:**
- RowModel
- ColumnModel
- TreeProjection
- SortState
- Comparator
- Selection state
- Edit state
- EngineInvalidation (except topology_dirty)
- Token lifecycle
- EvaluationPhaseGuard

**Responsible for:**
- Sequencing: Topology → Geometry → Virtualization → Realization
- Providing sort closure to topology engine
- Passing tree projection output into topology engine
- Marking virtualization_dirty when topology recomputes
- Enforcing token discipline
- Enforcing evaluation guards

**Does NOT own:**
- Sorted indices storage
- Topology version
- Base row order epoch
- Topology dirty flag
- Visible window computation logic

---

### 2.2 RowTopologyEngine

**Owns:**
- sorted_indices
- sorted_indices_epoch
- base_row_order_epoch
- topology_version
- topology_dirty

**Authority:**
- Decides WHEN sorted order must be recomputed
- Clears topology_dirty internally
- Bumps topology_version on recompute
- Tags sorted_indices_epoch

**Receives from substrate:**
- row_count
- tree_visible_indices
- sort_dirty
- sorter closure

**Does NOT:**
- Import widget types
- Import ColumnId
- Import SortState
- Mutate SortState
- Mutate EngineInvalidation
- Access scroll offset
- Access geometry
- Perform virtualization

**INVARIANT-261-RT-1:**
All topology gating logic lives inside ensure_sorted_order.

**INVARIANT-261-RT-2:**
Substrate must not pre-gate topology with an outer topology_dirty check.

**INVARIANT-261-RT-3:**
topology_dirty is engine-owned. EngineInvalidation must not contain it.

---

### 2.3 ColumnGeometryEngine

Concrete type (no trait object).

**Owns:**
- Column width resolution
- Geometry epoch stamping
- Staleness detection

**Does NOT:**
- Access topology state
- Access sorted indices
- Access scroll offset
- Access virtualization

**INVARIANT-261-GE-1:**
Geometry resolution must not depend on virtualization state.

---

### 2.4 VirtualizationEngine

**Owns:**
- Visible range computation
- Prefix-sum windowing
- Scroll-to-row mapping

**Receives:**
- RowModel
- sorted_indices (read-only)
- topology_version
- viewport bounds

**Does NOT:**
- Access SortState
- Access TreeProjection
- Access ColumnModel
- Access selection
- Mutate sorted_indices

**INVARIANT-261-VZ-1:**
Virtualization must treat sorted_indices as immutable input.

**INVARIANT-261-VZ-2:**
Virtualization must not perform topology recomputation.

---

## 3. EngineInvalidation Graph Law

EngineInvalidation contains:
- geometry_dirty
- virtualization_dirty
- selection_dirty
- last_geometry_width

It does NOT contain:
- topology_dirty

**INVARIANT-261-INV-1:**
Topology dirtiness is internal to RowTopologyEngine.

**INVARIANT-261-INV-2:**
If topology recomputes, substrate must set virtualization_dirty = true.

**INVARIANT-261-INV-3:**
Geometry invalidation must not automatically imply topology invalidation.

---

## 4. Evaluation Sequence (Binding Order)

Within a frame:

1. **Topology resolution**
   `ensure_sorted_order(...)` → may set virtualization_dirty
2. **Geometry resolution**
   `resolve_geometry(...)` → may set geometry_dirty = false
3. **Virtualization**
   `compute_visible_range(...)`
4. **Realization**
   `realize_with_range(...)`

**INVARIANT-261-SEQ-1:**
Topology must be resolved before virtualization.

**INVARIANT-261-SEQ-2:**
Geometry resolution must not depend on virtualization output.

**INVARIANT-261-SEQ-3:**
Realization must consume sorted_indices provided by topology engine.

---

## 5. Closure-Based Sorting Contract

This section is a legacy/materialized TabularSubstrate contract. It is not a lawful sorting path for a
SPEC-HAL-188-backed consumer under SPEC-HAL-353.

Sorting logic is provided by substrate as:

```rust
&dyn Fn(&mut [usize])
```

**Engine controls:**
- When to call it
- Whether to recompute
- Storage of results

**Substrate controls:**
- Comparator implementation
- Column selection
- Direction
- Fallback behavior

**INVARIANT-261-SORT-1:**
Topology engine must not know about ColumnId.

**INVARIANT-261-SORT-2:**
Topology engine must not own SortState.

**INVARIANT-261-SORT-3:**
Sorter closure must be side-effect free outside index mutation.

---

## 6. Versioning & Epoch Law

**base_row_order_epoch:**
- Incremented only by substrate via `increment_base_epoch()`.

**sorted_indices_epoch:**
- Updated only by topology engine during recompute.

**topology_version:**
- Incremented only when sorted_indices storage changes.

**INVARIANT-261-EPOCH-1:**
sorted_indices_epoch must equal base_row_order_epoch after recompute.

**INVARIANT-261-EPOCH-2:**
sorted_indices_epoch must be 0 after base epoch increment.

**INVARIANT-261-EPOCH-3:**
topology_version must bump exactly once per recompute.

---

## 7. Forbidden Behaviors

The following are architectural violations:

1. Reintroducing sorted_indices field into substrate.
2. Reintroducing topology_dirty into EngineInvalidation.
3. Adding widget imports inside engines::row_topology.
4. Making ColumnGeometryEngine a trait object.
5. Allowing VirtualizationEngine to mutate topology.
6. Performing outer topology_dirty gating in substrate.
7. Allowing topology engine to mutate SortState.
8. Allowing virtualization to depend on geometry.

Any of the above breaks SPEC-HAL-261.

---

## 8. Extraction Status

After this SPEC:
- Column geometry extraction: structurally complete.
- Row topology extraction: structurally complete.
- Virtualization extraction: structurally complete.
- Invalidation ownership separation: complete.

Remaining future extractions (NOT in this SPEC):
- Sort type relocation (blocked by ColumnId location)
- Tree projection extraction
- Selection extraction

Those require separate SPECs.

---

## 9. Architectural Outcome

After adoption:

**TabularSubstrate becomes:**
- Pure orchestrator
- Stateless regarding row order storage
- Boundary enforcer

**Engines become:**
- Deterministic subsystems
- State-owning
- Gating-authoritative within their domain

This stabilizes the engine boundary and prevents regression into substrate-centralized state.

---

## 10. Compliance Record

**Audit date:** 2026-02-15
**Audit scope:** Full structural compliance against all invariants.

**Files audited:**
- `src/widgets/tabular_view/mod.rs` (TabularSubstrate, EngineInvalidation)
- `src/engines/row_topology/mod.rs` (RowTopologyEngine, TopologyEngine trait)
- `src/engines/column_geometry/mod.rs` (ColumnGeometryEngine, GeometryEngine trait)
- `src/engines/virtualization/mod.rs` (VirtualizationEngine, VirtualizationStrategy trait)

**Results:**

| Checklist Item | Result |
|---|---|
| 1. Topology ownership (5 fields in engine, 0 in substrate/invalidation) | PASS |
| 2. Gating authority (no substrate pre-gates) | PASS (after fix) |
| 3. Closure contract (zero widget imports in engine) | PASS |
| 4. Virtualization isolation (no cross-layer access) | PASS |
| 5. Invalidation graph law (topology_dirty not in EngineInvalidation) | PASS |
| 6. Evaluation sequence (Topology → Geometry → Virtualization → Realization) | PASS |
| 7. Forbidden patterns (none found) | PASS |

**Violation found and resolved:**
- `effective_row_order()` contained substrate-side topology pre-gate (epoch comparison,
  sort_dirty check, tree_dirty check) that duplicated engine gating and omitted
  `topology_dirty` and length-mismatch checks. Violated INVARIANT-261-RT-1.
- Fix: removed outer `needs_recompute` guard; `ensure_sorted_order()` called
  unconditionally, matching all other call sites. 35/35 tests pass.
