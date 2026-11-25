// Theme Toggle Demo
// Demonstrates runtime dark/light theme switching via FormBinder.
// Press F5 to toggle theme. Close the window to exit.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::theme::Theme;
use halcyon::core::widget::Widget;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::replace_widget;
use halcyon::widgets::button::Button;
use halcyon::widgets::checkbox::Checkbox;
use halcyon::widgets::progress_bar::ProgressBar;
use halcyon::widgets::radio_group::RadioId;
use halcyon::widgets::radio_group_widget::{RadioGroup, RadioGroupId};
use halcyon::widgets::slider::Slider;
use halcyon::widgets::switch::Switch;
use halcyon::widgets::text_field::TextField;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Theme Toggle Demo ===");
    println!("Press F5 to toggle between light and dark theme");
    println!("Close the window to exit");
    println!("=========================\n");

    let mut theme = Theme::dark();
    theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(
        AppConfig::new("Theme Toggle Demo")
            .with_size(480.0, 600.0)
            .with_theme(theme),
    );
    app.run(ThemeToggleScreen::new());
}

struct DemoModel {
    is_dark: bool,
}

struct ThemeToggleScreen {
    needs_rebuild: bool,
    placeholder: Label,
    model: DemoModel,
    binder: FormBinder,
}

impl ThemeToggleScreen {
    fn new() -> Self {
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            model: DemoModel { is_dark: true },
            binder: FormBinder::new(),
        }
    }
}

impl Screen for ThemeToggleScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // Model syncs from system truth — handles external theme changes (F5, OS, other screens)
        self.model.is_dark = cx.app.theme.is_dark();

        let writer = self.binder.writer();

        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        // Top padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        // Title
        let theme_name = if self.model.is_dark {
            "Dark Theme"
        } else {
            "Light Theme"
        };
        cx.tree.add_child(
            root,
            Box::new(Label::new(format!("Theme Toggle Demo — {}", theme_name))),
            None,
            1,
        );

        // Help text
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Toggle switch or press F5 to switch theme | Tab to navigate | Close the window to exit",
            )),
            None,
            2,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 3);

        // Content column
        let content = cx
            .tree
            .add_child(root, Box::new(Flex::column().with_spacing(10.0)), None, 4);

        // Button
        let btn_placeholder =
            cx.tree
                .add_child(content, Box::new(Label::new("placeholder")), None, 0);
        let btn_label =
            cx.tree
                .add_child(btn_placeholder, Box::new(Label::new("Click Me")), None, 0);
        replace_widget!(
            cx.tree,
            btn_placeholder,
            Button::new(btn_label).with_padding(12.0)
        );

        // Checkboxes
        cx.tree.add_child(
            content,
            Box::new(Checkbox::new().with_label("Enable feature")),
            None,
            1,
        );
        cx.tree.add_child(
            content,
            Box::new(Checkbox::checked(true).with_label("Auto-save")),
            None,
            2,
        );

        // Radio group
        cx.tree.add_child(
            content,
            Box::new(RadioGroup::with_labeled_options(
                RadioGroupId(0),
                &[(RadioId(0), "Option A"), (RadioId(1), "Option B")],
            )),
            None,
            3,
        );

        // Switch bound to "dark_mode" key
        let mut dark_switch = Switch::on(self.model.is_dark).with_label("Dark mode");
        dark_switch.set_binding("dark_mode", writer.clone());
        cx.tree.add_child(content, Box::new(dark_switch), None, 4);

        // Text field
        cx.tree
            .add_child(content, Box::new(TextField::new()), None, 5);

        // Slider
        cx.tree.add_child(
            content,
            Box::new(Slider::new(0.0, 100.0).with_value(65.0)),
            None,
            6,
        );

        // Progress bar
        cx.tree.add_child(
            content,
            Box::new(ProgressBar::new().with_value(0.72)),
            None,
            7,
        );

        // Status label
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 5);
        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Shadows, borders, and focus rings adapt to theme",
            )),
            None,
            6,
        );

        // Bottom padding
        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 7);

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown { key_code, .. } => match key_code {
                KeyCode::F5 => {
                    // Direct system state change — model syncs from system in build()
                    let density = cx.app.theme.density;
                    let mut theme = if self.model.is_dark {
                        Theme::light_with_density(density)
                    } else {
                        Theme::dark_with_density(density)
                    };
                    theme.button_material_policy = cx.app.theme.button_material_policy;
                    cx.set_theme(theme);
                    cx.invalidate();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let old_dark = self.model.is_dark;
        let mut new_dark = self.model.is_dark;

        self.binder.bind(cx, |b| {
            b.bool("dark_mode").sync(&mut new_dark);
        });

        if new_dark != old_dark {
            self.model.is_dark = new_dark;
            let density = cx.app.theme.density;
            let mut theme = if new_dark {
                Theme::dark_with_density(density)
            } else {
                Theme::light_with_density(density)
            };
            theme.button_material_policy = cx.app.theme.button_material_policy;
            cx.set_theme(theme);
            cx.invalidate();
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
