# SPEC-HAL-216: PopupMenu Control Invariants

**Status:** Binding  
**Layer:** Control / Transient UI  
**Scope:** PopupMenu control only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-215

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the PopupMenu control.

A PopupMenu provides **temporary, explicit choice selection**
triggered by user action, without becoming a navigation system,
state container, or behavioral authority.

This SPEC exists to prevent PopupMenus from turning into:
- mini view hierarchies
- hidden state machines
- focus traps
- “smart” context engines

PopupMenu is **ephemeral, explicit, and dumb by design**.

---

## 1. Conceptual Definition

A **PopupMenu** is a transient control that:

- appears in response to an explicit trigger
- presents a finite list of actions or options
- emits a single selection intent
- then dismisses itself

It must **never**:
- persist state
- infer intent
- coordinate application logic
- manage navigation or focus beyond itself

---

## Invariant Class A — Ownership & Authority

### INV-A1: No Application State Ownership

PopupMenu does not own:
- selected state
- toggled state
- model values

It emits **selection intent only**.

---

### INV-A2: Transient Lifetime

PopupMenu exists only while visible.

It must not:
- survive across frames invisibly
- cache prior selections
- restore state on reopen

---

## Invariant Class B — Invocation Semantics

### INV-B1: Explicit Trigger Only

PopupMenu appears only due to:
- explicit user action
- explicit application command

No hover-triggered popups.
No implicit time-based appearance.

---

### INV-B2: Single Active Instance

A PopupMenu instance is:
- self-contained
- not re-entrant
- not shared

Multiple menus require multiple instances.

---

## Invariant Class C — Selection Semantics

### INV-C1: Single Selection Emission

Each PopupMenu interaction results in:
- zero selections (dismiss)
- or exactly one selection

No multi-select.
No chained actions.

---

### INV-C2: Explicit Dismissal

PopupMenu dismisses on:
- selection
- outside click
- explicit cancel command

Dismissal emits **no selection**.

---

### INV-C3: No Sticky State

PopupMenu must not remember:
- last selection
- hovered item
- cursor position

Every appearance starts clean.

---

## Invariant Class D — Interaction & Focus

### INV-D1: Local Focus Scope

PopupMenu may temporarily capture focus,
but must release it entirely on dismissal.

No focus persistence.

---

### INV-D2: No Global Input Hijacking

PopupMenu must not:
- disable unrelated widgets
- intercept unrelated shortcuts
- alter global input routing

---

## Invariant Class E — Composition & Content

### INV-E1: Flat Item Model

PopupMenu content is:
- a flat list of items
- rendered in order

No implicit grouping.
No hierarchy.

---

### INV-E2: No Embedded Widgets

PopupMenu items are:
- labels
- icons
- separators

No buttons.
No sliders.
No text input.

(Complex content requires a different control.)

---

## Invariant Class F — Layout & Rendering

### INV-F1: Self-Contained Geometry

PopupMenu computes its own geometry
based solely on its items and theme.

No dependency on parent layout.

---

### INV-F2: No Layout Mutation

PopupMenu appearance must not:
- shift surrounding layout
- reflow parent containers

It overlays.

---

## Invariant Class G — Animation & Time

### INV-G1: Animation Is Non-Semantic

Any animation is:
- purely visual
- externally configurable
- non-authoritative

---

### INV-G2: No Timing Ownership

PopupMenu must not:
- auto-dismiss on timers
- delay selection
- infer intent from time

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Interaction

Given identical input sequences,
the same selection (or dismissal) occurs.

---

### INV-H2: Zero Allocation Steady State

No per-frame allocation during idle display.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:
- “expected UX”
- platform mimicry
- convenience

Any deviation requires a new SPEC.

---

## 2. Explicit Non-Goals

This SPEC does **not** define:
- context menus vs dropdowns
- keyboard navigation details
- accessibility roles
- submenus
- checked / radio menu items

Those require separate SPECs.

---

## 3. Summary (Non-Normative)

- PopupMenu is **transient and explicit**
- Emits exactly one intent or none
- Owns no state, no logic, no time
- Exists briefly, then disappears

This SPEC locks PopupMenu as a **clean, disposable decision surface** —
nothing more.
