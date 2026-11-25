// Production scene construction needs the engine's private frame bookkeeping.
use crate::core::app::{build_scene_with_appearance, test_begin_frame_layout_tracking};
use crate::core::event::{
    Event, EventPhase, InteractionDomains, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers,
    PointerButton, PointerEvent, PointerEventKind, PointerId, PointerType, TextInputEvent,
    WindowId,
};
use crate::core::icon::IconRegistry;
use crate::core::layout::{Alignment, Constraints};
use crate::core::layout_transition::ActiveLayoutTransitions;
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::TrackDisplayStyle;
use crate::core::scene::{Color, SceneNode};
use crate::core::text_engine::TextEngine;
use crate::core::theme::Theme;
use crate::core::transition::TransitionLayer;
use crate::core::widget::{EventContext, Widget};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::slider::{Slider, ValueFieldSide};
use crate::widgets::{flex::Flex, range_slider::RangeSlider, tooltip_host::TooltipHost};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
struct Geometry {
    body: Option<RectF>,
    field: Option<RectF>,
    label: Option<RectF>,
    thumb: Option<RectF>,
    handles: Vec<RectF>,
}

fn collect_geometry(
    nodes: &[SceneNode],
    geometry: &mut Geometry,
    style: TrackDisplayStyle,
    bounds: RectF,
    field_color: Color,
    text_engine: &mut FontManagerTextEngine,
) {
    for node in nodes {
        match node {
            SceneNode::RetainedFragment(fragment) => collect_geometry(
                &fragment.children,
                geometry,
                style,
                bounds,
                field_color,
                text_engine,
            ),
            SceneNode::Clip(clip) => collect_geometry(
                &clip.children,
                geometry,
                style,
                bounds,
                field_color,
                text_engine,
            ),
            SceneNode::RadialGauge(gauge) => geometry.body = Some(gauge.rect),
            SceneNode::RoundedRect(thumb) if thumb.z_index == 5 => {
                geometry.thumb = Some(thumb.rect);
                geometry.handles.push(thumb.rect);
            }
            SceneNode::RoundedRect(thumb) if thumb.z_index == 4 => {
                geometry.handles.push(thumb.rect);
            }
            SceneNode::RoundedRect(track) if track.z_index == 2 => {
                geometry.body = Some(track.rect);
            }
            SceneNode::Rect(handle) if handle.z_index == 4 && style == TrackDisplayStyle::Bar => {
                geometry.handles.push(handle.rect);
            }
            SceneNode::Rect(rect) if rect.color == field_color => {
                if rect.z_index == 3 && rect.rect.height == bounds.height {
                    assert!(geometry.field.replace(rect.rect).is_none());
                } else if (style == TrackDisplayStyle::Track && rect.z_index == 2)
                    || (matches!(style, TrackDisplayStyle::Bar | TrackDisplayStyle::Pill)
                        && rect.z_index == 1)
                {
                    geometry.body = Some(rect.rect);
                }
            }
            SceneNode::Text(text) if text.text == "Alpha" => {
                let (width, _) = text_engine.measure(&text.text, text.font_size);
                let metrics = text_engine.font_metrics(text.font_size);
                geometry.label = Some(RectF::new(
                    text.position.x,
                    text.position.y,
                    width,
                    metrics.ascent + metrics.descent,
                ));
            }
            _ => {}
        }
    }
}

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    control: NodeId,
    text_engine: FontManagerTextEngine,
}

struct Requests {
    handled: bool,
    pointer: Option<PointerButton>,
    keyboard: bool,
}

impl Fixture {
    fn new(style: TrackDisplayStyle, side: ValueFieldSide, label: bool, field: bool) -> Self {
        let mut slider = Slider::new(0.0, 100.0)
            .with_value(42.0)
            .with_display_style(style)
            .with_value_field_side(side);
        if label {
            slider = slider.with_label("Alpha");
        }
        if field {
            slider = slider.with_value_display();
        }
        Self::from_widget(Box::new(slider), None)
    }

    fn from_widget(widget: Box<dyn Widget>, host_alignment: Option<Alignment>) -> Self {
        let mut tree = WidgetTree::new();
        let (root, control) = if let Some(alignment) = host_alignment {
            let root = tree.create_root(
                Box::new(Flex::column().with_align_items(alignment)),
                None,
                0,
            );
            let host = tree.add_child(root, Box::new(TooltipHost::new("Value")), None, 0);
            let control = tree.add_child(host, widget, None, 0);
            (root, control)
        } else {
            let root = tree.create_root(widget, None, 0);
            (root, root)
        };
        Self {
            tree,
            root,
            control,
            text_engine: FontManagerTextEngine::new(),
        }
    }

    fn scene(&mut self, theme: &Theme, style: TrackDisplayStyle) -> (Geometry, RectF) {
        self.scene_with_constraints(theme, style, Constraints::tight(Size::new(320.0, 32.0)))
    }

    fn scene_with_constraints(
        &mut self,
        theme: &Theme,
        style: TrackDisplayStyle,
        constraints: Constraints,
    ) -> (Geometry, RectF) {
        let owner = self.tree.widget_id_for_node(self.control).unwrap();
        let value = self.values();
        let domains = InteractionDomains::new();
        test_begin_frame_layout_tracking();
        let scene = build_scene_with_appearance(
            &mut self.tree,
            self.root,
            constraints,
            theme,
            &mut self.text_engine,
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
        );
        assert_eq!(self.tree.widget_id_for_node(self.control), Some(owner));
        assert_eq!(self.values(), value);
        assert_eq!(domains.primary_state().focus_manager.focused(), None);
        let bounds = self.tree.get_absolute_paint_rect(self.control).unwrap();
        let mut geometry = Geometry::default();
        collect_geometry(
            &scene.root,
            &mut geometry,
            style,
            bounds,
            theme.colors.surfaces.content.into(),
            &mut self.text_engine,
        );
        (geometry, bounds)
    }

    fn values(&self) -> (f32, Option<f32>) {
        let widget = self.tree.node(self.control).unwrap().widget.as_any();
        if let Some(slider) = widget.downcast_ref::<Slider>() {
            (slider.value(), None)
        } else {
            let value = widget.downcast_ref::<RangeSlider>().unwrap().value();
            (value.low, Some(value.high))
        }
    }

    fn value(&self) -> f32 {
        self.values().0
    }

    fn focused(&self) -> bool {
        self.tree
            .node(self.control)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .state()
            .focused
    }

    fn event(&mut self, event: Event, captured: bool, mode: KeyboardAuthorityMode) -> Requests {
        let owner = self.tree.widget_id_for_node(self.control).unwrap();
        let bounds = self.tree.get_absolute_paint_rect(self.control).unwrap();
        let tree_ptr = &mut self.tree as *mut WidgetTree;
        let mut cx = EventContext::new(
            owner,
            tree_ptr,
            0,
            std::ptr::null_mut(),
            Some(&mut self.text_engine),
            None,
            captured,
            false,
            mode,
        );
        let handled = self
            .tree
            .node_mut(self.control)
            .unwrap()
            .widget
            .handle_event(&mut cx, &event, EventPhase::Target);
        let requests = Requests {
            handled,
            pointer: cx.is_capture_requested(),
            keyboard: cx.is_keyboard_capture_requested(),
        };
        assert_eq!(self.tree.widget_id_for_node(self.control), Some(owner));
        assert_eq!(
            self.tree.get_absolute_paint_rect(self.control),
            Some(bounds)
        );
        requests
    }

    fn pointer(&mut self, kind: PointerEventKind, x: f32, y: f32, captured: bool) -> Requests {
        self.event(
            Event::Pointer(PointerEvent {
                id: PointerId(1),
                global_pos: (x, y),
                local_pos: (x, y),
                kind,
                button: Some(PointerButton::Primary),
                modifiers: modifiers(),
                pointer_type: PointerType::Mouse,
            }),
            captured,
            KeyboardAuthorityMode::Navigation,
        )
    }
}

fn modifiers() -> Modifiers {
    Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    }
}

fn assert_inside(inner: RectF, outer: RectF) {
    let epsilon = 1.0;
    assert!(inner.x >= outer.x - epsilon && inner.y >= outer.y - epsilon);
    assert!(inner.x + inner.width <= outer.x + outer.width + epsilon);
    assert!(inner.y + inner.height <= outer.y + outer.height + epsilon);
}

fn assert_placement(geometry: &Geometry, bounds: RectF, side: ValueFieldSide, gap: f32) {
    let painted_body = geometry.body.unwrap();
    let field = geometry.field.unwrap();
    let body = if geometry.thumb.is_some() {
        let slot_left = match side {
            ValueFieldSide::Left => field.x + field.width + gap,
            ValueFieldSide::Right => bounds.x,
        };
        let slot_right = geometry.label.map_or_else(
            || match side {
                ValueFieldSide::Left => bounds.x + bounds.width,
                ValueFieldSide::Right => field.x - gap,
            },
            |label| label.x - gap,
        );
        let left_clearance = painted_body.x - slot_left;
        let right_clearance = slot_right - painted_body.x - painted_body.width;
        assert!(left_clearance > 0.0 && right_clearance > 0.0);
        assert!((left_clearance - right_clearance).abs() <= 1.0);
        RectF::new(
            slot_left,
            painted_body.y,
            slot_right - slot_left,
            painted_body.height,
        )
    } else {
        painted_body
    };
    assert!(
        body.intersection(&field).is_none(),
        "body {body:?} overlaps field {field:?}"
    );
    assert_inside(body, bounds);
    assert_inside(field, bounds);
    let after_body = if let Some(label) = geometry.label {
        assert!(body.intersection(&label).is_none());
        assert!(field.intersection(&label).is_none());
        assert!((label.x - body.x - body.width - gap).abs() <= 1.0);
        assert_inside(label, bounds);
        label.x + label.width
    } else {
        body.x + body.width
    };
    match side {
        ValueFieldSide::Left => {
            assert!((field.x - bounds.x).abs() <= 1.0);
            assert!((body.x - field.x - field.width - gap).abs() <= 1.0);
        }
        ValueFieldSide::Right => {
            assert!((body.x - bounds.x).abs() <= 1.0);
            assert!((field.x - after_body - gap).abs() <= 1.0);
        }
    }
}

#[test]
fn slider_label_pointer_click_does_not_change_value_or_start_drag() {
    let theme = Theme::light();
    for (style, side) in [
        (TrackDisplayStyle::Track, ValueFieldSide::Right),
        (TrackDisplayStyle::Bar, ValueFieldSide::Right),
        (TrackDisplayStyle::Pill, ValueFieldSide::Right),
        (TrackDisplayStyle::Radial, ValueFieldSide::Right),
        (TrackDisplayStyle::Track, ValueFieldSide::Left),
    ] {
        let mut fixture = Fixture::new(style, side, true, true);
        let (geometry, _) = fixture.scene(&theme, style);
        let label = geometry.label.unwrap();
        let requests = fixture.pointer(
            PointerEventKind::Down,
            label.x + label.width * 0.5,
            label.y + label.height * 0.5,
            false,
        );
        assert!(requests.handled, "{style:?}: label remains hit-testable");
        assert!(fixture.focused(), "{style:?}: label did not focus Slider");
        assert_eq!(
            fixture.value(),
            42.0,
            "{style:?}: label click changed value"
        );
        assert_eq!(requests.pointer, None, "{style:?}: label started capture");
        assert!(!requests.keyboard);
        let body = geometry.body.unwrap();
        let moved = fixture.pointer(
            PointerEventKind::Move,
            body.x + body.width * 0.75,
            body.y + body.height * 0.5,
            false,
        );
        assert!(!moved.handled, "{style:?}: label click started a drag");
        assert_eq!(fixture.value(), 42.0);

        let down = fixture.pointer(
            PointerEventKind::Down,
            body.x + body.width * 0.25,
            body.y + body.height * 0.5,
            false,
        );
        assert!(down.handled);
        assert_eq!(down.pointer, Some(PointerButton::Primary));
        let initial = fixture.value();
        assert_ne!(initial, 42.0, "{style:?}: body click did not change value");
        let moved = fixture.pointer(
            PointerEventKind::Move,
            body.x + body.width * 0.75,
            body.y + body.height * 0.5,
            true,
        );
        assert!(moved.handled);
        assert_ne!(
            fixture.value(),
            initial,
            "{style:?}: body drag did not move"
        );
    }
}

#[test]
fn slider_label_pointer_outside_actual_label_preserves_slider_input() {
    for (style, side) in [
        (TrackDisplayStyle::Track, ValueFieldSide::Left),
        (TrackDisplayStyle::Radial, ValueFieldSide::Right),
    ] {
        let mut fixture = Fixture::new(style, side, true, true);
        let (geometry, bounds) = fixture.scene(&Theme::light(), style);
        let body = geometry.body.unwrap();
        let label = geometry.label.unwrap();
        let (x, y) = match style {
            TrackDisplayStyle::Track => (
                (body.x + body.width + label.x) * 0.5,
                label.y + label.height * 0.5,
            ),
            _ => (label.x + label.width * 0.5, bounds.y + bounds.height - 1.0),
        };
        let point = Point::new(x, y);
        assert!(bounds.contains(point));
        assert!(x > body.x + body.width);
        assert!(!label.contains(point));
        assert!(!geometry.field.unwrap().contains(point));
        let down = fixture.pointer(PointerEventKind::Down, x, y, false);
        assert!(down.handled && fixture.focused());
        assert_eq!(down.pointer, Some(PointerButton::Primary));
        assert_ne!(
            fixture.value(),
            42.0,
            "{style:?}: non-label click was swallowed"
        );
    }
}

#[test]
fn slider_label_pointer_absence_preserves_track_drag() {
    let style = TrackDisplayStyle::Track;
    let mut fixture = Fixture::new(style, ValueFieldSide::Right, false, false);
    let (geometry, _) = fixture.scene(&Theme::light(), style);
    assert!(geometry.label.is_none());
    let body = geometry.body.unwrap();
    let down = fixture.pointer(
        PointerEventKind::Down,
        body.x + body.width * 0.25,
        body.y + body.height * 0.5,
        false,
    );
    assert!(down.handled);
    assert_eq!(down.pointer, Some(PointerButton::Primary));
    let initial = fixture.value();
    let moved = fixture.pointer(
        PointerEventKind::Move,
        body.x + body.width * 0.75,
        body.y + body.height * 0.5,
        true,
    );
    assert!(moved.handled);
    assert!(initial > 0.0 && fixture.value() > initial && fixture.value() < 100.0);
}

#[test]
fn radial_value_field_side_geometry_uses_reserved_width() {
    for theme in [Theme::light(), Theme::dark()] {
        for label in [false, true] {
            let mut widths = Vec::new();
            for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
                let mut fixture = Fixture::new(TrackDisplayStyle::Radial, side, label, true);
                let (geometry, bounds) = fixture.scene(&theme, TrackDisplayStyle::Radial);
                assert_placement(&geometry, bounds, side, theme.geometry.slider.label_gap);
                assert_eq!(geometry.label.is_some(), label);
                widths.push(bounds.width);
            }
            assert_eq!(widths[0], widths[1]);
        }
    }
}

#[test]
fn radial_pointer_mapping_uses_the_painted_dial_origin() {
    for theme in [Theme::light(), Theme::dark()] {
        for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
            let mut fixture = Fixture::new(TrackDisplayStyle::Radial, side, true, true);
            let (geometry, _) = fixture.scene(&theme, TrackDisplayStyle::Radial);
            let dial = geometry.body.unwrap();
            let x = dial.x + dial.width * 0.5;
            let y = dial.y + dial.height * 0.5;
            let radius = dial.width * 0.35;
            let positions = [
                (x, y - radius, 0.0),
                (x + radius, y, 25.0),
                (x, y + radius, 50.0),
                (x - radius, y, 75.0),
            ];
            for (index, (px, py, expected)) in positions.into_iter().enumerate() {
                let kind = if index == 0 {
                    PointerEventKind::Down
                } else {
                    PointerEventKind::Move
                };
                let requests = fixture.pointer(kind, px, py, index != 0);
                assert!(requests.handled);
                assert!(!requests.keyboard);
                if index == 0 {
                    assert_eq!(requests.pointer, Some(PointerButton::Primary));
                }
                assert!(
                    (fixture.value() - expected).abs() < 0.001,
                    "{side:?}: painted point ({px}, {py}) mapped to {}, expected {expected}",
                    fixture.value()
                );
            }
        }
    }
}

#[test]
fn radial_value_field_hit_region_enters_the_existing_editor() {
    for theme in [Theme::light(), Theme::dark()] {
        for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
            let mut fixture = Fixture::new(TrackDisplayStyle::Radial, side, true, true);
            let (geometry, _) = fixture.scene(&theme, TrackDisplayStyle::Radial);
            let field = geometry.field.unwrap();
            let x = field.x + field.width - 1.0;
            let y = field.y + field.height * 0.5;
            let single = fixture.pointer(PointerEventKind::Down, x, y, false);
            assert!(single.handled);
            assert_eq!(single.pointer, None);
            assert!(single.keyboard);
            assert_eq!(fixture.value(), 42.0);
            assert!(fixture.pointer(PointerEventKind::Up, x, y, false).handled);
            let text = fixture.event(
                Event::Text(TextInputEvent {
                    text: "73".into(),
                    modifiers: modifiers(),
                }),
                false,
                KeyboardAuthorityMode::Capture,
            );
            assert!(text.handled);
            assert_eq!(fixture.value(), 42.0);
            let commit = fixture.event(
                Event::Key(KeyEvent {
                    key_code: KeyCode::Enter,

                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                    is_down: true,
                    is_repeat: false,
                    modifiers: modifiers(),
                    text: None,
                }),
                false,
                KeyboardAuthorityMode::Capture,
            );
            assert!(commit.handled);
            assert_eq!(fixture.value(), 73.0);
        }
    }
}

#[test]
fn slider_label_edges_keep_logical_hits_when_the_track_origin_is_snapped() {
    for style in [
        TrackDisplayStyle::Track,
        TrackDisplayStyle::Bar,
        TrackDisplayStyle::Pill,
        TrackDisplayStyle::Radial,
    ] {
        for edge in [0.01, -0.01] {
            let mut fixture = Fixture::new(style, ValueFieldSide::Left, true, true);
            let (geometry, _) = fixture.scene(&Theme::light(), style);
            let label = geometry.label.unwrap();
            let x = if edge > 0.0 {
                label.x + edge
            } else {
                label.right() + edge
            };
            let down = fixture.pointer(
                PointerEventKind::Down,
                x,
                label.y + label.height * 0.5,
                false,
            );
            assert!(down.handled);
            assert_eq!(down.pointer, None, "{style:?}: label edge starts a drag");
            assert!(!down.keyboard);
            assert_eq!(fixture.value(), 42.0, "{style:?}: label edge changes value");
        }
    }
}

#[test]
fn slider_track_thumb_centers_map_to_exact_endpoints_with_cue_clearance() {
    for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
        for value in [0.0, 100.0] {
            let mut fixture = Fixture::new(TrackDisplayStyle::Track, side, true, true);
            fixture
                .tree
                .node_mut(fixture.root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Slider>()
                .unwrap()
                .set_value(value);
            let (geometry, _) = fixture.scene(&Theme::light(), TrackDisplayStyle::Track);
            let thumb = geometry.thumb.unwrap();
            let track = geometry.body.unwrap();
            if value == 0.0 {
                assert!((thumb.x - track.x).abs() < 0.001);
            } else {
                assert!((thumb.right() - track.right()).abs() < 0.001);
            }
            let down = fixture.pointer(
                PointerEventKind::Down,
                thumb.x + thumb.width * 0.5,
                thumb.y + thumb.height * 0.5,
                false,
            );
            assert!(down.handled && down.pointer == Some(PointerButton::Primary));
            assert_eq!(
                fixture.value(),
                value,
                "{side:?}: endpoint moved on its thumb"
            );
        }
    }
}

#[test]
fn radial_left_without_value_field_preserves_the_right_origin() {
    for theme in [Theme::light(), Theme::dark()] {
        let mut geometries = Vec::new();
        for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
            let mut fixture = Fixture::new(TrackDisplayStyle::Radial, side, true, false);
            let (geometry, bounds) = fixture.scene(&theme, TrackDisplayStyle::Radial);
            assert_eq!(geometry.body.unwrap().x, bounds.x);
            assert!(geometry.field.is_none());
            geometries.push((geometry.body, geometry.label, bounds));
        }
        assert_eq!(geometries[0], geometries[1]);
    }
}

#[test]
fn track_bar_pill_value_field_side_preserves_geometry_and_pointer_mapping() {
    for theme in [Theme::light(), Theme::dark()] {
        for style in [
            TrackDisplayStyle::Track,
            TrackDisplayStyle::Bar,
            TrackDisplayStyle::Pill,
        ] {
            for side in [ValueFieldSide::Left, ValueFieldSide::Right] {
                let mut fixture = Fixture::new(style, side, true, true);
                let (geometry, bounds) = fixture.scene(&theme, style);
                assert_placement(&geometry, bounds, side, theme.geometry.slider.label_gap);
                let body = geometry.body.unwrap();
                let requests = fixture.pointer(
                    PointerEventKind::Down,
                    body.x + body.width * 0.5,
                    body.y + body.height * 0.5,
                    false,
                );
                assert!(requests.handled && !requests.keyboard);
                assert_eq!(requests.pointer, Some(PointerButton::Primary));
                // The scene body is pixel-snapped; input retains logical geometry.
                assert!(
                    (fixture.value() - 50.0).abs() <= 100.0 / body.width,
                    "{style:?}/{side:?}: midpoint mapped to {}",
                    fixture.value()
                );
            }
        }
    }
}

#[test]
fn slider_explicit_width_propagates_through_stretched_tooltip_and_resize() {
    let theme = Theme::light();
    let style = TrackDisplayStyle::Track;
    let make = || {
        Slider::new(0.0, 100.0)
            .with_value(42.0)
            .with_label("Alpha")
            .with_value_display()
    };
    let loose = Constraints::new(0.0, 384.0, 0.0, f32::INFINITY);
    let mut direct = Fixture::from_widget(Box::new(make()), None);
    let (_, natural) = direct.scene_with_constraints(&theme, style, loose);
    let mut start = Fixture::from_widget(Box::new(make()), Some(Alignment::Start));
    let (_, start_bounds) = start.scene_with_constraints(&theme, style, loose);
    assert_eq!(start_bounds.width, natural.width);
    assert!(natural.width < loose.max_width);

    let changes = Rc::new(Cell::new(0));
    let observed = changes.clone();
    let mut hosted = Fixture::from_widget(
        Box::new(make().on_value_changed(move |_| observed.set(observed.get() + 1))),
        Some(Alignment::Stretch),
    );
    let (before, bounds) = hosted.scene_with_constraints(&theme, style, loose);
    let (direct_geometry, direct_bounds) = direct.scene_with_constraints(
        &theme,
        style,
        Constraints::new(384.0, 384.0, 0.0, f32::INFINITY),
    );
    assert_eq!(bounds.width, loose.max_width);
    assert_eq!(direct_bounds, bounds);
    assert_eq!(direct_geometry.body, before.body);
    let wider = Constraints::new(0.0, 512.0, 0.0, f32::INFINITY);
    let (after, resized) = hosted.scene_with_constraints(&theme, style, wider);
    assert_eq!(resized.width, wider.max_width);
    assert_eq!(resized.height, bounds.height);
    assert!(
        (after.body.unwrap().width - before.body.unwrap().width - (resized.width - bounds.width))
            .abs()
            <= 1.0
    );
    assert_eq!(after.field.unwrap().width, before.field.unwrap().width);
    assert_eq!(after.label.unwrap().width, before.label.unwrap().width);
    assert!((after.field.unwrap().right() - resized.right()).abs() <= 1.0);
    assert_eq!(changes.get(), 0, "allocation must not emit a value");
    let body = after.body.unwrap();
    let down = hosted.pointer(
        PointerEventKind::Down,
        body.x + body.width * 0.5,
        body.y + body.height * 0.5,
        false,
    );
    assert_eq!(down.pointer, Some(PointerButton::Primary));
    assert!((hosted.value() - 50.0).abs() <= 100.0 / body.width);
    assert_eq!(changes.get(), 1);
}

#[test]
fn range_slider_allocated_track_matches_both_pointer_handles() {
    let theme = Theme::light();
    for style in [TrackDisplayStyle::Track, TrackDisplayStyle::Bar] {
        let make = || {
            RangeSlider::new(0.0, 100.0, 0.0, 100.0)
                .with_label("Alpha")
                .with_display_style(style)
        };
        let mut direct = Fixture::from_widget(Box::new(make()), None);
        let (_, natural) = direct.scene_with_constraints(
            &theme,
            style,
            Constraints::new(0.0, 384.0, 0.0, f32::INFINITY),
        );
        assert!(natural.width < 384.0);
        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        let mut fixture = Fixture::from_widget(
            Box::new(make().on_change(move |_| observed.set(observed.get() + 1))),
            Some(Alignment::Stretch),
        );
        let (before, _) = fixture.scene_with_constraints(
            &theme,
            style,
            Constraints::new(0.0, 384.0, 0.0, f32::INFINITY),
        );
        let (mut after, bounds) = fixture.scene_with_constraints(
            &theme,
            style,
            Constraints::new(0.0, 512.0, 0.0, f32::INFINITY),
        );
        assert_eq!(bounds.width, 512.0);
        let body = after.body.unwrap();
        assert!((body.width - before.body.unwrap().width - 128.0).abs() <= 1.0);
        assert!((after.label.unwrap().right() - bounds.right()).abs() <= 1.0);
        assert_eq!(fixture.values(), (0.0, Some(100.0)));
        assert_eq!(changes.get(), 0);
        let label = after.label.unwrap();
        let label_down = fixture.pointer(
            PointerEventKind::Down,
            label.x + label.width * 0.5,
            label.y + label.height * 0.5,
            false,
        );
        assert_eq!(label_down.pointer, None);
        assert_eq!(changes.get(), 0);
        after.handles.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert_eq!(after.handles.len(), 2, "range keeps two handles");
        let low = after.handles[0];
        let high = after.handles[1];
        let low_x = low.x + low.width * 0.5;
        let high_x = high.x + high.width * 0.5;
        let y = body.y + body.height * 0.5;
        for x in [low_x, high_x] {
            let down = fixture.pointer(PointerEventKind::Down, x, y, false);
            assert_eq!(down.pointer, Some(PointerButton::Primary));
            fixture.pointer(PointerEventKind::Up, x, y, true);
            assert_eq!(fixture.values(), (0.0, Some(100.0)));
        }
        assert_eq!(changes.get(), 0, "clicking painted handle must not move it");
        fixture.pointer(PointerEventKind::Down, high_x, y, false);
        fixture.pointer(PointerEventKind::Move, (low_x + high_x) * 0.5, y, true);
        fixture.pointer(PointerEventKind::Up, (low_x + high_x) * 0.5, y, true);
        assert_eq!(fixture.values().0, 0.0);
        assert!((fixture.values().1.unwrap() - 50.0).abs() < 0.001);
        fixture.pointer(PointerEventKind::Down, low_x, y, false);
        fixture.pointer(
            PointerEventKind::Move,
            low_x + (high_x - low_x) * 0.25,
            y,
            true,
        );
        assert!((fixture.values().0 - 25.0).abs() < 0.001);
        assert!((fixture.values().1.unwrap() - 50.0).abs() < 0.001);
        assert_eq!(changes.get(), 2);
        let current = fixture.values();
        fixture.scene_with_constraints(
            &theme,
            style,
            Constraints::new(0.0, label.width, 0.0, f32::INFINITY),
        );
        let collapsed = fixture.pointer(PointerEventKind::Move, 0.0, y, true);
        assert!(!collapsed.handled);
        fixture.pointer(PointerEventKind::Up, 0.0, y, true);
        let down = fixture.pointer(PointerEventKind::Down, 0.0, y, false);
        assert_eq!(down.pointer, None);
        assert_eq!(fixture.values(), current);
        assert_eq!(
            changes.get(),
            2,
            "collapsed track must not manufacture a value"
        );
        let (restored, _) = fixture.scene_with_constraints(
            &theme,
            style,
            Constraints::new(0.0, 512.0, 0.0, f32::INFINITY),
        );
        assert_eq!(restored.body.unwrap().width, body.width);
        assert_eq!(fixture.values(), current);
    }

    let changes = Rc::new(Cell::new(0));
    let observed = changes.clone();
    let mut fixture = Fixture::from_widget(
        Box::new(
            RangeSlider::new(0.0, 100.0, 25.0, 75.0)
                .on_change(move |_| observed.set(observed.get() + 1)),
        ),
        None,
    );
    let style = TrackDisplayStyle::Track;
    let full = Constraints::new(384.0, 384.0, 0.0, f32::INFINITY);
    let diameter = theme.geometry.slider.thumb_diameter;
    for handle in 0..2 {
        let (mut geometry, _) = fixture.scene_with_constraints(&theme, style, full);
        geometry.handles.sort_by(|a, b| a.x.total_cmp(&b.x));
        let thumb = geometry.handles[handle];
        let x = thumb.x + thumb.width * 0.5;
        let y = thumb.y + thumb.height * 0.5;
        let down = fixture.pointer(PointerEventKind::Down, x, y, false);
        assert_eq!(down.pointer, Some(PointerButton::Primary));
        assert_eq!(fixture.values(), (25.0, Some(75.0)));
        let (collapsed, collapsed_bounds) = fixture.scene_with_constraints(
            &theme,
            style,
            Constraints::new(diameter, diameter, 0.0, f32::INFINITY),
        );
        assert_eq!(collapsed_bounds.width, diameter);
        let thumb = collapsed.handles[0];
        let center = thumb.x + thumb.width * 0.5;
        let moved = fixture.pointer(PointerEventKind::Move, center, y, true);
        assert!(!moved.handled);
        assert_eq!(fixture.values(), (25.0, Some(75.0)));
        assert_eq!(changes.get(), 0, "no travel must not emit a value");
        assert!(
            fixture
                .pointer(PointerEventKind::Up, center, y, true)
                .handled
        );
        let down = fixture.pointer(PointerEventKind::Down, center, y, false);
        assert!(!down.handled);
        assert_eq!(down.pointer, None);
        fixture.scene_with_constraints(&theme, style, full);
        assert_eq!(fixture.values(), (25.0, Some(75.0)));
        assert_eq!(changes.get(), 0);
        assert!(
            !fixture.pointer(PointerEventKind::Move, x, y, false).handled,
            "Up must retire the old drag"
        );
        assert_eq!(fixture.values(), (25.0, Some(75.0)));
        assert_eq!(changes.get(), 0);
    }
}

// INV-ALIGN-FINAL-GEOMETRY/OWNED-PARTS: shared logical endpoints must reach
// input before Paint; the older fixture covers only individual allocations.
#[test]
fn slider_alignment_owned_parts_are_current_before_paint() {
    use crate::core::app::{RetainedSceneTestHarness, layout_tree};
    use crate::widgets::ControlAlignmentGroup;
    let theme = Theme::light();
    let changes = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ControlAlignmentGroup::column()), None, 0);
    let host = tree.add_child(root, Box::new(TooltipHost::new("alpha")), None, 0);
    let observed = changes.clone();
    let first = tree.add_child(
        host,
        Box::new(
            Slider::new(0.0, 100.0)
                .with_label("Alpha")
                .with_value(42.0)
                .with_value_display()
                .on_value_changed(move |_| observed.set(observed.get() + 1)),
        ),
        None,
        0,
    );
    let second = tree.add_child(
        root,
        Box::new(
            Slider::new(0.0, 100.0)
                .with_label("Longer label")
                .with_value(75.0)
                .with_value_display()
                .with_value_field_side(ValueFieldSide::Left),
        ),
        None,
        1,
    );
    let range = tree.add_child(
        root,
        Box::new(RangeSlider::new(0.0, 100.0, 25.0, 75.0).with_label("Range")),
        None,
        2,
    );
    for row in [host, second, range] {
        tree.set_control_alignment_member(row, true);
    }
    let mut fixture = Fixture {
        tree,
        root,
        control: first,
        text_engine: FontManagerTextEngine::new(),
    };
    let layout_only = |fixture: &mut Fixture, constraints| {
        test_begin_frame_layout_tracking();
        let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
        layout_tree(
            &mut fixture.tree,
            root,
            constraints,
            &theme,
            &mut fixture.text_engine,
            None,
        );
    };
    let constraints = Constraints::new(420.0, 420.0, 0.0, f32::INFINITY);
    layout_only(&mut fixture, constraints);
    // Numeric entry works on the first input even though no Paint has populated legacy caches.
    let right_x = 419.0;
    let entry = fixture.pointer(
        PointerEventKind::Down,
        right_x,
        theme.geometry.text_field.height * 0.5,
        false,
    );
    assert!(entry.keyboard && entry.handled);
    assert_eq!(fixture.value(), 42.0);
    fixture.event(
        Event::Key(KeyEvent {
            key_code: KeyCode::Escape,
            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down: true,
            is_repeat: false,
            modifiers: modifiers(),
            text: None,
        }),
        false,
        KeyboardAuthorityMode::Capture,
    );
    let mut harness = RetainedSceneTestHarness::new();
    harness.frame(
        &mut fixture.tree,
        root,
        constraints,
        &theme,
        &mut fixture.text_engine,
        &IconRegistry::new(&[]),
    );
    let mut bodies = Vec::new();
    for control in [first, second, range] {
        let owner = fixture.tree.widget_id_for_node(control).unwrap();
        let fragment = harness.local_fragment(owner).unwrap();
        let bounds = fixture.tree.get_absolute_paint_rect(control).unwrap();
        let mut geometry = Geometry::default();
        collect_geometry(
            &fragment,
            &mut geometry,
            TrackDisplayStyle::Track,
            bounds,
            theme.colors.surfaces.content.into(),
            &mut fixture.text_engine,
        );
        if control == range {
            // BorderStyle emits the outer border and its inset fill; compare the
            // visible envelope, not the last (inner) fill observed by the legacy helper.
            geometry.body = fragment
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(r) if r.z_index == 2 => Some(r.rect),
                    _ => None,
                })
                .reduce(|a, b| {
                    RectF::new(
                        a.x.min(b.x),
                        a.y.min(b.y),
                        a.right().max(b.right()) - a.x.min(b.x),
                        a.bottom().max(b.bottom()) - a.y.min(b.y),
                    )
                });
        }
        bodies.push(geometry.body.unwrap());
    }
    for body in &bodies[1..] {
        assert_eq!(body.x, bodies[0].x);
        assert_eq!(body.right(), bodies[0].right());
    }
    assert_eq!(changes.get(), 0);
    // After resize, mapping uses the current Layout rather than the preceding painted interval.
    let wider = Constraints::new(560.0, 560.0, 0.0, f32::INFINITY);
    fixture.tree.mark_layout_dirty(root);
    layout_only(&mut fixture, wider);
    let midpoint =
        (bodies[0].x + bodies[0].right() + wider.min_width - constraints.min_width) * 0.5;
    fixture.control = range;
    let range_end = bodies[0].right() + wider.min_width - constraints.min_width - 1.0;
    assert_eq!(
        fixture
            .pointer(
                PointerEventKind::Down,
                range_end,
                theme.geometry.slider.thumb_diameter * 0.5,
                false
            )
            .pointer,
        Some(PointerButton::Primary)
    );
    assert_eq!(fixture.values(), (25.0, Some(100.0)));
    fixture.pointer(PointerEventKind::Up, range_end, 0.0, true);
    fixture.control = first;
    let down = fixture.pointer(
        PointerEventKind::Down,
        midpoint,
        theme.geometry.text_field.height * 0.5,
        false,
    );
    assert_eq!(down.pointer, Some(PointerButton::Primary));
    assert!(
        (fixture.value() - 50.0).abs()
            <= 100.0 / (bodies[0].width + wider.min_width - constraints.min_width)
    );
    fixture.pointer(PointerEventKind::Up, midpoint, 0.0, true);
    let value = fixture.value();
    let emissions = changes.get();
    // Collapse with a live gesture, then retire it through Up without fabricating a value.
    fixture.pointer(
        PointerEventKind::Down,
        midpoint,
        theme.geometry.text_field.height * 0.5,
        false,
    );
    assert_eq!(fixture.value(), value);
    let no_travel_width =
        constraints.min_width - bodies[0].width + theme.geometry.slider.thumb_diameter * 0.5;
    layout_only(
        &mut fixture,
        Constraints::tight(Size::new(no_travel_width, 1.0)),
    );
    fixture.pointer(PointerEventKind::Move, 0.0, 0.0, true);
    assert_eq!(fixture.value(), value);
    fixture.pointer(PointerEventKind::Up, 0.0, 0.0, true);
    assert_eq!(
        fixture
            .pointer(PointerEventKind::Down, 0.0, 0.0, false)
            .pointer,
        None
    );
    assert_eq!(changes.get(), emissions);
    layout_only(&mut fixture, Constraints::tight(Size::new(1.0, 1.0)));
    // The second row has logical content outside the allocated height, but tree routing clips it.
    let second_bounds = fixture.tree.get_absolute_paint_rect(second).unwrap();
    assert!(
        fixture
            .tree
            .hit_test(0.5, second_bounds.y + second_bounds.height * 0.5)
            .is_none()
    );
    let content = fixture.tree.get_content_size(root).unwrap();
    assert!(content.width > 1.0 && content.height > 1.0);
    layout_only(&mut fixture, wider);
    assert_eq!(fixture.value(), value);
    assert_eq!(changes.get(), emissions);
}

// INV-ALIGN-LOCAL/CACHE-DEPENDENCIES: actual mounted retained output must
// refresh affected rows while an independent group and settled frames reuse.
#[test]
fn slider_alignment_local_dependencies_preserve_retained_peers() {
    use crate::core::app::RetainedSceneTestHarness;
    use crate::widgets::ControlAlignmentGroup;
    let theme = Theme::light();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ControlAlignmentGroup::column()), None, 0);
    let independent = tree.add_child(root, Box::new(ControlAlignmentGroup::column()), None, 0);
    let peer = tree.add_child(
        independent,
        Box::new(Slider::new(0.0, 100.0).with_label("Independent")),
        None,
        0,
    );
    let a = tree.add_child(
        root,
        Box::new(Slider::new(0.0, 100.0).with_label("Alpha").with_value(42.0)),
        None,
        1,
    );
    let longest = tree.add_child(
        root,
        Box::new(RangeSlider::new(0.0, 100.0, 25.0, 75.0).with_label("The longest local label")),
        None,
        2,
    );
    for member in [a, longest, peer] {
        tree.set_control_alignment_member(member, true);
    }
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let constraints = Constraints::new(480.0, 480.0, 0.0, f32::INFINITY);
    let mut harness = RetainedSceneTestHarness::new();
    let body = |harness: &RetainedSceneTestHarness,
                tree: &WidgetTree,
                text: &mut FontManagerTextEngine| {
        let mut geometry = Geometry::default();
        collect_geometry(
            &harness
                .local_fragment(tree.widget_id_for_node(a).unwrap())
                .unwrap(),
            &mut geometry,
            TrackDisplayStyle::Track,
            tree.get_absolute_paint_rect(a).unwrap(),
            theme.colors.surfaces.content.into(),
            text,
        );
        geometry.body.unwrap()
    };
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let owner = tree.widget_id_for_node(a).unwrap();
    let peer_owner = tree.widget_id_for_node(peer).unwrap();
    let old = harness.local_fragment(owner).unwrap();
    let old_peer = harness.local_fragment(peer_owner).unwrap();
    let before = body(&harness, &tree, &mut text);
    tree.replace_widget_only(
        longest,
        Box::new(RangeSlider::new(0.0, 100.0, 25.0, 75.0).with_label("Still widest")),
    );
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let updated = harness.local_fragment(owner).unwrap();
    assert!(body(&harness, &tree, &mut text).right() > before.right());
    assert!(!std::sync::Arc::ptr_eq(&old, &updated));
    assert!(std::sync::Arc::ptr_eq(
        &old_peer,
        &harness.local_fragment(peer_owner).unwrap()
    ));
    tree.set_control_alignment_member(a, true);
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &theme, &mut text, &icons)
            .scene_built
    );
    let before_removal = body(&harness, &tree, &mut text);
    tree.remove_node(longest);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(body(&harness, &tree, &mut text).right() > before_removal.right());
    // Existing topology-generation eviction is global; the next nonstructural
    // updates must reuse this current independent fragment.
    let current_peer = harness.local_fragment(peer_owner).unwrap();
    let aligned = body(&harness, &tree, &mut text);
    tree.set_control_alignment_member(a, false);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(body(&harness, &tree, &mut text).x > aligned.x);
    tree.replace_widget_only(
        a,
        Box::new(
            Slider::new(0.0, 100.0)
                .with_label("Changed label and metrics")
                .with_value(42.0),
        ),
    );
    tree.set_control_alignment_member(a, true);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(body(&harness, &tree, &mut text).x, aligned.x);
    assert!(std::sync::Arc::ptr_eq(
        &current_peer,
        &harness.local_fragment(peer_owner).unwrap()
    ));
    let before_font = body(&harness, &tree, &mut text);
    let mut larger = theme.clone();
    larger.typography.font_size *= 1.5;
    for id in [a, peer] {
        tree.mark_layout_dirty(id);
    }
    harness.frame(&mut tree, root, constraints, &larger, &mut text, &icons);
    assert!(body(&harness, &tree, &mut text).width < before_font.width);
    assert_eq!(
        tree.node(a)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .value(),
        42.0
    );
    assert!(
        !harness
            .frame(&mut tree, root, constraints, &larger, &mut text, &icons)
            .scene_built
    );
    // Insertion into a settled root must reach Layout even for a fresh dirty subtree.
    let added_group = tree.add_child(root, Box::new(ControlAlignmentGroup::column()), None, 3);
    let added = tree.add_child(
        added_group,
        Box::new(Slider::new(0.0, 100.0).with_label("New local group")),
        None,
        0,
    );
    tree.set_control_alignment_member(added, true);
    harness.frame(&mut tree, root, constraints, &larger, &mut text, &icons);
    assert!(tree.get_rect(added).unwrap().height > 0.0);
}

// Mixed alignment contract: actual editor and popup owners must receive body-local
// input; slider-only proofs cannot detect a caption host rebasing input incorrectly.
#[test]
fn slider_alignment_mixed_editors_keep_body_routing_and_stable_columns() {
    use crate::core::app::{RetainedSceneTestHarness, layout_tree};
    use crate::core::event::{
        CursorIcon, FusedInputEvent, InteractionDomainState, PointerButtonsState, PointerDevice,
        ScrollSource,
    };
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::text_widget_test_harness::{
        TextWidgetTestHarness, TextWidgetTestHarnessRead,
    };
    use crate::widgets::{ComboBox, ControlAlignmentGroup, ControlCaption, text_field::TextField};
    use std::time::Instant;

    let theme = Theme::light();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ControlAlignmentGroup::column()), None, 0);
    // Keep a real leading value part: editor coordinates must be offset, not row-local.
    let slider = tree.add_child(
        root,
        Box::new(
            Slider::new(0.0, 100.0)
                .with_value(42.0)
                .with_label("Alpha")
                .with_value_display()
                .with_value_field_side(ValueFieldSide::Left),
        ),
        None,
        0,
    );
    let tip = tree.add_child(root, Box::new(TooltipHost::new("Node")), None, 1);
    let caption = tree.add_child(tip, Box::new(ControlCaption::new("Target node")), None, 0);
    let changes = Rc::new(Cell::new(0));
    let submits = Rc::new(Cell::new(0));
    let cancels = Rc::new(Cell::new(0));
    let mut editor = TextField::new().with_text("node");
    let calls = Rc::clone(&changes);
    editor.set_on_change(move |_| calls.set(calls.get() + 1));
    let calls = Rc::clone(&submits);
    editor.set_on_submit(move |_| calls.set(calls.get() + 1));
    let calls = Rc::clone(&cancels);
    editor.set_on_cancel(move || calls.set(calls.get() + 1));
    let field = tree.add_child(caption, Box::new(editor), None, 0);
    let combo_row = tree.add_child(root, Box::new(ControlCaption::new("Target fleet")), None, 2);
    let combo = tree.add_child(
        combo_row,
        Box::new(ComboBox::simple(
            (0..20).map(|i| format!("Fleet {i}")).collect(),
        )),
        None,
        0,
    );
    for row in [slider, tip, combo_row] {
        tree.set_control_alignment_member(row, true);
    }
    let constraints = Constraints::new(480.0, 480.0, 0.0, f32::INFINITY);
    let mut text = FontManagerTextEngine::new();
    test_begin_frame_layout_tracking();
    {
        let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
        layout_tree(&mut tree, root, constraints, &theme, &mut text, None);
    }
    let field_rect = tree.get_absolute_paint_rect(field).unwrap();
    let combo_rect = tree.get_absolute_paint_rect(combo).unwrap();
    assert!(field_rect.x > 0.0);
    assert_eq!(
        (field_rect.x, field_rect.right()),
        (combo_rect.x, combo_rect.right())
    );
    let mut router = EventRouter::new();
    let mut domains = InteractionDomainState::new();
    let mut bindings = BindingRegistry::new();
    let mut send = |tree: &mut WidgetTree,
                    text: &mut FontManagerTextEngine,
                    domains: &mut InteractionDomainState,
                    event| {
        router.route_input_event(tree, event, Some(root), text, None, domains, &mut bindings);
        router.take_requested_cursor()
    };
    let move_to = |pos| FusedInputEvent::PointerMove {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        delta: (0.0, 0.0),
        device: PointerDevice::Mouse,
        buttons: PointerButtonsState::default(),
    };
    let down = |pos| FusedInputEvent::PointerDown {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
        click_count: 1,
    };
    let up = |pos| FusedInputEvent::PointerUp {
        window_id: WindowId::PRIMARY,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        button: PointerButton::Primary,
    };
    let point = Point::new(
        field_rect.x
            + theme.geometry.text_field.padding_x
            + text.measure("no", theme.typography.font_size).0,
        field_rect.y + field_rect.height * 0.5,
    );
    assert_eq!(
        send(&mut tree, &mut text, &mut domains, move_to(point)),
        Some(CursorIcon::Text)
    );
    for event in [down(point), up(point)] {
        send(&mut tree, &mut text, &mut domains, event);
    }
    let field_owner = tree.widget_id_for_node(field).unwrap();
    assert_eq!(domains.focus_manager.focused(), Some(field_owner));
    assert_eq!(domains.keyboard_mode, KeyboardAuthorityMode::Capture);
    let editor = tree
        .node(field)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<TextField>()
        .unwrap();
    assert_eq!(
        editor.cursor_pos(),
        2,
        "first click before Paint must use child-local coordinates"
    );
    assert_eq!((editor.selection().anchor, editor.selection().head), (2, 2));
    send(
        &mut tree,
        &mut text,
        &mut domains,
        FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: "X".into(),
            modifiers: modifiers(),
        },
    );
    assert_eq!(
        tree.node(field)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextField>()
            .unwrap()
            .text(),
        "noXde"
    );
    assert_eq!((changes.get(), submits.get(), cancels.get()), (1, 0, 0));
    let (cursor, selection) = {
        let editor = tree
            .node_mut(field)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<TextField>()
            .unwrap();
        editor.set_cursor_pos_for_test(editor.text().len());
        (
            (editor.cursor_ref().pos, editor.cursor_ref().preferred_x),
            (editor.selection().anchor, editor.selection().head),
        )
    };
    let reserved_width = constraints.max_width - field_rect.width;
    let padding_width = theme.geometry.text_field.padding_x * 2.0;
    let caret_width = theme.typography.caret_thickness;
    for (stage, width) in [
        constraints.max_width,
        reserved_width + padding_width + caret_width * 2.0,
        reserved_width + padding_width + caret_width * 0.5,
        reserved_width,
        constraints.max_width,
    ]
    .into_iter()
    .enumerate()
    {
        let mut previous_scroll = None;
        for _ in 0..2 {
            tree.mark_layout_dirty(field);
            test_begin_frame_layout_tracking();
            {
                let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
                layout_tree(
                    &mut tree,
                    root,
                    Constraints::new(width, width, 0.0, f32::INFINITY),
                    &theme,
                    &mut text,
                    None,
                );
            }
            let actual = tree.get_absolute_paint_rect(field).unwrap();
            let viewport = (actual.width - padding_width).max(0.0);
            let editor = tree
                .node_mut(field)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<TextField>()
                .unwrap();
            assert_eq!(editor.text(), "noXde");
            assert_eq!(
                (editor.cursor_ref().pos, editor.cursor_ref().preferred_x),
                cursor
            );
            assert_eq!(
                (editor.selection().anchor, editor.selection().head),
                selection
            );
            let scroll = editor.scroll_x_for_test();
            let cache = editor.layout_cache_mut();
            let target = cache.boundaries().last().unwrap().x_logical;
            let extent = cache.content_width();
            let caret = cache.caret_thickness();
            match stage {
                1 => assert!(
                    viewport >= caret
                        && viewport < theme.typography.caret_margin * 2.0 + caret
                        && viewport < extent
                ),
                2 => assert!(viewport > 0.0 && viewport < caret),
                3 => assert_eq!(actual.width, 0.0),
                _ => assert!(viewport >= extent),
            }
            assert!(scroll.is_finite());
            assert!(scroll >= 0.0 && scroll <= (extent - viewport).max(0.0));
            if viewport >= caret {
                assert!(target >= scroll && target + caret <= scroll + viewport);
            }
            if let Some(previous) = previous_scroll {
                assert_eq!(scroll, previous, "unchanged Layout must be idempotent");
            }
            previous_scroll = Some(scroll);
            assert_eq!(tree.widget_id_for_node(field), Some(field_owner));
            assert_eq!(domains.focus_manager.focused(), Some(field_owner));
            assert_eq!(domains.keyboard_mode, KeyboardAuthorityMode::Capture);
            assert_eq!((changes.get(), submits.get(), cancels.get()), (1, 0, 0));
        }
    }
    assert_eq!(tree.get_absolute_paint_rect(field).unwrap(), field_rect);
    let mut harness = RetainedSceneTestHarness::new();
    let icons = IconRegistry::new(&[]);
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let mut geometry = Geometry::default();
    collect_geometry(
        &harness
            .local_fragment(tree.widget_id_for_node(slider).unwrap())
            .unwrap(),
        &mut geometry,
        TrackDisplayStyle::Track,
        tree.get_absolute_paint_rect(slider).unwrap(),
        theme.colors.surfaces.content.into(),
        &mut text,
    );
    let track = geometry.body.unwrap();
    // Compare visible edges after the existing paint projection, not logical floats to snapped pixels.
    assert_eq!(
        (track.x, track.right()),
        (
            crate::core::math::snap_to_physical(field_rect.x, 1.0),
            crate::core::math::snap_to_physical(field_rect.right(), 1.0)
        ),
    );
    assert_eq!(
        tree.get_absolute_paint_rect(field).unwrap(),
        field_rect,
        "editing must not resize the shared body"
    );
    let caption_point = Point::new(
        field_rect.right() + theme.geometry.slider.label_gap,
        point.y,
    );
    assert_ne!(
        tree.hit_test(caption_point.x, caption_point.y)
            .map(|h| h.widget_id),
        Some(field_owner)
    );
    assert_ne!(
        send(&mut tree, &mut text, &mut domains, move_to(caption_point)),
        Some(CursorIcon::Text)
    );

    let combo_point = Point::new(
        combo_rect.x + theme.geometry.text_field.padding_x,
        combo_rect.y + combo_rect.height * 0.5,
    );
    assert_eq!(
        send(&mut tree, &mut text, &mut domains, move_to(combo_point)),
        Some(CursorIcon::Text)
    );
    for event in [down(combo_point), up(combo_point)] {
        send(&mut tree, &mut text, &mut domains, event);
    }
    let combo_owner = tree.widget_id_for_node(combo).unwrap();
    assert_eq!(domains.focus_manager.focused(), Some(combo_owner));
    send(
        &mut tree,
        &mut text,
        &mut domains,
        FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: "F".into(),
            modifiers: modifiers(),
        },
    );
    assert_eq!(
        tree.node(combo)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap()
            .input_text(),
        "F"
    );
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert_eq!(tree.get_absolute_paint_rect(combo).unwrap(), combo_rect);
    let popup = ComboBox::test_popup_local_rect(&tree, combo);
    let popup_point = Point::new(
        combo_rect.x + popup.x + popup.width * 0.5,
        combo_rect.y + popup.y + popup.height * 0.25,
    );
    assert_eq!(
        tree.hit_test(popup_point.x, popup_point.y)
            .unwrap()
            .widget_id,
        combo_owner
    );
    send(&mut tree, &mut text, &mut domains, move_to(popup_point));
    let before_scroll = tree
        .node(combo)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ComboBox>()
        .unwrap()
        .test_highlighted_label()
        .unwrap()
        .to_owned();
    send(
        &mut tree,
        &mut text,
        &mut domains,
        FusedInputEvent::Scroll {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: popup_point,
            delta: (0.0, -theme.typography.line_height),
            source: ScrollSource::Wheel,
        },
    );
    send(&mut tree, &mut text, &mut domains, move_to(popup_point));
    let selected = tree
        .node(combo)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<ComboBox>()
        .unwrap()
        .test_highlighted_label()
        .unwrap()
        .to_owned();
    assert_ne!(
        selected, before_scroll,
        "wheel must reach the actual popup owner"
    );
    for event in [down(popup_point), up(popup_point)] {
        send(&mut tree, &mut text, &mut domains, event);
    }
    assert_eq!(
        tree.node(combo)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap()
            .committed_value_string(),
        Some(selected)
    );
    assert_eq!(
        tree.node(slider)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .value(),
        42.0
    );
}

// Explicit composition contract: named assignments must reach real numeric entry
// before Paint. Existing proofs exercise the default order, not these two opt-ins.
#[test]
fn slider_alignment_explicit_compositions_preserve_numeric_entry() {
    use crate::core::app::{RetainedSceneTestHarness, layout_tree};
    use crate::core::layout::ControlTrailingOrder;
    use crate::widgets::{ControlAlignmentGroup, slider::SliderLabelSide};
    let theme = Theme::light();
    for (order, side) in [
        (ControlTrailingOrder::ValueThenLabel, SliderLabelSide::Right),
        (ControlTrailingOrder::LabelThenValue, SliderLabelSide::Left),
    ] {
        let changes = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(ControlAlignmentGroup::column().with_trailing_order(order)),
            None,
            0,
        );
        let mut rows = Vec::new();
        for (label, value) in [("Alpha", 42.0), ("Longer caption", 75.0)] {
            let count = changes.clone();
            let node = tree.add_child(
                root,
                Box::new(
                    Slider::new(0.0, 100.0)
                        .with_value(value)
                        .with_label(label)
                        .with_label_side(side)
                        .with_value_display()
                        .on_value_changed(move |_| count.set(count.get() + 1)),
                ),
                None,
                0,
            );
            tree.set_control_alignment_member(node, true);
            rows.push(node);
        }
        let mut fixture = Fixture {
            tree,
            root,
            control: rows[0],
            text_engine: FontManagerTextEngine::new(),
        };
        let mut harness = RetainedSceneTestHarness::new();
        let initial = Constraints::new(480.0, 480.0, 0.0, f32::INFINITY);
        harness.frame(
            &mut fixture.tree,
            root,
            initial,
            &theme,
            &mut fixture.text_engine,
            &IconRegistry::new(&[]),
        );
        let mut parts = Vec::new();
        for &row in &rows {
            let mut geometry = Geometry::default();
            collect_geometry(
                &harness
                    .local_fragment(fixture.tree.widget_id_for_node(row).unwrap())
                    .unwrap(),
                &mut geometry,
                TrackDisplayStyle::Track,
                fixture.tree.get_absolute_paint_rect(row).unwrap(),
                theme.colors.surfaces.content.into(),
                &mut fixture.text_engine,
            );
            parts.push(geometry);
        }
        let body = parts[0].body.unwrap();
        let field = parts[0].field.unwrap();
        let label = parts[0].label.unwrap();
        assert_eq!(
            (body.x, body.right()),
            (parts[1].body.unwrap().x, parts[1].body.unwrap().right())
        );
        assert_eq!(
            (field.x, field.right()),
            (parts[1].field.unwrap().x, parts[1].field.unwrap().right())
        );
        match side {
            SliderLabelSide::Right => assert!(body.right() <= field.x && field.right() <= label.x),
            SliderLabelSide::Left => assert!(label.right() <= body.x && body.right() <= field.x),
        }
        assert_eq!(changes.get(), 0);
        // Side parts keep natural widths; an allocation change moves the trailing editor.
        let wider = Constraints::new(560.0, 560.0, 0.0, f32::INFINITY);
        fixture.tree.mark_layout_dirty(root);
        test_begin_frame_layout_tracking();
        {
            let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
            layout_tree(
                &mut fixture.tree,
                root,
                wider,
                &theme,
                &mut fixture.text_engine,
                None,
            );
        }
        let x = field.x + wider.min_width - initial.min_width + field.width * 0.9;
        let y = field.y + field.height * 0.5;
        let entry = fixture.pointer(PointerEventKind::Down, x, y, false);
        assert!(entry.handled && entry.keyboard);
        assert_eq!(
            entry.pointer, None,
            "numeric entry must not start a track drag"
        );
        assert_eq!(fixture.value(), 42.0);
        assert_eq!(changes.get(), 0);
        fixture.event(
            Event::Text(TextInputEvent {
                text: "63.0".into(),
                modifiers: modifiers(),
            }),
            false,
            KeyboardAuthorityMode::Capture,
        );
        // A second click places a caret at the end instead of reselecting the number.
        fixture.pointer(PointerEventKind::Down, x, y, false);
        fixture.event(
            Event::Text(TextInputEvent {
                text: "0".into(),
                modifiers: modifiers(),
            }),
            false,
            KeyboardAuthorityMode::Capture,
        );
        fixture.event(
            Event::Key(KeyEvent {
                key_code: KeyCode::Enter,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                is_down: true,
                is_repeat: false,
                modifiers: modifiers(),
                text: None,
            }),
            false,
            KeyboardAuthorityMode::Capture,
        );
        assert_eq!(
            fixture.value(),
            63.0,
            "first entry selects all and the next click preserves caret editing"
        );
        assert_eq!(changes.get(), 1);
        fixture.control = rows[1];
        assert_eq!(
            fixture.value(),
            75.0,
            "independent peer value is not synchronized"
        );
    }
}
