# SPEC-HAL-149: Virtualized Structures with Persistent State

**Status:** Binding
**Layer:** Structural Virtualization, State Continuity
**Scope:** Virtualized lists, trees, and grids with stable interaction, focus, and selection state
**Depends on:** SPEC-HAL-083, SPEC-HAL-135, SPEC-HAL-136, SPEC-HAL-148
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Virtualization is required for performance, but naïve virtualization destroys correctness:

* focus disappears when items scroll out
* selection collapses or jumps
* range anchors reset unpredictably
* animations restart or snap
* interaction state leaks between recycled items

Most UI systems treat virtualization as **instance reuse**.
This is structurally wrong.

Halcyon requires virtualization to preserve **logical continuity**, not visual coincidence.

This SPEC defines how virtualized structures retain **persistent state** across creation,
destruction, reuse, and reflow — without ghost state or implicit coupling.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Virtualization is structural, not semantic**
Only presentation is virtualized; logical entities persist.

**PRINCIPLE-2: Identity precedes existence**
State binds to identity, not live widget instances.

**PRINCIPLE-3: Absence is not destruction**
Leaving the viewport does not invalidate state.

**PRINCIPLE-4: Reuse is explicit**
Instance reuse must never imply state reuse.

**PRINCIPLE-5: State must be lossless**
No interaction state may be lost due to virtualization.

---

## SECTION C — Virtualized Structure Definition

A **Virtualized Structure** is a container that:

* presents only a subset of logical items
* creates/destroys widget instances dynamically
* preserves logical item identity and state

Examples:

* large lists
* trees
* tables
* infinite scroll views

---

## SECTION D — Identity Binding

Each logical item in a virtualized structure MUST have:

* a stable logical identity (SPEC-HAL-148)
* independent of viewport presence

**INV-VIRT-ID-STABLE**
Logical identity MUST persist regardless of visibility.

Widget instances may come and go; identity does not.

---

## SECTION E — State Persistence Rules

The following states MUST persist across virtualization:

* selection
* focus
* navigation cursor
* range anchors
* transactional selection state
* animation progress (if applicable)

**INV-VIRT-STATE-PERSIST**
Virtualization MUST NOT clear or reset interaction state.

---

## SECTION F — Instance Reuse Contract

Widget instances MAY be reused, but:

**INV-VIRT-REUSE-EXPLICIT**
Instance reuse MUST explicitly rebind identity and state.

**INV-VIRT-NO-LEAK**
No state from a previous identity may leak into a new one.

---

## SECTION G — Focus and Navigation

Focus and navigation cursor operate on **logical items**, not instances.

**INV-VIRT-FOCUS-LOGICAL**
Focus MUST remain valid even when the item is not currently realized.

When an off-screen focused item re-enters view, focus MUST be restored.

---

## SECTION H — Selection and Ranges

Range selection and transactional semantics MUST survive virtualization.

**INV-VIRT-RANGE-STABLE**
Range anchors and base selection MUST NOT depend on instance lifetime.

**INV-VIRT-TRX-STABLE**
Transactional selection MUST remain valid across virtualization boundaries.

---

## SECTION I — Animation and Transitions

Animations bound to logical identity MAY continue across virtualization gaps.

**INV-VIRT-ANIM-IDENTITY**
Animation continuation is permitted only if identity matches.

If identity is absent for too long, animation termination MUST be explicit.

---

## SECTION J — Scroll and Layout Interaction

Virtualization MUST NOT alter scroll semantics.

**INV-VIRT-SCROLL-INDEPENDENT**
Scroll position and intent MUST NOT be coupled to instance creation.

---

## SECTION K — Forbidden Behaviors

The following are categorically forbidden:

* binding state to widget instances
* clearing state on unmount
* implicit state reset on reuse
* focus tied to viewport presence
* selection tied to render order

---

## SECTION L — Performance and Predictability

**INV-VIRT-O1-LOOKUP**
State lookup by identity MUST be constant-time.

**INV-VIRT-DETERMINISTIC**
Virtualization behavior MUST be deterministic under identical inputs.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-149 is satisfied when:

* [ ] Virtualized lists preserve focus and selection off-screen
* [ ] Reordering does not reset interaction state
* [ ] Instance reuse never leaks state
* [ ] Transactional selection works across virtualization
* [ ] Animations bind strictly to identity

---

## SECTION N — Authoritative Statements

**AUTH-149-1:** “Virtualization hides widgets, not meaning.”
**AUTH-149-2:** “State belongs to identity, not to pixels.”
**AUTH-149-3:** “Performance is never an excuse to forget.”
