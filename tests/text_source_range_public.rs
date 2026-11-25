use halcyon::core::math::Point;
use halcyon::core::paint::{Color, LAYER_CONTENT};
use halcyon::core::paragraph::{LayoutParams, Paragraph, ParagraphLayoutEngine, TextStyle};
use halcyon::core::scene::{SceneBuilder, SceneNode};
use halcyon::core::text_engine::{
    QualifiedTextClusterRange, TextEngine, TextInputError, TextRangeEffectError,
    TextSourceRangeEffect,
};
use halcyon::platform::font_manager_text_engine::FontManagerTextEngine;

#[test]
fn text_source_range_public_complete_run() -> Result<(), TextRangeEffectError> {
    let mut backing = FontManagerTextEngine::new();
    let engine: &mut dyn TextEngine = &mut backing;
    let run = engine.shape("Price NETTO", 28.0);
    let origin = Point::new(6.0, 12.0);
    let geometry: QualifiedTextClusterRange = engine.qualify_source_range(&run, 6..11)?;
    assert_eq!(geometry.source(), 6..11);
    assert_eq!(geometry.clusters().len(), 5);
    for (cluster, range) in geometry.clusters().iter().zip((6..11).map(|i| i..i + 1)) {
        assert_eq!(cluster.source(), range);
        assert!(cluster.ink_bounds().is_some());
    }
    let pivot = geometry.pivot().unwrap();
    assert!(geometry.ink_bounds().unwrap().contains(pivot));
    let effect = TextSourceRangeEffect {
        source: geometry.source(),
        foreground: Color::from_hex(0xd5ac7a),
        scale: 1.10,
    };

    let mut scene = SceneBuilder::new();
    engine.draw_with_source_range_effects(
        &run,
        origin,
        Color::WHITE,
        LAYER_CONTENT,
        &[effect],
        &mut scene,
    )?;
    let graph = scene.build();
    let [SceneNode::Text(text)] = graph.root.as_slice() else {
        panic!("the public effect must preserve one complete logical TextNode");
    };
    assert_eq!(text.text, run.text);
    assert_eq!(text.font_size, run.font_size);
    assert_eq!(text.position, origin);
    assert_eq!(text.z_index, LAYER_CONTENT);
    assert_eq!(
        text.binding.as_ref().unwrap().face(),
        run.binding.as_ref().unwrap().face()
    );

    let mut ordinary = SceneBuilder::new();
    engine.draw(
        &run,
        origin.x,
        origin.y,
        Color::WHITE,
        LAYER_CONTENT,
        &mut ordinary,
    );
    let ordinary = ordinary.build();
    let mut empty = SceneBuilder::new();
    engine.draw_with_source_range_effects(
        &run,
        origin,
        Color::WHITE,
        LAYER_CONTENT,
        &[],
        &mut empty,
    )?;
    let empty = empty.build();
    let ([SceneNode::Text(ordinary)], [SceneNode::Text(empty)]) =
        (ordinary.root.as_slice(), empty.root.as_slice())
    else {
        panic!("ordinary node")
    };
    assert_eq!(empty.text, ordinary.text);
    assert_eq!(empty.binding, ordinary.binding);
    assert_eq!(empty.position, ordinary.position);
    assert_eq!(empty.font_size, ordinary.font_size);
    assert_eq!(empty.z_index, ordinary.z_index);
    assert_eq!(empty.color, ordinary.color);
    assert!(empty.backgrounds.is_empty() && ordinary.backgrounds.is_empty());
    Ok(())
}

#[test]
fn text_source_range_public_wrapped_paragraph_run() {
    let mut backing = FontManagerTextEngine::new();
    let engine: &mut dyn TextEngine = &mut backing;
    let style = TextStyle::default().with_size(24.0);
    let face = engine
        .resolve_text_style(&style)
        .unwrap()
        .binding
        .face()
        .font_id();
    let text = "First row. NETTO без налога";
    let paragraph = Paragraph::new(text, style.clone());
    let max_width = 120.0;
    let params = LayoutParams {
        max_width,
        scale: 1.0,
        canonical_metrics: engine.metrics_styled(Some(&style), 24.0).unwrap(),
    };
    let lines = ParagraphLayoutEngine::try_layout_with_catalog(
        &paragraph,
        &engine.font_face_catalog().unwrap(),
        face,
        &params,
    )
    .unwrap();
    assert!(lines.len() > 1);
    let start = text.find("NETTO").unwrap();
    let range = start..start + "NETTO".len();
    let (index, line, original) = lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| {
            line.runs
                .iter()
                .find(|run| run.range.start <= range.start && range.end <= run.range.end)
                .map(|run| (index, line, run))
        })
        .unwrap();
    assert!(index > 0, "the annotated term must be in a wrapped line");
    let run = ParagraphLayoutEngine::shape_laid_out_run(&paragraph, original, engine).unwrap();
    assert_eq!(run.text, &text[original.range.clone()]);
    assert_eq!(run.width, original.width);
    assert_eq!(
        run.binding.as_ref().unwrap().face().font_id(),
        original.font_id
    );
    let local = (range.start - original.range.start)..(range.end - original.range.start);
    let geometry = engine.qualify_source_range(&run, local.clone()).unwrap();
    assert_eq!(geometry.source(), local);
    assert_eq!(geometry.clusters().len(), 5);
    let origin = Point::new(2.0, line.baseline - run.ascent);
    let mut scene = SceneBuilder::new();
    engine
        .draw_with_source_range_effects(
            &run,
            origin,
            Color::WHITE,
            LAYER_CONTENT,
            &[TextSourceRangeEffect {
                source: local,
                foreground: Color::from_hex(0xc89e67),
                scale: 1.05,
            }],
            &mut scene,
        )
        .unwrap();
    assert!(matches!(&scene.build().root[..], [SceneNode::Text(node)] if node.text == run.text));
    let mut moved = original.clone();
    moved.glyphs[0].x_offset += 1.0;
    assert!(matches!(
        ParagraphLayoutEngine::shape_laid_out_run(&paragraph, &moved, engine),
        Err(TextInputError::UnsupportedTypography),
    ));
    let mut wrong_metrics = original.clone();
    wrong_metrics.metrics.line_height += 1.0;
    assert!(matches!(
        ParagraphLayoutEngine::shape_laid_out_run(&paragraph, &wrong_metrics, engine),
        Err(TextInputError::UnsupportedTypography),
    ));
}

#[cfg(target_os = "macos")]
fn system_font_asset(name: &str) -> halcyon::platform::font_manager::FontAsset {
    use halcyon::platform::font_manager::FontAsset;
    use halcyon::platform::font_manager_text_engine::FontIdentity;
    use std::sync::Arc;

    let data =
        Arc::new(std::fs::read(format!("/System/Library/Fonts/Supplemental/{name}.ttf")).unwrap());
    let identity = FontIdentity::from_font_data(&data, name);
    FontAsset {
        data,
        family_name: identity.family_name,
        weight: identity.weight,
        italic: identity.italic,
    }
}

#[cfg(target_os = "macos")]
#[test]
fn text_source_range_public_wrapped_fallback_runs() {
    use halcyon::core::paragraph::Span;
    use halcyon::core::text_shaper::TextDirection;
    use halcyon::platform::font_manager::FontManager;

    let mut primary_only = FontManager::new();
    let primary_asset = system_font_asset("Apple Chancery");
    let primary_style = TextStyle::default()
        .with_family(primary_asset.family_name.clone())
        .with_weight(primary_asset.weight)
        .with_italic(primary_asset.italic)
        .with_size(36.0);
    let primary = primary_only.register_font_asset(primary_asset).unwrap();
    let mut fonts = primary_only.clone();
    let fallback_asset = system_font_asset("Arial");
    let rtl_style = TextStyle::default()
        .with_family(fallback_asset.family_name.clone())
        .with_weight(fallback_asset.weight)
        .with_italic(fallback_asset.italic)
        .with_size(24.0);
    let fallback = fonts.register_font_asset(fallback_asset).unwrap();
    let catalog = fonts.face_catalog();
    let mut engine = FontManagerTextEngine::with_face_catalog(catalog.clone(), primary).unwrap();
    let mut without_fallback =
        FontManagerTextEngine::with_face_catalog(primary_only.face_catalog(), primary).unwrap();
    for scale in [1.0, 1.25, 1.5, 2.0] {
        engine.set_scale(scale);
        without_fallback.set_scale(scale);
        let lead = "NNNNNNNNNN ";
        let paragraph = Paragraph::new(format!("{lead}AV ЖЖ NETTO"), primary_style.clone());
        let width = engine
            .shape_styled(lead, Some(&primary_style), 36.0)
            .unwrap()
            .width
            + 0.05;
        let params = LayoutParams {
            max_width: width,
            scale: (&engine as &dyn TextEngine)
                .current_shaping_scale()
                .unwrap(),
            canonical_metrics: engine.metrics_styled(Some(&primary_style), 36.0).unwrap(),
        };
        let lines =
            ParagraphLayoutEngine::try_layout_with_catalog(&paragraph, &catalog, primary, &params)
                .unwrap();
        assert_eq!(lines.len(), 2);
        let start = paragraph.text.find("V ЖЖ").unwrap();
        let annotation = start..start + "V ЖЖ".len();
        let line_top = lines[0].height;
        let mut pen_x = 0.0;
        let mut prepared = Vec::new();
        for record in &lines[1].runs {
            // Keep the complete run's native ascent and the original line's baseline.
            let run =
                ParagraphLayoutEngine::shape_laid_out_run(&paragraph, record, &mut engine).unwrap();
            assert_eq!(run.text, &paragraph.text[record.range.clone()]);
            assert_eq!(
                run.binding.as_ref().unwrap().face().font_id(),
                record.font_id
            );
            let origin = Point::new(pen_x, line_top + lines[1].baseline - run.ascent);
            pen_x += record.width;
            let start = annotation.start.max(record.range.start);
            let end = annotation.end.min(record.range.end);
            let effect = if start < end {
                let local = (start - record.range.start)..(end - record.range.start);
                let geometry = engine.qualify_source_range(&run, local.clone()).unwrap();
                assert_eq!(geometry.source(), local);
                assert!(geometry.pivot().is_some());
                if record.font_id == fallback {
                    assert_eq!(
                        engine.qualify_source_range(&run, 0..1).unwrap_err(),
                        TextRangeEffectError::InvalidRange,
                    );
                    assert!(matches!(
                        ParagraphLayoutEngine::shape_laid_out_run(
                            &paragraph,
                            record,
                            &mut without_fallback
                        ),
                        Err(TextInputError::FontUnavailable),
                    ));
                }
                Some(TextSourceRangeEffect {
                    source: local,
                    foreground: Color::from_hex(0xc89e67),
                    scale: 1.05,
                })
            } else {
                None
            };
            prepared.push((run, origin, effect));
        }
        assert_eq!(
            prepared
                .iter()
                .filter(|(_, _, effect)| effect.is_some())
                .count(),
            2
        );
        let mut scene = SceneBuilder::new();
        for (run, origin, effect) in prepared {
            engine
                .draw_with_source_range_effects(
                    &run,
                    origin,
                    Color::WHITE,
                    LAYER_CONTENT,
                    effect.as_slice(),
                    &mut scene,
                )
                .unwrap();
        }
        assert_eq!(scene.build().root.len(), lines[1].runs.len());

        // The final Latin run belongs to an RTL span in an LTR paragraph.
        let rtl = "שלום AV";
        let text = format!("Start {rtl}");
        let other_style = rtl_style.clone().with_color(Color::from_hex(0xaabbcc));
        let paragraph = Paragraph::with_spans(
            text.clone(),
            vec![
                Span::new(0..6, other_style),
                Span::new(6..text.len(), rtl_style.clone()),
            ],
        );
        let label_width = engine
            .shape_styled("Start ", Some(&rtl_style), 24.0)
            .unwrap()
            .width;
        let rtl_width = engine
            .shape_styled("שלום ", Some(&rtl_style), 24.0)
            .unwrap()
            .width;
        let params = LayoutParams {
            max_width: label_width.max(rtl_width) + 0.05,
            ..params
        };
        let lines =
            ParagraphLayoutEngine::try_layout_with_catalog(&paragraph, &catalog, fallback, &params)
                .unwrap();
        let record = lines
            .iter()
            .flat_map(|line| &line.runs)
            .find(|run| &text[run.range.clone()] == "AV")
            .unwrap();
        let run =
            ParagraphLayoutEngine::shape_laid_out_run(&paragraph, record, &mut engine).unwrap();
        assert_eq!(run.clusters[0].source, 1..2);
        assert!(
            run.clusters
                .iter()
                .all(|cluster| cluster.direction == TextDirection::RightToLeft)
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn text_source_range_public_layout_reads_current_shaping_scale() {
    use halcyon::compositor::{Compositor, SceneKey};
    use halcyon::core::app::EngineTicker;
    use halcyon::core::layout::{Constraints, LayoutResult};
    use halcyon::core::math::{RectF, Size};
    use halcyon::core::paragraph::Line;
    use halcyon::core::rect_authority::LayoutAuthorityGuard;
    use halcyon::core::text_shaper::TextDirection;
    use halcyon::core::theme::Theme;
    use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
    use halcyon::core::widget_tree::{NodeId, WidgetTree};
    use halcyon::renderer::wgpu_renderer::WgpuRenderer;
    use halcyon::utils::id::WidgetInternalId;

    const LEAD: &str = "NNNNNNNNNN ";
    const MARKED: &str = "V ЖЖ";

    struct Heading {
        id: WidgetInternalId,
        paragraph: Paragraph,
        lines: Vec<Line>,
        scale: Option<f32>,
    }

    impl WidgetDowncast for Heading {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for Heading {
        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn layout(
            &mut self,
            cx: &mut LayoutContext,
            constraints: Constraints,
            _: &[NodeId],
        ) -> LayoutResult {
            let engine = &mut *cx.text_engine;
            let style = &self.paragraph.spans[0].style;
            let scale = engine.current_shaping_scale().unwrap();
            let face = engine
                .resolve_text_style(style)
                .unwrap()
                .binding
                .face()
                .font_id();
            let catalog = engine.font_face_catalog().unwrap();
            let lead = engine
                .shape_styled(LEAD, Some(style), style.font_size)
                .unwrap();
            let params = LayoutParams {
                max_width: (lead.width + 0.05).min(constraints.max_width),
                scale,
                canonical_metrics: engine.metrics_styled(Some(style), style.font_size).unwrap(),
            };
            self.lines = ParagraphLayoutEngine::try_layout_with_catalog(
                &self.paragraph,
                &catalog,
                face,
                &params,
            )
            .unwrap();
            self.scale = Some(scale);
            let width = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
            let height = self.lines.iter().map(|line| line.height).sum();
            LayoutResult::fit(constraints.constrain(Size::new(width, height)))
        }

        fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}
    }

    let primary = system_font_asset("Apple Chancery");
    let style = TextStyle::default()
        .with_family(primary.family_name.clone())
        .with_weight(primary.weight)
        .with_italic(primary.italic)
        .with_size(36.375)
        .with_cluster_gap(0.37)
        .unwrap();
    let text = format!("{LEAD}AV ЖЖ NETTO");
    let start = text.find(MARKED).unwrap();
    let annotation = start..start + MARKED.len();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(Heading {
            id: WidgetInternalId::new(),
            paragraph: Paragraph::new(text, style),
            lines: Vec::new(),
            scale: None,
        }),
        None,
        0,
    );
    let bounds = RectF::new(0.0, 0.0, 640.0, 240.0);
    let mut compositor = Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    renderer
        .admit_font_assets(&[primary, system_font_asset("Arial")])
        .unwrap();
    renderer
        .register_headless_render_target(surface, (640, 240), 1.0)
        .unwrap();
    let theme = Theme::dark();
    let mut ticker = EngineTicker::new(web_time::Instant::now());
    let mut previous: Option<Vec<Line>> = None;
    let mut refused_stale = false;
    for scale in [1.0_f32, 1.25, 1.5, 2.0] {
        // App calls this setter before Layout. The mounted widget receives only the trait object.
        ticker.begin_frame();
        renderer.set_scale_factor(surface, f64::from(scale));
        assert_eq!(renderer.scale_factor(surface), scale);
        let engine: &mut dyn TextEngine = renderer.text_engine_mut();
        {
            let _layout = LayoutAuthorityGuard::enter();
            let constraints =
                Constraints::loose(Size::new(bounds.width / scale, bounds.height / scale));
            LayoutContext::new(&mut tree, engine, &theme).layout_child(root, constraints);
        }
        let heading = tree
            .node(root)
            .unwrap()
            .widget()
            .as_any()
            .downcast_ref::<Heading>()
            .unwrap();
        assert_eq!(heading.scale, Some(scale));
        assert!(heading.lines.len() > 1, "heading must wrap at {scale}x");
        let primary_id = heading.lines[0].runs[0].font_id;
        let wrapped_selection = heading.lines.iter().skip(1).flat_map(|line| &line.runs);
        assert!(
            wrapped_selection.into_iter().any(|run| {
                run.font_id != primary_id
                    && run.range.start < annotation.end
                    && annotation.start < run.range.end
            }),
            "the wrapped annotation must cover a fallback face at {scale}x"
        );

        let mut line_top = 0.0;
        let mut prepared = Vec::new();
        for line in &heading.lines {
            let mut x = 0.0;
            for record in &line.runs {
                let run =
                    ParagraphLayoutEngine::shape_laid_out_run(&heading.paragraph, record, engine)
                        .unwrap();
                assert_eq!(run.text, heading.paragraph.text[record.range.clone()]);
                assert_eq!(run.width, record.width);
                assert_eq!(
                    run.binding.as_ref().unwrap().face().font_id(),
                    record.font_id
                );
                assert_eq!(run.glyphs.len(), record.glyphs.len());
                for (glyph, expected) in run.glyphs.iter().zip(&record.glyphs) {
                    assert_eq!(
                        (glyph.id, glyph.y, glyph.advance),
                        (expected.glyph_id, expected.y_offset, expected.advance)
                    );
                }
                assert!(
                    run.clusters
                        .iter()
                        .all(|cluster| cluster.direction == TextDirection::LeftToRight)
                );

                let start = record.range.start.max(annotation.start);
                let end = record.range.end.min(annotation.end);
                let effect = if start < end {
                    let local = (start - record.range.start)..(end - record.range.start);
                    let ink = engine.qualify_source_range(&run, local.clone()).unwrap();
                    assert_eq!(ink.source(), local);
                    assert!(ink.pivot().is_some());
                    Some(TextSourceRangeEffect {
                        source: local,
                        foreground: Color::from_hex(0xc89e67),
                        scale: 1.05,
                    })
                } else {
                    None
                };
                let origin = Point::new(x, line_top + line.baseline - run.ascent);
                prepared.push((run, origin, effect));
                x += record.width;
            }
            line_top += line.height;
        }
        assert_eq!(
            prepared
                .iter()
                .filter(|(_, _, effect)| effect.is_some())
                .count(),
            2
        );
        let mut scene = SceneBuilder::new();
        for (run, origin, effect) in &prepared {
            engine
                .draw_with_source_range_effects(
                    run,
                    *origin,
                    Color::WHITE,
                    LAYER_CONTENT,
                    effect.as_slice(),
                    &mut scene,
                )
                .unwrap();
        }
        let scene = scene.build();
        assert_eq!(scene.root.len(), prepared.len());
        for (node, (run, origin, effect)) in scene.root.iter().zip(&prepared) {
            let SceneNode::Text(node) = node else {
                panic!("expected one TextNode per original face run");
            };
            assert_eq!(
                (&node.text, node.position, node.font_size),
                (&run.text, *origin, run.font_size)
            );
            if effect.is_none() {
                assert_eq!(node.binding, run.binding);
                assert_eq!(node.color, Color::WHITE.into());
            }
        }

        if let Some(old_lines) = previous.replace(heading.lines.clone()) {
            for old in old_lines.iter().flat_map(|line| &line.runs) {
                if heading
                    .lines
                    .iter()
                    .flat_map(|line| &line.runs)
                    .any(|current| {
                        current.range == old.range
                            && current.font_id == old.font_id
                            && current.width != old.width
                    })
                {
                    assert!(matches!(
                        ParagraphLayoutEngine::shape_laid_out_run(&heading.paragraph, old, engine),
                        Err(TextInputError::UnsupportedTypography)
                    ));
                    refused_stale = true;
                }
            }
        }
    }
    assert!(
        refused_stale,
        "at least one prior scale produced a different run width"
    );

    let backend = renderer.text_engine_mut();
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        backend.set_scale(invalid);
        assert_eq!(
            (backend as &dyn TextEngine).current_shaping_scale(),
            Err(TextInputError::InvalidScale)
        );
    }
}

#[test]
fn text_source_range_public_shaping_scale_reports_typed_errors() {
    use halcyon::core::paint::{PaintSink, TextBackgroundSpan, ZIndex};
    use halcyon::core::text_engine::{GlyphRun, TextMetrics};

    struct ThirdPartyEngine;
    impl TextEngine for ThirdPartyEngine {
        fn measure(&mut self, _: &str, _: f32) -> (f32, f32) {
            unreachable!()
        }
        fn metrics(&mut self, _: &str, _: f32) -> TextMetrics {
            unreachable!()
        }
        fn shape(&mut self, _: &str, _: f32) -> GlyphRun {
            unreachable!()
        }
        fn draw(
            &mut self,
            _: &GlyphRun,
            _: f32,
            _: f32,
            _: Color,
            _: ZIndex,
            _: &mut dyn PaintSink,
        ) {
            unreachable!()
        }
        fn draw_with_backgrounds(
            &mut self,
            _: &GlyphRun,
            _: f32,
            _: f32,
            _: Color,
            _: Vec<TextBackgroundSpan>,
            _: ZIndex,
            _: &mut dyn PaintSink,
        ) {
            unreachable!()
        }
    }

    let unsupported: &dyn TextEngine = &ThirdPartyEngine;
    assert_eq!(
        unsupported.current_shaping_scale(),
        Err(TextInputError::UnsupportedTypography)
    );
    let mut font_manager = FontManagerTextEngine::new();
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        font_manager.set_scale(invalid);
        assert_eq!(
            (&font_manager as &dyn TextEngine).current_shaping_scale(),
            Err(TextInputError::InvalidScale)
        );
    }
}
