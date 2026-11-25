# SPEC-HAL-176: Explicit Keyboard Authority Modes

**Status:** Design / Binding
**Layer:** Interaction / Input Semantics / Focus Authority
**Scope:** Keyboard routing modes, authority transitions, spatial navigation, hierarchical capture
**Depends on:**

* SPEC-HAL-002 (Constitution)
* SPEC-HAL-020A (Action System Core Model)
* SPEC-HAL-020B (Contextual Action Resolution)
* SPEC-HAL-020C (Action Binding and Input Mapping)
* SPEC-HAL-020D (Action Execution and Scope Lifecycle)
* SPEC-HAL-038 (Focus Model & Input Routing — implemented baseline)
* SPEC-HAL-089 (Focus Memory Policy — implemented baseline)
* SPEC-HAL-126 (Focus Traversal and Tab Order Contract)
* SPEC-HAL-140 (Focus vs Capture Interaction Authority Separation)
* SPEC-HAL-151 (Multi-Window Focus and Interaction Domains)

**Non-Goals:**

* Visual focus ring rendering
* Accessibility narration
* IME behavior
* Command palette / search UI (deferred)
* Specific widget keybind tables (widget-level SPECs)

---

## Application-policy qualification — adopted 2026-10-14

The construction-time UiInteractionPolicy contract in SPEC-HAL-328 governs this
document's navigation delivery, activation, Capture entry and Tab traversal
clauses. NavigationAndCapture remains the unchanged default. PointerDriven
disables automatic UI keyboard delivery outside actual Capture while preserving
explicit Screen/App commands, pointer-started Capture and Capture key meanings.
Unhandled Capture Tab releases and retains the focused owner without traversal
or auto-capture chaining. Escape and domain lifecycle remain unchanged. There
are still exactly two KeyboardAuthorityMode values: Navigation and Capture.

## 1. Problem Statement

Most GUI systems conflate three distinct concerns:

* **Focus** — which widget is the navigational target
* **Capture** — which widget exclusively owns keyboard input
* **Navigation** — how the user moves intent across the UI

This conflation produces:

* accidental state mutation during navigation
* unreliable Escape semantics (per-widget guessing)
* focus traps with no predictable exit
* ambiguous keyboard ownership
* inability to traverse complex UIs without a mouse

Halcyon already separates focus and capture at the engine level (SPEC-HAL-140). What is missing is a **modal authority system** that governs how keyboard input is interpreted — navigation vs. mutation — and how transitions between these interpretations occur.

---

## 2. Core Concept

Halcyon introduces **Explicit Keyboard Authority Modes**.

Keyboard input is always routed according to a **single active authority mode**. The mode determines which binding table layer is active (per SPEC-HAL-020C). The mode is never inferred heuristically.

There are exactly **two modes**.

---

## 3. Authority Modes

### 3.1 Navigation Mode

**Definition:** Keyboard input is interpreted as **navigation intent**. No widget owns the keyboard. No keystroke mutates widget state.

**Properties:**

* Keys traverse the focus graph (spatial or structural)
* Widgets receive no mutating input
* Navigation is side-effect free
* The active binding table layer is `navigation`

**Bound actions (020C navigation layer):**

| Input | ActionId | Semantics |
|-------|----------|-----------|
| `h` / `ArrowLeft` | `action::focus_left` | Spatial: nearest focusable to the left |
| `j` / `ArrowDown` | `action::focus_down` | Spatial: nearest focusable below |
| `k` / `ArrowUp` | `action::focus_up` | Spatial: nearest focusable above |
| `l` / `ArrowRight` | `action::focus_right` | Spatial: nearest focusable to the right |
| `Tab` | `action::focus_next` | Structural: next in tree-order (SPEC-126) |
| `Shift+Tab` | `action::focus_prev` | Structural: previous in tree-order (SPEC-126) |
| `i` / `Enter` | `action::enter_widget` | Enter capture if capturable; activate if not (§4.1) |
| `Space` | `action::activate` | Inline activation without mode change (§4.3) |
| `Escape` | `action::exit_scope` | Pop one focus scope level (§5.2) |

`hjkl` and arrow keys are **spatial** — they consult layout geometry (paint rects) to find the nearest focusable widget in the requested direction. This is a distinct resolver from Tab's structural tree-order traversal (SPEC-126 INV-F3).

`Tab`/`Shift-Tab` remain **structural** — tree-order, per SPEC-126.

Both spatial and structural navigation coexist. They are not redundant — spatial is for "go to the thing I can see to my left," structural is for "go to the next thing in logical order."

---

### 3.2 Capture Mode

**Definition:** Keyboard input is routed **exclusively** to a captured widget. The widget owns the keyboard.

**Properties:**

* All keystrokes route to the captured widget's action scope
* The captured widget's action provider (020A §D) defines available actions
* Navigation keys are interpreted by the widget, not the focus system
* Other widgets receive no keyboard input
* The active binding table layer is `capture`

**Bound actions (020C capture layer):**

| Input | ActionId | Semantics |
|-------|----------|-----------|
| `Escape` | `action::release_capture` | Release keyboard capture, return to navigation (§4.2) |
| All others | Widget-defined | Routed to captured widget per 020B resolution |

The capture layer contains only one engine-level binding: Escape releases capture. Everything else is widget-owned. A TextField defines its own text editing actions. A Slider defines arrow-key value changes. These are exposed through the widget's ActionProvider (020A §D) and resolved through the normal 020B scope stack, with the captured widget at the top.

---

## 4. Authority Transitions

All mode transitions are **actions** resolved through SPEC-HAL-020B and executed through SPEC-HAL-020D. They are never raw events, never hardcoded key handlers, and never bypass InputFusion or the event router.

### 4.1 Entering Capture Mode (`action::enter_widget`)

Resolved when `i` or `Enter` is pressed in Navigation Mode.

**Resolution logic:**

1. Identify the currently focused widget
2. Query widget's `capture_eligible` declaration (§6)
3. If `capture_eligible == true`:
   - Push current focus scope onto the scope stack (§5)
   - Set keyboard authority to Capture Mode
   - Widget receives `CaptureGained` event
   - Active binding table switches to `capture` layer
4. If `capture_eligible == false` and widget is a **scope container** (§6.3):
   - Push the widget's focus scope onto the scope stack
   - Focus moves to scope's entry widget (SPEC-126 INV-F9)
   - Authority remains Navigation Mode (children are navigable)
5. If `capture_eligible == false` and widget is **not** a scope container:
   - Execute `action::activate` instead (inline activation)
   - Authority mode remains Navigation
   - Widget receives activation event (e.g., Button fires `on_click`)

**Pointer-initiated capture (mitigation path):**

Mouse click on a capturable widget **immediately enters Capture Mode**. Pointer interaction implies edit intent. This preserves mouse ergonomics without polluting keyboard semantics. The transition still goes through the action system: the click resolves to `action::enter_widget` for the clicked target.

### 4.2 Exiting Capture Mode (`action::release_capture`)

Resolved when `Escape` is pressed in Capture Mode.

**Semantics are absolute:**

* Release keyboard capture
* Return to Navigation Mode
* Focus remains on the same widget
* **No rollback**
* **No cancellation**
* **No state reset**
* **No clearing of input**
* **No widget-specific Escape behavior**

Escape means exactly one thing: release keyboard authority. The widget keeps its current state. If the user wants to undo changes, that is `Ctrl+Z` — a separate action, not Escape.

This invariant is non-negotiable. Any widget that overrides Escape to clear, cancel, or revert is **in violation of this SPEC**.

### 4.3 Inline Activation (`action::activate`)

Resolved when `Space` is pressed in Navigation Mode, or when `Enter`/`i` targets a non-capturable, non-scope widget.

**Semantics:**

* Fire the widget's primary activation action
* Do not change authority mode
* Remain in Navigation Mode

Examples: Button press, Checkbox toggle, Expander collapse/expand.

### 4.4 Transition Summary

```
Navigation Mode
  │
  ├─ i / Enter on capturable widget ──→ Capture Mode
  │                                        │
  │                                        ├─ Escape ──→ Navigation Mode
  │                                        │              (focus unchanged, state preserved)
  │                                        │
  │                                        └─ All other keys ──→ widget-owned
  │
  ├─ i / Enter on scope container ──→ push scope, enter children (stay Navigation)
  │
  ├─ i / Enter on non-capturable widget ──→ activate (stay in Navigation)
  │
  ├─ Space ──→ activate (stay in Navigation)
  │
  ├─ hjkl / arrows ──→ spatial focus move (stay in Navigation)
  │
  ├─ Tab / Shift-Tab ──→ structural focus move (stay in Navigation)
  │
  └─ Escape ──→ pop focus scope (§5.2)

Mouse click on capturable widget ──→ Capture Mode (direct)
```

---

## 5. Hierarchical Focus Scopes

### 5.1 Scope Stack

Focus scopes (SPEC-126 §6, INV-F8–F10) form a **stack**. Entering a compound widget that defines a focus scope pushes a new scope level. Escape pops one level.

Example flow:

```
[App root — Navigation Mode]
  │
  ├─ User presses i on a Panel (scope container)
  │  → Push Panel's focus scope
  │  → Focus enters Panel's first focusable child
  │  → Still Navigation Mode (Panel contains navigable children)
  │
  ├─ User navigates to a TextField inside the Panel (hjkl)
  │
  ├─ User presses i on the TextField
  │  → Enter Capture Mode
  │  → TextField owns keyboard
  │
  ├─ User presses Escape
  │  → Release capture
  │  → Return to Navigation Mode
  │  → Focus remains on TextField
  │  → Still inside Panel's focus scope
  │
  ├─ User presses Escape again
  │  → Pop Panel's focus scope
  │  → Focus returns to Panel in parent scope
  │  → Navigation continues at parent level
  │
  └─ User presses Escape at top level
     → No-op (no scope to pop, already at root)
```

### 5.2 Escape Semantics by Context

| Context | Escape Action | Result |
|---------|---------------|--------|
| Capture Mode | `action::release_capture` | Return to Navigation, keep focus, keep state |
| Navigation Mode, inside nested scope | `action::exit_scope` | Pop one focus scope level |
| Navigation Mode, root scope | No-op | Already at top, nothing to pop |
| Modal overlay active | `action::dismiss_overlay` | Overlay scope handles (020B §D.3) |

Escape resolution follows the normal 020B scope stack. The topmost scope that handles the Escape action wins. This means:

* If captured, capture owner handles Escape (release)
* If in nested scope, scope boundary handles Escape (pop)
* If modal overlay, overlay handles Escape (dismiss)
* If none of the above, unhandled (no-op per 020B §I)

No special-casing. Escape is an action like any other (020B PRINCIPLE-5).

---

## 6. Widget Capture Eligibility

### 6.1 Declaration

Every widget declares its capture eligibility:

```rust
fn capture_eligible(&self) -> bool;
```

Default: `false`.

Widgets that consume keyboard input for state mutation return `true`. Widgets that only need activation return `false`.

### 6.2 Eligibility by Widget Type

| Widget | `capture_eligible` | Rationale |
|--------|--------------------|-----------|
| TextField | `true` | Consumes character input |
| TextArea | `true` | Multi-line text editing |
| Slider | `true` | Arrow keys change value |
| NumericInput | `true` | Digit entry + arrows |
| Dropdown (open) | `true` | Keyboard list navigation |
| ColorPicker | `true` | Component editing |
| Button | `false` | Activation only |
| Checkbox | `false` | Toggle only |
| RadioButton | `false` | Toggle only |
| Expander | `false` | Collapse/expand only |
| Label | `false` | Non-interactive |
| Panel / Container | scope-based | Defines focus scope, not capture (§6.3) |
| List / Tree | scope-based | Contains navigable children |

### 6.3 Compound Widgets and Scope Entry

Compound widgets (Panel, List, Tree, TabBar) that contain focusable children define a **focus scope** rather than declaring `capture_eligible = true`. When `action::enter_widget` targets a compound widget:

1. Push the widget's focus scope
2. Focus moves to the scope's entry widget (SPEC-126 INV-F9)
3. Authority remains Navigation Mode (children are navigable)

This is distinct from capture. The user navigates inside the compound widget using the same hjkl/Tab keys. Escape exits the scope.

A widget declares scope behavior via:

```rust
fn defines_focus_scope(&self) -> bool;
```

Default: `false`.

A widget may be `capture_eligible` OR `defines_focus_scope`, never both. If both are declared `true`, the implementation is in violation.

---

## 7. Spatial Focus Resolution

### 7.1 Resolver

Spatial navigation (`action::focus_left/right/up/down`) requires a **spatial focus resolver** that consults layout geometry.

**Algorithm:**

1. Get the paint rect of the currently focused widget
2. Enumerate all focusable widgets within the current focus scope
3. For the requested direction, filter candidates:
   - `focus_left`: candidates whose right edge is ≤ current widget's left edge
   - `focus_right`: candidates whose left edge is ≥ current widget's right edge
   - `focus_up`: candidates whose bottom edge is ≤ current widget's top edge
   - `focus_down`: candidates whose top edge is ≥ current widget's bottom edge
4. Among filtered candidates, select the one with **minimum distance** from the current widget's center to the candidate's center, projected onto the primary axis, with secondary axis distance as tiebreaker
5. If no candidate exists in the requested direction, no-op (no wrap)

### 7.2 Invariants

**INV-SPATIAL-1:** Spatial resolution reads paint rects (post-layout geometry). It MUST NOT trigger layout.

**INV-SPATIAL-2:** Spatial resolution operates within the current focus scope only. It does not cross scope boundaries.

**INV-SPATIAL-3:** Spatial navigation does not wrap. Pressing `h` at the leftmost widget does nothing.

**INV-SPATIAL-4:** Spatial resolution is deterministic. Same geometry, same result.

**INV-SPATIAL-5:** If layout changes between frames, spatial resolution uses the current frame's geometry. No caching of spatial relationships across frames.

### 7.3 Coexistence with Structural Navigation

Spatial (hjkl/arrows) and structural (Tab/Shift-Tab) navigation are independent systems. They share the focus singleton (SPEC-126 INV-F0) but use different resolution algorithms. A user can freely alternate between them.

---

## 8. Integration with Action System (020A-D)

### 8.1 Mode as Binding Table Layer (020C)

The keyboard authority mode is implemented as a **binding table layer** per SPEC-HAL-020C §E.

* Navigation Mode activates the `navigation` binding layer
* Capture Mode activates the `capture` binding layer

Mode transition swaps the active layer. No new routing mechanism is introduced. The existing 020C binding → 020B resolution → 020D execution pipeline handles everything.

### 8.2 Mode-Specific Actions (020A)

New engine-level actions:

| ActionId | Canonical Name | Scope |
|----------|---------------|-------|
| `action::focus_left` | `halcyon.nav.focus_left` | App |
| `action::focus_right` | `halcyon.nav.focus_right` | App |
| `action::focus_up` | `halcyon.nav.focus_up` | App |
| `action::focus_down` | `halcyon.nav.focus_down` | App |
| `action::focus_next` | `halcyon.nav.focus_next` | App |
| `action::focus_prev` | `halcyon.nav.focus_prev` | App |
| `action::enter_widget` | `halcyon.nav.enter_widget` | App |
| `action::activate` | `halcyon.nav.activate` | App |
| `action::release_capture` | `halcyon.capture.release` | Capture owner |
| `action::exit_scope` | `halcyon.nav.exit_scope` | Current scope |

These are standard actions with stable ActionIds (020A INV-ACT-ID-1). They are resolved through the normal 020B scope stack.

### 8.3 Resolution Order (020B)

In Navigation Mode, navigation actions resolve at App scope (020B §C.1 level 5) because no widget has capture. The spatial resolver executes the focus move.

In Capture Mode, `action::release_capture` resolves at the captured widget's scope (020B §C.1 level 2). All other keys resolve within the captured widget's action provider.

Overlay boundaries are enforced automatically (020B §D). Modal overlays block navigation actions from reaching widgets below.

### 8.4 InputFusion Compliance

All keyboard events enter through InputFusion. Mode state does not bypass, short-circuit, or alter the InputFusion → EventRouter pipeline. The only thing that changes is which binding table layer is active when the event reaches the binding lookup step.

---

## 9. Integration with Existing Systems

### 9.1 SPEC-HAL-140 (Focus vs Capture Separation)

SPEC-176 is the **behavioral surface** of SPEC-140's architectural separation. SPEC-140 defines that focus and capture are distinct engine states. SPEC-176 defines when and how transitions between them occur via keyboard.

SPEC-140's interaction states map directly:

| SPEC-140 State | SPEC-176 Mode |
|--------------|--------------|
| Focused | Navigation Mode |
| Focused + Captured | Capture Mode |

### 9.2 SPEC-HAL-126 (Focus Traversal)

Tab/Shift-Tab traversal (SPEC-126) operates unchanged. SPEC-176 adds spatial traversal as a parallel system. Both share the focus singleton.

SPEC-126 focus scopes (INV-F8–F10) are used for the hierarchical scope stack (§5).

### 9.3 SPEC-HAL-089 (Focus Memory)

Focus memory policy applies normally. When capture releases, focus remains on the widget (no memory restore needed). When a scope pops, focus returns to the parent scope's remembered position per SPEC-089.

### 9.4 SPEC-HAL-151 (Interaction Domains)

Keyboard authority mode is per-interaction-domain (per-window). Each window independently tracks its mode state. Window deactivation does not change mode — it preserves mode for restoration on reactivation.

---

## 10. Invariants

**INV-KBD-MODE-1:** Exactly one keyboard authority mode is active at any time per interaction domain.

**INV-KBD-MODE-2:** Mode transitions are actions resolved through 020B. No direct mutation of mode state outside the action system.

**INV-KBD-MODE-3:** Navigation Mode never mutates widget state. Any keystroke that would mutate state in Navigation Mode is a violation.

**INV-KBD-MODE-4:** Escape in Capture Mode releases capture only. No rollback, no cancellation, no state reset, no widget-specific behavior.

**INV-KBD-MODE-5:** Escape in Navigation Mode pops one focus scope level. At root scope, Escape is a no-op.

**INV-KBD-MODE-6:** Pointer click on a capturable widget enters Capture Mode. The transition goes through the action system, not a bypass path.

**INV-KBD-MODE-7:** `capture_eligible` is a static widget declaration, not a runtime query. It does not change based on widget state.

**INV-KBD-MODE-8:** Spatial navigation reads post-layout paint rects. It does not trigger layout or modify geometry.

**INV-KBD-MODE-9:** Mode state persists across frames. It is not recomputed per-frame.

**INV-KBD-MODE-10:** hjkl bindings exist only in the Navigation binding layer. In Capture Mode, h/j/k/l are ordinary character keys routed to the widget.

---

## 11. Prohibitions

**FORBID-KBD-IMPLICIT-CAPTURE:** No widget enters Capture Mode by receiving focus. Focus alone grants no keyboard authority.

**FORBID-KBD-ESCAPE-SEMANTICS:** No widget may override Escape to perform state changes (clear, cancel, revert, close). Escape is structurally reserved for authority/scope transitions.

**FORBID-KBD-MODE-BYPASS:** No component may inspect or set keyboard authority mode directly. Mode changes occur only through action dispatch.

**FORBID-KBD-HEURISTIC-ROUTING:** Keyboard routing must never be heuristic. The active mode and binding table determine routing deterministically.

**FORBID-KBD-NAV-MUTATION:** Navigation-mode keystrokes must not produce side effects in any widget. If a widget receives a navigation key and mutates state, the implementation is incorrect.

---

## 12. Deferred

The following are explicitly out of scope for this SPEC:

* **Keyboard search / jump (`/` or `f`)** — requires transient overlay UI, matching target semantics, and interaction with type-ahead (SPEC-124). Separate SPEC.
* **Visual mode indicators** — how the UI signals current mode to the user. Separate SPEC.
* **Per-widget keybind tables** — widget-specific action providers are defined by widget-level SPECs, not here.
* **Gamepad / controller mapping** — future input device support. Bindings are extensible via 020C.

---

## 13. Implementation Phases

### Phase 1 — Mode State and Transitions

* Add `KeyboardAuthorityMode` enum (`Navigation`, `Capture { owner: TreeWidgetId }`) to interaction domain state
* Implement `action::enter_widget` and `action::release_capture` as engine actions
* Wire Escape in Capture Mode to `action::release_capture`
* Wire `i`/`Enter` in Navigation Mode to `action::enter_widget`
* Add `capture_eligible()` to Widget trait (default `false`)
* Pointer click on capturable widget triggers `action::enter_widget`

### Phase 2 — Spatial Focus Resolver

* Implement spatial focus resolver reading paint rects
* Wire `action::focus_left/right/up/down` to spatial resolver
* Wire `h/j/k/l` and arrow keys to spatial actions in navigation binding layer
* Existing Tab/Shift-Tab remains structural (SPEC-126)

### Phase 3 — Hierarchical Scope Stack

* Implement focus scope stack (push on scope entry, pop on Escape)
* Wire `action::exit_scope` to scope pop
* Wire `action::enter_widget` on compound widgets to scope push
* Integrate with SPEC-126 focus containment (INV-F8–F10) and SPEC-089 focus memory

### Phase 4 — Widget Integration

* Audit all widgets for `capture_eligible` declarations
* Ensure TextField, Slider, NumericInput etc. work correctly in Capture Mode
* Ensure Button, Checkbox, Expander etc. activate correctly from Navigation Mode
* Verify Escape never clears/cancels/reverts in any widget

---

## 14. Conclusion

Keyboard authority in Halcyon is **explicit, exclusive, and mode-driven**.

* Navigation never mutates.
* Capture is intentional.
* Escape changes authority, not state.
* Modes are binding table layers, not parallel routing mechanisms.
* All transitions are actions resolved through the standard 020A-D pipeline.
* Spatial and structural navigation coexist.
* Focus scopes provide hierarchical enter/exit.
* Mouse capture is allowed as an ergonomic path, routed through the same action system.

## Scoped physical-input prerequisite — 2026-10-10

**Author:** JC. The accepted prerequisite distinguishes physical key identity
from logical keys and composed text for applications that record physical-key
responses. Implementation is a 0.211.0-beta source candidate with retained focused
synthetic implementation proof. Independent source acceptance and native input
proof remain separate. No additional authority mode, draft adoption, or
constitutional amendment is made.

Native physical identity is retained through Raw, Fusion, and routed Key events;
unknown identity is explicit, and character/layout substitution is forbidden for
physical binding. Existing logical UI defaults remain unchanged. An eligible
widget may statically opt into physical responses under existing Capture. It
receives wrong/unbound keys as well as expected keys, and may remain captured
across responses without manufacturing action IDs. Only initial nonrepeat
nonmodifier Down is a response/recording intent.

Existing Escape release, widget-first Tab, focus/domain teardown and reserved
operations keep their authority. A physical capture owner receives a typed
notification before observed reservations or authority loss settle its pending
response; notification cannot veto the original operation or undo a committed
value. Deferred consumer responses must be qualified by the owner's current
capture lifetime and invalidated synchronously on leave/interruption. Halcyon
does not synthesize an OS key observation that the native transport did not supply.

The 2026-10-10 owner refinement preserves widget-first Tab and reserves actual
Escape, fullscreen and debug operations through explicit physical-capture
interruption. Clipboard annotation is conditional on the current existing
Capture owner: ordinary consumers retain the one text operation, while opted-in
physical Capture receives the original key and no paired clipboard text.
Navigation/Capture remain the existing two modes; no separate physical mode or
router is admitted. Dual-side modifier accounting, repeat filtering and existing
focus/unmount settlement remain required.

**Observable-boundary decision (2026-10-10; Author JC):** Framework-
observed hard reservations and declared App consumption interrupt the current
physical Capture before their reserved operation executes. Native fullscreen
recognition remains transport data until the existing qualified App drain has
settled Capture; the native owner then performs its original toggle. Actual
focus loss and capture loss interrupt pending work. Focus return never re-arms
Capture. If the OS supplies neither key nor lifecycle observation, no interruption
or answer is promised. No synthetic response, OS hook, or polling is admitted.
Reserved/unobserved combinations are exposed as unavailable physical training
bindings. The 0.211.0-beta candidate has retained focused synthetic implementation
proof; independent source acceptance and native proof remain separate.

**Original-key reservation correction (2026-10-10; Author JC):** A
selected registered Global Fusion shortcut is retained as private immutable
normalization evidence on its qualified original Down. Under opted-in physical
Capture, the shared App owner interrupts and leaves Capture before that Down can
reach the response/recording owner and suppresses its paired text residue. If App
already declares a separate operation on that Down, its original handling remains
after interruption; otherwise that Down delivery is consumed. The derived
Shortcut still reaches its existing App owner once. Global scope does not grant
Hard binding priority. Ordinary Capture
and Navigation retain their existing original-key and derived-shortcut delivery;
normalization does not inspect authority mode or widget state. No late
interruption can repair an already committed one-shot value, and no effectful
Screen callback is used for reservation prediction. This is the accepted bounded
correction, with retained focused implementation proof and independent source
review pending.
