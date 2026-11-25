# SPEC-HAL-118C: ListView Column Presentation Contract

**Status:** Deprecated
**Layer:** Presentation (Visual Only)

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Scope:** ListView column visual semantics
**Depends on:** SPEC-HAL-118B
**Enables:** SPEC-HAL-118I, SPEC-HAL-118J, SPEC-HAL-118D
**Non-Goals:** Interaction, scrolling, resizing, sorting, data ownership

---

## 1. Purpose

This SPEC defines the **pure presentation contract** for `ListView` columns.

It formalizes how columns are **laid out and rendered visually** across:

* header band
* body rows
* footer band

This SPEC introduces **no behavior**.
It exists to lock down **visual invariants** so future SPECs can safely layer interaction on top.

---

## 2. Core Principle

**ListView columns are visual projections only.**
They do not own data, state, interaction, or layout decisions beyond presentation.

---

## 3. Column Definition Model

Each column is defined by a `ListViewColumn` descriptor.

### 3.1 Required Properties

Each column MUST define:

* `column_id: ColumnId`
  Stable identifier. No ordering semantics.

* `width: f32`
  Logical column width in layout units.

---

### 3.2 Optional Presentation Properties

Columns MAY define:

* `min_width: Option<f32>`
* `max_width: Option<f32>`

These are **constraints only**, not resizing instructions.

* `horizontal_alignment: Left | Center | Right`
  Applies uniformly to header, body, and footer.

* `cell_padding: Insets { left, right, top, bottom }`

Defaults (if unspecified):

* alignment = `Left`
* padding = `{ left: 8, right: 8, top: 4, bottom: 4 }`

---

## 4. Column Band Symmetry

### 4.1 Mandatory Symmetry Rule

**INV-COL-SYM-1**

A column’s horizontal geometry MUST be identical across:

* header band
* body region
* footer band

This includes:

* x-position
* width
* alignment
* padding

---

### 4.2 Renderer Consistency

If a column provides:

* a header renderer
* a footer renderer

They MUST receive:

* the same column rect (x, width)
* independent y/height scoped to the band

---

## 5. Rendering Responsibility Split

### 5.1 What ListView Renders

ListView is responsible for rendering:

* column backgrounds (header and footer only)
* column separators (if enabled)
* header/footer content via render callbacks

---

### 5.2 What ListView Does NOT Render

ListView MUST NOT:

* render body row content
* render selection state
* render focus
* render hover
* render scroll effects

Body rows are rendered exclusively by `List`.

---

## 6. Column Separators (Visual Only)

Columns MAY enable vertical separators.

### 6.1 Separator Rules

* Separators are purely decorative
* They do not affect hit-testing
* They do not affect layout math
* They do not imply resizability

**INV-COL-SEP-1**

Separators MUST align with column boundaries and span all bands consistently.

---

## 7. Coordinate & Layout Invariants

### 7.1 Column Positioning

* Columns are laid out left-to-right
* Order is defined strictly by column declaration order
* No reordering allowed at this layer

**INV-COL-ORDER-1**

Column order is immutable at the presentation layer.

---

### 7.2 Width Resolution

**INV-COL-WIDTH-1**

The resolved width of a column MUST satisfy:

min_width ≤ width ≤ max_width
(if bounds exist)

No dynamic adjustment is allowed in this SPEC.

---

## 8. Forbidden Capabilities (Hard Constraints)

This SPEC explicitly forbids:

* sorting
* filtering
* resizing
* drag handles
* cursor changes
* hover feedback
* focus visuals
* scroll ownership
* horizontal overflow handling

Any attempt to implement these here is a contract violation.

---

## 9. Interaction with SPEC-HAL-118B

* Selection semantics remain owned by `List`
* Focus semantics remain owned by `List`
* Scroll semantics remain external

ListView remains a stateless visual wrapper.

---

## 10. Forward Compatibility Hooks (Non-Behavioral)

This SPEC intentionally allows future SPECs to:

* reinterpret `min_width / max_width`
* attach resize affordances
* attach sort indicators
* introduce horizontal scroll

None of those behaviors exist here.

---

## 11. Acceptance Criteria

This SPEC is considered correctly implemented if:

* Column widths align identically across header/body/footer
* ListView renders no body content
* Removing ListView leaves List behavior unchanged
* Visual-only changes do not alter interaction semantics

---

## 12. Summary

SPEC-HAL-118C locks down **what a column looks like**, nothing more.

It is narrow, strict, and intentionally boring.

That's the point.

---

## Implementation Notes (ListView v1)

Column presentation is implemented via `ComputedColumnGeometry` which provides per-column x-position and width. Separators are drawn by `ListViewContentContainer::paint()`. Header/footer rendering uses the same geometry as body. Column padding (`HEADER_CELL_PADDING_X`) is applied uniformly.

---

## Implementation Reference

- `ComputedColumnGeometry` — per-frame column layout
- `ColumnVisibleRect` — individual column x/width
- `HEADER_CELL_PADDING_X` — horizontal cell padding
- `ListViewContentContainer::paint()` — separator rendering
