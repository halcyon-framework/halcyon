# SPEC-HAL-118G: ListView Column Reflow Rules

**Status:** Deprecated
**Layer:** Layout Resolution / Reflow Semantics

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---
**Scope:** Column positioning and reflow rules for ListView
**Depends on:** SPEC-HAL-118F
**Enables:** SPEC-HAL-118H, SPEC-HAL-118K
**Non-Goals:** Animations, gestures, snapping, persistence, column hiding

---

## 1. Purpose

This SPEC defines **how columns are positioned and reflowed horizontally** inside `ListView`.

It formalizes:

* column ordering
* column X-position computation
* reflow behavior on resize and extent change
* alignment guarantees across header, body, and footer

This SPEC is strictly about **layout math and invariants**, not interaction.

---

## 2. Core Principle

**Column reflow is deterministic and cumulative.**

Column positions are:

* resolved left-to-right
* based solely on preceding column widths
* independent of scroll offset and rendering order

---

## 3. Column Order Contract

**INV-COL-ORDER-1**

Column order is:

* exactly the order defined in `ListView` column list
* immutable at runtime (no reordering in this SPEC)

Header, body, and footer MUST use the same order source.

---

## 4. Reflow Coordinate Model

### 4.1 Coordinate Space

All column layout is resolved in **content space**, not viewport space.

Let:

* `X0 = 0`
* `Wi` = resolved width of column `i`

Then:

```
Xi = sum(W0 .. W(i-1))
```

---

### 4.2 Column Rect Resolution

**INV-COL-REFLOW-1**

For each column `i`, its horizontal rect is:

* `x = Xi`
* `width = Wi`

This applies identically to:

* header cells
* body cells
* footer cells

---

## 5. Reflow Triggers

Reflow MUST occur when any of the following change:

* column width (resize)
* column list structure (future SPECs)
* ListView horizontal extent invalidation

Reflow MUST NOT occur on:

* scroll offset change
* selection change
* focus change

---

## 6. Interaction with Horizontal Scrolling

### 6.1 Scroll Independence

**INV-COL-REFLOW-2**

Reflow computation MUST be independent of scroll offset.

Scroll offset is applied **after** reflow during painting or traversal.

---

### 6.2 Overflow Handling

If total column width exceeds viewport width:

* horizontal overflow is reported
* scrollbars may appear per SPEC-HAL-118E
* column positions remain unchanged in content space

---

## 7. Alignment Guarantees

**INV-COL-ALIGN-1**

At any time:

* header column X positions == body column X positions == footer column X positions

Any deviation is a critical invariant violation.

---

## 8. No Implicit Normalization

The following are forbidden:

* auto-stretching columns to fill viewport
* proportional scaling of columns on resize
* snapping total width to viewport width
* rounding-based drift between bands

All widths and positions MUST be exact sums of declared widths.

---

## 9. Stability Guarantees

**INV-COL-STABILITY-1**

Given identical:

* column definitions
* column widths
* viewport size

Reflow MUST produce identical column positions across frames.

No frame-dependent jitter is allowed.

---

## 10. State Exposure

ListView MUST be able to report:

* per-column resolved width
* per-column resolved X offset
* total horizontal content width

This state is read-only to consumers.

---

## 11. Failure Modes

The following indicate a broken implementation:

* header columns drift relative to body
* footer columns clip while body scrolls
* column resize causes lateral jitter
* scroll offset affects column X positions

Any of these violate this SPEC.

---

## 12. Acceptance Criteria

This SPEC is satisfied if:

* column X positions are cumulative and deterministic
* resizing a column reflows all subsequent columns correctly
* header, body, and footer remain perfectly aligned
* horizontal scrolling applies after reflow, not during it

---

## 13. Summary

SPEC-HAL-118G locks down **column reflow as pure layout math**.

It ensures:

* deterministic positioning
* perfect cross-band alignment
* strict separation between layout and scrolling

This SPEC is a prerequisite for any advanced column features.
