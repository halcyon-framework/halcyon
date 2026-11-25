# **SPEC-HAL-030: Presentation Policy & Swapchain Semantics**

**Author:** JC
**Date:** 2025-11-24 (revised 2026-01-xx)
**Status:** Design — Partially Implemented
**Scope:** Rendering Backend, Swapchain Configuration, Platform Presentation
**Purpose:** Define deterministic, perceptually responsive presentation behavior across platforms by formalizing present-mode selection, swapchain configuration, and compositor interaction rules.
**Binding amendment:** SPEC-HAL-030A fixes the canonical window-surface queue-depth hint without implementing the broader present-mode abstraction.

---

## 0. Status & Architectural Alignment (2026)

Since this SPEC was first written, Halcyon has gained:

* a unified timing model (SPEC-HAL-041)
* a canonical frame loop (SPEC-HAL-042)
* explicit idle throttling (SPEC-HAL-043)
* gesture-driven redraw escalation (Stage 42I)

As a result, **this SPEC no longer owns frame timing, pacing loops, redraw cadence, or animation clocks**.

Those concerns are now **explicitly out of scope**.

This SPEC is retained and narrowed to serve as the **single authority for presentation policy** — i.e. how rendered frames are *presented* to the OS compositor and display.

---

## 1. Purpose (Revised)

This SPEC defines Halcyon’s **presentation policy** at the boundary between:

```
render submission → swapchain → compositor → display
```

Its goals are to:

* minimize perceptual latency during direct manipulation
* avoid compositor-induced jitter or multi-frame backlog
* provide consistent *perceptual* behavior across platforms
* allow platform-specific behavior where required
* prevent ad-hoc present-mode hacks in renderer code

This applies to all Halcyon use cases, including but not limited to:

* data-dense UIs
* visualization tools
* editors
* trading or monitoring applications (example only)

The SPEC is **not trading-specific**.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define or control:

* frame timing or `dt` computation
  → SPEC-HAL-041
* frame loop structure or ordering
  → SPEC-HAL-042
* redraw scheduling or throttling
  → SPEC-HAL-043, Stage 42I
* widget interaction semantics
* animation or motion logic

Any attempt to reintroduce those here is a violation.

---

## 3. Problem Statement

Modern graphics stacks introduce **implicit buffering and scheduling** between an application and the display:

* GPU driver queues
* swapchain buffering
* compositor policies (DWM, Quartz, Wayland, etc.)

These layers can introduce **2–3 frames of latency**, especially with vsync-based present modes.

The result is most visible during **direct manipulation** (e.g. drag-resize):

* window chrome updates immediately
* content visually “chases” behind
* perceptual mismatch causes eye strain and loss of responsiveness

This effect varies by platform:

* macOS compositing tends to mask or soften the effect
* Windows DWM exposes it clearly
* relying on default WGPU behavior (`AutoVsync`) is insufficient

Halcyon requires **explicit control** over presentation semantics.

---

## 4. Core Principle

> **Presentation policy may differ per platform in order to achieve perceptual equivalence.**

Uniform code paths are *not* a requirement.
Uniform *user perception* is.

---

## 5. Present Mode Abstraction (Policy Level)

Halcyon defines the following conceptual presentation intents:

```
enum PresentIntent {
    LowLatency,     // prioritize minimal queueing
    BalancedVsync,  // low latency with vsync when possible
    StableVsync,    // maximum stability, higher latency acceptable
}
```

This is a **policy abstraction**, not a direct mirror of WGPU enums.

Mapping to backend present modes is platform-dependent.

---

## 6. Mapping to WGPU / Backend Modes

### Canonical Mapping

| PresentIntent | Preferred WGPU Mode | Fallback  |
| ------------- | ------------------- | --------- |
| LowLatency    | Mailbox             | Immediate |
| BalancedVsync | Mailbox             | Fifo      |
| StableVsync   | Fifo                | (none)    |

Notes:

* Mailbox is preferred where available due to zero queue backlog.
* Immediate is **never** used unguarded or as a default.
* Unknown or vendor-specific modes must be rejected and logged once.

---

## 7. Interactive Presentation Overrides

### 7.1 Rationale

Certain interactions (notably **interactive resize**) require temporarily lower latency than steady-state rendering.

A permanent global switch to low-latency modes is undesirable due to:

* power usage
* tearing risk
* unnecessary GPU churn

Therefore, Halcyon permits **temporary present-mode overrides** scoped to interaction phases.

### 7.2 Rules

* Overrides are:

  * temporary
  * interaction-scoped
  * platform-specific
* Overrides must:

  * switch back automatically
  * tolerate one dropped frame
  * never affect timing semantics

### 7.3 Initial Target (Non-Binding)

The first intended use is:

* **Windows interactive resize**

  * switch from FIFO → Mailbox (or Immediate fallback)
  * revert after resize stabilizes

macOS is explicitly excluded at this stage due to acceptable baseline behavior and differing compositor semantics.

---

## 8. Swapchain Reconfiguration Rules

Swapchain recreation is required when:

* present mode changes
* surface size changes
* DPI scale changes
* display refresh rate changes
* GPU device resets

Reconfiguration must:

* be synchronous
* preserve logical frame timing state
* never drop pending input or interaction state
* be idempotent if repeated

---

## 9. Relationship to Other SPECs

| Concern                   | Owning SPEC      |
| ------------------------- | --------------- |
| Timing / clocks           | SPEC-HAL-041     |
| Frame loop                | SPEC-HAL-042     |
| Idle throttling           | SPEC-HAL-043     |
| Gesture redraw escalation | Stage 42I       |
| **Presentation policy**   | **SPEC-HAL-030** |

SPEC-HAL-030 is the **only** place where presentation semantics may be defined.

---

## 10. Implementation Status

### Implemented

* Unified timing (external SPECs)
* Gesture-driven redraw escalation
* Deterministic frame loop
* Canonical `AutoVsync` window-surface queue depth (`desired_maximum_frame_latency == 2`) under SPEC-HAL-030A

### Not Implemented (This SPEC)

* Explicit present-mode selection
* PresentIntent abstraction
* Swapchain mode switching
* Platform-specific override logic
* Capability logging and sanitization

---

## 11. Expected Outcome (When Implemented)

When SPEC-HAL-030 is implemented within its revised scope:

* Direct manipulation feels responsive on all platforms
* Windows resize no longer exhibits multi-frame “catch-up”
* Platform-specific behavior follows binding amendments; SPEC-HAL-030A fixes
  the measured high-refresh Metal queue-depth defect without changing present mode
* Presentation behavior is intentional, explicit, and debuggable
* Renderer code no longer contains ad-hoc present-mode decisions

---

## 12. Final Note

This SPEC is intentionally **narrow**.

It does not attempt to control the engine.
It defines the **last uncontested boundary**: how frames reach the screen.

That boundary is now the primary source of perceptual inconsistency — and this SPEC exists to fix exactly that.
