use std::collections::HashMap;

use halcyon::core::event::{Event, KeyCode, KeyEvent, Modifiers};
use halcyon::core::paint::Color;
use halcyon::model::canvas::{
    CanvasEntityId, CanvasProjection, CanvasTransform, CanvasViewportState, WorldPoint, WorldRect,
};
use halcyon::model::node_editor::*;
use halcyon::model::node_graph::*;
use halcyon::model::node_graph_projection::{
    FrameVisualState, NodeGraphProjection, NodeVisualState,
};
use halcyon::model::spatial::{SpatialGuideAxis, SpatialGuideRole};
use halcyon::runtime::actions::ActionId;
use halcyon::runtime::binding_registry::{BindingRegistry, InputPattern};

fn catalog_and_registry() -> (NodeTypeCatalog, PortTypeRegistry) {
    let number = PortTypeId(1);
    let boolean = PortTypeId(2);

    let mut registry = PortTypeRegistry::new();
    registry.register(PortTypeInfo {
        type_id: number,
        name: "Number".to_string(),
        color: Color::WHITE,
        compatible_with: vec![number],
    });
    registry.register(PortTypeInfo {
        type_id: boolean,
        name: "Boolean".to_string(),
        color: Color::BLACK,
        compatible_with: vec![boolean],
    });

    let mut catalog = NodeTypeCatalog::new();
    catalog.register(NodeType {
        type_id: 10,
        name: "Number Source".to_string(),
        category: "Math".to_string(),
        default_title: "Source".to_string(),
        default_color: Color::WHITE,
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
        name: "Number Sink".to_string(),
        category: "Math".to_string(),
        default_title: "Sink".to_string(),
        default_color: Color::BLACK,
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
        type_id: 30,
        name: "Boolean Sink".to_string(),
        category: "Logic".to_string(),
        default_title: "Boolean Sink".to_string(),
        default_color: Color::BLACK,
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

fn context<'a>(
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
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddNode {
                type_id,
                position,
                title: None,
            },
            context(catalog, registry),
        )
        .unwrap();
    match commit.delta {
        GraphDelta::AddNode { node } => node.id,
        _ => panic!("expected AddNode delta"),
    }
}

fn add_frame(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    position: (f64, f64),
    size: (f64, f64),
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddFrame {
                title: "Group".to_string(),
                position,
                size,
                color: (10, 20, 30),
            },
            context(catalog, registry),
        )
        .unwrap();
    match commit.delta {
        GraphDelta::AddFrame { frame } => frame.id,
        _ => panic!("expected AddFrame delta"),
    }
}

fn projection(
    document: &NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
) -> CanvasProjection {
    NodeGraphProjection::from_document(document, catalog, registry)
        .unwrap()
        .canvas()
        .clone()
}

fn key_event(key_code: KeyCode, is_down: bool, shift: bool) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down,
        is_repeat: false,
        modifiers: Modifiers {
            ctrl: false,
            alt: false,
            shift,
            meta: false,
        },
        text: None,
    })
}

#[test]
fn selection_intents_mutate_session_not_document() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (240.0, 0.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();

    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    session.apply_selection_intent(
        SelectionIntent::Toggle(CanvasEntityId::Node(sink)),
        &projection,
    );

    assert_eq!(
        session.selection.entities,
        vec![CanvasEntityId::Node(source), CanvasEntityId::Node(sink)]
    );
    assert_eq!(session.revision, SessionRevision(2));
    assert_eq!(document.revision(), GraphRevision(2));

    let delete = selection_delete_transaction(&session.selection).unwrap();
    assert!(matches!(delete, GraphTransaction::RemoveNodes { .. }));
}

#[test]
fn move_drag_transaction_commits_absolute_node_positions() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();

    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    session.apply_selection_intent(
        SelectionIntent::BeginMove {
            anchor: WorldPoint::new(10.0, 20.0),
            primary: CanvasEntityId::Node(source),
        },
        &projection,
    );
    session.apply_selection_intent(
        SelectionIntent::UpdateMove {
            delta: (15.0, -5.0),
        },
        &projection,
    );

    let transaction = move_drag_transaction(&document, session.drag.as_ref().unwrap()).unwrap();

    assert!(matches!(
        transaction,
        GraphTransaction::MoveNodes { moves } if moves == vec![(source, (25.0, 15.0))]
    ));
}

#[test]
fn rubber_band_selects_intersecting_graph_entities_only() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (300.0, 0.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();

    session.apply_selection_intent(
        SelectionIntent::RubberBandCommit {
            rect: WorldRect::new(-10.0, -10.0, 240.0, 120.0),
            additive: false,
        },
        &projection,
    );

    assert_eq!(
        session.selection.entities,
        vec![CanvasEntityId::Node(source)]
    );
    session.apply_selection_intent(SelectionIntent::SelectAll, &projection);
    assert!(
        session
            .selection
            .entities
            .contains(&CanvasEntityId::Node(sink))
    );
    assert!(!session.selection.entities.iter().any(|entity| {
        matches!(
            entity,
            CanvasEntityId::Port {
                node_id: _,
                port_id: _,
                direction: _
            }
        )
    }));
}

#[test]
fn wire_drag_emits_connection_transaction_without_mutating_document() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (240.0, 0.0));
    let mut session = NodeEditorSession::new();

    session
        .begin_wire_drag(
            &document,
            &catalog,
            PortEndpoint {
                node_id: source,
                port_id: 0,
                direction: PortDirection::Output,
            },
            WorldPoint::new(180.0, 44.0),
        )
        .unwrap();
    let transaction = session
        .commit_wire_drag(
            &document,
            &catalog,
            &registry,
            Some(PortEndpoint {
                node_id: sink,
                port_id: 0,
                direction: PortDirection::Input,
            }),
        )
        .unwrap()
        .unwrap();

    assert_eq!(document.connections().count(), 0);
    assert!(matches!(
        transaction,
        GraphTransaction::AddConnection {
            from_node,
            from_port: 0,
            to_node,
            to_port: 0
        } if from_node == source && to_node == sink
    ));
    assert!(session.wire_drag.is_none());
}

#[test]
fn connected_input_drag_reroutes_or_removes_existing_connection() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (240.0, 0.0));
    document
        .apply(
            GraphTransaction::AddConnection {
                from_node: source,
                from_port: 0,
                to_node: sink,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    let connection_id = document.connections().next().unwrap().id;
    let mut session = NodeEditorSession::new();

    session
        .begin_wire_drag(
            &document,
            &catalog,
            PortEndpoint {
                node_id: sink,
                port_id: 0,
                direction: PortDirection::Input,
            },
            WorldPoint::new(240.0, 44.0),
        )
        .unwrap();
    let transaction = session
        .commit_wire_drag(&document, &catalog, &registry, None)
        .unwrap()
        .unwrap();

    assert!(matches!(
        transaction,
        GraphTransaction::RemoveConnection { connection_id: id } if id == connection_id
    ));
    assert_eq!(document.connections().count(), 1);
}

#[test]
fn output_wire_drag_replaces_occupied_single_input() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let first_source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let second_source = add_node(&mut document, &catalog, &registry, 10, (0.0, 120.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (240.0, 0.0));
    document
        .apply(
            GraphTransaction::AddConnection {
                from_node: first_source,
                from_port: 0,
                to_node: sink,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    let old_connection_id = document.connections().next().unwrap().id;
    let mut session = NodeEditorSession::new();

    session
        .begin_wire_drag(
            &document,
            &catalog,
            PortEndpoint {
                node_id: second_source,
                port_id: 0,
                direction: PortDirection::Output,
            },
            WorldPoint::new(180.0, 164.0),
        )
        .unwrap();
    let transaction = session
        .commit_wire_drag(
            &document,
            &catalog,
            &registry,
            Some(PortEndpoint {
                node_id: sink,
                port_id: 0,
                direction: PortDirection::Input,
            }),
        )
        .unwrap()
        .unwrap();

    assert!(matches!(
        &transaction,
        GraphTransaction::ReplaceConnection {
            old_connection_id: id,
            from_node,
            from_port: 0,
            to_node,
            to_port: 0,
        } if *id == old_connection_id && *from_node == second_source && *to_node == sink
    ));

    document
        .apply(transaction, context(&catalog, &registry))
        .unwrap();
    let connections = document.connections().collect::<Vec<_>>();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].from_node, second_source);
    assert_eq!(connections[0].to_node, sink);
}

#[test]
fn connected_input_drag_to_output_reroutes_source_endpoint() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let first_source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let second_source = add_node(&mut document, &catalog, &registry, 10, (0.0, 120.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (240.0, 0.0));
    document
        .apply(
            GraphTransaction::AddConnection {
                from_node: first_source,
                from_port: 0,
                to_node: sink,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    let old_connection_id = document.connections().next().unwrap().id;
    let mut session = NodeEditorSession::new();

    session
        .begin_wire_drag(
            &document,
            &catalog,
            PortEndpoint {
                node_id: sink,
                port_id: 0,
                direction: PortDirection::Input,
            },
            WorldPoint::new(240.0, 44.0),
        )
        .unwrap();
    let transaction = session
        .commit_wire_drag(
            &document,
            &catalog,
            &registry,
            Some(PortEndpoint {
                node_id: second_source,
                port_id: 0,
                direction: PortDirection::Output,
            }),
        )
        .unwrap()
        .unwrap();

    assert!(matches!(
        &transaction,
        GraphTransaction::ReplaceConnection {
            old_connection_id: id,
            from_node,
            from_port: 0,
            to_node,
            to_port: 0,
        } if *id == old_connection_id && *from_node == second_source && *to_node == sink
    ));

    document
        .apply(transaction, context(&catalog, &registry))
        .unwrap();
    let connections = document.connections().collect::<Vec<_>>();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].from_node, second_source);
    assert_eq!(connections[0].to_node, sink);
}

#[test]
fn search_filters_by_query_frequency_and_dangling_wire() {
    let (catalog, registry) = catalog_and_registry();
    let mut metadata = HashMap::new();
    metadata.insert(
        30,
        NodeSearchMetadata {
            description: Some("Boolean receiver".to_string()),
            icon: None,
            frequency: 10,
        },
    );
    let entries = node_search_entries(&catalog, &metadata);

    let empty_results = search_node_entries(&entries, "", None, &registry);
    assert_eq!(empty_results[0].type_id, 30);

    let math_results = search_node_entries(&entries, "nso", None, &registry);
    assert_eq!(math_results[0].name, "Number Source");

    let dangling = DanglingWireContext {
        endpoint: PortEndpoint {
            node_id: 1,
            port_id: 0,
            direction: PortDirection::Output,
        },
        port_type: PortTypeId(1),
    };
    let compatible_results = search_node_entries(&entries, "", Some(dangling), &registry);
    assert_eq!(
        compatible_results
            .iter()
            .map(|entry| entry.type_id)
            .collect::<Vec<_>>(),
        vec![20]
    );
}

#[test]
fn coordinator_builds_projection_from_external_authorities() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    let coordinator = NodeFlowCoordinator::new();

    let node_projection = coordinator
        .build_node_graph_projection(NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        })
        .unwrap();

    assert_eq!(node_projection.nodes()[0].state, NodeVisualState::Selected);
    assert_eq!(node_projection.canvas().iter().count(), 2);
}

#[test]
fn coordinator_projection_applies_session_drag_without_mutating_document() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    session.apply_selection_intent(
        SelectionIntent::BeginMove {
            anchor: WorldPoint::new(10.0, 20.0),
            primary: CanvasEntityId::Node(source),
        },
        &projection,
    );
    session.apply_selection_intent(
        SelectionIntent::UpdateMove {
            delta: (15.0, -5.0),
        },
        &projection,
    );
    let coordinator = NodeFlowCoordinator::new();

    let projected = coordinator
        .build_node_graph_projection(NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        })
        .unwrap();

    assert_eq!(document.node(source).unwrap().position, (10.0, 20.0));
    assert_eq!(projected.nodes()[0].position, WorldPoint::new(25.0, 15.0));
    assert_eq!(
        projected.nodes()[0].outputs[0].world_pos,
        WorldPoint::new(205.0, 59.0)
    );
    assert!(projected.revision().0 > document.revision().0 * 31 + catalog.revision().0);
}

#[test]
fn frame_drag_preview_and_transaction_move_captured_nodes() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (20.0, 20.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (320.0, 20.0));
    let frame_id = add_frame(
        &mut document,
        &catalog,
        &registry,
        (0.0, 0.0),
        (260.0, 160.0),
    );
    let base_projection =
        NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let canvas_projection = base_projection.canvas().clone();
    let mut session = NodeEditorSession::new();

    assert!(session.begin_frame_drag(
        WorldPoint::new(0.0, 0.0),
        frame_id,
        base_projection.frames()
    ));
    session.apply_selection_intent(
        SelectionIntent::UpdateMove {
            delta: (40.0, 30.0),
        },
        &canvas_projection,
    );
    let coordinator = NodeFlowCoordinator::new();

    let projected = coordinator
        .build_node_graph_projection(NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        })
        .unwrap();
    let moved_frame = projected
        .frames()
        .iter()
        .find(|frame| frame.id == frame_id)
        .unwrap();
    let moved_source = projected
        .nodes()
        .iter()
        .find(|node| node.id == source)
        .unwrap();
    let fixed_sink = projected
        .nodes()
        .iter()
        .find(|node| node.id == sink)
        .unwrap();

    assert_eq!(document.frame(frame_id).unwrap().position, (0.0, 0.0));
    assert_eq!(document.node(source).unwrap().position, (20.0, 20.0));
    assert_eq!(moved_frame.position, WorldPoint::new(40.0, 30.0));
    assert_eq!(moved_source.position, WorldPoint::new(60.0, 50.0));
    assert_eq!(fixed_sink.position, WorldPoint::new(320.0, 20.0));

    let transaction = move_drag_transaction(&document, session.drag.as_ref().unwrap()).unwrap();
    assert!(matches!(
        transaction,
        GraphTransaction::MoveFrameWithNodes {
            frame_id: id,
            new_pos,
            node_moves,
        } if id == frame_id && new_pos == (40.0, 30.0) && node_moves == vec![(source, (60.0, 50.0))]
    ));
}

#[test]
fn create_frame_around_selection_uses_projected_node_bounds() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (20.0, 30.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (320.0, 80.0));
    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let mut session = NodeEditorSession::new();
    session.selection.entities = vec![
        CanvasEntityId::Node(source),
        CanvasEntityId::Wire(99),
        CanvasEntityId::Node(sink),
    ];

    let transaction = create_frame_around_selection_transaction(
        &session.selection,
        projection.nodes(),
        DEFAULT_FRAME_PADDING,
        "Selection",
        (1, 2, 3),
    )
    .unwrap();
    let source_bounds = projection
        .nodes()
        .iter()
        .find(|node| node.id == source)
        .unwrap()
        .bounds();
    let sink_bounds = projection
        .nodes()
        .iter()
        .find(|node| node.id == sink)
        .unwrap()
        .bounds();
    let min_x = source_bounds.x.min(sink_bounds.x);
    let min_y = source_bounds.y.min(sink_bounds.y);
    let max_x = (source_bounds.x + source_bounds.width).max(sink_bounds.x + sink_bounds.width);
    let max_y = (source_bounds.y + source_bounds.height).max(sink_bounds.y + sink_bounds.height);

    assert!(matches!(
        transaction,
        GraphTransaction::AddFrame {
            ref title,
            position,
            size,
            color,
        } if title == "Selection"
            && position == (min_x - DEFAULT_FRAME_PADDING, min_y - DEFAULT_FRAME_PADDING)
            && size == (
                max_x - min_x + DEFAULT_FRAME_PADDING * 2.0,
                max_y - min_y + DEFAULT_FRAME_PADDING * 2.0
            )
            && color == (1, 2, 3)
    ));
}

#[test]
fn frame_resize_preview_and_transaction_use_session_authority() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let frame_id = add_frame(
        &mut document,
        &catalog,
        &registry,
        (0.0, 0.0),
        (260.0, 160.0),
    );
    let mut session = NodeEditorSession::new();

    assert!(session.begin_frame_resize(frame_id, (260.0, 160.0)));
    assert!(session.update_frame_resize((320.0, 220.0)));
    let coordinator = NodeFlowCoordinator::new();
    let projected = coordinator
        .build_node_graph_projection(NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        })
        .unwrap();
    let resized_frame = projected
        .frames()
        .iter()
        .find(|frame| frame.id == frame_id)
        .unwrap();

    assert_eq!(document.frame(frame_id).unwrap().size, (260.0, 160.0));
    assert_eq!(resized_frame.size, (320.0, 220.0));
    assert_eq!(resized_frame.state, FrameVisualState::Resizing);

    let transaction = session.commit_frame_resize(&document).unwrap();
    assert!(session.frame_resize.is_none());
    assert!(matches!(
        transaction,
        GraphTransaction::ResizeFrame {
            frame_id: id,
            new_size,
        } if id == frame_id && new_size == (320.0, 220.0)
    ));
}

#[test]
fn inline_edit_commit_emits_node_and_frame_title_transactions() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (20.0, 30.0));
    let frame_id = add_frame(
        &mut document,
        &catalog,
        &registry,
        (0.0, 0.0),
        (260.0, 160.0),
    );
    let mut session = NodeEditorSession::new();

    assert!(session.begin_inline_edit(
        CanvasEntityId::Node(source),
        document.node(source).unwrap().title.clone()
    ));
    assert!(session.update_inline_edit("Renamed Source"));
    let transaction = session.commit_inline_edit(&document).unwrap();
    assert!(session.inline_edit.is_none());
    assert!(matches!(
        transaction,
        GraphTransaction::SetNodeTitle { node_id, ref title }
            if node_id == source && title == "Renamed Source"
    ));

    assert!(session.begin_inline_edit(
        CanvasEntityId::Frame(frame_id),
        document.frame(frame_id).unwrap().title.clone()
    ));
    assert!(session.update_inline_edit("Renamed Frame"));
    let transaction = session.commit_inline_edit(&document).unwrap();
    assert!(session.inline_edit.is_none());
    assert!(matches!(
        transaction,
        GraphTransaction::SetFrameTitle {
            frame_id: id,
            ref title,
        } if id == frame_id && title == "Renamed Frame"
    ));
}

#[test]
fn coordinator_read_only_gate_blocks_graph_search_and_connect_intents() {
    let mut coordinator = NodeFlowCoordinator::new().with_config(NodeFlowConfig {
        read_only: true,
        ..NodeFlowConfig::default()
    });

    assert_eq!(
        coordinator.emit_intent(NodeEditorIntent::Graph(GraphTransaction::AddNode {
            type_id: 10,
            position: (0.0, 0.0),
            title: None,
        })),
        NodeFlowIntentStatus::RejectedReadOnly
    );
    assert_eq!(
        coordinator.emit_intent(NodeEditorIntent::Session(SessionIntent::Search(
            NodeSearchIntent::Open {
                position: WorldPoint::new(0.0, 0.0),
                dangling_wire: None,
            },
        ))),
        NodeFlowIntentStatus::RejectedReadOnly
    );
    assert_eq!(
        coordinator.emit_intent(NodeEditorIntent::Session(SessionIntent::SetTool(
            NodeEditorTool::Connect,
        ))),
        NodeFlowIntentStatus::RejectedReadOnly
    );
    assert_eq!(
        coordinator.emit_intent(NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::Clear,
        ))),
        NodeFlowIntentStatus::Accepted
    );

    assert_eq!(coordinator.take_intents().len(), 1);
}

#[test]
fn node_flow_view_alias_builds_projection_from_external_authorities() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let session = NodeEditorSession::new();
    let view = NodeFlowView::new();

    let projection = view
        .build_projection(NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        })
        .unwrap();

    assert_eq!(projection.iter().count(), 2);
    assert!(!view.config().read_only);
}

#[test]
fn coordinator_read_only_gate_filters_action_intents() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    let mut coordinator = NodeFlowCoordinator::new().with_config(NodeFlowConfig {
        read_only: true,
        ..NodeFlowConfig::default()
    });
    let context = NodeFlowActionContext {
        session: &session,
        document: &document,
        projection: Some(&projection),
        pointer_position: None,
        viewport_center: WorldPoint::new(500.0, 600.0),
        grid_unit: 8.0,
    };

    assert!(
        coordinator
            .action_intents(ActionId::NODE_FLOW_DELETE_SELECTED, context)
            .is_empty()
    );
    assert!(
        coordinator
            .action_intents(ActionId::NODE_FLOW_OPEN_SEARCH, context)
            .is_empty()
    );
    assert!(
        coordinator
            .action_intents(ActionId::NODE_FLOW_UNDO, context)
            .is_empty()
    );
    assert!(matches!(
        &coordinator.action_intents(ActionId::NODE_FLOW_SELECT_ALL, context)[..],
        [NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::SelectAll
        ))]
    ));
}

#[test]
fn coordinator_view_helpers_emit_session_viewport_intents() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (100.0, 200.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.viewport = CanvasViewportState {
        transform: CanvasTransform {
            offset_x: 5.0,
            offset_y: 10.0,
            zoom: 2.0,
        },
    };
    let mut coordinator = NodeFlowCoordinator::new().with_viewport_size(800.0, 600.0);

    let SessionIntent::SetViewport(panned) =
        coordinator.pan_to_intent(&session, WorldPoint::new(25.0, 50.0))
    else {
        panic!("expected SetViewport");
    };
    assert_eq!(panned.transform.offset_x, 25.0);
    assert_eq!(panned.transform.offset_y, 50.0);
    assert_eq!(panned.transform.zoom, 2.0);

    let SessionIntent::SetViewport(fit_node) = coordinator
        .zoom_to_node_intent(&session, &projection, source)
        .unwrap()
    else {
        panic!("expected SetViewport");
    };
    assert!(fit_node.transform.zoom > 2.0);
    assert!(
        coordinator
            .zoom_to_node_intent(&session, &projection, source + 999)
            .is_none()
    );
}

#[test]
fn node_flow_default_bindings_resolve_to_stable_action_ids() {
    let mut registry = BindingRegistry::new();
    for binding in node_flow_default_bindings() {
        registry.add(binding);
    }

    assert_eq!(
        registry.lookup(&InputPattern::key_only(
            halcyon::core::event::KeyCode::Delete
        )),
        Some(ActionId::NODE_FLOW_DELETE_SELECTED)
    );
    assert_eq!(
        registry.lookup(&InputPattern::key_only(halcyon::core::event::KeyCode::Tab)),
        Some(ActionId::NODE_FLOW_OPEN_SEARCH)
    );
    assert_eq!(
        registry.lookup(&InputPattern::key_only(
            halcyon::core::event::KeyCode::Escape
        )),
        Some(ActionId::NODE_FLOW_CANCEL)
    );
    assert_eq!(
        registry.lookup(&InputPattern {
            key: (halcyon::core::event::KeyCode::Down).into(),
            ctrl: false,
            alt: false,
            shift: true,
            meta: false,
        }),
        Some(ActionId::NODE_FLOW_NUDGE_DOWN_FAST)
    );
}

#[test]
fn coordinator_action_intents_map_shortcuts_without_mutating_authorities() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    let mut coordinator = NodeFlowCoordinator::new();

    let intents = coordinator.action_intents(
        ActionId::NODE_FLOW_NUDGE_RIGHT_FAST,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center: WorldPoint::new(500.0, 600.0),
            grid_unit: 8.0,
        },
    );

    assert_eq!(document.node(source).unwrap().position, (10.0, 20.0));
    assert!(matches!(
        &intents[..],
        [NodeEditorIntent::Graph(GraphTransaction::MoveNodes { moves })]
            if moves == &vec![(source, (90.0, 20.0))]
    ));

    let search = coordinator.action_intents(
        ActionId::NODE_FLOW_OPEN_SEARCH,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center: WorldPoint::new(500.0, 600.0),
            grid_unit: 8.0,
        },
    );
    assert!(matches!(
        &search[..],
        [NodeEditorIntent::Session(SessionIntent::Search(NodeSearchIntent::Open {
            position,
            dangling_wire: None,
        }))] if *position == WorldPoint::new(500.0, 600.0)
    ));
}

#[test]
fn paste_action_uses_session_clipboard_and_target_position() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    session.apply_selection_intent(SelectionIntent::CopySelected, &projection);
    let mut coordinator = NodeFlowCoordinator::new();

    let intents = coordinator.action_intents(
        ActionId::NODE_FLOW_PASTE,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: Some(WorldPoint::new(100.0, 200.0)),
            viewport_center: WorldPoint::new(500.0, 600.0),
            grid_unit: 8.0,
        },
    );

    assert_eq!(document.nodes().count(), 1);
    assert!(matches!(
        &intents[..],
        [NodeEditorIntent::Graph(GraphTransaction::DuplicateSelection { entities, offset })]
            if entities == &vec![GraphEntityId::Node(source)] && *offset == (90.0, 180.0)
    ));
}

#[test]
fn cancel_action_targets_active_session_layer_first() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let frame_id = add_frame(
        &mut document,
        &catalog,
        &registry,
        (0.0, 0.0),
        (260.0, 160.0),
    );
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    session.open_search(WorldPoint::new(1.0, 2.0), None);
    let mut coordinator = NodeFlowCoordinator::new();

    let intents = coordinator.action_intents(
        ActionId::NODE_FLOW_CANCEL,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center: WorldPoint::new(0.0, 0.0),
            grid_unit: 1.0,
        },
    );

    assert!(matches!(
        &intents[..],
        [NodeEditorIntent::Session(SessionIntent::Search(
            NodeSearchIntent::Close
        ))]
    ));

    session.close_search();
    assert!(session.begin_frame_resize(frame_id, (260.0, 160.0)));
    let intents = coordinator.action_intents(
        ActionId::NODE_FLOW_CANCEL,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center: WorldPoint::new(0.0, 0.0),
            grid_unit: 1.0,
        },
    );

    assert!(matches!(
        &intents[..],
        [NodeEditorIntent::Session(SessionIntent::CancelFrameResize)]
    ));

    session.cancel_frame_resize();
    assert!(session.begin_inline_edit(CanvasEntityId::Frame(frame_id), "Group"));
    let intents = coordinator.action_intents(
        ActionId::NODE_FLOW_CANCEL,
        NodeFlowActionContext {
            session: &session,
            document: &document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center: WorldPoint::new(0.0, 0.0),
            grid_unit: 1.0,
        },
    );

    assert!(matches!(
        &intents[..],
        [NodeEditorIntent::Session(SessionIntent::CancelInlineEdit)]
    ));
}

#[test]
fn coordinator_handle_event_routes_key_actions_without_mutating_authorities() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let projection = projection(&document, &catalog, &registry);
    let mut session = NodeEditorSession::new();
    session.apply_selection_intent(
        SelectionIntent::Replace(CanvasEntityId::Node(source)),
        &projection,
    );
    let mut coordinator = NodeFlowCoordinator::new();

    coordinator.handle_event(
        NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        },
        &key_event(KeyCode::Delete, true, false),
    );

    assert_eq!(document.nodes().count(), 1);
    assert!(matches!(
        &coordinator.take_intents()[..],
        [NodeEditorIntent::Graph(GraphTransaction::RemoveNodes { node_ids })]
            if node_ids == &vec![source]
    ));

    coordinator.handle_event(
        NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        },
        &key_event(KeyCode::Delete, false, false),
    );

    assert!(coordinator.take_intents().is_empty());
}

#[test]
fn coordinator_handle_event_uses_default_bindings_and_read_only_gate() {
    let (catalog, registry) = catalog_and_registry();
    let document = NodeGraphDocument::new(1, "graph");
    let mut session = NodeEditorSession::new();
    session.viewport = CanvasViewportState {
        transform: CanvasTransform {
            offset_x: 10.0,
            offset_y: 20.0,
            zoom: 2.0,
        },
    };
    let mut coordinator = NodeFlowCoordinator::new().with_viewport_size(800.0, 600.0);

    coordinator.handle_event(
        NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        },
        &key_event(KeyCode::Tab, true, false),
    );

    assert!(matches!(
        &coordinator.take_intents()[..],
        [NodeEditorIntent::Session(SessionIntent::Search(NodeSearchIntent::Open {
            position,
            dangling_wire: None,
        }))] if *position == WorldPoint::new(210.0, 170.0)
    ));

    let mut read_only = NodeFlowCoordinator::new().with_config(NodeFlowConfig {
        read_only: true,
        ..NodeFlowConfig::default()
    });
    read_only.handle_event(
        NodeFlowInputs {
            document: &document,
            session: &session,
            catalog: &catalog,
            port_registry: &registry,
            execution_overlay: None,
        },
        &key_event(KeyCode::Tab, true, false),
    );

    assert!(read_only.take_intents().is_empty());
}

#[test]
fn alignment_snap_resolves_edge_delta_and_vertical_guide() {
    let moving = [WorldRect::new(0.0, 0.0, 100.0, 60.0)];
    let stationary = [WorldRect::new(205.0, 20.0, 80.0, 80.0)];

    let result = resolve_alignment_snap(
        &moving,
        &stationary,
        (102.0, 11.0),
        AlignmentSnapConfig {
            structural_guides: false,
            ..AlignmentSnapConfig::default()
        },
    );

    assert_eq!(result.delta, (105.0, 11.0));
    assert_eq!(result.guides.len(), 1);
    assert_eq!(result.guides[0].axis, AlignmentGuideAxis::Vertical);
    assert_eq!(result.guides[0].role, AlignmentGuideRole::ActiveSnap);
    assert_eq!(result.guides[0].position, 205.0);

    let spatial = spatial_guides_from_alignment_guides(&result.guides);
    assert_eq!(spatial.len(), 1);
    assert_eq!(spatial[0].axis, SpatialGuideAxis::Vertical);
    assert_eq!(spatial[0].role, SpatialGuideRole::Active);
    assert_eq!(spatial[0].position, result.guides[0].position);
    assert_eq!(spatial[0].start, result.guides[0].span_start);
    assert_eq!(spatial[0].end, result.guides[0].span_end);
}

#[test]
fn alignment_snap_can_resolve_center_only_horizontal_guide() {
    let moving = [WorldRect::new(0.0, 0.0, 100.0, 40.0)];
    let stationary = [WorldRect::new(300.0, 75.0, 120.0, 50.0)];
    let config = AlignmentSnapConfig {
        snap_edges: false,
        snap_centers: true,
        structural_guides: false,
        ..AlignmentSnapConfig::default()
    };

    let result = resolve_alignment_snap(&moving, &stationary, (20.0, 76.0), config);

    assert_eq!(result.delta, (20.0, 80.0));
    assert_eq!(result.guides.len(), 1);
    assert_eq!(result.guides[0].axis, AlignmentGuideAxis::Horizontal);
    assert_eq!(result.guides[0].role, AlignmentGuideRole::ActiveSnap);
    assert_eq!(result.guides[0].position, 100.0);
}

#[test]
fn alignment_snap_adds_bounded_structural_guides_without_changing_delta() {
    let moving = [WorldRect::new(0.0, 0.0, 100.0, 60.0)];
    let stationary = [
        WorldRect::new(240.0, 0.0, 100.0, 60.0),
        WorldRect::new(0.0, 145.0, 100.0, 60.0),
    ];
    let config = AlignmentSnapConfig {
        tolerance: 4.0,
        structural_guides: true,
        structural_guides_per_axis: 1,
        structural_guide_radius: 60.0,
        ..AlignmentSnapConfig::default()
    };

    let result = resolve_alignment_snap(&moving, &stationary, (100.0, 80.0), config);

    assert_eq!(result.delta, (100.0, 80.0));
    assert!(
        result
            .guides
            .iter()
            .any(|guide| guide.role == AlignmentGuideRole::Structural)
    );
    assert!(
        result
            .guides
            .iter()
            .filter(|guide| guide.axis == AlignmentGuideAxis::Vertical
                && guide.role == AlignmentGuideRole::Structural)
            .count()
            <= 1
    );
    assert!(
        result
            .guides
            .iter()
            .filter(|guide| guide.axis == AlignmentGuideAxis::Horizontal
                && guide.role == AlignmentGuideRole::Structural)
            .count()
            <= 1
    );

    let spatial = spatial_guides_from_alignment_guides(&result.guides);
    assert!(
        spatial
            .iter()
            .any(|guide| guide.role == SpatialGuideRole::Structural)
    );
}
