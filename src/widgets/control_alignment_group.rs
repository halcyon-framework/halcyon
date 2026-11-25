//! Local, explicit alignment of owned control parts. See docs/CONTROL-ALIGNMENT.md.
use crate::core::layout::{
    Constraints, ControlPartColumns, ControlSideRequirements, ControlTrailingOrder, LayoutResult,
};
use crate::core::math::{RectF, Size};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// A layout-only column aligning explicitly opted-in direct rows.
///
/// The group owns shared dimensions, never labels, editors, values or input.
/// Ordinary rows and nested groups do not implicitly join its columns.
pub struct ControlAlignmentGroup {
    id: WidgetInternalId,
    spacing: f32,
    trailing_order: ControlTrailingOrder,
    overflow_clip: Option<RectF>,
}

impl ControlAlignmentGroup {
    pub fn column() -> Self {
        Self {
            id: WidgetInternalId::new(),
            spacing: 0.0,
            trailing_order: ControlTrailingOrder::default(),
            overflow_clip: None,
        }
    }

    /// Select right-side slot order in this group only; the default is label then value.
    /// Participants consume the assigned named slots without moving ownership.
    pub fn with_trailing_order(mut self, order: ControlTrailingOrder) -> Self {
        self.trailing_order = order;
        self
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        assert!(spacing.is_finite() && spacing >= 0.0);
        self.spacing = spacing;
        self
    }
}

impl WidgetDowncast for ControlAlignmentGroup {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ControlAlignmentGroup {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let mut leading = ControlSideRequirements::default();
        let mut trailing = ControlSideRequirements::default();
        let mut body: f32 = 0.0;
        let mut ordinary_width: f32 = 0.0;
        let mut natural_height = 0.0;
        let mut rows = Vec::with_capacity(children.len());
        for &child in children {
            let member = cx
                .tree
                .node(child)
                .is_some_and(|n| n.control_alignment_member);
            let size = if member {
                let req = cx.measure_control_parts(child);
                leading.include(req.leading);
                trailing.include(req.trailing);
                body = body.max(req.body_width);
                Size::new(
                    req.leading.width() + req.body_width + req.trailing.width(),
                    req.height,
                )
            } else {
                let size = cx.measure_control_row(child).size;
                ordinary_width = ordinary_width.max(size.width);
                size
            };
            natural_height += size.height;
            rows.push((child, member, size));
        }
        if !rows.is_empty() {
            natural_height += self.spacing * (rows.len() - 1) as f32;
        }
        let natural_width = ordinary_width.max(leading.width() + body + trailing.width());
        let size = constraints.constrain(Size::new(natural_width, natural_height));
        let columns = ControlPartColumns::with_trailing_order(
            size.width,
            leading,
            trailing,
            self.trailing_order,
        );
        let mut y = 0.0;
        let mut content_width: f32 = columns.content_width();
        for (child, member, natural) in rows {
            let result = if member {
                cx.arrange_control_parts(
                    child,
                    Constraints::tight(Size::new(size.width, natural.height)),
                    columns,
                )
            } else {
                let width = natural.width.min(size.width);
                let child_size =
                    cx.layout_child(child, Constraints::tight(Size::new(width, natural.height)));
                LayoutResult::new(
                    child_size,
                    cx.tree.get_content_size(child).unwrap_or(child_size),
                )
            };
            cx.set_child_rect(
                child,
                RectF::new(0.0, y, result.size.width, result.size.height),
            );
            content_width = content_width.max(result.content_size.width);
            y += result.content_size.height.max(result.size.height) + self.spacing;
        }
        let content_height = if children.is_empty() {
            0.0
        } else {
            y - self.spacing
        };
        let content = Size::new(
            content_width.max(size.width),
            content_height.max(size.height),
        );
        self.overflow_clip = (content.width > size.width || content.height > size.height)
            .then_some(RectF::new(0.0, 0.0, size.width, size.height));
        LayoutResult::new(size, content)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn hit_testable(&self) -> bool {
        false
    }
    fn clip_bounds(&self) -> Option<RectF> {
        self.overflow_clip
    }
    fn debug_type_name(&self) -> &'static str {
        "ControlAlignmentGroup"
    }
}
