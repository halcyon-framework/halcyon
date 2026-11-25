// SPEC-HAL-031: Reactive Signals Demo
// Demonstrates targeted widget invalidation via SignalStore.
// A counter signal drives three display widgets that auto-update
// without full screen rebuild.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::{Event, EventPhase, FusedInputEvent, KeyCode};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::form_binder::FormBinder;
use halcyon::model::signal_store::SignalHandle;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::button::Button;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Signals Demo (SPEC-HAL-031) ===");
    println!("Press Up/Down or click +/- to change counter");
    println!("Only subscribed widgets re-layout on change");
    println!("Close the window to exit");
    println!("==================================\n");

    let mut config = AppConfig::new("Signals Demo").with_size(400.0, 400.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(SignalsDemoScreen::new());
}

// ---------------------------------------------------------------------------
// SignalLabel: custom widget that watches a Signal<String>
// ---------------------------------------------------------------------------

struct SignalLabel {
    id: WidgetInternalId,
    signal: SignalHandle<String>,
    cached_text: String,
    font_size: Option<f32>,
    color: Option<Color>,
}

impl SignalLabel {
    fn new(signal: SignalHandle<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            signal,
            cached_text: String::new(),
            font_size: None,
            color: None,
        }
    }

    fn with_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }
}

impl WidgetDowncast for SignalLabel {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SignalLabel {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Subscribe + get current value
        self.cached_text = cx.watch(self.signal);

        let font_size = self.font_size.unwrap_or(cx.theme.typography.font_size);
        let (width, _) = cx.text_engine.measure(&self.cached_text, font_size);
        let metrics = cx.text_engine.metrics(&self.cached_text, font_size);
        let height = metrics.line_height;

        let constrained = Size::new(
            width.clamp(constraints.min_width, constraints.max_width),
            height.clamp(constraints.min_height, constraints.max_height),
        );
        LayoutResult::new(constrained, Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let font_size = self.font_size.unwrap_or(cx.theme.typography.font_size);
        let run = cx.text_engine.shape(&self.cached_text, font_size);
        let color = self.color.unwrap_or(cx.theme.colors.typography.primary);
        cx.text_engine.draw(
            &run,
            rect.x,
            rect.y,
            color,
            halcyon::core::paint::LAYER_CONTENT,
            cx.scene,
        );
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "SignalLabel"
    }
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

struct DemoModel {
    counter_signal: SignalHandle<String>,
    even_odd_signal: SignalHandle<String>,
    progress_signal: SignalHandle<String>,
    count: i32,
}

struct SignalsDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    model: Option<DemoModel>,
    binder: FormBinder,
}

impl SignalsDemoScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            model: None,
            binder: FormBinder::new(),
        }
    }

    fn update_signals(&self, cx: &mut ScreenContext<'_>) {
        if let Some(model) = &self.model {
            cx.app
                .signals
                .set(model.counter_signal, format!("Count: {}", model.count));
            cx.app.signals.set(
                model.even_odd_signal,
                if model.count % 2 == 0 {
                    "Even".to_string()
                } else {
                    "Odd".to_string()
                },
            );
            let bar_count = ((model.count.unsigned_abs() % 21) as usize).min(20);
            let bar = "#".repeat(bar_count);
            cx.app.signals.set(
                model.progress_signal,
                format!("[{:<20}] {}/20", bar, bar_count),
            );
        }
    }
}

impl Screen for SignalsDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let model = self
            .model
            .as_ref()
            .expect("model must be initialized in on_enter");

        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 0);

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("Signals Demo (SPEC-HAL-031)")),
            None,
            1,
        );

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Up/Down or +/- buttons change counter | Close the window to exit",
            )),
            None,
            2,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // Counter display — watches counter_signal
        cx.tree.add_child(
            root,
            Box::new(SignalLabel::new(model.counter_signal).with_size(20.0)),
            None,
            4,
        );

        // Even/odd display — watches even_odd_signal
        cx.tree.add_child(
            root,
            Box::new(SignalLabel::new(model.even_odd_signal)),
            None,
            5,
        );

        // Progress display — watches progress_signal
        cx.tree.add_child(
            root,
            Box::new(SignalLabel::new(model.progress_signal)),
            None,
            6,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 7);

        // Button row
        let btn_row = cx
            .tree
            .add_child(root, Box::new(Flex::row().with_spacing(8.0)), None, 8);

        // Decrement button
        let writer = self.binder.writer();
        let dec_placeholder =
            cx.tree
                .add_child(btn_row, Box::new(Label::new("placeholder")), None, 0);
        let dec_label = cx
            .tree
            .add_child(dec_placeholder, Box::new(Label::new("  -  ")), None, 0);
        replace_widget!(
            cx.tree,
            dec_placeholder,
            Button::new(dec_label)
                .with_padding(8.0)
                .with_binding("decrement", writer.clone())
        );

        // Increment button
        let inc_placeholder =
            cx.tree
                .add_child(btn_row, Box::new(Label::new("placeholder")), None, 1);
        let inc_label = cx
            .tree
            .add_child(inc_placeholder, Box::new(Label::new("  +  ")), None, 0);
        replace_widget!(
            cx.tree,
            inc_placeholder,
            Button::new(inc_label)
                .with_padding(8.0)
                .with_binding("increment", writer)
        );

        // Bottom padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(24.0)), None, 9);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown { key_code, .. } => match key_code {
                KeyCode::Up => {
                    if let Some(model) = &mut self.model {
                        model.count += 1;
                    }
                    self.update_signals(cx);
                    true
                }
                KeyCode::Down => {
                    if let Some(model) = &mut self.model {
                        model.count -= 1;
                    }
                    self.update_signals(cx);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut inc = false;
        let mut dec = false;

        self.binder.bind(cx, |b| {
            inc = b.bool("increment").take().is_some();
            dec = b.bool("decrement").take().is_some();
        });

        if let Some(model) = &mut self.model {
            if inc {
                model.count += 1;
            }
            if dec {
                model.count -= 1;
            }
            if inc || dec {
                self.update_signals(cx);
            }
        }
    }

    fn on_enter(&mut self, cx: &mut ScreenContext<'_>) {
        // Create signals once on screen entry
        let counter_signal = cx.app.signals.create("Count: 0".to_string());
        let even_odd_signal = cx.app.signals.create("Even".to_string());
        let progress_signal = cx
            .app
            .signals
            .create("[                    ] 0/20".to_string());
        self.model = Some(DemoModel {
            counter_signal,
            even_odd_signal,
            progress_signal,
            count: 0,
        });
        self.needs_rebuild = true;
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
