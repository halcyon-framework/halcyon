# SPEC-HAL-116

**Tree & TreeItem Interaction Model**

**Status:** Design — Approved
**Scope:** Hierarchical tree structure interaction semantics
**Applies to:** Tree, TreeItem, Tree (non-virtualized)
**Non-Goals:** Virtualization (SPEC-HAL-118), columnar layout (SPEC-HAL-117), drag-and-drop, node reordering, styling

---

## 1. Problem Statement

Tree widgets fail due to:

* ambiguous expand/collapse trigger zones
* inconsistent keyboard navigation between levels
* selection/expansion state conflation
* unclear parent-child hit-testing
* navigation edge cases at tree boundaries

This SPEC defines a **strict interaction contract** for hierarchical tree structures, focusing on expand/collapse behavior and keyboard traversal.

---

## 2. Core Definitions

### 2.1 Tree

A **Tree** is a hierarchical widget containing:

* a root (visible or hidden)
* nodes that may have children
* expansion state per node

---

### 2.2 TreeItem (Node)

A **TreeItem** is a single node in the tree:

* has a stable identity (`NodeId`)
* may have zero or more children
* has an expansion state (if children exist)
* has a selection state

---

### 2.3 Depth

**Depth** is the nesting level of a node:

* root = depth 0
* children of root = depth 1
* etc.

Depth affects indentation but not interaction semantics.

---

### INV-T1 — One Widget per Tree Node

Each TreeItem MUST have a 1:1 relationship with a logical tree node.

- No item recycling
- No projection
- No viewport-based realization

Virtualized trees are defined exclusively in SPEC-HAL-118.

---

## 3. Expansion State Model

### INV-T2 — Expansion Is Per-Node

Each node with children has an expansion state:

* `expanded: bool`

Nodes without children have no expansion state.

---

### INV-T3 — Expansion State Is External

Expansion state MUST be stored externally:

* `expanded_nodes: Set<NodeId>`

Nodes themselves MUST NOT store expansion flags.

---

### INV-T4 — Collapsed Hides Descendants

When a node is collapsed:

* all descendants are hidden
* descendants retain their individual expansion states
* re-expanding restores previous expansion pattern

---

### INV-T5 — Expand-Only Rule

Expanding a node:

* MUST NOT move focus
* MUST NOT change selection
* MUST only reveal previously hidden descendants

Keyboard Right-Arrow on a collapsed node expands it but does NOT move focus into children until pressed again.

---

### INV-T6 — Flattened Visible Order

The **flattened visible order** is the authoritative traversal sequence for navigation:

* Pre-order depth-first traversal of expanded nodes only
* Collapsed subtrees contribute only their root node
* This order is used for:
    * Up/Down arrow navigation
    * Home/End navigation
    * Range selection

The flattened visible order MUST be recomputed on any expand/collapse operation.

---

## 4. Hit Zones

### INV-H1 — Two Hit Zones Per Node

Each TreeItem has two distinct hit zones:

1. **Expand/Collapse Zone**: the disclosure indicator (arrow/chevron)
2. **Content Zone**: the node label and icon

---

### INV-H2 — Expand Zone Behavior

Click on expand zone:

* toggles expansion state
* does NOT change selection
* does NOT change focus

---

### INV-H3 — Content Zone Behavior

Click on content zone:

* changes selection to this node
* changes focus to this node
* does NOT toggle expansion

---

### INV-H4 — Double-Click on Content

Double-click on content zone behavior by node type:

| Node Type | Has Children | Double-Click Action |
|-----------|--------------|---------------------|
| Parent (collapsed) | Yes | Toggle expansion (expand) |
| Parent (expanded) | Yes | Toggle expansion (collapse) |
| Leaf | No | Emit activation event |

What double-click MUST NOT do:

* MUST NOT change selection
* MUST NOT change focus
* MUST NOT trigger both expand and activate simultaneously

Behavior MUST be explicitly configured.

---

## 5. Keyboard Navigation

### INV-K1 — Vertical Navigation

* `Up Arrow` — move focus to previous visible node
* `Down Arrow` — move focus to next visible node

Navigation operates on the **visible** (flattened) tree, not logical structure.

---

### INV-K2 — Horizontal Navigation (Expansion)

* `Right Arrow`:
    * if collapsed with children → expand
    * if expanded → move focus to first child
    * if leaf → no-op
* `Left Arrow`:
    * if expanded → collapse
    * if collapsed or leaf → move focus to parent

---

### INV-K3 — Jump Navigation

* `Home` — move focus to first visible node
* `End` — move focus to last visible node

---

### INV-K4 — Activation

* `Enter` — activate focused node (emit activation event)
* `Space` — toggle selection of focused node (multi-select mode)

---

### INV-K5 — Type-Ahead (Optional)

Type-ahead search is OPTIONAL and MUST be explicitly enabled.

If enabled:

* Typing characters jumps to next visible node whose label starts with typed prefix
* Prefix matching is case-insensitive
* Prefix resets after configurable timeout (default: 500ms)
* Search wraps from end to beginning
* Only visible nodes participate (collapsed descendants excluded)

If disabled:

* Character input is ignored
* No implicit activation or selection on keypress

Type-ahead MUST NOT:

* change selection
* expand/collapse nodes
* activate nodes

---

## 6. Selection Model

### INV-S1 — Selection Is Node-Based

Selection is expressed as:

* `selected_nodes: Set<NodeId>`

Selection is independent of expansion state.

---

### INV-S2 — Selection Modes

Tree MUST declare exactly one selection mode:

1. **None** — no selection
2. **Single** — at most one node
3. **Multi** — multiple nodes

---

### INV-S3 — Selection Independence from Expansion

Collapsing a node:

* does NOT deselect it
* does NOT deselect its (now hidden) descendants

Selected but hidden nodes remain selected.

---

### INV-S3a — Multiple Selected Descendants on Collapse

When collapsing a node that has multiple selected descendants:

* All selected descendants remain selected (hidden but selected)
* Selection set is unchanged
* Focus relocates to collapsing parent (per INV-F3)
* `get_selection()` continues to return all selected NodeIds including hidden ones

Re-expanding reveals previously selected descendants still selected.

---

### INV-S4 — Keyboard Selection Modifiers

* Arrow without modifier → select focused node (single mode), or move focus only (multi mode)
* `Shift + Arrow` → extend selection range
* `Ctrl/Cmd + Arrow` → move focus without changing selection

---

## 7. Focus Model

### INV-F1 — Single Focus Node

Exactly one node may have keyboard focus.

Focus node is independent of selection.

---

### INV-F2 — Focus Visibility

Focus node MUST always be visible.

If focus is on a node that becomes hidden (parent collapsed), focus moves to the collapsed parent.

---

### INV-F3 — Focus Relocation on Collapse

When collapsing a node causes the focused node to become hidden:

* Focus MUST move to the collapsing parent node
* Focus MUST NOT move to siblings
* Focus MUST NOT move to root implicitly
* Focus MUST NOT move to any ancestor other than the immediate collapsing parent

This is the **only** automatic focus relocation permitted.

---

## 8. Expansion Animation (Out of Scope)

Visual expansion/collapse animation is a rendering concern, not an interaction concern.

This SPEC does not define animation timing or behavior.

---

## 9. Empty Tree Behavior

### INV-E1 — Empty Tree Is Inert

When tree has no nodes:

* no focus
* no selection
* keyboard navigation is no-op

---

## 10. Disabled State

### INV-D1 — Disabled Is Absolute

When disabled:

* no expansion changes
* no selection changes
* no focus changes
* no keyboard interaction

---

## 11. Programmatic Control

### INV-PROG1 — Explicit APIs

Tree MUST expose:

* `expand(node_id)` / `collapse(node_id)` / `toggle_expansion(node_id)`
* `expand_all()` / `collapse_all()`
* `select(node_id)` / `deselect(node_id)` / `clear_selection()`
* `set_focus(node_id)`
* `scroll_to(node_id)` — ensures node is visible (expands ancestors if needed)

---

### INV-PROG2 — User vs Programmatic Expansion

**User-triggered expansion** (click, keyboard):

* MUST emit `on_expand` / `on_collapse` events
* MUST follow all interaction invariants

**Programmatic expansion** (API calls):

* MAY suppress events (configurable: `silent: bool` parameter)
* MUST still follow focus relocation rules on collapse
* MUST NOT bypass invariants

Default: programmatic calls emit events unless explicitly silenced.

---

## 12. Responsibility Boundaries

**Tree** owns:

* expansion state (`expanded_nodes`)
* selection state (`selected_nodes`)
* focus state
* flattened visible order computation
* keyboard navigation logic

**Tree** MUST NOT own:

* visual rendering (delegated to TreeItem painters)
* node layout calculations (delegated to layout system)
* scroll mechanics (delegated to scroll container)
* node data model (delegated to application)

**TreeItem** owns:

* single node hit-zone (expand + content)
* visual indentation rendering

**TreeItem** MUST NOT own:

* expansion state
* selection state
* focus state
* sibling/parent relationships

**Delegated responsibilities:**

* Layout system: determines node rects and indentation
* Scroll container: handles overflow
* Application: provides tree data model
* SPEC-HAL-114: selection model semantics

---

## 13. Event Emission

### INV-E2 — Events

Tree emits:

* `on_expand(node_id)` / `on_collapse(node_id)`
* `on_selection_change(selected_nodes)`
* `on_activate(node_id)` — on Enter or double-click

---

## 14. Forbidden Behavior

Tree MUST NOT:

* change selection on expand/collapse
* deselect nodes on collapse
* move focus to hidden nodes
* expand/collapse on hover
* conflate selection with expansion
* allow focus on hidden nodes
* auto-expand on focus
* auto-expand on hover
* auto-expand on selection
* move focus on expand (only on collapse when focus becomes hidden)
* change selection as side effect of expand/collapse
* relocate focus to siblings or root on collapse

---

## 15. Test Requirements

Any Tree implementation MUST include tests for:

* expand/collapse via click on indicator
* expand/collapse via keyboard
* keyboard navigation (all arrow keys)
* selection independence from expansion
* focus movement on collapse
* multi-selection modifiers
* empty tree behavior
* disabled state

---

## 16. Reference Implementation

Planned widgets: `Tree`, `TreeItem`

Related SPECs:

* SPEC-HAL-114 (List Selection — flat selection model)
* SPEC-HAL-117 (Columnar Views — tree + columns)
* SPEC-HAL-118 (Virtualization — virtualized trees)

---

## Optional Visual Structure — Tree Tracing Lines

Tree implementations MAY render structural guide lines (solid, dotted, or styled),
including but not limited to:

- vertical ancestor continuation lines
- elbow / junction indicators
- depth guides

### INV-VIS1 — Non-Interactive Decoration

Tree tracing lines:

- MUST NOT participate in hit-testing
- MUST NOT receive hover state
- MUST NOT respond to clicks
- MUST NOT affect selection, focus, or activation
- MUST NOT alter layout or item geometry

They are strictly decorative.

### INV-VIS2 — Geometry Authority

The authoritative interactive regions remain:

- expand/collapse hit zones
- item content rect
- selection rect
- focus rect

Tracing lines MUST be rendered behind or orthogonal to these regions.

### INV-VIS3 — Style Ownership

- Line style (solid, dotted, opacity, color) is a theming concern
- Presence or absence of lines MUST NOT change behavior
- Disabling lines MUST NOT affect any invariant in this SPEC

---

## 18. Final Note

A Tree is a **hierarchical state machine** with expansion as first-class state.

Any implementation that conflates expansion with selection, or allows focus on hidden nodes, is **incorrect by definition**.

---

## Validation Checklist

- [ ] Scope is non-overlapping with other SPECs
- [ ] Ownership boundaries are explicit
- [ ] All cross-SPEC references are valid
- [ ] Forbidden behaviors are explicitly listed
- [ ] Behavior is mechanically testable
