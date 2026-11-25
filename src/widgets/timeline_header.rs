// SPEC-HAL-336 §4: TimelineHeader — Tier Projection Widget
//
// Thin projection over TemporalSubstrate. Paints the civil header tiers
// from the substrate's per-frame resolution.
//
// INV-336-1: This widget never computes tiers, gridlines, mapping, or
//            non-working spans. It reads substrate.resolution() — the one
//            resolution per frame shared with the canvas and hit-testing.
// FORBID-336-4: No zoom or scroll handlers; the header never owns or
//            mutates time geometry. Viewport gestures belong to the canvas
//            or the composing screen.
//
// Not focusable, not hit-testable, emits nothing.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::engines::temporal::TemporalSubstrate;
use crate::utils::id::WidgetInternalId;

pub struct TimelineHeader {
    id: WidgetInternalId,
    substrate: Rc<RefCell<TemporalSubstrate>>,
}

impl TimelineHeader {
    pub fn new(substrate: Rc<RefCell<TemporalSubstrate>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            substrate,
        }
    }
}

impl WidgetDowncast for TimelineHeader {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TimelineHeader {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let tier_h = cx.theme.list_geom.row_height;
        let tier_count = self.substrate.borrow().resolution().tiers.len().max(1) as f32;
        let height = (tier_count * tier_h).clamp(constraints.min_height, constraints.max_height);
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            constraints.min_width
        };
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let substrate = self.substrate.borrow();
        let resolution = substrate.resolution();

        let tier_h = cx.theme.list_geom.row_height;
        let pad = cx.theme.list_geom.padding_x;
        let font_size = cx.theme.typography.font_size;
        let bg = cx.theme.colors.surfaces.level1;
        let separator = cx.theme.colors.borders.subtle;
        let text_color = cx.theme.colors.typography.secondary;

        cx.scene.push_rect(rect, bg, LAYER_CONTENT, None);
        cx.scene.begin_clip(rect, None);

        for (tier_index, tier) in resolution.tiers.iter().enumerate() {
            let row_y = rect.y + tier_index as f32 * tier_h;

            for cell in &tier.cells {
                let cell_x = rect.x + cell.x;

                // Cell boundary tick at the cell start, when inside the viewport.
                if cell.x >= 0.0 && cell.x <= rect.width {
                    cx.scene.push_rect(
                        RectF::new(cell_x, row_y, 1.0, tier_h),
                        separator,
                        LAYER_CONTENT,
                        None,
                    );
                }

                let run = cx.text_engine.shape(&cell.label, font_size);
                let text_x = cell_x + pad;
                let text_y = row_y + (tier_h - run.height) / 2.0;
                cx.text_engine.draw(
                    &run,
                    text_x,
                    text_y,
                    text_color,
                    LAYER_CONTENT + 1,
                    cx.scene,
                );
            }

            // Tier separator under the row.
            cx.scene.push_rect(
                RectF::new(rect.x, row_y + tier_h - 1.0, rect.width, 1.0),
                separator,
                LAYER_CONTENT,
                None,
            );
        }

        cx.scene.end_clip();
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    /// Purely visual; clicks pass through (SPEC-HAL-336 §4).
    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "TimelineHeader"
    }
}
