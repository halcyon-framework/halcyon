use crate::core::event::{Event, EventPhase};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::RectF;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::text_area::TextArea;

/// Multi-line text editor widget.
///
/// `TextEdit` is a compatibility wrapper over the active `TextArea` editing
/// implementation. Text storage, layout, selection, caret, scrolling, focus,
/// capture, clipboard, IME, undo/redo, and rendering are owned by the inner
/// `TextArea`.
pub struct TextEdit {
    inner: TextArea,
}

impl TextEdit {
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.with_enabled(enabled);
        self
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.inner = self.inner.with_read_only(read_only);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }

    pub fn is_read_only(&self) -> bool {
        self.inner.is_read_only()
    }

    pub(crate) fn apply_availability_policy(&mut self, enabled: bool, read_only: bool) {
        self.inner.apply_availability_policy(enabled, read_only);
    }

    /// Create a new empty TextEdit.
    pub fn new() -> Self {
        Self {
            inner: TextArea::new(),
        }
    }

    /// Create a TextEdit with initial text.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            inner: TextArea::new().with_text(text),
        }
    }

    /// Set minimum height before content growth.
    pub fn with_min_height(mut self, height: f32) -> Self {
        self.inner = self.inner.with_min_height(height);
        self
    }

    /// Set maximum height before the editor scrolls.
    pub fn with_max_height(mut self, height: f32) -> Self {
        self.inner = self.inner.with_max_height(height);
        self
    }

    /// Set fixed editor height.
    pub fn with_fixed_height(mut self, height: f32) -> Self {
        self.inner = self.inner.with_fixed_height(height);
        self
    }

    /// Return the current text content.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Default for TextEdit {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for TextEdit {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TextEdit {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        self.inner.update(dt, cx);
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.inner.layout(cx, constraints, children)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.inner.paint(cx, rect, children);
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        self.inner.paint_overlay(cx, rect, tree_widget_id);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        self.inner.handle_event(cx, event, phase)
    }

    fn id(&self) -> WidgetInternalId {
        self.inner.id()
    }

    fn focusable(&self) -> bool {
        self.inner.focusable()
    }

    fn needs_animation(&self) -> bool {
        self.inner.needs_animation()
    }

    fn wants_scroll(&self) -> bool {
        self.inner.wants_scroll()
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.inner.on_focus(cx);
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.inner.on_blur(cx);
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        self.inner.on_hover_enter(cx);
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.inner.on_hover_leave(cx);
    }

    fn as_anchor_resolver(&self) -> Option<&dyn crate::core::scroll_state::AnchorResolver> {
        self.inner.as_anchor_resolver()
    }

    fn capture_eligible(&self) -> bool {
        self.inner.capture_eligible()
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        self.inner.on_capture_enter(cx);
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.inner.on_capture_leave(cx);
    }

    fn debug_type_name(&self) -> &'static str {
        "TextEdit"
    }
}
