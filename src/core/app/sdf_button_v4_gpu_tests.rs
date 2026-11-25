use super::*;
use crate::core::paint::{LAYER_CONTENT, LAYER_FOCUS};
use crate::renderer::sdf_material_status::{
    ChampagneV4Basis, SdfMaterialSource, SdfMaterialStatus, SdfMaterialUnavailableCause,
};

#[path = "sdf_button_scroll_gpu_tests.rs"]
mod scroll;

fn preparations(trial: &ReferenceETrial) -> u64 {
    trial
        .harness
        .cache
        .sdf_material_staging
        .as_ref()
        .unwrap()
        .preparation_totals_for_test()
        .0
}

fn enter_key(trial: &mut ReferenceETrial, down: bool, repeat: bool) {
    let time = Instant::now();
    let event = if down {
        FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat,
        }
    } else {
        FusedInputEvent::KeyUp {
            window_id: WindowId::PRIMARY,
            time,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKeyCode::Enter,
            reservation: None,
            modifiers: Modifiers::default(),
        }
    };
    trial.router.route_input_event(
        &mut trial.tree,
        event,
        Some(trial.root),
        trial.renderer.text_engine_mut(),
        None,
        trial.domains.primary_state_mut(),
        &mut trial.bindings,
    );
}

fn set_enabled(trial: &mut ReferenceETrial, enabled: bool) {
    trial
        .tree
        .request_widget_enabled(trial.button, enabled)
        .unwrap();
    let result = crate::core::widget_availability::apply_pending_widget_availability(
        &mut trial.tree,
        &mut trial.router,
        &mut trial.fusion,
        trial.domains.primary_state_mut(),
        &mut trial.bindings,
        trial.renderer.text_engine_mut(),
    );
    assert_eq!(result.changed.len(), 1);
    assert!(result.rejected.is_empty());
}

fn read_frame(
    trial: &mut ReferenceETrial,
    binding: &ImageBinding,
    state: &str,
    face: RectF,
    label_position: Point,
) -> Vec<[u8; 4]> {
    assert!(
        !trial.last_material_pending,
        "unexpected Pending at {state}"
    );
    let material = trial.image();
    let status = trial.harness.cache.selected_sdf_materials.as_ref().unwrap()[&trial.owner]
        .as_ref()
        .unwrap();
    assert_eq!(status.key(), &material.key);
    assert_eq!(
        status.status(),
        &SdfMaterialStatus::Ready {
            binding: binding.clone()
        }
    );
    assert_eq!(material.key.metrics().final_logical_rect, face);
    assert_eq!(trial.tree.get_absolute_paint_rect(trial.button), Some(face));
    let local = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    let images: Vec<_> = local
        .iter()
        .filter_map(|node| match node {
            SceneNode::Image(image) => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0].texture_source,
        ImageTextureSource::Admitted(binding.identity())
    );
    assert_eq!(images[0].z_index, LAYER_CONTENT);
    let label = local
        .iter()
        .find_map(|node| match node {
            SceneNode::Text(text) if text.text == "Stage" => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (label.position, label.color),
        (label_position, Color::from_hex(0x0A1117).into())
    );
    assert!(label.z_index > images[0].z_index);
    let pixels = trial.pixels(binding);
    let scale = trial.scale;
    let width = (240.0 * scale) as usize;
    // The center lies well inside the opaque surface, so dark pixels here are glyph ink.
    let ink = ((face.y + 7.0) * scale) as usize..((face.y + 23.0) * scale) as usize;
    assert!(
        ink.into_iter().any(|y| {
            let left = ((face.x + face.width * 0.32) * scale) as usize;
            let right = ((face.x + face.width * 0.68) * scale) as usize;
            pixels[y * width + left..y * width + right]
                .iter()
                .any(|p| p[0] < 80 && p[1] < 80)
        }),
        "expected dark glyph ink on the physical {state} face"
    );
    pixels
}

fn save_frame(
    trial: &mut ReferenceETrial,
    binding: &ImageBinding,
    directory: &std::path::Path,
    state: &str,
    face: RectF,
    label_position: Point,
) -> Vec<[u8; 4]> {
    let pixels = read_frame(trial, binding, state, face, label_position);
    let scale = trial.scale;
    let width = (240.0 * scale) as usize;
    let height = (100.0 * scale) as usize;
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    ppm.extend(pixels.iter().flat_map(|pixel| pixel[..3].iter().copied()));
    std::fs::write(directory.join(format!("{scale}x-{state}.ppm")), ppm).unwrap();
    pixels
}

#[test]
fn canonical_button_sdf_champagne_v4_states_and_custody() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/champagne-v4");
    std::fs::create_dir_all(&directory).unwrap();
    for scale in [1.0, 2.0] {
        let (mut trial, clicks, peer) = ReferenceETrial::new(true, scale, 120.0, false);
        let adapter = trial.renderer.headless_adapter_for_test();
        #[cfg(target_os = "macos")]
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        assert_ne!(adapter.device_type, wgpu::DeviceType::Cpu);
        eprintln!(
            "ChampagneV4 {scale}x on {:?}: {}",
            adapter.backend, adapter.name
        );
        let request_profile = |tree: &mut WidgetTree, node| {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
                .request_sdf_material(Some(ButtonSdfRestingProfile::ChampagneV4))
                .unwrap();
        };
        request_profile(&mut trial.tree, trial.button);
        trial.router.request_focus_for_widget(
            &mut trial.tree,
            trial.owner,
            trial.domains.primary_state_mut(),
        );
        trial.frame();
        let pending = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
        assert!(
            !pending
                .iter()
                .any(|node| matches!(node, SceneNode::Image(_)))
        );
        assert!(
            pending
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage"))
        );
        assert!(
            pending
                .iter()
                .any(|node| matches!(node, SceneNode::BorderRect(cue)
            if cue.z_index == LAYER_FOCUS + 1 && cue.radius == 8.0))
        );
        trial
            .router
            .clear_focus(&mut trial.tree, trial.domains.primary_state_mut());
        let rest = trial.settle(false);
        let rest_request = trial.request();
        let current_key = trial.image().key;
        assert_eq!(
            rest_request.source,
            SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Enabled)
        );
        assert!(!rest_request.hovered);
        assert_eq!(trial.image().image.unwrap().uv.y, 0.0);
        let rect = trial.tree.get_absolute_paint_rect(trial.button).unwrap();
        assert_eq!(rect, RectF::new(20.0, 20.0, 120.0, 30.0));
        let position = leaves(&trial.harness.local_fragment(trial.owner).unwrap())
            .iter()
            .find_map(|node| match node {
                SceneNode::Text(t) => Some(t.position),
                _ => None,
            })
            .unwrap();
        let peer_fragment = trial.harness.local_fragment(peer).unwrap();
        let rest_pixels = save_frame(&mut trial, &rest, &directory, "rest", rect, position);
        let pixel = |pixels: &[[u8; 4]], x: f32, y: f32| {
            pixels[(y * scale) as usize * (240.0 * scale) as usize + (x * scale) as usize]
        };
        // A radius-3 corner would cover this pixel. Radius 8 leaves it outside F's ink.
        assert_eq!(
            pixel(&rest_pixels, 21.0, 21.0),
            pixel(&rest_pixels, 10.0, 10.0)
        );
        assert_ne!(
            pixel(&rest_pixels, 25.0, 22.0),
            pixel(&rest_pixels, 10.0, 10.0)
        );
        for (sample_y, lo, hi) in [
            (23.0, (0.0, 0xF1D1A0), (0.22, 0xEBC597)),
            (33.0, (0.22, 0xEBC597), (0.66, 0xD3A776)),
            (48.0, (0.66, 0xD3A776), (1.0, 0xC69662)),
        ] {
            let center_y = ((sample_y * scale).floor() + 0.5) / scale;
            let fraction = ((center_y - rect.y) / rect.height - lo.0) / (hi.0 - lo.0);
            let low = Color::from_hex(lo.1);
            let high = Color::from_hex(hi.1);
            let expected = [(low.r, high.r), (low.g, high.g), (low.b, high.b)].map(|(a, b)| {
                (crate::core::scene::linear_to_srgb(a + (b - a) * fraction) * 255.0).round() as u8
            });
            let actual = pixel(&rest_pixels, 117.0, sample_y);
            assert!(
                expected
                    .into_iter()
                    .zip(actual)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "linear gradient at {scale}x y={sample_y}: {actual:?} vs {expected:?}"
            );
        }

        let warm_count = preparations(&trial);
        let inside = Point::new(rect.x + 1.0, 35.0);
        let outside = Point::new(rect.x - 1.0, 35.0);
        trial.hover_at(inside);
        trial.frame();
        let hover = trial.image().image.unwrap();
        assert_eq!(hover.binding, rest);
        assert_eq!((hover.uv.x, hover.uv.y), (0.0, 0.5));
        assert_eq!(trial.image().key, current_key);
        assert_eq!(preparations(&trial), warm_count);
        let hover_request = trial.request();
        assert!(hover_request.hovered);
        assert_eq!(hover_request.source, rest_request.source);
        assert_eq!(hover_request.basis_revision, rest_request.basis_revision);
        assert!(hover_request.revision > rest_request.revision);
        let hover_pixels = save_frame(&mut trial, &rest, &directory, "hover", rect, position);
        assert!(pixel(&hover_pixels, 43.0, 24.0)[1] > pixel(&rest_pixels, 43.0, 24.0)[1] + 3);
        // Sample every physical pixel in the middle 80% of the face, away from the rim.
        let width = (240.0 * scale) as usize;
        let middle = || {
            ((23.0 * scale) as usize..(47.0 * scale) as usize).flat_map(|y| {
                ((32.0 * scale) as usize..(128.0 * scale) as usize).map(move |x| y * width + x)
            })
        };
        let middle_count = middle().count();
        let luma =
            |[r, g, b, _]: [u8; 4]| 2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b);
        let brighter = middle()
            .filter(|&i| luma(hover_pixels[i]) > luma(rest_pixels[i]))
            .count();
        assert!(brighter * 5 > middle_count * 4, "{scale}x Hover body");

        // Match the 166 captured frames and 18 crossings in the consumer
        // witness. Read back the first frame after every change and all
        // intervening frames without waiting for another admission.
        for frame in 0..166 {
            let hovered = (frame / 9) % 2 == 0;
            trial.hover_at(if hovered { inside } else { outside });
            trial.frame();
            let input = trial.image();
            assert_eq!(input.key, current_key, "frame {frame}");
            let image = input.image.unwrap();
            assert_eq!(image.binding, rest, "frame {frame}");
            assert_eq!(
                (image.uv.x, image.uv.y),
                (0.0, if hovered { 0.5 } else { 0.0 })
            );
            assert_eq!(trial.request().hovered, hovered);
            assert_eq!(preparations(&trial), warm_count);
            let pixels = read_frame(&mut trial, &rest, "hover transition", rect, position);
            if frame == 9 {
                assert!(pixels == rest_pixels, "{scale}x first Rest frame changed");
            }
            let expected = if hovered { &hover_pixels } else { &rest_pixels };
            for (x, y) in [(43.0, 24.0), (117.0, 35.0)] {
                assert_eq!(pixel(&pixels, x, y), pixel(expected, x, y), "frame {frame}");
            }
        }
        eprintln!(
            "ChampagneV4 {scale}x: 166 rendered frames, 18 crossings, no face or caption dropouts"
        );

        let hover_request = trial.request();
        trial.frame();
        assert_eq!(trial.request(), hover_request);
        assert_eq!(preparations(&trial), warm_count);
        assert_eq!(trial.image().image.unwrap().binding, rest);
        trial.pointer(true);
        trial.frame();
        let pressed = trial.image().image.unwrap();
        assert_eq!(pressed.binding, rest);
        assert_eq!((pressed.uv.x, pressed.uv.y), (0.5, 0.5));
        assert_eq!(trial.image().key, current_key);
        assert_eq!(trial.request().basis_revision, hover_request.basis_revision);
        assert!(trial.request().revision > hover_request.revision);
        assert_eq!(clicks.get(), 0);
        assert_eq!(
            trial
                .domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(trial.owner)
        );
        let pressed_pixels = save_frame(&mut trial, &rest, &directory, "pressed", rect, position);
        assert!(pixel(&pressed_pixels, 117.0, 46.0)[0] + 8 < pixel(&hover_pixels, 117.0, 46.0)[0]);
        let held_deltas =
            middle().map(|i| luma(hover_pixels[i]) as i32 - luma(pressed_pixels[i]) as i32);
        let darker = held_deltas.clone().filter(|&delta| delta > 0).count();
        let darker_by_15 = held_deltas.filter(|&delta| delta >= 15 * 10_000).count();
        assert!(darker * 5 > middle_count * 4, "{scale}x Held body");
        assert!(darker_by_15 * 2 >= middle_count, "{scale}x Held contrast");
        trial.pointer(false);
        trial.frame();
        assert_eq!(trial.image().image.unwrap().uv.x, 0.0);
        assert_eq!(clicks.get(), 1);
        assert_eq!(preparations(&trial), warm_count);

        trial.router.request_focus_for_widget(
            &mut trial.tree,
            trial.owner,
            trial.domains.primary_state_mut(),
        );
        trial.frame();
        let focused = save_frame(&mut trial, &rest, &directory, "focus", rect, position);
        assert_eq!(
            pixel(&focused, 21.0, 21.0),
            pixel(&hover_pixels, 21.0, 21.0)
        );
        assert_ne!(
            pixel(&focused, 80.0, 20.0),
            pixel(&hover_pixels, 80.0, 20.0)
        );
        trial.hover_at(outside);
        enter_key(&mut trial, true, false);
        trial.frame();
        let image = trial.image().image.unwrap();
        assert_eq!(image.binding, rest);
        assert_eq!((image.uv.x, image.uv.y), (0.5, 0.0));
        assert_eq!(trial.image().key, current_key);
        let focused = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
        assert!(
            focused
                .iter()
                .any(|node| matches!(node, SceneNode::BorderRect(cue)
            if cue.z_index == LAYER_FOCUS + 1 && cue.rect == rect && cue.radius == 8.0))
        );
        let rest_pressed = save_frame(
            &mut trial,
            &rest,
            &directory,
            "keyboard-focus-pressed",
            rect,
            position,
        );
        assert!(pixel(&rest_pressed, 117.0, 46.0)[0] + 8 < pixel(&rest_pixels, 117.0, 46.0)[0]);
        trial.hover_at(inside);
        trial.frame();
        let image = trial.image().image.unwrap();
        assert_eq!(image.binding, rest);
        assert_eq!((image.uv.x, image.uv.y), (0.5, 0.5));
        let hover_pressed = read_frame(&mut trial, &rest, "keyboard hover pressed", rect, position);
        // The pressed sheen is deliberately subtle. Compare its region so
        // an individual 8-bit sample rounding to the same value is allowed.
        let sheen = |pixels: &[[u8; 4]]| {
            (24..32)
                .flat_map(|y| (30..61).map(move |x| pixel(pixels, x as f32, y as f32)[1] as u32))
                .sum::<u32>()
        };
        assert!(sheen(&hover_pressed) > sheen(&rest_pressed));
        assert_eq!(trial.image().key, current_key);
        enter_key(&mut trial, true, true);
        assert_eq!(clicks.get(), 1);
        enter_key(&mut trial, false, false);
        trial.frame();
        assert_eq!(trial.image().image.unwrap().uv.x, 0.0);
        assert_eq!(clicks.get(), 2);
        assert_eq!(preparations(&trial), warm_count);
        assert!(Arc::ptr_eq(
            &peer_fragment,
            &trial.harness.local_fragment(peer).unwrap()
        ));

        trial.pointer(true);
        trial.hover_at(Point::new(239.0, 99.0));
        trial.pointer_at(false, Point::new(239.0, 99.0));
        trial.frame();
        assert_eq!(clicks.get(), 2);
        assert_eq!(trial.image().image.unwrap().uv.x, 0.0);
        assert_eq!(preparations(&trial), warm_count);
        // Hover resolves again on the first uncaptured move after release.
        trial.hover_at(Point::new(238.0, 98.0));
        trial.frame();
        assert_eq!(
            trial.request().source,
            SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Enabled)
        );
        assert!(!trial.request().hovered);
        let exited = trial.image();
        assert_eq!(exited.key, current_key);
        assert_eq!(exited.image.unwrap().binding, rest);
        assert_eq!(preparations(&trial), warm_count);

        enter_key(&mut trial, true, false);
        trial.pointer(true);
        set_enabled(&mut trial, false);
        let disabled = trial.settle(false);
        assert_ne!(disabled.identity(), rest.identity());
        assert_ne!(trial.image().key, current_key);
        assert_eq!(
            trial.request().source,
            SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Disabled)
        );
        assert_eq!(
            trial.image().image.unwrap().uv,
            RectF::new(0.0, 0.0, 1.0, 1.0)
        );
        assert_eq!(
            trial
                .domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert_ne!(
            trial.domains.primary_state().focus_manager.focused(),
            Some(trial.owner)
        );
        trial.hover_at(Point::new(80.0, 35.0));
        trial.pointer(true);
        trial.pointer(false);
        enter_key(&mut trial, false, false);
        enter_key(&mut trial, true, false);
        enter_key(&mut trial, false, false);
        trial.frame();
        assert_eq!(clicks.get(), 2);
        assert_eq!(trial.image().image.unwrap().binding, disabled);
        let disabled_pixels = save_frame(
            &mut trial, &disabled, &directory, "disabled", rect, position,
        );
        assert!(pixel(&disabled_pixels, 43.0, 24.0)[1] + 3 < pixel(&rest_pixels, 43.0, 24.0)[1]);

        set_enabled(&mut trial, true);
        trial.hover_at(Point::new(239.0, 99.0));
        let enabled = trial.settle(false);
        assert_ne!(enabled.identity(), disabled.identity());
        trial.pixels(&enabled);
        let holder = trial.tree.children(trial.root)[0];
        crate::replace_widget!(
            trial.tree,
            holder,
            Container::empty().with_width(144.0).with_height(30.0)
        );
        trial.tree.mark_layout_dirty(trial.root);
        let resized = trial.settle(false);
        assert_ne!(resized.identity(), enabled.identity());
        assert_eq!(trial.image().key.owner(), trial.owner);
        assert_eq!(trial.image().key.metrics().final_logical_rect.width, 144.0);
        trial.pixels(&resized);

        let retired = trial.owner;
        trial.tree.remove_node(trial.button);
        trial.button = trial
            .tree
            .add_child(holder, Box::new(Button::with_text("Stage")), None, 0);
        trial.owner = trial.tree.node(trial.button).unwrap().id;
        request_profile(&mut trial.tree, trial.button);
        let remounted = trial.settle(false);
        assert_ne!(trial.owner, retired);
        assert_ne!(remounted.identity(), resized.identity());
        assert_eq!(trial.image().key.owner(), trial.owner);
        assert_eq!(trial.image().key.metrics().surface_id, trial.surface);

        trial.router.request_focus_for_widget(
            &mut trial.tree,
            trial.owner,
            trial.domains.primary_state_mut(),
        );
        trial
            .renderer
            .unregister_render_target(trial.surface)
            .unwrap();
        trial
            .renderer
            .register_headless_render_target(trial.surface, (360, 150), 1.5)
            .unwrap();
        trial.scale = 1.5;
        trial.frame();
        if !trial.last_material_pending {
            assert_ne!(
                trial
                    .image()
                    .image
                    .as_ref()
                    .map(|image| image.binding.identity()),
                Some(remounted.identity())
            );
        }
        let fractional = trial.settle(false);
        assert_ne!(fractional.identity(), remounted.identity());
        assert_eq!(trial.image().key.metrics().scale_factor, 1.5);
        assert_eq!(trial.image().key.revision(), trial.request().basis_revision);
        trial.pixels(&fractional);
        let count = preparations(&trial);
        trial.frame();
        assert_eq!(trial.image().image.unwrap().binding, fractional);
        assert_eq!(preparations(&trial), count);

        trial
            .renderer
            .unregister_render_target(trial.surface)
            .unwrap();
        trial
            .renderer
            .register_headless_render_target(trial.surface, (1020, 425), 4.25)
            .unwrap();
        trial.scale = 4.25;
        let textures = crate::renderer::custom_render_pass::audit::snapshot().custom_textures;
        trial.frame();
        let fallback = trial.image();
        assert_eq!(fallback.key.metrics().scale_factor, 4.25);
        assert!(fallback.image.is_none());
        assert_eq!(preparations(&trial), count);
        assert_eq!(
            crate::renderer::custom_render_pass::audit::snapshot().custom_textures,
            textures
        );
        assert!(matches!(
            trial.harness.cache.selected_sdf_materials.as_ref().unwrap()[&trial.owner]
                .as_ref()
                .unwrap()
                .status(),
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::UnsupportedScale
            }
        ));
        let local = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
        assert!(
            local
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text) if text.text == "Stage"))
        );
        assert!(
            local
                .iter()
                .any(|node| matches!(node, SceneNode::BorderRect(cue)
            if cue.z_index == LAYER_FOCUS + 1 && cue.radius == 8.0))
        );
        assert!(
            !local
                .iter()
                .any(|node| matches!(node, SceneNode::Image(_) | SceneNode::GradientRect(_)))
        );
    }
}

#[test]
fn canonical_button_sdf_champagne_v4_fractional_coverage_and_disabled_cta() {
    let viewport = RectF::new(0.0, 0.0, 320.0, 112.0);
    let face = RectF::new(13.375, 17.25, 290.375, 38.25);
    let center = Point::new(face.x + face.width * 0.5, face.y + face.height * 0.5);
    let (mut trial, clicks, _) = ReferenceETrial::new(true, 1.0, face.width, false);
    let adapter = trial.renderer.headless_adapter_for_test();
    #[cfg(target_os = "macos")]
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    assert_ne!(adapter.device_type, wgpu::DeviceType::Cpu);
    let holder = trial.tree.children(trial.root)[0];
    crate::replace_widget!(
        trial.tree,
        trial.root,
        Flex::column()
            .with_padding(EdgeInsets::new(face.y, 12.0, 12.0, face.x))
            .with_spacing(16.0)
            .with_align_items(Alignment::Start)
            .with_background(Color::TRANSPARENT)
    );
    crate::replace_widget!(
        trial.tree,
        holder,
        Container::empty()
            .with_width(face.width)
            .with_height(face.height)
    );
    trial.tree.mark_layout_dirty(trial.root);
    trial.compositor.set_surface_bounds(trial.surface, viewport);
    trial
        .tree
        .node_mut(trial.button)
        .unwrap()
        .widget
        .as_any_mut()
        .downcast_mut::<Button>()
        .unwrap()
        .request_sdf_material(Some(ButtonSdfRestingProfile::ChampagneV4))
        .unwrap();

    let target = |trial: &mut ReferenceETrial, scale: f32| {
        trial
            .renderer
            .unregister_render_target(trial.surface)
            .unwrap();
        trial
            .renderer
            .register_headless_render_target(
                trial.surface,
                (
                    (viewport.width * scale) as u32,
                    (viewport.height * scale) as u32,
                ),
                scale,
            )
            .unwrap();
        trial.scale = scale;
    };
    let read = |trial: &mut ReferenceETrial, binding: &ImageBinding| {
        trial.scenes.get_mut(&trial.surface).unwrap().clear_color =
            crate::core::scene::Color::TRANSPARENT;
        trial.pixels(binding)
    };
    let srgb = trial.renderer.gpu_resources().format.is_srgb();
    let linear = |v: u8| {
        let value = f32::from(v) / 255.0;
        if srgb {
            crate::core::paint::srgb_to_linear(value)
        } else {
            value
        }
    };
    let mut previous = None;
    for scale in [0.5_f32, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0] {
        target(&mut trial, scale);
        trial.frame();
        trial.hover_at(center);
        let binding = trial.settle(false);
        let input = trial.image();
        let metrics = input.key.metrics();
        assert_eq!(metrics.scale_factor, scale);
        assert_eq!(metrics.final_logical_rect, face);
        if let Some((key, identity)) = previous.replace((input.key.clone(), binding.identity())) {
            assert_ne!(key, input.key);
            assert_ne!(identity, binding.identity());
            assert_eq!(key.revision(), input.key.revision());
        }
        let image = input.image.unwrap();
        assert_eq!((image.uv.x, image.uv.y), (0.0, 0.5));
        let pad = 1.0 / scale;
        let coverage = RectF::new(
            face.left() - pad,
            face.top() - pad,
            face.width + pad + pad,
            face.height + pad + pad,
        );
        assert_eq!(image.coverage, coverage);
        let width = (viewport.width * scale) as usize;
        let pixel = |pixels: &[[u8; 4]], x: f32, y: f32| {
            pixels[(y * scale) as usize * width + (x * scale) as usize]
        };
        let hover = read(&mut trial, &binding);
        let inset = 1.0 / scale;
        assert_eq!(
            pixel(
                &hover,
                face.left() + inset * 0.25,
                face.top() + inset * 0.25
            ),
            [0; 4]
        );
        assert_eq!(
            pixel(
                &hover,
                face.right() - inset * 0.25,
                face.top() + inset * 0.25
            ),
            [0; 4]
        );
        assert_eq!(pixel(&hover, face.left() + 8.0, face.top() + 6.0)[3], 255);

        // Four flat sections must reach F and leave alpha at zero outside E.
        let scans = [
            (true, face.left(), center.y, -1.0),
            (true, face.right(), center.y, 1.0),
            (false, face.top(), face.right() - 20.0, -1.0),
            (false, face.bottom(), face.right() - 20.0, 1.0),
        ];
        let mut translucent = 0;
        for (horizontal, edge, along, outward) in scans {
            for offset in -3..=3 {
                let pos = (edge * scale).floor() + offset as f32 + 0.5;
                let distance = (pos - edge * scale) * outward;
                let p = if horizontal {
                    pixel(&hover, pos / scale, along)
                } else {
                    pixel(&hover, along, pos / scale)
                };
                if distance >= 1.0 {
                    assert_eq!(p, [0; 4], "{scale}x outside E at {distance}");
                } else if distance <= -1.0 {
                    assert_eq!(p[3], 255, "{scale}x inside F at {distance}");
                }
                if p[3] > 0 && p[3] < 255 {
                    translucent += 1;
                    let alpha = f32::from(p[3]) / 255.0;
                    assert!(
                        p[..3]
                            .iter()
                            .all(|&channel| linear(channel) <= alpha + 0.012)
                    );
                }
            }
        }
        assert!(
            translucent > 0,
            "{scale}x must have real premultiplied edge coverage"
        );

        let x = face.right() - 20.0;
        let y = face.top() + face.height * 0.43;
        let body = pixel(&hover, x, y);
        let sample_y = ((y * scale).floor() + 0.5) / scale;
        let t = ((sample_y - face.y) / face.height - 0.22) / (0.66 - 0.22);
        assert_eq!(body[3], 255);
        let light = Color::from_hex(0xEBC597);
        let dark = Color::from_hex(0xD3A776);
        for ((a, b), value) in [(light.r, dark.r), (light.g, dark.g), (light.b, dark.b)]
            .into_iter()
            .zip(body)
        {
            assert!(
                (linear(value) - (a + (b - a) * t) * 1.08).abs() <= 0.018,
                "{scale}x linear gold with Hover lift"
            );
        }
        let rim = (-2..=1)
            .map(|i| {
                let px = ((face.right() * scale).floor() + i as f32 + 0.5) / scale;
                pixel(&hover, px, y)
            })
            .filter(|p| p[3] > 32)
            .map(|p| linear(p[1]) / (f32::from(p[3]) / 255.0))
            .fold(0.0_f32, f32::max);
        assert!(
            rim > linear(body[1]) + 0.025,
            "{scale}x inset edge remains visible"
        );

        let count = preparations(&trial);
        trial.frame();
        assert_eq!(trial.image().image.unwrap().binding, binding);
        assert_eq!(preparations(&trial), count);
        // Keep the actual focus cue equal on both sides of the cell switch.
        trial.router.request_focus_for_widget(
            &mut trial.tree,
            trial.owner,
            trial.domains.primary_state_mut(),
        );
        trial.frame();
        assert_eq!(trial.image().image.unwrap().binding, binding);
        let focused_pixels = read(&mut trial, &binding);
        trial.hover_at(Point::zero());
        trial.frame();
        let rest_cell = trial.image();
        assert_eq!(rest_cell.key, input.key);
        let rest_cell = rest_cell.image.unwrap();
        assert_eq!(rest_cell.binding, binding);
        assert_eq!((rest_cell.uv.x, rest_cell.uv.y), (0.0, 0.0));
        let rest_pixels = read(&mut trial, &binding);
        for y in [
            face.top() - inset,
            face.top() + inset * 0.75,
            face.bottom() - inset * 0.75,
            face.bottom() + inset,
        ] {
            let hover_alpha = pixel(&focused_pixels, x, y)[3];
            assert!(
                pixel(&rest_pixels, x, y)[3].abs_diff(hover_alpha) <= 1,
                "{scale}x row switch changed coverage at y={y}"
            );
        }
        trial.hover_at(center);
        trial.pointer_at(true, center);
        trial.frame();
        let pressed = trial.image().image.unwrap();
        assert_eq!(
            (pressed.binding.identity(), pressed.uv.x, pressed.uv.y),
            (binding.identity(), 0.5, 0.5)
        );
        assert_eq!(
            (pressed.coverage, pressed.uv.size()),
            (coverage, image.uv.size())
        );
        let pressed_pixels = read(&mut trial, &binding);
        let lower_y = face.top() + face.height * 0.79;
        assert!(pixel(&pressed_pixels, x, lower_y)[0] + 6 < pixel(&focused_pixels, x, lower_y)[0]);
        for x in [
            face.left() - inset,
            face.right() + inset,
            face.left() + inset * 0.75,
            face.right() - inset * 0.75,
        ] {
            let before = pixel(&focused_pixels, x, y);
            let after = pixel(&pressed_pixels, x, y);
            assert!(
                before[3].abs_diff(after[3]) <= 1,
                "{scale}x at x={x}: {before:?} -> {after:?}"
            );
        }
        trial.pointer_at(false, center);
        // Scope the edge scans to the material. The independent focus border
        // is proved by the routed keyboard/pointer fixture above.
        trial
            .router
            .clear_focus(&mut trial.tree, trial.domains.primary_state_mut());
        trial.frame();
        assert_eq!(trial.image().image.unwrap().uv.x, 0.0);
        assert_eq!(preparations(&trial), count);
        eprintln!(
            "ChampagneV4 fractional {scale}x: exact F/E, {translucent} edge samples, Hover/Pressed"
        );
    }

    target(&mut trial, 1.25);
    set_enabled(&mut trial, false);
    let disabled = trial.settle(false);
    let disabled_input = trial.image();
    assert_eq!(
        disabled_input.key.metrics().target_logical_extent,
        viewport.size()
    );
    assert_eq!(
        trial.request().source,
        SdfMaterialSource::ChampagneV4(ChampagneV4Basis::Disabled)
    );
    assert_eq!(disabled_input.image.unwrap().uv.x, 0.0);
    let count = clicks.get();
    trial.pointer_at(true, center);
    trial.pointer_at(false, center);
    enter_key(&mut trial, true, false);
    enter_key(&mut trial, false, false);
    trial.frame();
    assert_eq!(clicks.get(), count);
    assert_eq!(trial.image().image.unwrap().binding, disabled);
    assert_eq!(
        trial
            .domains
            .primary_state()
            .pointer_capture_manager
            .current_capture(PointerId::MOUSE),
        None
    );
    let pixels = read(&mut trial, &disabled);
    let p = pixels[(center.y * 1.25) as usize * 400 + ((face.right() - 20.0) * 1.25) as usize];
    assert!(p[0] > p[1] && p[1] > p[2] && p[3] == 255);
    let local = leaves(&trial.harness.local_fragment(trial.owner).unwrap());
    assert!(
        local
            .iter()
            .any(|node| matches!(node, SceneNode::Text(text) if
        text.text == "Stage" && text.color == Color::from_hex(0x0A1117).into()))
    );
}
