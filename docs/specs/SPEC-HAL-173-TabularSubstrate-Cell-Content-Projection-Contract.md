# SPEC-HAL-173: TabularSubstrate — Cell Content Projection Contract

**Status:** Implemented / Closed
**Layer:** Content Projection / Virtualization-Safe Embedding  
**Audience:** Engine, Widget, Virtualization  
**Depends on:**  
SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-086, SPEC-HAL-131A/B/C, SPEC-HAL-141, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-167, SPEC-HAL-168

---

## 1. Purpose

This SPEC defines **what may be projected inside a TabularSubstrate cell** and the strict limits under which such content may
exist.

It exists to prevent:

- structural contamination by widgets
- layout feedback loops
- virtualization-unsafe editors
- hidden geometry authority leaks

Cells are **slots**, not containers.

---

## 2. Cell Content Definition

A **Cell Content Projection** is any renderable or interactive entity hosted inside a cell rect.

Examples:

- text renderer
- checkbox
- dropdown
- date picker
- inline editor
- composite widget

TabularSubstrate treats all of them as opaque.

---

## 3. Geometry Contract

Each cell is assigned a **fixed rect**:

```

(row_id, column_id) → Rect

```

**INVARIANT-1:**  
Cell content must render strictly within its assigned rect.

**INVARIANT-2:**  
Cell content must not read or modify:

- column geometry
- row geometry
- viewport geometry

---

## 4. Lifecycle and Virtualization

Cell content lifecycle is tied to **row realization**.

**INVARIANT-3:**  
Cell content may be destroyed and recreated at any time due to virtualization.

**INVARIANT-4:**  
Cell content must not assume persistence across frames.

Stateful editors must externalize state.

---

## 5. Interaction Constraints

Cell content may:

- receive pointer events
- receive keyboard events when focused
- emit intent directives

Cell content must not:

- capture pointer globally
- mutate scroll state
- trigger layout invalidation
- assume modal authority

---

## 6. Focus and Ownership

Focus ownership is external.

**INVARIANT-5:**  
Cell content must not claim global focus ownership.

Focus is projected into the cell by TabularSubstrate.

---

## 7. Rendering Isolation

Cell content rendering:

- occurs in body rendering pass
- respects clipping
- respects scroll transforms

Forbidden:

- absolute positioning outside the cell
- z-order escalation
- cross-cell drawing

---

## 8. Forbidden Behaviors

Explicitly illegal:

- resizing columns from cell content
- modifying row height
- caching geometry across frames
- registering global shortcuts
- storing state keyed by row index

---

## 9. Scope Boundaries

This SPEC does **not** define:

- inline editing lifecycle
- focus navigation
- selection semantics
- widget APIs

Those are layered separately.

---

## 10. Relationship to Deprecated SPECs

This SPEC replaces implicit cell-widget assumptions from:

- SPEC-HAL-118*
- SPEC-HAL-135*

No semantics are inherited.

---

## 11. Next SPEC

SPEC-HAL-174 will define:

**Inline Editing Lifecycle and Intent Model**

---

## 12. Closure

**This SPEC is fully implemented and closed.**

Implementation: `src/widgets/tabular_view/cell_content.rs`, body integration in `body.rs`.

Enforcement:

- **INVARIANT-1:** `CellContentProjection` is a plain data struct — no methods beyond field access, no rendering, no ownership.
- **INVARIANT-2:** `project_from_cell()` copies `cell.cell_id` and `cell.rect` directly — no independent derivation from row/column/viewport.
- **INVARIANT-3:** `content_projections` is cleared at the start of `realize_viewport_sorted()` alongside `realized_cells`. Content projections exist only for realized rows with visible columns.
- **INVARIANT-4:** `project_from_cell()` is a pure function — same CellProjection always produces same CellContentProjection.
- **INVARIANT-5:** No geometry table, column model, or row model is read or mutated by projection code.
- **INVARIANT-6:** `apply_tree_indentation()` mutates content projections lockstep with realized_cells — `proj.content_rect = cell.rect` after indent.
- **Count invariant:** `content_projections.len() == realized_cells.len()` — 1:1, same ordering.

11 integration tests in `cell_content_projection.rs`.

Future changes to cell content embedding, widget hosting, or content-rect derivation require a new SPEC.
