# SPEC-HAL-130

**Radio Group State Model & Interaction Contract**

**Status:** Design — In Progress
**Scope:** Mutual-exclusion selection groups (1-of-N)
**Applies to:** RadioGroup, RadioButton members
**Non-Goals:** Styling, theming, animation, accessibility APIs, list/tree integration

---

## 1. Problem Statement

Radio mutual exclusion requires a group-owned selection state. Without a defined group state vector:

* selection devolves into per-widget hacks
* mutual exclusion cannot be enforced at the state level
* focus and active states leak across members
* disabled behavior is inconsistent

This SPEC defines a **strict, group-owned state model** for Radio selection (1-of-N), independent of visuals and per-button widget state.

---

## Stage 1: Radio Group State Model

### State Vector (Group-Owned, Complete)

| Field      | Type                | Semantics                                      |
|------------|---------------------|------------------------------------------------|
| `selected` | `Option<RadioId>`   | Sole persistent selection value                |
| `disabled` | `bool`              | Hard override for the entire group             |
| `focused`  | `Option<RadioId>`   | Which member owns focus within the group       |
| `active`   | `Option<RadioId>`   | Which member is armed/pressed                  |

### State Ownership

Selection is **group-owned**, not member-owned.

Individual RadioButton members are **stateless projections** of the group state:
* A member is "selected" iff `group.selected == Some(member.id)`
* A member is "focused" iff `group.focused == Some(member.id)`
* A member is "active" iff `group.active == Some(member.id)`

### Binding Invariants

**INV-G1 — Selected Is Sole Persistent Value**

`selected` is the only state field that represents persistent semantic meaning.
All other fields are transient or modifier states.

**INV-G2 — At Most One Selected**

At most one member may be `selected` at any time.
`selected = Some(id)` implies all other members are unselected.
`selected = None` is legal (no selection).

**INV-G3 — At Most One Focused**

At most one member may be `focused` at any time within the group.
Focus is mutually exclusive across members.

**INV-G4 — At Most One Active**

At most one member may be `active` at any time.
Only one member can be armed/pressed simultaneously.

**INV-G5 — Disabled Overrides All**

When `disabled = true`:
* `focused` MUST be `None`
* `active` MUST be `None`
* No member state transitions permitted except `disabled` itself

**INV-G6 — Selected Survives Disable**

`selected` is never auto-cleared by disable/enable transitions.
Disabling a group preserves the selection value.

### Forbidden (Explicitly Illegal)

* **Multiple Selected Members** — Violates mutual exclusion; illegal by state shape
* **Per-RadioButton Persistent `checked` State** — Selection is group-owned, not member-owned
* **Tri-state / Indeterminate** — RadioGroup is strictly 0-or-1-of-N
* **Selection Derived from Visuals/Layout** — State is authoritative; visuals are projections
* **Implicit "Must Always Have Selection"** — `selected = None` is legal unless explicitly constrained by application

### Scope Boundary

This stage defines **only** the group-owned state model.

**Explicitly not addressed:**
* Member pointer/keyboard mapping
* Focus traversal rules
* Action emission
* Layout, styling, animation
* List/tree integration

---

## Stage 2: Semantic Event Surface (Group-Level)

This stage governs the group state reducer, `reduce_radio_group`. The RadioGroup
input adapter/widget shell may inspect normalized raw input solely to map it into
the six semantic events below. Raw input, source, timing, and device data MUST NOT
enter the semantic reducer.

### Semantic Event Set (Closed)

| Event                       | Semantics                                      |
|-----------------------------|------------------------------------------------|
| `Select(member: RadioId)`   | Request to select a member                     |
| `Activate(member: RadioId)` | Request to arm + commit selection              |
| `Cancel(member: RadioId)`   | Abort transient active state for member        |
| `FocusGain(member: RadioId)`| Member gains focus within group                |
| `FocusLoss(member: RadioId)`| Member loses focus within group                |
| `Press(member: RadioId)`    | Arm a member (visual feedback on pointer down) |

### Event Semantics

**`Select(member)`**
Direct selection request. Sets `selected = Some(member)` if preconditions met.
Used for explicit selection requests, including immediate primary mouse-Down
selection. Keyboard navigation maps to `FocusGain`, never `Select`.

**`Activate(member)`**
Arms the member and commits selection on completion.
Maps from explicit keyboard `i` / `Enter` / `Space` activation of the focused
member. Existing non-mouse pointer-release mapping remains unchanged.

**`Cancel(member)`**
Aborts transient `active` state without changing `selected`.
Maps from pointer leaving bounds or escape key.

**`FocusGain(member)`**
Transfers focus ownership to the specified member.
Previous focus holder implicitly loses focus.

**`FocusLoss(member)`**
Removes focus from the specified member.
Clears `active` if the losing member was armed.

**`Press(member)`**
Arms the member for visual feedback on pointer down.
Sets `active = Some(member)` without committing selection.
Maps from pointer down events.

JC's owner policy change of 2026-10-06 replaces mouse release-to-select:
primary mouse Down on an enabled, valid member maps to `FocusGain`, `Select`,
then `Press`. `Select` commits immediately; `Press` only preserves pressed
feedback. Primary mouse Up terminates the current active member through
`Cancel`, even outside the group or over a different member. Primary mouse pointer-capture
cancellation also dispatches `Cancel` to the current active member. Both
terminal paths clear armed state without changing selection or emitting
another selection delta; they never undo the Down selection. Non-mouse
pointer mapping remains unchanged.

Keyboard navigation changes only `focused`; explicit framework-mapped
`i` / `Enter` / `Space` activation commits the focused member via `Activate`
(SPEC-HAL-328 Category S).

### Binding Invariants

**INV-E1 — Semantic Reducer Processes Only These Events**

`reduce_radio_group` processes **only** the six semantic events listed above.
No other event type is legal input to the semantic reducer.

**INV-E2 — Members Never Mutate Group State Directly**

Members dispatch events to the group.
Members MUST NOT write to `selected`, `focused`, or `active` directly.

**INV-E3 — Raw Input Invisible to Semantic Reducer**

Raw input (pointer coordinates, key codes, timing, device type) is invisible to `reduce_radio_group`.
The semantic reducer sees only semantic events.

**INV-E4 — Deterministic Outcomes**

Given identical `(state, event)` pair, outcome is identical.
No randomness, no timing dependence, no external state consulted.

### Forbidden (Explicitly Illegal)

* **Member-Defined Semantic Events** — Members MUST NOT introduce custom events
* **Per-Member State Mutation Outside Group Reducer** — All state changes flow through group
* **Inspecting Input Source** — The semantic reducer MUST NOT see pointer vs keyboard origin
* **Inspecting Timing** — The semantic reducer MUST NOT see duration, repeat rate, or intervals
* **Inspecting Device** — The semantic reducer MUST NOT see mouse vs touch vs pen
* **Group-Level Hover Semantics** — Hover is not a group event
* **Group-Level Gesture Semantics** — Drag, swipe, long-press are not group events

### Scope Boundary

This stage defines **only** the semantic event surface.

**Explicitly not addressed:**
* State transitions triggered by events (Stage 3)
* How pointer/keyboard maps to these events
* Action emission
* Layout, styling, animation

---

## Stage 3: Group State Transition Rules

### State Transition Matrix

Given state vector `(selected, disabled, focused, active)` and semantic event, the following transitions are **exhaustive and deterministic**.

---

### Event: `Select(member)`

**Preconditions:**
* `disabled = false`
* `member` is a valid group member

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `Some(member)`      |
| `disabled` | `false`             | `false`             |
| `focused`  | `F`                 | `F`                 |
| `active`   | `A`                 | `A`                 |

**When `disabled = true`:** No effect. State unchanged.
**When `member` invalid:** No effect. State unchanged.

---

### Event: `Activate(member)`

**Preconditions:**
* `disabled = false`
* `member` is a valid group member

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `Some(member)`      |
| `disabled` | `false`             | `false`             |
| `focused`  | `F`                 | `F`                 |
| `active`   | `A`                 | `None`              |

**When `disabled = true`:** No effect. State unchanged.
**When `member` invalid:** No effect. State unchanged.

Note: `Activate` commits selection and clears armed state atomically.

---

### Event: `Cancel(member)`

**Preconditions:**
* `disabled = false`
* `active = Some(member)`

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `S`                 |
| `disabled` | `false`             | `false`             |
| `focused`  | `F`                 | `F`                 |
| `active`   | `Some(member)`      | `None`              |

**When `disabled = true`:** No effect. State unchanged.
**When `active != Some(member)`:** No effect. State unchanged.

---

### Event: `FocusGain(member)`

**Preconditions:**
* `disabled = false`
* `member` is a valid group member

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `S`                 |
| `disabled` | `false`             | `false`             |
| `focused`  | `F`                 | `Some(member)`      |
| `active`   | `A`                 | `A`                 |

**When `disabled = true`:** No effect. Request rejected.
**When `member` invalid:** No effect. Request rejected.
**When `focused = Some(member)` already:** No effect. Already focused.

Note: Previous focus holder loses focus implicitly (mutual exclusion by assignment).

---

### Event: `FocusLoss(member)`

**Preconditions:**
* `focused = Some(member)`

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `S`                 |
| `disabled` | `D`                 | `D`                 |
| `focused`  | `Some(member)`      | `None`              |
| `active`   | `A`                 | if `A = Some(member)` then `None` else `A` |

**When `focused != Some(member)`:** No effect. Already unfocused.

Note: If the losing member was armed, `active` is cleared.

---

### Event: `Press(member)`

**Preconditions:**
* `disabled = false`
* `member` is a valid group member

**Transition:**

| Field      | Before              | After               |
|------------|---------------------|---------------------|
| `selected` | `S`                 | `S`                 |
| `disabled` | `false`             | `false`             |
| `focused`  | `F`                 | `F`                 |
| `active`   | `A`                 | `Some(member)`      |

**When `disabled = true`:** No effect. State unchanged.
**When `member` invalid:** No effect. State unchanged.

Note: `Press` arms the member for visual feedback without committing selection.
For mouse input, the separate `Select` event has already committed selection
on Down; mouse Up only clears armed state through `Cancel`.
`active` is transient state, cleared by `Cancel`, `Activate`, or `FocusLoss`.

---

### Disabled Transition Summary

| Event                 | Behavior When `disabled = true` |
|-----------------------|---------------------------------|
| `Select(member)`      | No effect                       |
| `Activate(member)`    | No effect                       |
| `Cancel(member)`      | No effect                       |
| `FocusGain(member)`   | No effect (request rejected)    |
| `FocusLoss(member)`   | No effect (already unfocused)   |
| `Press(member)`       | No effect                       |

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
`FocusLoss` does not implicitly emit `Cancel` (it directly clears `active` if needed).

**INV-T4 — Synchronous Transitions**

All transitions complete within the same synchronous call.
No deferred or async state changes.

**INV-T5 — Mutual Exclusion Preserved by Shape**

Transitions preserve mutual exclusion invariants (INV-G2, INV-G3, INV-G4) by construction.
`selected`, `focused`, `active` are `Option<RadioId>`, so at-most-one is structural.

### Forbidden (Explicitly Illegal)

* **Multi-Step Transitions** — No event may require multiple frames or passes
* **Deferred Transitions** — No event may schedule future state changes
* **Conditional Behavior Beyond Preconditions** — No hidden conditions affect transitions
* **Event-Order–Dependent Outcomes** — `Select` then `FocusGain` equals `FocusGain` then `Select` (given same starting state, modulo which runs first)
* **Implicit Deselection** — `selected` only changes via explicit `Select` or `Activate`
* **Implicit Reselection** — No event auto-restores previous selection

### Scope Boundary

This stage defines **only** the state transition matrix.

**Explicitly not addressed:**
* How pointer/keyboard maps to events
* Action emission
* Layout, styling, animation

---

## Stage 4: Action Emission Contract + Hard Ban Closure

### Emitted Signal

| Signal                                      | Payload                    |
|---------------------------------------------|----------------------------|
| `SelectionChanged`                          | `new_selected: Option<RadioId>` |

This is the **sole external output** of the RadioGroup.

### Emission Rules

| Condition                                           | Emission                              |
|-----------------------------------------------------|---------------------------------------|
| `Select(member)` applied AND `selected` changed     | Emit `SelectionChanged(new_selected)` |
| `Activate(member)` applied AND `selected` changed   | Emit `SelectionChanged(new_selected)` |
| `Select(member)` applied AND `selected` unchanged   | No emission                           |
| `Activate(member)` applied AND `selected` unchanged | No emission                           |
| `Cancel(member)` applied                            | No emission                           |
| `FocusGain(member)` applied                         | No emission                           |
| `FocusLoss(member)` applied                         | No emission                           |
| `Press(member)` applied                             | No emission                           |
| Any event when `disabled = true`                    | No emission                           |

### Emission Timing

* **When:** Synchronous, immediately after applying the transition (Stage 3)
* **Count:** Exactly once per transition that changes `selected`
* **Guard:** Emitted only if `selected` actually changed (`state_before.selected != state_after.selected`)

### Binding Invariants

**INV-A1 — Emission Depends Only on Selected Delta**

Emission depends only on `(state_before.selected, state_after.selected)`.
If they differ, emit. If they match, do not emit.

**INV-A2 — Emission Does Not Trigger Events**

Emission MUST NOT trigger additional semantic events.
No cascading selections, no re-entrant event dispatch.

**INV-A3 — Emission Does Not Mutate State**

Emission MUST NOT mutate `focused`, `active`, or any other state.
State changes are fully determined by Stage 3; emission is output-only.

### Hard Ban Closure (Final)

The following are **permanently and explicitly illegal** for RadioGroup. This list is closed.

| Banned Behavior                                    | Reason                                              |
|----------------------------------------------------|-----------------------------------------------------|
| Per-Member Persistent `checked` State              | Selection is group-owned (Stage 1)                  |
| Multiple Selection                                 | Violates 1-of-N mutual exclusion                    |
| Implicit "Must Always Have Selection"              | `selected = None` is legal unless app constrains    |
| Selection from Keyboard Navigation                | Navigation changes focus only; explicit activation commits |
| Hover-Based Selection                              | Hover is not a group event (Stage 2)                |
| Gesture-Based Selection                            | Drag, swipe, long-press are not group events        |
| Time-Based Selection                               | Duration has no effect on selection                 |
| Raw Input Inspection in Semantic Reducer (pointer, keys, timing) | Reducer sees only semantic events; input adapter maps normalized raw input (Stage 2) |
| Device Inspection in Semantic Reducer (mouse, touch, pen) | Reducer is device-agnostic; input adapter may distinguish devices solely for semantic mapping |
| Extra Semantic Events Beyond Stage 2               | Event surface is closed: Select, Activate, Cancel, FocusGain, FocusLoss, Press |
| Output Other Than `SelectionChanged(Option<RadioId>)` | Emission surface is closed                       |

### Scope Boundary

This stage defines **only** the emission contract and finalizes the hard ban list.

**Explicitly not modified:**
* State vector (Stage 1)
* Semantic event set (Stage 2)
* Transition matrix (Stage 3)
* Pointer/keyboard mapping (future stage or external)
* Layout, styling, animation

**SPEC-HAL-130 RadioGroup contract is now closed.**

---
