// SPEC-HAL-213: Divider Primitive
// Visual-only separation element. Zero interaction, zero state, zero semantics.

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, RoundedCorners};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Purely visual separator line. Theme-driven geometry, no interaction, no state.
pub struct Divider {
    id: WidgetInternalId,
    orientation: Orientation,
    /// Optional color override. None = theme.colors.borders.default.
    color: Option<Color>,
}

impl Divider {
    pub fn horizontal() -> Self {
        Self {
            id: WidgetInternalId::new(),
            orientation: Orientation::Horizontal,
            color: None,
        }
    }

    pub fn vertical() -> Self {
        Self {
            id: WidgetInternalId::new(),
            orientation: Orientation::Vertical,
            color: None,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl WidgetDowncast for Divider {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Divider {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let thickness = cx.theme.geometry.divider.thickness;
        let span_width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            constraints.min_width
        };
        let span_height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            constraints.min_height
        };
        let size = match self.orientation {
            Orientation::Horizontal => Size::new(
                span_width,
                thickness.clamp(constraints.min_height, constraints.max_height),
            ),
            Orientation::Vertical => Size::new(
                thickness.clamp(constraints.min_width, constraints.max_width),
                span_height,
            ),
        };
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let color = self.color.unwrap_or(cx.theme.colors.borders.default);
        let scale = cx.scale_factor;
        let bx = crate::core::math::snap_to_physical(rect.x, scale);
        let by = crate::core::math::snap_to_physical(rect.y, scale);
        let br = crate::core::math::snap_to_physical(rect.x + rect.width, scale);
        let bb = crate::core::math::snap_to_physical(rect.y + rect.height, scale);
        // Must use push_rounded_rect to route through path pipeline (renders after backgrounds).
        // push_rect uses rect pipeline which renders BEFORE path pipeline — backgrounds overpaint it.
        cx.scene.push_rounded_rect(
            RectF::new(bx, by, br - bx, bb - by),
            color,
            RoundedCorners::uniform(0.0),
            1,
            cx.scene_key,
        );
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "Divider"
    }
}
