# SPEC-HAL-227: Tutorial, Coachmark, and Guided Hint Sequencing

**Status:** Draft (Binding upon adoption)
**Layer:** Policy / UX Orchestration
**Scope:** Tutorial flows, coachmarks, guided hints
**Applies To:** Onboarding, tutorials, contextual guidance
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-226

---

## 0. Purpose

Define engine-grade invariants for sequenced instructional guidance built on top of tooltip-like surfaces, without turning guidance into UI controls or hidden state machines.

This SPEC exists to prevent:
- tutorials embedding behavior into widgets
- guidance systems owning application logic
- implicit timing, heuristics, or "smart" UX engines

Guided hints are policy, not controls.

---

## 1. Conceptual Definition

A Guided Hint System is an orchestration layer that:
- sequences informational overlays (tooltips, coachmarks)
- reacts to explicit external signals
- advances deterministically
- never executes application actions

It does not render UI directly.
It commands informational surfaces defined by SPEC-HAL-226.

---

## 2. Architectural Positioning

| Layer | Responsibility |
|-------|----------------|
| SPEC-HAL-226 | Render informational overlays |
| SPEC-HAL-227 | Decide which overlay appears when |
| Application | Decide why and if guidance exists |

SPEC-HAL-227 owns ordering and visibility policy only.

---

## Invariant Class A — Authority & Ownership

### INV-227-A1: No UI Rendering

Guided Hint systems must not:
- draw widgets
- issue paint commands
- create overlay geometry

They only instruct SPEC-226 surfaces.

---

### INV-227-A2: No Application Action Authority

They must not:
- trigger commands
- modify models
- simulate user input

They react to signals only.

---

### INV-227-A3: No Implicit State Discovery

They must not:
- inspect widget internals
- query focus trees
- infer "what the user did"

All signals are explicit.

---

## Invariant Class B — Sequencing Semantics

### INV-227-B1: Explicit Step Graph

Tutorials are defined as:
- a linear sequence, or
- an explicit directed graph

No dynamic restructuring.

---

### INV-227-B2: Single Active Step

At most one guidance step may be active at any time.

---

### INV-227-B3: Deterministic Advancement

Step transitions occur only when:
- an explicit signal is received, or
- an explicit external command is issued

No timers.
No heuristics.

---

## Invariant Class C — Signals & Triggers

### INV-227-C1: External Signals Only

Transitions may react to:
- named application events
- explicit user acknowledgements
- explicit dismissal commands

Never raw input events.

---

### INV-227-C2: No Timing Authority

The system must not:
- delay steps
- auto-advance
- expire steps

Time is not a signal.

---

## Invariant Class D — Persistence & Memory

### INV-227-D1: Optional, Explicit Persistence

Progress persistence:
- is optional
- is external
- must be explicit

The system must not decide what to remember.

---

### INV-227-D2: Stateless by Default

Without an external store, the system is fully ephemeral.

---

## Invariant Class E — Geometry & Anchoring

### INV-227-E1: Anchoring Is Delegated

Each step provides:
- an anchor (point or rect)
- content payload

Rendering and placement are handled by SPEC-226 surfaces.

---

## 3. Minimal Architecture Shape

TutorialStep:
- id
- content payload
- anchor
- advancement conditions (signal IDs)

TutorialController:
- current step id
- explicit transition table
- emits "show / hide" commands to TooltipSurface

No widget access.
No paint access.
No input handling.

---

## 4. Explicit Non-Goals

SPEC-HAL-227 does not define:
- tooltip visuals
- animations
- input capture
- accessibility narration
- analytics
- A/B logic

Those are separate layers.

---

## 5. Acceptance Criteria

- Tutorials cannot execute application actions.
- No widget is aware it is being "tutorialized".
- Same signals → same progression.
- Removing the tutorial system leaves UI behavior unchanged.

---

## 6. Summary (Non-Normative)

- SPEC-227 is policy only
- It sequences informational overlays
- It owns no UI, no logic, no time
- It cannot "help too much"

This SPEC locks tutorials as explicit, deterministic guidance, not magic UX.
