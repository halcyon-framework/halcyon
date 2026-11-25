// Toast Demo: demonstrates SPEC-HAL-269 Toast Notification System.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::Justification;
use halcyon::model::form_binder::FormBinder;
use halcyon::model::toast::ToastAction;
use halcyon::prelude::*;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut config = AppConfig::new("Toast Demo").with_size(500.0, 400.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(ToastDemoScreen::new());
}

struct ToastDemoScreen {
    binder: FormBinder,
    needs_rebuild: bool,
    placeholder: Label,
    undo_counter: u32,
}

impl ToastDemoScreen {
    fn new() -> Self {
        Self {
            binder: FormBinder::new(),
            needs_rebuild: true,
            placeholder: Label::new(""),
            undo_counter: 0,
        }
    }
}

impl Screen for ToastDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let writer = self.binder.writer();

        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(12.0)
                .with_padding(EdgeInsets::all(24.0)),
        ));

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("Toast Demo").with_size(18.0)),
            None,
            0,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 1);

        // Button row
        let button_row = cx.tree.add_child(
            root,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_justify_content(Justification::Start),
            ),
            None,
            2,
        );

        // "Show Toast" button
        let btn1 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 0);
        let lbl1 = cx
            .tree
            .add_child(btn1, Box::new(Label::new("Show Toast")), None, 0);
        cx.tree.replace_widget_only(
            btn1,
            Box::new(Button::new(lbl1).with_binding("show_toast", writer.clone())),
        );

        // "Show Action Toast" button
        let btn2 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 1);
        let lbl2 = cx
            .tree
            .add_child(btn2, Box::new(Label::new("Show Action Toast")), None, 0);
        cx.tree.replace_widget_only(
            btn2,
            Box::new(Button::new(lbl2).with_binding("show_action_toast", writer.clone())),
        );

        // "Spam (×5)" button
        let btn3 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 2);
        let lbl3 = cx
            .tree
            .add_child(btn3, Box::new(Label::new("Spam (×5)")), None, 0);
        cx.tree.replace_widget_only(
            btn3,
            Box::new(Button::new(lbl3).with_binding("spam", writer.clone())),
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 3);

        // Undo counter label
        let counter_text = format!("Undo count: {}", self.undo_counter);
        cx.tree
            .add_child(root, Box::new(Label::new(&counter_text)), None, 4);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut show_toast = false;
        let mut show_action_toast = false;
        let mut spam = false;

        self.binder.bind(cx, |b| {
            if b.bool("show_toast").take().is_some() {
                show_toast = true;
            }
            if b.bool("show_action_toast").take().is_some() {
                show_action_toast = true;
            }
            if b.bool("spam").take().is_some() {
                spam = true;
            }
        });

        if show_toast {
            cx.push_toast("File saved successfully.", 4000, None);
            self.needs_rebuild = true;
        }

        if show_action_toast {
            cx.push_toast(
                "Item deleted.",
                6000,
                Some(ToastAction {
                    label: "Undo".to_string(),
                }),
            );
            self.undo_counter += 1;
            self.needs_rebuild = true;
        }

        if spam {
            for i in 0..5 {
                cx.push_toast(format!("Spam toast #{}", i + 1), 4000, None);
            }
            self.needs_rebuild = true;
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
