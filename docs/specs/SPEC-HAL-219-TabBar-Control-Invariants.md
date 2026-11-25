# SPEC-HAL-219: TabBar Control Invariants

**Status:** Binding  
**Layer:** Control / Navigation Surface  
**Scope:** TabBar control only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the TabBar control.

A TabBar is a **navigation selector** between mutually exclusive views.
It is not a container, not a layout manager, and not a content owner.

This SPEC exists to prevent TabBars from turning into:
- layout controllers
- content switches with hidden logic
- stateful view managers

TabBar **selects**, it does not **own**.

---

## 1. Conceptual Definition

A **TabBar** is a control that:

- presents a set of labeled tabs
- allows exactly one tab to be active at a time
- emits an explicit **tab selection intent**
- does not manage the content associated with tabs

TabBar **does not render or control tab content**.

---

## Invariant Class A — Ownership & Authority

### INV-A1: No Content Ownership

TabBar does not:
- own views
- mount widgets
- show or hide content

Content switching is handled externally.

---

### INV-A2: No Layout Authority

TabBar does not:
- position content
- reserve content space
- perform transitions

It occupies **only its own geometry**.

---

## Invariant Class B — Selection Semantics

### INV-B1: Single Active Tab

At most **one tab** is active at any time.

No multi-selection.
No partial activation.

---

### INV-B2: Explicit Selection Emission

Selecting a tab emits:
- a single, explicit selection intent
- identifying the selected tab

No implicit re-selection.
No auto-advance.

---

### INV-B3: No Selection Memory

TabBar must not:
- remember previous selections
- restore last state automatically
- synchronize selection internally

Selection persistence is application-owned.

---

## Invariant Class C — Input & Interaction

### INV-C1: Explicit Activation

Tab selection occurs only via:
- pointer activation
- keyboard activation

No hover-based switching.

---

### INV-C2: Keyboard Parity

Keyboard navigation must mirror pointer behavior:
- navigate tabs
- activate selection

---

### INV-C3: Re-Selecting Active Tab Is No-Op

Activating the already active tab:
- emits no event
- performs no action

---

## Invariant Class D — Visual & Layout Contract

### INV-D1: Tabs Are Visual Labels

Each tab consists of:
- a label (text or icon)
- an active/inactive visual state

Tabs are not buttons with side effects.

---

### INV-D2: Geometry Is Self-Contained

TabBar layout accounts only for:
- tabs
- internal spacing
- theme-defined padding

It does not depend on content size.

---

### INV-D3: Overflow Is Explicit

If tabs exceed available width:
- behavior must be explicit (scroll, wrap, clip)
- no implicit shrinking or hiding

---

## Invariant Class E — Disabled & Availability Semantics

### INV-E1: Disabled Tab Is Inert

Disabled tabs:
- cannot be activated
- emit no selection

---

### INV-E2: Disabled TabBar Is Absolute

When the TabBar is disabled:
- no selection changes
- no input handling

---

## Invariant Class F — Binding & Integration

### INV-F1: Selection Emission Only

TabBar emits selection intents only.

It must not:
- modify models directly
- perform routing
- bind values implicitly

---

### INV-F2: No Form Binding

TabBar must not integrate with FormBinder.

Navigation ≠ form input.

---

## Invariant Class G — Determinism & Safety

### INV-G1: Deterministic Selection

Given the same inputs,
the same tab is selected.

---

### INV-G2: No Per-Frame Allocation

Idle and steady interaction must not allocate.

---

### INV-G3: Invariant Supremacy

No invariant may be violated for:
- UX polish
- platform mimicry
- convenience

New behavior requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- tab content rendering
- transitions or animations
- swipe navigation
- routing or history integration

Those belong to higher-level systems.

---

## 3. Summary (Non-Normative)

- TabBar selects **which view**, not **what to render**
- No content ownership, no layout authority
- Explicit, stateless selection emission

This SPEC locks TabBar as a **pure navigation selector**, nothing more.
