# SPEC-HAL-078 — Layout Transitions v1

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
**Layer:** Core / Layout / Transition
**Scope:** Engine-level geometry transitions only
**Audience:** Engine implementers
**Policy:** No widget-level policy, no style animation policy

---

## 1. Motivation

Modern UI systems frequently suffer from **visual discontinuities** during structural layout changes:

* list insertions/removals snapping
* tree expand/collapse popping
* panels resizing abruptly
* reflow caused by content changes appearing as teleports

These issues are not caused by incorrect layout, but by the absence of a **temporal continuity layer** between successive layout solutions.

The purpose of Layout Transitions is to introduce **purely visual interpolation of geometry** across frames, **without modifying layout semantics, routing, focus, capture, or input authority**.

This SPEC defines the **engine-level contract** for layout transitions. It explicitly does **not** define widget APIs, animation policies, durations, or stylistic effects.

---

## 2. Core Principle

> **Layout transitions are paint-only geometry interpolation between two authoritative layout states.**

The layout engine remains the single source of truth.
Transitions never feed back into layout, routing, or input logic.

---

## 3. Definitions

* **from_rect**
  Geometry produced by the previous authoritative layout.

* **to_rect**
  Geometry produced by the current authoritative layout.

* **interpolated_rect**
  A visual-only rect derived from `(from_rect → to_rect)` as a function of time.

* **target geometry**
  The authoritative geometry (`to_rect`) used for all non-paint decisions.

* **ghost**
  An engine-owned paint record representing a widget that no longer exists in the layout tree but is still visually transitioning out.

**363/364 scoped amendment:** from_rect and to_rect SHALL retain their literal previous/current
authoritative-layout meanings. A presentation continuation endpoint is a distinct
SPEC-HAL-363/364 value and SHALL never be stored or reported as from_rect.
A ghost admitted by that pair is a historical paint occurrence, not a WidgetTree
node or a new semantic identity.

---

## 4. High-Level Flow

For each frame:

1. Routing and input resolution operate on **target geometry**.
2. Layout produces authoritative `to_rect` values.
3. Transition system matches nodes between frames.
4. Transition system evaluates interpolated geometry.
5. Paint uses interpolated geometry (or ghosts) where applicable.
6. When no transitions remain, system becomes a no-op.

---

## 5. Phase Placement (Authoritative)

**363/364 scoped amendment:** SPEC-HAL-042 owns the literal frame order. Pre-layout transition
bookkeeping belongs to the existing legal pre-layout owner boundary;
SPEC-HAL-363/364 control decisions and Timeline reservations belong to Update.
Canonical layout commits before geometry authorization/projection. Post-layout
work may capture/derive immutable transition and presentation values; it SHALL
NOT start, stop or mutate Timeline/control state. Custom preparation, retained
paint and rendering retain SPEC-HAL-042's order. No phase reordering is permitted.

---

## 6. Binding Invariants (LAW)

### LT-INV-1 — Target geometry is authoritative

The layout engine’s `to_rect` is the **only geometry allowed** to influence:

* hit-testing
* routing
* focus decisions
* capture decisions
* spatial acceleration structures
* scroll substrate math
* accessibility mapping

**Interpolated geometry is paint-only.**

---

### LT-INV-2 — No layout feedback

Interpolated geometry **must never** be used as input to:

* layout computation
* constraint resolution
* size measurement
* intrinsic sizing
* scroll offset calculation

Layout is a pure function of widget state + constraints + style.

---

### LT-INV-3 — Phase legality

* Pre-layout snapshot capture:

    * may read last-frame geometry
    * may write transition bookkeeping
    * may not mutate widget state

* Transition evaluation:

    * may interpolate geometry
    * may not mutate widget semantic state
    * may not run during layout or paint

* Layout:

    * may not read interpolated geometry

* Paint:

    * may read interpolated geometry
    * may not start, stop, or mutate transitions

**363/364 scoped amendment:** Permission to write transition bookkeeping SHALL NOT authorize writes
during paint or bypass SPEC-HAL-363's Update-only control boundary. Pure
frame-result construction is not a mutable transition-state update.

---

### LT-INV-4 — Single time authority

All transition evaluation must use **engine frame time** only.

* No `Instant::now()`
* No OS timestamps
* No wall-clock sampling

Time source must be the same as all other engine animations.

---

### LT-INV-5 — Identity matching is strict

Transition matching rules:

* Primary key: **TreeWidgetId**
* Scope: **sibling set under the same parent**
* Matching is deterministic or dropped

Rules:

* Duplicate IDs in a sibling set → affected nodes are excluded
* Missing IDs → excluded
* Parent mismatch → excluded
* No heuristics
* No fuzzy matching

If identity cannot be proven, **no transition occurs**.

---

### LT-INV-6 — Transition state is non-authoritative

* Transition system does not own widget state
* Transition system does not mutate widget state
* Transition state is engine-internal bookkeeping only

---

### LT-INV-7 — Exit transitions require ghosts

A disappearing widget may animate **only if**:

* its last painted geometry is captured
* a ghost paint record is created

Ghost record must include:

* last rect
* clip chain
* paint payload handle (or cached scene fragment reference)

Ghosts are engine-owned and not part of the widget tree.

**363/364 scoped amendment:** For a historical occurrence under SPEC-HAL-363/364, the minimum
payload list above is replaced by SPEC-HAL-364's complete certified source
closure, immutable paint/content generations, cue safety, coverage provenance,
dependencies and leases. A last rect plus an Arc or cached fragment is
insufficient. The occurrence remains engine-owned and outside the WidgetTree.

---

### LT-INV-8 — Ghost lifetime

A ghost is destroyed when **any** of the following occurs:

* its transition completes
* its clip context becomes invalid
* global transitions are disabled
* frame budget enforcement requires eviction (future extension)

No ghost may persist indefinitely.

**363/364 scoped amendment:** For SPEC-HAL-363/364 ghosts, logical retirement occurs on completion,
cancellation, invalid source/host/coverage, policy disable or an enforced bound.
Physical resource release follows the resource owner's safety rules.
No semantic teardown waits for a ghost. This replaces the lifetime list above
only for that admitted historical-value class.

---

### LT-INV-9 — Clip correctness

**363/364 scoped amendment:** Current/incoming presentation uses current authoritative clip geometry
and scope; neither origin nor extent is reconstructed from interpolation.
A historical occurrence preserves certified source-internal coverage topology
and transports it only as SPEC-HAL-364 §4 permits. Surviving external host,
surface and ancestor constraints remain live in their authoritative bases.
Presentation masks are additional paint restrictions and never replace either
class. Incomplete provenance refuses or settles the occurrence.

---

### LT-INV-10 — Input coherence during motion

* Routing, capture, focus use **target geometry**
* Hover/pressed visuals may follow interpolated geometry **only if** no contradiction occurs

If contradiction would occur (pointer appears over item visually but is not hit-testable):

> Visuals must snap to target geometry.

Input correctness overrides visual smoothness.

---

### LT-INV-11 — Continuous frame scheduling

If **any** layout transition (including ghosts) is active:

* the engine **must** request continuous frames
* idle throttling must be suspended until completion

---

### LT-INV-12 — Zero cost when idle

When no layout transitions are active:

* no per-frame heap allocation
* no full-tree scans beyond existing layout/paint passes
* transition system must be a fast no-op

---

### LT-INV-13 — Deterministic completion

Transition progress is a pure function of:

```
(start_time, duration, easing, frame_time.now)
```

No frame-rate dependent stepping.

---

## 7. Transition Eligibility Rules

A node may transition only if:

* it existed in the previous frame
* it exists in the current frame
* its identity matches under LT-INV-5
* its layout rect changed

Nodes that appear or disappear use **entry/exit rules**.

---

## 8. Entry Transitions (v1 behavior)

v1 does **not** define entry transitions by default.

* Newly appearing nodes render directly at `to_rect`
* Future SPECs may define optional entry behavior

**363/364 scoped amendment:** Default entry behavior remains immediate canonical presentation.
Explicit SPEC-HAL-363/364 entry admission MAY supply a presentation-only
occurrence; it SHALL NOT fabricate previous layout or implicit mount animation.

---

## 9. Exit Transitions (v1 behavior)

* Exiting nodes may animate via ghosts
* Ghosts are paint-only
* Ghosts do not participate in routing or input
* Ghost coverage obeys LT-INV-9 and SPEC-HAL-364 §4's certified internal/live split; an undifferentiated captured clip chain does not authorize replay.

---

## 10. Scroll Interaction Semantics

* Transitions operate in **logical (pre-scroll) space**
* Scroll offset is applied **after** interpolation
* Transition system must not modify scroll state
* Scroll virtualization may exclude off-screen nodes from transitions (future SPEC)

**363/364 scoped amendment:** Ordinary SPEC-HAL-078 transitions retain logical pre-scroll
interpolation followed by the existing scroll application. A SPEC-HAL-363/364
occurrence SHALL record actual pre/applied-scroll factorization and apply each
owner mapping once. This does not authorize animating scroll state or waive
SPEC-HAL-078A/078B ordinary scroll exclusions.

---

## 11. Performance Contract

* Transition bookkeeping must scale with:

    * number of transitioned nodes
    * not total scene size
* No O(n) scans per frame unless layout already does so
* Buffer reuse is mandatory

---

## 12. Explicit Non-Goals (v1)

This SPEC does **not** define:

* durations
* easing curves
* style interpolation
* opacity animation
* transform animation
* widget APIs
* user preferences
* accessibility animation policies

Those are layered on top.

---

## 13. Dependencies (Updated)

Hard dependencies:

* Unified frame timing
* Engine ticker / frame loop
* Stable widget identity
* Layout epochs
* Scene build pipeline

Non-dependencies:

* Style animation system
* Reactive signals
* Widget policy layers

---

## 14. Failure Mode Philosophy

When in doubt:

> **Drop the transition. Never guess.**

Visual smoothness must never compromise correctness, determinism, or input integrity.

---

## 15. Summary

Layout Transitions v1 introduce **visual continuity** without touching **semantic truth**.

They are:

* deterministic
* engine-driven
* identity-strict
* input-correct
* paint-only
* zero-cost when idle

This SPEC defines the **law** that all future layout transition behavior must obey.

---

## 16. Implementation Status

* Execution substrate: implemented (SPEC-HAL-145)
* Authorization gate: implemented (SPEC-HAL-078A)
* Conflict law: implemented (SPEC-HAL-078B)

