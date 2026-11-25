# SPEC-HAL-118F: ListView Column Resize Contract

**Status:** Deprecated
**Layer:** Column Interaction / Layout Contract

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

**Scope:** Column resize semantics and invariants for ListView
**Depends on:** SPEC-HAL-118D, SPEC-HAL-118E
**Enables:** SPEC-HAL-118G, SPEC-HAL-118H, SPEC-HAL-118K
**Non-Goals:** Resize gestures, UX affordances, snapping, persistence, animations

---

## 1. Purpose

This SPEC defines the **structural and behavioral contract** for column resizing in `ListView`.

It specifies:

* what resizing *means*
* which state is mutated
* how resizing affects layout and scrolling
* what ListView MUST and MUST NOT do

This SPEC does **not** define:

* how resize is triggered
* how resize handles look
* how input is captured

Those are layered on top later.

---

## 2. Core Principle

**Column resize mutates layout, not scroll.**

Resizing a column:

* changes column width
* changes horizontal content extent
* never directly moves scroll offset

---

## 3. Column Width Ownership

**INV-COL-OWN-1**

Each column has exactly one authoritative width value.

That width:

* lives in `ListView`
* is mutable only via the resize contract
* is never inferred from rendering output

---

## 4. Column Width Model

### 4.1 Width Types

Each column MUST declare a width mode:

* `Fixed(f32)`
* `Resizable { min: f32, max: f32 }`

No other width modes exist in this SPEC.

---

### 4.2 Effective Width Resolution

**INV-COL-WIDTH-1**

Resolved column width MUST:

* be deterministic
* be independent of scroll offset
* be identical for header, body, and footer

---

## 5. Resize Semantics

### 5.1 Resize Operation

A resize operation:

* targets exactly one column
* produces a new width
* is clamped to column constraints

**INV-COL-RESIZE-1**

Resize MUST be atomic:

* either fully applied
* or not applied at all

---

### 5.2 Constraints

**INV-COL-RESIZE-2**

After resize:

* width ≥ min
* width ≤ max
* no implicit redistribution occurs

Resizing one column MUST NOT resize others.

---

## 6. Layout Effects

### 6.1 Horizontal Extent Update

**INV-COL-LAYOUT-1**

After resize:

* horizontal content extent (`W_content`) MUST be recomputed
* viewport width remains unchanged
* horizontal overflow may appear or disappear

---

### 6.2 ScrollArea Interaction

Resizing a column:

* MAY cause horizontal scrollbar to appear or disappear
* MUST NOT modify scroll offset directly

**INV-COL-SCROLL-1**

If overflow resolves, ScrollArea clamps scroll offset as usual.

---

## 7. Rendering Guarantees

**INV-COL-RENDER-1**

During and after resize:

* header, body, and footer columns remain aligned
* no partial frame renders mismatched widths

---

## 8. Forbidden Behavior

The following are explicitly forbidden:

* resizing implemented via transforms
* resizing implemented via scroll offset shifts
* per-band width divergence
* auto-expanding sibling columns
* implicit normalization to viewport width

Any such behavior violates this SPEC.

---

## 9. State Exposure

**INV-COL-API-1**

ListView MUST expose:

* current column widths
* total horizontal content extent

This data is read-only to external consumers.

---

## 10. Failure Modes

The following indicate a broken implementation:

* header columns resize but body does not
* footer lags or clips incorrectly
* horizontal scroll jumps during resize
* resize causes selection or focus mutation

All are invariant violations.

---

## 11. Acceptance Criteria

This SPEC is satisfied if:

* resizing a column updates its width deterministically
* horizontal overflow reacts correctly
* no scroll offset mutation occurs
* header/body/footer stay aligned
* resize affects only the target column

---

## 12. Summary

SPEC-HAL-118F locks down **what resizing is allowed to do** and, more importantly, **what it is forbidden from doing**.

It establishes resizing as:

* a layout mutation
* a width-only operation
* fully orthogonal to scrolling and interaction

---

## Implementation Notes (ListView v1)

Column resize is implemented via drag interaction in the resize gutter. During drag, `active_resize` tracks the drag state. On release, a `ColumnStructureDirective::Resize` is emitted. Header, body, and footer remain aligned via shared `ComputedColumnGeometry`. Auto-size (double-click) uses content-based width measurement.

---

## Implementation Reference

- `ListView::active_resize` — drag state tracking
- `ColumnStructureDirective::Resize` — width change intent
- `MIN_COLUMN_WIDTH_PX` / `MAX_AUTO_COLUMN_WIDTH` — width constraints
- `compute_auto_width()` — content-based auto-sizing
