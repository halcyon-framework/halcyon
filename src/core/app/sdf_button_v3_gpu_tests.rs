use super::*;
use crate::core::event::*;
use crate::renderer::sdf_frame_custody::SdfFramePlan;
use crate::runtime::binding_registry::BindingRegistry;

#[path = "sdf_button_v4_gpu_tests.rs"]
mod champagne_v4;

struct ReferenceETrial {
    renderer: WgpuRenderer,
    compositor: Compositor,
    surface: SurfaceId,
    tree: WidgetTree,
    root: NodeId,
    button: NodeId,
    owner: TreeWidgetId,
    domains: InteractionDomains,
    router: EventRouter,
    fusion: InputFusion,
    bindings: BindingRegistry,
    harness: RetainedSceneTestHarness,
    scenes: HashMap<SurfaceId, SceneGraph>,
    theme: Theme,
    scale: f32,
    require_image: bool,
    last_material_pending: bool,
}

impl ReferenceETrial {
    fn new(
        dark: bool,
        scale: f32,
        face_width: f32,
        child_caption: bool,
    ) -> (Self, Rc<Cell<usize>>, TreeWidgetId) {
        let bounds = RectF::new(0.0, 0.0, 240.0, 100.0);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(20.0))
                    .with_spacing(16.0)
                    .with_align_items(Alignment::Start)
                    .with_background(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 })),
            ),
            None,
            0,
        );
        tree.set_overflow(root, OverflowMode::Clip);
        let holder = tree.add_child(
            root,
            Box::new(Container::empty().with_width(face_width).with_height(30.0)),
            None,
            0,
        );
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let button = if child_caption {
            let node = tree.add_child(holder, Box::new(Button::new(NodeId(0))), None, 0);
            let label = tree.add_child(node, Box::new(Label::new("Stage")), None, 0);
            crate::replace_widget!(
                tree,
                node,
                Button::new(label).on_click(move || count.set(count.get() + 1))
            );
            node
        } else {
            tree.add_child(
                holder,
                Box::new(Button::with_text("Stage").on_click(move || count.set(count.get() + 1))),
                None,
                0,
            )
        };
        tree.node_mut(button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(Some(if dark {
                ButtonSdfRestingProfile::DarkV3
            } else {
                ButtonSdfRestingProfile::LightV3
            }))
            .unwrap();
        let peer = tree.add_child(root, Box::new(Label::new("Peer")), None, 0);
        let peer_id = tree.node(peer).unwrap().id;
        let theme = if dark { Theme::dark() } else { Theme::light() };
        (
            Self::with_tree(tree, root, button, bounds, theme, scale),
            clicks,
            peer_id,
        )
    }

    fn with_tree(
        tree: WidgetTree,
        root: NodeId,
        button: NodeId,
        bounds: RectF,
        theme: Theme,
        scale: f32,
    ) -> Self {
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real V3 GPU required");
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        renderer
            .register_headless_render_target(
                surface,
                (
                    (bounds.width * scale) as u32,
                    (bounds.height * scale) as u32,
                ),
                scale,
            )
            .unwrap();
        let owner = tree.node(button).unwrap().id;
        Self {
            renderer,
            compositor,
            surface,
            tree,
            root,
            button,
            owner,
            domains: InteractionDomains::new(),
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            bindings: BindingRegistry::new(),
            harness: RetainedSceneTestHarness::new(),
            scenes: HashMap::new(),
            theme,
            scale,
            require_image: false,
            last_material_pending: false,
        }
    }

    fn request(&self) -> crate::widgets::button::ButtonSdfRequestSnapshot {
        self.tree
            .node(self.button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Button>()
            .unwrap()
            .sdf_request_snapshot()
            .unwrap()
    }

    fn image(&self) -> crate::core::widget::ButtonSdfPaintInput {
        self.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&self.owner].clone()
    }

    fn frame(&mut self) {
        self.frame_observing(true);
    }

    fn frame_observing(&mut self, observe_completion: bool) {
        test_begin_frame_layout_tracking();
        let target = self
            .renderer
            .custom_render_target_snapshot(self.surface)
            .unwrap();
        let focused = self.domains.primary_state().focus_manager.focused();
        let mode = self.domains.primary_state().keyboard_mode;
        let mut triggers = Vec::new();
        let outcome = build_canonical_scene_reusing(
            &mut self.tree,
            self.root,
            Constraints::tight(
                self.compositor
                    .snapshot_multi()
                    .get(self.surface)
                    .unwrap()
                    .bounds
                    .size(),
            ),
            &self.theme,
            &mut self.renderer,
            &crate::core::icons::builtin_registry(),
            self.harness.frame_time_ms,
            0.0,
            &mut self.harness.transition_layer,
            &mut self.harness.active_layout_transitions,
            None,
            self.scale,
            Some(target),
            &mut self.harness.custom_render_presentation_bindings,
            None,
            None,
            false,
            &mut triggers,
            TooltipSceneInput::Explicit(None),
            focused,
            mode,
            None,
            false,
            &mut self.harness.recycled_scene,
            &mut self.harness.cache,
            Some(GestureSceneAccess {
                domains: &mut self.domains,
                window: WindowId::PRIMARY,
                router: &mut self.router,
                fusion: &mut self.fusion,
            }),
            Some(prepare_canonical_sdf_materials),
            None,
            false,
        );
        self.last_material_pending = outcome.material_pending;
        if self.last_material_pending {
            assert!(outcome.scene.is_none());
            assert!(!outcome.visual_changed && !outcome.needs_next_frame);
            self.scenes.remove(&self.surface);
        } else if let Some(scene) = outcome.scene {
            self.scenes.insert(self.surface, scene);
        }
        self.harness.frame_time_ms += 16;
        self.renderer.drain_pending_image_registrations();
        self.harness
            .cache
            .admit_sdf_materials_after_paint(&mut self.renderer, &self.tree);
        if observe_completion {
            self.renderer
                .gpu_resources()
                .device
                .poll(wgpu::PollType::Poll)
                .unwrap();
        }
        if self.require_image && !self.last_material_pending {
            let input = self.image();
            let image = input
                .image
                .as_ref()
                .expect("V3 state transition cannot lose its image");
            assert_eq!(input.requested_revision, self.request().revision);
            assert_eq!(image.binding.availability(), Ok(()));
            assert!(
                self.harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&input.key, image.binding.identity())
            );
            assert!(
                self.harness
                    .cache
                    .sdf_material_staging
                    .as_ref()
                    .unwrap()
                    .retained_generation_count()
                    <= 2
            );
        }
    }

    fn settle(&mut self, pressed: bool) -> ImageBinding {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                Instant::now() < deadline,
                "real V3 generation did not settle"
            );
            self.frame();
            if self.last_material_pending {
                std::thread::yield_now();
                continue;
            }
            let request = self.request();
            assert_eq!(request.pressed, pressed);
            if let Some(input) = self
                .harness
                .cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&self.owner))
            {
                let current = self.harness.cache.current_sdf_frame_inputs(&self.tree);
                if current.iter().any(|current| current.key == input.key)
                    && input.key.revision() == request.basis_revision
                    && input.requested_revision == request.revision
                {
                    if let Some(image) = &input.image {
                        assert_eq!(image.uv.x, if pressed { 0.5 } else { 0.0 });
                        return image.binding.clone();
                    }
                }
            }
            std::thread::yield_now();
        }
    }

    fn pixels(&mut self, binding: &ImageBinding) -> Vec<[u8; 4]> {
        let bounds = self
            .compositor
            .snapshot_multi()
            .get(self.surface)
            .unwrap()
            .bounds;
        self.pixels_in_clip(binding, bounds)
    }

    fn pixels_in_clip(&mut self, binding: &ImageBinding, clip: RectF) -> Vec<[u8; 4]> {
        assert!(
            !self.last_material_pending,
            "Pending attempt has no current output"
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &self.scenes,
                self.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| self.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = self
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, self.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let data = compiled.plan().surface_compiled_data(self.surface).unwrap();
        let image_batch = data
            .frame_batches
            .batches
            .iter()
            .find(|batch| {
                batch.key.pipeline == PipelineId::Image
                    && batch.key.texture_id == Some(binding.identity().texture_id())
                    && batch.key.image_generation == Some(binding.identity().generation())
            })
            .expect("current admitted SDF Image batch");
        assert_eq!(
            image_batch.clip_state.rect, clip,
            "expanded E keeps the actual inherited clip"
        );
        let gpu = self.renderer.gpu_resources();
        let (width, height) = compiled
            .plan()
            .get_target(self.surface)
            .unwrap()
            .physical_size;
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reference-E canonical Button pixels"),
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
        let complete = compiled
            .submit_headless_for_test(
                &self.renderer,
                self.surface,
                &output.create_view(&Default::default()),
            )
            .unwrap();
        poll_until(&gpu.device, &complete);
        let pixels = read_pixels(&gpu, &output, width, height);
        self.harness.cache.note_sdf_target_output(self.surface);
        pixels
    }

    fn pointer(&mut self, down: bool) {
        self.pointer_at(down, Point::new(80.0, 35.0));
    }

    fn hover_at(&mut self, pos: Point) {
        self.router.route_input_event(
            &mut self.tree,
            FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                delta: (0.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState::default(),
            },
            Some(self.root),
            self.renderer.text_engine_mut(),
            None,
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
    }

    fn pointer_at(&mut self, down: bool, pos: Point) {
        let time = Instant::now();
        let event = if down {
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time,
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            }
        } else {
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time,
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            }
        };
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            self.renderer.text_engine_mut(),
            None,
            self.domains.primary_state_mut(),
            &mut self.bindings,
        );
    }
}

#[test]
fn canonical_button_sdf_theme_policy_all_densities() {
    use crate::core::appearance::ButtonMaterialPolicy;
    use crate::core::theme::DensityMode::{Comfortable, Compact, Intermediate30, Regular};
    use crate::renderer::sdf_material_status::SdfMaterialSource;

    let captions = ["Apply", "Rollback"];
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::row()
                    .with_padding(EdgeInsets::all(20.0))
                    .with_spacing(16.0)
                    .with_align_items(Alignment::Start)
                    .with_background(Color::from_hex(0x808080)),
            ),
            None,
            0,
        );
        tree.set_overflow(root, OverflowMode::Clip);
        let clicks = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
        let buttons: [NodeId; 2] = std::array::from_fn(|index| {
            let count = Rc::clone(&clicks[index]);
            let button = tree.add_child(root, Box::new(Button::new(NodeId(0))), None, index as i16);
            let caption = tree.add_child(button, Box::new(Label::new(captions[index])), None, 0);
            crate::replace_widget!(
                tree,
                button,
                Button::new(caption).on_click(move || count.set(count.get() + 1))
            );
            button
        });
        let owners = buttons.map(|button| tree.node(button).unwrap().id);
        let mut theme = Theme::light_with_density(Compact);
        theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
        let bounds = RectF::new(0.0, 0.0, 240.0, 100.0);
        let mut trial = ReferenceETrial::with_tree(tree, root, buttons[0], bounds, theme, scale);

        for (density, dark) in [
            (Compact, false),
            (Regular, false),
            (Intermediate30, false),
            (Comfortable, false),
            (Comfortable, true),
            (Intermediate30, true),
            (Regular, true),
            (Compact, true),
            (Compact, false),
        ] {
            trial.theme.set_density(density);
            if dark != trial.theme.is_dark() {
                trial.theme.toggle();
            }
            trial.tree.mark_subtree_dirty(root);
            let deadline = Instant::now() + Duration::from_secs(15);
            let bindings = loop {
                assert!(
                    Instant::now() < deadline,
                    "{density:?} dark={dark} {scale}x did not settle"
                );
                trial.frame();
                if trial.last_material_pending {
                    continue;
                }
                let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
                assert_eq!(current.len(), buttons.len());
                let mut bindings = Vec::new();
                for ((button, owner), caption) in buttons.into_iter().zip(owners).zip(captions) {
                    assert_eq!(trial.tree.node(button).unwrap().id, owner);
                    let request = current
                        .iter()
                        .find(|input| input.key.owner() == owner)
                        .unwrap();
                    assert_eq!(request.source, SdfMaterialSource::V3 { dark });
                    let face = trial.tree.get_absolute_paint_rect(button).unwrap();
                    assert_eq!(request.key.metrics().final_logical_rect, face);
                    assert_eq!(request.key.metrics().scale_factor, scale);
                    assert_eq!(face.height, trial.theme.geometry.button.height);

                    let local = trial.harness.local_fragment(owner).unwrap();
                    assert!(
                        leaves(&local)
                            .iter()
                            .any(|node| matches!(node, SceneNode::Text(text)
                        if text.text == caption))
                    );
                    let painted: Vec<_> = leaves(&local)
                        .into_iter()
                        .filter_map(|node| match node {
                            SceneNode::Image(image) => Some(image),
                            _ => None,
                        })
                        .collect();
                    let input = trial
                        .harness
                        .cache
                        .sdf_paint_inputs
                        .as_ref()
                        .and_then(|inputs| inputs.get(&owner));
                    if let Some((input, image)) =
                        input.and_then(|input| input.image.as_ref().map(|image| (input, image)))
                    {
                        assert_eq!(input.key, request.key, "stale geometry or material basis");
                        assert_eq!(input.requested_revision, request.request_revision);
                        assert_eq!(image.binding.availability(), Ok(()));
                        assert!(
                            trial
                                .harness
                                .cache
                                .sdf_scene_custody
                                .has_reference(&input.key, image.binding.identity(),)
                        );
                        assert_eq!(painted.len(), 1);
                        assert_eq!(
                            painted[0].texture_source,
                            ImageTextureSource::Admitted(image.binding.identity()),
                        );
                        assert!(painted[0].rect.contains(Point::new(
                            face.x + face.width / 2.0,
                            face.y + face.height / 2.0
                        )));
                        bindings.push(image.binding.clone());
                    } else {
                        assert!(
                            painted.is_empty(),
                            "only a current admission may paint an Image"
                        );
                    }
                }
                if bindings.len() == buttons.len() {
                    break bindings;
                }
                std::thread::yield_now();
            };
            assert_ne!(bindings[0].identity(), bindings[1].identity());
            // Compile and submit the actual admitted scene to the original GPU.
            trial.pixels(&bindings[0]);
        }
        for ((button, owner), count) in buttons.into_iter().zip(owners).zip(clicks) {
            let face = trial.tree.get_absolute_paint_rect(button).unwrap();
            let point = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
            trial.pointer_at(true, point);
            trial.pointer_at(false, point);
            assert_eq!(
                trial.domains.primary_state().focus_manager.focused(),
                Some(owner)
            );
            assert_eq!(count.get(), 1);
        }
    }
}

fn verify_narrow_stage(dark: bool, scale: f32, directory: &std::path::Path) {
    use crate::core::paint::{LAYER_CONTENT, LAYER_FOCUS};
    use std::io::Write;

    let (mut trial, clicks, peer) = ReferenceETrial::new(dark, scale, 84.0, true);
    let binding = trial.settle(false);
    trial.require_image = true;
    let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
    assert_eq!(face, RectF::new(20.0, 20.0, 84.0, 30.0));
    let run = trial
        .renderer
        .text_engine_mut()
        .shape_styled("Stage", None, 13.0)
        .unwrap();
    assert!(run.width + 2.0 * trial.theme.geometry.button.padding_x <= face.width);
    let peer_local = trial.harness.local_fragment(peer).unwrap();
    let local = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    assert!(!local.iter().any(
        |node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS + 1)
    ));
    let label = local
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(text) if text.text == "Stage" => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(label.font_size, 13.0);
    assert!((label.position.x - (face.x + (face.width - run.width) * 0.5)).abs() < 0.001);
    let input = trial.image();
    let image = input.image.unwrap();
    let outsets = if scale == 1.0 {
        [4.4, 4.4, 3.3, 5.5]
    } else {
        [4.15, 4.15, 3.05, 5.25]
    };
    let expected = RectF::new(
        face.x - outsets[0],
        face.y - outsets[2],
        face.width + outsets[0] + outsets[1],
        face.height + outsets[2] + outsets[3],
    );
    assert_eq!(image.coverage, expected);
    let resting = trial.pixels(&binding);
    let width = (240.0 * scale) as usize;
    let at = |pixels: &[[u8; 4]], x: f32, y: f32| {
        pixels[(y * scale) as usize * width + (x * scale) as usize]
    };
    assert!(at(&resting, 62.0, 52.0)[0] < at(&resting, 62.0, 56.0)[0]);
    assert_eq!(
        at(&resting, 62.0, 56.0),
        if dark {
            [36, 36, 36, 255]
        } else {
            [242, 242, 242, 255]
        }
    );
    let resting_request = trial.request();
    trial.hover_at(Point::new(62.0, 35.0));
    trial.frame();
    assert_eq!(
        trial.request(),
        resting_request,
        "hover does not regenerate the pair"
    );
    assert_eq!(
        trial.image().image.as_ref().unwrap().binding.identity(),
        binding.identity()
    );
    assert_eq!(trial.domains.primary_state().focus_manager.focused(), None);
    let hovered = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    let hover_border: Vec<_> = hovered
        .iter()
        .filter_map(|node| match node {
            SceneNode::BorderRect(border) if border.z_index == LAYER_CONTENT + 1 => Some(border),
            _ => None,
        })
        .collect();
    assert_eq!(hover_border.len(), 1);
    assert_eq!(hover_border[0].rect, face);
    assert_eq!(
        hover_border[0].radius, 3.0,
        "hover preserves the V3 face silhouette"
    );
    assert!(!hovered.iter().any(
        |node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS + 1)
    ));
    assert!(Arc::ptr_eq(
        &peer_local,
        &trial.harness.local_fragment(peer).unwrap()
    ));
    let hover_pixels = trial.pixels(&binding);
    trial.hover_at(Point::new(170.0, 85.0));
    trial.frame();
    assert_eq!(trial.request(), resting_request);
    assert!(!leaves(&trial.harness.local_fragment(trial.owner).unwrap()).iter().any(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_CONTENT + 1)));
    assert_eq!(
        trial.pixels(&binding),
        resting,
        "leaving hover restores the same retained material"
    );
    trial.pointer(true);
    trial.frame();
    assert_eq!(
        trial.image().image.as_ref().unwrap().binding.identity(),
        binding.identity()
    );
    assert_eq!(trial.image().image.as_ref().unwrap().uv.x, 0.5);
    let focused = leaves(&trial.scenes[&trial.surface].root);
    let cue: Vec<_> = focused
        .iter()
        .filter_map(|node| match node {
            SceneNode::BorderRect(border) if border.z_index == LAYER_FOCUS + 1 => Some(border),
            _ => None,
        })
        .collect();
    assert_eq!(
        cue.len(),
        1,
        "one complete current cue, no appended framework ring"
    );
    assert_eq!(cue[0].rect, face);
    assert_eq!(cue[0].radius, 3.0);
    assert!(focused.iter().any(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == LAYER_CONTENT + 1 && border.radius == 3.0)));
    assert!(cue[0].z_index > label.z_index);
    assert!(
        cue[0].z_index
            > local
                .iter()
                .find_map(|node| match node {
                    SceneNode::Image(image) => Some(image.z_index),
                    _ => None,
                })
                .unwrap()
    );
    let pressed_label = focused
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(text) if text.text == "Stage" => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(pressed_label.position, label.position);
    let pressed = trial.pixels(&binding);
    trial.pointer(false);
    trial.frame();
    assert_eq!(clicks.get(), 1);
    assert_eq!(
        trial.image().image.as_ref().unwrap().binding.identity(),
        binding.identity()
    );
    assert_eq!(trial.image().image.as_ref().unwrap().uv.x, 0.0);
    assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(face));
    assert!(Arc::ptr_eq(
        &peer_local,
        &trial.harness.local_fragment(peer).unwrap()
    ));
    let released_request = trial.request();
    trial.hover_at(Point::new(62.0, 35.0));
    trial.frame();
    assert_eq!(trial.request(), released_request);
    assert_eq!(
        trial.image().image.as_ref().unwrap().binding.identity(),
        binding.identity()
    );
    let focused_hover = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    for z in [LAYER_CONTENT + 1, LAYER_FOCUS + 1] {
        let borders: Vec<_> = focused_hover
            .iter()
            .filter_map(|node| match node {
                SceneNode::BorderRect(border) if border.z_index == z => Some(border),
                _ => None,
            })
            .collect();
        assert_eq!(borders.len(), 1);
        assert_eq!(borders[0].radius, 3.0);
    }
    let focused_hover_pixels = trial.pixels(&binding);
    trial.pointer_at(true, Point::new(130.0, 35.0));
    trial.pointer_at(false, Point::new(130.0, 35.0));
    trial.frame();
    assert_eq!(
        clicks.get(),
        1,
        "the old 120px allocation is not a hit target"
    );
    let narrow = directory.join("narrow");
    std::fs::create_dir_all(&narrow).unwrap();
    for (name, pixels) in [
        ("rest", &resting),
        ("hover", &hover_pixels),
        ("pressed", &pressed),
        ("focused-hover", &focused_hover_pixels),
    ] {
        let mut file = std::fs::File::create(narrow.join(format!(
            "{}-{}x-{name}.ppm",
            if dark { "dark" } else { "light" },
            scale as u32
        )))
        .unwrap();
        write!(file, "P6\n{} {}\n255\n", width, (100.0 * scale) as u32).unwrap();
        for pixel in pixels {
            file.write_all(&pixel[..3]).unwrap();
        }
    }
    trial.require_image = false;
    trial
        .tree
        .node_mut(trial.button)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Button>()
        .unwrap()
        .request_sdf_material(None)
        .unwrap();
    trial.frame();
    assert!(
        trial
            .harness
            .cache
            .sdf_scene_custody
            .references()
            .is_empty()
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while trial.harness.cache.sdf_material_staging.is_some() {
        assert!(Instant::now() < deadline);
        trial.frame();
    }
    assert_eq!(
        binding.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
    println!(
        "V3 narrow {} {scale}x: F={face:?}, glyph_width={}, padding=11+11, hover/press/focus outlines radius3, real enter/leave and stable pair/custody/hit/retirement PASS",
        if dark { "dark" } else { "light" },
        run.width
    );
}

#[test]
fn canonical_button_sdf_v3_reference_e_pixels_are_stable() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/stage-v3-hover-rounding/pixels");
    std::fs::create_dir_all(&directory).unwrap();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let (mut trial, clicks, peer_id) = ReferenceETrial::new(dark, scale, 120.0, false);
        let button = trial.button;
        let owner = trial.owner;
        let resting_binding = trial.settle(false);
        trial.require_image = true;
        let face = trial.tree.get_absolute_paint_rect(button).unwrap();
        assert_eq!(face, RectF::new(20.0, 20.0, 120.0, 30.0));
        let peer_local = trial.harness.local_fragment(peer_id).unwrap();
        let resting = trial.pixels(&resting_binding);
        let stable = trial.request();
        for _ in 0..8 {
            trial.frame();
        }
        assert_eq!(
            trial.request(),
            stable,
            "grain cannot churn material revisions"
        );
        assert_eq!(
            trial.image().image.unwrap().binding.identity(),
            resting_binding.identity()
        );
        assert_eq!(
            resting,
            trial.pixels(&resting_binding),
            "grain must not flicker between frames"
        );
        assert_eq!(trial.domains.primary_state().focus_manager.focused(), None);
        let width = (240.0 * scale) as usize;
        let sample = |pixels: &[[u8; 4]], x: f32, y: f32| {
            pixels[(y * scale) as usize * width + (x * scale) as usize]
        };
        let center = sample(&resting, 32.0, 35.0);
        if !dark {
            assert!(
                center[..3].iter().all(|value| (223..=225).contains(value)),
                "selected LightV3 resting tone: {center:?}"
            );
        }
        let edge = sample(&resting, 20.5, 35.0);
        assert!(
            edge[0] + 5 < center[0],
            "material perimeter exists without FocusCue"
        );
        // Broad illumination is constant along each of these interior rows.
        // Include every physical pixel; one logical row can quantize flat at 2x.
        let grain_range = ((28.0 * scale) as usize..(43.0 * scale) as usize)
            .map(|y| {
                let row = &resting
                    [y * width + (30.0 * scale) as usize..y * width + (46.0 * scale) as usize];
                row.iter().map(|pixel| pixel[0]).max().unwrap()
                    - row.iter().map(|pixel| pixel[0]).min().unwrap()
            })
            .max()
            .unwrap();
        assert!(
            (1..=3).contains(&grain_range),
            "subtle deterministic face grain: {grain_range}"
        );
        assert!(
            sample(&resting, 80.0, 52.0)[0] < sample(&resting, 80.0, 60.0)[0],
            "finite contact shadow must remain visible outside F"
        );
        if !dark {
            let background = sample(&resting, 80.0, 60.0);
            let contact = sample(&resting, 80.0, 52.0);
            let side = sample(&resting, 18.0, 35.0);
            let lower = sample(&resting, 80.0, 54.0);
            let tail = sample(&resting, 80.0, 56.0);
            println!(
                "LightV3 {scale}x shadow output: background {background:?}, contact {contact:?}, side {side:?}, lower {lower:?}, tail {tail:?}"
            );
            assert_eq!(tail, background, "LightV3 finite shadow tail at {scale}x");
            for channel in 0..3 {
                let contrast =
                    |pixel: [u8; 4]| i16::from(background[channel]) - i16::from(pixel[channel]);
                assert!(
                    (6..=16).contains(&contrast(contact)),
                    "LightV3 {scale}x contact contrast channel {channel}: {}",
                    contrast(contact)
                );
                assert!(
                    (1..=12).contains(&contrast(side)),
                    "LightV3 {scale}x side contrast channel {channel}: {}",
                    contrast(side)
                );
                assert!(
                    contrast(contact) > contrast(lower),
                    "LightV3 {scale}x contact must be darker than the lower falloff"
                );
            }
        }
        trial.pointer(true);
        trial.frame();
        let press_request = trial.request();
        assert_ne!(press_request.revision, stable.revision);
        assert_eq!(press_request.basis_revision, stable.basis_revision);
        assert_eq!(trial.image().requested_revision, press_request.revision);
        assert_eq!(trial.image().image.as_ref().unwrap().uv.x, 0.5);
        let pressed_binding = trial.image().image.unwrap().binding;
        assert_eq!(
            pressed_binding.identity(),
            resting_binding.identity(),
            "first warm Paint selects Pressed from the already admitted pair"
        );
        let pressed = trial.pixels(&pressed_binding);
        let pressed_center = sample(&pressed, 32.0, 35.0);
        if !dark {
            assert!(
                pressed_center[..3]
                    .iter()
                    .all(|value| (216..=218).contains(value)),
                "selected LightV3 pressed tone: {pressed_center:?}"
            );
            assert!((6..=8).contains(&(center[0] - pressed_center[0])));
        }
        assert!(
            pressed_center[0] + 2 < center[0],
            "pressed face is visibly depressed"
        );
        assert!(
            sample(&pressed, 80.0, 22.5)[0] < sample(&pressed, 80.0, 47.5)[0],
            "pressed relief must remain concave"
        );
        assert_eq!(clicks.get(), 0);
        trial.pointer(false);
        trial.frame();
        assert_eq!(trial.image().requested_revision, trial.request().revision);
        assert_eq!(trial.image().image.as_ref().unwrap().uv.x, 0.0);
        let released = trial.image().image.unwrap().binding;
        assert_eq!(
            released.identity(),
            resting_binding.identity(),
            "first release Paint uses Rest without a new generation"
        );
        assert_eq!(clicks.get(), 1);
        let release_pixels = trial.pixels(&released);
        for y in 24..46 {
            for x in 25..135 {
                assert_eq!(
                    sample(&resting, x as f32, y as f32),
                    sample(&release_pixels, x as f32, y as f32),
                    "same material UV after regeneration"
                );
            }
        }
        assert_eq!(trial.tree.get_absolute_paint_rect(button), Some(face));
        assert!(Arc::ptr_eq(
            &peer_local,
            &trial.harness.local_fragment(peer_id).unwrap()
        ));
        assert!(leaves(&trial.harness.local_fragment(owner).unwrap()).iter()
            .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage" && text.font_size == 13.0)));

        // A different authored basis cannot borrow either cell of this pair.
        // An actual unsubmitted compiled plan keeps its old generation alive.
        let old_plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let old_compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(old_plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        trial.require_image = false;
        trial
            .tree
            .node_mut(button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(Some(if dark {
                ButtonSdfRestingProfile::LightV3
            } else {
                ButtonSdfRestingProfile::DarkV3
            }))
            .unwrap();
        trial.frame();
        assert_ne!(trial.request().basis_revision, stable.basis_revision);
        assert!(
            trial.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&owner]
                .image
                .is_none(),
            "changed basis has no cross-basis last-good cell before admission"
        );
        assert!(
            !leaves(&trial.harness.local_fragment(owner).unwrap())
                .iter()
                .any(|node| matches!(node, SceneNode::Image(_)))
        );
        assert_eq!(resting_binding.availability(), Ok(()));
        let next_basis = trial.settle(false);
        assert_ne!(next_basis.identity(), resting_binding.identity());
        assert_eq!(
            resting_binding.availability(),
            Ok(()),
            "old CPU plan still owns custody"
        );
        drop(old_compiled);
        trial.require_image = true;
        let deadline = Instant::now() + Duration::from_secs(15);
        while resting_binding.availability().is_ok() {
            assert!(Instant::now() < deadline);
            trial.frame();
        }
        assert_eq!(next_basis.availability(), Ok(()));
        trial.require_image = false;
        trial
            .tree
            .node_mut(button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        trial.frame();
        assert!(
            trial
                .harness
                .cache
                .sdf_scene_custody
                .references()
                .is_empty()
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        while trial.harness.cache.sdf_material_staging.is_some() {
            assert!(Instant::now() < deadline);
            trial.frame();
        }
        assert_eq!(
            released.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(
            next_basis.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        for (name, pixels) in [("rest", &resting), ("pressed", &pressed)] {
            use std::io::Write;
            let mut file = std::fs::File::create(directory.join(format!(
                "{}-{}x-{name}.ppm",
                if dark { "dark" } else { "light" },
                scale as u32
            )))
            .unwrap();
            write!(file, "P6\n{} {}\n255\n", width, (100.0 * scale) as u32).unwrap();
            for pixel in pixels {
                file.write_all(&pixel[..3]).unwrap();
            }
        }
        println!(
            "V3 {} {}x: rest {center:?}, perimeter {edge:?}, grain range {grain_range}, pressed {pressed_center:?}; authentic pixels, stable UV/revision, real press/release and retirement PASS",
            if dark { "dark" } else { "light" },
            scale
        );
        verify_narrow_stage(dark, scale, &directory);
    }
}

#[test]
fn canonical_button_sdf_v3_press_release_publication_trace() {
    use crate::renderer::image_binding::ImageBindingIdentity;
    use crate::renderer::sdf_material_status::{SdfMaterialRevision, SdfMaterialStatus};
    use serde_json::json;

    struct Trace {
        start: Instant,
        frame: usize,
        revisions: Vec<(SdfMaterialRevision, bool)>,
        completions: Vec<(ImageBindingIdentity, Box<dyn Fn() -> bool>)>,
        pair_identity: Option<ImageBindingIdentity>,
        rows: Vec<serde_json::Value>,
    }

    impl Trace {
        fn record(&mut self, trial: &ReferenceETrial, transition: &str, phase: &str) {
            let button = trial
                .tree
                .node(trial.button)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap();
            let request = button.sdf_request_snapshot();
            if let Some(request) = request {
                if !self
                    .revisions
                    .iter()
                    .any(|(revision, _)| *revision == request.revision)
                {
                    self.revisions.push((request.revision, request.pressed));
                }
            }
            let cache = &trial.harness.cache;
            let selected = cache
                .selected_sdf_materials
                .as_ref()
                .and_then(|selected| selected.get(&trial.owner))
                .and_then(Option::as_ref);
            let image = cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&trial.owner))
                .and_then(|input| input.image.as_ref());
            let selected_pressed = image.map(|image| image.uv.x == 0.5);
            let row = json!({
                "transition": transition, "phase": phase, "frame": self.frame,
                "elapsed_ms": self.start.elapsed().as_secs_f64() * 1000.0,
                "monotonic": format!("{:?}", Instant::now()),
                "button_state": format!("{:?}", button.state),
                "keyboard_pressed": button.keyboard_pulse_active.get(),
                "request_revision": request.map(|request| format!("{:?}", request.revision)),
                "basis_revision": request.map(|request| format!("{:?}", request.basis_revision)),
                "request_pressed": request.map(|request| request.pressed),
                "selected_revision": selected.map(|selected| format!("{:?}", selected.key().revision())),
                "selected_pressed": selected_pressed,
                "image": image.map(|image| format!("{:?}", image.binding.identity())),
                "uv": image.map(|image| format!("{:?}", image.uv)),
                "generations": cache.sdf_material_staging.as_ref().map_or(0, |staging| staging.retained_generation_count()),
                "needs_followup": cache.sdf_material_staging.as_ref().is_some_and(|staging| staging.needs_followup()),
            });
            println!("SDF_TRACE {row}");
            self.rows.push(row);
        }
    }

    // The existing canonical hook brackets real preparation/submission. These
    // timestamps are CPU observations, never GPU execution/display timestamps.
    fn preparation(renderer: &mut WgpuRenderer, tree: &WidgetTree, cache: &mut RetainedSceneCache) {
        let current = cache.current_sdf_frame_inputs(tree);
        println!(
            "SDF_PREP begin {:?} requests={:?}",
            Instant::now(),
            current
                .iter()
                .map(|input| (input.key.revision(), input.request_revision, input.pressed))
                .collect::<Vec<_>>()
        );
        prepare_canonical_sdf_materials(renderer, tree, cache);
        println!(
            "SDF_PREP end {:?} generations={} followup={}",
            Instant::now(),
            cache
                .sdf_material_staging
                .as_ref()
                .map_or(0, |staging| staging.retained_generation_count()),
            cache
                .sdf_material_staging
                .as_ref()
                .is_some_and(|staging| staging.needs_followup())
        );
    }

    fn frame(
        trial: &mut ReferenceETrial,
        trace: &mut Trace,
        transition: &str,
        view: &wgpu::TextureView,
        cancel: bool,
    ) -> bool {
        trace.frame += 1;
        trace.record(trial, transition, "frame_begin");
        test_begin_frame_layout_tracking();
        let target = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap();
        let focused = trial.domains.primary_state().focus_manager.focused();
        let mode = trial.domains.primary_state().keyboard_mode;
        let mut triggers = Vec::new();
        let outcome = build_canonical_scene_reusing(
            &mut trial.tree,
            trial.root,
            Constraints::tight(Size::new(240.0, 100.0)),
            &trial.theme,
            &mut trial.renderer,
            &crate::core::icons::builtin_registry(),
            trial.harness.frame_time_ms,
            0.0,
            &mut trial.harness.transition_layer,
            &mut trial.harness.active_layout_transitions,
            None,
            trial.scale,
            Some(target),
            &mut trial.harness.custom_render_presentation_bindings,
            None,
            None,
            false,
            &mut triggers,
            TooltipSceneInput::Explicit(None),
            focused,
            mode,
            None,
            false,
            &mut trial.harness.recycled_scene,
            &mut trial.harness.cache,
            Some(GestureSceneAccess {
                domains: &mut trial.domains,
                window: WindowId::PRIMARY,
                router: &mut trial.router,
                fusion: &mut trial.fusion,
            }),
            Some(preparation),
            None,
            false,
        );
        trial.last_material_pending = outcome.material_pending;
        if trial.last_material_pending {
            assert!(outcome.scene.is_none());
            assert!(!outcome.visual_changed && !outcome.needs_next_frame);
            trial.scenes.remove(&trial.surface);
        } else if let Some(scene) = outcome.scene {
            trial.scenes.insert(trial.surface, scene);
        }
        trial.harness.frame_time_ms += 16;
        trace.record(trial, transition, "paint_end");
        println!(
            "SDF_FRAME_DEMAND frame={} needs_next_frame={}",
            trace.frame, outcome.needs_next_frame
        );
        trial.renderer.drain_pending_image_registrations();
        let followup = trial
            .harness
            .cache
            .admit_sdf_materials_after_paint(&mut trial.renderer, &trial.tree);
        let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
        let admitted = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map(|staging| staging.publications_before_paint(&mut trial.renderer, &current))
            .unwrap_or_default();
        println!(
            "SDF_ADMISSION {:?} frame={} followup={} current={:?}",
            Instant::now(),
            trace.frame,
            followup,
            admitted
                .iter()
                .map(|(publication, identity)| (publication.key().revision(), identity))
                .collect::<Vec<_>>()
        );
        trace.record(trial, transition, "admission_end");
        let mut current_image = false;
        if let Some(input) = trial
            .harness
            .cache
            .sdf_paint_inputs
            .as_ref()
            .and_then(|inputs| inputs.get(&trial.owner))
            .cloned()
        {
            let image = input
                .image
                .as_ref()
                .expect("a warmed transition cannot have a blank material");
            assert_eq!(image.binding.availability(), Ok(()));
            if let Some(identity) = trace.pair_identity {
                assert_eq!(
                    image.binding.identity(),
                    identity,
                    "warm endpoint selection must not create/admit another texture"
                );
            } else {
                trace.pair_identity = Some(image.binding.identity());
            }
            if trial.require_image {
                assert_eq!(
                    trial
                        .harness
                        .cache
                        .sdf_material_staging
                        .as_ref()
                        .unwrap()
                        .retained_generation_count(),
                    1
                );
                assert!(
                    !followup,
                    "warm UV selection has no preparation/admission follow-up"
                );
            }
            let publication = trial.harness.cache.selected_sdf_materials.as_ref().unwrap()
                [&trial.owner]
                .as_ref()
                .unwrap();
            assert_eq!(publication.key(), &input.key);
            assert!(
                matches!(publication.status(), SdfMaterialStatus::Ready { binding }
                if binding.identity() == image.binding.identity())
            );
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&input.key, image.binding.identity())
            );
            if let Some(request) = trial
                .tree
                .node(trial.button)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap()
                .sdf_request_snapshot()
            {
                assert_eq!(input.requested_revision, request.revision);
                current_image = input.key.revision() == request.basis_revision
                    && input.requested_revision == request.revision;
                assert_eq!(
                    image.uv.x,
                    if request.pressed { 0.5 } else { 0.0 },
                    "first legal warm Paint selects the current endpoint"
                );
                assert!(
                    current_image,
                    "a warm press cannot wait for another admission"
                );
            }
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &trial.scenes,
                    trial.compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    |_| trial.scale,
                    0,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            let compiled = trial
                .renderer
                .compile_sdf_frame(
                    SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                    None,
                )
                .unwrap()
                .expect_ready("native Compile must be ready");
            assert!(
                compiled
                    .plan()
                    .surface_compiled_data(trial.surface)
                    .unwrap()
                    .frame_batches
                    .batches
                    .iter()
                    .any(|batch| batch.key.pipeline == PipelineId::Image
                        && batch.key.texture_id == Some(image.binding.identity().texture_id())
                        && batch.key.image_generation
                            == Some(image.binding.identity().generation()))
            );
            trace.record(trial, transition, "image_assembled");
            if cancel {
                drop(compiled);
                trace.record(trial, transition, "compiled_cancelled");
            } else {
                let completed = compiled
                    .submit_headless_for_test(&trial.renderer, trial.surface, view)
                    .unwrap();
                trace
                    .completions
                    .push((image.binding.identity(), Box::new(completed)));
                trial.harness.cache.note_sdf_target_output(trial.surface);
                trace.record(trial, transition, "sampling_submitted");
            }
        } else if trial.require_image {
            panic!("warm request lost its selected material");
        }
        trial
            .renderer
            .gpu_resources()
            .device
            .poll(wgpu::PollType::Poll)
            .unwrap();
        trace.completions.retain(|(identity, complete)| {
            if complete() {
                println!(
                    "SDF_COMPLETION {:?} frame={} sampled={identity:?}",
                    Instant::now(),
                    trace.frame
                );
                false
            } else {
                true
            }
        });
        assert!(
            trial
                .harness
                .cache
                .sdf_material_staging
                .as_ref()
                .is_none_or(|staging| staging.retained_generation_count() <= 2)
        );
        trace.record(trial, transition, "frame_end");
        current_image
    }

    fn until_current(
        trial: &mut ReferenceETrial,
        trace: &mut Trace,
        name: &str,
        event_at: Instant,
        pressed: bool,
        view: &wgpu::TextureView,
    ) -> ImageBinding {
        let first_frame = trace.frame;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                Instant::now() < deadline,
                "{name}: bounded observation expired"
            );
            let current = frame(trial, trace, name, view, false);
            assert_eq!(trial.request().pressed, pressed);
            if current {
                println!(
                    "SDF_TRANSITION {}",
                    json!({"transition": name,
                    "frames": trace.frame - first_frame,
                    "event_to_current_submitted_ms": event_at.elapsed().as_secs_f64() * 1000.0,
                    "binding": format!("{:?}", trial.image().image.as_ref().unwrap().binding.identity())})
                );
                return trial.image().image.unwrap().binding;
            }
            std::thread::yield_now();
        }
    }

    fn route(
        trial: &mut ReferenceETrial,
        trace: &mut Trace,
        name: &str,
        raw: RawInputEvent,
    ) -> Instant {
        let at = Instant::now();
        println!("SDF_INPUT {:?} {name} raw={raw:?}", at);
        trace.record(trial, name, "event_before");
        let mut events = Vec::new();
        trial.fusion.process_raw_with_gesture_sessions(
            raw,
            InteractionDomainId::PRIMARY,
            &mut trial.domains.primary_state_mut().gesture_registry,
            &trial.tree,
            None,
            &mut events,
        );
        trial.fusion.flush_with_gesture_sessions(&mut events);
        assert!(!events.is_empty());
        for input in events {
            if !dispatch_engine_action(
                input.event(),
                &mut trial.bindings,
                &mut trial.router,
                &mut trial.tree,
                trial.root,
                trial.domains.primary_state_mut(),
                trial.renderer.text_engine_mut(),
                None,
            ) {
                trial.router.route_session_input_event(
                    &mut trial.tree,
                    input,
                    Some(trial.root),
                    trial.renderer.text_engine_mut(),
                    None,
                    trial.domains.primary_state_mut(),
                    &mut trial.bindings,
                );
            }
        }
        trace.record(trial, name, "event_after");
        println!(
            "SDF_EVENT_RETURN {name} elapsed_ms={}",
            at.elapsed().as_secs_f64() * 1000.0
        );
        at
    }

    fn pointer(trial: &mut ReferenceETrial, trace: &mut Trace, name: &str, down: bool) -> Instant {
        route(
            trial,
            trace,
            name,
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: if down {
                    RawButtonState::Pressed
                } else {
                    RawButtonState::Released
                },
                position: Point::new(80.0, 35.0),
            },
        )
    }

    fn key(trial: &mut ReferenceETrial, trace: &mut Trace, name: &str, down: bool) -> Instant {
        route(
            trial,
            trace,
            name,
            RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Enter,
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                state: if down { KeyState::Down } else { KeyState::Up },
                repeat: false,
                modifiers: Modifiers::default(),
            },
        )
    }

    let mut trace = Trace {
        start: Instant::now(),
        frame: 0,
        revisions: Vec::new(),
        completions: Vec::new(),
        pair_identity: None,
        rows: Vec::new(),
    };
    let mut renderer =
        pollster::block_on(WgpuRenderer::new_headless()).expect("real V3 GPU required");
    let bounds = RectF::new(0.0, 0.0, 240.0, 100.0);
    let mut compositor = Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (480, 200), 2.0)
        .unwrap();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Flex::column()
                .with_padding(EdgeInsets::all(20.0))
                .with_spacing(16.0)
                .with_align_items(Alignment::Start)
                .with_background(Color::from_hex(0xF2F2F2)),
        ),
        None,
        0,
    );
    tree.set_overflow(root, OverflowMode::Clip);
    let holder = tree.add_child(
        root,
        Box::new(Container::empty().with_width(120.0).with_height(30.0)),
        None,
        0,
    );
    let clicks = Rc::new(Cell::new(0));
    let click_times = Rc::new(RefCell::new(Vec::new()));
    let count = clicks.clone();
    let times = click_times.clone();
    let mut button = Button::with_text("Stage").on_click(move || {
        count.set(count.get() + 1);
        times.borrow_mut().push(Instant::now());
    });
    button
        .request_sdf_material(Some(ButtonSdfRestingProfile::LightV3))
        .unwrap();
    let button = tree.add_child(holder, Box::new(button), None, 0);
    let owner = tree.node(button).unwrap().id;
    let peer = tree.add_child(root, Box::new(Label::new("Peer")), None, 0);
    let peer_owner = tree.node(peer).unwrap().id;
    let mut trial = ReferenceETrial {
        renderer,
        compositor,
        surface,
        tree,
        root,
        button,
        owner,
        domains: InteractionDomains::new(),
        router: EventRouter::new(),
        fusion: InputFusion::new(),
        bindings: BindingRegistry::new(),
        harness: RetainedSceneTestHarness::new(),
        scenes: HashMap::new(),
        theme: Theme::light(),
        scale: 2.0,
        require_image: false,
        last_material_pending: false,
    };
    let gpu = trial.renderer.gpu_resources();
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("V3 latency genuine sampled scene"),
        size: wgpu::Extent3d {
            width: 480,
            height: 200,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = output.create_view(&Default::default());
    let cold = Instant::now();
    let initial = until_current(&mut trial, &mut trace, "cold-rest", cold, false, &view);
    trial.require_image = true;
    let face = trial.tree.get_absolute_paint_rect(button).unwrap();
    assert_eq!(face, RectF::new(20.0, 20.0, 120.0, 30.0));
    let peer_local = trial.harness.local_fragment(peer_owner).unwrap();
    let root_local = trial
        .harness
        .local_fragment(trial.tree.node(root).unwrap().id)
        .unwrap();

    let down = pointer(&mut trial, &mut trace, "warm-pointer-down", true);
    assert_eq!(clicks.get(), 0);
    let pressed = until_current(
        &mut trial,
        &mut trace,
        "warm-pointer-down",
        down,
        true,
        &view,
    );
    for _ in 0..3 {
        frame(&mut trial, &mut trace, "held", &view, false);
    }
    assert_eq!(
        trial.image().image.unwrap().binding.identity(),
        pressed.identity()
    );
    let up = pointer(&mut trial, &mut trace, "warm-pointer-up", false);
    assert_eq!(clicks.get(), 1, "click precedes any release-frame GPU work");
    println!(
        "SDF_CLICK pointer-up delay_ms={}",
        click_times.borrow()[0].duration_since(up).as_secs_f64() * 1000.0
    );
    let released = until_current(&mut trial, &mut trace, "warm-pointer-up", up, false, &view);
    for _ in 0..2 {
        assert!(frame(
            &mut trial,
            &mut trace,
            "release-stable",
            &view,
            false
        ));
    }

    pointer(&mut trial, &mut trace, "rapid-down", true);
    frame(&mut trial, &mut trace, "rapid-down-frame-A", &view, false);
    assert_eq!(trial.image().key.revision(), trial.request().basis_revision);
    assert_eq!(trial.image().requested_revision, trial.request().revision);
    assert_eq!(
        trial.image().image.as_ref().unwrap().binding.identity(),
        released.identity(),
        "rapid warm Down selects the preadmitted pair immediately"
    );
    assert_eq!(trial.image().image.as_ref().unwrap().uv.x, 0.5);
    let rapid_up = pointer(&mut trial, &mut trace, "rapid-up", false);
    assert_eq!(clicks.get(), 2);
    until_current(&mut trial, &mut trace, "rapid-up", rapid_up, false, &view);
    for _ in 0..2 {
        assert!(frame(
            &mut trial,
            &mut trace,
            "rapid-release-stable",
            &view,
            false
        ));
    }
    pointer(&mut trial, &mut trace, "same-intake-down", true);
    let immediate_up = pointer(&mut trial, &mut trace, "same-intake-up", false);
    assert_eq!(clicks.get(), 3);
    until_current(
        &mut trial,
        &mut trace,
        "same-intake-up",
        immediate_up,
        false,
        &view,
    );

    trial.router.request_focus_with_text_engine(
        &mut trial.tree,
        owner,
        trial.domains.primary_state_mut(),
        Some(trial.renderer.text_engine_mut()),
    );
    assert_eq!(
        trial.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    let keyboard_down = key(&mut trial, &mut trace, "keyboard-down", true);
    until_current(
        &mut trial,
        &mut trace,
        "keyboard-down",
        keyboard_down,
        true,
        &view,
    );
    assert_eq!(clicks.get(), 3);
    let keyboard_up = key(&mut trial, &mut trace, "keyboard-up", false);
    assert_eq!(clicks.get(), 4);
    until_current(
        &mut trial,
        &mut trace,
        "keyboard-up",
        keyboard_up,
        false,
        &view,
    );
    let cancelled_down = key(&mut trial, &mut trace, "cancel-key-down", true);
    until_current(
        &mut trial,
        &mut trace,
        "cancel-key-down",
        cancelled_down,
        true,
        &view,
    );
    let cancelled = Instant::now();
    handle_window_focus_changed(
        &mut trial.domains,
        &mut trial.router,
        &mut trial.tree,
        WindowId::PRIMARY,
        false,
        trial.renderer.text_engine_mut(),
    )
    .unwrap();
    trace.record(&trial, "window-cancel", "event_after");
    until_current(
        &mut trial,
        &mut trace,
        "window-cancel",
        cancelled,
        false,
        &view,
    );
    key(&mut trial, &mut trace, "cancel-late-key-up", false);
    assert_eq!(clicks.get(), 4, "cancelled key release cannot click");
    frame(&mut trial, &mut trace, "compiled-cancel", &view, true);
    assert!(!trial.request().pressed);
    assert_eq!(trial.tree.get_absolute_paint_rect(button), Some(face));
    assert!(Arc::ptr_eq(
        &peer_local,
        &trial.harness.local_fragment(peer_owner).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &root_local,
        &trial
            .harness
            .local_fragment(trial.tree.node(root).unwrap().id)
            .unwrap()
    ));
    assert!(leaves(&trial.harness.local_fragment(owner).unwrap()).iter()
        .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage" && text.font_size == 13.0)));

    trial.require_image = false;
    trial
        .tree
        .node_mut(button)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Button>()
        .unwrap()
        .request_sdf_material(None)
        .unwrap();
    frame(&mut trial, &mut trace, "withdraw", &view, false);
    assert!(
        trial
            .harness
            .cache
            .sdf_scene_custody
            .references()
            .is_empty()
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while trial.harness.cache.sdf_material_staging.is_some() || !trace.completions.is_empty() {
        assert!(
            Instant::now() < deadline,
            "certified cleanup did not finish"
        );
        frame(&mut trial, &mut trace, "retire", &view, false);
        std::thread::yield_now();
    }
    assert_eq!(
        initial.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
    assert_eq!(
        released.availability(),
        Err(ImageBindingFailure::Unregistered)
    );
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/stage-v3-hover-rounding");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&trace.rows).unwrap(),
    )
    .unwrap();
    println!(
        "SDF_TRACE_COMPLETE frames={} clicks={} all_sampling_callbacks_observed=true native_present_unproven=true",
        trace.frame,
        clicks.get()
    );
}

#[test]
fn canonical_button_default_relief_comparison() {
    use crate::core::appearance::ButtonPaintCase;
    use crate::core::theme::DensityMode;
    use std::io::Write;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/button-relief-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut manifest = std::fs::File::create(directory.join("frames.tsv")).unwrap();
    writeln!(
        manifest,
        "theme\tscale\tstate\tstage\tapply\trollback\tfile"
    )
    .unwrap();
    println!(
        "Button relief actual GPU comparison: {}",
        directory.display()
    );
    let states = [
        "rest",
        "hover-apply",
        "pressed-apply",
        "pressed-rollback",
        "pressed-stage",
        "released",
        "focus-apply",
        "disabled-defaults",
    ];
    let write_ppm = |path: &std::path::Path, width: u32, height: u32, pixels: &[[u8; 4]]| {
        assert_eq!(pixels.len(), (width * height) as usize);
        let mut file = std::fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels {
            file.write_all(&pixel[..3]).unwrap();
        }
    };
    for dark in [false, true] {
        for scale in [1.0, 2.0] {
            let name = if dark { "dark" } else { "light" };
            let (mut trial, stage_clicks, apply_id) = ReferenceETrial::new(dark, scale, 84.0, true);
            trial.theme = if dark {
                Theme::dark_with_density(DensityMode::Intermediate30)
            } else {
                Theme::light_with_density(DensityMode::Intermediate30)
            };
            crate::replace_widget!(
                trial.tree,
                trial.root,
                Flex::row()
                    .with_padding(EdgeInsets::all(8.0))
                    .with_spacing(8.0)
                    .with_wrap()
                    .with_align_items(Alignment::Start)
                    .with_background(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 }))
            );
            let apply = trial.tree.find_node_by_widget_id(apply_id).unwrap();
            let apply_label = trial
                .tree
                .add_child(apply, Box::new(Label::new("Apply")), None, 0);
            let apply_clicks = Rc::new(Cell::new(0));
            let observed = apply_clicks.clone();
            crate::replace_widget!(
                trial.tree,
                apply,
                Button::new(apply_label).on_click(move || observed.set(observed.get() + 1))
            );
            let rollback =
                trial
                    .tree
                    .add_child(trial.root, Box::new(Button::new(NodeId(0))), None, 2);
            let rollback_label =
                trial
                    .tree
                    .add_child(rollback, Box::new(Label::new("Rollback")), None, 0);
            let rollback_clicks = Rc::new(Cell::new(0));
            let observed = rollback_clicks.clone();
            crate::replace_widget!(
                trial.tree,
                rollback,
                Button::new(rollback_label).on_click(move || observed.set(observed.get() + 1))
            );
            let rollback_id = trial.tree.widget_id_for_node(rollback).unwrap();
            let binding = trial.settle(false);
            trial.require_image = true;
            let geometry = [trial.button, apply, rollback]
                .map(|node| trial.tree.get_absolute_paint_rect(node).unwrap());
            assert_eq!(geometry[0].size(), Size::new(84.0, 30.0));
            for face in geometry {
                assert!(face.width > 0.0 && face.height > 0.0);
                assert!(face.x >= 0.0 && face.y >= 0.0);
                assert!(face.x + face.width <= 240.0 && face.y + face.height <= 100.0);
            }
            let center =
                |face: RectF| Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
            let stage_pos = center(geometry[0]);
            let apply_pos = center(geometry[1]);
            let rollback_pos = center(geometry[2]);
            let (width, height) = ((240.0 * scale) as u32, (100.0 * scale) as u32);
            let mut contact = vec![[0_u8; 4]; (width * height * states.len() as u32) as usize];
            for (index, state) in states.into_iter().enumerate() {
                match state {
                    "hover-apply" => trial.hover_at(apply_pos),
                    "pressed-apply" => trial.pointer_at(true, apply_pos),
                    "pressed-rollback" => {
                        trial.pointer_at(false, apply_pos);
                        assert_eq!(apply_clicks.get(), 1);
                        trial.hover_at(rollback_pos);
                        trial.pointer_at(true, rollback_pos);
                    }
                    "pressed-stage" => {
                        trial.pointer_at(false, rollback_pos);
                        assert_eq!(rollback_clicks.get(), 1);
                        trial.hover_at(stage_pos);
                        trial.pointer_at(true, stage_pos);
                    }
                    "released" => {
                        trial.pointer_at(false, stage_pos);
                        assert_eq!(stage_clicks.get(), 1);
                        trial.hover_at(Point::new(238.0, 98.0));
                    }
                    "focus-apply" => {
                        trial.router.request_focus_for_widget(
                            &mut trial.tree,
                            apply_id,
                            trial.domains.primary_state_mut(),
                        );
                    }
                    "disabled-defaults" => {
                        trial.tree.request_widget_enabled(apply, false).unwrap();
                        trial.tree.request_widget_enabled(rollback, false).unwrap();
                        let settled =
                            crate::core::widget_availability::apply_pending_widget_availability(
                                &mut trial.tree,
                                &mut trial.router,
                                &mut trial.fusion,
                                trial.domains.primary_state_mut(),
                                &mut trial.bindings,
                                trial.renderer.text_engine_mut(),
                            );
                        assert!(settled.rejected.is_empty());
                        assert_eq!(settled.changed.len(), 2);
                    }
                    _ => {}
                }
                trial.frame();
                assert_eq!(
                    [trial.button, apply, rollback]
                        .map(|node| trial.tree.get_absolute_paint_rect(node).unwrap()),
                    geometry,
                    "visual state must preserve all three canonical allocations"
                );
                assert_eq!(
                    trial.image().image.as_ref().unwrap().binding.identity(),
                    binding.identity()
                );
                assert_eq!(
                    trial.image().image.as_ref().unwrap().uv.x,
                    if state == "pressed-stage" { 0.5 } else { 0.0 }
                );
                for (node, owner, pressed_state) in [
                    (apply, apply_id, "pressed-apply"),
                    (rollback, rollback_id, "pressed-rollback"),
                ] {
                    let button = trial
                        .tree
                        .node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<Button>()
                        .unwrap();
                    assert!(
                        button.sdf_request_snapshot().is_none(),
                        "ordinary controls use their default paint route"
                    );
                    let appearance = button.resolved_appearance().unwrap();
                    let expected = if state == "disabled-defaults" {
                        ButtonPaintCase::Disabled
                    } else if state == pressed_state {
                        ButtonPaintCase::Pressed
                    } else if state == "hover-apply" && node == apply {
                        ButtonPaintCase::Hovered
                    } else {
                        ButtonPaintCase::Resting
                    };
                    assert_eq!(appearance.paint_case(), expected);
                    if state == "focus-apply" && node == apply {
                        assert!(appearance.request().input.framework().focus_cue_visible());
                    }
                    assert!(
                        !leaves(&trial.harness.local_fragment(owner).unwrap())
                            .iter()
                            .any(|node| matches!(node, SceneNode::Image(_)))
                    );
                }
                assert_eq!(
                    trial
                        .domains
                        .primary_state()
                        .pointer_capture_manager
                        .is_any_captured(),
                    state.starts_with("pressed-")
                );
                let pixels = trial.pixels(&binding);
                let filename = format!("{name}-{scale}x-{state}.ppm");
                write_ppm(&directory.join(&filename), width, height, &pixels);
                writeln!(
                    manifest,
                    "{name}\t{scale}\t{state}\t{:?}\t{:?}\t{:?}\t{filename}",
                    geometry[0], geometry[1], geometry[2]
                )
                .unwrap();
                let column = index % 4;
                let row = index / 4;
                for y in 0..height as usize {
                    let destination =
                        (row * height as usize + y) * width as usize * 4 + column * width as usize;
                    contact[destination..destination + width as usize]
                        .copy_from_slice(&pixels[y * width as usize..(y + 1) * width as usize]);
                }
            }
            write_ppm(
                &directory.join(format!("{name}-{scale}x-contact.ppm")),
                width * 4,
                height * 2,
                &contact,
            );
            assert_eq!(stage_clicks.get(), 1);
            assert_eq!(apply_clicks.get(), 1);
            assert_eq!(rollback_clicks.get(), 1);
            trial.require_image = false;
            trial
                .tree
                .node_mut(trial.button)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
                .request_sdf_material(None)
                .unwrap();
            trial.frame();
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .references()
                    .is_empty()
            );
            let deadline = Instant::now() + Duration::from_secs(15);
            while trial.harness.cache.sdf_material_staging.is_some() {
                assert!(Instant::now() < deadline);
                trial.frame();
            }
            assert_eq!(
                binding.availability(),
                Err(ImageBindingFailure::Unregistered)
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn canonical_button_sdf_v3_resize_resource_trace() {
    use crate::renderer::custom_render_pass::audit::{self, CustomRenderAuditSnapshot};
    use crate::renderer::custom_render_prepare::CustomRenderTargetMetrics;
    use crate::renderer::image_binding::{ImageBindingFailure, ImageBindingIdentity};
    use crate::renderer::sdf_material_status::{SdfMaterialKey, SdfMaterialRevision};
    use serde_json::json;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct WarmResource {
        basis: SdfMaterialRevision,
        key: SdfMaterialKey,
        identity: ImageBindingIdentity,
        metrics: CustomRenderTargetMetrics,
        preparations: (u64, u128),
        producer_work: [u64; 5],
    }

    fn producer_work(snapshot: CustomRenderAuditSnapshot) -> [u64; 5] {
        [
            snapshot.custom_render_passes,
            snapshot.custom_textures,
            snapshot.custom_command_encoders,
            snapshot.custom_submissions,
            snapshot.custom_registrations,
        ]
    }

    fn warm_resource(trial: &ReferenceETrial, binding: &ImageBinding) -> WarmResource {
        let key = trial.image().key;
        WarmResource {
            basis: trial.request().basis_revision,
            identity: binding.identity(),
            metrics: key.metrics(),
            key,
            preparations: totals(trial),
            producer_work: producer_work(audit::snapshot()),
        }
    }

    fn totals(trial: &ReferenceETrial) -> (u64, u128) {
        trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or((0, 0), |staging| staging.preparation_totals_for_test())
    }

    fn observe(
        trial: &mut ReferenceETrial,
        directory: &std::path::Path,
        sequence: usize,
        transition: &str,
        expected_face: RectF,
        resource: Option<&WarmResource>,
        pressed: bool,
        rows: &mut Vec<serde_json::Value>,
    ) -> bool {
        let before = totals(trial);
        let audit_before = audit::snapshot();
        let frame_start = Instant::now();
        trial.frame();
        let frame_cpu_ns = frame_start.elapsed().as_nanos();
        let after = totals(trial);
        let audit_after = audit::snapshot();
        let request = trial.request();
        assert_eq!(request.pressed, pressed);
        if let Some(resource) = resource {
            assert_eq!(request.basis_revision, resource.basis);
            assert_eq!(before, resource.preparations);
            assert_eq!(
                after, resource.preparations,
                "compatible resize must not prepare a material generation"
            );
            assert_eq!(producer_work(audit_before), resource.producer_work);
            assert_eq!(
                producer_work(audit_after),
                resource.producer_work,
                "compatible resize must not allocate, encode, submit or register a material"
            );
        }
        assert_eq!(trial.tree.node(trial.button).unwrap().id, trial.owner);
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        assert_eq!(face, expected_face);
        let viewport = trial.tree.get_viewport(trial.button).unwrap();
        let metrics = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap()
            .metrics_for(face, viewport)
            .unwrap();
        assert_eq!(metrics.scale_factor, trial.scale);
        let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
        let current = current
            .iter()
            .find(|input| input.key.owner() == trial.owner)
            .unwrap();
        assert_eq!(current.key.metrics(), metrics);
        if let Some(resource) = resource {
            assert_eq!(metrics.surface_id, resource.metrics.surface_id);
            assert_eq!(
                metrics.final_logical_rect,
                resource.metrics.final_logical_rect
            );
            assert_eq!(
                metrics.final_logical_viewport,
                resource.metrics.final_logical_viewport
            );
            assert_eq!(metrics.scale_factor, resource.metrics.scale_factor);
            assert_eq!(metrics.activity, resource.metrics.activity);
            if metrics.target_logical_extent != resource.metrics.target_logical_extent
                || metrics.target_physical_extent != resource.metrics.target_physical_extent
            {
                assert_ne!(
                    current.key, resource.key,
                    "window extent remains part of the complete current K"
                );
            }
        }
        let frozen = trial
            .harness
            .cache
            .sdf_paint_inputs
            .as_ref()
            .and_then(|inputs| inputs.get(&trial.owner))
            .cloned();
        let image = frozen.as_ref().and_then(|input| input.image.as_ref());
        if let Some(resource) = resource {
            assert_eq!(
                image
                    .expect("compatible resize must retain the admitted face")
                    .binding
                    .identity(),
                resource.identity,
                "compatible resize retains the exact original generation"
            );
        }
        if let Some(image) = image {
            let frozen = frozen.as_ref().unwrap();
            assert_eq!(frozen.key, current.key);
            assert_eq!(
                frozen.key.metrics(),
                metrics,
                "paint eligibility must carry the fresh complete target metrics"
            );
            assert_eq!(frozen.requested_revision, request.revision);
            assert_eq!(image.binding.availability(), Ok(()));
            assert_eq!(image.uv.x, if pressed { 0.5 } else { 0.0 });
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&frozen.key, image.binding.identity())
            );
        } else {
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .references()
                    .is_empty()
            );
        }
        let local = trial.harness.local_fragment(trial.owner).unwrap();
        let local_leaves = leaves(&local);
        assert!(
            local_leaves
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage" && text.font_size == 13.0))
        );
        let retained_generations = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or(0, |staging| staging.retained_generation_count());
        assert!(retained_generations <= 2);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| trial.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let data = compiled
            .plan()
            .surface_compiled_data(trial.surface)
            .unwrap();
        let image_batches: Vec<_> = data
            .frame_batches
            .batches
            .iter()
            .filter(|batch| batch.key.pipeline == PipelineId::Image)
            .collect();
        if let Some(image) = image {
            assert!(image_batches.iter().any(|batch| {
                batch.key.texture_id == Some(image.binding.identity().texture_id())
                    && batch.key.image_generation == Some(image.binding.identity().generation())
            }));
        } else {
            assert!(image_batches.is_empty());
        }
        let image_identity = image.map(|image| format!("{:?}", image.binding.identity()));
        let image_uv = image.map(|image| format!("{:?}", image.uv));
        let selected = trial
            .harness
            .cache
            .selected_sdf_materials
            .as_ref()
            .and_then(|selected| selected.get(&trial.owner))
            .and_then(Option::as_ref)
            .map(|publication| format!("{:?}", publication.status()));
        let visible = image.is_some();
        let gpu = trial.renderer.gpu_resources();
        let extent = metrics.target_physical_extent;
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SDF resize current target pixels"),
            size: wgpu::Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        if visible {
            let completed = compiled
                .submit_headless_for_test(&trial.renderer, trial.surface, &view)
                .unwrap();
            poll_until(&gpu.device, &completed);
        } else {
            // No SDF use exists in this compiled attempt; submit its actual remaining content.
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .references()
                    .is_empty()
            );
            let command = trial
                .renderer
                .encode_compiled_target_for_test(compiled.plan(), trial.surface, &view)
                .unwrap();
            trial.renderer.submit_compiled_target_for_test(command);
            let done = Arc::new(AtomicBool::new(false));
            let signal = done.clone();
            gpu.queue
                .on_submitted_work_done(move || signal.store(true, Ordering::Release));
            poll_until(&gpu.device, &|| done.load(Ordering::Acquire));
        }
        let pixels = read_pixels(&gpu, &output, extent.width, extent.height);
        trial.harness.cache.note_sdf_target_output(trial.surface);
        let theme_name = if trial.theme.is_dark() {
            "dark"
        } else {
            "light"
        };
        let filename = format!(
            "{theme_name}-{}x-{sequence:02}-{transition}.ppm",
            trial.scale as u32
        );
        let mut file = std::fs::File::create(directory.join(&filename)).unwrap();
        write!(file, "P6\n{} {}\n255\n", extent.width, extent.height).unwrap();
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|pixel| pixel[..3].iter().copied())
            .collect();
        file.write_all(&rgb).unwrap();
        rows.push(json!({
            "theme": theme_name,
            "scale": trial.scale,
            "sequence": sequence,
            "transition": transition,
            "owner": format!("{:?}", trial.owner),
            "surface": format!("{:?}", trial.surface),
            "face": format!("{face:?}"),
            "viewport": format!("{viewport:?}"),
            "metrics": format!("{metrics:?}"),
            "current_key": format!("{:?}", current.key),
            "basis_revision": format!("{:?}", request.basis_revision),
            "request_revision": format!("{:?}", request.revision),
            "pressed": pressed,
            "selected_status": selected,
            "image_visible": visible,
            "image_identity": image_identity,
            "image_uv": image_uv,
            "retained_generations": retained_generations,
            "preparations_before": before.0,
            "preparations_after": after.0,
            "new_preparations": after.0 - before.0,
            "preparation_cpu_ns_before": before.1.to_string(),
            "preparation_cpu_ns_after": after.1.to_string(),
            "new_preparation_cpu_ns": (after.1 - before.1).to_string(),
            "original_generation_key": resource.map(|resource| format!("{:?}", resource.key)),
            "new_material_passes": audit_after.custom_render_passes - audit_before.custom_render_passes,
            "new_material_textures": audit_after.custom_textures - audit_before.custom_textures,
            "new_material_encoders": audit_after.custom_command_encoders - audit_before.custom_command_encoders,
            "new_producer_submissions": audit_after.custom_submissions - audit_before.custom_submissions,
            "new_material_registrations": audit_after.custom_registrations - audit_before.custom_registrations,
            "frame_cpu_ns": frame_cpu_ns.to_string(),
            "file": filename
        }));
        visible
    }

    fn resize(trial: &mut ReferenceETrial, size: Size) {
        let before = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap();
        trial
            .renderer
            .resize_render_target(
                trial.surface,
                (
                    (size.width * trial.scale) as u32,
                    (size.height * trial.scale) as u32,
                ),
            )
            .unwrap();
        trial
            .compositor
            .set_surface_bounds(trial.surface, RectF::new(0.0, 0.0, size.width, size.height));
        trial.tree.mark_layout_dirty(trial.root);
        let after = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap();
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        let viewport = trial.tree.get_viewport(trial.button).unwrap();
        let before = before.metrics_for(face, viewport).unwrap();
        let after = after.metrics_for(face, viewport).unwrap();
        assert_ne!(after.target_logical_extent, before.target_logical_extent);
        assert_ne!(after.target_physical_extent, before.target_physical_extent);
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/sdf-resize-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    let mut settlements = Vec::new();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let (mut trial, clicks, _) = ReferenceETrial::new(dark, scale, 84.0, false);
        let mut sequence = 0;
        let face = RectF::new(20.0, 20.0, 84.0, 30.0);
        assert!(!observe(
            &mut trial, &directory, sequence, "cold-a", face, None, false, &mut rows,
        ));
        let start = Instant::now();
        let before_settle = trial.harness.frame_time_ms;
        let first_binding = trial.settle(false);
        let resource = warm_resource(&trial, &first_binding);
        assert_eq!(resource.preparations.0, 1);
        settlements.push(json!({
            "theme_dark": dark, "scale": scale, "phase": "initial",
            "cpu_ns": start.elapsed().as_nanos().to_string(),
            "ordinary_frames": (trial.harness.frame_time_ms - before_settle) / 16,
            "preparation_totals": format!("{:?}", totals(&trial))
        }));
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "warm-rest",
            face,
            Some(&resource),
            false,
            &mut rows
        ));
        let held_plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let mut held = Some(
            trial
                .renderer
                .compile_sdf_frame(
                    SdfFramePlan::new(held_plan, trial.harness.cache.sdf_scene_custody.clone()),
                    None,
                )
                .unwrap()
                .expect_ready("native Compile must be ready"),
        );
        for (index, size) in [
            Size::new(248.0, 108.0),
            Size::new(256.0, 116.0),
            Size::new(264.0, 124.0),
            Size::new(272.0, 132.0),
        ]
        .into_iter()
        .enumerate()
        {
            resize(&mut trial, size);
            sequence += 1;
            observe(
                &mut trial,
                &directory,
                sequence,
                &format!("resize-rest-{index}"),
                face,
                Some(&resource),
                false,
                &mut rows,
            );
            if held.is_some() {
                assert_eq!(first_binding.availability(), Ok(()));
            }
            if index == 1 {
                drop(held.take());
            }
        }
        sequence += 1;
        observe(
            &mut trial,
            &directory,
            sequence,
            "same-size-rest",
            face,
            Some(&resource),
            false,
            &mut rows,
        );
        trial.settle(false);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "settled-rest",
            face,
            Some(&resource),
            false,
            &mut rows
        ));
        let stable = totals(&trial);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "stable-rest",
            face,
            Some(&resource),
            false,
            &mut rows
        ));
        assert_eq!(totals(&trial), stable);
        trial.pointer(true);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "warm-pressed",
            face,
            Some(&resource),
            true,
            &mut rows
        ));
        assert_eq!(totals(&trial), stable);
        for (index, size) in [Size::new(280.0, 140.0), Size::new(288.0, 148.0)]
            .into_iter()
            .enumerate()
        {
            resize(&mut trial, size);
            sequence += 1;
            observe(
                &mut trial,
                &directory,
                sequence,
                &format!("resize-pressed-{index}"),
                face,
                Some(&resource),
                true,
                &mut rows,
            );
        }
        sequence += 1;
        observe(
            &mut trial,
            &directory,
            sequence,
            "same-size-pressed",
            face,
            Some(&resource),
            true,
            &mut rows,
        );
        trial.settle(true);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "settled-pressed",
            face,
            Some(&resource),
            true,
            &mut rows
        ));
        let stable = totals(&trial);
        trial.pointer(false);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "warm-released",
            face,
            Some(&resource),
            false,
            &mut rows
        ));
        assert_eq!(totals(&trial), stable);
        assert_eq!(clicks.get(), 1);
        for (index, size) in [Size::new(296.0, 156.0), Size::new(240.0, 100.0)]
            .into_iter()
            .enumerate()
        {
            resize(&mut trial, size);
            sequence += 1;
            observe(
                &mut trial,
                &directory,
                sequence,
                &format!("resize-final-{index}"),
                face,
                Some(&resource),
                false,
                &mut rows,
            );
            sequence += 1;
            observe(
                &mut trial,
                &directory,
                sequence,
                "same-size-final",
                face,
                Some(&resource),
                false,
                &mut rows,
            );
            let start = Instant::now();
            let before_settle = trial.harness.frame_time_ms;
            trial.settle(false);
            settlements.push(json!({
                "theme_dark": dark, "scale": scale, "phase": format!("final-{index}"),
                "cpu_ns": start.elapsed().as_nanos().to_string(),
                "ordinary_frames": (trial.harness.frame_time_ms - before_settle) / 16,
                "preparation_totals": format!("{:?}", totals(&trial))
            }));
            sequence += 1;
            assert!(observe(
                &mut trial,
                &directory,
                sequence,
                "settled-final",
                face,
                Some(&resource),
                false,
                &mut rows
            ));
        }
        drop(held);
        let old_plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let old_compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(old_plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let holder = trial.tree.parent(trial.button).unwrap();
        trial
            .tree
            .node_mut(holder)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Container>()
            .unwrap()
            .fixed_width = Some(100.0);
        trial.tree.mark_layout_dirty(holder);
        let changed_face = RectF::new(20.0, 20.0, 100.0, 30.0);
        let before_change = totals(&trial);
        let before_work = producer_work(audit::snapshot());
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "changed-face-admitted",
            changed_face,
            None,
            false,
            &mut rows,
        ));
        let changed_binding = trial.settle(false);
        assert_ne!(changed_binding.identity(), first_binding.identity());
        assert_eq!(totals(&trial).0, before_change.0 + 1);
        assert_eq!(
            producer_work(audit::snapshot()),
            [
                before_work[0] + 1,
                before_work[1] + 2,
                before_work[2] + 1,
                before_work[3] + 1,
                before_work[4] + 1,
            ],
            "genuine face change uses one fresh producer pair and registration"
        );
        assert_eq!(
            first_binding.availability(),
            Ok(()),
            "old compiled plan still owns its original generation"
        );
        let changed_resource = warm_resource(&trial, &changed_binding);
        sequence += 1;
        assert!(observe(
            &mut trial,
            &directory,
            sequence,
            "changed-face-settled",
            changed_face,
            Some(&changed_resource),
            false,
            &mut rows,
        ));
        drop(old_compiled);
        let deadline = Instant::now() + Duration::from_secs(15);
        while first_binding.availability().is_ok() {
            assert!(
                Instant::now() < deadline,
                "superseded face generation did not retire after terminal cancellation"
            );
            trial.frame();
        }
        assert_eq!(
            first_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(changed_binding.availability(), Ok(()));
        assert_eq!(clicks.get(), 1);
        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while trial.harness.cache.sdf_material_staging.is_some() {
            assert!(
                Instant::now() < deadline,
                "resize fixture resource retirement timed out"
            );
            trial.frame();
        }
    }
    let trace = json!({
        "scope": "Actual canonical renderer compatible window-resize regression plus genuine face-change admission; no native window or visual acceptance",
        "timing": "CPU Instant; successful preparation bracket excludes GPU execution; thread-local producer audit measures passes/textures/encoders/submissions and actual reserved registrations, not frame composition submissions",
        "rows": rows,
        "settlements": settlements
    });
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&trace).unwrap(),
    )
    .unwrap();
    println!("SDF resize actual GPU trace: {}", directory.display());
}

#[test]
fn canonical_button_sdf_cold_request_preserves_default_until_ready() {
    fn assert_same_ordinary_scene(expected: &[SceneNode], actual: &[SceneNode]) {
        let expected = leaves(expected);
        let actual = leaves(actual);
        assert_eq!(expected.len(), actual.len());
        for (expected, actual) in expected.iter().zip(&actual) {
            match (expected, actual) {
                (SceneNode::Rect(expected), SceneNode::Rect(actual)) => assert_eq!(
                    (
                        expected.rect,
                        expected.color,
                        expected.z_index,
                        expected.key
                    ),
                    (actual.rect, actual.color, actual.z_index, actual.key)
                ),
                (SceneNode::RoundedRect(expected), SceneNode::RoundedRect(actual)) => assert_eq!(
                    (
                        expected.rect,
                        expected.color,
                        expected.radius,
                        expected.z_index,
                        expected.key
                    ),
                    (
                        actual.rect,
                        actual.color,
                        actual.radius,
                        actual.z_index,
                        actual.key
                    )
                ),
                (SceneNode::BorderRect(expected), SceneNode::BorderRect(actual)) => assert_eq!(
                    (
                        expected.rect,
                        expected.fill_color,
                        expected.border_color,
                        expected.border_width,
                        expected.radius,
                        expected.z_index,
                        expected.key
                    ),
                    (
                        actual.rect,
                        actual.fill_color,
                        actual.border_color,
                        actual.border_width,
                        actual.radius,
                        actual.z_index,
                        actual.key
                    )
                ),
                (SceneNode::GradientRect(expected), SceneNode::GradientRect(actual)) => assert_eq!(
                    (
                        expected.rect,
                        expected.radius,
                        &expected.gradient,
                        expected.z_index,
                        expected.key
                    ),
                    (
                        actual.rect,
                        actual.radius,
                        &actual.gradient,
                        actual.z_index,
                        actual.key
                    )
                ),
                (SceneNode::Text(expected), SceneNode::Text(actual)) => {
                    assert_eq!(
                        (
                            &expected.binding,
                            expected.position,
                            &expected.text,
                            expected.font_size,
                            expected.color,
                            expected.z_index,
                            expected.key
                        ),
                        (
                            &actual.binding,
                            actual.position,
                            &actual.text,
                            actual.font_size,
                            actual.color,
                            actual.z_index,
                            actual.key
                        )
                    );
                    assert_eq!(expected.backgrounds.len(), actual.backgrounds.len());
                    for (expected, actual) in expected.backgrounds.iter().zip(&actual.backgrounds) {
                        assert_eq!(
                            (
                                expected.x_offset,
                                expected.y_offset,
                                expected.width,
                                expected.height,
                                expected.color
                            ),
                            (
                                actual.x_offset,
                                actual.y_offset,
                                actual.width,
                                actual.height,
                                actual.color
                            )
                        );
                    }
                }
                _ => panic!("cold request changed an ordinary Button scene primitive"),
            }
        }
    }

    for (dark, scale, paired) in [
        (false, 1.0, false),
        (false, 2.0, false),
        (true, 1.0, false),
        (true, 2.0, false),
        (false, 1.0, true),
        (false, 2.0, true),
        (true, 1.0, true),
        (true, 2.0, true),
    ] {
        let (mut trial, clicks, peer_id) = ReferenceETrial::new(dark, scale, 84.0, false);
        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        trial.frame();
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        let ordinary = trial.harness.local_fragment(trial.owner).unwrap();
        let peer = trial.harness.local_fragment(peer_id).unwrap();
        assert!(leaves(&ordinary).iter().any(|node| matches!(
            node,
            SceneNode::GradientRect(_) | SceneNode::RoundedRect(_) | SceneNode::Rect(_)
        )));
        assert!(
            leaves(&ordinary)
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage"))
        );
        assert!(trial.harness.cache.sdf_material_staging.is_none());

        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(Some(match (dark, paired) {
                (false, false) => ButtonSdfRestingProfile::Light,
                (true, false) => ButtonSdfRestingProfile::Dark,
                (false, true) => ButtonSdfRestingProfile::LightV3,
                (true, true) => ButtonSdfRestingProfile::DarkV3,
            }))
            .unwrap();
        let ordinary_scene = trial.scenes.remove(&trial.surface).unwrap();
        #[cfg(debug_assertions)]
        let layout_epoch = current_layout_epoch();
        trial.frame();
        if !paired {
            assert!(
                !trial.scenes.contains_key(&trial.surface),
                "an unchanged ordinary projection must complete Frame A without a new scene"
            );
            assert!(Arc::ptr_eq(
                &ordinary,
                &trial.harness.local_fragment(trial.owner).unwrap()
            ));
        }
        trial.scenes.entry(trial.surface).or_insert(ordinary_scene);
        #[cfg(debug_assertions)]
        assert_eq!(current_layout_epoch(), layout_epoch);
        assert!(
            trial
                .harness
                .cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&trial.owner))
                .is_none(),
            "Frame A cannot consume its queued material evidence"
        );
        // Compare the actual ordinary projection, including current theme values.
        assert_same_ordinary_scene(
            &ordinary,
            &trial.harness.local_fragment(trial.owner).unwrap(),
        );
        assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(face));
        assert!(Arc::ptr_eq(
            &peer,
            &trial.harness.local_fragment(peer_id).unwrap()
        ));
        assert_eq!(clicks.get(), 0);

        let deadline = Instant::now() + Duration::from_secs(15);
        let binding = loop {
            assert!(
                Instant::now() < deadline,
                "cold material did not become Ready"
            );
            let before_commit = trial.harness.local_fragment(trial.owner).unwrap();
            #[cfg(debug_assertions)]
            let layout_epoch = current_layout_epoch();
            trial.frame();
            #[cfg(debug_assertions)]
            assert_eq!(
                current_layout_epoch(),
                layout_epoch,
                "material commitment must invalidate Paint without Layout"
            );
            assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(face));
            assert!(Arc::ptr_eq(
                &peer,
                &trial.harness.local_fragment(peer_id).unwrap()
            ));
            let committed = trial
                .harness
                .cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&trial.owner));
            if let Some(input) = committed {
                let image = input
                    .image
                    .as_ref()
                    .expect("real cold producer must become Ready");
                let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
                let current = current
                    .iter()
                    .find(|input| input.key.owner() == trial.owner)
                    .unwrap();
                assert_eq!(input.key, current.key);
                assert_eq!(input.requested_revision, trial.request().revision);
                assert_eq!(image.binding.availability(), Ok(()));
                assert_eq!(image.uv.x, 0.0);
                assert!(
                    trial
                        .harness
                        .cache
                        .sdf_scene_custody
                        .has_reference(&input.key, image.binding.identity())
                );
                let committed_local = trial.harness.local_fragment(trial.owner).unwrap();
                assert!(
                    !Arc::ptr_eq(&before_commit, &committed_local),
                    "the first resource commitment must repaint its owner"
                );
                let committed_leaves = leaves(&committed_local);
                assert_eq!(
                    committed_leaves
                        .iter()
                        .filter(|node| matches!(
                            node,
                            SceneNode::Image(image_node)
                                if image_node.texture_source
                                    == ImageTextureSource::Admitted(image.binding.identity())
                        ))
                        .count(),
                    1
                );
                assert!(
                    committed_leaves
                        .iter()
                        .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage"))
                );
                break image.binding.clone();
            }
            assert_same_ordinary_scene(
                &ordinary,
                &trial.harness.local_fragment(trial.owner).unwrap(),
            );
            std::thread::yield_now();
        };

        trial.require_image = true;
        let ready_local = trial.harness.local_fragment(trial.owner).unwrap();
        let ready_request = trial.request();
        let ready_key = trial.image().key;
        for _ in 0..3 {
            #[cfg(debug_assertions)]
            let layout_epoch = current_layout_epoch();
            trial.frame();
            #[cfg(debug_assertions)]
            assert_eq!(current_layout_epoch(), layout_epoch);
            assert_eq!(trial.request(), ready_request);
            assert_eq!(trial.image().key, ready_key);
            assert_eq!(
                trial.image().image.as_ref().unwrap().binding.identity(),
                binding.identity()
            );
            assert!(Arc::ptr_eq(
                &ready_local,
                &trial.harness.local_fragment(trial.owner).unwrap()
            ));
        }
        for (down, count) in [(true, 0), (false, 1)] {
            trial.pointer(down);
            #[cfg(debug_assertions)]
            let layout_epoch = current_layout_epoch();
            trial.frame();
            #[cfg(debug_assertions)]
            assert_eq!(current_layout_epoch(), layout_epoch);
            let input = trial.image();
            let image = input.image.unwrap();
            assert_eq!(input.key, ready_key);
            assert_eq!(input.requested_revision, trial.request().revision);
            assert_eq!(image.binding.identity(), binding.identity());
            assert_eq!(image.uv.x, if paired && down { 0.5 } else { 0.0 });
            assert_eq!(trial.request().pressed, paired && down);
            assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(face));
            assert!(Arc::ptr_eq(
                &peer,
                &trial.harness.local_fragment(peer_id).unwrap()
            ));
            assert_eq!(clicks.get(), count);
        }

        trial.require_image = false;
        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while trial.harness.cache.sdf_material_staging.is_some() {
            assert!(
                Instant::now() < deadline,
                "cold fixture retirement timed out"
            );
            trial.frame();
        }
    }
}

#[test]
fn canonical_button_sdf_scrollport_resize_trace() {
    use crate::core::layout::FlexParams;
    use crate::renderer::custom_render_pass::audit;
    use crate::widgets::groupbox::GroupBox;
    use serde_json::json;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn make_trial(dark: bool, scale: f32) -> (ReferenceETrial, NodeId, Rc<Cell<usize>>) {
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real Button GPU required");
        let bounds = RectF::new(0.0, 0.0, 508.0, 300.0);
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        renderer
            .register_headless_render_target(
                surface,
                (
                    (bounds.width * scale) as u32,
                    (bounds.height * scale) as u32,
                ),
                scale,
            )
            .unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::column()
                    .with_align_items(Alignment::Stretch)
                    .with_background(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 })),
            ),
            None,
            0,
        );
        let scroll = tree.add_child(root, Box::new(ScrollArea::new(NodeId(0))), None, 0);
        tree.set_flex(scroll, FlexParams::flexible(1.0));
        let content = tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_padding(EdgeInsets::all(10.0)),
            ),
            None,
            0,
        );
        crate::replace_widget!(tree, scroll, ScrollArea::new(content));
        let columns = tree.add_child(
            content,
            Box::new(
                Flex::row()
                    .with_spacing(10.0)
                    .with_align_items(Alignment::Start),
            ),
            None,
            0,
        );
        let left = tree.add_child(
            columns,
            Box::new(
                Flex::column()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        let right = tree.add_child(
            columns,
            Box::new(Flex::column().with_align_items(Alignment::Stretch)),
            None,
            1,
        );
        for column in [left, right] {
            tree.set_flex(
                column,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 0.0,
                },
            );
        }
        tree.add_child(right, Box::new(Label::new("Peer column")), None, 0);
        // A real wrapping predecessor translates the later control during reflow.
        let preceding = tree.add_child(
            left,
            Box::new(Flex::row().with_spacing(6.0).with_wrap()),
            None,
            0,
        );
        for index in 0..3 {
            tree.add_child(
                preceding,
                Box::new(Container::empty().with_width(64.0).with_height(30.0)),
                None,
                index,
            );
        }
        let group = tree.add_child(left, Box::new(GroupBox::new(NodeId(0), "Actions")), None, 1);
        let group_content =
            tree.add_child(group, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        crate::replace_widget!(
            tree,
            group,
            GroupBox::new(group_content, "Actions")
                .with_padding(6.0)
                .with_label_size(13.0)
        );
        let row = tree.add_child(
            group_content,
            Box::new(Flex::row().with_spacing(6.0).with_wrap()),
            None,
            0,
        );
        let holder = tree.add_child(
            row,
            Box::new(Container::empty().with_width(84.0).with_height(30.0)),
            None,
            0,
        );
        let button = tree.add_child(holder, Box::new(Button::new(NodeId(0))), None, 0);
        let label = tree.add_child(button, Box::new(Label::new("Run")), None, 0);
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let mut control = Button::new(label).on_click(move || count.set(count.get() + 1));
        control
            .request_sdf_material(Some(if dark {
                ButtonSdfRestingProfile::DarkV3
            } else {
                ButtonSdfRestingProfile::LightV3
            }))
            .unwrap();
        crate::replace_widget!(tree, button, control);
        crate::replace_widget!(
            tree,
            holder,
            Container::new(button, 0.0)
                .with_width(84.0)
                .with_height(30.0)
        );
        let owner = tree.node(button).unwrap().id;
        (
            ReferenceETrial {
                renderer,
                compositor,
                surface,
                tree,
                root,
                button,
                owner,
                domains: InteractionDomains::new(),
                router: EventRouter::new(),
                fusion: InputFusion::new(),
                bindings: BindingRegistry::new(),
                harness: RetainedSceneTestHarness::new(),
                scenes: HashMap::new(),
                theme: if dark { Theme::dark() } else { Theme::light() },
                scale,
                require_image: false,
                last_material_pending: false,
            },
            content,
            clicks,
        )
    }

    fn work(trial: &ReferenceETrial) -> ((u64, u128), [u64; 5]) {
        let preparation = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or((0, 0), |staging| staging.preparation_totals_for_test());
        let audit = audit::snapshot();
        (
            preparation,
            [
                audit.custom_render_passes,
                audit.custom_textures,
                audit.custom_command_encoders,
                audit.custom_submissions,
                audit.custom_registrations,
            ],
        )
    }

    fn observe(
        trial: &mut ReferenceETrial,
        directory: &std::path::Path,
        phase: &str,
        index: usize,
        rows: &mut Vec<serde_json::Value>,
    ) -> (bool, Vec<[u8; 4]>) {
        let before = work(trial);
        let start = Instant::now();
        trial.frame();
        let frame_cpu_ns = start.elapsed().as_nanos();
        let after = work(trial);
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        assert_eq!(face.size(), Size::new(84.0, 30.0));
        let viewport = trial.tree.get_viewport(trial.button).unwrap();
        let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
        let current = current
            .iter()
            .find(|input| input.key.owner() == trial.owner)
            .unwrap();
        let expected_metrics = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap()
            .metrics_for(face, viewport)
            .unwrap();
        assert_eq!(current.key.metrics(), expected_metrics);
        let frozen = trial
            .harness
            .cache
            .sdf_paint_inputs
            .as_ref()
            .and_then(|inputs| inputs.get(&trial.owner))
            .cloned();
        let image = frozen.as_ref().and_then(|input| input.image.as_ref());
        if let Some(image) = image {
            let frozen = frozen.as_ref().unwrap();
            assert_eq!(
                frozen.key, current.key,
                "never paint an obsolete target key"
            );
            assert_eq!(frozen.key.metrics().final_logical_rect, face);
            assert_eq!(frozen.key.metrics().final_logical_viewport, viewport);
            assert_eq!(frozen.requested_revision, trial.request().revision);
            assert_eq!(image.uv.x, if trial.request().pressed { 0.5 } else { 0.0 });
            assert_eq!(image.binding.availability(), Ok(()));
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&frozen.key, image.binding.identity())
            );
        }
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| trial.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let data = compiled
            .plan()
            .surface_compiled_data(trial.surface)
            .unwrap();
        let batches: Vec<_> = data
            .frame_batches
            .batches
            .iter()
            .filter(|batch| batch.key.pipeline == PipelineId::Image)
            .collect();
        if let Some(image) = image {
            assert!(batches.iter().any(|batch| {
                batch.key.texture_id == Some(image.binding.identity().texture_id())
                    && batch.key.image_generation == Some(image.binding.identity().generation())
            }));
        } else {
            assert!(
                batches.is_empty(),
                "no stale SDF batch when current evidence is absent"
            );
        }
        let gpu = trial.renderer.gpu_resources();
        let extent = current.key.metrics().target_physical_extent;
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Button scrollport resize actual output"),
            size: wgpu::Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        if image.is_some() {
            let done = compiled
                .submit_headless_for_test(&trial.renderer, trial.surface, &view)
                .unwrap();
            poll_until(&gpu.device, &done);
        } else {
            let command = trial
                .renderer
                .encode_compiled_target_for_test(compiled.plan(), trial.surface, &view)
                .unwrap();
            trial.renderer.submit_compiled_target_for_test(command);
            let done = Arc::new(AtomicBool::new(false));
            let signal = done.clone();
            gpu.queue
                .on_submitted_work_done(move || signal.store(true, Ordering::Release));
            poll_until(&gpu.device, &|| done.load(Ordering::Acquire));
        }
        let pixels = read_pixels(&gpu, &output, extent.width, extent.height);
        trial.harness.cache.note_sdf_target_output(trial.surface);
        let file = format!(
            "{}-{}x-{:03}-{phase}.ppm",
            if trial.theme.is_dark() {
                "dark"
            } else {
                "light"
            },
            trial.scale as u32,
            rows.len(),
        );
        let mut proof = std::fs::File::create(directory.join(&file)).unwrap();
        write!(proof, "P6\n{} {}\n255\n", extent.width, extent.height).unwrap();
        for pixel in &pixels {
            proof.write_all(&pixel[..3]).unwrap();
        }
        rows.push(json!({
            "theme_dark": trial.theme.is_dark(),
            "scale": trial.scale,
            "phase": phase,
            "path": if phase.starts_with("fresh-") {
                "fresh-production-reference"
            } else {
                "retained-generation"
            },
            "frame_within_phase": index,
            "owner": format!("{:?}", trial.owner),
            "face": [face.x, face.y, face.width, face.height],
            "viewport": [viewport.x, viewport.y, viewport.width, viewport.height],
            "current_key": format!("{:?}", current.key),
            "metrics": format!("{:?}", current.key.metrics()),
            "image_visible": image.is_some(),
            "image_identity": image.map(|image| format!("{:?}", image.binding.identity())),
            "destination": image.map(|image| format!("{:?}", image.coverage)),
            "uv": image.map(|image| format!("{:?}", image.uv)),
            "new_preparations": after.0.0 - before.0.0,
            "new_preparation_cpu_ns": (after.0.1 - before.0.1).to_string(),
            "preparations_total": after.0.0,
            "producer_work_total": after.1,
            "new_passes": after.1[0] - before.1[0],
            "new_textures": after.1[1] - before.1[1],
            "new_encoders": after.1[2] - before.1[2],
            "new_submissions": after.1[3] - before.1[3],
            "new_registrations": after.1[4] - before.1[4],
            "frame_cpu_ns": frame_cpu_ns.to_string(),
            "file": file,
        }));
        (image.is_some(), pixels)
    }

    fn retire(trial: &mut ReferenceETrial) {
        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while trial.harness.cache.sdf_material_staging.is_some() {
            assert!(
                Instant::now() < deadline,
                "scrollport fixture retirement timed out"
            );
            trial.frame();
        }
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/sdf-scrollport-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let (mut trial, content, clicks) = make_trial(dark, scale);
        let mut previous_geometry: Option<(RectF, RectF)> = None;
        let mut original_identity = None;
        let mut original_key = None;
        for (phase, width, height, pressed) in [
            ("initial", 508.0, 300.0, false),
            ("viewport-grow-a", 516.0, 308.0, false),
            ("viewport-grow-b", 524.0, 316.0, false),
            ("return-a", 508.0, 300.0, false),
            ("reflow-translation", 400.0, 300.0, false),
            ("fractional-translation", 400.0, 300.0, false),
            ("warm-press", 400.0, 300.0, true),
            ("viewport-pressed", 408.0, 308.0, true),
            ("release", 408.0, 308.0, false),
            ("partial-clip", 408.0, 122.0, false),
        ] {
            if phase != "initial" {
                trial
                    .renderer
                    .resize_render_target(
                        trial.surface,
                        ((width * scale) as u32, (height * scale) as u32),
                    )
                    .unwrap();
                trial
                    .compositor
                    .set_surface_bounds(trial.surface, RectF::new(0.0, 0.0, width, height));
                if phase == "fractional-translation" {
                    crate::replace_widget!(
                        trial.tree,
                        content,
                        Flex::column()
                            .with_spacing(6.0)
                            .with_padding(EdgeInsets::all(10.25))
                    );
                }
                if pressed != trial.request().pressed {
                    let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
                    trial.pointer_at(
                        pressed,
                        Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5),
                    );
                }
                trial.tree.mark_layout_dirty(trial.root);
            }
            let phase_work = work(&trial);
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut frame = 0;
            let pixels = loop {
                assert!(
                    Instant::now() < deadline,
                    "scrollport material did not settle"
                );
                let (visible, pixels) = observe(&mut trial, &directory, phase, frame, &mut rows);
                frame += 1;
                if phase != "initial" {
                    assert!(
                        visible,
                        "compatible current placement must retain its face in the first frame"
                    );
                    assert_eq!(
                        work(&trial),
                        phase_work,
                        "compatible current placement must perform no producer work"
                    );
                }
                if visible {
                    break pixels;
                }
            };
            let current_input = trial.image();
            let identity = current_input.image.as_ref().unwrap().binding.identity();
            if let Some(original) = original_identity.as_ref() {
                assert_eq!(
                    &identity, original,
                    "compatible placement must retain exact G"
                );
            } else {
                original_identity = Some(identity);
                original_key = Some(current_input.key.clone());
            }
            if phase == "return-a" {
                assert_eq!(Some(&current_input.key), original_key.as_ref());
            }
            let stable_work = work(&trial);
            let (visible, settled_pixels) =
                observe(&mut trial, &directory, phase, frame, &mut rows);
            assert!(visible, "settled successor cannot lose the material face");
            assert_eq!(work(&trial), stable_work);
            assert_eq!(
                &trial.image().image.as_ref().unwrap().binding.identity(),
                original_identity.as_ref().unwrap()
            );
            assert_eq!(
                settled_pixels, pixels,
                "unchanged successor must render identically"
            );
            let request = trial.request();
            assert_eq!(request.pressed, pressed);
            let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
            let viewport = trial.tree.get_viewport(trial.button).unwrap();
            assert_ne!(
                viewport, face,
                "fixture must exercise a real inherited scroll viewport"
            );
            if let Some((previous_face, previous_viewport)) = previous_geometry {
                match phase {
                    "viewport-grow-a" | "viewport-grow-b" | "return-a" | "viewport-pressed" => {
                        assert_eq!(
                            face, previous_face,
                            "viewport-only phase cannot translate F"
                        );
                        assert_ne!(viewport, previous_viewport);
                    }
                    "reflow-translation" => {
                        assert_ne!(
                            (face.x, face.y),
                            (previous_face.x, previous_face.y),
                            "actual wrapping must translate F"
                        );
                        assert_eq!(face.size(), previous_face.size());
                    }
                    "fractional-translation" => {
                        assert_eq!(face.x, previous_face.x + 0.25);
                        assert_eq!(face.y, previous_face.y + 0.25);
                        assert_eq!(viewport, previous_viewport);
                    }
                    "warm-press" | "release" => {
                        assert_eq!((face, viewport), (previous_face, previous_viewport));
                        assert_eq!(
                            work(&trial),
                            phase_work,
                            "warm endpoints cannot produce another texture"
                        );
                    }
                    "partial-clip" => {
                        let visible = face
                            .intersection(&viewport)
                            .expect("partially visible face");
                        assert!(visible.height > 0.0 && visible.height < face.height);
                        assert_eq!(face, previous_face);
                    }
                    _ => unreachable!(),
                }
            }
            previous_geometry = Some((face, viewport));

            if matches!(
                phase,
                "reflow-translation" | "fractional-translation" | "partial-clip"
            ) {
                // Produce independently at the SAME current geometry and clip.
                // This is a live equivalence check, never a fixed aesthetic golden.
                let (mut reference, reference_content, reference_clicks) = make_trial(dark, scale);
                reference
                    .renderer
                    .resize_render_target(
                        reference.surface,
                        ((width * scale) as u32, (height * scale) as u32),
                    )
                    .unwrap();
                reference
                    .compositor
                    .set_surface_bounds(reference.surface, RectF::new(0.0, 0.0, width, height));
                if phase != "reflow-translation" {
                    crate::replace_widget!(
                        reference.tree,
                        reference_content,
                        Flex::column()
                            .with_spacing(6.0)
                            .with_padding(EdgeInsets::all(10.25))
                    );
                }
                reference.tree.mark_layout_dirty(reference.root);
                let reference_phase = format!("fresh-{phase}");
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut reference_frame = 0;
                let mut reference_pixels = loop {
                    assert!(Instant::now() < deadline, "fresh material did not settle");
                    let (visible, pixels) = observe(
                        &mut reference,
                        &directory,
                        &reference_phase,
                        reference_frame,
                        &mut rows,
                    );
                    reference_frame += 1;
                    if visible {
                        break pixels;
                    }
                };
                assert_eq!(
                    reference.tree.get_absolute_paint_rect(reference.button),
                    Some(face)
                );
                assert_eq!(
                    reference.tree.get_viewport(reference.button),
                    Some(viewport)
                );
                assert_ne!(
                    reference.image().image.as_ref().unwrap().binding.identity(),
                    identity,
                    "reference must be a separately admitted production"
                );
                if phase == "partial-clip" {
                    // Match the original Button's real released-pointer focus facts.
                    let point = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
                    reference.pointer_at(true, point);
                    reference.pointer_at(false, point);
                    let before_focus = work(&reference);
                    let (visible, pixels) = observe(
                        &mut reference,
                        &directory,
                        &reference_phase,
                        reference_frame,
                        &mut rows,
                    );
                    assert!(visible);
                    assert_eq!(work(&reference), before_focus);
                    assert_eq!(reference_clicks.get(), 1);
                    reference_pixels = pixels;
                }
                assert_eq!(
                    pixels, reference_pixels,
                    "reused content must match fresh production at identical current placement/clip"
                );
                retire(&mut reference);
            }
        }
        assert_eq!(
            clicks.get(),
            1,
            "one original release callback after resize and held press"
        );
        retire(&mut trial);
    }
    let trace = json!({
        "scope": "Actual renderer-backed Button current-placement continuity in ScrollArea, two Flex columns, wrapping predecessor, GroupBox/row/fixed host and child caption. Retained-generation rows require same G and zero producer work after cold admission. Fresh-production-reference rows intentionally create independent material at the identical current placement and clip for dynamic GPU equivalence. Not native scheduling, recorded executable provenance or JC visual acceptance.",
        "timing": "CPU Instant around ordinary frame; producer preparation CPU bracket excludes GPU execution. Producer counters exclude composition submissions.",
        "rows": rows,
    });
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&trace).unwrap(),
    )
    .unwrap();
    println!(
        "Button ScrollArea resize GPU trace: {}",
        directory.display()
    );
}

#[test]
fn canonical_button_sdf_natural_tooltip_row_resize() {
    use crate::renderer::custom_render_pass::audit;
    use crate::widgets::tooltip_host::TooltipHost;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct RowTrial {
        trial: ReferenceETrial,
        hosts: [NodeId; 3],
        buttons: [NodeId; 3],
        clicks: [Rc<Cell<usize>>; 3],
    }

    fn make_row(dark: bool, scale: f32, fixed_host: bool) -> RowTrial {
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real Button GPU required");
        let bounds = RectF::new(0.0, 0.0, 360.0, 180.0);
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        renderer
            .register_headless_render_target(
                surface,
                (
                    (bounds.width * scale) as u32,
                    (bounds.height * scale) as u32,
                ),
                scale,
            )
            .unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(
                Flex::row()
                    .with_spacing(6.0)
                    .with_wrap()
                    .with_padding(EdgeInsets::all(10.0))
                    .with_background(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 })),
            ),
            None,
            0,
        );
        tree.set_overflow(root, OverflowMode::Clip);
        let fixed = fixed_host.then(|| tree.add_child(root, Box::new(Container::empty()), None, 0));
        let mut hosts = Vec::new();
        let mut buttons = Vec::new();
        let clicks: [Rc<Cell<usize>>; 3] = std::array::from_fn(|_| Rc::new(Cell::new(0)));
        for (index, text) in ["Stage", "Apply", "Rollback"].into_iter().enumerate() {
            let parent = if index == 0 {
                fixed.unwrap_or(root)
            } else {
                root
            };
            let host = tree.add_child(parent, Box::new(TooltipHost::new(text)), None, index as i16);
            let button = tree.add_child(host, Box::new(Button::new(NodeId(0))), None, 0);
            let label = tree.add_child(button, Box::new(Label::new(text)), None, 0);
            let count = clicks[index].clone();
            let mut control = Button::new(label).on_click(move || count.set(count.get() + 1));
            if index == 0 {
                control
                    .request_sdf_material(Some(if dark {
                        ButtonSdfRestingProfile::DarkV3
                    } else {
                        ButtonSdfRestingProfile::LightV3
                    }))
                    .unwrap();
            }
            crate::replace_widget!(tree, button, control);
            hosts.push(host);
            buttons.push(button);
        }
        if let Some(fixed) = fixed {
            crate::replace_widget!(
                tree,
                fixed,
                Container::new(hosts[0], 0.0)
                    .with_width(84.0)
                    .with_height(30.0)
            );
        }
        let button = buttons[0];
        let owner = tree.node(button).unwrap().id;
        RowTrial {
            trial: ReferenceETrial {
                renderer,
                compositor,
                surface,
                tree,
                root,
                button,
                owner,
                domains: InteractionDomains::new(),
                router: EventRouter::new(),
                fusion: InputFusion::new(),
                bindings: BindingRegistry::new(),
                harness: RetainedSceneTestHarness::new(),
                scenes: HashMap::new(),
                theme: if dark { Theme::dark() } else { Theme::light() },
                scale,
                require_image: false,
                last_material_pending: false,
            },
            hosts: hosts.try_into().unwrap(),
            buttons: buttons.try_into().unwrap(),
            clicks,
        }
    }

    fn work(trial: &ReferenceETrial) -> (u64, [u64; 5]) {
        let prepared = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or(0, |staging| staging.preparation_totals_for_test().0);
        let counts = audit::snapshot();
        (
            prepared,
            [
                counts.custom_render_passes,
                counts.custom_textures,
                counts.custom_command_encoders,
                counts.custom_submissions,
                counts.custom_registrations,
            ],
        )
    }

    fn output(trial: &mut ReferenceETrial, binding: Option<&ImageBinding>) -> Vec<[u8; 4]> {
        assert!(
            !trial.last_material_pending,
            "Pending attempt has no current output"
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| trial.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let bounds = trial
            .compositor
            .snapshot_multi()
            .get(trial.surface)
            .unwrap()
            .bounds;
        let data = compiled
            .plan()
            .surface_compiled_data(trial.surface)
            .unwrap();
        let image_batches: Vec<_> = data
            .frame_batches
            .batches
            .iter()
            .filter(|batch| batch.key.pipeline == PipelineId::Image)
            .collect();
        let target = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap();
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        let viewport = trial.tree.get_viewport(trial.button).unwrap();
        let metrics = target.metrics_for(face, viewport).unwrap();
        let frozen = trial
            .harness
            .cache
            .sdf_paint_inputs
            .as_ref()
            .and_then(|inputs| inputs.get(&trial.owner));
        if let Some(binding) = binding {
            let image = image_batches
                .iter()
                .find(|batch| {
                    batch.key.texture_id == Some(binding.identity().texture_id())
                        && batch.key.image_generation == Some(binding.identity().generation())
                })
                .expect("actual current SDF material Image must reach Compile");
            assert_eq!(image.clip_state.rect, bounds);
            let current = frozen.unwrap();
            assert_eq!(current.key.metrics(), metrics);
            assert_eq!(
                current.image.as_ref().unwrap().binding.identity(),
                binding.identity()
            );
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&current.key, binding.identity())
            );
        } else {
            assert!(frozen.and_then(|input| input.image.as_ref()).is_none());
            assert!(
                image_batches.is_empty(),
                "no obsolete Image in a pending attempt"
            );
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .references()
                    .is_empty()
            );
        }
        let gpu = trial.renderer.gpu_resources();
        let extent = metrics.target_physical_extent;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("natural TooltipHost Button row actual output"),
            size: wgpu::Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        if binding.is_some() {
            let done = compiled
                .submit_headless_for_test(&trial.renderer, trial.surface, &view)
                .unwrap();
            poll_until(&gpu.device, &done);
        } else {
            // Render the actual remaining scene, including ordinary cold chrome or a missing face.
            let command = trial
                .renderer
                .encode_compiled_target_for_test(compiled.plan(), trial.surface, &view)
                .unwrap();
            trial.renderer.submit_compiled_target_for_test(command);
            let done = Arc::new(AtomicBool::new(false));
            let signal = done.clone();
            gpu.queue
                .on_submitted_work_done(move || signal.store(true, Ordering::Release));
            poll_until(&gpu.device, &|| done.load(Ordering::Acquire));
        }
        let pixels = read_pixels(&gpu, &texture, extent.width, extent.height);
        trial.harness.cache.note_sdf_target_output(trial.surface);
        pixels
    }

    fn save(
        trial: &mut ReferenceETrial,
        binding: &ImageBinding,
        directory: &std::path::Path,
        filename: &str,
    ) {
        let pixels = output(trial, Some(binding));
        let target = trial.image().key.metrics().target_physical_extent;
        let mut file = std::fs::File::create(directory.join(filename)).unwrap();
        write!(file, "P6\n{} {}\n255\n", target.width, target.height).unwrap();
        for pixel in &pixels {
            file.write_all(&pixel[..3]).unwrap();
        }
    }

    fn rapid_attempt(
        row: &mut RowTrial,
        directory: &std::path::Path,
        phase: &str,
        frame_within_phase: usize,
        rows: &mut Vec<serde_json::Value>,
    ) -> bool {
        let before = work(&row.trial);
        let staging_before = row.trial.harness.cache.sdf_material_staging.is_some();
        let cpu_before = row
            .trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map(|staging| staging.preparation_totals_for_test().1);
        let started = Instant::now();
        row.trial.frame();
        let frame_cpu_ns = started.elapsed().as_nanos();
        let after = work(&row.trial);
        let staging_after = row.trial.harness.cache.sdf_material_staging.is_some();
        let cpu_after = row
            .trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map(|staging| staging.preparation_totals_for_test().1);
        let face = row
            .trial
            .tree
            .get_absolute_paint_rect(row.trial.button)
            .unwrap();
        let viewport = row.trial.tree.get_viewport(row.trial.button).unwrap();
        let target = row
            .trial
            .renderer
            .custom_render_target_snapshot(row.trial.surface)
            .unwrap();
        let metrics = target.metrics_for(face, viewport).unwrap();
        let current = row
            .trial
            .harness
            .cache
            .current_sdf_frame_inputs(&row.trial.tree);
        let current = current
            .iter()
            .find(|input| input.key.owner() == row.trial.owner);
        if let Some(current) = current {
            assert_eq!(
                current.key.metrics(),
                metrics,
                "current K uses the actual completed allocation"
            );
        }
        let request = row
            .trial
            .tree
            .node(row.trial.button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Button>()
            .unwrap()
            .sdf_request_snapshot();
        let frozen = row
            .trial
            .harness
            .cache
            .sdf_paint_inputs
            .as_ref()
            .and_then(|inputs| inputs.get(&row.trial.owner))
            .cloned()
            .filter(|_| !row.trial.last_material_pending);
        let image = frozen
            .as_ref()
            .and_then(|input| input.image.as_ref())
            .cloned();
        if let Some(image) = &image {
            let frozen = frozen.as_ref().unwrap();
            let request = request.as_ref().unwrap();
            assert_eq!(
                frozen.key,
                current.unwrap().key,
                "never compose an obsolete K"
            );
            assert_eq!(frozen.key.metrics(), metrics);
            assert_eq!(frozen.requested_revision, request.revision);
            assert_eq!(image.binding.availability(), Ok(()));
            assert_eq!(image.uv.x, if request.pressed { 0.5 } else { 0.0 });
            assert!(
                row.trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&frozen.key, image.binding.identity())
            );
        }
        let retained = row
            .trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or(0, |staging| staging.retained_generation_count());
        assert!(
            retained <= 2,
            "the diagnostic must not bypass the admitted A/B bound"
        );
        let owner_fragment = row.trial.harness.local_fragment(row.trial.owner).unwrap();
        let owner_leaves = leaves(&owner_fragment);
        let surface_primitives = owner_leaves
            .iter()
            .filter(|node| {
                let z = match node {
                    SceneNode::Rect(node) => node.z_index,
                    SceneNode::RoundedRect(node) => node.z_index,
                    SceneNode::BorderRect(node) => node.z_index,
                    SceneNode::GradientRect(node) => node.z_index,
                    _ => return false,
                };
                // Border/focus roles keep their own chrome above Surface.
                z == crate::core::paint::LAYER_CONTENT
            })
            .count();
        let sdf_image_primitives = owner_leaves
            .iter()
            .filter(|node| matches!(node, SceneNode::Image(_)))
            .count();
        let label = row.trial.tree.children(row.trial.button)[0];
        let label_node = row.trial.tree.node(label).unwrap();
        let child_text_presentation = label_node.widget.text_presentation().map(
            |presentation| json!({"text": presentation.text, "font_size": presentation.font_size}),
        );
        let label_fragment = row.trial.harness.local_fragment(label_node.id);
        let child_fragment_leaf_count = label_fragment
            .as_ref()
            .map_or(0, |fragment| leaves(fragment).len());
        let label_present = owner_leaves
            .iter()
            .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage"));
        assert!(
            label_present,
            "the actual Button-owned caption must remain present"
        );
        for index in 0..3 {
            let button = row.buttons[index];
            let allocation = row.trial.tree.get_absolute_paint_rect(button).unwrap();
            assert_eq!(
                row.trial
                    .tree
                    .get_absolute_paint_rect(row.hosts[index])
                    .unwrap(),
                allocation
            );
            assert_eq!(
                row.trial
                    .tree
                    .node(button)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<Button>()
                    .unwrap()
                    .layout_bounds,
                allocation.size()
            );
        }
        let selected = row
            .trial
            .harness
            .cache
            .selected_sdf_materials
            .as_ref()
            .and_then(|selected| selected.get(&row.trial.owner))
            .and_then(Option::as_ref)
            .map(|publication| format!("{:?}", publication.status()));
        let custody: Vec<_> = row
            .trial
            .harness
            .cache
            .sdf_scene_custody
            .references()
            .iter()
            .map(|reference| format!("{reference:?}"))
            .collect();
        let active_input = frozen.is_some();
        let omitted_procedural_face = active_input && image.is_none();
        let file = if row.trial.last_material_pending {
            assert!(!row.trial.scenes.contains_key(&row.trial.surface));
            None
        } else {
            assert!(
                !omitted_procedural_face,
                "produced output cannot lose an active replacement face"
            );
            if active_input {
                assert_eq!(sdf_image_primitives, 1);
                assert_eq!(surface_primitives, 0, "no substitute material");
            }
            let binding = image.as_ref().map(|image| &image.binding);
            let pixels = output(&mut row.trial, binding);
            let file = format!(
                "{}-{}x-rapid-{:03}-{phase}.ppm",
                if row.trial.theme.is_dark() {
                    "dark"
                } else {
                    "light"
                },
                row.trial.scale as u32,
                rows.len()
            );
            let extent = metrics.target_physical_extent;
            let mut proof = std::fs::File::create(directory.join(&file)).unwrap();
            write!(proof, "P6\n{} {}\n255\n", extent.width, extent.height).unwrap();
            for pixel in &pixels {
                proof.write_all(&pixel[..3]).unwrap();
            }
            Some(file)
        };
        let mut record = json!({
            "theme_dark": row.trial.theme.is_dark(),
            "scale": row.trial.scale,
            "phase": phase,
            "target_output_pending": row.trial.last_material_pending,
            "produced_frame": !row.trial.last_material_pending,
            "frame_within_phase": frame_within_phase,
            "owner": format!("{:?}", row.trial.owner),
            "face": [face.x, face.y, face.width, face.height],
            "viewport": [viewport.x, viewport.y, viewport.width, viewport.height],
            "current_key": current.map(|input| format!("{:?}", input.key)),
            "metrics": format!("{metrics:?}"),
            "paint_key": frozen.as_ref().map(|input| format!("{:?}", input.key)),
            "request": request.as_ref().map(|request| format!("{request:?}")),
            "selected_status": selected,
            "prospective_ordinary_chrome": !active_input && request.is_some(),
            "active_paint_input": active_input,
            "image_visible": image.is_some(),
            "omitted_procedural_face": omitted_procedural_face,
            "label_present": !row.trial.last_material_pending && label_present,
            "retained_label_present": label_present,
            "child_text_presentation": child_text_presentation,
            "child_label_fragment_present": label_fragment.is_some(),
            "child_label_fragment_leaf_count": child_fragment_leaf_count,
            "child_label_paint_policy": "Button::layout disables child Label paint and Button::paint owns the caption.",
            "ordinary_surface_primitives": surface_primitives,
        });
        let diagnostics = json!({
            "sdf_image_primitives": sdf_image_primitives,
            "label_only_candidate": omitted_procedural_face && label_present && surface_primitives == 0,
            "image_identity": image.as_ref().map(|image| format!("{:?}", image.binding.identity())),
            "destination": image.as_ref().map(|image| format!("{:?}", image.coverage)),
            "uv": image.as_ref().map(|image| format!("{:?}", image.uv)),
            "retained_generations": retained,
            "scene_custody": custody,
            "staging_before": staging_before,
            "staging_after": staging_after,
            "preparations_before": before.0,
            "preparations_after": after.0,
            "new_preparations": after.0.checked_sub(before.0),
            "preparation_cpu_ns_before": cpu_before.map(|value| value.to_string()),
            "preparation_cpu_ns_after": cpu_after.map(|value| value.to_string()),
            "new_preparation_cpu_ns": cpu_before.zip(cpu_after)
                .and_then(|(before, after)| after.checked_sub(before)).map(|value| value.to_string()),
            "producer_work_before": before.1,
            "producer_work_after": after.1,
            "producer_work_delta": after.1.iter().zip(before.1)
                .map(|(after, before)| *after - before).collect::<Vec<_>>(),
            "frame_cpu_ns": frame_cpu_ns.to_string(),
            "callbacks": row.clicks.iter().map(|count| count.get()).collect::<Vec<_>>(),
            "file": file,
        });
        record
            .as_object_mut()
            .unwrap()
            .extend(diagnostics.as_object().unwrap().clone());
        rows.push(record);
        !row.trial.last_material_pending && image.is_some()
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::env::temp_dir()
        .join("halcyon-button-natural-row-evidence")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    let mut rapid_rows = Vec::new();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        for fixed_host in [true, false] {
            let mut row = make_row(dark, scale, fixed_host);
            let mut previous: Option<(
                RectF,
                crate::renderer::image_binding::ImageBindingIdentity,
            )> = None;
            let mut effect_outsets: Option<(f32, f32, f32, f32)> = None;
            let font_size = row.trial.theme.typography.font_size;
            let run = row
                .trial
                .renderer
                .text_engine_mut()
                .shape_styled("Stage", None, font_size)
                .unwrap();
            let natural_width = run.width + row.trial.theme.geometry.button.padding_x * 2.0;
            assert!(natural_width > 10.0 && natural_width < 84.0);
            // Integer target metrics, with a real allocation below natural width.
            let narrow_width = (natural_width - 4.0).floor() + 20.0;
            for (phase, width) in [
                ("normal", 360.0),
                ("narrow", narrow_width),
                ("normal-return", 360.0),
            ] {
                let before = work(&row.trial);
                row.trial
                    .renderer
                    .resize_render_target(
                        row.trial.surface,
                        ((width * scale) as u32, (180.0 * scale) as u32),
                    )
                    .unwrap();
                row.trial
                    .compositor
                    .set_surface_bounds(row.trial.surface, RectF::new(0.0, 0.0, width, 180.0));
                row.trial.tree.mark_layout_dirty(row.trial.root);
                let binding = row.trial.settle(false);
                let after = work(&row.trial);
                let faces: Vec<_> = row
                    .buttons
                    .iter()
                    .map(|&button| row.trial.tree.get_absolute_paint_rect(button).unwrap())
                    .collect();
                let content = RectF::new(10.0, 10.0, width - 20.0, 160.0);
                for (index, &button) in row.buttons.iter().enumerate() {
                    let face = faces[index];
                    let host = row
                        .trial
                        .tree
                        .get_absolute_paint_rect(row.hosts[index])
                        .unwrap();
                    assert_eq!(
                        host, face,
                        "TooltipHost forwards the completed Button allocation"
                    );
                    let bounds = row
                        .trial
                        .tree
                        .node(button)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<Button>()
                        .unwrap()
                        .layout_bounds;
                    assert_eq!(bounds, face.size());
                    if !fixed_host || index != 0 || phase != "narrow" {
                        assert!(
                            face.x >= content.x - 0.001 && face.right() <= content.right() + 0.001
                        );
                        assert!(
                            face.y >= content.y - 0.001
                                && face.bottom() <= content.bottom() + 0.001
                        );
                    }
                    let visible = face
                        .intersection(&RectF::new(0.0, 0.0, width, 180.0))
                        .unwrap();
                    let center = Point::new(
                        visible.x + visible.width * 0.5,
                        visible.y + visible.height * 0.5,
                    );
                    let hit = row.trial.tree.hit_test(center.x, center.y).unwrap();
                    assert_eq!(hit.widget_id, row.trial.tree.node(button).unwrap().id);
                    assert_eq!(
                        hit.absolute_bounds,
                        (face.x, face.y, face.width, face.height)
                    );
                }
                let face = faces[0];
                if fixed_host {
                    assert_eq!(face.width, 84.0);
                    if phase == "narrow" {
                        assert!(
                            face.right() > content.right(),
                            "old fixed host reproduces overflow"
                        );
                    }
                } else {
                    let run = row
                        .trial
                        .renderer
                        .text_engine_mut()
                        .shape_styled("Stage", None, row.trial.theme.typography.font_size)
                        .unwrap();
                    let natural = run.width + row.trial.theme.geometry.button.padding_x * 2.0;
                    assert!((face.width - natural.min(content.width)).abs() < 0.001);
                    assert!(face.width >= 6.0 && face.height >= 6.0);
                    assert!(
                        face.width < 84.0,
                        "appearance does not impose the old demo width"
                    );
                }
                if phase == "narrow" {
                    assert!(faces[1].y >= face.bottom());
                    assert!(faces[2].y >= faces[1].bottom());
                } else {
                    assert_eq!(faces[1].y, face.y);
                    assert_eq!(faces[2].y, face.y);
                }
                if let Some((previous_face, previous_identity)) = previous {
                    if !fixed_host {
                        assert_ne!(
                            previous_face.size(),
                            face.size(),
                            "narrow/return must change natural F size"
                        );
                    }
                    if previous_face.size() == face.size() {
                        assert_eq!(binding.identity(), previous_identity);
                        assert_eq!(after, before, "unchanged F size needs no new producer work");
                    } else {
                        assert_ne!(
                            binding.identity(),
                            previous_identity,
                            "true size change needs new G"
                        );
                        assert_eq!(after.0, before.0 + 1);
                    }
                }
                previous = Some((face, binding.identity()));
                let image = row.trial.image().image.unwrap();
                let outsets = (
                    face.x - image.coverage.x,
                    face.y - image.coverage.y,
                    image.coverage.right() - face.right(),
                    image.coverage.bottom() - face.bottom(),
                );
                if let Some(expected) = effect_outsets {
                    for (actual, expected) in [outsets.0, outsets.1, outsets.2, outsets.3]
                        .into_iter()
                        .zip([expected.0, expected.1, expected.2, expected.3])
                    {
                        assert!(
                            (actual - expected).abs() < 0.001,
                            "current E must follow current F"
                        );
                    }
                } else {
                    effect_outsets = Some(outsets);
                }
                let name = format!(
                    "{}-{}x-{}-{phase}.ppm",
                    if dark { "dark" } else { "light" },
                    scale as u32,
                    if fixed_host {
                        "fixed-before"
                    } else {
                        "natural-after"
                    },
                );
                save(&mut row.trial, &binding, &directory, &name);
                let click_before = row.clicks[0].get();
                let stable_work = work(&row.trial);
                let visible = face
                    .intersection(&RectF::new(0.0, 0.0, width, 180.0))
                    .unwrap();
                let center = Point::new(
                    visible.x + visible.width * 0.5,
                    visible.y + visible.height * 0.5,
                );
                row.trial.pointer_at(true, center);
                row.trial.frame();
                assert_eq!(
                    row.trial.image().image.as_ref().unwrap().binding.identity(),
                    binding.identity()
                );
                assert_eq!(row.trial.image().image.as_ref().unwrap().uv.x, 0.5);
                assert_eq!(row.clicks[0].get(), click_before);
                save(
                    &mut row.trial,
                    &binding,
                    &directory,
                    &name.replace(".ppm", "-pressed.ppm"),
                );
                row.trial.pointer_at(false, center);
                row.trial.frame();
                assert_eq!(
                    row.trial.image().image.as_ref().unwrap().binding.identity(),
                    binding.identity()
                );
                assert_eq!(row.trial.image().image.as_ref().unwrap().uv.x, 0.0);
                assert_eq!(row.clicks[0].get(), click_before + 1);
                assert_eq!(work(&row.trial), stable_work);
                rows.push(json!({
                    "theme_dark": dark,
                    "scale": scale,
                    "fixed_host_before": fixed_host,
                    "phase": phase,
                    "face": [face.x, face.y, face.width, face.height],
                    "content": [content.x, content.y, content.width, content.height],
                    "peers": faces.iter().skip(1).map(|face| [face.x, face.y, face.width, face.height]).collect::<Vec<_>>(),
                    "identity": format!("{:?}", binding.identity()),
                    "destination": format!("{:?}", image.coverage),
                    "preparations": after.0 - before.0,
                    "producer_work_delta": after.1.iter().zip(before.1).map(|(after,before)| *after-before).collect::<Vec<_>>(),
                    "file": name,
                }));
            }
            assert_eq!(row.clicks[0].get(), 3);
            for index in 1..3 {
                let face = row
                    .trial
                    .tree
                    .get_absolute_paint_rect(row.buttons[index])
                    .unwrap();
                let center = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
                row.trial.pointer_at(true, center);
                row.trial.pointer_at(false, center);
                row.trial.frame();
                assert_eq!(row.clicks[index].get(), 1);
            }
            row.trial
                .tree
                .node_mut(row.trial.button)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
                .request_sdf_material(None)
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            while row.trial.harness.cache.sdf_material_staging.is_some() {
                assert!(Instant::now() < deadline);
                row.trial.frame();
            }
        }
        // Separate unsmoothed trace: one ordinary frame for each real size change.
        let mut row = make_row(dark, scale, false);
        let run = row
            .trial
            .renderer
            .text_engine_mut()
            .shape_styled("Stage", None, row.trial.theme.typography.font_size)
            .unwrap();
        let natural_width = run.width + row.trial.theme.geometry.button.padding_x * 2.0;
        let narrow_width = (natural_width - 4.0).floor() + 20.0;
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut attempt = 0;
        loop {
            assert!(
                Instant::now() < deadline,
                "rapid trace cold admission timed out"
            );
            if rapid_attempt(&mut row, &directory, "warm", attempt, &mut rapid_rows) {
                break;
            }
            attempt += 1;
        }
        let mut previous_face = row
            .trial
            .tree
            .get_absolute_paint_rect(row.trial.button)
            .unwrap();
        for (index, (phase, width)) in [
            ("normal-1", 360.0),
            ("narrow-1", narrow_width),
            ("normal-2", 360.0),
            ("narrow-2", narrow_width),
            ("normal-3", 360.0),
        ]
        .into_iter()
        .enumerate()
        {
            row.trial
                .renderer
                .resize_render_target(
                    row.trial.surface,
                    ((width * scale) as u32, (180.0 * scale) as u32),
                )
                .unwrap();
            row.trial
                .compositor
                .set_surface_bounds(row.trial.surface, RectF::new(0.0, 0.0, width, 180.0));
            row.trial.tree.mark_layout_dirty(row.trial.root);
            rapid_attempt(&mut row, &directory, phase, 0, &mut rapid_rows);
            let face = row
                .trial
                .tree
                .get_absolute_paint_rect(row.trial.button)
                .unwrap();
            assert!((face.width - natural_width.min(width - 20.0)).abs() < 0.001);
            if index > 0 {
                assert_ne!(
                    face.size(),
                    previous_face.size(),
                    "trace must change real F size"
                );
            }
            previous_face = face;
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut attempt = 0;
        loop {
            assert!(
                Instant::now() < deadline,
                "rapid trace final recovery timed out"
            );
            if rapid_attempt(&mut row, &directory, "recovery", attempt, &mut rapid_rows) {
                break;
            }
            attempt += 1;
        }
        let face = row
            .trial
            .tree
            .get_absolute_paint_rect(row.trial.button)
            .unwrap();
        let center = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
        row.trial.pointer_at(true, center);
        rapid_attempt(&mut row, &directory, "final-press", 0, &mut rapid_rows);
        assert_eq!(row.clicks[0].get(), 0);
        row.trial.pointer_at(false, center);
        rapid_attempt(&mut row, &directory, "final-release", 0, &mut rapid_rows);
        assert_eq!(row.clicks[0].get(), 1);
        assert_eq!(row.clicks[1].get(), 0);
        assert_eq!(row.clicks[2].get(), 0);
        row.trial
            .tree
            .node_mut(row.trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut attempt = 0;
        loop {
            assert!(
                Instant::now() < deadline,
                "rapid trace retirement timed out"
            );
            rapid_attempt(&mut row, &directory, "retirement", attempt, &mut rapid_rows);
            if row.trial.harness.cache.sdf_material_staging.is_none() {
                break;
            }
            attempt += 1;
        }
    }
    let evidence = json!({
        "scope": "Actual renderer-backed Flex wrapping row, TooltipHost and child-caption Button with Apply/Rollback peers; fixed-host before versus natural wrapperless after. Fixture topology only, not direct execution of the Showcase helper or native acceptance.",
        "showcase_source_sha256": format!("{:x}", Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/showcase/tab_inputs.rs")))),
        "fixture_source_sha256": format!("{:x}", Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/app/sdf_button_v3_gpu_tests.rs")))),
        "counter_scope": "Producer work only; composition/readback submissions are excluded. Existing rows settle; rapid_rows record every ordinary frame without intermediate settlement.",
        "rapid_scope": "Four wrapperless theme/scale cases: every cold warm-up, normal/narrow/normal/narrow/normal transition, recovery, final press/release and withdrawal/retirement attempt. Produced frames require an intact current material; target Pending records no fresh output and no readback. Each actual GPU readback waits for completion and therefore does not reproduce native asynchronous presentation cadence. Prospective ordinary cold chrome is distinct. This proves produced-frame continuity only, not native acceptance.",
        "rapid_rows": rapid_rows,
        "rows": rows,
    });
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "Natural TooltipHost Button row GPU evidence: {}",
        directory.display()
    );
}

#[test]
fn canonical_button_sdf_explicit_material_cost_trace() {
    use crate::renderer::custom_render_pass::audit;
    use crate::renderer::image_binding::ImageBindingIdentity;
    use crate::renderer::pixel_coverage::{PhysicalTargetMetrics, physical_render_extent};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    std::thread_local! {
        static PREPARE_NS: Cell<u128> = const { Cell::new(0) };
        static PREPARE_RETAINED: Cell<usize> = const { Cell::new(0) };
    }

    fn prepare(renderer: &mut WgpuRenderer, tree: &WidgetTree, cache: &mut RetainedSceneCache) {
        let started = Instant::now();
        prepare_canonical_sdf_materials(renderer, tree, cache);
        PREPARE_NS.set(started.elapsed().as_nanos());
        PREPARE_RETAINED.set(
            cache
                .sdf_material_staging
                .as_ref()
                .map_or(0, |staging| staging.retained_generation_count()),
        );
    }

    fn frame(trial: &mut ReferenceETrial) -> [u128; 4] {
        let started = Instant::now();
        PREPARE_NS.set(0);
        PREPARE_RETAINED.set(0);
        test_begin_frame_layout_tracking();
        let target = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap();
        let focused = trial.domains.primary_state().focus_manager.focused();
        let mode = trial.domains.primary_state().keyboard_mode;
        let mut triggers = Vec::new();
        let outcome = build_canonical_scene_reusing(
            &mut trial.tree,
            trial.root,
            Constraints::tight(
                trial
                    .compositor
                    .snapshot_multi()
                    .get(trial.surface)
                    .unwrap()
                    .bounds
                    .size(),
            ),
            &trial.theme,
            &mut trial.renderer,
            &crate::core::icons::builtin_registry(),
            trial.harness.frame_time_ms,
            0.0,
            &mut trial.harness.transition_layer,
            &mut trial.harness.active_layout_transitions,
            None,
            trial.scale,
            Some(target),
            &mut trial.harness.custom_render_presentation_bindings,
            None,
            None,
            false,
            &mut triggers,
            TooltipSceneInput::Explicit(None),
            focused,
            mode,
            None,
            false,
            &mut trial.harness.recycled_scene,
            &mut trial.harness.cache,
            Some(GestureSceneAccess {
                domains: &mut trial.domains,
                window: WindowId::PRIMARY,
                router: &mut trial.router,
                fusion: &mut trial.fusion,
            }),
            Some(prepare),
            None,
            false,
        );
        trial.last_material_pending = outcome.material_pending;
        if trial.last_material_pending {
            assert!(outcome.scene.is_none());
            assert!(!outcome.visual_changed && !outcome.needs_next_frame);
            trial.scenes.remove(&trial.surface);
        } else if let Some(scene) = outcome.scene {
            trial.scenes.insert(trial.surface, scene);
        }
        trial.harness.frame_time_ms += 16;
        let drain = Instant::now();
        trial.renderer.drain_pending_image_registrations();
        let drain_ns = drain.elapsed().as_nanos();
        let admission = Instant::now();
        trial
            .harness
            .cache
            .admit_sdf_materials_after_paint(&mut trial.renderer, &trial.tree);
        let admission_ns = admission.elapsed().as_nanos();
        trial
            .renderer
            .gpu_resources()
            .device
            .poll(wgpu::PollType::Poll)
            .unwrap();
        [
            started.elapsed().as_nanos(),
            PREPARE_NS.get(),
            drain_ns,
            admission_ns,
        ]
    }

    fn work(trial: &ReferenceETrial) -> ((u64, u128), [u64; 6]) {
        let prepared = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or((0, 0), |staging| staging.preparation_totals_for_test());
        let counts = audit::snapshot();
        (
            prepared,
            [
                counts.custom_render_passes,
                counts.custom_textures,
                counts.custom_command_encoders,
                counts.custom_submissions,
                counts.custom_registrations,
                counts.custom_unregistrations,
            ],
        )
    }

    // Source model only: the actual admitted E/UV must agree with the producer's
    // whole-cell/even-cell mapping. This is not a GPU descriptor/VRAM measurement.
    fn modeled_payload(
        input: &crate::core::widget::ButtonSdfPaintInput,
        image: &crate::core::widget::ButtonSdfPaintImage,
        format: wgpu::TextureFormat,
    ) -> Option<(u64, serde_json::Value)> {
        if format != wgpu::TextureFormat::Bgra8UnormSrgb {
            return None;
        }
        let metrics = input.key.metrics();
        let target = PhysicalTargetMetrics::new(
            metrics.target_logical_extent,
            metrics.target_physical_extent,
            metrics.scale_factor,
        )
        .ok()?;
        let extent = physical_render_extent(image.coverage.size(), target).ok()??;
        let contain = |value: u32, logical: f32| {
            if (value as f32) < logical * metrics.scale_factor {
                value.checked_add(1)
            } else {
                Some(value)
            }
        };
        let width = contain(extent.width, image.coverage.width)?;
        let cell_width = width.checked_add(width & 1)?;
        let texture_width = cell_width.checked_mul(2)?;
        let height = contain(extent.height, image.coverage.height)?;
        if image.uv.width != image.coverage.width * metrics.scale_factor / texture_width as f32
            || image.uv.height != image.coverage.height * metrics.scale_factor / height as f32
        {
            return None;
        }
        let bytes = u64::from(texture_width)
            .checked_mul(u64::from(height))?
            .checked_mul(4)?
            .checked_mul(2)?;
        Some((
            bytes,
            json!({
                "scope": "Modeled texture payload for this observed admitted generation only; not descriptors, complete live memory, driver allocation or VRAM.",
                "cell_width": cell_width,
                "cell_height": height,
                "horizontal_state_cells": 2,
                "texture_width": texture_width,
                "texture_height": height,
                "front_back_texture_objects": 2,
                "bytes_per_texel": 4,
                "format": format!("{format:?}"),
                "payload_bytes": bytes,
                "formula": "2 physical textures * (2 horizontal state cells * even contained cell_width) * contained height * 4 bytes",
                "source": "sdf_material_staging.rs71-168; custom_render_pass.rs369-387,607-629",
            }),
        ))
    }

    fn output(trial: &mut ReferenceETrial, capture: Option<&std::path::Path>) {
        assert!(
            !trial.last_material_pending,
            "Pending attempt has no current output"
        );
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| trial.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready");
        let gpu = trial.renderer.gpu_resources();
        let extent = trial
            .renderer
            .custom_render_target_snapshot(trial.surface)
            .unwrap()
            .metrics_for(
                trial.tree.get_absolute_paint_rect(trial.button).unwrap(),
                trial.tree.get_viewport(trial.button).unwrap(),
            )
            .unwrap()
            .target_physical_extent;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("explicit Button cost trace actual output"),
            size: wgpu::Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        if trial
            .harness
            .cache
            .sdf_scene_custody
            .references()
            .is_empty()
        {
            let data = compiled
                .plan()
                .surface_compiled_data(trial.surface)
                .unwrap();
            assert!(
                data.frame_batches
                    .batches
                    .iter()
                    .all(|batch| batch.key.pipeline != PipelineId::Image)
            );
            let command = trial
                .renderer
                .encode_compiled_target_for_test(compiled.plan(), trial.surface, &view)
                .unwrap();
            trial.renderer.submit_compiled_target_for_test(command);
            if capture.is_some() {
                let done = Arc::new(AtomicBool::new(false));
                let signal = done.clone();
                gpu.queue
                    .on_submitted_work_done(move || signal.store(true, Ordering::Release));
                poll_until(&gpu.device, &|| done.load(Ordering::Acquire));
            }
        } else {
            let done = compiled
                .submit_headless_for_test(&trial.renderer, trial.surface, &view)
                .unwrap();
            if capture.is_some() {
                poll_until(&gpu.device, &done);
            }
        }
        if let Some(path) = capture {
            let pixels = read_pixels(&gpu, &texture, extent.width, extent.height);
            let mut file = std::fs::File::create(path).unwrap();
            write!(file, "P6\n{} {}\n255\n", extent.width, extent.height).unwrap();
            for pixel in pixels {
                file.write_all(&pixel[..3]).unwrap();
            }
        }
        trial.harness.cache.note_sdf_target_output(trial.surface);
    }

    type Observed = HashMap<ImageBindingIdentity, (TreeWidgetId, ImageBinding, Option<u64>)>;

    fn observe(
        trial: &mut ReferenceETrial,
        buttons: &[NodeId],
        phase: &str,
        directory: &std::path::Path,
        capture: bool,
        known: &mut Observed,
        rows: &mut Vec<serde_json::Value>,
    ) -> (usize, usize) {
        let before = work(trial);
        let times = frame(trial);
        let after = work(trial);
        let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
        let mut ready = 0;
        let mut omitted = 0;
        let mut projected = Vec::new();
        for &button in buttons {
            let owner = trial.tree.node(button).unwrap().id;
            let face = trial.tree.get_absolute_paint_rect(button).unwrap();
            let viewport = trial.tree.get_viewport(button).unwrap();
            let request = trial
                .tree
                .node(button)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap()
                .sdf_request_snapshot();
            if let Some(input) = trial
                .harness
                .cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&owner))
                .filter(|_| !trial.last_material_pending)
            {
                if let Some(image) = &input.image {
                    let key = &current
                        .iter()
                        .find(|input| input.key.owner() == owner)
                        .unwrap()
                        .key;
                    assert_eq!(&input.key, key);
                    assert_eq!(key.metrics().final_logical_rect, face);
                    assert_eq!(key.metrics().final_logical_viewport, viewport);
                    assert_eq!(input.requested_revision, request.unwrap().revision);
                    assert_eq!(image.binding.availability(), Ok(()));
                    assert!(
                        trial
                            .harness
                            .cache
                            .sdf_scene_custody
                            .has_reference(key, image.binding.identity())
                    );
                    let model =
                        modeled_payload(input, image, trial.renderer.gpu_resources().format);
                    known.entry(image.binding.identity()).or_insert((
                        owner,
                        image.binding.clone(),
                        model.as_ref().map(|(bytes, _)| *bytes),
                    ));
                    projected.push(json!({
                        "owner": format!("{owner:?}"),
                        "key": format!("{key:?}"),
                        "identity": format!("{:?}", image.binding.identity()),
                        "face": [face.x, face.y, face.width, face.height],
                        "uv": format!("{:?}", image.uv),
                        "admitted_payload_model": model.map(|(_, model)| model),
                    }));
                    ready += 1;
                } else {
                    omitted += 1;
                }
            }
        }
        let retained = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or(0, |staging| staging.retained_generation_count());
        assert!(retained <= buttons.len() * 2);
        assert!(PREPARE_RETAINED.get() <= buttons.len() * 2);
        let mut available_per_owner: HashMap<TreeWidgetId, usize> = HashMap::new();
        let mut payload_bytes = 0u64;
        let mut payload_complete = true;
        for (owner, binding, model) in known.values() {
            if binding.availability().is_ok() {
                *available_per_owner.entry(*owner).or_default() += 1;
                if let Some(bytes) = model {
                    payload_bytes += bytes;
                } else {
                    payload_complete = false;
                }
            }
        }
        assert!(available_per_owner.values().all(|count| *count <= 2));
        assert_eq!(
            omitted, 0,
            "a produced active-size replacement cannot be label-only"
        );
        let filename = (capture && !trial.last_material_pending).then(|| {
            format!(
                "{}-{}x-{}buttons-{:03}-{phase}.ppm",
                if trial.theme.is_dark() {
                    "dark"
                } else {
                    "light"
                },
                trial.scale as u32,
                buttons.len(),
                rows.len()
            )
        });
        // All rendering/readback waits are outside the frame/phase CPU timers.
        if trial.last_material_pending {
            assert!(!trial.scenes.contains_key(&trial.surface));
        } else {
            output(
                trial,
                filename
                    .as_ref()
                    .map(|file| directory.join(file))
                    .as_deref(),
            );
        }
        let mut record = json!({
            "theme_dark": trial.theme.is_dark(),
            "scale": trial.scale,
            "buttons": buttons.len(),
            "phase": phase,
            "target_output_pending": trial.last_material_pending,
            "produced_frame": !trial.last_material_pending,
            "ready_images": ready,
            "active_omitted_images": omitted,
            "retained_generations_after_prepare": PREPARE_RETAINED.get(),
            "retained_generations_after_frame": retained,
            "per_owner_observed_available_generations": available_per_owner.iter()
                .map(|(owner, count)| json!({"owner":format!("{owner:?}"),"count":count})).collect::<Vec<_>>(),
            "observed_admitted_payload_bytes": payload_bytes,
            "observed_admitted_payload_models_complete": payload_complete,
            "preparations_before": before.0.0,
            "preparations_after": after.0.0,
            "new_successful_preparations": after.0.0.checked_sub(before.0.0),
            "successful_prepare_cpu_ns_before": before.0.1.to_string(),
            "successful_prepare_cpu_ns_after": after.0.1.to_string(),
            "successful_prepare_cpu_ns_delta": after.0.1.checked_sub(before.0.1).map(|value| value.to_string()),
        });
        let timings = json!({
            "frame_cpu_ns": times[0].to_string(),
            "prepare_hook_cpu_ns": times[1].to_string(),
            "registration_drain_cpu_ns": times[2].to_string(),
            "post_paint_admission_cpu_ns": times[3].to_string(),
            "producer_work_before": before.1,
            "producer_work_after": after.1,
            "producer_work_delta": after.1.iter().zip(before.1)
                .map(|(after, before)| *after-before).collect::<Vec<_>>(),
            "projected": projected,
            "file": filename,
        });
        record
            .as_object_mut()
            .unwrap()
            .extend(timings.as_object().unwrap().clone());
        rows.push(record);
        (ready, omitted)
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::env::temp_dir()
        .join("halcyon-button-explicit-material-cost")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    let mut summaries = Vec::new();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let renderer_start = Instant::now();
        let (mut trial, _, _) = ReferenceETrial::new(dark, scale, 84.0, false);
        let renderer_init_cpu_ns = renderer_start.elapsed().as_nanos();
        let shared_gpu = trial.renderer.gpu_resources();
        for count in [1usize, 16, 64] {
            assert!(Arc::ptr_eq(
                &shared_gpu.device,
                &trial.renderer.gpu_resources().device
            ));
            assert!(Arc::ptr_eq(
                &shared_gpu.queue,
                &trial.renderer.gpu_resources().queue
            ));
            assert!(trial.harness.cache.sdf_material_staging.is_none());
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(
                    Flex::column()
                        .with_spacing(6.0)
                        .with_padding(EdgeInsets::all(10.0))
                        .with_align_items(Alignment::Start)
                        .with_background(Color::from_hex(if dark { 0x242424 } else { 0xF2F2F2 })),
                ),
                None,
                0,
            );
            tree.set_overflow(root, OverflowMode::Clip);
            let mut holders = Vec::new();
            let mut buttons = Vec::new();
            let clicks = Rc::new(Cell::new(0));
            for index in 0..count {
                let holder = tree.add_child(root, Box::new(Container::empty()), None, 0);
                let click = clicks.clone();
                let mut button =
                    Button::with_text("Button").on_click(move || click.set(click.get() + 1));
                button
                    .request_sdf_material(Some(if dark {
                        ButtonSdfRestingProfile::DarkV3
                    } else {
                        ButtonSdfRestingProfile::LightV3
                    }))
                    .unwrap();
                let button = tree.add_child(holder, Box::new(button), None, index as i16);
                crate::replace_widget!(
                    tree,
                    holder,
                    Container::new(button, 0.0)
                        .with_width(84.0)
                        .with_height(30.0)
                );
                holders.push(holder);
                buttons.push(button);
            }
            trial.tree = tree;
            trial.root = root;
            trial.button = buttons[0];
            trial.owner = trial.tree.node(buttons[0]).unwrap().id;
            trial.harness = RetainedSceneTestHarness::new();
            trial.scenes.clear();
            trial.domains = InteractionDomains::new();
            trial.router = EventRouter::new();
            trial.fusion = InputFusion::new();
            let height = (count as f32 * 36.0 + 20.0).max(100.0);
            let resize = |trial: &mut ReferenceETrial, width: f32| {
                trial
                    .renderer
                    .resize_render_target(
                        trial.surface,
                        ((width * scale) as u32, (height * scale) as u32),
                    )
                    .unwrap();
                trial
                    .compositor
                    .set_surface_bounds(trial.surface, RectF::new(0.0, 0.0, width, height));
                trial.tree.mark_layout_dirty(trial.root);
            };
            resize(&mut trial, 120.0);
            let mut known = Observed::new();
            let first_row = rows.len();
            let start_work = work(&trial).1;
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "explicit cost cold admission did not settle"
                );
                if observe(
                    &mut trial, &buttons, "cold", &directory, false, &mut known, &mut rows,
                )
                .0 == count
                {
                    break;
                }
                std::thread::yield_now();
            }
            observe(
                &mut trial,
                &buttons,
                "warm-readback",
                &directory,
                true,
                &mut known,
                &mut rows,
            );
            let warm_work = work(&trial);
            let warm_identities: Vec<_> = buttons
                .iter()
                .map(|button| {
                    let owner = trial.tree.node(*button).unwrap().id;
                    trial.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&owner]
                        .image
                        .as_ref()
                        .unwrap()
                        .binding
                        .identity()
                })
                .collect();
            for _ in 0..3 {
                assert_eq!(
                    observe(
                        &mut trial, &buttons, "warm", &directory, false, &mut known, &mut rows
                    )
                    .0,
                    count
                );
                assert_eq!(
                    work(&trial),
                    warm_work,
                    "warm explicit materials must reuse production"
                );
            }
            trial.pointer_at(true, Point::new(52.0, 25.0));
            observe(
                &mut trial, &buttons, "press", &directory, false, &mut known, &mut rows,
            );
            assert_eq!(clicks.get(), 0);
            trial.pointer_at(false, Point::new(52.0, 25.0));
            observe(
                &mut trial, &buttons, "release", &directory, false, &mut known, &mut rows,
            );
            assert_eq!(clicks.get(), 1);
            assert_eq!(
                work(&trial),
                warm_work,
                "warm endpoint selection must not produce resources"
            );
            for (button, identity) in buttons.iter().zip(warm_identities) {
                let owner = trial.tree.node(*button).unwrap().id;
                assert_eq!(
                    trial.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&owner]
                        .image
                        .as_ref()
                        .unwrap()
                        .binding
                        .identity(),
                    identity
                );
            }
            for (index, width) in [76.0, 84.0, 76.0, 84.0].into_iter().enumerate() {
                for (&holder, &button) in holders.iter().zip(&buttons) {
                    crate::replace_widget!(
                        trial.tree,
                        holder,
                        Container::new(button, 0.0)
                            .with_width(width)
                            .with_height(30.0)
                    );
                }
                resize(&mut trial, if width == 76.0 { 112.0 } else { 120.0 });
                observe(
                    &mut trial,
                    &buttons,
                    "rapid-size",
                    &directory,
                    index == 0,
                    &mut known,
                    &mut rows,
                );
                for &button in &buttons {
                    assert_eq!(
                        trial.tree.get_absolute_paint_rect(button).unwrap().size(),
                        Size::new(width, 30.0)
                    );
                }
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "explicit cost recovery did not settle"
                );
                if observe(
                    &mut trial, &buttons, "recovery", &directory, false, &mut known, &mut rows,
                )
                .0 == count
                {
                    break;
                }
                std::thread::yield_now();
            }
            observe(
                &mut trial,
                &buttons,
                "recovered-readback",
                &directory,
                true,
                &mut known,
                &mut rows,
            );
            for &button in &buttons {
                trial
                    .tree
                    .node_mut(button)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<Button>()
                    .unwrap()
                    .request_sdf_material(None)
                    .unwrap();
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "explicit cost cleanup did not retire"
                );
                observe(
                    &mut trial,
                    &buttons,
                    "withdraw-cleanup",
                    &directory,
                    false,
                    &mut known,
                    &mut rows,
                );
                if trial.harness.cache.sdf_material_staging.is_none() {
                    break;
                }
                std::thread::yield_now();
            }
            assert!(
                trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .references()
                    .is_empty()
            );
            assert!(
                known
                    .values()
                    .all(|(_, binding, _)| binding.availability().is_err())
            );
            let baseline_work = work(&trial);
            for _ in 0..3 {
                assert_eq!(
                    observe(
                        &mut trial,
                        &buttons,
                        "plain-baseline",
                        &directory,
                        false,
                        &mut known,
                        &mut rows
                    ),
                    (0, 0)
                );
                assert_eq!(
                    work(&trial),
                    baseline_work,
                    "plain baseline must create no material work"
                );
                assert!(trial.harness.cache.sdf_material_staging.is_none());
                assert!(
                    trial
                        .harness
                        .cache
                        .sdf_scene_custody
                        .references()
                        .is_empty()
                );
            }
            let final_work = work(&trial).1;
            summaries.push(json!({
                "theme_dark":dark,"scale":scale,"buttons":count,
                "shared_renderer_reused_for_counts":[1,16,64],
                "renderer_init_cpu_ns":renderer_init_cpu_ns.to_string(),
                "first_row":first_row,"end_row":rows.len(),
                "producer_work_delta":final_work.iter().zip(start_work).map(|(after,before)|*after-before).collect::<Vec<_>>(),
                "observed_admitted_generations":known.len(),
                "cleanup_staging_empty":true,
                "cleanup_observed_bindings_unavailable":true,
            }));
        }
    }
    let evidence = json!({
        "scope":"Actual explicit legacy SDF Button cost trace; no default, supported-state, scale or architecture change.",
        "fixture_source_sha256":format!("{:x}",Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/core/app/sdf_button_v3_gpu_tests.rs")))),
        "app_source_sha256":format!("{:x}",Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/core/app.rs")))),
        "staging_source_sha256":format!("{:x}",Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/renderer/wgpu_renderer/sdf_material_staging.rs")))),
        "counter_order":["producer_passes","producer_textures_created","producer_encoders","producer_submissions","registrations","unregistrations"],
        "timing_scope":"CPU stopwatches around the real canonical frame, preparation hook, registrar drain and post-Paint admission; all output encoding/readback/GPU waits are outside these timers. Successful preparation timer is the existing producer-owned counter. Withdrawal preparation-hook time includes actual retirement plus owner bookkeeping, not a pure destructor measurement.",
        "resource_limits":"Retained generation totals are sampled after preparation/frame, not complete intra-driver live peaks. Per-owner figures are observed available admitted bindings only, excluding unadmitted/preparing generations. Modeled payload is admitted generations only, verified against source mapping and actual E/UV; excludes driver overhead, uniform/pipeline/command memory and unadmitted allocations. No actual VRAM/residency claim.",
        "cadence_limits":"Representative GPU readbacks wait for completion; dt0 and fixture cadence do not establish native timing or visual acceptance. No guessed performance thresholds.",
        "summaries":summaries,"rows":rows,
    });
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "Explicit Button production cost evidence: {}",
        directory.display()
    );
}

#[test]
fn canonical_button_sdf_active_size_pending_preserves_generation_custody() {
    use crate::renderer::custom_render_pass::audit;
    use crate::renderer::sdf_frame_custody::SdfCompiledFrame;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn counters() -> [u64; 6] {
        let counts = audit::snapshot();
        [
            counts.custom_render_passes,
            counts.custom_textures,
            counts.custom_command_encoders,
            counts.custom_submissions,
            counts.custom_registrations,
            counts.custom_unregistrations,
        ]
    }

    fn retained(trial: &ReferenceETrial) -> usize {
        trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .map_or(0, |staging| staging.retained_generation_count())
    }

    fn resize_face(trial: &mut ReferenceETrial, width: f32) {
        let holder = trial.tree.children(trial.root)[0];
        crate::replace_widget!(
            trial.tree,
            holder,
            Container::new(trial.button, 0.0)
                .with_width(width)
                .with_height(30.0)
        );
        trial.tree.mark_layout_dirty(trial.root);
    }

    fn compiled(trial: &mut ReferenceETrial) -> SdfCompiledFrame {
        assert!(!trial.last_material_pending);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &trial.scenes,
                trial.compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| trial.scale,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        trial
            .renderer
            .compile_sdf_frame(
                SdfFramePlan::new(plan, trial.harness.cache.sdf_scene_custody.clone()),
                None,
            )
            .unwrap()
            .expect_ready("native Compile must be ready")
    }

    fn held_sampling(trial: &mut ReferenceETrial) -> impl Fn() -> bool + use<> {
        let gpu = trial.renderer.gpu_resources();
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("current Button plan held sampling completion"),
            size: wgpu::Extent3d {
                width: (240.0 * trial.scale) as u32,
                height: (100.0 * trial.scale) as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        // Encode and submit while this exact compiled preparation is current.
        // Only its real completion observation stays held across later attempts.
        let completion = compiled(trial)
            .submit_headless_for_test(
                &trial.renderer,
                trial.surface,
                &output.create_view(&Default::default()),
            )
            .unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let signal = done.clone();
        gpu.queue
            .on_submitted_work_done(move || signal.store(true, Ordering::Release));
        poll_until(&gpu.device, &|| done.load(Ordering::Acquire));
        assert!(!completion(), "sampling certificate observation is held");
        completion
    }

    fn save(
        trial: &mut ReferenceETrial,
        binding: &ImageBinding,
        directory: &std::path::Path,
        phase: &str,
        rows: &mut Vec<serde_json::Value>,
    ) {
        assert!(!trial.last_material_pending);
        let input = trial.image();
        let current = trial.harness.cache.current_sdf_frame_inputs(&trial.tree);
        assert!(current.iter().any(|current| current.key == input.key));
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        assert_eq!(input.key.metrics().final_logical_rect, face);
        let pixels = trial.pixels(binding);
        let width = (240.0 * trial.scale) as u32;
        let height = (100.0 * trial.scale) as u32;
        let background =
            pixels[(5.0 * trial.scale) as usize * width as usize + (5.0 * trial.scale) as usize];
        let mut visible_face_pixels = 0usize;
        for y in ((face.y + face.height * 0.75) * trial.scale) as u32
            ..((face.y + face.height * 0.9) * trial.scale) as u32
        {
            for x in ((face.x + face.width * 0.2) * trial.scale) as u32
                ..((face.x + face.width * 0.8) * trial.scale) as u32
            {
                visible_face_pixels += usize::from(pixels[(y * width + x) as usize] != background);
            }
        }
        assert!(
            visible_face_pixels > 0,
            "the compiled current Image must produce a visible face below its caption"
        );
        let name = format!(
            "{}-{}x-{phase}.ppm",
            if trial.theme.is_dark() {
                "dark"
            } else {
                "light"
            },
            trial.scale as u32
        );
        let mut file = std::fs::File::create(directory.join(&name)).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for pixel in pixels {
            file.write_all(&pixel[..3]).unwrap();
        }
        rows.push(json!({
            "theme_dark": trial.theme.is_dark(),
            "scale": trial.scale,
            "phase": phase,
            "target_output_pending": false,
            "produced_frame": true,
            "key": format!("{:?}", input.key),
            "identity": format!("{:?}", binding.identity()),
            "face": [face.x, face.y, face.width, face.height],
            "visible_face_pixels": visible_face_pixels,
            "retained_generations": retained(trial),
            "producer_counters": counters(),
            "file": name,
        }));
    }

    fn pending(
        trial: &ReferenceETrial,
        phase: &str,
        held_work: [u64; 6],
        rows: &mut Vec<serde_json::Value>,
    ) {
        assert!(
            trial.last_material_pending,
            "two retained records defer only dependent output"
        );
        assert!(!trial.scenes.contains_key(&trial.surface));
        assert_eq!(retained(trial), 2);
        assert_eq!(
            counters(),
            held_work,
            "Pending must not allocate a third record or fabricate retirement"
        );
        rows.push(json!({
            "theme_dark": trial.theme.is_dark(),
            "scale": trial.scale,
            "phase": phase,
            "target_output_pending": true,
            "produced_frame": false,
            "face": format!("{:?}", trial.tree.get_absolute_paint_rect(trial.button).unwrap()),
            "request": format!("{:?}", trial.request()),
            "retained_generations": retained(trial),
            "producer_counters": counters(),
            "file": serde_json::Value::Null,
        }));
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let directory = std::env::temp_dir()
        .join("halcyon-button-active-size-custody")
        .join(format!("run-{stamp}"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let baseline = counters();
        let (mut trial, clicks, _) = ReferenceETrial::new(dark, scale, 84.0, false);
        trial.frame();
        assert!(
            !trial.last_material_pending,
            "initial optional activation remains staged"
        );
        assert!(
            trial
                .harness
                .cache
                .sdf_paint_inputs
                .as_ref()
                .and_then(|inputs| inputs.get(&trial.owner))
                .is_none_or(|input| input.image.is_none()),
            "cold frame cannot admit its optional Image before Paint"
        );
        let original = trial.settle(false);
        save(
            &mut trial,
            &original,
            &directory,
            "initial-output",
            &mut rows,
        );
        let original_key = trial.image().key;
        let held_original = compiled(&mut trial);
        let hold_original_sampling = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .unwrap()
            .hold_sampling_completion_observation_for_test(&original_key)
            .unwrap();
        hold_original_sampling.store(true, Ordering::Release);
        let original_sampling = held_sampling(&mut trial);

        resize_face(&mut trial, 76.0);
        trial.frame_observing(false);
        assert!(
            !trial.last_material_pending,
            "one free record admits the active replacement before Paint"
        );
        let replacement = trial.image();
        assert_eq!(
            replacement.key.metrics().final_logical_rect.size(),
            Size::new(76.0, 30.0)
        );
        let replacement_binding = replacement.image.as_ref().unwrap().binding.clone();
        assert_ne!(replacement_binding.identity(), original.identity());
        let hold_writer = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .unwrap()
            .hold_writer_completion_observation_for_test(&replacement.key)
            .unwrap();
        let hold_sampling = trial
            .harness
            .cache
            .sdf_material_staging
            .as_ref()
            .unwrap()
            .hold_sampling_completion_observation_for_test(&replacement.key)
            .unwrap();
        hold_sampling.store(false, Ordering::Release);
        hold_writer.store(true, Ordering::Release);
        let held_replacement = compiled(&mut trial);
        save(
            &mut trial,
            &replacement_binding,
            &directory,
            "ordered-write-replacement",
            &mut rows,
        );
        hold_sampling.store(true, Ordering::Release);
        let replacement_sampling = held_sampling(&mut trial);
        assert_eq!(retained(&trial), 2);
        let held_work = counters();
        assert_eq!(
            trial
                .harness
                .cache
                .sdf_material_staging
                .as_ref()
                .unwrap()
                .preparation_totals_for_test()
                .0,
            2
        );

        resize_face(&mut trial, 72.0);
        trial.frame();
        pending(
            &trial,
            "capacity-held-cpu-write-sample",
            held_work,
            &mut rows,
        );
        let face = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        assert_eq!(face.size(), Size::new(72.0, 30.0));
        let center = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
        trial.pointer_at(true, center);
        trial.frame();
        assert!(
            trial.request().pressed,
            "ordinary input/Update continues while output is Pending"
        );
        pending(&trial, "pending-press", held_work, &mut rows);
        trial.pointer_at(false, center);
        trial.frame();
        assert!(!trial.request().pressed);
        assert_eq!(clicks.get(), 1);
        pending(&trial, "pending-release", held_work, &mut rows);

        // Old compiled CPU work is cancelled after currentness changes. Its
        // previously submitted real sampling remains independently protected.
        drop(held_original);
        assert!(
            !original_sampling(),
            "real sampling completion remains unobserved after CPU cancellation"
        );
        trial.frame();
        assert!(original.availability().is_ok());
        pending(
            &trial,
            "cpu-released-later-sampling-held",
            held_work,
            &mut rows,
        );

        hold_original_sampling.store(false, Ordering::Release);
        assert!(
            original_sampling(),
            "the exact pre-resize sampling certificate becomes observable"
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                Instant::now() < deadline,
                "latest-F output did not resume after exact retirement"
            );
            trial.frame();
            if !trial.last_material_pending {
                break;
            }
            rows.push(json!({
                "theme_dark": dark, "scale": scale, "phase": "certified-capacity-retry",
                "target_output_pending": true, "produced_frame": false,
                "retained_generations": retained(&trial), "producer_counters": counters(),
                "file": serde_json::Value::Null,
            }));
            assert!(!trial.scenes.contains_key(&trial.surface));
            assert!(retained(&trial) <= 2);
        }
        let latest = trial.image();
        let latest_binding = latest.image.as_ref().unwrap().binding.clone();
        assert_ne!(latest_binding.identity(), original.identity());
        assert_ne!(latest_binding.identity(), replacement_binding.identity());
        assert_eq!(
            latest.key.metrics().final_logical_rect.size(),
            Size::new(72.0, 30.0)
        );
        assert_eq!(latest.requested_revision, trial.request().revision);
        assert_eq!(latest.image.as_ref().unwrap().uv.x, 0.0);
        assert_eq!(
            trial
                .harness
                .cache
                .sdf_material_staging
                .as_ref()
                .unwrap()
                .preparation_totals_for_test()
                .0,
            3
        );
        save(
            &mut trial,
            &latest_binding,
            &directory,
            "latest-current-output",
            &mut rows,
        );
        assert_eq!(
            original.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert!(replacement_binding.availability().is_ok());

        trial
            .tree
            .node_mut(trial.button)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
            .request_sdf_material(None)
            .unwrap();
        trial.frame();
        assert!(
            !trial.last_material_pending,
            "withdrawal ends only the scoped output gate"
        );
        drop(held_replacement);
        trial.frame();
        assert!(
            replacement_binding.availability().is_ok(),
            "real writer completion is still unobserved"
        );
        hold_writer.store(false, Ordering::Release);
        trial.frame();
        assert!(
            replacement_binding.availability().is_ok(),
            "writer completion cannot settle sampling custody"
        );
        hold_sampling.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(15);
        assert!(
            replacement_sampling(),
            "the exact replacement sampling certificate settles"
        );
        while trial.harness.cache.sdf_material_staging.is_some() {
            assert!(
                Instant::now() < deadline,
                "certified cleanup did not finish"
            );
            trial.frame();
        }
        assert_eq!(
            replacement_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(
            latest_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert!(
            trial
                .harness
                .cache
                .sdf_scene_custody
                .references()
                .is_empty()
        );
        assert_eq!(clicks.get(), 1);
        let final_counts = counters();
        assert_eq!(
            final_counts[3] - baseline[3],
            3,
            "three genuine producer submissions"
        );
        assert_eq!(
            final_counts[4] - baseline[4],
            3,
            "three genuine registrations"
        );
        assert_eq!(
            final_counts[5] - baseline[5],
            3,
            "each registered generation retires exactly once"
        );
        rows.push(json!({
            "theme_dark": dark, "scale": scale, "phase": "certified-cleanup",
            "producer_delta": final_counts.iter().zip(baseline).map(|(after,before)|*after-before).collect::<Vec<_>>(),
            "all_three_generations_unregistered": true, "callbacks": clicks.get(),
        }));
    }
    std::fs::write(
        directory.join("trace.json"),
        serde_json::to_vec_pretty(&json!({
            "scope": "Existing explicit active Rest/Pressed pair only. Actual initial output, same-attempt changed-F production and two-record Pending under actual CPU/writer/sampling custody; no default rollout.",
            "fixture_source_sha256": format!("{:x}", Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/app/sdf_button_v3_gpu_tests.rs")))),
            "app_source_sha256": format!("{:x}", Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/app.rs")))),
            "staging_source_sha256": format!("{:x}", Sha256::digest(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/renderer/wgpu_renderer/sdf_material_staging.rs")))),
            "proof_limits": "Observation holds only suppress genuine completion observation; they fabricate no completion. Sampling submits only while its exact compiled preparation is current; later real observations remain held across resize. Pending attempts submit no current output and have no screenshot. Readbacks establish headless produced-frame presence, not native presentation continuity or visual acceptance.",
            "rows": rows,
        })).unwrap(),
    ).unwrap();
    println!(
        "Active explicit Button size/custody GPU evidence: {}",
        directory.display()
    );
}
