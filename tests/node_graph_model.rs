use std::cell::Cell;

use halcyon::core::paint::Color;
use halcyon::model::node_graph::*;

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
        name: "Source".to_string(),
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
        name: "Sink".to_string(),
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
        name: "BooleanSink".to_string(),
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

struct CountingValidator {
    calls: Cell<u32>,
    saw_compound: Cell<bool>,
}

impl GraphValidator for CountingValidator {
    fn validate_transaction(
        &self,
        _graph: &NodeGraph,
        transaction: &GraphTransaction,
        _catalog: &NodeTypeCatalog,
        _registry: &PortTypeRegistry,
    ) -> Result<(), GraphError> {
        self.calls.set(self.calls.get() + 1);
        if matches!(transaction, GraphTransaction::Compound(_)) {
            self.saw_compound.set(true);
        }
        Ok(())
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
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddFrame {
                title: "Group".to_string(),
                position: (0.0, 0.0),
                size: (240.0, 160.0),
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

fn add_connection(
    document: &mut NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    from_node: u64,
    to_node: u64,
) -> u64 {
    let commit = document
        .apply(
            GraphTransaction::AddConnection {
                from_node,
                from_port: 0,
                to_node,
                to_port: 0,
            },
            context(catalog, registry),
        )
        .unwrap();
    match commit.delta {
        GraphDelta::AddConnection { connection } => connection.id,
        _ => panic!("expected AddConnection delta"),
    }
}

#[test]
fn graph_transactions_add_connect_and_sort() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");

    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (100.0, 20.0));
    let commit = document
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

    assert_eq!(commit.revision, GraphRevision(3));
    assert_eq!(document.revision(), GraphRevision(3));
    assert_eq!(document.connections_for_node(source).len(), 1);
    assert_eq!(document.topological_sort().unwrap(), vec![source, sink]);
    assert!(!document.has_cycles());
}

#[test]
fn frame_title_transaction_undo_redo_restores_exactly() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let frame_id = add_frame(&mut document, &catalog, &registry);

    let commit = document
        .apply(
            GraphTransaction::SetFrameTitle {
                frame_id,
                title: "Renamed".to_string(),
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(matches!(
        commit.delta,
        GraphDelta::SetFrameTitle {
            id,
            ref old_title,
            ref new_title,
        } if id == frame_id && old_title == "Group" && new_title == "Renamed"
    ));
    assert_eq!(document.frame(frame_id).unwrap().title, "Renamed");
    assert!(document.undo());
    assert_eq!(document.frame(frame_id).unwrap().title, "Group");
    assert!(document.redo());
    assert_eq!(document.frame(frame_id).unwrap().title, "Renamed");
}

#[test]
fn move_nodes_deduplicates_targets_for_exact_undo_redo() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));

    let commit = document
        .apply(
            GraphTransaction::MoveNodes {
                moves: vec![(source, (10.0, 10.0)), (source, (30.0, 40.0))],
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert_eq!(commit.revision, GraphRevision(2));
    assert!(matches!(
        commit.delta,
        GraphDelta::MoveNode {
            id,
            old_pos: (0.0, 0.0),
            new_pos: (30.0, 40.0),
        } if id == source
    ));
    assert_eq!(document.node(source).unwrap().position, (30.0, 40.0));
    assert!(document.undo());
    assert_eq!(document.revision(), GraphRevision(3));
    assert_eq!(document.node(source).unwrap().position, (0.0, 0.0));
    assert!(document.redo());
    assert_eq!(document.revision(), GraphRevision(4));
    assert_eq!(document.node(source).unwrap().position, (30.0, 40.0));
}

#[test]
fn move_frame_with_nodes_deduplicates_targets_for_exact_undo_redo() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let node_id = add_node(&mut document, &catalog, &registry, 10, (5.0, 5.0));
    let frame_id = add_frame(&mut document, &catalog, &registry);

    let commit = document
        .apply(
            GraphTransaction::MoveFrameWithNodes {
                frame_id,
                new_pos: (50.0, 50.0),
                node_moves: vec![(node_id, (15.0, 15.0)), (node_id, (25.0, 30.0))],
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(matches!(
        commit.delta,
        GraphDelta::MoveFrameWithNodes {
            frame_id: id,
            frame_old_pos: (0.0, 0.0),
            frame_new_pos: (50.0, 50.0),
            ref node_moves,
        } if id == frame_id
            && node_moves == &vec![(node_id, (5.0, 5.0), (25.0, 30.0))]
    ));
    assert_eq!(document.frame(frame_id).unwrap().position, (50.0, 50.0));
    assert_eq!(document.node(node_id).unwrap().position, (25.0, 30.0));
    assert!(document.undo());
    assert_eq!(document.frame(frame_id).unwrap().position, (0.0, 0.0));
    assert_eq!(document.node(node_id).unwrap().position, (5.0, 5.0));
    assert!(document.redo());
    assert_eq!(document.frame(frame_id).unwrap().position, (50.0, 50.0));
    assert_eq!(document.node(node_id).unwrap().position, (25.0, 30.0));
}

#[test]
fn undo_stack_depth_drops_oldest_delta() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph").with_max_undo_depth(2);
    let first = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let second = add_node(&mut document, &catalog, &registry, 10, (10.0, 0.0));
    let third = add_node(&mut document, &catalog, &registry, 10, (20.0, 0.0));

    assert_eq!(document.revision(), GraphRevision(3));
    assert!(document.undo());
    assert_eq!(document.revision(), GraphRevision(4));
    assert!(document.node(third).is_none());
    assert!(document.undo());
    assert_eq!(document.revision(), GraphRevision(5));
    assert!(document.node(second).is_none());
    assert!(!document.undo());
    assert_eq!(document.revision(), GraphRevision(5));
    assert!(document.node(first).is_some());
}

#[test]
fn new_successful_apply_clears_redo_stack() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let first = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let second = add_node(&mut document, &catalog, &registry, 10, (10.0, 0.0));

    assert!(document.undo());
    assert_eq!(document.revision(), GraphRevision(3));
    assert!(document.can_redo());
    assert!(document.node(second).is_none());

    document
        .apply(
            GraphTransaction::SetNodeTitle {
                node_id: first,
                title: "Retitled".to_string(),
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert_eq!(document.revision(), GraphRevision(4));
    assert!(!document.can_redo());
    assert!(!document.redo());
    assert_eq!(document.revision(), GraphRevision(4));
    assert!(document.node(second).is_none());
    assert_eq!(document.node(first).unwrap().title, "Retitled");
}

#[test]
fn failed_transaction_is_atomic() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));

    let result = document.apply(
        GraphTransaction::AddConnection {
            from_node: source,
            from_port: 99,
            to_node: 1000,
            to_port: 0,
        },
        context(&catalog, &registry),
    );

    assert!(result.is_err());
    assert_eq!(document.revision(), GraphRevision(1));
    assert_eq!(document.connections().count(), 0);
    assert!(!document.can_undo() || document.nodes().count() == 1);
}

#[test]
fn remove_node_cascades_and_undo_redo_restores_exactly() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (100.0, 0.0));
    add_connection(&mut document, &catalog, &registry, source, sink);

    document
        .apply(
            GraphTransaction::RemoveNode { node_id: source },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(document.node(source).is_none());
    assert_eq!(document.connections().count(), 0);
    assert!(document.undo());
    assert!(document.node(source).is_some());
    assert_eq!(document.connections().count(), 1);
    assert!(document.redo());
    assert!(document.node(source).is_none());
    assert_eq!(document.connections().count(), 0);
}

#[test]
fn remove_nodes_bulk_delete_is_one_undo_step() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (100.0, 0.0));
    add_connection(&mut document, &catalog, &registry, source, sink);

    let commit = document
        .apply(
            GraphTransaction::RemoveNodes {
                node_ids: vec![source, sink, source],
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(matches!(commit.delta, GraphDelta::Compound(ref deltas) if deltas.len() == 2));
    assert_eq!(document.nodes().count(), 0);
    assert_eq!(document.connections().count(), 0);
    assert!(document.undo());
    assert_eq!(document.nodes().count(), 2);
    assert_eq!(document.connections().count(), 1);
    assert!(document.redo());
    assert_eq!(document.nodes().count(), 0);
    assert_eq!(document.connections().count(), 0);
}

#[test]
fn remove_and_replace_connection_undo_redo_restore_exactly() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let first_source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let second_source = add_node(&mut document, &catalog, &registry, 10, (50.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (100.0, 0.0));
    let old_connection = add_connection(&mut document, &catalog, &registry, first_source, sink);

    let remove_commit = document
        .apply(
            GraphTransaction::RemoveConnection {
                connection_id: old_connection,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    assert!(matches!(
        remove_commit.delta,
        GraphDelta::RemoveConnection { ref connection } if connection.id == old_connection
    ));
    assert!(document.connection(old_connection).is_none());
    assert!(document.undo());
    assert!(document.connection(old_connection).is_some());

    let replace_commit = document
        .apply(
            GraphTransaction::ReplaceConnection {
                old_connection_id: old_connection,
                from_node: second_source,
                from_port: 0,
                to_node: sink,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();

    let new_connection = match replace_commit.delta {
        GraphDelta::Compound(ref deltas) if deltas.len() == 2 => match &deltas[1] {
            GraphDelta::AddConnection { connection } => connection.id,
            _ => panic!("expected AddConnection delta"),
        },
        _ => panic!("expected compound replace delta"),
    };
    assert!(document.connection(old_connection).is_none());
    assert_eq!(
        document
            .connection(new_connection)
            .map(|connection| (connection.from_node, connection.to_node)),
        Some((second_source, sink))
    );
    assert!(document.undo());
    assert!(document.connection(old_connection).is_some());
    assert!(document.connection(new_connection).is_none());
    assert!(document.redo());
    assert!(document.connection(old_connection).is_none());
    assert!(document.connection(new_connection).is_some());
}

#[test]
fn compound_duplicate_selection_is_one_undo_step() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (100.0, 0.0));
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

    let commit = document
        .apply(
            GraphTransaction::DuplicateSelection {
                entities: vec![GraphEntityId::Node(source), GraphEntityId::Node(sink)],
                offset: (20.0, 20.0),
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(matches!(commit.delta, GraphDelta::Compound(_)));
    assert_eq!(document.nodes().count(), 4);
    assert_eq!(document.connections().count(), 2);
    assert!(document.undo());
    assert_eq!(document.nodes().count(), 2);
    assert_eq!(document.connections().count(), 1);
}

#[test]
fn compound_transaction_validator_runs_once_for_whole_transaction() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let validator = CountingValidator {
        calls: Cell::new(0),
        saw_compound: Cell::new(false),
    };

    let commit = document
        .apply(
            GraphTransaction::Compound(vec![
                GraphTransaction::AddNode {
                    type_id: 10,
                    position: (0.0, 0.0),
                    title: None,
                },
                GraphTransaction::AddNode {
                    type_id: 20,
                    position: (100.0, 0.0),
                    title: None,
                },
            ]),
            GraphTransactionContext {
                catalog: &catalog,
                port_registry: &registry,
                validator: Some(&validator),
            },
        )
        .unwrap();

    assert_eq!(validator.calls.get(), 1);
    assert!(validator.saw_compound.get());
    assert_eq!(commit.revision, GraphRevision(1));
    assert!(matches!(commit.delta, GraphDelta::Compound(_)));
    assert_eq!(document.nodes().count(), 2);
    assert!(document.undo());
    assert_eq!(document.nodes().count(), 0);
}

#[test]
fn serialization_round_trip_preserves_graph_not_runtime_state() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(77, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    document
        .apply(
            GraphTransaction::SetParameter {
                node_id: source,
                key: "gain".to_string(),
                value: ParameterValue::Float(0.75),
            },
            context(&catalog, &registry),
        )
        .unwrap();

    let json = serde_json::to_string(&document.to_serializable()).unwrap();
    let graph: SerializableGraph = serde_json::from_str(&json).unwrap();
    let loaded = NodeGraphDocument::from_serializable(graph).unwrap();

    assert_eq!(loaded.to_serializable(), document.to_serializable());
    assert_eq!(loaded.revision(), GraphRevision(0));
    assert!(!loaded.can_undo());
    assert!(!loaded.can_redo());
}

#[test]
fn from_serializable_rejects_invalid_frame_size() {
    let graph: SerializableGraph = serde_json::from_str(
        r#"{
            "id": 1,
            "name": "invalid-frame",
            "nodes": {},
            "connections": {},
            "frames": {
                "7": {
                    "id": 7,
                    "title": "Bad Frame",
                    "description": null,
                    "color": [10, 20, 30],
                    "position": [0.0, 0.0],
                    "size": [0.0, 120.0],
                    "collapsed": false
                }
            },
            "metadata": {
                "created_at": null,
                "modified_at": null,
                "version": 1,
                "custom": {}
            }
        }"#,
    )
    .unwrap();

    let error = NodeGraphDocument::from_serializable(graph).unwrap_err();
    assert!(matches!(
        error,
        GraphError::InvalidFrameSize { size: (0.0, 120.0) }
    ));
}

#[test]
fn validation_detects_connection_rule_violations() {
    let (catalog, registry) = catalog_and_registry();
    let graph: SerializableGraph = serde_json::from_str(
        r#"{
            "id": 1,
            "name": "invalid",
            "nodes": {
                "1": {
                    "id": 1,
                    "type_id": 10,
                    "position": [0.0, 0.0],
                    "title": "Source A",
                    "collapsed": false,
                    "parameters": {}
                },
                "2": {
                    "id": 2,
                    "type_id": 10,
                    "position": [10.0, 0.0],
                    "title": "Source B",
                    "collapsed": false,
                    "parameters": {}
                },
                "3": {
                    "id": 3,
                    "type_id": 20,
                    "position": [20.0, 0.0],
                    "title": "Sink",
                    "collapsed": false,
                    "parameters": {}
                },
                "4": {
                    "id": 4,
                    "type_id": 30,
                    "position": [30.0, 0.0],
                    "title": "Boolean Sink",
                    "collapsed": false,
                    "parameters": {}
                },
                "5": {
                    "id": 5,
                    "type_id": 999,
                    "position": [40.0, 0.0],
                    "title": "Unknown",
                    "collapsed": false,
                    "parameters": {}
                }
            },
            "connections": {
                "10": { "id": 10, "from_node": 1, "from_port": 0, "to_node": 3, "to_port": 0 },
                "11": { "id": 11, "from_node": 1, "from_port": 0, "to_node": 3, "to_port": 0 },
                "12": { "id": 12, "from_node": 2, "from_port": 0, "to_node": 3, "to_port": 0 },
                "13": { "id": 13, "from_node": 1, "from_port": 0, "to_node": 4, "to_port": 0 },
                "14": { "id": 14, "from_node": 999, "from_port": 0, "to_node": 3, "to_port": 0 },
                "15": { "id": 15, "from_node": 3, "from_port": 0, "to_node": 1, "to_port": 0 },
                "16": { "id": 16, "from_node": 1, "from_port": 99, "to_node": 3, "to_port": 0 }
            },
            "frames": {},
            "metadata": {
                "created_at": null,
                "modified_at": null,
                "version": 1,
                "custom": {}
            }
        }"#,
    )
    .unwrap();
    let document = NodeGraphDocument::from_serializable(graph).unwrap();
    let errors = document.validate(&catalog, &registry);

    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::UnknownNodeType {
            node_id: 5,
            type_id: 999
        }
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::DuplicateConnection {
            from: (1, 0),
            to: (3, 0)
        }
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::CardinalityViolation {
            node_id: 3,
            port_id: 0,
            ..
        }
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::TypeMismatch {
            connection_id: 13,
            ..
        }
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::DanglingConnection {
            connection_id: 14,
            missing_node: 999
        }
    )));
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, GraphError::DirectionMismatch { connection_id: 15 }))
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        GraphError::UnknownPort {
            node_id: 1,
            port_id: 99
        }
    )));
}

#[test]
fn validate_reports_cycles_without_forcing_transaction_policy() {
    let (mut catalog, registry) = catalog_and_registry();
    catalog.register(NodeType {
        type_id: 40,
        name: "Pass".to_string(),
        category: "Math".to_string(),
        default_title: "Pass".to_string(),
        default_color: Color::WHITE,
        input_ports: vec![PortDefinition {
            port_id: 0,
            label: "in".to_string(),
            direction: PortDirection::Input,
            type_id: PortTypeId(1),
            max_connections: PortCardinality::Multiple,
        }],
        output_ports: vec![PortDefinition {
            port_id: 1,
            label: "out".to_string(),
            direction: PortDirection::Output,
            type_id: PortTypeId(1),
            max_connections: PortCardinality::Multiple,
        }],
        resizable: false,
        collapsible: true,
    });

    let mut document = NodeGraphDocument::new(77, "graph");
    let first = add_node(&mut document, &catalog, &registry, 40, (0.0, 0.0));
    let second = add_node(&mut document, &catalog, &registry, 40, (100.0, 0.0));
    document
        .apply(
            GraphTransaction::AddConnection {
                from_node: first,
                from_port: 1,
                to_node: second,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    document
        .apply(
            GraphTransaction::AddConnection {
                from_node: second,
                from_port: 1,
                to_node: first,
                to_port: 0,
            },
            context(&catalog, &registry),
        )
        .unwrap();

    assert!(document.has_cycles());
    assert!(
        document
            .validate(&catalog, &registry)
            .iter()
            .any(|error| matches!(error, GraphError::CycleDetected { .. }))
    );
}
