use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::animation::Timeline;
use crate::core::event::{
    Event, KeyboardAuthorityMode, Modifiers, PointerButton, PointerEvent, PointerEventKind,
    PointerId, PointerType,
};
use crate::core::icons::builtin_registry;
use crate::core::layout::Constraints;
use crate::core::math::Size;
use crate::core::paint::{Color, PaintSink, TextBackgroundSpan};
use crate::core::rect_authority::{enter_paint_authority, exit_paint_authority};
use crate::core::scene::{SceneBuilder, SceneNode};
use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget};
use crate::core::widget_tree::{OverlayHitRegion, WidgetTree};
use crate::model::toast::ToastManager;
use crate::utils::id::TreeWidgetId;
use crate::widgets::toast::ToastWidget;

struct ToastTestTextEngine;

impl TextEngine for ToastTestTextEngine {
    fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        (text.len() as f32 * font_size * 0.5, font_size + 4.0)
    }

    fn metrics(&mut self, _text: &str, font_size: f32) -> TextMetrics {
        self.font_metrics(font_size)
    }

    fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun {
        GlyphRun {
            binding: None,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            width: text.len() as f32 * font_size * 0.5,
            height: font_size + 4.0,
            text: text.to_string(),
            font_size,
            ascent: font_size,
            descent: 4.0,
        }
    }

    fn draw(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _z_index: crate::core::paint::ZIndex,
        _sink: &mut dyn PaintSink,
    ) {
    }

    fn draw_with_backgrounds(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _backgrounds: Vec<TextBackgroundSpan>,
        _z_index: crate::core::paint::ZIndex,
        _sink: &mut dyn PaintSink,
    ) {
    }

    fn font_metrics(&mut self, font_size: f32) -> TextMetrics {
        TextMetrics {
            ascent: font_size,
            descent: 4.0,
            line_gap: 0.0,
            line_height: font_size + 4.0,
        }
    }
}

#[test]
fn hidden_toasts_continue_lifecycle_after_capacity_eviction() {
    let mut manager = ToastManager::new();

    for i in 0..5 {
        manager.push(format!("first {i}"), 100, None);
    }
    manager.advance_lifecycle(0, false, 0, 10);

    for i in 0..5 {
        manager.push(format!("second {i}"), 100, None);
    }

    let stats = manager.stats();
    assert_eq!(stats.total_pushed, 10);
    assert_eq!(stats.visible + stats.pending, 5);
    assert_eq!(stats.exiting, 5);
    assert_eq!(stats.capacity_evictions, 5);
    assert_eq!(stats.render_tracked, 5);

    manager.advance_lifecycle(1_000, false, 0, 10);
    manager.advance_lifecycle(1_010, false, 0, 10);
    manager.flush_dismissed();

    let stats = manager.stats();
    assert_eq!(stats.queue_len, 5);
    assert_eq!(stats.active, 5);
    assert_eq!(stats.visible, 5);
    assert_eq!(stats.flushed_dismissed, 5);

    manager.advance_lifecycle(1_100, false, 0, 10);
    manager.advance_lifecycle(1_110, false, 0, 10);
    manager.flush_dismissed();

    let stats = manager.stats();
    assert_eq!(stats.queue_len, 0);
    assert_eq!(stats.active, 0);
    assert_eq!(stats.flushed_dismissed, 10);
}

#[test]
fn repeated_bursts_always_admit_new_visible_entries() {
    let mut manager = ToastManager::new();

    for burst in 0..3 {
        for i in 0..5 {
            manager.push(format!("burst {burst}.{i}"), 100, None);
        }

        let stats = manager.stats();
        assert_eq!(stats.pending + stats.visible, 5);
        assert_eq!(stats.render_tracked, 5);

        let start = burst * 1_000;
        manager.advance_lifecycle(start, false, 0, 10);
        manager.advance_lifecycle(start + 100, false, 0, 10);
        manager.advance_lifecycle(start + 110, false, 0, 10);
        manager.flush_dismissed();
    }

    let stats = manager.stats();
    assert_eq!(stats.total_pushed, 15);
    assert_eq!(stats.queue_len, 0);
    assert_eq!(stats.active, 0);
}

#[test]
fn toast_paint_uses_theme_surface_without_dismiss_affordance() {
    let manager = Rc::new(RefCell::new(ToastManager::new()));
    manager.borrow_mut().push("first", 10_000, None);
    manager.borrow_mut().push("second", 10_000, None);
    manager.borrow().advance_lifecycle(0, false, 0, 180);

    let manager_for_assert = Rc::clone(&manager);
    let mut widget = ToastWidget::new(manager);
    let theme = Theme::light();
    let mut tree = WidgetTree::new();
    let mut text_engine = ToastTestTextEngine;

    {
        let mut layout_cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        widget.layout(
            &mut layout_cx,
            Constraints::tight(Size::new(800.0, 600.0)),
            &[],
        );
    }

    let move_event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (400.0, 540.0),
        local_pos: (400.0, 540.0),
        kind: PointerEventKind::Move,
        button: None,
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    let mut event_cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    widget.handle_event(
        &mut event_cx,
        &move_event,
        crate::core::event::EventPhase::Target,
    );

    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects = HashMap::new();
    let mut overlay_hit_regions: Vec<OverlayHitRegion> = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let mut scene = SceneBuilder::new();

    {
        let mut paint_cx = PaintContext::new(
            &tree,
            &mut scene,
            &theme,
            &mut text_engine,
            &icon_registry,
            300,
            &timeline,
            &interpolated_rects,
            &mut overlay_hit_regions,
            &mut tooltip_triggers,
            1.0,
            None,
            KeyboardAuthorityMode::Navigation,
        );

        enter_paint_authority();
        widget.paint(&mut paint_cx, crate::core::math::RectF::zero(), &[]);
        widget.paint_overlay(
            &mut paint_cx,
            crate::core::math::RectF::zero(),
            TreeWidgetId::from_raw(1),
        );
        exit_paint_authority();
    }

    let graph = scene.build();
    let expected_bg: crate::core::scene::Color = theme.colors.surfaces.card.with_alpha(1.0).into();
    let has_light_theme_toast_bg = graph.root.iter().any(|node| match node {
        SceneNode::Text(text) => text
            .backgrounds
            .iter()
            .any(|bg| bg.width >= 200.0 && bg.color == expected_bg),
        _ => false,
    });
    assert!(has_light_theme_toast_bg);

    let dismiss_affordance_lines: Vec<_> = graph
        .root
        .iter()
        .filter_map(|node| match node {
            SceneNode::Line(line) => Some(line),
            _ => None,
        })
        .collect();
    assert!(dismiss_affordance_lines.is_empty());

    let has_text_fallback_close = graph
        .root
        .iter()
        .any(|node| matches!(node, SceneNode::Text(text) if text.text == "x"));
    assert!(!has_text_fallback_close);

    let has_hover_square_background = graph.root.iter().any(|node| match node {
        SceneNode::Text(text) => text.backgrounds.iter().any(|bg| {
            bg.width >= 16.0 && bg.width <= 24.0 && bg.height >= 16.0 && bg.height <= 24.0
        }),
        _ => false,
    });
    assert!(!has_hover_square_background);

    let hit_region = overlay_hit_regions
        .first()
        .expect("toast body hit region must be registered");
    let click_x = hit_region.abs_rect.x + hit_region.abs_rect.width * 0.5;
    let click_y = hit_region.abs_rect.y + hit_region.abs_rect.height * 0.5;
    let down_event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        global_pos: (click_x, click_y),
        local_pos: (click_x, click_y),
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        pointer_type: PointerType::Mouse,
    });
    let mut click_cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        std::ptr::null_mut(),
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    assert!(widget.handle_event(
        &mut click_cx,
        &down_event,
        crate::core::event::EventPhase::Target,
    ));
    let stats = manager_for_assert.borrow().stats();
    assert_eq!(stats.visible, 1);
    assert_eq!(stats.exiting, 1);
}
