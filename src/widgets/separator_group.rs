// SPEC-HAL-221: SeparatorGroup Layout Helper
// Arranges children along a single axis with inert visual separators between them.
// No interaction, no semantics, no ownership, no grouping identity.
// Separator visual matches SPEC-HAL-213 Divider primitive exactly.
//
// Cross-axis alignment policy:
//   SeparatorGroup does not define cross-axis alignment.
//   Children are placed at cross-origin (0.0) by convention.
//   Alignment is delegated to the parent layout.
//
// Divider span policy:
//   Dividers span the full group cross-axis extent.
//   No per-child divider sizing is attempted.

use crate::core::layout::{Axis, Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::RoundedCorners;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

pub struct SeparatorGroup {
    id: WidgetInternalId,
    axis: Axis,
}

impl SeparatorGroup {
    pub fn horizontal() -> Self {
        Self {
            id: WidgetInternalId::new(),
            axis: Axis::Horizontal,
        }
    }

    pub fn vertical() -> Self {
        Self {
            id: WidgetInternalId::new(),
            axis: Axis::Vertical,
        }
    }
}

impl WidgetDowncast for SeparatorGroup {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SeparatorGroup {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Theme-driven gap between children. Aliased for clarity: this is the
        // spacing reserved for each separator, sourced from divider geometry.
        let separator_gap = cx.theme.geometry.divider.thickness;
        let child_count = children.len();

        let mut main_cursor = 0.0f32;
        let mut max_cross = 0.0f32;

        for (i, &child) in children.iter().enumerate() {
            let child_constraints = match self.axis {
                Axis::Horizontal => Constraints::new(
                    0.0,
                    f32::INFINITY,
                    constraints.min_height,
                    constraints.max_height,
                ),
                Axis::Vertical => Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    0.0,
                    f32::INFINITY,
                ),
            };
            let child_size = cx.layout_child(child, child_constraints);

            let rect = match self.axis {
                Axis::Horizontal => {
                    RectF::new(main_cursor, 0.0, child_size.width, child_size.height)
                }
                Axis::Vertical => RectF::new(0.0, main_cursor, child_size.width, child_size.height),
            };
            cx.set_child_rect(child, rect);

            let (child_main, child_cross) = match self.axis {
                Axis::Horizontal => (child_size.width, child_size.height),
                Axis::Vertical => (child_size.height, child_size.width),
            };

            main_cursor += child_main;
            max_cross = max_cross.max(child_cross);

            if i + 1 < child_count {
                main_cursor += separator_gap;
            }
        }

        let size = match self.axis {
            Axis::Horizontal => Size::new(main_cursor, max_cross),
            Axis::Vertical => Size::new(max_cross, main_cursor),
        };
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        if children.len() < 2 {
            return;
        }

        let separator_gap = cx.theme.geometry.divider.thickness;
        let color = cx.theme.colors.borders.default;

        // Paint dividers between adjacent children.
        // Visual matches SPEC-HAL-213 Divider: theme color, theme thickness, zero radius.
        // Dividers span the full group cross-axis extent.
        for i in 0..children.len() - 1 {
            if let Some(child_rect) = cx.tree.get_rect(children[i]) {
                let divider_rect = match self.axis {
                    Axis::Horizontal => RectF::new(
                        rect.x + child_rect.x + child_rect.width,
                        rect.y,
                        separator_gap,
                        rect.height,
                    ),
                    Axis::Vertical => RectF::new(
                        rect.x,
                        rect.y + child_rect.y + child_rect.height,
                        rect.width,
                        separator_gap,
                    ),
                };
                cx.scene.push_rounded_rect(
                    divider_rect,
                    color,
                    RoundedCorners::uniform(0.0),
                    1,
                    None,
                );
            }
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}
