# SPEC-HAL-328: Keyboard Authority, Focus, and Capture

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

### *Single Source of Truth for the Entire Focus/Capture Domain*

**Author:** JC
**Status:** Binding
**Supersedes as unified reference:**
SPEC-HAL-038 (Focus Model), SPEC-HAL-089 (Focus Memory),
SPEC-HAL-126 (Tab Order), SPEC-HAL-140 (Focus vs Capture),
SPEC-HAL-176 (Keyboard Authority Modes), SPEC-HAL-184
(ListView Focus), SPEC-HAL-235 (Focus Resolution)

Those specs remain authoritative for their specific domains.
This document is the compiled operational reference — the
single place to check before writing any focus/capture code.

Visual-policy amendment: §12 governs skin-selected focus/capture projection together with adopted SPEC-HAL-012. The framework remains the sole focus/capture and keyboard-mode authority; configuration selects appearance only. This amendment does not change mode transitions, routing, capture eligibility, pointer/scroll capture ownership, Escape, activation, scope, navigation, focus memory or composite member semantics. This amendment is adopted together with the paired SPEC-HAL-012 revision on 2026-10-03. Specification adoption and source conformance are separate.

---

## Application interaction policy — adopted 2026-10-14

JC's 2026-10-14 pointer-driven interaction mandate qualifies navigation and cue clauses
below with one construction-time `UiInteractionPolicy`. `NavigationAndCapture`
is the unchanged default. `PointerDriven` uses the same owners, domain lifecycle,
input chain and immutable appearance publication; it is neither a third authority
mode nor a skin choice. AppConfig and UiThreadConfig propagate the declaration
to every existing domain construction before first input and paint. No runtime
toggle, persistence or subtree policy is introduced. Constitutional ownership,
deferred requests, frame phases and immutable snapshots remain unchanged.

In PointerDriven, outside actual Capture, keyboard input does not reach focused
UI controls or invoke automatic navigation/activation actions. Tab/ShiftTab,
arrows, hjkl, Home/End/Page keys and i/Enter/Space cannot traverse, select,
activate or enter Capture. Direct fallback, widget-first delivery, KeyUp and
paired Text obey the same boundary. Original identity, modifiers and repeat
remain unchanged. Existing explicit Screen/App commands retain their delivery
and priorities; there is no focus-only keyboard consumer. Pointer hover, press,
valid release activation, cancellation, focus and eligible editor Capture remain
unchanged. Explicit owner-requested programmatic focus remains legal.

Capture retains editing/IME, physical responses and Category-K recording.
Unhandled Tab first performs existing interruption/release without changing
committed content. PointerDriven then ends that operation once, retains the
focused owner and omits traversal and auto-capture chaining; the default retains
§3.7's existing chain. Category-K Tab remains reserved and never commits.
Escape is not disabled: capture release, pointer cancellation, scope/modal
unwind and residual original Screen/App delivery retain their once-only priority.
Releasing Capture cannot also close Configuration.

Framework appearance publishes distinct navigation-cue eligibility derived from
policy alongside honest focus and capture facts. PointerDriven suppresses
navigation focus/selection cues, including a noncaptured Button's first admitted
press/release frame, without clearing semantic focus. Actual Capture retains its
truthful visible cue/caret/recording prompt. Value selection, validation, hover
and press remain distinct and visible. INV-FC-2 and navigation-only cue
requirements below apply when navigation cues are eligible; INV-FC-3 still
requires the active Capture cue. Appearance never decides input authority.
Inactive/background domains, rebuilds, focus loss and modal suspension retain
their existing lifecycle; policy restores neither focus nor Capture and creates
no second state stack. No stale key or presentation state survives those owners.

**Scoped adopted execution — 2026-10-26:** the delegated architectural decision in
SPEC-HAL-108 §13.5 admits one engine-owned deferred availability transition.
Construction-time UiInteractionPolicy and static interaction capabilities remain
unchanged; effective disabledness gates delivery through that operation. Enabled
read-only Category-D editors remain explicitly Capture-eligible for selection,
navigation and Copy, while the editing policy denies every mutation. No focus-only
keyboard delivery, parallel authority or lifecycle bypass is admitted.

**Button material contract and implementation admission.**
Contract adoption does not establish source conformance. The existing renderer
and resource owners must verify producer identity, typed request/publication and
commit transport, precise failure causes, generation protection and the actual
Button content/focus/retained integration before execution is claimed.
Unresolved implementation admission remains unresolved and requires governing-owner
resolution, not implementation-side invention. This correction selects no new source, ABI,
target, runtime, material default or consumer behavior and adopts no Proposed
SPEC-HAL-363/364 clauses.
Verification establishes conformance and evidence; it does not admit another
source, ABI, target, runtime, capability or consumer, or lift an unresolved
admission. Such extensions require a separate governing-owner decision.

## 1. The Two States

A widget can be in one of four interaction states:

| State              | Focus | Capture | Description                        |
|--------------------|-------|---------|------------------------------------|
| Unfocused          | no    | no      | Default. No keyboard interaction.  |
| Focused            | yes   | no      | Navigation target. No key authority.|
| Focused + Captured | yes   | yes     | Widget owns the keyboard.          |
| Captured (pointer) | no    | yes     | Transient pointer capture only.    |

No other states exist. (SPEC-HAL-140 §C)

---

## 2. The Two Modes

The keyboard authority system has exactly two modes:

**INV-KBD-MODE-1**: Exactly one mode is active per
interaction domain (per window). Each window independently
tracks its mode state. Window deactivation preserves mode
for restoration on reactivation.

### 2.1 Navigation Mode

No widget owns the keyboard. Every keystroke is
interpreted as navigation intent. No widget state
mutation occurs from any keystroke.

Active bindings:

| Input            | Action                | Effect                          |
|------------------|-----------------------|---------------------------------|
| h / ArrowLeft    | focus_left            | Spatial: nearest left           |
| j / ArrowDown    | focus_down            | Spatial: nearest below          |
| k / ArrowUp      | focus_up              | Spatial: nearest above          |
| l / ArrowRight   | focus_right           | Spatial: nearest right          |
| Tab              | focus_next            | Structural: next in tree order  |
| Shift+Tab        | focus_prev            | Structural: prev in tree order  |
| i / Enter        | enter_widget          | Enter capture or scope, or activate |
| Space            | activate              | Inline activation, stay in Navigation |
| Escape           | exit_scope            | Pop one scope level; no-op at root |

**INV-KBD-MODE-3:** Navigation mode does not grant widgets permission to mutate value, selection, text or widget-owned interaction state through focus-navigation keystrokes. A read-only derived indication of a framework-authorized focus/scope transition under §12 is not such a mutation. This clarification changes no activation, capture-entry or scope-transition behavior explicitly specified elsewhere in this document and does not legalize the known Category S selection workaround in §11.2.

### 2.2 Capture Mode

One widget exclusively owns the keyboard. All keystrokes
route to the captured widget. The widget defines what each
key does (text editing, value changes, popup navigation).

Active bindings:

| Input   | Action           | Effect                    |
|---------|------------------|---------------------------|
| Escape  | release_capture  | Exit capture, keep focus  |
| Tab       | release_and_next | Widget-first; if unhandled: release capture, focus_next, auto-capture if eligible |
| Shift+Tab | release_and_prev | Widget-first; if unhandled: release capture, focus_prev, auto-capture if eligible |
| All others | widget-defined | Routed to captured widget |

**INV-KBD-MODE-10**: hjkl bindings exist ONLY in the
Navigation layer. In Capture mode, h/j/k/l are ordinary
character keys routed to the widget.

### 2.3 Key Handling in Capture Mode

When a key is pressed in Capture mode:

1. **Escape:** unconditional release (§3.2). Always.
2. **Tab / Shift+Tab:** widget-first, then framework (§3.7).
3. **Modifier combo (Ctrl/Alt/Meta + key):**
   a. Dispatch to captured widget first
   b. If widget returns false → binding registry lookup
   at App scope
   c. If binding found → execute action
   d. If no binding → drop silently
4. **All other keys (plain, Shift+key):**
   a. Dispatch to captured widget
   b. If widget returns false → drop silently
   c. No fallback navigation, no action dispatch

The distinction: modifier combos are potential app-level
commands (save, undo, quit). Plain keys are widget content
(typing, value changes). Modifiers fall through to the
app. Plain keys do not.

When the full Action System (020A-D) is implemented, this
distinction becomes unnecessary — the scope stack handles
fallthrough naturally for ALL keys.

---

## 3. Mode Transitions

All transitions are actions. No direct mutation of mode
state. No hardcoded key handlers. No bypass paths.
(SPEC-HAL-176 §4, INV-KBD-MODE-2)

### 3.1 Navigation → Capture

**Trigger:** i / Enter on a `capture_eligible` widget.
**Action:** `enter_widget`

1. Identify focused widget
2. Check `capture_eligible()`:
    - **true** → set Capture mode, call `on_capture_enter`,
      switch binding layer to `capture`
    - **false**, `defines_focus_scope() == true` → push
      scope, focus scope's entry widget, stay Navigation
    - **false**, not scope → execute `activate` instead
      (Button fires click, Checkbox toggles)
6. If none of the above apply (widget is focusable but
   declares no activation, capture, or scope):
    - enter_widget is a no-op
    - Mode stays Navigation
    - No event fires

**Pointer-initiated:** Mouse click on a capturable widget
immediately enters Capture. The transition goes through
the action system. (SPEC-HAL-176 §4.1)

**i / Enter equivalence (formal rule):** In Navigation
mode, `i` and `Enter` both map to `enter_widget`. The
framework makes no distinction between them. Any
divergent behavior is a bug. In Capture mode, `i` types
the letter (Category D) or is widget-defined; `Enter` is
widget-defined per category (§11).

**No phantom input on capture entry:** When `i` triggers
`enter_widget`, `dispatch_engine_action` returns true and
`route_input_event` is skipped. The captured widget
receives `on_capture_enter` but does NOT receive the
triggering `i` character as a Text event. The first
keystroke routed to the widget in Capture is the NEXT
key the user presses.

**No re-entry from Capture:** `enter_widget` is bound in
the Navigation binding layer only. The Capture layer does
NOT contain `enter_widget`. In Capture, pressing `i` or
`Enter` is an ordinary key handed to the widget — there
is no "double-enter" of Capture.

**Focus stability during capture entry.** If focus moves
away from the widget during on_capture_enter (via
redirect_focus, rebuild, or programmatic change), the
framework detects this after the callback returns.
Capture is released immediately — a widget cannot hold
capture without focus (§3.9 invariant). on_capture_leave
is called on the original widget.

### 3.2 Capture → Navigation

**Trigger:** Escape
**Action:** `release_capture`

Semantics are absolute and non-negotiable:

- Release keyboard capture
- Return to Navigation mode
- Focus REMAINS on the same widget
- **No rollback**
- **No cancellation**
- **No state reset**
- **No clearing of input**
- **No widget-specific Escape behavior**

Escape means exactly one thing: release keyboard authority.
The widget keeps its current state. Undo is Ctrl+Z, not
Escape. (SPEC-HAL-176 §4.2, INV-KBD-MODE-4)

The framework releases capture UNCONDITIONALLY. No widget
consultation. No `offer_capture_escape`. No veto.

Side effects of leaving capture (popup closing, caret
hiding) happen in `on_capture_leave` — a notification,
not a request.

### 3.3 Inline Activation

**Trigger:** Space in Navigation mode, or i/Enter on a
non-capturable non-scope widget.
**Action:** `activate`

- Fire the widget's primary action
- Do NOT change authority mode
- Remain in Navigation mode

Examples: Button press, Checkbox toggle, Switch toggle.

### 3.4 Transition Diagram

```
Navigation Mode
  │
  ├─ i/Enter on capturable ──→ Capture Mode
  │                              │
  │                              └─ Escape ──→ Navigation
  │                                  (focus unchanged, state preserved)
  │
  ├─ i/Enter on scope container ──→ push scope (stay Navigation)
  │
  ├─ i/Enter on non-capturable ──→ activate (stay Navigation)
  │
  ├─ Space ──→ activate (stay Navigation)
  │
  ├─ hjkl/arrows ──→ spatial focus move
  │
  ├─ Tab/Shift-Tab ──→ structural focus move
  │
  └─ Escape ──→ pop scope (or no-op at root)

Mouse click on capturable ──→ Capture Mode (direct)
```

### 3.5 Pointer Click During Capture

If the user clicks a different widget while in Capture mode:

1. Capture is released for the current widget
   (on_capture_leave fires)
2. Focus moves to the clicked widget (on_blur/on_focus fire)
3. If the clicked widget is capture_eligible, Capture is
   immediately entered on the new widget
   (on_capture_enter fires)
4. If the clicked widget is not capture_eligible, mode
   returns to Navigation

This is the standard behavior: click always wins. The
transition goes through the action system
(INV-KBD-MODE-6).

If the clicked widget is not capture_eligible:
- Capture is released (on_capture_leave)
- Focus moves to clicked widget (on_blur/on_focus)
- Mode returns to Navigation
- The click is a NORMAL pointer interaction — if the
  widget has click behavior (Button fires on_click,
  Checkbox toggles), that behavior fires. The widget
  receives the pointer event normally.

Single click = release capture + focus + interact.
The user does not need to click twice.

### 3.6 Captured Widget Destruction

If the captured widget is destroyed while in Capture mode:

1. Capture is released automatically
2. keyboard_mode returns to Navigation
3. on_capture_leave is NOT called (widget no longer exists)
4. Focus follows SPEC-HAL-126 INV-F12: relocate to nearest
   eligible neighbor or None

### 3.7 Tab During Capture (Auto-Capture Chain)

When Tab is pressed in Capture mode under NavigationAndCapture:

1. Offer Tab to the captured widget. If the widget
   handles it (returns true), Tab is consumed. Done.
2. If the widget does not handle Tab:
   a. Release capture (on_capture_leave fires)
   b. Execute focus_next (structural navigation)
   c. If the newly focused widget has
   `capture_eligible() == true`, immediately enter
   Capture on it (on_capture_enter fires)
   d. If not capture_eligible, mode returns to Navigation

This enables seamless form Tab-through: the user edits
TextField A, presses Tab, and TextField B is immediately
ready for editing — no `i` needed on each field.

Shift+Tab follows the same pattern with focus_prev.

Under PointerDriven, steps 1 and 2a still apply; the operation then ends without
moving focus or entering Capture on a neighbor.

The auto-capture chain ONLY fires when Tabbing FROM a
captured widget. Tab in Navigation mode never auto-captures
(INV-CAPTURE-EXPLICIT-ONLY still holds for Navigation).

**Target resolution and chain breaks:**

- Auto-capture targets whatever focus_next resolves to
  (standard structural traversal, non-focusable skipped)
- If focus_next reaches a non-capturable widget, mode
  returns to Navigation. Subsequent Tabs are normal.
  Auto-capture is an optimization for consecutive
  capturable widgets, not a guarantee.
- If focus_next resolves to None (end of chain), capture
  releases, no new focus acquired.

### 3.8 Widget-Initiated Capture Release

Two release paths exist:

1. **User-initiated** via Escape: unconditional, no
   rollback (§3.2). The framework releases capture
   without consulting the widget.
2. **Widget-initiated** via `cx.release_keyboard_capture()`:
   voluntary completion of an editing transaction.
   Examples: ComboBox commits the selected item on Enter,
   SpinBox commits the parsed value on Enter, single-line
   TextField confirms its content.

Both paths go through the framework's release machinery
(on_capture_leave fires, keyboard_mode returns to
Navigation, binding layer flips back). Neither is a
bypass — the widget cannot "secretly" stay captured after
calling release.

### 3.9 Programmatic Focus Change During Capture

**Invariant:** Capture requires focus. A widget cannot
remain captured without focus.

If focus moves away by any means while a widget is
captured — programmatic `request_focus_for_widget`,
validation error redirect, screen transition,
`clear_focus` — capture is released first
(on_capture_leave fires) and mode returns to Navigation
before the focus move completes.

Auto-capture on focus change happens only via Tab (§3.7)
or pointer click (§3.5). Programmatic focus moves do NOT
auto-capture the new target.

### 3.10 Overlay Interaction with Capture

When a transient overlay (context menu, tooltip surface
with input, popup with its own scope) opens while a widget
is captured, capture is **suspended**, not released:

1. The overlay's scope takes precedence
2. Capture state is preserved (caret position, edit text,
   selection, scroll)
3. When the overlay dismisses, if the suspended widget
   still exists and is still focused, capture **resumes**
   (on_capture_enter is NOT re-fired — capture is
   restored, not re-entered)
4. If the suspended widget was destroyed or lost focus
   while the overlay was active, capture is released
   normally (§3.6 / §3.9)

This preserves the editing context across transient
overlays so the user does not lose work to incidental UI.

Note: capture state suspension/restoration is framework-
internal bookkeeping. Widgets MUST NOT mirror this state
in their own fields (see §12.8, INV-FC-NO-PARALLEL-STATE).

---

## 4. Focus: What It Is and What It Isn't

**Focus is a pointer, not a weapon.** (SPEC-HAL-140 AUTH-140-1)

Focus identifies the navigation target. It grants
NO authority over any input stream.

**INV-FOCUS-NO-AUTHORITY**: Focus MUST NOT grant scroll,
wheel, drag, or keyboard authority.

**INV-FOCUS-STABILITY**: Focus changes MUST NOT mutate
scroll, layout, or value state.

**INV-FOCUS-VISIBILITY-ONLY**: Focus reveal may ensure
visibility (scroll-to-reveal). It may not reposition
unnecessarily.

**INV-F0** (SPEC-HAL-126): Focus is a singleton. Zero or
one widget has focus. Never more, never fractional, never
inferred.

---

## 5. Capture: What It Is

Capture is exclusive authority over one or more input
streams. (SPEC-HAL-140 §E)

**INV-CAPTURE-EXPLICIT-ONLY**: Capture entered only via
explicit activation (click, Enter, i). Never on focus
entry. Never on hover. Never implicitly.

**INV-CAPTURE-EXCLUSIVE**: While captured, input MUST NOT
leak to ancestors or siblings.

**FORBID-KBD-IMPLICIT-CAPTURE**: No widget enters Capture
by receiving focus. Focus alone grants no keyboard
authority. (SPEC-HAL-176 §11)

**Hover is independent of capture.** Pointer hover
operates normally during keyboard capture: a Button
hovered while a TextField holds keyboard capture still
shows its hover affordance. Hover state and capture state
are orthogonal — neither suppresses the other.

**Keyboard capture is independent of pointer and scroll
capture.** They are separate authority domains
(SPEC-HAL-140 INV-CAPTURE-DOMAINED). A widget may hold
keyboard capture while another widget holds pointer
capture (e.g., during a drag). Scroll capture is likewise
independent.

---

## 6. Escape Semantics

Escape is the structural unwind operator. It is NOT a
semantic action (cancel, revert, clear, close).

### In Capture Mode
Release capture. Return to Navigation. Focus stays. State
stays. No widget-specific behavior. Period.
(SPEC-HAL-176 §4.2, INV-KBD-MODE-4)

### In Navigation Mode, Inside a Scope
Pop one focus scope level. Focus returns to the scope's
parent. (SPEC-HAL-176 §5.2, INV-KBD-MODE-5)

### In Navigation Mode, Root Scope
No-op. Nothing to pop. Focus stays where it is.
(SPEC-HAL-176 §5.2)

### Escape NEVER:
- Defocuses (removes focus entirely)
- Writes focus memory
- Cancels edits
- Reverts values
- Closes popups directly (popup closes as side effect
  of on_capture_leave, not as Escape behavior)
- Is consumed by widgets to prevent mode transition

**FORBID-KBD-ESCAPE-SEMANTICS**: No widget may override
Escape to perform state changes. Escape is structurally
reserved for authority/scope transitions.
(SPEC-HAL-176 §11)

---

## 7. Focus Memory

Focus memory exists ONLY for window activation/deactivation.
(SPEC-HAL-089)

### Writes
- Window deactivation: remember focused widget, blur it

### Restores
- Window activation: restore if no widget focused, no
  active scope, widget still exists and focusable

### Never
- ESC does NOT write focus memory (SPEC-HAL-089 §5.1 rule 3)
- ESC does NOT read focus memory
- Navigation does NOT use focus memory
- There is no "park and resume" mechanism

---

## 8. Navigation: Spatial and Structural

Two independent navigation systems coexist.
(SPEC-HAL-176 §7.3)

### 8.1 Spatial Navigation (hjkl / arrows)

Consults post-layout paint rects. Operates within the
current focus scope only.

**Algorithm** (SPEC-HAL-176 §7.1):

1. Get paint rect of focused widget
2. Enumerate focusable widgets in current scope
3. Filter by direction using EDGES (not centers):
    - focus_left: candidates whose **right edge ≤** current left edge
    - focus_right: candidates whose **left edge ≥** current right edge
    - focus_up: candidates whose **bottom edge ≤** current top edge
    - focus_down: candidates whose **top edge ≥** current bottom edge
4. Select nearest by distance (primary axis projected,
   secondary as tiebreaker)
5. No candidate → no-op (no wrap)

**INV-SPATIAL-2**: Operates within current scope only.
Does not cross scope boundaries.

**INV-SPATIAL-3**: No wrap. Leftmost widget + Left = no-op.

### 8.2 Structural Navigation (Tab / Shift-Tab)

Follows widget tree order, not layout. (SPEC-HAL-126)

- `tab_index` controls order (0 = default, negative = skip)
- Layout geometry is irrelevant (INV-F3)
- Wrapping forbidden by default (INV-F7)
- Inside a focus scope: Tab is trapped (INV-F8)

### 8.3 Tree-Structured Navigation (SPEC-HAL-326)

When containers declare `layout_axis()`, directional
navigation follows the tree structure instead of pure
spatial distance. This provides deterministic, explainable
navigation within well-structured layouts.

- Vertical container + Down → next sibling
- Horizontal container + Right → next sibling
- Cross-axis → exit container to parent level
- Sticky cursor: cross-axis entry picks the child nearest
  to the source widget's x-position

Tree navigation is preferred over spatial when available.
Spatial is the fallback when no container declares an axis.

**INV-TREE-NAV-SCOPE:** Tree-structured navigation
operates within the current focus scope only, matching
INV-SPATIAL-2. It MUST NOT cross scope boundaries. When
tree navigation reaches the scope boundary (no more
siblings in the scope), it returns no-op — same as
spatial at the edge.

### 8.4 Navigation Resolution Priority

1. Tree-structured navigation (§8.3) runs first when any
   ancestor of the focused widget declares `layout_axis()`.
2. If tree navigation returns no target, spatial navigation
   (§8.1) is the fallback — but ONLY for Up/Down. Left/Right
   never falls back to spatial (prevents chaotic cross-layout
   jumps).
3. If no ancestor declares `layout_axis()`, spatial
   navigation is the sole resolver.
4. Structural navigation (Tab, §8.2) is always independent
   and runs on its own bindings.

### 8.5 Initial Focus Acquisition

If no widget is focused and a navigation key is pressed:

- **Tab / Shift+Tab:** Focus the first / last focusable
  widget in tree order (SPEC-126 INV-F5/F6).
- **hjkl / arrows:** Focus the first focusable widget in
  the current scope. The spatial resolver has no origin
  point without a focused widget, so the first widget in
  tree order is used as the entry point.
- **i / Enter / Space:** No-op (nothing to activate or
  enter).

This applies to app startup and after all focus is
cleared (e.g., clicking empty space).

---

## 9. Focus Scopes

Focus scopes are bounded regions where Tab is trapped and
arrows navigate internally. (SPEC-HAL-126 §6, SPEC-HAL-176 §5)

### Scope Stack
Entering a compound widget pushes a scope. Escape pops it.
Scopes are a stack, not a flat set.

### Entry
When i/Enter targets a scope container
(`defines_focus_scope() == true`):
- Push the scope
- Focus moves to scope's entry widget (INV-F9)
- Authority stays Navigation (children are navigable)

### Exit
Escape pops the scope. Focus returns to the parent scope's
remembered position. (SPEC-HAL-089 §9)

### Tab Containment
Tab/Shift-Tab is trapped inside the scope (INV-F8).
Tab alone cannot exit a scope — only Escape can (INV-F10).

### Mutual Exclusion
A widget may be `capture_eligible` OR `defines_focus_scope`.
Never both. (SPEC-HAL-176 §6.3)

---

## 10. Widget Declarations

Every widget declares its interaction contract through the existing trait methods:

- focusable() — default false;
- capture_eligible() — default false;
- defines_focus_scope() — default false; and
- accepts_keyboard_activation() — default false.

These remain static interaction declarations under INV-KBD-MODE-7. Appearance, hover, animation progress and skin selection MUST NOT change them.

The component's already-authorized internal target and committed geometry are inputs to its visual projection under §12. They are not additional interaction declarations. The legacy focus_indicator_geometry hook is covered by §12.9's compatibility disposition; it is not a new normative requirement.

### 10.1 Activation vs Capture Entry

`on_keyboard_activate()` fires when i/Enter targets a
non-capturable widget (Category A, or Category S outside
scope). It performs the widget's primary action (click,
toggle, tab switch).

`on_capture_enter()` fires when i/Enter targets a
capture-eligible widget (Categories C, D, E). It
transitions the widget into its editing/interactive state.

A widget receives ONE of these, never both. The framework
decides based on `capture_eligible()` and
`defines_focus_scope()` (§3.1).

---

## 11. Widget Categories

### 11.1 Category A: Activation-Only

Activate on i/Enter/Space. No capture. No scope.

**Widgets:** Button, Checkbox, Switch

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | false |
| `defines_focus_scope()`         | false |
| `accepts_keyboard_activation()` | true  |

Navigation: arrows → framework navigates. i/Enter/Space →
activate. Escape → pop scope or no-op.

### 11.2 Category S: Scope Containers

Compound widgets with internal navigable items. i enters
the scope. Arrows navigate within. Escape exits.

**Widgets:** SegmentedControl, RadioGroup, TreeView, ListView, BreadcrumbBar

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | false |
| `defines_focus_scope()`         | true  |
| `accepts_keyboard_activation()` | true  |

Outside scope: arrows → framework navigates. i → enter
scope. Inside scope: arrows → navigate within widget.
i/Enter → activate item. Escape → exit scope.

For RadioGroup, arrows and other focus navigation MUST change only the
internal focus/highlight, never selection. Explicit framework-mapped
`i` / `Enter` / `Space` activation commits the currently highlighted radio
through `on_keyboard_activate`. This also applies to the temporary behavior
below; Space remains the framework's inline activation action.

**Temporary behavior (until scope stack Phase 3):**

Until `defines_focus_scope` is wired into the framework's
ENTER_WIDGET handler, Category S widgets operate as follows:

- `defines_focus_scope()` returns true (declaration ready)
- `capture_eligible()` returns false
- `accepts_keyboard_activation()` returns true
- When focused, arrows are handled by the widget directly
  in `handle_event` WITHOUT checking keyboard_mode
  (known violation of INV-KBD-MODE-3)
- i/Enter triggers `on_keyboard_activate` (activate the
  currently highlighted item)
- Escape returns false (framework handles scope pop or
  no-op)
- At container boundaries (first/last item), arrow handler
  returns false so the framework navigates to the next
  widget

This is explicitly a transitional workaround. When scope
stack lands, these widgets stop handling arrows in
handle_event entirely — the scope mechanism handles it.

### 11.3 Category C: Value-Editing

Arrows change values. Only in Capture mode.

**Widgets:** Slider, RangeSlider, SpinBox, TimePicker

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | true  |
| `defines_focus_scope()`         | false |
| `accepts_keyboard_activation()` | false |

Navigation: arrows → framework navigates (return false).
i/Enter → enter Capture. Capture: arrows → change value.
Escape → release Capture.

**handle_event MUST** check `cx.keyboard_mode == Capture`
before handling ANY key for value changes.

**hjkl in Capture:** In Capture, hjkl are received as
ordinary Text events. Widgets MAY translate them internally
(e.g., h/l as decrement/increment for Slider) or ignore them.
Arrow keys are the primary value-change mechanism.

**SpinBox special case:** SpinBox is Category C with
embedded text editing. In Capture mode:
- Arrow Up/Down: increment/decrement value
- Typed digits: replace value text in embedded field
- Enter: commit the edited value (widget-internal)
- hjkl: received as text — 'h'/'l' are ignored or
  treated as invalid input, 'j'/'k' are ignored
- Backspace/Delete: edit text in embedded field

The tree-owned TextField editing child is NOT independently focusable or capturable. SpinBox remains the actual Focus/Capture owner. Only the engine may delegate text entry to its current qualified child/session under HAL-101 section 2.4; it supplies the child's real geometry/context and existing text-engine capability. SpinBox owns numeric/mode commands and shall not own, borrow or manually manage a TextField Widget. This structural reconciliation does not change the currently admitted numeric-versus-direct-text command policy.

**Enter in Capture (per-widget):**
- Slider: no-op
- SpinBox: commit parsed value, release capture
- TimePicker: commit, release capture

SpinBox numeric Up/Down stepping remains its non-direct-edit operation; an active direct text-edit session retains the existing TextField key handling. Slider's explicit value-editor Enter commits its current draft without introducing a Capture-release request; scalar Capture Enter remains otherwise governed by its existing policy.

### 11.4 Category D: Text-Input

All keyboard input requires Capture mode.

**Widgets:** TextField, TextArea, TagInput

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | true  |
| `defines_focus_scope()`         | false |
| `accepts_keyboard_activation()` | false |

Navigation: ALL keys → return false (framework navigates).
i/Enter → enter Capture. Capture: type text, move cursor.
Escape → release Capture.

**handle_event MUST** check `cx.keyboard_mode == Capture`
before handling ANY keyboard input.

**Enter in Capture (per-widget):**
- TextField (single-line): commit content, release capture
- TextArea: insert newline, stay captured
- TagInput: commit current tag, stay captured

TagInput's HAL-101 section 2.4 text-entry child does not become a second Focus/Capture owner. TagInput retains its existing Category-D capture, tag/popup policy and command interception. Engine-mediated delegation executes text operations and settles an owned current result before dependent host actions in the same routed batch. The ordinary standalone TextField's Enter/cancel lifecycle cannot replace TagInput's existing semantics. Engine Escape ownership and existing capture-release rules remain unchanged.

### 11.5 Category E: Popup

Popup opens on capture entry. Closes on capture leave.

**Widgets:** ComboBox, DatePickerField, ColorPickerField

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | true  |
| `defines_focus_scope()`         | false |
| `accepts_keyboard_activation()` | false |

Navigation: ALL keys → return false. i/Enter → enter
Capture, `on_capture_enter` opens popup. Capture: arrows
navigate popup items, text types in field. Escape →
release Capture, `on_capture_leave` closes popup.

Popup opening/closing is a SIDE EFFECT of capture
enter/leave — not an Escape or Enter override.

**hjkl in Capture:** In Capture, hjkl are received as Text
events. When the popup option list has focus (browsing
mode), the widget SHOULD translate j/k to Down/Up internally
for option navigation. When the text field has focus
(typing mode), hjkl type letters. This is a widget-internal
decision based on focus_zone, not a framework mechanism.

**Enter in Capture (per-widget):**
- ComboBox: commit highlighted option, close popup, release capture
- DatePickerField: commit selected date, close popup, release capture
- ColorPickerField: commit selected color, close popup, release capture

ComboBox's HAL-101 section 2.4 text-entry child does not become a second Focus/Capture owner. ComboBox retains its existing Category-E capture, popup/resolver policy and command interception. Engine-mediated delegation executes text operations and settles an owned current result before dependent host actions in the same routed batch. The ordinary standalone TextField's Enter/cancel lifecycle cannot replace ComboBox's existing semantics. Engine Escape ownership and existing capture-release rules remain unchanged.

### 11.6 Category F: Display-Only

Not focusable. Not in focus chain.

**Widgets:** Label, GroupBox, ProgressBar, Separator,
BusyIndicator, FlowGrid, Flex, ScrollArea, StatusLabel

### 11.K Category K: Single-Stroke Capture

Capture mode is entered for the purpose of binding one keystroke. The first non-modifier Key Down captures the combination as a value and releases capture programmatically.

**Widgets:** KeybindEditor (SPEC-HAL-329)

| Declaration                     | Value |
|---------------------------------|-------|
| `capture_eligible()`            | true  |
| `defines_focus_scope()`         | false |
| `accepts_keyboard_activation()` | false |

Navigation: ALL keys → return false (framework navigates). `i` / Enter → enter Capture. Capture: modifier Key Down/Up → tracked internally; non-modifier Key Down → build value, fire callback, release capture programmatically. Escape → framework releases unconditionally (§3.2); value unchanged. Tab / Shift+Tab → return false; framework handles (§3.7); value unchanged.

**handle_event MUST** check `cx.keyboard_mode == Capture` before treating any key as a binding intent.

**Escape and Tab are not capturable as bindings via Category K widgets.** Both are framework-reserved per §3.2 and §3.7. A future SPEC could introduce a "raw capture" sub-mode to lift this restriction; until then, this is a documented limitation of Category K.

---

## 12. Visual Indication of Focus and Capture

Focus and capture are framework-owned semantic facts. A selected skin controls their visual projection under SPEC-HAL-012 and this section. The framework supplies ownership and lifecycle truth; the component supplies its existing target and geometry; the selected appearance supplies finite admitted values and composition.

The permission includes modulation of resting primitives and additional decorative primitives. It does not introduce another semantic owner, layout authority, interaction target, animation system or renderer feature.

### 12.1 Authoritative facts and domains

Projection MUST consume the current framework-owned focused widget and the relevant domain-qualified capture/mode facts. A component may also supply its already-authorized internal focus target. The skin cannot select either owner or target.

Keyboard capture is distinct from pointer capture, scroll capture, hover, press, selection and activation. A global keyboard mode without matching owner/domain is insufficient to claim that this widget holds capture. PaintContext::is_focused() and has_capture() are current keyboard-facing queries; has_capture() is not a pointer/scroll capture query.

Where an interaction domain suspends capture, appearance MUST distinguish suspended/inactive state from current active capture in the active domain. A remembered owner or internal target is not active ownership. This requirement consumes framework truth; it adds no suspension, restoration or routing policy.

Widget-local semantic focus/capture mirrors are forbidden. Owner-bound presentation values or Timeline handles are permitted only as derived visual state, stored and updated separately from interaction state under SPEC-HAL-142/142A. They MUST NOT become authority for routing, eligibility, member selection or capture lifetime.

Existing local state for a genuinely different purpose, such as hover interpolation or caret presentation, may remain under its proper owner. Its name or visual effect does not make it a substitute for framework focus/capture truth.

HAL-101 section 2.4 admits a separate engine-owned editing-delegation fact for one real tree-owned TextField child of ComboBox, SpinBox, Slider or TagInput. It qualifies the exact host Capture/domain, both concrete mounts and current editing session. It does not alter actual FrameworkAppearanceFacts, focus, mode or capture ownership. Child caret/blink/motion may consume only that qualified fact together with child-owned geometry and availability; the host supplies the one complete outer focus/Capture cue. Inactive, missing, stale or retired delegation cannot be treated as child Capture or repaired by a local boolean.

### 12.2 One coordinated, deterministic composition

For one current semantic target in a domain, the component MUST use one declared complete focus/capture projection. A shared helper may emit that projection. The framework MUST NOT independently append another default indicator over it.

The composition may contain multiple primitives or coordinated part treatments. A semantic singleton does not mean exactly one GPU primitive, and it is not a cross-domain global singleton beyond the existing focus/capture rules.

The same authoritative fact snapshot, configuration/address/qualifiers, owner target geometry and frame-stable animation sample MUST produce the same values, composition and required order. Paint and resolution MUST remain read-only with respect to semantic state, widget presentation state and Timeline.

A skin MAY change existing colors, border widths, supported materials, radii or other admitted decorative parameters, and MAY add a finite admitted halo, glow, ring, Image treatment or decorative overlay. Primitive count/type, presence at rest and relative Z need not remain identical to the resting appearance.

The result MUST be a finite bounded composition of supported values consumed by existing paint paths. It is not an executable effect program, free-form SceneGraph, recursive graph or new renderer/region runtime.

**363/364 scoped amendment:** SPEC-HAL-363/364 SHALL consume the current authoritative fact snapshot
for current/incoming presentation. Historical paint SHALL never acquire current
focus/capture authority or compensate for a stale cue by appending a second
indicator. Every required current cue remains part of the one complete current
projection.

### 12.3 Geometry, clipping and Z

Every decorative primitive MUST derive from geometry supplied by the component/layout owner and from admitted decorative parameters. It MUST NOT produce another canonical widget rect or move the semantic target.

Decorative coverage may exceed the allocated rect where the existing composition owner admits it. It MUST NOT change measurement, padding, typography metrics, layout participation, child placement, hit regions, navigation candidate bounds, scroll extents, reveal targets or accessibility geometry.

No decoration may intercept input, create a tooltip trigger, create an interactive overlay, acquire focus/capture, change an interaction domain or influence routing. “Overlay” here names paint composition only.

Clip, scroll, surface and main/overlay scopes remain owned by the existing traversal/composition system. Appearance MUST NOT pop an ancestor clip, change OverflowMode, escape a viewport, route to another surface or choose a global maximum Z.

Required local order MUST lower to admitted explicit Z under SPEC-HAL-359. Equal-Z cross-pipeline order and incidental call order are not semantic ordering tools. Existing owner-admitted placement before/after component content or child composition is required; the skin cannot invent another traversal phase.

Retained replacement and any existing culling/damage calculation MUST include actual supported decorative coverage and its removal. This creates no OS partial-damage or public bounds subsystem.

### 12.4 Visibility, enclosure and transition truth

**INV-FC-2:** When navigation cues are eligible, the current Navigation-mode focused widget MUST have a visible focus indication wherever its accepted contract requires indication. A composite's current internal target MUST be identifiable while the composite holds framework focus, subject to 208 INV-D3's 2026-10-24 default-selector exception for Checkbox, RadioButton/RadioGroup options and Switch. That exception requires no substitute navigation-focus ornament and does not equate selection with focus or change the actual internal target. PointerDriven suppresses navigation cues without falsifying focus.

A Button may opt into `FocusCueModality::KeyboardOnly`. Its navigation cue also
requires accepted keyboard focus navigation or activation for the same current
mount in the active focus domain. An ordinary pointer focus assignment clears
that qualification even when the owner stays focused. Programmatic focus
acquisition supplies no keyboard qualification. Focus or domain retirement and
mount replacement end the qualification. This per-Button projection leaves
application policy, semantic focus, activation and required Capture indications intact.

**INV-FC-3:** In Capture mode, the current keyboard-capture owner MUST have a visible indication distinguishable from that same owner's focus-only state. A skin may choose admitted color, width, material, intensity or decoration differences; the distinction is no longer restricted to one color and width modulation.

An indication must remain effective on the actual content/background of the supported skin and through simultaneous error, hover, pressed or selected states. The only current-owner cue MUST NOT begin at invisible alpha, be deferred until an animation completes, or disappear at a pulse's minimum.

The current cue MUST follow authoritative state at the next admitted paint after a transition. Visual animation cannot delay semantic ownership, advance ownership before the framework does, or show an outgoing owner as still current.

A finite outgoing decorative tail is allowed only when all of these conditions hold:

1. From the first frame after owner/target loss, the outgoing composition no longer contains a cue that the selected skin uses to identify current focus or capture. Fading that same ownership cue at reduced alpha is not sufficient.
2. The active target's required cue remains immediately identifiable and unambiguous.
3. The outgoing treatment is explicitly declared as non-ownership decoration, stays attached to the same still-live owner/geometry, and uses a bounded Once effect under §12.6.
4. If the distinction cannot be established for the intended skin/state, the outgoing treatment is removed immediately.
5. Unmount, target invalidation, loss of its admitted scope or incompatible skin replacement cancels the tail and removes its fragments. Reacquisition retargets or cancels through the normal owner path; it cannot leave an old tail competing with the new cue.

This permission does not require outgoing tails and does not authorize infinite tails.

**INV-FC-ENCLOSURE:** Except for the default selectors under SPEC-HAL-208 INV-D3's 2026-10-24 ruling, a labeled control governed by SPEC-HAL-208 MUST have focus treatment covering its indicator, label and intervening gap as one control; an indicator-only ring is insufficient where enclosure is required. The component may supply its unified target rect or an equivalent coordinated complete treatment; it MUST NOT split semantic ownership or introduce label widgets. The scoped exception does not relax Capture or other controls' requirements. Indicator-before-label content ordering and existing typography restrictions remain binding.

Required indication remains subject to accepted clipping and visibility eligibility. Enclosure is defined against the full logical labeled-control target and clipped by the existing owner; it is not permission to escape a viewport, reveal a hidden target or change scroll behavior.

**363/364 scoped amendment:** Historical capture requires producer-certified cue safety before
pixels/primitives are flattened. Unknown cue provenance is refusal, not
permission to infer from color, erase matching pixels, fabricate a neutral
framework snapshot or assume opacity makes an ownership cue harmless.
A declared historical-safe paint purpose may produce non-ownership values
without falsifying facts; it applies only to the separate historical source.

The neutral outgoing-tail permission remains limited to the same still-live
owner/geometry and bounded Once effect. It does not authorize retaining that
tail after unmount or turning a current ownership cue into a movable ghost.
SPEC-HAL-363/364 historical admission has its own stricter certificate.

Presentation transforms, masks, opacity and order SHALL NOT delay, duplicate,
ambiguate or conceal a required current cue within the target's accepted visible
scope. Current indication, capture distinction and enclosure are tested against
the consuming control's contract. A recipe that cannot preserve them SHALL
settle/refuse as a complete group; it cannot append a compensating ring or alter
input authority.

### 12.5 Default border adapter and component examples

Theme::control_border(focused, captured, error) remains a default bordered-control adapter. The existing default recipe is:

| Branch | Color | Width |
|---|---|---|
| error | colors.semantic.error | 1.0 |
| captured | colors.borders.focus | focus_ring.captured_width |
| focused | colors.borders.focus | focus_ring.focused_width |
| neither | colors.borders.default | 1.0 |

That recipe is a default visual choice, not the universal appearance authority. A selected skin may supply different admitted border or complete decorative values. The same fact discipline and visibility guarantees bind every recipe.

Because the error branch masks focus/capture in this helper, an error-state control using it as its only cue does not satisfy §12.4. The complete default composition MUST preserve a separate truthful distinction or use an otherwise complete conforming resolved recipe. This amendment does not silently repair existing code.

The surviving focus-ring fields may continue as default adapter values. This amendment neither requires the removed focus_ring.capture_color field to be restored nor prohibits distinct capture colors in an admitted skin. API compatibility and appearance permission are separate.

Examples, not new widget policies:

- Text fields may modulate their existing border or use another admitted complete treatment; text/editing/capture behavior stays unchanged.
- Buttons may modulate their resting surface or add admitted decoration without acquiring keyboard capture.
- Default checkbox/radio/switch treatment follows 208 INV-D3's 2026-10-24 no-additional-enclosure ruling; value indicators and existing indicator modulation remain, with no substitute navigation ornament required. Their semantic contract and other controls' focus/capture requirements remain unchanged.
- Slider/range and other genuinely interactive controls retain their existing input contract. A radial gauge or other display-only widget does not become focusable because it can consume appearance.
- A composite may project its own focused member/row/tab through the selected skin while the framework focus owner remains the composite.
- Popup hover/highlight and list/tree state remain distinct from framework focus and capture; a skin cannot reinterpret them as ownership.
- Display-only content gets no focus/capture behavior from appearance configuration.

The default may remain restrained and intrinsic. Such a default is not a permanent primitive-count, Z, color or no-glow ceiling for other skins.

### 12.6 Timing, lifecycle and invalidation

SPEC-HAL-021 supplies the only generic interpolation/playback/handle mechanism, SPEC-HAL-042 supplies frame order and engine time, and the existing engine scheduler admits frames. This section creates no focus clock, private animator, timer or independent frame loop.

Skin configuration may provide admitted visual targets and transition parameters. Event/update ownership transitions may start, cancel or replace owner-bound Timeline handles through existing capabilities. Update publishes derived immutable presentation values where needed. Paint may read a frame-stable finite sample where 021 permits it, but MUST NOT mutate cached values, clear handles or start/cancel/advance playback.

Frame-stable sampling, exact completion, cancellation, missing/type-mismatch distinction and replacement follow 021. An optional animation capability being unavailable MUST select a declared conforming static treatment; it cannot delay the required cue or invent a local timer.

Current-owner effects may use explicitly admitted Once, Loop or Alternate playback. Repetition is explicit and cannot make the required current cue disappear. Outgoing tails use bounded Once only. No semantic state transition is driven by an interpolated value.

Ownership/mode, suspension/domain, internal target, configuration, geometry and sample changes MUST invalidate the affected appearance and necessary retained composition. A pure focus/capture visual change does not invalidate layout. An independent skin metric change still requires its correct layout/text invalidation under 012/354.

Retarget or replacement MUST cancel the superseded handle and bind the new one to the current engine frame under 021. Owner loss immediately removes the current-ownership cue; any allowed neutral tail follows §12.4. Unmount, owner replacement, invalid target, retirement or end of effect cancels/releases remaining owned handles. No stale primitive or anonymous frame demand may survive.

Registered resources used by retained or transitioning decoration remain subject to their existing resource lifetime owner. Appearance cannot release or reconstruct them itself.

Settled/cancelled effects report no continuing animation demand. Active admitted effects report demand through the existing owner; they do not wake the platform directly. Immediate repaint obligations remain in §15.

**363/364 scoped amendment:** Relevant focus/mode/domain/capture/configuration changes invalidate
current projections and every capture/admission certificate that depends on
them, even when callbacks request no repaint. The owning Update settles or
replaces active presentation control; immutable same-frame admission rejects
stale cue-bearing history before publication. Authoritative teardown and
deferred-focus semantics remain with their current owners.

**Scoped optional Button SDF requirements — 2026-10-25:**

Optional SDF material status under 012/266A changes no focus/capture owner,
eligibility, domain, lifecycle or activation rule. Material unavailability is
neither semantic disabledness nor optional absence. Required current FocusCue
remains independent, immediate, canonically positioned and correctly clipped/
ordered even when the material Image is unavailable; no extra cue is appended.

### 12.7 Rendering capability boundary

The permission to author richer focus/capture appearance creates no rendering capability. Every requested effect MUST have an admitted primitive/material, clip behavior, ordering, color/alpha treatment and resource binding for the actual path.

Unsupported group opacity, masks, backdrop blur, generic transforms or rounded-ancestor clipping MUST NOT be silently ignored or simulated by an undeclared substitute. Resolution must produce the declared complete fallback or an explicit unresolved result under 012.

SPEC-HAL-024, 085A, 263A/263B, 354, 359 and the actual accepted rendering/composition owners remain binding. SPEC-HAL-150 gives the compositor no independent geometry, semantics or animation authority. SPEC-HAL-147 remains the separate authority for authorized post-layout geometry transitions.

A helper or Draft renderer/style document is evidence of available code, not adoption of its policies. In particular, using DecorationStack does not transfer state, focus, clipping or composition ownership to that helper.

### 12.8 Invariants

**INV-FC-SEMANTIC-OWNER:** Only the existing framework and domain owners select focused/captured widgets and perform their lifecycle transitions. Skin configuration and visual parts have no semantic ownership.

**INV-FC-NO-PARALLEL-STATE:** Projection consumes current owner-qualified facts, not widget-local semantic mirrors or visual inference. Separate derived presentation state/handles do not confer semantic authority.

**INV-FC-SINGLETON:** One coordinated complete projection represents a current target in its domain; helpers may emit multiple primitives, but no independent framework/default indicator is appended.

**INV-FC-DETERMINISTIC:** Identical coherent facts, resolved appearance, target geometry and frame-stable presentation samples produce identical composition and required order.

**INV-FC-DECORATIVE-ONLY:** Additional visual coverage cannot change layout, hit-testing, navigation, scroll/reveal, accessibility or interaction semantics.

**INV-FC-CLIP-Z:** All projection obeys existing clip/scroll/surface/composition scope and admitted explicit Z; appearance owns none of them.

**INV-FC-2:** Eligible Navigation-mode focus is visibly indicated as required by §12.4, subject to 208 INV-D3's scoped default-selector exception; application policy supplies eligibility independently of focus truth.

**INV-FC-3:** Current keyboard capture is visibly distinguishable from focus-only state as required by §12.4, including through simultaneous states and permitted transitions.

**INV-FC-ENCLOSURE:** Required labeled-control focus treatment covers indicator, label and gap under 208, including its scoped default-selector exception, subject to existing clip/visibility authority.

**INV-FC-LIFECYCLE:** Existing 021/042/354 ownership governs timing, invalidation, cancellation, replacement and unmount; no stale decoration or orphaned frame demand survives.

**INV-FC-CONTROL-BORDER:** Theme::control_border is a default bordered-control adapter within the complete resolved appearance. Its use does not replace visibility/completeness requirements or constrain every skin to that recipe.

**INV-FC-7:** on_capture_enter and on_capture_leave MUST receive text_engine as specified in §13. This semantic lifecycle requirement is unchanged.

### 12.9 Legacy hooks and compatibility

The following legacy mechanisms are not required by the current appearance contract:

- Widget::focus_indicator_geometry;
- Widget::paints_own_focus_indicator;
- FocusIndicatorGeometry;
- an independent PaintContext::paint_focus_indicator_for call appended by framework traversal; and
- the removed Theme::focus_ring.capture_color field.

Their continued source/API presence does not grant focus, geometry or paint-policy authority. No caller may rely on a legacy path to append an independent default ring after a complete resolved component projection. An existing helper may be retained only as an explicit implementation of the selected complete projection under this section, with no competing emission path.

This section does not require the removed capture_color field to return. Existing focused_width, captured_width and borders.focus may remain default recipe inputs; they are not compulsory values for every admitted product skin.

The obsolete framework-owned-indicator, one-fixed-color/width, resting-primitive-count and no-additional-decoration rules do not survive in an index, example or comment as normative authority. Source migration remains a separate conformance obligation; adoption alone does not declare legacy consumers corrected.

### 12.10 Prohibitions

**FORBID-FC-PARALLEL-STATE:** No semantic focus/capture mirror or inference from visible/animated state.

**FORBID-FC-SEMANTIC-FEEDBACK:** No appearance-driven ownership, capture/routing decision, member selection, eligibility, semantic action or layout/input-geometry mutation.

**FORBID-FC-FRAMEWORK-INDICATOR:** No independent framework/default focus/capture paint layered over the component's selected complete projection.

**FORBID-FC-PRIVATE-ANIMATOR:** No private focus clock, Timeline, delta accumulator, timer restart loop, renderer sampling or autonomous scheduler.

**FORBID-FC-PAINT-MUTATION:** No animation lifecycle, cached-state or semantic mutation during layout, scene build or paint.

**FORBID-FC-STALE-CUE:** No delayed sole cue, outgoing cue that still claims current ownership, stale decoration after lifecycle loss, or anonymous continuing frame demand.

**FORBID-FC-SCOPE-ESCAPE:** No clip/scroll/surface/Z escape or interactive overlay created to expose decoration.

**FORBID-FC-SILENT-CAPABILITY:** No ignored required effect, undeclared fallback or false claim of renderer/resource support.

---

## 13. EventContext Requirements for Lifecycle Callbacks

ALL lifecycle callbacks that create an EventContext MUST
provide a COMPLETE context:

| Callback              | text_engine | animation | keyboard_mode |
|-----------------------|-------------|-----------|---------------|
| on_capture_enter      | REQUIRED    | optional  | Capture       |
| on_capture_leave      | REQUIRED    | optional  | Navigation    |
| on_focus              | optional    | optional  | current       |
| on_blur               | optional    | optional  | current       |
| on_keyboard_activate  | optional    | optional  | Navigation    |

"REQUIRED" means the EventContext MUST include it. Widgets
that call `cx.text_engine()` will panic if it's absent.

"optional" means the callback may function without it, but
providing it is preferred for future-proofing.

**INV-FC-7**: on_capture_enter and on_capture_leave MUST
receive text_engine. This is the root cause of SpinBox
panic — handle_engine_action created EventContext without
text_engine.

HAL-101 sections 2.4-2.5 use one bounded engine-owned composite transaction. CurrentInput origin retains the original event/domain/window/time; ordinary prerequisite focus/Capture activation completes before a child operation that requires it, with remaining directives finalized after its admitted continuation sequence and no duplicate activation/notification. Lifecycle and OwnerProperty origins carry their actual cause and cannot synthesize input or require user Capture as a substitute for their own authority. Each child scope has its own real geometry/widget/animation/invalidation identity and separate engine-qualified delegated input authority; focus/keyboard/pointer requests retain the actual host owner. Actual child focus/Capture facts remain false. Required lifecycle text-engine access and exactly-once role notification remain. No nested EventRouter call, host handle_event replay or independent child tab/Capture owner is admitted.

---

## 14. Composite Widget Focus Targets

Composite widgets (ListView, TreeView, RadioGroup, TabBar, SegmentedControl) hold framework focus on a single widget per INV-F0 — the composite itself. Internal navigation among items mutates internal state (focused row, active tab, focused member) without violating the focus singleton.

Visual indication for an internal target follows §12: the composite supplies its existing owner-authorized target and geometry, and the selected skin provides one coordinated complete projection while the composite holds framework focus. The skin may modulate existing emphasis or add admitted decoration; it cannot choose the target, create a child focus owner or conflate internal selected/focused/active identities. Required labeled-control enclosure remains binding subject to 208 INV-D3's 2026-10-24 default-selector exception, including RadioGroup options. There is no second composite focus authority.

Composite-specific rules unchanged from prior versions of this section:

- Focus enters only via explicit interaction.
- Navigation within is relative to internal structure.
- Scroll does not move focus.
- Focus is anchored by identity (RowId, ColumnId, WidgetId), not by index or pixel position.
- Virtualization must not destroy focus.

(SPEC-HAL-184)

The four HAL-101 section 2.4 hosts retain the sole actual Focus/Capture owner while the engine delegates only their authorized text-entry operation to the registered real child. Internal text delegation is not a child focus owner, another tab stop, copied host geometry or permission for peer access. Apply section 12.1's distinct delegation facts and one complete cue projection.

---

## 15. Repaint and Continuing Frame Demand

The framework MUST invalidate the affected visual projection after:

- Navigation/Capture mode transitions;
- framework focus changes;
- scope push/pop and relevant domain suspension/restoration;
- the Tab auto-capture chain under §3.7; and
- pointer-initiated keyboard capture under §3.1.

Capture entry/exit and focus changes MUST be reflected in the next admitted paint even if an affected widget callback omits its repaint request. This preserves the immediate repaint guarantee; it does not bypass engine frame admission or platform presentation policy.

Widgets remain responsible for correct invalidation after their own authorized internal state changes, such as tab/value/member/highlight changes. Appearance configuration, geometry and presentation-sample changes additionally follow §12.6 and 012/354.

The current EventRouter continuous_frames_requested flag is one implementation path for requesting that work. It is not permission for indefinite rendering after the change settles.

An actually unsettled admitted Timeline effect may keep the existing engine's animation demand active under 021/042/354. Once settled, cancelled, unmounted or replaced without a continuing effect, that demand ends. No mandatory 80 ms duration, framework-owned alpha animator or independent focus scheduler is specified here.

**363/364 scoped amendment:** The §12.6 occurrence dependencies inherit this unconditional
next-admitted-paint obligation. Domain absence remains explicit, and keyboard,
pointer and scroll channels remain independent. No Category-A keyboard capture
or focus-manager algorithm change is authorized.

---

## 16. Complete Invariant Index

From SPEC-HAL-140:
- INV-FOCUS-NO-AUTHORITY
- INV-FOCUS-STABILITY
- INV-CAPTURE-EXPLICIT-ONLY
- INV-CAPTURE-EXCLUSIVE
- INV-KEYBOARD-NO-AUTO-CAPTURE
- INV-ESCAPE-RELEASES-CAPTURE

From SPEC-HAL-176:
- INV-KBD-MODE-1: One mode active per domain
- INV-KBD-MODE-2: Transitions through actions only
- INV-KBD-MODE-3: Navigation never mutates state
- INV-KBD-MODE-4: Escape in Capture = release only
- INV-KBD-MODE-5: Escape in Navigation = pop scope
- INV-KBD-MODE-6: Click on capturable = enter Capture
- INV-KBD-MODE-7: `capture_eligible` is a configuration-based
  declaration. It MAY change with enabled/disabled state
  (e.g., `!self.state.disabled`). It MUST NOT change with
  transient interaction state (focus, hover, pressed,
  in_capture, popup_open, etc.).
- INV-KBD-MODE-8: Spatial nav reads paint rects
- INV-KBD-MODE-9: Mode persists across frames
- INV-KBD-MODE-10: hjkl only in Navigation layer

From SPEC-HAL-126:
- INV-F0: Focus singleton
- INV-F1: Traversal deterministic
- INV-F3: Structural authority (tree, not layout)
- INV-F7: No implicit wrap
- INV-F8: Scope traps Tab
- INV-F9: Scope entry focuses entry widget
- INV-F10: Exit scope only via Escape

From SPEC-HAL-089:
- Focus memory: window deactivation only
- ESC never writes focus memory

From this spec:

- INV-FC-SEMANTIC-OWNER: Framework/domain semantic ownership only (§12.8).
- INV-FC-NO-PARALLEL-STATE: Owner-qualified facts; no semantic mirrors (§12.8).
- INV-FC-SINGLETON: One complete projection per current target/domain, not one primitive (§12.8).
- INV-FC-DETERMINISTIC: Coherent snapshots yield the same values and order (§12.8).
- INV-FC-DECORATIVE-ONLY: Decoration has no layout/input/navigation/scroll/accessibility authority (§12.8).
- INV-FC-CLIP-Z: Existing clip, scroll, surface and explicit-Z ownership (§12.8).
- INV-FC-2: Visible current Navigation focus and composite target (§12.4).
- INV-FC-3: Visible capture, distinguishable from focus-only state (§12.4).
- INV-FC-ENCLOSURE: Indicator, label and gap form the required target (§12.4).
- INV-FC-LIFECYCLE: Existing animation/invalidation/lifetime ownership (§12.8).
- INV-FC-CONTROL-BORDER: Default adapter, subject to complete visibility (§12.8).
- INV-FC-7: text_engine in on_capture_enter and on_capture_leave (§13).

---

## 17. Complete Prohibition Index

- FORBID-KBD-IMPLICIT-CAPTURE: No capture on focus entry
- FORBID-KBD-ESCAPE-SEMANTICS: No widget overrides Escape
- FORBID-KBD-MODE-BYPASS: No direct mode mutation
- FORBID-KBD-HEURISTIC-ROUTING: No heuristic key routing
- FORBID-KBD-NAV-MUTATION: No state mutation in Navigation
- FORBID-FC-DEFOCUS-ON-ESCAPE: Escape does not defocus
- FORBID-FC-FOCUS-MEMORY-ON-ESCAPE: Escape does not
  write or read focus memory
- FORBID-FC-PARALLEL-STATE: No semantic mirrors or inference from visual state (§12.10).
- FORBID-FC-SEMANTIC-FEEDBACK: No appearance-driven semantic or geometry authority (§12.10).
- FORBID-FC-FRAMEWORK-INDICATOR: No independent default indicator over a complete projection (§12.10).
- FORBID-FC-PRIVATE-ANIMATOR: No private time/playback/scheduling owner (§12.10).
- FORBID-FC-PAINT-MUTATION: No lifecycle or state writes during read-only phases (§12.10).
- FORBID-FC-STALE-CUE: No misleading outgoing/current cue, stale decoration or orphan demand (§12.10).
- FORBID-FC-SCOPE-ESCAPE: No clip, scroll, surface, Z or interactive-overlay bypass (§12.10).
- FORBID-FC-SILENT-CAPABILITY: No ignored required effect or undeclared fallback (§12.10).

---

## 18. Integration with Action System (020A-D)

### 18.1 Target Architecture

When the full Action System is implemented, keyboard
authority integrates as follows:

**020C (Binding):** Input → ActionId. Context-agnostic.
`Enter → enter_widget` always, regardless of mode.
`Escape → release_capture` always. Mode does not affect
binding lookup.

**020B (Resolution):** ActionId → scope stack resolution.
- In Navigation mode: navigation actions (focus_left,
  enter_widget) resolve at Application Scope (020B §C.1
  level 5). No widget has a scope claim.
- In Capture mode: the captured widget is the topmost
  scope. Keys resolve through the widget's ActionProvider
  first. If unhandled, resolution falls through to
  Ancestors → Screen → App scope.

This naturally handles app-level shortcuts during Capture
(§2.4): Ctrl+S binds to ActionId::save at App scope.
The captured widget's ActionProvider doesn't handle
save → falls through → App scope handles it.

**020D (Execution):** Resolved action → execute.

**Escape as Hard binding (020C §F):** release_capture
is a Hard priority binding. No layer can override it.
This enforces FORBID-KBD-ESCAPE-SEMANTICS architecturally.

### 18.2 Current Implementation (Pragmatic)

The full 020B scope stack is not implemented. The current
code uses:

- `dispatch_engine_action` (app.rs): checks keyboard_mode,
  does direct binding lookup, calls handle_engine_action
- `handle_engine_action` (event_router.rs): directly
  processes engine actions (focus moves, enter_widget,
  release_capture)

This bypasses 020B resolution entirely. When the full
Action System lands:

1. dispatch_engine_action is replaced by the 020C→020B→020D
   pipeline
2. Mode-based gating is replaced by scope stack resolution
3. handle_engine_action is replaced by action executors
   registered at appropriate scope levels
4. Gap O (app-level shortcuts) is solved naturally by
   scope fallthrough — no special modifier-key handling
   needed

### 18.3 Migration Path

Until 020A-D lands, the current pragmatic implementation
is acceptable. It must maintain behavioral parity with the
target architecture: same actions fire, same resolution
order, same results. When 020A-D is implemented, the
migration is mechanical — replace the direct dispatch with
scope-stack resolution.


## Scoped physical response capture — 2026-10-10

**Author:** JC. The accepted physical-input prerequisite is independent
of unadopted Draft/Proposed scope. Implementation is a 0.211.0-beta source candidate
with retained focused implementation proof. Independent source
acceptance remains pending. The owner's later scoped refinement below supersedes
the candidate's original clipboard-interruption interpretation; earlier proof
remains bound to its original source.

An existing Capture-eligible widget may statically declare
`physical_keyboard_capture()`. This does not add a mode or turn Category K into
a continuous recorder. Ordinary responses remain captured and carry both logical
and native physical identity, including wrong/unbound keys. Existing logical UI
defaults retain their policy.

**Owner scope refinement (2026-10-10):** The existing native clipboard
annotation selects the ordinary clipboard/text path only outside opted-in
physical Capture. There it delivers the existing text payload exactly once and
does not additionally deliver the annotated Key. Inside live physical Capture,
the original physical Down/Up and authoritative modifiers reach that owner;
the paired clipboard text is suppressed, including after a one-shot recorder
has released Capture. No logical binding is reinterpreted.

Fullscreen/debug reservations and actual App/global action consumption produce
a typed `on_keyboard_capture_interrupted` notification for the current live
physical capture owner, followed by the existing release action. The original
operation remains owned by its existing path. Escape still releases
unconditionally and retains focus; notification cannot veto, roll back committed
data, or recapture. Tab remains widget-first: an unhandled Tab reports interruption
before the existing release/focus chain; a handled Tab does not acquire a new
navigation policy. Focus loss uses the existing domain teardown. Pending
uncommitted response work is settled synchronously on interruption/leave and the
existing `on_unmount` boundary before deferred consumer work; committed values
remain intact. A dead mount receives no synthesized capture callback.
An OS operation absent from both key and lifecycle transport cannot be invented
as a physical event. Unverified native/layout behavior is not claimed as proof.

**Observable-boundary decision (2026-10-10; Author JC):** Settlement for
an observed hard reservation or declared App/global consumption precedes the
original operation, not merely its return. The existing shared qualified drain
owns this ordering, including the native fullscreen actuator. Capture loss/focus
loss settle current pending work; focus return retains Navigation and does not
auto-rearm. No observation means no guarantee and no synthetic interruption or
answer. The existing event/binding owner exposes typed reserved/unobserved
classification so a consumer cannot count these as wrong physical responses or
accept them as training bindings. Tab's widget-first law and conditional clipboard
delivery remain unchanged. No mode, router, global hook, or polling is added.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under coordinated architectural adoption, with JC's final ownership and veto retained. The retained adoption record identifies the exact three source bindings and adopted scope. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.

## 19. Read-only selection clear reason - 2026-11-14

**ADOPTED narrow extension coordinated with HAL390 section16.5.** The existing
router classifies an uncaptured primary outside pointer-down separately from
an explicit request to clear a read-only selection. Selection retention does
not alter focus, keyboard mode, pointer/keyboard capture, modal precedence,
hit testing, event ownership, or the next widget's normal activation.

Widget gains the defaulted
clear_read_only_text_selection_on_outside_down(&mut self) -> bool hook.
Its default delegates exactly once to the existing
clear_read_only_text_selection(&mut self) -> bool. The existing explicit hook
and its original callers/implementations are preserved. Both return whether
selection state changed; neither return value consumes the pointer event.

EventRouter uses the new hook only at its existing
clear_focused_read_only_selection_on_outside_down call site, before the
original focus assignment. Keep actual focused-owner resolution, descendant
hit test and no-hit handling, the original non-touch/Primary/!is_captured
checks, and the existing changed-state invalidation. No policy inference from
the target's name, type, label or action; no alternate router or capture mirror.

Only HAL390's Enabled MarkdownView with RetainOnBlur currently overrides
this implicit operation to preserve its logical reading interval. Its real
focus loss and capture release still occur, and explicit clear still clears.
Default Markdown and every other current widget retain original behavior
through default delegation. Retention does not make an unfocused view an
input or Copy recipient. HAL315's remaining Draft scope is not adopted.

Implementation remains part of the HAL390 reading-state handoff. HAL390
section16.5 names the exact expanded files and real routed regression proof.
This owner clarification does not amend the Constitution or authorize a general
selection service.

## 20. Captured modifier keys reach the widget before the application - 2026-11-14

**ADOPTED scoped implementation correction under sections2.3 and3.7 and
SOL013 section17.4.** This is not a constitutional amendment, a new input
service, a new public API or adoption of the remaining Draft HAL018/HAL079.
The original user result is a Conversation-inclusive visual switcher that
cannot reinterpret a shortcut already consumed by its captured editor.

### 20.1 Source-qualified gap

Current app.rs:2514 calls AppRoot::handle_fused_input_modal_first before
the ordinary widget router. That method invokes Screen::handle_input and
App bindings before widget delivery (10180-10290). An application's Screen
handler can therefore consume Ctrl+Tab before the captured editor.
Conversely, event_router.rs:6648-6724 offers the key to the widget but has
no actual App continuation after an unhandled modifier; its binding argument
is unused and modified Tab can fall into ordinary Tab navigation. The comment
in dispatch_engine_action promising widget-first/App fallthrough does not
establish that behavior. This is a deterministic source-order gap, not a
new observation of a frozen window.

### 20.2 Selected ordinary route

Keep the existing qualified input and actual owning domain/window. Only defer
the pre-widget base-Screen/App dispatch when all of these facts hold:
- the input is an original KeyDown with Ctrl, Alt or Meta;
- its key is not Escape or a bare Control/Shift/Alt/Meta edge;
- the current live focused owner has normal keyboard Capture, not declared
  physical-key capture;
- no native/engine reservation or registered-global-shortcut receipt applies;
- no current modal/overlay owns the application input route.

Existing hard reservations, physical-response capture, overlay/modal-first
routing, window admission and input-session qualification execute first and
remain unchanged. Navigation mode, plain keys, Shift-only keys, modifier
edges and existing unmodified function-key handling are not reordered here.

Offer an eligible key to its actual captured widget EXACTLY ONCE through the
original EventRouter dispatch. A true handled result forbids all Screen/App
fallback and preserves existing paired-Text suppression. A false result can
offer that SAME original qualified event to the existing Screen/App path once,
after the widget dispatch and all its borrows have ended, before the existing
post-route Screen flush. No reconstructed Key, second routing pass, direct
widget call from a consumer or inferred unhandled result is allowed.

Use a private per-invocation CapturedAppKeyOutcome with explicit NotEligible,
WidgetHandled and AppFallback outcomes. Carry the outcome as a stack-local
return from the existing qualified router path to App orchestration. Its
ordinary public unit-returning wrappers remain source-compatible. A small
private common-inner/for-App wrapper is allowed; it must execute the SAME
router implementation once, not a second key path. Preserve the relative-facts
wrapper's actual authority without altering pointer/geometry semantics.
No persistent router latch, queue, public receipt, observer or callback registry.

AppFallback is produced only after the original normal captured widget
actually returned false. Before application delivery, revalidate the original
live concrete owner, domain/window and input root and absence of takeover.
An owner retirement/change or intervening modal/physical takeover cannot
redirect this old key to another owner. The disposition is dropped, not retried.
Any stored qualification is local to this invocation and uses existing IDs/
mount tokens, never a Screen/widget reference surviving dispatch.

The same production helper is used by run_ui_loop and ordinary App::run.
Call the existing AppRoot handling at most once for the deferred event; retain
its Screen -> global/binding semantics. Its navigation remains a request
settled at the original legal mutation phase. An App-consumed key arms only
the existing exact paired-Text suppression. If neither widget nor App handles
a modified key, drop it under section2.3: no focus navigation or capture release.
In particular Ctrl/Alt/Meta-modified Tab belongs to section2.3(3); only plain
Tab/Shift+Tab use the original section3.7 auto-capture chain.

All original input pipeline phases, availability settlement, clipboard
annotation, IME/editor authority, signals, frame ordering and original key-up
transport remain. No hard reservation for an application switcher is introduced.
No application key names, state or types enter the framework.

**INV-MODIFIER-WIDGET-FIRST:** an eligible captured key reaches its real widget
before Screen/App action handling, once.
**INV-MODIFIER-NO-REPLAY:** an actual handled result or stale owner forbids
application fallback and second widget delivery.
**INV-MODIFIER-NO-NAV-FALLBACK:** unhandled modified input does not masquerade
as plain Tab or spatial navigation.
**INV-MODIFIER-SAME-PATH:** native and automated UI loops use the same
production dispatch and existing deferred navigation.

### 20.3 Scope and complete proof

Implementation belongs to the existing App and EventRouter owners.

Own src/core/app.rs and src/core/event_router.rs, nearest existing inline
App/router tests and their qualified loop fixtures, and scoped release files.
No public Screen/Widget/Event API, platform input, widget algorithm, renderer,
application source or optical behavior change. This is an internal conformance
fix: increment the current package patch once under the existing release rule,
preserving the beta suffix and only its own lock record. Do not lower another
owner's intervening release.

Add the focused captured_modifier_ cases in the existing app.rs test surface.
Prove real target handling precedes Screen handling; consumed key never reaches
App; refused key reaches App once; no handler drops without focus/Capture
change; plain Tab retains its original path; paired Text is suppressed only
for consumption; stale/retired target cannot redirect; reservations, physical
capture and modal precedence remain on their existing paths. Use actual
TextField and a genuinely captured consuming widget, not a fabricated result.
Exercise the common production dispatch through the existing UI-loop fixture
and source-qualify both normal call sites. Tests must not call only the new
private result function while bypassing App intake.

Run focused new cases, affected existing keyboard/physical/modal tests,
changed refresh/check and package build sequentially. Preserve earlier RED
receipts and produce a final exact source/receipt boundary. No model job,
system clipboard access or replay of previously closed consumer proof is admitted.
The complete application-switcher native scenario supplies separate consumer
observation; this unit does not claim physical keystrokes or a completed product.
