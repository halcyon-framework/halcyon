use super::EventRouter;
use crate::core::app::{RetainedSceneTestHarness, next_qualified_widget_input};
use crate::core::event::{
    FusedInputEvent, InteractionDomainState, Modifiers, PointerButton, PointerButtonsState,
    PointerDevice, PointerId, RawButtonState, RawInputEvent, WindowId,
};
use crate::core::icons::builtin_registry;
use crate::core::input_fusion::InputFusion;
use crate::core::layout::{Constraints, FlexParams};
use crate::core::math::{Point, RectF, Size};
use crate::core::scene::{SceneNode, TextNode};
use crate::core::text_engine::TextEngine;
use crate::core::text_selection::x_for_offset;
use crate::core::theme::Theme;
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::runtime::binding_registry::BindingRegistry;
use crate::widgets::event_log::{EventLog, EventLogConfig, EventLogEntry, EventLogLevel};
use crate::widgets::flex::Flex;
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::text_block::TextBlock;
use web_time::Instant;

fn gather_text<'a>(nodes: &'a [SceneNode], output: &mut Vec<&'a TextNode>) {
    for node in nodes {
        match node {
            SceneNode::Text(text) => output.push(text),
            SceneNode::Clip(clip) => gather_text(&clip.children, output),
            SceneNode::RetainedFragment(fragment) => gather_text(&fragment.children, output),
            _ => {}
        }
    }
}

#[test]
fn eventlog_expanded_detail_select_copy_routes_to_textblock() {
    prove_eventlog_detail_selection(false);
}

#[test]
fn text_selection_palette_eventlog_detail_survives_theme_change() {
    prove_eventlog_detail_selection(true);
}

fn prove_eventlog_detail_selection(verify_palette: bool) {
    const DETAIL: &str = "alpha\n\nβeta\nγ";
    const TARGET_ROW: usize = 3;
    let mut log = EventLog::new(EventLogConfig::default());
    for index in 0..18 {
        let row = EventLogEntry::new(
            EventLogLevel::Info,
            0,
            "source".into(),
            format!("row {index}"),
        );
        log.push(if index == TARGET_ROW {
            row.with_detail(DETAIL.to_owned())
        } else {
            row
        });
    }
    let body_widget = log.create_body_widget();
    let footer = log.create_footer_widget();
    let list_widget = log.take_list_view();
    let list_core = list_widget.test_core_weak().upgrade().unwrap();

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Flex::column()), None, 0);
    let list = tree.add_child(root, Box::new(list_widget), None, 0);
    tree.set_flex(list, FlexParams::flexible(1.0));
    let scroll = tree.add_child(list, Box::new(ScrollArea::new_unwired()), None, 0);
    let body = tree.add_child(scroll, Box::new(body_widget), None, 0);
    let scroll_area = tree
        .node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap();
    scroll_area.wire_content(body);
    tree.add_child(list, Box::new(footer), None, 0);

    let theme = Theme::light();
    let icons = builtin_registry();
    let mut text_engine = FontManagerTextEngine::new();
    let metrics = text_engine.metrics("", 13.0);
    let line_height = metrics.ascent + metrics.descent;
    let mut frames = RetainedSceneTestHarness::new();
    let mut router = EventRouter::new();
    router
        .clipboard
        .as_mut()
        .unwrap()
        .use_memory_transport_for_test("sentinel");
    let mut domain = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();

    macro_rules! frame {
        ($width:expr) => {
            frames.frame(
                &mut tree,
                root,
                Constraints::tight(Size::new($width, 232.0)),
                &theme,
                &mut text_engine,
                &icons,
            )
        };
    }
    macro_rules! route {
        ($event:expr) => {
            router.route_input_event(
                &mut tree,
                $event,
                Some(root),
                &mut text_engine,
                None,
                &mut domain,
                &mut bindings,
            )
        };
    }
    let down = |pos, click_count| FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count,
    };
    let drag_to = |pos| FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState {
            primary: true,
            ..Default::default()
        },
    };
    let up = |pos| FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    let copy = || FusedInputEvent::TextInput {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        text: "c".into(),
        modifiers: Modifiers {
            meta: cfg!(target_os = "macos"),
            ctrl: !cfg!(target_os = "macos"),
            ..Default::default()
        },
    };

    frame!(420.0);
    let row_height = list_core.borrow().item_height;
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .scroll_to(0.0, 2.0 * row_height);
    tree.mark_layout_dirty(scroll);
    frame!(420.0);

    let body_rect = tree.get_absolute_paint_rect(body).unwrap();
    let (_, scroll_y) = tree.cumulative_scroll_offset(body);
    assert!(scroll_y > 0.0);
    let header = Point::new(
        body_rect.x + 80.0,
        body_rect.y + (TARGET_ROW as f32 + 0.5) * row_height - scroll_y,
    );
    assert_eq!(
        tree.hit_test(header.x, header.y).unwrap().widget_id,
        tree.widget_id_for_node(body).unwrap()
    );
    route!(down(header, 1));
    route!(up(header));
    let row = list_core.borrow().focused_row().unwrap();
    assert!(list_core.borrow().is_selected(row));
    assert_eq!(
        log.format_rows_for_copy(&[row]),
        "[00:00:00.000] [INFO] [source] row 3"
    );
    let row_focus = domain.focus_manager.focused().unwrap();
    route!(down(header, 2));
    route!(up(header));
    frame!(420.0);

    let detail = tree
        .children(body)
        .into_iter()
        .find(|&id| {
            tree.node(id)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextBlock>()
                .is_some_and(|block| block.text() == DETAIL)
        })
        .expect("expanded EventLog detail must mount its selectable TextBlock");
    let owner = tree.widget_id_for_node(detail).unwrap();
    let displayed_detail = |tree: &WidgetTree| {
        let geometry = tree.own_presentation_geometry(detail, 1.0).unwrap();
        let (x, y) = geometry.cumulative_scroll;
        assert!(y > 0.0);
        let rect = geometry.rect;
        RectF::new(rect.x - x, rect.y - y, rect.width, rect.height)
    };
    let rect = displayed_detail(&tree);
    let first = Point::new(rect.x + 0.1, rect.y + line_height * 0.5);
    let blank = Point::new(rect.x + 7.0, rect.y + line_height * 1.5);
    let last = Point::new(rect.x + 20.0, rect.y + line_height * 3.5);
    assert_eq!(tree.hit_test(blank.x, blank.y).unwrap().widget_id, owner);
    route!(down(blank, 1));
    assert_eq!(domain.focus_manager.focused(), Some(owner));
    assert_eq!(
        domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    route!(drag_to(last));
    route!(up(last));
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some("\nβeta\nγ")
    );

    let successor = frame!(420.0);
    let fragment = frames.local_fragment(owner).unwrap();
    let mut runs = Vec::new();
    gather_text(&fragment, &mut runs);
    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
        ["alpha", "βeta", "γ"]
    );
    assert!(runs[0].backgrounds.is_empty());
    assert!(
        runs[1..]
            .iter()
            .all(|run| run.backgrounds.iter().any(|span| span.width > 0.0))
    );

    if verify_palette {
        use crate::core::app::AppContext;
        use crate::core::paint::Color;
        use crate::tests::_support::assert_painted_text_backgrounds;

        let offsets = |tree: &WidgetTree| {
            tree.node(detail)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextBlock>()
                .unwrap()
                .test_selection_offsets()
        };
        let selected = Some(("alpha\n".len(), DETAIL.len()));
        assert_eq!(offsets(&tree), selected);
        for text in ["βeta", "γ"] {
            assert_painted_text_backgrounds(
                &successor.scene_root,
                text,
                &[Color::from_hex(0x1D7DFC).with_alpha(0.35)],
            );
        }

        let mut app = AppContext::new(theme);
        assert!(!app.take_theme_changed());
        app.set_theme(Theme::dark());
        assert!(app.take_theme_changed());
        tree.mark_subtree_dirty(root);
        let dark = frames.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(420.0, 232.0)),
            &app.theme,
            &mut text_engine,
            &icons,
        );
        assert_eq!(tree.widget_id_for_node(detail), Some(owner));
        assert_eq!(offsets(&tree), selected);
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        for text in ["βeta", "γ"] {
            assert_painted_text_backgrounds(
                &dark.scene_root,
                text,
                &[Color::from_hex(0x4B9AFF).with_alpha(0.35)],
            );
        }
        router
            .clipboard
            .as_mut()
            .unwrap()
            .use_memory_transport_for_test("sentinel");
        route!(copy());
        assert_eq!(
            router.clipboard.as_mut().unwrap().get_text().as_deref(),
            Some("\nβeta\nγ"),
        );
        assert!(list_core.borrow().is_selected(row));
        assert_eq!(
            log.format_rows_for_copy(&[row]),
            "[00:00:00.000] [INFO] [source] row 3",
        );
        return;
    }

    route!(down(first, 1));
    route!(drag_to(header));
    assert_eq!(
        domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    assert_eq!(list_core.borrow().focused_row(), Some(row));
    assert!(list_core.borrow().is_selected(row));
    let outside = Point::new(rect.x + rect.width + 10.0, rect.y + rect.height + 6.0);
    route!(drag_to(outside));
    route!(up(outside));
    assert_eq!(
        domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some(DETAIL)
    );

    frame!(49.0);
    let wrapped_rect = displayed_detail(&tree);
    assert!(wrapped_rect.height > rect.height);
    assert_eq!(tree.widget_id_for_node(detail), Some(owner));
    let fragment = frames.local_fragment(owner).unwrap();
    let mut wrapped_runs = Vec::new();
    gather_text(&fragment, &mut wrapped_runs);
    assert!(wrapped_runs.len() > runs.len());
    assert!(wrapped_runs.iter().all(|run| !run.backgrounds.is_empty()));
    let wrapped_first = Point::new(wrapped_rect.x + 0.1, wrapped_rect.y + line_height * 0.5);
    let wrapped_end = Point::new(
        wrapped_rect.x + wrapped_rect.width + 10.0,
        wrapped_rect.y + wrapped_rect.height + 6.0,
    );
    route!(down(wrapped_first, 1));
    route!(drag_to(wrapped_end));
    route!(up(wrapped_end));
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some(DETAIL)
    );

    // Keep the same physical press through every line transition, then retrace it.
    let first_run = wrapped_runs[0];
    let first_wrap = first_run.text.len();
    let hard_break = "alpha\n".len();
    assert!(first_run.text.starts_with("al") && first_wrap < "alpha".len());
    let beta_run_index = wrapped_runs
        .iter()
        .position(|run| run.text.starts_with('β'))
        .unwrap();
    let beta_run = wrapped_runs[beta_run_index];
    let beta_line = ((beta_run.position.y - first_run.position.y) / line_height).round() as usize;
    assert!(beta_line >= 3, "two wrapped lines must precede the blank");
    let blank_line = beta_line - 1;
    let line_count = (wrapped_rect.height / line_height).round() as usize;
    let offsets = |tree: &WidgetTree| {
        tree.node(detail)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextBlock>()
            .unwrap()
            .test_selection_offsets()
            .unwrap()
    };
    let start = Point::new(
        wrapped_rect.x + text_engine.shape("a", 13.0).width * 0.75,
        wrapped_rect.y + line_height * 0.25,
    );
    assert_eq!(tree.hit_test(start.x, start.y).unwrap().widget_id, owner);

    let mut fusion = InputFusion::new();
    let mut saw_pointer_move = false;
    let mut saw_drag_update = false;
    macro_rules! route_raw {
        ($raw:expr) => {{
            let mut pending = Vec::new();
            let input_domain = domain.focus_manager.interaction_domain_id();
            fusion.process_raw_with_gesture_sessions(
                $raw,
                input_domain,
                &mut domain.gesture_registry,
                &tree,
                None,
                &mut pending,
            );
            fusion.flush_with_gesture_sessions(&mut pending);
            let mut intake = pending.into_iter();
            let mut deferred = Vec::new();
            while let Some(input) = next_qualified_widget_input(&tree, &mut intake, &mut deferred) {
                saw_pointer_move |= matches!(input.event(), FusedInputEvent::PointerMove { .. });
                saw_drag_update |= matches!(input.event(), FusedInputEvent::DragUpdate { .. });
                router.route_session_input_event(
                    &mut tree,
                    input,
                    Some(root),
                    &mut text_engine,
                    None,
                    &mut domain,
                    &mut bindings,
                );
            }
            assert!(deferred.is_empty());
        }};
    }
    let button = |position, state| RawInputEvent::PointerButton {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        state,
        position,
    };
    let motion = |from: Point, position: Point| RawInputEvent::PointerMoved {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        device: PointerDevice::Mouse,
        position,
        delta: (position.x - from.x, position.y - from.y),
    };

    route_raw!(button(start, RawButtonState::Pressed));
    let anchor = 1;
    assert_eq!(offsets(&tree), (anchor, anchor));
    let first_move = Point::new(start.x, start.y + 1.0);
    route_raw!(motion(start, first_move));
    assert_eq!(offsets(&tree), (anchor, anchor));
    let mut downward = vec![(start, anchor), (first_move, anchor)];
    let shaped_first = text_engine
        .shape_bound_text(
            &first_run.text,
            first_run.font_size,
            first_run.binding.as_ref().unwrap(),
        )
        .unwrap();
    let anchor_x = x_for_offset(&shaped_first, anchor);
    assert!(anchor_x > 0.0);
    for quarter in 2..line_count * 4 {
        let (previous, previous_head) = *downward.last().unwrap();
        let position = Point::new(
            start.x,
            wrapped_rect.y + line_height * (quarter as f32 / 4.0),
        );
        assert!(position.y > previous.y);
        route_raw!(motion(previous, position));
        assert_eq!(
            domain
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        let (actual_anchor, head) = offsets(&tree);
        assert_eq!(actual_anchor, anchor);
        assert!(DETAIL.is_char_boundary(head));
        assert!(
            head >= previous_head,
            "downward sample {quarter} moved the selection head back from {previous_head} to {head}"
        );
        if quarter == 6 {
            assert!((first_wrap..hard_break).contains(&head), "first soft wrap");
        }
        if quarter == blank_line * 4 + 2 {
            assert_eq!(
                head, hard_break,
                "blank line's insertion after the first LF"
            );
            frame!(49.0);
            let fragment = frames.local_fragment(owner).unwrap();
            let mut at_blank = Vec::new();
            gather_text(&fragment, &mut at_blank);
            assert!((at_blank[0].backgrounds[0].x_offset - anchor_x).abs() < 0.001);
            assert!(
                at_blank[..beta_run_index]
                    .iter()
                    .all(|run| !run.backgrounds.is_empty())
            );
            assert!(
                at_blank[beta_run_index..]
                    .iter()
                    .all(|run| run.backgrounds.is_empty())
            );
        }
        if quarter == beta_line * 4 + 2 {
            assert!(head > hard_break, "first nonempty line after the blank");
        }
        downward.push((position, head));
    }

    let mut position = downward.last().unwrap().0;
    let end = Point::new(wrapped_rect.right() - 0.5, position.y + line_height * 0.125);
    route_raw!(motion(position, end));
    position = end;
    assert_eq!(offsets(&tree), (anchor, DETAIL.len()));
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some(&DETAIL[anchor..])
    );
    frame!(49.0);
    let fragment = frames.local_fragment(owner).unwrap();
    let mut selected = Vec::new();
    gather_text(&fragment, &mut selected);
    assert!((selected[0].backgrounds[0].x_offset - anchor_x).abs() < 0.001);
    assert!(selected.iter().all(|run| !run.backgrounds.is_empty()));

    for (previous, expected) in downward.into_iter().rev() {
        route_raw!(motion(position, previous));
        assert_eq!(offsets(&tree), (anchor, expected));
        position = previous;
    }
    let above_anchor = Point::new(wrapped_rect.x + 0.1, wrapped_rect.y + line_height * 0.125);
    route_raw!(motion(position, above_anchor));
    assert_eq!(offsets(&tree), (anchor, 0));
    route_raw!(button(above_anchor, RawButtonState::Released));
    assert!(saw_pointer_move && saw_drag_update);
    assert_eq!(
        domain
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some(&DETAIL[..anchor])
    );
    frame!(49.0);
    let fragment = frames.local_fragment(owner).unwrap();
    let mut reversed = Vec::new();
    gather_text(&fragment, &mut reversed);
    let highlight = &reversed[0].backgrounds[0];
    assert_eq!(highlight.x_offset, 0.0);
    assert!((highlight.width - anchor_x).abs() < 0.001);
    assert!(reversed[1..].iter().all(|run| run.backgrounds.is_empty()));
    assert_eq!(list_core.borrow().focused_row(), Some(row));
    assert!(list_core.borrow().is_selected(row));

    frame!(420.0);
    route!(down(header, 1));
    route!(up(header));
    assert_eq!(domain.focus_manager.focused(), Some(row_focus));
    assert!(list_core.borrow().is_selected(row));
    router
        .clipboard
        .as_mut()
        .unwrap()
        .use_memory_transport_for_test("row clipboard");
    route!(copy());
    assert_eq!(
        router.clipboard.as_mut().unwrap().get_text().as_deref(),
        Some("row clipboard")
    );
    assert_eq!(
        log.format_rows_for_copy(&[row]),
        "[00:00:00.000] [INFO] [source] row 3"
    );
}
