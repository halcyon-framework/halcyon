use halcyon::core::event::{KeyCode, Modifiers, PointerButton};
use halcyon::core::math::{Point, RectF};
use halcyon::core::paint::Color;
use halcyon::model::canvas::{
    CanvasEntityId, CanvasHit, CanvasIntent, CanvasViewportState, HitPriority, WorldPoint,
    WorldRect,
};
use halcyon::model::node_graph::*;
use halcyon::model::node_graph_projection::*;
use halcyon::model::spatial::{
    CanvasSpatialBridge, SpatialAdapter, SpatialCoordinateMode, SpatialHitPriority, SpatialIntent,
    SpatialPointerFacts, SpatialProjectionError, SpatialRoutePlan, SpatialViewportState,
    SpatialVisualPolicy, realize_spatial_route_plan,
};
use halcyon::renderer::path::PathCommand;

fn viewport_rect() -> RectF {
    RectF::new(0.0, 0.0, 800.0, 600.0)
}

fn catalog_and_registry() -> (NodeTypeCatalog, PortTypeRegistry) {
    let number = PortTypeId(1);

    let mut registry = PortTypeRegistry::new();
    registry.register(PortTypeInfo {
        type_id: number,
        name: "Number".to_string(),
        color: Color::from_linear(0.25, 0.5, 0.75, 1.0),
        compatible_with: vec![number],
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

#[test]
fn projects_nodes_ports_and_canvas_hit_regions() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();

    assert_eq!(
        projection.revision().0,
        document.revision().0 * 31 + catalog.revision().0
    );
    assert_eq!(projection.nodes().len(), 1);
    assert_eq!(projection.ports().count(), 1);

    let node = &projection.nodes()[0];
    assert_eq!(node.id, source);
    assert_eq!(node.size(), (180.0, 56.0));
    assert_eq!(node.outputs[0].world_pos, WorldPoint::new(190.0, 64.0));
    assert_eq!(node.outputs[0].connection_count, 0);

    let hit = projection
        .canvas()
        .hit_test(WorldPoint::new(190.0, 64.0))
        .unwrap();
    assert_eq!(
        hit.id,
        CanvasEntityId::Port {
            node_id: source,
            port_id: 0,
            direction: PortDirection::Output
        }
    );
    assert_eq!(hit.priority, HitPriority::Port);
}

#[test]
fn spatial_bridge_preserves_node_graph_canvas_projection_parity() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let bridge = projection
        .spatial_bridge(
            CanvasViewportState::default(),
            viewport_rect(),
            SpatialVisualPolicy::default(),
        )
        .unwrap();
    let spatial = bridge.projection();

    assert_eq!(spatial.revision().0, projection.revision().0);
    assert_eq!(
        spatial.coordinate_mode(),
        SpatialCoordinateMode::AffineSurface
    );
    assert_eq!(spatial.items().len(), projection.canvas().iter().count());
    assert!(spatial.viewport().is_some());
    assert!(spatial.axis_orientation().is_some());

    let canvas_hit = projection
        .canvas()
        .hit_test(WorldPoint::new(190.0, 64.0))
        .unwrap();
    let spatial_hit = spatial.hit_test(WorldPoint::new(190.0, 64.0)).unwrap();
    assert_eq!(
        bridge.canvas_entity(spatial_hit.id),
        Some(CanvasEntityId::Port {
            node_id: source,
            port_id: 0,
            direction: PortDirection::Output
        })
    );
    assert_eq!(bridge.canvas_entity(spatial_hit.id), Some(canvas_hit.id));
    assert_eq!(spatial_hit.priority, SpatialHitPriority::Anchor);
}

#[test]
fn node_graph_spatial_adapter_matches_bridge_projection() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let viewport = SpatialViewportState::default();
    let axis_orientation = CanvasSpatialBridge::axis_orientation();
    let spatial = NodeGraphSpatialAdapter
        .build_projection(
            &projection,
            &NodeGraphProjectionSession,
            Some(&viewport),
            Some(&axis_orientation),
            &SpatialVisualPolicy::default(),
        )
        .unwrap();

    assert_eq!(spatial.revision().0, projection.revision().0);
    assert_eq!(
        spatial.coordinate_mode(),
        SpatialCoordinateMode::AffineSurface
    );
    assert_eq!(spatial.items().len(), projection.canvas().iter().count());
    assert_eq!(spatial.viewport(), Some(viewport));
    assert_eq!(spatial.axis_orientation(), Some(axis_orientation));

    assert_eq!(
        NodeGraphSpatialAdapter
            .build_projection(
                &projection,
                &NodeGraphProjectionSession,
                Some(&viewport),
                None,
                &SpatialVisualPolicy::default(),
            )
            .unwrap_err(),
        SpatialProjectionError::InvalidCoordinateModeAdmission {
            coordinate_mode: SpatialCoordinateMode::AffineSurface,
            viewport_present: true,
            axis_orientation_present: false,
        }
    );
    assert_eq!(
        NodeGraphSpatialAdapter
            .build_projection(
                &projection,
                &NodeGraphProjectionSession,
                None,
                Some(&axis_orientation),
                &SpatialVisualPolicy::default(),
            )
            .unwrap_err(),
        SpatialProjectionError::InvalidCoordinateModeAdmission {
            coordinate_mode: SpatialCoordinateMode::AffineSurface,
            viewport_present: false,
            axis_orientation_present: true,
        }
    );
}

#[test]
fn node_graph_spatial_adapter_translates_press_to_canvas_intent() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let bridge = projection
        .spatial_bridge(
            CanvasViewportState::default(),
            viewport_rect(),
            SpatialVisualPolicy::default(),
        )
        .unwrap();
    let surface_pos = WorldPoint::new(190.0, 64.0);
    let spatial_hit = bridge.projection().hit_test(surface_pos).unwrap();
    let intent = SpatialIntent::Press(SpatialPointerFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        item: Some(spatial_hit.id),
        surface_pos,
        screen_pos: Point::zero(),
    });

    let translated = NodeGraphSpatialAdapter
        .translate_intent(&intent, &bridge)
        .unwrap();

    assert_eq!(
        translated,
        CanvasIntent::Hit(CanvasHit {
            id: CanvasEntityId::Port {
                node_id: source,
                port_id: 0,
                direction: PortDirection::Output,
            },
            priority: HitPriority::Port,
            z_order: spatial_hit.z_order,
        })
    );
    assert_eq!(
        NodeGraphSpatialAdapter
            .translate_intent(&SpatialIntent::KeyCommand(KeyCode::Delete), &bridge),
        None
    );
}

#[test]
fn node_graph_spatial_adapter_keeps_mutation_authority_outside_spatial() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    add_node(&mut document, &catalog, &registry, 10, (10.0, 20.0));

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();
    let bridge = projection
        .spatial_bridge(
            CanvasViewportState::default(),
            viewport_rect(),
            SpatialVisualPolicy::default(),
        )
        .unwrap();
    let facts = SpatialPointerFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        item: None,
        surface_pos: WorldPoint::new(10.0, 20.0),
        screen_pos: Point::zero(),
    };

    for intent in [
        SpatialIntent::DragStarted(facts),
        SpatialIntent::DragUpdated {
            facts,
            delta: WorldPoint::new(3.0, 4.0),
        },
        SpatialIntent::DragCommitted(facts),
        SpatialIntent::DragCanceled,
        SpatialIntent::KeyCommand(KeyCode::Delete),
        SpatialIntent::KeyCommand(KeyCode::Backspace),
    ] {
        assert_eq!(
            NodeGraphSpatialAdapter.translate_intent(&intent, &bridge),
            None
        );
    }

    let rect = WorldRect::new(0.0, 0.0, 40.0, 30.0);
    assert_eq!(
        NodeGraphSpatialAdapter
            .translate_intent(&SpatialIntent::RubberBandCommitted(rect), &bridge),
        Some(CanvasIntent::RubberBandCommitted(rect))
    );
}

#[test]
fn projects_wire_from_connection_endpoints() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (300.0, 0.0));
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

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();

    assert_eq!(projection.wires().len(), 1);
    let wire = &projection.wires()[0];
    assert_eq!(wire.from_pos, WorldPoint::new(180.0, 44.0));
    assert_eq!(wire.to_pos, WorldPoint::new(300.0, 44.0));
    assert_eq!(wire.color, Some(Color::from_linear(0.25, 0.5, 0.75, 1.0)));
    assert!(matches!(wire.path.commands[0], PathCommand::MoveTo(_, _)));
    assert!(matches!(
        wire.path.commands[1],
        PathCommand::CubicTo(_, _, _, _, _, _)
    ));

    assert_eq!(
        projection
            .canvas()
            .hit_test(WorldPoint::new(240.0, 44.0))
            .unwrap()
            .id,
        CanvasEntityId::Wire(wire.id)
    );
    assert_eq!(
        projection
            .ports()
            .map(|port| (port.node_id, port.connection_count))
            .collect::<Vec<_>>(),
        vec![(source, 1), (sink, 1)]
    );
}

#[test]
fn node_graph_spatial_projection_carries_right_angle_route_plan() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (0.0, 0.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (300.0, 0.0));
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

    let config = NodeGraphProjectionConfig {
        wire_style: WireStyle::RightAngle,
        ..NodeGraphProjectionConfig::default()
    };
    let projection = NodeGraphProjection::build(
        &document,
        &catalog,
        &registry,
        &config,
        &NodeGraphProjectionState::default(),
    )
    .unwrap();
    let wire = &projection.wires()[0];
    let bridge = projection
        .spatial_bridge(
            CanvasViewportState::default(),
            viewport_rect(),
            SpatialVisualPolicy::default(),
        )
        .unwrap();
    let spatial_id = bridge
        .spatial_id_for_canvas(CanvasEntityId::Wire(wire.id))
        .unwrap();

    assert!(bridge.projection().routes().is_empty());

    let spatial = projection
        .spatial_projection(
            Some(SpatialViewportState::default()),
            Some(CanvasSpatialBridge::axis_orientation()),
            SpatialVisualPolicy::default(),
        )
        .unwrap();
    let plan = spatial.routes().get(&spatial_id).unwrap();
    let SpatialRoutePlan::RightAngle(route) = *plan else {
        panic!("expected right-angle route plan");
    };
    assert_eq!(
        route.points,
        [
            Point::new(180.0, 44.0),
            Point::new(240.0, 44.0),
            Point::new(240.0, 44.0),
            Point::new(300.0, 44.0),
        ]
    );

    let realized = realize_spatial_route_plan(*plan);
    assert_eq!(realized.segments()[0].from, Point::new(180.0, 44.0));
    assert_eq!(realized.segments()[0].to, Point::new(240.0, 44.0));
    assert_eq!(realized.segments()[2].to, Point::new(300.0, 44.0));
}

#[test]
fn frame_containment_uses_smallest_enclosing_frame() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (20.0, 20.0));
    document
        .apply(
            GraphTransaction::AddFrame {
                title: "Outer".to_string(),
                position: (0.0, 0.0),
                size: (400.0, 300.0),
                color: (10, 20, 30),
            },
            context(&catalog, &registry),
        )
        .unwrap();
    document
        .apply(
            GraphTransaction::AddFrame {
                title: "Inner".to_string(),
                position: (10.0, 10.0),
                size: (240.0, 160.0),
                color: (10, 20, 30),
            },
            context(&catalog, &registry),
        )
        .unwrap();

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();

    let inner = projection
        .frames()
        .iter()
        .find(|frame| frame.title == "Inner")
        .unwrap();
    let outer = projection
        .frames()
        .iter()
        .find(|frame| frame.title == "Outer")
        .unwrap();
    assert_eq!(inner.contained_nodes, vec![source]);
    assert!(outer.contained_nodes.is_empty());

    let draw_order: Vec<CanvasEntityId> = projection
        .canvas()
        .in_draw_order()
        .into_iter()
        .map(|item| item.id)
        .collect();
    assert!(matches!(draw_order[0], CanvasEntityId::Frame(_)));
    assert_eq!(
        *draw_order.last().unwrap(),
        CanvasEntityId::Port {
            node_id: source,
            port_id: 0,
            direction: PortDirection::Output,
        }
    );
}

#[test]
fn collapsed_frame_hides_contained_nodes_ports_and_wires() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let source = add_node(&mut document, &catalog, &registry, 10, (20.0, 20.0));
    let sink = add_node(&mut document, &catalog, &registry, 20, (320.0, 20.0));
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
    let frame_id = match document
        .apply(
            GraphTransaction::AddFrame {
                title: "Group".to_string(),
                position: (0.0, 0.0),
                size: (260.0, 160.0),
                color: (10, 20, 30),
            },
            context(&catalog, &registry),
        )
        .unwrap()
        .delta
    {
        GraphDelta::AddFrame { frame } => frame.id,
        _ => panic!("expected AddFrame delta"),
    };
    document
        .apply(
            GraphTransaction::SetFrameCollapsed {
                frame_id,
                collapsed: true,
            },
            context(&catalog, &registry),
        )
        .unwrap();

    let projection = NodeGraphProjection::from_document(&document, &catalog, &registry).unwrap();

    assert_eq!(projection.frames()[0].contained_nodes, vec![source]);
    assert_eq!(
        projection
            .nodes()
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        vec![sink]
    );
    assert!(projection.wires().is_empty());
    assert!(
        projection
            .canvas()
            .iter()
            .all(|item| item.id != CanvasEntityId::Node(source))
    );
    assert!(!projection.ports().any(|port| port.node_id == source));
}

#[test]
fn projection_reports_unknown_catalog_types() {
    let (catalog, registry) = catalog_and_registry();
    let mut document = NodeGraphDocument::new(1, "graph");
    let _ = document
        .apply(
            GraphTransaction::AddNode {
                type_id: 10,
                position: (0.0, 0.0),
                title: None,
            },
            context(&catalog, &registry),
        )
        .unwrap();
    let empty_catalog = NodeTypeCatalog::new();

    let error =
        NodeGraphProjection::from_document(&document, &empty_catalog, &registry).unwrap_err();

    assert!(matches!(
        error,
        NodeGraphProjectionError::UnknownNodeType {
            node_id: _,
            type_id: 10
        }
    ));
}
