// SPEC-HAL-130/131 Stage 3: RadioGroup Input Mapping Tests
// Tests for pointer and keyboard event handling.

use halcyon::core::app::{AppContext, ModalStack, ScreenContext, ScreenStack};
use halcyon::core::event::{
    CancellationReason, Event, EventPhase, InteractionDomainState, KeyCode, KeyEvent,
    KeyboardAuthorityMode, Modifiers, PointerButton, PointerEvent, PointerEventKind, PointerId,
    PointerType,
};
use halcyon::core::event_router::EventRouter;
use halcyon::core::layout::Constraints;
use halcyon::core::math::{RectF, Size};
use halcyon::core::rect_authority::PaintAuthorityGuard;
use halcyon::core::scene::SceneBuilder;
use halcyon::core::text_engine::TextEngine;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget};
use halcyon::core::widget_tree::{OverlayHitRegion, WidgetDirty, WidgetTree};
use halcyon::model::form_binder::FormBinder;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::radio_group::{RadioGroupEvent, RadioId};
use halcyon::widgets::radio_group_widget::{RadioGroup, RadioGroupId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================================
// Test Helpers
// ============================================================================

fn make_pointer_down(local_y: f32) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (0.0, local_y),
        local_pos: (0.0, local_y),
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn make_pointer_up(local_y: f32) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (0.0, local_y),
        local_pos: (0.0, local_y),
        kind: PointerEventKind::Up,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn make_key_down(key_code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down: true,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

/// Run layout for the unlabeled-group fixtures.
/// Returns the indicator height and the actual allocated rect.
fn run_layout(group: &mut RadioGroup) -> (f32, RectF) {
    let mut tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();

    let constraints = Constraints {
        min_width: 0.0,
        max_width: 100.0,
        min_height: 0.0,
        max_height: 200.0,
    };
    let mut layout_cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let size = group.layout(&mut layout_cx, constraints, &[]).size;

    let member_height = theme.geometry.checkbox.size;
    let rect = RectF::new(0.0, 0.0, size.width, size.height);
    (member_height, rect)
}

#[test]
fn radio_group_input_hover_preserves_semantics_and_emission() {
    let mut group = RadioGroup::with_labeled_options(
        RadioGroupId(1),
        &[(RadioId(1), "Short"), (RadioId(2), "Longer option")],
    );
    let theme = Theme::dark();
    let mut text = FontManagerTextEngine::new();
    let font_size = theme.typography.font_size;
    let (label_width, text_height) = text.measure("Short", font_size);
    let row_height = theme
        .geometry
        .checkbox
        .size
        .max(text_height)
        .max(text.font_metrics(font_size).line_height);
    let content_width =
        theme.geometry.checkbox.size + theme.geometry.checkbox.label_gap + label_width;
    let mut tree = WidgetTree::new();
    let size = group
        .layout(
            &mut LayoutContext::new(&mut tree, &mut text, &theme),
            Constraints::tight(Size::new(400.0, 200.0)),
            &[],
        )
        .size;
    let rect = RectF::new(31.0, 47.0, size.width, size.height);
    group.set_selected(Some(RadioId(2)));
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));
    group.on_semantic_event(RadioGroupEvent::Press(RadioId(2)));
    let semantic = group.state().clone();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));
    let mut binder = FormBinder::new();
    group.set_binding("choice", binder.writer());
    let mut app = AppContext::new(theme);
    let mut screens = ScreenStack::new();
    let mut modals = ModalStack::new();
    let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
    for (x, y, changed) in [
        (1.0, row_height / 2.0, true),
        (content_width - 1.0, row_height / 2.0, false),
        (1.0, row_height + 0.5, false),
        (1.0, row_height + 2.0, true),
        (content_width + 1.0, row_height / 2.0, true),
        (-1.0, -1.0, false),
        (1.0, row_height / 2.0, true),
    ] {
        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (rect.x + x, rect.y + y),
            local_pos: (rect.x + x, rect.y + y),
            kind: PointerEventKind::Move,
            button: None,
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        });
        assert!(
            !group.handle_event(&event, rect),
            "hover observation must remain unconsumed"
        );
        assert_eq!(group.needs_update(), changed);
        let _update = halcyon::core::rect_authority::UpdateAuthorityGuard::enter();
        let mut cx = halcyon::core::widget::UpdateContext::new(None);
        group.update(0.0, &mut cx);
        assert!(!group.needs_update());
        assert_eq!(group.state(), &semantic);
        assert!(changes.borrow().is_empty());
        binder.bind(&screen, |binding| {
            assert_eq!(binding.selection("choice").take(), None)
        });
    }
    group.set_disabled(true);
    assert!(group.needs_update());
    {
        let _update = halcyon::core::rect_authority::UpdateAuthorityGuard::enter();
        let mut cx = halcyon::core::widget::UpdateContext::new(None);
        group.update(0.0, &mut cx);
    }
    assert!(!group.needs_update());
    let event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (rect.x + 1.0, rect.y + 1.0),
        local_pos: (rect.x + 1.0, rect.y + 1.0),
        kind: PointerEventKind::Move,
        button: None,
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    assert!(!group.handle_event(&event, rect));
    assert!(!group.needs_update());
    let mut disabled_semantic = semantic;
    disabled_semantic.disabled = true;
    assert_eq!(group.state(), &disabled_semantic);
    assert!(changes.borrow().is_empty());
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), None)
    });
}

#[test]
fn radio_group_input_short_label_ignores_stretched_row_whitespace() {
    let mut group = RadioGroup::with_labeled_options(
        RadioGroupId(1),
        &[
            (RadioId(1), "Short"),
            (RadioId(2), "A substantially longer label"),
        ],
    );
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));
    let mut binder = FormBinder::new();
    group.set_binding("choice", binder.writer());
    let theme = Theme::dark();
    let mut text = FontManagerTextEngine::new();
    let indicator = theme.geometry.checkbox.size;
    let gap = theme.geometry.checkbox.label_gap;
    let font_size = theme.typography.font_size;
    let (short_text_width, text_height) = text.measure("Short", font_size);
    let short_width = indicator + gap + short_text_width;
    let long_width = indicator + gap + text.measure("A substantially longer label", font_size).0;
    let row_height = indicator
        .max(text_height)
        .max(text.font_metrics(font_size).line_height);
    assert!(long_width > short_width && long_width < 400.0);
    let mut tree = WidgetTree::new();
    let size = group
        .layout(
            &mut LayoutContext::new(&mut tree, &mut text, &theme),
            Constraints::tight(Size::new(400.0, 300.0)),
            &[],
        )
        .size;
    let rect = RectF::new(31.0, 47.0, size.width, size.height);
    let mut app = AppContext::new(theme);
    let mut screens = ScreenStack::new();
    let mut modals = ModalStack::new();
    let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
    let before = group.state().clone();
    for (x, y) in [
        (short_width, row_height / 2.0),
        ((short_width + long_width) / 2.0, row_height / 2.0),
        (long_width + 1.0, row_height / 2.0),
        (size.width, row_height / 2.0),
        (-1.0, row_height / 2.0),
        (1.0, -1.0),
        (1.0, row_height + 1.0 + row_height),
    ] {
        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (rect.x + x, rect.y + y),
            local_pos: (rect.x + x, rect.y + y),
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        });
        assert!(!group.handle_event(&event, rect));
        assert_eq!(group.state(), &before);
    }
    assert!(changes.borrow().is_empty());
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), None)
    });

    let label_x = indicator + gap + short_text_width / 2.0;
    let down = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (rect.x + label_x, rect.y + row_height / 2.0),
        local_pos: (rect.x + label_x, rect.y + row_height / 2.0),
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    assert!(!group.handle_event(&down, RectF::new(rect.x, rect.y, indicator, rect.height)));
    assert!(!group.handle_event(
        &down,
        RectF::new(rect.x, rect.y, rect.width, row_height / 4.0)
    ));
    assert_eq!(group.state(), &before);
    assert!(group.handle_event(&down, rect));
    assert_eq!(group.selected(), Some(RadioId(1)));
    assert_eq!(group.state().focused, Some(RadioId(1)));
    assert_eq!(group.state().active, Some(RadioId(1)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(1))]);
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), Some(Some(1)))
    });
    assert!(group.handle_event(&down, rect));
    assert!(group.handle_event(&make_pointer_up(-10.0), rect));
    assert_eq!(group.state().active, None);
    assert_eq!(group.selected(), Some(RadioId(1)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(1))]);
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), None)
    });
}

#[test]
fn radio_group_input_widget_metric_rows_gaps_and_clipped_bounds() {
    let mut group = RadioGroup::with_labeled_options(
        RadioGroupId(2),
        &[(RadioId(1), ""), (RadioId(2), "Last row")],
    );
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));
    let mut binder = FormBinder::new();
    group.set_binding("choice", binder.writer());
    let mut theme = Theme::dark();
    theme.typography.font_size = 32.0;
    let mut text = FontManagerTextEngine::new();
    let indicator = theme.geometry.checkbox.size;
    let height = indicator
        .max(text.measure("", theme.typography.font_size).1)
        .max(text.font_metrics(theme.typography.font_size).line_height);
    assert!(height > indicator);
    let mut tree = WidgetTree::new();
    let size = group
        .layout(
            &mut LayoutContext::new(&mut tree, &mut text, &theme),
            Constraints::unbounded(),
            &[],
        )
        .size;
    assert_eq!(size.height, height + 1.0 + height);
    let mut app = AppContext::new(theme.clone());
    let mut screens = ScreenStack::new();
    let mut modals = ModalStack::new();
    let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
    let mut cx = EventContext::new(
        TreeWidgetId(1),
        &mut tree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    for (y, id) in [(height + 0.5, RadioId(1)), (height + 1.0, RadioId(2))] {
        assert!(Widget::handle_event(
            &mut group,
            &mut cx,
            &make_pointer_down(y),
            EventPhase::Target
        ));
        assert_eq!(group.selected(), Some(id));
        assert_eq!(group.state().active, Some(id));
        binder.bind(&screen, |binding| {
            assert_eq!(binding.selection("choice").take(), Some(Some(id.0)))
        });
        assert!(Widget::handle_event(
            &mut group,
            &mut cx,
            &make_pointer_up(-10.0),
            EventPhase::Target
        ));
        assert_eq!(group.state().active, None);
        binder.bind(&screen, |binding| {
            assert_eq!(binding.selection("choice").take(), None)
        });
    }
    let before = group.state().clone();
    for (x, y) in [
        (0.0, size.height),
        (0.0, -1.0),
        (-1.0, height / 2.0),
        (indicator + theme.geometry.checkbox.label_gap, height / 2.0),
        (size.width, height + 1.0),
    ] {
        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (x, y),
            local_pos: (x, y),
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        });
        assert!(!Widget::handle_event(
            &mut group,
            &mut cx,
            &event,
            EventPhase::Target
        ));
        assert_eq!(group.state(), &before);
    }
    drop(cx);
    theme.typography.font_size = 48.0;
    let taller = indicator
        .max(text.measure("", theme.typography.font_size).1)
        .max(text.font_metrics(theme.typography.font_size).line_height);
    assert!(taller > height + 1.0);
    let clipped = group
        .layout(
            &mut LayoutContext::new(&mut tree, &mut text, &theme),
            Constraints::tight(Size::new(indicator / 2.0, taller + 0.5)),
            &[],
        )
        .size;
    let mut cx = EventContext::new(
        TreeWidgetId(1),
        &mut tree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    // The old second-row origin is now inside the newly measured first row.
    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_pointer_down(height + 1.0),
        EventPhase::Target
    ));
    assert_eq!(group.selected(), Some(RadioId(1)));
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), Some(Some(1)))
    });
    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_pointer_up(-10.0),
        EventPhase::Target
    ));
    let before = group.state().clone();
    for (x, y) in [
        (0.0, clipped.height),
        (0.0, taller + 1.0),
        (clipped.width, taller / 2.0),
    ] {
        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (x, y),
            local_pos: (x, y),
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        });
        assert!(!Widget::handle_event(
            &mut group,
            &mut cx,
            &event,
            EventPhase::Target
        ));
        assert_eq!(group.state(), &before);
    }
    assert_eq!(
        *changes.borrow(),
        vec![Some(RadioId(1)), Some(RadioId(2)), Some(RadioId(1))]
    );
    binder.bind(&screen, |binding| {
        assert_eq!(binding.selection("choice").take(), None)
    });
}

// ============================================================================
// Test 1: Pointer click selects correct member
// ============================================================================

#[test]
fn radio_group_input_inherent_release_over_other_member_clears_active() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (member_height, rect) = run_layout(&mut group);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&changes);
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));

    // Click on second member (y in range [member_height, 2*member_height))
    let click_y = member_height + member_height / 2.0;

    group.handle_event(&make_pointer_down(click_y), rect);
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert_eq!(group.state().focused, Some(RadioId(2)));
    assert_eq!(group.state().active, Some(RadioId(2)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);

    group.handle_event(&make_pointer_up(member_height / 2.0), rect);
    assert_eq!(group.state().active, None);
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);

    assert_eq!(
        group.selected(),
        Some(RadioId(2)),
        "Click should select second member"
    );
}

#[test]
fn radio_group_input_widget_release_outside_and_capture_cancel_preserve_selection() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    group.set_selected(Some(RadioId(1)));
    let (member_height, _) = run_layout(&mut group);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&changes);
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));
    let mut binder = FormBinder::new();
    group.set_binding("choice", binder.writer());

    let mut app = AppContext::new(Theme::dark());
    let mut screens = ScreenStack::new();
    let mut modals = ModalStack::new();
    let screen_cx = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
    let mut tree = WidgetTree::new();
    let mut cx = EventContext::new(
        TreeWidgetId(1),
        &mut tree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let click_y = member_height + member_height / 2.0;

    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_pointer_down(click_y),
        EventPhase::Target,
    ));
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert_eq!(group.state().focused, Some(RadioId(2)));
    assert_eq!(group.state().active, Some(RadioId(2)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);
    binder.bind(&screen_cx, |binding| {
        assert_eq!(binding.selection("choice").take(), Some(Some(2)));
    });

    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_pointer_up(-10.0),
        EventPhase::Target,
    ));
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert_eq!(group.state().active, None);
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);
    binder.bind(&screen_cx, |binding| {
        assert_eq!(binding.selection("choice").take(), None);
    });

    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_pointer_down(click_y),
        EventPhase::Target,
    ));
    assert_eq!(group.state().active, Some(RadioId(2)));
    Widget::on_pointer_capture_cancel(
        &mut group,
        &mut cx,
        PointerId::MOUSE,
        CancellationReason::SystemCancel,
    );
    assert_eq!(group.state().active, None);
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert_eq!(group.state().focused, Some(RadioId(2)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);
    binder.bind(&screen_cx, |binding| {
        assert_eq!(binding.selection("choice").take(), None);
    });

    assert!(Widget::handle_event(
        &mut group,
        &mut cx,
        &make_key_down(KeyCode::Up),
        EventPhase::Target,
    ));
    assert_eq!(group.state().focused, Some(RadioId(1)));
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);
    binder.bind(&screen_cx, |binding| {
        assert_eq!(binding.selection("choice").take(), None);
    });

    // Framework i/Enter/Space actions all reach this activation hook.
    Widget::on_keyboard_activate(&mut group, &mut cx);
    assert_eq!(group.selected(), Some(RadioId(1)));
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2)), Some(RadioId(1))]);
    binder.bind(&screen_cx, |binding| {
        assert_eq!(binding.selection("choice").take(), Some(Some(1)));
    });
}

#[test]
fn pointer_click_first_member() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (member_height, rect) = run_layout(&mut group);

    let click_y = member_height / 2.0;

    group.handle_event(&make_pointer_down(click_y), rect);
    group.handle_event(&make_pointer_up(click_y), rect);

    assert_eq!(
        group.selected(),
        Some(RadioId(1)),
        "Click should select first member"
    );
}

#[test]
fn pointer_click_third_member() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (member_height, rect) = run_layout(&mut group);

    let click_y = member_height * 2.0 + member_height / 2.0;

    group.handle_event(&make_pointer_down(click_y), rect);
    group.handle_event(&make_pointer_up(click_y), rect);

    assert_eq!(
        group.selected(),
        Some(RadioId(3)),
        "Click should select third member"
    );
}

// ============================================================================
// Test 2: Pointer cancel clears active, not selection
// ============================================================================

#[test]
fn radio_group_input_pointer_cancel_preserves_down_selection() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    let (member_height, rect) = run_layout(&mut group);

    // First, select member 1 via click
    let click_y = member_height / 2.0;
    group.handle_event(&make_pointer_down(click_y), rect);
    group.handle_event(&make_pointer_up(click_y), rect);
    assert_eq!(group.selected(), Some(RadioId(1)));

    // Mouse Down selects and arms member 2.
    let click_y2 = member_height + member_height / 2.0;
    group.handle_event(&make_pointer_down(click_y2), rect);

    // Active should be set
    assert_eq!(
        group.state().active,
        Some(RadioId(2)),
        "Active should be set on pointer down"
    );

    // Selection is committed on Down.
    assert_eq!(
        group.selected(),
        Some(RadioId(2)),
        "Mouse Down should select immediately"
    );

    // Cancel via reducer
    group.on_semantic_event(RadioGroupEvent::Cancel(RadioId(2)));

    // Active cleared
    assert_eq!(group.state().active, None, "Cancel should clear active");

    // Selection preserved
    assert_eq!(
        group.selected(),
        Some(RadioId(2)),
        "Cancel should not change selection"
    );
}

// ============================================================================
// Test 3: Arrow navigation changes focus only
// ============================================================================

#[test]
fn radio_group_input_arrow_navigation_changes_focus_only() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Initial: focus on first member, no selection
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));
    assert_eq!(group.state().focused, Some(RadioId(1)));
    assert_eq!(group.selected(), None);

    // Arrow down should move focus
    group.handle_event(&make_key_down(KeyCode::Down), rect);

    assert_eq!(
        group.state().focused,
        Some(RadioId(2)),
        "Down arrow should move focus"
    );

    // Selection unchanged
    assert_eq!(group.selected(), None, "Arrow should not change selection");

    // Arrow down again
    group.handle_event(&make_key_down(KeyCode::Down), rect);
    assert_eq!(
        group.state().focused,
        Some(RadioId(3)),
        "Down arrow should move focus"
    );
    assert_eq!(group.selected(), None, "Arrow should not change selection");
}

#[test]
fn arrow_up_moves_focus() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Set initial focus to member 2
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));

    // Arrow up
    group.handle_event(&make_key_down(KeyCode::Up), rect);

    assert_eq!(
        group.state().focused,
        Some(RadioId(1)),
        "Up arrow should move focus up"
    );
}

// ============================================================================
// Test 4: Enter/Space activates focused member
// ============================================================================

#[test]
fn radio_group_input_keyboard_activation_invalidates_paint_on_selection_change() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    group.set_selected(Some(RadioId(1)));
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&changes);
    group.set_on_selection_changed(move |_, selected| observed.borrow_mut().push(selected));

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(group), None, 0);
    let widget_id = tree.node(root).unwrap().id();
    {
        let node = tree.node_mut(root).unwrap();
        node.dirty_layout = false;
        node.dirty_paint = false;
        node.dirty_composition = false;
        node.dirty_flags = WidgetDirty::Unchanged;
    }
    assert!(!tree.node(root).unwrap().dirty_paint);

    let mut domain = InteractionDomainState::new();
    domain.focus_manager.set_focus(Some(widget_id));
    let mut router = EventRouter::new();
    assert!(router.activate_focused_widget(&mut tree, &mut domain, None));

    let node = tree.node(root).unwrap();
    let group = node.widget().as_any().downcast_ref::<RadioGroup>().unwrap();
    assert_eq!(group.selected(), Some(RadioId(2)));
    assert!(node.dirty_paint);
    assert!(node.dirty_composition);
    assert!(!node.dirty_layout);
    assert_eq!(node.dirty_flags, WidgetDirty::PaintOnly);
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);

    {
        let node = tree.node_mut(root).unwrap();
        node.dirty_paint = false;
        node.dirty_composition = false;
        node.dirty_flags = WidgetDirty::Unchanged;
    }
    assert!(router.activate_focused_widget(&mut tree, &mut domain, None));
    let node = tree.node(root).unwrap();
    assert!(!node.dirty_paint);
    assert!(!node.dirty_composition);
    assert!(!node.dirty_layout);
    assert_eq!(node.dirty_flags, WidgetDirty::Unchanged);
    assert_eq!(*changes.borrow(), vec![Some(RadioId(2))]);
}

#[test]
fn radio_group_input_enter_activates_focused_member() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Set focus to member 2
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));

    // Press Enter
    group.handle_event(&make_key_down(KeyCode::Enter), rect);

    assert_eq!(
        group.selected(),
        Some(RadioId(2)),
        "Enter should activate focused member"
    );
}

#[test]
fn radio_group_input_space_activates_focused_member() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Set focus to member 3
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(3)));

    // Press Space
    group.handle_event(&make_key_down(KeyCode::Space), rect);

    assert_eq!(
        group.selected(),
        Some(RadioId(3)),
        "Space should activate focused member"
    );
}

#[test]
fn enter_without_focus_does_nothing() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // No focus set
    assert_eq!(group.state().focused, None);

    // Press Enter
    let handled = group.handle_event(&make_key_down(KeyCode::Enter), rect);

    assert!(!handled, "Enter without focus should not be handled");
    assert_eq!(group.selected(), None, "Selection should remain None");
}

// ============================================================================
// Test 5: Disabled group ignores all input
// ============================================================================

#[test]
fn radio_group_input_disabled_group_ignores_pointer() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    let (member_height, rect) = run_layout(&mut group);

    group.set_disabled(true);

    let click_y = member_height / 2.0;
    let handled = group.handle_event(&make_pointer_down(click_y), rect);

    assert!(!handled, "Disabled group should not handle pointer events");
    assert_eq!(
        group.state().active,
        None,
        "Active should not be set when disabled"
    );
    assert_eq!(
        group.selected(),
        None,
        "Selection should not change when disabled"
    );
}

#[test]
fn radio_group_input_disabled_group_ignores_keyboard() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    let (_, rect) = run_layout(&mut group);

    group.set_disabled(true);

    let handled = group.handle_event(&make_key_down(KeyCode::Down), rect);

    assert!(!handled, "Disabled group should not handle keyboard events");
    assert_eq!(
        group.state().focused,
        None,
        "Focus should not change when disabled"
    );
}

// ============================================================================
// Test 6: Focus stops at group boundaries
// ============================================================================

#[test]
fn focus_stops_at_end_down() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Set focus to last member
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(3)));

    // Arrow down at boundary is unhandled so framework navigation can take over
    let handled = group.handle_event(&make_key_down(KeyCode::Down), rect);

    assert!(!handled, "Down arrow at boundary should be unhandled");
    assert_eq!(
        group.state().focused,
        Some(RadioId(3)),
        "Focus should stay on last member"
    );
}

#[test]
fn focus_stops_at_beginning_up() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (_, rect) = run_layout(&mut group);

    // Set focus to first member
    group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(1)));

    // Arrow up at boundary is unhandled so framework navigation can take over
    let handled = group.handle_event(&make_key_down(KeyCode::Up), rect);

    assert!(!handled, "Up arrow at boundary should be unhandled");
    assert_eq!(
        group.state().focused,
        Some(RadioId(1)),
        "Focus should stay on first member"
    );
}

// ============================================================================
// Test 7: Paint unaffected by input
// ============================================================================

#[test]
fn paint_unaffected_by_input() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
    let (member_height, rect) = run_layout(&mut group);

    // Perform some input operations
    let click_y = member_height + member_height / 2.0;
    group.handle_event(&make_pointer_down(click_y), rect);
    group.handle_event(&make_pointer_up(click_y), rect);

    // Record state after input
    let state_after_input = group.state().clone();

    // Paint
    let tree = WidgetTree::new();
    let theme = Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut scene = SceneBuilder::new();
    let timeline = halcyon::core::animation::Timeline::new();
    let interpolated_rects: HashMap<TreeWidgetId, RectF> = HashMap::new();
    let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let icon_registry = halcyon::core::icons::builtin_registry();
    let mut paint_cx = PaintContext::new(
        &tree,
        &mut scene,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_hit_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        KeyboardAuthorityMode::Navigation,
    );
    let _paint_authority = PaintAuthorityGuard::enter();
    group.paint(&mut paint_cx, rect, &[]);

    // State should be unchanged after paint
    assert_eq!(
        group.state(),
        &state_after_input,
        "Paint should not mutate state"
    );
}

// ============================================================================
// Additional: Hit test boundary conditions
// ============================================================================

#[test]
fn hit_test_outside_bounds_ignored() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    let (_, rect) = run_layout(&mut group);

    // Click outside bounds (negative y)
    let handled_neg = group.handle_event(&make_pointer_down(-10.0), rect);
    assert!(!handled_neg, "Click outside bounds should not be handled");

    // Click outside bounds (beyond rect)
    let handled_beyond = group.handle_event(&make_pointer_down(rect.height + 10.0), rect);
    assert!(!handled_beyond, "Click beyond bounds should not be handled");
}

#[test]
fn escape_clears_active() {
    let mut group = RadioGroup::new(&[RadioId(1), RadioId(2)]);
    let (member_height, rect) = run_layout(&mut group);

    // Set focus and active via pointer down
    let click_y = member_height / 2.0;
    group.handle_event(&make_pointer_down(click_y), rect);

    assert_eq!(group.state().focused, Some(RadioId(1)));
    assert_eq!(group.state().active, Some(RadioId(1)));

    // Press Escape
    group.handle_event(&make_key_down(KeyCode::Escape), rect);

    // Active cleared via Cancel
    assert_eq!(group.state().active, None, "Escape should clear active");
}
