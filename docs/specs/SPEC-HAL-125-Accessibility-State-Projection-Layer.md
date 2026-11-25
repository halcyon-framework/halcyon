# SPEC-HAL-125

**Accessibility State Projection Layer (Non-UI)**

**Status:** Design — Approved
**Scope:** Projection of UI state into accessibility-facing semantic state
**Applies to:** All interactive widgets
**Layer:** Non-UI / Non-visual
**Depends on:**

* SPEC-HAL-111–124 (all interaction + state SPECs)
* SPEC-HAL-126 (Focus Traversal & Tab Order)

**Non-Goals:**

* Visual rendering
* Screen reader APIs (platform-specific)
* Voice input
* Accessibility tree layout
* Styling, animation, theming

---

## 1. Purpose

This SPEC defines a **pure projection layer** that translates internal widget state into **accessibility-relevant semantic state**.

This layer:

* does **not** own interaction
* does **not** own rendering
* does **not** mutate widget state
* does **not** invent behavior

It is a **read-only semantic mirror**.

Any implementation that allows accessibility to *change* widget behavior is **incorrect by definition**.

---

## 2. Core Principle

### INV-A11Y-0 — Projection, Not Control

Accessibility is a **consumer of state**, never a controller.

The accessibility layer:

* observes widget state
* projects semantic meaning
* emits accessibility events

It MUST NOT:

* trigger actions
* mutate focus
* change selection
* synthesize input

---

## 3. Accessibility Node Model

### 3.1 Accessibility Node

Each interactive widget MAY expose an **AccessibilityNode**.

An AccessibilityNode contains:

* stable `A11yId`
* semantic role
* state flags
* relationships (parent/children)
* optional textual description

It is **derived**, never authoritative.

---

### INV-A11Y-1 — Stable Identity

`A11yId` MUST:

* be stable for widget lifetime
* survive layout, scrolling, virtualization
* map 1:1 to logical widget identity

Recycling visual widgets MUST NOT recycle `A11yId`.

---

## 4. Roles

Each widget projects **exactly one role**.

Examples (non-exhaustive):

* Button
* Checkbox
* RadioButton
* ToggleSwitch
* List
* ListItem
* Tree
* TreeItem
* Menu
* MenuItem
* TextField
* Slider

Roles MUST be:

* explicit
* static per widget type
* never inferred dynamically

---

### INV-A11Y-2 — Role Stability

A widget’s role MUST NOT change at runtime.

If behavior changes → new widget, not new role.

---

## 5. State Flags

Accessibility state flags are **pure projections** of widget state.

Common flags:

* `focused`
* `selected`
* `expanded`
* `checked`
* `disabled`
* `hidden`
* `pressed` (transient)

---

### INV-A11Y-3 — No Synthetic State

Accessibility flags MUST map directly to real widget state.

Forbidden:

* “virtual focus”
* “implied selection”
* “predicted expansion”

If state does not exist internally, it MUST NOT appear in accessibility.

---

## 6. Focus Semantics

### INV-A11Y-4 — Focus Mirrors Focus System

Accessibility focus:

* MUST reflect the **actual focused widget**
* MUST NOT diverge
* MUST NOT lag

If no widget has focus → no accessibility focus.

---

### INV-A11Y-5 — Focus Visibility

Hidden or non-realized widgets:

* MUST NOT receive accessibility focus
* MUST NOT be navigable

Applies to:

* collapsed tree nodes
* virtualized offscreen items
* hidden popups

---

## 7. Selection Semantics

Selection is projected from:

* SPEC-HAL-114 (List)
* SPEC-HAL-116 (Tree)
* SPEC-HAL-121 (DnD reordering, if applicable)

---

### INV-A11Y-6 — Selection Is Observable, Not Actionable

Accessibility:

* MAY observe selection
* MUST NOT modify selection

All selection changes originate from interaction logic only.

---

## 8. Hierarchy Projection

Accessibility hierarchy MUST mirror **logical hierarchy**, not visual layout.

Examples:

* TreeItem children reflect tree structure, not indentation
* Virtualized items exist logically even if not realized

---

### INV-A11Y-7 — Logical Hierarchy Authority

Hierarchy source of truth:

* Tree model for trees
* List model for lists
* Menu model for menus

Layout tree is irrelevant.

---

## 9. Event Emission

Accessibility events are emitted on **state transitions** only.

Examples:

* focus gained/lost
* selection changed
* expanded/collapsed
* value toggled

---

### INV-A11Y-8 — Event Accuracy

Events MUST:

* correspond 1:1 with real state changes
* fire exactly once per transition
* never fire on no-op changes

Programmatic silent changes MAY suppress accessibility events.

---

## 10. Virtualization Interaction

### INV-A11Y-9 — Virtualization Transparency

Virtualization:

* MUST NOT hide logical nodes from accessibility
* MUST preserve ordering
* MUST preserve selection and focus projection

If platform APIs cannot represent offscreen items:

* a virtual proxy MUST exist

---

## 11. Responsibility Boundaries

### Accessibility Layer Owns:

* semantic projection
* role assignment
* state flag derivation
* accessibility event emission

### Accessibility Layer MUST NOT:

* own widget state
* own focus
* own input
* infer missing semantics
* reorder hierarchy

---

## 12. Forbidden Behavior

Accessibility layer MUST NOT:

* synthesize clicks
* auto-focus widgets
* change selection
* expand/collapse nodes
* infer intent
* “correct” widget behavior

Any such behavior is a **hard violation**.

---

## 13. Test Requirements

Implementations MUST test:

* stable A11yId under scrolling / virtualization
* focus mirroring accuracy
* selection projection correctness
* hidden item exclusion
* event emission accuracy
* no mutation via accessibility

---

## 14. Reference Implementation

Planned module:

* `accessibility::projection`

Consumes:

* Widget tree
* Focus manager
* Selection models

Produces:

* AccessibilityNode graph
* Event stream

---

## 15. Final Note

Accessibility is **truth telling**, not assistance.

If the UI state is wrong, accessibility must report it —
not hide it, smooth it, or compensate for it.

Any accessibility layer that “helps” by inventing semantics
is lying — and lying is worse than silence.
