# SPEC-HAL-082A — Multi-Window Architecture Amendment

**Status:** Binding
**Amends:** SPEC-HAL-082 — Multi-Window Architecture
**Authority:** SPEC-HAL-002 (Constitution and Architectural Invariants)
**Scope:** Authority, ownership, and sovereignty clarification
**Non-Goal:** No implementation steps are mandated by this amendment

---

## 0. Status Declaration

SPEC-HAL-082 remains authoritative **for intent and goals only**.

All structural, ownership, and authority semantics in SPEC-HAL-082 are **overridden** where they conflict with binding
SPECs listed in this amendment.

This document is a **constitutional amendment** and is binding.

---

## 1. Superseded Semantics

SPEC-HAL-082 **MUST yield** to the following binding SPECs wherever conflicts exist:

* **SPEC-HAL-002** — Constitution and Architectural Invariants
* **SPEC-HAL-150** — Compositor Authority and Limits
* **SPEC-HAL-151** — Multi-Window Focus and Interaction Domains
* **SPEC-HAL-081B** — Multi-Surface Compositor
* **SPEC-HAL-081C** — Overlay Surface Integration
* **SPEC-HAL-030** — Presentation Policy and Swapchain Semantics
* **SPEC-HAL-154** — Sealed-After-Layout Deferred Wiring Contract

Any interpretation of SPEC-HAL-082 that violates the above is **invalid**.

---

## 2. Window Redefinition

A *window* in Halcyon is **not** an owning runtime authority.

A window is strictly defined as:

* an operating-system–provided surface binding
* a stable identity handle (`WindowId`)
* an ingress point for raw platform events

A window **MUST NOT** own, control, or mutate:

* compositors
* renderers
* widget trees
* layout roots
* interaction routing
* focus, capture, or modality semantics

All such authority is governed exclusively by Halcyon SPECs.

---

## 3. OS Sovereignty Boundary (Binding)

Per **SPEC-HAL-002 — Engine Sovereignty**:

* The operating system provides **transport primitives only**.
* OS signals (focus, activation, visibility) are **advisory inputs**.
* OS signals MAY be ignored, transformed, deferred, or reinterpreted.

No OS callback or platform signal may directly mutate Halcyon semantic state.

---

## 4. HalcyonWindow

If a `HalcyonWindow` aggregate exists, it is:

* optional
* non-authoritative
* a convenience container only

`HalcyonWindow` **MUST NOT** own, implicitly construct, or mutate any state governed by:

* interaction (SPEC-HAL-151)
* layout (SPEC-HAL-142 / 145 / 146 / 158)
* composition (SPEC-HAL-150 / SPEC-HAL-081B / SPEC-HAL-081C)
* rendering or presentation (SPEC-HAL-030)

Any such ownership is illegal.

---

## 5. WindowManager

A `WindowManager`, if present, is limited to:

* `WindowId` allocation
* OS window creation and destruction
* OS visibility control (show / hide)
* maintaining a registry of active windows

A `WindowManager` is **explicitly forbidden** from:

* routing events
* managing focus or capture
* controlling layout or geometry
* owning compositors or renderers
* making semantic decisions of any kind

Violation constitutes authority bleed.

---

## 6. Per-Window State (Hard Rule)

The **only mandatory per-window state** is:

* `InteractionDomainState` as defined by **SPEC-HAL-151**

All other state is:

* surface-scoped
* system-scoped
* or explicitly wired by higher-level engine code

Implicit per-window state creation is forbidden.

---

## 7. Rendering Model Clarification

Rendering authority is defined as follows:

* A single shared GPU device is used.
* Swapchains are bound **per surface**, not per window.
* Renderer instances may exist per surface.
* Draw order is determined **exclusively** by compositor surface stacks.

Windows have **no authority** over rendering order or composition semantics.

---

## 8. Child and Transient Windows

OS child windows, tool windows, dialogs, inspectors, or transient windows:

* MAY exist as OS-level constructs
* MAY have OS parent/child relationships

Such relationships are **presentation-level only**.

They **MUST NOT** introduce semantic authority.

All hierarchy, modality, focus, capture, and routing semantics remain governed by Halcyon.

---

## 9. Explicit Non-Goals

The following are explicitly deferred and out of scope:

* multi-monitor placement policy
* window persistence and restore
* window type taxonomy (tool, dialog, inspector, etc.)
* cross-window drag-and-drop

No behavior in these areas is implied.

---

## 10. Intent Preservation

This amendment does **not** change the intent of SPEC-HAL-082.

It clarifies authority boundaries to align SPEC-HAL-082 with the binding Halcyon Constitution and the current engine
architecture.

---

**End of SPEC-HAL-082A**
