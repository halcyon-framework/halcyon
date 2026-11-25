# SPEC-HAL-217: ContextMenu Control Invariants

**Status:** Binding  
**Layer:** Control / Transient UI  
**Scope:** ContextMenu control only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-216

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the ContextMenu control.

A ContextMenu is a **pointer-invoked, contextual decision surface**.
It exposes actions relevant to a specific target **at a specific moment**,
without becoming a navigation system, editor, or semantic authority.

This SPEC exists to prevent ContextMenus from turning into:
- hidden application controllers
- persistent state holders
- shortcut multiplexers
- implicit permission or policy engines

ContextMenu is **situational, transient, and explicit**.

---

## 1. Conceptual Definition

A **ContextMenu** is a transient menu that:

- is invoked by a contextual gesture (e.g. secondary click)
- is anchored to a target or pointer position
- presents a finite list of context-relevant actions
- emits **at most one explicit selection intent**
- dismisses immediately after interaction

It must **never**:
- own application state
- infer context beyond what is explicitly provided
- persist visibility or memory across invocations

---

## Invariant Class A — Ownership & Authority

### INV-A1: No State Ownership

ContextMenu must not own or retain:
- selection state
- toggle state
- application data
- permission decisions

It emits **intent only**.

---

### INV-A2: Explicit Context Injection

All contextual meaning (target, scope, meaning) must be:

- provided explicitly at construction or invocation
- immutable for the lifetime of the menu instance

ContextMenu must not query or discover context on its own.

---

## Invariant Class B — Invocation Semantics

### INV-B1: Contextual Gesture Only

ContextMenu appears only via:
- explicit contextual input (e.g. right-click, long-press)
- explicit programmatic invocation

No hover-triggered appearance.
No time-based inference.

---

### INV-B2: Anchor Fidelity

ContextMenu must be anchored to:
- the invocation point, or
- an explicitly provided anchor rect

It must not re-anchor dynamically.

---

### INV-B3: One Invocation, One Instance

Each invocation creates a new ContextMenu instance.

No reuse.
No pooling.
No resurrection.

---

## Invariant Class C — Selection & Dismissal

### INV-C1: Single Selection or None

A ContextMenu interaction results in:
- exactly one emitted selection, or
- no selection (dismiss)

Never more.

---

### INV-C2: Explicit Dismissal Rules

ContextMenu dismisses on:
- item selection
- click outside
- escape / cancel command
- loss of context target

Dismissal emits **no selection**.

---

### INV-C3: No Sticky Memory

ContextMenu must not remember:
- previous selections
- hovered items
- open history

Every invocation starts clean.

---

## Invariant Class D — Interaction & Focus

### INV-D1: Localized Focus Scope

ContextMenu may temporarily capture focus
**only while visible**.

All focus must be fully released on dismissal.

---

### INV-D2: No Global Input Hijack

ContextMenu must not:
- block unrelated shortcuts
- disable unrelated widgets
- alter global input routing

Its scope is strictly local.

---

## Invariant Class E — Composition Model

### INV-E1: Flat Item List

ContextMenu content is a flat list of items.

No implicit hierarchy.
No nested submenus.
No grouping logic.

---

### INV-E2: Item Simplicity

Allowed item content:
- label
- optional icon
- optional separator

Forbidden:
- sliders
- text input
- toggles
- arbitrary widgets

(Those require separate controls.)

---

## Invariant Class F — Layout & Rendering

### INV-F1: Overlay-Only Rendering

ContextMenu must render as an overlay.

It must not:
- reflow parent layout
- reserve layout space
- affect geometry of other widgets

---

### INV-F2: Self-Contained Geometry

Size and shape depend only on:
- items
- theme
- explicit constraints

Never on surrounding layout.

---

## Invariant Class G — Animation & Time

### INV-G1: Animation Is Decorative

Any animation:
- is visual only
- does not delay interaction
- does not encode meaning

---

### INV-G2: No Time Authority

ContextMenu must not:
- auto-dismiss on timers
- infer intent from dwell time
- delay selection

All timing decisions are external.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Behavior

Given identical inputs and context,
the same selection or dismissal occurs.

---

### INV-H2: Zero Allocation Steady State

No per-frame allocation while visible.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:
- platform conventions
- “expected UX”
- convenience

Any extension requires a new SPEC.

---

## 2. Explicit Non-Goals

This SPEC does **not** define:
- keyboard navigation details
- accessibility semantics
- submenus
- checkable / radio items
- command routing logic

Those require separate SPECs.

---

## 3. Summary (Non-Normative)

- ContextMenu is **contextual, transient, and explicit**
- Emits exactly one intent or none
- Owns no state, no time, no logic
- Appears briefly, then disappears

This SPEC locks ContextMenu as a **situational action surface**, not a controller.
