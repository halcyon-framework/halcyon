# SPEC-HAL-146: Temporal Ownership of Geometry

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Satisfied by Enforcement
**Layer:** Geometry, Layout, Animation Timing
**Scope:** Time-scoped ownership of geometric state and transitions
**Depends on:** SPEC-HAL-008, SPEC-HAL-026, SPEC-HAL-078, SPEC-HAL-145
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Even with clear **authority domains** (SPEC-HAL-145), geometry still breaks when
**time is implicit**:

* layout produces rects “now”
* transitions interpolate “over time”
* animations update “per frame”
* scroll offsets apply “continuously”

Without a binding model of **who owns geometry at time *t***, systems overlap,
fight, or silently override each other.

This SPEC answers one question:

> **Who owns a rect at each moment in time, and when does ownership transfer?**

Temporal ownership is made explicit and enforceable.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Geometry is a time-bound resource**
A rect is not just *where*, but *when*.

**PRINCIPLE-2: Ownership is exclusive per time slice**
At any instant, exactly one owner controls a rect.

**PRINCIPLE-3: Ownership transitions are explicit**
Time-based handoffs must be declared, not inferred.

**PRINCIPLE-4: No mid-frame ambiguity**
Ownership may not change implicitly during a frame.

**PRINCIPLE-5: Deterministic replay**
Given the same timeline, geometry evolution is reproducible.

---

## SECTION C — Temporal Ownership States

Each rect exists in exactly one of the following temporal states:

1. **Layout-Owned (Discrete)**
   Owned by the layout engine at a single instant.

2. **Transition-Owned (Bounded Interval)**
   Owned by the transition system between two layout states.

3. **Animation-Owned (Visual Interval)**
   Owned by animation as a projection only.

4. **Viewport-Owned (Continuous Offset)**
   Owned by scroll / viewport transforms.

No other temporal ownership states are permitted.

**363/364 scoped amendment:** A Timeline reservation grants no geometry ownership.
SPEC-HAL-363/364 visual admission begins only after geometry finalization and
pure ownership resolution; no additional temporal ownership state is created.

---

## SECTION D — Ownership Entry Rules

**INV-TIME-ENTER-LAYOUT**
Layout ownership begins only at layout commit.

**INV-TIME-ENTER-TRANSITION**
Transition ownership begins only when:

* a previous layout state exists
* a next layout state exists
* a transition interval is declared

**INV-TIME-ENTER-ANIM**
Animation ownership begins only after geometry is finalized
(layout or transition).

**INV-TIME-ENTER-VIEWPORT**
Viewport ownership applies only after all other geometry resolution.

**363/364 scoped amendment:** Update timing reservation is distinct from visual admission under
Section C. It cannot enter geometry ownership before finalization.

---

## SECTION E — Ownership Exit Rules

**INV-TIME-EXIT-TRANSITION**
At transition end, ownership MUST return to layout state.

**INV-TIME-EXIT-ANIM**
Animation ownership ends without mutating canonical geometry.

**INV-TIME-EXIT-REFLOW**
**363/364 scoped amendment:** Reflow terminates the previous non-layout geometry interval and
returns canonical ownership to current layout. Canonical geometry SHALL NOT
blend with prior geometry. Only Section F's separately admitted new presentation
interval may follow; the terminated interval does not survive.

---

## SECTION F — Reflow and Temporal Reset

**363/364 scoped amendment:** **INV-REFLOW-CANCELS-TIME:** Reflow terminates the previous non-layout
geometry interval and returns canonical ownership to current layout.

**INV-REFLOW-NO-BLEND:** Canonical geometry SHALL NOT blend with prior geometry.
SPEC-HAL-363/364 MAY then admit a new presentation-only continuation, reserved at
a legal Update boundary, from certified execution-backed values to the current
presentation. This is a new visual interval with fresh dependency validation,
not survival of the terminated canonical interval. Failure to admit SHALL
settle canonically. Layout produces a new discrete truth.

---

## SECTION G — Frame Semantics

**363/364 scoped amendment:** This order is geometry resolution: canonical layout commit,
ordinary transition eligibility/resolution, admitted pure presentation
projection, viewport mapping at its established basis, then render execution.

**INV-FRAME-ORDERING:** Geometry resolution MUST follow that order. The literal
frame/time order remains SPEC-HAL-042/021. This sequence SHALL NOT authorize a
post-layout Timeline tick, sample or control mutation.

---

## SECTION H — Forbidden Behaviors

The following are categorically forbidden:

* changing ownership mid-frame
* animating geometry during layout ownership
* continuing a terminated interval after reflow; Section F alone admits a distinct new presentation interval
* mutating canonical rects during animation
* implicit time-based overrides

---

## SECTION I — Interaction with Existing SPECs

* SPEC-HAL-145 defines *who* may own geometry
* SPEC-HAL-078 defines layout transitions
* SPEC-HAL-026 defines reflow invalidation
* SPEC-HAL-008 defines layout computation

SPEC-HAL-146 defines **when** ownership applies.

---

## SECTION J — Performance and Debuggability

**INV-TIME-OWNERSHIP-O1**
Ownership resolution MUST be constant-time.

**INV-TIME-INSPECTABLE**
Current owner and ownership interval MUST be inspectable.

---

## SECTION K — Acceptance Criteria

SPEC-HAL-146 is satisfied when:

* [ ] Every rect has exactly one owner at any time
* [ ] Ownership transfers are explicit and bounded
* [ ] Reflow cancels prior temporal ownership; any new presentation interval satisfies Section F
* [ ] Animations never affect canonical geometry
* [ ] Frame order is deterministic and enforced

---

## SECTION L — Authoritative Statements

**AUTH-146-1:** "Geometry exists in time, not just space."
**AUTH-146-2:** "If time is implicit, bugs are inevitable."
**AUTH-146-3:** "Reflow resets canonical history; Section F never fabricates it."

---

## SECTION M — Enforcement Note

Temporal ownership is mechanically enforced via `TransitionAuthorityGuard` (SPEC-HAL-145).
The frame order Layout → Transition → Paint is guaranteed by the authority system.
