// SPEC-HAL-227: Tutorial Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// Demonstrates:
// - 3-step linear tutorial with explicit signal-driven advancement
// - Explicit dismissal (D key)
// - No timing behavior anywhere
// - TooltipSurface used only as a dumb renderer
// - Controller is pure policy — removing it leaves UI behavior unchanged

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::math::Point;
use halcyon::core::widget::Widget;
use halcyon::model::tooltip::TooltipPayload;
use halcyon::model::tutorial::{
    TutorialCommand, TutorialController, TutorialDefinition, TutorialStep, TutorialStepId,
};
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::tooltip_surface::TooltipSurface;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Tutorial Demo (SPEC-HAL-227) ===");
    println!("S     — start tutorial");
    println!("N     — send 'next' signal");
    println!("A     — send 'ack' signal");
    println!("D     — dismiss tutorial");
    println!("Space — increment counter (works through tutorial)");
    println!("Close the window to exit");
    println!("===================================\n");

    let app = App::new(AppConfig::new("Tutorial Demo").with_size(500.0, 350.0));
    app.run(TutorialDemoScreen::new());
}

fn build_tutorial() -> TutorialDefinition {
    TutorialDefinition::new(vec![
        TutorialStep {
            id: TutorialStepId(1),
            payload: TooltipPayload::Text("Step 1: Welcome! Press N to continue.".to_string()),
            anchor: Point::new(50.0, 160.0),
            advance_signals: vec!["next".to_string()],
        },
        TutorialStep {
            id: TutorialStepId(2),
            payload: TooltipPayload::Text(
                "Step 2: Try Space to increment. Press N when done.".to_string(),
            ),
            anchor: Point::new(50.0, 160.0),
            advance_signals: vec!["next".to_string()],
        },
        TutorialStep {
            id: TutorialStepId(3),
            payload: TooltipPayload::Text("Step 3: All done! Press A to acknowledge.".to_string()),
            anchor: Point::new(50.0, 160.0),
            advance_signals: vec!["ack".to_string()],
        },
    ])
}

struct TutorialDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    controller: TutorialController,
    // Tooltip state — wired from controller commands
    tooltip_visible: bool,
    tooltip_anchor: Point,
    tooltip_payload: Option<TooltipPayload>,
    counter: u32,
}

impl TutorialDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            controller: TutorialController::new(build_tutorial()),
            tooltip_visible: false,
            tooltip_anchor: Point::zero(),
            tooltip_payload: None,
            counter: 0,
        }
    }

    /// Apply a controller command to tooltip state.
    /// This is the ONLY wiring point between controller and UI.
    fn apply_command(&mut self, cmd: TutorialCommand) {
        match cmd {
            TutorialCommand::Show {
                anchor,
                payload,
                step_id,
            } => {
                println!("Tutorial: showing step {}", step_id.0);
                self.tooltip_visible = true;
                self.tooltip_anchor = anchor;
                self.tooltip_payload = Some(payload);
            }
            TutorialCommand::Hide => {
                println!("Tutorial: hidden");
                self.tooltip_visible = false;
                self.tooltip_payload = None;
            }
        }
        self.needs_rebuild = true;
    }
}

impl Screen for TutorialDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);
        cx.tree.add_child(
            root,
            Box::new(Label::new("Tutorial Demo (SPEC-HAL-227)")),
            None,
            1,
        );
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(4.0)), None, 2);
        cx.tree
            .add_child(root, Box::new(Divider::horizontal()), None, 3);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        // Status labels
        let tutorial_status = if self.controller.is_active() {
            format!(
                "Tutorial: ACTIVE (step {})",
                self.controller.current_step_id().unwrap().0
            )
        } else {
            "Tutorial: INACTIVE".to_string()
        };
        cx.tree
            .add_child(root, Box::new(Label::new(&tutorial_status)), None, 5);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 6);

        let counter_label = format!("Counter: {} (press Space to increment)", self.counter);
        cx.tree
            .add_child(root, Box::new(Label::new(&counter_label)), None, 7);
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 8);

        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "S=start, N=next, A=ack, D=dismiss, Space=increment. Close the window to exit",
            )),
            None,
            9,
        );

        // TooltipSurface — dumb renderer, wired from controller commands
        let mut tooltip = TooltipSurface::new();
        tooltip.set_visible(self.tooltip_visible);
        tooltip.set_anchor(self.tooltip_anchor);
        if let Some(ref payload) = self.tooltip_payload {
            tooltip.set_content(payload.clone());
        }
        cx.tree.add_child(root, Box::new(tooltip), None, 10);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::TextInput { text, .. } if text == "s" || text == "S" => {
                if let Some(cmd) = self.controller.start() {
                    self.apply_command(cmd);
                }
                true
            }
            FusedInputEvent::TextInput { text, .. } if text == "n" || text == "N" => {
                if let Some(cmd) = self.controller.signal("next") {
                    self.apply_command(cmd);
                }
                true
            }
            FusedInputEvent::TextInput { text, .. } if text == "a" || text == "A" => {
                if let Some(cmd) = self.controller.signal("ack") {
                    self.apply_command(cmd);
                }
                true
            }
            FusedInputEvent::TextInput { text, .. } if text == "d" || text == "D" => {
                if let Some(cmd) = self.controller.dismiss() {
                    self.apply_command(cmd);
                }
                true
            }
            FusedInputEvent::KeyDown {
                key_code: KeyCode::Space,
                ..
            } => {
                self.counter += 1;
                println!("Counter: {}", self.counter);
                self.needs_rebuild = true;
                true
            }
            _ => false,
        }
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
