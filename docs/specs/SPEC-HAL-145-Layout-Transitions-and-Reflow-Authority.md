# SPEC-HAL-145: Layout Transitions and Reflow — Authority Model

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Implemented / Binding
**Layer:** Layout & Geometry Engine
**Scope:** Authority, ownership, and legality of layout transitions and reflow
**Depends on:** SPEC-HAL-008, SPEC-HAL-026, SPEC-HAL-078, SPEC-HAL-142, SPEC-HAL-142A
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Modern UI systems allow **multiple subsystems to mutate geometry implicitly**:

* layout engine recomputes sizes
* animations tween rects
* transitions interpolate layout
* scrolling offsets geometry
* widgets “adjust” themselves

This leads to:

* geometry races
* non-deterministic reflow
* hit-testing desync
* animation/layout conflicts
* impossible-to-debug visual glitches

Halcyon already has:

* a layout engine (SPEC-HAL-008)
* dirty-based invalidation (SPEC-HAL-026)
* layout transitions (SPEC-HAL-078)

What is missing is a **binding authority model** answering one question:

> **Who is allowed to own geometry over time?**

This SPEC defines that authority and makes violations illegal.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Geometry has an owner**
At any moment in time, geometry is owned by exactly one subsystem.

**PRINCIPLE-2: Reflow is not animation**
Layout recomputation and visual interpolation are distinct phases.

**PRINCIPLE-3: Time is explicit**
If geometry changes over time, temporal ownership must be declared.

**PRINCIPLE-4: No silent overrides**
No subsystem may “adjust” geometry without authority.

**PRINCIPLE-5: Determinism first**
Given identical inputs, geometry evolution must be reproducible.

---

## SECTION C — Geometry Authority Domains

Halcyon recognizes exactly four geometry authority domains:

1. **Layout Authority**
   Owns geometry during structural reflow.

2. **Transition Authority**
   Owns interpolation between two authorized layout states.

3. **Animation Authority**
   Owns non-layout-affecting visual motion only.

4. **Scroll / Viewport Authority**
   Owns viewport-relative transforms only.

At any instant, **exactly one** domain owns a rect.

**363/364 scoped amendment:** These remain exactly four geometry-authority domains. Canonical
layout records and separate immutable presentation values SHALL NOT be conflated
into a jointly mutable rect. SPEC-HAL-363/364 adds no fifth geometry domain;
it coordinates existing animation/presentation ownership and certifies execution.

---

## SECTION D — Layout Authority

Layout authority belongs exclusively to the layout engine.

**INV-LAYOUT-AUTH-ONLY**
Only the layout engine may produce canonical rects.

**INV-LAYOUT-ATOMIC**
Layout recomputation produces a complete rect set atomically.

**INV-LAYOUT-NO-TWEEN**
Layout authority MUST NOT interpolate over time.

Layout outputs are **discrete states**, not animations.

---

## SECTION E — Transition Authority

Layout transitions interpolate **between two layout-authorized rect states**.

**INV-TRANSITION-EXPLICIT**
Transition authority is entered only when a layout transition is declared.

**INV-TRANSITION-BOUNDED**
Transitions interpolate only between:

* previous layout rect
* next layout rect

**INV-TRANSITION-NO-RECOMPUTE**
Transitions MUST NOT trigger layout recomputation.

---

## SECTION F — Animation Authority

Animation authority applies only to **visual transforms**.

**INV-ANIM-NO-LAYOUT**
Animations MUST NOT mutate layout rects.

**INV-ANIM-PURE-PROJECTION**
Animated geometry is a visual projection, not canonical state.

**363/364 scoped amendment:** An admitted SPEC-HAL-363/364 group MAY consume an eligible transition
result under one explicit effective-ownership plan. The consumed channel SHALL
NOT also receive the ordinary transition transform. Unrelated channels may
coexist only when composition order and ownership are proven. No presentation
operation overrides canonical layout.

---

## SECTION G — Scroll and Viewport Authority

Scroll authority applies to viewport-relative transforms.

**INV-SCROLL-VIEWPORT-ONLY**
Scroll MUST NOT mutate layout rects.

**INV-SCROLL-POST-LAYOUT**
Scroll is applied after layout and transitions.

Scroll authority never becomes canonical geometry.

---

## SECTION H — Reflow Rules

Reflow is a **structural event**.

**INV-REFLOW-TRIGGERS**
Reflow may occur only due to:

* content change
* constraint change
* explicit invalidation

**INV-REFLOW-CANCELS-TRANSITION**
**363/364 scoped amendment:** Reflow immediately ends the previous canonical geometry interval
and commits fresh layout truth. Ordinary transition endpoints remain actual
previous/current layout outputs. After that commit, a request lawfully reserved
in Update MAY admit a separately labelled presentation continuation under
SPEC-HAL-363/364 using certified prior execution and current dependencies.
Such continuation SHALL neither fabricate layout history nor retain pre-reflow
canonical ownership. Without that admission, presentation settles canonically.

---

## SECTION I — Forbidden Behaviors

The following are categorically forbidden:

* animating layout rects directly
* recomputing layout during a transition tick
* scrolling by mutating layout geometry
* widgets nudging their own rects
* multiple subsystems writing geometry in one frame

**363/364 scoped amendment:** An immutable presentation projection is not an additional canonical
geometry writer. The multiple-writer prohibition remains binding.

---

## SECTION J — Interaction with Existing SPECs

HAL-128's delegated 2026-10-08 geometry-retirement addition uses actual affected
active transitions or admitted causal reflow as pre-Layout invalidity evidence
for an explicitly bound Absolute session. Pending anchor/reveal/transition work
keeps its existing later numeric resolution. No interpolation/anchor algorithm,
geometry domain, phase or Proposed 363/364 clause is adopted or changed by that
reference.

* SPEC-HAL-008 defines layout computation
* SPEC-HAL-026 defines invalidation mechanics
* SPEC-HAL-078 defines layout transitions
* SPEC-HAL-142 / 142A enforce state separation

SPEC-HAL-145 adds **authority law**, not new features.

---

## SECTION K — Performance and Predictability

**INV-GEOM-OWNERSHIP-O1**
Geometry ownership checks MUST be constant-time.

**INV-GEOM-NO-HIDDEN-WORK**
Transitions and animations MUST NOT trigger hidden layout work.

---

## SECTION L — Acceptance Criteria

SPEC-HAL-145 is satisfied when:

* [ ] Only layout engine produces canonical rects
* [ ] Transitions interpolate between known layout states
* [ ] Animations never affect layout
* [ ] Scroll never mutates geometry
* [ ] Geometry ownership is unambiguous at all times

---

## SECTION M — Authoritative Statements

**AUTH-145-1:** "Layout computes truth; transitions interpolate truth."
**AUTH-145-2:** "Nothing moves geometry without owning time."
**AUTH-145-3:** "If two systems touch the same rect, one of them is wrong."

---

## SECTION N — Implementation Notes

* Transition authority is enforced at runtime via `TransitionAuthorityGuard`
* Interpolation runs in a dedicated pre-paint phase (`compute_interpolated_visual_rects`)
* Paint phase is projection-only; reads from pre-computed `InterpolatedVisualRects`
* Authority violations panic in debug builds

---

## SECTION O — Enforcement Summary

| Authority   | Permitted Operations       | Forbidden Operations |
|-------------|---------------------------|----------------------|
| Layout      | `set_rect` only           | Interpolation        |
| Transition  | `lerp_rect` only          | `set_rect`           |
| Paint       | Read-only                 | Any rect mutation    |

Violations panic in debug builds via `assert_rect_mutation_authorized` and `assert_rect_interpolation_authorized`.
