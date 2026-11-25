use std::collections::{HashMap, HashSet};

use crate::core::math::{Point, RectF};
use crate::core::paint::Color;
use crate::engines::route_geometry::{
    RouteDirection, RouteEndpoint, RouteParams, route_right_angle,
};
use crate::model::canvas::{
    CanvasEntityId, CanvasHit, CanvasHitRegion, CanvasIntent, CanvasPaintDescriptor,
    CanvasProjection, CanvasViewportState, HitPriority, ProjectedCanvasItem, ProjectionRevision,
    WorldPoint, WorldRect,
};
use crate::model::node_graph::{
    GraphConnection, GraphFrame, NodeGraphDocument, NodeType, NodeTypeCatalog, PortDefinition,
    PortDirection, PortTypeRegistry,
};
use crate::model::spatial::{
    CanvasSpatialBridge, SpatialAdapter, SpatialAxisOrientation, SpatialHitPriority, SpatialIntent,
    SpatialItemId, SpatialProjection, SpatialProjectionError, SpatialRoutePlan,
    SpatialViewportState, SpatialVisualPolicy,
};
use crate::renderer::path::Path;

const DEFAULT_NODE_WIDTH: f64 = 180.0;
const DEFAULT_TITLE_BAR_HEIGHT: f64 = 32.0;
const DEFAULT_PORT_ROW_HEIGHT: f64 = 24.0;
const DEFAULT_BODY_HEIGHT: f64 = 0.0;
const DEFAULT_PORT_RADIUS: f64 = 5.0;
const DEFAULT_PORT_HIT_RADIUS: f64 = 10.0;
const DEFAULT_WIRE_HIT_TOLERANCE: f64 = 5.0;
const FRAME_Z_ORDER: i32 = 0;
const WIRE_Z_ORDER: i32 = 50;
const EMPHASIZED_WIRE_Z_ORDER: i32 = 90;
const NODE_Z_ORDER: i32 = 100;
const PORT_Z_ORDER: i32 = 110;

#[derive(Debug, Clone)]
pub struct NodeGraphProjection {
    revision: ProjectionRevision,
    nodes: Vec<NodeProjection>,
    wires: Vec<WireProjection>,
    frames: Vec<FrameProjection>,
    canvas: CanvasProjection,
}

/// Stateless session marker for node graph spatial projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeGraphProjectionSession;

/// Spatial adapter for read-only node graph projection and Canvas intent translation.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeGraphSpatialAdapter;

#[derive(Debug, Clone)]
pub struct NodeProjection {
    pub id: u64,
    pub type_id: u64,
    pub position: WorldPoint,
    pub title: String,
    pub color: Color,
    pub collapsed: bool,
    pub state: NodeVisualState,
    pub inputs: Vec<PortProjection>,
    pub outputs: Vec<PortProjection>,
    pub body_height: f64,
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

#[derive(Debug, Clone)]
pub struct PortProjection {
    pub node_id: u64,
    pub definition: PortDefinition,
    pub connection_count: u32,
    pub world_pos: WorldPoint,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub struct WireProjection {
    pub id: u64,
    pub from_node: u64,
    pub from_port: u32,
    pub to_node: u64,
    pub to_port: u32,
    pub color: Option<Color>,
    pub style: WireStyle,
    pub state: WireVisualState,
    pub from_pos: WorldPoint,
    pub to_pos: WorldPoint,
    pub path: Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireStyle {
    #[default]
    Bezier,
    Straight,
    RightAngle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireVisualState {
    #[default]
    Default,
    Hovered,
    Selected,
    Critical,
    Preview,
    Dimmed,
}

#[derive(Debug, Clone)]
pub struct FrameProjection {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub color: Color,
    pub position: WorldPoint,
    pub size: (f64, f64),
    pub collapsed: bool,
    pub contained_nodes: Vec<u64>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphProjectionGeometry {
    pub node_width: f64,
    pub title_bar_height: f64,
    pub port_row_height: f64,
    pub body_height: f64,
    pub port_radius: f64,
    pub port_hit_radius: f64,
    pub wire_hit_tolerance: f64,
}

impl Default for NodeGraphProjectionGeometry {
    fn default() -> Self {
        Self {
            node_width: DEFAULT_NODE_WIDTH,
            title_bar_height: DEFAULT_TITLE_BAR_HEIGHT,
            port_row_height: DEFAULT_PORT_ROW_HEIGHT,
            body_height: DEFAULT_BODY_HEIGHT,
            port_radius: DEFAULT_PORT_RADIUS,
            port_hit_radius: DEFAULT_PORT_HIT_RADIUS,
            wire_hit_tolerance: DEFAULT_WIRE_HIT_TOLERANCE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeGraphProjectionConfig {
    pub geometry: NodeGraphProjectionGeometry,
    pub wire_style: WireStyle,
    pub show_arrowheads: bool,
}

impl Default for NodeGraphProjectionConfig {
    fn default() -> Self {
        Self {
            geometry: NodeGraphProjectionGeometry::default(),
            wire_style: WireStyle::Bezier,
            show_arrowheads: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeGraphProjectionState {
    pub revision_salt: u64,
    pub node_offsets: HashMap<u64, (f64, f64)>,
    pub frame_offsets: HashMap<u64, (f64, f64)>,
    pub frame_size_overrides: HashMap<u64, (f64, f64)>,
    pub node_states: HashMap<u64, NodeVisualState>,
    pub wire_states: HashMap<u64, WireVisualState>,
    pub frame_states: HashMap<u64, FrameVisualState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeGraphProjectionError {
    UnknownNodeType {
        node_id: u64,
        type_id: u64,
    },
    MissingConnectionNode {
        connection_id: u64,
        node_id: u64,
    },
    UnknownPort {
        connection_id: u64,
        node_id: u64,
        port_id: u32,
        direction: PortDirection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PortKey {
    node_id: u64,
    port_id: u32,
    direction: PortDirection,
}

impl NodeGraphProjection {
    pub fn from_document(
        document: &NodeGraphDocument,
        catalog: &NodeTypeCatalog,
        registry: &PortTypeRegistry,
    ) -> Result<Self, NodeGraphProjectionError> {
        Self::build(
            document,
            catalog,
            registry,
            &NodeGraphProjectionConfig::default(),
            &NodeGraphProjectionState::default(),
        )
    }

    pub fn build(
        document: &NodeGraphDocument,
        catalog: &NodeTypeCatalog,
        registry: &PortTypeRegistry,
        config: &NodeGraphProjectionConfig,
        state: &NodeGraphProjectionState,
    ) -> Result<Self, NodeGraphProjectionError> {
        let revision = ProjectionRevision(
            document
                .revision()
                .0
                .saturating_mul(31)
                .saturating_add(catalog.revision().0)
                .saturating_add(state.revision_salt),
        );
        let mut nodes = build_node_projections(document, catalog, registry, config, state)?;
        nodes.sort_by_key(|node| node.id);

        let mut frames = build_frame_projections(document, &nodes, state);
        frames.sort_by_key(|frame| frame.id);
        let hidden_nodes = collapsed_frame_nodes(&frames);
        let nodes: Vec<NodeProjection> = nodes
            .into_iter()
            .filter(|node| !hidden_nodes.contains(&node.id))
            .collect();

        let port_lookup = build_port_lookup(&nodes);
        let mut wires = build_wire_projections(
            document,
            &port_lookup,
            &hidden_nodes,
            registry,
            config,
            state,
        )?;
        wires.sort_by_key(|wire| wire.id);

        let mut items = Vec::new();
        append_frame_items(&mut items, &frames);
        append_wire_items(&mut items, &wires, &config.geometry);
        append_node_items(&mut items, &nodes, &config.geometry);

        let canvas = CanvasProjection::new(revision, items);
        Ok(Self {
            revision,
            nodes,
            wires,
            frames,
            canvas,
        })
    }

    pub fn revision(&self) -> ProjectionRevision {
        self.revision
    }

    pub fn nodes(&self) -> &[NodeProjection] {
        &self.nodes
    }

    pub fn ports(&self) -> impl Iterator<Item = &PortProjection> {
        self.nodes
            .iter()
            .flat_map(|node| node.inputs.iter().chain(node.outputs.iter()))
    }

    pub fn wires(&self) -> &[WireProjection] {
        &self.wires
    }

    pub fn frames(&self) -> &[FrameProjection] {
        &self.frames
    }

    pub fn canvas(&self) -> &CanvasProjection {
        &self.canvas
    }

    pub fn spatial_bridge(
        &self,
        viewport: CanvasViewportState,
        viewport_rect: RectF,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<CanvasSpatialBridge, SpatialProjectionError> {
        CanvasSpatialBridge::from_canvas_projection(
            &self.canvas,
            viewport,
            viewport_rect,
            visual_policy,
        )
    }

    pub fn spatial_projection(
        &self,
        viewport: Option<SpatialViewportState>,
        axis_orientation: Option<SpatialAxisOrientation>,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<SpatialProjection, SpatialProjectionError> {
        let (projection, canvas_entities) = CanvasSpatialBridge::project_canvas(
            &self.canvas,
            viewport,
            axis_orientation,
            visual_policy,
        )?;
        SpatialProjection::from_parts(
            projection.revision(),
            projection.viewport(),
            projection.axis_orientation(),
            projection.coordinate_mode(),
            projection.items().to_vec(),
            projection.hit_regions().to_vec(),
            node_graph_spatial_routes(&self.wires, &canvas_entities),
            projection.guides().to_vec(),
            projection.overlays().to_vec(),
            projection.visual_policy().clone(),
        )
    }
}

fn node_graph_spatial_routes(
    wires: &[WireProjection],
    canvas_entities: &HashMap<SpatialItemId, CanvasEntityId>,
) -> HashMap<SpatialItemId, SpatialRoutePlan> {
    let mut routes = HashMap::new();
    for wire in wires {
        let Some(route) = node_graph_wire_route_plan(wire) else {
            continue;
        };
        let entity = CanvasEntityId::Wire(wire.id);
        let Some(spatial_id) = canvas_entities
            .iter()
            .find_map(|(spatial_id, canvas_id)| (*canvas_id == entity).then_some(*spatial_id))
        else {
            continue;
        };
        routes.insert(spatial_id, route);
    }
    routes
}

fn node_graph_wire_route_plan(wire: &WireProjection) -> Option<SpatialRoutePlan> {
    match wire.style {
        WireStyle::RightAngle => {
            if wire.to_pos.x < wire.from_pos.x {
                return None;
            }
            let stub = ((wire.to_pos.x - wire.from_pos.x) * 0.5) as f32;
            let from = RouteEndpoint {
                point: Point::new(wire.from_pos.x as f32, wire.from_pos.y as f32),
                dir: RouteDirection::Right,
            };
            let to = RouteEndpoint {
                point: Point::new(wire.to_pos.x as f32, wire.to_pos.y as f32),
                dir: RouteDirection::Left,
            };
            Some(SpatialRoutePlan::RightAngle(route_right_angle(
                from,
                to,
                RouteParams {
                    stub,
                    head_length: 0.0,
                },
            )))
        }
        WireStyle::Bezier | WireStyle::Straight => None,
    }
}

impl SpatialAdapter for NodeGraphSpatialAdapter {
    type DomainSnapshot = NodeGraphProjection;
    type SessionSnapshot = NodeGraphProjectionSession;
    type PolicyInput = SpatialVisualPolicy;
    type Context = CanvasSpatialBridge;
    type DomainIntent = CanvasIntent;

    fn build_projection(
        &self,
        domain: &Self::DomainSnapshot,
        _session: &Self::SessionSnapshot,
        viewport: Option<&SpatialViewportState>,
        orientation: Option<&SpatialAxisOrientation>,
        policy: &Self::PolicyInput,
    ) -> Result<SpatialProjection, SpatialProjectionError> {
        domain.spatial_projection(viewport.copied(), orientation.copied(), policy.clone())
    }

    fn translate_intent(
        &self,
        intent: &SpatialIntent,
        context: &Self::Context,
    ) -> Option<Self::DomainIntent> {
        match intent {
            SpatialIntent::Press(facts) => match facts.item {
                Some(_) => {
                    let hit = context.projection().hit_test(facts.surface_pos)?;
                    let entity = context.canvas_entity(hit.id)?;
                    Some(CanvasIntent::Hit(CanvasHit {
                        id: entity,
                        priority: node_graph_hit_priority(entity, hit.priority),
                        z_order: hit.z_order,
                    }))
                }
                None => Some(CanvasIntent::EmptyClick(facts.surface_pos)),
            },
            SpatialIntent::RubberBandChanged(rect) => Some(CanvasIntent::RubberBandChanged(*rect)),
            SpatialIntent::RubberBandCommitted(rect) => {
                Some(CanvasIntent::RubberBandCommitted(*rect))
            }
            SpatialIntent::ViewportChanged(viewport) => {
                let canvas_viewport = context.canvas_viewport_state(*viewport).ok()?;
                Some(CanvasIntent::ViewportChanged(canvas_viewport.transform))
            }
            SpatialIntent::HoverChanged { .. }
            | SpatialIntent::DragStarted(_)
            | SpatialIntent::DragUpdated { .. }
            | SpatialIntent::DragCommitted(_)
            | SpatialIntent::DragCanceled
            | SpatialIntent::KeyCommand(_) => None,
        }
    }
}

impl NodeProjection {
    pub fn size(&self) -> (f64, f64) {
        self.cached_size
    }

    pub fn bounds(&self) -> WorldRect {
        WorldRect::new(
            self.position.x,
            self.position.y,
            self.cached_size.0,
            self.cached_size.1,
        )
    }

    pub fn center(&self) -> WorldPoint {
        self.bounds().center()
    }
}

impl FrameProjection {
    pub fn bounds(&self) -> WorldRect {
        WorldRect::new(self.position.x, self.position.y, self.size.0, self.size.1)
    }
}

fn node_graph_hit_priority(entity: CanvasEntityId, fallback: SpatialHitPriority) -> HitPriority {
    match entity {
        CanvasEntityId::Node(_) => HitPriority::Node,
        CanvasEntityId::Port { .. } => HitPriority::Port,
        CanvasEntityId::Wire(_) => HitPriority::Wire,
        CanvasEntityId::Frame(_) => HitPriority::Frame,
        CanvasEntityId::Annotation(_) | CanvasEntityId::Custom(_) => match fallback {
            SpatialHitPriority::Background => HitPriority::Background,
            SpatialHitPriority::Container => HitPriority::Frame,
            SpatialHitPriority::Route => HitPriority::Wire,
            SpatialHitPriority::Item | SpatialHitPriority::Anchor => HitPriority::Node,
            SpatialHitPriority::Overlay => HitPriority::Overlay,
        },
    }
}

pub fn wire_path(style: WireStyle, from: WorldPoint, to: WorldPoint) -> Path {
    match style {
        WireStyle::Bezier => bezier_wire_path(from, to),
        WireStyle::Straight => straight_wire_path(from, to),
        WireStyle::RightAngle => right_angle_wire_path(from, to),
    }
}

fn build_node_projections(
    document: &NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    config: &NodeGraphProjectionConfig,
    state: &NodeGraphProjectionState,
) -> Result<Vec<NodeProjection>, NodeGraphProjectionError> {
    let mut projections = Vec::new();
    for graph_node in document.nodes() {
        let node_type =
            catalog
                .get(graph_node.type_id)
                .ok_or(NodeGraphProjectionError::UnknownNodeType {
                    node_id: graph_node.id,
                    type_id: graph_node.type_id,
                })?;
        let offset = state
            .node_offsets
            .get(&graph_node.id)
            .copied()
            .unwrap_or((0.0, 0.0));
        projections.push(project_node(
            document,
            registry,
            node_type,
            graph_node.id,
            (
                graph_node.position.0 + offset.0,
                graph_node.position.1 + offset.1,
            ),
            graph_node.title.clone(),
            graph_node.collapsed,
            *state
                .node_states
                .get(&graph_node.id)
                .unwrap_or(&NodeVisualState::Default),
            &config.geometry,
        ));
    }
    Ok(projections)
}

fn project_node(
    document: &NodeGraphDocument,
    registry: &PortTypeRegistry,
    node_type: &NodeType,
    node_id: u64,
    position: (f64, f64),
    title: String,
    collapsed: bool,
    state: NodeVisualState,
    geometry: &NodeGraphProjectionGeometry,
) -> NodeProjection {
    let port_rows = node_type
        .input_ports
        .len()
        .max(node_type.output_ports.len()) as f64;
    let body_height = if collapsed { 0.0 } else { geometry.body_height };
    let height = geometry.title_bar_height + port_rows * geometry.port_row_height + body_height;
    let position = WorldPoint::new(position.0, position.1);

    let inputs = project_ports(
        document,
        registry,
        node_id,
        &node_type.input_ports,
        position,
        PortDirection::Input,
        geometry,
    );
    let outputs = project_ports(
        document,
        registry,
        node_id,
        &node_type.output_ports,
        position,
        PortDirection::Output,
        geometry,
    );

    NodeProjection {
        id: node_id,
        type_id: node_type.type_id,
        position,
        title,
        color: node_type.default_color,
        collapsed,
        state,
        inputs,
        outputs,
        body_height,
        cached_size: (geometry.node_width, height),
    }
}

fn project_ports(
    document: &NodeGraphDocument,
    registry: &PortTypeRegistry,
    node_id: u64,
    ports: &[PortDefinition],
    node_position: WorldPoint,
    direction: PortDirection,
    geometry: &NodeGraphProjectionGeometry,
) -> Vec<PortProjection> {
    ports
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            let x = match direction {
                PortDirection::Input => node_position.x,
                PortDirection::Output => node_position.x + geometry.node_width,
            };
            let y = node_position.y
                + geometry.title_bar_height
                + (index as f64 + 0.5) * geometry.port_row_height;
            PortProjection {
                node_id,
                definition: definition.clone(),
                connection_count: document
                    .connections_for_port(node_id, definition.port_id)
                    .len() as u32,
                world_pos: WorldPoint::new(x, y),
                color: registry.color(definition.type_id),
            }
        })
        .collect()
}

fn build_port_lookup(nodes: &[NodeProjection]) -> HashMap<PortKey, &PortProjection> {
    let mut lookup = HashMap::new();
    for node in nodes {
        for port in node.inputs.iter().chain(node.outputs.iter()) {
            lookup.insert(
                PortKey {
                    node_id: port.node_id,
                    port_id: port.definition.port_id,
                    direction: port.definition.direction,
                },
                port,
            );
        }
    }
    lookup
}

fn build_wire_projections(
    document: &NodeGraphDocument,
    ports: &HashMap<PortKey, &PortProjection>,
    hidden_nodes: &HashSet<u64>,
    registry: &PortTypeRegistry,
    config: &NodeGraphProjectionConfig,
    state: &NodeGraphProjectionState,
) -> Result<Vec<WireProjection>, NodeGraphProjectionError> {
    let mut wires = Vec::new();
    for connection in document.connections() {
        if hidden_nodes.contains(&connection.from_node)
            || hidden_nodes.contains(&connection.to_node)
        {
            continue;
        }
        let from = resolve_connection_port(
            connection,
            connection.from_node,
            connection.from_port,
            PortDirection::Output,
            ports,
        )?;
        let to = resolve_connection_port(
            connection,
            connection.to_node,
            connection.to_port,
            PortDirection::Input,
            ports,
        )?;
        let wire_state = *state
            .wire_states
            .get(&connection.id)
            .unwrap_or(&WireVisualState::Default);
        let color = Some(registry.color(from.definition.type_id));
        let path = wire_path(config.wire_style, from.world_pos, to.world_pos);
        wires.push(WireProjection {
            id: connection.id,
            from_node: connection.from_node,
            from_port: connection.from_port,
            to_node: connection.to_node,
            to_port: connection.to_port,
            color,
            style: config.wire_style,
            state: wire_state,
            from_pos: from.world_pos,
            to_pos: to.world_pos,
            path,
        });
    }
    Ok(wires)
}

fn resolve_connection_port<'a>(
    connection: &GraphConnection,
    node_id: u64,
    port_id: u32,
    direction: PortDirection,
    ports: &HashMap<PortKey, &'a PortProjection>,
) -> Result<&'a PortProjection, NodeGraphProjectionError> {
    ports
        .get(&PortKey {
            node_id,
            port_id,
            direction,
        })
        .copied()
        .ok_or(NodeGraphProjectionError::UnknownPort {
            connection_id: connection.id,
            node_id,
            port_id,
            direction,
        })
}

fn build_frame_projections(
    document: &NodeGraphDocument,
    nodes: &[NodeProjection],
    state: &NodeGraphProjectionState,
) -> Vec<FrameProjection> {
    let mut frames: Vec<FrameProjection> = document
        .frames()
        .map(|frame| project_frame(frame, state))
        .collect();
    assign_contained_nodes(&mut frames, nodes);
    frames
}

fn project_frame(frame: &GraphFrame, state: &NodeGraphProjectionState) -> FrameProjection {
    let offset = state
        .frame_offsets
        .get(&frame.id)
        .copied()
        .unwrap_or((0.0, 0.0));
    let size = state
        .frame_size_overrides
        .get(&frame.id)
        .copied()
        .unwrap_or(frame.size);
    FrameProjection {
        id: frame.id,
        title: frame.title.clone(),
        description: frame.description.clone(),
        color: Color::from_srgb(
            frame.color.0 as f32 / 255.0,
            frame.color.1 as f32 / 255.0,
            frame.color.2 as f32 / 255.0,
            1.0,
        ),
        position: WorldPoint::new(frame.position.0 + offset.0, frame.position.1 + offset.1),
        size,
        collapsed: frame.collapsed,
        contained_nodes: Vec::new(),
        state: *state
            .frame_states
            .get(&frame.id)
            .unwrap_or(&FrameVisualState::Default),
    }
}

fn assign_contained_nodes(frames: &mut [FrameProjection], nodes: &[NodeProjection]) {
    for node in nodes {
        let center = node.center();
        let mut containing_frames: Vec<(usize, f64)> = frames
            .iter()
            .enumerate()
            .filter_map(|(index, frame)| {
                let bounds = frame.bounds();
                bounds
                    .contains(center)
                    .then_some((index, frame.size.0 * frame.size.1))
            })
            .collect();
        containing_frames.sort_by(|left, right| left.1.total_cmp(&right.1));
        if let Some((index, _)) = containing_frames.first() {
            frames[*index].contained_nodes.push(node.id);
        }
    }

    for frame in frames {
        frame.contained_nodes.sort_unstable();
    }
}

fn collapsed_frame_nodes(frames: &[FrameProjection]) -> HashSet<u64> {
    frames
        .iter()
        .filter(|frame| frame.collapsed)
        .flat_map(|frame| frame.contained_nodes.iter().copied())
        .collect()
}

fn append_frame_items(items: &mut Vec<ProjectedCanvasItem>, frames: &[FrameProjection]) {
    for frame in frames {
        let bounds = frame.bounds();
        items.push(ProjectedCanvasItem {
            id: CanvasEntityId::Frame(frame.id),
            bounds,
            z_order: FRAME_Z_ORDER,
            hit_regions: vec![CanvasHitRegion::Rect {
                id: CanvasEntityId::Frame(frame.id),
                rect: bounds,
                priority: HitPriority::Frame,
            }],
            paint: CanvasPaintDescriptor::Frame(frame.id),
        });
    }
}

fn append_wire_items(
    items: &mut Vec<ProjectedCanvasItem>,
    wires: &[WireProjection],
    geometry: &NodeGraphProjectionGeometry,
) {
    for wire in wires {
        let bounds = wire_bounds(wire.from_pos, wire.to_pos, geometry.wire_hit_tolerance);
        let z_order = match wire.state {
            WireVisualState::Hovered | WireVisualState::Selected | WireVisualState::Critical => {
                EMPHASIZED_WIRE_Z_ORDER
            }
            WireVisualState::Default | WireVisualState::Preview | WireVisualState::Dimmed => {
                WIRE_Z_ORDER
            }
        };
        items.push(ProjectedCanvasItem {
            id: CanvasEntityId::Wire(wire.id),
            bounds,
            z_order,
            hit_regions: vec![CanvasHitRegion::Path {
                id: CanvasEntityId::Wire(wire.id),
                path: wire.path.clone(),
                tolerance: geometry.wire_hit_tolerance,
                priority: HitPriority::Wire,
            }],
            paint: CanvasPaintDescriptor::Wire(wire.id),
        });
    }
}

fn append_node_items(
    items: &mut Vec<ProjectedCanvasItem>,
    nodes: &[NodeProjection],
    geometry: &NodeGraphProjectionGeometry,
) {
    for node in nodes {
        let bounds = node.bounds();
        items.push(ProjectedCanvasItem {
            id: CanvasEntityId::Node(node.id),
            bounds,
            z_order: NODE_Z_ORDER,
            hit_regions: vec![CanvasHitRegion::Rect {
                id: CanvasEntityId::Node(node.id),
                rect: bounds,
                priority: HitPriority::Node,
            }],
            paint: CanvasPaintDescriptor::Node(node.id),
        });

        for port in node.inputs.iter().chain(node.outputs.iter()) {
            let id = CanvasEntityId::Port {
                node_id: port.node_id,
                port_id: port.definition.port_id,
                direction: port.definition.direction,
            };
            let bounds = circle_bounds(port.world_pos, geometry.port_hit_radius);
            items.push(ProjectedCanvasItem {
                id,
                bounds,
                z_order: PORT_Z_ORDER,
                hit_regions: vec![CanvasHitRegion::Circle {
                    id,
                    center: port.world_pos,
                    radius: geometry.port_hit_radius,
                    priority: HitPriority::Port,
                }],
                paint: CanvasPaintDescriptor::Node(node.id),
            });
        }
    }
}

fn bezier_wire_path(from: WorldPoint, to: WorldPoint) -> Path {
    if to.x < from.x {
        return backward_bezier_wire_path(from, to);
    }

    let dx = (to.x - from.x).abs().max(50.0) * 0.5;
    let mut path = Path::new();
    path.move_to(from.x as f32, from.y as f32);
    path.cubic_to(
        (from.x + dx) as f32,
        from.y as f32,
        (to.x - dx) as f32,
        to.y as f32,
        to.x as f32,
        to.y as f32,
    );
    path
}

fn backward_bezier_wire_path(from: WorldPoint, to: WorldPoint) -> Path {
    let mid_y = (from.y + to.y) * 0.5 + 100.0;
    let dx = (from.x - to.x).abs().max(50.0) * 0.5;
    let mid_x = (from.x + to.x) * 0.5;
    let mut path = Path::new();
    path.move_to(from.x as f32, from.y as f32);
    path.cubic_to(
        (from.x + dx) as f32,
        from.y as f32,
        (mid_x + dx) as f32,
        mid_y as f32,
        mid_x as f32,
        mid_y as f32,
    );
    path.cubic_to(
        (mid_x - dx) as f32,
        mid_y as f32,
        (to.x - dx) as f32,
        to.y as f32,
        to.x as f32,
        to.y as f32,
    );
    path
}

fn straight_wire_path(from: WorldPoint, to: WorldPoint) -> Path {
    let mut path = Path::new();
    path.move_to(from.x as f32, from.y as f32);
    path.line_to(to.x as f32, to.y as f32);
    path
}

fn right_angle_wire_path(from: WorldPoint, to: WorldPoint) -> Path {
    let mid_x = (from.x + to.x) * 0.5;
    let mut path = Path::new();
    path.move_to(from.x as f32, from.y as f32);
    path.line_to(mid_x as f32, from.y as f32);
    path.line_to(mid_x as f32, to.y as f32);
    path.line_to(to.x as f32, to.y as f32);
    path
}

fn wire_bounds(from: WorldPoint, to: WorldPoint, tolerance: f64) -> WorldRect {
    let min_x = from.x.min(to.x) - tolerance;
    let min_y = from.y.min(to.y) - tolerance;
    let max_x = from.x.max(to.x) + tolerance;
    let max_y = from.y.max(to.y) + tolerance;
    WorldRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn circle_bounds(center: WorldPoint, radius: f64) -> WorldRect {
    WorldRect::new(
        center.x - radius,
        center.y - radius,
        radius * 2.0,
        radius * 2.0,
    )
}
