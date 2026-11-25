// SPEC-HAL-336 §5: TimelineCanvas — Row-Bound Interval Projection Widget
//
// Thin projection over TemporalSubstrate. Paints intervals, bands, arrows,
// and markers from the substrate's per-frame snapshot, lane resolution, and
// geometry resolution.
//
// INV-336-1:  Mapping, spans, gridlines, and the now marker come from the
//             substrate (resolution + engine). The canvas computes none of it.
// INV-336-R1: push_rect, push_path (filled), and the text engine only.
//             Hollow style renders as four 1px strips — no border-rect or
//             rounded primitives.
// SPEC-PM-009-R3: All paint geometry lives in the pure functions below;
//             hit-testing (interaction task) calls the same functions.
//
// Interaction is implemented per TemporalCanvasPolicy (SPEC-HAL-336 §2):
// Display navigates the viewport only; Select adds selection and activation
// intents; Edit adds drag-move and drag-resize intents.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::ordered_selection::OrderedSelectionAction;
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
pub use crate::engines::temporal::RowGeometry;
use crate::engines::temporal::{
    IntervalId, IntervalLayer, IntervalStyle, LaneLayoutEngine, MarkerStyle, TemporalFrameSnapshot,
    TemporalIntent, TemporalSubstrate, TimeGeometryEngine,
};
use crate::model::spatial::{
    SpatialAdapter, SpatialGestureClass, SpatialGestureFacts, SpatialInteractionPolicy,
    SpatialProjection, SpatialProjectionError, SpatialRouteSegment, SpatialRouteSegmentAxis,
    SpatialVisualPolicy, SpatialWheelZoomPolicy, realize_spatial_route_plan,
};
use crate::model::time::{TimeRange, Timestamp};
use crate::renderer::path::Path;
use crate::utils::id::WidgetInternalId;
use crate::widgets::timeline_spatial::{
    TimelineProjectionSession, TimelineSpatialAdapter, TimelineSpatialSnapshot,
    timeline_arrow_route_plan,
};

// Deterministic rendering constants (SPEC-HAL-336 §5).
pub(crate) const BAR_MARGIN: f32 = 2.0;
pub(crate) const LANE_GAP: f32 = 1.0;
const PROGRESS_DARKEN: f32 = 0.25;
const BASELINE_ALPHA: f32 = 0.30;
const PREVIEW_ALPHA: f32 = 0.50;
const NON_WORKING_ALPHA: f32 = 0.50;
pub(crate) const DASH_LEN: f32 = 4.0;
pub(crate) const DASH_GAP: f32 = 3.0;
const OUTLINE: f32 = 1.0;
const HATCH_PITCH: f32 = 4.0;
pub(crate) const ARROW_STUB: f32 = 8.0;
pub(crate) const ARROW_HEAD: f32 = 5.0;
pub(crate) const MARKER_GLYPH: f32 = 8.0;
pub(crate) const RESIZE_GRIP: f32 = 4.0;

/// SPEC-HAL-336 §2: interaction policy. Display renders and navigates the
/// viewport only; Select adds selection and activation intents; Edit adds
/// drag-move and drag-resize intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemporalCanvasPolicy {
    #[default]
    Display,
    Select,
    Edit,
}

// =============================================================================
// Pure geometry (SPEC-PM-009-R3: paint and hit-test share these verbatim)
// =============================================================================

/// INV-336-R2: interval rect from resolved horizontal extent, row geometry,
/// and lane resolution. Non-milestone intervals paint at minimum 1px width.
pub(crate) fn interval_paint_rect(
    x0: f32,
    x1: f32,
    row_y: f32,
    row_h: f32,
    lane: u8,
    lane_count: u8,
    margin: f32,
    lane_gap: f32,
) -> RectF {
    let n = lane_count.max(1) as f32;
    let lane_h = (row_h - 2.0 * margin) / n;
    let y = row_y + margin + lane as f32 * lane_h;
    let h = (lane_h - lane_gap).max(1.0);
    let w = (x1 - x0).max(1.0);
    RectF::new(x0, y, w, h)
}

/// INV-336-R6: dash spans along an edge of the given length.
/// Returns (offset, len) pairs; the final dash clips to the edge.
pub(crate) fn dash_spans(len: f32, dash: f32, gap: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut at = 0.0;
    while at < len {
        out.push((at, dash.min(len - at)));
        at += dash + gap;
    }
    out
}

/// INV-336-R4: deterministic label placement rule.
pub(crate) fn label_fits_inside(run_width: f32, bar_width: f32, pad: f32) -> bool {
    run_width <= bar_width - 2.0 * pad
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HitZone {
    Body,
    ResizeEnd,
}

/// INV-336-H1 / SPEC-PM-009-R3: identity hit-testing over the Primary layer,
/// topmost-first, walking the same interval_paint_rect paint uses. Operates
/// in widget-local space (x_of is viewport-local; rows are canvas-local).
pub(crate) fn hit_test_primary(
    snapshot: &TemporalFrameSnapshot,
    lanes: &LaneLayoutEngine,
    geometry: &TimeGeometryEngine,
    row_rect: impl Fn(u64) -> Option<(f32, f32)>,
    pos: (f32, f32),
    edit_grip: bool,
) -> (Option<(IntervalId, HitZone)>, Timestamp) {
    let ts = geometry.instant_at(pos.0);
    let lo = snapshot
        .intervals
        .partition_point(|d| d.layer < IntervalLayer::Primary);
    let hi = snapshot
        .intervals
        .partition_point(|d| d.layer <= IntervalLayer::Primary);

    for i in (lo..hi).rev() {
        let d = &snapshot.intervals[i];
        let Some((row_y, row_h)) = row_rect(d.row) else {
            continue;
        };
        let x0 = geometry.x_of(d.range.start);
        let x1 = geometry.x_of(d.range.end);
        let bar = interval_paint_rect(
            x0,
            x1,
            row_y,
            row_h,
            lanes.lane_of(i),
            lanes.lane_count(d.row),
            BAR_MARGIN,
            LANE_GAP,
        );

        let hit = if d.style == IntervalStyle::Milestone {
            let half = bar.height / 2.0;
            let cy = bar.y + half;
            (pos.0 - x0).abs() <= half && (pos.1 - cy).abs() <= half
        } else {
            pos.0 >= bar.x
                && pos.0 < bar.x + bar.width
                && pos.1 >= bar.y
                && pos.1 < bar.y + bar.height
        };

        if hit {
            let zone = if edit_grip
                && d.style != IntervalStyle::Milestone
                && pos.0 >= bar.x + bar.width - RESIZE_GRIP
            {
                HitZone::ResizeEnd
            } else {
                HitZone::Body
            };
            return (Some((d.id, zone)), ts);
        }
    }
    (None, ts)
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    id: IntervalId,
    zone: HitZone,
    grab: Timestamp,
    orig: TimeRange,
    active: bool,
}

// =============================================================================
// Widget
// =============================================================================

pub struct TimelineCanvas {
    id: WidgetInternalId,
    substrate: Rc<RefCell<TemporalSubstrate>>,
    policy: TemporalCanvasPolicy,
    rows: Vec<RowGeometry>,
    drag: Option<DragState>,
    /// Composition-fed Space-held state (SPEC-HAL-336 §7 hand tool).
    pan_modifier: bool,
    /// Active hand-tool drag: last pointer position in widget-local space.
    pan: Option<(f32, f32)>,
    /// Cached layout size: handle_event needs the canvas height for the
    /// free-mode implicit row (events carry local_pos, not bounds).
    last_size: Size,
}

impl TimelineCanvas {
    pub fn new(substrate: Rc<RefCell<TemporalSubstrate>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            substrate,
            policy: TemporalCanvasPolicy::Display,
            rows: Vec::new(),
            drag: None,
            pan_modifier: false,
            pan: None,
            last_size: Size::new(0.0, 0.0),
        }
    }

    pub fn with_policy(mut self, policy: TemporalCanvasPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn policy(&self) -> TemporalCanvasPolicy {
        self.policy
    }

    /// Frame-resolved row geometry from the row authority. Empty = free
    /// mode: one implicit row spanning the canvas; descriptor row ids are
    /// ignored (SPEC-HAL-335 §7).
    pub fn set_row_geometry(&mut self, rows: Vec<RowGeometry>) {
        self.rows = rows;
    }

    /// Composition-fed Space-held state (SPEC-HAL-336 §7 hand tool). The
    /// modifier transport crosses the same boundary as row geometry; the
    /// canvas owns the gesture itself.
    pub fn set_pan_modifier(&mut self, held: bool) {
        self.pan_modifier = held;
        if !held {
            self.pan = None;
        }
    }

    /// Returns the current Timeline snapshot for read-only spatial projection.
    pub fn spatial_snapshot(&self, rect: RectF) -> TimelineSpatialSnapshot {
        let substrate = self.substrate.borrow();
        TimelineSpatialSnapshot::with_derived_revision(
            rect,
            substrate.resolution().clone(),
            substrate.snapshot().clone(),
            substrate.geometry().clone(),
            self.rows.clone(),
            substrate.lanes().lanes().to_vec(),
        )
    }

    /// Builds a read-only spatial projection over current Timeline geometry.
    pub fn spatial_projection(
        &self,
        rect: RectF,
    ) -> Result<SpatialProjection, SpatialProjectionError> {
        self.spatial_projection_with_policy(rect, SpatialVisualPolicy::default())
    }

    /// Builds a read-only spatial projection over current Timeline geometry.
    pub fn spatial_projection_with_policy(
        &self,
        rect: RectF,
        visual_policy: SpatialVisualPolicy,
    ) -> Result<SpatialProjection, SpatialProjectionError> {
        let snapshot = self.spatial_snapshot(rect);
        TimelineSpatialAdapter.build_projection(
            &snapshot,
            &TimelineProjectionSession,
            None,
            None,
            &visual_policy,
        )
    }

    /// Shared domain-neutral interaction policy facts for Timeline gestures.
    pub fn spatial_interaction_policy(&self) -> SpatialInteractionPolicy {
        SpatialInteractionPolicy {
            pan_gesture: SpatialGestureClass::SpacePrimaryDrag,
            alternate_pan_gesture: SpatialGestureClass::Disabled,
            wheel_zoom: SpatialWheelZoomPolicy::Unmodified,
            rubber_band_gesture: SpatialGestureClass::Disabled,
            fit_key: None,
            reset_key: None,
            delete_keys_enabled: false,
            route_creation_gesture: SpatialGestureClass::Disabled,
            ..SpatialInteractionPolicy::default()
        }
    }

    /// Four 1px strips forming a rectangle outline (INV-336-R1: no
    /// border-rect primitive).
    fn stroke_rect_strips(cx: &mut PaintContext, r: RectF, t: f32, color: Color) {
        cx.scene
            .push_rect(RectF::new(r.x, r.y, r.width, t), color, LAYER_CONTENT, None);
        cx.scene.push_rect(
            RectF::new(r.x, r.y + r.height - t, r.width, t),
            color,
            LAYER_CONTENT,
            None,
        );
        cx.scene.push_rect(
            RectF::new(r.x, r.y, t, r.height),
            color,
            LAYER_CONTENT,
            None,
        );
        cx.scene.push_rect(
            RectF::new(r.x + r.width - t, r.y, t, r.height),
            color,
            LAYER_CONTENT,
            None,
        );
    }

    fn dashed_rect_outline(cx: &mut PaintContext, r: RectF, color: Color) {
        for (off, len) in dash_spans(r.width, DASH_LEN, DASH_GAP) {
            cx.scene.push_rect(
                RectF::new(r.x + off, r.y, len, OUTLINE),
                color,
                LAYER_CONTENT,
                None,
            );
            cx.scene.push_rect(
                RectF::new(r.x + off, r.y + r.height - OUTLINE, len, OUTLINE),
                color,
                LAYER_CONTENT,
                None,
            );
        }
        for (off, len) in dash_spans(r.height, DASH_LEN, DASH_GAP) {
            cx.scene.push_rect(
                RectF::new(r.x, r.y + off, OUTLINE, len),
                color,
                LAYER_CONTENT,
                None,
            );
            cx.scene.push_rect(
                RectF::new(r.x + r.width - OUTLINE, r.y + off, OUTLINE, len),
                color,
                LAYER_CONTENT,
                None,
            );
        }
    }

    /// INV-336-R7: 45° hatch as filled parallelograms, clipped to the bar.
    fn hatch_rect(cx: &mut PaintContext, r: RectF, color: Color) {
        cx.scene.begin_clip(r, None);
        let mut x = r.x - r.height;
        while x < r.x + r.width {
            let mut p = Path::new();
            p.move_to(x, r.y + r.height);
            p.line_to(x + r.height, r.y);
            p.line_to(x + r.height + 1.0, r.y);
            p.line_to(x + 1.0, r.y + r.height);
            p.close();
            cx.scene.push_path(p, color, LAYER_CONTENT, None);
            x += HATCH_PITCH;
        }
        cx.scene.end_clip();
    }

    /// INV-336-R5: filled diamond, side = lane content height, centered on x.
    fn diamond(cx: &mut PaintContext, center_x: f32, center_y: f32, half: f32, color: Color) {
        let mut p = Path::new();
        p.move_to(center_x, center_y - half);
        p.line_to(center_x + half, center_y);
        p.line_to(center_x, center_y + half);
        p.line_to(center_x - half, center_y);
        p.close();
        cx.scene.push_path(p, color, LAYER_CONTENT, None);
    }

    fn h_seg(cx: &mut PaintContext, x0: f32, x1: f32, y: f32, t: f32, color: Color) {
        let (a, b) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        cx.scene.push_rect(
            RectF::new(a, y - t / 2.0, (b - a).max(t), t),
            color,
            LAYER_CONTENT,
            None,
        );
    }

    fn v_seg(cx: &mut PaintContext, x: f32, y0: f32, y1: f32, t: f32, color: Color) {
        let (a, b) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        cx.scene.push_rect(
            RectF::new(x - t / 2.0, a, t, (b - a).max(t)),
            color,
            LAYER_CONTENT,
            None,
        );
    }

    fn route_segment(cx: &mut PaintContext, segment: SpatialRouteSegment, t: f32, color: Color) {
        match segment.axis {
            SpatialRouteSegmentAxis::Horizontal => {
                Self::h_seg(cx, segment.from.x, segment.to.x, segment.from.y, t, color);
            }
            SpatialRouteSegmentAxis::Vertical => {
                Self::v_seg(cx, segment.from.x, segment.from.y, segment.to.y, t, color);
            }
            SpatialRouteSegmentAxis::Freeform => {}
        }
    }

    /// Filled triangle arrowhead at (x, y), pointing right (+1) or left (−1).
    fn arrow_head(cx: &mut PaintContext, x: f32, y: f32, dir: f32, color: Color) {
        let mut p = Path::new();
        p.move_to(x, y);
        p.line_to(x - dir * ARROW_HEAD, y - ARROW_HEAD * 0.6);
        p.line_to(x - dir * ARROW_HEAD, y + ARROW_HEAD * 0.6);
        p.close();
        cx.scene.push_path(p, color, LAYER_CONTENT, None);
    }
}

impl WidgetDowncast for TimelineCanvas {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TimelineCanvas {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            constraints.min_width
        };
        let height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            constraints.min_height
        };
        self.last_size = Size::new(width, height);
        // INV-335-G1: the canvas is the engine's single viewport-width
        // writer. Culling and resolution must track the laid-out width;
        // layout-time mutation of a geometry input mirrors column-geometry
        // resolution. The header never writes geometry (FORBID-336-4).
        self.substrate
            .borrow_mut()
            .geometry_mut()
            .set_viewport_width(width);
        LayoutResult::fit(self.last_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let substrate = self.substrate.borrow();
        let resolution = substrate.resolution();
        let snapshot = substrate.snapshot();
        let lanes = substrate.lanes();
        let geometry = substrate.geometry();
        let spatial_snapshot = TimelineSpatialSnapshot::with_derived_revision(
            rect,
            resolution.clone(),
            snapshot.clone(),
            geometry.clone(),
            self.rows.clone(),
            lanes.lanes().to_vec(),
        );

        let pad = cx.theme.list_geom.padding_x;
        let font_size = cx.theme.typography.font_size;
        let bg = cx.theme.colors.surfaces.base0;
        let non_working = cx
            .theme
            .colors
            .surfaces
            .content
            .with_alpha(NON_WORKING_ALPHA);
        let grid = cx.theme.colors.borders.subtle;
        let label_inside = cx.theme.colors.typography.inverse;
        let label_outside = cx.theme.colors.typography.primary;
        let label_secondary = cx.theme.colors.typography.secondary;
        let arrow_default = cx.theme.colors.borders.strong;
        let arrow_critical = cx.theme.colors.semantic.error;
        let now_color = cx.theme.colors.accents.primary;

        // Row map: id → (absolute y, height). Free mode: one implicit row
        // spanning the canvas; every descriptor row id maps to it.
        let free_mode = self.rows.is_empty();
        let mut row_map: HashMap<u64, (f32, f32)> = HashMap::new();
        if !free_mode {
            for r in &self.rows {
                row_map.insert(r.id, (rect.y + r.y, r.height));
            }
        }
        let row_rect = |row: u64| -> Option<(f32, f32)> {
            if free_mode {
                Some((rect.y, rect.height))
            } else {
                row_map.get(&row).copied()
            }
        };

        // SPEC-PM-007 INV-PM-007-3: the background fill is z-index 0, strictly
        // below LAYER_CONTENT. It is pushed outside the clip opened below, so at
        // the same z_layer it would order against the clipped content by clip_hash
        // (FORBID-PM-007-1/2) and could overpaint it. z=0 forces it beneath all
        // content via the primary z_layer sort, at every window size.
        cx.scene.push_rect(rect, bg, 0, None);
        cx.scene.begin_clip(rect, None);

        // 2. Non-working shading.
        for &(x0, x1) in &resolution.non_working {
            cx.scene.push_rect(
                RectF::new(rect.x + x0, rect.y, x1 - x0, rect.height),
                non_working,
                LAYER_CONTENT,
                None,
            );
        }

        // 3. Row bands.
        for band in &snapshot.bands {
            if let Some((ry, rh)) = row_rect(band.row) {
                let x0 = rect.x + geometry.x_of(band.range.start);
                let x1 = rect.x + geometry.x_of(band.range.end);
                cx.scene.push_rect(
                    RectF::new(x0, ry, (x1 - x0).max(1.0), rh),
                    band.color,
                    LAYER_CONTENT,
                    None,
                );
            }
        }

        // 4. Gridlines.
        for &gx in &resolution.gridlines {
            cx.scene.push_rect(
                RectF::new(rect.x + gx, rect.y, 1.0, rect.height),
                grid,
                LAYER_CONTENT,
                None,
            );
        }

        // 5/6/8. Intervals: Baseline → Primary → Preview (canonical order
        // already groups layers; explicit passes keep the paint order law).
        for pass in [
            IntervalLayer::Baseline,
            IntervalLayer::Primary,
            IntervalLayer::Preview,
        ] {
            for (i, d) in snapshot.intervals.iter().enumerate() {
                if d.layer != pass {
                    continue;
                }
                let Some((row_y, row_h)) = row_rect(d.row) else {
                    continue;
                };
                let lane = lanes.lane_of(i);
                let n = lanes.lane_count(d.row);
                let x0 = rect.x + geometry.x_of(d.range.start);
                let x1 = rect.x + geometry.x_of(d.range.end);
                let bar = interval_paint_rect(x0, x1, row_y, row_h, lane, n, BAR_MARGIN, LANE_GAP);

                match pass {
                    IntervalLayer::Baseline => {
                        cx.scene.push_rect(
                            bar,
                            d.color.with_alpha(BASELINE_ALPHA),
                            LAYER_CONTENT,
                            None,
                        );
                    }
                    IntervalLayer::Preview => {
                        cx.scene.push_rect(
                            bar,
                            d.color.with_alpha(PREVIEW_ALPHA),
                            LAYER_CONTENT,
                            None,
                        );
                        Self::dashed_rect_outline(cx, bar, d.color);
                    }
                    IntervalLayer::Primary => {
                        match d.style {
                            IntervalStyle::Milestone => {
                                let half = bar.height / 2.0;
                                Self::diamond(cx, x0, bar.y + half, half, d.color);
                                continue; // no progress, no label width rules
                            }
                            IntervalStyle::Solid => {
                                cx.scene.push_rect(bar, d.color, LAYER_CONTENT, None);
                            }
                            IntervalStyle::Hollow => {
                                Self::stroke_rect_strips(cx, bar, OUTLINE, d.color);
                            }
                            IntervalStyle::Hatched => {
                                Self::hatch_rect(cx, bar, d.color);
                            }
                        }

                        // INV-336-R3: progress fill.
                        if d.progress > 0.0 {
                            let pw = bar.width * d.progress.clamp(0.0, 1.0);
                            cx.scene.push_rect(
                                RectF::new(bar.x, bar.y, pw, bar.height),
                                d.color.darken(PROGRESS_DARKEN),
                                LAYER_CONTENT,
                                None,
                            );
                        }

                        // INV-336-R4: deterministic label rule.
                        if let Some(label) = &d.label {
                            let run = cx.text_engine.shape(label, font_size);
                            let ty = bar.y + (bar.height - run.height) / 2.0;
                            if label_fits_inside(run.width, bar.width, pad) {
                                cx.text_engine.draw(
                                    &run,
                                    bar.x + pad,
                                    ty,
                                    label_inside,
                                    LAYER_CONTENT + 1,
                                    cx.scene,
                                );
                            } else {
                                cx.text_engine.draw(
                                    &run,
                                    bar.x + bar.width + pad,
                                    ty,
                                    label_outside,
                                    LAYER_CONTENT + 1,
                                    cx.scene,
                                );
                            }
                        }
                    }
                }
            }
        }

        // 7. Arrows (painted after primaries; preview overlays them).
        for a in &snapshot.arrows {
            let Some(plan) = timeline_arrow_route_plan(&spatial_snapshot, a) else {
                continue;
            };

            let t = if a.critical { 2.0 } else { 1.0 };
            let color = if a.critical {
                arrow_critical
            } else {
                a.color.unwrap_or(arrow_default)
            };

            let route = realize_spatial_route_plan(plan);

            for segment in route.segments() {
                Self::route_segment(cx, *segment, t, color);
            }
            Self::arrow_head(
                cx,
                route.head.anchor.x,
                route.head.anchor.y,
                route.head.dir.sign(),
                color,
            );
        }

        // 9. Markers, then the now marker on top.
        for m in &snapshot.markers {
            let mx = rect.x + geometry.x_of(m.ts);
            match m.style {
                MarkerStyle::VerticalLine => {
                    cx.scene.push_rect(
                        RectF::new(mx, rect.y, 1.0, rect.height),
                        m.color,
                        LAYER_CONTENT,
                        None,
                    );
                }
                MarkerStyle::Diamond => {
                    cx.scene.push_rect(
                        RectF::new(mx, rect.y, 1.0, rect.height),
                        m.color,
                        LAYER_CONTENT,
                        None,
                    );
                    Self::diamond(
                        cx,
                        mx + 0.5,
                        rect.y + 1.0 + MARKER_GLYPH / 2.0,
                        MARKER_GLYPH / 2.0,
                        m.color,
                    );
                }
                MarkerStyle::Flag => {
                    cx.scene.push_rect(
                        RectF::new(mx, rect.y, 1.0, rect.height),
                        m.color,
                        LAYER_CONTENT,
                        None,
                    );
                    let mut p = Path::new();
                    p.move_to(mx + 1.0, rect.y + 1.0);
                    p.line_to(mx + 1.0 + MARKER_GLYPH, rect.y + 1.0 + MARKER_GLYPH * 0.4);
                    p.line_to(mx + 1.0, rect.y + 1.0 + MARKER_GLYPH * 0.8);
                    p.close();
                    cx.scene.push_path(p, m.color, LAYER_CONTENT, None);
                }
            }
            if let Some(label) = &m.label {
                let run = cx.text_engine.shape(label, font_size);
                cx.text_engine.draw(
                    &run,
                    mx + 3.0,
                    rect.y + 2.0,
                    label_secondary,
                    LAYER_CONTENT + 1,
                    cx.scene,
                );
            }
        }

        if let Some(nx) = resolution.now_x {
            cx.scene.push_rect(
                RectF::new(rect.x + nx, rect.y, 2.0, rect.height),
                now_color,
                LAYER_CONTENT,
                None,
            );
        }

        // 10. Selection/focus affordances. FORBID-336-1: never in Display.
        if self.policy != TemporalCanvasPolicy::Display {
            let selected_color = cx.theme.colors.accents.primary;
            let focus_color = cx.theme.colors.borders.focus;
            let selection = substrate.selection();
            let lo = snapshot
                .intervals
                .partition_point(|d| d.layer < IntervalLayer::Primary);
            let hi = snapshot
                .intervals
                .partition_point(|d| d.layer <= IntervalLayer::Primary);
            for i in lo..hi {
                let d = &snapshot.intervals[i];
                let is_selected = selection.is_selected(d.id);
                let is_focused = selection.is_focused(d.id);
                if !is_selected && !is_focused {
                    continue;
                }
                let Some((row_y, row_h)) = row_rect(d.row) else {
                    continue;
                };
                let bar = interval_paint_rect(
                    rect.x + geometry.x_of(d.range.start),
                    rect.x + geometry.x_of(d.range.end),
                    row_y,
                    row_h,
                    lanes.lane_of(i),
                    lanes.lane_count(d.row),
                    BAR_MARGIN,
                    LANE_GAP,
                );
                if is_selected {
                    Self::stroke_rect_strips(cx, bar, OUTLINE, selected_color);
                }
                if is_focused {
                    let inflated =
                        RectF::new(bar.x - 2.0, bar.y - 2.0, bar.width + 4.0, bar.height + 4.0);
                    Self::stroke_rect_strips(cx, inflated, OUTLINE, focus_color);
                }
            }
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target && phase != EventPhase::Bubble {
            return false;
        }

        // Free/bound row lookup in widget-local space (mirrors paint).
        let free_mode = self.rows.is_empty();

        match event {
            // ── Wheel: viewport navigation, all tiers (§7). ──
            Event::Pointer(pe) => {
                if let PointerEventKind::Scroll { delta_x, delta_y } = pe.kind {
                    let mut substrate = self.substrate.borrow_mut();
                    if pe.modifiers.shift {
                        // §7: Shift+wheel pans the time axis. Down or left
                        // advances forward (later); up or right moves back.
                        // scroll_by_px(+) advances the anchor (INV-335-G4).
                        let pan = delta_y - delta_x;
                        if pan == 0.0 {
                            return false;
                        }
                        substrate.geometry_mut().scroll_by_px(pan);
                    } else if delta_y != 0.0
                        && self
                            .spatial_interaction_policy()
                            .wheel_zoom_requested(pe.modifiers)
                    {
                        // §7: plain wheel zooms about the cursor — scroll up
                        // zooms in, scroll down zooms out (INV-335-G3 anchor
                        // law). Wheel-up reports a positive delta_y here.
                        let factor = if delta_y > 0.0 { 1.25 } else { 0.8 };
                        substrate.geometry_mut().zoom_at(pe.local_pos.0, factor);
                    } else {
                        // Pure-horizontal wheel, no modifier: nothing to do.
                        return false;
                    }
                    let range = substrate.geometry().visible_range();
                    substrate.push_intent(TemporalIntent::VisibleRangeChanged { range });
                    return true;
                }

                // Hit-test once for the pointer arms below.
                let hit = {
                    let substrate = self.substrate.borrow();
                    let rows = &self.rows;
                    let canvas_h = self.last_size.height;
                    let row_rect = |row: u64| -> Option<(f32, f32)> {
                        if free_mode {
                            // Free mode: one implicit row spanning the canvas
                            // (height cached from layout, SPEC-HAL-335 §7).
                            Some((0.0, canvas_h))
                        } else {
                            rows.iter().find(|r| r.id == row).map(|r| (r.y, r.height))
                        }
                    };
                    hit_test_primary(
                        substrate.snapshot(),
                        substrate.lanes(),
                        substrate.geometry(),
                        row_rect,
                        pe.local_pos,
                        self.policy == TemporalCanvasPolicy::Edit,
                    )
                };

                match pe.kind {
                    PointerEventKind::Move => {
                        // INV-336-I2: publish the cursor instant, all tiers.
                        let (_, ts) = hit;
                        let mut substrate = self.substrate.borrow_mut();
                        substrate.set_cursor(Some(ts));

                        // §7 hand tool: content follows the pointer, both axes.
                        if let Some(last) = self.pan {
                            let dx = pe.local_pos.0 - last.0;
                            let dy = pe.local_pos.1 - last.1;
                            self.pan = Some(pe.local_pos);
                            let mut range_changed = false;
                            if dx != 0.0 {
                                substrate.geometry_mut().scroll_by_px(-dx);
                                range_changed = true;
                            }
                            if !free_mode && dy != 0.0 {
                                substrate.push_intent(TemporalIntent::VerticalPanBy { dy: -dy });
                            }
                            if range_changed {
                                let range = substrate.geometry().visible_range();
                                substrate
                                    .push_intent(TemporalIntent::VisibleRangeChanged { range });
                            }
                            return true;
                        }

                        if let Some(drag) = self.drag {
                            if drag.active && self.policy == TemporalCanvasPolicy::Edit {
                                let cur = substrate.geometry().instant_at(pe.local_pos.0);
                                match drag.zone {
                                    HitZone::Body => {
                                        let shift = cur.0 - drag.grab.0;
                                        let new_range = TimeRange::new(
                                            Timestamp(drag.orig.start.0 + shift),
                                            Timestamp(drag.orig.end.0 + shift),
                                        );
                                        substrate.push_intent(TemporalIntent::DragMove {
                                            id: drag.id,
                                            new_range,
                                        });
                                    }
                                    HitZone::ResizeEnd => {
                                        let new_end = Timestamp(cur.0.max(drag.orig.start.0));
                                        substrate.push_intent(TemporalIntent::DragResize {
                                            id: drag.id,
                                            new_end,
                                        });
                                    }
                                }
                                return true;
                            }
                        }
                        false
                    }

                    PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                        let interaction_policy = self.spatial_interaction_policy();
                        let gesture = interaction_policy.gesture_class(SpatialGestureFacts {
                            modifiers: pe.modifiers,
                            button: PointerButton::Primary,
                            space_pressed: self.pan_modifier,
                        });
                        if interaction_policy.is_pan_gesture(gesture) {
                            // §7 hand tool (Display+): suppresses the selection
                            // and Edit-drag arms while active.
                            self.pan = Some(pe.local_pos);
                            cx.capture_pointer(PointerButton::Primary);
                            return true;
                        }
                        if self.policy == TemporalCanvasPolicy::Display {
                            return false;
                        }
                        let (target, _) = hit;
                        let mut substrate = self.substrate.borrow_mut();
                        match target {
                            Some((id, zone)) => {
                                // SPEC-HAL-135: commit on press.
                                let action = if pe.modifiers.ctrl || pe.modifiers.meta {
                                    OrderedSelectionAction::Toggle { key: id }
                                } else if pe.modifiers.shift {
                                    OrderedSelectionAction::ExtendTo { key: id }
                                } else {
                                    OrderedSelectionAction::Select { key: id }
                                };
                                substrate.reduce_selection(action);
                                substrate.push_intent(TemporalIntent::Select {
                                    id,
                                    modifiers: pe.modifiers,
                                });
                                if self.policy == TemporalCanvasPolicy::Edit {
                                    let orig = substrate.snapshot().interval(id).map(|d| d.range);
                                    if let Some(orig) = orig {
                                        let grab = substrate.geometry().instant_at(pe.local_pos.0);
                                        self.drag = Some(DragState {
                                            id,
                                            zone,
                                            grab,
                                            orig,
                                            active: false,
                                        });
                                        cx.capture_pointer(PointerButton::Primary);
                                    }
                                }
                                true
                            }
                            None => {
                                // §7 empty-space rule per SPEC-HAL-135.
                                substrate.reduce_selection(OrderedSelectionAction::ClearSelection);
                                substrate.push_intent(TemporalIntent::DeselectAll);
                                true
                            }
                        }
                    }

                    PointerEventKind::Down if pe.button == Some(PointerButton::Secondary) => {
                        if self.policy == TemporalCanvasPolicy::Display {
                            return false;
                        }
                        let (target, ts) = hit;
                        let mut substrate = self.substrate.borrow_mut();
                        substrate.push_intent(TemporalIntent::ContextMenu {
                            id: target.map(|(id, _)| id),
                            ts,
                        });
                        true
                    }

                    PointerEventKind::DoubleClick if pe.button == Some(PointerButton::Primary) => {
                        if self.policy == TemporalCanvasPolicy::Display {
                            return false;
                        }
                        if let (Some((id, _)), _) = hit {
                            let mut substrate = self.substrate.borrow_mut();
                            substrate.push_intent(TemporalIntent::Activate { id });
                            return true;
                        }
                        false
                    }

                    PointerEventKind::DragStart => {
                        // INV-LV-RESIZE-ACTIVATION: activation on DragStart only.
                        if self.policy == TemporalCanvasPolicy::Edit {
                            if let Some(drag) = &mut self.drag {
                                drag.active = true;
                                return true;
                            }
                        }
                        false
                    }

                    PointerEventKind::Up => {
                        if self.pan.take().is_some() {
                            return true;
                        }
                        if self.drag.take().is_some() {
                            return true;
                        }
                        false
                    }

                    _ => false,
                }
            }

            // ── Keyboard (§7): Enter activates, Space toggles focused. ──
            Event::Key(ke) if ke.is_down => {
                if self.policy == TemporalCanvasPolicy::Display {
                    return false;
                }
                match ke.key_code {
                    KeyCode::Enter => {
                        let focused = self.substrate.borrow().selection().focused_key();
                        if let Some(id) = focused {
                            self.substrate
                                .borrow_mut()
                                .push_intent(TemporalIntent::Activate { id });
                            return true;
                        }
                        false
                    }
                    // Space is the §7 pan modifier — unbound from ToggleFocused
                    // (JC ruling 2026-06-12); toggling remains via Cmd/Ctrl+click.
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "TimelineCanvas"
    }

    fn wants_scroll(&self) -> bool {
        // SPEC-HAL-336 §7: the canvas owns wheel navigation (zoom about the
        // cursor, two-axis pan). The router's scroll pipeline dispatches
        // wheel only to the nearest wants_scroll() ancestor and drops the
        // event otherwise — without this override the §7 wheel arm is
        // unreachable. Plain vertical in bound mode is consumed as a
        // deferred VerticalPanBy; in free mode it returns false and bubbles.
        true
    }
}
