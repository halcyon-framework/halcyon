# SPEC-HAL-141: Rect Mutation Authority — Geometry Governance

**Status:** Implemented / Binding
**Layer:** Layout & Geometry Substrate
**Scope:** Authorization and ownership of all rect mutations
**Depends on:** SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-026, SPEC-HAL-078, SPEC-HAL-078A, SPEC-HAL-078B, SPEC-HAL-140
**Constrains:** Engine-level semantics only (no widget-local exceptions)

---

## SECTION A — Problem Statement

Halcyon currently allows **rect mutations to occur from multiple execution paths**:

* layout recomputation
* scroll offset application
* layout transitions
* animation timelines
* ad-hoc widget logic
* implicit “ensure visible” adjustments

While individually correct, **their combination is not governed**.

This results in:

* geometry changing without a single accountable owner
* layout and animation fighting over the same rect
* hit-testing desynchronizing from visuals
* “ghost” interaction states caused by stale geometry
* bugs that reappear after unrelated fixes

Rect mutation is currently **possible**, but not **authorized**.

This SPEC establishes **exclusive rect mutation authority** and makes all other mutations **architecturally illegal**.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Geometry is a protected resource**
Rect state is not an incidental byproduct; it is core engine truth.

**PRINCIPLE-2: Mutation requires authority**
Every rect mutation must have a single, declared author.

**PRINCIPLE-3: One rect, one owner, at a time**
Concurrent ownership of geometry is forbidden.

**PRINCIPLE-4: Time matters**
Rect ownership may change over time, but never overlap.

**PRINCIPLE-5: No silent geometry changes**
If a rect changes, the engine must be able to explain why.

---

## SECTION C — Rect Ownership Model

At any moment, a rect may be owned by **exactly one** of the following authorities:

1. **Layout Authority**
2. **Scroll Authority**
3. **Transition Authority**
4. **Animation Authority**
5. **None (Frozen / Read-Only)**

All rects default to **Layout Authority** unless explicitly transferred.

---

## SECTION D — Layout Authority

Layout Authority is the **default and primary owner** of rects.

**INV-RECT-LAYOUT-ONLY**
Only the layout engine may assign rects during layout resolution.

**INV-RECT-LAYOUT-DETERMINISTIC**
Given identical inputs, layout MUST produce identical rects.

**INV-RECT-LAYOUT-EXCLUSIVE**
While layout owns a rect, no other system may mutate it.

---

## SECTION E — Scroll Authority

Scroll mutates **viewport projection**, not layout geometry.

**INV-RECT-SCROLL-NO-LAYOUT-MUTATION**
Scroll MUST NOT change layout-computed rects.

**INV-RECT-SCROLL-PROJECTION-ONLY**
Scroll affects only view transforms or offsets.

**INV-RECT-SCROLL-AUTHORIZED**
Scroll authority is active only while scroll capture exists.

---

## SECTION F — Transition Authority

Layout transitions temporarily own geometry **between two layout states**.

**INV-RECT-TRANSITION-EXPLICIT**
Transition authority must be explicitly entered.

**INV-RECT-TRANSITION-BOUNDED**
Transitions interpolate only between two layout-authorized rects.

**INV-RECT-TRANSITION-TEMPORAL**
Transition authority is time-limited and must end.

---

## SECTION G — Animation Authority

Animations may interpolate geometry **only when authorized**.

**INV-RECT-ANIM-NO-RAW-MUTATION**
Animations MUST NOT write rects directly.

**INV-RECT-ANIM-CONSTRAINT-ONLY**
Animations operate as constraints over authorized rect states.

**INV-RECT-ANIM-NO-DESYNC**
Animated geometry MUST remain hit-test consistent.

---

## SECTION H — Authority Transfer Rules

Rect ownership transfer is explicit and ordered.

Valid transfers:

* Layout → Transition
* Transition → Layout
* Layout → Animation (via transition)
* Any → None (freeze)

**INV-RECT-NO-IMPLICIT-TRANSFER**
Ownership MUST NOT change implicitly.

**INV-RECT-NO-SKIP**
Illegal transitions (e.g. Scroll → Animation) are forbidden.

---

## SECTION I — Forbidden Behaviors

The following are categorically forbidden:

* widget code mutating rects directly
* animations writing rect values
* scroll modifying layout geometry
* multiple authorities mutating the same rect
* rect mutation without recorded authority

---

## SECTION J — Debug & Enforcement

**INV-RECT-AUTH-ENFORCED**
Unauthorized rect mutation MUST panic in debug builds.

**INV-RECT-AUTH-INSPECTABLE**
Current rect authority must be inspectable at runtime.

---

## SECTION K — Interaction with Existing SPECs

HAL-128's accepted 2026-10-08 geometry-dependent Absolute contract retires an
invalid declared dependency at existing input/Update checkpoints before Layout.
It records causal invalidity, not a replacement rectangle or geometry authority.
No cancellation callback enters Layout/Transition/Scene/Paint; all rect ownership
and the ordinary geometry production sequence remain unchanged.

* Layout defined by SPEC-HAL-008
* Coordinate space defined by SPEC-HAL-024
* Invalidation governed by SPEC-HAL-026
* Transitions governed by SPEC-HAL-078 / 078A / 078B
* Interaction authority separation defined by SPEC-HAL-140

SPEC-HAL-141 introduces **no new features**, only **governance**.

---

## SECTION L — Performance and Predictability

**INV-RECT-O(1)**
Authority checks must be constant-time.

**INV-RECT-NO-EXTRA-PASSES**
No additional layout or render passes are introduced.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-141 is satisfied when:

* [ ] Every rect mutation has an explicit authority
* [ ] Unauthorized mutation is impossible
* [ ] Layout, scroll, animation never fight
* [ ] Hit-testing and visuals remain consistent
* [ ] Geometry bugs stop resurfacing indirectly

---

## SECTION N — Authoritative Statements

**AUTH-141-1:** "Geometry is law, not suggestion."
**AUTH-141-2:** "No rect moves without permission."
**AUTH-141-3:** "Time-bounded ownership prevents ghosts."

---

## SECTION O — Implementation Reference

Authority enforcement is wired into the frame pipeline via SPEC-HAL-145.
See `src/core/rect_authority.rs` for `LayoutAuthorityGuard`, `TransitionAuthorityGuard`, and runtime assertion functions.
