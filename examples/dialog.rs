// Dialog Demo: demonstrates Dialog modal with three configurations.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::Justification;
use halcyon::model::dialog::DialogHandle;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::widgets::dialog::{Dialog, DialogButton, DialogConfig};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut config = AppConfig::new("Dialog Demo").with_size(500.0, 400.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(DialogDemoScreen::new());
}

struct DialogDemoScreen {
    binder: FormBinder,
    needs_rebuild: bool,
    placeholder: Label,
    status: String,
    current_handle: Option<DialogHandle>,
}

impl DialogDemoScreen {
    fn new() -> Self {
        Self {
            binder: FormBinder::new(),
            needs_rebuild: true,
            placeholder: Label::new(""),
            status: "No dialog opened yet".to_string(),
            current_handle: None,
        }
    }
}

impl Screen for DialogDemoScreen {
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
            Box::new(Label::new("Dialog Demo").with_size(18.0)),
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

        // "Open Confirm Dialog" button
        let btn1 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 0);
        let lbl1 = cx
            .tree
            .add_child(btn1, Box::new(Label::new("Open Confirm Dialog")), None, 0);
        cx.tree.replace_widget_only(
            btn1,
            Box::new(Button::new(lbl1).with_binding("open_confirm", writer.clone())),
        );

        // "Open Info Dialog" button
        let btn2 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 1);
        let lbl2 = cx
            .tree
            .add_child(btn2, Box::new(Label::new("Open Info Dialog")), None, 0);
        cx.tree.replace_widget_only(
            btn2,
            Box::new(Button::new(lbl2).with_binding("open_info", writer.clone())),
        );

        // "Open Blocking Dialog" button
        let btn3 = cx
            .tree
            .add_child(button_row, Box::new(Label::new("")), None, 2);
        let lbl3 = cx
            .tree
            .add_child(btn3, Box::new(Label::new("Open Blocking Dialog")), None, 0);
        cx.tree.replace_widget_only(
            btn3,
            Box::new(Button::new(lbl3).with_binding("open_blocking", writer.clone())),
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 3);

        // Status label
        cx.tree
            .add_child(root, Box::new(Label::new(&self.status)), None, 4);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        // Check if dialog handle has result
        if let Some(ref handle) = self.current_handle {
            if !handle.is_open() {
                if let Some(key) = handle.pressed_key() {
                    self.status = format!("Dialog result: {}", key);
                } else {
                    self.status = "Dialog dismissed (no button pressed)".to_string();
                }
                self.current_handle = None;
                self.needs_rebuild = true;
            }
        }

        let mut open_confirm = false;
        let mut open_info = false;
        let mut open_blocking = false;

        self.binder.bind(cx, |b| {
            if b.bool("open_confirm").take().is_some() {
                open_confirm = true;
            }
            if b.bool("open_info").take().is_some() {
                open_info = true;
            }
            if b.bool("open_blocking").take().is_some() {
                open_blocking = true;
            }
        });

        if open_confirm {
            let handle = DialogHandle::new();
            self.current_handle = Some(handle.clone());
            cx.nav_open_modal(Box::new(Dialog::new(
                DialogConfig {
                    title: "Confirm Action".to_string(),
                    body: "Are you sure you want to proceed?".to_string(),
                    buttons: vec![
                        DialogButton {
                            label: "Cancel".to_string(),
                            key: "cancel".to_string(),
                            is_default: false,
                            is_cancel: true,
                        },
                        DialogButton {
                            label: "OK".to_string(),
                            key: "ok".to_string(),
                            is_default: true,
                            is_cancel: false,
                        },
                    ],
                    dismiss_on_escape: true,
                    dismiss_on_outside_click: true,
                },
                handle,
            )));
            self.needs_rebuild = true;
        }

        if open_info {
            let handle = DialogHandle::new();
            self.current_handle = Some(handle.clone());
            cx.nav_open_modal(Box::new(Dialog::new(
                DialogConfig {
                    title: "Information".to_string(),
                    body: "This is an informational dialog. Press Escape or click the button to close."
                        .to_string(),
                    buttons: vec![DialogButton {
                        label: "Got It".to_string(),
                        key: "got_it".to_string(),
                        is_default: true,
                        is_cancel: false,
                    }],
                    dismiss_on_escape: true,
                    dismiss_on_outside_click: false,
                },
                handle,
            )));
            self.needs_rebuild = true;
        }

        if open_blocking {
            let handle = DialogHandle::new();
            self.current_handle = Some(handle.clone());
            cx.nav_open_modal(Box::new(Dialog::new(
                DialogConfig {
                    title: "Blocking Dialog".to_string(),
                    body: "This dialog cannot be dismissed by Escape or clicking outside. Only the button closes it."
                        .to_string(),
                    buttons: vec![DialogButton {
                        label: "Acknowledge".to_string(),
                        key: "acknowledge".to_string(),
                        is_default: true,
                        is_cancel: false,
                    }],
                    dismiss_on_escape: false,
                    dismiss_on_outside_click: false,
                },
                handle,
            )));
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
