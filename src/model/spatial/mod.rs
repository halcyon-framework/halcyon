use std::collections::HashMap;

use crate::core::event::{KeyCode, Modifiers, PointerButton};
use crate::core::math::{Point, RectF};
use crate::core::paint::Color;
use crate::engines::route_geometry::{RightAnglePlan, RouteDirection};
use crate::model::canvas::{
    CanvasEntityId, CanvasHit, CanvasHitRegion, CanvasIntent, CanvasPaintDescriptor,
    CanvasProjection, CanvasTransform, CanvasViewportState, HitPriority, WorldPoint, WorldRect,
};
use crate::renderer::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpatialItemId(u64);

impl SpatialItemId {
    pub const fn from_opaque(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SpatialProjectionRevision(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDirection {
    IncreasingPixels,
    DecreasingPixels,
}

impl SpatialDirection {
    const fn sign(self) -> f64 {
        match self {
            Self::IncreasingPixels => 1.0,
            Self::DecreasingPixels => -1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialAxisOrientation {
    x: SpatialDirection,
    y: SpatialDirection,
}

impl SpatialAxisOrientation {
    pub const fn new(x: SpatialDirection, y: SpatialDirection) -> Self {
        Self { x, y }
    }

    pub const fn x(self) -> SpatialDirection {
        self.x
    }

    pub const fn y(self) -> SpatialDirection {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialViewportError {
    NonFiniteWorldCenter,
    NonFinitePixelsPerWorldUnit,
    NonPositivePixelsPerWorldUnit,
    NonFiniteViewportRect,
    NonPositiveViewportRect,
    NonFiniteWorldPoint,
    NonFinitePixelPoint,
    NonFinitePanDelta,
    NonFiniteZoomFactor,
    NonPositiveZoomFactor,
    ArithmeticOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialViewportState {
    world_center: WorldPoint,
    pixels_per_world_unit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialTransform {
    viewport: SpatialViewportState,
    axis_orientation: SpatialAxisOrientation,
    viewport_rect: RectF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialCoordinateMode {
    AffineSurface,
    DomainMappedSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialProjectionError {
    InvalidViewport(SpatialViewportError),
    InvalidCoordinateModeAdmission {
        coordinate_mode: SpatialCoordinateMode,
        viewport_present: bool,
        axis_orientation_present: bool,
    },
}

impl From<SpatialViewportError> for SpatialProjectionError {
    fn from(value: SpatialViewportError) -> Self {
        Self::InvalidViewport(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialItemRole {
    Surface,
    Item,
    Container,
    Anchor,
    Handle,
    Route,
    Guide,
    Label,
    Overlay,
    Background,
}

#[derive(Debug, Clone)]
pub struct SpatialItem {
    pub id: SpatialItemId,
    pub role: SpatialItemRole,
    pub bounds: WorldRect,
    pub z_order: i32,
    pub primitives: Vec<SpatialPrimitive>,
}

#[derive(Debug, Clone)]
pub enum SpatialPrimitive {
    Rect(WorldRect),
    RoundedRect {
        rect: WorldRect,
        radius: f32,
    },
    Circle {
        center: WorldPoint,
        radius: f64,
    },
    Line {
        from: WorldPoint,
        to: WorldPoint,
        stroke: SpatialStrokeRole,
    },
    Path(Path),
    Route(SpatialRoutePlan),
    Guide(SpatialGuidePlan),
    TextBounds(WorldRect),
    Overlay(SpatialOverlayPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialStrokeRole {
    Default,
    Selected,
    Hovered,
    Blocked,
    Preview,
}

#[derive(Debug, Clone)]
pub enum SpatialHitRegion {
    Rect {
        id: SpatialItemId,
        rect: WorldRect,
        priority: SpatialHitPriority,
    },
    Circle {
        id: SpatialItemId,
        center: WorldPoint,
        radius: f64,
        priority: SpatialHitPriority,
    },
    Path {
        id: SpatialItemId,
        path: Path,
        tolerance: f64,
        priority: SpatialHitPriority,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpatialHitPriority {
    Background,
    Container,
    Route,
    Item,
    Anchor,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialRoutePlan {
    RightAngle(RightAnglePlan),
    Polyline {
        points: [Point; 4],
        point_count: usize,
        head_anchor: Point,
        head_dir: RouteDirection,
    },
}

/// Segment orientation produced from a realized spatial route plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialRouteSegmentAxis {
    /// Segment lies on a constant y coordinate.
    Horizontal,
    /// Segment lies on a constant x coordinate.
    Vertical,
    /// Segment is not axis-aligned.
    Freeform,
}

/// A single realized route segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialRouteSegment {
    pub from: Point,
    pub to: Point,
    pub axis: SpatialRouteSegmentAxis,
}

/// Arrowhead metadata for a realized route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialRouteHead {
    pub anchor: Point,
    pub dir: RouteDirection,
}

/// Realized route geometry derived from a route plan without owning route math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialRouteGeometry {
    segments: [SpatialRouteSegment; 3],
    segment_count: usize,
    pub head: SpatialRouteHead,
}

impl SpatialRouteGeometry {
    /// Returns realized route segments in draw order.
    pub fn segments(&self) -> &[SpatialRouteSegment] {
        &self.segments[..self.segment_count.min(self.segments.len())]
    }
}

impl SpatialRoutePlan {
    pub fn polyline(
        points: &[Point],
        head_anchor: Point,
        head_dir: RouteDirection,
    ) -> Option<Self> {
        if !(2..=4).contains(&points.len()) {
            return None;
        }

        let mut stored = [Point::zero(); 4];
        for (index, point) in points.iter().enumerate() {
            stored[index] = *point;
        }

        Some(Self::Polyline {
            points: stored,
            point_count: points.len(),
            head_anchor,
            head_dir,
        })
    }

    pub fn polyline_points(&self) -> Option<&[Point]> {
        match self {
            SpatialRoutePlan::Polyline {
                points,
                point_count,
                ..
            } => Some(&points[..(*point_count).min(points.len())]),
            SpatialRoutePlan::RightAngle(_) => None,
        }
    }
}

/// Realizes a spatial route plan into drawable segment metadata.
pub fn realize_spatial_route_plan(plan: SpatialRoutePlan) -> SpatialRouteGeometry {
    match plan {
        SpatialRoutePlan::RightAngle(plan) => spatial_route_geometry(
            &plan.points,
            [
                SpatialRouteSegmentAxis::Horizontal,
                SpatialRouteSegmentAxis::Vertical,
                SpatialRouteSegmentAxis::Horizontal,
            ],
            plan.head_anchor,
            plan.head_dir,
        ),
        SpatialRoutePlan::Polyline {
            points,
            point_count,
            head_anchor,
            head_dir,
        } => {
            let count = point_count.min(points.len());
            let mut axes = [SpatialRouteSegmentAxis::Freeform; 3];
            for index in 0..count.saturating_sub(1).min(axes.len()) {
                axes[index] = spatial_route_segment_axis(points[index], points[index + 1]);
            }
            spatial_route_geometry(&points[..count], axes, head_anchor, head_dir)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialGuideRole {
    Active,
    Structural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialGuideAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialGuidePlan {
    pub axis: SpatialGuideAxis,
    pub role: SpatialGuideRole,
    pub position: f64,
    pub start: f64,
    pub end: f64,
}

impl SpatialGuidePlan {
    pub const fn vertical(role: SpatialGuideRole, position: f64, start: f64, end: f64) -> Self {
        Self {
            axis: SpatialGuideAxis::Vertical,
            role,
            position,
            start,
            end,
        }
    }

    pub const fn horizontal(role: SpatialGuideRole, position: f64, start: f64, end: f64) -> Self {
        Self {
            axis: SpatialGuideAxis::Horizontal,
            role,
            position,
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialOverlayAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialOverlayPlan {
    pub anchor: SpatialOverlayAnchor,
    pub offset: Point,
    pub size: RectF,
    pub edge_padding: f32,
}

impl SpatialOverlayPlan {
    pub const fn anchored(anchor: SpatialOverlayAnchor, size: RectF, edge_padding: f32) -> Self {
        Self {
            anchor,
            offset: Point::zero(),
            size,
            edge_padding,
        }
    }

    pub const fn with_offset(self, offset: Point) -> Self {
        Self { offset, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialRoleVisual {
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialVisualPolicy {
    pub canvas_background: Color,
    pub grid_minor: SpatialRoleVisual,
    pub grid_major: SpatialRoleVisual,
    pub active_guide: SpatialRoleVisual,
    pub structural_guide: SpatialRoleVisual,
    pub selection_outline: SpatialRoleVisual,
    pub drag_preview: SpatialRoleVisual,
    pub handle_anchor: SpatialRoleVisual,
    pub route_default: SpatialRoleVisual,
    pub route_selected: SpatialRoleVisual,
    pub route_hovered: SpatialRoleVisual,
    pub route_blocked: SpatialRoleVisual,
    pub route_preview: SpatialRoleVisual,
    pub hud_background: Color,
    pub hud_border: Color,
    pub hud_text: Color,
    pub hud_secondary_text: Color,
    pub hud_edge_padding: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialGridPolicy {
    pub enabled: bool,
    pub base_spacing: f64,
    pub major_every: u32,
    pub min_screen_spacing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialGridLineRole {
    Minor,
    Major,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialGridLine {
    pub axis: SpatialGuideAxis,
    pub role: SpatialGridLineRole,
    pub world_position: f64,
    pub rect: RectF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialGestureClass {
    Disabled,
    PrimaryDrag,
    ShiftPrimaryDrag,
    SpacePrimaryDrag,
    MiddleDrag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialGestureFacts {
    pub modifiers: Modifiers,
    pub button: PointerButton,
    pub space_pressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialCursorPolicy {
    Default,
    Text,
    Link,
    ResizeHorizontal,
    ResizeVertical,
    DragActive,
    CrosshairActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialWheelZoomPolicy {
    Disabled,
    Unmodified,
    ControlOrMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialInteractionPolicy {
    pub pan_gesture: SpatialGestureClass,
    pub alternate_pan_gesture: SpatialGestureClass,
    pub wheel_zoom: SpatialWheelZoomPolicy,
    pub rubber_band_gesture: SpatialGestureClass,
    pub selection_toggle_requires_command: bool,
    pub drag_cursor: SpatialCursorPolicy,
    pub rubber_band_cursor: SpatialCursorPolicy,
    pub fit_key: Option<KeyCode>,
    pub reset_key: Option<KeyCode>,
    pub delete_keys_enabled: bool,
    pub route_creation_gesture: SpatialGestureClass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialPointerFacts {
    pub modifiers: Modifiers,
    pub button: PointerButton,
    pub item: Option<SpatialItemId>,
    pub surface_pos: WorldPoint,
    pub screen_pos: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpatialIntent {
    ViewportChanged(SpatialViewportState),
    HoverChanged {
        previous: Option<SpatialItemId>,
        current: Option<SpatialItemId>,
    },
    Press(SpatialPointerFacts),
    DragStarted(SpatialPointerFacts),
    DragUpdated {
        facts: SpatialPointerFacts,
        delta: WorldPoint,
    },
    DragCommitted(SpatialPointerFacts),
    DragCanceled,
    RubberBandChanged(WorldRect),
    RubberBandCommitted(WorldRect),
    KeyCommand(KeyCode),
}

#[derive(Debug, Clone)]
pub struct SpatialProjection {
    revision: SpatialProjectionRevision,
    viewport: Option<SpatialViewportState>,
    axis_orientation: Option<SpatialAxisOrientation>,
    coordinate_mode: SpatialCoordinateMode,
    items: Vec<SpatialItem>,
    hit_regions: Vec<SpatialHitRegion>,
    routes: HashMap<SpatialItemId, SpatialRoutePlan>,
    guides: Vec<SpatialGuidePlan>,
    overlays: Vec<SpatialOverlayPlan>,
    visual_policy: SpatialVisualPolicy,
}

pub trait SpatialAdapter {
    type DomainSnapshot;
    type SessionSnapshot;
    type PolicyInput;
    type Context;
    type DomainIntent;

    fn build_projection(
        &self,
        domain: &Self::DomainSnapshot,
        session: &Self::SessionSnapshot,
        viewport: Option<&SpatialViewportState>,
        orientation: Option<&SpatialAxisOrientation>,
        policy: &Self::PolicyInput,
    ) -> Result<SpatialProjection, SpatialProjectionError>;

    fn translate_intent(
        &self,
        intent: &SpatialIntent,
        context: &Self::Context,
    ) -> Option<Self::DomainIntent>;
}

#[derive(Debug, Clone)]
pub struct CanvasSpatialBridge {
    projection: SpatialProjection,
    canvas_entities: HashMap<SpatialItemId, CanvasEntityId>,
    viewport_rect: RectF,
}

impl SpatialViewportState {
    pub fn new(
        world_center: WorldPoint,
        pixels_per_world_unit: f64,
    ) -> Result<Self, SpatialViewportError> {
        if !world_center.x.is_finite() || !world_center.y.is_finite() {
            return Err(SpatialViewportError::NonFiniteWorldCenter);
        }
        if !pixels_per_world_unit.is_finite() {
            return Err(SpatialViewportError::NonFinitePixelsPerWorldUnit);
        }
        if pixels_per_world_unit <= 0.0 {
            return Err(SpatialViewportError::NonPositivePixelsPerWorldUnit);
        }
        Ok(Self {
            world_center,
            pixels_per_world_unit,
        })
    }

    pub const fn world_center(self) -> WorldPoint {
        self.world_center
    }

    pub const fn pixels_per_world_unit(self) -> f64 {
        self.pixels_per_world_unit
    }

    pub fn pan(self, delta: WorldPoint) -> Result<Self, SpatialViewportError> {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return Err(SpatialViewportError::NonFinitePanDelta);
        }
        let world_center =
            WorldPoint::new(self.world_center.x + delta.x, self.world_center.y + delta.y);
        if !world_center.x.is_finite() || !world_center.y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Self::new(world_center, self.pixels_per_world_unit)
    }
}

impl Default for SpatialViewportState {
    fn default() -> Self {
        Self {
            world_center: WorldPoint::new(0.0, 0.0),
            pixels_per_world_unit: 1.0,
        }
    }
}

impl SpatialTransform {
    pub fn new(
        viewport: SpatialViewportState,
        axis_orientation: SpatialAxisOrientation,
        viewport_rect: RectF,
    ) -> Result<Self, SpatialViewportError> {
        validate_spatial_viewport_rect(viewport_rect)?;
        Ok(Self {
            viewport,
            axis_orientation,
            viewport_rect,
        })
    }

    pub const fn viewport(self) -> SpatialViewportState {
        self.viewport
    }

    pub const fn axis_orientation(self) -> SpatialAxisOrientation {
        self.axis_orientation
    }

    pub const fn viewport_rect(self) -> RectF {
        self.viewport_rect
    }

    pub fn world_to_screen(&self, point: WorldPoint) -> Result<(f32, f32), SpatialViewportError> {
        let (pixel_x, pixel_y) = self.world_to_screen_f64(point)?;
        Ok((checked_pixel_f32(pixel_x)?, checked_pixel_f32(pixel_y)?))
    }

    pub fn screen_to_world(
        &self,
        screen_x: f32,
        screen_y: f32,
    ) -> Result<WorldPoint, SpatialViewportError> {
        if !screen_x.is_finite() || !screen_y.is_finite() {
            return Err(SpatialViewportError::NonFinitePixelPoint);
        }
        self.screen_to_world_f64(f64::from(screen_x), f64::from(screen_y))
    }

    pub fn visible_world_bounds(&self) -> Result<WorldRect, SpatialViewportError> {
        let scale = self.viewport.pixels_per_world_unit;
        let half_width = f64::from(self.viewport_rect.width) / (2.0 * scale);
        let half_height = f64::from(self.viewport_rect.height) / (2.0 * scale);
        let center = self.viewport.world_center;
        let x = center.x - half_width;
        let y = center.y - half_height;
        let width = half_width * 2.0;
        let height = half_height * 2.0;
        let right = x + width;
        let bottom = y + height;
        if !half_width.is_finite()
            || !half_height.is_finite()
            || !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || !right.is_finite()
            || !bottom.is_finite()
        {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Ok(WorldRect::new(x, y, width, height))
    }

    pub fn anchored_zoom_at_pixel(
        &self,
        factor: f64,
        anchor_pixel: Point,
    ) -> Result<SpatialViewportState, SpatialViewportError> {
        if !anchor_pixel.x.is_finite() || !anchor_pixel.y.is_finite() {
            return Err(SpatialViewportError::NonFinitePixelPoint);
        }
        let anchor_world = self.screen_to_world(anchor_pixel.x, anchor_pixel.y)?;
        self.anchored_zoom_from_pair(
            factor,
            anchor_world,
            f64::from(anchor_pixel.x),
            f64::from(anchor_pixel.y),
        )
    }

    pub fn anchored_zoom_at_world(
        &self,
        factor: f64,
        anchor_world: WorldPoint,
    ) -> Result<SpatialViewportState, SpatialViewportError> {
        let (anchor_pixel_x, anchor_pixel_y) = self.world_to_screen_f64(anchor_world)?;
        self.anchored_zoom_from_pair(factor, anchor_world, anchor_pixel_x, anchor_pixel_y)
    }

    fn world_to_screen_f64(&self, point: WorldPoint) -> Result<(f64, f64), SpatialViewportError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(SpatialViewportError::NonFiniteWorldPoint);
        }
        let (pixel_center_x, pixel_center_y) = self.pixel_center()?;
        let scale = self.viewport.pixels_per_world_unit;
        let pixel_x = pixel_center_x
            + self.axis_orientation.x.sign() * (point.x - self.viewport.world_center.x) * scale;
        let pixel_y = pixel_center_y
            + self.axis_orientation.y.sign() * (point.y - self.viewport.world_center.y) * scale;
        if !pixel_x.is_finite() || !pixel_y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Ok((pixel_x, pixel_y))
    }

    fn screen_to_world_f64(
        &self,
        screen_x: f64,
        screen_y: f64,
    ) -> Result<WorldPoint, SpatialViewportError> {
        if !screen_x.is_finite() || !screen_y.is_finite() {
            return Err(SpatialViewportError::NonFinitePixelPoint);
        }
        let (pixel_center_x, pixel_center_y) = self.pixel_center()?;
        let scale = self.viewport.pixels_per_world_unit;
        let world_x = self.viewport.world_center.x
            + (screen_x - pixel_center_x) / (self.axis_orientation.x.sign() * scale);
        let world_y = self.viewport.world_center.y
            + (screen_y - pixel_center_y) / (self.axis_orientation.y.sign() * scale);
        if !world_x.is_finite() || !world_y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Ok(WorldPoint::new(world_x, world_y))
    }

    fn anchored_zoom_from_pair(
        &self,
        factor: f64,
        anchor_world: WorldPoint,
        anchor_pixel_x: f64,
        anchor_pixel_y: f64,
    ) -> Result<SpatialViewportState, SpatialViewportError> {
        if !factor.is_finite() {
            return Err(SpatialViewportError::NonFiniteZoomFactor);
        }
        if factor <= 0.0 {
            return Err(SpatialViewportError::NonPositiveZoomFactor);
        }
        if !anchor_world.x.is_finite() || !anchor_world.y.is_finite() {
            return Err(SpatialViewportError::NonFiniteWorldPoint);
        }
        let new_scale = self.viewport.pixels_per_world_unit * factor;
        if !new_scale.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        let (pixel_center_x, pixel_center_y) = self.pixel_center()?;
        let world_center = WorldPoint::new(
            anchor_world.x
                - (anchor_pixel_x - pixel_center_x) / (self.axis_orientation.x.sign() * new_scale),
            anchor_world.y
                - (anchor_pixel_y - pixel_center_y) / (self.axis_orientation.y.sign() * new_scale),
        );
        if !world_center.x.is_finite() || !world_center.y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        SpatialViewportState::new(world_center, new_scale)
    }

    fn pixel_center(&self) -> Result<(f64, f64), SpatialViewportError> {
        let center_x = f64::from(self.viewport_rect.x) + f64::from(self.viewport_rect.width) * 0.5;
        let center_y = f64::from(self.viewport_rect.y) + f64::from(self.viewport_rect.height) * 0.5;
        if !center_x.is_finite() || !center_y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Ok((center_x, center_y))
    }
}

fn validate_spatial_viewport_rect(rect: RectF) -> Result<(), SpatialViewportError> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
    {
        return Err(SpatialViewportError::NonFiniteViewportRect);
    }
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Err(SpatialViewportError::NonPositiveViewportRect);
    }
    Ok(())
}

fn checked_pixel_f32(value: f64) -> Result<f32, SpatialViewportError> {
    let value = value as f32;
    value
        .is_finite()
        .then_some(value)
        .ok_or(SpatialViewportError::ArithmeticOverflow)
}

impl Default for SpatialVisualPolicy {
    fn default() -> Self {
        Self::for_theme(true)
    }
}

impl SpatialVisualPolicy {
    pub fn for_theme(dark: bool) -> Self {
        let transparent = Color::from_linear(0.0, 0.0, 0.0, 0.0);
        let (
            background,
            minor,
            major,
            active,
            structural,
            selected,
            preview_fill,
            route,
            hud_background,
            hud_border,
            hud_text,
            hud_secondary_text,
        ) = if dark {
            (
                Color::from_srgb(0.025, 0.030, 0.040, 1.0),
                Color::from_srgb(0.12, 0.14, 0.18, 0.42),
                Color::from_srgb(0.18, 0.21, 0.27, 0.55),
                Color::from_srgb(1.0, 0.24, 0.62, 0.88),
                Color::from_srgb(1.0, 0.24, 0.62, 0.34),
                Color::from_srgb(0.56, 0.78, 1.0, 0.88),
                Color::from_srgb(0.38, 0.66, 1.0, 0.14),
                Color::from_srgb(0.72, 0.78, 0.86, 1.0),
                Color::from_srgb(0.05, 0.06, 0.08, 0.92),
                Color::from_srgb(0.45, 0.50, 0.58, 0.55),
                Color::WHITE,
                Color::from_srgb(0.78, 0.83, 0.90, 1.0),
            )
        } else {
            (
                Color::from_srgb(0.965, 0.970, 0.975, 1.0),
                Color::from_srgb(0.16, 0.18, 0.22, 0.09),
                Color::from_srgb(0.12, 0.14, 0.18, 0.17),
                Color::from_srgb(0.82, 0.0, 0.42, 0.78),
                Color::from_srgb(0.82, 0.0, 0.42, 0.28),
                Color::from_srgb(0.56, 0.78, 1.0, 0.88),
                Color::from_srgb(0.38, 0.66, 1.0, 0.14),
                Color::from_srgb(0.30, 0.34, 0.42, 1.0),
                Color::from_srgb(0.98, 0.985, 0.99, 0.88),
                Color::from_srgb(0.36, 0.40, 0.48, 0.34),
                Color::from_srgb(0.12, 0.14, 0.18, 1.0),
                Color::from_srgb(0.30, 0.34, 0.42, 1.0),
            )
        };
        Self {
            canvas_background: background,
            grid_minor: SpatialRoleVisual {
                fill: transparent,
                stroke: minor,
                stroke_width: 1.0,
            },
            grid_major: SpatialRoleVisual {
                fill: transparent,
                stroke: major,
                stroke_width: 1.0,
            },
            active_guide: SpatialRoleVisual {
                fill: transparent,
                stroke: active,
                stroke_width: 1.5,
            },
            structural_guide: SpatialRoleVisual {
                fill: transparent,
                stroke: structural,
                stroke_width: 1.0,
            },
            selection_outline: SpatialRoleVisual {
                fill: transparent,
                stroke: selected,
                stroke_width: 1.0,
            },
            drag_preview: SpatialRoleVisual {
                fill: preview_fill,
                stroke: selected,
                stroke_width: 1.0,
            },
            handle_anchor: SpatialRoleVisual {
                fill: background,
                stroke: selected,
                stroke_width: 1.0,
            },
            route_default: SpatialRoleVisual {
                fill: transparent,
                stroke: route,
                stroke_width: 1.0,
            },
            route_selected: SpatialRoleVisual {
                fill: transparent,
                stroke: selected,
                stroke_width: 2.0,
            },
            route_hovered: SpatialRoleVisual {
                fill: transparent,
                stroke: active,
                stroke_width: 2.0,
            },
            route_blocked: SpatialRoleVisual {
                fill: transparent,
                stroke: Color::from_linear(0.95, 0.22, 0.27, 1.0),
                stroke_width: 2.0,
            },
            route_preview: SpatialRoleVisual {
                fill: transparent,
                stroke: preview_fill,
                stroke_width: 1.0,
            },
            hud_background,
            hud_border,
            hud_text,
            hud_secondary_text,
            hud_edge_padding: 22.0,
        }
    }
}

impl Default for SpatialGridPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            base_spacing: 32.0,
            major_every: 4,
            min_screen_spacing: 8.0,
        }
    }
}

impl Default for SpatialInteractionPolicy {
    fn default() -> Self {
        Self {
            pan_gesture: SpatialGestureClass::SpacePrimaryDrag,
            alternate_pan_gesture: SpatialGestureClass::MiddleDrag,
            wheel_zoom: SpatialWheelZoomPolicy::ControlOrMeta,
            rubber_band_gesture: SpatialGestureClass::ShiftPrimaryDrag,
            selection_toggle_requires_command: true,
            drag_cursor: SpatialCursorPolicy::DragActive,
            rubber_band_cursor: SpatialCursorPolicy::CrosshairActive,
            fit_key: Some(KeyCode::Character('f')),
            reset_key: Some(KeyCode::Home),
            delete_keys_enabled: true,
            route_creation_gesture: SpatialGestureClass::PrimaryDrag,
        }
    }
}

impl SpatialInteractionPolicy {
    pub fn wheel_zoom_requested(&self, modifiers: Modifiers) -> bool {
        match self.wheel_zoom {
            SpatialWheelZoomPolicy::Disabled => false,
            SpatialWheelZoomPolicy::Unmodified => true,
            SpatialWheelZoomPolicy::ControlOrMeta => modifiers.ctrl || modifiers.meta,
        }
    }

    pub fn gesture_class(&self, facts: SpatialGestureFacts) -> SpatialGestureClass {
        resolve_spatial_gesture_class(facts)
    }

    pub fn is_pan_gesture(&self, gesture: SpatialGestureClass) -> bool {
        gesture != SpatialGestureClass::Disabled
            && (gesture == self.pan_gesture || gesture == self.alternate_pan_gesture)
    }

    pub fn is_rubber_band_gesture(&self, gesture: SpatialGestureClass) -> bool {
        gesture == self.rubber_band_gesture
    }

    pub fn is_route_creation_gesture(&self, gesture: SpatialGestureClass) -> bool {
        gesture == self.route_creation_gesture
    }

    pub fn selection_toggle_requested(&self, modifiers: Modifiers) -> bool {
        !self.selection_toggle_requires_command || modifiers.meta || modifiers.ctrl
    }

    pub fn cursor_for_gesture(
        &self,
        gesture: SpatialGestureClass,
        active: bool,
    ) -> SpatialCursorPolicy {
        if !active {
            return SpatialCursorPolicy::Default;
        }

        if self.is_rubber_band_gesture(gesture) {
            return self.rubber_band_cursor;
        }

        match gesture {
            SpatialGestureClass::PrimaryDrag
            | SpatialGestureClass::ShiftPrimaryDrag
            | SpatialGestureClass::SpacePrimaryDrag
            | SpatialGestureClass::MiddleDrag => self.drag_cursor,
            SpatialGestureClass::Disabled => SpatialCursorPolicy::Default,
        }
    }

    pub fn cursor_for_route_creation(
        &self,
        gesture: SpatialGestureClass,
        active: bool,
    ) -> SpatialCursorPolicy {
        if active && self.is_route_creation_gesture(gesture) {
            SpatialCursorPolicy::CrosshairActive
        } else {
            self.cursor_for_gesture(gesture, active)
        }
    }

    pub fn key_intent(&self, key: KeyCode) -> Option<SpatialIntent> {
        let is_delete =
            self.delete_keys_enabled && matches!(key, KeyCode::Delete | KeyCode::Backspace);
        let is_fit = self.fit_key == Some(key);
        let is_reset = self.reset_key == Some(key);

        (is_delete || is_fit || is_reset).then_some(SpatialIntent::KeyCommand(key))
    }
}

impl SpatialProjection {
    pub fn new(
        revision: SpatialProjectionRevision,
        viewport: Option<SpatialViewportState>,
        axis_orientation: Option<SpatialAxisOrientation>,
        coordinate_mode: SpatialCoordinateMode,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<Self, SpatialProjectionError> {
        Self::from_parts(
            revision,
            viewport,
            axis_orientation,
            coordinate_mode,
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            Vec::new(),
            Vec::new(),
            visual_policy,
        )
    }

    pub fn from_parts(
        revision: SpatialProjectionRevision,
        viewport: Option<SpatialViewportState>,
        axis_orientation: Option<SpatialAxisOrientation>,
        coordinate_mode: SpatialCoordinateMode,
        items: Vec<SpatialItem>,
        hit_regions: Vec<SpatialHitRegion>,
        routes: HashMap<SpatialItemId, SpatialRoutePlan>,
        guides: Vec<SpatialGuidePlan>,
        overlays: Vec<SpatialOverlayPlan>,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<Self, SpatialProjectionError> {
        let admitted = matches!(
            (coordinate_mode, viewport, axis_orientation),
            (SpatialCoordinateMode::AffineSurface, Some(_), Some(_))
                | (SpatialCoordinateMode::DomainMappedSurface, None, None)
        );
        if !admitted {
            return Err(SpatialProjectionError::InvalidCoordinateModeAdmission {
                coordinate_mode,
                viewport_present: viewport.is_some(),
                axis_orientation_present: axis_orientation.is_some(),
            });
        }
        Ok(Self {
            revision,
            viewport,
            axis_orientation,
            coordinate_mode,
            items,
            hit_regions,
            routes,
            guides,
            overlays,
            visual_policy,
        })
    }

    pub fn revision(&self) -> SpatialProjectionRevision {
        self.revision
    }

    pub fn viewport(&self) -> Option<SpatialViewportState> {
        self.viewport
    }

    pub fn axis_orientation(&self) -> Option<SpatialAxisOrientation> {
        self.axis_orientation
    }

    pub fn coordinate_mode(&self) -> SpatialCoordinateMode {
        self.coordinate_mode
    }

    pub fn items(&self) -> &[SpatialItem] {
        &self.items
    }

    pub fn hit_regions(&self) -> &[SpatialHitRegion] {
        &self.hit_regions
    }

    pub fn routes(&self) -> &HashMap<SpatialItemId, SpatialRoutePlan> {
        &self.routes
    }

    pub fn guides(&self) -> &[SpatialGuidePlan] {
        &self.guides
    }

    pub fn overlays(&self) -> &[SpatialOverlayPlan] {
        &self.overlays
    }

    pub fn visual_policy(&self) -> &SpatialVisualPolicy {
        &self.visual_policy
    }

    pub fn in_draw_order(&self) -> Vec<&SpatialItem> {
        let mut indexed: Vec<(usize, &SpatialItem)> = self.items.iter().enumerate().collect();
        indexed.sort_by_key(|(index, item)| (item.z_order, *index));
        indexed.into_iter().map(|(_, item)| item).collect()
    }

    pub fn in_region(&self, region: &WorldRect) -> Vec<&SpatialItem> {
        self.items
            .iter()
            .filter(|item| spatial_item_intersects_region(item, region))
            .collect()
    }

    pub fn label_bounds(&self) -> Vec<WorldRect> {
        self.items
            .iter()
            .flat_map(|item| item.primitives.iter().filter_map(spatial_label_bounds))
            .collect()
    }

    pub fn hit_test(&self, point: WorldPoint) -> Option<SpatialHit> {
        let mut best: Option<(SpatialHitPriority, i32, usize, SpatialItemId)> = None;
        for (index, item) in self.items.iter().enumerate() {
            for region in self
                .hit_regions
                .iter()
                .filter(|region| region.id() == item.id)
            {
                let Some((id, priority)) = region.hit(point) else {
                    continue;
                };
                let candidate = (priority, item.z_order, index, id);
                if best
                    .as_ref()
                    .is_none_or(|current| spatial_hit_candidate_beats(candidate, *current))
                {
                    best = Some(candidate);
                }
            }
        }

        best.map(|(priority, z_order, _, id)| SpatialHit {
            id,
            priority,
            z_order,
        })
    }

    pub fn world_bounds(&self) -> Option<WorldRect> {
        self.items
            .iter()
            .flat_map(|item| {
                std::iter::once(item.bounds)
                    .chain(item.primitives.iter().filter_map(spatial_label_bounds))
            })
            .reduce(|bounds, next| bounds.union(&next))
    }
}

fn spatial_label_bounds(primitive: &SpatialPrimitive) -> Option<WorldRect> {
    match primitive {
        SpatialPrimitive::TextBounds(rect) => Some(*rect),
        _ => None,
    }
}

fn spatial_item_intersects_region(item: &SpatialItem, region: &WorldRect) -> bool {
    item.bounds.intersects(region)
        || item
            .primitives
            .iter()
            .filter_map(spatial_label_bounds)
            .any(|bounds| bounds.intersects(region))
}

pub fn realize_spatial_grid(
    transform: SpatialTransform,
    policy: SpatialGridPolicy,
) -> Result<Vec<SpatialGridLine>, SpatialViewportError> {
    if !policy.enabled {
        return Ok(Vec::new());
    }

    let world_step = policy.base_spacing.max(1.0);
    let spacing = checked_pixel_f32(world_step * transform.viewport.pixels_per_world_unit)?;
    if spacing < policy.min_screen_spacing.max(1.0) {
        return Ok(Vec::new());
    }

    let viewport = transform.viewport_rect;
    let visible_bounds = transform.visible_world_bounds()?;
    let mut lines = Vec::new();
    let major_every = policy.major_every.max(1) as i64;

    let world_left = visible_bounds.x;
    let world_right = visible_bounds.x + visible_bounds.width;
    if !world_right.is_finite() {
        return Err(SpatialViewportError::ArithmeticOverflow);
    }
    let first_x = spatial_grid_floor_index(world_left / world_step)?
        .checked_sub(1)
        .ok_or(SpatialViewportError::ArithmeticOverflow)?;
    let last_x = spatial_grid_ceil_index(world_right / world_step)?
        .checked_add(1)
        .ok_or(SpatialViewportError::ArithmeticOverflow)?;
    for grid_x in first_x..=last_x {
        let world_x = grid_x as f64 * world_step;
        if !world_x.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        let (x, _) = transform
            .world_to_screen(WorldPoint::new(world_x, transform.viewport.world_center.y))?;
        let x = stable_spatial_grid_line(x);
        if x < viewport.x - 1.0 || x > viewport.x + viewport.width + 1.0 {
            continue;
        }
        lines.push(SpatialGridLine {
            axis: SpatialGuideAxis::Vertical,
            role: if grid_x.rem_euclid(major_every) == 0 {
                SpatialGridLineRole::Major
            } else {
                SpatialGridLineRole::Minor
            },
            world_position: world_x,
            rect: RectF::new(x, viewport.y, 1.0, viewport.height),
        });
    }

    let world_top = visible_bounds.y;
    let world_bottom = visible_bounds.y + visible_bounds.height;
    if !world_bottom.is_finite() {
        return Err(SpatialViewportError::ArithmeticOverflow);
    }
    let first_y = spatial_grid_floor_index(world_top / world_step)?
        .checked_sub(1)
        .ok_or(SpatialViewportError::ArithmeticOverflow)?;
    let last_y = spatial_grid_ceil_index(world_bottom / world_step)?
        .checked_add(1)
        .ok_or(SpatialViewportError::ArithmeticOverflow)?;
    for grid_y in first_y..=last_y {
        let world_y = grid_y as f64 * world_step;
        if !world_y.is_finite() {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        let (_, y) = transform
            .world_to_screen(WorldPoint::new(transform.viewport.world_center.x, world_y))?;
        let y = stable_spatial_grid_line(y);
        if y < viewport.y - 1.0 || y > viewport.y + viewport.height + 1.0 {
            continue;
        }
        lines.push(SpatialGridLine {
            axis: SpatialGuideAxis::Horizontal,
            role: if grid_y.rem_euclid(major_every) == 0 {
                SpatialGridLineRole::Major
            } else {
                SpatialGridLineRole::Minor
            },
            world_position: world_y,
            rect: RectF::new(viewport.x, y, viewport.width, 1.0),
        });
    }

    Ok(lines)
}

fn spatial_grid_floor_index(value: f64) -> Result<i64, SpatialViewportError> {
    spatial_grid_index(value.floor())
}

fn spatial_grid_ceil_index(value: f64) -> Result<i64, SpatialViewportError> {
    spatial_grid_index(value.ceil())
}

fn spatial_grid_index(value: f64) -> Result<i64, SpatialViewportError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= i64::MAX as f64 {
        return Err(SpatialViewportError::ArithmeticOverflow);
    }
    Ok(value as i64)
}

pub fn resolve_spatial_overlay_rect(viewport: RectF, plan: SpatialOverlayPlan) -> RectF {
    let width = plan.size.width;
    let height = plan.size.height;
    let left = viewport.x + plan.edge_padding + plan.offset.x;
    let right = viewport.x + viewport.width - plan.edge_padding - width + plan.offset.x;
    let top = viewport.y + plan.edge_padding + plan.offset.y;
    let bottom = viewport.y + viewport.height - plan.edge_padding - height + plan.offset.y;

    let (x, y) = match plan.anchor {
        SpatialOverlayAnchor::TopLeft => (left, top),
        SpatialOverlayAnchor::TopRight => (right, top),
        SpatialOverlayAnchor::BottomLeft => (left, bottom),
        SpatialOverlayAnchor::BottomRight => (right, bottom),
    };
    RectF::new(x, y, width, height)
}

pub fn resolve_spatial_gesture_class(facts: SpatialGestureFacts) -> SpatialGestureClass {
    match facts.button {
        PointerButton::Primary if facts.space_pressed => SpatialGestureClass::SpacePrimaryDrag,
        PointerButton::Primary if facts.modifiers.shift => SpatialGestureClass::ShiftPrimaryDrag,
        PointerButton::Primary => SpatialGestureClass::PrimaryDrag,
        PointerButton::Auxiliary => SpatialGestureClass::MiddleDrag,
        PointerButton::Secondary => SpatialGestureClass::Disabled,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialHit {
    pub id: SpatialItemId,
    pub priority: SpatialHitPriority,
    pub z_order: i32,
}

impl SpatialHitRegion {
    pub fn id(&self) -> SpatialItemId {
        match self {
            SpatialHitRegion::Rect { id, .. }
            | SpatialHitRegion::Circle { id, .. }
            | SpatialHitRegion::Path { id, .. } => *id,
        }
    }

    fn hit(&self, point: WorldPoint) -> Option<(SpatialItemId, SpatialHitPriority)> {
        match self {
            SpatialHitRegion::Rect { id, rect, priority } => {
                rect.contains(point).then_some((*id, *priority))
            }
            SpatialHitRegion::Circle {
                id,
                center,
                radius,
                priority,
            } => {
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                (dx * dx + dy * dy <= radius * radius).then_some((*id, *priority))
            }
            SpatialHitRegion::Path {
                id,
                path,
                tolerance,
                priority,
            } => spatial_path_hit_test(path, point, *tolerance).then_some((*id, *priority)),
        }
    }
}

impl CanvasSpatialBridge {
    pub const fn axis_orientation() -> SpatialAxisOrientation {
        SpatialAxisOrientation::new(
            SpatialDirection::IncreasingPixels,
            SpatialDirection::IncreasingPixels,
        )
    }

    pub fn from_canvas_projection(
        projection: &CanvasProjection,
        viewport: CanvasViewportState,
        viewport_rect: RectF,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<Self, SpatialProjectionError> {
        let viewport = Self::spatial_viewport_state(viewport, viewport_rect)?;
        let (projection, canvas_entities) = Self::project_canvas(
            projection,
            Some(viewport),
            Some(Self::axis_orientation()),
            visual_policy,
        )?;
        Ok(Self {
            projection,
            canvas_entities,
            viewport_rect,
        })
    }

    pub(crate) fn project_canvas(
        projection: &CanvasProjection,
        viewport: Option<SpatialViewportState>,
        axis_orientation: Option<SpatialAxisOrientation>,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<(SpatialProjection, HashMap<SpatialItemId, CanvasEntityId>), SpatialProjectionError>
    {
        let mut canvas_entities = HashMap::new();
        let mut hit_regions = Vec::new();
        let mut items = Vec::new();

        for item in projection.iter() {
            let id = spatial_item_id_for_canvas(item.id, canvas_role_discriminator(item));
            canvas_entities.insert(id, item.id);
            for region in &item.hit_regions {
                hit_regions.push(canvas_hit_region_to_spatial(region, id));
            }
            items.push(SpatialItem {
                id,
                role: canvas_item_role(item),
                bounds: item.bounds,
                z_order: item.z_order,
                primitives: canvas_item_primitives(item),
            });
        }

        let projection = SpatialProjection::from_parts(
            SpatialProjectionRevision(projection.revision().0),
            viewport,
            axis_orientation,
            SpatialCoordinateMode::AffineSurface,
            items,
            hit_regions,
            HashMap::new(),
            Vec::new(),
            Vec::new(),
            visual_policy,
        )?;
        Ok((projection, canvas_entities))
    }

    pub fn spatial_viewport_state(
        viewport: CanvasViewportState,
        viewport_rect: RectF,
    ) -> Result<SpatialViewportState, SpatialViewportError> {
        validate_spatial_viewport_rect(viewport_rect)?;
        let canvas = viewport.transform;
        if !canvas.offset_x.is_finite() || !canvas.offset_y.is_finite() {
            return Err(SpatialViewportError::NonFiniteWorldCenter);
        }
        if !canvas.zoom.is_finite() {
            return Err(SpatialViewportError::NonFinitePixelsPerWorldUnit);
        }
        if canvas.zoom <= 0.0 {
            return Err(SpatialViewportError::NonPositivePixelsPerWorldUnit);
        }
        let half_width = f64::from(viewport_rect.width) / (2.0 * canvas.zoom);
        let half_height = f64::from(viewport_rect.height) / (2.0 * canvas.zoom);
        let world_center =
            WorldPoint::new(canvas.offset_x + half_width, canvas.offset_y + half_height);
        if !half_width.is_finite()
            || !half_height.is_finite()
            || !world_center.x.is_finite()
            || !world_center.y.is_finite()
        {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        SpatialViewportState::new(world_center, canvas.zoom)
    }

    pub fn spatial_transform(
        viewport: CanvasViewportState,
        viewport_rect: RectF,
    ) -> Result<SpatialTransform, SpatialViewportError> {
        SpatialTransform::new(
            Self::spatial_viewport_state(viewport, viewport_rect)?,
            Self::axis_orientation(),
            viewport_rect,
        )
    }

    pub fn canvas_viewport_state(
        &self,
        viewport: SpatialViewportState,
    ) -> Result<CanvasViewportState, SpatialViewportError> {
        Self::canvas_viewport_state_for_rect(viewport, self.viewport_rect)
    }

    pub fn canvas_viewport_state_for_rect(
        viewport: SpatialViewportState,
        viewport_rect: RectF,
    ) -> Result<CanvasViewportState, SpatialViewportError> {
        validate_spatial_viewport_rect(viewport_rect)?;
        let scale = viewport.pixels_per_world_unit;
        let half_width = f64::from(viewport_rect.width) / (2.0 * scale);
        let half_height = f64::from(viewport_rect.height) / (2.0 * scale);
        let transform = CanvasTransform {
            offset_x: viewport.world_center.x - half_width,
            offset_y: viewport.world_center.y - half_height,
            zoom: scale,
        };
        if !half_width.is_finite()
            || !half_height.is_finite()
            || !transform.offset_x.is_finite()
            || !transform.offset_y.is_finite()
        {
            return Err(SpatialViewportError::ArithmeticOverflow);
        }
        Ok(CanvasViewportState { transform })
    }

    pub fn spatial_intent_from_canvas(
        &self,
        value: CanvasIntent,
    ) -> Result<SpatialIntent, SpatialViewportError> {
        Ok(match value {
            CanvasIntent::Hit(hit) => SpatialIntent::Press(SpatialPointerFacts {
                modifiers: Modifiers::default(),
                button: PointerButton::Primary,
                item: Some(spatial_item_id_for_canvas_hit(hit)),
                surface_pos: WorldPoint::new(0.0, 0.0),
                screen_pos: Point::zero(),
            }),
            CanvasIntent::EmptyClick(point) | CanvasIntent::EmptyContextMenu(point) => {
                SpatialIntent::Press(SpatialPointerFacts {
                    modifiers: Modifiers::default(),
                    button: PointerButton::Primary,
                    item: None,
                    surface_pos: point,
                    screen_pos: Point::zero(),
                })
            }
            CanvasIntent::RubberBandChanged(rect) => SpatialIntent::RubberBandChanged(rect),
            CanvasIntent::RubberBandCommitted(rect) => SpatialIntent::RubberBandCommitted(rect),
            CanvasIntent::ViewportChanged(transform) => {
                SpatialIntent::ViewportChanged(Self::spatial_viewport_state(
                    CanvasViewportState { transform },
                    self.viewport_rect,
                )?)
            }
        })
    }

    pub fn projection(&self) -> &SpatialProjection {
        &self.projection
    }

    pub fn into_projection(self) -> SpatialProjection {
        self.projection
    }

    pub fn canvas_entity(&self, id: SpatialItemId) -> Option<CanvasEntityId> {
        self.canvas_entities.get(&id).copied()
    }

    pub fn spatial_id_for_canvas(&self, entity: CanvasEntityId) -> Option<SpatialItemId> {
        self.canvas_entities
            .iter()
            .find_map(|(spatial_id, canvas_id)| (*canvas_id == entity).then_some(*spatial_id))
    }
}

impl From<HitPriority> for SpatialHitPriority {
    fn from(value: HitPriority) -> Self {
        match value {
            HitPriority::Background => SpatialHitPriority::Background,
            HitPriority::Frame => SpatialHitPriority::Container,
            HitPriority::Wire => SpatialHitPriority::Route,
            HitPriority::Node => SpatialHitPriority::Item,
            HitPriority::Port => SpatialHitPriority::Anchor,
            HitPriority::Overlay => SpatialHitPriority::Overlay,
        }
    }
}

fn canvas_item_role(item: &crate::model::canvas::ProjectedCanvasItem) -> SpatialItemRole {
    match item.id {
        CanvasEntityId::Port { .. } => SpatialItemRole::Anchor,
        _ => match item.paint {
            CanvasPaintDescriptor::Wire(_) => SpatialItemRole::Route,
            CanvasPaintDescriptor::Frame(_) => SpatialItemRole::Container,
            CanvasPaintDescriptor::Annotation(_) => SpatialItemRole::Overlay,
            CanvasPaintDescriptor::None
            | CanvasPaintDescriptor::Node(_)
            | CanvasPaintDescriptor::Custom(_) => SpatialItemRole::Item,
        },
    }
}

fn canvas_item_primitives(
    item: &crate::model::canvas::ProjectedCanvasItem,
) -> Vec<SpatialPrimitive> {
    if matches!(item.id, CanvasEntityId::Port { .. }) {
        return item
            .hit_regions
            .iter()
            .filter_map(|region| match region {
                CanvasHitRegion::Circle { center, radius, .. } => Some(SpatialPrimitive::Circle {
                    center: *center,
                    radius: *radius,
                }),
                _ => None,
            })
            .collect();
    }

    match item.paint {
        CanvasPaintDescriptor::Wire(_) => item
            .hit_regions
            .iter()
            .filter_map(|region| match region {
                CanvasHitRegion::Path { path, .. } => Some(SpatialPrimitive::Path(path.clone())),
                _ => None,
            })
            .collect(),
        CanvasPaintDescriptor::Frame(_) | CanvasPaintDescriptor::Node(_) => {
            vec![
                SpatialPrimitive::Rect(item.bounds),
                SpatialPrimitive::TextBounds(item.bounds),
            ]
        }
        CanvasPaintDescriptor::Annotation(_) | CanvasPaintDescriptor::Custom(_) => {
            vec![SpatialPrimitive::TextBounds(item.bounds)]
        }
        CanvasPaintDescriptor::None => vec![SpatialPrimitive::Rect(item.bounds)],
    }
}

fn canvas_hit_region_to_spatial(
    region: &CanvasHitRegion,
    spatial_id: SpatialItemId,
) -> SpatialHitRegion {
    match region {
        CanvasHitRegion::Rect { rect, priority, .. } => SpatialHitRegion::Rect {
            id: spatial_id,
            rect: *rect,
            priority: (*priority).into(),
        },
        CanvasHitRegion::Circle {
            center,
            radius,
            priority,
            ..
        } => SpatialHitRegion::Circle {
            id: spatial_id,
            center: *center,
            radius: *radius,
            priority: (*priority).into(),
        },
        CanvasHitRegion::Path {
            path,
            tolerance,
            priority,
            ..
        } => SpatialHitRegion::Path {
            id: spatial_id,
            path: path.clone(),
            tolerance: *tolerance,
            priority: (*priority).into(),
        },
    }
}

fn canvas_role_discriminator(item: &crate::model::canvas::ProjectedCanvasItem) -> u16 {
    match item.id {
        CanvasEntityId::Port { .. } => hit_priority_discriminator(HitPriority::Port),
        _ => match item.paint {
            CanvasPaintDescriptor::Frame(_) => hit_priority_discriminator(HitPriority::Frame),
            CanvasPaintDescriptor::Wire(_) => hit_priority_discriminator(HitPriority::Wire),
            CanvasPaintDescriptor::Annotation(_) => {
                hit_priority_discriminator(HitPriority::Overlay)
            }
            CanvasPaintDescriptor::None
            | CanvasPaintDescriptor::Node(_)
            | CanvasPaintDescriptor::Custom(_) => hit_priority_discriminator(HitPriority::Node),
        },
    }
}

fn hit_priority_discriminator(priority: HitPriority) -> u16 {
    match priority {
        HitPriority::Background => 0,
        HitPriority::Frame => 1,
        HitPriority::Wire => 2,
        HitPriority::Node => 3,
        HitPriority::Port => 4,
        HitPriority::Overlay => 5,
    }
}

fn spatial_item_id_for_canvas_hit(hit: CanvasHit) -> SpatialItemId {
    spatial_item_id_for_canvas(hit.id, hit_priority_discriminator(hit.priority))
}

fn spatial_item_id_for_canvas(entity: CanvasEntityId, role_discriminator: u16) -> SpatialItemId {
    let mut value = 0xcbf29ce484222325u64;
    mix_u16(&mut value, role_discriminator);
    let identity = entity.stable_identity_parts();
    mix_u8(&mut value, identity.kind);
    for word in identity.words[..identity.word_count].iter().copied() {
        mix_u64(&mut value, word);
    }
    SpatialItemId(value)
}

fn mix_u8(hash: &mut u64, value: u8) {
    *hash ^= value as u64;
    *hash = hash.wrapping_mul(0x100000001b3);
}

fn mix_u16(hash: &mut u64, value: u16) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        mix_u8(hash, byte);
    }
}

fn spatial_hit_candidate_beats(
    candidate: (SpatialHitPriority, i32, usize, SpatialItemId),
    current: (SpatialHitPriority, i32, usize, SpatialItemId),
) -> bool {
    candidate.0 > current.0
        || (candidate.0 == current.0
            && (candidate.1 > current.1 || (candidate.1 == current.1 && candidate.2 > current.2)))
}

fn spatial_route_geometry(
    points: &[Point],
    axes: [SpatialRouteSegmentAxis; 3],
    head_anchor: Point,
    head_dir: RouteDirection,
) -> SpatialRouteGeometry {
    let empty = SpatialRouteSegment {
        from: Point::zero(),
        to: Point::zero(),
        axis: SpatialRouteSegmentAxis::Freeform,
    };
    let mut segments = [empty; 3];
    let segment_count = points.len().saturating_sub(1).min(segments.len());
    for index in 0..segment_count {
        segments[index] = SpatialRouteSegment {
            from: points[index],
            to: points[index + 1],
            axis: axes[index],
        };
    }

    SpatialRouteGeometry {
        segments,
        segment_count,
        head: SpatialRouteHead {
            anchor: head_anchor,
            dir: head_dir,
        },
    }
}

fn spatial_route_segment_axis(from: Point, to: Point) -> SpatialRouteSegmentAxis {
    if (from.y - to.y).abs() <= f32::EPSILON {
        SpatialRouteSegmentAxis::Horizontal
    } else if (from.x - to.x).abs() <= f32::EPSILON {
        SpatialRouteSegmentAxis::Vertical
    } else {
        SpatialRouteSegmentAxis::Freeform
    }
}

fn stable_spatial_grid_line(value: f32) -> f32 {
    value.round()
}

fn spatial_path_hit_test(path: &Path, point: WorldPoint, tolerance: f64) -> bool {
    let points = path.flatten();
    if points.len() < 2 {
        return false;
    }

    points.windows(2).any(|segment| {
        let a = WorldPoint::new(segment[0][0] as f64, segment[0][1] as f64);
        let b = WorldPoint::new(segment[1][0] as f64, segment[1][1] as f64);
        spatial_point_to_segment_distance(point, a, b) <= tolerance
    })
}

fn spatial_point_to_segment_distance(point: WorldPoint, a: WorldPoint, b: WorldPoint) -> f64 {
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
