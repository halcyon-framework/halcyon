//! Renderer Opacity Reference Demo
//!
//! Canonical reference for opacity/alpha correctness across all primitives.
//! Verifies that alpha is handled consistently through the entire pipeline.
//!
//! ## Color Space Invariants (must hold)
//!
//! 1. All CPU-side colors are LINEAR
//! 2. Alpha is always LINEAR (never gamma-corrected)
//! 3. Shaders receive straight alpha
//! 4. Shaders premultiply exactly once
//! 5. Swapchain is sRGB and performs final conversion
//!
//! ## Visual Invariant
//!
//! For the same color + alpha:
//! - Rect == RoundedRect == Path == Text overlay
//! - Order of drawing must not change perceived opacity
//! - Background must influence result identically across primitives
//!
//! ## Mathematical Invariant
//!
//! For background B and foreground F with alpha a:
//! Result = F * a + B * (1 - a)
//! No darkening, no lightening, no double premultiply.
//!
//! ## Usage
//!
//! Run with: cargo run --release --example renderer_opacity_reference

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{Event, EventPhase, FusedInputEvent};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::{Color, RoundedCorners};
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

// Layout constants
const CELL_SIZE: f32 = 80.0;
const CELL_GAP: f32 = 10.0;
const ROW_LABEL_WIDTH: f32 = 100.0;
const COL_HEADER_HEIGHT: f32 = 30.0;
const PADDING: f32 = 20.0;

// Alpha levels to test
const ALPHA_LEVELS: [f32; 5] = [1.0, 0.75, 0.5, 0.25, 0.1];

// Row names
const ROW_NAMES: [&str; 4] = ["Rect", "RoundedRect", "Path", "Text"];

fn main() {
    println!("=== Renderer Opacity Reference Demo ===");
    println!();
    println!("Color Space Invariants:");
    println!("  1. All CPU colors are LINEAR");
    println!("  2. Alpha is always LINEAR (never gamma-corrected)");
    println!("  3. Shaders receive straight alpha");
    println!("  4. Shaders premultiply exactly once");
    println!("  5. Swapchain is sRGB for final conversion");
    println!();
    println!("Visual Invariant:");
    println!("  Rect == RoundedRect == Path == Text");
    println!("  All rows should appear identical at each alpha level");
    println!();
    println!("Mathematical Invariant:");
    println!("  Result = F * a + B * (1 - a)");
    println!("  No darkening, no lightening, no double premultiply");
    println!();
    println!("=========================================");

    let window_width = PADDING * 2.0 + ROW_LABEL_WIDTH + (CELL_SIZE + CELL_GAP) * 5.0;
    let window_height = PADDING * 2.0 + COL_HEADER_HEIGHT + (CELL_SIZE + CELL_GAP) * 4.0 + 60.0;

    let app = App::new(
        AppConfig::new("Renderer Opacity Reference").with_size(window_width, window_height),
    );
    app.run(OpacityReferenceScreen::new());
}

struct OpacityReferenceScreen {
    placeholder: Label,
}

impl OpacityReferenceScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
        }
    }
}

impl Screen for OpacityReferenceScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(OpacityReferenceWidget::new()));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }
}

struct OpacityReferenceWidget {
    id: WidgetInternalId,
}

impl OpacityReferenceWidget {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }

    /// Draw a single cell at the given row/column with the specified alpha
    fn draw_cell(&self, cx: &mut PaintContext, row: usize, col: usize, alpha: f32) {
        let x = PADDING + ROW_LABEL_WIDTH + col as f32 * (CELL_SIZE + CELL_GAP);
        let y = PADDING + COL_HEADER_HEIGHT + row as f32 * (CELL_SIZE + CELL_GAP);

        // Foreground color: bright red in sRGB, with given alpha
        // Using from_srgb to ensure proper sRGB → linear conversion
        let fg_color = Color::from_srgb(1.0, 0.2, 0.2, alpha);

        // Cell rect (slightly smaller to show grid)
        let cell_rect = RectF::new(x, y, CELL_SIZE, CELL_SIZE);

        match row {
            0 => {
                // Row A: Rect (primitive pipeline)
                cx.scene.push_rect(cell_rect, fg_color, 2, cx.scene_key);
            }
            1 => {
                // Row B: RoundedRect (path pipeline with radius)
                let radius = 8.0;
                cx.scene.push_rounded_rect(
                    cell_rect,
                    fg_color,
                    RoundedCorners::uniform(radius),
                    2,
                    cx.scene_key,
                );
            }
            2 => {
                // Row C: Path (zero-radius rounded rect = square via path pipeline)
                cx.scene.push_rounded_rect(
                    cell_rect,
                    fg_color,
                    RoundedCorners::uniform(0.0),
                    2,
                    cx.scene_key,
                );
            }
            3 => {
                // Row D: Text glyphs - fill entire cell with text
                // Use large text to fill the cell area (probe API per SPEC-HAL-159)
                #[cfg(debug_assertions)]
                {
                    let text = "ABC";
                    let font_size = 32.0;
                    let text_y = y + CELL_SIZE / 2.0 - font_size / 2.0;
                    let text_x = x + 8.0;
                    cx.scene.push_text_probe(
                        (text_x, text_y),
                        text,
                        font_size,
                        fg_color,
                        2,
                        cx.scene_key,
                    );
                }
            }
            _ => {}
        }

        // Draw alpha label inside each cell (probe API per SPEC-HAL-159)
        #[cfg(debug_assertions)]
        {
            let label = format!("a={:.2}", alpha);
            let label_y = y + CELL_SIZE - 14.0;
            let label_x = x + 4.0;
            // Label uses the same alpha as the cell content
            let label_color = Color::from_srgb(1.0, 1.0, 1.0, alpha);
            cx.scene.push_text_probe(
                (label_x, label_y),
                &label,
                11.0,
                label_color,
                3,
                cx.scene_key,
            );
        }
    }
}

impl WidgetDowncast for OpacityReferenceWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for OpacityReferenceWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Background: mid-gray in sRGB space (0.5, 0.5, 0.5)
        // This is the reference background that opacity blends against
        let bg_color = Color::from_srgb(0.5, 0.5, 0.5, 1.0);
        cx.scene.push_rect(rect, bg_color, 0, cx.scene_key);

        // Column headers and row labels (probe API per SPEC-HAL-159)
        #[cfg(debug_assertions)]
        {
            // Column headers (alpha values)
            for (col, &alpha) in ALPHA_LEVELS.iter().enumerate() {
                let x = PADDING + ROW_LABEL_WIDTH + col as f32 * (CELL_SIZE + CELL_GAP);
                let y = PADDING;
                let header = format!("a={:.2}", alpha);
                cx.scene
                    .push_text_probe((x, y), &header, 12.0, Color::WHITE, 1, cx.scene_key);
            }

            // Row labels
            for (row, &name) in ROW_NAMES.iter().enumerate() {
                let x = PADDING;
                let y = PADDING
                    + COL_HEADER_HEIGHT
                    + row as f32 * (CELL_SIZE + CELL_GAP)
                    + CELL_SIZE / 2.0
                    - 6.0;
                cx.scene
                    .push_text_probe((x, y), name, 12.0, Color::WHITE, 1, cx.scene_key);
            }
        }

        // Draw all cells
        for row in 0..4 {
            for (col, &alpha) in ALPHA_LEVELS.iter().enumerate() {
                self.draw_cell(cx, row, col, alpha);
            }
        }

        // Footer with test description (probe API per SPEC-HAL-159)
        #[cfg(debug_assertions)]
        {
            let footer_y = rect.height - 40.0;
            cx.scene.push_text_probe(
                (PADDING, footer_y),
                "All rows should appear identical at each alpha level.",
                12.0,
                Color::WHITE,
                1,
                cx.scene_key,
            );
            cx.scene.push_text_probe(
                (PADDING, footer_y + 16.0),
                "Mismatch = alpha handling bug in that pipeline.",
                12.0,
                Color::from_srgb(0.8, 0.8, 0.8, 1.0),
                1,
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
