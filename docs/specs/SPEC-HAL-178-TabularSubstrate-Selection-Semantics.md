# SPEC-HAL-178: TabularSubstrate — Row Selection Semantics

**Status:** Implemented / Closed
**Layer:** Engine / Interaction Semantics
**Owner:** Core Architecture
**Audience:** Engine, Interaction, Widget Composition
**Parent:** SPEC-HAL-164 (TabularSubstrate Core)
**Depends on:** SPEC-HAL-174 (Edit Authority), SPEC-HAL-175 (Focus Semantics)

**Amended by:** SPEC-HAL-347 for current ListView storage only. ListView stores its single `OrderedSelectionState<RowId>` in `ListViewCore`, as recorded by SPEC-HAL-262, and mutates it only through the shared reducer. The selection semantics and single-authority laws in this SPEC remain binding; obsolete `substrate.ordered_selection()` storage/accessor wording does not require a second selection state in current ListView.
**Extended by:** SPEC-HAL-353 for range/ordered selection over provider-confirmed revision-scoped order windows in
SPEC-HAL-188-backed consumers

---

## 0. Scope and Intent

This SPEC defines **row selection semantics at the TabularSubstrate engine layer**.

TabularSubstrate is an **abstraction substrate**, not a widget:
it exists to support ListView, TreeView, PropertyView, and future tabular constructs.

Selection defined here is therefore:

* **structural**
* **identity-based**
* **projection-only**
* **completely UI-agnostic**

Anything visual, stylistic, or gesture-driven is **explicitly out of scope**.

---

## 1. Binding Invariants (Law)

### INV-178-1 — Selection Unit

Selection operates **exclusively on `RowId`**.

* No cell-level selection
* No column-level selection
* No index-based selection
* No geometry-based selection

`RowId` is the sole identity.

---

### INV-178-2 — Selection / Focus Separation

Selection **must not read, mutate, or depend on `FocusState`**.

* Focus may move without affecting selection
* Selection may change without affecting focus
* No implicit synchronization is permitted

When a SelectionIntent arrives during an active edit session, the edit system cancels the edit and restores pre-edit focus **before** the selection handler executes. This is a prerequisite side-effect owned by the edit system (SPEC-HAL-174), not by selection. Selection never observes or causes FocusState mutation.

Any direct coupling between selection logic and FocusState is illegal.

---

### INV-178-3 — Identity Stability

Selection is **identity-sticky**.

* Sort changes do not alter selection
* Tree expand/collapse does not alter selection
* Scrolling / virtualization does not alter selection
* Column visibility does not alter selection

Visibility != existence.

---

### INV-178-4 — Anchor Semantics

Selection may have an **anchor** (`Option<RowId>`).

* Anchor is owned by `OrderedSelectionState`
* Anchor is set only by explicit selection intents
* Anchor is cleared only by `ClearSelection` or edit begin (INV-178-5)

Anchor is **not** focus. Anchor is **not** geometry.

**Anchor assignment rules per intent:**

| Intent | Anchor behavior |
|--------|----------------|
| `SelectRow` | Anchor = target row |
| `ExtendToRow` (anchor None) | Anchor = target row |
| `ExtendToRow` (anchor Some) | Anchor unchanged |
| `ToggleRow` (adding) | Anchor unchanged if already set; anchor = added row if None |
| `ToggleRow` (removing, removed row != anchor) | Anchor unchanged |
| `ToggleRow` (removing, removed row == anchor) | Anchor = first remaining selected row, or None if empty |
| `ClearSelection` | Anchor = None |

---

### INV-178-5 — Edit Boundary

Entering edit mode **clears selection** (anchor and set).

Rationale:

* Editing is single-row, single-cell authority
* Multi-row selection during edit is undefined and forbidden

Edit begin is a **hard boundary**.

Selection during active edit triggers edit cancellation (SPEC-HAL-174 destabilization) **before** the selection intent executes. The edit system owns focus restoration; selection does not participate.

---

### INV-178-6 — Monotonic Versioning

Selection changes are versioned.

* Version increments **exactly once** per accepted semantic change
* No-op operations do not increment
* Version monotonicity is strict

Versioning exists solely for downstream invalidation.

---

### INV-178-7 — Range Computation Order

Range selection (`ExtendToRow`) is computed over:

```
effective_row_order()
```

Which is defined as:

* tree-visible order (when tree active)
* with active sort applied (when sort active)
* natural model order (when neither active)

`effective_row_order()` ensures fresh tree indices before computing order.

For a SPEC-HAL-188-backed consumer, `effective_row_order()` means the provider-confirmed `OrderRevision`. The selection
oracle resolves anchor/target RowIds through `LocateRow` and enumerates only the bounded windows needed for the inclusive
range. It MUST NOT compute local sorted indices or require a complete RowId order snapshot. If required lookup/window
testimony is pending, the selection intent is not accepted and selection/version remain unchanged.

Range is **semantic**, not positional.

---

### INV-178-8 — Non-Mutation Rule

Selection logic **must not mutate**:

* RowModel
* ColumnModel
* Geometry tables
* Layout state
* Scroll state
* FocusState

Selection is a **pure projection over identity**.

---

### INV-178-9 — Single Write Authority

**All selection mutations flow through a single path:**

```
SelectionIntent
  → TabularSubstrate::handle_selection_intent
  → reduce_ordered_selection (reducer)
  → mirror_ordered_into_legacy_selection (temporary compatibility)
```

**Forbidden mutation paths:**

* No widget may mutate selection directly
* No consumer may bypass the reducer
* No code may mutate the legacy `SelectionState` except `mirror_ordered_into_legacy_selection`

This invariant is enforced by architecture. Violations are bugs.

---

## 2. Selection State Architecture

### 2.1 Authoritative State

Selection state is owned by `OrderedSelectionState<RowId>`:

* `selected_keys: HashSet<RowId>` — currently selected rows
* `anchor_key: Option<RowId>` — persistent anchor for range operations
* `range_anchor_key: Option<RowId>` — dedicated anchor for Shift+* operations
* `selection_mode: SelectionMode` — None / Single / Multi
* `interaction_model: SelectionInteractionModel` — Immediate / Transactional

This is the **single source of truth**. All selection logic flows through this structure via the reducer.

### 2.2 Legacy Compatibility Mirror

`SelectionState` exists solely for backward compatibility during migration:

* It is a **write-only mirror** of `OrderedSelectionState`
* It is updated **only** by `mirror_ordered_into_legacy_selection()`
* It will be removed once all consumers migrate

**Do not use `SelectionState` for new code.**

---

## 3. Read API

### 3.1 Authoritative Read Surface

```rust
substrate.ordered_selection() -> &OrderedSelectionState<RowId>
```

Available queries:

| Method | Description |
|--------|-------------|
| `is_selected(row_id)` | Check if a row is selected |
| `is_selection_empty()` | Check if selection is empty |
| `selection_count()` | Number of selected rows |
| `selected_keys_iter()` | Iterator over selected RowIds (unordered) |
| `anchor_key()` | Persistent anchor for range operations |
| `range_anchor_key()` | Dedicated range anchor for Shift+* |
| `is_transactional()` | Check if transactional mode |
| `is_transaction_active()` | Check if transaction in progress |
| `working_selection()` | Current working selection |
| `committed_selection()` | Last committed selection |

**All new code must use this API.**

### 3.2 Deprecated Read Surface

```rust
#[deprecated]
substrate.legacy_selection_state() -> &SelectionState
```

This accessor exists only for:

* List v1 (legacy widget)
* ListView (intentionally disposable)
* Tests (marked for migration)

**Migration path:**

| Legacy | Authoritative |
|--------|---------------|
| `legacy_selection_state().is_selected(id)` | `ordered_selection().is_selected(id)` |
| `legacy_selection_state().selected_rows().iter()` | `ordered_selection().selected_keys_iter()` |
| `legacy_selection_state().has_selection()` | `!ordered_selection().is_selection_empty()` |
| `legacy_selection_state().count()` | `ordered_selection().selection_count()` |
| `legacy_selection_state().anchor()` | `ordered_selection().anchor_key()` |

---

## 4. Interaction Models

### 4.1 Immediate Mode (Default)

Selection mutates immediately on input.

* `selected_keys` reflects visible selection at all times
* No commit/cancel semantics
* Standard for most list/table interactions

### 4.2 Transactional Mode

Selection is a working set with explicit commit/cancel.

* `begin_transaction()` — snapshot current selection, start working set
* `commit_transaction()` — make working set permanent
* `cancel_transaction()` — revert to pre-transaction state

**State visibility during transaction:**

| Accessor | Returns |
|----------|---------|
| `ordered_selection().selected_keys` | Working selection |
| `ordered_selection().committed_selection()` | Pre-transaction snapshot |

Transactional mode is configured via `set_interaction_model()` and locked after first input event.

---

## 5. Explicit Non-Guarantees

### 5.1 No Iteration Order

`selected_keys_iter()` returns keys in arbitrary `HashSet` order.

For ordered iteration, consumers must:

1. Collect selected RowIds
2. Resolve each to an ordinal in the current confirmed order (`sorted_indices` only for legacy/materialized paths;
   `LocateRow` under SPEC-HAL-353)
3. Sort by that revision-scoped ordinal and discard the ordering if the revision changes

There are no "first selected" or "last selected" helpers.

### 5.2 Stale Keys Tolerated

Selected RowIds may reference rows no longer in the RowModel.

* Stale selections are silently tolerated until next mutation
* No automatic cleanup on row deletion
* Consumer is responsible for validating RowIds if needed

### 5.3 No Index-Based Reasoning

Index-based selection is explicitly non-authoritative.

* Selection is by RowId, not by visual position
* Any index-based logic belongs to the consumer, not the engine

---

## 6. What Selection Is *Not*

Explicitly forbidden at this layer:

* Cell-level selection
* Visual styling rules
* Hit-testing
* Modifier interpretation (Shift/Ctrl/etc.)
* Drag selection gesture handling
* Clipboard semantics

Those belong **upstream** in widget adapters.

---

## 7. Intent-Driven Model

TabularSubstrate does **not** interpret input.

It consumes **already-decoded intents**.

The engine recognizes exactly these semantic directives:

```rust
SelectRow { row_id }
ExtendToRow { row_id }
ToggleRow { row_id }
ClearSelection
SetSelection { rows, anchor }
```

Anything else is upstream policy.

---

## 8. Range and Fallback Semantics

### 8.1 ExtendToRow — Normal Case

When anchor exists and **both** anchor and target are found in the current effective order oracle:

* Select the inclusive range from anchor to target in effective order
* Anchor is unchanged
* Direction is irrelevant (min..=max)
* For a SPEC-HAL-188-backed consumer, enumerate the inclusive range through bounded current-revision windows; pending
  testimony leaves selection unchanged and is not treated as a missing-row fallback

### 8.2 ExtendToRow — Anchor Missing (None)

When anchor is `None`:

* Anchor = target
* Selection = `[target]`

### 8.3 ExtendToRow — Anchor Not in Effective Order

When anchor exists but is not found in `effective_row_order()` (tree-collapsed, etc.):

* Selection = `[target]`
* Anchor is unchanged (remains stale)

### 8.4 ExtendToRow — Target Not in Effective Order

When target is not found in `effective_row_order()`:

* Selection = `[target]`
* Anchor is unchanged

### 8.5 ExtendToRow — Both Missing from Effective Order

* Selection = `[target]`
* Anchor is unchanged

### 8.6 Rationale

In all fallback cases, the target row is selected unconditionally. This preserves INV-178-3 (identity stability): a row may be selected regardless of its visibility in the current effective order. No implicit resurrection of hidden rows into the range occurs — only the explicit target is selected.

---

## 9. Interaction With Other Systems

### Focus

* Independent
* No coupling
* No side effects
* Edit cancellation may restore focus as a prerequisite to selection dispatch, but this is SPEC-HAL-174 authority, not selection authority

### Sort

* Provider-confirmed sorting replaces the effective order revision for SPEC-HAL-188-backed consumers
* Does not alter selection set

### Tree

* Visibility affects range computation only
* Hidden rows remain selected logically

### Virtualization

* Selection persists regardless of realization

### Editing

* Edit begin clears selection (INV-178-5)
* Selection intent during edit cancels edit first (SPEC-HAL-174 destabilization), then selection proceeds normally

---

## 10. Implementation Notes (Non-Normative)

### 10.1 State Ownership

* `OrderedSelectionState<RowId>` — authoritative selection state
* `SelectionState` — deprecated compatibility mirror (will be removed)

### 10.2 Mutation Path

All mutations flow through:

```rust
handle_selection_intent(&TabularIntent::Selection(...))
  → reduce_ordered_selection(&mut ordered_selection_state, action, oracle)
  → mirror_ordered_into_legacy_selection()  // temporary
```

### 10.3 Read Accessors

* `ordered_selection()` — authoritative, use for all new code
* `legacy_selection_state()` — deprecated, compatibility only

### 10.4 Invariant Identifiers

Code-level invariants are documented as:

* `INV-SEL-1` through `INV-SEL-4` in `ordered_selection/mod.rs`
* `INV-TAB-SEL-1` through `INV-TAB-SEL-3` in `tabular_view/mod.rs`

These are constraints, not suggestions.

---

## 11. Closure

**This SPEC is fully implemented and closed.**

Implementation:

* `src/core/ordered_selection/mod.rs` — OrderedSelectionState, reducer, actions
* `src/widgets/tabular_view/mod.rs` — TabularSubstrate integration, mirror function
* `src/widgets/tabular_view/intents.rs` — SelectionIntent enum

Enforcement:

- **INV-178-1:** `OrderedSelectionState.selected_keys: HashSet<RowId>` — row identity only.
- **INV-178-2:** `handle_selection_intent` never reads or writes `FocusState`.
- **INV-178-3:** No sort/tree/scroll/column operation touches selection state.
- **INV-178-4:** Anchor assignment per intent follows the table exactly.
- **INV-178-5:** `handle_edit_intent(RequestBeginEdit)` clears selection unconditionally.
- **INV-178-6:** Version incremented only when state actually changes.
- **INV-178-7:** Range computation uses the authoritative effective-order oracle with fallback semantics; data-backed
  consumers use provider revision/window testimony under SPEC-HAL-353, never local sorted indices.
- **INV-178-8:** Selection logic does not mutate RowModel, ColumnModel, geometry, layout, scroll, or FocusState.
- **INV-178-9:** All mutations flow through the reducer. No other mutation path is permitted.

Integration tests in `tests/tabular_view/selection.rs`.

Selection semantics are frozen. No further work permitted under this SPEC. Any change to selection unit, anchor policy, range semantics, multi-anchor, column selection, or gesture interpretation requires a new SPEC.
