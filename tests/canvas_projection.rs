use halcyon::model::canvas::*;
use halcyon::model::node_graph::PortDirection;
use halcyon::renderer::path::Path;

fn item(
    id: CanvasEntityId,
    bounds: WorldRect,
    z_order: i32,
    priority: HitPriority,
) -> ProjectedCanvasItem {
    ProjectedCanvasItem {
        id,
        bounds,
        z_order,
        hit_regions: vec![CanvasHitRegion::Rect {
            id,
            rect: bounds,
            priority,
        }],
        paint: CanvasPaintDescriptor::None,
    }
}

#[test]
fn transform_round_trip_uses_spec_formula() {
    let transform = CanvasTransform {
        offset_x: 100.0,
        offset_y: -50.0,
        zoom: 2.5,
    };
    let point = WorldPoint::new(140.0, -10.0);

    let screen = transform.world_to_screen(point);
    assert_eq!(screen, (100.0, 100.0));
    assert_eq!(transform.screen_to_world(screen.0, screen.1), point);
}

#[test]
fn zoom_around_screen_point_preserves_world_anchor() {
    let mut canvas = InfiniteCanvas::new().with_zoom_range(0.5, 4.0);
    let mut viewport = CanvasViewportState {
        transform: CanvasTransform {
            offset_x: 10.0,
            offset_y: 20.0,
            zoom: 1.0,
        },
    };
    let before = viewport.transform.screen_to_world(200.0, 100.0);

    canvas.zoom_around_screen_point(&mut viewport, 3.0, 200.0, 100.0);

    assert_eq!(viewport.transform.screen_to_world(200.0, 100.0), before);
    assert_eq!(viewport.transform.zoom, 3.0);
}

#[test]
fn zoom_to_fit_frames_projection_bounds() {
    let projection = CanvasProjection::new(
        ProjectionRevision(7),
        vec![item(
            CanvasEntityId::Node(1),
            WorldRect::new(100.0, 200.0, 300.0, 100.0),
            10,
            HitPriority::Node,
        )],
    );
    let mut canvas = InfiniteCanvas::new().with_viewport_size(800.0, 600.0);
    let mut viewport = CanvasViewportState::default();

    assert!(canvas.zoom_to_fit(&mut viewport, &projection));

    let left_top = viewport
        .transform
        .world_to_screen(WorldPoint::new(100.0, 200.0));
    let right_bottom = viewport
        .transform
        .world_to_screen(WorldPoint::new(400.0, 300.0));
    assert!(left_top.0 >= 31.9);
    assert!(left_top.1 >= 31.9);
    assert!(right_bottom.0 <= 768.1);
    assert!(right_bottom.1 <= 568.1);
}

#[test]
fn projection_draw_order_is_stable_by_z_then_projection_order() {
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![
            item(
                CanvasEntityId::Node(1),
                WorldRect::new(0.0, 0.0, 10.0, 10.0),
                20,
                HitPriority::Node,
            ),
            item(
                CanvasEntityId::Frame(1),
                WorldRect::new(0.0, 0.0, 10.0, 10.0),
                10,
                HitPriority::Frame,
            ),
            item(
                CanvasEntityId::Node(2),
                WorldRect::new(0.0, 0.0, 10.0, 10.0),
                20,
                HitPriority::Node,
            ),
        ],
    );
    let order: Vec<CanvasEntityId> = projection
        .in_draw_order()
        .into_iter()
        .map(|item| item.id)
        .collect();

    assert_eq!(
        order,
        vec![
            CanvasEntityId::Frame(1),
            CanvasEntityId::Node(1),
            CanvasEntityId::Node(2)
        ]
    );
}

#[test]
fn projection_hit_test_uses_priority_then_reverse_draw_order() {
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![
            item(
                CanvasEntityId::Wire(1),
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                90,
                HitPriority::Wire,
            ),
            item(
                CanvasEntityId::Node(1),
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                20,
                HitPriority::Node,
            ),
            item(
                CanvasEntityId::Port {
                    node_id: 1,
                    port_id: 0,
                    direction: PortDirection::Output,
                },
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                10,
                HitPriority::Port,
            ),
        ],
    );

    let hit = projection.hit_test(WorldPoint::new(50.0, 50.0)).unwrap();

    assert_eq!(
        hit.id,
        CanvasEntityId::Port {
            node_id: 1,
            port_id: 0,
            direction: PortDirection::Output
        }
    );
}

#[test]
fn equal_priority_hit_test_uses_reverse_projection_order() {
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![
            item(
                CanvasEntityId::Node(1),
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                20,
                HitPriority::Node,
            ),
            item(
                CanvasEntityId::Node(2),
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                20,
                HitPriority::Node,
            ),
        ],
    );

    assert_eq!(
        projection.hit_test(WorldPoint::new(1.0, 1.0)).unwrap().id,
        CanvasEntityId::Node(2)
    );
}

#[test]
fn region_query_and_world_bounds_use_projection_bounds() {
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![
            item(
                CanvasEntityId::Node(1),
                WorldRect::new(0.0, 0.0, 10.0, 10.0),
                10,
                HitPriority::Node,
            ),
            item(
                CanvasEntityId::Node(2),
                WorldRect::new(20.0, 5.0, 10.0, 10.0),
                10,
                HitPriority::Node,
            ),
        ],
    );

    assert_eq!(
        projection.world_bounds(),
        Some(WorldRect::new(0.0, 0.0, 30.0, 15.0))
    );
    assert_eq!(
        projection
            .in_region(&WorldRect::new(5.0, 0.0, 20.0, 20.0))
            .len(),
        2
    );
}

#[test]
fn path_hit_region_uses_distance_to_flattened_path() {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(100.0, 0.0);
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![ProjectedCanvasItem {
            id: CanvasEntityId::Wire(1),
            bounds: WorldRect::new(0.0, -5.0, 100.0, 10.0),
            z_order: 50,
            hit_regions: vec![CanvasHitRegion::Path {
                id: CanvasEntityId::Wire(1),
                path,
                tolerance: 5.0,
                priority: HitPriority::Wire,
            }],
            paint: CanvasPaintDescriptor::Wire(1),
        }],
    );

    assert!(projection.hit_test(WorldPoint::new(50.0, 4.9)).is_some());
    assert!(projection.hit_test(WorldPoint::new(50.0, 5.1)).is_none());
}

#[test]
fn canvas_intents_are_drained_without_projection_mutation() {
    let mut canvas = InfiniteCanvas::new();
    let mut viewport = CanvasViewportState::default();

    canvas.pan_to(&mut viewport, WorldPoint::new(10.0, 20.0));
    canvas.emit_empty_click(WorldPoint::new(1.0, 2.0));
    let intents = canvas.take_intents();

    assert_eq!(intents.len(), 2);
    assert!(matches!(intents[0], CanvasIntent::ViewportChanged(_)));
    assert_eq!(canvas.take_intents(), Vec::new());
}
