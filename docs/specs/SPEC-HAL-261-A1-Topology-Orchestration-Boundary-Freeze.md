# SPEC-HAL-261-A1: Extraction Freeze Amendment — Topology Orchestration Boundary

**Status:** Binding
**Layer:** TabularSubstrate / RowTopologyEngine Boundary Law
**Amends:** SPEC-HAL-255, SPEC-HAL-256, SPEC-HAL-257, SPEC-HAL-258, SPEC-HAL-261
**Amended by:** SPEC-HAL-353 for SPEC-HAL-188-backed consumers: provider-authoritative order windows replace local
comparator/sorted-index topology fulfillment

---

## 1. Purpose

This amendment freezes the topology extraction boundary after successful structural compliance.

The objective is to:
- Lock engine ownership of topology state.
- Centralize orchestration at a single substrate entry point.
- Prevent regression into duplicated gating logic.
- Stabilize the closure-based sorting boundary until sort types are extracted from widgets.

This is a structural freeze. No behavioral changes.

For ListView and every SPEC-HAL-188-backed consumer, SPEC-HAL-353 supersedes §§2–5 wherever they require local
`sorted_indices`, comparator state, an index-sorting closure, or complete-row topology as the sorting fulfillment path.
Those consumers receive already ordered, revision-scoped RowId windows from the data layer. This freeze remains binding
for legacy/materialized TabularSubstrate paths outside SPEC-HAL-353 and does not authorize retaining the incompatible
path in data-backed ListView.

---

## 2. Centralized Orchestration Law

**INVARIANT-261-A1-1 — Single Orchestration Point**

All topology orchestration inside TabularSubstrate MUST occur exclusively through:

```rust
fn ensure_topology(&mut self)
```

This method is the only location allowed to:
- Build the sorting closure.
- Call `RowTopologyEngine::ensure_sorted_order`.
- Clear sort_state dirty flag.
- Set `engine_invalidation.virtualization_dirty = true`.

No other method may:
- Construct a sorting closure.
- Call `ensure_sorted_order` directly.
- Clear sort dirty state.
- Mutate `virtualization_dirty` based on topology recompute.

Violation ends discussion.

---

## 3. Gating Authority Law

**INVARIANT-261-A1-2 — Engine Owns All Topology Gating**

All recompute gating logic MUST live inside:

```rust
RowTopologyEngine::ensure_sorted_order
```

Substrate MUST NOT:
- Re-implement epoch comparisons.
- Check `sorted_indices_epoch`.
- Compare against `base_row_order_epoch`.
- Check `sorted_indices.len()` against row count.
- Implement any `needs_recompute` pre-gate.

Substrate MAY:
- Provide `sort_dirty`.
- Provide tree-visible indices.
- Trigger `ensure_topology()`.

Nothing else.

---

## 4. State Ownership Freeze

**INVARIANT-261-A1-3 — Topology State Exclusivity**

The following fields MUST exist only inside RowTopologyEngine:
- `sorted_indices`
- `sorted_indices_epoch`
- `base_row_order_epoch`
- `topology_version`
- `topology_dirty`

They MUST NOT exist in:
- TabularSubstrate
- EngineInvalidation
- Any widget module

No mirrors. No shadow fields. No duplication.

---

## 5. Closure Injection Boundary

This closure boundary is not a lawful sort path for a SPEC-HAL-188-backed consumer. Under SPEC-HAL-353, provider RowId
windows already carry the confirmed order and no comparator/sort closure may permute them.

**INVARIANT-261-A1-4 — Sort Injection Contract**

RowTopologyEngine MUST remain independent of:
- ColumnId
- SortState
- RowComparator
- Any widget-level types

Sorting MUST be injected via:

```rust
&dyn Fn(&mut [usize])
```

The closure may:
- Sort indices in-place.
- Read comparator and sort state.

The closure MUST NOT:
- Mutate substrate state.
- Access geometry.
- Access selection.
- Access scroll state.

This boundary remains frozen until sort types are extracted into a non-widget layer.

---

## 6. Virtualization Separation

**INVARIANT-261-A1-5 — Strict Phase Order**

The execution order remains permanently:

```
Topology → Geometry → Virtualization → Realization
```

Topology recompute MUST NOT occur after geometry resolution inside the same frame.

Virtualization MUST NOT mutate topology.

---

## 7. Forbidden Regressions

The following changes are explicitly forbidden:
1. Reintroducing `compute_sorted_indices`.
2. Reintroducing substrate-side `topology_dirty` flags.
3. Adding new topology pre-gates outside `ensure_topology`.
4. Bypassing `ensure_topology()` at any call site.
5. Moving sorting logic into the engine directly.
6. Making GeometryEngine polymorphic.
7. Allowing VirtualizationEngine to read sort state.

Any of these constitute architectural regression.

---

## 8. Stability Declaration

As of this amendment:
- Topology extraction is structurally complete.
- Gating authority is engine-owned.
- Orchestration is centralized.
- Boundaries are hardened.

Further work may extend functionality, but the structural boundary defined here is frozen unless amended by a new SPEC.
