**SPEC-HAL-097A — Motion Authorization & Layout Transition Law**

**Status:** REJECTED / RETIRED / INACTIVE / NOT BINDING
**Owner disposition:** JC, 2026-11-04 — HAL-REJECT-FRAMEWORK-WIDE-AESTHETIC-MOTION-BANS-01

## JC owner disposition — 2026-11-04

JC rejects this proposal's generic all-spatial-motion authorization requirement.
The claims that motion is a privilege requiring an additional permit, that every
spatial effect must enter the proposed HAL-078/078A/078B pipeline, and that motion
otherwise must not exist are retired, not pending approval. The blanket denials
and imported aesthetic restrictions from
[HAL-097](SPEC-HAL-097-Motion-and-Micro-Animation-Rules.md) are not universal law.

Applications own their artistic expression. Spring, bounce,
overshoot, rotation, scale and long transitions are not categorically prohibited
by Halcyon. There is no replacement authorization pipeline, per-animation permit,
genre test, curve/duration restriction, mode exception or Showcase-only exemption.
Products use the framework's actual capabilities; this documentary disposition
does not implement an unavailable capability or authorize runtime work.

[HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), binding
[HAL-021](SPEC-HAL-021-Animations.md), and separately adopted widget, geometry,
input, rendering and lifetime contracts retain their exact scopes. HAL-021A's
existing status and pressed-local scope are unchanged; HAL-363/364 remain Proposed.
A dependency, cross-reference, proposed amendment or historical final-law claim
cannot adopt or revive this rejected proposal or the rejected
[HAL-332](SPEC-HAL-332-Professional-Visual-Charter.md).

## Historical proposal — provenance only

The entire original proposal below, including its former Engine Law/Scope labels,
authorization pipeline, prohibitions and Final Law, is historical nonbinding
provenance. It supplies no current motion-approval system or implementation task.

**Historical proposal status:** PROPOSED; never adopted as universal framework law.
**Layer:** Engine Law / Motion Authorization
**Scope:** All motion that affects spatial position, size, or layout continuity
**Depends On:**

* SPEC-HAL-078 — Layout Transitions (core mechanism)
* SPEC-HAL-078A — Layout Transition Authorization
* SPEC-HAL-078B — Layout Transition Conflict Law
* SPEC-HAL-097 — Motion & Micro-Animation Rules

---

## 1. Purpose

This SPEC defines **when motion is legally allowed to occur**.

It does **not** define:

* timing
* easing
* curves
* durations
* stylistic rules

Those remain fully owned by **SPEC-HAL-097**.

This SPEC exists to prevent **illegal motion**, **double authority**, and **spatial lies**.

---

## 2. Core Principle

> **Motion is not an entitlement. Motion is a privilege granted by authority.**

Any motion that changes:

* widget position
* widget size
* widget bounds
* perceived spatial continuity

**must be authorized** before it is executed.

Unauthorized motion is **architecturally illegal**, even if visually subtle.

---

## 3. Motion Classes

This SPEC classifies motion into two legal categories:

### 3.1 Non-Spatial Motion (Always Allowed)

Motion that does **not** affect layout geometry:

* opacity
* color
* shadow opacity
* border color
* glow
* underline
* caret blink
* shimmer
* progress fill

These motions:

* do not require layout transition authorization
* do not participate in conflict resolution
* are governed solely by SPEC-HAL-097

---

### 3.2 Spatial Motion (Authorization Required)

Any motion that affects:

* x / y position
* width / height
* bounds
* clipping region
* perceived spatial continuity

Including but not limited to:

* layout transitions
* height expand / collapse
* slide-in / slide-out
* offset-based reveal
* elevation that changes hit geometry
* any transform that substitutes for layout movement

All such motion **must** pass through the Layout Transition Pipeline.

---

## 4. Mandatory Authorization Path

All spatial motion **must** satisfy the following pipeline:

```
Rect Change
  → SPEC-HAL-078 Stage 1: Candidate Detection
  → SPEC-HAL-078A: Authorization
  → SPEC-HAL-078B: Conflict Resolution
  → SPEC-HAL-078 Stage 2+: Binding & Execution
```

If a motion cannot enter this pipeline, it **must not exist**.

---

## 5. Hard Prohibitions

The following are strictly forbidden:

### 5.1 Render-Time Rect Mutation

No system may:

* mutate rects during paint
* apply layout-affecting transforms at render time
* simulate motion by bypassing layout ownership

All spatial motion must originate from layout authority.

---

### 5.2 Dual Authority Motion

A widget may not be simultaneously affected by:

* scroll transforms **and**
* layout transitions

or:

* virtualization projection **and**
* spatial animation

Conflicts resolve by **denial**, not blending.

---

### 5.3 Semantic Lifetime Overrides

Widgets participating in:

* overlays
* tooltips
* popovers
* menus
* transient UI layers

may not receive layout transitions unless explicitly authorized by a future SPEC.

---

## 6. Relationship to SPEC-HAL-097

SPEC-HAL-097 defines **how motion looks**.
SPEC-HAL-097A defines **whether motion is allowed to exist**.

Rules from SPEC-HAL-097 **only apply after** authorization succeeds.

If authorization fails:

* no easing
* no duration
* no fallback animation
* no silent downgrade

The motion simply does not occur.

---

## 7. Determinism Guarantee

This SPEC enforces:

* single spatial authority per axis
* deterministic motion outcomes
* identical behavior across frame rates
* zero ambiguity in motion ownership

There is no “best effort” motion.

---

## 8. Performance Invariants

* O(n) over transition candidates
* no per-frame tree scans beyond precomputed flags
* zero allocations in hot paths
* frame-local state only
* no impact on steady-state rendering

Authorization must be cheaper than animation itself.

---

## 9. Non-Goals

This SPEC does **not**:

* define new animation APIs
* add new motion types
* permit additional effects
* soften restrictions from SPEC-HAL-097
* introduce heuristics or guesses

---

## 10. Final Law

> **If motion changes space, it must be authorized.
> If it is not authorized, it must not happen.**

No exceptions.

---

## Disposition confirmation

The historical proposal remains REJECTED / RETIRED / INACTIVE / NOT BINDING.
Its Mandatory Authorization Path and Final Law are rejected historical text,
not a current condition for motion to exist.
