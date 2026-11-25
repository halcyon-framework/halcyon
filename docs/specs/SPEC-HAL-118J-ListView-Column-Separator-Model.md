# SPEC-HAL-118J: ListView Column Separator Model

**Status:** Deprecated
**Scope:** ListView only

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---
**Layer:** Column presentation / visual structure
**Depends on:** SPEC-HAL-118B, SPEC-HAL-118C
**Enables:** SPEC-HAL-118D, SPEC-HAL-118E, SPEC-HAL-118F, SPEC-HAL-118G, SPEC-HAL-118H, SPEC-HAL-118K
**Related:** SPEC-HAL-118I
**Out of scope:** Interactive resize mechanics, hit-testing, theming system, row separators

---

## 1. Motivation

Column separators are a **structural affordance**, not decoration.

Without a binding model, separators tend to:

* leak into renderers,
* get duplicated across header/body/footer,
* interfere with resizing and reflow,
* or become impossible to reason about once horizontal scrolling is introduced.

This SPEC defines column separators as a **first-class column boundary concept**, owned by ListView layout, not by renderers.

---

## 2. Separator Model

A **column separator** is a vertical visual element rendered **between adjacent columns**.

Separators are:

* column-boundary–derived
* layout-resolved
* uniform across header, body, and footer bands
* visually passive (non-interactive in this SPEC)

---

## 3. Separator Placement

Separators are placed:

* **between column N and column N+1**
* spanning the full vertical extent of the ListView body area
* optionally spanning header and footer (see Section 6)

No separator exists:

* before the first column
* after the last column

---

## 4. Binding Invariants

### INV-COL-SEP-1

Separators are derived **only** from the column layout model.

### INV-COL-SEP-2

Separators are rendered **after column widths are finalized** and **before cell rendering**.

### INV-COL-SEP-3

Separators MUST NOT affect column width, layout, or scroll extents.

### INV-COL-SEP-4

Separators are visually consistent across header, body, and footer unless explicitly disabled per band.

### INV-COL-SEP-5

Separators are owned by ListView and MUST NOT be rendered by column renderers.

---

## 5. Geometry

Each separator has:

* a fixed x-position equal to the trailing edge of column N
* a fixed width (default: 1 logical pixel)
* a vertical span defined by band policy (Section 6)

Separator geometry is computed in **ListView local coordinates** and participates in clipping like any other visual element.

---

## 6. Band Coverage Policy

Separators MAY be rendered in the following bands:

* Header
* Body
* Footer

This SPEC defines the default policy:

* Body: **enabled**
* Header: **enabled**
* Footer: **enabled**

Band-level enable/disable is a future extension and MUST NOT be implemented ad-hoc.

---

## 7. Rendering Order

Within ListView paint order:

1. Backgrounds (header/body/footer)
2. Column separators
3. Header cell content
4. Body content (owned by List → ScrollArea)
5. Footer cell content

Separators MUST NOT overpaint body content during scrolling.

---

## 8. Interaction With Other SPECs

* **SPEC-HAL-118F (Column Resize):**
  Separator position updates live with column width changes.

* **SPEC-HAL-118G (Column Reflow):**
  Separators reflow exactly with their associated column boundaries.

* **SPEC-HAL-118I (Column Alignment):**
  Alignment has no effect on separator placement.

---

## 9. Non-Goals

This SPEC explicitly does NOT define:

* draggable resize handles
* hover or active states
* hit-testing behavior
* row separators
* styling or theming APIs

Those require separate SPECs.

---

## 10. Summary

Column separators are:

* structural, not decorative
* layout-owned, not renderer-owned
* uniform across bands
* independent of scrolling and alignment

This SPEC establishes the minimal, immutable separator model required before resizing and advanced table behavior can be safely layered on top.
