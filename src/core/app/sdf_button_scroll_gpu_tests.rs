use super::*;
use crate::core::layout::{FlexParams, LayoutResult};
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::renderer::custom_render_pass::audit;
use crate::utils::id::WidgetInternalId;
use crate::widgets::scroll_area::ScrollArea;

struct SlotLayout {
    id: WidgetInternalId,
    extent: Size,
    slot: RectF,
}

impl WidgetDowncast for SlotLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SlotLayout {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let [child] = children else {
            panic!("one child slot")
        };
        cx.layout_child(*child, Constraints::tight(self.slot.size()));
        cx.set_child_rect(*child, self.slot);
        LayoutResult::new(constraints.constrain(self.extent), self.extent)
    }

    fn paint(&self, _: &mut PaintContext, _: RectF, _: &[NodeId]) {}
}

fn slot_layout(extent: Size, slot: RectF) -> Box<dyn Widget> {
    Box::new(SlotLayout {
        id: WidgetInternalId::new(),
        extent,
        slot,
    })
}

fn mount_scroll(
    tree: &mut WidgetTree,
    parent: NodeId,
    extent: Size,
    slot: RectF,
) -> (NodeId, NodeId) {
    let scroll = tree.add_child(
        parent,
        Box::new(ScrollArea::new_unwired().with_scrollbars(false)),
        None,
        0,
    );
    let content = tree.add_child(scroll, slot_layout(extent, slot), None, 0);
    scroll_area(tree, scroll).wire_content(content);
    (scroll, content)
}

fn scroll_area(tree: &mut WidgetTree, node: NodeId) -> &mut ScrollArea {
    tree.node_mut(node)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut()
        .unwrap()
}

fn scroll_to(trial: &mut ReferenceETrial, node: NodeId, x: f32, y: f32) {
    let area = scroll_area(&mut trial.tree, node);
    area.scroll_to(x, y);
    assert_eq!(area.scroll_offset(), Some((x, y)));
    trial.tree.mark_paint_dirty(node);
}

struct ScrollTrial {
    trial: ReferenceETrial,
    outer: NodeId,
    inner: Option<NodeId>,
    clicks: Rc<Cell<usize>>,
}

impl ScrollTrial {
    fn new(viewport: RectF, scale: f32, outer_slot: RectF, nested: bool) -> Self {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_align_items(Alignment::Stretch)),
            None,
            0,
        );
        let (outer, content) = mount_scroll(
            &mut tree,
            root,
            Size::new(viewport.width + if nested { 60.0 } else { 0.0 }, 1332.0),
            outer_slot,
        );
        tree.set_flex(outer, FlexParams::flexible(1.0));
        let (host, inner) = if nested {
            let clip_slot = RectF::new(190.625, 150.75, 102.75, 40.25);
            let (inner, content) =
                mount_scroll(&mut tree, content, Size::new(380.0, 330.0), clip_slot);
            let visible_slot = RectF::new(0.0, 0.0, 70.0, 25.0);
            let clip = tree.add_child(
                content,
                slot_layout(clip_slot.size(), visible_slot),
                None,
                0,
            );
            tree.set_overflow(clip, OverflowMode::Clip);
            let face_slot = RectF::new(-18.25, 4.125, 140.5, 34.25);
            let visible =
                tree.add_child(clip, slot_layout(visible_slot.size(), face_slot), None, 0);
            tree.set_overflow(visible, OverflowMode::Visible);
            (visible, Some(inner))
        } else {
            (content, None)
        };
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let mut control = Button::with_text("Early access")
            .with_enabled(false)
            .on_click(move || count.set(count.get() + 1));
        control
            .request_sdf_material(Some(ButtonSdfRestingProfile::ChampagneV4))
            .unwrap();
        let button = tree.add_child(host, Box::new(control), None, 0);
        let owner = tree.node(button).unwrap().id;

        let mut compositor = Compositor::new(viewport);
        let surface = compositor.create_surface(0, viewport, SceneKey::BASE);
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let adapter = renderer.headless_adapter_for_test();
        #[cfg(target_os = "macos")]
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        assert_ne!(adapter.device_type, wgpu::DeviceType::Cpu);
        renderer
            .register_headless_render_target(surface, physical_size(viewport, scale), scale)
            .unwrap();
        Self {
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
                theme: Theme::dark(),
                scale,
                require_image: false,
                last_material_pending: false,
            },
            outer,
            inner,
            clicks,
        }
    }
}

fn physical_size(viewport: RectF, scale: f32) -> (u32, u32) {
    (
        (viewport.width * scale).round() as u32,
        (viewport.height * scale).round() as u32,
    )
}

fn assert_no_current_material(trial: &ReferenceETrial) {
    assert!(
        trial
            .harness
            .cache
            .current_sdf_frame_inputs(&trial.tree)
            .is_empty()
    );
    let scene = &trial.scenes[&trial.surface];
    assert!(
        !leaves(&scene.root)
            .iter()
            .any(|node| matches!(node, SceneNode::Image(_)))
    );
}

fn read_current_face(
    trial: &mut ReferenceETrial,
    binding: &ImageBinding,
    raw: RectF,
    composed: RectF,
    clip: RectF,
) -> Vec<[u8; 4]> {
    let input = trial.image();
    assert_eq!(input.key.metrics().final_logical_rect, raw);
    assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(raw));
    let target = trial
        .renderer
        .custom_render_target_snapshot(trial.surface)
        .unwrap();
    let expected = target.metrics_for(composed, clip).unwrap();
    assert_eq!(input.key.metrics().activity, expected.activity);
    let visible = composed.intersection(&clip).unwrap();
    assert_eq!(
        input.key,
        input
            .key
            .clone()
            .with_composited_placement(composed, visible)
    );
    let publication = trial.harness.cache.selected_sdf_materials.as_ref().unwrap()[&trial.owner]
        .as_ref()
        .unwrap();
    assert_eq!(publication.key(), &input.key);
    assert_eq!(
        publication.status(),
        &SdfMaterialStatus::Ready {
            binding: binding.clone()
        }
    );
    assert!(
        trial
            .harness
            .cache
            .sdf_scene_custody
            .has_reference(&input.key, binding.identity())
    );
    let image = input.image.unwrap();
    let pad = 1.0 / trial.scale;
    assert_eq!(
        image.coverage,
        RectF::new(
            raw.x - pad,
            raw.y - pad,
            raw.width + pad + pad,
            raw.height + pad + pad
        ),
    );
    let local = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    let images: Vec<_> = local
        .iter()
        .filter_map(|node| match node {
            SceneNode::Image(face) => Some(face),
            _ => None,
        })
        .collect();
    let [face] = images.as_slice() else {
        panic!("expected one native material Image");
    };
    assert_eq!(
        face.texture_source,
        ImageTextureSource::Admitted(binding.identity())
    );
    assert_eq!((face.rect, face.uv_rect), (image.coverage, image.uv));
    assert!(
        local
            .iter()
            .any(|node| matches!(node, SceneNode::Text(caption)
        if caption.text == "Early access"
            && caption.z_index > face.z_index
            && caption.color == Color::from_hex(0x0A1117).into()))
    );
    trial.scenes.get_mut(&trial.surface).unwrap().clear_color =
        crate::core::scene::Color::TRANSPARENT;
    trial.pixels_in_clip(binding, clip)
}

fn pixel(pixels: &[[u8; 4]], viewport: RectF, scale: f32, x: f32, y: f32) -> [u8; 4] {
    let (width, _) = physical_size(viewport, scale);
    pixels[(y * scale) as usize * width as usize + (x * scale) as usize]
}

fn check_gold_and_caption(pixels: &[[u8; 4]], viewport: RectF, scale: f32, face: RectF) {
    let p = |x, y| pixel(pixels, viewport, scale, x, y);
    let y = face.y + face.height * 0.46;
    let gold = p(face.x + 24.0, y);
    assert!(
        gold[0] > gold[1] && gold[1] > gold[2] && gold[3] == 255,
        "{gold:?}"
    );
    assert_eq!(p(face.x + 1.0, face.y + 1.0), [0; 4], "rounded corner");
    assert_eq!(p(face.x + 5.0, face.y + 5.0)[3], 255, "radius-8 shoulder");
    let linear = |encoded: u8| {
        let srgb = f32::from(encoded) / 255.0;
        if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    };
    // A narrow rim can have coverage only at a translucent pixel center.
    let rim_green = (-1..=2)
        .map(|i| p(((face.x * scale).floor() + i as f32 + 0.5) / scale, y))
        .filter(|pixel| pixel[3] > 32)
        .map(|pixel| linear(pixel[1]) / (f32::from(pixel[3]) / 255.0))
        .fold(0.0_f32, f32::max);
    assert!(
        rim_green > linear(gold[1]) + 0.015,
        "the warm inset edge is visible"
    );

    let (width, _) = physical_size(viewport, scale);
    let left = ((face.x + face.width * 0.24) * scale) as usize;
    let right = ((face.x + face.width * 0.76) * scale) as usize;
    let top = ((face.y + 8.0) * scale) as usize;
    let bottom = ((face.bottom() - 8.0) * scale) as usize;
    assert!(
        (top..bottom).any(|row| {
            pixels[row * width as usize + left..row * width as usize + right]
                .iter()
                .any(|p| p[0] < 76 && p[1] < 76 && p[3] == 255)
        }),
        "dark native caption ink above the golden face"
    );
}

#[test]
fn canonical_champagne_v4_scroll_reveals_disabled_material() {
    for (width, height, scale, initially_visible) in [
        (320.0, 568.0, 1.25, false),
        (320.0, 568.0, 2.0, false),
        (390.0, 844.0, 2.5, false),
        (320.0, 1350.0, 1.25, true),
        (1600.0, 900.0, 1.0, true),
    ] {
        let viewport = RectF::new(0.0, 0.0, width, height);
        let raw = RectF::new(
            12.375,
            if width > 1000.0 { 750.25 } else { 920.25 },
            (width - 24.75).min(296.0),
            42.25,
        );
        let ScrollTrial {
            mut trial,
            outer,
            clicks,
            ..
        } = ScrollTrial::new(viewport, scale, raw, false);
        let textures = audit::snapshot().custom_textures;
        trial.frame();
        let root_owner = trial.tree.node(trial.root).unwrap().id;
        let root_paint = trial.harness.local_fragment(root_owner).unwrap();
        let scroll = if initially_visible {
            0.0
        } else {
            assert_no_current_material(&trial);
            assert_eq!(audit::snapshot().custom_textures, textures);
            let offset = raw.y - (height - 196.125);
            scroll_to(&mut trial, outer, 0.0, offset);
            trial.frame();
            assert_eq!(
                trial
                    .harness
                    .cache
                    .current_sdf_frame_inputs(&trial.tree)
                    .len(),
                1
            );
            offset
        };
        let binding = trial.settle(false);
        let placed = raw.translate(0.0, -scroll);
        let pixels = read_current_face(&mut trial, &binding, raw, placed, viewport);
        check_gold_and_caption(&pixels, viewport, scale, placed);
        assert_eq!(preparations(&trial), 1);
        assert!(Arc::ptr_eq(
            &root_paint,
            &trial.harness.local_fragment(root_owner).unwrap()
        ));
        let center = Point::new(
            placed.x + placed.width * 0.5,
            placed.y + placed.height * 0.5,
        );
        trial.pointer_at(true, center);
        trial.pointer_at(false, center);
        enter_key(&mut trial, true, false);
        enter_key(&mut trial, false, false);
        assert_eq!(clicks.get(), 0);
        assert_eq!(trial.domains.primary_state().focus_manager.focused(), None);
        assert_eq!(
            trial
                .domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );

        if !initially_visible {
            let before = trial.image().key;
            scroll_to(&mut trial, outer, 0.0, scroll + 0.03125);
            trial.frame();
            let moved = trial.image();
            assert_ne!(
                moved.key, before,
                "fractional placement is part of the current K"
            );
            assert_eq!(moved.key.metrics().activity, before.metrics().activity);
            assert_eq!(moved.image.unwrap().binding.identity(), binding.identity());
            assert_eq!(preparations(&trial), 1);
            assert!(
                !trial
                    .harness
                    .cache
                    .sdf_scene_custody
                    .has_reference(&before, binding.identity())
            );

            scroll_to(&mut trial, outer, 0.0, 0.0);
            trial.frame();
            assert_no_current_material(&trial);
            scroll_to(&mut trial, outer, 0.0, scroll);
            let reentry = trial.settle(false);
            assert_ne!(reentry.identity(), binding.identity());
            let pixels = read_current_face(&mut trial, &reentry, raw, placed, viewport);
            check_gold_and_caption(&pixels, viewport, scale, placed);
        }
        eprintln!("{width}x{height} DPR {scale}: authentic ChampagneV4 face and dark caption");
    }
}

#[test]
fn canonical_champagne_v4_nested_scroll_keeps_composition_and_clips() {
    let viewport = RectF::new(0.0, 0.0, 320.0, 568.0);
    let scale = 1.25;
    let inner_slot = RectF::new(31.375, 820.5, 220.5, 106.25);
    let ScrollTrial {
        mut trial,
        outer,
        inner,
        ..
    } = ScrollTrial::new(viewport, scale, inner_slot, true);
    trial.frame();
    assert_no_current_material(&trial);
    let raw = RectF::new(203.75, 975.375, 140.5, 34.25);
    let legacy_viewport = trial.tree.get_viewport(trial.button);
    scroll_to(&mut trial, outer, 13.125, 650.25);
    scroll_to(&mut trial, inner.unwrap(), 141.5, 80.0);
    trial.frame();
    assert_eq!(
        trial
            .harness
            .cache
            .current_sdf_frame_inputs(&trial.tree)
            .len(),
        1
    );
    let binding = trial.settle(false);
    let placed = RectF::new(49.125, 245.125, 140.5, 34.25);
    let clip = RectF::new(67.375, 241.0, 102.75, 35.5);
    let pixels = read_current_face(&mut trial, &binding, raw, placed, clip);
    assert_eq!(trial.tree.get_viewport(trial.button), legacy_viewport);
    assert_eq!(
        trial.image().key.metrics().final_logical_viewport,
        legacy_viewport.unwrap()
    );
    for (x, y) in [
        (clip.x - 2.0, 258.0),
        (clip.right() + 2.0, 258.0),
        (104.0, clip.bottom() + 1.5),
    ] {
        assert!(placed.contains_point(x, y));
        assert_eq!(pixel(&pixels, viewport, scale, x, y), [0; 4]);
        assert_ne!(
            trial.tree.hit_test(x, y).map(|hit| hit.widget_id),
            Some(trial.owner)
        );
    }
    // Beyond the Visible host's own allocation, but still inside both scrollports and Clip.
    let inside = pixel(&pixels, viewport, scale, 154.0, 259.0);
    assert!(inside[0] > inside[1] && inside[1] > inside[2] && inside[3] == 255);
}
