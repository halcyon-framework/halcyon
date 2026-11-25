// Notification Demo: exercises SPEC-HAL-269 (toast stack) and SPEC-HAL-270 (notification log).

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::Justification;
use halcyon::model::form_binder::FormBinder;
use halcyon::model::notification::NotificationLevel;
use halcyon::model::toast::ToastAction;
use halcyon::prelude::*;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut config = AppConfig::new("Notification Demo").with_size(800.0, 520.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(NotificationDemoScreen::new());
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

struct NotificationDemoScreen {
    binder: FormBinder,
    needs_rebuild: bool,
    placeholder: Label,
    undo_counter: u32,
    notification_counter: u32,
    spam_counter: u32,
    notification_log: Option<NotificationLogWidget>,
}

impl NotificationDemoScreen {
    fn new() -> Self {
        Self {
            binder: FormBinder::new(),
            needs_rebuild: true,
            placeholder: Label::new(""),
            undo_counter: 0,
            notification_counter: 0,
            spam_counter: 0,
            notification_log: None,
        }
    }
}

impl Screen for NotificationDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let writer = self.binder.writer();

        if self.notification_log.is_none() {
            self.notification_log = Some(NotificationLogWidget::new(Rc::clone(
                &cx.app.notification_manager,
            )));
        }

        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(12.0)
                .with_padding(EdgeInsets::all(24.0)),
        ));

        // Title
        let mut slot: i16 = 0;
        cx.tree.add_child(
            root,
            Box::new(Label::new("Notification Demo").with_size(18.0)),
            None,
            slot,
        );
        slot += 1;

        // Main row: left controls + right log
        let main_row = cx.tree.add_child(
            root,
            Box::new(
                Flex::row()
                    .with_spacing(16.0)
                    .with_justify_content(Justification::Start),
            ),
            None,
            slot,
        );

        // =====================================================================
        // Left column: controls
        // =====================================================================
        let left_col = cx.tree.add_child(
            main_row,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );

        let mut left_slot: i16 = 0;

        // Toast button row
        let toast_row = cx.tree.add_child(
            left_col,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_justify_content(Justification::Start),
            ),
            None,
            left_slot,
        );
        left_slot += 1;

        // [Show Toast]
        let btn = cx
            .tree
            .add_child(toast_row, Box::new(Label::new("")), None, 0);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Show Toast")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("show_toast", writer.clone())),
        );

        // [Show Action Toast]
        let btn = cx
            .tree
            .add_child(toast_row, Box::new(Label::new("")), None, 1);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Show Action Toast")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("show_action_toast", writer.clone())),
        );

        // [Spam (x5)]
        let btn = cx
            .tree
            .add_child(toast_row, Box::new(Label::new("")), None, 2);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Spam (\u{00d7}5)")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("spam", writer.clone())),
        );

        // Spacer
        cx.tree.add_child(
            left_col,
            Box::new(FixedSpacer::vertical(8.0)),
            None,
            left_slot,
        );
        left_slot += 1;

        // Notification button row
        let notif_row = cx.tree.add_child(
            left_col,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_justify_content(Justification::Start),
            ),
            None,
            left_slot,
        );
        left_slot += 1;

        // [Log Info]
        let btn = cx
            .tree
            .add_child(notif_row, Box::new(Label::new("")), None, 0);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Log Info")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("log_info", writer.clone())),
        );

        // [Log Warning]
        let btn = cx
            .tree
            .add_child(notif_row, Box::new(Label::new("")), None, 1);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Log Warning")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("log_warning", writer.clone())),
        );

        // [Log Error]
        let btn = cx
            .tree
            .add_child(notif_row, Box::new(Label::new("")), None, 2);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Log Error")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("log_error", writer.clone())),
        );

        // [Log + Toast]
        let btn = cx
            .tree
            .add_child(left_col, Box::new(Label::new("")), None, left_slot);
        let lbl = cx
            .tree
            .add_child(btn, Box::new(Label::new("Log + Toast")), None, 0);
        cx.tree.replace_widget_only(
            btn,
            Box::new(Button::new(lbl).with_binding("log_and_toast", writer.clone())),
        );
        left_slot += 1;

        // Spacer
        cx.tree.add_child(
            left_col,
            Box::new(FixedSpacer::vertical(16.0)),
            None,
            left_slot,
        );
        left_slot += 1;

        // Undo counter label
        let counter_text = format!("Undo count: {}", self.undo_counter);
        cx.tree.add_child(
            left_col,
            Box::new(Label::new(&counter_text)),
            None,
            left_slot,
        );

        // =====================================================================
        // Right column: notification log via NotificationLogWidget
        // =====================================================================
        let right_col = cx.tree.add_child(
            main_row,
            Box::new(Flex::column().with_spacing(6.0)),
            None,
            1,
        );

        // Header
        cx.tree.add_child(
            right_col,
            Box::new(Label::new("Notifications").with_size(13.0)),
            None,
            0,
        );

        // Mount the notification log panel
        self.notification_log
            .as_mut()
            .unwrap()
            .build_into(right_col, cx, 1);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut show_toast = false;
        let mut show_action_toast = false;
        let mut spam = false;
        let mut log_info = false;
        let mut log_warning = false;
        let mut log_error = false;
        let mut log_and_toast = false;

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
            if b.bool("log_info").take().is_some() {
                log_info = true;
            }
            if b.bool("log_warning").take().is_some() {
                log_warning = true;
            }
            if b.bool("log_error").take().is_some() {
                log_error = true;
            }
            if b.bool("log_and_toast").take().is_some() {
                log_and_toast = true;
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
            self.spam_counter += 1;
            for i in 0..5 {
                cx.push_toast(
                    format!("Spam #{} toast {}/5", self.spam_counter, i + 1),
                    4000,
                    None,
                );
            }
            self.needs_rebuild = true;
        }

        if log_info {
            cx.push_notification(
                NotificationLevel::Info,
                format!("Info event #{}", self.notification_counter),
                None::<String>,
                None,
                now_ms(),
            );
            self.notification_counter += 1;
            self.needs_rebuild = true;
        }

        if log_warning {
            cx.push_notification(
                NotificationLevel::Warning,
                format!("Warning event #{}", self.notification_counter),
                None::<String>,
                None,
                now_ms(),
            );
            self.notification_counter += 1;
            self.needs_rebuild = true;
        }

        if log_error {
            cx.push_notification(
                NotificationLevel::Error,
                format!("Error event #{}", self.notification_counter),
                None::<String>,
                None,
                now_ms(),
            );
            self.notification_counter += 1;
            self.needs_rebuild = true;
        }

        if log_and_toast {
            cx.push_notification(
                NotificationLevel::Info,
                "Synced successfully.",
                None::<String>,
                None,
                now_ms(),
            );
            cx.push_toast("Synced.", 3000, None);
            self.needs_rebuild = true;
        }

        // Delegate dismiss/clear to NotificationLogWidget
        if let Some(ref mut log) = self.notification_log {
            if log.flush_pending_actions(cx) {
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
