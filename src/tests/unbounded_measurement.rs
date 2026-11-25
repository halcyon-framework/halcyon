use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use crate::core::layout::Constraints;
use crate::core::widget::{EventContext, LayoutContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::utils::id::TreeWidgetId;
use crate::widgets::auxiliary_text::AuxiliaryText;
use crate::widgets::divider::Divider;
use crate::widgets::section_header::{SectionHeader, SectionHeaderEmphasis};
use crate::widgets::section_surfaces::{
    DisclosureSectionHeader, SectionBandHeader, SectionBandLevel, SectionBandMarker,
    SectionMetaHeader,
};
use crate::widgets::text_block::TextBlock;

fn unbounded_constraints() -> Constraints {
    Constraints {
        min_width: 0.0,
        max_width: f32::INFINITY,
        min_height: 0.0,
        max_height: f32::INFINITY,
    }
}

fn assert_finite_layout(widget: &mut dyn Widget) {
    let theme = crate::core::theme::Theme::dark();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let result = widget.layout(&mut cx, unbounded_constraints(), &[]);

    assert!(result.size.width.is_finite());
    assert!(result.size.height.is_finite());
    assert!(result.content_size.width.is_finite());
    assert!(result.content_size.height.is_finite());
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

fn key_up_event(key_code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        key_code,

        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
        is_down: false,
        is_repeat: false,
        modifiers: Modifiers::default(),
        text: None,
    })
}

fn layout_for_events(widget: &mut dyn Widget) {
    let theme = crate::core::theme::Theme::dark();
    let mut tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    let constraints = Constraints {
        min_width: 0.0,
        max_width: 240.0,
        min_height: 0.0,
        max_height: 24.0,
    };
    widget.layout(&mut cx, constraints, &[]);
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
fn text_widgets_report_finite_size_under_unbounded_measurement() {
    let mut text_block = TextBlock::new("Transient UI surfaces");
    let mut auxiliary =
        AuxiliaryText::new("Modal routing and verified multi-window runtime state.");
    let mut section = SectionHeader::new("Overlays and Windows");

    assert_finite_layout(&mut text_block);
    assert_finite_layout(&mut auxiliary);
    assert_finite_layout(&mut section);
}

#[test]
fn section_header_subtle_emphasis_preserves_case_without_undersizing_label() {
    let primary = SectionHeader::new("Client [client-local]");
    let subtle =
        SectionHeader::new("Client [client-local]").with_emphasis(SectionHeaderEmphasis::Subtle);

    assert_eq!(primary.emphasis(), SectionHeaderEmphasis::Primary);
    assert_eq!(primary.test_primary_text(), "CLIENT [CLIENT-LOCAL]");
    assert_eq!(subtle.emphasis(), SectionHeaderEmphasis::Subtle);
    assert_eq!(subtle.test_primary_text(), "Client [client-local]");
    assert_eq!(
        subtle.test_resolved_primary_font_size(14.0),
        primary.test_resolved_primary_font_size(14.0)
    );
}

#[test]
fn section_surface_family_keeps_distinct_header_authority() {
    let mut band = SectionBandHeader::new("Identity")
        .with_level(SectionBandLevel::Section)
        .with_marker(SectionBandMarker::Focus);
    let mut meta = SectionMetaHeader::new("Projection", "4 fields")
        .with_level(SectionBandLevel::SubSection)
        .with_marker(SectionBandMarker::Selection);
    let mut disclosure = DisclosureSectionHeader::new("Recovery", true);

    assert_eq!(band.level(), SectionBandLevel::Section);
    assert_eq!(band.marker(), SectionBandMarker::Focus);
    assert_eq!(band.test_label(), "Identity");
    assert_eq!(meta.level(), SectionBandLevel::SubSection);
    assert_eq!(meta.marker(), SectionBandMarker::Selection);
    assert_eq!(meta.meta(), "4 fields");
    assert!(disclosure.expanded());

    assert!(!band.hit_testable());
    assert!(!meta.hit_testable());
    assert!(disclosure.hit_testable());

    assert_finite_layout(&mut band);
    assert_finite_layout(&mut meta);
    assert_finite_layout(&mut disclosure);
}

#[test]
fn disclosure_section_header_requests_explicit_toggle_without_owning_state() {
    use std::cell::Cell;
    use std::rc::Rc;

    let toggle_count = Rc::new(Cell::new(0));
    let toggle_count_for_callback = Rc::clone(&toggle_count);
    let mut disclosure = DisclosureSectionHeader::new("Recovery", false).on_toggle(move || {
        toggle_count_for_callback.set(toggle_count_for_callback.get() + 1);
    });
    let mut tree = WidgetTree::new();
    let mut cx = event_context(&mut tree);

    assert!(!disclosure.expanded());
    layout_for_events(&mut disclosure);
    assert!(disclosure.handle_event(
        &mut cx,
        &pointer_event(PointerEventKind::Down, (16.0, 12.0)),
        EventPhase::Target
    ));
    assert_eq!(toggle_count.get(), 0);
    assert!(disclosure.handle_event(
        &mut cx,
        &pointer_event(PointerEventKind::Up, (16.0, 12.0)),
        EventPhase::Target
    ));
    assert_eq!(toggle_count.get(), 1);
    assert!(!disclosure.expanded());

    disclosure.set_expanded(true);
    assert!(disclosure.expanded());
}

#[test]
fn disclosure_section_header_cancels_toggle_when_released_outside() {
    use std::cell::Cell;
    use std::rc::Rc;

    let toggle_count = Rc::new(Cell::new(0));
    let toggle_count_for_callback = Rc::clone(&toggle_count);
    let mut disclosure = DisclosureSectionHeader::new("Recovery", false).on_toggle(move || {
        toggle_count_for_callback.set(toggle_count_for_callback.get() + 1);
    });
    let mut tree = WidgetTree::new();
    let mut cx = event_context(&mut tree);

    layout_for_events(&mut disclosure);
    assert!(disclosure.handle_event(
        &mut cx,
        &pointer_event(PointerEventKind::Down, (16.0, 12.0)),
        EventPhase::Target
    ));
    assert!(disclosure.handle_event(
        &mut cx,
        &pointer_event(PointerEventKind::Up, (280.0, 4.0)),
        EventPhase::Target
    ));
    assert_eq!(toggle_count.get(), 0);
    assert!(!disclosure.expanded());
}

#[test]
fn disclosure_section_header_keyboard_activation_requests_toggle() {
    use std::cell::Cell;
    use std::rc::Rc;

    let toggle_count = Rc::new(Cell::new(0));
    let toggle_count_for_callback = Rc::clone(&toggle_count);
    let mut disclosure = DisclosureSectionHeader::new("Recovery", false).on_toggle(move || {
        toggle_count_for_callback.set(toggle_count_for_callback.get() + 1);
    });
    let mut tree = WidgetTree::new();
    let mut cx = event_context(&mut tree);

    assert!(disclosure.focusable());
    assert!(disclosure.accepts_keyboard_activation());
    disclosure.on_keyboard_activate(&mut cx);
    assert_eq!(toggle_count.get(), 0);
    assert!(disclosure.handle_event(&mut cx, &key_up_event(KeyCode::Enter), EventPhase::Target));
    assert_eq!(toggle_count.get(), 1);
    assert!(!disclosure.expanded());
}

#[test]
fn dividers_report_finite_size_under_unbounded_measurement() {
    let mut horizontal = Divider::horizontal();
    let mut vertical = Divider::vertical();

    assert_finite_layout(&mut horizontal);
    assert_finite_layout(&mut vertical);
}
