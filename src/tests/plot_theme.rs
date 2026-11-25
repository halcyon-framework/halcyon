use crate::core::paint::Color;
use crate::core::theme::Theme;
use crate::core::widget::{UpdateContext, Widget};
use crate::plot::{
    BarSeries, BarStyle, CandleSeries, LabelConfig, LineSeries, LineStyle, Plot, PlotColorScheme,
    Series,
};

#[test]
fn plot_defaults_resolve_from_the_active_theme() {
    let light = Theme::light();
    let expected_light = PlotColorScheme::from_theme(&light);
    let mut plot = Plot::new();
    let initial_generation = plot.render_generation_for_test();

    let mut light_cx = UpdateContext::with_theme(None, &light);
    plot.update(0.0, &mut light_cx);

    assert_eq!(plot.color_scheme, expected_light);
    assert_eq!(
        plot.label_colors_for_test(),
        Some((expected_light.label_text, expected_light.label_background))
    );
    assert_eq!(plot.render_generation_for_test(), initial_generation + 1);
    assert_ne!(expected_light.background, expected_light.candle_up);
    assert_ne!(expected_light.background, expected_light.candle_down);
    assert_ne!(expected_light.background, expected_light.bar);
    assert_ne!(expected_light.background, expected_light.line);
    assert_ne!(expected_light.background, expected_light.marker_fill);
    assert_eq!(expected_light.background, Color::from_hex(0xFAFBFC));
    assert_ne!(expected_light.background, Color::WHITE);

    let mut line = LineSeries::new();
    line.apply_color_scheme(&expected_light);
    assert_eq!(line.color_for_test(), expected_light.line);

    let mut bar = BarSeries::new();
    bar.apply_color_scheme(&expected_light);
    assert_eq!(bar.color_for_test(), expected_light.bar);
    assert_eq!(
        bar.viewport_scrim_color_for_test(),
        expected_light.volume_scrim
    );
    assert_eq!(expected_light.volume_scrim, Color::TRANSPARENT);

    let mut candles = CandleSeries::new();
    candles.apply_color_scheme(&expected_light);
    assert_eq!(
        candles.colors_for_test(),
        (
            expected_light.candle_up.to_array(),
            expected_light.candle_down.to_array()
        )
    );

    let stable_generation = plot.render_generation_for_test();
    plot.update(0.0, &mut light_cx);
    assert_eq!(plot.render_generation_for_test(), stable_generation);

    let dark = Theme::dark();
    let mut dark_cx = UpdateContext::with_theme(None, &dark);
    plot.update(0.0, &mut dark_cx);

    assert_eq!(plot.color_scheme, PlotColorScheme::dark());
    assert_eq!(plot.color_scheme.background, Color::BLACK);
    assert_eq!(
        plot.color_scheme.candle_up,
        Color::from_linear(0.0, 1.0, 0.0, 1.0)
    );
    assert_eq!(
        plot.color_scheme.candle_down,
        Color::from_linear(1.0, 0.0, 0.0, 1.0)
    );
    assert_eq!(
        plot.color_scheme.volume_scrim,
        Color::from_linear(0.0, 0.0, 0.0, 0.65)
    );
    assert_eq!(plot.render_generation_for_test(), stable_generation + 1);
}

#[test]
fn explicit_plot_and_series_colors_survive_theme_switches() {
    let custom_label = Color::from_hex(0x654321);
    let custom_label_background = Color::from_hex(0xFEDCBA);
    let mut custom_scheme = PlotColorScheme::dark();
    custom_scheme.background = Color::from_hex(0x123456);
    custom_scheme.line = Color::from_hex(0xABCDEF);

    let mut plot = Plot::new()
        .with_label_config(Some(LabelConfig {
            color: custom_label,
            bg_color: custom_label_background,
            ..LabelConfig::default()
        }))
        .with_color_scheme(custom_scheme);

    let light = Theme::light();
    let mut light_cx = UpdateContext::with_theme(None, &light);
    plot.update(0.0, &mut light_cx);
    let dark = Theme::dark();
    let mut dark_cx = UpdateContext::with_theme(None, &dark);
    plot.update(0.0, &mut dark_cx);

    assert_eq!(plot.color_scheme, custom_scheme);
    assert_eq!(
        plot.label_colors_for_test(),
        Some((custom_label, custom_label_background))
    );

    let default_labels_after_scheme = Plot::new()
        .with_color_scheme(custom_scheme)
        .with_label_config(Some(LabelConfig::default()));
    assert_eq!(
        default_labels_after_scheme.label_colors_for_test(),
        Some((custom_scheme.label_text, custom_scheme.label_background))
    );

    let custom_line = Color::from_hex(0xB03060);
    let mut line = LineSeries::new().with_style(LineStyle {
        color: custom_line,
        width: 2.0,
    });
    line.apply_color_scheme(&PlotColorScheme::light());
    assert_eq!(line.color_for_test(), custom_line);

    let custom_bar = Color::from_hex(0x7B3F00);
    let mut bar = BarSeries::new().with_style(BarStyle {
        color: custom_bar,
        ..BarStyle::default()
    });
    bar.apply_color_scheme(&PlotColorScheme::light());
    assert_eq!(bar.color_for_test(), custom_bar);
    assert_eq!(
        bar.viewport_scrim_color_for_test(),
        PlotColorScheme::light().volume_scrim
    );

    let custom_scrim = Color::from_hex(0x654321).with_alpha(0.25);
    let mut viewport_bars = BarSeries::new().with_viewport_scrim_color(custom_scrim);
    viewport_bars.apply_color_scheme(&PlotColorScheme::light());
    assert_eq!(viewport_bars.viewport_scrim_color_for_test(), custom_scrim);

    let custom_up = Color::from_hex(0x00FFFF);
    let custom_down = Color::from_hex(0xFF00FF);
    let mut candles = CandleSeries::new().with_colors(custom_up, custom_down);
    candles.apply_color_scheme(&PlotColorScheme::light());
    assert_eq!(
        candles.colors_for_test(),
        (custom_up.to_array(), custom_down.to_array())
    );
}

#[test]
fn theme_switch_marks_plot_render_state_dirty_during_update() {
    let dark = Theme::dark();
    let light = Theme::light();
    let mut plot = Plot::new();

    let mut dark_cx = UpdateContext::with_theme(None, &dark);
    plot.update(0.0, &mut dark_cx);
    let dark_generation = plot.render_generation_for_test();

    let mut light_cx = UpdateContext::with_theme(None, &light);
    plot.update(0.0, &mut light_cx);

    assert_eq!(plot.render_generation_for_test(), dark_generation + 1);
    assert_eq!(plot.color_scheme, PlotColorScheme::light());
}
