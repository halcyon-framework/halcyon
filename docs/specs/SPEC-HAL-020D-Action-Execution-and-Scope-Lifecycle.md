# SPEC-HAL-020D: Action Execution and Scope Lifecycle

**Status:** Design / Binding
**Layer:** Action execution (post-resolution)
**Scope:** Action invocation, scope ownership, lifecycle, cancellation, propagation
**Depends on:**

* SPEC-HAL-020A (Action System Core Model and Invariants)
* SPEC-HAL-020B (Contextual Action Resolution and Dispatch Contract)
* SPEC-HAL-020C (Action Binding and Input Mapping)
  **Constrains:** Event router, widget tree, overlay stack
  **Explicitly Excludes:** Input binding, action resolution logic

---

## SECTION A — Purpose

This SPEC defines **how an ActionId is executed** once it has been resolved.

It answers:

* Who executes an action
* Where execution happens
* How execution is scoped
* How cancellation, bubbling, and capture behave
* How lifecycle boundaries are enforced

This layer **assumes exactly one ActionId** has already been chosen.

---

## SECTION B — Execution Boundary

**INV-ACT-EXEC-BOUND-1**
Action execution MUST occur only after resolution is complete.

**INV-ACT-EXEC-BOUND-2**
Execution MUST NOT re-enter binding or resolution layers.

**INV-ACT-EXEC-BOUND-3**
Execution MUST be synchronous with respect to the event loop step that triggered it.

---

## SECTION C — Execution Target

An action is executed by a **Scope Owner**.

Scope Owners MAY be:

* Widget instance
* Overlay
* Application root
* Engine subsystem

**INV-ACT-EXEC-TGT-1**
Exactly one Scope Owner executes an action.

**INV-ACT-EXEC-TGT-2**
Execution ownership MUST be explicit and deterministic.

---

## SECTION D — Scope Model

A **Scope** is a bounded execution domain.

Examples:

* Focused widget scope
* Overlay scope
* Modal scope
* Application scope

**INV-ACT-EXEC-SCOPE-1**
Scopes form a strict hierarchy.

**INV-ACT-EXEC-SCOPE-2**
An action executes in exactly one scope.

**INV-ACT-EXEC-SCOPE-3**
Scopes MUST NOT overlap ambiguously.

---

## SECTION E — Scope Lifecycle

Each scope has a lifecycle:

1. Created
2. Active
3. Suspended (optional)
4. Destroyed

**INV-ACT-EXEC-LIFE-1**
Actions MUST NOT execute in destroyed scopes.

**INV-ACT-EXEC-LIFE-2**
Suspended scopes MAY intercept or reject execution.

---

## SECTION F — Execution Semantics

An executed action produces one of:

* **Handled**
* **Rejected**
* **Deferred**

**INV-ACT-EXEC-SEM-1**
Exactly one outcome MUST be produced.

**INV-ACT-EXEC-SEM-2**
Deferred actions MUST specify a resumption trigger or be dropped.

---

## SECTION G — Cancellation

Actions MAY be cancellable.

**INV-ACT-EXEC-CANCEL-1**
Cancellation MUST be explicit.

**INV-ACT-EXEC-CANCEL-2**
Cancelled actions MUST NOT produce side effects.

**FORBID-ACT-EXEC-IMPLICIT-CANCEL**
Actions MUST NOT be implicitly cancelled by focus loss or redraw.

---

## SECTION H — Propagation Rules

Execution MAY propagate **upward** if rejected.

**INV-ACT-EXEC-PROP-1**
Propagation is linear and ordered.

**INV-ACT-EXEC-PROP-2**
Propagation stops on first handler that returns Handled.

**FORBID-ACT-EXEC-MULTI-HANDLED**
An action MUST NOT be handled more than once.

---

## SECTION I — Capture and Modal Enforcement

Modal scopes enforce capture.

**INV-ACT-EXEC-MODAL-1**
When a modal scope is active, execution MUST NOT escape it.

**INV-ACT-EXEC-MODAL-2**
Actions targeting outer scopes MUST be rejected, not deferred.

---

## SECTION J — Action Reentrancy

**INV-ACT-EXEC-REENT-1**
Actions MUST NOT re-enter the same scope concurrently.

**INV-ACT-EXEC-REENT-2**
Nested execution MUST occur only in child scopes.

---

## SECTION K — Error Handling

**INV-ACT-EXEC-ERR-1**
Execution errors MUST be contained within the executing scope.

**INV-ACT-EXEC-ERR-2**
Errors MUST NOT crash the event loop.

---

## SECTION L — Observability

**INV-ACT-EXEC-OBS-1**
Execution MUST emit diagnostics hooks (action id, scope, outcome).

**INV-ACT-EXEC-OBS-2**
Observability MUST NOT affect execution order or timing.

---

## SECTION M — Performance Invariants

**INV-ACT-EXEC-PERF-1**
Execution MUST be O(1) per action in steady state.

**INV-ACT-EXEC-PERF-2**
No allocation is allowed during execution except explicitly deferred actions.

---

## SECTION N — Forbidden Behaviors

**FORBID-ACT-EXEC-BIND**
Execution MUST NOT inspect input bindings.

**FORBID-ACT-EXEC-RESOLVE**
Execution MUST NOT perform resolution.

**FORBID-ACT-EXEC-GLOBAL-LEAK**
Actions MUST NOT bypass scope hierarchy.

---

## SECTION O — Acceptance Criteria

SPEC-HAL-020D is satisfied when:

* [ ] Each action executes in exactly one scope
* [ ] Modal capture is enforced
* [ ] Cancellation is explicit and safe
* [ ] No action executes twice
* [ ] Execution is deterministic and bounded

---

## SECTION P — Relationship to Other SPECs

* 020A defines what actions are
* 020B decides which action fires
* 020C maps input to action intent
* 020D executes the action and enforces scope lifecycle
