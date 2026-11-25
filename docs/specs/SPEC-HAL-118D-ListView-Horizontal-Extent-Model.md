# SPEC-HAL-118D: ListView Horizontal Extent Model

**Status:** Deprecated
**Layer:** Layout / Viewport Geometry

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---
**Scope:** Horizontal extent, overflow, and scrolling semantics for ListView
**Depends on:** SPEC-HAL-118B, SPEC-HAL-118C, SPEC-HAL-118I, SPEC-HAL-118J
**Enables:** SPEC-HAL-118E, SPEC-HAL-118F, SPEC-HAL-118G, SPEC-HAL-118H, SPEC-HAL-118K
**Non-Goals:** Column resizing, sorting, interaction, data semantics

---

## 1. Purpose

This SPEC defines how `ListView` behaves when the **total column width exceeds the available viewport width**.

It formalizes:

* horizontal extent calculation
* overflow detection
* horizontal scrolling eligibility
* separation of responsibilities between `ListView`, `ScrollArea`, and `List`

No interaction behavior is introduced.

---

## 2. Core Principle

**Horizontal scrolling is a viewport concern, not a data concern.**

`ListView` computes horizontal extent.
`ScrollArea` performs scrolling.
`List` remains unaware of horizontal overflow.

---

## 3. Horizontal Extent Definition

### 3.1 Column Sum Rule

**INV-HX-EXTENT-1**

The horizontal content extent of a ListView is defined as:

sum(column.width for all columns)

No implicit padding, margin, or separator width is added unless explicitly defined by SPEC-HAL-118C.

---

### 3.2 Extent Stability

**INV-HX-EXTENT-2**

Horizontal extent MUST remain stable across:

* header band
* body region
* footer band

---

## 4. Viewport vs Content Width

Let:

* `W_viewport` = allocated width from layout constraints
* `W_content` = resolved horizontal extent

### 4.1 No Overflow Case

If:

W_content ≤ W_viewport

Then:

* no horizontal scrolling is active
* content is rendered fully visible
* horizontal offset is zero

---

### 4.2 Overflow Case

If:

W_content > W_viewport

Then:

* horizontal overflow exists
* horizontal scrolling becomes eligible
* clipping MUST occur at viewport bounds

**INV-HX-OVERFLOW-1**

Overflow detection is purely geometric.
No heuristics. No tolerance bands.

---

## 5. Scroll Authority Separation

### 5.1 Ownership

**INV-HX-SCROLL-1**

`ListView`:

* computes `W_content`
* exposes horizontal extent metadata

`ScrollArea`:

* applies horizontal scroll offset
* renders horizontal scrollbar (if enabled)

`List`:

* is unaware of horizontal scrolling
* renders rows at x = 0 in logical space

---

### 5.2 Prohibited Behavior

`ListView` MUST NOT:

* apply horizontal translation itself
* maintain horizontal scroll state
* intercept horizontal scroll input

---

## 6. Horizontal Scroll Activation

Horizontal scrolling is enabled **only if**:

* `W_content > W_viewport`
* the parent `ScrollArea` has horizontal scrolling enabled

**INV-HX-ACTIVATE-1**

Horizontal scrolling MUST NOT be active when no overflow exists.

---

## 7. Clipping Rules

**INV-HX-CLIP-1**

When overflow exists:

* content MUST be clipped strictly to the viewport rect
* clipping applies uniformly to header, body, and footer

No band may escape horizontal clipping.

---

## 8. Alignment Interaction

Column alignment (left / center / right):

* applies **within column rect only**
* does not affect column positioning
* does not affect horizontal extent

Alignment never changes `W_content`.

---

## 9. Forbidden Capabilities

This SPEC explicitly forbids:

* column resizing
* drag-based scrolling
* momentum / inertial scroll tuning
* scroll snapping
* elastic bounds
* per-column overflow behavior

Any such behavior requires a separate SPEC.

---

## 10. Compatibility Guarantees

* Existing List behavior remains unchanged
* Vertical scrolling semantics are unaffected
* Header/footer remain fixed vertically
* Horizontal overflow affects all bands uniformly

---

## 11. Acceptance Criteria

This SPEC is correctly implemented if:

* Horizontal scrollbar appears only when columns overflow
* Header, body, and footer scroll together horizontally
* Removing ListView leaves List horizontal behavior unchanged
* No horizontal logic exists in List

---

## 12. Summary

SPEC-HAL-118D defines **when horizontal scrolling exists** and **who owns it**.

It does not define how scrolling feels.
It does not define how columns resize.
It defines geometry. Nothing more.
