# SPEC-HAL-290: **Node Selection and Multi-Select**

### *Single, Multi, and Rubber-Band Selection for Projected Node-Editor Entities*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define the selection model and bulk operations for projected node-editor entities on the infinite canvas. Selection is the gateway to manipulation — move, delete, duplicate, copy/paste, align, group.

Selection storage belongs to `NodeEditorSession`, not the canvas and not `CanvasProjection`.

Selection-relevant session fields:

```rust
pub struct NodeEditorSession {
    pub selection: SelectionSet,
    pub drag: Option<DragSession>,
    pub clipboard: Option<NodeClipboard>,
    pub revision: SessionRevision,
    // Other session fields are defined by SPEC-HAL-294.
}

pub struct SelectionSet {
    pub entities: Vec<CanvasEntityId>,
    pub anchor: Option<CanvasEntityId>,
}
```

**INV-290-A1: Session Owns Selection**
Selection is editor/session authority. The graph does not serialize it; the canvas does not store it; projected items only reflect it as derived visual state.

---

# 2. Selection Modes

**INV-290-S1: Single Select**
Left-click on a selectable projected entity -> replace `NodeEditorSession.selection` with that entity.

**INV-290-S2: Additive Select**
Shift+click on a selectable projected entity -> toggle it in `NodeEditorSession.selection` without affecting others.

**INV-290-S3: Ctrl+Click**
Same as Shift+click (platform-standard alternative).

**INV-290-S4: Rubber Band**
Left-click drag on empty canvas → draw selection rectangle. On release:
- Without Shift: replace selection with items intersecting the rectangle
- With Shift: add intersecting items to existing selection

**INV-290-S5: Select All**
Ctrl+A -> select all selectable projected entities in the current projection.

**INV-290-S6: Deselect All**
Click on empty canvas (no drag) → deselect all. Escape while no drag active → deselect all.

---

# 3. Rubber Band Rendering

**INV-290-RB1: Visual**
Selection rectangle renders as a semi-transparent `accents.primary` fill (15% opacity) with a 1px `accents.primary` border. Renders above all items (z = max).

**INV-290-RB2: Coordinates**
The rubber band operates in screen space but the intersection test uses world-space bounds. The rectangle is defined by the drag start point and the current cursor position, both converted to world coordinates.

**INV-290-RB3: Live Preview**
During drag, items that would be selected highlight immediately (before release). This gives the user feedback about what will be selected.

---

# 4. Bulk Operations

### Move

**INV-290-M1: Drag Selected**
Drag any selected movable entity -> `NodeEditorSession.drag` stores the provisional world-space delta. Projection applies the provisional delta for preview. Pointer release commits one graph transaction.

**INV-290-M2: Grid Snap**
If grid snapping is enabled, the owner applies snap to the primary drag target's committed transaction. All other selected nodes maintain their relative offsets.

**INV-290-M3: Arrow Key Nudge**
Arrow keys (in capture mode) -> commit one graph transaction that nudges all selected movable graph entities by 1 grid unit (or 1 pixel if no grid). Shift+Arrow -> nudge by 10 units.

**INV-290-M4: One Undo Step Per Gesture**
A pointer drag or keyboard nudge produces one graph transaction and therefore one undo step. Per-frame preview movement is session state only.

### Delete

**INV-290-D1: Delete Key**
Delete or Backspace -> commit one graph transaction that removes all selected deletable graph entities. Wires connected to deleted nodes are also removed.

**INV-290-D2: Cascade Delete**
When a node is deleted, all wires connected to any of its ports are automatically removed.

### Duplicate

**INV-290-DUP1: Ctrl+D**
Duplicate all selected graph entities in one graph transaction. New nodes/frames are placed at a world-space offset (e.g., +20, +20 from originals). Internal connections between duplicated nodes are preserved. Connections to non-duplicated nodes are NOT copied.

**INV-290-DUP2: Selection Transfer**
After duplication, the selection moves to the new copies (originals are deselected).

### Copy/Paste

**INV-290-CP1: Ctrl+C**
Copy selected graph entities to `NodeEditorSession.clipboard` (not OS clipboard — the graph structure doesn't serialize to plain text naturally).

**INV-290-CP2: Ctrl+V**
Paste from session clipboard at the cursor position (or viewport center if cursor outside). Paste commits one graph transaction. Same duplication rules as Ctrl+D.

**INV-290-CP3: Ctrl+X**
Cut = copy + delete.

### Alignment (Future Extension)

**INV-290-A1: Align Left/Right/Top/Bottom/Center**
Future: align selected items to a shared edge or center. Not required for v1.

---

# 5. Intent and Transaction Boundary

```rust
pub enum SelectionIntent {
    Replace(CanvasEntityId),
    Toggle(CanvasEntityId),
    SelectAll,
    Clear,
    RubberBandCommit { rect: WorldRect, additive: bool },
    BeginMove { anchor: WorldPoint, primary: CanvasEntityId },
    UpdateMove { delta: (f64, f64) },
    CommitMove,
    DeleteSelected,
    DuplicateSelected,
    CopySelected,
    PasteClipboard { position: WorldPoint },
    CutSelected,
}
```

**INV-290-I1: Selection Intents Mutate Session First**
Selection changes mutate `NodeEditorSession` and bump `SessionRevision`.

**INV-290-I2: Graph Changes Are Transactions**
Move, delete, duplicate, paste, cut, and nudge operations mutate `NodeGraphDocument` only through graph transactions.

**INV-290-I3: Projection IDs Are Translated at Boundary**
`CanvasEntityId` values in selection are translated by the owner into graph-native `GraphEntityId` values before constructing a `GraphTransaction`. `NodeGraphDocument` never consumes canvas projection IDs.

---

# 6. Acceptance Criteria

- Single click selects one item, deselects others
- Shift+click toggles selection additively
- Rubber band selects all intersecting items
- Rubber band shows live preview during drag
- Drag selected items previews through session state and commits one graph transaction
- Delete removes selected items and their connected wires
- Ctrl+D duplicates with offset, preserving internal connections
- Ctrl+A selects all
- Escape deselects all
- Arrow keys nudge selected items
- Selection state is absent from graph, canvas, and projection storage

---

# 7. Non-Goals

- Lasso selection (freeform polygon instead of rectangle — future)
- Alignment tools (future extension)
- Grouping into visual containers (SPEC-292 handles frames)
