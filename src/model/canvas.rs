use crate::core::paint::Color;
use crate::model::node_graph::PortDirection;
use crate::renderer::path::Path;

pub const DEFAULT_MIN_ZOOM: f64 = 0.1;
pub const DEFAULT_MAX_ZOOM: f64 = 10.0;
pub const DEFAULT_ZOOM_TO_FIT_PADDING: f64 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasTransform {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasViewportState {
    pub transform: CanvasTransform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasGrid {
    pub enabled: bool,
    pub base_spacing: f64,
    pub subdivisions: u32,
    pub snap_enabled: bool,
    pub color_major: Color,
    pub color_minor: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasBackground {
    Solid,
    Dots,
    Lines,
    CrossDots,
}

#[derive(Debug, Clone)]
pub struct InfiniteCanvas {
    min_zoom: f64,
    max_zoom: f64,
    grid: CanvasGrid,
    background: CanvasBackground,
    viewport_size: (f64, f64),
    intents: Vec<CanvasIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanvasEntityId {
    Node(u64),
    Port {
        node_id: u64,
        port_id: u32,
        direction: PortDirection,
    },
    Wire(u64),
    Frame(u64),
    Annotation(u64),
    Custom(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanvasEntityIdentityParts {
    pub(crate) kind: u8,
    pub(crate) words: [u64; 3],
    pub(crate) word_count: usize,
}

impl CanvasEntityId {
    pub(crate) fn stable_identity_parts(self) -> CanvasEntityIdentityParts {
        match self {
            CanvasEntityId::Node(id) => CanvasEntityIdentityParts {
                kind: 1,
                words: [id, 0, 0],
                word_count: 1,
            },
            CanvasEntityId::Port {
                node_id,
                port_id,
                direction,
            } => CanvasEntityIdentityParts {
                kind: 2,
                words: [
                    node_id,
                    port_id as u64,
                    match direction {
                        PortDirection::Input => 1,
                        PortDirection::Output => 2,
                    },
                ],
                word_count: 3,
            },
            CanvasEntityId::Wire(id) => CanvasEntityIdentityParts {
                kind: 3,
                words: [id, 0, 0],
                word_count: 1,
            },
            CanvasEntityId::Frame(id) => CanvasEntityIdentityParts {
                kind: 4,
                words: [id, 0, 0],
                word_count: 1,
            },
            CanvasEntityId::Annotation(id) => CanvasEntityIdentityParts {
                kind: 5,
                words: [id, 0, 0],
                word_count: 1,
            },
            CanvasEntityId::Custom(id) => CanvasEntityIdentityParts {
                kind: 6,
                words: [id, 0, 0],
                word_count: 1,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectedCanvasItem {
    pub id: CanvasEntityId,
    pub bounds: WorldRect,
    pub z_order: i32,
    pub hit_regions: Vec<CanvasHitRegion>,
    pub paint: CanvasPaintDescriptor,
}

#[derive(Debug, Clone)]
pub enum CanvasHitRegion {
    Rect {
        id: CanvasEntityId,
        rect: WorldRect,
        priority: HitPriority,
    },
    Circle {
        id: CanvasEntityId,
        center: WorldPoint,
        radius: f64,
        priority: HitPriority,
    },
    Path {
        id: CanvasEntityId,
        path: Path,
        tolerance: f64,
        priority: HitPriority,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitPriority {
    Background,
    Frame,
    Wire,
    Node,
    Port,
    Overlay,
}

#[derive(Debug, Clone)]
pub enum CanvasPaintDescriptor {
    None,
    Node(u64),
    Wire(u64),
    Frame(u64),
    Annotation(u64),
    Custom(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ProjectionRevision(pub u64);

#[derive(Debug, Clone)]
pub struct CanvasProjection {
    revision: ProjectionRevision,
    items: Vec<ProjectedCanvasItem>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasHit {
    pub id: CanvasEntityId,
    pub priority: HitPriority,
    pub z_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasIntent {
    Hit(CanvasHit),
    EmptyClick(WorldPoint),
    EmptyContextMenu(WorldPoint),
    RubberBandChanged(WorldRect),
    RubberBandCommitted(WorldRect),
    ViewportChanged(CanvasTransform),
}

impl WorldPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl WorldRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, p: WorldPoint) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    pub fn intersects(&self, other: &WorldRect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn union(&self, other: &WorldRect) -> WorldRect {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.width).max(other.x + other.width);
        let max_y = (self.y + self.height).max(other.y + other.height);
        WorldRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn center(&self) -> WorldPoint {
        WorldPoint::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }
}

impl CanvasTransform {
    pub const fn identity() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
        }
    }

    pub fn world_to_screen(&self, point: WorldPoint) -> (f32, f32) {
        (
            ((point.x - self.offset_x) * self.zoom) as f32,
            ((point.y - self.offset_y) * self.zoom) as f32,
        )
    }

    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> WorldPoint {
        WorldPoint::new(
            screen_x as f64 / self.zoom + self.offset_x,
            screen_y as f64 / self.zoom + self.offset_y,
        )
    }
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Default for CanvasViewportState {
    fn default() -> Self {
        Self {
            transform: CanvasTransform::identity(),
        }
    }
}

impl Default for CanvasGrid {
    fn default() -> Self {
        Self {
            enabled: true,
            base_spacing: 20.0,
            subdivisions: 5,
            snap_enabled: false,
            color_major: Color::from_linear(1.0, 1.0, 1.0, 0.16),
            color_minor: Color::from_linear(1.0, 1.0, 1.0, 0.06),
        }
    }
}

impl InfiniteCanvas {
    pub fn new() -> Self {
        Self {
            min_zoom: DEFAULT_MIN_ZOOM,
            max_zoom: DEFAULT_MAX_ZOOM,
            grid: CanvasGrid::default(),
            background: CanvasBackground::Dots,
            viewport_size: (1.0, 1.0),
            intents: Vec::new(),
        }
    }

    pub fn with_grid(mut self, grid: CanvasGrid) -> Self {
        self.grid = grid;
        self
    }

    pub fn with_background(mut self, background: CanvasBackground) -> Self {
        self.background = background;
        self
    }

    pub fn with_zoom_range(mut self, min_zoom: f64, max_zoom: f64) -> Self {
        assert!(min_zoom > 0.0, "min_zoom must be positive");
        assert!(max_zoom >= min_zoom, "max_zoom must be >= min_zoom");
        self.min_zoom = min_zoom;
        self.max_zoom = max_zoom;
        self
    }

    pub fn with_viewport_size(mut self, width: f64, height: f64) -> Self {
        self.viewport_size = (width.max(1.0), height.max(1.0));
        self
    }

    pub fn grid(&self) -> &CanvasGrid {
        &self.grid
    }

    pub fn background(&self) -> CanvasBackground {
        self.background
    }

    pub fn pan_to(&mut self, viewport: &mut CanvasViewportState, point: WorldPoint) {
        viewport.transform.offset_x = point.x;
        viewport.transform.offset_y = point.y;
        self.emit_viewport_changed(viewport.transform);
    }

    pub fn pan_by_screen_delta(
        &mut self,
        viewport: &mut CanvasViewportState,
        delta_x: f32,
        delta_y: f32,
    ) {
        let zoom = viewport.transform.zoom.max(f64::EPSILON);
        viewport.transform.offset_x -= delta_x as f64 / zoom;
        viewport.transform.offset_y -= delta_y as f64 / zoom;
        self.emit_viewport_changed(viewport.transform);
    }

    pub fn zoom_to(&mut self, viewport: &mut CanvasViewportState, zoom: f64) {
        viewport.transform.zoom = self.clamp_zoom(zoom);
        self.emit_viewport_changed(viewport.transform);
    }

    pub fn zoom_around_screen_point(
        &mut self,
        viewport: &mut CanvasViewportState,
        zoom: f64,
        screen_x: f32,
        screen_y: f32,
    ) {
        let fixed_world = viewport.transform.screen_to_world(screen_x, screen_y);
        let zoom = self.clamp_zoom(zoom);
        viewport.transform.zoom = zoom;
        viewport.transform.offset_x = fixed_world.x - screen_x as f64 / zoom;
        viewport.transform.offset_y = fixed_world.y - screen_y as f64 / zoom;
        self.emit_viewport_changed(viewport.transform);
    }

    pub fn zoom_to_fit(
        &mut self,
        viewport: &mut CanvasViewportState,
        projection: &CanvasProjection,
    ) -> bool {
        let Some(bounds) = projection.world_bounds() else {
            return false;
        };
        let available_width = (self.viewport_size.0 - DEFAULT_ZOOM_TO_FIT_PADDING * 2.0).max(1.0);
        let available_height = (self.viewport_size.1 - DEFAULT_ZOOM_TO_FIT_PADDING * 2.0).max(1.0);
        let fit_width = if bounds.width > 0.0 {
            available_width / bounds.width
        } else {
            self.max_zoom
        };
        let fit_height = if bounds.height > 0.0 {
            available_height / bounds.height
        } else {
            self.max_zoom
        };
        let zoom = self.clamp_zoom(fit_width.min(fit_height));
        let center = bounds.center();

        viewport.transform.zoom = zoom;
        viewport.transform.offset_x = center.x - self.viewport_size.0 * 0.5 / zoom;
        viewport.transform.offset_y = center.y - self.viewport_size.1 * 0.5 / zoom;
        self.emit_viewport_changed(viewport.transform);
        true
    }

    pub fn reset_view(&mut self, viewport: &mut CanvasViewportState) {
        viewport.transform = CanvasTransform::identity();
        self.emit_viewport_changed(viewport.transform);
    }

    pub fn world_to_screen(&self, viewport: &CanvasViewportState, point: WorldPoint) -> (f32, f32) {
        viewport.transform.world_to_screen(point)
    }

    pub fn screen_to_world(
        &self,
        viewport: &CanvasViewportState,
        screen_x: f32,
        screen_y: f32,
    ) -> WorldPoint {
        viewport.transform.screen_to_world(screen_x, screen_y)
    }

    pub fn viewport_center(&self, viewport: &CanvasViewportState) -> WorldPoint {
        viewport.transform.screen_to_world(
            (self.viewport_size.0 * 0.5) as f32,
            (self.viewport_size.1 * 0.5) as f32,
        )
    }

    pub fn hit_test(
        &self,
        _viewport: &CanvasViewportState,
        projection: &CanvasProjection,
        world_point: WorldPoint,
    ) -> Option<CanvasHit> {
        projection.hit_test(world_point)
    }

    pub fn emit_empty_click(&mut self, point: WorldPoint) {
        self.intents.push(CanvasIntent::EmptyClick(point));
    }

    pub fn emit_empty_context_menu(&mut self, point: WorldPoint) {
        self.intents.push(CanvasIntent::EmptyContextMenu(point));
    }

    pub fn emit_rubber_band_changed(&mut self, rect: WorldRect) {
        self.intents.push(CanvasIntent::RubberBandChanged(rect));
    }

    pub fn emit_rubber_band_committed(&mut self, rect: WorldRect) {
        self.intents.push(CanvasIntent::RubberBandCommitted(rect));
    }

    pub fn take_intents(&mut self) -> Vec<CanvasIntent> {
        std::mem::take(&mut self.intents)
    }

    pub fn adaptive_grid_spacing(&self, zoom: f64) -> f64 {
        let mut spacing = self.grid.base_spacing.max(1.0);
        let screen_min = 8.0;
        let screen_max = 80.0;
        while spacing * zoom < screen_min {
            spacing *= self.grid.subdivisions.max(2) as f64;
        }
        while spacing * zoom > screen_max {
            spacing /= self.grid.subdivisions.max(2) as f64;
        }
        spacing
    }

    fn emit_viewport_changed(&mut self, transform: CanvasTransform) {
        self.intents.push(CanvasIntent::ViewportChanged(transform));
    }

    fn clamp_zoom(&self, zoom: f64) -> f64 {
        zoom.clamp(self.min_zoom, self.max_zoom)
    }
}

impl Default for InfiniteCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasProjection {
    pub fn new(revision: ProjectionRevision, items: Vec<ProjectedCanvasItem>) -> Self {
        Self { revision, items }
    }

    pub fn revision(&self) -> ProjectionRevision {
        self.revision
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProjectedCanvasItem> {
        self.items.iter()
    }

    pub fn in_draw_order(&self) -> Vec<&ProjectedCanvasItem> {
        let mut indexed: Vec<(usize, &ProjectedCanvasItem)> =
            self.items.iter().enumerate().collect();
        indexed.sort_by_key(|(index, item)| (item.z_order, *index));
        indexed.into_iter().map(|(_, item)| item).collect()
    }

    pub fn in_region(&self, region: &WorldRect) -> Vec<&ProjectedCanvasItem> {
        self.items
            .iter()
            .filter(|item| item.bounds.intersects(region))
            .collect()
    }

    pub fn hit_test(&self, point: WorldPoint) -> Option<CanvasHit> {
        let mut best: Option<(HitPriority, i32, usize, CanvasEntityId)> = None;
        for (index, item) in self.items.iter().enumerate() {
            for region in &item.hit_regions {
                let Some((id, priority)) = region.hit(point) else {
                    continue;
                };
                let candidate = (priority, item.z_order, index, id);
                if best
                    .as_ref()
                    .is_none_or(|current| hit_candidate_beats(candidate, *current))
                {
                    best = Some(candidate);
                }
            }
        }

        best.map(|(priority, z_order, _, id)| CanvasHit {
            id,
            priority,
            z_order,
        })
    }

    pub fn world_bounds(&self) -> Option<WorldRect> {
        self.items
            .iter()
            .map(|item| item.bounds)
            .reduce(|bounds, next| bounds.union(&next))
    }
}

impl CanvasHitRegion {
    fn hit(&self, point: WorldPoint) -> Option<(CanvasEntityId, HitPriority)> {
        match self {
            CanvasHitRegion::Rect { id, rect, priority } => {
                rect.contains(point).then_some((*id, *priority))
            }
            CanvasHitRegion::Circle {
                id,
                center,
                radius,
                priority,
            } => {
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                (dx * dx + dy * dy <= radius * radius).then_some((*id, *priority))
            }
            CanvasHitRegion::Path {
                id,
                path,
                tolerance,
                priority,
            } => path_hit_test(path, point, *tolerance).then_some((*id, *priority)),
        }
    }
}

fn hit_candidate_beats(
    candidate: (HitPriority, i32, usize, CanvasEntityId),
    current: (HitPriority, i32, usize, CanvasEntityId),
) -> bool {
    candidate.0 > current.0
        || (candidate.0 == current.0
            && (candidate.1 > current.1 || (candidate.1 == current.1 && candidate.2 > current.2)))
}

fn path_hit_test(path: &Path, point: WorldPoint, tolerance: f64) -> bool {
    let points = path.flatten();
    if points.len() < 2 {
        return false;
    }

    points.windows(2).any(|segment| {
        let a = WorldPoint::new(segment[0][0] as f64, segment[0][1] as f64);
        let b = WorldPoint::new(segment[1][0] as f64, segment[1][1] as f64);
        point_to_segment_distance(point, a, b) <= tolerance
    })
}

fn point_to_segment_distance(point: WorldPoint, a: WorldPoint, b: WorldPoint) -> f64 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = point.x - a.x;
    let ap_y = point.y - a.y;
    let len_sq = ab_x * ab_x + ab_y * ab_y;
    if len_sq == 0.0 {
        return ((point.x - a.x).powi(2) + (point.y - a.y).powi(2)).sqrt();
    }

    let t = ((ap_x * ab_x + ap_y * ab_y) / len_sq).clamp(0.0, 1.0);
    let nearest = WorldPoint::new(a.x + ab_x * t, a.y + ab_y * t);
    ((point.x - nearest.x).powi(2) + (point.y - nearest.y).powi(2)).sqrt()
}
