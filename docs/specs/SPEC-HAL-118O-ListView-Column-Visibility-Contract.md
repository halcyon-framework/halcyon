# SPEC-HAL-118O: ListView Column Visibility Contract

**Status:** Deprecated
**Layer:** ListView / Column Projection

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Depends on:**

* SPEC-HAL-118B – ListView Columnar Contract
* SPEC-HAL-118C – ListView Column Presentation
* SPEC-HAL-118N – ListView Column State Externalization
* SPEC-HAL-118K – ListView Column Pinning Preparation

---

## 1. Purpose

This SPEC defines the **column visibility model** for ListView.

It specifies how columns may be shown or hidden, how visibility interacts with layout and projection, and where authority over visibility state resides.

This SPEC exists to lock semantics **before** visibility becomes an implicit side effect of pinning or resizing.

---

## 2. Core Principle

**Visibility is a projection decision, not a layout hack.**

Hidden columns are **non-existent** to layout, scrolling, hit-testing, and rendering.

---

## 3. Visibility State

Each column has a boolean visibility flag:

* `visible = true` → column participates fully
* `visible = false` → column is excluded entirely

Visibility state is externally owned per SPEC-HAL-118N.

---

## 4. Effects of Hidden Columns

When a column is hidden:

* It contributes **zero width**
* It is excluded from horizontal extent
* It receives no header, body, or footer rendering
* It receives no hit-testing
* It participates in no resize, sort, or pin logic

From ListView’s perspective, the column does not exist.

---

## 5. Layout and Ordering

Hidden columns are removed **before**:

* column ordering
* pin grouping
* reflow computation
* horizontal extent calculation

Ordering among visible columns remains stable.

---

## 6. Interaction with Pinning

Pinned state on a hidden column has no effect.

When a hidden column becomes visible:

* its previous pin state may be restored by the host
* ListView does not infer pin state

---

## 7. Interaction with Resizing

Hidden columns:

* have no width
* cannot be resized
* do not affect neighbor resize math

When re-shown, width is host-provided.

---

## 8. Interaction with Sorting

Hidden columns may still be used as sort keys **by the data layer**, but:

* ListView MUST NOT display sort affordances for hidden columns
* ListView MUST NOT emit sort intents for hidden columns

---

## 9. State Input and Emission

ListView receives visibility state as input.

ListView MAY emit intents such as:

* `ColumnVisibilityToggleIntent`

ListView MUST NOT mutate visibility directly.

---

## 10. No Implicit Collapse

Visibility is NOT implemented via:

* width = 0
* opacity = 0
* clip hacks

Visibility is structural exclusion.

---

## 11. Invariants

**INV-COL-VIS-1**
Hidden columns are excluded from all layout and interaction.

**INV-COL-VIS-2**
Visibility state is externally owned.

**INV-COL-VIS-3**
ListView renders only visible columns.

---

## 12. Relationship to Other SPECs

* SPEC-HAL-118N governs ownership
* SPEC-HAL-118D uses only visible columns for extent
* SPEC-HAL-118F resizes only visible columns
* SPEC-HAL-118K pinning applies only to visible columns

---

## 13. Rationale

Every table eventually needs hide/show.

Retrofit visibility later and you corrupt:

* scroll math
* pinning semantics
* resize logic

This SPEC prevents that.

---

## 14. Summary

Column visibility is explicit, external, and structural.

Hidden means **gone**, not "drawn differently."

No shortcuts.

---

## Implementation Notes (ListView v1)

Column visibility is a boolean field in `ColumnDef`. Hidden columns are filtered out during `compute_column_geometry()` and do not participate in layout, hit-testing, or rendering. Visibility is externally owned via `ColumnStructureController`.

---

## Implementation Reference

- `ColumnDef::visible` — visibility flag
- `ListView::compute_column_geometry()` — filters hidden columns
- `ColumnStructureController` — visibility state owner
