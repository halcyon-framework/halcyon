// SPEC-HAL-332: AuxiliaryText
// Compact muted supporting copy for descriptions, hints, and secondary UI text.
// Read-only selection only; no mutation, navigation, or semantic ownership.

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::RectF;
use crate::core::text_selection::TextSelectionPolicy;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::text_block::TextBlock;

/// Compact muted text for auxiliary descriptions and supporting copy.
pub struct AuxiliaryText {
    text: String,
    size: Option<f32>,
    block: TextBlock,
}

impl AuxiliaryText {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            block: TextBlock::new(text.clone()).with_selection_policy(TextSelectionPolicy::Enabled),
            text,
            size: None,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        let mut block =
            TextBlock::new(self.text.clone()).with_selection_policy(TextSelectionPolicy::Enabled);
        if let Some(size) = self.size {
            block = block.with_size(size);
        }
        self.block = block;
    }

    pub(crate) fn default_font_size(base: f32) -> f32 {
        (base - 1.0).max(10.0).round()
    }

    fn resolved_font_size(&self, base: f32) -> f32 {
        self.size.unwrap_or_else(|| Self::default_font_size(base))
    }

    fn sync_block_style(&mut self, cx: &LayoutContext<'_>) {
        self.block
            .set_font_size(self.resolved_font_size(cx.theme.typography.font_size));
        self.block.set_color(Some(cx.theme.colors.typography.muted));
        self.block
            .set_selection_policy(TextSelectionPolicy::Enabled);
    }
}

impl WidgetDowncast for AuxiliaryText {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for AuxiliaryText {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.sync_block_style(cx);
        self.block.layout(cx, constraints, children)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.block.paint(cx, rect, children);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        self.block.handle_event(cx, event, phase)
    }

    fn id(&self) -> WidgetInternalId {
        self.block.id()
    }

    fn hit_testable(&self) -> bool {
        self.block.hit_testable()
    }

    fn focusable(&self) -> bool {
        self.block.focusable()
    }

    fn capture_eligible(&self) -> bool {
        self.block.capture_eligible()
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.block.on_capture_leave(cx);
    }

    fn debug_type_name(&self) -> &'static str {
        "AuxiliaryText"
    }
}
