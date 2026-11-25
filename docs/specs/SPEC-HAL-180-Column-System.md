# SPEC-HAL-180: Column System (Horizontal Partition Authority)

**Status:** Draft (Binding upon adoption)
**Layer:** Widget Subsystems / Layout Consumer
**Scope:** ListView, TabularView, TreeView, all row-based tabular views
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Restates:** SPEC-HAL-165 (Column Model), SPEC-HAL-168 (Horizontal Geometry Resolution)
**Depends on:** SPEC-HAL-141, SPEC-HAL-158, SPEC-HAL-038, SPEC-HAL-079
**Non-Goals:** Widget ownership, layout, content measurement, vertical geometry, rendering, scrolling, focus

---

## 1. Purpose

This SPEC defines the Column System as a first-class, governed subsystem responsible for horizontal space partitioning in row-based views.

The Column System is not a feature.
It is a geometry authority with explicit invariants.

**Authority relationship:** The Column System is implemented by TabularSubstrate (SPEC-HAL-164–178). This SPEC is a formal invariant consolidation of SPEC-HAL-165 (Column Model) and SPEC-HAL-168 (Horizontal Geometry Resolution), expressed for embedding component consumption. It does not define a parallel authority.

Violations of this SPEC are architectural errors.

---

## 2. Definition

Column System = TabularSubstrate's column infrastructure (SPEC-HAL-165, SPEC-HAL-168), which:

* partitions available horizontal space into columns
* owns column widths and X offsets
* governs column resize and reorder interactions (via intent execution per SPEC-HAL-177)
* preserves column identity across time

This is the same authority defined in SPEC-HAL-165 (Column Model) and SPEC-HAL-168 (Horizontal Geometry Resolution). No parallel column authority exists.

The Column System does not:

* own widgets
* perform layout (layout authority belongs to TabularSubstrate layout phase)
* measure content
* control vertical geometry
* render
* scroll
* manage focus

---

## 3. Terminology

* **Column** — a persistent horizontal partition with identity
* **ColumnId** — stable identifier of a column
* **Cell** — the intersection of a row's vertical rect and a column's horizontal slice
* **Row** — the sole owner of vertical (Y) geometry

---

## 4. Column Identity Invariants

### INV-COL-ID-1: Stable Identity

Each column MUST have a persistent ColumnId.

* Identity MUST survive:
    * resize
    * reorder
    * hide/show
* Identity MUST NOT depend on index or position.

Forbidden: index-based column identity.

---

### INV-COL-ID-2: Identity Preservation on Reorder

Reordering columns MUST move identity, width, and bindings together.

* No recreation
* No teardown
* No rebinding

---

## 5. Authority Invariants

### INV-COL-AUTH-1: Exclusive Width Authority

Only TabularSubstrate (via the Column System) MAY mutate column widths.

* Rows MAY read widths.
* Widgets MAY NOT influence widths.
* Layout consumers MUST NOT negotiate widths.
* Embedding components (ListView, TreeView) provide configuration inputs but do not mutate widths directly.

---

### INV-COL-AUTH-2: Horizontal Authority Only

The Column System MAY mutate X geometry only.

It MUST NOT:

* affect row height
* affect widget vertical layout
* trigger re-measurement of content

---

## 6. Width Conservation Invariants

### INV-COL-WIDTH-1: Width Conservation

For non-elastic configurations:

```
Σ column_widths = available_row_width
```

No silent overflow.
No silent underflow.

---

### INV-COL-WIDTH-2: Explicit Elasticity Only

If elasticity exists, it MUST be explicit and governed (see section 10).

Implicit redistribution is forbidden.

---

## 7. Minimum / Maximum Width Invariants

### INV-COL-SIZE-1: Explicit Minimum Width

Every column MUST declare a minimum width.

* Minimum width is configuration-defined.
* Content MUST NOT affect minimum width.
* Text truncates; columns do not shrink.

---

### INV-COL-SIZE-2: Explicit or Unbounded Maximum Width

Maximum width MUST be either:

* explicitly configured, or
* unbounded

Content-derived maxima are forbidden.

---

## 8. Resize Interaction Invariants

Resize interaction flows through TabularSubstrate intent execution (SPEC-HAL-169, SPEC-HAL-177). Embedding components emit resize intents; TabularSubstrate owns semantic execution.

### INV-COL-RESIZE-1: Two-Column Locality

Dragging a divider affects exactly two columns:

* left column
* right column

No other columns may change width.

---

### INV-COL-RESIZE-2: Edge Column Asymmetry

* First column:
    * left edge is immovable
    * resize affects (col₀, col₁) only
* Last column:
    * right edge is immovable
    * resize affects (colₙ₋₁, colₙ) only

---

### INV-COL-RESIZE-3: Hard Minimum Enforcement

If a column reaches its minimum width:

* resize MUST stop immediately
* no resistance curves
* no redistribution
* no spring behavior

---

### INV-COL-RESIZE-4: Geometry-Only Resize

During resize:

* no text reflow across columns
* no widget re-measurement
* no row height changes

Only X offsets and widths change.

---

## 9. Column Reordering Invariants

Column reordering flows through TabularSubstrate intent execution (SPEC-HAL-177). Embedding components emit reorder intents; TabularSubstrate owns semantic execution.

### INV-COL-ORDER-1: X-Only Mutation

Reordering columns MAY change:

* column order
* X offsets

It MUST NOT:

* trigger layout invalidation
* trigger widget measurement
* affect Y geometry

---

### INV-COL-ORDER-2: Binding Preservation

Column reordering MUST preserve:

* column identity
* column width
* row-level column bindings

---

## 10. Elastic Columns (Optional, Explicit)

### INV-COL-ELASTIC-1: Singular Elastic Column

At most one elastic column MAY exist.

It absorbs remaining space:

```
remaining = available_width − Σ fixed_columns
```

---

### INV-COL-ELASTIC-2: Elastic Minimum Enforcement

Elastic columns MUST still respect minimum width.

If remaining space < min width:

* clamp
* overflow behavior MUST be explicit (scroll or clip)

---

## 11. Cells (Projection Law)

### INV-CELL-PROJ-1: Cells Are Projections

Cells are non-owning projections defined by:

```
cell_rect = intersect(row_rect, column_slice)
```

Cells MUST NOT:

* own widgets
* measure content
* store state
* participate in layout authority

---

## 12. Interaction and Routing Invariants

### INV-COL-HIT-1: Routing via Column Binding

Hit-testing MAY resolve column to widget via bindings.

Cells MUST NOT receive focus.
Columns MUST NOT receive focus.

---

## 13. Persistence Invariants

### INV-COL-PERSIST-1: Identity-Keyed Persistence

Persistable state MUST be keyed by ColumnId:

* width
* order
* visibility

Index-keyed persistence is forbidden.

---

## 14. Forbidden Behaviors (Non-Exhaustive)

The following are architecturally illegal:

* content-driven column sizing
* implicit width redistribution
* column-owned widgets
* cell-owned widgets
* resize affecting more than two columns
* minimum width violations
* index-based identity

---

## 15. Acceptance Criteria

SPEC-HAL-180 is satisfied when:

* Column widths are mutated only by TabularSubstrate (via the Column System)
* Resize behavior is local, predictable, and min-safe
* Reordering preserves identity without relayout
* Rows own Y; columns own X; no overlap exists
* No content can influence column geometry
* Embedding components (ListView, TreeView) emit intents, not direct mutations

---

## 16. Strategic Note (Non-Normative)

This SPEC exists because columns failed twice when treated as UI sugar.
They are mechanical geometry law.

This SPEC consolidates the invariants of SPEC-HAL-165/168 into a single reference document for embedding components. It does not supersede or compete with TabularSubstrate authority — it restates it for clarity and enforcement.

There will not be a third attempt without this contract.
