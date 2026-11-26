use super::*;

#[cfg(target_os = "macos")]
#[test]
fn u1_textfield_unicode_motion_real_metal_1x_2x() {
    use crate::core::event::InteractionDomains;
    use crate::core::text_engine::TextEngine;
    use crate::widgets::text_field::TextField;
    fn caret_rects(nodes: &[SceneNode], scale: f32, out: &mut Vec<RectF>) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => for span in &text.backgrounds {
                    out.push(RectF::new((text.position.x+span.x_offset)*scale,
                        (text.position.y+span.y_offset)*scale, span.width*scale, span.height*scale));
                },
                SceneNode::Clip(c) => caret_rects(&c.children, scale, out),
                SceneNode::RetainedFragment(f) => caret_rects(&f.children, scale, out),
                _ => {},
            }
        }
    }
    fn frame(
        renderer: &mut WgpuRenderer,
        surface: SurfaceId,
        tree: &mut WidgetTree,
        root: crate::core::widget_tree::NodeId,
        h: &mut RetainedSceneTestHarness,
        domains: &InteractionDomains,
        theme: &Theme,
        now: f64,
    ) -> SceneGraph {
        h.advance_transition_time(now);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let result = h.frame_with_optional_custom_render_target(
            tree,
            root,
            Constraints::tight(Size::new(360.0, 90.0)),
            theme,
            renderer.text_engine_mut(),
            &builtin_registry(),
            Some(target),
            Some((domains, WindowId::PRIMARY)),
        );
        let mut scene = SceneBuilder::new();
        scene.push_retained_fragment(result.scene_root.into());
        scene.build()
    }
    let directory = std::path::PathBuf::from(format!(
        "target/evidence/u1-motion/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0_f32, 2.0] {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(360, 90, scale).expect("U1 requires actual Metal");
        assert_eq!(
            renderer.gpu_resources().adapter.get_info().backend,
            wgpu::Backend::Metal
        );
        renderer.text_engine_mut().set_scale(scale);
        for (case, prefix, append) in [
            ("ascii", "Browser input", "!"),
            ("cyrillic", "Привет", " мир"),
            ("combining", "e", "\u{301}"),
        ] {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let node =
                tree.add_child(root, Box::new(TextField::new().with_text(prefix)), None, 0);
            let owner = tree.node(node).unwrap().id;
            let mut h = RetainedSceneTestHarness::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let theme = Theme::default();
            frame(
                &mut renderer,
                surface,
                &mut tree,
                root,
                &mut h,
                &domains,
                &theme,
                0.0,
            );
            router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
            assert!(router.handle_engine_action(
                &mut tree,
                crate::runtime::actions::ActionId::ENTER_WIDGET,
                Some(root),
                domains.primary_state_mut(),
                renderer.text_engine_mut(),
                None
            ));
            let scene = frame(
                &mut renderer,
                surface,
                &mut tree,
                root,
                &mut h,
                &domains,
                &theme,
                0.002,
            );
            let before =
                render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
            router.route_input_event(
                &mut tree,
                FusedInputEvent::TextInput {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    text: append.into(),
                    modifiers: Modifiers::default(),
                },
                Some(root),
                renderer.text_engine_mut(),
                Some(h.animation_context()),
                domains.primary_state_mut(),
                &mut BindingRegistry::new(),
            );
            let expected = format!("{prefix}{append}");
            assert_eq!(
                tree.node(node)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextField>()
                    .unwrap()
                    .text(),
                expected
            );
            let run = renderer
                .text_engine_mut()
                .shape(&expected, theme.typography.font_size);
            assert!(run.glyphs.iter().all(|g| g.id != 0));
            let mut captures = vec![("before", before)];
            let mut edge_regions = Vec::new();
            caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge_regions);
            for (stage, time) in [
                ("start", 0.010),
                ("mid", 0.030),
                ("after", 0.080),
                ("quiet", 0.090),
            ] {
                let scene = frame(
                    &mut renderer,
                    surface,
                    &mut tree,
                    root,
                    &mut h,
                    &domains,
                    &theme,
                    time,
                );
                if stage == "start" { caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge_regions); }
                captures.push((
                    stage,
                    render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale),
                ));
            }
            let width = (360.0 * scale) as usize;
            let height = (90.0 * scale) as usize;
            for (stage, rgba) in &captures {
                let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
                for pixel in rgba.chunks_exact(4) {
                    ppm.extend_from_slice(&pixel[..3]);
                }
                std::fs::write(directory.join(format!("{case}-{scale}x-{stage}.ppm")), ppm)
                    .unwrap();
            }
            let changes: Vec<_> = captures[0].1.chunks_exact(4).zip(captures[1].1.chunks_exact(4))
                .enumerate().filter_map(|(i, (a,b))| (a != b).then_some((i % width, i / width))).collect();
            println!("U1 start comparison {case}/{scale}: pixels={} first={:?} captures={}",
                changes.len(), changes.first(), directory.display());
            assert!(changes.iter().all(|&(x,y)| edge_regions.iter().any(|r| {
                (x+1) as f32 > r.x && (x as f32) < r.right()
                    && (y+1) as f32 > r.y && (y as f32) < r.bottom()
            })), "exact old glyph pixels outside the actual snapped caret/clip-edge footprint: {case}/{scale}; regions={edge_regions:?}");
            assert!(captures[1].1 != captures[2].1, "real intermediate GPU pixels: {case}/{scale}");
            assert!(captures[2].1 != captures[3].1, "settled canonical GPU pixels: {case}/{scale}");
            assert!(captures[3].1 == captures[4].1, "quiet final pixels: {case}/{scale}");
        }
    }
    println!("U1 original Metal captures: {}", directory.display());
}

#[cfg(target_os = "macos")]
#[test]
fn u1_textfield_rtl_emoji_motion_real_metal_1x_2x() {
    use crate::core::event::{InteractionDomains, PhysicalKeyCode};
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::{FontAsset, FontManager};
    use crate::platform::native_font_backend::NativeFontBackend;
    use crate::renderer::TextRealizationOutcome;
    use crate::widgets::text_field::TextField;
    use sha2::{Digest, Sha256};

    fn field(tree: &WidgetTree, node: crate::core::widget_tree::NodeId) -> &TextField {
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }
    fn frame(
        renderer: &mut WgpuRenderer,
        surface: SurfaceId,
        tree: &mut WidgetTree,
        root: crate::core::widget_tree::NodeId,
        h: &mut RetainedSceneTestHarness,
        domains: &InteractionDomains,
        theme: &Theme,
        now: f64,
    ) -> crate::core::app::RetainedSceneTestFrame {
        h.advance_transition_time(now);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        h.frame_with_optional_custom_render_target(
            tree,
            root,
            Constraints::tight(Size::new(360.0, 90.0)),
            theme,
            renderer.text_engine_mut(),
            &builtin_registry(),
            Some(target),
            Some((domains, WindowId::PRIMARY)),
        )
    }
    fn pixels(
        renderer: &mut WgpuRenderer,
        compositor: &Compositor,
        surface: SurfaceId,
        frame: &crate::core::app::RetainedSceneTestFrame,
        scale: f32,
        font_id: u64,
    ) -> Vec<u8> {
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, control_order_scene(frame))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("ordinary Build admission");
        for primitive in &plan.surface_data[&surface].primitives {
            if let crate::renderer::batcher::PrimitiveData::Text { binding, .. } = &primitive.data {
                assert!(
                    binding
                        .as_ref()
                        .is_some_and(|b| b.face().font_id() == font_id),
                    "every emitted text run must keep the admitted face"
                );
            }
        }
        let compiled = renderer
            .compile_multi(&plan)
            .expect("ordinary Compile")
            .expect_ready("native target");
        let outcome = compiled.text_realization(surface).unwrap().outcome();
        assert!(
            matches!(outcome, TextRealizationOutcome::Complete { .. }),
            "required complete native text: {outcome:?}"
        );
        read_compiled_surface(renderer, &compiled, surface)
            .expect("ordinary encode/submit/readback")
    }
    fn caret_rects(nodes: &[SceneNode], scale: f32, result: &mut Vec<RectF>) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => {
                    for b in &text.backgrounds {
                        result.push(RectF::new(
                            (text.position.x + b.x_offset) * scale,
                            (text.position.y + b.y_offset) * scale,
                            b.width * scale,
                            b.height * scale,
                        ));
                    }
                }
                SceneNode::Clip(c) => caret_rects(&c.children, scale, result),
                SceneNode::RetainedFragment(f) => caret_rects(&f.children, scale, result),
                SceneNode::Layer(layer) => caret_rects(&layer.children, scale, result),
                _ => {}
            }
        }
    }
    fn save(directory: &std::path::Path, label: &str, rgba: &[u8], scale: f32) {
        let mut ppm = format!(
            "P6\n{} {}\n255\n",
            (360.0 * scale) as u32,
            (90.0 * scale) as u32
        )
        .into_bytes();
        for p in rgba.chunks_exact(4) {
            ppm.extend_from_slice(&p[..3]);
        }
        std::fs::write(directory.join(format!("{label}.ppm")), ppm).unwrap();
    }

    let directory = std::path::PathBuf::from(format!(
        "target/evidence/u1-rtl-emoji/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (path, cases) in [
        (
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            vec![
                ("arabic", "ب", "ب", "ب"),
                ("hebrew", "שלו", "ם", "שלו"),
                ("mixed", "Hello ", "שלום", "Hello שלו"),
            ],
        ),
        (
            "/System/Library/Fonts/Apple Color Emoji.ttc",
            vec![("zwj", "👩", "\u{200d}💻", "")],
        ),
    ] {
        let mut manager = FontManager::new();
        let id = manager
            .load_font_file(path)
            .expect("explicit installed covering test face");
        let face = manager.get_face(id).unwrap();
        let asset = FontAsset {
            family_name: face.family_name.clone(),
            weight: face.weight,
            italic: face.italic,
            data: face.data.clone(),
        };
        let font_sha = format!("{:x}", Sha256::digest(asset.data.as_slice()));
        for scale in [1.0_f32, 2.0] {
            let (mut renderer, compositor, surface) =
                setup_with_size_and_scale(360, 90, scale).expect("actual native GPU required");
            let adapter = renderer.gpu_resources().adapter.get_info();
            assert_eq!(adapter.backend, wgpu::Backend::Metal);
            renderer
                .admit_font_assets(std::slice::from_ref(&asset))
                .unwrap();
            let catalog = renderer.font_face_catalog();
            let admitted = catalog
                .get(id)
                .expect("same manager-derived immutable face ID");
            assert!(admitted.binding.data().as_slice() == asset.data.as_slice());
            let engine = renderer.text_engine_mut();
            // set_shaper requires its default native metrics/raster face to match.
            // Bootstrap both from the admitted immutable bytes before any frame.
            *engine.backend_mut() = NativeFontBackend::from_font_data(asset.data.clone(), id)
                .expect("native face from the same admitted bytes");
            engine.set_shaper(admitted.shaper.clone(), id);
            engine.set_scale(scale);
            assert_eq!(engine.font_id(), id);
            let native_backend = std::any::type_name_of_val(engine.backend()).to_owned();

            for &(case, prefix, append, deleted) in &cases {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(Flex::column()), None, 0);
                let node =
                    tree.add_child(root, Box::new(TextField::new().with_text(prefix)), None, 0);
                let owner = tree.node(node).unwrap().id;
                let mut h = RetainedSceneTestHarness::new();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let theme = Theme::default();
                frame(
                    &mut renderer,
                    surface,
                    &mut tree,
                    root,
                    &mut h,
                    &domains,
                    &theme,
                    0.0,
                );
                router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
                assert!(router.handle_engine_action(
                    &mut tree,
                    crate::runtime::actions::ActionId::ENTER_WIDGET,
                    Some(root),
                    domains.primary_state_mut(),
                    renderer.text_engine_mut(),
                    None
                ));
                let before_frame = frame(
                    &mut renderer,
                    surface,
                    &mut tree,
                    root,
                    &mut h,
                    &domains,
                    &theme,
                    0.002,
                );
                assert!(!before_frame.needs_next_frame);
                let mode = domains.primary_state().keyboard_mode;
                let label = format!("{case}-{scale}x");
                let mut before = pixels(
                    &mut renderer,
                    &compositor,
                    surface,
                    &before_frame,
                    scale,
                    id,
                );
                save(
                    &directory,
                    &format!("{label}-append-before"),
                    &before,
                    scale,
                );
                let full = format!("{prefix}{append}");
                let mut shape_proof = Vec::new();
                for text in [prefix, full.as_str(), deleted] {
                    let engine = renderer.text_engine_mut();
                    let run = engine.shape(text, theme.typography.input_font_size);
                    let sources = engine
                        .verified_glyph_source_ranges(&run)
                        .expect("full source-to-glyph reproduction");
                    assert!(
                        run.binding
                            .as_ref()
                            .is_some_and(|b| b.face().font_id() == id)
                    );
                    assert!(
                        run.glyphs.iter().all(|g| g.id != 0),
                        "{case}: actual glyph cover required"
                    );
                    let (_, keys) = engine.shape_with_keys(text, theme.typography.input_font_size);
                    assert!(keys.iter().all(|key| key.font_id == id));
                    shape_proof.push(serde_json::json!({"source":text,"width":run.width,
                        "glyphs":run.glyphs.iter().zip(sources).map(|(g,s)| {
                            serde_json::json!([g.id,g.x,g.y,g.advance,s.start,s.end])
                        }).collect::<Vec<_>>(),
                        "clusters":run.clusters.iter().map(|c| {
                            serde_json::json!([c.source.start,c.source.end,c.x_start,c.x_end,format!("{:?}",c.direction)])
                        }).collect::<Vec<_>>(), "atlas_keys":format!("{keys:?}")}));
                }
                let geometry = tree.own_presentation_geometry(node, scale).unwrap();
                let proof = serde_json::json!({"case":case,"scale":scale,"font_path":path,
                    "family":asset.family_name,"font_id":id,"font_sha256":font_sha,
                    "backend":native_backend,"adapter":adapter.name,"adapter_backend":format!("{:?}",adapter.backend),
                    "rect":[geometry.rect.x,geometry.rect.y,geometry.rect.width,geometry.rect.height],
                    "shapes":shape_proof});
                std::fs::write(
                    directory.join(format!("{label}-source.json")),
                    serde_json::to_vec_pretty(&proof).unwrap(),
                )
                .unwrap();
                println!(
                    "U1_NATIVE {label} family={} font_id={id} backend={native_backend} source={}/{} captures={}",
                    asset.family_name,
                    prefix,
                    full,
                    directory.display()
                );
                let mut time = Instant::now();
                for (operation, expected, start) in [
                    ("append", full.as_str(), 0.010),
                    ("backspace", deleted, 0.100),
                ] {
                    router.set_frame_time_ms(if operation == "append" { 2 } else { 90 });
                    let event = if operation == "append" {
                        FusedInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time,
                            text: append.into(),
                            modifiers: Modifiers::default(),
                        }
                    } else {
                        FusedInputEvent::KeyDown {
                            window_id: WindowId::PRIMARY,
                            time,
                            key_code: KeyCode::Backspace,
                            physical_key: PhysicalKeyCode::Unknown,
                            reservation: None,
                            modifiers: Modifiers::default(),
                            repeat: false,
                        }
                    };
                    router.route_input_event(
                        &mut tree,
                        event,
                        Some(root),
                        renderer.text_engine_mut(),
                        Some(h.animation_context()),
                        domains.primary_state_mut(),
                        &mut BindingRegistry::new(),
                    );
                    time += std::time::Duration::from_nanos(1);
                    assert_eq!(
                        field(&tree, node).text(),
                        expected,
                        "immediate {case}/{operation}"
                    );
                    assert_eq!(field(&tree, node).cursor_pos(), expected.len());
                    assert_eq!(field(&tree, node).selection().anchor, expected.len());
                    assert_eq!(field(&tree, node).selection().head, expected.len());
                    assert_eq!(
                        field(&tree, node).scroll_x_for_test(),
                        0.0,
                        "no scroll-changing qualification"
                    );
                    let mut edge = Vec::new();
                    caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge);
                    let mut samples = Vec::new();
                    for (stage, now) in [
                        ("start", start),
                        ("mid", start + f64::from(0.040_f32) * 0.5),
                        ("final", (start + f64::from(0.040_f32)).next_up()),
                        ("quiet", start + 0.060),
                    ] {
                        let f = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            now,
                        );
                        assert_eq!(h.animation_context().frame_timestamp_secs(), Some(now),
                            "sample the original engine Timeline timestamp");
                        assert_eq!(field(&tree, node).text(), expected);
                        assert_eq!(field(&tree, node).scroll_x_for_test(), 0.0);
                        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                        assert_eq!(domains.primary_state().keyboard_mode, mode);
                        let rgba = pixels(&mut renderer, &compositor, surface, &f, scale, id);
                        save(
                            &directory,
                            &format!("{label}-{operation}-{stage}"),
                            &rgba,
                            scale,
                        );
                        println!(
                            "U1_NATIVE_FRAME {label}/{operation}/{stage} time={now:.9} harness_next_frame={} sha={:x}",
                            f.needs_next_frame,
                            Sha256::digest(&rgba)
                        );
                        if stage == "start" {
                            caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge);
                        }
                        samples.push((f.needs_next_frame, rgba));
                    }
                    let width = (360.0 * scale) as usize;
                    let changes: Vec<_> = before
                        .chunks_exact(4)
                        .zip(samples[0].1.chunks_exact(4))
                        .enumerate()
                        .filter_map(|(i, (a, b))| (a != b).then_some((i % width, i / width)))
                        .collect();
                    assert!(
                        changes.iter().all(|&(x, y)| edge.iter().any(|r| {
                            (x + 1) as f32 > r.x
                                && (x as f32) < r.right()
                                && (y + 1) as f32 > r.y
                                && (y as f32) < r.bottom()
                        })),
                        "{label}/{operation}: start must preserve old ink outside actual caret footprint"
                    );
                    assert!(
                        samples[0].1 != samples[1].1,
                        "{label}/{operation}: intermediate native ink"
                    );
                    assert!(
                        samples[1].1 != samples[2].1,
                        "{label}/{operation}: settled native ink"
                    );
                    assert!(
                        samples[2].1 == samples[3].1,
                        "{label}/{operation}: quiet pixels"
                    );
                    tree.node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<TextField>()
                        .unwrap()
                        .set_smooth_caret_prototype(false);
                    let plain_frame = frame(
                        &mut renderer,
                        surface,
                        &mut tree,
                        root,
                        &mut h,
                        &domains,
                        &theme,
                        start + 0.065,
                    );
                    let canonical =
                        pixels(&mut renderer, &compositor, surface, &plain_frame, scale, id);
                    save(
                        &directory,
                        &format!("{label}-{operation}-canonical"),
                        &canonical,
                        scale,
                    );
                    assert!(
                        canonical == samples[2].1,
                        "{label}/{operation}: exact same-owner canonical pixels"
                    );
                    tree.node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<TextField>()
                        .unwrap()
                        .set_smooth_caret_prototype(true);
                    router.set_frame_time_ms(((start + 0.065) * 1000.0) as u64);
                    for (redo, history_expected) in [
                        (
                            false,
                            if operation == "append" {
                                prefix
                            } else {
                                full.as_str()
                            },
                        ),
                        (true, expected),
                    ] {
                        router.route_input_event(
                            &mut tree,
                            FusedInputEvent::TextInput {
                                window_id: WindowId::PRIMARY,
                                time,
                                text: "z".into(),
                                modifiers: Modifiers {
                                    meta: true,
                                    shift: redo,
                                    ..Default::default()
                                },
                            },
                            Some(root),
                            renderer.text_engine_mut(),
                            Some(h.animation_context()),
                            domains.primary_state_mut(),
                            &mut BindingRegistry::new(),
                        );
                        time += std::time::Duration::from_nanos(1);
                        assert_eq!(
                            field(&tree, node).text(),
                            history_expected,
                            "one immediate Undo/Redo"
                        );
                        assert_eq!(field(&tree, node).cursor_pos(), history_expected.len());
                    }
                    if operation == "append" {
                        let next = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            0.090,
                        );
                        assert!(!next.needs_next_frame);
                        // Undo/Redo invalidate Layout. The .090 frame updates before
                        // rebuilding that Layout; qualify the rebuilt owner on its
                        // next ordinary Update before starting the independent edit.
                        let qualified = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            0.095,
                        );
                        before = pixels(&mut renderer, &compositor, surface, &qualified, scale, id);
                        save(
                            &directory,
                            &format!("{label}-backspace-before"),
                            &before,
                            scale,
                        );
                        assert!(before == canonical, "same source after original Undo/Redo");
                    }
                }
            }
        }
    }
    println!(
        "U1 RTL/emoji original Metal captures: {}",
        directory.display()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn u1_textfield_long_visible_edit_real_metal_1x_2x() {
    use crate::core::event::{InteractionDomains, PhysicalKeyCode};
    use crate::core::text_engine::TextEngine;
    use crate::platform::font_manager::{FontAsset, FontManager};
    use crate::platform::native_font_backend::NativeFontBackend;
    use crate::renderer::TextRealizationOutcome;
    use crate::widgets::text_field::TextField;
    use sha2::{Digest, Sha256};

    fn field(tree: &WidgetTree, node: crate::core::widget_tree::NodeId) -> &TextField {
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }
    fn frame(
        renderer: &mut WgpuRenderer,
        surface: SurfaceId,
        tree: &mut WidgetTree,
        root: crate::core::widget_tree::NodeId,
        h: &mut RetainedSceneTestHarness,
        domains: &InteractionDomains,
        theme: &Theme,
        now: f64,
    ) -> crate::core::app::RetainedSceneTestFrame {
        h.advance_transition_time(now);
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        h.frame_with_optional_custom_render_target(
            tree,
            root,
            Constraints::tight(Size::new(360.0, 90.0)),
            theme,
            renderer.text_engine_mut(),
            &builtin_registry(),
            Some(target),
            Some((domains, WindowId::PRIMARY)),
        )
    }
    fn pixels(
        renderer: &mut WgpuRenderer,
        compositor: &Compositor,
        surface: SurfaceId,
        frame: &crate::core::app::RetainedSceneTestFrame,
        scale: f32,
        font_id: u64,
    ) -> Vec<u8> {
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, control_order_scene(frame))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("ordinary Build admission");
        for primitive in &plan.surface_data[&surface].primitives {
            if let crate::renderer::batcher::PrimitiveData::Text { binding, .. } = &primitive.data {
                assert!(
                    binding
                        .as_ref()
                        .is_some_and(|b| b.face().font_id() == font_id),
                    "every emitted text run must keep the admitted face"
                );
            }
        }
        let compiled = renderer
            .compile_multi(&plan)
            .expect("ordinary Compile")
            .expect_ready("native target");
        let outcome = compiled.text_realization(surface).unwrap().outcome();
        assert!(
            matches!(outcome, TextRealizationOutcome::Complete { .. }),
            "required complete native text: {outcome:?}"
        );
        read_compiled_surface(renderer, &compiled, surface)
            .expect("ordinary encode/submit/readback")
    }
    fn caret_rects(nodes: &[SceneNode], scale: f32, result: &mut Vec<RectF>) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => {
                    for b in &text.backgrounds {
                        result.push(RectF::new(
                            (text.position.x + b.x_offset) * scale,
                            (text.position.y + b.y_offset) * scale,
                            b.width * scale,
                            b.height * scale,
                        ));
                    }
                }
                SceneNode::Clip(c) => caret_rects(&c.children, scale, result),
                SceneNode::RetainedFragment(f) => caret_rects(&f.children, scale, result),
                SceneNode::Layer(layer) => caret_rects(&layer.children, scale, result),
                _ => {}
            }
        }
    }
    fn save(directory: &std::path::Path, label: &str, rgba: &[u8], scale: f32) {
        let mut ppm = format!(
            "P6\n{} {}\n255\n",
            (360.0 * scale) as u32,
            (90.0 * scale) as u32
        )
        .into_bytes();
        for p in rgba.chunks_exact(4) {
            ppm.extend_from_slice(&p[..3]);
        }
        std::fs::write(directory.join(format!("{label}.ppm")), ppm).unwrap();
    }

    fn source_viewport(nodes: &[SceneNode], source: &str, clips: &mut Vec<RectF>) -> Option<RectF> {
        for node in nodes {
            let found = match node {
                SceneNode::Text(text) if text.text == source => clips.last().copied(),
                SceneNode::Clip(clip) => {
                    clips.push(clip.rect);
                    let result = source_viewport(&clip.children, source, clips);
                    clips.pop();
                    result
                }
                SceneNode::RetainedFragment(fragment) => {
                    source_viewport(&fragment.children, source, clips)
                }
                SceneNode::Layer(layer) => source_viewport(&layer.children, source, clips),
                _ => None,
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    let directory = std::path::PathBuf::from(format!(
        "target/evidence/u1-long-visible/metal-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let precursor = "Ж".repeat(19);
    let full_source = "Ж".repeat(20);
    assert_eq!(precursor.len(), 38);
    assert_eq!(full_source.len(), 40);
    for (path, cases) in [(
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        vec![
            ("end", precursor.as_str(), "Ж", precursor.as_str()),
            ("range", full_source.as_str(), "", ""),
        ],
    )] {
        let mut manager = FontManager::new();
        let id = manager
            .load_font_file(path)
            .expect("explicit installed covering test face");
        let face = manager.get_face(id).unwrap();
        let asset = FontAsset {
            family_name: face.family_name.clone(),
            weight: face.weight,
            italic: face.italic,
            data: face.data.clone(),
        };
        let font_sha = format!("{:x}", Sha256::digest(asset.data.as_slice()));
        for scale in [1.0_f32, 2.0] {
            let (mut renderer, compositor, surface) =
                setup_with_size_and_scale(360, 90, scale).expect("actual native GPU required");
            let adapter = renderer.gpu_resources().adapter.get_info();
            assert_eq!(adapter.backend, wgpu::Backend::Metal);
            renderer
                .admit_font_assets(std::slice::from_ref(&asset))
                .unwrap();
            let catalog = renderer.font_face_catalog();
            let admitted = catalog
                .get(id)
                .expect("same manager-derived immutable face ID");
            assert!(admitted.binding.data().as_slice() == asset.data.as_slice());
            let engine = renderer.text_engine_mut();
            // set_shaper requires its default native metrics/raster face to match.
            // Bootstrap both from the admitted immutable bytes before any frame.
            *engine.backend_mut() = NativeFontBackend::from_font_data(asset.data.clone(), id)
                .expect("native face from the same admitted bytes");
            engine.set_shaper(admitted.shaper.clone(), id);
            engine.set_scale(scale);
            assert_eq!(engine.font_id(), id);
            let native_backend = std::any::type_name_of_val(engine.backend()).to_owned();

            for &(case, prefix, append, deleted) in &cases {
                let mut tree = WidgetTree::new();
                let root = tree.create_root(Box::new(Flex::column()), None, 0);
                let node =
                    tree.add_child(root, Box::new(TextField::new().with_text(prefix)), None, 0);
                let owner = tree.node(node).unwrap().id;
                let mut h = RetainedSceneTestHarness::new();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let theme = Theme::default();
                frame(
                    &mut renderer,
                    surface,
                    &mut tree,
                    root,
                    &mut h,
                    &domains,
                    &theme,
                    0.0,
                );
                router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
                assert!(router.handle_engine_action(
                    &mut tree,
                    crate::runtime::actions::ActionId::ENTER_WIDGET,
                    Some(root),
                    domains.primary_state_mut(),
                    renderer.text_engine_mut(),
                    None
                ));
                let before_frame = frame(
                    &mut renderer,
                    surface,
                    &mut tree,
                    root,
                    &mut h,
                    &domains,
                    &theme,
                    0.002,
                );
                assert!(!before_frame.needs_next_frame);
                let mode = domains.primary_state().keyboard_mode;
                let label = format!("{case}-{scale}x");
                let mut before = pixels(
                    &mut renderer,
                    &compositor,
                    surface,
                    &before_frame,
                    scale,
                    id,
                );
                save(
                    &directory,
                    &format!("{label}-append-before"),
                    &before,
                    scale,
                );
                let mut original_caret = Vec::new();
                caret_rects(
                    &h.local_fragment(owner).unwrap(),
                    scale,
                    &mut original_caret,
                );
                assert!(!original_caret.is_empty(), "real captured caret required");
                let viewport =
                    source_viewport(&h.local_fragment(owner).unwrap(), prefix, &mut Vec::new())
                        .expect("the original retained text clip publishes the viewport");
                let full = format!("{prefix}{append}");
                let full_run = renderer
                    .text_engine_mut()
                    .shape(&full, theme.typography.input_font_size);
                let full_width = full_run.width;
                let source_span = full_run
                    .clusters
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), c| {
                        (a.min(c.x_start).min(c.x_end), b.max(c.x_start).max(c.x_end))
                    });
                assert!(full.len() > 32);
                assert!(full_width.is_finite() && full_width > viewport.width * 0.5);
                assert!(source_span.1 - source_span.0 > viewport.width * 0.5);
                assert!(source_span.0 >= 0.0 && source_span.1 <= full_width);
                assert!(
                    full_width < viewport.width,
                    "fixed viewport must fit the exact source"
                );
                println!(
                    "U1_LONG_GEOMETRY {label} source_bytes={} full_width={full_width} cluster_span={source_span:?} viewport={viewport:?} end_edit_advance={}",
                    full.len(),
                    full_width
                        - renderer
                            .text_engine_mut()
                            .shape(&precursor, theme.typography.input_font_size)
                            .width
                );
                let mut shape_proof = Vec::new();
                for text in [prefix, full.as_str(), deleted] {
                    let engine = renderer.text_engine_mut();
                    let run = engine.shape(text, theme.typography.input_font_size);
                    let sources = engine
                        .verified_glyph_source_ranges(&run)
                        .expect("full source-to-glyph reproduction");
                    assert!(
                        run.binding
                            .as_ref()
                            .is_some_and(|b| b.face().font_id() == id)
                    );
                    assert!(
                        run.glyphs.iter().all(|g| g.id != 0),
                        "{case}: actual glyph cover required"
                    );
                    let (_, keys) = engine.shape_with_keys(text, theme.typography.input_font_size);
                    assert!(keys.iter().all(|key| key.font_id == id));
                    shape_proof.push(serde_json::json!({"source":text,"width":run.width,
                        "glyphs":run.glyphs.iter().zip(sources).map(|(g,s)| {
                            serde_json::json!([g.id,g.x,g.y,g.advance,s.start,s.end])
                        }).collect::<Vec<_>>(),
                        "clusters":run.clusters.iter().map(|c| {
                            serde_json::json!([c.source.start,c.source.end,c.x_start,c.x_end,format!("{:?}",c.direction)])
                        }).collect::<Vec<_>>(), "atlas_keys":format!("{keys:?}")}));
                }
                let geometry = tree.own_presentation_geometry(node, scale).unwrap();
                assert_eq!(geometry.rect.width, 360.0);
                assert_eq!(geometry.rect.height, 28.0);
                assert_eq!(field(&tree, node).scroll_x_for_test(), 0.0);
                let proof = serde_json::json!({"case":case,"scale":scale,"font_path":path,
                    "family":asset.family_name,"font_id":id,"font_sha256":font_sha,
                    "backend":native_backend,"adapter":adapter.name,"adapter_backend":format!("{:?}",adapter.backend),
                    "rect":[geometry.rect.x,geometry.rect.y,geometry.rect.width,geometry.rect.height],
                    "viewport":[viewport.x,viewport.y,viewport.width,viewport.height],
                    "source_bytes":full.len(),"full_width":full_width,"source_cluster_span":[source_span.0,source_span.1],
                    "source_revision_evidence":"whole current source, exact mounted owner and ordinary fresh-cache paint; numeric private revision is not exported",
                    "shapes":shape_proof});
                std::fs::write(
                    directory.join(format!("{label}-source.json")),
                    serde_json::to_vec_pretty(&proof).unwrap(),
                )
                .unwrap();
                println!(
                    "U1_LONG_NATIVE {label} family={} font_id={id} backend={native_backend} source={}/{} captures={}",
                    asset.family_name,
                    prefix,
                    full,
                    directory.display()
                );
                let mut time = Instant::now();
                let mut selected_pixels = None;
                if case == "range" {
                    for (key_code, modifiers) in [
                        (KeyCode::Home, Modifiers::default()),
                        (
                            KeyCode::End,
                            Modifiers {
                                shift: true,
                                ..Default::default()
                            },
                        ),
                    ] {
                        router.set_frame_time_ms(2);
                        router.route_input_event(
                            &mut tree,
                            FusedInputEvent::KeyDown {
                                window_id: WindowId::PRIMARY,
                                time,
                                key_code,
                                physical_key: PhysicalKeyCode::Unknown,
                                reservation: None,
                                modifiers,
                                repeat: false,
                            },
                            Some(root),
                            renderer.text_engine_mut(),
                            Some(h.animation_context()),
                            domains.primary_state_mut(),
                            &mut BindingRegistry::new(),
                        );
                        time += std::time::Duration::from_nanos(1);
                    }
                    frame(
                        &mut renderer,
                        surface,
                        &mut tree,
                        root,
                        &mut h,
                        &domains,
                        &theme,
                        0.003,
                    );
                    let selected = frame(
                        &mut renderer,
                        surface,
                        &mut tree,
                        root,
                        &mut h,
                        &domains,
                        &theme,
                        0.004,
                    );
                    assert_eq!(field(&tree, node).text(), full_source);
                    assert_eq!(field(&tree, node).cursor_pos(), 40);
                    assert_eq!(field(&tree, node).selection().anchor, 0);
                    assert_eq!(field(&tree, node).selection().head, 40);
                    assert_eq!(field(&tree, node).scroll_x_for_test(), 0.0);
                    assert_eq!(
                        tree.own_presentation_geometry(node, scale).unwrap(),
                        geometry
                    );
                    let rgba = pixels(&mut renderer, &compositor, surface, &selected, scale, id);
                    save(
                        &directory,
                        &format!("{label}-selection-before"),
                        &rgba,
                        scale,
                    );
                    assert!(
                        rgba != before,
                        "original selection presentation must be real"
                    );
                    selected_pixels = Some(rgba);
                }
                for (operation, expected, start) in [
                    ("append", full.as_str(), 0.010),
                    ("backspace", deleted, 0.100),
                ] {
                    if case == "range" && operation == "append" {
                        continue;
                    }
                    let duration: f32 = if case == "range" {
                        (80.0 + 1.25 * full_width).clamp(100.0, 220.0) / 1000.0
                    } else {
                        0.040
                    };
                    router.set_frame_time_ms(if operation == "append" {
                        2
                    } else if case == "range" {
                        4
                    } else {
                        95
                    });
                    let event = if operation == "append" {
                        FusedInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time,
                            text: append.into(),
                            modifiers: Modifiers::default(),
                        }
                    } else {
                        FusedInputEvent::KeyDown {
                            window_id: WindowId::PRIMARY,
                            time,
                            key_code: KeyCode::Backspace,
                            physical_key: PhysicalKeyCode::Unknown,
                            reservation: None,
                            modifiers: Modifiers::default(),
                            repeat: false,
                        }
                    };
                    router.route_input_event(
                        &mut tree,
                        event,
                        Some(root),
                        renderer.text_engine_mut(),
                        Some(h.animation_context()),
                        domains.primary_state_mut(),
                        &mut BindingRegistry::new(),
                    );
                    time += std::time::Duration::from_nanos(1);
                    assert_eq!(
                        field(&tree, node).text(),
                        expected,
                        "immediate {case}/{operation}"
                    );
                    assert_eq!(field(&tree, node).cursor_pos(), expected.len());
                    assert_eq!(field(&tree, node).selection().anchor, expected.len());
                    assert_eq!(field(&tree, node).selection().head, expected.len());
                    assert_eq!(
                        field(&tree, node).scroll_x_for_test(),
                        0.0,
                        "no scroll-changing qualification"
                    );
                    // The range's old-ink reference is its captured UNSELECTED frame.
                    // Selection chrome disappears immediately with the logical edit;
                    // never mask its whole highlight as a supposed caret footprint.
                    let mut edge = if case == "range" {
                        original_caret.clone()
                    } else {
                        let mut edge = Vec::new();
                        caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge);
                        edge
                    };
                    let mut samples = Vec::new();
                    for (stage, now) in [
                        ("start", start),
                        ("mid", start + f64::from(duration) * 0.5),
                        ("final", (start + f64::from(duration)).next_up()),
                        ("quiet", start + f64::from(duration) + 0.005),
                    ] {
                        let f = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            now,
                        );
                        assert_eq!(
                            h.animation_context().frame_timestamp_secs(),
                            Some(now),
                            "sample the original engine Timeline timestamp"
                        );
                        assert_eq!(field(&tree, node).text(), expected);
                        assert_eq!(field(&tree, node).scroll_x_for_test(), 0.0);
                        assert_eq!(
                            tree.own_presentation_geometry(node, scale).unwrap(),
                            geometry
                        );
                        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                        assert_eq!(domains.primary_state().keyboard_mode, mode);
                        let rgba = pixels(&mut renderer, &compositor, surface, &f, scale, id);
                        save(
                            &directory,
                            &format!("{label}-{operation}-{stage}"),
                            &rgba,
                            scale,
                        );
                        println!(
                            "U1_LONG_NATIVE_FRAME {label}/{operation}/{stage} time={now:.9} harness_next_frame={} sha={:x}",
                            f.needs_next_frame,
                            Sha256::digest(&rgba)
                        );
                        if stage == "start" {
                            caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut edge);
                        }
                        samples.push((f.needs_next_frame, rgba));
                    }
                    let width = (360.0 * scale) as usize;
                    let changes: Vec<_> = before
                        .chunks_exact(4)
                        .zip(samples[0].1.chunks_exact(4))
                        .enumerate()
                        .filter_map(|(i, (a, b))| (a != b).then_some((i % width, i / width)))
                        .collect();
                    assert!(
                        changes.iter().all(|&(x, y)| edge.iter().any(|r| {
                            (x + 1) as f32 > r.x
                                && (x as f32) < r.right()
                                && (y + 1) as f32 > r.y
                                && (y as f32) < r.bottom()
                        })),
                        "{label}/{operation}: start must preserve old ink outside actual caret footprint"
                    );
                    assert!(
                        samples[0].1 != samples[1].1,
                        "{label}/{operation}: intermediate native ink"
                    );
                    assert!(
                        samples[1].1 != samples[2].1,
                        "{label}/{operation}: settled native ink"
                    );
                    assert!(
                        samples[2].1 == samples[3].1,
                        "{label}/{operation}: quiet pixels"
                    );
                    if case == "range" {
                        let mut excluded = original_caret.clone();
                        caret_rects(&h.local_fragment(owner).unwrap(), scale, &mut excluded);
                        let changed_ink: Vec<_> = before
                            .chunks_exact(4)
                            .zip(samples[2].1.chunks_exact(4))
                            .enumerate()
                            .filter_map(|(i, (a, b))| {
                                let (x, y) = (i % width, i / width);
                                let caret = excluded.iter().any(|r| {
                                    (x + 1) as f32 > r.x
                                        && (x as f32) < r.right()
                                        && (y + 1) as f32 > r.y
                                        && (y as f32) < r.bottom()
                                });
                                (a != b && !caret).then_some((x, y))
                            })
                            .collect();
                        let left = changed_ink
                            .iter()
                            .map(|p| p.0)
                            .min()
                            .expect("real long old ink");
                        let right = changed_ink.iter().map(|p| p.0).max().unwrap() + 1;
                        let ink_span = (right - left) as f32 / scale;
                        assert!(
                            ink_span > viewport.width * 0.5,
                            "actual removed native ink exceeds half the real viewport"
                        );
                        assert!(
                            left as f32 >= viewport.x * scale
                                && right as f32 <= viewport.right() * scale
                        );
                        println!(
                            "U1_LONG_NATIVE_INK {label} removed_span={ink_span} pixels={left}..{right} duration={duration}"
                        );
                        std::fs::write(
                            directory.join(format!("{label}-visible-ink.json")),
                            serde_json::to_vec_pretty(
                                &serde_json::json!({"removed_bytes":full.len(),
                                "viewport_width":viewport.width,"actual_removed_ink_span":ink_span,
                                "pixel_interval":[left,right],"p7_duration":duration,"scroll":0.0}),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                    }
                    tree.node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<TextField>()
                        .unwrap()
                        .set_smooth_caret_prototype(false);
                    let plain_frame = frame(
                        &mut renderer,
                        surface,
                        &mut tree,
                        root,
                        &mut h,
                        &domains,
                        &theme,
                        start + f64::from(duration) + 0.010,
                    );
                    let canonical =
                        pixels(&mut renderer, &compositor, surface, &plain_frame, scale, id);
                    save(
                        &directory,
                        &format!("{label}-{operation}-canonical"),
                        &canonical,
                        scale,
                    );
                    assert!(
                        canonical == samples[2].1,
                        "{label}/{operation}: exact same-owner canonical pixels"
                    );
                    tree.node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<TextField>()
                        .unwrap()
                        .set_smooth_caret_prototype(true);
                    router
                        .set_frame_time_ms(((start + f64::from(duration) + 0.010) * 1000.0) as u64);
                    for (redo, history_expected) in [
                        (
                            false,
                            if operation == "append" {
                                prefix
                            } else {
                                full.as_str()
                            },
                        ),
                        (true, expected),
                    ] {
                        router.route_input_event(
                            &mut tree,
                            FusedInputEvent::TextInput {
                                window_id: WindowId::PRIMARY,
                                time,
                                text: "z".into(),
                                modifiers: Modifiers {
                                    meta: true,
                                    shift: redo,
                                    ..Default::default()
                                },
                            },
                            Some(root),
                            renderer.text_engine_mut(),
                            Some(h.animation_context()),
                            domains.primary_state_mut(),
                            &mut BindingRegistry::new(),
                        );
                        time += std::time::Duration::from_nanos(1);
                        assert_eq!(
                            field(&tree, node).text(),
                            history_expected,
                            "one immediate Undo/Redo"
                        );
                        assert_eq!(field(&tree, node).cursor_pos(), history_expected.len());
                        assert_eq!(
                            field(&tree, node).selection().anchor,
                            if case == "range" && !redo {
                                0
                            } else {
                                history_expected.len()
                            }
                        );
                        assert_eq!(field(&tree, node).selection().head, history_expected.len());
                        if case == "range" && !redo {
                            let undo = frame(
                                &mut renderer,
                                surface,
                                &mut tree,
                                root,
                                &mut h,
                                &domains,
                                &theme,
                                start + f64::from(duration) + 0.025,
                            );
                            let rgba =
                                pixels(&mut renderer, &compositor, surface, &undo, scale, id);
                            save(&directory, &format!("{label}-undo"), &rgba, scale);
                            assert!(
                                rgba == *selected_pixels.as_ref().unwrap(),
                                "same-owner selected source restored by one Undo"
                            );
                        }
                    }
                    if operation == "append" {
                        let next = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            0.090,
                        );
                        assert!(!next.needs_next_frame);
                        // Undo/Redo invalidate Layout. The .090 frame updates before
                        // rebuilding that Layout; qualify the rebuilt owner on its
                        // next ordinary Update before starting the independent edit.
                        let qualified = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            0.095,
                        );
                        before = pixels(&mut renderer, &compositor, surface, &qualified, scale, id);
                        save(
                            &directory,
                            &format!("{label}-backspace-before"),
                            &before,
                            scale,
                        );
                        assert!(before == canonical, "same source after original Undo/Redo");
                    } else {
                        frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            start + f64::from(duration) + 0.040,
                        );
                        let restored = frame(
                            &mut renderer,
                            surface,
                            &mut tree,
                            root,
                            &mut h,
                            &domains,
                            &theme,
                            start + f64::from(duration) + 0.045,
                        );
                        let rgba =
                            pixels(&mut renderer, &compositor, surface, &restored, scale, id);
                        save(
                            &directory,
                            &format!("{label}-{operation}-redo"),
                            &rgba,
                            scale,
                        );
                        assert!(
                            rgba == canonical,
                            "same-owner canonical result after one Undo/Redo"
                        );
                    }
                }
            }
        }
    }
    println!(
        "U1 long visible original Metal captures: {}",
        directory.display()
    );
}
