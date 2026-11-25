use crate::core::app::{build_scene_with_appearance, test_begin_frame_layout_tracking};
use crate::core::event::{
    Event, EventPhase, InteractionDomains, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers,
    PointerButton, PointerEvent, PointerEventKind, PointerId, PointerType, WindowId,
};
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::layout_transition::ActiveLayoutTransitions;
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_FOCUS, TrackDisplayStyle};
use crate::core::scene::{Color, SceneNode};
use crate::core::theme::Theme;
use crate::core::transition::TransitionLayer;
use crate::core::widget::EventContext;
use crate::core::widget_tree::{OverflowMode, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::container::Container;
use crate::widgets::range_slider::{RangeHandle, RangeSlider};

#[derive(Debug, PartialEq)]
struct FocusCue {
    rect: RectF,
    color: Color,
    width: f32,
    radius: f32,
    clips: Vec<RectF>,
}

#[derive(Default)]
struct Presentation {
    cues: Vec<FocusCue>,
    thumbs: Vec<(RectF, Color)>,
    thumb_clips: Vec<Vec<RectF>>,
    bar_handles: Vec<(RectF, Color)>,
}

fn collect_presentation(
    nodes: &[SceneNode],
    presentation: &mut Presentation,
    clips: &mut Vec<RectF>,
) {
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(fragment) => {
                collect_presentation(&fragment.children, presentation, clips);
            }
            SceneNode::Clip(clip) => {
                clips.push(clip.rect);
                collect_presentation(&clip.children, presentation, clips);
                clips.pop();
            }
            SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS => {
                presentation.cues.push(FocusCue {
                    rect: border.rect,
                    color: border.border_color,
                    width: border.border_width,
                    radius: border.radius,
                    clips: clips.clone(),
                });
            }
            SceneNode::RoundedRect(thumb) if thumb.z_index == 4 || thumb.z_index == 5 => {
                presentation.thumbs.push((thumb.rect, thumb.color));
                presentation.thumb_clips.push(clips.clone());
            }
            SceneNode::GradientRect(thumb) if thumb.z_index == 5 || thumb.z_index == 8 => {
                let base_color = nodes
                    .iter()
                    .find_map(|node| match node {
                        SceneNode::GradientRect(body)
                            if body.z_index == thumb.z_index + 1
                                && thumb.rect.contains(body.rect.center()) =>
                        {
                            body.gradient
                                .stops
                                .iter()
                                .find(|stop| stop.offset == 0.45)
                                .map(|stop| stop.color.into())
                        }
                        _ => None,
                    })
                    .expect("the thumb's material retains its canonical base-color stop");
                presentation.thumbs.push((thumb.rect, base_color));
                presentation.thumb_clips.push(clips.clone());
            }
            SceneNode::Rect(handle) if handle.z_index == 4 => {
                presentation.bar_handles.push((handle.rect, handle.color));
            }
            _ => {}
        }
    }
}

fn range_scene(
    theme: &Theme,
    style: TrackDisplayStyle,
    mode: Option<KeyboardAuthorityMode>,
) -> (Presentation, RectF) {
    range_scene_at(theme, style, mode, 25.0, 75.0, RangeHandle::Low, false)
}

fn range_scene_at(
    theme: &Theme,
    style: TrackDisplayStyle,
    mode: Option<KeyboardAuthorityMode>,
    low: f32,
    high: f32,
    handle: RangeHandle,
    parent_clip: bool,
) -> (Presentation, RectF) {
    range_scene_after_pointer(theme, style, mode, low, high, handle, parent_clip, None)
}

fn range_scene_after_pointer(
    theme: &Theme,
    style: TrackDisplayStyle,
    mode: Option<KeyboardAuthorityMode>,
    low: f32,
    high: f32,
    handle: RangeHandle,
    parent_clip: bool,
    gesture: Option<(RangeHandle, bool)>,
) -> (Presentation, RectF) {
    let mut tree = WidgetTree::new();
    let slider = Box::new(RangeSlider::new(0.0, 100.0, low, high).with_display_style(style));
    let (root, control) = if parent_clip {
        let root = tree.create_root(
            Box::new(Container::empty().with_width(320.0).with_height(32.0)),
            None,
            0,
        );
        tree.set_overflow(root, OverflowMode::Clip);
        let control = tree.add_child(root, slider, None, 0);
        (root, control)
    } else {
        let root = tree.create_root(slider, None, 0);
        (root, root)
    };
    let owner = tree.widget_id_for_node(control).unwrap();
    let mut domains = InteractionDomains::new();
    if let Some(mode) = mode {
        domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        let initial_mode = if handle == RangeHandle::High {
            KeyboardAuthorityMode::Capture
        } else {
            mode
        };
        domains.primary_state_mut().keyboard_mode = initial_mode;
        let tree_ptr = &mut tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            owner,
            tree_ptr,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            initial_mode,
        );
        let widget = &mut tree.node_mut(control).unwrap().widget;
        widget.on_focus(&mut cx);
        if initial_mode == KeyboardAuthorityMode::Capture {
            widget.on_capture_enter(&mut cx);
        }
        if handle == RangeHandle::High {
            assert!(widget.handle_event(
                &mut cx,
                &Event::Key(KeyEvent {
                    key_code: KeyCode::Tab,

                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    is_down: true,
                    is_repeat: false,
                    modifiers: Modifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                        meta: false,
                    },
                    text: None,
                }),
                EventPhase::Target,
            ));
            if mode == KeyboardAuthorityMode::Navigation {
                cx.keyboard_mode = mode;
                widget.on_capture_leave(&mut cx);
            }
        }
        domains.primary_state_mut().keyboard_mode = mode;
    }
    let authority_mode = domains.primary_state().keyboard_mode;
    test_begin_frame_layout_tracking();
    let build = |tree: &mut WidgetTree| {
        build_scene_with_appearance(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 32.0)),
            theme,
            &mut FontManagerTextEngine::new(),
            &IconRegistry::new(&[]),
            0,
            0.0,
            &mut TransitionLayer::new(),
            &mut ActiveLayoutTransitions::new(),
            None,
            1.0,
            None,
            None,
            false,
            &mut Vec::new(),
            None,
            &domains,
            WindowId::PRIMARY,
        )
    };
    let mut scene = build(&mut tree);
    if let Some((pressed, release)) = gesture {
        let mut initial = Presentation::default();
        collect_presentation(&scene.root, &mut initial, &mut Vec::new());
        initial.thumbs.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        let point = initial.thumbs[if pressed == RangeHandle::Low { 0 } else { 1 }]
            .0
            .center();
        for kind in [
            Some(PointerEventKind::Down),
            release.then_some(PointerEventKind::Up),
        ]
        .into_iter()
        .flatten()
        {
            let tree_ptr = &mut tree as *mut WidgetTree;
            let mut cx = EventContext::new(
                owner,
                tree_ptr,
                0,
                std::ptr::null_mut(),
                None,
                None,
                kind == PointerEventKind::Up,
                false,
                authority_mode,
            );
            assert!(tree.node_mut(control).unwrap().widget.handle_event(
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
        }
        scene = build(&mut tree);
    }
    assert_eq!(tree.widget_id_for_node(control), Some(owner));
    assert_eq!(domains.primary_state().keyboard_mode, authority_mode);
    let widget = &tree.node(control).unwrap().widget;
    assert!(widget.capture_eligible());
    let value = widget
        .as_any()
        .downcast_ref::<RangeSlider>()
        .unwrap()
        .value();
    assert_eq!((value.low, value.high), (low, high));
    let rect = tree.get_absolute_paint_rect(control).unwrap();
    let mut presentation = Presentation::default();
    collect_presentation(&scene.root, &mut presentation, &mut Vec::new());
    (presentation, rect)
}

fn indicators(presentation: &Presentation, style: TrackDisplayStyle) -> Vec<(RectF, Color)> {
    let mut result = match style {
        TrackDisplayStyle::Track | TrackDisplayStyle::Radial => presentation.thumbs.clone(),
        TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => presentation.bar_handles.clone(),
    };
    result.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    result
}

// HAL328 / HAL275: Capture belongs to the range and colors both endpoints.
// Navigation still identifies its current target; Capture uses no separate halo.
#[test]
fn range_slider_focus_appearance_capture_enlarges_both_round_thumbs() {
    let theme = Theme::dark();
    for style in [
        TrackDisplayStyle::Track,
        TrackDisplayStyle::Bar,
        TrackDisplayStyle::Pill,
        TrackDisplayStyle::Radial,
    ] {
        for handle in [RangeHandle::Low, RangeHandle::High] {
            let (focused, bounds) = range_scene_at(
                &theme,
                style,
                Some(KeyboardAuthorityMode::Navigation),
                25.0,
                75.0,
                handle,
                false,
            );
            let (captured, captured_bounds) = range_scene_at(
                &theme,
                style,
                Some(KeyboardAuthorityMode::Capture),
                25.0,
                75.0,
                handle,
                false,
            );
            assert_eq!(bounds, captured_bounds);
            assert!(!focused.cues.is_empty());
            assert!(
                focused
                    .cues
                    .iter()
                    .all(|cue| cue.color != Color::from(theme.focus_ring.capture_color))
            );
            assert!(captured.cues.is_empty());
            let idle = indicators(&focused, style);
            let active = indicators(&captured, style);
            let selected = if handle == RangeHandle::Low { 0 } else { 1 };
            for i in 0..2 {
                assert_eq!(idle[i].0.center(), active[i].0.center());
                if matches!(style, TrackDisplayStyle::Track | TrackDisplayStyle::Radial)
                    || i == selected
                {
                    assert!(active[i].0.width > idle[i].0.width);
                    assert!(active[i].0.height > idle[i].0.height);
                } else {
                    assert_eq!(idle[i].0, active[i].0);
                }
                assert_eq!(active[i].1, theme.track_accent_colors(true).0.into());
            }
        }
    }
}

// Either pointer target enlarges both thumbs. Release retains that size only
// while the framework still grants keyboard Capture to the range.
#[test]
fn range_slider_focus_appearance_pointer_release_preserves_widget_activity() {
    let theme = Theme::default();
    for style in [TrackDisplayStyle::Track, TrackDisplayStyle::Radial] {
        for mode in [
            None,
            Some(KeyboardAuthorityMode::Navigation),
            Some(KeyboardAuthorityMode::Capture),
        ] {
            for pressed in [RangeHandle::Low, RangeHandle::High] {
                let (resting, bounds) = range_scene(&theme, style, mode);
                let (dragging, dragging_bounds) = range_scene_after_pointer(
                    &theme,
                    style,
                    mode,
                    25.0,
                    75.0,
                    RangeHandle::Low,
                    false,
                    Some((pressed, false)),
                );
                let (released, released_bounds) = range_scene_after_pointer(
                    &theme,
                    style,
                    mode,
                    25.0,
                    75.0,
                    RangeHandle::Low,
                    false,
                    Some((pressed, true)),
                );
                assert_eq!(bounds, dragging_bounds);
                assert_eq!(bounds, released_bounds);
                let idle = indicators(&resting, style);
                let active = indicators(&dragging, style);
                for i in 0..2 {
                    assert_eq!(idle[i].0.center(), active[i].0.center());
                    assert_eq!(
                        active[i].0.width,
                        theme.geometry.slider.thumb_diameter * 1.25
                    );
                    assert_eq!(active[i].0.width, active[i].0.height);
                }
                let expected = theme
                    .track_accent_colors(mode == Some(KeyboardAuthorityMode::Capture))
                    .0;
                for (_, color) in &active {
                    assert_eq!(*color, expected.into());
                }
                assert!(dragging.cues.is_empty());
                assert_eq!(indicators(&released, style), idle);
            }
        }
    }
}

#[test]
fn range_slider_focus_appearance_endpoint_growth_is_centered_and_keeps_parent_clip() {
    let theme = Theme::default();
    let style = TrackDisplayStyle::Track;
    let (resting, bounds) = range_scene_at(&theme, style, None, 0.0, 100.0, RangeHandle::Low, true);
    let (captured, captured_bounds) = range_scene_at(
        &theme,
        style,
        Some(KeyboardAuthorityMode::Capture),
        0.0,
        100.0,
        RangeHandle::Low,
        true,
    );
    assert_eq!(bounds, captured_bounds);
    let idle = indicators(&resting, style);
    let active = indicators(&captured, style);
    for i in 0..2 {
        assert_eq!(idle[i].0.center(), active[i].0.center());
        assert!(active[i].0.width > idle[i].0.width);
        assert_eq!(
            active[i].0.width, active[i].0.height,
            "the emitted endpoint remains a whole circle"
        );
    }
    assert_eq!(resting.thumb_clips, captured.thumb_clips);
    assert!(
        captured
            .thumb_clips
            .iter()
            .all(|clips| clips.contains(&bounds))
    );
    // This proves clip inheritance, not full visibility outside a tight ancestor clip.
    assert!(captured.cues.is_empty());
}
