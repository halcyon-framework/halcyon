# SPEC-HAL-215: CollapsibleSection Controller Invariants

**Status:** Binding  
**Layer:** Controller / Composite Behavior  
**Scope:** CollapsibleSection controller only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-214

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the `CollapsibleSection` controller.

A CollapsibleSection coordinates **visibility of content** under explicit user control,
without becoming a layout primitive, widget container, or semantic authority.

This SPEC exists to prevent collapsible constructs from degenerating into:
- implicit layout engines
- animation-driven state machines
- accordion-style global coordinators
- widgets that “know” too much

CollapsibleSection is a **controller**, not a widget primitive.

---

## 1. Conceptual Definition

A **CollapsibleSection** is a controller that:

- owns a boolean expanded/collapsed state
- exposes that state explicitly
- applies visibility to associated content
- delegates all rendering to other widgets

It must **never**:
- invent layout semantics
- animate implicitly
- coordinate with sibling sections
- infer user intent

---

## Invariant Class A — Ownership & Authority

### INV-A1: Controller, Not Primitive

CollapsibleSection is **not**:
- a widget
- a visual primitive
- a layout container

It does not paint.
It does not measure.

---

### INV-A2: Explicit State Ownership

The controller owns exactly one piece of state:

```rust
expanded: bool,
```

No derived, inferred, or time-based state is allowed.

---

### INV-A3: No Global Coordination

A CollapsibleSection must never:
- know about other sections
- auto-collapse siblings
- enforce “only one open” behavior

Accordion behavior is explicitly **out of scope**.

---

## Invariant Class B — Interaction Semantics

### INV-B1: Explicit Toggle Only

State changes occur only via:
- explicit user action
- explicit application command

No automatic transitions.

---

### INV-B2: No Implicit Input Handling

The controller itself does not:
- receive pointer events
- receive keyboard events
- manage focus

Input handling belongs to external widgets (e.g. buttons).

---

## Invariant Class C — Composition Contract

### INV-C1: External Header Widget

The header (label, button, icon, etc.) is:
- an external widget
- composed by the application
- wired to the controller explicitly

The controller does not own header visuals.

---

### INV-C2: External Content Ownership

The controlled content:
- is owned by the application
- is not a child of the controller
- is shown/hidden via explicit wiring

---

### INV-C3: Visibility, Not Layout

The controller affects:
- visibility
- participation in layout flow (shown vs hidden)

It does **not**:
- resize content
- animate height
- interpolate geometry

---

## Invariant Class D — Animation & Time

### INV-D1: No Animation Ownership

CollapsibleSection must not:
- own animations
- schedule transitions
- interpolate values

If animation is desired, it must be:
- external
- layered
- explicitly wired

---

### INV-D2: Instantaneous State Transitions

Expanded ↔ collapsed transitions are **instant** at the controller level.

Any perceived smoothness is not semantic.

---

## Invariant Class E — Binding & Integration

### INV-E1: Application-Layer Control

The expanded state may be:
- bound to application state
- driven by a FormBinder
- controlled programmatically

No internal persistence.

---

### INV-E2: No Side Effects

Changing expanded state must not:
- scroll
- focus
- reparent widgets
- trigger navigation

---

## Invariant Class F — Determinism & Safety

### INV-F1: Deterministic Behavior

Given the same sequence of toggle commands,
the visible state is identical.

---

### INV-F2: Zero Allocation Steady State

No per-frame allocation in idle or toggling.

---

### INV-F3: Invariant Supremacy

No invariant may be violated for:
- UX polish
- animation smoothness
- platform imitation

Any extension requires a new SPEC.

---

## 2. Explicit Non-Goals

This SPEC does **not** define:
- accordion behavior
- animated expand/collapse
- disclosure triangles
- keyboard navigation rules
- accessibility roles

Those are separate layers.

---

## 3. Summary (Non-Normative)

- CollapsibleSection is a **controller**
- It owns exactly one boolean
- It coordinates visibility only
- No animation, no layout authority, no global knowledge

This SPEC locks CollapsibleSection as a **minimal, explicit control surface** —
powerful because it refuses to be clever.
