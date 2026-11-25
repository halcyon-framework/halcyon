use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::core::paint::Color;

pub const DEFAULT_MAX_UNDO_DEPTH: usize = 100;

#[derive(Debug, Clone)]
pub struct NodeGraphDocument {
    graph: NodeGraph,
    revision: GraphRevision,
    next_id: u64,
    undo_stack: Vec<GraphDelta>,
    redo_stack: Vec<GraphDelta>,
    max_undo_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct GraphRevision(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeGraph {
    pub id: u64,
    pub name: String,
    nodes: HashMap<u64, GraphNode>,
    connections: HashMap<u64, GraphConnection>,
    frames: HashMap<u64, GraphFrame>,
    pub metadata: GraphMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: u64,
    pub type_id: u64,
    pub position: (f64, f64),
    pub title: String,
    pub collapsed: bool,
    pub parameters: HashMap<String, ParameterValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphFrame {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub color: (u8, u8, u8),
    pub position: (f64, f64),
    pub size: (f64, f64),
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub version: u32,
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(u8, u8, u8, u8),
    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Enum(u32),
    Custom(Vec<u8>),
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CatalogRevision(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDefinition {
    pub port_id: u32,
    pub label: String,
    pub direction: PortDirection,
    pub type_id: PortTypeId,
    pub max_connections: PortCardinality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCardinality {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortTypeId(pub u64);

impl PortTypeId {
    pub const ANY: Self = Self(0);
}

#[derive(Debug, Clone)]
pub struct PortTypeInfo {
    pub type_id: PortTypeId,
    pub name: String,
    pub color: Color,
    pub compatible_with: Vec<PortTypeId>,
}

#[derive(Debug, Clone, Default)]
pub struct PortTypeRegistry {
    types: HashMap<PortTypeId, PortTypeInfo>,
}

#[derive(Debug, Clone)]
pub enum GraphTransaction {
    AddNode {
        type_id: u64,
        position: (f64, f64),
        title: Option<String>,
    },
    RemoveNode {
        node_id: u64,
    },
    RemoveNodes {
        node_ids: Vec<u64>,
    },
    MoveNodes {
        moves: Vec<(u64, (f64, f64))>,
    },
    SetNodeTitle {
        node_id: u64,
        title: String,
    },
    SetNodeCollapsed {
        node_id: u64,
        collapsed: bool,
    },
    SetParameter {
        node_id: u64,
        key: String,
        value: ParameterValue,
    },
    AddConnection {
        from_node: u64,
        from_port: u32,
        to_node: u64,
        to_port: u32,
    },
    RemoveConnection {
        connection_id: u64,
    },
    ReplaceConnection {
        old_connection_id: u64,
        from_node: u64,
        from_port: u32,
        to_node: u64,
        to_port: u32,
    },
    AddFrame {
        title: String,
        position: (f64, f64),
        size: (f64, f64),
        color: (u8, u8, u8),
    },
    RemoveFrame {
        frame_id: u64,
    },
    SetFrameTitle {
        frame_id: u64,
        title: String,
    },
    MoveFrame {
        frame_id: u64,
        new_pos: (f64, f64),
    },
    MoveFrameWithNodes {
        frame_id: u64,
        new_pos: (f64, f64),
        node_moves: Vec<(u64, (f64, f64))>,
    },
    ResizeFrame {
        frame_id: u64,
        new_size: (f64, f64),
    },
    SetFrameCollapsed {
        frame_id: u64,
        collapsed: bool,
    },
    DuplicateSelection {
        entities: Vec<GraphEntityId>,
        offset: (f64, f64),
    },
    Compound(Vec<GraphTransaction>),
}

#[derive(Clone, Copy)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct GraphCommit {
    pub revision: GraphRevision,
    pub delta: GraphDelta,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphDelta {
    AddNode {
        node: GraphNode,
    },
    RemoveNode {
        node: GraphNode,
        connections: Vec<GraphConnection>,
    },
    MoveNode {
        id: u64,
        old_pos: (f64, f64),
        new_pos: (f64, f64),
    },
    MoveNodes {
        moves: Vec<(u64, (f64, f64), (f64, f64))>,
    },
    SetNodeTitle {
        id: u64,
        old_title: String,
        new_title: String,
    },
    SetNodeCollapsed {
        id: u64,
        old: bool,
        new: bool,
    },
    SetParameter {
        node_id: u64,
        key: String,
        old: Option<ParameterValue>,
        new: ParameterValue,
    },
    AddConnection {
        connection: GraphConnection,
    },
    RemoveConnection {
        connection: GraphConnection,
    },
    AddFrame {
        frame: GraphFrame,
    },
    RemoveFrame {
        frame: GraphFrame,
    },
    SetFrameTitle {
        id: u64,
        old_title: String,
        new_title: String,
    },
    MoveFrame {
        id: u64,
        old_pos: (f64, f64),
        new_pos: (f64, f64),
    },
    MoveFrameWithNodes {
        frame_id: u64,
        frame_old_pos: (f64, f64),
        frame_new_pos: (f64, f64),
        node_moves: Vec<(u64, (f64, f64), (f64, f64))>,
    },
    ResizeFrame {
        id: u64,
        old_size: (f64, f64),
        new_size: (f64, f64),
    },
    SetFrameCollapsed {
        id: u64,
        old: bool,
        new: bool,
    },
    Compound(Vec<GraphDelta>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    DanglingConnection {
        connection_id: u64,
        missing_node: u64,
    },
    UnknownPort {
        node_id: u64,
        port_id: u32,
    },
    DirectionMismatch {
        connection_id: u64,
    },
    TypeMismatch {
        connection_id: u64,
        output_type: PortTypeId,
        input_type: PortTypeId,
    },
    CycleDetected {
        involved_nodes: Vec<u64>,
    },
    DuplicateConnection {
        from: (u64, u32),
        to: (u64, u32),
    },
    UnknownNodeType {
        node_id: u64,
        type_id: u64,
    },
    CardinalityViolation {
        node_id: u64,
        port_id: u32,
        max: u32,
        actual: u32,
    },
    UnknownNode {
        node_id: u64,
    },
    UnknownConnection {
        connection_id: u64,
    },
    UnknownFrame {
        frame_id: u64,
    },
    SelfConnection {
        node_id: u64,
    },
    InvalidFrameSize {
        size: (f64, f64),
    },
    EmptyCompound,
    InvalidSerializedGraph {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleError {
    pub involved_nodes: Vec<u64>,
}

pub type SerializableGraph = NodeGraph;

impl NodeGraph {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            nodes: HashMap::new(),
            connections: HashMap::new(),
            frames: HashMap::new(),
            metadata: GraphMetadata::default(),
        }
    }

    pub fn node(&self, id: u64) -> Option<&GraphNode> {
        self.nodes.get(&id)
    }

    pub fn connection(&self, id: u64) -> Option<&GraphConnection> {
        self.connections.get(&id)
    }

    pub fn frame(&self, id: u64) -> Option<&GraphFrame> {
        self.frames.get(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &GraphNode> {
        self.nodes.values()
    }

    pub fn connections(&self) -> impl Iterator<Item = &GraphConnection> {
        self.connections.values()
    }

    pub fn frames(&self) -> impl Iterator<Item = &GraphFrame> {
        self.frames.values()
    }
}

impl Default for GraphMetadata {
    fn default() -> Self {
        Self {
            created_at: None,
            modified_at: None,
            version: 1,
            custom: HashMap::new(),
        }
    }
}

impl NodeTypeCatalog {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            revision: CatalogRevision(0),
        }
    }

    pub fn register(&mut self, node_type: NodeType) {
        self.types.insert(node_type.type_id, node_type);
        self.revision.0 += 1;
    }

    pub fn get(&self, type_id: u64) -> Option<&NodeType> {
        self.types.get(&type_id)
    }

    pub fn revision(&self) -> CatalogRevision {
        self.revision
    }

    pub fn types(&self) -> impl Iterator<Item = &NodeType> {
        self.types.values()
    }
}

impl Default for NodeTypeCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl PortTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, info: PortTypeInfo) {
        self.types.insert(info.type_id, info);
    }

    pub fn get(&self, id: PortTypeId) -> Option<&PortTypeInfo> {
        self.types.get(&id)
    }

    pub fn are_compatible(&self, output_type: PortTypeId, input_type: PortTypeId) -> bool {
        if output_type == PortTypeId::ANY || input_type == PortTypeId::ANY {
            return true;
        }

        self.types
            .get(&output_type)
            .is_some_and(|info| info.compatible_with.contains(&input_type))
    }

    pub fn color(&self, id: PortTypeId) -> Color {
        self.types
            .get(&id)
            .map(|info| info.color)
            .unwrap_or(Color::TRANSPARENT)
    }
}

impl NodeGraphDocument {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self::from_graph(NodeGraph::new(id, name))
    }

    pub fn from_graph(graph: NodeGraph) -> Self {
        let next_id = next_id_after_graph(&graph);
        Self {
            graph,
            revision: GraphRevision(0),
            next_id,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_depth: DEFAULT_MAX_UNDO_DEPTH,
        }
    }

    pub fn with_max_undo_depth(mut self, max_undo_depth: usize) -> Self {
        self.max_undo_depth = max_undo_depth;
        self.truncate_undo_stack();
        self
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }

    pub fn revision(&self) -> GraphRevision {
        self.revision
    }

    pub fn apply(
        &mut self,
        transaction: GraphTransaction,
        context: GraphTransactionContext<'_>,
    ) -> Result<GraphCommit, GraphError> {
        let mut graph = self.graph.clone();
        let mut next_id = self.next_id;
        let delta = execute_transaction(&mut graph, &mut next_id, &transaction, context)?;

        self.graph = graph;
        self.next_id = next_id;
        self.revision.0 += 1;
        self.undo_stack.push(delta.clone());
        self.truncate_undo_stack();
        self.redo_stack.clear();

        Ok(GraphCommit {
            revision: self.revision,
            delta,
        })
    }

    pub fn node(&self, id: u64) -> Option<&GraphNode> {
        self.graph.node(id)
    }

    pub fn connection(&self, id: u64) -> Option<&GraphConnection> {
        self.graph.connection(id)
    }

    pub fn frame(&self, id: u64) -> Option<&GraphFrame> {
        self.graph.frame(id)
    }

    pub fn connections_for_node(&self, node_id: u64) -> Vec<&GraphConnection> {
        self.graph
            .connections
            .values()
            .filter(|connection| connection.from_node == node_id || connection.to_node == node_id)
            .collect()
    }

    pub fn connections_for_port(&self, node_id: u64, port_id: u32) -> Vec<&GraphConnection> {
        self.graph
            .connections
            .values()
            .filter(|connection| {
                (connection.from_node == node_id && connection.from_port == port_id)
                    || (connection.to_node == node_id && connection.to_port == port_id)
            })
            .collect()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &GraphNode> {
        self.graph.nodes()
    }

    pub fn connections(&self) -> impl Iterator<Item = &GraphConnection> {
        self.graph.connections()
    }

    pub fn frames(&self) -> impl Iterator<Item = &GraphFrame> {
        self.graph.frames()
    }

    pub fn undo(&mut self) -> bool {
        let Some(delta) = self.undo_stack.pop() else {
            return false;
        };
        apply_delta_inverse(&mut self.graph, &delta);
        self.next_id = next_id_after_graph(&self.graph).max(self.next_id);
        self.redo_stack.push(delta);
        self.revision.0 += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(delta) = self.redo_stack.pop() else {
            return false;
        };
        apply_delta_forward(&mut self.graph, &delta);
        self.next_id = next_id_after_graph(&self.graph).max(self.next_id);
        self.undo_stack.push(delta);
        self.truncate_undo_stack();
        self.revision.0 += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn validate(
        &self,
        catalog: &NodeTypeCatalog,
        registry: &PortTypeRegistry,
    ) -> Vec<GraphError> {
        validate_graph(&self.graph, catalog, registry)
    }

    pub fn topological_sort(&self) -> Result<Vec<u64>, CycleError> {
        topological_sort_graph(&self.graph)
    }

    pub fn has_cycles(&self) -> bool {
        self.topological_sort().is_err()
    }

    pub fn to_serializable(&self) -> SerializableGraph {
        self.graph.clone()
    }

    pub fn from_serializable(data: SerializableGraph) -> Result<Self, GraphError> {
        validate_serialized_shape(&data)?;
        Ok(Self::from_graph(data))
    }

    fn truncate_undo_stack(&mut self) {
        if self.max_undo_depth == 0 {
            self.undo_stack.clear();
            return;
        }

        let overflow = self.undo_stack.len().saturating_sub(self.max_undo_depth);
        if overflow > 0 {
            self.undo_stack.drain(0..overflow);
        }
    }
}

fn execute_transaction(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    transaction: &GraphTransaction,
    context: GraphTransactionContext<'_>,
) -> Result<GraphDelta, GraphError> {
    if let Some(validator) = context.validator {
        validator.validate_transaction(
            graph,
            transaction,
            context.catalog,
            context.port_registry,
        )?;
    }

    execute_transaction_unvalidated(graph, next_id, transaction, context)
}

fn execute_transaction_unvalidated(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    transaction: &GraphTransaction,
    context: GraphTransactionContext<'_>,
) -> Result<GraphDelta, GraphError> {
    match transaction {
        GraphTransaction::AddNode {
            type_id,
            position,
            title,
        } => add_node(
            graph,
            next_id,
            context.catalog,
            *type_id,
            *position,
            title.clone(),
        ),
        GraphTransaction::RemoveNode { node_id } => remove_node(graph, *node_id),
        GraphTransaction::RemoveNodes { node_ids } => {
            let mut deltas = Vec::new();
            for node_id in stable_unique_ids(node_ids.iter().copied()) {
                deltas.push(remove_node(graph, node_id)?);
            }
            Ok(GraphDelta::Compound(deltas))
        }
        GraphTransaction::MoveNodes { moves } => move_nodes(graph, moves),
        GraphTransaction::SetNodeTitle { node_id, title } => {
            let node = graph
                .nodes
                .get_mut(node_id)
                .ok_or(GraphError::UnknownNode { node_id: *node_id })?;
            let old_title = node.title.clone();
            node.title = title.clone();
            Ok(GraphDelta::SetNodeTitle {
                id: *node_id,
                old_title,
                new_title: title.clone(),
            })
        }
        GraphTransaction::SetNodeCollapsed { node_id, collapsed } => {
            let node = graph
                .nodes
                .get_mut(node_id)
                .ok_or(GraphError::UnknownNode { node_id: *node_id })?;
            let old = node.collapsed;
            node.collapsed = *collapsed;
            Ok(GraphDelta::SetNodeCollapsed {
                id: *node_id,
                old,
                new: *collapsed,
            })
        }
        GraphTransaction::SetParameter {
            node_id,
            key,
            value,
        } => {
            let node = graph
                .nodes
                .get_mut(node_id)
                .ok_or(GraphError::UnknownNode { node_id: *node_id })?;
            let old = node.parameters.insert(key.clone(), value.clone());
            Ok(GraphDelta::SetParameter {
                node_id: *node_id,
                key: key.clone(),
                old,
                new: value.clone(),
            })
        }
        GraphTransaction::AddConnection {
            from_node,
            from_port,
            to_node,
            to_port,
        } => add_connection(
            graph,
            next_id,
            context.catalog,
            context.port_registry,
            *from_node,
            *from_port,
            *to_node,
            *to_port,
        ),
        GraphTransaction::RemoveConnection { connection_id } => {
            remove_connection(graph, *connection_id)
        }
        GraphTransaction::ReplaceConnection {
            old_connection_id,
            from_node,
            from_port,
            to_node,
            to_port,
        } => {
            let remove_delta = remove_connection(graph, *old_connection_id)?;
            let add_delta = add_connection(
                graph,
                next_id,
                context.catalog,
                context.port_registry,
                *from_node,
                *from_port,
                *to_node,
                *to_port,
            )?;
            Ok(GraphDelta::Compound(vec![remove_delta, add_delta]))
        }
        GraphTransaction::AddFrame {
            title,
            position,
            size,
            color,
        } => add_frame(graph, next_id, title.clone(), *position, *size, *color),
        GraphTransaction::RemoveFrame { frame_id } => {
            let frame = graph
                .frames
                .remove(frame_id)
                .ok_or(GraphError::UnknownFrame {
                    frame_id: *frame_id,
                })?;
            Ok(GraphDelta::RemoveFrame { frame })
        }
        GraphTransaction::SetFrameTitle { frame_id, title } => {
            let frame = graph
                .frames
                .get_mut(frame_id)
                .ok_or(GraphError::UnknownFrame {
                    frame_id: *frame_id,
                })?;
            let old_title = frame.title.clone();
            frame.title = title.clone();
            Ok(GraphDelta::SetFrameTitle {
                id: *frame_id,
                old_title,
                new_title: title.clone(),
            })
        }
        GraphTransaction::MoveFrame { frame_id, new_pos } => {
            let frame = graph
                .frames
                .get_mut(frame_id)
                .ok_or(GraphError::UnknownFrame {
                    frame_id: *frame_id,
                })?;
            let old_pos = frame.position;
            frame.position = *new_pos;
            Ok(GraphDelta::MoveFrame {
                id: *frame_id,
                old_pos,
                new_pos: *new_pos,
            })
        }
        GraphTransaction::MoveFrameWithNodes {
            frame_id,
            new_pos,
            node_moves,
        } => move_frame_with_nodes(graph, *frame_id, *new_pos, node_moves),
        GraphTransaction::ResizeFrame { frame_id, new_size } => {
            validate_frame_size(*new_size)?;
            let frame = graph
                .frames
                .get_mut(frame_id)
                .ok_or(GraphError::UnknownFrame {
                    frame_id: *frame_id,
                })?;
            let old_size = frame.size;
            frame.size = *new_size;
            Ok(GraphDelta::ResizeFrame {
                id: *frame_id,
                old_size,
                new_size: *new_size,
            })
        }
        GraphTransaction::SetFrameCollapsed {
            frame_id,
            collapsed,
        } => {
            let frame = graph
                .frames
                .get_mut(frame_id)
                .ok_or(GraphError::UnknownFrame {
                    frame_id: *frame_id,
                })?;
            let old = frame.collapsed;
            frame.collapsed = *collapsed;
            Ok(GraphDelta::SetFrameCollapsed {
                id: *frame_id,
                old,
                new: *collapsed,
            })
        }
        GraphTransaction::DuplicateSelection { entities, offset } => {
            duplicate_selection(graph, next_id, entities, *offset)
        }
        GraphTransaction::Compound(transactions) => {
            if transactions.is_empty() {
                return Err(GraphError::EmptyCompound);
            }

            let mut deltas = Vec::with_capacity(transactions.len());
            for transaction in transactions {
                deltas.push(execute_transaction_unvalidated(
                    graph,
                    next_id,
                    transaction,
                    context,
                )?);
            }
            Ok(GraphDelta::Compound(deltas))
        }
    }
}

fn add_node(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    catalog: &NodeTypeCatalog,
    type_id: u64,
    position: (f64, f64),
    title: Option<String>,
) -> Result<GraphDelta, GraphError> {
    let node_type = catalog.get(type_id).ok_or(GraphError::UnknownNodeType {
        node_id: 0,
        type_id,
    })?;
    let node = GraphNode {
        id: allocate_id(next_id),
        type_id,
        position,
        title: title.unwrap_or_else(|| node_type.default_title.clone()),
        collapsed: false,
        parameters: HashMap::new(),
    };
    graph.nodes.insert(node.id, node.clone());
    Ok(GraphDelta::AddNode { node })
}

fn remove_node(graph: &mut NodeGraph, node_id: u64) -> Result<GraphDelta, GraphError> {
    let node = graph
        .nodes
        .remove(&node_id)
        .ok_or(GraphError::UnknownNode { node_id })?;
    let connection_ids: Vec<u64> = graph
        .connections
        .values()
        .filter(|connection| connection.from_node == node_id || connection.to_node == node_id)
        .map(|connection| connection.id)
        .collect();
    let mut connections = Vec::with_capacity(connection_ids.len());
    for connection_id in connection_ids {
        if let Some(connection) = graph.connections.remove(&connection_id) {
            connections.push(connection);
        }
    }
    connections.sort_by_key(|connection| connection.id);
    Ok(GraphDelta::RemoveNode { node, connections })
}

fn move_nodes(
    graph: &mut NodeGraph,
    moves: &[(u64, (f64, f64))],
) -> Result<GraphDelta, GraphError> {
    let moves = stable_latest_node_moves(moves);
    for (node_id, _) in &moves {
        if !graph.nodes.contains_key(node_id) {
            return Err(GraphError::UnknownNode { node_id: *node_id });
        }
    }

    let mut delta_moves = Vec::with_capacity(moves.len());
    for (node_id, new_pos) in moves {
        let node = graph
            .nodes
            .get_mut(&node_id)
            .expect("validated node exists");
        let old_pos = node.position;
        node.position = new_pos;
        delta_moves.push((node_id, old_pos, new_pos));
    }

    if delta_moves.len() == 1 {
        let (id, old_pos, new_pos) = delta_moves[0];
        Ok(GraphDelta::MoveNode {
            id,
            old_pos,
            new_pos,
        })
    } else {
        Ok(GraphDelta::MoveNodes { moves: delta_moves })
    }
}

fn add_connection(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    from_node: u64,
    from_port: u32,
    to_node: u64,
    to_port: u32,
) -> Result<GraphDelta, GraphError> {
    let connection = GraphConnection {
        id: allocate_id(next_id),
        from_node,
        from_port,
        to_node,
        to_port,
    };
    validate_connection(graph, &connection, catalog, registry)?;
    graph.connections.insert(connection.id, connection.clone());
    Ok(GraphDelta::AddConnection { connection })
}

fn remove_connection(graph: &mut NodeGraph, connection_id: u64) -> Result<GraphDelta, GraphError> {
    let connection = graph
        .connections
        .remove(&connection_id)
        .ok_or(GraphError::UnknownConnection { connection_id })?;
    Ok(GraphDelta::RemoveConnection { connection })
}

fn add_frame(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    title: String,
    position: (f64, f64),
    size: (f64, f64),
    color: (u8, u8, u8),
) -> Result<GraphDelta, GraphError> {
    validate_frame_size(size)?;
    let frame = GraphFrame {
        id: allocate_id(next_id),
        title,
        description: None,
        color,
        position,
        size,
        collapsed: false,
    };
    graph.frames.insert(frame.id, frame.clone());
    Ok(GraphDelta::AddFrame { frame })
}

fn move_frame_with_nodes(
    graph: &mut NodeGraph,
    frame_id: u64,
    frame_new_pos: (f64, f64),
    node_moves: &[(u64, (f64, f64))],
) -> Result<GraphDelta, GraphError> {
    let node_moves = stable_latest_node_moves(node_moves);
    for (node_id, _) in &node_moves {
        if !graph.nodes.contains_key(node_id) {
            return Err(GraphError::UnknownNode { node_id: *node_id });
        }
    }

    let frame = graph
        .frames
        .get_mut(&frame_id)
        .ok_or(GraphError::UnknownFrame { frame_id })?;
    let frame_old_pos = frame.position;
    frame.position = frame_new_pos;

    let mut delta_node_moves = Vec::with_capacity(node_moves.len());
    for (node_id, new_pos) in node_moves {
        let node = graph
            .nodes
            .get_mut(&node_id)
            .expect("validated node exists");
        let old_pos = node.position;
        node.position = new_pos;
        delta_node_moves.push((node_id, old_pos, new_pos));
    }

    Ok(GraphDelta::MoveFrameWithNodes {
        frame_id,
        frame_old_pos,
        frame_new_pos,
        node_moves: delta_node_moves,
    })
}

fn duplicate_selection(
    graph: &mut NodeGraph,
    next_id: &mut u64,
    entities: &[GraphEntityId],
    offset: (f64, f64),
) -> Result<GraphDelta, GraphError> {
    let node_ids: Vec<u64> = stable_unique_ids(entities.iter().filter_map(|entity| match entity {
        GraphEntityId::Node(node_id) => Some(*node_id),
        _ => None,
    }));
    let frame_ids: Vec<u64> =
        stable_unique_ids(entities.iter().filter_map(|entity| match entity {
            GraphEntityId::Frame(frame_id) => Some(*frame_id),
            _ => None,
        }));
    let explicitly_selected_connections: HashSet<u64> = entities
        .iter()
        .filter_map(|entity| match entity {
            GraphEntityId::Connection(connection_id) => Some(*connection_id),
            _ => None,
        })
        .collect();

    for node_id in &node_ids {
        if !graph.nodes.contains_key(node_id) {
            return Err(GraphError::UnknownNode { node_id: *node_id });
        }
    }
    for frame_id in &frame_ids {
        if !graph.frames.contains_key(frame_id) {
            return Err(GraphError::UnknownFrame {
                frame_id: *frame_id,
            });
        }
    }
    for connection_id in &explicitly_selected_connections {
        if !graph.connections.contains_key(connection_id) {
            return Err(GraphError::UnknownConnection {
                connection_id: *connection_id,
            });
        }
    }

    let selected_nodes: HashSet<u64> = node_ids.iter().copied().collect();
    let mut node_map = HashMap::new();
    let mut deltas = Vec::new();

    for node_id in node_ids {
        let original = graph.nodes.get(&node_id).expect("validated node exists");
        let mut node = original.clone();
        node.id = allocate_id(next_id);
        node.position = (node.position.0 + offset.0, node.position.1 + offset.1);
        node_map.insert(node_id, node.id);
        graph.nodes.insert(node.id, node.clone());
        deltas.push(GraphDelta::AddNode { node });
    }

    for frame_id in frame_ids {
        let original = graph.frames.get(&frame_id).expect("validated frame exists");
        let mut frame = original.clone();
        frame.id = allocate_id(next_id);
        frame.position = (frame.position.0 + offset.0, frame.position.1 + offset.1);
        graph.frames.insert(frame.id, frame.clone());
        deltas.push(GraphDelta::AddFrame { frame });
    }

    let mut connections: Vec<GraphConnection> = graph
        .connections
        .values()
        .filter(|connection| {
            selected_nodes.contains(&connection.from_node)
                && selected_nodes.contains(&connection.to_node)
                && (explicitly_selected_connections.is_empty()
                    || explicitly_selected_connections.contains(&connection.id)
                    || selected_nodes.contains(&connection.from_node)
                        && selected_nodes.contains(&connection.to_node))
        })
        .cloned()
        .collect();
    connections.sort_by_key(|connection| connection.id);

    for original in connections {
        let connection = GraphConnection {
            id: allocate_id(next_id),
            from_node: node_map[&original.from_node],
            from_port: original.from_port,
            to_node: node_map[&original.to_node],
            to_port: original.to_port,
        };
        graph.connections.insert(connection.id, connection.clone());
        deltas.push(GraphDelta::AddConnection { connection });
    }

    Ok(GraphDelta::Compound(deltas))
}

fn validate_graph(
    graph: &NodeGraph,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
) -> Vec<GraphError> {
    let mut errors = Vec::new();

    for node in graph.nodes.values() {
        if catalog.get(node.type_id).is_none() {
            errors.push(GraphError::UnknownNodeType {
                node_id: node.id,
                type_id: node.type_id,
            });
        }
    }

    let mut endpoint_pairs = HashSet::new();
    for connection in graph.connections.values() {
        if let Err(error) = validate_connection(graph, connection, catalog, registry) {
            errors.push(error);
        }

        let pair = (
            (connection.from_node, connection.from_port),
            (connection.to_node, connection.to_port),
        );
        if !endpoint_pairs.insert(pair) {
            errors.push(GraphError::DuplicateConnection {
                from: pair.0,
                to: pair.1,
            });
        }
    }

    if let Err(error) = topological_sort_graph(graph) {
        errors.push(GraphError::CycleDetected {
            involved_nodes: error.involved_nodes,
        });
    }

    errors
}

fn validate_connection(
    graph: &NodeGraph,
    connection: &GraphConnection,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
) -> Result<(), GraphError> {
    let from_node =
        graph
            .nodes
            .get(&connection.from_node)
            .ok_or(GraphError::DanglingConnection {
                connection_id: connection.id,
                missing_node: connection.from_node,
            })?;
    let to_node = graph
        .nodes
        .get(&connection.to_node)
        .ok_or(GraphError::DanglingConnection {
            connection_id: connection.id,
            missing_node: connection.to_node,
        })?;

    if connection.from_node == connection.to_node {
        return Err(GraphError::SelfConnection {
            node_id: connection.from_node,
        });
    }

    let from_type = catalog
        .get(from_node.type_id)
        .ok_or(GraphError::UnknownNodeType {
            node_id: from_node.id,
            type_id: from_node.type_id,
        })?;
    let to_type = catalog
        .get(to_node.type_id)
        .ok_or(GraphError::UnknownNodeType {
            node_id: to_node.id,
            type_id: to_node.type_id,
        })?;

    let from_port = find_port(from_type, connection.from_port).ok_or(GraphError::UnknownPort {
        node_id: connection.from_node,
        port_id: connection.from_port,
    })?;
    let to_port = find_port(to_type, connection.to_port).ok_or(GraphError::UnknownPort {
        node_id: connection.to_node,
        port_id: connection.to_port,
    })?;

    if from_port.direction != PortDirection::Output || to_port.direction != PortDirection::Input {
        return Err(GraphError::DirectionMismatch {
            connection_id: connection.id,
        });
    }

    if !registry.are_compatible(from_port.type_id, to_port.type_id) {
        return Err(GraphError::TypeMismatch {
            connection_id: connection.id,
            output_type: from_port.type_id,
            input_type: to_port.type_id,
        });
    }

    for existing in graph.connections.values() {
        if existing.id != connection.id
            && existing.from_node == connection.from_node
            && existing.from_port == connection.from_port
            && existing.to_node == connection.to_node
            && existing.to_port == connection.to_port
        {
            return Err(GraphError::DuplicateConnection {
                from: (connection.from_node, connection.from_port),
                to: (connection.to_node, connection.to_port),
            });
        }
    }

    validate_cardinality(graph, connection.from_node, from_port, connection.id)?;
    validate_cardinality(graph, connection.to_node, to_port, connection.id)?;

    Ok(())
}

fn validate_cardinality(
    graph: &NodeGraph,
    node_id: u64,
    port: &PortDefinition,
    pending_connection_id: u64,
) -> Result<(), GraphError> {
    let max = match port.max_connections {
        PortCardinality::Single => 1,
        PortCardinality::Multiple => return Ok(()),
    };
    let actual = graph
        .connections
        .values()
        .filter(|connection| {
            connection.id != pending_connection_id
                && ((connection.from_node == node_id && connection.from_port == port.port_id)
                    || (connection.to_node == node_id && connection.to_port == port.port_id))
        })
        .count() as u32
        + 1;

    if actual > max {
        Err(GraphError::CardinalityViolation {
            node_id,
            port_id: port.port_id,
            max,
            actual,
        })
    } else {
        Ok(())
    }
}

fn find_port(node_type: &NodeType, port_id: u32) -> Option<&PortDefinition> {
    node_type
        .input_ports
        .iter()
        .chain(node_type.output_ports.iter())
        .find(|port| port.port_id == port_id)
}

fn topological_sort_graph(graph: &NodeGraph) -> Result<Vec<u64>, CycleError> {
    let mut indegree: HashMap<u64, usize> = graph.nodes.keys().map(|id| (*id, 0)).collect();
    let mut outgoing: HashMap<u64, Vec<u64>> = HashMap::new();

    for connection in graph.connections.values() {
        if graph.nodes.contains_key(&connection.from_node)
            && graph.nodes.contains_key(&connection.to_node)
        {
            *indegree.entry(connection.to_node).or_insert(0) += 1;
            outgoing
                .entry(connection.from_node)
                .or_default()
                .push(connection.to_node);
        }
    }

    let mut ready: VecDeque<u64> = indegree
        .iter()
        .filter_map(|(node_id, degree)| (*degree == 0).then_some(*node_id))
        .collect();
    ready.make_contiguous().sort_unstable();

    let mut sorted = Vec::with_capacity(graph.nodes.len());
    while let Some(node_id) = ready.pop_front() {
        sorted.push(node_id);
        if let Some(targets) = outgoing.get(&node_id) {
            let mut targets = targets.clone();
            targets.sort_unstable();
            for target in targets {
                let degree = indegree.get_mut(&target).expect("target node has indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(target);
                }
            }
        }
    }

    if sorted.len() == graph.nodes.len() {
        Ok(sorted)
    } else {
        let mut involved_nodes: Vec<u64> = indegree
            .into_iter()
            .filter_map(|(node_id, degree)| (degree > 0).then_some(node_id))
            .collect();
        involved_nodes.sort_unstable();
        Err(CycleError { involved_nodes })
    }
}

fn apply_delta_forward(graph: &mut NodeGraph, delta: &GraphDelta) {
    match delta {
        GraphDelta::AddNode { node } => {
            graph.nodes.insert(node.id, node.clone());
        }
        GraphDelta::RemoveNode { node, connections } => {
            graph.nodes.remove(&node.id);
            for connection in connections {
                graph.connections.remove(&connection.id);
            }
        }
        GraphDelta::MoveNode { id, new_pos, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.position = *new_pos;
            }
        }
        GraphDelta::MoveNodes { moves } => {
            for (id, _, new_pos) in moves {
                if let Some(node) = graph.nodes.get_mut(id) {
                    node.position = *new_pos;
                }
            }
        }
        GraphDelta::SetNodeTitle { id, new_title, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.title = new_title.clone();
            }
        }
        GraphDelta::SetNodeCollapsed { id, new, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.collapsed = *new;
            }
        }
        GraphDelta::SetParameter {
            node_id, key, new, ..
        } => {
            if let Some(node) = graph.nodes.get_mut(node_id) {
                node.parameters.insert(key.clone(), new.clone());
            }
        }
        GraphDelta::AddConnection { connection } => {
            graph.connections.insert(connection.id, connection.clone());
        }
        GraphDelta::RemoveConnection { connection } => {
            graph.connections.remove(&connection.id);
        }
        GraphDelta::AddFrame { frame } => {
            graph.frames.insert(frame.id, frame.clone());
        }
        GraphDelta::RemoveFrame { frame } => {
            graph.frames.remove(&frame.id);
        }
        GraphDelta::SetFrameTitle { id, new_title, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.title = new_title.clone();
            }
        }
        GraphDelta::MoveFrame { id, new_pos, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.position = *new_pos;
            }
        }
        GraphDelta::MoveFrameWithNodes {
            frame_id,
            frame_new_pos,
            node_moves,
            ..
        } => {
            if let Some(frame) = graph.frames.get_mut(frame_id) {
                frame.position = *frame_new_pos;
            }
            for (node_id, _, new_pos) in node_moves {
                if let Some(node) = graph.nodes.get_mut(node_id) {
                    node.position = *new_pos;
                }
            }
        }
        GraphDelta::ResizeFrame { id, new_size, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.size = *new_size;
            }
        }
        GraphDelta::SetFrameCollapsed { id, new, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.collapsed = *new;
            }
        }
        GraphDelta::Compound(deltas) => {
            for delta in deltas {
                apply_delta_forward(graph, delta);
            }
        }
    }
}

fn apply_delta_inverse(graph: &mut NodeGraph, delta: &GraphDelta) {
    match delta {
        GraphDelta::AddNode { node } => {
            graph.nodes.remove(&node.id);
            let connection_ids: Vec<u64> = graph
                .connections
                .values()
                .filter(|connection| {
                    connection.from_node == node.id || connection.to_node == node.id
                })
                .map(|connection| connection.id)
                .collect();
            for connection_id in connection_ids {
                graph.connections.remove(&connection_id);
            }
        }
        GraphDelta::RemoveNode { node, connections } => {
            graph.nodes.insert(node.id, node.clone());
            for connection in connections {
                graph.connections.insert(connection.id, connection.clone());
            }
        }
        GraphDelta::MoveNode { id, old_pos, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.position = *old_pos;
            }
        }
        GraphDelta::MoveNodes { moves } => {
            for (id, old_pos, _) in moves {
                if let Some(node) = graph.nodes.get_mut(id) {
                    node.position = *old_pos;
                }
            }
        }
        GraphDelta::SetNodeTitle { id, old_title, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.title = old_title.clone();
            }
        }
        GraphDelta::SetNodeCollapsed { id, old, .. } => {
            if let Some(node) = graph.nodes.get_mut(id) {
                node.collapsed = *old;
            }
        }
        GraphDelta::SetParameter {
            node_id, key, old, ..
        } => {
            if let Some(node) = graph.nodes.get_mut(node_id) {
                if let Some(old) = old {
                    node.parameters.insert(key.clone(), old.clone());
                } else {
                    node.parameters.remove(key);
                }
            }
        }
        GraphDelta::AddConnection { connection } => {
            graph.connections.remove(&connection.id);
        }
        GraphDelta::RemoveConnection { connection } => {
            graph.connections.insert(connection.id, connection.clone());
        }
        GraphDelta::AddFrame { frame } => {
            graph.frames.remove(&frame.id);
        }
        GraphDelta::RemoveFrame { frame } => {
            graph.frames.insert(frame.id, frame.clone());
        }
        GraphDelta::SetFrameTitle { id, old_title, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.title = old_title.clone();
            }
        }
        GraphDelta::MoveFrame { id, old_pos, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.position = *old_pos;
            }
        }
        GraphDelta::MoveFrameWithNodes {
            frame_id,
            frame_old_pos,
            node_moves,
            ..
        } => {
            if let Some(frame) = graph.frames.get_mut(frame_id) {
                frame.position = *frame_old_pos;
            }
            for (node_id, old_pos, _) in node_moves {
                if let Some(node) = graph.nodes.get_mut(node_id) {
                    node.position = *old_pos;
                }
            }
        }
        GraphDelta::ResizeFrame { id, old_size, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.size = *old_size;
            }
        }
        GraphDelta::SetFrameCollapsed { id, old, .. } => {
            if let Some(frame) = graph.frames.get_mut(id) {
                frame.collapsed = *old;
            }
        }
        GraphDelta::Compound(deltas) => {
            for delta in deltas.iter().rev() {
                apply_delta_inverse(graph, delta);
            }
        }
    }
}

fn validate_serialized_shape(graph: &NodeGraph) -> Result<(), GraphError> {
    for (id, node) in &graph.nodes {
        if *id != node.id {
            return Err(GraphError::InvalidSerializedGraph {
                reason: format!("node map key {} does not match node id {}", id, node.id),
            });
        }
    }
    for (id, connection) in &graph.connections {
        if *id != connection.id {
            return Err(GraphError::InvalidSerializedGraph {
                reason: format!(
                    "connection map key {} does not match connection id {}",
                    id, connection.id
                ),
            });
        }
    }
    for (id, frame) in &graph.frames {
        if *id != frame.id {
            return Err(GraphError::InvalidSerializedGraph {
                reason: format!("frame map key {} does not match frame id {}", id, frame.id),
            });
        }
        validate_frame_size(frame.size)?;
    }
    Ok(())
}

fn validate_frame_size(size: (f64, f64)) -> Result<(), GraphError> {
    if size.0 > 0.0 && size.1 > 0.0 {
        Ok(())
    } else {
        Err(GraphError::InvalidFrameSize { size })
    }
}

fn allocate_id(next_id: &mut u64) -> u64 {
    let id = *next_id;
    *next_id += 1;
    id
}

fn next_id_after_graph(graph: &NodeGraph) -> u64 {
    graph
        .nodes
        .keys()
        .chain(graph.connections.keys())
        .chain(graph.frames.keys())
        .copied()
        .max()
        .unwrap_or(0)
        + 1
}

fn stable_unique_ids(ids: impl Iterator<Item = u64>) -> Vec<u64> {
    let mut seen = BTreeSet::new();
    ids.filter(|id| seen.insert(*id)).collect()
}

fn stable_latest_node_moves(moves: &[(u64, (f64, f64))]) -> Vec<(u64, (f64, f64))> {
    let mut index_by_id: HashMap<u64, usize> = HashMap::new();
    let mut normalized: Vec<(u64, (f64, f64))> = Vec::new();
    for (node_id, position) in moves {
        if let Some(index) = index_by_id.get(node_id).copied() {
            normalized[index].1 = *position;
        } else {
            index_by_id.insert(*node_id, normalized.len());
            normalized.push((*node_id, *position));
        }
    }
    normalized
}
