# SPEC-HAL-113

**ToggleSwitch Behavior Contract**

**Status:** Design — Approved
**Scope:** Binary visual switch controls
**Applies to:** ToggleSwitch
**Non-Goals:** Checkbox semantics (SPEC-111), RadioButton semantics (SPEC-111), visual styling, animation, theming

---

## 1. Problem Statement

ToggleSwitch widgets are commonly confused with checkboxes but have distinct interaction semantics:

* visual metaphor of a physical switch
* potential for drag-based interaction (future)
* distinct accessibility semantics
* often used for immediate-effect settings

This SPEC defines a **strict interaction contract** for ToggleSwitch, building upon the activation semantics of SPEC-HAL-112 while clarifying ToggleSwitch-specific behavior.

---

## 2. Core Definitions

### 2.1 ToggleSwitch

A **ToggleSwitch** is a binary state control that:

* owns a persistent `on: bool` state
* visually represents a sliding switch metaphor
* changes state via explicit activation
* typically represents immediate-effect settings

---

### 2.2 State

Logical state values:

* `on = true`
* `on = false`

State transitions are **discrete**, not animated (animation is a visual concern, not a state concern).

---

## 3. Inheritance from SPEC-HAL-112

### INV-TS1 — Timing Semantics Inheritance

ToggleSwitch **inherits** all activation timing rules from SPEC-HAL-112 (Button Interaction Contract):

* activation on release, not press
* pointer capture during press
* exit cancels activation
* single activation per interaction
* disabled state handling

All invariants from SPEC-HAL-112 sections 4-9 apply unless explicitly overridden below.

---

## 4. Activation Key Override

### INV-TS2 — Space-Only Activation

ToggleSwitch **overrides** SPEC-HAL-112 INV-K2 (Activation Keys):

* `Space` activates ToggleSwitch
* `Enter` MUST NOT activate ToggleSwitch

Rationale: Consistency with SPEC-HAL-111 (Checkbox/RadioButton), which reserves Enter for default button activation.

---

## 5. Toggle Semantics

### INV-TS3 — Toggle on Activation

On activation:

```
on = !on
```

Exactly one state flip per activation.

---

### INV-TS4 — No Implicit Reversion

Once toggled, state persists until the next explicit activation.

No automatic rollback, timeout-based reversion, or hover-triggered changes.

---

## 6. Pointer Interaction

### INV-TS5 — Click-to-Toggle

ToggleSwitch activates via press-release cycle:

* press inside → release inside → toggle
* press inside → release outside → cancel
* press outside → release inside → no activation

Identical to SPEC-HAL-112 pointer semantics.

---

### INV-TS6 — No Drag Toggle (Current Scope)

Drag-based toggle (sliding the switch) is **out of scope** for this SPEC.

If implemented in the future, it must be defined in a separate SPEC or amendment.

---

## 7. Focus Semantics

### INV-TS7 — Focus Is Non-Destructive

Focus changes MUST NOT alter state.

* Tabbing to ToggleSwitch does nothing
* Focus ring is purely visual
* Focus is required for keyboard activation

---

## 8. Disabled State

### INV-TS8 — Disabled Is Absolute

When disabled:

* no activation
* no focus
* no hover effects
* no keyboard interaction

Inherits INV-D1 from SPEC-HAL-112.

---

## 9. Programmatic Control

### INV-TS9 — Explicit API

Programmatic state changes occur via:

* `set_on(bool)`

Programmatic changes:

* update logical state
* update visuals
* MUST NOT emit user-activation events

---

## 10. Responsibility Boundaries

**ToggleSwitch** owns:

* on/off state
* activation handling
* keyboard response (Space only)

**ToggleSwitch** MUST NOT own:

* visual rendering (delegated to painter)
* layout calculations (delegated to layout system)
* animation timing (delegated to animation system)

**Delegated responsibilities:**

* Layout system: determines widget rect
* Input system: delivers normalized input events
* SPEC-HAL-112: activation timing semantics (inherited)

---

## 11. Event Emission

### INV-TS10 — Event on User Activation Only

ToggleSwitch emits:

* `on_toggle(new_state: bool)`

Only on user-triggered activation, not programmatic changes.

---

## 12. Forbidden Behavior

ToggleSwitch MUST NOT:

* toggle on press
* toggle on hover
* toggle on focus
* toggle twice per interaction
* change state while disabled
* activate on Enter key

---

## 13. Test Requirements

Any ToggleSwitch implementation MUST include tests for:

* pointer toggle behavior
* keyboard Space activation
* Enter key rejection
* repeat suppression
* disabled state freezing
* focus non-effect
* programmatic state changes

---

## 14. Reference Implementation

Planned widget: `ToggleSwitch`

Related SPECs:

* SPEC-HAL-111 (Checkbox & RadioButton Contract)
* SPEC-HAL-112 (Button Interaction Contract)
* SPEC-HAL-086 (Input Fusion)

---

## 15. Final Note

ToggleSwitch is **not a fancy checkbox**.

It is a distinct control with specific semantics. Any implementation that conflates ToggleSwitch with Checkbox is **incorrect by definition**.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
