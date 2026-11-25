# SPEC-HAL-112

**Button Interaction & Activation Contract**

**Status:** Design — Approved
**Scope:** Discrete activation controls
**Applies to:** Button, IconButton, ToolbarButton (future)
**Non-Goals:** Visual styling, animation curves, theming, accessibility metadata

---

## 1. Problem Statement

Buttons are deceptively simple but frequently broken due to:

* ambiguous press vs activation timing
* duplicated mouse / keyboard logic
* double-activation bugs
* inconsistent disabled behavior
* focus and capture edge cases

This SPEC defines a **strict, invariant interaction contract** for all button-like widgets to ensure **single, deterministic activation** across input modalities.

---

## 2. Core Definitions

### 2.1 Button

A **discrete action control** that:

* represents a single atomic command
* does not maintain internal on/off state
* emits at most one activation per interaction cycle

### 2.2 Activation

A **logical action event** emitted by the button, e.g.:

* `on_click`
* `on_activate`

Activation is **not** equivalent to press, hover, or focus.

---

## 3. Input Modalities

Buttons MUST support:

* Pointer (mouse / pen)
* Keyboard
* Programmatic activation (future automation)

Touch pointer activation is admitted only by the scoped contract below.
Long-press, double-tap recognition, pan and pinch remain out of scope.

### Scoped contact activation — Reader iOS (2026-10-21)

JC's Reader continuation requests delivery of native contacts to the existing
Button press–release contract. A contact is an independent primary pointer:
inside press arms, first exit disarms, and a matching inside release activates
at most once. Native cancellation is terminal cancellation, never release.

Button owns the initiating `PointerId` until its matching release/cancellation,
including after exit has disarmed activation. Another pointer cannot re-arm,
release or cancel that interaction. Independent buttons may own different
contacts. Existing keyboard activation remains independent.

Transported touch does not implicitly transfer keyboard focus. This scoped
Button interaction uses INV-F2's optional-focus policy, preserving independent
Mouse/keyboard presses. A future touch-enabled editor needs its own admitted
explicit focus request; contact transport does not grant keyboard authority.

Touch delivery is a static widget opt-in through
`Widget::accepts_touch_pointer_input`, default false and true for Button.
The framework enforces this declaration before pointer dispatch and gesture
registration. This does not admit existing direct selection/resize handlers
merely because transport now carries touch. Widgets do not branch on device
type or implement a second router. Touch does not manufacture desktop hover.

HAL-128 owns contact identity, qualified press provenance and terminal teardown.
For the scoped contact-activation baseline above, its Touch+Select legality is
unchanged.

The accepted direct touch-content-scroll profile in [HAL-396](SPEC-HAL-396-Touch-Content-Scrolling.md) separately extends touch recognition and adds one engine-owned cancellation of an unfinished Button press, including its first-exit-disarmed state, for the first eligible ScrollArea winner of the same qualified press. It does not re-arm activation, change ordinary taps or keyboard input, or permit replacing an existing gesture winner.

---

## 4. Activation Invariant

### INV-A1 — Single Activation per Interaction

A button MUST emit **at most one activation** for a single user interaction, regardless of:

* pointer jitter
* key repeat
* focus changes mid-press

Double activation is **forbidden**.

---

## 5. Pointer Interaction Contract

### INV-P1 — Press–Release Activation

Activation via pointer occurs **only** when:

1. Pointer press occurs **inside** button hit rect
2. Pointer release occurs **inside** button hit rect
3. Button was not disabled at either moment

Press alone MUST NOT activate.

For the ordinary rectangular Button, layout computes its natural content-plus-padding box, constrains that box to the admitted parent constraints, and uses the same final size for LayoutResult and the layout-owned activation bounds. Tight constraints define the final control box; loose constraints preserve natural size when it fits. Position remains WidgetTree/layout-owned. Paint and input consume that completed geometry; neither recomputes or repairs it. This does not enlarge intentionally smaller indicator/label hit regions and changes no activation, capture or cancellation rule.

Delegated R5 clarification accepted by the delegated technical reviewer on 2026-10-08 under JC's standing development delegation.

---

### INV-P2 — Capture Stability

Once pointer press begins inside the button:

* the button **captures** the pointer
* hover state MAY update
* activation decision is deferred until release

Pointer capture is released on pointer-up or cancel.

---

### INV-P3 — Exit Cancels Activation

If the pointer leaves the hit rect **before release**:

* activation MUST be canceled
* no action is emitted on release

Re-entering does not re-arm activation.

---

## 6. Keyboard Interaction Contract

### INV-K1 — Focus Required

Keyboard activation requires:

* button has keyboard focus
* button is enabled

No focus → no activation.

---

### INV-K2 — Activation Keys

Activation MUST occur on **key release** of:

* `Enter`
* `Space`

Key press alone MUST NOT activate.

---

### INV-K3 — Repeat Suppression

Key repeat MUST NOT cause repeated activations.

* Holding Space or Enter → at most one activation
* Activation occurs on first key release only

---

## 7. Disabled State Invariants

### INV-D1 — Disabled Means Inert

When disabled, a button:

* MUST NOT activate
* MUST NOT capture pointer
* MUST NOT respond to keyboard activation
* MAY still receive layout and paint

Disabled state overrides all other interaction rules.

**Scoped adopted execution — 2026-10-26:** the JC-delegated decision in
SPEC-HAL-108 §13.5 governs ordinary Button's private enabled policy and deferred
runtime requests. Disable cancels its pending activation/captures without click;
re-enable requires fresh activation. No writable effective bypass, skin-only
disable or changed UiInteractionPolicy is admitted. Existing status is retained.

---

## 8. Hover and Press States

### INV-H1 — Hover Is Informational Only

Hover state:

* affects visuals only
* MUST NOT influence activation logic

---

### INV-H2 — Press State Is Transient

Press state:

* exists only between press and release
* MUST be cleared on:
    * pointer cancel
    * focus loss
    * widget disable
    * window focus loss

---

## 9. Focus Interaction

### INV-F1 — Focus Does Not Activate

Gaining or losing focus MUST NOT trigger activation.

---

### INV-F2 — Pointer Press Does Not Imply Focus (Optional)

Buttons MAY request focus on pointer press, but:

* focus acquisition MUST NOT activate
* activation still requires valid release

---

## 10. Programmatic Activation

### INV-PROG1 — Logical Equivalence

Programmatic activation (e.g. `button.activate()`) MUST:

* emit exactly one activation
* bypass hover / press state
* respect disabled state

---

## 11. Responsibility Boundaries

**Button** owns:

* activation logic
* press/release state tracking
* keyboard activation handling
* disabled state enforcement

**Button** MUST NOT own:

* visual rendering (delegated to painter)
* layout calculations (delegated to layout system)
* action execution (delegated to event handlers)
* focus ring rendering (delegated to focus system)

**Delegated responsibilities:**

* Layout system: determines widget rect
* Input system: delivers normalized input events
* Event system: routes activation events to handlers

---

## 12. Event Ordering Invariant

### INV-E1 — Activation After Release

For both pointer and keyboard:

press → (optional hover changes) → release → activate


Activation MUST be the **final** event in the sequence.

---

## 13. Forbidden Behavior

Buttons MUST NOT:

* activate on press
* activate on hover
* activate twice per interaction
* activate when disabled
* activate due to focus change
* activate due to key repeat

Any of the above is a correctness bug.

---

## 14. Test Requirements

Any Button implementation MUST have tests covering:

* pointer press + release inside → activation
* press inside, release outside → no activation
* press outside → no activation
* keyboard Enter / Space activation
* key repeat suppression
* disabled state behavior
* focus loss mid-press
* window focus loss mid-press

---

## 15. Reference Implementation

Reference widget: `Button` (Tier 1)
Related SPECs:

* SPEC-HAL-010 (Widget System)
* SPEC-HAL-011 (Tier 1 Widgets)
* SPEC-HAL-086 (Input Fusion)

---

## 16. Final Note

Buttons are **not** simple.
They are **state machines under input stress**.

Any deviation from this contract results in:
* accidental double actions
* broken keyboard UX
* nondeterministic behavior

From this point forward, any button-like widget violating this SPEC is **incorrect by definition**.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
