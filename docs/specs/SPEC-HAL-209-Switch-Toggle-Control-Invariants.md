# SPEC-HAL-209: Switch / Toggle Control Invariants

**Status:** Binding  
**Layer:** Widget Contract  
**Scope:** Switch / Toggle control only  
**Ground Truth:** SPEC-HAL-111, SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208

---

## 0. Purpose

This SPEC defines **engine-grade binding invariants** for a Switch / Toggle control.

The Switch is a **semantic sibling of Checkbox**, not a visual variant.
It exists to represent **binary state with continuous affordance** (sliding / on-off),
while preserving the same binding, ownership, and determinism laws.

This SPEC specifies **what must never change**, regardless of animation,
theme, platform, or future extensions.

No implementation details beyond invariant-relevant structure.

---

## 1. Conceptual Definition

A **Switch** is a binary control with:

- exactly two semantic states: `on` / `off`
- a **continuous visual affordance** (track + thumb)
- **explicit label ownership** (same model as SPEC-HAL-208)

The Switch **does not introduce new state semantics** beyond Checkbox.
It introduces **new visual language only**.

---

## Invariant Class A — State & Ownership

### INV-A1: Binary State Only

Switch state is strictly binary.

- Allowed: `true | false`
- Forbidden: tristate, indeterminate, partial, transitional states

Visual animation does not imply intermediate semantic states.

---

### INV-A2: No Synthetic State

Switch must never invent semantic state for:

- animation progress
- gesture position
- drag offset
- hover interpolation

All semantic state derives from a single boolean.

---

### INV-A3: Label Ownership

Switch **owns its label directly**.

- Label is not a child widget
- Label is not a sibling widget
- Label is not externally composed

This mirrors SPEC-HAL-208 (Checkbox / Radio).

---

## Invariant Class B — Input Semantics

### INV-B1: Explicit Commit Only

State changes occur **only** via explicit user intent:

- Pointer release inside control bounds
- Keyboard activation (Space / Enter)

No state change on:

- pointer down
- hover
- drag without release
- animation completion

---

### INV-B2: Drag Is Intent, Not State

Dragging the thumb is an **interaction affordance**, not a state mutation.

- Drag may preview movement
- State flips **only on release**

---

### INV-B3: Keyboard Parity

Keyboard activation must be semantically identical to pointer activation.

- Space / Enter toggles
- Focused + disabled → no-op

---

## Invariant Class C — Visual & Layout Contract

### INV-C1: Label-Aware Geometry

Layout size must account for:

- switch track
- label gap
- label text metrics

No overlap. No clipping. No hardcoded spacing.

---

### INV-C2: Baseline-Aligned Label

Label text must be baseline-aligned using:

- font ascent / descent
- control baseline rules (SPEC-HAL-092 / 208)

---

### INV-C3: Full-Area Hit Testing

Entire control area (track + label) is hit-testable.

Clicking the label toggles the switch.

---

## Invariant Class D — Animation & Time

### INV-D1: Animation Is Non-Semantic

Animation must never:

- alter logical state
- gate input
- delay commits
- introduce timing dependency

Animation is **purely visual**.

---

### INV-D2: No Internal Timers

Switch widget must not:

- own timers
- own clocks
- measure time
- debounce internally

All timing lives above the widget layer.

---

## Invariant Class E — Disabled & Focus Semantics

### INV-E1: Disabled Is Absolute

When disabled:

- No state changes
- No drag preview
- No keyboard activation

Visual affordance may change, semantics must not.

---

### INV-E2: Focus Is Visual Only

Focus determines keyboard eligibility. Visual emphasis is recipe-defined under
SPEC-HAL-208 INV-D3, including JC's 2026-10-24 default-selector exception: the
default Switch has no additional enclosing focus ornament.

Focus must never mutate state or be equated with the binary value.

---

## Invariant Class F — Binding & Integration

### INV-F1: Binding Parity with Checkbox

Switch must integrate with the **same binding pipeline** as Checkbox:

- Emits via `ChangeWriter::set_bool`
- Consumed via `FormBinder` / `BindingContext`
- Exactly-once semantics per bind pass

---

### INV-F2: No Direct Model Access

Switch must never:

- read application model
- store references to model data
- synchronize state internally

All state sync happens at application layer.

---

## Invariant Class G — Determinism & Safety

### INV-G1: Deterministic Replay

Given identical event sequences and initial state,
Switch behavior must be identical across runs.

---

### INV-G2: No Per-Frame Allocation

Steady-state interaction must not allocate.

---

### INV-G3: Invariant Supremacy

No invariant may be violated to:

- “improve UX”
- “simplify animation”
- “match platform behavior”

New behavior requires new SPECs.

---

## 2. Non-Goals (Explicit)

This SPEC **does not define**:

- visual style
- animation curves
- thumb geometry
- platform imitation
- accessibility narration

Those are layered above this contract.

---

## 3. Summary (Non-Normative)

- Switch is **semantically identical to Checkbox**
- Visual continuity does not imply semantic continuity
- Label ownership and binding rules are identical to SPEC-HAL-208
- Animation is decoration, never logic

This SPEC locks the Switch into a **safe, deterministic, composable primitive**.
