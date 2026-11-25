# SPEC-HAL-222: MenuItem Primitive Invariants

**Status:** Binding  
**Layer:** Visual / Action Primitive  
**Scope:** MenuItem only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the MenuItem primitive.

A MenuItem is the **atomic action unit** used by:
- PopupMenu
- ContextMenu
- DropdownMenu
- MenuSurface (SPEC-HAL-223)

This SPEC exists to prevent MenuItem from becoming:
- a container
- a controller
- a state machine
- a layout manager

MenuItem is **an addressable action row**, nothing more.

---

## 1. Conceptual Definition

A **MenuItem** represents a single actionable intent.

It may visually include:
- a label
- an optional icon
- an optional shortcut hint
- an optional disclosure indicator (submenu affordance)

It must **never**:
- own menus
- open surfaces
- manage selection
- infer hierarchy
- control navigation

MenuItem emits **exactly one explicit intent** when activated.

---

## Invariant Class A — Authority & Ownership

### INV-A1: No Structural Ownership

MenuItem does not own:
- child MenuItems
- submenus
- surfaces
- containers

Hierarchy is expressed **outside** MenuItem.

---

### INV-A2: No State Authority

MenuItem does not own:
- selected state
- checked state
- open/closed state
- hover state beyond rendering

All state is **provided** or **derived externally**.

---

## Invariant Class B — Action Semantics

### INV-B1: Single Explicit Action

A MenuItem represents **one action**.

Activation emits:
- exactly one intent
- exactly once per activation

No chaining.
No implicit follow-ups.

---

### INV-B2: No Implicit Behavior

MenuItem must not:
- close menus
- open submenus
- navigate
- mutate application state directly

It emits intent only.

---

### INV-B3: Disabled Is Absolute

When disabled:
- activation is impossible
- no intent emission
- visuals may change

---

## Invariant Class C — Interaction Semantics

### INV-C1: Explicit Activation Only

Activation occurs only via:
- pointer release
- keyboard activation (Enter / Space)

No activation on:
- hover
- focus
- pointer down

---

### INV-C2: Keyboard Parity

Keyboard and pointer activation are semantically identical.

---

## Invariant Class D — Visual Composition

### INV-D1: Fixed Visual Slots

MenuItem may expose **fixed visual slots**:
- leading icon (optional)
- primary label (mandatory)
- trailing hint / shortcut (optional)
- disclosure indicator (optional)

Slots are declarative, not arbitrary children.

---

### INV-D2: No Layout Authority

MenuItem does not decide:
- row height
- column alignment
- menu width

Those belong to MenuSurface.

---

## Invariant Class E — Hierarchy Signaling

### INV-E1: Submenu Is Declarative Only

MenuItem may be **marked** as having a submenu.

This mark:
- is visual only
- implies no behavior
- does not open anything

---

### INV-E2: No Recursive Composition

MenuItem must never contain another MenuItem.

---

## Invariant Class F — Binding & Integration

### INV-F1: Intent Emission Only

MenuItem integrates by emitting:
- an explicit action identifier
- optional payload (opaque to MenuItem)

No direct model access.

---

### INV-F2: No Surface Knowledge

MenuItem must not know:
- which surface it lives in
- whether it is a popup, dropdown, or context menu

---

## Invariant Class G — Determinism & Safety

### INV-G1: Deterministic Activation

Given identical input and state,
MenuItem emits identical intents.

---

### INV-G2: No Per-Frame Allocation

No allocations during steady hover or idle.

---

### INV-G3: Invariant Supremacy

No invariant may be violated for:
- convenience
- UX expectations
- platform mimicry

Extensions require a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- menu layout
- submenu mechanics
- separators
- selection models
- keyboard navigation policy

Those are layered elsewhere.

---

## 3. Summary (Non-Normative)

- MenuItem is **atomic**
- Emits intent, nothing else
- Owns no structure, no state, no surfaces
- Safe to reuse everywhere

This SPEC locks MenuItem as a **pure action primitive**.
