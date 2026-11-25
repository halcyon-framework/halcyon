use super::*;

#[test]
fn time_picker_default_gpu_evidence() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::{Alignment, EdgeInsets};
    use crate::core::text_engine::{TextEngine, compute_control_text_baseline_y};
    use crate::core::theme::DensityMode;
    use crate::runtime::actions::ActionId;
    use crate::widgets::time_picker::{TimeDisplayMode, TimePicker, TimeValue};
    use std::io::Write;

    const W: u32 = 240;
    const H: u32 = 80;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/time-picker-default-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(
        manifest,
        "theme\tdensity\tscale\tvariant\tstate\tallocation\tface\tvalue\tworking\tfile"
    )
    .unwrap();
    println!("TimePicker actual GPU evidence: {}", directory.display());
    let write_ppm = |path: &std::path::Path, width: u32, height: u32, pixels: &[u8]| {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels.chunks_exact(4) {
            file.write_all(&pixel[..3]).unwrap();
        }
    };
    for (dark, theme_name) in [(false, "light"), (true, "dark")] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, scale)
                .expect("TimePicker evidence requires a real adapter and GPU readback");
            renderer.text_engine_mut().set_scale(scale);
            let validation_scope = renderer
                .gpu_resources()
                .device
                .push_error_scope(wgpu::ErrorFilter::Validation);
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let width = (W as f32 * scale) as u32;
            let height = (H as f32 * scale) as u32;
            let states = ["rest", "focus", "capture", "committed-quiet"];
            let mut contact = vec![0_u8; (width * height * 4 * 8 * 4) as usize];
            for (density_row, (density, density_name)) in [
                (DensityMode::Compact, "compact24"),
                (DensityMode::Regular, "regular28"),
                (DensityMode::Intermediate30, "intermediate30"),
                (DensityMode::Comfortable, "comfortable32"),
            ]
            .into_iter()
            .enumerate()
            {
                let theme = if dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                for (variant_row, (seconds, variant)) in [(false, "24h"), (true, "12h-seconds")]
                    .into_iter()
                    .enumerate()
                {
                    let initial = TimeValue::new(14, 30, seconds.then_some(45));
                    let committed = TimeValue::new(14, 31, initial.second);
                    let commits = Rc::new(Cell::new(0));
                    let observed = commits.clone();
                    let mut tree = WidgetTree::new();
                    let root = tree.create_root(
                        Box::new(
                            Flex::column()
                                .with_padding(EdgeInsets::all(16.0))
                                .with_align_items(Alignment::Start),
                        ),
                        None,
                        0,
                    );
                    let picker = TimePicker::new(initial)
                        .with_seconds(seconds)
                        .with_display_mode(if seconds {
                            TimeDisplayMode::H12
                        } else {
                            TimeDisplayMode::H24
                        })
                        .on_commit(move |_| observed.set(observed.get() + 1));
                    let node = tree.add_child(root, Box::new(picker), None, 0);
                    let owner = tree.widget_id_for_node(node).unwrap();
                    let mut domains = InteractionDomains::new();
                    let mut router = EventRouter::new();
                    let mut bindings = BindingRegistry::new();
                    let mut harness = RetainedSceneTestHarness::new();
                    let icons = builtin_registry();
                    let constraints = Constraints::tight(Size::new(W as f32, H as f32));
                    let mut allocation = None;
                    macro_rules! frame {
                        () => {
                            harness.frame_with_optional_custom_render_target(
                                &mut tree,
                                root,
                                constraints,
                                &theme,
                                renderer.text_engine_mut(),
                                &icons,
                                Some(target),
                                Some((&domains, WindowId::PRIMARY)),
                            )
                        };
                    }
                    macro_rules! key {
                        ($code:expr) => {
                            router.route_input_event(
                                &mut tree,
                                FusedInputEvent::KeyDown {
                                    window_id: WindowId::PRIMARY,
                                    time: Instant::now(),
                                    key_code: $code,
                                    physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                                    reservation: None,
                                    modifiers: Modifiers::default(),
                                    repeat: false,
                                },
                                Some(root),
                                renderer.text_engine_mut(),
                                None,
                                domains.primary_state_mut(),
                                &mut bindings,
                            )
                        };
                    }
                    for (column, state) in states.into_iter().enumerate() {
                        let time_ms = column as u64 * 100;
                        router.set_frame_time_ms(time_ms);
                        harness.advance_transition_time(time_ms as f64 / 1000.0);
                        match state {
                            "focus" => router.request_focus_for_widget(
                                &mut tree,
                                owner,
                                domains.primary_state_mut(),
                            ),
                            "capture" => {
                                assert!(router.handle_engine_action(
                                    &mut tree,
                                    ActionId::ENTER_WIDGET,
                                    Some(root),
                                    domains.primary_state_mut(),
                                    renderer.text_engine_mut(),
                                    None,
                                ));
                                key!(KeyCode::Right);
                                key!(KeyCode::Up);
                            }
                            "committed-quiet" => {
                                key!(KeyCode::Enter);
                            }
                            _ => {}
                        }
                        let mut frame = frame!();
                        assert!(
                            frame.scene_built,
                            "state transition produces a current scene"
                        );
                        if state == "committed-quiet" {
                            router.set_frame_time_ms(time_ms + 100);
                            harness.advance_transition_time((time_ms + 100) as f64 / 1000.0);
                            let quiet = frame!();
                            assert!(!quiet.needs_next_frame);
                            if quiet.scene_built {
                                frame = quiet;
                            }
                        }
                        let allocated = tree.get_absolute_paint_rect(node).unwrap();
                        if let Some(before) = allocation {
                            assert_eq!(allocated, before, "state preserves canonical allocation");
                        } else {
                            allocation = Some(allocated);
                        }
                        let fragment = harness.local_fragment(owner).unwrap();
                        let mut face: Option<RectF> = None;
                        visit_scene_nodes(&fragment, &mut |entry| {
                            let candidate = match entry {
                                SceneNode::Rect(rect) if rect.z_index == LAYER_CONTENT => {
                                    Some(rect.rect)
                                }
                                SceneNode::RoundedRect(rect) if rect.z_index == LAYER_CONTENT => {
                                    Some(rect.rect)
                                }
                                _ => None,
                            };
                            if let Some(candidate) = candidate {
                                if face.is_none_or(|current| candidate.width > current.width) {
                                    face = Some(candidate);
                                }
                            }
                        });
                        let face = face.expect("TimePicker emits its standalone face");
                        let geom = &theme.geometry.time_picker;
                        let expected_width = if seconds {
                            geom.column_width * 3.0 + geom.separator_width * 2.0 + geom.ampm_width
                        } else {
                            geom.column_width * 2.0 + geom.separator_width
                        };
                        assert_eq!(face.width, expected_width);
                        assert_eq!(face.height, geom.column_height);
                        assert_eq!((face.x, face.y), (allocated.x, allocated.y));
                        let value = tree
                            .node(node)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<TimePicker>()
                            .unwrap();
                        let expected_value = if state == "committed-quiet" {
                            committed
                        } else {
                            initial
                        };
                        let expected_working = if column >= 2 { committed } else { initial };
                        assert_eq!(value.value(), expected_value);
                        assert_eq!(value.working(), expected_working);
                        assert_eq!(commits.get(), usize::from(state == "committed-quiet"));
                        let focused = domains.primary_state().focus_manager.focused();
                        assert_eq!(focused, (state != "rest").then_some(owner));
                        assert_eq!(
                            domains.primary_state().keyboard_mode,
                            if state == "capture" {
                                KeyboardAuthorityMode::Capture
                            } else {
                                KeyboardAuthorityMode::Navigation
                            }
                        );
                        let mut captions = Vec::new();
                        visit_scene_nodes(&fragment, &mut |entry| {
                            if let SceneNode::Text(text) = entry {
                                captions.push((text.text.clone(), text.position.y, text.font_size));
                            }
                        });
                        let minute = format!("{:02}", expected_working.minute);
                        let expected = if seconds {
                            vec![" 2", ":", &minute, ":", "45", "PM"]
                        } else {
                            vec!["14", ":", &minute]
                        };
                        assert_eq!(
                            captions
                                .iter()
                                .map(|caption| caption.0.as_str())
                                .collect::<Vec<_>>(),
                            expected
                        );
                        for (_, y, font_size) in &captions {
                            assert_eq!(*font_size, theme.typography.font_size);
                            let metrics = renderer.text_engine_mut().font_metrics(*font_size);
                            let expected_baseline = face.y
                                + compute_control_text_baseline_y(
                                    face.height,
                                    metrics.ascent,
                                    metrics.descent,
                                    theme.typography.baseline_offset,
                                );
                            assert!((*y + metrics.ascent - expected_baseline).abs() < 0.001);
                            assert!(*y >= face.y);
                            assert!(*y + metrics.ascent + metrics.descent <= face.bottom());
                        }
                        let mut scene = control_order_scene(&frame);
                        scene.clear_color = theme.colors.surfaces.base0.into();
                        let plan = DrawPlanBuilder::new()
                            .build_multi(
                                &HashMap::from([(surface, scene)]),
                                compositor.snapshot_multi(),
                                &mut MeshCache::new(),
                                |_| scale,
                                0,
                            )
                            .expect("scene must satisfy DrawPlan Build admission");
                        let compiled = renderer
                            .compile_multi(&plan)
                            .unwrap()
                            .expect_ready("native Compile must be ready");
                        assert!(
                            compiled
                                .text_realization(surface)
                                .unwrap()
                                .outcome()
                                .is_complete()
                        );
                        let pixels = read_compiled_surface(&renderer, &compiled, surface)
                            .expect("TimePicker actual GPU output");
                        let filename =
                            format!("{theme_name}-{density_name}-{scale}x-{variant}-{state}.ppm");
                        write_ppm(&directory.join(&filename), width, height, &pixels);
                        writeln!(
                            manifest,
                            "{theme_name}\t{density_name}\t{scale}\t{variant}\t{state}\t{allocated:?}\t{face:?}\t{expected_value:?}\t{expected_working:?}\t{filename}"
                        )
                        .unwrap();
                        let row = density_row * 2 + variant_row;
                        for y in 0..height as usize {
                            let destination =
                                ((row * height as usize + y) * width as usize * states.len()
                                    + column * width as usize)
                                    * 4;
                            contact[destination..destination + width as usize * 4].copy_from_slice(
                                &pixels[y * width as usize * 4..(y + 1) * width as usize * 4],
                            );
                        }
                    }
                }
            }
            assert!(
                pollster::block_on(validation_scope.pop()).is_none()
            );
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-contact.ppm")),
                width * 4,
                height * 8,
                &contact,
            );
        }
    }
}

#[test]
fn tag_input_default_gpu_evidence() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::{Alignment, EdgeInsets};
    use crate::core::text_engine::TextEngine;
    use crate::core::theme::DensityMode;
    use crate::runtime::actions::ActionId;
    use crate::widgets::tag_input::{TagCreationMode, TagDisplayMode, TagEntry, TagInputPolicy};
    use std::io::Write;

    const W: u32 = 360;
    const H: u32 = 280;
    const BASIC: [&str; 4] = ["rest", "focus", "capture", "filtered"];
    const EXTRA: [&str; 5] = [
        "toggled",
        "no-match",
        "settled",
        "validation-error",
        "custom-created",
    ];
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/tag-input-default-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(
        manifest,
        "theme\tdensity\tscale\tstate\tallocations\tselection\tcustom\tchanges\tfocus\tcapture\tfile"
    )
    .unwrap();
    println!("TagInput actual GPU evidence: {}", directory.display());
    let write_ppm = |path: &std::path::Path, width: u32, height: u32, pixels: &[u8]| {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels.chunks_exact(4) {
            file.write_all(&pixel[..3]).unwrap();
        }
    };
    let mut frames_written = 0;
    for (dark, theme_name) in [(false, "light"), (true, "dark")] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, scale)
                .expect("TagInput evidence requires an actual GPU adapter and readback");
            renderer.text_engine_mut().set_scale(scale);
            let validation_scope = renderer
                .gpu_resources()
                .device
                .push_error_scope(wgpu::ErrorFilter::Validation);
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let width = (W as f32 * scale) as u32;
            let height = (H as f32 * scale) as u32;
            let mut contact = vec![0_u8; (width * height * 4 * 4 * 4) as usize];
            let mut detail_contact = vec![0_u8; (width * height * 4 * 5) as usize];
            for (row, (density, density_name)) in [
                (DensityMode::Compact, "compact24"),
                (DensityMode::Regular, "regular28"),
                (DensityMode::Intermediate30, "intermediate30"),
                (DensityMode::Comfortable, "comfortable32"),
            ]
            .into_iter()
            .enumerate()
            {
                let theme = if dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                let items: Vec<_> = (0..12)
                    .map(|id| TagItem {
                        id,
                        label: match id {
                            0 => "gpu".into(),
                            1 => "release".into(),
                            2 => "telemetry".into(),
                            3 => "audit".into(),
                            _ => format!("worker-{id}"),
                        },
                        group: None,
                        enabled: id != 11,
                    })
                    .collect();
                let changes = Rc::new(Cell::new(0));
                let observed = changes.clone();
                let main = TagInput::new()
                    .with_items(items.clone())
                    .with_selection(&[0, 1])
                    .with_tag_creation(TagCreationMode::Hybrid)
                    .with_placeholder("Select or type")
                    .on_change(move |_| observed.set(observed.get() + 1));
                let custom_changes = Rc::new(Cell::new(0));
                let observed = custom_changes.clone();
                let custom = TagInput::new()
                    .with_tag_creation(TagCreationMode::FreeFormOnly)
                    .with_tag_validator(|text| text.chars().count() >= 2)
                    .with_placeholder("Create a tag")
                    .on_change(move |_| observed.set(observed.get() + 1));
                let controls = [
                    main,
                    TagInput::new()
                        .with_items(items.clone())
                        .with_selection(&[0, 1, 2])
                        .with_display_mode(TagDisplayMode::CountBadge),
                    TagInput::new()
                        .with_items(items.clone())
                        .with_selection(&[0, 1, 2])
                        .with_display_mode(TagDisplayMode::Summary),
                    TagInput::new()
                        .with_items(items.clone())
                        .with_selection(&[0, 1])
                        .with_policy(TagInputPolicy::Locked),
                    TagInput::new()
                        .with_items(items)
                        .with_selection(&[0, 1, 2])
                        .with_display_mode(TagDisplayMode::Summary)
                        .with_policy(TagInputPolicy::ReadOnly),
                    custom,
                ];
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(
                        Flex::column()
                            .with_padding(EdgeInsets::all(14.0))
                            .with_spacing(8.0)
                            .with_align_items(Alignment::Stretch),
                    ),
                    None,
                    0,
                );
                let nodes: Vec<_> = controls
                    .into_iter()
                    .enumerate()
                    .map(|(order, control)| {
                        tree.add_child(root, Box::new(control), None, order as i16)
                    })
                    .collect();
                let owner = tree.widget_id_for_node(nodes[0]).unwrap();
                let custom_owner = tree.widget_id_for_node(nodes[5]).unwrap();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                let mut harness = RetainedSceneTestHarness::new();
                let icons = builtin_registry();
                let constraints = Constraints::tight(Size::new(W as f32, H as f32));
                let mut allocation = None;
                macro_rules! frame {
                    () => {
                        harness.frame_with_optional_custom_render_target(
                            &mut tree,
                            root,
                            constraints,
                            &theme,
                            renderer.text_engine_mut(),
                            &icons,
                            Some(target),
                            Some((&domains, WindowId::PRIMARY)),
                        )
                    };
                }
                macro_rules! route {
                    ($event:expr) => {
                        router.route_input_event(
                            &mut tree,
                            $event,
                            Some(root),
                            renderer.text_engine_mut(),
                            None,
                            domains.primary_state_mut(),
                            &mut bindings,
                        )
                    };
                }
                macro_rules! key {
                    ($code:expr) => {
                        route!(FusedInputEvent::KeyDown {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            key_code: $code,
                            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                            reservation: None,
                            modifiers: Modifiers::default(),
                            repeat: false,
                        })
                    };
                }
                macro_rules! text {
                    ($text:expr) => {
                        route!(FusedInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            text: $text.into(),
                            modifiers: Modifiers::default(),
                        })
                    };
                }
                macro_rules! action {
                    ($action:expr) => {
                        router.handle_engine_action(
                            &mut tree,
                            $action,
                            Some(root),
                            domains.primary_state_mut(),
                            renderer.text_engine_mut(),
                            None,
                        )
                    };
                }
                let states: Vec<_> = BASIC
                    .into_iter()
                    .chain(
                        (density == DensityMode::Intermediate30)
                            .then_some(EXTRA)
                            .into_iter()
                            .flatten(),
                    )
                    .collect();
                for (column, state) in states.into_iter().enumerate() {
                    let time_ms = column as u64 * 100;
                    router.set_frame_time_ms(time_ms);
                    harness.advance_transition_time(time_ms as f64 / 1000.0);
                    match state {
                        "focus" => router.request_focus_for_widget(
                            &mut tree,
                            owner,
                            domains.primary_state_mut(),
                        ),
                        "capture" => assert!(action!(ActionId::ENTER_WIDGET)),
                        "filtered" => {
                            text!("tel");
                        }
                        "toggled" => {
                            key!(KeyCode::Enter);
                        }
                        "no-match" => {
                            text!("new-tag");
                        }
                        "settled" => assert!(action!(ActionId::RELEASE_CAPTURE)),
                        "validation-error" => {
                            router.request_focus_for_widget(
                                &mut tree,
                                custom_owner,
                                domains.primary_state_mut(),
                            );
                            assert!(action!(ActionId::ENTER_WIDGET));
                            text!("x");
                            key!(KeyCode::Enter);
                        }
                        "custom-created" => {
                            key!(KeyCode::Backspace);
                            text!("ops");
                            key!(KeyCode::Enter);
                        }
                        _ => {}
                    }
                    let frame = frame!();
                    assert!(
                        frame.scene_built,
                        "current input state must produce a scene"
                    );
                    let allocated: Vec<_> = nodes
                        .iter()
                        .map(|node| tree.get_absolute_paint_rect(*node).unwrap())
                        .collect();
                    if let Some(before) = &allocation {
                        assert_eq!(
                            &allocated, before,
                            "input state preserves canonical allocations"
                        );
                    } else {
                        allocation = Some(allocated.clone());
                    }
                    for rect in &allocated {
                        assert_eq!(rect.height, theme.geometry.text_field.height);
                        assert_eq!(rect.width, W as f32 - 28.0);
                        assert!(rect.x >= 0.0 && rect.y >= 0.0);
                        assert!(rect.right() <= W as f32 && rect.bottom() <= H as f32);
                    }
                    for index in [3, 4] {
                        assert!(!tree.node(nodes[index]).unwrap().widget.capture_eligible());
                    }
                    assert!(!tree.node(nodes[4]).unwrap().widget.focusable());
                    let main = tree
                        .node(nodes[0])
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<TagInput>()
                        .unwrap();
                    let expected_ids = if column >= BASIC.len() {
                        vec![0, 1, 2]
                    } else {
                        vec![0, 1]
                    };
                    assert_eq!(main.committed_ids(), expected_ids);
                    assert_eq!(changes.get(), usize::from(column >= BASIC.len()));
                    let custom = tree
                        .node(nodes[5])
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<TagInput>()
                        .unwrap();
                    let expected_custom = if state == "custom-created" {
                        vec![TagEntry::Custom("ops".into())]
                    } else {
                        Vec::new()
                    };
                    assert_eq!(custom.committed_entries(), expected_custom.as_slice());
                    assert_eq!(custom_changes.get(), usize::from(state == "custom-created"));
                    let focused = domains.primary_state().focus_manager.focused();
                    let focus_name = if focused == Some(owner) {
                        "main"
                    } else if focused == Some(custom_owner) {
                        "custom"
                    } else {
                        "none"
                    };
                    assert_eq!(
                        focus_name,
                        if state == "rest" {
                            "none"
                        } else if matches!(state, "validation-error" | "custom-created") {
                            "custom"
                        } else {
                            "main"
                        }
                    );
                    let captured = !matches!(state, "rest" | "focus" | "settled");
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        if captured {
                            KeyboardAuthorityMode::Capture
                        } else {
                            KeyboardAuthorityMode::Navigation
                        }
                    );
                    let mut popup_texts = Vec::new();
                    let mut content_texts = Vec::new();
                    visit_scene_nodes(&frame.scene_root, &mut |node| {
                        if let SceneNode::Text(text) = node {
                            if text.z_index >= LAYER_OVERLAY {
                                popup_texts.push(text.text.clone());
                            } else {
                                content_texts.push(text.text.clone());
                            }
                        }
                    });
                    let popup_open =
                        matches!(state, "capture" | "filtered" | "toggled" | "no-match");
                    assert_eq!(!popup_texts.is_empty(), popup_open);
                    if state == "filtered" {
                        assert!(popup_texts.iter().any(|text| text == "telemetry"));
                    }
                    if state == "validation-error" {
                        assert!(content_texts.iter().any(|text| text == "x"));
                    }
                    if state == "custom-created" {
                        assert!(content_texts.iter().any(|text| text == "ops"));
                    }
                    let mut scene = control_order_scene(&frame);
                    scene.clear_color = theme.colors.surfaces.base0.into();
                    let plan = DrawPlanBuilder::new()
                        .build_multi(
                            &HashMap::from([(surface, scene)]),
                            compositor.snapshot_multi(),
                            &mut MeshCache::new(),
                            |_| scale,
                            0,
                        )
                        .expect("scene must satisfy DrawPlan Build admission");
                    let compiled = renderer
                        .compile_multi(&plan)
                        .unwrap()
                        .expect_ready("native Compile must be ready");
                    assert!(
                        compiled
                            .text_realization(surface)
                            .unwrap()
                            .outcome()
                            .is_complete()
                    );
                    let pixels = read_compiled_surface(&renderer, &compiled, surface)
                        .expect("TagInput actual GPU readback");
                    let filename = format!("{theme_name}-{density_name}-{scale}x-{state}.ppm");
                    write_ppm(&directory.join(&filename), width, height, &pixels);
                    writeln!(
                        manifest,
                        "{theme_name}\t{density_name}\t{scale}\t{state}\t{allocated:?}\t{expected_ids:?}\t{expected_custom:?}\t({}, {})\t{focus_name}\t{captured}\t{filename}",
                        changes.get(),
                        custom_changes.get()
                    )
                    .unwrap();
                    frames_written += 1;
                    let (target_contact, target_column, target_row, columns) =
                        if column < BASIC.len() {
                            (&mut contact, column, row, BASIC.len())
                        } else {
                            (&mut detail_contact, column - BASIC.len(), 0, EXTRA.len())
                        };
                    for y in 0..height as usize {
                        let destination =
                            ((target_row * height as usize + y) * width as usize * columns
                                + target_column * width as usize)
                                * 4;
                        target_contact[destination..destination + width as usize * 4]
                            .copy_from_slice(
                                &pixels[y * width as usize * 4..(y + 1) * width as usize * 4],
                            );
                    }
                }
                assert!(
                    action!(ActionId::RELEASE_CAPTURE)
                        || domains.primary_state().keyboard_mode
                            == KeyboardAuthorityMode::Navigation
                );
                let _settled = frame!();
            }
            assert!(
                pollster::block_on(validation_scope.pop()).is_none()
            );
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-contact.ppm")),
                width * BASIC.len() as u32,
                height * 4,
                &contact,
            );
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-detail-contact.ppm")),
                width * EXTRA.len() as u32,
                height,
                &detail_contact,
            );
        }
    }
    assert_eq!(frames_written, 126);
}

#[test]
fn text_area_default_gpu_evidence() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::{Alignment, EdgeInsets};
    use crate::core::text_engine::TextEngine;
    use crate::core::theme::DensityMode;
    use crate::runtime::actions::ActionId;
    use crate::widgets::text_area::TextArea;
    use std::io::Write;

    const W: u32 = 400;
    const H: u32 = 360;
    const AREA_HEIGHT: f32 = 72.0;
    const BASIC: [&str; 3] = ["rest", "focus", "capture"];
    const EXTRA: [&str; 4] = [
        "selection",
        "edited-scrolled",
        "read-only-selection",
        "settled",
    ];
    const DOCUMENT: &str = "Release notes\nInput material\nStable geometry\nRetained scene\nCurrent focus\nEditing capture\nViewport clipping\nSelection range\nScroll ownership\nBoth themes\nAll densities\nFinal line";
    const READ_ONLY: &str = "Read-only notes\nSelection remains available";
    const DISABLED: &str = "Disabled notes\nUnavailable for input";

    fn area(tree: &WidgetTree, node: crate::core::widget_tree::NodeId) -> &TextArea {
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextArea>()
            .unwrap()
    }

    let first_line_len = DOCUMENT.find('\n').unwrap();
    let read_only_first_line_len = READ_ONLY.find('\n').unwrap();
    let edited_document = format!("Refined notes.{}\nSaved", &DOCUMENT[first_line_len..]);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/text-area-default-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(
        manifest,
        "theme\tdensity\tscale\tstate\tallocations\tvalues\tcursor\tselection\ttarget_scroll\tvisual_scroll\tviewports\tcarets\tfocus\tcapture\tfile"
    )
    .unwrap();
    println!("TextArea actual GPU evidence: {}", directory.display());
    let write_ppm = |path: &std::path::Path, width: u32, height: u32, pixels: &[u8]| {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels.chunks_exact(4) {
            file.write_all(&pixel[..3]).unwrap();
        }
    };
    let mut frames_written = 0;
    for (dark, theme_name) in [(false, "light"), (true, "dark")] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, scale)
                .expect("TextArea evidence requires an actual GPU adapter and readback");
            renderer.text_engine_mut().set_scale(scale);
            let validation_scope = renderer
                .gpu_resources()
                .device
                .push_error_scope(wgpu::ErrorFilter::Validation);
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let width = (W as f32 * scale) as u32;
            let height = (H as f32 * scale) as u32;
            let mut contact = vec![0_u8; (width * height * 4 * 3 * 4) as usize];
            let mut detail_contact = vec![0_u8; (width * height * 4 * 4) as usize];
            for (row, (density, density_name)) in [
                (DensityMode::Compact, "compact24"),
                (DensityMode::Regular, "regular28"),
                (DensityMode::Intermediate30, "intermediate30"),
                (DensityMode::Comfortable, "comfortable32"),
            ]
            .into_iter()
            .enumerate()
            {
                let theme = if dark {
                    Theme::dark_with_density(density)
                } else {
                    Theme::light_with_density(density)
                };
                let controls = [
                    TextArea::new()
                        .with_fixed_height(AREA_HEIGHT)
                        .with_text(DOCUMENT),
                    TextArea::new()
                        .with_fixed_height(AREA_HEIGHT)
                        .with_placeholder("Write release notes"),
                    TextArea::new()
                        .with_fixed_height(AREA_HEIGHT)
                        .with_text(READ_ONLY)
                        .with_read_only(true),
                    TextArea::new()
                        .with_fixed_height(AREA_HEIGHT)
                        .with_text(DISABLED)
                        .with_enabled(false),
                ];
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(
                        Flex::column()
                            .with_padding(EdgeInsets::all(14.0))
                            .with_spacing(8.0)
                            .with_align_items(Alignment::Stretch),
                    ),
                    None,
                    0,
                );
                let nodes: Vec<_> = controls
                    .into_iter()
                    .enumerate()
                    .map(|(order, control)| {
                        tree.add_child(root, Box::new(control), None, order as i16)
                    })
                    .collect();
                let owner = tree.widget_id_for_node(nodes[0]).unwrap();
                let read_only_owner = tree.widget_id_for_node(nodes[2]).unwrap();
                let disabled_owner = tree.widget_id_for_node(nodes[3]).unwrap();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                let mut harness = RetainedSceneTestHarness::new();
                let icons = builtin_registry();
                let constraints = Constraints::tight(Size::new(W as f32, H as f32));
                let mut allocation = None;
                macro_rules! frame {
                    () => {
                        harness.frame_with_optional_custom_render_target(
                            &mut tree,
                            root,
                            constraints,
                            &theme,
                            renderer.text_engine_mut(),
                            &icons,
                            Some(target),
                            Some((&domains, WindowId::PRIMARY)),
                        )
                    };
                }
                macro_rules! route {
                    ($event:expr) => {
                        router.route_input_event(
                            &mut tree,
                            $event,
                            Some(root),
                            renderer.text_engine_mut(),
                            None,
                            domains.primary_state_mut(),
                            &mut bindings,
                        )
                    };
                }
                macro_rules! key {
                    ($code:expr, $modifiers:expr) => {
                        route!(FusedInputEvent::KeyDown {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            key_code: $code,
                            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                            reservation: None,
                            modifiers: $modifiers,
                            repeat: false,
                        })
                    };
                }
                macro_rules! text {
                    ($text:expr) => {
                        route!(FusedInputEvent::TextInput {
                            window_id: WindowId::PRIMARY,
                            time: Instant::now(),
                            text: $text.into(),
                            modifiers: Modifiers::default(),
                        })
                    };
                }
                macro_rules! action {
                    ($action:expr) => {
                        router.handle_engine_action(
                            &mut tree,
                            $action,
                            Some(root),
                            domains.primary_state_mut(),
                            renderer.text_engine_mut(),
                            None,
                        )
                    };
                }
                let states: Vec<_> = BASIC
                    .into_iter()
                    .chain(
                        (density == DensityMode::Intermediate30)
                            .then_some(EXTRA)
                            .into_iter()
                            .flatten(),
                    )
                    .collect();
                for (column, state) in states.into_iter().enumerate() {
                    let time_ms = column as u64 * 100;
                    router.set_frame_time_ms(time_ms);
                    harness.advance_transition_time(time_ms as f64 / 1000.0);
                    match state {
                        "focus" => router.request_focus_for_widget(
                            &mut tree,
                            owner,
                            domains.primary_state_mut(),
                        ),
                        "capture" => assert!(action!(ActionId::ENTER_WIDGET)),
                        "selection" => {
                            key!(
                                KeyCode::End,
                                Modifiers {
                                    shift: true,
                                    ..Modifiers::default()
                                }
                            );
                        }
                        "edited-scrolled" => {
                            text!("Refined notes.");
                            for _ in 0..20 {
                                key!(KeyCode::Down, Modifiers::default());
                            }
                            key!(KeyCode::End, Modifiers::default());
                            key!(KeyCode::Enter, Modifiers::default());
                            text!("Saved");
                        }
                        "read-only-selection" => {
                            assert!(action!(ActionId::RELEASE_CAPTURE));
                            router.request_focus_for_widget(
                                &mut tree,
                                read_only_owner,
                                domains.primary_state_mut(),
                            );
                            assert!(action!(ActionId::ENTER_WIDGET));
                            key!(
                                KeyCode::End,
                                Modifiers {
                                    shift: true,
                                    ..Modifiers::default()
                                }
                            );
                            let before = (
                                area(&tree, nodes[2]).text().to_string(),
                                area(&tree, nodes[2]).cursor_byte_pos_for_test(),
                                area(&tree, nodes[2]).selection_range_for_test(),
                            );
                            text!("blocked");
                            assert_eq!(
                                (
                                    area(&tree, nodes[2]).text().to_string(),
                                    area(&tree, nodes[2]).cursor_byte_pos_for_test(),
                                    area(&tree, nodes[2]).selection_range_for_test(),
                                ),
                                before,
                                "read-only input preserves document and selection"
                            );
                            router.request_focus_for_widget(
                                &mut tree,
                                disabled_owner,
                                domains.primary_state_mut(),
                            );
                            assert_eq!(
                                domains.primary_state().focus_manager.focused(),
                                Some(read_only_owner),
                                "disabled editor cannot receive focus"
                            );
                        }
                        "settled" => assert!(action!(ActionId::RELEASE_CAPTURE)),
                        _ => {}
                    }
                    let mut frame = frame!();
                    if state == "edited-scrolled" {
                        for _ in 0..64 {
                            let control = area(&tree, nodes[0]);
                            if control.scroll_y_for_test() == control.visual_scroll_y_for_test() {
                                break;
                            }
                            frame = frame!();
                        }
                    }
                    assert!(
                        frame.scene_built,
                        "current editor state must produce a scene"
                    );
                    let allocated: Vec<_> = nodes
                        .iter()
                        .map(|node| tree.get_absolute_paint_rect(*node).unwrap())
                        .collect();
                    if let Some(before) = &allocation {
                        assert_eq!(
                            &allocated, before,
                            "editing, selection and scrolling preserve canonical allocations"
                        );
                    } else {
                        allocation = Some(allocated.clone());
                    }
                    for rect in &allocated {
                        assert_eq!(rect.height, AREA_HEIGHT);
                        assert_eq!(rect.width, W as f32 - 28.0);
                        assert!(rect.x >= 0.0 && rect.y >= 0.0);
                        assert!(rect.right() <= W as f32 && rect.bottom() <= H as f32);
                    }
                    assert!(area(&tree, nodes[2]).is_read_only());
                    assert!(tree.node(nodes[2]).unwrap().widget.focusable());
                    assert!(tree.node(nodes[2]).unwrap().widget.capture_eligible());
                    assert!(!area(&tree, nodes[3]).is_enabled());
                    assert!(!tree.node(nodes[3]).unwrap().widget.focusable());
                    assert!(!tree.node(nodes[3]).unwrap().widget.capture_eligible());
                    let expected_document = if column >= BASIC.len() + 1 {
                        edited_document.as_str()
                    } else {
                        DOCUMENT
                    };
                    let values: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).text().to_string())
                        .collect();
                    assert_eq!(
                        values,
                        [
                            expected_document.to_string(),
                            String::new(),
                            READ_ONLY.to_string(),
                            DISABLED.to_string(),
                        ]
                    );
                    if state == "selection" {
                        assert_eq!(
                            area(&tree, nodes[0]).selection_range_for_test(),
                            (0, first_line_len)
                        );
                    }
                    if matches!(state, "read-only-selection" | "settled") {
                        assert_eq!(
                            area(&tree, nodes[2]).selection_range_for_test(),
                            (0, read_only_first_line_len)
                        );
                    }
                    let focused = domains.primary_state().focus_manager.focused();
                    let focus_name = if focused == Some(owner) {
                        "main"
                    } else if focused == Some(read_only_owner) {
                        "read-only"
                    } else {
                        "none"
                    };
                    assert_eq!(
                        focus_name,
                        if state == "rest" {
                            "none"
                        } else if matches!(state, "read-only-selection" | "settled") {
                            "read-only"
                        } else {
                            "main"
                        }
                    );
                    let captured = !matches!(state, "rest" | "focus" | "settled");
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        if captured {
                            KeyboardAuthorityMode::Capture
                        } else {
                            KeyboardAuthorityMode::Navigation
                        }
                    );
                    let cursor: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).cursor_byte_pos_for_test())
                        .collect();
                    let selection: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).selection_range_for_test())
                        .collect();
                    let target_scroll: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).scroll_y_for_test())
                        .collect();
                    let visual_scroll: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).visual_scroll_y_for_test())
                        .collect();
                    let viewports: Vec<_> = nodes
                        .iter()
                        .map(|&node| area(&tree, node).clip_rect_for_test())
                        .collect();
                    let carets: Vec<_> = nodes
                        .iter()
                        .map(|&node| {
                            area(&tree, node)
                                .caret_rect_for_test(area(&tree, node).visual_scroll_y_for_test())
                        })
                        .collect();
                    for index in 0..nodes.len() {
                        assert!(values[index].is_char_boundary(cursor[index]));
                        assert!(target_scroll[index].is_finite() && target_scroll[index] >= 0.0);
                        assert!(visual_scroll[index].is_finite() && visual_scroll[index] >= 0.0);
                        let viewport = viewports[index];
                        assert!(viewport.width > 0.0 && viewport.height > 0.0);
                        assert!(viewport.x >= 0.0 && viewport.y >= 0.0);
                        assert!(viewport.right() <= allocated[index].width);
                        assert!(viewport.bottom() <= allocated[index].height);
                        let absolute = RectF::new(
                            allocated[index].x + viewport.x,
                            allocated[index].y + viewport.y,
                            viewport.width,
                            viewport.height,
                        );
                        let mut matching_clips = 0;
                        let mut clipped_text = 0;
                        let mut selection_backgrounds = 0;
                        visit_scene_nodes(&frame.scene_root, &mut |node| {
                            if let SceneNode::Clip(clip) = node {
                                if clip.rect == absolute {
                                    matching_clips += 1;
                                    visit_scene_nodes(&clip.children, &mut |child| {
                                        if let SceneNode::Text(text) = child {
                                            clipped_text += 1;
                                            selection_backgrounds += text.backgrounds.len();
                                        }
                                    });
                                }
                            }
                        });
                        assert_eq!(matching_clips, 1, "each editor owns one text viewport clip");
                        assert!(
                            clipped_text > 0,
                            "editor text remains inside its viewport clip"
                        );
                        if (state == "selection" && index == 0)
                            || (matches!(state, "read-only-selection" | "settled") && index == 2)
                        {
                            assert!(
                                selection_backgrounds > 0,
                                "routed selection emits text-native backgrounds inside the clip"
                            );
                        }
                    }
                    if state == "edited-scrolled" {
                        assert!(target_scroll[0] > 0.0, "caret navigation reaches overflow");
                        assert_eq!(
                            target_scroll[0], visual_scroll[0],
                            "existing Update path settles visual scrolling"
                        );
                        let caret = carets[0].expect("captured editor has a current caret");
                        assert!(caret.y >= viewports[0].y - 0.001);
                        assert!(caret.bottom() <= viewports[0].bottom() + 0.001);
                        assert_eq!(cursor[0], edited_document.len());
                    }
                    let mut scene = control_order_scene(&frame);
                    scene.clear_color = theme.colors.surfaces.base0.into();
                    let plan = DrawPlanBuilder::new()
                        .build_multi(
                            &HashMap::from([(surface, scene)]),
                            compositor.snapshot_multi(),
                            &mut MeshCache::new(),
                            |_| scale,
                            0,
                        )
                        .expect("scene must satisfy DrawPlan Build admission");
                    let compiled = renderer
                        .compile_multi(&plan)
                        .unwrap()
                        .expect_ready("native Compile must be ready");
                    assert!(
                        compiled
                            .text_realization(surface)
                            .unwrap()
                            .outcome()
                            .is_complete()
                    );
                    let pixels = read_compiled_surface(&renderer, &compiled, surface)
                        .expect("TextArea actual GPU readback");
                    let filename = format!("{theme_name}-{density_name}-{scale}x-{state}.ppm");
                    write_ppm(&directory.join(&filename), width, height, &pixels);
                    writeln!(
                        manifest,
                        "{theme_name}\t{density_name}\t{scale}\t{state}\t{allocated:?}\t{values:?}\t{cursor:?}\t{selection:?}\t{target_scroll:?}\t{visual_scroll:?}\t{viewports:?}\t{carets:?}\t{focus_name}\t{captured}\t{filename}"
                    )
                    .unwrap();
                    frames_written += 1;
                    let (target_contact, target_column, target_row, columns) =
                        if column < BASIC.len() {
                            (&mut contact, column, row, BASIC.len())
                        } else {
                            (&mut detail_contact, column - BASIC.len(), 0, EXTRA.len())
                        };
                    for y in 0..height as usize {
                        let destination =
                            ((target_row * height as usize + y) * width as usize * columns
                                + target_column * width as usize)
                                * 4;
                        target_contact[destination..destination + width as usize * 4]
                            .copy_from_slice(
                                &pixels[y * width as usize * 4..(y + 1) * width as usize * 4],
                            );
                    }
                }
                assert!(
                    action!(ActionId::RELEASE_CAPTURE)
                        || domains.primary_state().keyboard_mode
                            == KeyboardAuthorityMode::Navigation
                );
                let _settled = frame!();
            }
            assert!(
                pollster::block_on(validation_scope.pop()).is_none()
            );
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-contact.ppm")),
                width * BASIC.len() as u32,
                height * 4,
                &contact,
            );
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-detail-contact.ppm")),
                width * EXTRA.len() as u32,
                height,
                &detail_contact,
            );
        }
    }
    assert_eq!(frames_written, 96);
}

#[cfg(target_os = "macos")]
#[test]
fn text_area_frame_gpu_evidence() {
    use crate::core::event::InteractionDomains;
    use crate::core::layout::{Alignment, EdgeInsets};
    use crate::core::text_engine::TextEngine;
    use crate::core::theme::DensityMode;
    use crate::runtime::actions::ActionId;
    use crate::widgets::TooltipHost;
    use crate::widgets::groupbox::GroupBox;
    use crate::widgets::text_area::TextArea;
    use std::io::Write;

    const W: u32 = 420;
    const H: u32 = 190;
    const DOCUMENT: &str = "[session]\nprofile = \"canary\"\nmax_inflight = 128\ntimeout_ms = 2500\ntrace_sample = 0.10\nrollback_on_error = true";
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/text-area-frame-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(
        manifest,
        "theme\tdensity\tscale\tstate\tallocation\tviewport\tparent_z\tchrome_z\ttext_z\tfocus\tcapture\tfile"
    )
    .unwrap();
    println!(
        "TextArea composed frame GPU evidence: {}",
        directory.display()
    );
    let mut order_failures = Vec::new();
    let mut frames_written = 0;
    for (dark, theme_name) in [(false, "light"), (true, "dark")] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut renderer, compositor, surface) = setup_with_size_and_scale(W, H, scale)
                .expect("TextArea frame evidence requires an actual GPU adapter");
            renderer.text_engine_mut().set_scale(scale);
            let validation_scope = renderer
                .gpu_resources()
                .device
                .push_error_scope(wgpu::ErrorFilter::Validation);
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let width = (W as f32 * scale) as u32;
            let height = (H as f32 * scale) as u32;
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
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(
                        Flex::column()
                            .with_padding(EdgeInsets::all(14.0))
                            .with_align_items(Alignment::Stretch),
                    ),
                    None,
                    0,
                );
                let group = tree.add_child(
                    root,
                    Box::new(GroupBox::empty("Connection Profile")),
                    None,
                    0,
                );
                let content =
                    tree.add_child(group, Box::new(Flex::column().with_spacing(5.0)), None, 0);
                let mut section = GroupBox::new(content, "Connection Profile")
                    .with_padding(6.0)
                    .with_label_size(13.0);
                if dark {
                    section = section.with_background(theme.colors.surfaces.level1);
                }
                crate::replace_widget!(tree, group, section);
                let tip = tree.add_child(
                    content,
                    Box::new(TooltipHost::new("Session profile (TOML)")),
                    None,
                    0,
                );
                let node = tree.add_child(
                    tip,
                    Box::new(TextArea::new().with_text(DOCUMENT).with_min_height(88.0)),
                    None,
                    0,
                );
                let owner = tree.widget_id_for_node(node).unwrap();
                let group_owner = tree.widget_id_for_node(group).unwrap();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let mut harness = RetainedSceneTestHarness::new();
                let icons = builtin_registry();
                let constraints = Constraints::tight(Size::new(W as f32, H as f32));
                let mut original_allocation = None;
                for (index, state) in ["rest", "focus", "capture"].into_iter().enumerate() {
                    let time_ms = index as u64 * 100;
                    router.set_frame_time_ms(time_ms);
                    harness.advance_transition_time(time_ms as f64 / 1000.0);
                    if state == "focus" {
                        router.request_focus_for_widget(
                            &mut tree,
                            owner,
                            domains.primary_state_mut(),
                        );
                    } else if state == "capture" {
                        assert!(router.handle_engine_action(
                            &mut tree,
                            ActionId::ENTER_WIDGET,
                            Some(root),
                            domains.primary_state_mut(),
                            renderer.text_engine_mut(),
                            None,
                        ));
                    }
                    let frame = harness.frame_with_optional_custom_render_target(
                        &mut tree,
                        root,
                        constraints,
                        &theme,
                        renderer.text_engine_mut(),
                        &icons,
                        Some(target),
                        Some((&domains, WindowId::PRIMARY)),
                    );
                    assert!(frame.scene_built);
                    let allocation = tree.get_absolute_paint_rect(node).unwrap();
                    if let Some(original) = original_allocation {
                        assert_eq!(
                            allocation, original,
                            "state changes preserve allocated geometry"
                        );
                    } else {
                        original_allocation = Some(allocation);
                    }
                    assert!(allocation.height >= 88.0);
                    assert!(allocation.x >= 0.0 && allocation.y >= 0.0);
                    assert!(allocation.right() <= W as f32);
                    assert!(allocation.bottom() <= H as f32);
                    let area = tree
                        .node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<TextArea>()
                        .unwrap();
                    assert_eq!(area.text(), DOCUMENT);
                    let viewport = area.clip_rect_for_test();
                    assert!(viewport.width > 0.0 && viewport.height > 0.0);
                    assert!(viewport.x >= 0.0 && viewport.y >= 0.0);
                    assert!(viewport.right() <= allocation.width);
                    assert!(viewport.bottom() <= allocation.height);
                    let absolute_viewport = RectF::new(
                        allocation.x + viewport.x,
                        allocation.y + viewport.y,
                        viewport.width,
                        viewport.height,
                    );
                    let local = harness.local_fragment(owner).unwrap();
                    let parent = harness.local_fragment(group_owner).unwrap();
                    let parent_z = parent
                        .iter()
                        .find_map(|node| {
                            if let SceneNode::RoundedRect(rect) = node {
                                Some(rect.z_index)
                            } else {
                                None
                            }
                        })
                        .expect("opaque GroupBox host owns its surface");
                    let chrome: Vec<_> = local
                        .iter()
                        .filter_map(|node| {
                            if let SceneNode::RoundedRect(rect) = node {
                                Some(rect)
                            } else {
                                None
                            }
                        })
                        .take(3)
                        .collect();
                    assert_eq!(chrome.len(), 3, "TextArea retains its three-band material");
                    assert_eq!(chrome[0].rect, allocation);
                    assert!(chrome[1].rect.x >= allocation.x);
                    assert!(chrome[2].rect.x >= chrome[1].rect.x);
                    assert!(chrome[2].rect.right() <= chrome[1].rect.right());
                    let chrome_z: Vec<_> = chrome.iter().map(|rect| rect.z_index).collect();
                    let mut text_z = Vec::new();
                    let mut matching_clips = 0;
                    visit_scene_nodes(&local, &mut |node| {
                        if let SceneNode::Clip(clip) = node {
                            if clip.rect == absolute_viewport {
                                matching_clips += 1;
                                visit_scene_nodes(&clip.children, &mut |child| {
                                    if let SceneNode::Text(text) = child {
                                        text_z.push(text.z_index);
                                    }
                                });
                            }
                        }
                    });
                    assert_eq!(matching_clips, 1);
                    assert!(
                        !text_z.is_empty(),
                        "actual editor glyphs remain in the viewport"
                    );
                    let focused = domains.primary_state().focus_manager.focused() == Some(owner);
                    let captured =
                        domains.primary_state().keyboard_mode == KeyboardAuthorityMode::Capture;
                    assert_eq!(focused, state != "rest");
                    assert_eq!(captured, state == "capture");
                    if !(parent_z < chrome_z[0]
                        && chrome_z[0] < chrome_z[1]
                        && chrome_z[1] < chrome_z[2]
                        && text_z.iter().all(|&z| chrome_z[2] < z))
                    {
                        order_failures.push(format!(
                            "{theme_name}/{density_name}/{scale}/{state}: parent={parent_z}, chrome={chrome_z:?}, text={text_z:?}"
                        ));
                    }
                    let mut scene = control_order_scene(&frame);
                    scene.clear_color = theme.colors.surfaces.base0.into();
                    let plan = DrawPlanBuilder::new()
                        .build_multi(
                            &HashMap::from([(surface, scene)]),
                            compositor.snapshot_multi(),
                            &mut MeshCache::new(),
                            |_| scale,
                            0,
                        )
                        .expect("scene must satisfy DrawPlan Build admission");
                    let compiled = renderer
                        .compile_multi(&plan)
                        .unwrap()
                        .expect_ready("native Compile must be ready");
                    assert!(
                        compiled
                            .text_realization(surface)
                            .unwrap()
                            .outcome()
                            .is_complete()
                    );
                    let pixels = read_compiled_surface(&renderer, &compiled, surface)
                        .expect("TextArea composed frame actual GPU readback");
                    let filename = format!("{theme_name}-{density_name}-{scale}x-{state}.ppm");
                    let mut file = std::fs::File::create(directory.join(&filename)).unwrap();
                    write!(file, "P6\n{width} {height}\n255\n").unwrap();
                    for pixel in pixels.chunks_exact(4) {
                        file.write_all(&pixel[..3]).unwrap();
                    }
                    writeln!(
                        manifest,
                        "{theme_name}\t{density_name}\t{scale}\t{state}\t{allocation:?}\t{viewport:?}\t{parent_z}\t{chrome_z:?}\t{text_z:?}\t{focused}\t{captured}\t{filename}"
                    ).unwrap();
                    frames_written += 1;
                }
            }
            assert!(
                pollster::block_on(validation_scope.pop()).is_none()
            );
        }
    }
    assert_eq!(frames_written, 72);
    assert!(
        order_failures.is_empty(),
        "TextArea frame must survive its opaque section host while preserving strict content order: {order_failures:?}"
    );
}
