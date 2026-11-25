// SPEC-HAL-011: FixedSpacer Widget
// A simple spacer with fixed dimensions. No children, no events, no painting.

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Fixed-size spacer widget.
///
/// Use in FlexColumn for vertical spacing, FlexRow for horizontal spacing.
/// Returns exact dimensions in layout, paints nothing.
///
/// # Examples
/// ```ignore
/// // Vertical spacer in a column
/// tree.add_child(column, Box::new(FixedSpacer::vertical(100.0)), None, 0);
///
/// // Horizontal spacer in a row
/// tree.add_child(row, Box::new(FixedSpacer::horizontal(50.0)), None, 0);
/// ```
pub struct FixedSpacer {
    id: WidgetInternalId,
    width: f32,
    height: f32,
}

impl FixedSpacer {
    /// Create a spacer with explicit width and height.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            width,
            height,
        }
    }

    /// Create a vertical spacer (for use in FlexColumn).
    /// Width is 0, height is the specified value.
    pub fn vertical(height: f32) -> Self {
        Self::new(0.0, height)
    }

    /// Create a horizontal spacer (for use in FlexRow).
    /// Height is 0, width is the specified value.
    pub fn horizontal(width: f32) -> Self {
        Self::new(width, 0.0)
    }
}

impl WidgetDowncast for FixedSpacer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FixedSpacer {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Return fixed size, clamped to constraints
        let size = Size::new(
            self.width
                .clamp(constraints.min_width, constraints.max_width),
            self.height
                .clamp(constraints.min_height, constraints.max_height),
        );
        LayoutResult::fit(size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // No visual representation
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "FixedSpacer"
    }
}
