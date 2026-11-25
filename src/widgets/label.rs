// SPEC-HAL-011: Label (Text Display)

use crate::core::event::{Event, EventPhase};
use crate::core::event::{KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::text_selection::{
    ReadOnlyTextSelection, TextSelectionPolicy, hit_test_run, selection_spans,
};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Text label widget
pub struct Label {
    id: WidgetInternalId,
    pub text: String,
    /// Font size override. None = use theme.typography.font_size (default).
    pub size: Option<f32>,
    /// Text color. None = use theme.colors.typography.primary (default).
    pub color: Option<Color>,
    typography: Option<crate::core::paragraph::TextStyle>,
    paint_disabled: bool,
    /// Resolved font size from last layout (for text_info() which lacks theme access)
    resolved_font_size: f32,
    /// SPEC-HAL-315: Optional read-only text selection.
    /// None = no selection (default, zero overhead). Some = opt-in selectable.
    text_selection: Option<ReadOnlyTextSelection>,
    /// Optional ref/for target: when set, clicking the label dispatches a
    /// synthetic primary-button press to the target widget so a Label can
    /// activate a sibling Switch/Checkbox/Button.
    for_widget_id: Option<u64>,
}

impl Label {
    pub fn with_typography(
        mut self,
        style: crate::core::paragraph::TextStyle,
    ) -> Result<Self, crate::core::text_engine::TextInputError> {
        style.validate()?;
        self.typography = Some(style);
        Ok(self)
    }

    pub fn set_typography(
        &mut self,
        style: Option<crate::core::paragraph::TextStyle>,
        cx: &mut crate::core::widget::UpdateContext,
    ) -> Result<bool, crate::core::text_engine::TextInputError> {
        if let Some(style) = &style {
            style.validate()?;
        }
        if self.typography == style {
            return Ok(false);
        }
        self.typography = style;
        cx.invalidate_layout();
        Ok(true)
    }

    pub fn typography(&self) -> Option<&crate::core::paragraph::TextStyle> {
        self.typography.as_ref()
    }

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            text: text.into(),
            size: None,  // Default: use theme.typography.font_size
            color: None, // Default: use theme.colors.typography.primary
            typography: None,
            paint_disabled: false,
            resolved_font_size: 13.0, // EnterpriseGrid baseline (synced in layout)
            text_selection: None,
            for_widget_id: None,
        }
    }

    /// Set the target widget that clicks on this label should activate.
    pub fn set_for_widget(&mut self, widget_id: u64) {
        self.for_widget_id = Some(widget_id);
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        // INV-315-1: Sync selection text, clamp offsets.
        if let Some(ref mut sel) = self.text_selection {
            sel.set_text(self.text.clone());
        }
    }

    /// SPEC-HAL-315: Enable text selection with the given policy.
    /// Default is Disabled (no selection, no overhead).
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        if policy != TextSelectionPolicy::Disabled {
            self.text_selection = Some(ReadOnlyTextSelection::new(policy, self.text.clone()));
        }
        self
    }

    /// SPEC-HAL-315: Set selection policy at runtime.
    pub fn set_selection_policy(&mut self, policy: TextSelectionPolicy) {
        if policy == TextSelectionPolicy::Disabled {
            self.text_selection = None;
        } else {
            match &mut self.text_selection {
                Some(sel) => sel.set_policy(policy),
                None => {
                    self.text_selection =
                        Some(ReadOnlyTextSelection::new(policy, self.text.clone()));
                }
            }
        }
    }

    /// Whether this label has selectable text.
    fn is_selectable(&self) -> bool {
        self.text_selection.is_some()
    }
}

impl WidgetDowncast for Label {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Label {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Resolve font size: explicit override or theme default
        let font_size = self.typography.as_ref().map_or_else(
            || self.size.unwrap_or(cx.theme.typography.font_size),
            |style| style.font_size,
        );
        // Cache for text_info() which lacks theme access
        self.resolved_font_size = font_size;

        // SPEC-HAL-011 Section 3.5: Use LayoutContext text metrics to measure text
        let width = cx
            .text_engine
            .shape_styled(&self.text, self.typography.as_ref(), font_size)
            .expect("UI-TYPOGRAPHY-INPUT: Label measurement failed")
            .width;

        // Get accurate font metrics for height
        let metrics = if self.typography.is_some() {
            cx.text_engine
                .metrics_styled(self.typography.as_ref(), font_size)
                .expect("UI-TYPOGRAPHY-INPUT: Label metrics failed")
        } else {
            cx.text_engine.metrics(&self.text, font_size)
        };
        let height = metrics.line_height; // Use line_height for accurate spacing

        // Constrain to bounds
        let final_width = width.clamp(constraints.min_width, constraints.max_width);
        let final_height = height.clamp(constraints.min_height, constraints.max_height);

        // PM-005: Report natural text extent as content_size so ScrollArea
        // can detect horizontal overflow even when rendered size is clamped.
        let constrained = Size::new(final_width, final_height);
        let natural = Size::new(width, height);
        LayoutResult::new(constrained, natural)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.paint_disabled {
            return;
        }

        let font_size = self.size.unwrap_or(cx.theme.typography.font_size);
        let run = cx
            .text_engine
            .shape_styled(&self.text, self.typography.as_ref(), font_size)
            .expect("UI-TYPOGRAPHY-INPUT: Label paint input failed");
        let color = self.color.unwrap_or(cx.theme.colors.typography.primary);

        // SPEC-HAL-078B §7: Interpolated geometry positions text; completed
        // layout owns clipping. An unbound caller supplies its own bounds.
        let clip_rect = cx.authoritative_rect_for_paint().unwrap_or(rect);
        let needs_clip = run.width > clip_rect.width + 0.5;

        if needs_clip {
            cx.scene.begin_clip(clip_rect, None);
        }

        // SPEC-HAL-315: Draw with selection highlight if active.
        if let Some(ref sel) = self.text_selection {
            if sel.has_selection() {
                let (start, end) = sel.range();
                let highlight_color = cx.theme.text_selection_color();
                let spans = selection_spans(&run, start, end, run.height, 0.0, highlight_color);
                cx.text_engine.draw_with_backgrounds(
                    &run,
                    rect.x,
                    rect.y,
                    color,
                    spans,
                    crate::core::paint::LAYER_CONTENT,
                    cx.scene,
                );
                if needs_clip {
                    cx.scene.end_clip();
                }
                return;
            }
        }

        cx.text_engine.draw(
            &run,
            rect.x,
            rect.y,
            color,
            crate::core::paint::LAYER_CONTENT,
            cx.scene,
        );

        if needs_clip {
            cx.scene.end_clip();
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // ref/for: if no text selection but a for_widget_id is set, forward
        // primary-button presses to the target widget. Switch/Checkbox toggle
        // on PointerDown so a synthetic Down with local_pos inside the
        // target's bounds is enough.
        if self.text_selection.is_none() {
            if let Some(target_raw) = self.for_widget_id {
                if let Event::Pointer(pe) = event {
                    if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                        && pe.kind == PointerEventKind::Down
                        && pe.button == Some(PointerButton::Primary)
                    {
                        let target = crate::utils::id::TreeWidgetId(target_raw);
                        let synthetic = Event::Pointer(crate::core::event::PointerEvent {
                            id: crate::core::event::PointerId::MOUSE,
                            global_pos: pe.global_pos,
                            local_pos: (1.0, 1.0),
                            kind: PointerEventKind::Down,
                            button: Some(PointerButton::Primary),
                            modifiers: pe.modifiers,
                            pointer_type: crate::core::event::PointerType::Mouse,
                        });
                        cx.dispatch_to_child(target, &synthetic, EventPhase::Target);
                        return true;
                    }
                }
            }
            return false;
        }
        let sel = self.text_selection.as_mut().unwrap();

        // Resolve font size once for all branches.
        let font_size = self.resolved_font_size;

        match event {
            // ── PointerDown: start selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Down
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let click_count = cx.click_count();
                let in_capture = cx.has_pointer_capture();
                let run = cx
                    .text_engine()
                    .shape_styled(&self.text, self.typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Label selection failed");
                let offset = hit_test_run(&run, pe.local_pos.0);
                let shift = pe.modifiers.shift;
                if sel.pointer_down(offset, click_count, shift, in_capture, shift) {
                    cx.capture_pointer(PointerButton::Primary);
                    return true;
                }
                false
            }

            // ── DoubleClick: word selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::DoubleClick
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let in_capture = cx.has_pointer_capture();
                let run = cx
                    .text_engine()
                    .shape_styled(&self.text, self.typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Label selection failed");
                let offset = hit_test_run(&run, pe.local_pos.0);
                if sel.pointer_down(offset, 2, false, in_capture, false) {
                    return true;
                }
                false
            }

            // ── TripleClick: select all ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::TripleClick
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let in_capture = cx.has_pointer_capture();
                let run = cx
                    .text_engine()
                    .shape_styled(&self.text, self.typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Label selection failed");
                let offset = hit_test_run(&run, pe.local_pos.0);
                if sel.pointer_down(offset, 3, false, in_capture, false) {
                    return true;
                }
                false
            }

            // ── PointerMove: extend drag selection ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Move
                    && sel.is_dragging() =>
            {
                let run = cx
                    .text_engine()
                    .shape_styled(&self.text, self.typography.as_ref(), font_size)
                    .expect("UI-TYPOGRAPHY-INPUT: Label selection failed");
                let offset = hit_test_run(&run, pe.local_pos.0);
                sel.pointer_move(offset);
                true
            }

            // ── PointerUp: end drag ──
            Event::Pointer(pe)
                if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                    && pe.kind == PointerEventKind::Up
                    && pe.button == Some(PointerButton::Primary) =>
            {
                if sel.is_dragging() {
                    sel.pointer_up();
                    cx.release_pointer();
                    return true;
                }
                false
            }

            // ── Keyboard: Escape clears selection ──
            Event::Key(ke)
                if phase == EventPhase::Target && ke.is_down && ke.key_code == KeyCode::Escape =>
            {
                if sel.has_selection() {
                    sel.clear();
                    return true;
                }
                false
            }

            // ── Text: Ctrl/Cmd+C copy, Ctrl/Cmd+A select all ──
            Event::Text(te) if phase == EventPhase::Target => {
                #[cfg(target_os = "macos")]
                let shortcut_mod = te.modifiers.meta;
                #[cfg(not(target_os = "macos"))]
                let shortcut_mod = te.modifiers.ctrl;

                if !shortcut_mod {
                    return false;
                }

                match te.text.as_str() {
                    "c" | "C" => {
                        if let Some(text) = sel.copy_to_clipboard() {
                            cx.clipboard_set_text(&text);
                            return true;
                        }
                        false
                    }
                    "a" | "A" => {
                        sel.select_all();
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        self.is_selectable() || self.for_widget_id.is_some()
    }

    fn focusable(&self) -> bool {
        self.is_selectable()
    }

    fn capture_eligible(&self) -> bool {
        self.is_selectable()
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.clear_read_only_text_selection();
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        // SPEC-HAL-315: Clear selection when leaving capture mode,
        // but only if selection is gated on capture (OnCapture policy).
        // For Enabled policy, selection persists across capture transitions.
        if let Some(ref mut sel) = self.text_selection {
            if sel.policy() == TextSelectionPolicy::OnCapture {
                sel.clear();
            }
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        _cx: &mut EventContext,
        _pointer_id: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        if let Some(selection) = self.text_selection.as_mut() {
            selection.pointer_up();
        }
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        let Some(sel) = self.text_selection.as_mut() else {
            return false;
        };
        if !sel.has_selection() && !sel.is_dragging() {
            return false;
        }
        sel.clear();
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "Label"
    }

    fn text_info(&self) -> Option<(&str, f32, Color)> {
        // Returns resolved font size from last layout (theme-aware)
        Some((
            &self.text,
            self.resolved_font_size,
            self.color.unwrap_or(Color::WHITE),
        ))
    }

    fn set_paint_disabled(&mut self, disabled: bool) {
        self.paint_disabled = disabled;
    }

    fn text_presentation(&self) -> Option<crate::core::widget::WidgetTextPresentation<'_>> {
        Some(crate::core::widget::WidgetTextPresentation {
            text: &self.text,
            font_size: self.resolved_font_size,
            color: self.color.unwrap_or(Color::WHITE),
            typography: self.typography.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        CancellationReason, FusedInputEvent, InteractionDomainState, PointerButtonsState,
        PointerDevice, PointerId, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::icon::IconRegistry;
    use crate::core::math::Point;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::binding_registry::BindingRegistry;
    use std::time::Instant;

    fn selection(tree: &WidgetTree, node: NodeId) -> &ReadOnlyTextSelection {
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Label>()
            .unwrap()
            .text_selection
            .as_ref()
            .unwrap()
    }

    #[test]
    fn text_selection_palette_label_paints_selected_characters() {
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        for (theme, expected) in text_selection_palette_cases() {
            let mut label = Label::new("selected label")
                .with_selection_policy(TextSelectionPolicy::Enabled);
            label.text_selection.as_mut().unwrap().select_all();
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(label), None, 0);
            let frame = RetainedSceneTestHarness::new().frame(
                &mut tree,
                root,
                Constraints::loose(Size::new(300.0, 100.0)),
                &theme,
                &mut FontManagerTextEngine::new(),
                &IconRegistry::new(&[]),
            );
            assert_painted_text_backgrounds(&frame.scene_root, "selected label", &[expected]);
        }
    }

    #[test]
    fn gesture_audit_label_pointer_cancel_ends_drag_preserves_selection() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Label::new("hello world")
                    .with_size(20.0)
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            ),
            None,
            0,
        );
        let mut text = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame(
            &mut tree,
            root,
            Constraints::loose(Size::new(300.0, 100.0)),
            &Theme::dark(),
            &mut text,
            &IconRegistry::new(&[]),
        );
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        let mut route = |tree: &mut WidgetTree, state: &mut InteractionDomainState, event| {
            router.route_input_event(
                tree,
                event,
                Some(root),
                &mut text,
                None,
                state,
                &mut bindings,
            );
        };
        route(
            &mut tree,
            &mut state,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(1.0, 5.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
        );
        route(
            &mut tree,
            &mut state,
            FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(40.0, 5.0),
                delta: (39.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState::new(),
            },
        );
        drop(route);
        let owner = tree.widget_id_for_node(root).unwrap();
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert!(selection(&tree, root).is_dragging());
        assert!(selection(&tree, root).has_selection());
        let selected = selection(&tree, root).range();
        let focused = state.focus_manager.focused();
        let keyboard_mode = state.keyboard_mode;

        router.cancel_pointer_session_with_tree(
            &mut tree,
            PointerId::MOUSE,
            CancellationReason::OverlayTakeover,
            &mut state,
        );
        assert_eq!(
            state
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert!(!selection(&tree, root).is_dragging());
        assert_eq!(selection(&tree, root).range(), selected);
        assert_eq!(state.focus_manager.focused(), focused);
        assert_eq!(state.keyboard_mode, keyboard_mode);

        router.cancel_pointer_session_with_tree(
            &mut tree,
            PointerId::MOUSE,
            CancellationReason::OverlayTakeover,
            &mut state,
        );
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(85.0, 5.0),
                delta: (45.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState::new(),
            },
            Some(root),
            &mut text,
            None,
            &mut state,
            &mut bindings,
        );
        assert!(!selection(&tree, root).is_dragging());
        assert_eq!(selection(&tree, root).range(), selected);
        assert_eq!(
            selection(&tree, root).policy(),
            TextSelectionPolicy::Enabled
        );
    }
}
