use crate::core::layout::{Constraints, EdgeInsets, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

const EPSILON: f32 = 0.01;
const SYNTHETIC_BAND_HEIGHT: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionFlowDebug {
    Off,
    PaintExclusions,
    PaintBands,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FreeSegment {
    pub x: f32,
    pub width: f32,
}

impl FreeSegment {
    fn right(&self) -> f32 {
        self.x + self.width
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FreeBand {
    pub y: f32,
    pub height: f32,
    pub segments: Vec<FreeSegment>,
}

impl FreeBand {
    fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpaceMap {
    content_rect: RectF,
    exclusions: Vec<RectF>,
    bands: Vec<FreeBand>,
}

impl SpaceMap {
    pub(crate) fn new(content_rect: RectF, exclusions: &[RectF]) -> Self {
        if !valid_positive_rect(content_rect) {
            return Self {
                content_rect,
                exclusions: Vec::new(),
                bands: Vec::new(),
            };
        }

        let mut clipped = Vec::new();
        for rect in exclusions {
            if !valid_positive_rect(*rect) {
                continue;
            }
            if let Some(clipped_rect) = rect.intersection(&content_rect) {
                if valid_positive_rect(clipped_rect) {
                    clipped.push(clipped_rect);
                }
            }
        }

        let mut edges = vec![content_rect.top(), content_rect.bottom()];
        for rect in &clipped {
            edges.push(rect.top());
            edges.push(rect.bottom());
        }
        edges.sort_by(|a, b| a.total_cmp(b));
        dedupe_f32(&mut edges);

        let mut bands = Vec::new();
        for pair in edges.windows(2) {
            let top = pair[0];
            let bottom = pair[1];
            let height = bottom - top;
            if height <= EPSILON {
                continue;
            }

            let mut segments = vec![FreeSegment {
                x: content_rect.left(),
                width: content_rect.width,
            }];

            for rect in &clipped {
                if rect.bottom() <= top + EPSILON || rect.top() >= bottom - EPSILON {
                    continue;
                }
                subtract_rect_from_segments(&mut segments, *rect);
            }

            bands.push(FreeBand {
                y: top,
                height,
                segments,
            });
        }

        Self {
            content_rect,
            exclusions: clipped,
            bands,
        }
    }

    pub(crate) fn bands(&self) -> &[FreeBand] {
        &self.bands
    }

    pub(crate) fn exclusions(&self) -> &[RectF] {
        &self.exclusions
    }

    fn content_rect(&self) -> RectF {
        self.content_rect
    }

    fn rect_intersects_exclusion(&self, rect: RectF) -> bool {
        self.exclusions
            .iter()
            .any(|exclusion| rect.intersects(exclusion))
    }
}

pub struct ExclusionFlow {
    id: WidgetInternalId,
    padding: EdgeInsets,
    spacing: f32,
    exclusions: Vec<RectF>,
    debug: ExclusionFlowDebug,
    last_space_map: Option<SpaceMap>,
}

impl ExclusionFlow {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            padding: EdgeInsets::zero(),
            spacing: 0.0,
            exclusions: Vec::new(),
            debug: ExclusionFlowDebug::Off,
            last_space_map: None,
        }
    }

    pub fn with_padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    pub fn with_exclusion(mut self, rect: RectF) -> Self {
        self.exclusions.push(rect);
        self
    }

    pub fn with_exclusions(mut self, rects: impl IntoIterator<Item = RectF>) -> Self {
        self.exclusions.extend(rects);
        self
    }

    pub fn with_debug(mut self, debug: ExclusionFlowDebug) -> Self {
        self.debug = debug;
        self
    }
}

impl Default for ExclusionFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for ExclusionFlow {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ExclusionFlow {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let measured = measure_children(cx, constraints, self.padding, children);
        let content_width = resolve_content_width(constraints, self.padding, &measured);
        if content_width <= EPSILON {
            self.last_space_map = Some(SpaceMap::new(RectF::zero(), &[]));
            return LayoutResult::zero();
        }

        let provisional_height = resolve_provisional_height(
            constraints,
            self.padding,
            &measured,
            &self.exclusions,
            self.spacing,
        );
        let content_rect = RectF::new(
            self.padding.left,
            self.padding.top,
            content_width,
            provisional_height.max(EPSILON),
        );
        let space_map = SpaceMap::new(content_rect, &self.exclusions);

        let mut max_child_bottom = self.padding.top;
        let mut cursor = FlowCursor::new(content_rect.x, content_rect.y);
        for item in &measured {
            if item.preferred_size.width <= EPSILON || item.preferred_size.height <= EPSILON {
                cx.set_child_rect(item.node, RectF::zero());
                continue;
            }

            let placed = place_child(&space_map, item.preferred_size, self.spacing, cursor)
                .unwrap_or_else(|| {
                    place_below_exclusions(&space_map, item.preferred_size, cursor.y)
                });

            cx.set_child_rect(item.node, placed);

            let relayout_constraints =
                Constraints::new(placed.width, placed.width, 0.0, f32::INFINITY);
            cx.relayout_child_with_constraints(item.node, relayout_constraints);

            let mut final_rect = cx.tree.get_rect(item.node).unwrap_or(placed);
            final_rect.x = placed.x;
            final_rect.y = placed.y;
            final_rect.width = placed.width;

            if space_map.rect_intersects_exclusion(final_rect) {
                final_rect = place_child(
                    &space_map,
                    Size::new(final_rect.width, final_rect.height),
                    self.spacing,
                    cursor,
                )
                .unwrap_or_else(|| {
                    place_below_exclusions(
                        &space_map,
                        Size::new(final_rect.width, final_rect.height),
                        cursor.y,
                    )
                });
            }

            cx.set_child_rect(item.node, final_rect);
            max_child_bottom = max_child_bottom.max(final_rect.bottom() + self.spacing);
            cursor.advance(final_rect, self.spacing);
        }

        let natural_height = if measured.is_empty() {
            self.padding.vertical()
        } else {
            (max_child_bottom - self.spacing + self.padding.bottom).max(self.padding.vertical())
        };
        let natural_size = Size::new(content_width + self.padding.horizontal(), natural_height);
        let render_size = constraints.constrain(natural_size);

        self.last_space_map = Some(space_map);
        LayoutResult::new(render_size, natural_size)
    }

    fn paint(&self, cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        let Some(space_map) = &self.last_space_map else {
            return;
        };

        match self.debug {
            ExclusionFlowDebug::Off => {}
            ExclusionFlowDebug::PaintExclusions => {
                let color = cx.theme.colors.accents.primary.with_alpha(0.18);
                for exclusion in space_map.exclusions() {
                    cx.scene.push_rect(*exclusion, color, 0, cx.scene_key);
                }
            }
            ExclusionFlowDebug::PaintBands => {
                let color = Color::from_linear(0.2, 0.8, 0.4, 0.12);
                for band in space_map.bands() {
                    for segment in &band.segments {
                        cx.scene.push_rect(
                            RectF::new(segment.x, band.y, segment.width, band.height),
                            color,
                            0,
                            cx.scene_key,
                        );
                    }
                }
            }
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "ExclusionFlow"
    }
}

#[derive(Debug, Clone, Copy)]
struct MeasuredChild {
    node: NodeId,
    preferred_size: Size,
}

fn measure_children(
    cx: &mut LayoutContext,
    constraints: Constraints,
    padding: EdgeInsets,
    children: &[NodeId],
) -> Vec<MeasuredChild> {
    let max_width = if constraints.max_width.is_finite() {
        (constraints.max_width - padding.horizontal()).max(0.0)
    } else {
        f32::INFINITY
    };

    let measure_constraints = Constraints::new(0.0, max_width, 0.0, f32::INFINITY);
    let mut measured = Vec::with_capacity(children.len());

    for &child_id in children {
        let size = cx.layout_child(child_id, measure_constraints);
        let flex = cx.tree.get_flex(child_id);
        let width = if flex.basis > 0.0 {
            flex.basis
        } else {
            size.width
        };

        measured.push(MeasuredChild {
            node: child_id,
            preferred_size: Size::new(sanitize_dimension(width), sanitize_dimension(size.height)),
        });
    }

    measured
}

fn resolve_content_width(
    constraints: Constraints,
    padding: EdgeInsets,
    measured: &[MeasuredChild],
) -> f32 {
    if constraints.max_width.is_finite() {
        return (constraints.max_width - padding.horizontal()).max(0.0);
    }

    measured
        .iter()
        .map(|item| item.preferred_size.width)
        .fold(0.0, f32::max)
}

fn resolve_provisional_height(
    constraints: Constraints,
    padding: EdgeInsets,
    measured: &[MeasuredChild],
    exclusions: &[RectF],
    spacing: f32,
) -> f32 {
    let measured_height: f32 = measured.iter().map(|item| item.preferred_size.height).sum();
    let total_spacing = measured.len().saturating_sub(1) as f32 * spacing;
    let natural_stack = measured_height + total_spacing;
    let exclusion_extent = exclusions
        .iter()
        .filter(|rect| rect.height > EPSILON && rect.bottom().is_finite())
        .map(RectF::bottom)
        .fold(padding.top, f32::max)
        - padding.top;

    if constraints.max_height.is_finite() {
        return (constraints.max_height - padding.vertical())
            .max(natural_stack)
            .max(exclusion_extent)
            .max(0.0)
            + SYNTHETIC_BAND_HEIGHT;
    }

    natural_stack.max(exclusion_extent) + SYNTHETIC_BAND_HEIGHT
}

fn place_child(
    space_map: &SpaceMap,
    preferred_size: Size,
    spacing: f32,
    cursor: FlowCursor,
) -> Option<RectF> {
    let must_clamp = widest_segment(space_map)
        .map(|segment| segment.width + EPSILON < preferred_size.width)
        .unwrap_or(false);

    if let Some(rect) = find_in_row(
        space_map,
        preferred_size,
        cursor.y,
        cursor.x,
        spacing,
        must_clamp,
    ) {
        return Some(rect);
    }

    find_from_y(
        space_map,
        preferred_size,
        cursor.row_bottom + spacing,
        spacing,
        must_clamp,
    )
}

fn find_from_y(
    space_map: &SpaceMap,
    preferred_size: Size,
    min_y: f32,
    spacing: f32,
    must_clamp: bool,
) -> Option<RectF> {
    for band in space_map.bands() {
        if band.bottom() <= min_y + EPSILON {
            continue;
        }

        let y = band.y.max(min_y);
        if let Some(rect) = find_in_row(
            space_map,
            preferred_size,
            y,
            space_map.content_rect().x,
            spacing,
            must_clamp,
        ) {
            return Some(rect);
        }
    }

    None
}

fn find_in_row(
    space_map: &SpaceMap,
    preferred_size: Size,
    y: f32,
    min_x: f32,
    spacing: f32,
    must_clamp: bool,
) -> Option<RectF> {
    let band = space_map
        .bands()
        .iter()
        .find(|band| band.y <= y + EPSILON && band.bottom() > y + EPSILON)?;

    let preferred_width = preferred_size.width;
    for segment in &band.segments {
        let width = if must_clamp {
            preferred_width.min(segment.width)
        } else {
            if segment.width + EPSILON < preferred_width {
                continue;
            }
            preferred_width
        };
        if width <= EPSILON {
            continue;
        }

        let mut x = segment.x.max(min_x);
        while x + width <= segment.right() + EPSILON {
            let candidate = RectF::new(x, y, width, preferred_size.height);
            if !space_map.rect_intersects_exclusion(candidate) {
                return Some(candidate);
            }
            x += spacing.max(1.0);
        }
    }

    None
}

fn place_below_exclusions(space_map: &SpaceMap, preferred_size: Size, min_y: f32) -> RectF {
    let content = space_map.content_rect();
    let y = min_y
        .max(
            space_map
                .exclusions()
                .iter()
                .map(RectF::bottom)
                .fold(content.y, f32::max),
        )
        .max(content.y);
    let width = preferred_size.width.min(content.width).max(0.0);
    RectF::new(content.x, y, width, preferred_size.height)
}

fn widest_segment(space_map: &SpaceMap) -> Option<FreeSegment> {
    space_map
        .bands()
        .iter()
        .flat_map(|band| band.segments.iter().copied())
        .max_by(|a, b| a.width.total_cmp(&b.width))
}

#[derive(Debug, Clone, Copy)]
struct FlowCursor {
    x: f32,
    y: f32,
    row_bottom: f32,
}

impl FlowCursor {
    fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            row_bottom: y,
        }
    }

    fn advance(&mut self, rect: RectF, spacing: f32) {
        if rect.y > self.y + EPSILON {
            self.y = rect.y;
            self.row_bottom = rect.bottom();
        } else {
            self.row_bottom = self.row_bottom.max(rect.bottom());
        }
        self.x = rect.right() + spacing;
    }
}

fn subtract_rect_from_segments(segments: &mut Vec<FreeSegment>, rect: RectF) {
    let mut out = Vec::with_capacity(segments.len() + 1);
    for segment in segments.drain(..) {
        let left = segment.x;
        let right = segment.right();
        let cut_left = rect.left().max(left);
        let cut_right = rect.right().min(right);

        if cut_right <= left + EPSILON || cut_left >= right - EPSILON {
            out.push(segment);
            continue;
        }

        if cut_left > left + EPSILON {
            out.push(FreeSegment {
                x: left,
                width: cut_left - left,
            });
        }

        if cut_right < right - EPSILON {
            out.push(FreeSegment {
                x: cut_right,
                width: right - cut_right,
            });
        }
    }
    out.sort_by(|a, b| a.x.total_cmp(&b.x));
    *segments = out;
}

fn valid_positive_rect(rect: RectF) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > EPSILON
        && rect.height > EPSILON
}

fn sanitize_dimension(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn dedupe_f32(values: &mut Vec<f32>) {
    values.dedup_by(|a, b| (*a - *b).abs() <= EPSILON);
}
