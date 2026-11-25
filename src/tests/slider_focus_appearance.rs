// The scene entry requires private engine frame bookkeeping between frames.
use crate::core::app::{
    RetainedSceneTestHarness, build_scene_with_appearance, test_begin_frame_layout_tracking,
};
use crate::core::event::{
    Event, EventPhase, InteractionDomains, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType, WindowId,
};
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::layout_transition::ActiveLayoutTransitions;
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_FOCUS, TrackDisplayStyle, ZIndex};
use crate::core::scene::{Color, SceneNode};
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::transition::TransitionLayer;
use crate::core::widget::EventContext;
use crate::core::widget_tree::{OverflowMode, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::container::Container;
use crate::widgets::slider::{Slider, ValueFieldSide};

#[derive(Debug, PartialEq)]
struct FocusCue {
    rect: RectF,
    color: Color,
    width: f32,
    radius: f32,
    clips: Vec<RectF>,
    z_index: ZIndex,
}

#[derive(Default)]
struct Presentation {
    cues: Vec<FocusCue>,
    texts: Vec<RectF>,
    value_fields: Vec<RectF>,
    dial: Option<RectF>,
    track: Option<RectF>,
    indicator: Option<(RectF, f32)>,
    indicator_color: Option<Color>,
    dial_thickness: Option<f32>,
    indicator_z: Option<ZIndex>,
}

fn collect_presentation(
    nodes: &[SceneNode],
    presentation: &mut Presentation,
    text_engine: &mut FontManagerTextEngine,
    field_color: Color,
    clips: &mut Vec<RectF>,
) {
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(fragment) => {
                collect_presentation(
                    &fragment.children,
                    presentation,
                    text_engine,
                    field_color,
                    clips,
                );
            }
            SceneNode::Clip(clip) => {
                clips.push(clip.rect);
                collect_presentation(
                    &clip.children,
                    presentation,
                    text_engine,
                    field_color,
                    clips,
                );
                clips.pop();
            }
            SceneNode::Text(text) => {
                let (width, _) = text_engine.measure(&text.text, text.font_size);
                let metrics = text_engine.font_metrics(text.font_size);
                presentation.texts.push(RectF::new(
                    text.position.x,
                    text.position.y,
                    width,
                    metrics.ascent + metrics.descent,
                ));
            }
            SceneNode::Rect(field) if field.z_index == 3 && field.color == field_color => {
                presentation.value_fields.push(field.rect);
            }
            SceneNode::Rect(track) if track.z_index == 2 && track.color == field_color => {
                presentation.track = Some(track.rect);
            }
            SceneNode::RadialGauge(dial) => {
                presentation.dial = Some(dial.rect);
                presentation.dial_thickness = Some(dial.thickness);
                presentation.indicator = Some((dial.rect, dial.rect.height * 0.5));
                presentation.indicator_z = Some(dial.z_index);
            }
            SceneNode::RoundedRect(thumb) if thumb.z_index == 5 => {
                presentation.indicator = Some((thumb.rect, thumb.rect.height * 0.5));
                presentation.indicator_z = Some(thumb.z_index);
                presentation.indicator_color = Some(thumb.color);
            }
            SceneNode::Rect(handle) if handle.z_index == 4 => {
                // Keep the indicator already found before the owned value-field chrome.
                if presentation.indicator.is_none() && handle.rect.height > handle.rect.width {
                    presentation.indicator = Some((handle.rect, 0.0));
                    presentation.indicator_z = Some(handle.z_index);
                    presentation.indicator_color = Some(handle.color);
                }
            }
            SceneNode::BorderRect(border) if border.z_index >= LAYER_FOCUS => {
                presentation.cues.push(FocusCue {
                    rect: border.rect,
                    color: border.border_color,
                    width: border.border_width,
                    radius: border.radius,
                    clips: clips.clone(),
                    z_index: border.z_index,
                });
            }
            _ => {}
        }
    }
}

fn slider_scene_at(
    theme: &Theme,
    style: TrackDisplayStyle,
    focus: Option<KeyboardAuthorityMode>,
    side: ValueFieldSide,
) -> (Presentation, RectF) {
    slider_scene_case(
        theme,
        focus,
        SceneCase {
            style,
            side,
            value: 42.0,
            label: true,
            value_field: true,
        },
    )
}

#[derive(Clone, Copy)]
struct SceneCase {
    style: TrackDisplayStyle,
    side: ValueFieldSide,
    value: f32,
    label: bool,
    value_field: bool,
}

fn slider_scene_case(
    theme: &Theme,
    focus: Option<KeyboardAuthorityMode>,
    case: SceneCase,
) -> (Presentation, RectF) {
    slider_scene_case_scaled(theme, focus, case, 1.0)
}

fn slider_scene_case_scaled(
    theme: &Theme,
    focus: Option<KeyboardAuthorityMode>,
    case: SceneCase,
    scale: f32,
) -> (Presentation, RectF) {
    let mut tree = WidgetTree::new();
    let frame_width = if case.style == TrackDisplayStyle::Radial && !case.label && !case.value_field
    {
        theme.geometry.track_bar.bar_height * 2.0
    } else {
        320.0
    };
    let root = tree.create_root(
        Box::new(Container::empty().with_width(frame_width).with_height(32.0)),
        None,
        0,
    );
    tree.set_overflow(root, OverflowMode::Clip);
    let mut slider = Slider::new(0.0, 100.0)
        .with_value(case.value)
        .with_value_field_side(case.side)
        .with_display_style(case.style);
    if case.label {
        slider = slider.with_label("Alpha");
    }
    if case.value_field {
        slider = slider.with_value_display();
    }
    let control = tree.add_child(root, Box::new(slider), None, 0);
    let owner = tree.widget_id_for_node(control).unwrap();
    let mut domains = InteractionDomains::new();
    if let Some(mode) = focus {
        domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        domains.primary_state_mut().keyboard_mode = mode;
    }
    let mode = domains.primary_state().keyboard_mode;
    test_begin_frame_layout_tracking();
    let mut text_engine = FontManagerTextEngine::new();
    let scene = build_scene_with_appearance(
        &mut tree,
        root,
        Constraints::tight(Size::new(frame_width, 32.0)),
        theme,
        &mut text_engine,
        &IconRegistry::new(&[]),
        0,
        0.0,
        &mut TransitionLayer::new(),
        &mut ActiveLayoutTransitions::new(),
        None,
        scale,
        None,
        None,
        false,
        &mut Vec::new(),
        None,
        &domains,
        WindowId::PRIMARY,
    );
    assert_eq!(tree.widget_id_for_node(control), Some(owner));
    assert_eq!(domains.primary_state().keyboard_mode, mode);
    assert_eq!(
        tree.node(control)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .value(),
        case.value,
    );
    let rect = tree.get_absolute_paint_rect(control).unwrap();
    let mut presentation = Presentation::default();
    collect_presentation(
        &scene.root,
        &mut presentation,
        &mut text_engine,
        theme.colors.surfaces.content.into(),
        &mut Vec::new(),
    );
    presentation
        .value_fields
        .retain(|field| field.height == rect.height);
    (presentation, rect)
}

// JC VISUAL08A / HAL328: Capture must remain visible without a separate halo,
// while the value center, allocation and owned text regions stay stable.
#[test]
fn slider_focus_appearance_capture_modulates_the_existing_shape() {
    let theme = Theme::light();
    for style in [
        TrackDisplayStyle::Track,
        TrackDisplayStyle::Bar,
        TrackDisplayStyle::Pill,
        TrackDisplayStyle::Radial,
    ] {
        let (focused, bounds) = slider_scene_at(
            &theme,
            style,
            Some(KeyboardAuthorityMode::Navigation),
            ValueFieldSide::Right,
        );
        let (captured, captured_bounds) = slider_scene_at(
            &theme,
            style,
            Some(KeyboardAuthorityMode::Capture),
            ValueFieldSide::Right,
        );
        let (resting, resting_bounds) = slider_scene_at(&theme, style, None, ValueFieldSide::Right);
        assert_eq!(bounds, captured_bounds);
        assert_eq!(bounds, resting_bounds);
        assert!(!focused.cues.is_empty());
        assert!(
            focused
                .cues
                .iter()
                .all(|cue| cue.color != Color::from(theme.focus_ring.capture_color))
        );
        assert!(
            captured.cues.is_empty(),
            "Capture is the existing shape, not a halo: {style:?}"
        );
        assert!(resting.cues.is_empty());
        assert_eq!(captured.track, resting.track);
        assert_eq!(captured.value_fields, resting.value_fields);
        let idle = resting.indicator.unwrap().0;
        let active = captured.indicator.unwrap().0;
        assert_eq!(idle.center(), active.center());
        if style == TrackDisplayStyle::Radial {
            assert_eq!(idle, active);
            assert!(captured.dial_thickness.unwrap() > resting.dial_thickness.unwrap());
        } else {
            assert!(active.width > idle.width && active.height > idle.height);
            assert_eq!(
                captured.indicator_color,
                Some(theme.colors.accents.primary.into())
            );
        }
        for region in &captured.value_fields {
            assert!(
                active.intersection(region).is_none(),
                "active {style:?} overlaps its editor"
            );
        }
    }
}

#[test]
fn slider_focus_appearance_radial_left_value_field_keeps_dial_available() {
    let (presentation, _) = slider_scene_at(
        &Theme::default(),
        TrackDisplayStyle::Radial,
        Some(KeyboardAuthorityMode::Navigation),
        ValueFieldSide::Left,
    );
    assert!(
        presentation
            .dial
            .unwrap()
            .intersection(&presentation.value_fields[0])
            .is_none()
    );
}

// Endpoint decoration may grow, but cannot recenter the value or move the track.
#[test]
fn slider_focus_appearance_endpoint_growth_preserves_center_and_track() {
    let theme = Theme::dark();
    for value in [0.0, 100.0] {
        let case = SceneCase {
            style: TrackDisplayStyle::Track,
            side: ValueFieldSide::Left,
            value,
            label: true,
            value_field: true,
        };
        let (resting, bounds) = slider_scene_case_scaled(&theme, None, case, 1.5);
        let (captured, captured_bounds) =
            slider_scene_case_scaled(&theme, Some(KeyboardAuthorityMode::Capture), case, 1.5);
        let idle = resting.indicator.unwrap().0;
        let active = captured.indicator.unwrap().0;
        assert_eq!(bounds, captured_bounds);
        assert_eq!(resting.track, captured.track);
        assert_eq!(idle.center(), active.center());
        assert!(active.width > idle.width);
        assert!(active.left() >= bounds.left() && active.right() <= bounds.right());
        assert!(active.top() >= bounds.top() && active.bottom() <= bounds.bottom());
        assert!(captured.cues.is_empty());
    }
}

// Existing scene proof did not exercise the two distinct live owners: track drag
// and the embedded editor. Observe their real events and ordinary retained frames.
#[test]
fn slider_focus_appearance_drag_release_and_editor_keep_authority_truthful() {
    let theme = Theme::default();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Slider::new(0.0, 100.0)
                .with_value(50.0)
                .with_value_display(),
        ),
        None,
        0,
    );
    let owner = tree.widget_id_for_node(root).unwrap();
    let mut domains = InteractionDomains::new();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut frame = |tree: &mut WidgetTree,
                     domains: &InteractionDomains,
                     text_engine: &mut FontManagerTextEngine| {
        let frame = harness.frame_with_appearance(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 32.0)),
            &theme,
            text_engine,
            &IconRegistry::new(&[]),
            domains,
            WindowId::PRIMARY,
        );
        let mut projection = Presentation::default();
        collect_presentation(
            &frame.scene_root,
            &mut projection,
            text_engine,
            theme.colors.surfaces.content.into(),
            &mut Vec::new(),
        );
        let bounds = tree.get_absolute_paint_rect(root).unwrap();
        projection
            .value_fields
            .retain(|field| field.height == bounds.height);
        (projection, harness.local_fragment(owner).unwrap())
    };
    let pointer = |tree: &mut WidgetTree,
                   text_engine: &mut FontManagerTextEngine,
                   point: crate::core::math::Point,
                   kind,
                   captured,
                   mode| {
        let tree_ptr = tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            owner,
            tree_ptr,
            0,
            std::ptr::null_mut(),
            Some(text_engine),
            None,
            captured,
            false,
            mode,
        );
        assert!(tree.node_mut(root).unwrap().widget.handle_event(
            &mut cx,
            &Event::Pointer(PointerEvent {
                id: PointerId(1),
                global_pos: (point.x, point.y),
                local_pos: (point.x, point.y),
                kind,
                button: Some(PointerButton::Primary),
                modifiers: Modifiers {
                    ctrl: false,
                    alt: false,
                    shift: false,
                    meta: false
                },
                pointer_type: PointerType::Mouse,
            }),
            EventPhase::Target
        ));
        let keyboard_requested = cx.is_keyboard_capture_requested();
        // Direct owner callbacks do not run EventRouter's handled-event invalidation.
        tree.mark_paint_dirty(root);
        keyboard_requested
    };
    let (resting, _) = frame(&mut tree, &domains, &mut text_engine);
    let center = resting.indicator.unwrap().0.center();
    pointer(
        &mut tree,
        &mut text_engine,
        center,
        PointerEventKind::Down,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let (dragging, _) = frame(&mut tree, &domains, &mut text_engine);
    assert_eq!(
        domains.primary_state().keyboard_mode,
        KeyboardAuthorityMode::Navigation
    );
    assert!(dragging.indicator.unwrap().0.width > resting.indicator.unwrap().0.width);
    pointer(
        &mut tree,
        &mut text_engine,
        center,
        PointerEventKind::Up,
        true,
        KeyboardAuthorityMode::Navigation,
    );
    let (released, fragment) = frame(&mut tree, &domains, &mut text_engine);
    assert_eq!(released.indicator, resting.indicator);
    let (_, settled) = frame(&mut tree, &domains, &mut text_engine);
    assert!(std::sync::Arc::ptr_eq(&fragment, &settled));
    let value_before_editor = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<Slider>()
        .unwrap()
        .value();

    domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    domains.primary_state_mut().keyboard_mode = KeyboardAuthorityMode::Capture;
    let (captured, _) = frame(&mut tree, &domains, &mut text_engine);
    assert!(captured.indicator.unwrap().0.width > resting.indicator.unwrap().0.width);
    assert!(
        pointer(
            &mut tree,
            &mut text_engine,
            resting.value_fields[0].center(),
            PointerEventKind::Down,
            false,
            KeyboardAuthorityMode::Capture,
        ),
        "numeric-field Down must actually enter its editor"
    );
    let (editing, _) = frame(&mut tree, &domains, &mut text_engine);
    assert_eq!(
        editing.indicator, resting.indicator,
        "editor Capture must not activate the track"
    );
    assert_eq!(
        tree.node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .value(),
        value_before_editor
    );
}
