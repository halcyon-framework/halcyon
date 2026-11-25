# SPEC-HAL-089 — Focus Memory Policy

Status: Implemented  
Depends on: SPEC-HAL-038 Focus Model  
Author: JC  
Version: 1

## 1. Purpose

Define a **focus memory policy** for Halcyon:

- When and how the system **remembers** which widget was focused.
- When and how that focus is **restored** (or deliberately *not* restored).
- How this interacts with **window activation**, **focus scopes**, and **destruction** of widgets.

This SPEC turns the ad-hoc “remember last focus on deactivation and restore on activation” behavior (currently in `FocusManager` + `EventRouter`) into a formal, testable contract.

---

## 2. Problem Statement

Right now:

- SPEC-HAL-038 Stages 9–10 implement:
    - Blur on window deactivation.
    - Auto-restore focus on window activation (with guards).
- The behavior is correct, but the *policy* is implicit:
    - Which transitions write to focus memory.
    - Which transitions restore from it.
    - Which transitions clear it.
    - What happens with scopes, destroyed widgets, or app-level overrides.

We need a **clear policy** so:

- You know exactly when a widget will be auto-refocused.
- ESC/scope/modals don’t accidentally “resurrect” unwanted focus.
- Future extensions (multi-window, multi-document, custom app policies) have a stable base.

---

## 3. Non-Goals

- No new visual styling or decoration.
- No changes to Tab/Shift-Tab focus navigation semantics (SPEC-HAL-038 Stages 2–3).
- No complex “multi-slot” historical focus stacks (that’s a later SPEC if ever).
- No cross-application or OS-global behavior (strictly inside a Halcyon app window).

---

## 4. Terms

- **Focused widget** — the widget that currently owns keyboard focus.
- **Focus memory** — an internal slot storing a `TreeWidgetId` to be potentially restored later.
- **Window deactivation** — Halcyon window loses OS focus (Alt-Tab away, click another app, etc.).
- **Window activation** — Halcyon window regains OS focus.
- **Focus scope** — logical focus island (modal, palette, etc.) as defined in SPEC-HAL-038.

---

## 5. Policy Overview

### 5.1 High-level Rules

1. **Deactivation = remember + blur**

    - When the OS deactivates the window, Halcyon:
        - Stores the currently focused widget into focus memory.
        - Blurs it (fires `on_blur`) and clears focus.

2. **Activation = attempt restore**

    - When the OS activates the window, Halcyon:
        - *Attempts* to restore focus from memory under strict conditions.
        - If any condition fails, no restore happens.

3. **ESC is a hard user decision**

    - ESC-driven focus changes (clearing focus or closing scopes) **do not** write focus memory.
    - ESC is treated as “the user intentionally broke the chain”.

4. **Scopes have priority over memory**

    - Active focus scopes can **block** restoration.
    - Memory must not revive a widget that conflicts with active modal/scope rules.

5. **Destroyed / non-focusable widgets are never restored**

    - If the widget is gone or no longer focusable, memory is ignored.

6. **User actions beat memory**

    - If the user already focused something manually after activation, auto-restore must not overwrite that.

---

## 6. Current Implementation Snapshot (Baseline)

This SPEC starts from the behavior already implemented in SPEC-HAL-038 Stage 10:

- `FocusManager` has:

    - `focused: Option<TreeWidgetId>`
    - `last_focused_before_deactivate: Option<TreeWidgetId>`

- `FocusManager` methods already exist:

    - `remember_focus(id)`
    - `take_last_focus()`
    - `has_remembered_focus()`

- `EventRouter`:

    - On `FusedInputEvent::FocusChanged { focused: false }`:
        - If something is focused → `remember_focus(id)`, then `clear_focus(tree)`.
    - On `FusedInputEvent::FocusChanged { focused: true }`:
        - Calls `try_restore_focus(tree)` with conditions:
            - No widget currently focused.
            - No active focus scope.
            - Widget still exists.
            - Widget still focusable.

- ESC behavior:

    - `handle_escape`:
        - If scope active → pop scope.
        - Else if widget focused → `clear_focus(tree)`.
        - Else → no-op.
    - ESC uses `clear_focus` **without writing focus memory** (good, we keep this).

This SPEC **codifies** that behavior and **extends it slightly**.

---

## 7. Behavioral Matrix (Informal)

Describe in human language, not code:

1. **Alt-Tab away from Halcyon window**

    - If widget A is focused:
        - Memory = A
        - A gets `on_blur`
        - Focus = None

2. **Alt-Tab back**

    - If:
        - No widget is focused
        - No focus scope active
        - Widget A still exists
        - A is still focusable
    - Then:
        - Focus = A
        - A gets `on_focus`
        - Memory is consumed.

3. **Widget destroyed while app is inactive**

    - Deactivation remembered A.
    - A is destroyed while inactive.
    - On activation:
        - Memory points to a dead ID → no restore.
        - Memory is cleared.

4. **Widget no longer focusable**

    - Deactivation remembered A.
    - A changes `focusable() -> false`.
    - On activation:
        - No restore. Memory cleared.

5. **Scope active at activation**

    - Deactivation remembered A (outside or inside a scope).
    - While inactive, a focus scope becomes active (e.g., modal opened).
    - On activation:
        - Scope is active → auto-restore is blocked.
        - Memory is dropped.
        - Focus stays None (or scope-managed).

6. **User focuses something before or around activation**

    - If user interaction (click, Tab, etc.) already gave focus to some widget B by the time restore runs:
        - Restore is skipped.
        - Memory is put back or dropped depending on policy (defined below).

---

## 8. Policy Details

### 8.1 When to write focus memory

**WRITE focus memory only when:**

1. Window deactivation (`FocusChanged { focused: false }` from the platform) *and*:
    - Some widget is currently focused.

2. (Optional future stage) App explicitly calls a “snapshot” API — not in scope for Stage 1.

**DO NOT write focus memory when:**

- ESC blurs focus.
- App programmatically clears focus for UX reasons.
- A focus scope is popped and focus is reassigned as part of scope logic.
- Tab navigation moves focus — this is a normal in-window transition.

The guiding idea: **only OS-induced deactivation** populates the automatic memory slot.

### 8.2 When to clear focus memory

Memory should be cleared when:

- It has been successfully used to restore focus.
- The referenced widget no longer exists.
- The referenced widget is no longer focusable.
- A focus scope has blocked restoration (we treat this as “memory invalid in this world”).

Memory should **not** automatically clear just because focus moved, *unless* that movement is a deliberate “hard break” (ESC, closing scope, etc.). For now we keep it simple:

- If `try_restore_focus` runs and fails due to:
    - missing widget
    - non-focusable widget
    - active scope
      → memory is dropped.

- If `try_restore_focus` sees that some widget is already focused:
  → it does **not** override; it may choose to:
    - re-store the memory (current behavior: store back), or
    - drop it (if we decide the new focus breaks the chain).

SPEC-HAL-089 will pick one.

### 8.3 Final decision for user-overridden focus

In case:

- Window activates,
- Auto-restore runs,
- But user focus was already set (e.g. click in another field that races with FocusChanged events):

Policy:

- **Do not restore** (user wins).
- **Drop memory**.
    - Reason: user has clearly moved to a different context; we don’t keep undead focus around.

This is a small change from current behavior (currently it re-`remember_focus`), and will be done in a later stage.

---

## 9. Interactions with Focus Scopes

Rules:

1. If a **scope is active** at activation time:
    - Auto-restore is **blocked**.
    - Memory is **dropped**.
    - Scope logic is responsible for focus (e.g., default button in modal, first field, etc.).

2. If focus was inside a scope when deactivated, and that scope still exists and is active:
    - Auto-restore must **not** bypass the scope’s own rules.
    - For now: we block and drop memory regardless of where A was.

3. If the scope was closed while inactive:
    - If A was inside that scope -> widget is gone → normal “dead widget” handling applies.
    - If A was outside the scope → allowed to be restored *if* no new scope is active.

This keeps the rule simple: **active scope acts as a firewall**.

---

## 10. Staging Plan

### Stage 1 — Codify and freeze current behavior

Scope: minimal

- Document current behavior as baseline, with tests:
    - Deactivate → remember + blur.
    - Activate → `try_restore_focus` with conditions:
        - no focused widget
        - no active scope
        - widget exists
        - widget focusable
- Add/extend tests in:
    - `tests/focus_window_activation.rs`
    - `tests/focus_auto_restore.rs`
- No behavior changes, just SPEC + comments aligned with 089.

### Stage 2 — Tighten user-override semantics

Scope: small, targeted

- Change `try_restore_focus` behavior when a widget is already focused:
    - Current: re-`remember_focus(last_focus)` (memory preserved).
    - New: drop memory.
- Justification: user actions override OS-based memory.
- Update/add tests:
    - “restore_skipped_if_user_reassigned_focus_early” → expect memory cleared.

### Stage 3 — Scope-aware memory policy

Scope: moderate

- Clarify and enforce that:
    - If a focus scope is active at activation, auto-restore is blocked.
    - Memory is **always** cleared in that case.
- Add tests:
    - “restore_blocked_by_scope_drops_memory”:
        - Deactivate (A focused), memory = A.
        - While inactive: open a modal (scope).
        - Activate:
            - Confirm no restore.
            - Confirm memory is empty.

### Stage 4 — App-level hooks (optional)

Scope: future, can be skipped for now

- Introduce optional hooks or APIs:
    - `AppFocusPolicy` or similar, allowing app to:
        - Mark certain widgets as “non-restorable” (`no_focus_memory`).
        - Override restoration for specific situations (e.g., security-sensitive fields).
- This stage is **not** required to stabilize base behavior. It’s an extension.

---

## 11. Implementation Notes

- All logic remains inside:
    - `FocusManager` (memory storage).
    - `EventRouter` (policy execution).
- No renderer/layout/text involvement.
- No new public API exposed yet; behavior is entirely internal.

---

## 12. Acceptance Criteria

SPEC-HAL-089 is considered implemented (Stages 1–3) when:

1. The behavior under:
    - Window deactivate/activate,
    - ESC,
    - Focus scopes,
    - Widget destruction / becoming non-focusable,
    - User focus override
      is fully covered by tests in `tests/focus_window_activation.rs` and `tests/focus_auto_restore.rs`.

2. The following invariants hold:

    - Window deactivation always blurs and writes memory **if** something was focused.
    - Window activation either:
        - Restores focus once, then clears memory, **or**
        - Declines restoration and clears memory on:
            - dead widget,
            - non-focusable widget,
            - active scope,
            - user already focused something else.
    - ESC never writes or uses focus memory.

3. No regressions in existing focus tests (tab navigation, capture, scopes, ESC, etc.).
