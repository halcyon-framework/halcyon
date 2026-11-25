# SPEC-HAL-123

**Tooltip & Popover Interaction Contract**

**Status:** Design — Approved
**Scope:** Non-modal transient informational overlays
**Applies to:** Tooltip, Popover, InfoBubble, HoverCard
**Non-Goals:** Visual styling, animation curves, accessibility APIs, modal dialogs

---

## 1. Problem Statement

Tooltips and popovers routinely break UI integrity due to:

* implicit hover timing
* accidental focus capture
* blocking pointer interaction
* inconsistent dismissal rules
* misuse as lightweight dialogs

This SPEC defines **strict behavioral separation** between:

* **Tooltip** — passive, non-interactive, informational
* **Popover** — transient, optionally interactive, focus-aware

They are **overlays**, not widgets in the normal focus chain.

---

## 2. Core Definitions

### 2.1 Tooltip

A **purely informational**, non-interactive overlay bound to a target.

### 2.2 Popover

A transient overlay anchored to a target that **may contain interactive content**.

---

## 3. Tooltip Contract

### INV-TT1 — Passive Only

Tooltips:

* MUST NOT accept focus
* MUST NOT receive pointer events
* MUST NOT block interaction with underlying widgets
* MUST NOT contain interactive controls

---

### INV-TT2 — Hover Trigger Rules

Tooltip shows when:

* pointer hovers target for ≥ dwell delay (configurable, default ~500ms)

Tooltip hides when:

* pointer leaves target
* any pointer button is pressed
* keyboard focus changes
* scroll occurs

---

### INV-TT3 — Keyboard Suppression

Tooltips:

* MUST NOT appear due to keyboard navigation
* MUST NOT respond to key events

---

## 4. Popover Contract

### INV-PO1 — Explicit Invocation

Popovers open only via:

* explicit click / tap
* explicit keyboard activation
* explicit API call

Never on hover alone.

---

### INV-PO2 — Focus Behavior

Popovers operate in one of two modes (explicitly configured):

1. **Non-Focusable Popover**

    * does not take focus
    * clicks inside allowed
    * focus remains on invoking widget

2. **Focusable Popover**

    * captures focus on open
    * traps focus within popover
    * returns focus to invoker on close

---

### INV-PO3 — Dismissal Rules

Popovers close when:

* click outside (unless configured sticky)
* Esc key pressed
* invoking widget destroyed
* explicit programmatic close

---

## 5. Anchor & Geometry Rules

### INV-G1 — Anchor Ownership

Anchor system owns:

* positioning
* viewport clamping
* flipping (top/bottom/left/right)

Tooltip/Popover MUST NOT compute geometry independently.

---

### INV-G2 — Non-Layout Affecting

Overlays:

* MUST NOT affect layout
* MUST NOT resize parent widgets
* MUST NOT participate in Flex or Grid layout

---

## 6. Event Emission

### INV-E1 — Explicit Lifecycle Events

Allowed events:

* `on_open`
* `on_close`

No hover-change spam. No implicit activation events.

---

## 7. Responsibility Boundaries

### Tooltip/Popover owns:

* visibility state
* dismissal logic
* interaction mode (focusable vs non-focusable)

### MUST NOT own:

* anchor positioning math
* visual styling
* content semantics
* input normalization

---

## 8. Forbidden Behavior

Tooltips/Popovers MUST NOT:

* steal focus implicitly
* block pointer events unintentionally
* activate actions on hover
* behave like dialogs
* persist across navigation changes

---

## 9. Test Requirements

Any implementation MUST include tests for:

* hover dwell timing (Tooltip)
* non-interactivity enforcement (Tooltip)
* focus capture vs non-capture (Popover)
* outside-click dismissal
* Esc handling
* anchor clamping behavior

---

## 10. Reference Implementation

Planned widgets:

* `Tooltip`
* `Popover`
* `HoverCard`

Related SPECs:

* SPEC-HAL-115 (Dropdown & ComboBox — popup focus handling)
* SPEC-HAL-122 (Menu — transient overlay focus traps)
* SPEC-HAL-110 (Scroll Overlay & Viewport Contract)

---

## 11. Final Note

Tooltips inform.
Popovers assist.
Dialogs decide.

Any implementation that blurs these roles is **architecturally invalid**.
