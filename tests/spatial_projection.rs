use std::cell::RefCell;
use std::rc::Rc;

use halcyon::core::event::{KeyCode, Modifiers, PointerButton};
use halcyon::core::math::{Point, RectF};
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::core::paint::Color;
use halcyon::engines::route_geometry::{
    RouteDirection, RouteEndpoint, RouteParams, route_right_angle,
};
use halcyon::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer,
    IntervalProjection, IntervalSource, IntervalStyle, LaneLayoutEngine, LanePolicy,
    MarkerDescriptor, MarkerStyle, RowBandDescriptor, RowGeometry, TemporalDescriptors,
    TemporalInvalidationEvent, TemporalSubstrate, TimeGeometryEngine, TimeScale,
};
use halcyon::model::canvas::*;
use halcyon::model::node_graph::PortDirection;
use halcyon::model::spatial::*;
use halcyon::model::time::{TimeRange, Timestamp};
use halcyon::plot::{
    AxisKind, PlotBezier, PlotCircle, PlotLine, PlotPath, PlotProjectionContext,
    PlotProjectionIntent, PlotProjectionKey, PlotProjectionSession, PlotRegion, PlotSpatialAdapter,
    PlotSpatialSnapshot, PlotStyledPath, PlotTag, PriceLevel, StyledSegment, TagAnchor, TagLine,
    Transform, ViewState,
};
use halcyon::renderer::path::Path;
use halcyon::widgets::timeline_canvas::TimelineCanvas;
use halcyon::widgets::timeline_spatial::{
    TimelineProjectionContext, TimelineProjectionIntent, TimelineProjectionKey,
    TimelineProjectionSession, TimelineSpatialAdapter, TimelineSpatialSnapshot,
};

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

fn increasing_pixel_orientation() -> SpatialAxisOrientation {
    SpatialAxisOrientation::new(
        SpatialDirection::IncreasingPixels,
        SpatialDirection::IncreasingPixels,
    )
}

fn canvas_viewport_rect() -> RectF {
    RectF::new(10.0, 20.0, 640.0, 480.0)
}

fn canvas_bridge(projection: &CanvasProjection) -> CanvasSpatialBridge {
    CanvasSpatialBridge::from_canvas_projection(
        projection,
        CanvasViewportState::default(),
        canvas_viewport_rect(),
        SpatialVisualPolicy::default(),
    )
    .unwrap()
}

#[test]
fn spatial_transform_preserves_canvas_formula_for_non_origin_rect() {
    let canvas = CanvasTransform {
        offset_x: 100.0,
        offset_y: -50.0,
        zoom: 2.5,
    };
    let rect = canvas_viewport_rect();
    let spatial =
        CanvasSpatialBridge::spatial_transform(CanvasViewportState { transform: canvas }, rect)
            .unwrap();
    let point = WorldPoint::new(140.0, -10.0);
    let (canvas_x, canvas_y) = canvas.world_to_screen(point);

    assert_eq!(
        spatial.world_to_screen(point).unwrap(),
        (rect.x + canvas_x, rect.y + canvas_y)
    );
    assert_eq!(
        spatial
            .screen_to_world(rect.x + 100.0, rect.y + 100.0)
            .unwrap(),
        canvas.screen_to_world(100.0, 100.0)
    );
    assert_eq!(
        CanvasSpatialBridge::canvas_viewport_state_for_rect(spatial.viewport(), rect)
            .unwrap()
            .transform,
        canvas
    );
}

#[test]
fn spatial_transform_round_trips_all_axis_orientations_with_equal_scale() {
    let viewport = SpatialViewportState::new(WorldPoint::new(12.0, -8.0), 2.5).unwrap();
    let rect = RectF::new(13.0, 29.0, 320.0, 180.0);
    let point = WorldPoint::new(20.0, 4.0);

    for x in [
        SpatialDirection::IncreasingPixels,
        SpatialDirection::DecreasingPixels,
    ] {
        for y in [
            SpatialDirection::IncreasingPixels,
            SpatialDirection::DecreasingPixels,
        ] {
            let orientation = SpatialAxisOrientation::new(x, y);
            let transform = SpatialTransform::new(viewport, orientation, rect).unwrap();
            let (pixel_x, pixel_y) = transform.world_to_screen(point).unwrap();
            let round_trip = transform.screen_to_world(pixel_x, pixel_y).unwrap();
            let bounds = transform.visible_world_bounds().unwrap();

            assert!((round_trip.x - point.x).abs() < 0.000_01);
            assert!((round_trip.y - point.y).abs() < 0.000_01);
            assert_eq!(bounds, WorldRect::new(-52.0, -44.0, 128.0, 72.0));
            assert_eq!(transform.axis_orientation(), orientation);
            assert_eq!(transform.viewport_rect(), rect);
        }
    }
}

#[test]
fn spatial_pan_and_anchored_zoom_replace_durable_camera_state() {
    let viewport = SpatialViewportState::new(WorldPoint::new(10.0, -20.0), 1.5).unwrap();
    let panned = viewport.pan(WorldPoint::new(3.0, 7.0)).unwrap();
    assert_eq!(panned.world_center(), WorldPoint::new(13.0, -13.0));
    assert_eq!(panned.pixels_per_world_unit(), 1.5);

    let rect = RectF::new(25.0, 40.0, 500.0, 300.0);
    let anchor_pixel = Point::new(171.25, 109.5);
    for orientation in [
        increasing_pixel_orientation(),
        SpatialAxisOrientation::new(
            SpatialDirection::IncreasingPixels,
            SpatialDirection::DecreasingPixels,
        ),
    ] {
        let old_transform = SpatialTransform::new(viewport, orientation, rect).unwrap();
        let anchor_world = old_transform
            .screen_to_world(anchor_pixel.x, anchor_pixel.y)
            .unwrap();
        let zoomed = old_transform
            .anchored_zoom_at_pixel(1.75, anchor_pixel)
            .unwrap();
        let new_transform = SpatialTransform::new(zoomed, orientation, rect).unwrap();
        let (new_pixel_x, new_pixel_y) = new_transform.world_to_screen(anchor_world).unwrap();

        assert!((new_pixel_x - anchor_pixel.x).abs() < 0.000_1);
        assert!((new_pixel_y - anchor_pixel.y).abs() < 0.000_1);
        assert_eq!(zoomed.pixels_per_world_unit(), 2.625);

        let world_anchored = old_transform
            .anchored_zoom_at_world(1.75, anchor_world)
            .unwrap();
        assert!((world_anchored.world_center().x - zoomed.world_center().x).abs() < 0.000_000_001);
        assert!((world_anchored.world_center().y - zoomed.world_center().y).abs() < 0.000_000_001);
        assert_eq!(
            world_anchored.pixels_per_world_unit(),
            zoomed.pixels_per_world_unit()
        );
    }
}

#[test]
fn spatial_viewport_rejects_invalid_and_overflowing_inputs() {
    assert_eq!(
        SpatialViewportState::new(WorldPoint::new(f64::NAN, 0.0), 1.0),
        Err(SpatialViewportError::NonFiniteWorldCenter)
    );
    assert_eq!(
        SpatialViewportState::new(WorldPoint::new(0.0, 0.0), f64::INFINITY),
        Err(SpatialViewportError::NonFinitePixelsPerWorldUnit)
    );
    assert_eq!(
        SpatialViewportState::new(WorldPoint::new(0.0, 0.0), 0.0),
        Err(SpatialViewportError::NonPositivePixelsPerWorldUnit)
    );

    let viewport = SpatialViewportState::default();
    assert_eq!(
        SpatialTransform::new(
            viewport,
            increasing_pixel_orientation(),
            RectF::new(0.0, 0.0, 0.0, 10.0),
        ),
        Err(SpatialViewportError::NonPositiveViewportRect)
    );
    let transform = SpatialTransform::new(
        viewport,
        increasing_pixel_orientation(),
        RectF::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap();
    assert_eq!(
        transform.world_to_screen(WorldPoint::new(f64::INFINITY, 0.0)),
        Err(SpatialViewportError::NonFiniteWorldPoint)
    );
    assert_eq!(
        transform.screen_to_world(f32::NAN, 0.0),
        Err(SpatialViewportError::NonFinitePixelPoint)
    );
    assert_eq!(
        transform.anchored_zoom_at_pixel(0.0, Point::zero()),
        Err(SpatialViewportError::NonPositiveZoomFactor)
    );
    assert_eq!(
        SpatialViewportState::new(WorldPoint::new(f64::MAX, 0.0), 1.0)
            .unwrap()
            .pan(WorldPoint::new(f64::MAX, 0.0)),
        Err(SpatialViewportError::ArithmeticOverflow)
    );
    let overflowing = SpatialTransform::new(
        SpatialViewportState::new(WorldPoint::new(0.0, 0.0), f64::MAX).unwrap(),
        increasing_pixel_orientation(),
        RectF::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap();
    assert_eq!(
        overflowing.world_to_screen(WorldPoint::new(2.0, 0.0)),
        Err(SpatialViewportError::ArithmeticOverflow)
    );
    assert_eq!(
        overflowing.anchored_zoom_at_pixel(2.0, Point::zero()),
        Err(SpatialViewportError::ArithmeticOverflow)
    );
}

#[test]
fn spatial_projection_coordinate_mode_admission_is_closed() {
    let viewport = SpatialViewportState::default();
    let orientation = increasing_pixel_orientation();
    let affine = SpatialProjection::new(
        SpatialProjectionRevision(1),
        Some(viewport),
        Some(orientation),
        SpatialCoordinateMode::AffineSurface,
        SpatialVisualPolicy::default(),
    )
    .unwrap();
    assert_eq!(affine.viewport(), Some(viewport));
    assert_eq!(affine.axis_orientation(), Some(orientation));

    let domain_mapped = SpatialProjection::new(
        SpatialProjectionRevision(2),
        None,
        None,
        SpatialCoordinateMode::DomainMappedSurface,
        SpatialVisualPolicy::default(),
    )
    .unwrap();
    assert_eq!(domain_mapped.viewport(), None);
    assert_eq!(domain_mapped.axis_orientation(), None);

    for (coordinate_mode, viewport, axis_orientation) in [
        (SpatialCoordinateMode::AffineSurface, None, None),
        (SpatialCoordinateMode::AffineSurface, Some(viewport), None),
        (
            SpatialCoordinateMode::AffineSurface,
            None,
            Some(orientation),
        ),
        (
            SpatialCoordinateMode::DomainMappedSurface,
            Some(viewport),
            Some(orientation),
        ),
        (
            SpatialCoordinateMode::DomainMappedSurface,
            Some(viewport),
            None,
        ),
        (
            SpatialCoordinateMode::DomainMappedSurface,
            None,
            Some(orientation),
        ),
    ] {
        let error = SpatialProjection::new(
            SpatialProjectionRevision(3),
            viewport,
            axis_orientation,
            coordinate_mode,
            SpatialVisualPolicy::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            SpatialProjectionError::InvalidCoordinateModeAdmission {
                coordinate_mode,
                viewport_present: viewport.is_some(),
                axis_orientation_present: axis_orientation.is_some(),
            }
        );
    }
}

#[test]
fn spatial_domain_mapped_adapters_reject_surplus_affine_inputs() {
    let viewport = SpatialViewportState::default();
    let orientation = increasing_pixel_orientation();
    let plot_error = PlotSpatialAdapter
        .build_projection(
            &empty_plot_snapshot(),
            &PlotProjectionSession,
            Some(&viewport),
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap_err();
    assert_eq!(
        plot_error,
        SpatialProjectionError::InvalidCoordinateModeAdmission {
            coordinate_mode: SpatialCoordinateMode::DomainMappedSurface,
            viewport_present: true,
            axis_orientation_present: false,
        }
    );

    let timeline_error = TimelineSpatialAdapter
        .build_projection(
            &timeline_spatial_snapshot(),
            &TimelineProjectionSession,
            None,
            Some(&orientation),
            &SpatialVisualPolicy::default(),
        )
        .unwrap_err();
    assert_eq!(
        timeline_error,
        SpatialProjectionError::InvalidCoordinateModeAdmission {
            coordinate_mode: SpatialCoordinateMode::DomainMappedSurface,
            viewport_present: false,
            axis_orientation_present: true,
        }
    );
}

#[test]
fn canvas_bridge_viewport_intent_round_trips_with_rect() {
    let projection = CanvasProjection::new(ProjectionRevision(1), Vec::new());
    let bridge = canvas_bridge(&projection);
    let canvas_transform = CanvasTransform {
        offset_x: -123.0,
        offset_y: 456.0,
        zoom: 3.25,
    };
    let spatial_intent = bridge
        .spatial_intent_from_canvas(CanvasIntent::ViewportChanged(canvas_transform))
        .unwrap();
    let SpatialIntent::ViewportChanged(viewport) = spatial_intent else {
        panic!("expected viewport intent");
    };

    assert_eq!(
        bridge.canvas_viewport_state(viewport).unwrap().transform,
        canvas_transform
    );
}

#[test]
fn canvas_bridge_keeps_projection_revision_bounds_and_hit_parity() {
    let projection = CanvasProjection::new(
        ProjectionRevision(9),
        vec![
            item(
                CanvasEntityId::Node(1),
                WorldRect::new(0.0, 0.0, 100.0, 100.0),
                20,
                HitPriority::Node,
            ),
            item(
                CanvasEntityId::Port {
                    node_id: 1,
                    port_id: 2,
                    direction: PortDirection::Output,
                },
                WorldRect::new(10.0, 10.0, 12.0, 12.0),
                30,
                HitPriority::Port,
            ),
        ],
    );

    let bridge = canvas_bridge(&projection);
    let spatial = bridge.projection();

    assert_eq!(spatial.revision(), SpatialProjectionRevision(9));
    assert_eq!(
        spatial.coordinate_mode(),
        SpatialCoordinateMode::AffineSurface
    );
    assert_eq!(
        spatial.world_bounds(),
        Some(WorldRect::new(0.0, 0.0, 100.0, 100.0))
    );
    let canvas_hit = projection.hit_test(WorldPoint::new(11.0, 11.0)).unwrap();
    let spatial_hit = spatial.hit_test(WorldPoint::new(11.0, 11.0)).unwrap();
    assert_eq!(bridge.canvas_entity(spatial_hit.id), Some(canvas_hit.id));
    assert_eq!(spatial_hit.priority, SpatialHitPriority::Anchor);
}

#[test]
fn canvas_bridge_keeps_hit_parity_when_priority_differs_from_item_role() {
    let entity = CanvasEntityId::Custom(44);
    let rect = WorldRect::new(0.0, 0.0, 20.0, 20.0);
    let projection = CanvasProjection::new(
        ProjectionRevision(11),
        vec![ProjectedCanvasItem {
            id: entity,
            bounds: rect,
            z_order: 5,
            hit_regions: vec![CanvasHitRegion::Rect {
                id: entity,
                rect,
                priority: HitPriority::Overlay,
            }],
            paint: CanvasPaintDescriptor::None,
        }],
    );

    let bridge = canvas_bridge(&projection);
    let canvas_hit = projection.hit_test(WorldPoint::new(10.0, 10.0)).unwrap();
    let spatial_hit = bridge
        .projection()
        .hit_test(WorldPoint::new(10.0, 10.0))
        .unwrap();

    assert_eq!(canvas_hit.id, entity);
    assert_eq!(canvas_hit.priority, HitPriority::Overlay);
    assert_eq!(spatial_hit.priority, SpatialHitPriority::Overlay);
    assert_eq!(bridge.canvas_entity(spatial_hit.id), Some(entity));
    assert!(bridge.spatial_id_for_canvas(entity).is_some());
}

#[test]
fn canvas_bridge_keeps_port_direction_identity_distinct() {
    let input = CanvasEntityId::Port {
        node_id: 1,
        port_id: 2,
        direction: PortDirection::Input,
    };
    let output = CanvasEntityId::Port {
        node_id: 1,
        port_id: 2,
        direction: PortDirection::Output,
    };
    let projection = CanvasProjection::new(
        ProjectionRevision(10),
        vec![
            item(
                input,
                WorldRect::new(0.0, 0.0, 12.0, 12.0),
                10,
                HitPriority::Port,
            ),
            item(
                output,
                WorldRect::new(20.0, 0.0, 12.0, 12.0),
                20,
                HitPriority::Port,
            ),
        ],
    );

    let bridge = canvas_bridge(&projection);
    let input_id = bridge.spatial_id_for_canvas(input).unwrap();
    let output_id = bridge.spatial_id_for_canvas(output).unwrap();

    assert_ne!(input_id, output_id);
    assert_eq!(bridge.canvas_entity(input_id), Some(input));
    assert_eq!(bridge.canvas_entity(output_id), Some(output));
}

#[test]
fn canvas_bridge_maps_canvas_roles_to_neutral_spatial_roles() {
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(100.0, 0.0);
    let projection = CanvasProjection::new(
        ProjectionRevision(1),
        vec![
            ProjectedCanvasItem {
                id: CanvasEntityId::Wire(7),
                bounds: WorldRect::new(0.0, -5.0, 100.0, 10.0),
                z_order: 10,
                hit_regions: vec![CanvasHitRegion::Path {
                    id: CanvasEntityId::Wire(7),
                    path,
                    tolerance: 5.0,
                    priority: HitPriority::Wire,
                }],
                paint: CanvasPaintDescriptor::Wire(7),
            },
            ProjectedCanvasItem {
                id: CanvasEntityId::Frame(3),
                bounds: WorldRect::new(-10.0, -10.0, 120.0, 80.0),
                z_order: 0,
                hit_regions: vec![CanvasHitRegion::Rect {
                    id: CanvasEntityId::Frame(3),
                    rect: WorldRect::new(-10.0, -10.0, 120.0, 80.0),
                    priority: HitPriority::Frame,
                }],
                paint: CanvasPaintDescriptor::Frame(3),
            },
        ],
    );

    let bridge = canvas_bridge(&projection);
    let roles: Vec<SpatialItemRole> = bridge
        .projection()
        .in_draw_order()
        .into_iter()
        .map(|item| item.role)
        .collect();

    assert_eq!(
        roles,
        vec![SpatialItemRole::Container, SpatialItemRole::Route]
    );
    let hit = bridge
        .projection()
        .hit_test(WorldPoint::new(50.0, 4.9))
        .unwrap();
    assert_eq!(bridge.canvas_entity(hit.id), Some(CanvasEntityId::Wire(7)));
}

#[test]
fn canvas_bridge_projects_text_bounds_for_text_bearing_items() {
    let node_bounds = WorldRect::new(0.0, 0.0, 120.0, 80.0);
    let frame_bounds = WorldRect::new(-20.0, -15.0, 180.0, 140.0);
    let projection = CanvasProjection::new(
        ProjectionRevision(2),
        vec![
            ProjectedCanvasItem {
                id: CanvasEntityId::Node(1),
                bounds: node_bounds,
                z_order: 10,
                hit_regions: vec![CanvasHitRegion::Rect {
                    id: CanvasEntityId::Node(1),
                    rect: node_bounds,
                    priority: HitPriority::Node,
                }],
                paint: CanvasPaintDescriptor::Node(1),
            },
            ProjectedCanvasItem {
                id: CanvasEntityId::Frame(2),
                bounds: frame_bounds,
                z_order: 0,
                hit_regions: vec![CanvasHitRegion::Rect {
                    id: CanvasEntityId::Frame(2),
                    rect: frame_bounds,
                    priority: HitPriority::Frame,
                }],
                paint: CanvasPaintDescriptor::Frame(2),
            },
        ],
    );
    let bridge = canvas_bridge(&projection);

    let node = bridge
        .projection()
        .items()
        .iter()
        .find(|item| bridge.canvas_entity(item.id) == Some(CanvasEntityId::Node(1)))
        .unwrap();
    assert!(node.primitives.iter().any(
        |primitive| matches!(primitive, SpatialPrimitive::Rect(rect) if *rect == node_bounds)
    ));
    assert!(node.primitives.iter().any(
        |primitive| matches!(primitive, SpatialPrimitive::TextBounds(rect) if *rect == node_bounds)
    ));

    let frame = bridge
        .projection()
        .items()
        .iter()
        .find(|item| bridge.canvas_entity(item.id) == Some(CanvasEntityId::Frame(2)))
        .unwrap();
    assert!(frame.primitives.iter().any(
        |primitive| matches!(primitive, SpatialPrimitive::Rect(rect) if *rect == frame_bounds)
    ));
    assert!(frame.primitives.iter().any(
        |primitive| matches!(primitive, SpatialPrimitive::TextBounds(rect) if *rect == frame_bounds)
    ));
}

#[test]
fn spatial_projection_world_bounds_include_label_bounds() {
    let item_bounds = WorldRect::new(0.0, 0.0, 10.0, 10.0);
    let label_bounds = WorldRect::new(20.0, -5.0, 30.0, 12.0);
    let projection = SpatialProjection::from_parts(
        SpatialProjectionRevision(3),
        None,
        None,
        SpatialCoordinateMode::DomainMappedSurface,
        vec![SpatialItem {
            id: SpatialItemId::from_opaque(42),
            role: SpatialItemRole::Item,
            bounds: item_bounds,
            z_order: 0,
            primitives: vec![
                SpatialPrimitive::Rect(item_bounds),
                SpatialPrimitive::TextBounds(label_bounds),
            ],
        }],
        Vec::new(),
        Default::default(),
        Vec::new(),
        Vec::new(),
        SpatialVisualPolicy::default(),
    )
    .unwrap();

    assert_eq!(projection.label_bounds(), vec![label_bounds]);
    assert_eq!(
        projection.world_bounds(),
        Some(WorldRect::new(0.0, -5.0, 50.0, 15.0))
    );
}

#[test]
fn spatial_projection_region_query_includes_label_bounds() {
    let item_bounds = WorldRect::new(0.0, 0.0, 10.0, 10.0);
    let label_bounds = WorldRect::new(20.0, -5.0, 30.0, 12.0);
    let item_id = SpatialItemId::from_opaque(42);
    let projection = SpatialProjection::from_parts(
        SpatialProjectionRevision(3),
        None,
        None,
        SpatialCoordinateMode::DomainMappedSurface,
        vec![SpatialItem {
            id: item_id,
            role: SpatialItemRole::Item,
            bounds: item_bounds,
            z_order: 0,
            primitives: vec![
                SpatialPrimitive::Rect(item_bounds),
                SpatialPrimitive::TextBounds(label_bounds),
            ],
        }],
        Vec::new(),
        Default::default(),
        Vec::new(),
        Vec::new(),
        SpatialVisualPolicy::default(),
    )
    .unwrap();

    let label_only_region = WorldRect::new(45.0, 2.0, 2.0, 2.0);
    let items = projection.in_region(&label_only_region);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, item_id);

    let empty_region = WorldRect::new(12.0, 12.0, 3.0, 3.0);
    assert!(projection.in_region(&empty_region).is_empty());
}

#[test]
fn spatial_route_plan_carries_route_geometry_without_recomputing_it() {
    let plan = route_right_angle(
        RouteEndpoint {
            point: Point::new(10.0, 20.0),
            dir: RouteDirection::Right,
        },
        RouteEndpoint {
            point: Point::new(100.0, 80.0),
            dir: RouteDirection::Left,
        },
        RouteParams {
            stub: 24.0,
            head_length: 8.0,
        },
    );
    let route = SpatialRoutePlan::RightAngle(plan);

    match route {
        SpatialRoutePlan::RightAngle(carried) => assert_eq!(carried, plan),
        SpatialRoutePlan::Polyline { .. } => panic!("expected right-angle plan"),
    }
}

#[test]
fn spatial_route_realization_preserves_right_angle_segment_axes() {
    let plan = route_right_angle(
        RouteEndpoint {
            point: Point::new(10.0, 20.0),
            dir: RouteDirection::Right,
        },
        RouteEndpoint {
            point: Point::new(42.0, 20.0),
            dir: RouteDirection::Left,
        },
        RouteParams {
            stub: 16.0,
            head_length: 16.0,
        },
    );
    let realized = realize_spatial_route_plan(SpatialRoutePlan::RightAngle(plan));
    let segments = realized.segments();

    assert_eq!(segments.len(), 3);
    assert_eq!(segments[0].axis, SpatialRouteSegmentAxis::Horizontal);
    assert_eq!(segments[1].axis, SpatialRouteSegmentAxis::Vertical);
    assert_eq!(segments[2].axis, SpatialRouteSegmentAxis::Horizontal);
    assert_eq!(segments[0].from, plan.points[0]);
    assert_eq!(segments[0].to, plan.points[1]);
    assert_eq!(segments[1].from, plan.points[1]);
    assert_eq!(segments[1].to, plan.points[2]);
    assert_eq!(segments[2].from, plan.points[2]);
    assert_eq!(segments[2].to, plan.points[3]);
    assert_eq!(realized.head.anchor, plan.head_anchor);
    assert_eq!(realized.head.dir, plan.head_dir);
}

#[test]
fn spatial_polyline_route_plan_preserves_supplied_geometry() {
    let points = [
        Point::new(0.0, 1.0),
        Point::new(12.0, 1.0),
        Point::new(12.0, 24.0),
    ];
    let route =
        SpatialRoutePlan::polyline(&points, Point::new(12.0, 24.0), RouteDirection::Right).unwrap();

    match &route {
        SpatialRoutePlan::Polyline {
            point_count,
            head_anchor,
            head_dir,
            ..
        } => {
            assert_eq!(*point_count, 3);
            assert_eq!(route.polyline_points(), Some(points.as_slice()));
            assert_eq!(*head_anchor, Point::new(12.0, 24.0));
            assert_eq!(*head_dir, RouteDirection::Right);
        }
        SpatialRoutePlan::RightAngle(_) => panic!("expected polyline plan"),
    }

    assert_eq!(
        SpatialRoutePlan::polyline(&points[..1], Point::zero(), RouteDirection::Right),
        None
    );
    assert_eq!(
        SpatialRoutePlan::polyline(
            &[
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(3.0, 0.0),
                Point::new(4.0, 0.0)
            ],
            Point::zero(),
            RouteDirection::Right
        ),
        None
    );
}

#[test]
fn spatial_route_realization_classifies_polyline_segment_axes() {
    let route = SpatialRoutePlan::polyline(
        &[
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 12.0),
            Point::new(16.0, 20.0),
        ],
        Point::new(16.0, 20.0),
        RouteDirection::Right,
    )
    .unwrap();
    let realized = realize_spatial_route_plan(route);
    let axes: Vec<_> = realized
        .segments()
        .iter()
        .map(|segment| segment.axis)
        .collect();

    assert_eq!(
        axes,
        vec![
            SpatialRouteSegmentAxis::Horizontal,
            SpatialRouteSegmentAxis::Vertical,
            SpatialRouteSegmentAxis::Freeform,
        ]
    );
}

fn plot_transform() -> Transform {
    Transform::from_view_state(&ViewState::new(0.0, 100.0, 0.0, 100.0), 200.0, 100.0)
}

fn empty_plot_snapshot() -> PlotSpatialSnapshot {
    PlotSpatialSnapshot {
        revision: SpatialProjectionRevision(44),
        transform: plot_transform(),
        price_levels: Vec::new(),
        regions: Vec::new(),
        tags: Vec::new(),
        circles: Vec::new(),
        lines: Vec::new(),
        beziers: Vec::new(),
        styled_paths: Vec::new(),
        paths: Vec::new(),
    }
}

#[derive(Debug, Clone, Default)]
struct TimelineSpatialSource {
    descriptors: TemporalDescriptors,
}

impl IntervalSource for TimelineSpatialSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.descriptors.intervals.clone());
        out.bands.extend(self.descriptors.bands.clone());
        out.arrows.extend(self.descriptors.arrows.clone());
        out.markers.extend(self.descriptors.markers.clone());
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        Vec::new()
    }

    fn interval_exists(&self, id: IntervalId) -> bool {
        self.descriptors
            .intervals
            .iter()
            .any(|interval| interval.id == id)
    }
}

fn timeline_spatial_snapshot() -> TimelineSpatialSnapshot {
    let mut source = TimelineSpatialSource::default();
    source.descriptors.intervals.extend([
        IntervalDescriptor {
            id: IntervalId(1),
            row: 10,
            range: TimeRange::new(Timestamp::from_secs(2), Timestamp::from_secs(8)),
            color: Color::WHITE,
            label: Some("alpha".to_string()),
            progress: 0.25,
            style: IntervalStyle::Solid,
            layer: IntervalLayer::Primary,
            lane: None,
        },
        IntervalDescriptor {
            id: IntervalId(2),
            row: 10,
            range: TimeRange::new(Timestamp::from_secs(3), Timestamp::from_secs(9)),
            color: Color::from_srgb(0.2, 0.6, 1.0, 1.0),
            label: None,
            progress: 0.0,
            style: IntervalStyle::Hollow,
            layer: IntervalLayer::Primary,
            lane: None,
        },
        IntervalDescriptor {
            id: IntervalId(3),
            row: 20,
            range: TimeRange::new(Timestamp::from_secs(12), Timestamp::from_secs(18)),
            color: Color::from_srgb(0.9, 0.3, 0.2, 1.0),
            label: None,
            progress: 0.0,
            style: IntervalStyle::Milestone,
            layer: IntervalLayer::Primary,
            lane: None,
        },
    ]);
    source.descriptors.bands.push(RowBandDescriptor {
        row: 10,
        range: TimeRange::new(Timestamp::from_secs(1), Timestamp::from_secs(4)),
        color: Color::from_srgb(0.1, 0.2, 0.3, 0.4),
    });
    source.descriptors.arrows.push(ArrowDescriptor {
        from: IntervalId(1),
        from_edge: IntervalEdge::End,
        to: IntervalId(3),
        to_edge: IntervalEdge::Start,
        critical: true,
        color: None,
    });
    source.descriptors.markers.push(MarkerDescriptor {
        ts: Timestamp::from_secs(10),
        color: Color::from_srgb(1.0, 0.8, 0.2, 1.0),
        label: Some("gate".to_string()),
        style: MarkerStyle::Flag,
    });

    let mut geometry =
        TimeGeometryEngine::new(Timestamp::from_secs(0), TimeScale::Seconds, 10.0, 220.0);
    geometry.set_now(Some(Timestamp::from_secs(15)));
    let resolution = geometry.resolve();
    let mut projection = IntervalProjection::new();
    let frame = projection
        .collect_frame(&source, Some(&[10, 20]), geometry.visible_range())
        .clone();
    let mut lanes = LaneLayoutEngine::new();
    lanes.resolve(&frame.intervals, LanePolicy::Auto);

    TimelineSpatialSnapshot::new(
        SpatialProjectionRevision(88),
        RectF::new(5.0, 7.0, 220.0, 90.0),
        resolution,
        frame,
        geometry,
        vec![
            RowGeometry {
                id: 10,
                y: 4.0,
                height: 40.0,
            },
            RowGeometry {
                id: 20,
                y: 50.0,
                height: 30.0,
            },
        ],
        lanes.lanes().to_vec(),
    )
}

fn derived_timeline_spatial_snapshot() -> TimelineSpatialSnapshot {
    let snapshot = timeline_spatial_snapshot();
    TimelineSpatialSnapshot::with_derived_revision(
        snapshot.canvas_rect,
        snapshot.resolution,
        snapshot.frame,
        snapshot.geometry,
        snapshot.rows,
        snapshot.lanes,
    )
}

#[test]
fn timeline_spatial_snapshot_derives_stable_nonzero_revision() {
    let first = derived_timeline_spatial_snapshot();
    let second = derived_timeline_spatial_snapshot();

    assert_ne!(first.revision, SpatialProjectionRevision(0));
    assert_eq!(first.revision, second.revision);
}

#[test]
fn timeline_spatial_snapshot_revision_tracks_projected_state() {
    let baseline = derived_timeline_spatial_snapshot();
    let mut changed = timeline_spatial_snapshot();
    changed.frame.intervals[0].progress = 0.5;
    let changed = TimelineSpatialSnapshot::with_derived_revision(
        changed.canvas_rect,
        changed.resolution,
        changed.frame,
        changed.geometry,
        changed.rows,
        changed.lanes,
    );

    assert_ne!(baseline.revision, changed.revision);
}

#[test]
fn plot_spatial_adapter_projects_only_domain_neutral_markers() {
    let mut snapshot = empty_plot_snapshot();
    snapshot.price_levels.push(PriceLevel {
        id: 1,
        y: 25.0,
        display_y: 25.0,
        color: Color::WHITE,
        draggable: true,
        ..PriceLevel::default()
    });
    snapshot.regions.push(PlotRegion {
        id: 2,
        x_min: Some(10.0),
        x_max: Some(20.0),
        y_min: Some(30.0),
        y_max: Some(40.0),
        fill_color: Color::from_srgb(0.2, 0.4, 0.8, 0.2),
        label: Some("range".to_string()),
        ..PlotRegion::default()
    });
    snapshot.tags.push(PlotTag {
        id: 3,
        x: 40.0,
        y: 40.0,
        tag: vec![TagLine::new("note")],
        color: Color::WHITE,
        anchor: TagAnchor::Above,
    });
    snapshot.circles.push(PlotCircle {
        id: 4,
        x: 50.0,
        y: 50.0,
        radius: 6.0,
        ..PlotCircle::default()
    });
    snapshot.lines.push(PlotLine {
        id: 5,
        x1: 10.0,
        y1: 10.0,
        x2: 90.0,
        y2: 10.0,
        width: 2.0,
        ..PlotLine::default()
    });
    snapshot
        .beziers
        .push(PlotBezier::arc(6, 10.0, 20.0, 90.0, 20.0, 0.2));
    snapshot.styled_paths.push(PlotStyledPath {
        id: 7,
        segments: vec![
            StyledSegment {
                x: 10.0,
                y: 80.0,
                color: Color::WHITE,
                width: 2.0,
                line_style: Default::default(),
            },
            StyledSegment {
                x: 50.0,
                y: 70.0,
                color: Color::WHITE,
                width: 3.0,
                line_style: Default::default(),
            },
        ],
        group: None,
    });
    snapshot.paths.push(PlotPath {
        id: 8,
        points: vec![(10.0, 90.0), (50.0, 95.0), (90.0, 90.0)],
        width: 2.0,
        ..PlotPath::default()
    });

    let projection = PlotSpatialAdapter
        .build_projection(
            &snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();

    assert_eq!(
        projection.coordinate_mode(),
        SpatialCoordinateMode::DomainMappedSurface
    );
    assert_eq!(projection.viewport(), None);
    assert_eq!(projection.axis_orientation(), None);
    assert_eq!(projection.items().len(), 8);
    assert_eq!(projection.hit_regions().len(), 6);
    assert_eq!(projection.routes().len(), 0);
    assert_eq!(projection.guides().len(), 0);

    let roles: Vec<_> = projection.items().iter().map(|item| item.role).collect();
    assert!(roles.contains(&SpatialItemRole::Route));
    assert!(roles.contains(&SpatialItemRole::Container));
    assert!(roles.contains(&SpatialItemRole::Label));
    assert!(roles.contains(&SpatialItemRole::Anchor));
    assert!(projection.items().iter().any(|item| {
        item.primitives
            .iter()
            .any(|primitive| matches!(primitive, SpatialPrimitive::TextBounds(_)))
    }));
}

#[test]
fn plot_spatial_adapter_preserves_non_hit_overlay_geometry() {
    let mut snapshot = empty_plot_snapshot();
    snapshot.regions.push(PlotRegion {
        id: 20,
        x_min: Some(10.0),
        x_max: Some(40.0),
        y_min: Some(20.0),
        y_max: Some(60.0),
        fill_color: Color::from_srgb(0.2, 0.4, 0.8, 0.2),
        label: Some("range".to_string()),
        ..PlotRegion::default()
    });
    snapshot.tags.push(PlotTag {
        id: 21,
        x: 50.0,
        y: 50.0,
        tag: vec![TagLine::new("note"), TagLine::new("second")],
        color: Color::WHITE,
        anchor: TagAnchor::Above,
    });

    let projection = PlotSpatialAdapter
        .build_projection(
            &snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let context = PlotProjectionContext::from_snapshot(&snapshot);
    let region = projection
        .items()
        .iter()
        .find(|item| context.key_for(item.id) == Some(PlotProjectionKey::Region(20)))
        .unwrap();
    let tag = projection
        .items()
        .iter()
        .find(|item| context.key_for(item.id) == Some(PlotProjectionKey::Tag(21)))
        .unwrap();

    assert_eq!(projection.hit_regions().len(), 0);
    assert_eq!(region.role, SpatialItemRole::Container);
    assert_eq!(region.bounds, WorldRect::new(20.0, 40.0, 60.0, 40.0));
    assert!(matches!(
        region.primitives.as_slice(),
        [SpatialPrimitive::Rect(rect), SpatialPrimitive::TextBounds(label)]
            if *rect == region.bounds && *label == region.bounds
    ));
    assert_eq!(tag.role, SpatialItemRole::Label);
    assert_eq!(tag.bounds, WorldRect::new(60.0, 16.0, 80.0, 30.0));
    assert!(matches!(
        tag.primitives.as_slice(),
        [SpatialPrimitive::TextBounds(bounds)] if *bounds == tag.bounds
    ));
    assert_eq!(projection.label_bounds(), vec![region.bounds, tag.bounds]);
    assert!(projection.hit_test(region.bounds.center()).is_none());
    assert!(projection.hit_test(tag.bounds.center()).is_none());
}

#[test]
fn plot_spatial_adapter_preserves_marker_hit_tolerances() {
    let mut circle_snapshot = empty_plot_snapshot();
    circle_snapshot.circles.push(PlotCircle {
        id: 10,
        x: 50.0,
        y: 50.0,
        radius: 4.0,
        ..PlotCircle::default()
    });
    let circle_projection = PlotSpatialAdapter
        .build_projection(
            &circle_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert_eq!(
        circle_projection
            .hit_test(WorldPoint::new(106.9, 50.0))
            .unwrap()
            .priority,
        SpatialHitPriority::Anchor
    );
    assert!(
        circle_projection
            .hit_test(WorldPoint::new(107.2, 50.0))
            .is_none()
    );

    let mut line_snapshot = empty_plot_snapshot();
    line_snapshot.lines.push(PlotLine {
        id: 11,
        x1: 0.0,
        y1: 50.0,
        x2: 100.0,
        y2: 50.0,
        width: 2.0,
        ..PlotLine::default()
    });
    let line_projection = PlotSpatialAdapter
        .build_projection(
            &line_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert_eq!(
        line_projection
            .hit_test(WorldPoint::new(100.0, 55.9))
            .unwrap()
            .priority,
        SpatialHitPriority::Route
    );
    assert!(
        line_projection
            .hit_test(WorldPoint::new(100.0, 56.2))
            .is_none()
    );

    let mut level_snapshot = empty_plot_snapshot();
    level_snapshot.price_levels.push(PriceLevel {
        id: 12,
        y: 50.0,
        display_y: 50.0,
        color: Color::WHITE,
        ..PriceLevel::default()
    });
    let level_projection = PlotSpatialAdapter
        .build_projection(
            &level_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert!(
        level_projection
            .hit_test(WorldPoint::new(100.0, 54.9))
            .is_some()
    );
    assert!(
        level_projection
            .hit_test(WorldPoint::new(100.0, 55.2))
            .is_none()
    );
}

#[test]
fn plot_spatial_adapter_preserves_path_marker_hit_tolerances() {
    let mut bezier_snapshot = empty_plot_snapshot();
    bezier_snapshot.beziers.push(PlotBezier {
        id: 13,
        x1: 0.0,
        y1: 50.0,
        cx1: 33.0,
        cy1: 50.0,
        cx2: 66.0,
        cy2: 50.0,
        x2: 100.0,
        y2: 50.0,
        width: 4.0,
        ..PlotBezier::default()
    });
    let bezier_projection = PlotSpatialAdapter
        .build_projection(
            &bezier_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert_eq!(
        bezier_projection
            .hit_test(WorldPoint::new(100.0, 56.9))
            .unwrap()
            .priority,
        SpatialHitPriority::Route
    );
    assert!(
        bezier_projection
            .hit_test(WorldPoint::new(100.0, 57.2))
            .is_none()
    );

    let mut styled_path_snapshot = empty_plot_snapshot();
    styled_path_snapshot.styled_paths.push(PlotStyledPath {
        id: 14,
        segments: vec![
            StyledSegment {
                x: 0.0,
                y: 50.0,
                color: Color::WHITE,
                width: 4.0,
                line_style: Default::default(),
            },
            StyledSegment {
                x: 100.0,
                y: 50.0,
                color: Color::WHITE,
                width: 4.0,
                line_style: Default::default(),
            },
        ],
        group: None,
    });
    let styled_path_projection = PlotSpatialAdapter
        .build_projection(
            &styled_path_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert_eq!(
        styled_path_projection
            .hit_test(WorldPoint::new(100.0, 56.9))
            .unwrap()
            .priority,
        SpatialHitPriority::Route
    );
    assert!(
        styled_path_projection
            .hit_test(WorldPoint::new(100.0, 57.2))
            .is_none()
    );

    let mut path_snapshot = empty_plot_snapshot();
    path_snapshot.paths.push(PlotPath {
        id: 15,
        points: vec![(0.0, 50.0), (100.0, 50.0)],
        width: 4.0,
        ..PlotPath::default()
    });
    let path_projection = PlotSpatialAdapter
        .build_projection(
            &path_snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    assert_eq!(
        path_projection
            .hit_test(WorldPoint::new(100.0, 56.9))
            .unwrap()
            .priority,
        SpatialHitPriority::Route
    );
    assert!(
        path_projection
            .hit_test(WorldPoint::new(100.0, 57.2))
            .is_none()
    );
}

#[test]
fn plot_spatial_adapter_rejects_partial_invalid_path_geometry() {
    let mut snapshot = empty_plot_snapshot();
    snapshot.transform = Transform::from_view_state_with_axes(
        &ViewState::new(1.0, 100.0, 1.0, 100.0),
        200.0,
        100.0,
        AxisKind::Log,
        AxisKind::Log,
    );
    snapshot.paths.push(PlotPath {
        id: 21,
        points: vec![(10.0, 10.0), (0.0, 20.0), (30.0, 30.0)],
        closed: true,
        width: 2.0,
        ..PlotPath::default()
    });
    snapshot.styled_paths.push(PlotStyledPath {
        id: 22,
        segments: vec![
            StyledSegment {
                x: 10.0,
                y: 10.0,
                color: Color::WHITE,
                width: 2.0,
                line_style: Default::default(),
            },
            StyledSegment {
                x: 20.0,
                y: -1.0,
                color: Color::WHITE,
                width: 2.0,
                line_style: Default::default(),
            },
        ],
        group: None,
    });

    let projection = PlotSpatialAdapter
        .build_projection(
            &snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();

    assert!(projection.items().is_empty());
    assert!(projection.hit_regions().is_empty());
}

#[test]
fn plot_spatial_adapter_translates_intents_without_mutation_authority() {
    let mut snapshot = empty_plot_snapshot();
    snapshot.circles.push(PlotCircle {
        id: 77,
        x: 50.0,
        y: 50.0,
        radius: 4.0,
        ..PlotCircle::default()
    });
    let projection = PlotSpatialAdapter
        .build_projection(
            &snapshot,
            &PlotProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let hit = projection.hit_test(WorldPoint::new(100.0, 50.0)).unwrap();
    let context = PlotProjectionContext::from_snapshot(&snapshot);

    let translated = PlotSpatialAdapter.translate_intent(
        &SpatialIntent::Press(SpatialPointerFacts {
            modifiers: Modifiers::default(),
            button: PointerButton::Primary,
            item: Some(hit.id),
            surface_pos: WorldPoint::new(100.0, 50.0),
            screen_pos: Point::new(100.0, 50.0),
        }),
        &context,
    );

    assert_eq!(
        translated,
        Some(PlotProjectionIntent::Press {
            key: Some(PlotProjectionKey::Circle(77)),
            surface_pos: WorldPoint::new(100.0, 50.0),
            screen_pos: Point::new(100.0, 50.0),
        })
    );
    assert_eq!(
        PlotSpatialAdapter.translate_intent(&SpatialIntent::KeyCommand(KeyCode::Delete), &context),
        None
    );
}

#[test]
fn plot_spatial_adapter_rejects_mutation_authority_intents() {
    let context = PlotProjectionContext::from_snapshot(&empty_plot_snapshot());
    let facts = SpatialPointerFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        item: None,
        surface_pos: WorldPoint::new(10.0, 20.0),
        screen_pos: Point::new(10.0, 20.0),
    };

    for intent in [
        SpatialIntent::DragStarted(facts),
        SpatialIntent::DragUpdated {
            facts,
            delta: WorldPoint::new(3.0, 4.0),
        },
        SpatialIntent::DragCommitted(facts),
        SpatialIntent::DragCanceled,
        SpatialIntent::RubberBandChanged(WorldRect::new(0.0, 0.0, 40.0, 30.0)),
        SpatialIntent::RubberBandCommitted(WorldRect::new(0.0, 0.0, 40.0, 30.0)),
        SpatialIntent::ViewportChanged(SpatialViewportState::default()),
        SpatialIntent::KeyCommand(KeyCode::Delete),
        SpatialIntent::KeyCommand(KeyCode::Backspace),
    ] {
        assert_eq!(PlotSpatialAdapter.translate_intent(&intent, &context), None);
    }
}

#[test]
fn timeline_spatial_adapter_projects_temporal_snapshot_without_taking_authority() {
    let snapshot = timeline_spatial_snapshot();
    let projection = TimelineSpatialAdapter
        .build_projection(
            &snapshot,
            &TimelineProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let context = snapshot.context();

    assert_eq!(
        projection.coordinate_mode(),
        SpatialCoordinateMode::DomainMappedSurface
    );
    assert_eq!(projection.viewport(), None);
    assert_eq!(projection.axis_orientation(), None);
    assert_eq!(projection.revision(), SpatialProjectionRevision(88));
    assert_eq!(projection.routes().len(), 1);
    assert_eq!(projection.guides().len(), 0);
    assert_eq!(projection.overlays().len(), 0);

    let roles: Vec<_> = projection.items().iter().map(|item| item.role).collect();
    assert!(roles.contains(&SpatialItemRole::Container));
    assert!(roles.contains(&SpatialItemRole::Background));
    assert!(roles.contains(&SpatialItemRole::Item));
    assert!(roles.contains(&SpatialItemRole::Route));
    assert!(roles.contains(&SpatialItemRole::Guide));
    assert!(projection.items().iter().any(|item| {
        context.key_for(item.id) == Some(TimelineProjectionKey::Lane { row: 10, lane: 1 })
    }));
    assert!(
        projection
            .routes()
            .values()
            .all(|route| matches!(route, SpatialRoutePlan::RightAngle(_)))
    );
}

#[test]
fn timeline_spatial_adapter_uses_time_geometry_for_x_mapping() {
    let snapshot = timeline_spatial_snapshot();
    let projection = TimelineSpatialAdapter
        .build_projection(
            &snapshot,
            &TimelineProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let context = snapshot.context();
    let interval = snapshot
        .frame
        .intervals
        .iter()
        .find(|interval| interval.id == IntervalId(1))
        .unwrap();
    let item = projection
        .items()
        .iter()
        .find(|item| {
            context.key_for(item.id)
                == Some(TimelineProjectionKey::Interval {
                    id: IntervalId(1),
                    layer: IntervalLayer::Primary,
                })
        })
        .unwrap();

    let expected_x0 =
        f64::from(snapshot.canvas_rect.x + snapshot.geometry.x_of(interval.range.start));
    let expected_x1 =
        f64::from(snapshot.canvas_rect.x + snapshot.geometry.x_of(interval.range.end));

    assert!((item.bounds.x - expected_x0).abs() < 0.001);
    assert!((item.bounds.width - (expected_x1 - expected_x0).max(1.0)).abs() < 0.001);

    let mut scaled = timeline_spatial_snapshot();
    scaled.geometry =
        TimeGeometryEngine::new(Timestamp::from_secs(0), TimeScale::Seconds, 24.0, 220.0);
    scaled.resolution = scaled.geometry.resolve();
    let scaled_projection = TimelineSpatialAdapter
        .build_projection(
            &scaled,
            &TimelineProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let scaled_context = scaled.context();
    let scaled_item = scaled_projection
        .items()
        .iter()
        .find(|item| {
            scaled_context.key_for(item.id)
                == Some(TimelineProjectionKey::Interval {
                    id: IntervalId(1),
                    layer: IntervalLayer::Primary,
                })
        })
        .unwrap();
    let scaled_interval = scaled
        .frame
        .intervals
        .iter()
        .find(|interval| interval.id == IntervalId(1))
        .unwrap();
    let scaled_x0 =
        f64::from(scaled.canvas_rect.x + scaled.geometry.x_of(scaled_interval.range.start));

    assert!((scaled_item.bounds.x - scaled_x0).abs() < 0.001);
    assert_ne!(scaled_item.bounds.x, item.bounds.x);
}

#[test]
fn timeline_spatial_adapter_preserves_primary_interval_hit_geometry() {
    let snapshot = timeline_spatial_snapshot();
    let projection = TimelineSpatialAdapter
        .build_projection(
            &snapshot,
            &TimelineProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let context = snapshot.context();

    let hit = projection.hit_test(WorldPoint::new(50.0, 15.0)).unwrap();
    assert_eq!(hit.priority, SpatialHitPriority::Item);
    assert_eq!(
        context.key_for(hit.id),
        Some(TimelineProjectionKey::Interval {
            id: IntervalId(1),
            layer: IntervalLayer::Primary
        })
    );
    assert!(projection.hit_test(WorldPoint::new(30.0, 80.0)).is_none());
}

#[test]
fn timeline_spatial_adapter_translates_press_without_mutation_authority() {
    let snapshot = timeline_spatial_snapshot();
    let projection = TimelineSpatialAdapter
        .build_projection(
            &snapshot,
            &TimelineProjectionSession,
            None,
            None,
            &SpatialVisualPolicy::default(),
        )
        .unwrap();
    let hit = projection.hit_test(WorldPoint::new(50.0, 15.0)).unwrap();
    let context = TimelineProjectionContext::from_snapshot(&snapshot);

    let translated = TimelineSpatialAdapter.translate_intent(
        &SpatialIntent::Press(SpatialPointerFacts {
            modifiers: Modifiers::default(),
            button: PointerButton::Primary,
            item: Some(hit.id),
            surface_pos: WorldPoint::new(50.0, 15.0),
            screen_pos: Point::new(50.0, 15.0),
        }),
        &context,
    );

    assert_eq!(
        translated,
        Some(TimelineProjectionIntent::Press {
            key: Some(TimelineProjectionKey::Interval {
                id: IntervalId(1),
                layer: IntervalLayer::Primary
            }),
            surface_pos: WorldPoint::new(50.0, 15.0),
            screen_pos: Point::new(50.0, 15.0),
        })
    );
    assert_eq!(
        TimelineSpatialAdapter
            .translate_intent(&SpatialIntent::KeyCommand(KeyCode::Delete), &context),
        None
    );
}

#[test]
fn timeline_spatial_adapter_rejects_mutation_authority_intents() {
    let snapshot = timeline_spatial_snapshot();
    let context = TimelineProjectionContext::from_snapshot(&snapshot);
    let facts = SpatialPointerFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        item: None,
        surface_pos: WorldPoint::new(10.0, 20.0),
        screen_pos: Point::new(10.0, 20.0),
    };

    for intent in [
        SpatialIntent::DragStarted(facts),
        SpatialIntent::DragUpdated {
            facts,
            delta: WorldPoint::new(3.0, 4.0),
        },
        SpatialIntent::DragCommitted(facts),
        SpatialIntent::DragCanceled,
        SpatialIntent::RubberBandChanged(WorldRect::new(0.0, 0.0, 40.0, 30.0)),
        SpatialIntent::RubberBandCommitted(WorldRect::new(0.0, 0.0, 40.0, 30.0)),
        SpatialIntent::ViewportChanged(SpatialViewportState::default()),
        SpatialIntent::KeyCommand(KeyCode::Delete),
        SpatialIntent::KeyCommand(KeyCode::Backspace),
    ] {
        assert_eq!(
            TimelineSpatialAdapter.translate_intent(&intent, &context),
            None
        );
    }
}

#[test]
fn spatial_grid_realization_matches_affine_viewport() {
    let rect = RectF::new(10.0, 20.0, 128.0, 96.0);
    let transform =
        CanvasSpatialBridge::spatial_transform(CanvasViewportState::default(), rect).unwrap();
    let lines = realize_spatial_grid(transform, SpatialGridPolicy::default()).unwrap();
    let vertical: Vec<_> = lines
        .iter()
        .filter(|line| line.axis == SpatialGuideAxis::Vertical)
        .collect();
    let horizontal: Vec<_> = lines
        .iter()
        .filter(|line| line.axis == SpatialGuideAxis::Horizontal)
        .collect();

    assert_eq!(vertical.len(), 5);
    assert_eq!(horizontal.len(), 4);
    assert_eq!(vertical[0].rect, RectF::new(10.0, 20.0, 1.0, 96.0));
    assert_eq!(vertical[0].role, SpatialGridLineRole::Major);
    assert_eq!(vertical[4].rect, RectF::new(138.0, 20.0, 1.0, 96.0));
    assert_eq!(vertical[4].role, SpatialGridLineRole::Major);
}

#[test]
fn spatial_grid_realization_respects_cartesian_y_orientation() {
    let transform = SpatialTransform::new(
        SpatialViewportState::default(),
        SpatialAxisOrientation::new(
            SpatialDirection::IncreasingPixels,
            SpatialDirection::DecreasingPixels,
        ),
        RectF::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap();
    let lines = realize_spatial_grid(transform, SpatialGridPolicy::default()).unwrap();
    let positive_y = lines
        .iter()
        .find(|line| line.axis == SpatialGuideAxis::Horizontal && line.world_position == 32.0)
        .unwrap();
    let negative_y = lines
        .iter()
        .find(|line| line.axis == SpatialGuideAxis::Horizontal && line.world_position == -32.0)
        .unwrap();

    assert_eq!(positive_y.rect.y, 18.0);
    assert_eq!(negative_y.rect.y, 82.0);
}

#[test]
fn spatial_overlay_rect_is_viewport_anchored() {
    let rect = resolve_spatial_overlay_rect(
        RectF::new(10.0, 20.0, 200.0, 100.0),
        SpatialOverlayPlan {
            anchor: SpatialOverlayAnchor::BottomLeft,
            offset: Point::new(3.0, -2.0),
            size: RectF::new(0.0, 0.0, 50.0, 20.0),
            edge_padding: 12.0,
        },
    );

    assert_eq!(rect, RectF::new(25.0, 86.0, 50.0, 20.0));
}

#[test]
fn spatial_overlay_rect_preserves_padding_for_all_anchors_and_resize() {
    let size = RectF::new(0.0, 0.0, 40.0, 16.0);
    let padding = 10.0;
    let viewport = RectF::new(0.0, 0.0, 100.0, 50.0);

    assert_eq!(
        resolve_spatial_overlay_rect(
            viewport,
            SpatialOverlayPlan::anchored(SpatialOverlayAnchor::TopLeft, size, padding)
        ),
        RectF::new(10.0, 10.0, 40.0, 16.0)
    );
    assert_eq!(
        resolve_spatial_overlay_rect(
            viewport,
            SpatialOverlayPlan::anchored(SpatialOverlayAnchor::TopRight, size, padding)
        ),
        RectF::new(50.0, 10.0, 40.0, 16.0)
    );
    assert_eq!(
        resolve_spatial_overlay_rect(
            viewport,
            SpatialOverlayPlan::anchored(SpatialOverlayAnchor::BottomLeft, size, padding)
        ),
        RectF::new(10.0, 24.0, 40.0, 16.0)
    );
    assert_eq!(
        resolve_spatial_overlay_rect(
            viewport,
            SpatialOverlayPlan::anchored(SpatialOverlayAnchor::BottomRight, size, padding)
        ),
        RectF::new(50.0, 24.0, 40.0, 16.0)
    );

    assert_eq!(
        resolve_spatial_overlay_rect(
            RectF::new(20.0, 30.0, 200.0, 120.0),
            SpatialOverlayPlan::anchored(SpatialOverlayAnchor::BottomRight, size, padding)
        ),
        RectF::new(170.0, 124.0, 40.0, 16.0)
    );
}

#[test]
fn spatial_guide_and_overlay_constructors_are_domain_neutral() {
    assert_eq!(
        SpatialGuidePlan::vertical(SpatialGuideRole::Active, 12.0, 3.0, 44.0),
        SpatialGuidePlan {
            axis: SpatialGuideAxis::Vertical,
            role: SpatialGuideRole::Active,
            position: 12.0,
            start: 3.0,
            end: 44.0,
        }
    );
    assert_eq!(
        SpatialGuidePlan::horizontal(SpatialGuideRole::Structural, 9.0, -10.0, 30.0),
        SpatialGuidePlan {
            axis: SpatialGuideAxis::Horizontal,
            role: SpatialGuideRole::Structural,
            position: 9.0,
            start: -10.0,
            end: 30.0,
        }
    );
    assert_eq!(
        SpatialOverlayPlan::anchored(
            SpatialOverlayAnchor::TopRight,
            RectF::new(0.0, 0.0, 80.0, 24.0),
            10.0
        )
        .with_offset(Point::new(-2.0, 6.0)),
        SpatialOverlayPlan {
            anchor: SpatialOverlayAnchor::TopRight,
            offset: Point::new(-2.0, 6.0),
            size: RectF::new(0.0, 0.0, 80.0, 24.0),
            edge_padding: 10.0,
        }
    );
}

#[test]
fn spatial_visual_policy_keeps_guides_distinct_from_grid_roles() {
    for policy in [
        SpatialVisualPolicy::for_theme(false),
        SpatialVisualPolicy::for_theme(true),
    ] {
        assert_ne!(policy.active_guide, policy.grid_minor);
        assert_ne!(policy.active_guide, policy.grid_major);
        assert_ne!(policy.structural_guide, policy.grid_minor);
        assert_ne!(policy.structural_guide, policy.grid_major);
        assert_ne!(policy.active_guide, policy.structural_guide);
    }
}

#[test]
fn spatial_visual_policy_resolves_light_dark_shared_roles() {
    let light = SpatialVisualPolicy::for_theme(false);
    let dark = SpatialVisualPolicy::for_theme(true);

    assert_ne!(light.canvas_background, dark.canvas_background);
    assert_ne!(light.grid_minor, dark.grid_minor);
    assert_ne!(light.grid_major, dark.grid_major);
    assert_ne!(light.active_guide, dark.active_guide);
    assert_ne!(light.structural_guide, dark.structural_guide);
    assert_ne!(light.route_default, dark.route_default);
    assert_ne!(light.hud_background, dark.hud_background);
    assert_ne!(light.hud_border, dark.hud_border);
    assert_ne!(light.hud_text, dark.hud_text);
    assert_ne!(light.hud_secondary_text, dark.hud_secondary_text);

    for policy in [light, dark] {
        assert!(policy.hud_edge_padding > 0.0);
        assert_ne!(policy.grid_minor, policy.grid_major);
        assert_ne!(policy.route_default, policy.route_selected);
        assert_ne!(policy.route_default, policy.route_hovered);
        assert_ne!(policy.route_default, policy.route_blocked);
        assert_ne!(policy.route_default, policy.route_preview);
        assert!(policy.route_selected.stroke_width > policy.route_default.stroke_width);
        assert!(policy.route_hovered.stroke_width > policy.route_default.stroke_width);
        assert!(policy.route_blocked.stroke_width > policy.route_default.stroke_width);
    }
}

#[test]
fn spatial_interaction_policy_resolves_shared_gestures() {
    let policy = SpatialInteractionPolicy::default();
    assert_eq!(policy.wheel_zoom, SpatialWheelZoomPolicy::ControlOrMeta);
    let primary = SpatialGestureFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        space_pressed: false,
    };
    let shift_primary = SpatialGestureFacts {
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        button: PointerButton::Primary,
        space_pressed: false,
    };
    let space_primary = SpatialGestureFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Primary,
        space_pressed: true,
    };
    let middle = SpatialGestureFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Auxiliary,
        space_pressed: false,
    };
    let secondary = SpatialGestureFacts {
        modifiers: Modifiers::default(),
        button: PointerButton::Secondary,
        space_pressed: false,
    };

    assert_eq!(
        policy.gesture_class(primary),
        SpatialGestureClass::PrimaryDrag
    );
    assert_eq!(
        policy.gesture_class(shift_primary),
        SpatialGestureClass::ShiftPrimaryDrag
    );
    assert_eq!(
        policy.gesture_class(space_primary),
        SpatialGestureClass::SpacePrimaryDrag
    );
    assert_eq!(
        policy.gesture_class(middle),
        SpatialGestureClass::MiddleDrag
    );
    assert_eq!(
        policy.gesture_class(secondary),
        SpatialGestureClass::Disabled
    );
    assert!(policy.is_pan_gesture(policy.gesture_class(space_primary)));
    assert!(policy.is_pan_gesture(policy.gesture_class(middle)));
    assert!(policy.is_rubber_band_gesture(policy.gesture_class(shift_primary)));
    assert!(policy.is_route_creation_gesture(policy.gesture_class(primary)));
}

#[test]
fn spatial_wheel_zoom_policy_resolves_every_modifier_combination() {
    for bits in 0_u8..16 {
        let modifiers = Modifiers {
            ctrl: bits & 0b0001 != 0,
            alt: bits & 0b0010 != 0,
            shift: bits & 0b0100 != 0,
            meta: bits & 0b1000 != 0,
        };

        for (wheel_zoom, expected) in [
            (SpatialWheelZoomPolicy::Disabled, false),
            (SpatialWheelZoomPolicy::Unmodified, true),
            (
                SpatialWheelZoomPolicy::ControlOrMeta,
                modifiers.ctrl || modifiers.meta,
            ),
        ] {
            let policy = SpatialInteractionPolicy {
                wheel_zoom,
                ..SpatialInteractionPolicy::default()
            };
            assert_eq!(policy.wheel_zoom_requested(modifiers), expected);
        }
    }
}

#[test]
fn timeline_canvas_consumes_shared_policy_for_pan_gesture() {
    let substrate = Rc::new(RefCell::new(TemporalSubstrate::new(
        TimeGeometryEngine::new(Timestamp(0), TimeScale::Days, 32.0, 200.0),
        SelectionMode::Multi,
        LanePolicy::Auto,
    )));
    let canvas = TimelineCanvas::new(substrate);
    let policy = canvas.spatial_interaction_policy();

    assert_eq!(policy.wheel_zoom, SpatialWheelZoomPolicy::Unmodified);

    assert!(
        policy.is_pan_gesture(policy.gesture_class(SpatialGestureFacts {
            modifiers: Modifiers::default(),
            button: PointerButton::Primary,
            space_pressed: true,
        }))
    );
    assert!(
        !policy.is_pan_gesture(policy.gesture_class(SpatialGestureFacts {
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            button: PointerButton::Primary,
            space_pressed: false,
        }))
    );
    assert!(!policy.is_pan_gesture(SpatialGestureClass::MiddleDrag));
    assert!(!policy.is_rubber_band_gesture(SpatialGestureClass::ShiftPrimaryDrag));
    assert!(!policy.is_route_creation_gesture(SpatialGestureClass::PrimaryDrag));
    assert_eq!(policy.key_intent(KeyCode::Delete), None);
    assert_eq!(policy.key_intent(KeyCode::Character('f')), None);
    assert_eq!(policy.key_intent(KeyCode::Home), None);
}

#[test]
fn spatial_interaction_policy_keeps_cursor_and_key_commands_domain_neutral() {
    let policy = SpatialInteractionPolicy::default();
    assert_eq!(
        policy.cursor_for_gesture(SpatialGestureClass::PrimaryDrag, false),
        SpatialCursorPolicy::Default
    );
    assert_eq!(
        policy.cursor_for_gesture(SpatialGestureClass::PrimaryDrag, true),
        SpatialCursorPolicy::DragActive
    );
    assert_eq!(
        policy.cursor_for_gesture(SpatialGestureClass::ShiftPrimaryDrag, true),
        SpatialCursorPolicy::CrosshairActive
    );
    assert_eq!(
        policy.cursor_for_route_creation(SpatialGestureClass::PrimaryDrag, true),
        SpatialCursorPolicy::CrosshairActive
    );
    assert!(policy.selection_toggle_requested(Modifiers {
        ctrl: true,
        ..Modifiers::default()
    }));
    assert!(policy.selection_toggle_requested(Modifiers {
        meta: true,
        ..Modifiers::default()
    }));
    assert!(!policy.selection_toggle_requested(Modifiers {
        shift: true,
        ..Modifiers::default()
    }));
    assert_eq!(
        policy.key_intent(KeyCode::Delete),
        Some(SpatialIntent::KeyCommand(KeyCode::Delete))
    );
    assert_eq!(
        policy.key_intent(KeyCode::Backspace),
        Some(SpatialIntent::KeyCommand(KeyCode::Backspace))
    );
    assert_eq!(
        policy.key_intent(KeyCode::Character('f')),
        Some(SpatialIntent::KeyCommand(KeyCode::Character('f')))
    );
    assert_eq!(
        policy.key_intent(KeyCode::Home),
        Some(SpatialIntent::KeyCommand(KeyCode::Home))
    );
    assert_eq!(policy.key_intent(KeyCode::Enter), None);

    let no_delete = SpatialInteractionPolicy {
        delete_keys_enabled: false,
        ..SpatialInteractionPolicy::default()
    };
    assert_eq!(no_delete.key_intent(KeyCode::Delete), None);
}
