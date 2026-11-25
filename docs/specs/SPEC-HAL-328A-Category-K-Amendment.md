# SPEC-HAL-329: **KeybindEditor — Single-Stroke Keybinding Capture Widget**

### *Producer of `Shortcut` values; consumer of nothing*

**Author:** JC
**Status:** Binding
**Depends on:**

- SPEC-HAL-001 (Core Architecture)
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-020A (Action System — Core Model and Invariants)
- SPEC-HAL-020C (Binding Registry)
- SPEC-HAL-328 (Keyboard Authority, Focus, and Capture)
- SPEC-HAL-260A (PropertyEditor — consumes this widget as the `PropertyValue::KeyBinding` cell editor)

---

# 1. Purpose and Scope

**Adopted application-policy qualification (2026-10-14):** SPEC-HAL-328's
UiInteractionPolicy qualifies this document's focus-only cues and Tab traversal.
The default NavigationAndCapture is unchanged. In PointerDriven, pointer entry
still starts genuine Category-K Capture with its visible recording cue; Tab
remains reserved, cancels without committing, releases Capture and retains the
focused owner without traversal or neighbor auto-capture. Navigation-only cues
require framework-published eligibility. Escape and physical key reservations
retain their existing meaning. No binding, authority mode or capture owner is
added.

## 1.1 What KeybindEditor is

`KeybindEditor` is a standalone Halcyon widget that edits a single `Option<Shortcut>` value via one-stroke keyboard capture. When focused and entered into capture mode, the widget waits for one non-modifier key press, captures the key + currently-held modifier state as a `Shortcut`, fires the editor's `on_change` callback, and releases capture programmatically.

The widget is a **producer of `Shortcut` values**. It does not consume the captured value. It does not register the captured value with the `BindingRegistry`. It does not bind the value to an `ActionId`. It does not check conflicts (except optionally for display).

`KeybindEditor` is to `BindingRegistry` what `TextField` is to `AppState`: a widget that edits a value. What is done with the value — and where it is stored — is the caller's concern.

## 1.2 What this SPEC governs

This SPEC defines:

- The widget's role and value type.
- The new keyboard widget category — Category K — that `KeybindEditor` declares; the corresponding amendment to SPEC-HAL-328 §11.
- The one-shot capture state machine, including modifier tracking and the commit / cancel paths.
- The public API surface.
- Display semantics (canonical formatting reuse).
- The optional conflict-surfacing callback.
- The set of framework-reserved keys that cannot be captured as bindings.
- The named invariants and forbidden behaviors that bind the implementation.
- The mapping to Halcyon's Constitution.

## 1.3 What this SPEC does not govern

- Binding registration (caller).
- Conflict resolution policy (caller; the `BindingRegistry` layer / priority model handles structural conflicts).
- Action selection ("which action does this `Shortcut` bind to?"; caller).
- Modifier-only bindings (Ctrl alone, Shift alone). Out of scope.
- Sequence bindings ("Ctrl+K Ctrl+S"). Out of scope.
- Right-click context menus, "reset to default" UI, drag-and-drop reordering of bindings. Out of scope.

## 1.4 Why this SPEC exists

Three reasons:

1. **PropertyEditor (SPEC-HAL-260A) needs an editor for `PropertyValue::KeyBinding(Shortcut)` cells.** The PE rewrite under SPEC-HAL-260A explicitly consumes `KeybindEditor` from this SPEC (§6.4 of SPEC-HAL-260A). PE does not implement the editor inline: transitional, partial, or replaced-later implementations are forbidden.
2. **Halcyon has an Action System (SPEC-HAL-020A) and a layered Binding Registry (SPEC-HAL-020C) but no UI for entering a shortcut.** This widget closes that gap.
3. **The capture semantics are subtle.** Pressing a key combination in capture mode is conceptually different from typing text or changing a value. The widget is its own category; making it a SPEC-grade declaration prevents future widgets from reinventing the one-shot capture model.

---

# 2. Dependencies

## 2.1 Constitutional

- **SPEC-HAL-002 §3.11 — No Interpretive Intent Resolution.** The captured value is verbatim. The widget does not normalize, infer, or reconcile. See §10.
- **SPEC-HAL-002 §4.11 — Explicit Commit Authority.** The commit (firing `on_change`) is triggered by exactly one explicit user action: the first non-modifier key press in capture mode. See §10.
- **SPEC-HAL-001 §2.5 — Sovereignty Boundary.** OS focus signals are advisory; the widget's capture state is engine-derived. See §10.

## 2.2 Subsystem

- **SPEC-HAL-328 — Keyboard Authority.** The widget declares its category, capture eligibility, and Escape semantics per HAL-328. The widget MUST NOT override Escape (FORBID-KBD-ESCAPE-SEMANTICS).
- **SPEC-HAL-020A — Action System.** The widget produces `Shortcut` values, the canonical type from `src/runtime/actions.rs`. Display uses `Shortcut::to_display_string()` — already documented in `actions.rs:207` as "Canonical format used by PropertyEditor keybind display, command palette, and serialization."
- **SPEC-HAL-020C — Binding Registry.** The widget does NOT call `BindingRegistry::add`, `BindingRegistry::remove_by_tag`, or any other registry method. Conflict surfacing (if enabled by the caller) reads via a caller-supplied callback; the widget does not look up the registry itself.

## 2.3 Reused infrastructure

| Subsystem | Source | Use |
|---|---|---|
| `Shortcut` type | `src/runtime/actions.rs:107` | Captured value type. |
| `Shortcut::to_display_string()` | `src/runtime/actions.rs:207` | Display formatting. |
| `KeyCode`, `Modifiers` | `src/core/event` | Input event parsing. |
| `Label` widget | `src/widgets/label.rs` | Display rendering of the formatted shortcut. |
| Capture API | framework (HAL-328) | `cx.request_keyboard_capture`, `cx.release_keyboard_capture`. |

The widget builds on these primitives. It does not duplicate the glyph atlas, the text shaper, the input parser, or the capture mechanism.

---

# 3. Value Model

## 3.1 Value type

The widget's value is `Option<Shortcut>`:

- `Some(shortcut)` — the widget holds a bound shortcut. Displayed via `shortcut.to_display_string()`.
- `None` — the widget holds no binding. Displayed via a caller-configurable placeholder string (default: empty or a "—" glyph).

The value can be set programmatically (via the builder API or a setter); the value can be cleared programmatically (`set_value(None)`). The widget itself never sets the value to `None` — the only widget-internal transition is capture-to-`Some` on a successful one-shot capture.

## 3.2 Reservation: framework-reserved keys

The following keys cannot be captured as bindings via this widget under their listed conditions; if pressed in capture mode they do not commit:

- **Escape.** The framework unconditionally releases capture (SPEC-HAL-328 §3.2). The widget does NOT intercept Escape; the framework's release path is followed; no commit occurs. See §6.2.
- **Tab and Shift+Tab.** The framework's `release_and_next` / `release_and_prev` path (SPEC-HAL-328 §3.7) handles these in capture; the widget returns `false` (does not consume), the framework releases capture and moves focus. No commit occurs.
- **Backspace alone (no modifiers held).** Reserved as the **clear** action: `value` is set to `None`, `on_change(None)` fires, capture is released programmatically. With any modifier held (`Ctrl+Backspace`, `Alt+Backspace`, etc.), Backspace is a normal non-modifier key and is captured as a binding via the standard path. See §5.2 / §5.5.

These limitations are documented as part of the widget's public contract. A future SPEC could introduce a "raw capture" sub-mode for binding Escape, Tab, or unmodified Backspace; that is out of scope.

## 3.3 What the widget does NOT capture

- Modifier-only patterns. A press of `Ctrl` alone (with no subsequent non-modifier key while held) does not commit. Modifier keys are tracked internally as state; commit requires a non-modifier key while the modifier(s) are held.
- Key Up events. The widget commits on Key Down of the non-modifier key, not on Up. Up events are observed only to maintain the internal modifier-state tracker.
- Held / repeating keys. The first Key Down of a non-modifier key commits; subsequent repeats in the same capture session do not re-commit (capture has already been released programmatically).

---

# 4. Widget Category Declaration

`KeybindEditor` is **Category K — Single-Stroke Capture**. This is a new category, added to SPEC-HAL-328 §11.

## 4.1 Declarations

```rust
fn focusable(&self) -> bool                  { true }
fn capture_eligible(&self) -> bool           { true }
fn defines_focus_scope(&self) -> bool        { false }
fn accepts_keyboard_activation(&self) -> bool { false }
```

## 4.2 Distinction from existing categories

| Category | Capture model | Why KeybindEditor isn't this |
|---|---|---|
| A (Activation) | No capture; i/Enter/Space activates inline. | KeybindEditor must capture to receive arbitrary keystrokes; activation alone is insufficient. |
| C (Value-Editing: Slider, SpinBox, TimePicker) | Capture mode; arrows change values; many keystrokes during capture; explicit commit. | KeybindEditor commits on a single keystroke, not on Enter after multi-key adjustment. |
| D (Text-Input: TextField, TextArea, TagInput) | Capture mode; every key is text input; cursor / Backspace / Delete; explicit commit on Enter. | KeybindEditor has no text cursor, no edit buffer, no Backspace handling. Keys are not text — they are the value. |
| E (Popup: ComboBox, DatePickerField, ColorPickerField) | Capture opens a popup; arrows navigate popup items; commit closes popup. | KeybindEditor has no popup; no item list to navigate. |

Category K's distinguishing feature: **one-shot.** Capture entry → first non-modifier Key Down → commit + release. No multi-step interaction during capture.

---

# 5. Capture State Machine

## 5.1 States

The widget operates in two states, mirroring the framework's keyboard authority model:

- **Idle / Navigation.** No capture. The widget displays its current `value`.
- **Capturing.** The widget owns the keyboard. The widget displays a capture affordance (default: a placeholder "Press shortcut…" string; configurable).

## 5.2 Transitions

| From | Event | To | Side effects |
|---|---|---|---|
| Idle | `i` / Enter (Navigation-mode activation) | Capturing | Framework engages capture; widget's `on_capture_enter` fires; widget initializes modifier-state tracker = empty; widget's display flips to capture affordance. |
| Idle | Pointer Down on widget (Primary button) | Capturing | Widget calls `cx.request_keyboard_capture()` and returns `true` (TextField pattern, SPEC-HAL-328 §3.1 pointer path). Framework routes the request through the engine action chain; `on_capture_enter` fires; modifier tracker initializes empty. |
| Capturing | Key Down: modifier key (Ctrl / Alt / Shift / Meta only) | Capturing | Update internal modifier-state tracker. No commit. |
| Capturing | Key Up: modifier key | Capturing | Update internal modifier-state tracker. No commit. |
| Capturing | Key Down: `Backspace` with NO modifiers held | Idle | Clear: `value = None`; fire `on_change(None)`; clear `last_conflict`; programmatically `cx.release_keyboard_capture`. See §5.5 / INV-KBE-BACKSPACE-CLEARS. |
| Capturing | Key Down: non-modifier key (letter, digit, F-key, arrow, navigation key, `Enter`, `Space`, `Delete`, `Insert`, or `Backspace` with at least one modifier held — see §5.4) | Idle | Build `Shortcut { key, ctrl, alt, shift, meta }` from event + tracker; `value = Some(shortcut)`; fire `on_change(Some(shortcut))`; call optional `conflict_check`; programmatically `cx.release_keyboard_capture`. Framework's `on_capture_leave` fires; widget's display flips back to formatted shortcut. |
| Capturing | Escape | Idle | Framework unconditionally releases (HAL-328 §3.2). Widget's modifier-state tracker is discarded. `value` is unchanged. `on_change` does NOT fire. |
| Capturing | Tab / Shift+Tab | Idle (focus moves) | Widget returns `false` from key handler; framework's `release_and_next` / `release_and_prev` (HAL-328 §3.7) executes. `value` is unchanged. `on_change` does NOT fire. |
| Capturing | PE-side forced unmount (host calls `cancel`) | Idle | Capture released; `value` unchanged; `on_change` does NOT fire. |

## 5.3 Why Escape and Tab implicitly "cancel"

Both Escape and Tab cause capture exit without the widget having captured any non-modifier key. The widget's value is not mutated during capture — it is mutated only by the explicit commit path on a non-modifier Key Down. Therefore Escape and Tab leave `value` unchanged, which is the user-facing "cancel" semantic. The widget does NOT intercept Escape or Tab — both are owned by the framework per SPEC-HAL-328 §3.2 and §3.7. The cancel-by-emergence pattern satisfies FORBID-KBD-ESCAPE-SEMANTICS.

## 5.4 Non-modifier key definition

A non-modifier key is any `KeyCode` that is NOT `Ctrl`, `Alt`, `Shift`, `Meta`, or any platform-specific modifier variant. Concretely:

- Letters: `KeyCode::Character('a'..='z' | 'A'..='Z' | '0'..='9' | any unicode)` — yes.
- Function keys: `F1..F12` — yes.
- Navigation: `Left`, `Right`, `Up`, `Down`, `Home`, `End`, `PageUp`, `PageDown` — yes.
- Whitespace: `Space`, `Enter` — yes (but see §2.2 framework-reserved discussion for Enter).
- Editing: `Delete`, `Insert` — yes. `Backspace` — **special**: see §5.5 (clear action when no modifiers held; normal binding capture when at least one modifier held).
- Reserved: `Escape`, `Tab` — see §2.2; framework intercepts; not capturable.

`Enter` is capturable as a binding via this widget. The widget commits on Enter (Key Down) by building a `Shortcut { key: KeyCode::Enter, … }`. This means Enter cannot serve as a "second-press to confirm" mechanism in capture mode — there is no two-step model.

## 5.5 Backspace clear behavior

`Backspace` with NO modifiers held is reserved as the **clear** action. In capture mode, pressing unmodified Backspace:

1. Sets `value = None`.
2. Fires `on_change(None)`.
3. Clears `last_conflict`.
4. Programmatically releases capture.

This is an explicit user command and satisfies INV-KBE-EXPLICIT-COMMIT — the `on_change` callback fires because the user pressed Backspace, a deliberate input. The clear path is distinct from the Escape / Tab cancellation paths (which leave `value` unchanged): the clear path mutates `value` to `None` and surfaces the change to the caller.

`Backspace` with any modifier held (`Ctrl+Backspace`, `Alt+Backspace`, `Shift+Backspace`, `Meta+Backspace`, or any combination) is a normal non-modifier key and is captured as a binding via the standard non-modifier Key Down path (§5.2). This preserves the ability to bind modified-Backspace combinations to actions while reserving the unmodified case for clearing.

The trade-off: `Backspace` alone cannot be bound as a single-key shortcut via this widget. This is a documented limitation of Category K, parallel to the Escape / Tab limitations (§3.2).

---

# 6. Display

## 6.1 Idle display

When in Idle state, the widget renders:

- If `value == Some(shortcut)`: the result of `shortcut.to_display_string()` (e.g., `"Ctrl+Shift+S"`, `"F4"`, `"Enter"`).
- If `value == None`: the configured placeholder string (default empty; commonly `"—"` or `"Unbound"` per caller).

Rendering is via a `Label` child widget. The widget does not implement its own text layout — text shaping and glyph atlas operations are entirely the `Label`'s concern.

## 6.2 Capturing display

When in Capturing state, the widget renders:

- The configured capture-prompt string (default `"Press shortcut…"`).

The capture affordance replaces the value display for the duration of the capture session. On capture exit (commit or cancel), the display flips back to the value (which may have changed via commit, or remained the same via cancel).

## 6.3 Focus and capture visual indicators

Per SPEC-HAL-328 §12:

- **Focused:** 1px border in `cx.theme.colors.borders.focus`.
- **Captured:** 2px border in `cx.theme.colors.borders.focus`.

The widget reuses the framework's `paint_focus_ring` and capture-indicator paint helpers. No custom focus visual.

## 6.4 Conflict display (optional)

If the caller has supplied a `conflict_check` callback (§7), the widget calls it immediately after a successful commit (after `value = Some(shortcut)` is set, after `on_change` fires). If the callback returns `Some(conflict_info)`, the widget renders a secondary line below or adjacent to the value display showing the conflict (e.g., `"Already bound to: Save"`). The conflict display is paint-only; it does NOT block the commit, does NOT call back into the framework, does NOT mutate any state.

`ConflictInfo` is a caller-defined display string carrier:

```rust
pub struct ConflictInfo {
    pub message: String,
}
```

The widget displays `message` verbatim. The widget does not interpret `ConflictInfo` semantics.

## 6.5 Displacement (caller pattern, non-normative)

Some applications enforce that a given `Shortcut` is bound to at most one action at a time: when the user binds a shortcut already bound elsewhere, the prior binding is cleared. The widget does NOT implement this — displacement is purely a caller pattern, composed from the existing public API.

Canonical pattern:

1. Caller maintains an external binding model (e.g., `Rc<RefCell<[Option<Shortcut>; N]>>` keyed by editor index, or `HashMap<EditorId, Option<Shortcut>>`).
2. Each editor's `on_change(Some(s))` handler:
   - Scans the model for any other entry whose value equals `s`. For each found, sets that entry to `None`.
   - Sets the calling editor's entry to `Some(s)`.
   - Triggers a rebuild (e.g., by setting a dirty flag the Screen's `take_rebuild_flag` consumes).
3. On the next layout cycle, the Screen rebuilds the widget tree from the updated model. Each `KeybindEditor` is instantiated with `.with_value(model[i])` — the displaced editor mounts with `None` and renders its placeholder.

Alternative without rebuild: caller holds widget references and calls `set_value(None)` on the displaced editor directly. `set_value` is silent (does NOT fire `on_change`) per §7.2, so this does not cascade.

This pattern is distinct from §6.4 conflict-warning display: §6.4 surfaces information about a conflict (no state change); displacement resolves a conflict by mutating state.

See `examples/keybind_editor.rs` for a working demonstration using the rebuild approach.

---

# 7. Public API

## 7.1 Constructor and builders

```rust
impl KeybindEditor {
    pub fn new() -> Self;

    pub fn with_value(self, value: Option<Shortcut>) -> Self;
    pub fn with_placeholder(self, placeholder: impl Into<String>) -> Self;
    pub fn with_capture_prompt(self, prompt: impl Into<String>) -> Self;
    pub fn with_conflict_check(
        self,
        check: impl Fn(Shortcut) -> Option<ConflictInfo> + 'static,
    ) -> Self;
    pub fn on_change(
        self,
        callback: impl Fn(Option<Shortcut>) + 'static,
    ) -> Self;
}
```

## 7.2 Setters and accessors

```rust
impl KeybindEditor {
    pub fn value(&self) -> Option<Shortcut>;
    pub fn set_value(&mut self, value: Option<Shortcut>);

    /// Programmatically request capture exit without committing.
    /// Used by host widgets (e.g., PropertyEditor's forced-unmount path)
    /// to cancel an in-flight capture session.
    pub fn cancel(&mut self);
}
```

`set_value` updates the value silently — it does NOT fire `on_change`. `on_change` fires exclusively from user-driven commits.

`cancel` is a host-side hook: when an outer widget needs to unmount the `KeybindEditor` mid-capture (e.g., PE's `EditExitReason::ForcedCancel`), the host calls `cancel`. The widget releases capture and resets its capture-internal state without mutating `value`. `on_change` does not fire.

## 7.3 Wiring with `Widget` trait

`KeybindEditor` implements the framework's `Widget` trait. The standard methods (`layout`, `paint`, `handle_event`, `on_capture_enter`, `on_capture_leave`, `id`, `focusable`, `capture_eligible`, etc.) are implemented per the declarations in §4.1 and the state machine in §5.

`handle_event`:

- In Navigation mode (`cx.keyboard_mode == Navigation`): returns `false` for all key events — framework handles navigation. On Pointer Down with the Primary button, calls `cx.request_keyboard_capture()` and returns `true` (TextField pattern); the framework routes through the engine action chain and dispatches `on_capture_enter`.
- In Capture mode (`cx.keyboard_mode == Capture`): processes Key events per §5.2. Returns `true` when consuming a non-modifier Key Down (commit path) or unmodified Backspace Key Down (clear path, §5.5); returns `true` while tracking modifier Key Down / Up; returns `false` for Tab / Shift+Tab (framework handles); returns `false` for Escape (framework handles).

---

# 8. PropertyEditor Integration

(Normative reference; the PE side of the contract is governed by SPEC-HAL-260A.)

SPEC-HAL-260A's `editor_registry.rs` mounts a `KeybindEditor` for `PropertyValue::KeyBinding(Shortcut)` cells:

```rust
PropertyValue::KeyBinding(s) => Box::new(make_keybind_editor(*s, cell_key, on_exit)),
```

Where `make_keybind_editor` wires the editor's `on_change` to PE's `EditExitReason` dispatch:

- `on_change(Some(shortcut))` → `on_exit(EditExitReason::Commit, Some(PropertyValue::KeyBinding(shortcut)))`.
- Programmatic `cancel` (PE-side forced unmount) → `on_exit(EditExitReason::ForcedCancel, None)`.

The widget is consumed by PE; PE never reaches into the `BindingRegistry` to register the captured value. The consumer's implementation of `PropertyDataSource::commit_cell` decides what to do with a `PropertyValue::KeyBinding` — if the consumer wants the new binding to take effect at runtime, the consumer's `commit_cell` body calls `BindingRegistry::add(Binding { … layer: User, … })`. This is the consumer's authority, not PE's, and not `KeybindEditor`'s.

---

# 9. Invariants

**INV-KBE-ONE-SHOT.** A successful commit fires `on_change` exactly once per capture session. After commit, capture is released; the widget returns to Idle. A second commit requires a new capture session (the user re-enters capture).

**INV-KBE-NO-OVERRIDE-ESCAPE.** The widget does NOT intercept Escape in capture mode. Escape is owned by the framework per SPEC-HAL-328 §3.2 / FORBID-KBD-ESCAPE-SEMANTICS. The widget's "cancel on Escape" behavior is emergent — Escape releases capture before any non-modifier key was captured; `value` is unchanged because the commit path did not execute.

**INV-KBE-NO-OVERRIDE-TAB.** The widget does NOT intercept Tab / Shift+Tab in capture mode. These keys are owned by the framework per SPEC-HAL-328 §3.7. The widget returns `false` for them; the framework's `release_and_next` / `release_and_prev` runs.

**INV-KBE-VERBATIM-COMMIT.** The committed `Shortcut` is built directly from the Key Down event and the current modifier-state tracker. The widget does not normalize, transform, lowercase, alias, or reinterpret the captured combination. (Reaffirms SPEC-HAL-002 §3.11.)

**INV-KBE-EXPLICIT-COMMIT.** The commit (`on_change` firing) is triggered by exactly one explicit user action: a non-modifier Key Down in capture mode. No other trigger commits. Programmatic `set_value` does not commit. OS-deactivation does not commit. PE-side forced unmount does not commit. (Reaffirms SPEC-HAL-002 §4.11.)

**INV-KBE-NO-REGISTRY.** The widget does NOT read from or write to `BindingRegistry`. Conflict surfacing (§6.4) is via a caller-supplied callback only.

**INV-KBE-DISPLAY-CANONICAL.** Value display uses `Shortcut::to_display_string()` verbatim. The widget does not implement its own formatting. If the format is changed in `actions.rs`, this widget's display tracks the change automatically.

**INV-KBE-SOVEREIGNTY.** The widget's capture state is engine-derived. OS-level focus or activation signals do not trigger commit, cancel, or capture state changes beyond what the framework's normal capture / blur path delivers. (Reaffirms SPEC-HAL-001 §2.5.)

**INV-KBE-BACKSPACE-CLEARS.** In capture mode, `Backspace` Key Down with no modifiers held mutates `value` to `None`, fires `on_change(None)`, clears `last_conflict`, and releases capture programmatically. `Backspace` with at least one modifier held is a normal non-modifier capture per the standard §5.2 commit path. This is the sole user-driven path that sets `value` to `None`; programmatic `set_value(None)` does not fire `on_change` (INV-KBE-EXPLICIT-COMMIT preserved — Backspace clear IS an explicit user action).

---

# 10. Forbidden Behaviors

**FORBID-KBE-NORMALIZATION.** The widget MUST NOT case-normalize letter keys, MUST NOT alias keys (e.g., treat `Return` as `Enter` silently), MUST NOT collapse equivalent modifier combinations, MUST NOT substitute "the closest match" for ambiguous input. The `Shortcut` is built from the event verbatim.

**FORBID-KBE-REGISTRY-ACCESS.** The widget MUST NOT call `BindingRegistry::add`, `BindingRegistry::remove_by_tag`, `BindingRegistry::resolve`, `BindingRegistry::lookup`, or any other method on the registry. Conflict-check callbacks supplied by the caller MAY look up the registry (caller's domain), but the widget itself does not.

**FORBID-KBE-OS-COMMIT.** The widget MUST NOT commit (fire `on_change`) in response to OS signals (window deactivation, application switching, focus loss to other windows). The commit path is exclusively the in-capture non-modifier Key Down event.

**FORBID-KBE-MULTI-COMMIT.** A single capture session MUST NOT fire `on_change` more than once. Implementation: after the commit path runs, capture is released; the widget's state transitions back to Idle; subsequent events route through framework navigation paths.

**FORBID-KBE-ESCAPE-OVERRIDE.** The widget MUST NOT consume Escape (must NOT return `true` from `handle_event` for Escape in capture mode). Doing so would violate SPEC-HAL-328 FORBID-KBD-ESCAPE-SEMANTICS.

**FORBID-KBE-TAB-OVERRIDE.** The widget MUST NOT consume Tab or Shift+Tab in capture mode. The framework's auto-capture-release path (HAL-328 §3.7) handles them.

**FORBID-KBE-MODIFIER-ONLY-COMMIT.** Releasing a modifier key (e.g., letting go of Ctrl with no subsequent non-modifier press during the hold) MUST NOT commit. Modifier-only bindings are out of scope.

---

# 11. Amendment to SPEC-HAL-328 §11 — Add Category K

This SPEC introduces a new keyboard widget category. SPEC-HAL-328 §11 is amended by appending the following subsection (the amendment may be applied directly to SPEC-HAL-328 via a documentation CC task, or rolled into a `SPEC-HAL-328A` amendment SPEC per the project's convention for letter-suffix amendments).

> ## §11.K Category K: Single-Stroke Capture
>
> Capture mode is entered for the purpose of binding one keystroke. The first non-modifier Key Down captures the combination as a value and releases capture programmatically.
>
> **Widgets:** KeybindEditor (SPEC-HAL-329)
>
> | Declaration                     | Value |
> |---------------------------------|-------|
> | `capture_eligible()`            | true  |
> | `defines_focus_scope()`         | false |
> | `accepts_keyboard_activation()` | false |
>
> Navigation: ALL keys → return false (framework navigates). `i` / Enter → enter Capture. Capture: modifier Key Down/Up → tracked internally; non-modifier Key Down → build value, fire callback, release capture programmatically. Escape → framework releases unconditionally (§3.2); value unchanged. Tab / Shift+Tab → return false; framework handles (§3.7); value unchanged.
>
> **handle_event MUST** check `cx.keyboard_mode == Capture` before treating any key as a binding intent.
>
> **Escape and Tab are not capturable as bindings via Category K widgets.** Both are framework-reserved per §3.2 and §3.7. A future SPEC could introduce a "raw capture" sub-mode to lift this restriction; until then, this is a documented limitation of Category K.

The above text is the exact insertion proposed for SPEC-HAL-328 §11 (appended after §11.6 Category F or wherever the categorical ordering of the SPEC places it).

---

# 12. Out of Scope

- **Binding registration.** Caller's responsibility.
- **Conflict resolution policy** (override / reject / layer-based override). Caller's responsibility; the `BindingRegistry`'s priority + layer model handles structural conflicts (SPEC-HAL-020C §B.2).
- **Action selection.** The widget edits a `Shortcut`; it does not associate the shortcut with an `ActionId`. The caller's UI (settings screen, PE consumer) handles association.
- **Modifier-only bindings** (Ctrl alone, Shift alone). Requires a separate Category K variant or a flag; out of scope.
- **Sequence bindings** (chord: `Ctrl+K Ctrl+S`). Would require a multi-step capture model; out of scope.
- **Raw capture mode** that allows binding Escape, Tab. Out of scope.
- **Reset-to-default UI, right-click context menu, drag-and-drop reordering, conflict-resolution dialogs.** All are caller's UI surfaces.
- **Theming beyond `cx.theme.colors.borders.focus` for focus / capture indicators.** Standard theme tokens only.
- **Accessibility (screen reader announcements, AT integration).** Deferred to a future SPEC covering accessibility for capture widgets generally.

---

# 13. Constitutional Compliance Citations

## 13.1 SPEC-HAL-001 §2.5 — Sovereignty Boundary

- **INV-KBE-SOVEREIGNTY** (§9): capture state is engine-derived; OS signals do not trigger commit, cancel, or capture transitions beyond the framework's normal capture lifecycle.

## 13.2 SPEC-HAL-002 §3.11 — No Interpretive Intent Resolution

- **INV-KBE-VERBATIM-COMMIT** (§9): the `Shortcut` is built directly from the Key Down event and modifier-state tracker; no normalization or interpretation.
- **FORBID-KBE-NORMALIZATION** (§10): explicit prohibition on case-normalization, key aliasing, modifier-combination collapsing, closest-match substitution.

## 13.3 SPEC-HAL-002 §4.11 — Explicit Commit Authority

- **INV-KBE-EXPLICIT-COMMIT** (§9): commit fires exclusively from a non-modifier Key Down in capture mode; programmatic and OS-originated paths do not commit.
- **FORBID-KBE-OS-COMMIT** (§10): OS deactivation, application switching, focus loss to other windows do not trigger commit.

## 13.4 SPEC-HAL-328 §3.2 — Escape Semantics

- **INV-KBE-NO-OVERRIDE-ESCAPE** (§9), **FORBID-KBE-ESCAPE-OVERRIDE** (§10): the widget does not intercept Escape; the framework's unconditional release path runs; cancel-on-Escape is emergent.

## 13.5 SPEC-HAL-328 §3.7 — Capture Tab Path

- **INV-KBE-NO-OVERRIDE-TAB** (§9), **FORBID-KBE-TAB-OVERRIDE** (§10): the widget does not intercept Tab / Shift+Tab; the framework's `release_and_next` / `release_and_prev` path runs.

## 13.6 SPEC-HAL-020A — Action System

- **INV-KBE-DISPLAY-CANONICAL** (§9): the widget displays via `Shortcut::to_display_string()`; format alignment with the Action System and command palette is automatic.

## 13.7 SPEC-HAL-020C — Binding Registry

- **INV-KBE-NO-REGISTRY** (§9), **FORBID-KBE-REGISTRY-ACCESS** (§10): the widget is a producer of `Shortcut` values; the registry is mutated exclusively by the caller using the captured value.

---

**End of SPEC-HAL-329.**


## Scoped physical recording — 2026-10-10

**Author:** JC. The delegated technical reviewer accepts an explicit construction-time physical-key
choice on the existing KeybindEditor. Implementation is a 0.211.0-beta source
candidate with focused executor proof recorded in PROJECT-ENTRY. Independent
source acceptance remains pending; all other scope and the document-wide status
remain unchanged.

`Shortcut` uses the canonical logical/physical `BindingKey` discriminator. Existing
constructors and the editor default stay logical. Physical recording stores the
actual position, never a character substitute. Modifier Down/Up are observed, but
the first nonrepeat nonmodifier Down's authoritative modifier snapshot determines
the value, including modifiers held before Capture and distinct Ctrl/Meta.
Unknown selected identity reports `KeyRecordingUnavailable`, produces no value or
commit callback, and releases through the existing Capture owner. Escape, Tab and
unmodified Backspace keep their existing laws. Equivalent repeated or trailing
events cannot issue a second commit. `on_unavailable` is a typed recording failure,
not an action or registry mutation.

**Observable-boundary decision (the delegated technical reviewer, 2026-10-10; Author JC):** The physical
choice uses the shared checked physical-response admission. Reserved/unsupported
input is reported through the existing typed `on_unavailable` callback with no
value/commit. An observed reserved interruption reports its reason before the
existing Capture leave/cancel. Unobserved OS input cannot be recorded or fabricated;
there is no automatic re-entry on focus return. Ordinary logical recording,
Escape/Tab, Backspace clear and the one-shot lifetime remain unchanged.
