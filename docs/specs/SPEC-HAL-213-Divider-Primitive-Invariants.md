# SPEC-HAL-213: Divider Primitive Invariants

**Status:** Binding  
**Layer:** Visual Primitive / Layout Helper  
**Scope:** Divider widget only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the Divider primitive.

A Divider is a **purely visual separation element**.
It exists to structure layouts visually without introducing
interaction, state, or semantics.

This SPEC exists to prevent Dividers from becoming:
- fake widgets with input handling
- layout hacks with hidden behavior
- semantic separators that imply hierarchy or navigation

Divider is **visual only**. Nothing more.

---

## 1. Conceptual Definition

A **Divider** is a thin visual line or gap that separates regions
in a layout.

It may be:
- horizontal
- vertical

It must **never**:
- accept input
- receive focus
- emit events
- carry semantic meaning

---

## Invariant Class A — Ownership & Authority

### INV-A1: No State Ownership

Divider owns **no mutable state**.

No flags, no toggles, no configuration mutation after construction.

---

### INV-A2: No Semantic Authority

Divider does not represent:
- grouping
- hierarchy
- section boundaries
- navigation structure

It is **visual only**.

---

## Invariant Class B — Interaction Semantics

### INV-B1: Non-Interactive

Divider must never:
- accept pointer events
- accept keyboard events
- be focusable
- participate in hit-testing

---

### INV-B2: Input Transparency

Input events must pass **through** the Divider.

Divider must not block interaction with widgets behind or around it.

---

## Invariant Class C — Layout Contract

### INV-C1: Explicit Orientation

Divider has an explicit orientation:
- horizontal **or**
- vertical

No auto-detection.

---

### INV-C2: Theme-Driven Geometry

Divider thickness, length behavior, and spacing are
**entirely theme-controlled**.

No hardcoded pixel values in widget code.

---

### INV-C3: Constraint-Respecting

Divider respects layout constraints:
- expands along its main axis if allowed
- uses intrinsic thickness on its minor axis

---

## Invariant Class D — Visual Rendering

### INV-D1: Stateless Rendering

Rendering depends **only** on:
- orientation
- theme geometry
- theme colors

No animation.
No interpolation.
No state memory.

---

### INV-D2: No Decoration Stack Abuse

Divider must not:
- use focus styles
- use hover styles
- simulate surfaces or controls

It is a line, not a control.

---

## Invariant Class E — Determinism & Safety

### INV-E1: Deterministic Output

Given identical layout constraints and theme,
Divider renders identically across frames.

---

### INV-E2: Zero Allocation Steady State

Divider must not allocate during layout or paint.

---

### INV-E3: Invariant Supremacy

No invariant may be violated for:
- aesthetics
- convenience
- “expected behavior”

Any extension requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- dashed lines
- labeled separators
- collapsible dividers
- interactive splitters

Those are separate primitives.

---

## 3. Summary (Non-Normative)

- Divider is a **visual separator only**
- No input, no state, no semantics
- Theme-driven, deterministic, inert

This SPEC locks Divider as a **safe layout primitive**.
