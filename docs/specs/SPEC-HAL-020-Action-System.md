# SPEC-HAL-020: Action System and Contextual Command Routing

**Status:** Design / Binding
**Layer:** Engine input & command routing
**Scope:** All widgets, screens, overlays, and applications
**Depends on:** SPEC-HAL-018, SPEC-HAL-127, SPEC-HAL-128, SPEC-HAL-135, SPEC-HAL-136
**Constrains:** No rendering, no layout, no widget mutation outside action handlers

---

## SECTION A — Problem Statement

Halcyon currently suffers from **implicit, fragile input behavior**:

* `ESC` exits demos/apps unexpectedly
* widgets hardcode key handling
* selection logic conflicts with navigation
* transactional workflows lack cancel/commit authority
* no unified notion of “who handles this input first”

These failures all stem from the same root cause:

> **Input is handled locally, but intent is global.**

We need a **first-class Action system** that:

* decouples *input* from *intent*
* supports **contextual routing**
* supports **priority and ownership**
* allows **explicit cancel / commit semantics**
* is zero-cost when idle

---

## SECTION B — Core Principle

**PRINCIPLE-1: Input produces Actions, not behavior**
Keys, buttons, gestures do not act directly — they *request* actions.

**PRINCIPLE-2: Actions are routed, not broadcast**
Exactly one handler executes, chosen by context + priority.

**PRINCIPLE-3: Context is explicit**
Widgets, screens, overlays, and apps declare what they can handle.

**PRINCIPLE-4: ESC is an Action, not a key**
Its meaning depends on context, never global hardcoding.

---

## SECTION C — Action Identity

Actions have **stable, non-string identity**.

```rust
struct ActionId {
    namespace: ActionNamespace,
    local_id: u32,
}
```

**INV-ACT-ID-1**
Action identity MUST be stable across builds.

**INV-ACT-ID-2**
Action identity MUST NOT depend on strings, hashes, or user input.

**INV-ACT-ID-3**
Two distinct actions MUST NEVER share an `ActionId`.

---

## SECTION D — Action Ownership

Each action is owned by **exactly one**:

* Widget
* Screen
* Overlay
* Application

**INV-ACT-OWN-1**
An action MUST have a single authoritative owner.

**INV-ACT-OWN-2**
Ownership defines default routing priority, not exclusivity.

---

## SECTION E — Action Context Stack

At runtime, the engine maintains a **Context Stack** (top = most specific):

1. Top Overlay (modal, popover, menu)
2. Focused Widget
3. Parent Widgets
4. Screen
5. Application

**INV-ACT-CTX-1**
Actions are resolved top-down.

**INV-ACT-CTX-2**
The first handler that accepts the action terminates routing.

---

## SECTION F — Action Routing

```rust
enum ActionResult {
    Handled,
    Ignored,
    Disabled,
}
```

**INV-ACT-ROUTE-1**
Exactly one handler may return `Handled`.

**INV-ACT-ROUTE-2**
`Ignored` MUST continue routing.

**INV-ACT-ROUTE-3**
`Disabled` MUST block lower-priority handlers.

---

## SECTION G — ESC Semantics (Authoritative)

ESC is defined as **Action::Cancel**.

**INV-ACT-ESC-1**
ESC MUST NOT exit the app by default.

**INV-ACT-ESC-2**
ESC resolves in this order:

1. Transactional selection → Cancel
2. Active overlay → Dismiss
3. Focused widget → Context cancel
4. Screen → Back / Close
5. Application → Exit (optional)

**INV-ACT-ESC-3**
If unhandled, ESC does nothing.

---

## SECTION H — Relationship to Transactional Selection (SPEC-136)

Transactional selection **registers actions**:

* `Selection.Toggle`
* `Selection.Commit`
* `Selection.Cancel`

**INV-ACT-TXN-1**
Transactional selection MUST rely on actions, not raw key handling.

**INV-ACT-TXN-2**
Commit and Cancel MUST be routable and overrideable.

---

## SECTION I — Action Binding

Bindings map inputs to actions:

```rust
struct ActionBinding {
    input: InputChord,
    action: ActionId,
}
```

**INV-ACT-BIND-1**
Bindings MUST be declarative.

**INV-ACT-BIND-2**
Bindings MUST NOT embed behavior.

---

## SECTION J — Performance Guarantees

**INV-ACT-PERF-1**
If no actions are registered, routing cost MUST be zero.

**INV-ACT-PERF-2**
Action routing MUST NOT trigger layout, paint, or tree rebuild.

---

## SECTION K — Forbidden Patterns

**FORBID-ACT-DIRECT-KEY-HANDLING**
Widgets MUST NOT hardcode key behavior.

**FORBID-ACT-GLOBAL-ESC**
Applications MUST NOT globally bind ESC without context.

**FORBID-ACT-IMPLICIT-COMMIT**
Actions MUST NOT infer commit or cancel implicitly.

---

## SECTION L — Acceptance Criteria

SPEC-HAL-020 is complete when:

* [ ] ESC no longer exits demos by default
* [ ] Lists use actions for commit/cancel
* [ ] Overlays consume cancel correctly
* [ ] Widgets declare actions instead of key logic
* [ ] No behavior depends on raw key codes

---

## AUTHORITATIVE STATEMENTS

**AUTH-020-1:**
“Input requests intent. Context decides behavior.”

**AUTH-020-2:**
“ESC is cancel, never exit.”

**AUTH-020-3:**
“Actions are the contract between user intent and system authority.”
