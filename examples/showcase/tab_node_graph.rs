use halcyon::core::app::ScreenFrameContext;
use halcyon::core::event::{
    CursorIcon, Event, EventPhase, KeyCode, KeyEvent, Modifiers, PointerButton, PointerEvent,
    PointerEventKind,
};
use halcyon::core::layout::{Constraints, FlexParams, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::theme::Theme;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::canvas::{
    CanvasEntityId, CanvasProjection, CanvasTransform, CanvasViewportState, WorldPoint, WorldRect,
};
use halcyon::model::node_editor::{
    AlignmentGuide, AlignmentGuideAxis, AlignmentGuideRole, AlignmentSnapConfig,
    AlignmentSnapResult, DEFAULT_FRAME_PADDING, ExecutionOverlay, NodeEditorIntent,
    NodeEditorSession, NodeFlowConfig, NodeFlowInputs, NodeFlowView, NodeSearchIntent,
    PortEndpoint, SearchCursorDelta, SelectionIntent, SessionIntent, WireDragOrigin,
    resolve_alignment_snap,
};
use halcyon::model::node_graph::{
    GraphDelta, GraphTransaction, GraphTransactionContext, NodeGraphDocument, NodeType,
    NodeTypeCatalog, PortCardinality, PortDefinition, PortDirection, PortTypeId, PortTypeInfo,
    PortTypeRegistry,
};
use halcyon::model::node_graph_projection::{
    FrameProjection, FrameVisualState, NodeGraphProjection, NodeProjection, NodeVisualState,
    PortProjection, WireProjection, WireStyle, WireVisualState, wire_path,
};
use halcyon::model::spatial::{
    CanvasSpatialBridge, SpatialCursorPolicy, SpatialGestureClass, SpatialGestureFacts,
    SpatialGridLineRole, SpatialGridPolicy, SpatialGuideRole, SpatialIntent,
    SpatialInteractionPolicy, SpatialOverlayAnchor, SpatialOverlayPlan, SpatialRoleVisual,
    SpatialVisualPolicy, realize_spatial_grid, resolve_spatial_overlay_rect,
};
use halcyon::utils::id::WidgetInternalId;

use super::ShowcaseScreen;

const WIDTH: f32 = 1180.0;
const HEIGHT: f32 = 760.0;
const DEMO_WORLD_LEFT: f64 = 44.0;
const DEMO_WORLD_TOP: f64 = 56.0;
const DEMO_WORLD_RIGHT: f64 = 1565.0;
const DEMO_WORLD_BOTTOM: f64 = 576.0;

impl ShowcaseScreen {
    pub(super) fn build_node_graph(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let node = cx
            .tree
            .add_child(parent, Box::new(NodeGraphDemoWidget::new()), None, 0);
        cx.tree.set_flex(node, FlexParams::flexible(1.0));
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
    viewport_fitted: bool,
    last_layout_size: Size,
    space_down: bool,
    hover_cursor: Option<CursorIcon>,
    last_pointer_local: Option<(f32, f32)>,
    alignment_snap: AlignmentSnapConfig,
    alignment_guides: Vec<AlignmentGuide>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DemoInteraction {
    Idle,
    MovingSelection,
    Panning {
        gesture: SpatialGestureClass,
        start_local: (f32, f32),
        start_offset: (f64, f64),
    },
    RubberBand,
    Wiring,
}

impl NodeGraphDemoWidget {
    fn new() -> Self {
        let (catalog, registry) = demo_catalog_and_registry();
        let mut document = NodeGraphDocument::new(900, "showcase node graph");

        let frame_inputs = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Market Inputs",
            (44.0, 56.0),
            (280.0, 520.0),
            (62, 116, 204),
        );
        let frame_features = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Feature Engineering",
            (360.0, 56.0),
            (430.0, 520.0),
            (71, 166, 123),
        );
        let frame_gates = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Decision Gates",
            (820.0, 92.0),
            (470.0, 420.0),
            (204, 146, 62),
        );
        let frame_outputs = add_frame(
            &mut document,
            &catalog,
            &registry,
            "Execution Outputs",
            (1285.0, 160.0),
            (280.0, 360.0),
            (138, 104, 206),
        );

        let price_feed = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 110.0),
            Some("Price"),
        );
        let volume_feed = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 205.0),
            Some("Volume"),
        );
        let volatility_feed = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 300.0),
            Some("Volatility"),
        );
        let liquidity_index = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 395.0),
            Some("Liquidity"),
        );
        let news_score = add_node(
            &mut document,
            &catalog,
            &registry,
            10,
            (86.0, 490.0),
            Some("News"),
        );

        let normalize_price = add_node(
            &mut document,
            &catalog,
            &registry,
            70,
            (400.0, 110.0),
            Some("Normalize"),
        );
        let volume_smooth = add_node(
            &mut document,
            &catalog,
            &registry,
            70,
            (400.0, 230.0),
            Some("Smooth Vol"),
        );
        let volatility_model = add_node(
            &mut document,
            &catalog,
            &registry,
            70,
            (400.0, 350.0),
            Some("Vol Model"),
        );
        let alpha_blend = add_node(
            &mut document,
            &catalog,
            &registry,
            20,
            (600.0, 170.0),
            Some("Alpha Blend"),
        );
        let risk_adjust = add_node(
            &mut document,
            &catalog,
            &registry,
            20,
            (600.0, 350.0),
            Some("Risk Adjust"),
        );
        let composite_score = add_node(
            &mut document,
            &catalog,
            &registry,
            20,
            (600.0, 470.0),
            Some("Composite"),
        );

        let trend_threshold = add_node(
            &mut document,
            &catalog,
            &registry,
            30,
            (850.0, 160.0),
            Some("Trend Gate"),
        );
        let risk_threshold = add_node(
            &mut document,
            &catalog,
            &registry,
            30,
            (850.0, 340.0),
            Some("Risk Gate"),
        );
        let signal_gate = add_node(
            &mut document,
            &catalog,
            &registry,
            50,
            (1085.0, 250.0),
            Some("Signal Gate"),
        );

        let strategy_output = add_node(
            &mut document,
            &catalog,
            &registry,
            40,
            (1325.0, 210.0),
            Some("Strategy"),
        );
        let alert_output = add_node(
            &mut document,
            &catalog,
            &registry,
            40,
            (1325.0, 330.0),
            Some("Alert"),
        );
        let audit_output = add_node(
            &mut document,
            &catalog,
            &registry,
            60,
            (1325.0, 450.0),
            Some("Audit"),
        );

        let wire_price = add_connection(
            &mut document,
            &catalog,
            &registry,
            price_feed,
            0,
            normalize_price,
            0,
        );
        let wire_volume = add_connection(
            &mut document,
            &catalog,
            &registry,
            volume_feed,
            0,
            volume_smooth,
            0,
        );
        let wire_volatility = add_connection(
            &mut document,
            &catalog,
            &registry,
            volatility_feed,
            0,
            volatility_model,
            0,
        );
        let wire_alpha_a = add_connection(
            &mut document,
            &catalog,
            &registry,
            normalize_price,
            1,
            alpha_blend,
            0,
        );
        let wire_alpha_b = add_connection(
            &mut document,
            &catalog,
            &registry,
            volume_smooth,
            1,
            alpha_blend,
            1,
        );
        let wire_risk_a = add_connection(
            &mut document,
            &catalog,
            &registry,
            volatility_model,
            1,
            risk_adjust,
            0,
        );
        let wire_risk_b = add_connection(
            &mut document,
            &catalog,
            &registry,
            liquidity_index,
            0,
            risk_adjust,
            1,
        );
        let wire_score_a = add_connection(
            &mut document,
            &catalog,
            &registry,
            alpha_blend,
            2,
            composite_score,
            0,
        );
        let wire_score_b = add_connection(
            &mut document,
            &catalog,
            &registry,
            news_score,
            0,
            composite_score,
            1,
        );
        let wire_trend = add_connection(
            &mut document,
            &catalog,
            &registry,
            composite_score,
            2,
            trend_threshold,
            0,
        );
        let wire_risk = add_connection(
            &mut document,
            &catalog,
            &registry,
            risk_adjust,
            2,
            risk_threshold,
            0,
        );
        let wire_gate_a = add_connection(
            &mut document,
            &catalog,
            &registry,
            trend_threshold,
            1,
            signal_gate,
            0,
        );
        let wire_gate_b = add_connection(
            &mut document,
            &catalog,
            &registry,
            risk_threshold,
            1,
            signal_gate,
            1,
        );
        let wire_strategy = add_connection(
            &mut document,
            &catalog,
            &registry,
            signal_gate,
            2,
            strategy_output,
            0,
        );
        let wire_alert = add_connection(
            &mut document,
            &catalog,
            &registry,
            trend_threshold,
            1,
            alert_output,
            0,
        );
        let wire_audit = add_connection(
            &mut document,
            &catalog,
            &registry,
            composite_score,
            2,
            audit_output,
            0,
        );

        let _ = (
            frame_inputs,
            frame_features,
            frame_gates,
            frame_outputs,
            wire_price,
            wire_volume,
            wire_volatility,
            wire_alpha_a,
            wire_alpha_b,
            wire_risk_a,
            wire_risk_b,
            wire_score_a,
            wire_score_b,
            wire_risk,
            wire_gate_a,
            wire_gate_b,
            wire_alert,
            wire_audit,
        );

        let mut session = NodeEditorSession::new();
        session.viewport = CanvasViewportState {
            transform: CanvasTransform {
                offset_x: 0.0,
                offset_y: 32.0,
                zoom: 0.55,
            },
        };
        session.selection.entities = vec![
            CanvasEntityId::Node(alpha_blend),
            CanvasEntityId::Node(risk_adjust),
            CanvasEntityId::Frame(frame_features),
        ];
        session.selection.anchor = Some(CanvasEntityId::Node(alpha_blend));

        let mut overlay = ExecutionOverlay::default();
        overlay
            .node_states
            .insert(price_feed, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(volume_feed, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(volatility_feed, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(normalize_price, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(volume_smooth, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(volatility_model, NodeVisualState::Completed);
        overlay
            .node_states
            .insert(composite_score, NodeVisualState::Running);
        overlay
            .node_states
            .insert(signal_gate, NodeVisualState::Running);
        overlay
            .wire_states
            .insert(wire_trend, WireVisualState::Critical);
        overlay
            .wire_states
            .insert(wire_strategy, WireVisualState::Critical);

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
            selected_node: alpha_blend,
            status: "Scroll zooms. Space-drag pans. Shift selects. Home/F fits.".to_string(),
            interaction: DemoInteraction::Idle,
            viewport_fitted: false,
            last_layout_size: Size::new(WIDTH, HEIGHT),
            space_down: false,
            hover_cursor: None,
            last_pointer_local: None,
            alignment_snap: AlignmentSnapConfig::default(),
            alignment_guides: Vec::new(),
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

    fn fit_viewport_to_demo(&mut self, size: Size) {
        let bounds = WorldRect {
            x: DEMO_WORLD_LEFT,
            y: DEMO_WORLD_TOP,
            width: DEMO_WORLD_RIGHT - DEMO_WORLD_LEFT,
            height: DEMO_WORLD_BOTTOM - DEMO_WORLD_TOP,
        };
        self.fit_viewport_to_bounds(size, bounds, 0.42, 0.82);
    }

    fn fit_viewport_to_graph(&mut self, size: Size) {
        let bounds = self
            .current_projection()
            .as_ref()
            .and_then(projection_content_bounds);
        if let Some(bounds) = bounds {
            self.fit_viewport_to_bounds(size, bounds, 0.25, 1.05);
        } else {
            self.fit_viewport_to_demo(size);
        }
    }

    fn fit_viewport_to_bounds(
        &mut self,
        size: Size,
        bounds: WorldRect,
        min_zoom: f64,
        max_zoom: f64,
    ) {
        let world_width = bounds.width.max(1.0);
        let world_height = bounds.height.max(1.0);
        let available_width = (size.width as f64 - 72.0).max(360.0);
        let available_height = (size.height as f64 - 118.0).max(260.0);
        let zoom = (available_width / world_width)
            .min(available_height / world_height)
            .clamp(min_zoom, max_zoom);

        self.session.viewport.transform = CanvasTransform {
            offset_x: bounds.x - 34.0 / zoom,
            offset_y: bounds.y - 22.0 / zoom,
            zoom,
        };
    }

    fn world_from_local(&self, local: (f32, f32)) -> WorldPoint {
        self.session
            .viewport
            .transform
            .screen_to_world(local.0, local.1)
    }

    fn spatial_interaction_policy(&self) -> SpatialInteractionPolicy {
        spatial_interaction_policy()
    }

    fn spatial_gesture_class(
        &self,
        modifiers: Modifiers,
        button: PointerButton,
    ) -> SpatialGestureClass {
        self.spatial_interaction_policy()
            .gesture_class(SpatialGestureFacts {
                modifiers,
                button,
                space_pressed: self.space_down,
            })
    }

    fn set_hover_cursor(&mut self, cx: &mut EventContext, cursor: CursorIcon) {
        self.hover_cursor = Some(cursor);
        cx.request_cursor(cursor);
    }

    fn clear_hover_cursor(&mut self) {
        self.hover_cursor = None;
    }

    fn request_stored_cursor(&self, cx: &mut EventContext) {
        if let Some(cursor) = self.hover_cursor {
            cx.request_cursor(cursor);
        }
    }

    fn refresh_hover_cursor(&mut self, cx: &mut EventContext, _world: WorldPoint) -> bool {
        self.clear_hover_cursor();
        cx.request_cursor(CursorIcon::Default);
        false
    }

    fn refresh_last_hover_cursor(&mut self, cx: &mut EventContext) -> bool {
        let Some(local) = self.last_pointer_local else {
            self.clear_hover_cursor();
            return false;
        };
        let world = self.world_from_local(local);
        self.refresh_hover_cursor(cx, world)
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, key: &KeyEvent) -> bool {
        match key.key_code {
            KeyCode::Space => {
                self.space_down = key.is_down;
                if key.is_down {
                    self.clear_hover_cursor();
                    cx.request_cursor(CursorIcon::Default);
                } else {
                    self.refresh_last_hover_cursor(cx);
                }
                true
            }
            _ if key.is_down && !key.is_repeat => {
                let Some(SpatialIntent::KeyCommand(command)) = self
                    .spatial_interaction_policy()
                    .key_intent(spatial_key_code(key.key_code))
                else {
                    return false;
                };

                match command {
                    KeyCode::Home | KeyCode::Character('f') => {
                        self.fit_viewport_to_graph(self.last_layout_size);
                        self.status = "Fit graph".to_string();
                        cx.invalidate_layout();
                        self.refresh_last_hover_cursor(cx);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
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
        let Some(drag) = self.session.drag.clone() else {
            return false;
        };
        let requested_delta = (world.x - drag.anchor.x, world.y - drag.anchor.y);
        if let Some(projection) = self.current_projection() {
            let snap = self.resolve_move_snap(&projection, &drag, requested_delta);
            let delta = snap.delta;
            self.alignment_guides = snap.guides;
            self.session
                .apply_selection_intent(SelectionIntent::UpdateMove { delta }, projection.canvas());
            self.status = if self.alignment_guides.is_empty() {
                format!("Move delta=({:.1}, {:.1})", delta.0, delta.1)
            } else {
                format!(
                    "Snapped delta=({:.1}, {:.1}) guides={}",
                    delta.0,
                    delta.1,
                    self.alignment_guides.len()
                )
            };
        } else {
            self.alignment_guides.clear();
            self.status = format!(
                "Move delta=({:.1}, {:.1})",
                requested_delta.0, requested_delta.1
            );
        }
        true
    }

    fn resolve_move_snap(
        &self,
        projection: &NodeGraphProjection,
        drag: &halcyon::model::node_editor::DragSession,
        requested_delta: (f64, f64),
    ) -> AlignmentSnapResult {
        let mut moving_nodes = Vec::new();
        let mut moving_frames = Vec::new();
        for entity in &drag.entities {
            match *entity {
                CanvasEntityId::Node(node_id) => moving_nodes.push(node_id),
                CanvasEntityId::Frame(frame_id) => moving_frames.push(frame_id),
                _ => {}
            }
        }
        for capture in &drag.frame_captures {
            for node_id in &capture.node_ids {
                if !moving_nodes.contains(node_id) {
                    moving_nodes.push(*node_id);
                }
            }
        }

        let mut moving = Vec::new();
        let mut stationary = Vec::new();
        for node in projection.nodes() {
            let bounds = translate_world_rect(node.bounds(), (-drag.delta.0, -drag.delta.1));
            if moving_nodes.contains(&node.id) {
                if drag.entities.contains(&CanvasEntityId::Node(node.id)) {
                    moving.push(bounds);
                }
            } else {
                stationary.push(node.bounds());
            }
        }
        for frame in projection.frames() {
            let bounds = translate_world_rect(frame.bounds(), (-drag.delta.0, -drag.delta.1));
            if moving_frames.contains(&frame.id) {
                moving.push(bounds);
            } else {
                let bounds = frame.bounds();
                stationary.push(bounds);
                if let Some(inner_bounds) = inset_world_rect(bounds, DEFAULT_FRAME_PADDING) {
                    stationary.push(inner_bounds);
                }
            }
        }

        resolve_alignment_snap(&moving, &stationary, requested_delta, self.alignment_snap)
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
        self.alignment_guides.clear();

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
                self.alignment_guides.clear();
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
        self.alignment_guides.clear();

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

    fn begin_pan(&mut self, local: (f32, f32), gesture: SpatialGestureClass) {
        self.alignment_guides.clear();
        self.interaction = DemoInteraction::Panning {
            gesture,
            start_local: local,
            start_offset: (
                self.session.viewport.transform.offset_x,
                self.session.viewport.transform.offset_y,
            ),
        };
        self.status = "Panning canvas".to_string();
    }

    fn begin_rubber_band(&mut self, world: WorldPoint, additive: bool) {
        self.alignment_guides.clear();
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
        self.last_pointer_local = Some(pe.local_pos);

        match pe.kind {
            PointerEventKind::Scroll { delta_y, .. } => {
                if delta_y == 0.0 {
                    return false;
                }
                let interaction_policy = self.spatial_interaction_policy();
                if !interaction_policy.wheel_zoom_requested(pe.modifiers) {
                    return false;
                }
                self.zoom_at(pe.local_pos, delta_y);
                let world = self.world_from_local(pe.local_pos);
                self.refresh_hover_cursor(cx, world);
                cx.invalidate_layout();
                true
            }
            PointerEventKind::Down if pe.button == Some(PointerButton::Auxiliary) => {
                let interaction_policy = self.spatial_interaction_policy();
                let gesture = self.spatial_gesture_class(pe.modifiers, PointerButton::Auxiliary);
                if !interaction_policy.is_pan_gesture(gesture) {
                    return false;
                }
                cx.request_focus();
                cx.capture_pointer(PointerButton::Auxiliary);
                self.begin_pan(pe.local_pos, gesture);
                cx.invalidate_layout();
                self.set_hover_cursor(
                    cx,
                    cursor_icon_for_spatial_policy(
                        interaction_policy.cursor_for_gesture(gesture, true),
                    ),
                );
                true
            }
            PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                let interaction_policy = self.spatial_interaction_policy();
                let gesture = self.spatial_gesture_class(pe.modifiers, PointerButton::Primary);
                cx.request_focus();
                cx.capture_pointer(PointerButton::Primary);

                if interaction_policy.is_pan_gesture(gesture) {
                    self.begin_pan(pe.local_pos, gesture);
                    cx.invalidate_layout();
                    self.set_hover_cursor(
                        cx,
                        cursor_icon_for_spatial_policy(
                            interaction_policy.cursor_for_gesture(gesture, true),
                        ),
                    );
                    return true;
                }

                let world = self.world_from_local(pe.local_pos);
                let Some(projection) = self.current_projection() else {
                    return false;
                };

                if let Some(endpoint) = self.hit_port(&projection, world) {
                    let handled = self.begin_wire_drag(endpoint, world);
                    if handled {
                        cx.invalidate_layout();
                        let route_gesture = if interaction_policy.is_route_creation_gesture(gesture)
                        {
                            gesture
                        } else {
                            interaction_policy.route_creation_gesture
                        };
                        self.set_hover_cursor(
                            cx,
                            cursor_icon_for_spatial_policy(
                                interaction_policy.cursor_for_route_creation(route_gesture, true),
                            ),
                        );
                    }
                    return handled;
                }

                if let Some(node_id) = self.hit_node(&projection, world) {
                    let moving = self.begin_node_drag(
                        &projection,
                        node_id,
                        world,
                        interaction_policy.selection_toggle_requested(pe.modifiers),
                    );
                    cx.invalidate_layout();
                    if moving {
                        self.set_hover_cursor(
                            cx,
                            cursor_icon_for_spatial_policy(
                                interaction_policy.cursor_for_gesture(gesture, true),
                            ),
                        );
                    }
                    return true;
                }

                if let Some(frame_id) = self.hit_frame(&projection, world) {
                    let moving = self.begin_frame_drag(
                        &projection,
                        frame_id,
                        world,
                        interaction_policy.selection_toggle_requested(pe.modifiers),
                    );
                    cx.invalidate_layout();
                    if moving {
                        self.set_hover_cursor(
                            cx,
                            cursor_icon_for_spatial_policy(
                                interaction_policy.cursor_for_gesture(gesture, true),
                            ),
                        );
                    }
                    return true;
                }

                if let Some(wire_id) = self.hit_wire(&projection, world) {
                    let entity = CanvasEntityId::Wire(wire_id);
                    let selection_intent =
                        if interaction_policy.selection_toggle_requested(pe.modifiers) {
                            SelectionIntent::Toggle(entity)
                        } else {
                            SelectionIntent::Replace(entity)
                        };
                    self.session
                        .apply_selection_intent(selection_intent, projection.canvas());
                    self.status = format!("Selected {} item(s)", self.selected_count());
                    cx.invalidate_layout();
                    self.clear_hover_cursor();
                    cx.request_cursor(CursorIcon::Default);
                    return true;
                }

                if interaction_policy.is_rubber_band_gesture(gesture) {
                    self.begin_rubber_band(
                        world,
                        interaction_policy.selection_toggle_requested(pe.modifiers),
                    );
                    self.set_hover_cursor(
                        cx,
                        cursor_icon_for_spatial_policy(
                            interaction_policy.cursor_for_gesture(gesture, true),
                        ),
                    );
                } else {
                    self.session
                        .apply_selection_intent(SelectionIntent::Clear, projection.canvas());
                    self.begin_pan(pe.local_pos, gesture);
                    self.set_hover_cursor(
                        cx,
                        cursor_icon_for_spatial_policy(
                            interaction_policy.cursor_for_gesture(gesture, true),
                        ),
                    );
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
                        self.set_hover_cursor(cx, CursorIcon::Move);
                    }
                    handled
                }
                DemoInteraction::Panning {
                    gesture,
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
                    self.set_hover_cursor(
                        cx,
                        cursor_icon_for_spatial_policy(
                            self.spatial_interaction_policy()
                                .cursor_for_gesture(gesture, true),
                        ),
                    );
                    true
                }
                DemoInteraction::RubberBand => {
                    let world = self.world_from_local(pe.local_pos);
                    let handled = self.update_rubber_band(world);
                    if handled {
                        cx.invalidate_layout();
                        let interaction_policy = self.spatial_interaction_policy();
                        self.set_hover_cursor(
                            cx,
                            cursor_icon_for_spatial_policy(
                                interaction_policy.cursor_for_gesture(
                                    interaction_policy.rubber_band_gesture,
                                    true,
                                ),
                            ),
                        );
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
                        let interaction_policy = self.spatial_interaction_policy();
                        self.set_hover_cursor(
                            cx,
                            cursor_icon_for_spatial_policy(
                                interaction_policy.cursor_for_route_creation(
                                    interaction_policy.route_creation_gesture,
                                    true,
                                ),
                            ),
                        );
                    }
                    handled
                }
                DemoInteraction::Idle => {
                    let world = self.world_from_local(pe.local_pos);
                    self.refresh_hover_cursor(cx, world)
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
                let world = self.world_from_local(pe.local_pos);
                self.refresh_hover_cursor(cx, world);
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
        paint_alignment_guides(
            cx,
            rect,
            &self.session.viewport.transform,
            &self.alignment_guides,
        );
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
        let policy = spatial_visual_policy(cx.theme);
        let panel_height = 58.0;
        let panel_width = (rect.width - policy.hud_edge_padding * 2.0).clamp(360.0, 520.0);
        let panel = resolve_spatial_overlay_rect(
            rect,
            SpatialOverlayPlan {
                anchor: SpatialOverlayAnchor::BottomLeft,
                offset: Point::zero(),
                size: RectF::new(0.0, 0.0, panel_width, panel_height),
                edge_padding: policy.hud_edge_padding,
            },
        );
        cx.scene.push_rounded_rect(
            panel,
            policy.hud_background,
            RoundedCorners::uniform(8.0),
            30,
            cx.scene_key,
        );
        cx.scene
            .push_border_rect(panel, policy.hud_border, 1.0, 31, cx.scene_key);
        paint_graph_text(
            cx,
            (panel.x + 14.0, panel.y + 14.0),
            "Node Graph Demo",
            13.0,
            policy.hud_text,
            32,
        );
        let stats = format!(
            "n={} w={} f={} items={} sel={}",
            self.document.nodes().count(),
            self.document.connections().count(),
            self.document.frames().count(),
            canvas.iter().count(),
            self.selected_node
        );
        let meta = fit_text(
            &format!("{stats}  {}", self.status),
            panel.width - 28.0,
            10.0,
        );
        paint_graph_text(
            cx,
            (panel.x + 14.0, panel.y + 38.0),
            &meta,
            10.0,
            policy.hud_secondary_text,
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
        let target_width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            WIDTH
        };
        let target_height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            HEIGHT
        };
        let size = constraints.constrain(Size::new(target_width, target_height));
        if size != self.last_layout_size {
            self.view = std::mem::take(&mut self.view)
                .with_viewport_size(size.width as f64, size.height as f64);
        }
        self.last_layout_size = size;
        if !self.viewport_fitted {
            self.fit_viewport_to_demo(size);
            self.viewport_fitted = true;
        }
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(
            rect,
            spatial_visual_policy(cx.theme).canvas_background,
            0,
            cx.scene_key,
        );

        if let Some(projection) = self.current_projection() {
            self.paint_graph(cx, rect, &projection);
        } else {
            paint_graph_text(
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

        if let Event::Key(key) = event {
            if self.handle_key_event(cx, key) {
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
            self.request_stored_cursor(cx);
            return false;
        }

        self.apply_intents(intents);
        self.request_stored_cursor(cx);
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
    catalog.register(NodeType {
        type_id: 50,
        name: "Boolean Gate".to_string(),
        category: "Logic".to_string(),
        default_title: "Boolean Gate".to_string(),
        default_color: Color::from_srgb(0.17, 0.62, 0.66, 1.0),
        input_ports: vec![
            PortDefinition {
                port_id: 0,
                label: "a".to_string(),
                direction: PortDirection::Input,
                type_id: boolean,
                max_connections: PortCardinality::Single,
            },
            PortDefinition {
                port_id: 1,
                label: "b".to_string(),
                direction: PortDirection::Input,
                type_id: boolean,
                max_connections: PortCardinality::Single,
            },
        ],
        output_ports: vec![PortDefinition {
            port_id: 2,
            label: "out".to_string(),
            direction: PortDirection::Output,
            type_id: boolean,
            max_connections: PortCardinality::Multiple,
        }],
        resizable: false,
        collapsible: true,
    });
    catalog.register(NodeType {
        type_id: 60,
        name: "Number Output".to_string(),
        category: "Output".to_string(),
        default_title: "Number Output".to_string(),
        default_color: Color::from_srgb(0.46, 0.42, 0.70, 1.0),
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "in".to_string(),
            direction: PortDirection::Input,
            type_id: number,
            max_connections: PortCardinality::Single,
        }],
        output_ports: vec![],
        resizable: false,
        collapsible: true,
    });
    catalog.register(NodeType {
        type_id: 70,
        name: "Numeric Transform".to_string(),
        category: "Math".to_string(),
        default_title: "Numeric Transform".to_string(),
        default_color: Color::from_srgb(0.22, 0.58, 0.76, 1.0),
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "in".to_string(),
            direction: PortDirection::Input,
            type_id: number,
            max_connections: PortCardinality::Single,
        }],
        output_ports: vec![PortDefinition {
            port_id: 1,
            label: "out".to_string(),
            direction: PortDirection::Output,
            type_id: number,
            max_connections: PortCardinality::Multiple,
        }],
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

fn spatial_visual_policy(theme: &Theme) -> SpatialVisualPolicy {
    SpatialVisualPolicy::for_theme(theme.is_dark())
}

fn spatial_interaction_policy() -> SpatialInteractionPolicy {
    SpatialInteractionPolicy::default()
}

fn spatial_key_code(key_code: KeyCode) -> KeyCode {
    match key_code {
        KeyCode::Character(character) => KeyCode::Character(character.to_ascii_lowercase()),
        _ => key_code,
    }
}

fn cursor_icon_for_spatial_policy(cursor: SpatialCursorPolicy) -> CursorIcon {
    match cursor {
        SpatialCursorPolicy::Default => CursorIcon::Default,
        SpatialCursorPolicy::Text => CursorIcon::Text,
        SpatialCursorPolicy::Link => CursorIcon::Pointer,
        SpatialCursorPolicy::ResizeHorizontal => CursorIcon::EwResize,
        SpatialCursorPolicy::ResizeVertical => CursorIcon::NsResize,
        SpatialCursorPolicy::DragActive => CursorIcon::Move,
        SpatialCursorPolicy::CrosshairActive => CursorIcon::Crosshair,
    }
}

fn spatial_guide_visual(policy: &SpatialVisualPolicy, role: SpatialGuideRole) -> SpatialRoleVisual {
    match role {
        SpatialGuideRole::Active => policy.active_guide,
        SpatialGuideRole::Structural => policy.structural_guide,
    }
}

fn spatial_wire_visual(
    policy: &SpatialVisualPolicy,
    state: WireVisualState,
    fallback: Option<Color>,
) -> SpatialRoleVisual {
    let mut visual = match state {
        WireVisualState::Default | WireVisualState::Dimmed => policy.route_default,
        WireVisualState::Hovered => policy.route_hovered,
        WireVisualState::Selected => policy.route_selected,
        WireVisualState::Critical => policy.route_blocked,
        WireVisualState::Preview => policy.route_preview,
    };

    if matches!(state, WireVisualState::Default) {
        if let Some(color) = fallback {
            visual.stroke = color;
        }
    } else if matches!(state, WireVisualState::Dimmed) {
        visual.stroke = visual.stroke.with_alpha(0.45);
    }

    visual
}

fn frame_title_color(theme: &Theme) -> Color {
    if theme.is_dark() {
        Color::from_srgb(0.82, 0.88, 0.94, 0.85)
    } else {
        Color::from_srgb(0.12, 0.15, 0.20, 0.76)
    }
}

fn node_body_color(theme: &Theme) -> Color {
    if theme.is_dark() {
        Color::from_srgb(0.09, 0.105, 0.13, 0.96)
    } else {
        Color::from_srgb(0.985, 0.990, 0.995, 0.98)
    }
}

fn fit_text(text: &str, width: f32, font_size: f32) -> String {
    let max_chars = (width.max(1.0) / (font_size * 0.56)).floor() as usize;
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let mut out: String = text.chars().take(max_chars - 3).collect();
    out.push_str("...");
    out
}

fn paint_grid(cx: &mut PaintContext, rect: RectF, transform: &CanvasTransform) {
    let policy = spatial_visual_policy(cx.theme);
    let Ok(transform) = CanvasSpatialBridge::spatial_transform(
        CanvasViewportState {
            transform: *transform,
        },
        rect,
    ) else {
        return;
    };
    let Ok(lines) = realize_spatial_grid(transform, SpatialGridPolicy::default()) else {
        return;
    };
    for line in lines {
        let visual = match line.role {
            SpatialGridLineRole::Minor => policy.grid_minor,
            SpatialGridLineRole::Major => policy.grid_major,
        };
        cx.scene
            .push_rect(line.rect, visual.stroke, 1, cx.scene_key);
    }
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
        FrameVisualState::Selected if cx.theme.is_dark() => Color::from_srgb(0.78, 0.88, 1.0, 0.95),
        FrameVisualState::Selected => Color::from_srgb(0.14, 0.36, 0.76, 0.92),
        FrameVisualState::Resizing if cx.theme.is_dark() => {
            Color::from_srgb(0.92, 0.82, 0.36, 0.95)
        }
        FrameVisualState::Resizing => Color::from_srgb(0.82, 0.52, 0.08, 0.92),
        FrameVisualState::Hovered if cx.theme.is_dark() => Color::from_srgb(0.82, 0.88, 0.94, 0.75),
        FrameVisualState::Hovered => frame.color.with_alpha(0.62),
        FrameVisualState::Default => {
            if cx.theme.is_dark() {
                frame.color.with_alpha(0.62)
            } else {
                frame.color.with_alpha(0.54)
            }
        }
    };
    let fill_alpha = if cx.theme.is_dark() { 0.10 } else { 0.11 };
    cx.scene.push_rounded_rect(
        rect,
        frame.color.with_alpha(fill_alpha),
        RoundedCorners::uniform(10.0),
        2,
        cx.scene_key,
    );
    cx.scene
        .push_border_rect(rect, border, 1.5, 3, cx.scene_key);
    let title = format!("{} ({})", frame.title, frame.contained_nodes.len());
    let label = fit_text(&title, rect.width - 28.0, 13.0);
    paint_graph_text(
        cx,
        (rect.x + 14.0, rect.y + 10.0),
        &label,
        13.0,
        frame_title_color(cx.theme),
        4,
    );
}

fn paint_wire(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    wire: &WireProjection,
) {
    let policy = spatial_visual_policy(cx.theme);
    let visual = spatial_wire_visual(&policy, wire.state, wire.color);
    let points: Vec<Point> = wire
        .path
        .flatten()
        .into_iter()
        .map(|point| {
            world_point_to_screen(
                origin,
                transform,
                WorldPoint::new(point[0] as f64, point[1] as f64),
            )
        })
        .collect();

    if points.len() >= 2 {
        cx.scene
            .push_polyline(points, visual.stroke_width, visual.stroke, 8, cx.scene_key);
    }
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
    let policy = spatial_visual_policy(cx.theme);
    let visual = if snapped {
        policy.route_selected
    } else {
        policy.route_preview
    };
    let points: Vec<Point> = wire_path(WireStyle::Bezier, from_world, to_world)
        .flatten()
        .into_iter()
        .map(|point| {
            world_point_to_screen(
                origin,
                transform,
                WorldPoint::new(point[0] as f64, point[1] as f64),
            )
        })
        .collect();

    if points.len() >= 2 {
        cx.scene
            .push_polyline(points, visual.stroke_width, visual.stroke, 18, cx.scene_key);
    }
}

fn paint_alignment_guides(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    guides: &[AlignmentGuide],
) {
    let visible = visible_world_rect(origin, transform);
    let policy = spatial_visual_policy(cx.theme);

    for guide in guides {
        let visual = match guide.role {
            AlignmentGuideRole::ActiveSnap => {
                spatial_guide_visual(&policy, SpatialGuideRole::Active)
            }
            AlignmentGuideRole::Structural => {
                spatial_guide_visual(&policy, SpatialGuideRole::Structural)
            }
        };
        let thickness = visual.stroke_width;

        match guide.axis {
            AlignmentGuideAxis::Vertical => {
                let start = world_point_to_screen(
                    origin,
                    transform,
                    WorldPoint::new(guide.position, visible.y),
                );
                let end = world_point_to_screen(
                    origin,
                    transform,
                    WorldPoint::new(guide.position, visible.y + visible.height),
                );
                let y = start.y.min(end.y);
                let height = (start.y - end.y).abs().max(1.0);
                cx.scene.push_rect(
                    RectF::new(start.x - thickness * 0.5, y, thickness, height),
                    visual.stroke,
                    24,
                    cx.scene_key,
                );
            }
            AlignmentGuideAxis::Horizontal => {
                let start = world_point_to_screen(
                    origin,
                    transform,
                    WorldPoint::new(visible.x, guide.position),
                );
                let end = world_point_to_screen(
                    origin,
                    transform,
                    WorldPoint::new(visible.x + visible.width, guide.position),
                );
                let x = start.x.min(end.x);
                let width = (start.x - end.x).abs().max(1.0);
                cx.scene.push_rect(
                    RectF::new(x, start.y - thickness * 0.5, width, thickness),
                    visual.stroke,
                    24,
                    cx.scene_key,
                );
            }
        }
    }
}

fn visible_world_rect(origin: RectF, transform: &CanvasTransform) -> WorldRect {
    let top_left = transform.screen_to_world(0.0, 0.0);
    let bottom_right = transform.screen_to_world(origin.width, origin.height);
    WorldRect::new(
        top_left.x.min(bottom_right.x),
        top_left.y.min(bottom_right.y),
        (top_left.x - bottom_right.x).abs(),
        (top_left.y - bottom_right.y).abs(),
    )
}

fn projection_content_bounds(projection: &NodeGraphProjection) -> Option<WorldRect> {
    let mut bounds = None;
    for frame in projection.frames() {
        bounds = Some(match bounds {
            Some(current) => union_world_rect(current, frame.bounds()),
            None => frame.bounds(),
        });
    }
    for node in projection.nodes() {
        bounds = Some(match bounds {
            Some(current) => union_world_rect(current, node.bounds()),
            None => node.bounds(),
        });
    }
    bounds
}

fn union_world_rect(a: WorldRect, b: WorldRect) -> WorldRect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    WorldRect::new(left, top, right - left, bottom - top)
}

fn normalized_world_rect(start: WorldPoint, current: WorldPoint) -> WorldRect {
    let min_x = start.x.min(current.x);
    let min_y = start.y.min(current.y);
    let max_x = start.x.max(current.x);
    let max_y = start.y.max(current.y);
    WorldRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn translate_world_rect(rect: WorldRect, delta: (f64, f64)) -> WorldRect {
    WorldRect::new(rect.x + delta.0, rect.y + delta.1, rect.width, rect.height)
}

fn inset_world_rect(rect: WorldRect, inset: f64) -> Option<WorldRect> {
    let width = rect.width - inset * 2.0;
    let height = rect.height - inset * 2.0;
    (width > 0.0 && height > 0.0)
        .then(|| WorldRect::new(rect.x + inset, rect.y + inset, width, height))
}

fn paint_rubber_band(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    world_rect: WorldRect,
) {
    let rect = world_rect_to_screen(origin, transform, world_rect);
    let policy = spatial_visual_policy(cx.theme);
    cx.scene.push_rounded_rect(
        rect,
        policy.drag_preview.fill,
        RoundedCorners::uniform(3.0),
        20,
        cx.scene_key,
    );
    cx.scene.push_border_rect(
        rect,
        policy.drag_preview.stroke,
        policy.drag_preview.stroke_width,
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
        NodeVisualState::Selected if cx.theme.is_dark() => Color::from_srgb(0.74, 0.86, 1.0, 1.0),
        NodeVisualState::Selected => Color::from_srgb(0.12, 0.38, 0.82, 1.0),
        NodeVisualState::Running if cx.theme.is_dark() => Color::from_srgb(1.0, 0.78, 0.22, 1.0),
        NodeVisualState::Running => Color::from_srgb(0.86, 0.52, 0.04, 1.0),
        NodeVisualState::Completed => Color::from_srgb(0.24, 0.70, 0.44, 1.0),
        NodeVisualState::Error => Color::from_srgb(0.92, 0.18, 0.18, 1.0),
        NodeVisualState::Hovered if cx.theme.is_dark() => Color::from_srgb(0.86, 0.91, 1.0, 0.95),
        NodeVisualState::Hovered => Color::from_srgb(0.12, 0.36, 0.74, 0.82),
        NodeVisualState::Default if cx.theme.is_dark() => Color::from_srgb(0.38, 0.44, 0.52, 0.88),
        NodeVisualState::Default => Color::from_srgb(0.36, 0.42, 0.50, 0.58),
    };
    let radius = RoundedCorners::uniform(8.0);
    let title_h = 32.0 * transform.zoom as f32;
    let title_rect = RectF::new(rect.x, rect.y, rect.width, title_h);

    cx.scene
        .push_rounded_rect(rect, node_body_color(cx.theme), radius, 10, cx.scene_key);
    cx.scene.push_rect(
        title_rect,
        node.color.with_alpha(if selected { 0.95 } else { 0.84 }),
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
    let font_size = (13.0 * transform.zoom as f32).clamp(10.0, 13.0);
    let title = fit_text(&node.title, rect.width - 28.0, font_size);
    paint_graph_text(
        cx,
        (rect.x + 14.0, rect.y + (title_h - font_size) * 0.5 - 1.0),
        &title,
        font_size,
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
    let radius = 3.5_f32.max((3.5 * transform.zoom as f32).min(5.5));
    let policy = spatial_visual_policy(cx.theme);
    let outline = if port.connection_count > 0 {
        policy.handle_anchor.stroke
    } else {
        policy.handle_anchor.stroke.with_alpha(0.58)
    };
    cx.scene.push_circle(
        center,
        radius + policy.handle_anchor.stroke_width.max(1.0),
        outline,
        16,
        cx.scene_key,
    );
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

fn paint_graph_text(
    cx: &mut PaintContext,
    position: (f32, f32),
    text: &str,
    font_size: f32,
    color: Color,
    z_index: u32,
) {
    let run = cx.text_engine.shape(text, font_size);
    cx.scene.push_text_with_backgrounds(
        &run,
        position.0,
        position.1,
        color,
        Vec::new(),
        z_index,
        None,
    );
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
