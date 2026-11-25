use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::animation::Timeline;
use crate::core::event::KeyboardAuthorityMode;
use crate::core::icons::builtin_registry;
use crate::core::layout_transition::InterpolatedVisualRects;
use crate::core::math::RectF;
use crate::core::rect_authority::PaintAuthorityGuard;
use crate::core::scene::SceneBuilder;
use crate::core::text_engine::TextMetrics;
use crate::core::theme::Theme;
use crate::core::widget::{PaintContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::platform::backend_text_engine::BackendTextEngine;
use crate::platform::native_font_backend::{GlyphBitmap, NativeFontBackend};
use crate::plot::Plot;
use crate::renderer::glyph_atlas::GlyphKey;

struct CountingFontBackend {
    metric_calls: Arc<AtomicUsize>,
}

impl NativeFontBackend for CountingFontBackend {
    fn font_metrics(&mut self, font_size: f32, _scale: f32) -> TextMetrics {
        self.metric_calls.fetch_add(1, Ordering::Relaxed);
        TextMetrics {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            line_gap: 0.0,
            line_height: font_size,
        }
    }

    fn rasterize_glyph(&mut self, _key: &GlyphKey, _scale: f32) -> Option<GlyphBitmap> {
        None
    }
}

fn paint_plot(plot: &Plot, text_engine: &mut BackendTextEngine<CountingFontBackend>) {
    let tree = WidgetTree::new();
    let theme = Theme::light();
    let mut scene = SceneBuilder::new();
    let icon_registry = builtin_registry();
    let timeline = Timeline::new();
    let interpolated_rects: InterpolatedVisualRects = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let mut cx = PaintContext::new(
        &tree,
        &mut scene,
        &theme,
        text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        KeyboardAuthorityMode::Navigation,
    );
    let _paint = PaintAuthorityGuard::enter();
    plot.paint(&mut cx, RectF::new(0.0, 0.0, 800.0, 500.0), &[]);
}

#[test]
fn repeated_plot_paint_reuses_exact_native_font_metrics() {
    let metric_calls = Arc::new(AtomicUsize::new(0));
    let backend = CountingFontBackend {
        metric_calls: Arc::clone(&metric_calls),
    };
    let mut text_engine = BackendTextEngine::new(backend);
    let plot = Plot::new();

    paint_plot(&plot, &mut text_engine);
    assert_eq!(metric_calls.load(Ordering::Relaxed), 1);

    paint_plot(&plot, &mut text_engine);
    assert_eq!(
        metric_calls.load(Ordering::Relaxed),
        1,
        "an unchanged Plot frame must not recreate native metrics for every axis label"
    );

    text_engine.set_scale(2.0);
    paint_plot(&plot, &mut text_engine);
    assert_eq!(
        metric_calls.load(Ordering::Relaxed),
        2,
        "a scale identity change must compute one new exact metrics entry"
    );
}
