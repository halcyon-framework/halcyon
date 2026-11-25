# SPEC-HAL-224: DisclosureIndicator Primitive Invariants

**Status:** Binding  
**Layer:** Visual Primitive  
**Scope:** DisclosureIndicator only  
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-223

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the DisclosureIndicator primitive.

A DisclosureIndicator is a **purely visual affordance** used to signal that
*additional content, actions, or hierarchy exists elsewhere*.

This SPEC exists to prevent DisclosureIndicators from becoming:
- interactive toggles
- state holders
- expansion controllers
- semantic authorities

DisclosureIndicator **communicates possibility**, not control.

---

## 1. Conceptual Definition

A **DisclosureIndicator** is a small visual glyph (e.g. arrow, chevron) that:

- hints at expandability, navigation, or submenu presence
- reflects externally provided state (expanded / collapsed, open / closed)
- never initiates behavior

It must **never**:
- own expansion state
- react to input
- emit events
- manage layout transitions

---

## Invariant Class A — Authority & Ownership

### INV-A1: No State Ownership

DisclosureIndicator does not own state.

Any notion of:
- expanded / collapsed
- open / closed
- active / inactive

is **provided externally**.

---

### INV-A2: No Control Authority

DisclosureIndicator must never:
- toggle state
- open menus
- expand sections
- trigger navigation

It reflects state only.

---

## Invariant Class B — Interaction Semantics

### INV-B1: Non-Interactive

DisclosureIndicator must not:
- accept pointer input
- accept keyboard input
- be focusable
- participate in hit testing

---

### INV-B2: No Implicit Actions

Hovering, clicking, or focusing must never:
- animate semantically
- change state
- emit signals

---

## Invariant Class C — Visual Semantics

### INV-C1: State Is Visual Only

If state is represented (e.g. rotated chevron):

- the mapping from state → visual form is deterministic
- there is no intermediate semantic state

---

### INV-C2: Theme-Driven Appearance

DisclosureIndicator uses:
- theme-provided geometry
- theme-provided colors

No hardcoded sizes, angles, or styles.

---

### INV-C3: Stateless Animation Only

If animation exists:
- it is driven entirely by external state changes
- it has no semantic meaning

---

## Invariant Class D — Composition Rules

### INV-D1: Always Embedded

DisclosureIndicator must always be embedded inside
another widget (e.g. MenuItem, SectionHeader variant, TreeRow).

It must never be mounted as a standalone widget.

---

### INV-D2: No Layout Authority

DisclosureIndicator does not:
- size parents
- influence alignment
- define spacing rules

Its geometry is consumed by the parent.

---

## Invariant Class E — Determinism & Safety

### INV-E1: Deterministic Rendering

Given identical external state and theme,
the rendered indicator is identical.

---

### INV-E2: Zero Allocation Steady State

No per-frame allocations during layout or paint.

---

### INV-E3: Invariant Supremacy

No invariant may be violated for:
- perceived UX expectations
- platform imitation
- convenience

New behavior requires a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- indicator shapes
- animation curves
- expansion logic
- hierarchy models

Those belong to higher-level controllers.

---

## 3. Summary (Non-Normative)

- DisclosureIndicator is **pure signal**
- It owns no state and no behavior
- It never reacts, only reflects
- Control always lives elsewhere

This SPEC locks DisclosureIndicator as a **strictly passive visual primitive**.
