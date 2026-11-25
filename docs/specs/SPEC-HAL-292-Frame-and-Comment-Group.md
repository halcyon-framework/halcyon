# SPEC-HAL-292: **Frame and Comment Group — Visual Containers**

### *Colored Rectangles for Visual Organization of Node Clusters*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define visual container rectangles that group nodes for organizational clarity. These are the "Comment" boxes in Unreal Blueprints, "Frame" nodes in Blender, or group boxes in other node editors. They provide visual context — color-coded regions with titles — without affecting the graph's logical structure.

---

# 2. Projection Model

```rust
/// Disposable projected view of GraphFrame.
pub struct FrameProjection {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub color: Color,
    pub position: WorldPoint,
    pub size: (f64, f64),
    pub collapsed: bool,
    /// Derived from spatial containment during projection build.
    pub contained_nodes: Vec<u64>,
    /// Derived from NodeEditorSession selection.
    pub state: FrameVisualState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameVisualState {
    #[default]
    Default,
    Hovered,
    Selected,
    Resizing,
}
```

**INV-292-A1: Frame Persistence Lives in Graph**
Frame identity, title, description, color, position, size, and collapsed state live in `GraphFrame` (SPEC-293).

**INV-292-A2: FrameProjection Is Not Authority**
`FrameProjection` is derived from `NodeGraphDocument` and `NodeEditorSession`. It may be cached for a frame, but it is disposable and exposes no mutator API.

**INV-292-A3: Containment Is Derived**
`contained_nodes` is derived during projection/transaction planning. It MUST NOT be serialized as independent persistent truth.

---

# 3. Containment Model

**INV-292-CM1: Spatial Containment**
A node is "inside" a frame if its center falls within the frame's world rect. Containment is recalculated when nodes or frames move.

**INV-292-CM2: Move Propagation**
Moving a frame commits one compound graph transaction that moves the frame and all nodes captured by containment at drag start by the same delta.

**INV-292-CM3: No Forced Containment**
Nodes can be moved out of a frame by dragging them beyond the frame's bounds. No snapping or trapping.

**INV-292-CM4: No Nesting**
Frames do not nest inside other frames for v1. A node can only be in one frame at a time (the smallest enclosing frame, spatially).

**INV-292-CM5: Containment Update**
Containment is recalculated after any move operation completes (pointer up), not during drag. This avoids flickering during drag.

**INV-292-CM6: Drag Capture Set**
When a frame drag begins, `NodeEditorSession` records the set of contained node IDs for that drag. The set remains stable until commit or cancel.

---

# 4. Interaction

**INV-292-I1: Create Frame**
Application-triggered: "Create Frame Around Selection" action. The owner commits a graph transaction that creates a frame sized to encompass all selected nodes with padding. Also available via right-click context menu.

**INV-292-I2: Resize Frame**
Drag frame edges/corners to resize. The resize preview lives in `NodeEditorSession`; pointer release commits one resize transaction.

**INV-292-I3: Move Frame**
Drag frame title bar -> preview frame + captured-node movement in session; pointer release commits one compound transaction.

**INV-292-I4: Select Frame**
Click on frame border/title/background hit region (not on a node inside it) -> update `NodeEditorSession.selection`. Shift+click for additive.

**INV-292-I5: Double-Click Title**
Double-click on frame title -> inline rename session. Enter commits a graph transaction; Escape discards the session draft.

**INV-292-I6: Delete Frame**
Delete key on selected frame -> commit a graph transaction that removes the frame. Contained nodes are NOT deleted — only the frame itself.

**INV-292-I7: Collapse**
Collapse a frame -> commit a graph transaction that changes `GraphFrame.collapsed`. Projection hides contained nodes while collapsed. Expand restores them.

---

# 5. Rendering

**INV-292-R1: Z-Order**
Frames render BEHIND nodes and wires. Default z_order = 10 (nodes at 100, wires at 50).

**INV-292-R2: Background**
Semi-transparent fill in the frame's color (15-25% opacity). Border in frame color at full opacity, 1px.

**INV-292-R3: Title Bar**
Title text at the top of the frame, outside or inside the top edge. Uses frame color as a solid background strip behind the title text.

**INV-292-R4: Description**
Optional description text below the title, in `typography.muted` color. Only visible at sufficient zoom.

**INV-292-R5: Selected**
Selected frame: border thickens to 2px, `accents.primary` border color.

**INV-292-R6: Resize Handles**
Small squares at corners and midpoints of edges when frame is selected. Visible only when hovered or selected.

**INV-292-R7: Level of Detail**
At low zoom: frame renders as a colored rectangle with title only. Description hidden. At very low zoom: just a colored rectangle, no text.

---

# 6. Canvas Projection Contribution

```rust
impl FrameProjection {
    pub fn to_projected_item(&self) -> ProjectedCanvasItem {
        ProjectedCanvasItem {
            id: CanvasEntityId::Frame(self.id),
            bounds: self.world_bounds(),
            z_order: 10,
            hit_regions: self.hit_regions(),
            paint: CanvasPaintDescriptor::Frame(self.id),
        }
    }
}
```

**INV-292-HT1: Border-Only Hit Test**
Frames are hit-testable only on their border, title bar, and resize handles. Clicking inside a frame's empty interior hits the canvas (for panning, rubber band selection, etc.), not the frame.

**INV-292-HT2: Projection Does Not Mutate**
Frame hit testing returns `CanvasEntityId::Frame(frame_id)` or resize-handle entity metadata. It does not select, move, resize, collapse, or delete the frame.

---

# 7. Acceptance Criteria

- Create frame around selected nodes with auto-sizing
- Moving frame previews through session state and commits one compound graph transaction
- Resize by dragging edges/corners commits one graph transaction
- Frame renders behind nodes and wires
- Click on frame border/title selects the frame
- Click inside frame (not on a node) passes through to canvas
- Delete frame preserves contained nodes
- Inline rename on double-click title
- Collapse hides contained nodes
- Containment is derived, not serialized as independent frame state

---

# 8. Non-Goals

- Nested frames (future extension)
- Auto-resize to fit contents as nodes move (future — "sticky" frames)
- Frame-level port aggregation (exposing internal ports on the frame border — that's subgraph/group nodes, SPEC-293)
- Logical grouping affecting execution (frames are purely visual)
