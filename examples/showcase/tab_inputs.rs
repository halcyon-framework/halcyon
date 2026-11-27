// Showcase — Inputs tab content.
// Widgets: button, checkbox, switch, slider, range slider, segmented control,
// combobox, spinbox, radio. Text widgets, pickers, tag input, divider, and
// section header live in tab_text_forms.rs. Live progress bars and gauges
// live in tab_display.rs.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Alignment, ControlTrailingOrder, EdgeInsets, FlexParams};
use halcyon::core::paint::Color;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::date_time::{DateTimeSelection, DateTimeValue};
use halcyon::model::date_value::DateValue;
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::model::time_value::TimeValue;
use halcyon::prelude::*;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::container::Container;
use halcyon::widgets::date_time_picker::DateTimePicker;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::flow_grid::FlowGrid;
use halcyon::widgets::gauge::{
    Gauge, GaugeConfig, GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone,
};
use halcyon::widgets::groupbox::GroupBox;
use halcyon::widgets::linear_gauge::{
    LinearGauge, LinearGaugeConfig, LinearGaugeOrientation, paint_linear_gauge,
};
use halcyon::widgets::list_view::{CellPainter, DefaultCellPainter, ListView};
use halcyon::widgets::panel::Panel;
use halcyon::widgets::progress_bar::ProgressBar;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::slider::{SliderLabelSide, ValueFieldSide};
use halcyon::widgets::sparkline::{Sparkline, SparklineConfig};
use halcyon::widgets::tab_bar::{TabBar, TabId};
use halcyon::widgets::tabular_view::Column;
use halcyon::widgets::tabular_view::column::ColumnId;
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use halcyon::widgets::{ControlAlignmentGroup, ControlCaption};
use std::collections::VecDeque;
use std::sync::Arc;
use web_time::{Duration, Instant};

use super::*;
use super::tip::Tip;

struct InputColumns {
    id: WidgetInternalId,
}

impl WidgetDowncast for InputColumns {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for InputColumns {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width;
        let side_by_side = width >= 640.0;
        let child_width = if side_by_side {
            (width - 10.0) / 2.0
        } else {
            width
        };
        let mut height: f32 = 0.0;
        let mut content_width = width;
        for (index, &child) in children.iter().enumerate() {
            let result = cx.layout_child_full(
                child,
                Constraints::new(child_width, child_width, 0.0, f32::INFINITY),
            );
            let x = if side_by_side {
                index as f32 * (child_width + 10.0)
            } else {
                0.0
            };
            let y = if side_by_side || index == 0 {
                0.0
            } else {
                height + 10.0
            };
            cx.set_child_rect(child, RectF::new(x, y, child_width, result.size.height));
            height = height.max(y + result.size.height);
            content_width = content_width.max(x + result.content_size.width);
        }
        let content = Size::new(content_width, height);
        LayoutResult::new(constraints.constrain(Size::new(width, height)), content)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }
}

fn schedule_point_available(value: DateTimeValue) -> bool {
    let seconds = value.time.hour as u32 * 3600
        + value.time.minute as u32 * 60
        + value.time.second.unwrap_or(0) as u32;
    halcyon::model::time::day_of_week(value.date) < 5
        && (9 * 3600..18 * 3600).contains(&seconds)
        && !(12 * 3600..13 * 3600).contains(&seconds)
}

fn schedule_range_available(selection: DateTimeSelection) -> bool {
    match selection {
        DateTimeSelection::DateRange { start, end } => {
            let length =
                halcyon::model::time::epoch_days(end) - halcyon::model::time::epoch_days(start) + 1;
            let first_weekday = halcyon::model::time::day_of_week(start) as i64;
            length > 0 && first_weekday < 5 && first_weekday + length <= 5
        }
        DateTimeSelection::DateTimeRange { start, end } => {
            let seconds = |value: DateTimeValue| {
                value.time.hour as u32 * 3600
                    + value.time.minute as u32 * 60
                    + value.time.second.unwrap_or(0) as u32
            };
            let from = seconds(start);
            let to = seconds(end);
            start.date == end.date
                && halcyon::model::time::day_of_week(start.date) < 5
                && from >= 9 * 3600
                && to <= 18 * 3600
                && from < to
                && !(from < 13 * 3600 && 12 * 3600 < to)
        }
        _ => false,
    }
}

fn schedule_picker(selection: DateTimeSelection) -> DateTimePicker {
    let mode = selection.mode();
    let mut picker = DateTimePicker::new(selection)
        .expect("Showcase contains a complete valid civil selection")
        .with_steps(1, 15)
        .expect("valid Showcase selection increments")
        .with_first_day_of_week(true);
    if mode.has_date() {
        picker = picker
            .with_year_bounds(2020, 2035)
            .expect("explicit Showcase civil selection bounds");
    }
    if mode.has_time() {
        if !mode.has_date() {
            picker = picker
                .with_reference_date(DateValue::new(2026, 10, 30).unwrap())
                .expect("valid explicit Showcase date context")
                .with_unavailable_time_slots(
                    halcyon::widgets::date_time_picker::UnavailableTimeSlotVisibility::Hide,
                );
        }
        picker = picker
            .with_date_time_availability(schedule_point_available)
            .expect("Showcase time policies have a complete date context");
    } else {
        picker = picker.with_date_availability(|date| halcyon::model::time::day_of_week(date) < 5);
    }
    if mode.is_range() {
        picker = picker
            .with_range_availability(schedule_range_available)
            .expect("range policy is attached only to range mode");
    }
    picker
}

#[cfg(test)]
mod command_button_tests {
    use super::*;
    use halcyon::core::app::{AppContext, EngineTicker, build_scene_with_appearance};
    use halcyon::core::event::{
        InteractionDomains, PointerButton, PointerDevice, UiInteractionPolicy, WindowId,
    };
    use halcyon::core::event_router::EventRouter;
    use halcyon::core::layout::Constraints;
    use halcyon::core::layout_transition::ActiveLayoutTransitions;
    use halcyon::core::math::{Point, RectF, Size};
    use halcyon::core::paint::LAYER_FOCUS;
    use halcyon::core::scene::SceneNode;
    use halcyon::core::text_engine::TextEngine;
    use halcyon::core::transition::TransitionLayer;
    use halcyon::core::widget_tree::WidgetTree;
    use halcyon::renderer::wgpu_renderer::WgpuRenderer;
    use halcyon::runtime::binding_registry::BindingRegistry;

    fn leaves(nodes: &[SceneNode]) -> Vec<SceneNode> {
        nodes
            .iter()
            .flat_map(|node| match node {
                SceneNode::RetainedFragment(n) => leaves(&n.children),
                SceneNode::Clip(n) => leaves(&n.children),
                SceneNode::Layer(n) => leaves(&n.children),
                SceneNode::Scroll(n) => leaves(&n.children),
                _ => vec![node.clone()],
            })
            .collect()
    }

    #[test]
    fn common_material_policy_preserves_command_button_layout_and_focus() {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        for dark in [false, true] {
            for scale in [1.0, 2.0] {
                for policy in [
                    UiInteractionPolicy::NavigationAndCapture,
                    UiInteractionPolicy::PointerDriven,
                ] {
                    let mut theme = showcase_app_config(std::iter::empty()).theme;
                    if dark {
                        theme.toggle();
                    }
                    assert_eq!(theme.button_material_policy, ButtonMaterialPolicy::SdfV3);
                    let mut app = AppContext::new(theme.clone());
                    let mut tree = WidgetTree::new();
                    let root = tree.create_root(
                        Box::new(
                            Flex::row()
                                .with_spacing(6.0)
                                .with_padding(EdgeInsets::all(20.0))
                                .with_align_items(Alignment::Start),
                        ),
                        None,
                        0,
                    );
                    ShowcaseScreen::add_command_buttons(
                        &mut ScreenFrameContext {
                            app: &mut app,
                            tree: &mut tree,
                            parent_node: root,
                        },
                        root,
                    );
                    let command_hosts = tree.children(root);
                    assert_eq!(command_hosts.len(), 3);
                    let commands = command_hosts
                        .iter()
                        .map(|&host| {
                            assert!(tree.node(host).unwrap().widget().as_any().is::<Tip>());
                            let children = tree.children(host);
                            assert_eq!(children.len(), 1);
                            assert!(tree.node(children[0]).unwrap().widget().as_any().is::<Button>());
                            children[0]
                        })
                        .collect::<Vec<_>>();
                    let [stage, apply, rollback] = <[NodeId; 3]>::try_from(commands).unwrap();
                    let mut domains = InteractionDomains::with_ui_interaction_policy(policy);
                    let mut router = EventRouter::new();
                    let mut bindings = BindingRegistry::new();
                    let mut ticker = EngineTicker::new(Instant::now());
                    let mut layer = TransitionLayer::default();
                    let mut transitions = ActiveLayoutTransitions::default();
                    let mut draw =
                        |tree: &mut WidgetTree,
                         domains: &InteractionDomains,
                         renderer: &mut WgpuRenderer| {
                            ticker.begin_frame();
                            let mut triggers = Vec::new();
                            let scene = build_scene_with_appearance(
                                tree,
                                root,
                                Constraints::tight(Size::new(360.0, 100.0)),
                                &theme,
                                renderer.text_engine_mut(),
                                &halcyon::core::icons::builtin_registry(),
                                0,
                                0.0,
                                &mut layer,
                                &mut transitions,
                                None,
                                scale,
                                None,
                                None,
                                false,
                                &mut triggers,
                                None,
                                domains,
                                WindowId::PRIMARY,
                            );
                            assert_eq!(triggers.len(), 3);
                            leaves(&scene.root)
                        };
                    let before = draw(&mut tree, &domains, &mut renderer);
                    let face = tree.get_absolute_paint_rect(stage).unwrap();
                    assert_eq!(
                        (face.x, face.y, face.height),
                        (20.0, 20.0, theme.geometry.button.height)
                    );
                    let peers = [
                        tree.get_absolute_paint_rect(apply),
                        tree.get_absolute_paint_rect(rollback),
                    ];
                    let apply_face = peers[0].unwrap();
                    let rollback_face = peers[1].unwrap();
                    assert!(face.right() < apply_face.x && apply_face.right() < rollback_face.x);
                    for (name, rect) in [("Stage", face), ("Apply", apply_face), ("Rollback", rollback_face)] {
                        let run = renderer
                            .text_engine_mut()
                            .shape_styled(name, None, theme.typography.font_size)
                            .unwrap();
                        assert!((rect.width - run.width - 2.0 * theme.geometry.button.padding_x).abs() < 0.001);
                        assert_eq!(rect.height, theme.geometry.button.height);
                        assert!(before.iter().any(|node| matches!(node, SceneNode::Text(text)
                            if text.text == name
                                && (text.position.x - (rect.x + (rect.width - run.width) * 0.5)).abs() < 0.001)));
                    }
                    let run = renderer
                        .text_engine_mut()
                        .shape_styled("Stage", None, 13.0)
                        .unwrap();
                    assert!(
                        (face.width - run.width - 2.0 * theme.geometry.button.padding_x).abs()
                            < 0.001
                    );
                    let caption = before
                        .iter()
                        .find_map(|node| match node {
                            SceneNode::Text(text) if text.text == "Stage" => Some(text.clone()),
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(caption.font_size, 13.0);
                    assert!(
                        (caption.position.x - (face.x + (face.width - run.width) * 0.5)).abs()
                            < 0.001
                    );
                    assert!(!before.iter().any(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS + 1)));
                    let owner = tree.node(stage).unwrap().id();
                    let position =
                        Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
                    assert_eq!(tree.hit_test(position.x, position.y).unwrap().widget_id, owner);
                    for down in [true, false] {
                        let time = Instant::now();
                        let event = if down {
                            FusedInputEvent::PointerDown {
                                window_id: WindowId::PRIMARY,
                                time,
                                pos: position,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary,
                                click_count: 1,
                            }
                        } else {
                            FusedInputEvent::PointerUp {
                                window_id: WindowId::PRIMARY,
                                time,
                                pos: position,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary,
                            }
                        };
                        router.route_input_event(
                            &mut tree,
                            event,
                            Some(root),
                            renderer.text_engine_mut(),
                            None,
                            domains.primary_state_mut(),
                            &mut bindings,
                        );
                        let after = draw(&mut tree, &domains, &mut renderer);
                        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                        let cue: Vec<_> = after
                            .iter()
                            .filter_map(|node| match node {
                                SceneNode::BorderRect(border)
                                    if border.z_index == LAYER_FOCUS + 1 =>
                                {
                                    Some(border)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(
                            cue.len(),
                            usize::from(policy == UiInteractionPolicy::NavigationAndCapture)
                        );
                        for cue in cue {
                            assert_eq!(cue.rect, face);
                            assert_eq!(cue.radius, 3.0);
                            assert!(cue.z_index > caption.z_index);
                        }
                        let current_caption = after
                            .iter()
                            .find_map(|node| match node {
                                SceneNode::Text(text) if text.text == "Stage" => Some(text),
                                _ => None,
                            })
                            .unwrap();
                        assert_eq!(current_caption.position, caption.position);
                        assert_eq!(tree.get_absolute_paint_rect(stage), Some(face));
                        assert_eq!(
                            [
                                tree.get_absolute_paint_rect(apply),
                                tree.get_absolute_paint_rect(rollback)
                            ],
                            peers
                        );
                    }
                }
            }
        }
    }
}

impl ShowcaseScreen {
    fn add_command_buttons(cx: &mut ScreenFrameContext<'_>, btn_row: NodeId) {
        let b1 = Self::add_tipped(
            cx,
            btn_row,
            0,
            "Stage the pending configuration without applying",
            Box::new(Button::new(NodeId(0))),
        );
        let b1_label = cx
            .tree
            .add_child(b1, Box::new(Label::new("Stage")), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            b1,
            Button::new(b1_label).on_click(|| {
                tracing::debug!("Stage clicked");
            })
        );

        let b2 = Self::add_tipped(
            cx,
            btn_row,
            1,
            "Apply the staged configuration to the target",
            Box::new(Button::new(NodeId(0))),
        );
        let b2_label = cx
            .tree
            .add_child(b2, Box::new(Label::new("Apply")), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            b2,
            Button::new(b2_label).on_click(|| {
                tracing::debug!("Apply clicked");
            })
        );

        let b3 = Self::add_tipped(
            cx,
            btn_row,
            2,
            "Revert to the last applied configuration",
            Box::new(Button::new(NodeId(0))),
        );
        let b3_label = cx
            .tree
            .add_child(b3, Box::new(Label::new("Rollback")), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            b3,
            Button::new(b3_label).on_click(|| {
                tracing::debug!("Rollback clicked");
            })
        );
    }

    fn compact_input_groupbox(content: NodeId, label: &str) -> GroupBox {
        GroupBox::new(content, label)
            .with_padding(6.0)
            .with_label_size(13.0)
    }

    /// SPEC-HAL-226A: wrap `widget` in a `Tip` carrying `tip`, add it under
    /// `parent` at `index`, and return the widget's NodeId (the Tip's child).
    /// `Tip` is non-hit-testable and layout-transparent, so the hosted control
    /// keeps its geometry and interaction.
    pub(crate) fn add_tipped(
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
        index: usize,
        tip: &str,
        widget: Box<dyn Widget>,
    ) -> NodeId {
        let t = cx
            .tree
            .add_child(parent, Box::new(Tip::new(tip)), None, index as i16);
        cx.tree.add_child(t, widget, None, 0)
    }

    pub(super) fn build_inputs_and_controls(
        &self,
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
    ) {
        let scroll = cx
            .tree
            .add_child(parent, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));

        let col = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_padding(EdgeInsets::new(10.0, 10.0, 10.0, 10.0)),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, scroll, ScrollArea::new(col));

        let columns_row = cx.tree.add_child(
            col,
            Box::new(InputColumns {
                id: WidgetInternalId::new(),
            }),
            None,
            0,
        );
        let col_left = cx.tree.add_child(
            columns_row,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        cx.tree.set_flex(
            col_left,
            FlexParams {
                grow: 1.0,
                shrink: 1.0,
                basis: 0.0,
            },
        );
        let col_right = cx.tree.add_child(
            columns_row,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            1,
        );
        cx.tree.set_flex(
            col_right,
            FlexParams {
                grow: 1.0,
                shrink: 1.0,
                basis: 0.0,
            },
        );

        // ─── GroupBox: Runtime Endpoint ───
        let num_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Runtime Endpoint")),
            None,
            0,
        );
        let num_col =
            cx.tree
                .add_child(num_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            num_box,
            Self::compact_input_groupbox(num_col, "Runtime Endpoint")
        );

        // SpinBox
        let sb = halcyon::widgets::spin_box::SpinBox::new()
            .with_label("Control port")
            .with_value(9443)
            .with_min(1)
            .with_max(65535)
            .with_step(1);
        Self::add_tipped(
            cx,
            num_col,
            0,
            "TCP port the control plane listens on",
            Box::new(sb),
        );

        let node_col = Self::add_tipped(
            cx,
            num_col,
            1,
            "Compute node that receives the control session",
            Box::new(Flex::column().with_spacing(1.0)),
        );
        cx.tree
            .add_child(node_col, Box::new(Label::new("Target node")), None, 0);
        let node_cb = halcyon::widgets::combo_box::ComboBox::simple(vec![
            "ops-gpu-17".into(),
            "ops-gpu-18".into(),
            "staging-gpu-03".into(),
            "local-sim".into(),
        ]);
        cx.tree.add_child(node_col, Box::new(node_cb), None, 1);

        // ─── GroupBox: Execution Flags ───
        let sel_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Execution Flags")),
            None,
            1,
        );
        let sel_col =
            cx.tree
                .add_child(sel_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            sel_box,
            Self::compact_input_groupbox(sel_col, "Execution Flags")
        );

        // Checkbox row
        let check_row = cx.tree.add_child(
            sel_col,
            Box::new(Flex::row().with_spacing(12.0).with_wrap()),
            None,
            0,
        );
        let cb1 = halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Audit trail");
        Self::add_tipped(
            cx,
            check_row,
            0,
            "Record every operator action to the audit log",
            Box::new(cb1),
        );
        let cb2 = halcyon::widgets::checkbox::Checkbox::new().with_label("Verbose trace");
        Self::add_tipped(
            cx,
            check_row,
            1,
            "Emit detailed per-stage trace output",
            Box::new(cb2),
        );
        let cb3 = halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Auto-retry");
        Self::add_tipped(
            cx,
            check_row,
            2,
            "Automatically retry transient failures",
            Box::new(cb3),
        );

        // Switch row
        let switch_row = cx.tree.add_child(
            sel_col,
            Box::new(Flex::row().with_spacing(12.0).with_wrap()),
            None,
            1,
        );
        let sw1 = halcyon::widgets::switch::Switch::new().with_label("Safe mode");
        Self::add_tipped(
            cx,
            switch_row,
            0,
            "Block destructive operations",
            Box::new(sw1),
        );
        let sw2 = halcyon::widgets::switch::Switch::on(true).with_label("Operator alerts");
        Self::add_tipped(
            cx,
            switch_row,
            1,
            "Notify the on-call operator on state changes",
            Box::new(sw2),
        );

        // SegmentedControl
        let sc = halcyon::widgets::segmented_control::SegmentedControl::new(
            vec![
                halcyon::widgets::segmented_control::Segment::new("Live"),
                halcyon::widgets::segmented_control::Segment::new("Replay"),
                halcyon::widgets::segmented_control::Segment::new("Canary"),
                halcyon::widgets::segmented_control::Segment::new("Locked"),
            ],
            0,
        )
        .with_fill_width(true);
        Self::add_tipped(
            cx,
            sel_col,
            2,
            "Execution mode for the session",
            Box::new(sc),
        );

        // ─── GroupBox: Envelope ───
        let range_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Envelope")),
            None,
            2,
        );
        let range_host = cx.tree.add_child(
            range_box,
            Box::new(Flex::column().with_align_items(Alignment::Stretch)),
            None,
            0,
        );
        let range_col = cx.tree.add_child(
            range_host,
            Box::new(ControlAlignmentGroup::column().with_spacing(5.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            range_box,
            Self::compact_input_groupbox(range_host, "Envelope")
        );

        // Slider
        let sl = halcyon::widgets::slider::Slider::new(0.0, 1.0)
            .with_label("Throttle")
            .with_value(0.7);
        let member = Self::add_tipped(
            cx,
            range_col,
            0,
            "Fraction of available throughput to use",
            Box::new(sl),
        );
        let host = cx.tree.node(member).unwrap().parent().unwrap();
        cx.tree.set_control_alignment_member(host, true);

        // Slider with different value
        let sl2 = halcyon::widgets::slider::Slider::new(0.0, 1.0)
            .with_label("GPU budget")
            .with_value(0.45);
        let member = Self::add_tipped(
            cx,
            range_col,
            1,
            "Share of GPU capacity allocated to this job",
            Box::new(sl2),
        );
        let host = cx.tree.node(member).unwrap().parent().unwrap();
        cx.tree.set_control_alignment_member(host, true);

        // RangeSlider
        let rs = halcyon::widgets::range_slider::RangeSlider::new(0.0, 1000.0, 200.0, 800.0)
            .with_label("Latency band");
        let member = Self::add_tipped(
            cx,
            range_col,
            2,
            "Acceptable end-to-end latency window",
            Box::new(rs),
        );
        let host = cx.tree.node(member).unwrap().parent().unwrap();
        cx.tree.set_control_alignment_member(host, true);

        let node_row = Self::add_tipped(
            cx,
            range_col,
            3,
            "Independent target node input",
            Box::new(ControlCaption::new("Target node")),
        );
        cx.tree.add_child(
            node_row,
            Box::new(
                halcyon::widgets::text_field::TextField::new()
                    .with_text("edge-router-07.ops.local")
                    .with_placeholder("host or service alias"),
            ),
            None,
            0,
        );
        let host = cx.tree.node(node_row).unwrap().parent().unwrap();
        cx.tree.set_control_alignment_member(host, true);

        let fleet_row = Self::add_tipped(
            cx,
            range_col,
            4,
            "Independent editable target fleet",
            Box::new(ControlCaption::new("Target fleet")),
        );
        cx.tree.add_child(
            fleet_row,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "prod-eu-west-gpu".into(),
                "prod-us-east-gpu".into(),
                "staging-hil".into(),
                "bench-lab".into(),
                "local-sim".into(),
            ])),
            None,
            0,
        );
        let host = cx.tree.node(fleet_row).unwrap().parent().unwrap();
        cx.tree.set_control_alignment_member(host, true);

        // ─── GroupBox: Commands ───
        let btn_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Commands")),
            None,
            3,
        );
        let btn_col =
            cx.tree
                .add_child(btn_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            btn_box,
            Self::compact_input_groupbox(btn_col, "Commands")
        );

        // Button row
        let btn_row = cx.tree.add_child(
            btn_col,
            Box::new(Flex::row().with_spacing(6.0).with_wrap()),
            None,
            0,
        );

        Self::add_command_buttons(cx, btn_row);

        // ─── GroupBox: Routing ───
        let dp_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Routing")),
            None,
            4,
        );
        let dp_col = cx
            .tree
            .add_child(dp_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            dp_box,
            Self::compact_input_groupbox(dp_col, "Routing")
        );

        // Environment (Label + ComboBox in a column)
        let env_col = Self::add_tipped(
            cx,
            dp_col,
            0,
            "Fleet the job is dispatched to",
            Box::new(Flex::column().with_spacing(1.0)),
        );
        cx.tree
            .add_child(env_col, Box::new(Label::new("Target fleet")), None, 0);
        let cb = halcyon::widgets::combo_box::ComboBox::simple(vec![
            "prod-eu-west-gpu".into(),
            "prod-us-east-gpu".into(),
            "staging-hil".into(),
            "bench-lab".into(),
            "local-sim".into(),
        ]);
        cx.tree.add_child(env_col, Box::new(cb), None, 1);

        // ─── GroupBox: Priority ───
        let rt_box = cx.tree.add_child(
            col_right,
            Box::new(GroupBox::new(NodeId(0), "Priority")),
            None,
            2,
        );
        let rt_col = cx
            .tree
            .add_child(rt_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            rt_box,
            Self::compact_input_groupbox(rt_col, "Priority")
        );

        // RadioGroup — Size selection
        let rg_options = [
            (halcyon::widgets::radio_group::RadioId(0), "Low"),
            (halcyon::widgets::radio_group::RadioId(1), "Normal"),
            (halcyon::widgets::radio_group::RadioId(2), "High"),
            (halcyon::widgets::radio_group::RadioId(3), "Critical"),
        ];
        let rg = halcyon::widgets::radio_group_widget::RadioGroup::with_labeled_options(
            halcyon::widgets::radio_group_widget::RadioGroupId(1),
            &rg_options,
        );
        Self::add_tipped(cx, rt_col, 0, "Execution priority", Box::new(rg));

        // ─── GroupBox: Calibration ───
        let sla_box = cx.tree.add_child(
            col_right,
            Box::new(GroupBox::new(NodeId(0), "Calibration")),
            None,
            0,
        );
        let sla_host = cx.tree.add_child(
            sla_box,
            Box::new(
                Flex::column()
                    .with_align_items(Alignment::Stretch)
                    .with_spacing(5.0),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            sla_box,
            Self::compact_input_groupbox(sla_host, "Calibration")
        );
        for (title, order, side) in [
            (
                "Slider · value · label",
                ControlTrailingOrder::ValueThenLabel,
                SliderLabelSide::Right,
            ),
            (
                "Label · slider · value",
                ControlTrailingOrder::LabelThenValue,
                SliderLabelSide::Left,
            ),
        ] {
            cx.tree
                .add_child(sla_host, Box::new(Label::new(title)), None, 0);
            let group = cx.tree.add_child(
                sla_host,
                Box::new(
                    ControlAlignmentGroup::column()
                        .with_spacing(5.0)
                        .with_trailing_order(order),
                ),
                None,
                0,
            );
            for (row, (label, value, tip)) in [
                ("Overlay alpha", 42.0, "Opacity of the diagnostic overlay"),
                ("Thermal cap", 75.0, "Maximum sustained thermal load"),
            ]
            .into_iter()
            .enumerate()
            {
                let slider = halcyon::widgets::slider::Slider::new(0.0, 100.0)
                    .with_value(value)
                    .with_label(label)
                    .with_label_side(side)
                    .with_value_display()
                    .with_value_field_side(ValueFieldSide::Right);
                let member = Self::add_tipped(cx, group, row, tip, Box::new(slider));
                let host = cx.tree.node(member).unwrap().parent().unwrap();
                cx.tree.set_control_alignment_member(host, true);
            }
        }

        // ─── GroupBox: Connection Profile ───
        let text_box = cx.tree.add_child(
            col_right,
            Box::new(GroupBox::new(NodeId(0), "Connection Profile")),
            None,
            1,
        );
        let text_col = cx.tree.add_child(
            text_box,
            Box::new(Flex::column().with_spacing(5.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            text_box,
            Self::compact_input_groupbox(text_col, "Connection Profile")
        );

        let tf = halcyon::widgets::text_field::TextField::new()
            .with_text("edge-router-07.ops.local")
            .with_placeholder("host or service alias");
        Self::add_tipped(
            cx,
            text_col,
            0,
            "Host or service alias for the control endpoint",
            Box::new(tf),
        );

        let tf2 = halcyon::widgets::text_field::TextField::new()
            .with_text("10.42.18.77:9443")
            .with_placeholder("control endpoint");
        Self::add_tipped(
            cx,
            text_col,
            1,
            "Control endpoint address and port",
            Box::new(tf2),
        );

        cx.tree
            .add_child(text_col, Box::new(Label::new("Operator secret")), None, 3);
        let password = halcyon::widgets::text_field::TextField::new()
            .with_text("rotate-this-secret")
            .with_placeholder("password or access token")
            .with_password(true)
            .with_max_len(Some(20));
        Self::add_tipped(
            cx,
            text_col,
            4,
            "SecureEntry masks painted text; copy/cut return an empty clipboard payload",
            Box::new(password),
        );

        let ta = halcyon::widgets::text_area::TextArea::new()
            .with_smooth_caret_prototype(true)
            .with_text("[session]\nprofile = \"canary\"\nmax_inflight = 128\ntimeout_ms = 2500\ntrace_sample = 0.10\nrollback_on_error = true")
            .with_min_height(88.0)
            .with_max_height(220.0);
        Self::add_tipped(cx, text_col, 5, "Session profile (TOML)", Box::new(ta));

        // ─── GroupBox: Schedule & Identity ───
        let pickers_box = cx.tree.add_child(
            col_left,
            Box::new(GroupBox::new(NodeId(0), "Schedule & Identity")),
            None,
            5,
        );
        let pickers_col = cx.tree.add_child(
            pickers_box,
            Box::new(Flex::column().with_spacing(5.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            pickers_box,
            Self::compact_input_groupbox(pickers_col, "Schedule & Identity")
        );

        // Single row: Start Date | Schedule Time | Theme Color
        // (Environment ComboBox column dropped — ComboBox stays in Inputs.)
        let pickers_row = cx.tree.add_child(
            pickers_col,
            Box::new(Flex::row().with_spacing(8.0)),
            None,
            0,
        );

        // Start Date — copy of tab_inputs.rs:324-331
        let sd_col = Self::add_tipped(
            cx,
            pickers_row,
            0,
            "Date the deployment is scheduled",
            Box::new(Flex::column().with_spacing(1.0)),
        );
        cx.tree
            .add_child(sd_col, Box::new(Label::new("Deploy date")), None, 0);
        let dpf = schedule_picker(DateTimeSelection::Date(
            DateValue::new(2026, 10, 30).expect("valid Showcase deployment date"),
        ));
        cx.tree.add_child(sd_col, Box::new(dpf), None, 1);

        // Schedule Time — copy of tab_inputs.rs:333-349
        let st_col = cx.tree.add_child(
            pickers_row,
            Box::new(
                Flex::column()
                    .with_spacing(1.0)
                    .with_align_items(halcyon::core::layout::Alignment::Start),
            ),
            None,
            1,
        );
        let cutover_tip = cx.tree.add_child(
            st_col,
            Box::new(Tip::new("Scheduled cutover time")),
            None,
            0,
        );
        cx.tree
            .add_child(cutover_tip, Box::new(Label::new("Cutover")), None, 0);
        let tp = schedule_picker(DateTimeSelection::Time(TimeValue::new(14, 30, None)));
        Self::add_tipped(cx, st_col, 1, "Scheduled cutover time", Box::new(tp));

        // Theme Color — copy of tab_inputs.rs:351-366
        let tc_col = cx.tree.add_child(
            pickers_row,
            Box::new(
                Flex::column()
                    .with_spacing(1.0)
                    .with_align_items(halcyon::core::layout::Alignment::Start),
            ),
            None,
            2,
        );
        let severity_tip = cx.tree.add_child(
            tc_col,
            Box::new(Tip::new("Severity indicator color")),
            None,
            0,
        );
        cx.tree
            .add_child(severity_tip, Box::new(Label::new("Severity")), None, 0);
        let cpf = halcyon::widgets::color_picker_field::ColorPickerField::new()
            .with_color(Color::from_hex(0xE3A322));
        Self::add_tipped(cx, tc_col, 1, "Severity indicator color", Box::new(cpf));

        let start = DateValue::new(2026, 11, 2).expect("valid Showcase range start");
        let end = DateValue::new(2026, 11, 6).expect("valid Showcase range end");
        let start_time = DateTimeValue::new(start, TimeValue::new(9, 7, None))
            .expect("valid off-step Showcase time");
        for (index, (label, value, tip)) in [
            (
                "Date + time",
                DateTimeSelection::DateTime(start_time),
                "Working days, 09:00–18:00; 12:00–13:00 occupied. Existing 09:07 is preserved with 15-minute choices.",
            ),
            (
                "Date range",
                DateTimeSelection::DateRange { start, end },
                "Inclusive dates. Choose Start or End independently; Apply confirms the complete available range.",
            ),
            (
                "Date + time range",
                DateTimeSelection::DateTimeRange {
                    start: start_time,
                    end: DateTimeValue::new(start, TimeValue::new(11, 30, None))
                        .expect("valid Showcase range endpoint"),
                },
                "End is excluded. The application checks the entire interval, including occupied time.",
            ),
        ].into_iter().enumerate() {
            let column = Self::add_tipped(
                cx, pickers_col, index + 1, tip,
                Box::new(Flex::column().with_spacing(1.0)),
            );
            cx.tree.add_child(column, Box::new(Label::new(label)), None, 0);
            cx.tree.add_child(column, Box::new(schedule_picker(value)), None, 1);
        }

        // ─── GroupBox: Classification ───
        let tags_box = cx.tree.add_child(
            col_right,
            Box::new(GroupBox::new(NodeId(0), "Classification")),
            None,
            3,
        );
        let tags_col = cx.tree.add_child(
            tags_box,
            Box::new(Flex::column().with_spacing(5.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            tags_box,
            Self::compact_input_groupbox(tags_col, "Classification")
        );

        // TagInput — copy of tab_inputs.rs:399-434
        let tag_items = vec![
            halcyon::widgets::tag_input::TagItem {
                id: 0,
                label: "gpu".to_string(),
                group: None,
                enabled: true,
            },
            halcyon::widgets::tag_input::TagItem {
                id: 1,
                label: "release".to_string(),
                group: None,
                enabled: true,
            },
            halcyon::widgets::tag_input::TagItem {
                id: 2,
                label: "safety".to_string(),
                group: None,
                enabled: true,
            },
            halcyon::widgets::tag_input::TagItem {
                id: 3,
                label: "telemetry".to_string(),
                group: None,
                enabled: true,
            },
            halcyon::widgets::tag_input::TagItem {
                id: 4,
                label: "audit".to_string(),
                group: None,
                enabled: true,
            },
        ];
        let ti = halcyon::widgets::tag_input::TagInput::new()
            .with_items(tag_items)
            .with_selection(&[0, 1, 2]);
        Self::add_tipped(
            cx,
            tags_col,
            0,
            "Classification tags applied to the run",
            Box::new(ti),
        );

        // ─── GroupBox: Validation ───
        let hs_box = cx.tree.add_child(
            col_right,
            Box::new(GroupBox::new(NodeId(0), "Validation")),
            None,
            4,
        );
        let hs_col = cx
            .tree
            .add_child(hs_box, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            hs_box,
            Self::compact_input_groupbox(hs_col, "Validation")
        );

        // SectionHeader — copy of tab_inputs.rs:483-485
        let sh = halcyon::widgets::section_header::SectionHeader::new("Ready for dry run")
            .with_secondary("3 checks passed, 1 warning acknowledged");
        cx.tree.add_child(hs_col, Box::new(sh), None, 0);

        // Bottom padding
        cx.tree
            .add_child(col, Box::new(FixedSpacer::vertical(2.0)), None, 1);
    }
}

#[cfg(test)]
mod bounded_text_area_tests {
    use super::*;
    use halcyon::core::app::{
        AppContext, EngineTicker, build_scene_with_gesture_sessions, layout_tree,
    };
    use halcyon::core::event::{
        InteractionDomains, KeyCode, KeyState, KeyboardAuthorityMode, KeyboardKeyReservation,
        Modifiers, PhysicalKeyCode, PointerButton, PointerDevice, RawButtonState, RawInputEvent,
        WheelDelta, WindowId,
    };
    use halcyon::core::event_router::EventRouter;
    use halcyon::core::input_fusion::InputFusion;
    use halcyon::core::layout::Constraints;
    use halcyon::core::layout_transition::ActiveLayoutTransitions;
    use halcyon::core::math::{Point, RectF, Size};
    use halcyon::core::rect_authority::{LayoutAuthorityGuard, UpdateAuthorityGuard};
    use halcyon::core::transition::TransitionLayer;
    use halcyon::core::widget_tree::WidgetTree;
    use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
    use halcyon::runtime::binding_registry::BindingRegistry;
    use halcyon::widgets::screen_host::ScreenHost;
    use halcyon::widgets::text_area::TextArea;

    struct MountedForm {
        tree: WidgetTree,
        root: NodeId,
        theme: Theme,
        text: FontManagerTextEngine,
        ticker: EngineTicker,
        now: Instant,
        domains: InteractionDomains,
        router: EventRouter,
        fusion: InputFusion,
        bindings: BindingRegistry,
        transition_layer: TransitionLayer,
        transitions: ActiveLayoutTransitions,
        size: Size,
    }

    impl MountedForm {
        fn new(size: Size) -> Self {
            Self::with_screen(size, ShowcaseScreen::new())
        }

        fn with_screen(size: Size, mut screen: ShowcaseScreen) -> Self {
            let config = showcase_app_config(std::iter::empty());
            let mut app = AppContext::new(config.theme.clone());
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(ScreenHost::new()), None, 0);
            screen.build(&mut ScreenFrameContext {
                app: &mut app,
                tree: &mut tree,
                parent_node: root,
            });
            let now = Instant::now();
            let mut ticker = EngineTicker::new(now);
            let mut text = FontManagerTextEngine::new_with_system_ui();
            ticker.begin_frame_at(now);
            {
                let _layout = LayoutAuthorityGuard::enter();
                layout_tree(
                    &mut tree,
                    root,
                    Constraints::tight(size),
                    &config.theme,
                    &mut text,
                    None,
                );
            }
            Self {
                tree,
                root,
                theme: config.theme,
                text,
                ticker,
                now,
                domains: InteractionDomains::with_ui_interaction_policy(
                    config.ui_interaction_policy,
                ),
                router: EventRouter::new(),
                fusion: InputFusion::new(),
                bindings: BindingRegistry::new(),
                transition_layer: TransitionLayer::default(),
                transitions: ActiveLayoutTransitions::default(),
                size,
            }
        }

        fn widget<T: Widget>(&self, node: NodeId) -> &T {
            self.tree
                .node(node)
                .unwrap()
                .widget()
                .as_any()
                .downcast_ref()
                .unwrap()
        }

        fn rect(&self, node: NodeId) -> RectF {
            self.tree.get_absolute_paint_rect(node).unwrap()
        }

        fn visible_rect(&self, node: NodeId) -> RectF {
            let rect = self.rect(node);
            let (x, y) = self.tree.cumulative_scroll_offset(node);
            RectF::new(rect.x - x, rect.y - y, rect.width, rect.height)
        }

        fn frame(&mut self) {
            self.now += Duration::from_millis(16);
            let frame = self.ticker.begin_frame_at(self.now);
            {
                let _update = UpdateAuthorityGuard::enter();
                self.tree.update_scroll_animations(frame.dt.as_secs_f32());
            }
            let focus_reveal = self.router.take_pending_focus_reveal();
            build_scene_with_gesture_sessions(
                &mut self.tree,
                self.root,
                Constraints::tight(self.size),
                &self.theme,
                &mut self.text,
                &halcyon::core::icons::builtin_registry(),
                self.ticker.frame_time_ms(frame.now),
                frame.dt.as_secs_f32(),
                &mut self.transition_layer,
                &mut self.transitions,
                focus_reveal,
                1.0,
                None,
                &mut Vec::new(),
                None,
                &mut self.domains,
                WindowId::PRIMARY,
                &mut self.router,
                &mut self.fusion,
            );
        }

        fn route(&mut self, raw: RawInputEvent) {
            self.router
                .set_frame_time_ms(self.ticker.frame_time_ms(self.now));
            let domain = self.domains.primary_state_mut();
            let mut events = Vec::new();
            self.fusion.process_raw_with_gesture_sessions(
                raw,
                domain.focus_manager.interaction_domain_id(),
                &mut domain.gesture_registry,
                &self.tree,
                None,
                &mut events,
            );
            self.fusion.flush_with_gesture_sessions(&mut events);
            for event in events {
                self.router.route_session_input_event(
                    &mut self.tree,
                    event,
                    Some(self.root),
                    &mut self.text,
                    None,
                    domain,
                    &mut self.bindings,
                );
            }
        }

        fn key(
            &mut self,
            code: KeyCode,
            physical: PhysicalKeyCode,
            modifiers: Modifiers,
            text: Option<&str>,
        ) {
            let pressed_at = self.now;
            let key = |state, time| RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time,
                key_code: code,
                physical_key: physical,
                reservation: text.map(|_| KeyboardKeyReservation::Clipboard),
                state,
                repeat: false,
                modifiers,
            };
            self.route(key(KeyState::Down, pressed_at));
            if let Some(text) = text {
                self.route(RawInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: pressed_at,
                    text: text.into(),
                    modifiers,
                });
            }
            self.now += Duration::from_micros(1);
            self.route(key(KeyState::Up, self.now));
        }

        fn shortcut(
            &mut self,
            physical: PhysicalKeyCode,
            character: char,
            text: &str,
            shift: bool,
        ) {
            self.key(
                KeyCode::Character(character),
                physical,
                Modifiers {
                    meta: cfg!(target_os = "macos"),
                    ctrl: !cfg!(target_os = "macos"),
                    shift,
                    ..Modifiers::default()
                },
                Some(text),
            );
        }

        fn replace_document(&mut self, text: &str) {
            self.shortcut(PhysicalKeyCode::KeyA, 'a', "a", false);
            self.route(RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: self.now,
                text: text.into(),
                modifiers: Modifiers::default(),
            });
        }

        fn wheel(&mut self, position: Point, pixels: f32) {
            self.route(RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: self.now,
                device: PointerDevice::Mouse,
                delta: WheelDelta::Pixels(0.0, pixels),
                position,
            });
        }

        fn settle_caret(&mut self, node: NodeId) {
            for _ in 0..70 {
                self.frame();
                let area = self.widget::<TextArea>(node);
                let visual = area.visual_scroll_y_for_test();
                if visual == area.scroll_y_for_test()
                    && let Some(caret) =
                        area.caret_rect_for_test(area.effective_vertical_offset_for_test().round())
                {
                    let viewport = area.debug_text_viewport_for_test();
                    assert!(
                        caret.x >= viewport.x - 0.5
                            && caret.right() <= viewport.right() + 0.5
                            && caret.y >= viewport.y - 0.5
                            && caret.bottom() <= viewport.bottom() + 0.5,
                        "settled caret {caret:?} leaves TextArea viewport {viewport:?} at scroll {visual}",
                    );
                    return;
                }
            }
            panic!("TextArea visual offset and visible caret did not settle");
        }
    }

    #[test]
    fn narrow_inputs_breakpoint_is_the_allocated_content_width() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(InputColumns { id: WidgetInternalId::new() }), None, 0,
        );
        let left = tree.add_child(root, Box::new(FixedSpacer::vertical(40.0)), None, 0);
        let right = tree.add_child(root, Box::new(FixedSpacer::vertical(60.0)), None, 1);
        let theme = Theme::default();
        let mut text = FontManagerTextEngine::new_with_system_ui();
        let mut ticker = EngineTicker::new(Instant::now());
        for width in [639.0, 640.0, 1000.0, 320.0] {
            ticker.begin_frame();
            let _layout = LayoutAuthorityGuard::enter();
            layout_tree(
                &mut tree, root, Constraints::tight(Size::new(width, 240.0)),
                &theme, &mut text, None,
            );
            let left = tree.get_rect(left).unwrap();
            let right = tree.get_rect(right).unwrap();
            if width < 640.0 {
                assert_eq!(left, RectF::new(0.0, 0.0, width, 40.0));
                assert_eq!(right, RectF::new(0.0, 50.0, width, 60.0));
            } else {
                let column_width = (width - 10.0) / 2.0;
                assert_eq!(left, RectF::new(0.0, 0.0, column_width, 40.0));
                assert_eq!(right, RectF::new(column_width + 10.0, 0.0, column_width, 60.0));
            }
        }
    }

    #[test]
    fn narrow_inputs_keep_mounted_controls_and_constraint_owned_columns() {
        use halcyon::widgets::spin_box::SpinBox;
        use halcyon::widgets::text_field::TextField;

        let mut form = MountedForm::with_screen(
            Size::new(780.0, 900.0),
            ShowcaseScreen::new_dedicated(),
        );
        form.frame();
        let mut nodes = Vec::new();
        let mut pending = vec![form.root];
        while let Some(node) = pending.pop() {
            nodes.push(node);
            pending.extend(form.tree.children(node));
        }
        let columns = *nodes.iter().find(|&&node| {
            form.tree.node(node).unwrap().widget().as_any().is::<InputColumns>()
        }).unwrap();
        let spin = *nodes.iter().find(|&&node| {
            form.tree.node(node).unwrap().widget().as_any().is::<SpinBox>()
        }).unwrap();
        let editor = *nodes.iter().find(|&&node| {
            form.tree.node(node).unwrap().widget().as_any().downcast_ref::<TextField>()
                .is_some_and(|field| field.text() == "10.42.18.77:9443")
        }).unwrap();
        let owners: Vec<_> = nodes.iter().map(|&node| form.tree.widget_id_for_node(node)).collect();
        let click = |form: &mut MountedForm, position| {
            for state in [RawButtonState::Pressed, RawButtonState::Released] {
                form.route(RawInputEvent::PointerButton {
                    window_id: WindowId::PRIMARY,
                    time: form.now,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    state,
                    position,
                });
            }
        };
        let geom = &form.theme.geometry;
        let font_size = form.theme.typography.input_font_size;
        let value_width = (form.text.measure("65535", font_size).0 + geom.text_field.padding_x * 2.0)
            .max(form.text.measure("999", font_size).0 + geom.text_field.padding_x * 2.0)
            .max(geom.spin_box.value_width);
        let spin_rect = form.visible_rect(spin);
        let increment = Point::new(
            spin_rect.x + value_width + geom.spin_box.button_width * 1.5,
            spin_rect.center().y,
        );
        click(&mut form, increment);
        assert_eq!(form.widget::<SpinBox>(spin).value(), 9444);
        let editor_center = form.visible_rect(editor).center();
        click(&mut form, editor_center);
        form.route(RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: form.now,
            text: "x".into(),
            modifiers: Modifiers::default(),
        });
        let edited = form.widget::<TextField>(editor).text().to_owned();
        assert!(edited.contains('x'));

        for size in [
            Size::new(320.0, 430.0),
            Size::new(390.0, 844.0),
            Size::new(664.0, 900.0),
            Size::new(780.0, 900.0),
        ] {
            form.size = size;
            form.tree.mark_layout_dirty(form.root);
            form.frame();
            let children = form.tree.children(columns);
            assert_eq!(children.len(), 2);
            let left = form.tree.get_rect(children[0]).unwrap();
            let right = form.tree.get_rect(children[1]).unwrap();
            let width = form.rect(columns).width;
            if width >= 640.0 {
                assert!((left.width - right.width).abs() < 0.01);
                assert!((right.x - left.right() - 10.0).abs() < 0.01);
                assert_eq!(left.y, right.y);
            } else {
                assert_eq!(left.x, right.x);
                assert!((left.width - width).abs() < 0.01);
                assert!((right.width - width).abs() < 0.01);
                assert!((right.y - left.bottom() - 10.0).abs() < 0.01);
            }
            for &column in &children {
                for pair in form.tree.children(column).windows(2) {
                    let first = form.rect(pair[0]);
                    let second = form.rect(pair[1]);
                    assert!(second.y >= first.bottom(), "groups overlap at {size:?}");
                }
            }
            assert_eq!(form.widget::<SpinBox>(spin).value(), 9444);
            assert_eq!(form.widget::<TextField>(editor).text(), edited);
            assert_eq!(nodes.iter().map(|&node| form.tree.widget_id_for_node(node)).collect::<Vec<_>>(), owners);
        }
    }

    #[test]
    fn text_area_bounded_growth() {
        let mut form = MountedForm::new(Size::new(780.0, 760.0));
        let mut pending = vec![form.root];
        let mut editors = Vec::new();
        let mut groups = Vec::new();
        while let Some(node) = pending.pop() {
            let widget = form.tree.node(node).unwrap().widget().as_any();
            if widget.is::<TextArea>() {
                editors.push(node);
            }
            if let Some(group) = widget.downcast_ref::<GroupBox>() {
                if matches!(group.label.as_str(), "Classification" | "Validation") {
                    groups.push((group.label.as_str(), node));
                }
            }
            pending.extend(form.tree.children(node));
        }
        let [editor] = <[NodeId; 1]>::try_from(editors).unwrap();
        groups.sort_by_key(|&(label, _)| label);
        let [(_, classification), (_, validation)] =
            <[(&str, NodeId); 2]>::try_from(groups).unwrap();
        let mut ancestor = form.tree.parent(editor).unwrap();
        assert!(
            form.tree
                .node(ancestor)
                .unwrap()
                .widget()
                .as_any()
                .is::<Tip>()
        );
        while !form
            .tree
            .node(ancestor)
            .unwrap()
            .widget()
            .as_any()
            .is::<ScrollArea>()
        {
            ancestor = form
                .tree
                .parent(ancestor)
                .expect("the Session profile is inside the form ScrollArea");
        }
        let scroll = ancestor;
        let owner = form.tree.widget_id_for_node(editor).unwrap();
        let instance = form.widget::<TextArea>(editor).id();
        assert_eq!(
            form.widget::<TextArea>(editor)
                .debug_height_policy_for_test(),
            (88.0, Some(220.0), None)
        );
        let initial = form.widget::<TextArea>(editor).text().to_owned();
        assert!(
            initial.starts_with("[session]\n") && initial.ends_with("rollback_on_error = true")
        );
        let initial_height = form.rect(editor).height;
        assert!(
            (88.0..220.0).contains(&initial_height),
            "initial TextArea height {initial_height}"
        );

        let position = form.visible_rect(editor).center();
        assert_eq!(
            form.tree
                .hit_test(position.x, position.y)
                .unwrap()
                .widget_id,
            owner
        );
        for state in [RawButtonState::Pressed, RawButtonState::Released] {
            form.route(RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time: form.now,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state,
                position,
            });
        }
        assert_eq!(
            form.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert_eq!(
            form.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        form.key(
            KeyCode::End,
            PhysicalKeyCode::End,
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            None,
        );
        assert_eq!(
            form.widget::<TextArea>(editor).cursor_byte_pos_for_test(),
            initial.len()
        );

        let form_geometry = |form: &MountedForm| {
            (
                form.rect(classification),
                form.rect(validation),
                form.widget::<ScrollArea>(scroll).content_extent_height(),
            )
        };
        let enter = |state, repeat, time| RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            state,
            repeat,
            modifiers: Modifiers::default(),
        };
        let enter_downs = 32;
        let mut expected = initial.clone();
        let mut previous_height = initial_height;
        let mut previous_content_height = form
            .widget::<TextArea>(editor)
            .debug_scroll_state_for_test()
            .0
            .height;
        let mut first_capped = None;
        let mut grew_before_cap = false;
        for press in 1..=enter_downs {
            form.route(enter(KeyState::Down, press > 1, form.now));
            expected.push('\n');
            form.frame();
            let area = form.widget::<TextArea>(editor);
            assert_eq!(
                area.text(),
                expected,
                "Enter down {press} must add one newline"
            );
            assert_eq!(area.cursor_byte_pos_for_test(), expected.len());
            assert_eq!(area.id(), instance);
            assert_eq!(form.tree.widget_id_for_node(editor), Some(owner));
            assert_eq!(
                form.domains.primary_state().focus_manager.focused(),
                Some(owner)
            );
            assert_eq!(
                form.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            let height = form.rect(editor).height;
            assert!(
                (previous_height..=220.0).contains(&height),
                "held Enter changed height from {previous_height} to {height}"
            );
            previous_height = height;
            grew_before_cap |= height > initial_height && height < 220.0;
            let content_height = area.debug_scroll_state_for_test().0.height;
            if height == 220.0 {
                let layout = form_geometry(&form);
                let (first_layout, _, _) =
                    first_capped.get_or_insert((layout, area.scroll_y_for_test(), press));
                assert_eq!(*first_layout, layout);
                assert!(
                    content_height > previous_content_height,
                    "Enter down {press} must grow the content at capped height"
                );
            }
            previous_content_height = content_height;
        }
        assert!(grew_before_cap);
        let (capped_layout, first_capped_scroll, first_capped_press) =
            first_capped.expect("held Enter should reach 220 logical pixels");
        assert!(
            first_capped_press < enter_downs,
            "repeat must continue after reaching the cap"
        );
        form.settle_caret(editor);
        let area = form.widget::<TextArea>(editor);
        let (entered_content, viewport, _, _) = area.debug_scroll_state_for_test();
        assert!(entered_content.height > viewport.height);
        assert!(area.visual_scroll_y_for_test() > first_capped_scroll);
        assert_eq!(area.text(), expected);
        assert_eq!(form.rect(editor).height, 220.0);
        form.route(enter(KeyState::Up, false, form.now));
        form.frame();
        let area = form.widget::<TextArea>(editor);
        assert_eq!(area.text(), expected);
        assert_eq!(area.cursor_byte_pos_for_test(), expected.len());
        assert_eq!(form.rect(editor).height, 220.0);

        let long = "λ β profile\n".repeat(96) + "Ω ready";
        let short = "λ ready";
        form.replace_document(&long);
        form.settle_caret(editor);
        let area = form.widget::<TextArea>(editor);
        assert_eq!(area.text(), long);
        assert_eq!(area.cursor_byte_pos_for_test(), long.len());
        assert!(area.debug_scroll_state_for_test().0.height > entered_content.height);
        assert_eq!(form.rect(editor).height, 220.0);
        assert_eq!(form_geometry(&form), capped_layout);

        form.replace_document(short);
        form.settle_caret(editor);
        assert_eq!(form.widget::<TextArea>(editor).text(), short);
        assert_eq!(
            form.rect(editor).height,
            88.0,
            "short replacement must shrink to the minimum"
        );
        assert_eq!(
            form.widget::<TextArea>(editor)
                .debug_scroll_state_for_test()
                .2,
            (0.0, 0.0)
        );
        assert_eq!(
            form.widget::<TextArea>(editor).visual_scroll_y_for_test(),
            0.0
        );
        assert_eq!(
            form.widget::<TextArea>(editor).visual_scroll_x_for_test(),
            0.0
        );

        form.shortcut(PhysicalKeyCode::KeyZ, 'z', "z", false);
        form.settle_caret(editor);
        let area = form.widget::<TextArea>(editor);
        assert_eq!(area.text(), long);
        assert_eq!(area.cursor_byte_pos_for_test(), long.len());
        assert_eq!(area.selection_range_for_test(), (0, long.len()));
        assert_eq!(form.rect(editor).height, 220.0);
        let internal_before = area.scroll_y_for_test();
        let outer_before = form.widget::<ScrollArea>(scroll).target_scroll_y();
        let area_rect = form.visible_rect(editor);
        let inside = Point::new(area_rect.x + 20.0, area_rect.y + 24.0);
        assert_eq!(
            form.tree.hit_test(inside.x, inside.y).unwrap().widget_id,
            owner
        );
        form.wheel(inside, 48.0);
        form.frame();
        let area = form.widget::<TextArea>(editor);
        assert!(
            area.scroll_y_for_test() < internal_before
                && area.visual_scroll_y_for_test() < internal_before
        );
        assert_eq!(
            form.widget::<ScrollArea>(scroll).target_scroll_y(),
            outer_before
        );
        assert_eq!(area.text(), long);
        assert_eq!(area.cursor_byte_pos_for_test(), long.len());
        assert_eq!(area.selection_range_for_test(), (0, long.len()));

        let viewport = form.rect(scroll);
        let outside = Point::new(viewport.x + 3.0, viewport.y + 12.0);
        assert!(!area_rect.contains(outside));
        let outer_extent = form.widget::<ScrollArea>(scroll).content_extent_height();
        assert!(
            outer_extent > viewport.height + outer_before,
            "outer form has no remaining scroll range: extent {outer_extent}, viewport {viewport:?}, offset {outer_before}, Validation {:?}",
            form.rect(validation),
        );
        let movement = (form.visible_rect(validation).bottom() - viewport.bottom() + 8.0).max(32.0);
        form.wheel(outside, -movement);
        assert_eq!(
            form.domains.primary_state().scroll_capture_owner,
            form.tree.widget_id_for_node(scroll),
        );
        assert!(
            form.widget::<ScrollArea>(scroll).target_scroll_y() > outer_before,
            "wheel reached the form but did not advance its target offset",
        );
        for _ in 0..32 {
            form.frame();
        }
        assert!(form.widget::<ScrollArea>(scroll).visual_scroll_y() > outer_before);
        for downstream in [classification, validation] {
            let visible = form.visible_rect(downstream);
            assert!(
                visible.y >= viewport.y && visible.bottom() <= viewport.bottom(),
                "downstream block {visible:?} remains outside form viewport {viewport:?}"
            );
        }

        form.shortcut(PhysicalKeyCode::KeyZ, 'Z', "Z", true);
        form.settle_caret(editor);
        let area = form.widget::<TextArea>(editor);
        assert_eq!(area.text(), short);
        assert_eq!(area.cursor_byte_pos_for_test(), short.len());
        assert_eq!(area.selection_range_for_test(), (short.len(), short.len()));
        assert_eq!(form.rect(editor).height, 88.0);

        let regular_width = form.rect(editor).width;
        for width in [660.0, 960.0] {
            form.size.width = width;
            form.frame();
            assert_eq!(form.widget::<TextArea>(editor).id(), instance);
            assert_eq!(form.tree.widget_id_for_node(editor), Some(owner));
            assert_eq!(
                form.domains.primary_state().focus_manager.focused(),
                Some(owner)
            );
            assert_eq!(
                form.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            assert_eq!(form.widget::<TextArea>(editor).text(), short);
            for node in [editor, classification, validation] {
                let rect = form.rect(node);
                assert!(
                    rect.x.is_finite()
                        && rect.y.is_finite()
                        && rect.width.is_finite()
                        && rect.height.is_finite()
                );
                assert!(rect.width > 0.0 && rect.height > 0.0);
                assert!(
                    rect.bottom()
                        <= form.rect(scroll).y
                            + form.widget::<ScrollArea>(scroll).content_extent_height()
                );
            }
            assert_eq!(form.rect(editor).height, 88.0);
            assert_eq!(form.rect(editor).width < regular_width, width < 780.0);
        }
    }
}
