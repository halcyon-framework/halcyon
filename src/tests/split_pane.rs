use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
use crate::core::event::{
    CursorIcon, Event, EventPhase, FusedInputEvent, InteractionDomainState, KeyboardAuthorityMode,
    Modifiers, PointerButton, PointerButtonsState, PointerDevice, PointerEvent, PointerEventKind,
    PointerId, PointerType, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icons::builtin_registry;
use crate::core::layout::{Axis, Constraints, LayoutResult};
use crate::core::layout_transition::InterpolatedVisualRects;
use crate::core::math::{Point, RectF, Size};
use crate::core::rect_authority::{LayoutAuthorityGuard, PaintAuthorityGuard};
use crate::core::scene::{SceneGraph, SceneNode};
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::utils::id::WidgetInternalId;
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::split_pane::{SplitPane, SplitPaneDividerVisibility, SplitPaneInteractionPolicy};
use std::collections::HashMap;
use std::time::Instant;

const HANDLE_THICKNESS: f32 = 3.0;
const HANDLE_HIT_THICKNESS: f32 = 9.0;
const WORKBENCH_REPRO_WIDTH: f32 = 591.0;
const LEADING_MIN: f32 = 420.0;
const TRAILING_MIN: f32 = 220.0;

struct HitTargetPane {
    id: WidgetInternalId,
}

struct ViewportBoundedContent {
    id: WidgetInternalId,
}

impl ViewportBoundedContent {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for ViewportBoundedContent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ViewportBoundedContent {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        assert!(constraints.max_width.is_finite());
        assert!(constraints.max_height.is_finite());
        LayoutResult::new(
            Size::new(constraints.max_width, constraints.max_height),
            Size::new(constraints.max_width, 1_200.0),
        )
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }
}

impl HitTargetPane {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for HitTargetPane {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for HitTargetPane {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }
}

fn assert_approx_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.1,
        "expected {actual} to approximately equal {expected}"
    );
}

fn pointer_event(kind: PointerEventKind, local_pos: (f32, f32)) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: local_pos,
        local_pos,
        kind,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn pointer_move_input(x: f32, y: f32) -> FusedInputEvent {
    FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(x, y),
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::new(),
    }
}

fn build_workbench_repro_split() -> (WidgetTree, NodeId, NodeId, NodeId) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            SplitPane::horizontal(NodeId(0), NodeId(0))
                .initial_ratio(0.74)
                .leading_min(LEADING_MIN)
                .trailing_min(TRAILING_MIN),
        ),
        None,
        0,
    );
    let leading = tree.add_child(root, Box::new(HitTargetPane::new()), None, 0);
    let trailing = tree.add_child(root, Box::new(HitTargetPane::new()), None, 1);

    let split = tree
        .node_mut(root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<SplitPane>()
        .unwrap();
    split.leading = leading;
    split.trailing = trailing;

    (tree, root, leading, trailing)
}

fn layout_split(tree: &mut WidgetTree, root: NodeId) {
    test_begin_frame_layout_tracking();
    let _guard = LayoutAuthorityGuard::enter();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    layout_tree(
        tree,
        root,
        Constraints::tight(Size::new(WORKBENCH_REPRO_WIDTH, 240.0)),
        &theme,
        &mut text_engine,
        None,
    );
}

fn assert_finite_rect(rect: RectF) {
    assert!(rect.x.is_finite());
    assert!(rect.y.is_finite());
    assert!(rect.width.is_finite());
    assert!(rect.height.is_finite());
}

fn build_wired_scroll_split() -> (WidgetTree, NodeId, [NodeId; 2], [NodeId; 2]) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(SplitPane::vertical(NodeId(0), NodeId(0))), None, 0);
    let leading = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let leading_body = tree.add_child(leading, Box::new(ViewportBoundedContent::new()), None, 0);
    tree.node_mut(leading)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(leading_body);

    let trailing = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 1);
    let trailing_body = tree.add_child(trailing, Box::new(ViewportBoundedContent::new()), None, 0);
    tree.node_mut(trailing)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(trailing_body);

    let split = tree
        .node_mut(root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<SplitPane>()
        .unwrap();
    split.leading = leading;
    split.trailing = trailing;
    (
        tree,
        root,
        [leading, trailing],
        [leading_body, trailing_body],
    )
}

fn paint_split_root(tree: &mut WidgetTree, root: NodeId) -> SceneGraph {
    let root_rect = RectF::new(0.0, 0.0, WORKBENCH_REPRO_WIDTH, 240.0);
    let (widget, children, stored_rect) = tree.take_widget(root).unwrap();

    let mut scene = crate::core::scene::SceneBuilder::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let icon_registry = builtin_registry();
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: InterpolatedVisualRects = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();

    {
        let mut cx = PaintContext::new(
            tree,
            &mut scene,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            &mut overlay_regions,
            &mut tooltip_triggers,
            1.0,
            None,
            KeyboardAuthorityMode::Navigation,
        );
        let _paint = PaintAuthorityGuard::enter();
        widget.paint(&mut cx, root_rect, &children);
    }

    let graph = scene.build();
    {
        let _guard = LayoutAuthorityGuard::enter();
        tree.restore_widget(root, widget, stored_rect);
    }
    graph
}

fn route_split_pointer_event(tree: &mut WidgetTree, root: NodeId, event: Event) -> bool {
    let root_widget_id = tree.node(root).unwrap().id;
    let tree_ptr = tree as *mut WidgetTree;
    let (mut widget, _children, rect) = tree.take_widget(root).unwrap();
    let mut cx = EventContext::new(
        root_widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let handled = widget.handle_event(&mut cx, &event, EventPhase::Target);
    {
        let _guard = LayoutAuthorityGuard::enter();
        tree.restore_widget(root, widget, rect);
    }
    handled
}

#[test]
fn split_pane_proportionally_clamps_when_minimums_exceed_available() {
    let (mut tree, root, leading, trailing) = build_workbench_repro_split();

    layout_split(&mut tree, root);

    let leading_rect = tree.get_rect(leading).unwrap();
    let trailing_rect = tree.get_rect(trailing).unwrap();
    let available = WORKBENCH_REPRO_WIDTH - HANDLE_THICKNESS;
    let expected_leading = (available * LEADING_MIN / (LEADING_MIN + TRAILING_MIN)).round();

    assert!(leading_rect.width.is_finite());
    assert!(trailing_rect.width.is_finite());
    assert!(leading_rect.width >= 0.0);
    assert!(trailing_rect.width >= 0.0);
    assert_approx_eq(leading_rect.width, expected_leading);
    assert_approx_eq(trailing_rect.width, available - expected_leading);
    assert_approx_eq(trailing_rect.x, leading_rect.width + HANDLE_THICKNESS);
}

#[test]
fn split_pane_wired_scroll_areas_keep_measurement_and_final_clips_finite() {
    let (mut tree, root, scroll_areas, bodies) = build_wired_scroll_split();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();

    test_begin_frame_layout_tracking();
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            root,
            Constraints::new(345.0, 345.0, 0.0, f32::INFINITY),
            &theme,
            &mut text_engine,
            None,
        );
    }

    for child in scroll_areas {
        let rect = tree.get_rect(child).expect("measured ScrollArea rect");
        assert_finite_rect(rect);
        let clip = tree
            .node(child)
            .and_then(|node| node.widget.clip_bounds())
            .expect("measured ScrollArea clip");
        assert_finite_rect(clip);
    }
    for body in bodies {
        let constraints = tree
            .node(body)
            .and_then(|node| node.last_constraints)
            .expect("measured viewport-bound content constraints");
        assert_eq!(constraints.max_height, 0.0);
    }

    test_begin_frame_layout_tracking();
    {
        let _guard = LayoutAuthorityGuard::enter();
        layout_tree(
            &mut tree,
            root,
            Constraints::tight(Size::new(345.0, 600.0)),
            &theme,
            &mut text_engine,
            None,
        );
    }

    for child in scroll_areas {
        let rect = tree.get_rect(child).expect("final ScrollArea rect");
        assert_finite_rect(rect);
        assert!(rect.width > 0.0);
        assert!(rect.height > 0.0);
        let clip = tree
            .node(child)
            .and_then(|node| node.widget.clip_bounds())
            .expect("final ScrollArea clip");
        assert_finite_rect(clip);
        assert_approx_eq(clip.width, rect.width);
        assert_approx_eq(clip.height, rect.height);
    }
}

#[test]
fn split_pane_drag_does_not_panic_when_minimums_exceed_available() {
    let (mut tree, root, leading, trailing) = build_workbench_repro_split();

    layout_split(&mut tree, root);

    let leading_rect = tree.get_rect(leading).unwrap();
    let root_widget_id = tree.node(root).unwrap().id;
    let tree_ptr = &mut tree as *mut WidgetTree;
    let (mut widget, _children, rect) = tree.take_widget(root).unwrap();

    let down = pointer_event(
        PointerEventKind::Down,
        (leading_rect.width + 1.0, leading_rect.y + 12.0),
    );
    let mut cx = EventContext::new(
        root_widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    assert!(widget.handle_event(&mut cx, &down, EventPhase::Target));

    let drag = pointer_event(
        PointerEventKind::Move,
        (leading_rect.width + 64.0, leading_rect.y + 12.0),
    );
    let mut cx = EventContext::new(
        root_widget_id,
        tree_ptr,
        0,
        std::ptr::null_mut(),
        None,
        None,
        true,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    assert!(widget.handle_event(&mut cx, &drag, EventPhase::Target));

    {
        let _guard = LayoutAuthorityGuard::enter();
        tree.restore_widget(root, widget, rect);
    }

    layout_split(&mut tree, root);

    let leading_rect = tree.get_rect(leading).unwrap();
    let trailing_rect = tree.get_rect(trailing).unwrap();
    assert!(leading_rect.width.is_finite());
    assert!(trailing_rect.width.is_finite());
    assert!(leading_rect.width >= 0.0);
    assert!(trailing_rect.width >= 0.0);
}

#[test]
fn split_pane_hover_cursor_covers_expanded_hit_zone() {
    let (mut tree, root, leading, _trailing) = build_workbench_repro_split();

    layout_split(&mut tree, root);

    let leading_rect = tree.get_rect(leading).unwrap();
    let hit_expansion = (HANDLE_HIT_THICKNESS - HANDLE_THICKNESS) / 2.0;
    let hover_x = leading_rect.width - hit_expansion + 1.0;
    assert!(hover_x < leading_rect.width);

    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut bindings = BindingRegistry::new();

    router.route_input_event(
        &mut tree,
        pointer_move_input(hover_x, leading_rect.y + 12.0),
        Some(root),
        &mut text_engine,
        None,
        &mut domain_state,
        &mut bindings,
    );

    assert_eq!(router.take_requested_cursor(), Some(CursorIcon::ColResize));
}

#[test]
fn split_pane_idle_separator_is_hidden_until_hover_or_active_state() {
    let (mut tree, root, leading, _trailing) = build_workbench_repro_split();

    layout_split(&mut tree, root);

    let idle_graph = paint_split_root(&mut tree, root);
    assert_eq!(
        idle_graph.root.len(),
        0,
        "idle SplitPane must not add a seam line over child-owned borders"
    );

    let leading_rect = tree.get_rect(leading).unwrap();
    let hover = pointer_event(
        PointerEventKind::Move,
        (leading_rect.width + 1.0, leading_rect.y + 12.0),
    );
    assert!(route_split_pointer_event(&mut tree, root, hover));

    let hover_graph = paint_split_root(&mut tree, root);
    assert_eq!(
        hover_graph.root.len(),
        1,
        "hovered SplitPane must still expose a visible handle affordance"
    );
    match &hover_graph.root[0] {
        SceneNode::RoundedRect(rect) => {
            assert_approx_eq(rect.rect.width, 1.0);
            assert_approx_eq(rect.rect.height, 240.0);
        }
        other => panic!("expected hovered SplitPane handle to be a rounded rect, got {other:?}"),
    }
}

#[test]
fn split_pane_divider_visibility_changes_only_paint_admission() {
    use SplitPaneDividerVisibility::{Always, OnInteraction};

    let theme = Theme::default();
    for axis in [Axis::Horizontal, Axis::Vertical] {
        let mut allocations = None;
        for divider_visibility in [OnInteraction, Always] {
            let mut tree = WidgetTree::new();
            let split = match axis {
                Axis::Horizontal => SplitPane::horizontal(NodeId(0), NodeId(0)),
                Axis::Vertical => SplitPane::vertical(NodeId(0), NodeId(0)),
            };
            let root = tree.create_root(
                Box::new(split.with_interaction_policy(SplitPaneInteractionPolicy {
                    divider_visibility,
                })),
                None,
                0,
            );
            let leading = tree.add_child(root, Box::new(HitTargetPane::new()), None, 0);
            let trailing = tree.add_child(root, Box::new(HitTargetPane::new()), None, 1);
            let split = tree
                .node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<SplitPane>()
                .unwrap();
            split.leading = leading;
            split.trailing = trailing;
            layout_split(&mut tree, root);

            let front = tree.get_rect(leading).unwrap();
            let back = tree.get_rect(trailing).unwrap();
            if let Some(expected) = allocations {
                assert_eq!((front, back), expected);
            }
            allocations = Some((front, back));
            let (gutter, line, center) = match axis {
                Axis::Horizontal => (
                    back.x - front.right(),
                    RectF::new(front.right() + 1.0, 0.0, 1.0, 240.0),
                    front.right() + 1.5,
                ),
                Axis::Vertical => (
                    back.y - front.bottom(),
                    RectF::new(0.0, front.bottom() + 1.0, WORKBENCH_REPRO_WIDTH, 1.0),
                    front.bottom() + 1.5,
                ),
            };
            assert_eq!(gutter, HANDLE_THICKNESS);
            let at = |offset, kind| {
                let along = center + offset;
                let pos = match axis {
                    Axis::Horizontal => (along, 12.0),
                    Axis::Vertical => (12.0, along),
                };
                pointer_event(kind, pos)
            };
            let assert_paint = |tree: &mut WidgetTree, color: Option<crate::core::paint::Color>| {
                let graph = paint_split_root(tree, root);
                match (color, graph.root.as_slice()) {
                    (Some(color), [SceneNode::RoundedRect(bar)]) => {
                        assert_eq!(bar.rect, line);
                        assert_eq!(bar.color, color.into());
                    }
                    (None, []) => {}
                    _ => panic!("unexpected divider paint: {:?}", graph.root),
                }
            };
            let idle = (divider_visibility == Always).then_some(theme.colors.borders.default);
            assert_paint(&mut tree, idle);
            let edge = HANDLE_HIT_THICKNESS / 2.0;
            assert!(!route_split_pointer_event(
                &mut tree, root, at(edge + 0.25, PointerEventKind::Down)
            ));
            assert!(route_split_pointer_event(
                &mut tree, root, at(edge - 0.25, PointerEventKind::Move)
            ));
            assert_paint(&mut tree, Some(theme.colors.borders.strong));
            route_split_pointer_event(&mut tree, root, at(30.0, PointerEventKind::Move));
            assert_paint(&mut tree, idle);
            assert!(route_split_pointer_event(
                &mut tree, root, at(edge - 0.25, PointerEventKind::Down)
            ));
            assert_paint(&mut tree, Some(theme.colors.accents.primary));
            assert!(route_split_pointer_event(&mut tree, root, at(30.0, PointerEventKind::Move)));
            assert!(route_split_pointer_event(&mut tree, root, at(30.0, PointerEventKind::Up)));
            layout_split(&mut tree, root);
            let moved = tree.get_rect(leading).unwrap();
            match axis {
                Axis::Horizontal => assert!(moved.width > front.width),
                Axis::Vertical => assert!(moved.height > front.height),
            }
        }
    }
}

mod explicit_focus {
    use super::*;
    use crate::core::app::app_root_overlay_input_tests::{
        RecordingInputScreen, dispatch_physical_raw,
    };
    use crate::core::app::{AppContext, AppRoot, RetainedSceneTestHarness};
    use crate::core::event::{
        InteractionDomains, KeyCode, KeyState, PhysicalKeyCode, RawButtonState, RawInputEvent,
    };
    use crate::core::icon::IconRegistry;
    use crate::core::input_fusion::InputFusion;
    use crate::core::layout::FlexParams;
    use crate::utils::id::TreeWidgetId;
    use crate::widgets::{flex::Flex, screen_host::ScreenHost, text_field::TextField};
    use std::{cell::RefCell, rc::Rc, time::Duration};

    struct Fixture {
        tree: WidgetTree,
        root: NodeId,
        split: NodeId,
        leading: NodeId,
        field: NodeId,
        domains: InteractionDomains,
        app: AppRoot,
        router: EventRouter,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        retained: RetainedSceneTestHarness,
        time: Instant,
    }

    impl Fixture {
        fn new() -> Self {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let split = tree.add_child(
                root,
                Box::new(SplitPane::horizontal(NodeId(0), NodeId(0))),
                None,
                0,
            );
            tree.set_flex(split, FlexParams::fixed(200.0));
            let leading = tree.add_child(split, Box::new(HitTargetPane::new()), None, 0);
            let trailing = tree.add_child(split, Box::new(HitTargetPane::new()), None, 1);
            let pane = tree
                .node_mut(split)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<SplitPane>()
                .unwrap();
            pane.leading = leading;
            pane.trailing = trailing;
            let field = tree.add_child(root, Box::new(TextField::new().with_text("kept")), None, 1);
            let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
            app.push_screen(Box::new(RecordingInputScreen {
                host: ScreenHost::new(),
                events: Rc::new(RefCell::new(Vec::new())),
            }));
            let mut f = Self {
                tree,
                root,
                split,
                leading,
                field,
                domains: InteractionDomains::new(),
                app,
                router: EventRouter::new(),
                fusion: InputFusion::new(),
                text: FontManagerTextEngine::new(),
                retained: RetainedSceneTestHarness::new(),
                time: Instant::now(),
            };
            f.frame();
            f
        }

        fn frame(&mut self) {
            self.retained.frame_with_appearance(
                &mut self.tree,
                self.root,
                Constraints::tight(Size::new(627.0, 280.0)),
                &Theme::dark(),
                &mut self.text,
                &IconRegistry::new(&[]),
                &self.domains,
                WindowId::PRIMARY,
            );
        }

        fn owner(&self, node: NodeId) -> TreeWidgetId {
            self.tree.widget_id_for_node(node).unwrap()
        }

        fn width(&self) -> f32 {
            self.tree.get_rect(self.leading).unwrap().width
        }

        fn center(&self, node: NodeId) -> Point {
            let r = self.tree.get_absolute_paint_rect(node).unwrap();
            Point::new(r.x + r.width * 0.5, r.y + r.height * 0.5)
        }

        fn divider(&self) -> Point {
            let r = self.tree.get_absolute_paint_rect(self.leading).unwrap();
            Point::new(r.x + r.width + HANDLE_THICKNESS * 0.5, r.y + 50.0)
        }

        fn next_time(&mut self) -> Instant {
            // Distinct fresh clicks, without sleeping or exercising multi-click policy.
            self.time += Duration::from_secs(1);
            self.time
        }

        fn raw(&mut self, event: RawInputEvent) {
            dispatch_physical_raw(
                &mut self.app,
                &mut self.tree,
                &mut self.router,
                self.domains.primary_state_mut(),
                &mut self.fusion,
                &mut self.text,
                event,
            );
        }

        fn button(&mut self, p: Point, down: bool) {
            let time = self.next_time();
            self.raw(RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: if down {
                    RawButtonState::Pressed
                } else {
                    RawButtonState::Released
                },
                position: p,
            });
        }

        fn click(&mut self, p: Point) {
            self.button(p, true);
            self.button(p, false);
        }

        fn key(&mut self, key: KeyCode) {
            for state in [KeyState::Down, KeyState::Up] {
                let time = self.next_time();
                self.raw(RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: key.clone(),
                    physical_key: PhysicalKeyCode::Unknown,
                    reservation: None,
                    state,
                    repeat: false,
                    modifiers: Modifiers::default(),
                });
            }
        }

        fn field_text(&self) -> &str {
            self.tree
                .node(self.field)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap()
                .text()
        }

        fn drag_divider(&mut self) {
            let p = self.divider();
            self.button(p, true);
            assert_eq!(
                self.domains.primary_state().focus_manager.focused(),
                Some(self.owner(self.split))
            );
            assert_eq!(
                self.domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                Some(self.owner(self.split))
            );
            assert_eq!(
                self.domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            let moved = Point::new(p.x + 32.0, p.y);
            let time = self.next_time();
            self.raw(RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: moved,
                delta: (32.0, 0.0),
            });
            self.button(moved, false);
            assert_eq!(
                self.domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                None
            );
            self.frame();
        }
    }

    #[test]
    fn hal_explicit_focus_request_splitpane_keyboard_lifecycle() {
        let mut f = Fixture::new();
        assert_eq!(f.width(), 312.0);
        f.click(f.center(f.leading));
        assert_eq!(f.domains.primary_state().focus_manager.focused(), None);
        f.key(KeyCode::Right);
        f.frame();
        assert_eq!(
            f.width(),
            312.0,
            "panel content must not focus its SplitPane"
        );

        f.drag_divider();
        assert_eq!(f.width(), 344.0);
        f.key(KeyCode::Right);
        f.frame();
        assert_eq!(f.width(), 352.0, "routed Right requires real on_focus");

        // The same-owner request is a lifecycle no-op and does not reset the ratio.
        f.click(f.divider());
        f.frame();
        assert_eq!(f.width(), 352.0);
        f.key(KeyCode::Right);
        f.frame();
        assert_eq!(f.width(), 360.0);
    }

    #[test]
    fn hal_explicit_focus_request_capture_release() {
        let mut f = Fixture::new();
        f.click(f.center(f.field));
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(f.owner(f.field))
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        f.key(KeyCode::End);
        f.key(KeyCode::Backspace);
        assert_eq!(f.field_text(), "kep");

        f.drag_divider();
        assert_eq!(f.width(), 344.0);
        f.key(KeyCode::Right);
        f.frame();
        assert_eq!(f.width(), 352.0);
        assert_eq!(f.field_text(), "kep", "old editor must no longer own keys");

        f.click(f.center(f.field));
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        f.key(KeyCode::End);
        f.key(KeyCode::Backspace);
        assert_eq!(
            f.field_text(),
            "ke",
            "ordinary TextField Capture still edits"
        );
        let width = f.width();
        f.key(KeyCode::Right);
        f.frame();
        assert_eq!(
            f.width(),
            width,
            "blurred SplitPane must not receive editor keys"
        );
    }

    #[test]
    fn hal_explicit_focus_request_capture_release_lifecycle_order() {
        type Lifecycle = (&'static str, &'static str, KeyboardAuthorityMode);
        struct Probe {
            id: WidgetInternalId,
            name: &'static str,
            capturable: bool,
            events: Rc<RefCell<Vec<Lifecycle>>>,
        }
        impl WidgetDowncast for Probe {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
        impl Widget for Probe {
            fn id(&self) -> WidgetInternalId {
                self.id
            }
            fn layout(
                &mut self,
                _cx: &mut LayoutContext,
                _constraints: Constraints,
                _children: &[NodeId],
            ) -> LayoutResult {
                LayoutResult::fit(Size::new(100.0, 50.0))
            }
            fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
            fn handle_event(
                &mut self,
                cx: &mut EventContext,
                event: &Event,
                phase: EventPhase,
            ) -> bool {
                if phase == EventPhase::Target
                    && matches!(event, Event::Pointer(p) if p.kind == PointerEventKind::Down)
                {
                    cx.request_focus();
                    if self.capturable {
                        cx.request_keyboard_capture();
                    }
                    return true;
                }
                false
            }
            fn capture_eligible(&self) -> bool {
                self.capturable
            }
            fn on_focus(&mut self, cx: &mut EventContext) {
                self.events
                    .borrow_mut()
                    .push((self.name, "focus", cx.keyboard_mode));
            }
            fn on_blur(&mut self, cx: &mut EventContext) {
                self.events
                    .borrow_mut()
                    .push((self.name, "blur", cx.keyboard_mode));
            }
            fn on_capture_enter(&mut self, cx: &mut EventContext) {
                self.events
                    .borrow_mut()
                    .push((self.name, "enter", cx.keyboard_mode));
            }
            fn on_capture_leave(&mut self, cx: &mut EventContext) {
                self.events
                    .borrow_mut()
                    .push((self.name, "leave", cx.keyboard_mode));
            }
        }
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::row()), None, 0);
        let mut nodes = Vec::new();
        for (name, capturable) in [("old", true), ("new", false)] {
            nodes.push(tree.add_child(
                root,
                Box::new(Probe {
                    id: WidgetInternalId::new(),
                    name,
                    capturable,
                    events: Rc::clone(&events),
                }),
                None,
                0,
            ));
        }
        let mut domains = InteractionDomains::new();
        let mut text = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame_with_appearance(
            &mut tree,
            root,
            Constraints::tight(Size::new(200.0, 50.0)),
            &Theme::dark(),
            &mut text,
            &IconRegistry::new(&[]),
            &domains,
            WindowId::PRIMARY,
        );
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let time = Instant::now();
        for (step, node) in [nodes[0], nodes[1], nodes[1]].into_iter().enumerate() {
            let r = tree.get_absolute_paint_rect(node).unwrap();
            router.route_input_event(
                &mut tree,
                FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY,
                    time: time + Duration::from_secs(step as u64),
                    pos: Point::new(r.x + r.width * 0.5, r.y + r.height * 0.5),
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    click_count: 1,
                },
                Some(root),
                &mut text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
            let state = domains.primary_state();
            assert_eq!(state.focus_manager.focused(), tree.widget_id_for_node(node));
            if step == 0 {
                assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Capture);
                assert_eq!(
                    &*events.borrow(),
                    &[
                        ("old", "focus", KeyboardAuthorityMode::Navigation),
                        ("old", "enter", KeyboardAuthorityMode::Capture),
                    ]
                );
                events.borrow_mut().clear();
            } else {
                assert_eq!(state.keyboard_mode, KeyboardAuthorityMode::Navigation);
                assert_eq!(
                    &*events.borrow(),
                    &[
                        ("old", "leave", KeyboardAuthorityMode::Navigation),
                        ("old", "blur", KeyboardAuthorityMode::Navigation),
                        ("new", "focus", KeyboardAuthorityMode::Navigation),
                    ],
                    "the repeated explicit request must not deliver another lifecycle"
                );
            }
        }
    }
}
