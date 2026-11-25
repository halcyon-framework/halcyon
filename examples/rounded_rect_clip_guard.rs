//! Rounded Rect Clip Regression Guard
//!
//! This example serves as a permanent regression guard to prevent re-introducing
//! the clip bleed bug from commit a0e0a1f (path pipeline routing for RoundedRect).
//!
//! ## The Bug (for posterity)
//!
//! Commit a0e0a1f changed `add_rounded_rect()` from the rect pipeline (with per-batch
//! GPU scissor clipping) to the path pipeline (full-viewport scissor + shader discard).
//! This caused hover overlays to bleed across widget boundaries.
//!
//! ## Acceptance Criteria
//!
//! 1. Hover on Button A: ONLY Button A shows hover state
//! 2. Hover on Button B: ONLY Button B shows hover state
//! 3. Hover on gap between buttons: NO button shows hover state
//! 4. NO visual bleed between adjacent buttons
//! 5. Clipping is strictly per-widget
//!
//! ## Controls
//!
//! - Mouse: Hover over buttons to test isolation
//! - Close the window to exit
//!
//! ## Contract (SPEC-RENDER-CLIP-001)
//!
//! RoundedRect primitives MUST use the rect pipeline until path pipeline
//! implements identical per-batch GPU scissor semantics.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::replace_widget;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Rounded Rect Clip Regression Guard ===");
    println!();
    println!("ACCEPTANCE CRITERIA:");
    println!("  1. Hover on Button A: ONLY Button A shows hover");
    println!("  2. Hover on Button B: ONLY Button B shows hover");
    println!("  3. Hover on gap: NO button shows hover");
    println!("  4. NO visual bleed between adjacent buttons");
    println!();
    println!("If any button other than the hovered one changes");
    println!("appearance, the regression has returned.");
    println!();
    println!("Close the window to exit.");
    println!("============================================");

    // Create a theme with exaggerated button radius for visual verification
    let mut theme = Theme::dark();
    theme.geometry.button.radius = 6.0;

    let app = App::new(
        AppConfig::new("RoundedRect Clip Guard")
            .with_size(500.0, 300.0)
            .with_theme(theme),
    );
    app.run(ClipGuardScreen::new());
}

struct ClipGuardScreen {
    needs_rebuild: bool,
    placeholder: Label,
}

impl ClipGuardScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
        }
    }
}

impl Screen for ClipGuardScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Root: centered column
        let root = cx.mount_child(Box::new(Flex::column().with_spacing(16.0)));

        // Title
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(30.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Rounded Rect Clip Regression Guard")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 2);

        // Row with two adjacent buttons (small gap)
        let button_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 3);

        // Left spacer to center the buttons
        cx.tree.add_child(
            button_row,
            Box::new(FixedSpacer::horizontal(100.0)),
            None,
            0,
        );

        // Button A
        let button_a_node =
            cx.tree
                .add_child(button_row, Box::new(Label::new("placeholder")), None, 1);
        let label_a = cx
            .tree
            .add_child(button_a_node, Box::new(Label::new("Button A")), None, 0);
        replace_widget!(
            cx.tree,
            button_a_node,
            Button::new(label_a).with_padding(12.0)
        );

        // Button B (adjacent)
        let button_b_node =
            cx.tree
                .add_child(button_row, Box::new(Label::new("placeholder")), None, 2);
        let label_b = cx
            .tree
            .add_child(button_b_node, Box::new(Label::new("Button B")), None, 0);
        replace_widget!(
            cx.tree,
            button_b_node,
            Button::new(label_b).with_padding(12.0)
        );

        // Right spacer
        cx.tree.add_child(
            button_row,
            Box::new(FixedSpacer::horizontal(100.0)),
            None,
            3,
        );

        // Status instructions
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(30.0)), None, 4);
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Hover each button. Only the hovered button should change.",
            )),
            None,
            5,
        );
        cx.tree.add_child(
            root,
            Box::new(Label::new("Visual bleed = REGRESSION BUG")),
            None,
            6,
        );

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
