// SPEC-HAL-011: Container Widget

use crate::core::layout::{
    Borders, Constraints, CornerRadius, LayoutResult, Margins, Padding, compute_box_model,
    shrink_constraints,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::style::{DecorationStack, DecorationState};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Generic wrapper for layout grouping & background
pub struct Container {
    id: WidgetInternalId,
    /// Wrapped child node. `None` when constructed standalone via
    /// `Container::empty` for SPEC-HAL-319 composition; the agent adds
    /// children later via `WidgetTree::add_child`.
    pub child: Option<NodeId>,
    pub padding: Padding,
    pub background: Option<Color>,
    pub radius: CornerRadius,
    pub fixed_width: Option<f32>,
    pub fixed_height: Option<f32>,
}

impl Container {
    pub fn new(child: NodeId, padding: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            child: Some(child),
            padding: Padding::all(padding),
            background: None,
            radius: CornerRadius::zero(),
            fixed_width: None,
            fixed_height: None,
        }
    }

    /// SPEC-HAL-319: Standalone constructor for runtime composition.
    /// No child — the agent adds children later via the tree.
    pub fn empty() -> Self {
        Self {
            id: WidgetInternalId::new(),
            child: None,
            padding: Padding::zero(),
            background: None,
            radius: CornerRadius::zero(),
            fixed_width: None,
            fixed_height: None,
        }
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadius::all(radius);
        self
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.fixed_width = Some(width);
        self
    }

    pub fn with_height(mut self, height: f32) -> Self {
        self.fixed_height = Some(height);
        self
    }
}

impl WidgetDowncast for Container {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Container {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Apply fixed width/height if specified
        let mut effective_constraints = constraints;
        if let Some(width) = self.fixed_width {
            effective_constraints.min_width = width;
            effective_constraints.max_width = width;
        }
        if let Some(height) = self.fixed_height {
            effective_constraints.min_height = height;
            effective_constraints.max_height = height;
        }

        // SPEC-HAL-010: Use canonical box model
        let mut child_constraints = shrink_constraints(
            effective_constraints,
            Margins::zero(),
            Borders::none(),
            self.padding,
        );

        // CRITICAL: Unless fixed_height is set, give child INFINITY height.
        // Container wraps to content - it should not constrain child height.
        // This fixes width-dependent height changes (FlowGrid rewrap during resize).
        if self.fixed_height.is_none() {
            child_constraints.max_height = f32::INFINITY;
        }

        // Layout child with reduced constraints
        let (child_size, child_content_size) = if let Some(child_id) = self.child {
            // SPEC-HAL-026 §8: descend via cx.layout_child_full. The full
            // variant returns LayoutResult so content_size propagates to
            // ScrollArea for scroll-extent calculation. Engine handles
            // dirty-bit clear, pre_layout, deferred_wiring, current_node,
            // cached_size, last_constraints, layout_epoch. Missing-child
            // structural corruption surfaces as the engine's INV-ZERO-2 panic.
            let result = cx.layout_child_full(child_id, child_constraints);
            let size = result.size;
            let content_size = result.content_size;

            // SPEC-HAL-010: Compute box model and position child
            let box_model = compute_box_model(size, Margins::zero(), Borders::none(), self.padding);
            let new_rect = RectF::new(
                box_model.content_offset.0,
                box_model.content_offset.1,
                size.width,
                size.height,
            );
            cx.set_child_rect(child_id, new_rect);

            (size, content_size)
        } else if !children.is_empty() {
            // SPEC-HAL-319: Standalone Container composed at runtime — no
            // single wired child. Stack the live tree's children
            // vertically inside the padded box.
            let box_offset =
                compute_box_model(Size::zero(), Margins::zero(), Borders::none(), self.padding);
            let (offset_x, offset_y) = box_offset.content_offset;
            let inner_max_w =
                (effective_constraints.max_width - self.padding.left - self.padding.right).max(0.0);
            let mut total = 0.0_f32;
            for &child_id in children {
                let child_size = cx.layout_child(child_id, child_constraints);
                cx.set_child_rect(
                    child_id,
                    RectF::new(
                        offset_x,
                        offset_y + total,
                        inner_max_w.max(child_size.width),
                        child_size.height,
                    ),
                );
                total += child_size.height;
            }
            let stacked = Size::new(inner_max_w, total);
            (stacked, stacked)
        } else {
            // SPEC-HAL-319: Standalone Container with no children yet.
            // Size is driven by fixed_width/fixed_height (or zero if neither set).
            (Size::zero(), Size::zero())
        };

        // SPEC-HAL-010: Final size from box model, or fixed dimensions if specified
        let box_model =
            compute_box_model(child_size, Margins::zero(), Borders::none(), self.padding);
        let size = Size::new(
            self.fixed_width.unwrap_or(box_model.size.width),
            self.fixed_height.unwrap_or(box_model.size.height),
        );

        // Propagate child's content_size with padding for scroll extent calculation
        let content_box_model = compute_box_model(
            child_content_size,
            Margins::zero(),
            Borders::none(),
            self.padding,
        );
        let full_content_size = Size::new(
            self.fixed_width.unwrap_or(content_box_model.size.width),
            self.fixed_height.unwrap_or(content_box_model.size.height),
        );

        LayoutResult::new(size, full_content_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // SPEC-HAL-010: Paint uses rect directly from traversal
        if let Some(bg_color) = self.background {
            let radius = self.radius.uniform();

            // Build decoration stack with background only (no border for Container)
            let stack = DecorationStack::background(bg_color, radius);

            // No focus, no hover for Container (pure chrome)
            let state = DecorationState::default();

            // base_z=1: background at z=1 (preserved)
            stack.paint(cx.scene, rect, state, 1);
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "Container"
    }

    fn height_stable(&self) -> bool {
        // Container with fixed_height won't change height when width changes
        self.fixed_height.is_some()
    }
}
