use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind, PointerId, PointerType,
};
use crate::core::layout::Constraints;
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, UpdateContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::plot::{DataUpdate, MarketDetailLink, Plot, RenderCandle, ViewState};
use crate::utils::id::TreeWidgetId;
use crate::widgets::AlignedOrderBook;

fn candle(x_open: f64, x_close: f64) -> RenderCandle {
    RenderCandle {
        x_open,
        x_close,
        high: 101.0,
        low: 99.0,
        body_top: 100.5,
        body_bottom: 99.5,
        is_up: true,
        volume: 1.0,
        body_color_override: None,
    }
}

fn candles(count: usize) -> Vec<RenderCandle> {
    (0..count)
        .map(|index| candle(index as f64, index as f64 + 1.0))
        .collect()
}

fn layout_plot(plot: &mut Plot) {
    let theme = Theme::dark();
    let mut layout_tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut layout_cx = LayoutContext::new(&mut layout_tree, &mut text_engine, &theme);
    plot.layout(
        &mut layout_cx,
        Constraints::tight(crate::core::math::Size::new(800.0, 500.0)),
        &[],
    );
}

fn handle_pointer(
    plot: &mut Plot,
    kind: PointerEventKind,
    button: Option<PointerButton>,
    local_pos: (f32, f32),
) -> bool {
    let mut event_tree = WidgetTree::new();
    let mut event_cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        &mut event_tree as *mut WidgetTree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    plot.handle_event(
        &mut event_cx,
        &Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: local_pos,
            local_pos,
            kind,
            button,
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        }),
        EventPhase::Target,
    )
}

fn handle_key(plot: &mut Plot, key_code: KeyCode, is_down: bool) -> bool {
    let mut event_tree = WidgetTree::new();
    let mut event_cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        &mut event_tree as *mut WidgetTree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    plot.handle_event(
        &mut event_cx,
        &Event::Key(KeyEvent {
            key_code,

            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: None,
        }),
        EventPhase::Target,
    )
}

#[test]
fn live_channel_defaults_to_follow_latest_and_tracks_the_latest_candle() {
    let view = ViewState::new(0.0, 100.0, 90.0, 110.0);
    let (mut plot, data_tx) = Plot::new()
        .with_view_state(view)
        .with_candles(candles(100))
        .with_data_channel();
    assert!(
        plot.view_state().follow_latest,
        "attaching a live channel must arm follow mode by default"
    );

    data_tx
        .send(DataUpdate::AppendCandle {
            series_index: 0,
            candle: candle(100.0, 101.0),
        })
        .unwrap();
    let theme = Theme::dark();
    let mut update_cx = UpdateContext::with_theme(None, &theme);
    plot.update(0.0, &mut update_cx);

    assert_eq!(plot.view_state().x_span(), 100.0);
    assert_eq!(plot.view_state().x_min, 6.0);
    assert_eq!(plot.view_state().x_max, 106.0);
}

#[test]
fn explicit_initial_follow_override_is_independent_of_builder_order() {
    let view = ViewState::new(0.0, 100.0, 90.0, 110.0);
    let (before_channel, _) = Plot::new()
        .with_initial_follow_latest(false)
        .with_view_state(view.clone())
        .with_data_channel();
    assert!(!before_channel.view_state().follow_latest);

    let (after_channel, _) = Plot::new()
        .with_initial_follow_latest(false)
        .with_data_channel();
    let after_view = after_channel.with_view_state(view.clone());
    assert!(!after_view.view_state().follow_latest);

    let (channel_first, _) = Plot::new().with_data_channel();
    let explicit_last = channel_first
        .with_view_state(view)
        .with_initial_follow_latest(false);
    assert!(!explicit_last.view_state().follow_latest);
}

#[test]
fn manual_time_pan_suspends_follow_and_live_edge_or_reset_rearms_it() {
    let view = ViewState::new(0.0, 100.0, 90.0, 110.0);
    let (mut plot, data_tx) = Plot::new()
        .with_view_state(view)
        .with_candles(candles(100))
        .with_data_channel();
    layout_plot(&mut plot);

    plot.pan_drag_for_test(80.0, 0.0, 800.0, 500.0, false);
    assert!(!plot.view_state().follow_latest);
    let manual_x = (plot.view_state().x_min, plot.view_state().x_max);

    data_tx
        .send(DataUpdate::AppendCandle {
            series_index: 0,
            candle: candle(100.0, 101.0),
        })
        .unwrap();
    let theme = Theme::dark();
    let mut update_cx = UpdateContext::with_theme(None, &theme);
    plot.update(0.0, &mut update_cx);
    assert_eq!(
        (plot.view_state().x_min, plot.view_state().x_max),
        manual_x,
        "live data must not snap a manually positioned time viewport"
    );

    plot.pan_drag_for_test(-128.0, 0.0, 800.0, 500.0, false);
    assert!(plot.finish_horizontal_pan_for_test());
    assert!(plot.view_state().follow_latest);
    assert_eq!(plot.view_state().x_min, 6.0);
    assert_eq!(plot.view_state().x_max, 106.0);

    data_tx
        .send(DataUpdate::AppendCandle {
            series_index: 0,
            candle: candle(101.0, 102.0),
        })
        .unwrap();
    plot.update(0.0, &mut update_cx);
    assert_eq!(plot.view_state().x_min, 7.0);
    assert_eq!(plot.view_state().x_max, 107.0);

    plot.pan_drag_for_test(80.0, 0.0, 800.0, 500.0, false);
    assert!(!plot.finish_horizontal_pan_for_test());
    assert!(!plot.view_state().follow_latest);
    assert!(handle_pointer(
        &mut plot,
        PointerEventKind::DoubleClick,
        None,
        (400.0, 250.0),
    ));
    assert!(plot.view_state().follow_latest);
    assert_eq!(plot.view_state().x_min, 7.0);
    assert_eq!(plot.view_state().x_max, 107.0);
}

#[test]
fn x_box_zoom_suspends_follow_but_aligned_price_pan_does_not() {
    let (mut plot, _) = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 90.0, 110.0))
        .with_data_channel();
    layout_plot(&mut plot);

    assert!(handle_pointer(
        &mut plot,
        PointerEventKind::Down,
        Some(PointerButton::Secondary),
        (100.0, 100.0),
    ));
    assert!(handle_pointer(
        &mut plot,
        PointerEventKind::Move,
        None,
        (700.0, 400.0),
    ));
    assert!(handle_pointer(
        &mut plot,
        PointerEventKind::Up,
        Some(PointerButton::Secondary),
        (700.0, 400.0),
    ));
    assert!(!plot.view_state().follow_latest);

    let link = MarketDetailLink::new(90.0, 110.0);
    let mut view = ViewState::new(0.0, 100.0, 90.0, 110.0);
    view.follow_latest = true;
    let mut plot = Plot::new()
        .with_view_state(view)
        .with_market_detail_link(link.clone());
    let mut ladder = AlignedOrderBook::new(link);
    ladder.project_viewport_for_test(200.0, 100.0);
    assert!(ladder.queue_price_pan_for_test(10.0));
    assert!(plot.apply_market_detail_navigation_for_test());
    assert!(
        plot.view_state().follow_latest,
        "Y-only aligned-book navigation must not suspend X auto-follow"
    );
}

#[test]
fn discrete_space_scroll_pan_suspends_follow_without_same_tick_rearm() {
    let (mut plot, _) = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 90.0, 110.0))
        .with_candles(candles(100))
        .with_data_channel();
    layout_plot(&mut plot);

    assert!(!handle_key(&mut plot, KeyCode::Space, true));
    assert!(handle_pointer(
        &mut plot,
        PointerEventKind::Scroll {
            delta_x: 0.0,
            delta_y: 1.0,
        },
        None,
        (400.0, 250.0),
    ));
    assert!(!plot.view_state().follow_latest);
}

#[test]
fn live_channel_polling_invalidates_paint_only_when_a_visual_update_is_consumed() {
    let (mut plot, data_tx) = Plot::new()
        .with_candles(vec![candle(0.0, 1.0)])
        .with_data_channel();
    let theme = Theme::dark();
    let mut update_cx = UpdateContext::with_theme(None, &theme);

    assert!(plot.needs_update());
    assert!(
        !plot.needs_animation(),
        "a live receiver is update work, not a continuously changing visual"
    );

    update_cx.begin_widget_update();
    plot.update(1.0 / 120.0, &mut update_cx);
    assert_eq!(update_cx.take_widget_invalidations(), (false, false, false));

    for index in 0..440 {
        data_tx
            .send(DataUpdate::UpdateLastCandle {
                series_index: 0,
                candle: candle(index as f64, index as f64 + 1.0),
            })
            .unwrap();
    }
    update_cx.begin_widget_update();
    plot.update(1.0 / 120.0, &mut update_cx);
    assert_eq!(
        update_cx.take_widget_invalidations(),
        (true, false, false),
        "one engine update coalesces the burst into paint-only invalidation"
    );

    update_cx.begin_widget_update();
    plot.update(1.0 / 120.0, &mut update_cx);
    assert_eq!(update_cx.take_widget_invalidations(), (false, false, false));
}

#[test]
fn pointer_scroll_zoom_suspends_live_channel_auto_follow_until_reset() {
    let mut view = ViewState::new(0.0, 100.0, 90.0, 110.0);
    view.follow_latest = true;
    let (mut plot, data_tx) = Plot::new()
        .with_view_state(view)
        .with_candles(vec![candle(0.0, 1.0)])
        .with_data_channel();

    let theme = Theme::dark();
    let mut layout_tree = WidgetTree::new();
    let mut text_engine = FontManagerTextEngine::new();
    let mut layout_cx = LayoutContext::new(&mut layout_tree, &mut text_engine, &theme);
    plot.layout(
        &mut layout_cx,
        Constraints::tight(crate::core::math::Size::new(800.0, 500.0)),
        &[],
    );

    let mut event_tree = WidgetTree::new();
    let mut event_cx = EventContext::new(
        TreeWidgetId::from_raw(1),
        &mut event_tree as *mut WidgetTree,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false,
        KeyboardAuthorityMode::Navigation,
    );
    let handled = plot.handle_event(
        &mut event_cx,
        &Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (400.0, 250.0),
            local_pos: (400.0, 250.0),
            kind: PointerEventKind::Scroll {
                delta_x: 0.0,
                delta_y: 1.0,
            },
            button: None,
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        }),
        EventPhase::Target,
    );

    assert!(handled);
    assert!(!plot.view_state().follow_latest);
    let manual_x = (plot.view_state().x_min, plot.view_state().x_max);

    data_tx
        .send(DataUpdate::AppendCandle {
            series_index: 0,
            candle: candle(150.0, 151.0),
        })
        .unwrap();
    let mut update_cx = UpdateContext::with_theme(None, &theme);
    plot.update(0.0, &mut update_cx);

    assert_eq!(
        (plot.view_state().x_min, plot.view_state().x_max),
        manual_x,
        "a live append must not re-enable or apply auto-follow after manual scroll zoom"
    );
}
