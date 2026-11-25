# SPEC-HAL-233: Step-Oriented Surface Progression Contract

**Status:** Draft (Binding upon adoption)
**Layer:** Navigation Authority / UI Orchestration
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-228, SPEC-HAL-229, SPEC-HAL-230, SPEC-HAL-231, SPEC-HAL-232, SPEC-HAL-234
**Applies To:** Wizard flows, multi-step surfaces, onboarding flows, guided progression systems

---

## 1. Purpose

This SPEC defines a step-oriented progression contract for navigation-driven surfaces.

It formalizes how multi-step UI flows (wizards, onboarding, guided processes) advance, regress, and terminate without embedding progression logic into widgets, compositor, or navigation resolution itself.

The goal is to:
- make step progression explicit
- keep navigation semantics authoritative
- avoid ad-hoc "current step" flags scattered across UI code

---

## 2. Scope

This SPEC governs:
- step sequencing
- forward / backward step transitions
- completion and cancellation semantics
- interaction between step progression and navigation paths

This SPEC does not define:
- visual layout
- animations
- validation logic
- data persistence
- domain-specific step meaning

---

## 3. Definitions

### Step

A discrete, ordered stage in a progression flow, represented by one complete semantic `NavigationEntry` plus its canonical root-to-step `Vec<NavigationEntry>` under SPEC-HAL-228.

### Step Flow

A navigation subtree representing an ordered progression of steps. Every declared step path shares one immutable flow prefix and contains the unique step entries from the first step through that step in declared order.

### Step Controller

An application-level component that requests step transitions.

### Completion

A terminal transition indicating successful end of the step flow.

### Cancellation

A terminal transition indicating aborted progression.

---

## 4. Authority Model

### 4.1 Navigation Authority

### INV-233-A1

NavigationHost remains the sole authority over navigation state.

Step progression does not bypass navigation resolution.

---

### 4.2 Step Control Authority

Step progression requests may originate from:
- application logic
- domain controllers
- orchestration code

Widgets may request progression but never execute it.

### INV-233-A2

Widgets must not mutate step state directly.

---

### 4.3 Execution Authority

Actual semantic navigation changes resulting from step progression are executed
only by the owning `NavigationHost` through its ordinary candidate, guard, and
commit pipeline. After a commit, `NavigationSurfaceOrchestrator` translates the
new committed path into surface lifecycle mutations; it does not execute or
reinterpret the step-navigation decision.

Step logic never calls compositor APIs.

---

## 5. Step Representation

Each step corresponds to one declarative descriptor:

```text
StepDescriptor {
    entry: NavigationEntry,
    path: Vec<NavigationEntry>,
    position: usize,
}
```

The descriptor's `entry` is unique and restorable. Its `path` is non-empty,
begins with the owning NavigationHost's immutable root, contains no duplicate
entry, and ends in `entry`. For one flow prefix `P` and ordered unique step
entries `S[0..N]`, every descriptor is canonical:

```text
step[N].path = P + S[0..=N]
```

`position` equals the descriptor's zero-based index in one dense ordered list.
The active step is the unique descriptor whose complete path equals the owning
NavigationHost's committed path. If no descriptor matches, that host is not
currently on a step in this flow; focus, surface visibility, or a retained
widget counter must not supply a different answer. The ordered list of steps is
explicit, not inferred.

### INV-233-S1

Step order and every canonical step path must be defined declaratively. The step
controller must reject a flow whose descriptor paths do not satisfy the shared
prefix and ordered-prefix rule before the flow can navigate.

---

## 6. Legal Step Transitions

### 6.1 Forward Progression

A forward step transition:
- moves from step N to step N+1
- emits `NavigationIntent::NavigatePath(step[N+1].path)` to the owning NavigationHost

### INV-233-T1

Skipping steps implicitly is forbidden unless explicitly declared.

---

### 6.2 Backward Progression

Backward transition:
- moves from step N to step N-1
- emits `NavigationIntent::NavigatePath(step[N-1].path)` to the owning NavigationHost

No special "step back" API exists. Step regression is progression within the
declared sequence; it is not `HistoryBack`, because chronological history may
contain entries outside the step flow, and it is not `NavigateUp` unless the
application separately emits that structural intent. The complete-path intent
prevents a non-ancestor step entry from being appended beneath the current step.

---

### 6.3 Jump Transitions

Explicit jumps:
- must be declared by the step controller
- must target a valid `StepDescriptor`
- emit exactly one `NavigationIntent::NavigatePath(target.path)`
- commit the descriptor's complete canonical path as one guard-gated history state; skipped steps are not committed as intermediate history entries

### INV-233-T2

Implicit jumps based on UI state are forbidden.

---

## 7. Completion Semantics

Completion is modeled as:
- navigation away from the step subtree
- termination of all step-associated surfaces (per SPEC-HAL-234)

Completion:
- must be explicit
- must not leave residual step surfaces

### INV-233-C1

A completed step flow must leave no active step surfaces.

---

## 8. Cancellation Semantics

Cancellation is modeled identically to completion but semantically distinct.

Cancellation:
- exits the step subtree
- may navigate to a caller-defined `NavigationEntry`

### INV-233-C2

Cancellation must not partially complete the flow.

---

## 9. Surface Implications

Step progression:
- creates surfaces for new steps
- deactivates or terminates surfaces for exited steps

All post-commit surface lifecycle effects are delegated to:
- NavigationSurfaceOrchestrator
- governed by SPEC-HAL-234

### INV-233-S2

Step logic must never manage surface visibility or destruction.

---

## 10. Failure Handling

If a step transition is denied (guarded):
- navigation state remains unchanged
- no surface mutation occurs

Step controllers must handle denial explicitly.

---

## 11. Forbidden Patterns

### INV-233-F1

No hidden step counters in widgets.

### INV-233-F2

No implicit "current step" derived from UI focus.

### INV-233-F3

No compositor-driven progression logic.

### INV-233-F4

No mutation of navigation path outside NavigationHost.

---

## 12. Non-Goals

This SPEC does not define:
- validation rules
- error messaging
- async step loading
- progress indicators
- UX affordances

---

## 13. Summary (Normative)

- Step progression is navigation-driven
- Order is explicit and declarative
- NavigationHost executes semantic progression; NavigationSurfaceOrchestrator applies only committed surface effects
- Widgets are request-only
- Surfaces follow navigation, never lead it
