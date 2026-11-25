# SPEC-HAL-151: Multi-Window Focus and Interaction Domains

**Status:** IMPLEMENTED (engine-level)
**Implementation date:** 2026-01
**Layer:** Windowing & Interaction Model
**Scope:** Focus, capture, and interaction isolation across multiple windows
**Depends on:** SPEC-HAL-038, SPEC-HAL-140, SPEC-HAL-142, SPEC-HAL-150
**Constrains:** Engine-level semantics only (no OS or platform policy invention)

---

## Implementation Summary

The engine-level implementation is complete:

* `InteractionDomainState` is per-window — each window has isolated focus, capture, gesture, hover, and scroll-capture state
* `EventRouter` is stateless with respect to interaction — all state is passed via `&mut InteractionDomainState`
* Active-window gating is enforced at intake — non-active windows cannot deliver input events (except `FocusChanged`, `Resize`, `Tick`)
* Focus loss triggers full domain teardown — capture, gestures, hover, scroll-capture, and focus are cleared atomically

Cross-window input leakage is architecturally impossible.

---

## SECTION A — Problem Statement

Multi-window systems frequently collapse into ambiguity:

* multiple windows appear “focused”
* input leaks across windows
* modal behavior breaks isolation
* background windows react to input
* focus becomes a global, fragile singleton

Most frameworks treat windows as **visual containers**, not **interaction domains**.

Halcyon treats each window as a **hard interaction boundary**.

This SPEC defines **how focus, capture, and intent are partitioned across windows**, and makes cross-window leakage architecturally illegal.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Windows are interaction domains**
A window is not just a surface; it is a boundary of authority.

**PRINCIPLE-2: Focus is scoped**
Focus always belongs to exactly one window.

**PRINCIPLE-3: Capture is never global**
Input capture cannot cross window boundaries.

**PRINCIPLE-4: Visibility ≠ activity**
A visible window is not necessarily interactive.

**PRINCIPLE-5: No ambient cross-window behavior**
Nothing reacts “just because another window did”.

---

## SECTION C — Window Interaction Domains

Each window defines an **Interaction Domain** consisting of:

* Focus tree
* Capture state
* Navigation cursor
* Selection state
* Intent routing context

Domains are **mutually isolated**.

---

## SECTION D — Focus Semantics Across Windows

**INV-WIN-FOCUS-SINGLE**
At most one window may be focused at any time.

**INV-WIN-FOCUS-OWNERSHIP**
Focus always belongs to a specific window.

**INV-WIN-FOCUS-NO-SHARE**
Focus MUST NOT be shared across windows.

### Focus Change Behavior (Implemented)

**INV-151-ACTIVE-WINDOW-GATE:** Non-active windows do not receive input events, except the following exempt event types:
* `FocusChanged` — required for focus switching
* `Resize` — window geometry is focus-independent
* `Tick` — timer events are global

All other events from non-active windows are dropped at intake.

**INV-151-FOCUS-TEARDOWN:** When the active window loses focus, full domain teardown occurs. The following state is cleared atomically:
* `pointer_capture_manager` — cancel all captures
* `pointer_capture` (legacy) — release
* `gesture_registry` — clear all sessions
* `hovered_widget` — cleared
* `scroll_capture_owner` — cleared
* `focus_manager` — cleared

After teardown, `active_window` is set to `None`. On focus gain, `active_window` is set to the newly focused window.

**Enforcement:**
* `InteractionDomains::should_route_event()` — enforces INV-151-ACTIVE-WINDOW-GATE
* `InteractionDomains::handle_focus_changed()` — enforces INV-151-FOCUS-TEARDOWN

---

## SECTION E — Capture Semantics Across Windows

**INV-WIN-CAPTURE-LOCAL**
Capture is scoped to the focused window only.

**INV-WIN-CAPTURE-NO-TRANSFER**
Capture MUST NOT transfer across windows. Focus loss triggers capture cancellation as part of INV-151-FOCUS-TEARDOWN (see Section D).

**INV-WIN-CAPTURE-NO-STEAL**
A background window MUST NOT acquire capture. INV-151-ACTIVE-WINDOW-GATE prevents input events from reaching non-active windows, making capture acquisition architecturally impossible.

---

## SECTION F — Input Routing Rules

All input events are routed as follows:

1. Determine active window
2. Resolve capture within that window
3. Dispatch intent inside that window only

**INV-WIN-INPUT-ISOLATION**
Input MUST NOT cross window boundaries.

HAL-101 section 2.5 executes only within the current actual interaction domain/window and its qualified tree. Every stage revalidates that scope. No default-domain fallback, foreign child/session or cross-window capture transfer is admitted; this clause does not activate deferred multi-window tree support.

---

## SECTION G — Navigation Cursor Semantics

Each window owns its own navigation cursor.

**INV-WIN-CURSOR-LOCAL**
Navigation cursor state is window-local.

Switching windows:

* suspends cursor state in old window
* resumes cursor state in new window

---

## SECTION H — Selection and State Persistence

**INV-WIN-STATE-PERSISTENCE**
Selection, focus, and cursor state persist per window.

Closing a window:

* destroys its interaction domain
* commits or discards transactional state per policy (outside scope)

---

## SECTION I — Modal and Transient Windows

Modal windows form **nested interaction domains**.

**INV-WIN-MODAL-EXCLUSIVE**
While a modal window is active:

* parent window is non-interactive
* focus and input are blocked at the domain boundary

---

## SECTION J — Forbidden Behaviors

The following are forbidden:

* background windows reacting to input
* shared focus across windows
* global capture
* implicit window activation
* input leakage between windows

---

## SECTION K — Performance and Determinism

**INV-WIN-O1**
Window routing decisions MUST be constant-time.

**INV-WIN-DETERMINISTIC**
Given the same window state and input, routing MUST be identical.

---

## SECTION L — Interaction with Existing SPECs

* Focus & capture: SPEC-HAL-140
* Interaction authority: SPEC-HAL-142
* Compositor limits: SPEC-HAL-150
* Focus routing substrate: SPEC-HAL-038

SPEC-HAL-151 introduces **no new windowing features**, only isolation rules.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-151 is satisfied when the following mechanically checkable criteria are met:

### M.1 — Invariant Enforcement Functions

| Invariant | Enforcement Function | Location |
|-----------|---------------------|----------|
| INV-151-ACTIVE-WINDOW-GATE | `InteractionDomains::should_route_event()` | `src/core/event/mod.rs` |
| INV-151-FOCUS-TEARDOWN | `InteractionDomains::handle_focus_changed()` | `src/core/event/mod.rs` |
| INV-151-PRIMARY-ONLY | `debug_assert!` guards at call sites | `src/core/app.rs` |

### M.2 — Event Loop Gating

Both event loops must gate input using `should_route_event()` and handle focus using `handle_focus_changed()`:
* `App::run()` polling event loop — `src/core/app.rs:2358`, `src/core/app.rs:2371`
* `App::run()` winit event loop — `src/core/app.rs:4481`, `src/core/app.rs:4494`

### M.3 — Engine-Internal Test Coverage

| Test | Invariant | Location |
|------|-----------|----------|
| `test_focus_switch_cancels_drag_in_losing_window` | INV-151-FOCUS-TEARDOWN | `src/tests/interaction_domain_isolation.rs` |
| `test_non_active_window_events_ignored` | INV-151-ACTIVE-WINDOW-GATE | `src/tests/interaction_domain_isolation.rs` |

### M.4 — Checklist

* [x] Only one window can be focused (enforced by `active_window: Option<WindowId>`)
* [x] Input never crosses windows (enforced by INV-151-ACTIVE-WINDOW-GATE)
* [x] Capture is always window-local (enforced by INV-151-FOCUS-TEARDOWN)
* [x] Background windows are inert (enforced by INV-151-ACTIVE-WINDOW-GATE)
* [ ] Modal windows fully isolate parents (deferred — see Section P)

---

## SECTION N — Authoritative Statements

**AUTH-151-1:** "A window is an interaction universe."
**AUTH-151-2:** "Focus never floats; it belongs somewhere."
**AUTH-151-3:** "If input crosses windows, the model is broken."

---

## SECTION O — Verified Invariants

The following invariants are enforced in code and covered by engine-internal tests (`src/tests/interaction_domain_isolation.rs`):

1. **INV-151-WINDOW-ORIGIN**
   Every `RawInputEvent` and `FusedInputEvent` carries `WindowId`.

2. **INV-151-DOMAIN-ISOLATION**
   Interaction state is strictly per-window; no shared or fallback state exists.

3. **INV-151-ACTIVE-WINDOW-GATE**
   Non-active windows cannot deliver input events (except `FocusChanged`, `Resize`, `Tick`).

4. **INV-151-FOCUS-TEARDOWN**
   Focus loss of the active window clears all interaction state (capture, gestures, hover, scroll-capture, focus).

5. **INV-151-NO-GLOBAL-ROUTING-STATE**
   `EventRouter` owns no interaction state.

These invariants are enforced in code and covered by engine-internal tests.

---

## SECTION O.1 — Primary-Only Constraint (Temporary)

The current implementation has a declared limitation:

* The single widget tree belongs to `WindowId::PRIMARY`
* `primary_state(_mut)` calls are guarded by `debug_assert!` at call sites
* Multi-window widget trees are explicitly deferred

This is a **declared limitation**, not an architectural leak.

### INV-151-PRIMARY-ONLY Call Sites

The following call sites use `primary_state_mut()` for widget tree lifecycle operations. Each is guarded by a `debug_assert!` ensuring the active window is PRIMARY or None:

| Operation | Location | Guard |
|-----------|----------|-------|
| Widget destruction cleanup (`on_widgets_destroyed`) | `src/core/app.rs:2443` | `debug_assert!` at line 2435 |
| Focus scope sync (`sync_focus_scopes`) | `src/core/app.rs:2460` | `debug_assert!` at line 2451 |
| Widget destruction cleanup (winit loop) | `src/core/app.rs:4588` | `debug_assert!` at line 4580 |
| Focus scope sync (winit loop) | `src/core/app.rs:4601` | `debug_assert!` at line 4594 |

The guard asserts:
```rust
debug_assert!(
    interaction_domains.active_window() == Some(WindowId::PRIMARY)
        || interaction_domains.active_window().is_none(),
    "INV-151-PRIMARY-ONLY: ... multi-window requires per-window trees"
);
```

When multi-window widget trees are implemented, these call sites will resolve the correct domain per-window instead of assuming PRIMARY.

---

## SECTION P — Deferred / Explicitly Out of Scope

The following are explicitly deferred and require new SPECs:

* Per-window widget trees
* Cross-window drag-and-drop
* OS-level modality semantics beyond focus gating
* Multi-window compositor scene ownership (addressed by SPEC-HAL-082)
* Window-to-window data transfer protocols

These are not forgotten — they are declared out of scope for SPEC-HAL-151.

---

## SECTION Q — Closure Statement

**This SPEC is CLOSED.**

The engine-level interaction domain isolation is complete. Cross-window input leakage is architecturally impossible given the current implementation.

Any changes to multi-window interaction semantics require a new SPEC.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
