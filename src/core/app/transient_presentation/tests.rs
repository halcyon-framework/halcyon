use super::*;
use crate::core::animation::Easing;
use crate::core::appearance::*;
use crate::core::layout::Padding;
use crate::core::paint::Color as PaintColor;
use crate::core::scene::Color;
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::renderer::pixel_coverage::PhysicalExtent;
use crate::widgets::{button::Button, flex::Flex, label::Label};

fn limits() -> PresentationLimits {
    crate::core::presentation::tests::limits()
}

struct Fixture {
    tree: WidgetTree,
    root: NodeId,
    target: NodeId,
    reference: NodeId,
    rows: [NodeId; 3],
    harness: RetainedSceneTestHarness,
    text: FontManagerTextEngine,
    theme: Theme,
    domains: InteractionDomains,
    attempts: AppFrameAttempts,
    cause: u64,
}

impl Fixture {
    fn new(enabled: bool) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let reference = tree.add_child(
            root,
            Box::new(Button::with_text("Fixed reference")),
            None,
            0,
        );
        let target = tree.add_child(root, Box::new(Button::with_text("Target")), None, 1);
        let rows = ["Status", "The widest middle diagnostic row", "Help"]
            .map(|text| tree.add_child(root, Box::new(Label::new(text)), None, 2));
        if enabled {
            tree.configure_presentation(PresentationConfiguration {
                owner: tree.widget_id_for_node(root).unwrap(),
                sources: std::iter::once(target)
                    .chain(rows)
                    .map(|node| PresentationSource::Widget(tree.widget_id_for_node(node).unwrap()))
                    .collect(),
                policy: PresentationMotionPolicy {
                    enabled: true,
                    allow_nonessential_motion: true,
                    limits: limits(),
                },
            })
            .unwrap();
        }
        let mut fixture = Self {
            tree,
            root,
            target,
            reference,
            rows,
            harness: RetainedSceneTestHarness::new(),
            text: FontManagerTextEngine::new(),
            theme: Theme::default(),
            domains: InteractionDomains::new(),
            attempts: AppFrameAttempts::new(),
            cause: 0,
        };
        fixture.set_product_appearance(false);
        fixture
    }
    fn source(&self, node: NodeId) -> PresentationSource {
        PresentationSource::Widget(self.tree.widget_id_for_node(node).unwrap())
    }
    fn frame(&mut self, time: f64, executed: bool) -> RetainedSceneTestFrame {
        self.frame_with_text(time, executed, SurfaceId::test_id(1), None)
    }
    fn frame_with_text(
        &mut self,
        time: f64,
        executed: bool,
        surface: SurfaceId,
        text: Option<&mut dyn TextEngine>,
    ) -> RetainedSceneTestFrame {
        self.harness.advance_transition_time(time);
        let attempt = self.attempts.next();
        let target = CustomRenderTargetSnapshot::new(
            surface,
            PhysicalExtent {
                width: 800,
                height: 600,
            },
            1.0,
        )
        .unwrap()
        .with_app_frame_attempt(attempt);
        let frame = self.harness.frame_with_optional_custom_render_target(
            &mut self.tree,
            self.root,
            Constraints {
                min_width: 0.0,
                max_width: 800.0,
                min_height: 0.0,
                max_height: 600.0,
            },
            &self.theme,
            text.unwrap_or(&mut self.text),
            &crate::core::icon::IconRegistry::new(&[]),
            Some(target),
            Some((&self.domains, WindowId::PRIMARY)),
        );
        // Explicit CPU protocol witness, not a native renderer/display claim.
        if executed && frame.scene_built {
            if let Some(runtime) = &mut self.harness.cache.presentation {
                runtime.submitted(attempt, surface, crate::renderer::FrameEpoch::zero());
            }
        }
        frame
    }
    fn replace(
        &mut self,
        product: bool,
        replaces: Option<PresentationGroupId>,
    ) -> PresentationTicket {
        self.cause += 1;
        let request = PresentationRequest {
            cause: self.cause,
            duration_secs: 1.0,
            easing: Easing::Linear,
            composition: PresentationComposition::Exchange(ExchangeRecipe {
                replacement: self.source(self.target),
                displaced: self.rows.map(|node| self.source(node)).to_vec(),
                axis: PresentationAxis::Vertical,
            }),
            replaces,
            settlement: PresentationSettlement::Canonical,
        };
        let ticket = self.tree.request_presentation(request).unwrap();
        self.set_product_appearance(product);
        ticket
    }
    fn set_product_appearance(&mut self, product: bool) {
        let product = product && self.cause != 0;
        let contract = ButtonAppearanceContract::standard();
        let mut values = self.theme.button_appearance_values(&contract);
        // Keep every authored role within the prototype's solid paint profile.
        for paint in [
            &mut values.resting,
            &mut values.hovered,
            &mut values.pressed,
            &mut values.disabled,
            &mut values.selected,
        ]
        .into_iter()
        .flatten()
        {
            if let AppearanceSurface::Gradient(gradient) = &paint.surface {
                paint.surface = AppearanceSurface::Solid(gradient.definition().stops[0].color);
            }
            if let Some(AppearanceSurface::Gradient(gradient)) = &paint.underlay {
                paint.underlay = Some(AppearanceSurface::Solid(gradient.definition().stops[0].color));
            }
        }
        let resting = values.resting.as_mut().unwrap();
        if product {
            values.metrics = Some(ButtonAppearanceMetrics {
                padding: Padding::symmetric(32.0, 24.0),
            });
            resting.surface =
                AppearanceSurface::Solid(PaintColor::from_srgb(0.08, 0.24, 0.65, 1.0));
            resting.radius = 12.0;
            resting.content_color = PaintColor::WHITE;
            resting.border = Some(AppearanceStroke {
                color: PaintColor::WHITE,
                width: 2.0,
            });
        } else {
            resting.surface = AppearanceSurface::Solid(self.theme.colors.controls.background);
        }
        let style = ButtonAppearanceConfiguration::new(
            AppearanceConfigurationId {
                owner: AppearanceConfigurationOwner::Product(ProductAppearanceOwner(1)),
                generation: u64::from(product),
            },
            contract,
            vec![ButtonAppearanceDefinition {
                selector: AppearanceSelector::button(),
                values,
            }],
            vec![],
        )
        .unwrap()
        .prepare_style(
            AppearanceSelector::button(),
            AppearanceFallbackPolicy::Reject,
        )
        .unwrap();
        self.target_button_mut()
            .set_appearance(style, None)
            .unwrap();
    }
    fn target_button_mut(&mut self) -> &mut Button {
        self.tree
            .node_mut(self.target)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Button>()
            .unwrap()
    }
    fn receipt(&self, ticket: PresentationTicket) -> PresentationOutcome {
        self.tree
            .presentation_receipts()
            .iter()
            .find(|value| value.ticket == ticket)
            .unwrap()
            .outcome
    }
}

#[test]
fn transient_presentation_exchange_passes_production_dispatch_and_headless_execution() {
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::scene_diff::{
        BasicCacheConsumer, CacheConsumers, SceneCache, SceneCacheExecutionMode,
        compute_scene_diff_for_cache, dispatch_cache_operations,
    };
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::renderer::{DrawPlanBuilder, mesh_cache::MeshCache};

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("Exchange execution acceptance requires a headless GPU adapter");
    let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
    let surface = compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (800, 600), 1.0)
        .unwrap();
    let device = renderer.device();
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Exchange execution acceptance"),
        size: wgpu::Extent3d {
            width: 800,
            height: 600,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: renderer.surface_format(),
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let mut fixture = Fixture::new(true);
    let mut cache = SceneCache::new();
    let mut consumers = CacheConsumers::new();
    consumers.register(Box::new(BasicCacheConsumer::new()));
    let mut builder = DrawPlanBuilder::new();
    let mut meshes = MeshCache::new();
    let mut previous_keys = None;
    let mut exchange = None;

    for (index, time) in [0.0, 0.0, 0.4, 0.4, 0.9, 1.4].into_iter().enumerate() {
        if index == 1 {
            exchange = Some(fixture.replace(true, None));
        } else if index == 3 {
            exchange = Some(fixture.replace(false, Some(exchange.unwrap().group)));
        }
        // The existing fixture supplies a CPU lifecycle witness. GPU execution
        // below is real, but headless submission is never claimed as native present.
        let frame = fixture.frame_with_text(time, true, surface, Some(renderer.text_engine_mut()));
        assert!(frame.scene_built);
        let mut scene = SceneGraph::new();
        scene.root = frame.scene_root;
        let expected = match index {
            1 | 3 => Some(PresentationOutcome::Reserved),
            2 | 4 => Some(PresentationOutcome::Admitted),
            _ => None,
        };
        if let Some(expected) = expected {
            let ticket = exchange.unwrap();
            assert_eq!(fixture.receipt(ticket), expected, "Exchange frame {index}");
            let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
            let groups = runtime.inspect();
            assert_eq!(groups.len(), 1, "Exchange frame {index}");
            assert_eq!(groups[0].ticket, ticket);
            assert_eq!(groups[0].frame_failure, None, "Exchange frame {index}");
            let mut painted = Vec::new();
            leaves(&scene.root, &mut painted);
            // Ordinary Button text has no key. Occurrence projection qualifies it.
            assert!(
                painted.iter().any(|node| matches!(
                    node, SceneNode::Text(text) if text.text == "Target" && text.key.is_some()
                )),
                "Exchange frame {index} must publish occurrence-qualified Button text"
            );
        }
        let (current_keys, delta) =
            compute_scene_diff_for_cache(previous_keys.as_ref(), &scene, &cache);
        dispatch_cache_operations(
            SceneCacheExecutionMode::Enabled,
            &mut consumers,
            &delta,
            &scene,
            &mut cache,
        );
        cache.assert_snapshot_stable();
        let scenes = HashMap::from([(surface, scene)]);
        let plan = builder
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut meshes,
                |_| 1.0,
                renderer.frame_counter(),
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("complete Exchange target");
        assert_eq!(compiled.target_count(), 1);
        assert!(compiled.has_text());
        assert!(
            compiled
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        let commands = renderer
            .encode_compiled_target_for_test(&compiled, surface, &view)
            .expect("complete Exchange command buffer");
        renderer.submit_compiled_target_for_test(commands);
        assert!(device.poll(wgpu::PollType::wait_indefinitely()).is_ok());
        assert!(pollster::block_on(validation_scope.pop()).is_none());
        renderer.advance_frame();
        previous_keys = Some(current_keys);
    }
}

fn texts(nodes: &[SceneNode]) -> Vec<(String, Point)> {
    let mut result = Vec::new();
    for node in nodes {
        match node {
            SceneNode::Text(text) => result.push((text.text.clone(), text.position)),
            SceneNode::Clip(value) => result.extend(texts(&value.children)),
            SceneNode::RetainedFragment(value) => result.extend(texts(&value.children)),
            _ => {}
        }
    }
    result
}

fn leaves<'a>(nodes: &'a [SceneNode], result: &mut Vec<&'a SceneNode>) {
    for node in nodes {
        match node {
            SceneNode::Clip(value) => leaves(&value.children, result),
            SceneNode::RetainedFragment(value) => leaves(&value.children, result),
            _ => result.push(node),
        }
    }
}

#[test]
fn transient_presentation_standard_gradient_source_refuses_exchange_and_keeps_canonical_paint() {
    let mut fixture = Fixture::new(true);
    fixture.target_button_mut().use_default_appearance();
    fixture.frame(0.0, true);
    let source = fixture.source(fixture.target);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    assert!(matches!(
        runtime.sources[&source],
        Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::SourceProducer
        ))
    ));
    let mut standard_paint = Vec::new();
    leaves(
        &fixture.harness.cache.entries[&source.owner()].local,
        &mut standard_paint,
    );
    let authored_key = standard_paint
        .iter()
        .find_map(|node| match node {
            SceneNode::GradientRect(face) => face.key,
            _ => None,
        })
        .expect("Standard Button must publish a keyed gradient face");

    let ticket = fixture.replace(true, None);
    let frame = fixture.frame(0.0, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::PendingCapability(
            PendingPresentationCapability::SourceProducer
        ))
    );
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    assert!(runtime.publication.as_ref().unwrap().groups.is_empty());
    let mut painted = Vec::new();
    leaves(&frame.scene_root, &mut painted);
    assert!(painted.iter().any(|node| matches!(
        node, SceneNode::RoundedRect(face) if face.key == Some(authored_key)
    )));
    assert!(painted.iter().any(|node| matches!(
        node, SceneNode::Text(text) if text.text == "Target" && text.key.is_none()
    )));
}

#[test]
fn transient_presentation_complete_button_stays_rigid_and_input_stays_canonical() {
    let mut fixture = Fixture::new(true);
    let activations = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = activations.clone();
    fixture.tree.replace_widget_only(
        fixture.target,
        Box::new(Button::with_text("Target").on_click(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })),
    );
    fixture.set_product_appearance(true);
    fixture.frame(0.0, true);
    assert!(
        fixture.harness.cache.presentation.as_ref().unwrap().sources
            [&fixture.source(fixture.target)]
            .is_ok(),
        "authored initial product source must be certified"
    );
    let id = fixture.tree.widget_id_for_node(fixture.target).unwrap();
    let count = fixture.tree.node_count();
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Reserved);
    let canonical = fixture
        .tree
        .get_absolute_paint_rect(fixture.target)
        .unwrap();
    let current = fixture.harness.cache.presentation.as_ref().unwrap().sources
        [&fixture.source(fixture.target)]
        .as_ref()
        .unwrap()
        .clone();
    let mut current_leaves = Vec::new();
    leaves(&current.paint, &mut current_leaves);
    let canonical_text = current_leaves
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(value) => Some(value),
            _ => None,
        })
        .unwrap();
    for time in [0.25, 0.75] {
        let frame = fixture.frame(time, true);
        assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
        assert!(frame.scene_built);
        let mut painted = Vec::new();
        leaves(&frame.scene_root, &mut painted);
        let surface = painted
            .iter()
            .find_map(|node| match node {
                SceneNode::RoundedRect(value)
                    if value.color == Color::from_srgb(0.08, 0.24, 0.65, 1.0) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        let border = painted
            .iter()
            .find_map(|node| match node {
                SceneNode::BorderRect(value)
                    if value.border_width == 2.0 && value.border_color == Color::WHITE =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        let text = painted
            .iter()
            .find_map(|node| match node {
                SceneNode::Text(value) if value.text == "Target" && value.color == Color::WHITE => {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(surface.rect.width, canonical.width);
        assert_eq!(surface.rect.height, canonical.height);
        assert_eq!(surface.radius, 12.0);
        assert_eq!(border.rect, surface.rect);
        assert_eq!(border.radius, 12.0);
        assert_eq!(text.font_size, canonical_text.font_size);
        assert_eq!(
            text.position.x - canonical_text.position.x,
            surface.rect.x - canonical.x
        );
        assert!(
            (text.position.y - canonical_text.position.y - (surface.rect.y - canonical.y)).abs()
                < 0.001
        );
        let hit = fixture
            .tree
            .hit_test(
                canonical.x + canonical.width * 0.5,
                canonical.y + canonical.height * 0.5,
            )
            .unwrap();
        assert_eq!(hit.widget_id, id);
        assert_eq!(
            hit.absolute_bounds,
            (canonical.x, canonical.y, canonical.width, canonical.height)
        );
        assert_eq!(fixture.tree.node_count(), count);
    }
    assert_eq!(activations.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[test]
fn transient_presentation_exchange_uses_one_progress_and_keeps_canonical_layout_and_sibling() {
    let mut fixture = Fixture::new(true);
    let before = fixture.frame(0.0, true);
    let positions = texts(&before.scene_root);
    let reference_id = fixture.tree.widget_id_for_node(fixture.reference).unwrap();
    let ticket = fixture.replace(true, None);
    let start = fixture.frame(0.0, true);
    // The existing canonical layout pass invalidates paint. Subsequent pure
    // presentation frames must retain this unaffected sibling fragment.
    let reference = fixture.harness.cache.entries[&reference_id].local.clone();
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Reserved);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    let group = &runtime.publication.as_ref().unwrap().groups[0];
    assert_eq!(group.progress, 0.0);
    assert!(group.first);
    assert_eq!(
        texts(&start.scene_root)
            .iter()
            .find(|(text, _)| text == "Status")
            .unwrap()
            .1,
        positions
            .iter()
            .find(|(text, _)| text == "Status")
            .unwrap()
            .1
    );
    let canonical = fixture
        .tree
        .get_absolute_paint_rect(fixture.rows[0])
        .unwrap();
    let middle = fixture.frame(0.5, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
    let moved = texts(&middle.scene_root);
    let old_y = positions
        .iter()
        .find(|(text, _)| text == "Status")
        .unwrap()
        .1
        .y;
    let mid_y = moved.iter().find(|(text, _)| text == "Status").unwrap().1.y;
    assert!(mid_y > old_y);
    assert_eq!(
        fixture
            .tree
            .get_absolute_paint_rect(fixture.rows[0])
            .unwrap(),
        canonical
    );
    assert!(Arc::ptr_eq(
        &fixture.harness.cache.entries[&reference_id].local,
        &reference
    ));
    assert!(
        fixture
            .harness
            .interpolated_rect(fixture.tree.widget_id_for_node(fixture.target).unwrap())
            .is_none()
    );
    fixture.frame(1.0, true);
    fixture.frame(1.01, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Completed);
}

#[test]
fn transient_presentation_unused_does_not_allocate_custody_or_rebuild_clean_scene() {
    let mut fixture = Fixture::new(false);
    fixture.frame(0.0, true);
    assert!(fixture.tree.presentation_inbox.is_none());
    assert!(fixture.harness.cache.presentation.is_none());
    assert!(!fixture.frame(0.0, true).scene_built);
}

#[test]
fn transient_presentation_unexecuted_first_attempt_cannot_supply_a_continuation() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, false);
    fixture.frame(0.5, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::UnavailableResource)
    );
    assert!(
        fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .publication
            .as_ref()
            .unwrap()
            .groups
            .is_empty()
    );
}

#[test]
fn transient_presentation_reverse_rebases_executed_values_without_rewriting_layout_history() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let forward = fixture.replace(true, None);
    fixture.frame(0.0, true);
    let before_reverse = fixture.frame(0.4, true);
    let reverse = fixture.replace(false, Some(forward.group));
    let reverse_start = fixture.frame(0.4, true);
    assert_eq!(
        texts(&before_reverse.scene_root),
        texts(&reverse_start.scene_root)
    );
    fixture.frame(0.9, true);
    assert_eq!(fixture.receipt(reverse), PresentationOutcome::Admitted);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    assert_eq!(
        runtime.publication.as_ref().unwrap().groups[0].progress,
        0.5
    );
    fixture.frame(1.4, true);
    fixture.frame(1.5, true);
    assert_eq!(fixture.receipt(reverse), PresentationOutcome::Completed);
}

#[test]
fn transient_presentation_focus_change_settles_complete_group_with_current_cue() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    let owner = fixture.tree.widget_id_for_node(fixture.target).unwrap();
    fixture
        .domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    fixture.frame(0.3, true);
    fixture.frame(0.4, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::RefusedCue)
    );
}

#[test]
fn transient_presentation_idle_custody_reuses_certificates_when_an_unrelated_owner_repaints() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    let visits = runtime.capture_visits;
    let generation = runtime.sources[&fixture.source(fixture.target)]
        .as_ref()
        .unwrap()
        .clone();
    fixture.tree.mark_paint_dirty(fixture.reference);
    fixture.frame(0.1, true);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    assert_eq!(runtime.capture_visits, visits);
    assert!(Arc::ptr_eq(
        &generation,
        runtime.sources[&fixture.source(fixture.target)]
            .as_ref()
            .unwrap()
    ));
}

fn certify_font_source(
    fixture: &Fixture,
    node: NodeId,
    font: Option<&TextGenerationLease>,
    previous: Option<&Arc<CertifiedSource>>,
    limits: PresentationLimits,
    work: &mut usize,
) -> Result<Arc<CertifiedSource>, PresentationFailure> {
    certify_source(
        &fixture.tree,
        &fixture.harness.cache,
        fixture.source(node),
        fixture.tree.widget_id_for_node(fixture.root).unwrap(),
        fixture.root,
        font,
        1.0,
        &fixture.harness.cache.interpolated_rects,
        fixture.harness.cache.appearance.as_ref(),
        limits,
        work,
        previous,
    )
}

#[test]
fn transient_presentation_font_non_text_source_has_no_lease_or_generation_dependency() {
    let mut fixture = Fixture::new(false);
    let mut button = Button::with_text("");
    button.text = None;
    fixture
        .tree
        .replace_widget_only(fixture.target, Box::new(button));
    fixture.set_product_appearance(false);
    fixture.frame(0.0, false);
    let bytes = Arc::new(vec![0; 1024]);
    let weak = Arc::downgrade(&bytes);
    let font = TextGenerationLease::new(1, bytes.clone());
    let owners = Arc::strong_count(&bytes);
    let mut limits = limits();
    limits.font_bytes = 0;
    let source =
        certify_font_source(&fixture, fixture.target, Some(&font), None, limits, &mut 0).unwrap();
    assert!(texts(&source.paint).is_empty());
    assert_eq!(source.charge.font_bytes, 0);
    assert!(
        source.font.is_none(),
        "non-text source retained a font lease"
    );
    assert_eq!(Arc::strong_count(&bytes), owners);

    let replacement = TextGenerationLease::new(2, Arc::new(vec![0; 2048]));
    for font in [Some(&replacement), None] {
        let mut work = 0;
        let reused = certify_font_source(
            &fixture,
            fixture.target,
            font,
            Some(&source),
            limits,
            &mut work,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&source, &reused));
        assert_eq!(work, 0, "unconsumed font generation caused recapture");
    }
    drop(font);
    drop(bytes);
    assert!(weak.upgrade().is_none());
}

#[test]
fn transient_presentation_font_text_source_retains_charges_and_tracks_exact_generation() {
    let mut fixture = Fixture::new(false);
    fixture.frame(0.0, false);
    let bytes = Arc::new(vec![0; 1024]);
    let weak = Arc::downgrade(&bytes);
    let font = TextGenerationLease::new(1, bytes.clone());
    let owners = Arc::strong_count(&bytes);
    let source = certify_font_source(
        &fixture,
        fixture.rows[1],
        Some(&font),
        None,
        limits(),
        &mut 0,
    )
    .unwrap();
    assert!(!texts(&source.paint).is_empty());
    assert!(source.font.as_ref().unwrap().same_generation(&font));
    assert_eq!(source.charge.font_bytes, font.retained_bytes());
    assert_eq!(Arc::strong_count(&bytes), owners + 1);

    let replacement = TextGenerationLease::new(2, Arc::new(vec![0; 2048]));
    let replaced = certify_font_source(
        &fixture,
        fixture.rows[1],
        Some(&replacement),
        Some(&source),
        limits(),
        &mut 0,
    )
    .unwrap();
    assert_ne!(source.generation, replaced.generation);
    assert!(
        replaced
            .font
            .as_ref()
            .unwrap()
            .same_generation(&replacement)
    );
    assert_eq!(replaced.charge.font_bytes, replacement.retained_bytes());
    assert!(matches!(
        certify_font_source(
            &fixture,
            fixture.rows[1],
            None,
            Some(&source),
            limits(),
            &mut 0,
        ),
        Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::TextGeneration
        ))
    ));
    drop(font);
    drop(bytes);
    assert!(weak.upgrade().is_some());
    drop(source);
    assert!(weak.upgrade().is_none());
}

#[test]
fn transient_presentation_font_text_to_non_text_replacement_keeps_historical_backing() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let source_address = fixture.source(fixture.target);
    let historical = fixture.harness.cache.presentation.as_ref().unwrap().sources[&source_address]
        .as_ref()
        .unwrap()
        .clone();
    assert!(historical.font.is_some());
    let ticket = fixture.replace(true, None);
    fixture
        .tree
        .node_mut(fixture.target)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Button>()
        .unwrap()
        .text = None;
    fixture.frame(0.0, true);
    let current = fixture.harness.cache.presentation.as_ref().unwrap().sources[&source_address]
        .as_ref()
        .unwrap();
    assert!(current.font.is_none());
    assert_eq!(current.charge.font_bytes, 0);
    let middle = fixture.frame(0.5, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
    assert!(
        texts(&middle.scene_root)
            .iter()
            .any(|(text, _)| text == "Target")
    );
}

#[test]
fn transient_presentation_subtree_certifies_descendant_scope_without_new_widget_identity() {
    let mut fixture = Fixture::new(false);
    fixture
        .tree
        .replace_widget_only(fixture.target, Box::new(Flex::column().with_spacing(4.0)));
    fixture
        .tree
        .add_child(fixture.target, Box::new(Label::new("One")), None, 0);
    fixture
        .tree
        .add_child(fixture.target, Box::new(Label::new("Two")), None, 1);
    let replacement =
        PresentationSource::Subtree(fixture.tree.widget_id_for_node(fixture.target).unwrap());
    let displaced: Vec<_> = fixture.rows.map(|node| fixture.source(node)).into();
    fixture
        .tree
        .configure_presentation(PresentationConfiguration {
            owner: fixture.tree.widget_id_for_node(fixture.root).unwrap(),
            sources: std::iter::once(replacement)
                .chain(displaced.iter().copied())
                .collect(),
            policy: PresentationMotionPolicy {
                enabled: true,
                allow_nonessential_motion: true,
                limits: limits(),
            },
        })
        .unwrap();
    fixture.frame(0.0, true);
    let node_count = fixture.tree.node_count();
    let ticket = fixture
        .tree
        .request_presentation(PresentationRequest {
            cause: 1,
            composition: PresentationComposition::Exchange(ExchangeRecipe {
                replacement,
                displaced,
                axis: PresentationAxis::Vertical,
            }),
            duration_secs: 1.0,
            easing: Easing::Linear,
            replaces: None,
            settlement: PresentationSettlement::Canonical,
        })
        .unwrap();
    fixture
        .tree
        .node_mut(fixture.target)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Flex>()
        .unwrap()
        .set_spacing(30.0);
    fixture.tree.mark_layout_dirty(fixture.target);
    fixture.frame(0.0, true);
    let middle = fixture.frame(0.5, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
    assert_eq!(fixture.tree.node_count(), node_count);
    assert_eq!(
        texts(&middle.scene_root)
            .iter()
            .filter(|(text, _)| text == "One")
            .count(),
        2
    );
}

#[test]
fn transient_presentation_live_ancestor_clip_remains_at_its_current_child_scope() {
    let mut fixture = Fixture::new(true);
    fixture.tree.replace_widget_only(
        fixture.root,
        Box::new(
            Flex::column()
                .with_spacing(12.0)
                .with_background(crate::core::paint::Color::from_srgb(0.1, 0.2, 0.3, 1.0)),
        ),
    );
    fixture.tree.set_overflow(fixture.root, OverflowMode::Clip);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    let middle = fixture.frame(0.5, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
    let SceneNode::RetainedFragment(root) = &middle.scene_root[0] else {
        panic!("root scope")
    };
    let clip = root
        .children
        .iter()
        .find_map(|node| match node {
            SceneNode::Clip(clip) => Some(clip),
            _ => None,
        })
        .unwrap();
    assert_eq!(clip.rect, fixture.tree.get_viewport(fixture.root).unwrap());
    assert!(
        matches!(&root.children[0], SceneNode::RetainedFragment(_)),
        "local paint remains before the child-only barrier"
    );
}

#[test]
fn transient_presentation_completion_releases_historical_generations_and_handles() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let old = Arc::downgrade(
        fixture.harness.cache.presentation.as_ref().unwrap().sources
            [&fixture.source(fixture.target)]
            .as_ref()
            .unwrap(),
    );
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    assert!(old.upgrade().is_some());
    fixture.frame(1.0, true);
    fixture.frame(1.1, true);
    assert_eq!(fixture.receipt(ticket), PresentationOutcome::Completed);
    assert!(old.upgrade().is_none());
    assert!(!fixture.harness.transition_layer.has_active_animations());
}

#[test]
fn transient_presentation_zero_duration_replacement_cancels_the_old_interval() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let old = fixture.replace(true, None);
    fixture.frame(0.0, true);
    fixture.frame(0.2, true);
    let replacement = fixture
        .tree
        .request_presentation(PresentationRequest {
            cause: 2,
            composition: PresentationComposition::Projections(vec![PresentationProjection {
                source: fixture.source(fixture.target),
                transform: PresentationTransform::Identity,
                mask: PresentationMask::None,
                isolated_opacity: None,
            }]),
            duration_secs: 0.0,
            easing: Easing::Linear,
            replaces: Some(old.group),
            settlement: PresentationSettlement::Canonical,
        })
        .unwrap();
    fixture.frame(0.2, true);
    assert_eq!(
        fixture.receipt(replacement),
        PresentationOutcome::SettledCanonical
    );
    assert!(
        fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .publication
            .as_ref()
            .unwrap()
            .groups
            .is_empty()
    );
    assert!(!fixture.harness.transition_layer.has_active_animations());
}

#[test]
fn transient_presentation_repeated_reverse_is_bounded_and_does_not_renew_historical_age() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let mut ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    let mut time = 0.0;
    for index in 0..16 {
        time += 0.1;
        fixture.frame(time, true);
        assert_eq!(fixture.receipt(ticket), PresentationOutcome::Admitted);
        let inspection = fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .inspect();
        let value = &inspection[0];
        assert_eq!(value.ticket, ticket);
        assert_eq!(value.cause, fixture.cause);
        assert_eq!(value.policy_revision, 1);
        assert!(!value.reserved);
        assert!(value.progress > 0.0 && value.progress < 1.0);
        assert_eq!(value.previous_canonical.len(), 4);
        assert_eq!(value.current_canonical.len(), 4);
        assert_eq!(value.occurrences.len(), 5);
        assert!(value.charge.within(limits()));
        assert!(value.frame_failure.is_none());
        let execution = value.execution.unwrap();
        assert_eq!(execution.surface, SurfaceId::test_id(1));
        assert!(
            execution.attempt.sequence() > value.continuation_execution.unwrap().attempt.sequence()
        );
        assert_eq!(execution.epoch, crate::renderer::FrameEpoch::zero());
        if index == 15 {
            eprintln!("Exchange measured conservative peak: {:?}", value.charge);
        }
        ticket = fixture.replace(index % 2 != 0, Some(ticket.group));
        fixture.frame(time, true);
    }
    fixture.frame(4.001, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::SettledCanonical
    );
    assert!(
        fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .inspect()
            .is_empty()
    );
    assert!(!fixture.harness.transition_layer.has_active_animations());
}

#[test]
fn transient_presentation_policy_disable_and_owner_removal_retire_without_authority_mutation() {
    for remove in [false, true] {
        let mut fixture = Fixture::new(true);
        fixture.frame(0.0, true);
        let ticket = fixture.replace(true, None);
        fixture.frame(0.0, true);
        let focused = fixture.domains.primary_state().focus_manager.focused();
        if remove {
            // Production EventRouter teardown still owns focus/input cleanup.
            // This exercises only the presentation owner's read-side retirement.
            fixture.tree.remove_node(fixture.target);
        } else {
            let mut configuration = fixture
                .tree
                .presentation_inbox
                .as_ref()
                .unwrap()
                .configuration
                .clone();
            configuration.policy.enabled = false;
            fixture.tree.configure_presentation(configuration).unwrap();
        }
        fixture.frame(0.3, true);
        assert!(
            fixture
                .harness
                .cache
                .presentation
                .as_ref()
                .unwrap()
                .inspect()
                .is_empty()
        );
        assert!(!fixture.harness.transition_layer.has_active_animations());
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            focused
        );
        assert!(matches!(
            fixture.receipt(ticket),
            PresentationOutcome::Cancelled | PresentationOutcome::SettledCanonical
        ));
    }
}

#[test]
fn transient_presentation_removed_custody_retires_in_update_without_paint() {
    for remove_configuration_owner in [false, true] {
        let mut fixture = Fixture::new(true);
        fixture.frame(0.0, true);
        let source = fixture.source(fixture.target);
        let sibling = fixture.source(fixture.rows[0]);
        let historical = Arc::downgrade(
            fixture.harness.cache.presentation.as_ref().unwrap().sources[&source]
                .as_ref()
                .unwrap(),
        );
        let ticket = fixture.replace(true, None);
        fixture.frame(0.0, true);
        let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
        let current = Arc::downgrade(runtime.sources[&source].as_ref().unwrap());
        let unchanged = Arc::downgrade(runtime.sources[&sibling].as_ref().unwrap());
        let focused = fixture.domains.primary_state().focus_manager.focused();

        // EventRouter owns authoritative teardown; this proves read-side custody retirement.
        fixture.tree.remove_node(if remove_configuration_owner {
            fixture.root
        } else {
            fixture.target
        });
        fixture.harness.advance_transition_time(0.3);
        {
            let _update_authority = UpdateAuthorityGuard::enter();
            let harness = &mut fixture.harness;
            harness.cache.presentation.as_mut().unwrap().update(
                &mut fixture.tree,
                &mut harness.transition_layer,
                &mut harness.active_layout_transitions,
                300,
                true,
            );
        }

        let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
        assert!(runtime.inspect().is_empty());
        assert!(!fixture.harness.transition_layer.has_active_animations());
        assert_eq!(
            fixture.receipt(ticket),
            PresentationOutcome::SettledCanonical
        );
        assert!(historical.upgrade().is_none());
        assert!(
            current.upgrade().is_none(),
            "removed current certificates must retire without another paint"
        );
        assert!(!runtime.sources.contains_key(&source));
        if remove_configuration_owner {
            assert!(runtime.sources.is_empty());
            assert!(unchanged.upgrade().is_none());
        } else {
            assert!(Arc::ptr_eq(
                &unchanged.upgrade().unwrap(),
                runtime.sources[&sibling].as_ref().unwrap(),
            ));
        }
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            focused,
        );
    }
}

#[test]
fn transient_presentation_wrong_execution_receipt_never_activates() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, false);
    let wrong_attempt = fixture.attempts.next();
    fixture
        .harness
        .cache
        .presentation
        .as_mut()
        .unwrap()
        .submitted(
            wrong_attempt,
            SurfaceId::test_id(2),
            crate::renderer::FrameEpoch::zero(),
        );
    fixture.frame(0.2, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::UnavailableResource)
    );
}

#[test]
fn transient_presentation_missing_target_expires_instead_of_joining_a_later_frame() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.harness.frame_with_appearance(
        &mut fixture.tree,
        fixture.root,
        Constraints {
            min_width: 0.0,
            max_width: 800.0,
            min_height: 0.0,
            max_height: 600.0,
        },
        &fixture.theme,
        &mut fixture.text,
        &crate::core::icon::IconRegistry::new(&[]),
        &fixture.domains,
        WindowId::PRIMARY,
    );
    fixture.frame(0.5, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::PendingCapability(
            PendingPresentationCapability::ExecutionTarget
        ))
    );
    assert!(
        fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .inspect()
            .is_empty()
    );
}

#[test]
fn transient_presentation_window_suspension_retires_without_restoring_motion_on_return() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    fixture.frame(0.0, true);
    fixture.domains.set_active_window(None);
    fixture.frame(0.3, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::SettledCanonical
    );
    fixture.domains.set_active_window(Some(WindowId::PRIMARY));
    fixture.frame(0.6, true);
    assert!(
        fixture
            .harness
            .cache
            .presentation
            .as_ref()
            .unwrap()
            .inspect()
            .is_empty()
    );
    assert!(!fixture.harness.transition_layer.has_active_animations());
}

#[test]
fn transient_presentation_first_candidate_budget_failure_is_atomic() {
    let mut fixture = Fixture::new(true);
    let mut configuration = fixture
        .tree
        .presentation_inbox
        .as_ref()
        .unwrap()
        .configuration
        .clone();
    configuration.policy.limits.occurrences = 4; // Exchange requires five complete occurrences.
    fixture.tree.configure_presentation(configuration).unwrap();
    fixture.frame(0.0, true);
    let ticket = fixture.replace(true, None);
    let frame = fixture.frame(0.0, true);
    assert_eq!(
        fixture.receipt(ticket),
        PresentationOutcome::Refused(PresentationFailure::RefusedBudget)
    );
    assert_eq!(
        texts(&frame.scene_root)
            .iter()
            .filter(|(value, _)| value == "Target")
            .count(),
        1
    );
    assert!(!fixture.harness.transition_layer.has_active_animations());
}

#[test]
fn transient_presentation_peak_depth_includes_occurrence_mask_scopes() {
    let mut fixture = Fixture::new(true);
    fixture.frame(0.0, true);
    fixture.replace(true, None);
    fixture.frame(0.0, true);
    let runtime = fixture.harness.cache.presentation.as_ref().unwrap();
    let group = &runtime.publication.as_ref().unwrap().groups[0];
    let mut limits = limits();
    limits.depth = 1; // Source leaves fit; the additional occurrence/mask scopes do not.
    assert!(matches!(
        prepare_group(
            group,
            &runtime.sources,
            fixture.text.presentation_text_generation().as_ref(),
            RectF::new(0.0, 0.0, 800.0, 600.0),
            limits
        ),
        Err(PresentationFailure::RefusedBudget)
    ));
}
