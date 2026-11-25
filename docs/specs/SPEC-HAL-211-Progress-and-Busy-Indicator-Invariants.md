# SPEC-HAL-211: Progress and Busy Indicator Invariants

**Status:** Binding  
**Layer:** Widget Contract  
**Scope:** ProgressBar / BusyIndicator controls  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for progress-reporting widgets:

- **ProgressIndicator** (determinate)
- **BusyIndicator** (indeterminate)

These widgets are **purely observational**.
They never accept input, never own timing, and never infer state.

This SPEC exists to prevent a very common failure mode:
UI elements that *pretend* to know progress, loading, or completion.

They must not.

---

## 1. Conceptual Definition

A Progress / Busy indicator is a **read-only visualization** of external state.

It may visualize:

- a scalar progress value (`0.0 .. 1.0`)
- an indeterminate “working” state

It must **never**:

- drive logic
- infer completion
- manage time
- emit events

---

## Invariant Class A — Ownership & Authority

### INV-A1: No State Authority

Progress / Busy indicators do not own progress state.

They only **render values pushed from the application layer**.

---

### INV-A2: No Async Ownership

These widgets must not:

- start tasks
- poll tasks
- await futures
- manage loaders

They reflect state. They do not manage it.

---

### INV-A3: No Implicit Transitions

Widgets must never infer transitions such as:

- “started”
- “finished”
- “failed”

All transitions are explicit inputs.

---

## Invariant Class B — Determinate Progress

### INV-B1: Explicit Progress Value

Determinate progress is represented by:

- a scalar value (typically `f32`)
- explicit domain (`0.0 .. 1.0`, or documented alternative)

No hidden normalization.

---

### INV-B2: No Clamping Without Instruction

If a value exceeds the domain:

- behavior must be explicit (clamp, wrap, ignore)
- default behavior must be documented

Silent correction is forbidden.

---

### INV-B3: Stateless Rendering

Rendering must depend **only** on the current value.
No accumulation, smoothing, or history.

---

## Invariant Class C — Indeterminate (Busy) State

### INV-C1: Busy Is Explicit

Indeterminate mode must be entered explicitly.

No inference such as:

- “value missing”
- “value unchanged for N ms”

---

### INV-C2: No Semantic Animation

Busy animation is **purely visual**.

Animation must not:

- signal progress
- imply duration
- encode completion

---

### INV-C3: Busy ≠ Progress

Busy indicators do not transition to determinate automatically.

Switching modes is an application decision.

---

## Invariant Class D — Visual & Layout Contract

### INV-D1: Non-Interactive

Progress / Busy indicators must never:

- accept focus
- receive pointer events
- respond to keyboard input

They are display-only.

---

### INV-D2: Geometry Is Self-Contained

Layout must include:

- track
- fill / activity region
- optional label

No external overlays required.

---

### INV-D3: Optional Label Is Visual Only

If a label exists:

- it is owned by the widget
- it is read-only
- it does not affect semantics

---

## Invariant Class E — Animation & Time

### INV-E1: No Time Authority

Widgets must not:

- own clocks
- schedule frames
- infer elapsed time

---

### INV-E2: Animation Is Frame-Driven Only

Any animation must be driven by:

- the global frame loop
- existing animation infrastructure

No timers inside the widget.

---

## Invariant Class F — Binding & Integration

### INV-F1: Application-Layer Feeding

Progress values are provided by the application layer.

No direct model access.
No polling.

---

### INV-F2: No Change Emission

These widgets never emit changes.

No callbacks.
No bindings.
No writers.

---

## Invariant Class G — Disabled & Visibility Semantics

### INV-G1: Disabled Is Visual Only

Disabled state affects:

- color
- opacity
- emphasis

It does not change logic (there is no logic).

---

### INV-G2: Hidden Is Silent

When hidden or removed:

- no lifecycle hooks
- no completion semantics

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Rendering

Given the same input values,
rendering output is identical.

---

### INV-H2: Zero Allocation Steady State

No per-frame allocations during idle or steady animation.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:

- “nice UX”
- “loading realism”
- “platform mimicry”

New behavior requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:

- exact visuals
- progress interpolation
- animation styles
- completion behavior

Those are layered concerns.

---

## 3. Summary (Non-Normative)

- Progress and Busy indicators are **read-only**
- They own no time, no state, no logic
- All meaning comes from the application layer
- Visual motion is never semantic

This SPEC locks them as **honest observers**, not actors.
