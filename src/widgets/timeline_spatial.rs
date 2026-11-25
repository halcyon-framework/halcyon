use std::collections::HashMap;

use crate::core::math::{Point, RectF};
use crate::engines::route_geometry::{
    RouteDirection, RouteEndpoint, RouteParams, route_right_angle,
};
use crate::engines::temporal::{
    ArrowDescriptor, IntervalEdge, IntervalId, IntervalLayer, IntervalStyle, MarkerStyle,
    RowGeometry, TemporalFrameSnapshot, TimeGeometryEngine, TimeGeometryResolution,
};
use crate::model::canvas::{WorldPoint, WorldRect};
use crate::model::spatial::{
    SpatialAdapter, SpatialAxisOrientation, SpatialCoordinateMode, SpatialHitPriority,
    SpatialHitRegion, SpatialIntent, SpatialItem, SpatialItemId, SpatialItemRole,
    SpatialPointerFacts, SpatialPrimitive, SpatialProjection, SpatialProjectionError,
    SpatialProjectionRevision, SpatialRoutePlan, SpatialStrokeRole, SpatialViewportState,
    SpatialVisualPolicy,
};
use crate::model::time::{TimeRange, Timestamp};
use crate::renderer::path::Path;
use crate::widgets::timeline_canvas::{
    ARROW_HEAD, ARROW_STUB, BAR_MARGIN, LANE_GAP, MARKER_GLYPH, interval_paint_rect,
};

const FREE_ROW_KIND: u64 = 1;
const ROW_KIND: u64 = 2;
const LANE_KIND: u64 = 3;
const BAND_KIND: u64 = 4;
const GRID_KIND: u64 = 5;
const NON_WORKING_KIND: u64 = 6;
const INTERVAL_KIND: u64 = 7;
const MARKER_KIND: u64 = 8;
const ARROW_KIND: u64 = 9;
const NOW_KIND: u64 = 10;

const SURFACE_Z: i32 = 0;
const BACKGROUND_Z: i32 = 10;
const ROW_Z: i32 = 20;
const LANE_Z: i32 = 30;
const INTERVAL_Z: i32 = 50;
const ROUTE_Z: i32 = 70;
const GUIDE_Z: i32 = 90;
const NOW_Z: i32 = 100;
const MARKER_HIT_TOLERANCE: f64 = 3.0;

/// Timeline-owned projection identity for neutral spatial output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineProjectionKey {
    FreeRow,
    Row(u64),
    Lane {
        row: u64,
        lane: u8,
    },
    Band {
        index: usize,
        row: u64,
    },
    GridLine(usize),
    NonWorkingSpan(usize),
    Interval {
        id: IntervalId,
        layer: IntervalLayer,
    },
    Marker {
        index: usize,
        ts: Timestamp,
    },
    Arrow {
        index: usize,
        from: IntervalId,
        to: IntervalId,
    },
    Now,
}

/// Immutable Timeline snapshot consumed by `TimelineSpatialAdapter`.
#[derive(Debug, Clone)]
pub struct TimelineSpatialSnapshot {
    pub revision: SpatialProjectionRevision,
    pub canvas_rect: RectF,
    pub resolution: TimeGeometryResolution,
    pub frame: TemporalFrameSnapshot,
    pub geometry: TimeGeometryEngine,
    pub rows: Vec<RowGeometry>,
    pub lanes: Vec<u8>,
    pub row_lane_counts: Vec<(u64, u8)>,
}

/// Immutable Timeline session facts used by the spatial adapter.
#[derive(Debug, Clone, Default)]
pub struct TimelineProjectionSession;

/// Pure context for translating spatial item IDs back into Timeline projection keys.
#[derive(Debug, Clone, Default)]
pub struct TimelineProjectionContext {
    keys: HashMap<SpatialItemId, TimelineProjectionKey>,
}

/// Timeline-level intent translated from a neutral spatial intent.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineProjectionIntent {
    HoverChanged {
        previous: Option<TimelineProjectionKey>,
        current: Option<TimelineProjectionKey>,
    },
    Press {
        key: Option<TimelineProjectionKey>,
        surface_pos: WorldPoint,
        screen_pos: Point,
    },
}

/// Pure adapter from Timeline snapshots to domain-neutral spatial projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimelineSpatialAdapter;

impl TimelineProjectionContext {
    pub fn from_snapshot(snapshot: &TimelineSpatialSnapshot) -> Self {
        let mut keys = HashMap::new();
        for key in timeline_projection_keys(snapshot) {
            keys.insert(timeline_spatial_id(key), key);
        }
        Self { keys }
    }

    pub fn key_for(&self, id: SpatialItemId) -> Option<TimelineProjectionKey> {
        self.keys.get(&id).copied()
    }
}

impl TimelineSpatialSnapshot {
    pub fn new(
        revision: SpatialProjectionRevision,
        canvas_rect: RectF,
        resolution: TimeGeometryResolution,
        frame: TemporalFrameSnapshot,
        geometry: TimeGeometryEngine,
        rows: Vec<RowGeometry>,
        lanes: Vec<u8>,
    ) -> Self {
        let row_lane_counts = row_lane_counts(&frame, &lanes);
        Self {
            revision,
            canvas_rect,
            resolution,
            frame,
            geometry,
            rows,
            lanes,
            row_lane_counts,
        }
    }

    pub fn with_derived_revision(
        canvas_rect: RectF,
        resolution: TimeGeometryResolution,
        frame: TemporalFrameSnapshot,
        geometry: TimeGeometryEngine,
        rows: Vec<RowGeometry>,
        lanes: Vec<u8>,
    ) -> Self {
        let row_lane_counts = row_lane_counts(&frame, &lanes);
        let revision = derive_timeline_revision(
            canvas_rect,
            &resolution,
            &frame,
            &geometry,
            &rows,
            &lanes,
            &row_lane_counts,
        );
        Self {
            revision,
            canvas_rect,
            resolution,
            frame,
            geometry,
            rows,
            lanes,
            row_lane_counts,
        }
    }

    pub fn context(&self) -> TimelineProjectionContext {
        TimelineProjectionContext::from_snapshot(self)
    }
}

impl SpatialAdapter for TimelineSpatialAdapter {
    type DomainSnapshot = TimelineSpatialSnapshot;
    type SessionSnapshot = TimelineProjectionSession;
    type PolicyInput = SpatialVisualPolicy;
    type Context = TimelineProjectionContext;
    type DomainIntent = TimelineProjectionIntent;

    fn build_projection(
        &self,
        domain: &Self::DomainSnapshot,
        _session: &Self::SessionSnapshot,
        viewport: Option<&SpatialViewportState>,
        orientation: Option<&SpatialAxisOrientation>,
        policy: &Self::PolicyInput,
    ) -> Result<SpatialProjection, SpatialProjectionError> {
        let mut items = Vec::new();
        let mut hit_regions = Vec::new();
        let mut routes = HashMap::new();

        push_rows_and_lanes(domain, &mut items);
        push_non_working_spans(domain, &mut items);
        push_bands(domain, &mut items);
        push_gridlines(domain, &mut items);
        push_intervals(domain, &mut items, &mut hit_regions);
        push_arrows(domain, &mut items, &mut routes);
        push_markers(domain, &mut items, &mut hit_regions);
        push_now_marker(domain, &mut items);

        SpatialProjection::from_parts(
            domain.revision,
            viewport.copied(),
            orientation.copied(),
            SpatialCoordinateMode::DomainMappedSurface,
            items,
            hit_regions,
            routes,
            Vec::new(),
            Vec::new(),
            policy.clone(),
        )
    }

    fn translate_intent(
        &self,
        intent: &SpatialIntent,
        context: &Self::Context,
    ) -> Option<Self::DomainIntent> {
        match intent {
            SpatialIntent::HoverChanged { previous, current } => {
                Some(TimelineProjectionIntent::HoverChanged {
                    previous: previous.and_then(|id| context.key_for(id)),
                    current: current.and_then(|id| context.key_for(id)),
                })
            }
            SpatialIntent::Press(SpatialPointerFacts {
                item,
                surface_pos,
                screen_pos,
                ..
            }) => Some(TimelineProjectionIntent::Press {
                key: item.and_then(|id| context.key_for(id)),
                surface_pos: *surface_pos,
                screen_pos: *screen_pos,
            }),
            _ => None,
        }
    }
}

fn push_rows_and_lanes(snapshot: &TimelineSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    if snapshot.rows.is_empty() {
        items.push(SpatialItem {
            id: timeline_spatial_id(TimelineProjectionKey::FreeRow),
            role: SpatialItemRole::Surface,
            bounds: rect_to_world(snapshot.canvas_rect),
            z_order: SURFACE_Z,
            primitives: vec![SpatialPrimitive::Rect(rect_to_world(snapshot.canvas_rect))],
        });
        for &(row, lane_count) in &snapshot.row_lane_counts {
            for lane in 0..lane_count {
                push_lane_item(
                    items,
                    TimelineProjectionKey::Lane { row, lane },
                    snapshot.canvas_rect.y,
                    snapshot.canvas_rect.height,
                    snapshot.canvas_rect.x,
                    snapshot.canvas_rect.x + snapshot.canvas_rect.width,
                    lane,
                    lane_count,
                );
            }
        }
        return;
    }

    for row in &snapshot.rows {
        let row_rect = RectF::new(
            snapshot.canvas_rect.x,
            snapshot.canvas_rect.y + row.y,
            snapshot.canvas_rect.width,
            row.height,
        );
        items.push(SpatialItem {
            id: timeline_spatial_id(TimelineProjectionKey::Row(row.id)),
            role: SpatialItemRole::Container,
            bounds: rect_to_world(row_rect),
            z_order: ROW_Z,
            primitives: vec![SpatialPrimitive::Rect(rect_to_world(row_rect))],
        });
        let lane_count = snapshot.lane_count(row.id);
        for lane in 0..lane_count {
            push_lane_item(
                items,
                TimelineProjectionKey::Lane { row: row.id, lane },
                row_rect.y,
                row_rect.height,
                row_rect.x,
                row_rect.x + row_rect.width,
                lane,
                lane_count,
            );
        }
    }
}

fn push_lane_item(
    items: &mut Vec<SpatialItem>,
    key: TimelineProjectionKey,
    row_y: f32,
    row_h: f32,
    x0: f32,
    x1: f32,
    lane: u8,
    lane_count: u8,
) {
    let rect = interval_paint_rect(x0, x1, row_y, row_h, lane, lane_count, BAR_MARGIN, LANE_GAP);
    items.push(SpatialItem {
        id: timeline_spatial_id(key),
        role: SpatialItemRole::Container,
        bounds: rect_to_world(rect),
        z_order: LANE_Z,
        primitives: vec![SpatialPrimitive::Rect(rect_to_world(rect))],
    });
}

fn push_non_working_spans(snapshot: &TimelineSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    for (index, &(x0, x1)) in snapshot.resolution.non_working.iter().enumerate() {
        let rect = RectF::new(
            snapshot.canvas_rect.x + x0,
            snapshot.canvas_rect.y,
            (x1 - x0).max(1.0),
            snapshot.canvas_rect.height,
        );
        items.push(SpatialItem {
            id: timeline_spatial_id(TimelineProjectionKey::NonWorkingSpan(index)),
            role: SpatialItemRole::Background,
            bounds: rect_to_world(rect),
            z_order: BACKGROUND_Z,
            primitives: vec![SpatialPrimitive::Rect(rect_to_world(rect))],
        });
    }
}

fn push_bands(snapshot: &TimelineSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    for (index, band) in snapshot.frame.bands.iter().enumerate() {
        let Some((row_y, row_h)) = snapshot.row_rect(band.row) else {
            continue;
        };
        let x0 = snapshot.canvas_rect.x + snapshot.geometry.x_of(band.range.start);
        let x1 = snapshot.canvas_rect.x + snapshot.geometry.x_of(band.range.end);
        let rect = RectF::new(x0, row_y, (x1 - x0).max(1.0), row_h);
        items.push(SpatialItem {
            id: timeline_spatial_id(TimelineProjectionKey::Band {
                index,
                row: band.row,
            }),
            role: SpatialItemRole::Background,
            bounds: rect_to_world(rect),
            z_order: BACKGROUND_Z,
            primitives: vec![SpatialPrimitive::Rect(rect_to_world(rect))],
        });
    }
}

fn push_gridlines(snapshot: &TimelineSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    for (index, gx) in snapshot.resolution.gridlines.iter().copied().enumerate() {
        let rect = RectF::new(
            snapshot.canvas_rect.x + gx,
            snapshot.canvas_rect.y,
            1.0,
            snapshot.canvas_rect.height,
        );
        items.push(SpatialItem {
            id: timeline_spatial_id(TimelineProjectionKey::GridLine(index)),
            role: SpatialItemRole::Guide,
            bounds: rect_to_world(rect),
            z_order: GUIDE_Z,
            primitives: vec![SpatialPrimitive::Line {
                from: WorldPoint::new(rect.x as f64, rect.y as f64),
                to: WorldPoint::new(rect.x as f64, (rect.y + rect.height) as f64),
                stroke: SpatialStrokeRole::Default,
            }],
        });
    }
}

fn push_intervals(
    snapshot: &TimelineSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for (index, interval) in snapshot.frame.intervals.iter().enumerate() {
        let Some((row_y, row_h)) = snapshot.row_rect(interval.row) else {
            continue;
        };
        let x0 = snapshot.canvas_rect.x + snapshot.geometry.x_of(interval.range.start);
        let x1 = snapshot.canvas_rect.x + snapshot.geometry.x_of(interval.range.end);
        let lane = snapshot.lane_of(index);
        let lane_count = snapshot.lane_count(interval.row);
        let bar = interval_paint_rect(x0, x1, row_y, row_h, lane, lane_count, BAR_MARGIN, LANE_GAP);
        let id = timeline_spatial_id(TimelineProjectionKey::Interval {
            id: interval.id,
            layer: interval.layer,
        });

        let mut primitives = Vec::new();
        let bounds = if interval.style == IntervalStyle::Milestone {
            let half = bar.height / 2.0;
            let center = Point::new(x0, bar.y + half);
            let path = diamond_path(center.x, center.y, half);
            primitives.push(SpatialPrimitive::Path(path));
            RectF::new(center.x - half, center.y - half, half * 2.0, half * 2.0)
        } else {
            primitives.push(SpatialPrimitive::Rect(rect_to_world(bar)));
            if interval.label.is_some() {
                primitives.push(SpatialPrimitive::TextBounds(rect_to_world(bar)));
            }
            bar
        };

        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Item,
            bounds: rect_to_world(bounds),
            z_order: interval_z(interval.layer),
            primitives,
        });

        if interval.layer != IntervalLayer::Primary {
            continue;
        }
        let rect = if interval.style == IntervalStyle::Milestone {
            let half = bar.height / 2.0;
            RectF::new(x0 - half, bar.y, half * 2.0, bar.height)
        } else {
            bar
        };
        hit_regions.push(SpatialHitRegion::Rect {
            id,
            rect: rect_to_world(rect),
            priority: SpatialHitPriority::Item,
        });
    }
}

fn push_arrows(
    snapshot: &TimelineSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    routes: &mut HashMap<SpatialItemId, SpatialRoutePlan>,
) {
    for (index, arrow) in snapshot.frame.arrows.iter().enumerate() {
        let Some(plan) = timeline_arrow_route_plan(snapshot, arrow) else {
            continue;
        };
        let id = timeline_spatial_id(TimelineProjectionKey::Arrow {
            index,
            from: arrow.from,
            to: arrow.to,
        });
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds: route_bounds(plan),
            z_order: ROUTE_Z + i32::from(arrow.critical),
            primitives: vec![SpatialPrimitive::Route(plan)],
        });
        routes.insert(id, plan);
    }
}

pub(crate) fn timeline_arrow_route_plan(
    snapshot: &TimelineSpatialSnapshot,
    arrow: &ArrowDescriptor,
) -> Option<SpatialRoutePlan> {
    let (from, to) = arrow_endpoints(snapshot, arrow)?;
    Some(SpatialRoutePlan::RightAngle(route_right_angle(
        from,
        to,
        RouteParams {
            stub: ARROW_STUB,
            head_length: ARROW_HEAD,
        },
    )))
}

fn push_markers(
    snapshot: &TimelineSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for (index, marker) in snapshot.frame.markers.iter().enumerate() {
        let x = snapshot.canvas_rect.x + snapshot.geometry.x_of(marker.ts);
        let id = timeline_spatial_id(TimelineProjectionKey::Marker {
            index,
            ts: marker.ts,
        });
        let line = SpatialPrimitive::Line {
            from: WorldPoint::new(x as f64, snapshot.canvas_rect.y as f64),
            to: WorldPoint::new(
                x as f64,
                (snapshot.canvas_rect.y + snapshot.canvas_rect.height) as f64,
            ),
            stroke: SpatialStrokeRole::Default,
        };
        let mut primitives = vec![line];
        let bounds = match marker.style {
            MarkerStyle::VerticalLine => {
                RectF::new(x, snapshot.canvas_rect.y, 1.0, snapshot.canvas_rect.height)
            }
            MarkerStyle::Diamond => {
                let half = MARKER_GLYPH / 2.0;
                let center = Point::new(x + 0.5, snapshot.canvas_rect.y + 1.0 + half);
                primitives.push(SpatialPrimitive::Path(diamond_path(
                    center.x, center.y, half,
                )));
                RectF::new(
                    x - half,
                    snapshot.canvas_rect.y,
                    half * 2.0 + 1.0,
                    MARKER_GLYPH + 2.0,
                )
            }
            MarkerStyle::Flag => {
                let mut path = Path::new();
                path.move_to(x + 1.0, snapshot.canvas_rect.y + 1.0);
                path.line_to(
                    x + 1.0 + MARKER_GLYPH,
                    snapshot.canvas_rect.y + 1.0 + MARKER_GLYPH * 0.4,
                );
                path.line_to(x + 1.0, snapshot.canvas_rect.y + 1.0 + MARKER_GLYPH * 0.8);
                path.close();
                primitives.push(SpatialPrimitive::Path(path));
                RectF::new(
                    x,
                    snapshot.canvas_rect.y,
                    MARKER_GLYPH + 2.0,
                    MARKER_GLYPH + 2.0,
                )
            }
        };
        if marker.label.is_some() {
            primitives.push(SpatialPrimitive::TextBounds(rect_to_world(RectF::new(
                x + 3.0,
                snapshot.canvas_rect.y + 2.0,
                MARKER_GLYPH * 2.0,
                MARKER_GLYPH,
            ))));
        }
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Guide,
            bounds: rect_to_world(bounds),
            z_order: GUIDE_Z,
            primitives,
        });
        hit_regions.push(SpatialHitRegion::Rect {
            id,
            rect: WorldRect::new(
                x as f64 - MARKER_HIT_TOLERANCE,
                snapshot.canvas_rect.y as f64,
                MARKER_HIT_TOLERANCE * 2.0,
                snapshot.canvas_rect.height as f64,
            ),
            priority: SpatialHitPriority::Anchor,
        });
    }
}

fn push_now_marker(snapshot: &TimelineSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    let Some(now_x) = snapshot.resolution.now_x else {
        return;
    };
    let rect = RectF::new(
        snapshot.canvas_rect.x + now_x,
        snapshot.canvas_rect.y,
        2.0,
        snapshot.canvas_rect.height,
    );
    items.push(SpatialItem {
        id: timeline_spatial_id(TimelineProjectionKey::Now),
        role: SpatialItemRole::Guide,
        bounds: rect_to_world(rect),
        z_order: NOW_Z,
        primitives: vec![SpatialPrimitive::Line {
            from: WorldPoint::new(rect.x as f64, rect.y as f64),
            to: WorldPoint::new(rect.x as f64, (rect.y + rect.height) as f64),
            stroke: SpatialStrokeRole::Default,
        }],
    });
}

impl TimelineSpatialSnapshot {
    fn row_rect(&self, row: u64) -> Option<(f32, f32)> {
        if self.rows.is_empty() {
            Some((self.canvas_rect.y, self.canvas_rect.height))
        } else {
            self.rows
                .iter()
                .find(|candidate| candidate.id == row)
                .map(|candidate| (self.canvas_rect.y + candidate.y, candidate.height))
        }
    }

    fn lane_of(&self, index: usize) -> u8 {
        self.lanes.get(index).copied().unwrap_or(0)
    }

    fn lane_count(&self, row: u64) -> u8 {
        self.row_lane_counts
            .iter()
            .find_map(|(candidate, count)| (*candidate == row).then_some(*count))
            .unwrap_or(1)
    }
}

fn arrow_endpoints(
    snapshot: &TimelineSpatialSnapshot,
    arrow: &ArrowDescriptor,
) -> Option<(RouteEndpoint, RouteEndpoint)> {
    let from_descriptor = snapshot.frame.interval(arrow.from)?;
    let to_descriptor = snapshot.frame.interval(arrow.to)?;
    let from_index = snapshot.frame.index_of(arrow.from)?;
    let to_index = snapshot.frame.index_of(arrow.to)?;
    let (from_y, from_h) = snapshot.row_rect(from_descriptor.row)?;
    let (to_y, to_h) = snapshot.row_rect(to_descriptor.row)?;
    let from_bar = interval_paint_rect(
        snapshot.canvas_rect.x + snapshot.geometry.x_of(from_descriptor.range.start),
        snapshot.canvas_rect.x + snapshot.geometry.x_of(from_descriptor.range.end),
        from_y,
        from_h,
        snapshot.lane_of(from_index),
        snapshot.lane_count(from_descriptor.row),
        BAR_MARGIN,
        LANE_GAP,
    );
    let to_bar = interval_paint_rect(
        snapshot.canvas_rect.x + snapshot.geometry.x_of(to_descriptor.range.start),
        snapshot.canvas_rect.x + snapshot.geometry.x_of(to_descriptor.range.end),
        to_y,
        to_h,
        snapshot.lane_of(to_index),
        snapshot.lane_count(to_descriptor.row),
        BAR_MARGIN,
        LANE_GAP,
    );
    Some((
        RouteEndpoint {
            point: Point::new(
                match arrow.from_edge {
                    IntervalEdge::Start => from_bar.x,
                    IntervalEdge::End => from_bar.x + from_bar.width,
                },
                from_bar.y + from_bar.height / 2.0,
            ),
            dir: match arrow.from_edge {
                IntervalEdge::Start => RouteDirection::Left,
                IntervalEdge::End => RouteDirection::Right,
            },
        },
        RouteEndpoint {
            point: Point::new(
                match arrow.to_edge {
                    IntervalEdge::Start => to_bar.x,
                    IntervalEdge::End => to_bar.x + to_bar.width,
                },
                to_bar.y + to_bar.height / 2.0,
            ),
            dir: match arrow.to_edge {
                IntervalEdge::Start => RouteDirection::Left,
                IntervalEdge::End => RouteDirection::Right,
            },
        },
    ))
}

fn timeline_projection_keys(snapshot: &TimelineSpatialSnapshot) -> Vec<TimelineProjectionKey> {
    let mut keys = Vec::new();
    if snapshot.rows.is_empty() {
        keys.push(TimelineProjectionKey::FreeRow);
        for &(row, lane_count) in &snapshot.row_lane_counts {
            keys.extend((0..lane_count).map(|lane| TimelineProjectionKey::Lane { row, lane }));
        }
    } else {
        keys.extend(
            snapshot
                .rows
                .iter()
                .map(|row| TimelineProjectionKey::Row(row.id)),
        );
        for row in &snapshot.rows {
            let lane_count = snapshot.lane_count(row.id);
            keys.extend(
                (0..lane_count).map(|lane| TimelineProjectionKey::Lane { row: row.id, lane }),
            );
        }
    }
    keys.extend(
        snapshot
            .frame
            .bands
            .iter()
            .enumerate()
            .map(|(index, band)| TimelineProjectionKey::Band {
                index,
                row: band.row,
            }),
    );
    keys.extend(
        snapshot
            .resolution
            .gridlines
            .iter()
            .enumerate()
            .map(|(index, _)| TimelineProjectionKey::GridLine(index)),
    );
    keys.extend(
        snapshot
            .resolution
            .non_working
            .iter()
            .enumerate()
            .map(|(index, _)| TimelineProjectionKey::NonWorkingSpan(index)),
    );
    keys.extend(
        snapshot
            .frame
            .intervals
            .iter()
            .map(|interval| TimelineProjectionKey::Interval {
                id: interval.id,
                layer: interval.layer,
            }),
    );
    keys.extend(
        snapshot
            .frame
            .markers
            .iter()
            .enumerate()
            .map(|(index, marker)| TimelineProjectionKey::Marker {
                index,
                ts: marker.ts,
            }),
    );
    keys.extend(
        snapshot
            .frame
            .arrows
            .iter()
            .enumerate()
            .map(|(index, arrow)| TimelineProjectionKey::Arrow {
                index,
                from: arrow.from,
                to: arrow.to,
            }),
    );
    if snapshot.resolution.now_x.is_some() {
        keys.push(TimelineProjectionKey::Now);
    }
    keys
}

fn row_lane_counts(frame: &TemporalFrameSnapshot, lanes: &[u8]) -> Vec<(u64, u8)> {
    let mut counts = Vec::<(u64, u8)>::new();
    for (index, interval) in frame.intervals.iter().enumerate() {
        let needed = lanes.get(index).copied().unwrap_or(0).saturating_add(1);
        if let Some((_, count)) = counts.iter_mut().find(|(row, _)| *row == interval.row) {
            *count = (*count).max(needed);
        } else {
            counts.push((interval.row, needed.max(1)));
        }
    }
    counts
}

fn timeline_spatial_id(key: TimelineProjectionKey) -> SpatialItemId {
    let mut value = match key {
        TimelineProjectionKey::FreeRow => mix_timeline_projection_id(FREE_ROW_KIND, 0),
        TimelineProjectionKey::Row(row) => mix_timeline_projection_id(ROW_KIND, row),
        TimelineProjectionKey::Lane { row, lane } => mix_many(LANE_KIND, &[row, u64::from(lane)]),
        TimelineProjectionKey::Band { index, row } => mix_many(BAND_KIND, &[index as u64, row]),
        TimelineProjectionKey::GridLine(index) => {
            mix_timeline_projection_id(GRID_KIND, index as u64)
        }
        TimelineProjectionKey::NonWorkingSpan(index) => {
            mix_timeline_projection_id(NON_WORKING_KIND, index as u64)
        }
        TimelineProjectionKey::Interval { id, layer } => {
            mix_many(INTERVAL_KIND, &[id.0, interval_layer_discriminator(layer)])
        }
        TimelineProjectionKey::Marker { index, ts } => {
            mix_many(MARKER_KIND, &[index as u64, ts.0 as u64])
        }
        TimelineProjectionKey::Arrow { index, from, to } => {
            mix_many(ARROW_KIND, &[index as u64, from.0, to.0])
        }
        TimelineProjectionKey::Now => mix_timeline_projection_id(NOW_KIND, 0),
    };
    if value == 0 {
        value = u64::MAX;
    }
    SpatialItemId::from_opaque(value)
}

fn interval_z(layer: IntervalLayer) -> i32 {
    match layer {
        IntervalLayer::Baseline => INTERVAL_Z,
        IntervalLayer::Primary => INTERVAL_Z + 10,
        IntervalLayer::Preview => INTERVAL_Z + 20,
    }
}

fn interval_layer_discriminator(layer: IntervalLayer) -> u64 {
    match layer {
        IntervalLayer::Baseline => 1,
        IntervalLayer::Primary => 2,
        IntervalLayer::Preview => 3,
    }
}

fn rect_to_world(rect: RectF) -> WorldRect {
    WorldRect::new(
        rect.x as f64,
        rect.y as f64,
        rect.width as f64,
        rect.height as f64,
    )
}

fn diamond_path(center_x: f32, center_y: f32, half: f32) -> Path {
    let mut path = Path::new();
    path.move_to(center_x, center_y - half);
    path.line_to(center_x + half, center_y);
    path.line_to(center_x, center_y + half);
    path.line_to(center_x - half, center_y);
    path.close();
    path
}

fn route_bounds(plan: SpatialRoutePlan) -> WorldRect {
    let geometry = crate::model::spatial::realize_spatial_route_plan(plan);
    let mut min_x = geometry.head.anchor.x;
    let mut max_x = geometry.head.anchor.x;
    let mut min_y = geometry.head.anchor.y;
    let mut max_y = geometry.head.anchor.y;
    for segment in geometry.segments() {
        for point in [segment.from, segment.to] {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }
    }
    WorldRect::new(
        (min_x - ARROW_HEAD) as f64,
        (min_y - ARROW_HEAD) as f64,
        (max_x - min_x + ARROW_HEAD * 2.0).max(1.0) as f64,
        (max_y - min_y + ARROW_HEAD * 2.0).max(1.0) as f64,
    )
}

fn derive_timeline_revision(
    canvas_rect: RectF,
    resolution: &TimeGeometryResolution,
    frame: &TemporalFrameSnapshot,
    geometry: &TimeGeometryEngine,
    rows: &[RowGeometry],
    lanes: &[u8],
    row_lane_counts: &[(u64, u8)],
) -> SpatialProjectionRevision {
    let mut value = 0xcbf2_9ce4_8422_2325;
    value = mix_rect_revision(value, canvas_rect);
    value = mix_range_revision(value, resolution.visible_range);
    for gridline in &resolution.gridlines {
        value = mix_revision(value, gridline.to_bits() as u64);
    }
    for (x0, x1) in &resolution.non_working {
        value = mix_revision(value, x0.to_bits() as u64);
        value = mix_revision(value, x1.to_bits() as u64);
    }
    value = mix_revision(
        value,
        resolution
            .now_x
            .map(|x| x.to_bits() as u64)
            .unwrap_or(u64::MAX),
    );
    value = mix_revision(value, geometry.anchor().0 as u64);
    value = mix_revision(value, time_scale_discriminator(geometry.scale()));
    value = mix_revision(value, geometry.px_per_unit().to_bits() as u64);
    value = mix_revision(value, geometry.viewport_width().to_bits() as u64);
    value = mix_revision(value, geometry.frame().utc_offset_minutes as u64);
    for row in rows {
        value = mix_revision(value, row.id);
        value = mix_revision(value, row.y.to_bits() as u64);
        value = mix_revision(value, row.height.to_bits() as u64);
    }
    for lane in lanes {
        value = mix_revision(value, u64::from(*lane));
    }
    for (row, count) in row_lane_counts {
        value = mix_revision(value, *row);
        value = mix_revision(value, u64::from(*count));
    }
    for interval in &frame.intervals {
        value = mix_revision(value, interval.id.0);
        value = mix_revision(value, interval.row);
        value = mix_range_revision(value, interval.range);
        value = mix_revision(value, u64::from(interval.label.is_some()));
        value = mix_revision(value, interval.progress.to_bits() as u64);
        value = mix_revision(value, interval_style_discriminator(interval.style));
        value = mix_revision(value, interval_layer_discriminator(interval.layer));
        value = mix_revision(value, interval.lane.map(u64::from).unwrap_or(u64::MAX));
    }
    for band in &frame.bands {
        value = mix_revision(value, band.row);
        value = mix_range_revision(value, band.range);
    }
    for marker in &frame.markers {
        value = mix_revision(value, marker.ts.0 as u64);
        value = mix_revision(value, u64::from(marker.label.is_some()));
        value = mix_revision(value, marker_style_discriminator(marker.style));
    }
    for arrow in &frame.arrows {
        value = mix_revision(value, arrow.from.0);
        value = mix_revision(value, interval_edge_discriminator(arrow.from_edge));
        value = mix_revision(value, arrow.to.0);
        value = mix_revision(value, interval_edge_discriminator(arrow.to_edge));
        value = mix_revision(value, u64::from(arrow.critical));
        value = mix_revision(value, u64::from(arrow.color.is_some()));
    }
    if value == 0 {
        value = 1;
    }
    SpatialProjectionRevision(value)
}

fn mix_rect_revision(value: u64, rect: RectF) -> u64 {
    let value = mix_revision(value, rect.x.to_bits() as u64);
    let value = mix_revision(value, rect.y.to_bits() as u64);
    let value = mix_revision(value, rect.width.to_bits() as u64);
    mix_revision(value, rect.height.to_bits() as u64)
}

fn mix_range_revision(value: u64, range: TimeRange) -> u64 {
    let value = mix_revision(value, range.start.0 as u64);
    mix_revision(value, range.end.0 as u64)
}

fn mix_revision(value: u64, part: u64) -> u64 {
    mix_timeline_projection_id(value, part)
}

fn mix_many(kind: u64, parts: &[u64]) -> u64 {
    let mut value = kind.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for part in parts {
        value = mix_timeline_projection_id(value, *part);
    }
    value
}

fn mix_timeline_projection_id(kind: u64, id: u64) -> u64 {
    let mut value = id ^ kind.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

fn time_scale_discriminator(scale: crate::engines::temporal::TimeScale) -> u64 {
    match scale {
        crate::engines::temporal::TimeScale::Seconds => 1,
        crate::engines::temporal::TimeScale::Minutes => 2,
        crate::engines::temporal::TimeScale::Hours => 3,
        crate::engines::temporal::TimeScale::Days => 4,
        crate::engines::temporal::TimeScale::Weeks => 5,
        crate::engines::temporal::TimeScale::Months => 6,
    }
}

fn interval_style_discriminator(style: IntervalStyle) -> u64 {
    match style {
        IntervalStyle::Solid => 1,
        IntervalStyle::Hatched => 2,
        IntervalStyle::Hollow => 3,
        IntervalStyle::Milestone => 4,
    }
}

fn interval_edge_discriminator(edge: IntervalEdge) -> u64 {
    match edge {
        IntervalEdge::Start => 1,
        IntervalEdge::End => 2,
    }
}

fn marker_style_discriminator(style: MarkerStyle) -> u64 {
    match style {
        MarkerStyle::VerticalLine => 1,
        MarkerStyle::Diamond => 2,
        MarkerStyle::Flag => 3,
    }
}
