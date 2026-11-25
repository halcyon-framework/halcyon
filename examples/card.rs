// Card Demo — exercises Panel::card() (Panel with optional title) in
// several configurations alongside GroupBox for visual comparison.
// Static content; no live data; no Escape consumption (Demo Law).

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Alignment, FlexParams};
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::prelude::*;
use halcyon::widgets::groupbox::GroupBox;
use halcyon::widgets::panel::Panel;
use halcyon::widgets::scroll_area::ScrollArea;

struct CardDemoScreen {
    placeholder: Label,
    needs_rebuild: bool,
}

impl CardDemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            needs_rebuild: true,
        }
    }

    // ── Helpers to build a labeled container in either flavor ───────

    fn build_card(
        &self,
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
        index: i16,
        title: &str,
    ) -> NodeId {
        let card_box =
            cx.tree
                .add_child(parent, Box::new(Panel::card(NodeId(0), title)), None, index);
        let card_col = cx.tree.add_child(
            card_box,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, card_box, Panel::card(card_col, title));
        card_box
    }

    fn build_groupbox(
        &self,
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
        index: i16,
        title: &str,
    ) -> NodeId {
        let gb_box = cx.tree.add_child(
            parent,
            Box::new(GroupBox::new(NodeId(0), title)),
            None,
            index,
        );
        let gb_col = cx
            .tree
            .add_child(gb_box, Box::new(Flex::column().with_spacing(6.0)), None, 0);
        halcyon::replace_widget!(cx.tree, gb_box, GroupBox::new(gb_col, title));
        gb_box
    }

    // Returns the inner column NodeId of the most-recently-added child
    // of `container_box` (i.e. the Flex column we put inside Card/GroupBox).
    // Saves us from threading two NodeIds out of the helpers.
    fn inner_col(cx: &mut ScreenFrameContext<'_>, container_box: NodeId) -> NodeId {
        let children = cx.tree.children(container_box).to_vec();
        children[0]
    }
}

impl Screen for CardDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        // BG prepass: root must be a Panel (bare Flex causes INV-BG-PREPASS crash).
        let panel_node = cx.mount_child(Box::new(Label::new("")));
        let outer = cx.tree.add_child(
            panel_node,
            Box::new(Flex::column().with_spacing(12.0)),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, panel_node, Panel::new(outer).with_padding(16.0));

        // ScrollArea wraps the page column so long content scrolls. The page
        // Flex carries the wireframe-style page background.
        let scroll = cx
            .tree
            .add_child(outer, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));
        let page = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_spacing(16.0)
                    .with_background(cx.app.theme.colors.surfaces.base0),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, scroll, ScrollArea::new(page));

        // ─── Section 1: Card vs GroupBox Comparison ────────────────
        cx.tree.add_child(
            page,
            Box::new(
                halcyon::widgets::section_header::SectionHeader::new("Card vs GroupBox Comparison")
                    .with_secondary("identical content, two container styles"),
            ),
            None,
            0,
        );

        // Row 1: User Profile (Card | GroupBox)
        let row1 = cx
            .tree
            .add_child(page, Box::new(Flex::row().with_spacing(12.0)), None, 1);

        let card1 = self.build_card(cx, row1, 0, "User Profile");
        cx.tree.set_flex(card1, FlexParams::flexible(1.0));
        let card1_col = Self::inner_col(cx, card1);
        cx.tree
            .add_child(card1_col, Box::new(Label::new("Name")), None, 0);
        cx.tree.add_child(
            card1_col,
            Box::new(halcyon::widgets::text_field::TextField::new().with_placeholder("Jane Doe")),
            None,
            1,
        );
        cx.tree
            .add_child(card1_col, Box::new(Label::new("Email")), None, 2);
        cx.tree.add_child(
            card1_col,
            Box::new(
                halcyon::widgets::text_field::TextField::new().with_placeholder("jane@example.com"),
            ),
            None,
            3,
        );

        let gb1 = self.build_groupbox(cx, row1, 1, "User Profile");
        cx.tree.set_flex(gb1, FlexParams::flexible(1.0));
        let gb1_col = Self::inner_col(cx, gb1);
        cx.tree
            .add_child(gb1_col, Box::new(Label::new("Name")), None, 0);
        cx.tree.add_child(
            gb1_col,
            Box::new(halcyon::widgets::text_field::TextField::new().with_placeholder("Jane Doe")),
            None,
            1,
        );
        cx.tree
            .add_child(gb1_col, Box::new(Label::new("Email")), None, 2);
        cx.tree.add_child(
            gb1_col,
            Box::new(
                halcyon::widgets::text_field::TextField::new().with_placeholder("jane@example.com"),
            ),
            None,
            3,
        );

        // Row 2: Preferences (Card | GroupBox)
        let row2 = cx
            .tree
            .add_child(page, Box::new(Flex::row().with_spacing(12.0)), None, 2);

        let card2 = self.build_card(cx, row2, 0, "Preferences");
        cx.tree.set_flex(card2, FlexParams::flexible(1.0));
        let card2_col = Self::inner_col(cx, card2);
        cx.tree.add_child(
            card2_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Dark mode")),
            None,
            0,
        );
        cx.tree.add_child(
            card2_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::new().with_label("Notifications")),
            None,
            1,
        );
        cx.tree.add_child(
            card2_col,
            Box::new(halcyon::widgets::switch::Switch::on(true).with_label("Auto-save")),
            None,
            2,
        );

        let gb2 = self.build_groupbox(cx, row2, 1, "Preferences");
        cx.tree.set_flex(gb2, FlexParams::flexible(1.0));
        let gb2_col = Self::inner_col(cx, gb2);
        cx.tree.add_child(
            gb2_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Dark mode")),
            None,
            0,
        );
        cx.tree.add_child(
            gb2_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::new().with_label("Notifications")),
            None,
            1,
        );
        cx.tree.add_child(
            gb2_col,
            Box::new(halcyon::widgets::switch::Switch::on(true).with_label("Auto-save")),
            None,
            2,
        );

        // ─── Section 2: Card Styles ────────────────────────────────
        cx.tree.add_child(
            page,
            Box::new(
                halcyon::widgets::section_header::SectionHeader::new("Card Styles")
                    .with_secondary("default, custom radius, custom background"),
            ),
            None,
            3,
        );

        let styles_row = cx
            .tree
            .add_child(page, Box::new(Flex::row().with_spacing(12.0)), None, 4);

        // Default Card
        let s1 = {
            let card_box = cx.tree.add_child(
                styles_row,
                Box::new(Panel::card(NodeId(0), "Default Card")),
                None,
                0,
            );
            let col = cx.tree.add_child(
                card_box,
                Box::new(
                    Flex::column()
                        .with_spacing(6.0)
                        .with_align_items(Alignment::Stretch),
                ),
                None,
                0,
            );
            halcyon::replace_widget!(cx.tree, card_box, Panel::card(col, "Default Card"));
            cx.tree.add_child(
                col,
                Box::new(Label::new("2px corners, theme padding")),
                None,
                0,
            );
            // Button via the Button(child) + Label child + replace_widget pattern
            let btn = cx.tree.add_child(
                col,
                Box::new(Button::new(NodeId(0)).with_padding(8.0)),
                None,
                1,
            );
            let btn_label = cx
                .tree
                .add_child(btn, Box::new(Label::new("Action")), None, 0);
            halcyon::replace_widget!(
                cx.tree,
                btn,
                Button::new(btn_label).with_padding(8.0).on_click(|| {
                    println!("Default card action");
                })
            );
            card_box
        };
        cx.tree.set_flex(s1, FlexParams::flexible(1.0));

        // Custom Radius Card
        let s2 = {
            let card_box = cx.tree.add_child(
                styles_row,
                Box::new(Panel::card(NodeId(0), "Rounded").with_radius(8.0)),
                None,
                1,
            );
            let col = cx.tree.add_child(
                card_box,
                Box::new(
                    Flex::column()
                        .with_spacing(6.0)
                        .with_align_items(Alignment::Stretch),
                ),
                None,
                0,
            );
            halcyon::replace_widget!(
                cx.tree,
                card_box,
                Panel::card(col, "Rounded").with_radius(8.0)
            );
            cx.tree.add_child(
                col,
                Box::new(Label::new("8px corner radius override")),
                None,
                0,
            );
            let sl = halcyon::widgets::slider::Slider::new(0.0, 100.0)
                .with_value(50.0)
                .with_label("Smoothing")
                .with_value_display();
            cx.tree.add_child(col, Box::new(sl), None, 1);
            card_box
        };
        cx.tree.set_flex(s2, FlexParams::flexible(1.0));

        // Custom Background Card
        let s3 = {
            let bg = Color::from_srgb(0.95, 0.95, 1.0, 1.0);
            let card_box = cx.tree.add_child(
                styles_row,
                Box::new(Panel::card(NodeId(0), "Highlighted").with_background(bg)),
                None,
                2,
            );
            let col = cx.tree.add_child(
                card_box,
                Box::new(
                    Flex::column()
                        .with_spacing(6.0)
                        .with_align_items(Alignment::Stretch),
                ),
                None,
                0,
            );
            halcyon::replace_widget!(
                cx.tree,
                card_box,
                Panel::card(col, "Highlighted").with_background(bg)
            );
            cx.tree.add_child(
                col,
                Box::new(Label::new("Explicit pale-blue background")),
                None,
                0,
            );
            let sb = halcyon::widgets::spin_box::SpinBox::new()
                .with_label("Threshold")
                .with_value(42)
                .with_min(0)
                .with_max(100)
                .with_step(1);
            cx.tree.add_child(col, Box::new(sb), None, 1);
            card_box
        };
        cx.tree.set_flex(s3, FlexParams::flexible(1.0));

        // ─── Section 3: Card with Complex Content ──────────────────
        cx.tree.add_child(
            page,
            Box::new(
                halcyon::widgets::section_header::SectionHeader::new("Card with Complex Content")
                    .with_secondary("a wide card with a 3-column inner row"),
            ),
            None,
            5,
        );

        let svc = self.build_card(cx, page, 6, "Service Configuration");
        let svc_col = Self::inner_col(cx, svc);
        let svc_row = cx
            .tree
            .add_child(svc_col, Box::new(Flex::row().with_spacing(12.0)), None, 0);

        // Column 1: ComboBox + TimePicker
        let c1 = cx.tree.add_child(
            svc_row,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        cx.tree.set_flex(c1, FlexParams::flexible(1.0));
        cx.tree
            .add_child(c1, Box::new(Label::new("Environment")), None, 0);
        let cb = halcyon::widgets::combo_box::ComboBox::simple(vec![
            "Production".into(),
            "Staging".into(),
            "Development".into(),
        ]);
        cx.tree.add_child(c1, Box::new(cb), None, 1);
        cx.tree
            .add_child(c1, Box::new(Label::new("Schedule Time")), None, 2);
        let tp = halcyon::widgets::time_picker::TimePicker::new(
            halcyon::widgets::time_picker::TimeValue::new(9, 0, None),
        );
        cx.tree.add_child(c1, Box::new(tp), None, 3);

        // Column 2: Slider + RangeSlider
        let c2 = cx.tree.add_child(
            svc_row,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            1,
        );
        cx.tree.set_flex(c2, FlexParams::flexible(1.0));
        let sl = halcyon::widgets::slider::Slider::new(0.0, 100.0)
            .with_value(60.0)
            .with_label("Throughput")
            .with_value_display();
        cx.tree.add_child(c2, Box::new(sl), None, 0);
        let rs = halcyon::widgets::range_slider::RangeSlider::new(0.0, 1000.0, 100.0, 750.0)
            .with_label("Port range");
        cx.tree.add_child(c2, Box::new(rs), None, 1);

        // Column 3: Buttons
        let c3 = cx.tree.add_child(
            svc_row,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            2,
        );
        cx.tree.set_flex(c3, FlexParams::flexible(1.0));
        let apply = cx.tree.add_child(
            c3,
            Box::new(Button::new(NodeId(0)).with_padding(8.0)),
            None,
            0,
        );
        let apply_lbl = cx
            .tree
            .add_child(apply, Box::new(Label::new("Apply")), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            apply,
            Button::new(apply_lbl).with_padding(8.0).on_click(|| {
                println!("Apply");
            })
        );
        let reset = cx.tree.add_child(
            c3,
            Box::new(Button::new(NodeId(0)).with_padding(8.0)),
            None,
            1,
        );
        let reset_lbl = cx
            .tree
            .add_child(reset, Box::new(Label::new("Reset")), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            reset,
            Button::new(reset_lbl).with_padding(8.0).on_click(|| {
                println!("Reset");
            })
        );

        // ─── Section 4: Plain Panel (No Title) ────────────────────
        cx.tree.add_child(
            page,
            Box::new(
                halcyon::widgets::section_header::SectionHeader::new("Plain Panel (No Title)")
                    .with_secondary("Panel::new defaults — no title, no separator"),
            ),
            None,
            7,
        );

        let plain_panel = cx
            .tree
            .add_child(page, Box::new(Panel::new(NodeId(0))), None, 8);
        let plain_col = cx.tree.add_child(
            plain_panel,
            Box::new(Flex::column().with_spacing(6.0)),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, plain_panel, Panel::new(plain_col));
        cx.tree.add_child(
            plain_col,
            Box::new(Label::new("This is a plain Panel without a title")),
            None,
            0,
        );

        // Bottom padding
        cx.tree
            .add_child(page, Box::new(FixedSpacer::vertical(8.0)), None, 9);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Demo Law: never consume input, never consume Escape.
        false
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::replace(&mut self.needs_rebuild, false)
    }

    fn update(&mut self, _tree: &mut halcyon::core::widget_tree::WidgetTree) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}
}

fn main() {
    let mut config = AppConfig::new("Card Demo").with_size(1200.0, 800.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    let app = App::new(config);
    app.run(CardDemoScreen::new());
}
