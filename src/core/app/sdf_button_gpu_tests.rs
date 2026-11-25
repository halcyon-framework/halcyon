//! Actual canonical Button/producer proof; headless encoding cannot prove present.

#[path = "sdf_button_v2_gpu_tests.rs"]
mod v2;

#[path = "sdf_button_v3_gpu_tests.rs"]
mod v3;

use super::*;
use crate::core::layout::{Alignment, EdgeInsets};
use crate::core::paint::Color;
use crate::core::scene::ImageTextureSource;
use crate::renderer::DrawPlanBuilder;
use crate::renderer::batcher::PipelineId;
use crate::renderer::image_binding::ImageBindingFailure;
use crate::renderer::mesh_cache::MeshCache;
use crate::widgets::{
    button::{Button, ButtonSdfRestingProfile},
    container::Container,
    flex::Flex,
    label::Label,
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn leaves(nodes: &[SceneNode]) -> Vec<SceneNode> {
    nodes
        .iter()
        .flat_map(|node| match node {
            SceneNode::RetainedFragment(n) => leaves(&n.children),
            SceneNode::Clip(n) => leaves(&n.children),
            SceneNode::Layer(n) => leaves(&n.children),
            SceneNode::Scroll(n) => leaves(&n.children),
            _ => vec![node.clone()],
        })
        .collect()
}

fn poll_until(device: &wgpu::Device, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "bounded GPU completion observation timed out"
        );
        device
            .poll(wgpu::PollType::Poll)
            .expect("nonblocking GPU poll");
        std::thread::yield_now();
    }
}

fn read_pixels(
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<[u8; 4]> {
    let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("canonical Button GPU readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([copy.finish()]);
    let mapped = Arc::new(Mutex::new(None));
    let callback = mapped.clone();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            *callback.lock().unwrap() = Some(result);
        });
    poll_until(&gpu.device, || mapped.lock().unwrap().is_some());
    mapped
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .expect("readback mapping");
    let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let pixels = (0..height as usize)
        .flat_map(|y| {
            bytes[y * padded as usize..y * padded as usize + width as usize * 4]
                .chunks_exact(4)
                .map(|p| [p[2], p[1], p[0], p[3]])
                .collect::<Vec<_>>()
        })
        .collect();
    drop(bytes);
    buffer.unmap();
    pixels
}

#[test]
fn canonical_button_sdf_gpu_frames_preserve_generation_custody() {
    use crate::renderer::sdf_frame_custody::SdfFramePlan;
    use std::io::Write;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/4e-button-p14")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    println!("legacy Button actual GPU evidence: {}", directory.display());

    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
            .expect("real GPU required; no Button pixel proof without one");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.format, wgpu::TextureFormat::Bgra8UnormSrgb);
        let size = Size::new(480.0, 200.0);
        let bounds = RectF::new(0.0, 0.0, size.width, size.height);
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        let (width, height) = ((size.width * scale) as u32, (size.height * scale) as u32);
        renderer
            .register_headless_render_target(surface, (width, height), scale)
            .unwrap();
        let background = Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 });
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(20.0))
                    .with_spacing(16.0)
                    .with_align_items(Alignment::Start)
                    .with_background(background),
            ),
            None,
            0,
        );
        tree.set_overflow(root, crate::core::widget_tree::OverflowMode::Clip);
        let holder = tree.add_child(
            root,
            Box::new(Container::empty().with_width(120.0).with_height(30.0)),
            None,
            0,
        );
        let mut button = Button::with_text("Stage");
        button
            .request_sdf_material(Some(if dark {
                ButtonSdfRestingProfile::Dark
            } else {
                ButtonSdfRestingProfile::Light
            }))
            .unwrap();
        let node = tree.add_child(holder, Box::new(button), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Unchanged peer")), None, 0);
        let owner = tree.node(node).unwrap().id;
        let root_owner = tree.node(root).unwrap().id;
        let peer_owner = tree.node(peer).unwrap().id;
        let domains = InteractionDomains::new();
        let focus = domains.primary_state().focus_manager.focused();
        let icons = crate::core::icons::builtin_registry();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |tree: &mut WidgetTree,
                     renderer: &mut WgpuRenderer,
                     harness: &mut RetainedSceneTestHarness| {
            test_begin_frame_layout_tracking();
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let mut triggers = Vec::new();
            let outcome = build_canonical_scene_reusing(
                tree,
                root,
                Constraints::tight(size),
                &theme,
                renderer,
                &icons,
                harness.frame_time_ms,
                0.0,
                &mut harness.transition_layer,
                &mut harness.active_layout_transitions,
                None,
                scale,
                Some(target),
                &mut harness.custom_render_presentation_bindings,
                None,
                None,
                false,
                &mut triggers,
                TooltipSceneInput::Explicit(None),
                focus,
                KeyboardAuthorityMode::Navigation,
                Some((&domains, WindowId::PRIMARY)),
                false,
                &mut harness.recycled_scene,
                &mut harness.cache,
                None,
                Some(prepare_canonical_sdf_materials),
                None,
                false,
            );
            harness.frame_time_ms += 16;
            outcome
        };
        let local = |harness: &RetainedSceneTestHarness, id| harness.local_fragment(id).unwrap();
        let geometry = |tree: &WidgetTree| {
            [root, holder, node, peer].map(|id| {
                let n = tree.node(id).unwrap();
                (
                    n.id,
                    n.widget.id(),
                    tree.get_absolute_paint_rect(id),
                    tree.get_viewport(id),
                    n.last_constraints,
                )
            })
        };

        // Frame A is the actual pre-Paint shader preparation; no binding is injected.
        let first = frame(&mut tree, &mut renderer, &mut harness);
        assert!(first.scene.is_some());
        assert!(harness.cache.selected_sdf_materials.is_none());
        assert!(harness.cache.sdf_scene_custody.references().is_empty());
        let first_local = local(&harness, owner);
        assert!(
            !leaves(&first_local)
                .iter()
                .any(|n| matches!(n, SceneNode::Image(_)))
        );
        let original_geometry = geometry(&tree);
        let face = tree.get_absolute_paint_rect(node).unwrap();
        assert_eq!(face, RectF::new(20.0, 20.0, 120.0, 30.0));
        let first_text: Vec<_> = leaves(&first_local)
            .into_iter()
            .filter_map(|n| {
                if let SceneNode::Text(t) = n {
                    Some((t.text, t.position, t.font_size, t.color, t.z_index))
                } else {
                    None
                }
            })
            .collect();
        assert!(first_text.iter().any(|t| t.0 == "Stage" && t.2 == 13.0));
        let first_focus: Vec<_> = leaves(&first_local).into_iter().filter(|n| {
            matches!(n, SceneNode::BorderRect(b) if b.z_index == crate::core::paint::LAYER_FOCUS)
        }).collect();
        let root_local = local(&harness, root_owner);
        let peer_local = local(&harness, peer_owner);
        #[cfg(debug_assertions)]
        let epochs = [root, holder, node, peer].map(|id| tree.node(id).unwrap().layout_epoch);

        // Observe actual producer submission completion, then the real registrar cutoff.
        let prepared = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback = prepared.clone();
        gpu.queue.on_submitted_work_done(move || {
            callback.store(true, std::sync::atomic::Ordering::Release)
        });
        poll_until(&gpu.device, || {
            prepared.load(std::sync::atomic::Ordering::Acquire)
        });
        assert!(
            harness
                .cache
                .admit_sdf_materials_after_paint(&mut renderer, &tree)
        );
        renderer.drain_pending_image_registrations();
        assert!(
            harness.cache.selected_sdf_materials.is_none(),
            "post-Paint admission is not Ready publication"
        );

        let second = frame(&mut tree, &mut renderer, &mut harness);
        let key = harness.cache.qualified_sdf_requests.as_ref().unwrap()[0]
            .key
            .clone();
        assert!(tree.is_live_mount(key.tree_lifetime(), key.owner()));
        assert_eq!(key.metrics().final_logical_rect, face);
        assert_eq!(key.metrics().scale_factor, scale);
        assert_eq!(key.metrics().surface_id, surface);
        let input = &harness.cache.sdf_paint_inputs.as_ref().unwrap()[&owner];
        assert!(input.key == key);
        let admitted = input.image.as_ref().expect("Frame B real P2 Ready").clone();
        let identity = admitted.binding.identity();
        assert_eq!(admitted.binding.availability(), Ok(()));
        assert!(
            harness
                .cache
                .sdf_scene_custody
                .has_reference(&key, identity)
        );
        assert_eq!(
            harness.cache.sdf_scene_custody.references(),
            vec![(key.clone(), identity)]
        );
        let [left, right, top, bottom] = if scale == 1.0 {
            [4.4, 4.4, 3.3, 5.5]
        } else {
            [4.15, 4.15, 3.05, 5.25]
        };
        assert_eq!(
            admitted.coverage,
            RectF::new(
                face.x - left,
                face.y - top,
                face.width + left + right,
                face.height + top + bottom
            )
        );
        let extent = (
            (admitted.coverage.width * scale).ceil(),
            (admitted.coverage.height * scale).ceil(),
        );
        assert!((admitted.uv.width - admitted.coverage.width * scale / extent.0).abs() < 1e-6);
        assert!((admitted.uv.height - admitted.coverage.height * scale / extent.1).abs() < 1e-6);
        let current_local = local(&harness, owner);
        let emitted = leaves(&current_local);
        let images: Vec<_> = emitted
            .iter()
            .filter_map(|n| {
                if let SceneNode::Image(i) = n {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(images.len(), 1);
        assert_eq!(
            images[0].texture_source,
            ImageTextureSource::Admitted(identity)
        );
        assert_eq!(images[0].rect, admitted.coverage);
        assert_eq!(images[0].uv_rect, admitted.uv);
        let current_text: Vec<_> = emitted
            .iter()
            .filter_map(|n| {
                if let SceneNode::Text(t) = n {
                    Some((t.text.clone(), t.position, t.font_size, t.color, t.z_index))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(current_text, first_text);
        assert!(current_text.iter().all(|t| t.4 > images[0].z_index));
        assert_eq!(
            first_focus.len(),
            emitted
                .iter()
                .filter(|n| matches!(n,
            SceneNode::BorderRect(b) if b.z_index == crate::core::paint::LAYER_FOCUS))
                .count()
        );
        assert!(
            !emitted
                .iter()
                .any(|n| matches!(n, SceneNode::GradientRect(_))),
            "no default face over SDF"
        );
        assert_eq!(geometry(&tree), original_geometry);
        assert!(Arc::ptr_eq(&root_local, &local(&harness, root_owner)));
        assert!(Arc::ptr_eq(&peer_local, &local(&harness, peer_owner)));

        let scene = second
            .scene
            .expect("Frame B replaces owner with admitted Image");
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, scene)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let frame_plan = SdfFramePlan::new(plan, harness.cache.sdf_scene_custody.clone());
        let cancelled = renderer
            .compile_sdf_frame(frame_plan.clone(), None)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let sampled = renderer
            .compile_sdf_frame(frame_plan, None)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let batches = &sampled
            .plan()
            .surface_compiled_data(surface)
            .unwrap()
            .frame_batches
            .batches;
        let batch = batches
            .iter()
            .find(|b| b.key.pipeline == PipelineId::Image)
            .unwrap();
        assert_eq!(batch.key.texture_id, Some(identity.texture_id()));
        assert_eq!(batch.key.image_generation, Some(identity.generation()));
        assert_eq!(
            key.metrics().final_logical_viewport,
            tree.get_viewport(node).unwrap()
        );
        assert_eq!(
            batch.clip_state.rect, bounds,
            "derived E inherits the root clip rather than clipping to F"
        );
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("actual canonical Button scene output"),
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
        let completed = sampled
            .submit_headless_for_test(&renderer, surface, &output.create_view(&Default::default()))
            .unwrap();
        let pending_at_observation = !completed();
        poll_until(&gpu.device, &completed);
        let before = read_pixels(&gpu, &output, width, height);
        let pixel = |pixels: &[[u8; 4]], x: f32, y: f32| {
            pixels[(y * scale) as usize * width as usize + (x * scale) as usize]
        };
        let background_pixel = pixel(&before, 5.0, 5.0);
        let face_pixel = pixel(&before, face.x + 12.0, face.y + 15.0);
        let shadow_pixel = pixel(&before, face.x + 60.0, face.y + face.height + 1.5);
        assert_eq!(face_pixel[3], 255);
        assert!(
            if dark {
                (35..100).contains(&face_pixel[0])
            } else {
                (190..250).contains(&face_pixel[0])
            },
            "actual face: {face_pixel:?}"
        );
        assert_eq!(shadow_pixel[3], 255);
        assert!(
            shadow_pixel[0] < background_pixel[0],
            "soft contact coverage: {shadow_pixel:?} vs {background_pixel:?}"
        );
        assert!(
            background_pixel[0] - shadow_pixel[0] < 40,
            "contact shadow stays soft"
        );

        tree.node_mut(node)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        let withdrawn = frame(&mut tree, &mut renderer, &mut harness);
        assert!(harness.cache.sdf_scene_custody.references().is_empty());
        assert!(
            harness
                .cache
                .sdf_scene_images
                .as_ref()
                .is_none_or(|ids| !ids.contains(&identity))
        );
        assert!(
            harness
                .cache
                .image_dependencies
                .as_ref()
                .is_none_or(|deps| !deps.by_identity.contains_key(&identity))
        );
        assert!(
            !leaves(&local(&harness, owner))
                .iter()
                .any(|n| matches!(n, SceneNode::Image(_)))
        );
        prepare_canonical_sdf_materials(&mut renderer, &tree, &mut harness.cache);
        assert_eq!(
            admitted.binding.availability(),
            Ok(()),
            "held compiled cancellation blocks retirement after old E removal"
        );
        drop(cancelled);
        let after_plan = DrawPlanBuilder::new()
            .build_multi(
                &HashMap::from([(surface, withdrawn.scene.expect("withdrawal removes old E"))]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                1,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let after_compiled = renderer
            .compile_multi(&after_plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let command = renderer
            .encode_compiled_target_for_test(
                &after_compiled,
                surface,
                &output.create_view(&Default::default()),
            )
            .unwrap();
        renderer.submit_compiled_target_for_test(command);
        let after = read_pixels(&gpu, &output, width, height);
        let mut cleared = 0;
        let peer_rect = tree.get_absolute_paint_rect(peer).unwrap();
        for y in 0..height {
            for x in 0..width {
                let p = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                let contains = |r: RectF| {
                    p.0 >= r.x && p.0 < r.x + r.width && p.1 >= r.y && p.1 < r.y + r.height
                };
                let i = (y * width + x) as usize;
                if contains(admitted.coverage) && !contains(face) {
                    assert_eq!(
                        after[i], background_pixel,
                        "withdrawn old E clears at {p:?}"
                    );
                    cleared += usize::from(before[i] != after[i]);
                }
                if contains(peer_rect) {
                    assert_eq!(before[i], after[i], "peer pixels unchanged");
                }
            }
        }
        assert!(cleared > 0);
        let deadline = Instant::now() + Duration::from_secs(15);
        while admitted.binding.availability().is_ok() {
            assert!(
                Instant::now() < deadline,
                "certified producer retirement timed out"
            );
            prepare_canonical_sdf_materials(&mut renderer, &tree, &mut harness.cache);
            gpu.device.poll(wgpu::PollType::Poll).unwrap();
        }
        assert_eq!(
            admitted.binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(geometry(&tree), original_geometry);
        #[cfg(debug_assertions)]
        assert_eq!(
            [root, holder, node, peer].map(|id| tree.node(id).unwrap().layout_epoch),
            epochs
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), focus);
        assert!(Arc::ptr_eq(&root_local, &local(&harness, root_owner)));
        assert!(Arc::ptr_eq(&peer_local, &local(&harness, peer_owner)));
        for (name, pixels) in [("ready", &before), ("withdrawn", &after)] {
            let path = directory.join(format!(
                "button-{}-{}x-{name}.ppm",
                if dark { "dark" } else { "light" },
                scale as u32
            ));
            let mut file = std::fs::File::create(&path).unwrap();
            write!(file, "P6\n{width} {height}\n255\n").unwrap();
            for p in pixels {
                file.write_all(&p[..3]).unwrap();
            }
        }
        println!(
            "canonical Button {} {}x: real Frame A/B, {:?}, face {:?}, shadow {:?}, cleared {} E pixels; nonempty Compile/cancel + same-queue submit completed (initially pending={}); native present UNPROVEN",
            if dark { "dark" } else { "light" },
            scale,
            identity,
            face_pixel,
            shadow_pixel,
            cleared,
            pending_at_observation
        );
    }
}

#[test]
fn canonical_button_default_gpu_evidence() {
    use crate::core::appearance::ButtonPaintCase;
    use crate::core::event::{FusedInputEvent, PointerButton, PointerButtonsState, PointerDevice};
    use crate::core::theme::DensityMode;
    use crate::runtime::binding_registry::BindingRegistry;
    use std::io::Write;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/button-default-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(manifest, "theme\tdensity\tscale\tstate\tface\tfile").unwrap();
    println!(
        "ordinary Button actual GPU evidence: {}",
        directory.display()
    );
    let write_ppm = |path: &std::path::Path, width: u32, height: u32, pixels: &[[u8; 4]]| {
        assert_eq!(pixels.len(), (width * height) as usize);
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels {
            file.write_all(&pixel[..3]).unwrap();
        }
    };
    let states = [
        ("rest", ButtonPaintCase::Resting),
        ("hover", ButtonPaintCase::Hovered),
        ("pressed", ButtonPaintCase::Pressed),
        ("focus", ButtonPaintCase::Resting),
        ("disabled", ButtonPaintCase::Disabled),
    ];
    for dark in [false, true] {
        for scale in [1.0_f32, 1.5, 2.0] {
            let theme_name = if dark { "dark" } else { "light" };
            let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
                .expect("actual Button GPU required");
            let gpu = renderer.gpu_resources();
            assert_eq!(gpu.format, wgpu::TextureFormat::Bgra8UnormSrgb);
            let bounds = RectF::new(0.0, 0.0, 160.0, 80.0);
            let mut compositor = Compositor::new(bounds);
            let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
            let (width, height) = ((160.0 * scale) as u32, (80.0 * scale) as u32);
            renderer
                .register_headless_render_target(surface, (width, height), scale)
                .unwrap();
            let target = renderer.custom_render_target_snapshot(surface).unwrap();
            let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ordinary default Button evidence"),
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
            let mut contact = vec![[0_u8; 4]; (width * height * states.len() as u32 * 4) as usize];
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
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(
                        Flex::column()
                            .with_padding(EdgeInsets::all(20.0))
                            .with_align_items(Alignment::Start)
                            .with_background(Color::from_hex(if dark {
                                0x242424
                            } else {
                                0xF2F2F2
                            })),
                    ),
                    None,
                    0,
                );
                let clicks = Rc::new(Cell::new(0));
                let observed = clicks.clone();
                let node = tree.add_child(
                    root,
                    Box::new(
                        Button::with_text("Apply")
                            .on_click(move || observed.set(observed.get() + 1)),
                    ),
                    None,
                    0,
                );
                let owner = tree.widget_id_for_node(node).unwrap();
                let mut domains = InteractionDomains::new();
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                let mut fusion = InputFusion::new();
                let mut harness = RetainedSceneTestHarness::new();
                let mut geometry = None;
                let mut caption = None;
                for (column, &(state, expected)) in states.iter().enumerate() {
                    let rect = tree
                        .get_absolute_paint_rect(node)
                        .unwrap_or(RectF::new(20.0, 20.0, 80.0, 30.0));
                    let pos = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
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
                    match state {
                        "hover" => {
                            route!(FusedInputEvent::PointerMove {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos,
                                delta: (0.0, 0.0),
                                device: PointerDevice::Mouse,
                                buttons: PointerButtonsState::default()
                            });
                        }
                        "pressed" => {
                            route!(FusedInputEvent::PointerDown {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary,
                                click_count: 1
                            });
                        }
                        "focus" => {
                            route!(FusedInputEvent::PointerUp {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos,
                                device: PointerDevice::Mouse,
                                button: PointerButton::Primary
                            });
                            route!(FusedInputEvent::PointerMove {
                                window_id: WindowId::PRIMARY,
                                time: Instant::now(),
                                pos: Point::new(2.0, 2.0),
                                delta: (0.0, 0.0),
                                device: PointerDevice::Mouse,
                                buttons: PointerButtonsState::default()
                            });
                            router.request_focus_for_widget(
                                &mut tree,
                                owner,
                                domains.primary_state_mut(),
                            );
                            assert_eq!(clicks.get(), 1, "routed release activates once");
                        }
                        "disabled" => {
                            tree.request_widget_enabled(node, false).unwrap();
                            let settled =
                                crate::core::widget_availability::apply_pending_widget_availability(
                                    &mut tree,
                                    &mut router,
                                    &mut fusion,
                                    domains.primary_state_mut(),
                                    &mut bindings,
                                    renderer.text_engine_mut(),
                                );
                            assert!(settled.rejected.is_empty());
                            assert_eq!(settled.changed, vec![owner]);
                        }
                        _ => {}
                    }
                    harness.advance_transition_time((column + 1) as f64);
                    let frame = harness.frame_with_scene_inputs(
                        &mut tree,
                        root,
                        Constraints::tight(bounds.size()),
                        &theme,
                        renderer.text_engine_mut(),
                        &crate::core::icons::builtin_registry(),
                        Some(target),
                        Some((&domains, WindowId::PRIMARY)),
                        TooltipSceneInput::Explicit(None),
                        false,
                    );
                    assert!(
                        frame.scene_built,
                        "state transition must produce a current scene"
                    );
                    let face = tree.get_absolute_paint_rect(node).unwrap();
                    assert!(face.width > 0.0 && face.height > 0.0);
                    if let Some(before) = geometry {
                        assert_eq!(face, before, "paint state preserves allocation");
                    } else {
                        geometry = Some(face);
                    }
                    let button = tree
                        .node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<Button>()
                        .unwrap();
                    assert!(button.sdf_request_snapshot().is_none());
                    let resolved = button.resolved_appearance().unwrap();
                    assert_eq!(resolved.paint_case(), expected);
                    let facts = resolved.request().input.button();
                    assert_eq!(facts.enabled, state != "disabled");
                    assert_eq!(facts.pressed, state == "pressed");
                    assert!(resolved.request().input.selection().is_none());
                    if state == "focus" {
                        assert!(resolved.request().input.framework().focus_cue_visible());
                    }
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Navigation
                    );
                    assert_eq!(
                        domains
                            .primary_state()
                            .pointer_capture_manager
                            .is_any_captured(),
                        state == "pressed"
                    );
                    assert!(harness.cache.sdf_scene_custody.references().is_empty());
                    let local = leaves(&harness.local_fragment(owner).unwrap());
                    assert!(!local.iter().any(|node| matches!(node, SceneNode::Image(_))));
                    let texts = local
                        .iter()
                        .filter_map(|node| match node {
                            SceneNode::Text(text) => {
                                Some((text.text.clone(), text.position, text.font_size))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert!(texts.iter().any(|text| text.0 == "Apply"));
                    if let Some(before) = &caption {
                        assert_eq!(&texts, before, "state preserves caption geometry");
                    } else {
                        caption = Some(texts);
                    }
                    let mut scene = SceneGraph::new();
                    scene.root = frame.scene_root;
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
                        .expect_ready("ordinary Button compiled target");
                    assert!(
                        compiled
                            .text_realization(surface)
                            .unwrap()
                            .outcome()
                            .is_complete()
                    );
                    let command = renderer
                        .encode_compiled_target_for_test(
                            &compiled,
                            surface,
                            &output.create_view(&Default::default()),
                        )
                        .expect("actual ordinary Button output");
                    renderer.submit_compiled_target_for_test(command);
                    let pixels = read_pixels(&gpu, &output, width, height);
                    let filename = format!("{theme_name}-{density_name}-{scale}x-{state}.ppm");
                    write_ppm(&directory.join(&filename), width, height, &pixels);
                    writeln!(
                        manifest,
                        "{theme_name}\t{density_name}\t{scale}\t{state}\t{face:?}\t{filename}"
                    )
                    .unwrap();
                    for y in 0..height as usize {
                        let destination =
                            (row * height as usize + y) * width as usize * states.len()
                                + column * width as usize;
                        contact[destination..destination + width as usize]
                            .copy_from_slice(&pixels[y * width as usize..(y + 1) * width as usize]);
                    }
                }
            }
            write_ppm(
                &directory.join(format!("{theme_name}-{scale}x-contact.ppm")),
                width * states.len() as u32,
                height * 4,
                &contact,
            );
        }
    }
}
