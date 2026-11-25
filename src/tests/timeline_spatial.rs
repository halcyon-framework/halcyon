use crate::core::math::{Point, RectF};
use crate::core::paint::Color;
use crate::engines::route_geometry::{
    RouteDirection, RouteEndpoint, RouteParams, route_right_angle,
};
use crate::engines::temporal::{
    ArrowDescriptor, IntervalDescriptor, IntervalEdge, IntervalId, IntervalLayer,
    IntervalProjection, IntervalSource, IntervalStyle, LaneLayoutEngine, LanePolicy, RowGeometry,
    TemporalDescriptors, TemporalInvalidationEvent, TimeGeometryEngine, TimeScale,
};
use crate::model::spatial::{
    SpatialAdapter, SpatialPrimitive, SpatialProjectionRevision, SpatialRoutePlan,
    SpatialVisualPolicy,
};
use crate::model::time::{TimeRange, Timestamp};
use crate::widgets::timeline_canvas::{
    ARROW_HEAD, ARROW_STUB, BAR_MARGIN, LANE_GAP, interval_paint_rect,
};
use crate::widgets::timeline_spatial::{
    TimelineProjectionKey, TimelineProjectionSession, TimelineSpatialAdapter,
    TimelineSpatialSnapshot, timeline_arrow_route_plan,
};

#[derive(Debug, Default)]
struct TimelineRouteSource {
    descriptors: TemporalDescriptors,
}

impl IntervalSource for TimelineRouteSource {
    fn collect(&self, _rows: Option<&[u64]>, _range: TimeRange, out: &mut TemporalDescriptors) {
        out.intervals.extend(self.descriptors.intervals.clone());
        out.arrows.extend(self.descriptors.arrows.iter().copied());
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

fn timeline_route_snapshot() -> TimelineSpatialSnapshot {
    let mut source = TimelineRouteSource::default();
    source.descriptors.intervals.extend([
        IntervalDescriptor {
            id: IntervalId(1),
            row: 10,
            range: TimeRange::new(Timestamp::from_secs(2), Timestamp::from_secs(8)),
            color: Color::WHITE,
            label: None,
            progress: 0.0,
            style: IntervalStyle::Solid,
            layer: IntervalLayer::Primary,
            lane: None,
        },
        IntervalDescriptor {
            id: IntervalId(3),
            row: 20,
            range: TimeRange::new(Timestamp::from_secs(12), Timestamp::from_secs(18)),
            color: Color::WHITE,
            label: None,
            progress: 0.0,
            style: IntervalStyle::Milestone,
            layer: IntervalLayer::Primary,
            lane: None,
        },
    ]);
    source.descriptors.arrows.push(ArrowDescriptor {
        from: IntervalId(1),
        from_edge: IntervalEdge::End,
        to: IntervalId(3),
        to_edge: IntervalEdge::Start,
        critical: true,
        color: None,
    });

    let geometry =
        TimeGeometryEngine::new(Timestamp::from_secs(0), TimeScale::Seconds, 10.0, 220.0);
    let resolution = geometry.resolve();
    let mut projection = IntervalProjection::new();
    let frame = projection
        .collect_frame(&source, Some(&[10, 20]), geometry.visible_range())
        .clone();
    let mut lanes = LaneLayoutEngine::new();
    lanes.resolve(&frame.intervals, LanePolicy::Auto);

    TimelineSpatialSnapshot::new(
        SpatialProjectionRevision(338),
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

#[test]
fn timeline_spatial_adapter_preserves_spec338_arrow_route_geometry() {
    let snapshot = timeline_route_snapshot();
    let arrow = snapshot.frame.arrows[0];
    let from = snapshot.frame.interval(arrow.from).unwrap();
    let to = snapshot.frame.interval(arrow.to).unwrap();
    let from_index = snapshot.frame.index_of(arrow.from).unwrap();
    let to_index = snapshot.frame.index_of(arrow.to).unwrap();
    let from_bar = interval_paint_rect(
        snapshot.canvas_rect.x + snapshot.geometry.x_of(from.range.start),
        snapshot.canvas_rect.x + snapshot.geometry.x_of(from.range.end),
        snapshot.canvas_rect.y + snapshot.rows[0].y,
        snapshot.rows[0].height,
        snapshot.lanes[from_index],
        snapshot.row_lane_counts[0].1,
        BAR_MARGIN,
        LANE_GAP,
    );
    let to_bar = interval_paint_rect(
        snapshot.canvas_rect.x + snapshot.geometry.x_of(to.range.start),
        snapshot.canvas_rect.x + snapshot.geometry.x_of(to.range.end),
        snapshot.canvas_rect.y + snapshot.rows[1].y,
        snapshot.rows[1].height,
        snapshot.lanes[to_index],
        snapshot.row_lane_counts[1].1,
        BAR_MARGIN,
        LANE_GAP,
    );
    let expected = SpatialRoutePlan::RightAngle(route_right_angle(
        RouteEndpoint {
            point: Point::new(
                from_bar.x + from_bar.width,
                from_bar.y + from_bar.height / 2.0,
            ),
            dir: RouteDirection::Right,
        },
        RouteEndpoint {
            point: Point::new(to_bar.x, to_bar.y + to_bar.height / 2.0),
            dir: RouteDirection::Left,
        },
        RouteParams {
            stub: ARROW_STUB,
            head_length: ARROW_HEAD,
        },
    ));

    assert_eq!(timeline_arrow_route_plan(&snapshot, &arrow), Some(expected));

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
    let (route_id, route) = projection.routes().iter().next().unwrap();
    assert_eq!(projection.routes().len(), 1);
    assert_eq!(
        context.key_for(*route_id),
        Some(TimelineProjectionKey::Arrow {
            index: 0,
            from: IntervalId(1),
            to: IntervalId(3),
        })
    );
    assert_eq!(*route, expected);

    let route_item = projection
        .items()
        .iter()
        .find(|item| item.id == *route_id)
        .unwrap();
    assert!(matches!(
        route_item.primitives.as_slice(),
        [SpatialPrimitive::Route(item_route)] if *item_route == expected
    ));
}
