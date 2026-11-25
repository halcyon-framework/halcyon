use halcyon::core::layout::Constraints;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, Widget};
use halcyon::core::widget_tree::WidgetTree;
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;
use halcyon::widgets::gauge::Gauge;

#[test]
fn gauge_layout_reports_finite_square_size_when_unbounded() {
    let mut gauge = Gauge::new();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);

    let result = gauge.layout(
        &mut cx,
        Constraints::new(0.0, f32::INFINITY, 0.0, f32::INFINITY),
        &[],
    );

    assert!(result.size.width.is_finite());
    assert!(result.size.height.is_finite());
    assert!(result.size.width > 0.0);
    assert_eq!(result.size.width, result.size.height);
}

#[test]
fn gauge_layout_preserves_square_with_one_finite_axis() {
    let mut gauge = Gauge::new();
    let mut tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = FontManagerTextEngine::new();
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);

    let result = gauge.layout(
        &mut cx,
        Constraints::new(0.0, 120.0, 0.0, f32::INFINITY),
        &[],
    );

    assert_eq!(result.size.width, 120.0);
    assert_eq!(result.size.height, 120.0);
}
