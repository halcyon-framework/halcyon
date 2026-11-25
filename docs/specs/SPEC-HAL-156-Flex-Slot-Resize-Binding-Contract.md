# SPEC-HAL-156: Flex Slot Resize Binding Contract

**Status:** Draft
**Layer:** Layout Engine / Flex System
**Depends on:** SPEC-HAL-155
**Scope:** Flex container slot sizing, resize propagation
**Non-Goals:** Visual widgets, rendering, input routing details

---

## 1. Purpose

This SPEC binds **resizable separators / gutters** (SPEC-HAL-155) to the **Flex layout model** explicitly.

It defines how dragging a separator **mutates Flex slot sizing**, not widgets directly.

> Separators do not resize widgets.
> Separators mutate **Flex slot constraints**.
> Layout reacts mechanically.

---

## 2. Core Principle

**PRINCIPLE-FLEX-RESIZE-1**
Resizable separators operate exclusively on **Flex slot sizing state**, never on widget geometry.

All visual movement is a **result of layout recomputation**, not direct manipulation.

---

## 3. Terminology

* **Flex Slot** — A child entry in a Flex container.
* **Primary Axis** — Flex direction (row → horizontal, column → vertical).
* **Resizable Slot** — A slot whose size may be user-adjusted.
* **Gutter** — Separator bound between two adjacent slots.
* **Slot Size Model** — The declarative size rule attached to a slot.

---

## 4. Flex Slot Size Model

Each Flex slot MUST declare a size model:

```text
Fixed(px)
Flex(weight)
MinMax {
  base: px,
  min: px,
  max: px | ∞
}
```

### Invariants

* **INV-FLEX-SIZE-1:** Every slot has exactly one size model.
* **INV-FLEX-SIZE-2:** Size models are declarative, not computed ad-hoc.
* **INV-FLEX-SIZE-3:** Resizing mutates only size models, never layout results.

---

## 5. Resizable Slot Eligibility

A slot is resizable iff:

* It participates in the **primary axis**
* Its size model is `Fixed` or `MinMax`
* It is adjacent to another eligible slot

### Forbidden

* **FORBID-FLEX-RESIZE-1:** Resizing `Flex(weight)` slots directly
* **FORBID-FLEX-RESIZE-2:** Resizing across non-adjacent slots
* **FORBID-FLEX-RESIZE-3:** Resizing across axis boundaries

---

## 6. Gutter ↔ Slot Binding

A gutter binds **exactly two adjacent slots**:

```
[ Slot A ] | Gutter | [ Slot B ]
```

### Binding Rules

* **INV-GUTTER-BIND-1:** One gutter ↔ exactly two slots
* **INV-GUTTER-BIND-2:** Gutter orientation matches Flex axis
* **INV-GUTTER-BIND-3:** Gutter has no independent size authority

---

## 7. Resize Semantics

Dragging a gutter produces a **delta** along the primary axis.

That delta is applied as:

```text
Slot A.size += delta
Slot B.size -= delta
```

### Constraint Resolution Order

1. Apply delta
2. Clamp to `min` / `max`
3. Redistribute overflow back to the opposite slot
4. If both clamp → motion stops

### Invariants

* **INV-FLEX-RESIZE-1:** Total combined size of slots is preserved
* **INV-FLEX-RESIZE-2:** No slot violates min/max constraints
* **INV-FLEX-RESIZE-3:** Layout recomputation is mandatory after mutation

---

## 8. Transaction Model

Resize is **transactional**:

* Begin → capture initial slot size models
* Update → compute proposed models (pure)
* Commit → replace slot size models
* Cancel → discard proposal

### Invariants

* **INV-FLEX-RESIZE-TXN-1:** No partial commits
* **INV-FLEX-RESIZE-TXN-2:** Cancel restores exact prior state

---

## 9. Interaction with Flex Layout

After commit:

* Flex container is marked **layout-dirty**
* Next layout pass consumes updated size models
* No widget participates in resize logic

---

## 10. Non-Goals

This SPEC does **not** define:

* Pointer capture
* Hit-testing
* Rendering
* Visual affordances
* Animation

Those belong to SPEC-HAL-155 and higher interaction layers.

---

## 11. Rationale

This contract ensures:

* No geometry ownership leaks
* No widget-level hacks
* Deterministic layout behavior
* Reuse across ListView, SplitView, Panels, IDE-style layouts

Flex remains the **single source of truth**.

---

## 12. Summary

Resizable separators:

* Bind to **Flex slots**
* Mutate **slot size models**
* Trigger **layout**, not geometry
* Remain **pure, transactional, constrained**

Anything else is illegal.
