use super::*;
use crate::core::event::*;
use crate::renderer::sdf_frame_custody::{SdfCompiledFrame, SdfFramePlan};
use crate::renderer::sdf_material_status::SdfMaterialStatus;
use crate::runtime::binding_registry::BindingRegistry;

#[test]
fn canonical_button_sdf_v2_pressed_pixels_follow_real_input() {
    struct Trial {
        renderer: WgpuRenderer,
        compositor: Compositor,
        surface: SurfaceId,
        tree: WidgetTree,
        root: NodeId,
        node: NodeId,
        peer: NodeId,
        owner: TreeWidgetId,
        domains: InteractionDomains,
        router: EventRouter,
        fusion: InputFusion,
        bindings: BindingRegistry,
        harness: RetainedSceneTestHarness,
        scenes: HashMap<SurfaceId, SceneGraph>,
        theme: Theme,
        scale: f32,
        clicks: Rc<Cell<u32>>,
        require_image: bool,
    }

    impl Trial {
        fn frame(&mut self) -> bool {
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
                Constraints::tight(Size::new(240.0, 100.0)),
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
            let built = outcome.scene.is_some();
            if let Some(scene) = outcome.scene {
                self.scenes.insert(self.surface, scene);
            }
            self.harness.frame_time_ms += 16;
            self.renderer.drain_pending_image_registrations();
            self.harness
                .cache
                .admit_sdf_materials_after_paint(&mut self.renderer, &self.tree);
            self.renderer
                .gpu_resources()
                .device
                .poll(wgpu::PollType::Poll)
                .unwrap();
            if self.require_image {
                self.assert_continuity();
            }
            built
        }

        fn assert_continuity(&mut self) {
            let request = self.request();
            let input = self.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&self.owner].clone();
            let image = input.image.as_ref().expect("no empty V2 transition frame");
            assert_eq!(input.requested_revision, request.revision);
            assert_eq!(image.binding.availability(), Ok(()));
            let publication = self.harness.cache.selected_sdf_materials.as_ref().unwrap()
                [&self.owner]
                .as_ref()
                .unwrap();
            assert_eq!(
                publication.key(),
                &input.key,
                "committed image keeps its actual K"
            );
            assert!(
                matches!(publication.status(), SdfMaterialStatus::Ready { binding }
                if binding.identity() == image.binding.identity())
            );
            let staging = self.harness.cache.sdf_material_staging.as_ref().unwrap();
            assert!(
                staging.retained_generation_count() <= 2,
                "bounded owner generations"
            );
            if input.key.revision() != request.revision {
                let current = self.harness.cache.current_sdf_frame_inputs(&self.tree);
                assert!(staging.can_continue_selection(&current[0], &input.key));
                assert_ne!(
                    publication.key().revision(),
                    request.revision,
                    "pending revision must not claim the old Image as Ready"
                );
            }
            assert!(
                self.harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&input.key, image.binding.identity())
            );
            assert!(leaves(&self.harness.local_fragment(self.owner).unwrap()).iter()
                .any(|node| matches!(node, SceneNode::Image(actual)
                    if actual.texture_source == ImageTextureSource::Admitted(image.binding.identity())
                        && actual.rect == image.coverage)));
            // Compile the actual retained scene on every transition frame; drop
            // terminally cancels this plan without claiming a GPU submission.
            let compiled = self.compiled();
            let data = compiled.plan().surface_compiled_data(self.surface).unwrap();
            assert!(
                data.frame_batches
                    .batches
                    .iter()
                    .any(|batch| batch.key.pipeline == PipelineId::Image
                        && batch.key.texture_id == Some(image.binding.identity().texture_id())
                        && batch.key.image_generation
                            == Some(image.binding.identity().generation()))
            );
        }

        fn request(&self) -> crate::widgets::button::ButtonSdfRequestSnapshot {
            self.tree
                .node(self.node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap()
                .sdf_request_snapshot()
                .unwrap()
        }

        fn settle(&mut self, pressed: bool) -> ImageBinding {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "actual V2 front failed to settle"
                );
                self.frame();
                let request = self.request();
                assert_eq!(request.pressed, pressed);
                if let Some(input) = self
                    .harness
                    .cache
                    .sdf_paint_inputs
                    .as_ref()
                    .and_then(|inputs| inputs.get(&self.owner))
                {
                    if input.key.revision() == request.revision {
                        if let Some(image) = &input.image {
                            assert!(
                                self.harness
                                    .cache
                                    .sdf_scene_custody
                                    .has_reference(&input.key, image.binding.identity())
                            );
                            return image.binding.clone();
                        }
                    }
                }
                std::thread::yield_now();
            }
        }

        fn compiled(&mut self) -> SdfCompiledFrame {
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &self.scenes,
                    self.compositor.snapshot_multi(),
                    &mut MeshCache::new(),
                    |_| self.scale,
                    0,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            self.renderer
                .compile_sdf_frame(
                    SdfFramePlan::new(plan, self.harness.cache.sdf_scene_custody.clone()),
                    None,
                )
                .unwrap()
                .expect_ready("native Compile must be ready")
        }

        fn pixels(&mut self, binding: &ImageBinding) -> Vec<[u8; 4]> {
            let compiled = self.compiled();
            let data = compiled.plan().surface_compiled_data(self.surface).unwrap();
            assert!(
                data.frame_batches
                    .batches
                    .iter()
                    .any(|b| b.key.pipeline == PipelineId::Image
                        && b.key.texture_id == Some(binding.identity().texture_id())
                        && b.key.image_generation == Some(binding.identity().generation()))
            );
            let gpu = self.renderer.gpu_resources();
            let (width, height) = ((240.0 * self.scale) as u32, (100.0 * self.scale) as u32);
            let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("V2 real retained Button pixels"),
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
            read_pixels(&gpu, &output, width, height)
        }

        fn route(&mut self, event: FusedInputEvent) {
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

        fn pointer(&mut self, down: bool) {
            let time = Instant::now();
            let pos = Point::new(70.0, 35.0);
            self.route(if down {
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
            });
        }

        fn key(&mut self, down: bool) {
            let time = Instant::now();
            self.route(if down {
                FusedInputEvent::KeyDown {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Enter,
                    physical_key: PhysicalKeyCode::Unknown,
                    reservation: None,
                    modifiers: Modifiers::default(),
                    repeat: false,
                }
            } else {
                FusedInputEvent::KeyUp {
                    window_id: WindowId::PRIMARY,
                    time,
                    key_code: KeyCode::Enter,
                    physical_key: PhysicalKeyCode::Unknown,
                    reservation: None,
                    modifiers: Modifiers::default(),
                }
            });
        }
    }

    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/4e-button-v2");
    std::fs::create_dir_all(&directory).unwrap();
    for (dark, scale) in [(false, 1.0), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real V2 GPU required");
        let bounds = RectF::new(0.0, 0.0, 240.0, 100.0);
        let mut compositor = Compositor::new(bounds);
        let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
        renderer
            .register_headless_render_target(
                surface,
                ((240.0 * scale) as u32, (100.0 * scale) as u32),
                scale,
            )
            .unwrap();
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
            Box::new(Container::empty().with_width(120.0).with_height(30.0)),
            None,
            0,
        );
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let mut button = Button::with_text("Stage").on_click(move || count.set(count.get() + 1));
        button
            .request_sdf_material(Some(if dark {
                ButtonSdfRestingProfile::DarkV2
            } else {
                ButtonSdfRestingProfile::LightV2
            }))
            .unwrap();
        let node = tree.add_child(holder, Box::new(button), None, 0);
        let owner = tree.node(node).unwrap().id;
        let peer = tree.add_child(root, Box::new(Label::new("Peer")), None, 0);
        let peer_id = tree.node(peer).unwrap().id;
        let root_id = tree.node(root).unwrap().id;
        let mut trial = Trial {
            renderer,
            compositor,
            surface,
            tree,
            root,
            node,
            peer,
            owner,
            domains: InteractionDomains::new(),
            router: EventRouter::new(),
            fusion: InputFusion::new(),
            bindings: BindingRegistry::new(),
            harness: RetainedSceneTestHarness::new(),
            scenes: HashMap::new(),
            theme: if dark { Theme::dark() } else { Theme::light() },
            scale,
            clicks,
            require_image: false,
        };
        let rest_binding = trial.settle(false);
        trial.require_image = true;
        let rest_revision = trial.request().revision;
        let face = trial.tree.get_absolute_paint_rect(node).unwrap();
        assert_eq!(face, RectF::new(20.0, 20.0, 120.0, 30.0));
        let geometry = [root, holder, node, peer].map(|n| {
            (
                trial.tree.node(n).unwrap().id,
                trial.tree.get_absolute_paint_rect(n),
                trial.tree.get_viewport(n),
            )
        });
        let peer_local = trial.harness.local_fragment(peer_id).unwrap();
        let root_local = trial.harness.local_fragment(root_id).unwrap();
        let resting = trial.pixels(&rest_binding);
        let cancelled = trial.compiled();
        trial.pointer(true);
        assert_eq!(trial.clicks.get(), 0);
        trial.frame();
        assert!(trial.request().pressed);
        assert_eq!(
            trial.request().revision,
            rest_revision.checked_increment().unwrap()
        );
        assert_eq!(
            rest_binding.availability(),
            Ok(()),
            "compiled old generation cannot retire"
        );
        assert!(
            trial
                .harness
                .cache
                .sdf_scene_custody
                .references()
                .iter()
                .any(|(_, id)| *id == rest_binding.identity())
        );
        let pending = &trial.harness.cache.sdf_paint_inputs.as_ref().unwrap()[&owner];
        assert_eq!(pending.key.revision(), rest_revision);
        assert_ne!(pending.key.revision(), trial.request().revision);
        let pressed_binding = trial.settle(true);
        assert_ne!(pressed_binding.identity(), rest_binding.identity());
        assert!(
            !trial
                .harness
                .cache
                .sdf_scene_custody
                .references()
                .iter()
                .any(|(_, identity)| *identity == rest_binding.identity())
        );
        assert_eq!(
            rest_binding.availability(),
            Ok(()),
            "cancelled old plan still owns CPU custody"
        );
        drop(cancelled);
        trial.frame();
        assert_eq!(
            rest_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        let pressed = trial.pixels(&pressed_binding);
        let width = (240.0 * scale) as usize;
        let sample =
            |p: &[[u8; 4]], x: f32, y: f32| p[(y * scale) as usize * width + (x * scale) as usize];
        let rest_face = sample(&resting, 32.0, 35.0);
        let press_face = sample(&pressed, 32.0, 35.0);
        assert!(
            press_face[0] + 3 < rest_face[0],
            "pressed face must visibly depress: {rest_face:?}->{press_face:?}"
        );
        assert!(
            sample(&pressed, 80.0, 21.5)[0] < sample(&pressed, 80.0, 48.5)[0],
            "concave upper edge is darker than lower edge"
        );
        trial.pointer(false);
        trial.settle(false);
        assert_eq!(trial.clicks.get(), 1);
        // A second press cancelled by the real router cannot emit a click.
        trial.pointer(true);
        trial.settle(true);
        trial.router.cancel_pointer_session_with_tree(
            &mut trial.tree,
            PointerId::MOUSE,
            CancellationReason::OverlayTakeover,
            trial.domains.primary_state_mut(),
        );
        trial.settle(false);
        trial.pointer(false);
        trial.settle(false);
        assert_eq!(trial.clicks.get(), 1);
        // Honest framework focus alone remains resting; keyboard down/up shares the material.
        trial
            .domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        trial.settle(false);
        trial.key(true);
        trial.settle(true);
        assert_eq!(trial.clicks.get(), 1);
        trial.key(false);
        trial.settle(false);
        assert_eq!(trial.clicks.get(), 2);
        trial.key(true);
        trial.settle(true);
        handle_window_focus_changed(
            &mut trial.domains,
            &mut trial.router,
            &mut trial.tree,
            WindowId::PRIMARY,
            false,
            trial.renderer.text_engine_mut(),
        )
        .unwrap();
        trial.settle(false);
        assert_eq!(trial.clicks.get(), 2, "focus loss cancels keyboard press");
        // Supersede pending generations faster than Frame A/B can commit them.
        // Every ordinary frame still compiles one authentic committed image.
        for _ in 0..4 {
            trial.pointer(true);
            trial.frame();
            trial.router.cancel_pointer_session_with_tree(
                &mut trial.tree,
                PointerId::MOUSE,
                CancellationReason::OverlayTakeover,
                trial.domains.primary_state_mut(),
            );
            trial.frame();
            trial.pointer(false);
            trial.frame();
        }
        assert_eq!(trial.clicks.get(), 2);
        trial
            .domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        for _ in 0..4 {
            trial.key(true);
            trial.frame();
            trial.key(false);
            trial.frame();
        }
        assert_eq!(trial.clicks.get(), 6);
        let released = trial.settle(false);
        let stable = trial.request();
        trial.frame();
        trial.frame();
        assert_eq!(
            trial.request(),
            stable,
            "settled input never increments revision"
        );
        assert_eq!(
            [root, holder, node, peer].map(|n| (
                trial.tree.node(n).unwrap().id,
                trial.tree.get_absolute_paint_rect(n),
                trial.tree.get_viewport(n)
            )),
            geometry
        );
        assert!(Arc::ptr_eq(
            &peer_local,
            &trial.harness.local_fragment(peer_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &trial.harness.local_fragment(root_id).unwrap()
        ));
        assert!(
            leaves(&trial.harness.local_fragment(owner).unwrap())
                .iter()
                .any(|n| matches!(n,SceneNode::Text(t) if t.text=="Stage" && t.font_size==13.0))
        );
        // Withdrawal removes current E before the final certified resource retirement.
        trial.require_image = false;
        trial
            .tree
            .node_mut(node)
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
        assert!(
            !leaves(&trial.harness.local_fragment(owner).unwrap())
                .iter()
                .any(|node| matches!(node, SceneNode::Image(_)))
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
        for (name, pixels) in [("rest", &resting), ("pressed", &pressed)] {
            use std::io::Write;
            let mut file = std::fs::File::create(directory.join(format!(
                "{}-{}x-{name}.ppm",
                if dark { "dark" } else { "light" },
                scale as u32
            )))
            .unwrap();
            write!(file, "P6\n{} {}\n255\n", width, (100.0 * scale) as u32).unwrap();
            for p in pixels {
                file.write_all(&p[..3]).unwrap();
            }
        }
        println!(
            "V2 {} {}x: rest {:?}, pressed {:?}; uninterrupted real pointer/key transitions, bounded exact generations, cancellation and retirement PASS",
            if dark { "dark" } else { "light" },
            scale,
            rest_face,
            press_face
        );
    }
}
