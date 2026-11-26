use super::renderer_z_order::{read_compiled_surface, setup_with_size_and_scale};
use super::retained_value_updates::{ui_typography_font_assets, ui_typography_texts};
use crate::compositor::{Compositor, SurfaceId};
use crate::core::app::{RetainedSceneTestFrame, RetainedSceneTestHarness};
use crate::core::event::{
    Event, EventPhase, FusedInputEvent, InteractionDomains, PointerDevice, PointerEventKind,
    WindowId,
};
use crate::core::event_router::EventRouter;
use crate::core::icons::builtin_registry;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT};
use crate::core::paragraph::{LayoutParams, Paragraph, ParagraphLayoutEngine, TextStyle};
use crate::core::scene::{SceneBuilder, SceneGraph, SceneNode};
use crate::core::text_engine::{
    GlyphRun, SourceInkEffect, TextEngine, TextRangeEffectError, TextSourceRangeEffect,
};
use crate::core::text_shaper::TextDirection;
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager::FontAsset;
use crate::platform::font_manager_text_engine::FontIdentity;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::WgpuRenderer;
use crate::renderer::{DrawPlanBuilder, MultiTargetDrawPlan, TextRealizationOutcome};
use crate::runtime::binding_registry::BindingRegistry;
use crate::utils::id::WidgetInternalId;
use std::any::Any;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

#[cfg(debug_assertions)]
#[path = "source_ink_scale_settlement.rs"]
mod scale_settlement;

const SIZE: Size = Size::new(390.0, 120.0);
const TEXT: &str = "AV    NETTO    office";
const FONT_SIZE: f32 = 36.0;
const ORIGIN: Point = Point::new(12.0, 10.0);
const CLIP: RectF = RectF::new(18.0, 0.0, 350.0, 75.0);
const RED: Color = Color::from_linear(1.0, 0.0, 0.0, 1.0);
const GREEN: Color = Color::from_linear(0.0, 1.0, 0.0, 1.0);

fn range(text: &str, part: &str) -> Range<usize> {
    let start = text.find(part).unwrap();
    start..start + part.len()
}

fn effect(source: Range<usize>, foreground: Color, scale: f32) -> TextSourceRangeEffect {
    TextSourceRangeEffect {
        source,
        foreground,
        scale,
    }
}

fn latin_ligature_font() -> FontAsset {
    let data =
        Arc::new(std::fs::read("/System/Library/Fonts/Supplemental/Apple Chancery.ttf").unwrap());
    let identity = FontIdentity::from_font_data(&data, "Apple Chancery");
    FontAsset {
        family_name: identity.family_name,
        weight: identity.weight,
        italic: identity.italic,
        data,
    }
}

struct InkTitle {
    id: WidgetInternalId,
    style: TextStyle,
    hovered: Option<f32>,
    layout_calls: Rc<Cell<usize>>,
}

impl WidgetDowncast for InkTitle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl InkTitle {
    fn shape(&self, engine: &mut dyn TextEngine) -> GlyphRun {
        engine
            .shape_styled(TEXT, Some(&self.style), FONT_SIZE)
            .unwrap()
    }

    fn set_hover(&mut self, hovered: Option<f32>, cx: &mut EventContext) {
        if self.hovered != hovered {
            self.hovered = hovered;
            cx.invalidate_paint();
        }
    }
}

impl Widget for InkTitle {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        _: &mut LayoutContext,
        constraints: Constraints,
        _: &[NodeId],
    ) -> LayoutResult {
        self.layout_calls.set(self.layout_calls.get() + 1);
        LayoutResult::fit(constraints.constrain(SIZE))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        let run = self.shape(cx.text_engine);
        let origin = Point::new(rect.x + ORIGIN.x, rect.y + ORIGIN.y);
        cx.scene.begin_clip(CLIP, None);
        if let Some(scale) = self.hovered {
            cx.text_engine
                .draw_with_source_range_effects(
                    &run,
                    origin,
                    Color::WHITE,
                    LAYER_CONTENT,
                    &[
                        effect(range(TEXT, "NETTO"), RED, scale),
                        effect(range(TEXT, "office"), GREEN, 1.0),
                    ],
                    cx.scene,
                )
                .unwrap();
        } else {
            cx.text_engine
                .draw_with_source_range_effects(
                    &run,
                    origin,
                    Color::WHITE,
                    LAYER_CONTENT,
                    &[],
                    cx.scene,
                )
                .unwrap();
        }
        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Move
            && phase == EventPhase::Target
        {
            let engine = cx.text_engine();
            let run = self.shape(engine);
            let geometry = engine
                .qualify_source_range(&run, range(TEXT, "NETTO"))
                .unwrap();
            let hit = Point::new(
                pointer.local_pos.0 - ORIGIN.x,
                pointer.local_pos.1 - ORIGIN.y,
            );
            let sample = geometry
                .clusters()
                .iter()
                .position(|cluster| cluster.ink_bounds().is_some_and(|rect| rect.contains(hit)));
            self.set_hover(sample.map(|index| [1.0, 1.05, 0.90, 1.15, 1.15][index]), cx);
        }
        false
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.set_hover(None, cx);
    }
}

fn compile(
    renderer: &mut WgpuRenderer,
    compositor: &Compositor,
    surface: SurfaceId,
    scale: f32,
    scene: SceneGraph,
) -> MultiTargetDrawPlan {
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &std::collections::HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| scale,
            0,
        )
        .unwrap();
    renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("source ink target")
}

fn scene(frame: &RetainedSceneTestFrame) -> SceneGraph {
    let mut builder = SceneBuilder::new();
    builder.push_retained_fragment(frame.scene_root.clone().into());
    builder.build()
}

fn qualified_rtl_scene(
    engine: &mut dyn TextEngine,
    style: &TextStyle,
    latin_style: &TextStyle,
) -> SceneGraph {
    let mut shape = |text| engine.shape_styled(text, Some(style), FONT_SIZE).unwrap();
    let combining = shape("Я\u{301}ё");
    let rtl = shape("سلام AV");
    let emoji = shape("A👩\u{200d}🔧B");
    let space = shape(" ");
    let kerned = shape("AV");
    let separate = shape("A").width + shape("V").width;
    assert!(
        kerned.width < separate,
        "chosen face must supply real pair kerning"
    );
    assert_eq!(
        engine.qualify_source_range(&combining, 0..2).unwrap_err(),
        TextRangeEffectError::SplitCluster
    );
    assert_eq!(
        engine.qualify_source_range(&combining, 0..1).unwrap_err(),
        TextRangeEffectError::InvalidRange
    );
    let accent = engine.qualify_source_range(&combining, 0..4).unwrap();
    assert_eq!(accent.clusters().len(), 1);
    assert_eq!(accent.clusters()[0].source(), 0..4);
    assert!(accent.clusters()[0].ink_bounds().is_some());

    let latin = engine
        .shape_styled("office", Some(latin_style), FONT_SIZE)
        .unwrap();
    assert!(
        latin
            .clusters
            .iter()
            .any(|cluster| cluster.source == (1..4))
    );
    assert_eq!(
        engine.qualify_source_range(&latin, 2..4).unwrap_err(),
        TextRangeEffectError::SplitCluster,
    );
    let complete_ligature = engine.qualify_source_range(&latin, 1..4).unwrap();
    assert_eq!(complete_ligature.clusters().len(), 1);
    assert_eq!(complete_ligature.clusters()[0].source(), 1..4);
    assert!(complete_ligature.ink_bounds().is_some());

    let ligature = range(&rtl.text, "لا");
    assert!(
        rtl.clusters
            .iter()
            .any(|cluster| cluster.source == ligature)
    );
    assert_eq!(
        engine
            .qualify_source_range(&rtl, ligature.start..ligature.start + "ل".len())
            .unwrap_err(),
        TextRangeEffectError::SplitCluster,
    );
    let geometry = engine
        .qualify_source_range(&rtl, 0..rtl.text.len())
        .unwrap();
    assert_eq!(
        geometry
            .clusters()
            .iter()
            .map(|cluster| cluster.source())
            .collect::<Vec<_>>(),
        rtl.clusters
            .iter()
            .map(|cluster| cluster.source.clone())
            .collect::<Vec<_>>(),
        "bound mixed-script visual order is preserved"
    );
    let later_source = range(&rtl.text, "AV");
    let ligature_ink = engine.qualify_source_range(&rtl, ligature.clone()).unwrap();
    let later = engine
        .qualify_source_range(&rtl, later_source.clone())
        .unwrap();
    assert!(ligature_ink.ink_bounds().unwrap().x > later.ink_bounds().unwrap().x);
    let joined = range(&emoji.text, "👩\u{200d}🔧");
    assert_eq!(
        engine
            .qualify_source_range(&emoji, joined.start..joined.start + "👩".len())
            .unwrap_err(),
        TextRangeEffectError::MissingGlyph,
    );
    let blank = engine.qualify_source_range(&space, 0..1).unwrap();
    assert_eq!(blank.source(), 0..1);
    assert_eq!(blank.clusters()[0].source(), 0..1);
    assert_eq!(blank.clusters()[0].ink_bounds(), None);
    assert_eq!(blank.ink_bounds(), None);
    assert_eq!(blank.pivot(), None);

    let mut builder = SceneBuilder::new();
    let mut unbound = rtl.clone();
    unbound.binding = None;
    assert_eq!(
        engine
            .qualify_source_range(&unbound, ligature.clone())
            .unwrap_err(),
        TextRangeEffectError::UnqualifiedRun,
    );
    let mut misplaced = rtl.clone();
    misplaced.glyphs[0].x += 1.0;
    assert_eq!(
        engine
            .qualify_source_range(&misplaced, ligature.clone())
            .unwrap_err(),
        TextRangeEffectError::UnqualifiedRun,
    );
    let legacy = engine.source_ink_geometry(&rtl, ligature.clone()).unwrap();
    assert_eq!(
        engine.draw_with_source_ink(
            &rtl.clone(),
            ORIGIN,
            Color::WHITE,
            LAYER_CONTENT,
            &[SourceInkEffect::try_new(&legacy, RED, 1.05).unwrap()],
            &mut builder,
        ),
        Err(TextRangeEffectError::StaleGeometry),
    );
    let bad = [
        (
            vec![
                effect(ligature.clone(), RED, 1.05),
                effect(ligature.clone(), GREEN, 1.0),
            ],
            TextRangeEffectError::OverlappingRanges,
        ),
        (
            vec![effect(ligature.clone(), RED, f32::NAN)],
            TextRangeEffectError::InvalidScale,
        ),
        (
            vec![effect(ligature.clone(), RED, f32::INFINITY)],
            TextRangeEffectError::InvalidScale,
        ),
        (
            vec![effect(ligature.clone(), RED, 0.89)],
            TextRangeEffectError::InvalidScale,
        ),
        (
            vec![effect(ligature.clone(), RED, 1.16)],
            TextRangeEffectError::InvalidScale,
        ),
        (
            vec![effect(ligature.clone(), RED.with_alpha(-0.25), 1.0)],
            TextRangeEffectError::InvalidColor,
        ),
    ];
    for (effects, error) in bad {
        assert_eq!(
            engine.draw_with_source_range_effects(
                &rtl,
                ORIGIN,
                Color::WHITE,
                LAYER_CONTENT,
                &effects,
                &mut builder,
            ),
            Err(error),
        );
    }
    assert_eq!(
        engine.draw_with_source_range_effects(
            &space,
            ORIGIN,
            Color::WHITE,
            LAYER_CONTENT,
            &[effect(0..1, RED, 0.90)],
            &mut builder,
        ),
        Err(TextRangeEffectError::NoInk),
    );
    assert!(builder.build().root.is_empty(), "refusal is atomic");

    let mut builder = SceneBuilder::new();
    engine
        .draw_with_source_range_effects(
            &rtl,
            ORIGIN,
            Color::WHITE,
            LAYER_CONTENT,
            &[
                effect(ligature, RED, 1.05),
                effect(later_source, GREEN, 1.0),
            ],
            &mut builder,
        )
        .unwrap();
    let scene = builder.build();
    assert_eq!(ui_typography_texts(&scene.root)[0].text, rtl.text);
    scene
}

fn ink_bounds(pixels: &[u8], stride: usize, channel: usize) -> (usize, usize, usize, usize, usize) {
    let (mut left, mut top, mut right, mut bottom, mut count) = (usize::MAX, usize::MAX, 0, 0, 0);
    for (index, p) in pixels.chunks_exact(4).enumerate() {
        if p[channel] > 120 && p[(channel + 1) % 3] < 10 && p[(channel + 2) % 3] < 10 {
            let (x, y) = (index % stride, index / stride);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
            count += 1;
        }
    }
    (left, top, right, bottom, count)
}

#[test]
fn text_source_ink_system_ui_heading_metal() {
    use crate::core::paint::{LocalAmbientGlowAppearance, LocalGlowLobe};
    use crate::platform::system_font_provider::load_default_system_ui_font;

    const HEADING: &str = "The Future, Rendered.";
    let source = 12..20;
    assert_eq!(&HEADING[source.clone()], "Rendered");
    let system = load_default_system_ui_font().expect("native SystemUI face required");
    let ivory = Color::from_hex(0xf6f0e5);
    let gold = Color::from_hex(0xf1d1a0);
    let origin = Point::new(28.0, 32.0);

    for (width, dpr) in [(960, 1.0_f32), (420, 2.0)] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(width, 160, dpr).expect("native Metal readback required");
        let info = renderer.gpu_resources().adapter.get_info();
        assert_eq!(info.backend, wgpu::Backend::Metal);

        let engine = renderer.text_engine_mut();
        engine.set_scale(dpr);
        let max_width = width as f32 - 2.0 * origin.x;
        let natural = engine.shape(HEADING, 68.0).width;
        let font_size = (68.0 * max_width / natural.max(1.0)).clamp(1.0, 68.0);
        assert_eq!(font_size == 68.0, dpr == 1.0);
        let run = engine.shape_styled(HEADING, None, font_size).unwrap();
        assert_eq!(run.text, HEADING);
        assert_eq!(run.binding.as_ref().unwrap().face().data(), &system.data);
        let qualified = engine.qualify_source_range(&run, source.clone()).unwrap();
        assert_eq!(qualified.clusters().len(), 8);
        let ink = qualified.ink_bounds().unwrap();
        let pivot = qualified.pivot().unwrap();
        let pivot = Point::new(origin.x + pivot.x, origin.y + pivot.y);
        let glow_rect = ink.translate(origin.x, origin.y);

        assert_eq!(run.clusters.len(), run.glyphs.len());
        let selected: Vec<_> = run
            .clusters
            .iter()
            .filter(|cluster| {
                !HEADING[cluster.source.clone()]
                    .chars()
                    .all(char::is_whitespace)
            })
            .map(|cluster| source.contains(&cluster.source.start))
            .collect();
        assert_eq!(selected.iter().filter(|&&selected| selected).count(), 8);

        let clip = (run.width > max_width + 0.5)
            .then_some(RectF::new(origin.x, origin.y, max_width, run.height));
        let render = |renderer: &mut WgpuRenderer,
                      mut scene: SceneBuilder,
                      effects: &[TextSourceRangeEffect]| {
            if let Some(clip) = clip {
                scene.begin_clip(clip, None);
            }
            renderer
                .text_engine_mut()
                .draw_with_source_range_effects(
                    &run,
                    origin,
                    ivory,
                    LAYER_CONTENT,
                    effects,
                    &mut scene,
                )
                .unwrap();
            if clip.is_some() {
                scene.end_clip();
            }
            let plan = compile(renderer, &compositor, surface, dpr, scene.build());
            assert!(
                plan.text_realization(surface)
                    .unwrap()
                    .outcome()
                    .is_complete()
            );
            let rgba = read_compiled_surface(renderer, &plan, surface).unwrap();
            (plan, rgba)
        };

        let (plain, baseline) = render(&mut renderer, SceneBuilder::new(), &[]);
        let original = &plain.surface_data[&surface].text_quads;
        assert_eq!(original.len(), selected.len());

        for (p, scale) in [(0.5_f32, 1.0), (1.0, 1.0), (1.0, 0.90), (1.0, 1.15)] {
            let t = p * p * (3.0 - 2.0 * p);
            let foreground = Color::from_linear(
                ivory.r + (gold.r - ivory.r) * t,
                ivory.g + (gold.g - ivory.g) * t,
                ivory.b + (gold.b - ivory.b) * t,
                ivory.a,
            );
            let mut scene = SceneBuilder::new();
            let glow_visible = p > 0.0 && p < 1.0;
            let glow_bounds = if glow_visible {
                let lobe = LocalGlowLobe::try_new(
                    Point::new(0.10 + 0.80 * p, 0.5),
                    Point::new(
                        (ink.height * 1.15).clamp(12.0, 48.0),
                        (ink.height * 0.78).clamp(10.0, 42.0),
                    ),
                    Point::new(1.0, 0.0),
                    Color::from_hex(0xf2c884),
                    0.32 * (std::f32::consts::PI * p).sin().powi(2),
                )
                .unwrap();
                let appearance = LocalAmbientGlowAppearance::try_new(&[lobe]).unwrap();
                scene
                    .push_local_ambient_glow(glow_rect, appearance, 0, None)
                    .unwrap();
                appearance.bounds(glow_rect).unwrap()
            } else {
                None
            };
            let (plan, rgba) = render(
                &mut renderer,
                scene,
                &[effect(source.clone(), foreground, scale)],
            );
            let painted = &plan.surface_data[&surface].text_quads;
            assert_eq!(painted.len(), original.len());
            let mut selected_rects = Vec::new();
            for ((before, after), &selected) in original.iter().zip(painted).zip(&selected) {
                assert_eq!(after.uv, before.uv);
                assert_eq!(after.clip_rect, before.clip_rect);
                assert_eq!(after.downsample_u, before.downsample_u);
                if selected {
                    assert_eq!(after.color, foreground.into());
                    selected_rects.extend([before.pos, after.pos]);
                } else {
                    assert_eq!(after.color, before.color);
                }
                if selected && scale != 1.0 {
                    assert!(
                        (after.pos.x - (pivot.x + (before.pos.x - pivot.x) * scale)).abs() < 0.0001
                    );
                    assert!(
                        (after.pos.y - (pivot.y + (before.pos.y - pivot.y) * scale)).abs() < 0.0001
                    );
                    assert!((after.pos.width - before.pos.width * scale).abs() < 0.0001);
                    assert!((after.pos.height - before.pos.height * scale).abs() < 0.0001);
                } else {
                    assert_eq!(after.pos, before.pos);
                }
            }

            let stride = (width as f32 * dpr) as usize;
            assert_eq!(rgba.len(), baseline.len());
            let (mut foreground_pixels, mut glow_pixels, mut unchanged_ink) = (0, 0, 0);
            for (index, (before, after)) in
                baseline.chunks_exact(4).zip(rgba.chunks_exact(4)).enumerate()
            {
                let point = Point::new(
                    ((index % stride) as f32 + 0.5) / dpr,
                    ((index / stride) as f32 + 0.5) / dpr,
                );
                if selected_rects.iter().any(|rect| rect.contains(point)) {
                    foreground_pixels += usize::from(before != after);
                } else if glow_bounds.is_some_and(|bounds| bounds.contains(point)) {
                    glow_pixels += usize::from(before[..3] == [0; 3] && after[..3] != [0; 3]);
                } else {
                    assert_eq!(before, after, "unchanged text or background at {point:?}");
                    unchanged_ink += usize::from(before[..3] != [0; 3]);
                }
            }
            assert!(foreground_pixels > 100 && unchanged_ink > 100);
            assert_eq!(glow_pixels > 100, glow_visible);
            println!(
                "SystemUI source ink on Metal {}: {dpr}x size={font_size:.3} p={p} scale={scale} changed foreground={foreground_pixels} glow-only={glow_pixels} unchanged ink={unchanged_ink}",
                info.name,
            );
        }
    }
}

fn wrapped_fallback_scenes(
    engine: &mut dyn TextEngine,
    style: &TextStyle,
    scale: f32,
) -> (SceneGraph, SceneGraph, f32) {
    let prefix = "NNNNNNNNNN ";
    let paragraph = Paragraph::new(format!("{prefix}AV ЖЖ NETTO"), style.clone());
    let selected = range(&paragraph.text, "V ЖЖ");
    let style_id = engine
        .resolve_text_style(style)
        .unwrap()
        .binding
        .face()
        .font_id();
    let catalog = engine.font_face_catalog().unwrap();
    let max_width = engine
        .shape_styled(prefix, Some(style), FONT_SIZE)
        .unwrap()
        .width
        + 0.05;
    let params = LayoutParams {
        max_width,
        scale,
        canonical_metrics: engine.metrics_styled(Some(style), FONT_SIZE).unwrap(),
    };
    let lines =
        ParagraphLayoutEngine::try_layout_with_catalog(&paragraph, &catalog, style_id, &params)
            .unwrap();
    assert_eq!(
        lines.len(),
        2,
        "the term must be on a genuinely wrapped second line"
    );
    let second_top = ORIGIN.y + lines[0].height;
    let mut prepared = Vec::new();
    let mut covered = Vec::new();
    let mut affected_faces = std::collections::HashSet::new();
    let mut line_top = ORIGIN.y;
    for (line_index, line) in lines.iter().enumerate() {
        let mut pen = ORIGIN.x;
        for original in &line.runs {
            let run =
                ParagraphLayoutEngine::shape_laid_out_run(&paragraph, original, engine).unwrap();
            let binding = run.binding.as_ref().unwrap();
            assert_eq!(binding.face().font_id(), original.font_id);
            assert_eq!(binding.cluster_gap(), original.style.cluster_gap);
            assert_eq!(run.text, &paragraph.text[original.range.clone()]);
            assert_eq!(run.width, original.width);
            for (glyph, original) in run.glyphs.iter().zip(&original.glyphs) {
                assert_eq!(glyph.id, original.glyph_id);
                assert_eq!(glyph.advance, original.advance);
            }
            assert!(
                run.clusters
                    .iter()
                    .all(|cluster| cluster.direction == TextDirection::LeftToRight)
            );
            let origin = Point::new(pen, line_top + line.baseline - run.ascent);
            assert!((origin.y + run.ascent - (line_top + line.baseline)).abs() < 0.0001);
            let start = selected.start.max(original.range.start);
            let end = selected.end.min(original.range.end);
            let decoration = if start < end {
                assert_eq!(line_index, 1);
                let local = (start - original.range.start)..(end - original.range.start);
                let geometry = engine.qualify_source_range(&run, local.clone()).unwrap();
                assert_eq!(geometry.source(), local);
                assert!(geometry.ink_bounds().is_some());
                covered.push(start..end);
                affected_faces.insert(original.font_id);
                Some(effect(
                    local,
                    if original.font_id == style_id {
                        RED
                    } else {
                        GREEN
                    },
                    1.05,
                ))
            } else {
                None
            };
            prepared.push((run, origin, decoration));
            pen += original.width;
        }
        assert!((pen - (ORIGIN.x + line.width)).abs() < 0.0001);
        assert!(line.width <= max_width + 0.001);
        line_top += line.height;
    }
    assert!(affected_faces.contains(&style_id) && affected_faces.len() >= 2);
    covered.sort_by_key(|r| r.start);
    let mut frontier = selected.start;
    for piece in covered {
        assert_eq!(piece.start, frontier);
        frontier = piece.end;
    }
    assert_eq!(
        frontier, selected.end,
        "qualify the entire annotation before painting"
    );

    let mut plain = SceneBuilder::new();
    let mut decorated = SceneBuilder::new();
    for (run, origin, effect) in prepared {
        engine.draw(
            &run,
            origin.x,
            origin.y,
            Color::WHITE,
            LAYER_CONTENT,
            &mut plain,
        );
        engine
            .draw_with_source_range_effects(
                &run,
                origin,
                Color::WHITE,
                LAYER_CONTENT,
                effect.as_slice(),
                &mut decorated,
            )
            .unwrap();
    }
    let (plain, decorated) = (plain.build(), decorated.build());
    let original = ui_typography_texts(&plain.root);
    let affected = ui_typography_texts(&decorated.root);
    assert_eq!(
        original.len(),
        lines.iter().map(|line| line.runs.len()).sum::<usize>()
    );
    assert_eq!(original.len(), affected.len());
    for (before, after) in original.iter().zip(&affected) {
        assert_eq!(before.text, after.text);
        assert_eq!(before.position, after.position);
        assert_eq!(before.font_size, after.font_size);
        assert_eq!(
            before.binding.as_ref().unwrap().face(),
            after.binding.as_ref().unwrap().face()
        );
    }
    (plain, decorated, second_top)
}

#[test]
fn text_source_range_native_ink_1x_2x() {
    for scale in [1.0, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(SIZE.width as u32, SIZE.height as u32, scale)
                .expect("native readback is required");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        let assets = [ui_typography_font_assets().remove(0), latin_ligature_font()];
        renderer.admit_font_assets(&assets).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let style = TextStyle::default()
            .with_family(assets[0].family_name.clone())
            .with_size(FONT_SIZE);
        let latin = TextStyle::default()
            .with_family(assets[1].family_name.clone())
            .with_weight(assets[1].weight)
            .with_italic(assets[1].italic)
            .with_size(FONT_SIZE);
        assert_eq!(
            renderer
                .text_engine_mut()
                .resolve_text_style(&latin)
                .unwrap()
                .binding
                .face()
                .data(),
            &assets[1].data,
            "the ligature fixture must use the admitted native face",
        );
        let rtl = qualified_rtl_scene(renderer.text_engine_mut(), &style, &latin);
        let compiled_rtl = compile(&mut renderer, &compositor, surface, scale, rtl);
        assert!(
            compiled_rtl
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let rtl_pixels = read_compiled_surface(&renderer, &compiled_rtl, surface).unwrap();
        let stride = (SIZE.width * scale) as usize;
        let rtl_red = ink_bounds(&rtl_pixels, stride, 0);
        let rtl_green = ink_bounds(&rtl_pixels, stride, 1);
        assert!(rtl_red.4 > 40 && rtl_green.4 > 40 && rtl_red.0 > rtl_green.2);
        let (fallback_plain, fallback_decorated, second_top) =
            wrapped_fallback_scenes(renderer.text_engine_mut(), &latin, scale);
        let compiled_plain = compile(&mut renderer, &compositor, surface, scale, fallback_plain);
        let plain_fallback_quads = compiled_plain.surface_data[&surface].text_quads.clone();
        let first_line_pixels = read_compiled_surface(&renderer, &compiled_plain, surface).unwrap();
        let compiled_fallback = compile(
            &mut renderer,
            &compositor,
            surface,
            scale,
            fallback_decorated,
        );
        assert!(
            compiled_fallback
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let fallback_quads = &compiled_fallback.surface_data[&surface].text_quads;
        assert_eq!(plain_fallback_quads.len(), fallback_quads.len());
        for (plain, colored) in plain_fallback_quads.iter().zip(fallback_quads) {
            if plain.color == colored.color {
                assert_eq!(plain.pos, colored.pos, "neighboring quad is identical");
            } else {
                assert!((plain.pos.width * 1.05 - colored.pos.width).abs() < 0.0001);
                assert!((plain.pos.height * 1.05 - colored.pos.height).abs() < 0.0001);
            }
            assert_eq!(plain.uv, colored.uv);
            assert_eq!(plain.clip_rect, colored.clip_rect);
        }
        let fallback_pixels =
            read_compiled_surface(&renderer, &compiled_fallback, surface).unwrap();
        let primary = ink_bounds(&fallback_pixels, stride, 0);
        let fallback = ink_bounds(&fallback_pixels, stride, 1);
        assert!(primary.4 > 30 && fallback.4 > 50);
        let first_rows = ((second_top - 1.0) * scale).floor() as usize * stride * 4;
        assert_eq!(
            &fallback_pixels[..first_rows],
            &first_line_pixels[..first_rows]
        );
        println!(
            "wrapped fallback Metal {scale}x: second-line selected pixels primary={} fallback={}",
            primary.4, fallback.4,
        );

        let layout_calls = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(InkTitle {
                id: WidgetInternalId::new(),
                style: style.clone(),
                hovered: None,
                layout_calls: Rc::clone(&layout_calls),
            }),
            None,
            0,
        );
        let theme = Theme::default();
        let icons = builtin_registry();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |renderer: &mut WgpuRenderer,
                     tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness| {
            harness.frame(
                tree,
                root,
                Constraints::tight(SIZE),
                &theme,
                renderer.text_engine_mut(),
                &icons,
            )
        };
        let initial = frame(&mut renderer, &mut tree, &mut harness);
        assert_eq!(layout_calls.get(), 1);
        let initial_rect = tree.get_rect(root).unwrap();
        let initial_text = ui_typography_texts(&initial.scene_root).pop().unwrap();
        assert_eq!(initial_text.text, TEXT);

        let draw = |renderer: &mut WgpuRenderer, frame: &RetainedSceneTestFrame| {
            let compiled = compile(renderer, &compositor, surface, scale, scene(frame));
            assert!(
                compiled
                    .text_realization(surface)
                    .unwrap()
                    .outcome()
                    .is_complete()
            );
            let quads = compiled.surface_data[&surface].text_quads.clone();
            let pixels = read_compiled_surface(renderer, &compiled, surface).unwrap();
            (quads, pixels)
        };
        let (plain_quads, plain) = draw(&mut renderer, &initial);
        let warm = renderer.text_rasterizations_for_test();
        assert_eq!(draw(&mut renderer, &initial).1, plain);
        // Existing ordinary spaces can be classified as no-ink each Compile.
        let ordinary_rasters = renderer.text_rasterizations_for_test() - warm;

        let engine = renderer.text_engine_mut();
        let run = engine.shape_styled(TEXT, Some(&style), FONT_SIZE).unwrap();
        let picked = engine
            .qualify_source_range(&run, range(TEXT, "NETTO"))
            .unwrap();
        assert_eq!(picked.clusters().len(), 5);
        let outline_bounds = picked.ink_bounds().unwrap();
        let pivot = picked.pivot().unwrap();
        assert_eq!(
            pivot,
            Point::new(
                outline_bounds.x + outline_bounds.width * 0.5,
                outline_bounds.y + outline_bounds.height * 0.5,
            )
        );
        let hit = |index: usize| {
            let b = picked.clusters()[index].ink_bounds().unwrap();
            Point::new(
                ORIGIN.x + b.x + b.width / 2.0,
                ORIGIN.y + b.y + b.height / 2.0,
            )
        };

        let mut route = |renderer: &mut WgpuRenderer, tree: &mut WidgetTree, pos: Point| {
            router.route_input_event(
                tree,
                FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    device: PointerDevice::Mouse,
                    delta: (0.0, 0.0),
                    buttons: Default::default(),
                },
                Some(root),
                renderer.text_engine_mut(),
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        };
        route(&mut renderer, &mut tree, hit(0));
        let tinted = frame(&mut renderer, &mut tree, &mut harness);
        let texts = ui_typography_texts(&tinted.scene_root);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].text, TEXT);
        assert_eq!(texts[0].font_size, initial_text.font_size);
        assert_eq!(texts[0].position, initial_text.position);
        assert!(texts[0].backgrounds.is_empty());
        let (tinted_quads, tinted_pixels) = draw(&mut renderer, &tinted);
        assert_eq!(plain_quads.len(), tinted_quads.len());
        for (normal, colored) in plain_quads.iter().zip(&tinted_quads) {
            assert_eq!(
                normal.pos, colored.pos,
                "scale 1 changes only the foreground"
            );
            assert_eq!(normal.uv, colored.uv);
            assert_eq!(normal.clip_rect, colored.clip_rect);
        }
        let red = ink_bounds(&tinted_pixels, (SIZE.width * scale) as usize, 0);
        let green = ink_bounds(&tinted_pixels, (SIZE.width * scale) as usize, 1);
        assert!(red.4 > 60 && green.4 > 60 && red.2 < green.0);

        route(&mut renderer, &mut tree, hit(1));
        let grown = frame(&mut renderer, &mut tree, &mut harness);
        let warm = renderer.text_rasterizations_for_test();
        let (grown_quads, grown_pixels) = draw(&mut renderer, &grown);
        assert_eq!(
            renderer.text_rasterizations_for_test() - warm,
            ordinary_rasters
        );
        assert_eq!(grown_quads.len(), tinted_quads.len());
        for (colored, enlarged) in tinted_quads.iter().zip(&grown_quads) {
            assert_eq!(colored.uv, enlarged.uv);
            assert_eq!(colored.clip_rect, enlarged.clip_rect);
            assert_eq!(colored.color, enlarged.color);
            if colored.color.r == 1.0 && colored.color.g == 0.0 {
                let pivot_x = ORIGIN.x + pivot.x;
                let pivot_y = ORIGIN.y + pivot.y;
                assert!(
                    (pivot_x + (colored.pos.x - pivot_x) * 1.05 - enlarged.pos.x).abs() < 0.0001
                );
                assert!(
                    (pivot_y + (colored.pos.y - pivot_y) * 1.05 - enlarged.pos.y).abs() < 0.0001
                );
                assert!((colored.pos.width * 1.05 - enlarged.pos.width).abs() < 0.0001);
                assert!((colored.pos.height * 1.05 - enlarged.pos.height).abs() < 0.0001);
            } else {
                assert_eq!(colored.pos, enlarged.pos);
            }
        }
        let larger_red = ink_bounds(&grown_pixels, stride, 0);
        assert!(larger_red.2 - larger_red.0 > red.2 - red.0 && larger_red.4 > red.4);
        for (index, (before, after)) in plain
            .chunks_exact(4)
            .zip(grown_pixels.chunks_exact(4))
            .enumerate()
        {
            let x = (index % stride) as f32 / scale;
            if x < ORIGIN.x + outline_bounds.x - 6.0 {
                assert_eq!(
                    before, after,
                    "the neighboring AV and its clipped edge remain exact"
                );
            }
            if x < CLIP.x || x >= CLIP.x + CLIP.width {
                assert_eq!(&after[..3], &[0, 0, 0]);
            }
        }

        route(&mut renderer, &mut tree, hit(2));
        let another_sample = frame(&mut renderer, &mut tree, &mut harness);
        let warm = renderer.text_rasterizations_for_test();
        let (_, another_pixels) = draw(&mut renderer, &another_sample);
        assert!(ink_bounds(&another_pixels, stride, 0).4 < red.4);
        assert_eq!(
            renderer.text_rasterizations_for_test() - warm,
            ordinary_rasters
        );
        route(&mut renderer, &mut tree, hit(3));
        let maximum_scale = frame(&mut renderer, &mut tree, &mut harness);
        let (_, maximum_pixels) = draw(&mut renderer, &maximum_scale);
        assert!(ink_bounds(&maximum_pixels, stride, 0).4 > larger_red.4);
        assert!(!frame(&mut renderer, &mut tree, &mut harness).scene_built);

        route(&mut renderer, &mut tree, Point::new(SIZE.width + 1.0, 20.0));
        let exited = frame(&mut renderer, &mut tree, &mut harness);
        assert_eq!(draw(&mut renderer, &exited).1, plain);
        assert_eq!(layout_calls.get(), 1);
        assert_eq!(tree.get_rect(root), Some(initial_rect));
        assert_eq!(domains.primary_state_mut().focus_manager.focused(), None);

        // Compile must independently reject the selected binding if the node's
        // text is replaced, even when the new text has valid byte boundaries.
        let mut foreign_text = texts[0].clone();
        foreign_text.text = TEXT.replace("NETTO", "OTHER");
        let mut foreign_direction = texts[0].clone();
        foreign_direction.binding = foreign_direction
            .binding
            .take()
            .map(|binding| binding.with_direction(TextDirection::RightToLeft));
        for tampered_text in [foreign_text, foreign_direction] {
            let mut tampered = SceneGraph::new();
            tampered.root.push(SceneNode::Text(tampered_text));
            let rejected = compile(&mut renderer, &compositor, surface, scale, tampered);
            assert!(matches!(
                rejected.text_realization(surface).unwrap().outcome(),
                TextRealizationOutcome::Unavailable(_)
            ));
            assert!(read_compiled_surface(&renderer, &rejected, surface).is_none());
        }
        println!(
            "source ink Metal {scale}x: NETTO red {} -> {} pixels, office green {}, no extra rasterizations",
            red.4, larger_red.4, green.4
        );
    }
}
