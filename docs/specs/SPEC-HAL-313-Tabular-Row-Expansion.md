# SPEC-HAL-313: **Tabular Row Expansion (Substrate Primitive)**

### *Versatile, substrate-layer row expansion with widget-subtree content*

**Author:** JC
**Status:** Active
**Supersedes:** SPEC-HAL-312A §4–§8 (expansion state, policy, height propagation, activation, cell-painter detail rendering)
**Depends on:**

* SPEC-HAL-188 (DataSource / DataInvalidationEvent)
* SPEC-HAL-200 (IntentPolicy / ActivationIntent)
* SPEC-HAL-176 (Keyboard Authority Modes)

**Preserves from SPEC-HAL-312A:**

* `RowHeights::set_height` height plumbing
* `DataInvalidationEvent::RowHeightsChanged` pipeline
* `AdapterInvalidationResult.height_changes` drain in `sync_data_source`
* `EventContext::invalidate_subtree` (Task 8 fix)

---

# 1. Purpose

Row expansion is a first-class substrate primitive. Both `TabularSubstrate` and `HierarchicalTabularSubstrate` support it. Consumers place arbitrary widget subtrees in the expanded region.

---

# 2. Design Principles

**P1 — Expansion is orthogonal to selection.** A row can be expanded and unselected; selected and unexpanded; both; neither. No code reads one to decide the other.

**P2 — Expanded content is a widget subtree.** Consumers supply a factory `Fn(RowId) -> Box<dyn Widget>`. The substrate has no knowledge of what the consumer renders inside the expanded region.

**P3 — Heights are exact and deterministic.** Collapsed rows use `COLLAPSED_HEIGHT`. Expanded rows use `COLLAPSED_HEIGHT + resolved_expanded_height(policy, widget)`. Resolved height is computed at layout time from the child's natural size and the height policy. No estimates, no approximations.

**P4 — Virtualization is partially preserved.** Collapsed rows remain cell-painted. Only expanded rows allocate widget subtrees. Memory cost is O(expanded_count), not O(total_row_count).

**P5 — Keybinds route through IntentPolicy.** No direct `KeyCode` matching survives in `handle_keyboard`. All key interpretation flows through `IntentPolicy::interpret`, which emits `ExpansionIntent` variants.

**P6 — No consumer knowledge in the substrate.** The tokens `event_log`, `EventLog`, `severity`, `detail_text` do not appear in any file under `src/model/tabular/`, `src/widgets/tabular_view/`, `src/widgets/list_view/`, `src/engines/hierarchical/`, or `src/widgets/tree_view/`.

---

# 3. Architecture

## 3.1 Model Layer — `RowExpansion`

**Location:** `src/model/tabular/row_expansion.rs` (new file, sibling of `row_heights.rs`).

```rust
pub struct RowExpansion {
    expanded: IndexSet<RowId>,
    policy: ExpansionPolicy,
    height_policy: ExpandedHeightPolicy,
    version: u64,
}

pub enum ExpansionPolicy {
    Disabled,
    SingleExpand,
    MultiExpand,
}

pub enum ExpandedHeightPolicy {
    Fixed(f32),
    Cap(f32),
    Natural,
}
```

**Contract:**

- Stores set membership, policy, and version counter. Nothing else.
- MUST NOT contain widget instances, NodeIds, height values, hit-testing logic, or any higher-layer concern.
- MUST NOT hold or reference the widget factory. Factory ownership lives on the body layer.
- Engines and substrates depend on this type. `RowExpansion` MUST NOT depend on engine or widget types.

## 3.2 Substrate Layer

Both `TabularSubstrate` and `HierarchicalTabularSubstrate` embed a `RowExpansion` field.

```rust
pub fn expansion(&self) -> &RowExpansion;
pub fn expansion_mut(&mut self) -> &mut RowExpansion;
pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy);
pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy);
```

Intent processing (`handle_intent`) gains an `ExpansionIntent` arm that mutates `RowExpansion` state. Height change notifications flow through `DataInvalidationEvent::RowHeightsChanged`.

## 3.3 Intent Layer

**Location:** `src/widgets/tabular_view/intents.rs`.

```rust
pub enum TabularIntent {
    // ... existing variants ...
    Expansion(ExpansionIntent),
}

pub enum ExpansionIntent {
    Expand { row_id: RowId },
    Collapse { row_id: RowId },
    Toggle { row_id: RowId },
    ToggleWithCapture { row_id: RowId },
}
```

`ExpansionIntent` is distinct from `TreeIntent`. `TreeIntent` is hierarchical-tree topology (show/hide child nodes). `ExpansionIntent` is row-detail expansion (show/hide detail subtree for a row). A TreeView node can be both tree-expanded and row-expanded independently.

### IntentPolicy Key Mappings

When expansion policy is not `Disabled`:

| Key | Condition | Intent Emitted |
|---|---|---|
| `Enter` | Always | `Toggle { focused_row }` |
| `Space` | Always | `Toggle { focused_row }` |
| `l` | Focused row is collapsed | `Expand { focused_row }` |
| `h` | Focused row is expanded | `Collapse { focused_row }` |
| `i` | Always | `ToggleWithCapture { focused_row }` |
| Double-click | Body hit | `Toggle { row_id_at_pointer }` |

**Fall-through rule:** If the key does not produce an `ExpansionIntent` (e.g. `h` on a row already collapsed, or policy is `Disabled`), the widget returns `false` from `handle_event`. The Action System fallback dispatches the system binding (`h` → `FOCUS_LEFT`, `l` → `FOCUS_RIGHT`). This preserves system-wide hjkl meaning and makes the override purely additive.

**Routing order (verified from `event_router.rs:4014`):** Widget sees key FIRST. If widget returns `true`, Action System never runs. If widget returns `false`, system-layer binding takes over.

## 3.4 Body Layer — `ExpandableRowHost`

**Location:** `src/widgets/tabular_view/expandable_row_host.rs` (new file).

Composable component embedded in both `ListViewBody` and `TreeViewBody`.

```rust
pub struct ExpandableRowHost {
    factory: Option<Box<dyn Fn(RowId) -> Box<dyn Widget>>>,
    mounted: HashMap<RowId, NodeId>,
}
```

### Lifecycle

- On `Expand`: invoke factory, add child to tree under the body's NodeId, record NodeId in `mounted`.
- On `Collapse`: remove node from tree, drop from `mounted`.
- Factory is called exactly once per `Expand` per row. The produced widget lives until `Collapse`.

### Layout

During body `layout`, for each expanded row in the realized range: lay out its child with constraints derived from the `ExpandedHeightPolicy`. Report resolved height through `RowHeights::set_height`.

### Cap Policy ScrollArea Wrapping

When `ExpandedHeightPolicy::Cap(cap)` is in effect and the child's natural height exceeds `cap`, the host wraps the factory's widget in a `ScrollArea`. When child height ≤ `cap`, no wrapping occurs.

### Hit-Testing and Event Routing

Standard tree traversal routes input to the mounted detail child using its
own geometry. The row owner handles row selection. An opted-in detail
TextBlock handles its own routed character selection and copy under HAL315.

## 3.5 Widget Layer

`ListViewBody` and `TreeViewBody` each embed `ExpandableRowHost` and:

- Forward `ExpansionIntent` from substrate intent processing into host lifecycle calls.
- In `layout`, after row-projection resolution, iterate the host's active rows and lay out each child at its row's post-collapsed-strip position.
- In `paint`, paint the expansion visual frame before children paint.

`ListView` and `TreeView` expose:

```rust
pub fn set_expansion_policy(&mut self, p: ExpansionPolicy);
pub fn set_expanded_height_policy(&mut self, p: ExpandedHeightPolicy);
pub fn set_row_widget_factory(&mut self, f: impl Fn(RowId) -> Box<dyn Widget> + 'static);
```

Consumer usage:

```rust
list_view
    .set_expansion_policy(ExpansionPolicy::SingleExpand)
    .set_expanded_height_policy(ExpandedHeightPolicy::Cap(160.0))
    .set_row_widget_factory(|row_id| {
        Box::new(Label::new(get_detail_text(row_id)))
    });
```

---

# 4. Visual Specification

- **Collapsed row:** unchanged from current painting.
- **Expanded row, collapsed strip (top `COLLAPSED_HEIGHT` px):** identical to a collapsed row. Selection overlay paints here only.
- **Expanded row, detail region (remaining height):**
  - Background: `theme.colors.surface.subtle`.
  - Top divider (1px): `theme.colors.separator.default`.
  - Left indent for child layout: 24px.
  - The row selection overlay ends at the collapsed strip. The detail
    child may paint its own text selection.
- **Child widget paints over the detail background.** Consumer styles their own content.

---

# 5. Focus Scope Integration

`ToggleWithCapture` (`i` binding) requires framework focus scopes to route keyboard focus into the expanded child subtree. `event_router.rs:2892` stubs this:

```rust
// defines_focus_scope → Phase 3 (push scope, not yet implemented)
```

### Ship Phases

**Phase A (this SPEC):** `Expand`, `Collapse`, `Toggle` are fully functional via `l`, `h`, `Enter`, `Space`, double-click. `ToggleWithCapture` behaves as `Toggle` — expands the row but does not push a focus scope. A `tracing::warn!` fires on every `ToggleWithCapture` invocation documenting the stub.

**Phase B (SPEC-HAL-176 Phase 3):** When focus scopes land, `ToggleWithCapture` pushes a scope into the expanded child's subtree. Escape pops the scope. No SPEC-HAL-313 code changes required beyond removing the stub guard.

---

# 6. Filter/Sort Interaction

When a sort or filter operation causes a currently-expanded row to leave the visible set:

- The row's mounted widget is **unmounted** (removed from tree).
- The row's RowId is **removed** from `RowExpansion.expanded`.
- When the row re-enters the visible set, it is treated as collapsed. The consumer may re-expand programmatically.

No invisible widget subtrees persist in the tree.

---

# 7. Invariants

- **INV-313-ORTHOGONAL-1**: Selection state and expansion state are independent. No code reads one to decide the other.
- **INV-313-SUBSTRATE-OWNS-1**: Expansion state is owned exclusively by the substrate's `RowExpansion`. No widget, consumer, or body holds its own copy.
- **INV-313-FACTORY-PURE-1**: The widget factory is called exactly once per `Expand` per row. Not on re-layout, re-paint, scroll, or any other frame event. The produced widget lives until `Collapse`.
- **INV-313-FACTORY-FN-1**: The factory is `Fn`, not `FnMut`. Per-row state belongs in the widget the factory produces, not in the factory closure.
- **INV-313-VIRT-BOUND-1**: Memory footprint of expanded widgets is O(|expanded set|).
- **INV-313-HEIGHT-EXACT-1**: `RowHeights::set_height` is called with an exact resolved value. If the child's natural height is not yet known (first frame), the height is `COLLAPSED_HEIGHT + expanded_default` and is corrected on the next layout.
- **INV-313-PHASE-SEP-1**: Expansion mutations happen only during intent processing. Layout and paint phases never mutate `RowExpansion` or the host's mounted map.
- **INV-313-FALLTHROUGH-1**: Expansion-irrelevant key presses return `false` from `handle_event`, permitting Action System fallback.
- **INV-313-NO-CONSUMER-1**: No file under `src/model/tabular/`, `src/widgets/tabular_view/`, `src/widgets/list_view/`, `src/engines/hierarchical/`, `src/widgets/tree_view/` contains the tokens `event_log`, `EventLog`, `severity`, or `detail_text`.
- **INV-313-CAP-WRAP-1**: ScrollArea wrapping for Cap policy occurs only when child natural height exceeds the cap value. No wrapping when child fits within cap.
- **INV-313-FILTER-UNMOUNT-1**: When an expanded row leaves the visible/realized set due to filter or sort, its mounted widget is unmounted, its tree node removed, and its RowId removed from the expanded set.

---

# 8. Forbidden Behaviors

- **FORBID-313-DIRECT-KEYCODE-1**: `handle_keyboard` / `handle_event` in ListView, TreeView, or their bodies MUST NOT match on `KeyCode::Enter`, `KeyCode::Space`, `KeyCode::Character('h'|'l'|'i'|'j'|'k')` directly. All key handling goes through `IntentPolicy::interpret`.
- **FORBID-313-WIDGET-FACTORY-IN-MODEL-1**: `RowExpansion` MUST NOT hold the widget factory, NodeIds, or any widget reference.
- **FORBID-313-SELECTION-OVERLAY-BLEED-1**: The row selection highlight MUST clip to the collapsed-strip rect. The detail child owns its own text highlights.
- **FORBID-313-CONSUMER-STATE-1**: EventLog post-313 MUST NOT contain `HashSet<RowId>`, `toggle_expansion`, `expanded_height`, or any direct expansion-state field. All expansion concerns are delegated to the substrate via `set_expansion_policy` + `set_row_widget_factory`.

---

# 9. Migration from SPEC-HAL-312A Phase 2

| Phase 2 artifact | Disposition |
|---|---|
| `EventLogDataSource.expanded: HashSet<RowId>` | Deleted. State moves to substrate `RowExpansion`. |
| `EventLogDataSource::toggle_expansion` | Deleted. Substrate owns toggle via `ExpansionIntent`. |
| `ExpansionPolicy` on `EventLogDataSource` | Moved to `src/model/tabular/row_expansion.rs`. |
| `EventLogCellPainter` detail rendering | Deleted. Detail is a widget subtree, not a cell paint. |
| `ListView::set_activation_callback` | Kept for non-expansion activation consumers. |
| Enter-key interception in `ListView::handle_keyboard` | Deleted. Replaced by IntentPolicy routing. |
| `EventContext::invalidate_subtree` | Kept. Permanent infrastructure. |
| `RowHeightsChanged` event + adapter plumbing | Kept. Substrate uses this for height change notifications. |
