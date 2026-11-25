# SPEC-HAL-347: ListView Focused-Row Observation

**Status:** Active / Binding
**Layer:** ListView Consumer API / Interaction Observation
**Scope:** ListView intent configuration, row-axis focus observation, activation delivery
**Depends on:** SPEC-HAL-002, SPEC-HAL-175, SPEC-HAL-178, SPEC-HAL-179, SPEC-HAL-187, SPEC-HAL-192, SPEC-HAL-193, SPEC-HAL-200, SPEC-HAL-262, SPEC-HAL-313
**Narrowly amends:** SPEC-HAL-178 selection-storage wording for current ListView; SPEC-HAL-187 selection/focus delegation wording; SPEC-HAL-193 §2–§3 public-surface lock; SPEC-HAL-200 pre-interaction configuration and multi-range-drag preset; SPEC-HAL-262 §14 ListView configuration statement

---

## 1. Purpose

ListView owns no business meaning, but a consumer must be able to observe the committed `RowId` that currently carries ListView row-axis focus. This is required for identity-following inspector surfaces and other read-only projections that must follow pointer and keyboard navigation without treating activation as selection.

This SPEC admits the smallest public surface required for that integration while preserving ListView's closed-composite architecture and TabularSubstrate authority.

---

## 2. Existing Authority and Semantic Separation

- **Selection semantics** are owned by the existing `OrderedSelectionState` engine and its reducer.
- **ListViewCore** is the storage host for exactly one `OrderedSelectionState<RowId>`, as already recorded by SPEC-HAL-262. Storage in the composite is not widget-local semantic reinvention.
- **Row order** remains data-authoritative. ListView's reducer oracle reads the current `DataSource`-projected `RowId` order; the callback layer owns no order or identity map.
- **Row-axis focus** is the single `OrderedSelectionState::focus_key` navigation cursor.
- **Cell focus** is the `FocusState` concept governed by SPEC-HAL-175. Row-axis focus does not replace or mutate cell focus.
- **Widget focus** remains framework-owned. A focused `RowId` is not a focused widget.
- **Activation** is an explicit open, run, invoke, or command semantic represented by `ActivationIntent`.

The focused-row callback observes row-axis focus only. It is not a selection callback and must never emit activation. The activation callback remains independent and fires only for `ActivationIntent::Activate`.

---

## 3. Admitted Public API

ListView may expose exactly these additional consumer APIs:

```rust
pub fn set_intent_policy(&mut self, policy: TabularIntentPolicy) -> bool;
pub fn intent_policy(&self) -> TabularIntentPolicy;
pub fn focused_row(&self) -> Option<RowId>;
pub fn set_focused_row_callback<F>(&mut self, callback: F)
where
    F: Fn(Option<RowId>) + 'static;
```

`set_intent_policy` configures the one existing `TabularIntentPolicy` instance owned by ListView and synchronizes its selection mode and interaction model into the existing ordered-selection state. It does not create a widget-local shadow policy. Configuration is accepted only before the first routed row interaction locks the ordered-selection interaction model. The method returns `false` when a different policy is requested after that lock; it must not partially apply the request.

`focused_row` is a read-only identity query. It exposes no mutable authority and no index.

`set_focused_row_callback` installs one consumer callback. This is not a general listener registry, event source, intent interceptor, or observer graph.

All other callback, listener, extension, and intent-interception prohibitions in SPEC-HAL-193 remain binding.

---

## 4. Notification Contract

The focused-row callback:

1. receives `Option<RowId>` only;
2. fires after a committed ListView event changes row-axis focus;
3. covers pointer row interaction and ListView-owned keyboard navigation;
4. fires at most once for one routed event;
5. does not fire when the focused `RowId` is unchanged;
6. runs only after every internal `RefCell` borrow used for dispatch has been released;
7. may enqueue the identity into a consumer-owned pending action/event buffer that is drained through the lawful application update path.

The callback must not directly mutate Halcyon `AppState`, `WidgetTree`, other widgets, layout state, navigation, or focus during event routing. It must not expose row indices, geometry, realization slots, widget references, mutable substrate access, or event reconstruction data.

---

## 5. Intent Policy and Activation

`TabularIntentPolicy::single()` remains the canonical single-selection/click-activation preset from SPEC-HAL-200.

`TabularIntentPolicy::multi()` preserves ListView's established Shift-range drag behavior by using `DragPolicy::RangeOnDrag`. Every drag move is interpreted through `IntentInput::DragMove`; widget handlers must not construct `SelectionIntent` directly.

When pointer interpretation produces `ActivationIntent::Activate` under `ActivationMode::OnClick`, ListView must invoke the existing activation callback after internal borrows are released. Enter and `OnDoubleClick` activation behavior must remain unchanged.

Focused-row notification and activation may both result from the same pointer event, but they remain separate callbacks with separate meanings.

---

## 6. Invariants

- **INV-347-IDENTITY-1:** Every focused-row notification is `Option<RowId>`. No index or coordinate API is admitted.
- **INV-347-AUTHORITY-1:** The callback observes the existing reducer-owned row-axis focus stored once in `ListViewCore`. The callback layer stores no duplicate focus or selection state.
- **INV-347-BORROW-1:** Consumer callbacks run only after ListView's internal `RefCell` borrows are released.
- **INV-347-DISTINCTION-1:** Focused-row observation never implies activation, and activation never substitutes for focused-row observation.
- **INV-347-POLICY-1:** ListView owns exactly one `TabularIntentPolicy`; public configuration replaces that value and synchronizes the existing ordered-selection configuration.
- **INV-347-POLICY-LOCK-1:** The first routed row interaction locks policy-backed selection configuration. Later different-policy requests fail atomically.
- **INV-347-DRAG-1:** Range drag routes through `IntentInput::DragMove` and `TabularIntentPolicy::interpret()` before reducer dispatch.
- **INV-347-PRESENTATION-1:** A routed selection or focused-row change invalidates the exact retained `ListViewBody` paint fragment in the same event boundary. Callback-observed identity and the visible selection/focus projection must not diverge; unchanged geometry does not authorize ListView layout invalidation.
- **INV-347-CLOSED-1:** ListView remains a closed composite under SPEC-HAL-192. No child extension point, pub/sub system, or authority projection is introduced.

---

## 7. Forbidden Behaviors

- Geometry or coordinate hit-testing outside ListView's existing owner event path.
- Visible-index inference or persisted row-index identity.
- Consumer reconstruction of pointer or keyboard events.
- Direct construction of selection or activation intents in ListView event handlers.
- A duplicate selection, focus, or activation state owned by the callback layer.
- Callback invocation while `ListViewCore` is mutably or immutably borrowed.
- Reusing the activation callback as the focused-row observer.
- Generic listener lists, intent interception, or consumer mutation of ListView authority.

---

## 8. Verification

Mechanical tests must prove:

1. one primary click under `TabularIntentPolicy::single()` selects/focuses the identity and immediately emits the focused `RowId`;
2. the same click delivers `OnClick` activation through the existing activation callback;
3. Up, Down, Home, End, PageUp, PageDown, and admitted capture-mode j/k navigation emit the resulting focused `RowId` without Enter;
4. a focused-row callback can re-enter ListView core reads without a borrow panic;
5. Enter and double-click activation behavior remains intact;
6. no callback is emitted for an unchanged focused `RowId`.
7. range drag is policy-interpreted and emits focused `RowId` changes without direct intent construction;
8. policy configuration succeeds before interaction and a different policy fails atomically after the first routed interaction;
9. pointer and keyboard selection changes repaint the retained body without layout, preserve every projected row, and leave an unchanged re-selection paint-clean.
