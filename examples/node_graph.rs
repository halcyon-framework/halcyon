//! SPEC-HAL-293/294: Node graph model and node flow surface demo.
//!
//! Run with:
//!   cargo run --example node_graph

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{
    CursorIcon, Event, EventPhase, FusedInputEvent, PointerButton, PointerEvent, PointerEventKind,
};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::canvas::{
    CanvasEntityId, CanvasProjection, CanvasTransform, CanvasViewportState, WorldPoint, WorldRect,
};
use halcyon::model::node_editor::{
    ExecutionOverlay, NodeEditorIntent, NodeEditorSession, NodeFlowConfig, NodeFlowInputs,
    NodeFlowView, NodeSearchIntent, PortEndpoint, SearchCursorDelta, SelectionIntent,
    SessionIntent, WireDragOrigin,
};
use halcyon::model::node_graph::{
    GraphDelta, GraphTransaction, GraphTransactionContext, NodeGraphDocument, NodeType,
    NodeTypeCatalog, PortCardinality, PortDefinition, PortDirection, PortTypeId, PortTypeInfo,
    PortTypeRegistry,
};
use halcyon::model::node_graph_projection::{
    FrameProjection, FrameVisualState, NodeGraphProjection, NodeProjection, NodeVisualState,
    PortProjection, WireProjection, WireVisualState,
};
use halcyon::model::spatial::{SpatialInteractionPolicy, SpatialWheelZoomPolicy};
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

const WIDTH: f32 = 1180.0;
const HEIGHT: f32 = 760.0;

fn spatial_interaction_policy() -> SpatialInteractionPolicy {
    SpatialInteractionPolicy {
        wheel_zoom: SpatialWheelZoomPolicy::Unmodified,
        ..SpatialInteractionPolicy::default()
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Node Graph Demo").with_size(WIDTH, HEIGHT));
    app.run(NodeGraphDemoScreen::new());
}

struct NodeGraphDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl NodeGraphDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for NodeGraphDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(NodeGraphDemoWidget::new()));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

struct NodeGraphDemoWidget {
    id: WidgetInternalId,
    document: NodeGraphDocument,
    catalog: NodeTypeCatalog,
    registry: PortTypeRegistry,
    session: NodeEditorSession,
    view: NodeFlowView,
    overlay: ExecutionOverlay,
    selected_node: u64,
    status: String,
    interaction: DemoInteraction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DemoInteraction {
    Idle,
    MovingSelection,
    Panning {
        start_local: (f32, f32),
        start_offset: (f64, f64),
    },
    RubberBand,
    Wiring,
}

impl NodeGraphDemoWidget {
    fn new() -> Self {
        let (catalog, registry) = demo_catalog_and_registry();
        let mut document = NodeGraphDocument::new(900, "node graph demo");

        let frame_inputs = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Inputs",
            (44.0, 56.0),
            (272.0, 352.0),
            (62, 116, 204),
        );
        let frame_processing = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Processing",
            (330.0, 80.0),
            (472.0, 304.0),
            (71, 166, 123),
        );
        let source_a = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 116.0),
            Some("Wave A"),
        );
        let source_b = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 276.0),
            Some("Wave B"),
        );
        let multiply = add_node(
            &mut document,
            &catalog,
            &registry,
            20,
            (390.0, 184.0),
            Some("Multiply"),
        );
        let threshold = add_node(
            &mut document,
            &catalog,
            &registry,
            30,
            (642.0, 184.0),
            Some("Threshold"),
        );
        let output = add_node(
            &mut document,
            &catalog,
            &registry,
            40,
            (886.0, 184.0),
            Some("Output"),
        );

        let wire_a = add_connection(&mut document, &catalog, &registry, source_a, 0, multiply, 0);
        let wire_b = add_connection(&mut document, &catalog, &registry, source_b, 0, multiply, 1);
        let wire_mul = add_connection(
            &mut document,
            &catalog,
            &registry,
            multiply,
            2,
            threshold,
            0,
        );
        let wire_out = add_connection(&mut document, &catalog, &registry, threshold, 1, output, 0);

        let _ = (frame_inputs, frame_processing, wire_a, wire_b, wire_out);

        let mut session = NodeEditorSession::new();
        session.viewport = CanvasViewportState {
            transform: CanvasTransform {
                offset_x: 0.0,
                offset_y: 0.0,
                zoom: 1.0,
            },
        };
        session.selection.replace(CanvasEntityId::Node(multiply));

        let mut overlay = ExecutionOverlay::default();
        overlay
            .node_states
            .insert(source_a, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(source_b, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(multiply, NodeVisualState::Running);
        overlay
            .wire_states
            .insert(wire_mul, WireVisualState::Critical);

        Self {
            id: WidgetInternalId::new(),
            document,
            catalog,
            registry,
            session,
            view: NodeFlowView::new()
                .with_config(NodeFlowConfig::default())
                .with_viewport_size(WIDTH as f64, HEIGHT as f64),
            overlay,
            selected_node: multiply,
            status: "Ready".to_string(),
            interaction: DemoInteraction::Idle,
        }
    }

    fn inputs(&self) -> NodeFlowInputs<'_> {
        NodeFlowInputs {
            document: &self.document,
            session: &self.session,
            catalog: &self.catalog,
            port_registry: &self.registry,
            execution_overlay: Some(&self.overlay),
        }
    }

    fn current_projection(&self) -> Option<NodeGraphProjection> {
        self.view.build_node_graph_projection(self.inputs()).ok()
    }

    fn world_from_local(&self, local: (f32, f32)) -> WorldPoint {
        self.session
            .viewport
            .transform
            .screen_to_world(local.0, local.1)
    }

    fn hit_port(
        &self,
        projection: &NodeGraphProjection,
        world: WorldPoint,
    ) -> Option<PortEndpoint> {
        let radius = 12.0 / self.session.viewport.transform.zoom.max(0.25);
        let radius_sq = radius * radius;
        let mut hit = None;

        for port in projection.ports() {
            let dx = world.x - port.world_pos.x;
            let dy = world.y - port.world_pos.y;
            if dx * dx + dy * dy <= radius_sq {
                hit = Some(PortEndpoint {
                    node_id: port.node_id,
                    port_id: port.definition.port_id,
                    direction: port.definition.direction,
                });
            }
        }

        hit
    }

    fn hit_node(&self, projection: &NodeGraphProjection, world: WorldPoint) -> Option<u64> {
        let mut hit = None;
        for node in projection.nodes() {
            if node.bounds().contains(world) {
                hit = Some(node.id);
            }
        }
        hit
    }

    fn hit_frame(&self, projection: &NodeGraphProjection, world: WorldPoint) -> Option<u64> {
        let mut hit = None;
        for frame in projection.frames() {
            if frame.bounds().contains(world) {
                hit = Some(frame.id);
            }
        }
        hit
    }

    fn hit_wire(&self, projection: &NodeGraphProjection, world: WorldPoint) -> Option<u64> {
        let tolerance = 8.0 / self.session.viewport.transform.zoom.max(0.25);
        let mut hit = None;

        for wire in projection.wires() {
            let points = wire.path.flatten();
            if points
                .windows(2)
                .any(|segment| distance_to_segment(world, segment[0], segment[1]) <= tolerance)
            {
                hit = Some(wire.id);
            }
        }

        hit
    }

    fn endpoint_position(
        &self,
        projection: &NodeGraphProjection,
        endpoint: PortEndpoint,
    ) -> Option<WorldPoint> {
        projection.ports().find_map(|port| {
            if port.node_id == endpoint.node_id
                && port.definition.port_id == endpoint.port_id
                && port.definition.direction == endpoint.direction
            {
                Some(port.world_pos)
            } else {
                None
            }
        })
    }

    fn apply_graph_transaction(&mut self, transaction: GraphTransaction, label: &str) -> bool {
        let context = graph_context(&self.catalog, &self.registry);
        match self.document.apply(transaction, context) {
            Ok(commit) => {
                self.status = format!("{label}; revision {}", commit.revision.0);
                true
            }
            Err(error) => {
                self.status = format!("Rejected graph transaction: {error:?}");
                false
            }
        }
    }

    fn selection_contains(&self, entity: CanvasEntityId) -> bool {
        self.session.selection.entities.contains(&entity)
    }

    fn selected_count(&self) -> usize {
        self.session.selection.entities.len()
    }

    fn sync_frame_drag_captures(&mut self, projection: &NodeGraphProjection) {
        let Some(drag) = &mut self.session.drag else {
            return;
        };

        let frames: Vec<FrameProjection> = projection.frames().iter().cloned().collect();
        let mut captures = Vec::new();
        for entity in &drag.entities {
            let CanvasEntityId::Frame(frame_id) = *entity else {
                continue;
            };
            if let Some(frame) = frames.iter().find(|frame| frame.id == frame_id) {
                captures.push(halcyon::model::node_editor::FrameDragCapture {
                    frame_id,
                    node_ids: frame.contained_nodes.clone(),
                });
            }
        }
        captures.sort_by_key(|capture| capture.frame_id);
        drag.frame_captures = captures;
    }

    fn begin_node_drag(
        &mut self,
        projection: &NodeGraphProjection,
        node_id: u64,
        world: WorldPoint,
        additive: bool,
    ) -> bool {
        let entity = CanvasEntityId::Node(node_id);
        let already_selected = self.selection_contains(entity);
        if additive {
            self.session
                .apply_selection_intent(SelectionIntent::Toggle(entity), projection.canvas());
            if already_selected {
                self.interaction = DemoInteraction::Idle;
                self.status = format!("Deselected node {node_id}");
                return false;
            }
        } else if !already_selected {
            self.session
                .apply_selection_intent(SelectionIntent::Replace(entity), projection.canvas());
        }

        self.session.apply_selection_intent(
            SelectionIntent::BeginMove {
                anchor: world,
                primary: entity,
            },
            projection.canvas(),
        );
        self.sync_frame_drag_captures(projection);
        self.selected_node = node_id;
        self.interaction = DemoInteraction::MovingSelection;
        self.status = format!("Dragging {} selected item(s)", self.selected_count());
        true
    }

    fn begin_frame_drag(
        &mut self,
        projection: &NodeGraphProjection,
        frame_id: u64,
        world: WorldPoint,
        additive: bool,
    ) -> bool {
        let entity = CanvasEntityId::Frame(frame_id);
        let already_selected = self.selection_contains(entity);
        if additive {
            self.session
                .apply_selection_intent(SelectionIntent::Toggle(entity), projection.canvas());
            if already_selected {
                self.interaction = DemoInteraction::Idle;
                self.status = format!("Deselected frame {frame_id}");
                return false;
            }
        }

        let frames: Vec<FrameProjection> = projection.frames().iter().cloned().collect();
        if self.session.begin_frame_drag(world, frame_id, &frames) {
            self.interaction = DemoInteraction::MovingSelection;
            self.status = format!("Dragging {} selected item(s)", self.selected_count());
            true
        } else {
            false
        }
    }

    fn update_active_move(&mut self, world: WorldPoint) -> bool {
        let Some(drag) = self.session.drag.as_ref() else {
            return false;
        };
        let delta = (world.x - drag.anchor.x, world.y - drag.anchor.y);
        if let Some(projection) = self.current_projection() {
            self.session
                .apply_selection_intent(SelectionIntent::UpdateMove { delta }, projection.canvas());
        }
        self.status = format!("Move delta=({:.1}, {:.1})", delta.0, delta.1);
        true
    }

    fn commit_active_move(&mut self) -> bool {
        let Some(drag) = self.session.drag.clone() else {
            self.interaction = DemoInteraction::Idle;
            return false;
        };

        let transaction = if drag.delta.0.abs() < 0.01 && drag.delta.1.abs() < 0.01 {
            None
        } else {
            self.move_transaction_from_drag(&drag)
        };

        if let Some(projection) = self.current_projection() {
            self.session
                .apply_selection_intent(SelectionIntent::CommitMove, projection.canvas());
        } else {
            self.session.drag = None;
        }
        self.interaction = DemoInteraction::Idle;

        if let Some(transaction) = transaction {
            self.apply_graph_transaction(transaction, "Committed move")
        } else {
            self.status = "Selected".to_string();
            true
        }
    }

    fn move_transaction_from_drag(
        &self,
        drag: &halcyon::model::node_editor::DragSession,
    ) -> Option<GraphTransaction> {
        let mut transactions = Vec::new();
        let mut moved_node_ids = Vec::new();
        let mut moved_frame_ids = Vec::new();

        for capture in &drag.frame_captures {
            let frame_id = capture.frame_id;
            if !drag.entities.contains(&CanvasEntityId::Frame(frame_id))
                || moved_frame_ids.contains(&frame_id)
            {
                continue;
            }
            let Some(frame) = self.document.frame(frame_id) else {
                continue;
            };
            moved_frame_ids.push(frame_id);

            let node_moves = capture
                .node_ids
                .iter()
                .filter_map(|node_id| {
                    self.document.node(*node_id).map(|node| {
                        if !moved_node_ids.contains(node_id) {
                            moved_node_ids.push(*node_id);
                        }
                        (
                            *node_id,
                            (
                                node.position.0 + drag.delta.0,
                                node.position.1 + drag.delta.1,
                            ),
                        )
                    })
                })
                .collect();

            transactions.push(GraphTransaction::MoveFrameWithNodes {
                frame_id,
                new_pos: (
                    frame.position.0 + drag.delta.0,
                    frame.position.1 + drag.delta.1,
                ),
                node_moves,
            });
        }

        for entity in &drag.entities {
            let CanvasEntityId::Frame(frame_id) = *entity else {
                continue;
            };
            if moved_frame_ids.contains(&frame_id) {
                continue;
            }
            let Some(frame) = self.document.frame(frame_id) else {
                continue;
            };
            moved_frame_ids.push(frame_id);
            transactions.push(GraphTransaction::MoveFrame {
                frame_id,
                new_pos: (
                    frame.position.0 + drag.delta.0,
                    frame.position.1 + drag.delta.1,
                ),
            });
        }

        let node_moves: Vec<(u64, (f64, f64))> = drag
            .entities
            .iter()
            .filter_map(|entity| match *entity {
                CanvasEntityId::Node(node_id) if !moved_node_ids.contains(&node_id) => {
                    self.document.node(node_id).map(|node| {
                        moved_node_ids.push(node_id);
                        (
                            node_id,
                            (
                                node.position.0 + drag.delta.0,
                                node.position.1 + drag.delta.1,
                            ),
                        )
                    })
                }
                _ => None,
            })
            .collect();
        if !node_moves.is_empty() {
            transactions.push(GraphTransaction::MoveNodes { moves: node_moves });
        }

        match transactions.len() {
            0 => None,
            1 => transactions.into_iter().next(),
            _ => Some(GraphTransaction::Compound(transactions)),
        }
    }

    fn begin_wire_drag(&mut self, endpoint: PortEndpoint, world: WorldPoint) -> bool {
        match self
            .session
            .begin_wire_drag(&self.document, &self.catalog, endpoint, world)
        {
            Ok(()) => {
                self.interaction = DemoInteraction::Wiring;
                self.status = format!(
                    "Dragging wire from node {} port {}",
                    endpoint.node_id, endpoint.port_id
                );
                true
            }
            Err(error) => {
                self.status = format!("Rejected wire drag: {error:?}");
                false
            }
        }
    }

    fn update_wire_drag(&mut self, projection: &NodeGraphProjection, world: WorldPoint) -> bool {
        let snapped_port = self.hit_port(projection, world);
        match self.session.update_wire_drag(world, snapped_port) {
            Ok(()) => {
                self.status = if let Some(endpoint) = snapped_port {
                    format!(
                        "Wire target node {} port {}",
                        endpoint.node_id, endpoint.port_id
                    )
                } else {
                    "Dragging wire".to_string()
                };
                true
            }
            Err(error) => {
                self.status = format!("Rejected wire update: {error:?}");
                false
            }
        }
    }

    fn commit_wire_drag(&mut self, projection: &NodeGraphProjection, world: WorldPoint) -> bool {
        let target = self.hit_port(projection, world);
        self.interaction = DemoInteraction::Idle;

        match self
            .session
            .commit_wire_drag(&self.document, &self.catalog, &self.registry, target)
        {
            Ok(Some(transaction)) => self.apply_graph_transaction(transaction, "Committed wire"),
            Ok(None) => {
                self.status = "Wire drag cancelled".to_string();
                true
            }
            Err(error) => {
                self.status = format!("Rejected wire commit: {error:?}");
                false
            }
        }
    }

    fn zoom_at(&mut self, local: (f32, f32), delta_y: f32) {
        let transform = &mut self.session.viewport.transform;
        let before = transform.screen_to_world(local.0, local.1);
        let factor = if delta_y > 0.0 { 1.15 } else { 0.87 };
        transform.zoom = (transform.zoom * factor).clamp(0.35, 2.75);
        transform.offset_x = before.x - local.0 as f64 / transform.zoom;
        transform.offset_y = before.y - local.1 as f64 / transform.zoom;
        self.status = format!("Zoom {:.2}", transform.zoom);
    }

    fn begin_pan(&mut self, local: (f32, f32)) {
        self.interaction = DemoInteraction::Panning {
            start_local: local,
            start_offset: (
                self.session.viewport.transform.offset_x,
                self.session.viewport.transform.offset_y,
            ),
        };
        self.status = "Panning canvas".to_string();
    }

    fn begin_rubber_band(&mut self, world: WorldPoint, additive: bool) {
        self.session.begin_rubber_band(world, additive);
        self.interaction = DemoInteraction::RubberBand;
        self.status = if additive {
            "Box selecting additively".to_string()
        } else {
            "Box selecting".to_string()
        };
    }

    fn update_rubber_band(&mut self, world: WorldPoint) -> bool {
        if self.session.rubber_band.is_none() {
            return false;
        }
        self.session.update_rubber_band(world);
        if let Some(rubber_band) = &self.session.rubber_band {
            let rect = normalized_world_rect(rubber_band.start, rubber_band.current);
            self.status = format!("Box {:.0}x{:.0}", rect.width, rect.height);
        }
        true
    }

    fn commit_rubber_band(&mut self) -> bool {
        let Some(rubber_band) = self.session.rubber_band.clone() else {
            self.interaction = DemoInteraction::Idle;
            return false;
        };
        let rect = normalized_world_rect(rubber_band.start, rubber_band.current);
        if let Some(projection) = self.current_projection() {
            self.session.apply_selection_intent(
                SelectionIntent::RubberBandCommit {
                    rect,
                    additive: rubber_band.additive,
                },
                projection.canvas(),
            );
            self.status = format!("Selected {} item(s)", self.selected_count());
        } else {
            self.session.rubber_band = None;
            self.status = "Box selection cancelled".to_string();
        }
        self.interaction = DemoInteraction::Idle;
        true
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, pe: &PointerEvent) -> bool {
        match pe.kind {
            PointerEventKind::Scroll { delta_y, .. } => {
                if delta_y == 0.0 {
                    return false;
                }
                if !spatial_interaction_policy().wheel_zoom_requested(pe.modifiers) {
                    return false;
                }
                self.zoom_at(pe.local_pos, delta_y);
                cx.invalidate_layout();
                true
            }
            PointerEventKind::Down if pe.button == Some(PointerButton::Auxiliary) => {
                cx.request_focus();
                cx.capture_pointer(PointerButton::Auxiliary);
                self.begin_pan(pe.local_pos);
                cx.invalidate_layout();
                cx.request_cursor(CursorIcon::Move);
                true
            }
            PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                cx.request_focus();
                cx.capture_pointer(PointerButton::Primary);

                let world = self.world_from_local(pe.local_pos);
                let Some(projection) = self.current_projection() else {
                    return false;
                };

                if let Some(endpoint) = self.hit_port(&projection, world) {
                    let handled = self.begin_wire_drag(endpoint, world);
                    if handled {
                        cx.invalidate_layout();
                    }
                    return handled;
                }

                if let Some(node_id) = self.hit_node(&projection, world) {
                    let moving = self.begin_node_drag(
                        &projection,
                        node_id,
                        world,
                        pe.modifiers.ctrl || pe.modifiers.meta,
                    );
                    cx.invalidate_layout();
                    if moving {
                        cx.request_cursor(CursorIcon::Move);
                    }
                    return true;
                }

                if let Some(frame_id) = self.hit_frame(&projection, world) {
                    let moving = self.begin_frame_drag(
                        &projection,
                        frame_id,
                        world,
                        pe.modifiers.ctrl || pe.modifiers.meta,
                    );
                    cx.invalidate_layout();
                    if moving {
                        cx.request_cursor(CursorIcon::Move);
                    }
                    return true;
                }

                if let Some(wire_id) = self.hit_wire(&projection, world) {
                    let entity = CanvasEntityId::Wire(wire_id);
                    let selection_intent = if pe.modifiers.ctrl || pe.modifiers.meta {
                        SelectionIntent::Toggle(entity)
                    } else {
                        SelectionIntent::Replace(entity)
                    };
                    self.session
                        .apply_selection_intent(selection_intent, projection.canvas());
                    self.status = format!("Selected {} item(s)", self.selected_count());
                    cx.invalidate_layout();
                    return true;
                }

                if pe.modifiers.shift {
                    self.begin_rubber_band(world, pe.modifiers.ctrl || pe.modifiers.meta);
                    cx.request_cursor(CursorIcon::Crosshair);
                } else {
                    self.session
                        .apply_selection_intent(SelectionIntent::Clear, projection.canvas());
                    self.begin_pan(pe.local_pos);
                    cx.request_cursor(CursorIcon::Move);
                }
                cx.invalidate_layout();
                true
            }
            PointerEventKind::Move => match self.interaction {
                DemoInteraction::MovingSelection => {
                    let world = self.world_from_local(pe.local_pos);
                    let handled = self.update_active_move(world);
                    if handled {
                        cx.invalidate_layout();
                        cx.request_cursor(CursorIcon::Move);
                    }
                    handled
                }
                DemoInteraction::Panning {
                    start_local,
                    start_offset,
                } => {
                    let zoom = self.session.viewport.transform.zoom;
                    self.session.viewport.transform.offset_x =
                        start_offset.0 - (pe.local_pos.0 - start_local.0) as f64 / zoom;
                    self.session.viewport.transform.offset_y =
                        start_offset.1 - (pe.local_pos.1 - start_local.1) as f64 / zoom;
                    self.status = format!(
                        "Pan offset=({:.1}, {:.1})",
                        self.session.viewport.transform.offset_x,
                        self.session.viewport.transform.offset_y
                    );
                    cx.invalidate_layout();
                    cx.request_cursor(CursorIcon::Move);
                    true
                }
                DemoInteraction::RubberBand => {
                    let world = self.world_from_local(pe.local_pos);
                    let handled = self.update_rubber_band(world);
                    if handled {
                        cx.invalidate_layout();
                        cx.request_cursor(CursorIcon::Crosshair);
                    }
                    handled
                }
                DemoInteraction::Wiring => {
                    let world = self.world_from_local(pe.local_pos);
                    let Some(projection) = self.current_projection() else {
                        return false;
                    };
                    let handled = self.update_wire_drag(&projection, world);
                    if handled {
                        cx.invalidate_layout();
                        cx.request_cursor(CursorIcon::Crosshair);
                    }
                    handled
                }
                DemoInteraction::Idle => {
                    let world = self.world_from_local(pe.local_pos);
                    let Some(projection) = self.current_projection() else {
                        return false;
                    };
                    if self.hit_port(&projection, world).is_some() {
                        cx.request_cursor(CursorIcon::Crosshair);
                        true
                    } else if self.hit_node(&projection, world).is_some()
                        || self.hit_frame(&projection, world).is_some()
                    {
                        cx.request_cursor(CursorIcon::Move);
                        true
                    } else if self.hit_wire(&projection, world).is_some() {
                        cx.request_cursor(CursorIcon::Pointer);
                        true
                    } else {
                        false
                    }
                }
            },
            PointerEventKind::Up
                if matches!(
                    pe.button,
                    Some(PointerButton::Primary | PointerButton::Auxiliary)
                ) =>
            {
                let handled = match self.interaction {
                    DemoInteraction::MovingSelection => self.commit_active_move(),
                    DemoInteraction::Panning { .. } => {
                        self.interaction = DemoInteraction::Idle;
                        self.status = "Ready".to_string();
                        true
                    }
                    DemoInteraction::RubberBand => self.commit_rubber_band(),
                    DemoInteraction::Wiring => {
                        let world = self.world_from_local(pe.local_pos);
                        let Some(projection) = self.current_projection() else {
                            self.session.cancel_wire_drag();
                            self.interaction = DemoInteraction::Idle;
                            return false;
                        };
                        self.commit_wire_drag(&projection, world)
                    }
                    DemoInteraction::Idle => false,
                };
                cx.release_pointer();
                cx.invalidate_layout();
                handled
            }
            _ => false,
        }
    }

    fn apply_intents(&mut self, intents: Vec<NodeEditorIntent>) {
        for intent in intents {
            match intent {
                NodeEditorIntent::Graph(transaction) => {
                    let context = graph_context(&self.catalog, &self.registry);
                    match self.document.apply(transaction, context) {
                        Ok(commit) => {
                            self.status = format!(
                                "Applied graph transaction; revision {}",
                                commit.revision.0
                            );
                        }
                        Err(error) => {
                            self.status = format!("Rejected graph transaction: {error:?}");
                        }
                    }
                }
                NodeEditorIntent::Session(intent) => self.apply_session_intent(intent),
                NodeEditorIntent::Undo => {
                    self.status = if self.document.undo() {
                        format!("Undo -> revision {}", self.document.revision().0)
                    } else {
                        "Nothing to undo".to_string()
                    };
                }
                NodeEditorIntent::Redo => {
                    self.status = if self.document.redo() {
                        format!("Redo -> revision {}", self.document.revision().0)
                    } else {
                        "Nothing to redo".to_string()
                    };
                }
                NodeEditorIntent::ContextMenu(_) => {
                    self.status = "Context menu intent emitted".to_string();
                }
            }
        }
    }

    fn apply_session_intent(&mut self, intent: SessionIntent) {
        match intent {
            SessionIntent::Selection(selection) => {
                if let Some(projection) = self.view.build_projection(self.inputs()).ok() {
                    self.session.apply_selection_intent(selection, &projection);
                    self.status = "Session selection intent applied".to_string();
                }
            }
            SessionIntent::Search(search) => self.apply_search_intent(search),
            SessionIntent::BeginRubberBand { start, additive } => {
                self.session.begin_rubber_band(start, additive);
                self.status = "Rubber-band selection started".to_string();
            }
            SessionIntent::UpdateRubberBand { current } => {
                self.session.update_rubber_band(current);
                self.status = "Rubber-band selection updated".to_string();
            }
            SessionIntent::CancelRubberBand => {
                self.session.cancel_rubber_band();
                self.status = "Rubber-band selection cancelled".to_string();
            }
            SessionIntent::BeginWireDrag { endpoint, cursor } => {
                let _ =
                    self.session
                        .begin_wire_drag(&self.document, &self.catalog, endpoint, cursor);
                self.status = "Wire drag session started".to_string();
            }
            SessionIntent::UpdateWireDrag {
                cursor,
                snapped_port,
            } => {
                let _ = self.session.update_wire_drag(cursor, snapped_port);
                self.status = "Wire drag session updated".to_string();
            }
            SessionIntent::CancelWireDrag => {
                self.session.cancel_wire_drag();
                self.status = "Wire drag session cancelled".to_string();
            }
            SessionIntent::CancelFrameResize => {
                self.session.cancel_frame_resize();
                self.status = "Frame resize session cancelled".to_string();
            }
            SessionIntent::CancelInlineEdit => {
                self.session.cancel_inline_edit();
                self.status = "Inline edit session cancelled".to_string();
            }
            SessionIntent::SetViewport(viewport) => {
                self.session.viewport = viewport;
                self.status = format!(
                    "Viewport offset=({:.1}, {:.1}) zoom={:.2}",
                    viewport.transform.offset_x,
                    viewport.transform.offset_y,
                    viewport.transform.zoom
                );
            }
            SessionIntent::SetTool(tool) => {
                self.session.active_tool = tool;
                self.status = format!("Tool set to {tool:?}");
            }
        }
    }

    fn apply_search_intent(&mut self, intent: NodeSearchIntent) {
        match intent {
            NodeSearchIntent::Open {
                position,
                dangling_wire,
            } => {
                self.session.open_search(position, dangling_wire);
                self.status = format!("Search opened at ({:.1}, {:.1})", position.x, position.y);
            }
            NodeSearchIntent::UpdateQuery(query) => {
                self.session.update_search_query(query);
                self.status = "Search query updated".to_string();
            }
            NodeSearchIntent::MoveCursor(delta) => {
                let delta_name = match delta {
                    SearchCursorDelta::Previous => "previous",
                    SearchCursorDelta::Next => "next",
                };
                self.session.move_search_cursor(delta, 1);
                self.status = format!("Search cursor moved {delta_name}");
            }
            NodeSearchIntent::Close => {
                self.session.close_search();
                self.status = "Search closed".to_string();
            }
            NodeSearchIntent::CommitCreate { type_id, position } => {
                let transaction = GraphTransaction::AddNode {
                    type_id,
                    position: (position.x, position.y),
                    title: None,
                };
                let context = graph_context(&self.catalog, &self.registry);
                if self.document.apply(transaction, context).is_ok() {
                    self.session.close_search();
                    self.status = "Search created a node".to_string();
                }
            }
            NodeSearchIntent::CommitCreateAndConnect {
                type_id, position, ..
            } => {
                let transaction = GraphTransaction::AddNode {
                    type_id,
                    position: (position.x, position.y),
                    title: None,
                };
                let context = graph_context(&self.catalog, &self.registry);
                if self.document.apply(transaction, context).is_ok() {
                    self.session.close_search();
                    self.status =
                        "Search created a node; dangling wire kept for owner routing".to_string();
                }
            }
        }
    }

    fn paint_graph(&self, cx: &mut PaintContext, rect: RectF, projection: &NodeGraphProjection) {
        paint_grid(cx, rect, &self.session.viewport.transform);

        for frame in projection.frames() {
            paint_frame(cx, rect, &self.session.viewport.transform, frame);
        }
        for wire in projection.wires() {
            paint_wire(cx, rect, &self.session.viewport.transform, wire);
        }
        for node in projection.nodes() {
            paint_node(cx, rect, &self.session.viewport.transform, node);
        }
        for port in projection.ports() {
            paint_port(cx, rect, &self.session.viewport.transform, port);
        }
        if let Some(rubber_band) = &self.session.rubber_band {
            paint_rubber_band(
                cx,
                rect,
                &self.session.viewport.transform,
                normalized_world_rect(rubber_band.start, rubber_band.current),
            );
        }
        if let Some(wire_drag) = &self.session.wire_drag {
            if let Some(origin) = wire_drag_origin_position(self, projection, &wire_drag.origin) {
                paint_wire_preview(
                    cx,
                    rect,
                    &self.session.viewport.transform,
                    origin,
                    wire_drag.cursor,
                    wire_drag.snapped_port.is_some(),
                );
            }
        }

        self.paint_overlay_text(cx, rect, projection.canvas());
    }

    fn paint_overlay_text(&self, cx: &mut PaintContext, rect: RectF, canvas: &CanvasProjection) {
        let panel = RectF::new(rect.x + 22.0, rect.y + rect.height - 116.0, 760.0, 86.0);
        cx.scene.push_rounded_rect(
            panel,
            Color::from_srgb(0.05, 0.06, 0.08, 0.92),
            RoundedCorners::uniform(8.0),
            30,
            cx.scene_key,
        );
        cx.scene.push_border_rect(
            panel,
            Color::from_srgb(0.45, 0.50, 0.58, 0.55),
            1.0,
            31,
            cx.scene_key,
        );
        paint_text_probe(
            cx,
            (panel.x + 18.0, panel.y + 16.0),
            "Node Graph Demo",
            18.0,
            Color::WHITE,
            32,
        );
        let stats = format!(
            "nodes={} wires={} frames={} canvas_items={} selected_node={}",
            self.document.nodes().count(),
            self.document.connections().count(),
            self.document.frames().count(),
            canvas.iter().count(),
            self.selected_node
        );
        paint_text_probe(
            cx,
            (panel.x + 18.0, panel.y + 42.0),
            &stats,
            13.0,
            Color::from_srgb(0.78, 0.83, 0.90, 1.0),
            32,
        );
        paint_text_probe(
            cx,
            (panel.x + 18.0, panel.y + 63.0),
            &self.status,
            12.0,
            Color::from_srgb(0.72, 0.78, 0.86, 1.0),
            32,
        );
    }
}

impl WidgetDowncast for NodeGraphDemoWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NodeGraphDemoWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(constraints.constrain(Size::new(WIDTH, HEIGHT)))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(
            rect,
            Color::from_srgb(0.025, 0.030, 0.040, 1.0),
            0,
            cx.scene_key,
        );

        if let Some(projection) = self.current_projection() {
            self.paint_graph(cx, rect, &projection);
        } else {
            paint_text_probe(
                cx,
                (rect.x + 24.0, rect.y + 24.0),
                "Node graph projection failed",
                18.0,
                Color::from_srgb(1.0, 0.25, 0.25, 1.0),
                10,
            );
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if let Event::Pointer(pointer) = event {
            if self.handle_pointer_event(cx, pointer) {
                return true;
            }
        }

        let inputs = NodeFlowInputs {
            document: &self.document,
            session: &self.session,
            catalog: &self.catalog,
            port_registry: &self.registry,
            execution_overlay: Some(&self.overlay),
        };
        self.view.handle_event(inputs, event);
        let intents = self.view.take_intents();
        if intents.is_empty() {
            return false;
        }

        self.apply_intents(intents);
        cx.invalidate_layout();
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "NodeGraphDemoWidget"
    }
}

fn demo_catalog_and_registry() -> (NodeTypeCatalog, PortTypeRegistry) {
    let number = PortTypeId(1);
    let boolean = PortTypeId(2);

    let mut registry = PortTypeRegistry::new();
    registry.register(PortTypeInfo {
        type_id: number,
        name: "Number".to_string(),
        color: Color::from_srgb(0.25, 0.68, 1.0, 1.0),
        compatible_with: vec![number],
    });
    registry.register(PortTypeInfo {
        type_id: boolean,
        name: "Boolean".to_string(),
        color: Color::from_srgb(0.94, 0.78, 0.22, 1.0),
        compatible_with: vec![boolean],
    });

    let mut catalog = NodeTypeCatalog::new();
    catalog.register(NodeType {
        type_id: 10,
        name: "Number Source".to_string(),
        category: "Input".to_string(),
        default_title: "Number Source".to_string(),
        default_color: Color::from_srgb(0.23, 0.50, 0.92, 1.0),
        input_ports: vec![],
        output_ports: vec![PortDefinition {
            port_id: 0,
            label: "out".to_string(),
            direction: PortDirection::Output,
            type_id: number,
            max_connections: PortCardinality::Multiple,
        }],
        resizable: false,
        collapsible: true,
    });
    catalog.register(NodeType {
        type_id: 20,
        name: "Multiply".to_string(),
        category: "Math".to_string(),
        default_title: "Multiply".to_string(),
        default_color: Color::from_srgb(0.18, 0.68, 0.48, 1.0),
        input_ports: vec![
            PortDefinition {
                port_id: 0,
                label: "a".to_string(),
                direction: PortDirection::Input,
                type_id: number,
                max_connections: PortCardinality::Single,
            },
            PortDefinition {
                port_id: 1,
                label: "b".to_string(),
                direction: PortDirection::Input,
                type_id: number,
                max_connections: PortCardinality::Single,
            },
        ],
        output_ports: vec![PortDefinition {
            port_id: 2,
            label: "out".to_string(),
            direction: PortDirection::Output,
            type_id: number,
            max_connections: PortCardinality::Multiple,
        }],
        resizable: false,
        collapsible: true,
    });
    catalog.register(NodeType {
        type_id: 30,
        name: "Threshold".to_string(),
        category: "Logic".to_string(),
        default_title: "Threshold".to_string(),
        default_color: Color::from_srgb(0.72, 0.45, 0.16, 1.0),
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "value".to_string(),
            direction: PortDirection::Input,
            type_id: number,
            max_connections: PortCardinality::Single,
        }],
        output_ports: vec![PortDefinition {
            port_id: 1,
            label: "passed".to_string(),
            direction: PortDirection::Output,
            type_id: boolean,
            max_connections: PortCardinality::Multiple,
        }],
        resizable: false,
        collapsible: true,
    });
    catalog.register(NodeType {
        type_id: 40,
        name: "Boolean Output".to_string(),
        category: "Output".to_string(),
        default_title: "Boolean Output".to_string(),
        default_color: Color::from_srgb(0.56, 0.44, 0.84, 1.0),
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "in".to_string(),
            direction: PortDirection::Input,
            type_id: boolean,
            max_connections: PortCardinality::Single,
        }],
        output_ports: vec![],
        resizable: false,
        collapsible: true,
    });

    (catalog, registry)
}

fn graph_context<'a>(
    catalog: &'a NodeTypeCatalog,
    registry: &'a PortTypeRegistry,
) -> GraphTransactionContext<'a> {
    GraphTransactionContext {
        catalog,
        port_registry: registry,
        validator: None,
    }
}

fn add_node(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    type_id: u64,
    position: (f64, f64),
    title: Option<&str>,
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddNode {
                type_id,
                position,
                title: title.map(str::to_string),
            },
            graph_context(catalog, registry),
        )
        .expect("demo node transaction must be valid");
    match commit.delta {
        GraphDelta::AddNode { node } => node.id,
        _ => unreachable!("AddNode transaction must produce AddNode delta"),
    }
}

fn add_connection(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    from_node: u64,
    from_port: u32,
    to_node: u64,
    to_port: u32,
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddConnection {
                from_node,
                from_port,
                to_node,
                to_port,
            },
            graph_context(catalog, registry),
        )
        .expect("demo connection transaction must be valid");
    match commit.delta {
        GraphDelta::AddConnection { connection } => connection.id,
        _ => unreachable!("AddConnection transaction must produce AddConnection delta"),
    }
}

fn add_frame(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    title: &str,
    position: (f64, f64),
    size: (f64, f64),
    color: (u8, u8, u8),
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddFrame {
                title: title.to_string(),
                position,
                size,
                color,
            },
            graph_context(catalog, registry),
        )
        .expect("demo frame transaction must be valid");
    match commit.delta {
        GraphDelta::AddFrame { frame } => frame.id,
        _ => unreachable!("AddFrame transaction must produce AddFrame delta"),
    }
}

fn paint_grid(cx: &mut PaintContext, rect: RectF, transform: &CanvasTransform) {
    let minor = Color::from_srgb(0.12, 0.14, 0.18, 0.42);
    let major = Color::from_srgb(0.18, 0.21, 0.27, 0.55);
    if transform.zoom <= 0.0 {
        return;
    }

    let world_step = 32.0;
    let spacing = (world_step * transform.zoom) as f32;
    if spacing < 8.0 {
        return;
    }

    let world_left = transform.offset_x;
    let world_right = transform.offset_x + rect.width as f64 / transform.zoom;
    let first_x = (world_left / world_step).floor() as i64 - 1;
    let last_x = (world_right / world_step).ceil() as i64 + 1;
    for grid_x in first_x..=last_x {
        let world_x = grid_x as f64 * world_step;
        let x = stable_grid_line(rect.x + ((world_x - transform.offset_x) * transform.zoom) as f32);
        if x < rect.x - 1.0 || x > rect.x + rect.width + 1.0 {
            continue;
        }
        let color = if grid_x.rem_euclid(4) == 0 {
            major
        } else {
            minor
        };
        cx.scene.push_rect(
            RectF::new(x, rect.y, 1.0, rect.height),
            color,
            1,
            cx.scene_key,
        );
    }

    let world_top = transform.offset_y;
    let world_bottom = transform.offset_y + rect.height as f64 / transform.zoom;
    let first_y = (world_top / world_step).floor() as i64 - 1;
    let last_y = (world_bottom / world_step).ceil() as i64 + 1;
    for grid_y in first_y..=last_y {
        let world_y = grid_y as f64 * world_step;
        let y = stable_grid_line(rect.y + ((world_y - transform.offset_y) * transform.zoom) as f32);
        if y < rect.y - 1.0 || y > rect.y + rect.height + 1.0 {
            continue;
        }
        let color = if grid_y.rem_euclid(4) == 0 {
            major
        } else {
            minor
        };
        cx.scene.push_rect(
            RectF::new(rect.x, y, rect.width, 1.0),
            color,
            1,
            cx.scene_key,
        );
    }
}

fn stable_grid_line(position: f32) -> f32 {
    position.round()
}

fn paint_frame(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    frame: &FrameProjection,
) {
    let rect = world_rect_to_screen(
        origin,
        transform,
        WorldRect::new(
            frame.position.x,
            frame.position.y,
            frame.size.0,
            frame.size.1,
        ),
    );
    let border = match frame.state {
        FrameVisualState::Selected => Color::from_srgb(0.78, 0.88, 1.0, 0.95),
        FrameVisualState::Resizing => Color::from_srgb(0.92, 0.82, 0.36, 0.95),
        FrameVisualState::Hovered => Color::from_srgb(0.82, 0.88, 0.94, 0.75),
        FrameVisualState::Default => frame.color.with_alpha(0.62),
    };
    cx.scene.push_rounded_rect(
        rect,
        frame.color.with_alpha(0.10),
        RoundedCorners::uniform(10.0),
        2,
        cx.scene_key,
    );
    cx.scene
        .push_border_rect(rect, border, 1.5, 3, cx.scene_key);
    let title = format!("{} ({})", frame.title, frame.contained_nodes.len());
    paint_text_probe(
        cx,
        (rect.x + 14.0, rect.y + 10.0),
        &title,
        13.0,
        Color::from_srgb(0.82, 0.88, 0.94, 0.85),
        4,
    );
}

fn paint_wire(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    wire: &WireProjection,
) {
    let from = world_point_to_screen(origin, transform, wire.from_pos);
    let to = world_point_to_screen(origin, transform, wire.to_pos);
    let color = match wire.state {
        WireVisualState::Selected => Color::from_srgb(0.82, 0.90, 1.0, 1.0),
        WireVisualState::Critical => Color::from_srgb(1.0, 0.72, 0.20, 1.0),
        WireVisualState::Preview => Color::from_srgb(0.90, 0.95, 1.0, 0.70),
        WireVisualState::Hovered => Color::from_srgb(0.70, 0.82, 1.0, 0.90),
        WireVisualState::Dimmed => Color::from_srgb(0.42, 0.48, 0.55, 0.50),
        WireVisualState::Default => wire
            .color
            .unwrap_or_else(|| Color::from_srgb(0.55, 0.64, 0.78, 0.82)),
    };
    let mid_x = (from.x + to.x) * 0.5;
    cx.scene.push_polyline(
        vec![
            Point::new(from.x, from.y),
            Point::new(mid_x, from.y),
            Point::new(mid_x, to.y),
            Point::new(to.x, to.y),
        ],
        3.0,
        color,
        8,
        cx.scene_key,
    );
}

fn wire_drag_origin_position(
    widget: &NodeGraphDemoWidget,
    projection: &NodeGraphProjection,
    origin: &WireDragOrigin,
) -> Option<WorldPoint> {
    match origin {
        WireDragOrigin::Output(endpoint) | WireDragOrigin::Input(endpoint) => {
            widget.endpoint_position(projection, *endpoint)
        }
        WireDragOrigin::Reroute { output, .. } => widget.endpoint_position(projection, *output),
    }
}

fn paint_wire_preview(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    from_world: WorldPoint,
    to_world: WorldPoint,
    snapped: bool,
) {
    let from = world_point_to_screen(origin, transform, from_world);
    let to = world_point_to_screen(origin, transform, to_world);
    let mid_x = (from.x + to.x) * 0.5;
    let color = if snapped {
        Color::from_srgb(0.95, 0.82, 0.25, 0.95)
    } else {
        Color::from_srgb(0.85, 0.92, 1.0, 0.72)
    };
    cx.scene.push_polyline(
        vec![
            Point::new(from.x, from.y),
            Point::new(mid_x, from.y),
            Point::new(mid_x, to.y),
            Point::new(to.x, to.y),
        ],
        3.0,
        color,
        18,
        cx.scene_key,
    );
}

fn normalized_world_rect(start: WorldPoint, current: WorldPoint) -> WorldRect {
    let min_x = start.x.min(current.x);
    let min_y = start.y.min(current.y);
    let max_x = start.x.max(current.x);
    let max_y = start.y.max(current.y);
    WorldRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn paint_rubber_band(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    world_rect: WorldRect,
) {
    let rect = world_rect_to_screen(origin, transform, world_rect);
    cx.scene.push_rounded_rect(
        rect,
        Color::from_srgb(0.38, 0.66, 1.0, 0.14),
        RoundedCorners::uniform(3.0),
        20,
        cx.scene_key,
    );
    cx.scene.push_border_rect(
        rect,
        Color::from_srgb(0.56, 0.78, 1.0, 0.88),
        1.0,
        21,
        cx.scene_key,
    );
}

fn paint_node(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    node: &NodeProjection,
) {
    let rect = world_rect_to_screen(origin, transform, node.bounds());
    let selected = matches!(
        node.state,
        NodeVisualState::Selected | NodeVisualState::Running
    );
    let border = match node.state {
        NodeVisualState::Selected => Color::from_srgb(0.74, 0.86, 1.0, 1.0),
        NodeVisualState::Running => Color::from_srgb(1.0, 0.78, 0.22, 1.0),
        NodeVisualState::Completed => Color::from_srgb(0.35, 0.92, 0.64, 1.0),
        NodeVisualState::Error => Color::from_srgb(1.0, 0.26, 0.26, 1.0),
        NodeVisualState::Hovered => Color::from_srgb(0.86, 0.91, 1.0, 0.95),
        NodeVisualState::Default => Color::from_srgb(0.38, 0.44, 0.52, 0.88),
    };
    let radius = RoundedCorners::uniform(8.0);
    let title_h = 32.0 * transform.zoom as f32;
    let title_rect = RectF::new(rect.x, rect.y, rect.width, title_h);

    cx.scene.push_rounded_rect(
        rect,
        Color::from_srgb(0.09, 0.105, 0.13, 0.96),
        radius,
        10,
        cx.scene_key,
    );
    cx.scene.push_rect(
        title_rect,
        node.color.with_alpha(if selected { 0.95 } else { 0.78 }),
        11,
        cx.scene_key,
    );
    cx.scene.push_border_rect(
        rect,
        border,
        if selected { 2.0 } else { 1.0 },
        12,
        cx.scene_key,
    );
    paint_text_probe(
        cx,
        (rect.x + 14.0, rect.y + 9.0),
        &node.title,
        13.0,
        Color::WHITE,
        13,
    );
}

fn paint_port(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    port: &PortProjection,
) {
    let center = world_point_to_screen(origin, transform, port.world_pos);
    let radius = 6.0_f32.max((5.0 * transform.zoom as f32).min(9.0));
    let outline = if port.connection_count > 0 {
        Color::WHITE
    } else {
        Color::from_srgb(0.40, 0.46, 0.55, 1.0)
    };
    cx.scene
        .push_circle(center, radius + 2.0, outline, 16, cx.scene_key);
    cx.scene
        .push_circle(center, radius, port.color, 17, cx.scene_key);
}

fn distance_to_segment(point: WorldPoint, start: [f32; 2], end: [f32; 2]) -> f64 {
    let px = point.x;
    let py = point.y;
    let ax = start[0] as f64;
    let ay = start[1] as f64;
    let bx = end[0] as f64;
    let by = end[1] as f64;
    let abx = bx - ax;
    let aby = by - ay;
    let length_sq = abx * abx + aby * aby;

    if length_sq <= f64::EPSILON {
        let dx = px - ax;
        let dy = py - ay;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = (((px - ax) * abx + (py - ay) * aby) / length_sq).clamp(0.0, 1.0);
    let closest_x = ax + abx * t;
    let closest_y = ay + aby * t;
    let dx = px - closest_x;
    let dy = py - closest_y;
    (dx * dx + dy * dy).sqrt()
}

fn paint_text_probe(
    cx: &mut PaintContext,
    position: (f32, f32),
    text: &str,
    font_size: f32,
    color: Color,
    z_index: u32,
) {
    #[cfg(debug_assertions)]
    {
        cx.scene
            .push_text_probe(position, text, font_size, color, z_index, cx.scene_key);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (cx, position, text, font_size, color, z_index);
    }
}

fn world_point_to_screen(origin: RectF, transform: &CanvasTransform, point: WorldPoint) -> Point {
    let (x, y) = transform.world_to_screen(point);
    Point::new(origin.x + x, origin.y + y)
}

fn world_rect_to_screen(origin: RectF, transform: &CanvasTransform, rect: WorldRect) -> RectF {
    let top_left = world_point_to_screen(origin, transform, WorldPoint::new(rect.x, rect.y));
    RectF::new(
        top_left.x,
        top_left.y,
        (rect.width * transform.zoom) as f32,
        (rect.height * transform.zoom) as f32,
    )
}
