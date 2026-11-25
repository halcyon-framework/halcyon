# SPEC-HAL-200: Tabular Intent Policy

**Status:** Draft (Binding upon adoption)
**Layer:** Interaction Semantics / Intent Interpretation
**Scope:** TabularSubstrate and all tabular consumers
**Applies To:** ListView, List, TreeView, Grid, Dropdown (ComboBox popup)
**Depends On:** SPEC-HAL-169, SPEC-HAL-171, SPEC-HAL-174, SPEC-HAL-175, SPEC-HAL-178, SPEC-HAL-179
**Supersedes:** `ListInteractionPolicy` (widget-scoped, deleted upon adoption)

**Amended by:** SPEC-HAL-347. Current ListView stores one policy and one reducer-owned `OrderedSelectionState` in `ListViewCore`; policy configuration is allowed only before the first routed row interaction; `multi()` preserves established Shift-range drag through `DragPolicy::RangeOnDrag`.

> **Note:** "List" refers to the sole list implementation (TabularSubstrate-backed). Prior v1/v2 distinctions are obsolete.

---

## 1. Purpose

This SPEC defines a **substrate-level, declarative intent policy** that governs interaction semantics for all tabular widgets.

The policy interprets raw input events (pointer, keyboard, modifiers) into semantic intents. It does not own state, mutate data, or depend on geometry. Widgets forward events to the policy; the shared ordered-selection reducer/executor executes the resulting intents. Per SPEC-HAL-262, a composite core may store the single reducer-owned state while using its projected row order as the oracle.

---

## 2. Non-Goals

This SPEC does NOT define:

- ComboBox widget design (see SPEC-HAL-199)
- TreeView-specific disclosure mechanics (see SPEC-HAL-172)
- Column pinning or visibility interaction (see SPEC-HAL-196, SPEC-HAL-197)
- Visual feedback or rendering rules
- Async or data-layer changes
- Inline edit lifecycle (see SPEC-HAL-174)

---

## 3. Problem Statement

Interaction semantics are currently fragmented across three independent implementations:

| Consumer | Modifier interpretation | Policy struct | Intent routing |
|----------|----------------------|---------------|----------------|
| List | `list.rs` | `TabularIntentPolicy` | Policy dispatch via `interpret()` |
| ListView | `body.rs` | `TabularIntentPolicy` | Policy dispatch via `interpret()` |

**Resolution:** All tabular consumers now use `TabularIntentPolicy` for consistent intent interpretation. The policy is stateless, testable, and centralized.

---

## 4. Core Principle (Binding)

**Interaction interpretation must be centralized, stateless, and semantic.**

> Widgets forward events.
> Policy interprets intent.
> Substrate executes intent.

No widget interprets modifier keys. No widget decides what a click means. The policy is the single source of truth for input-to-intent mapping.

---

## 5. Binding Invariants

### INV-INTENT-STATELESS-1 — No Runtime State

The policy struct contains configuration fields only. It stores no runtime state: no selection, no focus, no anchor, no hover, no drag state.

### INV-INTENT-PURE-1 — No Mutations

Policy methods are pure functions. They read input and context, return an intent, and mutate nothing.

### INV-INTENT-SEMANTIC-1 — Semantic Context Only

Policy operates on semantic context (`SelectionMode`, `is_focused`, `has_anchor`, modifier flags). It never receives or inspects geometry, pixel coordinates, widget rects, or scroll offsets.

### INV-INTENT-OWNERSHIP-1 — External State Ownership

All interaction state (selection, focus, anchor, hover, drag) is owned by `TabularSubstrate` or the widget. The policy does not own, cache, or shadow any state.

### INV-INTENT-IDENTITY-1 — Identity-Based Only

Policy reasons about `RowId`, never about indices. Widgets resolve hit-test coordinates to `RowId` before consulting the policy.

### INV-INTENT-SINGLE-1 — Single Policy Instance

There is exactly one `TabularIntentPolicy` per tabular consumer instance. No per-layer shadow policy and no per-event override is permitted.

---

## 6. Intent Model

### 6.1 Existing Intent Infrastructure

`TabularIntent` already exists in `src/widgets/tabular_view/intents.rs` (SPEC-HAL-169, 171, 174, 175, 178):

```rust
pub enum TabularIntent {
    Resize(ResizeIntent),
    Sort(SortIntent),
    Edit(EditIntent),
    Focus(FocusIntent),
    Tree(TreeIntent),
    Selection(SelectionIntent),
}
```

`SelectionIntent` already defines the canonical selection vocabulary:

```rust
pub enum SelectionIntent {
    SelectRow { row_id: RowId },
    ExtendToRow { row_id: RowId },
    ToggleRow { row_id: RowId },
    ClearSelection,
    SetSelection { rows: Vec<RowId>, anchor: Option<RowId> },
}
```

This SPEC does not replace or extend these enums. It defines the **policy that maps input events to them**.

### 6.2 Activation Intent (New)

Activation is semantically distinct from selection. Selection marks; activation opens, runs, or invokes.

```rust
pub enum ActivationIntent {
    /// Activate the target row (open, run, invoke).
    Activate { row_id: RowId },
}
```

This is a new variant added to `TabularIntent`:

```rust
pub enum TabularIntent {
    Resize(ResizeIntent),
    Sort(SortIntent),
    Edit(EditIntent),
    Focus(FocusIntent),
    Tree(TreeIntent),
    Selection(SelectionIntent),
    Activation(ActivationIntent),  // NEW
}
```

### 6.3 Composite Intents

A single input event may produce multiple intents. For example, a plain click in single-selection mode produces both `SelectionIntent::SelectRow` and `FocusIntent::RequestFocus`. The policy returns a `Vec<TabularIntent>`, not a single intent.

---

## 7. TabularIntentPolicy (Normative)

```rust
pub struct TabularIntentPolicy {
    /// How many items may be selected simultaneously.
    pub selection_mode: SelectionMode,

    /// When activation callbacks fire.
    pub activation_mode: ActivationMode,

    /// Whether Shift+click/Shift+nav performs range selection.
    pub range_selection: RangePolicy,

    /// Whether pointer drag performs range selection.
    pub drag_selection: DragPolicy,

    /// Whether clicking a subcontrol (checkbox, button) selects the parent row.
    pub subcontrol_behavior: SubcontrolBehavior,

    /// Immediate vs transactional selection model.
    pub interaction_model: SelectionInteractionModel,
}
```

All fields are configuration values set during consumer construction, before the first routed row interaction. None are mutated after that interaction locks configuration.

### 7.1 SelectionMode

Reuses the existing enum from `core::ordered_selection`:

```rust
pub enum SelectionMode {
    None,    // No selection allowed; focus only
    Single,  // At most one selected row
    Multi,   // Arbitrary number of selected rows
}
```

### 7.2 ActivationMode

```rust
pub enum ActivationMode {
    /// Activate on single click (typical for single-selection lists).
    OnClick,
    /// Activate on double-click (typical for multi-selection lists).
    OnDoubleClick,
    /// Activation only via explicit keyboard (Enter) or programmatic call.
    ExplicitOnly,
}
```

### 7.3 RangePolicy

```rust
pub enum RangePolicy {
    /// Shift has no special behavior.
    Disabled,
    /// Shift+click/Shift+nav selects all rows from anchor to target.
    ShiftRange,
}
```

### 7.4 DragPolicy

```rust
pub enum DragPolicy {
    /// Drag has no selection effect.
    Disabled,
    /// Drag from press point selects range continuously.
    RangeOnDrag,
}
```

### 7.5 SubcontrolBehavior

```rust
pub enum SubcontrolBehavior {
    /// Clicking anywhere in the row (including subcontrols) selects it.
    ClickSelectsRow,
    /// Clicking subcontrols does not select the row.
    ClickDoesNotSelectRow,
}
```

### 7.6 Factory Methods

```rust
impl TabularIntentPolicy {
    /// Single selection, click-to-activate, no range, no drag.
    pub fn single() -> Self { ... }

    /// Multi selection, double-click-to-activate, Shift range, range drag.
    pub fn multi() -> Self { ... }

    /// No selection allowed (focus-only navigation).
    pub fn focus_only() -> Self { ... }
}
```

---

## 8. Intent Interpretation Rules

The policy exposes a single interpretation entry point:

```rust
impl TabularIntentPolicy {
    pub fn interpret(
        &self,
        event: &IntentInput,
        context: &IntentContext,
    ) -> Vec<TabularIntent>;
}
```

### 8.1 IntentInput

Semantic input abstraction. Widgets translate raw `PointerEvent` / `KeyEvent` into this before consulting the policy:

```rust
pub enum IntentInput {
    Click {
        target: RowId,
        modifiers: Modifiers,
        is_subcontrol: bool,
    },
    DoubleClick {
        target: RowId,
    },
    DragStart {
        target: RowId,
        modifiers: Modifiers,
    },
    DragMove {
        target: RowId,
    },
    DragEnd,
    KeyPress {
        key: KeyCode,
        modifiers: Modifiers,
    },
}
```

Widgets are responsible for:

- Hit-testing pointer coordinates to `RowId` (geometry layer)
- Detecting click vs double-click vs drag (event layer)
- Classifying subcontrol hits (widget layer)

Widgets are NOT responsible for interpreting what these inputs mean.

### 8.2 IntentContext

State snapshot provided by the substrate/widget at interpretation time:

```rust
pub struct IntentContext {
    /// Whether the widget is focused.
    pub focused: bool,
    /// Whether a selection anchor exists.
    pub has_anchor: bool,
    /// Whether inline editing is active.
    pub is_editing: bool,
    /// Current focused row, if any.
    pub focused_row: Option<RowId>,
}
```

Context is read-only. The policy never writes to it.

### 8.3 Pointer Interpretation Rules (Normative)

These rules define the canonical modifier-to-intent mapping. They replace the hardcoded logic currently in `body.rs:435-456` and `list.rs:1193-1209`.

#### Plain Click (no modifiers)

| SelectionMode | Result |
|---------------|--------|
| `None` | `FocusIntent::RequestFocus` only |
| `Single` | `SelectionIntent::SelectRow` + `FocusIntent::RequestFocus` |
| `Multi` | `SelectionIntent::SelectRow` + `FocusIntent::RequestFocus` |

If `activation_mode == OnClick`: also emit `ActivationIntent::Activate`.

#### Ctrl/Cmd+Click

| SelectionMode | Result |
|---------------|--------|
| `None` | No effect |
| `Single` | `SelectionIntent::ToggleRow` + `FocusIntent::RequestFocus` |
| `Multi` | `SelectionIntent::ToggleRow` + `FocusIntent::RequestFocus` |

#### Shift+Click

| RangePolicy | SelectionMode | Result |
|-------------|---------------|--------|
| `Disabled` | Any | Same as plain click |
| `ShiftRange` | `None` | No effect |
| `ShiftRange` | `Single` | `SelectionIntent::SelectRow` (range not meaningful for single) |
| `ShiftRange` | `Multi` | `SelectionIntent::ExtendToRow` + `FocusIntent::RequestFocus` |

#### Double-Click

If `activation_mode == OnDoubleClick`: emit `ActivationIntent::Activate`.

Otherwise: no additional effect beyond the click that preceded it.

#### Subcontrol Click

If `subcontrol_behavior == ClickDoesNotSelectRow`: suppress selection intents, forward only to the subcontrol.

If `subcontrol_behavior == ClickSelectsRow`: treat as plain click plus subcontrol action.

### 8.4 Keyboard Interpretation Rules (Normative)

These rules replace the hardcoded logic in `list_view.rs:230-282` and `list.rs:709-851`.

#### Arrow Keys (Up/Down)

| Modifiers | SelectionMode | Result |
|-----------|---------------|--------|
| None | Any | `SelectionIntent::SelectRow` (next/prev) + `FocusIntent::RequestNavigate` |
| Shift | `Multi` + `ShiftRange` | `SelectionIntent::ExtendToRow` (next/prev) + `FocusIntent::RequestNavigate` |
| Ctrl/Cmd | Any | `FocusIntent::RequestNavigate` only (move focus without selecting) |
| Shift | `Single` or `Disabled` | Same as no modifiers |

#### Home / End

Same modifier logic as Arrow Keys, targeting first/last row.

#### Page Up / Page Down

Same modifier logic as Arrow Keys, targeting row one viewport-height away.

#### Enter

Emit `ActivationIntent::Activate` for the focused row, regardless of `activation_mode`.

#### Escape

If `is_editing`: emit `EditIntent::RequestCancelEdit`.
Otherwise: emit `SelectionIntent::ClearSelection`.

#### Space

| SelectionMode | Result |
|---------------|--------|
| `Multi` | `SelectionIntent::ToggleRow` for focused row |
| `Single` | `SelectionIntent::SelectRow` for focused row |
| `None` | No effect |

### 8.5 Drag Interpretation Rules (Normative)

| DragPolicy | Modifiers | Result |
|------------|-----------|--------|
| `Disabled` | Any | No selection effect from drag |
| `RangeOnDrag` | None | `SelectionIntent::ExtendToRow` on each DragMove |
| `RangeOnDrag` | Shift | `SelectionIntent::ExtendToRow` on each DragMove (same as above) |

Drag state (active, anchor) is owned by the widget. The policy only interprets intent per DragMove event.

---

## 9. Authority Boundaries (Binding)

| Layer | Responsibility | Forbidden |
|-------|---------------|-----------|
| **Widget** | Hit-test to `RowId`; detect click/double-click/drag; classify subcontrols; construct `IntentInput`; own transient drag state; visual feedback | Interpret modifiers; decide selection semantics; mutate selection directly |
| **Policy** | Map `IntentInput` + `IntentContext` → `Vec<TabularIntent>` | Own state; mutate anything; inspect geometry; access widget internals |
| **Selection engine / executor** | Execute intents through the shared reducer; own selection/focus/anchor semantics in one `OrderedSelectionState`; validate mode constraints | Interpret input events; know about pointer/keyboard/modifiers |
| **Data Layer** | Accept or reject mutations; own model data | Know about input, policy, or widget |

### INV-INTENT-AUTH-1 — Widget Event Forwarding

Widgets MUST forward input to the policy via `IntentInput`. Widgets MUST NOT contain `if is_ctrl_or_cmd { ... } else if is_shift { ... }` logic. All such logic is centralized in the policy.

### INV-INTENT-AUTH-2 — Shared Intent Execution

The tabular executor executes intents received from the policy through the existing shared engine/reducer. The executor MUST NOT interpret raw input events and MAY reject an intent if it violates mode constraints. Per SPEC-HAL-262, the single `OrderedSelectionState` may be stored in the composite core while retaining reducer-owned semantics.

### INV-INTENT-AUTH-3 — No Direct Mutation

No code path from pointer/keyboard event to selection mutation may bypass the policy. The chain is always: `event → widget → IntentInput → policy → TabularIntent → shared reducer/executor`.

---

## 10. Integration Rules

### 10.1 ListView

**Current state:** ListView pointer and keyboard handlers construct `IntentInput`, interpret it through the single configured policy, and forward the resulting intents to `ListViewCore`'s shared ordered-selection reducer/executor. `ListViewCore` stores the one reducer-owned `OrderedSelectionState` and uses its data-source-projected row order as the identity oracle per SPEC-HAL-262 and SPEC-HAL-347.

**Required wiring:**

1. `ListViewBody` replaces its `if is_ctrl_or_cmd / is_shift / else` block with construction of `IntentInput::Click { target, modifiers, is_subcontrol }`.
2. Calls `policy.interpret(&input, &context)` to obtain `Vec<TabularIntent>`.
3. Forwards each intent to `ListViewCore::dispatch_intents_and_focus()` and the shared ordered-selection reducer/executor.
4. Event handlers do not construct `SelectionIntent` or `ActivationIntent` directly.

Keyboard handling follows the same pattern: construct `IntentInput::KeyPress`, interpret, dispatch.

### 10.2 List

**Current state:** `list.rs:1193-1209` contains hardcoded modifier interpretation. Selection actions use widget-local methods (`action_ctrl_click`, `action_click`).

**Migration:**

1. Modifier interpretation replaced by `IntentInput` construction + `policy.interpret()`.
2. Widget-local action methods (`action_ctrl_click`, etc.) are replaced by generic `dispatch_intents(Vec<TabularIntent>)`.
3. Keyboard handling in `list.rs:709-851` follows the same pattern.

### 10.3 List (v1)

**Current state:** Uses `ListInteractionPolicy` (declared in `list_interaction_policy.rs`). Policy fields (`selection_mode`, `activation_mode`, `range_selection`, `drag_selection`, `subcontrol_policy`) are structurally identical to `TabularIntentPolicy` fields.

**Migration:**

1. Replace `ListInteractionPolicy` field with `TabularIntentPolicy`.
2. Factory methods `default_single()` and `multi()` map to `TabularIntentPolicy::single()` and `TabularIntentPolicy::multi()`.
3. Modifier interpretation logic (wherever it lives in `list.rs`) routes through `policy.interpret()`.

### 10.4 Future Consumers

TreeView, Grid, and Dropdown (ComboBox popup) consume the same `TabularIntentPolicy` with appropriate factory presets:

- **TreeView:** `TabularIntentPolicy::single()` with `DisclosureIntent` handled separately (SPEC-HAL-172).
- **Grid:** `TabularIntentPolicy::single()` with cell-level navigation and editing.
- **Dropdown:** `TabularIntentPolicy::single()` with `ActivationMode::OnClick`, no range, no drag, no multi-selection.

---

## 11. Deletion Plan

### 11.1 `ListInteractionPolicy` (Immediate Deletion)

Upon adoption of this SPEC:

- `src/widgets/list_interaction_policy.rs` is deleted entirely.
- `pub mod list_interaction_policy;` is removed from `src/widgets/mod.rs`.
- All imports of `ListInteractionPolicy`, `ActivationMode`, `RangeSelectionPolicy`, `DragSelectionPolicy`, `SubcontrolPolicy` from this module are removed.
- `List` struct field `interaction_policy: ListInteractionPolicy` is replaced with `intent_policy: TabularIntentPolicy`.
- Methods `set_interaction_policy()` and `interaction_policy()` are replaced with `set_intent_policy()` and `intent_policy()`.

### 11.2 Widget-Local Modifier Logic (Immediate Deletion)

Upon adoption:

- `body.rs:435-456` (ListViewBody modifier interpretation) is replaced with policy dispatch.
- `list.rs:1193-1209` (List modifier interpretation) is replaced with policy dispatch.
- `list_view.rs:230-282` (ListView keyboard handling) is replaced with policy dispatch.
- `list.rs:709-851` (List keyboard handling) is replaced with policy dispatch.

### 11.3 Duplicate SelectionMode

`SelectionMode` is currently defined in both `list_state.rs:292-300` and `core::ordered_selection::mod.rs:91-100`. Upon adoption, the canonical definition is `core::ordered_selection::SelectionMode`. The `list_state.rs` duplicate is deleted.

---

## 12. Forbidden Patterns (Binding)

### FORBID-INTENT-1 — Widget-Side Modifier Interpretation

No widget may contain `if modifiers.ctrl { ... } else if modifiers.shift { ... }` for selection/activation semantics. All modifier interpretation lives in `TabularIntentPolicy::interpret()`.

### FORBID-INTENT-2 — Widget-Scoped Interaction Policies

No widget-specific policy struct may exist. `ListInteractionPolicy`, or any future `TreeViewInteractionPolicy` / `GridInteractionPolicy`, is forbidden. One policy type serves all consumers.

### FORBID-INTENT-3 — Policy State Storage

The policy struct MUST NOT contain `Cell`, `RefCell`, `Rc`, `Arc`, or any interior mutability. All fields are `Copy` or `Clone` configuration values.

### FORBID-INTENT-4 — Index-Based Logic in Policy

The policy MUST NOT reason about row indices, column indices, or position integers. Only `RowId` identity types are permitted in intent output.

### FORBID-INTENT-5 — Geometry in Policy

The policy MUST NOT receive, inspect, or depend on pixel coordinates, widget rects, scroll offsets, or any geometric quantity.

### FORBID-INTENT-6 — Bypassing the Chain

No code path may produce a `SelectionIntent` or `ActivationIntent` without going through `TabularIntentPolicy::interpret()`. Direct construction of selection intents in widget event handlers is forbidden after adoption.

---

## 13. Test Oracle Requirement

A reference oracle MUST exist that:

- Constructs `IntentInput` values synthetically (no widget, no rendering).
- Constructs `IntentContext` values synthetically.
- Calls `TabularIntentPolicy::interpret()`.
- Asserts exact intent output.

Minimum test matrix:

| Input | SelectionMode | Modifiers | Expected Intent |
|-------|---------------|-----------|-----------------|
| Click | Single | None | `SelectRow` + `Activate` (if OnClick) |
| Click | Multi | None | `SelectRow` |
| Click | Multi | Ctrl | `ToggleRow` |
| Click | Multi | Shift | `ExtendToRow` |
| Click | None | Any | Focus only |
| DoubleClick | Multi | None | `Activate` (if OnDoubleClick) |
| KeyPress(Down) | Single | None | `SelectRow` (next) |
| KeyPress(Down) | Multi | Shift | `ExtendToRow` (next) |
| KeyPress(Down) | Any | Ctrl | Focus only |
| KeyPress(Enter) | Any | None | `Activate` |
| KeyPress(Esc) | Any | None | `ClearSelection` |
| KeyPress(Space) | Multi | None | `ToggleRow` |
| DragMove | Multi | None | `ExtendToRow` (if RangeOnDrag) |

All tests run without instantiating any widget, substrate, or renderer.

---

## 14. Acceptance Criteria

This SPEC is accepted when:

- `TabularIntentPolicy` is the sole policy struct for all tabular consumers.
- `TabularIntentPolicy::interpret()` is the sole entry point for input-to-intent mapping.
- `ListInteractionPolicy` is deleted.
- No widget contains hardcoded modifier-to-selection logic.
- ListView, List, and List (v1) all route through the same policy.
- The test oracle passes the minimum test matrix without rendering.
- TreeView, Grid, and Dropdown can consume the same policy type with appropriate presets.

---

## 15. Migration Sequence (Non-Normative)

Recommended implementation order:

1. Define `TabularIntentPolicy` struct, `IntentInput`, `IntentContext`, `ActivationIntent`.
2. Implement `TabularIntentPolicy::interpret()` with full interpretation rules.
3. Write test oracle covering the minimum test matrix.
4. Migrate ListView: replace body.rs modifier logic + keyboard logic with policy dispatch.
5. Migrate List: replace list.rs modifier logic + keyboard logic with policy dispatch.
6. Migrate List (v1): replace `ListInteractionPolicy` with `TabularIntentPolicy`.
7. Delete `src/widgets/list_interaction_policy.rs`.
8. Delete duplicate `SelectionMode` from `list_state.rs`.
9. Verify all consumers compile and pass tests.
