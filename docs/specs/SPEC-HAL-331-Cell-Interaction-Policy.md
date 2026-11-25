# SPEC-HAL-331: Cell Interaction Policy

### *Per-Cell Mount-Strategy and Display-Mode Capability Knobs for PropertyEditor, with Universal Click-Routing Precedence Under Capture*

**Author:** JC
**Status:** Draft
**Extends:** SPEC-HAL-260A §11 (PropertyEditorPolicy)
**Depends on:**

- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-260A (PropertyEditor) — §6.2 always-mounted Checkbox, §6.3 mount-on-demand editors, §11 interaction policy, §14.2 layout/paint phase
- SPEC-HAL-315 (Read-Only Text Selection) — for `DisplaySelection::Selectable` substrate; Phase 3 Label integration (`4d5a14fe`, `ae46725d`, `14880e9e`) is the working implementation
- SPEC-HAL-328 (Keyboard Authority, Focus, and Capture)

---

# 1. Purpose

SPEC-HAL-260A §11 defines PE's PE-instance-wide interaction policy: one `EditTrigger`, one `PointerDownAction`, one `EditDisposition` for blur/click-elsewhere. The policy is correct as a default-setter for an entire PE instance, but it does not address three concerns:

**1. Mount strategy.** PE's current model mounts the editor on edit-begin and unmounts on commit/cancel — the *display-then-edit* pattern (SPEC-HAL-260A §6.3). Some cells should never have a display widget at all: the editor IS the cell at all times. Forms, configuration panels, transient parameter input — the user expects every interactive field to be live, not gated behind a double-click. SPEC-HAL-260A §6.2 already implements this for `Bool` cells (always-mounted Checkbox); this SPEC generalizes the pattern.

**2. Display-mode capability.** When a cell is in display mode, the display widget might be inert text (read-only label) or it might support text selection (so the user can copy the displayed value without entering edit mode). SPEC-HAL-315 provides the selection substrate (Label integration landed in `4d5a14fe`); this SPEC routes it as a per-cell policy knob.

**3. Click-routing under capture.** The enclosing PE Body (and, by extension, any tabular consumer like TreeView) has its own pointer-Down gesture model: row selection, column cursor, double-click edit trigger. Embedded interactive widgets (TextField caret placement, SpinBox +/- click, ComboBox popup toggle, ColorPickerField swatch click) have their own pointer-Down semantics. The two compete. Without an explicit precedence rule, parent gestures can interfere with editor gestures while the user is mid-edit — a click meant to place a caret could be reinterpreted as a row-selection re-tap or re-trigger of edit-begin.

This SPEC introduces both new policy axes (mount strategy, display selection) as per-cell variation overriding the PE-instance default through a deterministic resolution chain, AND a universal click-routing precedence rule that suppresses PE Body's gesture interpretation for cells whose editor is currently mounted in edit state.

# 2. Design Principles

**DP-1: Layered resolution.** Each cell resolves its `CellInteractionPolicy` through a deterministic chain: PE-instance default → column default → cell-specific override → ReadOnly forced override. First non-`None` value wins. ReadOnly is the only forced override; everything else is a precedence cascade.

**DP-2: Three mount strategies, two display capabilities — orthogonal.** Mount strategy (`DisplayOnly` / `DisplayEditable` / `AlwaysEditor`) is one axis. Display-widget selection capability (`Inert` / `Selectable`) is a separate axis. They compose freely. `AlwaysEditor` cells have no display widget, so their selection axis has no effect; `DisplayOnly` cells may be selectable for copy-without-edit; `DisplayEditable` cells can be either.

**DP-3: Edit state is a per-cell state machine.** Each cell is in exactly one of two states: **display** or **edit**. Transitions are: display → edit on user trigger (per `EditTrigger`); edit → display on commit/cancel. `AlwaysEditor` cells are permanently in edit state. The state determines click routing (DP-4).

**DP-4: Click-routing follows cell state, universally.** When a cell is in **display state**, clicks on its bounds are interpreted by PE Body per its gesture model (row selection, column cursor, edit trigger). When a cell is in **edit state**, clicks on its bounds are EXCLUSIVELY the editor's domain — PE Body's gesture model is fully suspended for that cell. This is universal across editor types; the editor's per-type click semantics (caret placement, +/- click, popup toggle, etc.) operate without parent interference.

**DP-5: Extend, do not collide.** SPEC-HAL-260A §11's `PropertyEditorPolicy` stays as-is. The existing `edit_trigger` field becomes the PE-instance default for cells that resolve to `DisplayEditable`. The new `cell_policy_default` field is additive.

**DP-6: Reuse SPEC-HAL-315.** `DisplaySelection::Selectable` maps to `TextSelectionPolicy::OnCapture` on Label. PE Body constructs the display widget with the derived policy; SPEC-HAL-315's substrate (already in production) handles all gesture, highlight, and clipboard concerns.

**DP-7: AlwaysEditor cells are not edit sessions.** `AlwaysEditor` cells participate in the row's normal materialized structure — the editor is the cell's permanent child, not a transient overlay. SPEC-HAL-260A §6.3 (editor mount-on-demand) and §7 (`edit_session`) do not apply to them. The §6.2 always-mounted Checkbox pattern is the precedent.

---

# 3. Scope

## 3.1 In Scope

- `CellInteractionPolicy` struct (cell-level policy carrier).
- `CellMountStrategy` enum: `DisplayOnly` / `DisplayEditable` / `AlwaysEditor`.
- `DisplaySelection` enum: `Inert` / `Selectable`.
- §5 resolution layering: PE-instance → column → cell → ReadOnly forced override.
- §6.1 cell-materialization branch on `mount_strategy`.
- **§6.2 click-routing precedence rule — universal suppression of PE Body gesture dispatch for cells in edit state.**
- §6.3 pointer-Down policy branches for cells in display state.
- §6.4 `AlwaysEditor` Escape contract (separate from the Phase B `DisplayEditable` capture-leave observation work).
- §7 integration with SPEC-HAL-260A §11's `EditTrigger`: `DisplayEditable` cells take their effective trigger from the PE-instance `edit_trigger`.
- §7 integration with SPEC-HAL-315: `DisplaySelection::Selectable` → `TextSelectionPolicy::OnCapture` at display-widget construction.

## 3.2 Out of Scope

- Per-row policy. Policy attaches to (column, cell-kind), not to rows.
- Cross-cell selection (selecting text across multiple cells). Per SPEC-HAL-315 FORBID-315-4, selection is single-widget.
- Generalization to ListView / TreeView cells. Their cell models differ; this SPEC governs PE only. Future tabular-substrate-level SPEC may lift the click-routing precedence pattern, which is genuinely substrate-universal.
- Per-cell `EditTrigger` override. Reserved for future extension via an `Option<EditTrigger>` field on `CellInteractionPolicy`.
- `PropertyValue::ReadOnly` archetype redesign. This SPEC interacts with ReadOnly via the §5 forced override; it does not modify the archetype itself.
- Click suppression based on framework keyboard-capture target. This SPEC uses PE-state (`edit_session` matches hit cell, or cell is `AlwaysEditor`) as the gating condition, not the framework's keyboard-capture target. PE-state is the semantically correct level — `AlwaysEditor` cells suppress dispatch even when not actively focused, and `DisplayEditable` cells suppress for the brief lag between editor mount and capture grant.

---

# 4. Types

## 4.1 CellInteractionPolicy

```rust
/// Per-cell interaction policy. Resolved at cell-render time from the
/// layering chain (§5). Carries the mount strategy and the display-mode
/// capability axis.
///
/// SPEC-HAL-331 §4.1
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CellInteractionPolicy {
    pub mount_strategy: CellMountStrategy,
    pub display_selection: DisplaySelection,
}

impl Default for CellInteractionPolicy {
    fn default() -> Self {
        Self {
            mount_strategy: CellMountStrategy::DisplayEditable,
            display_selection: DisplaySelection::Inert,
        }
    }
}
```

## 4.2 CellMountStrategy

```rust
/// How a cell transitions between display and edit states.
///
/// SPEC-HAL-331 §4.2
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CellMountStrategy {
    /// Cell shows display widget only. Edit-begin gestures are rejected.
    /// Functionally equivalent to ReadOnly for that cell. The cell is
    /// permanently in display state (§3 DP-3); PE Body's gesture model
    /// applies to clicks on the cell's bounds.
    DisplayOnly,

    /// Cell shows display widget by default; user gesture (per the
    /// PE-instance `edit_trigger`) escalates to editor mount. The cell
    /// is in display state by default and transitions to edit state
    /// on edit-begin. Click routing follows §6.2 precedence: parent's
    /// gesture model applies in display state; editor owns clicks
    /// exclusively in edit state.
    DisplayEditable,

    /// Cell IS the editor widget at all times. No display widget mounts.
    /// The editor child is part of the row's materialized structure for
    /// the cell's lifetime. `edit_session` (§7) is not involved.
    /// The cell is permanently in edit state per §3 DP-3; PE Body's
    /// gesture model is unconditionally suppressed for clicks on the
    /// cell's bounds per §6.2.
    /// Used for forms / configuration panels where every cell is
    /// permanently interactive. Generalizes the SPEC-HAL-260A §6.2
    /// always-mounted Checkbox pattern to other types.
    AlwaysEditor,
}
```

## 4.3 DisplaySelection

```rust
/// What the display widget exposes when the cell is in display state.
/// Independent of mount strategy: `AlwaysEditor` cells have no display
/// widget, so this axis has no effect for them.
///
/// SPEC-HAL-331 §4.3
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DisplaySelection {
    /// Display text is non-selectable. Pointer interaction routes
    /// directly to cell-mode gestures (focus, edit-begin per trigger).
    /// Default.
    Inert,

    /// Display text supports SPEC-HAL-315 read-only text selection:
    /// drag-to-select, double-click word, triple-click line,
    /// Ctrl/Cmd+C copy, Ctrl/Cmd+A select-all, Escape clear.
    /// PE Body materializes the display widget with
    /// `TextSelectionPolicy::OnCapture` so selection is gated on the
    /// user explicitly entering capture mode on the cell —
    /// accidental drags during navigation do not start selection.
    Selectable,
}
```

---

# 5. Resolution Layering

The cell's effective `CellInteractionPolicy` is resolved at cell-materialization time through the following chain. First non-`None` value wins. Resolution is deterministic and side-effect-free.

1. **ReadOnly forced override.** If `Cell.editable` (per `CellSource::cell(row_id, column_id)`) is `false`, force `mount_strategy = DisplayOnly`. The `display_selection` field retains the value resolved through layers 2–4. ReadOnly cells can be selectable for copy.

2. **Cell-specific override.** *Reserved.* Future extension: `CellSource::cell_policy(row_id, column_id) -> Option<CellInteractionPolicy>` trait method. Not added in this SPEC; the layer is reserved so the resolution chain remains forward-compatible.

3. **Column default.** PE's column model carries an optional per-column policy: `column_policy: Option<CellInteractionPolicy>`. Set at column construction (`add_value_column_with_policy(...)`) or via a setter on `PropertyEditorCore` (`set_column_policy(column_id, policy)`). When `None`, falls through to layer 4.

4. **PE-instance default.** `PropertyEditorPolicy::cell_policy_default: CellInteractionPolicy` (new field on §11). Defaults to `CellInteractionPolicy::default()` (DisplayEditable + Inert), preserving current PE behavior for instances that don't override.

Body computes the resolved policy at cell-materialization time and uses it to drive mount strategy + display-widget construction. The resolved value is not cached in PE state (FORBID-331-1).

---

# 6. Body Integration

## 6.1 Cell-materialization branch

PE Body's cell materialization step (SPEC-HAL-260A §14.2 step 5 / step 6) gains a branch on the resolved `mount_strategy`:

| `mount_strategy` | Body's materialization action |
|---|---|
| `DisplayOnly` | Materialize display widget (Label) configured per `display_selection`. No editor mount. Edit-trigger handling disabled. Cell is permanently in display state. |
| `DisplayEditable` | Materialize display widget. Current SPEC-HAL-260A §6.3 lifecycle applies. On `EditIntent::Begin`, mount editor — cell transitions to edit state. On commit/cancel, unmount editor and restore display widget — cell transitions back to display state. |
| `AlwaysEditor` | Materialize editor widget directly as the cell child. No display widget ever mounts. The editor's `on_exit` callback commits the value to the data source via `CellSource::commit_cell`. Editor is NOT unmounted on commit/cancel — Commit writes through and stays mounted; Cancel reverts the editor's internal state to the cell's current value and stays mounted. Cell is permanently in edit state. |

## 6.2 Click-routing precedence (universal)

**Before evaluating any policy branch (§6.3), PE Body's pointer-Down handler MUST check whether the hit cell is currently in edit state. A cell is in edit state if any of:**

- **The cell's resolved `mount_strategy = AlwaysEditor`.** AlwaysEditor cells are permanently in edit state per §3 DP-3 / §4.2.
- **The cell's resolved `mount_strategy = DisplayEditable` AND `edit_session.is_some()` AND the session's `(row_id, column_id)` matches the hit cell.** DisplayEditable cells transition into edit state when their editor mounts; the session tracks the transition.

**When the hit cell is in edit state, PE Body's pointer-Down handler emits NO intents — no `PEIntent::Selection`, no `PEIntent::Column`, no `PEIntent::Edit`. The handler returns `false` (does not consume the event). The event propagates through PE Body unchanged; the editor child (deeper hit target) processes the click per its own semantics.**

This precedence rule is universal across editor types. The per-editor click semantics inside edit state are:

| Editor type | Click semantics inside edit state |
|---|---|
| TextField (String) | Click places caret; drag selects text; double-click selects word; triple-click selects line. |
| SpinBox (Integer) | Click on +/- buttons increments/decrements (no edit-end); click on value area re-positions caret in direct-edit. |
| ComboBox (Choice) | Click on dropdown trigger toggles popup; click on option commits; click outside cell bounds (handled by framework blur, not PE) cancels. |
| ColorPickerField (Color) | Click on swatch opens picker (via popup); click on picker UI interacts with picker; click on OK/Cancel ends edit. |
| KeybindEditor (KeyBinding) | Click is irrelevant during capture — the editor consumes the next *key* event. (Backspace clears; Escape via framework path cancels — Phase B work.) |
| Bool / Checkbox | Click toggles value and commits inline. This is the AlwaysEditor degenerate case already specified by SPEC-HAL-260A §6.2. |

PE Body does not need to know any of these per-editor semantics. The precedence rule (§6.2) suppresses Body's interpretation universally; the editor's own `handle_event` handles the click per its widget contract.

## 6.3 Pointer-Down policy branches (display state only)

When §6.2's precedence rule does NOT suppress dispatch (i.e., the hit cell is in display state — `DisplayOnly` always, or `DisplayEditable` with no matching edit_session), PE Body's pointer-Down handler branches on the resolved policy:

| Cell's resolved policy | Pointer-Down behavior |
|---|---|
| `DisplayOnly`, `display_selection = Inert` | Emit Selection + Column intents (focus the cell). Never `EditIntent::Begin`. |
| `DisplayOnly`, `display_selection = Selectable` | Down routes to the display widget's selection gesture handler (SPEC-HAL-315 Phase 3 — already implemented in Label). Label opts into hit-testing via `is_selectable()`; the click is consumed by Label and never reaches PE Body's handler. PE Body emits no intents. |
| `DisplayEditable`, `edit_trigger = SingleClick` | Emit Selection + Column intents AND emit `EditIntent::Begin` from the same gesture. |
| `DisplayEditable`, `edit_trigger = DoubleClick` | Emit Selection + Column intents on Down. DoubleClick (separate event) emits `EditIntent::Begin`. |
| `DisplayEditable`, `edit_trigger = EnterOrI` or `ManualOnly` | Emit Selection + Column intents on Down. No edit-begin from pointer. |

**Cross-cell interaction during edit state.** When the user clicks a DIFFERENT cell while an edit_session is active on another cell, the hit cell is NOT the edit-state cell. §6.2's precedence does NOT apply (the precedence is hit-cell-specific). §6.3's policy branches run normally for the new hit cell. The currently-active edit_session is resolved per SPEC-HAL-260A §11's `click_elsewhere_disposition` (Commit or Cancel) — this remains a separate, existing mechanism.

## 6.4 AlwaysEditor and Escape

`AlwaysEditor` cells are not edit sessions in PE's §7 sense — there is no `edit_session` to clear on Cancel. When the framework releases capture on Escape (per SPEC-HAL-328 Capture-mode Escape handling — the engine intercepts Escape and releases capture without delivering Escape to the captured widget), the AlwaysEditor cell's editor receives `on_capture_leave` and reverts its internal state to the cell's current value (per the editor's existing cancel-on-blur semantics — see SpinBox `on_capture_leave` at `spin_box.rs:1096-1101`). The editor stays mounted. No PE-state transition fires.

This contrasts with the Phase B Escape-doesn't-release work for `DisplayEditable` editors: that work is about PE Body observing capture-leave on the active edit child and emitting `EditIntent::Cancel` to clear `edit_session`. **The Phase B mechanism applies to `DisplayEditable` mode only.** `AlwaysEditor` cells handle capture-leave entirely within the editor's own contract; PE Body does nothing.

---

# 7. PE Policy Extension

`PropertyEditorPolicy` (SPEC-HAL-260A §11 / `src/widgets/property_editor_v2/policy.rs`) gains one new field:

```rust
pub struct PropertyEditorPolicy {
    // ... existing fields unchanged (wrap_row_axis, wrap_column_axis,
    //     tab_at_row_end, tab_at_last_cell, edit_trigger,
    //     pointer_down_on_cell, hover, blur_disposition,
    //     click_elsewhere_disposition, section_row_cursor_column,
    //     header_click, empty_area_click)

    /// SPEC-HAL-331: PE-instance default cell interaction policy.
    /// Per-column overrides (and future per-cell overrides) resolve
    /// through this as the floor of the §5 resolution chain.
    pub cell_policy_default: CellInteractionPolicy,
}
```

The default value is `CellInteractionPolicy::default()` — `DisplayEditable` + `Inert`, preserving current PE behavior for instances that don't override.

The existing `edit_trigger` field stays. For cells resolved to `DisplayEditable`, the effective trigger is the PE-instance `edit_trigger`. No per-cell trigger override in this SPEC.

---

# 8. Invariants

**INV-331-1 (Single resolved policy per cell per frame):** Every cell has exactly one resolved `CellInteractionPolicy` per render frame, deterministically computed via the §5 layering chain. The resolved policy is not stored in PE state; it is recomputed at cell-materialization time from the layered sources.

**INV-331-2 (DisplayOnly blocks edit-begin):** `CellMountStrategy::DisplayOnly` cells never produce `EditIntent::Begin`. PE Body's edit-trigger dispatch (pointer Down for `SingleClick`, DoubleClick event, `i`/Enter keyboard) is gated on `mount_strategy = DisplayEditable`.

**INV-331-3 (AlwaysEditor outside edit_session):** `CellMountStrategy::AlwaysEditor` cells have no `edit_session` entry. The editor child is part of the row's materialized structure for the cell's lifetime. PE's `edit_session: Option<EditSession>` field tracks `DisplayEditable` cells only.

**INV-331-4 (Selectable graceful degradation):** `DisplaySelection::Selectable` is only honored when the cell's display widget implements SPEC-HAL-315 (`Label` today). For display widgets that don't implement the contract, the resolution degrades silently to `Inert` (no panic, no fallback widget swap). PE Body's display-widget materialization is responsible for the degradation logic.

**INV-331-5 (Resolution side-effect-free):** Computing a cell's resolved policy does not mutate PE state, the data source, or the substrate. The §5 resolution chain is pure with respect to all parties.

**INV-331-6 (ReadOnly forced override):** `CellSource::cell(row_id, column_id).editable == false` forces `mount_strategy = DisplayOnly` regardless of the column or PE-instance default. The `display_selection` axis is independent — ReadOnly cells can still be `Selectable`.

**INV-331-7 (Click-routing precedence under edit state):** PE Body's pointer-Down handler emits no intents (Selection, Column, Edit) for clicks on a cell currently in edit state. A cell is in edit state when its `mount_strategy = AlwaysEditor`, OR its `mount_strategy = DisplayEditable` AND `edit_session.is_some()` AND the session matches the hit cell. The handler returns `false` for such clicks; the event propagates to the editor child for per-widget handling. This rule takes precedence over §6.3's policy branches.

**INV-331-8 (Per-cell state determinism):** A cell is in exactly one of two states per render frame: display or edit. The state is determined by `mount_strategy` + `edit_session`:
- `mount_strategy = DisplayOnly` → display state.
- `mount_strategy = AlwaysEditor` → edit state.
- `mount_strategy = DisplayEditable` AND `edit_session` matches cell → edit state.
- `mount_strategy = DisplayEditable` AND `edit_session` does not match cell → display state.

No third state. No partial state. The state transitions are atomic at PE-state mutation boundaries (`apply_edit_intent`).

---

# 9. Forbidden Behaviors

**FORBID-331-1:** PE Body must not store the resolved cell policy in PE state. The policy is computed per cell per frame from the layering chain. Caching is a future optimization, not part of this SPEC.

**FORBID-331-2:** PE Body must not promote a `DisplayOnly` cell to `DisplayEditable` based on transient user actions (focus, hover, click, key press). The resolved policy is the authority; user gestures are evaluated against it, not the reverse.

**FORBID-331-3:** `AlwaysEditor` cells must not participate in PE's `edit_session` machinery. PE's `apply_edit_intent` handler should guard against intents that name an `AlwaysEditor` cell (defensive — under correct dispatch per §6.2 precedence, no such intent should reach this handler because pointer-Down dispatch is suppressed at the source).

**FORBID-331-4:** `DisplaySelection::Selectable` must not silently change the selection policy of a display widget that the consumer constructed explicitly. If a future API path allows consumers to supply pre-built display widgets, the consumer's choice wins. This SPEC's `Selectable` resolution applies only to display widgets that PE Body itself constructs.

**FORBID-331-5:** No global / ambient / theme-level `CellInteractionPolicy`. The PE-instance default is the floor of the resolution chain; there is no per-application or per-theme default. Every PE instance carries its own.

**FORBID-331-6:** `AlwaysEditor` cells must not request framework keyboard capture at row-materialization time. Capture is requested by user gesture (click, `i`/Enter on cell) per SPEC-HAL-328 normal flow. Auto-capturing every AlwaysEditor cell as the row materializes would create capture thrashing across many cells per frame; capture is an explicit user-intent escalation, not a side-effect of materialization.

**FORBID-331-7 (No parent gesture interpretation during edit state):** PE Body must not interpret any pointer gesture (Down, Move, Up, DoubleClick, TripleClick) on a cell in edit state as cell-mode navigation, row-selection update, column-cursor move, edit re-trigger, or any other parent-level intent. Suppression per §6.2 is total — every gesture on the cell's bounds belongs to the editor while the cell is in edit state. The only exception is the framework's keyboard-capture-release path (Escape), which is handled outside the pointer-Down channel.

**FORBID-331-8 (No silent state transitions):** Cell state transitions (display → edit, edit → display) occur ONLY via:
- `apply_edit_intent::Begin` mutating `edit_session` (display → edit, DisplayEditable).
- `apply_edit_intent::Commit` / `apply_edit_intent::Cancel` clearing `edit_session` (edit → display, DisplayEditable).
- Cell-materialization branch instantiating the editor as permanent child (AlwaysEditor — no runtime transition; the cell is born in edit state).

No other code path mutates cell state. No transient mid-frame state changes. No "tentative" edit state during pointer-Down processing.

---

# 10. Implementation Phases

**Phase 1 — Types and resolution.**
Add `CellInteractionPolicy`, `CellMountStrategy`, `DisplaySelection` to `src/widgets/property_editor_v2/policy.rs`. Extend `PropertyEditorPolicy` with `cell_policy_default: CellInteractionPolicy`. Add resolution helper (`resolved_cell_policy(row_id, column_id) -> CellInteractionPolicy`) on `PropertyEditorCore`, reading from the layered sources (ReadOnly check via CellSource, column override via PE's column model, PE-instance default via policy). Per-column override storage: `HashMap<ColumnId, CellInteractionPolicy>` on `PropertyEditorCore` as the minimum-touch starting point.

**Phase 2 — Click-routing precedence in PE Body.**
PE Body's pointer-Down handler gains the §6.2 precedence check as the very first action after hit-testing the cell. If the hit cell is in edit state per INV-331-7's definition, return `false` immediately — no intent dispatch, no further processing. This phase MUST land before or alongside Phase 3, never after. The precedence rule is load-bearing for every subsequent phase.

**Phase 3 — Cell-materialization branch on `mount_strategy`.**
PE Body's cell materialization branches on the resolved `mount_strategy`. `DisplayEditable` keeps the current path. `DisplayOnly` skips edit-trigger dispatch but materializes the display widget normally. `AlwaysEditor` materializes the editor as the cell's permanent child (the editor_registry factory is reused; `on_exit` callback wires directly into `CellSource::commit_cell`; Cancel reverts in place).

**Phase 4 — `DisplaySelection::Selectable` integration.**
PE Body's display-widget materialization (Label construction for non-edit-mode cells) passes the `TextSelectionPolicy` derived from the resolved `display_selection`. `Inert → TextSelectionPolicy::Disabled` (or the construction simply doesn't enable selection); `Selectable → TextSelectionPolicy::OnCapture`. Label's existing SPEC-HAL-315 Phase 3 integration handles all selection mechanics, paint, gesture, and clipboard.

**Phase 5 — `AlwaysEditor` Escape contract verification.**
Author a test demo (or extend `examples/property_editor_v2.rs`) with at least one column configured as `AlwaysEditor`. Verify that Escape on an AlwaysEditor cell does NOT end the cell's mount, only releases capture and reverts the editor's internal state. Verify this contract is independent of the Phase B `DisplayEditable` capture-leave Cancel observation work.

**Phase 6 — `EditTrigger::SingleClick` end-to-end wiring.**
Verify (or add) that PE Body's pointer-Down dispatch honors `EditTrigger::SingleClick` for `DisplayEditable` cells in display state (subordinate to §6.2 precedence). The `EditTrigger` enum already includes the variant; the dispatch wiring may be partial. Phase 6 closes any gap so JC's stated requirement ("single-click to convert from display to edit mode") works under policy control.

**Phase 7 — `Selectable` display-cell pointer routing audit.**
Audit Label's `hit_testable() = is_selectable()` behavior (label.rs:339-341) against PE Body's pointer-Down dispatch. Label opts into hit-testing when selectable; the deeper hit-target wins event delivery; PE Body's handler is bypassed naturally. Phase 7 confirms the routing path or adds explicit §6.3 branch handling if the natural routing has edge cases.

**Phase 8 — Demo additions.**
Extend `examples/property_editor_v2.rs` (or add a sibling demo) to exercise each `mount_strategy × display_selection` combination on representative cells:
- A `DisplayOnly { Inert }` ReadOnly cell.
- A `DisplayOnly { Selectable }` cell (e.g., a computed "Save Interval" value the user can copy without editing).
- A `DisplayEditable { SingleClick, Inert }` configuration field.
- A `DisplayEditable { DoubleClick, Inert }` standard property (the existing default).
- A `DisplayEditable { DoubleClick, Selectable }` cell where the user can either copy or edit.
- An `AlwaysEditor` cell (e.g., a permanently-live SpinBox in a configuration row).

Validate §6.2 precedence end-to-end: clicking on a mid-edit cell does not affect row selection / column cursor / edit re-trigger; clicking on a different cell during edit does behave per `click_elsewhere_disposition`.

---

# 11. Migration Path

Existing PE consumers see no behavioral change. `PropertyEditorPolicy::cell_policy_default` defaults to `CellInteractionPolicy::default()` (DisplayEditable + Inert). Existing column construction does not set a column policy. Existing cells with `Cell.editable = false` continue to render as read-only (now via the §5 forced override; behavior is identical from the user's perspective).

The §6.2 click-routing precedence rule is new universal behavior. Existing PE demos do not exercise rapid intra-edit clicking — the change closes a latent bug where, e.g., clicking inside an active TextField could re-trigger row selection. Closing that hole is the architectural correction this SPEC is anchored on. No consumer code change required to benefit.

New behavior is opt-in beyond the precedence rule:
- Consumer sets `policy.cell_policy_default.mount_strategy = AlwaysEditor` to flip every cell into permanent-editor mode (forms pattern).
- Consumer sets a per-column policy to flip just one column (e.g., the "Value" column always live; the "Name" column display-only-selectable).
- Consumer sets `policy.cell_policy_default.display_selection = Selectable` to make every display widget selectable for copy.

---

# 12. Conclusion

This SPEC closes the gap between PE's current single-model cell behavior and the broader set of interaction models real applications need. The architecture is two orthogonal policy axes — mount strategy and display capability — resolved through a deterministic layering chain, plus a universal click-routing precedence rule that suppresses PE Body's gesture interpretation for cells in edit state.

The precedence rule is the load-bearing architectural correction. Without it, embedded interactive widgets compete with the enclosing PE's row/column gesture model on every click during an active edit — a TextField caret placement could be reinterpreted as a row-selection re-tap, a SpinBox +/- click could be reinterpreted as edit re-trigger, a ComboBox option click could be reinterpreted as column cursor move. The precedence rule eliminates the competition by defining cell state as the authority: when a cell is in edit state, PE Body is silent on its bounds, and the editor owns every click semantically. This is universal across all editor types; no per-widget exception is needed.

`AlwaysEditor` cells are first-class citizens, not edit sessions; `Selectable` display cells reuse SPEC-HAL-315's already-landed Label integration. The cell is configurable as "always the widget," "display with selectable text," or "display with edit-on-trigger" — by policy, not by hardcoding — and once any cell is in edit state, its editor's per-widget contract operates without interference from the enclosing PE.
