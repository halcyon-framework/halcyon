# SPEC-HAL-202: ComboBox Composition Audit with TabularSubstrate Consumers

**Status:** Binding
**Type:** Derived Composition Clarification (no new rules, no new APIs)
**Scope:** ComboBox as editor widget inside ListView, TreeView, Grid, PropertyEditor
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-188, SPEC-HAL-188A, SPEC-HAL-164, SPEC-HAL-187, SPEC-HAL-174
**Non-Goals:** Rendering, styling, engine changes, resolver logic changes, new widgets, performance

---

## 0. Framing

This document introduces no new rules, no new invariants, and no new authority.

Every constraint stated here is derived from existing, binding SPECs. A violation described in this document is already a violation of SPEC-HAL-201, SPEC-HAL-188, SPEC-HAL-188A, SPEC-HAL-164, SPEC-HAL-174, or SPEC-HAL-187 — this document merely traces the implication to its concrete form when ComboBox is embedded inside a TabularSubstrate consumer.

The purpose of this document is to:

- Freeze the canonical interpretation of how these SPECs compose for editor widgets
- Prevent future implementors from re-deriving these constraints (and potentially getting them wrong)
- Provide a single auditable reference for the composition boundary

Nothing here overrides, extends, or reinterprets its ground-truth SPECs.

### Status Justification

This document is Binding despite being non-generative because:

- It freezes a specific interpretation that cannot be otherwise without violating existing SPECs
- It defines the canonical composition pattern for editor widgets inside tabular consumers
- It prevents silent architectural drift where correct-looking code violates contracts that span multiple SPECs
- The composition boundary it traces is not obvious from any single ground-truth SPEC in isolation

---

## 1. Composition Diagram

```
                    ┌──────────────────────────────┐
                    │        Application Glue       │
                    │   (creates ResolverHandle,    │
                    │    provides option snapshots)  │
                    └──────────┬───────────────────┘
                               │
              ┌────────────────┼────────────────────┐
              │                │                     │
              ▼                ▼                     ▼
    ┌─────────────┐   ┌──────────────┐    ┌──────────────────┐
    │  ComboBox    │   │  ComboBox    │    │  ComboBox        │
    │  (widget)    │   │  (widget)    │    │  (widget)        │
    │              │   │              │    │                  │
    │  Owns:       │   │  Owns:       │    │  Owns:           │
    │  - snapshot  │   │  - snapshot  │    │  - snapshot      │
    │  - popup     │   │  - popup     │    │  - popup         │
    │  - input_text│   │  - input_text│    │  - input_text    │
    │  - latch     │   │  - latch     │    │  - latch         │
    └──────┬──────┘   └──────┬──────┘    └──────┬───────────┘
           │                  │                   │
           │ take_committed() │ take_committed()  │ take_committed()
           ▼                  ▼                   ▼
    ┌──────────────────────────────────────────────────────┐
    │              Editor Bridge (consumer-local)           │
    │                                                      │
    │  Follows from SPEC-HAL-201 §10 (widget boundary)     │
    │  and SPEC-HAL-174 (edit lifecycle ownership):         │
    │  - Poll take_committed() from ComboBox              │
    │  - Convert CommittedValue → EditPayload             │
    │  - Emit EditIntent::RequestCommitEdit               │
    │    with (RowId, ColumnId, EditPayload)              │
    │                                                      │
    │  This code lives IN the consumer (ListView, etc.),  │
    │  NOT in ComboBox and NOT in TabularSubstrate.       │
    └──────────────────────┬───────────────────────────────┘
                           │
                           │ TabularIntent::Edit(EditIntent::RequestCommitEdit)
                           ▼
    ┌──────────────────────────────────────────────────────┐
    │              TabularSubstrate                         │
    │                                                      │
    │  Per SPEC-HAL-174:                                    │
    │  - Manages EditState (Idle / Armed / Editing)        │
    │  - Validates target (RowId, ColumnId)                │
    │  - Does NOT interpret EditPayload                    │
    │  - Transitions EditState → Idle on commit            │
    │  - Fires on_intent callback (upstream observes)      │
    └──────────────────────┬───────────────────────────────┘
                           │
                           │ EditIntent observed by upstream
                           ▼
    ┌──────────────────────────────────────────────────────┐
    │              Data Layer (CellMutator contract)        │
    │                                                      │
    │  Per SPEC-HAL-188 §7:                                 │
    │  - Receives (RowId, ColumnId, EditPayload)           │
    │  - Applies or rejects mutation                       │
    │  - Emits DataInvalidationEvent if applied            │
    └──────────────────────────────────────────────────────┘
```

---

## 2. Authority Boundaries as Implied by Existing SPECs

### 2.1 Ownership Derivation

| Concern | Implied Owner | Follows From | Cannot Be Otherwise Without Violating |
|---------|--------------|--------------|--------------------------------------|
| RowId / ColumnId | TabularSubstrate (structural), DataSource (identity origin) | SPEC-HAL-164 INV-A1, SPEC-HAL-187 §4 | SPEC-HAL-187 §4.1 |
| Option data (snapshot) | ComboBox (owned copy) | SPEC-HAL-188A §4 | SPEC-HAL-188A §5 (prohibitions) |
| Option data (source of truth) | Application / Adapter | SPEC-HAL-188 §2.1 | SPEC-HAL-188 §10 |
| Commit decision | Resolver (via ResolverHandle) | SPEC-HAL-201 §5, SPEC-HAL-002 §3.11 | SPEC-HAL-201 §7 (prohibitions) |
| CommittedValue → EditPayload conversion | Consumer (editor bridge code) | SPEC-HAL-201 §10 + SPEC-HAL-174 §3 | SPEC-HAL-174 INV-174-3 |
| EditSession lifecycle | TabularSubstrate | SPEC-HAL-174 INV-174-3 | SPEC-HAL-174 §9 |
| Focus navigation | TabularSubstrate | SPEC-HAL-175 | SPEC-HAL-184 INV-FOCUS-LV-1 |
| Mutation side-effects | Data layer (CellMutator contract) | SPEC-HAL-188 §7 | SPEC-HAL-188 §10 |
| Popup open/close | ComboBox | SPEC-HAL-201 §10 | SPEC-HAL-201 §7 |
| Highlight tracking | ComboBox | SPEC-HAL-201 §10 | SPEC-HAL-201 §7 |

### 2.2 Type Boundary

Two distinct type hierarchies exist. Their separation follows from the fact that ComboBox (SPEC-HAL-201) and TabularSubstrate (SPEC-HAL-164/174) are independent systems with no mutual imports:

- **ComboBox world:** `CommittedValue` (`ExistingOption(OptionId)` | `CustomValue(String)`)
- **TabularSubstrate world:** `EditPayload` (`String` | `Number` | `Bool` | `None`)

These types are in different modules with no `From`/`Into` conversions.
The consumer's editor bridge code is the only place where `CommittedValue` is translated into `EditPayload`. This follows from SPEC-HAL-201 §10 (widget boundary) and SPEC-HAL-174 INV-174-3 (substrate owns edit state, not widgets).

---

## 3. Edit Flow Trace

### 3.1 Common Flow (All Consumers)

1. **Consumer hosts ComboBox as a cell editor widget** inside a row.
2. **Application glue provides:**
   - `ResolverHandle` (created from an `impl SelectionResolver`)
   - Option snapshot via `combo_box.set_options(&[(OptionId, &str)])` — follows from SPEC-HAL-188A §4.2 (replace-only).
3. **User interacts with ComboBox.** ComboBox manages popup, input text, highlight — all within its SPEC-HAL-201 authority.
4. **User commits.** ComboBox calls `attempt_commit(source)` → `resolver.resolve(&input)` — the only commit path, as implied by SPEC-HAL-201 §5.
5. **Resolver returns `ResolverDecision::Commit(value)`.** ComboBox stores in `committed_latch`.
6. **Consumer polls `combo_box.take_committed()`** during its event handling or frame update.
7. **Consumer converts `CommittedValue` → `EditPayload`** (application-specific).
8. **Consumer emits `EditIntent::RequestCommitEdit { row_id, column_id, payload }`** into TabularSubstrate — following from SPEC-HAL-174 intent model.
9. **TabularSubstrate transitions `EditState` → `Idle`.** Fires `on_intent` callback — per SPEC-HAL-174 INV-174-2.
10. **Upstream data layer observes the intent** and applies or rejects the mutation — per SPEC-HAL-188 §7.

### 3.2 ListView-Specific

- **Instantiation:** ComboBox is created as a cell widget inside `CellHost` (or equivalent cell rendering layer).
  ListView does not know it is a ComboBox — it hosts any `dyn Widget`. This follows from SPEC-HAL-187 §4.2 (ListView hosts row widgets only).
- **Snapshot provision:** Application glue (not ListView) calls `set_options()` on the ComboBox via downcast. ListView never calls `set_options()` — it cannot be otherwise without violating SPEC-HAL-188A §5 (widgets are snapshot consumers, adapters are data contract clients).
- **Polling:** ListView's cell hosting layer detects committed values by polling `take_committed()` on the downcast widget. This follows from the consumer's position between widget and substrate.
- **Routing:** ListView emits `EditIntent::RequestCommitEdit` with the `(RowId, ColumnId)` that it already knows from the cell hosting context. ComboBox never sees these IDs — it cannot, without violating SPEC-HAL-201 §10 (widget boundary).

### 3.3 TreeView-Specific

- TreeView is currently a skeleton (56 lines). When implemented, the same flow follows identically — the derivation is consumer-agnostic. TreeView will:
  - Own its own `TabularSubstrate` instance (SPEC-HAL-164)
  - Host cell widgets (including ComboBox) through its own cell hosting layer
  - Emit `EditIntent` through its own intent processing
- Tree expansion/collapse does not interact with ComboBox — these are row-structural operations (SPEC-HAL-172). If a ComboBox is in an editing cell when tree collapse occurs, `cancel_edit_for_destabilization()` fires (SPEC-HAL-174 INV-174-5), which transitions `EditState → Idle`. ComboBox receives `on_blur()` via the focus system, closing its popup without commit.

### 3.4 PropertyEditor-Specific (Future)

- PropertyEditor does not exist yet. When implemented:
  - Each property row is a (RowId, ColumnId) cell
  - ComboBox is instantiated for enum/choice properties
  - PropertyEditor acts as its own consumer with its own `TabularSubstrate` (SPEC-HAL-164)
  - The editor bridge pattern follows identically: poll `take_committed()`, convert, emit `EditIntent`
- PropertyEditor is the priority consumer for ComboBox (properties frequently require choice selection)

### 3.5 Grid-Specific (Future, If Distinct)

- If Grid is a separate widget (not ListView), the same derivation applies.
- Grid would own its own `TabularSubstrate` and follow the same editor bridge.
- No Grid-specific exceptions exist.

---

## 4. Couplings Proven Invalid by Existing Contracts

### 4.1 ComboBox Accessing RowId / ColumnId

**VERIFIED: Does not occur.**

`combo_box.rs` imports:
- `crate::core::event::*`
- `crate::core::layout::*`
- `crate::core::math::*`
- `crate::core::paint::*`
- `crate::core::scene_identity::*`
- `crate::core::widget::*`
- `crate::core::widget_tree::{NodeId, OverlayHitRegion, TreeWidgetId}`
- `crate::model::selection_resolver::*`
- `crate::utils::id::WidgetInternalId`

No import of `RowId`, `ColumnId`, `TabularSubstrate`, `EditIntent`, `EditPayload`, `EditSession`, `DataAdapter`, or any tabular_view types. This coupling cannot exist without violating SPEC-HAL-201 §10 (widget authority boundary).

### 4.2 ComboBox Touching Adapters or Data Contracts

**VERIFIED: Does not occur.**

ComboBox stores `Vec<(OptionId, String)>` as a presentation snapshot (SPEC-HAL-188A). This is populated by `set_options()` which takes `&[(OptionId, &str)]` — a borrowed view. ComboBox never accesses `DataSource`, `CellReader`, `CellMutator`, or any data contract. This coupling cannot exist without violating SPEC-HAL-188A §5 (explicit prohibitions).

### 4.3 TabularSubstrate Interpreting ComboBox Internals

**VERIFIED: Does not occur.**

TabularSubstrate receives `EditPayload` (opaque). From `intents.rs:194-195`: "SPEC-HAL-174: TabularSubstrate does not interpret payloads." TabularSubstrate's `handle_edit_intent()` processes lifecycle transitions only — it never inspects `EditPayload` contents. There is no reference to `ComboBox`, `CommittedValue`, `OptionId`, `ResolverHandle`, or any selection_resolver types in any tabular_view source file. This coupling cannot exist without violating SPEC-HAL-174 (payload opacity) and SPEC-HAL-164 INV-A1 (substrate authority scope).

### 4.4 ListView Inferring Semantics from Widget State

**VERIFIED: Does not occur (structurally prevented).**

ListView hosts `dyn Widget` objects. It cannot inspect ComboBox state without explicit downcast. The editor bridge (polling `take_committed()`) requires downcast, but this is the consumer's explicit, auditable integration code — not implicit inference. ListView does not read `popup_open`, `highlighted_id`, `focus_zone`, or `input_text` from ComboBox for any purpose other than the polling pattern. Inference from widget state would violate SPEC-HAL-187 §4.1 (ListView does not own selection or focus semantics).

### 4.5 Resolver Being Bypassed or Duplicated

**VERIFIED: Cannot occur.**

ComboBox has exactly one commit path: `attempt_commit()` at line 100. This function is the only caller of `self.resolver.resolve()`. All commit triggers (Enter key, pointer click on row) funnel through `attempt_commit()`. There is no second commit path, no shortcut, no "if only one option" auto-commit. This follows from SPEC-HAL-201 §5 (resolver is sole commit authority) and SPEC-HAL-002 §3.11 (no interpretive intent resolution).

The consumer's editor bridge does not call the resolver — it only observes the output (`CommittedValue`) via `take_committed()`.

---

## 5. Virtualization Safety

### 5.1 Editor Lifetime vs Row Recycling

When a row containing an editing ComboBox scrolls out of the viewport:

1. TabularSubstrate detects the destabilization (scroll during active edit).
2. `cancel_edit_for_destabilization()` fires — follows from SPEC-HAL-174 INV-174-5.
3. `EditState` transitions to `Idle`.
4. Pre-edit focus is restored.
5. ComboBox receives `on_blur()` → `close_without_commit()` fires.
6. `input_text` is preserved. `popup_open` and `highlighted_id` are cleared.
7. Row widget may be recycled (CellHost rebinding). ComboBox instance may be dropped.

No data loss occurs. The edit was cancelled via destabilization, not silently committed.

### 5.2 Base Order Replaced Mid-Edit

When `base_row_order_epoch` bumps during an active edit — per SPEC-HAL-187 INV-ROW-BASE-REPLACE:

1. Base order replacement triggers full row model rebuild.
2. Active edit is cancelled via destabilization.
3. ComboBox receives `on_blur()` and closes cleanly.
4. The RowId that was being edited may or may not exist in the new base order.
5. No stale references persist — focus anchor is RowId-based, not index-based. This follows from SPEC-HAL-187 INV-ROW-INDEX-EPHEMERAL.

### 5.3 Cell Capabilities Change Mid-Focus

If `CellCapabilities` change for the cell being edited (e.g., editability revoked):

1. The data layer emits `DataInvalidationEvent::PolicyInvalidated` — per SPEC-HAL-188 §9.
2. Consumer detects policy change and cancels the edit (application-level logic).
3. TabularSubstrate transitions `EditState → Idle`.
4. ComboBox receives `on_blur()`.

All behavior is RowId-stable and index-free. ComboBox never references indices. TabularSubstrate's `EditSession` stores `CellId` (which is `(RowId, ColumnId)`), never indices. This is implied by SPEC-HAL-187 INV-ROW-INDEX-EPHEMERAL and SPEC-HAL-174 INV-174-1.

---

## 6. Failure Modes

### 6.1 Resolver Rejects Commit

From `combo_box.rs:127-129`:
```rust
ResolverDecision::Reject(_) => {
    // No fallback. No retry. No state mutation.
}
```

ComboBox remains in its current state. Popup stays open. No `committed_latch` is set. Consumer's `take_committed()` returns `None`. No `EditIntent` is emitted. No implicit retry, no hidden commit. This follows from SPEC-HAL-201 §5 (widget acts on decision, does not second-guess) and SPEC-HAL-201 §7 (no fallback behavior).

### 6.2 Backend Rejects Mutation

The data layer (CellMutator) receives `EditPayload` and decides `MutationResult::Rejected`. This happens entirely in the data layer — per SPEC-HAL-188 §7. TabularSubstrate has already transitioned `EditState → Idle`. The rejection is communicated back via `DataInvalidationEvent` or application-level callback. ComboBox is not involved — it already closed its popup and cleared its latch.

### 6.3 Edit Cancelled via Blur / Escape

**Blur:** `on_blur()` → `close_without_commit()`:
- `popup_open = false`
- `highlighted_id = None`
- `input_text` preserved
- No `committed_latch` set
- No `EditIntent` emitted

**Escape:** `handle_event()` → `close_without_commit()` (same path).

Consumer detects that `take_committed()` returned `None` and emits `EditIntent::RequestCancelEdit`. TabularSubstrate restores pre-edit focus — per SPEC-HAL-174 INV-174-2.

### 6.4 Confirmed: No Implicit Retries, No Hidden Commits

- `attempt_commit()` is called exactly once per explicit user action.
- `close_without_commit()` never touches `committed_latch`.
- `on_blur()` never calls `attempt_commit()`.
- There is no timer, no deferred commit, no "commit on close" behavior.

This follows from SPEC-HAL-201 §7 (explicit prohibitions) and SPEC-HAL-002 §4.11 (explicit commit authority).

---

## 7. Derived Invariants (Implied, Not Introduced)

Every invariant below is a concrete restatement of an existing SPEC constraint applied to the ComboBox-in-consumer composition. None are new.

### Authority Invariants

**INV-CB-AUTH-1:** ComboBox does not access RowId, ColumnId, or any TabularSubstrate type. *(Follows from SPEC-HAL-201 §10 widget boundary.)*

**INV-CB-AUTH-2:** ComboBox does not access DataSource, DataAdapter, CellReader, CellMutator, or any data contract type. *(Follows from SPEC-HAL-188A §5 prohibitions.)*

**INV-CB-AUTH-3:** Only the consumer's editor bridge code converts `CommittedValue` to `EditPayload`. *(Follows from SPEC-HAL-201 §10 + SPEC-HAL-174 INV-174-3.)*

**INV-CB-AUTH-4:** TabularSubstrate does not interpret `EditPayload` contents. *(Restates SPEC-HAL-174 payload opacity.)*

**INV-CB-AUTH-5:** Only `ResolverHandle.resolve()` produces `CommittedValue`. *(Restates SPEC-HAL-201 §5 sole commit authority.)*

### Lifecycle Invariants

**INV-CB-LIFE-1:** ComboBox does not emit EditIntent. Only the consumer emits EditIntent. *(Follows from SPEC-HAL-201 §10: widget boundary excludes tabular_view types.)*

**INV-CB-LIFE-2:** `take_committed()` is the only mechanism for the consumer to observe ComboBox commit output. *(Follows from SPEC-HAL-201 §10 + SPEC-HAL-188A §4.2 replace-only snapshot.)*

**INV-CB-LIFE-3:** Edit cancellation (blur, Escape, destabilization) does not produce a committed value. *(Follows from SPEC-HAL-201 §7: no implicit commits.)*

**INV-CB-LIFE-4:** Virtualization-driven row recycling triggers edit cancellation before widget destruction. *(Follows from SPEC-HAL-174 INV-174-5 destabilization cancel.)*

### Focus Invariants

**INV-CB-FOCUS-1:** ComboBox manages its own internal popup focus zone (`PopupFocusZone`). This is invisible to TabularSubstrate's `FocusState`. *(Follows from SPEC-HAL-175: substrate focus is cell-level, not widget-internal.)*

**INV-CB-FOCUS-2:** TabularSubstrate focus and ComboBox popup focus are orthogonal. TabularSubstrate tracks which cell has focus. ComboBox tracks which popup zone is active. These never cross. *(Follows from SPEC-HAL-184 INV-FOCUS-LV-1: only widgets receive focus; cells and rows do not.)*

**INV-CB-FOCUS-3:** ComboBox popup close on blur does not emit focus intents into TabularSubstrate. Focus lifecycle is managed by TabularSubstrate, not by editor widgets. *(Follows from SPEC-HAL-175 + SPEC-HAL-184 INV-FOCUS-EXIT-1.)*

### Snapshot Invariants

**INV-CB-SNAP-1:** Option snapshots are replaced atomically via `set_options()`. No partial updates. *(Restates SPEC-HAL-188A §4.2 replace-only.)*

**INV-CB-SNAP-2:** ComboBox does not derive semantics from snapshot contents. Labels are rendered, never interpreted. *(Restates SPEC-HAL-201 §7 prohibitions.)*

**INV-CB-SNAP-3:** Snapshot staleness (options changed while popup is open) is not ComboBox's concern. The consumer is responsible for closing the editor or refreshing the snapshot if data changes. *(Follows from SPEC-HAL-188A §4: snapshot is authority-free.)*

---

## 8. Consumer-Specific Notes

### 8.1 ListView

- ComboBox is hosted as any `dyn Widget` inside `CellHost`.
- ListView's cell hosting layer is the natural location for the editor bridge.
- `CellHost` already manages cell widget lifecycle (creation, rebinding, dirty flags). The polling of `take_committed()` fits naturally into this lifecycle.
- No ListView code changes are required to host ComboBox — it is a standard focusable widget.

### 8.2 TreeView

- Skeleton only. When implemented, identical editor bridge pattern follows.
- Tree collapse during ComboBox edit is handled by TabularSubstrate's destabilization mechanism (same as scroll) — per SPEC-HAL-174 INV-174-5.
- No ComboBox-specific tree logic is needed.

### 8.3 PropertyEditor (Priority)

- Does not exist yet. This is the highest-priority consumer for ComboBox.
- PropertyEditor will likely have a simpler cell model (property name column + value column).
- The value column will host ComboBox for enum/choice properties.
- The editor bridge is simpler: one column, clear `CommittedValue` → `EditPayload` mapping.
- PropertyEditor's `set_options()` calls are driven by property metadata, not by row data.

### 8.4 Grid (If Distinct from ListView)

- No Grid widget exists. If implemented as a distinct widget, same derivation applies.
- No Grid-specific exceptions or accommodations.

---

## 9. Explicit Non-Goals

This audit intentionally does not solve:

- **CommittedValue → EditPayload conversion API.** The conversion is consumer-specific policy. No generic `From` impl is provided or endorsed.
- **Snapshot refresh timing.** When and how option snapshots are refreshed during an edit session is application policy, not widget or substrate responsibility.
- **Resolver selection per cell.** Which resolver a cell uses is application glue, not substrate or widget logic.
- **Multi-editor coordination.** Two ComboBoxes open simultaneously is prevented by the single-EditSession invariant (SPEC-HAL-174 INV-174-1). No additional coordination is needed.
- **Undo/redo integration.** Edit payloads are fire-and-forget from the UI perspective. Undo is data layer policy.
- **Validation feedback display.** If the data layer rejects a mutation, how that feedback reaches the user is application UX, not widget contract.
- **ComboBox as a standalone (non-editor) widget.** ComboBox works identically outside TabularSubstrate consumers. The editor bridge simply does not exist. `take_committed()` is polled by whatever parent hosts the ComboBox.

---

## 10. Template Applicability

This audit traces the composition pattern for all future editor widgets:

- DatePicker
- ColorPicker
- NumericSpinner
- TagEditor
- Any widget that produces a committed value inside a TabularSubstrate cell

The pattern, as implied by SPEC-HAL-201 + SPEC-HAL-174 + SPEC-HAL-188A:

1. Widget owns its internal state and interaction.
2. Widget produces a committed value via a one-shot latch (`take_committed()` or equivalent).
3. Consumer polls the latch.
4. Consumer converts widget-specific committed value to `EditPayload`.
5. Consumer emits `EditIntent::RequestCommitEdit`.
6. TabularSubstrate manages lifecycle.
7. Data layer decides mutation.

Widget cannot know it is inside a cell (SPEC-HAL-201 §10). Consumer cannot know what the widget decided (SPEC-HAL-174 payload opacity). TabularSubstrate cannot know what the payload means (SPEC-HAL-174 INV-174-7). Data layer cannot know which widget produced the payload (SPEC-HAL-188 §7).

Each layer sees only its immediate interfaces. This is not a new contract — it is the only arrangement that does not violate existing ones.
