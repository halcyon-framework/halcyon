use super::*;
use crate::compositor::SceneKey;
use crate::core::app::{AppContext, settle_ui_scale_for_frame};
use crate::core::input_fusion::InputFusion;
use crate::core::paragraph::Line;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::widgets::text_block::TextBlock;

const LEAD: &str = "NNNNNNNNNN ";
const INITIAL_SIZE: Size = Size::new(400.0, 192.0);
const INSET: f32 = 8.0;

struct Column(WidgetInternalId);

impl WidgetDowncast for Column {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for Column {
    fn id(&self) -> WidgetInternalId {
        self.0
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width - 2.0 * INSET;
        let child_constraints = Constraints::loose(Size::new(width, constraints.max_height));
        let mut y = INSET;
        for &child in children {
            let size = cx.layout_child(child, child_constraints);
            cx.set_child_rect(child, RectF::new(INSET, y, size.width, size.height));
            y += size.height + INSET;
        }
        LayoutResult::fit(constraints.constrain(Size::new(constraints.max_width, y)))
    }

    fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}
}

struct Heading {
    id: WidgetInternalId,
    paragraph: Paragraph,
    selected: Range<usize>,
    lines: Vec<Line>,
    ink: Vec<(Range<usize>, RectF)>,
    scale: f32,
    layouts: usize,
    hit: Option<Range<usize>>,
}

impl WidgetDowncast for Heading {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Heading {
    fn visit_runs(
        &self,
        engine: &mut dyn TextEngine,
        mut visit: impl FnMut(&mut dyn TextEngine, GlyphRun, Point, Range<usize>, Option<Range<usize>>),
    ) {
        let mut top = 0.0;
        for (line_index, line) in self.lines.iter().enumerate() {
            let mut x = 0.0;
            for original in &line.runs {
                let run =
                    ParagraphLayoutEngine::shape_laid_out_run(&self.paragraph, original, engine)
                        .expect("the first successor frame must use current paragraph layout");
                assert_eq!(run.text, self.paragraph.text[original.range.clone()]);
                assert_eq!(run.width, original.width);
                assert_eq!(
                    run.binding.as_ref().unwrap().face().font_id(),
                    original.font_id
                );
                let y = top + line.baseline - run.ascent;
                assert!((y + run.ascent - (top + line.baseline)).abs() < 0.001);
                let (start, end) = (
                    self.selected.start.max(original.range.start),
                    self.selected.end.min(original.range.end),
                );
                let local =
                    (start < end).then(|| start - original.range.start..end - original.range.start);
                if local.is_some() {
                    assert_eq!(line_index, 1);
                }
                visit(engine, run, Point::new(x, y), original.range.clone(), local);
                x += original.width;
            }
            assert!((x - line.width).abs() < 0.001);
            top += line.height;
        }
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
        let primary = engine
            .resolve_text_style(style)
            .unwrap()
            .binding
            .face()
            .font_id();
        let lead = engine
            .shape_styled(LEAD, Some(style), style.font_size)
            .unwrap();
        self.scale = engine.current_shaping_scale().unwrap();
        self.layouts += 1;
        self.lines = ParagraphLayoutEngine::try_layout_with_catalog(
            &self.paragraph,
            &engine.font_face_catalog().unwrap(),
            primary,
            &LayoutParams {
                max_width: constraints.max_width.min(lead.width + 0.05),
                scale: self.scale,
                canonical_metrics: engine.metrics_styled(Some(style), style.font_size).unwrap(),
            },
        )
        .unwrap();
        let mut ink = Vec::new();
        self.visit_runs(engine, |engine, run, origin, original, local| {
            if let Some(local) = local {
                let qualified = engine.qualify_source_range(&run, local).unwrap();
                ink.extend(qualified.clusters().iter().filter_map(|cluster| {
                    let source = cluster.source();
                    let bounds = cluster.ink_bounds()?.translate(origin.x, origin.y);
                    Some((
                        original.start + source.start..original.start + source.end,
                        bounds,
                    ))
                }));
            }
        });
        self.ink = ink;
        let width = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = self.lines.iter().map(|line| line.height).sum();
        LayoutResult::fit(constraints.constrain(Size::new(width, height)))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _: &[NodeId]) {
        assert_eq!(
            cx.text_engine.current_shaping_scale().unwrap(),
            self.scale,
            "Paint reached cached Layout from a different shaping scale"
        );
        let primary = cx
            .text_engine
            .resolve_text_style(&self.paragraph.spans[0].style)
            .unwrap()
            .binding
            .face()
            .font_id();
        self.visit_runs(cx.text_engine, |engine, run, origin, _, local| {
            let effects = local.map(|source| {
                let color = if run.binding.as_ref().unwrap().face().font_id() == primary {
                    RED
                } else {
                    GREEN
                };
                effect(source, color, 1.05)
            });
            engine
                .draw_with_source_range_effects(
                    &run,
                    Point::new(rect.x + origin.x, rect.y + origin.y),
                    Color::WHITE,
                    LAYER_CONTENT,
                    effects.as_slice(),
                    cx.scene,
                )
                .unwrap();
        });
    }

    fn handle_event(&mut self, _: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if let Event::Pointer(pointer) = event
            && pointer.kind == PointerEventKind::Move
            && phase == EventPhase::Target
        {
            self.hit = self.ink.iter().find_map(|(source, bounds)| {
                bounds
                    .contains_point(pointer.local_pos.0, pointer.local_pos.1)
                    .then(|| source.clone())
            });
        }
        false
    }
}

fn heading(tree: &WidgetTree, node: NodeId) -> &Heading {
    tree.node(node)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref()
        .unwrap()
}

fn physical(size: Size, scale: f32) -> (u32, u32) {
    ((size.width * scale) as u32, (size.height * scale) as u32)
}

#[test]
fn ui_scale_settlement_refreshes_mounted_source_ink_at_equal_constraints() {
    let (mut renderer, mut compositor, surface) =
        setup_with_size_and_scale(INITIAL_SIZE.width as u32, INITIAL_SIZE.height as u32, 1.0)
            .expect("native GPU required");
    #[cfg(target_os = "macos")]
    assert_eq!(
        renderer.headless_adapter_for_test().backend,
        wgpu::Backend::Metal
    );
    let auxiliary_bounds = RectF::new(0.0, 0.0, 96.0, 40.0);
    let auxiliary = compositor.create_surface(1, auxiliary_bounds, SceneKey::BASE);
    renderer
        .register_headless_render_target(auxiliary, (192, 80), 2.0)
        .unwrap();

    let primary_asset = latin_ligature_font();
    let style = TextStyle::default()
        .with_family(primary_asset.family_name.clone())
        .with_weight(primary_asset.weight)
        .with_italic(primary_asset.italic)
        .with_size(36.375)
        .with_cluster_gap(0.37)
        .unwrap();
    renderer
        .admit_font_assets(&[primary_asset, ui_typography_font_assets().remove(0)])
        .unwrap();
    let primary_id = renderer
        .text_engine_mut()
        .resolve_text_style(&style)
        .unwrap()
        .binding
        .face()
        .font_id();
    let mut font_manager =
        FontManagerTextEngine::with_face_catalog(renderer.font_face_catalog(), primary_id).unwrap();
    let text = format!("{LEAD}AV ЖЖ NETTO");
    let selected = range(&text, "V ЖЖ");
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Column(WidgetInternalId::new())), None, 0);
    let parent = tree.add_child(root, Box::new(Column(WidgetInternalId::new())), None, 0);
    let title = tree.add_child(
        parent,
        Box::new(Heading {
            id: WidgetInternalId::new(),
            paragraph: Paragraph::new(text, style),
            selected,
            lines: Vec::new(),
            ink: Vec::new(),
            scale: 0.0,
            layouts: 0,
            hit: None,
        }),
        None,
        0,
    );
    let neighbor = tree.add_child(parent, Box::new(TextBlock::new("Neighbor")), None, 1);
    let title_mount = tree.node(title).unwrap().id;
    let mut harness = RetainedSceneTestHarness::new();
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let mut fusion = InputFusion::new();
    let mut app = AppContext::new(Theme::dark());
    let icons = builtin_registry();
    let mut bindings = BindingRegistry::new();
    let mut old_lines: Option<Vec<Line>> = None;
    let mut previous_title_constraints = None;
    let mut previous_neighbor_epoch = 0;

    for (index, (scale, logical)) in [
        (1.25_f32, INITIAL_SIZE),
        (1.5, INITIAL_SIZE),
        (1.25, INITIAL_SIZE),
        (1.0, INITIAL_SIZE),
        (1.5, Size::new(360.0, 176.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let extent = physical(logical, scale);
        renderer.resize_render_target(surface, extent).unwrap();
        let auxiliary_scale = if index == 0 { 2.0 } else { 1.75 };
        let auxiliary_extent = physical(auxiliary_bounds.size(), auxiliary_scale);
        if index == 1 {
            renderer
                .resize_render_target(auxiliary, auxiliary_extent)
                .unwrap();
        }
        let targets = [
            (surface, scale, extent),
            (auxiliary, auxiliary_scale, auxiliary_extent),
        ];
        let prepared = settle_ui_scale_for_frame(
            &mut app,
            &mut tree,
            &mut router,
            domains.primary_state_mut(),
            &mut fusion,
            root,
            renderer.text_engine_mut().scale(),
            &targets,
            true,
        )
        .unwrap()
        .unwrap();
        let viewport = RectF::new(0.0, 0.0, logical.width, logical.height);
        assert_eq!(
            prepared,
            [
                (surface, scale, viewport),
                (auxiliary, auxiliary_scale, auxiliary_bounds)
            ]
        );
        for (id, scale, bounds) in prepared {
            renderer.set_target_scale_factor(id, scale, id == surface);
            compositor.set_surface_bounds(id, bounds);
        }
        compositor.set_viewport(viewport);
        assert_eq!(
            renderer.text_engine_mut().current_shaping_scale(),
            Ok(scale)
        );
        font_manager.set_scale(scale);
        if index == 1 {
            let original = heading(&tree, title);
            assert!(
                old_lines
                    .as_ref()
                    .unwrap()
                    .iter()
                    .flat_map(|line| &line.runs)
                    .any(|run| {
                        ParagraphLayoutEngine::shape_laid_out_run(
                            &original.paragraph,
                            run,
                            renderer.text_engine_mut(),
                        )
                        .is_err()
                    }),
                "the old 1.25 layout must expose the real 1.5 reconstruction failure"
            );
        }
        let constraints = Constraints::loose(logical);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let frame = harness.frame_with_optional_custom_render_target(
            &mut tree,
            root,
            constraints,
            &app.theme,
            renderer.text_engine_mut(),
            &icons,
            Some(target),
            Some((&domains, WindowId::PRIMARY)),
        );
        assert!(frame.scene_built);
        assert_eq!(tree.node(title).unwrap().id, title_mount);
        assert_eq!(tree.node(root).unwrap().last_constraints, Some(constraints));
        if (1..4).contains(&index) {
            assert_eq!(
                tree.node(title).unwrap().last_constraints,
                previous_title_constraints
            );
        }
        previous_title_constraints = tree.node(title).unwrap().last_constraints;
        let neighbor_epoch = tree.node(neighbor).unwrap().layout_epoch;
        assert!(neighbor_epoch > previous_neighbor_epoch);
        previous_neighbor_epoch = neighbor_epoch;
        let title_rect = tree.get_absolute_paint_rect(title).unwrap();
        let neighbor_rect = tree.get_absolute_paint_rect(neighbor).unwrap();
        assert_eq!(title_rect.x, neighbor_rect.x);
        assert!((neighbor_rect.y - title_rect.bottom() - INSET).abs() < 0.001);
        let content = heading(&tree, title);
        assert_eq!(content.layouts, index + 1);
        assert_eq!(content.scale, scale);
        assert_eq!(content.lines.len(), 2);
        let second = &content.lines[1];
        assert!(second.runs.iter().any(|run| run.font_id == primary_id));
        assert!(second.runs.iter().any(|run| run.font_id != primary_id));
        let texts = ui_typography_texts(&frame.scene_root);
        let mut title_texts = texts.iter().filter(|text| text.text != "Neighbor");
        content.visit_runs(&mut font_manager, |_, run, origin, _, local| {
            let node = title_texts.next().unwrap();
            assert_eq!(node.text, run.text);
            let binding = node.binding.as_ref().unwrap();
            assert!(
                binding.shape_binding().as_ref() == run.binding.as_ref().unwrap(),
                "scene shaping binding for {:?}",
                run.text
            );
            assert_eq!(binding.source_ink().is_some(), local.is_some());
            assert_eq!(
                node.position,
                Point::new(title_rect.x + origin.x, title_rect.y + origin.y)
            );
        });
        assert!(title_texts.next().is_none());
        assert!(texts.iter().any(|text| text.text == "Neighbor"));
        old_lines = Some(content.lines.clone());
        let picks: Vec<_> = content
            .ink
            .iter()
            .filter(|(source, _)| {
                let part = &content.paragraph.text[source.clone()];
                part == "V" || part == "Ж"
            })
            .cloned()
            .collect();
        assert_eq!(picks.len(), 3);

        let mut auxiliary_scene = SceneBuilder::new();
        let engine = renderer.text_engine_mut();
        let run = engine.shape("Auxiliary", 16.0);
        engine.draw(
            &run,
            8.0,
            8.0,
            Color::WHITE,
            LAYER_CONTENT,
            &mut auxiliary_scene,
        );
        let scenes = std::collections::HashMap::from([
            (surface, scene(&frame)),
            (auxiliary, auxiliary_scene.build()),
        ]);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |id| renderer.scale_factor(id),
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("both current targets");
        assert_eq!(compiled.get_target(surface).unwrap().physical_size, extent);
        assert_eq!(
            compiled.get_target(auxiliary).unwrap().physical_size,
            auxiliary_extent
        );
        assert_eq!(
            renderer.text_engine_mut().current_shaping_scale(),
            Ok(scale)
        );
        assert!(
            compiled
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let pixels = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        assert!(
            compiled
                .text_realization(auxiliary)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let auxiliary_pixels = read_compiled_surface(&renderer, &compiled, auxiliary).unwrap();
        assert_eq!(
            auxiliary_pixels.len(),
            auxiliary_extent.0 as usize * auxiliary_extent.1 as usize * 4
        );
        assert_eq!(pixels.len(), extent.0 as usize * extent.1 as usize * 4);
        let primary_ink = ink_bounds(&pixels, extent.0 as usize, 0);
        let fallback_ink = ink_bounds(&pixels, extent.0 as usize, 1);
        assert!(
            primary_ink.4 > 20 && fallback_ink.4 > 20 && primary_ink.0 < fallback_ink.0,
            "both selected face runs must reach the actual target: {primary_ink:?}, {fallback_ink:?}"
        );
        let rasters = renderer.text_rasterizations_for_test();
        assert!(rasters > 0);
        let clean = harness.frame_with_optional_custom_render_target(
            &mut tree,
            root,
            constraints,
            &app.theme,
            renderer.text_engine_mut(),
            &icons,
            Some(target),
            Some((&domains, WindowId::PRIMARY)),
        );
        assert!(!clean.scene_built);
        assert_eq!(heading(&tree, title).layouts, index + 1);
        assert_eq!(tree.node(neighbor).unwrap().layout_epoch, neighbor_epoch);
        assert_eq!(renderer.text_rasterizations_for_test(), rasters);

        for (source, rect) in picks {
            let pos = Point::new(
                title_rect.x + rect.x + rect.width * 0.5,
                title_rect.y + rect.y + rect.height * 0.5,
            );
            router.route_input_event(
                &mut tree,
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
            assert_eq!(heading(&tree, title).hit, Some(source));
        }
        eprintln!(
            "mounted source ink {scale}x {extent:?}: {} primary and {} fallback pixels",
            primary_ink.4, fallback_ink.4
        );
    }
}
