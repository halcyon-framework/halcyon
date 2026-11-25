# SPEC-HAL-078A: Layout Transition Authorization & Policy Gate (v1)

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
**Layer:** Layout → Animation Boundary
**Depends on:** SPEC-HAL-078
**Does NOT depend on:** Style, easing, widget policy, animation DSL
**Scope:** Engine-level authorization only
**Non-Scope:** Timing, easing, interpolation, paint mechanics

---

## 1. Motivation

SPEC-HAL-078 intentionally implements **mechanics without policy**:

* geometry snapshot
* eligibility detection
* animation binding
* paint-time interpolation

This is correct — but incomplete for a production engine.

Without an explicit **authorization layer**, layout transitions become:

* implicit
* contagious
* hard to reason about
* impossible to globally disable or audit

This SPEC introduces a **mandatory policy gate** that decides:

> *Which eligible layout transitions are allowed to become visual animations.*

No authorization → **no animation**, even if mechanics exist.

---

## 2. Architectural Positioning

This SPEC inserts a **strict filter phase**:

```
Stage 0  Geometry Snapshot
Stage 1  Eligibility Detection (rect changed)
Stage 1.5  AUTHORIZATION  ← (this SPEC)
Stage 2  Transition Binding
Stage 3  Visual Application
```

**Key rule:**
**363/364 scoped amendment:** An ordinary layout transition SHALL pass geometry authorization before
transition binding. A SPEC-HAL-363 request MAY create a bounded Timeline timing
reservation in Update after preliminary request admission and before current
layout is known. Reservation SHALL confer no visual authorization.
SPEC-HAL-363/364 complete post-layout/source admission MUST succeed in that same
request frame before first publication; otherwise the reservation expires.

---

## 3. Design Principles

### PRINCIPLE-1: Mechanics and policy are separate

Authorization must not know how animation works.

### PRINCIPLE-2: Default is denial

Transitions animate **only if explicitly allowed**.

### PRINCIPLE-3: Deterministic and pure

Authorization must be:

* side-effect free
* frame-local
* deterministic

### PRINCIPLE-4: Engine-level only

No widget-local heuristics. No callbacks. No user code.

---

## 4. Terminology

* **Candidate** — output of SPEC-HAL-078 Stage 1
* **Authorized Transition** — candidate allowed to animate
* **Denied Transition** — candidate rejected (still layouts correctly)

---

## 5. Authorization Phase Definition

### 5.1 Entry

Input:

* `&[LayoutTransitionCandidate]`

Output:

* `&mut Vec<LayoutTransitionCandidate>` (filtered in-place or copied)

No allocation required.

---

## 6. Authorization Invariants

### AUTH-INV-1: Authorization is mandatory

No transition may reach Stage 2 without passing authorization.

### AUTH-INV-2: Authorization is pure

No mutation of:

* WidgetTree
* TransitionLayer
* Timeline
* LayoutTransitionStore

### AUTH-INV-3: Authorization does not observe animation state

Only geometry + identity are visible.

### AUTH-INV-4: Authorization is deterministic

Same input → same output.

### AUTH-INV-5: Authorization is frame-local

No cross-frame memory.

**363/364 scoped amendment:** AUTH-INV-1–5 remain a pure, deterministic, frame-local geometry filter,
independent of animation progress/easing. The filter SHALL neither read retained
animation history nor mutate Timeline, Tree or transition control.
SPEC-HAL-363/364 lifecycle and coverage admission are distinct owner checks;
they SHALL NOT be smuggled into this filter as widget callbacks.

---

## 7. Hard Denial Rules (Non-Negotiable)

The following transitions are **always denied**:

### AUTH-DENY-1: Virtualized nodes

Any node whose layout rect is produced by virtualization projection.

Reason:

* rects are recycled
* identity continuity is false

### AUTH-DENY-2: Scroll-induced movement

Any rect change caused by:

* scroll offset
* scroll synchronization
* scroll reveal

Reason:

* scrolling already has its own motion model

### AUTH-DENY-3: Ephemeral widgets

Including but not limited to:

* tooltips
* popovers
* menus
* transient overlays

Reason:

* appear/disappear semantics override interpolation

### AUTH-DENY-4: Text editing surfaces

Including:

* TextArea
* TextEdit
* inline caret containers

Reason:

* caret + selection must remain pixel-stable

### AUTH-DENY-5: Reparented widgets

If TreeWidgetId is preserved but parent path changes.

Reason:

* semantic identity changed even if ID did not

---

## 8. Conditional Authorization Rules

These rules may allow animation **only if all conditions pass**.

### AUTH-COND-1: Stable identity

* Same TreeWidgetId
* Same parent path
* Same layout role

### AUTH-COND-2: Structural stability

* No subtree insertion or removal between frames
* Only rect delta, no topology delta

### AUTH-COND-3: Non-interactive widget

Widgets with active input capture are denied.

**363/364 scoped amendment:** AUTH-DENY-1–5 and AUTH-COND-1–3 govern ordinary same-widget layout
transitions. SPEC-HAL-363/364 MAY admit an already-realized stable member,
explicit enter/exit, replacement or reparent paint source through its own
certified source/profile contract. This SHALL NOT authorize ordinary
virtualization-based rect matching, offscreen realization, scroll-induced
animation, overlay-lifetime interference, text-editor motion or capture/input
contradiction. Each source profile must identify the owner law it satisfies and
the dependencies it preserves. An unadmitted profile is PendingCapability; a
failed source proof is refusal. Preserved WidgetId alone SHALL NOT prove
reparent continuity. The ordinary denial rules are not deleted.

---

## 9. Authorization Result Encoding

Each candidate must be classified as:

```
AUTHORIZED
DENIED(reason)
```

### AUTH-INV-6: Reasons are recorded

Denial reason must be tagged for debug tooling.

### AUTH-INV-7: Reasons do not affect runtime behavior

They are observational only.

---

## 10. Debug Visibility (Non-Runtime)

Authorization reasons must be:

* inspectable in debug builds
* invisible in release behavior

No logging. No panics.

---

## 11. Non-Goals (Explicit)

This SPEC does **not** define:

* animation duration
* easing curves
* per-widget animation styles
* user customization
* scripting hooks
* callbacks
* observers

Any attempt to add those here is a violation.

---

## 12. Performance Guarantees

### PERF-INV-1: O(n) over candidates

No tree walks.

### PERF-INV-2: Zero allocations

Caller owns buffers.

### PERF-INV-3: Zero cost when no candidates

Fast-exit.

---

## 13. Failure Modes (Defined)

* If authorization rejects **all** candidates → no layout animations
* Layout correctness is unaffected
* Input, hit-testing, scrolling unchanged

This is a safe failure.

---

## 14. Relationship to SPEC-HAL-078

* SPEC-HAL-078 remains **unchanged**
* 078A is strictly additive
* 078 can be frozen after this

---

## 15. Versioning

This is **v1**.

Future extensions:

* 078B — Animation style policy
* 078C — User-level overrides

Those are explicitly out of scope.

---

## 16. Acceptance Criteria

SPEC-HAL-078A is complete when:

* Authorization is enforced before Stage 2
* All hard denial rules are implemented
* No visual change occurs without authorization
* Removing 078A disables all layout animations globally

---

## 17. Final Statement

This SPEC converts layout transitions from a **mechanical side-effect**
into a **governed engine feature**.

No surprises. No magic. No regrets.

---

## 18. Implementation Status

Authorization gate enforced prior to conflict resolution.
Implemented in `src/core/layout_transition.rs`.
