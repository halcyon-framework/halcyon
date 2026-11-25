# SPEC-HAL-157 — PointerDown Capture-First Routing Authority Contract

**Status:** Binding
**Layer:** Core Event Routing / Input Authority
**Scope:** PointerDown routing, capture vs projected routing ordering
**Depends on:** Event phases (Capture / Target / Bubble), WidgetTree hit-testing, Pointer capture manager
**Constrains:** All PointerDown routing paths, including virtualized / projected targets

---

## SECTION A — Problem Statement

Certain interactions must be decided by **structural ancestors** before any **projected or synthetic targets** can consume input.

In the current architecture, **projected routing** (e.g. virtualized list items) may consume `PointerDown` *before* ancestor widgets have a chance to claim interaction authority during capture. This produces systemic failures:

* resize zones blocked by row selection
* composite widgets unable to assert ownership
* behavior fixed via ad-hoc router hacks
* regressions recurring under refactors

This is not a widget bug.
This is a **routing order violation**.

Halcyon requires a strict, engine-level law:

> **PointerDown authority is decided in capture phase, before any projected routing may run.**

---

## SECTION B — Design Principles

**PRINCIPLE-1: Structural ancestors decide first**
Widgets that structurally own interaction zones must be able to claim authority before descendants or projections.

**PRINCIPLE-2: Capture is the only legal preemption mechanism**
If a widget needs to override projected behavior, it must do so by consuming `PointerDown` during capture — not via router conditionals.

**PRINCIPLE-3: Routing order must be mechanical**
This rule must be enforced by the router itself, not by widget-specific hacks or “special cases”.

**PRINCIPLE-4: No duplicate phase execution**
Each routing phase must run **exactly once** per event, in a defined order.

---

## SECTION C — Core Routing Contract

### C1. Capture-first ordering for PointerDown

For `PointerDown` events, the router MUST execute:

1. **Capture phase (only)** — root → parent(target)
2. **If capture consumed:** STOP routing immediately
3. **If not consumed:** attempt projected routing
4. **If still unconsumed:** run target + bubble phases

Capture MUST run **before** projected routing.

### C2. Capture consumption short-circuits routing

If any widget consumes `PointerDown` during capture phase:

* projected routing MUST NOT run
* target phase MUST NOT run
* bubble phase MUST NOT run

Subsequent events (`Move`, `Up`) rely on pointer capture as usual.

### C3. No capture duplication

If capture phase has already executed in the capture-first step, it MUST NOT be executed again during normal routing.

---

## SECTION D — Coordinate Semantics

### D1. Capture-phase coordinates (visual space)

During capture phase of `PointerDown`:

* `local_pos` MUST be computed as:

  ```
  global_pos − absolute_paint_rect.origin
  ```
* **No scroll offset may be applied**

Rationale: capture decides visual-space authority (resize handles, gutters, frames).

### D2. Target / bubble coordinates (content space)

During target and bubble phases:

* `local_pos` MUST be computed as:

  ```
  global_pos + cumulative_scroll_offset − absolute_paint_rect.origin
  ```

This preserves existing content-space semantics.

---

## SECTION E — Authority Model

### E1. Legal authority claim

The ONLY legal way to preempt projected routing is:

* consume `PointerDown` during capture
* optionally request pointer capture

### E2. Forbidden authority mechanisms

The following are forbidden:

* router checks for widget types
* “has_resize_hit()” probes in the router
* screen-space magic thresholds
* conditional routing based on widget identity

All authority must be asserted via capture consumption.

---

## SECTION F — Binding Invariants (LAW)

**INV-PDOWN-ORDER-1 (Capture first):**
Capture phase MUST execute before projected routing for `PointerDown`.

**INV-PDOWN-ORDER-2 (Capture short-circuit):**
If capture consumes, no further routing may occur for that event.

**INV-PDOWN-ORDER-3 (No duplicate capture):**
Capture phase MUST NOT be executed more than once per event.

**INV-PDOWN-COORD-1 (Visual capture coords):**
Capture-phase `local_pos` MUST NOT include scroll offset.

**INV-PDOWN-COORD-2 (Content target coords):**
Target/bubble `local_pos` MUST include cumulative scroll offset.

**INV-PDOWN-AUTH-1 (Capture decides authority):**
Projected routing MUST NEVER override a capture-phase decision.

---

## SECTION G — Forbidden Patterns

**FORBID-PDOWN-1:** Projected routing before capture
**FORBID-PDOWN-2:** Widget-specific routing branches
**FORBID-PDOWN-3:** Hardcoded hit-test rectangles in router
**FORBID-PDOWN-4:** Debug-only enforcement
**FORBID-PDOWN-5:** Running capture twice “to be safe”

---

## SECTION H — Mechanical Test Cases

1. **Ancestor preemption**

    * Capture-phase widget consumes → no projected selection occurs

2. **Projected routing preserved**

    * No capture consumption → projected routing works as before

3. **Single capture execution**

    * Instrument capture handlers → exactly one invocation

4. **Coordinate correctness**

    * Capture uses visual coords
    * Target uses content coords

5. **Scroll unaffected**

    * Scroll and wheel behavior unchanged

---

## SECTION I — Applicability

This SPEC applies immediately to:

* `ListView` resize interaction
* any composite widget with structural interaction zones
* any virtualized or projected routing path

---

## SECTION J — Non-Goals

* This SPEC does not define resize behavior itself
* This SPEC does not alter gesture arbitration
* This SPEC does not modify hit-testing rules
* This SPEC does not affect PointerMove / PointerUp routing

---

## SECTION K — Acceptance Criteria

An implementation is compliant only if:

* Capture phase runs before projected routing for `PointerDown`
* Capture consumption fully short-circuits routing
* No router-level widget hacks remain
* Coordinate semantics are phase-correct
* Existing behaviors outside resize remain unchanged

---

This SPEC replaces all stage-specific descriptions of “resize preemption” with a **general, engine-law routing contract**.
