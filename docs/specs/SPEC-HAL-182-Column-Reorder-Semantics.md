# SPEC-HAL-182: Column Reorder Semantics

**Status:** Draft (Binding upon adoption)
**Layer:** TabularSubstrate Column System / Interaction Law
**Scope:** ListView, TabularView, TreeView projections
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Depends on:** SPEC-HAL-164, SPEC-HAL-165, SPEC-HAL-168, SPEC-HAL-169, SPEC-HAL-177, SPEC-HAL-180, SPEC-HAL-181
**Non-Goals:** Persistence format, animation, UX polish, rendering effects

---

## 1. Purpose

This SPEC defines the only lawful model for column reordering.

Column reorder is not geometry manipulation.
It is identity permutation under strict interaction arbitration rules, executed exclusively by TabularSubstrate via intent (SPEC-HAL-177).

This SPEC exists because prior failures treated reorder as:

* index shuffling,
* widget movement,
* geometry mutation,
* or ad-hoc drag behavior.

All of those are forbidden.

---

## 2. Definition

Column reorder is a structural permutation of ColumnId order within the Column Model.

Formally:

```
Reorder := Permutation<ColumnId>
```

Reorder:

* does not change ColumnId
* does not touch bindings
* does not mutate widgets
* does not directly manipulate geometry

---

## 3. Authority Placement

**INV-COL-REORDER-AUTH-1 — Single Authority**

* Column reorder is owned by TabularSubstrate (SPEC-HAL-164 semantic engine authority).
* Embedding components (ListView, TreeView) MAY emit reorder intent.
* Embedding components MUST NOT mutate column order directly.
* Geometry resolution remains governed by SPEC-HAL-168.
* Intent execution flows through SPEC-HAL-177.

There is exactly one authoritative column order at any time.

---

## 4. Interaction Origin and Capture

**INV-COL-REORDER-ORIGIN-1 — Header Only**

* Column reorder MAY originate only from header interaction.
* Body, cells, or content widgets MUST NOT initiate reorder.

**INV-COL-REORDER-CAP-1 — Exclusive Capture**

* Reorder interaction MUST acquire pointer capture.
* While active:
    * resize is suppressed
    * horizontal scroll is suppressed
    * selection is suppressed

Resize and reorder MUST NOT be active simultaneously.

---

## 5. Reorder Lifecycle

Column reorder follows a strict lifecycle:

1. **Begin**
    * Pointer down on reorder handle / header drag zone
    * Capture acquired
    * Initial ColumnId recorded
2. **Update**
    * Pointer move updates candidate permutation
    * No geometry mutation
    * No widget mutation
    * No binding mutation
3. **Commit**
    * `ColumnReorderIntent { permutation }` emitted
    * Intent is queued per SPEC-HAL-177
    * Capture released
4. **Cancel**
    * No structural changes
    * Capture released

At no point is column order mutated directly during pointer move.

---

## 6. Geometry Resolution

**INV-COL-REORDER-GEOM-1 — Post-Commit Only**

* Geometry recalculation occurs only after commit
* Per SPEC-HAL-168, geometry is recomputed from:
    * new column order
    * preferred widths
    * constraints

During Update:

* hit-testing uses existing geometry
* visual indicators MAY be rendered
* layout is frozen

---

## 7. Binding Stability Law

**INV-COL-REORDER-BIND-1 — Zero Binding Mutation**

Column reorder MUST satisfy:

* `(RowId, ColumnId) → WidgetBinding` remains unchanged
* Widgets remain row-owned
* No widget reparenting
* No widget recreation
* No focus reassignment

This is enforced by SPEC-HAL-181.

If reorder causes widget movement, the implementation is wrong.

---

## 8. Interaction with Inline Editing

**INV-COL-REORDER-EDIT-1 — Editing Gate**

* Column reorder MUST NOT commit while inline editing is active.
* If reorder is initiated during edit:
    * intent MUST be deferred (SPEC-HAL-177)
    * or reorder MUST be cancelled

Reorder and edit are mutually exclusive structural operations.

---

## 9. Interaction with Sorting

**INV-COL-REORDER-SORT-1 — Orthogonality**

* Column reorder MUST NOT:
    * trigger sorting
    * alter comparator association
    * reorder rows

Sorting semantics are owned by TabularSubstrate (SPEC-HAL-171); intent mapping is governed by SPEC-HAL-179A §4.2.

Column order and row order are orthogonal.

---

## 10. Forbidden Patterns (Hard Law)

The following are architecturally illegal:

* index-based reorder
* widget movement between columns
* geometry mutation during drag
* cell-level reorder logic
* reorder originating from body
* mixing resize and reorder
* reparenting widgets
* rebuilding bindings
* committing reorder mid-event dispatch

Violations are SPEC breaches, not bugs.

---

## 11. Minimal Guarantees

Any compliant implementation MUST guarantee:

* Column identity stability
* Binding stability
* Widget identity stability
* Deterministic reorder outcome
* No interaction cross-talk

---

## 12. Acceptance Criteria

SPEC-HAL-182 is satisfied when:

* Column reorder is identity-based, not index-based
* Geometry changes only after commit
* Bindings survive reorder unchanged
* Editing blocks reorder commit
* Only one column order authority exists

---

## 13. Strategic Note (Non-Normative)

Column reorder is a structural mutation with UI affordance, not a UI effect.

This SPEC specializes the column reorder intent execution defined by TabularSubstrate. Embedding components emit intents; TabularSubstrate owns execution. This is not a parallel authority — it is a semantic specialization under SPEC-HAL-164.

If it feels "heavy", good — that means it's safe.
