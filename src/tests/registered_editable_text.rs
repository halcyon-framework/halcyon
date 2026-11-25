//! HAL049 §18: the ordinary editable cache, retained scene and real raster agree.
#![cfg(all(target_os = "macos", debug_assertions))]

use super::renderer_z_order::{read_compiled_surface, setup_with_size_and_scale};
use crate::{
    core::{
        app::RetainedSceneTestHarness,
        event::{
            FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PhysicalKeyCode,
            PointerButton, PointerDevice, WindowId,
        },
        event_router::EventRouter,
        icon::IconRegistry,
        layout::Constraints,
        math::{Point, Size},
        paint::Color,
        scene::{Color as SceneColor, SceneGraph, SceneNode, TextNode},
        text_engine::{GlyphRun, ResolvedTextStyle, TextEngine, TextInputError, TextMetrics},
        theme::Theme,
        widget_tree::{NodeId, WidgetTree},
    },
    platform::font_manager::{FontAsset, FontManager, FontWeight},
    renderer::{DrawPlanBuilder, TextRealizationOutcome, mesh_cache::MeshCache},
    runtime::binding_registry::BindingRegistry,
    widgets::{
        text_area::TextArea,
        text_field::{TextField, TextFieldHandoffError, TextFieldHandoffStatus},
        text_layout_cache::TextLayoutCache,
    },
};
use std::{collections::HashMap, path::PathBuf};

const REQUEST: &str = "Reply with the single Chinese character 看.";

fn supplemental_asset() -> FontAsset {
    asset("Arial Unicode.ttf", "Arial Unicode MS")
}

fn asset(file: &str, expected_family: &str) -> FontAsset {
    let mut manager = FontManager::new();
    let id = manager
        .load_font_file(&format!("/System/Library/Fonts/Supplemental/{file}"))
        .unwrap();
    let face = manager.get_face(id).unwrap();
    let parsed = ttf_parser::Face::parse(&face.data, 0).unwrap();
    let family = parsed
        .names()
        .into_iter()
        .filter(|name| name.name_id == ttf_parser::name_id::FAMILY)
        .find_map(|name| name.to_string())
        .unwrap();
    assert_eq!(family, expected_family);
    assert_eq!(
        FontWeight::from_value(parsed.weight().to_number()),
        FontWeight::Regular
    );
    assert!(!parsed.is_italic());
    if expected_family == "Arial Unicode MS" {
        assert_ne!(parsed.glyph_index('看').unwrap().0, 0);
    }
    FontAsset {
        family_name: family,
        weight: face.weight,
        italic: face.italic,
        data: face.data.clone(),
    }
}

#[derive(Default)]
struct Input {
    router: EventRouter,
    domains: InteractionDomainState,
    bindings: BindingRegistry,
}
impl Input {
    fn send(
        &mut self,
        tree: &mut WidgetTree,
        root: NodeId,
        engine: &mut dyn TextEngine,
        event: FusedInputEvent,
    ) {
        self.router.route_input_event(
            tree,
            event,
            Some(root),
            engine,
            None,
            &mut self.domains,
            &mut self.bindings,
        );
    }
    fn click(
        &mut self,
        tree: &mut WidgetTree,
        root: NodeId,
        engine: &mut dyn TextEngine,
        pos: Point,
    ) {
        self.send(
            tree,
            root,
            engine,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
        );
        self.send(
            tree,
            root,
            engine,
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        );
    }
    fn key(
        &mut self,
        tree: &mut WidgetTree,
        root: NodeId,
        engine: &mut dyn TextEngine,
        key_code: KeyCode,
        modifiers: Modifiers,
    ) {
        self.send(
            tree,
            root,
            engine,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                key_code,
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers,
                repeat: false,
            },
        );
    }
    fn text(
        &mut self,
        tree: &mut WidgetTree,
        root: NodeId,
        engine: &mut dyn TextEngine,
        text: &str,
    ) {
        self.send(
            tree,
            root,
            engine,
            FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                text: text.into(),
                modifiers: Modifiers::default(),
            },
        );
    }
}

fn render_frame(
    renderer: &mut crate::renderer::wgpu_renderer::WgpuRenderer,
    compositor: &crate::compositor::Compositor,
    surface: crate::compositor::SurfaceId,
    scale: f32,
    frame: &crate::core::app::RetainedSceneTestFrame,
) -> (TextRealizationOutcome, Vec<u8>) {
    let mut scene = SceneGraph::new();
    scene.root = frame.scene_root.clone();
    scene.clear_color = SceneColor::TRANSPARENT;
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &HashMap::from([(surface, scene)]),
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| scale,
            0,
        )
        .unwrap();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    let outcome = compiled
        .text_realization(surface)
        .unwrap()
        .outcome()
        .clone();
    (
        outcome,
        read_compiled_surface(renderer, &compiled, surface).unwrap(),
    )
}

#[test]
fn registered_editable_cache_identity_clusters_and_atomic_refusal() {
    use crate::core::{editing::ClusterBoundary, paragraph::TextStyle};
    let (mut renderer, _, _) = setup_with_size_and_scale(200, 120, 1.0).unwrap();
    renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
    let source = "A Ж 看 e\u{301} Z";
    let mut cache = TextLayoutCache::new(13.0, Color::WHITE);
    cache
        .try_rebuild(source, 7, 100_000.0, renderer.text_engine_mut())
        .unwrap();
    assert!(
        !cache
            .typography_changed(renderer.text_engine_mut())
            .unwrap()
    );
    let initial_revision = cache.typography_revision();
    let primary = renderer
        .text_engine_mut()
        .resolve_text_style(&TextStyle::default().with_size(13.0))
        .unwrap()
        .binding;
    assert_eq!(
        cache.lines()[0].runs[0].font_id,
        primary.face().font_id(),
        "primary-supported prefix remains primary"
    );
    assert!(
        cache.lines()[0]
            .runs
            .iter()
            .any(|run| run.font_id != primary.face().font_id())
    );
    let boundaries = ClusterBoundary::build_from_lines(cache.lines(), source);
    let tuple = |b: &ClusterBoundary| {
        (
            b.byte_start,
            b.byte_end,
            b.line_index,
            b.x_logical.to_bits(),
        )
    };
    assert_eq!(
        cache.boundaries().iter().map(tuple).collect::<Vec<_>>(),
        boundaries.iter().map(tuple).collect::<Vec<_>>()
    );
    let combining_inside = source.find('e').unwrap() + 1;
    assert!(
        !cache
            .boundaries()
            .iter()
            .any(|b| b.byte_start == combining_inside),
        "scalar boundary inside genuine shaped cluster is not editable"
    );
    for run in &cache.lines()[0].runs {
        let shaped = cache
            .shape_run(source, run, renderer.text_engine_mut())
            .unwrap();
        assert_eq!(
            shaped.glyphs.iter().map(|g| g.id).collect::<Vec<_>>(),
            run.glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>()
        );
        assert!((shaped.width - run.width).abs() < 0.001);
    }
    assert_eq!(
        cache.try_rebuild("not published", 8, f32::NAN, renderer.text_engine_mut()),
        Err(TextInputError::PositionOverflow)
    );
    assert_eq!(cache.cached_doc_len(), source.len());
    assert_eq!(cache.typography_revision(), initial_revision);
    assert!(!cache.needs_rebuild(7, 100_000.0));
    // Advertised but contradictory catalog must not silently use a valid default
    // shaper. The actual backing remains the same native engine underneath.
    let mut invalid = InvalidCatalog(renderer.text_engine_mut());
    assert_eq!(
        cache.try_rebuild("not published", 8, 100_000.0, &mut invalid),
        Err(TextInputError::FontUnavailable)
    );
    assert_eq!(cache.cached_doc_len(), source.len());
    renderer
        .admit_font_assets(&[asset("Arial.ttf", "Arial")])
        .unwrap();
    assert!(
        cache
            .typography_changed(renderer.text_engine_mut())
            .unwrap()
    );
    cache
        .try_rebuild(source, 7, 100_000.0, renderer.text_engine_mut())
        .unwrap();
    assert!(cache.typography_revision() > initial_revision);
    let revision = cache.typography_revision();
    cache.rebuild(source, 7, 100_000.0, renderer.text_engine_mut());
    assert_eq!(
        cache.typography_revision(),
        revision,
        "equal typography does not restart qualified motion"
    );
    assert!(
        cache.needs_rebuild(7, 99_999.95),
        "actual width is part of eligibility, even below the old tolerance"
    );
    cache.set_font_size(13.0001);
    assert!(cache.needs_rebuild(7, 100_000.0));
    cache.rebuild(source, 7, 100_000.0, renderer.text_engine_mut());
    cache.set_caret_thickness(3.0);
    assert!(cache.needs_rebuild(7, 100_000.0));
    cache.rebuild(source, 7, 100_000.0, renderer.text_engine_mut());
    assert!((cache.content_width() - cache.lines()[0].width - 3.0).abs() < 0.001);
    cache.set_font_size(19.0);
    assert!(cache.needs_rebuild(7, 100_000.0));
    cache.rebuild(source, 7, 65.0, renderer.text_engine_mut());
    assert!(cache.line_count() > 1);
}

struct InvalidCatalog<'a>(&'a mut dyn TextEngine);
impl TextEngine for InvalidCatalog<'_> {
    fn font_face_catalog(&self) -> Option<crate::platform::font_manager::FontFaceCatalog> {
        Some(Default::default())
    }
    fn resolve_text_style(
        &self,
        style: &crate::core::paragraph::TextStyle,
    ) -> Result<ResolvedTextStyle, TextInputError> {
        self.0.resolve_text_style(style)
    }
    fn measure(&mut self, s: &str, size: f32) -> (f32, f32) {
        self.0.measure(s, size)
    }
    fn metrics(&mut self, s: &str, size: f32) -> TextMetrics {
        self.0.metrics(s, size)
    }
    fn shape(&mut self, s: &str, size: f32) -> GlyphRun {
        self.0.shape(s, size)
    }
    fn draw(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        z: u32,
        sink: &mut dyn crate::core::paint::PaintSink,
    ) {
        self.0.draw(run, x, y, color, z, sink)
    }
    fn draw_with_backgrounds(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        backgrounds: Vec<crate::core::paint::TextBackgroundSpan>,
        z: u32,
        sink: &mut dyn crate::core::paint::PaintSink,
    ) {
        self.0
            .draw_with_backgrounds(run, x, y, color, backgrounds, z, sink)
    }
}

#[test]
fn registered_editable_reverse_selection_hit_handoff_and_continued_editing() {
    let (mut renderer, compositor, surface) = setup_with_size_and_scale(360, 46, 1.0).unwrap();
    renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
    let source = "A Ж 看 e\u{301} Z";
    let mut tree = WidgetTree::new();
    let node = tree.create_root(Box::new(TextField::new().with_text(source)), None, 0);
    let theme = Theme::default();
    let icons = IconRegistry::new(&[]);
    let constraints = Constraints::tight(Size::new(360.0, 46.0));
    let mut harness = RetainedSceneTestHarness::new();
    harness.frame(
        &mut tree,
        node,
        constraints,
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    let mut cache = TextLayoutCache::new(theme.typography.input_font_size, Color::WHITE);
    cache.rebuild(source, 0, 100_000.0, renderer.text_engine_mut());
    let mut input = Input::default();
    let cjk = source.find('看').unwrap();
    input.click(
        &mut tree,
        node,
        renderer.text_engine_mut(),
        Point::new(
            theme.geometry.text_field.padding_x + cache.caret_x_for_byte(cjk),
            23.0,
        ),
    );
    assert_eq!(
        field(&tree, node)
            .capture_edit_handoff()
            .unwrap()
            .cursor_byte(),
        cjk,
        "actual hit uses the final mixed-face boundary"
    );
    input.key(
        &mut tree,
        node,
        renderer.text_engine_mut(),
        KeyCode::End,
        Modifiers::default(),
    );
    input.key(
        &mut tree,
        node,
        renderer.text_engine_mut(),
        KeyCode::Home,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    let saved = field(&tree, node).capture_edit_handoff().unwrap();
    assert_eq!(
        (saved.selection_anchor_byte(), saved.selection_head_byte()),
        (source.len(), 0)
    );
    assert_eq!(
        &saved.text()[saved.selection_head_byte()..saved.selection_anchor_byte()],
        source,
        "unchanged source bytes for selection/copy"
    );
    assert_selection_copy(&mut tree, node, renderer.text_engine_mut(), source);
    let mut target = TextField::new();
    target.try_stage_edit_handoff(&saved).unwrap();
    let mut tree = WidgetTree::new();
    let mut input = Input::default();
    let target_node = tree.create_root(Box::new(target), None, 0);
    assert_eq!(
        field(&tree, target_node).edit_handoff_status(),
        TextFieldHandoffStatus::Pending
    );
    tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);
    assert_eq!(
        field(&tree, target_node).edit_handoff_status(),
        TextFieldHandoffStatus::Applied
    );
    let restored = field(&tree, target_node).capture_edit_handoff().unwrap();
    assert_eq!(
        (
            restored.text(),
            restored.cursor_byte(),
            restored.selection_anchor_byte(),
            restored.selection_head_byte()
        ),
        (source, 0, source.len(), 0)
    );
    let restored_frame = harness.frame(
        &mut tree,
        target_node,
        constraints,
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    assert!(
        render_frame(&mut renderer, &compositor, surface, 1.0, &restored_frame)
            .0
            .is_complete()
    );
    input.click(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        Point::new(340.0, 23.0),
    );
    input.key(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        KeyCode::Backspace,
        Modifiers::default(),
    );
    assert_eq!(
        field(&tree, target_node).text(),
        &source[..source.len() - 1]
    );
    input.send(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: std::time::Instant::now(),
            text: "z".into(),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
        },
    );
    assert_eq!(field(&tree, target_node).text(), source);
    harness.frame(
        &mut tree,
        target_node,
        constraints,
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    input.key(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        KeyCode::Backspace,
        Modifiers::default(),
    );
    input.key(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        KeyCode::Backspace,
        Modifiers::default(),
    );
    input.key(
        &mut tree,
        target_node,
        renderer.text_engine_mut(),
        KeyCode::Backspace,
        Modifiers::default(),
    );
    assert_eq!(
        field(&tree, target_node).text(),
        "A Ж 看 ",
        "combining cluster deleted atomically after Z and space"
    );
    input.text(&mut tree, target_node, renderer.text_engine_mut(), "看");
    assert_eq!(field(&tree, target_node).text(), "A Ж 看 看");
    let updated = harness.frame(
        &mut tree,
        target_node,
        constraints,
        &theme,
        renderer.text_engine_mut(),
        &icons,
    );
    assert!(
        render_frame(&mut renderer, &compositor, surface, 1.0, &updated)
            .0
            .is_complete()
    );
    tree.process_text_field_handoffs(renderer.text_engine_mut(), &theme);
    assert_eq!(
        field(&tree, target_node).text(),
        "A Ж 看 看",
        "one-shot handoff cannot overwrite subsequent edits"
    );
    let mut refused = TextField::new();
    refused.try_stage_edit_handoff(&saved).unwrap();
    let mut refused_tree = WidgetTree::new();
    let refused_node = refused_tree.create_root(Box::new(refused), None, 0);
    refused_tree
        .process_text_field_handoffs(&mut InvalidCatalog(renderer.text_engine_mut()), &theme);
    assert_eq!(
        field(&refused_tree, refused_node).edit_handoff_status(),
        TextFieldHandoffStatus::Rejected(TextFieldHandoffError::ShapingUnavailable)
    );
    assert_eq!(
        field(&refused_tree, refused_node).text(),
        "",
        "failed registered preparation is atomic"
    );
}

#[test]
fn registered_editable_horizontal_scroll_keeps_real_hit_and_glyph_geometry() {
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(220, 46, scale).unwrap();
        renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let source = "Latin Ж 看 ".repeat(24);
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(TextField::new().with_text(&source)), None, 0);
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(220.0, 46.0));
        let mut harness = RetainedSceneTestHarness::new();
        let frame = harness.frame(
            &mut tree,
            node,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let mut emitted = Vec::new();
        texts(&frame.scene_root, &mut emitted);
        let origin = emitted
            .iter()
            .find(|text| !text.text.is_empty())
            .unwrap()
            .position
            .x;
        assert!(
            origin < 0.0,
            "real long-text paint has the original horizontal scroll applied"
        );
        let mut cache = TextLayoutCache::new(theme.typography.input_font_size, Color::WHITE);
        cache.rebuild(&source, 0, 100_000.0, renderer.text_engine_mut());
        let boundaries = cache.boundaries();
        let (index, hit) = boundaries
            .iter()
            .enumerate()
            .find(|(_, b)| {
                let x = origin + b.x_logical;
                x > theme.geometry.text_field.padding_x + 20.0 && x < 170.0
            })
            .unwrap();
        let cursor = hit.byte_start;
        let previous = boundaries[index - 1].byte_start;
        let mut input = Input::default();
        input.click(
            &mut tree,
            node,
            renderer.text_engine_mut(),
            Point::new(origin + hit.x_logical, 23.0),
        );
        assert_eq!(
            field(&tree, node)
                .capture_edit_handoff()
                .unwrap()
                .cursor_byte(),
            cursor
        );
        input.key(
            &mut tree,
            node,
            renderer.text_engine_mut(),
            KeyCode::Backspace,
            Modifiers::default(),
        );
        let mut expected = source.clone();
        expected.replace_range(previous..cursor, "");
        assert_eq!(field(&tree, node).text(), expected);
        let changed = harness.frame(
            &mut tree,
            node,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let (outcome, pixels) = render_frame(&mut renderer, &compositor, surface, scale, &changed);
        assert!(outcome.is_complete());
        evidence(
            &format!("scroll-{scale}x"),
            &pixels,
            (220.0 * scale) as usize,
            (46.0 * scale) as usize,
        );
    }
}

#[test]
fn registered_editable_placeholder_and_password_keep_their_source_contracts() {
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(260, 46, scale).unwrap();
        renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(260.0, 46.0));
        for password in [false, true] {
            let widget = if password {
                TextField::new()
                    .with_text("Ж看e\u{301}")
                    .with_password(true)
            } else {
                TextField::new().with_placeholder("Placeholder 看")
            };
            let mut tree = WidgetTree::new();
            let node = tree.create_root(Box::new(widget), None, 0);
            let mut harness = RetainedSceneTestHarness::new();
            let frame = harness.frame(
                &mut tree,
                node,
                constraints,
                &theme,
                renderer.text_engine_mut(),
                &icons,
            );
            let mut emitted = Vec::new();
            texts(&frame.scene_root, &mut emitted);
            if password {
                assert!(!emitted.iter().any(|node| node.text.contains('看')
                    || node.text.contains('Ж')
                    || node.text.contains('e')));
                assert_eq!(field(&tree, node).text(), "Ж看e\u{301}");
            } else {
                assert!(emitted.iter().any(|node| node.text.contains('看')
                    && node.binding.as_ref().unwrap().face().family_name() == "Arial Unicode MS"));
                assert_eq!(field(&tree, node).text(), "");
            }
            let (outcome, pixels) =
                render_frame(&mut renderer, &compositor, surface, scale, &frame);
            assert!(outcome.is_complete());
            evidence(
                &format!("password-{password}-{scale}x"),
                &pixels,
                (260.0 * scale) as usize,
                (46.0 * scale) as usize,
            );
        }
    }
}

#[test]
fn registered_editable_text_area_wrapping_direction_and_metal_1x_2x() {
    for scale in [1.0_f32, 2.0] {
        for source in [
            "A Ж 看 e\u{301} line with enough words to wrap several times.\nSecond 看 line.",
            "אבג 看 Latin Ж e\u{301}\nאבג end",
        ] {
            let (mut renderer, compositor, surface) =
                setup_with_size_and_scale(180, 110, scale).unwrap();
            renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
            renderer.text_engine_mut().set_scale(scale);
            let mut tree = WidgetTree::new();
            let node = tree.create_root(
                Box::new(TextArea::new().with_text(source).with_fixed_height(110.0)),
                None,
                0,
            );
            let mut harness = RetainedSceneTestHarness::new();
            let theme = Theme::default();
            let icons = IconRegistry::new(&[]);
            let constraints = Constraints::tight(Size::new(180.0, 110.0));
            let frame = harness.frame(
                &mut tree,
                node,
                constraints,
                &theme,
                renderer.text_engine_mut(),
                &icons,
            );
            let mut emitted = Vec::new();
            texts(&frame.scene_root, &mut emitted);
            assert!(emitted.iter().any(|text| text.text.contains('看')
                && text.binding.as_ref().unwrap().face().family_name() == "Arial Unicode MS"));
            for text in emitted {
                let binding = text.binding.as_ref().unwrap();
                assert_eq!(
                    binding.direction_for(&text.text),
                    crate::core::text_shaper::TextDirection::detect(source)
                );
                let run = renderer
                    .text_engine_mut()
                    .shape_bound_text(&text.text, text.font_size, binding)
                    .unwrap();
                assert!(run.glyphs.iter().all(|glyph| glyph.id != 0));
            }
            let (outcome, pixels) =
                render_frame(&mut renderer, &compositor, surface, scale, &frame);
            assert!(outcome.is_complete(), "{outcome:?}");
            evidence(
                &format!("area-{}-{scale}x", source.starts_with('א')),
                &pixels,
                (180.0 * scale) as usize,
                (110.0 * scale) as usize,
            );
            let mut input = Input::default();
            input.click(
                &mut tree,
                node,
                renderer.text_engine_mut(),
                Point::new(30.0, 25.0),
            );
            input.key(
                &mut tree,
                node,
                renderer.text_engine_mut(),
                KeyCode::End,
                Modifiers {
                    meta: true,
                    ..Default::default()
                },
            );
            input.text(&mut tree, node, renderer.text_engine_mut(), "看");
            let current = tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<TextArea>()
                .unwrap()
                .text();
            assert_eq!(current.len(), source.len() + 3);
            assert!(current.contains("看"));
            let changed = harness.frame(
                &mut tree,
                node,
                constraints,
                &theme,
                renderer.text_engine_mut(),
                &icons,
            );
            assert!(
                render_frame(&mut renderer, &compositor, surface, scale, &changed)
                    .0
                    .is_complete()
            );
        }
    }
}

#[test]
fn registered_editable_uncovered_degraded_preserves_source_and_same_surface_input() {
    use crate::{core::layout::FlexParams, widgets::flex::Flex};
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(360, 92, scale).unwrap();
        renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let source = "Uncovered \u{10ffff} 看";
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let bad = tree.add_child(root, Box::new(TextField::new().with_text(source)), None, 0);
        tree.set_flex(bad, FlexParams::fixed(46.0));
        let good = tree.add_child(root, Box::new(TextField::new()), None, 0);
        tree.set_flex(good, FlexParams::fixed(46.0));
        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(360.0, 92.0));
        harness.frame(
            &mut tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let rect = tree.get_absolute_paint_rect(good).unwrap();
        let mut input = Input::default();
        input.click(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            Point::new(rect.x + 10.0, rect.y + rect.height * 0.5),
        );
        input.text(&mut tree, root, renderer.text_engine_mut(), "Healthy 看");
        assert_eq!(
            field(&tree, bad).capture_edit_handoff().unwrap().text(),
            source
        );
        assert_eq!(field(&tree, good).text(), "Healthy 看");
        let frame = harness.frame(
            &mut tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let (outcome, pixels) = render_frame(&mut renderer, &compositor, surface, scale, &frame);
        assert!(matches!(
            outcome,
            TextRealizationOutcome::Degraded {
                missing_cluster_count: 1,
                ..
            }
        ));
        assert!(outcome.is_renderable() && !outcome.is_complete());
        evidence(
            &format!("uncovered-{scale}x"),
            &pixels,
            (360.0 * scale) as usize,
            (92.0 * scale) as usize,
        );
        let rect = tree.get_absolute_paint_rect(bad).unwrap();
        input.click(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            Point::new(rect.x + 10.0, rect.y + rect.height * 0.5),
        );
        input.key(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            KeyCode::End,
            Modifiers::default(),
        );
        input.key(
            &mut tree,
            root,
            renderer.text_engine_mut(),
            KeyCode::Home,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_selection_copy(&mut tree, bad, renderer.text_engine_mut(), source);
    }
}

fn assert_selection_copy(
    tree: &mut WidgetTree,
    node: NodeId,
    engine: &mut dyn TextEngine,
    source: &str,
) {
    use crate::core::{
        event::{Event, EventPhase, KeyboardAuthorityMode, TextInputEvent},
        widget::{EventContext, Widget},
    };
    // Reuse the existing TextField command owner and test memory transport;
    // neither read nor write the user's system clipboard.
    let before = field(tree, node).capture_edit_handoff().unwrap();
    let mut clipboard = crate::platform::clipboard::Clipboard::new().expect("clipboard facade");
    clipboard.use_memory_transport_for_test("untouched");
    let owner = tree.node(node).unwrap().id;
    let tree_ptr = tree as *mut WidgetTree;
    let mut cx = EventContext::new(
        owner,
        tree_ptr,
        0,
        &mut clipboard,
        Some(engine),
        None,
        false,
        false,
        KeyboardAuthorityMode::Capture,
    );
    assert!(
        tree.node_mut(node)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<TextField>()
            .unwrap()
            .handle_event(
                &mut cx,
                &Event::Text(TextInputEvent {
                    text: "c".into(),
                    modifiers: Modifiers {
                        meta: true,
                        ..Modifiers::default()
                    },
                }),
                EventPhase::Target
            )
    );
    assert_eq!(clipboard.get_text().as_deref(), Some(source));
    let after = field(tree, node).capture_edit_handoff().unwrap();
    assert_eq!(
        (
            after.text(),
            after.cursor_byte(),
            after.selection_anchor_byte(),
            after.selection_head_byte()
        ),
        (
            before.text(),
            before.cursor_byte(),
            before.selection_anchor_byte(),
            before.selection_head_byte()
        )
    );
}

fn texts<'a>(nodes: &'a [SceneNode], out: &mut Vec<&'a TextNode>) {
    for node in nodes {
        match node {
            SceneNode::Text(text) => out.push(text),
            SceneNode::RetainedFragment(fragment) => texts(&fragment.children, out),
            SceneNode::Clip(clip) => texts(&clip.children, out),
            SceneNode::Scroll(scroll) => texts(&scroll.children, out),
            SceneNode::Layer(layer) => texts(&layer.children, out),
            _ => {}
        }
    }
}

fn field(tree: &WidgetTree, node: NodeId) -> &TextField {
    tree.node(node)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref()
        .unwrap()
}

fn evidence(name: &str, pixels: &[u8], width: usize, height: usize) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let directory = std::env::temp_dir().join(format!(
        "halcyon-registered-editable-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(directory.join(format!("{name}.rgba")), pixels).unwrap();
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for pixel in pixels.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(directory.join(format!("{name}.ppm")), ppm).unwrap();
    directory
}

#[test]
fn registered_editable_field_covering_runs_metal_1x_2x() {
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(680, 46, scale).unwrap();
        renderer.admit_font_assets(&[supplemental_asset()]).unwrap();
        renderer.text_engine_mut().set_scale(scale);
        let primary = renderer.text_engine_mut().shape("看", 13.0);
        assert!(
            primary.glyphs.iter().any(|glyph| glyph.id == 0),
            "independent actual primary lacks the source cluster"
        );
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(TextField::new()), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(680.0, 46.0));
        harness.frame(
            &mut tree,
            node,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let mut router = EventRouter::new();
        let mut domains = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                pos: Point::new(10.0, 23.0),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                text: REQUEST.into(),
                modifiers: Modifiers::default(),
            },
        ] {
            router.route_input_event(
                &mut tree,
                event,
                Some(node),
                renderer.text_engine_mut(),
                None,
                &mut domains,
                &mut bindings,
            );
        }
        assert_eq!(
            field(&tree, node).text(),
            REQUEST,
            "genuine routed input, no direct document assignment"
        );
        let frame = harness.frame(
            &mut tree,
            node,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
        );
        let mut nodes = Vec::new();
        texts(&frame.scene_root, &mut nodes);
        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root.clone();
        scene.clear_color = SceneColor::TRANSPARENT;
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .unwrap();
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let outcome = compiled
            .text_realization(surface)
            .unwrap()
            .outcome()
            .clone();
        let pixels = read_compiled_surface(&renderer, &compiled, surface).unwrap();
        let directory = evidence(
            &format!("covering-{scale}x"),
            &pixels,
            (680.0 * scale) as usize,
            (46.0 * scale) as usize,
        );
        eprintln!(
            "REGISTERED_EDITABLE DPR={scale} bytes={} outcome={outcome:?} evidence={}",
            REQUEST.len(),
            directory.display()
        );
        assert!(
            outcome.is_complete(),
            "admitted covering face must be Complete, not a Degraded marker: {outcome:?}"
        );
        let covering: Vec<_> = nodes
            .iter()
            .filter(|node| node.text.contains('看'))
            .collect();
        assert!(!covering.is_empty());
        for text in covering {
            let binding = text.binding.as_ref().unwrap();
            assert_eq!(binding.face().family_name(), "Arial Unicode MS");
            let catalog = renderer.text_engine_mut().font_face_catalog().unwrap();
            assert_eq!(
                &catalog.get(binding.face().font_id()).unwrap().binding,
                binding.face()
            );
            let run = renderer
                .text_engine_mut()
                .shape_bound_text(&text.text, text.font_size, binding)
                .unwrap();
            assert!(!run.glyphs.is_empty() && run.glyphs.iter().all(|glyph| glyph.id != 0));
        }
        assert!(
            pixels
                .chunks_exact(4)
                .filter(|pixel| pixel[..3].iter().any(|c| *c > 150))
                .count()
                > 40
        );
    }
}
