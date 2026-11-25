# SPEC-HAL-312A: **EventLog Row Expansion**

### *Phase 2 — Deterministic Row Expansion for EventLog*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-312 (EventLog Phase 1)
* SPEC-HAL-188 (DataSource / DataInvalidationEvent)
* SPEC-HAL-200 (IntentPolicy / ActivationIntent)

---

# 1. Purpose

This SPEC defines the row expansion model for EventLog Phase 2: the ability to expand a row to reveal its detail content (full message text, stack traces, metadata), and collapse it back.

The expansion model is **fully deterministic**. Every height in the system is known exactly at toggle time. No measurement, no approximation, no width-dependent computation, no per-frame recalculation. This is the critical property that distinguishes EventLog expansion from Console's variable-height wrap mode.

---

# 2. Existing Infrastructure

Phase 2 builds on infrastructure that already exists and is verified working:

| Component | Location | Status |
|-----------|----------|--------|
| `RowHeights` variable mode | `src/model/tabular/row_heights.rs` | `set_height(index, h)` bumps `height_version` |
| `VirtualizationEngine` variable path | `src/engines/virtualization/mod.rs` | O(N) prefix-sum rebuild on version change, O(log N) binary search |
| Body paint variable heights | `src/widgets/list_view/body.rs:243-271` | Uses `row_projection.y_offset` and `.height` from realization |
| `CellPainter` receives full rect | `src/widgets/list_view/cell_painter.rs:28-38` | `rect.height` reflects actual row height |
| `SeverityLookup` injection pattern | `src/widgets/event_log/severity_painter.rs:21` | `Rc<RefCell<dyn SeverityLookup>>` injected at construction |
| `EventLogEntry.detail` field | `src/widgets/event_log/types.rs:57` | `Option<String>`, exists since Phase 1 |
| `ExpansionPolicy` enum | `src/widgets/event_log/event_log.rs:36-40` | `NoExpand`, `SingleExpand`, `MultiExpand` — stored, not yet enforced |
| `DataInvalidationEvent` | `src/widgets/tabular_view/data_adapter.rs:172-189` | Marked FROZEN but "Variants may be added" |
| `ActivationIntent::Activate` | `src/widgets/tabular_view/intents.rs:370-373` | `Activate { row_id }` — exists, routed through `TabularIntent` |
| `ActivationMode::ExplicitOnly` | `src/widgets/tabular_view/intent_policy.rs:53-54` | "Enter or programmatic" — exactly what EventLog needs |

Nothing fundamental needs to be built from scratch. Phase 2 is integration work: threading a new concern (expansion state) through existing extension points.

---

# 3. Design Principle: Deterministic Heights

Console (SPEC-HAL-305) wrap mode was hard because heights were **content-dependent and width-dependent** — a row's height changed whenever the viewport width changed, requiring per-frame measurement, dual-source geometry management, and the pathology patterns documented in SPEC-PM-009 and SPEC-PM-010.

EventLog expansion is fundamentally different:

- A collapsed row has height `COLLAPSED_HEIGHT` (24px from config).
- An expanded row has height `COLLAPSED_HEIGHT + DETAIL_PANEL_HEIGHT`.
- `DETAIL_PANEL_HEIGHT` is a fixed constant (defined in `EventLogConfig`).
- Both values are known at toggle time. Neither depends on content, viewport width, font metrics, or any other dynamic input.

Consequences:

- `RowHeights::set_height` is called exactly once per toggle with an exact value.
- `VirtualizationEngine` rebuilds prefix sums from exact values — O(N) additions, no approximation.
- No "measure pass" exists. No "approximate then correct" exists. No dual-source drift is possible.
- The PM-009 and PM-010 pathology classes do not apply.

---

# 4. Expansion State Model

## 4.1 State Owner

EventLog owns expansion state as a `HashSet<RowId>` of expanded row IDs.

```rust
pub struct EventLog {
    list_view: Option<ListView<SharedEventLogSource>>,
    source: Rc<RefCell<EventLogDataSource>>,
    config: EventLogConfig,
    source_column_id: ColumnId,
    /// Phase 2: Set of currently expanded row IDs.
    expanded: HashSet<RowId>,
}
```

## 4.2 Policy Enforcement

`ExpansionPolicy` (already defined, not yet enforced) governs toggle behavior:

- **`NoExpand`** — `toggle_expansion` is a no-op. No row can be expanded.
- **`SingleExpand`** — at most one row expanded at a time. Toggling a row collapses the previously expanded row (if any) and expands the new one. Toggling the already-expanded row collapses it.
- **`MultiExpand`** — any number of rows can be expanded simultaneously. Each toggle is independent.

## 4.3 Toggle API

```rust
impl EventLog {
    /// Toggle expansion for a row. Respects ExpansionPolicy.
    ///
    /// Returns true if the expansion state changed.
    pub fn toggle_expansion(&mut self, row_id: RowId) -> bool { ... }

    /// Query whether a row is expanded.
    pub fn is_expanded(&self, row_id: RowId) -> bool { ... }

    /// Collapse all expanded rows.
    pub fn collapse_all(&mut self) { ... }
}
```

`toggle_expansion` does three things:
1. Updates `self.expanded` (insert or remove, respecting policy).
2. Computes the new height for each affected row.
3. Pushes `DataInvalidationEvent::RowHeightsChanged` through the DataSource for each changed row.

## 4.4 Height Computation

```rust
fn row_height(&self, row_id: RowId, expanded: bool) -> f32 {
    if expanded {
        self.config.row_height + self.config.detail_panel_height
    } else {
        self.config.row_height
    }
}
```

`detail_panel_height` is a new field on `EventLogConfig`, defaulting to 72.0 (3 lines at ~20px + padding). Fixed, not content-dependent. Consumer can override at construction.

---

# 5. Height Propagation

## 5.1 New DataInvalidationEvent Variant

```rust
pub enum DataInvalidationEvent {
    // ... existing variants ...

    /// Row heights have changed. The substrate must update RowModel heights
    /// for the specified rows.
    ///
    /// Heights are exact and final — no measurement or approximation required.
    RowHeightsChanged {
        changes: Vec<(RowId, f32)>,
    },
}
```

This variant is emitted by EventLog's DataSource when `toggle_expansion` is called.

## 5.2 DataAdapter Processing

In `DataAdapter::process_invalidation` (data_adapter.rs:393-435), add a new match arm:

```rust
DataInvalidationEvent::RowHeightsChanged { changes } => {
    result.row_heights_changed = true;
    result.height_changes.extend(changes);
}
```

`AdapterInvalidationResult` gains:
```rust
pub row_heights_changed: bool,
pub height_changes: Vec<(RowId, f32)>,
```

## 5.3 Substrate → RowModel Update

When the TabularSubstrate processes `AdapterInvalidationResult` and sees `row_heights_changed == true`, it:

1. For each `(row_id, new_height)` in `height_changes`:
    - Looks up the row's index in RowModel via its identity registry.
    - Calls `RowHeights::set_height(index, new_height)`.
2. `set_height` bumps `height_version`.
3. On the next `resolve_layout_geometry` call, `VirtualizationEngine::ensure_prefix_sums` detects the stale version and rebuilds.

The RowModel must be in **variable-height mode** for this to work. EventLog must configure the ListView's RowModel as variable-height at construction time, even though all rows start at the same collapsed height. This is because `RowHeights::set_height` is a no-op in fixed-height mode.

Alternative: stay in fixed-height mode and switch to variable-height mode on first expansion. This avoids the O(N) prefix-sum cost until expansion is actually used. Decision: start in fixed-height mode, switch to variable on first expansion. The switch sets all rows to the fixed height explicitly, then applies the expansion height. One-time cost, O(N).

---

# 6. CellPainter Expansion Awareness

## 6.1 ExpansionLookup Trait

Same pattern as `SeverityLookup`:

```rust
pub trait ExpansionLookup {
    /// Whether the given row is currently expanded.
    fn is_expanded(&self, row_id: RowId) -> bool;
}
```

`EventLogDataSource` implements this (or EventLog injects a separate lookup object). The CellPainter receives it via `Rc<RefCell<dyn ExpansionLookup>>` at construction.

## 6.2 Expanded Cell Rendering

When `is_expanded(row_id)` returns true:

- **Severity column**: renders the indicator at the top (same as collapsed). Bottom area is blank.
- **Timestamp column**: renders the timestamp at the top. Bottom area is blank.
- **Source column**: renders the source at the top. Bottom area is blank.
- **Message column**: renders the message text at the top (same as collapsed). In the remaining vertical space below, renders the detail text.

The detail area occupies `rect.y + config.row_height` to `rect.y + rect.height`. The message column painter clips its detail rendering to this sub-rect.

Detail rendering:
- Read `entry.detail` from the DataSource (via a new `read_detail(row_id)` method on EventLogDataSource, or by reading the entry directly via the `SeverityLookup`-style injection).
- If `detail` is `Some(text)`: render the text with reduced font size, dimmed color, left-aligned, wrapping within the available width.
- If `detail` is `None`: render the full `message` text wrapped (useful when message was truncated in collapsed view).

## 6.3 Visual Separation

A thin horizontal separator line at `rect.y + config.row_height` visually divides the collapsed header from the detail area. Painted by the message column painter (or by a dedicated separator pass).

---

# 7. Keyboard Activation

## 7.1 Activation Chain

The chain already exists:

1. User presses Enter on a focused row.
2. ListView's `handle_keyboard` currently does NOT handle Enter (falls through to `_ => None`).
3. **New**: ListView handles Enter by consulting the IntentPolicy with `IntentInput::KeyPress { key: Enter, modifiers }`.
4. IntentPolicy (with `ActivationMode::ExplicitOnly`) interprets Enter as `ActivationIntent::Activate { row_id }`.
5. Substrate receives the intent via `handle_intent`.
6. **New**: Substrate (or ListView) emits the activation through a callback or event that EventLog can observe.

## 7.2 Activation Callback

EventLog needs to know when a row is activated. Two options:

**Option A — Callback on ListView.** EventLog registers an `on_activate: Box<dyn Fn(RowId)>` callback on ListView before handing it to the widget tree. ListView calls it when ActivationIntent is dispatched.

**Option B — DataSource event.** The activation intent is forwarded through the DataSource as a new invalidation event. EventLog's DataSource receives it and toggles expansion.

Option A is simpler and doesn't pollute the DataSource interface with activation semantics. Activation is a UI concern, not a data concern.

```rust
impl<S: DataSource + 'static> ListView<S> {
    pub fn set_activation_callback(&mut self, callback: Box<dyn Fn(RowId)>) { ... }
}
```

When `handle_intent` receives `TabularIntent::Activation(ActivationIntent::Activate { row_id })`, it calls the callback.

EventLog wires this before `take_list_view()`:

```rust
let expanded_clone = self.expanded.clone(); // Rc<RefCell<HashSet<RowId>>>
let source_clone = self.source.clone();
event_log.list_view_mut().set_activation_callback(Box::new(move |row_id| {
    // Toggle expansion and push height-change invalidation
    ...
}));
```

## 7.3 Space Key

Space toggles expansion (standard in tree views and expandable lists). Add Space alongside Enter in the activation key set. The IntentPolicy already handles this — `ExplicitOnly` mode just needs to recognize Space as an activation key.

## 7.4 Enter Handling in ListView

ListView's `handle_keyboard` (list_view.rs:366-458) currently handles only navigation keys. Add Enter and Space:

```rust
KeyCode::Enter | KeyCode::Space => {
    // Activation: consult policy, dispatch ActivationIntent
    ...
}
```

This goes in the existing `match event.key_code { ... }` block, as a new arm before the `_ => None` fallback.

---

# 8. EventLogConfig Changes

```rust
pub struct EventLogConfig {
    pub policy: EventLogPolicy,
    pub row_height: f32,
    pub font_size: f32,
    pub show_source_column: bool,
    pub timestamp_formatter: Option<TimestampFormatter>,
    pub expansion_policy: ExpansionPolicy,
    /// Height of the detail panel area when a row is expanded.
    /// Total expanded row height = row_height + detail_panel_height.
    pub detail_panel_height: f32,        // NEW — default 72.0
    pub capacity: usize,
}
```

---

# 9. Invariants

**INV-312A-EXACT-HEIGHT**: Every row height in the system is an exact constant known at toggle time. No row height depends on content measurement, viewport width, font metrics, or any other dynamic input. The height is `config.row_height` when collapsed and `config.row_height + config.detail_panel_height` when expanded. No other values are possible.

**INV-312A-SINGLE-SOURCE**: Expansion state is owned exclusively by EventLog's `HashSet<RowId>`. No other component stores, caches, or derives expansion state. CellPainter queries it via `ExpansionLookup`; RowModel receives height values through invalidation events. Both are projections of the single source.

**INV-312A-POLICY-ENFORCE**: `ExpansionPolicy` is enforced at toggle time, not at render time. If policy is `SingleExpand`, toggling row B when row A is expanded collapses A and expands B in a single `toggle_expansion` call. Both height changes are emitted in the same invalidation batch.

**INV-312A-VARIABLE-SWITCH**: RowModel starts in fixed-height mode. On first expansion, RowModel switches to variable-height mode (initializing all rows to the fixed height, then applying the expansion height). This avoids O(N) prefix-sum cost until expansion is actually used. The switch is irreversible within a session.

**INV-312A-ACTIVATION-AUTHORITY**: Activation flows through the IntentPolicy system. No widget bypasses the policy to toggle expansion directly. The chain is: KeyDown → IntentInput → IntentPolicy → ActivationIntent → callback → toggle_expansion → DataInvalidationEvent.

---

# 10. Forbidden Behaviors

**FORBID-312A-MEASURE**: No measurement pass for detail content height. Height is a config constant, not computed from content.

**FORBID-312A-WIDTH-DEPENDENT**: No row height may depend on column width or viewport width. This eliminates the dual-source geometry drift (SPEC-PM-009) and incremental cache fragility (SPEC-PM-010) pathologies by construction.

**FORBID-312A-DIRECT-ROWMODEL**: EventLog must not reach into RowModel directly. Height changes flow through DataInvalidationEvent → DataAdapter → Substrate → RowModel. This respects the TabularView authority boundary.

**FORBID-312A-PAINT-TIME-STATE**: CellPainter must not mutate expansion state during paint. `ExpansionLookup::is_expanded` is a read-only query. Toggle occurs only through the activation callback.

---

# 11. Implementation Breakdown

Each item is a self-contained CC task. Order matters — each builds on the previous.

**Task 1 — DataInvalidationEvent::RowHeightsChanged**

Add the new variant to `DataInvalidationEvent`. Add `row_heights_changed: bool` and `height_changes: Vec<(RowId, f32)>` to `AdapterInvalidationResult`. Add the match arm in `DataAdapter::process_invalidation`. Add Substrate handling to translate `(RowId, f32)` into `RowModel::set_height` calls (requires RowId → index lookup). Add RowModel fixed→variable mode switch when first height change arrives for a fixed-height model.

Files: `data_adapter.rs`, `row.rs` (RowModel index_of lookup), Substrate processing code.

**Task 2 — EventLog expansion state + toggle API**

Add `expanded: HashSet<RowId>` to EventLog. Implement `toggle_expansion`, `is_expanded`, `collapse_all`. Wire `toggle_expansion` to emit `RowHeightsChanged` through the DataSource. Add `detail_panel_height` to EventLogConfig. Enforce `ExpansionPolicy` in `toggle_expansion`.

Files: `event_log.rs`, `data_source.rs`, `types.rs` (if needed).

**Task 3 — ListView Enter/Space activation**

Add Enter and Space handling to `ListView::handle_keyboard`. Consult IntentPolicy, dispatch `ActivationIntent::Activate`. Add `set_activation_callback` on ListView. Wire substrate's `handle_intent` for the Activation variant to call the callback.

Files: `list_view.rs`.

**Task 4 — Expansion-aware CellPainter**

Add `ExpansionLookup` trait. Implement on `EventLogDataSource`. Extend `SeverityPainter` (or create `ExpandableSeverityPainter`) to query expansion state and render detail content in the expanded area of the message column. Wire the lookup at EventLog construction.

Files: `severity_painter.rs` (or new `expansion_painter.rs`), `data_source.rs`.

**Task 5 — Demo + integration test**

Update `eventlog_demo.rs` to use `ExpansionPolicy::SingleExpand`, populate entries with `with_detail(...)`, demonstrate Enter/Space toggle. Verify: expand → row grows → detail visible → collapse → row shrinks → scroll position stable.

Files: `examples/eventlog_demo.rs`.

---

# 12. Scroll Position Stability

When a row above the viewport top expands, rows below shift down. When a row below the viewport top expands, the viewport anchor is unaffected.

For SingleExpand: one row expands, one collapses (or just one toggles). The net height change is bounded: at most `±detail_panel_height`. Scroll position adjustment: if the expanding row is above the current scroll position, add `detail_panel_height` to scroll_y. If the collapsing row is above, subtract. This preserves the visual anchor.

For MultiExpand: each toggle changes one row's height. Same adjustment logic per toggle.

This adjustment happens inside `toggle_expansion` as part of the invalidation batch, before the next layout. The substrate (or EventLog) adjusts the scroll position through the existing scroll API.

---

# 13. Future Extensions (Out of Scope)

- **Content-dependent detail height**: computing wrapped line count from `entry.detail` text. Would reintroduce width-dependency. Deferred to Phase 2.5 if needed. Current fixed-height panel handles most cases.
- **Detail panel as embedded widget subtree**: rich content (links, tables, syntax highlighting) in the detail area. Requires sub-widget composition within a CellPainter, which is a larger architectural question.
- **Animated expansion/collapse**: transitioning row height over N frames. The animation system could interpolate the height value, but this reintroduces per-frame height changes. Deferred.
- **Per-entry detail_panel_height**: different entries with different panel heights. Requires `entry.detail_height: Option<f32>`. Simple extension of the current model — height is still exact and known at push time.

---

# 14. Conclusion

EventLog row expansion is deterministic integration work across existing extension points. Every height is a known constant. The infrastructure exists: variable RowHeights, VirtualizationEngine prefix sums, CellPainter with injected lookups, ActivationIntent through IntentPolicy, DataInvalidationEvent with sanctioned variant addition.

No new fundamental mechanisms are required. No approximation. No measurement. No pathology risk. Five focused implementation tasks, each self-contained and independently verifiable.
