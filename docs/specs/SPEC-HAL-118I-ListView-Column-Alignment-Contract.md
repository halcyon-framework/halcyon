# SPEC-HAL-118I: ListView Column Alignment Contract

**Status:** Deprecated
**Scope:** ListView only

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---
**Layer:** Column presentation semantics
**Depends on:** SPEC-HAL-118B, SPEC-HAL-118C
**Enables:** SPEC-HAL-118D, SPEC-HAL-118E, SPEC-HAL-118F, SPEC-HAL-118G, SPEC-HAL-118H, SPEC-HAL-118K
**Related:** SPEC-HAL-118J
**Out of scope:** Sorting logic, resizing mechanics, pinning, text formatting rules

---

## 1. Motivation

ListView columns must render content with **predictable, uniform alignment** across all visual bands:

* Header
* Body
* Footer

Without a binding alignment contract, alignment logic leaks into renderers, becomes inconsistent across bands, and blocks future extensions such as column pinning, numeric formatting, or auto-layout strategies.

This SPEC defines **alignment as a column-level property**, not a renderer concern.

---

## 2. Alignment Model

Each column defines a single **horizontal alignment mode** that applies uniformly to:

* header cells
* body cells
* footer cells

Vertical alignment is **not** handled by this SPEC.

---

## 3. Alignment Modes

A column MAY specify one of the following alignment modes:

* `Start` — content aligned to the leading edge of the column
* `Center` — content centered within the column
* `End` — content aligned to the trailing edge of the column

Text direction (LTR / RTL) is out of scope; `Start` and `End` are resolved in layout space, not linguistic space.

---

## 4. Binding Invariants

### INV-COL-ALIGN-1

Each ListView column has **exactly one** alignment mode.

### INV-COL-ALIGN-2

The alignment mode applies identically to header, body, and footer cells.

### INV-COL-ALIGN-3

Alignment is resolved **after column width is finalized** and **before renderer invocation**.

### INV-COL-ALIGN-4

Renderers receive a **pre-aligned cell rect** and MUST NOT perform alignment adjustments themselves.

### INV-COL-ALIGN-5

Changing alignment MUST NOT affect column width, scroll extents, or virtualization state.

---

## 5. Renderer Contract

Render callbacks receive:

* a rect already positioned and sized according to alignment
* no alignment flags or hints

Renderers are responsible only for **painting within the provided rect**.

Any renderer that performs its own horizontal alignment is in violation of this SPEC.

---

## 6. Defaults

If no alignment is specified:

* Default alignment is `Start`

Defaults are applied at column construction time and are immutable unless explicitly overridden.

---

## 7. Interaction With Other SPECs

* **SPEC-HAL-118F (Column Resize):**
  Alignment does not change during resize; only the aligned rect width changes.

* **SPEC-HAL-118G (Column Reflow):**
  Alignment is preserved across reflow operations.

* **SPEC-HAL-118H (Sort Affordance):**
  Sort indicators are rendered within the aligned header cell rect.

---

## 8. Non-Goals

This SPEC explicitly does NOT define:

* text formatting rules (numeric, locale-aware, etc.)
* per-row or per-cell alignment overrides
* vertical alignment
* baseline alignment
* auto-alignment based on data type

Each of these requires a separate SPEC.

---

## 9. Summary

Column alignment in ListView is:

* column-scoped
* declarative
* uniform across bands
* renderer-agnostic
* layout-resolved

This contract is foundational and MUST be implemented before higher-level table features are introduced.
