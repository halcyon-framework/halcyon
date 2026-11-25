// SPEC-HAL-241: Drag and Drop Demo
// Demonstrates list item reorder via drag-and-drop.
// DraggableItem widgets produce drag payloads and accept drops,
// communicating reorder events via FormBinder.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{
    Event, EventPhase, FusedInputEvent, PointerButton, PointerEventKind,
};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::LAYER_CONTENT;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::form_binder::{ChangeWriter, FormBinder};
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Drag & Drop Demo (SPEC-HAL-241) ===");
    println!("Drag items to reorder | Close the window to exit");
    println!("======================================\n");

    let app = App::new(AppConfig::new("Drag & Drop Demo").with_size(400.0, 500.0));
    app.run(DragDropScreen::new());
}

// ---------------------------------------------------------------------------
// DraggableItem: a labeled row that supports drag and drop
// ---------------------------------------------------------------------------

struct DraggableItem {
    id: WidgetInternalId,
    index: usize,
    label: String,
    drag_highlight: bool,
    writer: ChangeWriter,
}

impl DraggableItem {
    fn new(index: usize, label: String, writer: ChangeWriter) -> Self {
        Self {
            id: WidgetInternalId::new(),
            index,
            label,
            drag_highlight: false,
            writer,
        }
    }
}

impl WidgetDowncast for DraggableItem {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DraggableItem {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let font_size = cx.theme.typography.font_size;
        let (text_w, _) = cx.text_engine.measure(&self.label, font_size);
        let height = 36.0_f32;
        let width = constraints.max_width.min(400.0).max(constraints.min_width);
        let size = Size::new(width, height);
        let content = Size::new(text_w + 24.0, height);
        LayoutResult::new(size, content)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Background — highlight when drag-over
        let bg = if self.drag_highlight {
            cx.theme.colors.list.row_hover
        } else {
            cx.theme.colors.list.row_background
        };
        cx.scene.push_rect(rect, bg, 1, cx.scene_key);

        // Bottom border
        let border_rect = RectF::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
        cx.scene
            .push_rect(border_rect, cx.theme.colors.controls.border, 1, None);

        // Label text
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.label, font_size);
        let metrics = cx.text_engine.font_metrics(font_size);
        let text_y = rect.y + (rect.height - metrics.ascent - metrics.descent) * 0.5;
        let text_x = rect.x + 12.0;
        cx.text_engine.draw(
            &run,
            text_x,
            text_y,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT,
            cx.scene,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button == Some(PointerButton::Primary) {
                        cx.capture_pointer(PointerButton::Primary);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "DraggableItem"
    }

    // SPEC-HAL-241: Drag payload = source index
    fn drag_payload(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(self.index))
    }

    fn paint_drag_proxy(&self, cx: &mut PaintContext, rect: RectF, alpha: f32) {
        use halcyon::core::paint::{self, LAYER_OVERLAY};

        // Semi-transparent background
        let bg = cx.theme.colors.surfaces.level3.with_alpha(0.85 * alpha);
        cx.scene.push_rounded_rect(
            rect,
            bg,
            paint::RoundedCorners::uniform(4.0),
            LAYER_OVERLAY + 10,
            None,
        );

        // Drop shadow
        cx.scene.push_shadow(
            rect,
            4.0,
            paint::ShadowMode::Drop,
            (0.0, 2.0),
            0.0,
            6.0,
            paint::Color::from_srgb(0.0, 0.0, 0.0, 0.15 * alpha),
            LAYER_OVERLAY + 9,
            None,
        );

        // Label text
        let font_size = cx.theme.typography.font_size;
        let run = cx.text_engine.shape(&self.label, font_size);
        let metrics = cx.text_engine.font_metrics(font_size);
        let text_y = rect.y + (rect.height - metrics.ascent - metrics.descent) * 0.5;
        let text_x = rect.x + 12.0;
        cx.text_engine.draw(
            &run,
            text_x,
            text_y,
            cx.theme.colors.typography.primary.with_alpha(0.9 * alpha),
            LAYER_OVERLAY + 11,
            cx.scene,
        );
    }

    // SPEC-HAL-241: Accept drops of usize (item index)
    fn accepts_drop(&self, payload_type: std::any::TypeId) -> bool {
        payload_type == std::any::TypeId::of::<usize>()
    }

    fn on_drag_enter(&mut self) {
        self.drag_highlight = true;
    }

    fn on_drag_leave(&mut self) {
        self.drag_highlight = false;
    }

    fn on_drop(&mut self, payload: Box<dyn std::any::Any>) -> bool {
        if let Ok(source_index) = payload.downcast::<usize>() {
            let source_key = self.writer.intern("drag_source");
            let target_key = self.writer.intern("drag_target");
            self.writer.set_f32(source_key, *source_index as f32);
            self.writer.set_f32(target_key, self.index as f32);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

struct DragDropScreen {
    needs_rebuild: bool,
    placeholder: Label,
    items: Vec<String>,
    binder: FormBinder,
}

impl DragDropScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            items: vec![
                "Apple".to_string(),
                "Banana".to_string(),
                "Cherry".to_string(),
                "Date".to_string(),
                "Elderberry".to_string(),
                "Fig".to_string(),
                "Grape".to_string(),
                "Honeydew".to_string(),
            ],
            binder: FormBinder::new(),
        }
    }
}

impl Screen for DragDropScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let writer = self.binder.writer();

        let root = cx.mount_child(Box::new(Flex::column().with_spacing(0.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 0);

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("Drag & Drop Demo (SPEC-HAL-241)")),
            None,
            1,
        );

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new("Drag items to reorder | Close the window to exit")),
            None,
            2,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 3);

        // Item list
        for (i, item) in self.items.iter().enumerate() {
            let label = format!("  {}. {}", i + 1, item);
            cx.tree.add_child(
                root,
                Box::new(DraggableItem::new(i, label, writer.clone())),
                None,
                (4 + i) as i16,
            );
        }

        // Bottom padding
        cx.tree.add_child(
            root,
            Box::new(FixedSpacer::vertical(16.0)),
            None,
            (4 + self.items.len()) as i16,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut source: Option<usize> = None;
        let mut target: Option<usize> = None;

        self.binder.bind(cx, |b| {
            if let Some(s) = b.f32("drag_source").take() {
                source = Some(s as usize);
            }
            if let Some(t) = b.f32("drag_target").take() {
                target = Some(t as usize);
            }
        });

        if let (Some(s), Some(t)) = (source, target) {
            if s != t && s < self.items.len() && t < self.items.len() {
                let item = self.items.remove(s);
                self.items.insert(t, item);
                self.needs_rebuild = true;
            }
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
