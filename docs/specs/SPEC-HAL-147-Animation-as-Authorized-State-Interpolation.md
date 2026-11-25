# SPEC-HAL-147: Animation as Authorized Geometry-State Interpolation

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
**Layer:** Post-layout geometry-transition projection
**Scope:** Visual interpolation between authorized geometry states under Section C; generic value sampling/playback remains owned by SPEC-HAL-021
**Depends on:** SPEC-HAL-021, SPEC-HAL-078, SPEC-HAL-145, SPEC-HAL-146
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Traditional UI geometry-animation systems treat transition projection as a
**free-form pixel driver**:

* animations push rects directly
* geometry drifts from layout truth
* hit-testing desyncs from visuals
* transitions fight reflow
* bugs appear only under motion

Halcyon explicitly rejects this model for layout geometry. This SPEC does not
constrain generic non-geometry value interpolation, handle lifecycle, or playback;
SPEC-HAL-021 owns those contracts.

In Halcyon:

> **Animation does not create geometry.
> Animation interpolates between authorized geometry states.**

This SPEC makes that rule binding and mechanically enforceable.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Geometry animation is not authority**
Geometry-transition projection never owns canonical geometry.

**PRINCIPLE-2: Authorized geometry states, not free pixels**
Geometry transitions interpolate between declared geometry states, not
independently authored pixel rects.

**PRINCIPLE-3: Authorization first**
Only geometry states produced by authorized owners may be animated.

**PRINCIPLE-4: Visual-only projection**
Geometry-transition projection affects rendering, never layout, hit-testing, or
state truth.

**PRINCIPLE-5: Determinism over expressiveness**
Predictable systems outrank flashy motion.

---

## SECTION C — Canonical Geometry States

**363/364 scoped amendment:** Canonical geometry states are actual layout-authorized records.
Ordinary SPEC-HAL-078 projection uses its expressly authorized previous/current
layout states; the projection itself remains visual. SPEC-HAL-363/364 additionally
permits a certified presentation endpoint: an owned immutable source/projection
value with exact occurrence, basis, coverage, cue and execution provenance.
It SHALL be labelled presentation-only and SHALL NOT be passed as canonical
layout history. These are the only endpoint classes authorized for their
respective paths.

**INV-ANIM-ENDPOINT-AUTH:** Animation endpoints MUST belong to the authorized
endpoint class for the consuming path. Interpolation does not certify them.

---

## SECTION D — Animation Model

Geometry-transition projection is defined as:

```
AnimatedRect(t) = interpolate( State_A, State_B, t )
```

Where:

* `State_A` and `State_B` are authorized geometry states
* `t ∈ [0, 1]`
* interpolation is pure and side-effect-free

**INV-ANIM-PURE**
Geometry-transition interpolation MUST NOT mutate state.

**363/364 scoped amendment:** A SPEC-HAL-363/364 projection is a pure finite function of its
certified source/endpoints, admitted operation descriptor, current authoritative
dependencies and one Update-published group parameter. Interpolation never
certifies its own endpoint or source.

---

## SECTION E — Ownership and Time

Per SPEC-HAL-146:

* Geometry-transition projection owns **no time slice** of canonical geometry
* Geometry-transition projection operates strictly after ownership resolution

**INV-ANIM-TEMPORAL-SECONDARY**
Geometry-transition projection MUST run after layout and transition resolution.

---

## SECTION F — Interaction with Reflow

**363/364 scoped amendment:** **INV-ANIM-REFLOW-CANCEL:** Reflow ends the previous geometry projection.
Canonical blending and catch-up remain forbidden. Only an explicit
SPEC-HAL-363/364 request admitted after current layout and effective-ownership
resolution may preserve presentation continuity using a new certified visual
interval. The request must have been reserved in Update; no post-layout
controller/timing mutation is permitted.

---

## SECTION G — Hit Testing and Input

Hit-testing and input routing use **canonical geometry only**.

**INV-ANIM-NO-HITTEST**
Animated geometry MUST NOT affect hit-testing.

**INV-ANIM-NO-INPUT**
Input MUST NOT follow animated visuals.

Visual drift does not imply interaction drift.

---

## SECTION H — Forbidden Behaviors

The following are categorically forbidden:

* animating layout-owned rects
* animating during reflow
* writing projected geometry values back to canonical state
* interpolating from non-authorized states
* animation-driven hit-testing

**363/364 scoped amendment:** “Animating during reflow” forbids projection feeding or running as
layout authority. Applying the admitted pure projection after layout is the
sole scoped continuation exception in Section F. Canonical hit-testing and
Sections E/G are unchanged.

---

## SECTION I — Interaction with Existing SPECs

* SPEC-HAL-021 defines generic Timeline timing, playback, lifecycle, and sampling
* SPEC-HAL-078 defines layout transitions
* SPEC-HAL-145 defines geometry authority
* SPEC-HAL-146 defines temporal ownership

SPEC-HAL-147 constrains post-layout projection of Section C's authorized
endpoint classes. It neither narrows nor duplicates SPEC-HAL-021 generic value
sampling. SPEC-HAL-363/364 supplies only the explicitly scoped presentation class.

---

## SECTION J — Performance and Predictability

**INV-ANIM-O1**
Animation interpolation MUST be constant-time per frame.

**INV-ANIM-REPLAYABLE**
Given the same inputs and time, animation output is deterministic.

---

## SECTION K — Acceptance Criteria

SPEC-HAL-147 is satisfied when:

* [ ] Geometry transitions interpolate only between authorized states
* [ ] Canonical geometry is never mutated by transition projection
* [ ] Reflow cancels the old geometry transition immediately; a new presentation interval satisfies Section F
* [ ] Hit-testing ignores animated geometry
* [ ] Geometry-transition output is deterministic and inspectable

---

## SECTION L — Authoritative Statements

**AUTH-147-1:** "Geometry animation is a lens, not a force."
**AUTH-147-2:** "If geometry-transition projection writes canonical state, the
system is already broken."
**AUTH-147-3:** "Geometry is decided once; animation only explains it visually."

---

## SECTION M — Enforcement Note

Animation as authorized interpolation is mechanically enforced by the transition authority system (SPEC-HAL-145).
`lerp_rect` is pure interpolation; `set_rect` is forbidden under transition authority.
