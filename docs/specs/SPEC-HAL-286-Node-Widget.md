# SPEC-HAL-286: **Node Widget — Visual Processing Block**

### *The Rectangular Block Element in a Node Graph*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-287 (Port Model)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define the visual representation and interaction model for a node — the rectangular box with a title bar, typed input/output ports, and optional body content that is the fundamental element in any visual programming or flow-based interface.

---

# 2. Projection Model

Node type definitions are owned by `NodeTypeCatalog` (SPEC-293). This spec defines the projected visual shape of a graph node.

```rust
/// Disposable projected view of a graph node for paint and hit testing.
pub struct NodeProjection {
    pub id: u64,
    pub type_id: u64,
    /// Derived from GraphNode.position plus any active session drag delta.
    pub position: WorldPoint,
    pub title: String,
    pub color: Color,
    pub collapsed: bool,
    /// Derived from NodeEditorSession and application execution overlays.
    pub state: NodeVisualState,
    /// Port projections derived from NodeType, NodeGraphDocument, and layout.
    pub inputs: Vec<PortProjection>,
    pub outputs: Vec<PortProjection>,
    /// Optional body content height (world units). 0.0 if no body.
    pub body_height: f64,
    /// Cached layout in the projection. Rebuildable, never authoritative.
    cached_size: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeVisualState {
    #[default]
    Default,
    Hovered,
    Selected,
    Error,
    Running,
    Completed,
}
```

**INV-286-A1: Node Projection Is Not Authority**
`NodeProjection` is derived from `NodeGraphDocument`, `NodeTypeCatalog`, and `NodeEditorSession`. It MUST NOT be stored as the graph source of truth.

**INV-286-A2: Persistent Node State Lives in the Graph**
Node identity, type, position, title, collapsed state, and parameter values live in `GraphNode` (SPEC-293).

**INV-286-A3: Ephemeral Editor State Lives in Session**
Selection, hover, active drag delta, inline rename draft, and execution highlight overlay are session/application state. They do not live in `GraphNode` and do not live in the canvas.

---

# 3. Node Layout

Nodes have a fixed structure:

```
┌──────────────────────────┐
│  ● Title Bar (drag handle) │  ← category color, title text, collapse toggle
├──────────────────────────┤
│ ○ input_1    output_1 ● │  ← ports with labels
│ ○ input_2    output_2 ● │
│ ○ input_3              │
├──────────────────────────┤
│  [ Body Content ]        │  ← optional: parameters, preview, custom content
└──────────────────────────┘
```

### Invariants

**INV-286-L1: Fixed Width, Variable Height**
Node width is fixed per type (configurable, default 180 world units). Height is computed from: title bar height + port rows + body height.

**INV-286-L2: Title Bar**
Always visible. Shows node title text, category color as a left-edge stripe or background tint, and collapse toggle icon (if `collapsible`).

**INV-286-L3: Port Layout**
Input ports align left. Output ports align right. Ports are laid out top-to-bottom in definition order. Port row height is fixed from theme.

**INV-286-L4: Collapsed Mode**
When collapsed, only the title bar and ports are visible. Body content is hidden. Total height = title_bar_height + max(input_count, output_count) × port_row_height.

**INV-286-L5: Size Caching**
Node size may be cached inside the projection builder. The cache is valid only for the source document/session/catalog revisions used to build it and may be discarded at any frame boundary.

---

# 4. Node Theme Geometry

```rust
pub struct NodeGeometry {
    pub default_width: f64,
    pub title_bar_height: f64,
    pub port_row_height: f64,
    pub port_radius: f64,
    pub port_hit_radius: f64,
    pub body_padding: f64,
    pub corner_radius: f64,
    pub border_width: f64,
    pub category_stripe_width: f64,
    pub title_font_size: f32,
    pub port_font_size: f32,
    pub min_body_height: f64,
}
```

---

# 5. Rendering

**INV-286-R1: Background**
Node background: `push_rect` with node color at slight opacity (0.95). Border: `push_border_rect`. Per SPEC-PM-006.

**INV-286-R2: Title Bar**
Category stripe (thin colored rect on the left edge) + title text. Title text truncated with ellipsis if too wide.

**INV-286-R3: Ports**
Input ports: small circle on the left edge. Output ports: small circle on the right edge. Filled circle = connected, hollow = disconnected. Port color from port type.

**INV-286-R4: Port Labels**
Input labels right of the port circle. Output labels left of the port circle (right-aligned).

**INV-286-R5: Visual States**
- Default: standard background + border
- Hovered: lightened background (lerp 0.05 toward white)
- Selected: `accents.primary` border, 2px
- Error: `accents.error` border, pulsing
- Running: animated border (e.g., spinning dash pattern or glow)
- Completed: subtle green checkmark in title bar

**INV-286-R6: Level of Detail**
At low zoom (< 0.3): render as a solid colored rectangle with no text, no ports, just the title bar stripe. At medium zoom (0.3-0.7): render title + ports, no body. At high zoom (> 0.7): full render.

**INV-286-R7: Shadow**
Optional drop shadow behind the node for depth. Uses `push_shadow` with `ShadowMode::Drop`.

---

# 6. Interaction

**INV-286-I1: Drag to Move**
Left-click on title bar or body emits a node-drag intent. During drag, `NodeEditorSession` stores the provisional delta for preview. On pointer release, the owner commits one graph transaction that moves all selected nodes.

**INV-286-I2: Click to Select**
Left-click on node emits a selection intent. `NodeEditorSession` updates the selection set. The node projection only reflects the resulting selected visual state.

**INV-286-I3: Double-Click Title**
Double-click on title bar opens an inline rename session. Escape discards the session draft. Enter commits a graph transaction that changes the node title.

**INV-286-I4: Collapse Toggle**
Click collapse icon emits a collapse-toggle intent. The owner commits a graph transaction that changes `GraphNode.collapsed`.

**INV-286-I5: Right-Click**
Right-click on node emits a context-menu intent with the node ID.

**INV-286-I6: Port Interaction**
Port clicks and drags are handled by the Connection Interaction protocol (SPEC-289). The node delegates port events upward.

---

# 7. Canvas Projection Contribution

`NodeProjection` contributes a `ProjectedCanvasItem` (SPEC-285):

```rust
impl NodeProjection {
    pub fn to_projected_item(&self) -> ProjectedCanvasItem {
        ProjectedCanvasItem {
            id: CanvasEntityId::Node(self.id),
            bounds: self.world_bounds(),
            z_order: 100 + selected_z_lift(self.state),
            hit_regions: self.hit_regions(),
            paint: CanvasPaintDescriptor::Node(self.id),
        }
    }
}
```

**INV-286-P1: No Node Mutators in Projection**
The projection contribution MUST NOT expose movement, selection, deletion, duplication, title mutation, collapse mutation, or parameter mutation APIs.

**INV-286-P2: Port Hit Regions Beat Node Body**
Port hit regions use `HitPriority::Port`; node title/body hit regions use `HitPriority::Node`.

---

# 8. Body Content

**INV-286-B1: Application-Defined**
Node body content is opaque to the framework. The framework allocates the body rect; the application paints whatever it wants inside from immutable graph/projection inputs:

```rust
pub type NodeBodyPainter =
    Box<dyn Fn(&mut CanvasPaintContext, WorldRect, &GraphNode, &NodeProjection)>;
```

**INV-286-B2: No Child Widgets**
Node bodies do NOT contain child Halcyon widgets. They are painted directly. This avoids the complexity of nesting the widget tree inside canvas items. Interactive body content (sliders, text fields for parameters) is a future extension.

---

# 9. Acceptance Criteria

- Nodes render with title bar, ports, optional body
- Drag on title bar previews through session state and commits one graph transaction
- Node visual states (default, hovered, selected, error, running)
- Collapsed mode hides body
- Port circles show connected/disconnected state
- Level-of-detail at low zoom
- Multi-select move works
- Context menu on right-click
- Inline title rename on double-click
- Node projection exposes no mutator API

---

# 10. Non-Goals

- Interactive body content (widget-in-node — future extension)
- Custom node shapes (all nodes are rectangles for v1)
- Node resizing by drag (future extension)
- Node grouping into sub-graphs (SPEC-293 covers this)
