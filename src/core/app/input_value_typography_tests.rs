use super::*;
use crate::core::layout::EdgeInsets;
use crate::core::scene::{SceneNode, TextNode};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::runtime::actions::ActionId;
use crate::widgets::{
    button::Button, combo_box::ComboBox, flex::Flex, label::Label, slider::Slider,
    spin_box::SpinBox, text_area::TextArea, text_edit::TextEdit, text_field::TextField,
};

fn texts(nodes: &[SceneNode]) -> Vec<TextNode> {
    nodes
        .iter()
        .flat_map(|node| match node {
            SceneNode::Text(text) => vec![text.clone()],
            SceneNode::RetainedFragment(fragment) => texts(&fragment.children),
            SceneNode::Clip(clip) => texts(&clip.children),
            SceneNode::Scroll(scroll) => texts(&scroll.children),
            _ => Vec::new(),
        })
        .collect()
}

fn spin_border_geometry(
    nodes: &[SceneNode],
    edges: &mut Vec<crate::core::scene::RectNode>,
    closed: &mut usize,
) {
    for node in nodes {
        match node {
            SceneNode::Rect(rect) if rect.z_index == 3 => edges.push(rect.clone()),
            SceneNode::BorderRect(border) if border.z_index == 3 => *closed += 1,
            SceneNode::RetainedFragment(fragment) => {
                spin_border_geometry(&fragment.children, edges, closed)
            }
            SceneNode::Clip(clip) => spin_border_geometry(&clip.children, edges, closed),
            SceneNode::Scroll(scroll) => spin_border_geometry(&scroll.children, edges, closed),
            _ => {}
        }
    }
}

#[test]
fn input_value_typography_preserves_control_geometry_and_editor_alignment() {
    for (dark, scale) in [
        (false, 1.0),
        (false, 1.25),
        (false, 1.5),
        (false, 1.75),
        (false, 2.0),
        (true, 1.0),
        (true, 1.25),
        (true, 1.5),
        (true, 1.75),
        (true, 2.0),
    ] {
        let theme = if dark { Theme::dark() } else { Theme::light() };
        assert_eq!(theme.typography.font_size, 13.0);
        assert_eq!(theme.typography.input_font_size, 14.0);
        assert_eq!(theme.typography.line_height, 20.0);
        assert_eq!(theme.geometry.text_field.height, 28.0);
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real text engine");
        let bounds = RectF::new(0.0, 0.0, 640.0, 720.0);
        let mut compositor = crate::compositor::Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, crate::compositor::SceneKey::BASE);
        renderer
            .register_headless_render_target(
                surface,
                ((640.0 * scale) as u32, (720.0 * scale) as u32),
                scale,
            )
            .unwrap();
        let target = renderer.custom_render_target_snapshot(surface).unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(12.0))
                    .with_spacing(6.0),
            ),
            None,
            0,
        );
        let mut host = TextField::new().with_text("edge-router-07.ops.local");
        host.set_select_all_on_capture(true);
        let host = tree.add_child(root, Box::new(host), None, 0);
        let placeholder = tree.add_child(
            root,
            Box::new(TextField::new().with_placeholder("Target node")),
            None,
            0,
        );
        let password = tree.add_child(
            root,
            Box::new(TextField::new().with_text("secret").with_password(true)),
            None,
            0,
        );
        let spin = tree.add_child(
            root,
            Box::new(SpinBox::new().with_value(9443).with_label("Control port")),
            None,
            0,
        );
        let slider = tree.add_child(
            root,
            Box::new(
                Slider::new(0.0, 100.0)
                    .with_value(42.0)
                    .with_label("Overlay alpha")
                    .with_value_display(),
            ),
            None,
            0,
        );
        let mut choice = ComboBox::simple(vec!["Choice".into()]);
        choice
            .set_input_text("Choice".into())
            .expect("unmounted initial text");
        let combo = tree.add_child(root, Box::new(choice), None, 0);
        let mut choice = ComboBox::simple(vec!["Static choice".into()]);
        choice
            .set_input_text("Static choice".into())
            .expect("unmounted initial text");
        choice.set_text_input_enabled(false);
        let selection = tree.add_child(root, Box::new(choice), None, 0);
        let area = tree.add_child(
            root,
            Box::new(
                TextArea::new()
                    .with_text("profile = canary\ntimeout_ms = 1200")
                    .with_fixed_height(100.0),
            ),
            None,
            0,
        );
        let edit = tree.add_child(
            root,
            Box::new(TextEdit::with_text("Compatibility editor").with_fixed_height(60.0)),
            None,
            0,
        );
        let label = tree.add_child(root, Box::new(Label::new("Field label")), None, 0);
        let button = tree.add_child(root, Box::new(Button::with_text("Apply")), None, 0);
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut harness = RetainedSceneTestHarness::new();
        let icons = crate::core::icons::builtin_registry();
        let constraints = Constraints::tight(Size::new(640.0, 720.0));
        let frame = harness.frame_with_scene_inputs(
            &mut tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
            Some(target),
            Some((&domains, WindowId::PRIMARY)),
            TooltipSceneInput::Explicit(None),
            false,
        );
        assert!(frame.scene_built);
        // The shared TextField owns each interior seam. Only the six
        // horizontal/exterior button frame segments remain at layer 3.
        let spin_fragment = harness.local_fragment(tree.node(spin).unwrap().id).unwrap();
        let mut edges = Vec::new();
        let mut closed = 0;
        spin_border_geometry(&spin_fragment, &mut edges, &mut closed);
        assert_eq!(
            closed, 0,
            "adjacent closed button borders double the input seams"
        );
        assert_eq!(
            edges.len(),
            6,
            "two top, two bottom and two exterior edge segments"
        );
        let one_px = 1.0 / scale;
        for edge in edges {
            assert!(
                (edge.rect.height - one_px).abs() < 0.001
                    || (edge.rect.width - one_px).abs() < 0.001,
                "button frame segments must remain one physical pixel wide",
            );
        }
        let local = |harness: &RetainedSceneTestHarness, tree: &WidgetTree, node| {
            texts(&harness.local_fragment(tree.node(node).unwrap().id).unwrap())
        };
        for node in [host, placeholder, password, spin, slider, combo, selection] {
            assert_eq!(
                tree.get_rect(node).unwrap().height,
                28.0,
                "same logical field height at {scale}x"
            );
        }
        assert_eq!(tree.get_rect(area).unwrap().height, 100.0);
        assert_eq!(tree.get_rect(edit).unwrap().height, 60.0);
        // HAL-101 §2.4: the editable value belongs to the registered tree child,
        // while the ComboBox's local fragment contains only its shell.
        let combo_owner = tree.node(combo).unwrap().id;
        let combo_id = tree.composite_editor_id(combo_owner).unwrap();
        let combo_editor = tree.composite_child(&combo_id).unwrap();
        assert_eq!(combo_id.host(), combo_owner);
        assert_eq!(tree.node(combo_editor).unwrap().id, combo_id.child());
        assert_eq!(tree.parent(combo_editor), Some(combo));
        assert_eq!(tree.children(combo), vec![combo_editor]);
        assert!(
            tree.node(combo_editor)
                .unwrap()
                .widget
                .as_any()
                .is::<TextField>()
        );
        assert!(tree.composite_child_presentation_visible(combo_id.child()));
        let combo_source = tree.composite_editor_snapshot(&combo_id).unwrap();
        assert_eq!(combo_source.text, "Choice");
        assert!(local(&harness, &tree, combo).is_empty());
        let combo_rect = tree.get_absolute_paint_rect(combo_editor).unwrap();
        let combo_shell = tree.get_absolute_paint_rect(combo).unwrap();
        let combo_viewport = tree.get_viewport(combo_editor).unwrap();
        assert_eq!(combo_rect.x, combo_shell.x);
        assert_eq!(combo_rect.y, combo_shell.y);
        assert_eq!(combo_rect.height, combo_shell.height);
        assert!(combo_rect.width > 0.0 && combo_rect.width < combo_shell.width);
        assert!(
            combo_rect.x.max(combo_viewport.x) < combo_rect.right().min(combo_viewport.right())
        );
        assert!(
            combo_rect.y.max(combo_viewport.y) < combo_rect.bottom().min(combo_viewport.bottom())
        );
        for (node, value) in [
            (host, "edge-router-07.ops.local"),
            (placeholder, "Target node"),
            (spin, "9443"),
            (slider, "42.0"),
            (combo_editor, "Choice"),
        ] {
            let output = local(&harness, &tree, node);
            let value = output
                .iter()
                .find(|text| text.text == value)
                .unwrap_or_else(|| panic!("missing value {value:?} in {output:?}"));
            assert_eq!(value.font_size, 14.0);
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            let metrics = renderer.text_engine_mut().font_metrics(14.0);
            let expected = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                )
                - metrics.ascent;
            assert!((value.position.y - expected).abs() < 0.001);
            assert!(value.position.y >= rect.y);
            assert!(value.position.y + metrics.ascent + metrics.descent <= rect.bottom());
            if node == combo_editor {
                assert!(value.position.x >= combo_rect.x.max(combo_viewport.x));
                assert!(value.position.x < combo_rect.right().min(combo_viewport.right()));
                assert!(value.position.y >= combo_viewport.y);
                assert!(
                    value.position.y + metrics.ascent + metrics.descent <= combo_viewport.bottom()
                );
            }
        }
        for (node, caption) in [
            (spin, "Control port"),
            (slider, "Overlay alpha"),
            (selection, "Static choice"),
            (label, "Field label"),
            (button, "Apply"),
        ] {
            assert_eq!(
                local(&harness, &tree, node)
                    .iter()
                    .find(|text| text.text == caption)
                    .unwrap()
                    .font_size,
                13.0
            );
        }
        for node in [area, edit] {
            assert!(
                local(&harness, &tree, node)
                    .iter()
                    .all(|text| text.font_size == 14.0)
            );
            assert!(!local(&harness, &tree, node).is_empty());
        }
        let masks = local(&harness, &tree, password);
        assert_eq!(masks.len(), 6);
        assert!(
            masks
                .iter()
                .all(|mask| mask.font_size == 14.0 && mask.text != "secret")
        );
        for pair in masks.windows(2) {
            let advance = (pair[1].position.x - pair[0].position.x) * scale;
            assert!(
                (advance - advance.round()).abs() < 0.001,
                "password slots retain physical phase"
            );
        }
        let peer = harness
            .local_fragment(tree.node(label).unwrap().id)
            .unwrap();
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::FOCUS_NEXT,
            Some(root),
            domains.primary_state_mut(),
            renderer.text_engine_mut(),
            None
        ));
        assert_eq!(
            domains.primary_state().focus_manager.focused(),
            Some(tree.node(host).unwrap().id)
        );
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            domains.primary_state_mut(),
            renderer.text_engine_mut(),
            None
        ));
        harness.frame_with_scene_inputs(
            &mut tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
            Some(target),
            Some((&domains, WindowId::PRIMARY)),
            TooltipSceneInput::Explicit(None),
            false,
        );
        let selected = local(&harness, &tree, host);
        let metrics = renderer.text_engine_mut().font_metrics(14.0);
        assert!(
            selected.iter().any(|text| text
                .backgrounds
                .iter()
                .any(|background| background.width > 20.0
                    && (background.height - metrics.ascent - metrics.descent).abs() < 0.001)),
            "selection follows the enlarged cache metrics"
        );
        assert!(
            selected
                .iter()
                .any(
                    |text| text.backgrounds.iter().any(|background| (background.width
                        - theme.typography.caret_thickness)
                        .abs()
                        < 0.001
                        && (background.height - metrics.ascent - metrics.descent).abs() < 0.001)
                ),
            "captured caret shares value metrics"
        );
        assert!(Arc::ptr_eq(
            &peer,
            &harness
                .local_fragment(tree.node(label).unwrap().id)
                .unwrap()
        ));
        router.request_focus_for_widget(&mut tree, combo_owner, domains.primary_state_mut());
        assert_eq!(
            domains.primary_state().focus_manager.focused(),
            Some(combo_owner)
        );
        harness.frame_with_scene_inputs(
            &mut tree,
            root,
            constraints,
            &theme,
            renderer.text_engine_mut(),
            &icons,
            Some(target),
            Some((&domains, WindowId::PRIMARY)),
            TooltipSceneInput::Explicit(None),
            false,
        );
        assert_eq!(tree.composite_editor_id(combo_owner).unwrap(), combo_id);
        assert_eq!(tree.composite_child(&combo_id), Some(combo_editor));
        let focused_source = tree.composite_editor_snapshot(&combo_id).unwrap();
        assert_eq!(focused_source.text, combo_source.text);
        assert_eq!(focused_source.revision, combo_source.revision);
        assert_eq!(tree.get_absolute_paint_rect(combo_editor), Some(combo_rect));
        let focused_output = local(&harness, &tree, combo_editor);
        let focused_value = focused_output
            .iter()
            .find(|text| text.text == "Choice")
            .unwrap();
        assert_eq!(focused_value.font_size, 14.0);
        let metrics = renderer.text_engine_mut().font_metrics(14.0);
        let expected = combo_rect.y
            + compute_control_text_baseline_y(
                combo_rect.height,
                metrics.ascent,
                metrics.descent,
                theme.typography.baseline_offset,
            )
            - metrics.ascent;
        assert!((focused_value.position.y - expected).abs() < 0.001);
        assert!(focused_value.position.y >= combo_rect.y.max(combo_viewport.y));
        assert!(
            focused_value.position.y + metrics.ascent + metrics.descent
                <= combo_rect.bottom().min(combo_viewport.bottom())
        );
        println!(
            "INPUT TYPOGRAPHY {} {scale}x: value14, captions13; field28, baseline/placeholder/password/caret/selection, retained peer and registered ComboBox child Choice before/after Focus PASS",
            if dark { "dark" } else { "light" }
        );
    }
}

fn export_compact_input_scene(
    renderer: &mut WgpuRenderer,
    compositor: &crate::compositor::Compositor,
    surface: crate::compositor::SurfaceId,
    nodes: Vec<SceneNode>,
    theme: &Theme,
    scale: f32,
    name: &str,
) {
    use std::io::Write;
    let mut scene = crate::core::scene::SceneGraph::new();
    scene.root = nodes;
    scene.clear_color = theme.colors.surfaces.base0.into();
    let mut scenes = HashMap::new();
    scenes.insert(surface, scene);
    let plan = crate::renderer::DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut crate::renderer::mesh_cache::MeshCache::new(),
            |_| scale,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    renderer.advance_frame();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    assert!(!matches!(
        compiled.text_realization(surface).unwrap().outcome(),
        crate::renderer::TextRealizationOutcome::Unavailable(_)
    ));
    let gpu = renderer.gpu_resources();
    let (width, height) = compiled.get_target(surface).unwrap().physical_size;
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("compact input evidence"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render = renderer
        .encode_compiled_target_for_test(
            &compiled,
            surface,
            &output.create_view(&Default::default()),
        )
        .unwrap();
    let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("compact input readback"),
        size: u64::from(stride * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu.device.create_command_encoder(&Default::default());
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    renderer.submit_compiled_target_for_test(render);
    gpu.queue.submit([copy.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        gpu.device.poll(wgpu::PollType::Poll).unwrap();
        match receiver.try_recv() {
            Ok(result) => {
                result.unwrap();
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                assert!(
                    Instant::now() < deadline,
                    "compact input GPU readback deadline"
                );
                std::thread::yield_now();
            }
            Err(error) => panic!("compact input readback: {error}"),
        }
    }
    let directory = std::path::Path::new("target/compact-input-appearance");
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(format!("{name}.ppm"));
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "P6\n{width} {height}\n255\n").unwrap();
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .expect("mapped readback range");
    for row in mapped.chunks_exact(stride as usize).take(height as usize) {
        for pixel in row[..(width * 4) as usize].chunks_exact(4) {
            let rgb = match gpu.format {
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                    [pixel[2], pixel[1], pixel[0]]
                }
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                    [pixel[0], pixel[1], pixel[2]]
                }
                format => panic!("unsupported compact input evidence format {format:?}"),
            };
            file.write_all(&rgb).unwrap();
        }
    }
    drop(mapped);
    buffer.unmap();
    println!("actual GPU output: {} ({width}x{height})", path.display());
}

#[test]
fn standard_inputs_preserve_metrics_and_routed_ownership() {
    use crate::core::event::{FusedInputEvent, Modifiers, PointerButton, PointerDevice};
    use crate::core::theme::DensityMode;
    use crate::runtime::binding_registry::BindingRegistry;

    fn contains_local(nodes: &[SceneNode], local: &Arc<[SceneNode]>) -> bool {
        nodes.iter().any(|node| match node {
            SceneNode::RetainedFragment(fragment) => {
                Arc::ptr_eq(&fragment.children, local) || contains_local(&fragment.children, local)
            }
            SceneNode::Clip(clip) => contains_local(&clip.children, local),
            SceneNode::Scroll(scroll) => contains_local(&scroll.children, local),
            SceneNode::Layer(layer) => contains_local(&layer.children, local),
            _ => false,
        })
    }

    for dark in [false, true] {
        for scale in [1.0, 2.0] {
            for (density, density_name) in [
                (DensityMode::Compact, "compact24"),
                (DensityMode::Regular, "regular28"),
                (DensityMode::Intermediate30, "intermediate30"),
                (DensityMode::Comfortable, "comfortable32"),
            ] {
                let theme = if dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                let mut renderer =
                    pollster::block_on(WgpuRenderer::new_headless()).expect("real GPU required");
                let bounds = RectF::new(0.0, 0.0, 320.0, 240.0);
                let mut compositor = crate::compositor::Compositor::new(bounds);
                let surface =
                    compositor.create_surface(0, bounds, crate::compositor::SceneKey::BASE);
                renderer
                    .register_headless_render_target(
                        surface,
                        ((320.0 * scale) as u32, (240.0 * scale) as u32),
                        scale,
                    )
                    .unwrap();
                let target = renderer.custom_render_target_snapshot(surface).unwrap();
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(
                        Flex::column()
                            .with_padding(EdgeInsets::all(12.0))
                            .with_spacing(8.0),
                    ),
                    None,
                    0,
                );
                let changes = Rc::new(Cell::new(0));
                let observed = changes.clone();
                let mut field = TextField::new().with_text("Node name");
                field.set_select_all_on_capture(true);
                field.set_on_change(move |_| observed.set(observed.get() + 1));
                let field = tree.add_child(root, Box::new(field), None, 0);
                let mut combo = ComboBox::simple(vec!["Canary".into(), "Stable".into()]);
                combo
                    .set_input_text("Canary".into())
                    .expect("unmounted initial text");
                let editable = tree.add_child(root, Box::new(combo), None, 0);
                let mut combo = ComboBox::simple(vec!["Canary".into(), "Stable".into()]);
                combo
                    .set_input_text("Stable".into())
                    .expect("unmounted initial text");
                combo.set_text_input_enabled(false);
                let selection = tree.add_child(root, Box::new(combo), None, 0);
                let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 0);
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                let mut harness = RetainedSceneTestHarness::new();
                harness.advance_transition_time(0.0);
                let mut time_ms = 0_u64;
                macro_rules! advance {
                    ($elapsed:expr) => {{
                        time_ms += $elapsed;
                        harness.advance_transition_time(time_ms as f64 / 1000.0);
                        router.set_frame_time_ms(time_ms);
                    }};
                }
                let icons = crate::core::icons::builtin_registry();
                let constraints = Constraints::tight(Size::new(320.0, 240.0));
                macro_rules! frame {
                    () => {
                        harness.frame_with_scene_inputs(
                            &mut tree,
                            root,
                            constraints,
                            &theme,
                            renderer.text_engine_mut(),
                            &icons,
                            Some(target),
                            Some((&domains, WindowId::PRIMARY)),
                            TooltipSceneInput::Explicit(None),
                            false,
                        )
                    };
                }
                let first = frame!();
                let peer_owner = tree.node(peer).unwrap().id;
                let rects = [field, editable, selection]
                    .map(|node| tree.get_absolute_paint_rect(node).unwrap());
                eprintln!(
                    "COMPACT_INPUT_TREE_TEXT case={} {scale}x {density_name}",
                    if dark { "dark" } else { "light" }
                );
                for node in [field, editable, selection] {
                    let rect = tree.get_absolute_paint_rect(node).unwrap();
                    assert_eq!(rect.height, theme.geometry.text_field.height);
                    let host_owner = tree.node(node).unwrap().id;
                    let text_node = if node == editable {
                        let id = tree
                            .composite_editor_id(host_owner)
                            .expect("current editable ComboBox relation");
                        let child = tree
                            .composite_child(&id)
                            .expect("mounted tree-owned TextField");
                        assert_eq!(id.host(), host_owner);
                        assert_eq!(tree.node(child).unwrap().id, id.child());
                        assert_eq!(tree.parent(child), Some(node));
                        assert!(tree.composite_child_presentation_visible(id.child()));
                        assert!(
                            texts(&harness.local_fragment(host_owner).unwrap()).is_empty(),
                            "editable host paints shell, not duplicate child text"
                        );
                        let local = harness
                            .local_fragment(id.child())
                            .expect("current child text fragment");
                        assert!(
                            contains_local(&harness.cache.entries[&host_owner].composed, &local),
                            "the child's actual local Arc must be in the host composed scene"
                        );
                        assert_eq!(
                            tree.composite_editor_snapshot(&id).unwrap().text,
                            tree.node(node)
                                .unwrap()
                                .widget
                                .as_any()
                                .downcast_ref::<ComboBox>()
                                .unwrap()
                                .input_text()
                        );
                        let child_rect = tree.get_absolute_paint_rect(child).unwrap();
                        assert_eq!(child_rect.x, rect.x);
                        assert_eq!(child_rect.y, rect.y);
                        assert_eq!(child_rect.height, rect.height);
                        assert!(child_rect.width > 0.0 && child_rect.width < rect.width);
                        child
                    } else {
                        // Standalone TextField and noneditable ComboBox author their own text.
                        node
                    };
                    let paint_rect = tree.get_absolute_paint_rect(text_node).unwrap();
                    let viewport = tree.get_viewport(text_node).unwrap();
                    let output = texts(
                        &harness
                            .local_fragment(tree.node(text_node).unwrap().id)
                            .unwrap(),
                    );
                    assert!(!output.is_empty());
                    for text in output {
                        let metrics = renderer.text_engine_mut().font_metrics(text.font_size);
                        eprintln!(
                            "COMPACT_INPUT_TREE_TEXT role={} host={} painter={} size={} rect={paint_rect:?} viewport={viewport:?}",
                            if node == field {
                                "standalone"
                            } else if node == editable {
                                "editable-child"
                            } else {
                                "static-caption"
                            },
                            host_owner.0,
                            tree.node(text_node).unwrap().id.0,
                            text.font_size
                        );
                        assert!(text.position.y >= rect.y);
                        assert!(
                            text.position.y + metrics.ascent + metrics.descent <= rect.bottom()
                        );
                        if node == editable {
                            assert!(text.position.x >= paint_rect.x.max(viewport.x));
                            assert!(text.position.x < paint_rect.right().min(viewport.right()));
                            assert!(text.position.y >= paint_rect.y.max(viewport.y));
                            assert!(
                                text.position.y + metrics.ascent + metrics.descent
                                    <= paint_rect.bottom().min(viewport.bottom())
                            );
                        }
                    }
                }
                let prefix = format!(
                    "{}-{}x-{density_name}",
                    if dark { "dark" } else { "light" },
                    scale as u32,
                );
                export_compact_input_scene(
                    &mut renderer,
                    &compositor,
                    surface,
                    first.scene_root,
                    &theme,
                    scale,
                    &format!("{prefix}-rest"),
                );
                let owner = tree.node(field).unwrap().id;
                router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
                assert!(router.handle_engine_action(
                    &mut tree,
                    ActionId::ENTER_WIDGET,
                    Some(root),
                    domains.primary_state_mut(),
                    renderer.text_engine_mut(),
                    None
                ));
                let captured = frame!();
                assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
                assert_eq!(changes.get(), 0);
                export_compact_input_scene(
                    &mut renderer,
                    &compositor,
                    surface,
                    captured.scene_root,
                    &theme,
                    scale,
                    &format!("{prefix}-capture"),
                );
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::TextInput {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        text: "Canary".into(),
                        modifiers: Modifiers::default(),
                    },
                    Some(root),
                    renderer.text_engine_mut(),
                    None,
                    domains.primary_state_mut(),
                    &mut bindings,
                );
                frame!();
                let value = tree
                    .node(field)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<TextField>()
                    .unwrap();
                assert_eq!(value.text(), "Canary");
                assert_eq!(value.cursor_pos(), "Canary".len());
                assert_eq!(value.selection().anchor, value.selection().head);
                assert_eq!(changes.get(), 1);
                for node in [editable, selection] {
                    advance!(100);
                    let owner = tree.node(node).unwrap().id;
                    let before = tree
                        .node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ComboBox>()
                        .unwrap()
                        .input_text()
                        .to_owned();
                    let rect = tree.get_absolute_paint_rect(node).unwrap();
                    let pos = crate::core::math::Point::new(
                        rect.right() - 4.0,
                        rect.y + rect.height * 0.5,
                    );
                    router.route_input_event(
                        &mut tree,
                        FusedInputEvent::PointerDown {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            pos,
                            device: PointerDevice::Mouse,
                            button: PointerButton::Primary,
                            click_count: 1,
                        },
                        Some(root),
                        renderer.text_engine_mut(),
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    );
                    let opening = frame!();
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        opening.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-opening-start",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    advance!(10);
                    let opening = frame!();
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        opening.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-opening-interior",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    advance!(40);
                    let opened = frame!();
                    assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Capture
                    );
                    assert!(!harness.overlay_fragment(owner).unwrap().is_empty());
                    assert_eq!(
                        tree.node(node)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<ComboBox>()
                            .unwrap()
                            .input_text(),
                        before
                    );
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        opened.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-open",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    assert!(router.handle_engine_action(
                        &mut tree,
                        ActionId::RELEASE_CAPTURE,
                        Some(root),
                        domains.primary_state_mut(),
                        renderer.text_engine_mut(),
                        Some(harness.animation_context())
                    ));
                    frame!();
                    advance!(50);
                    let cancelled = frame!();
                    assert!(harness.overlay_fragment(owner).unwrap().is_empty());
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Navigation
                    );
                    assert!(
                        tree.node_mut(node)
                            .unwrap()
                            .widget
                            .as_any_mut()
                            .downcast_mut::<ComboBox>()
                            .unwrap()
                            .take_committed()
                            .is_none()
                    );
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        cancelled.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-cancelled-settled",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    assert_eq!(
                        tree.node(node)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<ComboBox>()
                            .unwrap()
                            .input_text(),
                        before
                    );
                    advance!(50);
                    router.route_input_event(
                        &mut tree,
                        FusedInputEvent::PointerDown {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            pos,
                            device: PointerDevice::Mouse,
                            button: PointerButton::Primary,
                            click_count: 1,
                        },
                        Some(root),
                        renderer.text_engine_mut(),
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    );
                    frame!();
                    advance!(50);
                    frame!();
                    let index = if node == editable { 1.0 } else { 0.0 };
                    let row = crate::core::math::Point::new(
                        rect.x + 20.0,
                        rect.bottom() + index * 24.0 + 12.0,
                    );
                    for down in [true, false] {
                        let event = if down {
                            FusedInputEvent::PointerDown {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos: row,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary,
                                click_count: 1,
                            }
                        } else {
                            FusedInputEvent::PointerUp {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos: row,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary,
                            }
                        };
                        router.route_input_event(
                            &mut tree,
                            event,
                            Some(root),
                            renderer.text_engine_mut(),
                            Some(harness.animation_context()),
                            domains.primary_state_mut(),
                            &mut bindings,
                        );
                    }
                    let closing = frame!();
                    assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Navigation
                    );
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        closing.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-committed-start",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    advance!(10);
                    let closing = frame!();
                    assert!(!harness.overlay_fragment(owner).unwrap().is_empty());
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        closing.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-committed-interior",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    advance!(40);
                    let committed = frame!();
                    assert!(committed.scene_built);
                    assert!(harness.overlay_fragment(owner).unwrap().is_empty());
                    assert!(!tree.node(node).unwrap().widget.needs_animation());
                    assert_eq!(
                        tree.node(node)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<ComboBox>()
                            .unwrap()
                            .input_text(),
                        if node == editable { "Stable" } else { "Canary" }
                    );
                    let settled_scene = committed.scene_root.clone();
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        committed.scene_root,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-committed",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    let settled_local = harness.local_fragment(owner).unwrap();
                    let settled_peer = harness.local_fragment(peer_owner).unwrap();
                    let value = tree
                        .node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<ComboBox>()
                        .unwrap();
                    assert_eq!(
                        value.committed_value_string().as_deref(),
                        Some(if node == editable { "Stable" } else { "Canary" })
                    );
                    assert!(value.take_committed().is_some());
                    advance!(50);
                    let quiet = frame!();
                    assert!(
                        !quiet.scene_built,
                        "settled close must not demand a repair frame"
                    );
                    assert!(Arc::ptr_eq(
                        &settled_local,
                        &harness.local_fragment(owner).unwrap()
                    ));
                    assert!(Arc::ptr_eq(
                        &settled_peer,
                        &harness.local_fragment(peer_owner).unwrap()
                    ));
                    assert!(
                        tree.node_mut(node)
                            .unwrap()
                            .widget
                            .as_any_mut()
                            .downcast_mut::<ComboBox>()
                            .unwrap()
                            .take_committed()
                            .is_none()
                    );
                    // An idle frame reuses the last presented root; its empty work result is not a scene.
                    export_compact_input_scene(
                        &mut renderer,
                        &compositor,
                        surface,
                        settled_scene,
                        &theme,
                        scale,
                        &format!(
                            "{prefix}-{}-committed-quiet",
                            if node == editable {
                                "editable"
                            } else {
                                "selection"
                            }
                        ),
                    );
                    tree.mark_paint_dirty(node);
                    frame!();
                    assert_eq!(
                        format!("{settled_local:?}"),
                        format!("{:?}", harness.local_fragment(owner).unwrap()),
                        "terminal main projection must equal a fresh closed-owner repaint",
                    );
                }
                assert_eq!(
                    [field, editable, selection]
                        .map(|node| tree.get_absolute_paint_rect(node).unwrap()),
                    rects
                );
                let settled_peer = harness.local_fragment(peer_owner).unwrap();
                frame!();
                assert!(Arc::ptr_eq(
                    &settled_peer,
                    &harness.local_fragment(peer_owner).unwrap()
                ));
            }
        }
    }
}
