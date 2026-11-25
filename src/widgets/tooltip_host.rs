// SPEC-HAL-226A: Consumer-facing tooltip trigger wrapper.

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tooltip::{TooltipPayload, TooltipPlacement};
use crate::utils::id::WidgetInternalId;

/// Layout-transparent single-child wrapper that binds tooltip content to its bounds.
///
/// The child remains a normal [`crate::core::widget_tree::WidgetTree`] child; this
/// wrapper does not own another widget directly. Tooltip timing, visibility, and
/// placement are managed by Halcyon's tooltip manager.
pub struct TooltipHost {
    id: WidgetInternalId,
    content: TooltipPayload,
    placement: TooltipPlacement,
}

impl TooltipHost {
    /// Creates a tooltip host with plain text and automatic placement.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            content: TooltipPayload::Text(text.into()),
            placement: TooltipPlacement::Auto,
        }
    }

    /// Sets the preferred tooltip placement.
    pub fn with_placement(mut self, placement: TooltipPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Replaces the tooltip text while preserving the wrapper instance.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.content = TooltipPayload::Text(text.into());
    }
}

impl WidgetDowncast for TooltipHost {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TooltipHost {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let Some(&child) = children.first() else {
            return LayoutResult::fit(Size::zero());
        };
        let result = cx.forward_control_parts(child, constraints);
        cx.set_child_rect(
            child,
            RectF::new(0.0, 0.0, result.size.width, result.size.height),
        );
        LayoutResult::new(result.size, result.content_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.register_tooltip_at(rect, self.content.clone(), self.placement);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "TooltipHost"
    }
}
