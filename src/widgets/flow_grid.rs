// FlowGrid - A preset wrapper around Flex for responsive grid layouts
//
// FlowGrid provides a simple API for creating responsive grids where items
// automatically wrap to new rows as the container width changes.

use crate::core::layout::{Alignment, FlexParams, LayoutResult};
use crate::core::math::RectF;
use crate::core::paint::Color;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::flex::Flex;

/// A responsive grid that wraps items into rows
///
/// FlowGrid is a convenience wrapper around Flex that configures it for
/// flow/grid behavior. Items automatically wrap to new rows and grow to
/// fill available space while respecting a minimum item width.
pub struct FlowGrid {
    id: WidgetInternalId,
    flex: Flex,
    min_item_width: f32,
    spacing: f32,
}

impl FlowGrid {
    /// Create a new FlowGrid with the specified minimum item width
    ///
    /// Items will grow to fill available space but won't shrink below
    /// min_item_width. When items don't fit in a row, they wrap to the next.
    pub fn new(min_item_width: f32) -> Self {
        let flex = Flex::row().with_wrap().with_align_items(Alignment::Stretch);

        Self {
            id: WidgetInternalId::new(),
            flex,
            min_item_width,
            spacing: 0.0,
        }
    }

    /// Set spacing between items (both horizontal and vertical)
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self.flex = Flex::row()
            .with_wrap()
            .with_align_items(Alignment::Stretch)
            .with_spacing(spacing);
        self
    }

    /// Set background color
    pub fn with_background(mut self, color: Color) -> Self {
        self.flex = Flex::row()
            .with_wrap()
            .with_align_items(Alignment::Stretch)
            .with_spacing(self.spacing)
            .with_background(color);
        self
    }

    /// Get the FlexParams for flow grid items
    ///
    /// Call this when adding children to configure them for flow behavior
    pub fn item_flex_params(&self) -> FlexParams {
        FlexParams {
            grow: 1.0,
            shrink: 1.0,
            basis: self.min_item_width,
        }
    }
}

impl WidgetDowncast for FlowGrid {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FlowGrid {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: crate::core::layout::Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Invariant: FlowGrid requires positive min_item_width for proper tile sizing
        debug_assert!(
            self.min_item_width > 0.0,
            "FlowGrid: min_item_width must be > 0 for proper wrap behavior, got {}",
            self.min_item_width
        );

        // Configure each child with flow grid flex params
        let params = self.item_flex_params();
        debug_assert!(
            params.basis > 0.0,
            "FlowGrid: basis must be > 0, got {}",
            params.basis
        );

        for &child_id in children {
            cx.tree.set_flex(child_id, params);
        }

        // SPEC-HAL-086 Stage 2: Delegate to internal Flex (returns LayoutResult)
        let result = self.flex.layout(cx, constraints, children);

        // Compute actual content height from child positions (post-Flex layout)
        // SPEC-HAL-141: Use getter for private geometry field
        let mut max_child_bottom: f32 = 0.0;
        for &child_id in children {
            if let Some(rect) = cx.tree.get_rect(child_id) {
                let child_bottom = rect.y + rect.height;
                if child_bottom > max_child_bottom {
                    max_child_bottom = child_bottom;
                }
            }
        }

        // HAL_DEBUG_FLOWGRID: Diagnostic for content height reporting
        #[cfg(debug_assertions)]
        if std::env::var_os("HAL_DEBUG_FLOWGRID").is_some() {
            eprintln!(
                "[HAL_DEBUG_FLOWGRID] FlowGrid layout: flex_size_h={:.1} flex_content_h={:.1} computed_content_h={:.1} max_h={:.1}",
                result.size.height,
                result.content_size.height,
                max_child_bottom,
                constraints.max_height,
            );
        }

        // FlowGrid reports actual content height for both size and content_size.
        // Parent containers (GroupBox, Flex) need the true height to position siblings correctly.
        // ScrollArea will clip and scroll if content exceeds viewport.
        LayoutResult::new(
            crate::core::math::Size::new(result.size.width, max_child_bottom),
            crate::core::math::Size::new(result.content_size.width, max_child_bottom),
        )
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.flex.paint(cx, rect, children)
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "FlowGrid"
    }
}
