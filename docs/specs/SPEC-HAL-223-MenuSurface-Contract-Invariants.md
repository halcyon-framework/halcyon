# SPEC-HAL-223: MenuSurface Contract Invariants

**Status:** Binding  
**Layer:** Surface / Interaction Contract  
**Scope:** MenuSurface only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-222

---

## 0. Purpose

This SPEC defines **engine-grade contract invariants** for MenuSurface.

MenuSurface is the **structural and behavioral host** for menu presentation.
It is the only entity allowed to:
- host MenuItems
- manage menu layout
- manage menu lifetime and dismissal
- coordinate pointer / keyboard routing inside menus

This SPEC exists to prevent:
- MenuItem-driven behavior
- widget-level menu logic
- duplicated popup / context / dropdown mechanics

MenuSurface is the **single authority** for menus.

---

## 1. Conceptual Definition

A **MenuSurface** is a transient UI surface that:
- presents a list (or grid) of MenuItems
- optionally hosts separators and headers
- captures input while active
- resolves activation and dismissal rules

MenuSurface is **not**:
- an application controller
- a data source
- a navigation engine

It is an **interaction boundary**.

---

## Invariant Class A — Authority & Ownership

### INV-A1: Exclusive Menu Ownership

MenuSurface owns:
- MenuItem layout
- hit testing
- focus routing
- keyboard navigation
- dismissal rules

No child widget may override these.

---

### INV-A2: No Application Authority

MenuSurface must not:
- mutate application state
- interpret menu meaning
- decide what an action does

It coordinates interaction only.

---

## Invariant Class B — Composition Rules

### INV-B1: Allowed Children Only

MenuSurface may contain only:
- MenuItem
- MenuSeparator (SPEC-HAL-224)
- MenuHeader (SPEC-HAL-225)

No arbitrary widgets.

---

### INV-B2: Flat Structural Model

MenuSurface layout is **structural, not semantic**.

- No implicit hierarchy
- No recursive layout rules

Submenus are separate MenuSurfaces.

---

## Invariant Class C — Input Capture & Routing

### INV-C1: Exclusive Input Capture

While active, MenuSurface captures:
- pointer events
- keyboard events

Events do not leak to underlying UI.

---

### INV-C2: Deterministic Routing

Input routing inside MenuSurface is deterministic:
- hover → highlight
- activation → MenuItem intent emission
- outside click → dismissal

No ambiguity.

---

### INV-C3: Explicit Dismissal Rules

MenuSurface dismisses only on:
- outside pointer activation
- explicit cancel (Escape)
- successful MenuItem activation (if configured)

Dismissal policy is explicit.

---

## Invariant Class D — Keyboard Semantics

### INV-D1: Menu-Scoped Navigation

Keyboard navigation is scoped to MenuSurface:
- Arrow keys move highlight
- Enter / Space activates highlighted item
- Escape dismisses

No global shortcuts.

---

### INV-D2: No Implicit Focus Transfer

MenuSurface must not:
- steal focus permanently
- reassign focus on close implicitly

Focus restoration is explicit.

---

## Invariant Class E — Submenu Coordination

### INV-E1: Submenus Are Separate Surfaces

A submenu is:
- a new MenuSurface
- anchored to a MenuItem
- managed by MenuSurface orchestration

MenuItem never opens submenus itself.

---

### INV-E2: Single Active Path

At any time:
- one root MenuSurface
- one active submenu chain

No parallel open branches.

---

## Invariant Class F — Layout & Geometry

### INV-F1: Geometry Ownership

MenuSurface owns:
- menu width
- row height
- padding
- alignment

MenuItem does not influence geometry.

---

### INV-F2: Screen Boundary Responsibility

MenuSurface is responsible for:
- clamping to screen bounds
- flipping / repositioning if necessary

No child involvement.

---

## Invariant Class G — Animation & Time

### INV-G1: Animation Is Non-Semantic

Animation may affect:
- opacity
- transform
- appearance

It must not affect:
- routing
- timing of activation
- dismissal logic

---

### INV-G2: No Timers for Semantics

MenuSurface must not:
- delay activation
- infer hover intent via time

All semantics are event-driven.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Interaction

Given identical input sequences,
MenuSurface behavior is identical.

---

### INV-H2: Zero Allocation Steady State

No per-frame allocations during idle or hover.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:
- platform imitation
- perceived UX norms
- convenience

Extensions require a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- visual styling
- animation curves
- menu item semantics
- command routing
- application shortcuts

Those belong elsewhere.

---

## 3. Summary (Non-Normative)

- MenuSurface is the **sole menu authority**
- MenuItems are inert action rows
- Submenus are surfaces, not children
- Input capture is absolute and deterministic

This SPEC locks MenuSurface as a **strict interaction boundary**.
