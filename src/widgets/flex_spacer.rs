// SPEC-HAL-011: FlexSpacer Widget
// A flexible spacer that expands to fill available space.
// Use with tree.set_flex(node, FlexParams::flexible(1.0)) for flex behavior.

use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Flexible spacer widget.
///
/// Returns zero size in layout; relies on FlexParams to receive space from parent.
/// Use `tree.set_flex(node, FlexParams::flexible(1.0))` after adding to tree.
///
/// # Examples
/// ```ignore
/// // Flex spacer between two buttons in a row
/// let spacer = tree.add_child(row, Box::new(FlexSpacer::new()), None, 0);
/// tree.set_flex(spacer, FlexParams::flexible(1.0));
/// ```
pub struct FlexSpacer {
    id: WidgetInternalId,
}

impl FlexSpacer {
    /// Create a new flexible spacer.
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl Default for FlexSpacer {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for FlexSpacer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for FlexSpacer {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Invariant 4.1.4: Spacer returns 0 during natural measurement pass (loose constraints)
        // Second pass (relayout): constraints are tight with distributed width, so we return that.
        //
        // This behavior ensures:
        // - First pass: min_width=0 → returns 0, allowing flex distribution to compute share
        // - Second pass: min_width=distributed_width → returns exactly that width
        let size = Size::new(constraints.min_width, constraints.min_height);

        // Guard: When constraints are tight (min==max), we must return the exact size
        debug_assert!(
            size.width >= 0.0 && size.height >= 0.0,
            "FlexSpacer: size must be non-negative, got {:?}",
            size
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
        "FlexSpacer"
    }
}
