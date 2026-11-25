# SPEC-HAL-021-A: Pressed Visual Phase (Button Only)

**Status:** Draft / Binding Amendment
**Applies to:** SPEC-HAL-021 (Generic Animation Timeline)
**Scope:** Visual feedback only
**Initial Target Widget:** Button
**Non-goals:** Input semantics, gesture arbitration, state transitions

---

## 1. Motivation

Hover animations are continuous, reversible, and non-authoritative.
Click interactions are **authoritative** and **discrete**.

To provide responsive visual feedback for pointer press without violating input semantics, we introduce a **Pressed Visual Phase** — a strictly visual, transient effect decoupled from click meaning.

This amendment defines **what is allowed** and **what is forbidden** when expressing press feedback.

---

## 2. Definition: Pressed Visual Phase

The **Pressed Visual Phase** is a short-lived visual state that:

- Begins on **primary PointerDown**
- Ends on:
    - primary PointerUp
    - PointerCancel
    - PointerLeave (bounds exit)
- Never delays, blocks, or conditions click dispatch
- Never affects widget logic or state transitions
- Never persists beyond the press interaction

It is **not** a “click animation”.

---

## 3. Architectural Placement

Pressed Visual Phase is:

- **Visual-only**
- **Widget-owned (Button only, initially)**
- **Capability-driven**
- **Optional**
- **Best-effort**

It does **not** introduce new routing phases.

It does **not** change EventRouter policy.

It does **not** introduce new TransitionLayer semantics.

---

## 4. Capability Rules

### 4.1 Capability Source

- Capability is exposed via existing `EventContext`
- Capability presence is **not guaranteed**
- Absence must be handled as a silent no-op

### 4.2 Injection Policy (Strict)

| Event Type            | Capability Allowed |
|-----------------------|--------------------|
| PointerDown (primary) | YES (visual only)  |
| PointerUp (primary)   | YES (visual only)  |
| PointerMove           | NO                 |
| KeyDown / KeyUp       | NO                 |
| Programmatic Click    | NO                 |

No other events may receive this capability.

---

## 5. Invariants (Binding)

### PV-INV-1 — No Semantic Coupling
Pressed visuals must not affect click logic, focus, activation, or command dispatch.

### PV-INV-2 — No Temporal Authority
Pressed visuals must not delay, defer, or gate input processing.

### PV-INV-3 — Always Reversible
Pressed visuals must end deterministically on PointerUp, Cancel, or Leave.

### PV-INV-4 — Visual-Only State
No widget logic may branch on the presence or absence of pressed visuals.

### PV-INV-5 — Button-Only (Initial)
Only Button may implement this phase until explicitly extended by SPEC.

---

## 6. Animation Constraints

If animation is used:

- Maximum duration: **≤ 100ms**
- Easing must be monotonic (no overshoot)
- Must tolerate frame drops
- Must resolve instantly if capability disappears

Animation is optional; instant visual changes are allowed.

---

## 7. Explicit Non-Goals

This amendment does **not**:

- Add “click animations”
- Modify EventRouter architecture
- Introduce gesture semantics
- Affect accessibility activation paths
- Apply to keyboard activation
- Apply to long-press or double-click

---

## 8. Forward Compatibility

This amendment intentionally leaves room for:

- Gesture-based press feedback
- Accessibility-aligned pressed visuals
- Themed press responses

Any extension requires a new SPEC amendment.

---

## 9. Summary

The Pressed Visual Phase:

- Is **not** an animation of a click
- Is **not** input authority
- Is **not** required for correctness
- Exists solely to improve perceived responsiveness

This preserves Halcyon’s core principle:

> **Input decides reality.
> Animation expresses perception.**
