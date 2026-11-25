# SPEC-HAL-118A: List v1 Scope and Non-Goals

**Status:** Deprecated
**Layer:** Widget contract (List)

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.
**Depends on:** SPEC-HAL-118, SPEC-HAL-114, SPEC-HAL-131A/B/C
**Affects:** `src/widgets/list.rs`, `src/widgets/list_item.rs`
**Audience:** Engine, Widget, and Demo authors

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion  

---

## 1. Purpose

This SPEC **explicitly constrains** the scope of the `List` widget in its first stable incarnation (**List v1**).

Its goal is to:
- eliminate ambiguity around List responsibilities,
- prevent architectural drift,
- and mechanically forbid unsupported behaviors that compromise projection correctness.

This SPEC is **normative**. Any behavior not explicitly allowed here is forbidden in List v1.

---

## 2. Definition: What List v1 Is

**List v1 is a virtualized, uniform-row, index-addressable collection widget.**

It is designed to efficiently render and interact with large, ordered collections where:
- item identity is stable,
- geometry is deterministic,
- and realization is driven strictly by viewport and scroll state.

---

## 3. Guaranteed Properties (List v1 Scope)

List v1 guarantees the following properties:

### 3.1 Uniform Item Height

- All list items share **one authoritative row height**.
- This height is:
  - either explicitly configured, or
  - derived once from the canonical ListItem layout.
- The row height is **constant across all items and frames**.

There is no per-item height variance.

---

### 3.2 Stable Logical Identity

- Items are identified by `LogicalItemId`.
- Selection, focus, hover, and input routing are keyed by logical identity.
- These states **survive realization churn** and scrolling.

Widget identity is never used as a source of truth.

---

### 3.3 Viewport-Driven Virtualization

- Realization is strictly driven by:
  - viewport geometry, and
  - scroll offset.
- Only items intersecting the viewport may be realized.
- Realization cost is bounded and independent of total item count.

---

### 3.4 Composite Content Within Rows

- A list row may contain multiple visual and interactive elements:
  - checkboxes,
  - icons,
  - labels,
  - indicators, etc.
- Inner widgets:
  - do not own selection state,
  - do not affect row geometry,
  - do not influence scroll projection.
- Hit-testing and interaction are resolved via input projection.

---

### 3.5 Deterministic Geometry

- Scroll projection geometry and paint geometry are identical.
- There are no heuristics, fallbacks, or deferred corrections.
- The list’s scroll math is exact and stable.

---

## 4. Explicit Non-Goals (Forbidden in List v1)

The following behaviors are **explicitly unsupported and forbidden** in List v1.

### 4.1 Variable-Height Items

- List v1 does **not** support per-item height variation.
- Any configuration where realized item geometry differs from the List’s row height is invalid.
- Height measurement feedback loops are not permitted.

---

### 4.2 Expandable or Collapsing Rows

- Accordion-style rows, disclosure widgets, or dynamically expanding items are not List features.
- These belong to TreeView, OutlineView, or dedicated composite widgets.

---

### 4.3 Flow-Based or Reflowing Layouts

- List is not a general scroll container.
- Wrapping, masonry, flex-flow, or responsive reflow is out of scope.

---

### 4.4 Implicit Geometry Inference

- List does not infer scroll geometry from child layout.
- Child widgets do not report geometry to influence projection.

---

### 4.5 SlotFactory Geometry Autonomy

- SlotFactory-provided widgets must conform to the List’s fixed row height.
- SlotFactory exists for **content composition**, not layout autonomy.

---

## 5. Architectural Rationale

These constraints exist to preserve the following invariants:

- Scroll projection must remain mathematically exact.
- Realization and projection must never diverge.
- Selection and input semantics must remain stable under churn.
- Performance characteristics must be predictable.

Relaxing these constraints requires a **different widget**, not an extension of List v1.

---

## 6. Forward Compatibility

This SPEC does **not** forbid future widgets that support:
- variable-height items,
- measured geometry,
- dynamic content extent.

Such functionality must be introduced under a **new widget type** or a **List v2** contract with explicit SPEC coverage.

---

## 7. Enforcement

- Implementations MUST enforce uniform row height in List v1.
- Debug builds SHOULD assert on geometry violations.
- Demos MUST NOT imply unsupported behavior.

Any violation of this SPEC is a contract breach.

---

## 8. Status

This SPEC is **binding** for all List v1 implementations.

No behavior outside this scope is permitted without a superseding SPEC.
