# SPEC-HAL-020B: Contextual Action Resolution and Dispatch Contract

**Status:** Design / Binding
**Layer:** Engine coordination (input → intent → action resolution)
**Scope:** Action resolution order, priority, dispatch rules, rejection semantics
**Depends on:** SPEC-HAL-020A (Action System Core Model), SPEC-HAL-038 (focus & input routing), SPEC-HAL-127 / SPEC-HAL-137 (overlay boundaries)
**Constrains:** Event router, focus manager, overlay root, action resolver (NO bindings, NO execution logic)

---

## SECTION A — Problem Statement

Halcyon defines *what* an action is in SPEC-HAL-020A.
What is still missing is a **deterministic, context-aware mechanism** that answers:

* Which action should fire **right now**?
* Which scope wins when multiple scopes expose the same `ActionId`?
* How overlays, focus, modality, and capture affect action routing
* When an action is considered “handled” vs “unhandled”
* How ESC, Enter, Space, etc. are resolved *without fragile hard-coding*

This SPEC defines **resolution and dispatch**, not binding or execution.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Context beats global**
More specific scopes always override more general ones.

**PRINCIPLE-2: Deterministic order**
Resolution order is fixed, structural, and testable.

**PRINCIPLE-3: No hidden fallthrough**
If an action is rejected, the reason must be explicit.

**PRINCIPLE-4: Overlay boundaries are hard**
Actions do not “leak” across modal boundaries.

**PRINCIPLE-5: ESC is just an action**
No special keys in the engine. Only actions.

---

## SECTION C — Action Resolution Stack

Resolution proceeds through an **ordered scope stack**, evaluated top-down.

### C.1 Resolution Order (Highest → Lowest Priority)

1. **Active Overlay Scope** (topmost overlay only)
2. **Focused Widget Scope**
3. **Focused Widget Ancestors** (up the tree, until root or overlay boundary)
4. **Screen Scope**
5. **Application Scope**

**INV-ACT-RES-ORDER-1**
Resolution MUST stop at the first scope that produces a matching, enabled action.

**INV-ACT-RES-ORDER-2**
Lower-priority scopes MUST NOT see the action if a higher scope handled or rejected it.

---

## SECTION D — Overlay Boundary Rules

**INV-ACT-OVL-1**
When a modal overlay is present, resolution MUST NOT proceed to scopes below the overlay boundary.

**INV-ACT-OVL-2**
Non-modal overlays MAY allow fallthrough if explicitly marked as non-blocking.

**INV-ACT-OVL-3**
Overlay dismissal actions (e.g. Escape) MUST be resolved in overlay scope first.

---

## SECTION E — Focus and Capture Interaction

**INV-ACT-FOCUS-1**
Focused widget scope MUST be evaluated before ancestor widgets.

**INV-ACT-FOCUS-2**
If no widget is focused, widget scopes are skipped entirely.

**INV-ACT-CAPTURE-1**
Pointer capture affects pointer events only; it MUST NOT affect action resolution for keyboard-driven actions.

**INV-ACT-CAPTURE-2**
Action resolution MUST NOT consult pointer capture state.

---

## SECTION F — Matching and Rejection Semantics

Each scope can return one of:

* **Handled** — action accepted for execution
* **Rejected** — action explicitly refused
* **Not Present** — scope does not expose the action

**INV-ACT-REJ-1**
If a scope returns *Rejected*, resolution MUST stop and report rejection.

**INV-ACT-REJ-2**
Rejection MUST include a reason code (for debugging / tooling).

**INV-ACT-REJ-3**
Disabled actions result in rejection, not “not present”.

---

## SECTION G — Dispatch Contract

Dispatch means *choosing* the action target — not executing it.

**INV-ACT-DISP-1**
Dispatch MUST produce exactly one `ActionInstance` or a terminal failure.

**INV-ACT-DISP-2**
Dispatch MUST be side-effect free.

**INV-ACT-DISP-3**
Dispatch MUST NOT mutate widget state, focus, overlays, or selection.

Execution happens later (SPEC-HAL-020D).

---

## SECTION H — ESC, Enter, Space (Key-Agnostic Rule)

Keys are irrelevant at this layer.

**INV-ACT-KEYLESS-1**
The resolver operates on `ActionId`, not key codes.

Examples:

* `ActionId::escape`
* `ActionId::confirm`
* `ActionId::toggle_selection`
* `ActionId::clear_selection`

**INV-ACT-KEYLESS-2**
If no scope handles an action, it is considered **unhandled**, not fatal.

---

## SECTION I — Unhandled Actions

**INV-ACT-UNH-1**
Unhandled actions MUST bubble up to the application scope.

**INV-ACT-UNH-2**
If still unhandled at application scope, the engine MUST ignore the action silently.

**FORBID-ACT-UNH-EXIT**
The engine MUST NOT exit the application implicitly on unhandled actions (e.g. ESC).

(Application exit must be an explicit bound action.)

---

## SECTION J — Error and Debug Visibility

**INV-ACT-DBG-1**
Resolution MUST be observable in debug tooling (scope hit, rejection reason).

**INV-ACT-DBG-2**
Resolution order MUST be testable deterministically.

---

## SECTION K — Performance Invariants

**INV-ACT-PERF-RES-1**
Resolution MUST be O(d) where d is scope depth, not widget count.

**INV-ACT-PERF-RES-2**
No allocation per resolution step in steady state.

---

## SECTION L — Forbidden Behaviors

**FORBID-ACT-GLOBAL-FIRST**
Application scope MUST NOT be evaluated before widget/overlay scopes.

**FORBID-ACT-SPECIAL-KEYS**
No hard-coded key semantics in resolver.

**FORBID-ACT-CROSS-OVERLAY**
Actions MUST NOT cross modal overlay boundaries.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-020B is satisfied when:

* [ ] Action resolution order is fixed and deterministic
* [ ] Overlay boundaries are enforced
* [ ] ESC does not implicitly exit the app
* [ ] Rejection vs absence is explicit
* [ ] Dispatch is side-effect free
* [ ] Unhandled actions are safely ignored

---

## SECTION N — Relationship to Other SPECs

* 020A defines *what* an action is
* 020B defines *where* it resolves
* 020C defines *how* inputs bind to actions
* 020D defines *what happens* when actions execute
