use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::icon::IconId;
use crate::core::icons::builtin_registry;
use crate::core::math::RectF;
use crate::core::paint::{
    Color, LuminousContourAppearance, LuminousContourError, LuminousContourFronts, PaintSink,
    RoundedCorners, TextBackgroundSpan, ZIndex,
};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
use crate::core::theme::Theme;
use crate::core::widget::PaintContext;
use crate::core::widget_tree::WidgetTree;
use crate::widgets::disclosure_indicator::paint_disclosure_indicator;

struct NullTextEngine;

impl TextEngine for NullTextEngine {
    fn measure(&mut self, _text: &str, _font_size: f32) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn metrics(&mut self, _text: &str, _font_size: f32) -> TextMetrics {
        TextMetrics {
            ascent: 0.0,
            descent: 0.0,
            line_gap: 0.0,
            line_height: 0.0,
        }
    }

    fn shape(&mut self, _text: &str, _font_size: f32) -> GlyphRun {
        GlyphRun {
            binding: None,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            width: 0.0,
            height: 0.0,
            text: String::new(),
            font_size: 0.0,
            ascent: 0.0,
            descent: 0.0,
        }
    }

    fn draw(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _z_index: ZIndex,
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
        _z_index: ZIndex,
        _sink: &mut dyn PaintSink,
    ) {
    }

    fn font_metrics(&mut self, _font_size: f32) -> TextMetrics {
        TextMetrics {
            ascent: 0.0,
            descent: 0.0,
            line_gap: 0.0,
            line_height: 0.0,
        }
    }
}

/// Paint sink that tracks push_icon calls.
struct IconTrackingSink {
    icon_calls: Cell<Vec<u32>>,
    line_calls: Cell<usize>,
    rounded_rects: Cell<Vec<RectF>>,
}

impl IconTrackingSink {
    fn new() -> Self {
        Self {
            icon_calls: Cell::new(Vec::new()),
            line_calls: Cell::new(0),
            rounded_rects: Cell::new(Vec::new()),
        }
    }

    fn take_icon_ids(&self) -> Vec<u32> {
        self.icon_calls.take()
    }

    fn take_line_count(&self) -> usize {
        self.line_calls.take()
    }

    fn take_rounded_rects(&self) -> Vec<RectF> {
        self.rounded_rects.take()
    }
}

impl PaintSink for IconTrackingSink {
    fn scale_factor(&self) -> f32 {
        1.0
    }

    fn begin_clip(&mut self, _rect: RectF, _rounded: Option<RoundedCorners>) {}
    fn end_clip(&mut self) {}
    fn begin_scroll(&mut self, _offset: (f32, f32)) {}
    fn end_scroll(&mut self) {}

    fn push_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_border_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _border_width: f32,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_rounded_rect(
        &mut self,
        rect: RectF,
        _color: Color,
        _corners: RoundedCorners,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
        let mut rects = self.rounded_rects.take();
        rects.push(rect);
        self.rounded_rects.set(rects);
    }

    fn push_border_rounded_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _border_width: f32,
        _corners: RoundedCorners,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_shadow(
        &mut self,
        _shape_rect: RectF,
        _shape_radius: f32,
        _mode: crate::core::paint::ShadowMode,
        _offset: (f32, f32),
        _spread: f32,
        _blur_radius: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_gradient_rect(
        &mut self,
        _rect: RectF,
        _radius: f32,
        _gradient: &crate::core::paint::GradientDef,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_color_picker_sv(
        &mut self,
        _rect: RectF,
        _hue: f32,
        _radius: f32,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_radial_gauge(
        &mut self,
        _rect: RectF,
        _fill: f32,
        _thickness: f32,
        _track_color: Color,
        _fill_color: Color,
        _fill_color_negative: Color,
        _start_angle: f32,
        _sweep_angle: f32,
        _origin_fill: f32,
        _mode: u32,
        _fill_style: u32,
        _zone_count: u32,
        _zones: [(f32, f32, [f32; 4]); 4],
        _major_divisions: u32,
        _minor_per_major: u32,
        _tick_length_major: f32,
        _tick_length_minor: f32,
        _tick_color: [f32; 4],
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_circle(
        &mut self,
        _center: crate::core::math::Point,
        _radius: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_ellipse(
        &mut self,
        _center: crate::core::math::Point,
        _radii: crate::core::math::Point,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_text_with_backgrounds(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _backgrounds: Vec<TextBackgroundSpan>,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_path(
        &mut self,
        _path: crate::renderer::path::Path,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_icon(
        &mut self,
        definition: &crate::core::icon::IconDefinition,
        _x: f32,
        _y: f32,
        _size: f32,
        _color: Color,
        _z_index: ZIndex,
    ) {
        let mut ids = self.icon_calls.take();
        ids.push(definition.id.0);
        self.icon_calls.set(ids);
    }

    fn push_image(
        &mut self,
        _rect: RectF,
        _texture_id: u64,
        _uv_rect: RectF,
        _tint: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_line(
        &mut self,
        _a: crate::core::math::Point,
        _b: crate::core::math::Point,
        _width: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
        self.line_calls.set(self.line_calls.get() + 1);
    }

    fn push_polyline(
        &mut self,
        _points: Vec<crate::core::math::Point>,
        _width: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_luminous_contour(
        &mut self,
        _points: Arc<[crate::core::math::Point]>,
        _core_width: f32,
        _color: Color,
        _appearance: LuminousContourAppearance,
        _fronts: LuminousContourFronts,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) -> Result<(), LuminousContourError> {
        Ok(())
    }

    fn set_current_painting_widget(&mut self, _widget_id: u64) {}
    fn clear_current_painting_widget(&mut self) {}

    #[cfg(debug_assertions)]
    fn push_text_probe(
        &mut self,
        _position: (f32, f32),
        _text: &str,
        _font_size: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }
}

#[test]
fn local_ambient_glow_unsupported_sink_refuses_without_emission() {
    use crate::core::paint::{LocalAmbientGlowAppearance, LocalAmbientGlowError};
    let mut sink = IconTrackingSink::new();
    assert_eq!(
        sink.push_local_ambient_glow(
            RectF::new(0.0, 0.0, 10.0, 10.0),
            LocalAmbientGlowAppearance::try_new(&[]).unwrap(),
            0,
            None,
        ),
        Err(LocalAmbientGlowError::UnsupportedPaintSink)
    );
    assert_eq!(sink.take_line_count(), 0);
    assert!(sink.take_rounded_rects().is_empty());
}

#[test]
#[should_panic(expected = "this PaintSink does not support target-bound Image identities")]
fn legacy_paint_sink_target_bound_default_is_fail_closed() {
    let mut sink = IconTrackingSink::new();
    let presentation_id = crate::renderer::CustomRenderAdmission::for_test().presentation_id();

    sink.push_target_bound_image(
        RectF::new(0.0, 0.0, 10.0, 10.0),
        presentation_id,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        0,
        None,
    );
}

#[test]
fn test_paint_context_has_icon_registry() {
    let tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = NullTextEngine;
    let icon_registry = builtin_registry();
    let mut sink = IconTrackingSink::new();
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: HashMap<_, _> = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();

    let cx = PaintContext::new(
        &tree,
        &mut sink,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );

    assert!(
        cx.icon_registry.get(IconId::FOLDER).is_some(),
        "FOLDER must be accessible via PaintContext"
    );
}

#[test]
fn test_push_icon_through_paint_context() {
    let tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = NullTextEngine;
    let icon_registry = builtin_registry();
    let mut sink = IconTrackingSink::new();
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: HashMap<_, _> = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();

    let mut cx = PaintContext::new(
        &tree,
        &mut sink,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );

    // Push folder icon through the PaintContext scene sink
    let folder_def = cx.icon_registry.get(IconId::FOLDER).unwrap();
    assert_eq!(folder_def.layers.len(), 3, "folder should have 3 layers");
    cx.scene
        .push_icon(folder_def, 10.0, 20.0, 14.0, Color::WHITE, 0);

    // Verify the sink received the call
    let ids = sink.take_icon_ids();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], IconId::FOLDER.0);
}

#[test]
fn disclosure_indicator_paints_vertical_bar_affordance() {
    let tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = NullTextEngine;
    let icon_registry = builtin_registry();
    let mut sink = IconTrackingSink::new();
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: HashMap<_, _> = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();

    {
        let mut cx = PaintContext::new(
            &tree,
            &mut sink,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            &mut overlay_regions,
            &mut tooltip_triggers,
            1.0,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        paint_disclosure_indicator(&mut cx, RectF::new(0.0, 0.0, 16.0, 24.0), false, 0);
        paint_disclosure_indicator(&mut cx, RectF::new(0.0, 0.0, 16.0, 24.0), true, 0);
    }

    assert_eq!(sink.take_icon_ids(), Vec::<u32>::new());
    assert_eq!(sink.take_line_count(), 0);
    let rects = sink.take_rounded_rects();
    assert_eq!(rects.len(), 3);

    let collapsed_left = rects[0];
    let collapsed_right = rects[1];
    let expanded = rects[2];

    assert!(collapsed_left.height > collapsed_left.width * 2.0);
    assert!(collapsed_right.height > collapsed_right.width * 2.0);
    assert!(expanded.height > expanded.width * 2.0);
    assert!(collapsed_right.x > collapsed_left.x + collapsed_left.width);
    assert_eq!(collapsed_left.y, collapsed_right.y);
    assert_eq!(collapsed_left.height, collapsed_right.height);
}

#[test]
fn test_all_builtins_accessible() {
    let tree = WidgetTree::new();
    let theme = Theme::default();
    let mut text_engine = NullTextEngine;
    let icon_registry = builtin_registry();
    let mut sink = IconTrackingSink::new();
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: HashMap<_, _> = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();

    let cx = PaintContext::new(
        &tree,
        &mut sink,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );

    let all_ids = [
        IconId::CLOSE,
        IconId::CHEVRON_RIGHT,
        IconId::CHEVRON_DOWN,
        IconId::PLUS,
        IconId::MINUS,
        IconId::CHECK,
        IconId::SEARCH,
        IconId::ARROW_UP,
        IconId::ARROW_DOWN,
        IconId::WARNING,
        IconId::FOLDER,
        IconId::FILE,
        IconId::FOLDER_OPEN,
        IconId::CALENDAR,
        IconId::SELECT_ALL,
        IconId::CLEAR_ALL,
    ];

    for id in &all_ids {
        assert!(
            cx.icon_registry.get(*id).is_some(),
            "builtin {:?} must be accessible via PaintContext",
            id
        );
    }
}
