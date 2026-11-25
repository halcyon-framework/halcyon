//! Renderer Path/Rect Parity Test
//!
//! This test compares identical square geometry rendered via rect vs path pipelines.
//!
//! Controls:
//! - Space: Cycle through display modes (Both | Primitive Only | Path Only | Blink Diff)
//!
//! Expected: In "Blink Diff" mode, NO flicker should occur (both pipelines identical).
//! Flicker = bug in path pipeline (blending, clipping, or coordinate mismatch).

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent, KeyCode};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;
use std::cell::Cell;
use std::rc::Rc;

const RECT_SIZE: f32 = 100.0;
const PADDING: f32 = 30.0;
const GAP: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum DisplayMode {
    Both,
    PrimitiveOnly,
    PathOnly,
    BlinkDiff,
}

impl DisplayMode {
    fn next(self) -> Self {
        match self {
            DisplayMode::Both => DisplayMode::PrimitiveOnly,
            DisplayMode::PrimitiveOnly => DisplayMode::PathOnly,
            DisplayMode::PathOnly => DisplayMode::BlinkDiff,
            DisplayMode::BlinkDiff => DisplayMode::Both,
        }
    }

    fn name(self) -> &'static str {
        match self {
            DisplayMode::Both => "Both (side-by-side)",
            DisplayMode::PrimitiveOnly => "Primitive Only",
            DisplayMode::PathOnly => "Path Only",
            DisplayMode::BlinkDiff => "Blink Diff (flicker=bug)",
        }
    }
}

fn main() {
    println!("=== Renderer Path/Rect Parity Test ===");
    println!();
    println!("Controls:");
    println!("  Space: Cycle display mode");
    println!();
    println!("In 'Blink Diff' mode, NO flicker should occur.");
    println!("Flicker indicates mismatch between pipelines.");
    println!("======================================");

    let app = App::new(AppConfig::new("Path/Rect Parity").with_size(800.0, 600.0));
    app.run(ParityScreen::new());
}

struct ParityScreen {
    placeholder: Label,
    mode: Rc<Cell<DisplayMode>>,
    frame_counter: Rc<Cell<u64>>,
}

impl ParityScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            mode: Rc::new(Cell::new(DisplayMode::Both)),
            frame_counter: Rc::new(Cell::new(0)),
        }
    }
}

impl Screen for ParityScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(ParityWidget::new(
            self.mode.clone(),
            self.frame_counter.clone(),
        )));
        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        if let FusedInputEvent::KeyDown { key_code, .. } = event {
            if *key_code == KeyCode::Space {
                let current = self.mode.get();
                let next = current.next();
                self.mode.set(next);
                println!("Mode: {}", next.name());
                return true;
            }
        }
        false
    }
}

struct ParityWidget {
    id: WidgetInternalId,
    mode: Rc<Cell<DisplayMode>>,
    frame_counter: Rc<Cell<u64>>,
}

impl ParityWidget {
    fn new(mode: Rc<Cell<DisplayMode>>, frame_counter: Rc<Cell<u64>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            mode,
            frame_counter,
        }
    }

    fn draw_test_pair(
        &self,
        cx: &mut PaintContext,
        x: f32,
        y: f32,
        color: Color,
        label: &str,
        show_primitive: bool,
        show_path: bool,
    ) {
        // Column 1: Primitive rect (or empty)
        if show_primitive {
            let rect = RectF::new(x, y, RECT_SIZE, RECT_SIZE);
            cx.scene.push_rect(rect, color, 1, cx.scene_key);
        }

        // Column 2: Path rect as square (radius=0 for parity test)
        if show_path {
            let path_x = if show_primitive {
                x + RECT_SIZE + GAP
            } else {
                x
            };
            let rect = RectF::new(path_x, y, RECT_SIZE, RECT_SIZE);
            cx.scene
                .push_rounded_rect(rect, color, RoundedCorners::uniform(0.0), 1, cx.scene_key);
        }

        // Label below (probe API per SPEC-HAL-159)
        #[cfg(debug_assertions)]
        {
            let label_y = y + RECT_SIZE + 5.0;
            cx.scene
                .push_text_probe((x, label_y), label, 12.0, Color::WHITE, 2, cx.scene_key);
        }
    }
}

impl WidgetDowncast for ParityWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ParityWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(800.0, 600.0))
    }

    fn paint(&self, cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // Increment frame counter
        let frame = self.frame_counter.get();
        self.frame_counter.set(frame + 1);

        let mode = self.mode.get();

        // Determine what to show based on mode
        let (show_primitive, show_path) = match mode {
            DisplayMode::Both => (true, true),
            DisplayMode::PrimitiveOnly => (true, false),
            DisplayMode::PathOnly => (false, true),
            DisplayMode::BlinkDiff => {
                // Alternate every frame
                if frame % 2 == 0 {
                    (true, false)
                } else {
                    (false, true)
                }
            }
        };

        // Dark background
        let bg_rect = RectF::new(0.0, 0.0, 800.0, 600.0);
        cx.scene.push_rect(
            bg_rect,
            Color::from_srgb(0.1, 0.1, 0.1, 1.0),
            0,
            cx.scene_key,
        );

        // Test colors - STRAIGHT alpha (shader premultiplies)
        let white_20 = Color::from_srgb(1.0, 1.0, 1.0, 0.20);
        let white_50 = Color::from_srgb(1.0, 1.0, 1.0, 0.50);
        let white_100 = Color::WHITE;
        let red_50 = Color::from_srgb(1.0, 0.0, 0.0, 0.50);

        // Row 1: Alpha 0.20
        self.draw_test_pair(
            cx,
            PADDING,
            PADDING,
            white_20,
            "alpha=0.20",
            show_primitive,
            show_path,
        );

        // Row 1: Alpha 0.50
        self.draw_test_pair(
            cx,
            PADDING + (RECT_SIZE + GAP) * 2.0 + 40.0,
            PADDING,
            white_50,
            "alpha=0.50",
            show_primitive,
            show_path,
        );

        // Row 1: Alpha 1.0
        self.draw_test_pair(
            cx,
            PADDING + (RECT_SIZE + GAP) * 4.0 + 80.0,
            PADDING,
            white_100,
            "alpha=1.0",
            show_primitive,
            show_path,
        );

        // Row 2: Red at 0.50 alpha
        let row2_y = PADDING + RECT_SIZE + 40.0;
        self.draw_test_pair(
            cx,
            PADDING,
            row2_y,
            red_50,
            "red alpha=0.50",
            show_primitive,
            show_path,
        );

        // Mode indicator and instructions (probe API per SPEC-HAL-159)
        #[cfg(debug_assertions)]
        {
            let mode_text = format!("Mode: {} | Frame: {}", mode.name(), frame);
            cx.scene.push_text_probe(
                (PADDING, 560.0),
                &mode_text,
                14.0,
                Color::WHITE,
                3,
                cx.scene_key,
            );

            cx.scene.push_text_probe(
                (PADDING, 580.0),
                "Space=cycle mode",
                12.0,
                Color::from_srgb(0.6, 0.6, 0.6, 1.0),
                3,
                cx.scene_key,
            );
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}
