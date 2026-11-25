//! Renderer Border Ring Invariant Test
//!
//! Visual invariant test for rounded border rendering via ring mesh triangulation.
//! Displays a grid of rounded borders with:
//! - Varying corner radii (0, 4, 8, 16)
//! - Varying border widths (1, 2, 4)
//! - Fractional pixel positions (stress test for anti-aliasing)
//!
//! Expected: All borders should have uniform thickness on all sides.
//! No seams, no side-dependent thickness variations.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

const CELL_SIZE: f32 = 80.0;
const CELL_GAP: f32 = 16.0;
const PADDING: f32 = 40.0;

// Test parameters
const RADII: [f32; 4] = [0.0, 4.0, 8.0, 16.0];
const BORDER_WIDTHS: [f32; 3] = [1.0, 2.0, 4.0];

fn main() {
    println!("=== Renderer Border Ring Invariant Test ===");
    println!();
    println!("This test verifies rounded border rendering via ring mesh.");
    println!();
    println!("Grid layout:");
    println!("  Columns: radius = 0, 4, 8, 16");
    println!("  Rows: border_width = 1, 2, 4");
    println!();
    println!("Expected:");
    println!("  - All borders have UNIFORM thickness on all sides");
    println!("  - No seams or gaps in corners");
    println!("  - No side-dependent thickness variations");
    println!();
    println!("Failure indicators:");
    println!("  - Border thicker on one side than another");
    println!("  - Visible seams at corners");
    println!("  - Missing border segments");
    println!("================================================");

    let app = App::new(AppConfig::new("Border Ring Invariant").with_size(600.0, 500.0));
    app.run(BorderInvariantScreen::new());
}

struct BorderInvariantScreen {
    placeholder: Label,
}

impl BorderInvariantScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
        }
    }
}

impl Screen for BorderInvariantScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(BorderInvariantWidget::new()));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }
}

struct BorderInvariantWidget {
    id: WidgetInternalId,
}

impl BorderInvariantWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }

    fn draw_bordered_rect(
        &self,
        cx: &mut PaintContext,
        x: f32,
        y: f32,
        radius: f32,
        border_width: f32,
    ) {
        let rect = RectF::new(x, y, CELL_SIZE, CELL_SIZE);

        // Fill with dark background
        let fill_color = Color::from_srgb(0.15, 0.15, 0.2, 1.0);
        cx.scene.push_rounded_rect(
            rect,
            fill_color,
            RoundedCorners::uniform(radius),
            1,
            cx.scene_key,
        );

        // Draw border using push_border_rounded_rect (routes through ring mesh)
        let border_color = Color::from_srgb(0.4, 0.7, 1.0, 1.0);
        cx.scene.push_border_rounded_rect(
            rect,
            border_color,
            border_width,
            RoundedCorners::uniform(radius),
            2,
            cx.scene_key,
        );
    }
}

impl WidgetDowncast for BorderInvariantWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for BorderInvariantWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Dark background
        let bg_color = Color::from_srgb(0.08, 0.08, 0.1, 1.0);
        cx.scene.push_rect(rect, bg_color, 0, cx.scene_key);

        // Column headers (radius values)
        #[cfg(debug_assertions)]
        {
            for (col, &radius) in RADII.iter().enumerate() {
                let x = PADDING + col as f32 * (CELL_SIZE + CELL_GAP);
                let header = format!("r={}", radius as i32);
                cx.scene
                    .push_text_probe((x, 12.0), &header, 12.0, Color::WHITE, 10, cx.scene_key);
            }

            // Row labels (border width values)
            for (row, &bw) in BORDER_WIDTHS.iter().enumerate() {
                let y = PADDING + row as f32 * (CELL_SIZE + CELL_GAP) + CELL_SIZE / 2.0;
                let label = format!("bw={}", bw as i32);
                cx.scene.push_text_probe(
                    (rect.width - 50.0, y),
                    &label,
                    12.0,
                    Color::WHITE,
                    10,
                    cx.scene_key,
                );
            }
        }

        // Draw the grid of bordered rectangles
        for (row, &border_width) in BORDER_WIDTHS.iter().enumerate() {
            for (col, &radius) in RADII.iter().enumerate() {
                // Use fractional positions to stress-test anti-aliasing
                // Offset by 0.3 pixels to ensure we hit fractional coordinates
                let x = PADDING + col as f32 * (CELL_SIZE + CELL_GAP) + 0.3;
                let y = PADDING + row as f32 * (CELL_SIZE + CELL_GAP) + 0.3;

                self.draw_bordered_rect(cx, x, y, radius, border_width);
            }
        }

        // Footer with test description
        #[cfg(debug_assertions)]
        {
            let footer_y = rect.height - 50.0;
            cx.scene.push_text_probe(
                (PADDING, footer_y),
                "All borders should have UNIFORM thickness on all sides.",
                12.0,
                Color::WHITE,
                10,
                cx.scene_key,
            );
            cx.scene.push_text_probe(
                (PADDING, footer_y + 16.0),
                "Varying thickness = ring mesh triangulation bug.",
                12.0,
                Color::from_srgb(0.6, 0.6, 0.6, 1.0),
                10,
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
