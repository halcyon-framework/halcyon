# SPEC-HAL-226: Tooltip, Hint, and Help Surfaces (Overlay-Only, Non-Action)

**Status:** Draft (Binding upon adoption)
**Layer:** Transient UI / Informational Overlays
**Scope:** Tooltip / hint / help surfaces only
**Applies To:** Tooltips, tutorial hints, inline help bubbles (non-interactive)
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-216, SPEC-HAL-217

---

## 0. Purpose

Define engine-grade invariants for informational overlay surfaces that display content without becoming controls.

This SPEC exists to prevent:
- tooltips turning into controllers
- timing logic embedded in widgets
- tooltips hijacking input or focus

---

## 1. Conceptual Definition

A TooltipSurface is:
- overlay-rendered
- non-interactive
- explicitly anchored
- explicitly shown and hidden by an owner

It emits no intent.

---

## 2. Binding Invariants

### Invariant Class A — Authority

### INV-226-A1: Informational Only

No actions, no selection, no commands.

---

### INV-226-A2: No App State Ownership

No model data, no inferred context, no permissions.

---

### INV-226-A3: Explicit Content Injection

All content (text, icon, variant) is provided explicitly.

---

### Invariant Class B — Time Authority

### INV-226-B1: No Timers Inside Tooltip

No delays, no dwell logic, no auto-dismiss.

---

### INV-226-B2: Immediate Rendering

Tooltip presence must not delay or block anything.

---

### Invariant Class C — Input & Focus

### INV-226-C1: Non-Interactive

No hit regions, no pointer capture, no keyboard capture.

---

### INV-226-C2: No Input Hijack

Underlying UI remains fully interactive.

---

### INV-226-C3: External Dismissal Policy

Tooltip never decides when to hide itself.

---

### Invariant Class D — Geometry & Rendering

### INV-226-D1: Overlay-Only Rendering

No layout impact, no reflow.

---

### INV-226-D2: Explicit Anchor Fidelity

Anchored only to provided point or rect.

---

### INV-226-D3: Self-Contained Geometry

Size depends only on content + theme.

---

### INV-226-D4: Deterministic

Same inputs → same output.

---

### INV-226-D5: Allocation Discipline

No per-frame allocations introduced by tooltip code.

---

## 3. Minimal Architecture Shape

Model (optional):
- visibility flag
- anchor
- payload

Widget:
- zero-size layout
- paint_overlay only
- no event handling

---

## 4. Non-Goals

This SPEC does not define:
- hover detection
- delay timing
- accessibility narration
- tutorial sequencing

Those are separate policy layers.

---

## 5. Acceptance Criteria

- Tooltips never block input.
- No timers exist inside tooltip code.
- Tooltip behavior is fully externally driven.
- Overlay-only rendering is preserved.
