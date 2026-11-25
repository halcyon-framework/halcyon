//! Side captions for real tree-owned controls participating in local alignment.
use crate::core::layout::{
    Constraints, ControlPartColumns, ControlPartRequirement, ControlPartsRequest,
    ControlPartsRequirements, ControlSideRequirements, LayoutResult,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// A layout-only right caption with one real control child.
///
/// The child must implement body-only control-parts measurement (TextField or
/// ComboBox). It retains all editing, hit, focus, Capture and popup ownership.
/// Mark this row, or its transparent TooltipHost, as an alignment member.
/// The caption itself is never an input target. See docs/CONTROL-ALIGNMENT.md.
pub struct ControlCaption {
    id: WidgetInternalId,
    caption: String,
    text_origin: (f32, f32),
    overflow_clip: Option<RectF>,
}

impl ControlCaption {
    pub fn new(caption: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            caption: caption.into(),
            text_origin: (0.0, 0.0),
            overflow_clip: None,
        }
    }

    pub fn set_caption(&mut self, caption: impl Into<String>, cx: &mut UpdateContext) {
        let caption = caption.into();
        if self.caption != caption {
            self.caption = caption;
            cx.invalidate_layout();
        }
    }
}

impl WidgetDowncast for ControlCaption {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ControlCaption {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        assert_eq!(
            children.len(),
            1,
            "ControlCaption requires one real control child"
        );
        let child = children[0];
        let body = cx.measure_control_parts(child);
        assert_eq!(
            body.leading,
            ControlSideRequirements::default(),
            "ControlCaption requires a body-only child"
        );
        assert_eq!(
            body.trailing,
            ControlSideRequirements::default(),
            "ControlCaption requires a body-only child"
        );
        let font_size = cx.theme.typography.font_size;
        let metrics = cx.text_engine.font_metrics(font_size);
        let (caption_width, _) = cx.text_engine.measure(&self.caption, font_size);
        let req = ControlPartsRequirements {
            leading: ControlSideRequirements::default(),
            trailing: ControlSideRequirements {
                label: Some(ControlPartRequirement {
                    width: caption_width,
                    gap: cx.theme.geometry.slider.label_gap,
                }),
                value: None,
            },
            body_width: body.body_width,
            height: body.height.max(metrics.line_height),
        };
        let natural = Size::new(req.body_width + req.trailing.width(), req.height);
        if cx.control_parts_request() == Some(ControlPartsRequest::Measure) {
            cx.report_control_parts(req);
            return LayoutResult::fit(natural);
        }
        let size = constraints.constrain(natural);
        let columns = match cx.control_parts_request() {
            Some(ControlPartsRequest::Arrange(columns)) => columns,
            _ => ControlPartColumns::new(size.width, req.leading, req.trailing),
        };
        let logical_height = size.height.max(req.height);
        let child_result = cx.arrange_control_parts(
            child,
            Constraints::tight(Size::new(columns.body.width, logical_height)),
            ControlPartColumns::new(columns.body.width, body.leading, body.trailing),
        );
        cx.set_child_rect(
            child,
            RectF::new(
                columns.body.start,
                0.0,
                child_result.size.width,
                child_result.size.height,
            ),
        );
        let baseline = compute_control_text_baseline_y(
            logical_height,
            metrics.ascent,
            metrics.descent,
            cx.theme.typography.baseline_offset,
        );
        self.text_origin = (columns.trailing_label.start, baseline - metrics.ascent);
        let content = Size::new(
            columns
                .content_width()
                .max(columns.body.start + child_result.content_size.width)
                .max(size.width),
            logical_height.max(child_result.content_size.height),
        );
        self.overflow_clip = (content.width > size.width || content.height > size.height)
            .then_some(RectF::new(0.0, 0.0, size.width, size.height));
        LayoutResult::new(size, content)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let run = cx
            .text_engine
            .shape(&self.caption, cx.theme.typography.font_size);
        cx.text_engine.draw(
            &run,
            rect.x + self.text_origin.0,
            rect.y + self.text_origin.1,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT,
            cx.scene,
        );
    }
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
        "ControlCaption"
    }
}
