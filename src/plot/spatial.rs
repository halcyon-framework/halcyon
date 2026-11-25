use std::collections::HashMap;

use crate::core::math::Point;
use crate::model::canvas::{WorldPoint, WorldRect};
use crate::model::spatial::{
    SpatialAdapter, SpatialAxisOrientation, SpatialCoordinateMode, SpatialHitPriority,
    SpatialHitRegion, SpatialIntent, SpatialItem, SpatialItemId, SpatialItemRole,
    SpatialPointerFacts, SpatialPrimitive, SpatialProjection, SpatialProjectionError,
    SpatialProjectionRevision, SpatialStrokeRole, SpatialViewportState, SpatialVisualPolicy,
};
use crate::plot::{
    Plot, PlotBezier, PlotCircle, PlotLine, PlotPath, PlotRegion, PlotStyledPath, PlotTag,
    PriceLevel, TagAnchor, Transform,
};
use crate::renderer::path::Path;

const PRICE_LEVEL_KIND: u64 = 1;
const REGION_KIND: u64 = 2;
const TAG_KIND: u64 = 3;
const CIRCLE_KIND: u64 = 4;
const LINE_KIND: u64 = 5;
const BEZIER_KIND: u64 = 6;
const STYLED_PATH_KIND: u64 = 7;
const PATH_KIND: u64 = 8;

const PRICE_LEVEL_Z: i32 = 20;
const REGION_Z: i32 = 10;
const TAG_Z: i32 = 60;
const CIRCLE_Z: i32 = 50;
const ROUTE_Z: i32 = 40;

const PRICE_LEVEL_HIT_TOLERANCE: f64 = 5.0;
const CONNECTOR_HIT_TOLERANCE: f64 = 5.0;
const CIRCLE_HIT_EXTRA: f64 = 3.0;
const TAG_WIDTH: f32 = 80.0;
const TAG_LINE_HEIGHT: f32 = 12.0;
const TAG_PADDING_Y: f32 = 3.0;
const TAG_OFFSET: f32 = 4.0;

/// Plot-owned projection identity for domain-neutral spatial marker output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlotProjectionKey {
    PriceLevel(u64),
    Region(u64),
    Tag(u64),
    Circle(u64),
    Line(u64),
    Bezier(u64),
    StyledPath(u64),
    Path(u64),
}

/// Immutable Plot marker snapshot consumed by `PlotSpatialAdapter`.
#[derive(Debug, Clone)]
pub struct PlotSpatialSnapshot {
    pub revision: SpatialProjectionRevision,
    pub transform: Transform,
    pub price_levels: Vec<PriceLevel>,
    pub regions: Vec<PlotRegion>,
    pub tags: Vec<PlotTag>,
    pub circles: Vec<PlotCircle>,
    pub lines: Vec<PlotLine>,
    pub beziers: Vec<PlotBezier>,
    pub styled_paths: Vec<PlotStyledPath>,
    pub paths: Vec<PlotPath>,
}

/// Immutable Plot session facts used by the spatial adapter.
#[derive(Debug, Clone, Default)]
pub struct PlotProjectionSession;

/// Pure context for translating spatial item IDs back into Plot projection keys.
#[derive(Debug, Clone, Default)]
pub struct PlotProjectionContext {
    keys: HashMap<SpatialItemId, PlotProjectionKey>,
}

/// Plot-level domain intent translated from a neutral spatial intent.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotProjectionIntent {
    HoverChanged {
        previous: Option<PlotProjectionKey>,
        current: Option<PlotProjectionKey>,
    },
    Press {
        key: Option<PlotProjectionKey>,
        surface_pos: WorldPoint,
        screen_pos: Point,
    },
}

/// Pure adapter from Plot marker snapshots to domain-neutral spatial projection.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlotSpatialAdapter;

impl PlotProjectionContext {
    pub fn from_snapshot(snapshot: &PlotSpatialSnapshot) -> Self {
        let mut keys = HashMap::new();
        for key in plot_projection_keys(snapshot) {
            keys.insert(plot_spatial_id(key), key);
        }
        Self { keys }
    }

    pub fn key_for(&self, id: SpatialItemId) -> Option<PlotProjectionKey> {
        self.keys.get(&id).copied()
    }
}

impl PlotSpatialSnapshot {
    pub fn context(&self) -> PlotProjectionContext {
        PlotProjectionContext::from_snapshot(self)
    }
}

impl Plot {
    /// Returns the current Plot marker snapshot for spatial projection.
    pub fn spatial_snapshot(&self) -> Option<PlotSpatialSnapshot> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }

        Some(PlotSpatialSnapshot {
            revision: SpatialProjectionRevision(self.render_generation),
            transform: self.transform(w, h),
            price_levels: self.price_levels.clone(),
            regions: self.regions.clone(),
            tags: self.tags.clone(),
            circles: self.circles.clone(),
            lines: self.lines.clone(),
            beziers: self.beziers.clone(),
            styled_paths: self.styled_paths.clone(),
            paths: self.paths.clone(),
        })
    }

    /// Builds a read-only spatial projection over current Plot marker geometry.
    pub fn spatial_projection(&self) -> Result<Option<SpatialProjection>, SpatialProjectionError> {
        self.spatial_projection_with_policy(SpatialVisualPolicy::default())
    }

    /// Builds a read-only spatial projection over current Plot marker geometry.
    pub fn spatial_projection_with_policy(
        &self,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<Option<SpatialProjection>, SpatialProjectionError> {
        let Some(snapshot) = self.spatial_snapshot() else {
            return Ok(None);
        };
        PlotSpatialAdapter
            .build_projection(
                &snapshot,
                &PlotProjectionSession,
                None,
                None,
                &visual_policy,
            )
            .map(Some)
    }
}

impl SpatialAdapter for PlotSpatialAdapter {
    type DomainSnapshot = PlotSpatialSnapshot;
    type SessionSnapshot = PlotProjectionSession;
    type PolicyInput = SpatialVisualPolicy;
    type Context = PlotProjectionContext;
    type DomainIntent = PlotProjectionIntent;

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

        push_price_levels(domain, &mut items, &mut hit_regions);
        push_regions(domain, &mut items);
        push_tags(domain, &mut items);
        push_circles(domain, &mut items, &mut hit_regions);
        push_lines(domain, &mut items, &mut hit_regions);
        push_beziers(domain, &mut items, &mut hit_regions);
        push_styled_paths(domain, &mut items, &mut hit_regions);
        push_paths(domain, &mut items, &mut hit_regions);

        SpatialProjection::from_parts(
            domain.revision,
            viewport.copied(),
            orientation.copied(),
            SpatialCoordinateMode::DomainMappedSurface,
            items,
            hit_regions,
            HashMap::new(),
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
                Some(PlotProjectionIntent::HoverChanged {
                    previous: previous.and_then(|id| context.key_for(id)),
                    current: current.and_then(|id| context.key_for(id)),
                })
            }
            SpatialIntent::Press(SpatialPointerFacts {
                item,
                surface_pos,
                screen_pos,
                ..
            }) => Some(PlotProjectionIntent::Press {
                key: item.and_then(|id| context.key_for(id)),
                surface_pos: *surface_pos,
                screen_pos: *screen_pos,
            }),
            _ => None,
        }
    }
}

fn push_price_levels(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    let w = snapshot.transform.width_px;
    if w <= 0.0 {
        return;
    }

    for level in &snapshot.price_levels {
        let Some(y) = snapshot.transform.y_to_px_checked(level.display_y) else {
            continue;
        };
        let id = plot_spatial_id(PlotProjectionKey::PriceLevel(level.id));
        let from = WorldPoint::new(0.0, y as f64);
        let to = WorldPoint::new(w as f64, y as f64);
        let path = line_path(from, to);
        let bounds = path_bounds(&path, PRICE_LEVEL_HIT_TOLERANCE);

        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds,
            z_order: PRICE_LEVEL_Z + i32::from(level.draggable),
            primitives: vec![SpatialPrimitive::Line {
                from,
                to,
                stroke: SpatialStrokeRole::Default,
            }],
        });
        hit_regions.push(SpatialHitRegion::Path {
            id,
            path,
            tolerance: PRICE_LEVEL_HIT_TOLERANCE,
            priority: SpatialHitPriority::Route,
        });
    }
}

fn push_regions(snapshot: &PlotSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    let w = snapshot.transform.width_px;
    let h = snapshot.transform.height_px;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    for region in &snapshot.regions {
        let Some(left) = optional_x(&snapshot.transform, region.x_min, 0.0) else {
            continue;
        };
        let Some(right) = optional_x(&snapshot.transform, region.x_max, w) else {
            continue;
        };
        let Some(top) = optional_y(&snapshot.transform, region.y_min, 0.0) else {
            continue;
        };
        let Some(bottom) = optional_y(&snapshot.transform, region.y_max, h) else {
            continue;
        };
        let rect = screen_rect(left, top, right, bottom);
        let id = plot_spatial_id(PlotProjectionKey::Region(region.id));
        let mut primitives = vec![SpatialPrimitive::Rect(rect)];
        if region.label.is_some() {
            primitives.push(SpatialPrimitive::TextBounds(rect));
        }
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Container,
            bounds: rect,
            z_order: REGION_Z,
            primitives,
        });
    }
}

fn push_tags(snapshot: &PlotSpatialSnapshot, items: &mut Vec<SpatialItem>) {
    for tag in &snapshot.tags {
        if tag.tag.is_empty() {
            continue;
        }
        let Some((x, y)) = snapshot.transform.point_to_px_checked(tag.x, tag.y) else {
            continue;
        };
        let rect = tag_rect(x, y, tag.anchor, tag.tag.len());
        let id = plot_spatial_id(PlotProjectionKey::Tag(tag.id));
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Label,
            bounds: rect,
            z_order: TAG_Z,
            primitives: vec![SpatialPrimitive::TextBounds(rect)],
        });
    }
}

fn push_circles(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for circle in &snapshot.circles {
        let Some((x, y)) = snapshot.transform.point_to_px_checked(circle.x, circle.y) else {
            continue;
        };
        let center = WorldPoint::new(x as f64, y as f64);
        let id = plot_spatial_id(PlotProjectionKey::Circle(circle.id));
        let hit_radius = circle.radius as f64 + CIRCLE_HIT_EXTRA;
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Anchor,
            bounds: rect_around(center, circle.radius as f64 + 1.0),
            z_order: CIRCLE_Z,
            primitives: vec![SpatialPrimitive::Circle {
                center,
                radius: circle.radius as f64,
            }],
        });
        hit_regions.push(SpatialHitRegion::Circle {
            id,
            center,
            radius: hit_radius,
            priority: SpatialHitPriority::Anchor,
        });
    }
}

fn push_lines(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for line in &snapshot.lines {
        let Some((x1, y1)) = snapshot.transform.point_to_px_checked(line.x1, line.y1) else {
            continue;
        };
        let Some((x2, y2)) = snapshot.transform.point_to_px_checked(line.x2, line.y2) else {
            continue;
        };
        let from = WorldPoint::new(x1 as f64, y1 as f64);
        let to = WorldPoint::new(x2 as f64, y2 as f64);
        let id = plot_spatial_id(PlotProjectionKey::Line(line.id));
        let path = line_path(from, to);
        let tolerance = line.width as f64 / 2.0 + CONNECTOR_HIT_TOLERANCE;
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds: path_bounds(&path, tolerance),
            z_order: ROUTE_Z,
            primitives: vec![SpatialPrimitive::Line {
                from,
                to,
                stroke: SpatialStrokeRole::Default,
            }],
        });
        hit_regions.push(SpatialHitRegion::Path {
            id,
            path,
            tolerance,
            priority: SpatialHitPriority::Route,
        });
    }
}

fn push_beziers(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for bezier in &snapshot.beziers {
        let Some((x1, y1)) = snapshot.transform.point_to_px_checked(bezier.x1, bezier.y1) else {
            continue;
        };
        let Some((cx1, cy1)) = snapshot
            .transform
            .point_to_px_checked(bezier.cx1, bezier.cy1)
        else {
            continue;
        };
        let Some((cx2, cy2)) = snapshot
            .transform
            .point_to_px_checked(bezier.cx2, bezier.cy2)
        else {
            continue;
        };
        let Some((x2, y2)) = snapshot.transform.point_to_px_checked(bezier.x2, bezier.y2) else {
            continue;
        };

        let mut path = Path::new();
        path.move_to(x1, y1);
        path.cubic_to(cx1, cy1, cx2, cy2, x2, y2);
        let id = plot_spatial_id(PlotProjectionKey::Bezier(bezier.id));
        let tolerance = bezier.width as f64 / 2.0 + CONNECTOR_HIT_TOLERANCE;
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds: path_bounds(&path, tolerance),
            z_order: ROUTE_Z,
            primitives: vec![SpatialPrimitive::Path(path.clone())],
        });
        hit_regions.push(SpatialHitRegion::Path {
            id,
            path,
            tolerance,
            priority: SpatialHitPriority::Route,
        });
    }
}

fn push_styled_paths(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for styled_path in &snapshot.styled_paths {
        if styled_path.segments.len() < 2 {
            continue;
        }
        let mut points = Vec::with_capacity(styled_path.segments.len());
        let mut max_width = 0.0f32;
        for segment in &styled_path.segments {
            let Some((x, y)) = snapshot.transform.point_to_px_checked(segment.x, segment.y) else {
                points.clear();
                break;
            };
            max_width = max_width.max(segment.width);
            points.push((x, y));
        }
        if points.len() < 2 {
            continue;
        }
        let mut path = Path::new();
        path.move_to(points[0].0, points[0].1);
        for (x, y) in points.iter().skip(1) {
            path.line_to(*x, *y);
        }
        let id = plot_spatial_id(PlotProjectionKey::StyledPath(styled_path.id));
        let tolerance = max_width as f64 / 2.0 + CONNECTOR_HIT_TOLERANCE;
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds: path_bounds(&path, tolerance),
            z_order: ROUTE_Z,
            primitives: vec![SpatialPrimitive::Path(path.clone())],
        });
        hit_regions.push(SpatialHitRegion::Path {
            id,
            path,
            tolerance,
            priority: SpatialHitPriority::Route,
        });
    }
}

fn push_paths(
    snapshot: &PlotSpatialSnapshot,
    items: &mut Vec<SpatialItem>,
    hit_regions: &mut Vec<SpatialHitRegion>,
) {
    for plot_path in &snapshot.paths {
        if plot_path.points.len() < 2 {
            continue;
        }
        let mut points = Vec::with_capacity(plot_path.points.len());
        for (world_x, world_y) in &plot_path.points {
            let Some((x, y)) = snapshot.transform.point_to_px_checked(*world_x, *world_y) else {
                points.clear();
                break;
            };
            points.push((x, y));
        }
        if points.len() < 2 {
            continue;
        }
        let mut path = Path::new();
        path.move_to(points[0].0, points[0].1);
        for (x, y) in points.iter().skip(1) {
            path.line_to(*x, *y);
        }
        if plot_path.closed {
            path.close();
        }
        if path.commands.len() < 2 {
            continue;
        }
        let id = plot_spatial_id(PlotProjectionKey::Path(plot_path.id));
        let tolerance = plot_path.width as f64 / 2.0 + CONNECTOR_HIT_TOLERANCE;
        items.push(SpatialItem {
            id,
            role: SpatialItemRole::Route,
            bounds: path_bounds(&path, tolerance),
            z_order: ROUTE_Z,
            primitives: vec![SpatialPrimitive::Path(path.clone())],
        });
        hit_regions.push(SpatialHitRegion::Path {
            id,
            path,
            tolerance,
            priority: SpatialHitPriority::Route,
        });
    }
}

fn plot_projection_keys(snapshot: &PlotSpatialSnapshot) -> Vec<PlotProjectionKey> {
    let mut keys = Vec::new();
    keys.extend(
        snapshot
            .price_levels
            .iter()
            .map(|level| PlotProjectionKey::PriceLevel(level.id)),
    );
    keys.extend(
        snapshot
            .regions
            .iter()
            .map(|region| PlotProjectionKey::Region(region.id)),
    );
    keys.extend(
        snapshot
            .tags
            .iter()
            .map(|tag| PlotProjectionKey::Tag(tag.id)),
    );
    keys.extend(
        snapshot
            .circles
            .iter()
            .map(|circle| PlotProjectionKey::Circle(circle.id)),
    );
    keys.extend(
        snapshot
            .lines
            .iter()
            .map(|line| PlotProjectionKey::Line(line.id)),
    );
    keys.extend(
        snapshot
            .beziers
            .iter()
            .map(|bezier| PlotProjectionKey::Bezier(bezier.id)),
    );
    keys.extend(
        snapshot
            .styled_paths
            .iter()
            .map(|path| PlotProjectionKey::StyledPath(path.id)),
    );
    keys.extend(
        snapshot
            .paths
            .iter()
            .map(|path| PlotProjectionKey::Path(path.id)),
    );
    keys
}

fn plot_spatial_id(key: PlotProjectionKey) -> SpatialItemId {
    let (kind, id) = match key {
        PlotProjectionKey::PriceLevel(id) => (PRICE_LEVEL_KIND, id),
        PlotProjectionKey::Region(id) => (REGION_KIND, id),
        PlotProjectionKey::Tag(id) => (TAG_KIND, id),
        PlotProjectionKey::Circle(id) => (CIRCLE_KIND, id),
        PlotProjectionKey::Line(id) => (LINE_KIND, id),
        PlotProjectionKey::Bezier(id) => (BEZIER_KIND, id),
        PlotProjectionKey::StyledPath(id) => (STYLED_PATH_KIND, id),
        PlotProjectionKey::Path(id) => (PATH_KIND, id),
    };
    SpatialItemId::from_opaque(mix_plot_projection_id(kind, id))
}

fn mix_plot_projection_id(kind: u64, id: u64) -> u64 {
    let mut value = id ^ kind.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

fn optional_x(transform: &Transform, value: Option<f64>, fallback: f32) -> Option<f32> {
    value
        .map(|x| transform.x_to_px_checked(x))
        .unwrap_or(Some(fallback))
}

fn optional_y(transform: &Transform, value: Option<f64>, fallback: f32) -> Option<f32> {
    value
        .map(|y| transform.y_to_px_checked(y))
        .unwrap_or(Some(fallback))
}

fn tag_rect(anchor_x: f32, anchor_y: f32, anchor: TagAnchor, line_count: usize) -> WorldRect {
    let h = line_count as f32 * TAG_LINE_HEIGHT + TAG_PADDING_Y * 2.0;
    let (x, y) = match anchor {
        TagAnchor::Above => (anchor_x - TAG_WIDTH / 2.0, anchor_y - h - TAG_OFFSET),
        TagAnchor::Below => (anchor_x - TAG_WIDTH / 2.0, anchor_y + TAG_OFFSET),
        TagAnchor::Right => (anchor_x + TAG_OFFSET, anchor_y - h / 2.0),
        TagAnchor::Left => (anchor_x - TAG_WIDTH - TAG_OFFSET, anchor_y - h / 2.0),
    };
    WorldRect::new(x as f64, y as f64, TAG_WIDTH as f64, h as f64)
}

fn line_path(from: WorldPoint, to: WorldPoint) -> Path {
    let mut path = Path::new();
    path.move_to(from.x as f32, from.y as f32);
    path.line_to(to.x as f32, to.y as f32);
    path
}

fn screen_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> WorldRect {
    let left = x1.min(x2);
    let right = x1.max(x2);
    let top = y1.min(y2);
    let bottom = y1.max(y2);
    WorldRect::new(
        left as f64,
        top as f64,
        (right - left) as f64,
        (bottom - top) as f64,
    )
}

fn rect_around(center: WorldPoint, radius: f64) -> WorldRect {
    WorldRect::new(
        center.x - radius,
        center.y - radius,
        radius * 2.0,
        radius * 2.0,
    )
}

fn path_bounds(path: &Path, margin: f64) -> WorldRect {
    let bounds = path.bounds();
    WorldRect::new(
        bounds.x as f64 - margin,
        bounds.y as f64 - margin,
        bounds.width as f64 + margin * 2.0,
        bounds.height as f64 + margin * 2.0,
    )
}
