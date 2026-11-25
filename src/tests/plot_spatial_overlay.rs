use crate::core::math::RectF;
use crate::plot::chart_info_overlay_rect;

#[test]
fn plot_chart_info_overlay_preserves_viewport_edge_padding() {
    assert_eq!(
        chart_info_overlay_rect(RectF::new(0.0, 0.0, 320.0, 180.0)),
        RectF::new(8.0, 8.0, 0.0, 0.0)
    );
    assert_eq!(
        chart_info_overlay_rect(RectF::new(40.0, 30.0, 800.0, 450.0)),
        RectF::new(48.0, 38.0, 0.0, 0.0)
    );
}
