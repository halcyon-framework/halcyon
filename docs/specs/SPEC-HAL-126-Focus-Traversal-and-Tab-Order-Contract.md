# SPEC-HAL-126

**Focus Traversal and Tab Order Contract**

**Status:** Design — Approved
**Scope:** Deterministic keyboard focus traversal and tab-order semantics
**Applies to:** All focusable widgets
**Layer:** Interaction / Navigation
**Depends on:**

* SPEC-HAL-007 (State & Event Routing)
* SPEC-HAL-111–125 (Widget interaction contracts, Accessibility Projection)

**Non-Goals:**

* Visual focus ring rendering
* Accessibility narration (SPEC-HAL-125)
* Pointer-based focus acquisition
* Platform-specific IME quirks

---

## 1. Purpose

This SPEC defines **global, invariant-based rules** for:

* keyboard focus traversal
* Tab / Shift+Tab behavior
* focus containment and escape
* predictable focus order across widget trees

Focus traversal is **infrastructure**, not widget flavor.

Any implementation that allows ad-hoc focus jumps, layout-dependent ordering, or implicit focus mutation is **incorrect by definition**.

---

## 2. Core Principles

### INV-F0 — Focus Is a Singleton

At any time:

* **zero or one** widget has focus
* never more than one
* never fractional
* never inferred

Focus is a **global state**, not per-container.

---

### INV-F1 — Traversal Is Deterministic

Given the same widget tree and state:

* Tab traversal order MUST be identical
* independent of layout, rendering order, or frame timing

---

## 3. Focus Eligibility

A widget is **focusable** iff:

* it explicitly declares `focusable = true`
* it is enabled
* it is visible
* it is not structurally hidden

---

### INV-F2 — No Implicit Focusability

Widgets MUST NOT become focusable because they:

* are clickable
* have hover
* contain text
* have children

Focusability is explicit or nonexistent.

---

## 4. Tab Order Model

### 4.1 Logical Focus Order

Focus order is derived from:

1. widget tree structure
2. explicit `tab_index` (optional)
3. container-defined traversal boundaries

Layout geometry is irrelevant.

---

### INV-F3 — Structural Authority

The **logical widget tree**, not layout or paint order, defines traversal order.

---

### 4.2 Tab Index

`tab_index` rules:

* default: `0`
* lower values come first
* equal values preserve tree order
* negative values remove widget from Tab traversal

---

### INV-F4 — Stable Ordering

Changing layout MUST NOT change focus order unless:

* widget structure changes, or
* `tab_index` changes explicitly

---

## 5. Tab Traversal Rules

### INV-F5 — Forward Traversal (Tab)

On `Tab`:

* move focus to next eligible widget
* wrap only if container allows wrap
* otherwise stop at boundary

---

### INV-F6 — Reverse Traversal (Shift+Tab)

On `Shift+Tab`:

* move focus backward
* obey same boundaries and eligibility rules

---

### INV-F7 — No Implicit Wrap

Wrapping focus (last → first) is:

* **forbidden by default**
* allowed only in explicitly marked containers (e.g. dialogs)

---

## 6. Focus Containers

Some widgets define **focus scopes**:

Examples:

* modal dialogs
* popovers
* menus
* dropdown popups

---

### INV-F8 — Focus Containment

When inside a focus container:

* Tab traversal MUST stay within container
* focus MUST NOT escape via Tab
* escape requires explicit action (e.g. Escape key)

---

### INV-F9 — Focus Entry Rule

When entering a focus container:

* focus moves to container-defined entry widget
* never remains on background widgets

---

## 7. Focus Exit Semantics

### INV-F10 — Explicit Exit Only

Focus leaves a container only if:

* container closes
* explicit focus transfer is requested
* Escape key is handled by container

Tab alone MUST NOT exit a focus scope.

---

## 8. Disabled and Hidden Widgets

### INV-F11 — Focus Skipping

Disabled or hidden widgets:

* MUST be skipped in traversal
* MUST NOT receive focus
* MUST NOT trap focus

If currently focused widget becomes disabled:

* focus MUST move to next eligible widget

**Scoped adopted execution — 2026-10-26:** SPEC-HAL-108 §13.5's JC-delegated
technical decision applies availability atomically through the existing engine
deferred executor. The focus owner selects next eligible within the current
scope, or None, with no implicit wrap, scope escape, neighbor auto-Capture or
focus resurrection. Release old Capture before transfer under SPEC-HAL-328 §3.9.
This selects no new traversal rule and preserves this document's existing status.

---

## 9. Dynamic Tree Changes

### INV-F12 — Focus Stability Under Mutation

If the focused widget is removed:

* focus relocates to nearest eligible neighbor
* never to root implicitly
* never to arbitrary widget

If no eligible widget exists:

* focus becomes `None`

---

## 10. Programmatic Focus Control

### INV-F13 — Explicit API Only

Programmatic focus change:

* MUST use explicit API
* MUST obey eligibility rules
* MUST emit focus change events

Silent focus jumps are forbidden.

---

## 11. Responsibility Boundaries

### Focus System Owns:

* global focus state
* traversal logic
* container boundaries
* Tab / Shift+Tab handling

### Widgets Own:

* declaring focusability
* reacting to focus gain/loss

### Widgets MUST NOT:

* move focus arbitrarily
* steal focus on hover
* auto-focus on render

---

## 12. Forbidden Behavior

Focus system MUST NOT:

* depend on pixel geometry
* depend on paint order
* auto-wrap without permission
* focus hidden widgets
* focus disabled widgets
* infer intent

---

## 13. Test Requirements

Implementations MUST test:

* deterministic tab order
* reverse traversal correctness
* focus containment
* focus relocation on removal
* disabled widget skipping
* no layout-dependent traversal

---

## 14. Reference Implementation

Planned module:

* `focus::traversal`

Consumes:

* widget tree
* focus scopes
* tab_index metadata

Produces:

* next/previous focus resolution

---

## 15. Final Note

Focus traversal is **muscle memory**.

If Tab behaves unpredictably even once,
users stop trusting the entire interface.

There is no such thing as a “mostly correct” focus system.
