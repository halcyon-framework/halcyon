// SPEC-HAL-128 Stage 4: Gesture Cancellation & Teardown Tests
//
// These tests verify the cancellation and teardown invariants:
//   INV-PC4 - Escape Cancels Everything
//   INV-PC5 - Lossless Cancellation
//   INV-PC7 - Overlay Supremacy
//   INV-GA-CANCEL - Cancellation Beats Priority
//
// Stage 4 scope: cancellation, teardown, overlay supremacy.

use halcyon::core::animation::Timeline;
use halcyon::core::app::{EngineTicker, layout_tree};
use halcyon::core::event::{
    CancellationReason, Event, EventPhase, FusedInputEvent, GestureKind, GestureRegistry,
    InteractionDomainState, KeyCode, Modifiers, PointerButton, PointerButtonsState, PointerDevice,
    PointerEventKind, PointerId, PointerType, SessionQualifiedInput, TouchPhase, WheelDelta,
    WindowId,
};
use halcyon::core::event::{KeyboardAuthorityMode, RawButtonState, RawInputEvent};
use halcyon::core::event_router::EventRouter;
use halcyon::core::icon::IconRegistry;
use halcyon::core::input_fusion::InputFusion;
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::layout_transition::InterpolatedVisualRects;
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::rect_authority::{LayoutAuthorityGuard, PaintAuthorityGuard, UpdateAuthorityGuard};
use halcyon::core::scene::{SceneBuilder, SceneNode};
use halcyon::core::theme::Theme;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetDirty, WidgetTree};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::prelude::{Button, Flex, Label, ScrollArea};
use halcyon::runtime::binding_registry::BindingRegistry;
use halcyon::utils::id::{TreeWidgetId, WidgetInternalId};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct TouchButtonFixture {
    tree: WidgetTree,
    root: NodeId,
    buttons: [NodeId; 2],
    clicks: [Rc<Cell<usize>>; 2],
    domain: InteractionDomainState,
    router: EventRouter,
    fusion: InputFusion,
    text: FontManagerTextEngine,
    bindings: BindingRegistry,
    window: WindowId,
    ticker: EngineTicker,
}

impl TouchButtonFixture {
    fn new() -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::row()), None, 0);
        let clicks = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
        let buttons = std::array::from_fn(|index| {
            let count = clicks[index].clone();
            tree.add_child(
                root,
                Box::new(Button::with_text("Reader").on_click(move || {
                    count.set(count.get() + 1);
                })),
                None,
                index as i16,
            )
        });
        Self::with_tree(tree, root, buttons, clicks, Size::new(320.0, 80.0))
    }

    fn with_tree(
        tree: WidgetTree,
        root: NodeId,
        buttons: [NodeId; 2],
        clicks: [Rc<Cell<usize>>; 2],
        viewport: Size,
    ) -> Self {
        let mut fixture = Self {
            tree,
            root,
            buttons,
            clicks,
            domain: InteractionDomainState::new(),
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            text: FontManagerTextEngine::new(),
            bindings: BindingRegistry::new(),
            window: WindowId::PRIMARY,
            ticker: EngineTicker::new(Instant::now()),
        };
        fixture.layout(viewport);
        fixture
    }

    fn layout(&mut self, viewport: Size) {
        self.ticker.begin_frame();
        let _layout = LayoutAuthorityGuard::enter();
        let theme = Theme::dark();
        let layout_root = self.tree.root().unwrap();
        layout_tree(
            &mut self.tree,
            layout_root,
            Constraints::tight(viewport),
            &theme,
            &mut self.text,
            None,
        );
    }

    fn center(&self, index: usize) -> Point {
        let rect = self
            .tree
            .get_absolute_paint_rect(self.buttons[index])
            .unwrap();
        Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
    }

    fn input(&mut self, raw: RawInputEvent) -> Vec<halcyon::core::event::SessionQualifiedInput> {
        let mut events = Vec::new();
        self.ingest(raw, &mut events);
        self.fusion.flush_with_gesture_sessions(&mut events);
        for event in &events {
            self.deliver(event.clone());
        }
        events
    }

    fn ingest(&mut self, raw: RawInputEvent, events: &mut Vec<SessionQualifiedInput>) {
        self.fusion.process_raw_with_gesture_sessions(
            raw,
            self.domain.focus_manager.interaction_domain_id(),
            &mut self.domain.gesture_registry,
            &self.tree,
            None,
            events,
        );
    }

    fn deliver(&mut self, event: SessionQualifiedInput) {
        self.router.route_session_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            &mut self.domain,
            &mut self.bindings,
        );
    }

    fn touch(
        &mut self,
        contact_id: u64,
        phase: halcyon::core::event::TouchPhase,
        pos: Point,
    ) -> Option<PointerId> {
        self.touch_at(contact_id, phase, pos, Instant::now())
    }

    fn touch_at(
        &mut self, contact_id: u64, phase: TouchPhase, pos: Point, time: Instant,
    ) -> Option<PointerId> {
        self.input(RawInputEvent::Touch {
            window_id: self.window,
            time,
            contact_id,
            phase,
            pos,
        })
        .iter()
        .find_map(|event| match event.event() {
            FusedInputEvent::TouchDown { pointer_id, .. }
            | FusedInputEvent::TouchMove { pointer_id, .. }
            | FusedInputEvent::TouchUp { pointer_id, .. }
            | FusedInputEvent::TouchCancel { pointer_id, .. } => Some(*pointer_id),
            _ => None,
        })
    }

    fn advance_kinetic_scroll(&mut self, now: Instant) -> bool {
        let frame = self.ticker.begin_frame_at(now);
        let _update = UpdateAuthorityGuard::enter();
        self.tree.advance_kinetic_scroll_for_frame(&frame)
    }
}

struct TouchScrollContent {
    id: WidgetInternalId,
    extent: Size,
    children: Vec<RectF>,
    requested_scroll_y: Option<f32>,
}

impl WidgetDowncast for TouchScrollContent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TouchScrollContent {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        assert_eq!(children.len(), self.children.len());
        for (&child, &rect) in children.iter().zip(&self.children) {
            cx.layout_child(
                child,
                Constraints::tight(Size::new(rect.width, rect.height)),
            );
            cx.set_child_rect(child, rect);
        }
        LayoutResult::new(constraints.constrain(self.extent), self.extent)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn take_requested_scroll_y(&mut self) -> Option<f32> {
        self.requested_scroll_y.take()
    }
}

impl TouchButtonFixture {
    fn scrollable(viewport: Size, extent: Size, horizontal: bool) -> (Self, NodeId) {
        let mut tree = WidgetTree::new();
        let screen = tree.create_root(
            Box::new(TouchScrollContent {
                id: WidgetInternalId::new(),
                extent: viewport,
                children: vec![RectF::new(0.0, 0.0, viewport.width, viewport.height)],
                requested_scroll_y: None,
            }),
            None,
            0,
        );
        let root = tree.add_child(
            screen,
            Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(horizontal)),
            None,
            0,
        );
        let content = tree.add_child(
            root,
            Box::new(TouchScrollContent {
                id: WidgetInternalId::new(),
                extent,
                children: vec![
                    RectF::new(20.0, 40.0, 86.0, 24.0),
                    RectF::new(120.0, 40.0, 86.0, 24.0),
                ],
                requested_scroll_y: None,
            }),
            None,
            0,
        );
        tree.node_mut(root)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(content);
        let clicks = std::array::from_fn(|_| Rc::new(Cell::new(0)));
        let buttons = std::array::from_fn(|index| {
            let clicks = clicks[index].clone();
            tree.add_child(
                content,
                Box::new(Button::with_text("Tap").on_click(move || clicks.set(clicks.get() + 1))),
                None,
                1,
            )
        });
        (
            Self::with_tree(tree, root, buttons, clicks, viewport),
            content,
        )
    }

    fn push_content_child(
        &mut self,
        parent: NodeId,
        rect: RectF,
        widget: Box<dyn Widget>,
    ) -> NodeId {
        self.tree
            .node_mut(parent)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<TouchScrollContent>()
            .unwrap()
            .children
            .push(rect);
        self.tree.add_child(parent, widget, None, 2)
    }

    fn add_nested_scroll(
        &mut self,
        content: NodeId,
        viewport: Size,
        extent: Size,
        horizontal: bool,
    ) -> NodeId {
        let nested = self.push_content_child(
            content,
            RectF::new(110.0, 100.0, 180.0, 120.0),
            Box::new(ScrollArea::new_unwired().enable_horizontal_scrollbar(horizontal)),
        );
        let child = self.tree.add_child(
            nested,
            Box::new(TouchScrollContent {
                id: WidgetInternalId::new(),
                extent,
                children: Vec::new(),
                requested_scroll_y: None,
            }),
            None,
            0,
        );
        self.area_mut(nested).wire_content(child);
        self.layout(viewport);
        nested
    }

    fn area(&self, node: NodeId) -> &ScrollArea {
        self.tree
            .node(node)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<ScrollArea>()
            .unwrap()
    }

    fn area_mut(&mut self, node: NodeId) -> &mut ScrollArea {
        self.tree
            .node_mut(node)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
    }

    fn owner(&self, node: NodeId) -> TreeWidgetId {
        self.tree.node(node).unwrap().id()
    }

    fn captured(&self, pointer: PointerId) -> Option<TreeWidgetId> {
        self.domain.pointer_capture_manager.current_capture(pointer)
    }

    fn wheel(&mut self, pos: Point, delta: f32) {
        self.input(RawInputEvent::PointerWheel {
            window_id: self.window,
            time: Instant::now(),
            delta: WheelDelta::Pixels(0.0, delta),
            position: pos,
            device: PointerDevice::Mouse,
        });
    }

    fn mouse_button(&mut self, state: RawButtonState, position: Point) {
        self.input(RawInputEvent::PointerButton {
            window_id: self.window,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position,
        });
    }

    fn render(
        &mut self,
        dpr: f32,
    ) -> (
        halcyon::core::scene::SceneGraph,
        Vec<halcyon::core::widget_tree::OverlayHitRegion>,
    ) {
        let (scene, hits) = {
            let _paint = PaintAuthorityGuard::enter();
            halcyon::core::app::paint_tree(
            &self.tree,
            self.tree.root().unwrap(),
            &Theme::dark(),
            &mut self.text,
            &IconRegistry::new(&[]),
            0,
            &Timeline::new(),
            &InterpolatedVisualRects::default(),
            dpr,
            None,
            &mut Vec::new(),
            None,
            self.domain.focus_manager.focused(),
            self.domain.keyboard_mode,
            )
        };
        self.tree.set_overlay_hit_regions(hits.clone());
        (scene, hits)
    }
}

fn has_scroll_node(nodes: &[SceneNode], offset: (f32, f32)) -> bool {
    nodes.iter().any(|node| match node {
        SceneNode::Scroll(scroll) => {
            scroll.offset == offset || has_scroll_node(&scroll.children, offset)
        }
        SceneNode::Clip(clip) => has_scroll_node(&clip.children, offset),
        _ => false,
    })
}

#[test]
fn hal397_tap_and_pan_share_one_original_session_across_controls() {
    use halcyon::widgets::{
        checkbox::Checkbox, combo_box::ComboBox, radio_group::RadioId,
        radio_group_widget::{RadioGroup, RadioGroupId}, slider::Slider, spin_box::SpinBox,
        switch::Switch, text_area::TextArea, text_field::TextField,
    };

    for (width, dpr) in [(320.0, 2.0), (390.0, 3.0)] {
        let viewport = Size::new(width, 170.0);
        let controls: Vec<Box<dyn Widget>> = vec![
            Box::new(TextField::new().with_text("é already here")),
            Box::new(TextArea::new().with_text("One\nTwo")),
            Box::new(ComboBox::simple(vec!["A".into(), "B".into()])),
            Box::new(Checkbox::new().with_label("Consent")),
            Box::new(Switch::new().with_label("Choice")),
            Box::new(RadioGroup::with_labeled_options(RadioGroupId(1), &[(RadioId(1), "Choice")])),
            Box::new(SpinBox::new().with_value(5)),
            Box::new(Slider::new(0.0, 1.0).with_value(0.5)),
        ];
        for (index, control) in controls.into_iter().enumerate() {
            let (mut fixture, content) =
                TouchButtonFixture::scrollable(viewport, Size::new(width, 800.0), false);
            fixture.text.set_scale(dpr);
            let node = fixture.push_content_child(content, RectF::new(20.0, 78.0, 190.0, 42.0), control);
            fixture.layout(viewport);
            fixture.render(dpr);
            let root = fixture.root;
            let owner = fixture.owner(node);
            let point = Point::new(28.0, 92.0);
            let pointer = fixture.touch(1, TouchPhase::Started, point).unwrap();
            assert_eq!(fixture.captured(pointer), None, "control {index}");
            assert_eq!(fixture.domain.focus_manager.focused(), None);

            fixture.touch(1, TouchPhase::Moved, Point::new(point.x, point.y - 4.0));
            assert_eq!(fixture.captured(pointer), Some(fixture.owner(root)));
            fixture.touch(1, TouchPhase::Ended, Point::new(point.x, point.y - 4.0));
            assert_eq!(fixture.domain.focus_manager.focused(), None);
            assert!(has_scroll_node(&fixture.render(dpr).0.root, (0.0, 4.0)));
            fixture.area_mut(root).scroll_to(0.0, 0.0);
            fixture.layout(viewport);

            let pointer = fixture.touch(2, TouchPhase::Started, point).unwrap();
            assert_eq!(fixture.captured(pointer), None);
            fixture.touch(2, TouchPhase::Ended, point);
            assert_eq!(fixture.captured(pointer), None);
            assert_eq!(fixture.domain.focus_manager.focused(), Some(owner), "control {index}");
            let widget = fixture.tree.node(node).unwrap().widget();
            match index {
                0 | 1 => assert_eq!(fixture.domain.keyboard_mode, KeyboardAuthorityMode::Capture),
                2 => assert!(widget.wants_scroll()),
                3 => assert!(widget.as_any().downcast_ref::<Checkbox>().unwrap().is_checked()),
                4 => assert!(widget.as_any().downcast_ref::<Switch>().unwrap().is_on()),
                5 => assert_eq!(widget.as_any().downcast_ref::<RadioGroup>().unwrap().selected(), Some(RadioId(1))),
                6 => assert_eq!(widget.as_any().downcast_ref::<SpinBox>().unwrap().value(), 4),
                7 => assert!(widget.as_any().downcast_ref::<Slider>().unwrap().value() < 0.5),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn hal397_slider_drag_wins_on_its_declared_axis_and_cancelled_tap_is_inert() {
    use halcyon::widgets::{checkbox::Checkbox, slider::Slider};

    let viewport = Size::new(390.0, 200.0);
    let (mut fixture, content) =
        TouchButtonFixture::scrollable(viewport, Size::new(viewport.width, 800.0), false);
    let slider = fixture.push_content_child(
        content, RectF::new(20.0, 80.0, 200.0, 32.0), Box::new(Slider::new(0.0, 1.0)),
    );
    let checkbox = fixture.push_content_child(
        content, RectF::new(20.0, 128.0, 160.0, 32.0), Box::new(Checkbox::new()),
    );
    fixture.layout(viewport);
    fixture.render(1.0);
    let position = Point::new(30.0, 90.0);
    let pointer = fixture.touch(1, TouchPhase::Started, position).unwrap();
    assert_eq!(fixture.captured(pointer), None);
    fixture.touch(1, TouchPhase::Moved, Point::new(170.0, 90.0));
    assert_eq!(fixture.captured(pointer), Some(fixture.owner(slider)));
    assert_eq!(fixture.domain.gesture_registry.committed_candidate(pointer).unwrap().kind, GestureKind::Drag);
    fixture.touch(1, TouchPhase::Ended, Point::new(180.0, 90.0));
    assert_eq!(fixture.captured(pointer), None);
    assert_eq!(fixture.area(fixture.root).visual_scroll_y(), 0.0);
    assert!(
        fixture.tree.node(slider).unwrap().widget().as_any()
            .downcast_ref::<Slider>().unwrap().value() > 0.5
    );

    let position = Point::new(25.0, 140.0);
    fixture.touch(2, TouchPhase::Started, position);
    fixture.touch(2, TouchPhase::Cancelled, position);
    fixture.touch(2, TouchPhase::Ended, position);
    assert!(
        !fixture.tree.node(checkbox).unwrap().widget().as_any()
            .downcast_ref::<Checkbox>().unwrap().is_checked()
    );
}

#[test]
fn hal399_routed_flick_uses_the_final_up_and_the_same_curve_at_every_frame_rate() {
    use TouchPhase::*;
    let viewport = Size::new(390.0, 190.0);
    let samples: Vec<Vec<f64>> = [30.0, 60.0, 120.0].into_iter()
        .map(|rate| (1..=(rate * 0.5) as usize).map(|tick| tick as f64 / rate).collect())
        .chain([vec![0.014, 0.033, 0.061, 0.173, 0.286, 0.391, 0.5]])
        .collect();
    for times in samples {
        let (mut f, _) = TouchButtonFixture::scrollable(
            viewport, Size::new(viewport.width, 1500.0), false,
        );
        let start = Instant::now();
        let released_at = start + Duration::from_millis(25);
        let pointer = f.touch_at(1, Started, Point::new(28.0, 54.0), start).unwrap();
        assert_eq!(f.captured(pointer), Some(f.owner(f.buttons[0])));
        f.touch_at(1, Moved, Point::new(28.0, 24.0), start + Duration::from_millis(20));
        assert_eq!(f.area(f.root).visual_scroll_y(), 30.0);
        f.touch_at(1, Ended, Point::new(28.0, 14.0), released_at);
        assert_eq!(f.area(f.root).visual_scroll_y(), 40.0);
        assert_eq!(f.captured(pointer), None);
        assert_eq!(f.clicks[0].get(), 0);
        for elapsed in times {
            assert!(f.advance_kinetic_scroll(released_at + Duration::from_secs_f64(elapsed)));
            let expected = 40.0 + 400.0 * (1.0 - (-4.0 * elapsed).exp());
            assert!((f.area(f.root).visual_scroll_y() - expected as f32).abs() < 0.002);
        }
        assert!(!f.advance_kinetic_scroll(released_at + Duration::from_secs(2)));
        assert_eq!(f.area(f.root).visual_scroll_y(), 440.0);
        assert!(has_scroll_node(&f.render(2.0).0.root, (0.0, 440.0)));
        assert!(!f.advance_kinetic_scroll(released_at + Duration::from_secs(3)));
        assert_eq!(f.area(f.root).visual_scroll_y(), 440.0);
    }
}

#[test]
fn hal399_admitted_touch_wheel_and_programmatic_intents_interrupt_without_rewind() {
    use TouchPhase::*;
    let viewport = Size::new(320.0, 190.0);
    let (mut f, _) = TouchButtonFixture::scrollable(
        viewport, Size::new(viewport.width, 1500.0), false,
    );
    let origin = Point::new(15.0, 135.0);
    let end = Point::new(15.0, 95.0);
    let start = Instant::now();
    for (index, interruption) in ["touch", "wheel", "jump", "reveal"].into_iter().enumerate() {
        let root = f.root;
        f.area_mut(root).jump_to_y(0.0);
        f.layout(viewport);
        let down = start + Duration::from_secs(index as u64 + 1);
        let released = down + Duration::from_millis(30);
        f.touch_at(3, Started, origin, down);
        f.touch_at(3, Moved, end, down + Duration::from_millis(20));
        f.touch_at(3, Ended, end, released);
        assert!(f.advance_kinetic_scroll(released + Duration::from_millis(70)));
        let before = f.area(root).visual_scroll_y();
        match interruption {
            "touch" => {
                f.touch_at(4, Started, origin, released + Duration::from_millis(80));
                f.touch_at(4, Cancelled, origin, released + Duration::from_millis(90));
            }
            "wheel" => f.wheel(origin, -22.0),
            "jump" => f.area_mut(root).jump_to_y(220.0),
            "reveal" => f.area_mut(root).resolve_reveal_rect(
                RectF::new(0.0, 330.0, 25.0, 20.0),
                halcyon::core::event::ScrollAlign::Start,
            ),
            _ => unreachable!(),
        }
        assert!(!f.advance_kinetic_scroll(released + Duration::from_millis(300)));
        assert_eq!(f.area(root).visual_scroll_y(), if interruption == "jump" { 220.0 } else { before });
        match interruption {
            "wheel" => assert_eq!(f.area(root).target_scroll_y(), before + 22.0),
            "reveal" => assert_eq!(f.area(root).target_scroll_y(), 330.0),
            _ => {}
        }
    }
}

#[test]
fn hal399_slow_stale_cancelled_and_out_of_order_samples_do_not_invent_velocity() {
    use TouchPhase::*;
    let viewport = Size::new(320.0, 180.0);
    let (mut f, _) = TouchButtonFixture::scrollable(
        viewport, Size::new(viewport.width, 1400.0), false,
    );
    let start = Instant::now();
    for (index, (dy, movement_ms, release_ms, terminal, expected)) in [
        (5.0, 80, 80, Ended, 5.0),
        (60.0, 20, 121, Ended, 60.0),
        (60.0, 20, 30, Cancelled, 0.0),
    ].into_iter().enumerate() {
        let root = f.root;
        f.area_mut(root).jump_to_y(0.0);
        f.layout(viewport);
        let down = start + Duration::from_secs(index as u64 + 1);
        let position = Point::new(18.0, 130.0);
        let moved = Point::new(position.x, position.y - dy);
        f.touch_at(5, Started, position, down);
        f.touch_at(5, Moved, moved, down + Duration::from_millis(movement_ms));
        assert!(f.touch_at(5, Moved, Point::new(18.0, 0.0), down).is_none());
        assert!(f.touch_at(5, Moved, Point::new(18.0, f32::NAN), down + Duration::from_millis(movement_ms)).is_none());
        let release = down + Duration::from_millis(release_ms);
        f.touch_at(5, terminal, moved, release);
        assert!(!f.advance_kinetic_scroll(release + Duration::from_millis(500)));
        assert_eq!(f.area(root).visual_scroll_y(), expected);
    }
}

#[test]
fn hal399_horizontal_flick_caps_velocity_and_stops_at_the_new_bound() {
    use TouchPhase::*;
    let viewport = Size::new(320.0, 180.0);
    let (mut f, content) = TouchButtonFixture::scrollable(
        viewport, Size::new(1200.0, 950.0), true,
    );
    let start = Instant::now();
    let released = start + Duration::from_millis(11);
    f.touch_at(3, Started, Point::new(125.0, 120.0), start);
    f.touch_at(3, Moved, Point::new(25.0, 120.0), start + Duration::from_millis(10));
    f.touch_at(3, Ended, Point::new(25.0, 120.0), released);
    assert_eq!(f.area(f.root).target_scroll_x(), 100.0);
    assert!(f.advance_kinetic_scroll(released + Duration::from_millis(100)));
    let expected = 100.0 + 1000.0 * (1.0 - (-0.4_f32).exp());
    assert!((f.area(f.root).target_scroll_x() - expected).abs() < 0.002);
    assert_eq!(f.area(f.root).visual_scroll_y(), 0.0);
    f.tree.node_mut(content).unwrap().widget_mut().as_any_mut()
        .downcast_mut::<TouchScrollContent>().unwrap().extent.width = 520.0;
    f.tree.mark_layout_dirty(content);
    f.layout(viewport);
    let bound = f.area(f.root).scroll_container().unwrap().max_scroll().0;
    assert_eq!(bound, 210.0); // Opposing vertical rail occludes 10 logical pixels.
    assert!(!f.advance_kinetic_scroll(released + Duration::from_millis(200)));
    assert_eq!(f.area(f.root).target_scroll_x(), bound);
    assert!(has_scroll_node(&f.render(3.0).0.root, (bound, 0.0)));
}

#[test]
fn touch_scroll_threshold_and_incremental_delta() {
    for (width, dpr) in [(320.0, 2.0), (390.0, 3.0), (900.0, 1.0)] {
        let (mut f, _) =
            TouchButtonFixture::scrollable(Size::new(width, 170.0), Size::new(width, 800.0), false);
        let root = f.root;
        let owner = f.owner(root);
        let start = Point::new(200.0, 88.0);
        let pointer = f.touch(9, TouchPhase::Started, start).unwrap();
        f.touch(9, TouchPhase::Moved, Point::new(200.0, 84.01));
        assert_eq!(f.captured(pointer), None);
        assert_eq!(f.area(root).visual_scroll_y(), 0.0);
        f.tree.node_mut(root).unwrap().dirty_paint = false;
        f.touch(9, TouchPhase::Moved, Point::new(200.0, 84.0));
        assert_eq!(f.captured(pointer), Some(owner));
        assert_eq!(f.domain.scroll_capture_owner, Some(owner));
        assert!(f.domain.gesture_registry.is_committed(pointer));
        assert_eq!(f.area(root).visual_scroll_y(), 4.0);
        assert!(f.tree.node(root).unwrap().dirty_paint);
        assert!(has_scroll_node(&f.render(dpr).0.root, (0.0, 4.0)));
        f.tree.node_mut(root).unwrap().dirty_paint = false;
        f.touch(9, TouchPhase::Moved, Point::new(200.0, 84.0));
        assert!(!f.tree.node(root).unwrap().dirty_paint);
        f.touch(9, TouchPhase::Moved, Point::new(200.0, 72.0));
        assert_eq!(f.area(root).visual_scroll_y(), 16.0);
        f.touch(9, TouchPhase::Moved, Point::new(200.0, -1000.0));
        let max = f.area(root).scroll_container().unwrap().max_scroll().1;
        assert_eq!(f.area(root).visual_scroll_y(), max);
        f.touch(9, TouchPhase::Moved, Point::new(200.0, -999.0));
        assert_eq!(f.area(root).visual_scroll_y(), max - 1.0);
        f.touch(9, TouchPhase::Ended, Point::new(200.0, -996.0));
        assert_eq!(f.area(root).visual_scroll_y(), max - 4.0);
        assert_eq!(f.captured(pointer), None);
        assert_eq!(f.domain.scroll_capture_owner, None);
    }
}

#[test]
fn touch_scroll_button_handoff_armed_and_disarmed() {
    for disarm_first in [false, true] {
        let (mut f, _) =
            TouchButtonFixture::scrollable(Size::new(320.0, 170.0), Size::new(320.0, 800.0), false);
        let root = f.root;
        let owner = f.owner(root);
        let button = f.buttons[0];
        let button_owner = f.owner(button);
        let center = f.center(0);
        let start = if disarm_first {
            Point::new(center.x, 41.0)
        } else {
            center
        };
        let pointer = f.touch(7, TouchPhase::Started, start).unwrap();
        let session = f.domain.gesture_registry.session_id(pointer).unwrap();
        assert_eq!(f.captured(pointer), Some(button_owner));
        if disarm_first {
            f.touch(7, TouchPhase::Moved, Point::new(start.x, start.y - 2.0));
            assert_eq!(f.captured(pointer), Some(button_owner));
        }
        assert!(
            f.tree
                .node(button)
                .unwrap()
                .widget()
                .has_unfinished_touch_press(pointer)
        );
        f.tree.node_mut(button).unwrap().dirty_paint = false;
        let end = Point::new(start.x, start.y - 4.0);
        f.touch(7, TouchPhase::Moved, end);
        assert_eq!(f.captured(pointer), Some(owner));
        assert_eq!(f.domain.gesture_registry.session_id(pointer), Some(session));
        let committed = f
            .domain
            .gesture_registry
            .committed_candidate(pointer)
            .unwrap();
        assert_eq!(
            (committed.widget_id, committed.kind),
            (owner, GestureKind::Scroll)
        );
        assert!(
            !f.tree
                .node(button)
                .unwrap()
                .widget()
                .has_unfinished_touch_press(pointer)
        );
        assert!(f.tree.node(button).unwrap().dirty_paint);
        assert_eq!(f.area(root).visual_scroll_y(), 4.0);
        f.touch(7, TouchPhase::Ended, end);
        assert_eq!(f.clicks[0].get(), 0);
        let tapped = Point::new(center.x, center.y - 4.0);
        f.touch(7, TouchPhase::Started, tapped);
        f.touch(7, TouchPhase::Ended, tapped);
        assert_eq!(f.clicks[0].get(), 1);
    }
}

#[derive(Default)]
struct TouchDragTrace {
    moves: Cell<usize>,
    commits: Cell<usize>,
    cancels: Cell<usize>,
    releases: Cell<usize>,
}

struct TouchDragProbe {
    id: WidgetInternalId,
    trace: Rc<TouchDragTrace>,
    capture_on_down: bool,
    request_drag: bool,
}

impl WidgetDowncast for TouchDragProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for TouchDragProbe {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let size = constraints.constrain(Size::new(60.0, 50.0));
        for &child in children {
            cx.layout_child(child, Constraints::tight(size));
            cx.set_child_rect(child, RectF::new(0.0, 0.0, size.width, size.height));
        }
        LayoutResult::fit(size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
    fn hit_testable(&self) -> bool {
        true
    }
    fn accepts_touch_pointer_input(&self) -> bool {
        true
    }
    fn gesture_surface(&self) -> &'static [GestureKind] {
        &[GestureKind::Drag]
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let Event::Pointer(pointer) = event else {
            return false;
        };
        match pointer.kind {
            PointerEventKind::Down if phase == EventPhase::Target && self.capture_on_down => {
                cx.capture_pointer(PointerButton::Primary)
            }
            PointerEventKind::Move | PointerEventKind::DragStart => {
                self.trace.moves.set(self.trace.moves.get() + 1);
                if self.request_drag {
                    cx.request_gesture_commit();
                }
            }
            PointerEventKind::Up if phase == EventPhase::Target => {
                self.trace.releases.set(self.trace.releases.get() + 1)
            }
            _ => (),
        }
        false
    }

    fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
        assert_eq!(kind, GestureKind::Drag);
        assert!(cx.has_pointer_capture());
        self.trace.commits.set(self.trace.commits.get() + 1);
    }

    fn on_gesture_cancel(&mut self, _cx: &mut EventContext, _kind: GestureKind) {
        self.trace.cancels.set(self.trace.cancels.get() + 1);
    }
}

#[test]
fn positioned_consumer_absolute_button_capture_excludes_ancestor_motion() {
    let viewport = Size::new(320.0, 170.0);
    let (mut f, content) = TouchButtonFixture::scrollable(viewport, Size::new(320.0, 800.0), false);
    let trace = Rc::new(TouchDragTrace::default());
    let ancestor = f.push_content_child(
        content,
        RectF::new(220.0, 55.0, 70.0, 55.0),
        Box::new(TouchDragProbe {
            id: WidgetInternalId::new(),
            trace: trace.clone(),
            capture_on_down: false,
            request_drag: true,
        }),
    );
    let clicks = Rc::new(Cell::new(0));
    let click_count = clicks.clone();
    let button = f.tree.add_child(
        ancestor,
        Box::new(Button::with_text("Tap").on_click(move || click_count.set(click_count.get() + 1))),
        None,
        0,
    );
    f.layout(viewport);
    let down = Point::new(235.0, 80.0);
    let end = Point::new(243.0, 80.0);
    f.mouse_button(RawButtonState::Pressed, down);
    assert_eq!(f.captured(PointerId::MOUSE), Some(f.owner(button)));
    assert!(
        f.domain
            .gesture_registry
            .candidates(PointerId::MOUSE)
            .iter()
            .any(|candidate| candidate.widget_id == f.owner(ancestor))
    );
    let events = f.input(RawInputEvent::PointerMoved {
        window_id: f.window,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        position: end,
        delta: (8.0, 0.0),
    });
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event(), FusedInputEvent::DragStart { .. }))
    );
    assert_eq!(
        trace.moves.get(),
        0,
        "captured DragStart/Move cannot reach the ancestor"
    );
    assert!(!f.domain.gesture_registry.is_committed(PointerId::MOUSE));
    assert_eq!((trace.commits.get(), trace.cancels.get()), (0, 0));
    assert_eq!(f.captured(PointerId::MOUSE), Some(f.owner(button)));
    f.mouse_button(RawButtonState::Released, end);
    assert_eq!(clicks.get(), 1);
    assert_eq!(f.captured(PointerId::MOUSE), None);
}

#[test]
fn touch_scroll_capture_refusal_preserves_source() {
    let viewport = Size::new(320.0, 170.0);
    let (mut f, content) = TouchButtonFixture::scrollable(viewport, Size::new(320.0, 900.0), false);
    let trace = Rc::new(TouchDragTrace::default());
    let probe = f.push_content_child(
        content,
        RectF::new(220.0, 55.0, 70.0, 55.0),
        Box::new(TouchDragProbe {
            id: WidgetInternalId::new(),
            trace: trace.clone(),
            capture_on_down: true,
            request_drag: false,
        }),
    );
    f.layout(viewport);
    let pointer = f
        .touch(6, TouchPhase::Started, Point::new(245.0, 85.0))
        .unwrap();
    let source_owner = f.owner(probe);
    assert_eq!(f.captured(pointer), Some(source_owner));
    let session = f.domain.gesture_registry.session_id(pointer);
    assert_eq!(f.domain.gesture_registry.candidates(pointer).len(), 2);
    f.touch(6, TouchPhase::Moved, Point::new(245.0, 74.0));
    assert_eq!(f.captured(pointer), Some(source_owner));
    assert_eq!(f.domain.gesture_registry.session_id(pointer), session);
    assert!(
        f.domain
            .gesture_registry
            .committed_candidate(pointer)
            .is_none()
    );
    assert_eq!(f.domain.gesture_registry.candidates(pointer).len(), 2);
    assert!(
        f.domain
            .gesture_registry
            .take_commit_requests(pointer)
            .is_empty()
    );
    assert_eq!(
        (trace.moves.get(), trace.commits.get(), trace.cancels.get()),
        (1, 0, 0)
    );
    assert_eq!(f.area(f.root).visual_scroll_y(), 0.0);
    f.touch(6, TouchPhase::Ended, Point::new(245.0, 74.0));
    assert_eq!(trace.releases.get(), 1);
    assert_eq!(f.captured(pointer), None);
}

#[test]
fn touch_scroll_pending_non_scroll_wins_and_is_not_replaced() {
    for capture_on_down in [false, true] {
        let viewport = Size::new(320.0, 170.0);
        let (mut f, content) =
            TouchButtonFixture::scrollable(viewport, Size::new(320.0, 900.0), false);
        let trace = Rc::new(TouchDragTrace::default());
        let probe = f.push_content_child(
            content,
            RectF::new(220.0, 55.0, 70.0, 55.0),
            Box::new(TouchDragProbe {
                id: WidgetInternalId::new(),
                trace: trace.clone(),
                capture_on_down,
                request_drag: true,
            }),
        );
        f.layout(viewport);
        let pointer = f
            .touch(6, TouchPhase::Started, Point::new(245.0, 85.0))
            .unwrap();
        assert_eq!(f.captured(pointer), capture_on_down.then(|| f.owner(probe)));
        assert_eq!(f.domain.gesture_registry.candidates(pointer).len(), 2);
        f.touch(6, TouchPhase::Moved, Point::new(245.0, 74.0));
        let winner = *f
            .domain
            .gesture_registry
            .committed_candidate(pointer)
            .unwrap();
        assert_eq!(
            (winner.widget_id, winner.kind),
            (f.owner(probe), GestureKind::Drag)
        );
        assert_eq!(f.captured(pointer), Some(winner.widget_id));
        f.touch(6, TouchPhase::Moved, Point::new(245.0, 67.0));
        assert_eq!(
            f.domain
                .gesture_registry
                .committed_candidate(pointer)
                .unwrap()
                .candidate_id,
            winner.candidate_id
        );
        assert_eq!((trace.commits.get(), trace.cancels.get()), (1, 0));
        assert_eq!(f.area(f.root).visual_scroll_y(), 0.0);
        f.touch(6, TouchPhase::Ended, Point::new(245.0, 60.0));
        assert_eq!(trace.releases.get(), 1);
    }
}

#[test]
fn touch_scroll_nested_axis_and_exclusive_bounds() {
    let viewport = Size::new(390.0, 300.0);
    let (mut f, content) = TouchButtonFixture::scrollable(viewport, Size::new(640.0, 980.0), true);
    let root = f.root;
    let nested = f.add_nested_scroll(content, viewport, Size::new(180.0, 480.0), false);
    let start = Point::new(180.0, 150.0);
    let pointer = f.touch(12, TouchPhase::Started, start).unwrap();
    f.touch(12, TouchPhase::Moved, Point::new(184.0, 154.0));
    assert_eq!(
        f.captured(pointer),
        Some(f.owner(nested)),
        "vertical tie and deepest positive range at the top bound"
    );
    assert_eq!(f.area(nested).visual_scroll_y(), 0.0);
    f.touch(12, TouchPhase::Moved, Point::new(204.0, 154.0));
    assert_eq!(
        f.area(nested).visual_scroll_y(),
        0.0,
        "axis remains vertical"
    );
    f.touch(12, TouchPhase::Moved, Point::new(204.0, 150.0));
    assert_eq!(
        f.area(nested).visual_scroll_y(),
        4.0,
        "reverse immediately from the bound"
    );
    assert_eq!(f.area(root).scroll_offset(), Some((0.0, 0.0)));
    f.touch(12, TouchPhase::Ended, Point::new(204.0, 150.0));

    let pointer = f.touch(13, TouchPhase::Started, start).unwrap();
    f.touch(13, TouchPhase::Moved, Point::new(174.0, 151.0));
    assert_eq!(
        f.captured(pointer),
        Some(f.owner(root)),
        "inner supports no horizontal scroll"
    );
    assert_eq!(f.area(root).scroll_offset(), Some((6.0, 0.0)));
    f.touch(13, TouchPhase::Ended, Point::new(174.0, 151.0));

    let inner_content = f.area(nested).content();
    f.tree
        .node_mut(inner_content)
        .unwrap()
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<TouchScrollContent>()
        .unwrap()
        .extent
        .height = 100.0;
    f.tree.mark_layout_dirty(inner_content);
    f.layout(viewport);
    let pointer = f.touch(14, TouchPhase::Started, start).unwrap();
    f.touch(14, TouchPhase::Moved, Point::new(180.0, 145.0));
    assert_eq!(
        f.captured(pointer),
        Some(f.owner(root)),
        "zero range excludes the inner owner"
    );
    assert_eq!(f.area(root).scroll_offset(), Some((6.0, 5.0)));
    f.touch(14, TouchPhase::Ended, Point::new(180.0, 145.0));
}

#[test]
fn touch_scroll_contact_lifecycle_isolation() {
    let (mut f, _) =
        TouchButtonFixture::scrollable(Size::new(320.0, 170.0), Size::new(320.0, 800.0), false);
    let owner = f.owner(f.root);
    let start = Point::new(245.0, 90.0);
    let pointer = f.touch(20, TouchPhase::Started, start).unwrap();
    f.touch(20, TouchPhase::Moved, Point::new(245.0, 80.0));
    let later = f.touch(21, TouchPhase::Started, start).unwrap();
    f.touch(21, TouchPhase::Moved, Point::new(245.0, 60.0));
    assert_eq!(f.captured(later), None);
    f.touch(21, TouchPhase::Cancelled, start);
    assert_eq!(f.captured(pointer), Some(owner));
    assert_eq!(f.area(f.root).target_scroll_y(), 10.0);

    let button = f.center(0);
    let tap = Point::new(button.x, button.y - 10.0);
    f.touch(22, TouchPhase::Started, tap);
    f.touch(22, TouchPhase::Ended, tap);
    assert_eq!(f.clicks[0].get(), 1);

    let later = f.touch(23, TouchPhase::Started, start).unwrap();
    f.touch(20, TouchPhase::Ended, Point::new(245.0, 80.0));
    f.touch(23, TouchPhase::Moved, Point::new(245.0, 60.0));
    assert_eq!(
        f.captured(later),
        None,
        "a later contact never gains eligibility"
    );
    f.touch(23, TouchPhase::Ended, start);
    let fresh = f.touch(24, TouchPhase::Started, start).unwrap();
    f.touch(24, TouchPhase::Moved, Point::new(245.0, 80.0));
    assert_eq!(f.captured(fresh), Some(owner));
    f.input(RawInputEvent::FocusChanged {
        window_id: f.window,
        time: Instant::now(),
        focused: false,
    });
    assert_eq!(f.captured(fresh), None);
    assert_eq!(
        f.area(f.root).target_scroll_y(),
        10.0,
        "focus loss rolls back the matching live pan"
    );
}

#[test]
fn touch_scroll_batched_terminal_order() {
    let (mut f, _) =
        TouchButtonFixture::scrollable(Size::new(320.0, 170.0), Size::new(320.0, 800.0), false);
    let mut events = Vec::new();
    for (phase, y) in [
        (TouchPhase::Started, 90.0),
        (TouchPhase::Moved, 80.0),
        (TouchPhase::Ended, 70.0),
    ] {
        f.ingest(
            RawInputEvent::Touch {
                window_id: f.window,
                time: Instant::now(),
                contact_id: 0,
                phase,
                pos: Point::new(240.0, y),
            },
            &mut events,
        );
    }
    let pointer = events[0].touch_motion().unwrap().press.pointer;
    f.deliver(events[0].clone());
    f.deliver(events[1].clone());
    assert_eq!(f.captured(pointer), Some(f.owner(f.root)));
    assert_eq!(f.area(f.root).visual_scroll_y(), 10.0);
    f.deliver(events[2].clone());
    assert_eq!(f.area(f.root).visual_scroll_y(), 20.0);
    assert_eq!(f.captured(pointer), None);

    let next = f
        .touch(0, TouchPhase::Started, Point::new(240.0, 90.0))
        .unwrap();
    assert_ne!(next, pointer);
    f.deliver(events[2].clone());
    assert!(f.domain.gesture_registry.has_session(next));
    f.touch(0, TouchPhase::Ended, Point::new(240.0, 65.0));
    assert_eq!(
        f.area(f.root).visual_scroll_y(),
        20.0,
        "Up alone cannot recognize a pan"
    );
}

#[test]
fn touch_scroll_cancel_restores_current_geometry_clamp() {
    let viewport = Size::new(320.0, 170.0);
    let (mut f, content) = TouchButtonFixture::scrollable(viewport, Size::new(320.0, 800.0), false);
    let root = f.root;
    f.area_mut(root).jump_to_y(80.0);
    let start = Point::new(240.0, 90.0);
    f.touch(11, TouchPhase::Started, start);
    f.touch(11, TouchPhase::Moved, Point::new(240.0, 70.0));
    assert_eq!(f.area(root).visual_scroll_y(), 100.0);
    f.tree
        .node_mut(content)
        .unwrap()
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<TouchScrollContent>()
        .unwrap()
        .extent
        .height = 195.0;
    f.tree.mark_layout_dirty(content);
    f.layout(viewport);
    assert_eq!(f.area(root).content_extent_height(), 195.0);
    let max = f.area(root).scroll_container().unwrap().max_scroll().1;
    assert_eq!(max, 25.0);
    f.touch(11, TouchPhase::Moved, Point::new(240.0, 82.0));
    assert_eq!(f.area(root).visual_scroll_y(), 13.0);
    f.touch(11, TouchPhase::Cancelled, start);
    assert_eq!(f.area(root).visual_scroll_y(), max);
    assert_eq!(f.area(root).content_extent_height(), 195.0);
    assert_eq!(f.domain.scroll_capture_owner, None);
    f.touch(12, TouchPhase::Started, start);
    f.touch(12, TouchPhase::Moved, Point::new(240.0, 94.0));
    f.touch(12, TouchPhase::Ended, Point::new(240.0, 100.0));
    assert_eq!(f.area(root).visual_scroll_y(), max - 10.0);
}

#[test]
fn touch_scroll_preserves_current_user_authority() {
    use halcyon::core::event::{InteractionDomains, ScrollAlign};

    let viewport = Size::new(390.0, 300.0);
    let extent = Size::new(390.0, 800.0);
    let (mut f, content) = TouchButtonFixture::scrollable(viewport, extent, false);
    let root = f.root;
    let nested = f.add_nested_scroll(content, viewport, Size::new(180.0, 520.0), false);
    let initial_focus = f.owner(f.buttons[0]);
    f.router
        .request_focus_for_widget(&mut f.tree, initial_focus, &mut f.domain);
    let start = Point::new(305.0, 85.0);
    f.wheel(start, -25.0);
    assert_eq!(f.domain.scroll_capture_owner, Some(f.owner(root)));
    f.area_mut(root).scroll_to_y(210.0);
    assert_eq!(f.area(root).target_scroll_y(), 210.0);
    assert_eq!(f.area(root).visual_scroll_y(), 0.0);

    let pointer = f.touch(1, TouchPhase::Started, start).unwrap();
    assert_eq!(
        f.domain.scroll_capture_owner, None,
        "first touch terminates wheel-only authority"
    );
    f.touch(1, TouchPhase::Moved, Point::new(305.0, 75.0));
    assert_eq!(f.captured(pointer), Some(f.owner(root)));
    assert_eq!(f.area(root).target_scroll_y(), 10.0);
    assert_eq!(
        f.area(root).visual_scroll_y(),
        10.0,
        "use the visible baseline instead of the smoothing target"
    );

    let nested_point = Point::new(180.0, 130.0);
    f.wheel(start, -50.0);
    f.wheel(nested_point, -50.0);
    f.area_mut(root).scroll_to_y(180.0);
    f.area_mut(nested).jump_to_y(100.0);
    let reveal = RectF::new(0.0, 400.0, 10.0, 5.0);
    f.area_mut(root)
        .resolve_reveal_rect(reveal, ScrollAlign::Nearest);
    f.area_mut(nested)
        .resolve_reveal_rect(reveal, ScrollAlign::Nearest);
    let other_button = f.center(1);
    let click = Point::new(other_button.x, other_button.y - 10.0);
    f.mouse_button(RawButtonState::Pressed, click);
    f.mouse_button(RawButtonState::Released, click);
    assert_eq!(f.clicks[1].get(), 1);
    assert_eq!(
        f.domain.focus_manager.focused(),
        Some(f.owner(f.buttons[1]))
    );
    assert!(
        f.router.take_pending_focus_reveal().is_none(),
        "focus changes do not enqueue competing reveal"
    );
    assert_eq!(
        f.domain.scroll_capture_owner,
        Some(f.owner(root)),
        "unrelated mouse Down/Up cannot clear the pan"
    );
    assert_eq!(f.area(root).target_scroll_y(), 10.0);
    assert_eq!(f.area(nested).target_scroll_y(), 0.0);

    let (mut independent, _) = TouchButtonFixture::scrollable(viewport, extent, false);
    let mut domains = InteractionDomains::new();
    independent.window = WindowId(2);
    let domain = domains.register_window(independent.window);
    independent.domain = std::mem::take(domains.state_mut(domain).unwrap());
    let other_root = independent.root;
    independent.wheel(start, -50.0);
    assert!(independent.area(other_root).target_scroll_y() > 0.0);
    assert_eq!(f.area(root).target_scroll_y(), 10.0);

    f.tree
        .node_mut(content)
        .unwrap()
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<TouchScrollContent>()
        .unwrap()
        .requested_scroll_y = Some(300.0);
    f.touch(1, TouchPhase::Ended, Point::new(305.0, 75.0));
    halcyon::core::app::post_layout_resolve_scroll_anchors(&mut f.tree);
    assert_eq!(f.domain.scroll_capture_owner, None);
    assert_eq!(
        f.area(root).target_scroll_y(),
        10.0,
        "no rejected intent replay"
    );
    assert_eq!(f.area(nested).target_scroll_y(), 0.0);
    f.wheel(start, -20.0);
    assert!(f.area(root).target_scroll_y() > 10.0);
}

#[test]
fn touch_scroll_scrollbar_hit_admission() {
    let (mut f, _) =
        TouchButtonFixture::scrollable(Size::new(320.0, 170.0), Size::new(320.0, 800.0), false);
    let root = f.root;
    let owner = f.owner(root);
    for y in [7.0, 110.0] {
        f.area_mut(root).jump_to_y(0.0);
        let (_, hits) = f.render(2.0);
        let start = Point::new(317.0, y);
        assert!(
            hits.iter()
                .any(|hit| hit.widget_id == owner && hit.abs_rect.contains(start))
        );
        assert_eq!(f.tree.hit_test(start.x, start.y).unwrap().widget_id, owner);
        let pointer = f.touch(2, TouchPhase::Started, start).unwrap();
        assert_eq!(
            f.captured(pointer),
            None,
            "ordinary scrollbar Down stays mouse-only"
        );
        assert_eq!(f.area(root).target_scroll_y(), 0.0);
        f.touch(2, TouchPhase::Moved, Point::new(start.x, start.y - 6.0));
        assert_eq!(f.captured(pointer), Some(owner));
        assert_eq!(
            f.area(root).visual_scroll_y(),
            6.0,
            "content pan, not thumb travel"
        );
        f.touch(2, TouchPhase::Ended, Point::new(start.x, start.y - 6.0));
    }

    f.area_mut(root).jump_to_y(0.0);
    f.render(2.0);
    let thumb = Point::new(317.0, 7.0);
    f.mouse_button(RawButtonState::Pressed, thumb);
    assert_eq!(f.captured(PointerId::MOUSE), Some(owner));
    f.input(RawInputEvent::PointerMoved {
        window_id: f.window,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        position: Point::new(317.0, 24.0),
        delta: (0.0, 17.0),
    });
    assert!(
        f.area(root).target_scroll_y() > 17.0,
        "mouse thumb travel retains the scroll ratio"
    );
    f.mouse_button(RawButtonState::Released, Point::new(317.0, 24.0));
    assert_eq!(f.captured(PointerId::MOUSE), None);
}

#[test]
fn touch_contact_button_tap_cancel_and_exit() {
    use halcyon::core::event::TouchPhase::*;
    let mut fixture = TouchButtonFixture::new();
    let center = fixture.center(0);
    let owner = fixture.tree.node(fixture.buttons[0]).unwrap().id();
    let pointer = fixture.touch(0, Started, center).unwrap();
    assert_ne!(pointer, PointerId::MOUSE);
    assert_eq!(fixture.clicks[0].get(), 0);
    assert_eq!(
        fixture
            .domain
            .pointer_capture_manager
            .current_capture(pointer),
        Some(owner)
    );
    assert_eq!(
        fixture
            .domain
            .gesture_registry
            .session_pointer_type(pointer),
        Some(PointerType::Touch)
    );
    fixture.touch(0, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 1);
    assert_eq!(
        fixture
            .domain
            .pointer_capture_manager
            .current_capture(pointer),
        None
    );
    fixture.touch(0, Started, center);
    fixture.touch(0, Cancelled, center);
    fixture.touch(0, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 1);
    fixture.touch(0, Started, center);
    fixture.touch(0, Moved, Point::new(500.0, 500.0));
    fixture.touch(0, Moved, center);
    fixture.touch(0, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 1);
    fixture.touch(0, Started, center);
    fixture.touch(0, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 2);
}

#[test]
fn touch_contact_same_button_keeps_initiating_pointer() {
    use halcyon::core::event::TouchPhase::*;
    let mut fixture = TouchButtonFixture::new();
    let center = fixture.center(0);
    let first = fixture.touch(0, Started, center).unwrap();
    let second = fixture.touch(1, Started, center).unwrap();
    assert_ne!(first, second);
    assert!(fixture.domain.pointer_capture_manager.is_captured(first));
    assert!(!fixture.domain.pointer_capture_manager.is_captured(second));
    fixture.touch(1, Cancelled, center);
    assert!(fixture.domain.pointer_capture_manager.is_captured(first));
    fixture.touch(0, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 1);
    fixture.touch(0, Started, center);
    fixture.touch(0, Moved, Point::new(500.0, 500.0));
    fixture.touch(1, Started, center);
    fixture.touch(0, Ended, center);
    fixture.touch(1, Ended, center);
    assert_eq!(fixture.clicks[0].get(), 1);
}

#[test]
fn touch_contact_different_buttons_have_independent_capture() {
    use halcyon::core::event::TouchPhase::*;
    let mut fixture = TouchButtonFixture::new();
    let left = fixture.center(0);
    let right = fixture.center(1);
    let first = fixture.touch(0, Started, left).unwrap();
    let second = fixture.touch(1, Started, right).unwrap();
    assert!(fixture.domain.pointer_capture_manager.is_captured(first));
    assert!(fixture.domain.pointer_capture_manager.is_captured(second));
    fixture.touch(0, Cancelled, left);
    assert!(!fixture.domain.pointer_capture_manager.is_captured(first));
    assert!(fixture.domain.pointer_capture_manager.is_captured(second));
    fixture.touch(1, Ended, right);
    assert_eq!(fixture.clicks[0].get(), 0);
    assert_eq!(fixture.clicks[1].get(), 1);
}

#[test]
fn touch_contact_cancel_preserves_independent_mouse_capture() {
    use halcyon::core::event::TouchPhase::*;
    let mut fixture = TouchButtonFixture::new();
    let left = fixture.center(0);
    let right = fixture.center(1);
    fixture.input(RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        state: RawButtonState::Pressed,
        position: right,
    });
    let mouse_owner = fixture.domain.pointer_capture.captured_widget;
    assert!(mouse_owner.is_some());
    fixture.touch(0, Started, left);
    fixture.touch(0, Cancelled, left);
    assert_eq!(fixture.domain.pointer_capture.captured_widget, mouse_owner);
    assert_eq!(
        fixture
            .domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        mouse_owner
    );
    fixture.input(RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        state: RawButtonState::Released,
        position: right,
    });
    assert_eq!(fixture.clicks[0].get(), 0);
    assert_eq!(fixture.clicks[1].get(), 1);
}

#[test]
fn touch_contact_focus_loss_cancels_buttons_without_activation() {
    use halcyon::core::event::TouchPhase::*;
    let mut fixture = TouchButtonFixture::new();
    let left = fixture.center(0);
    let right = fixture.center(1);
    fixture.touch(0, Started, left);
    fixture.touch(1, Started, right);
    fixture.input(RawInputEvent::FocusChanged {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        focused: false,
    });
    assert!(
        fixture
            .domain
            .pointer_capture_manager
            .active_pointers()
            .is_empty()
    );
    assert_eq!(fixture.clicks[0].get(), 0);
    assert_eq!(fixture.clicks[1].get(), 0);
    fixture.touch(0, Ended, left);
    fixture.touch(1, Ended, right);
    assert_eq!(fixture.clicks[0].get(), 0);
    assert_eq!(fixture.clicks[1].get(), 0);
}

#[test]
fn touch_contact_nonparticipating_selection_remains_inert() {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Label::new("Reader selection")
                .with_selection_policy(halcyon::core::text_selection::TextSelectionPolicy::Enabled),
        ),
        None,
        0,
    );
    let mut text = FontManagerTextEngine::new();
    {
        let _layout = LayoutAuthorityGuard::enter();
        LayoutContext::new(&mut tree, &mut text, &Theme::dark())
            .layout_child(root, Constraints::tight(Size::new(200.0, 60.0)));
        halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);
    }
    let mut fusion = InputFusion::new();
    let mut router = EventRouter::new();
    let mut domain = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    for phase in [
        halcyon::core::event::TouchPhase::Started,
        halcyon::core::event::TouchPhase::Moved,
        halcyon::core::event::TouchPhase::Ended,
    ] {
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::Touch {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                contact_id: 0,
                phase,
                pos: Point::new(10.0, 10.0),
            },
            halcyon::core::event::InteractionDomainId::PRIMARY,
            &mut domain.gesture_registry,
            &tree,
            None,
            &mut events,
        );
        for event in events {
            router.route_session_input_event(
                &mut tree,
                event,
                Some(root),
                &mut text,
                None,
                &mut domain,
                &mut bindings,
            );
        }
        assert!(domain.pointer_capture_manager.active_pointers().is_empty());
        assert_eq!(domain.focus_manager.focused(), None);
        assert_eq!(domain.keyboard_mode, KeyboardAuthorityMode::Navigation);
    }
}

// ============================================================================
// Test Widget: Tracks events and can register/commit gestures
// ============================================================================

#[derive(Clone, Default)]
struct CancellationTestState {
    events: Vec<(&'static str, f32, f32)>,
    commit_requested: bool,
}

struct CancellationTestWidget {
    internal_id: WidgetInternalId,
    state: Rc<RefCell<CancellationTestState>>,
    register_kind: Option<GestureKind>,
    request_commit_on_move: bool,
    commit_threshold: f32,
    start_pos: RefCell<Option<(f32, f32)>>,
}

impl CancellationTestWidget {
    fn new(
        state: Rc<RefCell<CancellationTestState>>,
        register_kind: Option<GestureKind>,
        request_commit_on_move: bool,
        commit_threshold: f32,
    ) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            state,
            register_kind,
            request_commit_on_move,
            commit_threshold,
            start_pos: RefCell::new(None),
        }
    }
}

impl WidgetDowncast for CancellationTestWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for CancellationTestWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(100.0, 100.0))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.internal_id
    }

    /// SPEC-HAL-128 Stage 8: Static gesture surface declaration.
    /// Returns the gesture kind this widget supports (based on register_kind).
    fn gesture_surface(&self) -> &'static [GestureKind] {
        match self.register_kind {
            Some(GestureKind::Drag) => &[GestureKind::Drag],
            Some(GestureKind::Scroll) => &[GestureKind::Scroll],
            Some(GestureKind::Resize) => &[GestureKind::Resize],
            Some(GestureKind::Select) => &[GestureKind::Select],
            Some(GestureKind::Pinch) => &[GestureKind::Pinch],
            Some(GestureKind::Swipe) => &[GestureKind::Swipe],
            Some(GestureKind::Draw) => &[GestureKind::Draw],
            Some(GestureKind::Custom) => &[GestureKind::Custom],
            None => &[],
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if let Event::Pointer(pe) = event {
            let kind_str = match pe.kind {
                PointerEventKind::Down => "down",
                PointerEventKind::Up => "up",
                PointerEventKind::Move => "move",
                PointerEventKind::Scroll { .. } => "scroll",
                PointerEventKind::DoubleClick => "double",
                PointerEventKind::TripleClick => "triple",
                PointerEventKind::DragStart => "dragstart",
            };
            self.state
                .borrow_mut()
                .events
                .push((kind_str, pe.local_pos.0, pe.local_pos.1));

            // On PointerDown: capture pointer (candidates registered by system via gesture_surface)
            if matches!(pe.kind, PointerEventKind::Down) {
                cx.capture_pointer(PointerButton::Primary);
                *self.start_pos.borrow_mut() = Some(pe.local_pos);
            }

            // On PointerMove: check threshold and request commit if exceeded
            if matches!(pe.kind, PointerEventKind::Move) && self.request_commit_on_move {
                if let Some((start_x, start_y)) = *self.start_pos.borrow() {
                    let dx = pe.local_pos.0 - start_x;
                    let dy = pe.local_pos.1 - start_y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    if distance >= self.commit_threshold {
                        cx.request_gesture_commit();
                        self.state.borrow_mut().commit_requested = true;
                    }
                }
            }

            true
        } else {
            false
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn setup_widget_for_cancellation(
    state: Rc<RefCell<CancellationTestState>>,
    register_kind: Option<GestureKind>,
    request_commit_on_move: bool,
    commit_threshold: f32,
) -> (
    WidgetTree,
    NodeId,
    TreeWidgetId,
    InteractionDomainState,
    FontManagerTextEngine,
) {
    let mut tree = WidgetTree::new();

    let root = tree.create_root(
        Box::new(CancellationTestWidget::new(
            state,
            register_kind,
            request_commit_on_move,
            commit_threshold,
        )),
        None,
        0,
    );
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
    halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
    tree.compute_viewports(root);

    let id = tree.node(root).unwrap().id();
    let domain_state = InteractionDomainState::new();
    let text_engine = FontManagerTextEngine::new();

    (tree, root, id, domain_state, text_engine)
}

fn send_pointer_down(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
    x: f32,
    y: f32,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(x, y),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

fn send_pointer_move(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
    x: f32,
    y: f32,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::PointerMove {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(x, y),
            delta: (0.0, 0.0),
            device: PointerDevice::Mouse,
            buttons: PointerButtonsState {
                primary: true,
                secondary: false,
                auxiliary: false,
            },
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

fn send_escape(
    router: &mut EventRouter,
    tree: &mut WidgetTree,
    root: NodeId,
    text_engine: &mut FontManagerTextEngine,
    domain_state: &mut InteractionDomainState,
) {
    router.route_input_event(
        tree,
        FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code: KeyCode::Escape,
            physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        },
        Some(root),
        text_engine,
        None,
        domain_state,
        &mut halcyon::runtime::binding_registry::BindingRegistry::new(),
    );
}

// ============================================================================
// Test: escape_cancels_active_gesture (INV-PC4)
// ============================================================================

/// INV-PC4: Pressing Escape while a gesture session is active or pointer captured
/// MUST cancel all session state.
#[test]
fn escape_cancels_active_gesture() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CancellationTestState::default()));
    let (mut tree, root, _widget_id, mut domain_state, mut text_engine) =
        setup_widget_for_cancellation(state.clone(), Some(GestureKind::Drag), true, 5.0);

    let mut router = EventRouter::new();

    // PointerDown at (50, 50) - begins session, registers candidate, captures pointer
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify session and capture are active
    assert!(
        domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "Session should be active after PointerDown"
    );
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture should be active after PointerDown"
    );

    // Move to trigger commit (beyond threshold)
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        60.0,
        60.0,
    );

    // Verify committed
    assert!(
        domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "Gesture should be committed after move"
    );

    // NOW press Escape - this should cancel everything
    send_escape(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
    );

    // INV-PC4 post-conditions: everything cancelled
    assert!(
        !domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "is_committed must be false after Escape"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "is_captured must be false after Escape"
    );
    assert!(
        !domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "has_session must be false after Escape"
    );
}

// ============================================================================
// Test: escape_prevents_commit_same_frame (INV-GA-CANCEL)
// ============================================================================

/// INV-GA-CANCEL: If a commit request and cancellation occur in the same frame,
/// cancellation wins. No commit after cancel.
#[test]
fn escape_prevents_commit_same_frame() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CancellationTestState::default()));
    let (mut tree, root, _widget_id, mut domain_state, mut text_engine) =
        setup_widget_for_cancellation(state.clone(), Some(GestureKind::Drag), true, 5.0);

    let mut router = EventRouter::new();

    // PointerDown
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Verify session active
    assert!(domain_state.gesture_registry.has_session(PointerId::MOUSE));

    // Queue a commit request by moving past threshold
    // Widget will call request_gesture_commit which pushes to pending_commits
    // But before resolution, we cancel
    {
        // Simulate the widget pushing a commit request
        let candidates = domain_state.gesture_registry.candidates(PointerId::MOUSE);
        assert!(!candidates.is_empty(), "Should have candidates");
        let candidate_id = candidates[0].candidate_id;
        domain_state
            .gesture_registry
            .push_commit_request(PointerId::MOUSE, candidate_id);
    }

    // NOW cancel - this should drain pending commits and prevent any commit
    router.cancel_pointer_session(
        PointerId::MOUSE,
        CancellationReason::Escape,
        &mut domain_state,
    );

    // Verify no commit occurred
    assert!(
        !domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "No commit should occur after cancellation"
    );
    assert!(
        !domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "Session should be cleared"
    );
}

// ============================================================================
// Test: overlay_cancels_existing_capture (INV-PC7)
// ============================================================================

/// INV-PC7: When an overlay becomes active, all existing gesture sessions and
/// pointer captures MUST be cancelled.
#[test]
fn overlay_cancels_existing_capture() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CancellationTestState::default()));
    let (mut tree, root, _widget_id, mut domain_state, mut text_engine) =
        setup_widget_for_cancellation(state.clone(), Some(GestureKind::Drag), true, 5.0);

    let mut router = EventRouter::new();

    // PointerDown - establishes session and capture
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Move to commit
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        60.0,
        60.0,
    );

    // Verify active state
    assert!(domain_state.gesture_registry.has_session(PointerId::MOUSE));
    assert!(
        domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );
    assert!(domain_state.gesture_registry.is_committed(PointerId::MOUSE));

    // Simulate overlay takeover using cancel_all_pointer_sessions
    router.cancel_all_pointer_sessions(CancellationReason::OverlayTakeover, &mut domain_state);

    // INV-PC7: All session state must be cleared
    assert!(
        !domain_state.gesture_registry.has_session(PointerId::MOUSE),
        "Session must be cleared after overlay takeover"
    );
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE),
        "Capture must be released after overlay takeover"
    );
    assert!(
        !domain_state.gesture_registry.is_committed(PointerId::MOUSE),
        "Committed state must be cleared after overlay takeover"
    );
}

// ============================================================================
// Test: cancel_clears_pending_commits (INV-PC5)
// ============================================================================

/// INV-PC5: On cancellation, no partial state persists.
/// Specifically: pending commits are drained.
#[test]
fn cancel_clears_pending_commits() {
    let mut registry = GestureRegistry::new();

    // Begin session
    registry.begin_session(PointerId::MOUSE, PointerType::Mouse);

    // Register a candidate
    let widget_id = TreeWidgetId(1);
    registry.register_candidate_request(PointerId::MOUSE, widget_id, GestureKind::Drag, None);

    let candidates = registry.candidates(PointerId::MOUSE);
    let candidate_id = candidates[0].candidate_id;

    // Push a commit request
    registry.push_commit_request(PointerId::MOUSE, candidate_id);

    // Verify pending exists
    let pending = registry.take_commit_requests(PointerId::MOUSE);
    assert_eq!(pending.len(), 1, "Should have 1 pending commit");

    // Re-push for the clear test
    registry.push_commit_request(PointerId::MOUSE, candidate_id);

    // Now clear session (simulating cancellation)
    // First drain pending, then clear candidates
    let drained = registry.take_commit_requests(PointerId::MOUSE);
    assert_eq!(drained.len(), 1, "Should drain pending commit");

    registry.clear_candidates(PointerId::MOUSE);

    // Verify no session, no pending
    assert!(
        !registry.has_session(PointerId::MOUSE),
        "Session should be cleared"
    );

    // Any attempt to get pending commits should return empty
    let after_clear = registry.take_commit_requests(PointerId::MOUSE);
    assert!(
        after_clear.is_empty(),
        "No pending commits should exist after clear"
    );
}

// ============================================================================
// Test: no_events_after_cancellation (INV-PC5)
// ============================================================================

/// INV-PC5: After cancellation, no further pointer events are delivered to
/// the cancelled widget (because capture is released).
#[test]
fn no_events_after_cancellation() {
    let _guard = LayoutAuthorityGuard::enter();
    let state = Rc::new(RefCell::new(CancellationTestState::default()));
    let (mut tree, root, _widget_id, mut domain_state, mut text_engine) =
        setup_widget_for_cancellation(state.clone(), Some(GestureKind::Drag), true, 5.0);

    let mut router = EventRouter::new();

    // PointerDown at (50, 50) - captures pointer
    send_pointer_down(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        50.0,
        50.0,
    );

    // Clear state to track only post-cancel events
    state.borrow_mut().events.clear();

    // Move event while captured - widget should receive it
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        55.0,
        55.0,
    );
    assert_eq!(
        state.borrow().events.len(),
        1,
        "Should receive move while captured"
    );

    state.borrow_mut().events.clear();

    // Cancel via Escape
    send_escape(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
    );

    // Verify capture released
    assert!(
        !domain_state
            .pointer_capture_manager
            .is_captured(PointerId::MOUSE)
    );

    state.borrow_mut().events.clear();

    // Move event OUTSIDE the widget bounds (e.g., at 200, 200)
    // With capture released, this should NOT be delivered to our widget
    send_pointer_move(
        &mut router,
        &mut tree,
        root,
        &mut text_engine,
        &mut domain_state,
        200.0,
        200.0,
    );

    // No events should be delivered since capture is released and pointer is outside widget
    assert!(
        state.borrow().events.is_empty(),
        "No events should be delivered to widget after cancellation when pointer is outside"
    );
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PositionedDragState {
    events: Vec<(&'static str, (f32, f32))>,
    down: Option<(f32, f32)>,
    value: (f32, f32),
    preview: Option<(f32, f32)>,
    gesture_commits: usize,
    application_commits: usize,
    candidate_cancels: usize,
    capture_cancels: Vec<(PointerId, CancellationReason, bool)>,
    rollbacks: usize,
    unmounts: usize,
}

struct PositionedAbsoluteDragProbe {
    internal_id: WidgetInternalId,
    state: Rc<RefCell<PositionedDragState>>,
    request_on_drag_start: bool,
}

impl PositionedAbsoluteDragProbe {
    fn rollback(&mut self, cx: &mut EventContext) {
        let changed = {
            let mut state = self.state.borrow_mut();
            state.down = None;
            let changed = state.preview.take().is_some();
            if changed {
                state.rollbacks += 1;
            }
            changed
        };
        if changed {
            cx.invalidate_paint();
        }
    }
}

impl WidgetDowncast for PositionedAbsoluteDragProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PositionedAbsoluteDragProbe {
    fn id(&self) -> WidgetInternalId {
        self.internal_id
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(20.0, 20.0))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let state = self.state.borrow();
        let offset = state.preview.unwrap_or(state.value);
        let color = if state.preview.is_some() {
            Color::from_linear(0.0, 1.0, 1.0, 1.0)
        } else {
            Color::WHITE
        };
        cx.scene.push_rect(
            RectF::new(
                rect.x + offset.0,
                rect.y + offset.1,
                rect.width,
                rect.height,
            ),
            color,
            0,
            None,
        );
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        &[GestureKind::Drag]
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        let Event::Pointer(pointer) = event else {
            return false;
        };
        let mut state = self.state.borrow_mut();
        match pointer.kind {
            PointerEventKind::Down if pointer.button == Some(PointerButton::Primary) => {
                state.events.push(("down", pointer.local_pos));
                state.down = Some(pointer.local_pos);
                true
            }
            PointerEventKind::DragStart if state.down.is_some() => {
                state.events.push(("dragstart", pointer.local_pos));
                if self.request_on_drag_start {
                    cx.request_gesture_commit();
                }
                true
            }
            PointerEventKind::Move if state.preview.is_some() => {
                state.events.push(("move", pointer.local_pos));
                let down = state.down.expect("committed handle retains its Down facts");
                state.preview = Some((
                    state.value.0 + pointer.local_pos.0 - down.0,
                    state.value.1 + pointer.local_pos.1 - down.1,
                ));
                true
            }
            PointerEventKind::Up if pointer.button == Some(PointerButton::Primary) => {
                state.events.push(("up", pointer.local_pos));
                state.down = None;
                if let Some(value) = state.preview.take() {
                    state.value = value;
                    state.application_commits += 1;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
        assert_eq!(kind, GestureKind::Drag);
        {
            let mut state = self.state.borrow_mut();
            assert!(state.down.is_some());
            state.gesture_commits += 1;
            state.preview = Some(state.value);
        }
        cx.invalidate_paint();
    }

    fn on_gesture_cancel(&mut self, cx: &mut EventContext, kind: GestureKind) {
        assert_eq!(kind, GestureKind::Drag);
        self.state.borrow_mut().candidate_cancels += 1;
        self.rollback(cx);
    }

    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        pointer_id: PointerId,
        reason: CancellationReason,
    ) {
        self.state.borrow_mut().capture_cancels.push((
            pointer_id,
            reason,
            cx.has_pointer_capture(),
        ));
        self.rollback(cx);
    }

    fn on_unmount(&mut self) {
        let mut state = self.state.borrow_mut();
        state.down = None;
        state.preview = None;
        state.unmounts += 1;
    }
}

struct PositionedDragParent {
    internal_id: WidgetInternalId,
    size: Size,
}

impl WidgetDowncast for PositionedDragParent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PositionedDragParent {
    fn id(&self) -> WidgetInternalId {
        self.internal_id
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        _constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        for (index, &child) in children.iter().enumerate() {
            let rect = if index == 0 {
                RectF::new(40.0, 30.0, 20.0, 20.0)
            } else {
                RectF::new(140.0, 100.0, 30.0, 30.0)
            };
            cx.layout_child(
                child,
                Constraints::tight(Size::new(rect.width, rect.height)),
            );
            cx.set_child_rect(child, rect);
        }
        LayoutResult::fit(self.size)
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
}

struct PositionedDragFixture {
    tree: WidgetTree,
    root: NodeId,
    handle: NodeId,
    sibling: NodeId,
    owner: TreeWidgetId,
    state: Rc<RefCell<PositionedDragState>>,
    domain: InteractionDomainState,
    router: EventRouter,
    fusion: InputFusion,
    text_engine: FontManagerTextEngine,
    bindings: BindingRegistry,
}

impl PositionedDragFixture {
    fn new(request_on_drag_start: bool) -> Self {
        let state = Rc::new(RefCell::new(PositionedDragState::default()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(PositionedDragParent {
                internal_id: WidgetInternalId::new(),
                size: Size::new(200.0, 160.0),
            }),
            None,
            0,
        );
        let handle = tree.add_child(
            root,
            Box::new(PositionedAbsoluteDragProbe {
                internal_id: WidgetInternalId::new(),
                state: state.clone(),
                request_on_drag_start,
            }),
            None,
            1,
        );
        let sibling = tree.add_child(
            root,
            Box::new(PositionedDragParent {
                internal_id: WidgetInternalId::new(),
                size: Size::new(30.0, 30.0),
            }),
            None,
            0,
        );
        let mut text_engine = FontManagerTextEngine::new();
        {
            let _layout = LayoutAuthorityGuard::enter();
            let theme = Theme::dark();
            LayoutContext::new(&mut tree, &mut text_engine, &theme)
                .layout_child(root, Constraints::tight(Size::new(200.0, 160.0)));
            halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let owner = tree.node(handle).unwrap().id();
        let mut fixture = Self {
            tree,
            root,
            handle,
            sibling,
            owner,
            state,
            domain: InteractionDomainState::new(),
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            text_engine,
            bindings: BindingRegistry::new(),
        };
        fixture.clean();
        fixture
    }

    fn route_all(&mut self, events: &[FusedInputEvent]) {
        for event in events {
            self.router.route_input_event(
                &mut self.tree,
                event.clone(),
                Some(self.root),
                &mut self.text_engine,
                None,
                &mut self.domain,
                &mut self.bindings,
            );
        }
    }

    fn input(&mut self, raw: RawInputEvent) -> Vec<FusedInputEvent> {
        let mut events = Vec::new();
        self.fusion.process_raw(raw, &mut events);
        self.fusion.flush(&mut events);
        self.route_all(&events);
        events
    }

    fn button(&mut self, state: RawButtonState, x: f32, y: f32) -> Vec<FusedInputEvent> {
        self.input(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position: Point::new(x, y),
        })
    }

    fn move_to(&mut self, x: f32, y: f32) -> Vec<FusedInputEvent> {
        self.input(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            device: PointerDevice::Mouse,
            position: Point::new(x, y),
            delta: (0.0, 0.0),
        })
    }

    fn cancel(&mut self) -> Vec<FusedInputEvent> {
        let mut events = Vec::new();
        self.fusion.cancel_drag(&mut events);
        self.fusion.flush(&mut events);
        self.route_all(&events);
        events
    }

    fn clean(&mut self) {
        for id in [self.root, self.handle, self.sibling] {
            let node = self.tree.node_mut(id).unwrap();
            node.dirty_layout = false;
            node.dirty_paint = false;
            node.dirty_composition = false;
            node.dirty_flags = WidgetDirty::Unchanged;
        }
    }

    fn assert_clean(&self, id: NodeId) {
        let node = self.tree.node(id).unwrap();
        assert!(!node.dirty_layout);
        assert!(!node.dirty_paint);
        assert!(!node.dirty_composition);
        assert_eq!(node.dirty_flags, WidgetDirty::Unchanged);
    }

    fn assert_session_cleared(&mut self) {
        assert_eq!(
            self.domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert_eq!(self.domain.pointer_capture.captured_widget, None);
        assert!(!self.domain.gesture_registry.has_session(PointerId::MOUSE));
        assert!(
            self.domain
                .gesture_registry
                .candidates(PointerId::MOUSE)
                .is_empty()
        );
        assert!(
            self.domain
                .gesture_registry
                .take_commit_requests(PointerId::MOUSE)
                .is_empty()
        );
    }

    fn painted_handle(&mut self) -> (RectF, halcyon::core::scene::Color) {
        let mut scene = SceneBuilder::new();
        let theme = Theme::dark();
        let icons = IconRegistry::new(&[]);
        let timeline = Timeline::new();
        let interpolated = InterpolatedVisualRects::default();
        let mut overlays = Vec::new();
        let mut tooltips = Vec::new();
        let mut cx = PaintContext::new(
            &self.tree,
            &mut scene,
            &theme,
            &mut self.text_engine,
            &icons,
            0,
            &timeline,
            &interpolated,
            &mut overlays,
            &mut tooltips,
            1.0,
            self.domain.focus_manager.focused(),
            self.domain.keyboard_mode,
        );
        self.tree.node(self.handle).unwrap().widget().paint(
            &mut cx,
            self.tree.get_absolute_paint_rect(self.handle).unwrap(),
            &[],
        );
        let graph = scene.build();
        let [SceneNode::Rect(rect)] = graph.root.as_slice() else {
            panic!("the handle must paint its actual preview projection");
        };
        (rect.rect, rect.color)
    }

    fn begin_outside_drag(&mut self) -> Vec<FusedInputEvent> {
        self.button(RawButtonState::Pressed, 50.0, 40.0);
        self.move_to(90.0, 70.0)
    }
}

#[test]
fn positioned_consumer_absolute_drag_start_outside_handle_commits_and_releases_once() {
    let mut f = PositionedDragFixture::new(true);
    let rect = f.tree.get_rect(f.handle).unwrap();
    let resting = f.painted_handle();
    f.button(RawButtonState::Pressed, 50.0, 40.0);
    assert_eq!(
        f.domain.gesture_registry.candidates(PointerId::MOUSE).len(),
        1
    );
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    assert_eq!(f.state.borrow().preview, None);
    assert_eq!(f.state.borrow().application_commits, 0);
    assert_eq!(f.painted_handle(), resting);

    let events = f.move_to(90.0, 70.0);
    assert!(matches!(
        events.as_slice(),
        [
            FusedInputEvent::DragStart { .. },
            FusedInputEvent::DragUpdate { .. }
        ]
    ));
    assert_eq!(
        f.state.borrow().events,
        vec![
            ("down", (10.0, 10.0)),
            ("dragstart", (10.0, 10.0)),
            ("move", (50.0, 40.0))
        ]
    );
    assert_eq!(f.state.borrow().gesture_commits, 1);
    assert_eq!(f.state.borrow().preview, Some((40.0, 30.0)));
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner)
    );
    assert_eq!(f.painted_handle().0, RectF::new(80.0, 60.0, 20.0, 20.0));

    f.clean();
    f.move_to(120.0, 90.0);
    assert_eq!(
        f.state.borrow().events.last(),
        Some(&("move", (80.0, 60.0)))
    );
    assert_eq!(f.state.borrow().preview, Some((70.0, 50.0)));
    assert_eq!(f.tree.get_rect(f.handle), Some(rect));
    for id in [f.root, f.handle, f.sibling] {
        assert!(!f.tree.node(id).unwrap().dirty_layout);
    }
    let release = f.button(RawButtonState::Released, 120.0, 90.0);
    assert!(matches!(
        release.as_slice(),
        [
            FusedInputEvent::DragEnd {
                cancelled: false,
                ..
            },
            FusedInputEvent::PointerUp { .. }
        ]
    ));
    assert_eq!(f.state.borrow().application_commits, 1);
    assert_eq!(f.state.borrow().value, (70.0, 50.0));
    assert_eq!(f.state.borrow().preview, None);
    f.assert_session_cleared();
    f.button(RawButtonState::Released, 50.0, 40.0);
    assert_eq!(f.state.borrow().application_commits, 1);
    assert_eq!(f.state.borrow().candidate_cancels, 0);
    assert!(f.state.borrow().capture_cancels.is_empty());
}

#[test]
fn positioned_consumer_absolute_cancel_after_commit_restores_and_invalidates() {
    let mut f = PositionedDragFixture::new(true);
    let resting = f.painted_handle();
    f.begin_outside_drag();
    assert_ne!(f.painted_handle(), resting);
    let independent_focus = f.tree.node(f.sibling).unwrap().id();
    f.domain.focus_manager.set_focus(Some(independent_focus));
    f.domain.keyboard_mode = KeyboardAuthorityMode::Capture;
    f.clean();

    let cancelled = f.cancel();
    assert!(matches!(
        cancelled.as_slice(),
        [FusedInputEvent::DragEnd {
            cancelled: true,
            ..
        }]
    ));
    {
        let state = f.state.borrow();
        assert_eq!(state.application_commits, 0);
        assert_eq!(state.value, (0.0, 0.0));
        assert_eq!(state.preview, None);
        assert_eq!(state.down, None);
        assert_eq!(state.candidate_cancels, 1);
        assert_eq!(
            state.capture_cancels,
            vec![(PointerId::MOUSE, CancellationReason::SystemCancel, false)]
        );
        assert_eq!(state.rollbacks, 1);
        assert!(!state.events.iter().any(|(kind, _)| *kind == "up"));
    }
    f.assert_session_cleared();
    assert!(f.tree.node(f.handle).unwrap().dirty_paint);
    assert!(f.tree.node(f.root).unwrap().dirty_composition);
    assert!(!f.tree.node(f.root).unwrap().dirty_paint);
    for id in [f.root, f.handle, f.sibling] {
        assert!(!f.tree.node(id).unwrap().dirty_layout);
    }
    f.assert_clean(f.sibling);
    assert_eq!(f.domain.focus_manager.focused(), Some(independent_focus));
    assert_eq!(f.domain.keyboard_mode, KeyboardAuthorityMode::Capture);
    assert_eq!(f.painted_handle(), resting);

    f.route_all(&cancelled);
    assert!(f.cancel().is_empty());
    f.button(RawButtonState::Released, 50.0, 40.0);
    assert_eq!(f.state.borrow().candidate_cancels, 1);
    assert_eq!(f.state.borrow().capture_cancels.len(), 1);
    assert_eq!(f.state.borrow().rollbacks, 1);
    assert_eq!(f.state.borrow().application_commits, 0);
    assert_eq!(f.state.borrow().preview, None);
    f.assert_session_cleared();
}

#[test]
fn positioned_consumer_absolute_cancel_before_commit_drains_pending_intent() {
    let mut f = PositionedDragFixture::new(false);
    let resting = f.painted_handle();
    let events = f.begin_outside_drag();
    assert!(matches!(
        events.as_slice(),
        [
            FusedInputEvent::DragStart { .. },
            FusedInputEvent::DragUpdate { .. }
        ]
    ));
    assert!(f.fusion.is_dragging());
    assert!(!f.domain.gesture_registry.is_committed(PointerId::MOUSE));
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    let candidates = f.domain.gesture_registry.candidates(PointerId::MOUSE);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].widget_id, f.owner);
    let candidate = candidates[0].candidate_id;
    // Host fixture setup, not consumer-side registry inspection.
    f.domain
        .gesture_registry
        .push_commit_request(PointerId::MOUSE, candidate);
    f.clean();

    let cancelled = f.cancel();
    assert!(matches!(
        cancelled.as_slice(),
        [FusedInputEvent::DragEnd {
            cancelled: true,
            ..
        }]
    ));
    assert_eq!(f.state.borrow().gesture_commits, 0);
    assert_eq!(f.state.borrow().application_commits, 0);
    assert_eq!(f.state.borrow().candidate_cancels, 1);
    assert!(f.state.borrow().capture_cancels.is_empty());
    assert_eq!(f.state.borrow().rollbacks, 0);
    assert_eq!(f.state.borrow().preview, None);
    assert_eq!(f.state.borrow().down, None);
    f.assert_session_cleared();
    for id in [f.root, f.handle, f.sibling] {
        f.assert_clean(id);
    }
    assert_eq!(f.painted_handle(), resting);
}

#[test]
fn positioned_consumer_absolute_cancel_after_owner_removal_is_inert() {
    let mut f = PositionedDragFixture::new(true);
    f.begin_outside_drag();
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner)
    );
    f.tree.remove_node(f.handle);
    let removed = f.tree.drain_pending_removals();
    assert_eq!(removed, vec![f.owner]);
    f.router.on_widgets_destroyed(&removed, &mut f.domain);
    assert!(f.tree.find_node_by_widget_id(f.owner).is_none());
    assert_eq!(f.state.borrow().unmounts, 1);
    assert_eq!(f.state.borrow().preview, None);
    let retired = f.state.borrow().clone();

    let cancelled = f.cancel();
    assert!(matches!(
        cancelled.as_slice(),
        [FusedInputEvent::DragEnd {
            cancelled: true,
            ..
        }]
    ));
    f.button(RawButtonState::Released, 50.0, 40.0);
    assert_eq!(*f.state.borrow(), retired);
    assert_eq!(f.state.borrow().application_commits, 0);
    assert_eq!(f.state.borrow().candidate_cancels, 0);
    assert!(f.state.borrow().capture_cancels.is_empty());
    f.assert_session_cleared();
}

struct GeometryAbsoluteProbe {
    inner: PositionedAbsoluteDragProbe,
    dependency: halcyon::core::widget::AbsoluteDragGeometryDependency,
    invalidate_on_commit_request: bool,
}

impl WidgetDowncast for GeometryAbsoluteProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for GeometryAbsoluteProbe {
    fn id(&self) -> WidgetInternalId {
        self.inner.id()
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        self.inner.layout(cx, constraints, children)
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        self.inner.paint(cx, rect, children);
    }
    fn gesture_surface(&self) -> &'static [GestureKind] {
        &[GestureKind::Drag]
    }
    fn absolute_drag_geometry_dependency(
        &self,
    ) -> Option<halcyon::core::widget::AbsoluteDragGeometryDependency> {
        Some(self.dependency)
    }
    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase == EventPhase::Target {
            if let Event::Pointer(pointer) = event {
                if matches!(
                    pointer.kind,
                    PointerEventKind::DoubleClick | PointerEventKind::TripleClick
                ) && pointer.button == Some(PointerButton::Primary)
                {
                    let mut state = self.inner.state.borrow_mut();
                    state.down = Some(pointer.local_pos);
                    state.events.push((
                        if pointer.kind == PointerEventKind::DoubleClick {
                            "double"
                        } else {
                            "triple"
                        },
                        pointer.local_pos,
                    ));
                    return true;
                }
            }
        }
        let handled = self.inner.handle_event(cx, event, phase);
        if self.invalidate_on_commit_request
            && phase == EventPhase::Target
            && matches!(event, Event::Pointer(pointer) if pointer.kind == PointerEventKind::DragStart)
        {
            cx.invalidate_layout();
        }
        handled
    }
    fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
        self.inner.on_gesture_commit(cx, kind);
    }
    fn on_gesture_cancel(&mut self, cx: &mut EventContext, kind: GestureKind) {
        self.inner.on_gesture_cancel(cx, kind);
    }
    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        pointer: PointerId,
        reason: CancellationReason,
    ) {
        self.inner.on_pointer_capture_cancel(cx, pointer, reason);
    }
    fn on_unmount(&mut self) {
        self.inner.on_unmount();
    }
}

impl PositionedDragFixture {
    fn geometry(invalidate_on_commit_request: bool) -> Self {
        let mut tree = WidgetTree::new();
        let state = Rc::new(RefCell::new(PositionedDragState::default()));
        let root = tree.create_root(
            Box::new(PositionedDragParent {
                internal_id: WidgetInternalId::new(),
                size: Size::new(200.0, 160.0),
            }),
            None,
            0,
        );
        let handle = tree.add_child(
            root,
            Box::new(GeometryAbsoluteProbe {
                inner: PositionedAbsoluteDragProbe {
                    internal_id: WidgetInternalId::new(),
                    state: state.clone(),
                    request_on_drag_start: true,
                },
                dependency: halcyon::core::widget::AbsoluteDragGeometryDependency {
                    viewport_owner: root,
                    reflow_root: root,
                },
                invalidate_on_commit_request,
            }),
            None,
            1,
        );
        let sibling = tree.add_child(
            root,
            Box::new(PositionedDragParent {
                internal_id: WidgetInternalId::new(),
                size: Size::new(30.0, 30.0),
            }),
            None,
            0,
        );
        let mut text_engine = FontManagerTextEngine::new();
        {
            let _layout = LayoutAuthorityGuard::enter();
            LayoutContext::new(&mut tree, &mut text_engine, &Theme::dark())
                .layout_child(root, Constraints::tight(Size::new(200.0, 160.0)));
            halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
            halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
            tree.compute_viewports(root);
        }
        let owner = tree.node(handle).unwrap().id();
        Self {
            tree,
            root,
            handle,
            sibling,
            owner,
            state,
            domain: InteractionDomainState::new(),
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            text_engine,
            bindings: BindingRegistry::new(),
        }
    }

    fn normalize_geometry(
        &mut self,
        raw: RawInputEvent,
    ) -> Vec<halcyon::core::event::SessionQualifiedInput> {
        let mut events = Vec::new();
        self.fusion.process_raw_with_gesture_sessions(
            raw,
            halcyon::core::event::InteractionDomainId::PRIMARY,
            &mut self.domain.gesture_registry,
            &self.tree,
            None,
            &mut events,
        );
        self.fusion.flush_with_gesture_sessions(&mut events);
        events
    }

    fn route_geometry(&mut self, events: &[halcyon::core::event::SessionQualifiedInput]) {
        for event in events {
            self.router.route_session_input_event(
                &mut self.tree,
                event.clone(),
                Some(self.root),
                &mut self.text_engine,
                None,
                &mut self.domain,
                &mut self.bindings,
            );
        }
    }

    fn geometry_button(
        &mut self,
        state: RawButtonState,
        time: Instant,
    ) -> Vec<halcyon::core::event::SessionQualifiedInput> {
        self.normalize_geometry(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position: Point::new(50.0, 40.0),
        })
    }

    fn geometry_move(&mut self, time: Instant) -> Vec<halcyon::core::event::SessionQualifiedInput> {
        self.normalize_geometry(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time,
            device: PointerDevice::Mouse,
            position: Point::new(90.0, 70.0),
            delta: (40.0, 30.0),
        })
    }
}

#[test]
fn geometry_dependent_absolute_reflow_before_threshold_does_not_retarget() {
    let mut f = PositionedDragFixture::geometry(false);
    let time = Instant::now();
    let down = f.geometry_button(RawButtonState::Pressed, time);
    f.route_geometry(&down);
    assert_eq!(
        f.domain.gesture_registry.candidates(PointerId::MOUSE)[0].widget_id,
        f.owner
    );
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    f.tree.mark_layout_dirty(f.root);
    let movement = f.geometry_move(time + std::time::Duration::from_millis(10));
    f.route_geometry(&movement);
    let up = f.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(20),
    );
    f.route_geometry(&up);
    f.assert_session_cleared();
    assert_eq!(f.state.borrow().gesture_commits, 0);
    assert_eq!(f.state.borrow().application_commits, 0);
    assert_eq!(f.state.borrow().candidate_cancels, 1);
    assert_eq!(f.state.borrow().down, None);
    assert!(
        !f.state
            .borrow()
            .events
            .iter()
            .any(|(kind, _)| *kind == "dragstart" || *kind == "up")
    );

    // Normalization's cutoff, rather than the later drain, owns this queued Down.
    let mut queued = PositionedDragFixture::geometry(false);
    let down = queued.geometry_button(RawButtonState::Pressed, time);
    queued.tree.mark_layout_dirty(queued.handle);
    queued.route_geometry(&down);
    let movement = queued.geometry_move(time + std::time::Duration::from_millis(10));
    queued.route_geometry(&movement);
    queued.assert_session_cleared();
    assert!(queued.state.borrow().events.is_empty());
}

#[test]
fn geometry_dependent_absolute_invalidity_beats_pending_commit() {
    let mut f = PositionedDragFixture::geometry(true);
    let time = Instant::now();
    let down = f.geometry_button(RawButtonState::Pressed, time);
    f.route_geometry(&down);
    let movement = f.geometry_move(time + std::time::Duration::from_millis(10));
    assert!(matches!(
        movement[0].event(),
        FusedInputEvent::DragStart { .. }
    ));
    f.route_geometry(&movement);
    assert_eq!(f.state.borrow().events.last().unwrap().0, "dragstart");
    assert_eq!(f.state.borrow().gesture_commits, 0);
    assert_eq!(f.state.borrow().candidate_cancels, 1);
    assert!(f.state.borrow().capture_cancels.is_empty());
    f.assert_session_cleared();
    let up = f.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(20),
    );
    f.route_geometry(&up);
    assert_eq!(f.state.borrow().application_commits, 0);
}

#[test]
fn geometry_dependent_absolute_retired_terminal_cannot_touch_replacement_session() {
    let mut f = PositionedDragFixture::geometry(false);
    let time = Instant::now();
    let down = f.geometry_button(RawButtonState::Pressed, time);
    f.route_geometry(&down);
    let movement = f.geometry_move(time + std::time::Duration::from_millis(10));
    f.route_geometry(&movement);
    let old_session = f
        .domain
        .gesture_registry
        .session_id(PointerId::MOUSE)
        .unwrap();
    let old_up = f.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(20),
    );
    f.tree.mark_layout_dirty(f.handle);
    let later_down = f.geometry_button(
        RawButtonState::Pressed,
        time + std::time::Duration::from_millis(30),
    );
    assert_eq!(
        f.domain.gesture_registry.session_id(PointerId::MOUSE),
        Some(old_session)
    );
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner)
    );
    assert!(matches!(
        later_down[0].event(),
        FusedInputEvent::PointerDown { click_count: 2, .. }
    ));
    f.route_geometry(&old_up);
    f.route_geometry(&later_down);
    let movement2 = f.geometry_move(time + std::time::Duration::from_millis(40));
    f.route_geometry(&movement2);
    let new_session = f
        .domain
        .gesture_registry
        .session_id(PointerId::MOUSE)
        .unwrap();
    assert_ne!(new_session, old_session);
    let retained_state = f.state.borrow().clone();
    f.route_geometry(&old_up);
    f.route_geometry(&movement);
    assert_eq!(*f.state.borrow(), retained_state);
    assert_eq!(
        f.domain.gesture_registry.session_id(PointerId::MOUSE),
        Some(new_session)
    );
    assert_eq!(
        f.domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(f.owner)
    );
    let up = f.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(50),
    );
    f.route_geometry(&up);
    assert_eq!(f.state.borrow().application_commits, 1);
    let triple = f.geometry_button(
        RawButtonState::Pressed,
        time + std::time::Duration::from_millis(60),
    );
    assert!(matches!(
        triple[0].event(),
        FusedInputEvent::PointerDown { click_count: 3, .. }
    ));
    f.route_geometry(&triple);
    let movement3 = f.geometry_move(time + std::time::Duration::from_millis(70));
    f.route_geometry(&movement3);
    assert!(
        f.state
            .borrow()
            .events
            .iter()
            .any(|(kind, _)| *kind == "double")
    );
    assert!(
        f.state
            .borrow()
            .events
            .iter()
            .any(|(kind, _)| *kind == "triple")
    );
    assert_eq!(f.state.borrow().candidate_cancels, 1);
    assert_eq!(f.state.borrow().capture_cancels.len(), 1);
    assert_eq!(f.state.borrow().rollbacks, 1);

    let mut registry_replacement = PositionedDragFixture::geometry(false);
    let unrouted = registry_replacement.geometry_button(RawButtonState::Pressed, time);
    let unrouted_up = registry_replacement.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(10),
    );
    registry_replacement.domain.gesture_registry = halcyon::core::event::GestureRegistry::new();
    let fresh = registry_replacement.geometry_button(
        RawButtonState::Pressed,
        time + std::time::Duration::from_secs(1),
    );
    registry_replacement.route_geometry(&fresh);
    let fresh_move =
        registry_replacement.geometry_move(time + std::time::Duration::from_millis(1010));
    registry_replacement.route_geometry(&fresh_move);
    let before_registry_replay = registry_replacement.state.borrow().clone();
    let registry_session = registry_replacement
        .domain
        .gesture_registry
        .session_id(PointerId::MOUSE);
    registry_replacement.route_geometry(&unrouted);
    registry_replacement.route_geometry(&unrouted_up);
    assert_eq!(*registry_replacement.state.borrow(), before_registry_replay);
    assert_eq!(
        registry_replacement
            .domain
            .gesture_registry
            .session_id(PointerId::MOUSE),
        registry_session
    );
    assert_eq!(
        registry_replacement
            .domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(registry_replacement.owner)
    );

    // Numeric IDs can restart in a different tree and registry; the seal cannot.
    let mut replacement = PositionedDragFixture::geometry(false);
    assert_eq!(replacement.owner, f.owner);
    let fresh = replacement.geometry_button(RawButtonState::Pressed, time);
    replacement.route_geometry(&fresh);
    let fresh_move = replacement.geometry_move(time + std::time::Duration::from_millis(10));
    replacement.route_geometry(&fresh_move);
    let before = replacement.state.borrow().clone();
    let current = replacement
        .domain
        .gesture_registry
        .session_id(PointerId::MOUSE);
    replacement.route_geometry(&old_up);
    replacement.route_geometry(&movement3);
    assert_eq!(*replacement.state.borrow(), before);
    assert_eq!(
        replacement
            .domain
            .gesture_registry
            .session_id(PointerId::MOUSE),
        current
    );
    assert_eq!(
        replacement
            .domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(replacement.owner)
    );
    replacement.tree.mark_layout_dirty(replacement.handle);
    replacement.route_geometry(&old_up);
    replacement.route_geometry(&movement3);
    assert_eq!(*replacement.state.borrow(), before);
    assert_eq!(
        replacement
            .domain
            .gesture_registry
            .session_id(PointerId::MOUSE),
        current,
        "a foreign retired envelope cannot run a newer session's checkpoint"
    );
    assert_eq!(
        replacement
            .domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(replacement.owner)
    );
    replacement.tree.remove_node(replacement.handle);
    let removed = replacement.tree.drain_pending_removals();
    replacement
        .router
        .on_widgets_destroyed(&removed, &mut replacement.domain);
    let retired = replacement.state.borrow().clone();
    replacement.route_geometry(&fresh_move);
    assert_eq!(*replacement.state.borrow(), retired);
    replacement.assert_session_cleared();

    let old_node = replacement.handle;
    let old_owner = replacement.owner;
    let retired_state = replacement.state.clone();
    let new_state = Rc::new(RefCell::new(PositionedDragState::default()));
    replacement.handle = replacement.tree.add_child(
        replacement.root,
        Box::new(GeometryAbsoluteProbe {
            inner: PositionedAbsoluteDragProbe {
                internal_id: WidgetInternalId::new(),
                state: new_state.clone(),
                request_on_drag_start: true,
            },
            dependency: halcyon::core::widget::AbsoluteDragGeometryDependency {
                viewport_owner: replacement.root,
                reflow_root: replacement.root,
            },
            invalidate_on_commit_request: false,
        }),
        None,
        1,
    );
    assert_eq!(replacement.handle, old_node);
    replacement.tree.remove_node(replacement.sibling);
    let removed = replacement.tree.drain_pending_removals();
    replacement
        .router
        .on_widgets_destroyed(&removed, &mut replacement.domain);
    replacement.owner = replacement.tree.node(replacement.handle).unwrap().id();
    replacement.state = new_state;
    assert_ne!(replacement.owner, old_owner);
    assert!(replacement.tree.find_node_by_widget_id(old_owner).is_none());
    {
        let _layout = LayoutAuthorityGuard::enter();
        LayoutContext::new(
            &mut replacement.tree,
            &mut replacement.text_engine,
            &Theme::dark(),
        )
        .layout_child(
            replacement.root,
            Constraints::tight(Size::new(200.0, 160.0)),
        );
        halcyon::core::app::compute_absolute_positions(
            &mut replacement.tree,
            replacement.root,
            Point::zero(),
        );
        halcyon::core::app::compute_absolute_paint_rects(
            &mut replacement.tree,
            replacement.root,
            Point::zero(),
        );
        replacement.tree.compute_viewports(replacement.root);
    }
    let release = replacement.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(20),
    );
    replacement.route_geometry(&release);
    let replacement_down = replacement.geometry_button(
        RawButtonState::Pressed,
        time + std::time::Duration::from_secs(1),
    );
    replacement.route_geometry(&replacement_down);
    let moved = replacement.geometry_move(time + std::time::Duration::from_millis(1010));
    replacement.route_geometry(&moved);
    let current = replacement
        .domain
        .gesture_registry
        .session_id(PointerId::MOUSE);
    let replacement_before_replay = replacement.state.borrow().clone();
    replacement.route_geometry(&fresh_move);
    replacement.route_geometry(&fresh);
    assert_eq!(*replacement.state.borrow(), replacement_before_replay);
    assert_eq!(
        replacement
            .domain
            .gesture_registry
            .session_id(PointerId::MOUSE),
        current
    );
    assert_eq!(*retired_state.borrow(), retired);
    let release = replacement.geometry_button(
        RawButtonState::Released,
        time + std::time::Duration::from_millis(1020),
    );
    replacement.route_geometry(&release);
    assert_eq!(replacement.state.borrow().application_commits, 1);
}

#[test]
fn geometry_dependent_absolute_qualified_secondary_down_preserves_ordinary_capture() {
    let actions = Rc::new(RefCell::new(0));
    let observed_actions = actions.clone();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            halcyon::widgets::button::Button::with_text("CAPTURE")
                .on_click(move || *observed_actions.borrow_mut() += 1),
        ),
        None,
        0,
    );
    let mut text = FontManagerTextEngine::new();
    let size = Size::new(200.0, 160.0);
    {
        let _layout = LayoutAuthorityGuard::enter();
        assert_eq!(
            LayoutContext::new(&mut tree, &mut text, &Theme::dark())
                .layout_child(root, Constraints::tight(size)),
            size
        );
        halcyon::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        halcyon::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);
    }
    let owner = tree.widget_id_for_node(root).unwrap();
    let mut domain = InteractionDomainState::new();
    let mut router = EventRouter::new();
    let mut fusion = InputFusion::new();
    let mut bindings = BindingRegistry::new();
    let time = Instant::now();
    let observations = [
        (PointerButton::Primary, RawButtonState::Pressed),
        (PointerButton::Secondary, RawButtonState::Pressed),
        (PointerButton::Primary, RawButtonState::Released),
        (PointerButton::Secondary, RawButtonState::Released),
    ];
    let mut queued = Vec::new();
    // Normalize the physical releases ahead of delivery too: observation is
    // not permission to retire the earlier, still semantically captured press.
    for (index, &(button, state)) in observations.iter().enumerate() {
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time: time + std::time::Duration::from_millis(index as u64 * 10),
                device: PointerDevice::Mouse,
                button,
                state,
                position: Point::new(50.0, 40.0),
            },
            halcyon::core::event::InteractionDomainId::PRIMARY,
            &mut domain.gesture_registry,
            &tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        assert_eq!(events.len(), 1);
        match (state, events[0].event()) {
            (RawButtonState::Pressed, FusedInputEvent::PointerDown { button: actual, .. })
            | (RawButtonState::Released, FusedInputEvent::PointerUp { button: actual, .. }) => {
                assert_eq!(*actual, button);
            }
            _ => panic!("physical button observation must preserve its own payload"),
        }
        queued.push(events.pop().unwrap());
    }
    let mut initiating_session = None;
    for (index, event) in queued.into_iter().enumerate() {
        router.route_session_input_event(
            &mut tree,
            event,
            Some(root),
            &mut text,
            None,
            &mut domain,
            &mut bindings,
        );
        if index == 0 {
            initiating_session = domain.gesture_registry.session_id(PointerId::MOUSE);
            assert!(initiating_session.is_some());
        }
        assert_eq!(*actions.borrow(), [0, 0, 1, 1][index]);
        if index < 2 {
            assert_eq!(
                domain.gesture_registry.session_id(PointerId::MOUSE),
                initiating_session
            );
            assert_eq!(
                domain
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                Some(owner)
            );
            assert_eq!(domain.pointer_capture.captured_widget, Some(owner));
            assert_eq!(domain.pointer_capture.button, Some(PointerButton::Primary));
        } else {
            assert!(!domain.gesture_registry.has_session(PointerId::MOUSE));
            assert_eq!(
                domain
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                None
            );
            assert_eq!(domain.pointer_capture.captured_widget, None);
            assert_eq!(domain.pointer_capture.button, None);
        }
    }
}
