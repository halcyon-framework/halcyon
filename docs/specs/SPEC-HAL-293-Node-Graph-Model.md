# SPEC-HAL-293: **Node Graph Model — Data Structure and Serialization**

### *Pure Data Model for Node Graphs with Undo/Redo and Validation*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-287 (Port Model)

---

# 1. Purpose

Define the document authority and pure data model for a node graph: nodes, connections, frames, metadata, transactions, undo/redo, validation, and serialization. This model is independent of rendering — it can exist without a canvas, be serialized/deserialized, validated, and manipulated programmatically. The visual layer (SPEC-294) projects this model onto the canvas.

`NodeGraphDocument` is the authoritative owner of graph mutations. `NodeGraph` is serializable graph data. Canvas, projection, and view widgets never mutate graph data directly.

---

# 2. Graph Data Structure

```rust
pub struct NodeGraphDocument {
    graph: NodeGraph,
    revision: GraphRevision,
    next_id: u64,
    undo_stack: Vec<GraphDelta>,
    redo_stack: Vec<GraphDelta>,
    max_undo_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphRevision(pub u64);

pub struct NodeGraph {
    pub id: u64,
    pub name: String,
    /// All nodes in the graph.
    nodes: HashMap<u64, GraphNode>,
    /// All connections.
    connections: HashMap<u64, GraphConnection>,
    /// All frames.
    frames: HashMap<u64, GraphFrame>,
    /// Metadata.
    pub metadata: GraphMetadata,
}

pub struct GraphNode {
    pub id: u64,
    pub type_id: u64,
    pub position: (f64, f64),
    pub title: String,
    pub collapsed: bool,
    /// Per-node parameter values (opaque to the framework).
    pub parameters: HashMap<String, ParameterValue>,
}

pub struct GraphConnection {
    pub id: u64,
    pub from_node: u64,
    pub from_port: u32,
    pub to_node: u64,
    pub to_port: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphEntityId {
    Node(u64),
    Connection(u64),
    Frame(u64),
}

pub struct GraphFrame {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub color: (u8, u8, u8),
    pub position: (f64, f64),
    pub size: (f64, f64),
    pub collapsed: bool,
}

pub struct GraphMetadata {
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub version: u32,
    pub custom: HashMap<String, String>,
}
```

### Node Type Catalog

```rust
pub struct NodeTypeCatalog {
    types: HashMap<u64, NodeType>,
    revision: CatalogRevision,
}

#[derive(Debug, Clone)]
pub struct NodeType {
    pub type_id: u64,
    pub name: String,
    pub category: String,
    pub default_title: String,
    pub default_color: Color,
    pub input_ports: Vec<PortDefinition>,
    pub output_ports: Vec<PortDefinition>,
    pub resizable: bool,
    pub collapsible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogRevision(pub u64);
```

**INV-293-CAT1: Catalog Is Application Authority**
`NodeTypeCatalog` is supplied by the application/document layer. The graph document reads it for validation and node defaults; canvas, projection, and view widgets do not mutate it.

**INV-293-CAT2: Catalog Defines Ports**
Port existence, direction, type, and cardinality come from the catalog's `PortDefinition` lists, not from projected port records.

**INV-293-A1: Document Is Mutation Authority**
All graph mutation goes through `NodeGraphDocument`. No widget, canvas, projection, or session object may hold `&mut NodeGraph`.

**INV-293-A2: Graph Is Pure Serializable Data**
`NodeGraph` contains persistent graph data only. It does not contain selection, viewport transform, drag state, search state, hover state, or execution visualization state.

**INV-293-A3: Revision Bumps on Commit**
Every successful graph transaction increments `GraphRevision` exactly once. Failed validation does not change revision.

### Parameter Values

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(u8, u8, u8, u8),
    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Enum(u32),
    Custom(Vec<u8>),  // opaque application data
}
```

---

# 3. Graph Transactions

All mutations go through `GraphTransaction` values applied to `NodeGraphDocument`. Successful transactions produce `GraphDelta` entries for undo/redo.

```rust
#[derive(Debug, Clone)]
pub enum GraphTransaction {
    AddNode { type_id: u64, position: (f64, f64), title: Option<String> },
    RemoveNode { node_id: u64 },
    RemoveNodes { node_ids: Vec<u64> },
    MoveNodes { moves: Vec<(u64, (f64, f64))> },
    SetNodeTitle { node_id: u64, title: String },
    SetNodeCollapsed { node_id: u64, collapsed: bool },
    SetParameter { node_id: u64, key: String, value: ParameterValue },
    AddConnection { from_node: u64, from_port: u32, to_node: u64, to_port: u32 },
    RemoveConnection { connection_id: u64 },
    ReplaceConnection { old_connection_id: u64, from_node: u64, from_port: u32, to_node: u64, to_port: u32 },
    AddFrame { title: String, position: (f64, f64), size: (f64, f64), color: (u8, u8, u8) },
    RemoveFrame { frame_id: u64 },
    MoveFrame { frame_id: u64, new_pos: (f64, f64) },
    MoveFrameWithNodes { frame_id: u64, new_pos: (f64, f64), node_moves: Vec<(u64, (f64, f64))> },
    ResizeFrame { frame_id: u64, new_size: (f64, f64) },
    SetFrameCollapsed { frame_id: u64, collapsed: bool },
    DuplicateSelection { entities: Vec<GraphEntityId>, offset: (f64, f64) },
    Compound(Vec<GraphTransaction>),
}

pub struct GraphTransactionContext<'a> {
    pub catalog: &'a NodeTypeCatalog,
    pub port_registry: &'a PortTypeRegistry,
    pub validator: Option<&'a dyn GraphValidator>,
}

pub trait GraphValidator {
    fn validate_transaction(
        &self,
        graph: &NodeGraph,
        transaction: &GraphTransaction,
        catalog: &NodeTypeCatalog,
        registry: &PortTypeRegistry,
    ) -> Result<(), GraphError>;
}

pub struct GraphCommit {
    pub revision: GraphRevision,
    pub delta: GraphDelta,
}

impl NodeGraphDocument {
    pub fn graph(&self) -> &NodeGraph;
    pub fn revision(&self) -> GraphRevision;
    pub fn apply(
        &mut self,
        transaction: GraphTransaction,
        context: GraphTransactionContext<'_>,
    ) -> Result<GraphCommit, GraphError>;

    // === Query ===
    pub fn node(&self, id: u64) -> Option<&GraphNode>;
    pub fn connection(&self, id: u64) -> Option<&GraphConnection>;
    pub fn connections_for_node(&self, node_id: u64) -> Vec<&GraphConnection>;
    pub fn connections_for_port(&self, node_id: u64, port_id: u32) -> Vec<&GraphConnection>;
    pub fn nodes(&self) -> impl Iterator<Item = &GraphNode>;
    pub fn connections(&self) -> impl Iterator<Item = &GraphConnection>;
    pub fn frames(&self) -> impl Iterator<Item = &GraphFrame>;

    // === Undo/Redo ===
    pub fn undo(&mut self) -> bool;
    pub fn redo(&mut self) -> bool;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;

    // === Validation ===
    pub fn validate(&self, catalog: &NodeTypeCatalog, registry: &PortTypeRegistry) -> Vec<GraphError>;
    pub fn topological_sort(&self) -> Result<Vec<u64>, CycleError>;
    pub fn has_cycles(&self) -> bool;
}
```

**INV-293-T1: Transactions Are Atomic**
A transaction either validates and commits completely or changes nothing.

**INV-293-T2: Compound Means One Commit**
`GraphTransaction::Compound` validates as a single unit, commits as a single unit, bumps revision once, and creates one undo step.

**INV-293-T3: No Raw Mutable Graph Access**
Public API exposes immutable graph queries and transaction application only. `graph_mut()` or equivalent direct mutable access is forbidden.

**INV-293-T4: Policy Lives in Validator**
Domain policy such as cycle allowance, execution constraints, and application-specific connection rules belongs in `GraphValidator` or in explicit transaction context. It is not node view configuration.

---

# 4. Undo/Redo

### Delta Model

```rust
#[derive(Debug, Clone)]
pub enum GraphDelta {
    AddNode { node: GraphNode },
    RemoveNode { node: GraphNode, connections: Vec<GraphConnection> },
    MoveNode { id: u64, old_pos: (f64, f64), new_pos: (f64, f64) },
    MoveNodes { moves: Vec<(u64, (f64, f64), (f64, f64))> },
    SetNodeTitle { id: u64, old_title: String, new_title: String },
    SetNodeCollapsed { id: u64, old: bool, new: bool },
    SetParameter { node_id: u64, key: String, old: Option<ParameterValue>, new: ParameterValue },
    AddConnection { connection: GraphConnection },
    RemoveConnection { connection: GraphConnection },
    AddFrame { frame: GraphFrame },
    RemoveFrame { frame: GraphFrame },
    MoveFrame { id: u64, old_pos: (f64, f64), new_pos: (f64, f64) },
    MoveFrameWithNodes { frame_id: u64, frame_old_pos: (f64, f64), frame_new_pos: (f64, f64), node_moves: Vec<(u64, (f64, f64), (f64, f64))> },
    ResizeFrame { id: u64, old_size: (f64, f64), new_size: (f64, f64) },
    SetFrameCollapsed { id: u64, old: bool, new: bool },
    /// Compound delta: multiple operations as one undo step.
    Compound(Vec<GraphDelta>),
}
```

### Invariants

**INV-293-U1: Every Mutation Produces a Delta**
Every successful `NodeGraphDocument::apply()` call pushes one `GraphDelta` to the undo stack and clears the redo stack.

**INV-293-U2: Compound Operations**
Bulk operations (duplicate, remove multiple nodes, move frame with contained nodes, create-and-connect from search) produce a single `Compound` delta so they undo as one step.

**INV-293-U3: Cascading Deltas**
Removing a node also removes all its connections. The delta captures both the node AND the connections, so undo restores everything.

**INV-293-U4: Move Coalescing**
A drag gesture produces one `MoveNodes`, `MoveFrame`, or `MoveFrameWithNodes` transaction at commit. Per-frame preview movement lives in `NodeEditorSession`, not in `NodeGraphDocument`. There is no delta per pointer-move event.

**INV-293-U5: Undo Stack Limit**
Configurable maximum undo depth (default 100). Oldest deltas are dropped when the limit is exceeded.

**INV-293-U6: Undo/Redo Bump Revision**
Successful undo and redo operations increment `GraphRevision` exactly once because projection output may change.

---

# 5. Serialization

**INV-293-S1: Serde-Compatible**
`GraphNode`, `GraphConnection`, `GraphFrame`, `GraphMetadata`, and `ParameterValue` all derive `serde::Serialize` and `serde::Deserialize`.

**INV-293-S2: Format-Agnostic**
The framework provides the serde derives. The application chooses the format (JSON, MessagePack, CBOR, etc.).

**INV-293-S2A: Document Runtime State Is Not Serialized**
Undo stack, redo stack, `GraphRevision`, next ID allocator internals, selection, viewport, search, hover, drag, and execution visualization state are not serialized as graph content. Loading a graph creates a fresh document runtime around serialized graph data.

**INV-293-S3: Export/Import**
```rust
impl NodeGraphDocument {
    pub fn to_serializable(&self) -> SerializableGraph;
    pub fn from_serializable(data: SerializableGraph) -> Result<Self, GraphError>;
}
```

**INV-293-S4: Version Field**
`GraphMetadata.version` allows forward-compatible schema evolution. Applications can migrate old graph formats.

---

# 6. Validation

```rust
#[derive(Debug)]
pub enum GraphError {
    DanglingConnection { connection_id: u64, missing_node: u64 },
    UnknownPort { node_id: u64, port_id: u32 },
    DirectionMismatch { connection_id: u64 },
    TypeMismatch { connection_id: u64, output_type: PortTypeId, input_type: PortTypeId },
    CycleDetected { involved_nodes: Vec<u64> },
    DuplicateConnection { from: (u64, u32), to: (u64, u32) },
    UnknownNodeType { node_id: u64, type_id: u64 },
    CardinalityViolation { node_id: u64, port_id: u32, max: u32, actual: u32 },
}
```

**INV-293-V1: Validate on Demand**
Validation is explicit. `NodeGraphDocument::apply()` validates transactions before commit; the application may also call `document.validate()` when needed (e.g., before execution, after load).

**INV-293-V2: Topological Sort**
`topological_sort()` returns nodes in execution order for DAGs. Returns `CycleError` if the graph has cycles.

**INV-293-V3: Catalog-Aware Validation**
Validation requires `NodeTypeCatalog` and `PortTypeRegistry`. Unknown node types, unknown ports, direction mismatches, type mismatches, duplicate connections, dangling connections, and cardinality violations are all detected from authoritative graph/catalog/registry state.

**INV-293-V4: Projection Data Is Ignored**
Validation MUST NOT read `NodeProjection`, `PortProjection`, `WireProjection`, `CanvasProjection`, or cached connection counts.

---

# 7. Subgraph Extraction (Future Extension)

**INV-293-SG1: Extract Subgraph**
Select a set of nodes → extract as a reusable subgraph. Internal connections are preserved. External connections become exposed ports on the subgraph's boundary.

This is deferred to a future SPEC. The data model supports it (a subgraph is a `NodeGraph` with additional "boundary port" metadata), but the interaction and rendering are complex.

---

# 8. Acceptance Criteria

- Add/remove/move nodes, connections, and frames through `GraphTransaction`
- Undo reverses the last operation exactly
- Redo re-applies after undo
- Compound operations (duplicate, bulk delete) undo as one step
- Removing a node removes all its connections (and undo restores all)
- Drag commit produces one undo step; per-frame preview movement is not graph state
- Serialization round-trips: `to_serializable → serialize → deserialize → from_serializable` produces equivalent graph
- `validate(catalog, registry)` detects dangling connections, unknown node types, unknown ports, direction mismatches, type mismatches, duplicate connections, cardinality violations, and cycles
- `topological_sort()` returns correct execution order for DAGs
- `has_cycles()` detects cycles
- No public API exposes `&mut NodeGraph`
- Every successful graph mutation bumps `GraphRevision` once

---

# 9. Non-Goals

- Execution engine (running the graph — application layer)
- Live data flow (passing values through connections — application layer)
- Graph diffing (comparing two versions of a graph — future)
- Collaborative editing (multi-user concurrent edits — future)
- Subgraph composition (future extension, §7)
