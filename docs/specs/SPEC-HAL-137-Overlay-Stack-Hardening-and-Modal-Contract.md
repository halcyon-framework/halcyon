# SPEC-HAL-137: Overlay Stack Hardening and Modal Contract

**Status:** Design / Binding
**Layer:** Engine coordination (event routing, focus, capture, overlay stacking)
**Scope:** Overlays: modal dialogs, popovers, menus, tooltips, transient panels
**Depends on:** SPEC-HAL-018, SPEC-HAL-122, SPEC-HAL-123, SPEC-HAL-127, SPEC-HAL-128
**Refined by:** SPEC-HAL-235 (central focus/restoration authority), SPEC-HAL-236 (domain overlay admission)
**Constrains:** Engine event pipeline, focus manager, overlay stack owner, widget event routing

---

## SECTION A — Problem Statement

Halcyon has overlays (menus, tooltips/popovers, dialogs) and already has contracts for overlay stack and capture (SPEC-HAL-127/128), but the overall overlay model must be **rock-solid and deterministic**:

* multi-overlay stacking must be explicit and stable
* modal input capture must be structural, not heuristic
* Escape/dismiss routing must be unambiguous
* focus transfer + restoration must be defined
* pointer capture interaction with overlays must be defined
* overlays must not trigger surprise work (layout/realization) outside the frame pipeline

This SPEC hardens the overlay system into a single engine-owned contract usable by higher-level systems (confirmation/validation pipeline, editors, wizards, etc.) without inventing bespoke ad-hoc behavior per widget.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Engine owns overlays**
Widgets may request overlays. They do not coordinate overlays among themselves.

**PRINCIPLE-2: Determinism over convenience**
For any input event, it must be possible to answer “who receives it?” with zero ambiguity.

**PRINCIPLE-3: Modal means captured**
A modal overlay structurally captures all input (except explicitly whitelisted global shortcuts).

**PRINCIPLE-4: Non-modal means non-capturing**
Tooltips/popovers may block-hit-test locally but do not capture global input unless explicitly configured.

**PRINCIPLE-5: No surprise work**
Overlay open/close never triggers synchronous layout/realization/anchor resolution. It schedules state that is processed in the normal frame phases.

---

## SECTION C — Terminology

**Overlay**
A UI surface rendered above the base content, inserted into the overlay stack.

**Overlay Stack**
Engine-owned ordered set of overlays, topmost last.

**Overlay Kind**
One of: `Tooltip`, `Popover`, `Menu`, `Panel`, `Dialog`, `SystemModal`.

**Modal Overlay**
Overlay that captures keyboard and pointer routing globally.

**Dismiss Policy**
Rules for closing: Escape, click outside, selection commit, timeout, explicit close.

**Focus Trap**
Constraint that focus navigation must remain within the modal overlay subtree.

**Owner**
Logical source of overlay request (widget id / feature id).

---

## SECTION D — Overlay Classification and Capabilities

### D.1 Overlay kinds

**INV-OVL-KIND-1**
Every overlay instance declares an `OverlayKind`.

**INV-OVL-KIND-2**
`Tooltip` is always non-modal and non-focusable.

**INV-OVL-KIND-3**
`Menu` is non-modal but input-prioritized; it may consume pointer/keys while open.

**INV-OVL-KIND-4**
`Dialog` is modal and focusable.

### D.2 Capabilities flags

Each overlay declares:

* `is_modal: bool`
* `takes_focus_on_open: bool`
* `traps_focus: bool`
* `dismiss_on_escape: bool`
* `dismiss_on_click_outside: bool`
* `blocks_pointer_to_underlay: bool`

**INV-OVL-CAPS-1**
Capabilities are immutable for the lifetime of an overlay instance.

---

## SECTION E — Stacking and Lifetime

### E.1 Stack ordering

**INV-OVL-STACK-1**
Overlay stack order is strict total order; topmost overlay is the last element.

**INV-OVL-STACK-2**
Topmost modal overlay (if any) defines the **active modal boundary**.

**INV-OVL-STACK-3**
Non-modal overlays may exist above a modal overlay only if explicitly declared `allowed_above_modal=true` (default false).

### E.2 Overlay identity

**INV-OVL-ID-1**
Each overlay has a stable `OverlayId` for its lifetime.

**INV-OVL-ID-2**
Overlay open/close is performed only through the engine overlay coordinator by `OverlayId`.

### E.3 Lifetime semantics

**INV-OVL-LIFE-1**
An overlay is either `Opening`, `Open`, `Closing`, or `Closed`. No other states.

**FORBID-OVL-REENTRANT-CLOSE**
Closing an overlay must be idempotent; repeated close requests are ignored.

---

## SECTION F — Input Routing Contract

### F.1 Global routing priority

For each input event:

1. active pointer capture target (SPEC-HAL-128), if applicable
2. topmost modal overlay subtree, if present
3. topmost non-modal overlay that blocks pointer to underlay (hit-test gated)
4. base content tree

**INV-OVL-INPUT-1**
If a modal overlay exists, **no base content widget** may receive input events (keyboard or pointer), except whitelisted global shortcuts.

**INV-OVL-INPUT-2**
If pointer capture exists to a widget outside the active modal boundary, the capture must be cancelled immediately when the modal opens.

**INV-OVL-INPUT-3**
Non-modal overlays may consume pointer events only when hit-tested within their bounds (unless they explicitly capture pointer via SPEC-HAL-128 rules).

### F.2 Escape handling

**INV-OVL-ESC-1**
Escape is routed to the topmost overlay with `dismiss_on_escape=true`.

**INV-OVL-ESC-2**
If no overlay consumes Escape, it bubbles to base content (normal rules).

**FORBID-OVL-ESC-AMBIG**
Escape must never dismiss multiple overlays in one keypress.

### F.3 Click outside handling

**INV-OVL-OUTSIDE-1**
If `dismiss_on_click_outside=true`, a pointer-down outside the overlay bounds dismisses that overlay **only if it is topmost among overlays that request outside-dismiss**.

**INV-OVL-OUTSIDE-2**
Outside-dismiss must not leak the same pointer-down to base content in the same event cycle (no “click-through”).

---

## SECTION G — Focus Semantics

### G.1 Focus on open

**INV-OVL-FOCUS-OPEN-1**
If `takes_focus_on_open=true`, the engine assigns focus to the overlay’s initial focus target during the next focus phase (not synchronously in input handler).

**INV-OVL-FOCUS-OPEN-2**
If `takes_focus_on_open=false`, focus remains unchanged (but may become inactive if modal boundary forbids it).

### G.2 Focus trap (modal)

**INV-OVL-FOCUS-TRAP-1**
If `traps_focus=true`, focus navigation (Tab / Shift+Tab / arrow-based focus moves where applicable) must remain inside the overlay subtree.

**FORBID-OVL-FOCUS-LEAK**
No focus move may land on a widget outside the active modal boundary while the modal is open.

### G.3 Focus restore on close

**INV-OVL-FOCUS-RESTORE-1**
Engine stores `previous_focused_widget` on overlay open if it takes focus.

In a SPEC-HAL-235 focus domain this testimony is stored exactly once as
`FocusRestorationFrame::superseded_target` inside the authoritative per-domain
`FocusManager`. Overlay owners retain only opaque restoration tokens and must not
duplicate or reinterpret the previous target.

**INV-OVL-FOCUS-RESTORE-2**
On close, focus restores to `previous_focused_widget` if it remains registered,
eligible, and inside the surviving superseded focus domain; otherwise the central
resolver selects the deterministic lawful fallback (nearest ancestor when
available, or none). A fallback may not cross an exiting overlay-domain subtree.

---

## SECTION H — Pointer Capture Interaction

**INV-OVL-CAPTURE-1**
Pointer capture may only exist within the active modal boundary when a modal overlay is open.

**INV-OVL-CAPTURE-2**
If a non-modal overlay is opened, existing pointer capture remains valid unless the overlay explicitly requests capture cancellation.

**FORBID-OVL-CAPTURE-CROSS-MODAL**
Pointer capture across modal boundary is forbidden.

---

## SECTION I — Scheduling and Frame Phases

**INV-OVL-SCHED-1**
Overlay open/close requests are recorded as intents and applied in a dedicated engine phase before `layout_tree()`.

**INV-OVL-SCHED-2**
Applying overlay intents must not call layout, realization, or anchor resolution synchronously.

**INV-OVL-SCHED-3**
Dismiss actions arising from input (Escape/outside click) enqueue close intents; they do not mutate overlay stack immediately inside event routing.

---

## SECTION J — Minimal API Surface

Engine owns:

* `OverlayCoordinator`

    * `open_overlay(spec) -> OverlayId`
    * `close_overlay(OverlayId)`
    * `topmost_modal() -> Option<OverlayId>`
    * `route_input(event) -> RoutedTarget`

**FORBID-OVL-WIDGET-STACK**
Widgets must not maintain independent overlay stacks.

**FORBID-OVL-SHARED-MUT**
No overlay may mutate base widgets directly as a side effect of being opened/closed (only via explicit commands/events).

---

## SECTION K — Acceptance Criteria

SPEC-HAL-137 is satisfied when:

* [ ] Escape always dismisses exactly one overlay (topmost eligible)
* [ ] Outside click dismiss does not click-through
* [ ] Modal overlay prevents any base content input
* [ ] Focus trap works: focus cannot leave modal subtree
* [ ] Focus restore on close works deterministically
* [ ] Pointer capture cannot cross modal boundary
* [ ] Overlay open/close does not trigger synchronous layout/realization
* [ ] Tests exist covering: stacking order, modal boundary, escape routing, outside click, focus restore, capture cancellation

---

## SECTION L — Authoritative Statements

**AUTH-137-1: “Modal is structural capture, not convention.”**
**AUTH-137-2: “Dismiss is deterministic: one event, one target.”**
**AUTH-137-3: “Overlay lifecycle is scheduled, never synchronous surprise work.”**
