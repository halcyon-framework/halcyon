# SPEC-HAL-260: PropertyEditor & Grid Specialization Model

**Author:** JC
**Status:** Superseded — by SPEC-HAL-260A (PropertyEditor).

> **Supersession notice:** The implementation built against this SPEC — PropertyEditor v1 — has been deleted (commit `a72904e2`); PropertyEditor v2 was promoted to `widgets::property_editor` and is governed by SPEC-HAL-260A, which formally supersedes this document. Retained for historical reference only — do not implement from it.
**Layer:** Substrate Specialization
**Depends On:**
- SPEC-HAL-255 (Column Geometry Engine Extraction) — **COMPLETE**
- SPEC-HAL-256 (Row Topology Engine Architecture) — **COMPLETE**
- SPEC-HAL-257 (Virtualization Strategy Layer) — **COMPLETE**
- SPEC-HAL-258 (Engine Invalidation Graph & Data Flow Law) — **COMPLETE**
- SPEC-HAL-259 (HierarchicalTabularSubstrate Architecture) — **COMPLETE**
- SPEC-HAL-189 (CellHost — Cell Widget Lifecycle Manager) — **COMPLETE**
- SPEC-HAL-174 (Inline Edit Lifecycle) — **COMPLETE** (EditState/EditIntent/EditPayload in TabularSubstrate)

---

## 1. Purpose

Define how PropertyEditor (and future Grid) are built as
specializations of extracted structural engines — not as forks
of ListView or ad-hoc widget branches.

This SPEC formalizes:
- Engine composition
- Topology specialization
- Property data model
- Editor widget mounting strategy
- Editing integration boundaries
- Phased implementation plan

---

## 2. Core Principle

There is no "ListView reuse" for these systems.

There is:
- Shared engines (all extracted, all in production)
- Shared cell lifecycle management (CellHost)
- Shared editing types (EditState, EditIntent, EditPayload)
- Specialized topology metadata
- Specialized editor mounting
- Specialized navigation behavior

No substrate may depend on ListView internals.

---

## 3. Shared Engine Baseline — Implementation Status

All engines exist in `src/engines/` and are in production use
by ListView (via TabularSubstrate) and TreeView (via HTS).

| Engine | Location | Status |
|--------|----------|--------|
| ColumnGeometryEngine | `src/engines/column_geometry/mod.rs` | Production (846 lines) |
| TopologyEngine trait + FlatRowTopology | `src/engines/row_topology/mod.rs` | Production (276 lines) |
| VirtualizationStrategy + VirtualizationEngine | `src/engines/virtualization/mod.rs` | Production (261 lines) |
| HierarchicalRowTopology | `src/engines/hierarchical/topology.rs` | Production (659 lines) |
| HierarchicalBlockVirtualization | `src/engines/hierarchical/virtualization.rs` | Production (153 lines) |
| HierarchicalTabularSubstrate (HTS) | `src/engines/hierarchical/mod.rs` | Production (593 lines) |

Shared lifecycle management:

| Component | Location | Status |
|-----------|----------|--------|
| CellHost | `src/widgets/list_view/cell_host.rs` | Production — generic (zero ListView dependencies). Needs relocation to shared module. |
| DirtyFlags / DirtyOutcome / UpdatePhase | `src/widgets/list_view/row_host.rs` | Production — pure dirty tracking primitives. Needs relocation with CellHost. |
| EditState / EditSession | `src/widgets/tabular_view/mod.rs:83-139` | Production — Idle/Armed/Editing lifecycle |
| EditIntent / EditPayload | `src/widgets/tabular_view/intents.rs:183-243` | Production — Begin/Commit/Cancel with String/Number/Bool/None |

---

## PART I — PropertyEditor

---

## 4. PropertyEditor Structural Shape

PropertyEditor is:
- Hierarchical (grouped properties, nested objects)
- Two-column (name + value editor)
- Type-aware (different editor per property type)
- Inline-editable (click value cell → editor appears)
- Row-driven (primary axis is vertical)
- Usable for: settings panels, inspector panels, keybind editors, preference dialogs, debug property sheets

It composes:
- HierarchicalTabularSubstrate (HTS) — owns hierarchy, topology, virtualization, scroll, columns
- CellHost — per-cell editor widget lifecycle
- EditState/EditIntent types — editing lifecycle (reused, not reimplemented)
- PropertyEditor widget — orchestration, layout, paint, input

It does NOT use ListView or TabularSubstrate.

---

## 5. Property Data Model

### 5.1 PropertyValue

```rust
/// Value types supported by PropertyEditor.
///
/// Each variant determines which editor widget is mounted
/// in the value cell.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Free-form text. Editor: TextField.
    String(String),
    /// Integer. Editor: SpinBox or TextField with validation.
    Integer(i64),
    /// Floating point. Editor: SpinBox or TextField with validation.
    Float(f64),
    /// Boolean toggle. Editor: Checkbox or Switch.
    Bool(bool),
    /// Choice from fixed options. Editor: ComboBox or DropdownMenu.
    Choice { selected: usize, options: Vec<String> },
    /// Color value. Editor: ColorPickerField.
    Color(Color),
    /// Key binding. Editor: KeyCaptureField (new, simple).
    KeyBinding(String),
    /// Read-only display value. Editor: Label (no editing).
    ReadOnly(String),
    /// Section header (group label, no value). No editor.
    Section,
}
```

### 5.2 PropertyNode

```rust
/// A single property in the PropertyEditor tree.
///
/// Wraps HTS NodeMeta with property-specific metadata.
/// The `node_id` field maps 1:1 to NodeMeta.node_id.
pub struct PropertyNode {
    /// Stable identifier. Same as NodeMeta.node_id.
    pub node_id: u64,
    /// Display name shown in the name column.
    pub label: String,
    /// Current value and implied editor type.
    pub value: PropertyValue,
    /// Whether the user can edit this property.
    pub editable: bool,
    /// Parent node_id (None for root-level properties).
    pub parent_id: Option<u64>,
}
```

### 5.3 PropertyDataSource

```rust
/// Data source for PropertyEditor.
///
/// Consumer implements this to provide property data.
/// PropertyEditor calls these methods during rebuild.
pub trait PropertyDataSource {
    /// Return the full property tree.
    ///
    /// Called when the editor is first built or when the consumer
    /// signals data has changed via `revision()`.
    fn properties(&self) -> Vec<PropertyNode>;

    /// Monotonic revision counter. PropertyEditor re-reads
    /// properties() when revision changes.
    fn revision(&self) -> u64;

    /// Called when the user commits an edit.
    ///
    /// The consumer validates and applies the change.
    /// Returns true if accepted, false if rejected.
    fn commit(&mut self, node_id: u64, value: PropertyValue) -> bool;
}
```

### 5.4 Data Flow

```
Consumer provides PropertyDataSource
    → PropertyEditor reads properties() on revision change
    → Builds Vec<NodeMeta> from PropertyNode tree (label, parent, depth)
    → Feeds NodeMeta to HTS via set_tree_data()
    → Keeps PropertyNode metadata in a parallel HashMap<u64, PropertyNode>
    → CellHost mounts editor widgets per value type in the value column
    → On edit commit: calls consumer.commit(node_id, new_value)
    → Consumer updates data, bumps revision
    → PropertyEditor re-reads on next frame
```

### 5.5 Topology Requirements (from original SPEC)

PropertyEditor uses HierarchicalRowTopology (already implemented
in `src/engines/hierarchical/topology.rs`). The following
capabilities are required and satisfied:

Required capabilities:
- Parent/child relationships
- Expand/collapse
- Depth calculation
- Deterministic projection
- O(1) row_index lookup

Forbidden:
- No column knowledge in topology
- No editing logic in topology
- No rendering semantics in topology

These constraints are enforced by HierarchicalRowTopology's
existing API surface — it exposes only structural queries
(depth, parent, children, expand/collapse) and never
references columns, editors, or rendering.

---

## 6. Editor Mounting Strategy

### 6.1 Editor Selection

Each `PropertyValue` variant maps to a specific editor widget:

| PropertyValue | Editor Widget | Behavior |
|---------------|---------------|----------|
| String | TextField | Free-form text entry |
| Integer | SpinBox | Up/down + direct entry, integer validation |
| Float | SpinBox (float mode) | Up/down + direct entry, float validation |
| Bool | Checkbox | Toggle on click |
| Choice | ComboBox | Dropdown selection from options |
| Color | ColorPickerField | Color swatch + picker popup |
| KeyBinding | TextField (capture mode) | Captures next key combination |
| ReadOnly | Label | Display only, non-editable |
| Section | (none) | Group header, no value cell |

### 6.2 Editor Lifecycle

PropertyEditor reuses the existing editing types from
`tabular_view/intents.rs`:

- **EditIntent::RequestBeginEdit** — user clicks/Enter on value cell
- **EditIntent::RequestCommitEdit** — user presses Enter or clicks away
- **EditIntent::RequestCancelEdit** — user presses Escape

PropertyEditor owns its own `EditState` instance (not shared with
TabularSubstrate — each PropertyEditor has independent edit
lifecycle). The types are shared; the instances are separate.

### 6.3 CellHost Integration

CellHost manages editor widget creation and dirty tracking:

- **CellKey** = (RowId from HTS visible row, value column ColumnId)
- **Creation**: when a row becomes visible, CellHost.get_or_create()
  creates the appropriate editor widget based on PropertyValue type
- **Rebind**: when PropertyDataSource revision changes, CellHost
  marks affected cells as binding-dirty → editor widgets re-read values
- **Eviction**: when a row scrolls out of viewport, the widget
  survives in CellHost (identity-stable). No creation cost on re-scroll.
- **Identity death**: only on full data replacement (revision jump)

---

## 7. PropertyEditor Layout

### 7.1 Column Structure

PropertyEditor has exactly two logical columns:

| Column | Content | Behavior |
|--------|---------|----------|
| Name | Property label with tree indentation + disclosure toggle | Read-only, tree-painted |
| Value | Editor widget (type-dependent) | Editable (if property.editable) |

Both columns use ColumnGeometryEngine. The name column has a
fixed minimum width. The separator between name and value is
resizable (same resize mechanism as ListView).

Section rows (PropertyValue::Section) span the full width —
no value column. They render as group headers with expand/collapse.

### 7.2 Row Height

Fixed row height for all property rows (e.g., 28px). Section
headers may have a different height (e.g., 32px). This is
handled by HTS's RowHeights in variable-height mode with
section rows set to the header height.

### 7.3 Tree Rendering

The name column renders:
- Indentation based on depth (same TREE_INDENT_STEP as TreeView)
- Disclosure toggle for nodes with children (same DisclosureIndicator)
- Property label text

This reuses the tree painting infrastructure from
`tabular_view/tree_paint.rs` — guide lines, stem-down,
indentation. No reimplementation.

---

## 8. PropertyEditor Composition

```
PropertyEditor (widget)
  ├─ HierarchicalTabularSubstrate (HTS)
  │   ├─ ColumnGeometryEngine (shared, SPEC-255)
  │   ├─ HierarchicalRowTopology (SPEC-259)
  │   ├─ HierarchicalBlockVirtualization (SPEC-259)
  │   ├─ RowHeights
  │   └─ ScrollState
  ├─ CellHost (shared, relocated from list_view/)
  │   └─ Editor widgets (TextField, Checkbox, ComboBox, etc.)
  ├─ EditState (type reused from tabular_view, instance owned)
  ├─ PropertyDataSource (consumer-provided)
  └─ PropertyNodeMap: HashMap<u64, PropertyNode>
```

---

## 9. PropertyEditor Invariants

**INV-PE-001 — Topology Isolation**

Editing must not trigger topology rebuild unless adapter
revision changes. Editor commit calls consumer.commit(),
consumer bumps revision, PropertyEditor re-reads properties()
on next frame. The editing lifecycle is decoupled from topology.

**INV-PE-002 — Column Geometry Stability**

Type changes must not mutate column widths. The name/value
split is controlled by the user via resize. Editor widget
type changes (e.g., String → Choice after data refresh) do
not affect column geometry.

**INV-PE-003 — Deterministic Projection**

Projection depends only on:
- PropertyDataSource revision
- Tree expansion state (expand/collapse of groups)

Editing state does not affect projection order.

**INV-PE-004 — Virtualization Independence**

Collapsed subtrees remain single virtualization blocks.
Inline editing does not expand or shrink the virtualization
window.

**INV-PE-005 — Editor Widget Identity**

Editor widgets are keyed by (node_id, value_column_id) via
CellHost. They survive scroll, collapse/expand of sibling
groups, and value changes. They are destroyed only on full
data replacement or when the node is removed from the tree.

**INV-PE-006 — No ListView Dependency**

PropertyEditor must not import, reference, or depend on any
type from `src/widgets/list_view/`. It composes the same
shared engines that ListView uses, but through its own
orchestration.

---

## 10. PropertyEditor Navigation

Keyboard navigation follows TreeView patterns:

| Key | Action |
|-----|--------|
| Up/Down | Move focus between property rows |
| Left | Collapse current group (if expanded), or move to parent |
| Right | Expand current group (if collapsed), or move to value cell |
| Enter | Begin editing focused value cell |
| Escape | Cancel editing, return focus to name cell |
| Tab | Move to next editable value cell |
| Shift+Tab | Move to previous editable value cell |

Section headers (groups) respond to Left/Right for
expand/collapse. They do not receive Tab focus.

---

## PART II — Grid

**Status:** Deferred. Implementation follows after PropertyEditor
validates the engine composition approach. All sections below
remain binding for future implementation.

---

## 11. Grid Structural Shape

Grid is:
- Flat row topology
- Two-dimensional navigation
- Potential row + column virtualization
- Cell-level selection
- Cell-level editing

It uses:
- FlatRowTopology
- ColumnGeometryEngine
- GridVirtualizationStrategy
- CellNavigationEngine
- SelectionEngine
- EditingEngine

Grid is not a ListView.

---

## 12. GridRowTopology

GridRowTopology is:
- Flat
- Identity-based
- Deterministic row order

No hierarchy.

No expansion state.

No depth semantics.

---

## 13. Grid Virtualization

Grid may require:
- Row virtualization
- Column virtualization
- Cell windowing

GridVirtualizationStrategy must:
- Compute visible rows
- Compute visible columns
- Produce visible cell window

Forbidden:
- No topology mutation
- No geometry mutation
- No selection mutation

---

## 14. CellNavigationEngine

Grid requires explicit 2D navigation:
- Up/Down = row movement
- Left/Right = column movement
- Tab traversal
- Range selection via shift

This engine operates on:
- (row_index, column_index)
- Visible window mapping

It must not:
- Mutate topology
- Mutate geometry

---

## 15. Grid Invariants

**INV-GRID-001 — Row/Column Separation**

Row topology must not depend on column geometry.

Column geometry must not depend on row topology.

---

**INV-GRID-002 — Cell Window Determinism**

Given:
- Row window
- Column window

Visible cell matrix must be deterministic.

---

**INV-GRID-003 — Virtualization Non-Feedback**

Scrolling must not mutate:
- Topology
- Selection
- Geometry

---

## 16. PropertyEditor vs Grid — Structural Contrast

| Aspect | PropertyEditor | Grid |
|--------|----------------|------|
| Topology | Hierarchical | Flat |
| Columns | 2 (name + value) | Many |
| Navigation | Vertical-first, tree-aware | 2D |
| Virtualization | Hierarchical block | Row + column |
| Editing | Per-row property value | Per-cell |
| Depth | Yes | No |
| Data source | PropertyDataSource trait | DataSource trait |

They share engines (geometry, virtualization) and cell
lifecycle (CellHost), not topology or orchestration.

---

## 17. CellHost Relocation

CellHost currently lives at `src/widgets/list_view/cell_host.rs`.
It has zero ListView dependencies — it imports only RowId,
ColumnId, and DirtyFlags (all generic tabular types).

Before PropertyEditor implementation begins, CellHost and its
supporting types (DirtyFlags, DirtyOutcome, UpdatePhase from
`row_host.rs`) must be relocated to a shared location:

**Target:** `src/widgets/tabular_view/cell_host.rs`

This is a file move + import update. No code changes. Both
ListView and PropertyEditor import from the shared location.

---

## 18. Performance Model

PropertyEditor:
- O(1) node_id → row_index lookup (HashMap in HierarchicalRowTopology)
- O(k) visible slice (k = viewport rows, typically 20-40)
- Collapsed subtrees avoid projection cost
- CellHost: O(1) cell lookup, identity-stable across scroll
- Editor widget creation: amortized O(1) (created once, reused)

No full-tree scan per frame.

Grid:
- O(1) row lookup
- O(1) column lookup
- O(vr x vc) cell window where vr, vc are visible counts

No full-table scan allowed per frame.

---

## 19. Anti-Patterns Explicitly Forbidden

- Copying ListView and renaming it
- Embedding hierarchy logic inside TabularSubstrate
- Mixing editing state into topology engine
- Allowing geometry change to trigger topology rebuild
- Allowing selection to rebuild projection
- Building a new CellHost instead of reusing the existing one
- Reimplementing tree painting instead of reusing tree_paint.rs
- Reimplementing dirty tracking instead of reusing DirtyFlags

---

## 20. Migration Path — Status

| Step | Description | Status |
|------|-------------|--------|
| 1 | Extract ColumnGeometryEngine (SPEC-255) | **COMPLETE** |
| 2 | Introduce RowTopology abstraction (SPEC-256) | **COMPLETE** |
| 3 | Introduce VirtualizationStrategy abstraction (SPEC-257) | **COMPLETE** |
| 4 | Refactor TabularSubstrate to use extracted engines | **COMPLETE** |
| 5 | Implement HierarchicalTabularSubstrate (SPEC-259) | **COMPLETE** |
| 6 | Relocate CellHost to shared module | **PENDING** |
| 7 | Define PropertyValue / PropertyNode / PropertyDataSource | **PENDING** |
| 8 | Implement PropertyEditor widget | **PENDING** |
| 9 | Implement GridSubstrate (future) | DEFERRED |

---

## 21. Phased Implementation Plan

### Phase 0: CellHost Relocation

Move CellHost + DirtyFlags/DirtyOutcome/UpdatePhase from
`list_view/` to `tabular_view/`. Update imports in ListView.
No behavioral changes. Compile-only task.

### Phase 1: Data Model + Scaffold

- Define `PropertyValue`, `PropertyNode`, `PropertyDataSource`
  in `src/widgets/property_editor/types.rs`
- Create `PropertyEditor` widget struct with HTS + CellHost
- Build property tree from `PropertyDataSource::properties()`
  → `Vec<NodeMeta>` → `HTS::set_tree_data()`
- Two-column layout (name + value) via ColumnGeometryEngine
- Render property labels with tree indentation (reuse tree_paint)
- No editing yet — value column shows read-only labels
- Demo: settings panel with grouped properties

### Phase 2: Editor Mounting

- Mount type-appropriate editor widgets in value cells via CellHost
- TextField for String, Checkbox for Bool, ComboBox for Choice,
  SpinBox for Integer/Float
- Editor widgets receive current value on mount, display it
- No commit/cancel yet — display only

### Phase 3: Editing Lifecycle

- EditState instance owned by PropertyEditor
- Click/Enter on value cell → RequestBeginEdit → editor activates
- Enter → RequestCommitEdit → consumer.commit() called
- Escape → RequestCancelEdit → editor reverts
- Tab/Shift+Tab → commit current, begin next/previous editable
- Keyboard navigation (Up/Down/Left/Right for tree traversal)

### Phase 4: Polish

- Section header styling (group separators)
- Alternating row backgrounds
- Focus ring on active editor
- Scroll-to-reveal on keyboard navigation
- ReadOnly and Section variants
- Color and KeyBinding editor types (if not already covered)

---

## 22. Architectural Commitment

This SPEC establishes:
- PropertyEditor is not a specialized ListView.
- Grid is not a more advanced ListView.
- Both are engine compositions with specialized topology.
- The engine extraction (SPEC-255 through 259) is complete.
- CellHost is a shared component, not a ListView internal.

If this separation is violated, the framework will collapse
under combinatorial special cases.

---

End of SPEC-HAL-260 (Revised April 2026)
