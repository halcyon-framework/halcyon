# SPEC-HAL-148: Stable Widget Identity

**Status:** Binding
**Layer:** Widget Identity, Diffing, Interaction Continuity
**Scope:** Stable, authoritative identity for widgets across frames, reflow, and virtualization
**Depends on:** SPEC-HAL-007, SPEC-HAL-015, SPEC-HAL-035, SPEC-HAL-037, SPEC-HAL-083
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Without stable identity, UI systems devolve into **frame-by-frame amnesia**:

* focus jumps unexpectedly
* selection ghosts appear or disappear
* animations restart incorrectly
* transitions mismatch old and new geometry
* virtualization breaks continuity

Many systems attempt to infer identity from position, index, or pointer equality.
This is insufficient and fundamentally unsound.

Halcyon requires **explicit, stable widget identity** as a first-class engine concept.

This SPEC defines how widget identity is created, preserved, compared, and invalidated —
and makes identity loss an explicit, detectable event.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Identity is explicit**
Widget identity is never inferred implicitly.

**PRINCIPLE-2: Identity survives movement**
Layout position, z-order, or hierarchy changes do not affect identity.

**PRINCIPLE-3: Identity is orthogonal to state**
Identity does not encode visual, interaction, or layout state.

**PRINCIPLE-4: Identity continuity enables correctness**
Focus, selection, animation, and transitions depend on identity stability.

**PRINCIPLE-5: Loss of identity is observable**
Identity invalidation is a first-class event, not silent failure.

---

## SECTION C — Definition of Widget Identity

A **Widget Identity (WID)** is:

* a stable, unique identifier
* assigned at widget instantiation
* preserved across frames
* comparable across scene diffs

**INV-WID-UNIQUE**
No two live widgets may share the same identity.

**INV-WID-STABLE**
A widget’s identity MUST NOT change during its lifetime.

---

## SECTION D — Identity vs Structure

Widget identity is **independent of**:

* parent container
* sibling order
* layout rect
* render batch
* virtualized reuse slot

**INV-WID-NO-POSITIONAL-COUPLING**
Identity MUST NOT be derived from index, order, or tree position.

---

## SECTION E — Identity Lifecycle

### Creation

Identity is assigned exactly once.

**INV-WID-CREATE-ONCE**
Identity MUST be created at widget construction time.

### Preservation

Across scene diffs:

**INV-WID-DIFF-MATCH**
Diffing MUST match widgets by identity, not structure.

### Destruction

When a widget is removed:

**INV-WID-DESTROY-EXPLICIT**
Identity destruction MUST be explicit and observable.

---

## SECTION F — Identity and Diffing

The scene diff engine (SPEC-HAL-083) MUST:

* match nodes by identity first
* treat unmatched identities as removal/addition
* never merge unrelated identities

**INV-WID-DIFF-SAFETY**
No diff operation may reassign identity.

---

## SECTION G — Identity and Interaction State

Interaction state binds to identity, not instances.

This includes:

* focus
* selection
* navigation cursor
* capture
* hover tracking

**INV-WID-INTERACTION-BIND**
Interaction state MUST reference widget identity exclusively.

---

## SECTION H — Identity and Animation

Animations bind to identity.

**INV-WID-ANIM-STABILITY**
Animations MUST continue across frames only if identity persists.

If identity changes, animation is terminated immediately.

---

## SECTION I — Virtualization Rules

Virtualization MAY reuse widget instances,
but MUST NOT reuse identities incorrectly.

**INV-WID-VIRT-REMAP-ONLY**
Virtualized widgets MUST remap identity explicitly when reused.

Silent reuse is forbidden.

---

## SECTION J — Forbidden Behaviors

The following are categorically forbidden:

* identity derived from index
* identity derived from memory address
* implicit identity reassignment
* identity reuse without explicit remap
* diffing by structural similarity alone

---

## SECTION K — Performance and Predictability

**INV-WID-O1**
Identity comparison MUST be constant-time.

**INV-WID-DETERMINISTIC**
Given identical inputs, identity assignment is deterministic.

---

## SECTION L — Acceptance Criteria

SPEC-HAL-148 is satisfied when:

* [ ] All widgets have explicit stable identity
* [ ] Diffing matches widgets by identity
* [ ] Interaction state survives reflow and reorder
* [ ] Animations persist only with stable identity
* [ ] Virtualization requires explicit identity remap

---

## SECTION M — Authoritative Statements

**AUTH-148-1:** “Identity is the spine of continuity.”
**AUTH-148-2:** “If identity drifts, correctness is already lost.”
**AUTH-148-3:** “Nothing interactive exists without identity.”
