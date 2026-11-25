# SPEC-HAL-118K: ListView Column Pinning Preparation

**Status:** Deprecated
**Scope:** ListView only

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked as preparation SPEC. Actual pinning deferred to ListView.

---
**Layer:** Column layout / future interaction preparation
**Depends on:** SPEC-HAL-118E, SPEC-HAL-118G
**Related:** SPEC-HAL-118H
**Out of scope:** Actual pin/unpin interaction, UI affordances, drag logic, persistence

---

## 1. Motivation

Column pinning (a.k.a. frozen columns) is a **structural layout concern**, not an interaction feature.

If pinning is not planned for early, later additions cause:

* duplicated scroll logic,
* desynchronized header/body rendering,
* broken horizontal scroll invariants,
* or incompatible column resize behavior.

This SPEC introduces **preparatory invariants and structure** so that column pinning can be added later **without refactoring ListView internals**.

No pinning behavior is implemented here.

---

## 2. Conceptual Model

Columns are conceptually divided into **zones**:

* **Leading zone** (future pinned-left)
* **Scrollable zone**
* **Trailing zone** (future pinned-right)

In this SPEC:

* all columns belong to the *scrollable zone*
* zone boundaries exist logically but are inactive

---

## 3. Binding Invariants

### INV-COL-PIN-1

ListView MUST treat columns as an **ordered sequence with zone metadata**, even if all zones are identical.

### INV-COL-PIN-2

Column layout computation MUST be expressible as the concatenation of independent column zones.

### INV-COL-PIN-3

Horizontal scrolling MUST be applied **only** to the scrollable zone.

### INV-COL-PIN-4

Leading and trailing zones MUST NOT depend on scroll offset.

### INV-COL-PIN-5

In absence of active pinning, all columns reside in the scrollable zone.

---

## 4. Layout Preparation Rules

ListView layout logic MUST:

* compute total width per zone
* compute x-offsets relative to:

    * left edge (leading zone)
    * scroll origin (scrollable zone)
    * right edge (trailing zone)

Even if only one zone exists, this structure MUST be preserved.

---

## 5. Rendering Preparation

Rendering MUST be expressible as:

1. Render leading zone (no horizontal scroll)
2. Render scrollable zone (with horizontal scroll)
3. Render trailing zone (no horizontal scroll)

In this SPEC:

* steps 1 and 3 are empty
* step 2 renders all columns

---

## 6. Interaction With Other SPECs

* **SPEC-HAL-118D (Horizontal Extent Model):**
  Extent calculation must remain compatible with multiple zones.

* **SPEC-HAL-118E (Horizontal Scroll Integration):**
  Scroll offset applies only to scrollable zone.

* **SPEC-HAL-118F (Column Resize):**
  Resize affects the zone-local width, not global scroll logic.

* **SPEC-HAL-118J (Column Separator Model):**
  Separators are zone-aware and follow column ownership.

---

## 7. Explicit Non-Goals

This SPEC does NOT define:

* pin/unpin state
* drag handles
* persistence
* hit-testing
* visual styling
* keyboard interaction

All of those require a dedicated SPEC.

---

## 8. Forward Compatibility Guarantee

Any future SPEC implementing column pinning MUST:

* respect zone boundaries defined here
* avoid introducing additional scroll authorities
* reuse the zone-based layout model

Violating these constraints constitutes an architectural regression.

---

## 9. Summary

This SPEC establishes:

* zone-based column thinking
* scroll isolation guarantees
* layout structure that survives pinning

Nothing is pinned yet.

But once pinning is added, it will be **boring**, **clean**, and **non-destructive** — exactly as it should be.
