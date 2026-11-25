use halcyon::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use halcyon::core::paint::Color;
use halcyon::core::widget::{EventContext, Widget};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::model::canvas::{CanvasEntityId, CanvasProjection};
use halcyon::model::node_editor::{
    NodeEditorIntent, NodeEditorSession, NodeFlowConfig, NodeFlowCoordinator, NodeFlowInputs,
    SelectionIntent, SessionIntent,
};
use halcyon::model::node_graph::{
    GraphDelta, GraphTransaction, GraphTransactionContext, NodeGraphDocument, NodeType,
    NodeTypeCatalog, PortCardinality, PortDefinition, PortDirection, PortTypeId, PortTypeInfo,
    PortTypeRegistry,
};
use halcyon::model::node_graph_projection::NodeGraphProjection;
use halcyon::widgets::label::Label;
use halcyon::widgets::{NodeFlowSurface, NodeFlowSurfaceBinding};

struct OwnerState {
    document: NodeGraphDocument,
    session: NodeEditorSession,
    catalog: NodeTypeCatalog,
    registry: PortTypeRegistry,
    source_id: u64,
    sink_id: u64,
}

impl OwnerState {
    fn inputs(&self) -> NodeFlowInputs<'_> {
        NodeFlowInputs {
            document: &self.document,
            session: &self.session,
            catalog: &self.catalog,
            port_registry: &self.registry,
            execution_overlay: None,
        }
    }

    fn projection(&self, config: NodeFlowConfig) -> NodeGraphProjection {
        NodeFlowCoordinator::new()
            .with_config(config)
            .build_node_graph_projection(self.inputs())
            .unwrap()
    }

    fn canvas_projection(&self) -> CanvasProjection {
        NodeFlowCoordinator::new()
            .build_projection(self.inputs())
            .unwrap()
    }

    fn apply_session(&mut self, intent: SessionIntent) {
        match intent {
            SessionIntent::Selection(selection) => {
                let projection = self.canvas_projection();
                self.session.apply_selection_intent(selection, &projection);
            }
            SessionIntent::BeginRubberBand { start, additive } => {
                self.session.begin_rubber_band(start, additive);
            }
            SessionIntent::UpdateRubberBand { current } => {
                self.session.update_rubber_band(current);
            }
            SessionIntent::CancelRubberBand => self.session.cancel_rubber_band(),
            SessionIntent::BeginWireDrag { endpoint, cursor } => {
                self.session
                    .begin_wire_drag(&self.document, &self.catalog, endpoint, cursor)
                    .unwrap();
            }
            SessionIntent::UpdateWireDrag {
                cursor,
                snapped_port,
            } => {
                self.session.update_wire_drag(cursor, snapped_port).unwrap();
            }
            SessionIntent::CancelWireDrag => self.session.cancel_wire_drag(),
            SessionIntent::CancelFrameResize => self.session.cancel_frame_resize(),
            SessionIntent::CancelInlineEdit => self.session.cancel_inline_edit(),
            SessionIntent::SetViewport(viewport) => self.session.viewport = viewport,
            SessionIntent::SetTool(tool) => self.session.active_tool = tool,
            SessionIntent::Search(_) => {}
        }
    }
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

fn owner_state() -> OwnerState {
    let number = PortTypeId(1);
    let mut registry = PortTypeRegistry::new();
    registry.register(PortTypeInfo {
        type_id: number,
        name: "Number".to_string(),
        color: Color::WHITE,
        compatible_with: vec![number],
    });

    let mut catalog = NodeTypeCatalog::new();
    catalog.register(NodeType {
        type_id: 10,
        name: "Source".to_string(),
        category: "Test".to_string(),
        default_title: "Source".to_string(),
        default_color: Color::from_srgb(0.2, 0.5, 0.9, 1.0),
        input_ports: Vec::new(),
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
        name: "Sink".to_string(),
        category: "Test".to_string(),
        default_title: "Sink".to_string(),
        default_color: Color::from_srgb(0.2, 0.7, 0.4, 1.0),
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "in".to_string(),
            direction: PortDirection::Input,
            type_id: number,
            max_connections: PortCardinality::Single,
        }],
        output_ports: Vec::new(),
        resizable: false,
        collapsible: true,
    });

    let mut document = NodeGraphDocument::new(1, "surface test");
    let source_id = add_node(&mut document, &catalog, &registry, 10, (60.0, 70.0));
    let sink_id = add_node(&mut document, &catalog, &registry, 20, (360.0, 190.0));
    OwnerState {
        document,
        session: NodeEditorSession::new(),
        catalog,
        registry,
        source_id,
        sink_id,
    }
}

fn add_node(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    type_id: u64,
    position: (f64, f64),
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddNode {
                type_id,
                position,
                title: None,
            },
            graph_context(catalog, registry),
        )
        .unwrap();
    match commit.delta {
        GraphDelta::AddNode { node } => node.id,
        _ => unreachable!(),
    }
}

fn event_tree() -> (WidgetTree, NodeId) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Label::new("root")), None, 0);
    (tree, root)
}

fn event_context(
    tree: &mut WidgetTree,
    node: NodeId,
    captured_pointer: bool,
    mode: KeyboardAuthorityMode,
) -> EventContext<'_> {
    let id = tree.node(node).unwrap().id();
    EventContext::new(
        id,
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,
        None,
        captured_pointer,
        false,
        mode,
    )
}

fn pointer(
    kind: PointerEventKind,
    button: Option<PointerButton>,
    local_pos: (f32, f32),
    modifiers: Modifiers,
) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: local_pos,
        local_pos,
        kind,
        button,
        modifiers,
        pointer_type: PointerType::Mouse,
    })
}

fn key(key_code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

fn publish(binding: &NodeFlowSurfaceBinding, owner: &OwnerState) {
    binding.publish(owner.inputs()).unwrap();
}

#[test]
fn node_flow_surface_node_move_emits_intents_without_mutating_owner() {
    let mut owner = owner_state();
    let config = NodeFlowConfig::default();
    let projection = owner.projection(config.clone());
    let node = projection
        .nodes()
        .iter()
        .find(|node| node.id == owner.source_id)
        .unwrap();
    let down_pos = (
        (node.position.x + 24.0) as f32,
        (node.position.y + 12.0) as f32,
    );
    let move_pos = (down_pos.0 + 42.0, down_pos.1 + 28.0);
    let binding = NodeFlowSurfaceBinding::new(config);
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Primary),
            down_pos,
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    assert!(owner.session.selection.entities.is_empty());
    assert!(owner.session.drag.is_none());

    let down_intents = binding.take_intents();
    assert_eq!(down_intents.len(), 2);
    for intent in down_intents {
        let NodeEditorIntent::Session(intent) = intent else {
            panic!("pointer-down must only emit session intents");
        };
        owner.apply_session(intent);
    }
    assert_eq!(
        owner.session.selection.entities,
        vec![CanvasEntityId::Node(owner.source_id)]
    );
    assert!(owner.session.drag.is_some());
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(PointerEventKind::Move, None, move_pos, Modifiers::default(),),
        EventPhase::Target,
    ));
    let move_intents = binding.take_intents();
    assert_eq!(move_intents.len(), 1);
    let NodeEditorIntent::Session(move_intent) = move_intents.into_iter().next().unwrap() else {
        panic!("pointer move must emit a session intent");
    };
    owner.apply_session(move_intent);
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Up,
            Some(PointerButton::Primary),
            move_pos,
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let up_intents = binding.take_intents();
    assert!(up_intents.iter().any(|intent| matches!(
        intent,
        NodeEditorIntent::Graph(GraphTransaction::MoveNodes { moves })
            if moves == &vec![(owner.source_id, (102.0, 98.0))]
    )));
    assert!(up_intents.iter().any(|intent| matches!(
        intent,
        NodeEditorIntent::Session(SessionIntent::Selection(SelectionIntent::CommitMove))
    )));
    assert_eq!(
        owner.document.node(owner.source_id).unwrap().position,
        (60.0, 70.0)
    );
}

#[test]
fn node_flow_surface_drag_preserves_existing_multi_selection() {
    let mut owner = owner_state();
    let config = NodeFlowConfig::default();
    let projection = owner.projection(config.clone());
    owner.session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(owner.source_id)),
        projection.canvas(),
    );
    owner.session.apply_selection_intent(
        SelectionIntent::Toggle(CanvasEntityId::Node(owner.sink_id)),
        projection.canvas(),
    );
    let source = projection
        .nodes()
        .iter()
        .find(|node| node.id == owner.source_id)
        .unwrap();
    let point = (
        (source.position.x + 24.0) as f32,
        (source.position.y + 12.0) as f32,
    );
    let binding = NodeFlowSurfaceBinding::new(config);
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Primary),
            point,
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    assert!(matches!(
        binding.take_intents().as_slice(),
        [NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::BeginMove { .. }
        ))]
    ));
    assert_eq!(owner.session.selection.entities.len(), 2);
}

#[test]
fn node_flow_surface_pan_and_control_scroll_emit_viewport_intents() {
    let owner = owner_state();
    let binding = NodeFlowSurfaceBinding::default();
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Auxiliary),
            (20.0, 30.0),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    assert!(binding.take_intents().is_empty());

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Move,
            None,
            (50.0, 70.0),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let pan = binding.take_intents();
    assert!(matches!(
        pan.as_slice(),
        [NodeEditorIntent::Session(SessionIntent::SetViewport(viewport))]
            if viewport.transform.offset_x == -30.0
                && viewport.transform.offset_y == -40.0
                && viewport.transform.zoom == 1.0
    ));

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(!surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            },
            None,
            (100.0, 100.0),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    assert!(binding.take_intents().is_empty());

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            },
            None,
            (100.0, 100.0),
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        ),
        EventPhase::Target,
    ));
    let zoom = binding.take_intents();
    assert!(matches!(
        zoom.as_slice(),
        [NodeEditorIntent::Session(SessionIntent::SetViewport(viewport))]
            if viewport.transform.zoom > 1.0
    ));
}

#[test]
fn node_flow_surface_wire_drag_emits_validated_graph_transaction() {
    let mut owner = owner_state();
    let config = NodeFlowConfig::default();
    let projection = owner.projection(config.clone());
    let source = projection
        .ports()
        .find(|port| {
            port.node_id == owner.source_id && port.definition.direction == PortDirection::Output
        })
        .unwrap()
        .world_pos;
    let target = projection
        .ports()
        .find(|port| {
            port.node_id == owner.sink_id && port.definition.direction == PortDirection::Input
        })
        .unwrap()
        .world_pos;
    let binding = NodeFlowSurfaceBinding::new(config);
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Primary),
            (source.x as f32, source.y as f32),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let start = binding.take_intents();
    assert_eq!(start.len(), 1);
    let NodeEditorIntent::Session(start) = start.into_iter().next().unwrap() else {
        panic!("wire start must be a session intent");
    };
    owner.apply_session(start);
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Move,
            None,
            (target.x as f32, target.y as f32),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let update = binding.take_intents();
    let NodeEditorIntent::Session(update) = update.into_iter().next().unwrap() else {
        panic!("wire update must be a session intent");
    };
    owner.apply_session(update);
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Up,
            Some(PointerButton::Primary),
            (target.x as f32, target.y as f32),
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let commit = binding.take_intents();
    assert!(commit.iter().any(|intent| matches!(
        intent,
        NodeEditorIntent::Graph(GraphTransaction::AddConnection {
            from_node,
            from_port: 0,
            to_node,
            to_port: 0,
        }) if *from_node == owner.source_id && *to_node == owner.sink_id
    )));
    assert!(commit.iter().any(|intent| matches!(
        intent,
        NodeEditorIntent::Session(SessionIntent::CancelWireDrag)
    )));
    assert_eq!(owner.document.connections().count(), 0);
}

#[test]
fn node_flow_surface_rubber_band_stays_in_external_session() {
    let mut owner = owner_state();
    let binding = NodeFlowSurfaceBinding::default();
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Primary),
            (20.0, 20.0),
            shift,
        ),
        EventPhase::Target,
    ));
    assert!(owner.session.rubber_band.is_none());
    let NodeEditorIntent::Session(begin) = binding.take_intents().pop().unwrap() else {
        panic!("rubber-band start must be a session intent");
    };
    owner.apply_session(begin);
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(PointerEventKind::Move, None, (300.0, 160.0), shift,),
        EventPhase::Target,
    ));
    let NodeEditorIntent::Session(update) = binding.take_intents().pop().unwrap() else {
        panic!("rubber-band update must be a session intent");
    };
    owner.apply_session(update);
    publish(&binding, &owner);

    let mut cx = event_context(&mut tree, root, true, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Up,
            Some(PointerButton::Primary),
            (300.0, 160.0),
            shift,
        ),
        EventPhase::Target,
    ));
    for intent in binding.take_intents() {
        let NodeEditorIntent::Session(intent) = intent else {
            panic!("rubber-band commit must only emit session intents");
        };
        owner.apply_session(intent);
    }
    assert_eq!(
        owner.session.selection.entities,
        vec![CanvasEntityId::Node(owner.source_id)]
    );
    assert!(owner.session.rubber_band.is_none());
}

#[test]
fn node_flow_surface_read_only_allows_view_state_but_rejects_mutation() {
    let mut owner = owner_state();
    let mut config = NodeFlowConfig::default();
    config.read_only = true;
    let projection = owner.projection(config.clone());
    let node = projection
        .nodes()
        .iter()
        .find(|node| node.id == owner.sink_id)
        .unwrap();
    let point = (
        (node.position.x + 24.0) as f32,
        (node.position.y + 12.0) as f32,
    );
    owner.session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(owner.source_id)),
        projection.canvas(),
    );
    let binding = NodeFlowSurfaceBinding::new(config);
    publish(&binding, &owner);
    let mut surface = NodeFlowSurface::new(binding.clone());
    let (mut tree, root) = event_tree();

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Down,
            Some(PointerButton::Primary),
            point,
            Modifiers::default(),
        ),
        EventPhase::Target,
    ));
    let click = binding.take_intents();
    assert_eq!(click.len(), 1);
    assert!(matches!(
        click.as_slice(),
        [NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::Replace(CanvasEntityId::Node(_))
        ))]
    ));

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Capture);
    assert!(!surface.handle_event(&mut cx, &key(KeyCode::Delete), EventPhase::Target,));
    assert!(binding.take_intents().is_empty());

    let mut cx = event_context(&mut tree, root, false, KeyboardAuthorityMode::Navigation);
    assert!(surface.handle_event(
        &mut cx,
        &pointer(
            PointerEventKind::Scroll {
                delta_x: 0.0,
                delta_y: -1.0,
            },
            None,
            point,
            Modifiers {
                meta: true,
                ..Modifiers::default()
            },
        ),
        EventPhase::Target,
    ));
    assert!(matches!(
        binding.take_intents().as_slice(),
        [NodeEditorIntent::Session(SessionIntent::SetViewport(_))]
    ));
}
