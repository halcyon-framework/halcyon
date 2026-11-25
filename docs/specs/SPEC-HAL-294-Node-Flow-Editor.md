# SPEC-HAL-294: **Node Flow View — Top-Level Composition Surface**

### *The Assembled Node Graph Editing Surface*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-286 (Node Widget)
* SPEC-HAL-287 (Port Model)
* SPEC-HAL-288 (Wire Rendering)
* SPEC-HAL-289 (Connection Interaction)
* SPEC-HAL-290 (Node Selection and Multi-Select)
* SPEC-HAL-291 (Node Search / Quick Add)
* SPEC-HAL-292 (Frame and Comment Group)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define the top-level composition surface that assembles all node graph subsystems into a single embeddable node editing view.

`NodeFlowView` owns no graph document and no editor session authority. Applications provide `NodeGraphDocument`, `NodeEditorSession`, `NodeTypeCatalog`, and `PortTypeRegistry`. The view builds a disposable `CanvasProjection`, delegates coordinate behavior to `InfiniteCanvas`, and emits intents. The owning application applies session changes and graph transactions.

The retained Widget adapter is `NodeFlowSurface`. It consumes owner-published snapshots through `NodeFlowSurfaceBinding` and exposes the same `NodeEditorIntent` boundary. `NodeFlowView` / `NodeFlowCoordinator` remains the model-level composition and intent coordinator used by the Widget.

---

# 2. Authority Architecture

```rust
pub struct NodeFlowView {
    id: WidgetInternalId,
    /// Coordinate substrate and projection renderer.
    canvas: InfiniteCanvas,
    /// Configuration.
    config: NodeFlowConfig,
    /// Outbound intents for the owning application.
    intents: Vec<NodeEditorIntent>,
}

pub struct NodeFlowInputs<'a> {
    pub document: &'a NodeGraphDocument,
    pub session: &'a NodeEditorSession,
    pub catalog: &'a NodeTypeCatalog,
    pub port_registry: &'a PortTypeRegistry,
    pub execution_overlay: Option<&'a ExecutionOverlay>,
}

pub struct NodeFlowSurfaceBinding {
    inner: Rc<RefCell<NodeFlowSurfaceMailbox>>,
}

pub struct NodeFlowSurface {
    id: WidgetInternalId,
    binding: NodeFlowSurfaceBinding,
    coordinator: NodeFlowCoordinator,
    interaction_policy: SpatialInteractionPolicy,
    // Pointer capture mechanics only; never session authority.
    gesture: SurfaceGesture,
}
```

```rust
pub struct NodeEditorSession {
    pub viewport: CanvasViewportState,
    pub selection: SelectionSet,
    pub active_tool: NodeEditorTool,
    pub drag: Option<DragSession>,
    pub wire_drag: Option<WireDragSession>,
    pub rubber_band: Option<RubberBandSession>,
    pub search: Option<NodeSearchSession>,
    pub inline_edit: Option<InlineEditSession>,
    pub clipboard: Option<NodeClipboard>,
    pub revision: SessionRevision,
}
```

**INV-294-A1: Document Authority Is External**
`NodeFlowView` MUST NOT own `NodeGraphDocument`, `NodeGraph`, `NodeTypeCatalog`, or `PortTypeRegistry`.

**INV-294-A2: Session Authority Is External**
Selection, viewport, active interaction, search overlay, clipboard, and inline edit draft live in `NodeEditorSession`, not in `NodeFlowView` or `InfiniteCanvas`.

**INV-294-A3: Projection Is Disposable**
Node, port, wire, and frame visuals are rebuilt into `CanvasProjection` from `NodeGraphDocument + NodeEditorSession + catalogs`. The view MUST NOT store `node_visuals`, `wire_visuals`, or `frame_visuals` as authoritative mutable state.

**INV-294-A4: Retained Snapshot Staging Is Not Authority**
`NodeFlowSurfaceBinding::publish` may replace an immutable snapshot containing cloned document/session/catalog/registry inputs and their derived projection so retained paint and input phases do not borrow application state. The snapshot has no write-back path, is replaced wholesale, and MUST NOT be mutated or treated as current authority. The owner applies emitted intents to the real authorities and republishes afterward.

**INV-294-A5: Widget State Is Mechanical Only**
`NodeFlowSurface` may retain pointer button, last pointer position, and gesture classification needed for capture continuity. Selection, viewport, node/frame deltas, rubber-band geometry, wire-drag state, search, clipboard, and inline editing remain in `NodeEditorSession` and arrive through the next published snapshot.

---

# 3. Configuration

```rust
pub struct NodeFlowConfig {
    /// Whether the editor is read-only (view only, no editing).
    pub read_only: bool,
    /// Whether to show the minimap.
    pub show_minimap: bool,
    /// Wire style.
    pub wire_style: WireStyle,
    /// Whether to show arrowheads on wires.
    pub show_arrowheads: bool,
    /// Whether grid snapping is enabled.
    pub snap_to_grid: bool,
    /// Whether dropping a wire on empty space opens node search.
    pub auto_search_on_drop: bool,
    /// Background style.
    pub background: CanvasBackground,
}
```

---

# 4. Builder API

```rust
NodeFlowView::new()
    .with_config(NodeFlowConfig {
        read_only: false,
        show_minimap: true,
        wire_style: WireStyle::Bezier,
        snap_to_grid: true,
        auto_search_on_drop: true,
        ..Default::default()
    })
```

The owning application stores the document/session/catalog/registry and passes them through `NodeFlowInputs` during event, layout, and paint phases.

Retained mounting uses the owner/widget mailbox:

```rust
let config = NodeFlowConfig::default();
let binding = NodeFlowSurfaceBinding::new(config);
binding.publish(NodeFlowInputs {
    document: &document,
    session: &session,
    catalog: &catalog,
    port_registry: &port_registry,
    execution_overlay: None,
})?;
let surface = NodeFlowSurface::new(binding.clone());
```

After routed widget events, the owner drains `binding.take_intents()`, applies each intent through its existing session/document transaction handling, and republishes `NodeFlowInputs`. One binding is paired with one mounted surface. The owner publishes initially and after any authority change that affects projection or interaction state.

---

# 5. Projection Synchronization

**INV-294-MV1: Document + Session Are Sources of Truth**
`NodeGraphDocument` (SPEC-293) is the authoritative persistent graph model. `NodeEditorSession` is the authoritative ephemeral editor model. `CanvasProjection` is derived from both.

**INV-294-MV2: Projection on Change**
When document revision, session revision, catalog revision, or registry revision changes, the projection is rebuilt or incrementally refreshed to identical output. Projection is one-way: authorities -> projection. Projection never mutates authorities.

**INV-294-MV3: Visual-Only State**
Some state is editor/session-only and does NOT live in the graph model:
- Hover state
- Selection state (selection is ephemeral, not serialized)
- Animation progress
- Active drag / wire preview
- Search overlay state
- Inline edit draft

Collapsed state is persistent graph state (`GraphNode.collapsed`, `GraphFrame.collapsed`) and appears in projection only as derived visual output.

**INV-294-MV4: Bulk Sync**
When loading a graph into a new `NodeGraphDocument`, projection is rebuilt from scratch. When revisions change incrementally, caches may update only affected projected records if the result is byte-for-byte equivalent to a fresh rebuild.

**INV-294-MV5: No Visual Backflow**
`NodeProjection`, `WireProjection`, `FrameProjection`, and `CanvasProjection` are not write-back buffers. They expose no setters and cannot be used to mutate graph or session state.

---

# 6. Event Orchestration

`NodeFlowView` translates input events into `NodeEditorIntent` values. It reads the current document/session/projection state but does not mutate document or session authority directly.

```
Event arrives (pointer, key)
├── Build/read CanvasProjection from current inputs
├── If search overlay is open -> emit search/session intents
├── If inline editing -> emit inline-edit/session or graph-transaction intent
├── If dragging wire -> emit wire-drag session intent or connection transaction intent
├── If dragging nodes -> emit drag session intent or move transaction intent
├── If rubber band -> emit rubber-band session intent or selection intent
├── Else -> standard dispatch:
│   ├── Hit test: port? -> emit start-wire-drag intent
│   ├── Hit test: node title? -> emit start-node-drag or inline-rename intent
│   ├── Hit test: node body? -> emit start-node-drag intent
│   ├── Hit test: wire? -> emit selection intent
│   ├── Hit test: frame border? -> emit select/drag/resize-frame intent
│   ├── Hit test: empty space? -> emit rubber-band/pan/search intent
│   └── Keyboard:
│       ├── Delete -> emit delete-selected transaction intent
│       ├── Ctrl+A -> emit select-all session intent
│       ├── Ctrl+D -> emit duplicate-selected transaction intent
│       ├── Ctrl+C/V/X -> emit copy/paste/cut intents
│       ├── Ctrl+Z/Y -> emit undo/redo document intent
│       ├── Tab -> emit open-search session intent
│       ├── F -> emit viewport zoom-to-fit session intent
│       └── Escape -> emit cancel/clear session intent
```

```rust
pub enum NodeEditorIntent {
    Session(SessionIntent),
    Graph(GraphTransaction),
    Undo,
    Redo,
    ContextMenu(NodeContextMenuRequest),
}
```

**INV-294-E1: Intent Boundary**
`NodeFlowView` emits intents. The owner applies them to `NodeEditorSession` or `NodeGraphDocument`.

**INV-294-E2: Read-Only Gate**
When `read_only` is true, `NodeFlowView` MUST NOT emit graph-mutating `GraphTransaction` intents. It may still emit session intents for selection, pan, zoom, hover, and inspection.

**INV-294-E3: Widget Uses Existing Intent Paths**
`NodeFlowSurface` routes keyboard actions through `NodeFlowCoordinator`, viewport mechanics through `InfiniteCanvas`, hit testing through `CanvasProjection`, wire commits through the node-editor connection transaction path, and node movement through `move_drag_transaction`. It MUST NOT apply an emitted session intent or graph transaction itself.

---

# 7. Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Delete / Backspace | Delete selected items |
| Ctrl+A | Select all |
| Ctrl+D | Duplicate selected |
| Ctrl+C | Copy selected |
| Ctrl+V | Paste |
| Ctrl+X | Cut |
| Ctrl+Z | Undo |
| Ctrl+Y / Ctrl+Shift+Z | Redo |
| Tab | Open node search at viewport center |
| Escape | Deselect all / close overlay / cancel drag |
| F | Zoom to fit all items |
| Home / 0 | Reset view (zoom=1, center) |
| +/= | Zoom in |
| - | Zoom out |
| Arrows | Nudge selected items (1 grid unit) |
| Shift+Arrows | Nudge selected items (10 grid units) |
| Space+Drag | Pan canvas |
| Middle-click+Drag | Pan canvas |
| Ctrl+Scroll | Zoom |

## 7.1 SPEC-HAL-343 Spatial Interaction Policy

Node flow spatial surfaces consume `SpatialInteractionPolicy` for domain-neutral
gesture classification. The binding table is:

| Policy entry | Node flow value | Authority |
|---|---|---|
| `pan_gesture` | `SpacePrimaryDrag` | Viewport mutation remains `NodeEditorSession.viewport` / `InfiniteCanvas` compatibility authority. |
| `alternate_pan_gesture` | `MiddleDrag` | Same pan authority; no document mutation. |
| `wheel_zoom` | `ControlOrMeta` | Wheel zoom requires Ctrl/Cmd and resolves through viewport/session intent. |
| `rubber_band_gesture` | `ShiftPrimaryDrag` | Selection state remains `NodeEditorSession`. |
| `selection_toggle_requires_command` | `true` | Cmd/Ctrl click toggles through session selection intents. |
| `route_creation_gesture` | `PrimaryDrag` on an anchor | Wire creation remains `NodeEditorSession` until a graph transaction is emitted by the owner. |
| `delete_keys_enabled` | `true` | Delete/Backspace emit graph intents; the spatial layer never deletes nodes, ports, frames, or wires. |
| `fit_key` / `reset_key` | `F` / `Home` | Viewport commands emit session intents. |

---

# 8. Read-Only Mode

**INV-294-RO1: View Only**
When `config.read_only == true`:
- All document-mutating interactions are disabled (no drag-to-connect commit, no move commit, no delete, no add)
- Selection still works through `NodeEditorSession` (for inspection)
- Pan and zoom still work through `NodeEditorSession.viewport`
- Context menus show only "view" options
- Node search is disabled
- Undo/redo graph intents are disabled

Useful for documentation, code review, debugging visualization.

---

# 9. Execution Visualization

**INV-294-EV1: Active Node Highlighting**
The application can provide execution overlay state:

```rust
pub struct ExecutionOverlay {
    pub node_states: HashMap<u64, NodeVisualState>,
    pub wire_states: HashMap<u64, WireVisualState>,
}
```

This affects projection output without modifying the graph document or editor session.

**INV-294-EV2: Wire Flow Animation (Future)**
Future: animated dots on wires showing data flow during execution. Uses the `time_seconds` shader uniform. Deferred.

---

# 10. Public API

```rust
impl NodeFlowView {
    // === Projection ===
    pub fn build_projection(&self, inputs: NodeFlowInputs<'_>) -> CanvasProjection;

    // === Events ===
    pub fn handle_event(&mut self, inputs: NodeFlowInputs<'_>, event: &Event);
    pub fn take_intents(&mut self) -> Vec<NodeEditorIntent>;

    // === View Helpers ===
    pub fn zoom_to_fit_intent(&self, projection: &CanvasProjection) -> SessionIntent;
    pub fn zoom_to_node_intent(&self, projection: &CanvasProjection, node_id: u64) -> Option<SessionIntent>;
    pub fn pan_to_intent(&self, pos: WorldPoint) -> SessionIntent;
    pub fn reset_view_intent(&self) -> SessionIntent;
}

impl NodeFlowSurfaceBinding {
    pub fn new(config: NodeFlowConfig) -> Self;
    pub fn publish(
        &self,
        inputs: NodeFlowInputs<'_>,
    ) -> Result<ProjectionRevision, NodeFlowBuildError>;
    pub fn take_intents(&self) -> Vec<NodeEditorIntent>;
    pub fn has_snapshot(&self) -> bool;
}

impl NodeFlowSurface {
    pub fn new(binding: NodeFlowSurfaceBinding) -> Self;
    pub fn binding(&self) -> NodeFlowSurfaceBinding;
}
```

**INV-294-API1: No Graph Ownership API**
`NodeFlowView` MUST NOT expose `graph()`, `graph_mut()`, `set_graph()`, `with_graph()`, or equivalent graph ownership/mutation APIs.

**INV-294-API2: No External Catalog Mutation API**
Catalog and registry updates happen by changing the external inputs supplied by the owner. `NodeFlowView` does not store or mutate them.

**INV-294-API3: Capture-Mode Keyboard Authority**
`NodeFlowSurface` is focusable and capture-eligible. It handles graph/session-changing keyboard commands only while `EventContext.keyboard_mode == Capture`; pointer selection, pointer gestures, and viewport scroll follow their ordinary routed input authority.

---

# 11. Acceptance Criteria

- Empty editor renders with grid background and minimap
- Adding nodes from search overlay places them on canvas
- Dragging from port to port creates connections with bezier wires
- Multi-select with rubber band, move as group
- Delete removes selected nodes and their connections, undoable
- Ctrl+Z/Y undoes/redoes all operations
- Read-only mode prevents all editing but allows pan/zoom/select
- Supplying a new `NodeGraphDocument` displays that graph through projection
- `NodeGraphDocument::to_serializable()` round-trips correctly
- Execution visualization supplied through `ExecutionOverlay` highlights nodes/wires
- All keyboard shortcuts from §7 work
- Zoom-to-fit frames all nodes
- `NodeFlowView` owns no graph document, editor session, catalog, registry, or mutable visual maps
- All graph changes are emitted as `GraphTransaction` intents and applied by the owner
- `NodeFlowSurface` mounts through `Widget`, stages only immutable owner snapshots, and exposes emitted intents through `NodeFlowSurfaceBinding`

---

# 12. Non-Goals

- Graph execution engine (application runs the graph)
- Live value display on wires (future extension)
- Custom node body widgets (v1 uses paint callbacks, not child widgets)
- Multi-graph tabs (application composes multiple editors)
- Collaborative real-time editing (future)
- Plugin system for custom interactions (future)
