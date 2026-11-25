# SPEC-HAL-260A: **PropertyEditor**

### *Composite widget for hierarchical property inspection and editing, governed by Data-Layer Sovereignty*

**Author:** JC
**Status:** Binding
**Supersedes:** SPEC-HAL-260 (the prior PropertyEditor design — "PropertyEditor and Grid Specialization Model" — is retired as a structural specification; that document remains as a historical reference only).
**Depends on:**

- SPEC-HAL-001 (Core Architecture)
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-002A (Constitution Amendment Guidelines)
- SPEC-HAL-259 (HierarchicalTabularSubstrate Architecture)
- SPEC-HAL-328 (Keyboard Authority, Focus, and Capture)
- SPEC-HAL-020A/C (Action System core model; Binding Registry)
- SPEC-HAL-329 (KeybindEditor — Single-Stroke Keybinding Capture Widget) — forthcoming; consumed by PE's `PropertyValue::KeyBinding` editor binding.

---

# 1. Purpose and Scope

## 1.1 What PropertyEditor is

PropertyEditor is Halcyon's composite widget for inspecting and editing a hierarchical set of named property values. It renders a multi-column tabular view over a tree of `PropertyNode` rows, supports per-cell editing through framework-standard inline editor widgets, and exposes selection, focus, expand/collapse, and cell-mode navigation semantics consistent with the rest of Halcyon's data-list widget family (ListView, TreeView).

PropertyEditor is built on the HierarchicalTabularSubstrate (HTS, SPEC-HAL-259) for row-axis virtualization, column geometry, and hierarchical topology. It composes the framework's existing inline editor widgets — `TextField`, `SpinBox`, `Checkbox`, `ComboBox`, `ColorPickerField`, `KeybindEditor` (SPEC-HAL-329) — without reinventing any of them.

## 1.2 What this SPEC governs

This SPEC defines:

- The architectural principle (Data-Layer Sovereignty) under which PropertyEditor operates.
- The widget's structural composition and module organization.
- The data model carried into and out of PropertyEditor.
- The state model owned by `PropertyEditorCore`.
- Cell-mode navigation: the two-axis cursor, its addressing model, its preservation rules across row-kind transitions.
- The interaction policy surface.
- The intent dispatch pipeline and the permission gate seam.
- The editor lifecycle (mount-on-demand for value editors; always-mounted Checkbox for Bool).
- The layout and paint phase order.
- The complete set of named invariants and forbidden behaviors that bind the implementation.
- The mapping to Halcyon's Constitution (SPEC-HAL-002 §3.7, §3.11, §4.11) and Core Architecture (SPEC-HAL-001 §2.5).

## 1.3 Supersession

The prior SPEC-HAL-260 ("PropertyEditor and Grid Specialization Model") is retired as the governing structural specification. Its content remains in `docs/specs/` for historical reference; it must not be cited as authority in new work. All implementations and task artifacts authored after this SPEC lands cite SPEC-HAL-260A.

A "Superseded by SPEC-HAL-260A" header on the prior SPEC document is added as a separate documentation task.

## 1.4 Non-goals

Section §16 enumerates non-goals in detail. Summary: filtering, sorting, search, dynamic grouping, pagination, multi-select cell-mode (spreadsheet-style range selection), framework-level multi-clip child support, the full access-policy port, and any new `PropertyValue` variants are explicitly out of scope.

---

# 2. Dependencies

## 2.1 Constitutional

- **SPEC-HAL-001 §2.5** — Sovereignty Boundary. OS focus signals are advisory; semantic authority is engine-owned. See §17 for citation map.
- **SPEC-HAL-002 §3.7** — No Direct UI Mutation From Callbacks. Editor `on_exit` callbacks mutate PE-owned state only; tree-structural mutations (child node removal on edit-end) are deferred to Body's next layout pass. See §9 / §17.
- **SPEC-HAL-002 §3.11** — No Interpretive Intent Resolution. Editor commits are verbatim; PE does not normalize, auto-match, or reconcile typed input against existing options at commit time. See §9 / §17.
- **SPEC-HAL-002 §4.11** — Explicit Commit Authority. Every commit originates from an explicit user command (pointer activation, keyboard activation, or programmatic via Context). See §9 / §17.

## 2.2 Subsystem

- **SPEC-HAL-259** — HierarchicalTabularSubstrate. PE consumes HTS for row-axis virtualization, column geometry, expansion state, and visible-row enumeration. PE does not extend HTS; PE does not touch the SEALED BASE (TabularSubstrate, SPECs 164–178).
- **SPEC-HAL-328** — Keyboard Authority, Focus, and Capture. PE is a composite focusable widget; its inline editors are Category C/D/E/K (see §11.3–§11.5 and §11.K of SPEC-HAL-328 / SPEC-HAL-329 amendment).
- **SPEC-HAL-020A** — Action System Core Model. PE does not register actions; PE's keyboard handling routes through framework engine actions (ENTER_WIDGET, RELEASE_CAPTURE, FOCUS_*) per SPEC-HAL-328.
- **SPEC-HAL-020C** — Binding Registry. PE does not interact with the BindingRegistry directly. The `KeybindEditor` widget (SPEC-HAL-329) likewise produces values only; registration is the caller's responsibility — for PE, that means the consumer's `commit_cell` handler decides what (if anything) to do with a committed `PropertyValue::KeyBinding`.
- **SPEC-HAL-329** — KeybindEditor. PE's editor registry returns a `KeybindEditor` instance for `PropertyValue::KeyBinding` cells; the editor's `on_change` callback is wired into PE's standard commit funnel.

## 2.3 Reused infrastructure (consumed, not modified)

| Subsystem | Source |
|---|---|
| Hierarchical topology / virtualization | `src/engines/hierarchical/` (HTS) |
| Row-axis selection reducer | `src/core/ordered_selection/` |
| Horizontal scroll linking | `src/core/scroll_container::HorizontalScrollLink` |
| Tree paint primitives (disclosure, guide lines) | `src/widgets/tabular_view/tree_paint.rs` |
| Direction enum | `src/core/event_router::Direction` |
| WidgetKey encoding | `src/core/widget_tree::WidgetKey` |
| Inline editor widgets | `text_field`, `spin_box`, `checkbox`, `combo_box`, `color_picker_field`, `keybind_editor` (SPEC-HAL-329) |
| ScrollArea (composite outer wrapping Body) | `src/widgets/scroll_area.rs` |

PE rewrites none of the above. All infrastructure is consumed via existing public APIs.

---

# 3. Architectural Principle: Data-Layer Sovereignty

PropertyEditor operates under a strict separation between two state domains. The boundary is architectural and binding on every present and future feature.

## 3.1 Data domain vs view domain

**Data domain** — owned by the consumer's implementation of the `PropertyDataSource` trait. Includes:

- The set of property nodes (existence, count).
- Parent / child relationships (tree topology).
- Row order (default order, sort).
- The visible subset (filter result).
- The values held in cells.
- Validation rules and outcomes.
- Permission rules and outcomes.
- Per-cell formatting beyond `PropertyValue::display_text()`.

**View domain** — owned by `PropertyEditorCore`. Includes:

- Expansion state (delegated to HTS).
- Row-axis focus and selection (delegated to the `OrderedSelectionState` reducer).
- Cell-mode cursor (`focus: PeFocus`, §10.1).
- Column widths (delegated to HTS).
- Column display order.
- Active edit session.
- Hover state, scroll position.
- Cached display copies of permission and validation outcomes.

## 3.2 Intent emission vs decision

PropertyEditor **emits intents** about data-domain concerns. The data layer **decides**. PropertyEditor never applies data-domain operations.

PropertyEditor **mutates view-domain state directly**, subject to the invariants and forbidden behaviors in §9.

When PE needs the data layer to do something — commit a value, accept an edit, change permission state — the request travels through an intent. The data layer's response is observed by PE only by reading back: tree topology via `PropertyDataSource::nodes()`, cell values via `CellSource::cell(row_id, column_id)`, both after a `revision()` bump, or through other query methods on `PropertyDataSource`.

## 3.3 Read-only awareness

Data-Layer Sovereignty does not require obliviousness. PropertyEditor MAY read data-layer state to display it. PropertyEditor MUST NOT mutate that state.

Concrete examples (none implemented in this SPEC's scope; all consequent on future feature work):

- If data is sorted by column X descending, PE renders a sort indicator on column X's header. Sort state is read from `PropertyDataSource::sort_state()` (to be added when sort lands).
- If a filter is active, PE may render an "N of M shown" status. Filter state is read from `PropertyDataSource::filter_state()` (to be added when filter lands).
- If grouping is applied, PE renders group separators / counts as the consumer reports them.

Pattern: the widget reads via a data-layer query; the widget displays based on the result; the widget does not mutate the underlying state. Each new query method lands on `PropertyDataSource` as the corresponding feature lands.

## 3.4 INV-DLS-1

**INV-DLS-1 (Data-Layer Sovereignty):** PropertyEditor never mutates the visible set of rows, their order, their values, or their validation/permission outcomes. PropertyEditor emits intents for data-domain operations and observes their effect by re-reading the data layer after a revision bump.

Violation indicators: PE state that mirrors filter query strings, sort columns, validation results held independently of the data layer, value caches that mutate without a `commit_cell` round-trip.

## 3.5 FORBID-DLS-1

**FORBID-DLS-1:** No PropertyEditor field or method may modify, transform, filter, sort, group, or otherwise project the data-domain state away from what `PropertyDataSource::nodes()` returns (for row topology) or what `CellSource::cell()` returns (for cell values). The displayed set of rows is exactly what the data source returns; the order is exactly the order it returns them in.

The validation-error display cache (§5 / §8) is a single-string read-back of the data layer's last validation outcome, not a parallel validation engine. It is reset by Body when the editing cell is replaced or re-entered.

---

# 4. PropertyEditor Structural Shape

## 4.1 Composition

```
PropertyEditor (composite outer widget — focusable)
  ├─ ScrollArea (scroll state, scrollbars, clipping, smooth scroll)
  │   └─ PropertyEditorBody (content widget — owns layout, paint, body pointer events)
  │         └─ HierarchicalTabularSubstrate (HTS)
  │             [accessed via shared Rc<RefCell<PropertyEditorCore>>]
  └─ PropertyEditorFooter (optional second child — pure visual)
```

Mirrors TreeView's composition exactly. No filter child widget is part of the structural diagram (§16 non-goals; §3 sovereignty).

## 4.2 Module organization

PropertyEditor lives at `src/widgets/property_editor/`. Files:

| File | Role |
|---|---|
| `mod.rs` | Module banner with SPEC citations; public re-exports. |
| `types.rs` | `PropertyValue`, `PropertyNode`, `PropertyNodeKind`, `PropertyDataSource`, `ColumnKind`, `EditExitReason`, `AdvanceDirection`, `PermissionQuery`, `PermissionResult`, `PermissionAction`. |
| `property_editor.rs` | `PropertyEditor` composite outer widget. Keyboard handling. Column resize. |
| `body.rs` | `PropertyEditorBody` content widget. Layout, paint, pointer events, child widget reconciliation. |
| `core.rs` | `PropertyEditorCore` struct, `EditSession`, `PeFocus` enum (§8.1, §10.1), helper functions (`request_scroll_to_focus`, `ensure_focus_visible`, `dispatch_intent`, `set_focus_row`). |
| `policy.rs` | `PropertyEditorPolicy` and sub-enums. |
| `editor_registry.rs` | `create_edit_widget` factory; per-variant editor mounting. |
| `footer.rs` | `PropertyEditorFooter`. Optional, parallels `tree_view/footer.rs`. |

The migration path (§15) builds the new module beside the old (`property_editor_v2/`) and swaps at the end.

---

# 5. Data Model

## 5.1 The trait hierarchy and addressing model

PropertyEditor's data model is layered. The `CellSource` trait (in `src/engines/cell_source/`) is a universal cell-access primitive shared by every cell-addressed widget. PropertyEditor consumes it today; future tabular widgets — flat data grids, spreadsheets — consume it when they land. `PropertyDataSource` extends `CellSource` with PE-specific concerns: tree topology (`nodes()`) and the permission gate (§13).

The hierarchy separates STRUCTURE from CONTENT — plus a third universal axis, ROW ARCHETYPE:

- **STRUCTURE** is widget-specific. PE uses tree topology (`PropertyNode` with parent linkage). A flat grid would use ordered row enumeration. A spreadsheet would use sparse infinite addressing. Each widget's structural trait sits on top of `CellSource`.
- **CONTENT** is universal. Every cell-addressed widget addresses cells by `(row_id, column_id: ColumnId)` and reads / writes `Cell { value, editable }`. That surface lives in `CellSource`.
- **ROW ARCHETYPE** is universal. `CellSource::row_archetype(row_id) -> RowArchetype` reports each row's archetype — `RowArchetype::Data` for cell-bearing rows, `RowArchetype::Band` for structural grouping rows that host no cells. Bands are hit-tested, focused, and selected as whole-row units; per-column cell-mode operations do not apply on them. Default impl returns `Data` so existing `CellSource` impls stay correct without code change. Widgets that introduce band-archetype rows override the method; their `cell()` impl returns `None` at every column for Band rows. Per-widget richer archetype enums (PE's `PropertyNodeKind { Data, Section, SubSection }`, future widgets' own) project to this core binary at the trait boundary — the visual distinction between Section and SubSection in PE (see §5.5.1) is a PE-paint concern; the core archetype concept binds every cell-addressed widget that has band rows to a uniform structural primitive.

The addressing model at the data-source boundary is `ColumnId`. This is the stable column identity used throughout PE internally — see §6.1 editor pool keying, §6.3 mount factory, §8.1 PropertyEditorCore column fields, §10.1 cursor addressing. The data-source trait boundary matches.

The permission gate (§13) is the one exception. `PermissionQuery.column_index: Option<usize>` (§5.9) carries an integer column index, not a `ColumnId`. The gate is the consumer-facing surface for the deferred access policy port (§16); the port was designed against integer indices and the gate retains that shape to receive it. PE Core bridges `ColumnId → column_index` via `column_index_of` at gate-query construction time (§13.2).

## 5.2 PropertyValue

`PropertyValue` is the typed value carried by a `Cell`. It is defined in `src/engines/cell_source/mod.rs` because it is part of the universal content surface, not PE-specific.

```rust
pub enum PropertyValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Choice { selected: usize, options: Vec<String> },
    Color(Color),
    KeyBinding(Option<Shortcut>), // see §6.4 — Shortcut from SPEC-HAL-020A
    ReadOnly(Box<PropertyValue>),
}

impl PropertyValue {
    pub fn display_text(&self) -> String;
    pub fn is_editable(&self) -> bool;
}
```

`PropertyValue::KeyBinding` carries an `Option<Shortcut>` where `Shortcut` is the canonical type from the Action System (SPEC-HAL-020A, `src/runtime/actions.rs`), not a free-form string. The `Option` enables the steal-binding UX (JC directive 2026-05-21): when a shortcut is reassigned from one cell to another, the source cell transitions to `KeyBinding(None)` so the display honestly reflects "no binding." PE displays bindings using `Shortcut::to_display_string()` for `Some(_)` and an empty string for `None`.

`is_editable()` returns `false` only for the `ReadOnly` variant; all other variants are variant-level editable. Per-cell editability is the `Cell.editable` flag (§5.3) layered on top of this; consumer-side gating layers again via `permission_check` (§13).

**Band rows are not a `PropertyValue` variant.** Section and SubSection rows carry no values at any column; they are structural row archetypes per `PropertyNode.kind` (§5.5 — `PropertyNodeKind::Section`, `PropertyNodeKind::SubSection`). When a cell-addressed widget queries `CellSource::cell(row_id, column_id)` for a column on a band row, the source returns `None`. PE Body paints band rows with distinct backgrounds per archetype (§14.3 pass 2 — `header_background` for Section, `subheader_background` for SubSection).

## 5.3 Cell

```rust
pub struct Cell {
    pub value: PropertyValue,
    pub editable: bool,
}
```

`Cell` is the universal unit returned by `CellSource::cell(row_id, column_id) -> Option<Cell>`. `None` from `cell()` signals "no cell exists at this address" — used for section rows in PE, and for sparse-storage cells in future widgets that have not been written.

`editable` is per-cell. The consumer computes it — typically the column's default editability AND'd with row-level or cell-level overrides (calculated cells non-editable, locked rows non-editable). Per-cell `editable` layers on top of `PropertyValue::is_editable()` (variant-level; only `ReadOnly` is false there) and is gated again by `PropertyDataSource::permission_check` (consumer-side policy, §13). A cell is effectively editable only when all three permit. PE Body's editor mounting (§6) respects the composite predicate.

Per-cell `editable` is the ONLY override `Cell` carries. Permission outcomes, validation results, formatting hints, and column-kind echoes are held off `Cell` — they are either queries with their own lifecycle and caching strategy, or column-level schema set once at registration. `Cell` is the read-hot path: every paint pass calls `cell()` per visible row × per visible column. `Cell` stays cheap.

## 5.4 CellSource trait

```rust
pub trait CellSource {
    /// Read the cell at (row_id, column_id). Returns `None` when no cell
    /// exists at this address.
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<Cell>;

    /// Commit a new value to the cell at (row_id, column_id). Returns
    /// `true` to accept, `false` to reject. On rejection, the consuming
    /// widget displays the `validate()` error (if any) inline against
    /// the editing cell.
    fn commit_cell(
        &mut self,
        row_id: u64,
        column_id: ColumnId,
        new_value: PropertyValue,
    ) -> bool;

    /// Validate a candidate value before commit. Returns `Ok(())` if
    /// valid, `Err(msg)` if invalid. Default: always valid.
    fn validate(
        &self,
        _row_id: u64,
        _column_id: ColumnId,
        _candidate: &PropertyValue,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Monotonic revision counter. Consuming widgets re-read cells when
    /// revision changes.
    fn revision(&self) -> u64;
}
```

`CellSource` is the universal cell-access seam. Any widget that presents row-keyed cellular data in `(row_id, column_id)` addressing consumes a `CellSource`.

`commit_cell` returns `bool`: `true` accepts the new value; `false` rejects it. On rejection, PE displays the `validate()` error string (if any) inline against the editing cell — see §8.1's `validation_error` cache, which is the display-side read-back of the data layer's last validation outcome (§3.5 FORBID-DLS-1 confirms this is a one-string display cache, not a parallel validation engine).

`revision()` is the monotonic counter consuming widgets watch to know when the data has changed. PE Body re-reads `nodes()` and re-resolves visible cells via `cell()` when the cached `data_revision` falls behind `revision()`.

## 5.5 PropertyNode

```rust
pub struct PropertyNode {
    pub node_id: u64,
    pub label: String,
    pub parent_id: Option<u64>,
    pub kind: PropertyNodeKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PropertyNodeKind {
    /// A property row. Cells exist at every Data column; `cell()`
    /// returns `Some` per column when the consumer populates. Eligible
    /// for cell focus, selection, and edit. Renders per-column content
    /// in PE Body's data-cell paint pass.
    Data,
    /// A top-level grouping band. No cells: `cell()` returns `None` at
    /// every column. Not selectable. Not edit-eligible. Renders as a
    /// prominent full-width header strip (`list_colors.header_background`).
    /// May host a disclosure toggle on the tree column when it has
    /// children. Structurally: a Section's `parent_id` resolves through
    /// the topology without crossing another Section or a SubSection
    /// (i.e. Sections are top-level groupings within their subtree —
    /// see INV-ARCHETYPE-HIERARCHY, §5.5.1).
    Section,
    /// A nested grouping band within a Section. No cells. Not selectable.
    /// Not edit-eligible. Renders as a subdued full-width strip
    /// (`list_colors.subheader_background`) — visually distinct from a
    /// Section. May host a disclosure toggle when it has children.
    /// Structurally: a SubSection's `parent_id` must resolve to a
    /// Section ancestor. SubSection at root, SubSection nested under a
    /// Data row, and SubSection nested under another SubSection are
    /// all forbidden — see INV-ARCHETYPE-HIERARCHY, §5.5.1.
    SubSection,
}
```

The PE-specific row data type — tree-structural metadata only. One node per row. Holds identity, parent linkage, the display label, and the row archetype.

Cell values are not carried by `PropertyNode`; they come from the `CellSource` via `cell(row_id, column_id)`.

`node_id` is stable, maps 1:1 to HTS `NodeMeta.node_id`. `parent_id` is `None` for root-level nodes. `kind` discriminates the three row archetypes — Data, Section, SubSection — and governs paint, hit-test, focus, navigation, selection, and edit behavior per §5.5.1.

### 5.5.1 Row archetypes (Data, Section, SubSection)

The three `PropertyNodeKind` variants are first-class structural archetypes — not "a Data row plus a flag." Each archetype has distinct semantics across every PE consumer:

| Concern | Data | Section | SubSection |
|---|---|---|---|
| `CellSource::cell()` returns | `Some` per Data column when populated | `None` at every column | `None` at every column |
| Cell focus eligible | yes | no | no |
| Band focus eligible | no | yes | yes |
| Row selectable | yes | no | no |
| Edit-eligible | yes (per `Cell.editable`) | no | no |
| Disclosure toggle | yes if `has_children` | yes if `has_children` | yes if `has_children` |
| Paint primitive | per-column cells | single full-width band | single full-width band |
| Band theme token | n/a | `list_colors.header_background` | `list_colors.subheader_background` |
| Allowed `parent_id` topology | any | root or any Data/Section/SubSection (with no Section/SubSection between this node and root via the parent chain) | must resolve to a Section ancestor; no other Section/SubSection between this node and that ancestor |
| Hit-test result on cell-column hit | `Cell { row_id, col_id }` | `Band { row_id }` (cell coordinates suppressed) | `Band { row_id }` (cell coordinates suppressed) |

**INV-ARCHETYPE-HIERARCHY.** The consumer's `nodes()` topology must satisfy: SubSection's `parent_id` resolves to a Section ancestor; no SubSection at root; no SubSection under a Data row; no SubSection nested under another SubSection. PE debug-asserts these invariants during `build_node_metas`. Release builds render whatever topology the consumer supplies — PE does not silently restructure data per §3 Data-Layer Sovereignty.

**Section vs SubSection in code paths:** the two band archetypes are distinguished only at paint (theme token) and at the structural invariant check above. All other PE consumers (hit-test, focus, navigation, selection, edit, intent funnel) treat Section and SubSection identically — both produce `Band` focus, both suppress cell coordinates, both are non-selectable and non-editable. The distinction lives in the data model and the paint pass; it does not propagate into the navigation or edit state machines.

## 5.6 PropertyDataSource trait

```rust
pub trait PropertyDataSource: CellSource {
    /// Return the full property tree (rows as PropertyNode topology).
    /// Cell values come separately via `CellSource::cell()`.
    fn nodes(&self) -> Vec<PropertyNode>;

    /// Per-cell permission gate. Default: always permit, preserving
    /// today's behavior for callers that have not opted into the
    /// access policy port (§16).
    fn permission_check(&self, _query: PermissionQuery) -> PermissionResult {
        PermissionResult::Permit
    }

    // Future query hooks (each lands with its corresponding feature
    // and the data-domain principle of §3 Data-Layer Sovereignty):
    // fn sort_state(&self) -> Option<SortState>;
    // fn filter_state(&self) -> Option<FilterState>;
    // fn group_state(&self) -> Option<GroupState>;
}
```

`PropertyDataSource` extends `CellSource` with PE-specific concerns:

- `nodes()` returns the property tree as `PropertyNode` topology. Cell values come separately via the inherited `CellSource::cell()` / `commit_cell()` / `validate()` / `revision()`.
- `permission_check` is the §13 gate. Default: always permit. The gate is a foundational seam reserved for the access policy port (§16); the port supplies caching, batched queries, and richer action enum variants without breaking this SPEC.

The future query hooks (sort / filter / group) land on `PropertyDataSource` when the corresponding features land, per §3 Data-Layer Sovereignty. They are structural queries about which rows to return and in what order — distinct from the universal cell-access surface that stays in `CellSource`.

## 5.7 ColumnKind

```rust
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColumnKind {
    /// Holds property values; subject to per-cell editability.
    Data,
    /// Structural / informational (tree label, future Indicator or Action
    /// columns). Not editable. May render disclosure toggles. On Data
    /// rows, NonData columns are valid cursor targets per
    /// `navigable_columns_for_row` (§10.2). On Section / SubSection
    /// rows, no column is a cursor target — the cursor on band-archetype
    /// rows is `PeFocus::Band { row_id }`, a row-level value with no
    /// column component (§10.1).
    NonData,
}
```

`#[non_exhaustive]` is mandatory — future widening to `Name | Indicator | Action | …` must not break trait users.

## 5.8 EditExitReason

```rust
#[derive(Copy, Clone, Debug)]
pub enum EditExitReason {
    /// Editor signaled value should be committed.
    Commit,
    /// Editor signaled value should be discarded (Escape path).
    Cancel,
    /// Editor signaled commit and request cell-mode advance.
    CommitAndAdvance { direction: AdvanceDirection },
    /// Editor was forcibly unmounted (PE-side, e.g. blur policy = Cancel).
    ForcedCancel,
}

#[derive(Copy, Clone, Debug)]
pub enum AdvanceDirection { Next, Previous }
```

## 5.9 Permission types

```rust
pub struct PermissionQuery {
    pub row_id: u64,
    pub column_index: Option<usize>,
    pub action: PermissionAction,
}

#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub enum PermissionAction {
    Focus,
    Select,
    Edit,
    Activate,
    DragExtend,
    Tree,
}

#[derive(Copy, Clone, Debug)]
pub enum PermissionResult {
    Permit,
    Deny,
}
```

`column_index: Option<usize>` — the gate's consumer-facing addressing is integer-indexed per §5.1. PE Core bridges `ColumnId → column_index` via `column_index_of` at gate-query construction time (§13.2). `PermissionAction` is `#[non_exhaustive]` — the access policy port (deferred per §16) extends it without breaking callers.

---

# 6. Editor Mounting Strategy

## 6.1 Two pools plus the active edit session

PropertyEditorBody owns child editor widgets through three mechanisms, each reconciled in every layout pass:

| Mechanism | Storage | Lifecycle | Used for |
|---|---|---|---|
| Disclosure-toggle pool | `disclosure_toggles: HashMap<u64, NodeId>` keyed by `node_id` | Mounted when the row has children AND is visible; unmounted otherwise. | Tree expand / collapse toggles (identical to TreeView body). |
| Always-mounted-editor pool | `always_mounted_editors: HashMap<(u64, ColumnId), NodeId>` keyed by `(node_id, column_id)` | Mounted when the cell is `Bool` AND visible AND `permission_check(Edit)` permits; unmounted otherwise. | The Checkbox for `PropertyValue::Bool` cells. |
| Active edit session | `core.edit_session: Option<EditSession>` | Mounted on `RequestBeginEdit`; unmounted on commit / cancel / forced unmount. | All other editor variants (mount-on-demand). |

The active edit session is single-valued: at most one edit session is active at any moment (INV-PE-SINGLE-EDIT, §9).

## 6.2 Always-mounted Checkbox for Bool

Bool cells are toggled by direct activation, with no separate "begin edit" / "commit edit" cycle. The Checkbox is mounted as a child of the cell when the cell is visible and editable; its `on_change` closure calls `consumer.commit_cell` directly. This matches the DisclosureToggle pattern in TreeView body.

The always-mounted Checkbox does NOT route through the `edit_session` mechanism. It is keyed in a separate `HashMap` and reconciled independently.

## 6.3 Mount-on-demand factory

```rust
pub(crate) fn create_edit_widget(
    value: &PropertyValue,
    cell_key: (u64, ColumnId),
    on_exit: Box<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> Box<dyn Widget> {
    match value {
        PropertyValue::String(s)         => Box::new(make_text_field(s, cell_key, on_exit)),
        PropertyValue::Integer(n)        => Box::new(make_integer_field(*n, cell_key, on_exit)),
        PropertyValue::Float(f)          => Box::new(make_float_field(*f, cell_key, on_exit)),
        PropertyValue::Choice { selected, options } =>
            Box::new(make_combo_box(*selected, options.clone(), cell_key, on_exit)),
        PropertyValue::Color(c)          => Box::new(make_color_picker_field(*c, cell_key, on_exit)),
        PropertyValue::KeyBinding(s)     => Box::new(make_keybind_editor(*s, cell_key, on_exit)),
        PropertyValue::Bool(_)           => panic!("Bool is always-mounted; not produced by create_edit_widget"),
        PropertyValue::ReadOnly(_)       => panic!("ReadOnly is non-editable; not produced"),
    }
}
```

Each `make_*` constructor wires the editor's `on_change` / `on_commit` / `on_cancel` / `on_advance` callbacks into the `on_exit` closure with the appropriate `EditExitReason`. Per-editor mapping:

| Editor (SPEC) | Trigger | EditExitReason |
|---|---|---|
| TextField (HAL-013, 014) | Enter | `Commit` |
| TextField | Escape (framework path) | `Cancel` (via `on_capture_leave` handler) |
| TextField | Tab / Shift+Tab | `CommitAndAdvance { Next | Previous }` |
| ComboBox (HAL-201–206) | option selection | `Commit` |
| ColorPickerField (HAL-272) | color selection | `Commit` |
| KeybindEditor (HAL-329) | one-shot key capture | `Commit` |
| any editor | PE blur, policy = Commit | `Commit` (via outer on_blur) |
| any editor | PE blur, policy = Cancel | `Cancel` |
| any editor | forced unmount | `ForcedCancel` |

## 6.4 KeybindEditor integration (SPEC-HAL-329)

For `PropertyValue::KeyBinding(Option<Shortcut>)` cells, the editor is a `KeybindEditor` widget per SPEC-HAL-329. The widget captures a single keystroke via the one-shot capture model (SPEC-HAL-329 §3): first non-modifier Key Down in capture mode builds a `Shortcut`, fires the editor's `on_change`, and releases capture programmatically. Escape via framework path releases capture without committing.

PE does not register the captured `Shortcut` with the `BindingRegistry` itself. The captured value flows through `commit_cell(row_id, col_index, PropertyValue::KeyBinding(shortcut))`. The consumer (the application owning the keybind data) is responsible for any subsequent registration with `BindingRegistry` at its chosen layer and priority. PE remains stateless re: data-layer state (§3, §3.5).

The `KeybindEditor` widget is consumed by PE through `make_keybind_editor` in `editor_registry.rs`. The widget itself is not part of the PropertyEditor module — it lives at `src/widgets/keybind_editor.rs` per SPEC-HAL-329.

## 6.5 Editor exit and tree-structural deferral

The editor's `on_exit` closure is invoked by the editor when the user triggers a commit, cancel, or advance. The closure:

1. Mutates `core.edit_session` (clears it on commit/cancel; leaves it for advance with the new cell coordinates).
2. May mutate `core.validation_error` to surface a rejection message.
3. May call `consumer.commit_cell(...)` on the data source.

The closure does NOT directly remove the editor's child node from the widget tree. Tree-structural removal of the editor child is **deferred to Body's next layout pass**, which observes `edit_session == None` together with a lingering `editor_node_id` and removes the node at that point.

This deferral satisfies SPEC-HAL-002 §3.7 (No Direct UI Mutation From Callbacks); see §17.

---

# 7. Column Structure

## 7.1 Column model

PropertyEditor presents one tree column plus zero or more data columns. The tree column carries the hierarchical name display (disclosure toggle, indent, label). Data columns carry editable per-cell values.

The tree column is always a `NonData` column; it is the *first* `NonData` column. The current rewrite emits exactly one `NonData` column (the tree column) plus one or more `Data` columns. Future widening (Indicator, Action) is permitted by `#[non_exhaustive] ColumnKind`.

## 7.2 Column registration (builder API)

```rust
impl PropertyEditor {
    pub fn add_name_column(&mut self, title: impl Into<String>) -> ColumnId;
    pub fn add_value_column(&mut self, title: impl Into<String>) -> ColumnId;
}
```

`add_name_column` registers a `NonData` column and sets `tree_column_id`. Calling `add_name_column` more than once is a programming error (panic in debug).

`add_value_column` registers a `Data` column.

## 7.3 WidgetKey encoding for cell children

Cell editor child widgets are identified by composite keys:

```rust
fn cell_widget_key(node_id: u64, column_index: u32) -> WidgetKey {
    // Bits 63..16: node_id low bits. Bits 15..1: column_index. Bit 0: type tag = 1.
    let tag = 1u64;
    let col = (column_index as u64 & 0x7FFF) << 1;
    let node = (node_id & 0x0000_FFFF_FFFF_FFFF) << 16;
    WidgetKey::U64(node | col | tag)
}

fn disclosure_widget_key(node_id: u64) -> WidgetKey {
    WidgetKey::U64((node_id << 1) | 1)
}
```

The two pools' key layouts do not collide. Column index is capped at 32767 (well above realistic). Node IDs above 2^48 are not supported by this packing — documented limit; not a runtime concern given current `node_id` allocation.

---

# 8. PropertyEditor Composition (state ownership)

## 8.1 PropertyEditorCore

```rust
/// PE cursor. Owned by `PropertyEditorCore::focus`. Mutations are
/// funnelled through `Core::set_focus_row` (row-axis transitions) and
/// `apply_column_intent` (column-axis transitions) so the well-formedness
/// invariants (INV-CELL-FOCUS-WELLFORMED, INV-BAND-FOCUS-WELLFORMED — §9.1)
/// hold at the constructor boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PeFocus {
    /// No cursor. Initial state, or after scope-pop.
    None,
    /// Cell-mode focus on a Data row at a specific column.
    /// Construction precondition: row's `kind == Data` AND
    /// `col_id ∈ navigable_columns_for_row(row_id)`.
    Cell { row_id: u64, col_id: ColumnId },
    /// Band-mode focus on a Section or SubSection row.
    /// Construction precondition: row's `kind == Section | SubSection`.
    Band { row_id: u64 },
}

pub(crate) struct PropertyEditorCore {
    // --- Topology + geometry (HTS owns row + column axes) ---
    pub(crate) hts: HierarchicalTabularSubstrate,
    pub(crate) column_kinds: HashMap<ColumnId, ColumnKind>,
    pub(crate) name_column_id: Option<ColumnId>,
    pub(crate) tree_column_id: Option<ColumnId>,
    pub(crate) value_column_ids: Vec<ColumnId>,
    pub(crate) column_titles: HashMap<ColumnId, String>,

    // --- Property data (consumer-provided) ---
    pub(crate) data_source: Option<Rc<RefCell<dyn PropertyDataSource>>>,
    pub(crate) property_nodes: HashMap<u64, PropertyNode>,
    pub(crate) data_revision: u64,

    // --- Row-axis selection / focus (reducer-managed) ---
    pub(crate) selection: OrderedSelectionState<u64>,

    // --- Cell-mode focus (PE-local; §10.1) ---
    /// The PE cursor. Encodes (None | Cell on Data row | Band on
    /// Section/SubSection row). INV-CELL-FOCUS-WELLFORMED + INV-BAND-FOCUS-WELLFORMED
    /// (§9.1) hold by construction — mutations go through
    /// `Core::set_focus_row` and `apply_column_intent`, which read the
    /// target row's `PropertyNodeKind` and build the variant accordingly.
    pub(crate) focus: PeFocus,
    /// Column-preservation cache for the §10.3 transition rules. Carries
    /// the most-recently-focused Data column across band-archetype rows
    /// and across `None` interludes, so re-entering a Data row from a
    /// band restores the prior column when it is still navigable on the
    /// new row.
    pub(crate) last_data_column: Option<ColumnId>,

    // --- Edit state (single-edit invariant) ---
    pub(crate) edit_session: Option<EditSession>,

    // --- Drag selection (TreeView pattern) ---
    pub(crate) selection_drag_anchor: Option<u64>,
    pub(crate) selection_drag_active: bool,
    pub(crate) selection_drag_did_extend: bool,

    // --- Column resize (outer-managed; TreeView pattern) ---
    pub(crate) hovered_separator: Option<ColumnId>,
    pub(crate) active_resize_separator: Option<ColumnId>,

    // --- Hover (cell-level, paint affordance) ---
    pub(crate) hovered_cell: Option<(u64, ColumnId)>,

    // --- Body ↔ outer signaling ---
    pub(crate) body_widget_id: Option<TreeWidgetId>,
    pub(crate) requested_scroll_y: Option<f32>,
    pub(crate) focused: bool,
    pub(crate) scale_factor: f32,
    pub(crate) horizontal_scroll_link: Option<HorizontalScrollLink>,

    // --- Interaction policy ---
    pub(crate) policy: PropertyEditorPolicy,

    // --- Validation feedback (display cache only; §3.5 FORBID-DLS-1) ---
    pub(crate) validation_error: Option<String>,
}

pub(crate) struct EditSession {
    pub(crate) row_id: u64,
    pub(crate) column_id: ColumnId,
    pub(crate) child_widget_id: Option<TreeWidgetId>,
    pub(crate) editor_node_id: Option<NodeId>,
}
```

Reducer-managed concerns (row-axis selection — `focus_key` lives in `selection`) and HTS-managed concerns (expansion, geometry) are not duplicated. PE's cell-mode cursor (`focus: PeFocus`) is PE-local; the reducer owns row-axis focus and PE projects it into the variant via `set_focus_row`.

## 8.2 Ownership table

| Field | Writer(s) | Reader(s) |
|---|---|---|
| `hts` | Outer (mutations via API), Body (visible_rows, scroll sync) | Outer, Body, Footer |
| `column_kinds`, `name_column_id`, `tree_column_id`, `value_column_ids`, `column_titles` | Outer (builder API + `rebuild_hts_data`) | Outer, Body |
| `data_source` | Outer (`set_data_source`) | Outer, Body |
| `property_nodes`, `data_revision` | Outer (`rebuild_hts_data` after `revision()` change) | Outer, Body |
| `selection` | Outer + Body (via reducer, through `dispatch_intent`) | Outer, Body, Footer |
| `focus`, `last_data_column` | Outer (keyboard) + Body (pointer), through `dispatch_intent` → `Core::set_focus_row` + `apply_column_intent` | Outer, Body (paint) |
| `edit_session` | Body (mount on intent) + edit callback (clear) | Outer (guards), Body |
| `selection_drag_*` | Body | Body |
| `hovered_separator`, `active_resize_separator` | Outer | All (paint) |
| `hovered_cell` | Body | Body (paint) |
| `body_widget_id` | Body (first event) | Outer (invalidate) |
| `requested_scroll_y` | Outer (after keyboard nav) | Body (`take_requested_scroll_y`) |
| `focused` | Outer (`on_focus` / `on_blur`) | Body (paint color) |
| `scale_factor` | Outer (paint) | Outer (geometry resolve) |
| `horizontal_scroll_link` | Outer (init) | All (paint) |
| `policy` | Outer (builder API) | Outer, Body |
| `validation_error` | Outer (post-commit) + Body (clear on edit re-enter) | Body (paint inline error) |

Column-wide default editability and row- or cell-level overrides are resolved by the data source when it computes `Cell.editable` (§5.3). PropertyEditor does not maintain a separate column-level editability policy. Variant-level eligibility and the existing permission gate remain governed by §§5.2–5.3 and §13.

## 8.3 The shared-cell discipline

`PropertyEditorCore` is held behind `Rc<RefCell<PropertyEditorCore>>`. Outer, Body, and Footer hold clones of the `Rc`. All cross-widget communication is through Core. No widget holds a reference to another widget.

This satisfies SPEC-HAL-002 §3.1 (no direct references between UI elements) and Pillar C (ID-through-IDs, not references). The `Rc<RefCell<…>>` shares ownership of one cell, not references between widgets.

---

# 9. PropertyEditor Invariants

## 9.1 Core invariants

**INV-DLS-1 (Data-Layer Sovereignty).** PropertyEditor never mutates the visible set of rows, their order, their values, or their validation / permission outcomes. PE emits intents for data-domain operations and re-reads the data source — `nodes()` for topology, `cell()` for cell values — after revision bump. (§3.4)

**INV-PE-SINGLE-EDIT.** At most one edit session is active. `core.edit_session` is `Some` for exactly one cell at a time. Attempting to begin an edit while another is active is a programming error: PE must commit / cancel the active session first.

**INV-CELL-FOCUS-WELLFORMED.** When `core.focus == PeFocus::Cell { row_id, col_id }`, the row at `row_id` has `PropertyNodeKind::Data` AND `col_id ∈ navigable_columns_for_row(row_id)`. This holds at the constructor boundary — `Core::set_focus_row` produces `Cell` only when the row's kind is `Data`; `apply_column_intent` (which mutates `col_id`) rejects targets outside the row's navigable set. INV-CELL-FOCUS-WELLFORMED replaces the prior INV-CELL-1 and INV-CELL-2.

**INV-BAND-FOCUS-WELLFORMED.** When `core.focus == PeFocus::Band { row_id }`, the row at `row_id` has `PropertyNodeKind::Section` or `PropertyNodeKind::SubSection`. Holds at the constructor boundary — `Core::set_focus_row` produces `Band` only when the row's kind is `Section` or `SubSection`.

**INV-PE-INTENT-FUNNEL.** All mutations of `selection`, `focus`, `edit_session`, `hts` (tree state), and `validation_error` route through `PropertyEditorCore::dispatch_intent` (§12). No event handler may mutate these fields directly. The permission gate (§13) lives inside `dispatch_intent`.

**INV-PE-COMMIT-EXPLICIT.** Every commit originates from an explicit user command — pointer activation, keyboard activation, programmatic command routed through Context, or PE blur in the policy `blur_disposition = Commit` configuration where the blur itself was caused by user action. PE never infers a commit from OS-deactivation signals or other advisory inputs. (Reaffirms SPEC-HAL-002 §4.11; see §17.)

**INV-PE-VERBATIM-COMMIT.** When `consumer.commit_cell` is called, the value passed is exactly the value the editor surfaced to PE via `on_exit`. PE does not normalize, auto-match, lowercase, trim, substitute, or otherwise reinterpret the editor's output. Editor-internal disambiguation (e.g., ComboBox's selected option vs typed text) is the editor's responsibility, governed by SPEC-HAL-002 §3.11. (See §17.)

**INV-PE-SOVEREIGNTY.** PropertyEditor derives focus / capture / modality / routing dispositions from engine state, not OS state. OS focus signals are advisory only. The blur-disposition policy (§11) acts on engine-state blur, not OS window deactivation. (Reaffirms SPEC-HAL-001 §2.5; see §17.)

**INV-PE-TREE-DEFERRED.** Editor `on_exit` callbacks do not directly remove editor child widgets from the widget tree. Tree-structural changes for editor mount / unmount happen during Body's layout pass (§14). The callback mutates `core.edit_session`; the next layout pass observes the change and applies the structural mutation. (Reaffirms SPEC-HAL-002 §3.7; see §17.)

**INV-PE-CELL-FOCUS-ONLY.** PE Body paints exactly one focus indicator per cursor state:

- `PeFocus::Cell { row_id, col_id }`: a cell-sized rectangle at the resolved cell, on `LAYER_FOCUS`, with both X edges snapped to physical pixel boundaries. No row-spanning border. Tree-column cells receive the same indicator as Data-column cells.
- `PeFocus::Band { row_id }`: PE paints no extra focus rectangle. The band's distinct background (§14.3 band paint pass) carries the focused state visually. The disclosure toggle child widget, when focused via Tab into capture, paints its own framework focus ring per SPEC-HAL-328; that is the band-focus affordance.
- `PeFocus::None`: no focus indicator.

The indicator is emitted as step 4c of the unified per-cell loop (§14.3 pass 4), in the same iteration that emits hover overlay (4a) and cell content (4b).

**INV-CELL-PAINT-UNIFORM.** PE Body's paint pipeline contains exactly one per-cell iteration loop for `RowArchetype::Data` rows. That loop iterates every column slot uniformly and dispatches cell content by the column's registered cell kind inside the loop body. Hover, focus, and selection affordances apply across every cell of every kind through the same structural primitive. No paint pass at the outer structural level may branch on a specific `ColumnId` value. Cell-kind dispatch — `ColumnKind`, or the richer kind metadata that replaces it when introduced — is the only legal column-discrimination mechanism, applied inside the per-cell loop body. The row-archetype fork at the row level (Band vs Data) is the only structural fork in the paint pipeline.

## 9.2 Forbidden behaviors

**FORBID-DLS-1.** No PE field or method may modify, transform, filter, sort, group, or otherwise project the data-domain state away from what the data source returns — `nodes()` for topology, `cell()` for cell values. (§3.5)

**FORBID-PE-MULTI-EDIT.** Mounting a second editor while an `edit_session` is active is forbidden. Begin-edit operations that target a different cell first commit (or cancel, per policy) the active session.

**FORBID-PE-BYPASS-FUNNEL.** Direct mutation of `selection`, `focus`, `edit_session`, `hts` topology / expansion, or `validation_error` from event handlers or from outside `dispatch_intent` is forbidden. All mutations route through the funnel.

**FORBID-PE-PERMISSION-BYPASS.** Every state-changing event handler must construct a `PermissionQuery` and pass it through `dispatch_intent`. The `permission_check` default (`Permit`) is the no-op path; consumer-provided implementations gate access.

**FORBID-PE-AUTO-MATCH-AT-COMMIT.** PE does not match typed input to existing options at commit time. PE does not case-normalize commits. PE does not substitute "the closest option" for "what the user typed." (Reaffirms SPEC-HAL-002 §3.11.)

**FORBID-PE-COMMIT-FROM-OS-BLUR.** PE does not commit edits in response to OS window deactivation or other OS-originated focus signals. Blur disposition fires only when blur originated from user action within engine semantics. (Reaffirms SPEC-HAL-001 §2.5.)

**FORBID-PE-CALLBACK-TREE-MUTATION.** Editor `on_exit` callbacks must not call into the widget tree's structural mutation API (`tree.remove_node`, `tree.add_child`, etc.) on PE's editor children. Such mutations are performed during Body's layout pass.

**FORBID-PE-FILTER-IN-VIEW.** PropertyEditor does not implement a filter UI, a sort UI, a search UI, or a dynamic-grouping UI. These features, when introduced, route through `PropertyDataSource` query methods and intents emitted to the consumer. (§3, §16.)

**FORBID-PE-COLUMN-ID-CARVE-OUT.** No PE paint pass, hit-test function, or interaction handler may branch on a specific `ColumnId` value at the outer structural level. The tree-kind cell is identified by the column's registered `ColumnKind` (or richer kind metadata when introduced), never by literal `column_id == tree_column_id` comparison at structural seams. Kind dispatch is the only legal column-discrimination mechanism. Enforced by INV-CELL-PAINT-UNIFORM (§9.1).

---

# 10. Cell-Mode Navigation

## 10.1 The PeFocus cursor

The PE cursor is a single `PeFocus` value, owned by `PropertyEditorCore.focus` (§8.1). The variant encodes both the row-axis projection of `selection.focus_key` and the column-axis position in one closed type:

```rust
pub enum PeFocus {
    None,                                    // no cursor
    Cell { row_id: u64, col_id: ColumnId },  // cell-mode focus on a Data row
    Band { row_id: u64 },                    // band-mode focus on a Section / SubSection row
}
```

Per INV-CELL-FOCUS-WELLFORMED + INV-BAND-FOCUS-WELLFORMED (§9.1), the variant carries its own constructor preconditions: `Cell` is only constructible on Data rows with a navigable column; `Band` is only constructible on Section / SubSection rows. The constructor seam is `Core::set_focus_row` for row-axis transitions and `Core::apply_column_intent` for column-axis transitions. Direct construction of `core.focus = PeFocus::Cell { ... }` outside these seams is a funnel bypass and forbidden by INV-PE-INTENT-FUNNEL (§9.1).

The reducer-managed `selection.focus_key: Option<u64>` remains the canonical row-axis focus identity. After the reducer mutates it, PE projects the new `focus_key` into `core.focus` via `set_focus_row`:

```rust
impl PropertyEditorCore {
    /// Project `focus_key` onto `core.focus`. Reads the target row's
    /// archetype and produces the matching `PeFocus` variant. The single
    /// constructor seam for row-axis focus mutation. Idempotent and
    /// total — handles `None`, missing rows, archetype lookup, column
    /// resolution, and the §10.2 column-preservation rules in one place.
    fn set_focus_row(&mut self, new_focus_key: Option<u64>) {
        let prior = self.focus;
        self.focus = match new_focus_key {
            None => PeFocus::None,
            Some(row_id) => match self.property_node_kind(row_id) {
                None => PeFocus::None, // unknown row — treat as no focus
                Some(PropertyNodeKind::Section) | Some(PropertyNodeKind::SubSection) => {
                    // Cache out the prior Data column before transitioning
                    // into a band (§10.2's column-preservation rule).
                    if let PeFocus::Cell { col_id, .. } = prior {
                        if self.is_data_column(col_id) {
                            self.last_data_column = Some(col_id);
                        }
                    }
                    PeFocus::Band { row_id }
                }
                Some(PropertyNodeKind::Data) => {
                    let col_id = self.resolve_cell_focus_column(row_id, prior);
                    match col_id {
                        Some(c) => PeFocus::Cell { row_id, col_id: c },
                        None => PeFocus::None, // no navigable column — should not happen with non-empty column set
                    }
                }
            },
        };
    }

    /// Resolve which column to use when entering a Data row.
    /// Resolution order, first match wins:
    ///   1. Prior `Cell.col_id` if still navigable on the new row.
    ///   2. `last_data_column` if navigable on the new row.
    ///   3. First Data column (`value_column_ids.first()`).
    ///   4. Tree column (`tree_column_id`).
    fn resolve_cell_focus_column(&self, row_id: u64, prior: PeFocus) -> Option<ColumnId> {
        let navigable = self.navigable_columns_for_row(row_id);
        if let PeFocus::Cell { col_id, .. } = prior {
            if navigable.contains(&col_id) {
                return Some(col_id);
            }
        }
        if let Some(cc) = self.last_data_column {
            if navigable.contains(&cc) {
                return Some(cc);
            }
        }
        self.value_column_ids.first().copied()
            .or(self.tree_column_id)
    }
}
```

The 22-field `PropertyEditorCore` (§8.1) holds `focus: PeFocus` and `last_data_column: Option<ColumnId>`. The prior SPEC split cell-mode focus across two separate Core fields; the archetype model unifies the row-axis projection and the column-axis position into the single `PeFocus` variant, with `last_data_column` retained as the column-preservation cache.

## 10.2 Column navigation operations and the navigable set

`navigable_columns_for_row` returns the set of valid column targets for cell-mode focus on a row. Section and SubSection rows have an empty navigable set — they do not host cell focus:

```rust
fn navigable_columns_for_row(&self, row_id: u64) -> &[ColumnId] {
    match self.property_node_kind(row_id) {
        Some(PropertyNodeKind::Data) => &self.all_columns_in_order,
        Some(PropertyNodeKind::Section) | Some(PropertyNodeKind::SubSection) => &[],
        None => &[],
    }
}
```

Column-axis intents (`ColumnIntent::To { column_id }`) dispatched through `apply_column_intent` mutate `core.focus`'s column position. The operation is meaningful only when `core.focus == PeFocus::Cell { .. }`; on `Band` and `None`, it is a silent no-op:

```rust
fn apply_column_intent(&mut self, intent: ColumnIntent) {
    match intent {
        ColumnIntent::To { column_id } => {
            let PeFocus::Cell { row_id, .. } = self.focus else {
                return; // Band / None — no column to set.
            };
            if !self.navigable_columns_for_row(row_id).contains(&column_id) {
                return; // INV-CELL-FOCUS-WELLFORMED guard.
            }
            self.focus = PeFocus::Cell { row_id, col_id: column_id };
            if self.is_data_column(column_id) {
                self.last_data_column = Some(column_id);
            }
        }
    }
}
```

Wrap behavior follows `policy.wrap_column_axis`. Default: `NoWrap`. Wrap behavior is moot on `Band` (no movement possible) and `None` (no current column).

Operation emitters (Left / Right arrow, Tab) consult `navigable_columns_for_row(focus_key)` and emit `ColumnIntent::To` with a navigable target. The empty navigable set on band-archetype rows produces "no target" → no intent emitted → keypress consumed without movement (Left/Right) or routed to row-axis behavior (Tab).

## 10.3 Cross-archetype row transitions

Row-axis navigation (`OrderedSelectionAction::FocusPrevious | FocusNext | FocusFirst | FocusLast | FocusPage | MoveFocus*`) mutates `selection.focus_key` via the reducer. The funnel then projects the new `focus_key` into `core.focus` via `set_focus_row` (§10.1).

The transition matrix:

| Prior `focus` | New row's `kind` | Resulting `focus` |
|---|---|---|
| `Cell { col }` | `Data` | `Cell { row: new, col: col if navigable on new, else last_data_column if navigable, else first Data col }` |
| `Cell { col }` | `Section` / `SubSection` | `Band { row: new }`; cache `last_data_column = col` if col is Data |
| `Cell { col }` | (row absent) | `None`; cache `last_data_column = col` if col is Data |
| `Band` | `Data` | `Cell { row: new, col: last_data_column if navigable, else first Data col }` |
| `Band` | `Section` / `SubSection` | `Band { row: new }` |
| `Band` | (row absent) | `None` |
| `None` | `Data` | `Cell { row: new, col: first Data col }` |
| `None` | `Section` / `SubSection` | `Band { row: new }` |
| `None` | (row absent) | `None` |

The four cases of the prior §10.3 (Data → Section; Section → Data with cache hit; Section → Data with cache miss; Section → Data with no cache) are encoded as rows of this matrix. The funnel computes the transition in `set_focus_row`'s one place; consumers do not implement the rules per-call-site.

`ensure_focus_visible` calls `set_focus_row(self.selection.focus_key)` after expand / collapse-induced re-anchor — the projection is re-run against the (possibly different) new row, and INV-CELL-FOCUS-WELLFORMED + INV-BAND-FOCUS-WELLFORMED hold by construction.

## 10.4 Tab cell traversal

Tab and Shift+Tab traverse cells within PE under the policy `tab_at_row_end` and `tab_at_last_cell`. Defaults: `tab_at_row_end = WrapToNextRow`, `tab_at_last_cell = NoOp`.

**INV-PE-TAB-PE-DRIVEN.** Tab cell traversal is PE-driven, not framework-driven via `focus_next` + auto-capture. Rationale: mount-on-demand makes per-cell focusable children impossible in Navigation mode; the framework's auto-capture chain (SPEC-HAL-328 §3.7) targets adjacent capture-eligible widgets in structural tree order, which PE's virtual cells do not provide.

Concrete behavior:

- **Navigation Tab in PE.** PE Outer's `handle_keyboard` consumes Tab in Navigation mode: advances cursor cell-by-cell per `tab_at_row_end` / `tab_at_last_cell`; returns `true` to consume.
- **Capture Tab in editor.** Editor commits, fires `on_exit(CommitAndAdvance, value)`, returns `true`. PE's exit closure advances cursor + mounts next editor + requests capture.
- **At PE's last cell with `tab_at_last_cell = NoOp` (default).** Tab is consumed as a no-op (INV-F8 trap inside PE scope).
- **At PE's last cell with `tab_at_last_cell = WrapToFirstCell`.** Tab wraps to the first navigable cell of the first visible row.

The framework's release + focus_next + auto-capture chain (§3.7 of HAL-328) is reserved for boundary transitions out of PE entirely; it is not invoked for PE-internal cell traversal.

## 10.5 Keyboard binding summary

In Navigation mode, with PE focused:

| Input | Action |
|---|---|
| h / ArrowLeft | Cursor left within row (or no-op if leftmost; no tree-collapse from data columns) |
| l / ArrowRight | Cursor right within row (or descend to first child on expanded tree row from the tree column) |
| j / ArrowDown | Row focus down |
| k / ArrowUp | Row focus up |
| Tab | Cell next (§10.4) |
| Shift+Tab | Cell previous |
| i / Enter | On `PeFocus::Cell { row, col }`: if cell is editable and non-Bool — begin edit (mount editor + capture); if cell is Bool — toggle (always-mounted Checkbox activate). On `PeFocus::Band { row }`: if row has children — toggle expansion; otherwise no-op. On `PeFocus::None`: no-op. |
| Space | On `PeFocus::Cell` — toggle row selection (`OrderedSelectionAction::ToggleFocused`). On `PeFocus::Band` — no-op (Section / SubSection are not selectable per §5.5.1). On `PeFocus::None` — no-op. |
| Escape | Framework: release capture if in capture; otherwise pop scope or no-op. PE never overrides. (SPEC-HAL-328 §3.2; FORBID-KBD-ESCAPE-SEMANTICS.) |

---

# 11. Interaction Policy

```rust
#[derive(Clone, Debug)]
pub struct PropertyEditorPolicy {
    pub wrap_row_axis: WrapBehavior,
    pub wrap_column_axis: WrapBehavior,
    pub tab_at_row_end: TabRowEnd,
    pub tab_at_last_cell: TabBoundary,
    pub edit_trigger: EditTrigger,
    pub pointer_down_on_cell: PointerDownAction,
    pub hover: HoverAction,
    pub blur_disposition: EditDisposition,
    pub click_elsewhere_disposition: EditDisposition,
    pub header_click: HeaderClickAction,
    pub empty_area_click: EmptyAreaClickAction,
}

#[derive(Copy, Clone, Debug)] pub enum WrapBehavior { NoWrap, Wrap }
#[derive(Copy, Clone, Debug)] pub enum TabRowEnd { WrapToNextRow, StayInRow }
#[derive(Copy, Clone, Debug)] pub enum TabBoundary { NoOp, WrapToFirstCell }
#[derive(Copy, Clone, Debug)] pub enum EditTrigger { SingleClick, DoubleClick, EnterOrI, ManualOnly }
#[derive(Copy, Clone, Debug)] pub enum PointerDownAction { MoveCursor, MoveCursorAndBeginEdit }
#[derive(Copy, Clone, Debug)] pub enum HoverAction { None, Highlight }
#[derive(Copy, Clone, Debug)] pub enum EditDisposition { Commit, Cancel }
#[derive(Copy, Clone, Debug)] pub enum HeaderClickAction { None, Sort }
#[derive(Copy, Clone, Debug)] pub enum EmptyAreaClickAction { FocusEditor, NoOp }
```

Rationale: `SectionCursorColumn` and `section_row_cursor_column` selected which column hosted the cursor on a section row. The archetype model retires this — Section / SubSection rows host `PeFocus::Band { row_id }` with no column component. The policy field has no semantic target.

## 11.1 Defaults

`Default for PropertyEditorPolicy`:

```text
wrap_row_axis              = NoWrap
wrap_column_axis           = NoWrap
tab_at_row_end             = WrapToNextRow
tab_at_last_cell           = NoOp
edit_trigger               = DoubleClick
pointer_down_on_cell       = MoveCursor
hover                      = Highlight
blur_disposition           = Commit
click_elsewhere_disposition= Commit
header_click               = None
empty_area_click           = FocusEditor
```

## 11.2 `HeaderClickAction::Sort`

`Sort` does not implement sorting inside PE; it emits an intent that the consumer's data layer handles. The intent variant is reserved in `DataLayerIntent` (§12). No emitter ships in this rewrite; the variant exists so the future feature has its seam.

## 11.3 Blur disposition (`blur_disposition` and `click_elsewhere_disposition`)

Both are `EditDisposition::Commit` by default. Both fire only on engine-state blur (focus change initiated by user action within PE / Halcyon's domain). OS-deactivation blur is not a trigger (INV-PE-SOVEREIGNTY; FORBID-PE-COMMIT-FROM-OS-BLUR).

---

# 12. Intent Dispatch Pipeline

## 12.1 Funnel

```rust
pub enum PEIntent {
    Selection(OrderedSelectionAction<u64>),
    Column(ColumnIntent),
    Edit(EditIntent),
    Tree(TreeIntent),
    Activate { row_id: u64 },
    DataLayer(DataLayerIntent),
}

pub enum DataLayerIntent {
    // Reserved for future filter / sort / search / grouping intents.
    // No variants emit in this SPEC's scope. The variant exists so the
    // funnel honors data-layer intents without architectural change
    // when those features land.
}
```

Every event handler that would mutate `selection`, `focus`, `edit_session`, `hts` topology, or `validation_error` constructs a `PEIntent` and calls `core.dispatch_intent(intent) -> bool`.

## 12.2 `dispatch_intent`

```rust
impl PropertyEditorCore {
    pub(crate) fn dispatch_intent(&mut self, intent: PEIntent) -> bool {
        // Permission gate.
        if let Some(ds) = &self.data_source {
            let query = self.build_permission_query(&intent);
            if ds.borrow().permission_check(query) == PermissionResult::Deny {
                return false;
            }
        }
        match intent {
            PEIntent::Selection(action) => {
                reduce_ordered_selection(&mut self.selection, action, &self.hts);
            }
            PEIntent::Column(c)         => self.apply_column_intent(c),
            PEIntent::Edit(e)           => self.apply_edit_intent(e),
            PEIntent::Tree(t)           => self.apply_tree_intent(t),
            PEIntent::Activate { row_id } => self.activate_row(row_id),
            PEIntent::DataLayer(d)      => self.forward_to_data_layer(d),
        }
        true
    }
}
```

`apply_column_intent`, `apply_edit_intent`, `apply_tree_intent` are PE-local helpers that perform the view-domain mutation. `forward_to_data_layer` issues the corresponding call on `data_source` (when variants ship).

## 12.3 Post-dispatch side effects

After a successful `dispatch_intent` call, the caller (event handler) applies side effects:

- `request_scroll_to_focus(core)` after focus changes.
- `ensure_focus_visible(core, visible_rows)` after tree-structural changes.
- `cx.invalidate_layout()` or `cx.invalidate_layout_by_id(body_id)` after any visible state change.

These are not in `dispatch_intent` itself, because they require `cx`. They are uniformly applied at the event-handler return path.

---

# 13. Permission Gate

## 13.1 Foundational seam

`PropertyDataSource::permission_check` (§5.3) is the seam. Default implementation: `Permit`. Consumers that opt into the access policy port (future SPEC) override.

## 13.2 Query construction

`PropertyEditorCore::build_permission_query(intent)` maps a `PEIntent` to a `PermissionQuery`:

```rust
fn build_permission_query(&self, intent: &PEIntent) -> PermissionQuery {
    match intent {
        PEIntent::Selection(action) => PermissionQuery {
            row_id: extract_row_from_action(action).unwrap_or(0),
            column_index: None,
            action: match action {
                OrderedSelectionAction::DragSelect { .. } => PermissionAction::DragExtend,
                _ => PermissionAction::Select,
            },
        },
        PEIntent::Column(c) => PermissionQuery {
            row_id: self.selection.focus_key.unwrap_or(0),
            column_index: Some(self.column_index_of(c.target_column())),
            action: PermissionAction::Focus,
        },
        PEIntent::Edit(e) => PermissionQuery {
            row_id: e.row_id(),
            column_index: Some(self.column_index_of(e.column_id())),
            action: PermissionAction::Edit,
        },
        PEIntent::Tree(t) => PermissionQuery {
            row_id: self.node_id_for_index(t.row_index()),
            column_index: None,
            action: PermissionAction::Tree,
        },
        PEIntent::Activate { row_id } => PermissionQuery {
            row_id: *row_id,
            column_index: None,
            action: PermissionAction::Activate,
        },
        PEIntent::DataLayer(_) => /* port supplies query shape per variant */ {
            PermissionQuery { row_id: 0, column_index: None, action: PermissionAction::Select }
        },
    }
}
```

## 13.3 Caching

Per-frame caching of permission outcomes is deferred to the access policy port. This SPEC mandates only the seam and the default-permit behavior. The port is free to add caching, batch queries, or staleness invalidation without breaking this SPEC.

---

# 14. Layout and Paint Phase Order

## 14.1 PE Outer layout

1. Cache theme values (`font_size`, `text_padding`).
2. Resolve HTS column geometry once at composite level.
3. Layout `children[0]` (ScrollArea wrapping Body) at the body area.
4. Layout `children[1]` (Footer) below.
5. If `edit_session.is_some()` AND the editor's column was resized between layouts: `cx.invalidate_layout_by_id(body_id)` to re-position the editor child.

## 14.2 PE Body layout

1. `core.hts.set_viewport_height(...)`.
2. `(visible, total) = core.hts.visible_rows_with_total()`.
3. Compute `first_sorted`, `content_height`.
4. Reconcile `disclosure_toggles` pool.
5. Reconcile `always_mounted_editors` pool (Bool cells in visible rows where `kind == Data` AND `cell(row, col)` returns `Some(Cell { value: Bool(_), editable: true, .. })` AND `permission_check(Edit)` permits). Section / SubSection rows skip this pass — their cell archetype is `None` (§5.5.1).
6. **Edit session reconciliation:**
    - If `edit_session.is_some()` AND `editor_node_id.is_none()`: create editor via `create_edit_widget`; add as child to Body; store `editor_node_id`; layout_child + set_child_rect inside the cell; `cx.request_keyboard_capture(editor_widget_id)`.
    - If `edit_session.is_some()` AND `editor_node_id.is_some()`: keep existing editor; layout_child + set_child_rect (cell rect may have moved).
    - If `edit_session.is_none()` AND Body's `current_edit_node.is_some()`: remove the editor child node from the tree (deferred tree-structural mutation per INV-PE-TREE-DEFERRED); clear `current_edit_node`.
7. Set `tree_col_clip` via `clip_bounds`.
8. Return `LayoutResult::new(viewport_size, content_size)`.

## 14.3 PE Body paint (Z-order, lowest to highest)

Body's paint pipeline contains exactly one structural row-level fork: `RowArchetype` (derived from `PropertyNodeKind`). Band rows (Section / SubSection) execute the whole-row band-paint path in pass 2 and skip passes 3 and 4. Data rows execute the selection background in pass 2 and the unified per-cell loop in pass 4. No paint pass at the outer structural level branches on a specific `ColumnId` value. Cell-content dispatch by column kind happens inside pass 4's loop body — the kind-dispatch seam — per INV-CELL-PAINT-UNIFORM (§9.1) and FORBID-PE-COLUMN-ID-CARVE-OUT (§9.2).

1. `LAYER_CONTENT − 2`: default row background (full content width × full content height; underlies all per-row primitives).

2. `LAYER_CONTENT − 2`: per-row archetype paint, branched on `RowArchetype` (the only structural row-level fork):
   - **Data + selected**: `list_colors.row_selected` (focused) or `row_selected_inactive` (unfocused), full row width.
   - **Data + not selected**: no extra background (pass 1's row_background carries).
   - **Section**: `list_colors.header_background`, full band width; band label painted with full-band-width clip (no truncation at the tree-column right edge — the band reads as a continuous strip); if the row is band-hovered (`core.hovered_row == Some(row_id)`): `theme.band_hover_overlay`; if `core.focus == PeFocus::Band { row_id }` AND `core.focused`: `theme.band_focused_overlay`.
   - **SubSection**: `list_colors.subheader_background`, full band width; band label painted with full-band-width clip; overlays as for Section.

3. `LAYER_CONTENT − 1`: tree guide lines (**Data rows only** — clipped to tree column).

4. `LAYER_CONTENT`: **unified per-cell loop (Data rows only).** For each visible Data row, for each column slot in the row's column order — with no `continue` on any specific `ColumnId`:
   - **4a (hover overlay).** If `core.hovered_cell == Some((row_id, slot.column_id))`: push `list_colors.row_hover` rect at the cell rect on `LAYER_CONTENT`. Applies uniformly across every cell-kind of every Data row.
   - **4b (cell content, dispatched on the column's registered `ColumnKind`).** The kind-dispatch seam — the only legal column-discrimination mechanism in pass 4. Per `#[non_exhaustive] ColumnKind`:
     - **Tree-kind cell** (the column whose id equals `tree_column_id`, identified by its registered kind metadata — not by a literal `column_id == tree_column_id` comparison at structural level): reserve `TREE_CONTROL_WIDTH` for the disclosure-toggle child widget; apply depth indent; paint the tree label clipped to the cell rect.
     - **`ColumnKind::Data` cell**: paint the value display per `PropertyValue` variant; paint validation-error decoration on the editing cell. Paint is suppressed for the cell hosting the active editor (the editor child widget paints its own content within its set child rect) and for Bool cells (the always-mounted Checkbox child occupies that paint space).
     - **Future kinds**: per the `#[non_exhaustive] ColumnKind` model, additional variants dispatch here when introduced. No consumers ship in this SPEC's scope.
   - **4c (cell focus border).** If `core.focus == PeFocus::Cell { row_id, col_id }` matches this cell: push a cell-sized focus rectangle at `LAYER_FOCUS`, with both X edges snapped to physical pixel boundaries. No row-spanning border. Tree-kind cells receive the same indicator as `ColumnKind::Data` cells.

5. `LAYER_CONTENT + 1`: column separators (subtle pass). Y-segmented to skip Section / SubSection bands — the bands read as continuous strips, not carved-up rows.

6. `LAYER_CONTENT + 2`: column separators (hovered / active pass). Same Y-segmentation as pass 5.

Editor child widget paints at the framework's child paint pass within its set child rect; pass 4b emits no content paint for the cell occupied by the active editor and for Bool cells (the always-mounted Checkbox child paints that cell). Section / SubSection rows do not enter pass 3 or pass 4 — the row-archetype fork at pass 2 routes their entire content (background, label, hover overlay, focused overlay) through the band-paint primitive.

---

# 15. Migration Path

## 15.1 Build beside the old, swap at the end

1. New module lives at `src/widgets/property_editor_v2/` during construction.
2. The existing `src/widgets/property_editor/` continues to compile and run. Showcase and demos that depend on it remain functional throughout the migration.
3. Each CC task implementing this SPEC is bounded: one section of this SPEC, complete and verifiable on its own.
4. When the new module is complete and demonstrated, `src/widgets/mod.rs` re-exports from `property_editor_v2`; the old `property_editor/` directory is removed in a final cleanup task.

## 15.2 Task sequencing (informative; the binding plan is Step 4)

Indicative order:

1. `types.rs` carry-over + new types.
2. `core.rs` skeleton.
3. `policy.rs`.
4. `editor_registry.rs` factory + types (consumes existing editor widgets + the forthcoming `KeybindEditor` per SPEC-HAL-329).
5. `body.rs` skeleton (Widget impl, HTS wiring).
6. `body.rs` layout (visible rows, geometry, scroll sync).
7. `body.rs` paint (rows, cells, selection, focus).
8. `body.rs` disclosure reconciliation.
9. `body.rs` always-mounted editor reconciliation (Bool).
10. `body.rs` edit_session reconciliation (mount-on-demand editors).
11. `body.rs` pointer event handling (cell focus, drag-select, double-click activation).
12. `property_editor.rs` skeleton + composition.
13. `property_editor.rs` layout + paint (header).
14. `property_editor.rs` keyboard handling (Navigation mode bindings).
15. `property_editor.rs` column resize.
16. `dispatch_intent` integration (permission gate location).
17. Demo: multi-column showcase exercising every PropertyValue archetype.
18. Swap imports; delete old `src/widgets/property_editor/`.

The KeybindEditor SPEC (SPEC-HAL-329) and its implementation land before step 4 of the above sequence, so `editor_registry.rs` consumes a real widget. Step 4's task plan sequences the SPEC-HAL-329 work explicitly.

---

# 16. Non-Goals

This SPEC governs only the PropertyEditor rewrite. The following are out of scope:

- **TabularSubstrate (SEALED BASE, SPECs 164–178).** Not touched.
- **ListView, TreeView.** PE mirrors TreeView's composition patterns; TreeView itself is unchanged.
- **HierarchicalTabularSubstrate, hierarchical engines.** Consumed only; not modified.
- **Inline editor widgets** (`TextField`, `Checkbox`, `ComboBox`, `ColorPickerField`). Consumed only; not modified.
- **`KeybindEditor`** lives at `src/widgets/keybind_editor.rs` per SPEC-HAL-329, not inside the PropertyEditor module.
- **Framework focus / capture / keyboard machinery** (SPECs 038, 089, 126, 140, 176, 184, 235, 326, 328). PE complies; PE does not modify.
- **Access policy / permissions (full port).** Deferred. This SPEC mandates the foundational seam (§13) and the default-permit behavior; the port introduces caching, batching, action enum extensions, and richer query shapes.
- **Filter / sort / search.** Data-layer concerns per §3 / FORBID-PE-FILTER-IN-VIEW. UI affordances, if introduced later, route through data-layer queries and intents. New `PropertyDataSource` query methods (`sort_state`, `filter_state`, etc.) land with their corresponding features.
- **Dynamic grouping / aggregation.** Deferred. Static grouping via `PropertyNode.kind` (§5.5 — `Section` and `SubSection` archetypes) is supported.
- **Pagination.** N/A. HTS virtualization renders arbitrarily large datasets without pagination.
- **File / Resource cell archetype.** Deferred. The general "resource" abstraction (covering file, link, etc.) is a future SPEC.
- **Date / Duration / URL / Email / Range / Tags / other future `PropertyValue` variants.** Deferred.
- **Multi-clip child widget support in the framework.** Deferred. Cell-level paint discipline relies on each editor staying inside its set child rect.
- **Sort on header click.** The intent variant is reserved (§11.2); no emitter ships.
- **Context menu (right-click).** Deferred.
- **Multi-select cell-mode (spreadsheet-style range across cells × rows).** Out of scope; PE's selection is row-axis only.

---

# 17. Constitutional Compliance Citations

## 17.1 SPEC-HAL-001 §2.5 — Sovereignty Boundary

OS focus signals are advisory; engine state is authoritative.

- **INV-PE-SOVEREIGNTY** (§9.1): PE derives focus / capture / modality / routing dispositions from engine state, not OS state.
- **FORBID-PE-COMMIT-FROM-OS-BLUR** (§9.2): blur disposition fires only on engine-state blur; OS-deactivation does not trigger commit.

## 17.2 SPEC-HAL-002 §3.7 — No Direct UI Mutation From Callbacks

Widget callbacks must not mutate other widgets or the tree directly.

- **INV-PE-TREE-DEFERRED** (§9.1): editor `on_exit` callbacks mutate `core.edit_session` only. Tree-structural removal of the editor child happens during Body's next layout pass (§14.2 step 6).
- **FORBID-PE-CALLBACK-TREE-MUTATION** (§9.2): editor exit callbacks must not call `tree.remove_node`, `tree.add_child`, or any other structural tree API.

## 17.3 SPEC-HAL-002 §3.11 — No Interpretive Intent Resolution

Widgets must not reinterpret, normalize, infer, or reconcile user intent.

- **INV-PE-VERBATIM-COMMIT** (§9.1): values passed to `commit_cell` are exactly what the editor surfaced via `on_exit`. No normalization at the PE boundary.
- **FORBID-PE-AUTO-MATCH-AT-COMMIT** (§9.2): PE does not match typed input to existing options, does not case-normalize, does not substitute closest options.

Editor-internal disambiguation (e.g., ComboBox's selected option vs free-typed text) is the editor's responsibility, governed independently by SPEC-HAL-002 §3.11 within each editor's SPEC.

## 17.4 SPEC-HAL-002 §4.11 — Explicit Commit Authority

All commits originate from explicit user command.

- **INV-PE-COMMIT-EXPLICIT** (§9.1): every commit traces to pointer activation, keyboard activation, or programmatic command via Context. Blur-disposition commits are permitted only when the blur was caused by user action within engine semantics. OS-deactivation blur, programmatic focus shifts triggered by non-user code, and other inferred signals do not commit.

## 17.5 SPEC-HAL-259 — HierarchicalTabularSubstrate Architecture

HTS is the sole topology authority for PE's row and column axes. PE consumes HTS APIs; PE does not extend or modify HTS or its underlying TabularSubstrate.

## 17.6 SPEC-HAL-328 — Keyboard Authority, Focus, and Capture

PE complies with the keyboard authority model. PE's editors are Category D (TextField), E (ComboBox / ColorPickerField), and K (KeybindEditor, SPEC-HAL-329). PE Outer is a focus-scope container during cell-mode operation (Tab is trapped per INV-PE-TAB-PE-DRIVEN, §10.4); Escape is owned unconditionally by the framework per SPEC-HAL-328 §3.2 and FORBID-KBD-ESCAPE-SEMANTICS.

---

# 18. Resolved Decisions

The Step 2 design document raised two open questions for JC. Resolutions are in force here:

## 18.1 KeyBinding editor (Step 2 §14.1, OQ6)

**Resolved: SPEC-HAL-329 (forthcoming) — a standalone `KeybindEditor` widget at `src/widgets/keybind_editor.rs`.** PE consumes it via `editor_registry.rs`. The widget produces `Shortcut` values via one-shot capture; registration with `BindingRegistry` is the consumer's responsibility, not the widget's and not PE's. See §6.4.

## 18.2 Filter feature (Step 2 §14.2, OQ8)

**Resolved: drop filter UI from PE per §3 Data-Layer Sovereignty.** Filtering is a data-layer concern. The consumer's `nodes()` returns the filtered row topology and `cell()` returns the filtered cells; PE re-reads via the data source after a `revision()` bump and displays the result. If a filter UI affordance is needed in the future, it returns as a separate composable widget that wraps PE and emits intents to the data layer (FORBID-PE-FILTER-IN-VIEW, §9.2; non-goal, §16).

---

**End of SPEC-HAL-260A.**
