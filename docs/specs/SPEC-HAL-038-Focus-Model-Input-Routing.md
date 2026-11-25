# **SPEC-HAL-038 — Focus Model & Input Routing Hierarchy**

**Author:** JC
**Date:** 2025-11-24
**Status:** Implemented / Locked
**Category:** Core / Input / Architecture
**Target:** `core::event.rs`, `core::event_router.rs`, `core::widget.rs`, `core::widget_tree.rs`, `widgets/*`, `tests/`

---

## 1. Purpose

This SPEC defines a **formal focus model** and its integration with the **input routing hierarchy**.

We already have:

* Event routing and phases (SPEC-HAL-009)
* Widget identity & lifecycle (SPEC-HAL-037)
* TextField & advanced input planned (SPEC-HAL-013, 014)
* Dirty flags, diffing, caching (015, 026, 035)

What’s missing:

* A single, clear answer to **“who gets keyboard input right now?”**
* How focus moves (Tab, click, programmatic)
* How focus respects **scopes** (dialogs, panels, docked views)
* How focus interacts with event routing and shortcuts

Goal:

> Deterministic, scope-aware keyboard focus and input routing,
> suitable for trading-grade hotkeys, text fields, and complex layouts.

---

## 2. Concepts & Definitions

### 2.1. Focus

**Focus** is the **exclusive right** to receive:

* keyboard events (KeyDown / KeyUp / TextInput),
* focus-specific events (`FocusGained`, `FocusLost`),
* and to draw focus indicators (focus ring, caret).

At any point in time:

* There is **zero or one** *focused widget* per **focus tree**.
* Focus is identified by `WidgetId`.

### 2.2. Focusable Widget

A widget is **focusable** if:

* it opts in via the `Widget` trait (`fn focusable(&self) -> bool`),
* and it lives in a focusable scope.

Examples:

* Focusable: `TextField`, `Button`, `Checkbox`, list items, chart panes.
* Non-focusable: `Spacer`, `Label`, purely decorative panels.

### 2.3. Focus Scope

A **focus scope** is a subtree in which:

* Tab/Shift+Tab traversal is limited,
* default focus can be chosen,
* modal dialogs can trap focus.

Examples:

* A modal dialog box
* A docked panel
* A sub-pane in an application

There is always a **root scope** (the whole app), and optionally nested scopes.

### 2.4. Active Scope

The **active scope** is the scope that:

* currently owns focus,
* receives Tab-order traversal,
* traps keyboard navigation if modal.

A modal dialog, when active, becomes the **only** focus scope for keyboard traversal until dismissed.

---

## 3. High-Level Behavior

### 3.1. Keyboard Events

1. **Focused widget** gets first shot at:

    * `KeyDown`, `KeyUp`, `TextInput`, `Character`, etc.
2. If the focused widget:

    * **handles** the event → event stops.
    * **does not handle** the event → event may bubble upward to:

        * its parent,
        * the scope root,
        * global shortcuts handler.

### 3.2. Pointer Events and Focus

* Click on a focusable widget:

    * requests focus for that widget.
* Click on non-focusable area:

    * optional: clear focus
    * or leave focus as-is (configurable per app)

For TextField and similar:

* Click inside → set focus to that widget.
* Click outside (empty area) → usually **clears focus**.

### 3.3. Tab Navigation

* `Tab` moves focus to **next** focusable widget in the active scope.
* `Shift+Tab` moves focus to **previous** focusable widget.
* Traversal order:

    * primary: tree order (depth-first, consistent),
    * optional future: explicit tab-order index.

If there is no next widget:

* wrap-around within the same scope (`last → first` and reverse)
  — unless a future SPEC overrides this behavior.

### 3.4. Focus Scopes & Modals

A focus scope can be:

* **normal**: participates in global focus chain.
* **modal**: captures keyboard focus and prevents Tab from leaving the scope.

When a modal scope is active:

* Tab/Shift+Tab stays inside that scope.
* Global shortcuts may still be processed, but **text navigation and widget focus do not escape**.

---

## 4. Data Structures

### 4.1. FocusManager

We introduce a **FocusManager** owned by the app/event layer.

Conceptual struct (no code here, just shape):

```rust
pub struct FocusManager {
    /// Currently focused widget (if any)
    focused: Option<WidgetId>,

    /// Active scope root (WidgetId)
    active_scope_root: WidgetId,

    /// Map: scope_root → ordered list of focusable children
    scopes: HashMap<WidgetId, Vec<WidgetId>>,
}
```

Responsibilities:

* track which widget is focused,
* maintain which **scope** is active,
* maintain per-scope focusable lists,
* provide navigation (next, previous) within scope.

### 4.2. FocusNode (Conceptual)

Focus information per widget:

```rust
pub struct FocusNode {
    pub widget_id: WidgetId,
    pub parent_scope: WidgetId, // nearest scope root
    pub focusable: bool,
}
```

This can be stored:

* directly in `WidgetTree`,
* or derived when building focus scopes.

---

## 5. Integration with WidgetTree

### 5.1. Scope Roots

By default:

* The **root widget** is the root focus scope.

Any widget can opt in as a **scope root** by:

* implementing a method or flag, e.g.:

```rust
trait Widget {
    fn is_focus_scope_root(&self) -> bool {
        false
    }
}
```

Examples:

* Modal dialog, tab panel, main app container.

### 5.2. Focus Rebuild

On each “structure-changed” event:

* when the widget tree changes in a way that may affect focusable widgets:

    * add/remove widgets,
    * scope creation/destruction,

we must rebuild the focus information.

Algorithm (high-level):

1. DFS over the widget tree.
2. Track current scope root as you descend.
3. For each widget:

    * if `is_focus_scope_root()` → treat as new scope root.
    * if `focusable()` → add its `WidgetId` to that scope’s list.

This produces `scopes: HashMap<WidgetId, Vec<WidgetId>>`.

We **reuse** this for Tab navigation.

---

## 6. Widget Trait Extensions

### 6.1. Focusable & Focus Events

Extend `Widget` trait with minimal focus API:

```rust
trait Widget {
    /// Can this widget acquire keyboard focus?
    fn focusable(&self) -> bool {
        false
    }

    /// Called when this widget becomes focused
    fn on_focus(&mut self, _cx: &mut EventContext) {}

    /// Called when this widget loses focus
    fn on_blur(&mut self, _cx: &mut EventContext) {}
}
```

### 6.2. Scope Root

As above:

```rust
fn is_focus_scope_root(&self) -> bool {
    false
}
```

By default, `false` for all widgets, except:

* main app container,
* modals,
* docked panes (as needed).

---

## 7. Event Routing Model (Keyboard)

We integrate focus into `EventRouter` semantics.

### 7.1. Keyboard Path

When a keyboard event arrives:

1. Ask `FocusManager` for `focused: Option<WidgetId>`.
2. If `Some(widget_id)`:

    * route the event **directly** to that widget as target.
    * apply the usual event phases (Capture/Target/Bubble), but with:

        * Capture/Bubble along the parent chain of that widget, **within the active scope**.
3. If `None`:

    * route to active scope root,
    * optionally trigger “global” key handlers.

### 7.2. Pointer → Focus Interaction

When pointer events (e.g. `MouseDown`) are processed:

1. Normal hit-testing determines `target_widget_id`.
2. If widget is `focusable()` and event indicates activation (e.g. left-button click):

    * `FocusManager::request_focus(widget_id)` is called.

`request_focus()`:

* If same as current focus → no change.
* If different:

    * call `on_blur()` on old focused widget (if any),
    * set `focused = Some(widget_id)`,
    * call `on_focus()` on new widget.

### 7.3. Tab/Shift+Tab Handling

Special handling for:

* `KeyDown { key: Tab, modifiers }`.

Algorithm:

1. If there is an `active_scope_root`, get its focus list (`Vec<WidgetId>`).
2. Determine current index of `focused` in this list.
3. If `Tab` and no `Shift`:

    * next index = `(current_index + 1) % len`.
4. If `Tab` + `Shift`:

    * prev index = `(current_index + len - 1) % len`.
5. Request focus for that widget via `FocusManager`.

If there is **no focused widget** in the scope:

* Tab moves to the **first** focusable widget.
* Shift+Tab moves to the **last** focusable.

---

## 8. Active Scope Management

### 8.1. Default

By default:

* `active_scope_root` = root widget’s `WidgetId`.

### 8.2. Modals

When a modal is shown:

* It declares itself as a focus scope root (`is_focus_scope_root() == true`).
* App / widget opens it and calls:

```rust
focus_manager.set_active_scope(modal_widget_id);
focus_manager.request_default_focus_in_scope(modal_widget_id);
```

`request_default_focus_in_scope`:

* if scope has any focusable widgets:

    * focus the first one,
* otherwise:

    * focus the scope root itself (if focusable),
    * or leave focus empty.

When modal closes:

* `set_active_scope(previous_scope_root)`
* focus may:

    * return to the previously focused widget (if still alive),
    * or be recomputed.

---

## 9. Styling & Theme Integration

Focus is not just routing — it’s visible.

### 9.1. Theme Hooks

In `Theme` (SPEC-HAL-012), add:

* `focus_ring_color`
* `focus_ring_width`
* optional `focus_glow` or similar

### 9.2. Widget Behavior

Focus-aware widgets:

* `TextField`:

    * shows caret, selection, and border when focused.
* `Button`:

    * highlight border or outer glow.

Paint logic must check:

* `cx.focus_manager.is_focused(self.id())`
  (or equivalent accessor out of `PaintContext`).

---

## 10. Interaction with Other SPECs

### 10.1. SPEC-HAL-013, 014 (TextField & Advanced Input)

This SPEC is a **hard dependency**:

* TextField requires focus for:

    * text input,
    * selection,
    * navigation keys.

### 10.2. SPEC-HAL-033 (Input Fusion Layer)

Input fusion (combining mouse/keyboard/shortcuts) will depend on:

* active scope,
* focused widget,
* the bubble path in the focus tree.

### 10.3. SPEC-HAL-036 (Input Coalescing)

Coalesced input still routes to:

* focused widget (keyboard),
* hit-tested widget (pointer).

Nothing new, but cleaner guarantees.

---

## 11. Implementation Checklist (for Claude Code)

No code, just exact steps and targets.

### 11.1. Core Types & Trait Extensions

1. **Update `src/core/widget.rs`:**

    * Add to `Widget` trait:

        * `fn focusable(&self) -> bool { false }`
        * `fn on_focus(&mut self, _cx: &mut EventContext) {}`
        * `fn on_blur(&mut self, _cx: &mut EventContext) {}`
        * `fn is_focus_scope_root(&self) -> bool { false }`

    * Do **not** change existing behavior in widgets yet (no-op defaults).

2. **Update `src/core/event.rs`:**

    * Ensure `Event` enum has:

        * `KeyDown`, `KeyUp`, and text input variants.
        * Add `FocusGained(WidgetId)` and `FocusLost(WidgetId)` if not already present.
    * If focus events already exist, align semantics to this SPEC.

### 11.2. FocusManager Skeleton

3. **Create a FocusManager type** in `src/core/event_router.rs` or a new `focus.rs` module under `core/`:

    * Store:

        * `focused: Option<WidgetId>`
        * `active_scope_root: WidgetId`
        * `scopes: HashMap<WidgetId, Vec<WidgetId>>`

    * Methods (signatures only, simple logic first):

        * `fn set_active_scope(&mut self, root: WidgetId)`
        * `fn rebuild_scopes(&mut self, tree: &WidgetTree)`
        * `fn request_focus(&mut self, id: WidgetId, tree: &mut WidgetTree, cx: &mut EventContext)`
        * `fn clear_focus(&mut self, tree: &mut WidgetTree, cx: &mut EventContext)`
        * `fn handle_tab_navigation(&mut self, shift: bool, tree: &mut WidgetTree, cx: &mut EventContext)`

    * Inside `request_focus`:

        * If `Some(old)` and `old != id`:

            * send `FocusLost` to `old` and call `on_blur`.
        * Set `focused = Some(id)`:

            * send `FocusGained` to `id` and call `on_focus`.

4. **Add scope rebuild**:

    * Implement `rebuild_scopes()` as a DFS over `WidgetTree`:

        * track current scope root while descending,
        * for each widget:

            * if `is_focus_scope_root()` → treat as new root,
            * if `focusable()` → push its `WidgetId` into that root’s `Vec<WidgetId>`.

    * Keep behavior simple and correct; no optimization needed yet.

### 11.3. EventRouter Integration

5. **Update `src/core/event_router.rs` keyboard handling:**

    * For keyboard events:

        * If `FocusManager.focused` is `Some(id)`:

            * route event to that widget as target.
        * If `None`:

            * route to `active_scope_root` or root widget.

    * For `Tab` / `Shift+Tab`:

        * intercept in router before sending to widget,
        * call `focus_manager.handle_tab_navigation(shift, ...)`,
        * mark event as handled.

6. **Update pointer handling in EventRouter:**

    * On pointer down (e.g. left button) on a hit-tested widget:

        * if `widget.focusable()`:

            * call `focus_manager.request_focus(widget_id, ...)`.

    * Decide policy for clicking empty space:

        * For now: do **not** clear focus automatically.
        * This can be adjusted later in TextField SPEC or app layer.

### 11.4. Widget Implementations

7. **In `src/widgets/button.rs`:**

    * Implement `focusable() -> true`.
    * Optionally, in `paint()`:

        * check via `PaintContext` (once focus info is exposed) and draw a simple focus indicator:

            * e.g. inner/outer border.

8. **In `src/widgets/text_block.rs`:**

    * Keep non-focusable by default.

9. **In `TextField` (when SPEC-HAL-013 is implemented or already present):**

    * Make it `focusable() -> true`.
    * On `on_focus()`:

        * enable caret and selection rendering.
    * On `on_blur()`:

        * clear selection, hide caret, commit any pending edits if needed.

10. **Scope examples (future):**

* When modal dialog / panel widgets are added:

    * override `is_focus_scope_root() -> true`.

### 11.5. Tests

11. **Add tests in `tests/event_routing_tests.rs` or new `focus_tests.rs`:**

* Case 1: Click → focus:

    * create simple app with two Buttons,
    * simulate click on button A: `FocusManager.focused == A`,
    * simulate click on button B: `FocusManager.focused == B` and A receives blur.

* Case 2: Tab navigation:

    * create scope with three focusable widgets,
    * set focus to first,
    * send Tab: ensure focus rotates in correct order,
    * send Shift+Tab: ensure reverse order.

* Case 3: Unmount:

    * focus a widget,
    * remove that widget from the tree,
    * ensure:

        * `on_unmount()` called,
        * focus cleared or moved (depending on simple strategy).

---

## 12. Locked Invariants

The following invariants are established by this SPEC and MUST NOT be violated.

### INV-FOCUS-VIEWPORT-ENTRY

TAB focus entry operates at **widget level only**.

When no widget is currently focused:
- TAB focuses the **first visible focusable widget** in viewport
- Visibility = **screen-space intersection**, not content-space

List items are **not** part of TAB navigation. Internal widget navigation (e.g., arrow keys within lists) is activated **only after widget focus** has been established.

This is intentional and required for scalability and encapsulation.

### INV-FOCUS-COORDSPACE

All visibility tests for focus entry MUST be done in **screen space**.

- Screen space = `absolute_paint_rect - cumulative_scroll_offset`
- Viewport = root/window paint rect

Mixing content-space and screen-space is forbidden.

---

## 12.1 Focus vs Visual Reveal — Binding Semantics

This section establishes binding law for the separation between structural
focus (navigational ownership) and visual reveal (scroll-into-view behavior).
These invariants are engine-grade, survive refactors, and override widget
preferences. Violations are engine bugs.

---

### INV-FOCUS-STRUCTURAL-1 — Focus is navigational

Focus represents **navigational ownership only**.

Focus establishes:
- Which widget receives keyboard events
- Which widget draws focus indicators
- Navigation traversal position (Tab/Shift+Tab)

Focus MUST NOT:
- Capture scroll
- Consume scroll input
- Mutate widget state beyond focus indicator
- Adjust viewport position (directly)

Focus is an input routing mechanism, not a visual positioning mechanism.

---

### INV-FOCUS-VISUAL-1 — Focus does not imply reveal

Structural focus alone MUST NOT cause scrolling or viewport adjustment.

The act of gaining focus is separate from the act of scrolling into view.
Focus-follow-scroll is an explicit, opt-in behavior — not an automatic
consequence of focus acquisition.

---

### INV-FOCUS-VISUAL-2 — Visual focus target is widget-defined

Only widgets that explicitly declare a **visual focus target** may request
focus-follow-scroll.

The engine MUST NOT infer or guess which widgets want scroll reveal.
Widgets opt in explicitly by:
- Declaring themselves as leaf interactive widgets
- Being part of an explicit reveal registration

Default expectation:
- Leaf interactive widgets (Button, TextField, Checkbox) → opt-in to reveal
- Container widgets (ScrollArea, List, FlowGrid, Panel) → opt-out

---

### INV-FOCUS-VISUAL-3 — Containers never self-reveal

Container widgets MUST NOT trigger focus-follow-scroll for their own rect.

Containers include but are not limited to:
- ScrollArea
- List
- FlowGrid
- Composite layouts
- Panels and panes

Containers may:
- Manage internal visibility via their own mechanisms (anchor systems, virtualization)
- Forward reveal requests to their children

Containers MUST NOT:
- Scroll themselves into view when they gain focus
- Request reveal for their own bounding rect

---

## 12.2 Interaction States: Focus vs Capture — Binding Semantics

This section establishes binding law for the distinction between focus and
capture as separate interaction states. These invariants are engine-grade.

---

### INV-INTERACTION-STATE-1 — Focus and capture are distinct

Focus and capture are **separate interaction states** with non-overlapping authority.

| State   | Purpose                    | Authority                          |
|---------|----------------------------|------------------------------------|
| Focus   | Navigational ownership     | Keyboard routing, focus indicators |
| Capture | Interaction ownership      | Exclusive input consumption        |

Focus enables navigation.
Capture enables interaction.

They MUST NEVER be conflated or treated as synonyms.

---

### INV-INTERACTION-STATE-2 — Capture is explicit

A widget may enter capture state ONLY via an explicit user action.

Explicit capture actions:
- Pointer down inside widget bounds
- Scrollbar thumb grab
- Explicit activation gesture (Enter key on certain widget types)

FORBIDDEN:
- Implicit capture on focus
- Implicit capture on hover
- Implicit capture on scroll

Focus alone does not grant capture. Capture must be earned through
explicit user intent.

---

### INV-INTERACTION-STATE-3 — Capture is exclusive

While a widget is in capture state:

- Input routing is exclusive to the captured widget
- Navigation outside the widget is suspended (Tab does not change focus)
- Scroll does not bubble or escape to ancestors
- Pointer events outside the widget may still be delivered (for drag tracking)

Capture creates an interaction "bubble" that isolates the widget from
normal input routing until capture is released.

---

### INV-INTERACTION-STATE-4 — Escape semantics are layered

Escape key behavior follows a strict hierarchy:

1. **First Escape**: Exits capture state
   - Widget returns to focused-but-not-captured state
   - Focus remains on the widget
   - Normal navigation resumes

2. **Subsequent Escape**: Affects navigation context
   - May clear focus
   - May exit focus scope
   - Behavior is context-dependent (out of scope for this SPEC)

Widgets MUST NOT clear focus when exiting capture unless explicitly
specified by widget-specific semantics.

---

### INV-INTERACTION-STATE-5 — Scroll requires capture

A widget MUST NOT consume scroll input unless it is in capture state.

Scroll consumption authority hierarchy:
1. Captured widget has exclusive scroll authority
2. Without capture, scroll routes to captured ancestor (if any)
3. Without any capture, scroll routes via hit-testing to deepest scrollable

A focused-but-not-captured widget does NOT consume scroll.
This prevents accidental scroll hijacking by focused widgets.

---

## 12.3 Focus Acquisition and Scroll Stability — Binding Semantics

This section establishes binding law for when focus acquisition may trigger
scrolling. These invariants prevent the "jump on click" bug where clicking
a visible item causes unexpected scroll repositioning.

---

### INV-FOCUS-NO-SCROLL-IF-VISIBLE

On focus acquisition, auto-reveal MAY run.

Auto-reveal MUST first check:
- Nearest scroll viewport geometry
- Target widget bounding rect

If the widget is **fully visible** within the viewport → **NO SCROLL**.

Auto-reveal is ONLY permitted when the target widget is partially or fully
outside the viewport bounds.

---

### INV-FOCUS-AUTO-REVEAL-SCOPE

Auto-reveal may only scroll the **minimal amount** required to make the
widget fully visible.

FORBIDDEN:
- Align-to-top when bottom edge is cut off
- Align-to-center when top edge is cut off
- Any repositioning beyond what visibility requires

The `ScrollAlign::Nearest` algorithm is the reference implementation:
- If above viewport → align top
- If below viewport → align bottom
- If visible → no scroll

---

### INV-CLICK-NO-FOCUS-REPOSITION

Pointer click that assigns focus MUST NOT reposition scroll.

Click semantics = activation, NOT navigation.

When a user clicks an item:
- The item is already visible (by definition — you cannot click what you cannot see)
- Focus may transfer to the clicked item
- Scroll position MUST remain unchanged

This invariant is enforced by suppressing scroll intent for pointer-originated
focus changes. See `ListAction::Click` and related variants in `list.rs`.

---

### INV-FOCUS-NO-IMPLICIT-CAPTURE

Focus acquisition MUST NOT establish scroll capture.

Scroll capture is reserved for explicit interaction:
- Wheel gesture capture (SPEC-HAL-038 scroll authority)
- Scrollbar drag
- Explicit activation

Focus alone does not grant scroll authority.

---

### INV-FOCUS-ENTRY-SCROLL-NEUTRALITY (Engine-Global)

**Scope:** Engine-global. Applies to ALL widgets, ALL containers, ALL focus sources.

Focus acquisition MUST NOT mutate scroll state unless an explicit reveal is requested.

More precisely:
- When focus transitions from None → Some(widget), scroll state MUST remain unchanged
- Pointer-originated focus entry has zero scroll authority
- Only focus CHANGES (old_focus.is_some()) may trigger reveal
- Focus entry (first focus acquisition) MUST be scroll-neutral

This invariant prevents the "scroll to top on first click" bug where clicking
a visible item in an unfocused list causes scroll reset. The fix is in
`request_focus_for_widget()`: only schedule `pending_focus_reveal` when
`old_focus.is_some()`, not when focus is first established.

Keyboard navigation after focus is established still scrolls as expected
because arrow key navigation creates explicit reveal requests through
scroll intent, which is separate from focus reveal.

**Enforcement:** Debug assertion in `request_focus_for_widget()` verifies this
invariant in debug/test builds. See `src/core/event_router.rs`.

---

## 13. Summary

This SPEC gives Halcyon a **real focus system**:

* single-source-of-truth FocusManager,
* WidgetId-based identity from SPEC-HAL-037,
* scope-aware navigation, modals included,
* predictable keyboard routing and Tab/Shift+Tab behavior.

It’s the foundation for:

* TextField,
* complex hotkey models,
* docked application layouts,
* and sane trading UI keyboard behavior overall.
