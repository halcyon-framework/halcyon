use crate::core::app::RetainedSceneTestHarness;
use crate::core::event::{
    FusedInputEvent, InteractionDomainState, PointerButton, PointerDevice, ScrollSource, WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::math::{Point, Size};
use crate::core::rect_authority::UpdateAuthorityGuard;
use crate::core::scene::SceneNode;
use crate::core::theme::Theme;
use crate::core::widget::{UpdateContext, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::console::{Console, ConsoleConfig, ConsoleOverflowMode, ConsoleViewMode};
use crate::widgets::scroll_area::ScrollArea;
use std::time::Instant;

#[test]
fn console_capacity_override_survives_clear() {
    for console in [
        Console::new().with_capacity(3),
        Console::with_config(ConsoleConfig {
            capacity: 5,
            ..ConsoleConfig::default()
        })
        .with_capacity(3),
    ] {
        let initial_capacity = console.state.borrow().config.capacity;
        for text in ["before-a", "before-b", "before-c", "before-d"] {
            console.push_text(text, None);
        }
        {
            let state = console.state.borrow();
            assert_eq!(state.lines.len(), 3);
            assert_eq!(state.lines.capacity(), 3);
            assert_eq!(
                state
                    .lines
                    .iter()
                    .map(|line| line.spans[0].text.as_str())
                    .collect::<Vec<_>>(),
                ["before-b", "before-c", "before-d"],
            );
        }

        console.clear();
        assert!(console.state.borrow().lines.is_empty());
        let cleared_capacity = console.state.borrow().config.capacity;
        for text in ["after-a", "after-b", "after-c", "after-d"] {
            console.push_text(text, None);
        }
        let state = console.state.borrow();
        assert_eq!(
            state.lines.len(),
            3,
            "clear must preserve the builder's bound"
        );
        assert_eq!(state.lines.capacity(), 3);
        assert_eq!(
            state
                .lines
                .iter()
                .map(|line| line.spans[0].text.as_str())
                .collect::<Vec<_>>(),
            ["after-b", "after-c", "after-d"],
        );
        assert_eq!(
            [initial_capacity, cleared_capacity, state.config.capacity],
            [3; 3]
        );
        assert_eq!(state.view_mode, ConsoleViewMode::Live);
    }

    // Zero remains an accepted empty buffer; no new minimum or normalization.
    let zero = Console::new().with_capacity(0);
    assert_eq!(zero.state.borrow().lines.capacity(), 0);
    zero.clear();
    assert_eq!(zero.state.borrow().lines.capacity(), 0);
    assert_eq!(zero.state.borrow().config.capacity, 0);
}

fn scroll_input(x: f32, y: f32, delta_y: f32) -> FusedInputEvent {
    FusedInputEvent::Scroll {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(x, y),
        delta: (0.0, delta_y),
        source: ScrollSource::Wheel,
    }
}

fn pointer_down_input(x: f32, y: f32) -> FusedInputEvent {
    FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos: Point::new(x, y),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    }
}

fn scroll_target_y(tree: &WidgetTree, scroll_area: NodeId) -> f32 {
    tree.node(scroll_area)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap()
        .target_scroll_y()
}

fn first_scroll_y(nodes: &[SceneNode]) -> Option<f32> {
    for node in nodes {
        let found = match node {
            SceneNode::RetainedFragment(fragment) => first_scroll_y(&fragment.children),
            SceneNode::Clip(clip) => first_scroll_y(&clip.children),
            SceneNode::Layer(layer) => first_scroll_y(&layer.children),
            SceneNode::Scroll(scroll) => {
                return Some(scroll.offset.1);
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

#[test]
fn console_scroll_review_resume_cycle_does_not_require_resize_layout() {
    let mut console = Console::new();
    let state = console.state.clone();
    for index in 0..80 {
        console.push_text(&format!("line {index}"), None);
    }
    let body = console.create_body();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(console), None, 0);
    let scroll_area = tree.add_child(
        root,
        Box::new(
            ScrollArea::new_unwired()
                .with_scrollbars(true)
                .enable_horizontal_scrollbar(false),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll_area, Box::new(body), None, 0);
    tree.node_mut(scroll_area)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.node_mut(root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Console>()
        .unwrap()
        .set_scroll_area(scroll_area);

    let constraints = Constraints::tight(Size::new(320.0, 180.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let mut harness = RetainedSceneTestHarness::new();
    let mut router = EventRouter::new();
    let mut domain_state = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();

    let initial_frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let bottom = scroll_target_y(&tree, scroll_area);
    assert!(bottom > 0.0);
    assert_eq!(state.borrow().view_mode, ConsoleViewMode::Live);
    assert_eq!(first_scroll_y(&initial_frame.scene_root), Some(0.0));

    {
        let _update = UpdateAuthorityGuard::enter();
        tree.update_scroll_animations(1.0 / 60.0);
    }
    let following_frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let following_y = first_scroll_y(&following_frame.scene_root).unwrap();
    assert!(following_y > 0.0);

    router.route_input_event(
        &mut tree,
        scroll_input(160.0, 90.0, 80.0),
        Some(root),
        &mut text,
        None,
        &mut domain_state,
        &mut bindings,
    );
    assert!(scroll_target_y(&tree, scroll_area) < bottom);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(state.borrow().view_mode, ConsoleViewMode::Review);

    {
        let _update = UpdateAuthorityGuard::enter();
        let mut cx = UpdateContext::new(None);
        for dt in [10.0, 1.0] {
            tree.node_mut(root).unwrap().widget.update(dt, &mut cx);
        }
    }
    tree.mark_paint_dirty(root);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);

    let snapshot_len = state.borrow().snapshot.as_ref().unwrap().len();
    {
        let console = tree
            .node_mut(root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Console>()
            .unwrap();
        for index in 80..100 {
            console.push_text(&format!("line {index}"), None);
        }
    }
    tree.mark_layout_dirty(root);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    {
        let state = state.borrow();
        assert_eq!(state.view_mode, ConsoleViewMode::Review);
        assert_eq!(state.snapshot.as_ref().unwrap().len(), snapshot_len);
        assert_eq!(state.push_count - state.snapshot_push_count, 20);
    }

    {
        let _update = UpdateAuthorityGuard::enter();
        tree.update_scroll_animations(1.0 / 60.0);
    }
    let review_frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let review_y = first_scroll_y(&review_frame.scene_root).unwrap();

    router.route_input_event(
        &mut tree,
        pointer_down_input(160.0, 150.0),
        Some(root),
        &mut text,
        None,
        &mut domain_state,
        &mut bindings,
    );
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(state.borrow().view_mode, ConsoleViewMode::Live);
    assert!(scroll_target_y(&tree, scroll_area) > bottom);

    {
        let _update = UpdateAuthorityGuard::enter();
        tree.update_scroll_animations(1.0 / 60.0);
    }
    let resumed_frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(resumed_frame.scene_built);
    assert!(first_scroll_y(&resumed_frame.scene_root).unwrap() > review_y);

    router.route_input_event(
        &mut tree,
        scroll_input(160.0, 90.0, 80.0),
        Some(root),
        &mut text,
        None,
        &mut domain_state,
        &mut bindings,
    );
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(state.borrow().view_mode, ConsoleViewMode::Review);
}

#[test]
fn wrapped_console_measures_visible_rows_and_corrects_scroll_extent() {
    let config = ConsoleConfig {
        overflow: ConsoleOverflowMode::Wrap,
        ..ConsoleConfig::default()
    };
    let console = Console::with_config(config);
    let state = console.state.clone();
    for index in 0..24 {
        console.push_text(
            &format!(
                "wrapped entry {index}: {}",
                "alpha beta gamma delta epsilon zeta eta theta ".repeat(4)
            ),
            None,
        );
    }
    let body_widget = console.create_body();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(console), None, 0);
    let scroll_area = tree.add_child(
        root,
        Box::new(
            ScrollArea::new_unwired()
                .with_scrollbars(true)
                .enable_horizontal_scrollbar(false),
        ),
        None,
        0,
    );
    let body = tree.add_child(scroll_area, Box::new(body_widget), None, 0);
    tree.node_mut(scroll_area)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(body);
    tree.node_mut(root)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Console>()
        .unwrap()
        .set_scroll_area(scroll_area);

    let constraints = Constraints::tight(Size::new(220.0, 120.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let mut harness = RetainedSceneTestHarness::new();
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);

    let estimated_total = {
        let state = state.borrow();
        state.effective_line_count() as f32 * state.config.row_height
    };
    let measured_total = {
        let state = state.borrow();
        assert!(state.row_heights.fixed_height().is_none());
        (0..state.effective_line_count())
            .map(|index| state.row_heights.height_at(index))
            .sum::<f32>()
    };
    assert!(measured_total > estimated_total);
    assert!(tree.node(body).unwrap().widget.needs_update());

    let layout_invalidated = {
        let _update = UpdateAuthorityGuard::enter();
        let mut cx = UpdateContext::new(None);
        cx.begin_widget_update();
        tree.node_mut(body)
            .unwrap()
            .widget
            .update(1.0 / 60.0, &mut cx);
        let (_, _, layout_invalidated) = cx.take_widget_invalidations();
        layout_invalidated
    };
    assert!(layout_invalidated);
    tree.mark_layout_dirty(body);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);

    let scroll_area = tree
        .node(scroll_area)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ScrollArea>()
        .unwrap();
    assert!(scroll_area.content_extent_height() > estimated_total);
    assert!(scroll_area.test_max_scroll_y() > estimated_total - constraints.max_height);
}
