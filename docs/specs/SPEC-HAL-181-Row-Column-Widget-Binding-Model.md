# SPEC-HAL-181: Row–Column–Widget Binding Model

**Status:** Draft (Binding upon adoption)
**Layer:** Core Widget / Interaction Subsystems
**Scope:** ListView, TreeView, all row-centric tabular views
**Depends on:** SPEC-HAL-179, SPEC-HAL-180, SPEC-HAL-167, SPEC-HAL-173, SPEC-HAL-178, SPEC-PM-001
**Non-Goals:** Rendering, styling, data providers, business logic

---

## 1. Purpose

This SPEC defines the binding model between:

* rows (semantic units),
* columns (horizontal partitions),
* widgets (actual interactive/rendered entities).

The binding model exists to prevent structural ambiguity when columns are resized, reordered, edited, virtualized, or projected into trees.

This SPEC exists because previous ListView failures were caused by implicit, index-based, or container-style cell models.

---

## 2. Definition

A binding is a pure association, not containment.

```
Binding := (RowId, ColumnId) → WidgetBinding
```

Where WidgetBinding refers to a widget owned by the row subtree.

Bindings do not imply:

* widget ownership,
* widget parentage,
* geometry authority,
* lifetime control.

Bindings are identity-based projections.

---

## 3. Authority and Ownership

### 3.1 Binding Ownership

**INV-BIND-OWN-1 — Row-Owned Bindings**

Bindings are owned by the Row System.

* Created when a row is realized
* Destroyed when the row is destroyed
* Persist across:
    * column resize
    * column reorder
    * scrolling / virtualization

Columns and cells MUST NOT own bindings.

---

### 3.2 What Bindings Do NOT Own

Bindings MUST NOT:

* own widgets
* reparent widgets
* clone widgets
* store geometry
* store layout state
* store selection or focus state

Bindings are references only.

---

## 4. Widget Ownership Law

**INV-WIDGET-OWN-1 — Widgets Are Row-Owned**

All widgets displayed in cells:

* are children of the row widget subtree
* participate in row layout
* participate in global widget identity lifecycle

Cells and columns MUST NOT contain widgets.

This is non-negotiable.

---

## 5. Cell Projection Interaction

**INV-CELL-BIND-1 — Cells Consume Bindings**

A cell:

```
Cell = intersect(RowRect, ColumnSlice)
```

Consumes:

* a binding (if one exists)
* a clip rect
* an X-offset constraint

Cells:

* do not store bindings
* do not mutate bindings
* do not own widgets

Cells are read-only projection sites.

---

## 6. Column Reorder Safety

**INV-BIND-REORDER-1 — Binding Stability Under Reorder**

When columns are reordered:

* ColumnId moves
* binding follows ColumnId
* widget identity is preserved
* row subtree is untouched

No rebinding.
No widget movement.
No relayout beyond X offsets.

Index-based binding is forbidden.

---

## 7. Inline Editing Semantics

**INV-BIND-EDIT-1 — Binding Swap, Not Widget Mutation**

Inline editing is modeled as binding substitution, not widget mutation.

Example:

```
Normal mode:
(RowId, ColumnId) → TextWidget

Edit mode:
(RowId, ColumnId) → TextFieldWidget
```

Rules:

* widgets are swapped at the binding level
* original widget is preserved (not destroyed)
* edit lifecycle follows SPEC-HAL-174 / 174A
* commit/cancel restores original binding

Cells do not change.
Columns do not change.
Row height does not change.

---

## 8. Virtualization Law

**INV-BIND-VIRT-1 — Bindings Are Identity-Stable**

Bindings MUST:

* be keyed by (RowId, ColumnId)
* survive virtualization
* survive invisibility
* never depend on realization index

Visibility MUST NOT affect binding existence.

This is required to satisfy SPEC-PM-001.

---

## 9. Forbidden Patterns (Hard Law)

The following are architecturally illegal:

* cell-owned widgets
* column-owned widgets
* binding by column index
* binding by visible row index
* reparenting widgets on reorder
* moving widgets between rows
* storing widgets in cell structs
* rebuilding bindings on scroll

Any of these is a design violation, not a bug.

---

## 10. Relationship to Other Systems

**Column System (SPEC-HAL-180)**

* Columns define X slices only
* Bindings follow ColumnId
* Columns never see widgets

**Selection System (SPEC-HAL-178)**

* Selection is row-based
* Bindings do not participate

**Focus System (SPEC-HAL-175 / SPEC-HAL-038)**

* Focus belongs to widgets
* Cells and bindings never hold focus

---

## 11. Acceptance Criteria

SPEC-HAL-181 is satisfied when:

* No widget is owned by a cell or column
* Reordering columns preserves widget identity without mutation
* Inline editing swaps bindings, not widgets
* Virtualization does not affect bindings
* All bindings are identity-based and row-owned

---

## 12. Strategic Note (Non-Normative)

This SPEC is the keystone between:

* row semantics,
* column geometry,
* widget reality.

Without it, ListView will collapse again under reorder, edit, or virtualization.

With it, implementation becomes mechanical.
