# SPEC-HAL-221: SeparatorGroup Layout Helper Invariants

**Status:** Binding  
**Layer:** Layout Helper / Visual Composition  
**Scope:** SeparatorGroup only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-213, SPEC-HAL-214

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the SeparatorGroup layout helper.

A SeparatorGroup provides a **consistent, declarative way** to arrange
visual separators (e.g. Dividers) between sibling elements without
introducing semantics, ownership, or control flow.

This SPEC exists to prevent separator logic from:
- leaking into application code
- being reimplemented ad-hoc
- turning into containers with hidden behavior

SeparatorGroup is **layout assistance only**.

---

## 1. Conceptual Definition

A **SeparatorGroup** is a layout helper that:

- arranges a sequence of peer elements
- inserts visual separators between them
- applies theme-consistent spacing rules

It must **never**:
- own or manage child semantics
- introduce interaction
- imply hierarchy or grouping authority

---

## Invariant Class A — Authority & Ownership

### INV-A1: No Content Ownership

SeparatorGroup does not own content semantics.

Children remain:
- independent widgets
- fully responsible for their own behavior

---

### INV-A2: No Structural Authority

SeparatorGroup does not define:
- hierarchy
- containment semantics
- grouping identity

It is not a logical container.

---

## Invariant Class B — Separator Semantics

### INV-B1: Separators Are Visual Only

Separators inserted by SeparatorGroup:

- are inert
- are non-interactive
- carry no semantic meaning

They must conform to SPEC-HAL-213.

---

### INV-B2: No Leading or Trailing Separators

By default, SeparatorGroup must:

- insert separators **only between items**
- never before the first
- never after the last

Any deviation requires explicit configuration.

---

### INV-B3: Deterministic Insertion

Given the same child list and configuration,
separator placement is identical across runs.

---

## Invariant Class C — Layout Behavior

### INV-C1: Orientation-Driven

SeparatorGroup operates along a single axis:

- horizontal **or**
- vertical

Cross-axis layout is delegated to underlying layout primitives.

---

### INV-C2: Theme-Driven Spacing

Spacing between items and separators:

- must be derived from theme geometry
- must not be hardcoded

---

### INV-C3: No Child Measurement Authority

SeparatorGroup must not:
- reinterpret child sizes
- override child constraints
- enforce equal sizing

---

## Invariant Class D — Interaction & Events

### INV-D1: Event Transparency

SeparatorGroup must not:
- intercept events
- alter event routing
- synthesize input behavior

Events flow directly to children.

---

### INV-D2: Non-Focusable

SeparatorGroup itself is never focusable.

---

## Invariant Class E — Composition Rules

### INV-E1: Helper, Not Widget Authority

SeparatorGroup may be implemented as:
- a thin widget wrapper **or**
- a layout helper abstraction

But it must never grow behavior beyond layout assistance.

---

### INV-E2: Compatible With Visual Primitives

SeparatorGroup must compose cleanly with:
- Divider (SPEC-HAL-213)
- SectionHeader (SPEC-HAL-214)
- MenuItem / MenuSurface (SPEC-HAL-222 / 223)

No special casing.

---

## Invariant Class F — Determinism & Safety

### INV-F1: Deterministic Layout

Given identical inputs and theme,
layout output is identical.

---

### INV-F2: Zero Allocation Steady State

No per-frame allocations during layout or paint.

---

### INV-F3: Invariant Supremacy

No invariant may be violated for:
- convenience
- reduced verbosity
- stylistic preference

Extensions require a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- visual styles of separators
- interactive grouping
- collapsible behavior
- semantic sections

Those are layered constructs.

---

## 3. Summary (Non-Normative)

- SeparatorGroup standardizes separator layout
- It is visual glue, not structure
- No semantics, no interaction, no ownership

This SPEC locks SeparatorGroup as a **pure layout helper**, nothing more.
