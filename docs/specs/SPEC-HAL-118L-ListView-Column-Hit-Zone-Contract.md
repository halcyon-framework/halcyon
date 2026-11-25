# SPEC-HAL-118L: ListView Column Hit-Zone Contract

**Status:** Deprecated
**Layer:** ListView / Header Interaction Geometry

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
* SPEC-HAL-118F – ListView Column Resize Contract
* SPEC-HAL-118H – ListView Sort Affordance Contract
* SPEC-HAL-118J – ListView Column Separator Model
* SPEC-HAL-009 – Event Routing & Hit-Testing
* SPEC-HAL-128 – Pointer Capture & Gesture Arbitration

---

## 1. Purpose

This SPEC defines **exact, non-ambiguous hit-zones** for ListView column headers.

Its purpose is to prevent interaction conflicts between:

* column resize
* sort affordance
* separator interaction
* inert header regions

Without this contract, later SPECs silently conflict at runtime.

This SPEC is **foundational**.
All ListView header interactions MUST conform to it.

---

## 2. Scope

This SPEC applies **only** to the ListView **HeaderBand**.

It explicitly does **not** affect:

* body rows
* List selection semantics
* scrolling behavior
* virtualization

---

## 3. Definitions

### 3.1 Header Cell Rect

For each column `C`, the header cell rect is defined as:

* x = column.left
* y = header.top
* width = column.width
* height = header.height

This rectangle is purely geometric and MUST be computable deterministically.

---

### 3.2 Resize Gutter Rect

Each column MAY expose a resize gutter.

The resize gutter rect is defined as:

* x = column.right − (RESIZE_GUTTER_WIDTH / 2)
* y = header.top
* width = RESIZE_GUTTER_WIDTH
* height = header.height

Constraints:

* RESIZE_GUTTER_WIDTH MUST be ≥ 6 px
* The gutter MUST be symmetric around the column boundary

Resize gutters are **exclusive capture zones**.

---

### 3.3 Separator Hit Rect

If column separators are enabled (SPEC-HAL-118J):

* Separator hit rect MUST be identical to the resize gutter rect

Separators MUST NOT introduce independent hit zones.
They reuse the resize gutter interaction area.

---

### 3.4 Sort Hit Rect

The sort hit rect for a column is defined as:

* HeaderCellRect(C) minus ResizeGutterRect(C)

Sorting is allowed **only** inside this remaining header area.

---

## 4. Hit-Zone Priority Order (MANDATORY)

If hit-zones overlap, resolution MUST follow this exact priority order:

1. Resize gutter
2. Separator
3. Sort affordance
4. Header background (inert)

This order is **absolute** and non-negotiable.

---

## 5. Interaction Outcomes

| Hit Zone          | Result                              |
| ----------------- | ----------------------------------- |
| Resize Gutter     | Begin resize capture (SPEC-HAL-118F) |
| Separator         | Alias of resize behavior            |
| Sort Hit Rect     | Emit sort intent (SPEC-HAL-118H)     |
| Header Background | Consume event, no action            |

The header background MUST NEVER:

* start selection
* initiate scrolling
* activate body-level logic

---

## 6. Pointer Capture Rules

* Resize gutter MUST capture pointer on PointerDown
* Capture MUST persist until:

    * PointerUp
    * Cancellation
    * Escape

Sort interactions MUST NOT capture pointer.

---

## 7. Forbidden Behavior

The following behaviors are **architectural violations**:

* Resize initiated outside the resize gutter
* Sort triggered from within the resize gutter
* Header click affecting List body selection
* Multiple simultaneous header interaction modes
* Gesture resolution based on registration order or heuristics

Violations produce undefined behavior.

---

## 8. Invariants

**INV-HITZONE-1**
Header hit-zones MUST be deterministic and pixel-exact.

**INV-HITZONE-2**
Resize zones have absolute priority over all other header interactions.

**INV-HITZONE-3**
Sort affordance MUST NOT overlap resize gutters.

**INV-HITZONE-4**
Header interactions MUST NOT propagate to the List body.

---

## 9. Relationship to Other SPECs

* SPEC-HAL-118F defines *what resizing means*; this SPEC defines *where resizing begins*
* SPEC-HAL-118H defines *sorting semantics*; this SPEC defines *where sorting is allowed*
* SPEC-HAL-118J defines *separator visuals*; this SPEC defines *their interaction behavior*
* SPEC-HAL-118M (Header Interaction Routing) builds on this SPEC

---

## 10. Rationale

Table UIs historically fail due to implicit hit-testing rules.

This SPEC makes header interaction geometry:

* explicit
* measurable
* enforceable

No heuristics.
No guessing.
No interaction roulette.

---

## 11. Non-Goals

This SPEC does not define:

* visual styling
* keyboard navigation
* touch-specific affordances
* accessibility semantics (future SPEC)

---

## 12. Summary

This SPEC freezes ListView header hit geometry as **law**.

All future header behavior is built **on top of it**, never around it.

---

## Implementation Notes (ListView v1)

Hit-zones are implemented via `ListView::hit_test_column()` which returns both column index and separator proximity. The resize gutter width is defined by `RESIZE_ZONE_HALF_WIDTH`. Priority order (resize > separator > sort > inert) is enforced in event handling.

---

## Implementation Reference

- `ListView::hit_test_column()` — column and separator hit detection
- `RESIZE_ZONE_HALF_WIDTH` — resize gutter half-width constant
- `HitTestColumnResult` — result type with column index and separator info
