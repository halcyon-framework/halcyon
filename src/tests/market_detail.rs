use crate::core::app::RetainedSceneTestHarness;
use crate::core::icon::IconRegistry;
use crate::core::layout::Constraints;
use crate::core::math::Size;
use crate::core::paint::Color;
use crate::core::scene::SceneNode;
use crate::core::theme::Theme;
use crate::core::widget::{UpdateContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::model::order_book::{BookLevel, BookSide, BookSideKind, LevelClassification};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::plot::grid::format_value_for_interval;
use crate::plot::market_detail::MarketDetailReferenceLine;
use crate::plot::render_pool::{CompletedRenderPublication, render_job_is_coherent_for_test};
use crate::plot::{
    AxisAuthority, AxisKind, MarketDetailConfig, MarketDetailGrouping, MarketDetailLink,
    MarketDetailPriceStep, MarketDetailProjector, Plot, PlotColorScheme, PriceLevel,
    PriceLevelLine, PriceTransform, ProjectedBookLevel, ProjectedPriceRange, Transform, ViewState,
    VisibleDepthScale,
};
use crate::widgets::order_book::OrderBookConfig;
use crate::widgets::order_book::OrderBookUpdate;
use crate::widgets::{AlignedOrderBook, AlignedOrderBookRowLayout, AlignedOrderBookSelection};
use std::sync::Arc;

fn level(price: f64, quantity: f64, metadata: &str) -> BookLevel {
    BookLevel {
        price,
        quantity,
        cumulative: 0.0,
        order_count: None,
        classification: LevelClassification::Normal,
        metadata: vec![metadata.to_owned()],
    }
}

fn dense_book(mid: f64, count: usize, tick: f64) -> (BookSide, BookSide) {
    let bids = BookSide {
        levels: (0..count)
            .map(|index| {
                level(
                    mid - (index + 1) as f64 * tick,
                    1.0 + (index % 7) as f64,
                    &format!("bid-{index}"),
                )
            })
            .collect(),
    };
    let asks = BookSide {
        levels: (0..count)
            .map(|index| {
                level(
                    mid + (index + 1) as f64 * tick,
                    1.0 + (index % 11) as f64,
                    &format!("ask-{index}"),
                )
            })
            .collect(),
    };
    (bids, asks)
}

#[test]
fn plot_and_market_detail_use_the_same_price_transform() {
    let view = ViewState::new(10.0, 20.0, 99.0, 101.0);
    let plot = Transform::from_view_state_with_axes(
        &view,
        800.0,
        420.0,
        AxisKind::Linear,
        AxisKind::Linear,
    );
    let detail = PriceTransform::new(99.0, 101.0, AxisKind::Linear, 420.0);

    for price in [99.0, 99.25, 100.0, 100.75, 101.0] {
        assert_eq!(plot.y_to_px_checked(price), detail.price_to_px(price));
    }
}

#[test]
fn default_axis_formatting_keeps_large_fractional_ticks_distinct() {
    assert_eq!(format_value_for_interval(59_968.2, 0.2), "59968.2");
    assert_eq!(format_value_for_interval(59_968.4, 0.2), "59968.4");
    assert_eq!(format_value_for_interval(100_000.0, 20.0), "100000");
}

#[test]
fn dense_projection_is_pixel_bounded_and_retains_exact_hover_source() {
    let (bids, asks) = dense_book(100.0, 1_000, 0.001);
    let mut projector = MarketDetailProjector::default();
    projector.replace_book(bids, asks);
    let config = MarketDetailConfig {
        min_bucket_height: 10.0,
        ..MarketDetailConfig::default()
    };
    let projection = projector.project(
        PriceTransform::new(99.9, 100.1, AxisKind::Linear, 100.0),
        config,
    );

    assert!(projection.levels.len() <= 22);
    assert!(projection.above.level_count > 0);
    assert!(projection.below.level_count > 0);
    assert!(
        projection
            .levels
            .iter()
            .any(|level| level.source_level_count > 1)
    );

    let exact = projector
        .nearest_exact(BookSideKind::Ask, 100.0374, 99.9, 100.1)
        .expect("visible ask source level");
    assert_eq!(exact.price, 100.037);
    assert_eq!(exact.metadata, vec!["ask-36"]);
}

#[test]
fn grouped_price_ranges_keep_stable_identity_across_quantity_churn() {
    let transform = PriceTransform::new(99.0, 101.0, AxisKind::Linear, 200.0);
    let config = MarketDetailConfig {
        min_bucket_height: 20.0,
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_bids(BookSide {
        levels: vec![level(99.21, 1.0, "low"), level(99.35, 9.0, "high")],
    });
    let before = projector.project(transform, config);

    projector.replace_bids(BookSide {
        levels: vec![level(99.21, 9.0, "low"), level(99.35, 1.0, "high")],
    });
    let after = projector.project(transform, config);

    let before_row = before
        .levels
        .iter()
        .find(|row| row.side == BookSideKind::Bid)
        .expect("initial stable price-range row");
    let after_row = after
        .levels
        .iter()
        .find(|row| row.side == BookSideKind::Bid)
        .expect("updated stable price-range row");
    assert_eq!(before_row.price_range, after_row.price_range);
    assert_eq!(before_row.price, after_row.price);
    assert_eq!(before_row.y, after_row.y);
    assert_eq!(before_row.height, after_row.height);
    assert!((before_row.price_range.min - 99.2).abs() < 1e-12);
    assert!((before_row.price_range.max - 99.4).abs() < 1e-12);
    assert!((before_row.price - 99.3).abs() < 1e-12);
    assert_eq!(before_row.quantity, 10.0);
    assert_eq!(after_row.quantity, 10.0);
    assert_eq!(before_row.source_level_count, 2);
    assert_eq!(after_row.source_level_count, 2);

    let anchor_y = transform
        .price_to_px(after_row.price)
        .expect("stable row anchor");
    assert!((anchor_y - (after_row.y + after_row.height * 0.5)).abs() < 1e-4);
}

#[test]
fn globally_anchored_automatic_price_ranges_survive_equal_span_pan() {
    let config = MarketDetailConfig {
        min_bucket_height: 14.0,
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_asks(BookSide {
        levels: vec![level(100.22, 4.0, "ask")],
    });
    let first_transform = PriceTransform::new(99.03, 100.97, AxisKind::Linear, 280.0);
    let second_transform = PriceTransform::new(99.08, 101.02, AxisKind::Linear, 280.0);
    let first = projector.project(first_transform, config);
    let second = projector.project(second_transform, config);
    let first_row = first.levels.first().expect("first anchored row");
    let second_row = second.levels.first().expect("second anchored row");

    assert!((first_row.price_range.min - second_row.price_range.min).abs() < 1e-12);
    assert!((first_row.price_range.max - second_row.price_range.max).abs() < 1e-12);
    assert!((first_row.price - second_row.price).abs() < 1e-12);
    assert_ne!(first_row.y, second_row.y);
    let expected_interval = (100.97 - 99.03) * 14.0 / 280.0;
    assert!(
        (first_row.price_range.max - first_row.price_range.min - expected_interval).abs() < 1e-12
    );
    assert!(
        (first_transform
            .price_to_px(first_row.price)
            .expect("first row anchor")
            - (first_row.y + first_row.height * 0.5))
            .abs()
            < 1e-4
    );
    assert!(
        (second_transform
            .price_to_px(second_row.price)
            .expect("second row anchor")
            - (second_row.y + second_row.height * 0.5))
            .abs()
            < 1e-4
    );
}

#[test]
fn default_grouping_uses_the_exact_label_bearing_collision_floor() {
    let transform = PriceTransform::new(59_085.0, 59_120.0, AxisKind::Linear, 950.0);
    let config = MarketDetailConfig::default();
    let mut projector = MarketDetailProjector::default();
    projector.replace_asks(BookSide {
        levels: vec![level(59_100.1, 2.0, "ask")],
    });

    let projection = projector.project(transform, config);
    let row = projection.levels.first().expect("visible grouped row");
    let interval = row.price_range.max - row.price_range.min;

    assert_eq!(config.grouping, MarketDetailGrouping::AutoPixel);
    assert_eq!(config.min_bucket_height, 10.0);
    assert!((interval - 350.0 / 950.0).abs() < 1e-8);
    assert!((row.height - 10.0).abs() < 1e-3);
}

#[test]
fn readable_anchored_layout_decouples_clickable_rows_from_price_collision_grouping() {
    let link = MarketDetailLink::new(90.0, 110.0);
    let (bids, asks) = dense_book(100.0, 40, 0.01);
    let mut strict = AlignedOrderBook::new(link.clone()).with_book(bids.clone(), asks.clone());
    strict.project_viewport_for_test(300.0, 400.0);
    let strict_count = strict.presented_projection_for_test().levels.len();

    let mut readable = AlignedOrderBook::new(link)
        .with_row_layout(AlignedOrderBookRowLayout::ReadableAnchored)
        .with_book(bids, asks);
    readable.project_viewport_for_test(300.0, 400.0);
    let projection = readable.presented_projection_for_test();
    let rows = readable.presented_rows_for_test();

    assert_eq!(
        strict.row_layout_for_test(),
        AlignedOrderBookRowLayout::PriceAligned
    );
    assert!(projection.levels.len() > strict_count);
    assert!(!rows.is_empty());
    assert!(rows.len() <= ((400.0 - 2.0 * 18.0) / 19.0) as usize);
    for pair in rows.windows(2) {
        assert!(pair[0].1 + pair[0].2 <= pair[1].1 + 1e-4);
    }
    assert!(
        rows.iter()
            .all(|(_, _, height, _)| (*height - 19.0).abs() < 1e-4)
    );
    assert!(
        rows.iter()
            .any(|(level, y, _, _)| (level.y - *y).abs() > 1.0)
    );

    let transform = PriceTransform::new(90.0, 110.0, AxisKind::Linear, 400.0);
    for (level, _, _, anchor_y) in &rows {
        assert!(
            (transform
                .price_to_px(level.price)
                .expect("projected price anchor")
                - anchor_y)
                .abs()
                < 1e-3
        );
    }

    let displayed_source_count: usize = rows
        .iter()
        .map(|(level, _, _, _)| level.source_level_count)
        .sum();
    let (above, below) = readable.presented_summaries_for_test();
    assert_eq!(
        displayed_source_count + above.level_count + below.level_count,
        80
    );
}

#[test]
fn readable_anchored_rows_keep_exact_source_hover_and_selection() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let (bids, asks) = dense_book(100.0, 12, 0.01);
    let mut ladder = AlignedOrderBook::new(link)
        .with_row_layout(AlignedOrderBookRowLayout::ReadableAnchored)
        .with_book(bids, asks);
    ladder.project_viewport_for_test(200.0, 160.0);
    let (row, y, height, _) = ladder
        .presented_rows_for_test()
        .into_iter()
        .find(|(row, _, _, _)| row.side == BookSideKind::Bid)
        .expect("readable bid row");
    let pointer_y = y + height * 0.5;

    assert!(ladder.hover_at_for_test(40.0, pointer_y));
    let hovered_price = match ladder.take_events().as_slice() {
        [
            crate::widgets::order_book::OrderBookEvent::LevelHoverEnter {
                price,
                side: BookSideKind::Bid,
                ..
            },
        ] => *price,
        events => panic!("unexpected hover events: {events:?}"),
    };
    assert!(hovered_price >= row.price_range.min && hovered_price <= row.price_range.max);

    let selection = ladder
        .select_at_for_test(40.0, pointer_y)
        .expect("readable row selection");
    assert_eq!(selection.side, BookSideKind::Bid);
    assert_eq!(selection.price, hovered_price);
}

#[test]
fn decimal_grouping_uses_rounding_bins_with_stable_decimal_labels() {
    let transform = PriceTransform::new(99.0, 101.0, AxisKind::Linear, 400.0);
    let config = MarketDetailConfig {
        grouping: MarketDetailGrouping::DecimalPlaces(1),
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_asks(BookSide {
        levels: vec![level(100.01, 2.0, "low"), level(100.04, 3.0, "high")],
    });

    let projection = projector.project(transform, config);
    let row = projection.levels.first().expect("decimal group");

    assert_eq!(projection.levels.len(), 1);
    assert!((row.price_range.min - 99.95).abs() < 1e-10);
    assert!((row.price_range.max - 100.05).abs() < 1e-10);
    assert!((row.price - 100.0).abs() < 1e-10);
    assert_eq!(row.quantity, 5.0);
    assert_eq!(row.source_level_count, 2);
}

#[test]
fn semantic_grouping_merges_only_the_minimum_bins_needed_for_pixel_safety() {
    let transform = PriceTransform::new(99.0, 101.0, AxisKind::Linear, 100.0);
    let config = MarketDetailConfig {
        grouping: MarketDetailGrouping::DecimalPlaces(1),
        min_bucket_height: 10.0,
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_asks(BookSide {
        levels: vec![level(100.04, 2.0, "ask")],
    });

    let projection = projector.project(transform, config);
    let row = projection.levels.first().expect("pixel-safe decimal group");
    let interval = row.price_range.max - row.price_range.min;

    assert!((interval - 0.2).abs() < 1e-10);
    assert!((row.height - 10.0).abs() < 1e-4);
}

#[test]
fn explicit_price_step_is_validated_and_projects_on_its_global_grid() {
    assert!(MarketDetailPriceStep::new(0.0).is_none());
    assert!(MarketDetailPriceStep::new(-0.25).is_none());
    assert!(MarketDetailPriceStep::new(f64::NAN).is_none());
    let step = MarketDetailPriceStep::new(0.25).expect("positive finite step");
    assert_eq!(step.get(), 0.25);

    let config = MarketDetailConfig {
        grouping: MarketDetailGrouping::PriceStep(step),
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_bids(BookSide {
        levels: vec![level(100.11, 1.0, "bid")],
    });
    let projection = projector.project(
        PriceTransform::new(99.0, 101.0, AxisKind::Linear, 400.0),
        config,
    );
    let row = projection.levels.first().expect("fixed-step group");

    assert!((row.price_range.min - 99.875).abs() < 1e-10);
    assert!((row.price_range.max - 100.125).abs() < 1e-10);
    assert!((row.price - 100.0).abs() < 1e-10);
}

#[test]
fn logarithmic_price_ranges_use_stable_axis_coordinate_midpoints() {
    let transform = PriceTransform::new(10.0, 1_000.0, AxisKind::Log, 200.0);
    let config = MarketDetailConfig {
        min_bucket_height: 20.0,
        ..MarketDetailConfig::default()
    };
    let mut projector = MarketDetailProjector::default();
    projector.replace_asks(BookSide {
        levels: vec![level(120.0, 2.0, "ask")],
    });
    let projection = projector.project(transform, config);
    let row = projection.levels.first().expect("logarithmic grouped row");

    assert!((row.price_range.min - 100.0).abs() < 1e-10);
    assert!((row.price_range.max - 10_f64.powf(2.2)).abs() < 1e-10);
    assert!((row.price - 10_f64.powf(2.1)).abs() < 1e-10);
    assert!(
        (transform.price_to_px(row.price).expect("log row anchor") - (row.y + row.height * 0.5))
            .abs()
            < 1e-4
    );
}

#[test]
fn sparse_near_mid_levels_are_pixel_bucketed_without_row_collisions() {
    let (bids, asks) = dense_book(100.0, 12, 0.01);
    let mut projector = MarketDetailProjector::default();
    projector.replace_book(bids, asks);
    let config = MarketDetailConfig {
        min_bucket_height: 14.0,
        ..MarketDetailConfig::default()
    };
    let projection = projector.project(
        PriceTransform::new(50.0, 150.0, AxisKind::Linear, 280.0),
        config,
    );

    assert_eq!(projection.levels.len(), 2);
    assert!(
        projection
            .levels
            .iter()
            .all(|level| level.source_level_count == 12)
    );
    for side in [BookSideKind::Bid, BookSideKind::Ask] {
        let mut rows: Vec<_> = projection
            .levels
            .iter()
            .filter(|level| level.side == side)
            .collect();
        rows.sort_by(|a, b| a.y.total_cmp(&b.y));
        assert!(
            rows.windows(2)
                .all(|pair| pair[0].y + pair[0].height <= pair[1].y)
        );
    }

    let exact = projector
        .nearest_exact(BookSideKind::Bid, 99.941, 50.0, 150.0)
        .expect("exact source level remains available");
    assert_eq!(exact.price, 99.94);
    assert_eq!(exact.metadata, vec!["bid-5"]);
}

#[test]
fn projected_group_labels_show_one_price_and_omit_internal_multiplicity() {
    let (bids, asks) = dense_book(100.0, 12, 0.01);
    let link = MarketDetailLink::new(50.0, 150.0);
    let mut ladder = AlignedOrderBook::new(link).with_book(bids, asks);
    ladder.project_viewport_for_test(300.0, 280.0);
    let bucket = ladder
        .projection()
        .levels
        .iter()
        .find(|level| level.source_level_count > 1)
        .expect("projected multi-level bucket")
        .clone();

    let label = ladder.quantity_label_for_test(&bucket);
    assert!(!label.contains('×'));
    let price_label = ladder.price_label_for_test(&bucket);
    assert_eq!(price_label, format!("{:.2}", bucket.price));
    assert!(!price_label.contains('–'));
}

#[test]
fn linked_reference_is_the_boundary_between_adjacent_grouped_rows() {
    let link = MarketDetailLink::new(99.0, 101.0);
    link.set_reference_line(Some(MarketDetailReferenceLine {
        price: 100.13,
        color: Color::from_hex(0x0B6EFD),
        line_style: PriceLevelLine::Solid,
    }));
    let mut ladder = AlignedOrderBook::new(link)
        .with_projection_config(MarketDetailConfig {
            grouping: MarketDetailGrouping::DecimalPlaces(1),
            ..MarketDetailConfig::default()
        })
        .with_book(
            BookSide {
                levels: vec![level(100.01, 1.0, "bid-low"), level(100.12, 2.0, "bid")],
            },
            BookSide {
                levels: vec![level(100.14, 3.0, "ask"), level(100.29, 4.0, "ask-high")],
            },
        );
    ladder.project_viewport_for_test(300.0, 200.0);

    let line_y = ladder
        .reference_line_y_for_test(200.0)
        .expect("linked reference Y");
    let bid = ladder
        .projection()
        .levels
        .iter()
        .find(|row| row.side == BookSideKind::Bid && (row.price_range.max - 100.13).abs() < 1e-10)
        .expect("grouped bid below reference");
    let ask = ladder
        .projection()
        .levels
        .iter()
        .find(|row| row.side == BookSideKind::Ask && (row.price_range.min - 100.13).abs() < 1e-10)
        .expect("grouped ask above reference");

    assert!((bid.price_range.max - 100.13).abs() < 1e-10);
    assert!((ask.price_range.min - 100.13).abs() < 1e-10);
    assert!((bid.y - line_y).abs() < 1e-4);
    assert!((ask.y + ask.height - line_y).abs() < 1e-4);
    assert_eq!(
        ladder.price_label_for_test(bid),
        format!("{:.2}", bid.price)
    );
    assert_eq!(
        ladder.price_label_for_test(ask),
        format!("{:.2}", ask.price)
    );
}

#[test]
fn opted_in_plot_price_level_extends_through_aligned_book_with_plot_style() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let mut plot = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 99.0, 101.0))
        .with_market_detail_link(link.clone())
        .with_market_detail_reference_level(10)
        .with_price_level(PriceLevel {
            id: 10,
            y: 100.0,
            line_style: PriceLevelLine::Dotted,
            ..PriceLevel::default()
        });
    let light = Theme::light();
    let mut light_cx = UpdateContext::with_theme(None, &light);
    plot.update(0.0, &mut light_cx);

    let reference = link.reference_line().expect("linked reference line");
    assert_eq!(reference.price, 100.0);
    assert_eq!(reference.color, PlotColorScheme::light().annotation_neutral);
    assert_eq!(reference.line_style, PriceLevelLine::Dotted);

    let ladder = AlignedOrderBook::new(link.clone());
    assert_eq!(ladder.reference_line_y_for_test(200.0), Some(100.0));

    let explicit = Color::from_hex(0x0B6EFD);
    plot.update_price_level(10, |level| level.color = explicit);
    let dark = Theme::dark();
    let mut dark_cx = UpdateContext::with_theme(None, &dark);
    plot.update(0.0, &mut dark_cx);
    assert_eq!(
        link.reference_line()
            .expect("explicit linked reference line")
            .color,
        explicit
    );

    plot.set_market_detail_reference_level(None);
    assert!(link.reference_line().is_none());
}

#[test]
fn quantity_fill_defaults_accept_a_second_one_third_speed_increase() {
    let config = MarketDetailConfig::default();
    assert_eq!(config.scale_attack, 64.0 / 3.0);
    assert_eq!(config.scale_release, 16.0 / 3.0);

    let mut scale = VisibleDepthScale::default();
    scale.set_visible_levels(
        &[ProjectedBookLevel {
            side: BookSideKind::Bid,
            price: 100.0,
            price_range: ProjectedPriceRange {
                min: 99.5,
                max: 100.5,
            },
            quantity: 5.0,
            source_level_count: 1,
            y: 0.0,
            height: 14.0,
        }],
        config,
    );
    let dt = 0.1_f32;
    scale.advance(dt, config);
    let expected_scale = 1.0 + 4.0 * (1.0 - (-(64.0_f32 / 3.0) * dt).exp()) as f64;
    assert!((scale.current() - expected_scale).abs() < 1e-12);

    let custom = MarketDetailConfig {
        scale_attack: 2.5,
        scale_release: 0.75,
        ..config
    };
    let ladder =
        AlignedOrderBook::new(MarketDetailLink::new(99.0, 102.0)).with_projection_config(custom);
    assert_eq!(ladder.projection_config_for_test(), custom);
}

#[test]
fn nominated_market_detail_reference_matches_authoritative_price_without_independent_easing() {
    let link = MarketDetailLink::new(99.0, 102.0);
    let mut plot = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 99.0, 102.0))
        .with_market_detail_link(link.clone())
        .with_market_detail_reference_level(10)
        .with_price_level(PriceLevel {
            id: 10,
            y: 100.0,
            ..PriceLevel::default()
        });
    plot.update_price_level(10, |level| level.y = 101.0);

    assert_eq!(plot.price_level_display_y_for_test(10), Some(101.0));
    assert_eq!(
        link.reference_line()
            .expect("synchronized linked price line")
            .price,
        101.0
    );

    let mut update = UpdateContext::new(None);
    plot.update(0.1, &mut update);

    assert_eq!(plot.price_level_display_y_for_test(10), Some(101.0));
    assert_eq!(
        link.reference_line()
            .expect("frame-synchronized linked price line")
            .price,
        101.0
    );
}

#[test]
fn plot_render_jobs_require_matching_series_and_overlay_generations() {
    assert!(render_job_is_coherent_for_test(4, 9, 5, 9, false));
    assert!(!render_job_is_coherent_for_test(4, 10, 5, 9, false));
    assert!(!render_job_is_coherent_for_test(5, 9, 5, 9, false));
    assert!(!render_job_is_coherent_for_test(4, 9, 5, 9, true));
}

fn observe_aligned_reference_repaint(
    link: &MarketDetailLink,
    line_color: Color,
    initial_y: f32,
) -> impl FnOnce(f32) {
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(AlignedOrderBook::new(link.clone())), None, 0);
    let owner = tree.node(root).unwrap().id;
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(240.0, 200.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let scene_line_color: crate::core::scene::Color = line_color.into();
    let line_y = move |nodes: &[SceneNode]| {
        let mut lines = nodes.iter().filter_map(|node| match node {
            SceneNode::Rect(line) if line.color == scene_line_color => Some(line.rect),
            _ => None,
        });
        let line = lines.next().expect("published ladder reference line");
        assert!(lines.next().is_none());
        line.y + line.height * 0.5
    };

    let initial = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    assert!(initial.scene_built);
    let initial_fragment = harness.local_fragment(owner).unwrap();
    assert_eq!(line_y(&initial_fragment), initial_y);
    assert!(!tree.any_widget_needs_update());

    move |published_y| {
        assert!(tree.any_widget_needs_update());
        assert!(!tree.any_widget_needs_animation());

        let updated = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(updated.scene_built && updated.visual_changed);
        let updated_fragment = harness.local_fragment(owner).unwrap();
        assert_eq!(line_y(&updated_fragment), published_y);
        assert!(!tree.any_widget_needs_update());

        let idle = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &updated_fragment,
            &harness.local_fragment(owner).unwrap(),
        ));
    }
}

#[test]
fn completed_plot_frame_publishes_reference_and_price_transform_together() {
    let link = MarketDetailLink::new(99.0, 102.0);
    let color = Color::from_hex(0x0B6EFD);
    link.set_reference_line(Some(MarketDetailReferenceLine {
        price: 101.25,
        color,
        line_style: PriceLevelLine::Solid,
    }));
    let assert_repaint = observe_aligned_reference_repaint(&link, color, 50.0);
    let source_axis = link.price_axis();
    source_axis
        .lock()
        .expect("source price axis")
        .set_range(80.0, 120.0);
    assert_eq!(link.price_range(), (99.0, 102.0));

    let mut presented_labels = None;
    CompletedRenderPublication {
        generation: 7,
        view_state: ViewState::new(0.0, 100.0, 90.0, 110.0),
        reference_tag: None,
        market_detail_publication: Some((
            link.clone(),
            Some(MarketDetailReferenceLine {
                price: 100.0,
                color,
                line_style: PriceLevelLine::Solid,
            }),
        )),
    }
    .publish(&mut presented_labels);

    assert_eq!(link.price_range(), (90.0, 110.0));
    assert_eq!(
        presented_labels.map(|labels| (labels.view_state.y_min, labels.view_state.y_max)),
        Some((90.0, 110.0))
    );
    assert_eq!(link.reference_line().unwrap().price, 100.0);
    assert_repaint(100.0);
}

#[test]
fn aligned_rows_use_the_presented_plot_transform_even_when_the_book_updated_first() {
    let link = MarketDetailLink::new(90.0, 110.0);
    let mut ladder = AlignedOrderBook::new(link.clone()).with_book(
        BookSide {
            levels: vec![level(100.0, 2.0, "bid")],
        },
        BookSide::default(),
    );
    ladder.project_viewport_for_test(240.0, 200.0);
    let initial = ladder.projection().clone();

    // Reproduce sibling traversal where the ladder has already updated before Plot
    // presents its completed texture, Y domain, and reference.
    ladder.poll_projection_for_test();
    CompletedRenderPublication {
        generation: 8,
        view_state: ViewState::new(0.0, 100.0, 95.0, 115.0),
        reference_tag: None,
        market_detail_publication: Some((
            link,
            Some(MarketDetailReferenceLine {
                price: 100.0,
                color: Color::from_hex(0x0B6EFD),
                line_style: PriceLevelLine::Solid,
            }),
        )),
    }
    .publish(&mut None);

    let same_frame_projection = ladder.presented_projection_for_test();
    assert_ne!(same_frame_projection, initial);
    assert_eq!(ladder.reference_line_y_for_test(200.0), Some(150.0));

    ladder.poll_projection_for_test();
    assert_eq!(same_frame_projection, *ladder.projection());
}

#[test]
fn aligned_book_repaints_when_completed_reference_publication_changes() {
    let link = MarketDetailLink::new(99.0, 103.0);
    let color = Color::from_hex(0x0B6EFD);
    let mut plot = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 99.0, 103.0))
        .with_market_detail_link(link.clone())
        .with_market_detail_reference_level(10)
        .with_price_level(PriceLevel {
            id: 10,
            y: 100.0,
            color,
            line_style: PriceLevelLine::Solid,
            ..PriceLevel::default()
        });
    let assert_repaint = observe_aligned_reference_repaint(&link, color, 150.0);

    plot.update_price_level(10, |level| level.y = 101.0);
    assert_eq!(plot.price_level_display_y_for_test(10), Some(101.0));
    assert_eq!(link.price_range(), (99.0, 103.0));
    assert_eq!(link.reference_line().unwrap().price, 101.0);
    assert_repaint(100.0);
}

#[test]
fn reference_reanchoring_projects_once_per_observed_generation() {
    let link = MarketDetailLink::new(99.0, 101.0);
    link.set_reference_line(Some(MarketDetailReferenceLine {
        price: 100.0,
        color: Color::from_hex(0x0B6EFD),
        line_style: PriceLevelLine::Solid,
    }));
    let mut ladder = AlignedOrderBook::new(link.clone()).with_book(
        BookSide {
            levels: vec![level(99.91, 2.0, "bid")],
        },
        BookSide {
            levels: vec![level(100.14, 3.0, "ask")],
        },
    );
    ladder.project_viewport_for_test(300.0, 200.0);
    let initial_count = ladder.projection_count();

    link.set_reference_line(Some(MarketDetailReferenceLine {
        price: 100.13,
        color: Color::from_hex(0x0B6EFD),
        line_style: PriceLevelLine::Solid,
    }));
    ladder.poll_projection_for_test();
    assert_eq!(ladder.projection_count(), initial_count + 1);

    ladder.poll_projection_for_test();
    assert_eq!(ladder.projection_count(), initial_count + 1);
}

#[test]
fn market_detail_link_preserves_plot_time_and_free_pan_gestures() {
    let link = MarketDetailLink::new(90.0, 110.0);
    let mut plot = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 90.0, 110.0))
        .with_market_detail_link(link.clone());

    plot.pan_drag_for_test(20.0, 10.0, 200.0, 100.0, false);
    assert_eq!(plot.view_state().x_min, -10.0);
    assert_eq!(plot.view_state().x_max, 90.0);
    assert_eq!(plot.view_state().y_min, 90.0);
    assert_eq!(plot.view_state().y_max, 110.0);
    assert_eq!(link.price_range(), (90.0, 110.0));

    plot.pan_drag_for_test(0.0, 10.0, 200.0, 100.0, true);
    assert_eq!(plot.view_state().x_min, -10.0);
    assert_eq!(plot.view_state().x_max, 90.0);
    assert_eq!(plot.view_state().y_min, 92.0);
    assert_eq!(plot.view_state().y_max, 112.0);
    assert_eq!(link.price_range(), (92.0, 112.0));
}

#[test]
fn aligned_hover_emits_local_events_without_publishing_a_shared_probe() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let probe = link.probe();
    let mut ladder = AlignedOrderBook::new(link).with_bids(BookSide {
        levels: vec![level(99.5, 2.0, "bid")],
    });
    ladder.project_viewport_for_test(200.0, 200.0);
    let row = ladder
        .presented_projection_for_test()
        .levels
        .into_iter()
        .find(|row| row.side == BookSideKind::Bid)
        .expect("visible bid row");

    assert!(ladder.hover_at_for_test(40.0, row.y + row.height * 0.5));
    assert!(!probe.lock().expect("shared probe").is_active());
    assert!(matches!(
        ladder.take_events().as_slice(),
        [
            crate::widgets::order_book::OrderBookEvent::LevelHoverEnter {
                price: 99.5,
                side: BookSideKind::Bid,
                quantity: 2.0,
            }
        ]
    ));
}

#[test]
fn aligned_hover_tracks_the_projected_row_and_leaves_real_liquidity_gaps_empty() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let mut ladder = AlignedOrderBook::new(link).with_bids(BookSide {
        levels: vec![level(99.2, 1.0, "low"), level(99.8, 2.0, "high")],
    });
    ladder.project_viewport_for_test(200.0, 200.0);

    let mut rows: Vec<_> = ladder
        .presented_projection_for_test()
        .levels
        .into_iter()
        .filter(|row| row.side == BookSideKind::Bid)
        .collect();
    rows.sort_by(|left, right| left.y.total_cmp(&right.y));
    assert_eq!(rows.len(), 2);

    let hovered_y = rows[0].y + rows[0].height * 0.5;
    assert!(ladder.hover_at_for_test(40.0, hovered_y));
    let hovered = ladder
        .hovered_projected_level_for_test()
        .expect("hovered projected row");
    assert_eq!(hovered.price_range, rows[0].price_range);

    let gap_top = rows[0].y + rows[0].height;
    let gap_bottom = rows[1].y;
    assert!(gap_bottom > gap_top);
    assert!(ladder.hover_at_for_test(40.0, (gap_top + gap_bottom) * 0.5));
    assert!(ladder.hovered_projected_level_for_test().is_none());
    assert!(matches!(
        ladder.take_events().as_slice(),
        [
            crate::widgets::order_book::OrderBookEvent::LevelHoverEnter { .. },
            crate::widgets::order_book::OrderBookEvent::LevelHoverLeave
        ]
    ));
}

#[test]
fn aligned_wheel_pan_is_coalesced_and_consumed_by_the_plot_owner() {
    let link = MarketDetailLink::new(90.0, 110.0);
    let mut plot = Plot::new()
        .with_view_state(ViewState::new(0.0, 100.0, 90.0, 110.0))
        .with_market_detail_link(link.clone());
    let mut ladder = AlignedOrderBook::new(link.clone());
    ladder.project_viewport_for_test(200.0, 100.0);

    assert!(ladder.queue_price_pan_for_test(4.0));
    assert!(ladder.queue_price_pan_for_test(6.0));
    assert!(plot.apply_market_detail_navigation_for_test());
    assert_eq!(plot.view_state().y_min, 92.0);
    assert_eq!(plot.view_state().y_max, 112.0);
    assert_eq!(link.price_range(), (92.0, 112.0));
    assert!(!plot.apply_market_detail_navigation_for_test());
    assert!(ladder.wants_scroll());
    assert!(
        !AlignedOrderBook::new(link)
            .with_wheel_price_pan(false)
            .wants_scroll()
    );
}

#[test]
fn aligned_selection_keeps_exact_identity_and_never_reattaches_to_a_nearby_level() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let mut ladder = AlignedOrderBook::new(link).with_bids(BookSide {
        levels: vec![level(99.4, 1.0, "selected"), level(99.6, 3.0, "nearby")],
    });
    ladder.project_viewport_for_test(200.0, 200.0);
    let selected_row = ladder
        .presented_projection_for_test()
        .levels
        .into_iter()
        .find(|row| {
            row.side == BookSideKind::Bid
                && 99.4 >= row.price_range.min
                && 99.4 < row.price_range.max
        })
        .expect("projected selected level");

    let selected = ladder
        .select_at_for_test(40.0, selected_row.y + selected_row.height * 0.5)
        .expect("exact visible selection");
    assert_eq!(
        selected,
        AlignedOrderBookSelection {
            side: BookSideKind::Bid,
            price: 99.4,
        }
    );
    assert_eq!(ladder.selection(), Some(selected));
    assert!(
        ladder
            .selected_projected_level_for_test()
            .is_some_and(|row| {
                selected.price >= row.price_range.min && selected.price <= row.price_range.max
            })
    );

    ladder.set_bids(BookSide {
        levels: vec![level(99.6, 3.0, "nearby")],
    });
    ladder.poll_projection_for_test();
    assert_eq!(ladder.selection(), Some(selected));
    assert!(ladder.selected_projected_level_for_test().is_none());

    ladder.clear_selection();
    assert_eq!(ladder.selection(), None);
}

#[test]
fn visible_depth_scale_rejects_one_transient_outlier_and_releases_smoothly() {
    let mut projector = MarketDetailProjector::default();
    projector.replace_book(
        BookSide {
            levels: (0..10)
                .map(|index| {
                    level(
                        99.0 + index as f64 * 0.01,
                        if index == 9 { 10_000.0 } else { 10.0 },
                        "bid",
                    )
                })
                .collect(),
        },
        BookSide::new(),
    );
    let config = MarketDetailConfig {
        min_bucket_height: 10.0,
        ..MarketDetailConfig::default()
    };
    let projection = projector.project(
        PriceTransform::new(98.9, 99.2, AxisKind::Linear, 200.0),
        config,
    );
    let mut scale = VisibleDepthScale::default();
    scale.set_visible_levels(&projection.levels, config);
    for _ in 0..120 {
        scale.advance(1.0 / 120.0, config);
    }

    assert!(scale.current() <= 40.0);
}

#[test]
fn unchanged_axis_ranges_do_not_invalidate_or_reproject_aligned_mode() {
    let mut axis = AxisAuthority::new(99.0, 101.0);
    axis.set_range(99.0, 101.0);
    assert_eq!(axis.generation(), 0);
    axis.set_range(99.5, 100.5);
    assert_eq!(axis.generation(), 1);
    axis.set_range(99.5, 100.5);
    assert_eq!(axis.generation(), 1);

    let link = MarketDetailLink::new(99.0, 101.0);
    let (bids, asks) = dense_book(100.0, 100, 0.01);
    let mut ladder = AlignedOrderBook::new(link.clone()).with_book(bids, asks);
    ladder.project_viewport_for_test(300.0, 200.0);
    let initial_count = ladder.projection_count();

    ladder.poll_projection_for_test();
    ladder.poll_projection_for_test();
    assert_eq!(ladder.projection_count(), initial_count);

    link.set_price_range_for_test(99.5, 100.5);
    ladder.poll_projection_for_test();
    assert_eq!(ladder.projection_count(), initial_count + 1);

    link.set_price_range_for_test(99.5, 100.5);
    ladder.poll_projection_for_test();
    assert_eq!(ladder.projection_count(), initial_count + 1);
}

#[test]
fn aligned_mode_preserves_explicit_consumer_side_colors() {
    let bid = Color::from_hex(0x0066CC);
    let ask = Color::from_hex(0xCC5500);
    let config = OrderBookConfig {
        bid_color: bid,
        ask_color: ask,
        price_formatter: Some(Arc::new(|price| format!("{price:.1}"))),
        ..OrderBookConfig::default()
    };
    let ladder = AlignedOrderBook::new(MarketDetailLink::new(99.0, 101.0)).with_config(config);

    assert_eq!(ladder.config_for_test().bid_color, bid);
    assert_eq!(ladder.config_for_test().ask_color, ask);
    assert_eq!(
        ladder.price_label_for_test(&ProjectedBookLevel {
            side: BookSideKind::Ask,
            price: 100.0,
            price_range: ProjectedPriceRange {
                min: 99.5,
                max: 100.5,
            },
            quantity: 2.0,
            source_level_count: 2,
            y: 0.0,
            height: 14.0,
        }),
        "100.0"
    );
}

#[test]
fn rapid_snapshot_bursts_rebuild_the_source_index_once_per_update() {
    let link = MarketDetailLink::new(99.0, 101.0);
    let (mut ladder, tx) = AlignedOrderBook::new(link).with_update_channel();
    ladder.project_viewport_for_test(300.0, 200.0);

    for count in [10, 20, 30, 40] {
        let (bids, asks) = dense_book(100.0, count, 0.01);
        tx.send(OrderBookUpdate::Snapshot { bids, asks })
            .expect("snapshot send");
    }

    let mut update = UpdateContext::new(None);
    ladder.update(1.0 / 120.0, &mut update);

    assert_eq!(ladder.source_generation_for_test(), 1);
    assert!(!ladder.projection().levels.is_empty());
}
