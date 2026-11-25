use super::*;

struct ButtonAppearanceFixture {
    tree: WidgetTree,
    nodes: [NodeId; 4],
    layouts: Rc<Cell<u32>>,
    clicks: Rc<Cell<u32>>,
    domains: crate::core::event::InteractionDomains,
    harness: RetainedSceneTestHarness,
    text: FontManagerTextEngine,
    theme: Theme,
}

impl ButtonAppearanceFixture {
    fn new() -> Self {
        let mut tree = WidgetTree::new();
        let layouts = Rc::new(Cell::new(0));
        let clicks = Rc::new(Cell::new(0));
        let root = tree.create_root(Box::new(LayoutProbe::new(layouts.clone())), None, 0);
        let count = clicks.clone();
        let button = crate::widgets::button::Button::with_text("Run")
            .on_click(move || count.set(count.get() + 1));
        let nodes = [
            root,
            tree.add_child(root, Box::new(button), None, 0),
            tree.add_child(
                root,
                Box::new(crate::widgets::button::Button::with_text("Other")),
                None,
                0,
            ),
            tree.add_child(root, Box::new(LayoutProbe::new(Rc::default())), None, 0),
        ];
        Self {
            tree,
            nodes,
            layouts,
            clicks,
            domains: crate::core::event::InteractionDomains::new(),
            harness: RetainedSceneTestHarness::new(),
            text: FontManagerTextEngine::new(),
            theme: Theme::default(),
        }
    }

    fn owner(&self, index: usize) -> crate::utils::id::TreeWidgetId {
        self.tree.widget_id_for_node(self.nodes[index]).unwrap()
    }

    fn button(&self) -> &crate::widgets::button::Button {
        self.tree
            .node(self.nodes[1])
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    fn button_mut(&mut self) -> &mut crate::widgets::button::Button {
        self.tree
            .node_mut(self.nodes[1])
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn event(
        &mut self,
        f: impl FnOnce(&mut crate::widgets::button::Button, &mut crate::core::widget::EventContext),
    ) {
        let mut cx = crate::core::widget::EventContext::new(
            self.owner(1),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );
        f(self.button_mut(), &mut cx);
    }

    fn frame(&mut self) -> crate::core::app::RetainedSceneTestFrame {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.nodes[0],
            Constraints {
                min_width: 0.0,
                max_width: 320.0,
                min_height: 0.0,
                max_height: 180.0,
            },
            &self.theme,
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }
}

pub(super) fn button_product_values(
    contract: &crate::core::appearance::ButtonAppearanceContract,
    padding: crate::core::layout::Padding,
) -> crate::core::appearance::ButtonAppearanceValues {
    use crate::core::appearance::*;
    let mut values = Theme::default().button_appearance_values(contract);
    values.metrics = Some(ButtonAppearanceMetrics { padding });
    let paint = ButtonPaintValues {
        surface: AppearanceSurface::Solid(Color::from_hex(0x542D82)),
        radius: 11.0,
        content_color: Color::from_hex(0xF5E7C5),
        underlay: Some(AppearanceSurface::Solid(Color::from_hex(0x302030))),
        border: Some(AppearanceStroke {
            color: Color::from_hex(0xCE655A),
            width: 2.5,
        }),
        overlay: Some(AppearanceStroke {
            color: Color::from_hex(0xA2724E),
            width: 1.25,
        }),
    };
    values.resting = Some(paint.clone());
    values.hovered = Some(ButtonPaintValues {
        surface: AppearanceSurface::Solid(Color::from_hex(0x8350B4)),
        ..paint.clone()
    });
    values.pressed = Some(ButtonPaintValues {
        surface: AppearanceSurface::Solid(Color::from_hex(0x301743)),
        ..paint.clone()
    });
    values.disabled = Some(ButtonPaintValues {
        content_color: Color::from_hex(0xAAAAAA),
        ..paint.clone()
    });
    values.selected = contract.selection_owner().map(|_| ButtonPaintValues {
        surface: AppearanceSurface::Solid(Color::from_hex(0x276657)),
        ..paint
    });
    values.focus = Some(AppearanceStroke {
        color: Color::from_hex(0x9ADF36),
        width: 3.0,
    });
    values
}

pub(super) fn button_product_style(
    contract: crate::core::appearance::ButtonAppearanceContract,
    values: crate::core::appearance::ButtonAppearanceValues,
    generation: u64,
) -> Result<
    crate::core::appearance::PreparedButtonStyle,
    crate::core::appearance::UnresolvedButtonStyle,
> {
    use crate::core::appearance::*;
    let selector = contract.selectors()[0].clone();
    ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Product(ProductAppearanceOwner(77)),
            generation,
        },
        contract,
        vec![ButtonAppearanceDefinition {
            selector: selector.clone(),
            values,
        }],
        vec![],
    )
    .unwrap()
    .prepare_style(selector, AppearanceFallbackPolicy::Reject)
}

fn button_scene_nodes(
    frame: &crate::core::app::RetainedSceneTestFrame,
) -> Vec<&crate::core::scene::SceneNode> {
    use crate::core::scene::SceneNode;
    fn flatten<'a>(node: &'a SceneNode, out: &mut Vec<&'a SceneNode>) {
        match node {
            SceneNode::RetainedFragment(fragment) => {
                for child in fragment.children.iter() {
                    flatten(child, out);
                }
            }
            _ => out.push(node),
        }
    }
    let [SceneNode::RetainedFragment(root)] = frame.scene_root.as_slice() else {
        panic!("expected retained root");
    };
    let mut out = Vec::new();
    flatten(&root.children[1], &mut out);
    out
}

#[test]
fn button_appearance_surface_fill_gradient_reuses_identity_layout_and_siblings() {
    use crate::core::appearance::*;
    use crate::core::paint::{ColorStop, GradientDef, GradientKind, SurfaceGradient};
    use crate::core::scene::SceneNode;
    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    let before = f.frame();
    let metrics = f.button().resolved_appearance().unwrap().metrics();
    let contract = ButtonAppearanceContract::standard();
    let mut values = button_product_values(&contract, metrics.padding);
    let gradient = SurfaceGradient::new(GradientDef {
        kind: GradientKind::Linear,
        stops: vec![
            ColorStop {
                offset: 0.0,
                color: Color::from_hex(0x542D82),
            },
            ColorStop {
                offset: 1.0,
                color: Color::from_hex(0x8350B4),
            },
        ],
        p0: (0.0, 0.0),
        p1: (0.0, 1.0),
    })
    .unwrap();
    values.resting.as_mut().unwrap().surface = AppearanceSurface::Gradient(gradient.clone());
    f.button_mut()
        .set_appearance(button_product_style(contract, values, 8).unwrap(), None)
        .unwrap();
    let after = f.frame();
    assert_focus_appearance_recomposition(&before, &after, &[1]);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.owner(1), owner);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(f.clicks.get(), 0);
    let nodes = button_scene_nodes(&after);
    let surfaces: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::GradientRect(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(surfaces.len(), 1);
    assert_eq!(&surfaces[0].gradient, gradient.definition());
    assert_eq!(surfaces[0].radius, 11.0);
    let inspection = f.button().inspect_appearance();
    let surface = inspection
        .layers
        .iter()
        .find(|layer| layer.role == AppearanceRole::Surface)
        .unwrap();
    assert_eq!(
        surface.profile,
        crate::widgets::button::ButtonAppearancePrimitiveProfile::GradientRoundedRect
    );
    assert_eq!(surface.z_index, surfaces[0].z_index);
    assert!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .framework()
            .focused()
    );
    let unchanged = f.frame();
    assert!(!unchanged.scene_built);
    assert_eq!(f.layouts.get(), 1);
}

#[test]
fn image_admission_button_retains_generation_and_invalidates_only_its_consumers() {
    use crate::core::appearance::*;
    use crate::core::scene::{ImageTextureSource, SceneNode};
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(263_250, 1, 1, &[255, 0, 0, 255]);
    let binding = renderer.admit_image(263_250).unwrap();
    let mut f = ButtonAppearanceFixture::new();
    let before = f.frame();
    let contract = ButtonAppearanceContract::standard();
    let mut values = button_product_values(
        &contract,
        f.button().resolved_appearance().unwrap().metrics().padding,
    );
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
        paint.radius = 0.0;
        paint.surface = AppearanceSurface::Image(AppearanceImage {
            source: AppearanceImageSource::Admitted(binding.clone()),
            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
            tint: Color::from_srgb(0.8, 0.4, 0.2, 0.5),
        });
    }
    assert_eq!(f.harness.image_dependency_counts(), None);
    f.button_mut()
        .set_appearance(
            button_product_style(contract.clone(), values.clone(), 10).unwrap(),
            None,
        )
        .unwrap();
    let after = f.frame();
    assert_eq!(f.harness.image_dependency_counts(), Some((1, 1)));
    assert_focus_appearance_recomposition(&before, &after, &[1]);
    assert_eq!(f.layouts.get(), 1);
    let image = button_scene_nodes(&after)
        .into_iter()
        .find_map(|node| match node {
            SceneNode::Image(image) => Some(image),
            _ => None,
        })
        .expect("ordinary Button must emit its admitted Image");
    assert_eq!(
        image.texture_source,
        ImageTextureSource::Admitted(binding.identity())
    );
    assert_eq!(image.tint, Color::from_srgb(0.8, 0.4, 0.2, 0.5).into());

    let bounds = RectF::new(0.0, 0.0, 320.0, 180.0);
    let mut compositor = crate::compositor::Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, crate::compositor::SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (320, 180), 1.0)
        .unwrap();
    let mut scene = crate::core::scene::SceneGraph::new();
    scene.root = after.scene_root.clone();
    let scenes = std::collections::HashMap::from([(surface, scene)]);
    let plan = crate::renderer::DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut crate::renderer::mesh_cache::MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    assert!(
        renderer.compile_multi(&plan).unwrap().is_ready(),
        "actual Button scene must reach the admitted renderer path"
    );
    assert!(!f.frame().scene_built);

    renderer.unregister_image(263_250);
    let previous = f.button().resolved_appearance().unwrap().clone();
    let same_generation = f.button_mut().set_appearance(
        button_product_style(contract.clone(), values.clone(), 10).unwrap(),
        None,
    );
    assert!(
        matches!(same_generation,
            Err(crate::widgets::button::ButtonAppearanceError::ImageResourceUnavailable {
                identity, failure: crate::renderer::image_binding::ImageBindingFailure::Unregistered,
            }) if identity == binding.identity()
        ),
        "unchanged input must not bypass current resource admission"
    );
    let rejected = f.button_mut().set_appearance(
        button_product_style(contract.clone(), values, 11).unwrap(),
        None,
    );
    assert!(
        matches!(rejected, Err(crate::widgets::button::ButtonAppearanceError::ImageResourceUnavailable {
        identity, failure: crate::renderer::image_binding::ImageBindingFailure::Unregistered,
    }) if identity == binding.identity())
    );
    assert_eq!(f.button().resolved_appearance(), Ok(&previous));
    let invalidated = f.frame();
    assert!(
        invalidated.scene_built,
        "resource revocation must invalidate its retained consumer before reuse"
    );
    assert_focus_appearance_recomposition(&after, &invalidated, &[1]);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.clicks.get(), 0);
    assert!(
        !f.frame().scene_built,
        "terminal resource loss must not create polling/animation demand"
    );

    let solid = button_product_values(&contract, previous.metrics().padding);
    f.button_mut()
        .set_appearance(button_product_style(contract, solid, 12).unwrap(), None)
        .unwrap();
    f.frame();
    assert_eq!(
        f.harness.image_dependency_counts(),
        None,
        "replacing the last Image consumer retires its dependency storage"
    );
    assert_eq!(f.layouts.get(), 1);
    assert!(!f.frame().scene_built);
}

#[test]
fn nine_patch_button_admission_replacement_and_retained_generation_lifetime() {
    use crate::core::appearance::*;
    use crate::core::event::PointerId;
    use crate::core::paint::{
        LAYER_CONTENT, NinePatchDestinationInsets, NinePatchSourceInsets, SurfaceNinePatch,
    };
    use crate::core::scene::{ImageTextureSource, SceneNode};
    use crate::core::scene_identity::SceneNodeKey;
    use crate::renderer::image_binding::ImageBindingFailure;
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::widgets::button::{ButtonAppearanceError, ButtonAppearancePrimitiveProfile};

    const IMAGE: u64 = 19_250;
    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("headless renderer required for the admitted NinePatch proof");
    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[255, 0, 0, 255]);
    let binding = renderer.admit_image(IMAGE).unwrap();
    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    let widget_id = f.button().id();
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    f.domains
        .primary_state_mut()
        .pointer_capture_manager
        .request_capture(PointerId::MOUSE, owner)
        .unwrap();
    let before = f.frame();
    let previous = f.button().resolved_appearance().unwrap().clone();
    assert!(previous.request().input.framework().focused());
    assert!(previous.request().input.framework().pointer_captured());
    assert!(!previous.request().input.framework().keyboard_captured());
    let rect = f.tree.get_rect(f.nodes[1]).unwrap();
    let contract = ButtonAppearanceContract::standard();
    let padding = previous.metrics().padding;
    let tint = Color::from_srgb(0.8, 0.4, 0.2, 0.5);
    let configured = |source: AppearanceImageSource, insets, tint| {
        let material = SurfaceNinePatch::new(
            source,
            RectF::new(0.125, 0.125, 0.5, 0.5),
            NinePatchSourceInsets::new(0.25, 0.125, 0.125, 0.25),
            insets,
            tint,
        )
        .unwrap();
        let mut values = button_product_values(&contract, padding);
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
            paint.radius = 0.0;
            paint.surface = AppearanceSurface::NinePatch(material.clone());
            paint.underlay = Some(AppearanceSurface::NinePatch(material.clone()));
        }
        values
    };
    let insets = NinePatchDestinationInsets::new(2.0, 3.0, 4.0, 5.0);
    let unadmitted = configured(
        AppearanceImageSource::CurrentBinding { texture_id: IMAGE },
        insets,
        tint,
    );
    assert!(matches!(
        f.button_mut().set_appearance(
            button_product_style(contract.clone(), unadmitted, 20).unwrap(),
            None,
        ),
        Err(ButtonAppearanceError::PendingCapabilityAdmission {
            surface: AppearanceSurface::NinePatch(_),
        })
    ));
    assert_eq!(f.button().resolved_appearance(), Ok(&previous));
    assert!(!f.frame().scene_built);

    let values = configured(
        AppearanceImageSource::Admitted(binding.clone()),
        insets,
        tint,
    );
    let mut rounded = values.clone();
    // Reject a non-current used state before publishing any of this replacement.
    rounded.disabled.as_mut().unwrap().radius = 1.0;
    assert!(matches!(
        f.button_mut().set_appearance(
            button_product_style(contract.clone(), rounded, 20).unwrap(),
            None,
        ),
        Err(ButtonAppearanceError::PendingCapabilityAdmission {
            surface: AppearanceSurface::NinePatch(_),
        })
    ));
    assert_eq!(f.button().resolved_appearance(), Ok(&previous));
    assert!(!f.frame().scene_built);
    assert_eq!(f.harness.image_dependency_counts(), None);

    f.button_mut()
        .set_appearance(
            button_product_style(contract.clone(), values, 20).unwrap(),
            None,
        )
        .unwrap();
    let after = f.frame();
    assert_focus_appearance_recomposition(&before, &after, &[1]);
    assert_eq!(f.harness.image_dependency_counts(), Some((1, 1)));
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), rect);
    assert_eq!(f.owner(1), owner);
    assert_eq!(f.button().id(), widget_id);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .framework(),
        previous.request().input.framework()
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    assert_eq!(f.clicks.get(), 0);

    let images: Vec<_> = button_scene_nodes(&after)
        .into_iter()
        .filter_map(|node| {
            if let SceneNode::Image(image) = node {
                Some(image)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(images.len(), 18);
    for z in [LAYER_CONTENT - 1, LAYER_CONTENT] {
        let cells: Vec<_> = images.iter().filter(|image| image.z_index == z).collect();
        assert_eq!(cells.len(), 9);
        for (index, image) in cells.iter().enumerate() {
            assert_eq!(
                image.texture_source,
                ImageTextureSource::Admitted(binding.identity())
            );
            assert_eq!(image.tint, tint.into());
            assert_eq!(
                image.key,
                (z == LAYER_CONTENT && index == 0).then_some(SceneNodeKey::from(widget_id.0))
            );
        }
    }
    let inspection = f.button().inspect_appearance();
    for role in [AppearanceRole::Underlay, AppearanceRole::Surface] {
        assert!(inspection.layers.iter().any(|layer| layer.role == role
            && layer.profile
                == ButtonAppearancePrimitiveProfile::NinePatchImageCells {
                    identity: binding.identity(),
                    sampling: binding.sampling(),
                    availability: Ok(()),
                }));
    }
    assert!(
        inspection
            .layers
            .iter()
            .any(|layer| layer.role == AppearanceRole::FocusCue)
    );

    let bounds = RectF::new(0.0, 0.0, 320.0, 180.0);
    let mut compositor = crate::compositor::Compositor::new(bounds);
    let surface = compositor.create_surface(0, bounds, crate::compositor::SceneKey::BASE);
    renderer
        .register_headless_render_target(surface, (320, 180), 1.0)
        .unwrap();
    let mut scene = crate::core::scene::SceneGraph::new();
    scene.root = after.scene_root.clone();
    let scenes = std::collections::HashMap::from([(surface, scene)]);
    let plan = crate::renderer::DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut crate::renderer::mesh_cache::MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    assert!(renderer.compile_multi(&plan).unwrap().is_ready());
    assert_eq!(binding.availability(), Ok(()));
    assert!(!f.frame().scene_built);

    let replacement_tint = Color::from_srgb(0.2, 0.7, 0.4, 0.75);
    let mut replacement_values = configured(
        AppearanceImageSource::Admitted(binding.clone()),
        NinePatchDestinationInsets::all(1.0),
        replacement_tint,
    );
    for paint in [
        &mut replacement_values.resting,
        &mut replacement_values.hovered,
        &mut replacement_values.pressed,
        &mut replacement_values.disabled,
        &mut replacement_values.selected,
    ]
    .into_iter()
    .flatten()
    {
        paint.underlay = Some(AppearanceSurface::Image(AppearanceImage {
            source: AppearanceImageSource::Admitted(binding.clone()),
            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
            tint,
        }));
    }
    let replacement_style = button_product_style(contract.clone(), replacement_values, 21).unwrap();
    f.button_mut()
        .set_appearance(replacement_style.clone(), None)
        .unwrap();
    let replaced = f.frame();
    assert_focus_appearance_recomposition(&after, &replaced, &[1]);
    assert_eq!(f.harness.image_dependency_counts(), Some((1, 1)));
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), rect);
    assert_eq!(f.owner(1), owner);
    assert_eq!(f.button().id(), widget_id);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .framework(),
        previous.request().input.framework()
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    let replacement_images: Vec<_> = button_scene_nodes(&replaced)
        .into_iter()
        .filter_map(|node| {
            if let SceneNode::Image(image) = node {
                Some(image)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(replacement_images.len(), 10);
    let surface_cell = replacement_images
        .iter()
        .find(|image| image.z_index == LAYER_CONTENT)
        .unwrap();
    assert_eq!(surface_cell.tint, replacement_tint.into());
    assert_eq!(surface_cell.rect.width, 1.0);
    assert_eq!(surface_cell.key, Some(SceneNodeKey::from(widget_id.0)));
    let inspection = f.button().inspect_appearance();
    assert!(
        inspection
            .layers
            .iter()
            .any(|layer| layer.role == AppearanceRole::Underlay
                && layer.profile
                    == ButtonAppearancePrimitiveProfile::ImageRect {
                        identity: binding.identity(),
                        sampling: binding.sampling(),
                        availability: Ok(()),
                    })
    );
    assert!(
        inspection
            .layers
            .iter()
            .any(|layer| layer.role == AppearanceRole::Surface
                && layer.profile
                    == ButtonAppearancePrimitiveProfile::NinePatchImageCells {
                        identity: binding.identity(),
                        sampling: binding.sampling(),
                        availability: Ok(()),
                    })
    );
    assert!(!f.frame().scene_built);

    renderer.register_image_rgba8_srgb(IMAGE, 1, 1, &[0, 0, 255, 255]);
    let new_binding = renderer.admit_image(IMAGE).unwrap();
    assert_ne!(new_binding.identity(), binding.identity());
    assert_eq!(binding.availability(), Err(ImageBindingFailure::Replaced));
    let last_publication = f.button().resolved_appearance().unwrap().clone();
    assert!(
        matches!(f.button_mut().set_appearance(replacement_style, None),
        Err(ButtonAppearanceError::ImageResourceUnavailable {
            identity, failure: ImageBindingFailure::Replaced,
        }) if identity == binding.identity())
    );
    assert_eq!(f.button().resolved_appearance(), Ok(&last_publication));
    let invalidated = f.frame();
    assert!(invalidated.scene_built);
    assert_focus_appearance_recomposition(&replaced, &invalidated, &[1]);
    assert!(
        button_scene_nodes(&invalidated)
            .into_iter()
            .filter_map(|node| {
                if let SceneNode::Image(image) = node {
                    Some(image)
                } else {
                    None
                }
            })
            .all(|image| image.texture_source == ImageTextureSource::Admitted(binding.identity()))
    );
    for layer in f.button().inspect_appearance().layers {
        if let ButtonAppearancePrimitiveProfile::ImageRect { availability, .. }
        | ButtonAppearancePrimitiveProfile::NinePatchImageCells { availability, .. } =
            layer.profile
        {
            assert_eq!(availability, Err(ImageBindingFailure::Replaced));
        }
    }
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.owner(1), owner);
    assert_eq!(f.button().id(), widget_id);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    assert_eq!(f.clicks.get(), 0);
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .framework(),
        previous.request().input.framework()
    );
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    assert!(!f.button().needs_animation());
    assert!(
        !f.frame().scene_built,
        "revocation does not introduce resource polling"
    );

    let solid = button_product_values(&contract, padding);
    f.button_mut()
        .set_appearance(button_product_style(contract, solid, 22).unwrap(), None)
        .unwrap();
    f.frame();
    assert_eq!(f.harness.image_dependency_counts(), None);
    assert_eq!(f.layouts.get(), 1);
    assert!(!f.frame().scene_built);
}

#[test]
fn button_appearance_window_loss_cancels_keyboard_press_without_late_activation() {
    use crate::core::appearance::ButtonPaintCase;
    use crate::core::event::{FusedInputEvent, KeyCode, Modifiers, WindowId};
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;
    use std::time::Instant;

    fn route(
        f: &mut ButtonAppearanceFixture,
        router: &mut EventRouter,
        bindings: &mut BindingRegistry,
        event: FusedInputEvent,
    ) {
        assert!(f.domains.should_route_event(&event));
        let state = if let FusedInputEvent::FocusChanged {
            window_id, focused, ..
        } = &event
        {
            crate::core::app::handle_window_focus_changed(
                &mut f.domains,
                router,
                &mut f.tree,
                *window_id,
                *focused,
                &mut f.text,
            )
            .unwrap()
        } else {
            f.domains.resolve_window_mut(event.window_id()).unwrap()
        };
        router.route_input_event(
            &mut f.tree,
            event,
            Some(f.nodes[0]),
            &mut f.text,
            None,
            state,
            bindings,
        );
    }

    for key_code in [KeyCode::Enter, KeyCode::Space] {
        let mut f = ButtonAppearanceFixture::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        f.frame();
        let owner = f.owner(1);
        router.request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
        route(
            &mut f,
            &mut router,
            &mut bindings,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
        assert!(f.button().keyboard_pulse_active.get());
        let pressed = f.frame();
        let resolved = f.button().resolved_appearance().unwrap();
        assert!(resolved.request().input.button().pressed);
        assert!(resolved.request().input.framework().focused());
        assert_eq!(resolved.paint_case(), ButtonPaintCase::Pressed);

        route(
            &mut f,
            &mut router,
            &mut bindings,
            FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            },
        );
        assert!(
            !f.button().keyboard_pulse_active.get(),
            "window loss must deliver the existing press cancellation"
        );
        let inactive = f.frame();
        assert_focus_appearance_recomposition(&pressed, &inactive, &[1]);
        let resolved = f.button().resolved_appearance().unwrap();
        assert!(!resolved.request().input.button().pressed);
        assert!(!resolved.request().input.framework().focused());
        assert_eq!(resolved.paint_case(), ButtonPaintCase::Resting);
        assert_eq!(f.clicks.get(), 0);
        assert_eq!(f.layouts.get(), 1);

        route(
            &mut f,
            &mut router,
            &mut bindings,
            FusedInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: true,
            },
        );
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        route(
            &mut f,
            &mut router,
            &mut bindings,
            FusedInputEvent::KeyUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
            },
        );
        assert_eq!(
            f.clicks.get(),
            0,
            "cancelled press cannot activate on return"
        );
        for event in [
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            FusedInputEvent::KeyUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
            },
        ] {
            route(&mut f, &mut router, &mut bindings, event);
        }
        assert_eq!(f.clicks.get(), 1, "a fresh press still activates once");
        f.frame();
        assert_eq!(f.layouts.get(), 1);
        assert_eq!(f.owner(1), owner);
    }
}

struct PointerAppearanceFixture {
    tree: WidgetTree,
    root: NodeId,
    clicks: Rc<Cell<u32>>,
    domains: crate::core::event::InteractionDomains,
    router: crate::core::event_router::EventRouter,
    bindings: crate::runtime::binding_registry::BindingRegistry,
    text: FontManagerTextEngine,
    harness: RetainedSceneTestHarness,
}

impl PointerAppearanceFixture {
    fn new() -> Self {
        let mut tree = WidgetTree::new();
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let root = tree.create_root(
            Box::new(
                crate::widgets::button::Button::with_text("Run")
                    .on_click(move || count.set(count.get() + 1)),
            ),
            None,
            0,
        );
        Self {
            tree,
            root,
            clicks,
            domains: crate::core::event::InteractionDomains::new(),
            router: crate::core::event_router::EventRouter::new(),
            bindings: crate::runtime::binding_registry::BindingRegistry::new(),
            text: FontManagerTextEngine::new(),
            harness: RetainedSceneTestHarness::new(),
        }
    }

    fn button_mut(&mut self) -> &mut crate::widgets::button::Button {
        self.tree
            .node_mut(self.root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn pressed(&self) -> bool {
        self.tree
            .node(self.root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::button::Button>()
            .unwrap()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .button()
            .pressed
    }

    fn frame(&mut self) {
        self.harness.frame_with_appearance(
            &mut self.tree,
            self.root,
            Constraints::tight(Size::new(120.0, 40.0)),
            &Theme::default(),
            &mut self.text,
            &IconRegistry::new(&[]),
            &self.domains,
            crate::core::event::WindowId::PRIMARY,
        );
    }

    fn route(&mut self, event: crate::core::event::FusedInputEvent) {
        use crate::core::event::FusedInputEvent;
        assert!(self.domains.should_route_event(&event));
        let state = if let FusedInputEvent::FocusChanged {
            window_id, focused, ..
        } = &event
        {
            crate::core::app::handle_window_focus_changed(
                &mut self.domains,
                &mut self.router,
                &mut self.tree,
                *window_id,
                *focused,
                &mut self.text,
            )
            .unwrap()
        } else {
            self.domains.resolve_window_mut(event.window_id()).unwrap()
        };
        self.router.route_input_event(
            &mut self.tree,
            event,
            Some(self.root),
            &mut self.text,
            None,
            state,
            &mut self.bindings,
        );
    }

    fn pointer(&mut self, kind: crate::core::event::PointerEventKind, x: f32) {
        use crate::core::event::*;
        let time = std::time::Instant::now();
        let window_id = WindowId::PRIMARY;
        let pos = crate::core::math::Point::new(x, 10.0);
        let device = PointerDevice::Mouse;
        self.route(match kind {
            PointerEventKind::Down => FusedInputEvent::PointerDown {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
                click_count: 1,
            },
            PointerEventKind::Up => FusedInputEvent::PointerUp {
                window_id,
                time,
                pos,
                device,
                button: PointerButton::Primary,
            },
            PointerEventKind::Move => FusedInputEvent::PointerMove {
                window_id,
                time,
                pos,
                device,
                delta: (0.0, 0.0),
                buttons: PointerButtonsState {
                    primary: true,
                    secondary: false,
                    auxiliary: false,
                },
            },
            _ => unreachable!("fixture routes only Down, Move and Up"),
        });
    }

    fn window_focus(&mut self, focused: bool) {
        self.route(crate::core::event::FusedInputEvent::FocusChanged {
            window_id: crate::core::event::WindowId::PRIMARY,
            time: std::time::Instant::now(),
            focused,
        });
    }

    fn cancel_pointer(&mut self) {
        use crate::core::event::{CancellationReason, PointerId, WindowId};
        self.router.cancel_pointer_session_with_tree(
            &mut self.tree,
            PointerId::MOUSE,
            CancellationReason::OverlayTakeover,
            self.domains.resolve_window_mut(WindowId::PRIMARY).unwrap(),
        );
    }

    fn key(&mut self, down: bool) {
        use crate::core::event::{FusedInputEvent, KeyCode, Modifiers, WindowId};
        let time = std::time::Instant::now();
        self.route(if down {
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time,
                key_code: KeyCode::Enter,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            }
        } else {
            FusedInputEvent::KeyUp {
                window_id: WindowId::PRIMARY,
                time,
                key_code: KeyCode::Enter,
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
            }
        });
    }
}

#[test]
fn button_appearance_pointer_cancel_clears_press_without_focus_or_layout_change() {
    use crate::core::event::{PointerEventKind::*, PointerId};
    let mut f = PointerAppearanceFixture::new();
    f.frame();
    let epoch = f.tree.node(f.root).unwrap().layout_epoch;
    let owner = f.tree.widget_id_for_node(f.root).unwrap();
    f.pointer(Down, 10.0);
    f.frame();
    assert!(f.pressed());
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(owner)
    );
    assert!(
        f.domains
            .primary_state()
            .gesture_registry
            .candidates(PointerId::MOUSE)
            .is_empty(),
        "ordinary Button is a capture-only owner, not a gesture candidate"
    );
    f.cancel_pointer();
    assert_eq!(
        f.domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    f.frame();
    assert!(
        !f.pressed(),
        "capture cancellation must end the pointer press"
    );
    assert_eq!(f.clicks.get(), 0);
    assert_eq!(f.tree.node(f.root).unwrap().layout_epoch, epoch);
    f.cancel_pointer();
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 0, "cancelled release cannot activate");
    f.pointer(Down, 10.0);
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 1, "a fresh pointer cycle still activates");
    assert_eq!(f.tree.widget_id_for_node(f.root), Some(owner));
}

#[test]
fn button_appearance_pointer_cancel_preserves_independent_keyboard_press() {
    use crate::core::event::PointerEventKind::*;
    let mut f = PointerAppearanceFixture::new();
    f.frame();
    f.pointer(Down, 10.0);
    let owner = f.tree.widget_id_for_node(f.root).unwrap();
    f.key(true);
    f.cancel_pointer();
    f.frame();
    assert!(
        f.pressed(),
        "the independent held keyboard press remains current"
    );
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 0, "pointer cancellation cannot activate");
    f.key(false);
    assert_eq!(
        f.clicks.get(),
        1,
        "the real keyboard release still activates once"
    );
    f.frame();
    assert!(!f.pressed());
}

#[test]
fn button_appearance_pointer_activation_is_independent_of_visual_state() {
    use crate::core::event::PointerEventKind::*;
    use crate::widgets::button::ButtonState;
    let mut f = PointerAppearanceFixture::new();
    f.frame();
    f.button_mut().state = ButtonState::Pressed;
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 0, "visual press cannot grant activation");
    f.pointer(Down, 10.0);
    f.button_mut().state = ButtonState::Normal;
    f.frame();
    assert!(
        f.pressed(),
        "appearance must project the real activation lifetime"
    );
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 1, "visual removal cannot cancel activation");
    f.pointer(Up, 10.0);
    assert_eq!(f.clicks.get(), 1, "release consumes the activation once");
}

#[test]
fn button_appearance_pointer_activation_cancels_on_exit_and_window_loss() {
    use crate::core::event::PointerEventKind::*;
    for window_loss in [false, true] {
        let mut f = PointerAppearanceFixture::new();
        f.frame();
        let epoch = f.tree.node(f.root).unwrap().layout_epoch;
        let owner = f.tree.widget_id_for_node(f.root).unwrap();
        f.pointer(Down, 10.0);
        f.frame();
        assert!(f.pressed());
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        if window_loss {
            f.window_focus(false);
        } else {
            f.pointer(Move, 150.0);
            f.pointer(Move, 10.0);
        }
        f.frame();
        assert!(
            !f.pressed(),
            "cancelled pointer press cannot remain a current cue"
        );
        if window_loss {
            f.window_focus(true);
        }
        f.pointer(Up, 10.0);
        assert_eq!(f.clicks.get(), 0, "cancelled release cannot activate");
        f.pointer(Down, 10.0);
        f.pointer(Up, 10.0);
        assert_eq!(
            f.clicks.get(),
            1,
            "a fresh pointer cycle still activates once"
        );
        f.frame();
        assert_eq!(f.tree.node(f.root).unwrap().layout_epoch, epoch);
        assert_eq!(f.tree.widget_id_for_node(f.root), Some(owner));
    }
}

#[test]
fn button_appearance_window_loss_retires_hover_until_pointer_moves() {
    use crate::core::appearance::ButtonPaintCase;
    use crate::core::event::*;
    use crate::core::event_router::EventRouter;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::{button::Button, flex::Flex};
    use std::time::Instant;

    let mut tree = WidgetTree::new();
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let root = tree.create_root(Box::new(Flex::row().with_spacing(12.0)), None, 0);
    let first = tree.add_child(
        root,
        Box::new(Button::with_text("Open tab").on_click(move || count.set(count.get() + 1))),
        None,
        0,
    );
    let second = tree.add_child(root, Box::new(Button::with_text("Other")), None, 0);
    let [first_owner, second_owner] = [first, second].map(|node| tree.node(node).unwrap().id());
    let mut domains = InteractionDomains::new();
    let mut router = EventRouter::new();
    let mut bindings = BindingRegistry::new();
    let mut harness = RetainedSceneTestHarness::new();
    let mut text = FontManagerTextEngine::new();
    let window_id = WindowId::PRIMARY;
    let mut route = |tree: &mut WidgetTree,
                     domains: &mut InteractionDomains,
                     text: &mut FontManagerTextEngine,
                     event| {
        assert!(domains.should_route_event(&event));
        let state = if let FusedInputEvent::FocusChanged { focused, .. } = &event {
            crate::core::app::handle_window_focus_changed(
                domains,
                &mut router,
                tree,
                window_id,
                *focused,
                text,
            )
            .unwrap()
        } else {
            domains.resolve_window_mut(window_id).unwrap()
        };
        router.route_input_event(tree, event, Some(root), text, None, state, &mut bindings);
    };
    let mut frame =
        |tree: &mut WidgetTree, domains: &InteractionDomains, text: &mut FontManagerTextEngine| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::loose(Size::new(320.0, 120.0)),
                &Theme::default(),
                text,
                &IconRegistry::new(&[]),
                domains,
                window_id,
            )
        };
    let appearance = |tree: &WidgetTree| {
        [first, second].map(|node| {
            let button = tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<Button>()
                .unwrap();
            let resolved = button.resolved_appearance().unwrap();
            (
                resolved.paint_case(),
                resolved.request().input.button().pressed,
            )
        })
    };
    let move_to = |pos, delta| FusedInputEvent::PointerMove {
        window_id,
        time: Instant::now(),
        pos,
        device: PointerDevice::Mouse,
        delta,
        buttons: PointerButtonsState::default(),
    };

    assert!(frame(&mut tree, &domains, &mut text).scene_built);
    let [a, b] = [first, second].map(|node| tree.get_absolute_paint_rect(node).unwrap().center());
    let epoch = tree.node(root).unwrap().layout_epoch;
    route(&mut tree, &mut domains, &mut text, move_to(a, (1.0, 0.0)));
    route(
        &mut tree,
        &mut domains,
        &mut text,
        FusedInputEvent::PointerDown {
            window_id,
            time: Instant::now(),
            pos: a,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        },
    );
    route(
        &mut tree,
        &mut domains,
        &mut text,
        FusedInputEvent::PointerUp {
            window_id,
            time: Instant::now(),
            pos: a,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        },
    );
    assert_eq!(clicks.get(), 1);
    assert_eq!(domains.primary_state().hovered_widget, Some(first_owner));
    assert_eq!(
        domains.primary_state().focus_manager.focused(),
        Some(first_owner)
    );
    assert!(frame(&mut tree, &domains, &mut text).scene_built);
    use ButtonPaintCase::{Hovered, Resting};
    assert_eq!(appearance(&tree), [(Hovered, false), (Resting, false)]);

    let focus_change = |focused| FusedInputEvent::FocusChanged {
        window_id,
        time: Instant::now(),
        focused,
    };
    route(&mut tree, &mut domains, &mut text, focus_change(false));
    assert_eq!(domains.primary_state().hovered_widget, None);
    assert!(frame(&mut tree, &domains, &mut text).scene_built);
    assert_eq!(
        appearance(&tree),
        [(Resting, false), (Resting, false)],
        "loss retires the mounted Button's hover paint"
    );
    route(&mut tree, &mut domains, &mut text, focus_change(true));
    assert_eq!(
        domains.primary_state().focus_manager.focused(),
        Some(first_owner)
    );
    assert!(frame(&mut tree, &domains, &mut text).scene_built);
    assert_eq!(
        appearance(&tree),
        [(Resting, false), (Resting, false)],
        "focus restoration leaves hover clear without pointer evidence"
    );

    route(
        &mut tree,
        &mut domains,
        &mut text,
        move_to(b, (b.x - a.x, b.y - a.y)),
    );
    assert_eq!(domains.primary_state().hovered_widget, Some(second_owner));
    assert!(frame(&mut tree, &domains, &mut text).scene_built);
    assert_eq!(appearance(&tree), [(Resting, false), (Hovered, false)]);
    assert_eq!(tree.node(root).unwrap().layout_epoch, epoch);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn button_appearance_noop_default_preserves_initial_and_instance_publication() {
    use crate::core::layout::Padding;

    let mut f = ButtonAppearanceFixture::new();
    assert!(f.button().needs_update());
    f.button_mut().use_default_appearance();
    assert!(
        f.button().needs_update(),
        "initial publication must remain queued"
    );
    f.frame();
    let published = f.button().inspect_appearance();
    assert!(!f.button().needs_update());
    f.button_mut().use_default_appearance();
    assert!(
        !f.button().needs_update(),
        "unchanged default must not demand another update"
    );
    assert_eq!(f.button().inspect_appearance(), published);
    assert!(!f.frame().scene_built);
    assert_eq!(f.layouts.get(), 1);

    let padding = Padding {
        top: 17.0,
        right: 23.0,
        bottom: 17.0,
        left: 23.0,
    };
    f.button_mut().padding = Some(padding);
    f.button_mut().use_default_appearance();
    assert!(
        f.button().needs_update(),
        "changed instance input needs publication"
    );
    f.frame();
    assert_eq!(
        f.button().resolved_appearance().unwrap().metrics().padding,
        padding
    );
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .instance
            .padding,
        Some(padding)
    );
    assert_eq!(f.layouts.get(), 2);
    f.button_mut().use_default_appearance();
    assert!(!f.button().needs_update());
    assert!(!f.frame().scene_built);
}

#[test]
fn button_appearance_noop_product_preserves_pending_generation_and_instance_changes() {
    use crate::core::appearance::*;
    use crate::core::layout::Padding;

    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    f.frame();
    let contract = ButtonAppearanceContract::standard();
    let values = button_product_values(
        &contract,
        f.button().resolved_appearance().unwrap().metrics().padding,
    );
    let style = button_product_style(contract.clone(), values.clone(), 51).unwrap();
    f.button_mut().set_appearance(style.clone(), None).unwrap();
    f.button_mut().set_appearance(style.clone(), None).unwrap();
    assert!(
        f.button().needs_update(),
        "an equivalent pending request must not erase publication"
    );
    f.frame();
    let published = f.button().inspect_appearance();
    f.button_mut().set_appearance(style, None).unwrap();
    assert!(
        !f.button().needs_update(),
        "unchanged product must not demand another update"
    );
    assert_eq!(f.button().inspect_appearance(), published);
    assert!(!f.frame().scene_built);

    let next = button_product_style(contract, values, 52).unwrap();
    f.button_mut().set_appearance(next.clone(), None).unwrap();
    assert!(f.button().needs_update());
    assert!(
        !f.frame().scene_built,
        "new identity with equal paint retains the scene"
    );
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .configuration
            .generation,
        52
    );
    assert_eq!(f.layouts.get(), 1);

    let padding = Padding {
        top: 15.0,
        right: 27.0,
        bottom: 15.0,
        left: 27.0,
    };
    f.button_mut().padding = Some(padding);
    f.button_mut().set_appearance(next.clone(), None).unwrap();
    assert!(f.button().needs_update());
    f.frame();
    assert_eq!(
        f.button().resolved_appearance().unwrap().metrics().padding,
        padding
    );
    assert_eq!(f.layouts.get(), 2);
    f.button_mut().set_appearance(next, None).unwrap();
    assert!(!f.button().needs_update());
    assert_eq!(f.owner(1), owner);
    assert_eq!(f.clicks.get(), 0);
}

#[test]
fn button_appearance_noop_preserves_pending_interaction_and_selection_changes() {
    use crate::core::appearance::*;

    let mut f = ButtonAppearanceFixture::new();
    f.frame();
    let product = ProductAppearanceOwner(77);
    let contract =
        ButtonAppearanceContract::new(vec![AppearanceSelector::button()], Some(product)).unwrap();
    let values = button_product_values(
        &contract,
        f.button().resolved_appearance().unwrap().metrics().padding,
    );
    let style = button_product_style(contract, values, 61).unwrap();
    let selection = Some(ProductSelectionFact {
        owner: product,
        selected: false,
    });
    f.button_mut()
        .set_appearance(style.clone(), selection)
        .unwrap();
    f.frame();
    f.event(|button, cx| button.on_hover_enter(cx));
    f.button_mut()
        .set_appearance(style.clone(), selection)
        .unwrap();
    assert!(
        f.button().needs_update(),
        "no-op appearance must preserve interaction work"
    );
    f.frame();
    assert!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .button()
            .hovered
    );
    assert!(
        !f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .selection()
            .unwrap()
            .selected
    );
    f.button_mut()
        .set_appearance(
            style,
            Some(ProductSelectionFact {
                owner: product,
                selected: true,
            }),
        )
        .unwrap();
    assert!(f.button().needs_update());
    f.frame();
    let resolved = f.button().resolved_appearance().unwrap();
    assert!(resolved.request().input.button().hovered);
    assert!(resolved.request().input.selection().unwrap().selected);
    assert_eq!(resolved.paint_case(), ButtonPaintCase::Hovered);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.clicks.get(), 0);
}

#[test]
fn button_appearance_inspection_describes_the_published_result_without_changing_it() {
    use crate::core::appearance::*;
    use crate::core::scene::SceneNode;
    use crate::widgets::button::{ButtonAppearancePrimitiveProfile, ButtonAppearanceScope};

    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    let first_frame = f.frame();
    let first = f.button().inspect_appearance();
    assert_eq!(first.scope, ButtonAppearanceScope::InheritedWidget);
    assert_eq!(first.resolution.as_ref(), f.button().resolved_appearance());
    assert_eq!(
        first
            .layers
            .iter()
            .map(|layer| layer.profile)
            .collect::<Vec<_>>(),
        vec![
            ButtonAppearancePrimitiveProfile::GradientRoundedRect,
            ButtonAppearancePrimitiveProfile::GradientRoundedRect,
            ButtonAppearancePrimitiveProfile::BorderRoundedRect,
        ]
    );
    let nodes = button_scene_nodes(&first_frame);
    let surface = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::GradientRect(value)
                if value.z_index == crate::core::paint::LAYER_CONTENT =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    let underlay = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::GradientRect(value)
                if value.z_index == crate::core::paint::LAYER_CONTENT - 1 =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    let border = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::BorderRect(value) => Some(value),
            _ => None,
        })
        .unwrap();
    let resolved = first.resolution.as_ref().unwrap();
    let resolved_surface = resolved
        .composition()
        .iter()
        .find(|layer| layer.role == AppearanceRole::Surface)
        .unwrap();
    let AppearancePrimitive::Surface(AppearanceSurface::Gradient(gradient)) =
        &resolved_surface.primitive
    else {
        panic!("default Button must publish its Gradient surface")
    };
    let resolved_underlay = resolved
        .composition()
        .iter()
        .find(|layer| layer.role == AppearanceRole::Underlay)
        .unwrap();
    let AppearancePrimitive::Surface(AppearanceSurface::Gradient(rim)) =
        &resolved_underlay.primitive
    else {
        panic!("default Button must publish its Gradient underlay")
    };
    assert_eq!(&underlay.gradient, rim.definition());
    assert_eq!(&surface.gradient, gradient.definition());
    assert_eq!(underlay.radius, resolved.radius());
    assert!(surface.radius.is_finite() && surface.radius >= 0.0);
    assert!(surface.radius <= underlay.radius);
    assert_eq!(underlay.rect, f.tree.get_rect(f.nodes[1]).unwrap());
    assert_eq!(underlay.rect, border.rect);
    assert!(surface.rect.x > underlay.rect.x && surface.rect.y > underlay.rect.y);
    assert!(surface.rect.right() < underlay.rect.right());
    assert!(surface.rect.bottom() < underlay.rect.bottom());
    assert_eq!(first.layers[0].role, AppearanceRole::Underlay);
    assert_eq!(first.layers[0].z_index, underlay.z_index);
    assert_eq!(first.layers[1].role, AppearanceRole::Surface);
    assert_eq!(first.layers[1].z_index, surface.z_index);
    assert_eq!(first.layers[2].role, AppearanceRole::Border);
    assert_eq!(first.layers[2].z_index, border.z_index);
    assert!(underlay.z_index < surface.z_index && surface.z_index < border.z_index);
    let content = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert_eq!(first.content_z_index, content.z_index);

    let product = ProductAppearanceOwner(77);
    let selector = AppearanceSelector::new(
        AppearanceAddress::BUTTON_BODY,
        AppearanceVariant::Default,
        vec![AppearanceQualifier {
            owner: product,
            key: 9,
            value: QualifierValue::Flag(true),
        }],
    )
    .unwrap();
    let contract = ButtonAppearanceContract::new(vec![selector.clone()], Some(product)).unwrap();
    let padding = first.resolution.as_ref().unwrap().metrics().padding;
    let values = button_product_values(&contract, padding);
    f.button_mut()
        .set_appearance(
            button_product_style(contract, values, 23).unwrap(),
            Some(ProductSelectionFact {
                owner: product,
                selected: true,
            }),
        )
        .unwrap();
    assert_eq!(
        f.button().inspect_appearance(),
        first,
        "pending authoring must not replace the published inspection"
    );
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    f.event(|button, cx| button.on_hover_enter(cx));
    let product_frame = f.frame();
    let inspected = f.button().inspect_appearance();
    let resolved = inspected.resolution.as_ref().unwrap();
    assert_eq!(resolved.request().configuration.generation, 23);
    assert_eq!(
        resolved.request().configuration.owner,
        AppearanceConfigurationOwner::Product(product)
    );
    assert_eq!(resolved.request().selector, selector);
    assert!(resolved.request().input.framework().focused());
    assert!(resolved.request().input.button().hovered);
    assert!(resolved.request().input.selection().unwrap().selected);
    assert_eq!(resolved.paint_case(), ButtonPaintCase::Hovered);
    assert_eq!(resolved.metrics().padding, padding);
    assert_eq!(
        inspected
            .layers
            .iter()
            .map(|layer| layer.role)
            .collect::<Vec<_>>(),
        resolved
            .composition()
            .iter()
            .map(|layer| layer.role)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        inspected
            .layers
            .iter()
            .filter(|layer| layer.role == AppearanceRole::FocusCue)
            .count(),
        1
    );
    assert_eq!(
        button_scene_nodes(&product_frame)
            .iter()
            .filter(|node| matches!(node,
        SceneNode::BorderRect(value) if value.border_width == 3.0))
            .count(),
        1
    );
    assert_eq!(f.button().inspect_appearance(), inspected);
    assert!(!f.frame().scene_built);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.clicks.get(), 0);
    assert_eq!(f.owner(1), owner);
    assert_eq!(
        first.resolution.unwrap().request().configuration.owner,
        AppearanceConfigurationOwner::Framework
    );
}

#[test]
fn button_appearance_inspection_preserves_fallback_absence_and_unpublished_outcomes() {
    use crate::core::appearance::*;
    use crate::widgets::button::ButtonAppearanceError;

    let mut f = ButtonAppearanceFixture::new();
    let unpublished = f.button().inspect_appearance();
    assert_eq!(
        unpublished.resolution,
        Err(ButtonAppearanceError::MissingFrameworkPublication)
    );
    assert!(unpublished.layers.is_empty());
    let product = ProductAppearanceOwner(77);
    let fallback = AppearanceFallbackId {
        owner: product,
        key: 12,
    };
    let contract = ButtonAppearanceContract::standard();
    let selector = AppearanceSelector::button();
    let mut values = f.theme.button_appearance_values(&contract);
    values.resting.as_mut().unwrap().border = None;
    let configuration = ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Product(product),
            generation: 31,
        },
        contract,
        vec![],
        vec![ButtonAppearanceFallback {
            id: fallback,
            selector: selector.clone(),
            values,
        }],
    )
    .unwrap();
    let style = configuration
        .prepare_style(selector, AppearanceFallbackPolicy::Approve(fallback))
        .unwrap();
    f.button_mut().set_appearance(style, None).unwrap();
    let frame = f.frame();
    let inspected = f.button().inspect_appearance();
    let resolved = inspected.resolution.as_ref().unwrap();
    assert_eq!(
        resolved.coverage(),
        AppearanceCoverage::Fallback {
            id: fallback,
            primary_problem: AppearanceProblem::Missing(MissingAppearanceValue::Definition),
        }
    );
    assert_eq!(
        resolved.request().fallback,
        AppearanceFallbackPolicy::Approve(fallback)
    );
    assert!(
        resolved
            .optional_absences()
            .contains(&AppearanceRole::Border)
    );
    assert_eq!(
        inspected
            .layers
            .iter()
            .map(|layer| layer.role)
            .collect::<Vec<_>>(),
        vec![AppearanceRole::Underlay, AppearanceRole::Surface]
    );
    assert!(
        !button_scene_nodes(&frame)
            .iter()
            .any(|node| matches!(node, crate::core::scene::SceneNode::BorderRect(_)))
    );
    assert_eq!(
        resolved.admission(),
        AppearanceAdmission::PendingCapabilityAdmission
    );
}

#[test]
fn button_appearance_inspection_observes_image_generation_failure_without_fallback() {
    use crate::core::appearance::*;
    use crate::core::scene::{ImageTextureSource, SceneNode};
    use crate::renderer::image_binding::{ImageBindingFailure, ImageBindingSampling};
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::widgets::button::ButtonAppearancePrimitiveProfile;

    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(263_261, 1, 1, &[255, 128, 0, 255]);
    let binding = renderer.admit_image(263_261).unwrap();
    let mut f = ButtonAppearanceFixture::new();
    f.frame();
    let contract = ButtonAppearanceContract::standard();
    let mut values = button_product_values(
        &contract,
        f.button().resolved_appearance().unwrap().metrics().padding,
    );
    for paint in [
        &mut values.resting,
        &mut values.hovered,
        &mut values.pressed,
        &mut values.disabled,
    ]
    .into_iter()
    .flatten()
    {
        paint.radius = 0.0;
        paint.surface = AppearanceSurface::Image(AppearanceImage {
            source: AppearanceImageSource::Admitted(binding.clone()),
            uv_rect: RectF::new(0.1, 0.2, 0.6, 0.7),
            tint: Color::from_srgb(0.8, 0.4, 0.2, 0.5),
        });
        paint.underlay = Some(paint.surface.clone());
    }
    f.button_mut()
        .set_appearance(button_product_style(contract, values, 41).unwrap(), None)
        .unwrap();
    let frame = f.frame();
    let before = f.button().inspect_appearance();
    let image_profile = |inspection: &crate::widgets::button::ButtonAppearanceInspection| {
        inspection
            .layers
            .iter()
            .find(|layer| layer.role == AppearanceRole::Surface)
            .unwrap()
            .profile
    };
    assert_eq!(
        image_profile(&before),
        ButtonAppearancePrimitiveProfile::ImageRect {
            identity: binding.identity(),
            sampling: ImageBindingSampling::Linear,
            availability: Ok(()),
        }
    );
    let shared_profiles: Vec<_> = before
        .layers
        .iter()
        .filter(|layer| {
            matches!(
                layer.role,
                AppearanceRole::Underlay | AppearanceRole::Surface
            )
        })
        .map(|layer| layer.profile)
        .collect();
    assert_eq!(shared_profiles, vec![image_profile(&before); 2]);
    assert_eq!(
        button_scene_nodes(&frame)
            .iter()
            .filter(|node| matches!(node, SceneNode::Image(_)))
            .count(),
        2
    );
    let image = button_scene_nodes(&frame)
        .into_iter()
        .find_map(|node| match node {
            SceneNode::Image(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        image.texture_source,
        ImageTextureSource::Admitted(binding.identity())
    );
    assert_eq!(image.tint, Color::from_srgb(0.8, 0.4, 0.2, 0.5).into());
    assert!(!f.frame().scene_built);
    renderer.unregister_image(263_261);
    let after = f.button().inspect_appearance();
    assert_eq!(before.resolution, after.resolution);
    assert_eq!(
        image_profile(&after),
        ButtonAppearancePrimitiveProfile::ImageRect {
            identity: binding.identity(),
            sampling: ImageBindingSampling::Linear,
            availability: Err(ImageBindingFailure::Unregistered),
        }
    );
    assert!(
        after
            .layers
            .iter()
            .filter(|layer| matches!(
                layer.role,
                AppearanceRole::Underlay | AppearanceRole::Surface
            ))
            .all(|layer| layer.profile == image_profile(&after))
    );
    assert_eq!(
        image_profile(&before),
        ButtonAppearancePrimitiveProfile::ImageRect {
            identity: binding.identity(),
            sampling: ImageBindingSampling::Linear,
            availability: Ok(()),
        }
    );
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.clicks.get(), 0);
}

#[test]
fn button_appearance_default_consumes_theme_values_and_existing_measurement() {
    use crate::core::appearance::*;
    use crate::core::scene::SceneNode;
    let mut f = ButtonAppearanceFixture::new();
    let frame = f.frame();
    let values = f
        .theme
        .button_appearance_values(&ButtonAppearanceContract::standard());
    let expected = values.resting.unwrap();
    let resolved = f.button().resolved_appearance().unwrap();
    assert_eq!(
        resolved.request().configuration.owner,
        AppearanceConfigurationOwner::Framework
    );
    assert_eq!(*resolved.metrics(), values.metrics.unwrap());
    assert_eq!(
        f.button().layout_bounds,
        Size::new(
            40.0 + resolved.metrics().padding.horizontal(),
            f.theme.typography.line_height + resolved.metrics().padding.vertical(),
        )
    );
    let allocation = f.tree.get_rect(f.nodes[1]).unwrap();
    let nodes = button_scene_nodes(&frame);
    let fills: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::GradientRect(value) => Some(value),
            _ => None,
        })
        .collect();
    let AppearanceSurface::Gradient(face) = expected.surface else {
        unreachable!()
    };
    let Some(AppearanceSurface::Gradient(rim)) = expected.underlay else {
        panic!("default Button must resolve its raised rim")
    };
    assert_eq!(fills.len(), 2);
    let outer = fills
        .iter()
        .find(|fill| fill.z_index == crate::core::paint::LAYER_CONTENT - 1)
        .unwrap();
    let inner = fills
        .iter()
        .find(|fill| fill.z_index == crate::core::paint::LAYER_CONTENT)
        .unwrap();
    assert_eq!(&outer.gradient, rim.definition());
    assert_eq!(&inner.gradient, face.definition());
    assert_eq!(outer.rect, allocation);
    assert!(inner.rect.x > outer.rect.x && inner.rect.y > outer.rect.y);
    assert!(inner.rect.right() < outer.rect.right());
    assert!(inner.rect.bottom() < outer.rect.bottom());
    for fill in [outer, inner] {
        assert!(fill.radius.is_finite() && fill.radius >= 0.0);
        assert!(fill.radius <= fill.rect.width.min(fill.rect.height) * 0.5);
    }
    assert!(inner.radius <= outer.radius);
    assert!(outer.z_index < inner.z_index);
    let stroke = expected.border.expect("default Button has a frame");
    let borders: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::BorderRect(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(borders.len(), 1);
    assert_eq!(borders[0].rect, allocation);
    assert_eq!(borders[0].border_color, stroke.color.into());
    assert_eq!(borders[0].border_width, stroke.width);
    assert_eq!(borders[0].radius, outer.radius);
    assert!(borders[0].z_index > inner.z_index);
    let text = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.text, "Run");
    assert_eq!(text.color, expected.content_color.into());
    assert_eq!(text.font_size, f.theme.typography.font_size);
    assert!(text.z_index > borders[0].z_index);
    assert_eq!(f.layouts.get(), 1);
    assert!(!f.frame().scene_built);
}
#[test]
fn button_appearance_relief_small_geometry_preserves_prepared_surface_coverage() {
    use crate::core::appearance::*;
    use crate::core::scene::SceneNode;

    fn frame(
        f: &mut ButtonAppearanceFixture,
        size: Size,
    ) -> crate::core::app::RetainedSceneTestFrame {
        f.harness.frame_with_appearance(
            &mut f.tree,
            f.nodes[0],
            Constraints::tight(size),
            &f.theme,
            &mut f.text,
            &IconRegistry::new(&[]),
            &f.domains,
            crate::core::event::WindowId::PRIMARY,
        )
    }

    for dark in [false, true] {
        for size in [
            Size::new(1.0, 1.0),
            Size::new(3.0, 2.0),
            Size::new(14.0, 7.0),
        ] {
            let mut f = ButtonAppearanceFixture::new();
            f.theme = if dark { Theme::dark() } else { Theme::light() };
            let before = frame(&mut f, size);
            let allocation = f.tree.get_rect(f.nodes[1]).unwrap();
            let owner = f.owner(1);
            let metrics = *f.button().resolved_appearance().unwrap().metrics();
            assert_eq!(allocation.size(), size);
            assert_eq!(f.button().layout_bounds, size);
            let nodes = button_scene_nodes(&before);
            let fills: Vec<_> = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::GradientRect(value) => Some(value),
                    _ => None,
                })
                .collect();
            assert_eq!(fills.len(), 2);
            for fill in &fills {
                let rect = fill.rect;
                assert!(
                    [rect.x, rect.y, rect.width, rect.height]
                        .into_iter()
                        .all(f32::is_finite)
                );
                assert!(rect.width > 0.0 && rect.height > 0.0);
                assert!(rect.x >= allocation.x && rect.y >= allocation.y);
                assert!(rect.right() <= allocation.right());
                assert!(rect.bottom() <= allocation.bottom());
                assert!(fill.radius.is_finite() && fill.radius >= 0.0);
                assert!(fill.radius <= rect.width.min(rect.height) * 0.5);
            }
            let outer = fills.iter().find(|fill| fill.rect == allocation).unwrap();
            let inner = fills.iter().find(|fill| fill.rect != allocation).unwrap();
            assert!(outer.z_index < inner.z_index);
            assert!(inner.rect.x > allocation.x && inner.rect.y > allocation.y);
            assert!(inner.rect.right() < allocation.right());
            assert!(inner.rect.bottom() < allocation.bottom());
            assert!(!frame(&mut f, size).scene_built);

            let contract = ButtonAppearanceContract::standard();
            let values = f.theme.button_appearance_values(&contract);
            let radius = values.resting.as_ref().unwrap().radius;
            f.button_mut()
                .set_appearance(button_product_style(contract, values, 1).unwrap(), None)
                .unwrap();
            let prepared = frame(&mut f, size);
            let prepared_nodes = button_scene_nodes(&prepared);
            let prepared_fills: Vec<_> = prepared_nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::GradientRect(value) => Some(value),
                    _ => None,
                })
                .collect();
            assert_eq!(prepared_fills.len(), 2);
            assert!(prepared_fills.iter().all(|fill| fill.rect == allocation));
            assert!(prepared_fills.iter().all(|fill| fill.radius == radius));
            assert_eq!(
                *f.button().resolved_appearance().unwrap().metrics(),
                metrics
            );
            assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), allocation);
            assert_eq!(f.button().layout_bounds, size);
            assert_eq!(f.owner(1), owner);
            assert_eq!(f.layouts.get(), 1);
            assert_eq!(f.clicks.get(), 0);
            assert!(!frame(&mut f, size).scene_built);

            f.button_mut().use_default_appearance();
            let restored = frame(&mut f, size);
            let restored_nodes = button_scene_nodes(&restored);
            let restored_geometry: Vec<_> = restored_nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::GradientRect(value) => {
                        Some((value.rect, value.radius, value.z_index))
                    }
                    _ => None,
                })
                .collect();
            let default_geometry: Vec<_> = fills
                .iter()
                .map(|fill| (fill.rect, fill.radius, fill.z_index))
                .collect();
            assert_eq!(restored_geometry, default_geometry);
            assert_eq!(
                *f.button().resolved_appearance().unwrap().metrics(),
                metrics
            );
            assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), allocation);
            assert_eq!(f.button().layout_bounds, size);
            assert_eq!(f.owner(1), owner);
            assert_eq!(f.layouts.get(), 1);
            assert_eq!(f.clicks.get(), 0);
            assert!(!frame(&mut f, size).scene_built);
        }
    }
}

#[test]
fn button_appearance_default_states_preserve_density_metrics_and_shape() {
    use crate::core::appearance::*;
    use crate::core::theme::DensityMode;

    for dark in [false, true] {
        for density in [
            DensityMode::Compact,
            DensityMode::Regular,
            DensityMode::Comfortable,
            DensityMode::Intermediate30,
        ] {
            let mut f = ButtonAppearanceFixture::new();
            f.theme = if dark {
                Theme::dark_with_density(density)
            } else {
                Theme::light_with_density(density)
            };
            f.frame();
            let initial = f.button().resolved_appearance().unwrap();
            let framework = initial.request().input.framework();
            let metrics = *initial.metrics();
            let radius = initial.radius();
            let bounds = f.button().layout_bounds;
            let selector = AppearanceSelector::button();
            let contract = ButtonAppearanceContract::standard();
            let configuration = ButtonAppearanceConfiguration::new(
                AppearanceConfigurationId {
                    owner: AppearanceConfigurationOwner::Framework,
                    generation: 0,
                },
                contract.clone(),
                vec![ButtonAppearanceDefinition {
                    selector: selector.clone(),
                    values: f.theme.button_appearance_values(&contract),
                }],
                Vec::new(),
            )
            .unwrap();
            let style = configuration
                .prepare_style(selector, AppearanceFallbackPolicy::Reject)
                .unwrap();

            for (enabled, hovered, pressed, expected) in [
                (true, false, false, ButtonPaintCase::Resting),
                (true, true, false, ButtonPaintCase::Hovered),
                (true, true, true, ButtonPaintCase::Pressed),
                (false, true, false, ButtonPaintCase::Disabled),
            ] {
                let facts = ButtonAppearanceFacts {
                    enabled,
                    hovered,
                    pressed,
                };
                let value =
                    match style.resolve(AppearanceResolutionInput::new(framework, facts, None)) {
                        AppearanceValueResolution::Resolved(value) => value,
                        AppearanceValueResolution::Unresolved(failure) => {
                            panic!("default Button state is unresolved: {failure:?}")
                        }
                    };
                assert_eq!(value.paint_case(), expected);
                assert_eq!(value.request().input.button(), facts);
                assert_eq!(*value.metrics(), metrics);
                assert_eq!(value.radius(), radius);
                assert!(value.radius().is_finite() && value.radius() >= 0.0);
                let surface = value
                    .composition()
                    .iter()
                    .find(|layer| layer.role == AppearanceRole::Surface)
                    .unwrap();
                let AppearancePrimitive::Surface(AppearanceSurface::Gradient(gradient)) =
                    &surface.primitive
                else {
                    panic!("default Button state must use the admitted Gradient path")
                };
                let definition = gradient.definition();
                assert!(
                    [
                        definition.p0.0,
                        definition.p0.1,
                        definition.p1.0,
                        definition.p1.1,
                    ]
                    .into_iter()
                    .chain(definition.stops.iter().flat_map(|stop| {
                        [
                            stop.offset,
                            stop.color.r,
                            stop.color.g,
                            stop.color.b,
                            stop.color.a,
                        ]
                    }))
                    .all(f32::is_finite)
                );
                let border = value
                    .composition()
                    .iter()
                    .find(|layer| layer.role == AppearanceRole::Border)
                    .unwrap();
                let AppearancePrimitive::Stroke(stroke) = &border.primitive else {
                    panic!("default Button frame must use the admitted stroke path")
                };
                assert!(stroke.width.is_finite() && stroke.width >= 0.0);
            }
            assert_eq!(f.button().layout_bounds, bounds);
            assert_eq!(f.layouts.get(), 1);
            assert_eq!(f.clicks.get(), 0);
            assert!(!f.frame().scene_built);
        }
    }
}

#[test]
fn button_appearance_paint_replacement_retains_siblings_layout_and_input_authority() {
    use crate::core::appearance::*;
    use crate::core::event::PointerId;
    use crate::core::scene::SceneNode;
    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    let pointer_owner = f.owner(2);
    let scroll_owner = f.owner(3);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    f.domains
        .primary_state_mut()
        .pointer_capture_manager
        .request_capture(PointerId::MOUSE, pointer_owner)
        .unwrap();
    f.domains.primary_state_mut().scroll_capture_owner = Some(scroll_owner);
    let before = f.frame();
    let before_resolution = f.button().resolved_appearance().unwrap().clone();
    let epoch = f.tree.node(f.nodes[1]).unwrap().layout_epoch;
    let contract = ButtonAppearanceContract::standard();
    let values = button_product_values(&contract, before_resolution.metrics().padding);
    f.button_mut()
        .set_appearance(
            button_product_style(contract, values.clone(), 1).unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(
        f.button().resolved_appearance().unwrap(),
        &before_resolution,
        "selection publishes only in update"
    );
    let after = f.frame();
    assert_focus_appearance_recomposition(&before, &after, &[1]);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.tree.node(f.nodes[1]).unwrap().layout_epoch, epoch);
    assert_eq!(f.owner(1), owner);
    assert_eq!(f.clicks.get(), 0);
    assert!(f.button().focusable());
    assert!(f.button().accepts_keyboard_activation());
    assert!(!f.button().capture_eligible());
    let state = f.domains.primary_state();
    assert_eq!(state.focus_manager.focused(), Some(owner));
    assert_eq!(
        state
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        Some(pointer_owner)
    );
    assert_eq!(state.scroll_capture_owner, Some(scroll_owner));
    assert_eq!(
        state.keyboard_mode,
        crate::core::event::KeyboardAuthorityMode::Navigation
    );
    let resolved = f.button().resolved_appearance().unwrap();
    assert!(resolved.request().input.framework().focused());
    assert!(!resolved.request().input.framework().pointer_captured());
    assert_eq!(
        resolved.request().configuration.owner,
        AppearanceConfigurationOwner::Product(ProductAppearanceOwner(77))
    );
    let nodes = button_scene_nodes(&after);
    let surface = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::RoundedRect(value) if value.z_index == crate::core::paint::LAYER_CONTENT => {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    let AppearanceSurface::Solid(fill) = values.resting.as_ref().unwrap().surface else {
        unreachable!()
    };
    assert_eq!(surface.color, fill.into());
    assert_eq!(surface.radius, 11.0);
    let text = nodes
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        text.color,
        values.resting.as_ref().unwrap().content_color.into()
    );
    let focus: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::BorderRect(value)
                if value.border_color == values.focus.unwrap().color.into() =>
            {
                Some(value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        focus.len(),
        1,
        "complete Button projection must not append a second indicator"
    );
    assert!(!nodes.iter().any(|node| matches!(node, SceneNode::BorderRect(value) if value.border_color == f.theme.colors.borders.focus.into())));
    let mut z: Vec<_> = nodes
        .iter()
        .filter_map(|node| match node {
            SceneNode::RoundedRect(value) => Some(value.z_index),
            SceneNode::BorderRect(value) => Some(value.z_index),
            SceneNode::Text(value) => Some(value.z_index),
            _ => None,
        })
        .collect();
    z.sort_unstable();
    assert_eq!(z, vec![1, 2, 3, 4, 5, 6]);
    assert!(!f.frame().scene_built);

    // A different configuration identity with identical effective dependencies
    // updates provenance without invalidating retained paint or layout.
    let contract = ButtonAppearanceContract::standard();
    f.button_mut()
        .set_appearance(button_product_style(contract, values, 2).unwrap(), None)
        .unwrap();
    assert!(!f.frame().scene_built);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .configuration
            .generation,
        2
    );

    f.button_mut().use_default_appearance();
    let restored = f.frame();
    assert_focus_appearance_recomposition(&after, &restored, &[1]);
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(
        f.button().resolved_appearance().unwrap(),
        &before_resolution
    );
    assert!(!f.frame().scene_built);
}

#[test]
fn button_appearance_padding_replacement_and_instance_precedence_use_same_frame_layout() {
    use crate::core::appearance::*;
    use crate::core::layout::Padding;
    let mut f = ButtonAppearanceFixture::new();
    f.frame();
    let owner = f.owner(1);
    let initial = f.button().layout_bounds;
    let initial_padding = f.button().resolved_appearance().unwrap().metrics().padding;
    let contract = ButtonAppearanceContract::standard();
    let padding = Padding::symmetric(23.0, 17.0);
    let values = button_product_values(&contract, padding);
    f.button_mut()
        .set_appearance(
            button_product_style(contract, values.clone(), 1).unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(f.button().layout_bounds, initial);
    assert!(f.frame().scene_built);
    assert_eq!(f.layouts.get(), 2);
    assert_eq!(f.owner(1), owner);
    assert_eq!(
        f.button().layout_bounds,
        Size::new(
            initial.width - initial_padding.horizontal() + padding.horizontal(),
            initial.height - initial_padding.vertical() + padding.vertical(),
        )
    );
    assert_eq!(
        f.button().resolved_appearance().unwrap().metrics().padding,
        padding
    );
    assert_eq!(
        f.tree.get_rect(f.nodes[1]).unwrap().size(),
        f.button().layout_bounds
    );

    f.button_mut().padding = Some(Padding::all(9.0));
    assert!(f.frame().scene_built);
    assert_eq!(f.layouts.get(), 3);
    assert_eq!(
        f.button().resolved_appearance().unwrap().metrics().padding,
        Padding::all(9.0)
    );
    let bounds = f.button().layout_bounds;
    let mut changed = values;
    changed.metrics = Some(ButtonAppearanceMetrics {
        padding: Padding::all(30.0),
    });
    f.button_mut()
        .set_appearance(
            button_product_style(ButtonAppearanceContract::standard(), changed, 2).unwrap(),
            None,
        )
        .unwrap();
    assert!(
        !f.frame().scene_built,
        "masked metric replacement is an effective no-op"
    );
    assert_eq!(f.layouts.get(), 3);
    assert_eq!(f.button().layout_bounds, bounds);
}

#[test]
fn button_appearance_independent_hover_press_selection_and_focus_preserve_activation() {
    use crate::core::appearance::*;
    use crate::core::event::*;
    let mut f = ButtonAppearanceFixture::new();
    let owner = f.owner(1);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    let product = ProductAppearanceOwner(77);
    let selector = AppearanceSelector::new(
        AppearanceAddress::BUTTON_BODY,
        AppearanceVariant::Default,
        vec![AppearanceQualifier {
            owner: product,
            key: 1,
            value: QualifierValue::Flag(true),
        }],
    )
    .unwrap();
    let contract = ButtonAppearanceContract::new(vec![selector.clone()], Some(product)).unwrap();
    let padding = f
        .theme
        .button_appearance_values(&ButtonAppearanceContract::standard())
        .metrics
        .unwrap()
        .padding;
    let values = button_product_values(&contract, padding);
    f.button_mut()
        .set_appearance(
            button_product_style(contract, values, 1).unwrap(),
            Some(ProductSelectionFact {
                owner: product,
                selected: true,
            }),
        )
        .unwrap();
    f.frame();
    assert_eq!(
        f.button().resolved_appearance().unwrap().paint_case(),
        ButtonPaintCase::Selected
    );
    f.event(|button, cx| button.on_hover_enter(cx));
    let hovered = f.frame();
    let resolved = f.button().resolved_appearance().unwrap();
    assert_eq!(resolved.request().selector, selector);
    assert!(resolved.request().input.button().hovered);
    assert!(resolved.request().input.selection().unwrap().selected);
    assert!(resolved.request().input.framework().focused());
    assert_eq!(resolved.paint_case(), ButtonPaintCase::Hovered);

    let pointer = |kind| {
        Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            global_pos: (1.0, 1.0),
            local_pos: (1.0, 1.0),
            kind,
            button: Some(PointerButton::Primary),
            modifiers: Modifiers::default(),
            pointer_type: PointerType::Mouse,
        })
    };
    f.event(|button, cx| {
        assert!(button.handle_event(cx, &pointer(PointerEventKind::Down), EventPhase::Target))
    });
    let pressed = f.frame();
    assert_focus_appearance_recomposition(&hovered, &pressed, &[1]);
    let resolved = f.button().resolved_appearance().unwrap();
    assert_eq!(resolved.paint_case(), ButtonPaintCase::Pressed);
    assert!(resolved.request().input.button().hovered);
    assert!(resolved.request().input.button().pressed);
    assert!(resolved.request().input.framework().focused());
    assert_eq!(
        resolved
            .composition()
            .iter()
            .filter(|layer| layer.role == AppearanceRole::FocusCue)
            .count(),
        1
    );
    assert_eq!(f.clicks.get(), 0);
    f.event(|button, cx| {
        assert!(button.handle_event(cx, &pointer(PointerEventKind::Up), EventPhase::Target))
    });
    assert_eq!(f.clicks.get(), 1);
    f.event(|button, cx| button.on_keyboard_activate(cx));
    f.frame();
    assert!(
        f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .button()
            .pressed
    );
    let key_up = Event::Key(KeyEvent {
        key_code: KeyCode::Enter,

        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
        is_down: false,
        modifiers: Modifiers::default(),
        is_repeat: false,
        text: None,
    });
    f.event(|button, cx| assert!(button.handle_event(cx, &key_up, EventPhase::Target)));
    assert_eq!(f.clicks.get(), 2);
    f.event(|button, cx| button.on_hover_leave(cx));
    f.frame();
    assert!(
        !f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .button()
            .hovered
    );
    assert!(
        !f.button()
            .resolved_appearance()
            .unwrap()
            .request()
            .input
            .button()
            .pressed
    );
    assert_eq!(f.layouts.get(), 1);
    assert_eq!(f.owner(1), owner);
    assert_eq!(
        f.domains.primary_state().focus_manager.focused(),
        Some(owner)
    );
}

#[test]
fn button_appearance_rejects_pending_material_and_incomplete_activation_atomically() {
    use crate::core::appearance::*;
    use crate::widgets::button::ButtonAppearanceError;
    let mut f = ButtonAppearanceFixture::new();
    f.frame();
    let before = f.button().resolved_appearance().unwrap().clone();
    let contract = ButtonAppearanceContract::standard();
    let mut values = button_product_values(&contract, crate::core::layout::Padding::all(40.0));
    // A noncurrent but declared case must be admitted before activation.
    values.hovered.as_mut().unwrap().surface = AppearanceSurface::Image(AppearanceImage {
        source: AppearanceImageSource::CurrentBinding { texture_id: 19 },
        uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
        tint: Color::WHITE,
    });
    let pending = button_product_style(contract.clone(), values.clone(), 1).unwrap();
    let request = AppearanceResolutionInput::new(
        before.request().input.framework(),
        ButtonAppearanceFacts {
            hovered: true,
            ..ButtonAppearanceFacts::default()
        },
        None,
    );
    let AppearanceValueResolution::Resolved(image) = pending.resolve(request) else {
        panic!("semantic Image value is complete")
    };
    assert_eq!(
        image.admission(),
        AppearanceAdmission::PendingCapabilityAdmission
    );
    assert!(matches!(
        f.button_mut().set_appearance(pending, None),
        Err(ButtonAppearanceError::PendingCapabilityAdmission {
            surface: AppearanceSurface::Image(_)
        })
    ));
    assert_eq!(f.button().resolved_appearance().unwrap(), &before);
    assert!(!f.frame().scene_built);
    assert_eq!(f.layouts.get(), 1);

    values.pressed = None;
    let failure = button_product_style(contract, values, 2).unwrap_err();
    assert_eq!(
        failure.problem,
        AppearanceProblem::Missing(MissingAppearanceValue::Paint(ButtonPaintCase::Pressed))
    );
    assert!(!f.frame().scene_built);
    assert_eq!(f.button().resolved_appearance().unwrap(), &before);
}

#[test]
fn button_appearance_child_label_keeps_typography_measurement_owner() {
    use crate::core::appearance::*;
    use crate::core::layout::Padding;
    let mut f = ButtonAppearanceFixture::new();
    let button_node = f.nodes[1];
    let child = f.tree.add_child(
        button_node,
        Box::new(crate::widgets::label::Label::new("Child").with_size(27.0)),
        None,
        0,
    );
    f.button_mut().child = Some(child);
    f.button_mut().text = None;
    f.frame();
    let child_size = f.tree.get_rect(child).unwrap().size();
    let font_size = f.tree.node(child).unwrap().widget.text_info().unwrap().1;
    assert_eq!(font_size, 27.0);
    let padding = Padding::symmetric(18.0, 11.0);
    let contract = ButtonAppearanceContract::standard();
    f.button_mut()
        .set_appearance(
            button_product_style(
                contract.clone(),
                button_product_values(&contract, padding),
                1,
            )
            .unwrap(),
            None,
        )
        .unwrap();
    let frame = f.frame();
    assert_eq!(f.tree.get_rect(child).unwrap().size(), child_size);
    assert_eq!(f.tree.get_rect(child).unwrap().x, padding.left);
    assert_eq!(f.tree.get_rect(child).unwrap().y, padding.top);
    assert_eq!(
        f.button().layout_bounds,
        Size::new(
            child_size.width + padding.horizontal(),
            child_size.height + padding.vertical()
        )
    );
    let text = button_scene_nodes(&frame)
        .into_iter()
        .find_map(|node| match node {
            crate::core::scene::SceneNode::Text(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.font_size, font_size);
    assert_eq!(text.text, "Child");
}
#[test]
fn button_appearance_scene_entry_requires_the_real_framework_publication() {
    use crate::core::app::build_scene_with_appearance;
    use crate::core::event::WindowId;
    use crate::core::layout_transition::ActiveLayoutTransitions;
    use crate::core::transition::TransitionLayer;
    use crate::widgets::button::ButtonAppearanceError;
    let mut f = ButtonAppearanceFixture::new();
    let constraints = Constraints {
        min_width: 0.0,
        max_width: 320.0,
        min_height: 0.0,
        max_height: 180.0,
    };
    f.harness.frame(
        &mut f.tree,
        f.nodes[0],
        constraints,
        &f.theme,
        &mut f.text,
        &IconRegistry::new(&[]),
    );
    assert_eq!(
        f.button().resolved_appearance(),
        Err(&ButtonAppearanceError::MissingFrameworkPublication)
    );
    let owner = f.owner(1);
    f.domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owner));
    test_begin_frame_layout_tracking();
    let scene = build_scene_with_appearance(
        &mut f.tree,
        f.nodes[0],
        constraints,
        &f.theme,
        &mut f.text,
        &IconRegistry::new(&[]),
        0,
        0.0,
        &mut TransitionLayer::default(),
        &mut ActiveLayoutTransitions::default(),
        None,
        1.0,
        None,
        None,
        false,
        &mut Vec::new(),
        None,
        &f.domains,
        WindowId::PRIMARY,
    );
    assert!(!scene.root.is_empty());
    let facts = f
        .button()
        .resolved_appearance()
        .unwrap()
        .request()
        .input
        .framework();
    assert!(facts.focused());
    assert_eq!(facts.window(), WindowId::PRIMARY);
    assert_eq!(facts.focus_domain(), None);
    assert_eq!(f.layouts.get(), 1);
}

#[cfg(debug_assertions)]
#[test]
fn button_appearance_public_frame_begin_supports_successive_layouts() {
    use crate::core::app::{EngineTicker, build_scene_with_appearance};
    use crate::core::appearance::{AppearancePrimitive, AppearanceRole, AppearanceSurface};
    use crate::core::event::{InteractionDomains, WindowId};
    use crate::core::layout_transition::ActiveLayoutTransitions;
    use crate::core::scene::SceneNode;
    use crate::core::transition::TransitionLayer;
    use crate::widgets::button::Button;

    fn collect<'a>(nodes: &'a [SceneNode], out: &mut Vec<&'a SceneNode>) {
        for node in nodes {
            out.push(node);
            match node {
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, out),
                SceneNode::Clip(clip) => collect(&clip.children, out),
                SceneNode::Scroll(scroll) => collect(&scroll.children, out),
                SceneNode::Layer(layer) => collect(&layer.children, out),
                _ => {}
            }
        }
    }

    let mut tree = WidgetTree::new();
    let root_layouts = Rc::new(Cell::new(0));
    let root = tree.create_root(Box::new(LayoutProbe::new(root_layouts.clone())), None, 0);
    let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
    let layouts = Rc::new(Cell::new(0));
    let button_slot = RectF::new(24.0, 48.0, 160.0, 48.0);
    let content = tree.add_child(
        scroll,
        Box::new(ControlOrderLayout::new(
            Size::new(400.0, 400.0),
            vec![button_slot],
            layouts.clone(),
            Rc::new(Cell::new(0)),
        )),
        None,
        0,
    );
    let button = tree.add_child(content, Box::new(Button::with_text("Run")), None, 0);
    tree.node_mut(scroll)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<ScrollArea>()
        .unwrap()
        .wire_content(content);
    let owners = [root, scroll, content, button].map(|id| tree.node(id).unwrap().id);
    let button_key = tree.node(button).unwrap().widget.id().0;
    let mut domains = InteractionDomains::new();
    domains
        .primary_state_mut()
        .focus_manager
        .set_focus(Some(owners[3]));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let start = Instant::now();
    let mut ticker = EngineTicker::new(start);
    let mut transitions = TransitionLayer::default();
    let mut active_layout = ActiveLayoutTransitions::default();
    let mut tooltips = Vec::new();
    let mut previous_epoch = 0;

    for (index, (viewport, scroll_y, label)) in [
        (Size::new(320.0, 180.0), 0.0, "Run"),
        (Size::new(360.0, 220.0), 16.0, "Run"),
        (Size::new(360.0, 220.0), 16.0, "Updated"),
    ]
    .into_iter()
    .enumerate()
    {
        if index == 1 {
            tree.node_mut(scroll)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScrollArea>()
                .unwrap()
                .scroll_to(0.0, scroll_y);
        } else if index == 2 {
            tree.node_mut(button)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
                .text = Some(label.to_string());
            tree.mark_layout_dirty(button);
        }

        let now = start + Duration::from_millis(index as u64 * 16);
        let timing = ticker.begin_frame_at(now);
        assert_eq!(timing.now, now);
        assert_eq!(timing.frame_index, index as u64);
        assert_eq!(
            timing.dt,
            Duration::from_millis(if index == 0 { 0 } else { 16 })
        );
        tree.begin_layout_transition_frame();
        transitions.advance(timing.now.duration_since(start).as_secs_f64());
        let scene = build_scene_with_appearance(
            &mut tree,
            root,
            Constraints::tight(viewport),
            &theme,
            &mut text,
            &icons,
            ticker.frame_time_ms(timing.now),
            timing.dt.as_secs_f32(),
            &mut transitions,
            &mut active_layout,
            None,
            1.0,
            None,
            None,
            domains
                .primary_state()
                .pointer_capture_manager
                .is_any_captured(),
            &mut tooltips,
            None,
            &domains,
            WindowId::PRIMARY,
        );

        assert_eq!(root_layouts.get(), index as u32 + 1);
        assert_eq!(layouts.get(), index as u32 + 1);
        let epoch = tree.node(root).unwrap().layout_epoch;
        assert!(epoch > previous_epoch);
        previous_epoch = epoch;
        assert_eq!(tree.get_cached_size(root), Some(viewport));
        assert_eq!(tree.get_cached_size(scroll), Some(viewport));
        assert_eq!(
            tree.get_rect(root),
            Some(RectF::new(0.0, 0.0, viewport.width, viewport.height))
        );
        assert_eq!(tree.get_rect(button), Some(button_slot));
        assert_eq!(tree.get_cached_size(button), Some(button_slot.size()));
        for (id, owner) in [root, scroll, content, button].into_iter().zip(owners) {
            assert_eq!(tree.node(id).unwrap().id, owner);
            assert!(!tree.node(id).unwrap().dirty_layout);
        }
        let mounted = tree
            .node(button)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<Button>()
            .unwrap();
        assert_eq!(mounted.layout_bounds, button_slot.size());
        assert_eq!(mounted.id().0, button_key);
        let appearance = mounted.resolved_appearance().unwrap();
        assert!(appearance.request().input.framework().focused());
        assert_eq!(
            appearance.request().input.framework().window(),
            WindowId::PRIMARY
        );
        let surface = appearance
            .composition()
            .iter()
            .find(|layer| layer.role == AppearanceRole::Surface)
            .unwrap();
        let AppearancePrimitive::Surface(AppearanceSurface::Gradient(gradient)) =
            &surface.primitive
        else {
            panic!("default Button must publish its Gradient surface");
        };

        let underlay = appearance
            .composition()
            .iter()
            .find(|layer| layer.role == AppearanceRole::Underlay)
            .unwrap();
        let AppearancePrimitive::Surface(AppearanceSurface::Gradient(rim)) = &underlay.primitive
        else {
            panic!("default Button must publish its Gradient underlay");
        };

        let mut nodes = Vec::new();
        collect(&scene.root, &mut nodes);
        let fill = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::GradientRect(value) if value.key == Some(button_key.into()) => {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        let outer = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::GradientRect(value)
                    if value.z_index == crate::core::paint::LAYER_CONTENT - 1 =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(outer.rect, button_slot);
        assert_eq!(&outer.gradient, rim.definition());
        assert_eq!(outer.radius, appearance.radius());
        assert_eq!(&fill.gradient, gradient.definition());
        assert!(fill.rect.x > outer.rect.x && fill.rect.y > outer.rect.y);
        assert!(fill.rect.right() < outer.rect.right());
        assert!(fill.rect.bottom() < outer.rect.bottom());
        assert!(fill.radius.is_finite() && fill.radius >= 0.0);
        assert!(fill.radius <= outer.radius);
        assert!(outer.z_index < fill.z_index);
        let text = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::Text(value) if value.text == label => Some(value),
                _ => None,
            })
            .unwrap();
        assert_eq!(text.color, appearance.content_color().into());
        let scroll = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::Scroll(value) => Some(value),
                _ => None,
            })
            .unwrap();
        assert_eq!(scroll.offset, (0.0, scroll_y));
        let viewport_rect = RectF::new(0.0, 0.0, viewport.width, viewport.height);
        assert_eq!(tree.get_viewport(root), Some(viewport_rect));
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node, SceneNode::Clip(value) if value.rect == viewport_rect))
        );
        let presented = RectF::new(
            outer.rect.x,
            outer.rect.y - scroll.offset.1,
            outer.rect.width,
            outer.rect.height,
        );
        assert_eq!(presented, RectF::new(24.0, 48.0 - scroll_y, 160.0, 48.0));
        assert!(presented.x >= viewport_rect.x && presented.right() <= viewport_rect.right());
        assert!(presented.y >= viewport_rect.y && presented.bottom() <= viewport_rect.bottom());
        assert!(text.position.x >= presented.x && text.position.x < presented.right());
        assert!(text.position.y - scroll.offset.1 >= presented.y);
        assert!(text.position.y - scroll.offset.1 < presented.bottom());
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "SINGLE-LAYOUT-PER-FRAME")]
fn button_appearance_public_frame_begin_preserves_single_layout_guard() {
    use crate::core::app::{EngineTicker, build_scene_with_appearance};
    use crate::core::event::{InteractionDomains, WindowId};
    use crate::core::layout_transition::ActiveLayoutTransitions;
    use crate::core::transition::TransitionLayer;
    use crate::widgets::button::Button;

    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(Button::with_text("Guard")), None, 0);
    let domains = InteractionDomains::new();
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    let start = Instant::now();
    let mut ticker = EngineTicker::new(start);
    let timing = ticker.begin_frame_at(start);
    let mut transitions = TransitionLayer::default();
    transitions.advance(timing.now.duration_since(start).as_secs_f64());
    let mut active_layout = ActiveLayoutTransitions::default();
    let mut tooltips = Vec::new();
    tree.begin_layout_transition_frame();
    let mut build = |tree: &mut WidgetTree| {
        build_scene_with_appearance(
            tree,
            root,
            Constraints::tight(Size::new(320.0, 180.0)),
            &theme,
            &mut text,
            &icons,
            ticker.frame_time_ms(timing.now),
            timing.dt.as_secs_f32(),
            &mut transitions,
            &mut active_layout,
            None,
            1.0,
            None,
            None,
            domains
                .primary_state()
                .pointer_capture_manager
                .is_any_captured(),
            &mut tooltips,
            None,
            &domains,
            WindowId::PRIMARY,
        )
    };
    assert!(!build(&mut tree).root.is_empty());
    tree.mark_layout_dirty(root);
    let _ = build(&mut tree);
}

fn assert_label_layout_transition_clip(new_text: &str, clipped: bool) {
    use crate::core::layout::{Alignment, Justification};
    use crate::core::scene::{SceneNode, TextNode};
    use crate::widgets::{flex::Flex, label::Label};

    fn collect(nodes: &[SceneNode], clips: &mut Vec<RectF>, texts: &mut Vec<TextNode>) {
        for node in nodes {
            match node {
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, clips, texts),
                SceneNode::Clip(clip) => {
                    clips.push(clip.rect);
                    collect(&clip.children, clips, texts);
                }
                SceneNode::Text(text) => texts.push(text.clone()),
                _ => {}
            }
        }
    }

    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(
            Flex::row()
                .with_justify_content(Justification::Center)
                .with_align_items(Alignment::Center),
        ),
        None,
        0,
    );
    let label = tree.add_child(root, Box::new(Label::new("Short")), None, 0);
    let owner = tree.widget_id_for_node(label).unwrap();
    let mut harness = RetainedSceneTestHarness::new();
    let constraints = Constraints::tight(Size::new(320.0, 80.0));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    harness.advance_transition_time(0.0);
    tree.begin_layout_transition_frame();
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let previous = tree.get_absolute_paint_rect(label).unwrap();
    let previous_root = tree.get_absolute_paint_rect(root).unwrap();
    {
        let _guard = UpdateAuthorityGuard::enter();
        tree.node_mut(label)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Label>()
            .unwrap()
            .set_text(new_text);
        tree.mark_layout_dirty(label);
    }
    tree.begin_layout_transition_frame();
    harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
    let authoritative = tree.get_absolute_paint_rect(label).unwrap();
    let layout_rect = tree.get_rect(label).unwrap();
    let absolute_xy = tree.get_absolute_xy(label).unwrap();
    let viewport = tree.get_viewport(label).unwrap();
    let layout_epoch = tree.node(label).unwrap().layout_epoch;
    assert!(authoritative.width > previous.width);

    for time in [0.025, 0.1, 0.2] {
        harness.advance_transition_time(time);
        tree.begin_layout_transition_frame();
        let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let visual = harness.interpolated_rect(owner).unwrap_or_else(|| {
            panic!(
                "missing Label transition: {previous:?} -> {authoritative:?}; root {previous_root:?} -> {:?}",
                tree.get_absolute_paint_rect(root).unwrap()
            )
        });
        assert!(visual.width > previous.width && visual.width < authoritative.width);
        let mut clips = Vec::new();
        let mut texts = Vec::new();
        collect(&frame.scene_root, &mut clips, &mut texts);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].text, new_text);
        assert!((texts[0].position.x - visual.x).abs() < 0.001);
        if clipped {
            assert_eq!(
                clips,
                vec![authoritative],
                "clip must use completed layout geometry"
            );
        } else {
            assert!(
                clips.is_empty(),
                "interpolated width must not clip newly laid out content"
            );
        }
        assert_eq!(tree.get_absolute_paint_rect(label), Some(authoritative));
        assert_eq!(tree.get_rect(label), Some(layout_rect));
        assert_eq!(tree.get_absolute_xy(label), Some(absolute_xy));
        assert_eq!(tree.get_viewport(label), Some(viewport));
        assert_eq!(tree.node(label).unwrap().layout_epoch, layout_epoch);
        assert!(!tree.node(label).unwrap().dirty_layout);
    }
}

#[test]
fn label_layout_transition_content_growth_does_not_reveal_a_clipped_suffix() {
    assert_label_layout_transition_clip("A longer status message", false);
}

#[test]
fn label_layout_transition_overflow_keeps_authoritative_clip() {
    assert_label_layout_transition_clip(
        "A much longer status message that cannot fit inside the assigned label width.",
        true,
    );
}

#[test]
fn button_appearance_text_offset_preserves_geometry_and_retained_siblings() {
    use crate::core::paragraph::TextAlign;
    use crate::core::text_engine::TextInputError;
    use crate::prelude::{Button, Label};
    for child_backed in [false, true] {
        let mut f = ButtonAppearanceFixture::new();
        if child_backed {
            let child = f
                .tree
                .add_child(f.nodes[1], Box::new(Label::new("Run")), None, 0);
            f.button_mut().child = Some(child);
            f.button_mut().text = None;
        }
        let contract = crate::core::appearance::ButtonAppearanceContract::standard();
        let values = button_product_values(
            &contract,
            crate::core::layout::Padding::new(3.0, 17.0, 7.0, 29.0),
        );
        f.button_mut()
            .set_appearance(button_product_style(contract, values, 1).unwrap(), None)
            .unwrap();
        let before = f.frame();
        let original = ui_typography_texts(&before.scene_root)
            .into_iter()
            .find(|text| text.text == "Run")
            .unwrap();
        let owner = f.owner(1);
        let rect = f.tree.get_rect(f.nodes[1]).unwrap();
        let bounds = f.button().layout_bounds;
        let layouts = f.layouts.get();
        for offset in [-1.0, 2.0, 0.0] {
            let _phase = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            assert!(f.button_mut().set_text_offset_y(offset, &mut cx).unwrap());
            assert_eq!(cx.take_widget_invalidations(), (true, false, false));
            f.tree.mark_paint_dirty(f.nodes[1]);
            drop(_phase);
            let after = f.frame();
            assert_focus_appearance_recomposition(&before, &after, &[1]);
            let moved = ui_typography_texts(&after.scene_root)
                .into_iter()
                .find(|text| text.text == "Run")
                .unwrap();
            assert_eq!(moved.position.y, original.position.y + offset);
            assert_eq!(moved.position.x, original.position.x);
            assert_eq!(moved.binding, original.binding);
            assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), rect);
            assert_eq!(f.button().layout_bounds, bounds);
            assert_eq!(f.layouts.get(), layouts);
            assert_eq!(f.owner(1), owner);
            let _phase = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            assert!(!f.button_mut().set_text_offset_y(offset, &mut cx).unwrap());
            for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    f.button_mut().set_text_offset_y(invalid, &mut cx),
                    Err(TextInputError::PositionOverflow)
                );
            }
            assert_eq!(cx.take_widget_invalidations(), (false, false, false));
            drop(_phase);
            assert!(!f.frame().scene_built);
        }
        for alignment in [TextAlign::Left, TextAlign::Center] {
            let _phase = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            assert!(f.button_mut().set_caption_alignment(alignment, &mut cx));
            assert_eq!(cx.take_widget_invalidations(), (true, false, false));
            f.tree.mark_paint_dirty(f.nodes[1]);
            drop(_phase);
            let after = f.frame();
            assert_focus_appearance_recomposition(&before, &after, &[1]);
            let caption = ui_typography_texts(&after.scene_root)
                .into_iter()
                .find(|text| text.text == "Run")
                .unwrap();
            let expected_x = match alignment {
                TextAlign::Left => f.tree.get_absolute_paint_rect(f.nodes[1]).unwrap().x + 29.0,
                TextAlign::Center => original.position.x,
                TextAlign::Right => unreachable!(),
            };
            assert_eq!(caption.position.x, expected_x);
            assert_eq!(caption.position.y, original.position.y);
            assert_eq!(caption.binding, original.binding);
            assert_eq!(f.tree.get_rect(f.nodes[1]).unwrap(), rect);
            assert_eq!(f.button().layout_bounds, bounds);
            assert_eq!(f.layouts.get(), layouts);
            assert_eq!(f.owner(1), owner);
            let _phase = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(None);
            cx.begin_widget_update_for_owner(owner);
            assert!(!f.button_mut().set_caption_alignment(alignment, &mut cx));
            assert_eq!(cx.take_widget_invalidations(), (false, false, false));
            drop(_phase);
            assert!(!f.frame().scene_built);
        }
        assert_eq!(f.clicks.get(), 0);
    }
    let mut f = ButtonAppearanceFixture::new();
    let plain = f.frame();
    let original = ui_typography_texts(&plain.scene_root)
        .into_iter()
        .find(|text| text.text == "Run")
        .unwrap();
    let mut shifted = ButtonAppearanceFixture::new();
    shifted.tree.replace_widget_only(
        shifted.nodes[1],
        Box::new(Button::with_text("Run").with_text_offset_y(-1.0).unwrap()),
    );
    let first = shifted.frame();
    let caption = ui_typography_texts(&first.scene_root)
        .into_iter()
        .find(|text| text.text == "Run")
        .unwrap();
    assert_eq!(caption.position.y, original.position.y - 1.0);
    assert_eq!(shifted.button().layout_bounds, f.button().layout_bounds);
    let mut start = ButtonAppearanceFixture::new();
    start.tree.replace_widget_only(
        start.nodes[1],
        Box::new(
            Button::with_text("Run")
                .with_padding(11.0)
                .with_caption_alignment(TextAlign::Left),
        ),
    );
    let first = start.frame();
    let caption = ui_typography_texts(&first.scene_root)
        .into_iter()
        .find(|text| text.text == "Run")
        .unwrap();
    assert_eq!(
        caption.position.x,
        start
            .tree
            .get_absolute_paint_rect(start.nodes[1])
            .unwrap()
            .x
            + 11.0,
    );
}

// Contract: default controls share a text-row reserve and contain glyphs (JC,012,092§8A).
// Risk/gap: existing Button tests miss child-backed/default-field parity and a theme font larger
// than its configured line. Minimum proof: equal natural heights and unclipped text in real Layout.
#[test]
fn button_default_row_matches_fields_and_contains_theme_font() {
    use crate::widgets::{
        button::Button, combo_box::ComboBox, label::Label, spin_box::SpinBox, text_field::TextField,
    };
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(TypographyLayout {
            id: WidgetInternalId::new(),
        }),
        None,
        0,
    );
    let standalone = tree.add_child(root, Box::new(Button::with_text("Run")), None, 0);
    let child_button = tree.add_child(root, Box::new(Button::with_text("")), None, 0);
    let label = tree.add_child(child_button, Box::new(Label::new("Apply")), None, 0);
    let button = tree
        .node_mut(child_button)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Button>()
        .unwrap();
    button.child = Some(label);
    button.text = None;
    let field = tree.add_child(root, Box::new(TextField::new().with_text("Field")), None, 0);
    let mut combo = ComboBox::simple(vec!["Choice".into()]);
    combo
        .set_input_text("Choice".into())
        .expect("unmounted initial text");
    let combo = tree.add_child(root, Box::new(combo), None, 0);
    let spin = tree.add_child(root, Box::new(SpinBox::new().with_value(75)), None, 0);
    let controls = [standalone, child_button, field, combo, spin];
    let mut theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let mut harness = RetainedSceneTestHarness::new();
    let domains = crate::core::event::InteractionDomains::new();
    for font in [theme.typography.font_size, theme.typography.font_size * 2.0] {
        theme.typography.font_size = font;
        theme.typography.input_font_size = font; // This proof explicitly compares equal text metrics.
        tree.mark_subtree_dirty(root); // Same theme-change invalidation used by App.
        let frame = harness.frame_with_appearance(
            &mut tree,
            root,
            Constraints::tight(Size::new(640.0, 420.0)),
            &theme,
            &mut text,
            &IconRegistry::new(&[]),
            &domains,
            crate::core::event::WindowId::PRIMARY,
        );
        let heights = controls.map(|node| tree.get_rect(node).unwrap().height);
        assert!(
            heights
                .iter()
                .all(|height| (height - heights[0]).abs() < 0.001)
        );
        let texts = ui_typography_texts(&frame.scene_root);
        for (node, caption) in controls
            .iter()
            .zip(["Run", "Apply", "Field", "Choice", "75"])
        {
            let rect = tree.get_rect(*node).unwrap();
            let output = texts.iter().find(|value| value.text == caption).unwrap();
            let metrics = crate::core::text_engine::TextEngine::font_metrics(&mut text, font);
            assert!(output.position.y >= rect.y + theme.geometry.text_field.padding_y - 0.001);
            assert!(
                output.position.y + metrics.ascent + metrics.descent
                    <= rect.bottom() - theme.geometry.text_field.padding_y + 0.001
            );
        }
        println!("ROW CONTRACT font={font}: natural heights={heights:?}");
    }
}

// Contract: equivalent appearance metrics preserve Layout; explicit padding retains priority (012/354).
// Risk/gap: existing standalone proofs miss default child captions during style replacement.
// Minimum proof: identical metrics keep natural height/layout; changed padding changes the box model.
#[test]
fn button_default_row_preserves_equivalent_style_and_explicit_padding() {
    use crate::core::appearance::ButtonAppearanceContract;
    let mut f = ButtonAppearanceFixture::new();
    let label = f.tree.add_child(
        f.nodes[1],
        Box::new(crate::widgets::label::Label::new("Run")),
        None,
        0,
    );
    f.button_mut().child = Some(label);
    f.button_mut().text = None;
    f.frame();
    let height = f.button().layout_bounds.height;
    let layouts = f.layouts.get();
    let contract = ButtonAppearanceContract::standard();
    let values = f.theme.button_appearance_values(&contract);
    let padding = values.metrics.unwrap().padding;
    f.button_mut()
        .set_appearance(button_product_style(contract, values, 1).unwrap(), None)
        .unwrap();
    f.frame();
    assert_eq!(f.button().layout_bounds.height, height);
    assert_eq!(f.layouts.get(), layouts);
    f.button_mut().use_default_appearance();
    f.frame();
    assert_eq!(f.button().layout_bounds.height, height);
    assert_eq!(f.layouts.get(), layouts);
    let explicit = crate::core::layout::Padding::all(9.0);
    f.button_mut().padding = Some(explicit);
    f.frame();
    assert_eq!(
        f.button().layout_bounds.height,
        height - padding.vertical() + explicit.vertical()
    );
    assert_eq!(
        f.button().resolved_appearance().unwrap().metrics().padding,
        explicit
    );
}
