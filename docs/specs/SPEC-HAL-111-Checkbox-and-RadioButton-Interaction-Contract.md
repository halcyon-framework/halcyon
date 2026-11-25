# SPEC-HAL-111

**Checkbox & RadioButton Interaction Contract**

**Status:** Design — Approved
**Scope:** Discrete boolean / single-choice input widgets
**Applies to:** Checkbox, RadioButton
**Non-Goals:** Styling, theming, animation, layout containers, accessibility APIs (handled separately)

---

## 1. Problem Statement

Checkboxes and radio buttons appear trivial but routinely break due to:

* ambiguous toggle semantics
* inconsistent keyboard behavior
* state desynchronization between visual, logical, and focus state
* incorrect grouping rules for radio buttons
* accidental drag / pointer jitter toggles

This SPEC defines a **strict, invariant-driven interaction contract** for Checkbox and RadioButton widgets, ensuring deterministic behavior across mouse, keyboard, and programmatic control.

---

## Stage 1: Checkbox State Definition

Stages 1–8 define the Checkbox contract. RadioButton selection is group-owned
and follows the radio-specific clauses below, SPEC-HAL-130, and SPEC-HAL-328.

### State Vector (Complete, Minimal)

| Field      | Type   | Semantics                                      |
|------------|--------|------------------------------------------------|
| `checked`  | `bool` | Persistent semantic value                      |
| `disabled` | `bool` | Hard override — blocks all interaction         |
| `focused`  | `bool` | Focus ownership                                |
| `active`   | `bool` | Transient pressed/armed state                  |

### State Invariants (Binding)

**INV-S1 — Checked is Sole Persistent Value**

`checked` is the only state field that represents persistent semantic meaning.
All other fields are transient or modifier states.

**INV-S2 — Active is Transient**

`active` is transient and MUST NOT survive cancellation.
When pointer exits bounds during press, or activation is otherwise cancelled, `active` resets to `false`.

**INV-S3 — Focused is Orthogonal to Checked**

Focus state has no causal relationship to checked state.
Gaining or losing focus MUST NOT alter `checked`.

**INV-S4 — Disabled Overrides All**

When `disabled = true`:
* `focused` MUST be `false`
* `active` MUST be `false`
* No state transitions permitted except `disabled` itself

### Forbidden (Explicitly Illegal)

* **Tri-state / Indeterminate** — Checkbox is strictly binary
* **Derived or Implicit State** — No state may be computed from other state or external sources
* **Visual-Driven State** — State MUST NOT be inferred from rendering output
* **External Mutation of Checked Without Semantic Event** — Direct assignment to `checked` without going through the activation path is illegal for user-initiated changes

### Scope Boundary

This stage defines **only** state existence and invariants.

**Explicitly not addressed:**
* Input handling
* Pointer or keyboard logic
* Focus acquisition rules
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 2: Semantic Event Contract

### Semantic Event Surface (Complete, Closed)

| Event       | Semantics                                      |
|-------------|------------------------------------------------|
| `Activate`  | Request to toggle `checked` state              |
| `Cancel`    | Abort transient `active` state                 |
| `FocusGain` | Widget receives focus ownership                |
| `FocusLoss` | Widget loses focus ownership                   |

### Binding Invariants

**INV-E1 — Semantic Events Only**

Widget processes **only** semantic events.
Raw input (pointer coordinates, key codes, timing) MUST NOT be visible to the widget.

**INV-E2 — Engine Maps Input to Semantics**

Pointer and keyboard inputs MUST be mapped to these four semantic events by the engine.
The widget never inspects input source.

**INV-E3 — Synchronous and Deterministic**

Semantic events are delivered synchronously.
Given identical state and event, output is identical.

**INV-E4 — No Direct Visual or Layout Mutation**

No semantic event may directly mutate layout or visuals.
Events affect state; state drives visuals through the paint cycle.

### Forbidden (Explicitly Illegal)

* **Inspecting Pointer Data** — Coordinates, button identity, pressure, device type
* **Inspecting Key Codes** — Scan codes, virtual keys, modifiers beyond what engine provides
* **Inspecting Gestures** — Drag distance, velocity, multi-touch
* **Inspecting Timing** — Duration, repeat rate, double-click intervals
* **Widget-Defined Semantic Events** — No events beyond the four listed
* **Implicit Semantic Inference** — No deriving events from state or context
* **Time-Based Semantics** — No hold-to-activate, long-press, or delay-triggered behavior
* **Gesture-Based Semantics** — No swipe, drag, or compound gesture recognition

### Scope Boundary

This stage defines **only** the semantic event surface.

**Explicitly not addressed:**
* State transitions triggered by events
* How pointer/keyboard maps to these events
* Focus acquisition rules
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 3: Pointer Interaction Semantics

### Pointer Lifecycle → Semantic Event Mapping

| Pointer Event                                      | Condition                | Effect                                      |
|----------------------------------------------------|--------------------------|---------------------------------------------|
| `PointerDown` inside hit target                    | `disabled = false`       | Set `active = true`                         |
| `PointerDown` inside hit target                    | `disabled = true`        | No effect                                   |
| `PointerUp` inside hit target                      | `active = true`          | Emit `Activate`, set `active = false`       |
| `PointerUp` inside hit target                      | `active = false`         | No effect                                   |
| `PointerUp` outside hit target                     | `active = true`          | Emit `Cancel`, set `active = false`         |
| `PointerCancel`                                    | `active = true`          | Emit `Cancel`, set `active = false`         |
| Pointer leaves hit target while pressed            | `active = true`          | Emit `Cancel`, set `active = false`         |

### Binding Invariants

**INV-P1 — No Toggle on PointerDown**

`PointerDown` MUST NOT toggle `checked`.
`PointerDown` only arms the widget by setting `active = true`.

**INV-P2 — Active Resets on All Cancellation Paths**

`active` MUST reset to `false` on:
* `PointerUp` outside hit target
* `PointerCancel`
* Pointer leaving hit target while pressed

No path may leave `active = true` after pointer release or cancellation.

**INV-P3 — Pointer Interaction Through Semantic Events**

Pointer interaction MUST NOT bypass semantic events.
The widget receives only `Activate` or `Cancel`, never raw pointer data.

**INV-P4 — Device-Agnostic Behavior**

Pointer behavior MUST be identical regardless of device type.
Mouse, touch, pen, and stylus follow the same rules.

### Forbidden (Explicitly Illegal)

* **Drag-Based Toggling** — Moving pointer while pressed MUST NOT toggle
* **Long-Press Activation** — Duration has no effect on activation
* **Delayed Activation** — No time-based triggering
* **Partial Activation** — PointerDown alone or hover alone MUST NOT activate
* **Pointer Identity Inspection** — Widget MUST NOT see pointer ID, button, or device
* **Pointer Position Tracking** — Widget MUST NOT see coordinates inside bounds

### Scope Boundary

This stage defines **only** pointer lifecycle to semantic event mapping.

**Explicitly not addressed:**
* Keyboard behavior
* Focus acquisition rules (focus granted by separate mechanism)
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 4: Keyboard Interaction Semantics

### Keyboard → Semantic Event Mapping

| Key       | Condition          | Effect           |
|-----------|--------------------|------------------|
| `Space`   | `disabled = false` | Emit `Activate`  |
| `Space`   | `disabled = true`  | No effect        |
| `Enter`   | `disabled = false` | Emit `Activate`  |
| `Enter`   | `disabled = true`  | No effect        |
| `Escape`  | `active = true`    | Emit `Cancel`    |
| `Escape`  | `active = false`   | No effect        |

### Binding Invariants

**INV-K1 — Single Activation Per Keypress**

Keyboard activation toggles state **exactly once** per keypress.
Key-down triggers activation; key-up has no effect on state.

**INV-K2 — Disabled Respected**

Keyboard activation MUST respect `disabled`.
When `disabled = true`, no keyboard input produces any effect.

**INV-K3 — Space and Enter Are Semantically Identical**

`Space` and `Enter` produce identical `Activate` events.
No behavioral difference exists between them.

**INV-K4 — Key Repeat Suppressed**

Key repeat MUST NOT cause repeated activation.
Held key produces exactly one activation, regardless of OS repeat settings.

**INV-K5 — Semantic Equivalence with Pointer**

Keyboard activation path MUST NOT differ from pointer path at the semantic level.
Both paths emit `Activate`; widget cannot distinguish source.

### Forbidden (Explicitly Illegal)

* **Key-Repeat Toggling** — Holding key MUST NOT toggle multiple times
* **Modifier-Dependent Behavior** — Shift, Ctrl, Alt MUST NOT alter activation semantics
* **Alternate Keyboard-Only Paths** — No keyboard shortcuts that bypass `Activate`
* **Time-Based Activation** — Press duration has no effect
* **Press-Duration Activation** — No hold-to-toggle or long-press semantics
* **Keyboard Inspection Inside Widget** — Widget MUST NOT see scan codes, virtual keys, or modifiers

### Scope Boundary

This stage defines **only** keyboard to semantic event mapping.

**Explicitly not addressed:**
* Pointer behavior
* Focus acquisition rules
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 5: Focus Rules

### Focus State Definition

Focus is a boolean state (`focused`) owned by the widget.
Focus represents keyboard routing ownership, not visual emphasis.

### Focus Acquisition Rules

| Trigger                          | Condition          | Effect                  |
|----------------------------------|--------------------|-------------------------|
| Pointer activation completes     | `disabled = false` | `focused = true`        |
| Pointer activation completes     | `disabled = true`  | No effect               |
| Keyboard navigation (Tab)        | `disabled = false` | `focused = true`        |
| Keyboard navigation (Tab)        | `disabled = true`  | Skip widget             |
| Programmatic focus request       | `disabled = false` | `focused = true`        |
| Programmatic focus request       | `disabled = true`  | Request rejected        |

### Focus Loss Rules

| Trigger                          | Effect                                    |
|----------------------------------|-------------------------------------------|
| Focus moves to another widget    | `focused = false`, `active = false`       |
| Widget becomes disabled          | `focused = false`, `active = false`       |
| Programmatic blur                | `focused = false`, `active = false`       |

### Binding Invariants

**INV-F1 — Focus Never Inferred from Hover**

Hover state has no causal relationship to focus.
Pointer entering bounds MUST NOT grant focus.

**INV-F2 — Focus Never Implicit on Repaint**

Focus is not a visual property.
Repaint cycles MUST NOT alter focus state.

**INV-F3 — Focus Cannot Exist When Disabled**

When `disabled = true`, `focused` MUST be `false`.
Disabling a focused widget MUST clear focus.

**INV-F4 — Focus Changes Do Not Toggle Checked**

Gaining or losing focus MUST NOT alter `checked`.
Focus and selection are orthogonal.

**INV-F5 — Losing Focus Clears Active**

When `focused` transitions to `false`, `active` MUST be set to `false`.
No armed state survives focus loss.

**INV-F6 — Focus Persists Across Checked Changes**

Toggling `checked` MUST NOT alter `focused`.
Activation does not blur.

### Forbidden (Explicitly Illegal)

* **Focus-on-Hover** — Pointer enter MUST NOT grant focus
* **Implicit Blur on Activation** — Activating MUST NOT lose focus
* **Focus Persistence While Disabled** — `disabled = true` AND `focused = true` is illegal state
* **Visual-Only Focus** — Focus ring without `focused = true` is forbidden
* **Focus Changes Driven by Layout** — Layout pass MUST NOT alter focus
* **Focus Changes Driven by Animation** — Animation MUST NOT alter focus

### Scope Boundary

This stage defines **only** focus acquisition, persistence, and loss rules.

**Explicitly not addressed:**
* Pointer semantics (how pointer grants focus is input layer concern)
* Keyboard semantics (how Tab grants focus is input layer concern)
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 6: Disabled Semantics

### Disabled Steady State

When `disabled = true`, the following MUST hold:

| Field     | Required Value | Reason                              |
|-----------|----------------|-------------------------------------|
| `focused` | `false`        | Disabled widgets cannot own focus   |
| `active`  | `false`        | Disabled widgets cannot be armed    |
| `checked` | unchanged      | Disabled does not alter value       |

### Disabled Transition Rules

**Transition: `disabled` = `false` → `true`**

| Effect                      | Timing      |
|-----------------------------|-------------|
| `focused` set to `false`    | Immediate   |
| `active` set to `false`     | Immediate   |
| `checked` unchanged         | —           |

**Transition: `disabled` = `true` → `false`**

| Effect                      | Timing      |
|-----------------------------|-------------|
| No automatic state changes  | —           |
| Focus not restored          | —           |
| Widget becomes receptive    | Immediate   |

### Semantic Event Behavior While Disabled

| Event       | Effect When `disabled = true` |
|-------------|-------------------------------|
| `Activate`  | No effect                     |
| `Cancel`    | No effect                     |
| `FocusGain` | No effect (request rejected)  |
| `FocusLoss` | No effect (already unfocused) |

### Binding Invariants

**INV-D1 — Disabled Is Hard Override**

`disabled = true` overrides all other state considerations.
No condition bypasses disabled.

**INV-D2 — No Partial Disable**

Disabled is binary and total.
There is no "partially disabled" or "disabled for pointer but not keyboard" state.

**INV-D3 — No Delayed Disable**

Disable effects are synchronous and immediate.
No deferred state clearing, no animation-gated disable.

**INV-D4 — Disabled Widgets Are Completely Inert**

While disabled, the widget produces no state changes and emits no events.
It is a black hole for input.

### Forbidden (Explicitly Illegal)

* **Visual-Only Disable** — Grayed appearance without `disabled = true` is forbidden
* **Disabled-But-Focusable** — `disabled = true` AND `focused = true` is illegal state
* **Disabled-But-Active** — `disabled = true` AND `active = true` is illegal state
* **Deferred Disable Effects** — Disable MUST NOT wait for animation or layout
* **Disabled Responding to Events** — Any semantic event producing effect while disabled is a bug

### Scope Boundary

This stage defines **only** disabled state behavior and transitions.

**Explicitly not addressed:**
* Pointer semantics
* Keyboard semantics
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 7: State Transition Rules

### State Transition Matrix

Given state vector `(checked, disabled, focused, active)` and semantic event, the following transitions are **exhaustive and deterministic**.

---

### Event: `Activate`

**Preconditions:**
* `disabled = false`

**Transition:**

| Field     | Before | After       |
|-----------|--------|-------------|
| `checked` | `C`    | `!C`        |
| `disabled`| `false`| `false`     |
| `focused` | `F`    | `F`         |
| `active`  | `A`    | `false`     |

**When `disabled = true`:** No effect. State unchanged.

---

### Event: `Cancel`

**Preconditions:**
* `disabled = false`
* `active = true`

**Transition:**

| Field     | Before | After       |
|-----------|--------|-------------|
| `checked` | `C`    | `C`         |
| `disabled`| `false`| `false`     |
| `focused` | `F`    | `F`         |
| `active`  | `true` | `false`     |

**When `disabled = true`:** No effect. State unchanged.
**When `active = false`:** No effect. State unchanged.

---

### Event: `FocusGain`

**Preconditions:**
* `disabled = false`
* `focused = false`

**Transition:**

| Field     | Before  | After       |
|-----------|---------|-------------|
| `checked` | `C`     | `C`         |
| `disabled`| `false` | `false`     |
| `focused` | `false` | `true`      |
| `active`  | `A`     | `A`         |

**When `disabled = true`:** No effect. Request rejected.
**When `focused = true`:** No effect. Already focused.

---

### Event: `FocusLoss`

**Preconditions:**
* `focused = true`

**Transition:**

| Field     | Before | After       |
|-----------|--------|-------------|
| `checked` | `C`    | `C`         |
| `disabled`| `D`    | `D`         |
| `focused` | `true` | `false`     |
| `active`  | `A`    | `false`     |

**When `focused = false`:** No effect. Already unfocused.

---

### Binding Invariants

**INV-T1 — Single Legal Outcome**

Each `(state_before, event)` pair produces **exactly one** `state_after`.
No ambiguity, no implementation choice.

**INV-T2 — Atomic Application**

Events apply atomically. No partial state updates visible.
Either all fields update or none do.

**INV-T3 — No Implicit Secondary Transitions**

An event MUST NOT trigger another event.
`Activate` does not implicitly emit `FocusGain`.
`FocusLoss` does not implicitly emit `Cancel` (it directly clears `active`).

**INV-T4 — Synchronous Transitions**

All transitions complete within the same synchronous call.
No deferred or async state changes.

### Forbidden (Explicitly Illegal)

* **Conditional Toggling Beyond Preconditions** — No hidden conditions affect transitions
* **Multi-Step Transitions** — No event may require multiple frames or passes
* **Hidden Side Effects** — No state change beyond what is specified in the table
* **Event-Order–Dependent Outcomes** — `Activate` after `FocusGain` produces same result as `FocusGain` after `Activate` (given same starting state)

### Scope Boundary

This stage defines **only** the state transition matrix.

**Explicitly not addressed:**
* Pointer mapping (how pointer produces events)
* Keyboard mapping (how keyboard produces events)
* Action emission
* Layout, styling, animation
* Grouping or radio semantics

---

## Stage 8: Action Emission Contract + Hard Ban Closure

### Emitted Signal

| Signal                         | Payload              |
|--------------------------------|----------------------|
| `ToggleChanged`                | `new_checked: bool`  |

This is the **sole external output** of the Checkbox widget.

### Emission Rules

| Condition                                      | Emission                        |
|------------------------------------------------|---------------------------------|
| `Activate` applied AND `checked` changed       | Emit `ToggleChanged(new_checked)` |
| `Activate` applied AND `disabled = true`       | No emission                     |
| `Cancel` applied                               | No emission                     |
| `FocusGain` applied                            | No emission                     |
| `FocusLoss` applied                            | No emission                     |

### Emission Timing

* **When:** Synchronous, immediately after applying the `Activate` transition (Stage 7)
* **Count:** Exactly once per successful `Activate`
* **Guard:** Emitted only if `checked` actually changed (always true for Checkbox since `Activate` toggles)

### Binding Invariants

**INV-A1 — Emission Is Pure Consequence**

Emission is a pure consequence of the `Activate` transition.
Emission does not depend on whether the trigger was pointer or keyboard.

**INV-A2 — Emission Does Not Trigger Events**

Emission MUST NOT trigger additional semantic events.
No cascading activations, no re-entrant event dispatch.

**INV-A3 — Emission Does Not Mutate State**

Emission MUST NOT mutate `focused`, `active`, or any other state.
State changes are fully determined by Stage 7; emission is output-only.

### Hard Ban Closure (Final)

The following are **permanently and explicitly illegal** for Checkbox. This list is closed.

| Banned Behavior                              | Reason                                           |
|----------------------------------------------|--------------------------------------------------|
| Tri-state / Indeterminate                    | Checkbox is strictly binary                      |
| Grouping Awareness                           | Checkbox is standalone; groups are RadioButton   |
| Long-Press / Press-Duration Behavior         | Time has no effect on activation                 |
| Drag-to-Toggle / Swipe / Gesture Semantics   | Pointer lifecycle is closed (Stage 3)            |
| Hover-Triggered Toggle                       | Hover MUST NOT alter `checked`                   |
| Key-Repeat Multi-Toggle                      | Single activation per keypress (Stage 4)         |
| Layout-Driven State                          | Layout MUST NOT alter state                      |
| Animation-Driven State                       | Animation MUST NOT alter state                   |
| Raw Input Inspection (pointer, keys, timing) | Widget sees only semantic events (Stage 2)       |
| Extra Semantic Events Beyond Stage 2         | Event surface is closed: Activate, Cancel, FocusGain, FocusLoss |
| Output Other Than `ToggleChanged(bool)`      | Emission surface is closed                       |

### Scope Boundary

This stage defines **only** the emission contract and finalizes the hard ban list.

**Explicitly not modified:**
* State vector (Stage 1)
* Semantic event set (Stage 2)
* Pointer mapping (Stage 3)
* Keyboard mapping (Stage 4)
* Focus rules (Stage 5)
* Disabled rules (Stage 6)
* State transition matrix (Stage 7)

**SPEC-HAL-111 Checkbox contract is now closed.**

---

## 2. Core Definitions

### 2.1 Logical State

* **Checkbox:** `checked: bool`
* **RadioButton:** `selected: bool` within a group

Logical state is authoritative and must never be inferred from visuals.

---

### 2.2 Visual State

Derived exclusively from logical + focus + hover state.

Visual state MUST NOT mutate logical state.

---

### 2.3 Activation

An **activation** is a user action that may change logical state.

Activation sources:
* primary mouse button click
* keyboard activation key
* programmatic trigger

---

## 3. Activation Semantics

### INV-A1 — Click-to-Toggle (Checkbox)

A checkbox **toggles state exactly once** on activation.

* `checked = !checked`
* No multi-toggle
* No press/release ambiguity

Dragging after press MUST NOT toggle again.

---

### INV-A2 — Select-Only (RadioButton)

A radio button **may only transition from unselected → selected**.

* Clicking an already-selected radio button does **nothing**
* Deselection occurs **only** via another radio button in the same group

---

### INV-A3 — Pointer Commit Timing

Checkbox activation MUST occur on **pointer release**, not press.

* Press inside → release inside → activate
* Press inside → release outside → cancel
* Press outside → release inside → no activation

This prevents accidental toggles during drag.

JC's owner policy change of 2026-10-06 supersedes release-to-select for
RadioButton mouse input only. A primary mouse `PointerDown` on an enabled
radio member MUST focus and select that member immediately through its
RadioGroup. The group may retain its pressed visual until release or
cancellation, but mouse `PointerUp` MUST NOT change selection or emit another
selection delta. Release or cancellation does not undo the Down selection.
Non-mouse pointer semantics remain unchanged.

---

## 4. Keyboard Semantics

### INV-K1 — Space Activates

When focused:

* Checkbox: `Space` activates; `Enter` MUST NOT activate (reserved for default buttons)
* RadioGroup: framework-mapped `i`, `Enter`, and `Space` activate the internally
  focused/highlighted radio through the group reducer (SPEC-HAL-328 Category S).

---

### INV-K2 — Arrow Navigation (Radio Groups Only)

Within a radio group:

* Arrow keys move internal focus/highlight only; selection MUST remain unchanged
* Boundary navigation follows SPEC-HAL-328 Category S
* Direction is layout-agnostic (no RTL assumptions)

Checkboxes do **not** respond to arrow navigation.

---

## 5. Focus & Hover Semantics

### INV-F1 — Focus Is Independent of State

Focus changes MUST NOT change logical state.

* Tabbing to a checkbox does nothing
* Hover does nothing
* Focus ring is purely visual

---

### INV-F2 — Hover Is Non-Authoritative

Hover state:
* may affect visuals
* MUST NOT affect logical state
* MUST NOT trigger activation

---

## 6. Radio Group Contract

### INV-R1 — Explicit Group Ownership

Radio buttons MUST belong to exactly one group.

* Group identity is explicit (ID or container-provided)
* No implicit grouping by proximity

---

### INV-R2 — Single Selection Guarantee

Within a group:

* At most **one** radio button may be selected
* Selecting one **must deselect all others atomically**

No intermediate states allowed.

---

### INV-R3 — Group Integrity

A radio button without a group is **invalid by definition**.

* Must assert in debug
* Must no-op in release

---

## 7. Disabled State

### INV-D1 — Disabled Is Absolute

When disabled:

* No activation
* No focus
* No hover effects
* No keyboard interaction

Visuals may reflect disabled state but MUST NOT imply interactivity.

---

## 8. Programmatic Control

### INV-P1 — State Change Is Silent

Programmatic state changes:

* MUST NOT emit activation events
* MUST update visuals immediately
* MUST respect group invariants (radio)

---

### INV-P2 — Event Emission Only on User Action

Only user-triggered activations emit:

* `on_toggle` (checkbox)
* `on_select` (radio)

---

## 9. Pointer Robustness

### INV-M1 — Jitter Immunity

Small pointer movement during press MUST NOT:

* cancel activation
* retrigger activation

Hit-testing is evaluated **only** at press and release.

---

## 10. Layout Independence

### INV-L1 — Geometry-Agnostic Interaction

Interaction semantics MUST NOT depend on:

* widget size
* shape
* visual style
* DPI scaling

Only hit-test rect matters.

---

## 11. Responsibility Boundaries

**Checkbox/RadioButton** owns:

* logical state (`checked` / `selected`)
* activation handling
* keyboard response
* focus acceptance

**Checkbox/RadioButton** MUST NOT own:

* visual rendering (delegated to painter)
* layout calculations (delegated to layout system)
* group membership logic (RadioGroup owns this)
* accessibility announcements (delegated to accessibility layer)

**Delegated responsibilities:**

* RadioGroup: manages group membership and exclusivity
* Layout system: determines widget rect
* Input system: delivers normalized input events

---

## 12. Forbidden Behavior

Checkbox/RadioButton MUST NOT:

* toggle state on hover
* toggle state on focus gain/loss
* allow multiple toggles per click
* Checkbox: activate on key press (only key release)
* RadioGroup: change selection through keyboard navigation instead of explicit
  `i` / `Enter` / `Space` activation
* ignore disabled state
* emit events on programmatic state change

---

## 13. Test Requirements

Any implementation MUST include tests for:

* single-click toggle
* press-drag-release cancellation (Checkbox); RadioButton mouse release/cancellation preserves the Down selection
* keyboard activation
* radio group exclusivity
* arrow navigation
* disabled behavior
* programmatic state changes

---

## 14. Reference Implementation Plan

* Implement Checkbox first
* Implement RadioButton reusing Checkbox activation core
* Group logic owned by RadioGroup helper (not widget tree)

---

## 15. Future Work (Out of Scope)

* Accessibility roles / ARIA
* Visual theming
* Indeterminate checkbox state
* Touch gesture extensions

---

## 16. Final Note

Checkboxes and radio buttons are **state machines**, not decorations.

Any deviation from this contract introduces ambiguity and user mistrust.
Violations are bugs, not stylistic choices.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
