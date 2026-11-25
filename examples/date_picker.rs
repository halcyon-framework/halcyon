// DatePicker Demo: exercises SPEC-HAL-238 inline date picker.

#![allow(unused_assignments)]

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::EdgeInsets;
use halcyon::core::paint::Color;
use halcyon::model::date_value::DateValue;
use halcyon::model::form_binder::FormBinder;
use halcyon::prelude::*;
use halcyon::widgets::date_picker::{
    DateMark, DateMarkKind, DatePickerState, DatePickerWidget, DatePolicy, PolicyHighlight,
};
use halcyon::widgets::date_picker_field::DatePickerField;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("DatePicker Demo").with_size(1060.0, 700.0));
    app.run(DatePickerDemoScreen::new());
}

struct DatePickerDemoScreen {
    binder: FormBinder,
    needs_rebuild: bool,
    placeholder: Label,
    sel1: Option<DateValue>,
    sel2: Option<DateValue>,
    sel3: Option<DateValue>,
    sel4: Option<DateValue>,
}

impl DatePickerDemoScreen {
    fn new() -> Self {
        Self {
            binder: FormBinder::new(),
            needs_rebuild: true,
            placeholder: Label::new(""),
            sel1: None,
            sel2: None,
            sel3: None,
            sel4: None,
        }
    }
}

impl Screen for DatePickerDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let writer = self.binder.writer();

        let root = cx.mount_child(Box::new(
            Flex::column()
                .with_spacing(16.0)
                .with_padding(EdgeInsets::all(24.0)),
        ));

        let mut slot: i16 = 0;

        // Title
        cx.tree.add_child(
            root,
            Box::new(Label::new("DatePicker Demo").with_size(18.0)),
            None,
            slot,
        );
        slot += 1;

        // Main row: two pickers side by side
        let main_row =
            cx.tree
                .add_child(root, Box::new(Flex::row().with_spacing(32.0)), None, slot);
        slot += 1;

        // =====================================================================
        // Left picker: no constraints, first_dow=0 (Sunday)
        // =====================================================================
        let left_col = cx.tree.add_child(
            main_row,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );

        cx.tree.add_child(
            left_col,
            Box::new(Label::new("Unconstrained (Sunday start)")),
            None,
            0,
        );

        let today = DateValue::new(2026, 3, 10).unwrap();
        let demo_marks = vec![
            DateMark {
                date: DateValue::new(2026, 3, 15).unwrap(),
                kind: DateMarkKind::Dot,
            },
            DateMark {
                date: DateValue::new(2026, 3, 20).unwrap(),
                kind: DateMarkKind::Dot,
            },
            DateMark {
                date: DateValue::new(2026, 3, 25).unwrap(),
                kind: DateMarkKind::Tint,
            },
        ];
        let picker1 = DatePickerWidget::new(self.sel1, None, None, 0)
            .with_today(today)
            .with_marks(demo_marks)
            .with_binding("picker_1", writer.clone());
        cx.tree.add_child(left_col, Box::new(picker1), None, 1);

        let label_text_1 = match self.sel1 {
            Some(d) => d.to_iso8601(),
            None => "(none)".to_string(),
        };
        cx.tree
            .add_child(left_col, Box::new(Label::new(&label_text_1)), None, 2);

        // =====================================================================
        // Right picker: min=today, max=today+30, first_dow=1 (Monday)
        // =====================================================================
        let right_col = cx.tree.add_child(
            main_row,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            1,
        );

        cx.tree.add_child(
            right_col,
            Box::new(Label::new("Constrained (Monday start)")),
            None,
            0,
        );

        // Compute max_date = today + 30 days
        let mut max_date = today;
        for _ in 0..30 {
            if let Some(next) = max_date.succ() {
                max_date = next;
            }
        }

        let picker2 = DatePickerWidget::new(self.sel2, Some(today), Some(max_date), 1)
            .with_today(today)
            .with_binding("picker_2", writer.clone());
        cx.tree.add_child(right_col, Box::new(picker2), None, 1);

        let label_text_2 = match self.sel2 {
            Some(d) => d.to_iso8601(),
            None => "(none)".to_string(),
        };
        cx.tree
            .add_child(right_col, Box::new(Label::new(&label_text_2)), None, 2);

        // =====================================================================
        // Policy picker: weekends tinted red, first-of-month dotted, day>25 disabled
        // =====================================================================
        let policy_col = cx.tree.add_child(
            main_row,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            2,
        );

        cx.tree.add_child(
            policy_col,
            Box::new(Label::new("Policy: weekends / dots / disabled")),
            None,
            0,
        );

        fn day_of_week(y: i32, m: u8, d: u8) -> u8 {
            let t: [u8; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
            let y = if m < 3 { y - 1 } else { y };
            ((y + y / 4 - y / 100 + y / 400 + t[(m as usize) - 1] as i32 + d as i32).rem_euclid(7))
                as u8
        }

        let policy = DatePolicy::new()
            .with_highlight(|date| {
                let dow = day_of_week(date.year, date.month, date.day);
                if dow == 0 || dow == 6 {
                    return Some(PolicyHighlight::Custom(Color::from_hex(0xC0392B)));
                }
                if date.day == 1 {
                    return Some(PolicyHighlight::Dot);
                }
                None
            })
            .with_disabled(|date| date.day > 25);

        let picker3 = DatePickerWidget::new(self.sel3, None, None, 1)
            .with_today(today)
            .with_policy(policy)
            .with_binding("picker_3", writer.clone());
        cx.tree.add_child(policy_col, Box::new(picker3), None, 1);

        let label_text_3 = match self.sel3 {
            Some(d) => d.to_iso8601(),
            None => "(none)".to_string(),
        };
        cx.tree
            .add_child(policy_col, Box::new(Label::new(&label_text_3)), None, 2);

        // =====================================================================
        // DatePickerField (popup) — separate row below inline pickers
        // =====================================================================
        let field_col =
            cx.tree
                .add_child(root, Box::new(Flex::column().with_spacing(8.0)), None, slot);
        slot += 1;

        cx.tree.add_child(
            field_col,
            Box::new(Label::new("DatePickerField (popup)")),
            None,
            0,
        );

        fn day_of_week_field(y: i32, m: u8, d: u8) -> u8 {
            let t: [u8; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
            let y = if m < 3 { y - 1 } else { y };
            ((y + y / 4 - y / 100 + y / 400 + t[(m as usize) - 1] as i32 + d as i32).rem_euclid(7))
                as u8
        }

        let field_policy = DatePolicy::new().with_highlight(|date| {
            let dow = day_of_week_field(date.year, date.month, date.day);
            if dow == 0 || dow == 6 {
                return Some(PolicyHighlight::Custom(Color::from_hex(0xC0392B)));
            }
            None
        });

        let field_initial_state = DatePickerState::new(self.sel4, None, None);
        let field = DatePickerField::new()
            .with_today(today)
            .with_state(field_initial_state)
            .with_policy(field_policy)
            .with_placeholder("Select a date...")
            .with_binding("picker_4", writer.clone());
        cx.tree.add_child(field_col, Box::new(field), None, 1);

        let label_text_4 = match self.sel4 {
            Some(d) => d.to_iso8601(),
            None => "(none)".to_string(),
        };
        cx.tree
            .add_child(field_col, Box::new(Label::new(&label_text_4)), None, 2);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        let mut new_1: Option<String> = None;
        let mut new_2: Option<String> = None;
        let mut new_3: Option<String> = None;
        let mut new_4: Option<String> = None;

        self.binder.bind(cx, |b| {
            new_1 = b.text("picker_1").take();
            new_2 = b.text("picker_2").take();
            new_3 = b.text("picker_3").take();
            new_4 = b.text("picker_4").take();
        });

        if let Some(val) = new_1 {
            self.sel1 = DateValue::from_iso8601(&val);
            self.needs_rebuild = true;
        }
        if let Some(val) = new_2 {
            self.sel2 = DateValue::from_iso8601(&val);
            self.needs_rebuild = true;
        }
        if let Some(val) = new_3 {
            self.sel3 = DateValue::from_iso8601(&val);
            self.needs_rebuild = true;
        }
        if let Some(val) = new_4 {
            self.sel4 = DateValue::from_iso8601(&val);
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
