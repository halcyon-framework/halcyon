use crate::core::event::{
    Event, EventPhase, KeyboardAuthorityMode, Modifiers, PointerButton, PointerEvent,
    PointerEventKind, PointerId, PointerType,
};
use crate::core::layout::Constraints;
use crate::core::widget::{EventContext, LayoutContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::utils::id::TreeWidgetId;
use crate::widgets::checkbox::Checkbox;

fn pointer_event(kind: PointerEventKind) -> Event {
    Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (10.0, 10.0),
        local_pos: (10.0, 10.0),
        kind,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    })
}

fn layout_checkbox(checkbox: &mut Checkbox, tree: &mut WidgetTree) {
    let theme = crate::core::theme::Theme::dark();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(tree, &mut text_engine, &theme);
    checkbox.layout(
        &mut cx,
        Constraints {
            min_width: 0.0,
            max_width: 100.0,
            min_height: 0.0,
            max_height: 32.0,
        },
        &[],
    );
}

fn event_context(tree: &mut WidgetTree) -> EventContext<'_> {
    EventContext::new(
        TreeWidgetId::from_raw(1),
        tree as *mut WidgetTree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    )
}

#[test]
fn checkbox_pointer_down_previews_unchecked_target_without_toggling_state() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    layout_checkbox(&mut checkbox, &mut tree);

    let mut cx = event_context(&mut tree);
    assert!(Widget::handle_event(
        &mut checkbox,
        &mut cx,
        &pointer_event(PointerEventKind::Down),
        EventPhase::Target,
    ));

    assert!(!checkbox.is_checked());
    assert_eq!(checkbox.check_visual_progress_for_test(), 1.0);
}

#[test]
fn checkbox_pointer_down_previews_checked_target_without_toggling_state() {
    let mut checkbox = Checkbox::checked(true);
    let mut tree = WidgetTree::new();
    layout_checkbox(&mut checkbox, &mut tree);

    let mut cx = event_context(&mut tree);
    assert!(Widget::handle_event(
        &mut checkbox,
        &mut cx,
        &pointer_event(PointerEventKind::Down),
        EventPhase::Target,
    ));

    assert!(checkbox.is_checked());
    assert_eq!(checkbox.check_visual_progress_for_test(), 0.0);
}

#[test]
fn checkbox_pointer_up_updates_visual_after_real_toggle() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    layout_checkbox(&mut checkbox, &mut tree);

    {
        let mut cx = event_context(&mut tree);
        assert!(Widget::handle_event(
            &mut checkbox,
            &mut cx,
            &pointer_event(PointerEventKind::Down),
            EventPhase::Target,
        ));
    }

    let mut cx = event_context(&mut tree);
    assert!(Widget::handle_event(
        &mut checkbox,
        &mut cx,
        &pointer_event(PointerEventKind::Up),
        EventPhase::Target,
    ));

    assert!(checkbox.is_checked());
    assert_eq!(checkbox.check_visual_progress_for_test(), 1.0);
}

#[test]
fn checkbox_hover_visual_stays_pinned_during_active_press() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    layout_checkbox(&mut checkbox, &mut tree);

    {
        let mut cx = event_context(&mut tree);
        Widget::on_hover_enter(&mut checkbox, &mut cx);
    }
    assert_eq!(checkbox.hover_visual_progress_for_test(), 1.0);

    {
        let mut cx = event_context(&mut tree);
        assert!(Widget::handle_event(
            &mut checkbox,
            &mut cx,
            &pointer_event(PointerEventKind::Down),
            EventPhase::Target,
        ));
    }

    let mut cx = event_context(&mut tree);
    Widget::on_hover_leave(&mut checkbox, &mut cx);

    assert!(!checkbox.is_checked());
    assert_eq!(checkbox.hover_visual_progress_for_test(), 1.0);
    assert_eq!(checkbox.check_visual_progress_for_test(), 1.0);
}

#[test]
fn checkbox_pointer_up_outside_restores_visual_preview_without_toggling_state() {
    let mut checkbox = Checkbox::new();
    let mut tree = WidgetTree::new();
    layout_checkbox(&mut checkbox, &mut tree);

    {
        let mut cx = event_context(&mut tree);
        assert!(Widget::handle_event(
            &mut checkbox,
            &mut cx,
            &pointer_event(PointerEventKind::Down),
            EventPhase::Target,
        ));
    }
    assert_eq!(checkbox.check_visual_progress_for_test(), 1.0);

    let mut up_outside = pointer_event(PointerEventKind::Up);
    if let Event::Pointer(pointer) = &mut up_outside {
        pointer.local_pos = (40.0, 10.0);
    }

    let mut cx = event_context(&mut tree);
    assert!(Widget::handle_event(
        &mut checkbox,
        &mut cx,
        &up_outside,
        EventPhase::Target,
    ));

    assert!(!checkbox.is_checked());
    assert_eq!(checkbox.check_visual_progress_for_test(), 0.0);
}
