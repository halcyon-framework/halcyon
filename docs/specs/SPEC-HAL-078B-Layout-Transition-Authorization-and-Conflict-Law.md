# SPEC-HAL-078B: Layout Transition Authorization and Conflict Law

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
**Layer:** Layout → Animation → Rendering Boundary
**Depends on:** SPEC-HAL-078, SPEC-HAL-078A
**Does NOT depend on:** Style system, animation DSL, widget policy
**Scope:** Engine-level conflict resolution and legality rules
**Non-Scope:** Timing, easing, interpolation math, paint mechanics

---

## 1. Motivation

SPEC-HAL-078 defines **mechanics**.
SPEC-HAL-078A defines **authorization**.

What remains unresolved is **conflict law**:
what happens when *multiple lawful systems* attempt to influence geometry in incompatible ways.

Examples:

* layout transition vs scroll transform
* layout transition vs viewport anchoring
* layout transition vs virtualization reuse
* layout transition vs overlay stacking
* layout transition vs render-time transforms

Without explicit conflict law, the engine risks:

* double application of motion
* visual desynchronization
* frame-order dependence
* non-local bugs that cannot be reasoned about

This SPEC defines **hard, engine-level precedence and exclusion rules** that make conflicts **illegal by construction**.

---

## 2. Architectural Position

This SPEC applies **after authorization (078A)** but **before visual application (Stage 3)**.

Pipeline becomes:

Stage 0 — Geometry Snapshot
Stage 1 — Eligibility Detection
Stage 1.5 — Authorization (SPEC-HAL-078A)
Stage 1.75 — Conflict Resolution (THIS SPEC)
Stage 2 — Transition Binding
Stage 3 — Visual Application

Authorization answers: *may this animate?*
Conflict law answers: *may this animate together with everything else?*

---

## 3. Design Principles

### PRINCIPLE-1: Single visual authority per axis

At most **one system** may visually influence a widget’s position or size on a given frame.

### PRINCIPLE-2: Layout is always authoritative

Layout output is the ground truth.
All animation is a *visual deviation*, never a state mutation.

### PRINCIPLE-3: Conflict resolution is global, not widget-local

No widget may opt out or override conflict law.

### PRINCIPLE-4: Conflicts resolve by denial, not blending

If two systems conflict, **one is denied**.
No mixing. No averaging. No heuristics.

---

## 4. Terminology

* **Visual Influence** — any non-layout modification of rect, transform, or clip
* **Conflict** — two or more visual influences targeting the same widget
* **Dominant System** — the system that wins the conflict
* **Suppressed System** — the system that is disabled for that widget/frame

**363/364 scoped amendment:** For SPEC-HAL-363/364, a participant is a certified source occurrence
with an explicit affected geometric channel set. One effective presentation
owner SHALL govern each participant/channel in a frame. Coupled affine channels
SHALL be arbitrated together. A group may consume an otherwise eligible ordinary
reflow projection only through an explicit engine ownership plan that suppresses
duplicate ordinary projection. Source identity, causal ownership and dependency
certificates determine eligibility, never animation progress/easing.

For this scope, PRINCIPLE-4's “one is denied” means: an already admitted owner
retains its channels unless an authorized replacement or higher-priority existing
authority requires settlement. Incompatible new groups without an authorized
replacement relationship SHALL be denied as complete groups. No area, width,
WidgetId or sibling motion-envelope heuristic selects a winner.

---

## 5. Conflict Detection Scope

Conflict law observes only:

* TreeWidgetId
* authoritative rect
* authorized transition candidates
* active scroll context
* active overlay context
* render-time transforms

It does **not** observe:

* animation progress
* easing
* widget type internals
* user code

**363/364 scoped amendment:** Static intersection of independent sibling swept envelopes is not a
conflict under this law. A conflict requires competing ownership of the same
affected contribution/channel or violation of an explicit input, coverage,
scroll, anchor or order contract. SPEC-HAL-363/364 adds certified occurrence
scope/channel facts to the listed geometry inputs, not widget internals,
progress/easing or user callbacks.

---

## 6. Hard Conflict Rules (Non-Negotiable)

### CONFLICT-1: Layout Transition vs Scroll Transform

If a widget is inside a scroll container that applies **visual scroll transforms**:

* Layout transition is **DENIED**

Reason:
Scroll already defines motion semantics. Dual motion is illegal.

This applies to:

* SceneBuilder.begin_scroll
* Batcher.push_scroll
* any render-time scroll transform

---

### CONFLICT-2: Layout Transition vs Virtualization Projection

If a widget’s rect originates from virtualization reuse or projection:

* Layout transition is **DENIED**

Reason:
Identity continuity is not real; rects are recycled.

---

### CONFLICT-3: Layout Transition vs Overlay Stacking

If a widget participates in:

* tooltip
* popover
* menu
* transient overlay layer

Layout transition is **DENIED**

Reason:
Overlay lifetimes are semantic, not spatial.

---

### CONFLICT-4: Layout Transition vs Viewport Anchoring

If rect movement is compensated by:

* scroll anchoring
* prepend/extend content offset stabilization

Layout transition is **DENIED**

Reason:
Anchor preservation has higher priority than animation.

---

### CONFLICT-5: Layout Transition vs Render-Time Rect Mutation

Any render-time system that mutates rects directly (not via paint input):

* forces layout transition **DENIAL**

Examples:

* Batcher.apply_scroll_to_rect
* any post-paint rect rewrite

---

## 7. Conditional Conflict Rules

### CONFLICT-COND-1: Nested Layout Transitions

Multiple layout transitions in ancestor/child relationships are allowed **only if**:

* they do not share scroll context
* neither participates in overlay routing

Otherwise, child transitions are denied.

**363/364 scoped amendment:** The rule above remains for ordinary nested layout transitions.
SPEC-HAL-363/364 ancestor/descendant contributions require one resolved ownership
plan over the actual leaf closure; two live projections SHALL NOT transform the
same leaf twice. A certified frozen inner expression is paint data, not an
independently running controller. Scroll/overlay/anchor authority is preserved
or admission fails.

---

### CONFLICT-COND-2: Clip vs Visual Rect

**363/364 scoped amendment:** Current/incoming authoritative clipping SHALL use canonical geometry
and scope, never a visual rect. Historical source-internal coverage and surviving
live barriers SHALL follow SPEC-HAL-364 C1–C10. A presentation mask is a distinct
admitted restriction. It cannot redefine authoritative clip or hit geometry.

---

## 8. Precedence Table

Highest → Lowest priority:

1. Layout correctness
2. Input & hit-testing
3. Scroll anchoring
4. Overlay semantics
5. Layout transitions
6. Render-time transforms

Lower-priority systems are suppressed automatically.

**363/364 scoped amendment:** For SPEC-HAL-363/364 the first four priorities remain unchanged.
The final two entries resolve as follows: eligible layout reflow and admitted
presentation composition are resolved into one effective projection before
render execution. Render execution consumes that projection and has no
independent competing motion authority. Ordinary hard conflict rules remain;
source admission cannot silently waive them.

---

## 9. Conflict Resolution Outcome

Each authorized candidate is reclassified as:

* ALLOWED
* DENIED(conflict_reason)

A denied candidate:

* still layouts correctly
* produces no animation
* produces no visual side effects

---

## 10. Debug Observability

Conflict denials must record:

* conflict rule ID
* dominant system
* suppressed system

Debug-only. No runtime cost.

---

## 11. Performance Invariants

* O(n) over authorized candidates
* zero allocations
* no tree traversal
* frame-local only

---

## 12. Explicit Non-Goals

This SPEC does NOT define:

* animation blending
* priority weights
* widget-level overrides
* user configuration
* callbacks or hooks

Any such addition violates this SPEC.

---

## 13. Relationship to Other SPECs

* SPEC-HAL-078 remains purely mechanical
* SPEC-HAL-078A decides **permission**
* SPEC-HAL-078B decides **coexistence legality**

All three are required for production safety.

---

## 14. Acceptance Criteria

This SPEC is complete when:

* Conflicting systems never affect the same widget visually
* Removing render-time scroll transforms does not break layout transitions
* Layout transitions never double-apply motion
* All conflict outcomes are deterministic

---

## 15. Final Statement

This SPEC makes layout transitions **safe in a real engine**.

No ambiguity.
No stacked motion.
No hidden coupling.

If two systems disagree — one is silenced.

---

## 16. Implementation Status

* Stage 1 (hierarchy conflicts): implemented
* Stage 2 (spatial conflicts): implemented
* Conflict resolution is deterministic and order-independent
* Enforced in `src/core/layout_transition.rs`
