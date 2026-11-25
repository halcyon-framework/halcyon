# SPEC-HAL-081A — Compositor Enforcement and Invariant Hardening

**Status:** CLOSED (Fully Implemented)
**Closure Date:** 2026-01-18
**Freeze Date:** 2026-01-16
**Amendment Policy:** No changes without new SPEC. Bug fixes require SPEC amendment.
**Amends:** SPEC-HAL-081 (Compositor v1)
**Scope:** Compositor / Overlay / Multi-Window Enforcement
**Audience:** Compositor, Renderer, Window, Overlay implementers

> **CLOSURE NOTICE:** This SPEC is fully implemented and permanently closed as of 2026-01-18.
> All invariants are binding and enforced in code. No provisional state remains.
> See SPEC-HAL-081B for the companion multi-surface execution model (also closed).

---

## 1. Purpose and Binding Status

SPEC-HAL-081A is the **binding enforcement contract** for the Halcyon compositor subsystem.

* SPEC-HAL-081A is **binding**. Violations are implementation errors.
* SPEC-HAL-081 is **design-only** without 081A. It describes intent, not enforcement.
* This SPEC hardens the following by reference:
  * SPEC-HAL-081 (Compositor v1)
  * SPEC-HAL-150 (Compositor Authority and Limits)
  * SPEC-HAL-127 (Overlay Stack and Input Capture)
  * SPEC-HAL-137 / SPEC-HAL-137B (Overlay Hardening & Modal Boundaries)
  * SPEC-HAL-082 (Multi-Window Architecture)
  * SPEC-HAL-151 (Multi-Window Focus & Interaction Domains)

No invariant in those SPECs is enforceable until codified here or in a subsequent 081-series amendment.

---

## 2. Compositor Negative Authority Invariants

The compositor MUST NOT:

| ID | Invariant |
|----|-----------|
| INV-COMP-N01 | MUST NOT mutate widget geometry |
| INV-COMP-N02 | MUST NOT participate in layout computation |
| INV-COMP-N03 | MUST NOT resolve styles, themes, or visual properties |
| INV-COMP-N04 | MUST NOT own widgets |
| INV-COMP-N05 | MUST NOT own SceneGraph nodes |
| INV-COMP-N06 | MUST NOT generate visuals not backed by surfaces or layers |
| INV-COMP-N07 | MUST NOT interpret widget semantics |
| INV-COMP-N08 | MUST NOT transform input events beyond routing |
| INV-COMP-N09 | MUST NOT hold references to widget state |
| INV-COMP-N10 | MUST NOT create or destroy widgets |
| INV-COMP-N11 | MUST NOT perform hit-testing at any level (surface or widget) |
| INV-COMP-N12 | MUST NOT decide capture, dismissal, focus, or modality; it only exposes ordered surface descriptors |

---

## 3. Compositor Positive Authority

The compositor MAY ONLY:

| ID | Authority |
|----|-----------|
| INV-COMP-P01 | Order surfaces by z-index |
| INV-COMP-P02 | Composite layers into final output |
| INV-COMP-P03 | Expose ordered, inspectable SurfaceStack descriptors for upstream routing (EventFlow / router consumes) |
| INV-COMP-P04 | Hand off composed output to RenderGraph / swapchain |
| INV-COMP-P05 | Track surface lifetime (create, destroy, reorder) |
| INV-COMP-P06 | Apply surface-level occlusion (not widget-level) |
| INV-COMP-P07 | Expose surface bounds/clips/transforms for routing correctness; does not execute routing |

Any compositor behavior not listed above is FORBIDDEN.

---

## 4. Overlay Hardening Invariants

| ID | Invariant |
|----|-----------|
| INV-OVERLAY-01 | Overlays are compositor-owned surfaces, NOT widgets |
| INV-OVERLAY-02 | Overlay surface renders its own root subtree/SceneGraph; parent surface provides only anchoring/provenance, never rendering ownership |
| INV-OVERLAY-03 | Overlay lifetime is managed exclusively by the compositor |
| INV-OVERLAY-04 | Modal overlays REQUIRE upstream routing layer to enforce capture per SPEC-127/137/137B; compositor only provides ordering + provenance |
| INV-OVERLAY-05 | Non-modal overlays MUST NOT capture input outside their bounds |
| INV-OVERLAY-06 | Input capture precedence follows surface z-order (highest first) |
| INV-OVERLAY-07 | Overlay dismissal MUST release input capture immediately |
| INV-OVERLAY-08 | Nested modals are FORBIDDEN without explicit phase gate approval |
| INV-OVERLAY-09 | Overlays MUST NOT outlive their parent surface |
| INV-OVERLAY-10 | Overlay z-index MUST be strictly greater than base surface |

---

## 5. Multi-Window Preconditions

Multi-window implementation is FORBIDDEN until all preconditions are met.

| ID | Precondition |
|----|--------------|
| PRE-MW-01 | Single-window compositor exists and passes all INV-COMP invariants |
| PRE-MW-02 | Overlay invariants (INV-OVERLAY-*) are enforced and tested |
| PRE-MW-03 | SPEC-HAL-150 authority boundaries are implemented |
| PRE-MW-04 | Input routing correctly respects surface boundaries |

**Multi-Window Invariants (effective only after preconditions met):**

| ID | Invariant |
|----|-----------|
| INV-MW-01 | Each window MUST have exactly one compositor instance |
| INV-MW-02 | Cross-window input routing is FORBIDDEN |
| INV-MW-03 | Cross-window surface sharing is FORBIDDEN |
| INV-MW-04 | Window focus is managed by platform, not compositor |
| INV-MW-05 | Compositor MUST NOT assume window ordering knowledge |

---

## 6. Phase Gates and Stop Conditions

### 6.1 Compositor Phase 1 — Core Surfaces (FROZEN 2026-01-16)

Phase 1 is complete. ALL requirements verified:

* [x] Single window with compositor-managed surface stack
* [x] INV-COMP-N01 through INV-COMP-N12 verified (no violations)
* [x] INV-COMP-P01 through INV-COMP-P07 implemented
* [x] Base surface renders correctly via RenderGraph

**Code-Level Seal:**
* `BaseContract` is the ONLY production contract (sealed trait)
* `SceneKey` flow: Compositor → snapshot → begin_frame → batches (mandatory by construction)
* `MultiSurfaceContract` placeholder exists but is NOT implemented (SPEC-HAL-081B seam)

### 6.2 Compositor Phase 2 — Overlays

Phase 2 may begin only after Phase 1 is complete. Phase 2 is complete when ALL of the following are true:

* [ ] Upstream routing/capture enforcement exists per SPEC-127/137/137B
* [ ] At least one overlay type (tooltip or dropdown) functional
* [ ] INV-OVERLAY-01 through INV-OVERLAY-05 verified

### 6.3 Stop Conditions

Implementation MUST HALT immediately if any of the following occur:

| ID | Condition |
|----|-----------|
| STOP-01 | Compositor directly modifies widget geometry |
| STOP-02 | Compositor participates in layout pass |
| STOP-03 | Compositor owns widget references |
| STOP-04 | Input events bypass surface routing |
| STOP-05 | Overlay outlives parent surface |
| STOP-06 | Multi-window work begins before PRE-MW-* satisfied |
| STOP-07 | Cross-window input routing attempted |
| STOP-08 | Any compositor code traverses widget tree or calls hit-test APIs (surface or widget) |
| STOP-09 | Overlay work begins before Phase 1 is complete |

Upon STOP condition: cease implementation, file deviation report, await SPEC amendment.

---

## 7. Non-Goals and Explicit Exclusions

This SPEC exists to prevent the following failure modes:

| Failure Mode | Prevention |
|--------------|------------|
| Compositor becomes layout engine | INV-COMP-N02 |
| Compositor owns widgets | INV-COMP-N04, INV-COMP-N10 |
| Overlays treated as widgets | INV-OVERLAY-01 |
| Compositor performs hit-testing or routing | INV-COMP-N11, INV-COMP-N12, STOP-08 |
| Premature multi-window implementation | PRE-MW-*, STOP-06 |
| Cross-window state leakage | INV-MW-02, INV-MW-03, STOP-07 |
| Unbounded overlay nesting | INV-OVERLAY-08 |
| Zombie overlays after parent death | INV-OVERLAY-09, STOP-05 |

---

## 8. Amendment Rules

Any of the following require SPEC-HAL-081A amendment:

* Adding compositor authority
* Relaxing any MUST NOT invariant
* Modifying phase gate criteria
* Enabling multi-window before preconditions

No silent drift is permitted.

---

## 9. Final Status

**SPEC-HAL-081A is the sole binding compositor enforcement contract.**

SPEC-HAL-081 and related design SPECs describe intent.
SPEC-HAL-081A defines what is permitted and what is forbidden.
